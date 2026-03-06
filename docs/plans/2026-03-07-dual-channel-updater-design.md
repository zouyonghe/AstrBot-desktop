# Dual-Channel Updater Design

**Date:** 2026-03-07

**Goal:** Add stable / nightly updater channels with explicit user-controlled channel switching and forward-only upgrade rules.

## Requirements

- Default automatic update checks stay on the app's current preferred channel.
- `stable -> stable` and `nightly -> nightly` upgrades continue to work normally.
- Users may explicitly switch channels.
- Channel switches must never downgrade the effective base version.
- `stable 4.29.x -> nightly 4.29.x` is allowed only after the user switches to `nightly`.
- `nightly 4.29.x -> stable 4.29.x` is not allowed.
- `nightly 4.29.x -> stable 4.30.x` is allowed.

## Approach

Use two updater manifests and resolve the effective updater endpoint at runtime.

- Stable manifest: `latest-stable.json`
- Nightly manifest: `latest-nightly.json`
- Stable endpoint: GitHub Releases `latest` alias
- Nightly endpoint: fixed `nightly` release tag

The Tauri updater plugin remains enabled, but bridge commands stop relying on the static endpoint from `tauri.conf.json`. Instead, each update check/install builds an updater instance with:

- the preferred channel's manifest URL
- a custom version comparator that enforces AstrBot's channel transition rules

## Channel State

Persist `updateChannel` in the existing `desktop_state.json` file alongside shell locale state.

- If no explicit channel is stored, infer the default from the installed version:
  - prerelease containing `nightly` => `nightly`
  - otherwise => `stable`
- Once the user changes channel, future update checks follow the stored preference.

This allows a stable build to opt into future nightly builds before the nightly update is installed, and likewise allows a nightly build to opt back into stable while still blocking same-base downgrades.

## Upgrade Rules

Let `current` be the installed app version and `remote` the target manifest version.

- Same channel, `stable -> stable`: require `remote > current`
- Same channel, `nightly -> nightly`: compare `baseVersion` first; if equal, newer nightly date wins, and same-date different-hash nightly is also allowed
- Cross-channel, `stable -> nightly`: require `remote.baseVersion >= current.baseVersion`
- Cross-channel, `nightly -> stable`: require `remote.baseVersion > current.baseVersion`

`baseVersion` means the semver value with any `-nightly.*` suffix removed.

## Workflow Changes

- Generate channel-specific updater manifests during release publishing.
- Stable releases upload `latest-stable.json`.
- Nightly releases upload `latest-nightly.json`.
- Manifest payloads include `channel`, `baseVersion`, and `releaseTag` in addition to the Tauri-required fields.

Tauri ignores unknown manifest fields, so the extra metadata is safe for the plugin while remaining useful for future UI/debugging work.

## Client Surface

Extend `window.astrbotAppUpdater` with channel APIs:

- `getUpdateChannel()`
- `setUpdateChannel(channel)`
- `checkForAppUpdate()`
- `installAppUpdate()`

`checkForAppUpdate()` and `installAppUpdate()` always use the current preferred channel and the custom comparator logic.

## Verification

- Rust unit tests for channel parsing, persistence defaults, and upgrade rule comparisons.
- JS bridge contract test for the new updater methods.
- Python unit tests for the manifest generator helper logic.
- Focused `cargo test`, `node --test`, and `python3 -m unittest` runs.
