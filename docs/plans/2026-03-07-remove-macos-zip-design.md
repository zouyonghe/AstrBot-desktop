# Remove macOS Zip Design

## Goal

Stop producing and accepting macOS `.zip` artifacts so the release pipeline only publishes the `.app.tar.gz` archives that Tauri updater actually uses.

## Current State

- The macOS workflow still packages a manual-download `.zip` alongside the real updater archive.
- The updater manifest generator still accepts macOS `.zip.sig` files as valid updater inputs.
- This creates an ambiguous release shape where a non-updater macOS artifact looks updater-adjacent.

## Decision

Remove macOS `.zip` from both the build pipeline and updater parsing rules.

That means:

- delete the workflow step that packages `bundle/zip/*.zip`
- stop uploading macOS `.zip` artifacts
- stop recognizing macOS `.zip.sig` as a valid updater signature input
- keep `.app.tar.gz` / `.app.tar.gz.sig` as the only macOS updater artifacts

## Why

- Tauri updater on macOS installs `.app.tar.gz`, not `.zip`
- keeping `.zip` around increases user and maintainer confusion
- removing parser support prevents future accidental reintroduction through stray assets

## Impact

- macOS release artifacts become narrower and clearer
- updater behavior stays aligned with Tauri's real installer expectations
- users lose the extra manual-download `.zip`, but no updater functionality is lost

## Verification

- workflow no longer references macOS `bundle/zip/*.zip`
- manifest generator rejects macOS `.zip.sig`
- `.app.tar.gz` macOS updater tests still pass
- project script tests still pass
