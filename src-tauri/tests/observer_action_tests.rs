//! ADR-0004 C3 — what a human *did* to the running world, on the record.
//!
//! # The premise the ADR got wrong
//!
//! O2 deferred `ObserverSample.actions` because the engine supposedly had no embodied actions yet —
//! the observer had a camera and nothing else. That was false. Four IPC commands already wrote
//! straight into a running world with no declaration, no record and no attribution:
//!
//! | Command | What it writes |
//! |---|---|
//! | `update_evolution_settings` | `mutation_rate` and `selection_bias`, into the settings the evo thread reads |
//! | `toggle_evolution` | selection on or off |
//! | `trigger_migration` | agents leave for another shard |
//! | `set_sharding_config` | how the world is partitioned |
//!
//! ADR-0004 called the camera the fifth source of outside-world leakage. These are stronger: the
//! camera changes *which agents think*, while the first of these changes the laws selection runs
//! under, mid-run, on a population that was living under the old ones.
//!
//! # What these tests hold, and what they do not
//!
//! They hold the seam: an action queued by a command reaches the trace, stamped with the tick the
//! world saw it on and rooted at `CAUSE_OBSERVER`. They do **not** hold that a command *cannot*
//! write without going through here — that is enforcement, and it needs the causal ledger to exist
//! in the live Bevy world (G2). Record first, enforce second, mirroring O2-then-O3.

use anima_domain::causal::{CausalLedger, CAUSE_BACKGROUND, CAUSE_OBSERVER};
use anima_engine_lib::core::observer::{
    drain_observer_actions_system, ObserverAction, ObserverTrace, SharedObserverActions,
    DEFAULT_OBSERVER_ACTION_CAPACITY,
};
use bevy_ecs::prelude::*;

fn settings_change(rate: f64) -> ObserverAction {
    ObserverAction::EvolutionSettingsChanged {
        mutation_rate: rate,
        selection_bias: 1.5,
        grid_resolution: 50,
    }
}

/// A world wired the way `start` wires it: the queue as a shared handle, the trace as a resource,
/// and the drain as the only thing that moves one into the other.
fn world_with(capacity: usize) -> (World, Schedule, SharedObserverActions) {
    let mut world = World::new();
    let queue = SharedObserverActions::new();
    world.insert_resource(queue.clone());
    world.insert_resource(ObserverTrace::with_capacity(capacity));
    let mut schedule = Schedule::default();
    schedule.add_systems(drain_observer_actions_system);
    (world, schedule, queue)
}

// --- the seam ------------------------------------------------------------------------------------

/// **The C3 gate.** An action a command queued reaches the trace, attributed to the observer.
#[test]
fn a_queued_action_reaches_the_trace_rooted_at_the_observer() {
    let (mut world, mut schedule, queue) = world_with(64);

    queue.push(settings_change(0.15));
    schedule.run(&mut world);

    let trace = world.resource::<ObserverTrace>();
    assert_eq!(trace.actions().len(), 1);
    let rec = trace.actions()[0];
    assert_eq!(rec.action, settings_change(0.15));
    assert_eq!(
        rec.cause_id, CAUSE_OBSERVER,
        "an action a human took must root at CAUSE_OBSERVER, or the ledger files it as baseline \
         dynamics — the specific lie ADR-0004 exists to stop telling"
    );
    assert_eq!(rec.tick, 1, "the drain stamps the tick the world saw it on");
}

/// The tick is stamped by the drain, because that is the only place that knows it. A command runs on
/// another thread with no idea what tick the world is on.
#[test]
fn actions_are_stamped_with_the_tick_they_were_drained_on() {
    let (mut world, mut schedule, queue) = world_with(64);

    schedule.run(&mut world); // tick 1, nothing queued
    queue.push(ObserverAction::EvolutionToggled { running: false });
    schedule.run(&mut world); // tick 2, drains the toggle
    schedule.run(&mut world); // tick 3, nothing
    queue.push(ObserverAction::MigrationTriggered { target_port: 7001 });
    schedule.run(&mut world); // tick 4

    let trace = world.resource::<ObserverTrace>();
    let ticks: Vec<u64> = trace.actions().iter().map(|a| a.tick).collect();
    assert_eq!(ticks, vec![2, 4]);
}

/// **Unlike focus samples, actions are never de-duplicated.** Two identical actions a second apart
/// are two things a human did. Collapsing them would under-report what the world was subjected to.
#[test]
fn two_identical_actions_are_two_records() {
    let (mut world, mut schedule, queue) = world_with(64);

    queue.push(ObserverAction::EvolutionToggled { running: true });
    queue.push(ObserverAction::EvolutionToggled { running: true });
    schedule.run(&mut world);

    assert_eq!(
        world.resource::<ObserverTrace>().actions().len(),
        2,
        "an action is an event, not a state — de-duplicating it loses one human decision"
    );
}

/// Full means declared full, the same contract the focus buffer has. Counted separately, because
/// losing an action is a provenance hole while losing a focus sample only costs replay fidelity.
#[test]
fn a_full_action_buffer_counts_what_it_could_not_keep() {
    let mut trace = ObserverTrace::with_capacity(4);
    let cap = trace.actions().len(); // 0
    assert_eq!(cap, 0);

    // The action buffer is sized by its own constant, not by the focus capacity above.
    for i in 0..(DEFAULT_OBSERVER_ACTION_CAPACITY + 5) {
        trace.record_action(anima_engine_lib::core::observer::ObserverActionRecord {
            tick: i as u64,
            action: ObserverAction::EvolutionToggled { running: true },
            cause_id: CAUSE_OBSERVER,
        });
    }
    assert_eq!(trace.actions().len(), DEFAULT_OBSERVER_ACTION_CAPACITY);
    assert_eq!(trace.dropped_actions(), 5);
    assert_eq!(
        trace.dropped(),
        0,
        "dropped focus samples and dropped actions are different holes and must not share a counter"
    );
}

/// Inert without the resources, like every other part of this subsystem: a run that never installed
/// them behaves exactly as it did before ADR-0004 rather than panicking.
#[test]
fn without_the_resources_the_drain_does_nothing() {
    let mut world = World::new();
    let mut schedule = Schedule::default();
    schedule.add_systems(drain_observer_actions_system);
    schedule.run(&mut world);
    assert!(world.get_resource::<ObserverTrace>().is_none());
}

/// A queue with nothing in it must not manufacture a record. Sounds trivial; it is the difference
/// between "no human touched this run" and "we lost the record of who did".
#[test]
fn an_empty_queue_records_nothing() {
    let (mut world, mut schedule, _queue) = world_with(64);
    for _ in 0..20 {
        schedule.run(&mut world);
    }
    assert!(world.resource::<ObserverTrace>().actions().is_empty());
}

// --- the four commands must keep queueing --------------------------------------------------------

/// A source scan, in the style of `sim_determinism_tests`'s guard against `thread_rng()` returning.
///
/// The seam cannot be enforced by types yet — a command holds `State<AppState>` and could always
/// reach past the queue and write shared state directly, which is exactly what all four used to do.
/// Until enforcement lands (G2), this is what stops one of them quietly going back to writing the
/// world without saying so.
#[test]
fn every_world_mutating_command_still_records_an_observer_action() {
    let cases = [
        ("src/commands/evolution.rs", "update_evolution_settings"),
        ("src/commands/evolution.rs", "toggle_evolution"),
        ("src/commands/networking.rs", "trigger_migration"),
        ("src/commands/networking.rs", "set_sharding_config"),
    ];

    for (path, command) in cases {
        let src = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("cannot read {path}: {e}. Has the file moved?"));
        let start = src
            .find(&format!("pub fn {command}"))
            .unwrap_or_else(|| panic!("{path} no longer defines `{command}`"));
        // The body ends where the next `#[tauri::command]` begins, or at end of file.
        let rest = &src[start..];
        let end = rest[1..]
            .find("#[tauri::command]")
            .map(|i| i + 1)
            .unwrap_or(rest.len());
        let body = &rest[..end];

        assert!(
            body.contains("observer_actions"),
            "`{command}` in {path} writes to a running world but no longer records an \
             ObserverAction. Either route it through `state.engine.observer_actions.push(..)`, or if \
             it genuinely stopped mutating the world, drop it from this list and say why in the \
             commit. ADR-0004 C3."
        );
    }
}

/// The negative control for the scan above. A read-only command must **not** be in the list, and the
/// scan must be capable of noticing an absent push — otherwise it would pass on any file at all.
#[test]
fn the_source_scan_can_actually_fail() {
    let src = std::fs::read_to_string("src/commands/evolution.rs").expect("read evolution.rs");
    let start = src
        .find("pub fn get_map_elites_grid")
        .expect("get_map_elites_grid should exist");
    let rest = &src[start..];
    let end = rest[1..]
        .find("#[tauri::command]")
        .map(|i| i + 1)
        .unwrap_or(rest.len());
    assert!(
        !rest[..end].contains("observer_actions"),
        "a read-only getter is recording an observer action, which means the scan above would pass \
         even if the mutating commands stopped recording"
    );
}

// --- provenance ----------------------------------------------------------------------------------

/// An effect chain that began with an observer action traces back to the observer, however far down
/// it is read. The ledger lives in the headless slice, so this is where C3's provenance claim is
/// provable today.
#[test]
fn effects_of_an_observer_action_trace_back_to_the_observer() {
    let mut ledger = CausalLedger::new();
    let action = ObserverAction::EvolutionSettingsChanged {
        mutation_rate: 0.9,
        selection_bias: 1.5,
        grid_resolution: 50,
    };

    let root = ledger.record(
        CAUSE_OBSERVER,
        None,
        1_200,
        "evolution.mutation_rate",
        0.9,
        0.75,
        action.mechanism(),
    );
    let drift = ledger.record(
        CAUSE_BACKGROUND,
        Some(root),
        1_400,
        "morphology_variance",
        3.1,
        2.2,
        "offspring diverged faster under the raised mutation rate",
    );

    assert_eq!(ledger.root_cause(drift), Some(CAUSE_OBSERVER));
    assert!(
        ledger
            .get(root)
            .expect("root exists")
            .mechanism
            .contains("mutation_rate=0.9"),
        "the mechanism string has to carry the value the world actually took, or the record cannot \
         be checked against the run"
    );
}

/// Negative control: a chain the observer did not start must not name them.
#[test]
fn a_chain_the_observer_did_not_start_does_not_name_them() {
    let mut ledger = CausalLedger::new();
    let seasonal = ledger.record(
        CAUSE_BACKGROUND,
        None,
        1_200,
        "precip",
        0.2,
        -0.8,
        "seasonal dry spell",
    );
    assert_eq!(ledger.root_cause(seasonal), Some(CAUSE_BACKGROUND));
    assert_ne!(ledger.root_cause(seasonal), Some(CAUSE_OBSERVER));
}

/// Every variant names the IPC command it came from, so a trace is readable against `PROJECT.md`'s
/// documented surface rather than against this enum's own naming.
#[test]
fn every_action_names_its_command() {
    let cases = [
        (settings_change(0.1), "update_evolution_settings"),
        (
            ObserverAction::EvolutionToggled { running: true },
            "toggle_evolution",
        ),
        (
            ObserverAction::MigrationTriggered { target_port: 1 },
            "trigger_migration",
        ),
        (
            ObserverAction::ShardingConfigChanged { local_port: 1 },
            "set_sharding_config",
        ),
    ];
    for (action, expected) in cases {
        assert_eq!(action.command_name(), expected);
        assert!(
            !action.mechanism().is_empty(),
            "{expected} has no mechanism string for the ledger"
        );
    }
}
