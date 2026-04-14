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
