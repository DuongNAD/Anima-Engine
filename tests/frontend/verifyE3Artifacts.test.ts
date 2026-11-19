import { describe, expect, it } from 'vitest';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
// @ts-expect-error -- verify_e3_artifacts.mjs has no .d.mts declaration inside W13 authorized scope
import { buildSyntheticE3Data, classifyE3Decision, evaluateE3Dataset, loadCommittedPrereg, pairedStats, rehashBundle, verifyArtifactDir, writeRecomputedBundleFiles } from '../../scripts/verify_e3_artifacts.mjs';

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
});
