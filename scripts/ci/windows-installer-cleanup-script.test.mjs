import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const scriptPath = new URL('../../src-tauri/windows/kill-backend-processes.ps1', import.meta.url);
const hookPath = new URL('../../src-tauri/windows/nsis-installer-hooks.nsh', import.meta.url);

test('windows cleanup script emits diagnostic logging for install root and process termination', async () => {
  const source = await readFile(scriptPath, 'utf8');

  assert.match(source, /Write-Output\s+"\[astrbot-installer\]\s+install root:/);
  assert.match(source, /Write-Output\s+"\[astrbot-installer\]\s+matched process:/);
  assert.match(source, /Write-Output\s+"\[astrbot-installer\]\s+stopping process:/);
});

test('windows cleanup script only matches processes under the provided install root', async () => {
  const source = await readFile(scriptPath, 'utf8');

  assert.match(source, /return \$normalized -ieq \$installRoot -or \$normalized\.StartsWith\(\$installRootWithSep/);
  assert.doesNotMatch(source, /ExpandEnvironmentVariables\('%LOCALAPPDATA%'\)/);
});

test('nsis installer hook looks for the install-root cleanup script before updater fallback', async () => {
  const source = await readFile(hookPath, 'utf8');

  assert.doesNotMatch(source, /\$UpdateMode = 1/);
  assert.match(source, /StrCpy \$1 "\$INSTDIR\\kill-backend-processes\.ps1"/);
  assert.match(source, /StrCpy \$1 "\$INSTDIR\\_up_\\resources\\kill-backend-processes\.ps1"/);
  assert.doesNotMatch(source, /StrCpy \$1 "\$INSTDIR\\resources\\kill-backend-processes\.ps1"/);
});
