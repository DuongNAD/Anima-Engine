use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

/// Represents the terrain substrate class for physical speed coupling and styling.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiomeType {
    Grassland,
    Mud,
    Sand,
    Rock,
    Water,
}

/// Represents the state of a single cell in the simulation grid.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct TerrainCell {
    pub elevation: f32,
    pub water_depth: f32,
    pub biome: BiomeType,
}

/// Bevy Resource holding the complete terrain map.
/// Includes `water_scratch` to allow zero-heap-allocation double-buffering.
#[derive(Resource, Debug, Clone)]
pub struct TerrainGrid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<TerrainCell>,
    pub water_scratch: Vec<f32>,
}

impl TerrainGrid {
    /// Initializer creating a grid populated with default cells.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![
                TerrainCell {
                    elevation: 0.0,
                    water_depth: 0.0,
                    biome: BiomeType::Grassland,
                };
                width * height
            ],
            water_scratch: vec![0.0; width * height],
        }
    }

    /// Maps a 3D physical coordinate (X, Z plane) to a 1D grid index.
    /// Supports toroidal wrapping matching `wrap_coordinates_system`.
    #[inline]
    pub fn pos_to_index(&self, pos: glam::Vec3, bounds: &crate::core::ecs::MapBounds) -> Option<usize> {
        if self.width == 0 || self.height == 0 {
            return None;
        }

        if !pos.x.is_finite() || !pos.y.is_finite() || !pos.z.is_finite() {
            return None;
        }

        let x_min = bounds.min.x;
        let x_max = bounds.max.x;
        let z_min = bounds.min.z;
        let z_max = bounds.max.z;

        let x_range = x_max - x_min;
        let z_range = z_max - z_min;

        if x_range <= 0.0 || z_range <= 0.0 {
            return None;
        }

        let wrapped_x = x_min + (pos.x - x_min).rem_euclid(x_range);
        let wrapped_z = z_min + (pos.z - z_min).rem_euclid(z_range);

        let norm_x = (wrapped_x - x_min) / x_range;
        let norm_z = (wrapped_z - z_min) / z_range;

        let cx = ((norm_x * self.width as f32).floor() as usize).min(self.width - 1);
        let cz = ((norm_z * self.height as f32).floor() as usize).min(self.height - 1);

        Some(cz * self.width + cx)
    }
}

/// Thread-safe double-buffered state structure for Tauri IPC serialization.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TerrainGridState {
    pub width: usize,
    pub height: usize,
    pub elevations: Vec<f32>,
    pub water_depths: Vec<f32>,
    pub biomes: Vec<BiomeType>,
}

/// Pseudo-random 2D coordinate hashing.
fn hash2d(x: f32, y: f32) -> f32 {
    let dot = x * 12.9898 + y * 78.233;
    let val = dot.sin() * 43_758.547;
    val - val.floor()
}

/// 2D Value Noise with smoothstep bilinear interpolation.
fn noise2d(x: f32, y: f32) -> f32 {
    let ix = x.floor();
    let iy = y.floor();
    let fx = x - ix;
    let fy = y - iy;

    // Smoothstep curve for natural interpolation
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);

    let a = hash2d(ix, iy);
    let b = hash2d(ix + 1.0, iy);
    let c = hash2d(ix, iy + 1.0);
    let d = hash2d(ix + 1.0, iy + 1.0);

    let lerp_ab = a + ux * (b - a);
    let lerp_cd = c + ux * (d - c);
    lerp_ab + uy * (lerp_cd - lerp_ab)
}

/// Fractional Brownian Motion (fBm) combining multiple noise octaves.
pub fn fbm2d(x: f32, y: f32, octaves: usize) -> f32 {
    let mut value = 0.0;
    let mut amplitude = 0.5;
    let mut frequency = 1.0;
    for _ in 0..octaves {
        value += amplitude * noise2d(x * frequency, y * frequency);
        frequency *= 2.0;
        amplitude *= 0.5;
    }
    value
}

/// Procedurally populates a `TerrainGrid` using fBm maps for elevation and moisture.
pub fn generate_terrain(width: usize, height: usize, seed: u32) -> TerrainGrid {
    let mut cells = Vec::with_capacity(width * height);
    let seed_offset = (seed % 1000) as f32 * 13.37;

    for z in 0..height {
        for x in 0..width {
            let nx = (x as f32) / 16.0 + seed_offset;
            let nz = (z as f32) / 16.0 + seed_offset * 1.5;

            // Compute separate maps for height and moisture
            let elevation = fbm2d(nx, nz, 4); // [0.0, 1.0]
            let moisture = fbm2d(nx + 100.0, nz + 100.0, 3); // [0.0, 1.0]

            // Biome classification mapping
            let biome = if elevation < 0.25 {
                BiomeType::Water
            } else if elevation < 0.35 {
                if moisture > 0.5 {
                    BiomeType::Mud
                } else {
                    BiomeType::Sand
                }
            } else if elevation < 0.75 {
                if moisture > 0.3 {
                    BiomeType::Grassland
                } else {
                    BiomeType::Sand
                }
            } else {
                BiomeType::Rock
            };

            // Water biomes get starting water depth
            let water_depth = if biome == BiomeType::Water {
                (0.25 - elevation).max(0.0) * 8.0
            } else {
                0.0
            };

            cells.push(TerrainCell {
                elevation,
                water_depth,
                biome,
            });
        }
    }

    TerrainGrid {
        width,
        height,
        cells,
        water_scratch: vec![0.0; width * height],
    }
}

/// Bevy system that updates water depth and biomes using gravity-driven gradient flows,
/// evaporation, and mass conservation. Utilizes a pre-allocated scratch buffer for zero-allocations.
pub fn water_dynamics_system(
    mut grid: ResMut<TerrainGrid>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    let dt = time_step.0;
    let width = grid.width;
    let height = grid.height;
    let flow_rate = 0.2; // Flow speed coefficient
    let evaporation_rate = 0.01; // Water loss per second

    // Clear the pre-allocated scratch buffer without heap allocations
    grid.water_scratch.fill(0.0);

    // 1. Calculate flows down elevation gradients
    for z in 0..height {
        for x in 0..width {
            let idx = z * width + x;
            let cell = grid.cells[idx];
            if cell.water_depth <= 1e-4 {
                continue;
            }

            let h_curr = cell.elevation + cell.water_depth;

            // 4-way cardinal neighbors with toroidal wrapping
            let neighbors = [
                ((x + width - 1) % width, z),   // West
                ((x + 1) % width, z),           // East
                (x, (z + height - 1) % height), // North
                (x, (z + 1) % height),          // South
            ];

            let mut total_diff = 0.0;
            let mut active_neighbors = [None; 4];
            let mut num_active = 0;

            for &n in &neighbors {
                let n_idx = n.1 * width + n.0;
                let n_cell = grid.cells[n_idx];
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
                        grid.water_scratch[idx] -= transfer;
                        grid.water_scratch[n_idx] += transfer;
                    }
                }
            }
        }
    }

    // 2. Apply net flows and evaporation back to cells
    for idx in 0..(width * height) {
        let net_flow = grid.water_scratch[idx];
        let cell = &mut grid.cells[idx];
        let mut new_water = (cell.water_depth + net_flow).max(0.0);

        if new_water > 0.0 {
            new_water = (new_water - evaporation_rate * dt).max(0.0);
        }

        cell.water_depth = new_water;

        // Dynamically adjust biome type if fully flooded / dried out
        if cell.water_depth > 0.1 {
            cell.biome = BiomeType::Water;
        } else if cell.water_depth > 0.0 {
            cell.biome = BiomeType::Mud;
        } else if cell.biome == BiomeType::Water || cell.biome == BiomeType::Mud {
            // Revert back to dry biome based on elevation
            cell.biome = if cell.elevation < 0.35 {
                BiomeType::Sand
            } else if cell.elevation < 0.75 {
                BiomeType::Grassland
            } else {
                BiomeType::Rock
            };
        }
    }
}
