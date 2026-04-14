import assert from 'node:assert/strict';
import path from 'node:path';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, '..', '..');
const workflowPath = path.join(projectRoot, '.github', 'workflows', 'build-desktop-tauri.yml');

const readWorkflow = async () => readFile(workflowPath, 'utf8');

test('macOS workflow prepares resources before any conditional pre-signing', async () => {
  const workflow = await readWorkflow();

  assert.match(
    workflow,
    /- name: Prepare desktop resources \(macOS\)[\s\S]*?run: \|[\s\S]*?pnpm run prepare:resources/,
  );
  assert.doesNotMatch(
    workflow,
    /- name: Pre-sign backend resources \(macOS\)[\s\S]*?run: \|[\s\S]*?pnpm run prepare:resources/,
  );
  assert.match(
    workflow,
    /Build desktop app bundle \(macOS\)[\s\S]*?# Resources are already prepared and, when available, pre-signed in earlier steps\./,
  );
});
