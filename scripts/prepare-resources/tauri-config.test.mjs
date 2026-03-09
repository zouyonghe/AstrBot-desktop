import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import assert from 'node:assert/strict';

const tauriConfigPath = new URL('../../src-tauri/tauri.conf.json', import.meta.url);

test('main Tauri window disables background throttling', async () => {
  const tauriConfig = JSON.parse(await readFile(tauriConfigPath, 'utf8'));
  const mainWindow = tauriConfig?.app?.windows?.[0];

  assert.ok(mainWindow, 'expected tauri config to define a main window');
  assert.equal(mainWindow.backgroundThrottling, 'disabled');
});
