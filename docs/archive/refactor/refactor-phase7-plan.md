# AstrBot Desktop 重构计划（Phase 7）

## 1. 背景

Phase 6 后，`main.rs` 已下降到约 237 行，但仍承载较多“类型与默认值定义”内容。

当前剩余聚合点：

- `BackendState`、`LaunchPlan`、`TrayMenuState` 等结构定义仍在入口文件
- `AtomicFlagGuard` 与 `BackendState::default` 仍位于入口
- 入口文件同时维护常量、类型、helper，阅读密度仍偏高

## 2. 目标

1. 将核心类型定义从入口文件迁移到独立模块。
2. 保持现有模块调用路径不变（通过 re-export 维持 `crate::Type` 引用）。
3. 不改变行为，仅优化代码组织与可读性。

## 3. 非目标

- 不改动后端生命周期逻辑。
- 不新增或变更 IPC 命令。
- 不调整环境变量语义与默认值。

## 4. 执行策略

- 先更新文档，先迁类型，再跑全量校验。
- 使用 `pub(crate)` 字段最小化可见性变更范围。
- 迁移后统一执行 `make lint` 与 `make test`。

## 5. 拆分顺序（建议）

1. 新建 `app_types.rs`
- 迁移 `TrayMenuState`、`RuntimeManifest`、`LaunchPlan`、`BackendState`、`BackendBridge*`、`AtomicFlagGuard`。

2. 在 `main.rs` 做 re-export
- 保持其他模块继续使用 `crate::Type` 不改调用点。

3. 同步文档索引
- 更新 `README.md`、`docs/architecture.md`、`docs/repository-structure.md`。

## 6. 验收标准

- `main.rs` 进一步精简并聚焦入口常量/helper。
- 类型可见性与行为兼容现有模块。
- 本地 `make lint` 与 `make test` 全通过。

## 7. 实施记录（归档）

1. 新增 Phase 7 计划文档。
2. 新增共享类型模块（`src-tauri/src/app_types.rs`），迁移核心结构定义与 `AtomicFlagGuard`。
3. 将 `BackendState::default` 迁移至 `app_types.rs`，统一状态初始化职责。
4. `main.rs` 通过 `pub(crate) use` 做类型 re-export，保持 `crate::Type` 调用兼容。
5. `main.rs` 进一步精简至约 143 行，聚焦常量、入口与共享 helper。
6. 同步文档索引与架构/目录说明（`README.md`、`docs/architecture.md`、`docs/repository-structure.md`）。
7. 本地验证通过：`make lint`、`make test`。
