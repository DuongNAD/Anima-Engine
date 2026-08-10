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
fn test_water_dynamics_extremes_flat() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Initialize a 5x5 flat grid (all elevations = 0.0)
    let mut grid = TerrainGrid::new(5, 5);
    for cell in &mut grid.cells {
        cell.elevation = 0.0;
        cell.water_depth = 0.0;
        cell.biome = BiomeType::Grassland;
    }

    // Place water in the center (row 2, col 2 => index 12)
    grid.cells[12] = TerrainCell {
        elevation: 0.0,
        water_depth: 10.0,
        biome: BiomeType::Water,
    };

    let mut world = World::new();
    world.insert_resource(grid);
    world.insert_resource(TimeStep(0.0166)); // ~60 FPS dt

    let mut schedule = Schedule::default();
    schedule.add_systems(water_dynamics_system);

    // Run for 10 steps to see symmetric diffusion
    for _ in 0..10 {
        schedule.run(&mut world);
    }

    let grid_after = world.resource::<TerrainGrid>();

    // Neighbors of center cell (2,2): West (2,1 = index 11), East (2,3 = index 13), North (1,2 = index 7), South (3,2 = index 17)
    let w = grid_after.cells[11].water_depth;
    let e = grid_after.cells[13].water_depth;
    let n = grid_after.cells[7].water_depth;
    let s = grid_after.cells[17].water_depth;

    // Check symmetry
    assert!((w - e).abs() < 1e-5, "Asymmetry in West/East diffusion: W={}, E={}", w, e);
    assert!((n - s).abs() < 1e-5, "Asymmetry in North/South diffusion: N={}, S={}", n, s);
    assert!((w - n).abs() < 1e-5, "Asymmetry in orthogonal diffusion: W={}, N={}", w, n);

    // Check all water depths are non-negative
    for cell in &grid_after.cells {
        assert!(cell.water_depth >= 0.0, "Negative water depth found: {}", cell.water_depth);
    }
}

#[test]
fn test_water_dynamics_extremes_steep() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // 1. Standard steep slope (downhill towards row 0)
    {
        let mut grid = TerrainGrid::new(5, 5);
        for z in 0..5 {
            for x in 0..5 {
                let idx = z * 5 + x;
                grid.cells[idx] = TerrainCell {
                    elevation: z as f32 * 10.0, // steep slope rising as z increases
                    water_depth: 0.0,
                    biome: BiomeType::Grassland,
                };
            }
        }

        // Place water in the center (row 2, col 2 => index 12, elevation = 20.0)
        grid.cells[12].water_depth = 5.0;
        grid.cells[12].biome = BiomeType::Water;

        let mut world = World::new();
        world.insert_resource(grid);
        world.insert_resource(TimeStep(0.0166));

        let mut schedule = Schedule::default();
        schedule.add_systems(water_dynamics_system);

        // Run for 10 steps
        for _ in 0..10 {
            schedule.run(&mut world);
        }

        let grid_after = world.resource::<TerrainGrid>();

        // Center is row 2. North is row 1 (z=1, downhill). South is row 3 (z=3, uphill).
        // Elevation: row 1 is 10.0, row 3 is 30.0.
        // Water should flow downhill to North (row 1), but NEVER uphill to South (row 3).
        let downhill_neighbor = grid_after.cells[7].water_depth; // row 1, col 2
        let uphill_neighbor = grid_after.cells[17].water_depth; // row 3, col 2

        assert!(downhill_neighbor > 0.0, "Water failed to flow downhill (row 1 depth is 0)");
        assert_eq!(uphill_neighbor, 0.0, "Water flowed uphill! (row 3 depth is {})", uphill_neighbor);
    }

    // 2. Very steep slope (cliff, elevation difference of 1,000,000)
    {
        let mut grid = TerrainGrid::new(3, 3);
        grid.cells[4] = TerrainCell {
            elevation: 1000000.0,
            water_depth: 10.0,
            biome: BiomeType::Water,
        };
        for idx in &[1, 3, 5, 7] {
            grid.cells[*idx] = TerrainCell {
                elevation: 0.0,
                water_depth: 0.0,
                biome: BiomeType::Grassland,
            };
        }

        let mut world = World::new();
        world.insert_resource(grid);
        world.insert_resource(TimeStep(0.0166));

        let mut schedule = Schedule::default();
        schedule.add_systems(water_dynamics_system);

        // Run for 5 steps. It should transfer all water instantly or smoothly without panic/Nan/Inf.
        for _ in 0..5 {
            schedule.run(&mut world);
        }

        let grid_after = world.resource::<TerrainGrid>();
        for cell in &grid_after.cells {
            assert!(cell.water_depth.is_finite(), "Non-finite water depth found!");
            assert!(cell.water_depth >= 0.0, "Negative water depth found: {}", cell.water_depth);
        }
    }
}

#[test]
fn test_mass_conservation_1000_steps() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let width = 8;
    let height = 8;
    let mut grid = TerrainGrid::new(width, height);
    
    // Create random-like elevations and place initial water
    for z in 0..height {
        for x in 0..width {
            let idx = z * width + x;
            grid.cells[idx] = TerrainCell {
                elevation: ((x * z) as f32).sin() * 5.0,
                water_depth: if (x + z) % 3 == 0 { 2.0 } else { 0.0 },
                biome: BiomeType::Grassland,
            };
        }
    }

    let dt = 0.0166;
    let mut world = World::new();
    world.insert_resource(grid);
    world.insert_resource(TimeStep(dt));

    let mut schedule = Schedule::default();
    schedule.add_systems(water_dynamics_system);

    let initial_water: f32 = world.resource::<TerrainGrid>().cells.iter().map(|c| c.water_depth).sum();
    let mut total_evaporated_accumulated = 0.0;

    for _step in 0..1000 {
        // Clone state before system run
        let grid_before = world.resource::<TerrainGrid>().clone();

        schedule.run(&mut world);

        let grid_after = world.resource::<TerrainGrid>();

        // Verify that sum(net_flow) is 0.0
        let sum_net_flow: f32 = grid_after.water_scratch.iter().sum();
        assert!(sum_net_flow.abs() < 1e-4, "Net flow sum is not zero: {}", sum_net_flow);

        // Calculate evaporation step-by-step
        let mut step_evaporated = 0.0;
        for idx in 0..(width * height) {
            let net_flow = grid_after.water_scratch[idx];
            let cell_before = grid_before.cells[idx];
            let cell_after = grid_after.cells[idx];

            let pre_evap = (cell_before.water_depth + net_flow).max(0.0);
            let evap_loss = pre_evap - cell_after.water_depth;
            assert!(evap_loss >= 0.0, "Negative evaporation loss found in step!");
            step_evaporated += evap_loss;
        }

        total_evaporated_accumulated += step_evaporated;

        let total_water_after: f32 = grid_after.cells.iter().map(|c| c.water_depth).sum();
        let expected_water = initial_water - total_evaporated_accumulated;

        // Verify that total water is conserved (accounting for evaporation)
        assert!((total_water_after - expected_water).abs() < 1e-3,
            "Mass conservation violated! Step: {}, Actual water: {}, Expected: {}",
            _step, total_water_after, expected_water
        );
    }

    println!("Initial water: {}, final water: {}, total evaporated: {}",
             initial_water,
             world.resource::<TerrainGrid>().cells.iter().map(|c| c.water_depth).sum::<f32>(),
             total_evaporated_accumulated);
}

#[test]
fn test_safety_boundaries_and_nan_handling() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // 1. Nan values in elevation/water
    {
        let mut grid = TerrainGrid::new(3, 3);
        grid.cells[4] = TerrainCell {
            elevation: f32::NAN,
            water_depth: f32::NAN,
            biome: BiomeType::Water,
        };

        let mut world = World::new();
        world.insert_resource(grid);
        world.insert_resource(TimeStep(0.0166));

        let mut schedule = Schedule::default();
        schedule.add_systems(water_dynamics_system);

        // Run system, check that it does not panic and recovers
        schedule.run(&mut world);

        let grid_after = world.resource::<TerrainGrid>();
        for cell in &grid_after.cells {
            assert!(cell.water_depth >= 0.0, "NaN resulted in negative water depth");
            assert!(cell.water_depth.is_finite(), "NaN not recovered to 0.0");
        }
    }

    // 2. Inf values in elevation/water
    {
        let mut grid = TerrainGrid::new(3, 3);
        grid.cells[4] = TerrainCell {
            elevation: f32::INFINITY,
            water_depth: f32::INFINITY,
            biome: BiomeType::Water,
        };

        let mut world = World::new();
        world.insert_resource(grid);
        world.insert_resource(TimeStep(0.0166));

        let mut schedule = Schedule::default();
        schedule.add_systems(water_dynamics_system);

        schedule.run(&mut world);

        let grid_after = world.resource::<TerrainGrid>();
        for cell in &grid_after.cells {
            assert!(cell.water_depth >= 0.0, "Inf resulted in negative water depth");
        }
    }
}

#[test]
fn test_division_by_zero_underflow_guard() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Create a scenario where height difference is at the absolute precision limit of f32
    // using elevation and water depth around 1e-4.
    let mut grid = TerrainGrid::new(3, 3);
    
    // Center cell has water depth slightly above 1e-4 threshold
    grid.cells[4] = TerrainCell {
        elevation: -0.0001,
        water_depth: 0.00010001, // h_curr = 0.00000001
        biome: BiomeType::Water,
    };
    
    // Neighbor has slightly lower height
    grid.cells[1] = TerrainCell {
        elevation: -0.0001,
        water_depth: 0.0001, // h_neigh = 0.0
        biome: BiomeType::Water,
    };

    let mut world = World::new();
    world.insert_resource(grid);
    world.insert_resource(TimeStep(0.0166));

    let mut schedule = Schedule::default();
    schedule.add_systems(water_dynamics_system);

    // This should run without dividing by zero or panicking
    schedule.run(&mut world);

    let grid_after = world.resource::<TerrainGrid>();
    assert!(grid_after.cells[4].water_depth.is_finite());
    assert!(grid_after.cells[1].water_depth.is_finite());
}


#[test]
fn test_zero_heap_allocations_during_ticking_challenger() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    let grid = generate_terrain(64, 64, 12345);
    world.insert_resource(grid);
    world.insert_resource(TimeStep(1.0 / 60.0));

    let mut schedule = Schedule::default();
    schedule.add_systems(water_dynamics_system);

    // Warm up Bevy schedule
    schedule.run(&mut world);

    // Enable tracking of allocations
    ALLOCATOR.start_tracking();

    // Execute water dynamics
    schedule.run(&mut world);

    // Stop tracking
    let allocs = ALLOCATOR.stop_tracking();

    assert_eq!(allocs, 0, "Water dynamics system allocated on hot path: {} allocs!", allocs);
}
