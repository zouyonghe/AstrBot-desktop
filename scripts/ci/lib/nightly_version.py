from __future__ import annotations

import json
import re
from functools import lru_cache
from pathlib import Path


SPEC_PATH = Path(__file__).resolve().parents[3] / "src-tauri" / "nightly-version-format.json"
BASE_VERSION_PATTERN = r"[0-9]+(?:\.[0-9]+){1,2}"


@lru_cache(maxsize=1)
def nightly_version_spec() -> dict[str, object]:
    return json.loads(SPEC_PATH.read_text(encoding="utf-8"))


NIGHTLY_CANONICAL_FORMAT = str(nightly_version_spec()["canonicalFormat"])
NIGHTLY_VERSION_RE = re.compile(
    rf"^(?P<base>{BASE_VERSION_PATTERN})-nightly\.(?P<date>[0-9]{{{int(nightly_version_spec()['dateDigits'])}}})\.(?P<sha>[0-9a-fA-F]{{{int(nightly_version_spec()['shaHexDigits'])}}})$"
)
