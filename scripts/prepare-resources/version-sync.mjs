import { existsSync } from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

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
    const cargoLockVersionPattern =
      /(\[\[package\]\]\s+name = "astrbot-desktop-tauri"\s+version = ")[^"]+(")/m;
    if (!cargoLockVersionPattern.test(cargoLock)) {
      throw new Error(`Cannot update Cargo.lock package version in ${cargoLockPath}`);
    }
    const updatedCargoLock = cargoLock.replace(cargoLockVersionPattern, `$1${version}$2`);
    if (updatedCargoLock !== cargoLock) {
      await writeFile(cargoLockPath, updatedCargoLock, 'utf8');
    }
  }
};
