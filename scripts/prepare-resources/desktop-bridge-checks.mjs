import { existsSync } from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const DESKTOP_BRIDGE_PATTERNS = {
  trayRestartGuard: /if\s*\(\s*!desktopBridge\s*\?\.\s*onTrayRestartBackend\s*\)\s*\{/,
  trayRestartPromptInvoke:
    /await\s+globalWaitingRef\s*\.\s*value\s*\?\.\s*check\s*\?\.\s*\(\s*[^)]*\s*\)\s*;?/,
  desktopRuntimeImport:
    /import\s+\{\s*getDesktopRuntimeInfo\s*\}\s+from\s+['"]@\/utils\/desktopRuntime['"]\s*;?/,
  desktopRuntimeUsageInRestart:
    /hasDesktopRestartCapability[\s\S]*?await\s+getDesktopRuntimeInfo\s*\(\s*\)/,
  desktopRuntimeUsageInHeader:
    /const\s+runtimeInfo\s*=\s*await\s+getDesktopRuntimeInfo\s*\(\s*\)\s*;?[\s\S]*?isDesktopReleaseMode\.value\s*=\s*runtimeInfo\.isDesktopRuntime/,
  desktopReleaseModeFlag: /\bisDesktopReleaseMode\b/,
  desktopRuntimeProbeWarn: /console\.warn\([\s\S]*desktop runtime/i,
};

const DESKTOP_BRIDGE_EXPECTATIONS = [
  {
    filePath: ['src', 'App.vue'],
    pattern: DESKTOP_BRIDGE_PATTERNS.trayRestartGuard,
    label: 'tray restart desktop guard',
    hint: "Expected `if (!desktopBridge?.onTrayRestartBackend) {` in App.vue.",
    required: false,
  },
  {
    filePath: ['src', 'App.vue'],
    pattern: DESKTOP_BRIDGE_PATTERNS.trayRestartPromptInvoke,
    label: 'tray restart waiting prompt',
    hint: 'Expected tray callback to call `globalWaitingRef.value?.check?.(...)`.',
    required: false,
  },
  {
    filePath: ['src', 'utils', 'restartAstrBot.ts'],
    pattern: DESKTOP_BRIDGE_PATTERNS.desktopRuntimeImport,
    label: 'desktop runtime helper import',
    hint: 'Expected `import { getDesktopRuntimeInfo } from "@/utils/desktopRuntime"`.',
    required: true,
  },
  {
    filePath: ['src', 'utils', 'restartAstrBot.ts'],
    pattern: DESKTOP_BRIDGE_PATTERNS.desktopRuntimeUsageInRestart,
    label: 'desktop runtime helper usage in restart flow',
    hint: 'Expected restart flow to use `hasDesktopRestartCapability` + `await getDesktopRuntimeInfo()`.',
    required: true,
  },
  {
    filePath: ['src', 'layouts', 'full', 'vertical-header', 'VerticalHeader.vue'],
    pattern: DESKTOP_BRIDGE_PATTERNS.desktopReleaseModeFlag,
    label: 'desktop release mode flag',
    hint: 'Expected `isDesktopReleaseMode` flag in header update UI.',
    required: false,
  },
  {
    filePath: ['src', 'layouts', 'full', 'vertical-header', 'VerticalHeader.vue'],
    pattern: DESKTOP_BRIDGE_PATTERNS.desktopRuntimeUsageInHeader,
    label: 'desktop runtime helper usage in header',
    hint: 'Expected header runtime probe: `const runtimeInfo = await getDesktopRuntimeInfo()`.',
    required: true,
  },
  {
    filePath: ['src', 'utils', 'desktopRuntime.ts'],
    pattern: DESKTOP_BRIDGE_PATTERNS.desktopRuntimeProbeWarn,
    label: 'desktop runtime probe warning',
    hint: 'Expected warning log when desktop runtime detection fails.',
    required: false,
  },
];

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

export const patchDesktopReleaseRedirectBehavior = async ({
  dashboardDir,
  projectRoot,
  strictPatternMatch = false,
}) => {
  const headerFile = path.join(
    dashboardDir,
    'src',
    'layouts',
    'full',
    'vertical-header',
    'VerticalHeader.vue',
  );
  if (!existsSync(headerFile)) {
    return;
  }

  const source = await readFile(headerFile, 'utf8');
  let patched = source;
  const reportPatternMiss = (description) => {
    const message =
      `[prepare-resources] Failed to patch VerticalHeader.vue: pattern not found for ${description}`;
    if (strictPatternMatch) {
      throw new Error(message);
    }
    console.warn(`${message} (compatibility patch skipped)`);
  };

  const desktopRedirectSnippet = `pendingRedirectUrl.value = getReleaseUrlForDesktop();
    resolvingReleaseTarget.value = false;
    requestExternalRedirect(pendingRedirectUrl.value);
    return;`;

  const openFunctionReplacement = `const open = (link: string) => {
  if (!link) return;
  const bridgeOpenExternalUrl = (window as any).astrbotDesktop?.openExternalUrl;
  if (typeof bridgeOpenExternalUrl === 'function') {
    void bridgeOpenExternalUrl(link);
    return;
  }
  const opened = window.open(link, '_blank', 'noopener,noreferrer');
  if (!opened) {
    window.location.assign(link);
  }
};`;

  // 1) open() implementation
  if (/const open = \(link: string\) => \{[\s\S]*?\n\};/m.test(patched)) {
    patched = patched.replace(
      /const open = \(link: string\) => \{[\s\S]*?\n\};/m,
      openFunctionReplacement,
    );
  } else if (/bridgeOpenExternalUrl/.test(patched)) {
    // already patched with desktop bridge support
  } else {
    reportPatternMiss('open() implementation');
  }

  // 2) handleUpdateClick desktop redirect flow
  if (
    /function handleUpdateClick\(\)\s*\{[\s\S]*?\n\}(?=\n\n\/\/ 检测是否为预发布版本)/m.test(
      patched,
    )
  ) {
    patched = patched.replace(
      /function handleUpdateClick\(\)\s*\{[\s\S]*?\n\}(?=\n\n\/\/ 检测是否为预发布版本)/m,
      `function handleUpdateClick() {
  if (isDesktopReleaseMode.value) {
    ${desktopRedirectSnippet}
  }
  checkUpdate();
  getReleases();
  updateStatusDialog.value = true;
}`,
    );
  } else if (/pendingRedirectUrl\.value = getReleaseUrlForDesktop\(\);/.test(patched)) {
    // already patched with desktop redirect fallback
  } else {
    reportPatternMiss('handleUpdateClick desktop redirect flow');
  }

  if (patched !== source) {
    await writeFile(headerFile, patched, 'utf8');
    console.log(
      `[prepare-resources] Patched desktop release redirect behavior in ${path.relative(projectRoot, headerFile)}`,
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

  const shouldEnforceDesktopBridgeExpectation = (expectation) => {
    if (isDesktopBridgeExpectationStrict) {
      return true;
    }
    return expectation.required && !isTaggedRelease;
  };

  for (const expectation of DESKTOP_BRIDGE_EXPECTATIONS) {
    const mustPass = shouldEnforceDesktopBridgeExpectation(expectation);
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
