import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import net from 'node:net';
import { spawn } from 'node:child_process';
import { setTimeout as sleep } from 'node:timers/promises';

const args = process.argv.slice(2);
const defaultBackendDir = path.resolve('resources', 'backend');
const defaultWebuiDir = path.resolve('resources', 'webui');

const usageMessage = () => `
Usage: node scripts/ci/backend-smoke-test.mjs [options]

Options:
  --backend-dir <path>         Backend resources directory (default: resources/backend)
  --webui-dir <path>           WebUI resources directory (default: resources/webui)
  --startup-timeout-ms <ms>    Startup timeout in milliseconds (default: 45000)
  --poll-interval-ms <ms>      Readiness poll interval in milliseconds (default: 500)
  --label <name>               Optional log label
  -h, --help                   Show this message
`.trim();

const parseCliOptions = (argv) => {
  const parsed = {
    backendDir: defaultBackendDir,
    webuiDir: defaultWebuiDir,
    startupTimeoutMs: 45_000,
    pollIntervalMs: 500,
    label: '',
  };

  const requireValue = (flag, index) => {
    const next = argv[index + 1];
    if (next === undefined || next.startsWith('--')) {
      throw new Error(`Missing value for ${flag}.\n\n${usageMessage()}`);
    }
    return next;
  };

  const parsePositiveNumber = (flag, rawValue) => {
    const value = Number(rawValue);
    if (!Number.isFinite(value) || value <= 0) {
      throw new Error(`Invalid numeric value for ${flag}: ${rawValue}\n\n${usageMessage()}`);
    }
    return value;
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '-h' || arg === '--help') {
      console.log(usageMessage());
      process.exit(0);
    } else if (arg === '--backend-dir') {
      const raw = requireValue(arg, i).trim();
      if (!raw) {
        throw new Error(`Empty value for ${arg}.\n\n${usageMessage()}`);
      }
      parsed.backendDir = path.resolve(raw);
      i += 1;
    } else if (arg === '--webui-dir') {
      const raw = requireValue(arg, i).trim();
      if (!raw) {
        throw new Error(`Empty value for ${arg}.\n\n${usageMessage()}`);
      }
      parsed.webuiDir = path.resolve(raw);
      i += 1;
    } else if (arg === '--startup-timeout-ms') {
      const raw = requireValue(arg, i);
      parsed.startupTimeoutMs = parsePositiveNumber(arg, raw);
      i += 1;
    } else if (arg === '--poll-interval-ms') {
      const raw = requireValue(arg, i);
      parsed.pollIntervalMs = parsePositiveNumber(arg, raw);
      i += 1;
    } else if (arg === '--label') {
      parsed.label = requireValue(arg, i);
      i += 1;
    } else {
      throw new Error(`Unsupported argument: ${arg}\n\n${usageMessage()}`);
    }
  }

  return parsed;
};

const options = parseCliOptions(args);

const tracePrefix = options.label ? `[backend-smoke:${options.label}]` : '[backend-smoke]';

const assertPathExists = (targetPath, description) => {
  if (!fs.existsSync(targetPath)) {
    throw new Error(`${description} not found: ${targetPath}`);
  }
};

const reserveLoopbackPort = async () =>
  new Promise((resolve, reject) => {
    const server = net.createServer();
    server.unref();
    server.on('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const address = server.address();
      if (!address || typeof address !== 'object') {
        server.close(() => reject(new Error('Failed to reserve loopback port.')));
        return;
      }
      const { port } = address;
      server.close((error) => {
        if (error) {
          reject(error);
          return;
        }
        resolve(port);
      });
    });
  });

const fetchWithTimeout = async (url, timeoutMs) => {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetch(url, { method: 'GET', signal: controller.signal });
  } finally {
    clearTimeout(timer);
  }
};

const terminateChild = async (child, timeoutMs = 4_000) => {
  if (!child || child.exitCode !== null) {
    return;
  }
  child.kill();
  const start = Date.now();
  while (child.exitCode === null && Date.now() - start < timeoutMs) {
    await sleep(100);
  }
  if (child.exitCode === null) {
    if (process.platform === 'win32') {
      child.kill();
    } else {
      child.kill('SIGKILL');
    }
  }
};

const main = async () => {
  const backendDir = options.backendDir;
  const webuiDir = options.webuiDir;
  const manifestPath = path.join(backendDir, 'runtime-manifest.json');
  const launcherPath = path.join(backendDir, 'launch_backend.py');
  const appMainPath = path.join(backendDir, 'app', 'main.py');

  assertPathExists(backendDir, 'Backend directory');
  assertPathExists(webuiDir, 'WebUI directory');
  assertPathExists(manifestPath, 'Backend runtime manifest');
  assertPathExists(launcherPath, 'Backend launcher');
  assertPathExists(appMainPath, 'Backend app main.py');

  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  if (!manifest.python || typeof manifest.python !== 'string') {
    throw new Error(`Invalid runtime manifest python entry: ${manifestPath}`);
  }
  const pythonPath = path.join(backendDir, manifest.python);
  assertPathExists(pythonPath, 'Runtime python executable');

  const dashboardPort = await reserveLoopbackPort();
  const backendRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'astrbot-backend-smoke-'));
  const backendUrl = `http://127.0.0.1:${dashboardPort}/`;
  const childLogs = [];
  const maxLogLines = 200;
  const appendLog = (kind, chunk) => {
    const lines = String(chunk)
      .split(/\r?\n/)
      .map((line) => line.trimEnd())
      .filter(Boolean);
    for (const line of lines) {
      childLogs.push(`${kind}: ${line}`);
      if (childLogs.length > maxLogLines) {
        childLogs.shift();
      }
    }
  };
  let spawnError = null;

  const child = spawn(
    pythonPath,
    [launcherPath, '--webui-dir', webuiDir],
    {
      cwd: backendRoot,
      env: {
        ...process.env,
        ASTRBOT_ROOT: backendRoot,
        ASTRBOT_DESKTOP_CLIENT: '1',
        ASTRBOT_WEBUI_DIR: webuiDir,
        DASHBOARD_HOST: '127.0.0.1',
        DASHBOARD_PORT: String(dashboardPort),
        PYTHONUNBUFFERED: '1',
        PYTHONUTF8: process.env.PYTHONUTF8 || '1',
        PYTHONIOENCODING: process.env.PYTHONIOENCODING || 'utf-8',
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    },
  );

  child.stdout?.on('data', (chunk) => appendLog('stdout', chunk));
  child.stderr?.on('data', (chunk) => appendLog('stderr', chunk));
  child.on('error', (error) => {
    const message = error instanceof Error ? error.message : String(error);
    spawnError = error instanceof Error ? error : new Error(message);
    appendLog('spawn-error', message);
  });

  console.log(
    `${tracePrefix} started backend pid=${child.pid} url=${backendUrl} root=${backendRoot}`,
  );

  const deadline = Date.now() + options.startupTimeoutMs;
  let ready = false;
  let lastProbeError = '';

  try {
    while (Date.now() < deadline) {
      if (spawnError) {
        throw new Error(`Failed to spawn backend process: ${spawnError.message}`);
      }
      if (child.exitCode !== null) {
        throw new Error(
          `Backend exited before readiness check passed (exit=${child.exitCode}).`,
        );
      }

      try {
        const response = await fetchWithTimeout(backendUrl, 1_200);
        if (response.ok) {
          ready = true;
          break;
        }
        lastProbeError = `HTTP ${response.status}`;
      } catch (error) {
        lastProbeError = error instanceof Error ? error.message : String(error);
      }
      await sleep(options.pollIntervalMs);
    }

    if (!ready) {
      throw new Error(
        `Backend did not become HTTP-reachable within ${options.startupTimeoutMs}ms (${lastProbeError || 'no response'}).`,
      );
    }

    // Keep the process alive for a short extra window to catch immediate crash loops.
    await sleep(1_200);
    if (child.exitCode !== null) {
      throw new Error(`Backend crashed after readiness (exit=${child.exitCode}).`);
    }
    console.log(`${tracePrefix} backend startup smoke test passed.`);
  } catch (error) {
    const details = childLogs.length
      ? `\n${tracePrefix} recent backend logs:\n${childLogs.join('\n')}`
      : '';
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`${reason}${details}`);
  } finally {
    await terminateChild(child);
    fs.rmSync(backendRoot, { recursive: true, force: true });
  }
};

main().catch((error) => {
  console.error(`${tracePrefix} FAILED: ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
});
