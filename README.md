![AstrBot-Logo-Simplified](https://github.com/user-attachments/assets/ffd99b6b-3272-4682-beaa-6fe74250f7d9)

<div align="center">

English ｜ <a href="./README_zh.md">简体中文</a>

# AstrBot Desktop

The desktop edition of AstrBot, designed for fast local installation and convenient access to ChatUI and plugins.

<p>
  <a href="https://github.com/AstrBotDevs/AstrBot">Upstream AstrBot</a>
  <span> · </span>
  <a href="https://astrbot.app/">Documentation</a>
  <span> · </span>
  <a href="https://github.com/AstrBotDevs/AstrBot-desktop/releases/latest">Releases</a>
  <span> · </span>
  <a href="https://github.com/AstrBotDevs/AstrBot-desktop/issues">Issue Tracker</a>
</p>

<br>

<div>
<img src="https://img.shields.io/github/v/release/AstrBotDevs/AstrBot-desktop?color=76bad9" alt="Latest release">
<img src="https://img.shields.io/badge/Tauri-2.10.0-24C8D8?logo=tauri&logoColor=white" alt="Tauri 2.10.0">
<img src="https://img.shields.io/badge/Runtime-CPython%203.12-blue" alt="CPython 3.12">
<img src="https://img.shields.io/badge/Upstream-AstrBotDevs%2FAstrBot-181717?logo=github" alt="AstrBotDevs/AstrBot">
</div>

</div>

AstrBot Desktop is a packaged desktop distribution of AstrBot for local use. It bundles the WebUI, backend runtime, and desktop shell into a single app, making it a good fit for users who want a quick local setup with ChatUI, plugins, and knowledge base features. If you plan to run AstrBot on a server for long-term use, the upstream AstrBot source, Docker, or panel-based deployment is still the better choice.

<!-- section: best-fit -->
## Best Fit For

- You want to install AstrBot directly on Windows, macOS, or Linux without preparing a full command-line environment first.
- You mainly use ChatUI, the plugin marketplace, and the knowledge base on your local machine.
- You want your data stored in a local directory for easier backup, migration, and troubleshooting.
- You need both `stable` and `nightly` release channels for daily use or early access testing.

<!-- section: highlights -->
## Highlights

1. Ready-to-use desktop installation experience with the WebUI and backend runtime included by default.
2. Compatible with the upstream AstrBot ecosystem for local ChatUI, plugins, and common workflows.
3. Uses an isolated local data directory by default, making configuration, plugins, and logs easier to manage.
4. Provides both `stable` and `nightly` channels for stable usage or early access to recent changes.
5. Supports migration to source-based deployment, and migration back from source deployment to the desktop app.

<!-- section: one-click-install -->
## One-Click Install

If you only want to use the app and do not need to build it locally, download the installer for your platform from [`Releases`](https://github.com/AstrBotDevs/AstrBot-desktop/releases/latest).

- [Stable](https://github.com/AstrBotDevs/AstrBot-desktop/releases/latest): recommended for most users.
- [Nightly](https://github.com/AstrBotDevs/AstrBot-desktop/releases/tag/nightly): automatically built from newer upstream changes for early access to fixes and features.
- Choose the package that matches your operating system and CPU architecture.

<!-- section: data-and-configuration-location -->
## Data and Configuration Location

AstrBot Desktop stores the AstrBot root directory under the user home directory as `.astrbot` by default:

- macOS / Linux: `~/.astrbot`
- Windows: `C:\Users\<username>\.astrbot`

Common directories:

| Path | Description |
| --- | --- |
| `data/config/` | Configuration files |
| `data/plugins/` | Plugins |
| `data/plugin_data/` | Plugin data |
| `data/knowledge_base/` | Knowledge base data |
| `data/webchat/` | WebChat-related data |
| `logs/` | Desktop and backend logs |

If you need to migrate data between the desktop app and a source-based deployment, read [`docs/data-migration.md`](docs/data-migration.md) first.

<!-- section: updates-and-release-channels -->
## Updates and Release Channels

- `stable`: recommended for everyday use.
- `nightly`: closer to the latest upstream commits, suitable for testing new features or fixes.
- On Windows, macOS, and Linux AppImage builds, the desktop updater usually works directly in-app. Some Linux installation methods may still require manual download and installation.

<!-- section: faq -->
## FAQ

<!-- faq: server-deployment -->
### Is it suitable for server deployment?

Not really. AstrBot Desktop is intended for local desktop usage and personal workflows. If you need long-running, stable server deployment, use the upstream AstrBot source, Docker, or panel-based deployment instead.

<!-- faq: lan-webui-access -->
### How can I access the WebUI from another device on my LAN?

AstrBot Desktop listens on `127.0.0.1:6185` by default, so only the local machine can access the WebUI. If you explicitly want LAN access, set the dashboard host to `0.0.0.0` in the desktop config file.

Config file path:

```text
~/.astrbot/data/config/desktop.json
```

On Windows, this is usually:

```text
C:\Users\<username>\.astrbot\data\config\desktop.json
```

Write this content:

```json
{
  "dashboard": {
    "host": "0.0.0.0",
    "port": 6185
  }
}
```

Fully quit and restart AstrBot Desktop after saving the file, then visit this URL from another device:

```text
http://<LAN IP of the machine running AstrBot Desktop>:6185/
```

To restore local-only access, remove the `dashboard` config or set `host` back to `127.0.0.1`, then restart the app.

Environment variables still work as advanced overrides and take precedence over `desktop.json`:

```bash
ASTRBOT_DASHBOARD_HOST=0.0.0.0
ASTRBOT_DASHBOARD_PORT=6185
```

Before enabling LAN access, make sure your system firewall allows port `6185`, and do not expose the port on untrusted networks or the public internet.

<!-- faq: macos-quarantine -->
### macOS says the app is damaged or cannot be opened

If you downloaded the installer from the internet, macOS may attach a quarantine flag to the app. Run:

```bash
xattr -dr com.apple.quarantine /Applications/AstrBot.app
```

Then restart the app. If the app is not located in `/Applications`, replace the path with the actual one.

<!-- faq: missing-runtime-tools -->
### Why do some MCP tools say `node`, `npx`, or `uvx` is missing?

Some MCP tools depend on `node`, `npx`, or `uvx` from your system environment. These dependencies are not bundled with the desktop installer, so you need to install them separately.

- Node.js installation docs: <https://nodejs.org/>
- uv installation docs: <https://docs.astral.sh/uv/getting-started/installation/>

After installation, you can verify them yourself:

```bash
node -v
npm -v
npx -v
uvx --version
```

<!-- section: further-documentation -->
## Further Documentation

The following repository documents are currently written in Chinese:

- [`docs/data-migration.md`](docs/data-migration.md): data migration between the desktop app and source deployment.
- [`docs/development.md`](docs/development.md): local build, maintenance commands, version syncing, and release notes.
- [`docs/environment-variables.md`](docs/environment-variables.md): environment variable reference.
- [`docs/architecture.md`](docs/architecture.md): current desktop architecture.
- [`docs/repository-structure.md`](docs/repository-structure.md): repository structure overview.

<!-- section: feedback -->
## Feedback

If you need installation help or want to report issues and suggestions, you can reach out through:

- GitHub Issues: <https://github.com/AstrBotDevs/AstrBot-desktop/issues>
- QQ Group: 1060046189

<!-- section: license -->
## License

This project is licensed under `AGPL-3.0`. See [`LICENSE`](./LICENSE) for the full text.
