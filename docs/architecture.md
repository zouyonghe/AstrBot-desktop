# AstrBot Desktop 架构说明

本文档描述当前桌面端（Tauri）运行时架构、关键模块边界和主要流程。

## 1. 总体架构

系统由三层组成：

1. 桌面壳层（Tauri + Rust）
2. WebUI 资源层（`resources/webui`）
3. 后端运行时层（`resources/backend` + CPython runtime）

桌面壳层负责：

- 进程生命周期管理（拉起、探活、重启、停止）
- 托盘与窗口行为
- 前端桥接注入与 IPC 命令
- 配置解析、日志落盘、退出流程协调

## 2. Rust 模块边界

### 2.1 `src-tauri/src/main.rs`

入口与编排层，主要保留：

- 模块声明与统一 re-export 出口
- 进程入口（`main`）与运行委托（`app_runtime::run()`）
- 维持 `crate::CONST` / `crate::fn` / `crate::Type` 调用兼容

### 2.2 `src-tauri/src/backend_config.rs`

后端配置解析模块：

- ready path 解析
- timeout clamp 与默认值策略
- readiness config 聚合（含 `BackendReadinessConfig`）
- backend URL 归一化

### 2.3 `src-tauri/src/logging.rs`

日志模块：

- 日志轮转
- 日志路径解析（desktop/backend）
- 日志落盘
- 日志分类：`startup/runtime/restart/shutdown`

### 2.4 `src-tauri/src/startup_mode.rs`

启动模式纯逻辑：

- 环境变量到启动模式映射
- WebUI 文件存在性到启动模式映射

### 2.5 `src-tauri/src/backend_path.rs`

后端 PATH 覆盖逻辑：

- 平台特定路径候选
- 去重与合并
- 诊断日志输出

### 2.6 `src-tauri/src/webui_paths.rs`

打包模式 WebUI 回退路径逻辑：

- fallback 探测目录
- fallback 可用性判断
- 诊断展示路径生成

### 2.7 `src-tauri/src/exit_state.rs`

退出状态机：

- 状态：`Running` / `QuittingRequested` / `CleanupInProgress` / `ReadyToExit` / `Exiting`
- 能力：开始清理、放行下次退出请求、状态读取

### 2.8 `src-tauri/src/http_response.rs`

HTTP 响应解析模块：

- 状态码提取（status line）
- chunked body 解码
- JSON 响应提取（仅 2xx 状态码）
- 后端 `start_time` 字段解析

### 2.9 `src-tauri/src/process_control.rs`

进程停止控制模块：

- 子进程退出等待
- graceful stop / force stop 命令编排
- 跟随等待时间计算与失败降级策略

### 2.10 `src-tauri/src/origin_policy.rs`

桥接注入来源策略模块：

- URL 同源判定
- loopback host 判定
- tray bridge 注入来源决策

### 2.11 `src-tauri/src/tray_actions.rs`

托盘菜单动作映射模块：

- 菜单 ID 常量集中定义
- 菜单 ID 到动作枚举映射
- 托盘事件分发入口去字符串硬编码

### 2.12 `src-tauri/src/shell_locale.rs`

桌面 locale 模块：

- locale 归一化与兜底
- `desktop_state.json` 缓存 locale 读取
- 托盘文案（中英）映射

### 2.13 `src-tauri/src/main_window.rs`

主窗口操作模块：

- main window show/hide/reload
- 导航到 backend dashboard
- 主窗口异常处理日志收敛

### 2.14 `src-tauri/src/runtime_paths.rs`

运行时路径模块：

- AstrBot source root 探测
- 默认打包根目录解析（`~/.astrbot`）
- 资源路径定位（含 `_up_/resources` 回退探测）

### 2.15 `src-tauri/src/packaged_webui.rs`

打包 WebUI 解析模块：

- embedded/fallback webui 目录决策
- fallback index 诊断路径组装
- 多语言不可用错误文案生成

### 2.16 `src-tauri/src/ui_dispatch.rs`

UI 分发模块：

- 主线程任务调度包装
- startup error 日志与退出流程
- startup error 主线程派发兜底

### 2.17 `src-tauri/src/tray_bridge_event.rs`

托盘 bridge 事件模块：

- tray restart signal token 递增
- 向主窗口发射重启事件
- 事件发送失败日志收敛

### 2.18 `src-tauri/src/startup_loading.rs`

启动 loading 模块：

- 是否应用 startup loading 的 URL/窗口判定
- startup mode 解析与缓存读写
- startup mode 前端注入脚本执行

### 2.19 `src-tauri/src/desktop_bridge.rs`

desktop bridge 模块：

- bridge bootstrap 模板装配
- bridge script 缓存
- bridge script 注入执行
- bridge 注入判定（backend/page URL）

### 2.20 `src-tauri/src/tray_labels.rs`

托盘文案模块：

- 托盘菜单文案按 locale 刷新
- 主窗口可见性与 toggle 文案联动
- `set_text` 失败日志收敛

### 2.21 `src-tauri/src/exit_cleanup.rs`

退出清理模块：

- 退出清理并发判定
- ExitRequested/Exit fallback 分支日志语义
- backend stop 后续退出放行日志

### 2.22 `src-tauri/src/restart_backend_flow.rs`

重启任务流程模块：

- backend action 并发判定
- 重启任务异步执行与结果归一
- bridge 与 tray 重启入口复用

### 2.23 `src-tauri/src/tray_menu_handler.rs`

托盘菜单处理模块：

- 菜单动作分发执行
- tray 触发重启流程的编排
- tray quit 退出路径收敛

### 2.24 `src-tauri/src/window_actions.rs`

窗口动作模块：

- 主窗口 show/hide/toggle/reload 统一封装
- 主窗口动作与 tray label 刷新联动
- 主窗口可见性判定日志收敛

### 2.25 `src-tauri/src/tray_setup.rs`

托盘初始化模块：

- tray 菜单项构建与状态注册
- tray icon 事件绑定
- tray setup 失败错误收敛

### 2.26 `src-tauri/src/launch_plan.rs`

启动计划模块：

- custom/packaged/dev 启动计划构建
- 打包运行时 manifest 解析
- 启动目录与 webui 路径策略收敛

### 2.27 `src-tauri/src/startup_task.rs`

启动任务模块：

- 后端就绪等待任务启动
- 启动完成后主线程导航派发
- 启动失败错误分发与退出路径

### 2.28 `src-tauri/src/exit_events.rs`

退出事件模块：

- ExitRequested 分支编排
- Exit fallback 分支编排
- 退出分支与清理模块解耦

### 2.29 `src-tauri/src/backend_runtime.rs`

后端运行时参数模块：

- backend timeout 解析
- readiness 配置解析
- backend/bridge ping timeout 解析与缓存

### 2.30 `src-tauri/src/backend_http.rs`

后端 HTTP 能力模块：

- backend TCP 探活（ping）
- 原始 HTTP 请求封装（method/path/body/token）
- status/json/start_time 响应提取调用链

### 2.31 `src-tauri/src/backend_restart.rs`

后端重启策略模块：

- restart auth token 读写与归一化
- graceful restart 请求与轮询等待
- managed/unmanaged 重启策略决策
- bridge backend state 组装

### 2.32 `src-tauri/src/backend_launch.rs`

后端启动计划与拉起模块：

- 启动计划解析（custom/packaged/dev）
- 子进程启动参数与环境注入
- backend 进程拉起与日志输出重定向

### 2.33 `src-tauri/src/backend_readiness.rs`

后端就绪探测模块：

- 启动前快速探活与 auto-start 判定
- readiness 轮询与超时控制
- HTTP/TCP 探测结果日志收敛

### 2.34 `src-tauri/src/backend_process_lifecycle.rs`

后端进程生命周期模块：

- backend graceful stop
- backend 日志轮转 worker 启停
- child PID 存活判定与轮转退出协同

### 2.35 `src-tauri/src/backend_exit_state.rs`

退出状态包装模块：

- `exit_state` 锁读写包装
- 退出流程状态方法迁移（mark/is_quitting/cleanup allow）
- 锁异常日志语义统一

### 2.36 `src-tauri/src/desktop_bridge_commands.rs`

bridge 命令模块：

- `desktop_bridge_*` IPC 命令定义
- backend action 并发判定与返回结构统一
- bridge 命令与运行编排解耦

### 2.37 `src-tauri/src/app_runtime.rs`

应用运行编排模块：

- Tauri Builder 构建与 invoke handler 挂载
- window/page load/setup/run 事件绑定
- 启动日志与退出事件分支编排

### 2.38 `src-tauri/src/app_types.rs`

共享类型模块：

- `BackendState`、`LaunchPlan`、`TrayMenuState` 等核心结构定义
- `BackendBridgeState/Result` 返回结构定义
- `AtomicFlagGuard` 与 `BackendState::default` 收敛

### 2.39 `src-tauri/src/app_constants.rs`

共享常量模块：

- timeout/readiness/ping 相关运行常量
- 日志与托盘常量
- 平台特定（Windows）进程创建 flags

### 2.40 `src-tauri/src/app_helpers.rs`

共享 helper 模块：

- 日志写入 helper（startup/runtime/restart/shutdown）
- desktop bridge 注入 helper
- backend PATH 覆写与 debug command 组装
- 主窗口导航 helper

## 3. 关键流程

### 3.1 启动流程

1. Tauri 启动并初始化托盘与窗口事件。
2. 异步 worker 执行后端就绪检查与必要拉起。
3. 成功后导航主窗口；失败时进入 startup error 路径。
4. 页面加载阶段按规则注入 desktop bridge。

### 3.2 重启流程

1. 触发源：托盘菜单或 bridge IPC。
2. 统一进入 `run_restart_backend_task`。
3. 原子门禁阻止并发重启/拉起。
4. 按策略执行 graceful 或 fallback 重启。

### 3.3 退出流程

1. `ExitRequested` 阶段先 `prevent_exit`。
2. 退出状态机尝试进入清理态。
3. 异步执行 `stop_backend`。
4. 清理完成后放行下一次退出请求并 `exit(0)`。
5. `Exit` 分支作为 fallback 清理路径。

## 4. 脚本架构（prepare-resources）

入口：`scripts/prepare-resources.mjs`（编排层）

子模块：

- `source-repo.mjs`：源码仓库 URL/ref 解析与同步
- `version-sync.mjs`：版本读取与三处文件同步
- `backend-runtime.mjs`：CPython runtime 解析/准备
- `mode-tasks.mjs`：`webui/backend/all` 任务实现
- `desktop-bridge-checks.mjs`：bridge 工件校验

## 5. 测试与校验

本地：

- `make lint`
- `make test`

CI：

- `check-rust.yml`：fmt/clippy/check + 关键 Rust 单测
- `check-scripts.yml`：Node/Python 语法 + Node 行为测试

## 6. 演进建议

- 继续把 `main.rs` 中仍偏纯函数的工具逻辑按职责迁移。
- 为退出/重启流程补充更贴近事件流的集成测试。
- 维持“编排层薄、模块层厚”的边界纪律。
