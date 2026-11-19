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
use crate::core::world_artifact::WorldIdentity;
use rand::{rngs::StdRng, SeedableRng};
use serde::{Deserialize, Serialize};

mod ensemble;
mod fork;
mod statistics;

pub use ensemble::*;
pub use fork::*;
pub use statistics::*;

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

    /// Named scalar observables in a STABLE order. `drive` calls this on every **sample** tick and
    /// once at the end of the run; `run_branch_tree` also calls it at the start of each node.
    ///
    /// For a model with window observables (counts since the previous read, such as the live
    /// world's `live.births`), the end-of-run read in [`RunResult::final_observables`] therefore
    /// counts the events since the last sample: zero when `duration_ticks` is a multiple of
    /// `sample_period`, the whole run when `sample_period == 0`.
    fn observables(&self) -> Vec<(String, f64)>;

    /// The observables on a tick that is **not** sampled, read only to check that every value is
    /// finite and every name unique. The default is [`Self::observables`].
    ///
    /// A model whose observables count events since the previous read (a window that each read
    /// closes) must override this with a read that leaves the window open. Otherwise the per-tick
    /// check closes the window on every tick, and each sample counts only its own tick.
    ///
    /// Known limit, for the live world: its snapshot does not carry the window
    /// (`VitalStatistics` is outside both the checksum and the save format). A checkpoint fork at a
    /// tick that is not a sample tick therefore starts each branch with an empty window. The events
    /// of the open window then appear in the prefix's `final_observables`, not in the branch's
    /// first sample. Checksums are unaffected.
    fn observables_unsampled(&self) -> Vec<(String, f64)> {
        self.observables()
    }

    /// The current MU budget, if this model has an exotic-energy field (`None` on the baseline path).
    fn exotic_budget(&self) -> Option<ExoticEnergyBudget> {
        None
    }

    /// Declared source of the simulation's terrain when the model runs on a `TerrainMap` (e.g.
    /// `Some("generated")` for [`crate::core::live_experiment::LiveExperimentAdapter`]).
    fn terrain_source(&self) -> Option<&'static str> {
        None
    }

    /// Observed [`WorldIdentity`] (seed, generator version, dims, FNV-1a checksum) of the
    /// `TerrainMap` the model actually ran on.
    fn world_identity(&self) -> Option<WorldIdentity> {
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
    /// Feature ecological-selection F: which fitness rule this run ranked epoch replacement by,
    /// read from the declared initial conditions (never the environment). Empty on provenance
    /// records written before branch F existed — `serde(default)` keeps them loadable.
    #[serde(default)]
    pub fitness_rule: String,
    /// Declared terrain source (`Some("generated")` for live headless runs); `None` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terrain_source: Option<String>,
    /// Observed [`WorldIdentity`] of the terrain the simulation actually ran on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world_identity: Option<WorldIdentity>,
}

impl RunProvenance {
    pub(crate) fn with_model_terrain<M: ExperimentModel>(mut self, model: &M) -> Self {
        self.terrain_source = model.terrain_source().map(str::to_owned);
        self.world_identity = model.world_identity();
        self
    }

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
            fitness_rule: declared_fitness_rule(&manifest.initial_conditions),
            terrain_source: None,
            world_identity: None,
        }
    }
}

/// Feature ecological-selection F: which fitness rule the manifest declares, from the
/// initial-condition key alone. Absent ⇒ locomotion, which is what an absent key means
/// everywhere else in this feature.
pub(crate) fn declared_fitness_rule(
    initial: &crate::core::experiment::InitialConditionSet,
) -> String {
    use crate::core::live_experiment::LIVE_KEY_ECOLOGICAL_FITNESS;
    for (key, value) in &initial.values {
        if key == LIVE_KEY_ECOLOGICAL_FITNESS && *value == 1.0 {
            return crate::evolution::fitness::FITNESS_ECOLOGICAL.to_owned();
        }
    }
    crate::evolution::fitness::FITNESS_LOCOMOTION.to_owned()
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
            let sampled = sample_period != 0 && tick.is_multiple_of(sample_period);
            let observables = if sampled {
                model.observables()
            } else {
                model.observables_unsampled()
            };
            if let Some((name, _)) = observables.iter().find(|(_, value)| !value.is_finite()) {
                return Some(format!("observable '{name}' became non-finite"));
            }
            if let Some(name) = duplicate_observable_name(&observables) {
                return Some(format!("observable '{name}' was emitted more than once"));
            }
            if sampled {
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
    let provenance = provenance.with_model_terrain(&model);

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

#[cfg(test)]
mod tests;
