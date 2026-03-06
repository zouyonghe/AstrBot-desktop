import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const bootstrapPath = new URL('../../src-tauri/src/bridge_bootstrap.js', import.meta.url);

test('bridge bootstrap defines astrbotAppUpdater methods', async () => {
  const source = await readFile(bootstrapPath, 'utf8');

  assert.match(source, /window\.astrbotAppUpdater\s*=\s*\{/);
  assert.match(source, /getUpdateChannel:\s*\(\)\s*=>/);
  assert.match(source, /setUpdateChannel:\s*\(channel\)\s*=>/);
  assert.match(source, /checkForAppUpdate:\s*\(\)\s*=>/);
  assert.match(source, /installAppUpdate:\s*\(\)\s*=>/);
});
