#!/usr/bin/env python3

from __future__ import annotations

import argparse
import pathlib
import re
import sys

from .lib.artifact_arch import normalize_arch_alias
from .lib.release_artifacts import (
    ARCH_PATTERN,
    ARTIFACT_EXTENSIONS,
    LOCALE_PATTERN,
    MACOS_CANONICAL_ARTIFACT_STEM_PATTERN,
    SHORT_SHA_PATTERN,
    VERSION_PATTERN,
    WINDOWS_ARTIFACT_STEM_PATTERN_FRAGMENT,
)

NIGHTLY_DATE_PATTERN = re.compile(r"(?:-|_)nightly[._-][0-9]{8}[._-][0-9a-fA-F]{7,40}")
NIGHTLY_HASH_PATTERN = re.compile(r"(?:-|_)nightly[-_][0-9a-fA-F]{7,40}")
HEX_SHA_PATTERN = re.compile(r"^[0-9a-fA-F]{8,64}$")

# Intentionally match both legacy names (`AstrBot_<version>_<arch>`) and
# canonical names (`AstrBot_<version>_linux_<arch>`).
LINUX_ARTIFACT_STEM_PATTERN = re.compile(
    rf"^AstrBot_(?P<version>{VERSION_PATTERN})_(?:linux_)?(?P<arch>{ARCH_PATTERN})$"
)
LINUX_CANONICAL_RULE: tuple[re.Pattern[str], str] = (
    LINUX_ARTIFACT_STEM_PATTERN,
    "AstrBot_{version}_linux_{arch}",
)
CANONICALIZE_RULES: dict[str, tuple[tuple[re.Pattern[str], str], ...]] = {
    ".AppImage": (LINUX_CANONICAL_RULE,),
    ".rpm": (
        (
            re.compile(
                rf"^AstrBot-(?P<version>{VERSION_PATTERN})-\d+\.(?P<arch>{ARCH_PATTERN})$"
            ),
            "AstrBot_{version}_linux_{arch}",
        ),
        LINUX_CANONICAL_RULE,
    ),
    ".deb": (LINUX_CANONICAL_RULE,),
    ".exe": (
        (
            re.compile(rf"{WINDOWS_ARTIFACT_STEM_PATTERN_FRAGMENT}(?:-setup|_setup)$"),
            "AstrBot_{version}_windows_{arch}_setup",
        ),
    ),
    ".msi": (
        (
            re.compile(
                rf"{WINDOWS_ARTIFACT_STEM_PATTERN_FRAGMENT}_(?P<locale>{LOCALE_PATTERN})$"
            ),
            "AstrBot_{version}_windows_{arch}_{locale}",
        ),
    ),
    ".zip": (
        (
            MACOS_CANONICAL_ARTIFACT_STEM_PATTERN,
            "AstrBot_{version}_macos_{arch}",
        ),
        (
            re.compile(
                rf"{WINDOWS_ARTIFACT_STEM_PATTERN_FRAGMENT}(?:-portable|_portable)(?P<nightly_suffix>_nightly_{SHORT_SHA_PATTERN})?$"
            ),
            "AstrBot_{version}_windows_{arch}_portable{nightly_suffix}",
        ),
    ),
    ".app.tar.gz": (
        (
            MACOS_CANONICAL_ARTIFACT_STEM_PATTERN,
            "AstrBot_{version}_macos_{arch}",
        ),
    ),
}


def normalize_arch(arch: str, warned_unknown_arches: set[str]) -> str:
    normalized = normalize_arch_alias(arch)
    if normalized is not None:
        return normalized
    if arch not in warned_unknown_arches:
        warned_unknown_arches.add(arch)
        print(
            f"::warning::[normalize-artifacts] unknown architecture alias '{arch}', keeping as-is"
        )
    return arch


def resolve_nightly_source_sha(source_git_ref: str) -> str:
    """Resolve a commit-like SHA from source_git_ref."""
    candidate = source_git_ref.strip()
    if not candidate:
        raise RuntimeError(
            "nightly build requires a non-empty --source-git-ref commit SHA "
            "(8-64 hex chars), but got an empty value"
        )
    if HEX_SHA_PATTERN.fullmatch(candidate):
        return candidate

    tail = candidate.rsplit("/", 1)[-1]
    if HEX_SHA_PATTERN.fullmatch(tail):
        print(
            "::warning::[normalize-artifacts] nightly source_git_ref was not a bare SHA; "
            f"using trailing SHA component '{tail}'."
        )
        return tail

    raise RuntimeError(
        "nightly build requires --source-git-ref to be a hex commit SHA (8-64 chars), "
        "or to end with one (for example: origin/<sha>); "
        f"got {source_git_ref!r}"
    )


def detect_artifact_extension(path: pathlib.Path) -> str | None:
    lower_name = path.name.lower()
    best_match: str | None = None
    best_len = -1

    for ext in ARTIFACT_EXTENSIONS:
        if not lower_name.endswith(ext.lower()):
            continue
        ext_len = len(ext)
        if ext_len > best_len:
            best_len = ext_len
            best_match = ext

    return best_match


def strip_extension(name: str, ext: str) -> str:
    return name[: -len(ext)] if ext else name


def canonicalization_extension(ext: str) -> str:
    if ext.endswith(".sig"):
        return ext[:-4]
    return ext


def should_normalize_file(path: pathlib.Path) -> bool:
    ext = detect_artifact_extension(path)
    if ext is None:
        return False
    stem = strip_extension(path.name, ext)
    return stem.startswith("AstrBot_") or stem.startswith("AstrBot-")


def strip_nightly_suffix(stem: str) -> str:
    stem = NIGHTLY_DATE_PATTERN.sub("", stem)
    return NIGHTLY_HASH_PATTERN.sub("", stem)


def should_preserve_original_nightly_suffix(stem: str, ext: str) -> bool:
    return ext == ".zip" and ("_portable" in stem or "-portable" in stem)


def canonicalize_stem(
    stem: str, ext: str, warned_unknown_arches: set[str]
) -> tuple[str, bool]:
    for pattern, normalized_template in CANONICALIZE_RULES.get(ext, ()):
        match = pattern.fullmatch(stem)
        if not match:
            continue
        groups = {key: value or "" for key, value in match.groupdict().items()}
        if "arch" in groups:
            groups["arch"] = normalize_arch(groups["arch"], warned_unknown_arches)
        return normalized_template.format(**groups), True

    return stem, False


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Normalize AstrBot release artifact filenames before publishing."
    )
    parser.add_argument(
        "--root", required=True, help="Directory containing release artifacts."
    )
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
        print(
            f"[normalize-artifacts] artifact directory does not exist, skipping: {root}"
        )
        return 0
    if not root.is_dir():
        raise RuntimeError(f"artifact path is not a directory: {root}")

    build_mode = args.build_mode.strip().lower()
    source_git_ref = args.source_git_ref.strip()
    is_nightly = build_mode == "nightly"
    resolved_source_sha = ""
    if is_nightly:
        resolved_source_sha = resolve_nightly_source_sha(source_git_ref)
    short_sha = resolved_source_sha[:8]
    nightly_suffix = f"_nightly_{short_sha}" if is_nightly else ""

    unmatched_messages: list[str] = []
    renamed_count = 0
    skipped_count = 0
    warned_unknown_arches: set[str] = set()
    rename_plan: list[tuple[pathlib.Path, pathlib.Path]] = []
    target_sources: dict[pathlib.Path, list[pathlib.Path]] = {}
    all_sources: set[pathlib.Path] = set()

    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        if not should_normalize_file(path):
            skipped_count += 1
            continue

        ext = detect_artifact_extension(path)
        if ext is None:
            skipped_count += 1
            continue

        original_name = path.name
        original_stem = strip_extension(original_name, ext)
        canonical_ext = canonicalization_extension(ext)

        stripped_stem = strip_nightly_suffix(original_stem)
        candidate_stems = [stripped_stem]
        if should_preserve_original_nightly_suffix(original_stem, canonical_ext):
            candidate_stems.insert(0, original_stem)

        normalized_stem = original_stem
        matched = False
        for candidate_stem in candidate_stems:
            normalized_stem, matched = canonicalize_stem(
                candidate_stem, canonical_ext, warned_unknown_arches
            )
            if matched:
                break

        if not matched:
            unmatched_messages.append(
                f"[normalize-artifacts] unmatched naming pattern: original={original_name}, "
                f"normalized={original_name}"
            )
            if args.strict_unmatched:
                continue

        final_stem = normalized_stem if matched else original_stem
        if matched and nightly_suffix and not final_stem.endswith(nightly_suffix):
            final_stem = f"{final_stem}{nightly_suffix}"

        new_path = path.with_name(f"{final_stem}{ext}")
        all_sources.add(path)
        target_sources.setdefault(new_path, []).append(path)
        if new_path != path:
            rename_plan.append((path, new_path))

    collisions = {
        target: sources
        for target, sources in target_sources.items()
        if len(sources) > 1
    }
    if collisions:
        collision_details = []
        for target, sources in sorted(
            collisions.items(), key=lambda item: str(item[0])
        ):
            source_list = ", ".join(str(source.name) for source in sorted(sources))
            collision_details.append(f"{target.name} <= {source_list}")
        raise RuntimeError(
            "artifact filename normalization collision detected:\n"
            + "\n".join(collision_details)
        )

    for source_path, target_path in rename_plan:
        if target_path.exists() and target_path not in all_sources:
            raise RuntimeError(
                "artifact filename normalization target already exists and is outside "
                f"the normalization set: source={source_path.name}, target={target_path.name}"
            )

    staged_renames: list[tuple[pathlib.Path, pathlib.Path, str]] = []
    for index, (source_path, target_path) in enumerate(rename_plan):
        temp_path = source_path.with_name(f".normalize_tmp_{index}_{source_path.name}")
        while temp_path.exists():
            temp_path = temp_path.with_name(f".normalize_tmp_{index}_{temp_path.name}")
        source_path.rename(temp_path)
        staged_renames.append((temp_path, target_path, source_path.name))

    for staged_path, target_path, original_name in staged_renames:
        if target_path.exists():
            raise RuntimeError(
                "artifact filename normalization target already exists after staging: "
                f"source={original_name}, target={target_path.name}"
            )
        staged_path.rename(target_path)
        renamed_count += 1
        print(f"[normalize-artifacts] renamed: {original_name} -> {target_path.name}")

    if unmatched_messages:
        if args.strict_unmatched:
            for message in unmatched_messages:
                print(message, file=sys.stderr)
            print(
                f"{len(unmatched_messages)} artifact(s) did not match canonical naming patterns",
                file=sys.stderr,
            )
            return 1
        for message in unmatched_messages:
            print(f"::warning::{message}")

    print(
        f"[normalize-artifacts] completed: renamed={renamed_count}, "
        f"unmatched={len(unmatched_messages)}, skipped_non_target={skipped_count}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
