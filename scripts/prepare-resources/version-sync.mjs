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
const CARGO_LOCK_ANY_HEADER = /^\s*\[\[/;
const CARGO_LOCK_VERSION_LINE = /^\s*version\s*=/;
const escapeTomlBasicString = (value) => String(value).replace(/\\/g, '\\\\').replace(/"/g, '\\"');

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

  const trailingWhitespace = beforeComment.match(/\s*$/u)?.[0] ?? '';
  const updatedLine = `${left} = "${escapeTomlBasicString(version)}"`;

  if (!comment) {
    return `${updatedLine}${trailingWhitespace}`;
  }

  return `${updatedLine}${trailingWhitespace}${comment}`;
};

const updateCargoLockPackageVersion = ({ cargoLock, packageName, version }) => {
  const lines = cargoLock.split(/\r?\n/);
  const newline = cargoLock.includes('\r\n') ? '\r\n' : '\n';
  const packageNameLinePattern = new RegExp(
    `^\\s*name\\s*=\\s*"${escapeRegExp(packageName)}"\\s*(?:#.*)?$`,
  );

  let inPackageBlock = false;
  let inTargetPackage = false;
  let foundTargetPackage = false;

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];

    if (CARGO_LOCK_PACKAGE_HEADER.test(line)) {
      if (inTargetPackage) {
        throw new Error(
          `Cannot update Cargo.lock: version entry for package "${packageName}" not found or has an unexpected layout`,
        );
      }
      inPackageBlock = true;
      inTargetPackage = false;
      continue;
    }

    if (inPackageBlock && CARGO_LOCK_ANY_HEADER.test(line)) {
      if (inTargetPackage) {
        throw new Error(
          `Cannot update Cargo.lock: version entry for package "${packageName}" not found or has an unexpected layout`,
        );
      }
      inPackageBlock = false;
      inTargetPackage = false;
    }

    if (!inPackageBlock) {
      continue;
    }

    if (!inTargetPackage && packageNameLinePattern.test(line)) {
      inTargetPackage = true;
      foundTargetPackage = true;
      continue;
    }

    if (!inTargetPackage || !CARGO_LOCK_VERSION_LINE.test(line)) {
      continue;
    }

    const updatedLine = updateVersionLine(line, version);
    if (updatedLine === null) {
      throw new Error(
        `Cannot update Cargo.lock: version entry for package "${packageName}" not found or has an unexpected layout`,
      );
    }

    lines[index] = updatedLine;
    return { content: lines.join(newline), updated: true, foundTargetPackage: true };
  }

  if (inTargetPackage) {
    throw new Error(
      `Cannot update Cargo.lock: version entry for package "${packageName}" not found or has an unexpected layout`,
    );
  }

  return { content: cargoLock, updated: false, foundTargetPackage };
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
    const { content: updatedCargoLock, updated, foundTargetPackage } = updateCargoLockPackageVersion({
      cargoLock,
      packageName: DESKTOP_TAURI_CRATE_NAME,
      version,
    });

    if (!foundTargetPackage) {
      console.warn(
        `${cargoLockPath}: package "${DESKTOP_TAURI_CRATE_NAME}" not found. Skipping Cargo.lock version sync.`,
      );
    } else if (updated && updatedCargoLock !== cargoLock) {
      await writeFile(cargoLockPath, updatedCargoLock, 'utf8');
    }
  }
};
