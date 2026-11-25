//! Card I3 acceptance & negative-control gates (`ADR-0014` / `QD-SHAREDISLAND`):
//!
//! 1. Round-trip fidelity for `IslandJoinSnapshot`, `IslandSnapshot`, and quantized `IslandDelta`
//!    in `bincode` framing (`ISLAND-D3(a)`).
//! 2. Interest-area threshold & radius culling (`ISLAND-D4(b)`): whole-island `<= 250` agents,
//!    spatial radius culling `> 250` agents while keeping global census accurate.
//! 3. Bounded non-blocking slow-consumer drop/coalesce fan-out ring (`MAX_ISLAND_GUESTS` cap +
//!    stalled guest resynchronizes cleanly on self-contained `Snapshot` without blocking healthy guests).
//! 4. Mandatory Negative Control #2: `IslandJoinSnapshot` strips `SimRng` seed/cursor, `AgentBrain`
//!    weights, and internal worker/learner state from both JSON and raw `bincode` wire bytes.
//! 5. `1,000`-agent serialized stream byte budget stays far below the `1,209,987 B`/tick JSON
//!    baseline (`C11`).
//! 6. Card `I3` DONE WHEN #1 + Mandatory Negative Control #1: `host_with_spectate_guests_matches_solo`
//!    matches Solo bit-for-bit across `1,800` ticks while moving guest cameras and projecting off-tick
//!    at `~20 Hz`, whereas leaking a spectator camera into `SharedLodFocus` diverges `world_checksum`.

use anima_engine_lib::ai::pheromone::{PheromoneGrid, PheromoneGridState, CELL_COUNT, GRID_SIZE};
use anima_engine_lib::core::components::{
    AgentBrain, AgentType, CombatEvent, EnvironmentalElement, EnvironmentalState,
};
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::island::stream::{
    decode_stream_frame_bincode, encode_stream_frame_bincode, GuestInterestWindow,
    IslandJoinSnapshot, IslandStreamError, IslandStreamFrame, IslandStreamHub,
    IslandStreamReconstructor, MAX_ISLAND_STREAM_FRAME_BYTES, WHOLE_ISLAND_BROADCAST_AGENT_LIMIT,
};
use anima_engine_lib::core::island::{
    GenesisPhase, GuestId, GuestRole, IslandId, DEFAULT_ISLAND_MAX_GUESTS,
};
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LIVE_KEY_DEHYDRATION_TICKS, LIVE_KEY_DRIVE_FLOOR,
    LIVE_KEY_EVOLVED_BRAINS, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS, LIVE_KEY_LAKES,
    LIVE_KEY_LAKE_LAYOUT, LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY, LIVE_KEY_PREDATOR_FRACTION,
    LIVE_KEY_PREY_CAP, LIVE_KEY_REPRODUCTION, LIVE_KEY_THIRST_STEERING, LIVE_KEY_TREES,
};
use anima_engine_lib::core::observer::ObserverPolicy;
use anima_engine_lib::core::simulation_lod::{LodBands, LodFocus, SharedLodFocus};
use anima_engine_lib::core::simulation_state::{
    extract_environmental_state_from_world, extract_segment_states_from_world, ChronicleEvent,
    SegmentState, SimulationTickPayload,
};
use anima_engine_lib::evolution::brain_genotype::{ArchSpec, BrainGenotype};
use std::collections::HashMap;
use std::sync::Arc;

const C11_JSON_BASELINE_BYTES_PER_TICK: usize = 1_209_987;

fn make_segment(
    agent_id: u32,
    segment_id: u32,
    parent_segment_id: Option<u32>,
    pos: [f32; 3],
    yaw: f32,
    energy: f32,
    agent_type: AgentType,
) -> SegmentState {
    SegmentState {
        agent_id,
        segment_id,
        parent_segment_id,
        length: 1.25,
        radius: 0.35,
        x: pos[0],
        y: pos[1],
        z: pos[2],
        yaw,
        pitch: 0.12,
        roll: -0.08,
        joint_anchor_x: 0.0,
        joint_anchor_y: 0.0,
        joint_anchor_z: 0.0,
        joint_axis_x: 0.0,
        joint_axis_y: 1.0,
        joint_axis_z: 0.0,
        energy,
        hydration: 72.4,
        head_direction: [0.0, 0.0, 1.0],
        agent_type: Some(agent_type),
    }
}

fn make_sample_env(
    tree_count: usize,
    lake_count: usize,
    resource_scale: f32,
) -> EnvironmentalState {
    let mut elements = Vec::with_capacity(tree_count + lake_count);
    for i in 0..lake_count {
        let angle = (i as f32) * 0.37;
        elements.push(EnvironmentalElement {
            element_type: "lake".to_string(),
            x: angle.cos() * 80.0,
            y: angle.sin() * 80.0,
            radius: 4.5,
            resources: (50.0 + (i % 17) as f32) * resource_scale,
        });
    }
    for i in 0..tree_count {
        let x = ((i % 25) as f32 - 12.0) * 14.0;
        let z = ((i / 25) as f32 - 6.0) * 14.0;
        elements.push(EnvironmentalElement {
            element_type: "tree".to_string(),
            x,
            y: z,
            radius: 1.8,
            resources: (10.0 + (i % 9) as f32) * resource_scale,
        });
    }
    EnvironmentalState { elements }
}

fn build_island_world(seed: u64) -> LiveExperimentAdapter {
    let initial = InitialConditionSet::new(vec![
        (LIVE_KEY_FOUNDERS.into(), 18.0),
        (LIVE_KEY_PREDATOR_FRACTION.into(), 0.25),
        (LIVE_KEY_PREY_CAP.into(), 24.0),
        (LIVE_KEY_TREES.into(), 6.0),
        (LIVE_KEY_LAKES.into(), 8.0),
        (LIVE_KEY_LAKE_LAYOUT.into(), 1.0),
        (LIVE_KEY_DEHYDRATION_TICKS.into(), 960.0),
        (LIVE_KEY_FOOD_CAP.into(), 40.0),
        (LIVE_KEY_MORTALITY.into(), 1.0),
        (LIVE_KEY_REPRODUCTION.into(), 1.0),
        (LIVE_KEY_EVOLVED_BRAINS.into(), 1.0),
        (LIVE_KEY_LOCOMOTION.into(), 1.0),
        (LIVE_KEY_DRIVE_FLOOR.into(), 0.5),
        (LIVE_KEY_THIRST_STEERING.into(), 1.0),
    ]);
    let adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        seed,
        (16, 16),
        1_800,
    )
    .expect("island benchmark world builds");

    {
        let mut world = adapter.world();
        // Arm `ObserverPolicy::Inhabit` with tight bands while keeping `SharedLodFocus` disabled on
        // the solo baseline. Nothing can blame a dormant LOD configuration if any stray camera
        // write leaks into `SharedLodFocus`: it triggers tier divergence immediately.
        world.insert_resource(SharedLodFocus::new_disabled());
        world.insert_resource(ObserverPolicy::Inhabit { cause_id: 7 });
        world.insert_resource(LodBands {
            hot_radius: 10.0,
            warm_radius: 25.0,
            warm_interval: 4,
        });
    }

    adapter
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn round_trip_fidelity_and_delta_reconstruction() {
    let island_id = IslandId::new(101).unwrap();
    let guest = GuestId::new(1).unwrap();
    let mut hub = IslandStreamHub::with_defaults(island_id);
    hub.register_guest(
        GuestInterestWindow::new(guest, GuestRole::Spectate, 0.0, 0.0, 120.0).unwrap(),
    )
    .unwrap();

    let mut segments = vec![
        make_segment(
            10,
            0,
            None,
            [-42.31, 0.5, 19.77],
            2.85,
            88.4,
            AgentType::Prey,
        ),
        make_segment(
            10,
            1,
            Some(0),
            [-43.1, 0.5, 19.2],
            -3.05,
            88.4,
            AgentType::Prey,
        ),
        make_segment(
            22,
            0,
            None,
            [15.62, 0.5, -11.4],
            0.45,
            120.0,
            AgentType::Predator,
        ),
    ];
    let mut env = make_sample_env(4, 2, 1.0);
    let mut pheromone = PheromoneGridState {
        grid: vec![0.0; CELL_COUNT],
        width: GRID_SIZE as u32,
        height: GRID_SIZE as u32,
    };
    pheromone.grid[512] = 6.25;
    let combat = vec![CombatEvent {
        predator_id: 22,
        prey_id: 10,
        damage: 4.5,
        energy_transferred: 3.2,
    }];

    hub.project_tick(1, &segments, &env, Some(&pheromone), &combat)
        .unwrap();
    let snap_pkt = hub.pop_guest_frame(guest).expect("keyframe queued");
    assert!(snap_pkt.is_keyframe);
    let snap_frame = decode_stream_frame_bincode(&snap_pkt.bytes).unwrap();
    assert!(matches!(snap_frame, IslandStreamFrame::Snapshot(_)));

    let mut recon = IslandStreamReconstructor::new();
    recon.apply_frame(&snap_frame).unwrap();
    assert_eq!(recon.segments().len(), 3);
    assert_eq!(recon.census.total_agents, 2);
    assert_eq!(recon.census.predators, 1);
    assert_eq!(recon.census.prey, 1);
    assert!(!recon.census.interest_culled);

    // Negative check: delta whose base_frame_seq doesn't match last_frame_seq is rejected.
    let bad_delta = IslandStreamFrame::Delta(anima_engine_lib::core::island::stream::IslandDelta {
        island_id,
        base_frame_seq: 99,
        frame_seq: 100,
        tick: 2,
        census: recon.census,
        added_segments: Vec::new(),
        updated_segments: Vec::new(),
        removed_segments: Vec::new(),
        env_deltas: Vec::new(),
        pheromone_deltas: Vec::new(),
        combat_events: Vec::new(),
    });
    assert!(matches!(
        recon.apply_frame(&bad_delta),
        Err(IslandStreamError::DeltaBaseMismatch {
            expected: 1,
            found: 99
        })
    ));

    // Tick 2..4: move segments, deplete one tree, mutate pheromone, spawn agent 33, remove agent 22.
    segments[0].x += 1.75;
    segments[0].z -= 0.9;
    segments[0].yaw = -2.95;
    segments[0].energy = 79.2;
    segments[1].x += 1.6;
    segments.pop(); // remove agent 22
    segments.push(make_segment(
        33,
        0,
        None,
        [5.5, 0.5, -2.25],
        1.1,
        64.0,
        AgentType::Prey,
    ));
    env.elements[2].resources = 3.4;
    pheromone.grid[512] = 0.0;
    pheromone.grid[900] = 8.0;

    hub.project_tick(2, &segments, &env, Some(&pheromone), &[])
        .unwrap();
    let delta_pkt = hub.pop_guest_frame(guest).expect("delta queued");
    assert!(!delta_pkt.is_keyframe);
    let delta_frame = decode_stream_frame_bincode(&delta_pkt.bytes).unwrap();
    assert!(matches!(delta_frame, IslandStreamFrame::Delta(_)));
    recon.apply_frame(&delta_frame).unwrap();

    let rebuilt = recon.to_segment_states();
    assert_eq!(rebuilt.len(), segments.len());
    for expected in &segments {
        let actual = rebuilt
            .iter()
            .find(|s| s.agent_id == expected.agent_id && s.segment_id == expected.segment_id)
            .expect("reconstructed segment present");
        assert_eq!(actual.parent_segment_id, expected.parent_segment_id);
        assert_eq!(actual.agent_type, expected.agent_type);
        assert!((actual.x - expected.x).abs() <= 0.032, "x err");
        assert!((actual.y - expected.y).abs() <= 0.032, "y err");
        assert!((actual.z - expected.z).abs() <= 0.032, "z err");
        assert!((actual.length - expected.length).abs() <= 0.002);
        assert!((actual.radius - expected.radius).abs() <= 0.002);
        assert!((actual.energy - expected.energy).abs() <= 0.11);
        assert!((actual.hydration - expected.hydration).abs() <= 0.11);
        let dyaw = (actual.yaw - expected.yaw)
            .sin()
            .atan2((actual.yaw - expected.yaw).cos());
        assert!(dyaw.abs() <= 0.001, "yaw err {dyaw}");
    }
    assert!(
        (recon.env_features()[2].to_environmental_element().resources - 3.4).abs() <= 0.11,
        "env resource delta must propagate"
    );
    assert_eq!(recon.pheromone_cells()[512], 0);
    assert!(recon.pheromone_cells()[900] > 180);
}

#[test]
fn interest_area_threshold_and_slow_consumer_coalescing() {
    let island_id = IslandId::new(202).unwrap();

    // Part A: `<= 250` whole-island broadcast vs `> 250` spatial radius cull (`ISLAND-D4(b)`).
    let guest_cull = GuestId::new(1).unwrap();
    let mut hub_250 = IslandStreamHub::with_defaults(island_id);
    hub_250
        .register_guest(
            GuestInterestWindow::new(guest_cull, GuestRole::Spectate, 0.0, 0.0, 30.0).unwrap(),
        )
        .unwrap();

    let mut segs_250 = Vec::with_capacity(WHOLE_ISLAND_BROADCAST_AGENT_LIMIT);
    for id in 1..=(WHOLE_ISLAND_BROADCAST_AGENT_LIMIT as u32) {
        segs_250.push(make_segment(
            id,
            0,
            None,
            [(id as f32) * 2.0, 0.0, 0.0],
            0.0,
            80.0,
            AgentType::Prey,
        ));
    }
    let env = make_sample_env(2, 1, 1.0);
    hub_250.project_tick(1, &segs_250, &env, None, &[]).unwrap();
    let frame_250 =
        decode_stream_frame_bincode(&hub_250.pop_guest_frame(guest_cull).unwrap().bytes).unwrap();
    let IslandStreamFrame::Snapshot(snap_250) = frame_250 else {
        panic!("expected snapshot");
    };
    assert!(!snap_250.census.interest_culled);
    assert_eq!(
        snap_250.census.total_agents as usize,
        WHOLE_ISLAND_BROADCAST_AGENT_LIMIT
    );
    assert_eq!(
        snap_250.census.visible_agents as usize,
        WHOLE_ISLAND_BROADCAST_AGENT_LIMIT
    );
    assert_eq!(snap_250.segments.len(), WHOLE_ISLAND_BROADCAST_AGENT_LIMIT);

    // 251 agents: crosses threshold -> radius filter kicks in while keeping global census intact.
    let mut segs_251 = segs_250;
    segs_251.push(make_segment(
        251,
        0,
        None,
        [-500.0, 0.0, 0.0],
        0.0,
        80.0,
        AgentType::Predator,
    ));
    hub_250.project_tick(2, &segs_251, &env, None, &[]).unwrap();
    let frame_251 =
        decode_stream_frame_bincode(&hub_250.pop_guest_frame(guest_cull).unwrap().bytes).unwrap();
    let IslandStreamFrame::Delta(delta_251) = frame_251 else {
        panic!("expected delta");
    };
    assert!(delta_251.census.interest_culled);
    assert_eq!(delta_251.census.total_agents, 251);
    assert_eq!(delta_251.census.predators, 1);
    assert_eq!(delta_251.census.prey, 250);
    assert_eq!(delta_251.census.visible_agents, 15); // x = 2,4,...,30

    // Part B: guest limit cap (`DEFAULT_ISLAND_MAX_GUESTS = 16`) & slow-consumer coalescing.
    let mut hub = IslandStreamHub::with_defaults(island_id);
    for raw in 1..=u64::from(DEFAULT_ISLAND_MAX_GUESTS) {
        let g = GuestId::new(raw).unwrap();
        let role = if raw % 2 == 0 {
            GuestRole::Inhabit
        } else {
            GuestRole::Spectate
        };
        hub.register_guest(GuestInterestWindow::new(g, role, 0.0, 0.0, 80.0).unwrap())
            .unwrap();
    }
    assert_eq!(
        hub.register_guest(
            GuestInterestWindow::new(
                GuestId::new(999).unwrap(),
                GuestRole::Spectate,
                0.0,
                0.0,
                80.0
            )
            .unwrap()
        ),
        Err(IslandStreamError::GuestLimitReached {
            max: DEFAULT_ISLAND_MAX_GUESTS
        })
    );

    let fast = GuestId::new(1).unwrap();
    let slow = GuestId::new(2).unwrap();
    let mut dyn_segs = vec![make_segment(
        1,
        0,
        None,
        [1.0, 0.0, 1.0],
        0.0,
        80.0,
        AgentType::Prey,
    )];
    let mut fast_recon = IslandStreamReconstructor::new();
    for tick in 1..=20u64 {
        dyn_segs[0].x += 0.5;
        hub.project_tick(tick, &dyn_segs, &env, None, &[]).unwrap();
        let pkt = hub
            .pop_guest_frame(fast)
            .expect("fast guest always receives");
        let frame = decode_stream_frame_bincode(&pkt.bytes).unwrap();
        fast_recon.apply_frame(&frame).unwrap();
        // `slow` deliberately never drains during the burst!
    }

    let fast_stats = hub.guest_stats(fast).unwrap();
    let slow_stats = hub.guest_stats(slow).unwrap();
    assert_eq!(fast_stats.dropped_frames, 0);
    assert_eq!(fast_stats.keyframes_sent, 1);
    assert_eq!(fast_stats.deltas_sent, 19);
    assert!(
        slow_stats.dropped_frames > 0,
        "slow consumer must drop stale frames"
    );
    assert!(
        slow_stats.queued_frames <= 4,
        "slow mailbox must stay bounded"
    );

    // When slow consumer finally drains its bounded mailbox (<= 4 frames), its first popped frame
    // must be a self-contained Snapshot keyframe (tick 17) followed cleanly by queued deltas to 20.
    let mut slow_recon = IslandStreamReconstructor::new();
    let mut drained = 0usize;
    while let Some(pkt) = hub.pop_guest_frame(slow) {
        if drained == 0 {
            assert!(
                pkt.is_keyframe,
                "first surviving frame after mailbox coalesce must be a Snapshot keyframe"
            );
        }
        let frame = decode_stream_frame_bincode(&pkt.bytes).unwrap();
        slow_recon.apply_frame(&frame).unwrap();
        drained += 1;
    }
    assert!((1..=4).contains(&drained));
    assert_eq!(slow_recon.last_tick, 20);
    assert!((slow_recon.segments()[0].to_segment_state().x - 11.0).abs() <= 0.032);
}

#[test]
fn join_snapshot_strips_sim_rng_and_brain_weights() {
    let mut adapter = build_island_world(42_424);
    for _ in 0..8 {
        adapter.run_schedule_once();
    }
    let mut saved = adapter.snapshot().saved;

    // Inject unmistakable sentinels into every forbidden secret compartment.
    const SENTINEL_SEED: u64 = 0xDEAD_BEEF_CAFE_BABEu64;
    const SENTINEL_POS: u128 = 0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210u128;
    const SENTINEL_WEIGHT: f32 = 0.7319421;
    saved.sim_rng_seed = SENTINEL_SEED;
    saved.sim_rng_pos = SENTINEL_POS;
    let arch = ArchSpec::new(15, 12, 4);
    let sentinel_brain = AgentBrain {
        genotype: Arc::new(
            BrainGenotype::from_weights(arch, vec![SENTINEL_WEIGHT; arch.param_count()]).unwrap(),
        ),
        learned: Some(Arc::new(
            BrainGenotype::from_weights(arch, vec![-0.6180339; arch.param_count()]).unwrap(),
        )),
    };
    for agent in &mut saved.agents {
        agent.brain = Some(sentinel_brain.clone());
    }
    saved.chronicle_history.push(ChronicleEvent {
        id: "founder-1".into(),
        event_type: "Abundance".into(),
        timestamp: 0,
        title: "Founders Placed".into(),
        description: "Hidden internal note".into(),
        parameter_delta: HashMap::new(),
    });

    let island_id = IslandId::new(77).unwrap();
    let guests = [GuestId::new(9).unwrap(), GuestId::new(3).unwrap()];
    let join = IslandJoinSnapshot::from_saved_state(island_id, GenesisPhase::G1, &guests, &saved);
    assert_eq!(join.genesis_label, "designed founders");
    assert_eq!(
        join.connected_guest_ids,
        vec![GuestId::new(3).unwrap(), GuestId::new(9).unwrap()]
    );
    assert_eq!(
        join.initial_snapshot.census.total_agents as usize,
        saved.agents.len()
    );

    let frame = IslandStreamFrame::Join(join);
    let json = serde_json::to_string(&frame).unwrap();
    let bincode_bytes = encode_stream_frame_bincode(&frame).unwrap();

    // JSON wire inspection: forbidden fields and sentinel values must never appear.
    for forbidden_key in [
        "sim_rng",
        "sim_rng_seed",
        "sim_rng_pos",
        "brain",
        "learned",
        "weights",
        "genotype",
        "evolution_worker",
        "shared_learning",
        "map_elites",
        "dormant_cohorts",
        "0.7319421",
        "-0.6180339",
        "Hidden internal note",
    ] {
        assert!(
            !json.contains(forbidden_key),
            "IslandJoinSnapshot leaked forbidden substring `{forbidden_key}` in JSON"
        );
    }
    assert!(!json.contains(&SENTINEL_SEED.to_string()));
    assert!(!json.contains(&SENTINEL_POS.to_string()));

    // Raw binary inspection: sentinel bytes of SimRng and brain weights must never cross wire.
    assert!(!contains_bytes(
        &bincode_bytes,
        &SENTINEL_SEED.to_le_bytes()
    ));
    assert!(!contains_bytes(
        &bincode_bytes,
        &SENTINEL_SEED.to_be_bytes()
    ));
    assert!(!contains_bytes(&bincode_bytes, &SENTINEL_POS.to_le_bytes()));
    assert!(!contains_bytes(
        &bincode_bytes,
        &SENTINEL_WEIGHT.to_le_bytes()
    ));
    assert!(!contains_bytes(
        &bincode_bytes,
        &(-0.6180339f32).to_le_bytes()
    ));

    // Round-trip decode fidelity holds.
    let decoded = decode_stream_frame_bincode(&bincode_bytes).unwrap();
    let mut recon = IslandStreamReconstructor::new();
    recon.apply_frame(&decoded).unwrap();
    assert_eq!(
        recon.segments().len(),
        saved
            .agents
            .iter()
            .map(|a| 1 + a.segments.len())
            .sum::<usize>()
    );
}

#[test]
fn thousand_agent_stream_size_below_json_baseline() {
    let mut segments = Vec::with_capacity(3_000);
    for idx in 0..1_000u32 {
        let agent_id = idx + 1;
        let gx = (idx % 40) as f32 * 12.0 - 240.0;
        let gz = (idx / 40) as f32 * 12.0 - 150.0;
        let kind = if idx % 5 == 0 {
            AgentType::Predator
        } else {
            AgentType::Prey
        };
        segments.push(make_segment(
            agent_id,
            0,
            None,
            [gx, 0.5, gz],
            0.3,
            95.0,
            kind,
        ));
        segments.push(make_segment(
            agent_id,
            1,
            Some(0),
            [gx - 0.6, 0.5, gz],
            0.31,
            95.0,
            kind,
        ));
        segments.push(make_segment(
            agent_id,
            2,
            Some(1),
            [gx - 1.2, 0.5, gz],
            0.32,
            95.0,
            kind,
        ));
    }
    let env = make_sample_env(150, 50, 1.0);
    let mut pheromone = PheromoneGridState {
        grid: vec![0.0; CELL_COUNT],
        width: GRID_SIZE as u32,
        height: GRID_SIZE as u32,
    };
    for cell in 0..50 {
        pheromone.grid[cell * 17] = 4.0;
    }

    // Legacy JSON SimulationTickPayload baseline on the exact same 1,000-agent scene.
    let mut head_directions = HashMap::with_capacity(1_000);
    for seg in &segments {
        if seg.parent_segment_id.is_none() {
            head_directions.insert(seg.agent_id, seg.head_direction);
        }
    }
    let json_payload = SimulationTickPayload {
        segments: segments.clone(),
        environmental_state: env.clone(),
        head_directions,
    };
    let legacy_json_bytes = serde_json::to_vec(&json_payload).unwrap().len();

    // Whole-island broadcast (`radius = 10_000` covers full arena) AND spatial interest cull (`radius = 75`).
    let island_id = IslandId::new(303).unwrap();
    let g_whole = GuestId::new(1).unwrap();
    let g_culled = GuestId::new(2).unwrap();
    let mut hub = IslandStreamHub::with_defaults(island_id);
    hub.register_guest(
        GuestInterestWindow::new(g_whole, GuestRole::Spectate, 0.0, 0.0, 10_000.0).unwrap(),
    )
    .unwrap();
    hub.register_guest(
        GuestInterestWindow::new(g_culled, GuestRole::Spectate, 0.0, 0.0, 75.0).unwrap(),
    )
    .unwrap();

    hub.project_tick(1, &segments, &env, Some(&pheromone), &[])
        .unwrap();
    let snap_whole_bytes = hub.pop_guest_frame(g_whole).unwrap().bytes.len();
    let snap_culled_bytes = hub.pop_guest_frame(g_culled).unwrap().bytes.len();

    // Next tick: every single agent and child segment moves slightly (worst-case 100% churn delta).
    for seg in &mut segments {
        seg.x += 0.15;
        seg.z += 0.10;
        seg.yaw += 0.02;
        seg.energy -= 0.1;
    }
    hub.project_tick(2, &segments, &env, Some(&pheromone), &[])
        .unwrap();
    let delta_whole_bytes = hub.pop_guest_frame(g_whole).unwrap().bytes.len();
    let delta_culled_frame = hub.pop_guest_frame(g_culled).unwrap();
    let delta_culled_bytes = delta_culled_frame.bytes.len();
    let IslandStreamFrame::Delta(d) =
        decode_stream_frame_bincode(&delta_culled_frame.bytes).unwrap()
    else {
        panic!("expected delta");
    };
    assert!(d.census.interest_culled);
    assert_eq!(d.census.total_agents, 1_000);
    assert!(d.census.visible_agents < 250);

    eprintln!(
        "I3 1000-agent wire sizes: legacy_scene_json={} B, c11_baseline={} B, \
         bincode_whole_snap={} B, bincode_whole_delta={} B, bincode_culled_snap={} B, bincode_culled_delta={} B",
        legacy_json_bytes,
        C11_JSON_BASELINE_BYTES_PER_TICK,
        snap_whole_bytes,
        delta_whole_bytes,
        snap_culled_bytes,
        delta_culled_bytes,
    );

    assert!(snap_whole_bytes < MAX_ISLAND_STREAM_FRAME_BYTES);
    assert!(
        snap_whole_bytes * 5 < C11_JSON_BASELINE_BYTES_PER_TICK,
        "whole-island 1000-agent snapshot ({snap_whole_bytes} B) exceeded 20% of C11 JSON baseline"
    );
    assert!(
        delta_whole_bytes * 10 < C11_JSON_BASELINE_BYTES_PER_TICK,
        "whole-island 1000-agent delta ({delta_whole_bytes} B) exceeded 10% of C11 JSON baseline"
    );
    assert!(
        delta_culled_bytes * 20 < C11_JSON_BASELINE_BYTES_PER_TICK,
        "culled 1000-agent delta ({delta_culled_bytes} B) exceeded 5% of C11 JSON baseline"
    );
}

#[test]
fn host_with_spectate_guests_matches_solo() {
    const SEED: u64 = 20_261_124;
    const CHECKPOINTS: [u64; 4] = [0, 180, 900, 1_800];

    // 1. Solo reference run.
    let mut solo = build_island_world(SEED);
    let mut solo_checks = vec![(0u64, solo.checksum())];
    for &target in CHECKPOINTS.iter().skip(1) {
        while solo.tick() < target {
            solo.run_schedule_once();
        }
        solo_checks.push((target, solo.checksum()));
    }

    // 2. Host with 3 active Spectate guests panning cameras and off-tick projecting every 3 ticks (~20 Hz).
    let mut host = build_island_world(SEED);
    let island_id = IslandId::new(808).unwrap();
    let mut hub = IslandStreamHub::with_defaults(island_id);
    let guest_ids = [
        GuestId::new(1).unwrap(),
        GuestId::new(2).unwrap(),
        GuestId::new(3).unwrap(),
    ];
    for (idx, &gid) in guest_ids.iter().enumerate() {
        hub.register_guest(
            GuestInterestWindow::new(gid, GuestRole::Spectate, (idx as f32) * 30.0, 0.0, 60.0)
                .unwrap(),
        )
        .unwrap();
    }
    let mut seg_buf = Vec::with_capacity(256);
    let mut env_buf = EnvironmentalState::default();
    let mut phero_buf = PheromoneGridState {
        grid: vec![0.0; CELL_COUNT],
        width: GRID_SIZE as u32,
        height: GRID_SIZE as u32,
    };
    let mut recons = [
        IslandStreamReconstructor::new(),
        IslandStreamReconstructor::new(),
        IslandStreamReconstructor::new(),
    ];
    let mut decoded_frames = [0usize; 3];

    let mut host_checks = vec![(0u64, host.checksum())];
    for &target in CHECKPOINTS.iter().skip(1) {
        while host.tick() < target {
            host.run_schedule_once();
            let tick = host.tick();
            for (i, &gid) in guest_ids.iter().enumerate() {
                let sweep = ((tick as f32) * 0.4 + (i as f32) * 25.0).sin() * 200.0;
                hub.update_guest_interest(gid, sweep, 40.0 + (i as f32) * 15.0, 65.0)
                    .unwrap();
            }
            if tick.is_multiple_of(3) {
                {
                    let mut world = host.world();
                    extract_segment_states_from_world(&mut world, &mut seg_buf);
                    extract_environmental_state_from_world(&mut world, &mut env_buf);
                    if let Some(grid) = world.get_resource::<PheromoneGrid>() {
                        phero_buf.grid.copy_from_slice(&grid.values);
                    }
                }
                hub.project_tick(tick, &seg_buf, &env_buf, Some(&phero_buf), &[])
                    .unwrap();
                for (i, &gid) in guest_ids.iter().enumerate() {
                    if let Some(pkt) = hub.pop_guest_frame(gid) {
                        let frame = decode_stream_frame_bincode(&pkt.bytes).unwrap();
                        recons[i].apply_frame(&frame).unwrap();
                        decoded_frames[i] += 1;
                    }
                }
            }
        }
        host_checks.push((target, host.checksum()));
    }

    assert_eq!(
        solo_checks, host_checks,
        "Host with 3 active Spectate guests and 20 Hz off-tick projection must match Solo bit-for-bit"
    );
    for (i, &count) in decoded_frames.iter().enumerate() {
        assert_eq!(
            count, 600,
            "guest {i} expected 600 projected frames across 1800 ticks"
        );
    }

    // 3. Mandatory Negative Control #1: routing a spectator camera into host `SharedLodFocus`
    //    must diverge `world_checksum`.
    let mut leaked = build_island_world(SEED);
    let leaked_shared = {
        let world = leaked.world();
        world.resource::<SharedLodFocus>().clone()
    };
    while leaked.tick() < 1_800 {
        let cam_x = 180.0 + (leaked.tick() as f32) * 0.1;
        *leaked_shared.0.write().unwrap() = LodFocus::at(glam::Vec3::new(cam_x, 0.0, 180.0));
        leaked.run_schedule_once();
    }
    let leaked_end = (1_800u64, leaked.checksum());
    assert_ne!(
        solo_checks[3], leaked_end,
        "negative control failed: routing spectator camera into SharedLodFocus did not perturb world_checksum"
    );
}
