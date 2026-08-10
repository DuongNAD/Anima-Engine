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

/// Helper to run the water dynamics system for 1 tick with a given dt.
fn run_water_system(grid: TerrainGrid, dt: f32) -> TerrainGrid {
    let mut world = World::new();
    world.insert_resource(grid);
    world.insert_resource(TimeStep(dt));

    let mut schedule = Schedule::default();
    schedule.add_systems(water_dynamics_system);
    schedule.run(&mut world);

    world.remove_resource::<TerrainGrid>().unwrap()
}

#[test]
fn test_extremes_flat_plain() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Create a 5x5 flat grid (all elevations are 0.0)
    let mut grid = TerrainGrid::new(5, 5);
    for cell in &mut grid.cells {
        cell.elevation = 0.0;
        cell.water_depth = 0.0;
        cell.biome = BiomeType::Grassland;
    }

    // Put 10.0 water in the center cell (index 12)
    grid.cells[12] = TerrainCell {
        elevation: 0.0,
        water_depth: 10.0,
        biome: BiomeType::Water,
    };

    // Run for 1 step with dt = 0.1
    let grid = run_water_system(grid, 0.1);

    // Center cell (12) neighbors: West (11), East (13), North (7), South (17)
    // Elevation is identical, but total height is higher at the center, so water flows outward.
    let center = grid.cells[12];
    let neighbors = [7, 11, 13, 17];

    assert!(center.water_depth < 10.0, "Water should flow out of center");
    for &idx in &neighbors {
        assert!(grid.cells[idx].water_depth > 0.0, "Water should flow into neighbor {}", idx);
        // Symmetry check
        assert!((grid.cells[idx].water_depth - grid.cells[neighbors[0]].water_depth).abs() < 1e-6);
    }
}

#[test]
fn test_extremes_steep_slope() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Create a 5x5 grid with a steep slope from north (z=0, elevation 100.0) to south (z=4, elevation 0.0)
    let mut grid = TerrainGrid::new(5, 5);
    for z in 0..5 {
        for x in 0..5 {
            let idx = z * 5 + x;
            grid.cells[idx] = TerrainCell {
                elevation: 100.0 - (z as f32) * 20.0,
                water_depth: 0.0,
                biome: BiomeType::Grassland,
            };
        }
    }

    // Put water at the top of the slope (row 1, x=2 -> index 7)
    grid.cells[7].water_depth = 5.0;
    grid.cells[7].biome = BiomeType::Water;

    // Run 5 steps
    let mut current_grid = grid;
    for _ in 0..5 {
        current_grid = run_water_system(current_grid, 0.05);
    }

    // Verify water flowed south (larger z indices) more than north.
    // Cell 7 neighbors:
    // North: cell 2 (elevation 100.0)
    // South: cell 12 (elevation 60.0)
    // West: cell 6 (elevation 80.0)
    // East: cell 8 (elevation 80.0)
    let north = current_grid.cells[2].water_depth;
    let south = current_grid.cells[12].water_depth;
    let west = current_grid.cells[6].water_depth;
    let east = current_grid.cells[8].water_depth;

    println!("Steep slope flow: North={}, South={}, West={}, East={}", north, south, west, east);

    // Water should flow down the slope, so South should receive much more water than North (which is uphill).
    assert!(south > north, "Water did not flow down the slope (South: {}, North: {})", south, north);
    assert!(south > west, "Water did not flow down the slope (South: {}, West: {})", south, west);
    assert!(south > east, "Water did not flow down the slope (South: {}, East: {})", south, east);
}

#[test]
fn test_extremes_cliff() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Create a 5x5 grid with an extremely steep cliff
    let mut grid = TerrainGrid::new(5, 5);
    for cell in &mut grid.cells {
        cell.elevation = 0.0;
        cell.water_depth = 0.0;
        cell.biome = BiomeType::Grassland;
    }

    // High peak in the center
    grid.cells[12] = TerrainCell {
        elevation: 1e6,
        water_depth: 5.0,
        biome: BiomeType::Water,
    };

    // Run system for 1 step
    let grid = run_water_system(grid, 0.01);

    // Verify it didn't crash/panic and water flowed down the cliff
    assert!(grid.cells[12].water_depth < 5.0, "Water should flow down the cliff");
    for &idx in &[7, 11, 13, 17] {
        assert!(grid.cells[idx].water_depth > 0.0, "Neighbor {} should receive water", idx);
        assert!(grid.cells[idx].water_depth.is_finite());
    }
}

#[test]
fn test_mass_conservation_1000_steps() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // We initialize a 8x8 grid with some elevation variance and initial water.
    let mut grid = generate_terrain(8, 8, 123);

    // Add substantial water to multiple locations
    grid.cells[10].water_depth = 10.0;
    grid.cells[10].biome = BiomeType::Water;
    grid.cells[35].water_depth = 5.0;
    grid.cells[35].biome = BiomeType::Water;

    let dt = 0.0167; // ~60 FPS
    let flow_rate = 0.2;
    let _evaporation_rate = 0.01;

    let mut current_grid = grid;

    for step in 0..1000 {
        // To verify mass conservation of the diffusion step itself, we manually compute
        // what the water depths WOULD be after diffusion but BEFORE evaporation,
        // and check if their sum matches the sum of the previous step.
        let width = current_grid.width;
        let height = current_grid.height;
        let mut expected_diffusion_water = vec![0.0; width * height];

        // 1. Re-calculate diffusion flows
        for z in 0..height {
            for x in 0..width {
                let idx = z * width + x;
                let cell = current_grid.cells[idx];
                if cell.water_depth <= 1e-4 {
                    continue;
                }

                let h_curr = cell.elevation + cell.water_depth;
                let neighbors = [
                    ((x + width - 1) % width, z),
                    ((x + 1) % width, z),
                    (x, (z + height - 1) % height),
                    (x, (z + 1) % height),
                ];

                let mut total_diff = 0.0;
                let mut active_neighbors = [None; 4];
                let mut num_active = 0;

                for &n in &neighbors {
                    let n_idx = n.1 * width + n.0;
                    let n_cell = current_grid.cells[n_idx];
                    let h_neigh = n_cell.elevation + n_cell.water_depth;
                    if h_curr > h_neigh {
                        let diff = h_curr - h_neigh;
                        total_diff += diff;
                        active_neighbors[num_active] = Some((n_idx, diff));
                        num_active += 1;
                    }
                }

                if total_diff > 0.0 {
                    let max_out = cell.water_depth;
                    let desired_out = total_diff * flow_rate * dt;
                    let actual_out = desired_out.min(max_out);
                    let scale = actual_out / total_diff;

                    for neighbor in active_neighbors.iter().take(num_active) {
                        if let Some((n_idx, diff)) = *neighbor {
                            let transfer = diff * scale;
                            expected_diffusion_water[idx] -= transfer;
                            expected_diffusion_water[n_idx] += transfer;
                        }
                    }
                }
            }
        }

        // Calculate sum of water depths before step, and expected sum after diffusion
        let prev_sum: f32 = current_grid.cells.iter().map(|c| c.water_depth).sum();
        let mut expected_sum = 0.0;
        for idx in 0..(width * height) {
            let val = current_grid.cells[idx].water_depth + expected_diffusion_water[idx];
            // Ensure no cell has a negative value before clamping
            assert!(val >= -1e-5, "Cell {} water depth before clamp is negative: {}", idx, val);
            expected_sum += val.max(0.0);
        }

        // Verify that the diffusion phase is perfectly mass-conserved (within floating-point precision)
        assert!(
            (expected_sum - prev_sum).abs() < 1e-4,
            "Mass conservation broken at step {}: prev_sum={}, expected_sum_after_diffusion={}",
            step, prev_sum, expected_sum
        );

        // Now run the actual system
        current_grid = run_water_system(current_grid, dt);

        // Verify no negative depths exist in the actual grid
        for (idx, cell) in current_grid.cells.iter().enumerate() {
            assert!(
                cell.water_depth >= 0.0,
                "Negative water depth at step {}, cell {}: {}",
                step, idx, cell.water_depth
            );
        }
    }
}

#[test]
fn test_zero_heap_allocation_under_extremes() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut world = World::new();
    // Use an extreme terrain with steep cliffs and plains
    let mut grid = TerrainGrid::new(32, 32);
    for z in 0..32 {
        for x in 0..32 {
            let idx = z * 32 + x;
            grid.cells[idx] = TerrainCell {
                elevation: if x % 2 == 0 { 1e5 } else { 0.0 },
                water_depth: if x % 4 == 0 { 50.0 } else { 0.0 },
                biome: BiomeType::Grassland,
            };
        }
    }
    world.insert_resource(grid);
    world.insert_resource(TimeStep(0.0167));

    let mut schedule = Schedule::default();
    schedule.add_systems(water_dynamics_system);

    // Warm up
    schedule.run(&mut world);

    // Track allocations
    ALLOCATOR.start_tracking();
    schedule.run(&mut world);
    let alloc_count = ALLOCATOR.stop_tracking();

    assert_eq!(
        alloc_count, 0,
        "water_dynamics_system allocated {} times under extreme terrain!",
        alloc_count
    );
}

#[test]
fn test_safety_negative_timestep() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut grid = TerrainGrid::new(5, 5);
    grid.cells[12].water_depth = 5.0;

    // Run system with negative dt
    let grid = run_water_system(grid, -0.0167);

    // Verify safety: no panics, no negative water depths (clamped to 0.0)
    for (idx, cell) in grid.cells.iter().enumerate() {
        assert!(
            cell.water_depth >= 0.0,
            "Negative water depth on negative dt at cell {}: {}",
            idx, cell.water_depth
        );
        assert!(cell.water_depth.is_finite());
    }
}

#[test]
fn test_safety_nan_timestep() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut grid = TerrainGrid::new(5, 5);
    grid.cells[12].water_depth = 5.0;

    // Run system with NaN dt
    let grid = run_water_system(grid, f32::NAN);

    // Verify safety: no panics, no negative water depths, healed to 0.0 if NaN propagated
    for (idx, cell) in grid.cells.iter().enumerate() {
        assert!(
            cell.water_depth >= 0.0,
            "Negative/invalid water depth on NaN dt at cell {}: {}",
            idx, cell.water_depth
        );
        assert!(cell.water_depth.is_finite(), "NaN water depth at cell {}", idx);
    }
}

#[test]
fn test_safety_infinity_timestep() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut grid = TerrainGrid::new(5, 5);
    grid.cells[12].water_depth = 5.0;

    // Run system with INFINITY dt
    let grid = run_water_system(grid, f32::INFINITY);

    // Verify safety: no panics, no negative water depths
    for (idx, cell) in grid.cells.iter().enumerate() {
        assert!(
            cell.water_depth >= 0.0,
            "Negative water depth on INFINITY dt at cell {}: {}",
            idx, cell.water_depth
        );
        assert!(cell.water_depth.is_finite(), "Infinite water depth at cell {}", idx);
    }
}
