#!/usr/bin/env bash

set -euo pipefail

if [ -z "${RELEASE_TAG:-}" ]; then
  echo "RELEASE_TAG is required." >&2
  exit 1
fi

if [ -z "${GITHUB_REPOSITORY:-}" ]; then
  echo "GITHUB_REPOSITORY is required." >&2
  exit 1
fi

release_id="$(
  gh api "repos/${GITHUB_REPOSITORY}/releases/tags/${RELEASE_TAG}" \
    --jq '.id' 2>/dev/null || true
)"

if [ -z "${release_id}" ]; then
  echo "Release ${RELEASE_TAG} does not exist yet. No assets to clean."
  exit 0
fi

deleted_count=0
while IFS=$'\t' read -r asset_id asset_name; do
  [ -n "${asset_id}" ] || continue
  gh api -X DELETE "repos/${GITHUB_REPOSITORY}/releases/assets/${asset_id}" >/dev/null
  echo "Deleted existing release asset: id=${asset_id}, name=${asset_name}"
  deleted_count=$((deleted_count + 1))
done < <(
  gh api --paginate "repos/${GITHUB_REPOSITORY}/releases/${release_id}/assets?per_page=100" \
    --jq 'if type == "array" then .[] else empty end | [.id, .name] | @tsv'
)

if [ "${deleted_count}" -eq 0 ]; then
  echo "Release ${RELEASE_TAG} has no existing assets."
fi
