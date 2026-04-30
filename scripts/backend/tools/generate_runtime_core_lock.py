from __future__ import annotations

import argparse
import importlib.metadata as importlib_metadata
import json
from pathlib import Path


def _read_top_level_modules(distribution: importlib_metadata.Distribution) -> list[str]:
    try:
        text = distribution.read_text("top_level.txt") or ""
    except (FileNotFoundError, KeyError, UnicodeError):
        return []

    modules: set[str] = set()
    for line in text.splitlines():
        candidate = line.strip()
        if candidate and not candidate.startswith("#"):
            modules.add(candidate)
    return sorted(modules)


def _distribution_record(
    distribution: importlib_metadata.Distribution,
) -> dict[str, object] | None:
    name = distribution.metadata.get("Name")
    version = distribution.version
    if not name or not version:
        return None

    return {
        "name": name,
        "version": version,
        "top_level_modules": _read_top_level_modules(distribution),
    }


def build_lock() -> dict[str, object]:
    records = [
        record
        for distribution in importlib_metadata.distributions()
        if (record := _distribution_record(distribution)) is not None
    ]
    records.sort(key=lambda record: str(record["name"]).lower())
    return {
        "version": 1,
        "distributions": records,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        json.dumps(build_lock(), ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
