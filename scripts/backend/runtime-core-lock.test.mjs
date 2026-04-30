import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

import { generateRuntimeCoreLock } from './runtime-core-lock.mjs';

const resolvePython = () => process.env.PYTHON || (process.platform === 'win32' ? 'python' : 'python3');
const launcherTemplatePath = fileURLToPath(new URL('./templates/launch_backend.py', import.meta.url));
const generatorScriptPath = fileURLToPath(new URL('./tools/generate_runtime_core_lock.py', import.meta.url));

const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const shellQuote = (value) => `'${value.replace(/'/g, `'\\''`)}'`;

const createFakeRuntime = (fixtureRoot, jsSource) => {
  const driverPath = path.join(fixtureRoot, 'fake-runtime.js');
  fs.writeFileSync(driverPath, jsSource, 'utf8');

  if (process.platform === 'win32') {
    const wrapperPath = path.join(fixtureRoot, 'fake-runtime.cmd');
    fs.writeFileSync(wrapperPath, `@echo off\r\n"${process.execPath}" "${driverPath}" %*\r\n`, 'utf8');
    return wrapperPath;
  }

  const wrapperPath = path.join(fixtureRoot, 'fake-runtime');
  fs.writeFileSync(
    wrapperPath,
    `#!/bin/sh\nexec ${shellQuote(process.execPath)} ${shellQuote(driverPath)} "$@"\n`,
    'utf8',
  );
  fs.chmodSync(wrapperPath, 0o755);
  return wrapperPath;
};

test('generateRuntimeCoreLock writes installed distribution metadata', () => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'astrbot-runtime-core-lock-'));
  const outputPath = path.join(fixtureRoot, 'runtime-core-lock.json');

  try {
    generateRuntimeCoreLock({
      runtimePython: { absolute: resolvePython() },
      outputPath,
    });

    const lock = JSON.parse(fs.readFileSync(outputPath, 'utf8'));

    assert.equal(lock.version, 1);
    assert.equal(Array.isArray(lock.distributions), true);
    assert.ok(lock.distributions.length > 0);
    assert.ok(lock.distributions.some((dist) => dist.name && dist.version));
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('backend build invokes runtime core lock generation before manifest output', async () => {
  const buildBackendPath = new URL('./build-backend.mjs', import.meta.url);
  const source = await readFile(buildBackendPath, 'utf8');

  assert.match(source, /generateRuntimeCoreLock/);
  assert.match(source, /runtime-core-lock\.json/);
  assert.match(source, /generateRuntimeCoreLock\(\{\s*runtimePython/s);
});

test('backend launcher sets the runtime core lock path when the lock exists', () => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'astrbot-launch-backend-'));
  const appDir = path.join(fixtureRoot, 'app');
  const lockPath = path.join(appDir, 'runtime-core-lock.json');
  fs.mkdirSync(appDir, { recursive: true });
  fs.writeFileSync(lockPath, '{}');

  const script = String.raw`
import importlib.util
import os
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("launch_backend", sys.argv[1])
module = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(module)

module.APP_DIR = Path(sys.argv[2])
os.environ.pop(module.RUNTIME_CORE_LOCK_ENV, None)
module.configure_runtime_core_lock_path()
print(os.environ.get(module.RUNTIME_CORE_LOCK_ENV, ""))
`;

  try {
    const result = spawnSync(resolvePython(), ['-c', script, launcherTemplatePath, appDir], {
      encoding: 'utf8',
    });
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.equal(result.stdout.trim(), lockPath);
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('backend launcher preserves an explicit runtime core lock override', () => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'astrbot-launch-backend-override-'));
  const appDir = path.join(fixtureRoot, 'app');
  const lockPath = path.join(appDir, 'runtime-core-lock.json');
  const overridePath = path.join(fixtureRoot, 'override-lock.json');
  fs.mkdirSync(appDir, { recursive: true });
  fs.writeFileSync(lockPath, '{}');

  const script = String.raw`
import importlib.util
import os
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("launch_backend", sys.argv[1])
module = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(module)

module.APP_DIR = Path(sys.argv[2])
os.environ[module.RUNTIME_CORE_LOCK_ENV] = sys.argv[3]
module.configure_runtime_core_lock_path()
print(os.environ.get(module.RUNTIME_CORE_LOCK_ENV, ""))
`;

  try {
    const result = spawnSync(
      resolvePython(),
      ['-c', script, launcherTemplatePath, appDir, overridePath],
      { encoding: 'utf8' },
    );
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.equal(result.stdout.trim(), overridePath);
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('generateRuntimeCoreLock creates the output file when parent directories are missing', () => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'astrbot-runtime-core-lock-nested-'));
  const outputPath = path.join(fixtureRoot, 'nested', 'path', 'runtime-core-lock.json');

  try {
    generateRuntimeCoreLock({
      runtimePython: { absolute: resolvePython() },
      outputPath,
    });

    assert.equal(fs.existsSync(outputPath), true);
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('generateRuntimeCoreLock reports generator context for missing runtime python', () => {
  assert.throws(
    () =>
      generateRuntimeCoreLock({
        runtimePython: {},
        outputPath: '/tmp/runtime-core-lock.json',
      }),
    (error) => {
      assert.equal(error instanceof Error, true);
      assert.match(error.message, /Missing runtime Python executable/);
      assert.match(error.message, /python: undefined/);
      assert.match(error.message, new RegExp(`script: ${escapeRegExp(generatorScriptPath)}`));
      return true;
    },
  );
});

test('generateRuntimeCoreLock reports generator context for missing output path', () => {
  const pythonPath = resolvePython();

  assert.throws(
    () =>
      generateRuntimeCoreLock({
        runtimePython: { absolute: pythonPath },
      }),
    (error) => {
      assert.equal(error instanceof Error, true);
      assert.match(error.message, /Missing output path/);
      assert.match(error.message, new RegExp(`python: ${escapeRegExp(pythonPath)}`));
      assert.match(error.message, new RegExp(`script: ${escapeRegExp(generatorScriptPath)}`));
      return true;
    },
  );
});

test('generateRuntimeCoreLock reports generator context for process launch failures', () => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'astrbot-runtime-core-lock-launch-error-'));
  const missingPythonPath = path.join(fixtureRoot, 'missing-python');

  try {
    assert.throws(
      () =>
        generateRuntimeCoreLock({
          runtimePython: { absolute: missingPythonPath },
          outputPath: path.join(fixtureRoot, 'runtime-core-lock.json'),
        }),
      (error) => {
        assert.equal(error instanceof Error, true);
        assert.match(error.message, /Failed to generate runtime core lock/);
        assert.match(error.message, new RegExp(`python: ${escapeRegExp(missingPythonPath)}`));
        assert.match(error.message, new RegExp(`script: ${escapeRegExp(generatorScriptPath)}`));
        return true;
      },
    );
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('generateRuntimeCoreLock reports generator context for non-zero exits', () => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'astrbot-runtime-core-lock-exit-error-'));

  try {
    assert.throws(
      () =>
        generateRuntimeCoreLock({
          runtimePython: { absolute: process.execPath },
          outputPath: path.join(fixtureRoot, 'runtime-core-lock.json'),
        }),
      (error) => {
        assert.equal(error instanceof Error, true);
        assert.match(error.message, /Runtime core lock generation failed/);
        assert.match(error.message, new RegExp(`python: ${escapeRegExp(process.execPath)}`));
        assert.match(error.message, new RegExp(`script: ${escapeRegExp(generatorScriptPath)}`));
        return true;
      },
    );
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('generateRuntimeCoreLock reports signal-based termination clearly', () => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'astrbot-runtime-core-lock-signal-error-'));
  const fakeRuntimePath = createFakeRuntime(
    fixtureRoot,
    `process.kill(process.pid, ${JSON.stringify(process.platform === 'win32' ? 'SIGTERM' : 'SIGTERM')});\n`,
  );

  try {
    assert.throws(
      () =>
        generateRuntimeCoreLock({
          runtimePython: { absolute: fakeRuntimePath },
          outputPath: path.join(fixtureRoot, 'runtime-core-lock.json'),
        }),
      (error) => {
        assert.equal(error instanceof Error, true);
        assert.match(error.message, /Runtime core lock generation failed/);
        assert.match(error.message, /terminated by signal SIGTERM/);
        assert.doesNotMatch(error.message, /exit code null/);
        assert.match(error.message, new RegExp(`python: ${escapeRegExp(fakeRuntimePath)}`));
        assert.match(error.message, new RegExp(`script: ${escapeRegExp(generatorScriptPath)}`));
        return true;
      },
    );
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('generateRuntimeCoreLock rejects invalid generated lock content', () => {
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'astrbot-runtime-core-lock-invalid-output-'));
  const fakeRuntimePath = createFakeRuntime(
    fixtureRoot,
    `
const fs = require('node:fs');
const outputPath = process.argv[4];
fs.mkdirSync(require('node:path').dirname(outputPath), { recursive: true });
fs.writeFileSync(outputPath, 'not json', 'utf8');
`,
  );

  try {
    assert.throws(
      () =>
        generateRuntimeCoreLock({
          runtimePython: { absolute: fakeRuntimePath },
          outputPath: path.join(fixtureRoot, 'runtime-core-lock.json'),
        }),
      (error) => {
        assert.equal(error instanceof Error, true);
        assert.match(error.message, /did not create valid/);
        assert.match(error.message, new RegExp(escapeRegExp(path.join(fixtureRoot, 'runtime-core-lock.json'))));
        assert.match(error.message, new RegExp(`python: ${escapeRegExp(fakeRuntimePath)}`));
        assert.match(error.message, new RegExp(`script: ${escapeRegExp(generatorScriptPath)}`));
        return true;
      },
    );
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('runtime core lock helper only suppresses missing top-level metadata', () => {
  const script = String.raw`
import importlib.util
import sys

spec = importlib.util.spec_from_file_location("runtime_core_lock_helper", sys.argv[1])
module = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(module)

class MissingTopLevel:
    def read_text(self, name):
        raise FileNotFoundError

class BrokenEncoding:
    def read_text(self, name):
        raise UnicodeDecodeError("utf-8", b"\x80", 0, 1, "invalid start byte")

class UnexpectedFailure:
    def read_text(self, name):
        raise RuntimeError("boom")

assert module._read_top_level_modules(MissingTopLevel()) == []
assert module._read_top_level_modules(BrokenEncoding()) == []

try:
    module._read_top_level_modules(UnexpectedFailure())
except RuntimeError as exc:
    assert str(exc) == "boom"
else:
    raise AssertionError("RuntimeError was suppressed")
`;

  const result = spawnSync(resolvePython(), ['-c', script, generatorScriptPath], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr || result.stdout);
});
