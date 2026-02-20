![AstrBot-Logo-Simplified](https://github.com/user-attachments/assets/ffd99b6b-3272-4682-beaa-6fe74250f7d9)

<div align="center">

# AstrBot Desktop

AstrBot 桌面应用（Tauri）。

<a href="https://github.com/AstrBotDevs/AstrBot">上游项目仓库</a> ｜
<a href="https://astrbot.app/">官方文档</a> ｜
<br>

<img src="https://img.shields.io/badge/Tauri-2.10.0-24C8D8?logo=tauri&logoColor=white" alt="Tauri 2.10.0">
<img src="https://img.shields.io/badge/Rust-1.86%2B-000000?logo=rust" alt="Rust 1.86+">
<img src="https://img.shields.io/badge/Runtime-CPython%203.12-blue" alt="CPython 3.12">
<img src="https://img.shields.io/badge/Upstream-AstrBotDevs%2FAstrBot-181717?logo=github" alt="AstrBotDevs/AstrBot">

</div>

## 一键安装（推荐）

如果你只想使用软件，不需要本地构建，请直接从 Releases 下载对应系统的安装包：

[`Releases`](https://github.com/AstrBotDevs/AstrBot-desktop/releases/latest)

## 手动构建

适用于需要调试桌面应用、切换上游分支或验证本地改动的场景。
推荐优先使用 `make` 命令，仓库已封装常用流程。

### 1. 查看可用命令（推荐）

仓库内置了 `Makefile`，可直接查看常用命令：

```bash
make help
```

### 2. 安装依赖

```bash
make deps
```

也可以使用：

```bash
pnpm install
```

### 3. 准备资源

```bash
make prepare
```

也可以使用：

```bash
pnpm run prepare:resources
```

### 4. 本地开发运行

```bash
make dev
```

也可以使用：

```bash
pnpm run dev
```

### 5. 构建安装包

```bash
make build
```

也可以使用：

```bash
pnpm run build
```

等价命令（直接使用 Tauri CLI）：

```bash
cargo tauri build
```

构建产物目录：

- `src-tauri/target/release/bundle/`
- 若使用 `--target` 显式指定目标（例如 CI 的 macOS 构建），产物目录为 `src-tauri/target/<target-triple>/release/bundle/`

## 常用维护命令

代码检查与测试：

```bash
make lint
make test
```

环境排查：

```bash
make doctor
```

清理构建产物：

```bash
make clean
```

仅清理占用空间较大的本地缓存：

```bash
make prune
```

## 上游仓库策略

默认上游仓库：

- `https://github.com/AstrBotDevs/AstrBot.git`

如需覆盖默认值：

```bash
export ASTRBOT_SOURCE_GIT_URL=https://github.com/AstrBotDevs/AstrBot.git
export ASTRBOT_SOURCE_GIT_REF=master
```

使用本地 AstrBot 源码（优先级最高）：

```bash
export ASTRBOT_SOURCE_DIR=/path/to/AstrBot
```

临时测试仓库示例：

```bash
export ASTRBOT_SOURCE_GIT_URL=https://github.com/zouyonghe/AstrBot.git
export ASTRBOT_SOURCE_GIT_REF=cpython-runtime-refactor
```

## 构建流程说明

`src-tauri/tauri.conf.json` 已配置 `beforeBuildCommand=pnpm run prepare:resources`，构建时会自动执行以下流程：

1. 拉取或更新 AstrBot 上游源码
2. 构建 Dashboard 并同步 `resources/webui`
3. 下载或复用 CPython 运行时（缓存到 `runtime/`）
4. 生成 `resources/backend`（含 Python 运行时、依赖、启动脚本）
5. 调用 `cargo tauri build` 输出安装包
