#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

# Matches current and legacy Windows NSIS signature basenames, e.g.
# AstrBot_4.19.2_windows_amd64_setup.exe
# AstrBot_4.19.2_x64-setup.exe
# AstrBot_4.19.2-nightly.20260307.abcd1234_x64-setup.exe
WINDOWS_RE = re.compile(
    r"(?P<name>.+?)_(?P<version>[^_]+)_(?:windows_)?(?P<arch>[A-Za-z0-9_]+)"
    r"(?:-setup|_setup)(?:_nightly_[0-9A-Fa-f]{7,40})?\.exe$"
)
# Matches current and legacy macOS updater archive basenames, e.g.
# AstrBot_4.19.2_macos_arm64.zip
# AstrBot_4.19.2_macos_arm64_nightly_abcd1234.zip
# AstrBot_4.19.2-nightly.20260307.abcd1234_macos_aarch64.zip
MACOS_RE = re.compile(
    r"(?P<name>.+?)_(?P<version>[^_]+)_(?:macos_)?(?P<arch>[A-Za-z0-9_]+)"
    r"(?:_nightly_[0-9A-Fa-f]{7,40})?\.zip$"
)
NIGHTLY_VERSION_RE = re.compile(
    r"^(?P<base>[0-9]+(?:\.[0-9]+){1,2})-nightly\.(?P<date>[0-9]{8})\.(?P<sha>[0-9A-Za-z-]+)$"
)

ARCH_ALIAS = {
    "x86_64": "amd64",
    "x64": "amd64",
    "amd64": "amd64",
    "aarch64": "arm64",
    "arm64": "arm64",
}


def read_signature(path: Path) -> str:
    return path.read_text(encoding="utf-8").strip()


def asset_url(repo: str, tag: str, filename: str) -> str:
    return f"https://github.com/{repo}/releases/download/{tag}/{filename}"


def normalize_arch(arch: str) -> str:
    return ARCH_ALIAS.get(arch, arch)


def platform_key_for_windows(arch: str) -> str:
    arch = normalize_arch(arch)
    if arch == "amd64":
        return "windows-x86_64"
    if arch == "arm64":
        return "windows-aarch64"
    raise ValueError(f"Unsupported Windows arch: {arch}")


def platform_key_for_macos(arch: str) -> str:
    arch = normalize_arch(arch)
    if arch == "amd64":
        return "darwin-x86_64"
    if arch == "arm64":
        return "darwin-aarch64"
    raise ValueError(f"Unsupported macOS arch: {arch}")


def derive_nightly_filename_suffix(version: str, channel: str) -> str:
    if channel != "nightly":
        return ""

    match = NIGHTLY_VERSION_RE.match(version)
    if not match:
        raise ValueError(
            "Nightly manifest version must match <base>-nightly.<YYYYMMDD>.<sha>, "
            f"got {version!r}"
        )
    return f"_nightly_{match.group('sha')[:8]}"


def canonical_windows_filename(name: str, arch: str, version: str, channel: str) -> str:
    base_version = derive_base_version(version)
    arch = normalize_arch(arch)
    suffix = derive_nightly_filename_suffix(version, channel)
    return f"{name}_{base_version}_windows_{arch}_setup{suffix}.exe"


def canonical_macos_filename(name: str, arch: str, version: str, channel: str) -> str:
    base_version = derive_base_version(version)
    arch = normalize_arch(arch)
    suffix = derive_nightly_filename_suffix(version, channel)
    return f"{name}_{base_version}_macos_{arch}{suffix}.zip"


def collect_platforms(
    root: Path, repo: str, tag: str, *, version: str, channel: str
) -> dict[str, dict[str, str]]:
    platforms: dict[str, dict[str, str]] = {}

    for sig_path in root.rglob("*.sig"):
        sig_name = sig_path.name
        if sig_name.endswith(".exe.sig"):
            source_name = sig_name[:-4]
            match = WINDOWS_RE.match(source_name)
            if not match:
                raise ValueError(
                    "Unexpected Windows artifact name: "
                    f"{source_name}. Expected current CI Windows signature naming."
                )
            arch = match.group("arch")
            exe_name = canonical_windows_filename(
                match.group("name"), arch, version, channel
            )
            platform_key = platform_key_for_windows(arch)
            platforms[platform_key] = {
                "signature": read_signature(sig_path),
                "url": asset_url(repo, tag, exe_name),
            }
            continue

        if sig_name.endswith(".zip.sig"):
            source_name = sig_name[:-4]
            match = MACOS_RE.match(source_name)
            if not match:
                raise ValueError(
                    "Unexpected macOS artifact name: "
                    f"{source_name}. Expected current CI macOS signature naming."
                )
            arch = match.group("arch")
            zip_name = canonical_macos_filename(
                match.group("name"), arch, version, channel
            )
            platform_key = platform_key_for_macos(arch)
            platforms[platform_key] = {
                "signature": read_signature(sig_path),
                "url": asset_url(repo, tag, zip_name),
            }
            continue

        print(
            f"[generate-tauri-latest-json] Ignoring unsupported signature file: {sig_name}",
            file=sys.stderr,
        )

    return platforms


def derive_base_version(version: str) -> str:
    match = NIGHTLY_VERSION_RE.match(version)
    if match:
        return match.group("base")
    return version


def build_payload(
    *,
    version: str,
    notes: str,
    channel: str,
    base_version: str,
    release_tag: str,
    platforms: dict[str, dict[str, str]],
) -> dict[str, object]:
    return {
        "version": version,
        "notes": notes,
        "channel": channel,
        "baseVersion": base_version,
        "releaseTag": release_tag,
        "platforms": platforms,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifacts-root", required=True)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--channel", required=True, choices=["stable", "nightly"])
    parser.add_argument("--output", required=True)
    parser.add_argument("--notes", default="")
    args = parser.parse_args()

    root = Path(args.artifacts_root)
    platforms = collect_platforms(
        root,
        args.repo,
        args.tag,
        version=args.version,
        channel=args.channel,
    )
    if not platforms:
        raise SystemExit("No updater signatures found under artifacts root")

    payload = build_payload(
        version=args.version,
        notes=args.notes,
        channel=args.channel,
        base_version=derive_base_version(args.version),
        release_tag=args.tag,
        platforms=platforms,
    )
    Path(args.output).write_text(
        json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
