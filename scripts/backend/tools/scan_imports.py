#!/usr/bin/env python3
from __future__ import annotations

import ast
import json
import sys
from pathlib import Path


def scan_imports(source_path: Path) -> list[dict]:
    source = source_path.read_text(encoding="utf-8")
    tree = ast.parse(source, filename=str(source_path))
    descriptors: list[dict] = []

    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                descriptors.append(
                    {
                        "kind": "import",
                        "module": alias.name,
                    }
                )
            continue

        if isinstance(node, ast.ImportFrom):
            descriptors.append(
                {
                    "kind": "from",
                    "module": node.module or "",
                    "level": int(node.level or 0),
                    "names": [alias.name for alias in node.names],
                }
            )

    return descriptors


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: scan_imports.py <python-file>", file=sys.stderr)
        return 2

    file_path = Path(sys.argv[1])
    if not file_path.exists():
        print(f"file not found: {file_path}", file=sys.stderr)
        return 2

    try:
        descriptors = scan_imports(file_path)
    except (UnicodeDecodeError, SyntaxError, OSError) as error:
        print(
            f"{file_path.name}: failed to scan imports: {error}",
            file=sys.stderr,
        )
        return 1

    print(json.dumps(descriptors, ensure_ascii=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
