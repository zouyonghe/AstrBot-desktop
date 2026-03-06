# Release Updater Artifacts Implementation Plan

> **Execution note:** Implement this plan task-by-task and verify each step before moving on.

**Goal:** 修复 GitHub Actions release job，使其能拿到正确的 updater 签名与 macOS updater bundle，并成功生成 `latest.json`。

**Architecture:** 保持现有 release job 结构不变，只补齐缺失的 updater 产物上传，并让命名规范化脚本和 `latest.json` 生成脚本识别当前 canonical 文件名。macOS updater 使用 Tauri 产出的 `.app.tar.gz` / `.sig`，Windows updater 使用 NSIS `.exe` / `.sig`。

**Tech Stack:** GitHub Actions YAML, Python CI scripts, Node `node:test`

---

### Task 1: Add regression coverage for updater artifact flow

**Files:**
- Create: `scripts/ci/release-updater-artifacts.test.mjs`

**Step 1:** 写一个失败用例，模拟 release job 下载后的产物目录，覆盖 Windows `.exe.sig`、macOS `.app.tar.gz.sig`、nightly 命名和 `latest.json` 生成。

**Step 2:** 运行 `node --test scripts/ci/release-updater-artifacts.test.mjs`，确认在修复前失败。

### Task 2: Fix CI scripts for canonical updater artifact names

**Files:**
- Modify: `scripts/ci/normalize-release-artifact-filenames.py`
- Modify: `scripts/ci/generate-tauri-latest-json.py`

**Step 1:** 让规范化脚本支持 `.sig` 与 macOS `.app.tar.gz` updater bundle。

**Step 2:** 让 `latest.json` 生成脚本支持当前 canonical Windows / macOS updater 资产名称，并兼容旧格式。

**Step 3:** 重新运行单测，确认通过。

### Task 3: Fix workflow artifact upload

**Files:**
- Modify: `.github/workflows/build-desktop-tauri.yml`

**Step 1:** Windows job 上传 `.exe` 与 `.exe.sig`。

**Step 2:** macOS job 收集并上传带版本/架构信息的 `.app.tar.gz` 与 `.app.tar.gz.sig`，同时保留现有 zip 发布包。

**Step 3:** 检查 release job 的下载、规范化与 `latest.json` 生成链路仍然匹配。

### Task 4: Verify targeted behavior

**Files:**
- No new files required

**Step 1:** 运行 `node --test scripts/ci/release-updater-artifacts.test.mjs`。

**Step 2:** 运行 `pnpm run test:prepare-resources`，确认现有 JS 测试未回归。

**Step 3:** 复查 workflow 关键 globs 与脚本输出，确保 release job 能上传 `latest.json` 所需资产。
