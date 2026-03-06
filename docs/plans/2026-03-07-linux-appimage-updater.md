# Linux AppImage Updater Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Linux AppImage build output and enable desktop updater support only for AppImage installs, while keeping `deb` and `rpm` on a manual-download update path.

**Architecture:** Extend the current updater integration with a centralized runtime update-mode resolver and Linux AppImage manifest generation. Keep updater plugin usage unchanged for native-updater platforms and explicitly route non-AppImage Linux installs to a manual-download flow.

**Tech Stack:** Rust, Tauri 2, Python CI script, GitHub Actions, GitHub Releases

---

### Task 1: Add Linux runtime update mode resolution

**Files:**
- Modify: `src-tauri/src/bridge/commands.rs`
- Create or modify: shared helper module under `src-tauri/src/bridge/` if needed
- Test: Rust tests near the new helper

**Step 1: Write failing tests**

Cover:

- Windows/macOS resolve to native updater
- Linux with `APPIMAGE` resolves to native updater
- Linux without `APPIMAGE` resolves to manual download

**Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --locked updater_mode`

Expected: FAIL before helper implementation.

**Step 3: Implement the helper**

Create a centralized runtime update-mode resolver and replace the current hardcoded platform-only boolean.

**Step 4: Re-run focused tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --locked updater_mode`

Expected: PASS.

### Task 2: Route Linux AppImage vs deb/rpm bridge behavior

**Files:**
- Modify: `src-tauri/src/bridge/commands.rs`
- Modify: `src-tauri/src/bridge/updater_types.rs` if extra mapping helpers are needed

**Step 1: Write failing tests for result mapping**

Cover:

- AppImage path uses updater check/install flow
- non-AppImage Linux returns manual-download reason

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --locked updater`

Expected: FAIL before behavior mapping is updated.

**Step 3: Implement behavior split**

Keep Windows/macOS behavior unchanged. On Linux, distinguish AppImage from `deb`/`rpm` installs.

**Step 4: Re-run Rust tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --locked`

Expected: PASS.

### Task 3: Add AppImage output to Linux CI builds

**Files:**
- Modify: `.github/workflows/build-desktop-tauri.yml`

**Step 1: Update Linux build bundles**

Include AppImage in the Linux bundle list.

**Step 2: Update artifact upload patterns**

Ensure AppImage and signature outputs are included in uploaded artifacts.

**Step 3: Review workflow consistency**

Verify release-artifact collection still matches the later release job expectations.

### Task 4: Extend updater manifest generation for AppImage

**Files:**
- Modify: `scripts/ci/generate-tauri-latest-json.py`
- Test: Python syntax check and, if added, targeted script tests

**Step 1: Add AppImage artifact parsing**

Detect Linux AppImage updater artifacts and map them to the correct updater platform key.

**Step 2: Keep deb/rpm out of the updater manifest**

They remain downloadable release assets but should not appear as native updater targets.

**Step 3: Verify script syntax**

Run: `python3 -m py_compile scripts/ci/generate-tauri-latest-json.py`

Expected: PASS.

### Task 5: Final verification

**Files:**
- Modify docs if needed to reflect final Linux updater behavior

**Step 1: Run Rust verification**

Run:
- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`
- `cargo test --manifest-path src-tauri/Cargo.toml --locked`

Expected: PASS.

**Step 2: Run script verification**

Run:
- `pnpm run test:prepare-resources`
- `python3 -m py_compile scripts/ci/generate-tauri-latest-json.py`

Expected: PASS.

**Step 3: CI/release validation checklist**

Confirm that a Linux release produces:

- `deb`
- `rpm`
- `AppImage`
- AppImage signature file(s)
- `latest.json` containing Linux AppImage updater entry
