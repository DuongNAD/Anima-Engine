import { describe, expect, it } from 'vitest';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
// @ts-expect-error -- check_bundle_budget.mjs has no .d.mts declaration
import { checkBundleBudget, collectStaticChunkClosure, extractHtmlJsRefs, hasThreeJsSignature } from '../../scripts/check_bundle_budget.mjs';

interface EntryReport {
  file: string;
  label: string;
  referenced: string[];
  reachable: Array<{ file: string; content: string; missing: boolean }>;
  loadsThreeD: boolean;
  missing: boolean;
}

interface BudgetResult {
  ok: boolean;
  exitCode: number;
  error?: string;
  files: Array<{ file: string; name: string; kib: number }>;
  totalKib: number;
  entryReports: EntryReport[];
  failures: string[];
}

interface DistFixtureOptions {
  indexRefs?: string[];
  landscapeRefs?: string[];
  map2Refs?: string[];
  chunks: Record<string, string>;
}

const SCRIPT_PATH = resolve(import.meta.dirname, '../../scripts/check_bundle_budget.mjs');

function makeTempDist(opts: DistFixtureOptions): { dir: string; cleanup: () => void } {
  const dir = mkdtempSync(join(tmpdir(), 'anima-bundle-budget-'));
  const assetsDir = join(dir, 'assets');
  mkdirSync(assetsDir, { recursive: true });

  for (const [name, content] of Object.entries(opts.chunks)) {
    writeFileSync(join(assetsDir, name), content, 'utf8');
  }

  const makeHtml = (refs: string[]) =>
    `<!doctype html><html><head>${refs
      .map((f, i) =>
        i === 0
          ? `<script type="module" src="/assets/${f}"></script>`
          : `<link rel="modulepreload" href="/assets/${f}">`,
      )
      .join('')}</head><body></body></html>`;

  writeFileSync(join(dir, 'index.html'), makeHtml(opts.indexRefs ?? ['main-abc1234.js']), 'utf8');
  writeFileSync(
    join(dir, 'landscape.html'),
    makeHtml(opts.landscapeRefs ?? ['landscape-abc1234.js', 'vendor3d-abc1234.js']),
    'utf8',
  );
  writeFileSync(
    join(dir, 'map2.html'),
    makeHtml(opts.map2Refs ?? ['map2-abc1234.js', 'vendor3d-abc1234.js']),
    'utf8',
  );

  return {
    dir,
    cleanup: () => rmSync(dir, { recursive: true, force: true }),
  };
}

describe('check_bundle_budget — F-054 content-based 3D detection and static import closure', () => {
  it('hasThreeJsSignature detects Three.js markers without false positives on PixiJS symbols', () => {
    expect(hasThreeJsSignature('window.__THREE__="184";')).toBe(true);
    expect(hasThreeJsSignature('console.log(THREE.Matrix4, THREE.WebGLRenderer);')).toBe(true);
    expect(hasThreeJsSignature('class WebGLRenderer {} const Matrix4 = 1; const Quaternion = 2;')).toBe(
      false,
    );
    expect(hasThreeJsSignature('const x = "assets/react-three-fiber.esm-TSIJjToy.js";')).toBe(false);
    expect(hasThreeJsSignature('const my__THREE__helper = 1;')).toBe(false);
  });

  it('passes on a clean 2D/3D split', () => {
    const fixture = makeTempDist({
      indexRefs: ['main-abc1234.js', 'defineProperty-abc1234.js'],
      landscapeRefs: ['landscape-abc1234.js', 'react-three-fiber.esm-abc1234.js'],
      map2Refs: ['map2-abc1234.js', 'react-three-fiber.esm-abc1234.js'],
      chunks: {
        'main-abc1234.js': 'import"./defineProperty-abc1234.js";export const app=1;',
        'defineProperty-abc1234.js': 'export const def=1;',
        'landscape-abc1234.js': 'import"./react-three-fiber.esm-abc1234.js";',
        'map2-abc1234.js': 'import"./react-three-fiber.esm-abc1234.js";',
        'react-three-fiber.esm-abc1234.js': 'window.__THREE__="184";THREE.WebGLRenderer;',
      },
    });

    try {
      const result = checkBundleBudget(fixture.dir) as BudgetResult;
      expect(result.ok).toBe(true);
      expect(result.exitCode).toBe(0);
      expect(result.failures).toEqual([]);

      const byFile = Object.fromEntries(result.entryReports.map((r) => [r.file, r]));
      expect(byFile['index.html']?.loadsThreeD).toBe(false);
      expect(byFile['landscape.html']?.loadsThreeD).toBe(true);
      expect(byFile['map2.html']?.loadsThreeD).toBe(true);
    } finally {
      fixture.cleanup();
    }
  });

  it('catches F-054 when three.js is hoisted into a within-budget defineProperty-*.js chunk', () => {
    const paddedThree = `window.__THREE__="184";/*${'x'.repeat(120 * 1024)}*/`;
    const fixture = makeTempDist({
      indexRefs: ['main-abc1234.js', 'defineProperty-deadbeef.js'],
      landscapeRefs: ['landscape-abc1234.js', 'defineProperty-deadbeef.js'],
      map2Refs: ['map2-abc1234.js', 'defineProperty-deadbeef.js'],
      chunks: {
        'main-abc1234.js': 'import"./defineProperty-deadbeef.js";',
        'defineProperty-deadbeef.js': paddedThree,
        'landscape-abc1234.js': 'import"./defineProperty-deadbeef.js";',
        'map2-abc1234.js': 'import"./defineProperty-deadbeef.js";',
      },
    });

    try {
      const result = checkBundleBudget(fixture.dir) as BudgetResult;
      expect(result.ok).toBe(false);
      expect(result.exitCode).toBe(1);
      // defineProperty-deadbeef.js is ~120 KiB (< 170 KiB budget), so failure MUST come from route split
      expect(result.failures.length).toBe(1);
      expect(result.failures[0]).toMatch(/statically loads the three\.js chunk/);

      const indexReport = result.entryReports.find((r) => r.file === 'index.html');
      expect(indexReport?.loadsThreeD).toBe(true);
    } finally {
      fixture.cleanup();
    }
  });

  it('catches transitive static ESM imports from index.html even without modulepreload links', () => {
    const fixture = makeTempDist({
      indexRefs: ['main-abc1234.js'],
      landscapeRefs: ['landscape-abc1234.js'],
      map2Refs: ['map2-abc1234.js'],
      chunks: {
        'main-abc1234.js': 'import{x}from"./shared-def5678.js";console.log(x);',
        'shared-def5678.js': 'export const x=1;THREE.Matrix4;',
        'landscape-abc1234.js': 'import"./shared-def5678.js";',
        'map2-abc1234.js': 'import"./shared-def5678.js";',
      },
    });

    try {
      const closure = collectStaticChunkClosure(
        extractHtmlJsRefs('<script src="/assets/main-abc1234.js"></script>'),
        join(fixture.dir, 'assets'),
      ) as Array<{ file: string }>;
      expect(closure.map((c) => c.file)).toEqual(['main-abc1234.js', 'shared-def5678.js']);

      const result = checkBundleBudget(fixture.dir) as BudgetResult;
      expect(result.ok).toBe(false);
      expect(result.exitCode).toBe(1);
      expect(result.failures).toHaveLength(1);
      expect(result.failures[0]).toMatch(/statically loads the three\.js chunk/);
    } finally {
      fixture.cleanup();
    }
  });

  it('does not false-positive on PixiJS WebGLRenderer/Matrix4, preload strings, or dynamic import()', () => {
    const fixture = makeTempDist({
      indexRefs: ['main-abc1234.js'],
      landscapeRefs: ['landscape-abc1234.js', 'three-bundle-abc1234.js'],
      map2Refs: ['map2-abc1234.js', 'three-bundle-abc1234.js'],
      chunks: {
        'main-abc1234.js':
          'const __vite__mapDeps=["assets/react-three-fiber.esm-TSIJjToy.js"];' +
          'class WebGLRenderer{} const Matrix4=1,Quaternion=2;' +
          'const lazy3d=()=>import("./three-bundle-abc1234.js");' +
          'export{__vite__mapDeps,WebGLRenderer,Matrix4,Quaternion,lazy3d};',
        'landscape-abc1234.js': 'import"./three-bundle-abc1234.js";',
        'map2-abc1234.js': 'import"./three-bundle-abc1234.js";',
        'three-bundle-abc1234.js': 'window.__THREE__="184";',
      },
    });

    try {
      const result = checkBundleBudget(fixture.dir) as BudgetResult;
      expect(result.ok).toBe(true);
      expect(result.exitCode).toBe(0);
      const indexReport = result.entryReports.find((r) => r.file === 'index.html');
      expect(indexReport?.loadsThreeD).toBe(false);
    } finally {
      fixture.cleanup();
    }
  });

  it('fails when landscape.html or map2.html stops loading a 3D renderer', () => {
    const fixture = makeTempDist({
      indexRefs: ['main-abc1234.js'],
      landscapeRefs: ['landscape-abc1234.js'],
      map2Refs: ['map2-abc1234.js'],
      chunks: {
        'main-abc1234.js': 'export const app=1;',
        'landscape-abc1234.js': 'export const l=1;',
        'map2-abc1234.js': 'window.__THREE__="184";',
      },
    });

    try {
      const result = checkBundleBudget(fixture.dir) as BudgetResult;
      expect(result.ok).toBe(false);
      expect(result.exitCode).toBe(1);
      expect(result.failures).toHaveLength(1);
      expect(result.failures[0]).toMatch(/landscape\.html.*does not reference the three\.js chunk/);
    } finally {
      fixture.cleanup();
    }
  });

  it('preserves CLI stdout/stderr contract on clean and F-054 failing runs', () => {
    const cleanFixture = makeTempDist({
      indexRefs: ['main-abc1234.js'],
      landscapeRefs: ['landscape-abc1234.js', 'r3f-abc1234.js'],
      map2Refs: ['map2-abc1234.js', 'r3f-abc1234.js'],
      chunks: {
        'main-abc1234.js': 'export const ok=1;',
        'landscape-abc1234.js': 'import"./r3f-abc1234.js";',
        'map2-abc1234.js': 'import"./r3f-abc1234.js";',
        'r3f-abc1234.js': 'window.__THREE__="184";',
      },
    });
    const leakFixture = makeTempDist({
      indexRefs: ['main-abc1234.js', 'defineProperty-deadbeef.js'],
      landscapeRefs: ['landscape-abc1234.js', 'defineProperty-deadbeef.js'],
      map2Refs: ['map2-abc1234.js', 'defineProperty-deadbeef.js'],
      chunks: {
        'main-abc1234.js': 'import"./defineProperty-deadbeef.js";',
        'defineProperty-deadbeef.js': 'window.__THREE__="184";',
        'landscape-abc1234.js': 'import"./defineProperty-deadbeef.js";',
        'map2-abc1234.js': 'import"./defineProperty-deadbeef.js";',
      },
    });

    try {
      const cleanRun = spawnSync(process.execPath, [SCRIPT_PATH, cleanFixture.dir], {
        encoding: 'utf8',
      });
      expect(cleanRun.status).toBe(0);
      expect(cleanRun.stdout).toMatch(/entry index\.html\s+1 static chunk\(s\), 3D renderer: no/);
      expect(cleanRun.stdout).toMatch(/entry landscape\.html\s+2 static chunk\(s\), 3D renderer: yes/);
      expect(cleanRun.stdout).toMatch(/entry map2\.html\s+2 static chunk\(s\), 3D renderer: yes/);

      const leakRun = spawnSync(process.execPath, [SCRIPT_PATH, leakFixture.dir], {
        encoding: 'utf8',
      });
      expect(leakRun.status).toBe(1);
      expect(leakRun.stdout).toMatch(/entry index\.html\s+2 static chunk\(s\), 3D renderer: yes/);
      expect(leakRun.stderr).toContain('bundle budget FAILED:');
      expect(leakRun.stderr).toMatch(
        /^\s*- the 2D dashboard \(index\.html\) statically loads the three\.js chunk/m,
      );
    } finally {
      cleanFixture.cleanup();
      leakFixture.cleanup();
    }
  });
});
