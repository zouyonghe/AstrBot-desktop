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

const scanRuntimeTree = (
  runtimeDir,
  {
    wantSoFiles = false,
    wantPythonDynloadDirs = false,
    wantLibsDirsBySitePackages = false,
    wantPythonLibDirs = false,
  } = {},
) => {
  const runtimeLibDir = path.join(runtimeDir, 'lib');
  const soFiles = [];
  const pythonDynloadDirs = [];
  const libsDirsBySitePackages = new Map();
  const pythonLibDirs = [];

  if (!fs.existsSync(runtimeLibDir)) {
    return {
      runtimeLibDir,
      soFiles,
      pythonDynloadDirs,
      libsDirsBySitePackages,
      pythonLibDirs,
    };
  }

  if (wantPythonLibDirs) {
    for (const entry of fs.readdirSync(runtimeLibDir, { withFileTypes: true })) {
      if (entry.isDirectory() && entry.name.startsWith('python')) {
        pythonLibDirs.push(path.join(runtimeLibDir, entry.name));
      }
    }
  }

  const shouldTraverseRecursively =
    wantSoFiles || wantPythonDynloadDirs || wantLibsDirsBySitePackages;
  if (!shouldTraverseRecursively) {
    return {
      runtimeLibDir,
      soFiles,
      pythonDynloadDirs,
      libsDirsBySitePackages,
      pythonLibDirs,
    };
  }

  const stack = [runtimeLibDir];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const fullPath = path.join(current, entry.name);
      if (entry.isSymbolicLink()) {
        if (wantSoFiles && isSharedObjectFileName(entry.name)) {
          try {
            if (fs.statSync(fullPath).isFile()) {
              soFiles.push(fullPath);
            }
          } catch {
            // Ignore broken symlinks or inaccessible targets.
          }
        }
        continue;
      }

      if (entry.isDirectory()) {
        if (
          wantPythonDynloadDirs &&
          entry.name === 'lib-dynload' &&
          path.basename(path.dirname(fullPath)).startsWith('python')
        ) {
          pythonDynloadDirs.push(fullPath);
        }

        if (wantLibsDirsBySitePackages && entry.name.endsWith('.libs')) {
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

      if (wantSoFiles && entry.isFile() && isSharedObjectFileName(entry.name)) {
        soFiles.push(fullPath);
      }
    }
  }

  return {
    runtimeLibDir,
    soFiles,
    pythonDynloadDirs,
    libsDirsBySitePackages,
    pythonLibDirs,
  };
};

const collectLibsDirsForSo = (soFile, libsDirsBySitePackages) => {
  const libsDirs = [];
  for (const [sitePackagesRoot, siteLibsDirs] of libsDirsBySitePackages.entries()) {
    if (
      soFile !== sitePackagesRoot &&
      !soFile.startsWith(`${sitePackagesRoot}${path.sep}`)
    ) {
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

const logPatchelfFailure = (operation, soFile, spawnResult) => {
  let summary;
  if (spawnResult.error) {
    summary = spawnResult.error.message;
  } else {
    const stderr = (spawnResult.stderr || '').trim();
    summary = stderr
      ? (stderr.length > 240 ? `${stderr.slice(0, 240)}...` : stderr)
      : 'unknown error';
  }

  console.warn(
    `[build-backend] Warning: patchelf ${operation} failed for ${soFile} ` +
      `(status=${spawnResult.status ?? 'null'}): ${summary}`,
  );
};

const resolvePatchelfCommand = () => {
  const configuredPath = (process.env.BUILD_BACKEND_PATCHELF_PATH || '').trim();
  return configuredPath || 'patchelf';
};

const patchRuntimeSoFile = (
  soFile,
  { runtimeLibDir, pythonLibDirs, libsDirsBySitePackages, patchelfCommand },
) => {
  const underPythonLib = pythonLibDirs.some(
    (pythonLibDir) =>
      soFile === pythonLibDir || soFile.startsWith(`${pythonLibDir}${path.sep}`),
  );
  if (path.dirname(soFile) !== runtimeLibDir && !underPythonLib) {
    return false;
  }
  if (!hasElfMagic(soFile)) {
    return false;
  }

  const printRpathResult = spawnSync(patchelfCommand, ['--print-rpath', soFile], {
    encoding: 'utf8',
    windowsHide: true,
  });
  if (printRpathResult.error || printRpathResult.status !== 0) {
    logPatchelfFailure('--print-rpath', soFile, printRpathResult);
    return false;
  }

  const libsDirs = collectLibsDirsForSo(soFile, libsDirsBySitePackages);
  const searchEntries = computeRpathSearchEntries(soFile, libsDirs);

  const existingRpathEntries = (printRpathResult.stdout || '')
    .trim()
    .split(':')
    .map((entry) => entry.trim())
    .filter(Boolean);
  const finalEntries = Array.from(new Set([...searchEntries, ...existingRpathEntries]));

  const rpathUnchanged =
    finalEntries.length === existingRpathEntries.length &&
    finalEntries.every((entry, index) => entry === existingRpathEntries[index]);
  if (rpathUnchanged) {
    return false;
  }

  const setRpathResult = spawnSync(
    patchelfCommand,
    ['--set-rpath', finalEntries.join(':'), soFile],
    {
      encoding: 'utf8',
      windowsHide: true,
    },
  );
  if (setRpathResult.error || setRpathResult.status !== 0) {
    logPatchelfFailure('--set-rpath', soFile, setRpathResult);
    return false;
  }

  return true;
};

const isTruthyEnv = (value) => ['1', 'true'].includes((value || '').trim().toLowerCase());
const requirePatchelf = () =>
  isTruthyEnv(process.env.BUILD_BACKEND_REQUIRE_PATCHELF) ||
  isTruthyEnv(process.env.CI);

export const pruneLinuxTkinterRuntime = (runtimeDir) => {
  if (process.platform !== 'linux') {
    return;
  }

  const { runtimeLibDir, pythonDynloadDirs } = scanRuntimeTree(runtimeDir, {
    wantPythonDynloadDirs: true,
  });
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

  const patchelfCommand = resolvePatchelfCommand();
  const patchelfProbe = spawnSync(patchelfCommand, ['--version'], {
    encoding: 'utf8',
    windowsHide: true,
  });
  if (patchelfProbe.error || patchelfProbe.status !== 0) {
    if (requirePatchelf()) {
      throw new Error(
        `[build-backend] ${patchelfCommand} is required to normalize Linux runtime rpaths. ` +
          'Install patchelf, or unset BUILD_BACKEND_REQUIRE_PATCHELF / CI for local-only builds.',
      );
    }

    console.warn(
      `[build-backend] ${patchelfCommand} is unavailable; skipping Linux runtime rpath normalization. ` +
        'Set BUILD_BACKEND_REQUIRE_PATCHELF=1 or CI=1 to enforce this check.',
    );
    return;
  }

  const { runtimeLibDir, pythonLibDirs, libsDirsBySitePackages, soFiles } = scanRuntimeTree(
    runtimeDir,
    {
      wantSoFiles: true,
      wantLibsDirsBySitePackages: true,
      wantPythonLibDirs: true,
    },
  );

  const patchContext = {
    runtimeLibDir,
    pythonLibDirs,
    libsDirsBySitePackages,
    patchelfCommand,
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
