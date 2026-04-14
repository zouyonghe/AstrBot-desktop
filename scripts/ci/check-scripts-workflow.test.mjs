import assert from 'node:assert/strict';
import path from 'node:path';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { parse } from 'yaml';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, '..', '..');
const workflowPath = path.join(projectRoot, '.github', 'workflows', 'check-scripts.yml');

const readWorkflowObject = async () => {
  const content = await readFile(workflowPath, 'utf8');
  return parse(content);
};

const extractWorkflowJobSteps = (workflowObject, jobName) => {
  assert.ok(workflowObject.jobs, 'Expected workflow to define jobs.');
  const job = workflowObject.jobs[jobName];
  assert.ok(job, `Expected workflow job ${jobName} to exist.`);
  assert.ok(Array.isArray(job.steps), `Expected workflow job ${jobName} to define steps.`);
  return job.steps;
};

const findStepIndex = (steps, predicate, label) => {
  const index = steps.findIndex(predicate);
  assert.notEqual(index, -1, `Expected workflow step ${label} to exist.`);
  return index;
};

test('check-scripts workflow installs node dependencies before running node tests', async () => {
  const workflowObject = await readWorkflowObject();
  const steps = extractWorkflowJobSteps(workflowObject, 'scripts');

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
