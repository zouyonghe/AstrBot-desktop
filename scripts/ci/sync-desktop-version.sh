#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root_dir="$(cd "${script_dir}/../.." && pwd)"
# shellcheck source=./lib/version-utils.sh
. "${script_dir}/lib/version-utils.sh"

if [ "${#}" -ne 1 ]; then
  echo "Usage: $0 <astrbot-version>" >&2
  exit 2
fi

raw_version="$1"
normalized_version="$(normalize_version "${raw_version}")"

if [ -z "${normalized_version}" ]; then
  echo "Invalid AstrBot version input: '${raw_version}'" >&2
  exit 1
fi

export ASTRBOT_DESKTOP_VERSION="$(with_version_prefix "${normalized_version}")"
echo "Syncing desktop version with ASTRBOT_DESKTOP_VERSION=${ASTRBOT_DESKTOP_VERSION}"
(
  cd "${root_dir}"
  pnpm run sync:version
)
