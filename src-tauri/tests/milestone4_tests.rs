mod common;

use bevy_ecs::prelude::*;
use std::sync::Mutex;
use glam::Vec3;
use anima_engine_lib::core::ecs::{
    Position, Velocity, MapBounds, Prey, Agent, Flora, herbivore_grazing_system, wither_flora_system
};
use anima_engine_lib::core::terrain::{TerrainGrid, BiomeType, TerrainCell};
use anima_engine_lib::physics::dynamics::{RigidBody, integrate_physics_system};
use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::HomeostaticState;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_terrain_speed_coupling() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    let bounds = MapBounds {
        min: Vec3::new(-10.0, 0.0, -10.0),
        max: Vec3::new(10.0, 10.0, 10.0),
    };
    world.insert_resource(bounds);
    world.insert_resource(TimeStep(1.0)); // dt = 1.0

    // Create a 20x20 flat terrain grid
    let mut terrain = TerrainGrid::new(20, 20);
    for cell in &mut terrain.cells {
        cell.biome = BiomeType::Grassland;
        cell.elevation = 0.0;
        cell.water_depth = 0.0;
    }

    // Set custom biomes for testing
    // Center is (0.0, 0.0, 0.0) -> index 210
    // Set Grassland at (0.0, 0.0, 0.0) -> index 210
    terrain.cells[210].biome = BiomeType::Grassland;
    world.insert_resource(terrain.clone());

    // 1. Biome Factors:
    let biomes_to_test = vec![
        (BiomeType::Grassland, 1.0),
        (BiomeType::Rock, 0.95),
        (BiomeType::Sand, 0.85),
        (BiomeType::Mud, 0.65),
        (BiomeType::Water, 1.0),
    ];

    for (biome, expected_multiplier) in biomes_to_test {
        let mut t = terrain.clone();
        t.cells[210].biome = biome;
        world.insert_resource(t);

        let entity = world.spawn((
            Position(Vec3::new(0.0, 0.0, 0.0)),
            Velocity(Vec3::new(10.0, 0.0, 0.0)),
            RigidBody {
                mass: 1.0,
                velocity: Vec3::new(10.0, 0.0, 0.0),
                force: Vec3::ZERO,
            },
            Prey,
            HomeostaticState {
                energy: 100.0,
                energy_target: 100.0,
                hydration: 100.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
        )).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(integrate_physics_system);
        schedule.run(&mut world);

        let body = world.get::<RigidBody>(entity).unwrap();
        let expected_velocity = 10.0 * expected_multiplier;
        assert!(
            (body.velocity.x - expected_velocity).abs() < 1e-4,
            "Biome {:?}: expected velocity {}, got {}",
            biome,
            expected_velocity,
            body.velocity.x
        );

        world.despawn(entity);
    }

    // 2. Water depth drag:
    // With water depth = 1.0, Water biome, factor = 1.0, drag = (1.0 - 1.0 * 0.4).max(0.15) = 0.6
    {
        let mut t = terrain.clone();
        t.cells[210].biome = BiomeType::Water;
        t.cells[210].water_depth = 1.0;
        world.insert_resource(t);

        let entity = world.spawn((
            Position(Vec3::new(0.0, 0.0, 0.0)),
            Velocity(Vec3::new(10.0, 0.0, 0.0)),
            RigidBody {
                mass: 1.0,
                velocity: Vec3::new(10.0, 0.0, 0.0),
                force: Vec3::ZERO,
            },
            Prey,
            HomeostaticState {
                energy: 100.0,
                energy_target: 100.0,
                hydration: 100.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
        )).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(integrate_physics_system);
        schedule.run(&mut world);

        let body = world.get::<RigidBody>(entity).unwrap();
        let expected_velocity = 10.0 * 1.0 * 0.6; // multiplier = 0.6
        assert!(
            (body.velocity.x - expected_velocity).abs() < 1e-4,
            "Water depth 1.0 drag: expected velocity {}, got {}",
            expected_velocity,
            body.velocity.x
        );

        world.despawn(entity);
    }

    // With water depth = 3.0, Grassland biome, factor = 1.0, drag = (1.0 - 3.0 * 0.4).max(0.15) = 0.15
    {
        let mut t = terrain.clone();
        t.cells[210].biome = BiomeType::Grassland;
        t.cells[210].water_depth = 3.0;
        world.insert_resource(t);

        let entity = world.spawn((
            Position(Vec3::new(0.0, 0.0, 0.0)),
            Velocity(Vec3::new(10.0, 0.0, 0.0)),
            RigidBody {
                mass: 1.0,
                velocity: Vec3::new(10.0, 0.0, 0.0),
                force: Vec3::ZERO,
            },
            Prey,
            HomeostaticState {
                energy: 100.0,
                energy_target: 100.0,
                hydration: 100.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
        )).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(integrate_physics_system);
        schedule.run(&mut world);

        let body = world.get::<RigidBody>(entity).unwrap();
        let expected_velocity = 10.0 * 1.0 * 0.15; // multiplier = 0.15
        assert!(
            (body.velocity.x - expected_velocity).abs() < 1e-4,
            "Water depth 3.0 drag: expected velocity {}, got {}",
            expected_velocity,
            body.velocity.x
        );

        world.despawn(entity);
    }

    // 3. Slope Penalty (Moving uphill vs downhill)
    // Map bounds are min -10, max 10. width 20, height 20. Cell size is 1.0.
    // Center at index 210.
    // pos_east = (1.0, 0.0, 0.0) -> index 211
    // pos_west = (-1.0, 0.0, 0.0) -> index 209
    // h_x1 = elevation at pos_east, h_x0 = elevation at pos_west
    // grad_x = (h_x1 - h_x0) / 2.0. If h_x1 = 0.4, h_x0 = 0.0 -> grad_x = 0.2
    // Moving east (uphill): slope = 0.2. scale = (1.0 - 0.2 * 1.0).max(0.2) = 0.8.
    // Moving west (downhill): slope = -0.2. scale = (1.0 - (-0.2) * 0.5).min(1.25) = 1.1.
    {
        let mut t = terrain.clone();
        t.cells[210].biome = BiomeType::Grassland;
        t.cells[211].elevation = 0.4; // East
        t.cells[209].elevation = 0.0; // West
        world.insert_resource(t);

        // Moving Uphill (East)
        let uphill_entity = world.spawn((
            Position(Vec3::new(0.0, 0.0, 0.0)),
            Velocity(Vec3::new(10.0, 0.0, 0.0)),
            RigidBody {
                mass: 1.0,
                velocity: Vec3::new(10.0, 0.0, 0.0),
                force: Vec3::ZERO,
            },
            Prey,
            HomeostaticState {
                energy: 100.0,
                energy_target: 100.0,
                hydration: 100.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
        )).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(integrate_physics_system);
        schedule.run(&mut world);

        let body = world.get::<RigidBody>(uphill_entity).unwrap();
        let expected_velocity = 10.0 * 1.0 * 0.8; // slope = 0.2 uphill, scale = 0.8
        assert!(
            (body.velocity.x - expected_velocity).abs() < 1e-4,
            "Moving uphill: expected velocity {}, got {}",
            expected_velocity,
            body.velocity.x
        );

        world.despawn(uphill_entity);

        // Moving Downhill (West)
        let downhill_entity = world.spawn((
            Position(Vec3::new(0.0, 0.0, 0.0)),
            Velocity(Vec3::new(-10.0, 0.0, 0.0)),
            RigidBody {
                mass: 1.0,
                velocity: Vec3::new(-10.0, 0.0, 0.0),
                force: Vec3::ZERO,
            },
            Prey,
            HomeostaticState {
                energy: 100.0,
                energy_target: 100.0,
                hydration: 100.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
        )).id();

        let mut schedule2 = Schedule::default();
        schedule2.add_systems(integrate_physics_system);
        schedule2.run(&mut world);

        let body = world.get::<RigidBody>(downhill_entity).unwrap();
        let expected_velocity = -10.0 * 1.0 * 1.1; // slope = -0.2 downhill, scale = 1.1
        assert!(
            (body.velocity.x - expected_velocity).abs() < 1e-4,
            "Moving downhill: expected velocity {}, got {}",
            expected_velocity,
            body.velocity.x
        );

        world.despawn(downhill_entity);
    }
}

#[test]
fn test_herbivore_grazing() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    world.insert_resource(TimeStep(1.0)); // dt = 1.0

    // Spawn Prey agent
    let prey_entity = world.spawn((
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
        Prey,
        Agent,
    )).id();

    // Spawn Flora plant within range 1.5 (e.g. distance = 1.0)
    let flora_entity = world.spawn((
        Position(Vec3::new(1.0, 0.0, 0.0)),
        Flora {
            energy: 40.0,
            max_energy: 100.0,
            growth_rate: 1.5,
            decay_rate: 0.2,
        },
    )).id();

    let mut schedule = Schedule::default();
    schedule.add_systems((
        herbivore_grazing_system,
        wither_flora_system.after(herbivore_grazing_system),
        apply_deferred.after(wither_flora_system),
    ));

    // Run 1 tick
    schedule.run(&mut world);

    // Verify energy transfer:
    // dt = 1.0. max_grazable = 15.0. energy_needed = 50.0. flora.energy = 40.0.
    // actual_grazed = 15.0
    // flora.energy should be 40.0 - 15.0 = 25.0
    // prey energy should be 50.0 + 15.0 = 65.0
    // prey hydration should be 50.0 + 15.0 * 0.4 = 56.0
    let homeo = world.get::<HomeostaticState>(prey_entity).unwrap();
    let flora = world.get::<Flora>(flora_entity).unwrap();

    assert_eq!(flora.energy, 25.0);
    assert_eq!(homeo.energy, 65.0);
    assert_eq!(homeo.hydration, 56.0);

    // Now let's test despawning on depletion:
    // Set flora energy to 5.0
    world.entity_mut(flora_entity).get_mut::<Flora>().unwrap().energy = 5.0;

    // Run again:
    // max_grazable = 15.0. energy_needed = 35.0. flora.energy = 5.0.
    // actual_grazed = 5.0
    // flora.energy becomes 0.0
    // wither_flora_system despawns it
    schedule.run(&mut world);

    assert!(world.get::<Flora>(flora_entity).is_none(), "Flora should be despawned on depletion");
}

#[test]
fn test_zero_allocation_hot_path_m4() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    let bounds = MapBounds {
        min: Vec3::new(-100.0, 0.0, -100.0),
        max: Vec3::new(100.0, 10.0, 100.0),
    };
    world.insert_resource(bounds);
    world.insert_resource(TimeStep(1.0 / 60.0));

    let terrain = TerrainGrid::new(32, 32);
    world.insert_resource(terrain);

    // Spawn a prey agent
    let prey_entity = world.spawn((
        Position(Vec3::new(0.0, 0.0, 0.0)),
        Velocity(Vec3::new(1.0, 0.0, 0.0)),
        RigidBody {
            mass: 1.0,
            velocity: Vec3::new(1.0, 0.0, 0.0),
            force: Vec3::ZERO,
        },
        Prey,
        Agent,
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

    // Spawn a flora
    let _flora_entity = world.spawn((
        Position(Vec3::new(0.5, 0.0, 0.5)),
        Flora {
            energy: 40.0,
            max_energy: 100.0,
            growth_rate: 1.5,
            decay_rate: 0.2,
        },
    )).id();

    let mut schedule = Schedule::default();
    schedule.add_systems((integrate_physics_system, herbivore_grazing_system));

    // Warm up the systems to initialize query state and metadata
    schedule.run(&mut world);

    // Start tracking
    ALLOCATOR.start_tracking();

    // Execute hot path
    schedule.run(&mut world);

    // Stop tracking
    let alloc_count = ALLOCATOR.stop_tracking();

    assert_eq!(
        alloc_count, 0,
        "Milestone 4 systems allocated {} times on hot path!",
        alloc_count
    );
}
