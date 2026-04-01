#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import pathlib
import re
import shutil
import tempfile
from typing import Iterable

from scripts.ci.lib.artifact_arch import normalize_arch_alias
from scripts.ci.lib.release_artifacts import (
    SHORT_SHA_PATTERN,
    WINDOWS_UPDATER_PATTERNS,
    match_any,
)


WINDOWS_CANONICAL_INSTALLER_RE = re.compile(
    r"(?P<name>.+?)_(?P<version>[^_]+)_windows_(?P<arch>x86_64|x64|amd64|arm64|aarch64)"
    rf"_setup(?P<nightly_suffix>_nightly_{SHORT_SHA_PATTERN})?\.exe$"
)

PORTABLE_MARKER_NAME = "portable.flag"
PORTABLE_README_NAME = "README-portable.txt"
PORTABLE_README_TEXT = """AstrBot Windows portable package

- This package does not support automatic in-app updates.
- Download a newer portable zip from the GitHub release page and apply manual updates by replacing this folder.
- Microsoft Edge WebView2 Runtime must already be installed on this Windows machine.
"""
TAURI_CONFIG_RELATIVE_PATH = pathlib.Path("src-tauri") / "tauri.conf.json"
BACKEND_RESOURCE_RELATIVE_PATH = pathlib.Path("resources") / "backend"
WEBUI_RESOURCE_RELATIVE_PATH = pathlib.Path("resources") / "webui"
WINDOWS_CLEANUP_SCRIPT_RELATIVE_PATH = (
    pathlib.Path("src-tauri") / "windows" / "kill-backend-processes.ps1"
)


def normalize_arch(arch: str) -> str:
    return normalize_arch_alias(arch) or arch


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


def installer_to_portable_name(installer_name: str) -> str:
    canonical_match = WINDOWS_CANONICAL_INSTALLER_RE.fullmatch(installer_name)
    if canonical_match:
        name = canonical_match.group("name")
        version = canonical_match.group("version")
        arch = normalize_arch(canonical_match.group("arch"))
        nightly_suffix = canonical_match.group("nightly_suffix") or ""
        return f"{name}_{version}_windows_{arch}_portable{nightly_suffix}.zip"

    legacy_match = match_any(installer_name, WINDOWS_UPDATER_PATTERNS)
    if legacy_match:
        name = legacy_match.group("name")
        version = legacy_match.group("version")
        arch = normalize_arch(legacy_match.group("arch"))
        return f"{name}_{version}_windows_{arch}_portable.zip"

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


def resolve_product_name(project_root: pathlib.Path) -> str:
    config = load_tauri_config(project_root)
    product_name = str(config.get("productName", "")).strip()
    if not product_name:
        raise ValueError(f"Missing productName in {TAURI_CONFIG_RELATIVE_PATH}")
    return product_name


def resolve_release_dir(bundle_dir: pathlib.Path) -> pathlib.Path:
    return bundle_dir.parent.parent


def resolve_main_executable_path(
    bundle_dir: pathlib.Path, project_root: pathlib.Path
) -> pathlib.Path:
    release_dir = resolve_release_dir(bundle_dir)
    main_executable_path = release_dir / f"{resolve_product_name(project_root)}.exe"
    if not main_executable_path.is_file():
        raise FileNotFoundError(f"Main executable not found: {main_executable_path}")
    return main_executable_path


def copy_required_directory(
    source_root: pathlib.Path, destination_root: pathlib.Path
) -> None:
    if not source_root.is_dir():
        raise FileNotFoundError(f"Required directory not found: {source_root}")
    shutil.copytree(source_root, destination_root)


def copy_optional_file(
    source_path: pathlib.Path, destination_path: pathlib.Path
) -> None:
    if source_path.is_file():
        destination_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_path, destination_path)


def populate_portable_root(
    bundle_dir: pathlib.Path,
    destination_root: pathlib.Path,
    project_root: pathlib.Path,
) -> None:
    release_dir = resolve_release_dir(bundle_dir)
    main_executable_path = resolve_main_executable_path(bundle_dir, project_root)

    destination_root.mkdir(parents=True, exist_ok=True)
    shutil.copy2(main_executable_path, destination_root / main_executable_path.name)

    copy_optional_file(
        release_dir / "WebView2Loader.dll",
        destination_root / "WebView2Loader.dll",
    )
    copy_optional_file(
        project_root / WINDOWS_CLEANUP_SCRIPT_RELATIVE_PATH,
        destination_root / "kill-backend-processes.ps1",
    )

    resources_root = destination_root / "resources"
    copy_required_directory(
        project_root / BACKEND_RESOURCE_RELATIVE_PATH,
        resources_root / "backend",
    )
    copy_required_directory(
        project_root / WEBUI_RESOURCE_RELATIVE_PATH,
        resources_root / "webui",
    )

    add_portable_runtime_files(destination_root)
    validate_portable_root(destination_root)


def add_portable_runtime_files(destination_root: pathlib.Path) -> None:
    (destination_root / PORTABLE_MARKER_NAME).write_text("", encoding="utf-8")
    (destination_root / PORTABLE_README_NAME).write_text(
        PORTABLE_README_TEXT,
        encoding="utf-8",
    )


def validate_portable_root(destination_root: pathlib.Path) -> None:
    expected_paths = [
        destination_root / "resources" / "backend" / "runtime-manifest.json",
        destination_root / "resources" / "webui" / "index.html",
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
    installer_path: pathlib.Path, output_dir: pathlib.Path, project_root: pathlib.Path
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
            project_root=project_root,
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

    project_root = resolve_project_root()
    for installer_path in installer_paths:
        archive_path = package_installer(installer_path, output_dir, project_root)
        print(f"[windows-portable] created: {archive_path.name}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
