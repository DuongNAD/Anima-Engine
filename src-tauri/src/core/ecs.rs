pub use crate::core::components::*;
pub use crate::core::resources::*;
pub use crate::core::world_systems::*;
pub use crate::core::environmental_systems::*;

use bevy_ecs::prelude::World;

pub fn init_world() -> World {
    let mut world = World::new();
    world.insert_resource(SimulationSettings { target_fps: 60 });
    world.insert_resource(crate::ai::cpg::TimeStep(1.0 / 60.0));
    let bounds = MapBounds::default();
    world.insert_resource(crate::physics::SpatialHashGrid::new_prepopulated(10.0, &bounds));
    world.insert_resource(bounds);
    world.insert_resource(ActiveRaycasts { raycasts: Vec::with_capacity(1000) });
    world.insert_resource(CombatEvents {
        events: Vec::with_capacity(1000),
        predator_centroids: Vec::with_capacity(128),
        prey_centroids: Vec::with_capacity(128),
    });
    world.insert_resource(ActiveEnvironmentEvent::default());
    world
}
