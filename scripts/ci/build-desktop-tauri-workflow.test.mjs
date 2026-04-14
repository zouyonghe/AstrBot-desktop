import assert from 'node:assert/strict';
import path from 'node:path';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { parse } from 'yaml';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, '..', '..');
const workflowPath = path.join(projectRoot, '.github', 'workflows', 'build-desktop-tauri.yml');

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

const findStepByName = (steps, stepName) => {
  const step = steps.find((candidate) => candidate.name === stepName);
  assert.ok(step, `Expected workflow step ${stepName} to exist.`);
  return step;
};

test('macOS workflow exposes structured build-macos steps', async () => {
  const workflowObject = await readWorkflowObject();
  const steps = extractWorkflowJobSteps(workflowObject, 'build-macos');

  assert.ok(findStepByName(steps, 'Prepare desktop resources (macOS)'));
  assert.ok(findStepByName(steps, 'Pre-sign backend resources (macOS)'));
  assert.ok(findStepByName(steps, 'Build desktop app bundle (macOS)'));
});

test('macOS workflow prepares resources before optional pre-signing', async () => {
  const workflowObject = await readWorkflowObject();
  const steps = extractWorkflowJobSteps(workflowObject, 'build-macos');
  const prepareStep = findStepByName(steps, 'Prepare desktop resources (macOS)');
  const preSignStep = findStepByName(steps, 'Pre-sign backend resources (macOS)');
  const buildStep = findStepByName(steps, 'Build desktop app bundle (macOS)');

  assert.equal(prepareStep.if, undefined);
  assert.match(prepareStep.run, /pnpm run prepare:resources/);
  assert.match(prepareStep.run, /resources\/backend not found after prepare:resources/);

  assert.equal(preSignStep.if, "${{ steps.import_apple_certificate.outputs.signing_identity != '' }}");
  assert.doesNotMatch(preSignStep.run, /pnpm run prepare:resources/);

  assert.ok(steps.indexOf(prepareStep) < steps.indexOf(preSignStep));
  assert.ok(steps.indexOf(preSignStep) < steps.indexOf(buildStep));
  assert.match(
    buildStep.run,
    /# Resources are already prepared and, when available, pre-signed in earlier steps\./,
  );
});
