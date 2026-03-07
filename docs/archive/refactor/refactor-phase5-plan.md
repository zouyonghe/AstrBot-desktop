# AstrBot Desktop 重构计划（Phase 5）

## 1. 背景

Phase 4 已完成退出事件、启动任务、启动计划与运行时参数拆分，`main.rs` 已下降到约 1200 行。

当前剩余复杂度主要集中在：

- `BackendState` 的 HTTP 探活/请求细节与响应解析调用链
- 后端重启策略（graceful/fallback）与 token 管理
- 进程生命周期方法仍与入口编排同文件

## 2. 目标

1. 继续迁移 `BackendState` 细节逻辑，保留入口文件为编排层。
2. 明确 backend lifecycle 子职责边界（探活、重启策略、停止与桥接状态）。
3. 在不改变行为前提下保持测试与 lint 校验全通过。

## 3. 非目标

- 不调整对外 IPC 命令契约。
- 不改变托盘/窗口用户可见行为。
- 不改动打包与发布流程。

## 4. 执行策略

- 先更新文档，按职责簇小步提交。
- 每次抽离都保持函数签名与调用路径稳定。
- 每批改动后执行 `make lint` 与 `make test`。

## 5. 拆分顺序（建议）

1. 抽离 backend HTTP 能力模块
- 迁移 `ping_backend`、`request_backend_*`、`fetch_backend_start_time`。

2. 抽离 backend restart 策略模块
- 迁移 restart token 管理、graceful restart 轮询、restart strategy 决策。

3. 继续收敛 backend process lifecycle
- 视改动风险迁移 stop/log-rotation/bridge stop 等流程。

4. 同步文档索引
- 更新 `README.md`、`docs/architecture.md`、`docs/repository-structure.md`。

## 6. 验收标准

- `main.rs` 行数继续下降且职责更集中在入口编排。
- backend 重启/停止/探活行为与现状一致。
- 本地 `make lint` 与 `make test` 全通过。

## 7. 实施记录（归档）

1. 新增 Phase 5 计划文档。
2. 抽离 backend HTTP 模块（`src-tauri/src/backend_http.rs`），迁移 `ping_backend`、`request_backend_*`、`fetch_backend_start_time`。
3. 抽离 backend restart 模块（`src-tauri/src/backend_restart.rs`），迁移 restart token、graceful 轮询、restart strategy 与 bridge state 组装。
4. 抽离 backend startup 模块（`src-tauri/src/backend_startup.rs`），迁移启动计划解析、进程拉起与 readiness 轮询。
5. 抽离 backend process lifecycle 模块（`src-tauri/src/backend_process_lifecycle.rs`），迁移 stop/log-rotation worker 生命周期逻辑。
6. `main.rs` 进一步精简至约 457 行，仅保留入口编排、桥接命令与退出状态机包装方法。
7. 同步文档索引与架构/目录说明（`README.md`、`docs/architecture.md`、`docs/repository-structure.md`）。
8. 本地验证通过：`make lint`、`make test`。
