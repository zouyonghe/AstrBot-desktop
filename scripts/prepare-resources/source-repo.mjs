import { existsSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

export const DEFAULT_ASTRBOT_SOURCE_GIT_URL = 'https://github.com/AstrBotDevs/AstrBot.git';

const SOURCE_REPO_REF_COMMIT_HINT_TRUTHY = new Set(['1', 'true', 'yes', 'on']);
// Accept full SHAs and longer abbreviated SHAs (>= 12) to reduce false positives
// from hex-looking branch/tag names while still supporting common CI short refs.
const GIT_COMMIT_SHA_PATTERN = /^[0-9a-f]{12,64}$/i;
// Treat both `v1.2.3` and `1.2.3` style refs as release tags.
const VERSION_TAG_REF_PATTERN = /^v?\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;

export const normalizeSourceRepoConfig = (sourceRepoUrlRaw, sourceRepoRefRaw) => {
  if (!sourceRepoUrlRaw) {
    return { repoUrl: '', repoRef: sourceRepoRefRaw };
  }

  const treeMatch =
    /^https?:\/\/github\.com\/([^/]+\/[^/]+)\/tree\/([^/]+)\/?$/.exec(sourceRepoUrlRaw) ||
    /^https?:\/\/github\.com\/([^/]+\/[^/]+)\/tree\/([^/]+)\/.+$/.exec(sourceRepoUrlRaw);
  if (treeMatch) {
    const repoPath = treeMatch[1];
    const refFromUrl = treeMatch[2];
    return {
      repoUrl: `https://github.com/${repoPath}.git`,
      repoRef: sourceRepoRefRaw || refFromUrl,
    };
  }

  return {
    repoUrl: sourceRepoUrlRaw,
    repoRef: sourceRepoRefRaw,
  };
};

export const getSourceRefInfo = (resolvedRef, sourceRepoRefIsCommitEnvRaw) => {
  const ref = typeof resolvedRef === 'string' ? resolvedRef.trim() : '';
  const commitHintRaw = String(sourceRepoRefIsCommitEnvRaw || '')
    .trim()
    .toLowerCase();
  const hasExplicitCommitHint = SOURCE_REPO_REF_COMMIT_HINT_TRUTHY.has(commitHintRaw);
  const isCommit = !!ref && (GIT_COMMIT_SHA_PATTERN.test(ref) || hasExplicitCommitHint);
  const isVersionTag = VERSION_TAG_REF_PATTERN.test(ref);

  return { ref, isCommit, isVersionTag };
};

export const resolveSourceDir = (projectRoot, sourceDirOverrideRaw, cwd = process.cwd()) => {
  if (sourceDirOverrideRaw) {
    return path.resolve(cwd, sourceDirOverrideRaw);
  }
  return path.join(projectRoot, 'vendor', 'AstrBot');
};

export const ensureSourceRepo = ({
  sourceDir,
  sourceRepoUrl,
  sourceRepoRef,
  isSourceRepoRefCommitSha,
  sourceDirOverrideRaw,
}) => {
  if (sourceDirOverrideRaw) {
    if (!existsSync(path.join(sourceDir, 'main.py'))) {
      throw new Error(`ASTRBOT_SOURCE_DIR is set but invalid: ${sourceDir}. Cannot find main.py.`);
    }
    return;
  }

  if (!existsSync(path.join(sourceDir, '.git'))) {
    mkdirSync(path.dirname(sourceDir), { recursive: true });
    const cloneArgs = ['clone', '--depth', '1'];
    if (sourceRepoRef && !isSourceRepoRefCommitSha) {
      cloneArgs.push('--branch', sourceRepoRef);
    }
    cloneArgs.push(sourceRepoUrl, sourceDir);

    const cloneResult = spawnSync('git', cloneArgs, {
      stdio: 'inherit',
    });
    if (cloneResult.status !== 0) {
      throw new Error(`Failed to clone AstrBot from ${sourceRepoUrl}`);
    }
  } else {
    const setUrlResult = spawnSync(
      'git',
      ['-C', sourceDir, 'remote', 'set-url', 'origin', sourceRepoUrl],
      { stdio: 'inherit' },
    );
    if (setUrlResult.status !== 0) {
      throw new Error(`Failed to set origin url for ${sourceDir}`);
    }
  }

  if (sourceRepoRef) {
    const fetchResult = spawnSync(
      'git',
      ['-C', sourceDir, 'fetch', '--depth', '1', 'origin', sourceRepoRef],
      { stdio: 'inherit' },
    );
    if (fetchResult.status !== 0) {
      throw new Error(`Failed to fetch upstream ref ${sourceRepoRef}`);
    }

    const checkoutArgs = isSourceRepoRefCommitSha
      ? ['-C', sourceDir, 'checkout', '--detach', '-f', 'FETCH_HEAD']
      : ['-C', sourceDir, 'checkout', '-f', '-B', sourceRepoRef, 'FETCH_HEAD'];
    const checkoutResult = spawnSync('git', checkoutArgs, { stdio: 'inherit' });
    if (checkoutResult.status !== 0) {
      throw new Error(`Failed to checkout upstream ref ${sourceRepoRef}`);
    }
  }

  if (!existsSync(path.join(sourceDir, 'main.py'))) {
    throw new Error(`Resolved source repository is invalid: ${sourceDir}. Cannot find main.py.`);
  }
};
