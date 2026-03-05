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

const splitImportTargets = (value) => {
  const results = [];
  for (const rawSegment of value.replace(/[()]/g, ' ').split(',')) {
    let segment = rawSegment.trim();
    if (!segment) {
      continue;
    }
    const [base] = segment.split(/\s+as\s+/i);
    segment = base.replace(/\\\s*$/g, '').trim();
    if (!segment) {
      continue;
    }
    results.push(segment);
  }
  return results;
};

const stripPythonInlineComment = (line, state) => {
  let output = '';
  let index = 0;

  while (index < line.length) {
    const char = line[index];
    const tripleSingle = line.startsWith("'''", index);
    const tripleDouble = line.startsWith('"""', index);

    if (state.inTripleSingleQuote) {
      if (tripleSingle) {
        output += "'''";
        index += 3;
        state.inTripleSingleQuote = false;
        continue;
      }
      output += char;
      index += 1;
      continue;
    }

    if (state.inTripleDoubleQuote) {
      if (tripleDouble) {
        output += '"""';
        index += 3;
        state.inTripleDoubleQuote = false;
        continue;
      }
      output += char;
      index += 1;
      continue;
    }

    if (state.escaped) {
      output += char;
      state.escaped = false;
      index += 1;
      continue;
    }

    if (tripleSingle && !state.inDoubleQuote) {
      state.inTripleSingleQuote = true;
      output += "'''";
      index += 3;
      continue;
    }

    if (tripleDouble && !state.inSingleQuote) {
      state.inTripleDoubleQuote = true;
      output += '"""';
      index += 3;
      continue;
    }

    if (char === '\\') {
      output += char;
      state.escaped = true;
      index += 1;
      continue;
    }

    if (char === "'" && !state.inDoubleQuote) {
      state.inSingleQuote = !state.inSingleQuote;
      output += char;
      index += 1;
      continue;
    }

    if (char === '"' && !state.inSingleQuote) {
      state.inDoubleQuote = !state.inDoubleQuote;
      output += char;
      index += 1;
      continue;
    }

    if (char === '#' && !state.inSingleQuote && !state.inDoubleQuote) {
      break;
    }

    output += char;
    index += 1;
  }

  if (
    !state.inSingleQuote &&
    !state.inDoubleQuote &&
    !state.inTripleSingleQuote &&
    !state.inTripleDoubleQuote
  ) {
    state.escaped = false;
  }

  return output;
};

const collectImportStatements = (lines, filePath) => {
  const statements = [];
  const parsingWarnings = [];
  const commentStripState = {
    inSingleQuote: false,
    inDoubleQuote: false,
    inTripleSingleQuote: false,
    inTripleDoubleQuote: false,
    escaped: false,
  };
  let pendingStatement = '';
  let pendingStartLine = 0;
  let parenthesisDepth = 0;

  for (let index = 0; index < lines.length; index += 1) {
    const startedInTripleQuote =
      commentStripState.inTripleSingleQuote || commentStripState.inTripleDoubleQuote;
    const line = stripPythonInlineComment(lines[index], commentStripState).trim();
    if (startedInTripleQuote && !pendingStatement) {
      continue;
    }
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

    statements.push({ statement: pendingStatement, line: pendingStartLine });
    pendingStatement = '';
    pendingStartLine = 0;
    parenthesisDepth = 0;
  }

  if (pendingStatement) {
    parsingWarnings.push(
      `unterminated import statement in ${path.basename(filePath)}:${pendingStartLine}: ${pendingStatement}`,
    );
  }

  return { statements, parsingWarnings };
};

const applyImportStatement = (
  rawStatement,
  lineNumber,
  filePath,
  imports,
  warnings,
  availableModules = null,
) => {
  const statement = rawStatement.replace(/\s+/g, ' ').trim();
  if (!statement) {
    return;
  }

  const importMatch = statement.match(/^import\s+(.+)$/);
  if (importMatch) {
    for (const modulePart of splitImportTargets(importMatch[1])) {
      const rootModule = modulePart.split('.')[0].trim();
      if (rootModule) {
        imports.add(rootModule);
      }
    }
    return;
  }

  const bareRelativeFromImportMatch = statement.match(/^from\s+(\.+)\s+import\s+(.+)$/);
  const fromImportMatch = statement.match(
    /^from\s+((?:\.+[A-Za-z_][\w.]*)|(?:[A-Za-z_][\w.]*))\s+import\s+(.+)$/,
  );
  if (bareRelativeFromImportMatch || fromImportMatch) {
    const moduleSpec = (bareRelativeFromImportMatch?.[1] || fromImportMatch?.[1] || '').trim();
    const importedPart = (bareRelativeFromImportMatch?.[2] || fromImportMatch?.[2] || '').trim();

    if (moduleSpec.startsWith('.')) {
      const localModule = moduleSpec.replace(/^\.+/, '').split('.')[0].trim();
      if (localModule) {
        imports.add(localModule);
        return;
      }

      for (const importedName of splitImportTargets(importedPart)) {
        const candidateModule = importedName.split('.')[0].trim();
        if (!candidateModule || candidateModule === '*') {
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

  warnings.push(
    `unparsed import statement in ${path.basename(filePath)}:${lineNumber}: ${statement}`,
  );
};

const warnImportScannerWarnings = (warnings) => {
  if (warnings.length > 0) {
    console.warn(`[build-backend] ${warnings.join('; ')}`);
  }
};

const extractImportedRootModules = (filePath, availableModules = null) => {
  const content = fs.readFileSync(filePath, 'utf8');
  const imports = new Set();
  const lines = content.split(/\r?\n/);
  const { statements, parsingWarnings } = collectImportStatements(lines, filePath);

  for (const { statement, line } of statements) {
    applyImportStatement(statement, line, filePath, imports, parsingWarnings, availableModules);
  }

  warnImportScannerWarnings(parsingWarnings);
  return imports;
};

const listAvailableRootModules = (resolvedSourceDir) => {
  const modules = new Map();
  const entries = fs.readdirSync(resolvedSourceDir, { withFileTypes: true });

  for (const entry of entries) {
    if (!entry.isFile() || path.extname(entry.name) !== '.py') {
      continue;
    }
    const moduleName = path.basename(entry.name, '.py');
    modules.set(moduleName, {
      relativePath: entry.name,
      scanPath: path.join(resolvedSourceDir, entry.name),
    });
  }

  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }
    const moduleName = entry.name;
    const initPath = path.join(resolvedSourceDir, moduleName, '__init__.py');
    if (!fs.existsSync(initPath)) {
      continue;
    }
    if (modules.has(moduleName)) {
      console.warn(
        `[build-backend] both module file and package found for "${moduleName}", ` +
          `preferring ${modules.get(moduleName).relativePath}`,
      );
      continue;
    }
    modules.set(moduleName, {
      relativePath: moduleName,
      scanPath: initPath,
    });
  }

  return modules;
};

const warnUnresolvedImports = (unresolvedImports, entryFile) => {
  if (unresolvedImports.size === 0) {
    return;
  }
  console.warn(
    `[build-backend] unresolved root module imports while scanning ${entryFile}: ` +
      `${Array.from(unresolvedImports).sort().join(', ')} ` +
      '(these may be stdlib/third-party imports; verify local helper modules are present when needed).',
  );
};

const resolveRequiredRootPythonFiles = (resolvedSourceDir, entryFile = 'main.py') => {
  const availableModules = listAvailableRootModules(resolvedSourceDir);
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
      const moduleEntry = availableModules.get(importedModule);
      if (!moduleEntry) {
        unresolvedImports.add(`${path.basename(currentFile)} -> ${importedModule}`);
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

  warnUnresolvedImports(unresolvedImports, entryFile);
  return Array.from(required).sort();
};

const getRequiredSourceEntries = (resolvedSourceDir) =>
  Array.from(new Set([...requiredSourceEntries, ...resolveRequiredRootPythonFiles(resolvedSourceDir, 'main.py')]));

const copyAppSources = (resolvedSourceDir) => {
  const requiredEntries = getRequiredSourceEntries(resolvedSourceDir);

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
