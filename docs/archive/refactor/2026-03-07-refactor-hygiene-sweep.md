# Refactor Hygiene Sweep Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Perform a conservative post-refactor cleanup that removes small duplication and dead code, fixes a latent CI bug, and realigns maintenance docs with the current implementation.

**Architecture:** Keep the current subsystem layout intact. Apply only small refactors inside existing slices: share `desktop_state.json` path resolution, de-duplicate updater mode short-circuit logic in bridge commands, and clean CI helpers without changing release semantics.

**Tech Stack:** Rust, Tauri 2, Python 3, Node.js test runner, Markdown docs

---

### Task 1: Add regression coverage for updater bridge short-circuit helpers

**Files:**
- Modify: `src-tauri/src/bridge/commands.rs`
- Review: `src-tauri/src/bridge/updater_messages.rs`
- Review: `src-tauri/src/bridge/updater_mode.rs`
- Review: `src-tauri/src/bridge/updater_types.rs`

**Step 1: Write the failing test**

Add focused unit tests in `src-tauri/src/bridge/commands.rs` for small helper behavior:

- update check returns manual-download result when mode is `ManualDownload`
- update check returns unsupported error when mode is `Unsupported`
- update install returns manual-download error when mode is `ManualDownload`
- update install returns unsupported error when mode is `Unsupported`

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --locked updater_check_mode` 
Expected: FAIL because the helper-backed tests do not exist yet or the helper functions are missing.

**Step 3: Write minimal implementation**

In `src-tauri/src/bridge/commands.rs`, extract small helper functions that centralize mode short-circuit behavior for check/install before the updater is constructed.

Keep:

- existing log messages
- existing return payload shapes
- existing native-updater behavior unchanged

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --locked updater_`
Expected: PASS for the new updater helper tests.

### Task 2: Share desktop state path resolution

**Files:**
- Create: `src-tauri/src/desktop_state.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/shell_locale.rs`
- Modify: `src-tauri/src/update_channel.rs`

**Step 1: Write the failing test**

Add focused tests near the new helper for:

- `ASTRBOT_ROOT` overrides packaged-root fallback
- empty `ASTRBOT_ROOT` falls back to packaged root
- packaged root resolves to `<root>/data/desktop_state.json`

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --locked desktop_state`
Expected: FAIL because the shared helper module does not exist yet.

**Step 3: Write minimal implementation**

Create `src-tauri/src/desktop_state.rs` with a shared resolver for the `desktop_state.json` path, then switch both `shell_locale` and `update_channel` to call it.

Do not expand the module into a full persistence layer in this pass.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --locked desktop_state`
Expected: PASS.

### Task 3: Fix strict unmatched-artifact failure path and remove dead code

**Files:**
- Modify: `scripts/ci/normalize_release_artifact_filenames.py`
- Modify: `scripts/ci/lib/release_artifacts.py`
- Modify: `scripts/ci/release-updater-artifacts.test.mjs`

**Step 1: Write the failing test**

Add a regression test to `scripts/ci/release-updater-artifacts.test.mjs` that:

- creates an unmatched artifact file
- runs `python3 -m scripts.ci.normalize_release_artifact_filenames --strict-unmatched`
- asserts non-zero exit status
- asserts stderr mentions unmatched naming without a Python traceback

**Step 2: Run test to verify it fails**

Run: `node --test scripts/ci/release-updater-artifacts.test.mjs`
Expected: FAIL because strict unmatched mode currently references `sys.stderr` without importing `sys`.

**Step 3: Write minimal implementation**

- import `sys` in `scripts/ci/normalize_release_artifact_filenames.py`
- keep the existing strict-mode error contract intact
- remove the unused `ReleaseArtifactError` from `scripts/ci/lib/release_artifacts.py`

**Step 4: Run test to verify it passes**

Run: `node --test scripts/ci/release-updater-artifacts.test.mjs`
Expected: PASS.

### Task 4: Align maintenance docs with the current implementation

**Files:**
- Modify: `docs/architecture.md`
- Modify: `docs/repository-structure.md`
- Modify: `docs/environment-variables.md`

**Step 1: Update architecture and structure docs**

Document the current updater-related modules and responsibilities:

- `bridge/updater_messages.rs`
- `bridge/updater_mode.rs`
- `bridge/updater_types.rs`
- `update_channel.rs`
- shared `desktop_state.json` path handling

**Step 2: Update environment variable docs**

Add the updater-related variables that now exist in code or workflow behavior:

- `ASTRBOT_DESKTOP_MANUAL_DOWNLOAD_URL`
- `ASTRBOT_DESKTOP_UPDATER_STABLE_ENDPOINT`
- `ASTRBOT_DESKTOP_UPDATER_NIGHTLY_ENDPOINT`
- `ASTRBOT_DESKTOP_UPDATER_PUBLIC_KEY`

**Step 3: Verify docs reflect real code names**

Cross-check docs against the actual file names and env var identifiers in the repository.

### Task 5: Final verification

**Files:**
- Review: `src-tauri/src/bridge/commands.rs`
- Review: `src-tauri/src/desktop_state.rs`
- Review: `src-tauri/src/shell_locale.rs`
- Review: `src-tauri/src/update_channel.rs`
- Review: `scripts/ci/normalize_release_artifact_filenames.py`
- Review: `scripts/ci/lib/release_artifacts.py`
- Review: `scripts/ci/release-updater-artifacts.test.mjs`
- Review: `docs/architecture.md`
- Review: `docs/repository-structure.md`
- Review: `docs/environment-variables.md`

**Step 1: Run Rust verification**

Run:

- `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`
- `cargo test --manifest-path src-tauri/Cargo.toml --locked`

Expected: PASS.

**Step 2: Run script verification**

Run:

- `node --test scripts/ci/release-updater-artifacts.test.mjs`
- `pnpm run test:prepare-resources`
- `python3 -m py_compile scripts/ci/normalize_release_artifact_filenames.py scripts/ci/generate_tauri_latest_json.py`

Expected: PASS.

**Step 3: Review git diff for scope control**

Confirm the diff stays within conservative cleanup boundaries:

- no large subsystem migration
- no frontend contract change
- no release artifact naming behavior change beyond the strict-mode bug fix
