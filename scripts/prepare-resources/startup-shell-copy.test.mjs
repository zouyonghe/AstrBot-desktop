import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const startupShellPath = new URL('../../ui/index.html', import.meta.url);

test('startup shell initializes copy from STARTUP_COPY and exposes polite status updates', async () => {
  const source = await readFile(startupShellPath, 'utf8');

  assert.match(
    source,
    /<div class="status"[^>]*aria-live="polite"[^>]*>/,
    'expected startup status container to announce status changes politely',
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

  assert.match(
    source,
    /const\s+initialCopy\s*=\s*resolveStartupCopy\(STARTUP_MODES\.LOADING\);/,
    'expected the initial render to be sourced from STARTUP_COPY',
  );
  assert.match(
    source,
    /title\.textContent\s*=\s*initialCopy\.title;/,
    'expected title initial text to be assigned from initialCopy',
  );
  assert.match(
    source,
    /desc\.textContent\s*=\s*initialCopy\.desc;/,
    'expected description initial text to be assigned from initialCopy',
  );
  assert.match(
    source,
    /status\.textContent\s*=\s*initialCopy\.status;/,
    'expected status initial text to be assigned from initialCopy',
  );
});
