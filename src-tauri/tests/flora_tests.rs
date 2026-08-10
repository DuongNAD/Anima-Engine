mod common;

use bevy_ecs::prelude::*;
use rand::Rng;
use std::sync::Mutex;
use anima_engine_lib::core::ecs::{
    grow_flora_system, wither_flora_system, spawn_flora_system, Flora, FloraSpawnSettings, EnvironmentState, Position, MapBounds, ActiveEnvironmentEvent
};
use anima_engine_lib::core::terrain::{
    BiomeType, TerrainGrid, TerrainCell
};
use anima_engine_lib::ai::cpg::TimeStep;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_flora_growth_and_withering() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(TimeStep(1.0)); // dt = 1.0
    world.insert_resource(MapBounds::default());
    world.insert_resource(ActiveEnvironmentEvent::default());
    world.insert_resource(FloraSpawnSettings::default());
    world.insert_resource(EnvironmentState { ambient_light: 1.0, ..Default::default() });

    let mut grid = TerrainGrid::new(3, 3);
    // Grassland cell at index 4 (center)
    grid.cells[4] = TerrainCell {
        elevation: 0.5,
        water_depth: 0.0,
        biome: BiomeType::Grassland,
    };
    world.insert_resource(grid);

    // Spawn a plant in the center
    let plant = world.spawn((
        Flora {
            energy: 10.0,
            max_energy: 100.0,
            growth_rate: 1.5,
            decay_rate: 0.2,
        },
        Position(glam::Vec3::new(0.0, 0.0, 0.0)),
    )).id();

    let mut schedule = Schedule::default();
    schedule.add_systems(grow_flora_system);

    // Run once.
    // Grassland moisture factor is 1.0. ambient light is 1.0. dt is 1.0.
    // growth_delta = 1.5 * 1.0 * 1.0 * 1.0 = 1.5
    // decay_delta = 0.2 * 1.0 * 1.0 = 0.2
    // Net energy delta = 1.5 - 0.2 = 1.3
    // Expected energy = 10.0 + 1.3 = 11.3
    schedule.run(&mut world);

    let flora = world.get::<Flora>(plant).unwrap();
    assert!((flora.energy - 11.3).abs() < 1e-5, "Expected 11.3, got {}", flora.energy);

    // Now set ambient light to 0.0 (dark)
    world.insert_resource(EnvironmentState { ambient_light: 0.0, ..Default::default() });
    // Run again.
    // growth_delta = 0.0
    // decay_delta = 0.2
    // Net energy delta = -0.2
    // Expected energy = 11.3 - 0.2 = 11.1
    schedule.run(&mut world);

    let flora = world.get::<Flora>(plant).unwrap();
    assert!((flora.energy - 11.1).abs() < 1e-5, "Expected 11.1, got {}", flora.energy);

    // Test withering
    let mut wither_schedule = Schedule::default();
    wither_schedule.add_systems(wither_flora_system);

    // Set plant energy directly to 0.0
    world.get_mut::<Flora>(plant).unwrap().energy = 0.0;
    wither_schedule.run(&mut world);

    assert!(world.get::<Flora>(plant).is_none(), "Flora should have been despawned");
}

#[test]
fn test_flora_growth_biomes_and_stress() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let biomes_to_test = vec![
        (BiomeType::Grassland, 0.0, 1.0), // Grassland, water depth = 0.0, expected moisture = 1.0
        (BiomeType::Mud, 0.0, 0.9),       // Mud, water depth = 0.0, expected moisture = 0.9
        (BiomeType::Sand, 0.0, 0.3),      // Sand, water depth = 0.0, expected moisture = 0.3
        (BiomeType::Rock, 0.0, 0.05),     // Rock, water depth = 0.0, expected moisture = 0.05
        (BiomeType::Water, 0.2, 0.5),     // Shallow water, depth = 0.2, expected moisture = 0.5
        (BiomeType::Water, 0.8, 0.0),     // Deep water, depth = 0.8, expected moisture = 0.0 (drowning, 3x decay)
    ];

    for (biome, water_depth, expected_moisture) in biomes_to_test {
        let mut world = World::new();
        world.insert_resource(TimeStep(1.0)); // dt = 1.0
        world.insert_resource(MapBounds::default());
        world.insert_resource(ActiveEnvironmentEvent::default());
        world.insert_resource(FloraSpawnSettings::default());
        world.insert_resource(EnvironmentState { ambient_light: 1.0, ..Default::default() });

        let mut grid = TerrainGrid::new(3, 3);
        grid.cells[4] = TerrainCell {
            elevation: 0.5,
            water_depth,
            biome,
        };
        world.insert_resource(grid);

        let plant = world.spawn((
            Flora {
                energy: 50.0,
                max_energy: 100.0,
                growth_rate: 10.0,
                decay_rate: 2.0,
            },
            Position(glam::Vec3::new(0.0, 0.0, 0.0)),
        )).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(grow_flora_system);
        schedule.run(&mut world);

        let flora = world.get::<Flora>(plant).unwrap();
        let expected_growth: f32 = 10.0 * expected_moisture as f32 * 1.0 * 1.0;
        let expected_decay_multiplier: f32 = if biome == BiomeType::Water && water_depth > 0.5 { 3.0 } else { 1.0 };
        let expected_decay: f32 = 2.0 * expected_decay_multiplier * 1.0;
        let expected_energy: f32 = (50.0f32 + expected_growth - expected_decay).clamp(0.0f32, 100.0f32);

        assert!((flora.energy - expected_energy).abs() < 1e-5,
            "Biome {:?}, depth {}: expected energy {}, got {}", biome, water_depth, expected_energy, flora.energy);
    }
}

#[test]
fn test_flora_drought_stress() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(TimeStep(1.0)); // dt = 1.0
    world.insert_resource(MapBounds::default());
    // ResourceDrought active event
    world.insert_resource(ActiveEnvironmentEvent(
        anima_engine_lib::evolution::meta_ai::EnvironmentalEvent::ResourceDrought
    ));
    world.insert_resource(FloraSpawnSettings::default());
    world.insert_resource(EnvironmentState { ambient_light: 1.0, ..Default::default() });

    let mut grid = TerrainGrid::new(3, 3);
    grid.cells[4] = TerrainCell {
        elevation: 0.5,
        water_depth: 0.0,
        biome: BiomeType::Grassland,
    };
    world.insert_resource(grid);

    let plant = world.spawn((
        Flora {
            energy: 50.0,
            max_energy: 100.0,
            growth_rate: 1.5,
            decay_rate: 2.0,
        },
        Position(glam::Vec3::new(0.0, 0.0, 0.0)),
    )).id();

    let mut schedule = Schedule::default();
    schedule.add_systems(grow_flora_system);
    schedule.run(&mut world);

    let flora = world.get::<Flora>(plant).unwrap();
    // growth_delta = 1.5 * 1.0 * 1.0 * 1.0 = 1.5
    // decay_multiplier = 2.0 (Drought)
    // decay_delta = 2.0 * 2.0 * 1.0 = 4.0
    // Expected = 50.0 + 1.5 - 4.0 = 47.5
    assert!((flora.energy - 47.5).abs() < 1e-5, "Expected 47.5, got {}", flora.energy);
}

#[test]
fn test_flora_spontaneous_spawn() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(TimeStep(1.0));
    world.insert_resource(MapBounds {
        min: glam::Vec3::new(-10.0, 0.0, -10.0),
        max: glam::Vec3::new(10.0, 10.0, 10.0),
    });
    world.insert_resource(ActiveEnvironmentEvent::default());
    // Force spontaneous spawn chance to 1.0 (always spawn) and max flora to 10
    world.insert_resource(FloraSpawnSettings {
        max_flora_count: 10,
        initial_energy: 5.0,
        default_max_energy: 100.0,
        default_growth_rate: 1.5,
        default_decay_rate: 0.2,
        spontaneous_spawn_chance: 1.0,
        vegetative_reproduction_threshold: 80.0,
        vegetative_reproduction_chance: 0.0, // Disable vegetative reproduction for this test
    });

    // Generate a flat grassland terrain
    let mut grid = TerrainGrid::new(4, 4);
    for cell in &mut grid.cells {
        cell.biome = BiomeType::Grassland;
    }
    world.insert_resource(grid);

    let mut schedule = Schedule::default();
    schedule.add_systems(spawn_flora_system);

    // Run system once
    schedule.run(&mut world);

    let flora_count = world.query::<&Flora>().iter(&world).count();
    assert_eq!(flora_count, 1, "Should have spawned 1 plant spontaneously");
}

#[test]
fn test_flora_vegetative_seeding() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(TimeStep(1.0));
    world.insert_resource(MapBounds {
        min: glam::Vec3::new(-10.0, 0.0, -10.0),
        max: glam::Vec3::new(10.0, 10.0, 10.0),
    });
    world.insert_resource(ActiveEnvironmentEvent::default());
    // Settings: disable spontaneous spawn, force vegetative reproduction
    world.insert_resource(FloraSpawnSettings {
        max_flora_count: 5,
        initial_energy: 5.0,
        default_max_energy: 100.0,
        default_growth_rate: 1.5,
        default_decay_rate: 0.2,
        spontaneous_spawn_chance: 0.0,
        vegetative_reproduction_threshold: 20.0,
        vegetative_reproduction_chance: 1.0,
    });

    let mut grid = TerrainGrid::new(4, 4);
    for cell in &mut grid.cells {
        cell.biome = BiomeType::Grassland;
    }
    world.insert_resource(grid);

    // Spawn a mature plant at the center
    world.spawn((
        Flora {
            energy: 30.0, // above threshold 20
            max_energy: 100.0,
            growth_rate: 1.5,
            decay_rate: 0.2,
        },
        Position(glam::Vec3::new(0.0, 0.0, 0.0)),
    ));

    let mut schedule = Schedule::default();
    schedule.add_systems(spawn_flora_system);
    
    // Run the spawn schedule.
    schedule.run(&mut world);

    let flora_count = world.query::<&Flora>().iter(&world).count();
    // 1 original mature + 1 vegetative seed = 2
    assert_eq!(flora_count, 2, "Expected 2 flora entities, got {}", flora_count);
}

#[test]
fn test_zero_heap_allocation_flora_systems() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(MapBounds::default());
    world.insert_resource(ActiveEnvironmentEvent::default());
    world.insert_resource(FloraSpawnSettings {
        max_flora_count: 300,
        initial_energy: 5.0,
        default_max_energy: 100.0,
        default_growth_rate: 1.5,
        default_decay_rate: 0.2,
        spontaneous_spawn_chance: 0.0, // Disable spontaneous spawning to avoid allocations in command buffer
        vegetative_reproduction_threshold: 80.0,
        vegetative_reproduction_chance: 0.0, // Disable vegetative seeding to avoid allocations in command buffer
    });
    world.insert_resource(EnvironmentState::default());

    let mut grid = TerrainGrid::new(32, 32);
    for cell in &mut grid.cells {
        cell.biome = BiomeType::Grassland;
    }
    world.insert_resource(grid);

    // Spawn 100 plants in steady state
    for i in 0..100 {
        world.spawn((
            Flora {
                energy: 50.0,
                max_energy: 100.0,
                growth_rate: 1.5,
                decay_rate: 0.2,
            },
            Position(glam::Vec3::new(i as f32 * 0.1, 0.0, i as f32 * 0.1)),
        ));
    }

    let mut schedule = Schedule::default();
    schedule.add_systems((
        grow_flora_system,
        wither_flora_system,
        spawn_flora_system,
    ));

    // Warm up the schedule and systems (caching queries, metadata etc)
    schedule.run(&mut world);

    // Warm up thread-local RNG if any system uses it (though they are disabled here, let's call it just in case)
    let _ = rand::thread_rng().gen_bool(0.5);

    // Start tracking heap allocations
    ALLOCATOR.start_tracking();

    // Execute hot path system run
    schedule.run(&mut world);

    // Stop tracking heap allocations
    let alloc_count = ALLOCATOR.stop_tracking();

    assert_eq!(
        alloc_count, 0,
        "Flora systems allocated {} times on hot path!",
        alloc_count
    );
}
