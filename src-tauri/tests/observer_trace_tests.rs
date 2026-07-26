//! ADR-0004 O2 — recording the observer, and rooting what follows at them.
//!
//! O1 made the observer's relationship to the world *declared*. O2 makes it *evidence*: the trace
//! records what the world was actually subjected to, and `CAUSE_OBSERVER` gives the consequences a
//! root that is not "the world did this by itself".
//!
//! Replay is deliberately not in scope — that is O3 and it waits on the live engine becoming
//! deterministic (`DETERMINISM_CONTRACT` §5). "Why did that herd die out" is answerable long before
//! "run it again exactly" is, and these tests pin the first without implying the second.

use anima_domain::causal::{CausalLedger, CAUSE_BACKGROUND, CAUSE_OBSERVER};
use anima_engine_lib::core::observer::{
    record_observer_trace_system, ObserverPolicy, ObserverTrace,
};
use anima_engine_lib::core::simulation_lod::{sync_lod_focus_system, LodFocus, SharedLodFocus};
use bevy_ecs::prelude::*;
use glam::Vec3;

const TICKS: u64 = 12;

struct Harness {
    world: World,
    schedule: Schedule,
    shared: SharedLodFocus,
}

/// The live wiring in miniature: the camera writes the shared focus, the policy filters it, and the
/// recorder sees only what survived.
fn harness(policy: ObserverPolicy) -> Harness {
    let mut world = World::new();
    let shared = SharedLodFocus::new_disabled();
    world.insert_resource(shared.clone());
    world.insert_resource(LodFocus::default());
    world.insert_resource(policy);
    world.insert_resource(ObserverTrace::with_capacity(1024));

    let mut schedule = Schedule::default();
    schedule.add_systems((sync_lod_focus_system, record_observer_trace_system).chain());

    Harness {
        world,
        schedule,
        shared,
    }
}

impl Harness {
    /// Walk the camera one unit per tick and return the trace that resulted.
    fn walk_the_camera(mut self) -> ObserverTrace {
        for tick in 0..TICKS {
            if let Ok(mut guard) = self.shared.0.write() {
                *guard = LodFocus::at(Vec3::new(tick as f32, 0.0, 0.0));
            }
            self.schedule.run(&mut self.world);
        }
        self.world
            .remove_resource::<ObserverTrace>()
            .expect("trace")
    }
}

// --- what the world was subjected to -------------------------------------------------------------

/// Under `Inhabit` the camera reaches the world, so the walk is on the record.
#[test]
fn an_inhabited_camera_leaves_a_trace() {
    let trace = harness(ObserverPolicy::Inhabit {
        cause_id: CAUSE_OBSERVER,
    })
    .walk_the_camera();

    assert_eq!(
        trace.len(),
        TICKS as usize,
        "the camera moved on every tick and every move should be recorded"
    );
    assert!(!trace.is_truncated());
    assert!(
        trace.samples().iter().all(|s| s.focus.enabled),
        "an Inhabit trace should record an enabled focus — the thing the world actually tiered on"
    );
}

/// **The counterpart to `spectate_matches_absent`.** A `Spectate` run's camera moves just as far,
/// and the trace stays empty of movement — because the trace records what the *world* saw, not
/// where a human looked. A trace full of camera positions under `Spectate` would be evidence of a
/// perturbation that policy's entire promise is that it did not commit.
#[test]
fn a_spectating_camera_leaves_nothing_for_the_world_to_answer_for() {
    let trace = harness(ObserverPolicy::Spectate).walk_the_camera();

    assert!(
        trace
            .samples()
            .iter()
            .all(|s| s.focus == LodFocus::default()),
        "Spectate let the observer's position into the world. Merely clearing `enabled` is not \
         enough: a centre left behind is a live camera path any later reader could pick up, and it \
         fills this trace with movement the world never felt"
    );
    assert_eq!(
        trace.len(),
        1,
        "Spectate should settle into exactly one baseline sample; {} means the world kept \
         noticing a camera it promised to ignore",
        trace.len()
    );
}

/// Absent behaves as Spectate here for the same reason: neither lets the focus through.
#[test]
fn an_absent_observer_records_no_movement_either() {
    let trace = harness(ObserverPolicy::Absent).walk_the_camera();
    assert!(trace.samples().iter().all(|s| !s.focus.enabled));
}

/// Inert without the resource, like every other part of this subsystem. A run that never installed
/// a trace behaves exactly as it did before ADR-0004 rather than panicking on a missing resource.
#[test]
fn without_a_trace_resource_the_recorder_does_nothing() {
    let mut world = World::new();
    world.insert_resource(LodFocus::at(Vec3::X));
    let mut schedule = Schedule::default();
    schedule.add_systems(record_observer_trace_system);
    schedule.run(&mut world);
    assert!(world.get_resource::<ObserverTrace>().is_none());
}

// --- provenance ----------------------------------------------------------------------------------

/// **The O2 provenance gate.** A chain of consequences that began with the observer must trace back
/// to the observer, however far down it is read.
///
/// The ledger is the machinery that answers "why did this happen", and `CausalLedger::record`
/// already propagates a parent's cause down a chain. What O2 adds is a root that *means* a human:
/// before `CAUSE_OBSERVER` existed the only honest answer was `CAUSE_BACKGROUND`, which says the
/// world did it by itself — the specific lie ADR-0004 exists to stop telling.
#[test]
fn effects_downstream_of_the_observer_trace_back_to_the_observer() {
    let mut ledger = CausalLedger::new();

    let disturbance = ledger.record(
        CAUSE_OBSERVER,
        None,
        40_231,
        "herd_flight",
        1.0,
        1.0,
        "an observer walked through the herd",
    );
    let dispersal = ledger.record(
        CAUSE_BACKGROUND,
        Some(disturbance),
        40_232,
        "herbivores@cell(3,4)",
        0.0,
        -12.0,
        "the herd dispersed off the patch",
    );
    let regrowth = ledger.record(
        CAUSE_BACKGROUND,
        Some(dispersal),
        40_260,
        "npp@cell(3,4)",
        18.0,
        6.0,
        "grazing pressure lifted and the patch regrew",
    );

    assert_eq!(
        ledger.root_cause(regrowth),
        Some(CAUSE_OBSERVER),
        "three links downstream, the regrowth still has to name the human who caused it"
    );
    assert_eq!(
        ledger.trace_to_root(regrowth),
        vec![regrowth, dispersal, disturbance],
        "the chain itself must be walkable, not just its root"
    );
}

/// **Negative control for the gate above.** The same shape of chain, rooted at background dynamics,
/// must *not* come back naming the observer. Without this, the assertion above would pass just as
/// well if `root_cause` returned `CAUSE_OBSERVER` unconditionally.
#[test]
fn a_chain_the_observer_did_not_start_does_not_name_them() {
    let mut ledger = CausalLedger::new();
    let drought = ledger.record(
        CAUSE_BACKGROUND,
        None,
        40_231,
        "precip",
        0.2,
        -0.8,
        "seasonal dry spell",
    );
    let dieback = ledger.record(
        CAUSE_BACKGROUND,
        Some(drought),
        40_260,
        "npp@cell(3,4)",
        4.0,
        -14.0,
        "the patch dried out",
    );

    assert_eq!(ledger.root_cause(dieback), Some(CAUSE_BACKGROUND));
    assert_ne!(
        ledger.root_cause(dieback),
        Some(CAUSE_OBSERVER),
        "background dynamics were attributed to a human — provenance is reporting a lie"
    );
}
