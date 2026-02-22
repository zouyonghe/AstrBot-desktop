import { cp, mkdir, rm } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import {
  patchDesktopReleaseRedirectBehavior,
  patchMonacoCssNestingWarnings,
  verifyDesktopBridgeArtifacts,
} from './desktop-bridge-checks.mjs';
import { ensureBundledRuntime } from './backend-runtime.mjs';

const runChecked = (cmd, args, cwd, envExtra = {}, spawnExtra = {}) => {
  const result = spawnSync(cmd, args, {
    cwd,
    stdio: 'inherit',
    env: { ...process.env, ...envExtra },
    ...spawnExtra,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`Command failed: ${cmd} ${args.join(' ')}`);
  }
};

const runPnpmChecked = (args, cwd, envExtra = {}) => {
  runChecked('pnpm', args, cwd, envExtra, {
    shell: process.platform === 'win32',
  });
};

const ensurePackageInstall = (packageDir, installLabel) => {
  const nodeModulesDir = path.join(packageDir, 'node_modules');
  if (existsSync(nodeModulesDir)) {
    return;
  }

  console.log(`[prepare-resources] Installing dependencies for ${installLabel} ...`);
  const lockfilePath = path.join(packageDir, 'pnpm-lock.yaml');
  const installArgs = ['--dir', packageDir, 'install'];
  if (existsSync(lockfilePath)) {
    installArgs.push('--frozen-lockfile');
  }
  runPnpmChecked(installArgs, packageDir);
};

const syncResourceDir = async (source, target) => {
  await rm(target, { recursive: true, force: true });
  await mkdir(path.dirname(target), { recursive: true });
  await cp(source, target, { recursive: true });
};

const resolveDesktopReleaseBaseUrl = () => {
  const raw = process.env.ASTRBOT_DESKTOP_RELEASE_BASE_URL;
  const trimmed = typeof raw === 'string' ? raw.trim() : '';
  return trimmed || 'https://github.com/AstrBotDevs/AstrBot-desktop/releases';
};

export const prepareWebui = async ({
  sourceDir,
  projectRoot,
  sourceRepoRef,
  isSourceRepoRefVersionTag,
  isDesktopBridgeExpectationStrict,
}) => {
  const dashboardDir = path.join(sourceDir, 'dashboard');
  ensurePackageInstall(dashboardDir, 'AstrBot dashboard');
  await patchMonacoCssNestingWarnings({ dashboardDir, projectRoot });
  await patchDesktopReleaseRedirectBehavior({
    dashboardDir,
    projectRoot,
    strictPatternMatch: isDesktopBridgeExpectationStrict,
  });
  await verifyDesktopBridgeArtifacts({
    dashboardDir,
    projectRoot,
    sourceRepoRef,
    isSourceRepoRefVersionTag,
    isDesktopBridgeExpectationStrict,
  });
  runPnpmChecked(['--dir', dashboardDir, 'build'], sourceDir, {
    VITE_ASTRBOT_RELEASE_BASE_URL: resolveDesktopReleaseBaseUrl(),
  });

  const sourceWebuiDir = path.join(sourceDir, 'dashboard', 'dist');
  if (!existsSync(path.join(sourceWebuiDir, 'index.html'))) {
    throw new Error(`WebUI build output missing: ${sourceWebuiDir}`);
  }

  const targetWebuiDir = path.join(projectRoot, 'resources', 'webui');
  await syncResourceDir(sourceWebuiDir, targetWebuiDir);
};

export const prepareBackend = async ({
  sourceDir,
  projectRoot,
  pythonBuildStandaloneRelease,
  pythonBuildStandaloneVersion,
}) => {
  const runtimeRoot = ensureBundledRuntime({
    projectRoot,
    pythonBuildStandaloneRelease,
    pythonBuildStandaloneVersion,
  });
  runChecked(
    'node',
    [path.join(projectRoot, 'scripts', 'backend', 'build-backend.mjs')],
    projectRoot,
    {
      ASTRBOT_SOURCE_DIR: sourceDir,
      ASTRBOT_DESKTOP_CPYTHON_HOME: runtimeRoot,
    },
  );

  const sourceBackendDir = path.join(projectRoot, 'resources', 'backend');
  if (!existsSync(path.join(sourceBackendDir, 'runtime-manifest.json'))) {
    throw new Error(`Backend runtime output missing: ${sourceBackendDir}`);
  }
};

export const ensureStartupShellAssets = (projectRoot) => {
  const startupUiDir = path.join(projectRoot, 'ui');
  const requiredFiles = ['index.html', 'astrbot-logo.png'];
  for (const file of requiredFiles) {
    const candidate = path.join(startupUiDir, file);
    if (!existsSync(candidate)) {
      throw new Error(`Startup shell asset missing: ${candidate}`);
    }
  }
};
