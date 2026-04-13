#!/usr/bin/env bash
set -euo pipefail

if [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
  echo "::warning::APPLE_SIGNING_IDENTITY is not set; skipping code signing."
  exit 0
fi

TARGET="${1:?Usage: $0 <path-to-directory-or-app-bundle> [entitlements.plist]}"
ENTITLEMENTS_PATH="${2:-}"
ENTITLEMENTS_ARGS=()
if [ -n "${ENTITLEMENTS_PATH}" ] && [ -f "${ENTITLEMENTS_PATH}" ]; then
  ENTITLEMENTS_ARGS=(--entitlements "${ENTITLEMENTS_PATH}")
fi

SIGN_OPTS=(--force --options runtime --sign "${APPLE_SIGNING_IDENTITY}")

if [ ! -d "${TARGET}" ]; then
  echo "::error::Target not found: ${TARGET}" >&2
  exit 1
fi

BINARY_FILE_DESCRIPTIONS=()
NESTED_BINARIES=()
while IFS= read -r -d '' file; do
  file_description="$(file --brief "${file}")"
  if [[ "${file_description}" == *"Mach-O"* ]]; then
    NESTED_BINARIES+=("${file}")
    BINARY_FILE_DESCRIPTIONS+=("${file_description}")
  fi
done < <(find "${TARGET}" -type f -print0 2>/dev/null)

if [ "${#NESTED_BINARIES[@]}" -eq 0 ]; then
  echo "No Mach-O binaries found in ${TARGET}; nothing to sign."
  exit 0
fi

echo "Found ${#NESTED_BINARIES[@]} Mach-O binary(ies) to sign in ${TARGET}."

for index in "${!NESTED_BINARIES[@]}"; do
  binary="${NESTED_BINARIES[${index}]}"
  file_description="${BINARY_FILE_DESCRIPTIONS[${index}]}"
  echo "  Signing: ${binary}"
  # Only apply entitlements to executables, not dylibs
  CURRENT_ENTITLEMENTS=()
  if [ ${#ENTITLEMENTS_ARGS[@]} -gt 0 ] && [[ "${file_description}" == *"executable"* ]]; then
    CURRENT_ENTITLEMENTS=("${ENTITLEMENTS_ARGS[@]}")
  fi
  codesign "${SIGN_OPTS[@]}" \
    ${CURRENT_ENTITLEMENTS[@]+"${CURRENT_ENTITLEMENTS[@]}"} \
    "${binary}"
done

echo "Code signing completed successfully for ${#NESTED_BINARIES[@]} binary(ies)."
