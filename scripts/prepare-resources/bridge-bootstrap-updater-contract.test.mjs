import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const bootstrapPath = new URL('../../src-tauri/src/bridge_bootstrap.js', import.meta.url);
const chatTransportContractPath = new URL(
  '../../src-tauri/src/desktop_bridge_chat_transport_contract.json',
  import.meta.url,
);

test('bridge bootstrap defines astrbotAppUpdater methods', async () => {
  const source = await readFile(bootstrapPath, 'utf8');

  assert.match(source, /window\.astrbotAppUpdater\s*=\s*\{/);
  assert.match(source, /getUpdateChannel:\s*\(\)\s*=>/);
  assert.match(source, /setUpdateChannel:\s*\(channel\)\s*=>/);
  assert.match(source, /checkForAppUpdate:\s*\(\)\s*=>/);
  assert.match(source, /installAppUpdate:\s*\(\)\s*=>/);
});

test('bridge bootstrap transport placeholders are backed by the shared contract', async () => {
  const [source, rawContract] = await Promise.all([
    readFile(bootstrapPath, 'utf8'),
    readFile(chatTransportContractPath, 'utf8'),
  ]);
  const contract = JSON.parse(rawContract);

  assert.equal(typeof contract.storageKey, 'string');
  assert.equal(typeof contract.websocketValue, 'string');
  assert.match(source, /if \(typeof window === 'undefined'\) return;/);
  assert.match(source, /\{CHAT_TRANSPORT_MODE_STORAGE_KEY\}/);
  assert.match(source, /\{CHAT_TRANSPORT_MODE_WEBSOCKET\}/);
});
