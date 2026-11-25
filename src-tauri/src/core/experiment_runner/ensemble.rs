//! Ensembles of independent seeds and paired same-seed control/treatment ensembles (AE-108 / AE-S14).

use super::*;

// ---- Ensemble (AE-108 / AE-S14) --------------------------------------------------------------

/// The inferential procedure used to produce a report's two-sided 95% confidence interval.
///
/// This travels with JSON artifacts so a result remains interpretable without knowing which
/// executable version produced it. Enclosing reports use `Option<ConfidenceIntervalMethod>` so
/// legacy artifacts that predate this metadata continue to deserialize as `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceIntervalMethod {
    StudentTConservativeTable,
    PairedStudentTConservativeTable,
    WelchStudentTConservativeTable,
}

/// A per-observable statistical summary over an ensemble's completed runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricSummary {
    pub observable: String,
    pub n: usize,
    pub mean: f64,
    /// Sample standard deviation; `None` when fewer than two samples completed.
    pub std: Option<f64>,
    pub min: f64,
    pub max: f64,
    /// 95% Student-*t* confidence interval on the mean (`df = n − 1`).
    pub ci95_low: Option<f64>,
    pub ci95_high: Option<f64>,
    /// Procedure used for the interval; `None` in legacy or non-estimable artifacts.
    pub ci95_method: Option<ConfidenceIntervalMethod>,
}

/// The result of a seed-set ensemble: every requested run preserved (including failures), a
/// deterministic seed order, and per-observable summaries over the completed runs (ER11; **AE-S14
/// PARTIAL** — no control–treatment effect-size field exists yet).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnsembleSummary {
    pub experiment_id: String,
    pub manifest_fingerprint: u64,
    pub requested_runs: usize,
    pub completed_runs: usize,
    /// `(seed, reason)` for every failed run — never silently dropped.
    pub failed: Vec<(u64, String)>,
    /// The seed order runs were executed in (as listed in the manifest).
    pub seed_order: Vec<u64>,
    pub runs: Vec<RunResult>,
    pub metrics: Vec<MetricSummary>,
}

/// Run every seed of `manifest` as an independent run, in manifest-listed order, preserving failures.
///
/// The registry and manifest are validated **once, up front**: an invalid registry or manifest — which
/// includes an **empty seed set** ([`ExperimentError::EmptySeeds`]) — is a structured ensemble-level
/// error, not a misleading zero-run "summary". Per-seed *runtime* failures are still preserved inside
/// the returned [`EnsembleSummary`] (they are not ensemble-level errors). No fabricated seed is ever
/// introduced.
pub fn run_ensemble<M: ExperimentModel>(
    manifest: &ExperimentManifest,
    registry: &ObservableRegistry,
) -> Result<EnsembleSummary, ExperimentError> {
    registry.validate()?;
    manifest.validate(registry)?; // rejects empty/duplicate seeds, unknown observables, etc.

    let seed_order = manifest.seeds.clone();
    let mut runs = Vec::with_capacity(seed_order.len());
    let mut failed = Vec::new();

    for &seed in &seed_order {
        let res = run_manifest_seed::<M>(manifest, registry, seed, None, None);
        if let RunStatus::Failed { reason, .. } = &res.status {
            failed.push((seed, reason.clone()));
        }
        runs.push(res);
    }

    let completed: Vec<&RunResult> = runs.iter().filter(|r| r.status.is_completed()).collect();
    let metrics = summarize_metrics(&completed)?;

    Ok(EnsembleSummary {
        experiment_id: manifest.experiment_id.clone(),
        manifest_fingerprint: manifest.fingerprint(),
        requested_runs: seed_order.len(),
        completed_runs: completed.len(),
        failed,
        seed_order,
        runs,
        metrics,
    })
}

// ---- PAIRED control/treatment ensemble (AE-S14) ----------------------------------------------

/// One same-seed control/treatment pair. Both `RunResult`s are always kept — including a **one-sided
/// failure**, where one branch completed and the other did not — so a half-pair is never silently
/// dropped (ER11).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeedPair {
    pub seed: u64,
    pub control: RunResult,
    pub treatment: RunResult,
}

impl SeedPair {
    /// Whether **both** sides completed. Only complete pairs contribute to effect statistics.
    pub fn is_complete(&self) -> bool {
        self.control.status.is_completed() && self.treatment.status.is_completed()
    }
}

/// A **paired** (same-seed, within-subject) effect for one observable.
///
/// # Defined-ness contract
///
/// Every statistic is `Option<f64>` so the report can serialize to JSON without ever emitting
/// `NaN`/`Infinity` (which `serde_json` renders as `null` and cannot parse back into an `f64`).
/// Exactly:
///
/// | complete pairs `n` | means / `paired_mean_delta` | `paired_sd` / `paired_se` / CI | `paired_dz` |
/// |---|---|---|---|
/// | `0` | `None` | `None` | `None` |
/// | `1` | `Some` | `None` (no spread is estimable from one pair) | `None` |
/// | `≥2`, deltas vary | `Some` | `Some` | `Some` |
/// | `≥2`, all deltas identical (zero paired variance) | `Some` | `Some(0.0)` / `Some(0.0)` / zero-width `Some` | **`None`** — `dz = delta / 0` is undefined |
///
/// `paired_mean_delta` (treatment − control, in the observable's own unit) is the **primary effect**.
/// `paired_dz` is Cohen's *d_z* — the mean paired delta in units of the paired-delta SD.
///
/// This is a descriptive statistic over a seed ensemble, **not** a significance test, and — because
/// AE1–AE2.5 contain no pathway, reproduction or selection — **not** evidence of adaptation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PairedEffect {
    pub observable: String,
    /// Pairs requested (one per declared seed), whether or not they completed.
    pub n_requested: usize,
    /// Pairs where BOTH sides completed and both emitted this observable.
    pub n_complete_pairs: usize,
    pub control_mean: Option<f64>,
    pub treatment_mean: Option<f64>,
    /// Mean of the per-seed `treatment − control` deltas — the primary paired effect.
    pub paired_mean_delta: Option<f64>,
    /// Sample SD of the per-seed deltas.
    pub paired_sd: Option<f64>,
    /// Standard error of the mean paired delta (`sd / √n`).
    pub paired_se: Option<f64>,
    /// 95% paired Student-*t* interval on `paired_mean_delta` (`df = n − 1`).
    pub ci95_low: Option<f64>,
    pub ci95_high: Option<f64>,
    /// Procedure used for the interval; `None` in legacy or non-estimable artifacts.
    pub ci95_method: Option<ConfidenceIntervalMethod>,
    /// Cohen's *d_z* = `paired_mean_delta / paired_sd`; `None` when the paired SD is 0 or n < 2.
    pub paired_dz: Option<f64>,
}

/// The result of a paired, same-seed control/treatment ensemble — the AE-S14 gate artifact.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairedEnsembleReport {
    pub experiment_id: String,
    pub control_manifest_fingerprint: u64,
    pub treatment_manifest_fingerprint: u64,
    pub control_law_fingerprint: u64,
    pub treatment_law_fingerprint: u64,
    pub registry_fingerprint: u64,
    /// The manifest paths that actually differ between control and treatment (⊆ the allowlist).
    pub declared_factors: Vec<String>,
    /// The seeds, in manifest-declared order; both sides run exactly these, in this order.
    pub seed_order: Vec<u64>,
    /// One entry per seed, preserving one-sided failures.
    pub pairs: Vec<SeedPair>,
    /// Paired effects over the deterministic union of final observables emitted by **both** sides.
    pub effects: Vec<PairedEffect>,
    pub control_only: Vec<String>,
    pub treatment_only: Vec<String>,
}

impl PairedEnsembleReport {
    pub fn effect_of(&self, observable: &str) -> Option<&PairedEffect> {
        self.effects.iter().find(|e| e.observable == observable)
    }
    pub fn complete_pairs(&self) -> usize {
        self.pairs.iter().filter(|p| p.is_complete()).count()
    }
    pub fn incomplete_pairs(&self) -> usize {
        self.pairs.len() - self.complete_pairs()
    }
}

/// Run a **paired, same-seed** control/treatment ensemble — the causal design AE-S14 requires.
///
/// Preflight (all **before** any model construction or RNG work): the registry is validated, the
/// treatment manifest is validated, its [`control_variant`](ExperimentManifest::control_variant) is
/// derived and validated, and the control/treatment difference is checked against `allowed`. The
/// control therefore shares the treatment's seed list **by construction** — the same seeds, in the
/// same order — which is what makes the deltas paired rather than two independent samples.
///
/// Effects are computed only from pairs where **both** sides completed; incomplete pairs are still
/// present in `pairs`. See [`PairedEffect`] for the exact defined-ness contract.
pub fn run_paired_ensemble<M: ExperimentModel>(
    treatment: &ExperimentManifest,
    registry: &ObservableRegistry,
    allowed: &FactorDiff,
) -> Result<PairedEnsembleReport, ExperimentError> {
    // `control_variant` strips the exotic regime (law + its forcings) and clones everything else,
    // including the seed list — so the same ordered seed set holds by construction.
    let control = treatment.control_variant();
    run_paired_ensemble_with_control::<M>(&control, treatment, registry, allowed)
}

/// Run a paired ensemble against an **explicitly supplied** control manifest.
///
/// Use this when the declared factor is something other than the exotic regime (e.g. an
/// intervention), since [`ExperimentManifest::control_variant`] only strips the exotic law and its
/// forcings. The two manifests must declare the **same seeds in the same order** — that is what makes
/// the deltas paired — and a mismatch is a structured error, never silently re-ordered or truncated.
/// Reject experiment pairs or ensembles that mix organism genesis phases (`QD-GENESIS`, ADR-0014).
pub fn validate_uniform_genesis_phase(
    manifests: &[&ExperimentManifest],
) -> Result<Option<crate::core::island::GenesisPhase>, ExperimentError> {
    let Some((&first, rest)) = manifests.split_first() else {
        return Ok(None);
    };
    let expected = crate::core::island::effective_genesis_phase(first);
    for m in rest {
        let actual = crate::core::island::effective_genesis_phase(m);
        if actual != expected {
            return Err(ExperimentError::MixedGenesisPhase {
                control: expected,
                treatment: actual,
            });
        }
    }
    Ok(expected)
}

pub fn run_paired_ensemble_with_control<M: ExperimentModel>(
    control: &ExperimentManifest,
    treatment: &ExperimentManifest,
    registry: &ObservableRegistry,
    allowed: &FactorDiff,
) -> Result<PairedEnsembleReport, ExperimentError> {
    registry.validate()?;
    treatment.validate(registry)?;
    control.validate(registry)?;
    validate_uniform_genesis_phase(&[control, treatment])?;
    let declared_factors = allowed.validate(control, treatment)?;

    // Pairing requires the identical ordered seed set (not merely the same set).
    if control.seeds != treatment.seeds {
        return Err(ExperimentError::UndeclaredFactorDifference {
            path: "seeds (paired runs require the same seeds in the same order)".into(),
        });
    }
    let seed_order = treatment.seeds.clone();

    let mut pairs = Vec::with_capacity(seed_order.len());
    for &seed in &seed_order {
        let c = run_manifest_seed::<M>(control, registry, seed, None, None);
        let t = run_manifest_seed::<M>(treatment, registry, seed, None, None);
        pairs.push(SeedPair {
            seed,
            control: c,
            treatment: t,
        });
    }

    summarize_seed_pairs(
        control,
        treatment,
        registry,
        declared_factors,
        seed_order,
        pairs,
    )
}

pub(crate) fn summarize_seed_pairs(
    control: &ExperimentManifest,
    treatment: &ExperimentManifest,
    registry: &ObservableRegistry,
    declared_factors: Vec<String>,
    seed_order: Vec<u64>,
    pairs: Vec<SeedPair>,
) -> Result<PairedEnsembleReport, ExperimentError> {
    // Deterministic union of final observables per side, over ALL runs (a failed run can still
    // report final observables). Using every run means an observable stays *listed* — with
    // `n_complete_pairs = 0` and all-`None` statistics — even when no pair completed, rather than
    // vanishing from the report. Only COMPLETE pairs contribute samples (below).
    let side_names = |pick: fn(&SeedPair) -> &RunResult| -> std::collections::BTreeSet<String> {
        let mut s = std::collections::BTreeSet::new();
        for p in &pairs {
            for (k, _) in &pick(p).final_observables {
                s.insert(k.clone());
            }
        }
        s
    };
    let c_names = side_names(|p| &p.control);
    let t_names = side_names(|p| &p.treatment);

    let mut effects = Vec::new();
    for name in c_names.intersection(&t_names) {
        let mut deltas = Vec::new();
        let mut cs = Vec::new();
        let mut ts = Vec::new();
        for p in &pairs {
            if !p.is_complete() {
                continue;
            }
            if let (Some(cv), Some(tv)) = (p.control.observable(name), p.treatment.observable(name))
            {
                cs.push(cv);
                ts.push(tv);
                deltas.push(require_finite_aggregate(name, "paired delta", tv - cv)?);
            }
        }
        effects.push(paired_effect(name, seed_order.len(), &cs, &ts, &deltas)?);
    }
    effects.sort_by(|a, b| a.observable.cmp(&b.observable));

    Ok(PairedEnsembleReport {
        experiment_id: treatment.experiment_id.clone(),
        control_manifest_fingerprint: control.fingerprint(),
        treatment_manifest_fingerprint: treatment.fingerprint(),
        control_law_fingerprint: control.laws.fingerprint(),
        treatment_law_fingerprint: treatment.laws.fingerprint(),
        registry_fingerprint: registry.fingerprint(),
        declared_factors,
        seed_order,
        pairs,
        effects,
        control_only: c_names.difference(&t_names).cloned().collect(),
        treatment_only: t_names.difference(&c_names).cloned().collect(),
    })
}
