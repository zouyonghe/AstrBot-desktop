# AstrBot Desktop 重构计划（Phase 4）

## 1. 背景

Phase 3 进一步收敛了路径、桥接与托盘文案更新逻辑，`main.rs` 已降到 1800+ 行。

当前仍存在的主要复杂度集中在：

- 退出清理与事件分支协同
- 托盘事件主流程编排
- 后端生命周期状态与并发门禁路径

## 2. 目标

1. 继续把 `main.rs` 中的流程型 helper 迁移到模块层。
2. 保持退出与重启流程行为稳定。
3. 维持本地与 CI 校验流程无回归。

## 3. 非目标

- 不改变用户可见行为。
- 不替换现有技术栈。
- 不改动发布/打包策略。

## 4. 执行策略

- 先更新文档，按职责簇增量重构。
- 每个职责簇完成后立即跑 `make lint` / `make test`。
- 优先迁移低风险流程辅助函数，再处理高耦合主流程。

## 5. 拆分顺序（建议）

1. 退出清理流程模块化
- 迁移 `ExitRequested/Exit` 清理判定与 stop-backend 分支。

2. 托盘事件主流程继续收敛
- 迁移 tray 事件动作执行细节。

3. 后端生命周期流程收敛
- 迁移 backend startup/restart 的流程辅助函数。

## 6. 验收标准

- `main.rs` 继续精简且职责更聚焦入口编排。
- 退出/重启行为保持一致。
- 本地 `make lint` 与 `make test` 全通过。

## 7. 实施记录（归档）

1. 新增 Phase 4 计划文档。
2. 抽离退出清理流程模块（`src-tauri/src/exit_cleanup.rs`），统一 `ExitRequested/Exit` 清理判定与 stop-backend 分支。
3. 抽离重启任务流程模块（`src-tauri/src/restart_backend_flow.rs`），统一 backend action 并发判定与重启任务执行路径。
4. 抽离托盘事件处理模块（`src-tauri/src/tray_menu_handler.rs`），将菜单动作执行流程从入口文件分离。
5. 抽离窗口动作模块（`src-tauri/src/window_actions.rs`），统一 show/hide/toggle/reload 行为与托盘文案联动。
6. 收敛 desktop bridge 注入判定到 `desktop_bridge.rs`，统一 backend/page URL 判定入口。
7. 抽离托盘初始化模块（`src-tauri/src/tray_setup.rs`），迁移托盘菜单构建与事件绑定流程。
8. 将 backend URL 归一化逻辑迁移到 `backend_config.rs` 并补充行为测试。
9. 抽离 launch plan 解析模块（`src-tauri/src/launch_plan.rs`），迁移 custom/packaged/dev 三类启动计划构建逻辑。
10. 抽离 startup 异步任务模块（`src-tauri/src/startup_task.rs`），统一后端就绪等待与启动后导航流程。
11. 抽离退出事件处理模块（`src-tauri/src/exit_events.rs`），迁移 `RunEvent::ExitRequested/Exit` 分支编排。
12. 抽离 backend 运行时参数模块（`src-tauri/src/backend_runtime.rs`），集中 timeout/readiness/ping 解析与缓存。
