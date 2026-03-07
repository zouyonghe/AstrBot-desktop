import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { chmodSync } from 'node:fs';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, '..', '..');
const cleanupScript = path.join(projectRoot, 'scripts/ci/cleanup-release-assets.sh');

const createFakeGh = async (root, logFile) => {
  const binDir = path.join(root, 'bin');
  const ghPath = path.join(binDir, 'gh');
  await mkdir(binDir, { recursive: true });
  await writeFile(
    ghPath,
    `#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "${logFile}"
if [ "$1" = "api" ] && [ "$2" = "repos/test-owner/test-repo/releases/tags/nightly" ]; then
  printf '123\n'
  exit 0
fi
if [ "$1" = "api" ] && [ "$2" = "--paginate" ] && [ "$3" = "repos/test-owner/test-repo/releases/123/assets?per_page=100" ]; then
  printf '1\told-asset.zip\n'
  exit 0
fi
if [ "$1" = "api" ] && [ "$2" = "-X" ] && [ "$3" = "DELETE" ] && [ "$4" = "repos/test-owner/test-repo/releases/assets/1" ]; then
  exit 0
fi
printf 'unexpected gh args: %s\n' "$*" >&2
exit 1
`,
    'utf8',
  );
  chmodSync(ghPath, 0o755);
  return binDir;
};

const runCleanup = (envOverrides, cwd = projectRoot) =>
  spawnSync('bash', [cleanupScript], {
    cwd,
    encoding: 'utf8',
    env: {
      ...process.env,
      ...envOverrides,
    },
  });

test('cleanup-release-assets defaults to cleaning the current repository', async () => {
  const tempDir = await mkdtemp(path.join(os.tmpdir(), 'astrbot-cleanup-release-assets-'));

  try {
    const logFile = path.join(tempDir, 'gh.log');
    const binDir = await createFakeGh(tempDir, logFile);

    const result = runCleanup({
      GITHUB_REPOSITORY: 'test-owner/test-repo',
      RELEASE_TAG: 'nightly',
      PATH: `${binDir}:${process.env.PATH}`,
    });

    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /Deleted existing release asset/);

    const ghLog = await readFile(logFile, 'utf8');
    assert.match(ghLog, /repos\/test-owner\/test-repo\/releases\/tags\/nightly/);
    assert.match(ghLog, /repos\/test-owner\/test-repo\/releases\/assets\/1/);
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
});

test('cleanup-release-assets still skips when explicit target repository mismatches', async () => {
  const tempDir = await mkdtemp(path.join(os.tmpdir(), 'astrbot-cleanup-release-assets-'));

  try {
    const logFile = path.join(tempDir, 'gh.log');
    const binDir = await createFakeGh(tempDir, logFile);

    const result = runCleanup({
      GITHUB_REPOSITORY: 'test-owner/test-repo',
      RELEASE_TAG: 'nightly',
      ASTRBOT_RELEASE_CLEANUP_TARGET_REPOSITORY: 'AstrBotDevs/AstrBot-desktop',
      PATH: `${binDir}:${process.env.PATH}`,
    });

    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /Skipping release asset cleanup for non-target repository/);

    await assert.rejects(readFile(logFile, 'utf8'));
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
});
