# AstrBot Desktop 重构计划（Phase 6）

## 1. 背景

Phase 5 已完成 backend lifecycle 主体迁移，`main.rs` 降到 457 行。

剩余主要耦合点集中在：

- desktop bridge 命令定义与入口编排同文件
- Tauri builder/run 事件挂载仍位于入口文件
- `exit_state` 包装方法仍与入口文件共存

## 2. 目标

1. 让 `main.rs` 进一步聚焦类型定义与少量公共 helper。
2. 抽离命令入口与 app runtime 编排，降低入口认知负担。
3. 保持 IPC 契约、启动/退出行为不变。

## 3. 非目标

- 不引入新框架或改变 Tauri 事件模型。
- 不变更后端生命周期策略。
- 不调整打包和发布流程。

## 4. 执行策略

- 先更新文档，小步抽离。
- 先抽离低风险命令与状态包装，再抽离运行编排。
- 每批次通过 `make lint` 与 `make test` 后再提交。

## 5. 拆分顺序（建议）

1. 抽离 exit-state 包装模块
- 迁移 `mark_quitting`、`is_quitting`、`try_begin_exit_cleanup` 等包装方法。

2. 抽离 desktop bridge 命令模块
- 迁移 `desktop_bridge_*` 命令定义与统一返回结构。

3. 抽离 app runtime 编排模块
- 迁移 `tauri::Builder` 构建、事件挂载与 run 分支。

4. 同步文档索引
- 更新 `README.md`、`docs/architecture.md`、`docs/repository-structure.md`。

## 6. 验收标准

- `main.rs` 继续精简，入口职责更清晰。
- bridge 命令与 app 运行行为无回归。
- 本地 `make lint` 与 `make test` 全通过。

## 7. 实施记录（归档）

1. 新增 Phase 6 计划文档。
2. 抽离退出状态包装模块（`src-tauri/src/backend_exit_state.rs`），迁移 `mark_quitting` 等方法。
3. 抽离 bridge 命令模块（`src-tauri/src/desktop_bridge_commands.rs`），统一 `desktop_bridge_*` IPC 命令定义。
4. 抽离 app runtime 编排模块（`src-tauri/src/app_runtime.rs`），迁移 Builder 构建与 run 事件编排。
5. `main.rs` 进一步精简至约 237 行，聚焦类型定义、入口委托与共享 helper。
6. 同步文档索引与架构/目录说明（`README.md`、`docs/architecture.md`、`docs/repository-structure.md`）。
7. 本地验证通过：`make lint`、`make test`。
