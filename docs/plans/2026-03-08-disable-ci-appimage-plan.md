# Disable CI AppImage Publishing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove Linux AppImage from the upstream CI and release manifest pipeline while preserving local AppImage build capability.

**Architecture:** Shrink the Linux GitHub Actions build matrix output to `deb` and `rpm`, then remove AppImage handling from updater-manifest generation and release artifact normalization tests. Keep runtime AppImage detection untouched so local/manual AppImage builds still work outside the upstream release path.

**Tech Stack:** GitHub Actions YAML, Python, Node.js tests, gh CLI

---

### Task 1: Lock the release-pipeline contract in tests

**Files:**
- Modify: `scripts/ci/test_generate_tauri_latest_json.py`
- Modify: `scripts/ci/release-updater-artifacts.test.mjs`

**Step 1: Write the failing test changes**

Remove or rewrite AppImage-specific release-manifest expectations so the suite reflects the new contract: no AppImage updater artifacts should be recognized for published releases.

**Step 2: Run tests to verify they fail**

Run:

- `python3 -m unittest scripts.ci.test_generate_tauri_latest_json`
- `node --test scripts/ci/release-updater-artifacts.test.mjs`

Expected: FAIL because the implementation still recognizes AppImage release artifacts.

### Task 2: Remove AppImage from CI and release-manifest generation

**Files:**
- Modify: `.github/workflows/build-desktop-tauri.yml`
- Modify: `scripts/ci/generate_tauri_latest_json.py`

**Step 1: Update Linux CI build output**

Make Linux CI build only `deb,rpm` bundles and remove the AppImage upload step.

**Step 2: Remove AppImage updater manifest handling**

Delete Linux AppImage platform-key, filename, parsing, and collection logic from `generate_tauri_latest_json.py`.

**Step 3: Re-run targeted tests**

Run:

- `python3 -m unittest scripts.ci.test_generate_tauri_latest_json`
- `node --test scripts/ci/release-updater-artifacts.test.mjs`

Expected: PASS.

### Task 3: Update PR context and verify the change

**Files:**
- Review/Update PR body via `gh pr edit`

**Step 1: Run final verification**

Run:

- `python3 -m unittest scripts.ci.test_generate_tauri_latest_json`
- `node --test scripts/ci/release-updater-artifacts.test.mjs`

Expected: PASS.

**Step 2: Commit and push**

```bash
git add .github/workflows/build-desktop-tauri.yml scripts/ci/generate_tauri_latest_json.py scripts/ci/test_generate_tauri_latest_json.py scripts/ci/release-updater-artifacts.test.mjs
git commit -m "build: disable CI AppImage publishing"
git push
```

**Step 3: Update upstream PR description**

Use `gh pr edit` to explain that Linux AppImage publishing is temporarily disabled in upstream CI because AppImage release compatibility still depends on Linux graphics/runtime details such as FUSE availability, Wayland/XWayland behavior, and broader target-system validation.
