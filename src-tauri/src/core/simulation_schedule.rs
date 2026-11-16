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
//! **F-041 update.** The paragraph above and the "two additions" section below describe the
//! schedule as a set of `.after(...)` / `.before(...)` edges. That is no longer how it is built:
//! edges left pairs unordered, and the per-process topological sort chose an order for them, which
//! made one seed give different worlds in different processes. The schedule is now a single chain
//! (see [`build_tick_schedule`]); the "only `.after`" rule below is moot because every system has a
//! declared place, but the capture and intervention systems remain inert without their resources.
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
/// The single-threaded executor walks the schedule's topological order. That order is a function
/// of the declared constraints **only where the constraints leave no freedom**: for two systems
/// with no edge between them, `ScheduleGraph::topsort` picks by a hash seeded per process
/// (FINDINGS.md F-029, F-041). Twelve processes of one binary produced twelve different executed
/// orders of these 52 nodes. That is not harmless: two systems that conflict on a component, or one
/// that counts entities while another spawns or despawns them through `Commands` (F-041:
/// `spawn_food_system` counting `Food` against `detect_food_collisions_system` despawning it), give
/// different worlds in different orders.
///
/// # One total order (F-041)
///
/// So the order is no longer a set of edges with gaps: **every system, and every flush point, is
/// in one sequence** that is chained, which leaves the topological sort exactly one answer in every
/// process. (`((a, b), (c, d)).chain()` orders the *groups* only — each group is chained by its own
/// `.chain()` below; a group without one is an unordered set. The first draft of this change got
/// that wrong and the gate in `schedule_ambiguity_tests.rs` is what caught it.)
///
/// The sequence is a real execution order, taken from a process whose world matched the
/// majority of twelve processes in the configuration that used to vary (mortality + continuous
/// reproduction; nine of the twelve distinct orders in that cohort gave the same checksum). Because
/// the former per-system `.after(...)` edges were a subset of that order, nothing that was
/// already process-invariant changes: the trunk default world, the E2 control and the 60-founder
/// mortality world keep exactly the checksums they had before.
///
/// `auto_insert_apply_deferred` is **off** and the five `apply_deferred` nodes are declared in the
/// sequence. With it on, a sync point is inserted after every `Commands` system a chain edge
/// touches, which moves every spawn/despawn flush and changes the trajectory; with it off the
/// flush points are exactly where this list puts them. A system that spawns or despawns therefore
/// has its `Commands` applied at the next `apply_deferred` below, or at the end of the tick.
///
/// The causal reading of the sequence, which the old edges used to state:
/// energy pipeline acquire -> contest -> spend -> count (grazing, regrowth, food collisions,
/// combat, metabolism, census); `SimRng` draw order manual_migration -> spawn_food ->
/// seed_dropping -> autonomous_reproduction -> check_epoch_completion; capture checkpoints after
/// the phase they bound; interventions before environmental effects, regrowth and metabolism.
/// `tests/schedule_ambiguity_tests.rs` asserts each of these by position, so a reordering that
/// breaks one fails loudly.
///
/// A system added without a place in this sequence is caught by
/// `the_tick_schedule_has_no_ambiguous_pair_at_all`; put it in the chain and re-run the
/// multi-process gates in `schedule_total_order_tests.rs`.
pub fn build_tick_schedule(deterministic: DeterministicMode) -> Schedule {
    let mut schedule = Schedule::default();
    if deterministic.is_enabled() {
        schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    }
    schedule.set_build_settings(bevy_ecs::schedule::ScheduleBuildSettings {
        auto_insert_apply_deferred: false,
        ..Default::default()
    });
    schedule.add_systems(
        (
            (
                crate::core::simulation_lod::sync_lod_focus_system,
                crate::core::observer::record_observer_trace_system,
                crate::core::observer::drain_observer_actions_system,
                crate::core::live_experiment::apply_live_interventions_system,
                sensory_system,
                crate::core::live_experiment::live_inference_pump_system,
                action_resolution_system,
                crate::core::tick_capture::capture_checkpoint_sensor_brain_system,
                update_cpg_system,
                resolve_joints_system,
                crate::core::locomotion::steering_drive_system,
                integrate_physics_system,
                crate::core::aggregate_population::dehydrate_cold_agents_system,
                herbivore_grazing_system,
            )
                .chain(),
            (
                crate::core::aggregate_population::dormant_cohort_ecology_system,
                detect_environmental_collisions_system,
                crate::core::ecs::manual_migration_system,
                apply_deferred,
                resource_field_regrowth_system,
                detect_food_collisions_system,
                crate::core::ecs::process_inbound_migrations_system,
                crate::core::ecs::check_migration_boundaries_system,
                apply_deferred,
                wrap_coordinates_system,
                rebuild_spatial_grid_system,
                crate::core::tick_capture::capture_checkpoint_physics_system,
                update_agent_evaluation_system,
            )
                .chain(),
            (
                crate::ai::pheromone::agent_release_pheromone_system,
                crate::ai::pheromone::update_pheromone_grid_system,
                crate::ai::pheromone::agent_read_pheromone_system,
                crate::core::aggregate_population::rehydrate_wakeable_chunks_system,
                receive_environmental_events_system,
                apply_environmental_effects_system,
                lake_replenishment_system,
                fruit_growth_system,
                spawn_food_system,
                apply_deferred,
                seed_dropping_system,
                combat_system,
                metabolic_decay_system,
            )
                .chain(),
            (
                hrrl_learning_system,
                crate::core::mortality::mortality_system,
                apply_deferred,
                autonomous_reproduction_system,
                apply_deferred,
                check_epoch_completion_system,
                apply_staggered_evolution_system,
                apply_deferred,
                ecosystem_census_system,
                demographic_census_system,
                crate::core::tick_capture::capture_checkpoint_ecology_system,
                energy_flux_aggregation_system,
                sync_evolution_settings_system,
            )
                .chain(),
        )
            .chain(),
    );
    schedule
}
