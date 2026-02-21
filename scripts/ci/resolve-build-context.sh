#!/usr/bin/env bash

set -euo pipefail

DEFAULT_NIGHTLY_UTC_HOUR='3'
DEFAULT_LS_REMOTE_RETRY_ATTEMPTS='3'
DEFAULT_LS_REMOTE_RETRY_SLEEP_SECONDS='2'

temp_dirs=()
cleanup_temp_dirs() {
  local dir
  for dir in "${temp_dirs[@]-}"; do
    [ -n "${dir}" ] || continue
    rm -rf "${dir}" 2>/dev/null || true
  done
}
trap cleanup_temp_dirs EXIT

is_transient_git_error() {
  local message="$1"
  printf '%s' "${message}" | grep -Eiq \
    '(Could not resolve host|Failed to connect|Connection (timed out|reset|refused)|Operation timed out|Temporary failure|TLS|SSL|HTTP [0-9]*5[0-9]{2}|The requested URL returned error: 5[0-9]{2}|network is unreachable)'
}

sanitize_positive_int() {
  local raw="$1"
  local fallback="$2"
  local max_value="$3"
  case "${raw}" in
    ''|*[!0-9]*) printf '%s\n' "${fallback}" ;;
    *)
      if [ "${raw}" -lt 1 ] 2>/dev/null; then
        printf '%s\n' "${fallback}"
      elif [ "${raw}" -gt "${max_value}" ] 2>/dev/null; then
        printf '%s\n' "${max_value}"
      else
        printf '%s\n' "${raw}"
      fi
      ;;
  esac
}

git_ls_remote_with_retry() {
  local source_url="$1"
  local source_ref="$2"
  local label="$3"
  local attempts="$4"
  local sleep_seconds="$5"

  local attempt=1
  local output=""
  local error_class=""

  while [ "${attempt}" -le "${attempts}" ]; do
    if output="$(git ls-remote "${source_url}" "${source_ref}" 2>&1)"; then
      printf '%s\n' "${output}"
      return 0
    fi

    if is_transient_git_error "${output}"; then
      error_class="transient-network"
    else
      error_class="non-transient"
    fi
    echo "::warning::git ls-remote failed (${label}) attempt ${attempt}/${attempts}, class=${error_class}: ${output}"

    if [ "${error_class}" != "transient-network" ]; then
      break
    fi
    if [ "${attempt}" -lt "${attempts}" ]; then
      sleep "${sleep_seconds}"
    fi
    attempt=$((attempt + 1))
  done

  local final_attempt="${attempt}"
  if [ "${final_attempt}" -gt "${attempts}" ]; then
    final_attempt="${attempts}"
  fi
  echo "::error::Unable to resolve ${label} from ${source_url} after ${final_attempt} attempt(s)."
  return 1
}

source_git_url="${ASTRBOT_SOURCE_GIT_URL}"
source_git_ref="${ASTRBOT_SOURCE_GIT_REF}"
nightly_source_git_ref="${ASTRBOT_NIGHTLY_SOURCE_GIT_REF:-master}"
nightly_utc_hour="${ASTRBOT_NIGHTLY_UTC_HOUR:-${DEFAULT_NIGHTLY_UTC_HOUR}}"
workflow_build_mode_raw="${WORKFLOW_BUILD_MODE:-}"
if [ -z "${workflow_build_mode_raw}" ]; then
  if [ "${GITHUB_EVENT_NAME}" = "workflow_dispatch" ]; then
    workflow_build_mode_raw="nightly"
  else
    workflow_build_mode_raw="auto"
  fi
fi
requested_build_mode="$(printf '%s' "${workflow_build_mode_raw}" | tr '[:upper:]' '[:lower:]')"
should_build="true"
build_mode="${requested_build_mode}"
publish_release="false"
release_tag=""
release_name=""
release_prerelease="false"
workflow_source_git_ref_provided="false"

case "${requested_build_mode}" in
  auto|tag-poll|nightly) ;;
  *)
    if [ "${GITHUB_EVENT_NAME}" = "workflow_dispatch" ]; then
      echo "::error::invalid build_mode input '${requested_build_mode}'; expected tag-poll/nightly (auto is deprecated but still accepted and normalized to tag-poll for backward compatibility)."
    else
      echo "::error::invalid build_mode input '${requested_build_mode}'; expected auto/tag-poll/nightly."
    fi
    exit 1
    ;;
esac

case "${nightly_utc_hour}" in
  '')
    nightly_utc_hour="${DEFAULT_NIGHTLY_UTC_HOUR}"
    ;;
  *[!0-9]*)
    echo "WARN: non-numeric ASTRBOT_NIGHTLY_UTC_HOUR=${nightly_utc_hour}, fallback to ${DEFAULT_NIGHTLY_UTC_HOUR}."
    nightly_utc_hour="${DEFAULT_NIGHTLY_UTC_HOUR}"
    ;;
esac
if [ "${nightly_utc_hour}" -gt 23 ] 2>/dev/null; then
  echo "WARN: invalid ASTRBOT_NIGHTLY_UTC_HOUR=${nightly_utc_hour}, fallback to ${DEFAULT_NIGHTLY_UTC_HOUR}."
  nightly_utc_hour="${DEFAULT_NIGHTLY_UTC_HOUR}"
fi
nightly_utc_hour_padded="$(printf '%02d' "${nightly_utc_hour}")"
echo "Nightly UTC hour normalized to ${nightly_utc_hour_padded} (raw='${ASTRBOT_NIGHTLY_UTC_HOUR:-<unset>}')."

if [ "${GITHUB_EVENT_NAME}" = "workflow_dispatch" ]; then
  if [ -n "${WORKFLOW_SOURCE_GIT_URL:-}" ]; then
    source_git_url="${WORKFLOW_SOURCE_GIT_URL}"
  fi
  if [ -n "${WORKFLOW_SOURCE_GIT_REF:-}" ]; then
    source_git_ref="${WORKFLOW_SOURCE_GIT_REF}"
    workflow_source_git_ref_provided="true"
  fi
  if [ "${WORKFLOW_PUBLISH_RELEASE:-true}" = "true" ]; then
    publish_release="true"
  else
    publish_release="false"
  fi
fi

# Normalize build mode in one place to keep behavior explicit and predictable.
case "${GITHUB_EVENT_NAME}" in
  workflow_dispatch)
    if [ "${requested_build_mode}" = "auto" ]; then
      echo "::warning::workflow_dispatch build_mode=auto is deprecated; normalized to tag-poll."
      build_mode="tag-poll"
      if [ "${publish_release}" = "true" ]; then
        echo "::warning::workflow_dispatch build_mode=auto keeps legacy behavior: publish_release=true is normalized to false."
        publish_release="false"
      fi
    else
      build_mode="${requested_build_mode}"
    fi
    if [ "${build_mode}" = "tag-poll" ]; then
      echo "::notice::workflow_dispatch tag-poll selected. Prefer schedule runs for routine tag polling."
    fi
    ;;
  schedule)
    publish_release="true"
    current_utc_hour="$(date -u +%H)"
    if [ "${requested_build_mode}" = "auto" ]; then
      if [ "${current_utc_hour}" = "${nightly_utc_hour_padded}" ]; then
        build_mode="nightly"
        echo "::notice::schedule build_mode=auto resolved to nightly at UTC hour ${current_utc_hour}."
      else
        build_mode="tag-poll"
        echo "::notice::schedule build_mode=auto resolved to tag-poll at UTC hour ${current_utc_hour} (nightly hour ${nightly_utc_hour_padded})."
      fi
    else
      build_mode="${requested_build_mode}"
      echo "::notice::schedule run using explicit WORKFLOW_BUILD_MODE=${build_mode}."
    fi
    if [ "${build_mode}" = "nightly" ]; then
      echo "Scheduled nightly run at UTC hour ${current_utc_hour}."
    elif [ "${build_mode}" = "tag-poll" ]; then
      echo "Scheduled tag polling run at UTC hour ${current_utc_hour}."
    fi
    ;;
  *)
    if [ "${requested_build_mode}" = "auto" ]; then
      build_mode="tag-poll"
      echo "::notice::${GITHUB_EVENT_NAME} build_mode=auto normalized to tag-poll."
    else
      build_mode="${requested_build_mode}"
    fi
    ;;
esac

retry_attempts="$(
  sanitize_positive_int \
    "${ASTRBOT_LS_REMOTE_RETRY_ATTEMPTS:-${DEFAULT_LS_REMOTE_RETRY_ATTEMPTS}}" \
    "${DEFAULT_LS_REMOTE_RETRY_ATTEMPTS}" \
    "10"
)"
retry_sleep_seconds="$(
  sanitize_positive_int \
    "${ASTRBOT_LS_REMOTE_RETRY_SLEEP_SECONDS:-${DEFAULT_LS_REMOTE_RETRY_SLEEP_SECONDS}}" \
    "${DEFAULT_LS_REMOTE_RETRY_SLEEP_SECONDS}" \
    "60"
)"

if [ "${build_mode}" = "nightly" ]; then
  if [ "${workflow_source_git_ref_provided}" = "true" ]; then
    echo "::warning::workflow_dispatch nightly mode ignores source_git_ref='${WORKFLOW_SOURCE_GIT_REF:-}'. Using latest commit from configured nightly branch."
  fi
  nightly_branch="${nightly_source_git_ref}"
  if [ -z "${nightly_branch}" ]; then
    echo "ASTRBOT_NIGHTLY_SOURCE_GIT_REF must be set to a branch name or refs/heads/<branch> for nightly builds." >&2
    exit 1
  fi
  case "${nightly_branch}" in
    refs/heads/*)
      echo "Normalizing nightly source ref '${nightly_branch}' to branch name for git ls-remote."
      nightly_branch="${nightly_branch#refs/heads/}"
      ;;
    refs/*)
      echo "ASTRBOT_NIGHTLY_SOURCE_GIT_REF must be a branch name or refs/heads/<branch>; got '${nightly_branch}'." >&2
      exit 1
      ;;
  esac

  nightly_remote_output="$(
    git_ls_remote_with_retry \
      "${source_git_url}" \
      "refs/heads/${nightly_branch}" \
      "nightly branch refs/heads/${nightly_branch}" \
      "${retry_attempts}" \
      "${retry_sleep_seconds}"
  )"
  source_git_ref="$(printf '%s\n' "${nightly_remote_output}" | awk 'NR==1{print $1}')"
  if [ -z "${source_git_ref}" ]; then
    echo "Unable to resolve latest commit from ${source_git_url} refs/heads/${nightly_branch} (configured ASTRBOT_NIGHTLY_SOURCE_GIT_REF='${nightly_source_git_ref}')." >&2
    exit 1
  fi
  echo "Nightly source resolved from ${nightly_branch}@${source_git_ref} (configured ASTRBOT_NIGHTLY_SOURCE_GIT_REF='${nightly_source_git_ref}')."
elif [ "${build_mode}" = "tag-poll" ]; then
  if [ "${workflow_source_git_ref_provided}" = "true" ]; then
    echo "workflow_dispatch tag-poll mode: using explicit source ref override ${source_git_ref}"
  else
    tag_remote_output="$(
      git_ls_remote_with_retry \
        "${source_git_url}" \
        "refs/tags/*" \
        "upstream tags refs/tags/*" \
        "${retry_attempts}" \
        "${retry_sleep_seconds}"
    )"
    latest_tag="$(printf '%s\n' "${tag_remote_output}" \
      | awk '{print $2}' \
      | sed 's#refs/tags/##' \
      | sort -V \
      | tail -n 1)"
    if [ -z "${latest_tag}" ]; then
      echo "Unable to resolve latest tag from ${source_git_url}" >&2
      exit 1
    fi
    source_git_ref="${latest_tag}"
    echo "Tag polling run detected latest upstream tag: ${source_git_ref}"
  fi

  http_status="$(curl -sS -o /dev/null -w '%{http_code}' \
    -H "Authorization: Bearer ${GITHUB_TOKEN}" \
    -H "Accept: application/vnd.github+json" \
    "https://api.github.com/repos/${GH_REPOSITORY}/releases/tags/${source_git_ref}")"
  if [ "${http_status}" = "200" ]; then
    should_build="false"
    echo "Release ${source_git_ref} already exists. Tag unchanged, skipping build."
  else
    echo "Release ${source_git_ref} not found (HTTP ${http_status}). Build will run."
  fi
fi

version=""
if [ "${should_build}" = "true" ]; then
  if printf '%s' "${source_git_ref}" | grep -Eq '^v[0-9]+(\.[0-9]+){1,2}([.-][0-9A-Za-z.-]+)?$'; then
    version="${source_git_ref#v}"
    echo "Resolved version directly from source tag: ${source_git_ref}"
  else
    workdir="$(mktemp -d)"
    temp_dirs+=("${workdir}")
    repo_dir="${workdir}/AstrBot"
    git init "${repo_dir}"
    git -C "${repo_dir}" remote add origin "${source_git_url}"
    git -C "${repo_dir}" fetch --depth 1 origin "${source_git_ref}"
    git -C "${repo_dir}" checkout --detach FETCH_HEAD
    version="$(python3 scripts/ci/read-project-version.py "${repo_dir}/pyproject.toml")"
  fi
else
  version="${source_git_ref#v}"
  if [ -z "${version}" ] || [ "${version}" = "${source_git_ref}" ]; then
    version="unknown"
  fi
fi

if [ "${build_mode}" = "nightly" ] && [ "${should_build}" = "true" ]; then
  nightly_date="$(date -u +%Y%m%d)"
  short_sha="$(printf '%s' "${source_git_ref}" | cut -c1-8)"
  version="${version}-nightly.${nightly_date}.${short_sha}"
  release_tag="v${version}"
  release_name="AstrBot Desktop v${version}"
  release_prerelease="true"
elif [ "${publish_release}" = "true" ] && [ "${should_build}" = "true" ]; then
  release_tag="v${version}"
  release_name="AstrBot Desktop v${version}"
  release_prerelease="false"
fi

{
  echo "source_git_url=${source_git_url}"
  echo "source_git_ref=${source_git_ref}"
  echo "astrbot_version=${version}"
  echo "should_build=${should_build}"
  echo "build_mode=${build_mode}"
  echo "publish_release=${publish_release}"
  echo "release_tag=${release_tag}"
  echo "release_name=${release_name}"
  echo "release_prerelease=${release_prerelease}"
} >> "${GITHUB_OUTPUT}"

echo "Resolved source: ${source_git_url}@${source_git_ref}"
echo "Resolved AstrBot version: ${version}"
echo "Build enabled: ${should_build}"
echo "Build mode: ${build_mode}"
echo "Publish release: ${publish_release}"
echo "Release tag: ${release_tag:-<none>}"
echo "Release prerelease: ${release_prerelease}"
