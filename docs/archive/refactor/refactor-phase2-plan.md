# AstrBot Desktop 重构计划（Phase 2）

## 1. 背景

Phase 1 已完成入口精简与基础模块化，但 `src-tauri/src/main.rs` 仍承担了较多“可迁移的纯逻辑/工具逻辑”。

当前 Phase 2 目标是继续降低入口耦合度，强化“编排层薄、模块层厚”的边界，并在不改变产品行为前提下提升可测试性。

## 2. 目标

1. 继续收缩 `main.rs` 规模，优先抽离纯函数与工具函数簇。
2. 补齐新模块单测，保证关键行为稳定。
3. 保持构建、测试、发布流程不变（兼容现有 Make/CI 流程）。

## 3. 非目标

- 不调整产品功能和交互行为。
- 不改动打包分发策略。
- 不进行大规模架构重写（保持增量重构节奏）。

## 4. 执行策略

- 先更新文档，小步提交。
- 每次仅抽离一簇高内聚逻辑，避免跨职责大改。
- 每步都经过 `fmt`/`clippy`/`test` 校验。

## 5. 拆分顺序（建议）

1. HTTP 响应解析逻辑迁移
- 抽离状态码解析、chunked 解码、JSON 响应解析、后端 start_time 提取。

2. 进程停止与等待工具逻辑迁移
- 抽离 stop/wait 的平台无关纯策略与辅助函数。

3. 托盘/窗口事件分发器收敛
- 将事件处理映射与动作逻辑从入口分离，入口保留路由。

4. 后端 readiness 探测工具化
- 对 HTTP/TCP 探测和超时诊断格式化逻辑进行模块化。

## 6. 验收标准

- `main.rs` 进一步精简且职责更聚焦于编排。
- 新增模块具备清晰公开 API 与单测覆盖。
- 本地通过：
  - `make lint`
  - `make test`
- CI 现有检查无需额外绕行即可通过。

## 7. 实施记录（归档）

1. 抽离 HTTP 响应解析模块（`src-tauri/src/http_response.rs`），并补充模块单测。
2. 抽离进程停止控制模块（`src-tauri/src/process_control.rs`），统一 graceful/force stop 路径并补充纯逻辑单测。
3. 抽离桥接注入来源判定模块（`src-tauri/src/origin_policy.rs`），统一同源与 loopback 端口策略并补充单测。
4. 抽离托盘菜单动作映射模块（`src-tauri/src/tray_actions.rs`），把菜单 ID 解析与动作类型从入口分离。
5. 抽离 shell locale 模块（`src-tauri/src/shell_locale.rs`），集中 locale 归一化、缓存读取与托盘文案映射。
6. 将 readiness 配置结构与组装逻辑迁移到 `backend_config.rs`，入口仅保留调用。
7. 抽离主窗口操作模块（`src-tauri/src/main_window.rs`），统一 show/hide/reload/navigate 行为实现。
