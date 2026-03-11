import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const scriptPath = new URL('../../src-tauri/windows/kill-backend-processes.ps1', import.meta.url);
const hookPath = new URL('../../src-tauri/windows/nsis-installer-hooks.nsh', import.meta.url);

function extractNsisMacroBody(source, macroName) {
  const lines = source.split('\n');
  const startMarker = `!macro ${macroName}`;
  const startIdx = lines.findIndex((line) => {
    const normalizedLine = line.trimStart();
    return (
      normalizedLine.startsWith(startMarker) &&
      (normalizedLine.length === startMarker.length || /\s/.test(normalizedLine[startMarker.length]))
    );
  });

  assert.notEqual(startIdx, -1, `Expected NSIS macro ${macroName} to exist`);

  const endIdx = lines.findIndex((line, index) => {
    if (index <= startIdx) return false;

    return line.trim().toLowerCase().startsWith('!macroend');
  });

  assert.notEqual(endIdx, -1, `Expected end of NSIS macro ${macroName}`);
  return lines.slice(startIdx + 1, endIdx).map((line) => line.trim());
}

function findMatchingLineIndex(lines, pattern) {
  return lines.findIndex((line) => pattern.test(line));
}

function parseNsisDefines(source) {
  const defines = new Map();

  for (const line of source.split('\n')) {
    const trimmedLine = line.trim();
    const match = trimmedLine.match(/^!define\s+(\S+)\s+(?:"([^"]+)"|'([^']+)'|(\S+))/);

    if (trimmedLine.startsWith('!define ') && !match) {
      assert.fail(`Expected NSIS define line to be parseable: ${trimmedLine}`);
    }

    if (match) {
      defines.set(match[1], match[2] ?? match[3] ?? match[4]);
    }
  }

  return defines;
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
  const defines = parseNsisDefines(source);
  const primaryIdx = findMatchingLineIndex(
    macroBody,
    /StrCpy\s+\$1\s+"\$\{ASTRBOT_BACKEND_CLEANUP_SCRIPT_INSTALL_ROOT\}"/
  );
  const fileExistsIdx = findMatchingLineIndex(macroBody, /IfFileExists\s+"\$1"\s+\+2\s+0/);
  const fallbackIdx = findMatchingLineIndex(
    macroBody,
    /StrCpy\s+\$1\s+"\$\{ASTRBOT_BACKEND_CLEANUP_SCRIPT_UPDATER_FALLBACK\}"/
  );

  assert.equal(defines.get('ASTRBOT_BACKEND_CLEANUP_SCRIPT_INSTALL_ROOT'), '$INSTDIR\\kill-backend-processes.ps1');
  assert.equal(
    defines.get('ASTRBOT_BACKEND_CLEANUP_SCRIPT_UPDATER_FALLBACK'),
    '$INSTDIR\\_up_\\resources\\kill-backend-processes.ps1'
  );
  assert.notEqual(primaryIdx, -1);
  assert.notEqual(fileExistsIdx, -1);
  assert.notEqual(fallbackIdx, -1);
  assert.ok(primaryIdx < fileExistsIdx && fileExistsIdx < fallbackIdx);
  assert.ok(macroBody.some((line) => /nsExec::ExecToLog/.test(line)));
});
