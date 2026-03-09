import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import assert from 'node:assert/strict';

const tauriConfigPath = new URL('../../src-tauri/tauri.conf.json', import.meta.url);

test('main Tauri window disables background throttling', async () => {
  const tauriConfig = JSON.parse(await readFile(tauriConfigPath, 'utf8'));
  const windows = tauriConfig?.app?.windows;

  assert.ok(Array.isArray(windows), 'expected tauri config app.windows to be an array');

  const mainWindow = windows.find((windowConfig) => windowConfig.label === 'main');

  assert.ok(mainWindow, 'expected tauri config to define a main window');
  assert.equal(
    mainWindow.backgroundThrottling,
    'disabled',
    'expected the main window to disable background throttling',
  );
});
