import { existsSync } from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

export const DESKTOP_TAURI_CRATE_NAME = 'astrbot-desktop-tauri';

export const normalizeDesktopVersionOverride = (version) => {
  const trimmed = typeof version === 'string' ? version.trim() : '';
  if (!trimmed) {
    return '';
  }
  if (/^v\d/i.test(trimmed)) {
    return trimmed.slice(1);
  }
  return trimmed;
};

export const readAstrbotVersionFromPyproject = async ({ sourceDir }) => {
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

const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const CARGO_LOCK_PACKAGE_HEADER = /^\s*\[\[package\]\]\s*(?:#.*)?$/;

class CargoLockPackageNotFoundError extends Error {
  constructor(packageName) {
    super(`Cannot update Cargo.lock: package "${packageName}" not found`);
    this.name = 'CargoLockPackageNotFoundError';
  }
}

const findCargoLockPackageBlocks = (lines) => {
  const blocks = [];
  let start = null;

  for (let index = 0; index < lines.length; index += 1) {
    if (!CARGO_LOCK_PACKAGE_HEADER.test(lines[index])) {
      continue;
    }

    if (start !== null) {
      blocks.push({ start, end: index });
    }
    start = index;
  }

  if (start !== null) {
    blocks.push({ start, end: lines.length });
  }

  return blocks;
};

const updateVersionLine = (line, version) => {
  const commentIndex = line.indexOf('#');
  const beforeComment = commentIndex === -1 ? line : line.slice(0, commentIndex);
  const comment = commentIndex === -1 ? '' : line.slice(commentIndex);
  const separatorIndex = beforeComment.indexOf('=');

  if (separatorIndex === -1) {
    return null;
  }

  const left = beforeComment.slice(0, separatorIndex).trimEnd();
  const right = beforeComment.slice(separatorIndex + 1);
  if (!right.trim()) {
    return null;
  }

  const quote = right.includes("'") ? "'" : '"';
  const trailingWhitespace = beforeComment.match(/\s*$/u)?.[0] ?? '';
  const updatedLine = `${left} = ${quote}${version}${quote}`;

  if (!comment) {
    return `${updatedLine}${trailingWhitespace}`;
  }

  return `${updatedLine}${trailingWhitespace}${comment}`;
};

const updateCargoLockPackageVersion = ({ cargoLock, packageName, version }) => {
  const lines = cargoLock.split(/\r?\n/);
  const packageBlocks = findCargoLockPackageBlocks(lines);
  const packageNameLinePattern = new RegExp(
    `^\\s*name\\s*=\\s*"${escapeRegExp(packageName)}"\\s*(?:#.*)?$`,
  );

  for (const { start, end } of packageBlocks) {
    let packageNameLineIndex = -1;

    for (let index = start + 1; index < end; index += 1) {
      if (!packageNameLinePattern.test(lines[index])) {
        continue;
      }

      packageNameLineIndex = index;
      break;
    }

    if (packageNameLineIndex === -1) {
      continue;
    }

    for (let index = packageNameLineIndex + 1; index < end; index += 1) {
      if (!lines[index].trimStart().startsWith('version')) {
        continue;
      }

      const updatedLine = updateVersionLine(lines[index], version);
      if (updatedLine === null) {
        break;
      }

      lines[index] = updatedLine;
      return lines.join(cargoLock.includes('\r\n') ? '\r\n' : '\n');
    }

    throw new Error(
      `Cannot update Cargo.lock: version entry for package "${packageName}" not found or has an unexpected layout`,
    );
  }

  throw new CargoLockPackageNotFoundError(packageName);
};

export const syncDesktopVersionFiles = async ({ projectRoot, version }) => {
  const packageJsonPath = path.join(projectRoot, 'package.json');
  const tauriConfigPath = path.join(projectRoot, 'src-tauri', 'tauri.conf.json');
  const cargoTomlPath = path.join(projectRoot, 'src-tauri', 'Cargo.toml');
  const cargoLockPath = path.join(projectRoot, 'src-tauri', 'Cargo.lock');

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

  if (existsSync(cargoLockPath)) {
    const cargoLock = await readFile(cargoLockPath, 'utf8');
    let updatedCargoLock = cargoLock;
    try {
      updatedCargoLock = updateCargoLockPackageVersion({
        cargoLock,
        packageName: DESKTOP_TAURI_CRATE_NAME,
        version,
      });
    } catch (error) {
      if (error instanceof CargoLockPackageNotFoundError) {
        console.warn(`${cargoLockPath}: ${error.message}. Skipping Cargo.lock version sync.`);
      } else {
        throw error;
      }
    }
    if (updatedCargoLock !== cargoLock) {
      await writeFile(cargoLockPath, updatedCargoLock, 'utf8');
    }
  }
};
