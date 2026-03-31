#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root_dir="$(cd "${script_dir}/../.." && pwd)"

if [ -z "${WINDOWS_RELEASE_ASSET_GLOBS:-}" ]; then
  echo "WINDOWS_RELEASE_ASSET_GLOBS is required." >&2
  exit 1
fi

missing_patterns=0

(
  cd "${root_dir}"

  while IFS= read -r raw_pattern; do
    pattern="$(printf '%s' "${raw_pattern}" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//')"
    [ -n "${pattern}" ] || continue

    # Keep glob semantics explicit: this checker currently supports basic bash
    # globs and intentionally rejects `**` to avoid silent mismatches when
    # `globstar` is not enabled.
    if [[ "${pattern}" == *"**"* ]]; then
      echo "Unsupported WINDOWS_RELEASE_ASSET_GLOBS pattern '${pattern}': '**' requires globstar and is not supported here." >&2
      missing_patterns=1
      continue
    fi

    # Use compgen + mapfile to preserve spaces in matched paths.
    mapfile -t matches < <(compgen -G "${pattern}" || true)
    if [ "${#matches[@]}" -eq 0 ]; then
      echo "Missing Windows release asset output for pattern: ${pattern}" >&2
      missing_patterns=1
      continue
    fi

    echo "Detected Windows release assets for pattern ${pattern}:"
    printf '  %s\n' "${matches[@]}"
  done <<< "${WINDOWS_RELEASE_ASSET_GLOBS}"

  if [ "${missing_patterns}" -ne 0 ]; then
    exit 1
  fi
)
