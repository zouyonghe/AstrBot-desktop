# Remove macOS Zip Implementation Plan

> Use this plan task-by-task and verify each step before moving on.

**Goal:** Remove macOS zip packaging and updater parsing so release artifacts only include the `.app.tar.gz` assets actually used for macOS updates.

**Architecture:** Remove the macOS zip branch at the workflow edge, then tighten shared updater artifact parsing so macOS updater inputs only flow through `.app.tar.gz`. Update tests to prove zip is no longer accepted while `.app.tar.gz` remains supported.

**Tech Stack:** GitHub Actions workflow YAML, Python release tooling, Node test runner, Python unittest

---

### Task 1: Remove macOS zip packaging from workflow

**Files:**
- Modify: `.github/workflows/build-desktop-tauri.yml`

**Step 1: Write the failing test**

Use a targeted assertion by grepping the workflow for the zip packaging step and upload path.

**Step 2: Run test to verify it fails**

Run: `grep -n "Package macOS app as zip\|bundle/zip/\*\.zip" .github/workflows/build-desktop-tauri.yml`
Expected: output includes both the zip packaging step and the upload path.

**Step 3: Write minimal implementation**

- Remove the `Package macOS app as zip` step.
- Remove `src-tauri/target/${{ matrix.target }}/release/bundle/zip/*.zip` from the macOS upload artifact path list.
- Leave `.app.tar.gz` and `.app.tar.gz.sig` upload paths intact.

**Step 4: Run test to verify it passes**

Run: `grep -n "Package macOS app as zip\|bundle/zip/\*\.zip" .github/workflows/build-desktop-tauri.yml`
Expected: no output and exit status `1`.

### Task 2: Remove macOS zip updater parsing support

**Files:**
- Modify: `scripts/ci/generate_tauri_latest_json.py`
- Modify: `scripts/ci/lib/release_artifacts.py`

**Step 1: Write the failing test**

Add Python tests that assert macOS `.zip.sig` is rejected while `.app.tar.gz.sig` remains accepted.

**Step 2: Run test to verify it fails**

Run: `python3 -m unittest scripts.ci.test_generate_tauri_latest_json`
Expected: failure in the new zip-rejection test because the parser still accepts macOS `.zip.sig`.

**Step 3: Write minimal implementation**

- Remove macOS `.zip` patterns from `scripts/ci/lib/release_artifacts.py`.
- Update `parse_macos_artifact_name` error text in `scripts/ci/generate_tauri_latest_json.py` so it only documents `.app.tar.gz`.
- Change `collect_platforms()` so macOS updater handling only accepts `.app.tar.gz.sig`.

**Step 4: Run test to verify it passes**

Run: `python3 -m unittest scripts.ci.test_generate_tauri_latest_json`
Expected: all tests pass, with the new rejection behavior covered.

### Task 3: Remove obsolete JS coverage and keep regression protection

**Files:**
- Modify: `scripts/ci/release-updater-artifacts.test.mjs`

**Step 1: Write the failing test**

Update the JS integration coverage so macOS normalization/manifests assert `.app.tar.gz` only.

**Step 2: Run test to verify it fails**

Run: `node --test scripts/ci/release-updater-artifacts.test.mjs`
Expected: failure if any fixture or expectation still references macOS zip updater assets.

**Step 3: Write minimal implementation**

- Remove any macOS zip updater fixture usage.
- Keep macOS `.app.tar.gz` integration coverage intact.

**Step 4: Run test to verify it passes**

Run: `node --test scripts/ci/release-updater-artifacts.test.mjs`
Expected: all tests pass.

### Task 4: Run full verification

**Files:**
- Modify: none

**Step 1: Run focused verification**

Run: `python3 -m unittest scripts.ci.test_generate_tauri_latest_json && node --test scripts/ci/release-updater-artifacts.test.mjs`
Expected: all focused updater tests pass.

**Step 2: Run broader script verification**

Run: `pnpm run test:prepare-resources`
Expected: existing script test suite passes.

**Step 3: Verify workflow cleanup**

Run: `grep -n "Package macOS app as zip\|bundle/zip/\*\.zip" .github/workflows/build-desktop-tauri.yml`
Expected: no output and exit status `1`.

**Step 4: Commit**

```bash
git add .github/workflows/build-desktop-tauri.yml scripts/ci/generate_tauri_latest_json.py scripts/ci/lib/release_artifacts.py scripts/ci/release-updater-artifacts.test.mjs scripts/ci/test_generate_tauri_latest_json.py docs/plans/2026-03-07-remove-macos-zip-design.md docs/plans/2026-03-07-remove-macos-zip.md
git commit -m "fix(ci): remove macOS zip updater artifacts"
```
