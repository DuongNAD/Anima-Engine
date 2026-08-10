mod common;

use bevy_ecs::prelude::*;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::core::ecs::{
    Position, MapBounds, EnvironmentState, ActiveEnvironmentEvent,
    Flora, FloraSpawnSettings,
    update_day_night_cycle_system, grow_flora_system, wither_flora_system, spawn_flora_system
};
use anima_engine_lib::core::terrain::TerrainGrid;
use anima_engine_lib::core::agent_systems::{
    BevyEnvironmentSettings, sync_environment_settings_system
};
use anima_engine_lib::commands::EnvironmentSettings;

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_stress_boundary_time_speed() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let cases = vec![
        0.0,
        10.0,
        100.0,
        -5.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ];

    for &time_speed in &cases {
        let mut world = World::new();

        // Initialize resources
        world.insert_resource(TimeStep(1.0 / 60.0));
        world.insert_resource(FloraSpawnSettings::default());
        world.insert_resource(MapBounds::default());
        world.insert_resource(ActiveEnvironmentEvent(
            anima_engine_lib::evolution::meta_ai::EnvironmentalEvent::Stable
        ));
        world.insert_resource(EnvironmentState {
            ambient_light: 1.0,
            elapsed_time: 0.0,
            day_length: 120.0,
        });
        world.insert_resource(TerrainGrid::new(10, 10));

        let shared_settings = Arc::new(Mutex::new(EnvironmentSettings {
            time_speed,
            fertility: 1.0,
            weather_severity: 0.5,
        }));
        world.insert_resource(BevyEnvironmentSettings(shared_settings));

        let mut schedule = Schedule::default();
        schedule.add_systems((
            sync_environment_settings_system,
            update_day_night_cycle_system.after(sync_environment_settings_system),
            grow_flora_system.after(sync_environment_settings_system),
            wither_flora_system.after(grow_flora_system),
        ));

        // Spawn a dummy flora entity
        world.spawn((
            Flora {
                energy: 50.0,
                max_energy: 100.0,
                growth_rate: 1.0,
                decay_rate: 0.5,
            },
            Position(glam::Vec3::new(0.0, 0.0, 0.0)),
        ));

        // Run schedule for a few ticks - must not panic
        for _ in 0..10 {
            schedule.run(&mut world);
        }

        // Verify that after ticking, resources and entities are in a valid state
        let ts = world.resource::<TimeStep>().0;
        let env = world.resource::<EnvironmentState>();

        if time_speed.is_nan() {
            assert!((ts - (1.0 / 60.0)).abs() < 1e-6, "TimeStep should default to 1/60 when time_speed is NaN");
            assert!(env.ambient_light.is_finite(), "Ambient light should be finite");
        } else if time_speed == f32::INFINITY || time_speed == f32::NEG_INFINITY {
            assert!((ts - (1.0 / 60.0)).abs() < 1e-6, "TimeStep should default to 1/60 when time_speed is non-finite");
            assert!(env.ambient_light.is_finite(), "Ambient light should be finite");
        } else {
            // Under normal/negative time speeds
            assert!(ts.is_finite(), "TimeStep should be finite");
            assert!(env.ambient_light.is_finite(), "Ambient light should be finite");
        }
    }
}

#[test]
fn test_stress_boundary_fertility() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let cases = vec![
        0.0,
        100.0,
        100000.0,
        -10.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ];

    for &fertility in &cases {
        let mut world = World::new();

        // Initialize resources
        world.insert_resource(TimeStep(1.0 / 60.0));
        world.insert_resource(FloraSpawnSettings::default());
        world.insert_resource(MapBounds::default());
        world.insert_resource(TerrainGrid::new(10, 10));

        let shared_settings = Arc::new(Mutex::new(EnvironmentSettings {
            time_speed: 1.0,
            fertility,
            weather_severity: 0.5,
        }));
        world.insert_resource(BevyEnvironmentSettings(shared_settings));

        let mut schedule = Schedule::default();
        schedule.add_systems((
            sync_environment_settings_system,
            spawn_flora_system.after(sync_environment_settings_system),
        ));

        // Run schedule - must not panic
        for _ in 0..10 {
            schedule.run(&mut world);
        }

        let flora_settings = world.resource::<FloraSpawnSettings>();
        println!("Fertility {} resulted in max_flora_count: {}", fertility, flora_settings.max_flora_count);
        
        if fertility.is_nan() || fertility == f32::INFINITY || fertility == f32::NEG_INFINITY {
            assert_eq!(flora_settings.max_flora_count, 300, "Flora count should default to 300 when fertility is non-finite or NaN");
        } else if fertility < 0.0 {
            assert_eq!(flora_settings.max_flora_count, 0, "Flora count should saturate to 0 for negative fertility");
        }
    }
}

#[test]
fn test_thread_safety_high_rate_updates() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let shared_settings = Arc::new(Mutex::new(EnvironmentSettings {
        time_speed: 1.0,
        fertility: 1.0,
        weather_severity: 0.5,
    }));

    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));

    // Spawn 8 updater threads mutating the settings continuously
    let mut handles = Vec::new();
    for _thread_idx in 0..8 {
        let settings_clone = Arc::clone(&shared_settings);
        let running_clone = Arc::clone(&running);
        handles.push(thread::spawn(move || {
            let mut step = 0;
            while running_clone.load(std::sync::atomic::Ordering::Relaxed) {
                let mut settings = settings_clone.lock().unwrap();
                settings.time_speed = (step % 10) as f32;
                settings.fertility = (step % 5) as f32;
                settings.weather_severity = (step % 3) as f32 / 3.0;
                step += 1;
                
                // Yield thread to simulate high contention interleaving
                if step % 100 == 0 {
                    thread::yield_now();
                }
            }
        }));
    }

    // Spawn Bevy simulation ticker mimicking the ECS loop thread
    let settings_sim = Arc::clone(&shared_settings);
    let running_sim = Arc::clone(&running);
    let sim_handle = thread::spawn(move || {
        let mut world = World::new();
        world.insert_resource(TimeStep(1.0 / 60.0));
        world.insert_resource(FloraSpawnSettings::default());
        world.insert_resource(BevyEnvironmentSettings(settings_sim));

        let mut schedule = Schedule::default();
        schedule.add_systems(sync_environment_settings_system);

        let start = Instant::now();
        let mut tick_count = 0;

        while running_sim.load(std::sync::atomic::Ordering::Relaxed) {
            schedule.run(&mut world);
            tick_count += 1;

            // Stop after 100ms or 10,000 ticks
            if start.elapsed() > Duration::from_millis(100) || tick_count > 10000 {
                break;
            }
        }
        tick_count
    });

    // Wait for the simulation loop to finish, then stop updaters
    let total_ticks = sim_handle.join().expect("Sim thread failed");
    running.store(false, std::sync::atomic::Ordering::Relaxed);

    for h in handles {
        h.join().expect("Updater thread failed");
    }

    println!("Thread safety stress test completed. Sim ticked {} times.", total_ticks);
    assert!(total_ticks > 0, "Simulation did not tick");
}
