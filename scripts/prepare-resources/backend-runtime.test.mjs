import assert from 'node:assert/strict';
import { test } from 'node:test';

import * as backendRuntime from './backend-runtime.mjs';

const resolvePbsTarget = (options) => backendRuntime.resolvePbsTarget(options);

test('resolvePbsTarget defaults Windows ARM64 backend runtime to x64', () => {
  assert.equal(
    resolvePbsTarget({ platform: 'win32', arch: 'arm64', env: {} }),
    'x86_64-pc-windows-msvc',
  );
});

test('resolvePbsTarget accepts explicit Windows ARM64 backend overrides', () => {
  assert.equal(
    resolvePbsTarget({
      platform: 'win32',
      arch: 'arm64',
      env: { ASTRBOT_DESKTOP_WINDOWS_ARM_BACKEND_ARCH: 'amd64' },
    }),
    'x86_64-pc-windows-msvc',
  );

  assert.equal(
    resolvePbsTarget({
      platform: 'win32',
      arch: 'arm64',
      env: { ASTRBOT_DESKTOP_WINDOWS_ARM_BACKEND_ARCH: 'x64' },
    }),
    'x86_64-pc-windows-msvc',
  );

  assert.equal(
    resolvePbsTarget({
      platform: 'win32',
      arch: 'arm64',
      env: { ASTRBOT_DESKTOP_WINDOWS_ARM_BACKEND_ARCH: 'arm64' },
    }),
    'aarch64-pc-windows-msvc',
  );

  assert.equal(
    resolvePbsTarget({
      platform: 'win32',
      arch: 'arm64',
      env: { ASTRBOT_DESKTOP_WINDOWS_ARM_BACKEND_ARCH: 'aarch64' },
    }),
    'aarch64-pc-windows-msvc',
  );
});

test('resolvePbsTarget honors the explicit desktop target arch when process arch is emulated x64', () => {
  assert.equal(
    resolvePbsTarget({
      platform: 'win32',
      arch: 'x64',
      env: { ASTRBOT_DESKTOP_TARGET_ARCH: 'arm64' },
    }),
    'x86_64-pc-windows-msvc',
  );

  assert.equal(
    resolvePbsTarget({
      platform: 'win32',
      arch: 'x64',
      env: {
        ASTRBOT_DESKTOP_TARGET_ARCH: 'arm64',
        ASTRBOT_DESKTOP_WINDOWS_ARM_BACKEND_ARCH: 'arm64',
      },
    }),
    'aarch64-pc-windows-msvc',
  );
});

test('resolvePbsTarget rejects invalid Windows ARM64 backend override values', () => {
  assert.throws(
    () =>
      resolvePbsTarget({
        platform: 'win32',
        arch: 'arm64',
        env: { ASTRBOT_DESKTOP_WINDOWS_ARM_BACKEND_ARCH: 'wat' },
      }),
    /ASTRBOT_DESKTOP_WINDOWS_ARM_BACKEND_ARCH[\s\S]*amd64[\s\S]*x64[\s\S]*arm64[\s\S]*aarch64/,
  );
});

test('resolvePbsTarget rejects invalid explicit desktop target arch values', () => {
  assert.throws(
    () =>
      resolvePbsTarget({
        platform: 'win32',
        arch: 'x64',
        env: { ASTRBOT_DESKTOP_TARGET_ARCH: 'wat' },
      }),
    /ASTRBOT_DESKTOP_TARGET_ARCH[\s\S]*amd64[\s\S]*x64[\s\S]*arm64[\s\S]*aarch64/,
  );
});

test('resolvePbsTarget keeps same-arch mappings for other platform and arch combinations', () => {
  const cases = [
    { platform: 'linux', arch: 'x64', expected: 'x86_64-unknown-linux-gnu' },
    { platform: 'linux', arch: 'arm64', expected: 'aarch64-unknown-linux-gnu' },
    { platform: 'darwin', arch: 'x64', expected: 'x86_64-apple-darwin' },
    { platform: 'darwin', arch: 'arm64', expected: 'aarch64-apple-darwin' },
    { platform: 'win32', arch: 'x64', expected: 'x86_64-pc-windows-msvc' },
  ];

  for (const { platform, arch, expected } of cases) {
    assert.equal(resolvePbsTarget({ platform, arch, env: {} }), expected);
  }
});
