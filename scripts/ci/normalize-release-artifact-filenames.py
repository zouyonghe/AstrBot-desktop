#!/usr/bin/env python3

from __future__ import annotations

import argparse
import pathlib
import re
import sys

NIGHTLY_DATE_PATTERN = re.compile(r"(?:-|_)nightly[._-][0-9]{8}[._-][0-9a-fA-F]{7,40}")
NIGHTLY_HASH_PATTERN = re.compile(r"(?:-|_)nightly[-_][0-9a-fA-F]{7,40}")
HEX_SHA_PATTERN = re.compile(r"^[0-9a-fA-F]{8,64}$")
ARTIFACT_EXTENSIONS: set[str] = {
    ".rpm",
    ".deb",
    ".exe",
    ".msi",
    ".zip",
}

ARCH_ALIAS = {
    "x86_64": "amd64",
    "x64": "amd64",
    "amd64": "amd64",
    "aarch64": "arm64",
    "arm64": "arm64",
}
WARNED_UNKNOWN_ARCHES: set[str] = set()


def normalize_arch(arch: str) -> str:
    normalized = ARCH_ALIAS.get(arch)
    if normalized is not None:
        return normalized
    if arch not in WARNED_UNKNOWN_ARCHES:
        WARNED_UNKNOWN_ARCHES.add(arch)
        print(
            f"::warning::[normalize-artifacts] unknown architecture alias '{arch}', keeping as-is"
        )
    return arch


def resolve_nightly_source_sha(source_git_ref: str) -> tuple[str, bool]:
    """Resolve a commit-like SHA from source_git_ref.

    Returns (sha, normalized_from_path_component).
    """
    candidate = source_git_ref.strip()
    if HEX_SHA_PATTERN.fullmatch(candidate):
        return candidate, False

    tail = candidate.rsplit("/", 1)[-1]
    if HEX_SHA_PATTERN.fullmatch(tail):
        return tail, True

    raise RuntimeError(
        "nightly build requires --source-git-ref to be a hex commit SHA (8-64 chars), "
        "or to end with one (for example: origin/<sha>)"
    )


def should_normalize_file(path: pathlib.Path) -> bool:
    ext = path.suffix.lower()
    if ext not in ARTIFACT_EXTENSIONS:
        return False
    return path.stem.startswith("AstrBot_") or path.stem.startswith("AstrBot-")


def canonicalize_stem(stem: str, ext: str) -> tuple[str, bool]:
    # RPM: AstrBot-<ver>-<rel>.<arch> or AstrBot_<ver>_<arch>
    if ext == ".rpm":
        match = re.fullmatch(
            r"AstrBot-([0-9A-Za-z.+-]+)-\d+\.([A-Za-z0-9_]+)",
            stem,
        )
        if match:
            version, arch = match.groups()
            return f"AstrBot_{version}_{normalize_arch(arch)}", True
        match = re.fullmatch(
            r"AstrBot_([0-9A-Za-z.+-]+)_([A-Za-z0-9_]+)",
            stem,
        )
        if match:
            version, arch = match.groups()
            return f"AstrBot_{version}_{normalize_arch(arch)}", True

    # DEB: AstrBot_<ver>_<arch>
    if ext == ".deb":
        match = re.fullmatch(
            r"AstrBot_([0-9A-Za-z.+-]+)_([A-Za-z0-9_]+)",
            stem,
        )
        if match:
            version, arch = match.groups()
            return f"AstrBot_{version}_{normalize_arch(arch)}", True

    # EXE: AstrBot_<ver>_<arch>-setup or _setup
    if ext == ".exe":
        match = re.fullmatch(
            r"AstrBot_([0-9A-Za-z.+-]+)_([A-Za-z0-9_]+)(?:-setup|_setup)",
            stem,
        )
        if match:
            version, arch = match.groups()
            return f"AstrBot_{version}_{normalize_arch(arch)}_setup", True

    # MSI: AstrBot_<ver>_<arch>_<locale>
    if ext == ".msi":
        match = re.fullmatch(
            r"AstrBot_([0-9A-Za-z.+-]+)_([A-Za-z0-9_]+)_([A-Za-z0-9-]+)",
            stem,
        )
        if match:
            version, arch, locale = match.groups()
            return f"AstrBot_{version}_{normalize_arch(arch)}_{locale}", True

    # macOS zip: AstrBot_<ver>_macos_<arch>
    if ext == ".zip":
        match = re.fullmatch(
            r"AstrBot_([0-9A-Za-z.+-]+)_macos_([A-Za-z0-9_]+)",
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
        help="Source git ref used to derive nightly short SHA suffix (nightly requires hex SHA, 8-64 chars).",
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
    resolved_source_sha = ""
    normalized_sha_from_path_component = False
    if is_nightly:
        resolved_source_sha, normalized_sha_from_path_component = resolve_nightly_source_sha(
            source_git_ref
        )
    short_sha = resolved_source_sha[:8]
    nightly_suffix = f"_nightly_{short_sha}" if is_nightly else ""

    if normalized_sha_from_path_component:
        print(
            "::warning::[normalize-artifacts] nightly source_git_ref was not a bare SHA; "
            f"using trailing SHA component '{resolved_source_sha}'."
        )

    unmatched_messages: list[str] = []
    renamed_count = 0
    skipped_count = 0

    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        if not should_normalize_file(path):
            skipped_count += 1
            continue

        stem = path.stem
        ext = path.suffix.lower()

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

    print(
        f"[normalize-artifacts] completed: renamed={renamed_count}, "
        f"unmatched={len(unmatched_messages)}, skipped_non_target={skipped_count}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
