//! # Experiment runner (AE1 M2) — deterministic runs, forks, provenance, ensembles, results.
//!
//! Where [`crate::core::experiment`] defines the *identity* of an experiment, this module *runs* it.
//! An [`ExperimentModel`] is a manifest-aware, deterministic simulation (the successor to
//! [`crate::core::scenario::SimModel`], which stays untouched so its S13/S14 tests remain green —
//! AE-111). The runner drives it under the multi-rate [`SimClock`] with a seed-derived
//! [`StdRng`], never `thread_rng()`, so the same manifest + seed + build path yields the same
//! [`RunResult::final_checksum`] (AE-S02).
//!
//! Three run shapes are provided:
//!
//! - [`run_manifest_seed`] — one deterministic run, wrapped in [`RunProvenance`] (AE-105) and a
//!   self-describing [`RunResult`] (AE-110).
//! - [`genesis_fork`] — a control/treatment pair from one manifest, its control being the
//!   [`ExperimentManifest::control_variant`], with a [`FactorDiff`] guard proving they differ only in
//!   declared factors (AE-106 / AE-S08).
//! - [`run_ensemble`] — every seed as an independent run, **preserving failed runs** in the summary
//!   with a deterministic seed order (AE-108). This satisfies AE-S14 only **partially**: the summary
//!   carries N, per-observable CI and failures, but there is deliberately **no control–treatment
//!   effect-size API** in this slice, which AE-S14 also requires.

pub use crate::core::build_identity::build_id;
use crate::core::causal::CausalLedger;
use crate::core::exotic_energy::{ExoticEnergyBudget, ExoticIntervention, UnitId};
use crate::core::experiment::{
    fnv1a_64, validate_intervention, ExperimentError, ExperimentManifest, FactorDiff,
    InitialConditionSet, ObservableRegistry, ObservableSpec, WorldLawSet, MAX_INTERVENTIONS,
};
use crate::core::intervention::{InterventionCommand, InterventionQueue};
use crate::core::scenario::{StateSample, TargetDelta};
use crate::core::sim_clock::{SimClock, ECOLOGY_PERIOD};
use rand::{rngs::StdRng, SeedableRng};
use serde::{Deserialize, Serialize};

/// The model-version string embedded in every run's provenance. Bump when the reference model's
/// dynamics change in a way that would alter checksums.
pub const MODEL_VERSION: &str = "reference-evolution-world/1";

fn run_identity_base(
    experiment_id: &str,
    manifest_fingerprint: u64,
    registry_fingerprint: u64,
    model_version: &str,
    build_id: &str,
    seed: u64,
) -> String {
    format!(
        "{experiment_id}#{manifest_fingerprint:016x}#r{registry_fingerprint:016x}\
         #m{:016x}#b{:016x}#s{seed}",
        fnv1a_64(model_version.as_bytes()),
        fnv1a_64(build_id.as_bytes())
    )
}

/// The reference field grid used by the headless slice (small — the reference ecosystem is a single
/// aggregate, the field is the spatial demonstration substrate).
pub const REFERENCE_FIELD_DIM: usize = 16;

/// A deterministic, manifest-configured simulation the runner can drive. Unlike the legacy
/// [`crate::core::scenario::SimModel`] (which is `Default`-constructed), this is built *from a
/// manifest* — the AE-104 factory seam — so world laws and initial conditions flow in explicitly
/// rather than from a global default.
pub trait ExperimentModel: Sized {
    /// An in-memory snapshot of the model's full state, sufficient to resume a run mid-flight. `Clone`
    /// so the checkpoint fork can hand the same snapshot to several branches. (This is *not* on-disk
    /// persistence — that is AE4, out of scope; a checkpoint fork happens within one process.)
    type Snapshot: Clone;

    /// Stable identity of the numerical model that produced a checksum.
    ///
    /// There is deliberately no default: an adapter that has not named its numerical semantics
    /// must not compile into a runner capable of emitting apparently comparable checksums.
    fn model_version() -> &'static str;

    /// Construct the model at genesis from the run's laws, initial conditions, declared runtime
    /// exotic forcings (AE-209), RNG seed and the reference field grid. Returns a structured error
    /// rather than panicking on an impossible config.
    ///
    /// `forcings` are *state* effects the model applies as their windows fire; they never mutate
    /// `laws`, which stay immutable for the whole run (ER01).
    fn from_manifest(
        laws: &WorldLawSet,
        initial: &InitialConditionSet,
        forcings: &[ExoticIntervention],
        seed: u64,
        grid: (usize, usize),
        run_ticks: u64,
    ) -> Result<Self, ExperimentError>;

    /// Capture the model's full state at the current tick (for a mid-run checkpoint fork).
    fn snapshot(&self) -> Self::Snapshot;

    /// Resume a model from a snapshot (AE-107). Implementations start a fresh causal chain for the
    /// post-fork segment; the field/EU state and any accumulated MU ledger are restored exactly.
    fn from_snapshot(snapshot: &Self::Snapshot) -> Result<Self, ExperimentError>;

    /// Replace this model's runtime exotic forcings — used by a checkpoint fork to give the
    /// **treatment** branch its effective forcing set while the control keeps the base set. This
    /// changes only *declared forcings*, never the world laws.
    ///
    /// The default implementation accepts an empty set (nothing to configure) and otherwise reports a
    /// structured error, so a model that does not support forcings fails loudly instead of silently
    /// ignoring a declared treatment.
    fn reconfigure_forcings(
        &mut self,
        forcings: &[ExoticIntervention],
        _run_ticks: u64,
    ) -> Result<(), ExperimentError> {
        if forcings.is_empty() {
            Ok(())
        } else {
            Err(ExperimentError::InvalidExoticIntervention {
                id: forcings[0].id,
                reason: "this model does not support runtime exotic forcings".into(),
            })
        }
    }

    /// Advance one base tick under the `active` interventions, recording caused changes into `ledger`.
    fn step(
        &mut self,
        clock: &SimClock,
        active: &[&InterventionCommand],
        ledger: &mut CausalLedger,
        rng: &mut StdRng,
    );

    /// A deterministic content fingerprint of the current state.
    fn checksum(&self) -> u32;

    /// Named scalar observables in a STABLE order.
    fn observables(&self) -> Vec<(String, f64)>;

    /// The current MU budget, if this model has an exotic-energy field (`None` on the baseline path).
    fn exotic_budget(&self) -> Option<ExoticEnergyBudget> {
        None
    }
}

// ---- Provenance & result (AE-105 / AE-110) ---------------------------------------------------

/// The identity of a single run: which experiment/manifest it belongs to, its parent (for forks),
/// the fork tick, and the input fingerprints + versions needed to replay and to compare (ER08/ER11).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunProvenance {
    pub experiment_id: String,
    pub run_id: String,
    pub parent_run_id: Option<String>,
    pub fork_tick: Option<u64>,
    pub seed: u64,
    pub manifest_fingerprint: u64,
    pub law_fingerprint: u64,
    pub registry_fingerprint: u64,
    pub model_version: String,
    pub build_id: String,
}

impl RunProvenance {
    fn derive<M: ExperimentModel>(
        manifest: &ExperimentManifest,
        registry: &ObservableRegistry,
        seed: u64,
        parent_run_id: Option<String>,
        fork_tick: Option<u64>,
    ) -> Self {
        let manifest_fingerprint = manifest.fingerprint();
        let registry_fingerprint = registry.fingerprint();
        let model_version = M::model_version().to_string();
        let build_id = build_id();
        RunProvenance {
            experiment_id: manifest.experiment_id.clone(),
            run_id: run_identity_base(
                &manifest.experiment_id,
                manifest_fingerprint,
                registry_fingerprint,
                &model_version,
                &build_id,
                seed,
            ),
            parent_run_id,
            fork_tick,
            seed,
            manifest_fingerprint,
            law_fingerprint: manifest.laws.fingerprint(),
            registry_fingerprint,
            model_version,
            build_id,
        }
    }
}

/// Whether a run finished or failed (a failed run is preserved, never dropped — ER11).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RunStatus {
    Completed,
    Failed {
        tick: u64,
        reason: String,
        checksum: u32,
    },
}

impl RunStatus {
    pub fn is_completed(&self) -> bool {
        matches!(self, RunStatus::Completed)
    }
}

/// The self-describing outcome of one run (AE-110): provenance, status, final checksum + observables,
/// the sampled series, the causal ledger, the MU budget (if any) and the observable metadata for
/// **every observable this run emitted** — everything needed to interpret it without external context.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunResult {
    pub provenance: RunProvenance,
    pub status: RunStatus,
    pub final_checksum: u32,
    #[serde(with = "crate::core::scenario::json_f64_pairs")]
    pub final_observables: Vec<(String, f64)>,
    pub series: Vec<StateSample>,
    pub ledger: CausalLedger,
    pub exotic_budget: Option<ExoticEnergyBudget>,
    /// Metadata for the deterministic union of every observable emitted by this run — every name that
    /// appears in any [`StateSample`] of `series` plus every `final_observables` name — so a
    /// transient observable that only appears mid-run is still described. Emitted names absent from
    /// the registry are surfaced in `warnings`, never silently dropped.
    pub observable_specs: Vec<ObservableSpec>,
    pub warnings: Vec<String>,
}

impl RunResult {
    /// The final value of a named observable, if present.
    pub fn observable(&self, name: &str) -> Option<f64> {
        self.final_observables
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| *v)
    }
}

/// A pre-run configuration failure, rendered as a preserved [`RunStatus::Failed`] result so no caller
/// ever silently executes an invalid manifest/registry (Task-5 hardening / ER11).
fn failed_run(provenance: RunProvenance, reason: String) -> RunResult {
    RunResult {
        provenance,
        status: RunStatus::Failed {
            tick: 0,
            reason: reason.clone(),
            checksum: 0,
        },
        final_checksum: 0,
        final_observables: Vec::new(),
        series: Vec::new(),
        ledger: CausalLedger::new(),
        exotic_budget: None,
        observable_specs: Vec::new(),
        warnings: vec![reason],
    }
}

/// A model panic leaves its state unsuitable for further inspection. Preserve every sample/effect
/// completed before the panic, but do not call `observables` or `checksum` on the unwound model.
fn failed_runtime_run(
    provenance: RunProvenance,
    registry: &ObservableRegistry,
    series: Vec<StateSample>,
    ledger: CausalLedger,
    tick: u64,
    reason: String,
) -> RunResult {
    let (observable_specs, mut warnings) = specs_for_emitted(registry, &series, &[]);
    warnings.push(reason.clone());
    RunResult {
        provenance,
        status: RunStatus::Failed {
            tick,
            reason,
            checksum: 0,
        },
        final_checksum: 0,
        final_observables: Vec::new(),
        series,
        ledger,
        exotic_budget: None,
        observable_specs,
        warnings,
    }
}

/// Metadata for the **deterministic union of every observable a run emitted** — every name appearing
/// in any sampled [`StateSample`] plus every `final_observables` name, in first-appearance order
/// (series in tick order, then final). This catches a *transient* observable that only appears mid-run
/// and never at the end. Any emitted name missing from the registry is reported as a warning rather
/// than silently dropped, so the result is honest about a backend/registry gap.
fn specs_for_emitted(
    registry: &ObservableRegistry,
    series: &[StateSample],
    final_observables: &[(String, f64)],
) -> (Vec<ObservableSpec>, Vec<String>) {
    let mut specs = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    // First-appearance order is deterministic: sample ticks are monotonic and each sample's observable
    // order is stable, then the final observables.
    let names = series
        .iter()
        .flat_map(|s| s.observables.iter().map(|(n, _)| n))
        .chain(final_observables.iter().map(|(n, _)| n));
    for name in names {
        if !seen.insert(name.clone()) {
            continue;
        }
        match registry.get(name) {
            Some(s) => specs.push(s.clone()),
            None => warnings.push(format!("emitted observable '{name}' has no registry spec")),
        }
    }
    (specs, warnings)
}

/// Drive `model` forward `run_ticks` base ticks under `queue`, sampling every `sample_period` ticks
/// and stopping early on a non-finite observable. Shared by [`run_manifest_seed`] and
/// [`checkpoint_fork`] so a full run and a forked continuation use identical stepping logic.
struct DriveFailure {
    tick: u64,
    reason: String,
    /// `false` after an unwind: callers must not inspect the model again.
    model_usable: bool,
}

fn bounded_panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    const MAX_CHARS: usize = 1_024;
    let raw = payload
        .downcast_ref::<&str>()
        .map(|value| (*value).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_owned());
    let mut chars = raw.chars();
    let mut bounded: String = chars.by_ref().take(MAX_CHARS).collect();
    if chars.next().is_some() {
        bounded.push('…');
    }
    bounded
}

fn duplicate_observable_name(observables: &[(String, f64)]) -> Option<&str> {
    observables
        .iter()
        .enumerate()
        .find_map(|(index, (name, _))| {
            observables[..index]
                .iter()
                .any(|(seen, _)| seen == name)
                .then_some(name.as_str())
        })
}

fn validate_exotic_budget(
    budget: &ExoticEnergyBudget,
    expected_unit: Option<&UnitId>,
) -> Result<(), String> {
    if let Some(expected) = expected_unit {
        if budget.unit != *expected {
            return Err(format!(
                "exotic budget unit '{}' does not match the manifest unit '{}'",
                budget.unit.as_str(),
                expected.as_str()
            ));
        }
    }
    for (name, value) in [
        ("initial", budget.initial),
        ("sourced", budget.sourced),
        ("field", budget.field),
        ("organism_storage", budget.organism_storage),
        ("dissipated", budget.dissipated),
        ("exported", budget.exported),
    ] {
        if !value.is_finite() {
            return Err(format!(
                "exotic budget field '{name}' is non-finite ({value})"
            ));
        }
        if value < 0.0 {
            return Err(format!(
                "exotic budget field '{name}' is negative ({value})"
            ));
        }
    }
    if !budget.balance_error().is_finite() {
        return Err("exotic budget balance error is non-finite".to_string());
    }
    Ok(())
}

fn drive<M: ExperimentModel>(
    model: &mut M,
    rng: &mut StdRng,
    clock: &mut SimClock,
    ledger: &mut CausalLedger,
    queue: &InterventionQueue,
    run_ticks: u64,
    sample_period: u64,
) -> (Vec<StateSample>, Option<DriveFailure>) {
    let mut series = Vec::new();
    for _ in 0..run_ticks {
        let tick = clock.advance();
        let tick_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
            model.step(clock, &active, ledger, rng);
            let observables = model.observables();
            if let Some((name, _)) = observables.iter().find(|(_, value)| !value.is_finite()) {
                return Some(format!("observable '{name}' became non-finite"));
            }
            if let Some(name) = duplicate_observable_name(&observables) {
                return Some(format!("observable '{name}' was emitted more than once"));
            }
            if sample_period != 0 && tick.is_multiple_of(sample_period) {
                series.push(StateSample { tick, observables });
            }
            None
        }));
        match tick_result {
            Ok(Some(reason)) => {
                return (
                    series,
                    Some(DriveFailure {
                        tick,
                        reason,
                        model_usable: true,
                    }),
                );
            }
            Ok(None) => {}
            Err(payload) => {
                return (
                    series,
                    Some(DriveFailure {
                        tick,
                        reason: format!(
                            "model panicked during tick {tick}: {}",
                            bounded_panic_message(payload.as_ref())
                        ),
                        model_usable: false,
                    }),
                );
            }
        }
    }
    (series, None)
}

/// Assemble a self-describing [`RunResult`] from a finished model + its accumulated series/ledger and
/// an optional runtime failure. Attaches spec metadata for every emitted observable and warns on MU
/// budget drift (a warning, not a failure — the tolerance policy is AE-006, deferred to the user).
fn assemble_result<M: ExperimentModel>(
    provenance: RunProvenance,
    model: &M,
    registry: &ObservableRegistry,
    expected_exotic_unit: Option<&UnitId>,
    series: Vec<StateSample>,
    ledger: CausalLedger,
    completion_tick: u64,
    mut failure: Option<(u64, String)>,
) -> RunResult {
    let final_observables = model.observables();
    let final_checksum = model.checksum();
    let mut exotic_budget = model.exotic_budget();
    let (observable_specs, mut warnings) = specs_for_emitted(registry, &series, &final_observables);

    if let Some(reason) = exotic_budget
        .as_ref()
        .and_then(|budget| validate_exotic_budget(budget, expected_exotic_unit).err())
    {
        exotic_budget = None;
        if failure.is_none() {
            failure = Some((completion_tick, reason));
        } else {
            warnings.push(reason);
        }
    }

    if let Some(b) = &exotic_budget {
        if b.balance_error().abs() / b.throughput() > 1e-3 {
            warnings.push(format!(
                "MU budget error {} exceeds 1e-3 relative throughput {}",
                b.balance_error(),
                b.throughput()
            ));
        }
    }

    let status = match failure {
        Some((tick, reason)) => RunStatus::Failed {
            tick,
            reason,
            checksum: final_checksum,
        },
        None => RunStatus::Completed,
    };

    RunResult {
        provenance,
        status,
        final_checksum,
        final_observables,
        series,
        ledger,
        exotic_budget,
        observable_specs,
        warnings,
    }
}

fn finish_drive<M: ExperimentModel>(
    provenance: RunProvenance,
    model: &M,
    registry: &ObservableRegistry,
    expected_exotic_unit: Option<&UnitId>,
    series: Vec<StateSample>,
    ledger: CausalLedger,
    completion_tick: u64,
    failure: Option<DriveFailure>,
) -> RunResult {
    match failure {
        Some(failure) if !failure.model_usable => failed_runtime_run(
            provenance,
            registry,
            series,
            ledger,
            failure.tick,
            failure.reason,
        ),
        failure => assemble_result(
            provenance,
            model,
            registry,
            expected_exotic_unit,
            series,
            ledger,
            completion_tick,
            failure.map(|failure| (failure.tick, failure.reason)),
        ),
    }
}

/// Run a single seed of a manifest to completion (or failure), returning a self-describing result.
/// Deterministic: identical manifest + seed + build → identical `final_checksum`.
///
/// The registry and manifest are validated first: an invalid registry or manifest (unknown
/// observable, duplicate/empty seeds, bad duration, invalid law, …) yields a **preserved
/// [`RunStatus::Failed`]** result with a structured reason — the runner never silently executes an
/// invalid configuration (Task-5 hardening).
pub fn run_manifest_seed<M: ExperimentModel>(
    manifest: &ExperimentManifest,
    registry: &ObservableRegistry,
    seed: u64,
    parent_run_id: Option<String>,
    fork_tick: Option<u64>,
) -> RunResult {
    let provenance = RunProvenance::derive::<M>(manifest, registry, seed, parent_run_id, fork_tick);
    let fallback_provenance = provenance.clone();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_manifest_seed_inner::<M>(manifest, registry, seed, provenance)
    })) {
        Ok(result) => result,
        Err(payload) => failed_run(
            fallback_provenance,
            format!(
                "model panicked outside the tick boundary: {}",
                bounded_panic_message(payload.as_ref())
            ),
        ),
    }
}

fn run_manifest_seed_inner<M: ExperimentModel>(
    manifest: &ExperimentManifest,
    registry: &ObservableRegistry,
    seed: u64,
    provenance: RunProvenance,
) -> RunResult {
    if let Err(e) = registry.validate() {
        return failed_run(provenance, format!("invalid registry: {e}"));
    }
    if let Err(e) = manifest.validate(registry) {
        return failed_run(provenance, format!("invalid manifest: {e}"));
    }
    // Reject a seed absent from the manifest's declared seed set BEFORE building any model or RNG:
    // running an undeclared seed would produce a result with no reproducible provenance in the
    // manifest. Preserved as a Failed result (never silently executed).
    if !manifest.seeds.contains(&seed) {
        return failed_run(
            provenance,
            format!("{}", ExperimentError::SeedNotInManifest { seed }),
        );
    }

    let grid = (REFERENCE_FIELD_DIM, REFERENCE_FIELD_DIM);
    let mut model = match M::from_manifest(
        &manifest.laws,
        &manifest.initial_conditions,
        &manifest.exotic_interventions,
        seed,
        grid,
        manifest.duration_ticks,
    ) {
        Ok(m) => m,
        Err(e) => return failed_run(provenance, format!("model construction failed: {e}")),
    };

    let mut rng = StdRng::seed_from_u64(seed);
    let mut clock = SimClock::new();
    let mut ledger = CausalLedger::new();
    let queue = InterventionQueue::new(manifest.interventions.clone());
    let (series, failure) = drive(
        &mut model,
        &mut rng,
        &mut clock,
        &mut ledger,
        &queue,
        manifest.duration_ticks,
        manifest.sample_period,
    );

    finish_drive(
        provenance,
        &model,
        registry,
        manifest.laws.exotic_energy.as_ref().map(|law| &law.unit),
        series,
        ledger,
        clock.tick,
        failure,
    )
}

// ---- Genesis fork (AE-106 / AE-S08) ----------------------------------------------------------

/// A control/treatment pair from one treatment manifest, with the declared factor difference and the
/// per-observable final delta.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ForkReport {
    pub control: RunResult,
    pub treatment: RunResult,
    /// The manifest paths that actually differ (⊆ the allowlist).
    pub declared_factors: Vec<String>,
    pub delta: Vec<TargetDelta>,
}

impl ForkReport {
    /// The final delta (treatment − control) for a named observable, if present in both.
    pub fn delta_of(&self, target: &str) -> Option<f64> {
        self.delta
            .iter()
            .find(|d| d.target == target)
            .map(|d| d.delta)
    }
}

/// Compare only observables measured on both sides.
///
/// A missing control observation is not a measured zero. Treatment-only channels remain available
/// in the treatment [`RunResult`], but cannot support a control/treatment delta.
fn matched_final_deltas(control: &RunResult, treatment: &RunResult) -> Vec<TargetDelta> {
    treatment
        .final_observables
        .iter()
        .filter_map(|(target, treatment_final)| {
            control.observable(target).map(|control_final| TargetDelta {
                target: target.clone(),
                control_final,
                treatment_final: *treatment_final,
                delta: treatment_final - control_final,
            })
        })
        .collect()
}

/// Run a genesis fork: the `treatment` manifest and its [`control_variant`](ExperimentManifest::control_variant)
/// under the same (first) seed, after checking their difference lies entirely within `allowed`
/// (AE-S08). Both runs are genesis runs (no parent); only the declared factor differs.
pub fn genesis_fork<M: ExperimentModel>(
    treatment: &ExperimentManifest,
    registry: &ObservableRegistry,
    allowed: &FactorDiff,
) -> Result<ForkReport, ExperimentError> {
    // Preflight: the registry itself must be valid BEFORE any manifest check, model construction or
    // RNG work — a malformed catalogue would otherwise silently shape both runs' result metadata.
    registry.validate()?;
    treatment.validate(registry)?;
    let control = treatment.control_variant();
    control.validate(registry)?;
    let declared_factors = allowed.validate(&control, treatment)?;

    let seed = treatment.seeds[0];
    let control_res = run_manifest_seed::<M>(&control, registry, seed, None, None);
    let treatment_res = run_manifest_seed::<M>(treatment, registry, seed, None, None);

    // Per-observable final delta (treatment − control), matched by name.
    let delta = matched_final_deltas(&control_res, &treatment_res);

    Ok(ForkReport {
        control: control_res,
        treatment: treatment_res,
        declared_factors,
        delta,
    })
}

// ---- Checkpoint fork (AE-107 / AE-S09) -------------------------------------------------------

/// A mid-run checkpoint fork: a shared prefix over ticks `1..=fork_tick`, then two branches continued
/// from the **same in-memory snapshot** (captured *after* `fork_tick` was processed) — `control` (base
/// interventions) and `treatment` (base + extra interventions), each processing ticks
/// `fork_tick+1 ..= duration_ticks`. Both branches carry `parent_run_id = prefix` and the same
/// `fork_tick`, and start from byte-identical post-`fork_tick` state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointForkReport {
    /// The shared history run over ticks `1..=fork_tick` (the checkpoint is its end state).
    pub prefix: RunResult,
    pub control: RunResult,
    pub treatment: RunResult,
    pub fork_tick: u64,
    /// The treatment-only interventions applied after the fork, **fully structured** (empty if none).
    /// This is the authoritative, lossless declaration of what differs between the branches — the
    /// human-readable [`declared_factors`](Self::declared_factors) strings are a convenience summary
    /// and must never be parsed.
    pub treatment_extra: Vec<InterventionCommand>,
    /// The treatment-only **exotic source forcings** (AE-209) applied after the fork, fully
    /// structured (empty if none). Like `treatment_extra`, this is the authoritative declaration —
    /// `declared_factors` is only a display summary.
    #[serde(default)]
    pub treatment_extra_exotic: Vec<ExoticIntervention>,
    /// A human-readable summary of `treatment_extra` / `treatment_extra_exotic`. Display only;
    /// never machine-parsed.
    pub declared_factors: Vec<String>,
    pub delta: Vec<TargetDelta>,
}

impl CheckpointForkReport {
    pub fn delta_of(&self, target: &str) -> Option<f64> {
        self.delta
            .iter()
            .find(|d| d.target == target)
            .map(|d| d.delta)
    }
}

/// Run a checkpoint fork (AE-107).
///
/// **Exact tick semantics.** The shared prefix processes ticks `1..=fork_tick` (the clock advances
/// exactly `fork_tick` times); the snapshot and the live RNG are captured **after** `fork_tick` has
/// been processed. Each branch then continues, processing ticks `fork_tick+1 ..= duration_ticks`. So
/// the first post-fork tick a branch can act on is `fork_tick + 1`, and a `treatment_extra`
/// intervention can only affect the treatment branch if its start lies in the half-open-below window
/// `(fork_tick, duration_ticks]` — i.e. `fork_tick < start_tick <= duration_ticks`. An intervention
/// starting at or before `fork_tick` (it belongs to the shared prefix, which used only the base
/// interventions) or after `duration_ticks` (the run has ended) is **rejected** rather than declared
/// as an unapplied factor.
///
/// Because the snapshot + a cloned RNG fully capture the state, a `control` branch continued with
/// identical inputs reproduces an uninterrupted run **bit-for-bit** (AE-S09), and both branches are
/// byte-identical up to the fork; only `treatment_extra` makes them diverge afterward. This never
/// re-simulates the prefix per branch. If the shared prefix fails before reaching `fork_tick`, a
/// structured [`ExperimentError::CheckpointPrefixFailed`] is returned (no branch is continued from a
/// partial state).
pub fn checkpoint_fork<M: ExperimentModel>(
    manifest: &ExperimentManifest,
    registry: &ObservableRegistry,
    seed: u64,
    fork_tick: u64,
    treatment_extra: &[InterventionCommand],
) -> Result<CheckpointForkReport, ExperimentError> {
    checkpoint_fork_with_exotic::<M>(manifest, registry, seed, fork_tick, treatment_extra, &[])
}

/// Run a checkpoint fork that may additionally apply **treatment-only exotic source forcings**
/// (AE-209) after the fork — the "add / remove / pulse the source at generation G" experiment.
///
/// This is a superset of [`checkpoint_fork`], which delegates here with no exotic extras, so existing
/// callers are unaffected.
///
/// On top of the legacy checks, every exotic extra must (all validated **before** the model, RNG or
/// snapshot are built): be structurally valid and grid-applicable, be uniquely identified within the
/// extras and against the manifest's base exotic forcings, act on a world that actually has an exotic
/// field, and have an **ecology firing tick strictly after `fork_tick` and within `duration_ticks`**
/// — otherwise it could never apply post-fork and declaring it would be misleading.
///
/// The treatment branch is reconfigured from the checkpoint with the *effective* forcing set
/// (base + extras) while the control keeps the base set; the prefix is never re-simulated.
pub fn checkpoint_fork_with_exotic<M: ExperimentModel>(
    manifest: &ExperimentManifest,
    registry: &ObservableRegistry,
    seed: u64,
    fork_tick: u64,
    treatment_extra: &[InterventionCommand],
    treatment_extra_exotic: &[ExoticIntervention],
) -> Result<CheckpointForkReport, ExperimentError> {
    registry.validate()?;
    manifest.validate(registry)?;
    if !manifest.seeds.contains(&seed) {
        return Err(ExperimentError::SeedNotInManifest { seed });
    }
    if fork_tick == 0 || fork_tick >= manifest.duration_ticks {
        return Err(ExperimentError::OutOfRange {
            field: "fork_tick".into(),
            value: fork_tick as f64,
            min: 1.0,
            max: (manifest.duration_ticks.saturating_sub(1)) as f64,
        });
    }
    // The combined base + extra intervention set must respect the manifest ceiling.
    let combined = manifest
        .interventions
        .len()
        .saturating_add(treatment_extra.len());
    if combined > MAX_INTERVENTIONS {
        return Err(ExperimentError::ResourceLimit {
            field: "interventions (base + treatment_extra)".into(),
            limit: MAX_INTERVENTIONS,
            found: combined,
        });
    }
    // Every treatment-only intervention must be individually valid (same manifest-path helper the
    // manifest itself uses), uniquely identified within the extras AND against the base set, and able
    // to affect a post-fork tick in `(fork_tick, duration_ticks]`; otherwise declaring it as the
    // fork's factor is misleading.
    for (i, cmd) in treatment_extra.iter().enumerate() {
        // Fork-window checks run FIRST so the checkpoint-specific `InapplicableIntervention` (which
        // explains *why* it cannot apply to this fork) wins over the generic manifest-path
        // "never active in the run" error, which would otherwise mask it for a late start_tick.
        if cmd.start_tick <= fork_tick {
            return Err(ExperimentError::InapplicableIntervention {
                id: cmd.id,
                reason: format!(
                    "start_tick {} is at or before fork_tick {fork_tick}; it belongs to the shared \
                     prefix and would never be applied in the branch",
                    cmd.start_tick
                ),
            });
        }
        if cmd.start_tick > manifest.duration_ticks {
            return Err(ExperimentError::InapplicableIntervention {
                id: cmd.id,
                reason: format!(
                    "start_tick {} is after duration_ticks {}; it would never be applied",
                    cmd.start_tick, manifest.duration_ticks
                ),
            });
        }
        validate_intervention(cmd, manifest.duration_ticks)?;
        if manifest.interventions.iter().any(|b| b.id == cmd.id) {
            return Err(ExperimentError::DuplicateId {
                context: "intervention (treatment_extra collides with a base intervention)".into(),
                id: cmd.id.to_string(),
            });
        }
        for other in &treatment_extra[i + 1..] {
            if other.id == cmd.id {
                return Err(ExperimentError::DuplicateId {
                    context: "intervention (duplicate within treatment_extra)".into(),
                    id: cmd.id.to_string(),
                });
            }
        }
    }

    // Exotic treatment extras (AE-209 checkpoint channel): validated fully BEFORE any model/RNG or
    // snapshot work.
    if !treatment_extra_exotic.is_empty() && manifest.laws.exotic_energy.is_none() {
        return Err(ExperimentError::InvalidExoticIntervention {
            id: treatment_extra_exotic[0].id,
            reason: "exotic treatment extras declared but laws.exotic_energy is None".into(),
        });
    }
    let combined_exotic = manifest
        .exotic_interventions
        .len()
        .saturating_add(treatment_extra_exotic.len());
    if combined_exotic > MAX_INTERVENTIONS {
        return Err(ExperimentError::ResourceLimit {
            field: "exotic_interventions (base + treatment_extra_exotic)".into(),
            limit: MAX_INTERVENTIONS,
            found: combined_exotic,
        });
    }
    for (i, cmd) in treatment_extra_exotic.iter().enumerate() {
        // Post-fork applicability: there must be an ecology firing STRICTLY after `fork_tick` and
        // within the run, inside the command's own window.
        let end_exclusive = cmd
            .start_tick
            .checked_add(cmd.effective_duration())
            .ok_or_else(|| ExperimentError::InvalidExoticIntervention {
                id: cmd.id,
                reason: "start_tick + duration overflows u64".into(),
            })?;
        let lo = cmd.start_tick.max(fork_tick + 1);
        let first_firing = lo.div_ceil(ECOLOGY_PERIOD).saturating_mul(ECOLOGY_PERIOD);
        if first_firing >= end_exclusive
            || first_firing > manifest.duration_ticks
            || first_firing <= fork_tick
        {
            return Err(ExperimentError::InapplicableIntervention {
                id: cmd.id,
                reason: format!(
                    "exotic forcing has no ecology firing (period {ECOLOGY_PERIOD}) strictly after \
                     fork_tick {fork_tick} inside its window [{}, {end_exclusive}) and within \
                     duration_ticks {}",
                    cmd.start_tick, manifest.duration_ticks
                ),
            });
        }
        cmd.validate(manifest.duration_ticks)
            .map_err(|reason| ExperimentError::InvalidExoticIntervention { id: cmd.id, reason })?;
        if manifest.exotic_interventions.iter().any(|b| b.id == cmd.id) {
            return Err(ExperimentError::DuplicateId {
                context: "exotic_intervention (treatment extra collides with a base forcing)"
                    .into(),
                id: cmd.id.to_string(),
            });
        }
        for other in &treatment_extra_exotic[i + 1..] {
            if other.id == cmd.id {
                return Err(ExperimentError::DuplicateId {
                    context: "exotic_intervention (duplicate within treatment_extra_exotic)".into(),
                    id: cmd.id.to_string(),
                });
            }
        }
    }

    // The **effective treatment input** is the base manifest with the extras appended. It is
    // validated in its own right (so the combined set must itself be a legal manifest) and it —
    // not the base manifest — is what the treatment branch's provenance fingerprints, so a
    // treatment run is independently addressable and replayable. With no extras it is byte-identical
    // to the base manifest, so both branches then share one fingerprint.
    let effective_treatment = {
        let mut m = manifest.clone();
        m.interventions.extend_from_slice(treatment_extra);
        m.exotic_interventions
            .extend_from_slice(treatment_extra_exotic);
        m
    };
    effective_treatment.validate(registry)?;
    // The reference grid is fixed and known before any model or RNG exists, so reject a spatially
    // inapplicable base/extra forcing here. This keeps checkpoint preflight model-independent:
    // even a custom `ExperimentModel` cannot silently accept an out-of-grid treatment factor.
    for cmd in &effective_treatment.exotic_interventions {
        cmd.validate_grid_applicability(REFERENCE_FIELD_DIM, REFERENCE_FIELD_DIM)
            .map_err(|reason| ExperimentError::InvalidExoticIntervention { id: cmd.id, reason })?;
    }

    let mfp = manifest.fingerprint();
    let treatment_fp = effective_treatment.fingerprint();
    let registry_fingerprint = registry.fingerprint();
    let model_version = M::model_version().to_string();
    let build_id = build_id();
    let base_id = run_identity_base(
        &manifest.experiment_id,
        mfp,
        registry_fingerprint,
        &model_version,
        &build_id,
        seed,
    );
    let treatment_base_id = run_identity_base(
        &manifest.experiment_id,
        treatment_fp,
        registry_fingerprint,
        &model_version,
        &build_id,
        seed,
    );
    let prefix_id = format!("{base_id}#prefix@{fork_tick}");
    // `fingerprint` selects which manifest's identity a branch carries: the prefix and control run
    // under the base manifest, the treatment under the effective treatment manifest.
    let make_prov = |run_id: String,
                     parent: Option<String>,
                     fork: Option<u64>,
                     fingerprint: u64|
     -> RunProvenance {
        RunProvenance {
            experiment_id: manifest.experiment_id.clone(),
            run_id,
            parent_run_id: parent,
            fork_tick: fork,
            seed,
            manifest_fingerprint: fingerprint,
            // The world laws are identical across branches (a checkpoint fork never changes a law —
            // that would be a genesis fork), so the law fingerprint is shared.
            law_fingerprint: manifest.laws.fingerprint(),
            registry_fingerprint,
            model_version: model_version.clone(),
            build_id: build_id.clone(),
        }
    };

    let grid = (REFERENCE_FIELD_DIM, REFERENCE_FIELD_DIM);
    let base_queue = InterventionQueue::new(manifest.interventions.clone());

    // Shared prefix: process ticks 1..=fork_tick (the clock advances exactly `fork_tick` times).
    let mut model = M::from_manifest(
        &manifest.laws,
        &manifest.initial_conditions,
        &manifest.exotic_interventions,
        seed,
        grid,
        manifest.duration_ticks,
    )?;
    let mut rng = StdRng::seed_from_u64(seed);
    let mut clock = SimClock::new();
    let mut prefix_ledger = CausalLedger::new();
    let (prefix_series, prefix_failure) = drive(
        &mut model,
        &mut rng,
        &mut clock,
        &mut prefix_ledger,
        &base_queue,
        fork_tick,
        manifest.sample_period,
    );

    // If the prefix diverged before reaching the checkpoint, the clock did NOT reach `fork_tick`, so
    // `remaining` would be wrong and the branches would resume from a partial/failed state. Fail
    // structurally instead of continuing.
    if let Some(failure) = prefix_failure {
        return Err(ExperimentError::CheckpointPrefixFailed {
            tick: failure.tick,
            reason: failure.reason,
        });
    }

    // Capture the checkpoint: model state + the live RNG (cloned, so no RNG serialization is needed
    // and both branches share the exact same stream from the fork).
    let snapshot = model.snapshot();
    let rng_at_fork = rng.clone();
    let clock_at_fork = clock; // SimClock is Copy
    let remaining = manifest.duration_ticks - fork_tick;

    // The prefix reached the checkpoint successfully (any failure returned above).
    let prefix = assemble_result(
        make_prov(prefix_id.clone(), None, None, mfp),
        &model,
        registry,
        manifest.laws.exotic_energy.as_ref().map(|law| &law.unit),
        prefix_series,
        prefix_ledger,
        clock_at_fork.tick,
        None,
    );

    // Continue a branch from the checkpoint under `interventions`, stamping it with the manifest
    // fingerprint that actually describes that branch's input.
    let continue_branch = |run_id: String,
                           interventions: Vec<InterventionCommand>,
                           forcings: &[ExoticIntervention],
                           fingerprint: u64| {
        let mut branch_model = M::from_snapshot(&snapshot)?;
        // Each branch gets its OWN declared forcing set: the control keeps the base forcings,
        // the treatment gets base + exotic extras. The snapshot itself is shared and untouched.
        branch_model.reconfigure_forcings(forcings, manifest.duration_ticks)?;
        let mut branch_rng = rng_at_fork.clone();
        let mut branch_clock = clock_at_fork;
        let mut branch_ledger = CausalLedger::new();
        let queue = InterventionQueue::new(interventions);
        let (series, failure) = drive(
            &mut branch_model,
            &mut branch_rng,
            &mut branch_clock,
            &mut branch_ledger,
            &queue,
            remaining,
            manifest.sample_period,
        );
        Ok::<RunResult, ExperimentError>(finish_drive(
            make_prov(
                run_id,
                Some(prefix_id.clone()),
                Some(fork_tick),
                fingerprint,
            ),
            &branch_model,
            registry,
            manifest.laws.exotic_energy.as_ref().map(|law| &law.unit),
            series,
            branch_ledger,
            branch_clock.tick,
            failure,
        ))
    };

    let control = continue_branch(
        format!("{base_id}#control@{fork_tick}"),
        manifest.interventions.clone(),
        &manifest.exotic_interventions,
        mfp,
    )?;
    let treatment = continue_branch(
        format!("{treatment_base_id}#treatment@{fork_tick}"),
        effective_treatment.interventions.clone(),
        &effective_treatment.exotic_interventions,
        treatment_fp,
    )?;

    let declared_factors: Vec<String> = treatment_extra
        .iter()
        .map(|c| format!("intervention:{:?}@{}", c.kind, c.start_tick))
        .chain(
            treatment_extra_exotic
                .iter()
                .map(|c| format!("exotic:{:?}@{}", c.kind, c.start_tick)),
        )
        .collect();

    let delta = matched_final_deltas(&control, &treatment);

    Ok(CheckpointForkReport {
        prefix,
        control,
        treatment,
        fork_tick,
        treatment_extra: treatment_extra.to_vec(),
        treatment_extra_exotic: treatment_extra_exotic.to_vec(),
        declared_factors,
        delta,
    })
}

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
pub fn run_paired_ensemble_with_control<M: ExperimentModel>(
    control: &ExperimentManifest,
    treatment: &ExperimentManifest,
    registry: &ObservableRegistry,
    allowed: &FactorDiff,
) -> Result<PairedEnsembleReport, ExperimentError> {
    registry.validate()?;
    treatment.validate(registry)?;
    control.validate(registry)?;
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

/// Two-sided 95% Student-*t* critical value, selected conservatively.
///
/// The exact NIST 0.975 quantiles are used for integer degrees of freedom 1–30, which covers the
/// experiment contract's normal 5–30-seed range. For fractional Welch degrees of freedom, flooring
/// the value selects a wider interval than rounding upward. Above 30, the next-lowest tabulated
/// anchor is retained; this stays deterministic and conservative without adding a statistical
/// distribution dependency to the simulation runtime.
fn conservative_student_t_critical_95(df: f64) -> f64 {
    const T_975_DF_1_TO_30: [f64; 30] = [
        12.706_204_736_432_095,
        4.302_652_729_696_142,
        3.182_446_305_284_264,
        2.776_445_105_197_798_7,
        2.570_581_835_636_314,
        2.446_911_848_791_681,
        2.364_624_251_010_299_3,
        2.306_004_135_204_166,
        2.262_157_162_854_099_3,
        2.228_138_851_964_938_5,
        2.200_985_160_082_949,
        2.178_812_829_663_417_7,
        2.160_368_656_461_012_7,
        2.144_786_687_916_927_3,
        2.131_449_545_559_323,
        2.119_905_299_221_011_2,
        2.109_815_577_833_181,
        2.100_922_040_240_96,
        2.093_024_054_408_263,
        2.085_963_447_265_836_4,
        2.079_613_844_727_662,
        2.073_873_067_904_015,
        2.068_657_610_419_041,
        2.063_898_561_628_021,
        2.059_538_552_753_294,
        2.055_529_438_642_871,
        2.051_830_516_480_289,
        2.048_407_141_795_244,
        2.045_229_642_132_703,
        2.042_272_456_301_238,
    ];

    if !df.is_finite() || df < 1.0 {
        return f64::NAN;
    }
    let lower_df = df.floor();
    if lower_df <= 30.0 {
        return T_975_DF_1_TO_30[lower_df as usize - 1];
    }
    if lower_df < 40.0 {
        return T_975_DF_1_TO_30[29];
    }
    if lower_df < 60.0 {
        return 2.021_075_390_306_273_3;
    }
    if lower_df < 80.0 {
        return 2.000_297_821_058_262;
    }
    if lower_df < 100.0 {
        return 1.990_063_421_071_4;
    }
    if lower_df < 120.0 {
        return 1.983_971_518_449_633_4;
    }
    1.979_930_405_052_777
}

/// Build a [`PairedEffect`], honouring the defined-ness contract documented on that type.
fn paired_effect(
    name: &str,
    n_requested: usize,
    control: &[f64],
    treatment: &[f64],
    deltas: &[f64],
) -> Result<PairedEffect, ExperimentError> {
    let n = deltas.len();
    let mut e = PairedEffect {
        observable: name.to_string(),
        n_requested,
        n_complete_pairs: n,
        control_mean: None,
        treatment_mean: None,
        paired_mean_delta: None,
        paired_sd: None,
        paired_se: None,
        ci95_low: None,
        ci95_high: None,
        ci95_method: None,
        paired_dz: None,
    };
    if n == 0 {
        return Ok(e);
    }
    let nf = n as f64;
    let cm = finite_sample_stats(name, control)?.mean;
    let tm = finite_sample_stats(name, treatment)?.mean;
    let delta_stats = finite_sample_stats(name, deltas)?;
    let dm = delta_stats.mean;
    e.control_mean = Some(cm);
    e.treatment_mean = Some(tm);
    e.paired_mean_delta = Some(dm);
    if n < 2 {
        // One pair: the delta is known, but no spread is estimable.
        return Ok(e);
    }
    let sd = delta_stats.sample_sd.expect("n >= 2 has a sample SD");
    let se = require_finite_aggregate(name, "paired standard error", sd / nf.sqrt())?;
    let critical = require_finite_aggregate(
        name,
        "paired 95% Student-t critical value",
        conservative_student_t_critical_95(nf - 1.0),
    )?;
    let half = require_finite_aggregate(name, "paired 95% confidence half-width", critical * se)?;
    e.paired_sd = Some(sd);
    e.paired_se = Some(se);
    e.ci95_low = Some(require_finite_aggregate(
        name,
        "paired 95% confidence lower bound",
        dm - half,
    )?);
    e.ci95_high = Some(require_finite_aggregate(
        name,
        "paired 95% confidence upper bound",
        dm + half,
    )?);
    e.ci95_method = Some(ConfidenceIntervalMethod::PairedStudentTConservativeTable);
    // Zero paired variance ⇒ d_z is undefined (division by zero), never ±inf.
    if sd > 0.0 {
        e.paired_dz = Some(require_finite_aggregate(
            name,
            "paired Cohen's dz",
            dm / sd,
        )?);
    }
    Ok(e)
}

// ---- Independent-sample effect size (descriptive helper; NOT the AE-S14 gate) ------------------

/// The comparative effect of a treatment ensemble against a control ensemble, for one observable.
///
/// Reports the raw **mean difference** (treatment − control, in the observable's own unit), a
/// **standardized** effect size (Hedges' *g* — Cohen's *d* with the small-sample correction), and a
/// 95% Welch–Student-*t* interval on the mean difference. Sample counts are the *completed* runs on
/// each side; failed runs are never silently folded in (they are listed on
/// [`EnsembleComparison`]).
///
/// This is a descriptive statistic over a seed ensemble, **not** a significance test and **not**
/// evidence of adaptation or selection — AE1–AE2.5 contain no pathway or reproduction, so no
/// evolutionary claim may be drawn from it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectSize {
    pub observable: String,
    pub n_control: usize,
    pub n_treatment: usize,
    pub control_mean: f64,
    pub treatment_mean: f64,
    /// `treatment_mean − control_mean`, in the observable's unit.
    pub mean_difference: f64,
    /// Pooled standard deviation; `None` unless both groups contain at least two samples.
    pub pooled_sd: Option<f64>,
    /// Hedges' *g*: the mean difference in pooled-SD units, small-sample corrected. `None` when
    /// either group has fewer than two samples or the pooled SD is zero.
    pub hedges_g: Option<f64>,
    /// 95% Welch–Student-*t* interval on the **mean difference**.
    pub ci95_low: Option<f64>,
    pub ci95_high: Option<f64>,
    /// Procedure used for the interval; `None` in legacy or non-estimable artifacts.
    pub ci95_method: Option<ConfidenceIntervalMethod>,
}

/// A control-vs-treatment ensemble comparison: per-observable effect sizes over the observables both
/// sides emitted, the preserved failures from each side, and the observables that exist on only one
/// side (reported explicitly rather than compared against a fabricated zero).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnsembleComparison {
    pub control_experiment_id: String,
    pub treatment_experiment_id: String,
    pub control_manifest_fingerprint: u64,
    pub treatment_manifest_fingerprint: u64,
    /// Completed-run counts (the N behind every effect size).
    pub control_n: usize,
    pub treatment_n: usize,
    /// `(seed, reason)` for every failed run — preserved, never dropped (ER11).
    pub control_failed: Vec<(u64, String)>,
    pub treatment_failed: Vec<(u64, String)>,
    pub effects: Vec<EffectSize>,
    /// Observables emitted only by the control side.
    pub control_only: Vec<String>,
    /// Observables emitted only by the treatment side.
    pub treatment_only: Vec<String>,
}

impl EnsembleComparison {
    /// The effect size for a named observable, if both sides emitted it.
    pub fn effect_of(&self, observable: &str) -> Option<&EffectSize> {
        self.effects.iter().find(|e| e.observable == observable)
    }
}

/// Compare two **already-run, independent** ensembles descriptively.
///
/// **This is NOT the AE-S14 gate.** It treats the two ensembles as independent samples (pooled
/// Hedges' *g*, independent-sample interval) and does **not** require — or verify — that the two
/// sides used the same seeds, the same seed order, or a validated control/treatment factor diff. Two
/// ensembles compared this way may differ in uncontrolled ways, so the result is descriptive only.
///
/// For the same-seed causal control/treatment design the gate requires, use
/// [`run_paired_ensemble`] and [`PairedEnsembleReport`], which derives and validates the control
/// variant, guarantees the identical ordered seed set by construction, and reports **paired** deltas.
///
/// Only **completed** runs contribute samples; failures from both sides are carried into the report.
/// An observable present on only one side is listed in `control_only` / `treatment_only` and is *not*
/// given a fabricated zero-valued counterpart.
pub fn compare_ensembles(
    control: &EnsembleSummary,
    treatment: &EnsembleSummary,
) -> Result<EnsembleComparison, ExperimentError> {
    let samples = |e: &EnsembleSummary, name: &str| -> Vec<f64> {
        e.runs
            .iter()
            .filter(|r| r.status.is_completed())
            .filter_map(|r| r.observable(name))
            .collect()
    };
    let names = |e: &EnsembleSummary| -> Vec<String> {
        let mut seen = std::collections::BTreeSet::new();
        for r in e.runs.iter().filter(|r| r.status.is_completed()) {
            for (k, _) in &r.final_observables {
                seen.insert(k.clone());
            }
        }
        seen.into_iter().collect()
    };

    let c_names = names(control);
    let t_names = names(treatment);
    let mut effects = Vec::new();
    for name in &c_names {
        if !t_names.contains(name) {
            continue;
        }
        let cs = samples(control, name);
        let ts = samples(treatment, name);
        if cs.is_empty() || ts.is_empty() {
            continue;
        }
        effects.push(effect_size(name, &cs, &ts)?);
    }
    let control_only: Vec<String> = c_names
        .iter()
        .filter(|n| !t_names.contains(*n))
        .cloned()
        .collect();
    let treatment_only: Vec<String> = t_names
        .iter()
        .filter(|n| !c_names.contains(*n))
        .cloned()
        .collect();

    Ok(EnsembleComparison {
        control_experiment_id: control.experiment_id.clone(),
        treatment_experiment_id: treatment.experiment_id.clone(),
        control_manifest_fingerprint: control.manifest_fingerprint,
        treatment_manifest_fingerprint: treatment.manifest_fingerprint,
        control_n: control.completed_runs,
        treatment_n: treatment.completed_runs,
        control_failed: control.failed.clone(),
        treatment_failed: treatment.failed.clone(),
        effects,
        control_only,
        treatment_only,
    })
}

fn effect_size(
    name: &str,
    control: &[f64],
    treatment: &[f64],
) -> Result<EffectSize, ExperimentError> {
    let control_stats = finite_sample_stats(name, control)?;
    let treatment_stats = finite_sample_stats(name, treatment)?;
    let cm = control_stats.mean;
    let tm = treatment_stats.mean;
    let control_sd = control_stats.sample_sd.unwrap_or(0.0);
    let treatment_sd = treatment_stats.sample_sd.unwrap_or(0.0);
    let nc = control.len() as f64;
    let nt = treatment.len() as f64;
    let diff = require_finite_aggregate(name, "independent mean difference", tm - cm)?;

    // Pooled SD (Cohen). Both groups need their own estimable sample variance; treating an absent
    // variance as zero would falsely make a singleton arm look perfectly measured.
    let can_estimate_spread = control.len() >= 2 && treatment.len() >= 2;
    let df = (nc + nt - 2.0).max(1.0);
    let pooled_sd = if can_estimate_spread {
        let sd_scale = control_sd.max(treatment_sd);
        let value = if sd_scale > 0.0 {
            let control_scaled = control_sd / sd_scale;
            let treatment_scaled = treatment_sd / sd_scale;
            let pooled_scaled = (((nc - 1.0) * control_scaled * control_scaled
                + (nt - 1.0) * treatment_scaled * treatment_scaled)
                / df)
                .max(0.0)
                .sqrt();
            require_finite_aggregate(name, "pooled standard deviation", pooled_scaled * sd_scale)?
        } else {
            0.0
        };
        Some(value)
    } else {
        None
    };

    // Hedges' g = Cohen's d × small-sample correction J ≈ 1 − 3/(4·df − 1).
    let hedges_g = match pooled_sd {
        Some(sd) if sd > 0.0 => {
            let d = diff / sd;
            let j = 1.0 - 3.0 / (4.0 * df - 1.0);
            Some(require_finite_aggregate(name, "Hedges' g", d * j)?)
        }
        _ => None,
    };

    // 95% Welch-Student-t interval on the mean difference (independent samples). Scaling both
    // standard deviations before calculating SE and Satterthwaite df avoids overflow for otherwise
    // finite extreme-valued experiments.
    let (ci95_low, ci95_high) = if can_estimate_spread {
        let sd_scale = control_sd.max(treatment_sd);
        let half = if sd_scale > 0.0 {
            let control_scaled = control_sd / sd_scale;
            let treatment_scaled = treatment_sd / sd_scale;
            let control_se_variance = control_scaled * control_scaled / nc;
            let treatment_se_variance = treatment_scaled * treatment_scaled / nt;
            let se_scaled = control_se_variance
                .max(0.0)
                .sqrt()
                .hypot(treatment_se_variance.max(0.0).sqrt());
            let se =
                require_finite_aggregate(name, "independent standard error", se_scaled * sd_scale)?;
            let numerator = (control_se_variance + treatment_se_variance).powi(2);
            let denominator = control_se_variance.powi(2) / (nc - 1.0)
                + treatment_se_variance.powi(2) / (nt - 1.0);
            let welch_df = require_finite_aggregate(
                name,
                "Welch-Satterthwaite degrees of freedom",
                numerator / denominator,
            )?;
            let critical = require_finite_aggregate(
                name,
                "independent 95% Student-t critical value",
                conservative_student_t_critical_95(welch_df),
            )?;
            require_finite_aggregate(name, "independent 95% confidence half-width", critical * se)?
        } else {
            0.0
        };
        (
            Some(require_finite_aggregate(
                name,
                "independent 95% confidence lower bound",
                diff - half,
            )?),
            Some(require_finite_aggregate(
                name,
                "independent 95% confidence upper bound",
                diff + half,
            )?),
        )
    } else {
        (None, None)
    };
    Ok(EffectSize {
        observable: name.to_string(),
        n_control: control.len(),
        n_treatment: treatment.len(),
        control_mean: cm,
        treatment_mean: tm,
        mean_difference: diff,
        pooled_sd,
        hedges_g,
        ci95_low,
        ci95_high,
        ci95_method: can_estimate_spread
            .then_some(ConfidenceIntervalMethod::WelchStudentTConservativeTable),
    })
}

#[derive(Clone, Copy, Debug)]
struct FiniteSampleStats {
    mean: f64,
    sample_sd: Option<f64>,
}

fn aggregate_not_finite(observable: &str, statistic: &str) -> ExperimentError {
    ExperimentError::AggregateNotFinite {
        observable: observable.to_owned(),
        statistic: statistic.to_owned(),
    }
}

fn require_finite_aggregate(
    observable: &str,
    statistic: &str,
    value: f64,
) -> Result<f64, ExperimentError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| aggregate_not_finite(observable, statistic))
}

/// Compute a finite mean and sample SD without overflowing intermediate sums.
///
/// Scaling by a power of two is exact for normal values and bounds every normalized sample below
/// 2.0 in magnitude. The online mean therefore cannot overflow, and the second pass uses compensated
/// accumulation for squared deviations. A genuinely unrepresentable final statistic is reported as
/// structured data instead of being allowed to reach JSON as `null`.
fn finite_sample_stats(
    observable: &str,
    samples: &[f64],
) -> Result<FiniteSampleStats, ExperimentError> {
    debug_assert!(!samples.is_empty());
    if samples.is_empty() {
        return Err(aggregate_not_finite(observable, "empty sample"));
    }
    if samples.iter().any(|value| !value.is_finite()) {
        return Err(aggregate_not_finite(observable, "input sample"));
    }

    let max_abs = samples
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    if max_abs == 0.0 {
        return Ok(FiniteSampleStats {
            mean: 0.0,
            sample_sd: (samples.len() > 1).then_some(0.0),
        });
    }

    const EXPONENT_MASK: u64 = 0x7ff0_0000_0000_0000;
    let exponent = max_abs.to_bits() & EXPONENT_MASK;
    let scale = if exponent == 0 {
        f64::MIN_POSITIVE
    } else {
        f64::from_bits(exponent)
    };

    let mut mean_scaled = 0.0;
    for (index, value) in samples.iter().enumerate() {
        let normalized = value / scale;
        mean_scaled += (normalized - mean_scaled) / (index + 1) as f64;
    }
    let mean = require_finite_aggregate(observable, "mean", mean_scaled * scale)?;

    if samples.len() == 1 {
        return Ok(FiniteSampleStats {
            mean,
            sample_sd: None,
        });
    }

    let mut squared_sum = 0.0;
    let mut correction = 0.0;
    for value in samples {
        let deviation = value / scale - mean_scaled;
        let term = deviation * deviation;
        let adjusted = term - correction;
        let updated = squared_sum + adjusted;
        correction = (updated - squared_sum) - adjusted;
        squared_sum = updated;
    }
    let variance_scaled = require_finite_aggregate(
        observable,
        "sample variance",
        (squared_sum / (samples.len() - 1) as f64).max(0.0),
    )?;
    let sample_sd = require_finite_aggregate(
        observable,
        "sample standard deviation",
        variance_scaled.sqrt() * scale,
    )?;

    Ok(FiniteSampleStats {
        mean,
        sample_sd: Some(sample_sd),
    })
}

/// Summarize the deterministic union of observables across completed runs.
///
/// Names retain first-appearance order across the manifest's seed order. A model may legitimately
/// emit a seed-dependent final observable, so using only the first completed run as the name source
/// would silently discard later evidence.
fn summarize_metrics(completed: &[&RunResult]) -> Result<Vec<MetricSummary>, ExperimentError> {
    let mut seen = std::collections::BTreeSet::new();
    let mut names = Vec::new();
    for run in completed {
        for (name, _) in &run.final_observables {
            if seen.insert(name.as_str()) {
                names.push(name.as_str());
            }
        }
    }

    let mut out = Vec::new();
    for name in names {
        let samples: Vec<f64> = completed
            .iter()
            .filter_map(|r| r.observable(name))
            .collect();
        let n = samples.len();
        if n == 0 {
            continue;
        }
        let stats = finite_sample_stats(name, &samples)?;
        let mean = stats.mean;
        let std = stats.sample_sd;
        let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let (ci95_low, ci95_high) = if let Some(std) = std {
            let critical = require_finite_aggregate(
                name,
                "95% Student-t critical value",
                conservative_student_t_critical_95((n - 1) as f64),
            )?;
            let half = require_finite_aggregate(
                name,
                "95% confidence half-width",
                std / (n as f64).sqrt() * critical,
            )?;
            (
                Some(require_finite_aggregate(
                    name,
                    "95% confidence lower bound",
                    mean - half,
                )?),
                Some(require_finite_aggregate(
                    name,
                    "95% confidence upper bound",
                    mean + half,
                )?),
            )
        } else {
            (None, None)
        };
        let ci95_method = std.map(|_| ConfidenceIntervalMethod::StudentTConservativeTable);
        out.push(MetricSummary {
            observable: name.to_owned(),
            n,
            mean,
            std,
            min,
            max,
            ci95_low,
            ci95_high,
            ci95_method,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::experiment::{InitialConditionSet, WorldLawSet, MANIFEST_SCHEMA_VERSION};
    use crate::core::reference_world::ReferenceEvolutionWorld;
    use crate::core::scenario::{run_scenario, ReferenceEcosystem, Scenario};
    use crate::core::world_artifact::WorldIdentity;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn ref_init() -> InitialConditionSet {
        // Exactly the ReferenceEcosystem defaults, so a baseline manifest run is byte-identical to a
        // legacy scenario run (AE-S01).
        InitialConditionSet::new(vec![
            ("precip".into(), 1.0),
            ("temperature".into(), 0.5),
            ("npp".into(), 1.0),
            ("plants".into(), 100.0),
            ("herbivores".into(), 40.0),
            ("predators".into(), 8.0),
            ("detritus".into(), 0.0),
        ])
    }

    fn baseline_manifest(seeds: Vec<u64>) -> ExperimentManifest {
        ExperimentManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            experiment_id: "ref-baseline".into(),
            name: "ref-baseline".into(),
            observer: crate::core::observer::ObserverPolicy::default(),
            world_identity: WorldIdentity::default(),
            laws: WorldLawSet::baseline(),
            initial_conditions: ref_init(),
            interventions: vec![],
            seeds,
            duration_ticks: 6000,
            sample_period: 600,
            observable_ids: vec!["plants".into(), "herbivores".into(), "predators".into()],
            exotic_interventions: Vec::new(),
        }
    }

    static PREFLIGHT_MODEL_CONSTRUCTIONS: AtomicUsize = AtomicUsize::new(0);

    /// A minimal model used only to prove invalid experiment inputs are rejected before the model
    /// factory (and therefore before the runner seeds an RNG).
    struct PreflightCountingModel;

    impl ExperimentModel for PreflightCountingModel {
        type Snapshot = ();

        fn model_version() -> &'static str {
            "test-preflight-counting/1"
        }

        fn from_manifest(
            _laws: &WorldLawSet,
            _initial: &InitialConditionSet,
            _forcings: &[ExoticIntervention],
            _seed: u64,
            _grid: (usize, usize),
            _run_ticks: u64,
        ) -> Result<Self, ExperimentError> {
            PREFLIGHT_MODEL_CONSTRUCTIONS.fetch_add(1, Ordering::SeqCst);
            Ok(Self)
        }

        fn snapshot(&self) -> Self::Snapshot {}

        fn from_snapshot(_snapshot: &Self::Snapshot) -> Result<Self, ExperimentError> {
            PREFLIGHT_MODEL_CONSTRUCTIONS.fetch_add(1, Ordering::SeqCst);
            Ok(Self)
        }

        fn step(
            &mut self,
            _clock: &SimClock,
            _active: &[&InterventionCommand],
            _ledger: &mut CausalLedger,
            _rng: &mut StdRng,
        ) {
        }

        fn checksum(&self) -> u32 {
            0
        }

        fn observables(&self) -> Vec<(String, f64)> {
            Vec::new()
        }
    }

    // ---- AE-S02: replay determinism ---------------------------------------------------------

    #[test]
    fn build_identity_is_more_specific_than_the_package_version() {
        let id = build_id();
        let version = env!("CARGO_PKG_VERSION");

        assert_ne!(
            id, version,
            "a package version alone aliases binaries built from different source revisions"
        );
        assert!(
            id.starts_with(&format!("{version}+")),
            "build identity must retain the human-readable package version: {id}"
        );
    }

    #[test]
    fn ae_s02_same_manifest_and_seed_give_same_checksum() {
        let reg = ObservableRegistry::reference_default();
        let m = baseline_manifest(vec![42]);
        let a = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 42, None, None);
        let b = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 42, None, None);
        assert_eq!(a.final_checksum, b.final_checksum);
        assert_eq!(a.final_observables, b.final_observables);
        assert_eq!(a.provenance.run_id, b.provenance.run_id);
        assert!(a.status.is_completed());
    }

    #[test]
    fn run_identity_distinguishes_numerical_models() {
        let registry = ObservableRegistry::reference_default();
        let manifest = baseline_manifest(vec![42]);
        let reference =
            RunProvenance::derive::<ReferenceEvolutionWorld>(&manifest, &registry, 42, None, None);
        let other =
            RunProvenance::derive::<DuplicateObservableModel>(&manifest, &registry, 42, None, None);

        assert_ne!(reference.model_version, other.model_version);
        assert_ne!(
            reference.run_id, other.run_id,
            "two numerical models must not publish the same address for different runs"
        );
    }

    #[test]
    fn run_identity_distinguishes_observable_registries() {
        let registry = ObservableRegistry::reference_default();
        let mut alternate_json =
            serde_json::to_value(&registry).expect("the default registry serializes");
        alternate_json["specs"][0]["display_name"] =
            serde_json::Value::String("alternate display contract".into());
        let alternate: ObservableRegistry =
            serde_json::from_value(alternate_json).expect("the alternate registry parses");
        alternate
            .validate()
            .expect("the alternate registry is valid");

        let manifest = baseline_manifest(vec![42]);
        let original =
            RunProvenance::derive::<ReferenceEvolutionWorld>(&manifest, &registry, 42, None, None);
        let changed =
            RunProvenance::derive::<ReferenceEvolutionWorld>(&manifest, &alternate, 42, None, None);

        assert_ne!(
            original.registry_fingerprint, changed.registry_fingerprint,
            "the control must actually change the registry contract"
        );
        assert_ne!(
            original.run_id, changed.run_id,
            "different measurement contracts must not share a run address"
        );
    }

    #[test]
    fn run_identity_distinguishes_builds() {
        let first = run_identity_base("experiment", 1, 2, "model/1", "build-a", 42);
        let second = run_identity_base("experiment", 1, 2, "model/1", "build-b", 42);

        assert_ne!(
            first, second,
            "different binaries must not publish the same run address"
        );
    }

    // ---- AE-S01: baseline parity with the legacy scenario -----------------------------------

    #[test]
    fn ae_s01_baseline_manifest_matches_legacy_scenario_checksum() {
        let reg = ObservableRegistry::reference_default();
        let seed = 7u64;
        let manifest = baseline_manifest(vec![seed]);
        let via_manifest =
            run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &reg, seed, None, None);

        // The equivalent legacy scenario over the untouched ReferenceEcosystem.
        let scn = Scenario {
            name: "legacy".into(),
            seed,
            duration_ticks: 6000,
            sample_period: 600,
            interventions: vec![],
        };
        let legacy = run_scenario::<ReferenceEcosystem>(&scn);

        // exotic_energy = None must reproduce the legacy world byte-for-byte (checksum), with no MU
        // budget and no hidden field.
        assert_eq!(
            via_manifest.final_checksum, legacy.final_checksum,
            "baseline manifest must match the legacy scenario checksum"
        );
        assert!(via_manifest.exotic_budget.is_none());
        // The EU observables agree too.
        for name in ["plants", "herbivores", "predators", "detritus"] {
            let mv = via_manifest.observable(name).unwrap();
            let lv = legacy
                .final_observables
                .iter()
                .find(|(k, _)| k == name)
                .unwrap()
                .1;
            assert!((mv - lv).abs() < 1e-12, "{name}: {mv} vs {lv}");
        }
    }

    // ---- AE-108 / AE-S14: ensemble preserves failures + order -------------------------------

    #[test]
    fn ae_s14_ensemble_preserves_seed_order_and_summarizes() {
        let reg = ObservableRegistry::reference_default();
        let seeds = vec![10u64, 11, 12, 13, 14];
        let m = baseline_manifest(seeds.clone());
        let summary = run_ensemble::<ReferenceEvolutionWorld>(&m, &reg).expect("valid ensemble");
        assert_eq!(summary.seed_order, seeds);
        assert_eq!(summary.requested_runs, 5);
        assert_eq!(summary.completed_runs, 5);
        assert!(summary.failed.is_empty());
        // Each seed's run kept its own identity, in order.
        for (run, seed) in summary.runs.iter().zip(&seeds) {
            assert_eq!(run.provenance.seed, *seed);
        }
        // A summary exists for the requested observables with N and a CI.
        let plants = summary
            .metrics
            .iter()
            .find(|m| m.observable == "plants")
            .expect("plants metric");
        assert_eq!(plants.n, 5);
        let low = plants.ci95_low.expect("n=5 estimates a lower bound");
        let high = plants.ci95_high.expect("n=5 estimates an upper bound");
        assert!(low <= plants.mean && plants.mean <= high);
    }

    #[test]
    fn ensemble_metrics_include_observables_first_emitted_by_later_seeds() {
        let registry = ObservableRegistry::reference_default();
        let manifest = baseline_manifest(vec![10, 11]);
        let mut first =
            run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, 10, None, None);
        let mut second =
            run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, 11, None, None);
        first.final_observables = vec![("shared".into(), 1.0), ("first-only".into(), 10.0)];
        second.final_observables = vec![("shared".into(), 3.0), ("later-only".into(), 20.0)];

        let metrics = summarize_metrics(&[&first, &second]).expect("finite metric summaries");
        let names: Vec<&str> = metrics
            .iter()
            .map(|metric| metric.observable.as_str())
            .collect();

        assert_eq!(names, ["shared", "first-only", "later-only"]);
        assert_eq!(metrics[0].n, 2);
        assert_eq!(metrics[0].mean, 2.0);
        assert_eq!(metrics[1].n, 1);
        assert_eq!(metrics[2].n, 1);
    }

    struct DuplicateObservableModel;

    impl ExperimentModel for DuplicateObservableModel {
        type Snapshot = ();

        fn model_version() -> &'static str {
            "test-duplicate-observable/1"
        }

        fn from_manifest(
            _laws: &WorldLawSet,
            _initial: &InitialConditionSet,
            _forcings: &[ExoticIntervention],
            _seed: u64,
            _grid: (usize, usize),
            _run_ticks: u64,
        ) -> Result<Self, ExperimentError> {
            Ok(Self)
        }

        fn snapshot(&self) -> Self::Snapshot {}

        fn from_snapshot(_snapshot: &Self::Snapshot) -> Result<Self, ExperimentError> {
            Ok(Self)
        }

        fn step(
            &mut self,
            _clock: &SimClock,
            _active: &[&InterventionCommand],
            _ledger: &mut CausalLedger,
            _rng: &mut StdRng,
        ) {
        }

        fn checksum(&self) -> u32 {
            1
        }

        fn observables(&self) -> Vec<(String, f64)> {
            vec![("ambiguous".into(), 1.0), ("ambiguous".into(), 2.0)]
        }
    }

    #[test]
    fn a_duplicate_observable_name_fails_the_run_instead_of_choosing_one_value() {
        let registry = ObservableRegistry::reference_default();
        let mut manifest = baseline_manifest(vec![10]);
        manifest.duration_ticks = 1;
        manifest.sample_period = 1;

        let result =
            run_manifest_seed::<DuplicateObservableModel>(&manifest, &registry, 10, None, None);

        assert!(matches!(
            result.status,
            RunStatus::Failed { tick: 1, ref reason, .. }
                if reason.contains("observable 'ambiguous' was emitted more than once")
        ));
    }

    struct InvalidBudgetModel {
        seed: u64,
    }

    impl ExperimentModel for InvalidBudgetModel {
        type Snapshot = u64;

        fn model_version() -> &'static str {
            "test-non-finite-budget/1"
        }

        fn from_manifest(
            _laws: &WorldLawSet,
            _initial: &InitialConditionSet,
            _forcings: &[ExoticIntervention],
            seed: u64,
            _grid: (usize, usize),
            _run_ticks: u64,
        ) -> Result<Self, ExperimentError> {
            Ok(Self { seed })
        }

        fn snapshot(&self) -> Self::Snapshot {
            self.seed
        }

        fn from_snapshot(snapshot: &Self::Snapshot) -> Result<Self, ExperimentError> {
            Ok(Self { seed: *snapshot })
        }

        fn step(
            &mut self,
            _clock: &SimClock,
            _active: &[&InterventionCommand],
            _ledger: &mut CausalLedger,
            _rng: &mut StdRng,
        ) {
        }

        fn checksum(&self) -> u32 {
            7
        }

        fn observables(&self) -> Vec<(String, f64)> {
            vec![
                ("plants".into(), 1.0),
                ("herbivores".into(), 1.0),
                ("predators".into(), 1.0),
            ]
        }

        fn exotic_budget(&self) -> Option<ExoticEnergyBudget> {
            let mut budget = ExoticEnergyBudget {
                unit: crate::core::exotic_energy::UnitId::new(crate::core::exotic_energy::MU_UNIT),
                initial: 1.0,
                sourced: 0.0,
                field: 1.0,
                organism_storage: 0.0,
                dissipated: 0.0,
                exported: 0.0,
            };
            match self.seed {
                10 => budget.sourced = f64::NAN,
                11 => budget.field = -1.0,
                12 => {
                    budget.unit = crate::core::exotic_energy::UnitId::new("undeclared-budget-unit")
                }
                13 => {
                    budget.initial = f64::MAX;
                    budget.sourced = f64::MAX;
                }
                _ => {}
            }
            Some(budget)
        }
    }

    #[test]
    fn a_non_finite_exotic_budget_fails_and_remains_json_round_trippable() {
        let registry = ObservableRegistry::reference_default();
        let mut manifest = baseline_manifest(vec![10]);
        manifest.duration_ticks = 1;
        manifest.sample_period = 1;
        manifest.laws.exotic_energy = Some(
            crate::core::exotic_energy::ExoticEnergyLaw::mana_uniform(1.0),
        );

        let result = run_manifest_seed::<InvalidBudgetModel>(&manifest, &registry, 10, None, None);

        assert!(matches!(
            result.status,
            RunStatus::Failed { tick: 1, ref reason, .. }
                if reason.contains("exotic budget")
                    && reason.contains("sourced")
                    && reason.contains("non-finite")
        ));
        assert!(
            result.exotic_budget.is_none(),
            "an invalid budget must not survive into a JSON artifact"
        );
        let encoded = serde_json::to_vec(&result).expect("failed result serializes");
        assert!(
            !encoded
                .windows(b"\"sourced\":null".len())
                .any(|window| window == b"\"sourced\":null"),
            "invalid numeric fields must not be erased into JSON null: {}",
            String::from_utf8_lossy(&encoded)
        );
        serde_json::from_slice::<RunResult>(&encoded).expect("public result schema round-trips");
    }

    #[test]
    fn a_negative_exotic_budget_compartment_fails_the_run() {
        let registry = ObservableRegistry::reference_default();
        let mut manifest = baseline_manifest(vec![11]);
        manifest.duration_ticks = 1;
        manifest.sample_period = 1;
        manifest.laws.exotic_energy = Some(
            crate::core::exotic_energy::ExoticEnergyLaw::mana_uniform(1.0),
        );

        let result = run_manifest_seed::<InvalidBudgetModel>(&manifest, &registry, 11, None, None);

        assert!(matches!(
            result.status,
            RunStatus::Failed { tick: 1, ref reason, .. }
                if reason.contains("exotic budget")
                    && reason.contains("field")
                    && reason.contains("negative")
        ));
        assert!(result.exotic_budget.is_none());
    }

    #[test]
    fn an_exotic_budget_with_an_undeclared_unit_fails_the_run() {
        let registry = ObservableRegistry::reference_default();
        let mut manifest = baseline_manifest(vec![12]);
        manifest.duration_ticks = 1;
        manifest.sample_period = 1;
        manifest.laws.exotic_energy = Some(
            crate::core::exotic_energy::ExoticEnergyLaw::mana_uniform(1.0),
        );

        let result = run_manifest_seed::<InvalidBudgetModel>(&manifest, &registry, 12, None, None);

        assert!(matches!(
            result.status,
            RunStatus::Failed { tick: 1, ref reason, .. }
                if reason.contains("exotic budget unit")
                    && reason.contains("undeclared-budget-unit")
                    && reason.contains(crate::core::exotic_energy::MU_UNIT)
        ));
        assert!(result.exotic_budget.is_none());
    }

    #[test]
    fn a_budget_whose_finite_compartments_overflow_its_balance_fails_the_run() {
        let registry = ObservableRegistry::reference_default();
        let mut manifest = baseline_manifest(vec![13]);
        manifest.duration_ticks = 1;
        manifest.sample_period = 1;
        manifest.laws.exotic_energy = Some(
            crate::core::exotic_energy::ExoticEnergyLaw::mana_uniform(1.0),
        );

        let result = run_manifest_seed::<InvalidBudgetModel>(&manifest, &registry, 13, None, None);

        assert!(matches!(
            result.status,
            RunStatus::Failed { tick: 1, ref reason, .. }
                if reason.contains("exotic budget balance error")
                    && reason.contains("non-finite")
        ));
        assert!(result.exotic_budget.is_none());
    }

    #[test]
    fn ensemble_metrics_keep_finite_extreme_samples_json_safe() {
        let registry = ObservableRegistry::reference_default();
        let manifest = baseline_manifest(vec![10, 11]);
        let mut first =
            run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, 10, None, None);
        let mut second =
            run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, 11, None, None);
        first.final_observables = vec![("extreme".into(), f64::MAX)];
        second.final_observables = vec![("extreme".into(), f64::MAX)];

        let metrics = summarize_metrics(&[&first, &second]).expect("finite extreme summary");
        let extreme = metrics.first().expect("extreme metric");

        assert_eq!(extreme.mean, f64::MAX);
        assert_eq!(extreme.std, Some(0.0));
        assert!(extreme.ci95_low.is_some_and(f64::is_finite));
        assert!(extreme.ci95_high.is_some_and(f64::is_finite));
        let encoded = serde_json::to_string(&metrics).expect("metric summary must serialize");
        assert!(
            !encoded.contains("null"),
            "finite samples must not become JSON null: {encoded}"
        );
    }

    #[test]
    fn paired_effect_keeps_finite_extreme_samples_json_safe() {
        let effect = paired_effect(
            "extreme",
            2,
            &[f64::MAX, f64::MAX],
            &[f64::MAX, f64::MAX],
            &[0.0, 0.0],
        )
        .expect("finite paired effect");

        assert_eq!(effect.control_mean, Some(f64::MAX));
        assert_eq!(effect.treatment_mean, Some(f64::MAX));
        assert_eq!(effect.paired_mean_delta, Some(0.0));
        assert_eq!(effect.paired_sd, Some(0.0));
        assert!(
            [
                effect.control_mean,
                effect.treatment_mean,
                effect.paired_mean_delta,
                effect.paired_sd,
                effect.paired_se,
                effect.ci95_low,
                effect.ci95_high,
                effect.paired_dz,
            ]
            .into_iter()
            .flatten()
            .all(f64::is_finite),
            "every defined paired statistic must be finite"
        );
        let encoded = serde_json::to_value(&effect).expect("paired effect must serialize");
        for field in [
            "control_mean",
            "treatment_mean",
            "paired_mean_delta",
            "paired_sd",
            "paired_se",
            "ci95_low",
            "ci95_high",
        ] {
            assert!(
                encoded[field].as_f64().is_some(),
                "defined field {field} must remain numeric JSON: {encoded}"
            );
        }
        assert!(
            encoded["paired_dz"].is_null(),
            "zero paired variance makes dz undefined"
        );
    }

    #[test]
    fn independent_effect_keeps_large_finite_variance_and_interval() {
        let effect = effect_size("large", &[1.0e200, 3.0e200], &[2.0e200, 4.0e200])
            .expect("finite independent effect");

        assert_eq!(effect.control_mean, 2.0e200);
        assert_eq!(effect.treatment_mean, 3.0e200);
        assert!(effect
            .pooled_sd
            .is_some_and(|value| value.is_finite() && value > 0.0));
        assert!(effect.hedges_g.is_some_and(f64::is_finite));
        assert!(effect.ci95_low.is_some_and(f64::is_finite));
        assert!(effect.ci95_high.is_some_and(f64::is_finite));
        let encoded = serde_json::to_string(&effect).expect("effect size must serialize");
        assert!(
            !encoded.contains("null"),
            "finite effect statistics must not become JSON null: {encoded}"
        );
    }

    #[test]
    fn aggregate_reports_unrepresentable_finite_result_as_structured_error() {
        let error = finite_sample_stats("opposite-extremes", &[-f64::MAX, f64::MAX])
            .expect_err("sample SD exceeds finite f64");

        assert_eq!(
            error,
            ExperimentError::AggregateNotFinite {
                observable: "opposite-extremes".into(),
                statistic: "sample standard deviation".into(),
            }
        );
        let encoded = serde_json::to_value(&error).expect("structured error must serialize");
        assert_eq!(
            encoded["AggregateNotFinite"]["observable"],
            "opposite-extremes"
        );
        assert_eq!(
            encoded["AggregateNotFinite"]["statistic"],
            "sample standard deviation"
        );
    }

    #[test]
    fn singleton_metric_does_not_fabricate_spread_or_confidence() {
        let registry = ObservableRegistry::reference_default();
        let manifest = baseline_manifest(vec![10]);
        let mut run =
            run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, 10, None, None);
        run.final_observables = vec![("pilot".into(), 42.0)];

        let metrics = summarize_metrics(&[&run]).expect("singleton descriptive summary");
        let encoded = serde_json::to_value(&metrics[0]).expect("metric must serialize");

        assert_eq!(encoded["mean"], 42.0);
        assert_eq!(encoded["min"], 42.0);
        assert_eq!(encoded["max"], 42.0);
        assert!(encoded["std"].is_null(), "one sample has no sample SD");
        assert!(
            encoded["ci95_low"].is_null() && encoded["ci95_high"].is_null(),
            "one sample cannot estimate a confidence interval: {encoded}"
        );
    }

    #[test]
    fn singleton_independent_groups_do_not_fabricate_inference() {
        let effect =
            effect_size("pilot", &[1.0], &[3.0]).expect("raw singleton difference is descriptive");
        let encoded = serde_json::to_value(&effect).expect("effect must serialize");

        assert_eq!(encoded["control_mean"], 1.0);
        assert_eq!(encoded["treatment_mean"], 3.0);
        assert_eq!(encoded["mean_difference"], 2.0);
        for field in ["pooled_sd", "hedges_g", "ci95_low", "ci95_high"] {
            assert!(
                encoded[field].is_null(),
                "{field} is not estimable from singleton groups: {encoded}"
            );
        }

        let asymmetric =
            effect_size("pilot", &[1.0], &[2.0, 4.0]).expect("raw asymmetric difference");
        let asymmetric_json =
            serde_json::to_value(&asymmetric).expect("asymmetric effect must serialize");
        assert_eq!(asymmetric_json["mean_difference"], 2.0);
        for field in ["pooled_sd", "hedges_g", "ci95_low", "ci95_high"] {
            assert!(
                asymmetric_json[field].is_null(),
                "{field} needs variance estimates from both groups: {asymmetric_json}"
            );
        }
    }

    #[test]
    fn zero_pooled_sd_does_not_claim_zero_standardized_effect() {
        let effect =
            effect_size("constant", &[1.0, 1.0], &[3.0, 3.0]).expect("finite raw difference");
        let encoded = serde_json::to_value(&effect).expect("effect must serialize");

        assert_eq!(encoded["pooled_sd"], 0.0);
        assert!(
            encoded["hedges_g"].is_null(),
            "a nonzero difference divided by zero spread is undefined, not zero"
        );
        assert_eq!(encoded["ci95_low"], 2.0);
        assert_eq!(encoded["ci95_high"], 2.0);
    }

    #[test]
    fn student_t_critical_values_follow_the_nist_95_percent_table() {
        assert!((conservative_student_t_critical_95(1.0) - 12.706_204_736_432_095).abs() < 1e-12);
        assert!((conservative_student_t_critical_95(2.0) - 4.302_652_729_696_142).abs() < 1e-12);
        assert!((conservative_student_t_critical_95(30.0) - 2.042_272_456_301_238).abs() < 1e-12);

        let mut previous = f64::INFINITY;
        for df in 1..=30 {
            let current = conservative_student_t_critical_95(df as f64);
            assert!(current < previous, "critical values must fall as df grows");
            previous = current;
        }

        assert_eq!(
            conservative_student_t_critical_95(1.99),
            conservative_student_t_critical_95(1.0),
            "fractional Welch df uses the conservative lower integer"
        );
        assert!(
            conservative_student_t_critical_95(1_000_000.0) > 1.96,
            "large-df fallback remains conservative relative to the normal limit"
        );
        assert!(conservative_student_t_critical_95(0.0).is_nan());
        assert!(conservative_student_t_critical_95(f64::NAN).is_nan());
    }

    #[test]
    fn small_sample_metric_ci_uses_student_t_not_normal_cutoff() {
        let registry = ObservableRegistry::reference_default();
        let manifest = baseline_manifest(vec![10, 11]);
        let mut first =
            run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, 10, None, None);
        let mut second =
            run_manifest_seed::<ReferenceEvolutionWorld>(&manifest, &registry, 11, None, None);
        first.final_observables = vec![("pilot".into(), 0.0)];
        second.final_observables = vec![("pilot".into(), 2.0)];

        let metrics = summarize_metrics(&[&first, &second]).expect("two-sample summary");
        let pilot = metrics.first().expect("pilot summary");
        // For [0, 2], mean=1, sample SD=sqrt(2), SE=1 and df=1. NIST t(0.975, 1).
        let critical = 12.706_204_736_432_095;
        assert!((pilot.ci95_low.expect("lower bound") - (1.0 - critical)).abs() < 1e-12);
        assert!((pilot.ci95_high.expect("upper bound") - (1.0 + critical)).abs() < 1e-12);
        let encoded = serde_json::to_value(pilot).expect("summary JSON");
        assert_eq!(encoded["ci95_method"], "student_t_conservative_table");
    }

    #[test]
    fn small_sample_paired_ci_uses_student_t_not_normal_cutoff() {
        let effect = paired_effect("pilot", 2, &[0.0, 0.0], &[0.0, 2.0], &[0.0, 2.0])
            .expect("paired effect");

        // The paired deltas [0, 2] have mean=1, SE=1 and df=1.
        let critical = 12.706_204_736_432_095;
        assert!((effect.ci95_low.expect("lower bound") - (1.0 - critical)).abs() < 1e-12);
        assert!((effect.ci95_high.expect("upper bound") - (1.0 + critical)).abs() < 1e-12);
        let encoded = serde_json::to_value(&effect).expect("paired JSON");
        assert_eq!(
            encoded["ci95_method"],
            "paired_student_t_conservative_table"
        );
    }

    #[test]
    fn independent_ci_uses_welch_degrees_of_freedom() {
        let effect = effect_size("pilot", &[0.0, 2.0], &[2.0, 6.0]).expect("independent effect");

        // SE² = 1 + 4 = 5 and Welch df = 25/17 ≈ 1.47. The deterministic table uses the
        // conservative lower integer df=1, rather than the pooled df=2 or a normal cutoff.
        let half = 12.706_204_736_432_095 * 5.0_f64.sqrt();
        assert!((effect.ci95_low.expect("lower bound") - (3.0 - half)).abs() < 1e-12);
        assert!((effect.ci95_high.expect("upper bound") - (3.0 + half)).abs() < 1e-12);
        let encoded = serde_json::to_value(&effect).expect("independent JSON");
        assert_eq!(encoded["ci95_method"], "welch_student_t_conservative_table");
    }

    #[test]
    fn optional_inference_fields_read_existing_numeric_artifacts() {
        let metric: MetricSummary = serde_json::from_value(serde_json::json!({
            "observable": "legacy",
            "n": 5,
            "mean": 10.0,
            "std": 2.0,
            "min": 7.0,
            "max": 13.0,
            "ci95_low": 8.0,
            "ci95_high": 12.0
        }))
        .expect("existing numeric metric artifact");
        assert_eq!(metric.std, Some(2.0));
        assert_eq!(metric.ci95_low, Some(8.0));
        assert_eq!(metric.ci95_high, Some(12.0));
        assert_eq!(metric.ci95_method, None);

        let effect: EffectSize = serde_json::from_value(serde_json::json!({
            "observable": "legacy",
            "n_control": 5,
            "n_treatment": 5,
            "control_mean": 10.0,
            "treatment_mean": 12.0,
            "mean_difference": 2.0,
            "pooled_sd": 1.5,
            "hedges_g": 1.2,
            "ci95_low": 0.5,
            "ci95_high": 3.5
        }))
        .expect("existing numeric effect artifact");
        assert_eq!(effect.pooled_sd, Some(1.5));
        assert_eq!(effect.hedges_g, Some(1.2));
        assert_eq!(effect.ci95_low, Some(0.5));
        assert_eq!(effect.ci95_high, Some(3.5));
        assert_eq!(effect.ci95_method, None);
    }

    // A test-double model that fails to construct for one poisoned seed, proving the ensemble keeps
    // failed runs (rather than dropping them) with a deterministic order — AE-108 failure-preservation.
    struct FlakyModel(ReferenceEvolutionWorld);
    impl ExperimentModel for FlakyModel {
        type Snapshot = <ReferenceEvolutionWorld as ExperimentModel>::Snapshot;

        fn model_version() -> &'static str {
            "test-flaky-reference/1"
        }

        fn from_manifest(
            laws: &WorldLawSet,
            initial: &InitialConditionSet,
            forcings: &[ExoticIntervention],
            seed: u64,
            grid: (usize, usize),
            run_ticks: u64,
        ) -> Result<Self, ExperimentError> {
            if seed == 12 {
                return Err(ExperimentError::EmptyField {
                    field: "poisoned-seed-12".into(),
                });
            }
            Ok(FlakyModel(ReferenceEvolutionWorld::from_manifest(
                laws, initial, forcings, seed, grid, run_ticks,
            )?))
        }
        fn snapshot(&self) -> Self::Snapshot {
            self.0.snapshot()
        }
        fn from_snapshot(snapshot: &Self::Snapshot) -> Result<Self, ExperimentError> {
            Ok(FlakyModel(ReferenceEvolutionWorld::from_snapshot(
                snapshot,
            )?))
        }
        fn step(
            &mut self,
            clock: &SimClock,
            active: &[&InterventionCommand],
            ledger: &mut CausalLedger,
            rng: &mut StdRng,
        ) {
            self.0.step(clock, active, ledger, rng)
        }
        fn checksum(&self) -> u32 {
            self.0.checksum()
        }
        fn observables(&self) -> Vec<(String, f64)> {
            self.0.observables()
        }
    }

    #[test]
    fn ae_s14_ensemble_keeps_failed_runs() {
        let reg = ObservableRegistry::reference_default();
        let seeds = vec![10u64, 11, 12, 13, 14];
        let m = baseline_manifest(seeds.clone());
        let summary = run_ensemble::<FlakyModel>(&m, &reg).expect("valid ensemble");
        assert_eq!(summary.requested_runs, 5);
        assert_eq!(summary.completed_runs, 4);
        assert_eq!(summary.failed.len(), 1);
        assert_eq!(summary.failed[0].0, 12);
        // The failed run is still present in `runs`, in order, not dropped.
        assert_eq!(summary.runs.len(), 5);
        assert!(matches!(summary.runs[2].status, RunStatus::Failed { .. }));
    }

    struct PanickingModel {
        inner: ReferenceEvolutionWorld,
        panic_during_step: bool,
    }

    impl ExperimentModel for PanickingModel {
        type Snapshot = <ReferenceEvolutionWorld as ExperimentModel>::Snapshot;

        fn model_version() -> &'static str {
            "test-panicking-reference/1"
        }

        fn from_manifest(
            laws: &WorldLawSet,
            initial: &InitialConditionSet,
            forcings: &[ExoticIntervention],
            seed: u64,
            grid: (usize, usize),
            run_ticks: u64,
        ) -> Result<Self, ExperimentError> {
            Ok(Self {
                inner: ReferenceEvolutionWorld::from_manifest(
                    laws, initial, forcings, seed, grid, run_ticks,
                )?,
                panic_during_step: seed == 12,
            })
        }

        fn snapshot(&self) -> Self::Snapshot {
            self.inner.snapshot()
        }

        fn from_snapshot(snapshot: &Self::Snapshot) -> Result<Self, ExperimentError> {
            Ok(Self {
                inner: ReferenceEvolutionWorld::from_snapshot(snapshot)?,
                panic_during_step: false,
            })
        }

        fn step(
            &mut self,
            clock: &SimClock,
            active: &[&InterventionCommand],
            ledger: &mut CausalLedger,
            rng: &mut StdRng,
        ) {
            assert!(
                !(self.panic_during_step && clock.tick == 120),
                "poisoned seed panicked deliberately"
            );
            self.inner.step(clock, active, ledger, rng);
        }

        fn checksum(&self) -> u32 {
            self.inner.checksum()
        }

        fn observables(&self) -> Vec<(String, f64)> {
            self.inner.observables()
        }
    }

    #[test]
    fn ae_s14_a_panicking_seed_is_preserved_without_destroying_the_ensemble() {
        let registry = ObservableRegistry::reference_default();
        let seeds = vec![10, 11, 12, 13];
        let mut manifest = baseline_manifest(seeds.clone());
        manifest.sample_period = 10;
        let summary =
            run_ensemble::<PanickingModel>(&manifest, &registry).expect("manifest is valid");

        assert_eq!(summary.seed_order, seeds);
        assert_eq!(summary.completed_runs, 3);
        assert_eq!(summary.failed.len(), 1);
        assert_eq!(summary.failed[0].0, 12);
        assert!(matches!(
            &summary.runs[2].status,
            RunStatus::Failed { tick: 120, reason, checksum: 0 } if reason.contains("panicked")
        ));
        assert_eq!(
            summary.runs[2].series.len(),
            11,
            "samples through tick 110 must survive the panic at tick 120"
        );
    }

    // ---- AUDIT: AE-209 checkpoint exotic channel ---------------------------------------------

    fn exotic_removal(id: u32, start: u64, dur: u64) -> ExoticIntervention {
        use crate::core::exotic_energy::ExoticInterventionKind;
        use crate::core::intervention::Region;
        ExoticIntervention {
            id,
            cause_id: 900 + id,
            kind: ExoticInterventionKind::RemoveSource,
            region: Region::Global,
            start_tick: start,
            duration_ticks: dur,
            amount: 0.5,
            curve: crate::core::intervention::Curve::Step,
        }
    }

    fn mana_manifest_for_fork(seeds: Vec<u64>) -> ExperimentManifest {
        use crate::core::exotic_energy::ExoticEnergyLaw;
        let mut m = baseline_manifest(seeds);
        m.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(150.0, 4));
        m
    }

    #[test]
    fn ck_exotic_channel_diverges_only_after_the_fork_and_keeps_identities() {
        use crate::core::exotic_energy::ExoticInterventionKind;
        use crate::core::intervention::Curve;

        let reg = ObservableRegistry::reference_default();
        let mut m = mana_manifest_for_fork(vec![7]);
        // Observe both the last ecology tick before the treatment first fires and the firing tick.
        m.sample_period = 60;
        let fork_tick = 3000u64;
        let extra = exotic_removal(1, 3120, 60);

        let rep = checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
            &m,
            &reg,
            7,
            fork_tick,
            &[],
            std::slice::from_ref(&extra),
        )
        .expect("fork ok");

        // The report carries the STRUCTURED exotic extras, not display strings.
        assert_eq!(rep.treatment_extra_exotic, vec![extra.clone()]);

        // Effective-treatment provenance: treatment fingerprints the base + exotic extras manifest;
        // prefix and control keep the base identity.
        let mut effective = m.clone();
        effective.exotic_interventions.push(extra);
        assert_eq!(
            rep.treatment.provenance.manifest_fingerprint,
            effective.fingerprint()
        );
        assert_eq!(rep.control.provenance.manifest_fingerprint, m.fingerprint());
        assert_eq!(rep.prefix.provenance.manifest_fingerprint, m.fingerprint());
        assert_ne!(
            rep.treatment.provenance.manifest_fingerprint,
            rep.control.provenance.manifest_fingerprint
        );

        // A runtime forcing NEVER changes the law (ER01).
        assert_eq!(
            rep.treatment.provenance.law_fingerprint,
            rep.control.provenance.law_fingerprint
        );

        // Control continuation is bit-for-bit the uninterrupted control run (no prefix re-sim).
        let uninterrupted = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 7, None, None);
        assert_eq!(rep.control.final_checksum, uninterrupted.final_checksum);
        // The treatment diverges because of the post-fork suppression.
        assert_ne!(rep.treatment.final_checksum, rep.control.final_checksum);
        assert!(
            rep.treatment.observable("exotic.density_total").unwrap()
                < rep.control.observable("exotic.density_total").unwrap(),
            "suppressing the source after the fork must leave less MU"
        );

        let sampled = |run: &RunResult, tick: u64, name: &str| {
            run.series
                .iter()
                .find(|sample| sample.tick == tick)
                .and_then(|sample| {
                    sample
                        .observables
                        .iter()
                        .find(|(observable, _)| observable == name)
                        .map(|(_, value)| *value)
                })
                .unwrap_or_else(|| panic!("missing {name} sample at tick {tick}"))
        };
        assert_eq!(
            sampled(&rep.control, 3060, "exotic.density_total"),
            sampled(&rep.treatment, 3060, "exotic.density_total"),
            "branches must remain identical through the last ecology firing before the extra forcing"
        );
        assert!(
            sampled(&rep.treatment, 3120, "exotic.density_total")
                < sampled(&rep.control, 3120, "exotic.density_total"),
            "RemoveSource must first diverge on its applicable post-fork ecology firing"
        );

        // MU still closes; EU is untouched by the exotic treatment.
        let b = rep.treatment.exotic_budget.clone().expect("budget");
        assert!(b.balance_error().abs() / b.throughput() < 1e-4);
        for name in ["plants", "herbivores", "predators", "detritus"] {
            assert_eq!(
                rep.control.observable(name).unwrap(),
                rep.treatment.observable(name).unwrap(),
                "{name} must be unaffected by an exotic forcing"
            );
        }

        // Attribution under the forcing's own cause id.
        assert!(rep
            .treatment
            .ledger
            .all()
            .iter()
            .any(|e| e.target == "exotic.source_suppressed" && e.cause_id == 901));
        // ...and no such record exists before/at the fork in the shared prefix.
        assert!(!rep
            .prefix
            .ledger
            .all()
            .iter()
            .any(|e| e.target == "exotic.source_suppressed"));

        // The other two checkpoint treatment kinds inject (rather than suppress) MU, keep the same
        // immutable law, preserve the EU state, and attribute movement to their own cause ids.
        for (id, kind, curve) in [
            (2, ExoticInterventionKind::AddSource, Curve::Step),
            (3, ExoticInterventionKind::Pulse, Curve::RampDown),
        ] {
            let mut injection = exotic_removal(id, 3120, 60);
            injection.kind = kind;
            injection.curve = curve;
            injection.amount = 0.25;
            let injected = checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
                &m,
                &reg,
                7,
                fork_tick,
                &[],
                &[injection],
            )
            .expect("injection fork");
            assert_eq!(
                sampled(&injected.control, 3060, "exotic.density_total"),
                sampled(&injected.treatment, 3060, "exotic.density_total")
            );
            assert!(
                sampled(&injected.treatment, 3120, "exotic.density_total")
                    > sampled(&injected.control, 3120, "exotic.density_total"),
                "{kind:?} must inject MU on its first applicable post-fork ecology firing"
            );
            assert_eq!(
                injected.treatment.provenance.law_fingerprint,
                injected.control.provenance.law_fingerprint
            );
            assert!(injected
                .treatment
                .ledger
                .all()
                .iter()
                .any(|entry| entry.target == "exotic.forcing" && entry.cause_id == 900 + id));
            for name in ["plants", "herbivores", "predators", "detritus"] {
                assert_eq!(
                    injected.control.observable(name),
                    injected.treatment.observable(name),
                    "{kind:?} must not alter the EU model"
                );
            }
        }
    }

    #[test]
    fn ck_exotic_channel_replays_deterministically_and_round_trips() {
        let reg = ObservableRegistry::reference_default();
        let m = mana_manifest_for_fork(vec![7]);
        let extra = exotic_removal(1, 3060, 600);
        let a = checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
            &m,
            &reg,
            7,
            3000,
            &[],
            std::slice::from_ref(&extra),
        )
        .expect("ok");
        let b = checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
            &m,
            &reg,
            7,
            3000,
            &[],
            &[extra],
        )
        .expect("ok");
        assert_eq!(a.treatment.final_checksum, b.treatment.final_checksum);
        assert_eq!(a.control.final_checksum, b.control.final_checksum);

        let json = serde_json::to_string(&a).expect("serializes");
        let back: CheckpointForkReport = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back.treatment_extra_exotic, a.treatment_extra_exotic);
        assert_eq!(
            back.treatment.provenance.manifest_fingerprint,
            a.treatment.provenance.manifest_fingerprint
        );
    }

    #[test]
    fn ck_exotic_channel_rejects_invalid_and_non_post_fork_extras() {
        use crate::core::intervention::Region;

        let reg = ObservableRegistry::reference_default();
        let m = mana_manifest_for_fork(vec![7]);
        let fork_tick = 3000u64;

        // Starts at/before the fork ⇒ belongs to the shared prefix.
        assert!(checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
            &m,
            &reg,
            7,
            fork_tick,
            &[],
            &[exotic_removal(1, 2940, 60)]
        )
        .is_err());

        // Has no ecology firing STRICTLY after the fork: window [3001, 3060) contains no multiple
        // of 60 greater than fork_tick.
        assert!(checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
            &m,
            &reg,
            7,
            fork_tick,
            &[],
            &[exotic_removal(2, 3001, 59)]
        )
        .is_err());

        // Structurally invalid (NaN amount).
        let mut nan = exotic_removal(3, 3060, 60);
        nan.amount = f32::NAN;
        assert!(checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
            &m,
            &reg,
            7,
            fork_tick,
            &[],
            &[nan]
        )
        .is_err());

        // Duplicate id within the exotic extras.
        assert!(checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
            &m,
            &reg,
            7,
            fork_tick,
            &[],
            &[exotic_removal(4, 3060, 60), exotic_removal(4, 3120, 60)]
        )
        .is_err());

        // Collision with a BASE exotic forcing id.
        let mut with_base = mana_manifest_for_fork(vec![7]);
        with_base.exotic_interventions = vec![exotic_removal(9, 600, 60)];
        assert!(checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
            &with_base,
            &reg,
            7,
            fork_tick,
            &[],
            &[exotic_removal(9, 3060, 60)]
        )
        .is_err());

        // A partially overflowing rectangle is not a valid 16×16 checkpoint treatment. This pure
        // grid preflight must reject it before a generic model factory (and therefore RNG) runs.
        let mut outside_grid = exotic_removal(6, 3060, 60);
        outside_grid.region = Region::Rect {
            min_x: 15,
            min_y: 15,
            max_x: 16,
            max_y: 16,
        };
        PREFLIGHT_MODEL_CONSTRUCTIONS.store(0, Ordering::SeqCst);
        assert!(checkpoint_fork_with_exotic::<PreflightCountingModel>(
            &m,
            &reg,
            7,
            fork_tick,
            &[],
            &[outside_grid]
        )
        .is_err());
        assert_eq!(
            PREFLIGHT_MODEL_CONSTRUCTIONS.load(Ordering::SeqCst),
            0,
            "invalid checkpoint geometry must be rejected before model construction"
        );

        // Exotic extras on a world with no exotic field.
        let baseline = baseline_manifest(vec![7]);
        assert!(checkpoint_fork_with_exotic::<ReferenceEvolutionWorld>(
            &baseline,
            &reg,
            7,
            fork_tick,
            &[],
            &[exotic_removal(5, 3060, 60)]
        )
        .is_err());
    }

    #[test]
    fn ck_legacy_checkpoint_fork_still_works_unchanged() {
        // The pre-existing entry point keeps its signature and behaviour (no exotic extras).
        let reg = ObservableRegistry::reference_default();
        let m = baseline_manifest(vec![99]);
        let uninterrupted = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 99, None, None);
        let fork =
            checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 99, 2500, &[]).expect("fork ok");
        assert_eq!(fork.control.final_checksum, uninterrupted.final_checksum);
        assert!(fork.treatment_extra_exotic.is_empty());
    }

    // ---- AUDIT D1: PAIRED same-seed control/treatment (the real AE-S14 gate) -----------------

    fn mana_treatment(seeds: Vec<u64>) -> ExperimentManifest {
        use crate::core::exotic_energy::ExoticEnergyLaw;
        let mut m = baseline_manifest(seeds);
        m.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(150.0, 4));
        m
    }

    #[test]
    fn genesis_fork_does_not_fabricate_zero_for_a_treatment_only_observable() {
        let reg = ObservableRegistry::reference_default();
        let treatment = mana_treatment(vec![7]);
        let report = genesis_fork::<ReferenceEvolutionWorld>(
            &treatment,
            &reg,
            &FactorDiff::genesis_exotic(),
        )
        .expect("fork runs");

        assert!(report.control.observable("exotic.density_total").is_none());
        assert!(report
            .treatment
            .observable("exotic.density_total")
            .is_some());
        assert!(
            report.delta_of("exotic.density_total").is_none(),
            "a missing control observation is not a measured zero"
        );
    }

    #[test]
    fn d1_paired_runner_uses_same_ordered_seeds_and_reports_paired_stats() {
        let reg = ObservableRegistry::reference_default();
        let seeds = vec![1u64, 2, 3, 4, 5];
        let treatment = mana_treatment(seeds.clone());
        let rep = run_paired_ensemble::<ReferenceEvolutionWorld>(
            &treatment,
            &reg,
            &FactorDiff::genesis_exotic(),
        )
        .expect("paired run");

        // Same ordered seed set by construction, one pair per seed.
        assert_eq!(rep.seed_order, seeds);
        assert_eq!(rep.pairs.len(), seeds.len());
        for (p, s) in rep.pairs.iter().zip(&seeds) {
            assert_eq!(p.seed, *s);
            assert_eq!(p.control.provenance.seed, *s);
            assert_eq!(p.treatment.provenance.seed, *s);
            assert!(p.is_complete());
        }

        // Provenance/fingerprints of BOTH sides are carried.
        assert_eq!(rep.treatment_manifest_fingerprint, treatment.fingerprint());
        assert_eq!(
            rep.control_manifest_fingerprint,
            treatment.control_variant().fingerprint()
        );
        assert_ne!(
            rep.control_manifest_fingerprint,
            rep.treatment_manifest_fingerprint
        );
        assert_ne!(rep.control_law_fingerprint, rep.treatment_law_fingerprint);
        assert_eq!(rep.registry_fingerprint, reg.fingerprint());

        // EU observables: the exotic field never touches biomass, so every paired delta is exactly 0
        // and the paired SD is 0 — but the statistics are DEFINED (n = 5).
        let plants = rep.effect_of("plants").expect("plants effect");
        assert_eq!(plants.n_requested, 5);
        assert_eq!(plants.n_complete_pairs, 5);
        assert_eq!(plants.paired_mean_delta, Some(0.0));
        assert_eq!(plants.paired_sd, Some(0.0));
        assert_eq!(plants.paired_se, Some(0.0));
        assert_eq!(plants.ci95_low, Some(0.0));
        assert_eq!(plants.ci95_high, Some(0.0));
        // Zero paired variance ⇒ standardized dz is UNDEFINED, not infinite.
        assert_eq!(plants.paired_dz, None);
    }

    #[test]
    fn d1_paired_effects_have_known_sign_and_magnitude() {
        // A drought treatment, same seeds: every pair must move plants down by the same mechanism,
        // so the paired mean delta is negative and the paired CI excludes zero.
        let reg = ObservableRegistry::reference_default();
        let seeds = vec![21u64, 22, 23, 24, 25, 26];
        let control = baseline_manifest(seeds.clone());
        let mut treatment = baseline_manifest(seeds.clone());
        treatment.interventions = vec![drought(1, 600)];

        // `control_variant` only strips the EXOTIC regime, so a non-exotic factor needs an explicit
        // control manifest — that is what `run_paired_ensemble_with_control` is for.
        let allowed = FactorDiff {
            allowed_paths: vec!["interventions".to_string()],
        };
        let rep = run_paired_ensemble_with_control::<ReferenceEvolutionWorld>(
            &control, &treatment, &reg, &allowed,
        )
        .expect("paired run");

        let plants = rep.effect_of("plants").expect("plants effect");
        assert_eq!(plants.n_complete_pairs, 6);
        let delta = plants.paired_mean_delta.expect("defined");
        assert!(delta < 0.0, "drought must lower plants, got {delta}");
        assert!(plants.ci95_high.expect("defined") < 0.0, "CI excludes 0");
        let dz = plants
            .paired_dz
            .expect("paired dz must be defined for non-zero variance");
        assert!(
            dz.is_finite() && dz < 0.0,
            "drought dz must be finite and negative"
        );
        // Control/treatment means bracket the delta consistently.
        assert!(
            (plants.treatment_mean.unwrap() - plants.control_mean.unwrap() - delta).abs() < 1e-9
        );
    }

    #[test]
    fn d1_paired_single_pair_defines_means_but_not_spread() {
        let reg = ObservableRegistry::reference_default();
        let treatment = mana_treatment(vec![7]);
        let rep = run_paired_ensemble::<ReferenceEvolutionWorld>(
            &treatment,
            &reg,
            &FactorDiff::genesis_exotic(),
        )
        .expect("paired run");
        let plants = rep.effect_of("plants").expect("plants effect");
        assert_eq!(plants.n_complete_pairs, 1);
        assert!(plants.control_mean.is_some());
        assert!(plants.treatment_mean.is_some());
        assert!(plants.paired_mean_delta.is_some());
        // n = 1 ⇒ no spread is estimable.
        assert_eq!(plants.paired_sd, None);
        assert_eq!(plants.paired_se, None);
        assert_eq!(plants.ci95_low, None);
        assert_eq!(plants.ci95_high, None);
        assert_eq!(plants.paired_dz, None);
    }

    // A model that fails at runtime for ONE seed, to prove one-sided failures never silently drop a
    // half-pair and never contaminate the effect statistics.
    struct OneSidedFailModel {
        inner: ReferenceEvolutionWorld,
        fail: bool,
        ticks: u64,
    }
    impl ExperimentModel for OneSidedFailModel {
        type Snapshot = (<ReferenceEvolutionWorld as ExperimentModel>::Snapshot, u64);

        fn model_version() -> &'static str {
            "test-one-sided-failure/1"
        }

        fn from_manifest(
            laws: &WorldLawSet,
            initial: &InitialConditionSet,
            forcings: &[ExoticIntervention],
            seed: u64,
            grid: (usize, usize),
            run_ticks: u64,
        ) -> Result<Self, ExperimentError> {
            Ok(OneSidedFailModel {
                inner: ReferenceEvolutionWorld::from_manifest(
                    laws, initial, forcings, seed, grid, run_ticks,
                )?,
                // Only the TREATMENT side (which has the exotic law) fails, and only for seed 3.
                fail: seed == 3 && laws.exotic_energy.is_some(),
                ticks: 0,
            })
        }
        fn snapshot(&self) -> Self::Snapshot {
            (self.inner.snapshot(), self.ticks)
        }
        fn from_snapshot(s: &Self::Snapshot) -> Result<Self, ExperimentError> {
            Ok(OneSidedFailModel {
                inner: ReferenceEvolutionWorld::from_snapshot(&s.0)?,
                fail: false,
                ticks: s.1,
            })
        }
        fn step(
            &mut self,
            clock: &SimClock,
            active: &[&InterventionCommand],
            ledger: &mut CausalLedger,
            rng: &mut StdRng,
        ) {
            self.inner.step(clock, active, ledger, rng);
            self.ticks += 1;
        }
        fn checksum(&self) -> u32 {
            self.inner.checksum()
        }
        fn observables(&self) -> Vec<(String, f64)> {
            let mut o = self.inner.observables();
            if self.fail && self.ticks >= 120 {
                o.push(("diverged".into(), f64::NAN));
            }
            o
        }
    }

    #[test]
    fn d1_one_sided_failure_is_preserved_and_excluded_from_effects() {
        let reg = ObservableRegistry::reference_default();
        let seeds = vec![1u64, 2, 3, 4, 5];
        let treatment = mana_treatment(seeds.clone());
        let rep = run_paired_ensemble::<OneSidedFailModel>(
            &treatment,
            &reg,
            &FactorDiff::genesis_exotic(),
        )
        .expect("paired run");

        // The half-pair is PRESERVED, not dropped: 5 pairs recorded, 1 incomplete.
        assert_eq!(rep.pairs.len(), 5);
        let broken = rep.pairs.iter().find(|p| p.seed == 3).expect("seed 3 pair");
        assert!(!broken.is_complete());
        assert!(broken.control.status.is_completed());
        assert!(!broken.treatment.status.is_completed());
        assert_eq!(rep.incomplete_pairs(), 1);

        // Effects use only the 4 complete pairs, and n_requested still reports all 5.
        let plants = rep.effect_of("plants").expect("plants effect");
        assert_eq!(plants.n_requested, 5);
        assert_eq!(plants.n_complete_pairs, 4);
    }

    #[test]
    fn d1_zero_complete_pairs_yields_all_none_statistics() {
        // Force EVERY treatment run to fail, so no pair is complete.
        struct AlwaysFail(ReferenceEvolutionWorld, u64, bool);
        impl ExperimentModel for AlwaysFail {
            type Snapshot = <ReferenceEvolutionWorld as ExperimentModel>::Snapshot;

            fn model_version() -> &'static str {
                "test-always-fail/1"
            }

            fn from_manifest(
                laws: &WorldLawSet,
                initial: &InitialConditionSet,
                forcings: &[ExoticIntervention],
                seed: u64,
                grid: (usize, usize),
                run_ticks: u64,
            ) -> Result<Self, ExperimentError> {
                Ok(AlwaysFail(
                    ReferenceEvolutionWorld::from_manifest(
                        laws, initial, forcings, seed, grid, run_ticks,
                    )?,
                    0,
                    laws.exotic_energy.is_some(),
                ))
            }
            fn snapshot(&self) -> Self::Snapshot {
                self.0.snapshot()
            }
            fn from_snapshot(s: &Self::Snapshot) -> Result<Self, ExperimentError> {
                Ok(AlwaysFail(
                    ReferenceEvolutionWorld::from_snapshot(s)?,
                    0,
                    false,
                ))
            }
            fn step(
                &mut self,
                c: &SimClock,
                a: &[&InterventionCommand],
                l: &mut CausalLedger,
                r: &mut StdRng,
            ) {
                self.0.step(c, a, l, r);
                self.1 += 1;
            }
            fn checksum(&self) -> u32 {
                self.0.checksum()
            }
            fn observables(&self) -> Vec<(String, f64)> {
                let mut o = self.0.observables();
                if self.2 && self.1 >= 120 {
                    o.push(("diverged".into(), f64::NAN));
                }
                o
            }
        }

        let reg = ObservableRegistry::reference_default();
        let treatment = mana_treatment(vec![1, 2, 3]);
        let rep =
            run_paired_ensemble::<AlwaysFail>(&treatment, &reg, &FactorDiff::genesis_exotic())
                .expect("paired run");
        assert_eq!(rep.incomplete_pairs(), 3);
        let plants = rep.effect_of("plants").expect("plants is still listed");
        assert_eq!(plants.n_requested, 3);
        assert_eq!(plants.n_complete_pairs, 0);
        // n = 0 ⇒ EVERY statistic is None; nothing is fabricated.
        assert_eq!(plants.control_mean, None);
        assert_eq!(plants.treatment_mean, None);
        assert_eq!(plants.paired_mean_delta, None);
        assert_eq!(plants.paired_sd, None);
        assert_eq!(plants.paired_se, None);
        assert_eq!(plants.ci95_low, None);
        assert_eq!(plants.ci95_high, None);
        assert_eq!(plants.paired_dz, None);
    }

    #[test]
    fn d1_observable_missing_on_one_side_is_listed_not_compared() {
        let reg = ObservableRegistry::reference_default();
        let treatment = mana_treatment(vec![1, 2, 3]);
        let rep = run_paired_ensemble::<ReferenceEvolutionWorld>(
            &treatment,
            &reg,
            &FactorDiff::genesis_exotic(),
        )
        .expect("paired run");
        // The MU observables exist only on the treatment side.
        assert!(rep
            .treatment_only
            .iter()
            .any(|o| o == "exotic.density_total"));
        assert!(rep.effect_of("exotic.density_total").is_none());
        assert!(rep.control_only.is_empty());
    }

    #[test]
    fn d1_paired_report_json_round_trips_without_nan() {
        let reg = ObservableRegistry::reference_default();
        let treatment = mana_treatment(vec![1, 2]);
        let rep = run_paired_ensemble::<ReferenceEvolutionWorld>(
            &treatment,
            &reg,
            &FactorDiff::genesis_exotic(),
        )
        .expect("paired run");
        let json = serde_json::to_string(&rep).expect("serializes");
        let back: PairedEnsembleReport =
            serde_json::from_str(&json).expect("deserializes (no NaN/inf leaked)");
        assert_eq!(back.seed_order, rep.seed_order);
        assert_eq!(
            back.treatment_manifest_fingerprint,
            rep.treatment_manifest_fingerprint
        );
        assert_eq!(back.effects.len(), rep.effects.len());
    }

    #[test]
    fn d1_paired_runner_rejects_bad_inputs_before_any_model_or_rng() {
        let reg = ObservableRegistry::reference_default();
        let treatment = mana_treatment(vec![1, 2]);
        PREFLIGHT_MODEL_CONSTRUCTIONS.store(0, Ordering::SeqCst);

        // An allowlist that does not cover the exotic regime must be rejected up front.
        let too_strict = FactorDiff {
            allowed_paths: vec!["seeds".to_string()],
        };
        assert!(matches!(
            run_paired_ensemble::<PreflightCountingModel>(&treatment, &reg, &too_strict),
            Err(ExperimentError::UndeclaredFactorDifference { .. })
        ));

        // A malformed registry is rejected before anything runs.
        let mut bad_reg = ObservableRegistry::reference_default();
        bad_reg.version = 999;
        assert!(run_paired_ensemble::<PreflightCountingModel>(
            &treatment,
            &bad_reg,
            &FactorDiff::genesis_exotic()
        )
        .is_err());

        // An invalid manifest (empty seeds) is rejected before anything runs.
        let mut empty = mana_treatment(vec![]);
        empty.seeds = vec![];
        assert!(matches!(
            run_paired_ensemble::<PreflightCountingModel>(
                &empty,
                &reg,
                &FactorDiff::genesis_exotic()
            ),
            Err(ExperimentError::EmptySeeds)
        ));
        assert_eq!(
            PREFLIGHT_MODEL_CONSTRUCTIONS.load(Ordering::SeqCst),
            0,
            "all malformed inputs must fail before model construction or RNG seeding"
        );
    }

    // ---- Independent (NOT paired) descriptive helper — see `compare_ensembles` docs ----------

    #[test]
    fn ae_s14_m4_effect_size_reports_difference_g_and_interval() {
        use crate::core::exotic_energy::ExoticEnergyLaw;
        let reg = ObservableRegistry::reference_default();
        let seeds = vec![1u64, 2, 3, 4, 5];

        // Control: baseline. Treatment: same everything but a live exotic law. The EU pools are
        // deliberately unaffected by the exotic field (AE-S05), so the EU effect size must be ~0
        // while the MU observable exists only in the treatment.
        let control_m = baseline_manifest(seeds.clone());
        let mut treatment_m = baseline_manifest(seeds.clone());
        treatment_m.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(150.0, 4));
        treatment_m
            .observable_ids
            .push("exotic.density_total".into());

        let control = run_ensemble::<ReferenceEvolutionWorld>(&control_m, &reg).expect("control");
        let treatment =
            run_ensemble::<ReferenceEvolutionWorld>(&treatment_m, &reg).expect("treatment");

        let cmp = compare_ensembles(&control, &treatment).expect("finite comparison");

        // N is reported per side and failures are preserved (none here).
        assert_eq!(cmp.control_n, 5);
        assert_eq!(cmp.treatment_n, 5);
        assert!(cmp.control_failed.is_empty() && cmp.treatment_failed.is_empty());

        // A shared observable is compared; the EU pools show ~zero effect (exotic never touches EU).
        let plants = cmp.effect_of("plants").expect("plants compared");
        assert_eq!(plants.n_control, 5);
        assert_eq!(plants.n_treatment, 5);
        assert!(
            plants.mean_difference.abs() < 1e-9,
            "EU effect must be ~0, got {}",
            plants.mean_difference
        );
        // The interval brackets the mean difference.
        assert!(plants.ci95_low.expect("n=5 lower bound") <= plants.mean_difference);
        assert!(plants.mean_difference <= plants.ci95_high.expect("n=5 upper bound"));

        // An observable present only in the treatment is reported as treatment-only, not silently
        // dropped and not compared against a fabricated zero.
        assert!(
            cmp.treatment_only
                .iter()
                .any(|o| o == "exotic.density_total"),
            "treatment-only observables must be listed: {:?}",
            cmp.treatment_only
        );
        assert!(cmp.effect_of("exotic.density_total").is_none());
    }

    #[test]
    fn ae_s14_m4_effect_size_detects_a_real_difference_with_correct_sign() {
        // A drought treatment must produce a NEGATIVE mean difference on plants with a large
        // standardized effect size, versus an undroughted control.
        let reg = ObservableRegistry::reference_default();
        let seeds = vec![11u64, 12, 13, 14, 15, 16];
        let control_m = baseline_manifest(seeds.clone());
        let mut treatment_m = baseline_manifest(seeds.clone());
        treatment_m.interventions = vec![drought(1, 600)];

        let control = run_ensemble::<ReferenceEvolutionWorld>(&control_m, &reg).expect("control");
        let treatment =
            run_ensemble::<ReferenceEvolutionWorld>(&treatment_m, &reg).expect("treatment");
        let cmp = compare_ensembles(&control, &treatment).expect("finite comparison");

        let plants = cmp.effect_of("plants").expect("plants compared");
        assert!(
            plants.mean_difference < 0.0,
            "drought must lower plants, got {}",
            plants.mean_difference
        );
        let hedges_g = plants.hedges_g.expect("both groups estimate spread");
        assert!(
            hedges_g.abs() > 1.0,
            "a drought should be a large standardized effect, got g={}",
            hedges_g
        );
        // The 95% interval excludes zero for a real effect this large.
        assert!(
            plants.ci95_high.expect("n=6 upper bound") < 0.0,
            "interval should exclude 0"
        );
    }

    #[test]
    fn ae_s14_m4_effect_size_preserves_failed_runs_and_degenerate_variance() {
        let reg = ObservableRegistry::reference_default();
        let seeds = vec![10u64, 11, 12, 13, 14]; // FlakyModel poisons seed 12
        let m = baseline_manifest(seeds.clone());
        let control = run_ensemble::<FlakyModel>(&m, &reg).expect("control");
        let treatment = run_ensemble::<FlakyModel>(&m, &reg).expect("treatment");
        let cmp = compare_ensembles(&control, &treatment).expect("finite comparison");

        // Failures are surfaced, never dropped; N counts only completed runs.
        assert_eq!(cmp.control_failed.len(), 1);
        assert_eq!(cmp.treatment_failed.len(), 1);
        assert_eq!(cmp.control_n, 4);
        assert_eq!(cmp.treatment_n, 4);

        // Control == treatment here, so the raw difference is exactly 0. If between-seed spread is
        // nonzero, g is the defined value 0; if spread is zero, g is explicitly undefined.
        let plants = cmp.effect_of("plants").expect("plants compared");
        assert_eq!(plants.mean_difference, 0.0);
        match plants.pooled_sd.expect("n=4 estimates pooled spread") {
            0.0 => assert_eq!(plants.hedges_g, None),
            _ => assert_eq!(plants.hedges_g, Some(0.0)),
        }
    }

    // ---- Task-5: hardened entry points ------------------------------------------------------

    #[test]
    fn run_manifest_seed_fails_on_unknown_observable_not_silently() {
        let reg = ObservableRegistry::reference_default();
        let mut m = baseline_manifest(vec![1]);
        m.observable_ids = vec!["not_a_real_observable".into()];
        let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 1, None, None);
        // Never silently executed: a structured Failed result mentioning the invalid manifest.
        assert!(matches!(res.status, RunStatus::Failed { .. }));
        if let RunStatus::Failed { reason, .. } = &res.status {
            assert!(reason.contains("manifest") && reason.contains("not_a_real_observable"));
        }
    }

    #[test]
    fn run_manifest_seed_fails_on_invalid_manifest_and_registry() {
        let reg = ObservableRegistry::reference_default();

        // Duplicate seed → invalid manifest → Failed (not executed).
        let mut m = baseline_manifest(vec![1]);
        m.seeds = vec![5, 5];
        let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 5, None, None);
        assert!(matches!(res.status, RunStatus::Failed { .. }));

        // Invalid registry (unknown schema version) → Failed.
        let mut bad_reg = ObservableRegistry::reference_default();
        bad_reg.version = 999;
        let good_m = baseline_manifest(vec![1]);
        let res = run_manifest_seed::<ReferenceEvolutionWorld>(&good_m, &bad_reg, 1, None, None);
        assert!(matches!(res.status, RunStatus::Failed { .. }));
        if let RunStatus::Failed { reason, .. } = &res.status {
            assert!(reason.contains("registry"));
        }
    }

    #[test]
    fn ensemble_rejects_invalid_and_empty_inputs_at_ensemble_level() {
        // ISSUE-2: an invalid manifest/registry (or an EMPTY seed set) is a structured ensemble-level
        // error — NOT a misleading zero-run or all-failed "summary". No fabricated seed is introduced.
        let reg = ObservableRegistry::reference_default();

        // Unknown observable → Err.
        let mut m = baseline_manifest(vec![1, 2, 3]);
        m.observable_ids = vec!["nope".into()];
        assert!(run_ensemble::<ReferenceEvolutionWorld>(&m, &reg).is_err());

        // Empty seed set → Err (previously a misleading zero-run summary).
        let mut m = baseline_manifest(vec![]);
        m.seeds = vec![];
        assert!(matches!(
            run_ensemble::<ReferenceEvolutionWorld>(&m, &reg),
            Err(ExperimentError::EmptySeeds)
        ));

        // Invalid registry → Err.
        let mut bad_reg = ObservableRegistry::reference_default();
        bad_reg.version = 999;
        let good_m = baseline_manifest(vec![1]);
        assert!(run_ensemble::<ReferenceEvolutionWorld>(&good_m, &bad_reg).is_err());
    }

    #[test]
    fn result_is_self_describing_for_every_emitted_observable() {
        // The manifest requests only a subset, but the model also emits detritus/npp/etc. Every
        // EMITTED observable must carry a registry spec (not just the requested ones).
        let reg = ObservableRegistry::reference_default();
        let m = baseline_manifest(vec![1]);
        let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 1, None, None);
        assert!(res.status.is_completed());
        let spec_ids: std::collections::BTreeSet<&str> =
            res.observable_specs.iter().map(|s| s.id.as_str()).collect();
        for (name, _) in &res.final_observables {
            assert!(
                spec_ids.contains(name.as_str()),
                "emitted observable '{name}' has no spec in the self-describing result"
            );
        }
    }

    // ---- AE-107 / AE-S09: headless checkpoint fork ------------------------------------------

    fn drought(id: u32, start: u64) -> InterventionCommand {
        use crate::core::intervention::{Curve, InterventionKind, Region};
        InterventionCommand {
            id,
            cause_id: id,
            kind: InterventionKind::RainfallDelta,
            region: Region::Global,
            start_tick: start,
            duration_ticks: 6000,
            intensity: 0.5,
            signed_negative: true,
            curve: Curve::Step,
            reversible: true,
        }
    }

    #[test]
    fn ae_s09_checkpoint_continuation_equals_uninterrupted_run() {
        // The snapshot (model state + cloned RNG) is COMPLETE: a control branch continued from the
        // fork with identical inputs reproduces an uninterrupted run bit-for-bit — proving the fork
        // is not a re-simulation and captures the full state including the RNG stream.
        let reg = ObservableRegistry::reference_default();
        let m = baseline_manifest(vec![99]);
        let uninterrupted = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 99, None, None);

        let fork =
            checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 99, 2500, &[]).expect("fork ok");
        assert_eq!(
            fork.control.final_checksum, uninterrupted.final_checksum,
            "checkpoint continuation must equal the uninterrupted run"
        );
        // With no treatment interventions, both branches are identical.
        assert_eq!(fork.control.final_checksum, fork.treatment.final_checksum);
        // Provenance records the fork lineage.
        assert_eq!(fork.control.provenance.fork_tick, Some(2500));
        assert_eq!(
            fork.control.provenance.parent_run_id.as_deref(),
            Some(fork.prefix.provenance.run_id.as_str())
        );
        assert!(fork.prefix.status.is_completed());
    }

    #[test]
    fn ae_s09_post_fork_treatment_diverges_only_after_the_fork() {
        // A treatment intervention applied AFTER the fork tick makes the branches diverge, but they
        // share the identical pre-fork snapshot, so the prefix is common history.
        let reg = ObservableRegistry::reference_default();
        let m = baseline_manifest(vec![7]);
        let fork_tick = 3000;
        // Drought starting after the fork only affects the treatment branch.
        let extra = vec![drought(1, fork_tick + 60)];
        let fork = checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 7, fork_tick, &extra)
            .expect("fork ok");

        // Both branches restored from the SAME snapshot → identical pre-fork state (checksum of the
        // prefix is shared). The treatment diverges by the end.
        assert_ne!(
            fork.control.final_checksum, fork.treatment.final_checksum,
            "a post-fork intervention must change the treatment branch"
        );
        // Plants fall in the treatment (drought), a real declared effect.
        assert!(fork.delta_of("plants").unwrap() < 0.0);
        assert!(!fork.declared_factors.is_empty());
        // Both children point at the same parent prefix and fork tick.
        assert_eq!(
            fork.control.provenance.parent_run_id,
            fork.treatment.provenance.parent_run_id
        );
        assert_eq!(fork.treatment.provenance.fork_tick, Some(fork_tick));
    }

    #[test]
    fn checkpoint_fork_rejects_bad_fork_tick() {
        let reg = ObservableRegistry::reference_default();
        let m = baseline_manifest(vec![1]);
        assert!(checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 1, 0, &[]).is_err());
        assert!(
            checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 1, m.duration_ticks, &[]).is_err()
        );
    }

    // ---- ISSUE-1: a seed absent from the manifest is rejected before any model/RNG --------------

    #[test]
    fn run_manifest_seed_rejects_seed_not_in_manifest() {
        let reg = ObservableRegistry::reference_default();
        let m = baseline_manifest(vec![1, 2, 3]);
        let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 999, None, None);
        assert!(matches!(res.status, RunStatus::Failed { .. }));
        if let RunStatus::Failed { reason, tick, .. } = &res.status {
            assert_eq!(*tick, 0, "rejected before any tick was run");
            assert!(reason.contains("seed 999") && reason.contains("seed set"));
        }
        // A declared seed is accepted.
        assert!(
            run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 2, None, None)
                .status
                .is_completed()
        );
    }

    #[test]
    fn checkpoint_fork_rejects_seed_not_in_manifest() {
        let reg = ObservableRegistry::reference_default();
        let m = baseline_manifest(vec![1, 2, 3]);
        assert!(matches!(
            checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 42, 100, &[]),
            Err(ExperimentError::SeedNotInManifest { seed: 42 })
        ));
    }

    // ---- ISSUE-3: a transient (series-only) observable is described + warned --------------------

    // Emits an extra observable ONLY on the first two ticks, so it appears in the series but never in
    // `final_observables`. It is not in the registry, so the union must surface a missing-spec warning.
    struct TransientModel {
        inner: ReferenceEvolutionWorld,
        ticks: u64,
    }
    impl ExperimentModel for TransientModel {
        type Snapshot = (<ReferenceEvolutionWorld as ExperimentModel>::Snapshot, u64);

        fn model_version() -> &'static str {
            "test-transient-observable/1"
        }

        fn from_manifest(
            laws: &WorldLawSet,
            initial: &InitialConditionSet,
            forcings: &[ExoticIntervention],
            seed: u64,
            grid: (usize, usize),
            run_ticks: u64,
        ) -> Result<Self, ExperimentError> {
            Ok(TransientModel {
                inner: ReferenceEvolutionWorld::from_manifest(
                    laws, initial, forcings, seed, grid, run_ticks,
                )?,
                ticks: 0,
            })
        }
        fn snapshot(&self) -> Self::Snapshot {
            (self.inner.snapshot(), self.ticks)
        }
        fn from_snapshot(snapshot: &Self::Snapshot) -> Result<Self, ExperimentError> {
            Ok(TransientModel {
                inner: ReferenceEvolutionWorld::from_snapshot(&snapshot.0)?,
                ticks: snapshot.1,
            })
        }
        fn step(
            &mut self,
            clock: &SimClock,
            active: &[&InterventionCommand],
            ledger: &mut CausalLedger,
            rng: &mut StdRng,
        ) {
            self.inner.step(clock, active, ledger, rng);
            self.ticks += 1;
        }
        fn checksum(&self) -> u32 {
            self.inner.checksum()
        }
        fn observables(&self) -> Vec<(String, f64)> {
            let mut o = self.inner.observables();
            if self.ticks <= 2 {
                o.push(("transient.only_in_series".into(), 1.0));
            }
            o
        }
    }

    #[test]
    fn result_describes_transient_series_only_observable_with_missing_spec_warning() {
        let reg = ObservableRegistry::reference_default();
        let m = ExperimentManifest {
            duration_ticks: 10,
            sample_period: 1, // sample every tick, so the transient lands in the series
            ..baseline_manifest(vec![1])
        };
        let res = run_manifest_seed::<TransientModel>(&m, &reg, 1, None, None);
        assert!(res.status.is_completed());
        // The transient is present in the series but NOT in final_observables.
        assert!(res.series.iter().any(|s| s
            .observables
            .iter()
            .any(|(n, _)| n == "transient.only_in_series")));
        assert!(!res
            .final_observables
            .iter()
            .any(|(n, _)| n == "transient.only_in_series"));
        // The union-based metadata surfaces the missing-spec warning for the series-only observable.
        assert!(
            res.warnings
                .iter()
                .any(|w| w.contains("transient.only_in_series") && w.contains("no registry spec")),
            "series-only observable must be reported as a missing-spec warning; warnings: {:?}",
            res.warnings
        );
    }

    // ---- ISSUE-4: checkpoint window semantics + failing prefix ----------------------------------

    #[test]
    fn checkpoint_rejects_treatment_extra_outside_post_fork_window() {
        let reg = ObservableRegistry::reference_default();
        let m = baseline_manifest(vec![7]); // duration 6000
        let fork_tick = 3000u64;

        // start_tick == fork_tick → belongs to the shared prefix → rejected.
        assert!(matches!(
            checkpoint_fork::<ReferenceEvolutionWorld>(
                &m,
                &reg,
                7,
                fork_tick,
                &[drought(1, fork_tick)]
            ),
            Err(ExperimentError::InapplicableIntervention { id: 1, .. })
        ));

        // start_tick after the run ends → rejected.
        assert!(matches!(
            checkpoint_fork::<ReferenceEvolutionWorld>(
                &m,
                &reg,
                7,
                fork_tick,
                &[drought(2, m.duration_ticks + 1)]
            ),
            Err(ExperimentError::InapplicableIntervention { id: 2, .. })
        ));

        // Boundary OK cases: first post-fork tick, and the very last tick.
        assert!(
            checkpoint_fork::<ReferenceEvolutionWorld>(
                &m,
                &reg,
                7,
                fork_tick,
                &[drought(3, fork_tick + 1)]
            )
            .is_ok(),
            "start_tick == fork_tick+1 must be accepted"
        );
        assert!(
            checkpoint_fork::<ReferenceEvolutionWorld>(
                &m,
                &reg,
                7,
                fork_tick,
                &[drought(4, m.duration_ticks)]
            )
            .is_ok(),
            "start_tick == duration_ticks must be accepted (it fires on the last processed tick)"
        );
    }

    // A model whose observables go non-finite partway through the prefix, to exercise the
    // failing-prefix path of checkpoint_fork.
    struct PrefixFailModel {
        inner: ReferenceEvolutionWorld,
        ticks: u64,
    }
    impl ExperimentModel for PrefixFailModel {
        type Snapshot = (<ReferenceEvolutionWorld as ExperimentModel>::Snapshot, u64);

        fn model_version() -> &'static str {
            "test-prefix-failure/1"
        }

        fn from_manifest(
            laws: &WorldLawSet,
            initial: &InitialConditionSet,
            forcings: &[ExoticIntervention],
            seed: u64,
            grid: (usize, usize),
            run_ticks: u64,
        ) -> Result<Self, ExperimentError> {
            Ok(PrefixFailModel {
                inner: ReferenceEvolutionWorld::from_manifest(
                    laws, initial, forcings, seed, grid, run_ticks,
                )?,
                ticks: 0,
            })
        }
        fn snapshot(&self) -> Self::Snapshot {
            (self.inner.snapshot(), self.ticks)
        }
        fn from_snapshot(snapshot: &Self::Snapshot) -> Result<Self, ExperimentError> {
            Ok(PrefixFailModel {
                inner: ReferenceEvolutionWorld::from_snapshot(&snapshot.0)?,
                ticks: snapshot.1,
            })
        }
        fn step(
            &mut self,
            clock: &SimClock,
            active: &[&InterventionCommand],
            ledger: &mut CausalLedger,
            rng: &mut StdRng,
        ) {
            self.inner.step(clock, active, ledger, rng);
            self.ticks += 1;
        }
        fn checksum(&self) -> u32 {
            self.inner.checksum()
        }
        fn observables(&self) -> Vec<(String, f64)> {
            let mut o = self.inner.observables();
            if self.ticks >= 100 {
                o.push(("diverged".into(), f64::NAN));
            }
            o
        }
    }

    // ---- DEFECT A: a completed RunResult must survive a real JSON round-trip -----------------

    #[test]
    fn defect_a_run_result_json_round_trips_for_baseline_and_treatment() {
        use crate::core::exotic_energy::ExoticEnergyLaw;
        let reg = ObservableRegistry::reference_default();

        // Demonstrate the exact failure mode this guards against: an infinite bound serializes to
        // `null` and can no longer be read back as an f64. This is why the registry uses finite
        // bounds and why the round-trip below is a real regression guard.
        let mut infinite_spec = reg.specs()[0].clone();
        infinite_spec.valid_max = f64::INFINITY;
        let bad_json = serde_json::to_string(&infinite_spec).expect("serializes");
        assert!(bad_json.contains("\"valid_max\":null"));
        assert!(
            serde_json::from_str::<ObservableSpec>(&bad_json).is_err(),
            "a null bound must fail to deserialize — proving infinities lose data"
        );

        // Baseline (exotic = None) and treatment (live MU field) both round-trip losslessly.
        let baseline = baseline_manifest(vec![5]);
        let mut treatment = baseline_manifest(vec![5]);
        treatment.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(150.0, 4));
        treatment.observable_ids.push("exotic.density_total".into());

        for m in [baseline, treatment] {
            let res = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 5, None, None);
            assert!(res.status.is_completed());
            let json = serde_json::to_string(&res).expect("RunResult serializes");
            // Deserialization is the rigorous proof: `serde_json` renders a non-finite float as
            // `null`, and `null` cannot be parsed back into an `f64` — so a run carrying an infinite
            // spec bound (or observable) fails HERE. (Legitimate `Option::None` fields such as
            // `parent_run_id` also serialize as null, which is why a naive substring check would be
            // wrong; a successful round-trip plus the equality assertions below is what matters.)
            let back: RunResult = serde_json::from_str(&json).expect("RunResult deserializes");

            // Structural / integer / string content is EXACT across the round-trip.
            assert_eq!(back.final_checksum, res.final_checksum);
            assert_eq!(back.provenance, res.provenance);
            assert_eq!(back.observable_specs, res.observable_specs);
            assert_eq!(back.series.len(), res.series.len());
            assert_eq!(
                back.final_observables.len(),
                res.final_observables.len(),
                "no observable may be dropped by the round-trip"
            );

            // Float PAYLOAD values survive to serde_json's documented ±1 ULP accuracy (the
            // `float_roundtrip` feature is not enabled — the same caveat `dynamic_fields`' save/load
            // test records). This is an honest property of JSON export, not bit-exactness.
            for ((kb, vb), (kr, vr)) in back.final_observables.iter().zip(&res.final_observables) {
                assert_eq!(kb, kr);
                let tol = 1e-12 * vr.abs().max(1.0);
                assert!(
                    (vb - vr).abs() <= tol,
                    "observable '{kb}' drifted beyond 1 ULP: {vb} vs {vr}"
                );
            }

            // The spec ranges survived intact and finite (the point of the fix).
            for s in &back.observable_specs {
                assert!(s.valid_min.is_finite() && s.valid_max.is_finite());
            }
        }
    }

    // ---- DEFECT B: checkpoint_fork validates treatment_extra with the shared helper ----------

    #[test]
    fn defect_b_checkpoint_validates_treatment_extra_values_and_ids() {
        let reg = ObservableRegistry::reference_default();
        let mut m = baseline_manifest(vec![7]);
        m.interventions = vec![drought(1, 100)]; // a base intervention with id 1
        let fork_tick = 3000u64;

        // Invalid VALUE inside an otherwise well-placed extra (NaN intensity) → rejected.
        let mut bad = drought(2, fork_tick + 10);
        bad.intensity = f32::NAN;
        assert!(
            checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 7, fork_tick, &[bad]).is_err(),
            "a NaN-intensity extra must be rejected"
        );

        // Invalid geometry inside an extra → rejected.
        let mut bad = drought(3, fork_tick + 10);
        bad.region = crate::core::intervention::Region::Rect {
            min_x: 9,
            min_y: 0,
            max_x: 1,
            max_y: 1,
        };
        assert!(
            checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 7, fork_tick, &[bad]).is_err(),
            "an inverted-Rect extra must be rejected"
        );

        // Duplicate id WITHIN the extras → rejected.
        let dup = vec![drought(9, fork_tick + 10), drought(9, fork_tick + 20)];
        assert!(matches!(
            checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 7, fork_tick, &dup),
            Err(ExperimentError::DuplicateId { .. })
        ));

        // Duplicate id AGAINST the base interventions (base uses id 1) → rejected.
        let clash = vec![drought(1, fork_tick + 10)];
        assert!(matches!(
            checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 7, fork_tick, &clash),
            Err(ExperimentError::DuplicateId { .. })
        ));

        // A clean, uniquely-identified, post-fork extra is accepted.
        assert!(checkpoint_fork::<ReferenceEvolutionWorld>(
            &m,
            &reg,
            7,
            fork_tick,
            &[drought(42, fork_tick + 10)]
        )
        .is_ok());
    }

    #[test]
    fn defect_b_checkpoint_enforces_combined_intervention_limit() {
        use crate::core::experiment::MAX_INTERVENTIONS;
        let reg = ObservableRegistry::reference_default();
        let m = baseline_manifest(vec![7]);
        // More extras than the combined ceiling → structured ResourceLimit, before any model/RNG.
        let extras: Vec<InterventionCommand> = (0..(MAX_INTERVENTIONS as u32 + 1))
            .map(|i| drought(i, 3001))
            .collect();
        assert!(matches!(
            checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 7, 3000, &extras),
            Err(ExperimentError::ResourceLimit { .. })
        ));
    }

    // ---- DEFECT P1: effective-treatment provenance + structured report ----------------------

    #[test]
    fn p1_treatment_provenance_uses_effective_manifest_fingerprint() {
        let reg = ObservableRegistry::reference_default();
        let m = baseline_manifest(vec![7]);
        let fork_tick = 3000u64;
        let extra = drought(42, fork_tick + 10);

        let fork = checkpoint_fork::<ReferenceEvolutionWorld>(
            &m,
            &reg,
            7,
            fork_tick,
            std::slice::from_ref(&extra),
        )
        .expect("fork ok");

        // Independently reconstruct the effective treatment manifest: base + appended extras.
        let mut effective = m.clone();
        effective.interventions.push(extra.clone());
        let effective_fp = effective.fingerprint();
        let base_fp = m.fingerprint();
        assert_ne!(
            effective_fp, base_fp,
            "appending an extra must change the manifest fingerprint"
        );

        // Control keeps BASE provenance; treatment carries the EFFECTIVE fingerprint.
        assert_eq!(fork.control.provenance.manifest_fingerprint, base_fp);
        assert_eq!(
            fork.treatment.provenance.manifest_fingerprint, effective_fp,
            "treatment provenance must fingerprint the effective treatment input"
        );
        assert_ne!(
            fork.treatment.provenance.manifest_fingerprint,
            fork.control.provenance.manifest_fingerprint
        );
        // Run ids must differ too (they embed the fingerprint), so the two branches are
        // independently addressable for replay.
        assert_ne!(
            fork.treatment.provenance.run_id,
            fork.control.provenance.run_id
        );
        assert!(fork
            .treatment
            .provenance
            .run_id
            .contains(&format!("{effective_fp:016x}")));

        // The prefix is shared history under the BASE manifest.
        assert_eq!(fork.prefix.provenance.manifest_fingerprint, base_fp);
        assert!(fork.prefix.provenance.parent_run_id.is_none());
    }

    #[test]
    fn p1_no_extras_keeps_control_and_treatment_fingerprints_equal() {
        let reg = ObservableRegistry::reference_default();
        let m = baseline_manifest(vec![7]);
        let fork = checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 7, 3000, &[]).expect("ok");
        let base_fp = m.fingerprint();
        assert_eq!(fork.control.provenance.manifest_fingerprint, base_fp);
        assert_eq!(
            fork.treatment.provenance.manifest_fingerprint, base_fp,
            "with no extras the effective treatment input IS the base manifest"
        );
        assert_eq!(fork.prefix.provenance.manifest_fingerprint, base_fp);
    }

    #[test]
    fn p1_report_carries_structured_extras_that_survive_json_roundtrip() {
        let reg = ObservableRegistry::reference_default();
        let m = baseline_manifest(vec![7]);
        let fork_tick = 3000u64;
        let extra = drought(42, fork_tick + 10);
        let fork = checkpoint_fork::<ReferenceEvolutionWorld>(
            &m,
            &reg,
            7,
            fork_tick,
            std::slice::from_ref(&extra),
        )
        .expect("fork ok");

        // The report carries the FULL structured command, not a lossy "kind@start" string.
        assert_eq!(fork.treatment_extra, vec![extra.clone()]);
        let json = serde_json::to_string(&fork).expect("report serializes");
        let back: CheckpointForkReport = serde_json::from_str(&json).expect("report deserializes");
        assert_eq!(back.treatment_extra, vec![extra]);
        assert_eq!(
            back.treatment.provenance.manifest_fingerprint,
            fork.treatment.provenance.manifest_fingerprint
        );
        assert_eq!(back.fork_tick, fork_tick);
    }

    #[test]
    fn p1_fork_remains_deterministic_and_tick_exact() {
        // The provenance change must not perturb the simulation itself.
        let reg = ObservableRegistry::reference_default();
        let m = baseline_manifest(vec![99]);
        let uninterrupted = run_manifest_seed::<ReferenceEvolutionWorld>(&m, &reg, 99, None, None);
        let a = checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 99, 2500, &[]).expect("ok");
        let b = checkpoint_fork::<ReferenceEvolutionWorld>(&m, &reg, 99, 2500, &[]).expect("ok");
        assert_eq!(a.control.final_checksum, uninterrupted.final_checksum);
        assert_eq!(a.control.final_checksum, b.control.final_checksum);
        assert_eq!(a.treatment.final_checksum, b.treatment.final_checksum);
        assert_eq!(a.control.provenance.fork_tick, Some(2500));
    }

    // ---- DEFECT P2: genesis_fork preflight-validates the registry ---------------------------

    #[test]
    fn p2_genesis_fork_validates_registry_before_any_model_work() {
        use crate::core::exotic_energy::ExoticEnergyLaw;
        let mut treatment = baseline_manifest(vec![7]);
        treatment.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(150.0, 4));

        // A malformed registry (unsupported version) must fail preflight with a structured error.
        let mut bad_reg = ObservableRegistry::reference_default();
        bad_reg.version = 999;
        assert!(matches!(
            genesis_fork::<ReferenceEvolutionWorld>(
                &treatment,
                &bad_reg,
                &FactorDiff::genesis_exotic()
            ),
            Err(ExperimentError::UnsupportedSchemaVersion { .. })
        ));

        // A duplicate-id registry likewise fails preflight, never stepping a model.
        let mut dup_reg = ObservableRegistry::reference_default();
        dup_reg.push_duplicate_for_test();
        assert!(matches!(
            genesis_fork::<ReferenceEvolutionWorld>(
                &treatment,
                &dup_reg,
                &FactorDiff::genesis_exotic()
            ),
            Err(ExperimentError::DuplicateId { .. })
        ));

        // The valid registry still runs the fork.
        let good = ObservableRegistry::reference_default();
        assert!(genesis_fork::<ReferenceEvolutionWorld>(
            &treatment,
            &good,
            &FactorDiff::genesis_exotic()
        )
        .is_ok());
    }

    #[test]
    fn checkpoint_fork_fails_structurally_when_prefix_diverges() {
        let reg = ObservableRegistry::reference_default();
        let m = baseline_manifest(vec![1]); // duration 6000
                                            // The prefix diverges at tick 100, well before fork_tick 2500.
        let err = checkpoint_fork::<PrefixFailModel>(&m, &reg, 1, 2500, &[]).unwrap_err();
        assert!(matches!(
            err,
            ExperimentError::CheckpointPrefixFailed { tick: 100, .. }
        ));
    }
}
