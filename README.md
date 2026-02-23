![AstrBot-Logo-Simplified](https://github.com/user-attachments/assets/ffd99b6b-3272-4682-beaa-6fe74250f7d9)

<div align="center">

# AstrBot Desktop

AstrBot 桌面应用（Tauri）。

<p>
  <a href="https://github.com/AstrBotDevs/AstrBot">上游项目仓库</a>
  <span> · </span>
  <a href="https://astrbot.app/">官方文档</a>
</p>
<br>

<img src="https://img.shields.io/badge/Tauri-2.10.0-24C8D8?logo=tauri&logoColor=white" alt="Tauri 2.10.0">
<img src="https://img.shields.io/badge/Rust-1.86%2B-000000?logo=rust" alt="Rust 1.86+">
<img src="https://img.shields.io/badge/Runtime-CPython%203.12-blue" alt="CPython 3.12">
<img src="https://img.shields.io/badge/Upstream-AstrBotDevs%2FAstrBot-181717?logo=github" alt="AstrBotDevs/AstrBot">

</div>

## 一键安装（推荐）

如果你只想使用软件，不需要本地构建，请直接从 [`Releases`](https://github.com/AstrBotDevs/AstrBot-desktop/releases/latest) 下载对应系统的安装包。

版本说明：

- [正式版](https://github.com/AstrBotDevs/AstrBot-desktop/releases/latest)：适合大多数用户日常使用。
- [Nightly 版](https://github.com/AstrBotDevs/AstrBot-desktop/releases/tag/nightly)：基于上游最新提交自动构建，适合提前体验新改动。
- 下载时请按操作系统与 CPU 架构选择对应安装包。

## 开源协议

本项目采用 `AGPL-3.0` 开源协议，协议全文见：[`LICENSE`](./LICENSE)。

## 手动构建

推荐直接使用 Makefile：

```bash
make deps
make prepare
make dev
make build
```

可用命令总览：

```bash
make help
```

构建产物默认在 `src-tauri/target/release/bundle/`。

## 常用维护命令

```bash
make lint
make test
make doctor
make clean
make prune
```

`make test` 会执行：

- Rust 全量单元测试（`cargo test --locked`）
- 资源准备脚本行为测试（`pnpm run test:prepare-resources`，若本地无 `pnpm` 会跳过并提示）

## 版本维护（重要）

- `make update`：从上游同步版本（推荐日常使用）。
- `make sync-version`：从当前解析到的 AstrBot 源同步版本（会受本地环境变量影响）。
- `make build`：默认使用当前 `package.json` 的版本，可用 `ASTRBOT_DESKTOP_VERSION=...` 覆盖（支持 `v` 前缀，写入时会自动去掉）。

桌面端版本会同步到：
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

### 常用环境变量

- `ASTRBOT_SOURCE_GIT_URL` / `ASTRBOT_SOURCE_GIT_REF`：指定上游仓库与分支/标签（默认 `https://github.com/AstrBotDevs/AstrBot.git` + `master`）。
- `ASTRBOT_SOURCE_DIR`：指定本地 AstrBot 源码目录（用于 `sync-version`/资源准备，`build` 也会读取）。
- `ASTRBOT_BUILD_SOURCE_DIR`：仅用于本次 `make build` 的源码目录，优先级高于 `ASTRBOT_SOURCE_DIR`。
- `ASTRBOT_DESKTOP_VERSION`：覆盖写入桌面版本号（支持 `v` 前缀，内部会归一化为无 `v`）。

示例：

```bash
make update
make update ASTRBOT_SOURCE_GIT_REF=v4.17.5
make build ASTRBOT_DESKTOP_VERSION=v4.17.5
make build ASTRBOT_BUILD_SOURCE_DIR=/path/to/AstrBot
```

清理构建相关环境变量：

```bash
make clean-env
source .astrbot-reset-env.sh
```


## CI 版本同步策略

- 定时构建（`schedule`）检测到上游新 tag 时，会先自动同步版本文件并提交，再继续构建。
- 手动触发（`workflow_dispatch`）默认只构建，不自动回写版本文件。

## 构建流程说明

`src-tauri/tauri.conf.json` 配置了 `beforeBuildCommand=pnpm run prepare:resources`。构建时会自动完成：
1. 拉取/更新 AstrBot 源码
2. 构建并同步 `resources/webui`
3. 准备 `resources/backend`（含运行时与启动脚本）
4. 执行 Tauri 打包

## 常见问题

### macOS 提示“应用已损坏”或无法打开

如果你是从网络下载的安装包，macOS 可能给 `AstrBot.app` 打上 quarantine 标记。可执行：

```bash
xattr -dr com.apple.quarantine /Applications/AstrBot.app
```

然后重新启动应用。如果应用不在 `/Applications`，请替换为实际路径。

### 缺少 Node.js / npx / uvx

部分 MCP 工具依赖 `node`/`npx` 或 `uvx`。可按下面方式安装并校验。

1. 安装 Node.js（`npx` 随 npm 一起提供）

- macOS（Homebrew）：

```bash
brew install node
```

- Windows：
  使用 Node.js 官方安装器安装 LTS 版本：<https://nodejs.org/>
- Linux（Debian/Ubuntu）：

```bash
sudo apt-get update
sudo apt-get install -y nodejs npm
```

2. 安装 uv（提供 `uvx`）

- macOS（Homebrew）：

```bash
brew install uv
```

- 其他系统请参考官方安装文档：<https://docs.astral.sh/uv/getting-started/installation/>

3. 校验命令可用

```bash
node -v
# Debian/Ubuntu 某些环境中可执行文件名可能是 nodejs
nodejs -v
npm -v
npx -v
uvx --version
```
## 问题反馈

寻求安装帮助或反馈问题和意见请加入QQ群组。

QQ群：1060046189
