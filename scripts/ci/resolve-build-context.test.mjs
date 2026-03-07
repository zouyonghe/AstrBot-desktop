import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmodSync } from 'node:fs';
import { mkdtemp, readFile, rm, writeFile, mkdir } from 'node:fs/promises';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, '..', '..');
const resolveScript = path.join(projectRoot, 'scripts/ci/resolve-build-context.sh');

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

const createFakeExecutables = async (root) => {
  const binDir = path.join(root, 'bin');
  await mkdir(binDir, { recursive: true });

  const gitPath = path.join(binDir, 'git');
  await writeFile(
    gitPath,
    `#!/usr/bin/env bash
set -euo pipefail

repo_dir=""
if [ "\${1-}" = "-C" ]; then
  repo_dir="\${2-}"
  shift 2
fi

command_name="\${1-}"
if [ -z "\${command_name}" ]; then
  echo "missing git command" >&2
  exit 1
fi
shift || true

case "\${command_name}" in
  ls-remote)
    source_ref="\${2-}"
    case "\${source_ref}" in
      refs/tags/*)
        IFS='|' read -r -a entries <<< "\${ASTRBOT_TEST_GIT_TAGS:-}"
        for entry in "\${entries[@]}"; do
          [ -n "\${entry}" ] || continue
          printf '%s\n' "\${entry}"
        done
        ;;
      refs/heads/*)
        printf '%s %s\n' "\${ASTRBOT_TEST_NIGHTLY_REF:-3333333333333333333333333333333333333333}" "\${source_ref}"
        ;;
      *)
        printf 'unexpected git ls-remote ref: %s\n' "\${source_ref}" >&2
        exit 1
        ;;
    esac
    ;;
  init)
    mkdir -p "\${1-}"
    ;;
  remote)
    if [ "\${1-}" != "add" ]; then
      printf 'unexpected git remote args: %s\n' "$*" >&2
      exit 1
    fi
    ;;
  fetch)
    if [ -z "\${repo_dir}" ]; then
      echo 'git fetch expected -C <repo_dir>' >&2
      exit 1
    fi
    cat > "\${repo_dir}/pyproject.toml" <<EOF
[project]
version = "\${ASTRBOT_TEST_FETCHED_VERSION:-4.19.0}"
EOF
    ;;
  checkout)
    ;;
  *)
    printf 'unexpected git command: %s %s\n' "\${command_name}" "$*" >&2
    exit 1
    ;;
esac
`,
    'utf8',
  );
  chmodSync(gitPath, 0o755);

  const curlPath = path.join(binDir, 'curl');
  await writeFile(
    curlPath,
    `#!/usr/bin/env bash
set -euo pipefail
printf '%s' "\${ASTRBOT_TEST_CURL_HTTP_STATUS:-404}"
`,
    'utf8',
  );
  chmodSync(curlPath, 0o755);

  const sortPath = path.join(binDir, 'sort');
  await writeFile(
    sortPath,
    `#!/usr/bin/env python3
import re
import sys

raw_lines = [line.rstrip("\\n") for line in sys.stdin]
unique = any("u" in arg.lstrip("-") for arg in sys.argv[1:])
lines = list(dict.fromkeys(raw_lines)) if unique else raw_lines

def version_key(value):
    return [int(part) if part.isdigit() else part for part in re.split(r"(\\d+)", value)]

for line in sorted(lines, key=version_key):
    print(line)
`,
    'utf8',
  );
  chmodSync(sortPath, 0o755);

  return binDir;
};

const runResolveBuildContext = async (envOverrides = {}) => {
  const tempDir = await mkdtemp(path.join(os.tmpdir(), 'astrbot-resolve-build-context-'));

  try {
    const githubOutputPath = path.join(tempDir, 'github-output.txt');
    const binDir = await createFakeExecutables(tempDir);
    const result = spawnSync('bash', [resolveScript], {
      cwd: projectRoot,
      encoding: 'utf8',
      env: {
        ...process.env,
        PATH: `${binDir}:${process.env.PATH}`,
        ASTRBOT_SOURCE_GIT_URL: 'https://example.com/AstrBot.git',
        ASTRBOT_SOURCE_GIT_REF: 'master',
        ASTRBOT_NIGHTLY_SOURCE_GIT_REF: 'master',
        WORKFLOW_BUILD_MODE: 'tag-poll',
        WORKFLOW_PUBLISH_RELEASE: 'true',
        GITHUB_EVENT_NAME: 'workflow_dispatch',
        GITHUB_TOKEN: 'test-token',
        GH_REPOSITORY: 'AstrBotDevs/AstrBot-desktop',
        GITHUB_OUTPUT: githubOutputPath,
        ASTRBOT_TEST_GIT_TAGS:
          '1111111111111111111111111111111111111111 refs/tags/v4.18.0|2222222222222222222222222222222222222222 refs/tags/v4.19.0',
        ASTRBOT_TEST_NIGHTLY_REF: '3333333333333333333333333333333333333333',
        ASTRBOT_TEST_FETCHED_VERSION: '4.19.0',
        ASTRBOT_TEST_CURL_HTTP_STATUS: '404',
        ...envOverrides,
      },
    });

    const outputs = result.status === 0 ? await parseGithubOutput(githubOutputPath) : {};
    return { result, outputs };
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
};

test('workflow_dispatch tag-poll marks latest only when explicit source ref is the latest upstream tag', async () => {
  const { result, outputs } = await runResolveBuildContext({
    WORKFLOW_SOURCE_GIT_REF: 'v4.19.0',
  });

  assert.equal(result.status, 0, result.stderr);
  assert.equal(outputs.source_git_ref, 'v4.19.0');
  assert.equal(outputs.release_tag, 'v4.19.0');
  assert.equal(outputs.release_make_latest, 'true');
});

test('workflow_dispatch tag-poll does not mark latest when explicit source ref is an older upstream tag', async () => {
  const { result, outputs } = await runResolveBuildContext({
    WORKFLOW_SOURCE_GIT_REF: 'v4.18.0',
  });

  assert.equal(result.status, 0, result.stderr);
  assert.equal(outputs.source_git_ref, 'v4.18.0');
  assert.equal(outputs.release_tag, 'v4.18.0');
  assert.equal(outputs.release_make_latest, 'false');
});

test('workflow_dispatch tag-poll marks latest when no override is provided and latest upstream tag is selected', async () => {
  const { result, outputs } = await runResolveBuildContext();

  assert.equal(result.status, 0, result.stderr);
  assert.equal(outputs.source_git_ref, 'v4.19.0');
  assert.equal(outputs.release_tag, 'v4.19.0');
  assert.equal(outputs.release_make_latest, 'true');
});

test('workflow_dispatch nightly never marks latest', async () => {
  const { result, outputs } = await runResolveBuildContext({
    WORKFLOW_BUILD_MODE: 'nightly',
    ASTRBOT_TEST_GIT_TAGS: '',
  });

  assert.equal(result.status, 0, result.stderr);
  assert.equal(outputs.build_mode, 'nightly');
  assert.equal(outputs.release_tag, 'nightly');
  assert.equal(outputs.release_prerelease, 'true');
  assert.equal(outputs.release_make_latest, 'false');
});
