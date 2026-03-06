#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

# Matches Windows NSIS installer assets normalized by the release workflow, e.g.
# AstrBot_4.19.2_windows_amd64-setup.exe
WINDOWS_RE = re.compile(
    r"(?P<name>.+?)_(?P<version>[^_]+)_windows_(?P<arch>[^.]+)-setup\.exe$"
)
# Matches macOS updater archives normalized by the release workflow, e.g.
# AstrBot_4.19.2_macos_arm64.zip
MACOS_RE = re.compile(r"(?P<name>.+?)_(?P<version>[^_]+)_macos_(?P<arch>[^.]+)\.zip$")
# Matches Linux AppImage updater assets normalized by the release workflow, e.g.
# AstrBot_4.19.2_linux_amd64.AppImage
LINUX_APPIMAGE_RE = re.compile(
    r"(?P<name>.+?)_(?P<version>[^_]+)_linux_(?P<arch>[^.]+)\.AppImage$"
)


def read_signature(path: Path) -> str:
    return path.read_text(encoding="utf-8").strip()


def asset_url(repo: str, tag: str, filename: str) -> str:
    return f"https://github.com/{repo}/releases/download/{tag}/{filename}"


def platform_key_for_windows(arch: str) -> str:
    if arch == "amd64":
        return "windows-x86_64"
    if arch == "arm64":
        return "windows-aarch64"
    raise ValueError(f"Unsupported Windows arch: {arch}")


def platform_key_for_macos(arch: str) -> str:
    if arch == "amd64":
        return "darwin-x86_64"
    if arch == "arm64":
        return "darwin-aarch64"
    raise ValueError(f"Unsupported macOS arch: {arch}")


def platform_key_for_linux_appimage(arch: str) -> str:
    normalized_arch = arch.lower()
    if normalized_arch in {"amd64", "x86_64"}:
        return "linux-x86_64"
    if normalized_arch in {"arm64", "aarch64"}:
        return "linux-aarch64"
    raise ValueError(f"Unsupported Linux AppImage arch: {arch}")


def collect_platforms(root: Path, repo: str, tag: str) -> dict[str, dict[str, str]]:
    platforms: dict[str, dict[str, str]] = {}

    for sig_path in root.rglob("*.sig"):
        sig_name = sig_path.name
        if sig_name.endswith(".exe.sig"):
            exe_name = sig_name[:-4]
            match = WINDOWS_RE.match(exe_name)
            if not match:
                raise ValueError(
                    "Unexpected Windows artifact name: "
                    f"{exe_name}. Expected format: <name>_<version>_windows_<arch>-setup.exe"
                )
            arch = match.group("arch")
            platform_key = platform_key_for_windows(arch)
            platforms[platform_key] = {
                "signature": read_signature(sig_path),
                "url": asset_url(repo, tag, exe_name),
            }
            continue

        if sig_name.endswith(".zip.sig"):
            zip_name = sig_name[:-4]
            match = MACOS_RE.match(zip_name)
            if not match:
                raise ValueError(
                    "Unexpected macOS artifact name: "
                    f"{zip_name}. Expected format: <name>_<version>_macos_<arch>.zip"
                )
            platform_key = platform_key_for_macos(match.group("arch"))
            platforms[platform_key] = {
                "signature": read_signature(sig_path),
                "url": asset_url(repo, tag, zip_name),
            }
            continue

        if sig_name.endswith(".AppImage.sig"):
            appimage_name = sig_name[:-4]
            match = LINUX_APPIMAGE_RE.match(appimage_name)
            if not match:
                raise ValueError(
                    "Unexpected Linux AppImage artifact name: "
                    f"{appimage_name}. Expected format: <name>_<version>_linux_<arch>.AppImage"
                )
            platform_key = platform_key_for_linux_appimage(match.group("arch"))
            platforms[platform_key] = {
                "signature": read_signature(sig_path),
                "url": asset_url(repo, tag, appimage_name),
            }
            continue

        print(
            f"[generate-tauri-latest-json] Ignoring unsupported signature file: {sig_name}",
            file=sys.stderr,
        )

    return platforms


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifacts-root", required=True)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--notes", default="")
    args = parser.parse_args()

    root = Path(args.artifacts_root)
    platforms = collect_platforms(root, args.repo, args.tag)
    if not platforms:
        raise SystemExit("No updater signatures found under artifacts root")

    payload = {
        "version": args.version,
        "notes": args.notes,
        "platforms": platforms,
    }
    Path(args.output).write_text(
        json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
