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

const findStepByName = (steps, stepNameOrPattern) => {
  const matcher =
    stepNameOrPattern instanceof RegExp
      ? (candidateName) => stepNameOrPattern.test(candidateName ?? '')
      : (candidateName) => (candidateName ?? '').includes(stepNameOrPattern);
  const step = steps.find((candidate) => matcher(candidate.name));
  assert.ok(step, `Expected workflow step ${String(stepNameOrPattern)} to exist.`);
  return step;
};

test('findStepByName supports substring and regex matching', () => {
  const steps = [
    { name: 'Prepare desktop resources (macOS) [unsigned-compatible]' },
    { name: 'Build desktop app bundle (macOS) release artifacts' },
  ];

  assert.equal(findStepByName(steps, 'Prepare desktop resources (macOS)'), steps[0]);
  assert.equal(findStepByName(steps, /Build desktop app bundle \(macOS\)/), steps[1]);
});

test('macOS workflow exposes structured build-macos steps', async () => {
  const workflowObject = await readWorkflowObject();
  const steps = extractWorkflowJobSteps(workflowObject, 'build-macos');

  assert.ok(findStepByName(steps, 'Prepare desktop resources'));
  assert.ok(findStepByName(steps, 'Pre-sign backend resources'));
  assert.ok(findStepByName(steps, 'Build desktop app bundle'));
});

test('macOS workflow prepares resources before optional pre-signing', async () => {
  const workflowObject = await readWorkflowObject();
  const steps = extractWorkflowJobSteps(workflowObject, 'build-macos');
  const prepareStep = findStepByName(steps, 'Prepare desktop resources');
  const preSignStep = findStepByName(steps, 'Pre-sign backend resources');
  const buildStep = findStepByName(steps, 'Build desktop app bundle');

  assert.equal(prepareStep.if, undefined);
  assert.match(prepareStep.run, /pnpm run prepare:resources/);
  assert.match(prepareStep.run, /resources\/backend not found after prepare:resources/);

  assert.match(preSignStep.if ?? '', /import_apple_certificate\.outputs\.signing_identity/);
  assert.doesNotMatch(preSignStep.run, /pnpm run prepare:resources/);

  assert.ok(steps.indexOf(prepareStep) < steps.indexOf(preSignStep));
  assert.ok(steps.indexOf(preSignStep) < steps.indexOf(buildStep));
  assert.match(
    buildStep.run,
    /Resources are already prepared/,
  );
});
