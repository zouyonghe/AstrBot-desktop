#!/usr/bin/env python3
import re
import sys

raw_lines = [line.rstrip("\n") for line in sys.stdin]
unique = any("u" in arg.lstrip("-") for arg in sys.argv[1:])
lines = list(dict.fromkeys(raw_lines)) if unique else raw_lines


def version_key(value):
    return [int(part) if part.isdigit() else part for part in re.split(r"(\d+)", value)]


for line in sorted(lines, key=version_key):
    print(line)
