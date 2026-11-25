pub use crate::core::components::*;
pub use crate::core::environmental_systems::*;
pub use crate::core::resources::*;
pub use crate::core::world_systems::*;

use crate::core::terrain::{MapConfig, MapSettings, TerrainMap};
use crate::core::world_artifact::WorldIdentity;
use bevy_ecs::prelude::{Resource, World};

/// Normalized `ANMW v3` sea level inserted when booting from bundled `sim_256.anmw3`.
///
/// Absent on legacy v1/v2 and headless [`TerrainSource::Generated`] paths so lake-candidate checks
/// keep bit-for-bit v1 behaviour there while auditing relative to `sea_level_norm` on v3.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct WorldV3SeaLevel(pub f32);

/// Where `init_world_with` obtains its `TerrainMap` and `WorldIdentity`.
///
/// Headless experiment runners (`LiveExperimentAdapter`) must pass [`TerrainSource::Generated`] so
/// machine environment variables and app artifacts (`ANIMA_WORLD_ARTIFACT`, `ANIMA_CACHE_DIR`, and
/// `<temp_dir>/anima_world_artifact.anmw`) never silently change an experiment's terrain. The
/// desktop app keeps [`init_world`] (`AppArtifactOrGenerated`), which boots on verified `ANMW v3`
/// `sim_256.anmw3` (or explicit `ANIMA_WORLD_ARTIFACT` override).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainSource {
    /// Generate deterministically in-process from `MapSettings::default()` and
    /// `MapConfig::default()`, reading no environment variables, `map_config.json`, or
    /// artifact/cache files (`OnceLock`-memoized per process).
    Generated,
    /// Desktop app behaviour (`WV2-7`): load explicit `ANIMA_WORLD_ARTIFACT` if set, else bundled
    /// `world-v2/sim_256.anmw3` or dev `artifacts/world-v2/sim_256.anmw3` with SHA-256 verification.
    /// Legacy `<temp_dir>/anima_world_artifact.anmw` and `anima_world_cache` are never read, and any
    /// missing, corrupt, or SHA-mismatched artifact fails loudly without procedural fallback.
    AppArtifactOrGenerated,
}

static GENERATED_DEFAULT_TERRAIN: std::sync::OnceLock<(TerrainMap, WorldIdentity)> =
    std::sync::OnceLock::new();

pub fn init_world() -> World {
    init_world_with(TerrainSource::AppArtifactOrGenerated)
}

pub fn init_world_with(terrain_source: TerrainSource) -> World {
    try_init_world_with_bundled_path(terrain_source, None).unwrap_or_else(|err| {
        panic!("failed to initialize desktop world ({terrain_source:?}): {err}")
    })
}

/// Fallible counterpart to [`init_world_with`] accepting an optional bundled `sim_256.anmw3` path
/// resolved by the Tauri main thread (`WV2-7`).
pub fn try_init_world_with_bundled_path(
    terrain_source: TerrainSource,
    bundled_world_path: Option<&std::path::Path>,
) -> Result<World, String> {
    let map_settings = MapSettings::default();
    let loaded = match terrain_source {
        TerrainSource::Generated => {
            let (terrain, identity) = GENERATED_DEFAULT_TERRAIN
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
                .clone();
            crate::core::world_artifact::LoadedDesktopWorld {
                terrain,
                identity,
                v3_sea_level_norm: None,
            }
        }
        TerrainSource::AppArtifactOrGenerated => {
            crate::core::world_artifact::load_desktop_world_with_bundled_path(
                bundled_world_path,
                &map_settings,
            )?
        }
    };
    Ok(init_world_from_loaded(loaded))
}

/// Builds a fresh Bevy [`World`] from an already loaded/verified [`LoadedDesktopWorld`].
pub fn init_world_from_loaded(loaded: crate::core::world_artifact::LoadedDesktopWorld) -> World {
    let crate::core::world_artifact::LoadedDesktopWorld {
        terrain: terrain_map,
        identity: world_identity,
        v3_sea_level_norm,
    } = loaded;
    let mut world = World::new();
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
    if let Some(sea_level_norm) = v3_sea_level_norm {
        world.insert_resource(WorldV3SeaLevel(sea_level_norm));
    }
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
