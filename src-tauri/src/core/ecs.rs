pub use crate::core::components::*;
pub use crate::core::environmental_systems::*;
pub use crate::core::resources::*;
pub use crate::core::world_systems::*;

use crate::core::terrain::{MapSettings, TerrainMap};
use bevy_ecs::prelude::World;

pub fn init_world() -> World {
    let mut world = World::new();
    let map_settings = MapSettings::default();
    let terrain_map = TerrainMap::generate(&map_settings);
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
    });
    world.insert_resource(crate::core::ecology::SeasonClock::default());
    world.insert_resource(terrain_map);
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
    world
}
