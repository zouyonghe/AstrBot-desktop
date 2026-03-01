# AstrBot Desktop 重构计划（Phase 9）

## 1. 背景

Phase 8 已将入口拆分到最小壳层，结构重构目标基本完成。

剩余优化点：

- 新抽离模块（`app_types`、`app_helpers`）的行为单测覆盖还不足
- 需要对重构收尾阶段进行文档归档

## 2. 目标

1. 为核心新模块补充低风险、纯逻辑单测。
2. 确保重构后的关键 helper/guard 行为可以稳定回归验证。
3. 完成文档索引同步，做好阶段性归档收尾。

## 3. 非目标

- 不再做大规模结构搬迁。
- 不改动对外功能和 IPC 契约。
- 不引入集成测试框架调整。

## 4. 执行策略

- 优先补纯逻辑测试（无 UI/进程副作用）。
- 保持测试独立、快速、可重复。
- 修改后执行 `make lint` 与 `make test`。

## 5. 拆分顺序（建议）

1. 补充 `app_types` 测试
- 覆盖 `AtomicFlagGuard` 的 set/try_set/drop 行为。

2. 补充 `app_helpers` 测试
- 覆盖 `build_debug_command` 的参数拼接行为。

3. 文档同步
- 更新 `README.md`、`docs/architecture.md`、`docs/repository-structure.md`。

## 6. 验收标准

- 新增测试稳定通过。
- 全量 `make lint` / `make test` 通过。
- 重构归档文档完整可查。

## 7. 实施记录（归档）

1. 新增 Phase 9 计划文档。
2. 在 `src-tauri/src/app_types.rs` 新增 `AtomicFlagGuard` 行为测试（set/try_set/drop）。
3. 在 `src-tauri/src/app_helpers.rs` 新增 `build_debug_command` 行为测试。
4. Rust 单测总数提升到 41 项。
5. 同步文档索引（`README.md`、`docs/repository-structure.md`）。
6. 本地验证通过：`make lint`、`make test`。
