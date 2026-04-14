import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  extractWorkflowJobSteps,
  findStepIndex,
  readWorkflowObject,
} from './workflow-test-utils.mjs';

const WORKFLOW_FILE = 'check-scripts.yml';
const SCRIPTS_JOB = 'scripts';

test('check-scripts workflow installs node dependencies before running node tests', async () => {
  const workflowObject = await readWorkflowObject(WORKFLOW_FILE);
  const steps = extractWorkflowJobSteps(workflowObject, SCRIPTS_JOB);

  const pnpmSetupIndex = findStepIndex(
    steps,
    (step) => (step.uses ?? '').includes('pnpm/action-setup'),
    'using pnpm/action-setup',
  );
  const installIndex = findStepIndex(
    steps,
    (step) => /pnpm install/.test(step.run ?? ''),
    'installing node dependencies',
  );
  const nodeTestIndex = findStepIndex(
    steps,
    (step) => /node --test/.test(step.run ?? ''),
    'running Node script behavior tests',
  );

  assert.ok(pnpmSetupIndex < installIndex);
  assert.ok(installIndex < nodeTestIndex);
});
