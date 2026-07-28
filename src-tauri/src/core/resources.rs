use crate::core::components::{
    AgentMigrationData, CombatEvent, OutboundMigration, RaycastTelemetry, ShardingConfig,
};
use crate::evolution::genotype::MorphologyGenotype;
use crate::evolution::meta_ai::EnvironmentalEvent;
use bevy_ecs::prelude::*;
use glam::Vec3;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

#[derive(Resource, Default)]
pub struct ActiveRaycasts {
    pub raycasts: Vec<RaycastTelemetry>,
}

#[derive(Resource, Default)]
pub struct CombatEvents {
    pub events: Vec<CombatEvent>,
    pub predator_centroids: Vec<(Entity, Vec3, Vec3, u32)>,
    pub prey_centroids: Vec<(Entity, Vec3, Vec3, u32)>,
}

#[derive(Resource, serde::Serialize, serde::Deserialize, Clone, Copy, Debug)]
pub struct FoodSpawnSettings {
    pub max_food_count: usize,
    pub default_energy: f32,
    pub default_hydration: f32,
}

impl Default for FoodSpawnSettings {
    fn default() -> Self {
        Self {
            max_food_count: 50,
            default_energy: 30.0,
            default_hydration: 20.0,
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveEnvironmentEvent(pub EnvironmentalEvent);

impl Default for ActiveEnvironmentEvent {
    fn default() -> Self {
        Self(EnvironmentalEvent::Stable)
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct SimulationSettings {
    pub target_fps: u32,
}

#[derive(Resource, serde::Serialize, serde::Deserialize, Clone, Copy, Debug)]
pub struct MapBounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl Default for MapBounds {
    fn default() -> Self {
        Self {
            min: Vec3::new(-100.0, 0.0, -100.0),
            max: Vec3::new(100.0, 10.0, 100.0),
        }
    }
}

/// Fallback seed for a world that declares none — a bare `World::new()` in a test harness. A real
/// run always gets its seed from the world it lives in; see [`resolve_run_seed`].
///
/// Equal to [`crate::core::terrain::MapSettings`]'s default seed on purpose, so the fallback and the
/// default generator agree instead of silently disagreeing.
pub const DEFAULT_SIM_SEED: u64 = 1337;

/// The single seeded RNG for every stochastic decision the *live* simulation makes.
///
/// The headless experiment slice already bans `thread_rng()` — see the module docs on
/// [`crate::core::exotic_energy`] and [`crate::core::experiment_runner`] — so a manifest replays to
/// the same trajectory. This resource extends that guarantee to the live Bevy world: same seed plus
/// same tick order yields the same run.
///
/// Systems take `ResMut<SimRng>`, which makes Bevy serialise them against one another. That
/// serialisation is a *requirement* of a reproducible draw order, not an accident to optimise away —
/// two systems drawing from one stream in parallel would reintroduce exactly the non-determinism
/// this type exists to remove.
/// The stream is `ChaCha12Rng` rather than `StdRng` for one reason: a checkpoint has to restore the
/// draw *position*, not just the seed, or a resumed run diverges from an uninterrupted one on its
/// very next draw (G1.2). `StdRng` is a newtype over `ChaCha12Rng` in rand 0.8 but does not expose
/// `get_word_pos`/`set_word_pos`. Naming the concrete type is therefore not a behaviour change —
/// `simrng_stream_matches_stdrng_exactly` pins that the two produce identical sequences.
#[derive(Resource)]
pub struct SimRng {
    inner: rand_chacha::ChaCha12Rng,
    seed: u64,
}

impl SimRng {
    pub fn from_seed(seed: u64) -> Self {
        use rand::SeedableRng;
        Self {
            inner: rand_chacha::ChaCha12Rng::seed_from_u64(seed),
            seed,
        }
    }

    /// How far into the stream this generator has drawn, as a ChaCha word position.
    ///
    /// Together with [`seed`](Self::seed) this is the complete state of the stream, so a snapshot
    /// can put it back exactly rather than restarting it.
    pub fn stream_pos(&self) -> u128 {
        self.inner.get_word_pos()
    }

    /// Rebuild the stream at `seed` and fast-forward it to `word_pos` — the restore half of
    /// [`stream_pos`](Self::stream_pos). O(1): ChaCha seeks, it does not replay.
    pub fn restore(seed: u64, word_pos: u128) -> Self {
        let mut me = Self::from_seed(seed);
        me.inner.set_word_pos(word_pos);
        me
    }

    /// Seeded from the world the agents live in, via [`resolve_run_seed`].
    pub fn for_world(world_seed: u32) -> Self {
        Self::from_seed(resolve_run_seed(world_seed))
    }

    /// The seed this stream was constructed from. Save-state and provenance records report it so a
    /// run can be reconstructed later.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Restart the stream from `seed`, discarding draw position. Used when loading a saved run.
    pub fn reseed(&mut self, seed: u64) {
        *self = Self::from_seed(seed);
    }

    pub fn rng(&mut self) -> &mut rand_chacha::ChaCha12Rng {
        &mut self.inner
    }
}

impl Default for SimRng {
    fn default() -> Self {
        Self::from_seed(DEFAULT_SIM_SEED)
    }
}

/// An explicit `ANIMA_SIM_SEED` override, if one is set and parses.
///
/// This exists for headless experiment sweeps that need to vary the stochastic trajectory while
/// holding the world fixed. An unparseable value is ignored rather than fatal — a malformed env var
/// must not take down a run.
pub fn sim_seed_override_from_env() -> Option<u64> {
    std::env::var("ANIMA_SIM_SEED")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
}

/// The run's seed, resolved from the world the agents actually live in.
///
/// Invariant **D07** of `docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md` names
/// [`crate::core::world_artifact::WorldIdentity`]'s seed as the origin of every simulation RNG
/// stream, so the world is the authority and `ANIMA_SIM_SEED` is only an override on top of it.
/// Deriving the seed from the world is what makes "this trajectory belongs to this world" true
/// rather than merely likely.
pub fn resolve_run_seed(world_seed: u32) -> u64 {
    sim_seed_override_from_env().unwrap_or_else(|| u64::from(world_seed))
}

/// Whether newly created agents get their own heritable brain, and what shape it takes.
///
/// **Off by default.** With `evolved = false` nothing spawns a [`crate::core::components::AgentBrain`]
/// and every agent runs on the shared [`crate::ai::model::BrainModel`] exactly as before — the
/// rollback path ADR-0003 decision 7 requires, and the baseline gate EB-S04 compares against.
///
/// A resource rather than a scattered `std::env::var` so tests can set it directly and a run's
/// configuration is visible in one place. `ANIMA_EVOLVED_BRAINS` seeds it at world construction.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct BrainPolicy {
    pub evolved: bool,
    pub arch: crate::evolution::brain_genotype::ArchSpec,
    /// In-life learning — the Baldwin half of ADR-0003's hybrid. **Off by default, and behind its own
    /// flag**, because it is the expensive half: one backward pass per learning agent.
    pub lifetime_learning: LifetimeLearning,
    /// Energy an agent spends per second maintaining every 1,000 brain parameters. **`0.0` by
    /// default**, which is the baseline: no brain has ever cost anything to run.
    ///
    /// Neural tissue is metabolically expensive in real organisms, and a cost that scales with brain
    /// size is what stops selection from growing brains for free. Without it, a bigger brain is
    /// strictly better and the only limit is the memory budget — which selection cannot feel.
    ///
    /// ADR-0003 decision 10 requires this to stay `0.0` until **EB-S06** shows closed energy holds
    /// with it switched on, because the charge has to reach the detritus pool rather than vanish.
    pub brain_metabolic_cost: f32,
}

/// Configuration for in-life learning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LifetimeLearning {
    pub enabled: bool,
    pub learning_rate: f32,
    pub discount: f32,
    /// Ticks between updates. Learning replaces the agent's network, which allocates, so it runs on
    /// an interval rather than every tick — and the interval is the knob that trades adaptation speed
    /// against cost.
    pub interval: u32,
    /// Only agents within this distance of the world origin learn.
    ///
    /// The stand-in for Simulation-LOD, which is still owed at the backend (M3). ADR-0003 decision 6
    /// requires learning to be confined to an active radius; wiring that to the real LOD centre is
    /// part of the LOD work, and until then the radius is measured from the origin so the constraint
    /// exists and is testable rather than merely promised.
    pub active_radius: f32,
}

impl Default for LifetimeLearning {
    fn default() -> Self {
        Self {
            enabled: false,
            learning_rate: 1e-3,
            discount: 0.99,
            interval: 8,
            active_radius: f32::INFINITY,
        }
    }
}

impl Default for BrainPolicy {
    fn default() -> Self {
        Self {
            evolved: false,
            arch: crate::evolution::brain_genotype::EVOLVED_ARCH,
            lifetime_learning: LifetimeLearning::default(),
            brain_metabolic_cost: 0.0,
        }
    }
}

impl BrainPolicy {
    /// Enabled by `ANIMA_EVOLVED_BRAINS` set to anything other than `0`/`false`/empty. Anything
    /// unset or unparseable leaves the legacy behaviour in place — the safe direction for a flag
    /// that changes what the simulation *is*.
    pub fn from_env() -> Self {
        let evolved = std::env::var("ANIMA_EVOLVED_BRAINS")
            .map(|v| {
                let v = v.trim().to_ascii_lowercase();
                !(v.is_empty() || v == "0" || v == "false")
            })
            .unwrap_or(false);
        let learning = std::env::var("ANIMA_LIFETIME_LEARNING")
            .map(|v| {
                let v = v.trim().to_ascii_lowercase();
                !(v.is_empty() || v == "0" || v == "false")
            })
            .unwrap_or(false);
        Self {
            evolved,
            lifetime_learning: LifetimeLearning {
                // Learning without a per-agent brain would have nothing of its own to change, so it
                // is only meaningful alongside `evolved`.
                enabled: learning && evolved,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// A fresh brain for a newly created individual, or `None` when evolved brains are off.
    ///
    /// Called at genesis and evolutionary replacement only. Restore and migration must **not** call
    /// this — they carry a brain that already exists (invariant D01).
    pub fn new_brain(
        &self,
        rng: &mut impl rand::Rng,
    ) -> Option<crate::core::components::AgentBrain> {
        if !self.evolved {
            return None;
        }
        crate::evolution::brain_genotype::BrainGenotype::random(self.arch, rng)
            .ok()
            .map(crate::core::components::AgentBrain::from_genotype)
    }
}

/// Named sub-streams of the run seed.
///
/// The live simulation is multi-threaded: world setup, the Bevy schedule and the evolution thread
/// all draw random numbers, and they do not run in a fixed interleaving relative to one another.
/// Handing each its own stream keeps every draw sequence reproducible on its own terms; a single
/// shared stream would make the result depend on thread scheduling, which is precisely what this
/// work removes.
pub mod sim_stream {
    pub const WORLD_INIT: u64 = 1;
    pub const EVOLUTION: u64 = 2;
    /// Founder brains in a headless experiment run
    /// ([`crate::core::live_experiment`]).
    ///
    /// Separate from the ecology stream on purpose: a controlled comparison of "brains on" against
    /// "brains off" is only interpretable if the two arms make the *same* ecology draws, and a
    /// founding population drawing ~5,769 f32 per agent out of `SimRng` would displace every later
    /// draw in the run. See `live_experiment::genesis`.
    pub const LIVE_GENESIS_BRAINS: u64 = 3;
}

/// An independent, reproducible stream for code that is not a Bevy system (setup paths and worker
/// threads). Pass the run seed from [`resolve_run_seed`] and a constant from [`sim_stream`].
///
/// `run_seed` is a parameter rather than something read from the environment inside, so the caller's
/// choice of seed is visible at the call site and testable without touching process state.
pub fn derived_rng(run_seed: u64, stream: u64) -> rand::rngs::StdRng {
    use rand::SeedableRng;
    // Golden-ratio odd constant, same mixing trick `terrain.rs` uses to decorrelate noise seeds.
    let mixed = run_seed ^ stream.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    rand::rngs::StdRng::seed_from_u64(mixed)
}

#[derive(Resource, serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default)]
pub struct EpochManager {
    pub ticks_per_epoch: u64,
    pub current_epoch_ticks: u64,
    pub current_epoch: u32,
}

#[derive(Clone, Debug)]
pub struct AgentEpochStats {
    pub entity: Entity,
    pub genotype: MorphologyGenotype,
    pub fitness: f32,
    pub speed: f32,
    pub efficiency: f32,
    /// Total body mass (Metabolic-Theory master trait) — a MAP-Elites ecological niche axis.
    pub body_mass: f32,
    /// Distance roamed this epoch (foraging range / niche breadth) — the other niche axis.
    pub foraging_range: f32,
    pub position: glam::Vec3,
    pub lineage_id: String,
    pub generation: u32,
}

#[derive(Resource)]
pub struct EvolutionSender(pub crossbeam_channel::Sender<Vec<AgentEpochStats>>);

/// One offspring the evolution thread asks the world to spawn in place of a retired agent:
/// `(retired entity, genotype, position, lineage id, generation, parent lineage ids)`.
///
/// A name for a tuple that was already spelled out in five places. Purely an alias — the channel,
/// the queue and every existing call site keep their exact types — so a headless harness can build
/// the same channel without copying the shape and quietly getting one field wrong.
pub type EvolutionSpawn = (
    Entity,
    MorphologyGenotype,
    glam::Vec3,
    String,
    u32,
    Vec<String>,
);

#[derive(Resource, Clone, Debug, Default)]
pub struct EvolutionQueue {
    pub pending_replacements: Vec<(
        Entity,
        MorphologyGenotype,
        glam::Vec3,
        String,
        u32,
        Vec<String>,
    )>,
}

#[derive(Resource)]
pub struct EvolutionReceiver(
    pub  crossbeam_channel::Receiver<(
        Entity,
        MorphologyGenotype,
        glam::Vec3,
        String,
        u32,
        Vec<String>,
    )>,
);

#[derive(Resource, Clone)]
pub struct ShardingResource(pub Arc<RwLock<ShardingConfig>>);

#[derive(Resource)]
pub struct InboundMigrationReceiver(pub crossbeam_channel::Receiver<AgentMigrationData>);

/// Maximum number of complete agents waiting for the networking worker.
///
/// A fixed bound makes memory use independent of how many agents cross a shard boundary in one
/// tick. `256` is over twenty-five times the default ten-agent founding population, so ordinary
/// whole-population moves fit while adversarial bursts remain bounded. A slot owns one morphology,
/// lineage metadata and homeostatic state; evolved brain weights stay behind `Arc`, so the queue
/// does not deep-copy the ~23 KiB network per agent. Morphology size is variable, therefore this is
/// a count bound rather than a false byte-perfect claim.
///
/// The migration systems use `try_send`, so saturation reflects excess agents back into the local
/// shard instead of blocking the simulation or deleting scientific state.
pub const OUTBOUND_MIGRATION_QUEUE_CAPACITY: usize = 256;

pub fn outbound_migration_channel() -> (
    crossbeam_channel::Sender<OutboundMigration>,
    crossbeam_channel::Receiver<OutboundMigration>,
) {
    crossbeam_channel::bounded(OUTBOUND_MIGRATION_QUEUE_CAPACITY)
}

#[derive(Default)]
struct MigrationHandoffCounters {
    queued: AtomicU64,
    full_rejections: AtomicU64,
    disconnected_rejections: AtomicU64,
}

/// Auditable totals for the process-local migration handoff boundary.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MigrationHandoffSnapshot {
    #[ts(type = "number")]
    pub queued: u64,
    #[ts(type = "number")]
    pub full_rejections: u64,
    #[ts(type = "number")]
    pub disconnected_rejections: u64,
}

/// Shared, allocation-free migration handoff counters.
///
/// Clones share the same atomics, allowing the simulation world to record failures while an IPC
/// command reads a coherent-enough monotonic snapshot without locking or perturbing the tick.
#[derive(Resource, Clone, Default)]
pub struct MigrationHandoffDiagnostics(Arc<MigrationHandoffCounters>);

impl MigrationHandoffDiagnostics {
    fn increment(counter: &AtomicU64) {
        let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            Some(value.saturating_add(1))
        });
    }

    pub fn record_queued(&self) {
        Self::increment(&self.0.queued);
    }

    pub fn record_full_rejection(&self) {
        Self::increment(&self.0.full_rejections);
    }

    pub fn record_disconnected_rejection(&self) {
        Self::increment(&self.0.disconnected_rejections);
    }

    pub fn snapshot(&self) -> MigrationHandoffSnapshot {
        MigrationHandoffSnapshot {
            queued: self.0.queued.load(Ordering::Relaxed),
            full_rejections: self.0.full_rejections.load(Ordering::Relaxed),
            disconnected_rejections: self.0.disconnected_rejections.load(Ordering::Relaxed),
        }
    }

    pub fn reset(&self) {
        self.0.queued.store(0, Ordering::Relaxed);
        self.0.full_rejections.store(0, Ordering::Relaxed);
        self.0.disconnected_rejections.store(0, Ordering::Relaxed);
    }
}

#[derive(Resource)]
pub struct OutboundMigrationSender(pub crossbeam_channel::Sender<OutboundMigration>);

#[derive(Resource)]
pub struct BevyMigrationTrigger(pub crossbeam_channel::Receiver<u16>);

#[derive(Resource, serde::Serialize, serde::Deserialize, Clone, Copy, Debug)]
pub struct EnvironmentalSpawnSettings {
    pub max_tree_count: usize,
    pub default_lake_water: f32,
    pub default_lake_replenish: f32,
    pub default_tree_fruit: f32,
    pub default_tree_growth: f32,
    pub default_seed_cooldown: f32,
    pub default_seed_spread: f32,
}

impl Default for EnvironmentalSpawnSettings {
    fn default() -> Self {
        Self {
            max_tree_count: 50,
            default_lake_water: 500.0,
            default_lake_replenish: 5.0,
            default_tree_fruit: 100.0,
            default_tree_growth: 2.0,
            default_seed_cooldown: 15.0,
            default_seed_spread: 20.0,
        }
    }
}

// ---- ML backend ---------------------------------------------------------------------------

/// Whether the learner and inference should run on the wgpu GPU backend.
///
/// **Off by default since 2026-07-28, and the default is the point.** The GPU path leaks: burn-wgpu
/// asks a compute pipeline for its bind group layout on every dispatch, and wgpu-core 0.19.4 assigns
/// a fresh id into a registry `Vec` that only ever grows. Measured headless over three minutes with
/// ten agents: **+5.8 MB/min of live heap, unbounded**, against **0.00 MB/min** on the CPU backend —
/// and 29 MB of RSS instead of ~200 MB. In the desktop app it was 14 MB/min, which is 19 GB after a
/// day. See `STATE_OF_THE_PROJECT.md` §3.17.
///
/// A simulator whose whole purpose is to run for a long time cannot default to the backend that
/// cannot. `ANIMA_USE_GPU=1` opts back in for a short session where inference speed is what matters.
///
/// This lives here, in one function, because the decision used to be written out three times —
/// `simulation_loop::start` and both `ai::model` constructors — each with its own `unwrap_or(true)`.
/// Three copies of a default is three chances for them to disagree the next time one is changed.
pub fn gpu_backend_requested() -> bool {
    gpu_backend_from(std::env::var("ANIMA_USE_GPU").ok().as_deref())
}

/// The decision behind [`gpu_backend_requested`], pure so the tests never write process state.
pub fn gpu_backend_from(raw: Option<&str>) -> bool {
    match raw {
        None => false,
        Some(value) => {
            let trimmed = value.trim();
            !(trimmed.is_empty() || trimmed == "0" || trimmed.eq_ignore_ascii_case("false"))
        }
    }
}

// ---- Unattended start ---------------------------------------------------------------------

/// Whether the engine should begin simulating the moment the app launches.
///
/// **Off unless `ANIMA_AUTOSTART` says otherwise**, which is the interactive behaviour: a fresh
/// world waits at the Start button.
///
/// That wait is correct when someone is looking at the window and impossible when nobody is. A world
/// meant to run for hours and be looked at later, or a measured run driven from a script, has no one
/// to click — and the only path that started an engine without a click was *resuming an autosave*,
/// which is precisely what a run "from zero" does not have.
///
/// Accepts anything except the three spellings of no, matching `ANIMA_TICK_CAPTURE`: an unset,
/// empty, `0` or `false` value all mean the same thing, so a shell script that computes the value
/// and comes up empty does not silently start a simulation.
pub fn autostart_from_env() -> bool {
    autostart_requested(std::env::var("ANIMA_AUTOSTART").ok().as_deref())
}

/// The decision behind [`autostart_from_env`], separated so it is testable without touching
/// process-wide state.
pub fn autostart_requested(raw: Option<&str>) -> bool {
    match raw {
        None => false,
        Some(value) => {
            let trimmed = value.trim();
            !(trimmed.is_empty() || trimmed == "0" || trimmed.eq_ignore_ascii_case("false"))
        }
    }
}

// ---- Founding population ------------------------------------------------------------------

/// Founders genesis creates when nothing says otherwise.
///
/// Ten, laid out on a line, with the first seven [`crate::core::components::Prey`] and the rest
/// [`crate::core::components::Predator`] — the shape every measurement in
/// `BENCHMARK_BASELINE.md` before 2026-07-27 was taken against.
pub const DEFAULT_FOUNDING_POPULATION: usize = 10;

/// Prey share of the founding population, as a fraction. `7/10` reproduces the legacy split
/// exactly at the default count and generalises without a second constant to keep in sync.
const FOUNDING_PREY_NUMERATOR: usize = 7;
const FOUNDING_PREY_DENOMINATOR: usize = 10;

/// Upper bound on a requested founding population.
///
/// Not a physical limit — an honesty one. Every founder is a full ECS entity with segments, a
/// spatial-hash cell and a lineage root, so a mistyped `100000` would hang the app somewhere
/// unhelpful rather than fail. Rejecting the value names the problem at the moment it is made.
pub const MAX_FOUNDING_POPULATION: usize = 10_000;

/// Margin, in world units, between the grid layout and the map edge.
const FOUNDING_GRID_MARGIN: f32 = 5.0;

/// Why a requested founding population was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoundingPopulationError {
    NotANumber(String),
    Zero,
    TooLarge { found: usize, limit: usize },
}

impl std::fmt::Display for FoundingPopulationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotANumber(raw) => write!(f, "`{raw}` is not a whole number"),
            Self::Zero => write!(f, "a founding population of zero has nothing to evolve"),
            Self::TooLarge { found, limit } => {
                write!(f, "{found} founders exceeds the {limit} limit")
            }
        }
    }
}

/// Where genesis puts its founders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundingLayout {
    /// `x = i * 5.0` along `z = 0`. The layout every pre-2026-07-27 run used, kept **exactly** so
    /// that an unset `ANIMA_FOUNDING_POPULATION` is bit-identical to before this knob existed.
    ///
    /// It does not scale: the twenty-first founder is already at the `+100` map edge.
    Line,
    /// A square grid inset from the map edge by [`FOUNDING_GRID_MARGIN`], sized from the count.
    /// Every position is inside [`MapBounds`] by construction, at any count.
    Grid,
}

/// How many founders genesis creates, and how they are placed.
///
/// Split from the environment on purpose: [`FoundingPlan::parse`] is pure and carries every rule
/// worth testing, so the tests never write a process-wide environment variable to reach them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FoundingPlan {
    pub count: usize,
    pub layout: FoundingLayout,
}

impl Default for FoundingPlan {
    fn default() -> Self {
        Self {
            count: DEFAULT_FOUNDING_POPULATION,
            layout: FoundingLayout::Line,
        }
    }
}

impl FoundingPlan {
    /// Read `ANIMA_FOUNDING_POPULATION`.
    ///
    /// Unset means the default plan, which is the legacy run in full: ten founders on the legacy
    /// line. A malformed value is refused loudly on stderr and the run continues at the default —
    /// a benchmark that silently ran a different population than the one asked for is worse than
    /// one that says it ignored you.
    pub fn from_env() -> Self {
        match std::env::var("ANIMA_FOUNDING_POPULATION") {
            Err(_) => Self::default(),
            Ok(raw) => match Self::parse(&raw) {
                Ok(plan) => plan,
                Err(e) => {
                    eprintln!(
                        "ANIMA_FOUNDING_POPULATION is not usable ({e}); genesis uses the default \
                         {DEFAULT_FOUNDING_POPULATION}"
                    );
                    Self::default()
                }
            },
        }
    }

    /// Parse a requested count.
    ///
    /// Any accepted value uses [`FoundingLayout::Grid`], **including** a request for exactly the
    /// default count: the layout follows the request, not the number, so "I asked for something"
    /// and "I asked for nothing" stay distinguishable in the resulting run.
    pub fn parse(raw: &str) -> Result<Self, FoundingPopulationError> {
        let trimmed = raw.trim();
        let count = trimmed
            .parse::<usize>()
            .map_err(|_| FoundingPopulationError::NotANumber(trimmed.to_string()))?;
        if count == 0 {
            return Err(FoundingPopulationError::Zero);
        }
        if count > MAX_FOUNDING_POPULATION {
            return Err(FoundingPopulationError::TooLarge {
                found: count,
                limit: MAX_FOUNDING_POPULATION,
            });
        }
        Ok(Self {
            count,
            layout: FoundingLayout::Grid,
        })
    }

    /// Whether founder `i` is prey. The first 70% are, matching the legacy `i < 7` of ten.
    pub fn is_prey(&self, i: usize) -> bool {
        i * FOUNDING_PREY_DENOMINATOR < self.count * FOUNDING_PREY_NUMERATOR
    }

    /// Where founder `i` starts.
    ///
    /// No randomness: invariant **D07** forbids `thread_rng()` anywhere in genesis, and arithmetic
    /// that depends only on `(i, count, bounds)` satisfies it without needing a stream at all.
    pub fn position(&self, i: usize, bounds: &MapBounds) -> Vec3 {
        match self.layout {
            FoundingLayout::Line => Vec3::new(i as f32 * 5.0, 0.0, 0.0),
            FoundingLayout::Grid => {
                let side = grid_side(self.count);
                let col = (i % side) as f32;
                let row = (i / side) as f32;
                let last = (side - 1) as f32;
                let span_x = (bounds.max.x - bounds.min.x) - 2.0 * FOUNDING_GRID_MARGIN;
                let span_z = (bounds.max.z - bounds.min.z) - 2.0 * FOUNDING_GRID_MARGIN;
                let step_x = if side > 1 { span_x / last } else { 0.0 };
                let step_z = if side > 1 { span_z / last } else { 0.0 };
                Vec3::new(
                    bounds.min.x + FOUNDING_GRID_MARGIN + col * step_x,
                    0.0,
                    bounds.min.z + FOUNDING_GRID_MARGIN + row * step_z,
                )
            }
        }
    }
}

/// Side of the smallest square grid holding `count` founders.
fn grid_side(count: usize) -> usize {
    let mut side = (count as f64).sqrt().ceil() as usize;
    // `sqrt` on a large perfect square can land one below it after rounding; step up rather than
    // trust the float, because a side that is one too small silently drops the last row.
    while side * side < count {
        side += 1;
    }
    side.max(1)
}

#[cfg(test)]
mod gpu_backend_tests {
    use super::*;

    /// The whole reason this changed: a run that says nothing must not pick the backend that grows
    /// 5.8 MB every minute forever.
    #[test]
    fn unset_means_cpu() {
        assert!(!gpu_backend_from(None));
    }

    #[test]
    fn the_three_spellings_of_no_still_mean_no() {
        for off in ["", "  ", "0", "false", "FALSE"] {
            assert!(
                !gpu_backend_from(Some(off)),
                "{off:?} must not select the GPU"
            );
        }
    }

    /// Opting in is still one variable away, for a short session where inference speed is the thing
    /// that matters.
    #[test]
    fn anything_else_opts_back_in() {
        for on in ["1", "true", "yes", " 1 "] {
            assert!(gpu_backend_from(Some(on)), "{on:?} must select the GPU");
        }
    }
}

#[cfg(test)]
mod autostart_tests {
    use super::*;

    /// Unset is the interactive behaviour, and it is the one that must not change.
    #[test]
    fn unset_does_not_start_anything() {
        assert!(!autostart_requested(None));
    }

    /// A script that builds the value from another variable produces an empty string when that
    /// variable is missing. Starting a simulation on that would be the worst possible reading.
    #[test]
    fn the_three_spellings_of_no() {
        for off in ["", "  ", "0", "false", "FALSE", "False"] {
            assert!(
                !autostart_requested(Some(off)),
                "{off:?} should not autostart"
            );
        }
    }

    #[test]
    fn anything_else_is_yes() {
        for on in ["1", "true", "yes", "on", " 1 "] {
            assert!(autostart_requested(Some(on)), "{on:?} should autostart");
        }
    }
}

#[cfg(test)]
mod founding_population_tests {
    use super::*;

    /// The whole point of the default: a run that says nothing is the run that existed before this
    /// knob did. Ten founders, on the exact legacy line, split 7/3.
    #[test]
    fn default_reproduces_the_legacy_genesis_exactly() {
        let plan = FoundingPlan::default();
        let bounds = MapBounds::default();
        assert_eq!(plan.count, 10);
        assert_eq!(plan.layout, FoundingLayout::Line);
        for i in 0..plan.count {
            assert_eq!(
                plan.position(i, &bounds),
                Vec3::new(i as f32 * 5.0, 0.0, 0.0),
                "founder {i} moved off the legacy line"
            );
            assert_eq!(plan.is_prey(i), i < 7, "founder {i} changed role");
        }
    }

    /// Asking for the default count is still *asking*, so it takes the scalable layout. Without
    /// this the same number would mean two different runs depending on how it was reached.
    #[test]
    fn an_explicit_request_uses_the_grid_even_at_the_default_count() {
        let plan = FoundingPlan::parse("10").expect("10 is a usable count");
        assert_eq!(plan.count, 10);
        assert_eq!(plan.layout, FoundingLayout::Grid);
    }

    /// The defect that made this knob necessary: `for i in 0..1000` on the legacy line puts founder
    /// 20 on the map edge and founder 999 at x = 4995, fifty times outside the world.
    #[test]
    fn a_thousand_founders_all_land_inside_the_map() {
        let plan = FoundingPlan::parse("1000").expect("1000 is a usable count");
        let bounds = MapBounds::default();
        for i in 0..plan.count {
            let p = plan.position(i, &bounds);
            assert!(
                p.x >= bounds.min.x && p.x <= bounds.max.x,
                "founder {i} at x={} is outside [{}, {}]",
                p.x,
                bounds.min.x,
                bounds.max.x
            );
            assert!(
                p.z >= bounds.min.z && p.z <= bounds.max.z,
                "founder {i} at z={} is outside [{}, {}]",
                p.z,
                bounds.min.z,
                bounds.max.z
            );
        }
        // Positive control: the layout this replaces really does fail the assertion above, so the
        // test is measuring the fix rather than a property both layouts happen to have.
        let legacy = FoundingPlan {
            count: 1000,
            layout: FoundingLayout::Line,
        };
        assert!(legacy.position(999, &bounds).x > bounds.max.x);
    }

    /// Two founders on the same spot would be one collision pair the physics never resolves.
    #[test]
    fn grid_positions_are_distinct() {
        let plan = FoundingPlan::parse("1000").expect("1000 is a usable count");
        let bounds = MapBounds::default();
        let mut seen = std::collections::HashSet::new();
        for i in 0..plan.count {
            let p = plan.position(i, &bounds);
            assert!(
                seen.insert((p.x.to_bits(), p.z.to_bits())),
                "founder {i} shares a position with an earlier one"
            );
        }
    }

    /// The 7/3 split is the predator-prey premise of the whole ecology, so it has to survive the
    /// generalisation rather than only holding at ten.
    #[test]
    fn the_prey_share_holds_at_every_size() {
        for count in [1usize, 7, 10, 99, 1000, 9999] {
            let plan = FoundingPlan {
                count,
                layout: FoundingLayout::Grid,
            };
            let prey = (0..count).filter(|i| plan.is_prey(*i)).count();
            // `i * 10 < count * 7` admits exactly the first `ceil(7·count/10)` founders.
            assert_eq!(
                prey,
                (count * 7).div_ceil(10),
                "prey share drifted at {count}"
            );
            assert!(prey > 0, "no prey at {count}");
            assert!(prey < count || count == 1, "no predator at {count}");
        }
    }

    #[test]
    fn a_malformed_request_is_refused_rather_than_rounded_into_something_usable() {
        assert!(matches!(
            FoundingPlan::parse("0"),
            Err(FoundingPopulationError::Zero)
        ));
        assert!(matches!(
            FoundingPlan::parse("ten"),
            Err(FoundingPopulationError::NotANumber(_))
        ));
        assert!(matches!(
            FoundingPlan::parse("-5"),
            Err(FoundingPopulationError::NotANumber(_))
        ));
        assert!(matches!(
            FoundingPlan::parse(&(MAX_FOUNDING_POPULATION + 1).to_string()),
            Err(FoundingPopulationError::TooLarge { .. })
        ));
        assert_eq!(
            FoundingPlan::parse(" 250 ")
                .expect("surrounding space is not a syntax error")
                .count,
            250
        );
    }

    #[test]
    fn grid_side_covers_the_count() {
        for count in [1usize, 2, 4, 5, 9, 10, 100, 999, 1000, 10_000] {
            let side = grid_side(count);
            assert!(side * side >= count, "side {side} cannot hold {count}");
            assert!(
                side == 1 || (side - 1) * (side - 1) < count,
                "side {side} is larger than {count} needs"
            );
        }
    }
}
