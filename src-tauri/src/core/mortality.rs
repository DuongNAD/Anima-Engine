//! Death, as a declared policy — feature ecological-selection, branch M.
//!
//! The engine simulates hunger to the joule but nothing dies of it: `energy <= 0` only skips an
//! evaluation. This module adds the *condition* under which the already-atomic death primitive
//! fires. It adds no energy pathway of its own — rule B of the design's four rules — because
//! [`ReclaimAndDespawnAgentCommand`] already banks a corpse's reserve into detritus at the same
//! sync point that destroys the entity; subtracting energy anywhere else would leak EU while
//! looking perfectly reasonable.
//!
//! Placement (declared in `core::simulation_schedule::build_tick_schedule`, the one place the
//! tick schedule is built): **after** `metabolic_decay_system`, so an animal that just ate is
//! not killed by last tick's numbers, and **before** `ecosystem_census_system`, so the census
//! counts the world after the corpse has become detritus rather than a half-finished world.
//!
//! Inert by construction: with no [`MortalityPolicy`] resource, or with `enabled == false`, the
//! system returns before touching anything, so a flag-off run is today's run bit for bit.

use bevy_ecs::prelude::*;

use crate::ai::hrrl::HomeostaticState;
use crate::core::agent_systems::ReclaimAndDespawnAgentCommand;
use crate::core::ecs::Agent;
use crate::core::resources::MortalityPolicy;

/// Kills agents whose bottom-streak has outlasted the declared thresholds by queueing
/// [`ReclaimAndDespawnAgentCommand`] — the only sanctioned way to end an agent.
///
/// The counter (`HomeostaticState::starving_ticks`) is incremented inside
/// `metabolic_decay_system`, which runs earlier in the same tick, so the check below sees this
/// tick's post-decay state: an agent floored to zero by *this* tick's metabolism can die in
/// *this* tick. That timing is what MO-S04 pins.
pub fn mortality_system(
    mut commands: Commands,
    agents: Query<
        (
            Entity,
            &HomeostaticState,
            Option<&crate::core::agent_systems::AgentEvaluation>,
        ),
        With<Agent>,
    >,
    policy: Option<Res<MortalityPolicy>>,
    mut vitals: Option<ResMut<crate::core::resources::VitalStatistics>>,
) {
    // Missing resource ≠ off by coincidence — both are handled explicitly, and both mean the
    // legacy behaviour: nobody dies of hunger (ADR-0004's reading of an undeclared policy).
    let Some(policy) = policy else {
        return;
    };
    if !policy.enabled {
        return;
    }
    for (entity, homeo, opt_eval) in agents.iter() {
        let starved = homeo.energy <= 0.0 && homeo.starving_ticks >= policy.starve_ticks;
        let dehydrated = homeo.hydration <= 0.0 && homeo.starving_ticks >= policy.dehydration_ticks;
        if starved || dehydrated {
            // Turnover bookkeeping for the branch-O observables. The animal's lifespan IS its
            // `survival_ticks` at death — the counter only ever advanced while it was alive, so
            // no separate birth-tick field is needed (design rule C).
            if let Some(vitals) = vitals.as_mut() {
                let lifespan = opt_eval.map(|e| e.survival_ticks).unwrap_or(0);
                vitals.record_death(lifespan);
            }
            commands.add(ReclaimAndDespawnAgentCommand { root: entity });
        }
    }
}
