#!/usr/bin/env python3

from __future__ import annotations

import argparse
import os
import pathlib
import re
import shutil
import subprocess
import tempfile
from typing import Iterable

from scripts.ci.lib.artifact_arch import normalize_arch_alias
from scripts.ci.lib.release_artifacts import WINDOWS_UPDATER_PATTERNS, match_any


WINDOWS_CANONICAL_INSTALLER_RE = re.compile(
    r"(?P<name>.+?)_(?P<version>[^_]+)_windows_(?P<arch>x86_64|x64|amd64|arm64|aarch64)"
    r"_setup(?P<nightly_suffix>_nightly_[0-9A-Fa-f]{7,40})?\.exe$"
)

PORTABLE_MARKER_NAME = "portable.flag"
PORTABLE_README_NAME = "README-portable.txt"
PORTABLE_README_TEXT = """AstrBot Windows portable package

- This package does not support automatic in-app updates.
- Download a newer portable zip from the GitHub release page and apply manual updates by replacing this folder.
- Microsoft Edge WebView2 Runtime must already be installed on this Windows machine.
"""


def normalize_arch(arch: str) -> str:
    return normalize_arch_alias(arch) or arch


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


def find_nsis_payload_archive(extracted_installer_root: pathlib.Path) -> pathlib.Path:
    archives = sorted(
        path for path in extracted_installer_root.rglob("*.7z") if path.is_file()
    )
    if not archives:
        raise FileNotFoundError(
            f"No embedded .7z payload archive found under {extracted_installer_root}"
        )

    preferred = [path for path in archives if path.name.lower().startswith("app-")]
    candidates = preferred or archives
    if len(candidates) != 1:
        raise RuntimeError(
            "Expected exactly one NSIS payload archive, found: "
            + ", ".join(path.name for path in candidates)
        )
    return candidates[0]


def select_payload_root(extracted_payload_root: pathlib.Path) -> pathlib.Path:
    children = sorted(extracted_payload_root.iterdir())
    if len(children) == 1 and children[0].is_dir():
        return children[0]
    return extracted_payload_root


def resolve_7zip_executable() -> str:
    explicit = os.environ.get("SEVEN_ZIP_EXE", "").strip()
    if explicit:
        explicit_path = pathlib.Path(explicit)
        if explicit_path.exists():
            return str(explicit_path)

    for candidate in ("7z", "7z.exe"):
        resolved = shutil.which(candidate)
        if resolved:
            return resolved

    for candidate in (
        pathlib.Path("C:/Program Files/7-Zip/7z.exe"),
        pathlib.Path("C:/Program Files (x86)/7-Zip/7z.exe"),
    ):
        if candidate.exists():
            return str(candidate)

    raise FileNotFoundError(
        "Unable to locate 7z. Set SEVEN_ZIP_EXE or ensure 7-Zip is installed on PATH."
    )


def extract_archive(
    archive_path: pathlib.Path, output_dir: pathlib.Path, seven_zip: str
) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [seven_zip, "x", "-y", f"-o{output_dir}", str(archive_path)],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )


def copy_tree_contents(
    source_root: pathlib.Path, destination_root: pathlib.Path
) -> None:
    destination_root.mkdir(parents=True, exist_ok=True)
    for child in source_root.iterdir():
        target = destination_root / child.name
        if child.is_dir():
            shutil.copytree(child, target)
        else:
            shutil.copy2(child, target)


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
    return sorted(path for path in bundle_dir.glob("*.exe") if path.is_file())


def package_installer(
    installer_path: pathlib.Path, output_dir: pathlib.Path, seven_zip: str
) -> pathlib.Path:
    portable_name = installer_to_portable_name(installer_path.name)
    portable_stem = portable_name[: -len(".zip")]

    with tempfile.TemporaryDirectory(prefix="astrbot-portable-") as tmpdir:
        temp_root = pathlib.Path(tmpdir)
        extracted_installer_root = temp_root / "installer"
        extracted_payload_root = temp_root / "payload"
        staging_root = temp_root / "staging"
        archive_root = staging_root / portable_stem

        extract_archive(installer_path, extracted_installer_root, seven_zip)
        payload_archive = find_nsis_payload_archive(extracted_installer_root)
        extract_archive(payload_archive, extracted_payload_root, seven_zip)
        source_root = select_payload_root(extracted_payload_root)
        copy_tree_contents(source_root, archive_root)
        add_portable_runtime_files(archive_root)
        validate_portable_root(archive_root)

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
        description="Build portable zip artifacts from Windows NSIS installers."
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

    seven_zip = resolve_7zip_executable()
    for installer_path in installer_paths:
        archive_path = package_installer(installer_path, output_dir, seven_zip)
        print(f"[windows-portable] created: {archive_path.name}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
