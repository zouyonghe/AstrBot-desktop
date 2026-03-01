# AstrBot Desktop 重构计划（Phase 10）

## 1. 背景

Phase 9 后整体结构已稳定，但 `backend_startup.rs` 仍集中了承载两类职责：

- 启动计划解析与进程拉起
- 后端就绪探测与等待轮询

该模块规模较大，不利于按职责定位问题。

## 2. 目标

1. 将 startup 相关逻辑继续按职责拆分。
2. 拆成 launch 与 readiness 两个模块，降低单文件复杂度。
3. 保持现有调用链与行为不变。

## 3. 非目标

- 不改动启动策略语义。
- 不调整超时默认值和环境变量约定。
- 不引入异步模型变化。

## 4. 执行策略

- 复制方法到新模块后删除旧模块实现。
- 保持方法签名不变，避免调用点扩散。
- 完成后执行 `make lint` 与 `make test`。

## 5. 拆分顺序（建议）

1. 抽离 launch 模块
- `resolve_launch_plan`、`start_backend_process`。

2. 抽离 readiness 模块
- `ensure_backend_ready`、`wait_for_backend`、`probe/log timeout`。

3. 清理旧模块
- 删除 `backend_startup.rs`，更新 `main.rs` module 声明。

4. 文档同步
- 更新 `README.md`、`docs/architecture.md`、`docs/repository-structure.md`。

## 6. 验收标准

- startup 逻辑边界更清晰。
- 行为无回归，校验通过。
- 文档反映最新模块结构。

## 7. 实施记录（归档）

1. 新增 Phase 10 计划文档。
2. 抽离 `backend_launch.rs`，迁移 `resolve_launch_plan` 与 `start_backend_process`。
3. 抽离 `backend_readiness.rs`，迁移 `ensure_backend_ready` 与 readiness 轮询流程。
4. 删除 `backend_startup.rs` 并更新 `main.rs` 模块声明。
5. 同步文档索引与架构/目录说明（`README.md`、`docs/architecture.md`、`docs/repository-structure.md`）。
6. 本地验证通过：`make lint`、`make test`。
