#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))

from artifact_arch import normalize_arch_alias

WINDOWS_PATTERNS = (
    re.compile(
        r"(?P<name>.+?)_(?P<version>[^_]+)_windows_(?P<arch>[^_]+)(?:-setup|_setup(?:_nightly_[0-9a-fA-F]{8})?)\.exe$"
    ),
    re.compile(r"(?P<name>.+?)_(?P<version>.+?)_(?P<arch>x64|amd64|arm64)-setup\.exe$"),
)
MACOS_ARCHIVE_PATTERNS = (
    re.compile(
        r"(?P<name>.+?)_(?P<version>[^_]+)_macos_(?P<arch>[^_]+)(?:_nightly_[0-9a-fA-F]{8})?\.app\.tar\.gz$"
    ),
    re.compile(r"(?P<name>.+?)_(?P<version>.+?)_macos_(?P<arch>[^.]+)\.app\.tar\.gz$"),
    re.compile(
        r"(?P<name>.+?)_(?P<version>[^_]+)_macos_(?P<arch>[^_]+)(?:_nightly_[0-9a-fA-F]{8})?\.zip$"
    ),
    re.compile(r"(?P<name>.+?)_(?P<version>.+?)_macos_(?P<arch>[^.]+)\.zip$"),
)


def read_signature(path: Path) -> str:
    return path.read_text(encoding="utf-8").strip()


def asset_url(repo: str, tag: str, filename: str) -> str:
    return f"https://github.com/{repo}/releases/download/{tag}/{filename}"


def normalize_arch(arch: str, platform: str) -> str:
    normalized = normalize_arch_alias(arch)
    if normalized is None:
        raise ValueError(f"Unsupported {platform} arch: {arch}")
    return normalized


def platform_key_for_windows(arch: str) -> str:
    arch = normalize_arch(arch, "Windows")
    if arch == "amd64":
        return "windows-x86_64"
    if arch == "arm64":
        return "windows-aarch64"
    raise ValueError(f"Unsupported Windows arch: {arch}")


def platform_key_for_macos(arch: str) -> str:
    arch = normalize_arch(arch, "macOS")
    if arch == "amd64":
        return "darwin-x86_64"
    if arch == "arm64":
        return "darwin-aarch64"
    raise ValueError(f"Unsupported macOS arch: {arch}")


def match_any(filename: str, patterns: tuple[re.Pattern[str], ...]) -> re.Match[str] | None:
    for pattern in patterns:
        match = pattern.match(filename)
        if match:
            return match
    return None


def collect_platforms(root: Path, repo: str, tag: str) -> dict[str, dict[str, str]]:
    platforms: dict[str, dict[str, str]] = {}

    for sig_path in root.rglob("*.sig"):
        sig_name = sig_path.name
        if sig_name.endswith(".exe.sig"):
            exe_name = sig_name[:-4]
            match = match_any(exe_name, WINDOWS_PATTERNS)
            if not match:
                raise ValueError(
                    "Unexpected Windows artifact name: "
                    f"{exe_name}. Expected canonical or legacy NSIS installer format."
                )
            arch = match.group("arch")
            platform_key = platform_key_for_windows(arch)
            platforms[platform_key] = {
                "signature": read_signature(sig_path),
                "url": asset_url(repo, tag, exe_name),
            }
            continue

        if sig_name.endswith(".app.tar.gz.sig") or sig_name.endswith(".zip.sig"):
            archive_name = sig_name[:-4]
            match = match_any(archive_name, MACOS_ARCHIVE_PATTERNS)
            if not match:
                raise ValueError(
                    "Unexpected macOS artifact name: "
                    f"{archive_name}. Expected canonical or legacy macOS updater archive format."
                )
            platform_key = platform_key_for_macos(match.group("arch"))
            platforms[platform_key] = {
                "signature": read_signature(sig_path),
                "url": asset_url(repo, tag, archive_name),
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
