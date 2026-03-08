# Disable CI AppImage Publishing Design

**Goal:** Stop publishing Linux AppImage artifacts from CI and remove AppImage entries from the official updater release pipeline, while keeping local AppImage builds available for manual validation.

## Context

The current Linux workflow still builds and uploads AppImage bundles in CI, and the release manifest generator publishes Linux AppImage updater entries. That turns AppImage into an upstream-supported release target.

Recent AppImage platform guidance does not support treating that target as low-risk infrastructure:

- AppImage runtime behavior still depends on host FUSE availability, and AppImage's own troubleshooting guidance documents distro-specific install and recovery steps.
- AppImage's Wayland notes document display-server integration issues, including cases where bundled Qt stacks miss the required Wayland platform plugin and need XWayland or environment workarounds.
- AppImage best-practice guidance also expects broad target validation against older Linux bases and bundled dependency compatibility, which our current GitHub Actions packaging path does not prove.

Given that gap, upstream CI should stop producing and advertising AppImage release artifacts until Linux graphics/runtime compatibility is better validated.

## Chosen Approach

Remove AppImage from the official CI and release-manifest pipeline, but keep runtime detection and local build capability untouched.

This means:

1. Linux GitHub Actions builds produce only `deb` and `rpm` bundles.
2. CI no longer uploads `.AppImage` or `.AppImage.sig` artifacts.
3. Release manifest generation no longer recognizes or emits Linux AppImage updater platforms.
4. Tests covering AppImage artifact normalization and manifest inclusion are removed or updated to reflect the new release contract.
5. The upstream PR body explicitly explains that AppImage release publishing is paused because Linux graphics/runtime compatibility is not stable enough for automated upstream distribution.

## Alternatives Considered

### 1. Only remove AppImage from the Linux workflow

Rejected because the release-manifest tooling and tests would still advertise AppImage as an official updater platform, leaving dead or misleading release logic in place.

### 2. Remove CI, release-manifest handling, and runtime AppImage updater logic

Rejected for now because it broadens the change from release-scope rollback into product-behavior rollback. Local experiments and future re-enablement are easier if runtime support remains isolated.

## Impact

- Official upstream releases stop shipping AppImage updater assets.
- Official `latest.json` payloads stop containing `linux-*-appimage` entries.
- Local developers can still build AppImage manually, but that path is no longer an upstream-published release guarantee.

## Verification

- Linux workflow contains no `appimage` bundle target and no AppImage artifact upload step.
- Updater manifest generation tests pass without Linux AppImage support.
- Release artifact normalization tests no longer assert AppImage canonicalization for `latest.json` generation.
- PR description explains the FUSE / Wayland / Linux compatibility rationale for temporarily removing upstream AppImage publishing.
