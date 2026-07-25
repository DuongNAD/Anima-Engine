use crate::core::resources::MapBounds;
use bevy_ecs::prelude::*;
use glam::Vec3;
use noise::{NoiseFn, Perlin};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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

#[derive(Resource, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TerrainMap {
    pub width: usize,
    pub height: usize,
    pub elevations: Vec<f32>,
    pub moistures: Vec<f32>,
    /// Per-cell temperature in [0, 1] derived from latitude + altitude lapse rate.
    pub temperatures: Vec<f32>,
    pub biomes: Vec<u8>,
    pub flows: Vec<f32>,
    pub pois: Vec<(usize, usize)>,
}

/// Fractal Brownian motion (summed octaves of Perlin). Returns a value in [-1, 1].
#[inline]
fn fbm(noise: &Perlin, x: f64, y: f64, octaves: usize, lacunarity: f64, gain: f64) -> f64 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;
    for _ in 0..octaves {
        value += noise.get([x * frequency, y * frequency]) * amplitude;
        max_value += amplitude;
        amplitude *= gain;
        frequency *= lacunarity;
    }
    if max_value > 0.0 {
        value / max_value
    } else {
        0.0
    }
}

/// Ridged multifractal noise (sharp mountain ridges). Returns a value in [0, 1].
#[inline]
fn ridged(noise: &Perlin, x: f64, y: f64, octaves: usize, lacunarity: f64, gain: f64) -> f64 {
    let mut value = 0.0;
    let mut amplitude = 0.5;
    let mut frequency = 1.0;
    let mut max_value = 0.0;
    for _ in 0..octaves {
        let n = 1.0 - noise.get([x * frequency, y * frequency]).abs();
        let n = n * n; // sharpen the ridges
        value += n * amplitude;
        max_value += amplitude;
        amplitude *= gain;
        frequency *= lacunarity;
    }
    if max_value > 0.0 {
        value / max_value
    } else {
        0.0
    }
}

/// Hermite smoothstep in [0, 1].
#[inline]
fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    if (edge1 - edge0).abs() < f64::EPSILON {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Whittaker biome classification from temperature (cold→hot) and moisture (dry→wet),
/// mapped onto the engine's 11 `BiomeType` variants. Used for land cells that are not
/// ocean / beach / river / high mountain (those are handled separately by elevation).
#[inline]
fn whittaker_biome(temperature: f32, moisture: f32) -> BiomeType {
    let heat = ((temperature * 6.0) as usize).min(5); // 0=coldest .. 5=hottest
    let wet = ((moisture * 4.0) as usize).min(3); // 0=dryest .. 3=wettest
    match (wet, heat) {
        // Coldest column → ice, colder column → tundra (mapped to boreal forest).
        (_, 0) => BiomeType::Snow,
        (_, 1) => BiomeType::BorealForest,
        // Dry rows.
        (0, 2) => BiomeType::Grassland,
        (0, _) => BiomeType::Desert,
        (1, 2) | (1, 3) => BiomeType::TemperateForest, // woodland
        (1, _) => BiomeType::Grassland,                // savanna
        // Wet / wetter rows.
        (_, 2) => BiomeType::BorealForest,
        (_, 3) => BiomeType::TemperateForest, // seasonal forest
        (_, _) => BiomeType::Rainforest,
    }
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
        let warp_perlin = Perlin::new(settings.seed ^ 0x9E37_79B9);
        let mountain_perlin = Perlin::new(settings.seed.wrapping_add(200));
        let temperature_perlin = Perlin::new(settings.seed.wrapping_add(300));

        // Parallel generation for raw elevation: domain-warped continental fBm with
        // ridged-multifractal mountains blended in at altitude, shaped by an island falloff.
        let mut raw_elevations: Vec<f32> = (0..width * height)
            .into_par_iter()
            .map(|idx| {
                let col = idx % width;
                let row = idx / width;

                let x = (col as f64) / (width as f64) * config.noise_scale;
                let y = (row as f64) / (height as f64) * config.noise_scale;

                // Domain warp: bend sample coordinates to break grid symmetry and
                // produce meandering coastlines / valleys.
                let wx = warp_perlin.get([x * 0.5 + 5.2, y * 0.5 + 1.3]);
                let wy = warp_perlin.get([x * 0.5 + 9.7, y * 0.5 + 4.1]);
                let warp = 0.6;
                let sx = x + wx * warp;
                let sy = y + wy * warp;

                // Continental base in [0, 1].
                let base = (fbm(
                    &elevation_perlin,
                    sx,
                    sy,
                    settings.octaves,
                    settings.lacunarity,
                    settings.gain,
                ) as f32
                    + 1.0)
                    / 2.0;

                // Ridged mountains, only emerging over the continental interior.
                let ridge = ridged(
                    &mountain_perlin,
                    sx,
                    sy,
                    settings.octaves,
                    settings.lacunarity,
                    settings.gain,
                ) as f32;
                let mountain_mask = smoothstep(0.45, 0.80, base as f64) as f32;
                let mut elevation_val = base * 0.85 + ridge * mountain_mask * 0.5;

                // Gentle curve emphasising lowlands over peaks.
                elevation_val = elevation_val.max(0.0).powf(1.15);

                // Island falloff (finite landmass).
                let dx = (col as f32 / width as f32) * 2.0 - 1.0;
                let dy = (row as f32 / height as f32) * 2.0 - 1.0;
                let dist = (dx * dx + dy * dy).sqrt().min(1.0);
                let falloff = (1.0 - dist * dist).clamp(0.0, 1.0);
                elevation_val * falloff
            })
            .collect();

        // Normalize elevations to [0.0, 1.0]
        let (min_el, max_el) = raw_elevations.iter().fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(min_val, max_val), &val| (min_val.min(val), max_val.max(val)),
        );
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

                let h = h00 * (1.0 - tx) * (1.0 - ty)
                    + h10 * tx * (1.0 - ty)
                    + h01 * (1.0 - tx) * ty
                    + h11 * tx * ty;
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

                if new_px < 0.0
                    || new_px >= (width - 1) as f32
                    || new_py < 0.0
                    || new_py >= (height - 1) as f32
                {
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
                let new_h = nh00 * (1.0 - new_tx) * (1.0 - new_ty)
                    + nh10 * new_tx * (1.0 - new_ty)
                    + nh01 * (1.0 - new_tx) * new_ty
                    + nh11 * new_tx * new_ty;

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
        let (min_el, max_el) = elevations.iter().fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(min_val, max_val), &val| (min_val.min(val), max_val.max(val)),
        );
        let el_range = max_el - min_el;
        if el_range > 0.0 {
            elevations.par_iter_mut().for_each(|el| {
                *el = (*el - min_el) / el_range;
            });
        }

        let max_flow = flows.iter().fold(0.0f32, |max_val, &val| max_val.max(val));
        let norm_max_flow = if max_flow == 0.0 { 1.0 } else { max_flow };

        // Temperature field: hot at the equator (map centre along Z), cold at the poles,
        // cooled further by altitude (lapse rate), with mild local noise.
        // Sequential (not rayon): keeps the per-generate allocation count independent of
        // erosion so the zero-alloc hot-path assertion stays stable; these passes are cheap.
        let temperatures: Vec<f32> = (0..width * height)
            .map(|idx| {
                let col = idx % width;
                let row = idx / width;

                let lat = row as f32 / (height as f32 - 1.0).max(1.0); // 0=north .. 1=south
                let lat_temp = 1.0 - (2.0 * (lat - 0.5)).abs(); // 1 at equator, 0 at poles

                let altitude_above_sea = (elevations[idx] - sea_val).max(0.0);
                let lapse = altitude_above_sea * 1.1;

                let nx = (col as f64) / (width as f64) * config.noise_scale;
                let ny = (row as f64) / (height as f64) * config.noise_scale;
                let local = (fbm(&temperature_perlin, nx, ny, 3, 2.0, 0.5) as f32) * 0.08;

                (lat_temp - lapse + local).clamp(0.0, 1.0)
            })
            .collect();

        // Moisture field: domain-warped fBm boosted by river flow + sea proximity, and
        // dried by rain shadow behind upwind mountains (prevailing wind from the west).
        let moistures: Vec<f32> = (0..width * height)
            .map(|idx| {
                let col = idx % width;
                let row = idx / width;

                let x = (col as f64) / (width as f64) * config.noise_scale;
                let y = (row as f64) / (height as f64) * config.noise_scale;

                let base_moisture = (fbm(
                    &moisture_perlin,
                    x,
                    y,
                    settings.octaves,
                    settings.lacunarity,
                    settings.gain,
                ) as f32
                    + 1.0)
                    / 2.0;

                let elevation = elevations[idx];
                let flow_effect = (flows[idx] / norm_max_flow) * 0.4;

                // Wetter near / below the sea.
                let sea_proximity =
                    smoothstep((sea_val + 0.18) as f64, sea_val as f64, elevation as f64) as f32
                        * 0.2;

                // Rain shadow: scan up to 8 cells upwind (west) for a higher barrier.
                let mut barrier = 0.0f32;
                for k in 1..=8usize {
                    if col >= k {
                        let e = elevations[row * width + (col - k)];
                        if e > barrier {
                            barrier = e;
                        }
                    }
                }
                let rain_shadow = (barrier - elevation - 0.1).max(0.0) * 1.1;

                (base_moisture + flow_effect + sea_proximity - rain_shadow).clamp(0.0, 1.0)
            })
            .collect();

        // Biome determination: ocean / beach / river / high mountain by elevation & flow,
        // otherwise a Whittaker classification from temperature x moisture.
        let biomes: Vec<u8> = (0..width * height)
            .map(|idx| {
                let elevation = elevations[idx];
                let moisture = moistures[idx];
                let temperature = temperatures[idx];
                let flow = flows[idx];

                let biome = if elevation < sea_val / 2.0 {
                    BiomeType::DeepOcean
                } else if elevation < sea_val {
                    BiomeType::Ocean
                } else if elevation < sand_val {
                    BiomeType::Beach
                } else if elevation < 0.85 && flow > (max_flow * 0.05) && flow > 2.0 {
                    BiomeType::River
                } else if elevation >= snow_val {
                    // Snow line: frozen peaks where cold or humid, bare rock otherwise.
                    if temperature < 0.35 || moisture > 0.5 {
                        BiomeType::Snow
                    } else {
                        BiomeType::MountainRock
                    }
                } else if elevation >= 0.78 {
                    BiomeType::MountainRock
                } else {
                    whittaker_biome(temperature, moisture)
                };

                biome as u8
            })
            .collect();

        // Select exactly 10 random land-bound POIs (elevation >= 0.35) using StdRng (seed: settings.seed as u64 + 100)
        let mut pois = Vec::with_capacity(10);
        let mut poi_rng = StdRng::seed_from_u64(settings.seed as u64 + 100);
        let mut land_count = 0;
        for (idx, &elevation) in elevations.iter().enumerate() {
            if elevation >= sand_val {
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
            temperatures,
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
        if !(0.0..=1.0).contains(&tx) || !(0.0..=1.0).contains(&tz) {
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

/// Bump when the generation algorithm changes so stale on-disk world caches are ignored.
pub const WORLD_CACHE_VERSION: u32 = 1;

/// Deterministic cache key from everything that affects generation (settings + tuning config +
/// algorithm version). Same inputs → same key → same cached world file.
fn terrain_cache_key(settings: &MapSettings, config: &MapConfig) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    WORLD_CACHE_VERSION.hash(&mut h);
    settings.seed.hash(&mut h);
    settings.width.hash(&mut h);
    settings.height.hash(&mut h);
    settings.octaves.hash(&mut h);
    settings.lacunarity.to_bits().hash(&mut h);
    settings.gain.to_bits().hash(&mut h);
    settings.erosion_steps.hash(&mut h);
    config.sea_ratio.to_bits().hash(&mut h);
    config.sand_ratio.to_bits().hash(&mut h);
    config.snow_ratio.to_bits().hash(&mut h);
    config.noise_scale.to_bits().hash(&mut h);
    h.finish()
}

impl TerrainMap {
    /// Generate-once + cache: the authoritative agent world is expensive to reproduce and must be
    /// identical across runs, so we serialize it (compact binary via bincode) to `cache_dir` keyed
    /// by [`terrain_cache_key`]. On a cache hit we deserialize straight from disk and skip
    /// generation entirely; on a miss (or any corrupt/mismatched file) we generate and best-effort
    /// write the cache. Generation is already deterministic (seeded RNG), so the cache is a speed
    /// optimization layered on top of that guarantee — never a correctness dependency.
    pub fn load_or_generate(settings: &MapSettings, cache_dir: &Path) -> Self {
        let config = get_map_config();
        let key = terrain_cache_key(settings, config);
        let path = cache_dir.join(format!("world_{:016x}.bin", key));

        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok((map, _)) = bincode::serde::decode_from_slice::<TerrainMap, _>(
                &bytes,
                bincode::config::standard(),
            ) {
                // Guard against a truncated/mismatched cache slipping through.
                if map.width == settings.width
                    && map.height == settings.height
                    && map.elevations.len() == settings.width * settings.height
                    && map.biomes.len() == map.elevations.len()
                {
                    return map;
                }
            }
        }

        let map = Self::generate(settings);
        if let Ok(bytes) = bincode::serde::encode_to_vec(&map, bincode::config::standard()) {
            let _ = std::fs::create_dir_all(cache_dir);
            let _ = std::fs::write(&path, bytes);
        }
        map
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
            assert!((0.0..=1.0).contains(&elevation));
        }

        // Assert min is close to 0 and max is close to 1
        let min_el = map.elevations.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_el = map
            .elevations
            .iter()
            .fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        assert!(min_el < 0.05);
        assert!(max_el > 0.95);
    }

    #[test]
    fn test_erosion_alters_topography() {
        let settings_no_erosion = MapSettings {
            erosion_steps: 0,
            ..Default::default()
        };
        let map_no_erosion = TerrainMap::generate(&settings_no_erosion);

        let settings_with_erosion = MapSettings {
            erosion_steps: 20000,
            ..Default::default()
        };
        let map_with_erosion = TerrainMap::generate(&settings_with_erosion);

        assert_eq!(
            map_no_erosion.elevations.len(),
            map_with_erosion.elevations.len()
        );

        let mut diff_count = 0;
        for i in 0..map_no_erosion.elevations.len() {
            if (map_no_erosion.elevations[i] - map_with_erosion.elevations[i]).abs() > 1e-5 {
                diff_count += 1;
            }
        }

        // Erosion should have altered the topography significantly
        assert!(
            diff_count > 0,
            "Erosion must alter at least some raw height values"
        );
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
                0 => [15, 25, 65],     // DeepOcean
                1 => [35, 80, 145],    // Ocean
                2 => [235, 215, 165],  // Beach
                3 => [55, 135, 215],   // River
                4 => [140, 190, 100],  // Grassland
                5 => [65, 135, 75],    // TemperateForest
                6 => [45, 95, 80],     // BorealForest
                7 => [15, 85, 45],     // Rainforest
                8 => [215, 175, 105],  // Desert
                9 => [120, 125, 130],  // MountainRock
                10 => [245, 248, 250], // Snow
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
                if (0..1024).contains(&cx) {
                    img_buf.put_pixel(cx as u32, py as u32, image::Rgb([0, 255, 0]));
                }
            }
            // Vertical line
            for dy in -2..=2 {
                let cy = py as isize + dy;
                if (0..1024).contains(&cy) {
                    img_buf.put_pixel(px as u32, cy as u32, image::Rgb([0, 255, 0]));
                }
            }
        }

        let output_path = Path::new(r"e:\Project\Anima-Engine\anima_world_refined.png");
        img_buf
            .save(output_path)
            .expect("Failed to save sample map image");
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
            assert!(
                map.elevations[idx] >= config.sand_ratio as f32,
                "POI at ({}, {}) has elevation {}, which is below {}",
                col,
                row,
                map.elevations[idx],
                config.sand_ratio
            );
        }
    }

    fn tiny_settings() -> MapSettings {
        MapSettings {
            width: 24,
            height: 24,
            seed: 7,
            octaves: 3,
            lacunarity: 2.0,
            gain: 0.5,
            erosion_steps: 200,
        }
    }

    #[test]
    fn generate_is_deterministic() {
        // The whole generate-once + cache guarantee rests on this: same settings → identical world.
        let s = tiny_settings();
        let a = TerrainMap::generate(&s);
        let b = TerrainMap::generate(&s);
        assert_eq!(a.elevations, b.elevations);
        assert_eq!(a.biomes, b.biomes);
        assert_eq!(a.moistures, b.moistures);
        assert_eq!(a.temperatures, b.temperatures);
        assert_eq!(a.flows, b.flows);
        assert_eq!(a.pois, b.pois);
    }

    #[test]
    fn terrain_bincode_roundtrip_is_exact() {
        let m = TerrainMap::generate(&tiny_settings());
        let bytes = bincode::serde::encode_to_vec(&m, bincode::config::standard()).expect("encode");
        let (back, _): (TerrainMap, usize) =
            bincode::serde::decode_from_slice(&bytes, bincode::config::standard()).expect("decode");
        assert_eq!(back.width, m.width);
        assert_eq!(back.height, m.height);
        assert_eq!(back.elevations, m.elevations);
        assert_eq!(back.biomes, m.biomes);
        assert_eq!(back.moistures, m.moistures);
        assert_eq!(back.temperatures, m.temperatures);
        assert_eq!(back.flows, m.flows);
        assert_eq!(back.pois, m.pois);
    }

    #[test]
    fn load_or_generate_writes_cache_then_reloads_identically() {
        let dir = std::env::temp_dir().join("anima_test_cache_lorg");
        let _ = std::fs::remove_dir_all(&dir);
        let s = tiny_settings();

        // First call: cache miss → generate + write a cache file.
        let a = TerrainMap::load_or_generate(&s, &dir);
        let wrote = std::fs::read_dir(&dir)
            .expect("cache dir created")
            .filter_map(|e| e.ok())
            .any(|f| f.file_name().to_string_lossy().starts_with("world_"));
        assert!(
            wrote,
            "load_or_generate should write a world_*.bin cache file"
        );

        // Second call: cache hit → deserialized straight from disk, byte-identical world.
        let b = TerrainMap::load_or_generate(&s, &dir);
        assert_eq!(a.elevations, b.elevations);
        assert_eq!(a.biomes, b.biomes);
        assert_eq!(a.pois, b.pois);

        // And it equals a fresh generation (cache is transparent, not lossy).
        let fresh = TerrainMap::generate(&s);
        assert_eq!(a.elevations, fresh.elevations);
        assert_eq!(a.biomes, fresh.biomes);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
