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

const listImmediateDirectoriesByPrefix = (rootDir, prefixes) =>
  fs
    .readdirSync(rootDir, { withFileTypes: true })
    .filter(
      (entry) =>
        entry.isDirectory() && prefixes.some((prefix) => entry.name.startsWith(prefix)),
    )
    .map((entry) => path.join(rootDir, entry.name));

const removeDirectoriesByPrefix = (rootDir, prefixes, { recursive = false } = {}) => {
  if (!fs.existsSync(rootDir)) {
    return 0;
  }

  const candidates = recursive
    ? walkDirectoriesRecursively(
        rootDir,
        (_fullPath, dirName) => prefixes.some((prefix) => dirName.startsWith(prefix)),
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

const isPathInside = (candidatePath, rootPath) =>
  candidatePath === rootPath || candidatePath.startsWith(`${rootPath}${path.sep}`);

const getSitePackagesContextFor = (soFile, sitePackagesRoots, libsDirsBySitePackages) => {
  for (const root of sitePackagesRoots) {
    if (!isPathInside(soFile, root)) {
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
  path.dirname(soFile) === runtimeLibDir ||
  pythonLibDirs.some((pythonLibDir) => isPathInside(soFile, pythonLibDir));

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

export const pruneLinuxTkinterRuntime = (runtimeDir) => {
  if (process.platform !== 'linux') {
    return;
  }

  const runtimeLibDir = path.join(runtimeDir, 'lib');
  if (!fs.existsSync(runtimeLibDir)) {
    return;
  }

  let removedCount = 0;

  removedCount += removeFilesByPrefix(runtimeLibDir, ['libtcl', 'libtk']);
  removedCount += removeDirectoriesByPrefix(runtimeLibDir, ['tcl8', 'tcl9', 'tk8', 'tk9', 'itcl']);

  const pythonDynloadDirs = findPythonLibDynloadDirs(runtimeLibDir);
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
    console.warn(
      '[build-backend] patchelf is unavailable; skipping Linux runtime rpath normalization.',
    );
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
