import { mkdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  readAstrbotVersionFromPyproject,
  syncDesktopVersionFiles,
  validateAstrbotRuntimeVersion,
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

const resolveAstrbotVersionForSync = async ({
  needsSourceRepo,
  sourceDir,
  sourceRepoUrl,
  sourceRepoRef,
  isSourceRepoRefCommitSha,
  sourceDirOverrideInput,
  desktopVersionInput,
  desktopVersionOverride,
}) => {
  if (!needsSourceRepo) {
    console.log(
      '[prepare-resources] Skip source repo sync in version-only mode because ASTRBOT_DESKTOP_VERSION is set.',
    );
    return desktopVersionOverride;
  }

  ensureSourceRepo({
    sourceDir,
    sourceRepoUrl,
    sourceRepoRef,
    isSourceRepoRefCommitSha,
    sourceDirOverrideRaw: sourceDirOverrideInput,
  });

  const astrbotVersion =
    desktopVersionOverride || (await readAstrbotVersionFromPyproject({ sourceDir }));
  await validateAstrbotRuntimeVersion({
    sourceDir,
    expectedVersion: desktopVersionOverride ? undefined : astrbotVersion,
  });

  if (desktopVersionOverride) {
    const sourceVersion = await readAstrbotVersionFromPyproject({ sourceDir });
    if (sourceVersion !== desktopVersionOverride) {
      console.warn(
        `[prepare-resources] Version override drift detected: ASTRBOT_DESKTOP_VERSION=${desktopVersionInput} (normalized=${desktopVersionOverride}), source pyproject version=${sourceVersion} (${sourceDir})`,
      );
    }
  }

  return astrbotVersion;
};

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
  } = context;
  const needsSourceRepo = mode !== 'version' || !desktopVersionOverride;
  await mkdir(path.join(projectRoot, 'resources'), { recursive: true });

  if (desktopVersionInput && desktopVersionInput !== desktopVersionOverride) {
    console.log(
      `[prepare-resources] Normalized ASTRBOT_DESKTOP_VERSION from ${desktopVersionInput} to ${desktopVersionOverride}`,
    );
  }

  ensureStartupShellAssets(projectRoot);

  const astrbotVersion = await resolveAstrbotVersionForSync({
    needsSourceRepo,
    sourceDir,
    sourceRepoUrl,
    sourceRepoRef,
    isSourceRepoRefCommitSha,
    sourceDirOverrideInput,
    desktopVersionInput,
    desktopVersionOverride,
  });

  await syncDesktopVersionFiles({ projectRoot, version: astrbotVersion });
  if (desktopVersionOverride) {
    console.log(
      `[prepare-resources] Synced desktop version to override ${astrbotVersion} (ASTRBOT_DESKTOP_VERSION)`,
    );
  } else {
    console.log(`[prepare-resources] Synced desktop version to AstrBot ${astrbotVersion}`);
  }

  await runModeTasks(mode, context);
};

main().catch((error) => {
  if (error instanceof Error) {
    console.error(error.stack || error.message);
  } else {
    console.error(String(error));
  }
  process.exit(1);
});
