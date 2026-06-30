use bevy_ecs::prelude::*;
use glam::Vec3;
use noise::{NoiseFn, Perlin};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use rayon::prelude::*;
use crate::core::resources::MapBounds;
use std::sync::OnceLock;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::BufReader;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug)]
pub struct MapConfig {
    pub sea_ratio: f64,
    pub sand_ratio: f64,
    pub snow_ratio: f64,
    pub noise_scale: f64,
}

impl Default for MapConfig {
    fn default() -> Self {
        Self {
            sea_ratio: 0.3,
            sand_ratio: 0.35,
            snow_ratio: 0.85,
            noise_scale: 2.0,
        }
    }
}

static MAP_CONFIG: OnceLock<MapConfig> = OnceLock::new();

pub fn get_map_config() -> &'static MapConfig {
    MAP_CONFIG.get_or_init(|| {
        let candidates = [
            PathBuf::from("map_config.json"),
            PathBuf::from("src-tauri/map_config.json"),
            PathBuf::from("../map_config.json"),
        ];

        for path in &candidates {
            if path.exists() {
                if let Ok(config) = load_config_file(path) {
                    return config;
                }
            }
        }

        let mut current_dir = std::env::current_dir().ok();
        while let Some(dir) = current_dir {
            let path = dir.join("map_config.json");
            if path.exists() {
                if let Ok(config) = load_config_file(&path) {
                    return config;
                }
            }
            current_dir = dir.parent().map(|p| p.to_path_buf());
        }

        MapConfig::default()
    })
}

fn load_config_file(path: &Path) -> Result<MapConfig, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let config: MapConfig = serde_json::from_reader(reader)?;
    Ok(config)
}

const MAX_EROSION_ITERATIONS: usize = 100_000;
const MAX_DROPLET_LIFETIME: usize = 30;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug)]
pub struct MapSettings {
    pub width: usize,
    pub height: usize,
    pub seed: u32,
    pub octaves: usize,
    pub lacunarity: f64,
    pub gain: f64,
    pub erosion_steps: usize,
}

impl Default for MapSettings {
    fn default() -> Self {
        Self {
            width: 128,
            height: 128,
            seed: 1337,
            octaves: 6,
            lacunarity: 2.0,
            gain: 0.5,
            erosion_steps: 20000,
        }
    }
}

#[repr(u8)]
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiomeType {
    DeepOcean = 0,
    Ocean = 1,
    Beach = 2,
    River = 3,
    Grassland = 4,
    TemperateForest = 5,
    BorealForest = 6,
    Rainforest = 7,
    Desert = 8,
    MountainRock = 9,
    Snow = 10,
}

#[derive(Resource, Clone, Debug)]
pub struct TerrainMap {
    pub width: usize,
    pub height: usize,
    pub elevations: Vec<f32>,
    pub moistures: Vec<f32>,
    pub biomes: Vec<u8>,
    pub flows: Vec<f32>,
    pub pois: Vec<(usize, usize)>,
}

impl TerrainMap {
    pub fn generate(settings: &MapSettings) -> Self {
        let config = get_map_config();
        let sea_val = config.sea_ratio.clamp(0.0, 1.0) as f32;
        let sand_val = config.sand_ratio.clamp(sea_val as f64, 1.0) as f32;
        let snow_val = config.snow_ratio.clamp(sand_val as f64, 1.0) as f32;
        let width = settings.width.max(2);
        let height = settings.height.max(2);

        let elevation_perlin = Perlin::new(settings.seed);
        let moisture_perlin = Perlin::new(settings.seed.wrapping_add(100));

        // Parallel fBm generation for raw elevation
        let mut raw_elevations: Vec<f32> = (0..width * height)
            .into_par_iter()
            .map(|idx| {
                let col = idx % width;
                let row = idx / width;

                let x = (col as f64) / (width as f64) * config.noise_scale;
                let y = (row as f64) / (height as f64) * config.noise_scale;

                let mut value = 0.0;
                let mut amplitude = 1.0;
                let mut frequency = 1.0;
                let mut max_value = 0.0;

                for _ in 0..settings.octaves {
                    value += elevation_perlin.get([x * frequency, y * frequency]) * amplitude;
                    max_value += amplitude;
                    amplitude *= settings.gain;
                    frequency *= settings.lacunarity;
                }

                let elevation_val = ((value / max_value) as f32 + 1.0) / 2.0;
                let dx = (col as f32 / width as f32) * 2.0 - 1.0;
                let dy = (row as f32 / height as f32) * 2.0 - 1.0;
                let dist = (dx * dx + dy * dy).sqrt().min(1.0);
                let falloff = (1.0 - dist * dist).clamp(0.0, 1.0);
                elevation_val * falloff
            })
            .collect();

        // Normalize elevations to [0.0, 1.0]
        let (min_el, max_el) = raw_elevations.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(min_val, max_val), &val| {
            (min_val.min(val), max_val.max(val))
        });
        let el_range = max_el - min_el;
        if el_range > 0.0 {
            raw_elevations.par_iter_mut().for_each(|el| {
                *el = (*el - min_el) / el_range;
            });
        }

        let mut elevations = raw_elevations;
        let mut flows = vec![0.0f32; width * height];
        let mut rng = StdRng::seed_from_u64(settings.seed as u64);
        let erosion_steps = settings.erosion_steps.min(MAX_EROSION_ITERATIONS);

        // Zero-allocation droplet erosion
        for _ in 0..erosion_steps {
            let mut px: f32 = rng.gen_range(0.0..=(width - 2) as f32);
            let mut py: f32 = rng.gen_range(0.0..=(height - 2) as f32);
            let mut dir_x = 0.0f32;
            let mut dir_y = 0.0f32;
            let mut speed = 1.0f32;
            let mut water = 1.0f32;
            let mut sediment = 0.0f32;

            let inertia = 0.05f32;
            let capacity_coeff = 4.0f32;
            let min_slope = 0.01f32;
            let deposit_speed = 0.3f32;
            let erode_speed = 0.3f32;
            let gravity = 4.0f32;
            let evaporation_rate = 0.05f32;

            for _ in 0..MAX_DROPLET_LIFETIME {
                let ipx = px.floor() as usize;
                let ipy = py.floor() as usize;
                if ipx >= width - 1 || ipy >= height - 1 {
                    break;
                }

                let tx = px - ipx as f32;
                let ty = py - ipy as f32;

                let h00 = elevations[ipy * width + ipx];
                let h10 = elevations[ipy * width + ipx + 1];
                let h01 = elevations[(ipy + 1) * width + ipx];
                let h11 = elevations[(ipy + 1) * width + ipx + 1];

                let h = h00 * (1.0 - tx) * (1.0 - ty) + h10 * tx * (1.0 - ty) + h01 * (1.0 - tx) * ty + h11 * tx * ty;
                let grad_x = (h10 - h00) * (1.0 - ty) + (h11 - h01) * ty;
                let grad_y = (h01 - h00) * (1.0 - tx) + (h11 - h10) * tx;

                flows[ipy * width + ipx] += water;

                dir_x = dir_x * inertia - grad_x * (1.0 - inertia);
                dir_y = dir_y * inertia - grad_y * (1.0 - inertia);

                let len = (dir_x * dir_x + dir_y * dir_y).sqrt();
                if len > 0.0001 {
                    dir_x /= len;
                    dir_y /= len;
                } else {
                    dir_x = rng.gen_range(-1.0..1.0);
                    dir_y = rng.gen_range(-1.0..1.0);
                    let len = (dir_x * dir_x + dir_y * dir_y).sqrt();
                    if len > 0.0001 {
                        dir_x /= len;
                        dir_y /= len;
                    }
                }

                let new_px = px + dir_x;
                let new_py = py + dir_y;

                if new_px < 0.0 || new_px >= (width - 1) as f32 || new_py < 0.0 || new_py >= (height - 1) as f32 {
                    break;
                }

                let new_ipx = new_px.floor() as usize;
                let new_ipy = new_py.floor() as usize;
                let new_tx = new_px - new_ipx as f32;
                let new_ty = new_py - new_ipy as f32;

                let nh00 = elevations[new_ipy * width + new_ipx];
                let nh10 = elevations[new_ipy * width + new_ipx + 1];
                let nh01 = elevations[(new_ipy + 1) * width + new_ipx];
                let nh11 = elevations[(new_ipy + 1) * width + new_ipx + 1];
                let new_h = nh00 * (1.0 - new_tx) * (1.0 - new_ty) + nh10 * new_tx * (1.0 - new_ty) + nh01 * (1.0 - new_tx) * new_ty + nh11 * new_tx * new_ty;

                let delta_h = new_h - h;

                if delta_h > 0.0 {
                    let deposit = delta_h.min(sediment);
                    sediment -= deposit;

                    elevations[ipy * width + ipx] += deposit * (1.0 - tx) * (1.0 - ty);
                    elevations[ipy * width + ipx + 1] += deposit * tx * (1.0 - ty);
                    elevations[(ipy + 1) * width + ipx] += deposit * (1.0 - tx) * ty;
                    elevations[(ipy + 1) * width + ipx + 1] += deposit * tx * ty;
                } else {
                    let capacity = (-delta_h).max(min_slope) * speed * water * capacity_coeff;
                    if sediment > capacity {
                        let deposit = (sediment - capacity) * deposit_speed;
                        sediment -= deposit;

                        elevations[ipy * width + ipx] += deposit * (1.0 - tx) * (1.0 - ty);
                        elevations[ipy * width + ipx + 1] += deposit * tx * (1.0 - ty);
                        elevations[(ipy + 1) * width + ipx] += deposit * (1.0 - tx) * ty;
                        elevations[(ipy + 1) * width + ipx + 1] += deposit * tx * ty;
                    } else {
                        let erode = (capacity - sediment).min(-delta_h) * erode_speed;
                        sediment += erode;

                        elevations[ipy * width + ipx] -= erode * (1.0 - tx) * (1.0 - ty);
                        elevations[ipy * width + ipx + 1] -= erode * tx * (1.0 - ty);
                        elevations[(ipy + 1) * width + ipx] -= erode * (1.0 - tx) * ty;
                        elevations[(ipy + 1) * width + ipx + 1] -= erode * tx * ty;
                    }
                }

                speed = (speed * speed + delta_h * delta_h * gravity).sqrt();
                water *= 1.0 - evaporation_rate;

                px = new_px;
                py = new_py;
            }
        }

        // Re-normalize elevations to [0.0, 1.0] after erosion
        let (min_el, max_el) = elevations.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(min_val, max_val), &val| {
            (min_val.min(val), max_val.max(val))
        });
        let el_range = max_el - min_el;
        if el_range > 0.0 {
            elevations.par_iter_mut().for_each(|el| {
                *el = (*el - min_el) / el_range;
            });
        }

        let max_flow = flows.iter().fold(0.0f32, |max_val, &val| max_val.max(val));
        let norm_max_flow = if max_flow == 0.0 { 1.0 } else { max_flow };

        // Parallel fBm generation for moisture
        let moistures: Vec<f32> = (0..width * height)
            .into_par_iter()
            .map(|idx| {
                let col = idx % width;
                let row = idx / width;

                let x = (col as f64) / (width as f64) * config.noise_scale;
                let y = (row as f64) / (height as f64) * config.noise_scale;

                let mut value = 0.0;
                let mut amplitude = 1.0;
                let mut frequency = 1.0;
                let mut max_value = 0.0;

                for _ in 0..settings.octaves {
                    value += moisture_perlin.get([x * frequency, y * frequency]) * amplitude;
                    max_value += amplitude;
                    amplitude *= settings.gain;
                    frequency *= settings.lacunarity;
                }

                let base_moisture = ((value / max_value) as f32 + 1.0) / 2.0;
                let flow_ratio = flows[idx] / norm_max_flow;
                let flow_effect = flow_ratio * 0.4;

                (base_moisture + flow_effect).clamp(0.0, 1.0)
            })
            .collect();

        // Biome determination
        let biomes: Vec<u8> = (0..width * height)
            .into_par_iter()
            .map(|idx| {
                let elevation = elevations[idx];
                let moisture = moistures[idx];
                let flow = flows[idx];

                let biome = if elevation < sea_val / 2.0 {
                    BiomeType::DeepOcean
                } else if elevation < sea_val {
                    BiomeType::Ocean
                } else if elevation < sand_val {
                    BiomeType::Beach
                } else if elevation < 0.9 && flow > (max_flow * 0.05) && flow > 2.0 {
                    BiomeType::River
                } else if elevation >= 0.7 {
                    if elevation >= snow_val {
                        if moisture > 0.5 {
                            BiomeType::Snow
                        } else {
                            BiomeType::MountainRock
                        }
                    } else {
                        if moisture > 0.65 {
                            BiomeType::Rainforest
                        } else if moisture > 0.4 {
                            BiomeType::BorealForest
                        } else if moisture > 0.25 {
                            BiomeType::TemperateForest
                        } else {
                            BiomeType::MountainRock
                        }
                    }
                } else {
                    if moisture < 0.15 {
                        BiomeType::Desert
                    } else if moisture < 0.35 {
                        BiomeType::Grassland
                    } else if moisture < 0.55 {
                        BiomeType::TemperateForest
                    } else if moisture < 0.75 {
                        BiomeType::BorealForest
                    } else {
                        BiomeType::Rainforest
                    }
                };

                biome as u8
            })
            .collect();

        // Select exactly 10 random land-bound POIs (elevation >= 0.35) using StdRng (seed: settings.seed as u64 + 100)
        let mut pois = Vec::with_capacity(10);
        let mut poi_rng = StdRng::seed_from_u64(settings.seed as u64 + 100);
        let mut land_count = 0;
        for idx in 0..width * height {
            if elevations[idx] >= sand_val {
                let col = idx % width;
                let row = idx / width;
                land_count += 1;
                if pois.len() < 10 {
                    pois.push((col, row));
                } else {
                    let j = poi_rng.gen_range(0..land_count);
                    if j < 10 {
                        pois[j] = (col, row);
                    }
                }
            }
        }

        Self {
            width,
            height,
            elevations,
            moistures,
            biomes,
            flows,
            pois,
        }
    }

    pub fn get_map_indices(&self, pos: Vec3, bounds: &MapBounds) -> Option<(usize, usize)> {
        let x_range = bounds.max.x - bounds.min.x;
        let z_range = bounds.max.z - bounds.min.z;
        if x_range <= 0.0 || z_range <= 0.0 {
            return None;
        }
        let tx = (pos.x - bounds.min.x) / x_range;
        let tz = (pos.z - bounds.min.z) / z_range;
        if tx < 0.0 || tx > 1.0 || tz < 0.0 || tz > 1.0 {
            return None;
        }
        let col = ((tx * self.width as f32) as usize).min(self.width - 1);
        let row = ((tz * self.height as f32) as usize).min(self.height - 1);
        Some((col, row))
    }

    pub fn get_elevation_at_pos(&self, pos: Vec3, bounds: &MapBounds) -> f32 {
        let x_range = bounds.max.x - bounds.min.x;
        let z_range = bounds.max.z - bounds.min.z;
        if x_range <= 0.0 || z_range <= 0.0 {
            return 0.0;
        }
        let tx = (pos.x - bounds.min.x) / x_range;
        let tz = (pos.z - bounds.min.z) / z_range;
        
        let tx = tx.clamp(0.0, 1.0);
        let tz = tz.clamp(0.0, 1.0);

        let fx = tx * (self.width - 1) as f32;
        let fz = tz * (self.height - 1) as f32;

        let ix = (fx.floor() as usize).min(self.width - 1);
        let iz = (fz.floor() as usize).min(self.height - 1);

        let tx = fx - ix as f32;
        let tz = fz - iz as f32;

        let next_x = (ix + 1).min(self.width - 1);
        let next_z = (iz + 1).min(self.height - 1);

        let h00 = self.elevations[iz * self.width + ix];
        let h10 = self.elevations[iz * self.width + next_x];
        let h01 = self.elevations[next_z * self.width + ix];
        let h11 = self.elevations[next_z * self.width + next_x];

        let h_top = h00 * (1.0 - tx) + h10 * tx;
        let h_bottom = h01 * (1.0 - tx) + h11 * tx;
        
        h_top * (1.0 - tz) + h_bottom * tz
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_heightmap_range_normalization() {
        let settings = MapSettings::default();
        let map = TerrainMap::generate(&settings);
        assert!(!map.elevations.is_empty());
        for &elevation in &map.elevations {
            assert!(elevation >= 0.0 && elevation <= 1.0);
        }
        
        // Assert min is close to 0 and max is close to 1
        let min_el = map.elevations.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_el = map.elevations.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        assert!(min_el < 0.05);
        assert!(max_el > 0.95);
    }

    #[test]
    fn test_erosion_alters_topography() {
        let mut settings_no_erosion = MapSettings::default();
        settings_no_erosion.erosion_steps = 0;
        let map_no_erosion = TerrainMap::generate(&settings_no_erosion);

        let mut settings_with_erosion = MapSettings::default();
        settings_with_erosion.erosion_steps = 20000;
        let map_with_erosion = TerrainMap::generate(&settings_with_erosion);

        assert_eq!(map_no_erosion.elevations.len(), map_with_erosion.elevations.len());
        
        let mut diff_count = 0;
        for i in 0..map_no_erosion.elevations.len() {
            if (map_no_erosion.elevations[i] - map_with_erosion.elevations[i]).abs() > 1e-5 {
                diff_count += 1;
            }
        }
        
        // Erosion should have altered the topography significantly
        assert!(diff_count > 0, "Erosion must alter at least some raw height values");
    }

    #[test]
    fn test_reproducibility_via_seed() {
        let settings_1 = MapSettings {
            width: 64,
            height: 64,
            seed: 42,
            ..Default::default()
        };
        let settings_2 = MapSettings {
            width: 64,
            height: 64,
            seed: 42,
            ..Default::default()
        };
        let settings_3 = MapSettings {
            width: 64,
            height: 64,
            seed: 43,
            ..Default::default()
        };

        let map_1 = TerrainMap::generate(&settings_1);
        let map_2 = TerrainMap::generate(&settings_2);
        let map_3 = TerrainMap::generate(&settings_3);

        // Same seeds must match
        assert_eq!(map_1.elevations, map_2.elevations);
        assert_eq!(map_1.biomes, map_2.biomes);

        // Different seeds must differ
        let mut diff = false;
        for i in 0..map_1.elevations.len() {
            if (map_1.elevations[i] - map_3.elevations[i]).abs() > 1e-5 {
                diff = true;
                break;
            }
        }
        assert!(diff, "Different seeds should produce different heightmaps");
    }

    #[test]
    fn test_generate_sample_image() {
        let settings = MapSettings {
            width: 1024,
            height: 1024,
            seed: 1337,
            octaves: 6,
            lacunarity: 2.0,
            gain: 0.5,
            erosion_steps: 20000,
        };

        let map = TerrainMap::generate(&settings);

        // Map biomes to RGB colors
        let mut img_buf = image::ImageBuffer::new(1024, 1024);
        for (x, y, pixel) in img_buf.enumerate_pixels_mut() {
            let idx = (y as usize) * 1024 + (x as usize);
            let biome = map.biomes[idx];

            let mut rgb: [u8; 3] = match biome {
                0 => [15, 25, 65],      // DeepOcean
                1 => [35, 80, 145],     // Ocean
                2 => [235, 215, 165],   // Beach
                3 => [55, 135, 215],    // River
                4 => [140, 190, 100],   // Grassland
                5 => [65, 135, 75],     // TemperateForest
                6 => [45, 95, 80],      // BorealForest
                7 => [15, 85, 45],      // Rainforest
                8 => [215, 175, 105],   // Desert
                9 => [120, 125, 130],   // MountainRock
                10 => [245, 248, 250],  // Snow
                _ => [0, 0, 0],
            };

            // Grid overlay
            if x % 64 == 0 || y % 64 == 0 {
                rgb[0] = (rgb[0] as f32 * 0.85 + 255.0 * 0.15).round() as u8;
                rgb[1] = (rgb[1] as f32 * 0.85 + 255.0 * 0.15).round() as u8;
                rgb[2] = (rgb[2] as f32 * 0.85 + 255.0 * 0.15).round() as u8;
            }

            *pixel = image::Rgb(rgb);
        }

        // Draw green POI crosses
        for &(px, py) in &map.pois {
            // Horizontal line
            for dx in -2..=2 {
                let cx = px as isize + dx;
                if cx >= 0 && cx < 1024 {
                    img_buf.put_pixel(cx as u32, py as u32, image::Rgb([0, 255, 0]));
                }
            }
            // Vertical line
            for dy in -2..=2 {
                let cy = py as isize + dy;
                if cy >= 0 && cy < 1024 {
                    img_buf.put_pixel(px as u32, cy as u32, image::Rgb([0, 255, 0]));
                }
            }
        }

        let output_path = Path::new(r"e:\Project\Anima-Engine\anima_world_refined.png");
        img_buf.save(output_path).expect("Failed to save sample map image");
        assert!(output_path.exists());
    }

    #[test]
    fn test_terrain_map_dimensions_validation() {
        let settings = MapSettings {
            width: 1,
            height: 1,
            ..Default::default()
        };
        let map = TerrainMap::generate(&settings);
        assert_eq!(map.width, 2);
        assert_eq!(map.height, 2);
        assert_eq!(map.elevations.len(), 4);
        assert_eq!(map.biomes.len(), 4);
    }

    #[test]
    fn test_pois_are_on_land() {
        let settings = MapSettings::default();
        let map = TerrainMap::generate(&settings);
        assert!(!map.pois.is_empty());
        assert_eq!(map.pois.len(), 10);
        let config = get_map_config();
        for &(col, row) in &map.pois {
            let idx = row * map.width + col;
            assert!(map.elevations[idx] >= config.sand_ratio as f32, "POI at ({}, {}) has elevation {}, which is below {}", col, row, map.elevations[idx], config.sand_ratio);
        }
    }
}
