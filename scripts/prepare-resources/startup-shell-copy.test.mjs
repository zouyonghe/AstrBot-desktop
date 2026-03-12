import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const startupShellPath = new URL('../../ui/index.html', import.meta.url);
const startupCopyConfigPath = new URL('../../ui/startup-copy.js', import.meta.url);

test('startup shell loads shared copy config, reuses applyStartupMode, and exposes a focused live region', async () => {
  const source = await readFile(startupShellPath, 'utf8');
  const configSource = await readFile(startupCopyConfigPath, 'utf8').catch(() => '');

  assert.match(
    source,
    /<script src="\.\/startup-copy\.js"><\/script>/,
    'expected startup shell to load a dedicated startup copy config',
  );

  assert.match(
    source,
    /<span id="startup-status"[^>]*role="status"[^>]*aria-live="polite"[^>]*><\/span>/,
    'expected the status text element to be the polite live region',
  );
  assert.doesNotMatch(
    source,
    /aria-atomic="true"/,
    'expected startup shell to avoid verbose atomic live region announcements',
  );

  assert.doesNotMatch(
    source,
    /<h1 id="startup-title" class="title">[^<]+<\/h1>/,
    'expected startup title to be populated from STARTUP_COPY instead of duplicated static copy',
  );
  assert.doesNotMatch(
    source,
    /<p id="startup-desc" class="desc">[^<]+<\/p>/,
    'expected startup description to be populated from STARTUP_COPY instead of duplicated static copy',
  );
  assert.doesNotMatch(
    source,
    /<span id="startup-status">[^<]+<\/span>/,
    'expected startup status to be populated from STARTUP_COPY instead of duplicated static copy',
  );

  assert.doesNotMatch(
    source,
    /const\s+STARTUP_COPY\s*=/,
    'expected startup copy to be defined in a dedicated config instead of inline',
  );
  assert.match(
    source,
    /const\s+\{\s*STARTUP_MODES,\s*STARTUP_COPY\s*\}\s*=\s*window\.__astrbotStartupShell;/,
    'expected startup shell to read startup copy from the shared config',
  );
  assert.doesNotMatch(
    source,
    /const\s+initialCopy\s*=/,
    'expected initial render to reuse applyStartupMode instead of duplicating copy application',
  );
  assert.match(
    source,
    /applyStartupMode\(STARTUP_MODES\.LOADING\);/,
    'expected startup shell to initialize through applyStartupMode',
  );

  assert.match(
    configSource,
    /const\s+STARTUP_MODES\s*=\s*Object\.freeze\(/,
    'expected shared startup copy config to define startup modes',
  );
  assert.match(
    configSource,
    /const\s+STARTUP_COPY\s*=\s*Object\.freeze\(/,
    'expected shared startup copy config to define localized startup copy',
  );
  assert.match(
    configSource,
    /window\.__astrbotStartupShell\s*=\s*Object\.freeze\(/,
    'expected shared startup copy config to expose startup shell data on window',
  );
});
