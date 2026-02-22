#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import pathlib
import sys
from typing import Any


def normalize_version(raw: str) -> str:
    # NOTE: Keep this logic equivalent to normalize_version() in
    # scripts/ci/lib/version-utils.sh.
    # Run scripts/ci/assert-version-normalization-equivalence.sh after edits.
    trimmed = raw.strip()
    if not trimmed:
        return ""
    if trimmed[0] in {"v", "V"}:
        return trimmed[1:]
    return trimmed


def load_toml_parser():
    try:
        import tomllib as toml_parser  # type: ignore
    except ModuleNotFoundError:
        try:
            import tomli as toml_parser  # type: ignore
        except ModuleNotFoundError:
            print(
                "A TOML parser is required. Use Python 3.11+ (tomllib) or install tomli: python3 -m pip install tomli",
                file=sys.stderr,
            )
            raise SystemExit(1)
    return toml_parser


def read_json_file(path: pathlib.Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as error:
        print(f"Failed to parse {path}: {error}", file=sys.stderr)
        raise SystemExit(1)


def read_toml_file(path: pathlib.Path) -> Any:
    toml_parser = load_toml_parser()
    try:
        return toml_parser.loads(path.read_text(encoding="utf-8"))
    except Exception as error:
        print(f"Failed to parse {path}: {error}", file=sys.stderr)
        raise SystemExit(1)


def extract_string_field(mapping: Any, key: str) -> str:
    if not isinstance(mapping, dict):
        return ""
    value = mapping.get(key, "")
    return value if isinstance(value, str) else ""


def main() -> int:
    parser = argparse.ArgumentParser(description="Verify synced desktop version files.")
    parser.add_argument(
        "expected_version",
        nargs="?",
        help="Expected AstrBot desktop version.",
    )
    parser.add_argument(
        "--print-normalized",
        metavar="RAW_VERSION",
        help="Print normalized version and exit (used by CI parity checks).",
    )
    parser.add_argument(
        "--root",
        default=str(pathlib.Path(__file__).resolve().parents[2]),
        help="Repository root path. Defaults to two levels above this script.",
    )
    args = parser.parse_args()

    if args.print_normalized is not None:
        print(normalize_version(args.print_normalized))
        return 0

    if args.expected_version is None:
        parser.error("expected_version is required unless --print-normalized is used")

    expected = normalize_version(args.expected_version)
    if not expected:
        print(f"Invalid expected version input: '{args.expected_version}'", file=sys.stderr)
        return 1

    root = pathlib.Path(args.root).resolve()
    package_json_path = root / "package.json"
    tauri_conf_path = root / "src-tauri" / "tauri.conf.json"
    cargo_toml_path = root / "src-tauri" / "Cargo.toml"

    package_json = read_json_file(package_json_path)
    tauri_conf = read_json_file(tauri_conf_path)
    cargo_manifest = read_toml_file(cargo_toml_path)

    pkg_version = extract_string_field(package_json, "version")
    if not pkg_version:
        print(
            f"Failed to read the version from {package_json_path} (version is empty)",
            file=sys.stderr,
        )
        return 1

    tauri_version = extract_string_field(tauri_conf, "version")
    cargo_version = extract_string_field(cargo_manifest.get("package", {}), "version")
    if not cargo_version:
        print(f"Failed to resolve package.version from {cargo_toml_path}", file=sys.stderr)
        return 1

    if pkg_version != expected or tauri_version != expected or cargo_version != expected:
        print(
            "Version sync mismatch: "
            f"expected={expected}, "
            f"package.json={pkg_version}, "
            f"tauri.conf.json={tauri_version}, "
            f"Cargo.toml={cargo_version}",
            file=sys.stderr,
        )
        return 1

    print(f"Version sync verified: {expected}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
