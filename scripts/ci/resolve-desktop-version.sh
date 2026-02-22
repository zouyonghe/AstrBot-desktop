#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib/version-utils.sh
. "${script_dir}/lib/version-utils.sh"

if [ "${#}" -lt 1 ] || [ "${#}" -gt 2 ]; then
  echo "Usage: $0 <raw-version> [github-output-file]" >&2
  exit 2
fi

raw_version="$1"
normalized_version="$(normalize_version "${raw_version}")"
if [ -z "${normalized_version}" ]; then
  echo "Invalid version input: '${raw_version}'" >&2
  exit 1
fi

prefixed_version="$(with_version_prefix "${normalized_version}")"

if [ "${#}" -eq 2 ]; then
  output_file="$2"
  {
    echo "normalized=${normalized_version}"
    echo "prefixed=${prefixed_version}"
  } >> "${output_file}"
else
  echo "${prefixed_version}"
fi
