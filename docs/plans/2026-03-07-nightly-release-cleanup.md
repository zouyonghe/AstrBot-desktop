# Nightly Release Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make nightly asset cleanup default to the repository currently running the workflow so forks and test repositories do not accumulate stale release assets.

**Architecture:** Keep the current cleanup guardrails, but change the script's default cleanup target from a hardcoded upstream repository to `GITHUB_REPOSITORY`. Add regression tests so default current-repo cleanup and explicit mismatch skipping are both locked in.

**Tech Stack:** Bash, GitHub CLI, Node test runner

---

### Task 1: Add failing regression coverage for cleanup target resolution

**Files:**
- Modify: `scripts/ci/cleanup-release-assets.test.mjs`

**Step 1: Write the failing test**

Add tests for:

- default cleanup targeting current `GITHUB_REPOSITORY`
- explicit `ASTRBOT_RELEASE_CLEANUP_TARGET_REPOSITORY` mismatch still skipping cleanup

**Step 2: Run test to verify it fails**

Run: `node --test scripts/ci/cleanup-release-assets.test.mjs`
Expected: new default-current-repo test fails because the script still hardcodes upstream.

**Step 3: Write minimal implementation**

Do not change production code yet; only finish the test fixture needed to expose the current hardcoded default behavior.

**Step 4: Run test to verify it still fails for the expected reason**

Run: `node --test scripts/ci/cleanup-release-assets.test.mjs`
Expected: failure tied to the current hardcoded cleanup repository behavior.

### Task 2: Change cleanup default to current repository

**Files:**
- Modify: `scripts/ci/cleanup-release-assets.sh`

**Step 1: Implement minimal production change**

- Replace the hardcoded default cleanup repository with `${GITHUB_REPOSITORY}`.
- Keep `ASTRBOT_RELEASE_CLEANUP_TARGET_REPOSITORY` override support unchanged.
- Keep `ASTRBOT_RELEASE_CLEANUP_ALLOW_ANY_REPOSITORY` mismatch bypass unchanged.

**Step 2: Run focused tests**

Run: `node --test scripts/ci/cleanup-release-assets.test.mjs`
Expected: all cleanup-release-assets tests pass.

### Task 3: Run broader verification

**Files:**
- Modify: none

**Step 1: Run script suite**

Run: `pnpm run test:prepare-resources`
Expected: full script test suite passes.

**Step 2: Inspect diff**

Run: `git diff --stat upstream/main...HEAD`
Expected: diff only contains the cleanup script, its tests, and the new plan docs.

**Step 3: Commit**

```bash
git add scripts/ci/cleanup-release-assets.sh scripts/ci/cleanup-release-assets.test.mjs docs/plans/2026-03-07-nightly-release-cleanup-design.md docs/plans/2026-03-07-nightly-release-cleanup.md
git commit -m "fix(ci): clean nightly assets in current repository"
```
