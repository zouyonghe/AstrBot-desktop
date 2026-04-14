import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  extractWorkflowJobSteps,
  findStep,
  readWorkflowObject,
} from './workflow-test-utils.mjs';

const WORKFLOW_FILE = 'build-desktop-tauri.yml';
const BUILD_MACOS_JOB = 'build-macos';
const PREPARE_RESOURCES_RUN = /pnpm run prepare:resources/;
const PRESIGN_BACKEND_RUN = /codesign-macos-nested\.sh\s+"resources\/backend"/;
const BUILD_APP_BUNDLE_RUN = /cargo tauri build --verbose --target/;

test('findStep supports predicate and regex matching', () => {
  const steps = [
    { name: 'Prepare desktop resources (macOS) [unsigned-compatible]', run: 'pnpm run prepare:resources' },
    { name: 'Build desktop app bundle (macOS) release artifacts', run: 'cargo tauri build --verbose --target x86_64-apple-darwin' },
  ];

  assert.equal(findStep(steps, 'prepare resources run', (step) => PREPARE_RESOURCES_RUN.test(step.run ?? '')), steps[0]);
  assert.equal(findStep(steps, /Build desktop app bundle/, (step) => BUILD_APP_BUNDLE_RUN.test(step.run ?? '')), steps[1]);
});

test('macOS workflow exposes structured build-macos steps', async () => {
  const workflowObject = await readWorkflowObject(WORKFLOW_FILE);
  const steps = extractWorkflowJobSteps(workflowObject, BUILD_MACOS_JOB);

  assert.ok(findStep(steps, 'prepare resources step', (step) => PREPARE_RESOURCES_RUN.test(step.run ?? '')));
  assert.ok(findStep(steps, 'pre-sign resources step', (step) => PRESIGN_BACKEND_RUN.test(step.run ?? '')));
  assert.ok(findStep(steps, 'build app bundle step', (step) => BUILD_APP_BUNDLE_RUN.test(step.run ?? '')));
});

test('macOS workflow prepares resources before optional pre-signing', async () => {
  const workflowObject = await readWorkflowObject(WORKFLOW_FILE);
  const steps = extractWorkflowJobSteps(workflowObject, BUILD_MACOS_JOB);
  const prepareStep = findStep(steps, 'prepare resources step', (step) => PREPARE_RESOURCES_RUN.test(step.run ?? ''));
  const preSignStep = findStep(steps, 'pre-sign resources step', (step) => PRESIGN_BACKEND_RUN.test(step.run ?? ''));
  const buildStep = findStep(steps, 'build app bundle step', (step) => BUILD_APP_BUNDLE_RUN.test(step.run ?? ''));

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
