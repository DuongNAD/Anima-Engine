import { describe, expect, it } from 'vitest';
import { execFileSync } from 'node:child_process';
import { existsSync, lstatSync, mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import { join } from 'node:path';
// @ts-expect-error -- gate_controls.mjs has no .d.mts declaration
import { FAULTS_DIR, GATES, REPO_ROOT, VERDICTS, classifyGateResult, cleanupStaleWorktree, defaultCleanKey, evaluateGate, findMatchingLine, formatMarkdownTable, parseCliArgs, runSelfTest, sortAndSummarizeResults, unlinkNodeModules, validateGateRegistry } from '../../scripts/paper/gate_controls.mjs';

interface GateRecord {
  id: string;
  source: string;
  ciCommand: string;
  faultSummary: string;
  patch: string | null;
  cleanKey?: string;
  needsCargo?: boolean;
  needsNodeModules?: boolean;
  runClean?: () => { status: number; output: string };
  matchers?: RegExp[];
  staticResult?: {
    exitCode: string | number;
    namingLine: string;
    verdict: string;
  };
}

interface ClassificationResult {
  verdict: string;
  exitCode: number | string;
  namingLine: string;
  matched: boolean;
}

describe('gate_controls — clean-tree baseline, matcher-verified CATCHES, & registry integrity', () => {
  it('classifies CATCHES only when clean baseline passes, fault fails, and a matcher hits', () => {
    const res = classifyGateResult({
      baselineRes: { status: 0, output: 'ok' },
      faultRes: {
        status: 101,
        output: [
          'Compiling anima-engine v0.1.0',
          'test unrelated_ok ... ok',
          'error: use of a disallowed method `f32::sin`',
          'error: could not compile `anima-worldgen`',
        ].join('\n'),
      },
      matchers: [/disallowed method `f32::sin`/],
    }) as ClassificationResult;

    expect(res.verdict).toBe(VERDICTS.CATCHES);
    expect(res.exitCode).toBe(101);
    expect(res.matched).toBe(true);
    expect(res.namingLine).toBe('error: use of a disallowed method `f32::sin`');
  });

  it('classifies VACUOUS when clean baseline passes and faulted run exits 0', () => {
    const res = classifyGateResult({
      baselineRes: { status: 0, output: 'thread_rng() literal matches: 0' },
      faultRes: { status: 0, output: 'thread_rng() literal matches: 0' },
      matchers: [/thread_rng\(\) literal matches/],
    }) as ClassificationResult;

    expect(res.verdict).toBe(VERDICTS.VACUOUS);
    expect(res.exitCode).toBe(0);
    expect(res.namingLine).toBe('thread_rng() literal matches: 0');
  });

  it('classifies NOT RUN (baseline red) when clean baseline exits non-zero and never credits CATCHES', () => {
    const res = classifyGateResult({
      baselineRes: {
        status: 1,
        output: [
          'file:///repo/scripts/gen_third_party_licenses.mjs:302',
          '  throw new Error(',
          'Error: licensing/upstream/sources.json maps 3 component(s) that are not in the dependency graph:',
        ].join('\n'),
      },
      faultRes: {
        status: 1,
        output: 'should never credit this fault output',
      },
      matchers: [/should never credit/],
    }) as ClassificationResult;

    expect(res.verdict).toBe(VERDICTS.NOT_RUN_BASELINE_RED);
    expect(res.exitCode).toBe(1);
    expect(res.matched).toBe(false);
    expect(res.namingLine).toContain('licensing/upstream/sources.json maps 3 component(s)');
  });

  it('classifies UNMATCHED_FAILURE (not CATCHES) when fault fails for an unrelated reason', () => {
    const res = classifyGateResult({
      baselineRes: { status: 0, output: 'ok' },
      faultRes: {
        status: 1,
        output: 'error: missing tool cargo-foo on PATH',
      },
      matchers: [/RUSTSEC-2026-0367/],
    }) as ClassificationResult;

    expect(res.verdict).toBe(VERDICTS.UNMATCHED_FAILURE);
    expect(res.verdict).not.toBe(VERDICTS.CATCHES);
    expect(res.exitCode).toBe(1);
    expect(res.matched).toBe(false);
    expect(res.namingLine).toBe('error: missing tool cargo-foo on PATH');
  });

  it('strips ANSI escapes and cargo/npm noise before matching', () => {
    const picked = findMatchingLine(
      '\u001b[31m> npm run check\u001b[0m\ntest passing_test ... ok\n\u001b[31mFAIL\u001b[0m src/foo.ts: expected 1',
      [/FAIL\s+src\/foo\.ts/],
      true,
    ) as { matched: boolean; line: string; matcherIndex: number };

    expect(picked.matched).toBe(true);
    expect(picked.matcherIndex).toBe(0);
    expect(picked.line).toBe('FAIL src/foo.ts: expected 1');
  });

  it('evaluateGate skips fault injection when clean baseline is red and caches shared clean baselines', () => {
    let cleanCalls = 0;
    let faultCalls = 0;
    const gateA: GateRecord = {
      id: 'mock_gate_a',
      source: 'test',
      ciCommand: 'shared-cmd',
      faultSummary: 'fault a',
      patch: 'a.patch',
      matchers: [/fault_a/],
    };
    const gateB: GateRecord = {
      id: 'mock_gate_b',
      source: 'test',
      ciCommand: 'shared-cmd',
      faultSummary: 'fault b',
      patch: 'b.patch',
      matchers: [/fault_b/],
    };
    const ctx = { captures: {}, tempFilesToClean: [], cleanCache: new Map<string, unknown>() };

    const recA = evaluateGate(gateA, ctx, {
      runClean: () => {
        cleanCalls += 1;
        return { status: 2, output: 'Error: clean baseline broken' };
      },
      runFault: () => {
        faultCalls += 1;
        return { status: 1, output: 'fault_a' };
      },
    }) as { verdict: string; exitCode: number; baselineExitCode: number };

    const recB = evaluateGate(gateB, ctx, {
      runClean: () => {
        cleanCalls += 1;
        return { status: 0, output: 'should use cache' };
      },
      runFault: () => {
        faultCalls += 1;
        return { status: 1, output: 'fault_b' };
      },
    }) as { verdict: string; exitCode: number; baselineExitCode: number };

    expect(cleanCalls).toBe(1);
    expect(faultCalls).toBe(0);
    expect(recA.verdict).toBe(VERDICTS.NOT_RUN_BASELINE_RED);
    expect(recA.exitCode).toBe(2);
    expect(recA.baselineExitCode).toBe(2);
    expect(recB.verdict).toBe(VERDICTS.NOT_RUN_BASELINE_RED);
  });

  it('evaluateGate handles green baselines (CATCHES, VACUOUS, UNMATCHED_FAILURE) and reuses cleanCache', () => {
    let cleanCalls = 0;
    let faultCalls = 0;
    const sharedCmd = 'node scripts/check_example.mjs';
    const gateCatch: GateRecord = {
      id: 'mock_green_catch',
      source: 'test',
      ciCommand: sharedCmd,
      faultSummary: 'fault catch',
      patch: 'catch.patch',
      matchers: [/expected_fault_token/],
    };
    const gateVacuous: GateRecord = {
      id: 'mock_green_vacuous',
      source: 'test',
      ciCommand: sharedCmd,
      faultSummary: 'fault vacuous',
      patch: 'vacuous.patch',
      matchers: [/0 matches/],
    };
    const gateUnmatched: GateRecord = {
      id: 'mock_green_unmatched',
      source: 'test',
      ciCommand: 'node scripts/check_other.mjs',
      faultSummary: 'fault unmatched',
      patch: 'unmatched.patch',
      matchers: [/expected_fault_token/],
    };
    const ctx = { captures: {}, tempFilesToClean: [], cleanCache: new Map<string, unknown>() };

    const resCatch = evaluateGate(gateCatch, ctx, {
      runClean: () => {
        cleanCalls += 1;
        return { status: 0, output: 'clean ok' };
      },
      runFault: () => {
        faultCalls += 1;
        return { status: 1, output: 'FAIL expected_fault_token in file.ts' };
      },
    }) as { verdict: string; exitCode: number; baselineExitCode: number; matched: boolean; namingLine: string };

    const resVacuous = evaluateGate(gateVacuous, ctx, {
      runClean: () => {
        cleanCalls += 1;
        return { status: 1, output: 'must not bypass cache' };
      },
      runFault: () => {
        faultCalls += 1;
        return { status: 0, output: '0 matches' };
      },
    }) as { verdict: string; exitCode: number; baselineExitCode: number; matched: boolean };

    const resUnmatched = evaluateGate(gateUnmatched, ctx, {
      runClean: () => {
        cleanCalls += 1;
        return { status: 0, output: 'clean ok' };
      },
      runFault: () => {
        faultCalls += 1;
        return { status: 101, output: 'error: unrelated host panic' };
      },
    }) as { verdict: string; exitCode: number; baselineExitCode: number; matched: boolean; namingLine: string };

    expect(cleanCalls).toBe(2);
    expect(faultCalls).toBe(3);

    expect(resCatch.verdict).toBe(VERDICTS.CATCHES);
    expect(resCatch.exitCode).toBe(1);
    expect(resCatch.baselineExitCode).toBe(0);
    expect(resCatch.matched).toBe(true);
    expect(resCatch.namingLine).toBe('FAIL expected_fault_token in file.ts');

    expect(resVacuous.verdict).toBe(VERDICTS.VACUOUS);
    expect(resVacuous.exitCode).toBe(0);
    expect(resVacuous.baselineExitCode).toBe(0);

    expect(resUnmatched.verdict).toBe(VERDICTS.UNMATCHED_FAILURE);
    expect(resUnmatched.exitCode).toBe(101);
    expect(resUnmatched.baselineExitCode).toBe(0);
    expect(resUnmatched.matched).toBe(false);
    expect(resUnmatched.namingLine).toBe('error: unrelated host panic');
  });

  it('defaultCleanKey isolates gates with distinct runClean overrides while sharing plain commands', () => {
    const plainA: GateRecord = {
      id: 'plain_a',
      source: 'test',
      ciCommand: 'node check.mjs',
      faultSummary: 'a',
      patch: 'a.patch',
    };
    const plainB: GateRecord = {
      id: 'plain_b',
      source: 'test',
      ciCommand: 'node check.mjs',
      faultSummary: 'b',
      patch: 'b.patch',
    };
    expect(defaultCleanKey(plainA)).toBe(defaultCleanKey(plainB));

    const customA: GateRecord = {
      ...plainA,
      id: 'custom_a',
      runClean: () => ({ status: 1, output: 'custom a red' }),
      matchers: [/a/],
    };
    const customB: GateRecord = {
      ...plainB,
      id: 'custom_b',
      runClean: () => ({ status: 0, output: 'custom b green' }),
      matchers: [/fault_b/],
    };
    expect(defaultCleanKey(customA)).not.toBe(defaultCleanKey(customB));

    const ctx = { captures: {}, tempFilesToClean: [], cleanCache: new Map<string, unknown>() };
    const rA = evaluateGate(customA, ctx, {
      runClean: (g: GateRecord) => g.runClean!(),
      runFault: () => ({ status: 1, output: 'a' }),
    }) as { verdict: string };
    const rB = evaluateGate(customB, ctx, {
      runClean: (g: GateRecord) => g.runClean!(),
      runFault: () => ({ status: 1, output: 'fault_b' }),
    }) as { verdict: string };

    expect(rA.verdict).toBe(VERDICTS.NOT_RUN_BASELINE_RED);
    expect(rB.verdict).toBe(VERDICTS.CATCHES);
  });

  it('cleanupStaleWorktree and unlinkNodeModules remove real directories without EPERM and preserve symlink targets', () => {
    const tmpRoot = mkdtempSync(join(os.tmpdir(), 'gate-controls-cleanup-'));
    try {
      const realTarget = join(tmpRoot, 'real-node-modules');
      mkdirSync(realTarget, { recursive: true });
      const sentinelFile = join(realTarget, 'keep.txt');
      writeFileSync(sentinelFile, 'do-not-delete', 'utf8');

      const linkType = process.platform === 'win32' ? 'junction' : 'dir';

      // 1. unlinkNodeModules handles both a symlink and a real node_modules/.vite directory
      const wtSym = join(tmpRoot, 'wt-sym');
      mkdirSync(wtSym, { recursive: true });
      const symPath = join(wtSym, 'node_modules');
      symlinkSync(realTarget, symPath, linkType);

      const wtDir = join(tmpRoot, 'wt-dir');
      const realNmDir = join(wtDir, 'node_modules');
      mkdirSync(join(realNmDir, '.vite'), { recursive: true });
      writeFileSync(join(realNmDir, '.vite', 'cache.json'), '{}', 'utf8');

      expect(() => unlinkNodeModules([symPath, realNmDir])).not.toThrow();
      expect(lstatSync(symPath, { throwIfNoEntry: false })).toBeUndefined();
      expect(lstatSync(realNmDir, { throwIfNoEntry: false })).toBeUndefined();
      expect(existsSync(sentinelFile)).toBe(true);

      // 2. cleanupStaleWorktree handles a real node_modules/.vite dir and a tests/node_modules symlink
      const staleWt = join(tmpRoot, 'stale-wt');
      mkdirSync(join(staleWt, 'node_modules', '.vite'), { recursive: true });
      writeFileSync(join(staleWt, 'node_modules', '.vite', 'vitest.json'), '{}', 'utf8');
      mkdirSync(join(staleWt, 'tests'), { recursive: true });
      symlinkSync(realTarget, join(staleWt, 'tests', 'node_modules'), linkType);

      expect(() => cleanupStaleWorktree(staleWt)).not.toThrow();
      expect(existsSync(staleWt)).toBe(false);
      expect(existsSync(sentinelFile)).toBe(true);
    } finally {
      rmSync(tmpRoot, { recursive: true, force: true });
    }
  });

  it('orders VACUOUS and UNMATCHED_FAILURE first, then NOT RUN, then CATCHES', () => {
    const summary = sortAndSummarizeResults([
      { id: 'c1', ciCommand: 'cmd', fault: 'f', exitCode: 1, namingLine: 'err', verdict: VERDICTS.CATCHES },
      {
        id: 'r1',
        ciCommand: 'cmd',
        fault: 'f',
        exitCode: 1,
        namingLine: 'red',
        verdict: VERDICTS.NOT_RUN_BASELINE_RED,
      },
      {
        id: 't1',
        ciCommand: 'cmd',
        fault: 'f',
        exitCode: '—',
        namingLine: 'time',
        verdict: VERDICTS.NOT_RUN_TIME,
      },
      {
        id: 'u1',
        ciCommand: 'cmd',
        fault: 'f',
        exitCode: 1,
        namingLine: 'wrong',
        verdict: VERDICTS.UNMATCHED_FAILURE,
      },
      { id: 'v1', ciCommand: 'cmd', fault: 'f', exitCode: 0, namingLine: 'ok', verdict: VERDICTS.VACUOUS },
    ]) as {
      ordered: Array<{ id: string }>;
      counts: Record<string, number>;
    };

    expect(summary.ordered.map((r) => r.id)).toEqual(['v1', 'u1', 'r1', 't1', 'c1']);
    expect(summary.counts).toEqual({
      total: 5,
      vacuous: 1,
      unmatchedFailure: 1,
      catches: 1,
      notRunNoGate: 0,
      notRunTime: 1,
      notRunBaselineRed: 1,
    });

    const md = formatMarkdownTable(summary) as string;
    expect(md).toContain('UNMATCHED_FAILURE=1');
    expect(md).toContain('NOT_RUN_BASELINE_RED=1');
  });

  it('registers all trunk CI gates with valid fault patches and honest matchers', () => {
    const problems = validateGateRegistry(GATES, { faultsDir: FAULTS_DIR }) as string[];
    expect(problems).toEqual([]);

    const registeredIds = new Set((GATES as GateRecord[]).map((g) => g.id));
    for (const requiredId of [
      'thread_rng_scan',
      'thread_rng_scan_crates',
      'cargo_clippy_worldgen',
      'feature_separation',
      'feature_separation_engine',
      'feature_separation_worldgen',
      'cargo_test_worldgen',
      'check_test_targets_worldgen_unscheduled',
      'check_test_targets_worldgen_empty',
    ]) {
      expect(registeredIds.has(requiredId)).toBe(true);
    }

    for (const gate of GATES as GateRecord[]) {
      if (gate.staticResult) {
        expect(gate.patch).toBeNull();
      } else {
        expect(typeof gate.patch).toBe('string');
        expect(existsSync(join(FAULTS_DIR, gate.patch as string))).toBe(true);
        expect(Array.isArray(gate.matchers) && gate.matchers.length > 0).toBe(true);
      }
    }
  });

  it('passes built-in --self-test controls and CLI argument parsing', () => {
    const cli = parseCliArgs(['--gate', 'cargo_fmt,check_e3', '--out', 'out.md']) as {
      gateIds: string[];
      listOnly: boolean;
      selfTest: boolean;
      mdOut: string | null;
    };
    expect(cli.gateIds).toEqual(['cargo_fmt', 'check_e3']);
    expect(cli.listOnly).toBe(false);
    expect(cli.selfTest).toBe(false);
    expect(cli.mdOut).not.toBeNull();
    expect(() => parseCliArgs(['--unknown-flag'])).toThrow(/Unknown or incomplete option/);

    const st = runSelfTest() as { failures: string[]; controls: number };
    expect(st.failures).toEqual([]);
    expect(st.controls).toBeGreaterThanOrEqual(GATES.length);

    const out = execFileSync(process.execPath, [join(REPO_ROOT, 'scripts', 'paper', 'gate_controls.mjs'), '--list'], {
      cwd: REPO_ROOT,
      encoding: 'utf8',
    });
    expect(out).toContain('cargo_clippy_worldgen');
    expect(out).toContain('feature_separation_engine');
  });
});
