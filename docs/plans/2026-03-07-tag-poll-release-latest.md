# Tag-Poll Release Latest Guard Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Stop manual `tag-poll` workflow runs from marking non-latest upstream tags as the repository `latest` release.

**Architecture:** Keep release publication in the existing workflow, but move `latest` eligibility into `scripts/ci/resolve-build-context.sh`. The script resolves the current upstream latest tag, compares it with the effective build ref, emits `release_make_latest`, and the workflow uses that explicit output when publishing the release.

**Tech Stack:** Bash, GitHub Actions YAML, Node test runner

---

### Task 1: Add failing regression coverage for latest-release selection

**Files:**
- Create: `scripts/ci/resolve-build-context.test.mjs`

**Step 1: Write the failing test**

Add regression tests for:

- manual `workflow_dispatch` tag-poll with latest upstream tag -> `release_make_latest=true`
- manual `workflow_dispatch` tag-poll with older upstream tag -> `release_make_latest=false`
- automatic tag-poll latest selection -> `release_make_latest=true`
- nightly -> `release_make_latest=false`

**Step 2: Run test to verify it fails**

Run: `node --test "scripts/ci/resolve-build-context.test.mjs"`
Expected: failure because `resolve-build-context.sh` does not emit `release_make_latest` yet.

**Step 3: Keep test doubles minimal**

- Stub only the external commands needed by the shell script (`git`, `curl`, `sort`).
- Avoid mocking extra behavior that the script does not exercise.

**Step 4: Run test again to verify the failure is still for the expected reason**

Run: `node --test "scripts/ci/resolve-build-context.test.mjs"`
Expected: assertions fail on missing or incorrect `release_make_latest` output, not on broken test plumbing.

### Task 2: Emit explicit latest-release eligibility from resolve-build-context

**Files:**
- Modify: `scripts/ci/resolve-build-context.sh`

**Step 1: Implement minimal production change**

- Resolve the latest upstream tag once for `tag-poll` mode.
- Normalize annotated tag refs so `refs/tags/<tag>^{}` does not distort latest-tag detection.
- Add `release_make_latest=false` default.
- Set `release_make_latest=true` only when:
  - `should_build=true`
  - `publish_release=true`
  - `build_mode=tag-poll`
  - effective `source_git_ref` equals the resolved latest upstream tag

**Step 2: Expose the new output**

- Write `release_make_latest` to `GITHUB_OUTPUT`.
- Log the resolved value for debugging in CI output.

**Step 3: Run focused tests**

Run: `node --test "scripts/ci/resolve-build-context.test.mjs"`
Expected: all resolve-build-context tests pass.

### Task 3: Wire workflow release publishing to the new output

**Files:**
- Modify: `.github/workflows/build-desktop-tauri.yml`

**Step 1: Add workflow output mapping**

- Expose `release_make_latest` from the `resolve_build_context` job outputs.

**Step 2: Update release publishing**

- Change `softprops/action-gh-release` `make_latest` to read `needs.resolve_build_context.outputs.release_make_latest == 'true'`.

**Step 3: Re-run focused tests**

Run: `node --test "scripts/ci/resolve-build-context.test.mjs"`
Expected: tests remain green after the workflow wiring change.

### Task 4: Run broader verification and prepare review

**Files:**
- Modify: none

**Step 1: Run script suite**

Run: `pnpm run test:prepare-resources`
Expected: all Node-based script tests pass, including the new resolve-build-context regression coverage.

**Step 2: Inspect diff**

Run: `git diff --stat`
Expected: only the workflow, resolve-build-context script, new test, and plan docs are changed.

**Step 3: Commit**

```bash
git add .github/workflows/build-desktop-tauri.yml scripts/ci/resolve-build-context.sh scripts/ci/resolve-build-context.test.mjs docs/plans/2026-03-07-tag-poll-release-latest-design.md docs/plans/2026-03-07-tag-poll-release-latest.md
git commit -m "fix(ci): guard latest release for manual tag-poll runs"
```
