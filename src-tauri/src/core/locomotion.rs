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
    Agent, ChildrenLinks, Food, InertiaComponent, ParentLink, Position, Predator, Prey,
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
#[allow(clippy::type_complexity)]
pub fn steering_drive_system(
    policy: Option<Res<LocomotionPolicy>>,
    mut agents: Query<
        (
            &Position,
            &HomeostaticState,
            Option<&Predator>,
            Option<&ChildrenLinks>,
            &mut InertiaComponent,
        ),
        (With<Agent>, Without<ParentLink>),
    >,
    prey_query: Query<(&Position, &HomeostaticState), (With<Agent>, With<Prey>)>,
    food_query: Query<&Position, With<Food>>,
    oscillators: Query<&CpgOscillator>,
    mut caches: Local<(Vec<Vec3>, Vec<Vec3>)>,
) {
    if !policy.is_some_and(|p| p.enabled) {
        return;
    }
    let (prey_cache, food_cache) = &mut *caches;
    prey_cache.clear();
    for (pos, homeo) in prey_query.iter() {
        if homeo.energy > 0.0 {
            prey_cache.push(pos.0);
        }
    }
    food_cache.clear();
    for pos in food_query.iter() {
        food_cache.push(pos.0);
    }

    for (pos, homeo, predator, children, mut inertia) in agents.iter_mut() {
        if homeo.energy <= 0.0 {
            inertia.target_velocity = Vec3::ZERO;
            continue;
        }
        let (candidates, max_speed) = if predator.is_some() {
            (&*prey_cache, PREDATOR_MAX_SPEED)
        } else {
            (&*food_cache, PREY_MAX_SPEED)
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
                let speed = max_speed * drive_from_cpg(children, &oscillators);
                (target - pos.0).normalize_or_zero() * speed
            }
            _ => Vec3::ZERO,
        };
    }
}
