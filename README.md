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

如果你只想使用软件，不需要本地构建，请直接从 Releases 下载对应系统的安装包：

[`Releases`](https://github.com/AstrBotDevs/AstrBot-desktop/releases/latest)

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

## 版本维护（重要）

- `make update`：从上游同步版本（推荐日常使用）。
- `make sync-version`：从当前解析到的 AstrBot 源同步版本（会受本地环境变量影响）。
- `make build`：默认使用当前 `package.json` 的版本，可用 `ASTRBOT_DESKTOP_VERSION=...` 覆盖。

桌面端版本会同步到：
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

### 常用环境变量

- `ASTRBOT_SOURCE_GIT_URL` / `ASTRBOT_SOURCE_GIT_REF`：指定上游仓库与分支/标签（默认 `https://github.com/AstrBotDevs/AstrBot.git` + `master`）。
- `ASTRBOT_SOURCE_DIR`：指定本地 AstrBot 源码目录（用于 `sync-version`/资源准备，`build` 也会读取）。
- `ASTRBOT_BUILD_SOURCE_DIR`：仅用于本次 `make build` 的源码目录，优先级高于 `ASTRBOT_SOURCE_DIR`。
- `ASTRBOT_DESKTOP_VERSION`：覆盖写入桌面版本号。

示例：

```bash
make update
make update ASTRBOT_SOURCE_GIT_REF=v4.17.5
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
