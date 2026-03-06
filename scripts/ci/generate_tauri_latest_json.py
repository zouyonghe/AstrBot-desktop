#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
from pathlib import Path

from .lib.artifact_arch import normalize_arch_alias
from .lib.release_artifacts import (
    MACOS_UPDATER_ARCHIVE_PATTERNS,
    WINDOWS_UPDATER_PATTERNS,
    ReleaseArtifactError,
    match_any,
)


def read_signature(path: Path) -> str:
    return path.read_text(encoding="utf-8").strip()


def asset_url(repo: str, tag: str, filename: str) -> str:
    return f"https://github.com/{repo}/releases/download/{tag}/{filename}"


def normalize_arch(arch: str, platform: str) -> str:
    normalized = normalize_arch_alias(arch)
    if normalized is None:
        raise ReleaseArtifactError(f"Unsupported {platform} arch: {arch}")
    return normalized


def platform_key_for_windows(arch: str) -> str:
    arch = normalize_arch(arch, "Windows")
    if arch == "amd64":
        return "windows-x86_64"
    if arch == "arm64":
        return "windows-aarch64"
    raise ReleaseArtifactError(f"Unsupported Windows arch: {arch}")


def platform_key_for_macos(arch: str) -> str:
    arch = normalize_arch(arch, "macOS")
    if arch == "amd64":
        return "darwin-x86_64"
    if arch == "arm64":
        return "darwin-aarch64"
    raise ReleaseArtifactError(f"Unsupported macOS arch: {arch}")


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
        raise ReleaseArtifactError(
            f"Duplicate {platform_label} artifact for platform {platform_key!r}: "
            f"{artifact_name}. Multiple artifacts for the same platform are not allowed."
        )

    platforms[platform_key] = {
        "signature": read_signature(signature_path),
        "url": asset_url(repo, tag, artifact_name),
    }


def collect_platforms(root: Path, repo: str, tag: str) -> dict[str, dict[str, str]]:
    platforms: dict[str, dict[str, str]] = {}
    unsupported_signature_files: list[str] = []

    for sig_path in sorted(root.rglob("*.sig")):
        sig_name = sig_path.name
        if sig_name.endswith(".exe.sig"):
            exe_name = sig_name[:-4]
            match = match_any(exe_name, WINDOWS_UPDATER_PATTERNS)
            if not match:
                raise ReleaseArtifactError(
                    "Unexpected Windows artifact name: "
                    f"{exe_name}. Expected canonical or legacy NSIS installer format."
                )
            add_platform(
                platforms,
                platform_key_for_windows(match.group("arch")),
                "Windows",
                exe_name,
                sig_path,
                repo,
                tag,
            )
            continue

        if sig_name.endswith(".app.tar.gz.sig") or sig_name.endswith(".zip.sig"):
            archive_name = sig_name[:-4]
            match = match_any(archive_name, MACOS_UPDATER_ARCHIVE_PATTERNS)
            if not match:
                raise ReleaseArtifactError(
                    "Unexpected macOS artifact name: "
                    f"{archive_name}. Expected canonical or legacy macOS updater archive format."
                )
            add_platform(
                platforms,
                platform_key_for_macos(match.group("arch")),
                "macOS",
                archive_name,
                sig_path,
                repo,
                tag,
            )
            continue

        unsupported_signature_files.append(sig_name)

    if unsupported_signature_files:
        joined = ", ".join(unsupported_signature_files)
        raise ReleaseArtifactError(
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
    parser.add_argument("--output", required=True)
    parser.add_argument("--notes", default="")
    args = parser.parse_args()

    root = Path(args.artifacts_root)
    try:
        platforms = collect_platforms(root, args.repo, args.tag)
        if not platforms:
            raise ReleaseArtifactError("No updater signatures found under artifacts root")
    except ReleaseArtifactError as exc:
        raise SystemExit(str(exc)) from exc

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
