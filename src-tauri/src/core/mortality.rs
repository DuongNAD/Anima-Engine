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
/// The two counters (`HomeostaticState::starving_ticks` for energy, `dehydrating_ticks` for
/// hydration, separate since F-044) are incremented inside `metabolic_decay_system`, which runs
/// earlier in the same tick, so the check below sees this tick's post-decay state: an agent floored
/// to zero by *this* tick's metabolism can die in *this* tick. That timing is what MO-S04 pins.
pub fn mortality_system(
    mut commands: Commands,
    agents: Query<
        (
            Entity,
            &HomeostaticState,
            Option<&crate::core::agent_systems::AgentEvaluation>,
            Option<&crate::core::components::Predator>,
            Option<&crate::core::agent_systems::AgentLineageId>,
            Option<&crate::core::agent_systems::AgentGeneration>,
            Option<&crate::core::ecs::Position>,
        ),
        With<Agent>,
    >,
    policy: Option<Res<MortalityPolicy>>,
    mut vitals: Option<ResMut<crate::core::resources::VitalStatistics>>,
    mut tracker: Option<ResMut<crate::core::telemetry_export::DemographicTracker>>,
    mut drift: Option<ResMut<crate::core::drift_control::DriftControlState>>,
) {
    // Missing resource ≠ off by coincidence — both are handled explicitly, and both mean the
    // legacy behaviour: nobody dies of hunger (ADR-0004's reading of an undeclared policy).
    let Some(policy) = policy else {
        return;
    };
    if !policy.enabled {
        return;
    }

    if let Some(drift_res) = drift.as_deref_mut().filter(|d| d.is_replay()) {
        let scheduled = drift_res.current_turnover();
        let req_prey = scheduled.prey_deaths as usize;
        let req_pred = scheduled.pred_deaths as usize;
        let req_total = req_prey + req_pred;
        if req_total == 0 {
            return;
        }

        let world_tick = drift_res.current_world_tick;
        let mut actual_total = 0u64;
        let mut missed_total = 0u64;
        {
            let (prey_buf, pred_buf, drift_rng) = drift_res.split_death_buffers_and_rng();
            prey_buf.clear();
            pred_buf.clear();
            for (entity, _, _, opt_pred, ..) in agents.iter() {
                if opt_pred.is_some() {
                    pred_buf.push(entity);
                } else {
                    prey_buf.push(entity);
                }
            }

            for (req, buf) in [(req_prey, prey_buf), (req_pred, pred_buf)] {
                if req == 0 {
                    continue;
                }
                if buf.len() > 1 {
                    buf.sort_unstable_by(|&a, &b| match (agents.get(a), agents.get(b)) {
                        (Ok((ea, ha, _, _, la, ga, pa)), Ok((eb, hb, _, _, lb, gb, pb))) => {
                            crate::core::drift_control::candidate_cmp(
                                (
                                    la.map_or("", |l| l.0.as_str()),
                                    ga.map_or(0, |g| g.0),
                                    pa.map_or(0.0, |p| p.0.x),
                                    pa.map_or(0.0, |p| p.0.z),
                                    ha.energy,
                                    ea.index(),
                                ),
                                (
                                    lb.map_or("", |l| l.0.as_str()),
                                    gb.map_or(0, |g| g.0),
                                    pb.map_or(0.0, |p| p.0.x),
                                    pb.map_or(0.0, |p| p.0.z),
                                    hb.energy,
                                    eb.index(),
                                ),
                            )
                        }
                        _ => std::cmp::Ordering::Equal,
                    });
                }
                let victims = crate::core::drift_control::draw_victims_without_replacement(
                    buf.as_mut_slice(),
                    req,
                    drift_rng,
                );
                let actual = victims.len();
                actual_total += actual as u64;
                missed_total += req.saturating_sub(actual) as u64;
                for &victim in victims {
                    let lifespan = agents
                        .get(victim)
                        .ok()
                        .and_then(|(_, _, eval, ..)| eval.map(|e| e.survival_ticks))
                        .unwrap_or(0);
                    if let Some(vitals) = vitals.as_mut() {
                        vitals.record_death(lifespan);
                    }
                    if let Some(trk) = tracker.as_mut() {
                        let cause = if lifespan >= 300 {
                            "Senescence"
                        } else {
                            "Starvation"
                        };
                        trk.record_death(cause);
                    }
                    commands.add(ReclaimAndDespawnAgentCommand { root: victim });
                }
            }
        }
        drift_res.diagnostics.scheduled_deaths += req_total as u64;
        drift_res.diagnostics.actual_deaths += actual_total;
        if missed_total > 0 {
            drift_res.diagnostics.missed_deaths += missed_total;
            drift_res.diagnostics.ticks_with_missed_deaths += 1;
            drift_res.diagnostics.note_divergence(world_tick);
        }
        return;
    }

    // No collection, no sort: this is a tick system and must not allocate (CLAUDE.md). Query
    // iteration order is deterministic for a given world history.
    for (entity, homeo, opt_eval, opt_pred, ..) in agents.iter() {
        let Some(verdict) = lethal_cause(homeo, &policy) else {
            continue;
        };
        if let Some(drift_res) = drift.as_deref_mut() {
            drift_res.record_death(opt_pred.is_some());
        }
        // Turnover bookkeeping for the branch-O observables. The animal's lifespan IS its
        // `survival_ticks` at death — the counter only ever advanced while it was alive, so
        // no separate birth-tick field is needed (design rule C).
        let lifespan = opt_eval.map(|e| e.survival_ticks).unwrap_or(0);
        if let Some(vitals) = vitals.as_mut() {
            vitals.record_death_by(verdict.cause, lifespan, verdict.both_lethal);
        }
        // Cause rule carried over from `agent_mortality_system`. `DemographicTracker` has no
        // dehydration cause, so a death by thirst is filed under "Starvation" (the tracker's
        // own fallback for unknown causes) rather than inventing a new string. These labels are
        // known to be wrong (no ageing exists behind "Senescence"); the cause split that is
        // right lives in `VitalStatistics` (card S9), and fixing the tracker is a separate card.
        if let Some(trk) = tracker.as_mut() {
            let cause = if lifespan >= 300 {
                "Senescence"
            } else {
                "Starvation"
            };
            trk.record_death(cause);
        }
        commands.add(ReclaimAndDespawnAgentCommand { root: entity });
    }
}

/// What killed an agent, as `VitalStatistics` counts it (card S9). Exactly one cause per death.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeathCause {
    /// Energy exhausted for at least `MortalityPolicy::starve_ticks` consecutive ticks.
    Starvation,
    /// Hydration exhausted for at least `MortalityPolicy::dehydration_ticks` consecutive ticks.
    Dehydration,
}

/// The outcome of [`lethal_cause`]: the one cause a death is filed under, and whether the other
/// condition was lethal on the same tick as well.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeathVerdict {
    pub cause: DeathCause,
    /// Both conditions were past their grace on the tick of death. The death still counts once,
    /// under `cause`; this flag only lets a measurement say how many deaths the rule decided.
    pub both_lethal: bool,
}

/// Whether `homeo` dies under `policy` this tick, and of what.
///
/// # Attribution rule (card S9)
///
/// A death has exactly one cause. When only one condition is lethal, that is the cause. When both
/// are, the death goes to **the condition that became lethal first**: the one whose streak is
/// further past its own grace (`streak − grace`). An exact tie goes to **starvation**.
///
/// In an uninterrupted run with mortality enabled from the start, an agent dies on the first tick
/// any condition is lethal, so "both lethal" can only arise when both crossed on the same tick and
/// the tie-break decides it. Unequal overshoots arise after a policy is enabled mid-run or a save
/// is restored; the first-lethal rule then names the condition that would have killed it earlier.
///
/// The thirst streak is `dehydrating_ticks`, or the shared-era `starving_ticks` when the field is
/// `None` (a save from before F-044, before its first metabolic tick) — the same reading the old
/// rule used.
pub fn lethal_cause(homeo: &HomeostaticState, policy: &MortalityPolicy) -> Option<DeathVerdict> {
    let starved = homeo.energy <= 0.0 && homeo.starving_ticks >= policy.starve_ticks;
    let thirst = homeo.dehydrating_ticks.unwrap_or(homeo.starving_ticks);
    let dehydrated = homeo.hydration <= 0.0 && thirst >= policy.dehydration_ticks;
    let cause = match (starved, dehydrated) {
        (false, false) => return None,
        (true, false) => DeathCause::Starvation,
        (false, true) => DeathCause::Dehydration,
        (true, true) => {
            let starve_over = homeo.starving_ticks - policy.starve_ticks;
            let thirst_over = thirst - policy.dehydration_ticks;
            if thirst_over > starve_over {
                DeathCause::Dehydration
            } else {
                DeathCause::Starvation
            }
        }
    };
    Some(DeathVerdict {
        cause,
        both_lethal: starved && dehydrated,
    })
}
