# AstrBot Desktop 重构计划（Phase 8）

## 1. 背景

Phase 7 后，`main.rs` 已降到约 143 行，主要剩余内容为：

- 全局常量定义
- 共享 helper 函数（日志、bridge 注入、路径覆写）
- 入口 `main`

## 2. 目标

1. 将常量与共享 helper 迁移到专用模块。
2. 通过 `re-export` 维持现有 `crate::CONST` / `crate::fn` 调用兼容。
3. 把入口文件精简为最小启动壳层。

## 3. 非目标

- 不调整任何业务流程。
- 不修改模块调用方逻辑。
- 不改变日志、路径、timeout 等配置语义。

## 4. 执行策略

- 先迁移常量，再迁移 helper。
- 在 `main.rs` 统一 `pub(crate) use` 导出保持兼容。
- 迁移后执行 `make lint` 与 `make test`。

## 5. 拆分顺序（建议）

1. 新建 `app_constants.rs`
- 迁移全局 timeout/log/tray/startup 相关常量。

2. 新建 `app_helpers.rs`
- 迁移日志 helper、bridge 注入、路径覆写、debug command 组装。

3. 收敛 `main.rs`
- 保留模块声明、re-export 与 `main()`。

4. 文档同步
- 更新 `README.md`、`docs/architecture.md`、`docs/repository-structure.md`。

## 6. 验收标准

- `main.rs` 最小化且可读性显著提升。
- 所有模块调用路径保持兼容。
- 本地 `make lint` 与 `make test` 全通过。

## 7. 实施记录（归档）

1. 新增 Phase 8 计划文档。
2. 新增常量模块（`src-tauri/src/app_constants.rs`），迁移 timeout/log/tray/startup 常量。
3. 新增 helper 模块（`src-tauri/src/app_helpers.rs`），迁移日志、bridge 注入、路径覆写与导航 helper。
4. `main.rs` 统一 re-export 常量、helper、类型，保持现有 `crate::*` 调用兼容。
5. `main.rs` 收敛至约 55 行，成为最小入口壳层。
6. 同步文档索引与架构/目录说明（`README.md`、`docs/architecture.md`、`docs/repository-structure.md`）。
7. 本地验证通过：`make lint`、`make test`。
