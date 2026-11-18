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

/// With `LocomotionPolicy::grass_steering` on (card S11), a prey agent whose current pasture cell
/// holds at least this fraction of its carrying capacity `r_max` in grazeable grass (`max(r -
/// floor, 0)`) sets `target_velocity` to zero so it stays and grazes instead of chasing elsewhere.
pub const GRASS_STAY_FRACTION: f32 = 0.25;

/// Half-width in pasture cells of the square window (`(2 * R + 1) x (2 * R + 1)`, so `3` scans
/// a `7 x 7` window clipped to the field bounds) scanned for richer grass when a prey leaves a
/// depleted cell (card S11).
pub const GRASS_SCAN_RADIUS: usize = 3;

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
    field: Option<Res<crate::core::ecology::ResourceField>>,
    reserve: Option<Res<crate::core::resources::PlantReservePolicy>>,
    lake_query: Query<(&Position, &Lake)>,
    mut agents: Query<
        (
            &Position,
            &HomeostaticState,
            Option<&Predator>,
            Option<&Prey>,
            Option<&ChildrenLinks>,
            &mut InertiaComponent,
        ),
        (With<Agent>, Without<ParentLink>),
    >,
    prey_query: Query<(&Position, &HomeostaticState), (With<Agent>, With<Prey>)>,
    food_query: Query<&Position, With<Food>>,
    oscillators: Query<&CpgOscillator>,
    mut caches: Local<(Vec<Vec3>, Vec<Vec3>, Vec<Vec3>)>,
) {
    let Some(policy) = policy.filter(|p| p.enabled) else {
        return;
    };
    let drive_floor = policy.drive_floor.clamp(0.0, 1.0);
    // ADR-0007 option B: predators only chase prey holding at least this much energy. `0.0` (or no
    // resource) admits every living prey, which is the historical rule.
    let target_min_energy = income.map(|p| p.target_min_energy).unwrap_or(0.0);
    let reserve_policy = reserve.map(|r| *r).unwrap_or_default();
    let (prey_cache, food_cache, lake_cache) = &mut *caches;
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
    for pos in food_query.iter() {
        food_cache.push(pos.0);
    }

    for (pos, homeo, predator, prey, children, mut inertia) in agents.iter_mut() {
        if homeo.energy <= 0.0 {
            inertia.target_velocity = Vec3::ZERO;
            continue;
        }
        let max_speed = if predator.is_some() {
            PREDATOR_MAX_SPEED
        } else {
            PREY_MAX_SPEED
        };
        let thirsty = policy.thirst_steering
            && homeo.hydration < homeo.hydration_target * THIRST_STEERING_FRACTION;
        let steering_to_water = thirsty && !lake_cache.is_empty();

        // Card S11: grass-seeking steering applies only to prey that are not steering to water.
        if policy.grass_steering && !steering_to_water && predator.is_none() && prey.is_some() {
            if let Some(field) = field.as_deref() {
                let n_cells = field.width.saturating_mul(field.height);
                if n_cells > 0 && field.r.len() >= n_cells && field.r_max.len() >= n_cells {
                    if let Some(cur_idx) = field.cell_index(pos.0.x, pos.0.z) {
                        let cur_r_max = field.r_max[cur_idx];
                        let cur_floor = reserve_policy.floor(cur_r_max);
                        let cur_grazeable = (field.r[cur_idx] - cur_floor).max(0.0);
                        if cur_r_max > 0.0 && cur_grazeable >= GRASS_STAY_FRACTION * cur_r_max {
                            inertia.target_velocity = Vec3::ZERO;
                            continue;
                        }
                        let cur_row = cur_idx / field.width;
                        let cur_col = cur_idx % field.width;
                        let r0 = cur_row.saturating_sub(GRASS_SCAN_RADIUS);
                        let r1 = (cur_row + GRASS_SCAN_RADIUS).min(field.height - 1);
                        let c0 = cur_col.saturating_sub(GRASS_SCAN_RADIUS);
                        let c1 = (cur_col + GRASS_SCAN_RADIUS).min(field.width - 1);
                        let mut best_grazeable = cur_grazeable;
                        let mut best_cell: Option<(usize, usize)> = None;
                        for row in r0..=r1 {
                            for col in c0..=c1 {
                                let j = row * field.width + col;
                                let g =
                                    (field.r[j] - reserve_policy.floor(field.r_max[j])).max(0.0);
                                // Strictly greater breaks ties in favour of the lowest (row, col)
                                // and requires beating the current cell before leaving Food fallback.
                                if g > best_grazeable {
                                    best_grazeable = g;
                                    best_cell = Some((row, col));
                                }
                            }
                        }
                        if let Some((best_row, best_col)) = best_cell {
                            let cell_w = (field.max_x - field.min_x) / field.width as f32;
                            let cell_h = (field.max_z - field.min_z) / field.height as f32;
                            let grass_target = Vec3::new(
                                field.min_x + (best_col as f32 + 0.5) * cell_w,
                                0.0,
                                field.min_z + (best_row as f32 + 0.5) * cell_h,
                            );
                            let grass_dist_sq = pos.0.distance_squared(grass_target);
                            let mut nearest_food: Option<Vec3> = None;
                            let mut min_food_dist_sq = f32::MAX;
                            for food_pos in food_cache.iter() {
                                let d = pos.0.distance_squared(*food_pos);
                                if d < min_food_dist_sq {
                                    min_food_dist_sq = d;
                                    nearest_food = Some(*food_pos);
                                }
                            }
                            let (target, min_dist_sq) = match nearest_food {
                                Some(food_pos) if min_food_dist_sq < grass_dist_sq => {
                                    (food_pos, min_food_dist_sq)
                                }
                                _ => (grass_target, grass_dist_sq),
                            };
                            inertia.target_velocity =
                                if min_dist_sq > ARRIVAL_RADIUS * ARRIVAL_RADIUS {
                                    let speed = max_speed
                                        * drive_from_cpg(children, &oscillators).max(drive_floor);
                                    (target - pos.0).normalize_or_zero() * speed
                                } else {
                                    Vec3::ZERO
                                };
                            continue;
                        }
                    }
                }
            }
        }

        let candidates = if steering_to_water {
            &*lake_cache
        } else if predator.is_some() {
            &*prey_cache
        } else {
            &*food_cache
        };
        let mut nearest: Option<Vec3> = None;
        let mut min_dist_sq = f32::MAX;
        for target in candidates.iter() {
            let d = pos.0.distance_squared(*target);
            if d < min_dist_sq {
                min_dist_sq = d;
                nearest = Some(*target);
            }
        }
        inertia.target_velocity = match nearest {
            Some(target) if min_dist_sq > ARRIVAL_RADIUS * ARRIVAL_RADIUS => {
                let speed = max_speed * drive_from_cpg(children, &oscillators).max(drive_floor);
                (target - pos.0).normalize_or_zero() * speed
            }
            _ => Vec3::ZERO,
        };
    }
}
