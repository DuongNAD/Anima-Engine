pub use crate::core::components::*;
pub use crate::core::environmental_systems::*;
pub use crate::core::resources::*;
pub use crate::core::world_systems::*;

use crate::core::terrain::{MapConfig, MapSettings, TerrainMap};
use crate::core::world_artifact::{WorldArtifact, WorldIdentity};
use bevy_ecs::prelude::World;

/// Load the shared authoritative World Artifact the frontend wrote to
/// [`world_artifact::default_artifact_path`] and downsample it into the backend's working
/// `TerrainMap`, so the agents live in the SAME world that is rendered. Also returns the world's
/// [`WorldIdentity`] (seed + generator version + payload checksum) so a save can be pinned to it.
/// Returns `None` (→ caller falls back to local generate+cache) when no artifact exists yet or it is
/// corrupt, so the sim always boots.
fn load_shared_world_artifact(settings: &MapSettings) -> Option<(TerrainMap, WorldIdentity)> {
    let path = crate::core::world_artifact::default_artifact_path();
    let bytes = std::fs::read(&path).ok()?; // no shared world on disk yet → fall back
    if anima_worldgen::anmw_v3::is_anmw_v3_header(&bytes) {
        panic!(
            "refusing to boot on ANMW v3 shared world artifact at {} before WV2-7 desktop switchover (pre-switchover loud rejection)",
            path.display()
        );
    }
    match WorldArtifact::from_bytes(&bytes) {
        Ok(artifact) => {
            let terrain = artifact.to_terrain_map(settings.width, settings.height);
            let identity =
                WorldIdentity::from_terrain(&terrain, artifact.seed, artifact.generator_version);
            Some((terrain, identity))
        }
        Err(e) => {
            eprintln!(
                "shared world artifact at {} is invalid ({e}); using local terrain",
                path.display()
            );
            None
        }
    }
}

/// Where `init_world_with` obtains its `TerrainMap` and `WorldIdentity`.
///
/// Headless experiment runners (`LiveExperimentAdapter`) must pass [`TerrainSource::Generated`] so
/// machine environment variables and app artifacts (`ANIMA_WORLD_ARTIFACT`, `ANIMA_CACHE_DIR`, and
/// `<temp_dir>/anima_world_artifact.anmw`) never silently change an experiment's terrain. The
/// desktop app keeps [`init_world`] (`AppArtifactOrGenerated`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainSource {
    /// Generate deterministically in-process from `MapSettings::default()` and
    /// `MapConfig::default()`, reading no environment variables, `map_config.json`, or
    /// artifact/cache files (`OnceLock`-memoized per process).
    Generated,
    /// Desktop app behaviour: load `ANIMA_WORLD_ARTIFACT` or
    /// `<temp_dir>/anima_world_artifact.anmw` when present and valid, else generate+cache.
    AppArtifactOrGenerated,
}

static GENERATED_DEFAULT_TERRAIN: std::sync::OnceLock<(TerrainMap, WorldIdentity)> =
    std::sync::OnceLock::new();

pub fn init_world() -> World {
    init_world_with(TerrainSource::AppArtifactOrGenerated)
}

pub fn init_world_with(terrain_source: TerrainSource) -> World {
    let mut world = World::new();
    let map_settings = MapSettings::default();
    let (terrain_map, world_identity) = match terrain_source {
        TerrainSource::Generated => GENERATED_DEFAULT_TERRAIN
            .get_or_init(|| {
                let terrain =
                    TerrainMap::generate_with_config(&map_settings, &MapConfig::default());
                let identity = WorldIdentity::from_terrain(
                    &terrain,
                    map_settings.seed,
                    crate::core::terrain::WORLD_CACHE_VERSION,
                );
                (terrain, identity)
            })
            .clone(),
        TerrainSource::AppArtifactOrGenerated => {
            // World unification (direction A→B): if a shared World Artifact is provided, the agents
            // live in the SAME authoritative world the frontend generated & renders — the backend just
            // downsamples it into its working TerrainMap. Otherwise fall back to generate-once +
            // on-disk cache so the world is still produced only once and reloaded on later runs
            // (override cache via ANIMA_CACHE_DIR).
            load_shared_world_artifact(&map_settings).unwrap_or_else(|| {
                let cache_dir = std::env::var_os("ANIMA_CACHE_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| std::env::temp_dir().join("anima_world_cache"));
                let terrain = TerrainMap::load_or_generate(&map_settings, &cache_dir);
                let identity = WorldIdentity::from_terrain(
                    &terrain,
                    map_settings.seed,
                    crate::core::terrain::WORLD_CACHE_VERSION,
                );
                (terrain, identity)
            })
        }
    };
    world.insert_resource(SimulationSettings { target_fps: 60 });
    world.insert_resource(crate::ai::cpg::TimeStep(1.0 / 60.0));

    let bounds = MapBounds::default();

    // Ecosystem foundation: a per-cell NPP resource field whose carrying capacity is set by
    // each cell's Whittaker biome (rainforest rich, desert poor), plus the closed biomass
    // ledger that conserves energy across metabolism / predation / death. Built from the
    // terrain biomes before the map is handed to the ECS.
    let resource_field = crate::core::ecology::ResourceField::from_biomes(
        &terrain_map.biomes,
        terrain_map.width,
        terrain_map.height,
        bounds.min.x,
        bounds.min.z,
        bounds.max.x,
        bounds.max.z,
        0.02,
    );
    let starting_plants = resource_field.total_biomass();
    world.insert_resource(resource_field);
    world.insert_resource(crate::core::ecology::EcosystemBiomass {
        detritus: 0.0,
        plants: starting_plants,
        animals: 0.0,
        dormant: 0.0,
    });
    // The one route by which EU may enter a compartment from another compartment. Inserted next to
    // the biomass pool it operates on; its baseline is locked after genesis, not here, because
    // invariant D06 makes the founding population's energy a boundary condition rather than a
    // transaction.
    world.insert_resource(crate::core::energy_ledger::EnergyLedger::default());
    world.insert_resource(crate::core::ecology::SeasonClock::default());
    world.insert_resource(terrain_map);
    // The run's randomness is seeded from the world the agents live in, not from ambient process
    // state — invariant D07 of the creature-development contract. Resolved here, where the world's
    // identity is first known, so there is exactly one place that decides it.
    world.insert_resource(crate::core::resources::SimRng::for_world(
        world_identity.seed,
    ));
    // Off unless `ANIMA_EVOLVED_BRAINS` says otherwise, so a default run is the ADR-0003 baseline.
    world.insert_resource(crate::core::resources::BrainPolicy::from_env());
    // Off unless `ANIMA_DETERMINISTIC` says otherwise (G1.3). An interactive session keeps real
    // uuids, real timestamps and the multi-threaded executor; a run that must be reproducible turns
    // this on and gives up all three.
    world.insert_resource(crate::core::determinism::DeterministicMode::from_env());
    world.insert_resource(world_identity);
    world.insert_resource(crate::physics::SpatialHashGrid::new_prepopulated(
        10.0, &bounds,
    ));
    world.insert_resource(bounds);
    world.insert_resource(ActiveRaycasts {
        raycasts: Vec::with_capacity(1000),
    });
    world.insert_resource(CombatEvents {
        events: Vec::with_capacity(1000),
        predator_centroids: Vec::with_capacity(128),
        prey_centroids: Vec::with_capacity(128),
    });
    world.insert_resource(ActiveEnvironmentEvent::default());
    world.insert_resource(crate::core::telemetry_export::DemographicTracker::default());
    world.insert_resource(crate::core::telemetry_export::DemographicCensusState::default());
    world.insert_resource(crate::core::telemetry_export::EnergyFluxReport::default());
    world
}
