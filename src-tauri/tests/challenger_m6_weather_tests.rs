mod common;

use std::sync::{Arc, Mutex};
use bevy_ecs::prelude::*;
use anima_engine_lib::commands::EnvironmentSettings;
use anima_engine_lib::core::agent_systems::{
    sync_environment_settings_system, BevyEnvironmentSettings,
};
use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::core::ecs::{Cloud, Velocity, BaseCloudProperties, FloraSpawnSettings};
use glam::Vec3;

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn setup_test_world_with_clouds(
    settings: EnvironmentSettings,
    clouds: Vec<(f32, Vec3)>,
) -> (World, Vec<Entity>) {
    let mut world = World::new();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(FloraSpawnSettings::default());
    world.insert_resource(BevyEnvironmentSettings(Arc::new(Mutex::new(settings))));

    let mut entities = Vec::new();
    for (shadow, vel) in clouds {
        let entity = world
            .spawn((
                Cloud {
                    radius: 10.0,
                    shadow_intensity: shadow,
                },
                Velocity(vel),
            ))
            .id();
        entities.push(entity);
    }

    (world, entities)
}

#[test]
fn test_weather_severity_boundaries() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // We test multiple values of weather_severity
    let test_cases = vec![
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        -10.0,
        0.0,
        1e8,
        1.5,
    ];

    for &sev in &test_cases {
        let settings = EnvironmentSettings {
            time_speed: 1.0,
            fertility: 1.0,
            weather_severity: sev,
        };

        // Two clouds: one with 0.0 shadow intensity (critical case!), one with 0.5
        let (mut world, entities) = setup_test_world_with_clouds(
            settings,
            vec![
                (0.0, Vec3::new(1.0, 1.0, 1.0)),
                (0.5, Vec3::new(2.0, -2.0, 0.0)),
            ],
        );

        let mut schedule = Schedule::default();
        schedule.add_systems(sync_environment_settings_system);

        println!("Running test case for weather_severity = {}", sev);
        // Execute system run. If there's a panic (e.g. from clamp(NaN)), this will panic and fail the test.
        schedule.run(&mut world);

        // Verify that the values are safe (no NaNs in cloud shadow_intensity or velocity)
        for &entity in &entities {
            let cloud = world.entity(entity).get::<Cloud>().expect("Cloud component missing");
            let velocity = world.entity(entity).get::<Velocity>().expect("Velocity component missing");

            assert!(
                !cloud.shadow_intensity.is_nan(),
                "cloud.shadow_intensity is NaN for weather_severity = {}",
                sev
            );
            assert!(
                cloud.shadow_intensity >= 0.0 && cloud.shadow_intensity <= 1.0,
                "cloud.shadow_intensity out of bounds: {} for weather_severity = {}",
                cloud.shadow_intensity,
                sev
            );
            assert!(
                !velocity.0.is_nan(),
                "velocity.0 has NaN components: {:?} for weather_severity = {}",
                velocity.0,
                sev
            );
        }
    }
}
