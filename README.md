![AstrBot-Logo-Simplified](https://github.com/user-attachments/assets/ffd99b6b-3272-4682-beaa-6fe74250f7d9)

<div align="center">

# AstrBot Desktop

AstrBot 的桌面应用版本，适合在本机快速安装、使用 ChatUI 与插件能力。

<p>
  <a href="https://github.com/AstrBotDevs/AstrBot">上游 AstrBot</a>
  <span> · </span>
  <a href="https://astrbot.app/">官方文档</a>
  <span> · </span>
  <a href="https://github.com/AstrBotDevs/AstrBot-desktop/releases/latest">下载 Releases</a>
  <span> · </span>
  <a href="https://github.com/AstrBotDevs/AstrBot-desktop/issues">问题反馈</a>
</p>

<br>

<div>
<img src="https://img.shields.io/github/v/release/AstrBotDevs/AstrBot-desktop?color=76bad9" alt="Latest release">
<img src="https://img.shields.io/badge/Tauri-2.10.0-24C8D8?logo=tauri&logoColor=white" alt="Tauri 2.10.0">
<img src="https://img.shields.io/badge/Runtime-CPython%203.12-blue" alt="CPython 3.12">
<img src="https://img.shields.io/badge/Upstream-AstrBotDevs%2FAstrBot-181717?logo=github" alt="AstrBotDevs/AstrBot">
</div>

</div>

AstrBot Desktop 是面向本地桌面使用的 AstrBot 打包发行版。它内置 WebUI、后端运行时和桌面壳层，适合希望快速安装、在本机使用 ChatUI、插件与知识库能力的用户；如果你计划长期运行在服务器上，更推荐使用上游 AstrBot 的源码、Docker 或面板部署方式。

## 适用场景

- 想在 Windows、macOS、Linux 上直接安装 AstrBot，不想先配置完整命令行环境。
- 主要在本机使用 ChatUI、插件市场、知识库等能力。
- 希望把数据保存在本地目录中，便于备份、迁移和故障排查。
- 需要 stable / nightly 两种发布通道，日常使用与抢先体验都方便。

## 主要特点

1. 开箱即用的桌面安装体验，默认集成 WebUI 与后端运行时。
2. 与上游 AstrBot 生态保持兼容，适合本机体验 ChatUI、插件和常见工作流。
3. 默认使用独立本地数据目录，配置、插件和日志更容易管理。
4. 提供 stable / nightly 通道，适合稳定使用或提前体验最新改动。
5. 支持迁移到源码部署，也支持从源码部署迁移回桌面端。

## 一键安装

如果你只想使用软件，不需要本地构建，请直接从 [`Releases`](https://github.com/AstrBotDevs/AstrBot-desktop/releases/latest) 下载对应系统的安装包。

- [正式版](https://github.com/AstrBotDevs/AstrBot-desktop/releases/latest)：适合大多数用户日常使用。
- [Nightly 版](https://github.com/AstrBotDevs/AstrBot-desktop/releases/tag/nightly)：基于较新的上游改动自动构建，适合提前体验新功能或修复。
- 下载时请按操作系统与 CPU 架构选择对应安装包。

## 数据与配置位置

桌面端默认把 AstrBot 根目录放在用户主目录下的 `.astrbot`：

- macOS / Linux：`~/.astrbot`
- Windows：`C:\Users\<用户名>\.astrbot`

常见目录如下：

| 路径 | 说明 |
| --- | --- |
| `data/config/` | 配置文件目录 |
| `data/plugins/` | 插件目录 |
| `data/plugin_data/` | 插件数据目录 |
| `data/knowledge_base/` | 知识库数据 |
| `data/webchat/` | WebChat 相关数据 |
| `logs/` | 桌面端与后端日志 |

如果你需要在桌面端和源码部署之间迁移数据，请先阅读 [`docs/data-migration.md`](docs/data-migration.md)。

## 更新与版本通道

- `stable`：面向日常使用，默认推荐。
- `nightly`：更接近上游最新提交，适合测试新功能或修复。
- Windows、macOS 和 Linux AppImage 场景通常可以直接使用桌面端更新入口；部分 Linux 安装方式会退化为手动下载安装。

## 常见问题

### 适合部署在服务器上吗？

不推荐。AstrBot Desktop 更适合本地桌面使用和个人工作流体验；如果你要长期稳定运行在服务器上，更建议使用上游 AstrBot 的源码、Docker 或面板部署方式。

### macOS 提示“应用已损坏”或无法打开

如果你是从网络下载的安装包，macOS 可能会给应用打上 quarantine 标记。可执行：

```bash
xattr -dr com.apple.quarantine /Applications/AstrBot.app
```

然后重新启动应用。如果应用不在 `/Applications`，请替换为实际路径。

### 为什么某些 MCP 工具提示缺少 `node`、`npx` 或 `uvx`？

部分 MCP 工具需要依赖系统里的 `node` / `npx` / `uvx`。这类依赖不由桌面端安装包统一提供，需要你在系统里额外安装。

- Node.js 安装文档：<https://nodejs.org/>
- uv 安装文档：<https://docs.astral.sh/uv/getting-started/installation/>

安装后可自行检查：

```bash
node -v
npm -v
npx -v
uvx --version
```

## 进阶文档

- [`docs/data-migration.md`](docs/data-migration.md)：桌面端与源码部署之间的数据迁移说明。
- [`docs/development.md`](docs/development.md)：本地构建、维护命令、版本同步与发布说明。
- [`docs/environment-variables.md`](docs/environment-variables.md)：环境变量清单。
- [`docs/architecture.md`](docs/architecture.md)：当前桌面端架构说明。
- [`docs/repository-structure.md`](docs/repository-structure.md)：仓库目录职责总览。

## 问题反馈

如需安装帮助，或想反馈问题与建议，可以通过以下方式联系：

- GitHub Issues：<https://github.com/AstrBotDevs/AstrBot-desktop/issues>
- QQ 群：1060046189

## 开源协议

本项目采用 `AGPL-3.0` 开源协议，协议全文见 [`LICENSE`](./LICENSE)。
