mod common;

use bevy_ecs::prelude::*;
use glam::Vec3;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};
use anima_engine_lib::core::ecs::{
    detect_environmental_collisions_system, fruit_growth_system, lake_replenishment_system,
    seed_dropping_system, Agent, EnvironmentalSpawnSettings, EpochManager, FoodSpawnSettings, Lake,
    MapBounds, ParentAgent, Position, Prey, Tree,
};
use anima_engine_lib::core::simulation_lifecycle::{
    SavedSimulationState, SerializedLake, SerializedPheromoneGrid, SerializedTree,
    SimulationEngine, SimulationStatus,
};
use anima_engine_lib::evolution::meta_ai::EnvironmentalEvent;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

/// Stops the engine on the way out of the scope, including when an assertion panics on the way.
///
/// `TrackingAllocator` above is a `#[global_allocator]`, so it counts every allocation in the
/// process, not just the ones on the thread under test. An assertion that fires between `start()`
/// and `stop()` unwinds past the `stop()`, leaving the simulation, emit, learner and networking
/// threads running for the rest of the run — and the next test's zero-allocation measurement then
/// attributes their allocations to its own hot path. That is how one failure here produced a second,
/// unrelated-looking one reading "hot path should make 0 heap allocations, but made 130,946".
///
/// `TEST_LOCK` does not prevent it: the panicking test poisons the mutex, and the recovery path
/// (`unwrap_or_else(|e| e.into_inner())`) hands the lock straight to the next test while the leaked
/// threads are still live.
struct EngineGuard<'a>(&'a SimulationEngine);

impl Drop for EngineGuard<'_> {
    fn drop(&mut self) {
        self.0.stop();
    }
}

/// Disarms the process-wide counter if a measured system panics before the explicit read.
struct AllocationTrackingGuard;

impl Drop for AllocationTrackingGuard {
    fn drop(&mut self) {
        let _ = ALLOCATOR.stop_tracking();
    }
}

/// Polls until the background thread publishes a running status that has ticked, or the timeout
/// expires. Returns the last status seen either way, so the caller's assertions produce the message.
///
/// This replaced a flat `sleep(150ms)`, which was a bet on how long start-up takes rather than a
/// check that it happens. Under `--features desktop` that bet loses: `ml-wgpu` probes for a wgpu
/// adapter before the sim thread spawns, and enumerating adapters on a machine with a software or
/// otherwise slow GPU costs more than the whole 150 ms window. CI never saw it, because its cargo
/// test step sets `ANIMA_USE_GPU=0` and skips the probe entirely — so the failure only ever appeared
/// on a developer machine running the documented command.
fn wait_until_ticking(engine: &SimulationEngine, timeout: Duration) -> SimulationStatus {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let status = engine.get_status();
        if status.running && status.tick_count > 0 {
            return status;
        }
        if std::time::Instant::now() >= deadline {
            return status;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

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
            position: glam::Vec3::new(
                (i % 100) as f32 * 2.0 - 100.0,
                0.0,
                (i / 100) as f32 * 2.0 - 100.0,
            ),
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
        lakes: vec![SerializedLake {
            position: glam::Vec3::new(0.0, 0.0, 0.0),
            radius: 10.0,
            current_water: 100.0,
            max_water: 100.0,
            replenishment_rate: 5.0,
        }],
        trees,
        world_identity: Default::default(),
        // Everything not named above (closed-energy state, RNG stream position, season clock)
        // takes its zero value, which every restore path reads as "nothing was saved here".
        ..anima_engine_lib::core::simulation_state::empty_saved_state_for_tests()
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

        {
            // Guard scope: every assertion below runs with a stop() pending on unwind.
            let _guard = EngineGuard(&engine);

            let status = wait_until_ticking(&engine, Duration::from_secs(10));
            assert!(status.running, "Engine should be running");
            assert!(status.tick_count > 0, "Simulation should have ticked");

            // Make sure average tick time is within reasonable limits (no massive slowdowns)
            println!("Cycle {} status: {:?}", cycle, status);
            assert!(
                status.avg_tick_time_ms < 50.0,
                "Average tick time should be under 50ms, got {}",
                status.avg_tick_time_ms
            );
        }

        // Assert thread handles were taken and joined (which sets engine.threads to None or clears it)
        let threads_lock = engine.threads.lock().unwrap();
        assert!(
            threads_lock.is_none(),
            "Thread handles must be taken and joined successfully"
        );
    }
}

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
    // Stochastic systems draw from a declared stream; a world without one has no replay story.
    world.insert_resource(anima_engine_lib::core::resources::SimRng::from_seed(0x5EED));

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
            Position(Vec3::new(
                (i % 100) as f32 * 2.0 - 100.0,
                0.0,
                (i / 100) as f32 * 2.0 - 100.0,
            )),
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
            Position(Vec3::new(
                (i % 10) as f32 * 20.0 - 100.0,
                0.0,
                (i / 10) as f32 * 20.0 - 100.0,
            )),
            anima_engine_lib::physics::SpatialCollider { radius: 10.0 },
        ));
    }

    // Spawn 100 agents (heavy load)
    for i in 0..100 {
        let agent_entity = world
            .spawn((
                Agent,
                Prey,
                Position(Vec3::new(
                    (i % 10) as f32 * 15.0 - 75.0,
                    0.0,
                    (i / 10) as f32 * 15.0 - 75.0,
                )),
                HomeostaticState {
                    energy: 50.0,
                    energy_target: 100.0,
                    hydration: 50.0,
                    hydration_target: 100.0,
                    temperature: 37.0,
                    temp_target: 37.0,
                    previous_deviation: 0.0,
                },
            ))
            .id();

        // 3 segments per agent
        for j in 0..3 {
            world.spawn((
                Position(Vec3::new(
                    (i % 10) as f32 * 15.0 - 75.0 + j as f32 * 0.5,
                    0.0,
                    (i / 10) as f32 * 15.0 - 75.0,
                )),
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
    let _tracking_guard = AllocationTrackingGuard;
    for _ in 0..10 {
        schedule.run(&mut world);
    }
    let allocs = ALLOCATOR.stop_tracking();

    println!(
        "Allocations during environmental elements systems ticks: {}",
        allocs
    );
    assert_eq!(
        allocs, 0,
        "Environmental element systems hot path should make 0 heap allocations, but made {}",
        allocs
    );
}

/// One test thread keeps libtest's own thread startup outside the process-wide allocation window.
/// The allocation gate runs first so the lifecycle contract's simulation/backend threads cannot
/// contaminate it; the lifecycle helper owns an [`EngineGuard`] and joins those threads on return.
/// The helper names are intentionally not independently filterable.
#[test]
fn environmental_elements_stress_contracts() {
    eprintln!("allocation gate: maximum environmental load");
    let allocation_result =
        std::panic::catch_unwind(test_collision_logic_maximum_limits_zero_allocations);

    eprintln!("lifecycle gate: 10k trees");
    let lifecycle_result = std::panic::catch_unwind(test_10000_trees_spawning_and_lifecycle);

    // Run both contracts even on a red gate. The panic hook has already printed each failure; resume
    // the first one so libtest still marks the aggregate test failed with its original payload.
    if let Err(payload) = allocation_result {
        if lifecycle_result.is_err() {
            eprintln!("lifecycle gate also failed; see the panic above");
        }
        std::panic::resume_unwind(payload);
    }
    if let Err(payload) = lifecycle_result {
        std::panic::resume_unwind(payload);
    }
}
