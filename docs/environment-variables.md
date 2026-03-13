# AstrBot Desktop 环境变量清单

更新时间：2026-03-13  
主要来源：桌面运行时 Rust 模块（如 `src-tauri/src/backend/config.rs`、`src-tauri/src/update_channel.rs`、`src-tauri/src/bridge/updater_messages.rs`、`src-tauri/src/runtime_paths.rs`、`src-tauri/src/launch_plan.rs`）、资源准备脚本（`scripts/prepare-resources*.mjs`）和发布工作流（`.github/workflows/build-desktop-tauri.yml`）。以下按主要解析/写入阶段分组。

## 1. 桌面运行时直接读取（`src-tauri`）

| 变量 | 用途 | 默认值/行为 |
| --- | --- | --- |
| `ASTRBOT_BACKEND_URL` | 后端基础 URL | 默认 `http://127.0.0.1:6185/` |
| `ASTRBOT_BACKEND_AUTO_START` | 是否自动拉起后端 | 默认 `1`（启用） |
| `ASTRBOT_BACKEND_TIMEOUT_MS` | 后端就绪等待超时 | 开发模式默认 `20000`；打包模式默认回退 `300000` |
| `ASTRBOT_BACKEND_READY_HTTP_PATH` | 就绪探针 HTTP 路径 | 默认 `/api/stat/start-time` |
| `ASTRBOT_BACKEND_READY_PROBE_TIMEOUT_MS` | 就绪探针单次超时 | 默认回退到 `ASTRBOT_BACKEND_PING_TIMEOUT_MS` |
| `ASTRBOT_BACKEND_READY_POLL_INTERVAL_MS` | 就绪轮询间隔 | 默认 `300`，并按边界 clamp |
| `ASTRBOT_BACKEND_PING_TIMEOUT_MS` | 后端 ping 超时 | 默认 `800`，范围 `50~30000` |
| `ASTRBOT_BRIDGE_BACKEND_PING_TIMEOUT_MS` | 桥接层 ping 超时 | 默认回退到 `ASTRBOT_BACKEND_PING_TIMEOUT_MS` |
| `ASTRBOT_BACKEND_CMD` | 后端启动命令覆盖 | 未设置则按 launch plan 推导 |
| `ASTRBOT_BACKEND_CWD` | 后端工作目录覆盖 | 未设置则按 launch plan 推导 |
| `ASTRBOT_WEBUI_DIR` | WebUI 目录覆盖 | 未设置则按资源目录推导 |
| `ASTRBOT_ROOT` | AstrBot 根目录 | 未设置则按打包/临时目录回退 |
| `ASTRBOT_DASHBOARD_HOST` | 后端读取的 dashboard host 变量 | 若 `DASHBOARD_HOST` 与本变量都未设置，打包态桌面默认写入 `DASHBOARD_HOST=127.0.0.1` |
| `ASTRBOT_DASHBOARD_PORT` | 后端读取的 dashboard port 变量 | 若 `DASHBOARD_PORT` 与本变量都未设置，打包态桌面默认写入 `DASHBOARD_PORT=6185` |
| `ASTRBOT_DESKTOP_EXTRA_PATH` | 启动后端时追加 PATH | 未设置则不追加 |
| `ASTRBOT_DESKTOP_LOCALE` | 托盘/壳层文案语言 | 默认 `zh-CN` |
| `ASTRBOT_DESKTOP_LOG_PATH` | 桌面日志文件路径覆盖 | 未设置则回退到 `ASTRBOT_ROOT/logs/desktop.log` 或临时目录 |
| `ASTRBOT_DESKTOP_MANUAL_DOWNLOAD_URL` | manual-download reason 文案里的下载地址 | 默认 `https://github.com/AstrBotDevs/AstrBot-desktop/releases/latest` |
| `ASTRBOT_DESKTOP_STARTUP_MODE` | 启动画面模式提示 | 未设置则自动判定 `loading/panel-update` |
| `ASTRBOT_DESKTOP_UPDATER_STABLE_ENDPOINT` | stable 通道 manifest URL 覆盖 | 未设置则读 `plugins.updater.channelEndpoints.stable`，再回退 `plugins.updater.endpoints[0]` |
| `ASTRBOT_DESKTOP_UPDATER_NIGHTLY_ENDPOINT` | nightly 通道 manifest URL 覆盖 | 未设置则读 `plugins.updater.channelEndpoints.nightly` |

## 2. 源码与资源准备（开发态运行时 / `prepare-resources` / backend build）

| 变量 | 用途 | 默认值/行为 |
| --- | --- | --- |
| `ASTRBOT_SOURCE_DIR` | 源码目录覆盖 | 运行时未设置则自动探测；脚本未设置则用 `vendor/AstrBot` |
| `ASTRBOT_SOURCE_GIT_URL` | 资源准备时源仓库 URL | 默认 `https://github.com/AstrBotDevs/AstrBot.git` |
| `ASTRBOT_SOURCE_GIT_REF` | 资源准备时源仓库 ref | 默认空（不强制切 ref） |
| `ASTRBOT_SOURCE_GIT_REF_IS_COMMIT` | 将 ref 明确标记为 commit | 默认关闭 |
| `ASTRBOT_SOURCE_FORCE_CHECKOUT` | 强制 `git checkout -f` 覆盖本地改动（CI 默认启用） | 默认关闭 |
| `ASTRBOT_DESKTOP_VERSION` | 桌面版本号覆盖 | 默认读取源码 `pyproject.toml` |
| `ASTRBOT_DESKTOP_RELEASE_BASE_URL` | dashboard release 跳转基地址 | 默认 `https://github.com/AstrBotDevs/AstrBot-desktop/releases` |
| `ASTRBOT_DESKTOP_STRICT_BRIDGE_EXPECTATIONS` | bridge 产物校验严格模式 | 默认关闭 |
| `ASTRBOT_PBS_RELEASE` | python-build-standalone release | 默认 `20260211` |
| `ASTRBOT_PBS_VERSION` | python-build-standalone Python 版本 | 默认 `3.12.12` |
| `ASTRBOT_DESKTOP_BACKEND_RUNTIME` | 外部后端 runtime 根目录 | 存在时优先使用 |
| `ASTRBOT_DESKTOP_CPYTHON_HOME` | 外部 CPython 根目录 | 作为 bundled runtime 回退 |
| `ASTRBOT_DESKTOP_TARGET_ARCH` | 显式指定资源准备阶段要打包的桌面目标架构 | 默认空；未设置时回退到当前 Node 进程架构，CI 建议显式传 `amd64` 或 `arm64` |
| `ASTRBOT_DESKTOP_WINDOWS_ARM_BACKEND_ARCH` | Windows ARM64 构建时覆盖 bundled backend Python 架构 | 默认空；在 Windows ARM64 上默认为 `amd64`，可显式设为 `amd64`/`x64` 或 `arm64`/`aarch64` |

## 3. 桌面进程写入给后端子进程

| 变量 | 用途 | 默认值/行为 |
| --- | --- | --- |
| `ASTRBOT_DESKTOP_CLIENT` | 标记桌面客户端环境 | 打包态启动后端时写入 `1` |

## 4. 发布/CI（GitHub Actions）

| 变量 | 用途 | 默认值/行为 |
| --- | --- | --- |
| `ASTRBOT_DESKTOP_UPDATER_PUBLIC_KEY` | updater 公钥透传到构建步骤 | 默认空；当前由 `.github/workflows/build-desktop-tauri.yml` 传递，Rust 运行时不直接解析 |
| `ASTRBOT_DESKTOP_TARGET_ARCH` | 透传矩阵目标架构给资源准备脚本 | 默认空；Windows workflow 当前会传 `matrix.arch`，避免在 WOA 上误用仿真层 Node 的 `process.arch` |
| `ASTRBOT_DESKTOP_WINDOWS_ARM_BACKEND_ARCH` | 透传 Windows ARM64 backend runtime 架构覆盖配置到构建步骤 | 默认空；具体取值与默认行为见第 2 节 |

## 5. 维护约定

- 新增 `ASTRBOT_*` 变量时，必须同步更新本文件与对应模块注释。
- 变量解析与边界逻辑建议集中在 `src-tauri/src/backend/config.rs`、`src-tauri/src/update_channel.rs`、`src-tauri/src/bridge/updater_messages.rs` 或 `scripts/prepare-resources/*` 子模块，避免散落在入口文件。
