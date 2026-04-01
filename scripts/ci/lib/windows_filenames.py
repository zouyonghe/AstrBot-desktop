from __future__ import annotations

import re

WINDOWS_FILENAME_INVALID_CHARS_RE = re.compile(r'[<>:"/\\|?*]')
WINDOWS_FILENAME_INVALID_TRAILING_RE = re.compile(r"[ .]+$")
WINDOWS_RESERVED_DEVICE_NAMES = {
    "CON",
    "PRN",
    "AUX",
    "NUL",
    *{f"COM{i}" for i in range(1, 10)},
    *{f"LPT{i}" for i in range(1, 10)},
}


def validate_windows_filename(name: str) -> None:
    if not name or name in {".", ".."}:
        raise ValueError(f"invalid Windows filename: {name!r}")

    if WINDOWS_FILENAME_INVALID_CHARS_RE.search(name):
        raise ValueError(
            f"invalid Windows filename {name!r}: contains characters invalid in Windows filenames"
        )

    if WINDOWS_FILENAME_INVALID_TRAILING_RE.search(name):
        raise ValueError(
            f"invalid Windows filename {name!r}: trailing spaces or dots are not allowed"
        )

    stem = name.split(".", 1)[0].upper()
    if stem in WINDOWS_RESERVED_DEVICE_NAMES:
        raise ValueError(
            f"invalid Windows filename {name!r}: {stem!r} is a reserved device name"
        )
