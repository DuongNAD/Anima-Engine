mod common;

use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use std::thread;
use bevy_ecs::prelude::*;
use glam::Vec3;

use anima_engine_lib::core::ecs::{
    Agent, Prey, Position, ParentAgent, Lake, Tree, EnvironmentalSpawnSettings, MapBounds,
    FoodSpawnSettings, EpochManager,
    fruit_growth_system, lake_replenishment_system, seed_dropping_system, detect_environmental_collisions_system,
};
use anima_engine_lib::core::simulation_lifecycle::{
    SimulationEngine, SavedSimulationState, SerializedPheromoneGrid, SerializedLake, SerializedTree,
};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::evolution::meta_ai::EnvironmentalEvent;
use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator = common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_10000_trees_spawning_and_lifecycle() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let engine = SimulationEngine::new();
    let evo_settings = Arc::new(Mutex::new(EvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    }));
    let evo_running = Arc::new(AtomicBool::new(false));
    let map_elites_grid = Arc::new(Mutex::new(MapElitesGridState {
        grid: std::collections::HashMap::new(),
        grid_resolution: 50,
    }));

    // Create a state with 10,000 trees
    let mut trees = Vec::with_capacity(10000);
    for i in 0..10000 {
        trees.push(SerializedTree {
            position: glam::Vec3::new((i % 100) as f32 * 2.0 - 100.0, 0.0, (i / 100) as f32 * 2.0 - 100.0),
            radius: 1.5,
            current_fruit: 50.0,
            max_fruit: 100.0,
            fruit_growth_rate: 2.0,
            time_since_last_drop: 0.0,
            seed_drop_cooldown: 15.0,
            seed_spread_radius: 20.0,
        });
    }

    let state = SavedSimulationState {
        tick_count: 0,
        active_environment_event: EnvironmentalEvent::Stable,
        food_spawn_settings: FoodSpawnSettings {
            max_food_count: 10,
            default_energy: 25.0,
            default_hydration: 15.0,
        },
        map_bounds: MapBounds {
            min: glam::Vec3::new(-150.0, 0.0, -150.0),
            max: glam::Vec3::new(150.0, 10.0, 150.0),
        },
        epoch_manager: EpochManager {
            ticks_per_epoch: 500,
            current_epoch_ticks: 0,
            current_epoch: 0,
        },
        pheromone_grid: SerializedPheromoneGrid {
            values: vec![0.0; 128 * 128],
            diffusion_rate: 0.1,
            decay_rate: 0.05,
        },
        foods: vec![],
        agents: vec![],
        evolution_settings: EvolutionSettings {
            mutation_rate: 0.15,
            selection_bias: 1.5,
            grid_resolution: 50,
        },
        map_elites_grid: MapElitesGridState {
            grid: std::collections::HashMap::new(),
            grid_resolution: 50,
        },
        chronicle_history: vec![],
        lineage_nodes: vec![],
        lineage_relations: vec![],
        lakes: vec![
            SerializedLake {
                position: glam::Vec3::new(0.0, 0.0, 0.0),
                radius: 10.0,
                current_water: 100.0,
                max_water: 100.0,
                replenishment_rate: 5.0,
            }
        ],
        trees,
    };

    // Run start/stop cycles to verify no thread leaks
    for cycle in 1..=3 {
        *engine.pending_load_state.lock().unwrap() = Some(state.clone());
        engine.start::<tauri::test::MockRuntime>(
            None,
            Arc::clone(&evo_settings),
            Arc::clone(&evo_running),
            Arc::clone(&map_elites_grid),
        );

        // Let the simulation run for a short duration
        thread::sleep(Duration::from_millis(150));

        let status = engine.get_status();
        assert!(status.running, "Engine should be running");
        assert!(status.tick_count > 0, "Simulation should have ticked");

        // Make sure average tick time is within reasonable limits (no massive slowdowns)
        println!("Cycle {} status: {:?}", cycle, status);
        assert!(status.avg_tick_time_ms < 50.0, "Average tick time should be under 50ms, got {}", status.avg_tick_time_ms);

        engine.stop();
        
        // Assert thread handles were taken and joined (which sets engine.threads to None or clears it)
        let threads_lock = engine.threads.lock().unwrap();
        assert!(threads_lock.is_none(), "Thread handles must be taken and joined successfully");
    }
}

#[test]
fn test_collision_logic_maximum_limits_zero_allocations() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(TimeStep(0.016));
    world.insert_resource(MapBounds {
        min: Vec3::new(-150.0, 0.0, -150.0),
        max: Vec3::new(150.0, 10.0, 150.0),
    });
    world.insert_resource(EnvironmentalSpawnSettings {
        max_tree_count: 10000,
        default_lake_water: 500.0,
        default_lake_replenish: 5.0,
        default_tree_fruit: 100.0,
        default_tree_growth: 2.0,
        default_seed_cooldown: 15.0,
        default_seed_spread: 20.0,
    });

    // Spawn 10,000 trees
    for i in 0..10000 {
        world.spawn((
            Tree {
                current_fruit: 50.0,
                max_fruit: 100.0,
                fruit_growth_rate: 2.0,
                time_since_last_drop: 0.0,
                seed_drop_cooldown: 15.0,
                seed_spread_radius: 20.0,
            },
            Position(Vec3::new((i % 100) as f32 * 2.0 - 100.0, 0.0, (i / 100) as f32 * 2.0 - 100.0)),
            anima_engine_lib::physics::SpatialCollider { radius: 1.5 },
        ));
    }

    // Spawn 100 lakes
    for i in 0..100 {
        world.spawn((
            Lake {
                current_water: 500.0,
                max_water: 500.0,
                replenishment_rate: 5.0,
            },
            Position(Vec3::new((i % 10) as f32 * 20.0 - 100.0, 0.0, (i / 10) as f32 * 20.0 - 100.0)),
            anima_engine_lib::physics::SpatialCollider { radius: 10.0 },
        ));
    }

    // Spawn 100 agents (heavy load)
    for i in 0..100 {
        let agent_entity = world.spawn((
            Agent,
            Prey,
            Position(Vec3::new((i % 10) as f32 * 15.0 - 75.0, 0.0, (i / 10) as f32 * 15.0 - 75.0)),
            HomeostaticState {
                energy: 50.0,
                energy_target: 100.0,
                hydration: 50.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
        )).id();

        // 3 segments per agent
        for j in 0..3 {
            world.spawn((
                Position(Vec3::new((i % 10) as f32 * 15.0 - 75.0 + j as f32 * 0.5, 0.0, (i / 10) as f32 * 15.0 - 75.0)),
                ParentAgent(agent_entity),
            ));
        }
    }

    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems((
        detect_environmental_collisions_system,
        fruit_growth_system,
        lake_replenishment_system,
        seed_dropping_system,
    ));

    // Warm up systems to populate Bevy archetype and query caches
    for _ in 0..10 {
        schedule.run(&mut world);
    }

    // Measure allocations on the hot path
    ALLOCATOR.start_tracking();
    for _ in 0..10 {
        schedule.run(&mut world);
    }
    let allocs = ALLOCATOR.stop_tracking();

    println!("Allocations during environmental elements systems ticks: {}", allocs);
    assert_eq!(
        allocs, 0,
        "Environmental element systems hot path should make 0 heap allocations, but made {}",
        allocs
    );
}
