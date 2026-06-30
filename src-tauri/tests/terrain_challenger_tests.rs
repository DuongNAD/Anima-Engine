mod common;

use std::sync::Mutex;
use anima_engine_lib::core::terrain::{TerrainMap, MapSettings};

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator = common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_terrain_dimensions_zero_or_one() {
    let _lock = TEST_LOCK.lock().unwrap();

    // Width and height = 0
    let settings_0 = MapSettings {
        width: 0,
        height: 0,
        erosion_steps: 1000,
        ..Default::default()
    };
    let map_0 = TerrainMap::generate(&settings_0);
    assert!(map_0.width >= 2, "Width must be at least 2");
    assert!(map_0.height >= 2, "Height must be at least 2");
    assert_eq!(map_0.elevations.len(), map_0.width * map_0.height);

    // Width and height = 1
    let settings_1 = MapSettings {
        width: 1,
        height: 1,
        erosion_steps: 1000,
        ..Default::default()
    };
    let map_1 = TerrainMap::generate(&settings_1);
    assert!(map_1.width >= 2, "Width must be at least 2");
    assert!(map_1.height >= 2, "Height must be at least 2");
    assert_eq!(map_1.elevations.len(), map_1.width * map_1.height);
}

#[test]
fn test_terrain_elevation_strictly_bounded() {
    let _lock = TEST_LOCK.lock().unwrap();

    let settings_seeds = [1337, 0, u32::MAX, 42];
    let erosion_options = [0, 1000, 20000];

    for &seed in &settings_seeds {
        for &erosion_steps in &erosion_options {
            let settings = MapSettings {
                seed,
                erosion_steps,
                width: 64,
                height: 64,
                ..Default::default()
            };
            let map = TerrainMap::generate(&settings);
            
            for &el in &map.elevations {
                assert!(el >= 0.0 && el <= 1.0, "Elevation {} out of bounds [0, 1] for seed {} and erosion {}", el, seed, erosion_steps);
            }
        }
    }
}

#[test]
fn test_terrain_reproducibility() {
    let _lock = TEST_LOCK.lock().unwrap();

    let settings_a1 = MapSettings {
        seed: 9876,
        erosion_steps: 5000,
        width: 64,
        height: 64,
        ..Default::default()
    };
    let settings_a2 = MapSettings {
        seed: 9876,
        erosion_steps: 5000,
        width: 64,
        height: 64,
        ..Default::default()
    };
    let settings_b = MapSettings {
        seed: 9877,
        erosion_steps: 5000,
        width: 64,
        height: 64,
        ..Default::default()
    };

    let map_a1 = TerrainMap::generate(&settings_a1);
    let map_a2 = TerrainMap::generate(&settings_a2);
    let map_b = TerrainMap::generate(&settings_b);

    assert_eq!(map_a1.elevations, map_a2.elevations, "Elevations with same seed must match");
    assert_eq!(map_a1.moistures, map_a2.moistures, "Moistures with same seed must match");
    assert_eq!(map_a1.biomes, map_a2.biomes, "Biomes with same seed must match");
    assert_eq!(map_a1.flows, map_a2.flows, "Flows with same seed must match");

    assert_ne!(map_a1.elevations, map_b.elevations, "Elevations with different seeds should differ");
}

#[test]
fn test_terrain_zero_heap_allocations_erosion_hotpath() {
    let _lock = TEST_LOCK.lock().unwrap();

    let settings_no_erosion = MapSettings {
        width: 64,
        height: 64,
        erosion_steps: 0,
        ..Default::default()
    };

    let settings_with_erosion = MapSettings {
        width: 64,
        height: 64,
        erosion_steps: 5000,
        ..Default::default()
    };

    // Warm-up to initialize Rayon threads, lazy allocations, etc.
    let _ = TerrainMap::generate(&settings_no_erosion);
    let _ = TerrainMap::generate(&settings_with_erosion);

    // Track allocations for no erosion
    ALLOCATOR.start_tracking();
    let map_no_erosion = TerrainMap::generate(&settings_no_erosion);
    let allocs_no_erosion = ALLOCATOR.stop_tracking();

    // Track allocations for erosion
    ALLOCATOR.start_tracking();
    let map_with_erosion = TerrainMap::generate(&settings_with_erosion);
    let allocs_with_erosion = ALLOCATOR.stop_tracking();

    println!("Allocs without erosion: {}", allocs_no_erosion);
    println!("Allocs with erosion: {}", allocs_with_erosion);

    // Assert that adding erosion steps does not increase the number of allocations!
    // Since the erosion hot path has zero allocations, the allocation count should be exactly the same.
    assert!(
        allocs_with_erosion <= allocs_no_erosion,
        "Erosion hot path allocated memory! No erosion: {}, with erosion: {}",
        allocs_no_erosion,
        allocs_with_erosion
    );

    // Double check map generated has actually run erosion (i.e. is different and valid)
    assert_ne!(map_no_erosion.elevations, map_with_erosion.elevations);
}
