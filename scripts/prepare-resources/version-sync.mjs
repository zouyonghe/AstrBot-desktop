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

const updateCargoLockPackageVersion = ({ cargoLock, packageName, version }) => {
  const packageHeaderPattern = /^\s*\[\[package\]\]\s*(?:#.*)?$/;
  const packageNamePattern = new RegExp(
    `^\\s*name\\s*=\\s*"${escapeRegExp(packageName)}"\\s*(?:#.*)?$`,
  );
  const packageVersionPattern = /^(\s*version\s*=\s*")[^"]+(")(\s*(?:#.*)?)$/;
  const lines = cargoLock.split(/\r?\n/);

  let foundTargetPackage = false;
  let foundTargetVersion = false;

  const updateBlock = (startIndex, endIndex) => {
    let packageNameLineIndex = -1;

    for (let index = startIndex + 1; index < endIndex; index += 1) {
      if (packageNamePattern.test(lines[index])) {
        packageNameLineIndex = index;
        break;
      }
    }

    if (packageNameLineIndex === -1) {
      return false;
    }

    foundTargetPackage = true;

    for (let index = packageNameLineIndex + 1; index < endIndex; index += 1) {
      if (packageHeaderPattern.test(lines[index])) {
        break;
      }
      if (!packageVersionPattern.test(lines[index])) {
        continue;
      }

      foundTargetVersion = true;
      lines[index] = lines[index].replace(packageVersionPattern, `$1${version}$2$3`);
      return true;
    }

    return false;
  };

  let blockStartIndex = -1;
  for (let index = 0; index <= lines.length; index += 1) {
    const isBoundary = index === lines.length || packageHeaderPattern.test(lines[index]);
    if (!isBoundary) {
      continue;
    }

    if (blockStartIndex !== -1 && updateBlock(blockStartIndex, index)) {
      break;
    }

    blockStartIndex = index;
  }

  if (!foundTargetPackage || !foundTargetVersion) {
    throw new Error(`Cannot update Cargo.lock package version for ${packageName}`);
  }

  return lines.join(cargoLock.includes('\r\n') ? '\r\n' : '\n');
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
    const updatedCargoLock = updateCargoLockPackageVersion({
      cargoLock,
      packageName: DESKTOP_TAURI_CRATE_NAME,
      version,
    });
    if (updatedCargoLock !== cargoLock) {
      await writeFile(cargoLockPath, updatedCargoLock, 'utf8');
    }
  }
};
