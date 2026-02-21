#!/usr/bin/env python3

from __future__ import annotations

import argparse
import pathlib
import re
import sys

NIGHTLY_DATE_PATTERN = re.compile(r"(?:-|_)nightly[._-][0-9]{8}[._-][0-9a-fA-F]{7,40}")
NIGHTLY_HASH_PATTERN = re.compile(r"(?:-|_)nightly[-_][0-9a-fA-F]{7,40}")

ARCH_ALIAS = {
    "x86_64": "amd64",
    "x64": "amd64",
    "amd64": "amd64",
    "aarch64": "arm64",
    "arm64": "arm64",
}


def normalize_arch(arch: str) -> str:
    return ARCH_ALIAS.get(arch, arch)


def canonicalize_stem(stem: str, ext: str) -> tuple[str, bool]:
    # RPM: AstrBot-<ver>-<rel>.<arch> or AstrBot_<ver>_<arch>
    if ext == ".rpm":
        match = re.fullmatch(
            r"AstrBot-([0-9A-Za-z.+-]+)-\d+\.(x86_64|aarch64|amd64|arm64)",
            stem,
        )
        if match:
            version, arch = match.groups()
            return f"AstrBot_{version}_{normalize_arch(arch)}", True
        match = re.fullmatch(
            r"AstrBot_([0-9A-Za-z.+-]+)_(x86_64|aarch64|x64|amd64|arm64)",
            stem,
        )
        if match:
            version, arch = match.groups()
            return f"AstrBot_{version}_{normalize_arch(arch)}", True

    # DEB: AstrBot_<ver>_<arch>
    if ext == ".deb":
        match = re.fullmatch(
            r"AstrBot_([0-9A-Za-z.+-]+)_(x86_64|aarch64|x64|amd64|arm64)",
            stem,
        )
        if match:
            version, arch = match.groups()
            return f"AstrBot_{version}_{normalize_arch(arch)}", True

    # EXE: AstrBot_<ver>_<arch>-setup or _setup
    if ext == ".exe":
        match = re.fullmatch(
            r"AstrBot_([0-9A-Za-z.+-]+)_(x86_64|aarch64|x64|amd64|arm64)(?:-setup|_setup)",
            stem,
        )
        if match:
            version, arch = match.groups()
            return f"AstrBot_{version}_{normalize_arch(arch)}_setup", True

    # MSI: AstrBot_<ver>_<arch>_<locale>
    if ext == ".msi":
        match = re.fullmatch(
            r"AstrBot_([0-9A-Za-z.+-]+)_(x86_64|aarch64|x64|amd64|arm64)_([A-Za-z0-9-]+)",
            stem,
        )
        if match:
            version, arch, locale = match.groups()
            return f"AstrBot_{version}_{normalize_arch(arch)}_{locale}", True

    # macOS zip: AstrBot_<ver>_macos_<arch>
    if ext == ".zip":
        match = re.fullmatch(
            r"AstrBot_([0-9A-Za-z.+-]+)_macos_(x86_64|aarch64|x64|amd64|arm64)",
            stem,
        )
        if match:
            version, arch = match.groups()
            return f"AstrBot_{version}_macos_{normalize_arch(arch)}", True

    return stem, False


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Normalize AstrBot release artifact filenames before publishing."
    )
    parser.add_argument("--root", required=True, help="Directory containing release artifacts.")
    parser.add_argument(
        "--build-mode",
        default="",
        help="Build mode (nightly/tag-poll/etc.). Nightly adds _nightly_<sha8> suffix.",
    )
    parser.add_argument(
        "--source-git-ref",
        default="",
        help="Source git ref used to derive nightly short SHA suffix.",
    )
    parser.add_argument(
        "--strict-unmatched",
        action="store_true",
        help="Fail when an artifact filename does not match any canonicalization pattern.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = pathlib.Path(args.root)
    if not root.exists():
        raise RuntimeError(f"artifact directory does not exist: {root}")

    build_mode = args.build_mode.strip().lower()
    source_git_ref = args.source_git_ref.strip()
    is_nightly = build_mode == "nightly"
    short_sha = source_git_ref[:8]
    nightly_suffix = f"_nightly_{short_sha}" if is_nightly else ""

    if is_nightly and len(short_sha) < 7:
        raise RuntimeError(
            "nightly build requires --source-git-ref commit-like value to derive short sha suffix"
        )

    unmatched_messages: list[str] = []
    renamed_count = 0

    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        stem = path.stem
        ext = path.suffix

        normalized_stem = NIGHTLY_DATE_PATTERN.sub("", stem)
        normalized_stem = NIGHTLY_HASH_PATTERN.sub("", normalized_stem)
        normalized_stem, matched = canonicalize_stem(normalized_stem, ext)

        if not matched:
            unmatched_messages.append(
                f"[normalize-artifacts] unmatched naming pattern, kept stem: {path.name}"
            )

        if nightly_suffix and not normalized_stem.endswith(nightly_suffix):
            normalized_stem = f"{normalized_stem}{nightly_suffix}"

        new_path = path.with_name(f"{normalized_stem}{ext}")
        if new_path == path:
            continue
        if new_path.exists():
            raise RuntimeError(f"target already exists: {new_path}")

        path.rename(new_path)
        renamed_count += 1
        print(f"[normalize-artifacts] renamed: {path.name} -> {new_path.name}")

    if unmatched_messages:
        if args.strict_unmatched:
            for message in unmatched_messages:
                print(message, file=sys.stderr)
            raise RuntimeError(
                f"{len(unmatched_messages)} artifact(s) did not match canonical naming patterns"
            )
        for message in unmatched_messages:
            print(f"::warning::{message}")

    print(f"[normalize-artifacts] completed: renamed={renamed_count}, unmatched={len(unmatched_messages)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
