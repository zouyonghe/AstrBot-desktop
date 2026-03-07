# AstrBot Desktop 文件组织说明

本文档用于快速定位仓库目录职责，作为日常维护与新代码落位参考。

## 1. 顶层目录

- `src-tauri/`
  - 桌面壳层核心（Rust + Tauri 配置）。
- `scripts/`
  - 构建、资源准备、CI 辅助脚本。
- `resources/`
  - 构建产物资源目录（WebUI/Backend 运行时）。
- `ui/`
  - 启动壳层静态资源。
- `.github/`
  - GitHub Actions workflows 与复用 actions。
- `docs/`
  - 用户、维护与归档文档。

## 2. Rust 侧当前布局（`src-tauri/src`）

当前 Rust 代码采用“子系统目录 + 顶层共享模块”的布局；以下职责说明以当前落点为准。

- `tray/`
  - 托盘子系统目录。
  - `tray/actions.rs`
    - 托盘菜单 ID 与动作映射。
  - `tray/bridge_event.rs`
    - 托盘重启 bridge 事件发射与 token 管理。
  - `tray/labels.rs`
    - 托盘菜单文案刷新与安全更新。
  - `tray/menu_handler.rs`
    - 托盘菜单事件动作分发与重启流程处理。
  - `tray/setup.rs`
    - 托盘初始化、菜单构建与事件绑定。
- `window/`
  - 窗口子系统目录。
  - `window/main_window.rs`
    - 主窗口 show/hide/reload/navigate 操作封装。
  - `window/actions.rs`
    - 主窗口动作与 tray 文案刷新联动封装。
  - `window/startup_loading.rs`
    - 启动页 loading mode 判定与注入逻辑。
- `lifecycle/`
  - 生命周期子系统目录。
  - `lifecycle/cleanup.rs`
    - ExitRequested/Exit fallback 清理流程与 stop-backend 分支封装。
  - `lifecycle/events.rs`
    - RunEvent 退出分支处理与清理编排。
  - `lifecycle/backend_exit_state.rs`
    - 退出状态机包装方法与锁异常日志收敛。
- `bridge/`
  - bridge 子系统目录。
  - `bridge/desktop.rs`
    - desktop bridge bootstrap 组装与注入执行。
  - `bridge/commands.rs`
    - desktop bridge IPC 命令定义与返回结构收敛，含 shell locale / updater 相关入口。
  - `bridge/origin_policy.rs`
    - bridge 注入来源判定（同源/loopback/端口策略）。
  - `bridge/updater_messages.rs`
    - updater 文案、默认手动下载 URL 与 manual-download 原因组装。
  - `bridge/updater_mode.rs`
    - 运行时 updater 模式判定（`NativeUpdater` / `ManualDownload` / `Unsupported`）。
  - `bridge/updater_types.rs`
    - updater 检查/安装/通道 IPC 返回结构与映射 helper。
- `backend/`
  - backend 子系统目录。
  - `backend/config.rs`
    - 后端配置与 timeout/readiness 解析。
  - `backend/path.rs`
    - 后端 PATH 覆盖构建。
  - `backend/launch.rs`
    - backend 启动计划解析与进程拉起流程。
  - `backend/process_lifecycle.rs`
    - backend 停止、日志轮转 worker 生命周期与进程存活判定。
  - `backend/http.rs`
    - backend TCP/HTTP 探活、请求封装与响应解析调用链。
  - `backend/http_response.rs`
    - HTTP 响应解析与后端 start_time 提取。
  - `backend/runtime.rs`
    - backend 运行时参数（timeout/readiness/ping）解析与缓存。
  - `backend/readiness.rs`
    - backend 就绪探测、等待轮询与超时日志收敛。
  - `backend/restart.rs`
    - backend restart token 管理、graceful/fallback 策略与 bridge 状态组装。
  - `backend/restart_strategy.rs`
    - backend restart 策略与 graceful outcome/execution 决策。

- `main.rs`
  - 应用入口与流程编排。
- `desktop_state.rs`
  - `desktop_state.json` 共享路径解析，供 shell locale 与 update channel 共用。
- `app_runtime_events.rs`
  - 窗口/页面加载/退出事件的纯决策逻辑。
- `logging.rs`
  - 日志路径、日志轮转、日志写入与分类。
- `startup_mode.rs`
  - 启动模式纯逻辑。
- `webui_paths.rs`
  - 打包 WebUI fallback 路径逻辑。
- `exit_state.rs`
  - 退出状态机。
- `process_control.rs`
  - 子进程 graceful/force 停止控制与等待策略。
- `shell_locale.rs`
  - shell locale 归一化、共享状态缓存读写与托盘文案映射。
- `update_channel.rs`
  - stable/nightly 通道解析、updater endpoint 选择与 `updateChannel` 状态持久化。
- `runtime_paths.rs`
  - source root / packaged root / 资源路径探测逻辑。
- `packaged_webui.rs`
  - 打包 WebUI fallback 决策与错误文案组装。
- `ui_dispatch.rs`
  - 主线程任务调度与 startup error 分发封装。
- `restart_backend_flow.rs`
  - backend 重启任务与并发判定流程封装。
- `launch_plan.rs`
  - custom/packaged/dev 启动计划构建与路径解析。
- `startup_task.rs`
  - 启动阶段后端就绪等待与主线程导航分发。
- `app_runtime.rs`
  - Tauri builder/run 编排与窗口/页面事件挂载。
- `app_types.rs`
  - 共享核心类型定义（状态、启动计划、bridge 返回结构、原子 guard）。
- `app_constants.rs`
  - 全局运行常量（timeout/log/tray/startup/windows flags）。
- `app_helpers.rs`
  - 跨模块复用 helper（日志写入、bridge 注入、路径覆写、debug command）。
- `bridge_bootstrap.js`
  - 注入到 WebView 的 desktop bridge 脚本模板。

## 3. 脚本侧组织（`scripts/prepare-resources`）

- `prepare-resources.mjs`
  - 编排入口。
- `source-repo.mjs`
  - 上游仓库 URL/ref 处理与同步。
- `version-sync.mjs`
  - 版本读取与写回。
- `backend-runtime.mjs`
  - CPython runtime 解析与准备。
- `mode-tasks.mjs`
  - `webui/backend/all` 任务实现。
- `desktop-bridge-checks.mjs`
  - bridge 相关校验。
- `*.test.mjs`
  - Node 行为测试。

## 4. 文档组织（`docs/`）

- `architecture.md`
  - 当前子系统边界与主要流程。
- `development.md`
  - 本地构建、维护命令、版本同步与发布说明。
- `data-migration.md`
  - 桌面端与源码部署之间的数据迁移指南。
- `repository-structure.md`
  - 文件组织说明（本文档）。
- `environment-variables.md`
  - 环境变量单一来源文档。
- `archive/refactor/`
  - 历史重构计划、设计稿与阶段性归档。

## 5. 新增代码落位规则

1. 入口文件只做编排：
   - `main.rs` 与 `prepare-resources.mjs` 不承载复杂纯逻辑。
2. 纯逻辑优先模块化：
   - 路径、配置、状态机、策略函数落到独立模块。
3. 每个新模块至少满足：
   - 清晰职责。
   - 最小公开 API。
   - 对应单测或行为测试。
4. 变更同步文档：
   - 新增 `ASTRBOT_*` 变量时更新 `environment-variables.md`。
   - 目录职责变化时更新本文档与 `architecture.md`。

## 6. 测试入口约定

- 本地统一入口：`make test`
  - 全量 Rust 单测
  - `prepare-resources` Node 行为测试
- CI 同步校验：
  - `check-rust.yml`
  - `check-scripts.yml`
