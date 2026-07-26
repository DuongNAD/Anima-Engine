//! ADR-0004 O3 (first slice) — a recorded observer, played back.
//!
//! # What this proves, and what it deliberately does not
//!
//! It proves the **mechanism**: a trace recorded from a live camera, fed back through
//! `ObserverReplay`, reproduces the same tiering decisions the original session made — measured with
//! the same instrument O1 used, the per-tick timeline of which agent asked to think.
//!
//! It does **not** prove that a live `Inhabit` session replays bit-exactly. That is the ADR's stated
//! O3 gate and it remains blocked: physics and CPG run in parallel in the live schedule, so an
//! uninterrupted live run does not reproduce *itself* (`DETERMINISM_CONTRACT` §5), and the snapshot
//! gate says the same of its own scope (`SNAPSHOT_CONTRACT` §8). Like those gates, this one declares
//! its own schedule order and pins the subsystem, not the engine. ADR-0004 asks that replay not be
//! claimed before G2; this file is careful to claim the smaller thing.

use anima_engine_lib::ai::cpg::{CpgOscillator, TimeStep};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::{
    sensory_system, InferenceChannels, InferenceRequestBatch, InferenceResponseBatch,
};
use anima_engine_lib::core::ecs::{
    Agent, CognitiveState, InertiaComponent, ParentAgent, Position, Prey, Rotation,
    SensoryBufferComponent,
};
use anima_engine_lib::core::observer::{
    record_observer_trace_system, ObserverPolicy, ObserverReplay, ObserverSample, ObserverTrace,
};
use anima_engine_lib::core::simulation_lod::{
    sync_lod_focus_system, LodBands, LodFocus, SharedLodFocus,
};
use bevy_ecs::prelude::*;
use crossbeam_channel::Receiver;
use glam::{Quat, Vec3};

const DISTANCES: [f32; 5] = [0.0, 15.0, 40.0, 80.0, 150.0];
const TICKS: usize = 40;

fn bands() -> LodBands {
    LodBands {
        hot_radius: 10.0,
        warm_radius: 30.0,
        warm_interval: 4,
    }
}

/// The observer's path during the recorded session. Crosses every band so the tiering it produces
/// is varied enough for a mismatch to show.
fn camera_at(tick: usize) -> LodFocus {
    LodFocus::at(Vec3::new(tick as f32 * 4.0, 0.0, 0.0))
}

struct Harness {
    world: World,
    req_rx: Receiver<InferenceRequestBatch>,
    schedule: Schedule,
    agents: Vec<Entity>,
    shared: SharedLodFocus,
}

fn harness(replay: Option<ObserverReplay>, record: bool) -> Harness {
    let mut world = World::new();
    world.insert_resource(TimeStep(1.0 / 60.0));

    let (req_tx, req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
    let (recycle_req_tx, recycle_req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
    let (_res_tx, res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();
    let (recycle_res_tx, _recycle_res_rx) =
        crossbeam_channel::unbounded::<InferenceResponseBatch>();
    for _ in 0..64 {
        let _ = recycle_req_tx.send(InferenceRequestBatch {
            requests: Vec::with_capacity(64),
        });
    }
    world.insert_resource(InferenceChannels {
        req_tx,
        recycle_req_rx,
        res_rx,
        recycle_res_tx,
    });

    let shared = SharedLodFocus::new_disabled();
    world.insert_resource(shared.clone());
    world.insert_resource(LodFocus::default());
    world.insert_resource(bands());
    world.insert_resource(ObserverPolicy::Inhabit { cause_id: 7 });
    if let Some(replay) = replay {
        world.insert_resource(replay);
    }
    if record {
        world.insert_resource(ObserverTrace::with_capacity(1024));
    }

    let mut agents = Vec::new();
    for &d in DISTANCES.iter() {
        let e = world
            .spawn((
                Agent,
                Prey,
                Position(Vec3::new(d, 0.0, 0.0)),
                Rotation(Quat::IDENTITY),
                HomeostaticState {
                    energy: 80.0,
                    energy_target: 100.0,
                    hydration: 80.0,
                    hydration_target: 100.0,
                    temperature: 37.0,
                    temp_target: 37.0,
                    previous_deviation: 0.0,
                },
                CognitiveState::Ready,
                InertiaComponent::default(),
                SensoryBufferComponent::default(),
                CpgOscillator::new(1.0, 0.5),
            ))
            .id();
        world.entity_mut(e).insert(ParentAgent(e));
        agents.push(e);
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            sync_lod_focus_system,
            record_observer_trace_system,
            sensory_system,
        )
            .chain(),
    );

    Harness {
        world,
        req_rx,
        schedule,
        agents,
        shared,
    }
}

impl Harness {
    /// Run the session, optionally driving a live camera, and report the per-tick think timeline.
    fn run(&mut self, drive_camera: Option<fn(usize) -> LodFocus>) -> Vec<Vec<usize>> {
        let mut timeline = Vec::with_capacity(TICKS);
        for tick in 0..TICKS {
            if let Some(path) = drive_camera {
                if let Ok(mut guard) = self.shared.0.write() {
                    *guard = path(tick);
                }
            }
            for e in self.agents.clone() {
                if let Some(mut c) = self.world.get_mut::<CognitiveState>(e) {
                    *c = CognitiveState::Ready;
                }
            }
            self.schedule.run(&mut self.world);

            let mut asked = Vec::new();
            while let Ok(batch) = self.req_rx.try_recv() {
                for r in batch.requests.iter() {
                    if let Some(i) = self.agents.iter().position(|&a| a == r.entity) {
                        asked.push(i);
                    }
                }
            }
            asked.sort_unstable();
            timeline.push(asked);
        }
        timeline
    }

    fn take_trace(&mut self) -> ObserverTrace {
        self.world
            .remove_resource::<ObserverTrace>()
            .expect("trace")
    }
}

/// Run the original session and hand back what it did and what it recorded.
fn record_a_session() -> (Vec<Vec<usize>>, ObserverTrace) {
    let mut h = harness(None, true);
    let timeline = h.run(Some(camera_at));
    let trace = h.take_trace();
    (timeline, trace)
}

// --- the gate ------------------------------------------------------------------------------------

/// **The O3 slice gate.** Replaying the trace reproduces the tiering the live camera produced.
#[test]
fn a_replayed_trace_reproduces_the_session_it_recorded() {
    let (recorded, trace) = record_a_session();
    assert!(
        !trace.is_empty() && !trace.is_truncated(),
        "the recording is the input to this test; a truncated or empty one makes it vacuous"
    );

    // No camera at all this time — the trace is the only source.
    let replayed = harness(Some(ObserverReplay::from_trace(&trace)), false).run(None);

    assert_eq!(
        recorded, replayed,
        "replaying the trace produced different tiering than the session it came from"
    );
}

/// **Negative control.** A different trace must produce a different session, or the test above is
/// only proving that both runs ignored the trace entirely.
#[test]
fn replaying_a_different_trace_produces_a_different_session() {
    let (recorded, _) = record_a_session();

    // An observer who never moved from the origin: everything near it stays Hot, everything far
    // stays Cold, for the whole run.
    let still = ObserverReplay::from_samples(vec![ObserverSample {
        tick: 1,
        focus: LodFocus::at(Vec3::ZERO),
    }]);
    let other = harness(Some(still), false).run(None);

    assert_ne!(
        recorded, other,
        "a completely different observer path produced an identical session — the replay is not \
         reaching the world and the gate above proves nothing"
    );
}

/// **Replay excludes the live camera.** A `set_lod_focus` arriving mid-replay — a UI nobody
/// remembered to close — must not steer the run while the trace is being credited for it.
#[test]
fn a_live_camera_cannot_steer_a_replay() {
    let (recorded, trace) = record_a_session();

    // Drive a hostile camera the whole way: the exact inverse of the recorded path.
    fn hostile(tick: usize) -> LodFocus {
        LodFocus::at(Vec3::new(600.0 - tick as f32 * 4.0, 0.0, 0.0))
    }
    let replayed = harness(Some(ObserverReplay::from_trace(&trace)), false).run(Some(hostile));

    assert_eq!(
        recorded, replayed,
        "a live camera changed the outcome of a replay — the run was steered by something the \
         trace does not account for"
    );
}

// --- playback semantics --------------------------------------------------------------------------

/// The declared interpolation: the focus **holds** between recorded samples. Recording only stores a
/// change, so holding reconstructs the original signal exactly rather than approximating it.
#[test]
fn the_focus_holds_between_recorded_samples() {
    let mut replay = ObserverReplay::from_samples(vec![
        ObserverSample {
            tick: 2,
            focus: LodFocus::at(Vec3::new(1.0, 0.0, 0.0)),
        },
        ObserverSample {
            tick: 5,
            focus: LodFocus::at(Vec3::new(9.0, 0.0, 0.0)),
        },
    ]);

    assert_eq!(
        replay.focus_at(1),
        LodFocus::default(),
        "before the first sample the world has been told nothing"
    );
    assert_eq!(replay.focus_at(2).center.x, 1.0);
    assert_eq!(replay.focus_at(3).center.x, 1.0, "held, not interpolated");
    assert_eq!(replay.focus_at(4).center.x, 1.0);
    assert_eq!(replay.focus_at(5).center.x, 9.0);
}

/// Past the end the last focus keeps holding — the recording stopped because the camera stopped
/// changing, not because it vanished.
#[test]
fn playback_past_the_end_holds_the_last_focus() {
    let mut replay = ObserverReplay::from_samples(vec![ObserverSample {
        tick: 1,
        focus: LodFocus::at(Vec3::new(4.0, 0.0, 0.0)),
    }]);
    replay.focus_at(1);
    assert!(replay.is_exhausted());
    assert_eq!(replay.focus_at(9_999).center.x, 4.0);
}

/// A replay of an empty trace is a world that was never told anything, not a panic.
#[test]
fn an_empty_replay_leaves_the_world_untouched() {
    let mut replay = ObserverReplay::from_samples(vec![]);
    assert!(replay.is_exhausted());
    assert_eq!(replay.remaining(), 0);
    assert_eq!(replay.focus_at(1), LodFocus::default());
}
