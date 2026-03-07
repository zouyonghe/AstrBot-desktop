# Nightly Release Cleanup Design

## Goal

Make nightly release asset cleanup run against the repository that is actually publishing the release by default, while still allowing an explicit override target when needed.

## Current Problem

- `scripts/ci/cleanup-release-assets.sh` hardcodes `AstrBotDevs/AstrBot-desktop` as the default cleanup target.
- In forks or test repositories, the cleanup step exits early because `GITHUB_REPOSITORY` does not match that hardcoded target.
- The workflow still uploads fresh assets afterward, so nightly releases in non-upstream repos accumulate assets over time.

## Decision

Change the default cleanup target repository from a hardcoded upstream name to the current `GITHUB_REPOSITORY`.

Keep both existing escape hatches:

- `ASTRBOT_RELEASE_CLEANUP_TARGET_REPOSITORY` can still force cleanup to target a different repository.
- `ASTRBOT_RELEASE_CLEANUP_ALLOW_ANY_REPOSITORY` can still bypass the mismatch protection entirely.

## Why

- Default behavior becomes intuitive: clean the same repository that is about to publish assets.
- Forks, staging repos, and test repos stop accumulating nightly assets.
- Explicit repository targeting still works for special admin workflows.
- No dangerous default bypass is introduced.

## Impact

- Upstream behavior stays effectively the same.
- Fork nightly releases start deleting old assets before publishing new ones.
- The protection against accidental cross-repository deletion remains in place unless explicitly overridden.

## Verification

- Cleanup script defaults target repository to `GITHUB_REPOSITORY`.
- Mismatch protection still triggers when `ASTRBOT_RELEASE_CLEANUP_TARGET_REPOSITORY` is explicitly set to another repo.
- Tests cover default current-repo cleanup behavior and explicit mismatch skip behavior.
