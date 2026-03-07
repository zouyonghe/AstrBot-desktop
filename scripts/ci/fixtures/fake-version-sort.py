#!/usr/bin/env python3
import re
import sys


SUPPORTED_SHORT_FLAGS = {"V", "u"}


def parse_short_flags(argv):
    enabled = set()
    for arg in argv:
        if not arg.startswith("-") or arg == "-":
            raise SystemExit(f"unsupported sort argument: {arg}")
        if arg.startswith("--"):
            raise SystemExit(f"unsupported sort option: {arg}")

        for flag in arg[1:]:
            if flag not in SUPPORTED_SHORT_FLAGS:
                raise SystemExit(f"unsupported sort flag: -{flag}")
            enabled.add(flag)
    return enabled


enabled_flags = parse_short_flags(sys.argv[1:])
raw_lines = [line.rstrip("\n") for line in sys.stdin]
unique = "u" in enabled_flags
lines = list(dict.fromkeys(raw_lines)) if unique else raw_lines


def version_key(value):
    return [int(part) if part.isdigit() else part for part in re.split(r"(\d+)", value)]


for line in sorted(lines, key=version_key):
    print(line)
