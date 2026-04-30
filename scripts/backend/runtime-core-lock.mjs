import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const generatorScriptPath = path.join(__dirname, 'tools', 'generate_runtime_core_lock.py');
const formatGeneratorContext = (pythonPath) =>
  `(python: ${pythonPath ?? 'undefined'}, script: ${generatorScriptPath})`;

export const generateRuntimeCoreLock = ({ runtimePython, outputPath }) => {
  if (!runtimePython?.absolute) {
    throw new Error(
      `Missing runtime Python executable for runtime core lock generation ${formatGeneratorContext(runtimePython?.absolute)}.`,
    );
  }
  if (!outputPath) {
    throw new Error(
      `Missing output path for runtime core lock generation ${formatGeneratorContext(runtimePython.absolute)}.`,
    );
  }

  const result = spawnSync(
    runtimePython.absolute,
    [generatorScriptPath, '--output', outputPath],
    {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true,
    },
  );

  if (result.error) {
    throw new Error(
      `Failed to generate runtime core lock ${formatGeneratorContext(runtimePython.absolute)}: ${result.error.message}`,
    );
  }
  if (result.status !== 0) {
    let detail;
    if (result.signal) {
      detail = `terminated by signal ${result.signal}`;
    } else {
      detail = result.stderr?.trim() || result.stdout?.trim() || `exit code ${result.status}`;
    }
    throw new Error(
      `Runtime core lock generation failed ${formatGeneratorContext(runtimePython.absolute)}: ${detail}`,
    );
  }

  {
    const context = formatGeneratorContext(runtimePython.absolute);
    const baseMessage = `Runtime core lock generator did not create valid ${outputPath} ${context}.`;

    if (!fs.existsSync(outputPath)) {
      throw new Error(baseMessage);
    }

    try {
      const contents = fs.readFileSync(outputPath, 'utf8');
      if (!contents.trim()) {
        throw new Error('empty runtime core lock output');
      }
      JSON.parse(contents);
    } catch {
      throw new Error(baseMessage);
    }
  }
};
