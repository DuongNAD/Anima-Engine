use bevy_ecs::prelude::*;
use glam::Vec3;
use crate::core::ecs::{Position, Rotation, MapBounds};

pub const GRID_SIZE: usize = 128;
pub const CELL_COUNT: usize = GRID_SIZE * GRID_SIZE; // 16384
pub const MAX_CONCENTRATION: f32 = 10.0;

#[derive(Resource, Debug, Clone)]
pub struct PheromoneGrid {
    pub values: Vec<f32>,
    pub scratch: Vec<f32>,
    pub diffusion_rate: f32,
    pub decay_rate: f32,
}

impl Default for PheromoneGrid {
    fn default() -> Self {
        Self {
            values: vec![0.0; CELL_COUNT],
            scratch: vec![0.0; CELL_COUNT],
            diffusion_rate: 0.1,
            decay_rate: 0.05,
        }
    }
}

impl PheromoneGrid {
    pub fn new(diffusion_rate: f32, decay_rate: f32) -> Self {
        Self {
            values: vec![0.0; CELL_COUNT],
            scratch: vec![0.0; CELL_COUNT],
            diffusion_rate,
            decay_rate,
        }
    }

    /// Maps a 3D physical coordinate (X, Z plane) to a flat 1D grid index, supporting toroidal wrapping.
    #[inline]
    pub fn pos_to_index(&self, pos: Vec3, bounds: &MapBounds) -> Option<usize> {
        if !pos.x.is_finite() || !pos.y.is_finite() || !pos.z.is_finite() {
            return None;
        }

        let x_min = bounds.min.x;
        let x_max = bounds.max.x;
        let z_min = bounds.min.z;
        let z_max = bounds.max.z;

        if !x_min.is_finite() || !x_max.is_finite() || !z_min.is_finite() || !z_max.is_finite() {
            return None;
        }

        let x_range = x_max - x_min;
        let z_range = z_max - z_min;

        if x_range <= 0.0 || z_range <= 0.0 {
            return None;
        }

        // Toroidal coordinate wrapping
        let wrapped_x = x_min + (pos.x - x_min).rem_euclid(x_range);
        let wrapped_z = z_min + (pos.z - z_min).rem_euclid(z_range);

        let norm_x = (wrapped_x - x_min) / x_range;
        let norm_z = (wrapped_z - z_min) / z_range;

        let cx = ((norm_x * GRID_SIZE as f32).floor() as usize).min(GRID_SIZE - 1);
        let cz = ((norm_z * GRID_SIZE as f32).floor() as usize).min(GRID_SIZE - 1);

        Some(cz * GRID_SIZE + cx)
    }

    /// Samples the grid using bilinear interpolation with toroidal boundary wrapping.
    pub fn sample_bilinear(&self, pos: Vec3, bounds: &MapBounds) -> f32 {
        if !pos.x.is_finite() || !pos.y.is_finite() || !pos.z.is_finite() {
            return 0.0;
        }

        let x_min = bounds.min.x;
        let x_max = bounds.max.x;
        let z_min = bounds.min.z;
        let z_max = bounds.max.z;

        if !x_min.is_finite() || !x_max.is_finite() || !z_min.is_finite() || !z_max.is_finite() {
            return 0.0;
        }

        let x_range = x_max - x_min;
        let z_range = z_max - z_min;

        if x_range <= 0.0 || z_range <= 0.0 {
            return 0.0;
        }

        let wrapped_x = x_min + (pos.x - x_min).rem_euclid(x_range);
        let wrapped_z = z_min + (pos.z - z_min).rem_euclid(z_range);

        let norm_x = (wrapped_x - x_min) / x_range;
        let norm_z = (wrapped_z - z_min) / z_range;

        let gx = (norm_x * GRID_SIZE as f32).rem_euclid(GRID_SIZE as f32);
        let gz = (norm_z * GRID_SIZE as f32).rem_euclid(GRID_SIZE as f32);

        let gx0 = (gx.floor() as usize) % GRID_SIZE;
        let gz0 = (gz.floor() as usize) % GRID_SIZE;

        // Toroidal neighbors
        let gx1 = (gx0 + 1) % GRID_SIZE;
        let gz1 = (gz0 + 1) % GRID_SIZE;

        let tx = (gx - gx.floor()).clamp(0.0, 1.0);
        let tz = (gz - gz.floor()).clamp(0.0, 1.0);

        let c00 = self.values[gz0 * GRID_SIZE + gx0];
        let c10 = self.values[gz0 * GRID_SIZE + gx1];
        let c01 = self.values[gz1 * GRID_SIZE + gx0];
        let c11 = self.values[gz1 * GRID_SIZE + gx1];

        // Bilinear interpolation
        let top = c00 * (1.0 - tx) + c10 * tx;
        let bottom = c01 * (1.0 - tx) + c11 * tx;

        top * (1.0 - tz) + bottom * tz
    }
}

/// Component representing an agent's left and right olfactory sensors.
#[derive(Component, Debug, Clone)]
pub struct OlfactorySensors {
    pub left_offset: Vec3,
    pub right_offset: Vec3,
    pub left_reading: f32,
    pub right_reading: f32,
}

impl OlfactorySensors {
    pub fn new(left_offset: Vec3, right_offset: Vec3) -> Self {
        Self {
            left_offset,
            right_offset,
            left_reading: 0.0,
            right_reading: 0.0,
        }
    }
}

/// Component to allow agents to write/release pheromones to the environment.
#[derive(Component, Debug, Clone)]
pub struct PheromoneReleaser {
    pub strength: f32, // Amount released per second
}

impl PheromoneReleaser {
    pub fn new(strength: f32) -> Self {
        Self { strength }
    }
}

/// System to release pheromone from agents to the grid.
pub fn agent_release_pheromone_system(
    mut grid: ResMut<PheromoneGrid>,
    query: Query<(&Position, &PheromoneReleaser)>,
    bounds: Res<MapBounds>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    let dt = time_step.0;
    for (pos, releaser) in query.iter() {
        if let Some(idx) = grid.pos_to_index(pos.0, &bounds) {
            grid.values[idx] = (grid.values[idx] + releaser.strength * dt).min(MAX_CONCENTRATION);
        }
    }
}

/// System to simulate diffusion and decay of pheromones in the grid.
pub fn update_pheromone_grid_system(
    mut grid: ResMut<PheromoneGrid>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    let dt = time_step.0.max(0.0);
    // Explicit diffusion rate stability limit: diff_rate * dt < 0.25
    let d_dt = (grid.diffusion_rate * dt).min(0.24);
    let decay_factor = (1.0 - grid.decay_rate * dt).max(0.0);

    for z in 0..GRID_SIZE {
        let z_up = (z + GRID_SIZE - 1) % GRID_SIZE;
        let z_down = (z + 1) % GRID_SIZE;
        let z_offset = z * GRID_SIZE;
        let z_up_offset = z_up * GRID_SIZE;
        let z_down_offset = z_down * GRID_SIZE;

        for x in 0..GRID_SIZE {
            let x_left = (x + GRID_SIZE - 1) % GRID_SIZE;
            let x_right = (x + 1) % GRID_SIZE;

            let idx = z_offset + x;
            let center = grid.values[idx];
            let left = grid.values[z_offset + x_left];
            let right = grid.values[z_offset + x_right];
            let up = grid.values[z_up_offset + x];
            let down = grid.values[z_down_offset + x];

            let laplacian = left + right + up + down - 4.0 * center;
            grid.scratch[idx] = (center + d_dt * laplacian) * decay_factor;
        }
    }

    // Swap the value buffer and scratch buffer in-place in O(1) time
    let grid_ref = &mut *grid;
    std::mem::swap(&mut grid_ref.values, &mut grid_ref.scratch);
}

/// System to update the agent's olfactory readings.
pub fn agent_read_pheromone_system(
    grid: Res<PheromoneGrid>,
    mut query: Query<(&Position, &Rotation, &mut OlfactorySensors)>,
    bounds: Res<MapBounds>,
) {
    for (pos, rot, mut sensors) in query.iter_mut() {
        let world_left = pos.0 + (rot.0 * sensors.left_offset);
        let world_right = pos.0 + (rot.0 * sensors.right_offset);

        sensors.left_reading = grid.sample_bilinear(world_left, &bounds);
        sensors.right_reading = grid.sample_bilinear(world_right, &bounds);
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct PheromoneGridState {
    pub grid: Vec<f32>,
    pub width: u32,
    pub height: u32,
}
