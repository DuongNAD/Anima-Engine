import { describe, expect, it } from 'vitest';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
// @ts-expect-error -- verify_e3_artifacts.mjs has no .d.mts declaration inside W13 authorized scope
import { buildSyntheticE3Data, classifyE3Decision, evaluateE3Dataset, isAllowedDirtyPath, loadCommittedPrereg, pairedStats, rehashBundle, verifyArtifactDir, writeRecomputedBundleFiles, writeSyntheticSmokeDir } from '../../scripts/verify_e3_artifacts.mjs';

interface PairedStats {
  n_requested: number;
  n_complete_pairs: number;
  control_mean: number | null;
  treatment_mean: number | null;
  paired_mean_delta: number | null;
  paired_sd: number | null;
  paired_se: number | null;
  ci95_low: number | null;
  ci95_high: number | null;
  paired_dz: number | null;
  median_of_per_seed_deltas: number | null;
  between_seed_variance_of_deltas: number | null;
}

interface DecisionResult {
  decision: string;
  zero_control_mean_fallback_used: boolean;
}

describe('verify_e3_artifacts — preregistration threshold boundaries, Bessel, per-contrast exclusions, & tamper checks', () => {
  const prereg = loadCommittedPrereg();
  const t = prereg.thresholds;

  const handEffect = (
    n: number,
    delta: number,
    lo: number | null,
    hi: number | null,
    dz: number | null,
    controlMean = 10.0,
  ): PairedStats => ({
    n_requested: 12,
    n_complete_pairs: n,
    control_mean: controlMean,
    treatment_mean: controlMean + delta,
    paired_mean_delta: delta,
    paired_sd: dz !== null && dz !== 0 ? Math.abs(delta / dz) : 0,
    paired_se: 0.1,
    ci95_low: lo,
    ci95_high: hi,
    paired_dz: dz,
    median_of_per_seed_deltas: delta,
    between_seed_variance_of_deltas: 0.1,
  });

  it('reads all thresholds dynamically from e3-preregistration.json and classifies every decision boundary identically to Rust prereg tests', () => {
    expect(t.min_valid_pairs_for_decision).toBe(10);
    expect(t.dz_min).toBe(0.8);
    expect(t.relative_delta_min).toBe(0.05);
    expect(t.decision_enum).toEqual([
      'material_positive',
      'material_negative_opposite',
      'between_detectable_below_materiality',
      'null',
      'insufficient_evidence',
    ]);

    // 1. Minimum valid pairs: 9 vs 10.
    expect((classifyE3Decision(handEffect(9, 2.0, 1.5, 2.5, 2.0), t) as DecisionResult).decision).toBe(
      'insufficient_evidence',
    );
    expect((classifyE3Decision(handEffect(10, 2.0, 1.5, 2.5, 2.0), t) as DecisionResult).decision).toBe(
      'material_positive',
    );

    // 2. CI95 boundary around 0 (including exact 0.0 and null).
    expect((classifyE3Decision(handEffect(12, 1.0, -0.01, 2.01, 0.9), t) as DecisionResult).decision).toBe('null');
    expect((classifyE3Decision(handEffect(12, 1.0, 0.0, 2.0, 0.9), t) as DecisionResult).decision).toBe('null');
    expect((classifyE3Decision(handEffect(12, 1.0, null, null, 0.9), t) as DecisionResult).decision).toBe('null');
    expect((classifyE3Decision(handEffect(12, 1.0, 0.01, 1.99, 0.9), t) as DecisionResult).decision).toBe(
      'material_positive',
    );
    expect((classifyE3Decision(handEffect(12, -1.0, -1.99, -0.01, -0.9), t) as DecisionResult).decision).toBe(
      'material_negative_opposite',
    );

    // 3. Cohen dz boundary around 0.8 (0.7999 vs 0.80 positive/negative, null dz).
    expect((classifyE3Decision(handEffect(12, 1.0, 0.2, 1.8, 0.7999), t) as DecisionResult).decision).toBe(
      'between_detectable_below_materiality',
    );
    expect((classifyE3Decision(handEffect(12, 1.0, 0.2, 1.8, 0.8), t) as DecisionResult).decision).toBe(
      'material_positive',
    );
    expect((classifyE3Decision(handEffect(12, -1.0, -1.8, -0.2, -0.7999), t) as DecisionResult).decision).toBe(
      'between_detectable_below_materiality',
    );
    expect((classifyE3Decision(handEffect(12, -1.0, -1.8, -0.2, -0.8), t) as DecisionResult).decision).toBe(
      'material_negative_opposite',
    );
    expect((classifyE3Decision(handEffect(12, 1.0, 0.5, 1.5, null), t) as DecisionResult).decision).toBe(
      'between_detectable_below_materiality',
    );

    // 4. Relative shift boundary around 0.05 * |control_mean| (0.50 on control_mean = 10.0).
    expect((classifyE3Decision(handEffect(12, 0.4999, 0.2, 0.8, 1.2), t) as DecisionResult).decision).toBe(
      'between_detectable_below_materiality',
    );
    expect((classifyE3Decision(handEffect(12, 0.5, 0.2, 0.8, 1.2), t) as DecisionResult).decision).toBe(
      'material_positive',
    );
    expect((classifyE3Decision(handEffect(12, -0.4999, -0.8, -0.2, -1.2), t) as DecisionResult).decision).toBe(
      'between_detectable_below_materiality',
    );
    expect((classifyE3Decision(handEffect(12, -0.5, -0.8, -0.2, -1.2), t) as DecisionResult).decision).toBe(
      'material_negative_opposite',
    );

    // 5. Mutating thresholds dynamically changes the outcome (zero hard-coded thresholds).
    const mutated = { ...t, dz_min: 1.5, relative_delta_min: 0.1 };
    expect((classifyE3Decision(handEffect(12, 0.8, 0.2, 1.4, 1.1), mutated) as DecisionResult).decision).toBe(
      'between_detectable_below_materiality',
    );

    // 6. S3 zero control_mean fallback rule.
    const s3Zero = classifyE3Decision(handEffect(12, 0.4, 0.1, 0.7, 1.2, 0.0), t, {
      allowZeroControlFallback: true,
    }) as DecisionResult;
    expect(s3Zero.decision).toBe('material_positive');
    expect(s3Zero.zero_control_mean_fallback_used).toBe(true);

    // 7. Paired stats exact NIST Student-t round-trip.
    const ps = pairedStats(
      Array(10).fill(10.0),
      [10.55, 10.65, 10.55, 10.65, 10.55, 10.65, 10.55, 10.65, 10.55, 10.65],
      12,
    ) as PairedStats;
    expect(ps.n_complete_pairs).toBe(10);
    expect(ps.paired_mean_delta).toBeCloseTo(0.6, 12);
    expect((classifyE3Decision(ps, t) as DecisionResult).decision).toBe('material_positive');
  });

  it('computes Bessel sensitivity, tail window, empty-half S3 exclusion, and per-contrast extinction isolation cleanly', () => {
    const extSeeds = prereg.seeds.execution_order.slice(0, 3);
    const data = buildSyntheticE3Data(prereg, { extinctControlUniformSeeds: extSeeds });
    // Force seed 741001 in treatment_gradient to have singleton in tail window (tick 15000 < 2) while T=18000 stays valid,
    // and empty west half on seed 740001 in treatment_gradient at T=18000.
    for (const s of data.series) {
      if (s.seed === 741001 && s.arm === 'treatment_gradient' && s.tick === 15000) {
        s['live.agent_count'] = 1;
      }
    }
    const fEmptyHalf = data.finals.find((f: { seed: number; arm: string }) => f.seed === 740001 && f.arm === 'treatment_gradient');
    fEmptyHalf.final_observables['live.trait_mass_mean_west'] = 0;

    const evaluated = evaluateE3Dataset({
      runs: data.runs,
      series: data.series,
      finalObservables: data.finals,
      prereg,
    });

    const primary = evaluated.contrasts['treatment_gradient - control_uniform_scarcity'];
    // U extinction on 3 seeds does NOT touch H1 (T - S): all 12 pairs stay valid!
    expect(primary.H1.n_complete_pairs).toBe(12);
    expect(primary.H1.n_extinct_pairs_dropped).toBe(0);
    expect(primary.H1.decision).toBe('material_positive');

    // Bessel sensitivity scales var * n / (n - 1) on same 12 valid pairs.
    expect(primary.H1_bessel_sensitivity.n_complete_pairs).toBe(12);
    expect(primary.H1_bessel_sensitivity.control_mean).not.toBeCloseTo(primary.H1.control_mean, 4);

    // Tail window excludes seed 741001 because tail had agent_count=1 at tick 15000.
    expect(primary.H1_tail_window.n_complete_pairs).toBe(11);
    expect(primary.H1_tail_window.n_singleton_pairs_dropped).toBe(1);

    // S3 drops only the empty-half pair (seed 740001 -> 11 pairs) without affecting H1/S1/S2.
    expect(primary.S3.n_complete_pairs).toBe(11);
    expect(primary.S3.n_empty_half_pairs_dropped_s3).toBe(1);
    expect(primary.S2.n_complete_pairs).toBe(12);

    // U-involving contrasts drop the 3 extinct seeds -> insufficient_evidence (< 10 pairs).
    const cTU = evaluated.contrasts['treatment_gradient - control_uniform'];
    const cSU = evaluated.contrasts['control_uniform_scarcity - control_uniform'];
    expect(cTU.H1.n_complete_pairs).toBe(9);
    expect(cTU.H1.decision).toBe('insufficient_evidence');
    expect(cSU.H1.n_complete_pairs).toBe(9);
    expect(cSU.H1.decision).toBe('insufficient_evidence');

    // Additive decomposition runs strictly on the 9 seeds valid in all 3 arms with zero residual.
    expect(evaluated.additive_decomposition_table_on_all_three_valid_seeds).toHaveLength(9);
    for (const row of evaluated.additive_decomposition_table_on_all_three_valid_seeds) {
      expect(Math.abs(row.residual)).toBeLessThan(1e-12);
    }
  });

  it('rejects empty dirs, leaked smoke_seed, debug profile, missing world_artifact prerun check, and checksum/data tampering', () => {
    const root = mkdtempSync(join(tmpdir(), 'e3-vitest-'));
    const mFiles = [
      'e3-preregistration.json',
      prereg.manifests.control,
      prereg.manifests.control_uniform_scarcity,
      prereg.manifests.treatment,
    ];
    try {
      const emptyDir = join(root, 'empty');
      mkdirSync(emptyDir, { recursive: true });
      const emptyRes = verifyArtifactDir(emptyDir);
      expect(emptyRes.ok).toBe(false);
      expect(emptyRes.failures.some((m: string) => m.includes('empty'))).toBe(true);
      const bundleDir = join(root, 'bundle');
      const data = buildSyntheticE3Data(prereg);
      writeRecomputedBundleFiles(bundleDir, prereg, data);
      expect(verifyArtifactDir(bundleDir).ok).toBe(true);

      // Tamper provenance: debug profile & missing world_artifact_prerun_check -> rejected after rehash.
      const provPath = join(bundleDir, 'provenance.json');
      const prov = JSON.parse(readFileSync(provPath, 'utf8'));
      prov.build.profile = 'debug';
      prov.world_artifact_prerun_check.anima_world_artifact_unset = false;
      writeFileSync(provPath, `${JSON.stringify(prov, null, 2)}\n`);
      rehashBundle(bundleDir, mFiles);
      const badProv = verifyArtifactDir(bundleDir);
      expect(badProv.ok).toBe(false);
      expect(badProv.failures.some((m: string) => m.includes('release'))).toBe(true);
      expect(badProv.failures.some((m: string) => m.includes('world_artifact_prerun_check'))).toBe(true);

      // Smoke seed leak in runs.csv rejected even after rehash.
      const smokeDir = join(root, 'smoke_leak');
      const smokeData = buildSyntheticE3Data(prereg);
      smokeData.runs[0].seed = prereg.seeds.smoke_seed;
      writeRecomputedBundleFiles(smokeDir, prereg, {
        ...smokeData,
        // keep evaluateE3Dataset happy inside writeRecomputedBundleFiles by restoring valid runs then swapping csv
        runs: buildSyntheticE3Data(prereg).runs,
      });
      const runsCsv = readFileSync(join(smokeDir, 'runs.csv'), 'utf8').replace(
        String(prereg.seeds.execution_order[0]),
        String(prereg.seeds.smoke_seed),
      );
      writeFileSync(join(smokeDir, 'runs.csv'), runsCsv);
      rehashBundle(smokeDir, mFiles);
      const leakRes = verifyArtifactDir(smokeDir);
      expect(leakRes.ok).toBe(false);
      expect(leakRes.failures.some((m: string) => m.includes('smoke_seed'))).toBe(true);

      expect(typeof emptyDir).toBe('string');
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it('R1: rejects re-hashed tampering of reporting mediator/exclusion/failed_runs tables element by element', () => {
    const root = mkdtempSync(join(tmpdir(), 'e3-vitest-r1-'));
    const mFiles = [
      'e3-preregistration.json',
      prereg.manifests.control,
      prereg.manifests.control_uniform_scarcity,
      prereg.manifests.treatment,
    ];
    interface MutableEffectsDoc {
      agent_count_at_T_by_arm: Record<string, Array<{ seed: number; agent_count: number }>>;
      per_arm_empty_half_counts: Record<string, number>;
      failed_runs: Array<{ seed: number; arm: string; tick: number; reason: string }>;
      extinction_table_by_arm: Array<Record<string, unknown>>;
      singleton_table_by_arm: Array<Record<string, unknown>>;
      empty_half_table_by_arm: Array<Record<string, unknown>>;
      additive_decomposition_table_on_all_three_valid_seeds: Array<Record<string, number>>;
    }
    const editEffects = (dir: string, mutator: (doc: MutableEffectsDoc) => void) => {
      const p = join(dir, 'effects.json');
      const doc = JSON.parse(readFileSync(p, 'utf8')) as MutableEffectsDoc;
      mutator(doc);
      writeFileSync(p, `${JSON.stringify(doc, null, 2)}\n`);
      rehashBundle(dir, mFiles);
      return verifyArtifactDir(dir);
    };

    try {
      // 1. Mediator agent_count_at_T_by_arm tampered.
      const d1 = join(root, 'r1_agent_count');
      writeRecomputedBundleFiles(d1, prereg, buildSyntheticE3Data(prereg));
      const r1 = editEffects(d1, (j) => {
        j.agent_count_at_T_by_arm.control_uniform_scarcity[0].agent_count = 999;
      });
      expect(r1.ok).toBe(false);
      expect(r1.failures.some((m: string) => m.includes('agent_count_at_T_by_arm.control_uniform_scarcity'))).toBe(true);

      // 2. per_arm_empty_half_counts & failed_runs tampered.
      const d2 = join(root, 'r1_eh_failed');
      writeRecomputedBundleFiles(d2, prereg, buildSyntheticE3Data(prereg));
      const r2 = editEffects(d2, (j) => {
        j.per_arm_empty_half_counts.treatment_gradient = 5;
        j.failed_runs = [{ seed: 1, arm: 'control_uniform', tick: 1, reason: 'fake' }];
      });
      expect(r2.ok).toBe(false);
      expect(r2.failures.some((m: string) => m.includes('per_arm_empty_half_counts.treatment_gradient'))).toBe(true);
      expect(r2.failures.some((m: string) => m.includes('failed_runs'))).toBe(true);

      // 3. Same-length rewritten extinction_table_by_arm, singleton_table_by_arm, empty_half_table_by_arm, & additive col.
      const extSeeds = prereg.seeds.execution_order.slice(0, 2);
      const data3 = buildSyntheticE3Data(prereg, { extinctControlUniformSeeds: extSeeds });
      const tailSeed = Number(prereg.seeds.execution_order[4]);
      const ehSeed = Number(prereg.seeds.execution_order[5]);
      for (const s of data3.series) {
        if (s.seed === tailSeed && s.arm === 'treatment_gradient' && s.tick === 15000) {
          s['live.agent_count'] = 1;
        }
      }
      const ehFinal = data3.finals.find((f: { seed: number; arm: string }) => f.seed === ehSeed && f.arm === 'treatment_gradient');
      ehFinal.final_observables['live.trait_mass_mean_west'] = 0;

      const d3 = join(root, 'r1_excl_tables');
      writeRecomputedBundleFiles(d3, prereg, data3);
      const r3 = editEffects(d3, (j) => {
        j.extinction_table_by_arm[0] = { seed: 1, arm: 'control_uniform', first_extinct_tick: 1 };
        j.singleton_table_by_arm[0] = {
          seed: 1,
          arm: 'treatment_gradient',
          tick: 1,
          agent_count: 1,
          scope: 'final_T',
        };
        j.empty_half_table_by_arm[0] = {
          seed: 1,
          arm: 'treatment_gradient',
          tick: 1,
          empty_half: 'east',
        };
        j.additive_decomposition_table_on_all_three_valid_seeds[0].treatment_gradient += 2.0;
      });
      expect(r3.ok).toBe(false);
      expect(r3.failures.some((m: string) => m.includes('extinction_table_by_arm[0]'))).toBe(true);
      expect(r3.failures.some((m: string) => m.includes('singleton_table_by_arm[0]'))).toBe(true);
      expect(r3.failures.some((m: string) => m.includes('empty_half_table_by_arm[0]'))).toBe(true);
      expect(
        r3.failures.some((m: string) => m.includes('additive_decomposition_table_on_all_three_valid_seeds[0]')),
      ).toBe(true);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it('N1: clamps float noise on identical non-integer deltas to sd = 0 and matches Rust in full bundle verification', () => {
    for (const [ctrlVal, treatVal] of [
      [3.0, 1.1],
      [1.0, 1.1],
    ]) {
      const ps = pairedStats(Array(12).fill(ctrlVal), Array(12).fill(treatVal), 12) as PairedStats;
      expect(ps.paired_sd).toBe(0);
      expect(ps.paired_se).toBe(0);
      expect(ps.paired_dz).toBeNull();
      expect(ps.between_seed_variance_of_deltas).toBe(0);
      expect(ps.ci95_low).toBe(ps.ci95_high);
      expect((classifyE3Decision(ps, t) as DecisionResult).decision).toBe('between_detectable_below_materiality');
    }

    const root = mkdtempSync(join(tmpdir(), 'e3-vitest-n1-'));
    try {
      const data = buildSyntheticE3Data(prereg);
      for (const s of data.series) {
        if (s.arm === 'control_uniform_scarcity') s['live.trait_mass_var'] = 3.0;
        if (s.arm === 'treatment_gradient') s['live.trait_mass_var'] = 1.1;
      }
      for (const f of data.finals) {
        if (f.arm === 'control_uniform_scarcity') f.final_observables['live.trait_mass_var'] = 3.0;
        if (f.arm === 'treatment_gradient') f.final_observables['live.trait_mass_var'] = 1.1;
      }
      const d = join(root, 'n1_bundle');
      const eff = writeRecomputedBundleFiles(d, prereg, data);
      const h1 = eff.contrasts['treatment_gradient - control_uniform_scarcity'].H1;
      expect(h1.paired_sd).toBe(0);
      expect(h1.paired_dz).toBeNull();
      expect(h1.decision).toBe('between_detectable_below_materiality');
      expect(verifyArtifactDir(d).ok).toBe(true);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it('N2, N3, N4: enforces host smoke checksums/WP-2 gates, dirty paths under artifacts/ only, and commit-pinned manifest resolution', () => {
    const root = mkdtempSync(join(tmpdir(), 'e3-vitest-n234-'));
    const repoRoot = resolve(__dirname, '..', '..');
    const headCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repoRoot }).toString('utf8').trim();
    const mFiles = [
      'e3-preregistration.json',
      prereg.manifests.control,
      prereg.manifests.control_uniform_scarcity,
      prereg.manifests.treatment,
    ];

    try {
      // N3 path validator cases.
      expect(isAllowedDirtyPath('artifacts/experiments/e3-productivity-gradient-body-mass/host-smoke/')).toBe(true);
      expect(isAllowedDirtyPath('src-tauri/src/core/live_experiment.rs')).toBe(false);
      expect(isAllowedDirtyPath('artifacts/../src-tauri/src/lib.rs')).toBe(false);
      expect(isAllowedDirtyPath('')).toBe(false);

      // N2: tampered host smoke checksums.sha256 and failed WP-2 rejected.
      const smokeOkDir = writeSyntheticSmokeDir(join(root, 'smoke_ok'), {
        commit: headCommit,
        hostOs: 'windows',
        hostArch: 'x86_64',
      });
      const headBundle = join(root, 'n4_head');
      writeRecomputedBundleFiles(
        headBundle,
        prereg,
        buildSyntheticE3Data(prereg, {
          gitCommit: headCommit,
          smokeDir: smokeOkDir,
          gitDirtyPaths: ['artifacts/experiments/e3-productivity-gradient-body-mass/host-smoke/'],
        }),
        { manifestGitRef: headCommit },
      );
      const okRes = verifyArtifactDir(headBundle);
      expect(okRes.ok).toBe(true);
      expect(okRes.notes).toEqual([]);

      // Mutate smoke_ok/checksums.sha256 directly -> rejected by N2.
      writeFileSync(
        join(smokeOkDir, 'checksums.sha256'),
        `${'0'.repeat(64)}  provenance.json\n${'0'.repeat(64)}  smoke-report.json\n`,
      );
      const badSmokeCksum = verifyArtifactDir(headBundle);
      expect(badSmokeCksum.ok).toBe(false);
      expect(badSmokeCksum.failures.some((m: string) => m.includes('SHA-256 mismatch'))).toBe(true);

      // Failed WP-2 gate inside host smoke -> rejected by N2.
      const smokeFailWp2 = writeSyntheticSmokeDir(join(root, 'smoke_fail_wp2'), {
        commit: headCommit,
        wp2Passed: false,
      });
      const wp2Bundle = join(root, 'n2_wp2_fail');
      writeRecomputedBundleFiles(
        wp2Bundle,
        prereg,
        buildSyntheticE3Data(prereg, { gitCommit: headCommit, smokeDir: smokeFailWp2 }),
        { manifestGitRef: headCommit },
      );
      const wp2Res = verifyArtifactDir(wp2Bundle);
      expect(wp2Res.ok).toBe(false);
      expect(wp2Res.failures.some((m: string) => m.includes('smoke-report.json gate failed'))).toBe(true);

      // N3: dirty path outside artifacts/ or traversal rejected in bundle provenance.
      const dirtyBadBundle = join(root, 'n3_bad');
      const okSmoke2 = writeSyntheticSmokeDir(join(root, 'smoke_ok2'), { commit: headCommit });
      writeRecomputedBundleFiles(
        dirtyBadBundle,
        prereg,
        buildSyntheticE3Data(prereg, {
          gitCommit: headCommit,
          smokeDir: okSmoke2,
          gitDirtyPaths: ['artifacts/../src-tauri/tests/fixtures/experiments_e3/e3-treatment-gradient.json'],
        }),
        { manifestGitRef: headCommit },
      );
      const dirtyRes = verifyArtifactDir(dirtyBadBundle);
      expect(dirtyRes.ok).toBe(false);
      expect(dirtyRes.failures.some((m: string) => m.includes('disallowed dirty working tree path'))).toBe(true);

      // N4: unavailable recorded commit falls back to provenance.manifests.sha256_as_committed with explicit note.
      const fallbackBundle = join(root, 'n4_fallback');
      writeRecomputedBundleFiles(fallbackBundle, prereg, buildSyntheticE3Data(prereg));
      const fallbackRes = verifyArtifactDir(fallbackBundle);
      expect(fallbackRes.ok).toBe(true);
      expect(
        fallbackRes.notes.some((n: string) =>
          n.includes('fell back to recorded provenance.manifests.sha256_as_committed'),
        ),
      ).toBe(true);

      // N4: pre-amendment prereg copy differing from HEAD verifies against recorded sha256_as_committed.
      const editJsonFile = (
        filePath: string,
        mutator: (doc: Record<string, unknown>) => void,
      ): void => {
        const doc = JSON.parse(readFileSync(filePath, 'utf8')) as Record<string, unknown>;
        mutator(doc);
        writeFileSync(filePath, `${JSON.stringify(doc, null, 2)}\n`);
      };
      const priorAmendmentBundle = join(root, 'n4_prior_amendment');
      writeRecomputedBundleFiles(priorAmendmentBundle, prereg, buildSyntheticE3Data(prereg));
      const priorPreregPath = join(priorAmendmentBundle, 'manifests', 'e3-preregistration.json');
      editJsonFile(priorPreregPath, (doc) => {
        const defaults = doc.default_keys_left_absent as Record<string, number>;
        delete defaults['live.founder_layout'];
      });
      const priorPreregSha = createHash('sha256')
        .update(readFileSync(priorPreregPath, 'utf8').replace(/\r\n/g, '\n'), 'utf8')
        .digest('hex');
      editJsonFile(join(priorAmendmentBundle, 'provenance.json'), (doc) => {
        const manifests = doc.manifests as { sha256_as_committed: Record<string, string> };
        manifests.sha256_as_committed['e3-preregistration.json'] = priorPreregSha;
      });
      rehashBundle(priorAmendmentBundle, mFiles);
      const priorAmendmentRes = verifyArtifactDir(priorAmendmentBundle);
      expect(priorAmendmentRes.ok).toBe(true);
      expect(
        priorAmendmentRes.notes.some((n: string) =>
          n.includes('fell back to recorded provenance.manifests.sha256_as_committed'),
        ),
      ).toBe(true);

      // N4 negative control: tampered prereg copy mismatching recorded sha256_as_committed fails.
      const tamperedCopyBundle = join(root, 'n4_tampered_copy');
      writeRecomputedBundleFiles(tamperedCopyBundle, prereg, buildSyntheticE3Data(prereg));
      editJsonFile(join(tamperedCopyBundle, 'manifests', 'e3-preregistration.json'), (doc) => {
        const defaults = doc.default_keys_left_absent as Record<string, number>;
        defaults['live.founder_layout'] = 1;
      });
      rehashBundle(tamperedCopyBundle, mFiles);
      const tamperedCopyRes = verifyArtifactDir(tamperedCopyBundle);
      expect(tamperedCopyRes.ok).toBe(false);
      expect(tamperedCopyRes.failures.some((m: string) => m.includes('sha256_as_committed'))).toBe(true);

      // N4 negative control: unavailable commit missing provenance.manifests.sha256_as_committed fails.
      const missingShaBundle = join(root, 'n4_missing_sha');
      writeRecomputedBundleFiles(missingShaBundle, prereg, buildSyntheticE3Data(prereg));
      editJsonFile(join(missingShaBundle, 'provenance.json'), (doc) => {
        const manifests = doc.manifests as Record<string, unknown>;
        delete manifests.sha256_as_committed;
      });
      rehashBundle(missingShaBundle, mFiles);
      const missingShaRes = verifyArtifactDir(missingShaBundle);
      expect(missingShaRes.ok).toBe(false);
      expect(missingShaRes.failures.some((m: string) => m.includes('sha256_as_committed'))).toBe(true);

      // N4: when older commit 0dc00fb exists locally (non-shallow repo), pre-amendment prereg blobs verify cleanly.
      let hasOldCommit = false;
      try {
        execFileSync('git', ['cat-file', '-e', '0dc00fb:src-tauri/tests/fixtures/experiments_e3/e3-preregistration.json'], {
          cwd: repoRoot,
          stdio: ['ignore', 'ignore', 'ignore'],
        });
        hasOldCommit = true;
      } catch {
        hasOldCommit = false;
      }
      if (hasOldCommit) {
        const oldCommit = execFileSync('git', ['rev-parse', '0dc00fb'], { cwd: repoRoot }).toString('utf8').trim();
        const oldPrereg = loadCommittedPrereg(null, oldCommit);
        const oldSmoke = writeSyntheticSmokeDir(join(root, 'smoke_old'), { commit: oldCommit });
        const oldBundle = join(root, 'n4_old');
        writeRecomputedBundleFiles(
          oldBundle,
          oldPrereg,
          buildSyntheticE3Data(oldPrereg, { gitCommit: oldCommit, smokeDir: oldSmoke }),
          { manifestGitRef: oldCommit },
        );
        const oldRes = verifyArtifactDir(oldBundle);
        expect(oldRes.ok).toBe(true);
        expect(oldRes.notes).toEqual([]);
        expect(oldPrereg.default_keys_left_absent['live.strict_prey_cap']).toBeUndefined();
        expect(prereg.default_keys_left_absent['live.strict_prey_cap']).toBe(0);
      }
      expect(mFiles).toHaveLength(4);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
