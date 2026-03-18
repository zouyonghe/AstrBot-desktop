#!/usr/bin/env bash
set -euo pipefail

repo_dir=""
if [ "${1-}" = "-C" ]; then
  repo_dir="${2-}"
  shift 2
fi

command_name="${1-}"
shift || true

case "${command_name}" in
  ls-remote)
    source_ref="${2-}"
    case "${source_ref}" in
      refs/tags/*)
        if [ "${ASTRBOT_TEST_GIT_TAGS_FAIL:-0}" = "1" ]; then
          echo 'simulated tag lookup failure' >&2
          exit 1
        fi
        IFS='|' read -r -a entries <<< "${ASTRBOT_TEST_GIT_TAGS:-}"
        for entry in "${entries[@]}"; do
          [ -n "${entry}" ] || continue
          printf '%s\n' "${entry}"
        done
        ;;
      refs/heads/*)
        printf '%s %s\n' "${ASTRBOT_TEST_NIGHTLY_REF:-3333333333333333333333333333333333333333}" "${source_ref}"
        ;;
      *)
        printf 'unexpected git ls-remote ref: %s\n' "${source_ref}" >&2
        exit 1
        ;;
    esac
    ;;
  init)
    mkdir -p "${1-}"
    ;;
  remote|checkout)
    :
    ;;
  fetch)
    if [ -z "${repo_dir}" ]; then
      echo 'git fetch expected -C <repo_dir>' >&2
      exit 1
    fi
    mkdir -p "${repo_dir}"
    cat > "${repo_dir}/pyproject.toml" <<EOF
[project]
version = "${ASTRBOT_TEST_FETCHED_VERSION:-4.19.0}"
EOF
    ;;
  rev-parse)
    if [ "${1-}" != "HEAD" ]; then
      printf 'unexpected git rev-parse arg: %s\n' "${1-}" >&2
      exit 1
    fi
    printf '%s\n' "${ASTRBOT_TEST_FETCHED_SHA:-3333333333333333333333333333333333333333}"
    ;;
  *)
    printf 'unexpected git command: %s %s\n' "${command_name}" "$*" >&2
    exit 1
    ;;
esac
