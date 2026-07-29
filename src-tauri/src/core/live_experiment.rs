//! The live Bevy world as an [`ExperimentModel`] (§3.3 / G2 task 1).
//!
//! Until this module existed, "evolution simulation" and "evolution experiment" were two different
//! systems. The scientific machinery — [`ExperimentManifest`], [`WorldLawSet`], [`SimClock`],
//! [`InterventionCommand`], [`CausalLedger`], [`ObservableRegistry`] — drove
//! [`crate::core::reference_world::ReferenceEvolutionWorld`], a small deterministic aggregate model,
//! while the thing the app actually runs was a Bevy `World` that none of it could reach.
//! [`LiveExperimentAdapter`] is the seam that closes that: it runs the **real** schedule
//! ([`crate::core::simulation_schedule::build_tick_schedule`] — the same function
//! `SimulationEngine::start` calls) under the same manifest, the same clock, the same intervention
//! semantics and the same observable registry type as the reference path.
//!
//! # Why an adapter rather than `impl ExperimentModel for World`
//!
//! Two of the trait's methods take `&self`, and every Bevy query needs `&mut World`. The dishonest
//! ways out are a cached value that silently goes stale and an `unsafe` cast; the honest one is a
//! `RefCell<World>`, which is what this is. `checksum` and `observables` borrow the world mutably at
//! the moment they are asked, so they always describe the world as it actually is.
//!
//! # What the live path is, and what it is not
//!
//! It **is** the app's schedule, the app's systems, the app's terrain, resource field, closed-EU
//! ledger and season clock. It **is not** the app's threads: there is no learner, no evolution
//! thread, no websocket server and no Tauri handle. That is deliberate rather than a shortcut —
//! each of those is paced by a wall clock or a network, and a run whose trajectory depends on when a
//! background thread happened to deliver a model update is not reproducible in the sense a manifest
//! promises. The shared [`BrainModel`] is therefore **frozen** for the length of a run, which is the
//! same rule [`crate::core::determinism::DeterministicMode::allows_external_ai`] applies to the
//! external model: it may propose, never decide, mid-run.
//!
//! Agent inference is not skipped, though — skipping it would mean measuring a different engine. The
//! adapter pumps the inference channel synchronously between ticks with
//! [`crate::ai::model::run_inference_batch`], the same function the worker thread calls, so a request
//! made on tick *n* is answered on tick *n+1* exactly as the live worker's latency produces.
//!
//! # Numerical identity across paths is not claimed
//!
//! The reference world is seven scalars relaxing toward carrying capacities; the live world is
//! thousands of entities, a 256×256 resource field and a physics solver. They are not two
//! implementations of one equation and their numbers will never match. What *is* claimed, and
//! tested, is that a **shared law** moves both in the same direction with the same meaning — a
//! deforestation forcing lowers `plants` in EU in both, and `plants` means the same thing in both
//! registries.

use std::cell::RefCell;

use bevy_ecs::prelude::*;
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};

use crate::core::causal::{CausalLedger, CauseId};
use crate::core::ecology::{EcosystemBiomass, ResourceField};
use crate::core::energy_ledger::{Compartment, EnergyEvent, EnergyLedger};
use crate::core::exotic_energy::ExoticIntervention;
use crate::core::experiment::{ExperimentError, InitialConditionSet, WorldLawSet};
use crate::core::experiment_runner::ExperimentModel;
use crate::core::intervention::{InterventionCommand, InterventionKind};
use crate::core::sim_clock::{SimClock, ECOLOGY_PERIOD};
use crate::core::simulation_state::SavedSimulationState;

/// Schema version of [`LiveExperimentState`], which travels inside a snapshot.
pub const LIVE_EXPERIMENT_SCHEMA_VERSION: u16 = 1;

/// The model-version string a live run records in its provenance. Distinct from
/// [`crate::core::experiment_runner::MODEL_VERSION`] on purpose: a checksum from the live world and
/// one from the reference world are not comparable, and provenance is where that has to be visible.
pub const LIVE_MODEL_VERSION: &str = "live-bevy-world/1";

/// Intensity 1.0 of a [`InterventionKind::TemperatureDelta`], in degrees Celsius of homeostatic
/// target shift.
///
/// Chosen to match the magnitude the engine's own environmental events already use: a
/// `TemperatureSpike` shifts `temp_target` by 5 °C (`apply_environmental_effects_system`), so a
/// full-intensity declared forcing being twice that puts a manifest's strongest heat wave in the
/// same physical range as the game's, rather than in a unit nobody can interpret.
pub const TEMPERATURE_DELTA_FULL_SCALE_C: f32 = 10.0;

/// Fraction of the detritus pool a full-intensity [`InterventionKind::AddNutrient`] moves into
/// standing crop, per ecology-band firing.
pub const NUTRIENT_FULL_SCALE_FRACTION: f64 = 0.10;

// ---- Initial conditions the live world honours -------------------------------------------------

/// Prefix every initial-condition key the live adapter understands carries.
pub const LIVE_KEY_PREFIX: &str = "live.";

/// Founding population size.
pub const LIVE_KEY_FOUNDERS: &str = "live.founders";
/// Fraction of founders that are predators, in `[0, 1]`.
pub const LIVE_KEY_PREDATOR_FRACTION: &str = "live.predator_fraction";
/// Trees placed at genesis.
pub const LIVE_KEY_TREES: &str = "live.trees";
/// Lakes placed at genesis.
pub const LIVE_KEY_LAKES: &str = "live.lakes";
/// Ceiling on spawned food items.
pub const LIVE_KEY_FOOD_CAP: &str = "live.food_cap";
/// Whether founders get their own heritable brain (ADR-0003), as a **declared** input.
///
/// `1.0` is on, `0.0` and **absent** are off. Absent meaning off is what keeps every live manifest
/// written before this key existed — including the four in `live_experiment_tests.rs` and the E2
/// control — bit-identical to what it was.
///
/// It exists because `BrainPolicy` was otherwise unreachable from a manifest:
/// [`build_live_world`] deliberately replaces `init_world`'s `BrainPolicy::from_env()` so a run's
/// trajectory cannot depend on `ANIMA_EVOLVED_BRAINS`, and that left `BrainPolicy::default()` as
/// the only policy a live experiment could have. Taking the environment away was right; leaving no
/// declared input in its place meant one arm of a controlled comparison could not be expressed.
pub const LIVE_KEY_EVOLVED_BRAINS: &str = "live.evolved_brains";

/// Every key [`LiveWorldConfig::from_initial_conditions`] accepts.
pub const LIVE_KEYS: [&str; 6] = [
    LIVE_KEY_FOUNDERS,
    LIVE_KEY_PREDATOR_FRACTION,
    LIVE_KEY_TREES,
    LIVE_KEY_LAKES,
    LIVE_KEY_FOOD_CAP,
    LIVE_KEY_EVOLVED_BRAINS,
];

/// The genesis the manifest asked for.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiveWorldConfig {
    pub founders: usize,
    pub predator_fraction: f64,
    pub trees: usize,
    pub lakes: usize,
    pub food_cap: usize,
    /// ADR-0003's per-agent brains, off unless a manifest asks. `#[serde(default)]` so a config
    /// serialized before this field existed reads back as the legacy path rather than failing —
    /// the same direction of default the key itself takes.
    #[serde(default)]
    pub evolved_brains: bool,
}

impl Default for LiveWorldConfig {
    /// The engine's own genesis: ten founders, seven prey and three predators, and the stock
    /// environment-spawn settings — the population `SimulationEngine::start` builds when there is no
    /// save to load. Brains are off, which is `BrainPolicy::default()` and the ADR-0003 baseline.
    fn default() -> Self {
        Self {
            founders: 10,
            predator_fraction: 0.3,
            trees: 8,
            lakes: 2,
            food_cap: crate::core::ecs::FoodSpawnSettings::default().max_food_count,
            evolved_brains: false,
        }
    }
}

impl LiveWorldConfig {
    /// Read the config out of a manifest's initial conditions.
    ///
    /// **Every** key must be one this adapter honours. Silently ignoring an unknown key is the
    /// failure mode worth refusing: a manifest declaring `plants = 100` would run, finish, and
    /// report numbers from a world that never had its declared initial state — which is a
    /// reproducible run of the wrong experiment.
    pub fn from_initial_conditions(initial: &InitialConditionSet) -> Result<Self, ExperimentError> {
        let mut config = LiveWorldConfig::default();
        for (key, value) in &initial.values {
            if !LIVE_KEYS.contains(&key.as_str()) {
                return Err(ExperimentError::InvalidLaw {
                    reason: format!(
                        "initial condition '{key}' is not one the live Bevy world honours; it \
                         accepts {LIVE_KEYS:?}. A key this adapter ignores would run the wrong \
                         experiment and report it as the declared one."
                    ),
                });
            }
            if !value.is_finite() {
                return Err(ExperimentError::NotFinite { field: key.clone() });
            }
            match key.as_str() {
                LIVE_KEY_FOUNDERS => config.founders = bounded_count(key, *value, 0.0, 4096.0)?,
                LIVE_KEY_TREES => config.trees = bounded_count(key, *value, 0.0, 4096.0)?,
                LIVE_KEY_LAKES => config.lakes = bounded_count(key, *value, 0.0, 1024.0)?,
                LIVE_KEY_FOOD_CAP => config.food_cap = bounded_count(key, *value, 0.0, 65536.0)?,
                LIVE_KEY_PREDATOR_FRACTION => {
                    if !(0.0..=1.0).contains(value) {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: *value,
                            min: 0.0,
                            max: 1.0,
                        });
                    }
                    config.predator_fraction = *value;
                }
                // A boolean carried in an `f64`, so the only honest reading is an exact one.
                // Rounding 0.5 to *something* would run an arm nobody declared and then report it
                // under the declared manifest's fingerprint.
                LIVE_KEY_EVOLVED_BRAINS => match *value {
                    0.0 => config.evolved_brains = false,
                    1.0 => config.evolved_brains = true,
                    other => {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: other,
                            min: 0.0,
                            max: 1.0,
                        })
                    }
                },
                _ => unreachable!("key membership was checked above"),
            }
        }
        Ok(config)
    }

    /// How many founders are predators.
    pub fn predators(&self) -> usize {
        ((self.founders as f64) * self.predator_fraction).round() as usize
    }
}

fn bounded_count(key: &str, value: f64, min: f64, max: f64) -> Result<usize, ExperimentError> {
    if !(min..=max).contains(&value) || value.fract() != 0.0 {
        return Err(ExperimentError::OutOfRange {
            field: key.to_string(),
            value,
            min,
            max,
        });
    }
    Ok(value as usize)
}

// ---- The world's experiment state ---------------------------------------------------------------

/// What a running world knows about the experiment it is part of.
///
/// Inserted as a resource by [`LiveExperimentAdapter`] and **absent** from an ordinary app run,
/// which is what keeps every system that reads it inert by default. It is also the block
/// [`SavedSimulationState::experiment`] carries, so one type describes the state both live and on
/// disk rather than two that can drift.
#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct LiveExperimentState {
    pub schema_version: u16,
    /// Identity of the manifest this run belongs to.
    pub manifest_fingerprint: u64,
    /// Identity of the law set, which is **immutable for the run** (ER01). A resumed run whose laws
    /// hash differently is a different experiment and must be refused, not merged.
    pub law_fingerprint: u64,
    /// Identity of the observable registry the run's results are described by.
    pub registry_fingerprint: u64,
    pub seed: u64,
    /// The multi-rate clock's tick. Band membership — and therefore which interventions fire this
    /// tick — is a function of it, so a resume that forgot it would apply forcings on the wrong
    /// ticks while still looking plausible.
    pub clock_tick: u64,
    pub laws: WorldLawSet,
    pub initial_conditions: InitialConditionSet,
    /// Provenance accumulated in-world: one record per intervention application, rooted at the
    /// intervention's own cause id.
    pub ledger: CausalLedger,
    /// How many intervention applications have fired. Part of the resume fingerprint: two runs that
    /// applied a different number of forcings did not do the same thing.
    pub applied_interventions: u64,
}

impl LiveExperimentState {
    /// A fresh state for a run at tick zero.
    pub fn new(
        seed: u64,
        laws: WorldLawSet,
        initial_conditions: InitialConditionSet,
        manifest_fingerprint: u64,
        registry_fingerprint: u64,
    ) -> Self {
        let law_fingerprint = laws.fingerprint();
        Self {
            schema_version: LIVE_EXPERIMENT_SCHEMA_VERSION,
            manifest_fingerprint,
            law_fingerprint,
            registry_fingerprint,
            seed,
            clock_tick: 0,
            laws,
            initial_conditions,
            ledger: CausalLedger::new(),
            applied_interventions: 0,
        }
    }

    /// Refuse a restore whose laws are not the ones this state was written under (ER01).
    pub fn check_resumable(&self) -> Result<(), ExperimentError> {
        if self.schema_version != LIVE_EXPERIMENT_SCHEMA_VERSION {
            return Err(ExperimentError::UnsupportedSchemaVersion {
                component: "live_experiment".into(),
                found: self.schema_version,
                supported: LIVE_EXPERIMENT_SCHEMA_VERSION,
            });
        }
        let recomputed = self.laws.fingerprint();
        if recomputed != self.law_fingerprint {
            return Err(ExperimentError::InvalidLaw {
                reason: format!(
                    "the saved law fingerprint {:#018x} does not match the laws stored beside it \
                     ({recomputed:#018x}); a world law is immutable within a run (ER01), so this \
                     snapshot cannot be resumed as the same experiment",
                    self.law_fingerprint
                ),
            });
        }
        Ok(())
    }
}

/// The interventions active on the current tick, handed to the world by the adapter.
///
/// Absent from an app run. `apply_live_interventions_system` returns on its first line without it,
/// which is what makes the addition to the live schedule a no-op.
#[derive(Resource, Default, Debug)]
pub struct LiveInterventions {
    /// The tick these are active on.
    pub tick: u64,
    /// The active set, in the queue's deterministic `(start_tick, id)` order.
    pub active: Vec<InterventionCommand>,
}

impl LiveInterventions {
    /// A buffer sized for a manifest's declared interventions, so the hot path never grows it.
    pub fn with_capacity(n: usize) -> Self {
        Self {
            tick: 0,
            active: Vec::with_capacity(n),
        }
    }
}

/// Per-tick forcing multipliers a declared intervention applies to systems that already exist.
///
/// A rainfall forcing does not get its own regrowth loop: it scales the seasonal fertility the real
/// `resource_field_regrowth_system` already applies. A temperature forcing does not get its own
/// metabolism: it shifts the homeostatic target `apply_environmental_effects_system` already sets.
/// Both systems read this through `Option<Res<..>>`, so a world without the resource — every app run
/// — behaves exactly as it did.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct LiveForcings {
    /// Multiplies seasonal fertility in `resource_field_regrowth_system`. `1.0` is no forcing.
    pub fertility_multiplier: f32,
    /// Added to the homeostatic temperature target in `apply_environmental_effects_system`.
    pub temp_target_shift_c: f32,
}

impl Default for LiveForcings {
    fn default() -> Self {
        Self {
            fertility_multiplier: 1.0,
            temp_target_shift_c: 0.0,
        }
    }
}

// ---- The intervention system --------------------------------------------------------------------

/// Apply the tick's declared interventions to the live world.
///
/// Every kind lands on machinery the engine already has, and every kind that moves energy moves it
/// **through** [`EnergyLedger`] rather than around it, so the closed-EU invariant survives a
/// forcing. Rate-band membership comes from the shared [`SimClock`], so a manifest's forcing fires
/// on exactly the ticks the reference path would fire it on.
///
/// | Kind | Mechanism | Acts | Recorded |
/// |---|---|---|---|
/// | `RainfallDelta` | scales seasonal fertility (a rate, not a stock) | every tick | ecology band |
/// | `TemperatureDelta` | shifts the homeostatic temperature target | every tick | ecology band |
/// | `Deforest` | moves standing crop out of the region's cells into detritus | ecology band | ecology band |
/// | `AddNutrient` | moves detritus into the region's cells as standing crop | ecology band | ecology band |
/// | `RemovePredators` | reclaims and despawns a deterministic prefix of predators | ecology band | ecology band |
///
/// The two rate forcings act on every tick and are *recorded* on the band: a ledger entry is a
/// provenance sample, and one per base tick would grow the ledger by sixty records a second for the
/// length of a run.
pub fn apply_live_interventions_system(
    mut commands: Commands,
    active: Option<Res<LiveInterventions>>,
    state: Option<ResMut<LiveExperimentState>>,
    forcings: Option<ResMut<LiveForcings>>,
    field: Option<ResMut<ResourceField>>,
    pool: Option<ResMut<EcosystemBiomass>>,
    energy: Option<ResMut<EnergyLedger>>,
    predators: Query<
        Entity,
        (
            With<crate::core::components::Agent>,
            With<crate::core::components::Predator>,
        ),
    >,
    mut cull_buf: Local<Vec<Entity>>,
) {
    let (Some(active), Some(mut state)) = (active, state) else {
        return;
    };
    let tick = active.tick;

    // The rate forcings are recomputed from a neutral baseline **every tick**, so a forcing whose
    // window closed last tick stops forcing on this one. They act every tick because the systems
    // that read them act every tick — regrowth and the homeostatic target are rates, not stocks.
    let mut fertility_multiplier = 1.0f32;
    let mut temp_target_shift_c = 0.0f32;
    for cmd in &active.active {
        match cmd.kind {
            InterventionKind::RainfallDelta => {
                fertility_multiplier *= (1.0 + cmd.signed_intensity_at(tick)).max(0.0);
            }
            InterventionKind::TemperatureDelta => {
                temp_target_shift_c +=
                    cmd.signed_intensity_at(tick) * TEMPERATURE_DELTA_FULL_SCALE_C;
            }
            _ => {}
        }
    }
    if let Some(mut forcings) = forcings {
        forcings.fertility_multiplier = fertility_multiplier;
        forcings.temp_target_shift_c = temp_target_shift_c;
    }

    // Everything below runs on the ecology band, the same band the reference world's trophic pools
    // update on. Two different reasons converge on it:
    //
    // - the **stock**-moving forcings would strip a 256² field in seconds if they fired every base
    //   tick, and would not correspond to anything the reference path does;
    // - the **rate** forcings are recorded here rather than above because a ledger entry is a
    //   provenance *sample*, and one per base tick would grow the ledger by 60 records a second for
    //   the length of a run — unbounded memory for a forcing whose value the reader can already see
    //   in `live.fertility_multiplier`. The forcing still acts every tick; only the record is
    //   sampled, at a declared cadence, and band membership is a function of the restored clock so a
    //   resumed run samples on exactly the same ticks.
    if !SimClock::fires(tick, ECOLOGY_PERIOD) {
        return;
    }
    if fertility_multiplier != 1.0 {
        if let Some(cmd) = active
            .active
            .iter()
            .find(|c| c.kind == InterventionKind::RainfallDelta)
        {
            state.ledger.record(
                cmd.cause_id,
                None,
                tick,
                "live.fertility_multiplier",
                fertility_multiplier as f64,
                (fertility_multiplier - 1.0) as f64,
                "rainfall intervention scales seasonal fertility",
            );
            state.applied_interventions += 1;
        }
    }
    if temp_target_shift_c != 0.0 {
        if let Some(cmd) = active
            .active
            .iter()
            .find(|c| c.kind == InterventionKind::TemperatureDelta)
        {
            state.ledger.record(
                cmd.cause_id,
                None,
                tick,
                "live.temp_target_shift_c",
                temp_target_shift_c as f64,
                temp_target_shift_c as f64,
                "temperature intervention shifts the homeostatic target",
            );
            state.applied_interventions += 1;
        }
    }

    let mut field = field;
    let mut pool = pool;
    let mut energy = energy;

    for cmd in &active.active {
        match cmd.kind {
            InterventionKind::Deforest => {
                let (Some(field), Some(pool), Some(energy)) =
                    (field.as_mut(), pool.as_mut(), energy.as_mut())
                else {
                    continue;
                };
                let fraction = cmd.signed_intensity_at(tick).abs().clamp(0.0, 1.0) as f64;
                let removed = strip_region(field, cmd, fraction);
                if removed > 0.0 {
                    // Cleared biomass becomes detritus rather than vanishing: a declared forcing is
                    // still bound by the closed-EU law it runs under.
                    let moved = energy.transfer(
                        pool,
                        Compartment::Plants,
                        Compartment::Detritus,
                        removed,
                        EnergyEvent::Intervention,
                    );
                    state.ledger.record(
                        cmd.cause_id,
                        None,
                        tick,
                        "plants",
                        pool.plants,
                        -moved,
                        "deforestation moves standing crop into detritus",
                    );
                    state.applied_interventions += 1;
                }
            }
            InterventionKind::AddNutrient => {
                let (Some(field), Some(pool), Some(energy)) =
                    (field.as_mut(), pool.as_mut(), energy.as_mut())
                else {
                    continue;
                };
                let fraction = cmd.signed_intensity_at(tick).abs().clamp(0.0, 1.0) as f64;
                let budget = (pool.detritus.max(0.0) * fraction * NUTRIENT_FULL_SCALE_FRACTION)
                    .min(pool.detritus.max(0.0));
                let added = fertilise_region(field, cmd, budget);
                if added > 0.0 {
                    let moved = energy.transfer(
                        pool,
                        Compartment::Detritus,
                        Compartment::Plants,
                        added,
                        EnergyEvent::Intervention,
                    );
                    state.ledger.record(
                        cmd.cause_id,
                        None,
                        tick,
                        "plants",
                        pool.plants,
                        moved,
                        "added nutrient moves detritus into standing crop",
                    );
                    state.applied_interventions += 1;
                }
            }
            InterventionKind::RemovePredators => {
                let fraction = cmd.signed_intensity_at(tick).abs().clamp(0.0, 1.0) as f64;
                cull_buf.clear();
                cull_buf.extend(predators.iter());
                // Archetype iteration order is not a declared property of the world, so the cull
                // set is sorted by entity index before it is truncated. Without this a run and its
                // replay could remove different predators while removing the same *number* of them
                // — a difference that shows up only much later, as divergence with no obvious
                // cause.
                cull_buf.sort_unstable_by_key(|e| (e.index(), e.generation()));
                let n = ((cull_buf.len() as f64) * fraction).floor() as usize;
                if n == 0 {
                    continue;
                }
                for entity in cull_buf.iter().take(n) {
                    commands.add(crate::core::agent_systems::ReclaimAndDespawnAgentCommand {
                        root: *entity,
                    });
                }
                state.ledger.record(
                    cmd.cause_id,
                    None,
                    tick,
                    "live.predator_count",
                    (cull_buf.len() - n) as f64,
                    -(n as f64),
                    "predator removal intervention reclaims and despawns a deterministic prefix",
                );
                state.applied_interventions += 1;
            }
            InterventionKind::RainfallDelta | InterventionKind::TemperatureDelta => {}
        }
    }
}

/// Remove `fraction` of the standing crop from every cell the command covers, returning the exact
/// `f64` amount removed (measured from the stored `f32`s, so the ledger debit matches the field).
fn strip_region(field: &mut ResourceField, cmd: &InterventionCommand, fraction: f64) -> f64 {
    if fraction <= 0.0 {
        return 0.0;
    }
    let mut removed = 0.0f64;
    for y in 0..field.height {
        for x in 0..field.width {
            if !cmd.affects(x as u32, y as u32) {
                continue;
            }
            let i = y * field.width + x;
            let before = field.r[i] as f64;
            let after = (field.r[i] as f64 * (1.0 - fraction)).max(0.0) as f32;
            field.r[i] = after;
            removed += before - after as f64;
        }
    }
    removed.max(0.0)
}

/// Push up to `budget` EU of standing crop into the cells the command covers, never past capacity.
/// Returns the exact amount actually added.
fn fertilise_region(field: &mut ResourceField, cmd: &InterventionCommand, budget: f64) -> f64 {
    if budget <= 0.0 {
        return 0.0;
    }
    // Two passes: count the covered cells, then spread the budget evenly over them. Even spreading
    // keeps the result independent of iteration order, which a "fill the first cells first" split
    // would not be.
    let mut covered = 0usize;
    for y in 0..field.height {
        for x in 0..field.width {
            if cmd.affects(x as u32, y as u32) {
                covered += 1;
            }
        }
    }
    if covered == 0 {
        return 0.0;
    }
    let per_cell = budget / covered as f64;
    let mut added = 0.0f64;
    for y in 0..field.height {
        for x in 0..field.width {
            if !cmd.affects(x as u32, y as u32) {
                continue;
            }
            let i = y * field.width + x;
            let before = field.r[i] as f64;
            let after = (before + per_cell).min(field.r_max[i] as f64) as f32;
            field.r[i] = after;
            added += after as f64 - before;
        }
    }
    added.max(0.0)
}

// ---- In-tick inference --------------------------------------------------------------------------

/// The experiment adapter's end of the inference channel, so a tick can answer its own requests.
///
/// # Why the round trip happens inside the tick
///
/// The app hands a tick's inference requests to a worker thread and consumes the answers on some
/// *later* tick, whenever the thread happens to deliver them. That latency is real, and it is also
/// the reason a live run could not be checkpointed: at any tick boundary the pipeline holds a batch
/// of requests and a batch of answers, both keyed by `Entity` — and entity ids are not stable across
/// a restore, so there is no honest way to write that in-flight work into a snapshot. A resumed run
/// therefore lost one round of thought and diverged, quietly, from the run it was continuing.
///
/// A declared experiment resolves inference **within the tick that asked for it**, which makes a
/// tick atomic: at every boundary the pipeline is empty and the whole trajectory lives in the world.
/// That is a real difference from the app's timing, and it is the right one — the app's latency is a
/// function of thread scheduling, so it could not be part of a reproducible manifest even in
/// principle.
///
/// Absent from an app run, where the worker thread does this instead.
#[derive(Resource)]
pub struct LiveInferencePump {
    req_rx: crossbeam_channel::Receiver<crate::core::agent_systems::InferenceRequestBatch>,
    res_tx: crossbeam_channel::Sender<crate::core::agent_systems::InferenceResponseBatch>,
    recycle_req_tx: crossbeam_channel::Sender<crate::core::agent_systems::InferenceRequestBatch>,
    recycle_res_rx: crossbeam_channel::Receiver<crate::core::agent_systems::InferenceResponseBatch>,
    /// Allocated once and reused, for the same reason the worker thread's copy is.
    scratch: crate::ai::model::InferenceScratch,
    /// Monotonic evidence that the real schedule exercised the pump, exposed only inside the world.
    completed_batches: u64,
}

/// Answer the tick's inference requests, in the tick.
///
/// Runs the same [`crate::ai::model::run_inference_batch`] the worker thread runs, against the
/// world's own shared [`crate::ai::model::BrainModel`]. Inert — one `Option` check — without the
/// pump resource, which only [`LiveExperimentAdapter`] inserts.
pub fn live_inference_pump_system(
    pump: Option<ResMut<LiveInferencePump>>,
    brain: Option<Res<crate::ai::model::BrainModel>>,
) {
    let (Some(mut pump), Some(brain)) = (pump, brain) else {
        return;
    };
    let pump = &mut *pump;
    while let Ok(mut req_batch) = pump.req_rx.try_recv() {
        if !req_batch.requests.is_empty() {
            // A live experiment is single-threaded and explicitly scheduled
            // sensory -> pump -> action resolution. Sensory publishes at most one batch per tick,
            // and action resolution returns that tick's response batch before the boundary, so the
            // preloaded pool cannot be empty in a valid run. Allocating here would hide a broken
            // atomic-tick invariant and permanently raise the recycled pool's memory ceiling.
            let mut res_batch = match pump.recycle_res_rx.try_recv() {
                Ok(batch) => batch,
                Err(crossbeam_channel::TryRecvError::Empty) => panic!(
                    "live inference response pool invariant violated: every response buffer is \
                     outstanding while {} requests need one; a deterministic experiment must not \
                     allocate an undeclared response batch",
                    req_batch.requests.len()
                ),
                Err(crossbeam_channel::TryRecvError::Disconnected) => panic!(
                    "live inference response pool disconnected with {} requests still queued; \
                     the deterministic inference pipeline was torn down during a tick",
                    req_batch.requests.len()
                ),
            };
            res_batch.responses.clear();
            crate::ai::model::run_inference_batch(
                &brain,
                &req_batch.requests,
                &mut res_batch.responses,
                &mut pump.scratch,
            );
            pump.res_tx
                .send(res_batch)
                .expect("live inference response channel disconnected during a tick");
            pump.completed_batches = pump.completed_batches.saturating_add(1);
        }
        req_batch.requests.clear();
        pump.recycle_req_tx
            .send(req_batch)
            .expect("live inference request recycle channel disconnected during a tick");
    }
}

// ---- Observables ---------------------------------------------------------------------------------

/// Every observable id the live world emits, in emission order.
pub const LIVE_OBSERVABLE_IDS: [&str; 11] = [
    "plants",
    "detritus",
    "live.animals_eu",
    "live.closed_eu_total",
    "live.agent_count",
    "live.herbivore_count",
    "live.predator_count",
    "live.food_items",
    "live.standing_crop",
    "live.mean_agent_energy",
    "live.season_phase",
];

/// Read the live world's observables, in the stable order of [`LIVE_OBSERVABLE_IDS`].
pub fn live_observables(world: &mut World) -> Vec<(String, f64)> {
    use crate::core::components::{Agent, Food, Predator, Prey};

    let (plants, animals, detritus) = world
        .get_resource::<EcosystemBiomass>()
        .map(|p| (p.plants, p.animals, p.detritus))
        .unwrap_or((0.0, 0.0, 0.0));
    let standing_crop = world
        .get_resource::<ResourceField>()
        .map(|f| f.total_biomass())
        .unwrap_or(0.0);
    let season_phase = world
        .get_resource::<crate::core::ecology::SeasonClock>()
        .map(|c| c.phase as f64)
        .unwrap_or(0.0);

    let mut agents = 0u64;
    let mut energy_sum = 0.0f64;
    {
        let mut q = world.query_filtered::<&crate::ai::hrrl::HomeostaticState, With<Agent>>();
        for homeo in q.iter(world) {
            agents += 1;
            energy_sum += homeo.energy as f64;
        }
    }
    let herbivores = {
        let mut q = world.query_filtered::<(), (With<Agent>, With<Prey>)>();
        q.iter(world).count() as f64
    };
    let predators = {
        let mut q = world.query_filtered::<(), (With<Agent>, With<Predator>)>();
        q.iter(world).count() as f64
    };
    let food_items = {
        let mut q = world.query_filtered::<(), With<Food>>();
        q.iter(world).count() as f64
    };

    vec![
        ("plants".to_string(), plants),
        ("detritus".to_string(), detritus),
        ("live.animals_eu".to_string(), animals),
        (
            "live.closed_eu_total".to_string(),
            crate::core::energy_ledger::closed_total_eu(plants, animals, detritus),
        ),
        ("live.agent_count".to_string(), agents as f64),
        ("live.herbivore_count".to_string(), herbivores),
        ("live.predator_count".to_string(), predators),
        ("live.food_items".to_string(), food_items),
        ("live.standing_crop".to_string(), standing_crop),
        (
            "live.mean_agent_energy".to_string(),
            if agents == 0 {
                0.0
            } else {
                energy_sum / agents as f64
            },
        ),
        ("live.season_phase".to_string(), season_phase),
    ]
}

// ---- The adapter ----------------------------------------------------------------------------------

/// Everything the live world's systems need that the app supplies from a thread.
///
/// Held so the far end of each channel stays alive: a `Receiver` whose `Sender` has been dropped
/// starts returning `Disconnected`, which several systems treat as "nothing to do" — so dropping
/// these would silently change what the schedule does.
struct LiveChannels {
    transitions: crossbeam_channel::Receiver<crate::ai::hrrl::Transition>,
    epoch_stats: crossbeam_channel::Receiver<Vec<crate::core::resources::AgentEpochStats>>,
    // Held only so the receivers the world owns never see a disconnected channel.
    _spawn_tx: crossbeam_channel::Sender<crate::core::resources::EvolutionSpawn>,
    _env_tx: crossbeam_channel::Sender<crate::evolution::meta_ai::EnvironmentalEvent>,
    _inbound_tx: crossbeam_channel::Sender<crate::core::components::AgentMigrationData>,
    _outbound_rx: crossbeam_channel::Receiver<crate::core::components::OutboundMigration>,
    _migration_tx: crossbeam_channel::Sender<u16>,
}

/// The live Bevy world, driven by the experiment runner.
pub struct LiveExperimentAdapter {
    world: RefCell<World>,
    schedule: Schedule,
    channels: LiveChannels,
    clock: SimClock,
    config: LiveWorldConfig,
    seed: u64,
    /// How many effects of the world's own ledger have already been mirrored into the runner's.
    mirrored_effects: usize,
}

/// A `World` and a `Schedule` are not `Debug`, and the interesting part of an adapter is not their
/// contents anyway: it is which run this is and how far into it we are.
impl std::fmt::Debug for LiveExperimentAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveExperimentAdapter")
            .field("seed", &self.seed)
            .field("tick", &self.clock.tick)
            .field("config", &self.config)
            .field("mirrored_effects", &self.mirrored_effects)
            .finish_non_exhaustive()
    }
}

/// An in-memory checkpoint of a live run, sufficient to resume it.
#[derive(Clone, Debug)]
pub struct LiveSnapshot {
    /// The world, through the same serializer the save command uses.
    pub saved: SavedSimulationState,
    pub config: LiveWorldConfig,
    pub seed: u64,
}

impl LiveExperimentAdapter {
    /// Build the world a manifest describes, and the schedule the app runs.
    fn build(
        laws: &WorldLawSet,
        initial: &InitialConditionSet,
        seed: u64,
        restore: Option<&SavedSimulationState>,
        manifest_fingerprint: u64,
        registry_fingerprint: u64,
    ) -> Result<Self, ExperimentError> {
        if laws.exotic_energy.is_some() {
            return Err(ExperimentError::InvalidLaw {
                reason: "the live Bevy world has no exotic-energy field: nothing in its schedule \
                         sources, diffuses or spends MU. A manifest declaring \
                         laws.exotic_energy must run against the reference world \
                         (core::reference_world) — running it here would report a baseline as \
                         though it were a treatment."
                    .into(),
            });
        }
        laws.validate()?;
        let config = LiveWorldConfig::from_initial_conditions(initial)?;

        let mut world = build_live_world(seed, &config, restore)?;
        let channels = install_runtime_resources(&mut world, seed, &config);

        let state = match restore.and_then(|s| s.experiment.clone()) {
            Some(saved) => {
                saved.check_resumable()?;
                if saved.law_fingerprint != laws.fingerprint() {
                    return Err(ExperimentError::InvalidLaw {
                        reason: format!(
                            "this snapshot was written under law fingerprint {:#018x} and is being \
                             resumed under {:#018x}; a world law is immutable within a run (ER01)",
                            saved.law_fingerprint,
                            laws.fingerprint()
                        ),
                    });
                }
                saved
            }
            None => LiveExperimentState::new(
                seed,
                laws.clone(),
                initial.clone(),
                manifest_fingerprint,
                registry_fingerprint,
            ),
        };
        let clock = SimClock {
            tick: state.clock_tick,
        };
        let mirrored_effects = state.ledger.len();
        world.insert_resource(state);
        world.insert_resource(LiveForcings::default());
        world.insert_resource(LiveInterventions::with_capacity(16));

        // Deterministic by construction, not by environment: an experiment whose trajectory
        // depends on a shell variable is not the experiment the manifest describes.
        let deterministic = crate::core::determinism::DeterministicMode::on();
        world.insert_resource(deterministic);
        let schedule = crate::core::simulation_schedule::build_tick_schedule(deterministic);

        Ok(Self {
            world: RefCell::new(world),
            schedule,
            channels,
            clock,
            config,
            seed,
            mirrored_effects,
        })
    }

    /// The genesis config this run was built with.
    pub fn config(&self) -> LiveWorldConfig {
        self.config
    }

    /// The run's seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The multi-rate clock's current tick.
    pub fn tick(&self) -> u64 {
        self.clock.tick
    }

    /// Borrow the world, for tests and for the capture harness.
    pub fn world(&self) -> std::cell::RefMut<'_, World> {
        self.world.borrow_mut()
    }

    /// Run the schedule once without the experiment runner, for a harness that only wants ticks.
    pub fn run_schedule_once(&mut self) {
        self.clock.advance();
        self.run_tick();
        self.drain_absent_thread_queues();
    }

    /// Attach a tick capture to this run.
    ///
    /// Measurement only: the sink is not read by any system, is not part of
    /// [`checksum`](ExperimentModel::checksum), and is not serialized into a snapshot. Attaching one
    /// must leave the trajectory identical, which is what
    /// `capture_does_not_change_the_live_trajectory` checks rather than assumes.
    pub fn install_tick_capture(&mut self, shared: crate::core::tick_capture::SharedTickCapture) {
        shared.set_executor(crate::core::simulation_schedule::EXECUTOR_SINGLE_THREADED);
        let world = self.world.get_mut();
        world.insert_resource(crate::core::tick_capture::TickCaptureSink::new(shared));
    }

    /// Run the schedule once, bracketing it the way the simulation thread does so a capture sees
    /// the same phases from a headless run as it does from the app.
    fn run_tick(&mut self) {
        use crate::core::tick_capture::TickCaptureSink;
        let tick = self.clock.tick;
        let world = self.world.get_mut();
        let capturing = world
            .get_resource_mut::<TickCaptureSink>()
            .map(|mut sink| sink.begin_tick())
            .unwrap_or(false);
        let started = std::time::Instant::now();
        self.schedule.run(world);
        let schedule_ns = started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        if capturing {
            if let Some(mut sink) = world.get_resource_mut::<TickCaptureSink>() {
                // No telemetry publish in a headless run: there is no IPC to publish to, and
                // reporting a number for a phase that did not happen is the failure this module
                // exists to avoid. Zero here means "this phase did not run", and the export's
                // `executor` plus the run's own provenance say why.
                sink.commit(tick, schedule_ns, 0);
            }
        }
    }

    /// The schedule this adapter runs — the app's, from
    /// [`crate::core::simulation_schedule::build_tick_schedule`].
    pub fn schedule_mut(&mut self) -> &mut Schedule {
        &mut self.schedule
    }

    /// Empty the queues whose consumers the app runs on a thread and an experiment does not.
    ///
    /// The learner and the evolution thread are deliberately absent (see the module docs). Their
    /// channels still exist, because the systems that feed them are part of the live schedule and
    /// removing them would be a different schedule — so what a run does not consume, it drains,
    /// rather than letting an unbounded queue grow for the length of the run.
    fn drain_absent_thread_queues(&mut self) {
        while self.channels.transitions.try_recv().is_ok() {}
        while self.channels.epoch_stats.try_recv().is_ok() {}
    }

    /// Copy the effects the world recorded this tick into the runner's ledger.
    ///
    /// Only the *direct* effects of a forcing are recorded. The live world's downstream response —
    /// fewer herbivores three hundred ticks after a deforestation — emerges from real systems that
    /// know nothing about the intervention, and inventing a parent link for it would be attribution
    /// this engine has not earned.
    fn mirror_ledger(&mut self, ledger: &mut CausalLedger) {
        let world = self.world.get_mut();
        let Some(state) = world.get_resource::<LiveExperimentState>() else {
            return;
        };
        let effects = state.ledger.all();
        for rec in effects.iter().skip(self.mirrored_effects) {
            ledger.record(
                rec.cause_id,
                None,
                rec.tick,
                rec.target.clone(),
                rec.quantity,
                rec.delta,
                rec.mechanism.clone(),
            );
        }
        self.mirrored_effects = effects.len();
    }
}

impl ExperimentModel for LiveExperimentAdapter {
    type Snapshot = LiveSnapshot;

    fn model_version() -> &'static str {
        LIVE_MODEL_VERSION
    }

    fn from_manifest(
        laws: &WorldLawSet,
        initial: &InitialConditionSet,
        forcings: &[ExoticIntervention],
        seed: u64,
        _grid: (usize, usize),
        _run_ticks: u64,
    ) -> Result<Self, ExperimentError> {
        if let Some(first) = forcings.first() {
            return Err(ExperimentError::InvalidExoticIntervention {
                id: first.id,
                reason: "the live Bevy world has no exotic-energy field to force".into(),
            });
        }
        // The grid is the reference world's demonstration substrate. The live world's field is the
        // terrain's own, at `MapSettings::default()` — 256×256 in this build — and is not resized
        // to match a manifest, because the resource field is the world's, not the experiment's.
        Self::build(laws, initial, seed, None, 0, 0)
    }

    fn snapshot(&self) -> Self::Snapshot {
        let mut world = self.world.borrow_mut();
        // The clock lives on the adapter and in the resource; the resource is what the serializer
        // reads, so it is refreshed here rather than only at the end of `step`.
        if let Some(mut state) = world.get_resource_mut::<LiveExperimentState>() {
            state.clock_tick = self.clock.tick;
        }
        let deps = SaveHandles::new();
        let saved = crate::core::simulation_state::serialize_world_state(
            &mut world,
            self.clock.tick,
            &deps.chronicle,
            &deps.lineage,
            &deps.evolution,
            &deps.grid,
        );
        LiveSnapshot {
            saved,
            config: self.config,
            seed: self.seed,
        }
    }

    fn from_snapshot(snapshot: &Self::Snapshot) -> Result<Self, ExperimentError> {
        let Some(state) = snapshot.saved.experiment.clone() else {
            return Err(ExperimentError::InvalidLaw {
                reason: "this snapshot carries no live experiment state, so the laws and clock it \
                         ran under are unknown; it cannot be resumed as an experiment"
                    .into(),
            });
        };
        state.check_resumable()?;
        Self::build(
            &state.laws.clone(),
            &state.initial_conditions.clone(),
            state.seed,
            Some(&snapshot.saved),
            state.manifest_fingerprint,
            state.registry_fingerprint,
        )
    }

    fn step(
        &mut self,
        clock: &SimClock,
        active: &[&InterventionCommand],
        ledger: &mut CausalLedger,
        _rng: &mut StdRng,
    ) {
        // The runner's clock is authoritative; the adapter's copy exists so `snapshot` and
        // `from_snapshot` can carry it across a checkpoint.
        self.clock = *clock;
        {
            let world = self.world.get_mut();
            if let Some(mut queued) = world.get_resource_mut::<LiveInterventions>() {
                queued.tick = clock.tick;
                queued.active.clear();
                queued.active.extend(active.iter().map(|c| (*c).clone()));
            }
            if let Some(mut state) = world.get_resource_mut::<LiveExperimentState>() {
                state.clock_tick = clock.tick;
            }
        }
        self.run_tick();
        self.drain_absent_thread_queues();
        self.mirror_ledger(ledger);
    }

    fn checksum(&self) -> u32 {
        let mut world = self.world.borrow_mut();
        crate::core::snapshot::world_checksum(&mut world)
    }

    fn observables(&self) -> Vec<(String, f64)> {
        let mut world = self.world.borrow_mut();
        live_observables(&mut world)
    }
}

/// The four shared handles `serialize_world_state` takes.
///
/// The adapter has no app to borrow them from, so it owns a private set. The lineage tracker falls
/// back to in-memory when Neo4j is unreachable, which is what a headless run always is: the address
/// below is a port nothing listens on, chosen so the fallback is taken immediately rather than after
/// a connection timeout.
struct SaveHandles {
    chronicle:
        std::sync::Arc<std::sync::RwLock<Vec<crate::core::simulation_state::ChronicleEvent>>>,
    lineage: std::sync::Arc<crate::evolution::lineage::FallbackLineageTracker>,
    evolution: std::sync::Arc<std::sync::Mutex<crate::commands::EvolutionSettings>>,
    grid: std::sync::Arc<std::sync::Mutex<crate::commands::MapElitesGridState>>,
}

impl SaveHandles {
    fn new() -> Self {
        Self {
            chronicle: std::sync::Arc::new(std::sync::RwLock::new(Vec::new())),
            lineage: std::sync::Arc::new(crate::evolution::lineage::FallbackLineageTracker::new(
                "bolt://127.0.0.1:1",
                "neo4j",
                "password",
            )),
            evolution: std::sync::Arc::new(std::sync::Mutex::new(
                crate::commands::EvolutionSettings {
                    mutation_rate: 0.15,
                    selection_bias: 1.5,
                    grid_resolution: 50,
                },
            )),
            grid: std::sync::Arc::new(std::sync::Mutex::new(crate::commands::MapElitesGridState {
                grid: std::collections::HashMap::new(),
                grid_resolution: 50,
            })),
        }
    }
}

/// Build the world the schedule runs on: the app's `init_world`, with everything that reads the
/// process environment replaced by what the manifest declared.
fn build_live_world(
    seed: u64,
    config: &LiveWorldConfig,
    restore: Option<&SavedSimulationState>,
) -> Result<World, ExperimentError> {
    let mut world = crate::core::ecs::init_world();

    // `init_world` resolves these from the environment, which is exactly what a manifest cannot
    // tolerate: `ANIMA_SIM_SEED`, `ANIMA_EVOLVED_BRAINS` and `ANIMA_LIFETIME_LEARNING` would each
    // silently change the trajectory of a run whose whole point is that its inputs are declared.
    //
    // `evolved` comes from the manifest rather than the environment, and the other two fields stay
    // at their defaults on purpose: `lifetime_learning` and `brain_metabolic_cost` are separate
    // flags answering separate questions, and folding them in here would turn one declared factor
    // into a bundle no result could attribute.
    world.insert_resource(crate::core::resources::SimRng::from_seed(seed));
    world.insert_resource(crate::core::resources::BrainPolicy {
        evolved: config.evolved_brains,
        ..Default::default()
    });

    let bounds = restore.map(|s| s.map_bounds).unwrap_or_default();
    world.insert_resource(bounds);
    world.insert_resource(crate::physics::SpatialHashGrid::new_prepopulated(
        10.0, &bounds,
    ));

    match restore {
        Some(state) => {
            let pheromone = crate::ai::pheromone::PheromoneGrid {
                values: state.pheromone_grid.values.clone(),
                scratch: vec![0.0; crate::ai::pheromone::CELL_COUNT],
                diffusion_rate: state.pheromone_grid.diffusion_rate,
                decay_rate: state.pheromone_grid.decay_rate,
            };
            world.insert_resource(pheromone);
            world.insert_resource(state.food_spawn_settings);
            world.insert_resource(state.epoch_manager);
            world.insert_resource(crate::core::ecs::ActiveEnvironmentEvent(
                state.active_environment_event,
            ));
            for agent in &state.agents {
                crate::core::simulation_state::spawn_serialized_agent(&mut world, agent).map_err(
                    |error| ExperimentError::InvalidPopulation {
                        reason: format!(
                            "snapshot agent {} could not be restored: {error}",
                            agent.lineage_id
                        ),
                    },
                )?;
            }
            crate::core::simulation_state::restore_energy_state(&mut world, state);
            for food in &state.foods {
                world.spawn((
                    crate::core::ecs::Food {
                        energy_value: food.energy_value,
                        hydration_value: food.hydration_value,
                    },
                    crate::core::ecs::Position(food.position),
                    crate::physics::SpatialCollider { radius: 0.5 },
                ));
            }
            for lake in &state.lakes {
                world.spawn((
                    crate::core::ecs::Lake {
                        current_water: lake.current_water,
                        max_water: lake.max_water,
                        replenishment_rate: lake.replenishment_rate,
                    },
                    crate::core::ecs::Position(lake.position),
                    crate::physics::SpatialCollider {
                        radius: lake.radius,
                    },
                ));
            }
            for tree in &state.trees {
                world.spawn((
                    crate::core::ecs::Tree {
                        current_fruit: tree.current_fruit,
                        max_fruit: tree.max_fruit,
                        fruit_growth_rate: tree.fruit_growth_rate,
                        time_since_last_drop: tree.time_since_last_drop,
                        seed_drop_cooldown: tree.seed_drop_cooldown,
                        seed_spread_radius: tree.seed_spread_radius,
                    },
                    crate::core::ecs::Position(tree.position),
                    crate::physics::SpatialCollider {
                        radius: tree.radius,
                    },
                ));
            }
        }
        None => {
            world.insert_resource(crate::ai::pheromone::PheromoneGrid::default());
            world.insert_resource(crate::core::ecs::FoodSpawnSettings {
                max_food_count: config.food_cap,
                ..Default::default()
            });
            world.insert_resource(crate::core::ecs::EpochManager {
                ticks_per_epoch: 1000,
                current_epoch_ticks: 0,
                current_epoch: 0,
            });
            world.insert_resource(crate::core::ecs::ActiveEnvironmentEvent::default());
            genesis(&mut world, config, bounds, seed);
        }
    }

    Ok(world)
}

/// The founding population and environment a manifest declared.
///
/// Placement is a deterministic lattice rather than the app's biome-weighted shuffle. That is a real
/// difference and it is the right one: the app picks lake and tree sites by shuffling every
/// candidate cell of the terrain, so the founding layout depends on the terrain artifact present on
/// the machine. An experiment's initial conditions have to come from its manifest.
///
/// # Brains come from a stream of their own
///
/// Genesis creates individuals, so it develops brains (ADR-0003 invariant D01) — the app's genesis
/// in [`crate::core::simulation_loop`] does the same. It differs in **which** stream the weights
/// come from, and that difference is the identifying assumption of experiment E2 rather than a
/// detail: the app draws from `SimRng`, so turning brains on there consumes roughly
/// `founders × 5,769` f32 out of the ecology stream **before the first tick** and displaces every
/// later draw — every food position, every seed drop. Two arms compared that way would differ in the
/// brain *and* in the realised random sequence, inseparably, at every seed.
///
/// [`derived_rng`](crate::core::resources::derived_rng) gives the brains an independent stream, so
/// the ecology draw order is bit-identical whether or not the arm has brains and the only declared
/// difference is the one the manifest declared. The cost, stated because it is real: this is not
/// exactly what flipping the app's default would do. That shift carries no directional information —
/// it is a reshuffle, not a treatment — but it does mean a comparison made here holds the stochastic
/// trajectory fixed, which is the cleaner question and not quite the same one.
fn genesis(
    world: &mut World,
    config: &LiveWorldConfig,
    bounds: crate::core::ecs::MapBounds,
    seed: u64,
) {
    use crate::core::agent_systems::{
        AgentEvaluation, AgentGeneration, AgentGenotype, AgentLineageId,
    };
    use crate::core::components::{AgentParentLineageIds, FeatureTracker, Predator, Prey};
    use crate::evolution::genotype::{
        decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
    };

    let mut genotype = MorphologyGenotype::new();
    for (id, mass) in [(0u32, 1.5f32), (1, 1.0), (2, 0.8)] {
        genotype.add_node(MorphologyNode {
            id,
            length: 1.0,
            radius: 0.2,
            mass,
        });
    }
    for (source_node, target_node) in [(0u32, 1u32), (1, 2)] {
        genotype.add_edge(MorphologyEdge {
            source_node,
            target_node,
            joint_anchor: glam::Vec3::new(1.0, 0.0, 0.0),
            joint_axis: glam::Vec3::new(0.0, 0.0, 1.0),
        });
    }

    // Read once: the policy is the same resource the runtime systems read, so "the policy says
    // evolved" and "the founders have brains" cannot disagree.
    let policy = world
        .get_resource::<crate::core::resources::BrainPolicy>()
        .copied()
        .unwrap_or_default();
    let mut brain_rng = crate::core::resources::derived_rng(
        seed,
        crate::core::resources::sim_stream::LIVE_GENESIS_BRAINS,
    );

    let predators = config.predators();
    for i in 0..config.founders {
        let pos = lattice_point(i, config.founders.max(1), bounds, 0.35);
        let entity = decode_genotype(world, &genotype, pos, glam::Quat::IDENTITY);
        if let Some(brain) = policy.new_brain(&mut brain_rng) {
            world.entity_mut(entity).insert(brain);
        }
        world.entity_mut(entity).insert((
            AgentGenotype(genotype.clone()),
            AgentEvaluation {
                start_position: pos,
                total_distance: 0.0,
                total_energy_expended: 0.0,
                survival_ticks: 0,
                last_position: pos,
            },
            FeatureTracker::default(),
            AgentLineageId(format!("live-founder-{i:04}")),
            AgentGeneration(0),
            AgentParentLineageIds(Vec::new()),
        ));
        if i < predators {
            world.entity_mut(entity).insert(Predator);
        } else {
            world.entity_mut(entity).insert(Prey);
        }
    }

    let env = crate::core::ecs::EnvironmentalSpawnSettings::default();
    for i in 0..config.trees {
        let pos = lattice_point(i, config.trees.max(1), bounds, 0.6);
        world.spawn((
            crate::core::ecs::Tree {
                current_fruit: env.default_tree_fruit,
                max_fruit: env.default_tree_fruit,
                fruit_growth_rate: env.default_tree_growth,
                time_since_last_drop: 0.0,
                // Seed dropping draws from `SimRng`; a manifest that wants a fixed tree count gets
                // one by pushing the first drop past any plausible run rather than by switching the
                // system off, which would be a different schedule.
                seed_drop_cooldown: env.default_seed_cooldown,
                seed_spread_radius: env.default_seed_spread,
            },
            crate::core::ecs::Position(pos),
            crate::physics::SpatialCollider { radius: 2.0 },
        ));
    }
    for i in 0..config.lakes {
        let pos = lattice_point(i, config.lakes.max(1), bounds, 0.8);
        world.spawn((
            crate::core::ecs::Lake {
                current_water: env.default_lake_water,
                max_water: env.default_lake_water,
                replenishment_rate: env.default_lake_replenish,
            },
            crate::core::ecs::Position(pos),
            crate::physics::SpatialCollider { radius: 10.0 },
        ));
    }
}

/// The `i`-th of `n` points evenly spread along X at a fixed fraction of the Z span.
fn lattice_point(
    i: usize,
    n: usize,
    bounds: crate::core::ecs::MapBounds,
    z_fraction: f32,
) -> glam::Vec3 {
    let span_x = bounds.max.x - bounds.min.x;
    let span_z = bounds.max.z - bounds.min.z;
    let t = (i as f32 + 1.0) / (n as f32 + 1.0);
    glam::Vec3::new(
        bounds.min.x + t * span_x,
        0.0,
        bounds.min.z + z_fraction * span_z,
    )
}

/// Insert every resource the live schedule's systems require but `init_world` does not build,
/// creating the channels the app's threads would otherwise own.
fn install_runtime_resources(
    world: &mut World,
    seed: u64,
    config: &LiveWorldConfig,
) -> LiveChannels {
    use crate::core::agent_systems::{
        ActiveEvolutionSettings, BevyEvolutionRunning, BevyEvolutionSettings, BevyMapElitesArchive,
        BevyMapElitesGrid, EnvironmentalEventReceiver, InferenceChannels, InferenceRequestBatch,
        InferenceResponseBatch, NextNodeId,
    };
    use crate::core::resources::{EvolutionQueue, EvolutionReceiver, EvolutionSender};

    let (req_tx, req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
    let (recycle_req_tx, recycle_req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
    let (res_tx, res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();
    let (recycle_res_tx, recycle_res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();
    for _ in 0..16 {
        let _ = recycle_req_tx.send(InferenceRequestBatch {
            requests: Vec::with_capacity(128),
        });
        let _ = recycle_res_tx.send(InferenceResponseBatch {
            responses: Vec::with_capacity(128),
        });
    }
    world.insert_resource(InferenceChannels {
        req_tx,
        recycle_req_rx,
        res_rx,
        recycle_res_tx,
    });
    // The far end of the same channel, driven inside the schedule so a tick answers its own
    // requests — see `LiveInferencePump`.
    world.insert_resource(LiveInferencePump {
        req_rx,
        res_tx,
        recycle_req_tx,
        recycle_res_rx,
        scratch: crate::ai::model::InferenceScratch::with_capacity(256),
        completed_batches: 0,
    });

    // Unbounded, and drained every tick by `pump_inference`. The app uses a bounded channel with a
    // learner thread on the far end; here there is no learner, and `hrrl_learning_system` calls a
    // blocking `send`, so a bounded queue would deadlock the run the moment it filled.
    let (trans_tx, trans_rx) = crossbeam_channel::unbounded::<crate::ai::hrrl::Transition>();
    world.insert_resource(crate::ai::hrrl::TransitionSender(trans_tx));

    let (stats_tx, stats_rx) =
        crossbeam_channel::unbounded::<Vec<crate::core::resources::AgentEpochStats>>();
    let (spawn_tx, spawn_rx) =
        crossbeam_channel::unbounded::<crate::core::resources::EvolutionSpawn>();
    world.insert_resource(EvolutionSender(stats_tx));
    world.insert_resource(EvolutionReceiver(spawn_rx));
    world.insert_resource(EvolutionQueue::default());

    let (env_tx, env_rx) =
        crossbeam_channel::unbounded::<crate::evolution::meta_ai::EnvironmentalEvent>();
    world.insert_resource(EnvironmentalEventReceiver(env_rx));

    let (inbound_tx, inbound_rx) =
        crossbeam_channel::unbounded::<crate::core::components::AgentMigrationData>();
    // The headless adapter has no networking worker and must remain a pure deterministic model.
    // Keeping its inert channel unbounded means no wall-clock consumer timing can decide which
    // agents a seeded experiment retains. The bounded production channel lives in SimulationEngine.
    let (outbound_tx, outbound_rx) =
        crossbeam_channel::unbounded::<crate::core::components::OutboundMigration>();
    let (migration_tx, migration_rx) = crossbeam_channel::unbounded::<u16>();
    world.insert_resource(crate::core::ecs::InboundMigrationReceiver(inbound_rx));
    world.insert_resource(crate::core::ecs::OutboundMigrationSender(outbound_tx));
    world.insert_resource(crate::core::ecs::ShardingResource(std::sync::Arc::new(
        std::sync::RwLock::new(crate::core::ecs::ShardingConfig::default()),
    )));
    world.insert_resource(crate::core::ecs::BevyMigrationTrigger(migration_rx));

    let evolution_settings =
        std::sync::Arc::new(std::sync::Mutex::new(crate::commands::EvolutionSettings {
            mutation_rate: 0.15,
            selection_bias: 1.5,
            grid_resolution: 50,
        }));
    world.insert_resource(BevyEvolutionSettings(evolution_settings));
    world.insert_resource(BevyEvolutionRunning(std::sync::Arc::new(
        std::sync::atomic::AtomicBool::new(false),
    )));
    world.insert_resource(BevyMapElitesGrid(std::sync::Arc::new(
        std::sync::Mutex::new(crate::commands::MapElitesGridState {
            grid: std::collections::HashMap::new(),
            grid_resolution: 50,
        }),
    )));
    world.insert_resource(ActiveEvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    });
    world.insert_resource(BevyMapElitesArchive {
        archive: crate::evolution::map_elites::MapElitesArchive::new(1.0 / 50.0),
    });
    world.insert_resource(NextNodeId(3));
    world.insert_resource(crate::core::ecs::EnvironmentalSpawnSettings::default());

    // A manifest is the complete declared input to a scientific run. The interactive app may opt
    // into WGPU, but adapter availability and `ANIMA_USE_GPU` must not alter this trajectory.
    world.insert_resource(crate::ai::model::BrainModel::new_seeded_cpu(
        15, 64, 4, seed,
    ));
    world.insert_resource(crate::ai::model::BrainInferenceBuffer::default());

    // Simulation LOD, at its default — disabled, which tiers every agent `Hot`, i.e. the pre-LOD
    // behaviour. `SharedLodFocus` is deliberately NOT inserted: there is no camera, and ADR-0004's
    // rule is that a *missing* handle means nobody is observing.
    world.insert_resource(crate::core::simulation_lod::LodFocus::default());
    world.insert_resource(crate::core::simulation_lod::LodBands::default());
    // ADR-0004: a headless experiment declares that there is no observer, which is a stronger claim
    // than "none was wired up" and is what makes the run comparable to the reference path.
    world.insert_resource(crate::core::observer::ObserverPolicy::Absent);
    world.insert_resource(crate::core::observer::ObserverTrace::default());
    world.insert_resource(crate::core::observer::SharedObserverActions::new());

    let _ = config;

    LiveChannels {
        transitions: trans_rx,
        epoch_stats: stats_rx,
        _spawn_tx: spawn_tx,
        _env_tx: env_tx,
        _inbound_tx: inbound_tx,
        _outbound_rx: outbound_rx,
        _migration_tx: migration_tx,
    }
}

/// A cause id a live manifest may use for its own forcings without colliding with the engine's
/// reserved ids.
pub const CAUSE_LIVE_MANIFEST: CauseId = 1;

#[cfg(test)]
mod inference_pump_tests {
    use super::*;
    use crate::ai::model::BrainModelBackend;
    use crate::core::agent_systems::{
        AgentInferenceRequest, InferenceRequestBatch, InferenceResponseBatch,
    };
    use bevy_ecs::prelude::{Schedule, World};

    fn pump_world(
        response_buffers: usize,
    ) -> (
        World,
        crossbeam_channel::Sender<InferenceRequestBatch>,
        crossbeam_channel::Receiver<InferenceResponseBatch>,
        crossbeam_channel::Receiver<InferenceRequestBatch>,
        crossbeam_channel::Sender<InferenceResponseBatch>,
    ) {
        let (req_tx, req_rx) = crossbeam_channel::unbounded();
        let (res_tx, res_rx) = crossbeam_channel::unbounded();
        let (recycle_req_tx, recycle_req_rx) = crossbeam_channel::unbounded();
        let (recycle_res_tx, recycle_res_rx) = crossbeam_channel::unbounded();
        for _ in 0..response_buffers {
            recycle_res_tx
                .send(InferenceResponseBatch {
                    responses: Vec::with_capacity(1),
                })
                .unwrap();
        }

        let mut world = World::new();
        world.insert_resource(crate::ai::model::BrainModel::new_seeded(15, 64, 4, 7));
        world.insert_resource(LiveInferencePump {
            req_rx,
            res_tx,
            recycle_req_tx,
            recycle_res_rx,
            scratch: crate::ai::model::InferenceScratch::with_capacity(1),
            completed_batches: 0,
        });
        (world, req_tx, res_rx, recycle_req_rx, recycle_res_tx)
    }

    fn one_request() -> InferenceRequestBatch {
        InferenceRequestBatch {
            requests: vec![AgentInferenceRequest {
                entity: bevy_ecs::entity::Entity::from_raw(1),
                sensory_input: [0.0; 15],
                request_id: 11,
                brain: None,
            }],
        }
    }

    #[test]
    fn a_live_experiment_pins_the_shared_brain_to_cpu() {
        let adapter = LiveExperimentAdapter::build(
            &WorldLawSet::baseline(),
            &InitialConditionSet::new(Vec::new()),
            7,
            None,
            0,
            0,
        )
        .expect("default live experiment builds");
        let world = adapter.world();
        assert!(matches!(
            world.resource::<crate::ai::model::BrainModel>().backend(),
            BrainModelBackend::NdArray(..)
        ));
    }

    #[test]
    #[should_panic(expected = "live inference response pool invariant violated")]
    fn empty_response_pool_fails_loudly_instead_of_allocating() {
        // Holding the sender is intentional: this must exercise `Empty`, not `Disconnected`.
        let (mut world, req_tx, _res_rx, _recycle_req_rx, _recycle_res_tx) = pump_world(0);
        req_tx.send(one_request()).unwrap();

        let mut schedule = Schedule::default();
        schedule.add_systems(live_inference_pump_system);
        schedule.run(&mut world);
    }

    #[test]
    #[should_panic(expected = "live inference response pool disconnected")]
    fn disconnected_response_pool_is_diagnosed_separately() {
        let (mut world, req_tx, _res_rx, _recycle_req_rx, recycle_res_tx) = pump_world(0);
        drop(recycle_res_tx);
        req_tx.send(one_request()).unwrap();

        let mut schedule = Schedule::default();
        schedule.add_systems(live_inference_pump_system);
        schedule.run(&mut world);
    }

    #[test]
    fn a_recycled_response_buffer_completes_and_returns_the_request_batch() {
        let (mut world, req_tx, res_rx, recycle_req_rx, _recycle_res_tx) = pump_world(1);
        req_tx.send(one_request()).unwrap();

        let mut schedule = Schedule::default();
        schedule.add_systems(live_inference_pump_system);
        schedule.run(&mut world);

        let response = res_rx.try_recv().expect("pump must publish one response");
        assert_eq!(response.responses.len(), 1);
        assert_eq!(response.responses[0].request_id, 11);
        assert_eq!(world.resource::<LiveInferencePump>().completed_batches, 1);
        assert!(recycle_req_rx
            .try_recv()
            .expect("request batch must be recycled")
            .requests
            .is_empty());
    }

    #[test]
    fn real_tick_schedule_restores_the_complete_pool_at_every_boundary() {
        let initial = InitialConditionSet::new(vec![("live.founders".to_string(), 6.0)]);
        let mut adapter =
            LiveExperimentAdapter::build(&WorldLawSet::baseline(), &initial, 7, None, 0, 0)
                .expect("live experiment must build");

        for expected_tick in 1..=64 {
            adapter.run_schedule_once();
            let mut world = adapter.world.borrow_mut();
            let response_pool_len = world.resource::<LiveInferencePump>().recycle_res_rx.len();
            let request_queue_empty = world.resource::<LiveInferencePump>().req_rx.is_empty();
            let completed_batches = world.resource::<LiveInferencePump>().completed_batches;
            let response_queue_empty = world
                .resource::<crate::core::agent_systems::InferenceChannels>()
                .res_rx
                .is_empty();
            assert_eq!(
                response_pool_len,
                crate::core::agent_systems::INFERENCE_POOL_BATCHES,
                "response pool leaked at tick {expected_tick}"
            );
            assert!(
                request_queue_empty,
                "request queue crossed tick {expected_tick}"
            );
            assert!(
                response_queue_empty,
                "response queue crossed tick {expected_tick}"
            );
            assert_eq!(
                completed_batches, expected_tick,
                "the real schedule must exercise exactly one inference batch per tick"
            );

            let mut cognition = world.query::<&crate::core::ecs::CognitiveState>();
            let states: Vec<_> = cognition.iter(&world).copied().collect();
            assert!(!states.is_empty(), "fixture must contain live agents");
            assert!(
                states
                    .iter()
                    .all(|state| matches!(state, crate::core::ecs::CognitiveState::Ready)),
                "all requests must be resolved by tick boundary {expected_tick}"
            );
        }
    }
}
