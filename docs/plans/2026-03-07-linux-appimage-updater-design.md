# Linux AppImage Updater Design

## Goal

Add Linux AppImage build output and enable desktop self-update only for AppImage installs, while keeping Linux `deb` and `rpm` installs on a manual download path.

## Desired User Experience

- Windows and macOS continue to use the native updater path.
- Linux AppImage installs can check for and install updates through the desktop app updater flow.
- Linux `deb` and `rpm` installs do not attempt automatic updater installation.
- Linux `deb` and `rpm` users still get a clear message telling them to download the latest release package manually.

## Why AppImage Only

Linux distribution packages (`deb`, `rpm`) are normally updated through the system package manager, not by an embedded self-updater.

AppImage is the Linux packaging format that best matches the updater model already used by Tauri. That makes it the correct target for Linux desktop self-update support.

## Architecture

### 1. Build outputs

The Linux CI job should produce:

- `deb`
- `rpm`
- `AppImage`

Only AppImage artifacts should be represented in the updater manifest.

### 2. Updater manifest

`scripts/ci/generate-tauri-latest-json.py` should be extended to detect Linux AppImage updater assets and include them in `latest.json` using the correct Linux updater platform key.

`deb` and `rpm` artifacts should remain release assets, but should not be placed into the updater manifest.

### 3. Runtime update mode resolution

The current updater support logic is too coarse because it treats support as a simple platform boolean.

This should be replaced with a centralized helper that resolves update mode based on runtime platform and install channel.

Suggested states:

- `NativeUpdater`
- `ManualDownload`
- `Unsupported`

Linux should resolve as:

- AppImage runtime: `NativeUpdater`
- non-AppImage Linux runtime: `ManualDownload`

### 4. Linux install channel detection

AppImage detection should use the runtime signals normally available in AppImage environments, especially:

- `APPIMAGE`
- `APPDIR`

If those are present, the runtime should treat the current installation as AppImage.

If not present on Linux, the runtime should assume a non-self-updating distribution package or manual package install and return a manual-download path instead of trying the native updater.

### 5. Bridge behavior

The updater bridge should continue to expose the same WebUI-facing methods.

Behavior by mode:

- `NativeUpdater`
  - check/update through the updater plugin
- `ManualDownload`
  - check may still expose current version and a manual-download reason
  - install should return a failure state with a clear release-download message

If the frontend later needs a richer UX, the bridge can be extended with an explicit `updateMode`, but this is not required for the first implementation.

## CI / Release Considerations

- Linux AppImage artifacts and signatures must be uploaded into `release-artifacts/`
- `latest.json` must include Linux AppImage entries only
- `deb` and `rpm` should still be published as normal release assets

## Testing Strategy

- Rust tests for Linux update mode resolution
- Rust tests for AppImage vs manual-download updater behavior
- Python validation for AppImage artifact name parsing in `generate-tauri-latest-json.py`
- CI verification that Linux release outputs contain AppImage and AppImage signature files

## Success Criteria

- Linux CI produces AppImage artifacts alongside `deb` and `rpm`
- `latest.json` contains Linux AppImage updater entries
- Linux AppImage runtime uses native updater flow
- Linux `deb`/`rpm` runtime clearly reports manual-download behavior instead of pretending to support self-update
