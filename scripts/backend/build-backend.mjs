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

const extractImportedRootModules = (filePath, availableModules = null) => {
  const content = fs.readFileSync(filePath, 'utf8');
  const imports = new Set();
  const parsingWarnings = [];
  const lines = content.split(/\r?\n/);

  const splitImportTargets = (value) =>
    value
      .replace(/[()]/g, ' ')
      .split(',')
      .map((segment) => segment.trim())
      .filter(Boolean)
      .map((segment) => segment.split(/\s+as\s+/i)[0].trim())
      .map((segment) => segment.replace(/\\\s*$/g, '').trim())
      .filter(Boolean);

  const parseImportStatement = (statement, lineNumber) => {
    const normalizedStatement = statement.replace(/\\\s*/g, ' ').replace(/\s+/g, ' ').trim();
    if (!normalizedStatement) {
      return;
    }

    const importMatch = normalizedStatement.match(/^import\s+(.+)$/);
    if (importMatch) {
      for (const modulePart of splitImportTargets(importMatch[1])) {
        const rootModule = modulePart.split('.')[0].trim();
        if (rootModule) {
          imports.add(rootModule);
        }
      }
      return;
    }

    const fromImportMatch = normalizedStatement.match(/^from\s+([.A-Za-z_][\w.]*)\s+import\s+(.+)$/);
    if (fromImportMatch) {
      const moduleSpec = fromImportMatch[1].trim();
      const importedPart = fromImportMatch[2].trim();
      if (moduleSpec.startsWith('.')) {
        const localModule = moduleSpec.replace(/^\.+/, '').split('.')[0].trim();
        if (localModule) {
          imports.add(localModule);
          return;
        }

        for (const importedName of splitImportTargets(importedPart)) {
          const candidateModule = importedName.split('.')[0].trim();
          if (!candidateModule) {
            continue;
          }
          if (!availableModules || availableModules.has(candidateModule)) {
            imports.add(candidateModule);
          }
        }
        return;
      }

      const rootModule = moduleSpec.split('.')[0].trim();
      if (rootModule) {
        imports.add(rootModule);
      }
      return;
    }

    parsingWarnings.push(
      `unparsed import statement in ${path.basename(filePath)}:${lineNumber}: ${normalizedStatement}`,
    );
  };

  let pendingStatement = '';
  let pendingStartLine = 0;
  let parenthesisDepth = 0;

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index].split('#')[0].trim();
    if (!line && !pendingStatement) {
      continue;
    }

    if (!pendingStatement) {
      if (!/^(import|from)\b/.test(line)) {
        continue;
      }
      pendingStatement = line;
      pendingStartLine = index + 1;
      parenthesisDepth = (line.match(/\(/g) || []).length - (line.match(/\)/g) || []).length;
    } else {
      pendingStatement = `${pendingStatement} ${line}`.trim();
      parenthesisDepth += (line.match(/\(/g) || []).length - (line.match(/\)/g) || []).length;
    }

    const hasLineContinuation = /\\\s*$/.test(line);
    if (parenthesisDepth > 0 || hasLineContinuation) {
      continue;
    }

    parseImportStatement(pendingStatement, pendingStartLine);
    pendingStatement = '';
    pendingStartLine = 0;
    parenthesisDepth = 0;
  }

  if (pendingStatement) {
    parsingWarnings.push(
      `unterminated import statement in ${path.basename(filePath)}:${pendingStartLine}: ${pendingStatement}`,
    );
  }

  if (parsingWarnings.length > 0) {
    console.warn(`[build-backend] ${parsingWarnings.join('; ')}`);
  }

  return imports;
};

const resolveRequiredRootPythonFiles = (resolvedSourceDir, entryFile = 'main.py') => {
  const availableModules = new Set(
    fs
      .readdirSync(resolvedSourceDir, { withFileTypes: true })
      .filter((entry) => entry.isFile() && path.extname(entry.name) === '.py')
      .map((entry) => path.basename(entry.name, '.py')),
  );
  const required = new Set();
  const visitedFiles = new Set();
  const unresolvedImports = new Set();
  const queue = [path.join(resolvedSourceDir, entryFile)];

  while (queue.length > 0) {
    const currentFile = queue.shift();
    if (!currentFile || visitedFiles.has(currentFile) || !fs.existsSync(currentFile)) {
      continue;
    }
    visitedFiles.add(currentFile);
    const importedModules = extractImportedRootModules(currentFile, availableModules);

    for (const importedModule of importedModules) {
      if (!availableModules.has(importedModule)) {
        unresolvedImports.add(`${path.basename(currentFile)} -> ${importedModule}`);
        continue;
      }
      const relativeFile = `${importedModule}.py`;
      if (relativeFile === entryFile) {
        continue;
      }
      if (!required.has(relativeFile)) {
        required.add(relativeFile);
        queue.push(path.join(resolvedSourceDir, relativeFile));
      }
    }
  }

  if (unresolvedImports.size > 0) {
    console.warn(
      `[build-backend] unresolved root module imports while scanning ${entryFile}: ` +
        `${Array.from(unresolvedImports).sort().join(', ')} ` +
        '(these may be stdlib/third-party imports; verify local helper modules are present when needed).',
    );
  }

  return Array.from(required).sort();
};

const copyAppSources = (resolvedSourceDir) => {
  const requiredEntries = Array.from(
    new Set([...requiredSourceEntries, ...resolveRequiredRootPythonFiles(resolvedSourceDir, 'main.py')]),
  );

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
