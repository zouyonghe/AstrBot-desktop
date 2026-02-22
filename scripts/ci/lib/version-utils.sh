#!/usr/bin/env bash

# NOTE: Keep this logic equivalent to normalize_version() in
# scripts/ci/verify-desktop-version.py.
# Run scripts/ci/assert-version-normalization-equivalence.sh after edits.
normalize_version() {
  local raw="${1-}"
  printf '%s' "${raw}" \
    | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//; s/^[vV]//'
}

with_version_prefix() {
  local normalized
  normalized="$(normalize_version "${1-}")"
  if [ -z "${normalized}" ]; then
    printf '%s' ""
    return 0
  fi
  printf 'v%s' "${normalized}"
}
