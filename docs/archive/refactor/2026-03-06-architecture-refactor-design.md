# AstrBot Desktop Architecture Refactor Design

## Goal

Reduce long-term maintenance cost by evolving the repository from "many flat modules" to a clearer subsystem architecture, while keeping runtime behavior, build flow, and release flow stable.

## Current State

The repository has already completed a successful first-stage refactor:

- `src-tauri/src/main.rs` is now a thin entrypoint.
- Rust runtime logic is split into many focused modules.
- `scripts/prepare-resources.mjs` is already an orchestration layer instead of a large all-in-one script.
- Architecture and refactor history are documented in `docs/architecture.md` and `docs/archive/refactor/`.

This means the main problem is no longer "one huge file". The next problem is that many modules still live at the same architectural level, which makes the system harder to read as a whole.

## Problems To Address

### 1. Flat module layout hides subsystem boundaries

Rust modules are small enough individually, but `src-tauri/src` still behaves like a large flat namespace. It is easy to answer "what does this file do?" and harder to answer "which subsystem owns this behavior?".

### 2. `BackendState` is becoming a behavior hub

A large amount of backend logic is implemented as `impl BackendState` across multiple files. This keeps call sites convenient, but it also makes `BackendState` the default home for new behavior. Over time that risks recreating a God object with better file organization.

### 3. Runtime event registration and business flow logic are still close together

`src-tauri/src/app_runtime.rs` is reasonably small, but it still mixes Tauri event wiring with startup/loading/bridge behavior decisions. Future feature work can easily push it back toward a central coordinator with too many responsibilities.

### 4. Script-side compatibility rules are growing as hard-coded logic

`scripts/prepare-resources/desktop-bridge-checks.mjs` contains patterns, severity policy, tagged-release branching, and reporting decisions in one file. This is manageable now, but it will grow poorly if more compatibility checks are added.

### 5. Documentation is strong on module inventory, weaker on flow-level understanding

The current docs explain file responsibilities well. They do not yet serve as the best reference for the most important runtime flows: startup, restart, exit, and page-load bridge injection.

## Target Architecture

### Rust side

Move from a flat module list to subsystem-oriented grouping.

Proposed logical subsystems:

- `app/`: builder setup, shared app types, top-level constants, thin runtime helpers
- `backend/`: config, launch, readiness, restart, process lifecycle, HTTP access
- `tray/`: tray setup, menu actions, labels, event routing
- `window/`: window actions, main window operations, startup loading behavior
- `bridge/`: desktop bridge injection, bridge IPC commands, origin policy
- `lifecycle/`: exit events, exit cleanup, exit state coordination
- `support/`: logging, path helpers, HTTP parsing, locale utilities, packaged web UI helpers

This design is about ownership and navigation, not just moving files into folders.

### Backend domain design

Keep `BackendState` as the shared runtime state container, but stop making it the default home for orchestration logic.

Refactor backend behavior toward a few explicit roles:

- `BackendController`: start/stop/restart orchestration
- `BackendProbe`: ping/status/start-time/readiness helpers
- `BackendAuthState`: restart auth token handling and normalization
- `RestartCoordinator`: strategy resolution and fallback flow

These can begin as modules and helper functions before becoming separate structs if that is sufficient.

### Script-side design

Keep `scripts/prepare-resources.mjs` as orchestration-only.

Refactor toward:

- a single validated context/config object built from CLI args + env vars
- mode dispatch via handler map instead of condition chains
- compatibility checks split into rule data + runner + reporting policy

This keeps behavior stable while making script growth easier to reason about and test.

## Proposed Refactor Approaches

### Approach A: Continue splitting large files only

Pros:

- low risk per change
- easy to review

Cons:

- does not solve subsystem discoverability
- may increase fragmentation without improving architecture

### Approach B: Full directory reorganization first

Pros:

- quickly creates visible structure
- helps future code placement

Cons:

- produces a large, noisy diff
- higher merge conflict risk
- weak if behavior/API boundaries are not clarified first

### Approach C: Recommended incremental boundary refactor

Sequence:

1. document target subsystem boundaries and dependency rules
2. extract small flow-level facades/helpers under current layout
3. add tests around new boundaries
4. move modules into subsystem directories in small batches
5. update docs after each stable slice

Pros:

- keeps changes reviewable
- improves architecture before cosmetic moves
- reduces regressions by preserving behavior and adding tests first

Cons:

- takes multiple iterations
- requires discipline to avoid ad-hoc shortcuts

## Recommended Execution Order

### Phase 1: Script orchestration cleanup

- add tests around mode dispatch and validation
- move mode handling in `prepare-resources.mjs` to a dedicated dispatch module
- introduce a single context object for the prepare flow

This is the safest first slice because it is narrow, already modular, and easy to verify with existing Node tests.

### Phase 2: Backend restart flow cleanup

- separate strategy resolution from restart execution
- isolate graceful restart waiting logic from managed restart fallback
- keep current public behavior intact

This targets one of the densest Rust modules without forcing a full directory move.

### Phase 3: Runtime registration cleanup

- split `app_runtime.rs` into registration helpers for plugins, window events, page-load hooks, setup, and run events
- keep the builder flow readable and declarative

### Phase 4: Subsystem directory migration

- move stable modules into grouped directories
- update `main.rs`, `architecture.md`, and `repository-structure.md`
- avoid mixing API redesign and directory moves in the same slice where possible

## Dependency Rules

The refactor should enforce a few simple rules:

- entrypoints (`main.rs`, orchestration scripts) coordinate but do not own heavy logic
- tray/window/bridge layers call backend facades, not low-level backend internals
- low-level helpers do not depend back on UI/tray behavior
- tests should target behavior at the new seam, not internal implementation details

## Testing Strategy

- For scripts: `node --test "scripts/**/*.test.mjs"`
- For Rust slices: focused unit tests first, then targeted `cargo test --locked <module>` or full `cargo test --locked` when practical
- Add tests before moving logic where possible, especially for restart strategy, dispatch, and validation behavior

## Success Criteria

The refactor is successful when:

- new behavior has an obvious subsystem home
- `BackendState` stops accumulating orchestration responsibilities by default
- entrypoint/orchestration files stay small and declarative
- script-side rule growth becomes data-driven rather than branch-driven
- docs explain both module ownership and main runtime flows
