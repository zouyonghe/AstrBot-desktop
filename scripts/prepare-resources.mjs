import { cp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { existsSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const DEFAULT_ASTRBOT_SOURCE_GIT_URL = 'https://github.com/AstrBotDevs/AstrBot.git';
const sourceRepoUrlRaw =
  process.env.ASTRBOT_SOURCE_GIT_URL?.trim() || DEFAULT_ASTRBOT_SOURCE_GIT_URL;
const sourceRepoRefRaw = process.env.ASTRBOT_SOURCE_GIT_REF?.trim() || '';
const desktopVersionOverride = process.env.ASTRBOT_DESKTOP_VERSION?.trim() || '';
const PYTHON_BUILD_STANDALONE_RELEASE =
  process.env.ASTRBOT_PBS_RELEASE?.trim() || '20260211';
const PYTHON_BUILD_STANDALONE_VERSION =
  process.env.ASTRBOT_PBS_VERSION?.trim() || '3.12.12';
const mode = process.argv[2] || 'all';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '..');

const normalizeSourceRepoConfig = () => {
  if (!sourceRepoUrlRaw) {
    return { repoUrl: '', repoRef: sourceRepoRefRaw };
  }

  const treeMatch =
    /^https?:\/\/github\.com\/([^/]+\/[^/]+)\/tree\/([^/]+)\/?$/.exec(sourceRepoUrlRaw) ||
    /^https?:\/\/github\.com\/([^/]+\/[^/]+)\/tree\/([^/]+)\/.+$/.exec(sourceRepoUrlRaw);
  if (treeMatch) {
    const repoPath = treeMatch[1];
    const refFromUrl = treeMatch[2];
    return {
      repoUrl: `https://github.com/${repoPath}.git`,
      repoRef: sourceRepoRefRaw || refFromUrl,
    };
  }

  return {
    repoUrl: sourceRepoUrlRaw,
    repoRef: sourceRepoRefRaw,
  };
};

const { repoUrl: sourceRepoUrl, repoRef: sourceRepoRef } = normalizeSourceRepoConfig();

const resolveSourceDir = () => {
  const fromEnv = process.env.ASTRBOT_SOURCE_DIR?.trim();
  if (fromEnv) {
    return path.resolve(process.cwd(), fromEnv);
  }
  return path.join(projectRoot, 'vendor', 'AstrBot');
};

const ensureSourceRepo = (sourceDir) => {
  if (process.env.ASTRBOT_SOURCE_DIR?.trim()) {
    if (!existsSync(path.join(sourceDir, 'main.py'))) {
      throw new Error(
        `ASTRBOT_SOURCE_DIR is set but invalid: ${sourceDir}. Cannot find main.py.`,
      );
    }
    return;
  }

  if (!existsSync(path.join(sourceDir, '.git'))) {
    mkdirSync(path.dirname(sourceDir), { recursive: true });
    const cloneArgs = ['clone', '--depth', '1'];
    if (sourceRepoRef) {
      cloneArgs.push('--branch', sourceRepoRef);
    }
    cloneArgs.push(sourceRepoUrl, sourceDir);

    const cloneResult = spawnSync('git', cloneArgs, {
      stdio: 'inherit',
    });
    if (cloneResult.status !== 0) {
      throw new Error(`Failed to clone AstrBot from ${sourceRepoUrl}`);
    }
  } else {
    const setUrlResult = spawnSync(
      'git',
      ['-C', sourceDir, 'remote', 'set-url', 'origin', sourceRepoUrl],
      { stdio: 'inherit' },
    );
    if (setUrlResult.status !== 0) {
      throw new Error(`Failed to set origin url for ${sourceDir}`);
    }
  }

  if (sourceRepoRef) {
    const fetchResult = spawnSync(
      'git',
      ['-C', sourceDir, 'fetch', '--depth', '1', 'origin', sourceRepoRef],
      { stdio: 'inherit' },
    );
    if (fetchResult.status !== 0) {
      throw new Error(`Failed to fetch upstream ref ${sourceRepoRef}`);
    }

    const checkoutResult = spawnSync(
      'git',
      ['-C', sourceDir, 'checkout', '-B', sourceRepoRef, 'FETCH_HEAD'],
      { stdio: 'inherit' },
    );
    if (checkoutResult.status !== 0) {
      throw new Error(`Failed to checkout upstream ref ${sourceRepoRef}`);
    }
  }

  if (!existsSync(path.join(sourceDir, 'main.py'))) {
    throw new Error(
      `Resolved source repository is invalid: ${sourceDir}. Cannot find main.py.`,
    );
  }
};

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

const patchMonacoCssNestingWarnings = async (dashboardDir) => {
  const patchRules = [
    {
      file: path.join(
        dashboardDir,
        'node_modules',
        'monaco-editor',
        'esm',
        'vs',
        'editor',
        'browser',
        'widget',
        'multiDiffEditor',
        'style.css',
      ),
      selector: 'a',
    },
    {
      file: path.join(
        dashboardDir,
        'node_modules',
        'monaco-editor',
        'esm',
        'vs',
        'editor',
        'contrib',
        'inlineEdits',
        'browser',
        'inlineEditsWidget.css',
      ),
      selector: 'svg',
    },
  ];

  for (const { file, selector } of patchRules) {
    if (!existsSync(file)) {
      continue;
    }
    const css = await readFile(file, 'utf8');
    const pattern = new RegExp(`^(\\s*)${selector}\\s*\\{`, 'm');
    if (!pattern.test(css)) {
      continue;
    }

    const patched = css.replace(pattern, `$1& ${selector} {`);
    if (patched !== css) {
      await writeFile(file, patched, 'utf8');
      console.log(
        `[prepare-resources] Patched Monaco nested selector "${selector}" in ${path.relative(projectRoot, file)}`,
      );
    }
  }
};

const readAstrbotVersionFromPyproject = async (sourceDir) => {
  const pyprojectPath = path.join(sourceDir, 'pyproject.toml');
  if (!existsSync(pyprojectPath)) {
    throw new Error(`Cannot find pyproject.toml in source directory: ${sourceDir}`);
  }

  const content = await readFile(pyprojectPath, 'utf8');
  const lines = content.split(/\r?\n/);
  let inProjectSection = false;

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line || line.startsWith('#')) {
      continue;
    }

    if (line.startsWith('[') && line.endsWith(']')) {
      inProjectSection = line === '[project]';
      continue;
    }

    if (!inProjectSection) {
      continue;
    }

    const match = /^version\s*=\s*["']([^"']+)["']/.exec(line);
    if (match) {
      return match[1].trim();
    }
  }

  throw new Error(`Cannot resolve [project].version from ${pyprojectPath}`);
};

const syncDesktopVersionFiles = async (version) => {
  const packageJsonPath = path.join(projectRoot, 'package.json');
  const tauriConfigPath = path.join(projectRoot, 'src-tauri', 'tauri.conf.json');
  const cargoTomlPath = path.join(projectRoot, 'src-tauri', 'Cargo.toml');

  const packageJson = JSON.parse(await readFile(packageJsonPath, 'utf8'));
  if (packageJson.version !== version) {
    packageJson.version = version;
    await writeFile(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`, 'utf8');
  }

  const tauriConfig = JSON.parse(await readFile(tauriConfigPath, 'utf8'));
  if (tauriConfig.version !== version) {
    tauriConfig.version = version;
    await writeFile(tauriConfigPath, `${JSON.stringify(tauriConfig, null, 2)}\n`, 'utf8');
  }

  const cargoToml = await readFile(cargoTomlPath, 'utf8');
  const cargoVersionPattern = /(\[package\][\s\S]*?\nversion\s*=\s*")[^"]+(")/;
  if (!cargoVersionPattern.test(cargoToml)) {
    throw new Error(`Cannot update Cargo package version in ${cargoTomlPath}`);
  }
  const updatedCargoToml = cargoToml.replace(cargoVersionPattern, `$1${version}$2`);
  if (updatedCargoToml !== cargoToml) {
    await writeFile(cargoTomlPath, updatedCargoToml, 'utf8');
  }
};

const resolvePbsTarget = () => {
  const platformMap = {
    linux: 'linux',
    darwin: 'mac',
    win32: 'windows',
  };
  const archMap = {
    x64: 'amd64',
    arm64: 'arm64',
  };

  const normalizedPlatform = platformMap[process.platform];
  const normalizedArch = archMap[process.arch];
  if (!normalizedPlatform || !normalizedArch) {
    throw new Error(
      `Unsupported platform/arch for python-build-standalone: ${process.platform}/${process.arch}`,
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

const ensureBundledRuntime = () => {
  const externalRuntime =
    process.env.ASTRBOT_DESKTOP_BACKEND_RUNTIME || process.env.ASTRBOT_DESKTOP_CPYTHON_HOME;
  if (externalRuntime && existsSync(externalRuntime)) {
    return externalRuntime;
  }

  const pbsTarget = resolvePbsTarget();
  const runtimeBase = path.join(
    projectRoot,
    'runtime',
    `${pbsTarget}-${PYTHON_BUILD_STANDALONE_VERSION}`,
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
        PYTHON_BUILD_STANDALONE_RELEASE,
        PYTHON_BUILD_STANDALONE_VERSION,
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

const prepareWebui = async (sourceDir) => {
  const dashboardDir = path.join(sourceDir, 'dashboard');
  ensurePackageInstall(dashboardDir, 'AstrBot dashboard');
  await patchMonacoCssNestingWarnings(dashboardDir);
  runPnpmChecked(['--dir', dashboardDir, 'build'], sourceDir);

  const sourceWebuiDir = path.join(sourceDir, 'dashboard', 'dist');
  if (!existsSync(path.join(sourceWebuiDir, 'index.html'))) {
    throw new Error(`WebUI build output missing: ${sourceWebuiDir}`);
  }

  const targetWebuiDir = path.join(projectRoot, 'resources', 'webui');
  await syncResourceDir(sourceWebuiDir, targetWebuiDir);
};

const prepareBackend = async (sourceDir) => {
  const runtimeRoot = ensureBundledRuntime();
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

const ensureStartupShellAssets = () => {
  const startupUiDir = path.join(projectRoot, 'ui');
  const requiredFiles = ['index.html', 'astrbot-logo.png'];
  for (const file of requiredFiles) {
    const candidate = path.join(startupUiDir, file);
    if (!existsSync(candidate)) {
      throw new Error(`Startup shell asset missing: ${candidate}`);
    }
  }
};

const main = async () => {
  const sourceDir = resolveSourceDir();
  const needsSourceRepo = mode !== 'version' || !desktopVersionOverride;
  await mkdir(path.join(projectRoot, 'resources'), { recursive: true });
  if (needsSourceRepo) {
    ensureSourceRepo(sourceDir);
  } else {
    console.log(
      '[prepare-resources] Skip source repo sync in version-only mode because ASTRBOT_DESKTOP_VERSION is set.',
    );
  }
  ensureStartupShellAssets();
  const astrbotVersion = desktopVersionOverride || (await readAstrbotVersionFromPyproject(sourceDir));

  if (desktopVersionOverride && needsSourceRepo) {
    const sourceVersion = await readAstrbotVersionFromPyproject(sourceDir);
    if (sourceVersion !== desktopVersionOverride) {
      console.warn(
        `[prepare-resources] Version override drift detected: ASTRBOT_DESKTOP_VERSION=${desktopVersionOverride}, source pyproject version=${sourceVersion} (${sourceDir})`,
      );
    }
  }

  await syncDesktopVersionFiles(astrbotVersion);
  if (desktopVersionOverride) {
    console.log(
      `[prepare-resources] Synced desktop version to override ${astrbotVersion} (ASTRBOT_DESKTOP_VERSION)`,
    );
  } else {
    console.log(`[prepare-resources] Synced desktop version to AstrBot ${astrbotVersion}`);
  }

  if (mode === 'version') {
    return;
  }

  if (mode === 'webui') {
    await prepareWebui(sourceDir);
    return;
  }

  if (mode === 'backend') {
    await prepareBackend(sourceDir);
    return;
  }

  if (mode === 'all') {
    await prepareWebui(sourceDir);
    await prepareBackend(sourceDir);
    return;
  }

  throw new Error(`Unsupported mode: ${mode}. Expected version/webui/backend/all.`);
};

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
