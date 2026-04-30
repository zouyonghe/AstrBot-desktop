# Desktop Plugin Dependency Boundary Implementation Plan

**Goal:** Prevent packaged desktop plugin installs from replacing bundled backend runtime dependencies.

**Architecture:** Generate a runtime core dependency lock during desktop backend packaging, expose it through an environment variable at launch, and teach AstrBot's packaged pip installer to constrain and skip locked core modules. This keeps plugin-only dependencies installable while preventing unsafe in-process replacement of backend dependencies.

**Tech Stack:** Node.js resource preparation scripts, Python launcher/runtime code, AstrBot pip installer tests with pytest, Node `node:test`.

---

### Task 1: Generate the Desktop Runtime Core Lock

**Files:**
- Modify: `scripts/backend/build-backend.mjs`
- Create: `scripts/backend/templates/generate_runtime_core_lock.py`
- Test: `scripts/backend/runtime-core-lock.test.mjs`

**Step 1: Write the failing test**

Add a Node test that creates a fake runtime Python command, runs the lock generator entry point, and expects `runtime-core-lock.json` to be written under the backend app directory.

**Step 2: Run test to verify it fails**

Run: `node --test scripts/backend/runtime-core-lock.test.mjs`
Expected: FAIL because the helper/export does not exist yet.

**Step 3: Write minimal implementation**

Add a Python helper that uses `importlib.metadata.distributions()` to collect:
- distribution name
- distribution version
- `top_level.txt` modules when present

Call it from `build-backend.mjs` after runtime dependency installation and before the manifest is written.

**Step 4: Run test to verify it passes**

Run: `node --test scripts/backend/runtime-core-lock.test.mjs`
Expected: PASS.

### Task 2: Expose the Lock to the Packaged Backend

**Files:**
- Modify: `scripts/backend/templates/launch_backend.py`
- Test: `scripts/prepare-resources/backend-runtime.test.mjs` or a focused launcher template test

**Step 1: Write the failing test**

Add a test that verifies the launcher template contains `ASTRBOT_DESKTOP_CORE_LOCK_PATH` and points it at `APP_DIR / "runtime-core-lock.json"` only when the file exists.

**Step 2: Run test to verify it fails**

Run: `pnpm run test:prepare-resources`
Expected: FAIL because the launcher does not set the env var yet.

**Step 3: Write minimal implementation**

Set `os.environ["ASTRBOT_DESKTOP_CORE_LOCK_PATH"]` in `launch_backend.py` before `runpy.run_path()` when the lock file exists.

**Step 4: Run test to verify it passes**

Run: `pnpm run test:prepare-resources`
Expected: PASS.

### Task 3: Apply Lock Constraints in the Pip Installer

**Files:**
- Modify: `vendor/AstrBot/astrbot/core/utils/core_constraints.py`
- Modify: `vendor/AstrBot/astrbot/core/utils/pip_installer.py`
- Test: `vendor/AstrBot/tests/test_pip_installer.py`

**Step 1: Write the failing test**

Add a test that sets `ASTRBOT_DESKTOP_CORE_LOCK_PATH` to a JSON lock containing `openai==2.32.0`, then verifies `PipInstaller.install(package_name="Cua")` adds `-c <constraints file>` and the constraints include `openai==2.32.0`.

**Step 2: Run test to verify it fails**

Run: `cd vendor/AstrBot && uv run pytest tests/test_pip_installer.py::<test_name> -q`
Expected: FAIL because the lock is ignored.

**Step 3: Write minimal implementation**

Teach `CoreConstraintsProvider` to merge existing core metadata constraints with locked desktop runtime constraints in packaged mode.

**Step 4: Run test to verify it passes**

Run: `cd vendor/AstrBot && uv run pytest tests/test_pip_installer.py::<test_name> -q`
Expected: PASS.

### Task 4: Skip Locked Modules During Post-Install Preference

**Files:**
- Modify: `vendor/AstrBot/astrbot/core/utils/pip_installer.py`
- Test: `vendor/AstrBot/tests/test_pip_installer.py`

**Step 1: Write the failing test**

Add a test where candidate modules include `openai` and `cua_agent`, the lock marks `openai` as a core module, and `_ensure_plugin_dependencies_preferred` calls `_ensure_preferred_modules` only with `cua_agent`.

**Step 2: Run test to verify it fails**

Run: `cd vendor/AstrBot && uv run pytest tests/test_pip_installer.py::<test_name> -q`
Expected: FAIL because locked modules are still preferred.

**Step 3: Write minimal implementation**

Filter candidate modules using the lock's top-level module set before attempting `_prefer_module_from_site_packages`.

**Step 4: Run test to verify it passes**

Run: `cd vendor/AstrBot && uv run pytest tests/test_pip_installer.py::<test_name> -q`
Expected: PASS.

### Task 5: Full Verification and PR

**Files:**
- All touched files

**Step 1: Run focused tests**

Run:
- `pnpm run test:prepare-resources`
- `cd vendor/AstrBot && uv run pytest tests/test_pip_installer.py -q`

**Step 2: Run repository tests if feasible**

Run:
- `make test`

**Step 3: Commit and open PR**

Commit only intentional files, push `codex/desktop-plugin-dependency-lock`, and create a draft PR against `AstrBotDevs/AstrBot-desktop:main`.
