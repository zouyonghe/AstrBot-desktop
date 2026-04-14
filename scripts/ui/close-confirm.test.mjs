import assert from 'node:assert/strict';
import path from 'node:path';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, '..', '..');
const htmlPath = path.join(projectRoot, 'ui', 'close-confirm.html');

const readHtml = async () => readFile(htmlPath, 'utf8');

test('close confirm dialog uses locale copy as the single source of truth for button labels', async () => {
  const html = await readHtml();

  assert.match(html, /trayButton\.textContent = copy\.tray;/);
  assert.match(html, /exitButton\.textContent = copy\.exit;/);
});

test('close confirm dialog avoids exposing raw invoke errors to users', async () => {
  const html = await readHtml();

  assert.doesNotMatch(html, /invokeError\.message/);
  assert.match(html, /error\.textContent = copy\.submitError;/);
});

test('close confirm dialog routes Tauri command calls through a local invoke wrapper', async () => {
  const html = await readHtml();

  assert.match(html, /const invokeTauri =/);
  assert.doesNotMatch(html, /window\.__TAURI_INTERNALS__\?\.invoke/);
  assert.match(html, /await invokeTauri\(/);
});

test('close confirm dialog reads close action values from query params instead of hard-coded literals', async () => {
  const html = await readHtml();

  assert.match(html, /const trayAction = params\.get\("trayAction"\);/);
  assert.match(html, /const exitAction = params\.get\("exitAction"\);/);
  assert.doesNotMatch(html, /submit\("tray"\)/);
  assert.doesNotMatch(html, /submit\("exit"\)/);
});

test('close confirm dialog only schedules frontend close fallback for tray actions', async () => {
  const html = await readHtml();

  assert.match(html, /if \(action === trayAction\) \{/);
  assert.match(html, /recoveryTimer = window\.setTimeout\(/);
  assert.match(html, /window\.close\(\);/);
});

test('close confirm dialog suppresses invoke teardown errors for exit actions', async () => {
  const html = await readHtml();

  assert.match(html, /catch \(_invokeError\) \{/);
  assert.match(html, /if \(action === exitAction\) \{/);
  assert.match(html, /return;/);
});
