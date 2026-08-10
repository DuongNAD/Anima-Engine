mod common;

use bevy_ecs::prelude::*;
use std::sync::Mutex;
use anima_engine_lib::core::terrain::{
    generate_terrain, water_dynamics_system, BiomeType, TerrainGrid, TerrainCell
};
use anima_engine_lib::ai::cpg::TimeStep;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_terrain_generation_biome_distribution() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let grid = generate_terrain(128, 128, 42);

    assert_eq!(grid.width, 128);
    assert_eq!(grid.height, 128);
    assert_eq!(grid.cells.len(), 16384);

    let mut count_water = 0;
    let mut count_sand = 0;
    let mut count_mud = 0;
    let mut count_grassland = 0;
    let mut count_rock = 0;

    let mut min_elev = 999.0;
    let mut max_elev = -999.0;

    for cell in &grid.cells {
        if cell.elevation < min_elev { min_elev = cell.elevation; }
        if cell.elevation > max_elev { max_elev = cell.elevation; }
        
        match cell.biome {
            BiomeType::Water => {
                count_water += 1;
                assert!(cell.elevation < 0.25);
                assert!(cell.water_depth > 0.0);
            }
            BiomeType::Sand => {
                count_sand += 1;
                assert!(cell.elevation >= 0.25);
                assert_eq!(cell.water_depth, 0.0);
            }
            BiomeType::Mud => {
                count_mud += 1;
                assert!(cell.elevation >= 0.25 && cell.elevation < 0.35);
                assert_eq!(cell.water_depth, 0.0);
            }
            BiomeType::Grassland => {
                count_grassland += 1;
                assert!(cell.elevation >= 0.35 && cell.elevation < 0.75);
                assert_eq!(cell.water_depth, 0.0);
            }
            BiomeType::Rock => {
                count_rock += 1;
                assert!(cell.elevation >= 0.75);
                assert_eq!(cell.water_depth, 0.0);
            }
        }
    }

    println!("Elevation range: [{}, {}]", min_elev, max_elev);
    println!("Biome counts: water={}, sand={}, mud={}, grassland={}, rock={}", 
             count_water, count_sand, count_mud, count_grassland, count_rock);

    // Ensure we generated a varied layout
    assert!(count_water > 0, "No water biomes generated");
    assert!(count_sand > 0 || count_mud > 0 || count_grassland > 0 || count_rock > 0, "No land biomes generated");
}

#[test]
fn test_water_flow_dynamics_and_mass_conservation() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // 1. Test unconstrained flow (where total outflow <= current water depth)
    {
        let mut grid = TerrainGrid::new(3, 3);
        
        // Setup center cell (index 4) with elevation 1.0 and water depth 10.0
        // All neighbors have elevation 0.0, water depth 0.0
        grid.cells[4] = TerrainCell {
            elevation: 1.0,
            water_depth: 10.0,
            biome: BiomeType::Water,
        };

        let mut world = World::new();
        world.insert_resource(grid);
        world.insert_resource(TimeStep(1.0)); // dt = 1.0

        let mut schedule = Schedule::default();
        schedule.add_systems(water_dynamics_system);

        // Run system for 1 tick
        schedule.run(&mut world);

        let grid_after = world.resource::<TerrainGrid>();

        // Center cell calculations:
        // Total height center = 1.0 + 10.0 = 11.0. Total height neighbor = 0.0.
        // Diff = 11.0. Total diff = 44.0.
        // Desired out = 44.0 * 0.2 * 1.0 = 8.8.
        // Since 8.8 <= 10.0, scale = 8.8 / 44.0 = 0.2.
        // Transfer to each of 4 neighbors = 11.0 * 0.2 = 2.2.
        // Water before evap: Center = 10.0 - 8.8 = 1.2, Neighbors = 2.2.
        // Water after evap (rate 0.01): Center = 1.2 - 0.01 = 1.19, Neighbors = 2.2 - 0.01 = 2.19.
        
        let center = grid_after.cells[4];
        assert!((center.water_depth - 1.19).abs() < 1e-5, "Expected center water depth 1.19, got {}", center.water_depth);
        
        // Neighbors: West (3), East (5), North (1), South (7)
        for &idx in &[1, 3, 5, 7] {
            let neighbor = grid_after.cells[idx];
            assert!((neighbor.water_depth - 2.19).abs() < 1e-5, "Expected neighbor water depth 2.19, got {}", neighbor.water_depth);
        }

        // Verify total mass is conserved: Total starting = 10.0. Total final = 1.19 + 4 * 2.19 = 9.95. Evaporated = 5 * 0.01 = 0.05.
        let total_final_water: f32 = grid_after.cells.iter().map(|c| c.water_depth).sum();
        let expected_final_water = 9.95;
        assert!((total_final_water - expected_final_water).abs() < 1e-5, "Expected total final water 9.95, got {}", total_final_water);
    }

    // 2. Test constrained flow (where total outflow > current water depth, forcing scale down)
    {
        let mut grid = TerrainGrid::new(3, 3);
        
        // Setup center cell (index 4) with elevation 1.0 and water depth 2.0
        // All neighbors have elevation 0.0, water depth 0.0
        grid.cells[4] = TerrainCell {
            elevation: 1.0,
            water_depth: 2.0,
            biome: BiomeType::Water,
        };

        let mut world = World::new();
        world.insert_resource(grid);
        world.insert_resource(TimeStep(1.0)); // dt = 1.0

        let mut schedule = Schedule::default();
        schedule.add_systems(water_dynamics_system);

        schedule.run(&mut world);

        let grid_after = world.resource::<TerrainGrid>();

        // Center cell calculations:
        // Total height center = 1.0 + 2.0 = 3.0. Total height neighbor = 0.0.
        // Diff = 3.0. Total diff = 12.0.
        // Desired out = 12.0 * 0.2 * 1.0 = 2.4.
        // Since 2.4 > 2.0, flow is scaled: actual out = 2.0. scale = 2.0 / 12.0 = 1.0 / 6.0.
        // Transfer to each neighbor = 3.0 * (1.0 / 6.0) = 0.5.
        // Water before evap: Center = 2.0 - 2.0 = 0.0, Neighbors = 0.5.
        // Water after evap (rate 0.01): Center = 0.0 (no evap on 0), Neighbors = 0.5 - 0.01 = 0.49.

        let center = grid_after.cells[4];
        assert_eq!(center.water_depth, 0.0, "Expected center water depth 0.0, got {}", center.water_depth);

        for &idx in &[1, 3, 5, 7] {
            let neighbor = grid_after.cells[idx];
            assert!((neighbor.water_depth - 0.49).abs() < 1e-5, "Expected neighbor water depth 0.49, got {}", neighbor.water_depth);
        }

        // Verify total mass is conserved: Total starting = 2.0. Total final = 4 * 0.49 = 1.96. Evaporated = 4 * 0.01 = 0.04.
        let total_final_water: f32 = grid_after.cells.iter().map(|c| c.water_depth).sum();
        let expected_final_water = 1.96;
        assert!((total_final_water - expected_final_water).abs() < 1e-5, "Expected total final water 1.96, got {}", total_final_water);
    }
}

#[test]
fn test_zero_heap_allocation_water_dynamics() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    let grid = generate_terrain(32, 32, 42);
    world.insert_resource(grid);
    world.insert_resource(TimeStep(1.0 / 60.0));

    let mut schedule = Schedule::default();
    schedule.add_systems(water_dynamics_system);

    // Warm up the schedule and system (caching queries, metadata etc)
    schedule.run(&mut world);

    // Start tracking heap allocations
    ALLOCATOR.start_tracking();
    
    // Execute hot path system run
    schedule.run(&mut world);

    // Stop tracking heap allocations
    let alloc_count = ALLOCATOR.stop_tracking();

    assert_eq!(
        alloc_count, 0,
        "water_dynamics_system allocated {} times on hot path!",
        alloc_count
    );
}
