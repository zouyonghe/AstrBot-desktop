#!/usr/bin/env bash

set -euo pipefail

if ! command -v gh >/dev/null 2>&1; then
  echo "gh CLI is required to clean release assets, but was not found in PATH." >&2
  exit 1
fi

if [ -z "${RELEASE_TAG:-}" ]; then
  echo "RELEASE_TAG is required." >&2
  exit 1
fi

if [ -z "${GITHUB_REPOSITORY:-}" ]; then
  echo "GITHUB_REPOSITORY is required." >&2
  exit 1
fi

default_cleanup_repository="AstrBotDevs/AstrBot-desktop"
cleanup_repository="${ASTRBOT_RELEASE_CLEANUP_TARGET_REPOSITORY:-${default_cleanup_repository}}"
allow_any_repository_flag="$(printf '%s' "${ASTRBOT_RELEASE_CLEANUP_ALLOW_ANY_REPOSITORY:-0}" | tr '[:upper:]' '[:lower:]')"
case "${allow_any_repository_flag}" in
  1|true|yes|on) allow_any_repository="true" ;;
  *) allow_any_repository="false" ;;
esac

if [ "${allow_any_repository}" != "true" ] && [ "${GITHUB_REPOSITORY}" != "${cleanup_repository}" ]; then
  echo "Skipping release asset cleanup for non-target repository ${GITHUB_REPOSITORY} (target=${cleanup_repository})."
  echo "Set ASTRBOT_RELEASE_CLEANUP_ALLOW_ANY_REPOSITORY=1 to bypass this protection."
  exit 0
fi

release_lookup_err=""
assets_list_err=""
assets_list_output=""
cleanup_temp_files() {
  local candidate=""
  for candidate in "${release_lookup_err:-}" "${assets_list_err:-}" "${assets_list_output:-}"; do
    if [ -n "${candidate}" ]; then
      rm -f "${candidate}"
    fi
  done
}
trap cleanup_temp_files EXIT

release_lookup_err="$(mktemp)"
release_id=""
if release_id="$(
  gh api "repos/${GITHUB_REPOSITORY}/releases/tags/${RELEASE_TAG}" \
    --jq '.id' 2>"${release_lookup_err}"
)"; then
  :
else
  if grep -q "HTTP 404" "${release_lookup_err}"; then
    release_id=""
  else
    echo "Failed to resolve release ${RELEASE_TAG} from ${GITHUB_REPOSITORY}:" >&2
    cat "${release_lookup_err}" >&2
    exit 1
  fi
fi

if [ -z "${release_id}" ]; then
  echo "Release ${RELEASE_TAG} does not exist yet. No assets to clean."
  exit 0
fi

deleted_count=0
assets_list_err="$(mktemp)"
assets_list_output="$(mktemp)"
if gh api --paginate "repos/${GITHUB_REPOSITORY}/releases/${release_id}/assets?per_page=100" \
  --jq 'if type == "array" then .[] else empty end | [.id, .name] | @tsv' \
  >"${assets_list_output}" 2>"${assets_list_err}"; then
  :
else
  echo "Failed to list assets for release ${RELEASE_TAG} (id=${release_id}) from ${GITHUB_REPOSITORY}:" >&2
  cat "${assets_list_err}" >&2
  exit 1
fi

while IFS=$'\t' read -r asset_id asset_name; do
  [ -n "${asset_id}" ] || continue
  gh api -X DELETE "repos/${GITHUB_REPOSITORY}/releases/assets/${asset_id}" >/dev/null
  echo "Deleted existing release asset: id=${asset_id}, name=${asset_name}"
  deleted_count=$((deleted_count + 1))
done < "${assets_list_output}"

if [ "${deleted_count}" -eq 0 ]; then
  echo "Release ${RELEASE_TAG} has no existing assets."
fi
