import { prepareBackend, prepareWebui } from './mode-tasks.mjs';

const VALID_MODES = new Set(['version', 'webui', 'backend', 'all']);

const defaultTaskRunner = {
  prepareWebui,
  prepareBackend,
};

export const runModeTasks = async (
  mode,
  {
    sourceDir,
    projectRoot,
    sourceRepoRef,
    isSourceRepoRefVersionTag,
    isDesktopBridgeExpectationStrict,
    pythonBuildStandaloneRelease,
    pythonBuildStandaloneVersion,
  },
  taskRunner = defaultTaskRunner,
) => {
  if (!VALID_MODES.has(mode)) {
    throw new Error(`Unsupported mode: ${mode}. Expected version/webui/backend/all.`);
  }

  if (mode === 'version') {
    return;
  }

  if (mode === 'webui' || mode === 'all') {
    await taskRunner.prepareWebui({
      sourceDir,
      projectRoot,
      sourceRepoRef,
      isSourceRepoRefVersionTag,
      isDesktopBridgeExpectationStrict,
    });
  }

  if (mode === 'backend' || mode === 'all') {
    await taskRunner.prepareBackend({
      sourceDir,
      projectRoot,
      pythonBuildStandaloneRelease,
      pythonBuildStandaloneVersion,
    });
  }
};
