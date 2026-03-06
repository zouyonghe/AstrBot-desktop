# Dual-Channel Updater Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add stable / nightly updater channels with persisted channel preference and forward-only cross-channel upgrades.

**Architecture:** Runtime bridge commands build updater instances with channel-specific manifest endpoints instead of relying on one static `latest.json`. Update channel state is stored in `desktop_state.json`, and a dedicated Rust module centralizes channel inference, persistence, and version comparison rules.

**Tech Stack:** Rust, Tauri 2 updater plugin, Node.js bridge contract tests, Python 3 manifest generator tests, GitHub Actions workflow YAML.

---

### Task 1: Lock the channel rules in tests

**Files:**
- Modify: `scripts/prepare-resources/bridge-bootstrap-updater-contract.test.mjs`
- Create: `scripts/ci/test_generate_tauri_latest_json.py`
- Create: `src-tauri/src/update_channel.rs`

**Step 1:** Add JS bridge contract expectations for `getUpdateChannel` and `setUpdateChannel`.

**Step 2:** Add Python tests that expect helper functions for manifest `channel` and `baseVersion` metadata.

**Step 3:** Add Rust tests covering:
- default channel inference from version
- stored channel fallback behavior
- stable-to-nightly same-base allowance
- nightly-to-stable same-base rejection
- nightly-to-stable higher-base allowance

**Step 4:** Run focused tests and confirm they fail for missing behavior.

### Task 2: Implement channel metadata generation

**Files:**
- Modify: `scripts/ci/generate-tauri-latest-json.py`
- Modify: `.github/workflows/build-desktop-tauri.yml`

**Step 1:** Teach the manifest generator to emit channel-aware metadata.

**Step 2:** Split stable and nightly outputs into separate filenames.

**Step 3:** Pass the right channel and output filename from the release workflow.

**Step 4:** Run the Python tests and keep them green.

### Task 3: Implement runtime channel selection

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/bridge/commands.rs`
- Modify: `src-tauri/src/bridge/updater_types.rs`
- Modify: `src-tauri/src/app_runtime.rs`
- Modify: `src-tauri/tauri.conf.json`
- Create or complete: `src-tauri/src/update_channel.rs`

**Step 1:** Add the Rust helper module for channel parsing, persistence, endpoint resolution, and comparator rules.

**Step 2:** Add bridge commands to get/set the preferred update channel.

**Step 3:** Change update check/install commands to use `updater_builder()` with the selected endpoint and comparator.

**Step 4:** Keep existing updater return shapes intact unless extra metadata is strictly useful and backward-compatible.

**Step 5:** Run focused Rust tests and fix any failures.

### Task 4: Expose the new bridge methods

**Files:**
- Modify: `src-tauri/src/bridge_bootstrap.js`
- Modify: `scripts/prepare-resources/bridge-bootstrap-updater-contract.test.mjs`

**Step 1:** Add bridge command identifiers for get/set channel.

**Step 2:** Expose `getUpdateChannel()` and `setUpdateChannel(channel)` on `window.astrbotAppUpdater`.

**Step 3:** Run the JS contract test and keep it green.

### Task 5: Verify the end-to-end contract

**Files:**
- Review: `docs/plans/2026-03-07-dual-channel-updater-design.md`
- Review: `docs/plans/2026-03-07-dual-channel-updater.md`

**Step 1:** Run focused Rust, Node, and Python tests.

**Step 2:** Review the workflow diff for stable/nightly asset naming.

**Step 3:** Summarize any follow-up UI work separately from the updater core changes.
