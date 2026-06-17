mod common;

use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use std::thread;
use bevy_ecs::prelude::*;
use glam::Vec3;

use anima_engine_lib::core::ecs::{
    Agent, Prey, Position, ParentAgent, Lake, Tree, EnvironmentalSpawnSettings, MapBounds,
    fruit_growth_system, lake_replenishment_system, seed_dropping_system, detect_environmental_collisions_system,
};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::core::simulation_lifecycle::{SimulationEngine, SavedSimulationState, SerializedPheromoneGrid};
use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator = common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_fruit_growth_and_lake_replenishment() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(TimeStep(1.0));

    // Spawn a tree and a lake
    let tree_entity = world.spawn(Tree {
        current_fruit: 10.0,
        max_fruit: 100.0,
        fruit_growth_rate: 2.0,
        time_since_last_drop: 0.0,
        seed_drop_cooldown: 15.0,
        seed_spread_radius: 20.0,
    }).id();

    let lake_entity = world.spawn(Lake {
        current_water: 50.0,
        max_water: 100.0,
        replenishment_rate: 5.0,
    }).id();

    let mut schedule = Schedule::default();
    schedule.add_systems((fruit_growth_system, lake_replenishment_system));

    schedule.run(&mut world);

    let tree = world.get::<Tree>(tree_entity).unwrap();
    assert_eq!(tree.current_fruit, 12.0);

    let lake = world.get::<Lake>(lake_entity).unwrap();
    assert_eq!(lake.current_water, 55.0);

    // Test cap
    world.insert_resource(TimeStep(100.0));
    schedule.run(&mut world);

    let tree = world.get::<Tree>(tree_entity).unwrap();
    assert_eq!(tree.current_fruit, 100.0);

    let lake = world.get::<Lake>(lake_entity).unwrap();
    assert_eq!(lake.current_water, 100.0);
}

#[test]
fn test_seed_dropping_limits_and_bounds() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(TimeStep(15.0)); // Trigger cooldown immediately
    world.insert_resource(MapBounds {
        min: Vec3::new(-10.0, 0.0, -10.0),
        max: Vec3::new(10.0, 10.0, 10.0),
    });
    world.insert_resource(EnvironmentalSpawnSettings {
        max_tree_count: 3,
        default_lake_water: 500.0,
        default_lake_replenish: 5.0,
        default_tree_fruit: 100.0,
        default_tree_growth: 2.0,
        default_seed_cooldown: 15.0,
        default_seed_spread: 2.0,
    });

    // Spawn 1 parent tree
    world.spawn((
        Tree {
            current_fruit: 100.0,
            max_fruit: 100.0,
            fruit_growth_rate: 2.0,
            time_since_last_drop: 0.0,
            seed_drop_cooldown: 15.0,
            seed_spread_radius: 5.0,
        },
        Position(Vec3::new(0.0, 0.0, 0.0)),
        anima_engine_lib::physics::SpatialCollider { radius: 10.0 },
    ));

    let mut schedule = Schedule::default();
    schedule.add_systems(seed_dropping_system);

    // Run first time: drops 1 seed (total 2 trees)
    schedule.run(&mut world);
    let mut query = world.query::<&Tree>();
    let count = query.iter(&world).count();
    assert_eq!(count, 2);

    // Run second time: drops 1 seed (total 3 trees)
    // Advance time again to cooldown trigger
    for mut tree in world.query::<&mut Tree>().iter_mut(&mut world) {
        tree.time_since_last_drop = 15.0;
    }
    schedule.run(&mut world);
    let count = query.iter(&world).count();
    assert_eq!(count, 3);

    // Set time_since_last_drop to 15.0 again
    for mut tree in world.query::<&mut Tree>().iter_mut(&mut world) {
        tree.time_since_last_drop = 15.0;
    }
    // Now running it should NOT spawn any more trees because of max_tree_count = 3
    schedule.run(&mut world);
    let count = query.iter(&world).count();
    assert_eq!(count, 3);
}

#[test]
fn test_eating_and_drinking() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(TimeStep(1.0));

    // Spawn a Lake at (0, 0, 0)
    let lake_entity = world.spawn((
        Lake {
            current_water: 100.0,
            max_water: 100.0,
            replenishment_rate: 1.0,
        },
        Position(Vec3::new(0.0, 0.0, 0.0)),
        anima_engine_lib::physics::SpatialCollider { radius: 10.0 },
    )).id();

    // Spawn a Tree at (20, 0, 0)
    let tree_entity = world.spawn((
        Tree {
            current_fruit: 100.0,
            max_fruit: 100.0,
            fruit_growth_rate: 1.0,
            time_since_last_drop: 0.0,
            seed_drop_cooldown: 10.0,
            seed_spread_radius: 10.0,
        },
        Position(Vec3::new(20.0, 0.0, 0.0)),
        anima_engine_lib::physics::SpatialCollider { radius: 10.0 },
    )).id();

    // Spawn an agent at (0, 0, 0) (overlapping lake, far from tree)
    let agent_entity = world.spawn((
        Agent,
        Prey,
        Position(Vec3::new(0.0, 0.0, 0.0)),
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

    // Spawn agent's segment
    let segment_entity = world.spawn((
        Position(Vec3::new(0.0, 0.0, 0.0)),
        ParentAgent(agent_entity),
    )).id();

    let mut schedule = Schedule::default();
    schedule.add_systems(detect_environmental_collisions_system);

    // Run system
    schedule.run(&mut world);

    // Agent hydration should increase, lake water should decrease
    let agent_homeo = world.get::<HomeostaticState>(agent_entity).unwrap();
    assert!(agent_homeo.hydration > 50.0);
    assert_eq!(agent_homeo.energy, 50.0); // No eating since agent is far from tree

    let lake = world.get::<Lake>(lake_entity).unwrap();
    assert!(lake.current_water < 100.0);

    // Move agent to (20, 0, 0) overlapping tree
    if let Some(mut pos) = world.get_mut::<Position>(agent_entity) {
        pos.0 = Vec3::new(20.0, 0.0, 0.0);
    }
    if let Some(mut pos) = world.get_mut::<Position>(segment_entity) {
        pos.0 = Vec3::new(20.0, 0.0, 0.0);
    }

    // Run system again
    schedule.run(&mut world);

    let agent_homeo = world.get::<HomeostaticState>(agent_entity).unwrap();
    assert!(agent_homeo.energy > 50.0); // Prey eats from tree
    let tree = world.get::<Tree>(tree_entity).unwrap();
    assert!(tree.current_fruit < 100.0);
}

#[test]
fn test_environmental_collisions_zero_allocations() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(TimeStep(0.016));

    // Spawn Lake
    world.spawn((
        Lake {
            current_water: 100.0,
            max_water: 100.0,
            replenishment_rate: 1.0,
        },
        Position(Vec3::new(0.0, 0.0, 0.0)),
        anima_engine_lib::physics::SpatialCollider { radius: 5.0 },
    ));

    // Spawn Tree
    world.spawn((
        Tree {
            current_fruit: 100.0,
            max_fruit: 100.0,
            fruit_growth_rate: 1.0,
            time_since_last_drop: 0.0,
            seed_drop_cooldown: 10.0,
            seed_spread_radius: 10.0,
        },
        Position(Vec3::new(10.0, 0.0, 0.0)),
        anima_engine_lib::physics::SpatialCollider { radius: 5.0 },
    ));

    // Spawn Prey agent near the tree & lake
    let agent_entity = world.spawn((
        Agent,
        Prey,
        Position(Vec3::new(1.0, 0.0, 0.0)),
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

    world.spawn((
        Position(Vec3::new(1.0, 0.0, 0.0)),
        ParentAgent(agent_entity),
    ));

    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems(detect_environmental_collisions_system);

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

    assert_eq!(
        allocs, 0,
        "Environmental collision hot path should make 0 heap allocations, but made {}",
        allocs
    );
}

#[test]
fn test_spawning_10k_trees_performance_and_thread_leaks() {
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

    // Spawn a simulation with 10,000 tree entities
    let mut trees = Vec::with_capacity(10000);
    for i in 0..10000 {
        trees.push(anima_engine_lib::core::simulation_lifecycle::SerializedTree {
            position: glam::Vec3::new((i % 100) as f32 * 2.0, 0.0, (i / 100) as f32 * 2.0),
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
        active_environment_event: anima_engine_lib::evolution::meta_ai::EnvironmentalEvent::Stable,
        food_spawn_settings: anima_engine_lib::core::ecs::FoodSpawnSettings {
            max_food_count: 0,
            default_energy: 25.0,
            default_hydration: 15.0,
        },
        map_bounds: anima_engine_lib::core::ecs::MapBounds {
            min: glam::Vec3::new(-1000.0, 0.0, -1000.0),
            max: glam::Vec3::new(1000.0, 10.0, 1000.0),
        },
        epoch_manager: anima_engine_lib::core::ecs::EpochManager {
            ticks_per_epoch: 1000,
            current_epoch_ticks: 0,
            current_epoch: 0,
        },
        pheromone_grid: SerializedPheromoneGrid {
            values: vec![0.0; 16384],
            diffusion_rate: 0.12,
            decay_rate: 0.04,
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
        lakes: vec![],
        trees,
    };

    *engine.pending_load_state.lock().unwrap() = Some(state);

    let start_time = std::time::Instant::now();
    
    // Start engine (spawns simulation loop thread and others)
    engine.start::<tauri::test::MockRuntime>(
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_elites_grid),
    );

    // Let it run for 500ms to verify that 10k entities do not block/lock or leak threads
    thread::sleep(Duration::from_millis(500));

    // Verify it is running and ticking
    let status = engine.get_status();
    assert!(status.running, "Simulation should be running");
    assert!(status.tick_count > 0, "Simulation should have ticked");

    // Stop engine
    engine.stop();
    let elapsed = start_time.elapsed();
    println!("Simulation with 10k trees ran and stopped in {:?}", elapsed);

    // Verify status is not running
    let status_after = engine.get_status();
    assert!(!status_after.running, "Simulation should have stopped");

    // Verify threads joined successfully (Option is taken and set to None)
    let threads_lock = engine.threads.lock().unwrap();
    assert!(threads_lock.is_none(), "Threads should be joined and cleared, leaving no thread leaks");
}

#[test]
fn test_environmental_collisions_zero_allocations_heavy_load() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(TimeStep(0.016));

    // Spawn 100 Lakes
    for i in 0..100 {
        world.spawn((
            Lake {
                current_water: 100.0,
                max_water: 100.0,
                replenishment_rate: 1.0,
            },
            Position(Vec3::new(i as f32 * 10.0, 0.0, 0.0)),
            anima_engine_lib::physics::SpatialCollider { radius: 5.0 },
        ));
    }

    // Spawn 100 Trees
    for i in 0..100 {
        world.spawn((
            Tree {
                current_fruit: 100.0,
                max_fruit: 100.0,
                fruit_growth_rate: 1.0,
                time_since_last_drop: 0.0,
                seed_drop_cooldown: 10.0,
                seed_spread_radius: 10.0,
            },
            Position(Vec3::new(i as f32 * 10.0 + 5.0, 0.0, 0.0)),
            anima_engine_lib::physics::SpatialCollider { radius: 5.0 },
        ));
    }

    // Spawn 100 Agents, each with 5 segments
    for a in 0..100 {
        let agent_entity = world.spawn((
            Agent,
            Prey,
            Position(Vec3::new(a as f32 * 10.0 + 1.0, 0.0, 0.0)),
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

        for s in 0..5 {
            world.spawn((
                Position(Vec3::new(a as f32 * 10.0 + 1.0 + s as f32 * 0.1, 0.0, 0.0)),
                ParentAgent(agent_entity),
            ));
        }
    }

    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems(detect_environmental_collisions_system);

    // Warm up systems to populate Bevy archetype and query caches
    for _ in 0..10 {
        schedule.run(&mut world);
    }

    // Measure allocations on the hot path under heavy load
    ALLOCATOR.start_tracking();
    for _ in 0..10 {
        schedule.run(&mut world);
    }
    let allocs = ALLOCATOR.stop_tracking();

    assert_eq!(
        allocs, 0,
        "Environmental collision hot path under heavy load should make 0 heap allocations, but made {}",
        allocs
    );
}
