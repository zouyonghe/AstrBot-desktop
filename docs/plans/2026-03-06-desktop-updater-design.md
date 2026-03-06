# Desktop App Updater Design

## Goal

Implement a real desktop self-update path for AstrBot Desktop so the WebUI's updater entry can check for desktop releases and install them, instead of falling back to an always-failed bridge path.

## Current Problem

The current desktop shell and the upstream WebUI are out of sync:

- The upstream WebUI already calls `window.astrbotAppUpdater.checkForAppUpdate()` and `window.astrbotAppUpdater.installAppUpdate()`.
- The desktop shell currently injects `window.astrbotDesktop`, but does not inject `window.astrbotAppUpdater`.
- Tauri commands only expose desktop runtime and backend control functions.
- Tauri updater support is not configured in Cargo, `tauri.conf.json`, or the release workflow.

As a result, the update button in desktop mode does not represent a flaky path. It is structurally unimplemented and therefore fails every time.

## Desired Outcome

The desktop application should support a full app-update flow:

1. WebUI asks the desktop shell for current app version and available updates.
2. The desktop shell checks release metadata through Tauri updater.
3. If an update is available, the WebUI can show release information.
4. The user can trigger install/update through the desktop bridge.
5. CI publishes the updater metadata and signatures required by Tauri updater.

## Recommended Approach

Use Tauri's official updater plugin and align the desktop bridge around that capability.

This is the best fit because:

- the project already releases desktop artifacts through GitHub Releases
- the WebUI already expects an app-updater bridge abstraction
- Tauri updater provides the native install/update mechanics instead of forcing a custom downloader

## Architecture

### 1. Frontend contract

Keep the WebUI contract centered around `window.astrbotAppUpdater`.

The desktop shell should expose methods matching the upstream expectation. At minimum:

- `getCurrentVersion()`
- `checkForAppUpdate()`
- `installAppUpdate()`

If the upstream `desktop-bridge.d.ts` is already canonical, the shell should follow that contract exactly rather than inventing a parallel one.

### 2. Desktop bridge

The bridge bootstrap should inject a dedicated `window.astrbotAppUpdater` object, not hide updater functions under `window.astrbotDesktop`.

This bridge will route calls to new Tauri commands in `src-tauri/src/bridge/commands.rs`.

### 3. Tauri updater integration

The Tauri runtime should add the updater plugin and expose bridge commands that:

- get current desktop version
- check whether a newer app release exists
- install the downloaded update

The runtime should convert updater plugin results into stable bridge return structures so the frontend does not depend on plugin internals.

### 4. Configuration and secrets

The desktop app needs updater configuration in `src-tauri/tauri.conf.json`.

This includes at least:

- updater endpoints
- updater public key

The signing private key must live in CI secrets and never in the repository.

### 5. Release workflow

The GitHub Actions release flow must publish updater-compatible metadata, signatures, and platform artifacts.

Uploading only installer assets is not enough. The updater client needs release metadata and signed assets in the format expected by Tauri updater.

## API Shape

The exact return types should follow upstream expectations, but the runtime should support these states clearly:

- no update available
- update available with version metadata
- check failed with reason
- install started / install completed / restart required
- install failed with reason

The bridge should avoid `undefined`-driven behavior. Any unavailable capability should return a structured error state instead.

## Error Handling

- Missing updater configuration should surface as an explicit bridge failure state.
- Release metadata parse failures should return a structured reason.
- Installation failures should surface a reason suitable for desktop logs and frontend display.
- Unsupported platforms should return a clear unsupported result rather than silently failing.

## Platform and Release Notes

- Windows and macOS support must be verified against the packaging formats currently produced in CI.
- Linux support should only be exposed if the produced package format is actually supported by the updater pipeline.
- Release notes shown in the WebUI should come from the release metadata returned by the updater path when available.

## Testing Strategy

### Bridge and runtime

- Add tests for updater command result mapping where the logic is pure.
- Add tests for injected bridge contract shape if possible.

### Config and CI

- Verify updater configuration is present and loadable.
- Verify release workflow generates updater assets and signatures.

### End-to-end validation

- Create a test release in GitHub Releases.
- Install an older desktop build.
- Confirm desktop can detect the update and install it successfully.

## Implementation Sequence

1. Define updater bridge contract.
2. Add Tauri updater plugin and commands.
3. Inject `window.astrbotAppUpdater` in the bootstrap script.
4. Add updater config to `tauri.conf.json`.
5. Update CI/release workflow to publish signed updater artifacts.
6. Validate with a real release.

## Success Criteria

This work is successful when:

- the desktop update button no longer falls through to the current failure path
- `window.astrbotAppUpdater` exists in desktop runtime
- desktop builds can detect available updates from release metadata
- desktop installs can apply updates through the official updater mechanism
- CI publishes the metadata required for the updater flow to work in production
