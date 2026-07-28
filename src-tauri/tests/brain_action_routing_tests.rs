//! Step 5b of ADR-0003 — an agent's own brain actually reaches its behaviour.
//!
//! Until now `AgentBrain` was inert: it was inherited, saved and migrated, but nothing read it. This
//! covers the two ends of the wire that make it live — `sensory_system` attaching the genome to the
//! inference request, and `action_resolution_system` routing the widened action vector into
//! locomotion and the ecological gates.
//!
//! The worker thread in between is exercised indirectly: these tests feed it the requests it would
//! receive and check the responses it would produce are interpreted correctly. Its arithmetic is
//! already pinned by the EB-S02 parity gate.

use anima_engine_lib::ai::cpg::{CpgOscillator, TimeStep};
use anima_engine_lib::core::agent_systems::{
    action_resolution_system, sensory_system, AgentInferenceResponse, InferenceChannels,
    InferenceRequestBatch, InferenceResponseBatch, ACTION_SLOTS,
};
use anima_engine_lib::core::components::{ActionGates, AgentBrain};
use anima_engine_lib::core::ecs::{
    Agent, CognitiveState, InertiaComponent, ParentAgent, Position, Rotation,
    SensoryBufferComponent,
};
use anima_engine_lib::evolution::brain_genotype::{action_index, BrainGenotype, EVOLVED_ARCH};
use bevy_ecs::prelude::*;
use crossbeam_channel::{Receiver, Sender};
use glam::{Quat, Vec3};
use rand::rngs::StdRng;
use rand::SeedableRng;

struct Harness {
    world: World,
    req_rx: Receiver<InferenceRequestBatch>,
    res_tx: Sender<InferenceResponseBatch>,
    recycle_res_rx: Receiver<InferenceResponseBatch>,
    /// Kept so a test can put a batch back. Without a live sender the pool would also read as
    /// *disconnected* once drained, which is a different condition from *empty* and would let a
    /// test pass for the wrong reason.
    recycle_req_tx: Sender<InferenceRequestBatch>,
}

fn harness() -> Harness {
    let mut world = anima_engine_lib::core::ecs::init_world();
    world.insert_resource(TimeStep(1.0 / 60.0));

    let (req_tx, req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
    let (recycle_req_tx, recycle_req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
    let (res_tx, res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();
    let (recycle_res_tx, recycle_res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();

    for _ in 0..8 {
        let _ = recycle_req_tx.send(InferenceRequestBatch {
            requests: Vec::with_capacity(32),
        });
        let _ = recycle_res_tx.send(InferenceResponseBatch {
            responses: Vec::with_capacity(32),
        });
    }

    world.insert_resource(InferenceChannels {
        req_tx,
        recycle_req_rx,
        res_rx,
        recycle_res_tx,
    });

    Harness {
        world,
        req_rx,
        res_tx,
        recycle_res_rx,
        recycle_req_tx,
    }
}

fn spawn_agent(world: &mut World, brain: Option<AgentBrain>) -> Entity {
    let entity = world
        .spawn((
            Agent,
            Position(Vec3::ZERO),
            Rotation(Quat::IDENTITY),
            anima_engine_lib::ai::hrrl::HomeostaticState {
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
            ActionGates::default(),
            CpgOscillator::new(1.0, 0.5),
        ))
        .id();
    world.entity_mut(entity).insert(ParentAgent(entity));
    if let Some(b) = brain {
        world.entity_mut(entity).insert(b);
    }
    entity
}

fn evolved_brain(seed: u64) -> AgentBrain {
    let mut rng = StdRng::seed_from_u64(seed);
    AgentBrain::from_genotype(BrainGenotype::random(EVOLVED_ARCH, &mut rng).unwrap())
}

// --- request side -------------------------------------------------------------------------------

#[test]
fn an_evolved_agent_sends_its_own_brain_with_the_request() {
    let mut h = harness();
    let brain = evolved_brain(3);
    let agent = spawn_agent(&mut h.world, Some(brain.clone()));

    let mut schedule = Schedule::default();
    schedule.add_systems(sensory_system);
    schedule.run(&mut h.world);

    let batch = h.req_rx.try_recv().expect("a request must be sent");
    let req = batch
        .requests
        .iter()
        .find(|r| r.entity == agent)
        .expect("the agent must be in the batch");

    let carried = req
        .brain
        .as_ref()
        .expect("an evolved agent carries a brain");
    assert_eq!(**carried, *brain.genotype);
}

#[test]
fn an_evolved_agent_shares_the_genome_rather_than_copying_it() {
    // The zero-allocation rule is the reason the genome sits behind an `Arc`: if the request cloned
    // the weight vector, every agent would allocate ~23 KiB every tick. Pointer identity is what
    // proves no copy happened — a value comparison would pass either way.
    let mut h = harness();
    let brain = evolved_brain(4);
    let agent = spawn_agent(&mut h.world, Some(brain.clone()));

    let mut schedule = Schedule::default();
    schedule.add_systems(sensory_system);
    schedule.run(&mut h.world);

    let batch = h.req_rx.try_recv().unwrap();
    let req = batch.requests.iter().find(|r| r.entity == agent).unwrap();
    assert!(std::sync::Arc::ptr_eq(
        req.brain.as_ref().unwrap(),
        &brain.genotype
    ));
}

#[test]
fn a_legacy_agent_sends_no_brain() {
    let mut h = harness();
    let agent = spawn_agent(&mut h.world, None);

    let mut schedule = Schedule::default();
    schedule.add_systems(sensory_system);
    schedule.run(&mut h.world);

    let batch = h.req_rx.try_recv().unwrap();
    let req = batch.requests.iter().find(|r| r.entity == agent).unwrap();
    assert!(
        req.brain.is_none(),
        "a legacy agent must keep routing through the shared model"
    );
}

// --- response side ------------------------------------------------------------------------------

/// Push one response for `agent` and run the resolution system.
fn resolve(h: &mut Harness, agent: Entity, actions: [f32; ACTION_SLOTS]) {
    let request_id = match *h.world.get::<CognitiveState>(agent).unwrap() {
        CognitiveState::PendingInference(id) => id,
        other => panic!("agent must be awaiting inference, was {other:?}"),
    };

    let mut batch = h.recycle_res_rx.try_recv().unwrap();
    batch.responses.clear();
    batch.responses.push(AgentInferenceResponse {
        entity: agent,
        actions,
        request_id,
    });
    h.res_tx.send(batch).unwrap();

    let mut schedule = Schedule::default();
    schedule.add_systems(action_resolution_system);
    schedule.run(&mut h.world);
}

fn pend(h: &mut Harness) {
    let mut schedule = Schedule::default();
    schedule.add_systems(sensory_system);
    schedule.run(&mut h.world);
}

#[test]
fn gate_outputs_reach_the_action_gates() {
    let mut h = harness();
    let agent = spawn_agent(&mut h.world, Some(evolved_brain(5)));
    pend(&mut h);

    let mut actions = [0.0f32; ACTION_SLOTS];
    actions[..action_index::CPG_LEN].copy_from_slice(&[0.5, 0.8, 0.5, 0.8]);
    actions[action_index::PHEROMONE_EMIT] = 0.25;
    actions[action_index::ATTACK_INTENT] = 0.9;
    actions[action_index::FEED_INTENT] = 0.1;
    resolve(&mut h, agent, actions);

    let gates = h.world.get::<ActionGates>(agent).copied().unwrap();
    assert_eq!(gates.pheromone_emit, 0.25);
    assert_eq!(gates.attack_intent, 0.9);
    assert_eq!(gates.feed_intent, 0.1);
    // The brain has decided to hunt but not to eat — a distinction that was impossible to express
    // before this step, because there was no channel for it.
    assert!(gates.attacks());
    assert!(!gates.feeds());
}

#[test]
fn locomotion_still_comes_from_the_first_four_outputs() {
    let mut h = harness();
    let agent = spawn_agent(&mut h.world, Some(evolved_brain(6)));
    pend(&mut h);

    let mut actions = [0.0f32; ACTION_SLOTS];
    actions[..action_index::CPG_LEN].copy_from_slice(&[0.1, 0.2, 0.3, 0.4]);
    actions[action_index::PHEROMONE_EMIT] = 0.0;
    resolve(&mut h, agent, actions);

    assert_eq!(
        h.world
            .get::<InertiaComponent>(agent)
            .unwrap()
            .cpg_parameters,
        [0.1, 0.2, 0.3, 0.4],
        "widening the action vector must not shift the CPG block"
    );
    assert!(matches!(
        *h.world.get::<CognitiveState>(agent).unwrap(),
        CognitiveState::Ready
    ));
}

#[test]
fn a_shared_model_response_leaves_the_gates_open() {
    // The legacy path fills the gate slots with the fully-open default, so resolving a shared-model
    // response must not close anything. This is what keeps EB-S05 true now that gates are written.
    let mut h = harness();
    let agent = spawn_agent(&mut h.world, None);
    pend(&mut h);

    let mut actions = AgentInferenceResponse::open_gates_default();
    actions[..action_index::CPG_LEN].copy_from_slice(&[0.5, 0.8, 0.5, 0.8]);
    resolve(&mut h, agent, actions);

    assert_eq!(
        h.world.get::<ActionGates>(agent).copied().unwrap(),
        ActionGates::OPEN
    );
}

#[test]
fn the_fallback_action_vector_opens_gates_rather_than_shutting_them() {
    // A brain that fails to run, or a response with no shared-model result, must degrade to "carry
    // on as before". An all-zero vector would read as every gate shut — an agent that silently
    // stops eating and starves for reasons unrelated to selection.
    let fallback = AgentInferenceResponse::open_gates_default();

    assert_eq!(
        &fallback[..action_index::CPG_LEN],
        &[0.0; action_index::CPG_LEN]
    );
    assert_eq!(fallback[action_index::PHEROMONE_EMIT], 1.0);
    assert_eq!(fallback[action_index::ATTACK_INTENT], 1.0);
    assert_eq!(fallback[action_index::FEED_INTENT], 1.0);

    let gates = ActionGates {
        pheromone_emit: fallback[action_index::PHEROMONE_EMIT],
        attack_intent: fallback[action_index::ATTACK_INTENT],
        feed_intent: fallback[action_index::FEED_INTENT],
    };
    assert_eq!(gates, ActionGates::OPEN);
}

#[test]
fn a_legacy_agent_without_gates_still_resolves() {
    // Gates are `Option` in the query, so an agent spawned before they existed must not panic or be
    // skipped when a response arrives.
    let mut h = harness();
    let agent = spawn_agent(&mut h.world, None);
    h.world.entity_mut(agent).remove::<ActionGates>();
    pend(&mut h);

    let mut actions = AgentInferenceResponse::open_gates_default();
    actions[..action_index::CPG_LEN].copy_from_slice(&[0.2, 0.3, 0.2, 0.3]);
    resolve(&mut h, agent, actions);

    assert_eq!(
        h.world
            .get::<InertiaComponent>(agent)
            .unwrap()
            .cpg_parameters,
        [0.2, 0.3, 0.2, 0.3]
    );
    assert!(h.world.get::<ActionGates>(agent).is_none());
}

#[test]
fn two_brains_produce_different_gate_decisions() {
    // The point of the whole ADR, checked end to end on the brain itself: two agents with different
    // genomes, given identical senses, must be able to decide differently. Under the shared model
    // this assertion could never pass.
    let sensory = [0.4f32; 15];
    let mut scratch_a = Vec::new();
    let mut scratch_b = Vec::new();
    let mut out_a = [0.0f32; ACTION_SLOTS];
    let mut out_b = [0.0f32; ACTION_SLOTS];

    let a = evolved_brain(11);
    let b = evolved_brain(12);
    scratch_a.resize(a.genotype.scratch_len(), 0.0);
    scratch_b.resize(b.genotype.scratch_len(), 0.0);

    a.genotype
        .forward_into(&sensory, &mut scratch_a, &mut out_a)
        .unwrap();
    b.genotype
        .forward_into(&sensory, &mut scratch_b, &mut out_b)
        .unwrap();

    assert_ne!(
        out_a, out_b,
        "different genomes must be able to act differently on identical input"
    );
}

// --- the pool is a ceiling, not a starting size --------------------------------------------------

/// The inference recycle pool bounds how much memory the inference path can ever hold, and until
/// 2026-07-28 nothing enforced it: an empty pool made `sensory_system` allocate a fresh batch, which
/// was then recycled into the pool for good. A tick loop outrunning the inference worker grew the
/// pool by one batch per tick, forever — measured headless at 8.5 MB/min with ten agents, and in the
/// desktop app at 14 MB/min, which is 19 GB after a day.
///
/// This asserts the invariant rather than a memory figure. A byte threshold would depend on the
/// allocator and the machine's mood; "the pool never grows" does not.
#[test]
fn an_empty_recycle_pool_skips_the_tick_instead_of_allocating_another_batch() {
    let mut h = harness();
    spawn_agent(&mut h.world, None);

    let mut schedule = Schedule::default();
    schedule.add_systems(sensory_system);

    // Drain the pool the way a lagging inference worker does: take every batch and hold it.
    let pool = h.world.resource::<InferenceChannels>().clone();
    let mut held = Vec::new();
    while let Ok(batch) = pool.recycle_req_rx.try_recv() {
        held.push(batch);
    }
    assert!(!held.is_empty(), "the harness pre-fills the pool");

    // Many ticks against an empty pool. On the leaking version each of these allocated and sent a
    // new batch; the count below was unbounded.
    for _ in 0..50 {
        schedule.run(&mut h.world);
    }

    let sent = std::iter::from_fn(|| h.req_rx.try_recv().ok()).count();
    assert_eq!(
        sent, 0,
        "an empty pool must cost a tick of inference, not a new allocation: {sent} batch(es) were \
         created out of nothing"
    );

    // And the skip is not permanent damage: returning one batch resumes thinking on the next tick.
    h.recycle_req_tx
        .send(held.pop().expect("the pool was pre-filled"))
        .expect("the pool receiver is alive");
    schedule.run(&mut h.world);
    assert!(
        h.req_rx.try_recv().is_ok(),
        "a returned batch must let the next tick think again"
    );
}
