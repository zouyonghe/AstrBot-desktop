import { existsSync } from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import {
  getDesktopBridgeExpectations,
  shouldEnforceDesktopBridgeExpectation,
} from './desktop-bridge-expectations.mjs';

export const patchMonacoCssNestingWarnings = async ({ dashboardDir, projectRoot }) => {
  const patchRules = [
    {
      file: path.join(
        dashboardDir,
        'node_modules',
        'monaco-editor',
        'esm',
        'vs',
        'editor',
        'browser',
        'widget',
        'multiDiffEditor',
        'style.css',
      ),
      selector: 'a',
    },
    {
      file: path.join(
        dashboardDir,
        'node_modules',
        'monaco-editor',
        'esm',
        'vs',
        'editor',
        'contrib',
        'inlineEdits',
        'browser',
        'inlineEditsWidget.css',
      ),
      selector: 'svg',
    },
  ];

  for (const { file, selector } of patchRules) {
    if (!existsSync(file)) {
      continue;
    }
    const css = await readFile(file, 'utf8');
    const pattern = new RegExp(`^(\\s*)${selector}\\s*\\{`, 'm');
    if (!pattern.test(css)) {
      continue;
    }

    const patched = css.replace(pattern, `$1& ${selector} {`);
    if (patched !== css) {
      await writeFile(file, patched, 'utf8');
      console.log(
        `[prepare-resources] Patched Monaco nested selector "${selector}" in ${path.relative(projectRoot, file)}`,
      );
    }
  }
};

export const patchDesktopReleaseUpdateIndicator = async ({ dashboardDir, projectRoot }) => {
  const file = path.join(
    dashboardDir,
    'src',
    'layouts',
    'full',
    'vertical-header',
    'VerticalHeader.vue',
  );
  if (!existsSync(file)) {
    return;
  }

  const source = await readFile(file, 'utf8');
  if (source.includes('const backendHasNewVersion = !isDesktopReleaseMode.value && res.data.data.has_new_version;')) {
    return;
  }

  const target = "      hasNewVersion.value = res.data.data.has_new_version;\n\n      if (res.data.data.has_new_version) {";
  const replacement =
    "      const backendHasNewVersion = !isDesktopReleaseMode.value && res.data.data.has_new_version;\n" +
    "      hasNewVersion.value = backendHasNewVersion;\n\n" +
    "      if (backendHasNewVersion) {";

  if (!source.includes(target)) {
    return;
  }

  const patched = source.replace(target, replacement);
  if (patched !== source) {
    await writeFile(file, patched, 'utf8');
    console.log(
      `[prepare-resources] Patched desktop release update banner gating in ${path.relative(projectRoot, file)}`,
    );
  }
};

export const verifyDesktopBridgeArtifacts = async ({
  dashboardDir,
  projectRoot,
  sourceRepoRef,
  isSourceRepoRefVersionTag,
  isDesktopBridgeExpectationStrict,
}) => {
  const issues = [];
  const isTaggedRelease = isSourceRepoRefVersionTag;
  if (!isDesktopBridgeExpectationStrict && isTaggedRelease) {
    console.warn(
      `[prepare-resources] Desktop bridge required checks downgraded to warnings for source ref ${sourceRepoRef}. ` +
        'Set ASTRBOT_DESKTOP_STRICT_BRIDGE_EXPECTATIONS=1 to enforce.',
    );
  }

  for (const expectation of getDesktopBridgeExpectations()) {
    const mustPass = shouldEnforceDesktopBridgeExpectation(expectation, {
      isDesktopBridgeExpectationStrict,
      isTaggedRelease,
    });
    const file = path.join(dashboardDir, ...expectation.filePath);
    if (!existsSync(file)) {
      const relativePath = path.relative(projectRoot, file);
      const message = mustPass
        ? `[prepare-resources] Missing required file for ${expectation.label}: ${relativePath}`
        : `[prepare-resources] Missing optional (best-effort) file for ${expectation.label}: ${relativePath}`;
      if (mustPass) {
        issues.push(message);
      } else {
        console.warn(`${message} (compatibility check skipped)`);
      }
      continue;
    }

    const source = await readFile(file, 'utf8');
    if (!expectation.pattern.test(source)) {
      const message = `[prepare-resources] Expected ${expectation.label} in ${path.relative(projectRoot, file)}. ${expectation.hint || ''} Please sync AstrBot dashboard sources.`;
      if (mustPass) {
        issues.push(message);
      } else {
        console.warn(`${message} (compatibility check skipped)`);
      }
    }
  }

  if (issues.length > 0) {
    throw new Error(issues.join('\n'));
  }
};
