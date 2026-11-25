//! Steering drive: the only writer of `InertiaComponent.target_velocity` (ADR-0006 option A,
//! F-042).
//!
//! Joint forces are internal and the integrator has no ground contact, so before this system a
//! creature oscillated in place. The one external force already in the physics is
//! `(target_velocity - velocity) * mass * 2.0` on the root (`physics/dynamics.rs`); this system
//! gives it something to chase.
//!
//! **Opt-in, inert by construction.** With no [`LocomotionPolicy`] resource, or `enabled ==
//! false`, the system returns before touching a component, so `target_velocity` stays zero and the
//! legacy trajectory is bit-identical to every run made before it existed.
//!
//! **Geometry is sensing's, not a new one.** `sensory_system` picks the nearest target by plain
//! Euclidean distance (no torus), over the same sets: a predator chases the nearest living prey,
//! anything else the nearest food, ties broken by query order. This system builds the same
//! snapshots in one pass each and scans them the same way, so an agent steers at exactly the
//! thing it was told about. `sensory_system`'s own buffers are `Local`s, filled only for agents
//! that think this tick and not at all when the inference pool is empty, so they cannot be read
//! from here; the scan cost is the same O(agents x targets) as sensing's.
//!
//! **Speed is heritable.** It is `class_max * clamp(mean child CPG amplitude / 1.5, 0, 1)`: the
//! oscillator amplitude is what `apply_inertia_to_oscillators` writes from the agent's CPG
//! parameters (shared model or evolved `BrainGenotype`), so a mutated genome or brain changes how
//! fast its body moves. `class_max` is the birth-impulse speed (2.0 predator / 1.5 prey).
//!
//! No RNG, no heap allocation once warm (the two snapshot buffers keep their capacity), and no
//! energy charge: activity cost already flows through `total_cost` in `metabolic_decay_system`
//! as `k_velocity * speed`.
//!
//! With [`crate::core::resources::GrassSteeringPolicy::enabled`] on (`live.grass_steering = 1.0`,
//! ported from card S11 `226d4b2`/`95044c4b` and adapted to LAND1), non-predator `Prey` resolve
//! targets with explicit priority order:
//! 1. **Thirst (> 50% deficit):** nearest non-empty `Lake` when `thirst_steering` is on.
//! 2. **Local `Food` items:** nearest `Food` within `(GRASS_SCAN_RADIUS + 0.5)` cell widths.
//! 3. **Pasture grass (`ResourceField`):** stay (`target_velocity = Vec3::ZERO`) if current cell's
//!    walkable grazeable grass is `>= GRASS_STAY_FRACTION * r_max`, else steer to the walkable cell
//!    with strictly highest grazeable grass above the current cell in the clipped `(2R + 1)^2`
//!    scan window (ties broken by lowest `(row, col)`), falling back to global nearest `Food`.

use bevy_ecs::prelude::*;
use glam::Vec3;

use crate::ai::cpg::CpgOscillator;
use crate::ai::hrrl::HomeostaticState;
use crate::core::components::{
    Agent, ChildrenLinks, Food, InertiaComponent, Lake, ParentLink, Position, Predator, Prey,
};
use crate::core::resources::LocomotionPolicy;

/// Fastest a predator can drive itself (the birth-impulse speed, `SpawnOffspringCommand`).
pub const PREDATOR_MAX_SPEED: f32 = 2.0;
/// Fastest a prey animal can drive itself.
pub const PREY_MAX_SPEED: f32 = 1.5;
/// `apply_inertia_to_oscillators` maps a CPG parameter in `[0, 1]` to `amplitude = raw * 1.5`.
pub const CPG_AMPLITUDE_SCALE: f32 = 1.5;
/// Within this distance of its target the agent stops pushing.
pub const ARRIVAL_RADIUS: f32 = 0.3;

/// With `LocomotionPolicy::thirst_steering` on, an agent whose `hydration` is below this fraction
/// of its `hydration_target` steers to the nearest non-empty lake instead of its food/prey target
/// (ADR-0007 option A). Above it the agent returns to its normal target.
pub const THIRST_STEERING_FRACTION: f32 = 0.5;

/// With `GrassSteeringPolicy::enabled` on (`live.grass_steering = 1.0`, card SEL2P2-PREP / S11
/// `226d4b2`), a non-thirsty prey agent without local `Food` stays (`target_velocity = Vec3::ZERO`)
/// when its current walkable cell holds grazeable grass `>= GRASS_STAY_FRACTION * r_max`.
pub const GRASS_STAY_FRACTION: f32 = 0.25;

/// Half-width in pasture cells of the square window (`(2 * R + 1) x (2 * R + 1)`, so `3` scans
/// a clipped `7 x 7` window) searched for richer walkable grass when a prey leaves a depleted
/// cell (card SEL2P2-PREP / S11 `226d4b2`).
pub const GRASS_SCAN_RADIUS: usize = 3;

struct GrassContext<'a> {
    field: &'a crate::core::ecology::ResourceField,
    reserve: crate::core::resources::PlantReservePolicy,
    nav: Option<(
        &'a crate::core::terrain::TerrainMap,
        &'a crate::core::resources::MapBounds,
    )>,
    local_food_radius_sq: f32,
}

impl GrassContext<'_> {
    #[inline]
    fn is_walkable(&self, pos: Vec3) -> bool {
        match self.nav {
            Some((tm, bounds)) => crate::core::terrain_navigation::is_pos_walkable(tm, bounds, pos),
            None => true,
        }
    }
}

#[inline]
fn find_nearest_point(pos: Vec3, candidates: &[Vec3]) -> Option<(Vec3, f32)> {
    let mut nearest: Option<Vec3> = None;
    let mut min_dist_sq = f32::MAX;
    for target in candidates.iter() {
        let d = pos.distance_squared(*target);
        if d < min_dist_sq {
            min_dist_sq = d;
            nearest = Some(*target);
        }
    }
    nearest.map(|t| (t, min_dist_sq))
}

fn select_prey_grass_target(
    pos: Vec3,
    grass: &GrassContext<'_>,
    walkable_food_cache: &[Vec3],
) -> Option<Vec3> {
    if !pos.is_finite() {
        return None;
    }
    let field = grass.field;
    let nearest_food = find_nearest_point(pos, walkable_food_cache);
    // Priority 2: local walkable Food item inside the local scan radius wins over pasture grass.
    if let Some((food_pos, food_d2)) = nearest_food {
        if food_d2 <= grass.local_food_radius_sq {
            return Some(food_pos);
        }
    }

    // Priority 3: pasture grass (`ResourceField` + S10 reserve floor + LAND1 walkability).
    if let Some(cur_idx) = field.cell_index(pos.x, pos.z) {
        let cell_w = (field.max_x - field.min_x) / field.width as f32;
        let cell_h = (field.max_z - field.min_z) / field.height as f32;
        let cur_row = cur_idx / field.width;
        let cur_col = cur_idx % field.width;
        let cur_walkable = grass.is_walkable(pos);
        let cur_r_max = field.r_max[cur_idx];
        let cur_grazeable = (field.r[cur_idx] - grass.reserve.floor(cur_r_max)).max(0.0);
        if cur_walkable && cur_r_max > 0.0 && cur_grazeable >= GRASS_STAY_FRACTION * cur_r_max {
            return None;
        }

        let r0 = cur_row.saturating_sub(GRASS_SCAN_RADIUS);
        let r1 = (cur_row + GRASS_SCAN_RADIUS).min(field.height - 1);
        let c0 = cur_col.saturating_sub(GRASS_SCAN_RADIUS);
        let c1 = (cur_col + GRASS_SCAN_RADIUS).min(field.width - 1);
        let mut best_grazeable = if cur_walkable { cur_grazeable } else { 0.0 };
        let mut best_cell: Option<Vec3> = None;

        for row in r0..=r1 {
            let cz = field.min_z + (row as f32 + 0.5) * cell_h;
            for col in c0..=c1 {
                let cx = field.min_x + (col as f32 + 0.5) * cell_w;
                let center = Vec3::new(cx, 0.0, cz);
                if !grass.is_walkable(center) {
                    continue;
                }
                let j = row * field.width + col;
                let g = (field.r[j] - grass.reserve.floor(field.r_max[j])).max(0.0);
                // Strictly greater preserves lowest-(row, col) tie-breaking and requires beating
                // the current cell before leaving Food fallback.
                if g > best_grazeable {
                    best_grazeable = g;
                    best_cell = Some(center);
                }
            }
        }
        if let Some(grass_target) = best_cell {
            return Some(grass_target);
        }
    }

    nearest_food.map(|(p, _)| p)
}

/// Normalised drive in `[0, 1]` from the mean amplitude of the agent's direct body segments.
fn drive_from_cpg(children: Option<&ChildrenLinks>, oscillators: &Query<&CpgOscillator>) -> f32 {
    let Some(children) = children else { return 0.0 };
    let mut sum = 0.0f32;
    let mut n = 0u32;
    for &child in children.0.iter() {
        if let Ok(osc) = oscillators.get(child) {
            sum += osc.amplitude.abs();
            n += 1;
        }
    }
    if n == 0 {
        return 0.0;
    }
    (sum / n as f32 / CPG_AMPLITUDE_SCALE).clamp(0.0, 1.0)
}

/// Writes `InertiaComponent.target_velocity` toward each living agent's sensory target.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn steering_drive_system(
    policy: Option<Res<LocomotionPolicy>>,
    income: Option<Res<crate::core::resources::PredatorIncomePolicy>>,
    grass_policy: Option<Res<crate::core::resources::GrassSteeringPolicy>>,
    field: Option<Res<crate::core::ecology::ResourceField>>,
    reserve: Option<Res<crate::core::resources::PlantReservePolicy>>,
    terrain_nav: Option<Res<crate::core::resources::TerrainNavigation>>,
    terrain_map: Option<Res<crate::core::terrain::TerrainMap>>,
    bounds: Option<Res<crate::core::resources::MapBounds>>,
    lake_query: Query<(&Position, &Lake)>,
    mut agents: Query<
        (
            &Position,
            &HomeostaticState,
            Option<&Predator>,
            Option<&Prey>,
            Option<&ChildrenLinks>,
            Option<&crate::core::island::inhabit::InhabitedControl>,
            &mut InertiaComponent,
        ),
        (With<Agent>, Without<ParentLink>),
    >,
    prey_query: Query<(&Position, &HomeostaticState), (With<Agent>, With<Prey>)>,
    food_query: Query<&Position, With<Food>>,
    oscillators: Query<&CpgOscillator>,
    mut caches: Local<(Vec<Vec3>, Vec<Vec3>, Vec<Vec3>, Vec<Vec3>)>,
) {
    let Some(policy) = policy.filter(|p| p.enabled) else {
        return;
    };
    let drive_floor = policy.drive_floor.clamp(0.0, 1.0);
    // ADR-0007 option B: predators only chase prey holding at least this much energy. `0.0` (or no
    // resource) admits every living prey, which is the historical rule.
    let target_min_energy = income.map(|p| p.target_min_energy).unwrap_or(0.0);
    let grass_enabled = grass_policy.is_some_and(|p| p.enabled);
    let grass_ctx = if grass_enabled {
        let reserve_policy = reserve.map(|r| *r).unwrap_or_default();
        let nav = if terrain_nav.is_some_and(|t| t.enabled) {
            terrain_map.as_deref().zip(bounds.as_deref())
        } else {
            None
        };
        field.as_deref().and_then(|f| {
            let n_cells = f.width.checked_mul(f.height)?;
            if n_cells == 0
                || f.r.len() < n_cells
                || f.r_max.len() < n_cells
                || f.max_x <= f.min_x
                || f.max_z <= f.min_z
            {
                return None;
            }
            let cell_w = (f.max_x - f.min_x) / f.width as f32;
            let cell_h = (f.max_z - f.min_z) / f.height as f32;
            let local_radius = (GRASS_SCAN_RADIUS as f32 + 0.5) * cell_w.max(cell_h);
            Some(GrassContext {
                field: f,
                reserve: reserve_policy,
                nav,
                local_food_radius_sq: local_radius * local_radius,
            })
        })
    } else {
        None
    };

    let (prey_cache, food_cache, walkable_food_cache, lake_cache) = &mut *caches;
    lake_cache.clear();
    if policy.thirst_steering {
        for (pos, lake) in lake_query.iter() {
            if lake.current_water > 0.0 {
                lake_cache.push(pos.0);
            }
        }
    }
    prey_cache.clear();
    for (pos, homeo) in prey_query.iter() {
        if homeo.energy > 0.0 && homeo.energy >= target_min_energy {
            prey_cache.push(pos.0);
        }
    }
    food_cache.clear();
    walkable_food_cache.clear();
    if let Some(grass) = grass_ctx.as_ref() {
        for pos in food_query.iter() {
            let p = pos.0;
            food_cache.push(p);
            if p.is_finite() && grass.is_walkable(p) {
                walkable_food_cache.push(p);
            }
        }
    } else {
        for pos in food_query.iter() {
            food_cache.push(pos.0);
        }
    }

    for (pos, homeo, predator, prey, children, opt_inhabited, mut inertia) in agents.iter_mut() {
        if homeo.energy <= 0.0 {
            inertia.target_velocity = Vec3::ZERO;
            continue;
        }
        let max_speed = if predator.is_some() {
            PREDATOR_MAX_SPEED
        } else {
            PREY_MAX_SPEED
        };
        if let Some(inhabited) = opt_inhabited {
            if inhabited.kind == crate::core::island::OrganismTargetKind::Animal {
                let raw_dir = Vec3::new(
                    inhabited.creature.move_dir_x,
                    0.0,
                    inhabited.creature.move_dir_z,
                );
                let dir_len_sq = raw_dir.length_squared();
                if !dir_len_sq.is_finite() || dir_len_sq <= 1e-6 {
                    inertia.target_velocity = Vec3::ZERO;
                } else {
                    let dir = raw_dir / dir_len_sq.sqrt();
                    let requested = inhabited.creature.speed_fraction.clamp(0.0, 1.0);
                    let cpg_cap = drive_from_cpg(children, &oscillators).max(drive_floor);
                    let speed = max_speed * requested.min(cpg_cap);
                    inertia.target_velocity = dir * speed;
                }
                continue;
            }
        }
        let thirsty = policy.thirst_steering
            && homeo.hydration < homeo.hydration_target * THIRST_STEERING_FRACTION;
        let steering_to_water = thirsty && !lake_cache.is_empty();
        // Priority 1 (thirst) and legacy paths use candidate scans; Priority 2/3 run only for
        // non-thirsty Prey when GrassSteeringPolicy::enabled is true.
        let selected = if !steering_to_water && predator.is_none() && prey.is_some() {
            if let Some(grass) = grass_ctx.as_ref() {
                select_prey_grass_target(pos.0, grass, walkable_food_cache)
                    .map(|target| (target, pos.0.distance_squared(target)))
            } else {
                find_nearest_point(pos.0, food_cache)
            }
        } else {
            let candidates = if steering_to_water {
                &*lake_cache
            } else if predator.is_some() {
                &*prey_cache
            } else {
                &*food_cache
            };
            find_nearest_point(pos.0, candidates)
        };
        inertia.target_velocity = match selected {
            Some((target, min_dist_sq)) if min_dist_sq > ARRIVAL_RADIUS * ARRIVAL_RADIUS => {
                let speed = max_speed * drive_from_cpg(children, &oscillators).max(drive_floor);
                (target - pos.0).normalize_or_zero() * speed
            }
            _ => Vec3::ZERO,
        };
    }
}
