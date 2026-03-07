# AstrBot Desktop 数据迁移指南

本文档用于帮助用户在 `AstrBot Desktop` 和 `AstrBot` 源码部署之间迁移数据。

## 1. 适用范围

本文档主要覆盖两类迁移：

- 从桌面端迁移到源码部署
- 从源码部署迁移到桌面端

如果你使用的是 Docker、面板或其他特殊部署方式，请先确认目标环境的数据根目录与 Python 运行环境，再决定是否照搬本文步骤。

## 2. 迁移前准备

开始之前，请先完成这几件事：

1. 完全退出桌面端或停止源码部署进程。
2. 备份源环境数据目录。
3. 确认目标环境的数据根目录。
4. 不要在程序运行中直接覆盖数据目录。

## 3. 先确认两边的根目录

### 3.1 桌面端默认根目录

桌面端默认把 AstrBot 根目录放在：

- macOS / Linux：`~/.astrbot`
- Windows：`C:\Users\<用户名>\.astrbot`

常见子目录：

- `data/config/`
- `data/plugins/`
- `data/plugin_data/`
- `data/knowledge_base/`
- `data/webchat/`
- `logs/`

### 3.2 源码部署常见根目录

源码部署的实际根目录通常是你启动 AstrBot 时所在的工作目录；如果你显式设置了 `ASTRBOT_ROOT`，则以 `ASTRBOT_ROOT` 指向的目录为准。

在源码部署里，常见数据目录通常也位于：

- `data/config/`
- `data/plugins/`
- `data/plugin_data/`
- `data/knowledge_base/`
- `data/webchat/`
- `logs/`

## 4. 推荐迁移哪些内容

建议优先迁移这些长期数据：

| 路径 | 是否建议迁移 | 说明 |
| --- | --- | --- |
| `data/config/` | 是 | 配置文件通常需要保留 |
| `data/plugins/` | 是 | 插件代码和插件目录结构 |
| `data/plugin_data/` | 是 | 插件运行产生的数据 |
| `data/knowledge_base/` | 按需 | 如果你在使用知识库，建议一并迁移 |
| `data/webchat/` | 按需 | 如果需要保留 WebChat 相关数据，可一并迁移 |
| `logs/` | 否 | 日志通常不需要迁移，可只在排障时保留备份 |
| `data/temp/` | 否 | 临时目录建议让目标环境自动重建 |
| `data/site-packages/` | 一般不建议 | 强依赖 Python 版本、系统和架构，容易产生兼容性问题 |

## 5. 为什么不建议直接迁移 `data/site-packages/`

`data/site-packages/` 中通常包含插件依赖和第三方 Python 包。它可能依赖以下条件：

- Python 版本
- 操作系统
- CPU 架构
- 本地动态库 / 编译产物

桌面端使用内置运行时，而源码部署使用你的本地 Python / `uv` 环境时，两边并不一定完全一致。因此更稳妥的做法是：

- 迁移 `plugins` 与 `plugin_data`
- 迁移配置
- 在目标环境里重新安装或更新有 Python 依赖的插件

## 6. 从桌面端迁移到源码部署

推荐步骤：

1. 退出桌面端。
2. 备份桌面端根目录（默认 `~/.astrbot`）。
3. 在源码部署环境中先初始化一次目录结构，然后停止服务。
4. 从桌面端复制以下目录到源码部署根目录下的 `data/`：
   - `config/`
   - `plugins/`
   - `plugin_data/`
   - `knowledge_base/`（如果你在使用）
   - `webchat/`（如果你在使用）
5. 不要优先复制 `data/site-packages/`；先启动源码部署，按需重新安装插件依赖。
6. 启动源码部署后检查：
   - 配置是否正确载入
   - 插件是否识别
   - 有外部依赖的插件是否需要重新安装

补充说明：

- `desktop_state.json` 主要保存桌面端壳层状态，不是源码部署必需数据。
- 如果迁移目标系统与原系统不同，原来的二进制依赖更不建议直接复制。

## 7. 从源码部署迁移到桌面端

推荐步骤：

1. 停止源码部署进程。
2. 备份源码部署根目录。
3. 安装桌面端，并先启动一次后退出，让桌面端初始化自己的目录结构。
4. 把源码部署里的以下目录复制到桌面端根目录的 `data/` 下：
   - `config/`
   - `plugins/`
   - `plugin_data/`
   - `knowledge_base/`（如果你在使用）
   - `webchat/`（如果你在使用）
5. 启动桌面端，检查配置与插件状态。
6. 如果某些插件依赖 Python 扩展或特定系统库，优先在桌面端中重新安装或重新初始化这些依赖。

补充说明：

- 桌面端会自己维护 `desktop_state.json` 和桌面壳层相关日志。
- 如果源码部署里设置了自定义 `ASTRBOT_ROOT`，迁移时请确认你复制的是实际运行目录下的数据，而不是误复制仓库源码本体。

## 8. 跨系统迁移注意事项

如果你在不同操作系统之间迁移，例如：

- Linux 源码部署 -> Windows 桌面端
- Windows 桌面端 -> macOS 源码部署

请特别注意：

- 不要直接复用原环境里的 `data/site-packages/`
- 某些插件的原生依赖可能需要在目标环境重新安装
- 路径分隔符和外部程序路径可能不同，部分配置需要手动调整

## 9. 推荐的安全做法

- 迁移前先完整备份源数据目录
- 让目标环境先初始化一次，再覆盖长期数据目录
- 遇到插件异常时，优先重装插件依赖，而不是直接覆盖更多运行时文件
- 如果要回滚，只需恢复原数据备份并回到原环境启动

## 10. 相关文档

- [`../README.md`](../README.md)
- [`./development.md`](./development.md)
- [`./environment-variables.md`](./environment-variables.md)
