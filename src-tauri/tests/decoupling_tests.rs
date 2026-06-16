use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};
use anima_engine_lib::core::ecs::{
    Agent, Position, Rotation, Velocity, ParentAgent, Segment,
    CognitiveState, InertiaComponent, SensoryBufferComponent,
};
use anima_engine_lib::ai::cpg::{CpgOscillator, update_cpg_system};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::physics::dynamics::{RigidBody, integrate_physics_system};
use anima_engine_lib::core::agent_systems::{
    sensory_system, action_resolution_system, InferenceChannels,
    InferenceRequestBatch, InferenceResponseBatch, AgentInferenceResponse,
};

#[test]
fn test_decoupled_inference_cycle() {
    // 1. Setup world using the default initializer to populate necessary resources
    let mut world = anima_engine_lib::core::ecs::init_world();

    // 2. Setup InferenceChannels with mock recycling queues
    let (req_tx, req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
    let (recycle_req_tx, recycle_req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
    let (res_tx, res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();
    let (recycle_res_tx, recycle_res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();

    // Pre-populate recycling queues
    for _ in 0..8 {
        let req_batch = InferenceRequestBatch {
            requests: Vec::with_capacity(32),
        };
        let res_batch = InferenceResponseBatch {
            responses: Vec::with_capacity(32),
        };
        let _ = recycle_req_tx.send(req_batch);
        let _ = recycle_res_tx.send(res_batch);
    }

    let channels = InferenceChannels {
        req_tx,
        recycle_req_rx,
        res_rx,
        recycle_res_tx,
    };
    world.insert_resource(channels);

    // 3. Spawn agent entity with required components
    let agent = world.spawn((
        Agent,
        Position(Vec3::ZERO),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        RigidBody { mass: 1.0, velocity: Vec3::ZERO, force: Vec3::ZERO },
        HomeostaticState {
            energy: 100.0,
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        CognitiveState::Ready,
        InertiaComponent::default(),
        SensoryBufferComponent::default(),
    )).id();

    // Insert ParentAgent to identify itself
    world.entity_mut(agent).insert(ParentAgent(agent));

    // Spawn a child segment connected to the agent, with CpgOscillator
    let child_segment = world.spawn((
        ParentAgent(agent),
        Segment { id: 0, length: 1.0, radius: 0.1, mass: 1.0 },
        CpgOscillator::new(1.0, 0.5),
    )).id();

    // 4. Register systems in a single-threaded Bevy scheduler with explicit ordering
    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems((
        sensory_system,
        action_resolution_system.after(sensory_system),
        integrate_physics_system.after(action_resolution_system),
        update_cpg_system,
    ));

    // 5. RUN SCHEDULER & VERIFY CYCLE:

    // 5a. Verify that when the agent is Ready, sensory_system queues a request and transitions to PendingInference.
    assert!(matches!(
        *world.get::<CognitiveState>(agent).unwrap(),
        CognitiveState::Ready
    ));

    // Run 1: transitions state, queues request, increments ticks_pending to 1, ticks oscillator
    schedule.run(&mut world);

    // After 1 run:
    // - State transitions to PendingInference(ticket_id)
    let ticket_id = match *world.get::<CognitiveState>(agent).unwrap() {
        CognitiveState::PendingInference(id) => id,
        other => panic!("Expected CognitiveState::PendingInference, got {:?}", other),
    };

    // - Request should be queued in channels
    let req_batch = req_rx.try_recv().expect("Expected a queued inference request in req_tx");
    assert_eq!(req_batch.requests.len(), 1);
    let req = &req_batch.requests[0];
    assert_eq!(req.entity, agent);
    assert_eq!(req.request_id, ticket_id);

    // Recycle the request batch
    recycle_req_tx.send(req_batch).unwrap();

    // Verify ticks_pending is 1
    let inertia_after_1 = world.get::<InertiaComponent>(agent).unwrap();
    assert_eq!(inertia_after_1.ticks_pending, 1, "Expected ticks_pending to increment to 1 on first pending frame");

    // Record oscillator state
    let osc_after_1 = world.get::<CpgOscillator>(child_segment).unwrap().clone();
    assert!(osc_after_1.phase > 0.0, "Oscillator should have ticked once in the first run");

    // 5b. Verify that when the request is pending, the agent continues its movement (oscillators tick, etc.) and ticks_pending increments.
    // Run 2: state remains PendingInference, increments ticks_pending to 2, ticks oscillator again
    schedule.run(&mut world);

    let osc_after_2 = world.get::<CpgOscillator>(child_segment).unwrap().clone();
    assert!(osc_after_2.phase > osc_after_1.phase, "CPG Oscillator did not tick when request was pending");

    let inertia_after_2 = world.get::<InertiaComponent>(agent).unwrap();
    assert_eq!(inertia_after_2.ticks_pending, 2, "Expected ticks_pending to increment to 2");

    // 5c. Verify that sending a response via res_tx updates the agent's InertiaComponent::cpg_parameters and child segment oscillators, and resets the state to Ready.
    let mut res_batch = recycle_res_rx.try_recv().expect("Expected reusable response batch");
    res_batch.responses.clear();
    res_batch.responses.push(AgentInferenceResponse {
        entity: agent,
        actions: [0.5, 0.8, 0.5, 0.8], // test CPG actions
        request_id: ticket_id,
    });
    res_tx.send(res_batch).unwrap();

    // Run 3: processes response, resets state to Ready, resets ticks_pending to 0, updates parameters/oscillators
    schedule.run(&mut world);

    // Check reset to Ready
    assert!(matches!(
        *world.get::<CognitiveState>(agent).unwrap(),
        CognitiveState::Ready
    ));

    // Check ticks_pending reset to 0
    let inertia_res = world.get::<InertiaComponent>(agent).unwrap();
    assert_eq!(inertia_res.ticks_pending, 0);
    assert_eq!(inertia_res.cpg_parameters, [0.5, 0.8, 0.5, 0.8]);

    // Check child segment oscillator updated:
    // frequency = 0.1 + freq_raw * 2.9 = 0.1 + 0.5 * 2.9 = 1.55
    // amplitude = amp_raw * 1.5 = 0.8 * 1.5 = 1.2
    let osc_res = world.get::<CpgOscillator>(child_segment).unwrap();
    assert!((osc_res.frequency - 1.55).abs() < 1e-5, "Expected oscillator frequency to be 1.55, got {}", osc_res.frequency);
    assert!((osc_res.amplitude - 1.2).abs() < 1e-5, "Expected oscillator amplitude to be 1.2, got {}", osc_res.amplitude);

    // 5d. Verify fallback timeout after 5 ticks without response:
    // Run 4: since state is Ready, sensory_system runs and transitions to PendingInference, integrate_physics_system increments ticks_pending to 1
    schedule.run(&mut world);

    // Check state is PendingInference
    let new_ticket_id = match *world.get::<CognitiveState>(agent).unwrap() {
        CognitiveState::PendingInference(id) => id,
        other => panic!("Expected CognitiveState::PendingInference, got {:?}", other),
    };

    // Clean req_rx
    let req_batch = req_rx.try_recv().expect("Expected new inference request");
    assert_eq!(req_batch.requests[0].request_id, new_ticket_id);
    let _ = recycle_req_tx.send(req_batch);

    let inertia_pre = world.get::<InertiaComponent>(agent).unwrap();
    assert_eq!(inertia_pre.ticks_pending, 1);

    // Run 4 more ticks/frames without response.
    // Total runs in pending state: Run 4 (ticks_pending = 1), Run 5 (ticks_pending = 2),
    // Run 6 (ticks_pending = 3), Run 7 (ticks_pending = 4), Run 8 (ticks_pending = 5).
    for i in 2..=5 {
        schedule.run(&mut world);
        let inertia = world.get::<InertiaComponent>(agent).unwrap();
        assert_eq!(inertia.ticks_pending, i);
        assert!(matches!(
            *world.get::<CognitiveState>(agent).unwrap(),
            CognitiveState::PendingInference(id) if id == new_ticket_id
        ));
    }

    // Now ticks_pending is 5. One more run (Run 9, the 6th run in pending state)
    // will increment ticks_pending to 6, triggering the fallback (> 5 ticks).
    schedule.run(&mut world);

    let final_state = world.get::<CognitiveState>(agent).unwrap();
    assert!(matches!(*final_state, CognitiveState::Ready), "Expected timeout to reset state to Ready, got {:?}", final_state);

    let final_inertia = world.get::<InertiaComponent>(agent).unwrap();
    assert_eq!(final_inertia.ticks_pending, 0, "Expected ticks_pending to reset to 0");
    assert_eq!(final_inertia.cpg_parameters, [1.0, 0.0, 1.0, 0.0], "Expected baseline CPG parameters");

    // Check child segment oscillator updated to baseline parameters: [1.0, 0.0, 1.0, 0.0]
    // frequency = 0.1 + 1.0 * 2.9 = 3.0
    // amplitude = 0.0 * 1.5 = 0.0
    let final_osc = world.get::<CpgOscillator>(child_segment).unwrap();
    assert!((final_osc.frequency - 3.0).abs() < 1e-5, "Expected baseline oscillator frequency to be 3.0, got {}", final_osc.frequency);
    assert!((final_osc.amplitude - 0.0).abs() < 1e-5, "Expected baseline oscillator amplitude to be 0.0, got {}", final_osc.amplitude);
}
