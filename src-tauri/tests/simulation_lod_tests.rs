//! Simulation LOD at the ECS level — who actually gets to think each tick.
//!
//! The unit tests in `core::simulation_lod` pin the tier arithmetic. These pin the thing that
//! matters operationally: that the gate is wired into `sensory_system`, that turning it off changes
//! nothing, and that turning it on removes work rather than removing agents.
//!
//! The failure worth guarding against is not a crash. It is a LOD that quietly does nothing — every
//! agent still thinking, the saving imaginary — or one that quietly freezes agents that should have
//! been merely cheaper.

use anima_engine_lib::ai::cpg::{CpgOscillator, TimeStep};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::{
    sensory_system, InferenceChannels, InferenceRequestBatch, InferenceResponseBatch,
};
use anima_engine_lib::core::ecs::{
    Agent, CognitiveState, InertiaComponent, ParentAgent, Position, Prey, Rotation,
    SensoryBufferComponent,
};
use anima_engine_lib::core::simulation_lod::{LodBands, LodFocus};
use bevy_ecs::prelude::*;
use crossbeam_channel::Receiver;
use glam::{Quat, Vec3};

const HOT: f32 = 10.0;
const WARM: f32 = 30.0;
const INTERVAL: u32 = 4;

struct Harness {
    world: World,
    req_rx: Receiver<InferenceRequestBatch>,
    schedule: Schedule,
    agents: Vec<Entity>,
}

/// Spawn one agent at each of the given distances along +X from the origin.
fn harness(distances: &[f32], lod: Option<(LodFocus, LodBands)>) -> Harness {
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

    if let Some((focus, bands)) = lod {
        world.insert_resource(focus);
        world.insert_resource(bands);
    }

    let mut agents = Vec::new();
    for &d in distances {
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
    schedule.add_systems(sensory_system);

    Harness {
        world,
        req_rx,
        schedule,
        agents,
    }
}

fn bands() -> LodBands {
    LodBands {
        hot_radius: HOT,
        warm_radius: WARM,
        warm_interval: INTERVAL,
    }
}

impl Harness {
    /// Run one tick and report which agents submitted an inference request.
    ///
    /// Agents are reset to `Ready` first: without a worker to answer them they would otherwise stay
    /// `PendingInference` forever and every tick after the first would look identically empty —
    /// which would make a broken LOD indistinguishable from a working one.
    fn tick(&mut self) -> Vec<Entity> {
        let ids: Vec<Entity> = self.agents.clone();
        for e in ids {
            if let Some(mut c) = self.world.get_mut::<CognitiveState>(e) {
                *c = CognitiveState::Ready;
            }
        }
        self.schedule.run(&mut self.world);

        let mut asked = Vec::new();
        while let Ok(batch) = self.req_rx.try_recv() {
            asked.extend(batch.requests.iter().map(|r| r.entity));
        }
        asked
    }

    fn ask_counts(&mut self, ticks: usize) -> Vec<usize> {
        let mut counts = vec![0usize; self.agents.len()];
        for _ in 0..ticks {
            for e in self.tick() {
                if let Some(i) = self.agents.iter().position(|&a| a == e) {
                    counts[i] += 1;
                }
            }
        }
        counts
    }
}

// --- off by default ------------------------------------------------------------------------------

#[test]
fn without_a_focus_every_agent_thinks_every_tick() {
    // The legacy path. A world that never configured LOD must behave exactly as it did before the
    // module existed — this is the baseline the rest of the file is measured against.
    let mut h = harness(&[0.0, 20.0, 200.0, 5_000.0], None);
    let counts = h.ask_counts(12);
    assert_eq!(counts, vec![12, 12, 12, 12]);
}

#[test]
fn a_disabled_focus_changes_nothing() {
    // Inserting the resources but leaving the focus off must be indistinguishable from not
    // inserting them. Otherwise merely wiring LOD in would alter every existing run.
    let mut with = harness(&[0.0, 20.0, 200.0], Some((LodFocus::default(), bands())));
    let mut without = harness(&[0.0, 20.0, 200.0], None);
    assert_eq!(with.ask_counts(12), without.ask_counts(12));
}

// --- switched on ---------------------------------------------------------------------------------

#[test]
fn cold_agents_stop_asking_entirely() {
    let mut h = harness(
        &[0.0, 100.0, 1_000.0],
        Some((LodFocus::at(Vec3::ZERO), bands())),
    );
    let counts = h.ask_counts(16);

    assert_eq!(
        counts[0], 16,
        "the agent at the focus must think every tick"
    );
    assert_eq!(counts[1], 0, "an agent past the warm radius must not think");
    assert_eq!(counts[2], 0);
}

#[test]
fn warm_agents_think_at_the_reduced_rate() {
    let ticks = 64;
    let mut h = harness(&[20.0], Some((LodFocus::at(Vec3::ZERO), bands())));
    let counts = h.ask_counts(ticks);

    let expected = ticks / INTERVAL as usize;
    assert_eq!(
        counts[0], expected,
        "a warm agent should think {expected} times in {ticks} ticks, not {}",
        counts[0]
    );
}

#[test]
fn hot_agents_are_untouched_by_the_warm_band_existing() {
    let mut h = harness(&[0.0, 5.0, HOT], Some((LodFocus::at(Vec3::ZERO), bands())));
    let counts = h.ask_counts(20);
    assert_eq!(
        counts,
        vec![20, 20, 20],
        "everything inside the hot radius, boundary included, must be unaffected"
    );
}

#[test]
fn the_focus_moves_the_detailed_region_with_it() {
    // The point of a focus. The same agent is hot or cold depending only on where attention is —
    // if this failed, LOD would just be a fixed mask centred on the map.
    let far = Vec3::new(500.0, 0.0, 0.0);
    let mut near_origin = harness(&[0.0, 500.0], Some((LodFocus::at(Vec3::ZERO), bands())));
    let mut near_far = harness(&[0.0, 500.0], Some((LodFocus::at(far), bands())));

    assert_eq!(near_origin.ask_counts(8), vec![8, 0]);
    assert_eq!(near_far.ask_counts(8), vec![0, 8]);
}

#[test]
fn a_warm_band_does_not_fire_all_at_once() {
    // The saving is only real if the work is spread. A band of agents that all think on the same
    // tick does the same total work as `Hot`, arriving as a spike the frame budget feels and the
    // average hides.
    let ring: Vec<f32> = (0..INTERVAL as usize * 4).map(|_| 20.0).collect();
    let mut h = harness(&ring, Some((LodFocus::at(Vec3::ZERO), bands())));

    let mut per_tick = Vec::new();
    for _ in 0..INTERVAL as usize {
        per_tick.push(h.tick().len());
    }

    let total: usize = per_tick.iter().sum();
    assert_eq!(total, ring.len(), "every agent should think once per sweep");
    let peak = *per_tick.iter().max().unwrap();
    assert!(
        peak <= ring.len() / INTERVAL as usize + 1,
        "one tick carried {peak} of {} warm agents; the band is not staggered",
        ring.len()
    );
}

#[test]
fn a_skipped_agent_is_cheaper_not_frozen() {
    // A cold agent keeps its last CPG parameters and its components. LOD removes the thinking, not
    // the creature — an agent that vanished or went inert would be a different bug wearing the same
    // performance win.
    let mut h = harness(&[1_000.0], Some((LodFocus::at(Vec3::ZERO), bands())));
    let agent = h.agents[0];

    if let Some(mut inertia) = h.world.get_mut::<InertiaComponent>(agent) {
        inertia.cpg_parameters = [0.7, 0.3, 0.7, 0.3];
    }
    h.ask_counts(20);

    let inertia = h.world.entity(agent).get::<InertiaComponent>().unwrap();
    assert_eq!(
        inertia.cpg_parameters,
        [0.7, 0.3, 0.7, 0.3],
        "a cold agent should coast on its last command, not be reset"
    );
    assert!(h.world.entity(agent).get::<HomeostaticState>().is_some());
    assert!(h.world.entity(agent).get::<Agent>().is_some());
}

#[test]
fn tiering_is_reproducible() {
    // LOD decisions feed the inference stream, so a non-deterministic tier assignment would make
    // runs diverge for reasons unrelated to the simulation.
    let mk = || {
        harness(
            &[0.0, 20.0, 500.0],
            Some((LodFocus::at(Vec3::ZERO), bands())),
        )
    };
    let mut a = mk();
    let mut b = mk();
    assert_eq!(a.ask_counts(32), b.ask_counts(32));
}

// ---- Wiring ----------------------------------------------------------------------------------
//
// Until the focus could be set from outside, everything above tested a subsystem the running app
// never reached: `sensory_system` took `Option<Res<LodFocus>>` and nothing ever inserted it. These
// pin the connection itself, and the two properties that make it safe to have connected.

/// The app's focus reaches the world.
#[test]
fn a_focus_written_by_the_app_reaches_the_world_on_the_next_tick() {
    use anima_engine_lib::core::simulation_lod::{sync_lod_focus_system, SharedLodFocus};

    let mut world = World::new();
    let shared = SharedLodFocus::new_disabled();
    world.insert_resource(LodFocus::default());
    world.insert_resource(shared.clone());

    let mut schedule = Schedule::default();
    schedule.add_systems(sync_lod_focus_system);

    schedule.run(&mut world);
    assert!(
        !world.resource::<LodFocus>().enabled,
        "a focus nobody has set must stay disabled"
    );

    // What `set_lod_focus` does, from the other thread.
    *shared.0.write().unwrap() = LodFocus::at(Vec3::new(12.0, 0.0, -7.0));
    schedule.run(&mut world);

    let applied = *world.resource::<LodFocus>();
    assert!(applied.enabled);
    assert_eq!(applied.center, Vec3::new(12.0, 0.0, -7.0));

    // And it goes back. A focus that could be turned on but not off would make LOD a one-way door.
    *shared.0.write().unwrap() = LodFocus::default();
    schedule.run(&mut world);
    assert!(!world.resource::<LodFocus>().enabled);
}

/// A headless world has no handle to sync from, and the sync must not invent one.
///
/// This is the property that keeps the determinism gates and the experiment runner on exactly the
/// path they had before the wiring existed: no UI, no `SharedLodFocus`, no writes to `LodFocus`.
#[test]
fn without_the_shared_handle_the_sync_leaves_the_world_alone() {
    use anima_engine_lib::core::simulation_lod::sync_lod_focus_system;

    let mut world = World::new();
    world.insert_resource(LodFocus::at(Vec3::new(3.0, 0.0, 4.0)));

    let mut schedule = Schedule::default();
    schedule.add_systems(sync_lod_focus_system);
    schedule.run(&mut world);

    assert_eq!(
        *world.resource::<LodFocus>(),
        LodFocus::at(Vec3::new(3.0, 0.0, 4.0)),
        "with no handle to read, the sync must be a no-op rather than a reset"
    );
}

/// The aggregate tier stays off unless asked for, and "unset" means off.
///
/// The failure this forbids is the one every engine switch here is shaped against: a flag whose
/// absence turns the feature *on*. Tier two destroys entities and blocks saving, so reading an
/// unset variable as "yes" would be an expensive way to find out.
#[test]
fn the_aggregate_tier_is_off_unless_the_environment_asks_for_it() {
    use anima_engine_lib::core::aggregate_population::aggregate_lod_enabled_from_env;

    const VAR: &str = "ANIMA_AGGREGATE_LOD";
    let restore = std::env::var(VAR).ok();

    std::env::remove_var(VAR);
    assert!(!aggregate_lod_enabled_from_env(), "unset must mean off");

    for off in ["", "0", "false", "False", "  0  "] {
        std::env::set_var(VAR, off);
        assert!(!aggregate_lod_enabled_from_env(), "{off:?} must mean off");
    }
    for on in ["1", "true", "yes"] {
        std::env::set_var(VAR, on);
        assert!(aggregate_lod_enabled_from_env(), "{on:?} must mean on");
    }

    match restore {
        Some(v) => std::env::set_var(VAR, v),
        None => std::env::remove_var(VAR),
    }
}

/// The wire shape of `LodFocus` is what the frontend actually sends.
///
/// `set_lod_focus` takes a `LodFocus` straight off the IPC boundary, and glam's `Vec3` serialises
/// as a three-element sequence rather than `{x,y,z}`. Nothing else in the build would notice the
/// difference: a mismatched payload fails at runtime, in a command whose only job is to write a
/// value nobody reads back, so the focus would simply never move and the tier would look broken
/// rather than mis-wired. This pins the exact JSON `WorldShowcase` emits.
#[test]
fn the_focus_deserialises_from_the_json_the_frontend_sends() {
    let sent = r#"{"enabled":true,"center":[12.5,0.0,-7.25]}"#;
    let focus: LodFocus = serde_json::from_str(sent).expect("the frontend payload must parse");
    assert!(focus.enabled);
    assert_eq!(focus.center, Vec3::new(12.5, 0.0, -7.25));

    // And the disabled form the page sends on unmount.
    let off = r#"{"enabled":false,"center":[0.0,0.0,0.0]}"#;
    let focus: LodFocus = serde_json::from_str(off).expect("the disable payload must parse");
    assert!(!focus.enabled);
}

/// `get_lod_bands` serialises the field name the frontend reads.
///
/// `focusForViewport` refuses to tier when it cannot learn the hot radius, so a renamed field does
/// not break anything loudly — it silently turns simulation LOD off in the agent viewport and
/// leaves no trace. Pinning the wire name is what makes that a compile-adjacent failure instead.
#[test]
fn the_bands_serialise_the_field_name_the_viewport_reads() {
    let json = serde_json::to_value(LodBands::default()).expect("bands must serialise");
    let radius = json
        .get("hot_radius")
        .and_then(|v| v.as_f64())
        .expect("the frontend reads `hot_radius`");
    assert_eq!(radius, LodBands::default().hot_radius as f64);
    assert!(radius > 0.0, "a zero radius would tier every agent Cold");
}
