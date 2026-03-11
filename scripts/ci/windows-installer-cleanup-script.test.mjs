import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const scriptPath = new URL('../../src-tauri/windows/kill-backend-processes.ps1', import.meta.url);
const hookPath = new URL('../../src-tauri/windows/nsis-installer-hooks.nsh', import.meta.url);

function extractNsisMacroBody(source, macroName) {
  const lines = source.split('\n');
  const startMarker = `!macro ${macroName}`;
  const startIdx = lines.findIndex((line) => line.trim() === startMarker);

  assert.notEqual(startIdx, -1, `Expected NSIS macro ${macroName} to exist`);

  const endIdx = lines.findIndex((line, index) => index > startIdx && line.trim() === '!macroend');

  assert.notEqual(endIdx, -1, `Expected end of NSIS macro ${macroName}`);
  return lines.slice(startIdx + 1, endIdx).map((line) => line.trim());
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
  const primaryIdx = macroBody.indexOf('StrCpy $1 "${ASTRBOT_BACKEND_CLEANUP_SCRIPT_INSTALL_ROOT}"');
  const fileExistsIdx = macroBody.indexOf('IfFileExists "$1" +2 0');
  const fallbackIdx = macroBody.indexOf('StrCpy $1 "${ASTRBOT_BACKEND_CLEANUP_SCRIPT_UPDATER_FALLBACK}"');

  assert.match(
    source,
    /!define\s+ASTRBOT_BACKEND_CLEANUP_SCRIPT_INSTALL_ROOT\s+"\$INSTDIR\\kill-backend-processes\.ps1"/
  );
  assert.match(
    source,
    /!define\s+ASTRBOT_BACKEND_CLEANUP_SCRIPT_UPDATER_FALLBACK\s+"\$INSTDIR\\_up_\\resources\\kill-backend-processes\.ps1"/
  );
  assert.notEqual(primaryIdx, -1);
  assert.notEqual(fileExistsIdx, -1);
  assert.notEqual(fallbackIdx, -1);
  assert.ok(primaryIdx < fileExistsIdx && fileExistsIdx < fallbackIdx);
  assert.ok(macroBody.some((line) => line.startsWith("nsExec::ExecToLog '")));
});
