#!/usr/bin/env python3

from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime
import json
import os
import pathlib
import re
import shutil
import tempfile
import tomllib
from typing import Iterable

from scripts.ci.lib.artifact_arch import normalize_arch_alias
from scripts.ci.lib.release_artifacts import SHORT_SHA_PATTERN
from scripts.ci.lib.windows_filenames import validate_windows_filename


WINDOWS_CANONICAL_INSTALLER_RE = re.compile(
    r"(?P<name>.+?)_(?P<version>[^_]+)_windows_(?P<arch>x86_64|x64|amd64|arm64|aarch64)"
    rf"_setup(?P<nightly_suffix>_nightly_{SHORT_SHA_PATTERN})?\.exe$"
)
WINDOWS_LEGACY_INSTALLER_RE = re.compile(
    r"(?P<name>.+?)_(?P<version>.+?)_(?P<arch>x64|amd64|arm64|aarch64)-setup\.exe$"
)
LEGACY_NIGHTLY_BASE_VERSION_RE = re.compile(r"^[0-9A-Za-z.+]+(?:-[0-9A-Za-z.+]+)*$")

PORTABLE_README_NAME = "README-portable.txt"
PORTABLE_README_TEXT = """AstrBot Windows portable package

- This package does not support automatic in-app updates.
- Download a newer portable zip from the GitHub release page and apply manual updates by replacing this folder.
- Microsoft Edge WebView2 Runtime must already be installed on this Windows machine.
"""
TAURI_CONFIG_RELATIVE_PATH = pathlib.Path("src-tauri") / "tauri.conf.json"
CARGO_TOML_RELATIVE_PATH = pathlib.Path("src-tauri") / "Cargo.toml"
# These point to the source resource directories inside the repository checkout.
BACKEND_RESOURCE_RELATIVE_PATH = pathlib.Path("resources") / "backend"
WEBUI_RESOURCE_RELATIVE_PATH = pathlib.Path("resources") / "webui"
WINDOWS_CLEANUP_SCRIPT_RELATIVE_PATH = (
    pathlib.Path("src-tauri") / "windows" / "kill-backend-processes.ps1"
)
PORTABLE_RUNTIME_MARKER_RELATIVE_PATH = (
    pathlib.Path("src-tauri") / "windows" / "portable-runtime-marker.txt"
)


@dataclass(frozen=True)
class ProjectConfig:
    root: pathlib.Path
    product_name: str
    binary_name: str
    portable_marker_name: str
    backend_layout_relative_path: pathlib.Path
    webui_layout_relative_path: pathlib.Path


def normalize_arch(arch: str) -> str:
    return normalize_arch_alias(arch) or arch


def is_valid_nightly_date(date_value: str) -> bool:
    try:
        datetime.strptime(date_value, "%Y%m%d")
    except ValueError:
        return False
    return True


def resolve_project_root_from(start_path: pathlib.Path) -> pathlib.Path:
    candidate = start_path.resolve()
    if candidate.is_file():
        candidate = candidate.parent

    anchors = [TAURI_CONFIG_RELATIVE_PATH]
    for root in (candidate, *candidate.parents):
        if all((root / anchor).is_file() for anchor in anchors):
            return root

    raise FileNotFoundError(
        "Unable to locate project root from "
        f"{start_path}. Expected to find {TAURI_CONFIG_RELATIVE_PATH}."
    )


def resolve_project_root() -> pathlib.Path:
    return resolve_project_root_from(pathlib.Path(__file__))


def load_portable_runtime_marker(project_root: pathlib.Path) -> str:
    marker_path = project_root / PORTABLE_RUNTIME_MARKER_RELATIVE_PATH
    if not marker_path.is_file():
        raise FileNotFoundError(
            f"Portable runtime marker file not found: {marker_path}"
        )

    marker_name = marker_path.read_text(encoding="utf-8").strip()
    if not marker_name:
        raise ValueError(f"Portable runtime marker file is empty: {marker_path}")
    return marker_name


def resolve_bundle_resource_alias_from_tauri_config(
    project_root: pathlib.Path,
    tauri_config: dict,
    source_relative_path: pathlib.Path,
) -> pathlib.Path:
    # Keep validation rules aligned with src-tauri/build.rs::load_bundle_resource_alias.
    bundle_table = tauri_config.get("bundle")
    if not isinstance(bundle_table, dict):
        raise ValueError(f"Missing bundle object in {TAURI_CONFIG_RELATIVE_PATH}")

    resources_table = bundle_table.get("resources")
    if not isinstance(resources_table, dict):
        raise ValueError(
            f"Missing bundle.resources object in {TAURI_CONFIG_RELATIVE_PATH}"
        )

    tauri_config_dir = (project_root / TAURI_CONFIG_RELATIVE_PATH).parent.resolve()
    expected_source_path = (project_root / source_relative_path).resolve()
    expected_source_key = pathlib.PurePosixPath(
        os.path.relpath(expected_source_path, tauri_config_dir)
    ).as_posix()
    alias_text = resources_table.get(expected_source_key)
    if alias_text is None:
        raise ValueError(
            "Missing bundle.resources alias for "
            f"{expected_source_key} in {TAURI_CONFIG_RELATIVE_PATH}"
        )

    if not isinstance(alias_text, str):
        raise ValueError(
            "bundle.resources alias for "
            f"{expected_source_key} must be a string in {TAURI_CONFIG_RELATIVE_PATH}"
        )

    alias_path = pathlib.Path(alias_text.strip())
    if not alias_path.parts or alias_path.is_absolute():
        raise ValueError(
            "bundle.resources alias for "
            f"{expected_source_key} must be a relative path in "
            f"{TAURI_CONFIG_RELATIVE_PATH}: {alias_text}"
        )
    if any(part in (".", "..") for part in alias_path.parts):
        raise ValueError(
            "bundle.resources alias for "
            f"{expected_source_key} must be a relative path without traversal in "
            f"{TAURI_CONFIG_RELATIVE_PATH}: {alias_text}"
        )
    return alias_path


def load_project_config_from(start_path: pathlib.Path) -> ProjectConfig:
    project_root = resolve_project_root_from(start_path)
    tauri_config = load_tauri_config(project_root)
    product_name = resolve_product_name_from_tauri_config(tauri_config)
    binary_name = load_binary_name_from_cargo(project_root)
    portable_marker_name = load_portable_runtime_marker(project_root)
    backend_layout_relative_path = resolve_bundle_resource_alias_from_tauri_config(
        project_root,
        tauri_config,
        BACKEND_RESOURCE_RELATIVE_PATH,
    )
    webui_layout_relative_path = resolve_bundle_resource_alias_from_tauri_config(
        project_root,
        tauri_config,
        WEBUI_RESOURCE_RELATIVE_PATH,
    )
    return ProjectConfig(
        root=project_root,
        product_name=product_name,
        binary_name=binary_name,
        portable_marker_name=portable_marker_name,
        backend_layout_relative_path=backend_layout_relative_path,
        webui_layout_relative_path=webui_layout_relative_path,
    )


def normalize_legacy_nightly_version(version: str) -> tuple[str, str]:
    if "-nightly" not in version:
        return version, ""

    base_version, separator, nightly_part = version.partition("-nightly")
    if not separator or not LEGACY_NIGHTLY_BASE_VERSION_RE.fullmatch(base_version):
        return version, ""

    nightly_part = nightly_part.lstrip("._-")
    if not nightly_part:
        return base_version, ""

    parts = re.split(r"[._-]", nightly_part, maxsplit=2)
    if len(parts) != 2:
        return base_version, ""

    date_value, sha = parts[0], parts[1]
    if not is_valid_nightly_date(date_value):
        return base_version, ""
    if not re.fullmatch(SHORT_SHA_PATTERN, sha):
        return base_version, ""

    return base_version, f"_nightly_{sha}"


def load_project_config() -> ProjectConfig:
    return load_project_config_from(pathlib.Path(__file__))


def installer_to_portable_name(installer_name: str) -> str:
    canonical_match = WINDOWS_CANONICAL_INSTALLER_RE.fullmatch(installer_name)
    if canonical_match:
        name = canonical_match.group("name")
        version = canonical_match.group("version")
        arch = normalize_arch(canonical_match.group("arch"))
        nightly_suffix = canonical_match.group("nightly_suffix") or ""
        return f"{name}_{version}_windows_{arch}_portable{nightly_suffix}.zip"

    legacy_match = WINDOWS_LEGACY_INSTALLER_RE.fullmatch(installer_name)
    if legacy_match:
        name = legacy_match.group("name")
        version, nightly_suffix = normalize_legacy_nightly_version(
            legacy_match.group("version")
        )
        arch = normalize_arch(legacy_match.group("arch"))
        return f"{name}_{version}_windows_{arch}_portable{nightly_suffix}.zip"

    raise ValueError(
        "Unexpected Windows installer name: "
        f"{installer_name}. Expected a canonical installer like "
        "AstrBot_<version>_windows_<arch>_setup(.exe) or a legacy "
        "AstrBot_<version>_<arch>-setup.exe name."
    )


def is_installer_executable(path: pathlib.Path) -> bool:
    if not path.is_file() or path.suffix.lower() != ".exe":
        return False

    try:
        installer_to_portable_name(path.name)
    except ValueError:
        return False
    return True


def load_tauri_config(project_root: pathlib.Path) -> dict:
    config_path = project_root / TAURI_CONFIG_RELATIVE_PATH
    if not config_path.is_file():
        raise FileNotFoundError(f"Tauri config not found: {config_path}")
    return json.loads(config_path.read_text(encoding="utf-8"))


def load_binary_name_from_cargo(project_root: pathlib.Path) -> str:
    cargo_toml_path = project_root / CARGO_TOML_RELATIVE_PATH
    if not cargo_toml_path.is_file():
        raise FileNotFoundError(f"Cargo.toml not found: {cargo_toml_path}")

    with cargo_toml_path.open("rb") as handle:
        cargo_data = tomllib.load(handle)

    bins = cargo_data.get("bin")
    if isinstance(bins, list):
        for entry in bins:
            if isinstance(entry, dict):
                binary_name = str(entry.get("name", "")).strip()
                if binary_name:
                    return binary_name

    package_table = cargo_data.get("package")
    if not isinstance(package_table, dict):
        raise ValueError(f"Missing [package] in {cargo_toml_path}")

    binary_name = str(package_table.get("name", "")).strip()
    if not binary_name:
        raise ValueError(f"Missing [package].name in {cargo_toml_path}")

    return binary_name


def resolve_product_name_from_tauri_config(config: dict) -> str:
    product_name = str(config.get("productName", "")).strip()
    if not product_name:
        raise ValueError(f"Missing productName in {TAURI_CONFIG_RELATIVE_PATH}")
    if product_name.lower().endswith(".exe"):
        product_name = product_name[:-4].rstrip()
    if not product_name:
        raise ValueError(
            f"productName resolves to an empty executable name in {TAURI_CONFIG_RELATIVE_PATH}"
        )
    validate_windows_filename(product_name)
    return product_name


def resolve_product_name(project_root: pathlib.Path) -> str:
    return resolve_product_name_from_tauri_config(load_tauri_config(project_root))


def resolve_release_dir(bundle_dir: pathlib.Path) -> pathlib.Path:
    return bundle_dir.parent.parent


def resolve_main_executable_path(
    bundle_dir: pathlib.Path, project_config: ProjectConfig
) -> pathlib.Path:
    release_dir = resolve_release_dir(bundle_dir)
    main_executable_path = release_dir / f"{project_config.binary_name}.exe"
    if not main_executable_path.is_file():
        raise FileNotFoundError(f"Main executable not found: {main_executable_path}")
    return main_executable_path


def populate_portable_root(
    bundle_dir: pathlib.Path,
    destination_root: pathlib.Path,
    project_config: ProjectConfig,
) -> None:
    release_dir = resolve_release_dir(bundle_dir)
    main_executable_path = resolve_main_executable_path(bundle_dir, project_config)

    destination_root.mkdir(parents=True, exist_ok=True)
    shutil.copy2(
        main_executable_path,
        destination_root / f"{project_config.product_name}.exe",
    )

    webview_loader = release_dir / "WebView2Loader.dll"
    if webview_loader.is_file():
        shutil.copy2(webview_loader, destination_root / "WebView2Loader.dll")

    cleanup_script = project_config.root / WINDOWS_CLEANUP_SCRIPT_RELATIVE_PATH
    if cleanup_script.is_file():
        shutil.copy2(cleanup_script, destination_root / "kill-backend-processes.ps1")

    backend_src = project_config.root / BACKEND_RESOURCE_RELATIVE_PATH
    if not backend_src.is_dir():
        raise FileNotFoundError(f"Required directory not found: {backend_src}")
    shutil.copytree(
        backend_src, destination_root / project_config.backend_layout_relative_path
    )

    webui_src = project_config.root / WEBUI_RESOURCE_RELATIVE_PATH
    if not webui_src.is_dir():
        raise FileNotFoundError(f"Required directory not found: {webui_src}")
    shutil.copytree(
        webui_src, destination_root / project_config.webui_layout_relative_path
    )

    add_portable_runtime_files(destination_root, project_config)
    validate_portable_root(destination_root, project_config)


def add_portable_runtime_files(
    destination_root: pathlib.Path, project_config: ProjectConfig
) -> None:
    (destination_root / project_config.portable_marker_name).write_text(
        "", encoding="utf-8"
    )
    (destination_root / PORTABLE_README_NAME).write_text(
        PORTABLE_README_TEXT,
        encoding="utf-8",
    )


def validate_portable_root(
    destination_root: pathlib.Path, project_config: ProjectConfig | None = None
) -> None:
    if project_config is None:
        project_config = load_project_config()
    expected_paths = [
        destination_root / project_config.backend_layout_relative_path / "runtime-manifest.json",
        destination_root / project_config.webui_layout_relative_path / "index.html",
    ]
    missing = [
        str(path.relative_to(destination_root))
        for path in expected_paths
        if not path.is_file()
    ]
    top_level_exes = sorted(destination_root.glob("*.exe"))
    if not top_level_exes:
        missing.append("<top-level *.exe>")

    if missing:
        raise ValueError(
            "Portable package layout is missing expected files: " + ", ".join(missing)
        )


def iter_installer_paths(bundle_dir: pathlib.Path) -> Iterable[pathlib.Path]:
    return sorted(
        path for path in bundle_dir.glob("*.exe") if is_installer_executable(path)
    )


def package_installer(
    installer_path: pathlib.Path,
    output_dir: pathlib.Path,
    project_config: ProjectConfig,
) -> pathlib.Path:
    portable_name = installer_to_portable_name(installer_path.name)
    portable_stem = portable_name[: -len(".zip")]

    with tempfile.TemporaryDirectory(prefix="astrbot-portable-") as tmpdir:
        temp_root = pathlib.Path(tmpdir)
        staging_root = temp_root / "staging"
        archive_root = staging_root / portable_stem

        populate_portable_root(
            bundle_dir=installer_path.parent,
            destination_root=archive_root,
            project_config=project_config,
        )

        output_dir.mkdir(parents=True, exist_ok=True)
        archive_base = output_dir / portable_stem
        output_path = archive_base.with_suffix(".zip")
        if output_path.exists():
            output_path.unlink()

        created_archive = shutil.make_archive(
            str(archive_base),
            "zip",
            root_dir=staging_root,
            base_dir=portable_stem,
        )
        return pathlib.Path(created_archive)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build portable zip artifacts from Windows desktop release outputs."
    )
    parser.add_argument(
        "--bundle-dir",
        required=True,
        help="Directory containing Windows NSIS installer executables.",
    )
    parser.add_argument(
        "--output-dir",
        default="",
        help="Directory to write portable zip artifacts to. Defaults to --bundle-dir.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    bundle_dir = pathlib.Path(args.bundle_dir).resolve()
    if not bundle_dir.is_dir():
        raise SystemExit(f"Windows bundle directory not found: {bundle_dir}")

    output_dir = (
        pathlib.Path(args.output_dir).resolve() if args.output_dir else bundle_dir
    )
    installer_paths = list(iter_installer_paths(bundle_dir))
    if not installer_paths:
        raise SystemExit(f"No Windows installer executables found under: {bundle_dir}")

    project_config = load_project_config()
    for installer_path in installer_paths:
        archive_path = package_installer(installer_path, output_dir, project_config)
        print(f"[windows-portable] created: {archive_path.name}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
