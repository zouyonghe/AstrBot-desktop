import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { test } from 'node:test';

import * as runtimeLayoutUtils from './runtime-layout-utils.mjs';

test('createPythonInstallEnv forces PYTHONDONTWRITEBYTECODE while preserving other env vars', () => {
  assert.equal(typeof runtimeLayoutUtils.createPythonInstallEnv, 'function');

  const env = runtimeLayoutUtils.createPythonInstallEnv({
    PATH: '/tmp/bin',
    PYTHONDONTWRITEBYTECODE: '0',
  });

  assert.equal(env.PATH, '/tmp/bin');
  assert.equal(env.PYTHONDONTWRITEBYTECODE, '1');
});

test('prunePythonBytecodeArtifacts removes bytecode files and cache directories recursively', () => {
  assert.equal(typeof runtimeLayoutUtils.prunePythonBytecodeArtifacts, 'function');

  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'astrbot-bytecode-fixture-'));
  const nestedPackageDir = path.join(fixtureRoot, 'python', 'lib', 'python3.12', 'site-packages', 'demo');
  const cacheDir = path.join(nestedPackageDir, '__pycache__');
  const nestedCacheDir = path.join(cacheDir, 'nested');
  const sourceFile = path.join(nestedPackageDir, 'module.py');
  const bytecodeFile = path.join(cacheDir, 'module.cpython-312.pyc');
  const nestedCacheFile = path.join(nestedCacheDir, 'metadata.txt');
  const orphanBytecodeFile = path.join(fixtureRoot, 'python', 'bin', 'tool.pyc');

  fs.mkdirSync(nestedCacheDir, { recursive: true });
  fs.mkdirSync(path.dirname(orphanBytecodeFile), { recursive: true });
  fs.writeFileSync(sourceFile, 'value = 1\n', 'utf8');
  fs.writeFileSync(bytecodeFile, 'bytecode', 'utf8');
  fs.writeFileSync(nestedCacheFile, 'metadata', 'utf8');
  fs.writeFileSync(orphanBytecodeFile, 'bytecode', 'utf8');

  const stats = runtimeLayoutUtils.prunePythonBytecodeArtifacts(fixtureRoot);

  assert.deepEqual(stats, {
    removedCacheDirs: 1,
    removedBytecodeFiles: 2,
    removedOrphanBytecodeFiles: 1,
  });
  assert.equal(fs.existsSync(cacheDir), false);
  assert.equal(fs.existsSync(bytecodeFile), false);
  assert.equal(fs.existsSync(nestedCacheFile), false);
  assert.equal(fs.existsSync(orphanBytecodeFile), false);
  assert.equal(fs.existsSync(sourceFile), true);

  fs.rmSync(fixtureRoot, { recursive: true, force: true });
});
