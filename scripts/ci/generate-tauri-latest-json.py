#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


WINDOWS_RE = re.compile(r"(?P<name>.+?)-setup\.exe$")
MACOS_RE = re.compile(r"(?P<name>.+?)_(?P<version>[^_]+)_macos_(?P<arch>[^.]+)\.zip$")


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


def collect_platforms(root: Path, repo: str, tag: str) -> dict[str, dict[str, str]]:
    platforms: dict[str, dict[str, str]] = {}

    for sig_path in root.rglob("*.sig"):
        sig_name = sig_path.name
        if sig_name.endswith(".exe.sig"):
            exe_name = sig_name[:-4]
            arch = "arm64" if "arm64" in exe_name.lower() else "amd64"
            platform_key = platform_key_for_windows(arch)
            platforms[platform_key] = {
                "signature": read_signature(sig_path),
                "url": asset_url(repo, tag, exe_name),
            }
            continue

        if sig_name.endswith(".zip.sig"):
            zip_name = sig_name[:-4]
            match = MACOS_RE.match(zip_name)
            if match:
                platform_key = platform_key_for_macos(match.group("arch"))
                platforms[platform_key] = {
                    "signature": read_signature(sig_path),
                    "url": asset_url(repo, tag, zip_name),
                }

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
