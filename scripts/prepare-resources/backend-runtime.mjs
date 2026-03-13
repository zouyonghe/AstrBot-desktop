import { existsSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import {
  BUNDLED_RUNTIME_ARCH_ENV,
  resolveBundledRuntimeArch,
} from '../backend/runtime-arch-utils.mjs';

export const resolvePbsTarget = ({
  platform = process.platform,
  arch = process.arch,
  env = process.env,
} = {}) => {
  const platformMap = {
    linux: 'linux',
    darwin: 'mac',
    win32: 'windows',
  };

  const normalizedPlatform = platformMap[platform];
  const normalizedArch = resolveBundledRuntimeArch({ platform, arch, env });
  if (!normalizedPlatform || !normalizedArch) {
    throw new Error(
      `Unsupported platform/arch for python-build-standalone: ${platform}/${arch}`,
    );
  }

  const targetMap = {
    'linux/amd64': 'x86_64-unknown-linux-gnu',
    'linux/arm64': 'aarch64-unknown-linux-gnu',
    'mac/amd64': 'x86_64-apple-darwin',
    'mac/arm64': 'aarch64-apple-darwin',
    'windows/amd64': 'x86_64-pc-windows-msvc',
    'windows/arm64': 'aarch64-pc-windows-msvc',
  };

  const key = `${normalizedPlatform}/${normalizedArch}`;
  const target = targetMap[key];
  if (!target) {
    throw new Error(`Unsupported python-build-standalone mapping: ${key}`);
  }

  return target;
};

const resolveRuntimePythonPath = (runtimeRoot) => {
  const candidates =
    process.platform === 'win32'
      ? [path.join(runtimeRoot, 'python.exe'), path.join(runtimeRoot, 'Scripts', 'python.exe')]
      : [path.join(runtimeRoot, 'bin', 'python3'), path.join(runtimeRoot, 'bin', 'python')];
  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      return candidate;
    }
  }
  return null;
};

export const ensureBundledRuntime = ({
  projectRoot,
  pythonBuildStandaloneRelease,
  pythonBuildStandaloneVersion,
}) => {
  const externalRuntime =
    process.env.ASTRBOT_DESKTOP_BACKEND_RUNTIME || process.env.ASTRBOT_DESKTOP_CPYTHON_HOME;
  if (externalRuntime && existsSync(externalRuntime)) {
    return externalRuntime;
  }

  const runtimeArch = resolveBundledRuntimeArch();
  process.env[BUNDLED_RUNTIME_ARCH_ENV] = runtimeArch;
  const pbsTarget = resolvePbsTarget();
  const runtimeBase = path.join(
    projectRoot,
    'runtime',
    `${pbsTarget}-${pythonBuildStandaloneVersion}`,
  );
  const runtimeRoot = path.join(runtimeBase, 'astrbot-cpython-runtime');
  const runtimePython = resolveRuntimePythonPath(runtimeRoot);
  if (runtimePython) {
    process.env.ASTRBOT_DESKTOP_CPYTHON_HOME = runtimeRoot;
    return runtimeRoot;
  }

  mkdirSync(runtimeBase, { recursive: true });
  const resolverScript = path.join(
    projectRoot,
    'scripts',
    'cpython',
    'resolve_packaged_cpython_runtime.py',
  );

  const pythonCandidates = process.platform === 'win32' ? ['python', 'py'] : ['python3', 'python'];
  let lastErr = null;
  for (const cmd of pythonCandidates) {
    const args = cmd === 'py' ? ['-3', resolverScript] : [resolverScript];
    const result = spawnSync(cmd, args, {
      cwd: projectRoot,
      stdio: 'inherit',
      env: {
        ...process.env,
        RUNNER_TEMP_DIR: runtimeBase,
        PYTHON_BUILD_STANDALONE_RELEASE: pythonBuildStandaloneRelease,
        PYTHON_BUILD_STANDALONE_VERSION: pythonBuildStandaloneVersion,
        PYTHON_BUILD_STANDALONE_TARGET: pbsTarget,
      },
    });

    if (result.error && result.error.code === 'ENOENT') {
      lastErr = result.error;
      continue;
    }
    if (result.status !== 0) {
      throw new Error(`Failed to prepare CPython runtime via ${cmd}.`);
    }

    process.env.ASTRBOT_DESKTOP_CPYTHON_HOME = runtimeRoot;
    return runtimeRoot;
  }

  throw new Error(
    `Cannot find Python interpreter to resolve CPython runtime (${String(lastErr || '')}).`,
  );
};
