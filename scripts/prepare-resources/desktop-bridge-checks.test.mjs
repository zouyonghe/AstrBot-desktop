import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import {
  getDesktopBridgeExpectations,
  shouldEnforceDesktopBridgeExpectation,
} from './desktop-bridge-expectations.mjs';
import { patchDesktopReleaseUpdateIndicator } from './desktop-bridge-checks.mjs';

test('getDesktopBridgeExpectations returns stable expectation metadata', () => {
  const expectations = getDesktopBridgeExpectations();

  assert.ok(expectations.length > 0);
  assert.ok(expectations.some((expectation) => expectation.required === true));
  assert.ok(expectations.some((expectation) => expectation.required === false));
  assert.ok(expectations.some((expectation) => expectation.label === 'chat transport preference read'));
  assert.ok(expectations.some((expectation) => expectation.label === 'chat transport preference write'));
  assert.ok(
    expectations.some((expectation) => expectation.label === 'standalone chat transport preference read'),
  );

  for (const expectation of expectations) {
    assert.equal(Array.isArray(expectation.filePath), true);
    assert.equal(typeof expectation.label, 'string');
    assert.equal(expectation.pattern instanceof RegExp, true);
    assert.equal(typeof expectation.required, 'boolean');
  }
});

test('shouldEnforceDesktopBridgeExpectation always enforces in strict mode', () => {
  assert.equal(
    shouldEnforceDesktopBridgeExpectation(
      { required: false },
      { isDesktopBridgeExpectationStrict: true, isTaggedRelease: true },
    ),
    true,
  );
});

test('shouldEnforceDesktopBridgeExpectation skips optional expectations outside strict mode', () => {
  assert.equal(
    shouldEnforceDesktopBridgeExpectation(
      { required: false },
      { isDesktopBridgeExpectationStrict: false, isTaggedRelease: false },
    ),
    false,
  );
});

test('shouldEnforceDesktopBridgeExpectation downgrades required expectations on tagged release', () => {
  assert.equal(
    shouldEnforceDesktopBridgeExpectation(
      { required: true },
      { isDesktopBridgeExpectationStrict: false, isTaggedRelease: true },
    ),
    false,
  );
});

test('shouldEnforceDesktopBridgeExpectation enforces required expectations on non-tagged refs', () => {
  assert.equal(
    shouldEnforceDesktopBridgeExpectation(
      { required: true },
      { isDesktopBridgeExpectationStrict: false, isTaggedRelease: false },
    ),
    true,
  );
});

test('patchDesktopReleaseUpdateIndicator tolerates minor formatting changes in the upstream source', async () => {
  const dashboardDir = await mkdtemp(path.join(tmpdir(), 'astrbot-dashboard-'));
  const headerFile = path.join(
    dashboardDir,
    'src',
    'layouts',
    'full',
    'vertical-header',
    'VerticalHeader.vue',
  );
  await mkdir(path.dirname(headerFile), { recursive: true });
  await writeFile(
    headerFile,
    `function checkUpdate() {
  axios.get('/api/update/check')
    .then((res) => {
        hasNewVersion.value   =   res.data.data.has_new_version;
      if   ( res.data.data.has_new_version )   {
        releaseMessage.value = res.data.message;
        updateStatus.value = t('core.header.version.hasNewVersion');
      } else {
        updateStatus.value = res.data.message;
      }
      dashboardHasNewVersion.value = isDesktopReleaseMode.value
        ? false
        : res.data.data.dashboard_has_new_version;
    })
}
`,
    'utf8',
  );

  await patchDesktopReleaseUpdateIndicator({ dashboardDir, projectRoot: dashboardDir });

  const patched = await readFile(headerFile, 'utf8');
  assert.match(
    patched,
    /const backendHasNewVersion = !isDesktopReleaseMode\.value && res\.data\.data\.has_new_version;/,
  );
  assert.match(patched, /hasNewVersion\.value = backendHasNewVersion;/);
  assert.match(patched, /if \(backendHasNewVersion\) \{/);
  assert.doesNotMatch(patched, /hasNewVersion\.value = res\.data\.data\.has_new_version;/);
});

test('patchDesktopReleaseUpdateIndicator warns when the expected update pattern is missing', async () => {
  const dashboardDir = await mkdtemp(path.join(tmpdir(), 'astrbot-dashboard-'));
  const headerFile = path.join(
    dashboardDir,
    'src',
    'layouts',
    'full',
    'vertical-header',
    'VerticalHeader.vue',
  );
  await mkdir(path.dirname(headerFile), { recursive: true });
  await writeFile(
    headerFile,
    `function checkUpdate() {
  axios.get('/api/update/check')
    .then((res) => {
      updateStatus.value = res.data.message;
    })
}
`,
    'utf8',
  );

  const warnings = [];
  const originalWarn = console.warn;
  console.warn = (message) => warnings.push(String(message));

  try {
    await patchDesktopReleaseUpdateIndicator({ dashboardDir, projectRoot: dashboardDir });
  } finally {
    console.warn = originalWarn;
  }

  assert.equal(warnings.length, 1);
  assert.match(warnings[0], /Could not patch desktop release update banner gating/);
  assert.match(warnings[0], /VerticalHeader\.vue/);
});
