import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  extractWorkflowJobSteps,
  findStep,
  findStepIndex,
  readWorkflowObject,
} from './workflow-test-utils.mjs';

const WORKFLOW_FILE = 'check-scripts.yml';
const SCRIPTS_JOB = 'scripts';

test('check-scripts workflow installs node dependencies before running node tests', async () => {
  const workflowObject = await readWorkflowObject(WORKFLOW_FILE);
  const steps = extractWorkflowJobSteps(workflowObject, SCRIPTS_JOB);
  const setupToolchainsStep = findStep(
    steps,
    'setup toolchains step',
    (step) => (step.uses ?? '').includes('./.github/actions/setup-toolchains'),
  );
  const installStep = findStep(
    steps,
    'installing node dependencies',
    (step) => /pnpm install/.test(step.run ?? ''),
  );

  const pnpmSetupIndex = findStepIndex(
    steps,
    (step) => (step.uses ?? '').includes('pnpm/action-setup'),
    'using pnpm/action-setup',
  );
  const setupToolchainsIndex = findStepIndex(
    steps,
    (step) => step === setupToolchainsStep,
    'using setup toolchains',
  );
  const installIndex = findStepIndex(
    steps,
    (step) => step === installStep,
    'installing node dependencies',
  );
  const nodeTestIndex = findStepIndex(
    steps,
    (step) => /node --test/.test(step.run ?? ''),
    'running Node script behavior tests',
  );

  assert.equal(setupToolchainsStep.with['node-cache'], 'pnpm');
  assert.equal(setupToolchainsStep.with['node-cache-dependency-path'], 'pnpm-lock.yaml');
  assert.match(installStep.run, /pnpm install/);
  assert.match(installStep.run, /--frozen-lockfile/);
  assert.match(installStep.run, /--ignore-scripts/);

  assert.ok(pnpmSetupIndex < setupToolchainsIndex);
  assert.ok(pnpmSetupIndex < installIndex);
  assert.ok(installIndex < nodeTestIndex);
});
