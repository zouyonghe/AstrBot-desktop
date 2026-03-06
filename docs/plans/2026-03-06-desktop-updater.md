# Desktop Updater Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a real desktop self-update flow that satisfies the WebUI `astrbotAppUpdater` contract and works with the existing GitHub Release pipeline.

**Architecture:** Use Tauri updater as the runtime update engine, expose it through a dedicated desktop bridge object, and extend release automation to publish signed updater metadata. Keep WebUI behavior unchanged except that desktop updater actions now map to real bridge functionality.

**Tech Stack:** Tauri 2, Rust, JavaScript bridge bootstrap, GitHub Actions, GitHub Releases

---

### Task 1: Document the updater bridge contract

**Files:**
- Create: `docs/plans/2026-03-06-desktop-updater-design.md`
- Modify: `vendor/AstrBot/dashboard/src/types/desktop-bridge.d.ts`
- Modify: `docs/architecture.md`

**Step 1: Confirm the bridge surface expected by WebUI**

Record the exact `astrbotAppUpdater` methods and return shapes expected by the upstream dashboard.

**Step 2: Write down the desktop mapping**

Describe which Tauri commands back each updater bridge method.

**Step 3: Verify docs and types stay aligned**

Check that the documented contract and TypeScript declaration match.

### Task 2: Add the updater plugin to the desktop runtime

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/app_runtime.rs`

**Step 1: Write the failing build/test expectation**

Add the plugin dependency and wire it into runtime setup so compilation fails until imports and builder configuration are correct.

**Step 2: Run Rust checks**

Run: `cargo check --manifest-path src-tauri/Cargo.toml --locked`

Expected: FAIL before the plugin setup is complete.

**Step 3: Add minimal plugin registration**

Register the Tauri updater plugin in the same runtime setup path as other plugins.

**Step 4: Re-run Rust checks**

Run: `cargo check --manifest-path src-tauri/Cargo.toml --locked`

Expected: PASS.

### Task 3: Add updater commands to the bridge

**Files:**
- Modify: `src-tauri/src/bridge/commands.rs`
- Modify: `src-tauri/src/app_runtime.rs`
- Test: Rust tests in `src-tauri/src/bridge/commands.rs` or nearby pure helper module

**Step 1: Write failing tests for updater result mapping**

Cover at least:

- no update available
- update available with version metadata
- updater check failure
- install failure mapping

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --locked updater`

Expected: FAIL because updater commands/helpers are missing.

**Step 3: Implement minimal updater commands**

Add bridge commands for:

- current version
- check for update
- install update

**Step 4: Register commands in invoke handler**

Update `src-tauri/src/app_runtime.rs` to expose the new bridge commands.

**Step 5: Re-run focused tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --locked updater`

Expected: PASS.

### Task 4: Inject `window.astrbotAppUpdater`

**Files:**
- Modify: `src-tauri/src/bridge_bootstrap.js`
- Modify: `src-tauri/src/bridge/desktop.rs`
- Modify: `vendor/AstrBot/dashboard/src/types/desktop-bridge.d.ts`

**Step 1: Write the failing contract test if practical**

If there is no existing JS contract test harness, add a small bootstrap verification test or script-level check for the injected object shape.

**Step 2: Implement the updater bridge object**

Expose the updater methods on `window.astrbotAppUpdater` and route them to Tauri commands.

**Step 3: Keep existing `astrbotDesktop` behavior unchanged**

Do not regress current desktop runtime behavior while adding the new updater object.

**Step 4: Run relevant checks**

Run:
- `cargo test --manifest-path src-tauri/Cargo.toml --locked`

Expected: PASS.

### Task 5: Add updater config to Tauri

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: docs if environment/secrets are documented

**Step 1: Add updater configuration**

Wire updater endpoints and the public key into the Tauri config.

**Step 2: Verify config is loadable**

Run: `cargo check --manifest-path src-tauri/Cargo.toml --locked`

Expected: PASS.

**Step 3: Document secret requirements**

Record which CI secrets are required for signing and release publishing.

### Task 6: Extend release workflow for updater artifacts

**Files:**
- Modify: `.github/workflows/build-desktop-tauri.yml`
- Modify: release helper scripts if needed

**Step 1: Write workflow design notes inline or in docs**

Identify where signed updater metadata and platform assets are produced and uploaded.

**Step 2: Implement updater artifact generation**

Add the necessary build/sign/publish steps for Tauri updater.

**Step 3: Validate workflow syntax**

Run any local workflow lint/validation command available, or at minimum check YAML consistency and referenced secrets/outputs.

### Task 7: Final verification

**Files:**
- Modify: docs as needed to reflect final updater architecture

**Step 1: Run Rust verification**

Run:
- `cargo check --manifest-path src-tauri/Cargo.toml --locked`
- `cargo test --manifest-path src-tauri/Cargo.toml --locked`

Expected: PASS.

**Step 2: Run script verification**

Run:
- `pnpm run test:prepare-resources`

Expected: PASS.

**Step 3: Manual release validation checklist**

Document the exact steps to validate updater behavior with a test release and an older installed desktop build.
