import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  extractCompositeActionSteps,
  findStep,
  readActionObject,
} from './workflow-test-utils.mjs';

const ACTION_DIR = 'setup-toolchains';

test('setup-toolchains action forwards optional node cache settings to setup-node', async () => {
  const actionObject = await readActionObject(ACTION_DIR);
  const steps = extractCompositeActionSteps(actionObject, ACTION_DIR);
  const setupNodeStep = findStep(
    steps,
    'setup-node step',
    (step) => (step.uses ?? '').includes('actions/setup-node'),
  );

  assert.equal(actionObject.inputs['node-cache'].default, '');
  assert.equal(actionObject.inputs['node-cache-dependency-path'].default, '');
  assert.equal(setupNodeStep.with.cache, '${{ inputs.node-cache }}');
  assert.equal(
    setupNodeStep.with['cache-dependency-path'],
    '${{ inputs.node-cache-dependency-path }}',
  );
});
