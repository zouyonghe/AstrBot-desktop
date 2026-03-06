import { test } from 'node:test';
import assert from 'node:assert/strict';

import { runModeTasks } from './mode-dispatch.mjs';

const createContext = (calls) => ({
  sourceDir: '/tmp/source',
  projectRoot: '/tmp/project',
  sourceRepoRef: 'v4.19.2',
  isSourceRepoRefVersionTag: true,
  isDesktopBridgeExpectationStrict: false,
  pythonBuildStandaloneRelease: '20260211',
  pythonBuildStandaloneVersion: '3.12.12',
});

const createTaskRunner = (calls) => ({
  prepareWebui: async () => calls.push('webui'),
  prepareBackend: async () => calls.push('backend'),
});

test('runModeTasks skips handlers in version mode', async () => {
  const calls = [];

  await runModeTasks('version', createContext(calls), createTaskRunner(calls));

  assert.deepEqual(calls, []);
});

test('runModeTasks runs webui handler in webui mode', async () => {
  const calls = [];

  await runModeTasks('webui', createContext(calls), createTaskRunner(calls));

  assert.deepEqual(calls, ['webui']);
});

test('runModeTasks runs backend handler in backend mode', async () => {
  const calls = [];

  await runModeTasks('backend', createContext(calls), createTaskRunner(calls));

  assert.deepEqual(calls, ['backend']);
});

test('runModeTasks runs webui then backend handlers in all mode', async () => {
  const calls = [];

  await runModeTasks('all', createContext(calls), createTaskRunner(calls));

  assert.deepEqual(calls, ['webui', 'backend']);
});

test('runModeTasks throws for unsupported mode', async () => {
  await assert.rejects(
    () =>
      runModeTasks('desktop', createContext([]), createTaskRunner([])),
    /Unsupported mode: desktop\. Expected version\/webui\/backend\/all\./,
  );
});
