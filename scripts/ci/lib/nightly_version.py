from __future__ import annotations

import json
import re
from pathlib import Path


SPEC_PATH = Path(__file__).resolve().parents[3] / "src-tauri" / "nightly-version-format.json"
BASE_VERSION_PATTERN = r"[0-9]+(?:\.[0-9]+){1,2}(?:-[a-zA-Z0-9.]+)?"

_SPEC = json.loads(SPEC_PATH.read_text(encoding="utf-8"))
NIGHTLY_CANONICAL_FORMAT = str(_SPEC["canonicalFormat"])
_DATE_DIGITS = int(_SPEC["dateDigits"])
_SHA_HEX_DIGITS = int(_SPEC["shaHexDigits"])

NIGHTLY_VERSION_RE = re.compile(
    rf"^(?P<base>{BASE_VERSION_PATTERN})-nightly\."
    rf"(?P<date>[0-9]{{{_DATE_DIGITS}}})\."
    rf"(?P<sha>[0-9a-fA-F]{{{_SHA_HEX_DIGITS}}})$"
)
