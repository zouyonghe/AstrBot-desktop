import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  normalizeDesktopVersionOverride,
  readAstrbotVersionFromPyproject,
  syncDesktopVersionFiles,
} from './version-sync.mjs';

test('normalizeDesktopVersionOverride trims and strips leading v', () => {
  assert.equal(normalizeDesktopVersionOverride(' v1.2.3 '), '1.2.3');
  assert.equal(normalizeDesktopVersionOverride('2.0.0'), '2.0.0');
  assert.equal(normalizeDesktopVersionOverride('   '), '');
});

test('readAstrbotVersionFromPyproject reads [project].version', async () => {
  const tempDir = await mkdtemp(path.join(os.tmpdir(), 'astrbot-version-'));
  try {
    await writeFile(
      path.join(tempDir, 'pyproject.toml'),
      `
[build-system]
requires = ["setuptools"]

[project]
name = "astrbot"
version = "1.9.1"
`,
      'utf8',
    );

    const version = await readAstrbotVersionFromPyproject({ sourceDir: tempDir });
    assert.equal(version, '1.9.1');
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
});

test('syncDesktopVersionFiles updates package.json, tauri.conf.json, Cargo.toml and Cargo.lock', async () => {
  const tempDir = await mkdtemp(path.join(os.tmpdir(), 'astrbot-sync-'));
  try {
    const srcTauriDir = path.join(tempDir, 'src-tauri');
    await mkdir(srcTauriDir, { recursive: true });

    await writeFile(
      path.join(tempDir, 'package.json'),
      `${JSON.stringify({ name: 'test', version: '0.1.0' }, null, 2)}\n`,
      'utf8',
    );
    await writeFile(
      path.join(srcTauriDir, 'tauri.conf.json'),
      `${JSON.stringify({ version: '0.1.0' }, null, 2)}\n`,
      'utf8',
    );
    await writeFile(
      path.join(srcTauriDir, 'Cargo.toml'),
      `[package]\nname = "astrbot-desktop-tauri"\nversion = "0.1.0"\n`,
      'utf8',
    );
    await writeFile(
      path.join(srcTauriDir, 'Cargo.lock'),
      `version = 4

[[package]]
name = "astrbot-desktop-tauri"
version = "0.1.0"

[[package]]
name = "dep"
version = "9.9.9"
`,
      'utf8',
    );

    await syncDesktopVersionFiles({ projectRoot: tempDir, version: '2.3.4' });

    const packageJson = JSON.parse(await readFile(path.join(tempDir, 'package.json'), 'utf8'));
    const tauriConfig = JSON.parse(await readFile(path.join(srcTauriDir, 'tauri.conf.json'), 'utf8'));
    const cargoToml = await readFile(path.join(srcTauriDir, 'Cargo.toml'), 'utf8');
    const cargoLock = await readFile(path.join(srcTauriDir, 'Cargo.lock'), 'utf8');

    assert.equal(packageJson.version, '2.3.4');
    assert.equal(tauriConfig.version, '2.3.4');
    assert.match(cargoToml, /version\s*=\s*"2.3.4"/);
    assert.match(
      cargoLock,
      /\[\[package\]\]\nname = "astrbot-desktop-tauri"\nversion = "2.3.4"/,
    );
    assert.match(cargoLock, /\[\[package\]\]\nname = "dep"\nversion = "9.9.9"/);
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
});

test('syncDesktopVersionFiles tolerates extra Cargo.lock fields between package name and version', async () => {
  const tempDir = await mkdtemp(path.join(os.tmpdir(), 'astrbot-sync-'));
  try {
    const srcTauriDir = path.join(tempDir, 'src-tauri');
    await mkdir(srcTauriDir, { recursive: true });

    await writeFile(
      path.join(tempDir, 'package.json'),
      `${JSON.stringify({ name: 'test', version: '0.1.0' }, null, 2)}\n`,
      'utf8',
    );
    await writeFile(
      path.join(srcTauriDir, 'tauri.conf.json'),
      `${JSON.stringify({ version: '0.1.0' }, null, 2)}\n`,
      'utf8',
    );
    await writeFile(
      path.join(srcTauriDir, 'Cargo.toml'),
      `[package]\nname = "astrbot-desktop-tauri"\nversion = "0.1.0"\n`,
      'utf8',
    );
    await writeFile(
      path.join(srcTauriDir, 'Cargo.lock'),
      `version = 4

[[package]]
name = "astrbot-desktop-tauri"
source = "path+file:///workspace/src-tauri"
version = "0.1.0"
dependencies = [
 "dep",
]

[[package]]
name = "dep"
version = "9.9.9"
source = "registry+https://github.com/rust-lang/crates.io-index"
`,
      'utf8',
    );

    await syncDesktopVersionFiles({ projectRoot: tempDir, version: '2.3.4' });

    const cargoLock = await readFile(path.join(srcTauriDir, 'Cargo.lock'), 'utf8');

    assert.match(
      cargoLock,
      /\[\[package\]\]\nname = "astrbot-desktop-tauri"\nsource = "path\+file:\/\/\/workspace\/src-tauri"\nversion = "2.3.4"/,
    );
    assert.match(
      cargoLock,
      /\[\[package\]\]\nname = "dep"\nversion = "9.9.9"\nsource = "registry\+https:\/\/github.com\/rust-lang\/crates.io-index"/,
    );
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
});
