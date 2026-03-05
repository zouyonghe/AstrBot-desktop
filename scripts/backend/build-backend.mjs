import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import {
  copyTree,
  resolveAndValidateRuntimeSource,
  resolveRuntimePython,
} from './runtime-layout-utils.mjs';
import {
  resolveExpectedRuntimeVersion,
  validateRuntimePython,
} from './runtime-version-utils.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '..', '..');
const sourceDir = process.env.ASTRBOT_SOURCE_DIR
  ? path.resolve(process.env.ASTRBOT_SOURCE_DIR)
  : null;
const outputDir = path.join(projectRoot, 'resources', 'backend');
const appDir = path.join(outputDir, 'app');
const runtimeDir = path.join(outputDir, 'python');
const manifestPath = path.join(outputDir, 'runtime-manifest.json');
const launcherPath = path.join(outputDir, 'launch_backend.py');
const launcherTemplatePath = path.join(__dirname, 'templates', 'launch_backend.py');
const importScannerScriptPath = path.join(__dirname, 'tools', 'scan_imports.py');

const runtimeSource =
  process.env.ASTRBOT_DESKTOP_BACKEND_RUNTIME ||
  process.env.ASTRBOT_DESKTOP_CPYTHON_HOME;
const requirePipProbe = process.env.ASTRBOT_DESKTOP_REQUIRE_PIP === '1';

const requiredSourceEntries = ['astrbot', 'main.py', 'requirements.txt'];
const optionalSourceEntries = ['changelogs'];

const requireSourceDir = () => {
  if (!sourceDir) {
    throw new Error('Missing ASTRBOT_SOURCE_DIR for backend build.');
  }
  if (!fs.existsSync(path.join(sourceDir, 'main.py'))) {
    throw new Error(`Invalid ASTRBOT_SOURCE_DIR: ${sourceDir}. main.py not found.`);
  }
  return sourceDir;
};

const prepareOutputDirs = () => {
  fs.rmSync(outputDir, { recursive: true, force: true });
  fs.mkdirSync(outputDir, { recursive: true });
  fs.mkdirSync(appDir, { recursive: true });
};

const resolveImportScannerPythonExecutable = () => {
  if (process.env.ASTRBOT_DESKTOP_IMPORT_SCANNER_PYTHON) {
    return process.env.ASTRBOT_DESKTOP_IMPORT_SCANNER_PYTHON;
  }
  if (process.env.PYTHON) {
    return process.env.PYTHON;
  }
  return process.platform === 'win32' ? 'python' : 'python3';
};

const IMPORT_SCANNER_TIMEOUT_MS = 30_000;
const importScannerCache = new Map();

const runImportScanner = (filePath) => {
  let cacheKey = '';
  try {
    const stat = fs.statSync(filePath);
    cacheKey = `${filePath}:${stat.mtimeMs}:${stat.size}`;
  } catch {
    // Leave cacheKey empty when file metadata cannot be read.
  }

  if (cacheKey && importScannerCache.has(cacheKey)) {
    return importScannerCache.get(cacheKey);
  }

  const scannerPython = resolveImportScannerPythonExecutable();
  const result = spawnSync(scannerPython, [importScannerScriptPath, filePath], {
    encoding: 'utf8',
    windowsHide: true,
    timeout: IMPORT_SCANNER_TIMEOUT_MS,
  });

  const warnFailure = (details) => {
    console.warn(
      `[build-backend] failed to scan imports for ${path.basename(filePath)}: ${details}`,
    );
    return [];
  };

  if (result.error) {
    if (result.error.code === 'ETIMEDOUT') {
      console.warn(
        `[build-backend] import scanner timed out after ${IMPORT_SCANNER_TIMEOUT_MS}ms for ${path.basename(filePath)}; skipping import analysis for this file.`,
      );
      return [];
    }
    return warnFailure(result.error.message || 'unknown process error');
  }

  if (result.status !== 0) {
    const details = result.stderr?.trim() || `exit code ${result.status}`;
    return warnFailure(details);
  }

  try {
    const parsed = JSON.parse(result.stdout || '[]');
    if (!Array.isArray(parsed)) {
      throw new Error('scanner output is not an array');
    }
    if (cacheKey) {
      importScannerCache.set(cacheKey, parsed);
    }
    return parsed;
  } catch (error) {
    console.warn(
      `[build-backend] invalid import scanner output for ${path.basename(filePath)}: ${error instanceof Error ? error.message : String(error)}`,
    );
    return [];
  }
};

const extractImportedRootModules = (filePath) => {
  const imports = new Set();
  const relativeBareImports = new Set();
  const descriptors = runImportScanner(filePath);

  const addRoot = (name, fromRelativeBareImport = false) => {
    if (typeof name !== 'string') {
      return;
    }
    const rootModule = name.split('.')[0].trim();
    if (!rootModule || rootModule === '*') {
      return;
    }
    if (fromRelativeBareImport) {
      relativeBareImports.add(rootModule);
    }
    imports.add(rootModule);
  };

  for (const descriptor of descriptors) {
    if (!descriptor || typeof descriptor !== 'object') {
      continue;
    }

    if (descriptor.kind === 'import') {
      if (typeof descriptor.module === 'string' && descriptor.module.trim()) {
        addRoot(descriptor.module);
      }
      continue;
    }

    if (descriptor.kind === 'from') {
      const level = Number.isInteger(descriptor.level) ? descriptor.level : 0;
      const moduleSpec = typeof descriptor.module === 'string' ? descriptor.module.trim() : '';
      const names = Array.isArray(descriptor.names) ? descriptor.names : [];

      if (level > 0) {
        if (moduleSpec) {
          addRoot(moduleSpec);
          continue;
        }

        for (const importedName of names) {
          if (typeof importedName !== 'string') {
            continue;
          }
          addRoot(importedName, true);
        }
        continue;
      }

      if (moduleSpec) {
        addRoot(moduleSpec);
      }
    }
  }

  return { imports, relativeBareImports };
};

const listAvailableRootModules = (resolvedSourceDir) => {
  // The desktop bundle currently tracks root-level modules/packages under sourceDir.
  // Nested package-only relative imports are not traversed as independent entries because
  // top-level package directories are copied as whole trees once selected.
  const modules = new Map();
  const entries = fs.readdirSync(resolvedSourceDir, { withFileTypes: true });

  for (const entry of entries) {
    if (entry.isFile() && path.extname(entry.name) === '.py') {
      const moduleName = path.basename(entry.name, '.py');
      const existingModule = modules.get(moduleName);
      if (existingModule?.isPackage) {
        console.warn(
          `[build-backend] both module file and package found for "${moduleName}", ` +
            `preferring ${entry.name}`,
        );
      }
      modules.set(moduleName, {
        relativePath: entry.name,
        scanPath: path.join(resolvedSourceDir, entry.name),
        isPackage: false,
      });
      continue;
    }

    if (!entry.isDirectory()) {
      continue;
    }
    const moduleName = entry.name;
    const initPath = path.join(resolvedSourceDir, moduleName, '__init__.py');
    if (!fs.existsSync(initPath)) {
      continue;
    }
    const existingModule = modules.get(moduleName);
    if (existingModule && !existingModule.isPackage) {
      console.warn(
        `[build-backend] both module file and package found for "${moduleName}", ` +
          `preferring ${existingModule.relativePath}`,
      );
      continue;
    }
    if (existingModule?.isPackage) {
      continue;
    }
    modules.set(moduleName, {
      relativePath: moduleName,
      scanPath: initPath,
      isPackage: true,
    });
  }

  return modules;
};

const logUnresolvedImports = (unresolvedImports, entryFile) => {
  if (unresolvedImports.length === 0) {
    return;
  }

  const decorated = unresolvedImports
    .map(({ file, module }) => `${file} -> ${module}`)
    .sort()
    .join(', ');

  console.warn(
    `[build-backend] unresolved root module imports while scanning ${entryFile}: ` +
      `${decorated} ` +
      '(these may be stdlib/third-party imports; verify local helper modules are present when needed).',
  );
};

const resolveRequiredRootPythonFiles = (resolvedSourceDir, entryFile = 'main.py') => {
  const availableModules = listAvailableRootModules(resolvedSourceDir);
  const required = new Set();
  const visitedFiles = new Set();
  const unresolvedImports = [];
  const queue = [path.join(resolvedSourceDir, entryFile)];

  while (queue.length > 0) {
    const currentFile = queue.shift();
    if (!currentFile || visitedFiles.has(currentFile) || !fs.existsSync(currentFile)) {
      continue;
    }
    visitedFiles.add(currentFile);
    const { imports: importedModules, relativeBareImports } = extractImportedRootModules(currentFile);

    for (const importedModule of importedModules) {
      const moduleEntry = availableModules.get(importedModule);
      if (!moduleEntry) {
        if (relativeBareImports.has(importedModule)) {
          continue;
        }
        unresolvedImports.push({
          file: path.basename(currentFile),
          module: importedModule,
        });
        continue;
      }
      if (moduleEntry.relativePath === entryFile) {
        continue;
      }
      if (!required.has(moduleEntry.relativePath)) {
        required.add(moduleEntry.relativePath);
        queue.push(moduleEntry.scanPath);
      }
    }
  }

  logUnresolvedImports(unresolvedImports, entryFile);
  return Array.from(required).sort();
};

const copyAppSources = (resolvedSourceDir) => {
  const requiredEntries = new Set(requiredSourceEntries);
  for (const relativePath of resolveRequiredRootPythonFiles(resolvedSourceDir, 'main.py')) {
    requiredEntries.add(relativePath);
  }

  for (const relativePath of requiredEntries) {
    const sourcePath = path.join(resolvedSourceDir, relativePath);
    const targetPath = path.join(appDir, relativePath);
    if (!fs.existsSync(sourcePath)) {
      throw new Error(`Backend source path does not exist: ${sourcePath}`);
    }
    copyTree(sourcePath, targetPath);
  }

  // Changelog files are used by dashboard changelog APIs; keep build resilient for older sources.
  for (const relativePath of optionalSourceEntries) {
    const sourcePath = path.join(resolvedSourceDir, relativePath);
    if (!fs.existsSync(sourcePath)) {
      continue;
    }
    const targetPath = path.join(appDir, relativePath);
    copyTree(sourcePath, targetPath);
  }
};

const prepareRuntimeExecutable = (runtimeSourceReal) => {
  copyTree(runtimeSourceReal, runtimeDir, { dereference: true });
  const runtimePython = resolveRuntimePython({ runtimeRoot: runtimeDir, outputDir });
  if (!runtimePython) {
    throw new Error(
      `Cannot find Python executable in runtime: ${runtimeDir}. Expected python under bin/ or Scripts/.`,
    );
  }
  return runtimePython;
};

const writeLauncherScript = () => {
  if (!fs.existsSync(launcherTemplatePath)) {
    throw new Error(`Launcher template does not exist: ${launcherTemplatePath}`);
  }
  const content = fs.readFileSync(launcherTemplatePath, 'utf8');
  fs.writeFileSync(launcherPath, content, 'utf8');
};

const writeRuntimeManifest = (runtimePython) => {
  const manifest = {
    mode: 'cpython-runtime',
    python: runtimePython.relative,
    entrypoint: path.basename(launcherPath),
    app: path.relative(outputDir, appDir),
  };
  fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2), 'utf8');
};

const installRuntimeDependencies = (runtimePython) => {
  const requirementsPath = path.join(appDir, 'requirements.txt');
  if (!fs.existsSync(requirementsPath)) {
    throw new Error(`Backend requirements file does not exist: ${requirementsPath}`);
  }

  const runPipInstall = (pipArgs) => {
    const installArgs = [
      '-m',
      'pip',
      '--disable-pip-version-check',
      'install',
      ...pipArgs,
    ];
    return spawnSync(runtimePython.absolute, installArgs, {
      cwd: outputDir,
      stdio: 'inherit',
      windowsHide: true,
    });
  };

  const isWindowsArm64 = process.platform === 'win32' && process.arch === 'arm64';
  if (isWindowsArm64) {
    // Prefer prebuilt wheels and avoid compiling cryptography from source on Windows ARM64.
    // Fallback versions are configured by env var, for example:
    // ASTRBOT_DESKTOP_CRYPTOGRAPHY_FALLBACK_VERSIONS="43.0.3,42.0.8,41.0.7"
    const fallbackVersionsRaw = (
      process.env.ASTRBOT_DESKTOP_CRYPTOGRAPHY_FALLBACK_VERSIONS || ''
    ).trim();
    const cryptographyFallbackVersions = Array.from(
      new Set(
        fallbackVersionsRaw
          .split(/[,\s]+/)
          .map((value) => value.trim())
          .filter(Boolean),
      ),
    );
    const installAttempts =
      cryptographyFallbackVersions.length > 0 ? cryptographyFallbackVersions : [null];

    let installSucceeded = false;
    let lastFailureDetail = '';

    for (const version of installAttempts) {
      const pipArgs = ['--prefer-binary', '--only-binary=cryptography'];
      if (version) {
        const constraintsPath = path.join(
          outputDir,
          `constraints-win-arm64-cryptography-${version}.txt`,
        );
        fs.writeFileSync(constraintsPath, `cryptography==${version}\n`, 'utf8');
        pipArgs.push('--constraint', constraintsPath);
        console.log(
          `Installing backend dependencies on Windows ARM64 with cryptography fallback ${version} (binary only).`,
        );
      } else {
        console.log(
          'Installing backend dependencies on Windows ARM64 with cryptography binary-only mode (no fallback pin).',
        );
      }
      pipArgs.push('-r', requirementsPath);

      const installResult = runPipInstall(pipArgs);

      if (installResult.error) {
        lastFailureDetail = installResult.error.message;
        continue;
      }
      if (installResult.status === 0) {
        installSucceeded = true;
        break;
      }
      lastFailureDetail = `exit code ${installResult.status}`;
    }

    if (!installSucceeded) {
      throw new Error(
        `Backend runtime dependency installation failed on Windows ARM64 after cryptography binary/fallback attempts (${lastFailureDetail || 'unknown error'}). ` +
          'Set ASTRBOT_DESKTOP_CRYPTOGRAPHY_FALLBACK_VERSIONS to control fallback versions.',
      );
    }
  } else {
    const installResult = runPipInstall(['-r', requirementsPath]);
    if (installResult.error) {
      throw new Error(
        `Failed to install backend runtime dependencies: ${installResult.error.message}`,
      );
    }
    if (installResult.status !== 0) {
      throw new Error(
        `Backend runtime dependency installation failed with exit code ${installResult.status}.`,
      );
    }
  }

  if (process.platform === 'win32') {
    const msvcRuntimeResult = runPipInstall(['--only-binary=:all:', 'msvc-runtime']);
    if (msvcRuntimeResult.error) {
      throw new Error(
        `Failed to install Windows MSVC runtime package: ${msvcRuntimeResult.error.message}`,
      );
    }
    if (msvcRuntimeResult.status !== 0) {
      throw new Error(
        `Windows MSVC runtime installation failed with exit code ${msvcRuntimeResult.status}.`,
      );
    }
  }
};

const main = () => {
  const resolvedSourceDir = requireSourceDir();

  const runtimeSourceReal = resolveAndValidateRuntimeSource({
    projectRoot,
    outputDir,
    runtimeSource,
  });
  const expectedRuntimeConstraint = resolveExpectedRuntimeVersion({
    sourceDir: resolvedSourceDir,
  });

  const sourceRuntimePython = resolveRuntimePython({
    runtimeRoot: runtimeSourceReal,
    outputDir,
  });
  if (!sourceRuntimePython) {
    throw new Error(
      `Cannot find Python executable in runtime source: ${runtimeSourceReal}. Expected python under bin/ or Scripts/.`,
    );
  }
  validateRuntimePython({
    pythonExecutable: sourceRuntimePython.absolute,
    expectedRuntimeConstraint,
    requirePipProbe,
  });

  prepareOutputDirs();
  copyAppSources(resolvedSourceDir);
  const runtimePython = prepareRuntimeExecutable(runtimeSourceReal);
  installRuntimeDependencies(runtimePython);
  writeLauncherScript();
  writeRuntimeManifest(runtimePython);

  console.log(`Prepared CPython backend runtime in ${outputDir}`);
  console.log(`Runtime source: ${runtimeSourceReal}`);
  console.log(`Python executable: ${runtimePython.relative}`);
};

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
