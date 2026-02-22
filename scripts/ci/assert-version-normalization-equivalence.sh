#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=./lib/version-utils.sh
. "${script_dir}/lib/version-utils.sh"

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required to assert version normalization parity" >&2
  exit 1
fi

# Representative cases for trimming + single optional v/V prefix removal.
cases=(
  ""
  "1.2.3"
  " v1.2.3 "
  "V1.2.3"
  "vv1.2.3"
  "  v-nightly.20260222 "
  " release-1 "
)

for raw in "${cases[@]}"; do
  shell_normalized="$(normalize_version "${raw}")"
  python_normalized="$(
    python3 "${script_dir}/verify-desktop-version.py" --print-normalized "${raw}"
  )"

  if [ "${shell_normalized}" != "${python_normalized}" ]; then
    echo "Version normalization mismatch detected." >&2
    echo "raw='${raw}'" >&2
    echo "shell='${shell_normalized}'" >&2
    echo "python='${python_normalized}'" >&2
    exit 1
  fi
done

echo "Version normalization parity verified (shell <-> python)."
