# AstrBot Desktop 环境变量清单（Phase 1）

更新时间：2026-02-22  
范围：`src-tauri/src/main.rs`、`src-tauri/src/backend_config.rs`、`scripts/prepare-resources*.mjs`

## 1. 运行时（Tauri / Rust）

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
| `ASTRBOT_SOURCE_DIR` | 源码目录提示（开发态） | 未设置则自动探测 |
| `ASTRBOT_SOURCE_GIT_URL` | 源仓库 URL（开发态） | 未设置则使用默认官方仓库 |
| `ASTRBOT_SOURCE_GIT_REF` | 源仓库分支/标签/提交（开发态） | 未设置则跟随本地默认分支 |
| `ASTRBOT_DASHBOARD_HOST` | 面板 host（桥接场景） | 未设置则由后端 URL 推导 |
| `ASTRBOT_DASHBOARD_PORT` | 面板 port（桥接场景） | 未设置则由后端 URL 推导 |
| `ASTRBOT_DESKTOP_EXTRA_PATH` | 启动后端时追加 PATH | 未设置则不追加 |
| `ASTRBOT_DESKTOP_CLIENT` | 标记桌面客户端环境 | 运行时写入为 `1` |
| `ASTRBOT_DESKTOP_LOCALE` | 托盘/壳层文案语言 | 默认 `zh-CN` |
| `ASTRBOT_DESKTOP_LOG_PATH` | 桌面日志文件路径覆盖 | 未设置则回退到 `ASTRBOT_ROOT/logs/desktop.log` 或临时目录 |
| `ASTRBOT_DESKTOP_STARTUP_MODE` | 启动画面模式提示 | 未设置则自动判定 `loading/panel-update` |

## 2. 构建资源脚本（`prepare-resources`）

| 变量 | 用途 | 默认值/行为 |
| --- | --- | --- |
| `ASTRBOT_SOURCE_GIT_URL` | 资源准备时源仓库 URL | 默认 `https://github.com/AstrBotDevs/AstrBot.git` |
| `ASTRBOT_SOURCE_GIT_REF` | 资源准备时源仓库 ref | 默认空（不强制切 ref） |
| `ASTRBOT_SOURCE_GIT_REF_IS_COMMIT` | 将 ref 明确标记为 commit | 默认关闭 |
| `ASTRBOT_SOURCE_FORCE_CHECKOUT` | 强制 `git checkout -f` 覆盖本地改动（CI 默认启用） | 默认关闭 |
| `ASTRBOT_SOURCE_DIR` | 指定本地源码目录（跳过 clone/fetch） | 默认 `vendor/AstrBot` |
| `ASTRBOT_DESKTOP_VERSION` | 桌面版本号覆盖 | 默认读取源码 `pyproject.toml` |
| `ASTRBOT_DESKTOP_RELEASE_BASE_URL` | 构建 dashboard 时覆盖 release 跳转基地址（注入 `VITE_ASTRBOT_RELEASE_BASE_URL`） | 默认 `https://github.com/AstrBotDevs/AstrBot-desktop/releases` |
| `ASTRBOT_DESKTOP_STRICT_BRIDGE_EXPECTATIONS` | 桥接产物校验严格模式（关闭时按兼容模式处理，不匹配仅告警） | 默认关闭 |
| `ASTRBOT_PBS_RELEASE` | python-build-standalone release | 默认 `20260211` |
| `ASTRBOT_PBS_VERSION` | python-build-standalone Python 版本 | 默认 `3.12.12` |
| `ASTRBOT_DESKTOP_BACKEND_RUNTIME` | 外部后端运行时根目录 | 存在时优先使用 |
| `ASTRBOT_DESKTOP_CPYTHON_HOME` | 外部 CPython 根目录 | 作为 runtime 回退 |

## 3. 维护约定

- 新增 `ASTRBOT_*` 变量时，必须同步更新本文件与对应模块注释。
- 变量解析与边界逻辑优先收敛到 `src-tauri/src/backend_config.rs` 或 `scripts/prepare-resources/*` 子模块，避免散落在入口文件。
