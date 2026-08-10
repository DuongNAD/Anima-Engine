mod common;

use std::sync::{Arc, Mutex};
use bevy_ecs::prelude::*;
use anima_engine_lib::commands::EnvironmentSettings;
use anima_engine_lib::core::agent_systems::{
    sync_environment_settings_system, BevyEnvironmentSettings,
};
use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::core::ecs::{FloraSpawnSettings, MapBounds, EnvironmentState, ActiveEnvironmentEvent};
use anima_engine_lib::core::terrain::TerrainGrid;

static TEST_LOCK: Mutex<()> = Mutex::new(());

/// Helper to setup Bevy World with resources needed for sync_environment_settings_system
fn setup_test_world(settings: EnvironmentSettings) -> World {
    let mut world = World::new();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(FloraSpawnSettings::default());
    world.insert_resource(BevyEnvironmentSettings(Arc::new(Mutex::new(settings))));
    world
}

#[test]
fn test_time_speed_bounds() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // 1. Paused (time_speed = 0.0)
    let settings = EnvironmentSettings {
        time_speed: 0.0,
        fertility: 1.0,
        weather_severity: 1.0,
    };
    let mut world = setup_test_world(settings);
    let mut schedule = Schedule::default();
    schedule.add_systems(sync_environment_settings_system);
    schedule.run(&mut world);

    let ts = world.resource::<TimeStep>();
    assert_eq!(ts.0, 0.0, "TimeStep should be 0.0 when time_speed is 0.0");

    // 2. Extremely large (time_speed = 1000.0)
    let settings = EnvironmentSettings {
        time_speed: 1000.0,
        fertility: 1.0,
        weather_severity: 1.0,
    };
    let mut world = setup_test_world(settings);
    let mut schedule = Schedule::default();
    schedule.add_systems(sync_environment_settings_system);
    schedule.run(&mut world);

    let ts = world.resource::<TimeStep>();
    assert!((ts.0 - (5.0 / 60.0)).abs() < 1e-4, "TimeStep should be clamped to 5.0 (got: {})", ts.0);

    // 3. Negative speed (time_speed = -5.0)
    let settings = EnvironmentSettings {
        time_speed: -5.0,
        fertility: 1.0,
        weather_severity: 1.0,
    };
    let mut world = setup_test_world(settings);
    let mut schedule = Schedule::default();
    schedule.add_systems(sync_environment_settings_system);
    schedule.run(&mut world);

    let ts = world.resource::<TimeStep>();
    assert_eq!(ts.0, 0.0, "TimeStep should be clamped to 0.0 for negative speeds");

    // 4. NaN speed (time_speed = NaN)
    let settings = EnvironmentSettings {
        time_speed: f32::NAN,
        fertility: 1.0,
        weather_severity: 1.0,
    };
    let mut world = setup_test_world(settings);
    let mut schedule = Schedule::default();
    schedule.add_systems(sync_environment_settings_system);
    schedule.run(&mut world);

    let ts = world.resource::<TimeStep>();
    assert!((ts.0 - (1.0 / 60.0)).abs() < 1e-6, "TimeStep should default to 1/60 when time_speed is NaN");
}

#[test]
fn test_fertility_bounds() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // 1. Zero fertility
    let settings = EnvironmentSettings {
        time_speed: 1.0,
        fertility: 0.0,
        weather_severity: 1.0,
    };
    let mut world = setup_test_world(settings);
    let mut schedule = Schedule::default();
    schedule.add_systems(sync_environment_settings_system);
    schedule.run(&mut world);

    let flora = world.resource::<FloraSpawnSettings>();
    assert_eq!(flora.max_flora_count, 0, "max_flora_count should be 0 when fertility is 0.0");

    // 2. Negative fertility (fertility = -2.5)
    let settings = EnvironmentSettings {
        time_speed: 1.0,
        fertility: -2.5,
        weather_severity: 1.0,
    };
    let mut world = setup_test_world(settings);
    let mut schedule = Schedule::default();
    schedule.add_systems(sync_environment_settings_system);
    schedule.run(&mut world);

    let flora = world.resource::<FloraSpawnSettings>();
    // Since -750.0 as usize saturates to 0 in Rust, this should be 0.
    assert_eq!(flora.max_flora_count, 0, "max_flora_count should saturate to 0 for negative fertility");

    // 3. Extremely large fertility (fertility = 1e8)
    let settings = EnvironmentSettings {
        time_speed: 1.0,
        fertility: 1e8,
        weather_severity: 1.0,
    };
    let mut world = setup_test_world(settings);
    let mut schedule = Schedule::default();
    schedule.add_systems(sync_environment_settings_system);
    schedule.run(&mut world);

    let flora = world.resource::<FloraSpawnSettings>();
    let expected = (300.0_f32 * 3.0_f32) as usize; // clamped to 3.0
    assert_eq!(flora.max_flora_count, expected, "max_flora_count should scale to clamped limit of 900");

    // 4. NaN fertility
    let settings = EnvironmentSettings {
        time_speed: 1.0,
        fertility: f32::NAN,
        weather_severity: 1.0,
    };
    let mut world = setup_test_world(settings);
    let mut schedule = Schedule::default();
    schedule.add_systems(sync_environment_settings_system);
    schedule.run(&mut world);

    let flora = world.resource::<FloraSpawnSettings>();
    assert_eq!(flora.max_flora_count, 300, "max_flora_count should default to 300 for NaN fertility");
}

#[test]
fn test_simulation_under_settings_extremes() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // We initialize a world using init_world
    let mut world = anima_engine_lib::core::ecs::init_world();
    
    // Insert settings resources
    let shared_settings = Arc::new(Mutex::new(EnvironmentSettings {
        time_speed: -10.0, // negative time_speed
        fertility: 0.0,    // no flora spawn
        weather_severity: 5.0,
    }));
    world.insert_resource(BevyEnvironmentSettings(shared_settings.clone()));

    // Create schedule with systems
    let mut schedule = Schedule::default();
    schedule.add_systems((
        sync_environment_settings_system,
        anima_engine_lib::core::ecs::update_day_night_cycle_system,
        anima_engine_lib::core::ecs::grow_flora_system,
        anima_engine_lib::core::ecs::wither_flora_system,
        anima_engine_lib::core::ecs::spawn_flora_system,
    ));

    // Run for 10 ticks and ensure no panic/crash occurs
    for _ in 0..10 {
        schedule.run(&mut world);
    }

    // Now test with NaN and Infinity values
    {
        let mut settings = shared_settings.lock().unwrap();
        settings.time_speed = f32::NAN;
        settings.fertility = f32::INFINITY;
    }
    for _ in 0..10 {
        schedule.run(&mut world);
    }

    // Ensure it runs without panicking
    assert!(true);
}

#[test]
fn test_settings_high_rate_thread_safety() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let shared_settings = Arc::new(Mutex::new(EnvironmentSettings {
        time_speed: 1.0,
        fertility: 1.0,
        weather_severity: 1.0,
    }));

    let settings_writer = shared_settings.clone();
    let stop_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_signal_writer = stop_signal.clone();

    // Spawn a writer thread updating settings at high rate
    let writer_handle = std::thread::spawn(move || {
        let mut count = 0;
        while !stop_signal_writer.load(std::sync::atomic::Ordering::Relaxed) {
            let speed = (count % 10) as f32;
            let fert = (count % 5) as f32;
            if let Ok(mut settings) = settings_writer.try_lock() {
                settings.time_speed = speed;
                settings.fertility = fert;
            }
            count += 1;
            std::thread::yield_now();
        }
    });

    let mut world = World::new();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(FloraSpawnSettings::default());
    world.insert_resource(BevyEnvironmentSettings(shared_settings));

    let mut schedule = Schedule::default();
    schedule.add_systems(sync_environment_settings_system);

    // Run the sync system repeatedly to simulate high rate updates reading it
    for _ in 0..1000 {
        schedule.run(&mut world);
    }

    // Stop writer thread
    stop_signal.store(true, std::sync::atomic::Ordering::Relaxed);
    writer_handle.join().unwrap();

    assert!(true);
}
