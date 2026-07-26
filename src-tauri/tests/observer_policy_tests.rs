//! ADR-0004 O1 — the observer policy, at the seam where a camera actually reaches the world.
//!
//! `simulation_lod_tests.rs` proves the tiering machinery works. This file proves the *policy* over
//! it: that a declared `Spectate` run is trajectory-identical to a headless one even while a camera
//! moves through the world, and — the half that makes the first half mean anything — that the same
//! camera path under `Inhabit` genuinely changes who thinks.
//!
//! Without that negative control, `spectate_matches_absent` would pass just as happily if the camera
//! path were inert, the harness were misassembled, or the focus never reached the world at all. A
//! gate that cannot fail is not a gate. This is the same discipline `DETERMINISM_CONTRACT.md` §4
//! requires of the determinism gate, for the same reason.

use anima_engine_lib::ai::cpg::{CpgOscillator, TimeStep};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::{
    sensory_system, InferenceChannels, InferenceRequestBatch, InferenceResponseBatch,
};
use anima_engine_lib::core::ecs::{
    Agent, CognitiveState, InertiaComponent, ParentAgent, Position, Prey, Rotation,
    SensoryBufferComponent,
};
use anima_engine_lib::core::observer::ObserverPolicy;
use anima_engine_lib::core::simulation_lod::{
    sync_lod_focus_system, LodBands, LodFocus, SharedLodFocus,
};
use bevy_ecs::prelude::*;
use crossbeam_channel::Receiver;
use glam::{Quat, Vec3};

const HOT: f32 = 10.0;
const WARM: f32 = 30.0;
const INTERVAL: u32 = 4;

/// Agents spread from inside the hot radius to well beyond the warm one, so a camera walking past
/// them crosses every tier boundary rather than nudging one.
const DISTANCES: [f32; 5] = [0.0, 15.0, 40.0, 80.0, 150.0];
const TICKS: usize = 40;

fn bands() -> LodBands {
    LodBands {
        hot_radius: HOT,
        warm_radius: WARM,
        warm_interval: INTERVAL,
    }
}

struct Harness {
    world: World,
    req_rx: Receiver<InferenceRequestBatch>,
    schedule: Schedule,
    agents: Vec<Entity>,
    shared: SharedLodFocus,
}

/// A world with the LOD focus wired the way the app wires it: the camera writes [`SharedLodFocus`],
/// and [`sync_lod_focus_system`] copies it in before anything tiers on it.
fn harness(policy: Option<ObserverPolicy>) -> Harness {
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
    if let Some(policy) = policy {
        world.insert_resource(policy);
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
    schedule.add_systems((sync_lod_focus_system, sensory_system).chain());

    Harness {
        world,
        req_rx,
        schedule,
        agents,
        shared,
    }
}

impl Harness {
    /// Where the observer stands on `tick` — a camera walking out along +X across every band.
    fn camera_at(tick: usize) -> LodFocus {
        LodFocus::at(Vec3::new(tick as f32 * 4.0, 0.0, 0.0))
    }

    /// Walk the camera through the world and record, per tick, exactly which agents asked to think.
    ///
    /// The full per-tick sequence rather than a total: two policies could coincidentally sum to the
    /// same number of inferences while thinking on different ticks, and "same trajectory" means the
    /// same agents thinking at the same moments.
    fn run_camera_path(&mut self) -> Vec<Vec<usize>> {
        let mut timeline = Vec::with_capacity(TICKS);
        for tick in 0..TICKS {
            if let Ok(mut guard) = self.shared.0.write() {
                *guard = Self::camera_at(tick);
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
}

fn timeline_for(policy: Option<ObserverPolicy>) -> Vec<Vec<usize>> {
    harness(policy).run_camera_path()
}

// --- the gate ------------------------------------------------------------------------------------

/// **The O1 gate.** A camera may exist and move; under `Spectate` the world must not notice.
#[test]
fn spectate_matches_absent() {
    let absent = timeline_for(Some(ObserverPolicy::Absent));
    let spectate = timeline_for(Some(ObserverPolicy::Spectate));

    assert_eq!(
        absent, spectate,
        "a Spectate run diverged from a headless one — the observer perturbed the world it \
         promised only to watch"
    );
}

/// **Negative control for the gate above**, and not optional.
///
/// The same camera path, declared as `Inhabit`, must change who thinks. If this ever passes by
/// asserting equality instead, `spectate_matches_absent` has stopped proving anything: it would be
/// comparing two runs in which the camera was never consulted at all.
#[test]
fn an_inhabited_camera_actually_changes_who_thinks() {
    let absent = timeline_for(Some(ObserverPolicy::Absent));
    let inhabit = timeline_for(Some(ObserverPolicy::Inhabit { cause_id: 7 }));

    assert_ne!(
        absent, inhabit,
        "the camera path made no difference even under Inhabit — the harness is not exercising \
         the focus, so spectate_matches_absent proves nothing"
    );
}

/// Every agent stays `Hot` under `Spectate`, however far from the camera it is.
///
/// The mechanism behind the gate, asserted directly: this is the cost of the promise, and stating it
/// here means a future change that silently starts tiering under `Spectate` fails with a readable
/// reason rather than as a timeline mismatch.
#[test]
fn spectate_leaves_every_agent_thinking_every_tick() {
    let timeline = timeline_for(Some(ObserverPolicy::Spectate));
    let all: Vec<usize> = (0..DISTANCES.len()).collect();
    for (tick, asked) in timeline.iter().enumerate() {
        assert_eq!(
            asked, &all,
            "tick {tick}: Spectate dropped an agent — tiering is reaching the world"
        );
    }
}

/// A `Cold` agent under `Inhabit` really does stop asking, so the saving being declared is real.
#[test]
fn inhabit_actually_tiers_distant_agents_out() {
    let timeline = timeline_for(Some(ObserverPolicy::Inhabit { cause_id: 7 }));
    let last = DISTANCES.len() - 1;
    let asks_from_the_far_agent = timeline.iter().filter(|t| t.contains(&last)).count();
    assert!(
        asks_from_the_far_agent < TICKS,
        "the agent at {} units thought on every one of {TICKS} ticks — Inhabit is not tiering",
        DISTANCES[last]
    );
}

// --- the compatibility edge ----------------------------------------------------------------------

/// **Unset is not `Absent`.**
///
/// No policy resource means nobody declared one, and the engine must behave exactly as it did before
/// ADR-0004 — obeying the camera. `PixiViewport.tsx` has been driving `set_lod_focus` since
/// simulation LOD was wired up, and if "undeclared" denied the focus this module would have switched
/// that off and called it a safety improvement.
#[test]
fn an_undeclared_policy_still_obeys_the_camera() {
    let undeclared = timeline_for(None);
    let inhabit = timeline_for(Some(ObserverPolicy::Inhabit { cause_id: 7 }));
    assert_eq!(
        undeclared, inhabit,
        "an undeclared policy stopped obeying the camera — this silently disables the LOD saving \
         the live app already relies on"
    );
}

/// `Absent` and `Spectate` differ in what they declare, never in what the world does.
#[test]
fn absent_and_spectate_are_behaviourally_indistinguishable() {
    assert_eq!(
        ObserverPolicy::Absent.allows_focus(),
        ObserverPolicy::Spectate.allows_focus()
    );
    assert!(ObserverPolicy::Absent.is_comparable_to_headless());
    assert!(ObserverPolicy::Spectate.is_comparable_to_headless());
}

/// An `Inhabit` run is a different treatment, and says so — the guard against a UI presenting it
/// beside a headless run as two repeats of one experiment.
#[test]
fn an_inhabited_run_does_not_claim_to_be_comparable_to_headless() {
    assert!(!ObserverPolicy::Inhabit { cause_id: 7 }.is_comparable_to_headless());
}
