# AstrBot Desktop Refactor Hygiene Sweep Design

## Goal

Clean up the post-refactor codebase without another large architectural move: keep behavior stable, remove low-value duplication and dead code, align documentation with implementation, and fix the most credible latent bugs discovered during review.

## Why This Should Not Be A Large Refactor

The repository does not currently show signs that justify another wide module migration:

- subsystem grouping introduced by `d3a8bcc` is still holding
- recent changes are concentrated in updater and release-flow slices rather than across the whole runtime
- current risk comes more from local responsibility creep and doc drift than from a broken top-level architecture

That makes a conservative sweep the right tradeoff: small diffs, behavior-preserving refactors, and targeted bug prevention.

## Problems Identified

### 1. Updater bridge command flow is repeating mode-gating logic

`src-tauri/src/bridge/commands.rs` now contains two near-parallel flows for app-update check and install. Both branch on `DesktopUpdateMode`, log mode-specific messages, and map mode-specific results.

This is still manageable, but it increases the chance of future drift where one path gets a bug fix and the other does not.

### 2. Desktop state path resolution is duplicated

Both `src-tauri/src/shell_locale.rs` and `src-tauri/src/update_channel.rs` independently resolve `desktop_state.json` from `ASTRBOT_ROOT` or packaged root fallback.

The duplication is small, but it is the kind that silently diverges over time because both modules operate on the same state file.

### 3. CI normalization has a latent strict-mode bug

`scripts/ci/normalize_release_artifact_filenames.py` attempts to print strict unmatched-artifact errors to `sys.stderr` but does not import `sys`.

That means `--strict-unmatched` can fail with an unrelated `NameError` and traceback instead of surfacing a clean release-artifact validation error.

### 4. Small dead-code and naming drift exists in CI helpers

`scripts/ci/lib/release_artifacts.py` contains an unused `ReleaseArtifactError` type.

This is minor, but it is exactly the kind of dead code worth removing during a hygiene pass.

### 5. Docs are behind the implementation

The main docs do not yet fully describe:

- `bridge/updater_messages.rs`, `bridge/updater_mode.rs`, and `bridge/updater_types.rs`
- `update_channel.rs` and persisted `updateChannel` state
- updater-related environment variables and the Linux manual-download/AppImage split

This makes the codebase harder to navigate than the implementation itself.

## Recommended Approach

### Approach A: Documentation-only cleanup

Pros:

- lowest risk
- quick to review

Cons:

- leaves real code duplication in place
- does not fix the strict-mode CI bug

### Approach B: Conservative hygiene sweep (recommended)

Pros:

- fixes real latent bugs
- reduces small but growing duplication
- keeps runtime and release behavior stable
- aligns docs with the actual code layout

Cons:

- touches multiple layers in one pass
- still requires careful verification across Rust and Python paths

### Approach C: Continue subsystem migration now

Pros:

- creates cleaner long-term ownership boundaries

Cons:

- too much change for the current problem set
- higher merge and regression risk than the issues warrant

## Proposed Changes

### 1. Refactor updater bridge mode short-circuiting

Extract small helper functions in `src-tauri/src/bridge/commands.rs` so update check and install share the same mode-resolution framing instead of open-coded parallel branches.

This is intentionally a local refactor, not a module split.

### 2. Introduce a shared desktop state path helper

Create a small shared helper module for resolving `desktop_state.json` so `shell_locale` and `update_channel` stop carrying duplicate path logic.

This keeps the state-file contract in one place without forcing a larger persistence abstraction.

### 3. Add regression coverage for strict artifact normalization

Add a test that runs `normalize_release_artifact_filenames.py --strict-unmatched` against an unmatched file and asserts that the script fails cleanly without traceback noise.

Then fix the missing `sys` import and remove dead code while staying behavior-compatible.

### 4. Update core maintenance docs

Refresh the main docs so they describe the updater modules, desktop state usage, and updater-related environment variables that exist today.

Historical plan documents remain historical records and do not need to be rewritten.

## Potential Bugs To Verify While Working

- strict release-artifact normalization currently risks a `NameError` in strict mode
- updater check/install control flow can drift because of duplicated mode branches
- state-file handling can drift because locale and update-channel resolution duplicate the same path rules
- docs currently under-describe updater env vars and module responsibilities, which increases maintenance mistakes

## Testing Strategy

- Rust: focused unit tests for extracted helpers, then full `cargo test --manifest-path src-tauri/Cargo.toml --locked`
- Node/Python integration: targeted `node --test scripts/ci/release-updater-artifacts.test.mjs`, then `pnpm run test:prepare-resources`
- Python syntax: `python3 -m py_compile scripts/ci/normalize_release_artifact_filenames.py scripts/ci/generate_tauri_latest_json.py`

## Success Criteria

- no large-scale directory or API churn
- duplicated updater mode branching is reduced in `bridge/commands.rs`
- `desktop_state.json` path resolution has one shared source of truth
- strict artifact normalization fails cleanly and predictably
- dead code identified in this slice is removed
- docs accurately reflect current updater modules, env vars, and desktop state behavior
