mod common;

use bevy_ecs::prelude::*;
use glam::Vec3;
use std::sync::Mutex;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};

use anima_engine_lib::evolution::meta_ai::{
    EnvironmentalEvent, MetaAiClient, GeminiMetaAiClient
};
use anima_engine_lib::core::ecs::{
    Agent, Predator, Prey, Position, Velocity, MapBounds,
    Food, FoodSpawnSettings, ActiveEnvironmentEvent, Segment, ParentAgent,
    apply_environmental_effects_system, metabolic_decay_system, spawn_food_system,
    detect_food_collisions_system, combat_system, CombatEvents
};
use anima_engine_lib::core::engine::{
    EnvironmentalEventReceiver,
    receive_environmental_events_system
};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::ai::cpg::TimeStep;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

/// Verify that GeminiMetaAiClient falls back to MockMetaAiClient when API key is invalid
/// or connection times out, and handles boundaries properly.
#[test]
fn test_gemini_client_fallback_robustness() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // 1. Invalid API Key test
    std::env::set_var("GEMINI_API_KEY", "invalid_api_key");
    let client = GeminiMetaAiClient::new(Duration::from_millis(50));
    
    // The HTTP request will fail because the API key is invalid.
    // It should fall back to MockMetaAiClient.
    let event = client.generate_event(1, &[]);
    assert_eq!(event, EnvironmentalEvent::ResourceDrought, "Should fallback to Mock Client Epoch 1");
    
    let event2 = client.generate_event(2, &[]);
    assert_eq!(event2, EnvironmentalEvent::TemperatureSpike, "Should fallback to Mock Client Epoch 2");

    // 2. Timeout boundary test (1 nanosecond timeout)
    let client_timeout = GeminiMetaAiClient {
        api_key: Some("dummy_key".to_string()),
        timeout: Duration::from_nanos(1), // Extremely short timeout to force a timeout error
    };
    
    let start = std::time::Instant::now();
    let event_timeout = client_timeout.generate_event(3, &[]);
    let elapsed = start.elapsed();
    
    assert_eq!(event_timeout, EnvironmentalEvent::GlacialPeriod, "Should fallback immediately to Mock Client Epoch 3");
    assert!(elapsed < Duration::from_millis(100), "Should time out and fallback extremely quickly");
}

/// Verify that the environmental triggers and channel processing are non-blocking and do not
/// cause frame lag, even if the background meta-AI client is extremely slow (e.g. 100ms delay).
#[test]
fn test_environmental_systems_non_blocking_performance() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    
    let (env_tx, env_rx) = crossbeam_channel::bounded::<EnvironmentalEvent>(32);
    
    // Simulate background thread generating events slowly (e.g. network latency)
    let bg_running = std::sync::Arc::new(AtomicBool::new(true));
    let bg_running_clone = bg_running.clone();
    let bg_thread = std::thread::spawn(move || {
        let mut epoch = 0;
        while bg_running_clone.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(100)); // Simulate slow LLM generation
            epoch += 1;
            let event = match epoch % 3 {
                1 => EnvironmentalEvent::TemperatureSpike,
                2 => EnvironmentalEvent::ResourceDrought,
                _ => EnvironmentalEvent::Stable,
            };
            let _ = env_tx.send(event);
        }
    });

    // Initialize ECS World and resources
    let mut world = World::new();
    world.insert_resource(ActiveEnvironmentEvent::default());
    world.insert_resource(EnvironmentalEventReceiver(env_rx));
    world.insert_resource(FoodSpawnSettings::default());
    
    let mut schedule = Schedule::default();
    schedule.add_systems((
        receive_environmental_events_system,
        apply_environmental_effects_system.after(receive_environmental_events_system),
    ));
    
    // Run warmup
    schedule.run(&mut world);

    // Measure tick timing in the simulated main thread
    let start_time = std::time::Instant::now();
    let tick_count = 50;
    
    for _ in 0..tick_count {
        let tick_start = std::time::Instant::now();
        schedule.run(&mut world);
        let tick_duration = tick_start.elapsed();
        // Each tick must be extremely fast (< 20ms) because try_recv is non-blocking
        assert!(tick_duration < Duration::from_millis(20), "Tick took too long: {:?}", tick_duration);
        std::thread::sleep(Duration::from_millis(16)); // Target ~60 FPS
    }
    
    let total_elapsed = start_time.elapsed();
    assert!(total_elapsed < Duration::from_millis(1200), "Overall execution took too long: {:?}", total_elapsed);
    
    // Clean up
    bg_running.store(false, Ordering::SeqCst);
    let _ = bg_thread.join();
}

/// Verify that I22 hot path systems preserve the zero-dynamic-allocations contract.
#[test]
fn test_ecs_hot_path_zero_allocations_i22() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(MapBounds::default());
    world.insert_resource(ActiveEnvironmentEvent::default());
    
    // Set max food count to 50
    let food_settings = FoodSpawnSettings {
        max_food_count: 50,
        default_energy: 30.0,
        default_hydration: 20.0,
    };
    world.insert_resource(food_settings);

    // Bounded channels and receiver
    let (_env_tx, env_rx) = crossbeam_channel::bounded::<EnvironmentalEvent>(32);
    world.insert_resource(EnvironmentalEventReceiver(env_rx));

    // CombatEvents pre-allocated to avoid dynamic capacity growth
    world.insert_resource(CombatEvents {
        events: Vec::with_capacity(100),
        predator_centroids: Vec::with_capacity(128),
        prey_centroids: Vec::with_capacity(128),
    });

    // Spawn 10 Agents (7 prey, 3 predators)
    let mut agents = Vec::new();
    for i in 0..10 {
        let is_predator = i >= 7;
        let pos = Vec3::new(i as f32 * 2.0, 0.0, 0.0);
        let entity = world.spawn((
            Agent,
            Position(pos),
            Velocity(Vec3::new(0.1, 0.0, 0.0)),
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

        if is_predator {
            world.entity_mut(entity).insert(Predator);
        } else {
            world.entity_mut(entity).insert(Prey);
        }

        // Spawn a child segment for metabolic decay system
        world.spawn((
            Segment { id: 0, length: 1.0, radius: 0.2, mass: 1.0 },
            ParentAgent(entity),
            anima_engine_lib::physics::dynamics::RigidBody {
                mass: 1.0,
                velocity: Vec3::ZERO,
                force: Vec3::ZERO,
            },
            Velocity(Vec3::ZERO),
        ));

        agents.push(entity);
    }

    // Spawn 50 food entities to reach the max capacity (so spawn_food_system does not allocate new food)
    for i in 0..50 {
        world.spawn((
            Food { energy_value: 30.0, hydration_value: 20.0 },
            Position(Vec3::new(i as f32 * 5.0 + 100.0, 0.0, 100.0)), // Far away to avoid collisions during warmup
            anima_engine_lib::physics::SpatialCollider { radius: 0.5 },
        ));
    }

    let mut schedule = Schedule::default();
    schedule.add_systems((
        receive_environmental_events_system,
        apply_environmental_effects_system.after(receive_environmental_events_system),
        metabolic_decay_system,
        spawn_food_system,
        detect_food_collisions_system,
        combat_system,
    ));

    // Run warmup to compile systems and initialize query caches
    schedule.run(&mut world);

    // Verify setup
    assert_eq!(world.query::<&Food>().iter(&world).count(), 50);

    // Start tracking allocations
    ALLOCATOR.start_tracking();

    // Run 10 ticks of the schedule
    for _ in 0..10 {
        schedule.run(&mut world);
    }

    // Stop tracking
    let allocations = ALLOCATOR.stop_tracking();

    assert_eq!(allocations, 0, "Expected exactly 0 heap allocations during the hot path tick loop, but got {}", allocations);
}
