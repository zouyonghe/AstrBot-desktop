import { mkdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  readAstrbotVersionFromPyproject,
  syncDesktopVersionFiles,
} from './prepare-resources/version-sync.mjs';
import {
  ensureSourceRepo,
} from './prepare-resources/source-repo.mjs';
import {
  ensureStartupShellAssets,
} from './prepare-resources/mode-tasks.mjs';
import { runModeTasks } from './prepare-resources/mode-dispatch.mjs';
import { createPrepareResourcesContext } from './prepare-resources/context.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '..');

const main = async () => {
  const context = createPrepareResourcesContext({
    argv: process.argv,
    env: process.env,
    projectRoot,
  });
  const {
    mode,
    sourceDir,
    sourceRepoUrl,
    sourceRepoRef,
    isSourceRepoRefCommitSha,
    sourceDirOverrideInput,
    desktopVersionInput,
    desktopVersionOverride,
    isSourceRepoRefVersionTag,
    isDesktopBridgeExpectationStrict,
    pythonBuildStandaloneRelease,
    pythonBuildStandaloneVersion,
  } = context;
  const needsSourceRepo = mode !== 'version' || !desktopVersionOverride;
  await mkdir(path.join(projectRoot, 'resources'), { recursive: true });

  if (desktopVersionInput && desktopVersionInput !== desktopVersionOverride) {
    console.log(
      `[prepare-resources] Normalized ASTRBOT_DESKTOP_VERSION from ${desktopVersionInput} to ${desktopVersionOverride}`,
    );
  }

  if (needsSourceRepo) {
    ensureSourceRepo({
      sourceDir,
      sourceRepoUrl,
      sourceRepoRef,
      isSourceRepoRefCommitSha,
      sourceDirOverrideRaw: sourceDirOverrideInput,
    });
  } else {
    console.log(
      '[prepare-resources] Skip source repo sync in version-only mode because ASTRBOT_DESKTOP_VERSION is set.',
    );
  }

  ensureStartupShellAssets(projectRoot);
  const astrbotVersion =
    desktopVersionOverride || (await readAstrbotVersionFromPyproject({ sourceDir }));

  if (desktopVersionOverride && needsSourceRepo) {
    const sourceVersion = await readAstrbotVersionFromPyproject({ sourceDir });
    if (sourceVersion !== desktopVersionOverride) {
      console.warn(
        `[prepare-resources] Version override drift detected: ASTRBOT_DESKTOP_VERSION=${desktopVersionInput} (normalized=${desktopVersionOverride}), source pyproject version=${sourceVersion} (${sourceDir})`,
      );
    }
  }

  await syncDesktopVersionFiles({ projectRoot, version: astrbotVersion });
  if (desktopVersionOverride) {
    console.log(
      `[prepare-resources] Synced desktop version to override ${astrbotVersion} (ASTRBOT_DESKTOP_VERSION)`,
    );
  } else {
    console.log(`[prepare-resources] Synced desktop version to AstrBot ${astrbotVersion}`);
  }

  await runModeTasks(mode, {
    sourceDir,
    projectRoot,
    sourceRepoRef,
    isSourceRepoRefVersionTag,
    isDesktopBridgeExpectationStrict,
    pythonBuildStandaloneRelease,
    pythonBuildStandaloneVersion,
  });
};

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
