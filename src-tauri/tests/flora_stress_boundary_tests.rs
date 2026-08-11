mod common;

use bevy_ecs::prelude::*;
use std::sync::Mutex;
use anima_engine_lib::core::ecs::{
    Position, MapBounds, wrap_coordinates_system, Flora, FloraSpawnSettings, spawn_flora_system
};
use anima_engine_lib::core::terrain::{
    TerrainGrid, BiomeType
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_toroidal_wrapping_boundary_values() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(anima_engine_lib::core::rng::SimRng::default());
    let bounds = MapBounds {
        min: glam::Vec3::new(-10.0, 0.0, -10.0),
        max: glam::Vec3::new(10.0, 10.0, 10.0),
    };
    world.insert_resource(bounds);

    let mut schedule = Schedule::default();
    schedule.add_systems(wrap_coordinates_system);

    // Test cases: (input, expected)
    let cases = vec![
        // Within bounds
        (glam::Vec3::new(0.0, 0.0, 0.0), glam::Vec3::new(0.0, 0.0, 0.0)),
        (glam::Vec3::new(5.0, 2.0, -5.0), glam::Vec3::new(5.0, 2.0, -5.0)),
        
        // Exact lower bounds (rem_euclid(20) on 0 is 0, so -10 + 0 = -10)
        (glam::Vec3::new(-10.0, 0.0, -10.0), glam::Vec3::new(-10.0, 0.0, -10.0)),
        
        // Exact upper bounds (rem_euclid(20) on 20 is 0, so -10 + 0 = -10)
        (glam::Vec3::new(10.0, 0.0, 10.0), glam::Vec3::new(-10.0, 0.0, -10.0)),
        
        // Slightly outside upper bounds
        (glam::Vec3::new(10.1, 0.0, 10.5), glam::Vec3::new(-9.9, 0.0, -9.5)),
        
        // Slightly outside lower bounds
        (glam::Vec3::new(-10.1, 0.0, -10.5), glam::Vec3::new(9.9, 0.0, 9.5)),
        
        // Multiple wraps away (positive)
        (glam::Vec3::new(35.0, 0.0, 50.0), glam::Vec3::new(-5.0, 0.0, -10.0)),
        
        // Multiple wraps away (negative)
        (glam::Vec3::new(-35.0, 0.0, -50.0), glam::Vec3::new(5.0, 0.0, -10.0)),
    ];

    for (input_pos, expected_pos) in cases {
        let entity = world.spawn(Position(input_pos)).id();
        schedule.run(&mut world);
        let wrapped = world.get::<Position>(entity).unwrap().0;
        
        assert!(
            (wrapped.x - expected_pos.x).abs() < 1e-4,
            "X wrap failed: input {:?}, expected {:?}, got {:?}", input_pos, expected_pos, wrapped
        );
        assert!(
            (wrapped.z - expected_pos.z).abs() < 1e-4,
            "Z wrap failed: input {:?}, expected {:?}, got {:?}", input_pos, expected_pos, wrapped
        );
        // Height (Y) is not wrapped, should remain unchanged
        assert_eq!(
            wrapped.y, input_pos.y,
            "Y height changed: input {:?}, got {:?}", input_pos, wrapped
        );
        
        // Despawn to clean up
        world.despawn(entity);
    }
}

#[test]
fn test_terrain_grid_pos_to_index_wrapping() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let grid = TerrainGrid::new(4, 4); // 4x4 grid
    let bounds = MapBounds {
        min: glam::Vec3::new(-10.0, 0.0, -10.0),
        max: glam::Vec3::new(10.0, 10.0, 10.0),
    };

    // Center is (0.0, 0.0, 0.0) -> wrapped is (0.0, 0.0, 0.0) -> norm_x = 10/20 = 0.5 -> col = floor(0.5 * 4) = 2, row = 2. index = 2 * 4 + 2 = 10.
    let idx_center = grid.pos_to_index(glam::Vec3::new(0.0, 0.0, 0.0), &bounds);
    assert_eq!(idx_center, Some(10));

    // Outside bounds positive: (10.1, 0.0, 10.1) -> wraps to (-9.9, 0.0, -9.9) -> col = 0, row = 0 -> index = 0.
    let idx_out_pos = grid.pos_to_index(glam::Vec3::new(10.1, 0.0, 10.1), &bounds);
    assert_eq!(idx_out_pos, Some(0));

    // Outside bounds negative: (-10.1, 0.0, -10.1) -> wraps to (9.9, 0.0, 9.9) -> col = 3, row = 3 -> index = 15.
    let idx_out_neg = grid.pos_to_index(glam::Vec3::new(-10.1, 0.0, -10.1), &bounds);
    assert_eq!(idx_out_neg, Some(15));
}

#[test]
fn test_vegetative_seeding_toroidal_wrapping() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(anima_engine_lib::core::rng::SimRng::default());
    world.insert_resource(anima_engine_lib::ai::cpg::TimeStep(1.0));
    let bounds = MapBounds {
        min: glam::Vec3::new(-10.0, 0.0, -10.0),
        max: glam::Vec3::new(10.0, 10.0, 10.0),
    };
    world.insert_resource(bounds);
    
    // Create a 4x4 flat grassland grid
    let mut grid = TerrainGrid::new(4, 4);
    for cell in &mut grid.cells {
        cell.biome = BiomeType::Grassland;
    }
    world.insert_resource(grid);

    // Settings: disable spontaneous spawning, force vegetative seeding
    // Sprout offset will wrap around
    world.insert_resource(FloraSpawnSettings {
        max_flora_count: 2,
        initial_energy: 5.0,
        default_max_energy: 100.0,
        default_growth_rate: 1.5,
        default_decay_rate: 0.2,
        spontaneous_spawn_chance: 0.0,
        vegetative_reproduction_threshold: 10.0,
        vegetative_reproduction_chance: 1.0, // Always reproduce if above threshold
    });

    // Spawn parent at the boundary: x = 9.9, z = 9.9 (near top-right corner)
    world.spawn((
        Flora {
            energy: 50.0,
            max_energy: 100.0,
            growth_rate: 1.5,
            decay_rate: 0.2,
        },
        Position(glam::Vec3::new(9.9, 0.0, 9.9)),
    ));

    let mut schedule = Schedule::default();
    schedule.add_systems(spawn_flora_system);

    let mut attempts = 0;
    let mut wrapped_pos = glam::Vec3::ZERO;
    loop {
        attempts += 1;
        if attempts > 1000 {
            panic!("Failed to spawn wrapped offspring after 1000 attempts");
        }

        schedule.run(&mut world);

        let flora_count = world.query::<&Flora>().iter(&world).count();
        if flora_count == 2 {
            let mut has_wrapped = false;
            let mut offspring_entity = None;

            for (entity, pos) in world.query::<(Entity, &Position)>().iter(&world) {
                if (pos.0.x - 9.9).abs() > 1e-4 || (pos.0.z - 9.9).abs() > 1e-4 {
                    offspring_entity = Some(entity);
                    if pos.0.x < 0.0 || pos.0.z < 0.0 {
                        has_wrapped = true;
                        wrapped_pos = pos.0;
                    }
                }
            }

            if has_wrapped {
                break;
            } else if let Some(ent) = offspring_entity {
                world.despawn(ent);
            }
        }
    }

    // Verify that the wrapped offspring is inside bounds
    assert!(wrapped_pos.x >= -10.0 && wrapped_pos.x <= 10.0, "Offspring x out of bounds: {}", wrapped_pos.x);
    assert!(wrapped_pos.z >= -10.0 && wrapped_pos.z <= 10.0, "Offspring z out of bounds: {}", wrapped_pos.z);
}

#[test]
fn test_spawning_density_limits_under_long_runs() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(anima_engine_lib::core::rng::SimRng::default());
    world.insert_resource(anima_engine_lib::ai::cpg::TimeStep(1.0));
    let bounds = MapBounds {
        min: glam::Vec3::new(-10.0, 0.0, -10.0),
        max: glam::Vec3::new(10.0, 10.0, 10.0),
    };
    world.insert_resource(bounds);

    let mut grid = TerrainGrid::new(10, 10);
    for cell in &mut grid.cells {
        cell.biome = BiomeType::Grassland;
    }
    world.insert_resource(grid);

    // Highly aggressive spawning settings: max_flora_count = 50
    world.insert_resource(FloraSpawnSettings {
        max_flora_count: 50,
        initial_energy: 50.0,
        default_max_energy: 100.0,
        default_growth_rate: 10.0, // grow fast to always be above reproduction threshold
        default_decay_rate: 0.1,
        spontaneous_spawn_chance: 0.9, // extremely high spontaneous spawning chance
        vegetative_reproduction_threshold: 10.0,
        vegetative_reproduction_chance: 0.9, // extremely high vegetative seeding chance
    });

    world.insert_resource(anima_engine_lib::core::ecs::EnvironmentState { ambient_light: 1.0, ..Default::default() });
    world.insert_resource(anima_engine_lib::core::ecs::ActiveEnvironmentEvent::default());

    // Create schedule
    let mut schedule = Schedule::default();
    schedule.add_systems((
        anima_engine_lib::core::ecs::grow_flora_system,
        anima_engine_lib::core::ecs::wither_flora_system,
        anima_engine_lib::core::ecs::spawn_flora_system,
    ));

    // Spawn 5 initial flora
    for i in 0..5 {
        world.spawn((
            Flora {
                energy: 50.0,
                max_energy: 100.0,
                growth_rate: 10.0,
                decay_rate: 0.1,
            },
            Position(glam::Vec3::new(i as f32 - 2.0, 0.0, i as f32 - 2.0)),
        ));
    }

    // Run for 1000 ticks.
    for tick in 0..1000 {
        schedule.run(&mut world);
        
        let count = world.query::<&Flora>().iter(&world).count();
        assert!(
            count <= 50,
            "Flora count exceeded max_flora_count ({}) on tick {}: got {}", 50, tick, count
        );
    }

    let final_count = world.query::<&Flora>().iter(&world).count();
    assert_eq!(final_count, 50, "Expected final flora count to be capped at 50, got {}", final_count);
}

#[test]
fn test_spawning_density_limits_boundary_max_zero() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(anima_engine_lib::core::rng::SimRng::default());
    world.insert_resource(anima_engine_lib::ai::cpg::TimeStep(1.0));
    let bounds = MapBounds::default();
    world.insert_resource(bounds);

    let mut grid = TerrainGrid::new(4, 4);
    for cell in &mut grid.cells {
        cell.biome = BiomeType::Grassland;
    }
    world.insert_resource(grid);

    // max_flora_count = 0
    world.insert_resource(FloraSpawnSettings {
        max_flora_count: 0,
        initial_energy: 5.0,
        default_max_energy: 100.0,
        default_growth_rate: 1.5,
        default_decay_rate: 0.2,
        spontaneous_spawn_chance: 1.0,
        vegetative_reproduction_threshold: 10.0,
        vegetative_reproduction_chance: 1.0,
    });

    let mut schedule = Schedule::default();
    schedule.add_systems(spawn_flora_system);

    schedule.run(&mut world);
    let count = world.query::<&Flora>().iter(&world).count();
    assert_eq!(count, 0, "Flora spawned when max_flora_count was 0");

    world.spawn((
        Flora {
            energy: 50.0,
            max_energy: 100.0,
            growth_rate: 1.5,
            decay_rate: 0.2,
        },
        Position(glam::Vec3::new(0.0, 0.0, 0.0)),
    ));

    schedule.run(&mut world);
    let count = world.query::<&Flora>().iter(&world).count();
    assert_eq!(count, 1, "Flora count increased when max_flora_count was 0");
}

#[test]
fn test_spawning_density_limits_boundary_max_one() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(anima_engine_lib::core::rng::SimRng::default());
    world.insert_resource(anima_engine_lib::ai::cpg::TimeStep(1.0));
    let bounds = MapBounds::default();
    world.insert_resource(bounds);

    let mut grid = TerrainGrid::new(4, 4);
    for cell in &mut grid.cells {
        cell.biome = BiomeType::Grassland;
    }
    world.insert_resource(grid);

    // max_flora_count = 1
    world.insert_resource(FloraSpawnSettings {
        max_flora_count: 1,
        initial_energy: 5.0,
        default_max_energy: 100.0,
        default_growth_rate: 1.5,
        default_decay_rate: 0.2,
        spontaneous_spawn_chance: 1.0,
        vegetative_reproduction_threshold: 10.0,
        vegetative_reproduction_chance: 1.0,
    });

    let mut schedule = Schedule::default();
    schedule.add_systems(spawn_flora_system);

    schedule.run(&mut world);
    let count = world.query::<&Flora>().iter(&world).count();
    assert_eq!(count, 1, "Expected exactly 1 flora to be spawned");

    schedule.run(&mut world);
    let count = world.query::<&Flora>().iter(&world).count();
    assert_eq!(count, 1, "Flora count exceeded max_flora_count of 1");
}
