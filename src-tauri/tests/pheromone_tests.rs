mod common;

use bevy_ecs::prelude::*;
use glam::{Vec3, Quat};
use std::sync::Mutex;

use anima_engine_lib::ai::pheromone::{
    PheromoneGrid, OlfactorySensors, PheromoneReleaser, GRID_SIZE, MAX_CONCENTRATION,
    agent_release_pheromone_system, update_pheromone_grid_system, agent_read_pheromone_system,
};
use anima_engine_lib::core::ecs::{Position, Rotation, MapBounds};
use anima_engine_lib::ai::cpg::TimeStep;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_pheromone_decay() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = World::new();
    let grid = PheromoneGrid::new(0.0, 0.1); // diffusion 0, decay 0.1
    world.insert_resource(grid);
    world.insert_resource(TimeStep(1.0));

    // Put some pheromone concentration in a grid cell
    {
        let mut grid_res = world.resource_mut::<PheromoneGrid>();
        grid_res.values[0] = 5.0;
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(update_pheromone_grid_system);

    // Warm up
    schedule.run(&mut world);

    // After 1 step (dt = 1.0, decay_rate = 0.1), decay factor = (1.0 - 0.1 * 1.0) = 0.9
    // Expected concentration: 5.0 * 0.9 = 4.5
    let grid_res = world.resource::<PheromoneGrid>();
    assert!((grid_res.values[0] - 4.5).abs() < 1e-5, "Expected decayed value to be 4.5, got {}", grid_res.values[0]);
}

#[test]
fn test_pheromone_diffusion() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = World::new();
    let bounds = MapBounds {
        min: Vec3::new(-10.0, 0.0, -10.0),
        max: Vec3::new(10.0, 10.0, 10.0),
    };
    world.insert_resource(bounds);
    world.insert_resource(TimeStep(1.0));

    // Pure diffusion test (decay = 0.0, diffusion = 0.1)
    let grid = PheromoneGrid::new(0.1, 0.0);
    world.insert_resource(grid);

    // Place concentration at center (64, 64)
    {
        let mut grid_res = world.resource_mut::<PheromoneGrid>();
        grid_res.values[64 * GRID_SIZE + 64] = 10.0;
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(update_pheromone_grid_system);

    // Run 1 step
    // d_dt = (0.1 * 1.0) = 0.1 (clamped min is 0.24, so this is fine)
    // laplacian at (64, 64) = left(0) + right(0) + up(0) + down(0) - 4 * center(10) = -40
    // new center = center + d_dt * laplacian = 10 + 0.1 * (-40) = 6.0
    // neighbors = neighbor + d_dt * (center - 0) = 0 + 0.1 * 10 = 1.0
    schedule.run(&mut world);

    let grid_res = world.resource::<PheromoneGrid>();
    assert!((grid_res.values[64 * GRID_SIZE + 64] - 6.0).abs() < 1e-5, "Expected center 6.0, got {}", grid_res.values[64 * GRID_SIZE + 64]);
    assert!((grid_res.values[64 * GRID_SIZE + 65] - 1.0).abs() < 1e-5, "Expected neighbor 1.0, got {}", grid_res.values[64 * GRID_SIZE + 65]);
    assert!((grid_res.values[64 * GRID_SIZE + 63] - 1.0).abs() < 1e-5);
    assert!((grid_res.values[(64 + 1) * GRID_SIZE + 64] - 1.0).abs() < 1e-5);
    assert!((grid_res.values[(64 - 1) * GRID_SIZE + 64] - 1.0).abs() < 1e-5);

    // Verify conservation of mass (10.0 initially, should sum to 10.0)
    let total_mass: f32 = grid_res.values.iter().sum();
    assert!((total_mass - 10.0).abs() < 1e-4, "Expected total mass 10.0, got {}", total_mass);

    // Toroidal wrapping diffusion test: place concentration at (0, 0)
    let mut world2 = World::new();
    world2.insert_resource(bounds);
    world2.insert_resource(TimeStep(1.0));
    let grid2 = PheromoneGrid::new(0.1, 0.0);
    world2.insert_resource(grid2);

    {
        let mut grid_res2 = world2.resource_mut::<PheromoneGrid>();
        grid_res2.values[0] = 10.0;
    }

    let mut schedule2 = Schedule::default();
    schedule2.add_systems(update_pheromone_grid_system);
    schedule2.run(&mut world2);

    let grid_res2 = world2.resource::<PheromoneGrid>();
    assert!((grid_res2.values[0] - 6.0).abs() < 1e-5);
    // Neighbors of (0,0) under toroidal wrap are (1,0), (GRID_SIZE-1,0), (0,1), (0,GRID_SIZE-1)
    assert!((grid_res2.values[1] - 1.0).abs() < 1e-5);
    assert!((grid_res2.values[GRID_SIZE - 1] - 1.0).abs() < 1e-5);
    assert!((grid_res2.values[GRID_SIZE] - 1.0).abs() < 1e-5);
    assert!((grid_res2.values[(GRID_SIZE - 1) * GRID_SIZE] - 1.0).abs() < 1e-5);

    // Verify conservation of mass for toroidal wrap
    let total_mass2: f32 = grid_res2.values.iter().sum();
    assert!((total_mass2 - 10.0).abs() < 1e-4);
}

#[test]
fn test_olfactory_sensors() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = World::new();
    let bounds = MapBounds {
        min: Vec3::new(-10.0, 0.0, -10.0),
        max: Vec3::new(10.0, 10.0, 10.0),
    };
    world.insert_resource(bounds);

    let mut grid = PheromoneGrid::new(0.0, 0.0);
    // Write values to sample: (64, 64) is mapped to (0.0, 0.0, 0.0) in physical coordinates since min=-10, max=10
    let center_idx = 64 * GRID_SIZE + 64;
    grid.values[center_idx] = 4.0;
    grid.values[center_idx + 1] = 2.0;
    world.insert_resource(grid);

    // Spawn agent at center (0.0, 0.0, 0.0) rotated 90 degrees around Y axis
    let yaw_90 = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let agent_entity = world.spawn((
        Position(Vec3::new(0.0, 0.0, 0.0)),
        Rotation(yaw_90),
        OlfactorySensors::new(
            Vec3::new(-1.0, 0.0, 0.0), // left
            Vec3::new(1.0, 0.0, 0.0),  // right
        ),
    )).id();

    let mut schedule = Schedule::default();
    schedule.add_systems(agent_read_pheromone_system);

    schedule.run(&mut world);

    let sensors = world.get::<OlfactorySensors>(agent_entity).unwrap();
    assert!(sensors.left_reading >= 0.0);
    assert!(sensors.right_reading >= 0.0);
}

#[test]
fn test_agent_release_pheromone() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = World::new();
    let bounds = MapBounds {
        min: Vec3::new(-10.0, 0.0, -10.0),
        max: Vec3::new(10.0, 10.0, 10.0),
    };
    world.insert_resource(bounds);
    world.insert_resource(TimeStep(1.0));

    let grid = PheromoneGrid::default();
    world.insert_resource(grid);

    // Spawn agent at center (0.0, 0.0, 0.0), releasing 5.0 units/sec
    world.spawn((
        Position(Vec3::new(0.0, 0.0, 0.0)),
        PheromoneReleaser::new(5.0),
    ));

    let mut schedule = Schedule::default();
    schedule.add_systems(agent_release_pheromone_system);

    schedule.run(&mut world);

    let grid_res = world.resource::<PheromoneGrid>();
    // center idx for 128x128 grid mapped to [-10, 10]
    let center_idx = 64 * GRID_SIZE + 64;
    assert_eq!(grid_res.values[center_idx], 5.0);

    // Check clamping to MAX_CONCENTRATION
    for _ in 0..3 {
        schedule.run(&mut world);
    }
    let grid_res = world.resource::<PheromoneGrid>();
    assert_eq!(grid_res.values[center_idx], MAX_CONCENTRATION);
}

#[test]
fn test_hotpath_allocations() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = World::new();
    let bounds = MapBounds {
        min: Vec3::new(-10.0, 0.0, -10.0),
        max: Vec3::new(10.0, 10.0, 10.0),
    };
    world.insert_resource(bounds);
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(PheromoneGrid::default());

    // Spawn agents with sensors and releasers
    for i in 0..100 {
        world.spawn((
            Position(Vec3::new(i as f32 * 0.1, 0.0, 0.0)),
            Rotation(Quat::IDENTITY),
            OlfactorySensors::new(Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)),
            PheromoneReleaser::new(1.0),
        ));
    }

    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems((
        agent_release_pheromone_system,
        update_pheromone_grid_system.after(agent_release_pheromone_system),
        agent_read_pheromone_system.after(update_pheromone_grid_system),
    ));

    // Warm-up phase (allocations allowed here)
    schedule.run(&mut world);

    // Start tracking
    ALLOCATOR.start_tracking();

    // Execute 10 ticks (0 heap allocations expected in systems hot path)
    for _ in 0..10 {
        schedule.run(&mut world);
    }

    // Stop tracking
    let allocations = ALLOCATOR.stop_tracking();

    assert_eq!(
        allocations, 0,
        "Pheromone hot path performed {} heap allocation(s) inside the tick loop!",
        allocations
    );
}
