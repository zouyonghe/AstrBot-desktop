# AstrBot Desktop 重构计划（Phase 3）

## 1. 背景

Phase 2 已完成多簇纯逻辑迁移，`main.rs` 从 2526 行收敛到 2100+ 行，但入口层仍包含较多运行时路径解析与资源定位细节。

Phase 3 聚焦“运行时装配边界”：继续把路径解析、资源定位、UI 事件编排等横切逻辑收敛到模块层，进一步强化入口编排职责。

## 2. 目标

1. 继续收敛 `main.rs` 为编排层，减少路径/资源类细节。
2. 提升运行时路径与资源解析逻辑的可读性和可复用性。
3. 维持现有构建、测试、CI 校验稳定通过。

## 3. 非目标

- 不改变用户可见功能和交互。
- 不调整发布流程与打包策略。
- 不进行一次性大规模重写。

## 4. 执行策略

- 先更新文档，增量重构。
- 每次抽离一簇高内聚职责，并补必要单测。
- 每步通过 `make lint` 与 `make test`。

## 5. 拆分顺序（建议）

1. 运行时路径与资源定位模块化
- 抽离 workspace/source root 解析、默认打包根目录、资源路径探测。

2. 托盘/窗口事件编排继续收敛
- 将事件路由与动作执行进一步分层。

3. 启动流程编排精简
- 对 startup 期间的主线程调度与错误展示路径继续迁移。

## 6. 验收标准

- `main.rs` 规模和复杂度进一步下降。
- 新模块职责清晰、API 最小化。
- 本地 `make lint` 与 `make test` 持续通过。

## 7. 实施记录（归档）

1. 新增 Phase 3 计划文档。
2. 抽离运行时路径与资源定位模块（`src-tauri/src/runtime_paths.rs`），迁移 source root/packaged root/resource path 解析并补充模块单测。
3. 抽离打包 WebUI 解析模块（`src-tauri/src/packaged_webui.rs`），迁移 fallback 路径决策与多语言错误文案生成。
4. 抽离 UI 主线程分发与启动错误处理模块（`src-tauri/src/ui_dispatch.rs`），统一主线程任务调度与 startup error 展示路径。
5. 抽离托盘重启 bridge 事件模块（`src-tauri/src/tray_bridge_event.rs`），集中 token 递增和事件发送日志语义。
6. 抽离 startup loading 模块（`src-tauri/src/startup_loading.rs`），迁移启动页模式判定、缓存读取与前端注入逻辑。
7. 抽离 desktop bridge 注入模块（`src-tauri/src/desktop_bridge.rs`），迁移 bootstrap 脚本装配与注入执行。
8. 抽离托盘文案更新模块（`src-tauri/src/tray_labels.rs`），统一托盘菜单文案刷新与 set_text 错误处理。
