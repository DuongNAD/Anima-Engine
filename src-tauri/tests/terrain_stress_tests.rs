use anima_engine_lib::core::terrain::{TerrainMap, MapSettings, BiomeType};
use anima_engine_lib::core::resources::MapBounds;
use glam::Vec3;

#[test]
fn test_terrain_extreme_parameters() {
    // 1. Test standard map generation
    let settings = MapSettings::default();
    let map = TerrainMap::generate(&settings);
    assert_eq!(map.width, 128);
    assert_eq!(map.height, 128);
    assert_eq!(map.elevations.len(), 128 * 128);
    assert_eq!(map.biomes.len(), 128 * 128);

    // 2. Test map generation with 0 erosion steps (should work and produce 0 flows)
    let settings_no_erosion = MapSettings {
        erosion_steps: 0,
        ..Default::default()
    };
    let map_no_erosion = TerrainMap::generate(&settings_no_erosion);
    assert_eq!(map_no_erosion.flows.iter().sum::<f32>(), 0.0);

    // 3. Test get_elevation_at_pos with valid bounds
    let bounds = MapBounds {
        min: Vec3::new(-100.0, 0.0, -100.0),
        max: Vec3::new(100.0, 10.0, 100.0),
    };
    
    // Corners and center
    let el_center = map.get_elevation_at_pos(Vec3::new(0.0, 0.0, 0.0), &bounds);
    assert!(el_center >= 0.0 && el_center <= 1.0);

    let el_min = map.get_elevation_at_pos(Vec3::new(-100.0, 0.0, -100.0), &bounds);
    assert!(el_min >= 0.0 && el_min <= 1.0);

    let el_max = map.get_elevation_at_pos(Vec3::new(100.0, 0.0, 100.0), &bounds);
    assert!(el_max >= 0.0 && el_max <= 1.0);

    // Out of bounds pos should clamp to bounds edge
    let el_out = map.get_elevation_at_pos(Vec3::new(-200.0, 0.0, 500.0), &bounds);
    assert!(el_out >= 0.0 && el_out <= 1.0);

    // 4. Test invalid/zero bounds for get_elevation_at_pos
    let zero_bounds = MapBounds {
        min: Vec3::ZERO,
        max: Vec3::ZERO,
    };
    let el_zero = map.get_elevation_at_pos(Vec3::new(5.0, 0.0, 5.0), &zero_bounds);
    assert_eq!(el_zero, 0.0);

    // 5. Test invalid/zero bounds for get_map_indices
    let indices_zero = map.get_map_indices(Vec3::new(5.0, 0.0, 5.0), &zero_bounds);
    assert!(indices_zero.is_none());
}

#[test]
fn test_terrain_no_panic_on_too_small_width() {
    let settings = MapSettings {
        width: 1,
        height: 128,
        ..Default::default()
    };
    let map = TerrainMap::generate(&settings);
    assert_eq!(map.width, 2);
    assert_eq!(map.height, 128);
}

#[test]
fn test_terrain_no_panic_on_too_small_height() {
    let settings = MapSettings {
        width: 128,
        height: 1,
        ..Default::default()
    };
    let map = TerrainMap::generate(&settings);
    assert_eq!(map.width, 128);
    assert_eq!(map.height, 2);
}
