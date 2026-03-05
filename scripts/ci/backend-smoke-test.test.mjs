import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { test } from 'node:test';

import { main, parseCliOptions, runCli, usageMessage } from './backend-smoke-test.mjs';

const createFixtureLayout = async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'astrbot-backend-smoke-test-'));
  const backendDir = path.join(root, 'backend');
  const webuiDir = path.join(root, 'webui');
  const appDir = path.join(backendDir, 'app');
  const pythonDir = path.join(backendDir, 'python');
  const launcherPath = path.join(backendDir, 'launch_backend.py');
  const mainPath = path.join(appDir, 'main.py');
  const pythonPath = path.join(pythonDir, 'python');

  await mkdir(appDir, { recursive: true });
  await mkdir(pythonDir, { recursive: true });
  await mkdir(webuiDir, { recursive: true });
  await writeFile(launcherPath, '# launcher', 'utf8');
  await writeFile(mainPath, '# main', 'utf8');
  await writeFile(pythonPath, '#!/bin/sh\n', 'utf8');
  await writeFile(
    path.join(backendDir, 'runtime-manifest.json'),
    JSON.stringify({ python: 'python/python' }),
    'utf8',
  );

  return { root, backendDir, webuiDir };
};

const createFakeChild = () => {
  const child = new EventEmitter();
  child.pid = 12345;
  child.exitCode = null;
  child.stdout = new EventEmitter();
  child.stderr = new EventEmitter();
  child.kill = () => {
    if (child.exitCode === null) {
      child.exitCode = 0;
    }
    return true;
  };
  return child;
};

const createPreflightRuntime = (overrides = {}) => {
  let spawnCalled = false;
  const runtime = {
    fs,
    spawn: () => {
      spawnCalled = true;
      throw new Error('spawn should not be called during preflight validation');
    },
    reserveLoopbackPort: async () => {
      throw new Error('reserveLoopbackPort should not be called during preflight validation');
    },
    fetchWithTimeout: async () => ({ ok: false, status: 500 }),
    terminateChild: async () => {},
    sleep: async () => {},
    now: () => 0,
    mkdtempSync: (prefix) => fs.mkdtempSync(prefix),
    rmSync: (targetPath, options) => fs.rmSync(targetPath, options),
    tmpdir: () => os.tmpdir(),
    ...overrides,
  };
  return { runtime, wasSpawnCalled: () => spawnCalled };
};

test('parseCliOptions returns default values when no args provided', () => {
  const options = parseCliOptions([]);
  assert.equal(options.backendDir, path.resolve('resources/backend'));
  assert.equal(options.webuiDir, path.resolve('resources/webui'));
  assert.equal(options.startupTimeoutMs, 45_000);
  assert.equal(options.pollIntervalMs, 500);
  assert.equal(options.label, '');
  assert.equal(options.showHelp, false);
});

test('parseCliOptions parses all supported flags', () => {
  const options = parseCliOptions([
    '--backend-dir',
    'tmp/backend',
    '--webui-dir',
    'tmp/webui',
    '--startup-timeout-ms',
    '30000',
    '--poll-interval-ms',
    '250',
    '--label',
    'smoke',
  ]);

  assert.equal(options.backendDir, path.resolve('tmp/backend'));
  assert.equal(options.webuiDir, path.resolve('tmp/webui'));
  assert.equal(options.startupTimeoutMs, 30000);
  assert.equal(options.pollIntervalMs, 250);
  assert.equal(options.label, 'smoke');
  assert.equal(options.showHelp, false);
});

test('parseCliOptions marks help flag without exiting', () => {
  const options = parseCliOptions(['--help']);
  assert.equal(options.showHelp, true);
});

test('parseCliOptions marks short help flag without exiting', () => {
  const options = parseCliOptions(['-h']);
  assert.equal(options.showHelp, true);
});

test('parseCliOptions throws when value is missing for value-required flags', () => {
  const flags = [
    '--backend-dir',
    '--webui-dir',
    '--startup-timeout-ms',
    '--poll-interval-ms',
    '--label',
  ];

  for (const flag of flags) {
    assert.throws(
      () => parseCliOptions([flag]),
      (error) =>
        error instanceof Error &&
        error.message.includes(`Missing value for ${flag}.`) &&
        error.message.includes('Usage: node scripts/ci/backend-smoke-test.mjs [options]'),
    );
  }
});

test('parseCliOptions throws when path flags receive empty values', () => {
  const flags = ['--backend-dir', '--webui-dir'];
  for (const flag of flags) {
    assert.throws(
      () => parseCliOptions([flag, '   ']),
      (error) =>
        error instanceof Error &&
        error.message.includes(`Empty value for ${flag}.`) &&
        error.message.includes('Usage: node scripts/ci/backend-smoke-test.mjs [options]'),
    );
  }
});

test('parseCliOptions throws for invalid numeric values', () => {
  const invalidValues = ['abc', '0', '-1'];
  const numericFlags = ['--startup-timeout-ms', '--poll-interval-ms'];

  for (const flag of numericFlags) {
    for (const rawValue of invalidValues) {
      assert.throws(
        () => parseCliOptions([flag, rawValue]),
        (error) =>
          error instanceof Error &&
          error.message.includes(`Invalid numeric value for ${flag}: ${rawValue}`) &&
          error.message.includes('Usage: node scripts/ci/backend-smoke-test.mjs [options]'),
      );
    }
  }
});

test('parseCliOptions throws for unsupported arguments', () => {
  assert.throws(
    () => parseCliOptions(['--unsupported-flag']),
    (error) =>
      error instanceof Error &&
      error.message.includes('Unsupported argument: --unsupported-flag') &&
      error.message.includes('Usage: node scripts/ci/backend-smoke-test.mjs [options]'),
  );
});

test('usageMessage contains key flags', () => {
  const message = usageMessage();
  assert.match(message, /--backend-dir <path>/);
  assert.match(message, /--webui-dir <path>/);
  assert.match(message, /--startup-timeout-ms <ms>/);
  assert.match(message, /--poll-interval-ms <ms>/);
  assert.match(message, /--label <name>/);
});

test('runCli returns 0 on successful execution with no failure logs', async () => {
  const logs = [];
  const errorLogs = [];
  const exitCode = await runCli(['--label', 'ci-test'], {
    executeMain: async () => {},
    log: (line) => logs.push(String(line)),
    logError: (line) => errorLogs.push(String(line)),
  });

  assert.equal(exitCode, 0);
  assert.equal(errorLogs.length, 0);
  assert.equal(logs.length, 0);
});

test('runCli returns 1 and emits labeled failure message when main throws', async () => {
  const errorLogs = [];
  const exitCode = await runCli(['--label', 'ci-test'], {
    executeMain: async () => {
      throw new Error('boom');
    },
    logError: (line) => errorLogs.push(String(line)),
  });

  assert.equal(exitCode, 1);
  assert.equal(errorLogs.length, 1);
  assert.match(errorLogs[0], /^\[backend-smoke:ci-test\] FAILED: boom/);
});

test('runCli does not double-prefix errors that are already trace-prefixed', async () => {
  const errorLogs = [];
  const exitCode = await runCli(['--label', 'ci-test'], {
    executeMain: async () => {
      throw new Error('[backend-smoke:ci-test] some failure');
    },
    logError: (line) => errorLogs.push(String(line)),
  });

  assert.equal(exitCode, 1);
  assert.equal(errorLogs.length, 1);
  assert.equal(errorLogs[0], '[backend-smoke:ci-test] some failure');
});

test('runCli retries once on EADDRINUSE and then succeeds', async () => {
  const logs = [];
  const errorLogs = [];
  let calls = 0;

  const exitCode = await runCli(['--label', 'ci-test'], {
    executeMain: async () => {
      calls += 1;
      if (calls === 1) {
        throw new Error('listen EADDRINUSE: address already in use');
      }
    },
    log: (line) => logs.push(String(line)),
    logError: (line) => errorLogs.push(String(line)),
    addrInUseRetries: 1,
  });

  assert.equal(exitCode, 0);
  assert.equal(calls, 2);
  assert.equal(errorLogs.length, 0);
  assert.equal(logs.length, 1);
  assert.match(logs[0], /\[backend-smoke:ci-test\] detected EADDRINUSE, retrying startup/);
});

test('runCli returns 1 and emits parse errors with default prefix', async () => {
  const errorLogs = [];
  const exitCode = await runCli(['--startup-timeout-ms', '0'], {
    logError: (line) => errorLogs.push(String(line)),
  });

  assert.equal(exitCode, 1);
  assert.equal(errorLogs.length, 1);
  assert.match(errorLogs[0], /^\[backend-smoke\] FAILED: Invalid numeric value for --startup-timeout-ms: 0/);
  assert.match(errorLogs[0], /Usage: node scripts\/ci\/backend-smoke-test\.mjs \[options\]/);
});

test('runCli prints usage and returns 0 for --help', async () => {
  const logs = [];
  const errorLogs = [];
  const exitCode = await runCli(['--help'], {
    executeMain: async () => {
      throw new Error('main should not be called in help mode');
    },
    log: (line) => logs.push(String(line)),
    logError: (line) => errorLogs.push(String(line)),
  });

  assert.equal(exitCode, 0);
  assert.equal(errorLogs.length, 0);
  assert.equal(logs.length, 1);
  assert.match(logs[0], /Usage: node scripts\/ci\/backend-smoke-test\.mjs \[options\]/);
});

test('main fails when backend directory is missing', async () => {
  const fixture = await createFixtureLayout();
  try {
    const missingBackendDir = path.join(fixture.root, 'missing-backend');
    const { runtime, wasSpawnCalled } = createPreflightRuntime();

    await assert.rejects(
      () =>
        main({
          backendDir: missingBackendDir,
          webuiDir: fixture.webuiDir,
          startupTimeoutMs: 50,
          pollIntervalMs: 1,
          label: 'missing-backend',
        }, runtime),
      (error) =>
        error instanceof Error &&
        error.message.includes('Backend directory not found'),
    );

    assert.equal(wasSpawnCalled(), false);
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('main fails when webui directory is missing', async () => {
  const fixture = await createFixtureLayout();
  try {
    const missingWebuiDir = path.join(fixture.root, 'missing-webui');
    const { runtime, wasSpawnCalled } = createPreflightRuntime();

    await assert.rejects(
      () =>
        main(
          {
            backendDir: fixture.backendDir,
            webuiDir: missingWebuiDir,
            startupTimeoutMs: 50,
            pollIntervalMs: 1,
            label: 'missing-webui',
          },
          runtime,
        ),
      (error) =>
        error instanceof Error &&
        error.message.includes('WebUI directory not found'),
    );

    assert.equal(wasSpawnCalled(), false);
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('main fails when runtime-manifest.json is missing', async () => {
  const fixture = await createFixtureLayout();
  try {
    await rm(path.join(fixture.backendDir, 'runtime-manifest.json'), { force: true });
    const { runtime, wasSpawnCalled } = createPreflightRuntime();

    await assert.rejects(
      () =>
        main(
          {
            backendDir: fixture.backendDir,
            webuiDir: fixture.webuiDir,
            startupTimeoutMs: 50,
            pollIntervalMs: 1,
            label: 'missing-manifest',
          },
          runtime,
        ),
      (error) =>
        error instanceof Error &&
        error.message.includes('Backend runtime manifest not found'),
    );

    assert.equal(wasSpawnCalled(), false);
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('main fails when launch_backend.py is missing', async () => {
  const fixture = await createFixtureLayout();
  try {
    await rm(path.join(fixture.backendDir, 'launch_backend.py'), { force: true });
    const { runtime, wasSpawnCalled } = createPreflightRuntime();

    await assert.rejects(
      () =>
        main(
          {
            backendDir: fixture.backendDir,
            webuiDir: fixture.webuiDir,
            startupTimeoutMs: 50,
            pollIntervalMs: 1,
            label: 'missing-launcher',
          },
          runtime,
        ),
      (error) =>
        error instanceof Error &&
        error.message.includes('Backend launcher not found'),
    );

    assert.equal(wasSpawnCalled(), false);
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('main fails when app/main.py is missing', async () => {
  const fixture = await createFixtureLayout();
  try {
    await rm(path.join(fixture.backendDir, 'app', 'main.py'), { force: true });
    const { runtime, wasSpawnCalled } = createPreflightRuntime();

    await assert.rejects(
      () =>
        main(
          {
            backendDir: fixture.backendDir,
            webuiDir: fixture.webuiDir,
            startupTimeoutMs: 50,
            pollIntervalMs: 1,
            label: 'missing-app-main',
          },
          runtime,
        ),
      (error) =>
        error instanceof Error &&
        error.message.includes('Backend app main.py not found'),
    );

    assert.equal(wasSpawnCalled(), false);
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('main fails when manifest.python is invalid', async () => {
  const fixture = await createFixtureLayout();
  try {
    const runtimeManifestPath = path.join(fixture.backendDir, 'runtime-manifest.json');
    const manifest = JSON.parse(fs.readFileSync(runtimeManifestPath, 'utf8'));
    manifest.python = 42;
    fs.writeFileSync(runtimeManifestPath, JSON.stringify(manifest), 'utf8');
    const { runtime, wasSpawnCalled } = createPreflightRuntime();

    await assert.rejects(
      () =>
        main(
          {
            backendDir: fixture.backendDir,
            webuiDir: fixture.webuiDir,
            startupTimeoutMs: 50,
            pollIntervalMs: 1,
            label: 'invalid-manifest-python',
          },
          runtime,
        ),
      (error) =>
        error instanceof Error &&
        error.message.includes('Invalid runtime manifest python entry'),
    );

    assert.equal(wasSpawnCalled(), false);
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('main fails when manifest.python points to a non-existent executable', async () => {
  const fixture = await createFixtureLayout();
  try {
    const runtimeManifestPath = path.join(fixture.backendDir, 'runtime-manifest.json');
    const manifest = JSON.parse(fs.readFileSync(runtimeManifestPath, 'utf8'));
    manifest.python = 'python/not-found';
    fs.writeFileSync(runtimeManifestPath, JSON.stringify(manifest), 'utf8');
    const { runtime, wasSpawnCalled } = createPreflightRuntime();

    await assert.rejects(
      () =>
        main(
          {
            backendDir: fixture.backendDir,
            webuiDir: fixture.webuiDir,
            startupTimeoutMs: 50,
            pollIntervalMs: 1,
            label: 'missing-runtime-python',
          },
          runtime,
        ),
      (error) =>
        error instanceof Error &&
        error.message.includes('Runtime python executable not found'),
    );

    assert.equal(wasSpawnCalled(), false);
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('main succeeds on readiness and always runs terminate/cleanup', async () => {
  const fixture = await createFixtureLayout();
  try {
    const child = createFakeChild();
    let terminated = 0;
    let cleanedUpPath = '';

    await main(
      {
        backendDir: fixture.backendDir,
        webuiDir: fixture.webuiDir,
        startupTimeoutMs: 50,
        pollIntervalMs: 1,
        label: 'main-ok',
      },
      {
        fs,
        spawn: () => child,
        reserveLoopbackPort: async () => 6190,
        fetchWithTimeout: async () => ({ ok: true, status: 200 }),
        terminateChild: async (actualChild) => {
          assert.equal(actualChild, child);
          terminated += 1;
        },
        sleep: async () => {},
        now: () => 0,
        mkdtempSync: (prefix) => fs.mkdtempSync(prefix),
        rmSync: (targetPath, options) => {
          cleanedUpPath = targetPath;
          fs.rmSync(targetPath, options);
        },
        tmpdir: () => os.tmpdir(),
      },
    );

    assert.equal(terminated, 1);
    assert.ok(cleanedUpPath.length > 0);
    assert.equal(fs.existsSync(cleanedUpPath), false);
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('main fails when backend exits before readiness and includes prefixed logs', async () => {
  const fixture = await createFixtureLayout();
  try {
    const child = createFakeChild();
    let now = 0;
    const emitOnce = { done: false };
    let terminated = 0;
    let cleanedUpPath = '';

    await assert.rejects(
      () =>
        main(
          {
            backendDir: fixture.backendDir,
            webuiDir: fixture.webuiDir,
            startupTimeoutMs: 20,
            pollIntervalMs: 1,
            label: 'exit-early',
          },
          {
            fs,
            spawn: () => child,
            reserveLoopbackPort: async () => 6191,
            fetchWithTimeout: async () => {
              throw new Error('connect refused');
            },
            terminateChild: async () => {
              terminated += 1;
            },
            sleep: async () => {
              if (!emitOnce.done) {
                emitOnce.done = true;
                child.stderr.emit('data', 'boot log line');
                child.exitCode = 1;
              }
              now += 1;
            },
            now: () => now,
            mkdtempSync: (prefix) => fs.mkdtempSync(prefix),
            rmSync: (targetPath, options) => {
              cleanedUpPath = targetPath;
              fs.rmSync(targetPath, options);
            },
            tmpdir: () => os.tmpdir(),
          },
        ),
      (error) =>
        error instanceof Error &&
        error.message.includes('[backend-smoke:exit-early] Backend exited before readiness') &&
        error.message.includes('[backend-smoke:exit-early] recent backend logs:') &&
        error.message.includes('stderr: boot log line'),
    );

    assert.equal(terminated, 1);
    assert.ok(cleanedUpPath.length > 0);
    assert.equal(fs.existsSync(cleanedUpPath), false);
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('main fails on readiness timeout and keeps bounded recent logs', async () => {
  const fixture = await createFixtureLayout();
  try {
    const child = createFakeChild();
    let now = 0;

    await assert.rejects(
      () =>
        main(
          {
            backendDir: fixture.backendDir,
            webuiDir: fixture.webuiDir,
            startupTimeoutMs: 260,
            pollIntervalMs: 1,
            label: 'timeout',
          },
          {
            fs,
            spawn: () => child,
            reserveLoopbackPort: async () => 6192,
            fetchWithTimeout: async () => {
              throw new Error('still not reachable');
            },
            terminateChild: async () => {},
            sleep: async () => {
              child.stdout.emit('data', `line-${now}`);
              now += 1;
            },
            now: () => now,
            mkdtempSync: (prefix) => fs.mkdtempSync(prefix),
            rmSync: (targetPath, options) => fs.rmSync(targetPath, options),
            tmpdir: () => os.tmpdir(),
          },
        ),
      (error) =>
        error instanceof Error &&
        error.message.includes('[backend-smoke:timeout] Backend did not become HTTP-reachable') &&
        error.message.includes('stdout: line-250') &&
        !error.message.includes('stdout: line-0'),
    );
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('main fails on readiness timeout with HTTP status and reports last probe error', async () => {
  const fixture = await createFixtureLayout();
  try {
    const child = createFakeChild();
    let now = 0;

    await assert.rejects(
      () =>
        main(
          {
            backendDir: fixture.backendDir,
            webuiDir: fixture.webuiDir,
            startupTimeoutMs: 260,
            pollIntervalMs: 1,
            label: 'http-timeout',
          },
          {
            fs,
            spawn: () => child,
            reserveLoopbackPort: async () => 6292,
            fetchWithTimeout: async () => ({ ok: false, status: 503 }),
            terminateChild: async () => {},
            sleep: async () => {
              child.stdout.emit('data', `line-${now}`);
              now += 1;
            },
            now: () => now,
            mkdtempSync: (prefix) => fs.mkdtempSync(prefix),
            rmSync: (targetPath, options) => fs.rmSync(targetPath, options),
            tmpdir: () => os.tmpdir(),
          },
        ),
      (error) =>
        error instanceof Error &&
        error.message.includes('[backend-smoke:http-timeout] Backend did not become HTTP-reachable') &&
        error.message.includes('HTTP 503'),
    );
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('main fails when backend crashes after readiness', async () => {
  const fixture = await createFixtureLayout();
  try {
    const child = createFakeChild();
    let sleepCalls = 0;

    await assert.rejects(
      () =>
        main(
          {
            backendDir: fixture.backendDir,
            webuiDir: fixture.webuiDir,
            startupTimeoutMs: 50,
            pollIntervalMs: 1,
            label: 'crash-after-ready',
          },
          {
            fs,
            spawn: () => child,
            reserveLoopbackPort: async () => 6193,
            fetchWithTimeout: async () => ({ ok: true, status: 200 }),
            terminateChild: async () => {},
            sleep: async () => {
              sleepCalls += 1;
              if (sleepCalls === 1) {
                child.exitCode = 2;
              }
            },
            now: () => 0,
            mkdtempSync: (prefix) => fs.mkdtempSync(prefix),
            rmSync: (targetPath, options) => fs.rmSync(targetPath, options),
            tmpdir: () => os.tmpdir(),
          },
        ),
      (error) =>
        error instanceof Error &&
        error.message.includes('[backend-smoke:crash-after-ready] Backend crashed after readiness (exit=2).'),
    );
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});

test('main fails when spawn emits error', async () => {
  const fixture = await createFixtureLayout();
  try {
    const child = createFakeChild();
    let now = 0;

    await assert.rejects(
      () =>
        main(
          {
            backendDir: fixture.backendDir,
            webuiDir: fixture.webuiDir,
            startupTimeoutMs: 50,
            pollIntervalMs: 1,
            label: 'spawn-error',
          },
          {
            fs,
            spawn: () => {
              queueMicrotask(() => {
                child.emit('error', new Error('spawn failed'));
              });
              return child;
            },
            reserveLoopbackPort: async () => 6194,
            fetchWithTimeout: async () => {
              throw new Error('connection refused');
            },
            terminateChild: async () => {},
            sleep: async () => {
              now += 1;
            },
            now: () => now,
            mkdtempSync: (prefix) => fs.mkdtempSync(prefix),
            rmSync: (targetPath, options) => fs.rmSync(targetPath, options),
            tmpdir: () => os.tmpdir(),
          },
        ),
      (error) =>
        error instanceof Error &&
        error.message.includes('[backend-smoke:spawn-error] Failed to spawn backend process: spawn failed') &&
        error.message.includes('[backend-smoke:spawn-error] recent backend logs:') &&
        error.message.includes('spawn-error: spawn failed'),
    );
  } finally {
    await rm(fixture.root, { recursive: true, force: true });
  }
});
