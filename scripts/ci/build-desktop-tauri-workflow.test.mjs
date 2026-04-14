import assert from 'node:assert/strict';
import path from 'node:path';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, '..', '..');
const workflowPath = path.join(projectRoot, '.github', 'workflows', 'build-desktop-tauri.yml');

const readWorkflow = async () => readFile(workflowPath, 'utf8');

const TOP_LEVEL_JOB_PATTERN = /^  [A-Za-z0-9_-]+:\s*$/;
const MACOS_STEP_PREFIX = '      - ';
const STEP_FIELD_PREFIX = '        ';
const RUN_BLOCK_PREFIX = '          ';

const extractWorkflowJobSteps = (workflow, jobName) => {
  const lines = workflow.split(/\r?\n/);
  const jobHeader = `  ${jobName}:`;
  const jobStart = lines.findIndex((line) => line === jobHeader);
  assert.notEqual(jobStart, -1, `Expected workflow job ${jobName} to exist.`);

  let stepsStart = -1;
  let jobEnd = lines.length;
  for (let index = jobStart + 1; index < lines.length; index += 1) {
    const line = lines[index];

    if (stepsStart === -1 && line === '    steps:') {
      stepsStart = index;
      continue;
    }

    if (TOP_LEVEL_JOB_PATTERN.test(line)) {
      jobEnd = index;
      break;
    }
  }

  assert.notEqual(stepsStart, -1, `Expected workflow job ${jobName} to define steps.`);

  const steps = [];
  let currentStep = null;
  let collectingRunBlock = false;

  const finalizeCurrentStep = () => {
    if (!currentStep) {
      return;
    }

    currentStep.run = currentStep.runLines.join('\n').trimEnd();
    delete currentStep.runLines;
    steps.push(currentStep);
    currentStep = null;
    collectingRunBlock = false;
  };

  for (let index = stepsStart + 1; index < jobEnd; index += 1) {
    const line = lines[index];

    if (line.startsWith(MACOS_STEP_PREFIX)) {
      finalizeCurrentStep();
      currentStep = {
        name: null,
        if: null,
        run: '',
        runLines: [],
      };

      const inlineName = line.trim().match(/^- name:\s*(.+)$/);
      if (inlineName) {
        currentStep.name = inlineName[1];
      }
      continue;
    }

    if (!currentStep) {
      continue;
    }

    if (collectingRunBlock) {
      if (line.startsWith(RUN_BLOCK_PREFIX)) {
        currentStep.runLines.push(line.slice(RUN_BLOCK_PREFIX.length));
        continue;
      }
      if (line.trim() === '') {
        currentStep.runLines.push('');
        continue;
      }
      collectingRunBlock = false;
    }

    if (!line.startsWith(STEP_FIELD_PREFIX)) {
      continue;
    }

    const trimmed = line.trim();
    if (trimmed.startsWith('name: ')) {
      currentStep.name = trimmed.slice('name: '.length);
      continue;
    }
    if (trimmed.startsWith('if: ')) {
      currentStep.if = trimmed.slice('if: '.length);
      continue;
    }
    if (trimmed === 'run: |') {
      collectingRunBlock = true;
      continue;
    }
    if (trimmed.startsWith('run: ')) {
      currentStep.run = trimmed.slice('run: '.length);
    }
  }

  finalizeCurrentStep();
  return steps;
};

const findStepByName = (steps, stepName) => {
  const step = steps.find((candidate) => candidate.name === stepName);
  assert.ok(step, `Expected workflow step ${stepName} to exist.`);
  return step;
};

test('macOS workflow exposes structured build-macos steps', async () => {
  const workflow = await readWorkflow();
  const steps = extractWorkflowJobSteps(workflow, 'build-macos');

  assert.ok(findStepByName(steps, 'Prepare desktop resources (macOS)'));
  assert.ok(findStepByName(steps, 'Pre-sign backend resources (macOS)'));
  assert.ok(findStepByName(steps, 'Build desktop app bundle (macOS)'));
});

test('macOS workflow prepares resources before optional pre-signing', async () => {
  const workflow = await readWorkflow();
  const steps = extractWorkflowJobSteps(workflow, 'build-macos');
  const prepareStep = findStepByName(steps, 'Prepare desktop resources (macOS)');
  const preSignStep = findStepByName(steps, 'Pre-sign backend resources (macOS)');
  const buildStep = findStepByName(steps, 'Build desktop app bundle (macOS)');

  assert.equal(prepareStep.if, null);
  assert.match(prepareStep.run, /pnpm run prepare:resources/);
  assert.match(prepareStep.run, /resources\/backend not found after prepare:resources/);

  assert.equal(preSignStep.if, "${{ steps.import_apple_certificate.outputs.signing_identity != '' }}");
  assert.doesNotMatch(preSignStep.run, /pnpm run prepare:resources/);

  assert.match(
    buildStep.run,
    /# Resources are already prepared and, when available, pre-signed in earlier steps\./,
  );
});
