import fs from 'node:fs';
import path from 'node:path';

const isBytecodeFile = (entryName) => entryName.endsWith('.pyc') || entryName.endsWith('.pyo');

const shouldCopy = (sourcePath) => {
  const base = path.basename(sourcePath);
  if (base === '__pycache__' || base === '.pytest_cache' || base === '.ruff_cache') {
    return false;
  }
  if (base === '.git' || base === '.mypy_cache' || base === '.DS_Store') {
    return false;
  }
  if (isBytecodeFile(base)) {
    return false;
  }
  return true;
};

export const copyTree = (fromPath, toPath, { dereference = false } = {}) => {
  fs.cpSync(fromPath, toPath, {
    recursive: true,
    force: true,
    filter: shouldCopy,
    dereference,
  });
};

export const createPythonInstallEnv = (env = process.env) => ({
  ...env,
  PYTHONDONTWRITEBYTECODE: '1',
});

const countFilesInDirectory = (directoryPath) => {
  let total = 0;
  for (const entry of fs.readdirSync(directoryPath, { withFileTypes: true })) {
    const entryPath = path.join(directoryPath, entry.name);
    if (entry.isDirectory()) {
      total += countFilesInDirectory(entryPath);
      continue;
    }
    total += 1;
  }
  return total;
};

export const prunePythonBytecodeArtifacts = (rootDir) => {
  const stats = {
    removedCacheDirs: 0,
    removedBytecodeFiles: 0,
    removedOrphanBytecodeFiles: 0,
  };

  const visit = (directoryPath) => {
    for (const entry of fs.readdirSync(directoryPath, { withFileTypes: true })) {
      const entryPath = path.join(directoryPath, entry.name);

      if (entry.isDirectory()) {
        if (entry.name === '__pycache__') {
          stats.removedCacheDirs += 1;
          stats.removedBytecodeFiles += countFilesInDirectory(entryPath);
          fs.rmSync(entryPath, { recursive: true, force: true });
          continue;
        }

        visit(entryPath);
        continue;
      }

      if (!isBytecodeFile(entry.name)) {
        continue;
      }

      stats.removedOrphanBytecodeFiles += 1;
      fs.rmSync(entryPath, { force: true });
    }
  };

  if (!fs.existsSync(rootDir)) {
    return stats;
  }

  visit(rootDir);
  return stats;
};

export const resolveAndValidateRuntimeSource = ({ projectRoot, outputDir, runtimeSource }) => {
  if (!runtimeSource) {
    throw new Error(
      'Missing CPython runtime source. Set ASTRBOT_DESKTOP_CPYTHON_HOME ' +
        '(recommended) or ASTRBOT_DESKTOP_BACKEND_RUNTIME.',
    );
  }

  const runtimeSourceReal = path.isAbsolute(runtimeSource)
    ? runtimeSource
    : path.resolve(projectRoot, runtimeSource);
  if (!fs.existsSync(runtimeSourceReal)) {
    throw new Error(`CPython runtime source does not exist: ${runtimeSourceReal}`);
  }

  const normalizeForCompare = (targetPath) => {
    const resolved = path.resolve(targetPath).replace(/[\\/]+$/, '');
    return process.platform === 'win32' ? resolved.toLowerCase() : resolved;
  };

  const runtimeNorm = normalizeForCompare(runtimeSourceReal);
  const outputNorm = normalizeForCompare(outputDir);
  const projectRootNorm = normalizeForCompare(projectRoot);
  const runtimeIsOutputOrSub =
    runtimeNorm === outputNorm || runtimeNorm.startsWith(`${outputNorm}${path.sep}`);
  const outputIsRuntimeOrSub =
    outputNorm === runtimeNorm || outputNorm.startsWith(`${runtimeNorm}${path.sep}`);
  const runtimeContainsProjectRoot =
    runtimeNorm === projectRootNorm || projectRootNorm.startsWith(`${runtimeNorm}${path.sep}`);

  if (runtimeIsOutputOrSub || outputIsRuntimeOrSub) {
    throw new Error(
      `CPython runtime source overlaps with backend output directory. ` +
        `runtime=${runtimeSourceReal}, output=${outputDir}. ` +
        'Please set ASTRBOT_DESKTOP_CPYTHON_HOME to a separate runtime directory.',
    );
  }
  if (runtimeContainsProjectRoot) {
    throw new Error(
      `CPython runtime source is too broad and contains the project root. ` +
        `runtime=${runtimeSourceReal}, projectRoot=${path.resolve(projectRoot)}. ` +
        'Please point ASTRBOT_DESKTOP_CPYTHON_HOME (or ASTRBOT_DESKTOP_BACKEND_RUNTIME) ' +
        'to a dedicated CPython runtime directory instead of the repository root or its parent.',
    );
  }
  if (fs.existsSync(path.join(runtimeSourceReal, 'pyvenv.cfg'))) {
    throw new Error(
      `CPython runtime source must be a distributable CPython runtime, not a virtual environment: ${runtimeSourceReal}. ` +
        'Detected pyvenv.cfg. Please set ASTRBOT_DESKTOP_CPYTHON_HOME (or ASTRBOT_DESKTOP_BACKEND_RUNTIME) ' +
        'to a standalone CPython runtime directory.',
    );
  }

  return runtimeSourceReal;
};

export const resolveRuntimePython = ({ runtimeRoot, outputDir }) => {
  const candidates =
    process.platform === 'win32'
      ? ['python.exe', path.join('Scripts', 'python.exe')]
      : [path.join('bin', 'python3'), path.join('bin', 'python')];

  for (const relativeCandidate of candidates) {
    const candidate = path.join(runtimeRoot, relativeCandidate);
    if (fs.existsSync(candidate)) {
      return {
        absolute: candidate,
        relative: path.relative(outputDir, candidate),
      };
    }
  }

  return null;
};
