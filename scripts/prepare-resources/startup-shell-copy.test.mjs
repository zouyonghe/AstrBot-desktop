import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import vm from 'node:vm';

const startupShellPath = new URL('../../ui/index.html', import.meta.url);
const startupCopyConfigPath = new URL('../../ui/startup-copy.js', import.meta.url);
const startupTaskPath = new URL('../../src-tauri/src/startup_task.rs', import.meta.url);

const STARTUP_ELEMENT_IDS = [
  'startup-title',
  'startup-desc',
  'startup-status',
  'startup-summary-label',
  'startup-summary-text',
  'startup-diagnostics-toggle',
  'startup-diagnostics-toggle-text',
  'startup-diagnostics',
  'startup-stage-list',
  'startup-desktop-log-label',
  'startup-desktop-log-lines',
  'startup-backend-log-label',
  'startup-backend-log-lines',
];

class FakeElement {
  constructor(id) {
    this.id = id;
    this.textContent = '';
    this.title = '';
    this.hidden = false;
    this.className = '';
    this.dataset = {};
    this.children = [];
    this.attributes = new Map();
    this.listeners = new Map();
  }

  append(child) {
    this.children.push(child);
  }

  replaceChildren(...children) {
    this.children = children;
  }

  setAttribute(name, value) {
    this.attributes.set(name, String(value));
  }

  getAttribute(name) {
    return this.attributes.get(name);
  }

  addEventListener(type, listener) {
    const listeners = this.listeners.get(type) || [];
    listeners.push(listener);
    this.listeners.set(type, listeners);
  }
}

class FakeDocument {
  constructor(ids) {
    this.elements = new Map(ids.map((id) => [id, new FakeElement(id)]));
  }

  getElementById(id) {
    return this.elements.get(id) || null;
  }

  createElement(tagName) {
    return new FakeElement(tagName);
  }
}

const extractInlineStartupScript = (source) => {
  const match = source.match(/<script>\s*([\s\S]*?)\s*<\/script>\s*<\/body>/);
  assert.ok(match, 'expected startup shell to include an inline bootstrap script');
  return match[1];
};

const flushMicrotasks = async () => {
  await new Promise((resolve) => setImmediate(resolve));
  await new Promise((resolve) => setImmediate(resolve));
};

const renderStartupShell = async ({ locale = 'zh-CN', snapshot } = {}) => {
  const [source, configSource] = await Promise.all([
    readFile(startupShellPath, 'utf8'),
    readFile(startupCopyConfigPath, 'utf8'),
  ]);
  const inlineScript = extractInlineStartupScript(source);
  const document = new FakeDocument(STARTUP_ELEMENT_IDS);
  const windowListeners = [];
  const intervalCalls = [];
  const invokeCalls = [];
  const window = {
    astrbot: {},
    __TAURI_INTERNALS__: {
      invoke: async (command) => {
        invokeCalls.push(command);
        return snapshot ?? null;
      },
    },
    setInterval: (handler, delay) => {
      intervalCalls.push({ handler, delay });
      return intervalCalls.length;
    },
    clearInterval: () => {},
    addEventListener: (type, listener, options) => {
      windowListeners.push({ type, listener, options });
    },
  };

  const context = vm.createContext({
    window,
    document,
    navigator: { language: locale },
    console,
    Promise,
    Object,
    Array,
    Number,
    String,
    Boolean,
    JSON,
    Map,
    Set,
  });

  vm.runInContext(configSource, context, { filename: 'startup-copy.js' });
  vm.runInContext(inlineScript, context, { filename: 'startup-shell-inline.js' });
  await flushMicrotasks();

  return {
    window,
    document,
    windowListeners,
    intervalCalls,
    invokeCalls,
  };
};

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
    /const\s+startupShell\s*=\s*window\.astrbot\.startupShell;/,
    'expected startup shell to read its shared config from a named astrbot namespace',
  );
  assert.match(
    source,
    /const\s+\{\s*STARTUP_MODES,\s*STARTUP_COPY\s*\}\s*=\s*startupShell;/,
    'expected startup shell to destructure startup config from the namespaced object',
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
    source,
    /setStatusText\(next\.status\);/,
    'expected startup shell to route startup-mode status changes through the shared duplicate-announcement guard',
  );

  assert.match(
    configSource,
    /const\s+deepFreeze\s*=\s*\(obj\)\s*=>/,
    'expected shared startup copy config to use a deepFreeze helper',
  );
  assert.match(
    configSource,
    /const\s+STARTUP_MODES\s*=\s*\{/,
    'expected shared startup copy config to define startup modes',
  );
  assert.match(
    configSource,
    /window\.astrbot\s*=\s*window\.astrbot\s*\|\|\s*\{\};/,
    'expected shared startup copy config to allocate the astrbot namespace',
  );
  assert.match(
    configSource,
    /window\.astrbot\.startupShell\s*=\s*deepFreeze\(/,
    'expected shared startup copy config to expose startup shell under the astrbot namespace',
  );
  assert.match(
    configSource,
    /STARTUP_COPY:\s*\{/,
    'expected shared startup copy config to define localized startup copy',
  );
  assert.match(
    configSource,
    /en:\s*\{/,
    'expected shared startup copy config to include English startup copy',
  );
  assert.match(
    configSource,
    /zh:\s*\{/,
    'expected shared startup copy config to include Chinese startup copy',
  );
});

test('startup diagnostics keeps localized stage summaries in frontend copy and stacks logs on narrow widths', async () => {
  const source = await readFile(startupShellPath, 'utf8');
  const configSource = await readFile(startupCopyConfigPath, 'utf8').catch(() => '');

  assert.match(
    configSource,
    /en:\s*\{[\s\S]*stageSummaries:\s*\{[\s\S]*resolveLaunchPlan:\s*'Resolving launch plan'/,
    'expected English stage summaries to stay in the shared frontend copy config',
  );
  assert.match(
    configSource,
    /zh:\s*\{[\s\S]*stageSummaries:\s*\{[\s\S]*resolveLaunchPlan:\s*'正在解析启动计划'/,
    'expected Chinese stage summaries to stay in the shared frontend copy config',
  );
  assert.match(
    source,
    /const\s+resolveSnapshotSummary\s*=\s*\(snapshot\)\s*=>/,
    'expected startup shell to resolve diagnostics summaries through a dedicated helper',
  );
  assert.match(
    source,
    /snapshot\?\.stage\s*===\s*"failed"/,
    'expected startup shell to surface failure details from snapshot data only for failed startup states',
  );
  assert.doesNotMatch(
    source,
    /setSummaryText\(snapshot\.summary\)/,
    'expected startup shell not to blindly reuse backend summary text for every stage',
  );
  assert.match(
    source,
    /setSummaryText\(resolveSnapshotSummary\(snapshot\)\)/,
    'expected snapshot rendering to localize non-failure summaries before updating the compact row',
  );
  assert.match(
    source,
    /@media\s*\(max-width:\s*\d+px\)\s*\{[\s\S]*?\.startup-log-grid\s*\{[\s\S]*?grid-template-columns:\s*1fr;/,
    'expected diagnostics log cards to collapse to one column at narrow widths',
  );
  assert.match(
    source,
    /\.startup-diagnostics\s*\{[\s\S]*overflow-y:\s*auto;[\s\S]*overflow-x:\s*hidden;/,
    'expected the capped diagnostics panel to scroll vertically so stacked log cards remain reachable on narrow widths',
  );
});

test('startup diagnostics keeps the live status row in sync with localized snapshot summaries', async () => {
  const source = await readFile(startupShellPath, 'utf8');

  assert.match(
    source,
    /const\s+setStatusText\s*=\s*\(value\)\s*=>/,
    'expected startup shell to centralize live status updates through a helper',
  );
  assert.match(
    source,
    /if\s*\(status\.textContent\s*===\s*nextValue\)\s*return;/,
    'expected live status updates to preserve duplicate-announcement restraint',
  );
  assert.match(
    source,
    /setStatusText\(resolveSnapshotSummary\(snapshot\)\)/,
    'expected polled startup snapshots to keep the live status row aligned with the compact summary',
  );
});

test('startup mode updates keep snapshot-controlled status and summary text aligned after polling starts', async () => {
  const { document, window } = await renderStartupShell({
    locale: 'zh-CN',
    snapshot: {
      stage: 'tcpReachable',
      summary: 'TCP ready, waiting for HTTP',
      items: [],
      desktopLog: [],
      backendLog: [],
    },
  });

  const status = document.getElementById('startup-status');
  const summaryText = document.getElementById('startup-summary-text');
  const desc = document.getElementById('startup-desc');

  assert.equal(status.textContent, 'TCP 已就绪，正在等待 HTTP');
  assert.equal(summaryText.textContent, 'TCP 已就绪，正在等待 HTTP');

  window.__astrbotSetStartupMode('panel-update');

  assert.equal(
    desc.textContent,
    '检测到新面板版本，正在下载并应用。',
    'expected startup mode changes to keep updating the descriptive copy',
  );
  assert.equal(
    status.textContent,
    'TCP 已就绪，正在等待 HTTP',
    'expected startup mode changes to stop overriding the live status after snapshot polling takes over',
  );
  assert.equal(
    summaryText.textContent,
    'TCP 已就绪，正在等待 HTTP',
    'expected startup mode changes to keep the compact summary synchronized with the live status after snapshot polling takes over',
  );
});

test('failed snapshot prefers localized frontend failure copy when backend summary is empty or generic', async () => {
  for (const summary of ['', 'Startup failed']) {
    const { document } = await renderStartupShell({
      locale: 'zh-CN',
      snapshot: {
        stage: 'failed',
        summary,
        items: [],
        desktopLog: [],
        backendLog: [],
      },
    });

    assert.equal(
      document.getElementById('startup-status').textContent,
      '启动失败',
      'expected failed snapshots with empty or generic backend summaries to fall back to localized frontend copy for the live status',
    );
    assert.equal(
      document.getElementById('startup-summary-text').textContent,
      '启动失败',
      'expected failed snapshots with empty or generic backend summaries to fall back to localized frontend copy for the compact summary',
    );
  }
});

test('startup task records the desktop log offset before async startup work is spawned', async () => {
  const source = await readFile(startupTaskPath, 'utf8');
  const taskStartIndex = source.indexOf('pub fn spawn_startup_task');
  const testsStartIndex = source.indexOf('#[cfg(test)]');
  const taskSource = source.slice(
    taskStartIndex,
    testsStartIndex === -1 ? source.length : testsStartIndex,
  );
  const prepareCallIndex = taskSource.lastIndexOf('prepare_startup_panel_for_attempt(');
  const spawnAsyncIndex = taskSource.indexOf('tauri::async_runtime::spawn(async move {');

  assert.notEqual(
    prepareCallIndex,
    -1,
    'expected startup task to record the desktop log start offset for the startup panel',
  );
  assert.notEqual(
    spawnAsyncIndex,
    -1,
    'expected startup task to spawn its async startup work',
  );
  assert.ok(
    prepareCallIndex < spawnAsyncIndex,
    'expected startup task to capture the desktop log offset before the async startup work begins',
  );
});
