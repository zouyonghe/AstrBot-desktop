import {
  normalizeDesktopVersionOverride,
} from './version-sync.mjs';
import {
  DEFAULT_ASTRBOT_SOURCE_GIT_URL,
  getSourceRefInfo,
  normalizeSourceRepoConfig,
  resolveSourceDir,
} from './source-repo.mjs';

const TRUTHY_ENV_VALUES = new Set(['1', 'true', 'yes', 'on']);

const trimEnv = (env, key, fallback = '') => {
  const value = env[key];
  return typeof value === 'string' ? value.trim() : fallback;
};

export const createPrepareResourcesContext = ({ argv, env, projectRoot, cwd = process.cwd() }) => {
  const sourceRepoUrlInput =
    trimEnv(env, 'ASTRBOT_SOURCE_GIT_URL') || DEFAULT_ASTRBOT_SOURCE_GIT_URL;
  const sourceRepoRefInput = trimEnv(env, 'ASTRBOT_SOURCE_GIT_REF');
  const sourceRepoRefCommitHint = trimEnv(env, 'ASTRBOT_SOURCE_GIT_REF_IS_COMMIT');
  const sourceDirOverride = trimEnv(env, 'ASTRBOT_SOURCE_DIR');
  const desktopVersionInput = trimEnv(env, 'ASTRBOT_DESKTOP_VERSION');
  const pythonBuildStandaloneRelease = trimEnv(env, 'ASTRBOT_PBS_RELEASE', '20260211');
  const pythonBuildStandaloneVersion = trimEnv(env, 'ASTRBOT_PBS_VERSION', '3.12.12');
  const mode = argv[2] || 'all';

  const desktopVersionOverride = normalizeDesktopVersionOverride(desktopVersionInput);
  const isDesktopBridgeExpectationStrict = TRUTHY_ENV_VALUES.has(
    trimEnv(env, 'ASTRBOT_DESKTOP_STRICT_BRIDGE_EXPECTATIONS').toLowerCase(),
  );

  const { repoUrl: sourceRepoUrl, repoRef: sourceRepoRefInputNormalized } = normalizeSourceRepoConfig(
    sourceRepoUrlInput,
    sourceRepoRefInput,
  );

  const {
    ref: sourceRepoRef,
    isCommit: isSourceRepoRefCommitSha,
    isVersionTag: isSourceRepoRefVersionTag,
  } = getSourceRefInfo(sourceRepoRefInputNormalized, sourceRepoRefCommitHint);

  const sourceDir = resolveSourceDir(projectRoot, sourceDirOverride, cwd);

  return {
    mode,
    pythonBuildStandaloneRelease,
    pythonBuildStandaloneVersion,
    desktopVersionInput,
    desktopVersionOverride,
    isDesktopBridgeExpectationStrict,
    sourceRepoUrl,
    sourceRepoRef,
    isSourceRepoRefCommitSha,
    isSourceRepoRefVersionTag,
    sourceDirOverrideInput: sourceDirOverride,
    sourceDir,
  };
};
