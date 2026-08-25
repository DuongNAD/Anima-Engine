//! M3 — gate F1 "ngủ đông / đánh thức" (2026-08-25, nhánh `feat/living-world`).
//!
//! Ngủ đông là mục **duy nhất không rollback được** của living-world: một khi
//! EU của một cá thể rời thân nó, sổ sách phải đóng đúng ở cả hai chiều.
//!
//! Gate theo `docs/ai/testing/2026-08-25-feature-living-world.md`:
//! - **FD-S01** — hibernate chuyển ngăn + despawn tại **một** sync point,
//!   residual ≤ `RESIDUAL_ABS_TOLERANCE_EU`.
//! - **FD-S02** — wake **không** develop lại (D01/D02): phenotype và brain giữ
//!   nguyên bit-exact; cá thể thức dậy là cùng một cá thể.
//! - **FD-S03** — hibernate → save → JSON roundtrip → load → wake: residual
//!   ≤ tolerance, không mất agent dormant qua biên save/load.
//! - **F1-5** — ngủ đông KHÔNG đi qua detritus: đây không phải cái chết.

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::{
    AgentEvaluation, AgentGenotype, AgentLineageId, DormantAgents, HibernateAgentCommand,
    WakeAgentCommand,
};
use anima_engine_lib::core::components::{Agent, AgentBrain};
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::ecs::{init_world, ParentAgent, Position, Rotation, Segment, Velocity};
use anima_engine_lib::core::energy_ledger::{closed_total_eu, RESIDUAL_ABS_TOLERANCE_EU};
use anima_engine_lib::core::simulation_state::{
    restore_energy_state, serialize_world_state, spawn_serialized_agent, SavedSimulationState,
    SerializedAgent,
};
use anima_engine_lib::evolution::brain_genotype::{BrainGenotype, EVOLVED_ARCH};
use anima_engine_lib::evolution::genotype::{MorphologyGenotype, MorphologyNode};
use bevy_ecs::prelude::*;
use bevy_ecs::system::Command;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

// ---------------------------------------------------------------------------
// Dụng cụ — khuôn của brain_persistence_tests / snapshot_checkpoint_tests
// ---------------------------------------------------------------------------

fn brain(seed: u64) -> AgentBrain {
    let mut rng = StdRng::seed_from_u64(seed);
    AgentBrain::from_genotype(BrainGenotype::random(EVOLVED_ARCH, &mut rng).unwrap())
}

fn morphology() -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    g.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.5,
        mass: 1.0,
    });
    g
}

fn payload(brain_value: Option<AgentBrain>) -> SerializedAgent {
    SerializedAgent {
        genotype: morphology(),
        class: anima_engine_lib::core::ecs::AgentClass::Prey,
        lineage_id: "lin-dormant".to_string(),
        generation: 3,
        parent_ids: vec![],
        evaluation: AgentEvaluation {
            start_position: glam::Vec3::ZERO,
            total_distance: 1.0,
            total_energy_expended: 2.0,
            total_energy_gained: 0.0,
            survival_ticks: 10,
            last_position: glam::Vec3::ZERO,
        },
        feature_tracker: Default::default(),
        root_position: glam::Vec3::ZERO,
        root_rotation: glam::Quat::IDENTITY,
        root_velocity: glam::Vec3::ZERO,
        homeostatic_state: HomeostaticState {
            energy: 50.0,
            energy_target: 100.0,
            hydration: 50.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
            starving_ticks: 0,
        },
        last_transition_state: Default::default(),
        cognitive_state: Default::default(),
        inertia: Default::default(),
        action_gates: None,
        segments: vec![],
        brain: brain_value,
    }
}

/// Tổng closed-EU tính từ các kho **có thẩm quyền** — đúng định nghĩa của
/// `core::energy_ledger`: plants từ `ResourceField`, animals là tổng reserve
/// sống (không phải mirror), detritus và dormant từ pool.
fn closed_total(world: &mut World) -> f64 {
    let plants = world
        .get_resource::<ResourceField>()
        .map(|f| f.total_biomass())
        .unwrap_or(0.0);
    let (detritus, dormant) = world
        .get_resource::<EcosystemBiomass>()
        .map(|p| (p.detritus, p.dormant))
        .unwrap_or((0.0, 0.0));
    let mut animals = 0.0f64;
    let mut q = world.query_filtered::<&HomeostaticState, With<Agent>>();
    for homeo in q.iter(world) {
        animals += homeo.energy.max(0.0) as f64;
    }
    closed_total_eu(plants, animals, detritus, dormant)
}

/// Thế giới đầy đủ của engine với một con mồi mang `energy` EU reserve.
fn dormancy_world(energy: f32) -> (World, Entity) {
    let mut agent = payload(Some(brain(71)));
    agent.homeostatic_state.energy = energy;
    let mut world = init_world();
    let root = spawn_serialized_agent(&mut world, &agent).expect("valid payload");
    (world, root)
}

/// Bốn handle mà `serialize_world_state` cần — khuôn của
/// snapshot_checkpoint_tests. Lineage tracker fallback in-memory khi không có Neo4j.
fn save(world: &mut World) -> SavedSimulationState {
    let chronicle = Arc::new(RwLock::new(Vec::new()));
    let lineage = Arc::new(
        anima_engine_lib::evolution::lineage::FallbackLineageTracker::new(
            "bolt://127.0.0.1:1",
            "neo4j",
            "password",
        ),
    );
    let evolution = Arc::new(Mutex::new(anima_engine_lib::commands::EvolutionSettings {
        mutation_rate: 0.2,
        selection_bias: 1.2,
        grid_resolution: 30,
    }));
    let grid = Arc::new(Mutex::new(anima_engine_lib::commands::MapElitesGridState {
        grid: HashMap::new(),
        grid_resolution: 30,
    }));
    serialize_world_state(world, 7, &chronicle, &lineage, &evolution, &grid)
}

// ---------------------------------------------------------------------------
// FD-S01 + F1-5 — hibernate đóng sổ tại một sync point, không qua detritus
// ---------------------------------------------------------------------------

#[test]
fn fd_s01_hibernate_chuyen_ngan_va_despawn_tai_mot_sync_point() {
    let (mut world, root) = dormancy_world(50.0);
    let before = closed_total(&mut world);

    HibernateAgentCommand { root }.apply(&mut world);

    // Despawn phải xảy ra NGAY trong apply — nếu đợi Commands deferred thì có
    // một cửa sổ cả tick mà thân đã bị bank vẫn còn sống, đúng lỗi hai lần ghi
    // mà §3 hợp đồng ledger cấm.
    assert!(
        world.get_entity(root).is_none(),
        "FD-S01: hibernate phải despawn tức thời tại cùng sync point với việc chuyển ngăn"
    );

    let pool = *world.resource::<EcosystemBiomass>();
    assert_eq!(
        pool.dormant, 50.0,
        "F1-2: ngăn dormant nhận chính xác số EU reserve đã mất (f64 đo được)"
    );
    assert_eq!(
        pool.detritus, 0.0,
        "F1-5: ngủ đông KHÔNG đi qua detritus — đây không phải cái chết"
    );

    let store = world.resource::<DormantAgents>();
    assert_eq!(store.0.len(), 1, "đúng một bản ghi ngủ đông");
    assert_eq!(
        store.0[0].banked_eu, 50.0,
        "bản ghi phải mang số EU đã bank để wake hoàn lại đúng số đó"
    );
    assert_eq!(
        store.0[0].agent.homeostatic_state.energy, 0.0,
        "reserve phải nằm ở ĐÚNG MỘT nơi — bản ghi giữ 0 để census không đếm kép"
    );

    let after = closed_total(&mut world);
    assert!(
        (after - before).abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "FD-S01: hibernate dịch chuyển closed-EU bởi {} — EU đang chạy ngoài EnergyLedger",
        after - before
    );
}

#[test]
fn fd_s01_wake_hoan_ngan_va_residue_dong_so() {
    let (mut world, root) = dormancy_world(50.0);
    let before = closed_total(&mut world);

    HibernateAgentCommand { root }.apply(&mut world);
    WakeAgentCommand {
        index: 0,
        at: glam::Vec3::new(3.0, 1.0, -2.0),
    }
    .apply(&mut world);

    assert_eq!(
        world.resource::<EcosystemBiomass>().dormant,
        0.0,
        "wake phải rút cạn ngăn dormant về 0 — đó là con đường ra duy nhất"
    );
    let mut q = world.query_filtered::<&HomeostaticState, With<Agent>>();
    let reserves: Vec<f32> = q.iter(&world).map(|h| h.energy).collect();
    assert_eq!(reserves.len(), 1, "đúng một cá thể thức dậy");
    assert_eq!(
        reserves[0], 50.0,
        "F1-3: reserve được hoàn nguyên vẹn — không clip, không mất ULP"
    );

    let after = closed_total(&mut world);
    assert!(
        (after - before).abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "một vòng hibernate→wake dịch chuyển closed-EU bởi {}",
        after - before
    );
}

#[test]
fn fd_s03_wake_chi_so_ngoai_kho_khong_dong_su_eu() {
    let (mut world, _root) = dormancy_world(50.0);
    let before = closed_total(&mut world);

    WakeAgentCommand {
        index: 0,
        at: glam::Vec3::ZERO,
    }
    .apply(&mut world);

    assert!(
        world
            .get_resource::<DormantAgents>()
            .is_none_or(|s| s.0.is_empty()),
        "chỉ số ngoài kho không được tạo bản ghi nào"
    );
    assert_eq!(
        world.resource::<EcosystemBiomass>().dormant,
        0.0,
        "lệnh wake vô hiệu không được đụng ngăn dormant"
    );
    let after = closed_total(&mut world);
    assert!((after - before).abs() <= RESIDUAL_ABS_TOLERANCE_EU);
}

// ---------------------------------------------------------------------------
// FD-S02 — wake KHÔNG develop lại (D01/D02)
// ---------------------------------------------------------------------------

#[test]
fn fd_s02_wake_khong_develop_lai_phenotype_vao_brain_giu_nguyen() {
    let original = payload(Some(brain(71)));
    let mut world = init_world();
    let root = spawn_serialized_agent(&mut world, &original.clone()).expect("valid payload");
    HibernateAgentCommand { root }.apply(&mut world);

    let record = world.resource::<DormantAgents>().0[0].clone();

    WakeAgentCommand {
        index: 0,
        at: glam::Vec3::new(3.0, 1.0, -2.0),
    }
    .apply(&mut world);

    let mut q = world.query_filtered::<Entity, With<Agent>>();
    let agents: Vec<Entity> = q.iter(&world).collect();
    assert_eq!(agents.len(), 1, "đúng một cá thể sống lại");
    let new_root = agents[0];
    assert_ne!(
        new_root, root,
        "phải là entity mới dựng từ phenotype đã lưu"
    );

    // Vị trí đánh thức là tham số `at`.
    assert_eq!(
        world.get::<Position>(new_root).unwrap().0,
        glam::Vec3::new(3.0, 1.0, -2.0)
    );

    // D02 — não du hành cùng cá thể: cùng một object, không phải não mới roll.
    assert_eq!(
        world.get::<AgentBrain>(new_root).unwrap(),
        original.brain.as_ref().unwrap(),
        "D01/D02: wake phải giữ nguyên brain — não mới nghĩa là một sinh vật khác mặc cùng lineage id"
    );

    // Genotype bit-exact qua JSON (MorphologyGenotype không có PartialEq).
    let woken_genotype = world.get::<AgentGenotype>(new_root).unwrap();
    assert_eq!(
        serde_json::to_string(&woken_genotype.0).unwrap(),
        serde_json::to_string(&original.genotype).unwrap()
    );
    assert_eq!(
        world.get::<AgentLineageId>(new_root).unwrap().0,
        record.agent.lineage_id,
        "lineage id phải đi theo cá thể"
    );

    // Trạng thái segment giữ nguyên so với bản ghi chụp lúc ngủ.
    let mut live_q = world.query::<(
        Entity,
        &Segment,
        &Position,
        &Rotation,
        &Velocity,
        &ParentAgent,
    )>();
    let mut live: Vec<(u32, [f32; 3])> = live_q
        .iter(&world)
        .filter(|(e, _, _, _, _, pa)| *e != new_root && pa.0 == new_root)
        .map(|(_e, s, p, _, _, _)| (s.id, p.0.to_array()))
        .collect();
    live.sort_by_key(|e| e.0);
    let mut saved: Vec<(u32, [f32; 3])> = record
        .agent
        .segments
        .iter()
        .map(|s| (s.segment_id, s.position.to_array()))
        .collect();
    saved.sort_by_key(|e| e.0);
    assert_eq!(
        live, saved,
        "D02: trạng thái segment sau wake phải khớp bit-exact bản ghi lúc hibernate"
    );
}

// ---------------------------------------------------------------------------
// FD-S03 — hibernate → save → load → wake
// ---------------------------------------------------------------------------

#[test]
fn fd_s03_hibernate_save_load_wake_khong_mat_mot_eu_nao() {
    let (mut world, root) = dormancy_world(50.0);
    let before = closed_total(&mut world);

    HibernateAgentCommand { root }.apply(&mut world);

    // Đường save thật, rồi JSON roundtrip như envelope snapshot vận chuyển.
    let state = save(&mut world);
    let json = serde_json::to_string(&state).expect("SavedSimulationState phải serialize được");
    let state: SavedSimulationState = serde_json::from_str(&json).expect("JSON roundtrip");

    assert_eq!(
        state.eco_dormant, 50.0,
        "F1-4: ngăn dormant phải đi qua biên save/load"
    );
    assert_eq!(
        state.dormant_agents.len(),
        1,
        "FD-S03: agent dormant không được biến mất qua biên save/load"
    );

    // Load vào thế giới mới — đúng thứ tự đường restore thật.
    let mut restored = init_world();
    for agent in &state.agents {
        spawn_serialized_agent(&mut restored, agent).expect("checkpoint agent");
    }
    restored.insert_resource(DormantAgents(state.dormant_agents.clone()));
    restore_energy_state(&mut restored, &state);

    let mid = closed_total(&mut restored);
    assert!(
        (mid - before).abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "save/load với một agent đang ngủ đã dịch chuyển closed-EU bởi {}",
        mid - before
    );

    // Thức dậy sau khi tải lại — vòng đời trọn vẹn vẫn đóng sổ.
    WakeAgentCommand {
        index: 0,
        at: state.dormant_agents[0].agent.root_position,
    }
    .apply(&mut restored);

    let after = closed_total(&mut restored);
    assert!(
        (after - before).abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "hibernate→save→load→wake dịch chuyển closed-EU bởi {} (tolerance {})",
        after - before,
        RESIDUAL_ABS_TOLERANCE_EU
    );
    assert_eq!(restored.resource::<EcosystemBiomass>().dormant, 0.0);
}
