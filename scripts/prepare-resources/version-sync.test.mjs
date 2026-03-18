import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  DESKTOP_TAURI_CRATE_NAME,
  normalizeDesktopVersionOverride,
  readAstrbotVersionFromPyproject,
  syncDesktopVersionFiles,
} from './version-sync.mjs';

const createTempDesktopProject = async ({ cargoLockContents, version = '0.1.0' }) => {
  const tempDir = await mkdtemp(path.join(os.tmpdir(), 'astrbot-sync-'));
  const srcTauriDir = path.join(tempDir, 'src-tauri');

  await mkdir(srcTauriDir, { recursive: true });
  await writeFile(
    path.join(tempDir, 'package.json'),
    `${JSON.stringify({ name: 'test', version }, null, 2)}\n`,
    'utf8',
  );
  await writeFile(
    path.join(srcTauriDir, 'tauri.conf.json'),
    `${JSON.stringify({ version }, null, 2)}\n`,
    'utf8',
  );
  await writeFile(
    path.join(srcTauriDir, 'Cargo.toml'),
    `[package]\nname = "${DESKTOP_TAURI_CRATE_NAME}"\nversion = "${version}"\n`,
    'utf8',
  );

  if (typeof cargoLockContents === 'string') {
    await writeFile(path.join(srcTauriDir, 'Cargo.lock'), cargoLockContents, 'utf8');
  }

  return { tempDir, srcTauriDir };
};

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
  let tempDir;
  let srcTauriDir;
  try {
    ({ tempDir, srcTauriDir } = await createTempDesktopProject({
      cargoLockContents: `version = 4

[[package]]
name = "${DESKTOP_TAURI_CRATE_NAME}"
version = "0.1.0"

[[package]]
name = "dep"
version = "9.9.9"
`,
    }));

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
      new RegExp(`\\[\\[package\\]\\]\\nname = "${DESKTOP_TAURI_CRATE_NAME}"\\nversion = "2.3.4"`),
    );
    assert.match(cargoLock, /\[\[package\]\]\nname = "dep"\nversion = "9.9.9"/);
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
});

test('syncDesktopVersionFiles tolerates extra Cargo.lock fields between package name and version', async () => {
  let tempDir;
  let srcTauriDir;
  try {
    ({ tempDir, srcTauriDir } = await createTempDesktopProject({
      cargoLockContents: `version = 4

[[package]]
name = "${DESKTOP_TAURI_CRATE_NAME}"
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
    }));

    await syncDesktopVersionFiles({ projectRoot: tempDir, version: '2.3.4' });

    const cargoLock = await readFile(path.join(srcTauriDir, 'Cargo.lock'), 'utf8');

    assert.match(
      cargoLock,
      new RegExp(
        String.raw`\[\[package\]\]\nname = "${DESKTOP_TAURI_CRATE_NAME}"\nsource = "path\+file:///workspace/src-tauri"\nversion = "2.3.4"`,
      ),
    );
    assert.match(
      cargoLock,
      /\[\[package\]\]\nname = "dep"\nversion = "9.9.9"\nsource = "registry\+https:\/\/github.com\/rust-lang\/crates.io-index"/,
    );
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
});

test('syncDesktopVersionFiles preserves trailing Cargo.lock comments and spaces on name/version lines', async () => {
  let tempDir;
  let srcTauriDir;
  try {
    ({ tempDir, srcTauriDir } = await createTempDesktopProject({
      cargoLockContents: `version = 4

[[package]]   # package header
name = "${DESKTOP_TAURI_CRATE_NAME}"   # desktop package
version = "0.1.0"   # keep this comment

[[package]]
name = "dep"
version = "9.9.9"
`,
    }));

    await syncDesktopVersionFiles({ projectRoot: tempDir, version: '2.3.4' });

    const cargoLock = await readFile(path.join(srcTauriDir, 'Cargo.lock'), 'utf8');

    assert.match(
      cargoLock,
      new RegExp(
        String.raw`\[\[package\]\]\s+# package header\nname = "${DESKTOP_TAURI_CRATE_NAME}"\s+# desktop package\nversion = "2.3.4"\s+# keep this comment`,
      ),
    );
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
});

test('syncDesktopVersionFiles rewrites Cargo.lock version lines using double quotes', async () => {
  let tempDir;
  let srcTauriDir;
  try {
    ({ tempDir, srcTauriDir } = await createTempDesktopProject({
      cargoLockContents: `version = 4

[[package]]
name = "${DESKTOP_TAURI_CRATE_NAME}"
version = '0.1.0'   # keep this comment
`,
    }));

    await syncDesktopVersionFiles({ projectRoot: tempDir, version: '2.3.4' });

    const cargoLock = await readFile(path.join(srcTauriDir, 'Cargo.lock'), 'utf8');

    assert.match(cargoLock, /version = "2.3.4"\s+# keep this comment/);
    assert.doesNotMatch(cargoLock, /version = '2\.3\.4'/);
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
});

test('syncDesktopVersionFiles only updates the version key, not similarly prefixed keys', async () => {
  let tempDir;
  let srcTauriDir;
  try {
    ({ tempDir, srcTauriDir } = await createTempDesktopProject({
      cargoLockContents: `version = 4

[[package]]
name = "${DESKTOP_TAURI_CRATE_NAME}"
versioned_dep = "keep-me"
version = "0.1.0"
`,
    }));

    await syncDesktopVersionFiles({ projectRoot: tempDir, version: '2.3.4' });

    const cargoLock = await readFile(path.join(srcTauriDir, 'Cargo.lock'), 'utf8');

    assert.match(cargoLock, /versioned_dep = "keep-me"/);
    assert.match(cargoLock, /version = "2.3.4"/);
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
});

test('syncDesktopVersionFiles skips Cargo.lock updates when the desktop crate is missing', async () => {
  let tempDir;
  let srcTauriDir;
  const warnings = [];
  const originalWarn = console.warn;
  console.warn = (message) => {
    warnings.push(String(message));
  };

  try {
    const originalCargoLock = `version = 4

[[package]]
name = "dep"
version = "9.9.9"
`;
    ({ tempDir, srcTauriDir } = await createTempDesktopProject({
      cargoLockContents: originalCargoLock,
    }));

    await syncDesktopVersionFiles({ projectRoot: tempDir, version: '2.3.4' });

    const packageJson = JSON.parse(await readFile(path.join(tempDir, 'package.json'), 'utf8'));
    const tauriConfig = JSON.parse(await readFile(path.join(srcTauriDir, 'tauri.conf.json'), 'utf8'));
    const cargoToml = await readFile(path.join(srcTauriDir, 'Cargo.toml'), 'utf8');
    const cargoLock = await readFile(path.join(srcTauriDir, 'Cargo.lock'), 'utf8');

    assert.equal(packageJson.version, '2.3.4');
    assert.equal(tauriConfig.version, '2.3.4');
    assert.match(cargoToml, /version\s*=\s*"2.3.4"/);
    assert.equal(cargoLock, originalCargoLock);
    assert.equal(warnings.length, 1);
    assert.match(warnings[0], new RegExp(`package "${DESKTOP_TAURI_CRATE_NAME}" not found`));
  } finally {
    console.warn = originalWarn;
    await rm(tempDir, { recursive: true, force: true });
  }
});

test('syncDesktopVersionFiles throws a specific error when the desktop crate version entry is malformed', async () => {
  let tempDir;
  let srcTauriDir;
  try {
    ({ tempDir, srcTauriDir } = await createTempDesktopProject({
      cargoLockContents: `version = 4

[[package]]
name = "${DESKTOP_TAURI_CRATE_NAME}"
source = "path+file:///workspace/src-tauri"
checksum = "unexpected-layout"
`,
    }));

    await assert.rejects(
      syncDesktopVersionFiles({ projectRoot: tempDir, version: '2.3.4' }),
      new RegExp(
        `version entry for package "${DESKTOP_TAURI_CRATE_NAME}" not found or has an unexpected layout`,
      ),
    );
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
});
