# Architecture Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor AstrBot Desktop toward clearer subsystem boundaries without changing product behavior or release flow.

**Architecture:** Start with low-risk orchestration seams, especially the prepare-resources script and backend restart flow. Add tests before moving logic, then introduce explicit dispatch/facade boundaries, and only after that migrate files into subsystem-oriented directories.

**Tech Stack:** Rust, Tauri 2, Node.js ESM scripts, `node:test`, Cargo tests

---

### Task 1: Document target architecture and rules

**Files:**
- Create: `docs/plans/2026-03-06-architecture-refactor-design.md`
- Modify: `docs/architecture.md`
- Modify: `docs/repository-structure.md`

**Step 1: Write the design document**

Capture the target subsystem layout, current pain points, dependency rules, and phased execution strategy.

**Step 2: Update architecture docs with the intended direction**

Add a short section describing the planned subsystem model and flow-oriented documentation goals.

**Step 3: Verify docs are internally consistent**

Check that file/module descriptions and future directory guidance do not contradict each other.

### Task 2: Refactor prepare-resources mode dispatch with tests first

**Files:**
- Create: `scripts/prepare-resources/mode-dispatch.mjs`
- Create: `scripts/prepare-resources/mode-dispatch.test.mjs`
- Modify: `scripts/prepare-resources.mjs`

**Step 1: Write the failing test**

Create `scripts/prepare-resources/mode-dispatch.test.mjs` covering:

- valid modes: `version`, `webui`, `backend`, `all`
- unsupported mode error text
- execution behavior for `version`
- dispatch behavior for `webui`, `backend`, and `all`

**Step 2: Run test to verify it fails**

Run: `node --test scripts/prepare-resources/mode-dispatch.test.mjs`

Expected: FAIL because `./mode-dispatch.mjs` does not exist yet.

**Step 3: Write minimal implementation**

Implement a dispatch helper that validates the mode and runs the correct handler sequence. Keep it free of env access and file system side effects beyond handler invocation.

**Step 4: Wire orchestration script to the new module**

Update `scripts/prepare-resources.mjs` to delegate mode branching to the new helper.

**Step 5: Run focused tests**

Run: `node --test scripts/prepare-resources/mode-dispatch.test.mjs`

Expected: PASS.

**Step 6: Run broader script tests**

Run: `pnpm run test:prepare-resources`

Expected: PASS.

### Task 3: Introduce a prepare-resources context object

**Files:**
- Create: `scripts/prepare-resources/context.mjs`
- Create: `scripts/prepare-resources/context.test.mjs`
- Modify: `scripts/prepare-resources.mjs`

**Step 1: Write the failing test**

Cover env normalization, truthy strict-bridge parsing, and source ref resolution through a single context factory.

**Step 2: Run test to verify it fails**

Run: `node --test scripts/prepare-resources/context.test.mjs`

Expected: FAIL because the context factory does not exist.

**Step 3: Write minimal implementation**

Build a pure helper that translates CLI args and env into a normalized config object.

**Step 4: Delegate `prepare-resources.mjs` to the context builder**

Keep `prepare-resources.mjs` as orchestration-only.

**Step 5: Run focused and broader tests**

Run:
- `node --test scripts/prepare-resources/context.test.mjs`
- `pnpm run test:prepare-resources`

Expected: PASS.

### Task 4: Separate desktop bridge compatibility rules from execution policy

**Files:**
- Create: `scripts/prepare-resources/desktop-bridge-expectations.mjs`
- Modify: `scripts/prepare-resources/desktop-bridge-checks.mjs`
- Create: `scripts/prepare-resources/desktop-bridge-checks.test.mjs`

**Step 1: Write the failing test**

Test severity decisions for strict mode vs tagged release vs normal branch, and rule matching outcomes.

**Step 2: Run test to verify it fails**

Run: `node --test scripts/prepare-resources/desktop-bridge-checks.test.mjs`

Expected: FAIL before extraction.

**Step 3: Extract rules into data**

Move expectation metadata to a dedicated module.

**Step 4: Keep check runner focused on execution/reporting**

Do not change compatibility behavior.

**Step 5: Run script tests**

Run: `pnpm run test:prepare-resources`

Expected: PASS.

### Task 5: Refactor backend restart strategy into explicit seams

**Files:**
- Create: `src-tauri/src/backend_restart_strategy.rs`
- Create: `src-tauri/src/backend_restart_strategy_tests.rs` or inline tests
- Modify: `src-tauri/src/backend_restart.rs`
- Modify: `src-tauri/src/main.rs`

**Step 1: Write the failing test**

Add tests around restart strategy resolution and outcome mapping, independent from actual process launch.

**Step 2: Run test to verify it fails**

Run: `cargo test --locked compute_restart_strategy`

Expected: FAIL after moving the target API or before introducing the new module.

**Step 3: Write minimal implementation**

Move strategy selection and graceful outcome interpretation into a dedicated module or clearly separated functions.

**Step 4: Keep `restart_backend()` flow-oriented**

The function should read like a coordinator rather than a detail-heavy implementation.

**Step 5: Run focused Rust tests**

Run: `cargo test --locked backend_restart`

Expected: PASS.

### Task 6: Clean up app runtime event registration

**Files:**
- Modify: `src-tauri/src/app_runtime.rs`
- Create: helper module(s) under `src-tauri/src/`

**Step 1: Write the failing test or approval test where practical**

Prefer extracting pure decision helpers with unit tests rather than trying to test Tauri registration directly.

**Step 2: Extract event registration helpers**

Split plugin setup, window event handling, page-load handling, startup setup, and run-event handling into named functions.

**Step 3: Run focused Rust tests**

Run: `cargo test --locked`

Expected: PASS.

### Task 7: Migrate stable modules into subsystem directories

**Files:**
- Modify: `src-tauri/src/main.rs`
- Move: subsystem-owned Rust modules into grouped directories
- Modify: `docs/architecture.md`
- Modify: `docs/repository-structure.md`

**Step 1: Move one subsystem at a time**

Suggested order:

1. `tray/`
2. `window/`
3. `bridge/`
4. `lifecycle/`
5. `backend/`
6. `support/`

**Step 2: Verify compilation after each subsystem batch**

Run: `cargo check --locked`

Expected: PASS after every batch.

**Step 3: Run full tests after final batch**

Run:
- `cargo test --locked`
- `pnpm run test:prepare-resources`

Expected: PASS.

### Task 8: Final documentation pass

**Files:**
- Modify: `docs/architecture.md`
- Modify: `docs/repository-structure.md`
- Create: flow docs if needed under `docs/`

**Step 1: Document the new subsystem layout**

Make the docs match the final code shape.

**Step 2: Add runtime flow references**

Cover startup, restart, exit, and bridge injection flow at a high level.

**Step 3: Final verification**

Run:
- `cargo check --locked`
- `cargo test --locked`
- `pnpm run test:prepare-resources`

Expected: PASS.
