# AstrBot Desktop 重构归档（Phase 1）

## 1. 背景与目标

重构前主要问题：

- `src-tauri/src/main.rs` 规模过大（3000+ 行），进程管理、托盘、桥接、日志、配置解析高度耦合。
- `scripts/prepare-resources.mjs` 职责过重，源码同步、版本写入、运行时准备、模式任务混在同一入口文件。
- 自动化验证以语法/编译为主，缺少关键行为测试。

Phase 1 目标：

1. 降低入口文件复杂度，建立稳定模块边界。
2. 提升可维护性和可测试性（Rust 与脚本双侧）。
3. 保持构建、打包、发布流程稳定。

## 2. 非目标

- 不改动产品功能与发布渠道。
- 不引入大规模技术栈替换。
- 不一次性重写 CI，只做增量收敛。

## 3. 执行策略

- 先更新文档，小步重构。
- 单次仅处理一类职责，变更可回滚。
- 每步至少通过编译/语法检查，关键点补行为测试。

## 4. 重构范围

### 4.1 结构拆分

- 外置桌面桥接脚本。
- 抽离日志模块。
- 抽离后端配置解析模块。
- 抽离启动模式、路径工具等纯函数模块。

### 4.2 行为稳定性

- 统一重启入口并发控制。
- 收敛退出清理流程。
- 引入日志分类入口（startup/runtime/restart/shutdown）。

### 4.3 脚本与流水线

- 拆分 `prepare-resources` 为多子模块。
- 建立环境变量清单文档。
- 增加脚本行为测试并接入 CI 校验。

## 5. 实施记录（归档）

1. 外置桌面桥接 JS（`bridge_bootstrap.js`）。
2. 抽离日志轮转实现（`logging.rs`）。
3. 抽离后端配置解析（`backend_config.rs`）。
4. 统一托盘/桥接重启并发门禁与任务路径。
5. 为后端配置补充单测。
6. 拆分源码仓库同步子模块（`source-repo.mjs`）。
7. 建立环境变量清单（`docs/environment-variables.md`）。
8. 拆分 CPython 运行时准备子模块（`backend-runtime.mjs`）。
9. 拆分模式任务子模块（`mode-tasks.mjs`）。
10. 抽离启动模式判定模块（`startup_mode.rs`）。
11. 抽离后端 PATH 覆盖构建模块（`backend_path.rs`）。
12. 抽离打包 WebUI 回退路径模块（`webui_paths.rs`）。
13. 迁移桌面日志路径与写入实现到日志模块。
14. 增加脚本行为测试与 npm 入口（`test:prepare-resources`）。
15. 接入 CI 校验（Rust 关键单测 + Node 行为测试）。
16. 收敛 `ExitRequested/Exit` 清理分支为共享 helper。
17. 落地日志分类入口并接入关键路径。
18. 引入退出状态机（`exit_state.rs`）并补状态流转测试。
19. 补充 `webui_paths` 行为测试并接入 Rust 检查。
20. 增强 `make test`，统一执行全量 Rust 单测与脚本行为测试。

## 6. 结果摘要

### 6.1 新增/重构模块

- Rust：
  - `src-tauri/src/bridge_bootstrap.js`
  - `src-tauri/src/logging.rs`
  - `src-tauri/src/backend_config.rs`
  - `src-tauri/src/startup_mode.rs`
  - `src-tauri/src/backend_path.rs`
  - `src-tauri/src/webui_paths.rs`
  - `src-tauri/src/exit_state.rs`
- Scripts：
  - `scripts/prepare-resources/source-repo.mjs`
  - `scripts/prepare-resources/backend-runtime.mjs`
  - `scripts/prepare-resources/mode-tasks.mjs`
  - `scripts/prepare-resources/source-repo.test.mjs`
  - `scripts/prepare-resources/version-sync.test.mjs`
- Docs：
  - `docs/environment-variables.md`

### 6.2 验证流程

- Rust：`fmt` / `clippy -D warnings` / `check` / 单测。
- Scripts：`node --check` / `node --test`。
- Unified：`make test`。
- CI Workflow YAML 语法校验。

### 6.3 规模变化（Phase 1 收敛后）

- `src-tauri/src/main.rs`：约 `3240 -> 2526`
- `scripts/prepare-resources.mjs`：约 `651 -> 153`

## 7. 风险与回滚

- 风险点：桥接注入时机、WebUI 回退路径、退出清理流程。
- 控制方式：模块化拆分后保留原语义，并补关键行为测试与 CI 校验。
- 回滚策略：按提交粒度回滚，避免跨模块大范围回退。
