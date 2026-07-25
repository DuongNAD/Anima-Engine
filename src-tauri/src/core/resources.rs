use crate::core::components::{
    AgentMigrationData, CombatEvent, OutboundMigration, RaycastTelemetry, ShardingConfig,
};
use crate::evolution::genotype::MorphologyGenotype;
use crate::evolution::meta_ai::EnvironmentalEvent;
use bevy_ecs::prelude::*;
use glam::Vec3;
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
#[derive(Resource)]
pub struct SimRng {
    inner: rand::rngs::StdRng,
    seed: u64,
}

impl SimRng {
    pub fn from_seed(seed: u64) -> Self {
        use rand::SeedableRng;
        Self {
            inner: rand::rngs::StdRng::seed_from_u64(seed),
            seed,
        }
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

    pub fn rng(&mut self) -> &mut rand::rngs::StdRng {
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
