use anima_engine_lib::core::terrain::{MapSettings, TerrainMap};

#[test]
fn test_heightmap_range_strict() {
    let seeds = [0, 42, 1337, 99999];
    let sizes = [64, 128, 256];

    for &seed in &seeds {
        for &size in &sizes {
            let settings = MapSettings {
                width: size,
                height: size,
                seed,
                octaves: 6,
                lacunarity: 2.0,
                gain: 0.5,
                erosion_steps: 2000,
            };

            let map = TerrainMap::generate(&settings);

            assert!(!map.elevations.is_empty());
            assert_eq!(map.elevations.len(), size * size);

            for &el in &map.elevations {
                assert!(
                    el >= 0.0,
                    "Elevation was less than 0.0: {} for seed {} size {}",
                    el,
                    seed,
                    size
                );
                assert!(
                    el <= 1.0,
                    "Elevation was greater than 1.0: {} for seed {} size {}",
                    el,
                    seed,
                    size
                );
                assert!(
                    !el.is_nan(),
                    "Elevation was NaN for seed {} size {}",
                    seed,
                    size
                );
            }
        }
    }
}

#[test]
fn test_seed_reproducibility() {
    let settings_1 = MapSettings {
        width: 128,
        height: 128,
        seed: 12345,
        octaves: 6,
        lacunarity: 2.0,
        gain: 0.5,
        erosion_steps: 1000,
    };

    let settings_2 = MapSettings {
        width: 128,
        height: 128,
        seed: 12345,
        octaves: 6,
        lacunarity: 2.0,
        gain: 0.5,
        erosion_steps: 1000,
    };

    let settings_different_seed = MapSettings {
        width: 128,
        height: 128,
        seed: 54321,
        octaves: 6,
        lacunarity: 2.0,
        gain: 0.5,
        erosion_steps: 1000,
    };

    let map_1 = TerrainMap::generate(&settings_1);
    let map_2 = TerrainMap::generate(&settings_2);
    let map_diff = TerrainMap::generate(&settings_different_seed);

    // Identical seeds must produce identical maps
    assert_eq!(map_1.elevations, map_2.elevations);
    assert_eq!(map_1.moistures, map_2.moistures);
    assert_eq!(map_1.biomes, map_2.biomes);
    assert_eq!(map_1.flows, map_2.flows);

    // Different seeds must produce different maps
    assert_ne!(map_1.elevations, map_diff.elevations);
    assert_ne!(map_1.biomes, map_diff.biomes);
}
