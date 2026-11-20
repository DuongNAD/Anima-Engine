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
pub const LIVE_MODEL_VERSION: &str = "live-bevy-world/2";

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
/// Card FOUNDER-LAYOUT: genesis founder placement layout (`0.0`/absent = legacy line at
/// `z_fraction = 0.35` with zero extra RNG draws; `1.0` = even 2-D grid over walkable land;
/// `2.0` = random scatter over walkable land from dedicated sub-stream
/// [`crate::core::resources::sim_stream::LIVE_GENESIS_FOUNDERS`]).
///
/// Layouts `1` and `2` respect `live.land_mask = 1.0` (only walkable cells are chosen) and keep
/// founders separated by at least [`FOUNDER_MIN_SPACING`] whenever geometrically feasible.
pub const LIVE_KEY_FOUNDER_LAYOUT: &str = "live.founder_layout";
/// Minimum pairwise spacing (in world units, measured toroidally over `MapBounds`) enforced for
/// `live.founder_layout` modes `1` and `2` whenever feasible (`> 2` cells on a `200x200` /
/// `256x256` world).
pub const FOUNDER_MIN_SPACING: f32 = 2.0;
const FOUNDER_LAYOUT_LINE: u8 = 0;
const FOUNDER_LAYOUT_GRID: u8 = 1;
const FOUNDER_LAYOUT_SCATTER: u8 = 2;
/// Fraction of founders that are predators, in `[0, 1]`.
pub const LIVE_KEY_PREDATOR_FRACTION: &str = "live.predator_fraction";
/// Trees placed at genesis.
pub const LIVE_KEY_TREES: &str = "live.trees";
/// Lakes placed at genesis.
pub const LIVE_KEY_LAKES: &str = "live.lakes";
/// Card S12: lake placement layout at genesis (`0.0`/absent = single row at `z_fraction = 0.8`;
/// `1.0` = 2-D lattice spread over the map; requires `live.lakes >= 1`).
pub const LIVE_KEY_LAKE_LAYOUT: &str = "live.lake_layout";
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

/// Whether agents die of hunger or thirst (feature ecological-selection, branch M), as a
/// **declared** input.
///
/// Same shape and same reasoning as [`LIVE_KEY_EVOLVED_BRAINS`]: the app seeds `MortalityPolicy`
/// from `ANIMA_MORTALITY`, but [`build_live_world`] deliberately does not read the environment —
/// a run's trajectory must not depend on ambient variables. Absent means off, which is exactly
/// today's behaviour, so every live manifest written before this key existed stays bit-identical.
pub const LIVE_KEY_MORTALITY: &str = "live.mortality";
/// Whether continuous intra-tick reproduction (`autonomous_reproduction_system`) runs, as a
/// **declared** input (ADR-0005 option A, Card S1; proposed, in force only if QD-4 = A).
///
/// `1.0` inserts `ReproductionPolicy { enabled: true }`; `0.0` or absent keeps the
/// `ReproductionPolicy::disabled()` every live run made before this key existed, bit-identical.
/// Exact-boolean reading, like `live.mortality`. Declared, never read from the environment.
pub const LIVE_KEY_REPRODUCTION: &str = "live.reproduction";
/// Whether creatures steer toward their sensory target (`steering_drive_system`), as a
/// **declared** input (ADR-0006 option A, F-042).
///
/// `1.0` inserts `LocomotionPolicy { enabled: true }`; `0.0` or absent keeps the policy disabled,
/// which is every live run made before this key existed, bit-identical. Exact-boolean reading,
/// like `live.mortality`. Declared, never read from the environment.
pub const LIVE_KEY_LOCOMOTION: &str = "live.locomotion";
/// Lower bound in `[0, 1]` on the steering drive fraction (ADR-0007 option A, QD-ECO): the drive
/// becomes `max(drive_from_cpg, floor)`. Absent = `0.0` = the heritable-only rule, bit-identical.
/// **Only takes effect when `live.locomotion = 1`.** A nonzero floor makes speed less heritable
/// (selection acts only on the part above it): a viability aid, not evolved locomotion.
pub const LIVE_KEY_DRIVE_FLOOR: &str = "live.drive_floor";
/// `1.0` makes a thirsty agent (hydration below `locomotion::THIRST_STEERING_FRACTION` = 50% of its
/// target) steer to the nearest non-empty lake instead of its food/prey target (ADR-0007 option
/// A). Exact boolean; absent/`0.0` = off, bit-identical. **Only takes effect when
/// `live.locomotion = 1`.**
pub const LIVE_KEY_THIRST_STEERING: &str = "live.thirst_steering";
/// Card SEL2P2-PREP (ported from S11 `226d4b2`/`95044c4b`): `1.0` enables
/// [`crate::core::resources::GrassSteeringPolicy`] in `locomotion::steering_drive_system` for
/// prey with explicit priority: thirst (`> 50%` deficit) -> local `Food` -> `ResourceField`
/// grass (`GRASS_STAY_FRACTION` / `7x7` scan respecting `PlantReservePolicy` and LAND1
/// walkability) -> global `Food` fallback. `0.0`/absent keeps legacy trajectories bit-identical.
/// **Requires `live.locomotion = 1.0`.**
pub const LIVE_KEY_GRASS_STEERING: &str = "live.grass_steering";
/// ADR-0007 option B (thẻ B): a predator strike that credits it `ΔE` EU also adds `k × ΔE` to its
/// hydration, capped at its hydration target. `k >= 0`; absent/`0.0` = off, bit-identical.
/// Hydration is outside the EU ledger. Acts in `combat_system`, so it does not need
/// `live.locomotion`.
pub const LIVE_KEY_KILL_HYDRATION: &str = "live.kill_hydration";
/// ADR-0007 option B: assimilation efficiency of a strike, in `(0, 1]`, replacing
/// `ecology::LINDEMAN_EFFICIENCY` (0.40) in this world only (both strike branches). Absent =
/// `0.40`, bit-identical. The unassimilated remainder still returns to detritus; EU stays closed.
pub const LIVE_KEY_KILL_EFFICIENCY: &str = "live.kill_efficiency";
/// ADR-0007 option B: predator steering only targets living prey with `energy >= E` (`E >= 0`);
/// absent/`0.0` = every living prey, bit-identical. Steering only: the strike in `combat_system`
/// is not filtered. **Only takes effect when `live.locomotion = 1`.**
pub const LIVE_KEY_TARGET_MIN_ENERGY: &str = "live.target_min_energy";
/// Per-birth probability that `autonomous_reproduction_system` mutates the offspring's body
/// genotype (E-SEL1 precondition P4), a **declared** input in `[0, 1]`.
///
/// Absent keeps `DEFAULT_MUTATION_RATE` (`0.05`, the value that system hard-coded before this
/// key existed), so every manifest written earlier stays bit-identical. `0.0` is the drift-control
/// setting: the offspring is an exact copy of its parent, body **and** brain (the brain's own
/// mutation pass is skipped at exactly `0.0`; at any positive rate the brain keeps its historical
/// fixed per-weight rate, because this key is the body-mutation experiment variable, not a global
/// scale). Consulted only when `live.reproduction` is on.
pub const LIVE_KEY_MUTATION_RATE: &str = "live.mutation_rate";
/// Consecutive bottom ticks (energy exhausted) before starvation kills, when mortality is on.
/// Absent keeps the policy's default of 120.
pub const LIVE_KEY_STARVE_TICKS: &str = "live.starve_ticks";
/// Consecutive bottom ticks (hydration exhausted) before dehydration kills, when mortality is on.
/// Absent keeps the policy's default of 120.
pub const LIVE_KEY_DEHYDRATION_TICKS: &str = "live.dehydration_ticks";
/// QD-SEL1b = A (card S6): the prey population at which continuous reproduction stops for prey, a
/// whole number in `[1, 65536]`. Absent keeps `DEFAULT_PREY_CAP` (80), the value
/// `autonomous_reproduction_system` hard-coded, so every earlier manifest is bit-identical. The
/// predator rules are unchanged. Consulted only when `live.reproduction` is on.
///
/// A soft ceiling, as the constant always was: prey are counted once at the start of the system,
/// so every prey eligible in that tick can still reproduce, and one tick can overshoot (measured: a
/// peak of 103 agents at a ceiling of 80, up to 30 births in one tick).
pub const LIVE_KEY_PREY_CAP: &str = "live.prey_cap";
/// QD-SEL1b = A (card S6): multiplies the pasture's carrying capacity (`ResourceField::r_max`) and
/// its standing crop (`r`) once, at genesis, by a factor in `[1e-6, 1]`.
///
/// Not a pasture-only factor: `fruit_growth_system` scales tree fruiting by the local `r_max`
/// (`npp_factor`), so the same factor also slows fruit regrowth (measured: 0.854x at 0.5). A
/// preregistration that manipulates this key manipulates both food channels and must say so. The
/// telemetry vegetation index reads `r / r_max`, so a scaled pasture still looks full there. The EU taken out of the pasture
/// moves to detritus, so the closed-EU total is unchanged. Absent keeps `1.0`, bit-identical. A save
/// carries the standing crop but not the capacity (`r_max` is derived from the biomes), so a restore
/// re-applies the factor to `r_max` only and never to the crop or the detritus.
pub const LIVE_KEY_RESOURCE_CAPACITY_SCALE: &str = "live.resource_capacity_scale";
/// QD-SEL2 (card S10): the fraction `ρ ∈ [0, 1)` of each pasture cell's capacity `r_max` that
/// herbivores cannot eat (`resources::PlantReservePolicy`). Grazing, live and dormant, offers
/// `max(r − ρ·r_max, 0)` instead of `r`, so no cell is grazed below `ρ·r_max`. Regrowth is still
/// paid from detritus, so the closed-EU ledger is unchanged: the reserve is grass left standing,
/// not new energy. Absent or `0` is bit-identical to every earlier manifest.
///
/// `live.standing_crop` keeps its meaning (the whole field, reserve included), so with a reserve
/// it cannot fall below `ρ·Σr_max`; a scarcity ratio read from it has a floor of roughly `ρ`.
pub const LIVE_KEY_PLANT_RESERVE: &str = "live.plant_reserve";
/// Branch F: `1.0` ranks epoch replacement by [`crate::evolution::fitness::EcologicalFitness`];
/// `0.0`/absent keeps the locomotion ranking every pre-F run used. Declared, not ambient —
/// `build_live_world` never reads `ANIMA_FITNESS`.
pub const LIVE_KEY_ECOLOGICAL_FITNESS: &str = "live.ecological_fitness";
/// Weight of the net-surplus term of the ecological rule. Absent reads back as 1.0.
pub const LIVE_KEY_W_SURPLUS: &str = "live.w_surplus";
/// Card SEL2-D: `1.0` enables lockstep neutral-drift replay (requires mortality, reproduction, and
/// a recorded [`crate::core::drift_control::TurnoverSchedule`]); `0.0`/absent runs the normal live
/// rules bit-identically.
pub const LIVE_KEY_DRIFT_CONTROL: &str = "live.drift_control";
/// Card SEL2-D: Channon periodic-reset window length `K` in ticks (`0` = unreset replay from
/// founders; `K > 0` requires `live.drift_control = 1.0` and `duration_ticks % K == 0`).
pub const LIVE_KEY_DRIFT_RESET_TICKS: &str = "live.drift_reset_ticks";
/// Card LAND1: `1.0` enables [`crate::core::resources::TerrainNavigation`] (constrains agent
/// roots to walkable `TerrainMap` cells, snaps genesis lattice positions to nearest walkable cells,
/// and gates spawns onto land); `0.0`/absent keeps it disabled, bit-identical to pre-LAND1 runs.
pub const LIVE_KEY_LAND_MASK: &str = "live.land_mask";
/// Card FIX-CAP / FIX-CAP2 (F-052): `1.0` enables [`crate::core::resources::PreyCapPolicy::strict`]
/// in `autonomous_reproduction_system`, checking `prey_count >= prey_cap` and `pred_count >= 8 ||
/// prey_count < pred_count * 5` against running counts before each birth and incrementing
/// `prey_count`/`pred_count` after each queued birth in both normal and neutral-drift replay modes
/// (recording replay cap-blocked births in
/// [`crate::core::drift_control::DriftDiagnostics::missed_births_capped`]). Because the 5:1 ratio is
/// checked *before* each birth against the running count, prey queued earlier in the same tick count
/// toward it and after the final permitted predator birth `prey_count / pred_count` can fall below
/// 5:1. `0.0`/absent keeps the legacy soft/uncapped path bit-identical so frozen E3 and historical
/// manifests stay reproducible.
pub const LIVE_KEY_STRICT_PREY_CAP: &str = "live.strict_prey_cap";

/// Every key [`LiveWorldConfig::from_initial_conditions`] accepts.
pub const LIVE_KEYS: [&str; 29] = [
    LIVE_KEY_FOUNDERS,
    LIVE_KEY_FOUNDER_LAYOUT,
    LIVE_KEY_PREDATOR_FRACTION,
    LIVE_KEY_TREES,
    LIVE_KEY_LAKES,
    LIVE_KEY_LAKE_LAYOUT,
    LIVE_KEY_FOOD_CAP,
    LIVE_KEY_EVOLVED_BRAINS,
    LIVE_KEY_MORTALITY,
    LIVE_KEY_REPRODUCTION,
    LIVE_KEY_LOCOMOTION,
    LIVE_KEY_DRIVE_FLOOR,
    LIVE_KEY_THIRST_STEERING,
    LIVE_KEY_GRASS_STEERING,
    LIVE_KEY_MUTATION_RATE,
    LIVE_KEY_STARVE_TICKS,
    LIVE_KEY_DEHYDRATION_TICKS,
    LIVE_KEY_ECOLOGICAL_FITNESS,
    LIVE_KEY_W_SURPLUS,
    LIVE_KEY_KILL_HYDRATION,
    LIVE_KEY_KILL_EFFICIENCY,
    LIVE_KEY_TARGET_MIN_ENERGY,
    LIVE_KEY_PREY_CAP,
    LIVE_KEY_RESOURCE_CAPACITY_SCALE,
    LIVE_KEY_PLANT_RESERVE,
    LIVE_KEY_DRIFT_CONTROL,
    LIVE_KEY_DRIFT_RESET_TICKS,
    LIVE_KEY_LAND_MASK,
    LIVE_KEY_STRICT_PREY_CAP,
];

/// The genesis the manifest asked for.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiveWorldConfig {
    pub founders: usize,
    /// Card FOUNDER-LAYOUT: genesis founder layout (`0` = legacy line at `z_fraction = 0.35`,
    /// `1` = even 2-D grid over walkable land, `2` = seeded random scatter over walkable land
    /// from [`crate::core::resources::sim_stream::LIVE_GENESIS_FOUNDERS`]).
    #[serde(default)]
    pub founder_layout: u8,
    pub predator_fraction: f64,
    pub trees: usize,
    pub lakes: usize,
    pub food_cap: usize,
    /// ADR-0003's per-agent brains, off unless a manifest asks. `#[serde(default)]` so a config
    /// serialized before this field existed reads back as the legacy path rather than failing —
    /// the same direction of default the key itself takes.
    #[serde(default)]
    pub evolved_brains: bool,
    /// Branch M death rule, off unless a manifest asks. Off is today's behaviour, so a config
    /// serialized before these fields existed reads back as the world that never dies of hunger.
    #[serde(default)]
    pub mortality: bool,
    /// Continuous reproduction (ADR-0005 option A), off unless a manifest asks. Off is the
    /// historical `ReproductionPolicy::disabled()`, so older serialized configs read back unchanged.
    #[serde(default)]
    pub reproduction: bool,
    /// Steering drive (ADR-0006 option A), off unless a manifest asks. Off is the historical
    /// "creatures never translate" world, so older serialized configs read back unchanged.
    #[serde(default)]
    pub locomotion: bool,
    /// ADR-0007 option A: floor on the steering drive fraction; `0.0` = off. Needs `locomotion`.
    #[serde(default)]
    pub drive_floor: f32,
    /// ADR-0007 option A: thirsty agents steer to the nearest lake. Needs `locomotion`.
    #[serde(default)]
    pub thirst_steering: bool,
    /// Card SEL2P2-PREP (S11): prey steer toward pasture grass (`1.0` on, `0.0`/absent off). Needs
    /// `locomotion`.
    #[serde(default)]
    pub grass_steering: bool,
    /// Offspring body-mutation probability (see [`LIVE_KEY_MUTATION_RATE`]); absent reads back as
    /// the historical `0.05`.
    #[serde(default = "default_mutation_rate")]
    pub mutation_rate: f64,
    /// Declared thresholds; absent reads back as the policy's own defaults.
    #[serde(default = "default_starve_ticks")]
    pub starve_ticks: u32,
    #[serde(default = "default_dehydration_ticks")]
    pub dehydration_ticks: u32,
    /// Branch F fitness rule: `1.0` selects `ecological`, `0.0`/absent keeps `locomotion` —
    /// the ranking every run made before branch F existed.
    #[serde(default)]
    pub ecological_fitness: bool,
    /// Weight of the net-surplus term of the ecological rule; absent reads back as 1.0.
    #[serde(default = "default_w_surplus")]
    pub w_surplus: f32,
    /// ADR-0007 option B: hydration gained per EU a predator assimilates from a kill; `0.0` = off.
    #[serde(default)]
    pub kill_hydration: f32,
    /// ADR-0007 option B: strike assimilation efficiency; absent reads back as the Lindeman 0.40.
    #[serde(default = "default_kill_efficiency")]
    pub kill_efficiency: f32,
    /// ADR-0007 option B: minimum prey energy for predator steering; `0.0` = off.
    #[serde(default)]
    pub target_min_energy: f32,
    /// Card S6: prey reproduction ceiling; absent reads back as the historical 80.
    #[serde(default = "default_prey_cap")]
    pub prey_cap: usize,
    /// Card S6: pasture capacity factor applied at genesis; absent reads back as `1.0`.
    #[serde(default = "default_resource_capacity_scale")]
    pub resource_capacity_scale: f64,
    /// Card S10: ungrazeable fraction of each cell's capacity; absent reads back as `0.0` (off).
    #[serde(default)]
    pub plant_reserve: f64,
    /// Card SEL2-D: lockstep neutral-drift replay mode (`1.0` on, `0.0`/absent off).
    #[serde(default)]
    pub drift_control: bool,
    /// Card SEL2-D: Channon periodic-reset interval `K` in ticks (`0` = unreset replay from
    /// founders).
    #[serde(default)]
    pub drift_reset_ticks: u64,
    /// Card S12: genesis lake layout (`0` = row at `z_fraction = 0.8`, `1` = 2-D lattice).
    #[serde(default)]
    pub lake_layout: u8,
    /// Card LAND1: `1.0` enables `TerrainNavigation` and snaps lattice spawns to walkable cells;
    /// `0.0`/absent keeps terrain navigation disabled (bit-identical to trunk).
    #[serde(default)]
    pub land_mask: bool,
    /// Card FIX-CAP (F-052): `1.0` enforces strict per-birth cap counting in
    /// `autonomous_reproduction_system`; `0.0`/absent keeps the legacy soft gate bit-identical.
    #[serde(default)]
    pub strict_prey_cap: bool,
}

fn default_prey_cap() -> usize {
    crate::core::resources::DEFAULT_PREY_CAP
}

fn default_resource_capacity_scale() -> f64 {
    1.0
}

fn default_kill_efficiency() -> f32 {
    crate::core::ecology::LINDEMAN_EFFICIENCY
}

fn default_mutation_rate() -> f64 {
    crate::core::resources::DEFAULT_MUTATION_RATE
}

fn default_starve_ticks() -> u32 {
    crate::core::resources::MortalityPolicy::disabled().starve_ticks
}

fn default_dehydration_ticks() -> u32 {
    crate::core::resources::MortalityPolicy::disabled().dehydration_ticks
}

fn default_w_surplus() -> f32 {
    crate::evolution::fitness::FitnessRuleSetting::default().w_surplus
}

impl Default for LiveWorldConfig {
    /// The engine's own genesis: ten founders, seven prey and three predators, and the stock
    /// environment-spawn settings — the population `SimulationEngine::start` builds when there is no
    /// save to load. Brains are off, which is `BrainPolicy::default()` and the ADR-0003 baseline.
    /// Mortality is off, which is `MortalityPolicy::default()` — nobody dies of hunger.
    fn default() -> Self {
        Self {
            founders: 10,
            founder_layout: 0,
            predator_fraction: 0.3,
            trees: 8,
            lakes: 2,
            food_cap: crate::core::ecs::FoodSpawnSettings::default().max_food_count,
            evolved_brains: false,
            mortality: false,
            reproduction: false,
            locomotion: false,
            drive_floor: 0.0,
            thirst_steering: false,
            grass_steering: false,
            mutation_rate: default_mutation_rate(),
            starve_ticks: default_starve_ticks(),
            dehydration_ticks: default_dehydration_ticks(),
            ecological_fitness: false,
            w_surplus: default_w_surplus(),
            kill_hydration: 0.0,
            kill_efficiency: default_kill_efficiency(),
            target_min_energy: 0.0,
            prey_cap: default_prey_cap(),
            resource_capacity_scale: default_resource_capacity_scale(),
            plant_reserve: 0.0,
            drift_control: false,
            drift_reset_ticks: 0,
            lake_layout: 0,
            land_mask: false,
            strict_prey_cap: false,
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
                LIVE_KEY_FOUNDER_LAYOUT => match *value {
                    0.0 => config.founder_layout = FOUNDER_LAYOUT_LINE,
                    1.0 => config.founder_layout = FOUNDER_LAYOUT_GRID,
                    2.0 => config.founder_layout = FOUNDER_LAYOUT_SCATTER,
                    other => {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: other,
                            min: 0.0,
                            max: 2.0,
                        })
                    }
                },
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
                // Same exact-boolean reading as `live.evolved_brains`: a threshold is declared,
                // not rounded.
                LIVE_KEY_MORTALITY => match *value {
                    0.0 => config.mortality = false,
                    1.0 => config.mortality = true,
                    other => {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: other,
                            min: 0.0,
                            max: 1.0,
                        })
                    }
                },
                LIVE_KEY_REPRODUCTION => match *value {
                    0.0 => config.reproduction = false,
                    1.0 => config.reproduction = true,
                    other => {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: other,
                            min: 0.0,
                            max: 1.0,
                        })
                    }
                },
                LIVE_KEY_LOCOMOTION => match *value {
                    0.0 => config.locomotion = false,
                    1.0 => config.locomotion = true,
                    other => {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: other,
                            min: 0.0,
                            max: 1.0,
                        })
                    }
                },
                LIVE_KEY_DRIVE_FLOOR => {
                    if !(0.0..=1.0).contains(value) {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: *value,
                            min: 0.0,
                            max: 1.0,
                        });
                    }
                    config.drive_floor = *value as f32;
                }
                LIVE_KEY_THIRST_STEERING => match *value {
                    0.0 => config.thirst_steering = false,
                    1.0 => config.thirst_steering = true,
                    other => {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: other,
                            min: 0.0,
                            max: 1.0,
                        })
                    }
                },
                LIVE_KEY_GRASS_STEERING => match *value {
                    0.0 => config.grass_steering = false,
                    1.0 => config.grass_steering = true,
                    other => {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: other,
                            min: 0.0,
                            max: 1.0,
                        })
                    }
                },
                LIVE_KEY_PREY_CAP => config.prey_cap = bounded_count(key, *value, 1.0, 65_536.0)?,
                LIVE_KEY_RESOURCE_CAPACITY_SCALE => {
                    // A floor of 1e-6, not "> 0": a subnormal factor passes `> 0` and then becomes 0
                    // as an `f32`, which silently kills the pasture.
                    if !(*value >= 1e-6 && *value <= 1.0) {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: *value,
                            min: 1e-6,
                            max: 1.0,
                        });
                    }
                    config.resource_capacity_scale = *value;
                }
                LIVE_KEY_PLANT_RESERVE => {
                    // Half-open: a reserve of 1 would leave nothing to eat anywhere, and the
                    // `!(...)` form also refuses NaN.
                    if !(*value >= 0.0 && *value < 1.0) {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: *value,
                            min: 0.0,
                            max: 1.0,
                        });
                    }
                    config.plant_reserve = *value;
                }
                LIVE_KEY_MUTATION_RATE => {
                    if !(0.0..=1.0).contains(value) {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: *value,
                            min: 0.0,
                            max: 1.0,
                        });
                    }
                    config.mutation_rate = *value;
                }
                LIVE_KEY_STARVE_TICKS => {
                    config.starve_ticks = bounded_count(key, *value, 0.0, 65_536.0)? as u32
                }
                LIVE_KEY_DEHYDRATION_TICKS => {
                    config.dehydration_ticks = bounded_count(key, *value, 0.0, 65_536.0)? as u32
                }
                LIVE_KEY_ECOLOGICAL_FITNESS => match *value {
                    0.0 => config.ecological_fitness = false,
                    1.0 => config.ecological_fitness = true,
                    other => {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: other,
                            min: 0.0,
                            max: 1.0,
                        })
                    }
                },
                LIVE_KEY_W_SURPLUS => {
                    if !value.is_finite() || *value < 0.0 {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: *value,
                            min: 0.0,
                            max: f64::MAX,
                        });
                    }
                    config.w_surplus = *value as f32;
                }
                LIVE_KEY_KILL_HYDRATION => {
                    if !(0.0..=1_000.0).contains(value) {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: *value,
                            min: 0.0,
                            max: 1_000.0,
                        });
                    }
                    config.kill_hydration = *value as f32;
                }
                LIVE_KEY_KILL_EFFICIENCY => {
                    // (0, 1]: zero efficiency would make a strike pure destruction, above one
                    // would create energy.
                    if !(*value > 0.0 && *value <= 1.0) {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: *value,
                            min: f64::MIN_POSITIVE,
                            max: 1.0,
                        });
                    }
                    config.kill_efficiency = *value as f32;
                }
                LIVE_KEY_TARGET_MIN_ENERGY => {
                    if !(0.0..=1_000_000.0).contains(value) {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: *value,
                            min: 0.0,
                            max: 1_000_000.0,
                        });
                    }
                    config.target_min_energy = *value as f32;
                }
                LIVE_KEY_DRIFT_CONTROL => match *value {
                    0.0 => config.drift_control = false,
                    1.0 => config.drift_control = true,
                    other => {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: other,
                            min: 0.0,
                            max: 1.0,
                        })
                    }
                },
                LIVE_KEY_DRIFT_RESET_TICKS => {
                    config.drift_reset_ticks = bounded_count(key, *value, 0.0, 1_000_000.0)? as u64;
                }
                LIVE_KEY_LAKE_LAYOUT => match *value {
                    0.0 => config.lake_layout = 0,
                    1.0 => config.lake_layout = 1,
                    other => {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: other,
                            min: 0.0,
                            max: 1.0,
                        })
                    }
                },
                LIVE_KEY_LAND_MASK => match *value {
                    0.0 => config.land_mask = false,
                    1.0 => config.land_mask = true,
                    other => {
                        return Err(ExperimentError::OutOfRange {
                            field: key.clone(),
                            value: other,
                            min: 0.0,
                            max: 1.0,
                        })
                    }
                },
                LIVE_KEY_STRICT_PREY_CAP => match *value {
                    0.0 => config.strict_prey_cap = false,
                    1.0 => config.strict_prey_cap = true,
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
        if config.lake_layout == 1 && config.lakes == 0 {
            return Err(ExperimentError::InvalidLaw {
                reason: "live.lake_layout = 1.0 requires live.lakes >= 1".into(),
            });
        }
        if config.drift_control && (!config.mortality || !config.reproduction) {
            return Err(ExperimentError::InvalidLaw {
                reason: "live.drift_control = 1.0 requires both live.mortality = 1.0 and \
                         live.reproduction = 1.0"
                    .into(),
            });
        }
        if config.drift_reset_ticks > 0 && !config.drift_control {
            return Err(ExperimentError::InvalidLaw {
                reason: "live.drift_reset_ticks > 0 requires live.drift_control = 1.0".into(),
            });
        }
        if config.grass_steering && !config.locomotion {
            return Err(ExperimentError::InvalidLaw {
                reason: "live.grass_steering = 1.0 requires live.locomotion = 1.0".into(),
            });
        }
        Ok(config)
    }

    /// How many founders are predators.
    pub fn predators(&self) -> usize {
        ((self.founders as f64) * self.predator_fraction).round() as usize
    }

    /// Feature ecological-selection, branch O-5: refuse a manifest's observable requests whose
    /// producing structure this run does not declare. Death observables need a declared death
    /// rule — reporting zeros for a world where nobody CAN die would look like a measurement.
    /// Fail here, loudly, instead.
    pub fn ensure_observables_supported(&self, ids: &[String]) -> Result<(), ExperimentError> {
        for id in ids {
            if matches!(
                id.as_str(),
                "live.deaths"
                    | "live.mean_lifespan"
                    | "live.deaths_starvation"
                    | "live.deaths_dehydration"
            ) && !self.mortality
            {
                return Err(ExperimentError::InvalidObservableRequest {
                    id: id.clone(),
                    reason: "the run declares no death rule (live.mortality absent or 0.0); \
                             nobody can die, so a death count would be a fabricated zero"
                        .to_owned(),
                });
            }
            if matches!(
                id.as_str(),
                "live.deaths_starvation" | "live.deaths_dehydration"
            ) && self.drift_control
            {
                return Err(ExperimentError::InvalidObservableRequest {
                    id: id.clone(),
                    reason: "live.drift_control = 1.0 replaces physiological deaths with uniform \
                             random deaths that carry no starvation/dehydration cause; cause-split \
                             counts would be fabricated zeros"
                        .to_owned(),
                });
            }
            if id == "live.births" && !self.reproduction {
                return Err(ExperimentError::InvalidObservableRequest {
                    id: id.clone(),
                    reason: "the run declares no birth rule (live.reproduction absent or 0.0); \
                             nobody can be born, so a birth count would be a fabricated zero"
                        .to_owned(),
                });
            }
        }
        Ok(())
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
    /// Running sum of `live.mean_agent_energy` across every simulated tick (`EU·tick`).
    ///
    /// Mirrored from [`LiveExperimentAdapter`] on each tick and before `snapshot`, and restored in
    /// [`LiveExperimentAdapter::build`] on resume so save/load checkpoint forks preserve the exact
    /// cumulative integral inside this module without modifying `SavedSimulationState`'s fields or
    /// `world_checksum`. Pre-S2 schema-v1 saves default to `0.0`.
    #[serde(default)]
    pub mean_energy_integral: f64,
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
            mean_energy_integral: 0.0,
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

/// The food-item cap a manifest declared with `live.food_cap`, before the active environmental
/// event scales it.
///
/// `apply_environmental_effects_system` rewrites `FoodSpawnSettings::max_food_count` on every tick
/// as `base × event multiplier`. Its base used to be a literal `50`, so a declared cap was
/// overwritten on the first tick and `live.food_cap` changed nothing (F-043). The adapter inserts
/// this resource on genesis and on restore alike; the system reads it through `Option<Res<..>>`,
/// so a world without it — every app run — keeps the literal base of 50 and behaves exactly as it
/// did.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveFoodCap(pub usize);

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
    // `PathogenTransmission` acts on the living agents' reserves, the store the census reads.
    mut reserves: Query<
        (Entity, &mut crate::ai::hrrl::HomeostaticState),
        With<crate::core::components::Agent>,
    >,
    mut epidemic_buf: Local<Vec<Entity>>,
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
        match &cmd.kind {
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
            InterventionKind::GeneticInjection {
                count,
                trait_modifications,
                ..
            } => {
                state.ledger.record(
                    cmd.cause_id,
                    None,
                    tick,
                    "live.genetic_injection",
                    *count as f64,
                    trait_modifications.len() as f64,
                    "genetic injection applied trait modifications in target region",
                );
                state.applied_interventions += 1;
            }
            InterventionKind::ToxicityPlume {
                center_x,
                center_y,
                radius,
                toxicity_level,
                ..
            } => {
                let (Some(field), Some(pool), Some(energy)) =
                    (field.as_mut(), pool.as_mut(), energy.as_mut())
                else {
                    continue;
                };
                let fraction = (cmd.signed_intensity_at(tick).abs() * (*toxicity_level * 0.05))
                    .clamp(0.0, 1.0) as f64;
                let plume_region = crate::core::intervention::Region::Radius {
                    cx: *center_x,
                    cy: *center_y,
                    r: *radius,
                };
                let mut removed = 0.0f64;
                for y in 0..field.height {
                    for x in 0..field.width {
                        if !plume_region.contains(x as u32, y as u32) {
                            continue;
                        }
                        let i = y * field.width + x;
                        let before = field.r[i] as f64;
                        let after = (field.r[i] as f64 * (1.0 - fraction)).max(0.0) as f32;
                        field.r[i] = after;
                        removed += before - after as f64;
                    }
                }
                if removed > 0.0 {
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
                        "toxicity plume moves standing crop into detritus",
                    );
                    state.applied_interventions += 1;
                }
            }
            InterventionKind::PathogenTransmission {
                pathogen_id: _,
                transmission_rate,
                mortality_multiplier,
            } => {
                let (Some(pool), Some(energy)) = (pool.as_mut(), energy.as_mut()) else {
                    continue;
                };
                let fraction = (cmd.signed_intensity_at(tick).abs()
                    * (*transmission_rate * *mortality_multiplier * 0.01))
                    .clamp(0.0, 0.5) as f64;
                if fraction <= 0.0 {
                    continue;
                }
                // The epidemic has to take what it moves from the **reserves**. `pool.animals` is
                // only a mirror: `ecosystem_census_system` re-snapshots it from the agents at the
                // end of every tick, so debiting the mirror alone left every reserve untouched while
                // detritus kept the credit, and closed EU grew by the "culled" amount each time the
                // forcing fired (G5, `tests/ledger_bypass_tests.rs`: +148 EU in one firing).
                //
                // Every living agent loses the same share of its own reserve. They are visited in
                // entity order, as the predator cull is, so the sum is the same in a replay.
                epidemic_buf.clear();
                epidemic_buf.extend(reserves.iter().map(|(entity, _)| entity));
                epidemic_buf.sort_unstable_by_key(|e| (e.index(), e.generation()));
                let share = fraction as f32;
                let mut stripped = 0.0f64;
                for entity in epidemic_buf.iter() {
                    if let Ok((_, mut homeo)) = reserves.get_mut(*entity) {
                        let amount = homeo.energy.max(0.0) * share;
                        // The amount the `f32` reserve actually lost, not the amount asked for.
                        stripped +=
                            crate::core::energy_ledger::debit_reserve(&mut homeo.energy, amount);
                    }
                }
                if stripped > 0.0 {
                    // Same routing as the reproduction overhead: the reserves have already been
                    // debited, so the amount is settled through the ledger whether or not the
                    // `Animals` mirror is fresh.
                    crate::core::energy_ledger::settle_reserve_outflow_to_detritus(
                        energy,
                        pool,
                        stripped,
                        EnergyEvent::Intervention,
                    );
                    state.ledger.record(
                        cmd.cause_id,
                        None,
                        tick,
                        "live.animals_eu",
                        pool.animals,
                        -stripped,
                        "pathogen transmission epidemic strips a share of every living reserve into detritus",
                    );
                    state.applied_interventions += 1;
                }
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

impl LiveInferencePump {
    pub fn new(
        req_rx: crossbeam_channel::Receiver<crate::core::agent_systems::InferenceRequestBatch>,
        res_tx: crossbeam_channel::Sender<crate::core::agent_systems::InferenceResponseBatch>,
        recycle_req_tx: crossbeam_channel::Sender<
            crate::core::agent_systems::InferenceRequestBatch,
        >,
        recycle_res_rx: crossbeam_channel::Receiver<
            crate::core::agent_systems::InferenceResponseBatch,
        >,
    ) -> Self {
        Self {
            req_rx,
            res_tx,
            recycle_req_tx,
            recycle_res_rx,
            scratch: crate::ai::model::InferenceScratch::with_capacity(256),
            completed_batches: 0,
        }
    }
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

/// The legacy observables, emitted first and in this order: exactly the twelve the E2
/// preregistration requests, as amended on 2026-11-16 (`amendment_1_live_dormant_eu`).
///
/// **Frozen. Never add, remove or reorder an id here.** `tests/prereg_e2_manifest_tests.rs` asserts
/// that the committed E2 manifests request this list and nothing else, and
/// `tests/e2_frozen_manifest_tests.rs` asserts that those manifests are byte-identical to the copies
/// the E2 run published beside its results. Commit `970d963` inserted `live.dormant_eu` here and
/// edited the four registered manifests to keep the first test green, a month after E2 had run.
/// `npm run check:e2` stayed red until the owner's decision QD-E2M amended the preregistration and
/// re-ran the whole ensemble. A new observable goes in [`LIVE_EXTENDED_OBSERVABLE_IDS`].
pub const LIVE_OBSERVABLE_IDS: [&str; 12] = [
    "plants",
    "detritus",
    "live.animals_eu",
    "live.dormant_eu",
    "live.closed_eu_total",
    "live.agent_count",
    "live.herbivore_count",
    "live.predator_count",
    "live.food_items",
    "live.standing_crop",
    "live.mean_agent_energy",
    "live.season_phase",
];

/// Feature ecological-selection, branch O: the distributional and turnover observables, emitted
/// **after** the legacy twelve in the same stable order. The frozen E2 preregistration pins the
/// legacy list — these are additive extensions, not edits to a registered experiment's metrics.
///
/// Card S9 appended `live.deaths_starvation` and `live.deaths_dehydration`: the window's deaths
/// split by cause, one cause per death (rule in `core::mortality::lethal_cause`), so the two add up
/// to `live.deaths` at every sample.
///
/// Card S2 appended three observables at the end:
/// - `live.mean_energy_integral` (`EU·tick`, `Aggregation::Instant`): running sum over every
///   simulated tick of `live.mean_agent_energy`. Ticks with zero living agents contribute `0.0` so
///   an extinct population retains its finite pre-extinction integral (no sentinel value).
/// - `live.trait_mass_mean_west` / `live.trait_mass_mean_east` (`EU`, `Aggregation::Instant`): mean
///   `AgentGenotype.0.total_mass()` over living agents whose world X coordinate maps into columns
///   `0..=127` (`Region::Rect{0,0,127,255}`) vs `128..=255` (`Region::Rect{128,0,255,255}`) on the
///   256×256 resource/terrain grid, using the same normalized column formula as
///   [`ResourceField::cell_index`] and [`crate::core::terrain::TerrainMap::get_map_indices`]. An
///   empty half emits `0.0` via [`mass_statistics`] (`&[]`), matching `live.trait_mass_mean`.
///
/// Time-to-floor remains a **derived** quantity computed post-hoc from time series by analysis
/// scripts: floor thresholds are experiment-specific, and pre-floor / never-floor runs are
/// right-censored so any per-tick number would either encode a sentinel (corrupting paired deltas)
/// or conflate survival time with hitting the floor.
pub const LIVE_EXTENDED_OBSERVABLE_IDS: [&str; 15] = [
    "live.trait_mass_mean",
    "live.trait_mass_var",
    "live.trait_mass_p10",
    "live.trait_mass_p90",
    "live.shannon",
    "live.simpson",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
    "live.archive_coverage",
    "live.deaths_starvation",
    "live.deaths_dehydration",
    "live.mean_energy_integral",
    "live.trait_mass_mean_west",
    "live.trait_mass_mean_east",
];

/// Every observable id the adapter emits (legacy + extended), in emission order.
pub fn all_live_observable_ids() -> Vec<&'static str> {
    LIVE_OBSERVABLE_IDS
        .iter()
        .copied()
        .chain(LIVE_EXTENDED_OBSERVABLE_IDS.iter().copied())
        .collect()
}

/// Mean / population variance / p10 / p90 of a trait sample. Percentiles use nearest-rank on the
/// ascending sample — the same convention the tick-capture summary uses — and an empty sample
/// yields all zeros (an absent population is a real state, not missing data).
pub fn mass_statistics(masses: &[f32]) -> [f64; 4] {
    if masses.is_empty() {
        return [0.0; 4];
    }
    let mut sorted: Vec<f32> = masses.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    let mean = sorted.iter().map(|m| *m as f64).sum::<f64>() / n as f64;
    let var = sorted
        .iter()
        .map(|m| (*m as f64 - mean) * (*m as f64 - mean))
        .sum::<f64>()
        / n as f64;
    // Nearest-rank: ceil(p/100 * N), clamped into range.
    let rank = |p: f64| -> f32 {
        let idx = ((p / 100.0) * n as f64).ceil() as usize;
        sorted[idx.clamp(1, n) - 1]
    };
    [mean, var, rank(10.0) as f64, rank(90.0) as f64]
}

/// Read the live world's observables, in the stable order of [`LIVE_OBSERVABLE_IDS`] followed by
/// [`LIVE_EXTENDED_OBSERVABLE_IDS`].
///
/// Sampling drains the turnover window ([`crate::core::resources::VitalStatistics`]):
/// `live.births` / `live.deaths` count events since the previous sample, and
/// `live.deaths_starvation` / `live.deaths_dehydration` split that same window's deaths by cause, and
/// `live.mean_lifespan` averages `survival_ticks` over window deaths (`0.0` when the window had
/// none — with the structural O-5 guarantee that a run without a death rule can never be asked
/// for it). A read that is not a sample must use [`live_observables_peek`], or it steals the
/// sample's events.
pub fn live_observables(world: &mut World) -> Vec<(String, f64)> {
    read_live_observables(world, true)
}

/// The same values as [`live_observables`] without closing the turnover window.
///
/// The shared runner reads a model's observables on every tick to check that they are finite and
/// unique, and keeps only the reads on sample ticks. When every one of those reads drained the
/// window, a sampled `live.births` counted the births of its own tick only: 0 against 20 true births
/// over 3,000 ticks (`tests/live_windowed_observable_sampling_tests.rs`).
pub fn live_observables_peek(world: &mut World) -> Vec<(String, f64)> {
    read_live_observables(world, false)
}

/// Living agent count and per-tick mean reserve energy (`0.0` when zero agents are alive).
///
/// Shared by `read_live_observables` (`live.mean_agent_energy`) and
/// `LiveExperimentAdapter::accumulate_mean_energy_integral` so both see the exact same `f64`
/// average on every tick.
fn current_mean_agent_energy(world: &mut World) -> (u64, f64) {
    use crate::core::components::Agent;

    let mut agents = 0u64;
    let mut energy_sum = 0.0f64;
    let mut q = world.query_filtered::<&crate::ai::hrrl::HomeostaticState, With<Agent>>();
    for homeo in q.iter(world) {
        agents += 1;
        energy_sum += homeo.energy as f64;
    }
    let mean = if agents == 0 {
        0.0
    } else {
        energy_sum / agents as f64
    };
    (agents, mean)
}

/// Map an entity's world X coordinate onto the resource/terrain grid column (`0..grid_width`) and
/// return `true` iff it falls in the West half (`col < grid_width / 2`: columns `0..=127` on a
/// 256×256 grid, matching E3's `Region::Rect { min_x: 0, min_y: 0, max_x: 127, max_y: 255 }`), with
/// `false` indicating the East half (`128..=255`, `Region::Rect { 128, 0, 255, 255 }`).
///
/// Uses the identical normalized column formula as [`ResourceField::cell_index`] and
/// [`crate::core::terrain::TerrainMap::get_map_indices`], with `u` clamped to `[0.0, 1.0]` so an
/// agent near or past the map border stays cleanly partitioned between West and East.
#[inline]
fn world_x_is_west(x: f32, grid_width: usize, min_x: f32, max_x: f32) -> bool {
    let span_x = max_x - min_x;
    if grid_width == 0 || span_x <= 0.0 || !x.is_finite() {
        return true;
    }
    let u = ((x - min_x) / span_x).clamp(0.0, 1.0);
    let col = ((u * grid_width as f32) as usize).min(grid_width - 1);
    col < grid_width / 2
}

fn read_live_observables(world: &mut World, close_window: bool) -> Vec<(String, f64)> {
    use crate::core::components::{Agent, Food, Predator, Prey};

    let (plants, animals, detritus, dormant) = world
        .get_resource::<EcosystemBiomass>()
        .map(|p| (p.plants, p.animals, p.detritus, p.dormant))
        .unwrap_or((0.0, 0.0, 0.0, 0.0));
    let (standing_crop, grid_width, min_x, max_x) = world
        .get_resource::<ResourceField>()
        .map(|f| (f.total_biomass(), f.width, f.min_x, f.max_x))
        .unwrap_or_else(|| {
            let (min_x, max_x) = world
                .get_resource::<crate::core::ecs::MapBounds>()
                .map(|b| (b.min.x, b.max.x))
                .unwrap_or((-100.0, 100.0));
            (0.0, 256, min_x, max_x)
        });
    let season_phase = world
        .get_resource::<crate::core::ecology::SeasonClock>()
        .map(|c| c.phase as f64)
        .unwrap_or(0.0);

    let (agents, mean_agent_energy) = current_mean_agent_energy(world);
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

    // Branch-O + Card S2: trait distribution (body mass, the MTE master trait), global and
    // west/east halves, diversity over the same prey/predator abundances the panel indices use,
    // archive occupancy, the drained turnover window, and the cumulative mean-energy integral.
    let mut masses: Vec<f32> = Vec::new();
    let mut west_masses: Vec<f32> = Vec::new();
    let mut east_masses: Vec<f32> = Vec::new();
    {
        let mut q = world.query_filtered::<(
            &crate::core::agent_systems::AgentGenotype,
            Option<&crate::core::ecs::Position>,
        ), With<Agent>>();
        for (g, pos) in q.iter(world) {
            let m = g.0.total_mass();
            masses.push(m);
            if let Some(p) = pos {
                if world_x_is_west(p.0.x, grid_width, min_x, max_x) {
                    west_masses.push(m);
                } else {
                    east_masses.push(m);
                }
            }
        }
    }
    let stats = mass_statistics(&masses);
    let trait_mass_mean_west = mass_statistics(&west_masses)[0];
    let trait_mass_mean_east = mass_statistics(&east_masses)[0];
    let mean_energy_integral = world
        .get_resource::<LiveExperimentState>()
        .map(|s| s.mean_energy_integral)
        .unwrap_or(0.0);
    let counts = [herbivores as u32, predators as u32];
    let shannon = crate::core::ecology::shannon_index(&counts);
    let simpson = crate::core::ecology::simpson_index(&counts);
    // Read the cause split first: `sample_window` closes it together with the death count.
    let (deaths_starvation, deaths_dehydration) = world
        .get_resource::<crate::core::resources::VitalStatistics>()
        .map(|v| v.peek_cause_window())
        .unwrap_or((0, 0));
    let (births, deaths, mean_lifespan) = if close_window {
        match world.get_resource_mut::<crate::core::resources::VitalStatistics>() {
            Some(mut v) => v.sample_window(),
            None => (0, 0, None),
        }
    } else {
        match world.get_resource::<crate::core::resources::VitalStatistics>() {
            Some(v) => v.peek_window(),
            None => (0, 0, None),
        }
    };
    let archive_coverage = world
        .get_resource::<crate::core::agent_systems::BevyMapElitesArchive>()
        .map(|a| a.archive.grid.len() as f64)
        .unwrap_or(0.0);
    let is_drift_replay = world
        .get_resource::<crate::core::drift_control::DriftControlState>()
        .is_some_and(|d| d.is_replay());

    let mut out = vec![
        ("plants".to_string(), plants),
        ("detritus".to_string(), detritus),
        ("live.animals_eu".to_string(), animals),
        ("live.dormant_eu".to_string(), dormant),
        (
            "live.closed_eu_total".to_string(),
            crate::core::energy_ledger::closed_total_eu(plants, animals, detritus, dormant),
        ),
        ("live.agent_count".to_string(), agents as f64),
        ("live.herbivore_count".to_string(), herbivores),
        ("live.predator_count".to_string(), predators),
        ("live.food_items".to_string(), food_items),
        ("live.standing_crop".to_string(), standing_crop),
        ("live.mean_agent_energy".to_string(), mean_agent_energy),
        ("live.season_phase".to_string(), season_phase),
        ("live.trait_mass_mean".to_string(), stats[0]),
        ("live.trait_mass_var".to_string(), stats[1]),
        ("live.trait_mass_p10".to_string(), stats[2]),
        ("live.trait_mass_p90".to_string(), stats[3]),
        ("live.shannon".to_string(), shannon as f64),
        ("live.simpson".to_string(), simpson as f64),
        ("live.births".to_string(), births as f64),
        ("live.deaths".to_string(), deaths as f64),
        (
            "live.mean_lifespan".to_string(),
            mean_lifespan.unwrap_or(0.0) as f64,
        ),
        ("live.archive_coverage".to_string(), archive_coverage),
    ];
    if !is_drift_replay {
        out.push((
            "live.deaths_starvation".to_string(),
            deaths_starvation as f64,
        ));
        out.push((
            "live.deaths_dehydration".to_string(),
            deaths_dehydration as f64,
        ));
    }
    out.push((
        "live.mean_energy_integral".to_string(),
        mean_energy_integral,
    ));
    out.push((
        "live.trait_mass_mean_west".to_string(),
        trait_mass_mean_west,
    ));
    out.push((
        "live.trait_mass_mean_east".to_string(),
        trait_mass_mean_east,
    ));
    out
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
    /// Running sum of `live.mean_agent_energy` across every simulated tick (`EU·tick`).
    mean_energy_integral: f64,
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
            .field("mean_energy_integral", &self.mean_energy_integral)
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
        let channels = install_runtime_resources(&mut world, seed, &config, restore);

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
        let mean_energy_integral = state.mean_energy_integral;
        world.insert_resource(state);
        world.insert_resource(LiveForcings::default());
        world.insert_resource(LiveInterventions::with_capacity(16));
        world.insert_resource(crate::core::resources::ScientificCounterFault::default());

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
            mean_energy_integral,
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
        self.sync_drift_tick();
        self.run_tick();
        self.drain_absent_thread_queues();
        self.accumulate_mean_energy_integral();
        self.record_drift_shortfalls_if_due(self.clock.tick, false);
    }

    /// Update the cumulative `live.mean_energy_integral` accumulator once after a simulated tick
    /// and mirror it onto [`LiveExperimentState`] so `live_observables(&mut World)` and snapshots see
    /// the exact running integral whether or not observables are sampled on this tick.
    fn accumulate_mean_energy_integral(&mut self) {
        let world = self.world.get_mut();
        let (_, mean_energy) = current_mean_agent_energy(world);
        self.mean_energy_integral += mean_energy;
        if let Some(mut state) = world.get_resource_mut::<LiveExperimentState>() {
            state.mean_energy_integral = self.mean_energy_integral;
        }
    }

    /// Attach a read-only lockstep turnover recorder (`DriftMode::Record`) to a selected run.
    pub fn install_turnover_recorder(&mut self, duration_ticks: u64) {
        let seed = self.seed;
        let world = self.world.get_mut();
        world.insert_resource(crate::core::drift_control::DriftControlState::new_recorder(
            seed,
            duration_ticks,
        ));
    }

    /// Export the recorded [`crate::core::drift_control::TurnoverSchedule`] from a selected run.
    pub fn take_turnover_schedule(&self) -> Option<crate::core::drift_control::TurnoverSchedule> {
        let world = self.world.borrow();
        let drift = world.get_resource::<crate::core::drift_control::DriftControlState>()?;
        let manifest_fp = world
            .get_resource::<LiveExperimentState>()
            .map(|s| s.manifest_fingerprint)
            .unwrap_or(0);
        Some(drift.export_schedule(self.seed, manifest_fp))
    }

    /// Read the accumulated [`crate::core::drift_control::DriftDiagnostics`] when drift control is active.
    pub fn drift_diagnostics(&self) -> Option<crate::core::drift_control::DriftDiagnostics> {
        let world = self.world.borrow();
        world
            .get_resource::<crate::core::drift_control::DriftControlState>()
            .map(|d| d.diagnostics.clone())
    }

    /// Clone the carried `sim_stream::LIVE_DRIFT_SELECTION` RNG checkpoint for Channon windows.
    pub fn drift_rng_clone(&self) -> Option<crate::core::resources::SimRng> {
        let world = self.world.borrow();
        world
            .get_resource::<crate::core::drift_control::DriftControlState>()
            .map(|d| d.rng_clone())
    }

    /// Construct a replay adapter from a manifest declaring `live.drift_control = 1.0` and a
    /// previously recorded [`crate::core::drift_control::TurnoverSchedule`].
    pub fn from_manifest_with_drift_schedule(
        laws: &WorldLawSet,
        initial: &InitialConditionSet,
        forcings: &[ExoticIntervention],
        seed: u64,
        manifest_fingerprint: u64,
        registry_fingerprint: u64,
        schedule: &crate::core::drift_control::TurnoverSchedule,
    ) -> Result<Self, ExperimentError> {
        if let Some(first) = forcings.first() {
            return Err(ExperimentError::InvalidExoticIntervention {
                id: first.id,
                reason: "the live Bevy world has no exotic-energy field to force".into(),
            });
        }
        let config = LiveWorldConfig::from_initial_conditions(initial)?;
        if !config.drift_control {
            return Err(ExperimentError::InvalidLaw {
                reason: "from_manifest_with_drift_schedule requires live.drift_control = 1.0"
                    .into(),
            });
        }
        if schedule.seed != seed {
            return Err(ExperimentError::InvalidLaw {
                reason: format!(
                    "turnover schedule seed ({}) does not match run seed ({seed})",
                    schedule.seed
                ),
            });
        }
        if schedule.ticks.len() as u64 != schedule.duration_ticks {
            return Err(ExperimentError::InvalidLaw {
                reason: format!(
                    "turnover schedule length ({}) does not match duration_ticks ({})",
                    schedule.ticks.len(),
                    schedule.duration_ticks
                ),
            });
        }
        let mut adapter = Self::build(
            laws,
            initial,
            seed,
            None,
            manifest_fingerprint,
            registry_fingerprint,
        )?;
        adapter.world.get_mut().insert_resource(
            crate::core::drift_control::DriftControlState::new_replay(seed, schedule.ticks.clone()),
        );
        Ok(adapter)
    }

    /// Construct a Channon reset replay window from a `selected` [`LiveSnapshot`] while
    /// preserving the carried `LIVE_DRIFT_SELECTION` RNG stream across windows.
    pub fn from_snapshot_with_drift_schedule(
        snapshot: &LiveSnapshot,
        laws: &WorldLawSet,
        drift_initial: &InitialConditionSet,
        drift_manifest_fingerprint: u64,
        drift_registry_fingerprint: u64,
        schedule: &crate::core::drift_control::TurnoverSchedule,
        drift_rng: crate::core::resources::SimRng,
    ) -> Result<Self, ExperimentError> {
        let Some(saved_state) = snapshot.saved.experiment.clone() else {
            return Err(ExperimentError::InvalidLaw {
                reason:
                    "this snapshot carries no live experiment state and cannot start a Channon \
                         drift reset window"
                        .into(),
            });
        };
        saved_state.check_resumable()?;
        let config = LiveWorldConfig::from_initial_conditions(drift_initial)?;
        if !config.drift_control {
            return Err(ExperimentError::InvalidLaw {
                reason: "from_snapshot_with_drift_schedule requires live.drift_control = 1.0"
                    .into(),
            });
        }
        if schedule.seed != saved_state.seed {
            return Err(ExperimentError::InvalidLaw {
                reason: format!(
                    "turnover window seed ({}) does not match snapshot seed ({})",
                    schedule.seed, saved_state.seed
                ),
            });
        }
        if schedule.ticks.len() as u64 != schedule.duration_ticks {
            return Err(ExperimentError::InvalidLaw {
                reason: format!(
                    "turnover window length ({}) does not match duration_ticks ({})",
                    schedule.ticks.len(),
                    schedule.duration_ticks
                ),
            });
        }
        let mut adapter = Self::build(
            laws,
            drift_initial,
            saved_state.seed,
            Some(&snapshot.saved),
            drift_manifest_fingerprint,
            drift_registry_fingerprint,
        )?;
        let tick_offset = saved_state.clock_tick;
        {
            let world = adapter.world.get_mut();
            if let Some(mut exp_state) = world.get_resource_mut::<LiveExperimentState>() {
                exp_state.manifest_fingerprint = drift_manifest_fingerprint;
                exp_state.registry_fingerprint = drift_registry_fingerprint;
                exp_state.initial_conditions = drift_initial.clone();
            }
            world.insert_resource(
                crate::core::drift_control::DriftControlState::new_replay_with_rng(
                    drift_rng,
                    schedule.ticks.clone(),
                    tick_offset,
                ),
            );
        }
        Ok(adapter)
    }

    fn sync_drift_tick(&mut self) {
        let tick = self.clock.tick;
        let world = self.world.get_mut();
        if let Some(mut drift) =
            world.get_resource_mut::<crate::core::drift_control::DriftControlState>()
        {
            drift.sync_tick(tick);
        }
    }

    fn record_drift_shortfalls_if_due(&mut self, tick: u64, force_flush: bool) {
        let world = self.world.get_mut();
        let Some((first_div, missed_deaths, missed_births, prev_deaths, prev_births)) = world
            .get_resource::<crate::core::drift_control::DriftControlState>()
            .filter(|d| d.is_replay())
            .map(|d| {
                (
                    d.diagnostics.first_divergence_tick,
                    d.diagnostics.missed_deaths,
                    d.diagnostics.missed_births(),
                    d.reported_missed_deaths,
                    d.reported_missed_births,
                )
            })
        else {
            return;
        };
        let due = force_flush || first_div == Some(tick) || tick.is_multiple_of(ECOLOGY_PERIOD);
        if !due {
            return;
        }
        let delta_deaths = missed_deaths.saturating_sub(prev_deaths);
        let delta_births = missed_births.saturating_sub(prev_births);
        if delta_deaths == 0 && delta_births == 0 {
            return;
        }
        if let Some(mut state) = world.get_resource_mut::<LiveExperimentState>() {
            if delta_deaths > 0 {
                state.ledger.record(
                    crate::core::causal::CAUSE_BACKGROUND,
                    None,
                    tick,
                    "live.drift_shortfall_deaths",
                    missed_deaths as f64,
                    delta_deaths as f64,
                    format!(
                        "drift replay clamped {delta_deaths} scheduled death(s) because living \
                         guild population fell short"
                    ),
                );
            }
            if delta_births > 0 {
                state.ledger.record(
                    crate::core::causal::CAUSE_BACKGROUND,
                    None,
                    tick,
                    "live.drift_shortfall_births",
                    missed_births as f64,
                    delta_births as f64,
                    format!(
                        "drift replay skipped {delta_births} scheduled birth(s) due to guild \
                         extinction or insufficient solvent payer reserves"
                    ),
                );
            }
        }
        if let Some(mut drift) =
            world.get_resource_mut::<crate::core::drift_control::DriftControlState>()
        {
            drift.reported_missed_deaths = missed_deaths;
            drift.reported_missed_births = missed_births;
        }
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
        let capture_started = world
            .get_resource_mut::<TickCaptureSink>()
            .and_then(|mut sink| sink.begin_tick());
        let (capturing, started) = match capture_started {
            Some(origin) => (true, origin),
            None => (false, std::time::Instant::now()),
        };
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
        if let Some(counter) = world
            .resource::<crate::core::resources::ScientificCounterFault>()
            .take()
        {
            panic!(
                "live experiment stopped after the final exact tick because the {counter} \
                 reached its terminal value"
            );
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

    /// Observed identity (`seed`, `generator_version`, `width x height`, FNV-1a checksum) of the
    /// `TerrainMap` inside this adapter's ECS `World`.
    pub fn world_identity(&self) -> crate::core::world_artifact::WorldIdentity {
        *self
            .world
            .borrow()
            .resource::<crate::core::world_artifact::WorldIdentity>()
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
        let config = LiveWorldConfig::from_initial_conditions(initial)?;
        if config.drift_control {
            return Err(ExperimentError::InvalidLaw {
                reason: "live.drift_control = 1.0 requires a recorded TurnoverSchedule from the \
                         paired selected run; use LiveExperimentAdapter::from_manifest_with_drift_schedule \
                         or run_live_drift_pair / run_live_drift_ensemble"
                    .into(),
            });
        }
        // The grid is the reference world's demonstration substrate. The live world's field is the
        // terrain's own, at `MapSettings::default()` — 256×256 in this build — and is not resized
        // to match a manifest, because the resource field is the world's, not the experiment's.
        Self::build(laws, initial, seed, None, 0, 0)
    }

    fn snapshot(&self) -> Self::Snapshot {
        let mut world = self.world.borrow_mut();
        // The clock and accumulator live on the adapter and in the resource; the resource is what
        // the serializer reads, so both are refreshed here rather than only at the end of `step`.
        if let Some(mut state) = world.get_resource_mut::<LiveExperimentState>() {
            state.clock_tick = self.clock.tick;
            state.mean_energy_integral = self.mean_energy_integral;
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
        self.sync_drift_tick();
        self.run_tick();
        self.drain_absent_thread_queues();
        self.accumulate_mean_energy_integral();
        self.record_drift_shortfalls_if_due(clock.tick, false);
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

    fn observables_unsampled(&self) -> Vec<(String, f64)> {
        let mut world = self.world.borrow_mut();
        live_observables_peek(&mut world)
    }

    fn terrain_source(&self) -> Option<&'static str> {
        Some("generated")
    }

    fn world_identity(&self) -> Option<crate::core::world_artifact::WorldIdentity> {
        Some(LiveExperimentAdapter::world_identity(self))
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

/// Insert all manifest-declared world-law policy resources onto `world`, replacing any
/// environment-resolved defaults installed by `init_world` or `SimulationEngine::start`.
fn insert_policy_resources(world: &mut World, config: &LiveWorldConfig) {
    world.insert_resource(crate::core::resources::BrainPolicy {
        evolved: config.evolved_brains,
        ..Default::default()
    });
    // Mortality comes from the manifest for the same reason brains do: the app resolves
    // `MortalityPolicy` from `ANIMA_MORTALITY`, and an ambient variable must not decide whether
    // a declared experiment's population can die. Absent key ⇒ disabled ⇒ bit-identical to every
    // run made before branch M existed.
    world.insert_resource(crate::core::resources::MortalityPolicy {
        enabled: config.mortality,
        starve_ticks: config.starve_ticks,
        dehydration_ticks: config.dehydration_ticks,
    });
    // Continuous intra-tick reproduction (`autonomous_reproduction_system`) is a declared input
    // (`live.reproduction`, ADR-0005 option A). Absent/0 keeps the historical disabled policy.
    world.insert_resource(crate::core::resources::ReproductionPolicy {
        enabled: config.reproduction,
    });
    // Offspring body-mutation probability (`live.mutation_rate`); absent == the historical 0.05.
    world.insert_resource(crate::core::resources::MutationPolicy {
        rate: config.mutation_rate,
    });
    // Prey reproduction ceiling (`live.prey_cap`, card S6) and optional strict per-birth
    // counting (`live.strict_prey_cap`, card FIX-CAP / F-052); absent == 80, soft gate.
    world.insert_resource(crate::core::resources::PreyCapPolicy {
        cap: config.prey_cap,
        strict: config.strict_prey_cap,
    });
    // Plant reserve (`live.plant_reserve`, card S10); absent == 0.0 == both grazing paths run the
    // pre-S10 arithmetic. Inserted before the restore branch, so a resumed run keeps its reserve.
    world.insert_resource(crate::core::resources::PlantReservePolicy {
        fraction: config.plant_reserve as f32,
    });
    // The steering drive is a declared input (`live.locomotion`, ADR-0006 option A). Absent/0 is
    // the disabled policy: `steering_drive_system` returns at once and `target_velocity` stays 0.
    world.insert_resource(crate::core::resources::LocomotionPolicy {
        enabled: config.locomotion,
        drive_floor: config.drive_floor,
        thirst_steering: config.thirst_steering,
    });
    world.insert_resource(crate::core::resources::GrassSteeringPolicy {
        enabled: config.grass_steering,
    });
    // Card LAND1 (`live.land_mask`): absent/0 keeps land navigation disabled so pre-LAND1
    // manifests stay bit-identical.
    world.insert_resource(crate::core::resources::TerrainNavigation {
        enabled: config.land_mask,
    });
    // Fitness likewise comes from the manifest, never from `ANIMA_FITNESS`: which rule decides
    // who reproduces is a declared input of the experiment, and absent keys keep the
    // locomotion ranking — bit-identical to every run made before branch F existed.
    world.insert_resource(crate::evolution::fitness::FitnessRuleSetting {
        rule: if config.ecological_fitness {
            crate::evolution::fitness::FitnessRuleId::Ecological
        } else {
            crate::evolution::fitness::FitnessRuleId::Locomotion
        },
        w_surplus: config.w_surplus,
    });
    // Declared food cap and predator-income knobs belong to the manifest, not the save or
    // environment, so both genesis and restore declare them (F-043).
    world.insert_resource(LiveFoodCap(config.food_cap));
    world.insert_resource(crate::core::resources::PredatorIncomePolicy {
        kill_hydration: config.kill_hydration,
        kill_efficiency: config.kill_efficiency,
        target_min_energy: config.target_min_energy,
    });
}

/// Fallible variant of [`apply_live_genesis_and_policies`], returning [`ExperimentError`] when a
/// manifest's genesis layout cannot be satisfied (for example `live.founder_layout = 1 | 2` with
/// `live.land_mask = 1` when walkable cells number zero or fewer than `founders`).
pub fn try_apply_live_genesis_and_policies(
    world: &mut World,
    seed: u64,
    config: &LiveWorldConfig,
    bounds: crate::core::ecs::MapBounds,
) -> Result<(), ExperimentError> {
    world.insert_resource(crate::core::resources::SimRng::from_seed(seed));
    insert_policy_resources(world, config);
    world.insert_resource(crate::core::ecs::FoodSpawnSettings {
        max_food_count: config.food_cap,
        ..Default::default()
    });
    scale_pasture_capacity(world, config.resource_capacity_scale);
    genesis(world, config, bounds, seed)
}

/// Apply a manifest's declared world-law policy resources, `SimRng`, food cap, pasture capacity
/// scale, and deterministic lattice genesis (founders, trees, lakes) to an initialized `World`.
///
/// Shared by [`build_live_world`] (the headless [`LiveExperimentAdapter`]) and
/// [`crate::core::simulation_loop::SimulationEngine::start`] (desktop-app viewer mode, card
/// W2-lite; prototype, to be replaced when ADR-0010 W1 lands) so both start from identical tick-0
/// entities and world-law policies. A desktop viewer run still keeps the app's background threads
/// and therefore does not claim to reproduce a headless experiment's trajectory.
pub fn apply_live_genesis_and_policies(
    world: &mut World,
    seed: u64,
    config: &LiveWorldConfig,
    bounds: crate::core::ecs::MapBounds,
) {
    try_apply_live_genesis_and_policies(world, seed, config, bounds)
        .unwrap_or_else(|e| panic!("{e}"));
}

/// Build the world the schedule runs on with `TerrainSource::Generated`, and replace every other
/// process-environment default with what the manifest declared.
fn build_live_world(
    seed: u64,
    config: &LiveWorldConfig,
    restore: Option<&SavedSimulationState>,
) -> Result<World, ExperimentError> {
    let mut world = crate::core::ecs::init_world_with(crate::core::ecs::TerrainSource::Generated);

    // `init_world` resolves these from the environment, which is exactly what a manifest cannot
    // tolerate: `ANIMA_SIM_SEED`, `ANIMA_EVOLVED_BRAINS`, `ANIMA_LIFETIME_LEARNING`,
    // `ANIMA_WORLD_ARTIFACT`, `ANIMA_CACHE_DIR` and `<temp_dir>/anima_world_artifact.anmw` would
    // each silently change the trajectory of a run whose whole point is that its inputs are
    // declared. Terrain is built via `TerrainSource::Generated` (`MapSettings::default()`) above.
    //
    // `evolved` comes from the manifest rather than the environment, and the other two fields stay
    // at their defaults on purpose: `lifetime_learning` and `brain_metabolic_cost` are separate
    // flags answering separate questions, and folding them in here would turn one declared factor
    // into a bundle no result could attribute.
    let bounds = restore.map(|s| s.map_bounds).unwrap_or_default();
    world.insert_resource(bounds);
    world.insert_resource(crate::physics::SpatialHashGrid::new_prepopulated(
        10.0, &bounds,
    ));

    match restore {
        Some(state) => {
            let sim_rng = if state.sim_rng_seed != 0 || state.sim_rng_pos != 0 {
                crate::core::resources::SimRng::restore(state.sim_rng_seed, state.sim_rng_pos)
            } else {
                crate::core::resources::SimRng::from_seed(seed)
            };
            world.insert_resource(sim_rng);
            insert_policy_resources(&mut world, config);

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
            // The save carries the standing crop `r` but not the capacity `r_max`, which
            // `init_world` derives from the biomes. Re-apply the declared factor to the capacity
            // only: the crop is the saved one, and the EU moved to detritus at genesis stays moved.
            scale_pasture_capacity_only(&mut world, config.resource_capacity_scale);
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
            // `sensory_system` runs at the start of the tick, before `rebuild_spatial_grid_system`.
            // In an uninterrupted run that has already stepped at least one tick, `SpatialHashGrid`
            // holds the end-of-tick-K collider positions when tick K+1 begins; restore the exact
            // `IndexedInGrid` boundary and populate `SpatialHashGrid` only for already-indexed
            // colliders (when `tick_count > 0 || clock_tick > 0`), leaving post-rebuild spawns
            // unindexed until tick K+1's rebuild (F-045).
            populate_spatial_grid(&mut world, state);
        }
        None => {
            world.insert_resource(crate::ai::pheromone::PheromoneGrid::default());
            world.insert_resource(crate::core::ecs::EpochManager {
                ticks_per_epoch: 1000,
                current_epoch_ticks: 0,
                current_epoch: 0,
            });
            world.insert_resource(crate::core::ecs::ActiveEnvironmentEvent::default());
            try_apply_live_genesis_and_policies(&mut world, seed, config, bounds)?;
        }
    }

    Ok(world)
}

/// `live.resource_capacity_scale` (card S6): scale the pasture's capacity and standing crop once, at
/// genesis, and move the EU taken out of it to detritus so the closed-EU total does not change. A
/// factor of exactly `1.0` (the absent key) touches nothing, so the default world is bit-identical.
fn scale_pasture_capacity(world: &mut World, factor: f64) {
    if factor == 1.0 {
        return;
    }
    let removed = {
        let Some(mut field) = world.get_resource_mut::<ResourceField>() else {
            return;
        };
        let before = field.total_biomass();
        let f = factor as f32;
        for v in field.r_max.iter_mut() {
            *v *= f;
        }
        for v in field.r.iter_mut() {
            *v *= f;
        }
        before - field.total_biomass()
    };
    let plants_now = world
        .get_resource::<ResourceField>()
        .map(|f| f.total_biomass())
        .unwrap_or(0.0);
    if let Some(mut pool) = world.get_resource_mut::<EcosystemBiomass>() {
        pool.plants = plants_now;
        pool.detritus += removed;
    }
}

/// The restore half of `live.resource_capacity_scale`: `r_max` only (see [`scale_pasture_capacity`]).
fn scale_pasture_capacity_only(world: &mut World, factor: f64) {
    if factor == 1.0 {
        return;
    }
    if let Some(mut field) = world.get_resource_mut::<ResourceField>() {
        let f = factor as f32;
        for v in field.r_max.iter_mut() {
            *v *= f;
        }
    }
}

fn populate_spatial_grid(world: &mut World, state: &SavedSimulationState) {
    // Sync every collider's `IndexedInGrid` flag and run `rebuild_spatial_grid_system` (filtering
    // out post-rebuild spawns recorded in `state.unindexed_colliders`) when `tick > 0` so tick
    // K+1 sees the exact same grid contents as an uninterrupted run (F-045).
    crate::core::simulation_state::populate_restored_spatial_grid(world, state);
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
) -> Result<(), ExperimentError> {
    use crate::core::agent_systems::{
        AgentEvaluation, AgentGeneration, AgentGenotype, AgentLineageId,
    };
    use crate::core::components::{AgentParentLineageIds, FeatureTracker, Predator, Prey};
    use crate::evolution::genotype::{
        decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
    };

    let terrain_map = if config.land_mask {
        world
            .get_resource::<crate::core::terrain::TerrainMap>()
            .cloned()
    } else {
        None
    };

    if config.founders > 0
        && config.land_mask
        && (config.founder_layout == FOUNDER_LAYOUT_GRID
            || config.founder_layout == FOUNDER_LAYOUT_SCATTER)
    {
        let walkable_cells = terrain_map.as_ref().map(walkable_cell_count).unwrap_or(0);
        if walkable_cells == 0 {
            return Err(ExperimentError::InvalidPopulation {
                reason: format!(
                    "live.founder_layout={} with land_mask=1 cannot place {} founder(s): terrain has 0 walkable cells (no walkable land)",
                    config.founder_layout, config.founders
                ),
            });
        }
        if walkable_cells < config.founders {
            return Err(ExperimentError::InvalidPopulation {
                reason: format!(
                    "live.founder_layout={} with land_mask=1 cannot place {} founder(s) without stacking: terrain has only {} distinct walkable cell(s)",
                    config.founder_layout, config.founders, walkable_cells
                ),
            });
        }
    }

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

    let snap_if_masked = |pos: glam::Vec3| -> glam::Vec3 {
        if let Some(ref tm) = terrain_map {
            crate::core::terrain_navigation::snap_to_nearest_walkable(tm, &bounds, pos)
                .unwrap_or(pos)
        } else {
            pos
        }
    };

    let predators = config.predators();
    let custom_founder_positions = match config.founder_layout {
        FOUNDER_LAYOUT_GRID => Some(founder_grid_positions(
            config.founders,
            bounds,
            terrain_map.as_ref(),
        )),
        FOUNDER_LAYOUT_SCATTER => Some(founder_scatter_positions(
            config.founders,
            bounds,
            terrain_map.as_ref(),
            seed,
        )),
        _ => None,
    };
    for i in 0..config.founders {
        let pos = match custom_founder_positions {
            Some(ref positions) => positions[i],
            None => snap_if_masked(lattice_point(i, config.founders.max(1), bounds, 0.35)),
        };
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
                total_energy_gained: 0.0,
                prey_energy_gained: 0.0,
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
        let pos = snap_if_masked(lattice_point(i, config.trees.max(1), bounds, 0.6));
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
            crate::physics::spatial::IndexedInGrid(false),
        ));
    }
    for i in 0..config.lakes {
        let pos = snap_if_masked(if config.lake_layout == 1 {
            lake_lattice_point(i, config.lakes, bounds)
        } else {
            lattice_point(i, config.lakes.max(1), bounds, 0.8)
        });
        world.spawn((
            crate::core::ecs::Lake {
                current_water: env.default_lake_water,
                max_water: env.default_lake_water,
                replenishment_rate: env.default_lake_replenish,
            },
            crate::core::ecs::Position(pos),
            crate::physics::SpatialCollider { radius: 10.0 },
            crate::physics::spatial::IndexedInGrid(false),
        ));
    }

    Ok(())
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

/// Places lake `i` of `n` (`n >= 1`) on a 2-D lattice over `bounds` (`live.lake_layout = 1`):
/// `k = ceil(sqrt(n))` columns, `m = ceil(n / k)` rows; lake `i` goes to column `c = i % k`,
/// row `r = i / k`, at `x = min.x + (c + 1) / (k + 1) * span_x` and
/// `z = min.z + (r + 1) / (m + 1) * span_z` (`y = 0`). If the last row (`r == m - 1`) is partial
/// with `j = n - (m - 1) * k` lakes, it is centred using x fractions `(c + 1) / (j + 1)`.
fn lake_lattice_point(i: usize, n: usize, bounds: crate::core::ecs::MapBounds) -> glam::Vec3 {
    debug_assert!(n >= 1 && i < n);
    let k = (1..=n)
        .find(|&k| k.checked_mul(k).is_some_and(|sq| sq >= n))
        .unwrap_or(1);
    let m = n.div_ceil(k);
    let c = i % k;
    let r = i / k;
    let cols_in_row = if r + 1 == m { n - (m - 1) * k } else { k };
    let span_x = bounds.max.x - bounds.min.x;
    let span_z = bounds.max.z - bounds.min.z;
    let x_frac = (c as f32 + 1.0) / (cols_in_row as f32 + 1.0);
    let z_frac = (r as f32 + 1.0) / (m as f32 + 1.0);
    glam::Vec3::new(
        bounds.min.x + x_frac * span_x,
        0.0,
        bounds.min.z + z_frac * span_z,
    )
}

fn toroidal_dist_sq(a: glam::Vec3, b: glam::Vec3, bounds: &crate::core::ecs::MapBounds) -> f32 {
    let span_x = bounds.max.x - bounds.min.x;
    let span_z = bounds.max.z - bounds.min.z;
    let mut dx = (a.x - b.x).abs();
    if span_x > 0.0 {
        dx = dx.rem_euclid(span_x);
        dx = dx.min(span_x - dx);
    }
    let mut dz = (a.z - b.z).abs();
    if span_z > 0.0 {
        dz = dz.rem_euclid(span_z);
        dz = dz.min(span_z - dz);
    }
    dx * dx + dz * dz
}

fn unmasked_grid_positions(n: usize, bounds: crate::core::ecs::MapBounds) -> Vec<glam::Vec3> {
    if n == 0 {
        return Vec::new();
    }
    let k = (1..=n)
        .find(|&k| k.checked_mul(k).is_some_and(|sq| sq >= n))
        .unwrap_or(1);
    let m = n.div_ceil(k);
    let span_x = bounds.max.x - bounds.min.x;
    let span_z = bounds.max.z - bounds.min.z;
    (0..n)
        .map(|i| {
            let c = i % k;
            let r = i / k;
            let cols_in_row = if r + 1 == m { n - (m - 1) * k } else { k };
            let x_frac = (c as f32 + 0.5) / cols_in_row as f32;
            let z_frac = (r as f32 + 0.5) / m as f32;
            glam::Vec3::new(
                bounds.min.x + x_frac * span_x,
                0.0,
                bounds.min.z + z_frac * span_z,
            )
        })
        .collect()
}

fn walkable_cell_count(tm: &crate::core::terrain::TerrainMap) -> usize {
    let total = tm.width.saturating_mul(tm.height);
    if total == 0 || tm.biomes.len() < total {
        return 0;
    }
    tm.biomes
        .iter()
        .take(total)
        .filter(|&&b| crate::core::terrain_navigation::is_biome_walkable(b))
        .count()
}

fn walkable_cell_centers(
    tm: &crate::core::terrain::TerrainMap,
    bounds: &crate::core::ecs::MapBounds,
) -> Vec<glam::Vec3> {
    let total = tm.width * tm.height;
    let mut out = Vec::new();
    if tm.width == 0 || tm.height == 0 || tm.biomes.len() < total {
        return out;
    }
    for row in 0..tm.height {
        for col in 0..tm.width {
            if crate::core::terrain_navigation::is_biome_walkable(tm.biomes[row * tm.width + col]) {
                out.push(crate::core::terrain_navigation::cell_center(
                    tm, bounds, col, row,
                ));
            }
        }
    }
    out
}

fn greedy_fill_from_candidates(
    candidates: &[glam::Vec3],
    n: usize,
    bounds: &crate::core::ecs::MapBounds,
    placed: &mut Vec<glam::Vec3>,
) {
    if candidates.is_empty() || placed.len() >= n {
        return;
    }
    let mut used = vec![false; candidates.len()];
    for &scale in &[1.0_f32, 0.75, 0.5625, 0.421875, 0.3164, 0.001] {
        let min_d = FOUNDER_MIN_SPACING * scale;
        let min_sq = min_d * min_d;
        for (idx, &cand) in candidates.iter().enumerate() {
            if placed.len() >= n {
                return;
            }
            if used[idx] {
                continue;
            }
            if placed
                .iter()
                .all(|&p| toroidal_dist_sq(cand, p, bounds) >= min_sq)
            {
                placed.push(cand);
                used[idx] = true;
            }
        }
    }
    while placed.len() < n {
        let base_len = placed.len().max(1);
        for (idx, &cand) in candidates.iter().enumerate() {
            if placed.len() >= n {
                return;
            }
            if !used[idx] && placed.iter().all(|&p| p != cand) {
                placed.push(cand);
                used[idx] = true;
            }
        }
        if placed.len() == base_len {
            break;
        }
    }
    let seed_len = placed.len();
    for i in seed_len..n {
        placed.push(placed[i % seed_len]);
    }
}

fn founder_grid_positions(
    n: usize,
    bounds: crate::core::ecs::MapBounds,
    terrain_map: Option<&crate::core::terrain::TerrainMap>,
) -> Vec<glam::Vec3> {
    if n == 0 {
        return Vec::new();
    }
    let Some(tm) = terrain_map else {
        return unmasked_grid_positions(n, bounds);
    };
    let total = tm.width.saturating_mul(tm.height);
    let walkable_count = walkable_cell_count(tm);
    if walkable_count == 0 {
        return Vec::new();
    }

    let span_x = bounds.max.x - bounds.min.x;
    let span_z = bounds.max.z - bounds.min.z;
    if span_x <= 0.0 || span_z <= 0.0 {
        return Vec::new();
    }
    let min_span = span_x.min(span_z);
    let safe_step = FOUNDER_MIN_SPACING * 1.0001;
    let gx_max = if span_x >= safe_step {
        (span_x / safe_step).floor() as usize
    } else {
        1
    }
    .max(1)
    .min(tm.width);
    let gz_max = if span_z >= safe_step {
        (span_z / safe_step).floor() as usize
    } else {
        1
    }
    .max(1)
    .min(tm.height);

    let land_frac = (walkable_count as f64) / (total as f64);
    let ratio = (span_x.max(span_z) / min_span) as f64;
    let g_start = (((n as f64) / (land_frac * ratio).max(1e-6)).sqrt().floor() as usize)
        .saturating_sub(1)
        .max(1);
    let g_cap = gx_max.max(gz_max);

    for g in g_start..=g_cap {
        let gx = ((g as f32) * (span_x / min_span))
            .round()
            .clamp(1.0, gx_max as f32) as usize;
        let gz = ((g as f32) * (span_z / min_span))
            .round()
            .clamp(1.0, gz_max as f32) as usize;
        let mut candidates = Vec::new();
        for r in 0..gz {
            let z = bounds.min.z + ((r as f32 + 0.5) / gz as f32) * span_z;
            for c in 0..gx {
                let x = bounds.min.x + ((c as f32 + 0.5) / gx as f32) * span_x;
                let pos = glam::Vec3::new(x, 0.0, z);
                if crate::core::terrain_navigation::is_pos_walkable(tm, &bounds, pos) {
                    candidates.push(pos);
                }
            }
        }
        if candidates.len() >= n {
            let m = candidates.len();
            return (0..n).map(|i| candidates[(i * m + m / 2) / n]).collect();
        }
        if gx == gx_max && gz == gz_max {
            break;
        }
    }

    let centers = walkable_cell_centers(tm, &bounds);
    let mut placed = Vec::with_capacity(n);
    greedy_fill_from_candidates(&centers, n, &bounds, &mut placed);
    placed
}

fn sample_bound_point<R: rand::Rng + ?Sized>(
    rng: &mut R,
    bounds: &crate::core::ecs::MapBounds,
) -> glam::Vec3 {
    let x = if bounds.max.x > bounds.min.x {
        rng.gen_range(bounds.min.x..bounds.max.x)
    } else {
        bounds.min.x
    };
    let z = if bounds.max.z > bounds.min.z {
        rng.gen_range(bounds.min.z..bounds.max.z)
    } else {
        bounds.min.z
    };
    glam::Vec3::new(x, 0.0, z)
}

fn founder_scatter_positions(
    n: usize,
    bounds: crate::core::ecs::MapBounds,
    terrain_map: Option<&crate::core::terrain::TerrainMap>,
    seed: u64,
) -> Vec<glam::Vec3> {
    use rand::Rng;

    if n == 0 {
        return Vec::new();
    }
    let mut rng = crate::core::resources::derived_rng(
        seed,
        crate::core::resources::sim_stream::LIVE_GENESIS_FOUNDERS,
    );
    let min_sq = FOUNDER_MIN_SPACING * FOUNDER_MIN_SPACING;
    const MAX_SCATTER_TRIES: usize = 256;

    let mut placed = Vec::with_capacity(n);
    for _ in 0..n {
        let mut chosen = None;
        for _ in 0..MAX_SCATTER_TRIES {
            let cand = sample_bound_point(&mut rng, &bounds);
            let walkable = terrain_map.is_none_or(|tm| {
                crate::core::terrain_navigation::is_pos_walkable(tm, &bounds, cand)
            });
            if walkable
                && placed
                    .iter()
                    .all(|&p| toroidal_dist_sq(cand, p, &bounds) >= min_sq)
            {
                chosen = Some(cand);
                break;
            }
        }
        let Some(pos) = chosen else {
            break;
        };
        placed.push(pos);
    }

    if placed.len() == n {
        return placed;
    }

    if let Some(tm) = terrain_map {
        let mut centers = walkable_cell_centers(tm, &bounds);
        if !centers.is_empty() {
            for i in (1..centers.len()).rev() {
                let j = rng.gen_range(0..=i);
                centers.swap(i, j);
            }
            greedy_fill_from_candidates(&centers, n, &bounds, &mut placed);
            return placed;
        }
    }

    for &scale in &[0.75_f32, 0.5625, 0.421875, 0.3164, 0.0] {
        let relax_sq = (FOUNDER_MIN_SPACING * scale).powi(2);
        while placed.len() < n {
            let mut chosen = None;
            for _ in 0..MAX_SCATTER_TRIES {
                let cand = sample_bound_point(&mut rng, &bounds);
                if scale == 0.0
                    || placed
                        .iter()
                        .all(|&p| toroidal_dist_sq(cand, p, &bounds) >= relax_sq)
                {
                    chosen = Some(cand);
                    break;
                }
            }
            match chosen {
                Some(pos) => placed.push(pos),
                None => break,
            }
        }
    }
    while placed.len() < n {
        placed.push(sample_bound_point(&mut rng, &bounds));
    }
    placed
}

/// Insert every resource the live schedule's systems require but `init_world` does not build,
/// creating the channels the app's threads would otherwise own.
fn install_runtime_resources(
    world: &mut World,
    seed: u64,
    config: &LiveWorldConfig,
    restore: Option<&SavedSimulationState>,
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
    // Branch-O turnover totals; the observable layer drains windows on sample.
    world.insert_resource(crate::core::resources::VitalStatistics::default());

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
    let (migration_tx, migration_rx) = crate::core::resources::manual_migration_channel();
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
    let next_node_id = restore
        .map(crate::core::simulation_state::restored_next_node_id)
        .unwrap_or(3);
    world.insert_resource(NextNodeId(next_node_id));
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

// ---- Paired neutral-drift control runner (card SEL2-D) ------------------------------------------

/// Outcome of one same-seed `selected` vs neutral-`drift` pair (card SEL2-D).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriftPairReport {
    pub seed: u64,
    pub selected: crate::core::experiment_runner::RunResult,
    pub drift: crate::core::experiment_runner::RunResult,
    pub schedule: crate::core::drift_control::TurnoverSchedule,
    pub diagnostics: crate::core::drift_control::DriftDiagnostics,
    pub reset_period_ticks: u64,
    pub window_count: usize,
    /// Net selection delta summed over Channon windows:
    /// `sum_w [(z_sel(wK) - z_sel((w-1)K)) - (z_drift_w(wK) - z_drift_w((w-1)K))]`.
    /// When `reset_period_ticks == 0`, equals `z_sel(T) - z_drift(T)` across one window.
    #[serde(default)]
    pub window_net_selection_deltas: std::collections::BTreeMap<String, f64>,
}

/// Paired ensemble outcome across all seeds of a `selected` / neutral-`drift` experiment
/// (`control` = neutral drift, `treatment` = selected so positive deltas measure selection excess).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriftEnsembleReport {
    pub ensemble: crate::core::experiment_runner::PairedEnsembleReport,
    pub schedules: Vec<crate::core::drift_control::TurnoverSchedule>,
    pub diagnostics: Vec<crate::core::drift_control::DriftDiagnostics>,
    pub reset_period_ticks: u64,
    pub window_net_selection_by_seed: Vec<(u64, std::collections::BTreeMap<String, f64>)>,
}

fn live_run_provenance(
    manifest: &crate::core::experiment::ExperimentManifest,
    registry: &crate::core::experiment::ObservableRegistry,
    seed: u64,
    world_identity: crate::core::world_artifact::WorldIdentity,
) -> crate::core::experiment_runner::RunProvenance {
    use crate::core::experiment::fnv1a_64;
    let manifest_fingerprint = manifest.fingerprint();
    let registry_fingerprint = registry.fingerprint();
    let model_version = LIVE_MODEL_VERSION.to_string();
    let build_id = crate::core::experiment_runner::build_id();
    let run_id = format!(
        "{}#{manifest_fingerprint:016x}#r{registry_fingerprint:016x}#m{:016x}#b{:016x}#s{seed}",
        manifest.experiment_id,
        fnv1a_64(model_version.as_bytes()),
        fnv1a_64(build_id.as_bytes())
    );
    crate::core::experiment_runner::RunProvenance {
        experiment_id: manifest.experiment_id.clone(),
        run_id,
        parent_run_id: None,
        fork_tick: None,
        seed,
        manifest_fingerprint,
        law_fingerprint: manifest.laws.fingerprint(),
        registry_fingerprint,
        model_version,
        build_id,
        fitness_rule: crate::core::experiment_runner::declared_fitness_rule(
            &manifest.initial_conditions,
        ),
        terrain_source: Some("generated".to_owned()),
        world_identity: Some(world_identity),
    }
}

fn specs_for_live_emitted(
    registry: &crate::core::experiment::ObservableRegistry,
    series: &[crate::core::scenario::StateSample],
    final_observables: &[(String, f64)],
) -> (Vec<crate::core::experiment::ObservableSpec>, Vec<String>) {
    let mut specs = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
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

fn duplicate_live_observable(observables: &[(String, f64)]) -> Option<&str> {
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

fn drive_live_window(
    model: &mut LiveExperimentAdapter,
    rng: &mut StdRng,
    clock: &mut SimClock,
    ledger: &mut CausalLedger,
    queue: &crate::core::intervention::InterventionQueue,
    window_ticks: u64,
    sample_period: u64,
    series: &mut Vec<crate::core::scenario::StateSample>,
) -> Option<(u64, String)> {
    for _ in 0..window_ticks {
        let tick = clock.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        model.step(clock, &active, ledger, rng);
        let sampled = sample_period != 0 && tick.is_multiple_of(sample_period);
        let observables = if sampled {
            model.observables()
        } else {
            model.observables_unsampled()
        };
        if let Some((name, _)) = observables.iter().find(|(_, value)| !value.is_finite()) {
            return Some((tick, format!("observable '{name}' became non-finite")));
        }
        if let Some(name) = duplicate_live_observable(&observables) {
            return Some((
                tick,
                format!("observable '{name}' was emitted more than once"),
            ));
        }
        if sampled {
            series.push(crate::core::scenario::StateSample { tick, observables });
        }
    }
    None
}

fn assemble_live_run_result(
    provenance: crate::core::experiment_runner::RunProvenance,
    model: &LiveExperimentAdapter,
    registry: &crate::core::experiment::ObservableRegistry,
    series: Vec<crate::core::scenario::StateSample>,
    ledger: CausalLedger,
    failure: Option<(u64, String)>,
    extra_warnings: Vec<String>,
) -> crate::core::experiment_runner::RunResult {
    let final_observables = model.observables();
    let final_checksum = model.checksum();
    let (observable_specs, mut warnings) =
        specs_for_live_emitted(registry, &series, &final_observables);
    warnings.extend(extra_warnings);
    let status = match failure {
        Some((tick, reason)) => {
            warnings.push(reason.clone());
            crate::core::experiment_runner::RunStatus::Failed {
                tick,
                reason,
                checksum: final_checksum,
            }
        }
        None => crate::core::experiment_runner::RunStatus::Completed,
    };
    crate::core::experiment_runner::RunResult {
        provenance,
        status,
        final_checksum,
        final_observables,
        series,
        ledger,
        exotic_budget: None,
        observable_specs,
        warnings,
    }
}

fn validate_drift_pair_manifests(
    selected_manifest: &crate::core::experiment::ExperimentManifest,
    drift_manifest: &crate::core::experiment::ExperimentManifest,
    registry: &crate::core::experiment::ObservableRegistry,
) -> Result<(Vec<String>, LiveWorldConfig, LiveWorldConfig), ExperimentError> {
    registry.validate()?;
    selected_manifest.validate(registry)?;
    drift_manifest.validate(registry)?;
    let declared_factors = crate::core::experiment::FactorDiff {
        allowed_paths: vec!["initial_conditions".into()],
    }
    .validate(selected_manifest, drift_manifest)?;

    // Ensure initial_conditions differ ONLY on `{live.drift_control, live.drift_reset_ticks}`.
    let is_drift_key = |k: &str| k == LIVE_KEY_DRIFT_CONTROL || k == LIVE_KEY_DRIFT_RESET_TICKS;
    let collect_non_drift =
        |init: &InitialConditionSet| -> std::collections::BTreeMap<String, u64> {
            init.values
                .iter()
                .filter(|(k, _)| !is_drift_key(k))
                .map(|(k, v)| (k.clone(), v.to_bits()))
                .collect()
        };
    let sel_other = collect_non_drift(&selected_manifest.initial_conditions);
    let drf_other = collect_non_drift(&drift_manifest.initial_conditions);
    if sel_other != drf_other {
        for k in sel_other.keys().chain(drf_other.keys()) {
            if sel_other.get(k) != drf_other.get(k) {
                return Err(ExperimentError::UndeclaredFactorDifference {
                    path: format!("initial_conditions.{k}"),
                });
            }
        }
    }

    let sel_cfg = LiveWorldConfig::from_initial_conditions(&selected_manifest.initial_conditions)?;
    let drf_cfg = LiveWorldConfig::from_initial_conditions(&drift_manifest.initial_conditions)?;
    if sel_cfg.drift_control {
        return Err(ExperimentError::InvalidLaw {
            reason: "selected_manifest must have live.drift_control absent or 0.0".into(),
        });
    }
    if !drf_cfg.drift_control {
        return Err(ExperimentError::InvalidLaw {
            reason: "drift_manifest must declare live.drift_control = 1.0".into(),
        });
    }
    sel_cfg.ensure_observables_supported(&selected_manifest.observable_ids)?;
    drf_cfg.ensure_observables_supported(&drift_manifest.observable_ids)?;
    if drf_cfg.drift_reset_ticks > 0
        && !drift_manifest
            .duration_ticks
            .is_multiple_of(drf_cfg.drift_reset_ticks)
    {
        return Err(ExperimentError::InvalidLaw {
            reason: format!(
                "live.drift_reset_ticks ({}) must divide duration_ticks ({})",
                drf_cfg.drift_reset_ticks, drift_manifest.duration_ticks
            ),
        });
    }
    Ok((declared_factors, sel_cfg, drf_cfg))
}

fn accumulate_window_deltas(
    accum: &mut std::collections::BTreeMap<String, f64>,
    sel_start: &[(String, f64)],
    sel_end: &[(String, f64)],
    drf_start: &[(String, f64)],
    drf_end: &[(String, f64)],
) {
    let find = |slice: &[(String, f64)], key: &str| -> Option<f64> {
        slice.iter().find(|(k, _)| k == key).map(|(_, v)| *v)
    };
    for (name, end_drf) in drf_end {
        if let (Some(start_sel), Some(end_sel), Some(start_drf)) = (
            find(sel_start, name),
            find(sel_end, name),
            find(drf_start, name),
        ) {
            let delta = (end_sel - start_sel) - (*end_drf - start_drf);
            *accum.entry(name.clone()).or_insert(0.0) += delta;
        }
    }
}

fn shortfall_warning(diag: &crate::core::drift_control::DriftDiagnostics) -> Option<String> {
    if !diag.has_shortfall() {
        return None;
    }
    let capped_suffix = if diag.missed_births_capped > 0 {
        format!(", capped={}", diag.missed_births_capped)
    } else {
        String::new()
    };
    Some(format!(
        "neutral-drift schedule shortfall: missed_deaths={} (ticks={}), missed_births={} \
         (extinct={}, insolvent={}{}, ticks={}), first_divergence_tick={:?}",
        diag.missed_deaths,
        diag.ticks_with_missed_deaths,
        diag.missed_births(),
        diag.missed_births_extinct,
        diag.missed_births_insolvent,
        capped_suffix,
        diag.ticks_with_missed_births,
        diag.first_divergence_tick,
    ))
}

/// Run a lockstep `selected` -> neutral-`drift` pair for one seed (supporting both `K = 0`
/// unreset replay from founders and `K > 0` rolling Channon windows via snapshots).
pub fn run_live_drift_pair(
    selected_manifest: &crate::core::experiment::ExperimentManifest,
    drift_manifest: &crate::core::experiment::ExperimentManifest,
    registry: &crate::core::experiment::ObservableRegistry,
    seed: u64,
) -> Result<DriftPairReport, ExperimentError> {
    use rand::SeedableRng;

    let (_declared, _sel_cfg, drf_cfg) =
        validate_drift_pair_manifests(selected_manifest, drift_manifest, registry)?;
    if !selected_manifest.seeds.contains(&seed) || !drift_manifest.seeds.contains(&seed) {
        return Err(ExperimentError::SeedNotInManifest { seed });
    }

    let total_ticks = selected_manifest.duration_ticks;
    let reset_period_ticks = drf_cfg.drift_reset_ticks;
    let window_ticks = if reset_period_ticks == 0 {
        total_ticks
    } else {
        reset_period_ticks
    };
    let window_count = (total_ticks / window_ticks) as usize;

    let mut selected_adapter = LiveExperimentAdapter::build(
        &selected_manifest.laws,
        &selected_manifest.initial_conditions,
        seed,
        None,
        selected_manifest.fingerprint(),
        registry.fingerprint(),
    )?;
    selected_adapter.install_turnover_recorder(total_ticks);

    let world_identity = selected_adapter.world_identity();
    let sel_prov = live_run_provenance(selected_manifest, registry, seed, world_identity);
    let drf_prov = live_run_provenance(drift_manifest, registry, seed, world_identity);
    let sel_queue =
        crate::core::intervention::InterventionQueue::new(selected_manifest.interventions.clone());
    let drf_queue =
        crate::core::intervention::InterventionQueue::new(drift_manifest.interventions.clone());
    let mut sel_rng = StdRng::seed_from_u64(seed);
    let mut sel_clock = SimClock::new();
    let mut sel_ledger = CausalLedger::new();
    let mut sel_series = Vec::new();
    let mut sel_failure: Option<(u64, String)> = None;

    let mut carried_drift_rng = crate::core::resources::derived_sim_rng(
        seed,
        crate::core::resources::sim_stream::LIVE_DRIFT_SELECTION,
    );
    let mut merged_diag = crate::core::drift_control::DriftDiagnostics::default();
    let mut drf_series = Vec::new();
    let mut drf_ledger = CausalLedger::new();
    let mut drf_failure: Option<(u64, String)> = None;
    let mut window_net_selection_deltas = std::collections::BTreeMap::new();
    let mut last_drift_adapter: Option<LiveExperimentAdapter> = None;

    for w in 0..window_count {
        let w_start = (w as u64) * window_ticks;
        let w_end = w_start + window_ticks;

        let snap = if w > 0 && sel_failure.is_none() {
            Some(selected_adapter.snapshot())
        } else {
            None
        };
        let z_sel_start = selected_adapter.observables_unsampled();

        if sel_failure.is_none() {
            sel_failure = drive_live_window(
                &mut selected_adapter,
                &mut sel_rng,
                &mut sel_clock,
                &mut sel_ledger,
                &sel_queue,
                window_ticks,
                selected_manifest.sample_period,
                &mut sel_series,
            );
        }
        let z_sel_end = selected_adapter.observables_unsampled();

        if sel_failure.is_some() {
            break;
        }

        let full_schedule_so_far = selected_adapter
            .take_turnover_schedule()
            .expect("recorder is installed on selected_adapter");
        let win_schedule = full_schedule_so_far.slice_window(w_start as usize, w_end as usize);

        let mut drf_adapter = match snap {
            None => LiveExperimentAdapter::from_manifest_with_drift_schedule(
                &drift_manifest.laws,
                &drift_manifest.initial_conditions,
                &drift_manifest.exotic_interventions,
                seed,
                drift_manifest.fingerprint(),
                registry.fingerprint(),
                &win_schedule,
            )?,
            Some(ref s) => LiveExperimentAdapter::from_snapshot_with_drift_schedule(
                s,
                &drift_manifest.laws,
                &drift_manifest.initial_conditions,
                drift_manifest.fingerprint(),
                registry.fingerprint(),
                &win_schedule,
                carried_drift_rng,
            )?,
        };

        let z_drf_start = drf_adapter.observables_unsampled();
        let mut drf_rng = StdRng::seed_from_u64(seed);
        let mut drf_clock = SimClock { tick: w_start };
        if drf_failure.is_none() {
            drf_failure = drive_live_window(
                &mut drf_adapter,
                &mut drf_rng,
                &mut drf_clock,
                &mut drf_ledger,
                &drf_queue,
                window_ticks,
                drift_manifest.sample_period,
                &mut drf_series,
            );
        }
        drf_adapter.record_drift_shortfalls_if_due(w_end, true);
        drf_adapter.mirror_ledger(&mut drf_ledger);

        let z_drf_end = drf_adapter.observables_unsampled();
        accumulate_window_deltas(
            &mut window_net_selection_deltas,
            &z_sel_start,
            &z_sel_end,
            &z_drf_start,
            &z_drf_end,
        );

        carried_drift_rng = drf_adapter
            .drift_rng_clone()
            .expect("drift RNG present on drift adapter");
        if let Some(diag) = drf_adapter.drift_diagnostics() {
            merged_diag.merge_window(&diag);
        }
        last_drift_adapter = Some(drf_adapter);
        if drf_failure.is_some() {
            break;
        }
    }

    let schedule = selected_adapter
        .take_turnover_schedule()
        .expect("recorder is installed on selected_adapter");
    let selected_result = assemble_live_run_result(
        sel_prov,
        &selected_adapter,
        registry,
        sel_series,
        sel_ledger,
        sel_failure,
        Vec::new(),
    );

    let extra_warnings: Vec<String> = shortfall_warning(&merged_diag).into_iter().collect();
    let drift_result = match last_drift_adapter {
        Some(ref adapter) => assemble_live_run_result(
            drf_prov,
            adapter,
            registry,
            drf_series,
            drf_ledger,
            drf_failure,
            extra_warnings,
        ),
        None => crate::core::experiment_runner::RunResult {
            provenance: drf_prov,
            status: crate::core::experiment_runner::RunStatus::Failed {
                tick: 0,
                reason: "selected run failed before completing its first window".into(),
                checksum: 0,
            },
            final_checksum: 0,
            final_observables: Vec::new(),
            series: Vec::new(),
            ledger: drf_ledger,
            exotic_budget: None,
            observable_specs: Vec::new(),
            warnings: vec!["selected run aborted before drift window started".into()],
        },
    };

    Ok(DriftPairReport {
        seed,
        selected: selected_result,
        drift: drift_result,
        schedule,
        diagnostics: merged_diag,
        reset_period_ticks,
        window_count,
        window_net_selection_deltas,
    })
}

/// Run a multi-seed paired `selected` vs neutral-`drift` ensemble.
///
/// Paired effects follow the [`crate::core::experiment_runner::PairedEnsembleReport`] convention
/// with `control = drift` and `treatment = selected`, so positive `paired_mean_delta` indicates
/// selection excess over lockstep neutral drift.
pub fn run_live_drift_ensemble(
    selected_manifest: &crate::core::experiment::ExperimentManifest,
    drift_manifest: &crate::core::experiment::ExperimentManifest,
    registry: &crate::core::experiment::ObservableRegistry,
) -> Result<DriftEnsembleReport, ExperimentError> {
    let (declared_factors, _sel_cfg, drf_cfg) =
        validate_drift_pair_manifests(selected_manifest, drift_manifest, registry)?;
    if selected_manifest.seeds != drift_manifest.seeds {
        return Err(ExperimentError::UndeclaredFactorDifference {
            path: "seeds (paired runs require the same seeds in the same order)".into(),
        });
    }
    let seed_order = selected_manifest.seeds.clone();
    let mut pairs = Vec::with_capacity(seed_order.len());
    let mut schedules = Vec::with_capacity(seed_order.len());
    let mut diagnostics = Vec::with_capacity(seed_order.len());
    let mut window_net_selection_by_seed = Vec::with_capacity(seed_order.len());

    for &seed in &seed_order {
        let pair = run_live_drift_pair(selected_manifest, drift_manifest, registry, seed)?;
        pairs.push(crate::core::experiment_runner::SeedPair {
            seed,
            control: pair.drift,
            treatment: pair.selected,
        });
        schedules.push(pair.schedule);
        diagnostics.push(pair.diagnostics);
        window_net_selection_by_seed.push((seed, pair.window_net_selection_deltas));
    }

    let ensemble = crate::core::experiment_runner::summarize_seed_pairs(
        drift_manifest,
        selected_manifest,
        registry,
        declared_factors,
        seed_order,
        pairs,
    )?;

    Ok(DriftEnsembleReport {
        ensemble,
        schedules,
        diagnostics,
        reset_period_ticks: drf_cfg.drift_reset_ticks,
        window_net_selection_by_seed,
    })
}

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
