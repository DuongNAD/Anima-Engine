//! The live tick schedule, in one place.
//!
//! This is the schedule [`crate::core::simulation_loop::SimulationEngine::start`] runs, extracted
//! from the middle of its 900-line thread closure so that something other than the desktop app can
//! run it. That mattered: every headless test of the engine's dynamics used to declare its own
//! `.chain()`ed list of a dozen systems, which is a *different* schedule — different membership,
//! different ordering, different sync points — and a gate on a schedule the app does not run proves
//! very little about the app.
//!
//! The system list and every `.after(...)` below are the ones that were inline in `start`. Moving
//! them changes nothing about what runs or in what order; what it buys is that
//! [`crate::core::live_experiment`] and the app are now provably the same schedule, because they
//! call the same function.
//!
//! # The two additions, and why they cannot perturb anything
//!
//! Two things are here that were not in `start`: the tick-capture checkpoints
//! ([`crate::core::tick_capture`]) and the live intervention system
//! ([`crate::core::live_experiment::apply_live_interventions_system`]). Both are inert without a
//! resource that nothing inserts by default, so a stock run does exactly what it did before.
//!
//! Being *inert* is not enough on its own, though — a system can change a schedule without running
//! a single line of its body, by adding an ordering edge that constrains two systems that were
//! previously free of one another. So both additions follow one rule:
//!
//! - the capture checkpoints carry **only `.after(...)`** anchors (plus a chain among themselves),
//!   so every new edge points *into* a checkpoint and none points back out;
//! - the intervention system carries **only `.before(...)`** anchors, so every new edge points *out
//!   of* it and none points in.
//!
//! Neither shape can produce a path from one pre-existing system to another, which is the property
//! that would change the topological order. `capture_does_not_change_the_trajectory` and
//! `a_manifest_without_interventions_matches_the_bare_schedule` are the gates on that reasoning.

use bevy_ecs::prelude::*;

use crate::ai::cpg::update_cpg_system;
use crate::ai::model::hrrl_learning_system;
use crate::core::agent_systems::*;
use crate::core::determinism::DeterministicMode;
use crate::core::environmental_systems::*;
use crate::core::world_systems::*;
use crate::physics::{
    integrate_physics_system, rebuild_spatial_grid_system, resolve_joints_system,
};

/// The executor name a capture export records, so a reader can tell whether the
/// checkpoint-bounded phases could have overlapped.
pub const EXECUTOR_SINGLE_THREADED: &str = "single-threaded";
/// See [`EXECUTOR_SINGLE_THREADED`].
pub const EXECUTOR_MULTI_THREADED: &str = "multi-threaded";

/// Which executor [`build_tick_schedule`] would pick for `mode`.
pub fn executor_name(mode: DeterministicMode) -> &'static str {
    if mode.is_enabled() {
        EXECUTOR_SINGLE_THREADED
    } else {
        EXECUTOR_MULTI_THREADED
    }
}

/// Build the schedule the simulation thread runs once per tick.
///
/// G1.3: system execution order must be declared, not incidental.
///
/// Bevy's multi-threaded executor guarantees that two systems with conflicting access never run at
/// the same time, but NOT which of them goes first. The `.after(...)` constraints below pin the
/// order that matters causally; everything else was left to whatever the executor happened to pick,
/// which is not a property of the manifest. That is not a theoretical concern: G1.1 found an energy
/// residual whose *sign* changed between runs because of it, and G1.2's checkpoint gate had to
/// declare its own order to get a stable checksum at all.
///
/// The single-threaded executor walks the schedule's topological order, which is a function of the
/// declared constraints and insertion order alone — the same binary and manifest therefore produce
/// the same order every time. It costs parallelism, which is the correct trade for a run whose
/// purpose is to be reproducible; an interactive session leaves determinism off and keeps the
/// multi-threaded executor.
pub fn build_tick_schedule(deterministic: DeterministicMode) -> Schedule {
    let mut schedule = Schedule::default();
    if deterministic.is_enabled() {
        schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    }
    schedule.add_systems((
        sync_evolution_settings_system,
        receive_environmental_events_system,
        apply_environmental_effects_system.after(receive_environmental_events_system),
        sensory_system,
        action_resolution_system,
        update_cpg_system.after(action_resolution_system),
        resolve_joints_system.after(update_cpg_system),
        integrate_physics_system.after(resolve_joints_system),
        crate::ai::pheromone::agent_release_pheromone_system.after(integrate_physics_system),
        crate::ai::pheromone::update_pheromone_grid_system
            .after(crate::ai::pheromone::agent_release_pheromone_system),
        crate::ai::pheromone::agent_read_pheromone_system
            .after(crate::ai::pheromone::update_pheromone_grid_system),
    ));
    schedule.add_systems((
        update_agent_evaluation_system.after(integrate_physics_system),
        crate::core::ecs::check_migration_boundaries_system.after(integrate_physics_system),
        apply_deferred.after(crate::core::ecs::check_migration_boundaries_system),
        wrap_coordinates_system.after(apply_deferred),
        rebuild_spatial_grid_system.after(wrap_coordinates_system),
        crate::core::ecs::process_inbound_migrations_system.after(integrate_physics_system),
        metabolic_decay_system.after(integrate_physics_system),
        spawn_food_system.after(apply_environmental_effects_system),
        detect_food_collisions_system.after(integrate_physics_system),
        combat_system.after(integrate_physics_system),
        hrrl_learning_system.after(metabolic_decay_system),
        // Runs after `hrrl_learning_system`, which is where `LastTransitionState` and the
        // homeostatic deviation this reads are refreshed. Returns immediately unless both
        // evolved brains and lifetime learning are switched on.
        crate::ai::model::lifetime_learning_system.after(hrrl_learning_system),
        check_epoch_completion_system.after(metabolic_decay_system),
        apply_staggered_evolution_system.after(check_epoch_completion_system),
        crate::core::ecs::manual_migration_system.after(integrate_physics_system),
        fruit_growth_system.after(apply_environmental_effects_system),
        lake_replenishment_system.after(apply_environmental_effects_system),
        seed_dropping_system.after(apply_environmental_effects_system),
        detect_environmental_collisions_system.after(integrate_physics_system),
    ));

    // Ecosystem-dynamics systems (Phase 7) in their own tuple — Bevy caps a single
    // add_systems tuple at 20, and `.after(...)` ordering resolves across calls.
    schedule.add_systems((
        herbivore_grazing_system.after(integrate_physics_system),
        resource_field_regrowth_system.after(herbivore_grazing_system),
        // The app's focus reaches the world here, ahead of both readers — `sensory_system`,
        // which tiers inference, and the dormancy systems below. Ordered explicitly rather
        // than left to Bevy: an unconstrained sync would let a tick tier agents against
        // last tick's camera, which is harmless for a moving observer and confusing to
        // debug.
        crate::core::simulation_lod::sync_lod_focus_system
            .before(sensory_system)
            .before(crate::core::aggregate_population::dehydrate_cold_agents_system),
        // ADR-0004 O2. Records what the world actually saw of the observer, so it must run
        // *after* the policy has had its say — reading the raw shared focus instead would
        // file a camera path the world never experienced.
        crate::core::observer::record_observer_trace_system
            .after(crate::core::simulation_lod::sync_lod_focus_system),
        // Ordered after the focus recorder so a tick's samples and that tick's actions land
        // in the same trace in a fixed order — two runs of the same session then produce the
        // same record, which is what makes a trace comparable at all.
        crate::core::observer::drain_observer_actions_system
            .after(crate::core::observer::record_observer_trace_system),
        // Simulation LOD tier two. Both return immediately without a `DormantCohorts`
        // resource, which is absent unless `ANIMA_AGGREGATE_LOD` is set, so a stock run is
        // unaffected.
        //
        // After physics, so an agent is tiered on the position it actually reached this
        // tick; before the census, because the census is the only place a dormant cohort's
        // energy is counted and it has to see the result of both. Bevy inserts the sync
        // point that applies their commands from these ordering constraints.
        crate::core::aggregate_population::dehydrate_cold_agents_system
            .after(integrate_physics_system),
        crate::core::aggregate_population::rehydrate_wakeable_chunks_system
            .after(crate::core::aggregate_population::dehydrate_cold_agents_system),
        // The dormant cohorts' own ecology, sitting where its live counterparts sit: after
        // live grazing and before regrowth, so both consumers draw on the same standing
        // field before it grows back.
        crate::core::aggregate_population::dormant_cohort_ecology_system
            .after(herbivore_grazing_system)
            .before(resource_field_regrowth_system),
        // The census does not accumulate, it takes a **snapshot** — `pool.animals = Σ reserves`,
        // recomputed from scratch — so unlike `plants` and `detritus`, which are carried
        // incrementally, its value is entirely a function of *when* in the tick it is taken.
        //
        // It used to declare only the two edges below, and nothing about the systems that move
        // agent reserves. Two systems with no edge between them are ordered by the schedule's
        // topological sort, and that sort is not a declared property of the schedule — so the order
        // was chosen per process. Measured at commit `0bcb330`: five runs of one release binary at
        // one seed gave two distinct `live.animals_eu` trajectories, 0.186 EU apart, which is one
        // tick of metabolic decay across ten agents. Every other observable, and the world
        // checksum, were bit-identical; `world_checksum` covers reserves and the resource field but
        // not `EcosystemBiomass`, which is why no existing gate saw it.
        //
        // Every system that moves EU into or out of an agent's reserve is named here. Grazing is
        // already transitively ordered through regrowth; it is stated anyway, because a reader
        // checking this list should not have to prove a path through a third system, and because
        // regrowth's edge could change without anyone thinking about the census.
        ecosystem_census_system
            .after(resource_field_regrowth_system)
            .after(crate::core::aggregate_population::rehydrate_wakeable_chunks_system)
            .after(metabolic_decay_system)
            .after(detect_food_collisions_system)
            .after(combat_system)
            .after(herbivore_grazing_system),
    ));

    // A declared experiment's interventions reach the world here. `.before(...)` only — see the
    // module docs — and inert without a `LiveInterventions` resource, which only the experiment
    // adapter inserts.
    schedule.add_systems(
        crate::core::live_experiment::apply_live_interventions_system
            .before(apply_environmental_effects_system)
            .before(resource_field_regrowth_system)
            .before(metabolic_decay_system),
    );

    // A declared experiment answers its own inference, in the tick that asked for it, so a
    // checkpoint boundary holds no in-flight work — see `core::live_experiment::LiveInferencePump`.
    // Inert without the pump resource, which only the experiment adapter inserts.
    //
    // This is the one addition that carries **both** an `.after` and a `.before`, and therefore the
    // one that adds an edge between two pre-existing systems: `sensory_system` before
    // `action_resolution_system`. That is deliberate and it changes nothing, because the two
    // already conflict on `&mut CognitiveState` and so were already serialised — only *which* of
    // them went first was undeclared, and the single-threaded executor already picked this order
    // from insertion order. G1.3's whole argument is that an order the engine relies on should be
    // declared rather than incidental; this declares one it already relied on.
    schedule.add_systems(
        crate::core::live_experiment::live_inference_pump_system
            .after(sensory_system)
            .before(action_resolution_system),
    );

    // Tick-capture checkpoints. `.after(...)` only, plus a chain among themselves — see the module
    // docs — and inert without a `TickCaptureSink` resource.
    schedule.add_systems(
        (
            crate::core::tick_capture::capture_checkpoint_sensor_brain_system
                .after(action_resolution_system),
            crate::core::tick_capture::capture_checkpoint_physics_system
                .after(rebuild_spatial_grid_system),
            crate::core::tick_capture::capture_checkpoint_ecology_system
                .after(ecosystem_census_system),
        )
            .chain(),
    );

    schedule
}
