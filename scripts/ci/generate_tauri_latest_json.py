#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

from scripts.ci.lib.artifact_arch import normalize_arch_alias
from scripts.ci.lib.nightly_version import NIGHTLY_CANONICAL_FORMAT, NIGHTLY_VERSION_RE
from scripts.ci.lib.release_artifacts import (
    MACOS_UPDATER_ARCHIVE_PATTERNS,
    WINDOWS_UPDATER_PATTERNS,
    match_any,
)

WINDOWS_PREFIX_ALIAS_RE = re.compile(
    r"(?P<name>.+?)_(?P<version>[^_]+)_windows_(?P<arch>x86_64|x64|amd64|arm64|aarch64)"
    r"(?:-setup|_setup)(?:_nightly_[0-9A-Fa-f]{7,40})?\.exe$"
)


def read_signature(path: Path) -> str:
    return path.read_text(encoding="utf-8").strip()


def asset_url(repo: str, tag: str, filename: str) -> str:
    return f"https://github.com/{repo}/releases/download/{tag}/{filename}"


def normalize_arch(arch: str) -> str:
    return normalize_arch_alias(arch) or arch


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


def derive_release_metadata(version: str, channel: str | None) -> tuple[str, str, str]:
    inferred_channel = "nightly" if "nightly" in version.lower() else "stable"
    effective_channel = channel or inferred_channel
    match = NIGHTLY_VERSION_RE.match(version)

    if effective_channel == "nightly":
        if not match:
            message = (
                f"Nightly manifest version must match {NIGHTLY_CANONICAL_FORMAT}, "
                f"got {version!r}"
            )
            if channel is None:
                raise ValueError(
                    f"Invalid nightly version {version!r}: expected format {NIGHTLY_CANONICAL_FORMAT}"
                )
            raise ValueError(message)
        return effective_channel, match.group("base"), f"_nightly_{match.group('sha')[:8]}"

    base_version = match.group("base") if match else version
    return effective_channel, base_version, ""


def canonical_windows_filename(name: str, arch: str, version: str, channel: str) -> str:
    _, base_version, nightly_suffix = derive_release_metadata(version, channel)
    arch = normalize_arch(arch)
    return f"{name}_{base_version}_windows_{arch}_setup{nightly_suffix}.exe"


def canonical_macos_filename(
    name: str,
    arch: str,
    version: str,
    channel: str,
    archive_ext: str,
) -> str:
    _, base_version, nightly_suffix = derive_release_metadata(version, channel)
    arch = normalize_arch(arch)
    return f"{name}_{base_version}_macos_{arch}{nightly_suffix}{archive_ext}"


def parse_windows_artifact_name(source_name: str) -> re.Match[str]:
    match = match_any(source_name, WINDOWS_UPDATER_PATTERNS)
    if match:
        return match
    match = WINDOWS_PREFIX_ALIAS_RE.match(source_name)
    if match:
        return match
    raise ValueError(
        "Unexpected Windows artifact name: "
        f"{source_name}. Expected format: "
        "<name>_<version>_windows_<arch>_setup.exe or legacy "
        "<name>_<version>_<arch>-setup.exe "
        "(nightly builds may append _nightly_<sha> before .exe)."
    )


def parse_macos_artifact_name(source_name: str) -> tuple[re.Match[str], str]:
    match = match_any(source_name, MACOS_UPDATER_ARCHIVE_PATTERNS)
    if not match:
        raise ValueError(
            "Unexpected macOS artifact name: "
            f"{source_name}. Expected format: "
            "<name>_<version>_macos_<arch>.zip or "
            "<name>_<version>_macos_<arch>.app.tar.gz "
            "(nightly builds may append _nightly_<sha> before the extension)."
        )
    archive_ext = ".app.tar.gz" if source_name.endswith(".app.tar.gz") else ".zip"
    return match, archive_ext


def add_platform(
    platforms: dict[str, dict[str, str]],
    platform_key: str,
    platform_label: str,
    artifact_name: str,
    signature_path: Path,
    repo: str,
    tag: str,
) -> None:
    if platform_key in platforms:
        raise ValueError(
            f"Duplicate {platform_label} artifact for platform {platform_key!r}: "
            f"{artifact_name}. Multiple artifacts for the same platform are not allowed."
        )

    platforms[platform_key] = {
        "signature": read_signature(signature_path),
        "url": asset_url(repo, tag, artifact_name),
    }


def collect_platforms(
    root: Path,
    repo: str,
    tag: str,
    *,
    version: str,
    channel: str,
) -> dict[str, dict[str, str]]:
    platforms: dict[str, dict[str, str]] = {}
    unsupported_signature_files: list[str] = []

    for sig_path in sorted(root.rglob("*.sig")):
        sig_name = sig_path.name
        if sig_name.endswith(".exe.sig"):
            source_name = sig_name[:-4]
            match = parse_windows_artifact_name(source_name)
            artifact_name = canonical_windows_filename(
                match.group("name"),
                match.group("arch"),
                version,
                channel,
            )
            add_platform(
                platforms,
                platform_key_for_windows(match.group("arch")),
                "Windows",
                artifact_name,
                sig_path,
                repo,
                tag,
            )
            continue

        if sig_name.endswith(".app.tar.gz.sig") or sig_name.endswith(".zip.sig"):
            source_name = sig_name[:-4]
            match, archive_ext = parse_macos_artifact_name(source_name)
            artifact_name = canonical_macos_filename(
                match.group("name"),
                match.group("arch"),
                version,
                channel,
                archive_ext,
            )
            add_platform(
                platforms,
                platform_key_for_macos(match.group("arch")),
                "macOS",
                artifact_name,
                sig_path,
                repo,
                tag,
            )
            continue

        unsupported_signature_files.append(sig_name)

    if unsupported_signature_files:
        joined = ", ".join(unsupported_signature_files)
        raise ValueError(
            "Unsupported updater signature files under artifacts root: "
            f"{joined}"
        )

    return platforms


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifacts-root", required=True)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--channel", choices=["stable", "nightly"])
    parser.add_argument("--output", required=True)
    parser.add_argument("--notes", default="")
    args = parser.parse_args()

    root = Path(args.artifacts_root)
    try:
        channel, base_version, _nightly_suffix = derive_release_metadata(
            args.version,
            args.channel,
        )
        platforms = collect_platforms(
            root,
            args.repo,
            args.tag,
            version=args.version,
            channel=channel,
        )
        if not platforms:
            raise ValueError("No updater signatures found under artifacts root")
    except ValueError as exc:
        raise SystemExit(str(exc)) from exc

    payload = {
        "version": args.version,
        "notes": args.notes,
        "channel": channel,
        "baseVersion": base_version,
        "releaseTag": args.tag,
        "platforms": platforms,
    }
    Path(args.output).write_text(
        json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
