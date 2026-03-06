import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const SO_LIBRARY_NAME_PATTERN = /\.so(\.[0-9]+)*$/;
const ELF_MAGIC_BYTES = Buffer.from([0x7f, 0x45, 0x4c, 0x46]);

const isSharedObjectFileName = (fileName) => SO_LIBRARY_NAME_PATTERN.test(fileName);

const removeFilesByPrefix = (directory, prefixes) => {
  if (!fs.existsSync(directory)) {
    return 0;
  }

  let removedCount = 0;
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    if (!entry.isFile()) {
      continue;
    }
    if (!prefixes.some((prefix) => entry.name.startsWith(prefix))) {
      continue;
    }
    fs.rmSync(path.join(directory, entry.name), { force: true });
    removedCount += 1;
  }
  return removedCount;
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

const resolveSitePackagesRootForPath = (candidatePath) => {
  const marker = `${path.sep}site-packages${path.sep}`;
  const markerIndex = candidatePath.indexOf(marker);
  if (markerIndex === -1) {
    return null;
  }
  return candidatePath.slice(0, markerIndex + `${path.sep}site-packages`.length);
};

const scanRuntimeTree = (runtimeLibDir) => {
  const soFiles = [];
  const pythonDynloadDirs = [];
  const libsDirsBySitePackages = new Map();

  if (!fs.existsSync(runtimeLibDir)) {
    return { soFiles, pythonDynloadDirs, libsDirsBySitePackages };
  }

  const stack = [runtimeLibDir];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      if (entry.isSymbolicLink()) {
        continue;
      }

      const fullPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        if (
          entry.name === 'lib-dynload' &&
          path.basename(path.dirname(fullPath)).startsWith('python')
        ) {
          pythonDynloadDirs.push(fullPath);
        }

        if (entry.name.endsWith('.libs')) {
          const sitePackagesRoot = resolveSitePackagesRootForPath(fullPath);
          if (sitePackagesRoot) {
            const existing = libsDirsBySitePackages.get(sitePackagesRoot) || [];
            existing.push(fullPath);
            libsDirsBySitePackages.set(sitePackagesRoot, existing);
          }
        }

        stack.push(fullPath);
        continue;
      }

      if (entry.isFile() && isSharedObjectFileName(entry.name)) {
        soFiles.push(fullPath);
      }
    }
  }

  return { soFiles, pythonDynloadDirs, libsDirsBySitePackages };
};

const buildPatchContext = (runtimeDir) => {
  const runtimeLibDir = path.join(runtimeDir, 'lib');
  if (!fs.existsSync(runtimeLibDir)) {
    return {
      runtimeLibDir,
      pythonLibDirs: [],
      libsDirsBySitePackages: new Map(),
      soFiles: [],
      pythonDynloadDirs: [],
    };
  }

  const pythonLibDirs = fs
    .readdirSync(runtimeLibDir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && entry.name.startsWith('python'))
    .map((entry) => path.join(runtimeLibDir, entry.name));

  const { soFiles, pythonDynloadDirs, libsDirsBySitePackages } = scanRuntimeTree(runtimeLibDir);

  return {
    runtimeLibDir,
    pythonLibDirs,
    libsDirsBySitePackages,
    soFiles,
    pythonDynloadDirs,
  };
};

const isPathInside = (candidatePath, rootPath) =>
  candidatePath === rootPath || candidatePath.startsWith(`${rootPath}${path.sep}`);

const shouldPatchRuntimeSoFile = (soFile, runtimeLibDir, pythonLibDirs) =>
  path.dirname(soFile) === runtimeLibDir ||
  pythonLibDirs.some((pythonLibDir) => isPathInside(soFile, pythonLibDir));

const collectLibsDirsForSo = (soFile, libsDirsBySitePackages) => {
  const libsDirs = [];
  for (const [sitePackagesRoot, siteLibsDirs] of libsDirsBySitePackages.entries()) {
    if (!isPathInside(soFile, sitePackagesRoot)) {
      continue;
    }
    libsDirs.push(...siteLibsDirs);
  }
  return libsDirs;
};

const computeRpathSearchEntries = (soFile, libsDirs) => {
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

    const normalizedRelativePath = relativePath.split(path.sep).join('/');
    searchEntries.push(`$ORIGIN/${normalizedRelativePath}`);
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

const logPatchelfFailure = (operation, soFile, spawnResult) => {
  console.warn(
    `[build-backend] Warning: patchelf ${operation} failed for ${soFile} ` +
      `(status=${spawnResult.status ?? 'null'}): ${summarizeSpawnFailure(spawnResult)}`,
  );
};

const patchRuntimeSoFile = (
  soFile,
  { runtimeLibDir, pythonLibDirs, libsDirsBySitePackages },
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
    logPatchelfFailure('--print-rpath', soFile, printRpathResult);
    return false;
  }

  const libsDirs = collectLibsDirsForSo(soFile, libsDirsBySitePackages);
  const searchEntries = computeRpathSearchEntries(soFile, libsDirs);

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
    logPatchelfFailure('--set-rpath', soFile, setRpathResult);
    return false;
  }

  return true;
};

const isTruthyEnv = (value) => ['1', 'true'].includes((value || '').trim().toLowerCase());

export const pruneLinuxTkinterRuntime = (runtimeDir) => {
  if (process.platform !== 'linux') {
    return;
  }

  const { runtimeLibDir, pythonDynloadDirs } = buildPatchContext(runtimeDir);
  if (!fs.existsSync(runtimeLibDir)) {
    return;
  }

  let removedCount = 0;

  removedCount += removeFilesByPrefix(runtimeLibDir, ['libtcl', 'libtk']);

  const removableDirPrefixes = ['tcl8', 'tcl9', 'tk8', 'tk9', 'itcl'];
  for (const entry of fs.readdirSync(runtimeLibDir, { withFileTypes: true })) {
    if (!entry.isDirectory()) {
      continue;
    }
    if (!removableDirPrefixes.some((prefix) => entry.name.startsWith(prefix))) {
      continue;
    }

    fs.rmSync(path.join(runtimeLibDir, entry.name), { recursive: true, force: true });
    removedCount += 1;
  }

  for (const libDynloadDir of pythonDynloadDirs) {
    removedCount += removeFilesByPrefix(libDynloadDir, ['_tkinter']);
  }

  if (removedCount > 0) {
    console.log(
      `[build-backend] removed ${removedCount} tkinter/tcl runtime artifact(s) for Linux AppImage compatibility.`,
    );
  }
};

export const patchLinuxRuntimeRpaths = (runtimeDir) => {
  if (process.platform !== 'linux') {
    return;
  }

  const patchelfProbe = spawnSync('patchelf', ['--version'], {
    encoding: 'utf8',
    windowsHide: true,
  });
  if (patchelfProbe.error || patchelfProbe.status !== 0) {
    const allowMissingPatchelf = isTruthyEnv(process.env.BUILD_BACKEND_ALLOW_MISSING_PATCHELF);
    if (!allowMissingPatchelf) {
      throw new Error(
        '[build-backend] patchelf is required to normalize Linux runtime rpaths. ' +
          'Install patchelf or set BUILD_BACKEND_ALLOW_MISSING_PATCHELF=1 to skip rpath normalization (not recommended for CI/AppImage releases).',
      );
    }

    console.warn(
      '[build-backend] patchelf is unavailable; skipping Linux runtime rpath normalization due to BUILD_BACKEND_ALLOW_MISSING_PATCHELF.',
    );
    return;
  }

  const { runtimeLibDir, pythonLibDirs, libsDirsBySitePackages, soFiles } = buildPatchContext(runtimeDir);

  const patchContext = {
    runtimeLibDir,
    pythonLibDirs,
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
