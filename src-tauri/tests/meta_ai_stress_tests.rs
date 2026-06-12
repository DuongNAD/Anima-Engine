use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use bevy_ecs::prelude::*;
use glam::Vec3;
use crossbeam_channel;

use anima_engine_lib::core::ecs::{
    apply_environmental_effects_system, init_world, metabolic_decay_system,
    spawn_food_system, detect_food_collisions_system, combat_system,
    ActiveEnvironmentEvent, FoodSpawnSettings, Agent, Predator, Prey,
    Position, Velocity, ParentAgent, Food, FeatureTracker,
    SegmentJointForce
};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::physics::dynamics::RigidBody;
use anima_engine_lib::evolution::meta_ai::{
    EnvironmentalEvent, MetaAiClient, MockMetaAiClient, GeminiMetaAiClient
};
use anima_engine_lib::core::engine::EnvironmentalEventReceiver;

// Thread-local allocation tracker
struct TrackingAllocator;

thread_local! {
    static ALLOC_COUNT: Cell<usize> = Cell::new(0);
    static TRACK_ALLOC: Cell<bool> = Cell::new(false);
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK_ALLOC.with(|t| t.get()) {
            ALLOC_COUNT.with(|c| c.set(c.get() + 1));
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static A: TrackingAllocator = TrackingAllocator;

fn set_tracking(track: bool) {
    TRACK_ALLOC.with(|t| t.set(track));
}

fn get_alloc_count() -> usize {
    ALLOC_COUNT.with(|c| c.get())
}

fn reset_alloc_count() {
    ALLOC_COUNT.with(|c| c.set(0));
}

#[test]
fn test_stress_environmental_transitions_and_boundaries() {
    let mut world = World::new();
    world.insert_resource(ActiveEnvironmentEvent::default());
    world.insert_resource(FoodSpawnSettings::default());

    // Spawn some test agents
    let agent_1 = world.spawn((
        Agent,
        HomeostaticState {
            energy: 100.0,
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        }
    )).id();

    let agent_2 = world.spawn((
        Agent,
        HomeostaticState {
            energy: 80.0,
            energy_target: 100.0,
            hydration: 90.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        }
    )).id();

    let mut schedule = Schedule::default();
    schedule.add_systems(apply_environmental_effects_system);

    // Apply stable
    world.insert_resource(ActiveEnvironmentEvent(EnvironmentalEvent::Stable));
    schedule.run(&mut world);
    assert_eq!(world.resource::<FoodSpawnSettings>().max_food_count, 50);
    assert_eq!(world.get::<HomeostaticState>(agent_1).unwrap().temp_target, 37.0);
    assert_eq!(world.get::<HomeostaticState>(agent_2).unwrap().temp_target, 37.0);

    // Apply Drought
    world.insert_resource(ActiveEnvironmentEvent(EnvironmentalEvent::ResourceDrought));
    schedule.run(&mut world);
    assert_eq!(world.resource::<FoodSpawnSettings>().max_food_count, 25);
    assert_eq!(world.get::<HomeostaticState>(agent_1).unwrap().temp_target, 37.0);

    // Apply TemperatureSpike
    world.insert_resource(ActiveEnvironmentEvent(EnvironmentalEvent::TemperatureSpike));
    schedule.run(&mut world);
    assert_eq!(world.resource::<FoodSpawnSettings>().max_food_count, 50);
    assert_eq!(world.get::<HomeostaticState>(agent_1).unwrap().temp_target, 42.0);

    // Apply GlacialPeriod
    world.insert_resource(ActiveEnvironmentEvent(EnvironmentalEvent::GlacialPeriod));
    schedule.run(&mut world);
    assert_eq!(world.resource::<FoodSpawnSettings>().max_food_count, 50);
    assert_eq!(world.get::<HomeostaticState>(agent_1).unwrap().temp_target, 32.0);

    // Apply ToxicDeluge
    world.insert_resource(ActiveEnvironmentEvent(EnvironmentalEvent::ToxicDeluge));
    schedule.run(&mut world);
    assert_eq!(world.resource::<FoodSpawnSettings>().max_food_count, 40);
    assert_eq!(world.get::<HomeostaticState>(agent_1).unwrap().temp_target, 37.0);

    // Stress: Simultaneous/rapid transitions back and forth
    for _ in 0..100 {
        world.insert_resource(ActiveEnvironmentEvent(EnvironmentalEvent::TemperatureSpike));
        schedule.run(&mut world);
        assert_eq!(world.get::<HomeostaticState>(agent_1).unwrap().temp_target, 42.0);

        world.insert_resource(ActiveEnvironmentEvent(EnvironmentalEvent::GlacialPeriod));
        schedule.run(&mut world);
        assert_eq!(world.get::<HomeostaticState>(agent_1).unwrap().temp_target, 32.0);
    }
}

#[test]
fn test_rate_limiting_and_offline_fallback() {
    // GeminiMetaAiClient fallback when key is invalid
    std::env::remove_var("GEMINI_API_KEY");
    let client = GeminiMetaAiClient::new(Duration::from_millis(10));
    
    // Should fall back to MockMetaAiClient behavior seamlessly
    let event = client.generate_event(1, &[]);
    assert_eq!(event, EnvironmentalEvent::ResourceDrought);

    let event_2 = client.generate_event(2, &[]);
    assert_eq!(event_2, EnvironmentalEvent::TemperatureSpike);

    // Mock Rate Limit Test using custom wrap client
    struct RateLimitingClient {
        limit: usize,
        counter: AtomicUsize,
        fallback_counter: AtomicUsize,
    }

    impl MetaAiClient for RateLimitingClient {
        fn generate_event(&self, epoch: u32, history: &[EnvironmentalEvent]) -> EnvironmentalEvent {
            let current = self.counter.fetch_add(1, Ordering::SeqCst);
            if current >= self.limit {
                self.fallback_counter.fetch_add(1, Ordering::SeqCst);
                // Fall back to Mock client logic under rate limiting
                MockMetaAiClient.generate_event(epoch, history)
            } else {
                EnvironmentalEvent::Stable
            }
        }
    }

    let rate_limiter = RateLimitingClient {
        limit: 5,
        counter: AtomicUsize::new(0),
        fallback_counter: AtomicUsize::new(0),
    };

    // First 5 requests go through normal path
    for i in 0..5 {
        assert_eq!(rate_limiter.generate_event(i, &[]), EnvironmentalEvent::Stable);
    }
    // Requests starting from 6th hit the limit and fallback to mock behavior
    let next_event = rate_limiter.generate_event(6, &[]);
    // epoch 6 % 5 = 1 => ResourceDrought
    assert_eq!(next_event, EnvironmentalEvent::ResourceDrought);
    assert_eq!(rate_limiter.fallback_counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_non_blocking_event_trigger_processing() {
    let (tx, rx) = crossbeam_channel::bounded::<EnvironmentalEvent>(10000);
    let mut world = World::new();
    world.insert_resource(EnvironmentalEventReceiver(rx));
    world.insert_resource(ActiveEnvironmentEvent::default());

    let mut schedule = Schedule::default();
    schedule.add_systems(anima_engine_lib::core::engine::receive_environmental_events_system);

    // 1. Confirm non-blocking try_recv on empty channel does not block
    let start_empty = Instant::now();
    schedule.run(&mut world);
    let elapsed_empty = start_empty.elapsed();
    assert!(elapsed_empty < Duration::from_millis(1), "Empty channel processing took too long: {:?}", elapsed_empty);

    // 2. Queue up 1000 simultaneous adjustments (stress test)
    for _ in 0..1000 {
        tx.send(EnvironmentalEvent::TemperatureSpike).unwrap();
    }
    tx.send(EnvironmentalEvent::ResourceDrought).unwrap(); // last one is ResourceDrought

    let start_heavy = Instant::now();
    schedule.run(&mut world);
    let elapsed_heavy = start_heavy.elapsed();

    // Confirm last event was received and processed
    let active = world.resource::<ActiveEnvironmentEvent>();
    assert_eq!(active.0, EnvironmentalEvent::ResourceDrought);
    assert!(elapsed_heavy < Duration::from_millis(5), "Heavy event processing took too long: {:?}", elapsed_heavy);
}

#[test]
fn test_zero_dynamic_allocations_on_hot_path() {
    let mut world = init_world();
    world.insert_resource(FoodSpawnSettings {
        max_food_count: 50,
        default_energy: 30.0,
        default_hydration: 20.0,
    });

    // Spawn 10 agents (5 predators, 5 prey)
    let mut agents = Vec::new();
    for i in 0..10 {
        let is_predator = i < 5;
        let pos = Vec3::new(i as f32 * 10.0, 0.0, 0.0);
        let agent = world.spawn((
            Agent,
            Position(pos),
            Velocity(Vec3::ZERO),
            HomeostaticState {
                energy: 50.0,
                energy_target: 100.0,
                hydration: 50.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
            FeatureTracker::default(),
        )).id();
        if is_predator {
            world.entity_mut(agent).insert(Predator);
        } else {
            world.entity_mut(agent).insert(Prey);
        }
        agents.push(agent);
    }

    // Spawn 3 segments for each agent to exercise parent link iterations in metabolic_decay_system
    for &agent in &agents {
        for _ in 0..3 {
            world.spawn((
                ParentAgent(agent),
                RigidBody {
                    mass: 1.0,
                    velocity: Vec3::ZERO,
                    force: Vec3::ZERO,
                },
                Velocity(Vec3::ZERO),
                Position(Vec3::ZERO),
                SegmentJointForce(0.0),
            ));
        }
    }

    // Pre-spawn 50 foods so spawn_food_system has 0 work on hot path
    for k in 0..50 {
        world.spawn((
            Food {
                energy_value: 30.0,
                hydration_value: 20.0,
            },
            Position(Vec3::new(k as f32 * 2.0, 0.0, 50.0)), // Place far from agents to avoid despawns
            anima_engine_lib::physics::SpatialCollider { radius: 0.5 },
        ));
    }

    // Insert dummy channel receiver
    let (_, rx) = crossbeam_channel::bounded::<EnvironmentalEvent>(32);
    world.insert_resource(EnvironmentalEventReceiver(rx));

    let mut schedule = Schedule::default();
    schedule.add_systems((
        anima_engine_lib::core::engine::receive_environmental_events_system,
        apply_environmental_effects_system,
        metabolic_decay_system,
        spawn_food_system,
        detect_food_collisions_system,
        combat_system,
    ));

    // Warm up the systems to initialize queries and entity caches
    schedule.run(&mut world);
    schedule.run(&mut world);

    // Warm up allocation tracker thread-local (pre-initialize it)
    reset_alloc_count();
    set_tracking(true);
    let _x = Box::new(42); // will trigger tracking allocator
    set_tracking(false);
    assert!(get_alloc_count() > 0);
    reset_alloc_count();

    // Now run hot path under tracking allocator
    set_tracking(true);
    
    // Execute 10 ticks
    for _ in 0..10 {
        schedule.run(&mut world);
    }

    set_tracking(false);

    let allocations = get_alloc_count();
    println!("Total dynamic heap allocations in 10 ECS hot-path ticks: {}", allocations);
    assert_eq!(allocations, 0, "Expected exactly zero allocations on the hot path, but found {}", allocations);
}
