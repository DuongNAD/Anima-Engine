mod common;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::thread;
use rand::Rng;

use bevy_ecs::prelude::*;
use glam::Vec3;

use anima_engine_lib::core::ecs::{
    init_world, wrap_coordinates_system, combat_system, Position, Rotation, ParentAgent, Segment,
    FoodSpawnSettings, spawn_food_system, detect_food_collisions_system, MapBounds,
    SegmentJointForce, Velocity, EpochManager, FeatureTracker, Prey, AgentParentLineageIds
};
use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::{Transition, TransitionSender, HomeostaticState};
use anima_engine_lib::evolution::genotype::{MorphologyGenotype, MorphologyNode};
use anima_engine_lib::evolution::lineage::{
    FallbackLineageTracker, LineageTracker, RelationType
};
use anima_engine_lib::core::engine::{
    BevyEvolutionSettings, BevyEvolutionRunning, BevyMapElitesGrid, BevyAppHandle,
    ActiveEvolutionSettings, BevyMapElitesArchive, NextNodeId, EvolutionSender, EvolutionReceiver,
    AgentEpochStats, AgentGenotype, AgentEvaluation, AgentLineageId, AgentGeneration, EvolutionQueue,
};

// Bind to tracking allocator
#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator = common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_lineage_tracker_concurrency_and_fallback() {
    let _lock = TEST_LOCK.lock().unwrap();

    // Start with an offline address. Graces fallback to offline immediately without crash.
    let tracker = Arc::new(FallbackLineageTracker::new("bolt://localhost:9999", "neo4j", "password"));
    assert!(!tracker.is_online(), "FallbackLineageTracker should report offline for bad address");

    let num_threads: usize = 12;
    let ops_per_thread: usize = 40;
    let mut handles = vec![];

    let genotype = {
        let mut g = MorphologyGenotype::new();
        g.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.2, mass: 1.0 });
        g
    };

    // 1. Spawning multiple writer threads to test concurrent add_root and add_reproduction
    for thread_idx in 0..num_threads {
        let tracker_clone = Arc::clone(&tracker);
        let gen_clone = genotype.clone();
        
        let handle = thread::spawn(move || {
            let mut rng = rand::thread_rng();
            for op_idx in 0..ops_per_thread {
                let id = format!("thread-{}-node-{}", thread_idx, op_idx);
                if op_idx == 0 {
                    assert!(tracker_clone.add_root(id, gen_clone.clone()).is_ok());
                } else {
                    let parent = format!("thread-{}-node-{}", thread_idx, op_idx - 1);
                    assert!(tracker_clone.add_reproduction(
                        id,
                        op_idx as u32,
                        gen_clone.clone(),
                        vec![parent],
                        RelationType::Clone,
                    ).is_ok());
                }

                // Random yielding
                if rng.gen_bool(0.1) {
                    thread::sleep(Duration::from_millis(1));
                }
            }
        });
        handles.push(handle);
    }

    // 2. Spawn concurrent reader thread
    let tracker_reader = Arc::clone(&tracker);
    let reader_running = Arc::new(AtomicBool::new(true));
    let reader_running_clone = Arc::clone(&reader_running);
    let reader_handle = thread::spawn(move || {
        let mut rng = rand::thread_rng();
        while reader_running_clone.load(Ordering::SeqCst) {
            let _ = tracker_reader.get_lineage_graph();
            if rng.gen_bool(0.2) {
                thread::sleep(Duration::from_millis(1));
            }
        }
    });

    // 3. Spawn concurrent offline trigger thread
    let tracker_offline = Arc::clone(&tracker);
    let offline_running = Arc::new(AtomicBool::new(true));
    let offline_running_clone = Arc::clone(&offline_running);
    let offline_handle = thread::spawn(move || {
        let mut rng = rand::thread_rng();
        while offline_running_clone.load(Ordering::SeqCst) {
            tracker_offline.mark_offline();
            if rng.gen_bool(0.2) {
                thread::sleep(Duration::from_millis(2));
            }
        }
    });

    // Wait for all writers to finish
    for handle in handles {
        handle.join().unwrap();
    }

    // Stop helper threads
    reader_running.store(false, Ordering::SeqCst);
    offline_running.store(false, Ordering::SeqCst);
    reader_handle.join().unwrap();
    offline_handle.join().unwrap();

    // Verify consistency
    let (nodes, relations) = tracker.get_lineage_graph().unwrap();
    assert_eq!(nodes.len(), num_threads * ops_per_thread);
    assert_eq!(relations.len(), num_threads * (ops_per_thread - 1));
}

#[test]
fn test_ecs_hot_path_zero_heap_allocations() {
    let _lock = TEST_LOCK.lock().unwrap();

    let mut world = init_world();
    
    // Insert spatial hash grid resources
    let bounds = MapBounds {
        min: Vec3::new(-100.0, 0.0, -100.0),
        max: Vec3::new(100.0, 10.0, 100.0),
    };
    world.insert_resource(bounds);
    let grid = anima_engine_lib::physics::SpatialHashGrid::new_prepopulated(10.0, &bounds);
    world.insert_resource(grid);

    world.insert_resource(anima_engine_lib::ai::pheromone::PheromoneGrid::default());
    world.insert_resource(FoodSpawnSettings::default());
    world.insert_resource(TimeStep(1.0 / 60.0));

    // Dummy channels
    let (trans_tx, _trans_rx) = crossbeam_channel::bounded::<Transition>(4096);
    let (stats_tx, _stats_rx) = crossbeam_channel::bounded::<Vec<AgentEpochStats>>(128);
    let (_spawn_tx, spawn_rx) = crossbeam_channel::bounded::<(Entity, MorphologyGenotype, glam::Vec3, String, u32, Vec<String>)>(128);

    world.insert_resource(TransitionSender(trans_tx));
    world.insert_resource(EvolutionSender(stats_tx));
    world.insert_resource(EvolutionReceiver(spawn_rx));

    let evolution_settings = Arc::new(std::sync::Mutex::new(anima_engine_lib::commands::EvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    }));
    let evolution_running = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let map_elites_grid = Arc::new(std::sync::Mutex::new(anima_engine_lib::commands::MapElitesGridState {
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
    world.insert_resource(EpochManager {
        ticks_per_epoch: 1000,
        current_epoch_ticks: 0,
        current_epoch: 0,
    });
    world.insert_resource(EvolutionQueue::default());

    // Setup schedule systems (excluding neural network / HRRL update systems to avoid NdArray CPU tensor allocations)
    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems((
        anima_engine_lib::core::engine::sync_evolution_settings_system,
        anima_engine_lib::ai::cpg::update_cpg_system,
        anima_engine_lib::physics::resolve_joints_system.after(anima_engine_lib::ai::cpg::update_cpg_system),
        anima_engine_lib::physics::integrate_physics_system.after(anima_engine_lib::physics::resolve_joints_system),
        anima_engine_lib::ai::pheromone::agent_release_pheromone_system.after(anima_engine_lib::physics::integrate_physics_system),
        anima_engine_lib::ai::pheromone::update_pheromone_grid_system.after(anima_engine_lib::ai::pheromone::agent_release_pheromone_system),
        anima_engine_lib::ai::pheromone::agent_read_pheromone_system.after(anima_engine_lib::ai::pheromone::update_pheromone_grid_system),
        anima_engine_lib::core::engine::update_agent_evaluation_system.after(anima_engine_lib::physics::integrate_physics_system),
        wrap_coordinates_system.after(anima_engine_lib::physics::integrate_physics_system),
        anima_engine_lib::physics::rebuild_spatial_grid_system.after(wrap_coordinates_system),
        anima_engine_lib::core::ecs::metabolic_decay_system.after(anima_engine_lib::physics::integrate_physics_system),
        spawn_food_system.after(anima_engine_lib::physics::integrate_physics_system),
        detect_food_collisions_system.after(anima_engine_lib::physics::integrate_physics_system),
        combat_system.after(anima_engine_lib::physics::integrate_physics_system),
        anima_engine_lib::core::engine::check_epoch_completion_system.after(anima_engine_lib::core::ecs::metabolic_decay_system),
        anima_engine_lib::core::engine::apply_staggered_evolution_system.after(anima_engine_lib::core::engine::check_epoch_completion_system),
    ));

    // Spawn 1 agent with lineage components attached (representing typical simulation state)
    let initial_pos = glam::Vec3::new(0.0, 0.0, 0.0);
    let agent_entity = world.spawn((
        Position(initial_pos),
        Rotation(glam::Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        HomeostaticState {
            energy: 100.0,
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        AgentGenotype(MorphologyGenotype::default()),
        AgentEvaluation {
            start_position: initial_pos,
            total_distance: 0.0,
            total_energy_expended: 0.0,
            survival_ticks: 0,
            last_position: initial_pos,
        },
        FeatureTracker::default(),
        AgentLineageId("some-test-lineage-id".to_string()),
        AgentGeneration(0),
        AgentParentLineageIds(Vec::new()),
        Prey,
    )).id();

    // Spawn segment entities for agent
    world.spawn((
        ParentAgent(agent_entity),
        Position(initial_pos),
        Rotation(glam::Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 0, length: 1.0, radius: 0.2, mass: 1.0 },
        anima_engine_lib::physics::dynamics::RigidBody {
            mass: 1.0,
            velocity: Vec3::ZERO,
            force: Vec3::ZERO,
        },
        SegmentJointForce(0.0),
    ));

    // Warm-up to initialize internal Bevy archetype arrays & cache query states
    for _ in 0..100 {
        schedule.run(&mut world);
    }

    // Start tracking allocations
    ALLOCATOR.start_tracking();

    // Run active hot tick loop (no epoch completions, no spawn queues)
    for _ in 0..100 {
        schedule.run(&mut world);
    }

    // Stop tracking
    let allocations = ALLOCATOR.stop_tracking();

    // Assert zero heap allocations on the hot path (conforming to Phase 4 zero-alloc spec)
    assert_eq!(allocations, 0, "Expected 0 allocations on hot path ticks, but recorded {}", allocations);
}
