import assert from 'node:assert/strict';
import path from 'node:path';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { parse } from 'yaml';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, '..', '..');
const workflowsDir = path.join(projectRoot, '.github', 'workflows');

export const readWorkflowObject = async (workflowFileName) => {
  const workflowPath = path.join(workflowsDir, workflowFileName);
  const content = await readFile(workflowPath, 'utf8');
  return parse(content);
};

export const extractWorkflowJobSteps = (workflowObject, jobName) => {
  assert.ok(workflowObject.jobs, 'Expected workflow to define jobs.');
  const job = workflowObject.jobs[jobName];
  assert.ok(job, `Expected workflow job ${jobName} to exist.`);
  assert.ok(Array.isArray(job.steps), `Expected workflow job ${jobName} to define steps.`);
  return job.steps;
};

export const findStep = (steps, label, predicate) => {
  const step = steps.find(predicate);
  assert.ok(step, `Expected workflow step ${String(label)} to exist.`);
  return step;
};

export const findStepIndex = (steps, predicate, label) => {
  const index = steps.findIndex(predicate);
  assert.notEqual(index, -1, `Expected workflow step ${String(label)} to exist.`);
  return index;
};
