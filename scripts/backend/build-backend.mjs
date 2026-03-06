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

const buildScannerCacheKey = (filePath) => {
  try {
    const stat = fs.statSync(filePath);
    return `${filePath}:${stat.mtimeMs}:${stat.size}`;
  } catch {
    return '';
  }
};

const invokeScannerProcess = (filePath) => {
  const scannerPython = resolveImportScannerPythonExecutable();
  return spawnSync(scannerPython, [importScannerScriptPath, filePath], {
    encoding: 'utf8',
    windowsHide: true,
    timeout: IMPORT_SCANNER_TIMEOUT_MS,
  });
};

const parseScannerOutput = (result) => {
  if (result.error) {
    return { ok: false, type: 'process', reason: result.error };
  }

  if (result.status !== 0) {
    const details = result.stderr?.trim() || `exit code ${result.status}`;
    return { ok: false, type: 'exit', reason: new Error(details) };
  }

  try {
    const parsed = JSON.parse(result.stdout || '[]');
    if (!Array.isArray(parsed)) {
      throw new Error('scanner output is not an array');
    }
    return { ok: true, value: parsed };
  } catch (error) {
    return {
      ok: false,
      type: 'json',
      reason: error instanceof Error ? error : new Error(String(error)),
    };
  }
};

const warnImportScannerFailure = (filePath, details) => {
  console.warn(
    `[build-backend] failed to scan imports for ${path.basename(filePath)}: ${details}`,
  );
};

const runImportScanner = (filePath) => {
  const cacheKey = buildScannerCacheKey(filePath);
  if (cacheKey && importScannerCache.has(cacheKey)) {
    return importScannerCache.get(cacheKey);
  }

  const result = invokeScannerProcess(filePath);

  if (result.error) {
    if (result.error.code === 'ETIMEDOUT') {
      console.warn(
        `[build-backend] import scanner timed out after ${IMPORT_SCANNER_TIMEOUT_MS}ms for ${path.basename(filePath)}; skipping import analysis for this file.`,
      );
      return [];
    }
    warnImportScannerFailure(filePath, result.error.message || 'unknown process error');
    return [];
  }

  const parsedOutput = parseScannerOutput(result);
  if (!parsedOutput.ok) {
    if (parsedOutput.type === 'json') {
      console.warn(
        `[build-backend] invalid import scanner output for ${path.basename(filePath)}: ${parsedOutput.reason.message}`,
      );
    } else {
      warnImportScannerFailure(filePath, parsedOutput.reason.message);
    }
    return [];
  }

  if (cacheKey) {
    importScannerCache.set(cacheKey, parsedOutput.value);
  }
  return parsedOutput.value;
};

const addRootModule = (imports, relativeBareImports, name, fromRelativeBareImport = false) => {
  if (typeof name !== 'string') {
    return;
  }

  const rootModule = name.split('.')[0].trim();
  if (!rootModule || rootModule === '*') {
    return;
  }

  imports.add(rootModule);
  if (fromRelativeBareImport) {
    relativeBareImports.add(rootModule);
  }
};

const handleImportDescriptor = (descriptor, imports, relativeBareImports) => {
  if (typeof descriptor.module === 'string' && descriptor.module.trim()) {
    addRootModule(imports, relativeBareImports, descriptor.module);
  }
};

const handleFromDescriptor = (descriptor, imports, relativeBareImports) => {
  const level = Number.isInteger(descriptor.level) ? descriptor.level : 0;
  const moduleSpec = typeof descriptor.module === 'string' ? descriptor.module.trim() : '';
  const names = Array.isArray(descriptor.names) ? descriptor.names : [];

  if (level > 0) {
    if (moduleSpec) {
      addRootModule(imports, relativeBareImports, moduleSpec);
      return;
    }

    for (const importedName of names) {
      if (typeof importedName !== 'string') {
        continue;
      }
      addRootModule(imports, relativeBareImports, importedName, true);
    }
    return;
  }

  if (moduleSpec) {
    addRootModule(imports, relativeBareImports, moduleSpec);
  }
};

const extractImportedRootModules = (filePath) => {
  const imports = new Set();
  const relativeBareImports = new Set();
  for (const descriptor of runImportScanner(filePath)) {
    if (!descriptor || typeof descriptor !== 'object') {
      continue;
    }

    if (descriptor.kind === 'import') {
      handleImportDescriptor(descriptor, imports, relativeBareImports);
      continue;
    }

    if (descriptor.kind === 'from') {
      handleFromDescriptor(descriptor, imports, relativeBareImports);
    }
  }

  return { imports, relativeBareImports };
};

const buildModuleCandidate = (resolvedSourceDir, entry) => {
  if (entry.isFile() && path.extname(entry.name) === '.py') {
    return {
      name: path.basename(entry.name, '.py'),
      relativePath: entry.name,
      scanPath: path.join(resolvedSourceDir, entry.name),
      isPackage: false,
    };
  }

  if (!entry.isDirectory()) {
    return null;
  }

  const initPath = path.join(resolvedSourceDir, entry.name, '__init__.py');
  if (!fs.existsSync(initPath)) {
    return null;
  }

  return {
    name: entry.name,
    relativePath: entry.name,
    scanPath: initPath,
    isPackage: true,
  };
};

const choosePreferredModule = (existingModule, candidateModule) => {
  if (!existingModule) {
    return {
      module: candidateModule,
      warning: '',
    };
  }

  if (existingModule.isPackage && !candidateModule.isPackage) {
    return {
      module: candidateModule,
      warning:
        `[build-backend] both module file and package found for "${candidateModule.name}", ` +
        `preferring ${candidateModule.relativePath}`,
    };
  }

  if (!existingModule.isPackage && candidateModule.isPackage) {
    return {
      module: existingModule,
      warning:
        `[build-backend] both module file and package found for "${candidateModule.name}", ` +
        `preferring ${existingModule.relativePath}`,
    };
  }

  return {
    module: existingModule,
    warning: '',
  };
};

const listAvailableRootModules = (resolvedSourceDir) => {
  // The desktop bundle currently tracks root-level modules/packages under sourceDir.
  // Nested package-only relative imports are not traversed as independent entries because
  // top-level package directories are copied as whole trees once selected.
  const modules = new Map();
  const entries = fs.readdirSync(resolvedSourceDir, { withFileTypes: true });

  for (const entry of entries) {
    const candidateModule = buildModuleCandidate(resolvedSourceDir, entry);
    if (!candidateModule) {
      continue;
    }

    const { module, warning } = choosePreferredModule(
      modules.get(candidateModule.name),
      candidateModule,
    );
    if (warning) {
      console.warn(warning);
    }
    modules.set(candidateModule.name, module);
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

const visitFileAndCollectImports = (currentFile) => {
  if (!fs.existsSync(currentFile)) {
    return null;
  }
  return extractImportedRootModules(currentFile);
};

const handleImportedModule = ({
  importedModule,
  availableModules,
  relativeBareImports,
  currentFile,
  entryFile,
  required,
  queue,
  unresolvedImports,
}) => {
  const moduleEntry = availableModules.get(importedModule);
  if (!moduleEntry) {
    if (!relativeBareImports.has(importedModule)) {
      unresolvedImports.push({
        file: path.basename(currentFile),
        module: importedModule,
      });
    }
    return;
  }

  if (moduleEntry.relativePath === entryFile) {
    return;
  }

  if (!required.has(moduleEntry.relativePath)) {
    required.add(moduleEntry.relativePath);
    queue.push(moduleEntry.scanPath);
  }
};

const resolveRequiredRootPythonFiles = (resolvedSourceDir, entryFile = 'main.py') => {
  const availableModules = listAvailableRootModules(resolvedSourceDir);
  const required = new Set();
  const visitedFiles = new Set();
  const unresolvedImports = [];
  const queue = [path.join(resolvedSourceDir, entryFile)];

  while (queue.length > 0) {
    const currentFile = queue.shift();
    if (!currentFile || visitedFiles.has(currentFile)) {
      continue;
    }
    visitedFiles.add(currentFile);
    const importsInfo = visitFileAndCollectImports(currentFile);
    if (!importsInfo) {
      continue;
    }

    const { imports: importedModules, relativeBareImports } = importsInfo;
    for (const importedModule of importedModules) {
      handleImportedModule({
        importedModule,
        availableModules,
        relativeBareImports,
        currentFile,
        entryFile,
        required,
        queue,
        unresolvedImports,
      });
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

const SO_LIBRARY_NAME_PATTERN = /\.so(\.[0-9]+)*$/;
const ELF_MAGIC_BYTES = Buffer.from([0x7f, 0x45, 0x4c, 0x46]);

const isSharedObjectFileName = (fileName) => SO_LIBRARY_NAME_PATTERN.test(fileName);
const startsWithAnyPrefix = (value, prefixes) => prefixes.some((prefix) => value.startsWith(prefix));

const removeFilesByPrefix = (directory, prefixes) => {
  if (!fs.existsSync(directory)) {
    return 0;
  }
  let removedCount = 0;
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    if (!entry.isFile()) {
      continue;
    }
    if (!startsWithAnyPrefix(entry.name, prefixes)) {
      continue;
    }
    fs.rmSync(path.join(directory, entry.name), { force: true });
    removedCount += 1;
  }
  return removedCount;
};

const listImmediateDirectoriesByPrefix = (rootDir, prefixes) =>
  fs
    .readdirSync(rootDir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && startsWithAnyPrefix(entry.name, prefixes))
    .map((entry) => path.join(rootDir, entry.name));

const removeDirectoriesByPrefix = (rootDir, prefixes, { recursive = false } = {}) => {
  if (!fs.existsSync(rootDir)) {
    return 0;
  }

  const candidates = recursive
    ? walkDirectoriesRecursively(
        rootDir,
        (_fullPath, dirName) => startsWithAnyPrefix(dirName, prefixes),
      )
    : listImmediateDirectoriesByPrefix(rootDir, prefixes);

  let removedCount = 0;
  for (const candidatePath of candidates.sort((a, b) => b.length - a.length)) {
    if (!fs.existsSync(candidatePath)) {
      continue;
    }
    fs.rmSync(candidatePath, { recursive: true, force: true });
    removedCount += 1;
  }
  return removedCount;
};

const walkRecursively = (rootDir, { onFile, onDirectory } = {}) => {
  if (!fs.existsSync(rootDir)) {
    return;
  }
  const stack = [rootDir];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      if (entry.isSymbolicLink()) {
        continue;
      }
      const fullPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        if (onDirectory) {
          onDirectory(fullPath, entry.name);
        }
        stack.push(fullPath);
        continue;
      }
      if (entry.isFile() && onFile) {
        onFile(fullPath, entry.name);
      }
    }
  }
};

const walkFilesRecursively = (rootDir, predicate) => {
  const collected = [];
  walkRecursively(rootDir, {
    onFile: (fullPath, fileName) => {
      if (predicate(fullPath, fileName)) {
        collected.push(fullPath);
      }
    },
  });
  return collected;
};

const walkDirectoriesRecursively = (rootDir, predicate) => {
  const collected = [];
  walkRecursively(rootDir, {
    onDirectory: (fullPath, dirName) => {
      if (predicate(fullPath, dirName)) {
        collected.push(fullPath);
      }
    },
  });
  return collected;
};

const hasElfMagic = (filePath) => {
  let fd;
  try {
    fd = fs.openSync(filePath, 'r');
    const header = Buffer.alloc(ELF_MAGIC_BYTES.length);
    const bytesRead = fs.readSync(fd, header, 0, header.length, 0);
    return bytesRead === ELF_MAGIC_BYTES.length && header.equals(ELF_MAGIC_BYTES);
  } catch {
    return false;
  } finally {
    if (fd !== undefined) {
      try {
        fs.closeSync(fd);
      } catch {
        // Ignore close errors on best-effort ELF detection.
      }
    }
  }
};

const isPathInsideRoot = (candidatePath, rootPath) =>
  candidatePath === rootPath || candidatePath.startsWith(`${rootPath}${path.sep}`);

const isPathInsideAnyRoot = (candidatePath, roots) =>
  roots.some((rootPath) => isPathInsideRoot(candidatePath, rootPath));

const findPythonLibDynloadDirs = (runtimeLibDir) =>
  walkDirectoriesRecursively(
    runtimeLibDir,
    (dirPath, dirName) =>
      dirName === 'lib-dynload' && path.basename(path.dirname(dirPath)).startsWith('python'),
  );

const getPythonLibDirs = (runtimeLibDir) =>
  fs.existsSync(runtimeLibDir)
    ? fs
        .readdirSync(runtimeLibDir, { withFileTypes: true })
        .filter((entry) => entry.isDirectory() && entry.name.startsWith('python'))
        .map((entry) => path.join(runtimeLibDir, entry.name))
    : [];

const getSitePackagesRoots = (pythonLibDirs) =>
  pythonLibDirs
    .map((pythonDir) => path.join(pythonDir, 'site-packages'))
    .filter((sitePackagesDir) => fs.existsSync(sitePackagesDir));

const buildLibsDirsBySitePackages = (sitePackagesRoots) => {
  const libsDirsBySitePackages = new Map();
  for (const sitePackagesRoot of sitePackagesRoots) {
    const libsDirs = walkDirectoriesRecursively(
      sitePackagesRoot,
      (_fullPath, dirName) => dirName.endsWith('.libs'),
    );
    libsDirsBySitePackages.set(sitePackagesRoot, libsDirs);
  }
  return libsDirsBySitePackages;
};

const getSitePackagesContextFor = (soFile, sitePackagesRoots, libsDirsBySitePackages) => {
  for (const root of sitePackagesRoots) {
    if (!isPathInsideRoot(soFile, root)) {
      continue;
    }
    return {
      root,
      libsDirs: libsDirsBySitePackages.get(root) || [],
    };
  }
  return { root: null, libsDirs: [] };
};

const shouldPatchRuntimeSoFile = (soFile, runtimeLibDir, pythonLibDirs) =>
  path.dirname(soFile) === runtimeLibDir || isPathInsideAnyRoot(soFile, pythonLibDirs);

const computeRpathSearchEntries = (soFile, { libsDirs }) => {
  const searchEntries = [
    '$ORIGIN',
    '$ORIGIN/..',
    '$ORIGIN/../..',
    '$ORIGIN/../../..',
    '$ORIGIN/../../../..',
    '$ORIGIN/../.libs',
  ];

  for (const libsDir of libsDirs) {
    const relativePath = path.relative(path.dirname(soFile), libsDir);
    if (!relativePath || relativePath === '.') {
      searchEntries.push('$ORIGIN');
      continue;
    }
    if (relativePath.startsWith('..')) {
      searchEntries.push(`$ORIGIN/${relativePath.split(path.sep).join('/')}`);
    }
  }

  return searchEntries;
};

const parseRpathEntries = (rawRpath) =>
  (rawRpath || '')
    .trim()
    .split(':')
    .map((entry) => entry.trim())
    .filter(Boolean);

const summarizeSpawnFailure = (spawnResult) => {
  if (spawnResult.error) {
    return spawnResult.error.message;
  }
  const stderr = (spawnResult.stderr || '').trim();
  if (!stderr) {
    return 'unknown error';
  }
  return stderr.length > 240 ? `${stderr.slice(0, 240)}...` : stderr;
};

const warnPatchelfFailure = (operation, soFile, spawnResult) => {
  console.warn(
    `[build-backend] Warning: patchelf ${operation} failed for ${soFile} ` +
      `(status=${spawnResult.status ?? 'null'}): ${summarizeSpawnFailure(spawnResult)}`,
  );
};

const patchRuntimeSoFile = (
  soFile,
  { runtimeLibDir, pythonLibDirs, sitePackagesRoots, libsDirsBySitePackages },
) => {
  if (!shouldPatchRuntimeSoFile(soFile, runtimeLibDir, pythonLibDirs)) {
    return false;
  }
  if (!hasElfMagic(soFile)) {
    return false;
  }

  const printRpathResult = spawnSync('patchelf', ['--print-rpath', soFile], {
    encoding: 'utf8',
    windowsHide: true,
  });
  if (printRpathResult.error || printRpathResult.status !== 0) {
    warnPatchelfFailure('--print-rpath', soFile, printRpathResult);
    return false;
  }

  const sitePackagesContext = getSitePackagesContextFor(
    soFile,
    sitePackagesRoots,
    libsDirsBySitePackages,
  );
  const searchEntries = computeRpathSearchEntries(soFile, sitePackagesContext);

  const existingRpathEntries = parseRpathEntries(printRpathResult.stdout);
  const finalEntries = Array.from(new Set([...existingRpathEntries, ...searchEntries]));

  const rpathUnchanged =
    finalEntries.length === existingRpathEntries.length &&
    finalEntries.every((entry, index) => entry === existingRpathEntries[index]);
  if (rpathUnchanged) {
    return false;
  }

  const setRpathResult = spawnSync('patchelf', ['--set-rpath', finalEntries.join(':'), soFile], {
    encoding: 'utf8',
    windowsHide: true,
  });
  if (setRpathResult.error || setRpathResult.status !== 0) {
    warnPatchelfFailure('--set-rpath', soFile, setRpathResult);
    return false;
  }

  return true;
};

const pruneLinuxTkinterRuntime = () => {
  if (process.platform !== 'linux') {
    return;
  }

  const runtimeLibDir = path.join(runtimeDir, 'lib');
  if (!fs.existsSync(runtimeLibDir)) {
    return;
  }

  let removedCount = 0;

  // Tk/Tcl artifacts are not required by the desktop backend and can break
  // AppImage dependency resolution on some Linux hosts.
  removedCount += removeFilesByPrefix(runtimeLibDir, ['libtcl', 'libtk']);
  removedCount += removeDirectoriesByPrefix(runtimeLibDir, ['tcl8', 'tcl9', 'tk8', 'tk9', 'itcl']);

  const pythonDynloadDirs = findPythonLibDynloadDirs(runtimeLibDir);
  for (const libDynloadDir of pythonDynloadDirs) {
    removedCount += removeFilesByPrefix(libDynloadDir, ['_tkinter']);
  }

  if (removedCount > 0) {
    console.log(`[build-backend] removed ${removedCount} tkinter/tcl runtime artifact(s) for Linux AppImage compatibility.`);
  }
};

const patchLinuxRuntimeRpaths = () => {
  if (process.platform !== 'linux') {
    return;
  }

  const patchelfProbe = spawnSync('patchelf', ['--version'], {
    encoding: 'utf8',
    windowsHide: true,
  });
  if (patchelfProbe.error || patchelfProbe.status !== 0) {
    console.warn('[build-backend] patchelf is unavailable; skipping Linux runtime rpath normalization.');
    return;
  }

  const runtimeLibDir = path.join(runtimeDir, 'lib');
  const pythonLibDirs = getPythonLibDirs(runtimeLibDir);
  const sitePackagesRoots = getSitePackagesRoots(pythonLibDirs);
  const libsDirsBySitePackages = buildLibsDirsBySitePackages(sitePackagesRoots);
  const soFiles = walkFilesRecursively(runtimeLibDir, (_fullPath, fileName) =>
    isSharedObjectFileName(fileName),
  );
  const patchContext = {
    runtimeLibDir,
    pythonLibDirs,
    sitePackagesRoots,
    libsDirsBySitePackages,
  };

  let patchedCount = 0;
  for (const soFile of soFiles) {
    if (patchRuntimeSoFile(soFile, patchContext)) {
      patchedCount += 1;
    }
  }

  if (patchedCount > 0) {
    console.log(
      `[build-backend] normalized rpath for ${patchedCount} Linux runtime shared object(s) to stabilize AppImage dependency resolution.`,
    );
  }
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
  pruneLinuxTkinterRuntime();
  patchLinuxRuntimeRpaths();
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
