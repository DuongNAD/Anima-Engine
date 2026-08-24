import { describe, expect, it } from 'vitest';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const repoRoot = resolve(import.meta.dirname, '..', '..');

describe('E2 artifact portability', () => {
  it('checks out experiment inputs with the same LF bytes on every platform', () => {
    const fixture = 'src-tauri/tests/fixtures/experiments_e2/e2-preregistration.json';
    const result = spawnSync('git', ['check-attr', 'text', 'eol', '--', fixture], {
      cwd: repoRoot,
      encoding: 'utf8',
    });

    expect(result.status).toBe(0);
    expect(result.stdout).toContain(`${fixture}: text: set`);
    expect(result.stdout).toContain(`${fixture}: eol: lf`);
  });

  it('runs artifact verification in CI instead of relying on local discipline', () => {
    const workflow = readFileSync(resolve(repoRoot, '.github', 'workflows', 'ci.yml'), 'utf8');

    expect(workflow).toContain('run: npm run check:e2');
  });
});
