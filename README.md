![AstrBot-Logo-Simplified](https://github.com/user-attachments/assets/ffd99b6b-3272-4682-beaa-6fe74250f7d9)

<div align="center">

# AstrBot Desktop (Tauri)

AstrBot 的独立桌面端仓库。

<a href="https://github.com/AstrBotDevs/AstrBot">原始项目仓库</a> ｜
<a href="https://astrbot.app/">官方文档</a> ｜
<br>

<img src="https://img.shields.io/badge/Tauri-2.10.0-24C8D8?logo=tauri&logoColor=white" alt="Tauri 2.10.0">
<img src="https://img.shields.io/badge/Rust-1.86%2B-000000?logo=rust" alt="Rust 1.86+">
<img src="https://img.shields.io/badge/Runtime-CPython%203.12-blue" alt="CPython 3.12">
<img src="https://img.shields.io/badge/Upstream-AstrBotDevs%2FAstrBot-181717?logo=github" alt="AstrBotDevs/AstrBot">

</div>

## 一键安装（推荐）

如果你只想使用软件，不需要本地构建，请直接从 Release下载系统对应安装包：

最新版本：[`Releases`](./releases/latest)

## 手动构建

适用于需要调试桌面壳、替换上游分支、验证本地改动的场景。

### 1. 查看可用命令（推荐）

仓库内置了 `Makefile`，可直接查看常用命令：

```bash
make help
```

### 2. 安装依赖

```bash
pnpm install
```

也可使用：

```bash
make deps
```

### 3. 准备资源

```bash
make prepare
```

### 4. 本地开发运行

```bash
pnpm run dev
```

也可使用：

```bash
make dev
```

### 5. 构建安装包

```bash
pnpm run build
```

也可使用：

```bash
make build
```

等价命令（直接走 Tauri CLI）：

```bash
cargo tauri build
```

构建产物目录：

- `src-tauri/target/release/bundle/`

## 常用维护命令

代码检查：

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

仅清理大体积本地缓存：

```bash
make prune
```

## 上游仓库策略

默认上游仓库为官方：

- `https://github.com/AstrBotDevs/AstrBot.git`

如需覆盖：

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

`src-tauri/tauri.conf.json` 已配置 `beforeBuildCommand=pnpm run prepare:resources`，构建时自动执行：

1. 拉取或更新 AstrBot 上游源码
2. 构建 dashboard 并同步 `resources/webui`
3. 下载或复用 CPython runtime（缓存到 `runtime/`）
4. 生成 `resources/backend`（含 Python 运行时、依赖、启动脚本）
5. 调用 `cargo tauri build` 输出安装包
