import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const scriptPath = new URL('../../src-tauri/windows/kill-backend-processes.ps1', import.meta.url);
const hookPath = new URL('../../src-tauri/windows/nsis-installer-hooks.nsh', import.meta.url);
const nsisPrimaryCleanupDefine = 'ASTRBOT_BACKEND_CLEANUP_SCRIPT_INSTALL_ROOT';
const nsisFallbackCleanupDefine = 'ASTRBOT_BACKEND_CLEANUP_SCRIPT_UPDATER_FALLBACK';
const installRootCleanupPath = '$INSTDIR\\kill-backend-processes.ps1';
const updaterFallbackCleanupPath = '$INSTDIR\\_up_\\resources\\kill-backend-processes.ps1';

function escapeRegex(value) {
  return value.replaceAll(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function extractNsisMacroBody(source, macroName) {
  const match = source.match(new RegExp(`!macro\\s+${escapeRegex(macroName)}([\\s\\S]*?)!macroend`));
  assert.ok(match, `Expected NSIS macro ${macroName} to exist`);
  return match[1];
}

test('windows cleanup script emits diagnostic logging for install root and process termination', async () => {
  const source = await readFile(scriptPath, 'utf8');

  assert.match(source, /Write-Output\s+"\[astrbot-installer\]\s+install root:/);
  assert.match(source, /Write-Output\s+"\[astrbot-installer\]\s+matched process:/);
  assert.match(source, /Write-Output\s+"\[astrbot-installer\]\s+stopping process:/);
});

test('windows cleanup script only matches processes under the provided install root', async () => {
  const source = await readFile(scriptPath, 'utf8');

  assert.match(source, /function Test-IsUnderInstallRoot/);
  assert.match(source, /\$normalized -ieq \$installRoot/);
  assert.match(source, /\$normalized\.StartsWith\(\$installRootWithSep/);
});

test('nsis installer hook looks for the install-root cleanup script before updater fallback', async () => {
  const source = await readFile(hookPath, 'utf8');
  const macroBody = extractNsisMacroBody(source, 'NSIS_RUN_BACKEND_CLEANUP');

  assert.match(
    source,
    new RegExp(
      `!define\\s+${escapeRegex(nsisPrimaryCleanupDefine)}\\s+"${escapeRegex(installRootCleanupPath)}"`
    )
  );
  assert.match(
    source,
    new RegExp(
      `!define\\s+${escapeRegex(nsisFallbackCleanupDefine)}\\s+"${escapeRegex(updaterFallbackCleanupPath)}"`
    )
  );
  assert.match(
    macroBody,
    new RegExp(
      `StrCpy\\s+\\$1\\s+"\\$\\{${escapeRegex(nsisPrimaryCleanupDefine)}\\}"[\\s\\S]*?` +
        `IfFileExists\\s+"\\$1"\\s+\\+2\\s+0[\\s\\S]*?` +
        `StrCpy\\s+\\$1\\s+"\\$\\{${escapeRegex(nsisFallbackCleanupDefine)}\\}"`
    )
  );
  assert.match(macroBody, /nsExec::ExecToLog\s+'/);
});
