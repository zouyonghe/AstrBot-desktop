import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmodSync } from 'node:fs';
import { copyFile, mkdir, mkdtemp, readFile, rm } from 'node:fs/promises';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, '..', '..');
const resolveScript = path.join(projectRoot, 'scripts/ci/resolve-build-context.sh');
const fakeGitFixture = path.join(scriptDir, 'fixtures', 'fake-git.sh');
const fakeCurlFixture = path.join(scriptDir, 'fixtures', 'fake-curl.sh');
const fakeVersionSortFixture = path.join(scriptDir, 'fixtures', 'fake-version-sort.py');

const DEFAULT_TAG_POLL_TAGS =
  '1111111111111111111111111111111111111111 refs/tags/v4.18.0|' +
  '2222222222222222222222222222222222222222 refs/tags/v4.19.0';

const baseEnv = {
  ASTRBOT_SOURCE_GIT_URL: 'https://example.com/AstrBot.git',
  ASTRBOT_SOURCE_GIT_REF: 'master',
  ASTRBOT_NIGHTLY_SOURCE_GIT_REF: 'master',
  GITHUB_EVENT_NAME: 'workflow_dispatch',
  GITHUB_TOKEN: 'test-token',
  GH_REPOSITORY: 'AstrBotDevs/AstrBot-desktop',
};

const makeTagPollEnv = (overrides = {}) => ({
  ...baseEnv,
  WORKFLOW_BUILD_MODE: 'tag-poll',
  WORKFLOW_PUBLISH_RELEASE: 'true',
  ASTRBOT_TEST_GIT_TAGS: DEFAULT_TAG_POLL_TAGS,
  ASTRBOT_TEST_NIGHTLY_REF: '3333333333333333333333333333333333333333',
  ASTRBOT_TEST_FETCHED_VERSION: '4.19.0',
  ASTRBOT_TEST_CURL_HTTP_STATUS: '404',
  ...overrides,
});

const makeNightlyEnv = (overrides = {}) => ({
  ...baseEnv,
  WORKFLOW_BUILD_MODE: 'nightly',
  WORKFLOW_PUBLISH_RELEASE: 'true',
  ASTRBOT_TEST_NIGHTLY_REF: '3333333333333333333333333333333333333333',
  ASTRBOT_TEST_FETCHED_VERSION: '4.19.0',
  ASTRBOT_TEST_CURL_HTTP_STATUS: '404',
  ...overrides,
});

const makeCustomEnv = (overrides = {}) => ({
  ...baseEnv,
  WORKFLOW_BUILD_MODE: 'custom',
  WORKFLOW_PUBLISH_RELEASE: 'true',
  ASTRBOT_TEST_FETCHED_VERSION: '4.19.0',
  ASTRBOT_TEST_FETCHED_SHA: '4444444444444444444444444444444444444444',
  ...overrides,
});

const parseGithubOutput = async (outputPath) => {
  const raw = await readFile(outputPath, 'utf8');
  const entries = raw
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => {
      const separatorIndex = line.indexOf('=');
      if (separatorIndex === -1) {
        return null;
      }
      return [line.slice(0, separatorIndex), line.slice(separatorIndex + 1)];
    })
    .filter(Boolean);
  return Object.fromEntries(entries);
};

const copyFixtureExecutable = async (fixturePath, filePath) => {
  await copyFile(fixturePath, filePath);
  chmodSync(filePath, 0o755);
};

const createFakeGit = async (binDir) => {
  const gitPath = path.join(binDir, 'git');
  await copyFixtureExecutable(fakeGitFixture, gitPath);
};

const createFakeCurl = async (binDir) => {
  const curlPath = path.join(binDir, 'curl');
  await copyFixtureExecutable(fakeCurlFixture, curlPath);
};

const createFakeSort = async (binDir) => {
  const sortPath = path.join(binDir, 'sort');
  await copyFixtureExecutable(fakeVersionSortFixture, sortPath);
};

const createFakeExecutables = async (root) => {
  const binDir = path.join(root, 'bin');
  await mkdir(binDir, { recursive: true });
  await Promise.all([createFakeGit(binDir), createFakeCurl(binDir), createFakeSort(binDir)]);
  return binDir;
};

const setupSandbox = async (env) => {
  const tempDir = await mkdtemp(path.join(os.tmpdir(), 'astrbot-resolve-build-context-'));
  const githubOutputPath = path.join(tempDir, 'github-output.txt');
  const binDir = await createFakeExecutables(tempDir);

  return {
    tempDir,
    githubOutputPath,
    env: {
      ...process.env,
      ...env,
      PATH: `${binDir}:${process.env.PATH}`,
      GITHUB_OUTPUT: githubOutputPath,
    },
  };
};

const runInSandbox = async (sandbox) => {
  const result = spawnSync('bash', [resolveScript], {
    cwd: projectRoot,
    encoding: 'utf8',
    env: sandbox.env,
  });
  const outputs = result.status === 0 ? await parseGithubOutput(sandbox.githubOutputPath) : {};
  return { result, outputs };
};

const withSandbox = async (env, fn) => {
  const sandbox = await setupSandbox(env);

  try {
    return await fn(sandbox);
  } finally {
    await rm(sandbox.tempDir, { recursive: true, force: true });
  }
};

const runResolveBuildContext = async (env) => withSandbox(env, runInSandbox);

test('workflow_dispatch tag-poll marks latest only when explicit source ref is the latest upstream tag', async () => {
  const { result, outputs } = await runResolveBuildContext(makeTagPollEnv({
    WORKFLOW_SOURCE_GIT_REF: 'v4.19.0',
  }));

  assert.equal(result.status, 0, result.stderr);
  assert.equal(outputs.source_git_ref, 'v4.19.0');
  assert.equal(outputs.release_tag, 'v4.19.0');
  assert.equal(outputs.release_make_latest, 'true');
});

test('workflow_dispatch tag-poll does not mark latest when explicit source ref is an older upstream tag', async () => {
  const { result, outputs } = await runResolveBuildContext(makeTagPollEnv({
    WORKFLOW_SOURCE_GIT_REF: 'v4.18.0',
  }));

  assert.equal(result.status, 0, result.stderr);
  assert.equal(outputs.source_git_ref, 'v4.18.0');
  assert.equal(outputs.release_tag, 'v4.18.0');
  assert.equal(outputs.release_make_latest, 'false');
});

test('workflow_dispatch tag-poll keeps explicit source ref builds running when tag lookup fails', async () => {
  const { result, outputs } = await runResolveBuildContext(makeTagPollEnv({
    WORKFLOW_SOURCE_GIT_REF: 'deadbeefdeadbeefdeadbeefdeadbeefdeadbeef',
    ASTRBOT_TEST_GIT_TAGS_FAIL: '1',
    ASTRBOT_TEST_FETCHED_VERSION: '4.19.7',
  }));

  assert.equal(result.status, 0, result.stderr);
  assert.equal(outputs.source_git_ref, 'deadbeefdeadbeefdeadbeefdeadbeefdeadbeef');
  assert.equal(outputs.astrbot_version, '4.19.7');
  assert.equal(outputs.release_make_latest, 'false');
});

test('workflow_dispatch tag-poll marks latest when no override is provided and latest upstream tag is selected', async () => {
  const { result, outputs } = await runResolveBuildContext(makeTagPollEnv());

  assert.equal(result.status, 0, result.stderr);
  assert.equal(outputs.source_git_ref, 'v4.19.0');
  assert.equal(outputs.release_tag, 'v4.19.0');
  assert.equal(outputs.release_make_latest, 'true');
});

test('workflow_dispatch tag-poll normalizes annotated latest tags before latest comparison', async () => {
  const { result, outputs } = await runResolveBuildContext(makeTagPollEnv({
    WORKFLOW_SOURCE_GIT_REF: 'v4.19.0',
    ASTRBOT_TEST_GIT_TAGS:
      '1111111111111111111111111111111111111111 refs/tags/v4.18.0|' +
      'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa refs/tags/v4.19.0|' +
      'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb refs/tags/v4.19.0^{}',
  }));

  assert.equal(result.status, 0, result.stderr);
  assert.equal(outputs.source_git_ref, 'v4.19.0');
  assert.equal(outputs.release_make_latest, 'true');
});

test('workflow_dispatch nightly never marks latest', async () => {
  const { result, outputs } = await runResolveBuildContext(makeNightlyEnv());

  assert.equal(result.status, 0, result.stderr);
  assert.equal(outputs.build_mode, 'nightly');
  assert.equal(outputs.release_tag, 'nightly');
  assert.equal(outputs.release_prerelease, 'true');
  assert.equal(outputs.release_make_latest, 'false');
});

test('workflow_dispatch custom resolves explicit source ref to a pinned commit SHA', async () => {
  const { result, outputs } = await runResolveBuildContext(makeCustomEnv({
    WORKFLOW_SOURCE_GIT_REF: 'fix/windows-packaged-pip-build-env',
  }));

  assert.equal(result.status, 0, result.stderr);
  assert.equal(outputs.build_mode, 'custom');
  assert.equal(outputs.source_git_ref, '4444444444444444444444444444444444444444');
  assert.match(
    outputs.astrbot_version,
    /^4\.19\.0-custom\.\d{8}\.44444444$/,
  );
  assert.equal(outputs.release_prerelease, 'true');
  assert.equal(outputs.release_make_latest, 'false');
  assert.match(outputs.release_tag, /^custom-\d{8}-44444444$/);
});

test('workflow_dispatch custom requires an explicit source ref', async () => {
  const { result } = await runResolveBuildContext(makeCustomEnv({
    WORKFLOW_SOURCE_GIT_REF: '',
  }));

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /workflow_dispatch custom mode requires source_git_ref/i);
});
