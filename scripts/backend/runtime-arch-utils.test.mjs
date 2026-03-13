import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  isWindowsArm64BundledRuntime,
  resolveBundledRuntimeArch,
} from './runtime-arch-utils.mjs';

test('resolveBundledRuntimeArch defaults Windows ARM64 backend runtime to x64', () => {
  assert.equal(
    resolveBundledRuntimeArch({ platform: 'win32', arch: 'arm64', env: {} }),
    'amd64',
  );
});

test('resolveBundledRuntimeArch honors explicit target arch on emulated x64 Node', () => {
  assert.equal(
    resolveBundledRuntimeArch({
      platform: 'win32',
      arch: 'x64',
      env: { ASTRBOT_DESKTOP_TARGET_ARCH: 'arm64' },
    }),
    'amd64',
  );

  assert.equal(
    resolveBundledRuntimeArch({
      platform: 'win32',
      arch: 'x64',
      env: {
        ASTRBOT_DESKTOP_TARGET_ARCH: 'arm64',
        ASTRBOT_DESKTOP_WINDOWS_ARM_BACKEND_ARCH: 'arm64',
      },
    }),
    'arm64',
  );
});

test('isWindowsArm64BundledRuntime uses explicit bundled runtime arch handoff', () => {
  assert.equal(
    isWindowsArm64BundledRuntime({
      platform: 'win32',
      arch: 'x64',
      env: { ASTRBOT_DESKTOP_BUNDLED_RUNTIME_ARCH: 'arm64' },
    }),
    true,
  );

  assert.equal(
    isWindowsArm64BundledRuntime({
      platform: 'win32',
      arch: 'x64',
      env: { ASTRBOT_DESKTOP_BUNDLED_RUNTIME_ARCH: 'amd64' },
    }),
    false,
  );
});
