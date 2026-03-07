# Tag-Poll Release Latest Guard Design

## Goal

Prevent manual `tag-poll` workflow runs from promoting non-latest upstream tags to the repository `latest` GitHub Release.

## Current Problem

- `.github/workflows/build-desktop-tauri.yml` currently sets `make_latest` from `release_prerelease != 'true'`.
- That means any stable release published by the workflow becomes `latest`, even if a manual `workflow_dispatch` run targets an older tag through `source_git_ref`.
- Manual rebuilds of historical tags should remain publishable, but they must not replace the current latest stable release marker.

## Decision

Move `latest` eligibility into `scripts/ci/resolve-build-context.sh` and emit an explicit `release_make_latest` output.

- `nightly` stays `false`
- scheduled or manual `tag-poll` runs only set `release_make_latest=true` when the resolved `source_git_ref` matches the current upstream latest tag
- manual overrides to older tags, branches, or commits keep `release_make_latest=false`

The workflow should consume this new output directly instead of inferring `latest` from prerelease state.

## Why

- The decision about `latest` belongs next to the source-ref resolution logic, because that script already knows whether the build is nightly, auto-selected tag-poll, or a manual override.
- This preserves the existing behavior for real latest stable releases.
- It prevents historical rebuilds from unexpectedly changing user-facing release selection on GitHub.

## Impact

- Normal tag-poll runs for the newest upstream tag still publish as `latest`.
- Manual rebuilds for older tags continue to publish release assets, but do not take over the `latest` marker.
- Nightly releases remain prereleases and never become `latest`.

## Verification

- `resolve-build-context` tests cover:
  - manual latest-tag override -> `release_make_latest=true`
  - manual older-tag override -> `release_make_latest=false`
  - no override latest tag-poll -> `release_make_latest=true`
  - nightly -> `release_make_latest=false`
- `pnpm run test:prepare-resources` passes with the new regression test included.
