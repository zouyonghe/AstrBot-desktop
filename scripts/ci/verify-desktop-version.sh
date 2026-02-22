#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root_dir="$(cd "${script_dir}/../.." && pwd)"

if [ "${#}" -ne 1 ]; then
  echo "Usage: $0 <expected-astrbot-version>" >&2
  exit 2
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required to verify synced desktop version files" >&2
  exit 1
fi

python3 "${script_dir}/verify-desktop-version.py" "${1}" --root "${root_dir}"
