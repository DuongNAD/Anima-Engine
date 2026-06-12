mod common;

use std::sync::{Arc, Mutex};
use bevy_ecs::prelude::*;

// Verify modular submodules compile cleanly
#[allow(unused_imports)]
use anima_engine_lib::core::agent_systems;
#[allow(unused_imports)]
use anima_engine_lib::core::networking_systems;
#[allow(unused_imports)]
use anima_engine_lib::core::simulation_lifecycle;

use anima_engine_lib::core::ecs::{
    init_world, FoodSpawnSettings,
    OutboundMigrationSender, InboundMigrationReceiver, ShardingResource, ShardingConfig,
    BevyMigrationTrigger,
};
use anima_engine_lib::ai::model::{BrainModel, BrainInferenceBuffer, brain_inference_system};
use anima_engine_lib::ai::cpg::update_cpg_system;
use anima_engine_lib::physics::{resolve_joints_system, integrate_physics_system};
use anima_engine_lib::ai::hrrl::{Transition, TransitionSender};
use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};
use anima_engine_lib::core::simulation_lifecycle::{
    BevyEvolutionSettings, BevyEvolutionRunning, BevyMapElitesGrid, BevyAppHandle,
    ActiveEvolutionSettings, BevyMapElitesArchive, NextNodeId, EnvironmentalEventReceiver,
    EvolutionSender, EvolutionReceiver, EvolutionQueue,
    sync_evolution_settings_system,
};

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator = common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_modularity_zero_allocations_in_tick_loop() {
    let _lock = TEST_LOCK.lock().unwrap();

    let mut world = init_world();
    world.insert_resource(anima_engine_lib::ai::pheromone::PheromoneGrid::default());
    world.insert_resource(BrainModel::new(15, 64, 4));
    world.insert_resource(BrainInferenceBuffer::default());
    world.insert_resource(FoodSpawnSettings::default());

    let (trans_tx, _trans_rx) = crossbeam_channel::bounded::<Transition>(4096);
    world.insert_resource(TransitionSender(trans_tx));

    let evolution_settings = Arc::new(std::sync::Mutex::new(EvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    }));
    let evolution_running = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let map_elites_grid = Arc::new(std::sync::Mutex::new(MapElitesGridState {
        grid: std::collections::HashMap::new(),
        grid_resolution: 50,
    }));

    world.insert_resource(BevyEvolutionSettings(evolution_settings));
    world.insert_resource(BevyEvolutionRunning(evolution_running));
    world.insert_resource(BevyMapElitesGrid(map_elites_grid));
    world.insert_resource(BevyAppHandle::<tauri::test::MockRuntime>(None));
    world.insert_resource(ActiveEvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    });
    world.insert_resource(BevyMapElitesArchive {
        archive: anima_engine_lib::evolution::map_elites::MapElitesArchive::new(0.5),
    });
    world.insert_resource(NextNodeId(3));

    let (stats_tx, _stats_rx) = crossbeam_channel::bounded(128);
    let (_spawn_tx, spawn_rx) = crossbeam_channel::bounded(128);
    let (_env_tx, env_rx) = crossbeam_channel::bounded(32);

    world.insert_resource(EvolutionSender(stats_tx));
    world.insert_resource(EvolutionReceiver(spawn_rx));
    world.insert_resource(EnvironmentalEventReceiver(env_rx));
    world.insert_resource(anima_engine_lib::core::ecs::EpochManager {
        ticks_per_epoch: 1000,
        current_epoch_ticks: 0,
        current_epoch: 0,
    });
    world.insert_resource(EvolutionQueue::default());

    // Migration resources
    let (_inbound_tx, inbound_rx) = crossbeam_channel::unbounded();
    let (outbound_tx, _outbound_rx) = crossbeam_channel::unbounded();
    let sharding_config = Arc::new(std::sync::RwLock::new(ShardingConfig::default()));
    let (_manual_migration_tx, manual_migration_rx) = crossbeam_channel::unbounded();

    world.insert_resource(InboundMigrationReceiver(inbound_rx));
    world.insert_resource(OutboundMigrationSender(outbound_tx));
    world.insert_resource(ShardingResource(sharding_config));
    world.insert_resource(BevyMigrationTrigger(manual_migration_rx));

    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems((
        sync_evolution_settings_system,
        anima_engine_lib::core::simulation_lifecycle::receive_environmental_events_system,
        anima_engine_lib::core::ecs::apply_environmental_effects_system.after(anima_engine_lib::core::simulation_lifecycle::receive_environmental_events_system),
        brain_inference_system,
        update_cpg_system.after(brain_inference_system),
        resolve_joints_system.after(update_cpg_system),
        integrate_physics_system.after(resolve_joints_system),
        anima_engine_lib::ai::pheromone::agent_release_pheromone_system.after(integrate_physics_system),
        anima_engine_lib::ai::pheromone::update_pheromone_grid_system.after(anima_engine_lib::ai::pheromone::agent_release_pheromone_system),
        anima_engine_lib::ai::pheromone::agent_read_pheromone_system.after(anima_engine_lib::ai::pheromone::update_pheromone_grid_system),
    ));

    // Warm-up to cache Bevy archetype maps and query states
    for _ in 0..10 {
        schedule.run(&mut world);
    }

    // Start tracking allocations
    ALLOCATOR.start_tracking();

    // Run systems on hot path (performs 0 allocations once warmed up)
    for _ in 0..100 {
        schedule.run(&mut world);
    }

    // Stop tracking
    let allocations = ALLOCATOR.stop_tracking();

    // Assert zero heap allocations on the hot path
    assert_eq!(allocations, 0, "Expected 0 allocations on hot path, but recorded {}", allocations);
}
