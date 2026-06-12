use bevy_ecs::prelude::*;
use glam::Vec3;
use std::collections::HashMap;
use crate::core::ecs::{MapBounds, Position};

#[derive(Component, Debug, Clone, Copy)]
pub struct SpatialCollider {
    pub radius: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Ray3D {
    pub origin: Vec3,
    pub direction: Vec3,
}

#[derive(Debug, Clone, Copy)]
pub struct RaycastHit {
    pub entity: Entity,
    pub position: Vec3,
    pub distance: f32,
}

#[derive(Resource)]
pub struct SpatialHashGrid {
    pub cell_size: f32,
    pub cells: HashMap<(i32, i32), Vec<Entity>>,
    pub counts: HashMap<(i32, i32), usize>,
}

impl SpatialHashGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            cells: HashMap::new(),
            counts: HashMap::new(),
        }
    }

    pub fn new_prepopulated(cell_size: f32, bounds: &MapBounds) -> Self {
        let cell_size = if cell_size <= 0.0 || !cell_size.is_finite() {
            10.0
        } else {
            cell_size
        };

        let bounds_valid = bounds.min.is_finite() && bounds.max.is_finite() && bounds.max.x >= bounds.min.x && bounds.max.z >= bounds.min.z;

        if !bounds_valid {
            return Self {
                cell_size,
                cells: HashMap::new(),
                counts: HashMap::new(),
            };
        }

        let cx_start = (bounds.min.x / cell_size).floor() as i32;
        let cx_end = (bounds.max.x / cell_size).floor() as i32;
        let cy_start = (bounds.min.z / cell_size).floor() as i32;
        let cy_end = (bounds.max.z / cell_size).floor() as i32;

        let dx = (cx_end as i64).saturating_sub(cx_start as i64);
        let dy = (cy_end as i64).saturating_sub(cy_start as i64);

        if dx < 0 || dy < 0 || dx > 2000 || dy > 2000 || dx * dy > 200000 {
            return Self {
                cell_size,
                cells: HashMap::new(),
                counts: HashMap::new(),
            };
        }

        let mut cells = HashMap::new();
        let mut counts = HashMap::new();
        for cx in cx_start..=cx_end {
            for cy in cy_start..=cy_end {
                cells.insert((cx, cy), Vec::with_capacity(32));
                counts.insert((cx, cy), 0);
            }
        }

        Self {
            cell_size,
            cells,
            counts,
        }
    }

    pub fn clear(&mut self) {
        for vec in self.cells.values_mut() {
            vec.clear();
        }
    }

    pub fn insert(&mut self, position: Vec3, bounds: &MapBounds, entity: Entity) {
        let x_min = bounds.min.x;
        let x_max = bounds.max.x;
        let x_range = x_max - x_min;

        let z_min = bounds.min.z;
        let z_max = bounds.max.z;
        let z_range = z_max - z_min;

        let mut wrapped_x = position.x;
        if x_range > 0.0 {
            wrapped_x = x_min + (position.x - x_min).rem_euclid(x_range);
            wrapped_x = wrapped_x.clamp(x_min, x_max - 1e-4);
        }

        let mut wrapped_z = position.z;
        if z_range > 0.0 {
            wrapped_z = z_min + (position.z - z_min).rem_euclid(z_range);
            wrapped_z = wrapped_z.clamp(z_min, z_max - 1e-4);
        }

        let cx_start = (bounds.min.x / self.cell_size).floor() as i32;
        let cx_end = (bounds.max.x / self.cell_size).floor() as i32;
        let cy_start = (bounds.min.z / self.cell_size).floor() as i32;
        let cy_end = (bounds.max.z / self.cell_size).floor() as i32;

        let cx = ((wrapped_x / self.cell_size).floor() as i32).clamp(cx_start, cx_end);
        let cy = ((wrapped_z / self.cell_size).floor() as i32).clamp(cy_start, cy_end);

        if let Some(cell) = self.cells.get_mut(&(cx, cy)) {
            cell.push(entity);
        } else {
            // Fallback for safety
            self.cells.insert((cx, cy), vec![entity]);
        }
    }

    pub fn raycast(
        &self,
        ray: &Ray3D,
        max_distance: f32,
        bounds: &MapBounds,
        collider_query: &Query<(&Position, &SpatialCollider)>,
    ) -> Option<RaycastHit> {
        let dir = ray.direction.normalize_or_zero();
        if dir == Vec3::ZERO {
            return None;
        }

        let x_min = bounds.min.x;
        let x_max = bounds.max.x;
        let x_range = x_max - x_min;

        let z_min = bounds.min.z;
        let z_max = bounds.max.z;
        let z_range = z_max - z_min;

        let mut ox = ray.origin.x;
        let mut oz = ray.origin.z;

        if x_range > 0.0 {
            ox = x_min + (ox - x_min).rem_euclid(x_range);
        }
        if z_range > 0.0 {
            oz = z_min + (oz - z_min).rem_euclid(z_range);
        }

        let cx_start = (bounds.min.x / self.cell_size).floor() as i32;
        let cx_max = if x_range > 0.0 {
            ((bounds.min.x + x_range - 1e-4) / self.cell_size).floor() as i32
        } else {
            cx_start
        };
        let cx_range = (cx_max - cx_start + 1).max(1);

        let cy_start = (bounds.min.z / self.cell_size).floor() as i32;
        let cy_max = if z_range > 0.0 {
            ((bounds.min.z + z_range - 1e-4) / self.cell_size).floor() as i32
        } else {
            cy_start
        };
        let cy_range = (cy_max - cy_start + 1).max(1);

        let mut cx = (ox / self.cell_size).floor() as i32;
        let mut cy = (oz / self.cell_size).floor() as i32;

        cx = cx_start + (cx - cx_start).rem_euclid(cx_range);
        cy = cy_start + (cy - cy_start).rem_euclid(cy_range);

        let step_x = if dir.x > 0.0 { 1 } else if dir.x < 0.0 { -1 } else { 0 };
        let step_z = if dir.z > 0.0 { 1 } else if dir.z < 0.0 { -1 } else { 0 };

        let t_delta_x = if dir.x.abs() > 1e-6 {
            self.cell_size / dir.x.abs()
        } else {
            f32::INFINITY
        };

        let t_delta_z = if dir.z.abs() > 1e-6 {
            self.cell_size / dir.z.abs()
        } else {
            f32::INFINITY
        };

        let mut t_max_x = if dir.x.abs() > 1e-6 {
            let next_grid_x = if dir.x > 0.0 {
                (cx + 1) as f32 * self.cell_size
            } else {
                cx as f32 * self.self_cell_boundary_check_x()
            };
            (next_grid_x - ox) / dir.x
        } else {
            f32::INFINITY
        };

        let mut t_max_z = if dir.z.abs() > 1e-6 {
            let next_grid_z = if dir.z > 0.0 {
                (cy + 1) as f32 * self.cell_size
            } else {
                cy as f32 * self.self_cell_boundary_check_z()
            };
            (next_grid_z - oz) / dir.z
        } else {
            f32::INFINITY
        };

        let mut t_curr = 0.0;
        let mut step_count = 0;
        let max_steps = 100;
        let mut closest_hit: Option<RaycastHit> = None;

        while t_curr <= max_distance && step_count < max_steps {
            if let Some(entities) = self.cells.get(&(cx, cy)) {
                for &entity in entities {
                    if let Ok((pos, collider)) = collider_query.get(entity) {
                        let ray_pos = ray.origin + t_curr * dir;
                        let mut diff = pos.0 - ray_pos;
                        if x_range > 0.0 {
                            diff.x = diff.x - x_range * (diff.x / x_range).round();
                        }
                        if z_range > 0.0 {
                            diff.z = diff.z - z_range * (diff.z / z_range).round();
                        }
                        let c_virtual = ray_pos + diff;

                        if let Some(t_hit) = intersect_sphere(ray.origin, dir, c_virtual, collider.radius) {
                            if t_hit >= 0.0 && t_hit <= max_distance {
                                let mut hit_pos = ray.origin + t_hit * dir;
                                if x_range > 0.0 {
                                    hit_pos.x = x_min + (hit_pos.x - x_min).rem_euclid(x_range);
                                }
                                if z_range > 0.0 {
                                    hit_pos.z = z_min + (hit_pos.z - z_min).rem_euclid(z_range);
                                }
                                let hit = RaycastHit {
                                    entity,
                                    position: hit_pos,
                                    distance: t_hit,
                                };

                                if let Some(ref current_closest) = closest_hit {
                                    if t_hit < current_closest.distance {
                                        closest_hit = Some(hit);
                                    }
                                } else {
                                    closest_hit = Some(hit);
                                }
                            }
                        }
                    }
                }
            }

            let t_next = t_max_x.min(t_max_z);
            if let Some(hit) = closest_hit {
                if t_next > hit.distance {
                    break;
                }
            }

            if t_max_x < t_max_z {
                t_curr = t_max_x;
                cx += step_x;
                t_max_x += t_delta_x;
                cx = cx_start + (cx - cx_start).rem_euclid(cx_range);
            } else {
                t_curr = t_max_z;
                cy += step_z;
                t_max_z += t_delta_z;
                cy = cy_start + (cy - cy_start).rem_euclid(cy_range);
            }

            step_count += 1;
        }

        closest_hit
    }

    #[inline]
    fn self_cell_boundary_check_x(&self) -> f32 {
        self.cell_size
    }

    #[inline]
    fn self_cell_boundary_check_z(&self) -> f32 {
        self.cell_size
    }
}

fn intersect_sphere(
    ray_origin: Vec3,
    ray_direction: Vec3,
    sphere_center: Vec3,
    sphere_radius: f32,
) -> Option<f32> {
    let v = ray_origin - sphere_center;
    let a = ray_direction.dot(ray_direction);
    let b = 2.0 * v.dot(ray_direction);
    let c = v.dot(v) - sphere_radius * sphere_radius;
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return None;
    }
    let disc_sqrt = disc.sqrt();
    let t1 = (-b - disc_sqrt) / (2.0 * a);

    if t1 >= 0.0 {
        Some(t1)
    } else {
        None
    }
}

pub fn rebuild_spatial_grid_system(
    mut grid: ResMut<SpatialHashGrid>,
    bounds: Res<MapBounds>,
    query: Query<(Entity, &Position, &SpatialCollider)>,
) {
    let grid_ref = &mut *grid;
    
    // Reset our prepopulated counts
    for val in grid_ref.counts.values_mut() {
        *val = 0;
    }

    // Pass 1: Count entities per cell to pre-allocate/reserve capacity
    for (_entity, pos, _collider) in query.iter() {
        let x_min = bounds.min.x;
        let x_max = bounds.max.x;
        let x_range = x_max - x_min;

        let z_min = bounds.min.z;
        let z_max = bounds.max.z;
        let z_range = z_max - z_min;

        let mut wrapped_x = pos.0.x;
        if x_range > 0.0 {
            wrapped_x = x_min + (pos.0.x - x_min).rem_euclid(x_range);
            wrapped_x = wrapped_x.clamp(x_min, x_max - 1e-4);
        }

        let mut wrapped_z = pos.0.z;
        if z_range > 0.0 {
            wrapped_z = z_min + (pos.0.z - z_min).rem_euclid(z_range);
            wrapped_z = wrapped_z.clamp(z_min, z_max - 1e-4);
        }

        let cx_start = (bounds.min.x / grid_ref.cell_size).floor() as i32;
        let cx_end = (bounds.max.x / grid_ref.cell_size).floor() as i32;
        let cy_start = (bounds.min.z / grid_ref.cell_size).floor() as i32;
        let cy_end = (bounds.max.z / grid_ref.cell_size).floor() as i32;

        let cx = ((wrapped_x / grid_ref.cell_size).floor() as i32).clamp(cx_start, cx_end);
        let cy = ((wrapped_z / grid_ref.cell_size).floor() as i32).clamp(cy_start, cy_end);

        if let Some(c) = grid_ref.counts.get_mut(&(cx, cy)) {
            *c += 1;
        }
    }

    // Reserve necessary capacity for each cell vector to prevent reallocations
    for (key, cell) in grid_ref.cells.iter_mut() {
        let count = grid_ref.counts.get(key).copied().unwrap_or(0);
        if cell.capacity() < count {
            cell.reserve(count - cell.capacity());
        }
    }

    // Clear cells (preserving capacity)
    grid_ref.clear();

    // Pass 2: Insert actual entities
    for (entity, pos, _collider) in query.iter() {
        grid_ref.insert(pos.0, &bounds, entity);
    }
}
