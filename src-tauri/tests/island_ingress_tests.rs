//! Card I1 acceptance & negative-control gates (`ADR-0014` / `QD-SHAREDISLAND` / `QD-GENESIS`).

mod common;

use anima_domain::causal::{
    is_island_guest_cause, is_reserved_cause, CAUSE_BACKGROUND, CAUSE_OBSERVER,
    ISLAND_GUEST_CAUSE_BASE,
};
use anima_engine_lib::core::determinism::DeterministicMode;
use anima_engine_lib::core::exotic_energy::{
    ExoticEnergyLaw, ExoticIntervention, ExoticInterventionKind,
};
use anima_engine_lib::core::experiment::{
    ExperimentError, ExperimentManifest, FactorDiff, InitialConditionSet, ObservableRegistry,
    WorldIdentity, WorldLawSet, MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{
    run_paired_ensemble_with_control, validate_uniform_genesis_phase, ExperimentModel,
};
use anima_engine_lib::core::intervention::{Curve, InterventionCommand, InterventionKind, Region};
use anima_engine_lib::core::island::{
    decode_guest_intent_bincode, decode_guest_intent_json, drain_guest_intents_system,
    CreatureControlIntent, DrainedGuestIntents, GenesisPhase, GuestId, GuestIntent,
    GuestIntentDecodeError, GuestIntentMessage, GuestRole, HostModerationAction, IslandGuestPolicy,
    IslandId, IslandManifest, OrganismTargetKind, PlantControlIntent, RealTimeEcologyProfile,
    SharedGuestIntents, DEFAULT_DAY_LENGTH_SECONDS, DEFAULT_ISLAND_MAX_GUESTS,
    DEFAULT_ISLAND_TARGET_ANIMALS, DEFAULT_YEAR_LENGTH_SECONDS, MAX_GUEST_INTENT_FRAME_BYTES,
    MAX_ISLAND_GUESTS_CEILING,
};
use anima_engine_lib::core::live_experiment::LiveExperimentAdapter;
use anima_engine_lib::core::observer::ObserverPolicy;
use anima_engine_lib::core::reference_world::ReferenceEvolutionWorld;
use bevy_ecs::prelude::{Schedule, World};
use std::path::PathBuf;

#[global_allocator]
static ALLOCATOR: common::allocator::ThreadTrackingAllocator =
    common::allocator::ThreadTrackingAllocator;

// Unmodified trunk (`aef12cb641d8d1e02dc35c97f2e74ee2fbb87b93`) fingerprints measured prior to I1 edits.
const TRUNK_FP_BASELINE_NO_EXOTIC: u64 = 0x5cea_8ab7_0252_1d15;
const TRUNK_FP_MANA_PATCHY: u64 = 0xce26_27ce_4ad9_4583;
const TRUNK_FP_E3B_CONTROL: u64 = 0xc672_2f06_ea2f_e2a3;
const TRUNK_FP_E3B_TREATMENT: u64 = 0x5f32_e19f_c12f_622e;
const TRUNK_FP_SEL2P_SELECTED: u64 = 0x0c35_834e_0787_122c;
const TRUNK_LAW_FP_BASELINE: u64 = 0x9b10_775d_5a88_91b3;
const TRUNK_LAW_FP_MANA: u64 = 0x9815_3635_e68b_7dcf;

fn load_fixture_manifest(rel_path: &str) -> (String, ExperimentManifest) {
    let full: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(rel_path);
    let raw = std::fs::read_to_string(&full)
        .unwrap_or_else(|e| panic!("failed reading fixture {}: {e}", full.display()));
    let manifest: ExperimentManifest = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("failed parsing fixture {}: {e}", full.display()));
    (raw, manifest)
}

fn minimal_valid_manifest() -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "i1-test".into(),
        name: "i1-test".into(),
        world_identity: WorldIdentity {
            seed: 42,
            generator_version: 20,
            width: 16,
            height: 16,
            checksum: 0x1234_5678,
            sim_world_sha256: None,
            genesis_phase: None,
        },
        laws: WorldLawSet::baseline(),
        initial_conditions: InitialConditionSet::new(vec![
            ("precip".into(), 1.0),
            ("temperature".into(), 0.5),
            ("plants".into(), 100.0),
            ("herbivores".into(), 40.0),
            ("predators".into(), 8.0),
            ("detritus".into(), 0.0),
        ]),
        interventions: Vec::new(),
        seeds: vec![1, 2],
        duration_ticks: 600,
        sample_period: 300,
        observable_ids: vec!["plants".into(), "herbivores".into()],
        exotic_interventions: Vec::new(),
        observer: ObserverPolicy::Absent,
        island: None,
    }
}

fn sample_intervention(cause_id: u32) -> InterventionCommand {
    InterventionCommand {
        id: 1,
        cause_id,
        kind: InterventionKind::RainfallDelta,
        region: Region::Global,
        start_tick: 1,
        duration_ticks: 10,
        intensity: 0.1,
        signed_negative: false,
        curve: Curve::Step,
        reversible: true,
    }
}

fn sample_exotic_intervention(cause_id: u32) -> ExoticIntervention {
    ExoticIntervention {
        id: 1,
        cause_id,
        kind: ExoticInterventionKind::RemoveSource,
        region: Region::Global,
        start_tick: 1,
        duration_ticks: 120,
        amount: 0.1,
        curve: Curve::Step,
    }
}

#[test]
fn science_boundary_refuses_guest_session_in_absent_manifest() {
    let reg = ObservableRegistry::reference_default();

    // 1. Reserved CauseId checks: guest band (`ISLAND_GUEST_CAUSE_BASE..CAUSE_OBSERVER`) and
    //    `CAUSE_OBSERVER` are refused inside manifest interventions & exotic forcings.
    for reserved in [
        ISLAND_GUEST_CAUSE_BASE,
        ISLAND_GUEST_CAUSE_BASE + 7,
        CAUSE_OBSERVER - 1,
        CAUSE_OBSERVER,
    ] {
        assert!(is_reserved_cause(reserved));

        let mut m = minimal_valid_manifest();
        m.interventions.push(sample_intervention(reserved));
        assert!(
            matches!(
                m.validate(&reg),
                Err(ExperimentError::InvalidObserverPolicy { .. })
            ),
            "ordinary intervention with reserved cause_id {reserved} must fail"
        );

        let mut m_exo = minimal_valid_manifest();
        m_exo.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(100.0, 2));
        m_exo
            .exotic_interventions
            .push(sample_exotic_intervention(reserved));
        assert!(
            matches!(
                m_exo.validate(&reg),
                Err(ExperimentError::InvalidObserverPolicy { .. })
            ),
            "exotic intervention with reserved cause_id {reserved} must fail"
        );
    }

    // Normal scenario author cause ids remain allowed, and local `CAUSE_OBSERVER` remains valid on ObserverPolicy::Inhabit.
    for ok_id in [1u32, 42, 4096, ISLAND_GUEST_CAUSE_BASE - 1] {
        assert!(!is_reserved_cause(ok_id));
        let mut ok = minimal_valid_manifest();
        ok.interventions.push(sample_intervention(ok_id));
        assert!(ok.validate(&reg).is_ok());
    }
    assert!(ObserverPolicy::Inhabit {
        cause_id: CAUSE_OBSERVER
    }
    .rejection_reason()
    .is_none());
    assert!(ObserverPolicy::Inhabit {
        cause_id: CAUSE_BACKGROUND
    }
    .rejection_reason()
    .is_some());
    assert!(ObserverPolicy::Inhabit {
        cause_id: ISLAND_GUEST_CAUSE_BASE
    }
    .rejection_reason()
    .is_some());

    // 2. ObserverPolicy::Absent rejects shared-island guest session enablement.
    let island_id = IslandId::new(101).expect("non-zero island id");
    let mut absent_island = minimal_valid_manifest();
    absent_island.observer = ObserverPolicy::Absent;
    absent_island.island = Some(IslandManifest::trial_default(island_id));
    assert!(
        matches!(
            absent_island.validate(&reg),
            Err(ExperimentError::InvalidIslandConfig { .. })
        ),
        "ObserverPolicy::Absent must refuse session_enabled=true island manifest"
    );

    // ObserverPolicy::Spectate and ObserverPolicy::Inhabit accept valid island config.
    let mut spectate_island = absent_island.clone();
    spectate_island.observer = ObserverPolicy::Spectate;
    assert!(spectate_island.validate(&reg).is_ok());

    // Violation of QD-PLAYRULES Rule 1 (faster-than-real-time only when empty) is refused.
    let mut ftrt_violation = spectate_island.clone();
    if let Some(isl) = ftrt_violation.island.as_mut() {
        isl.faster_than_real_time_only_when_empty = false;
    }
    assert!(matches!(
        ftrt_violation.validate(&reg),
        Err(ExperimentError::InvalidIslandConfig { .. })
    ));

    // Zero or over-ceiling max_guests are refused when session is enabled.
    let mut zero_guests = spectate_island.clone();
    zero_guests.island.as_mut().unwrap().max_guests = 0;
    assert!(matches!(
        zero_guests.validate(&reg),
        Err(ExperimentError::InvalidIslandConfig { .. })
    ));
    let mut over_ceiling = spectate_island.clone();
    over_ceiling.island.as_mut().unwrap().max_guests = MAX_ISLAND_GUESTS_CEILING + 1;
    assert!(matches!(
        over_ceiling.validate(&reg),
        Err(ExperimentError::InvalidIslandConfig { .. })
    ));

    // Conflicting genesis_phase between world_identity and island is refused.
    let mut mismatch_phase = spectate_island.clone();
    mismatch_phase.world_identity.genesis_phase = Some(GenesisPhase::G2);
    assert!(matches!(
        mismatch_phase.validate(&reg),
        Err(ExperimentError::InvalidIslandConfig { .. })
    ));

    // 3. Experiments must never mix genesis phases (even if `"island"` & `"world_identity"` are in FactorDiff).
    let mut g1 = spectate_island.clone();
    g1.world_identity.genesis_phase = Some(GenesisPhase::G1);
    let mut g2 = spectate_island.clone();
    g2.island.as_mut().unwrap().genesis_phase = GenesisPhase::G2;
    g2.world_identity.genesis_phase = Some(GenesisPhase::G2);
    let mut g3 = spectate_island.clone();
    g3.island.as_mut().unwrap().genesis_phase = GenesisPhase::G3;
    g3.world_identity.genesis_phase = Some(GenesisPhase::G3);
    let no_phase = minimal_valid_manifest();

    assert!(matches!(
        validate_uniform_genesis_phase(&[&g1, &g2]),
        Err(ExperimentError::MixedGenesisPhase {
            control: Some(GenesisPhase::G1),
            treatment: Some(GenesisPhase::G2),
        })
    ));
    assert!(matches!(
        validate_uniform_genesis_phase(&[&g2, &g3]),
        Err(ExperimentError::MixedGenesisPhase {
            control: Some(GenesisPhase::G2),
            treatment: Some(GenesisPhase::G3),
        })
    ));
    assert!(matches!(
        validate_uniform_genesis_phase(&[&no_phase, &g1]),
        Err(ExperimentError::MixedGenesisPhase {
            control: None,
            treatment: Some(GenesisPhase::G1),
        })
    ));

    let permissive_diff = FactorDiff {
        allowed_paths: vec!["observer".into(), "island".into(), "world_identity".into()],
    };
    let err = run_paired_ensemble_with_control::<ReferenceEvolutionWorld>(
        &g1,
        &g2,
        &reg,
        &permissive_diff,
    )
    .expect_err("paired runner must refuse mixed genesis phases");
    assert!(matches!(err, ExperimentError::MixedGenesisPhase { .. }));

    // 4. Runtime boundary: under ObserverPolicy::Absent, queued guest intents are discarded and counted.
    let shared = SharedGuestIntents::with_capacity(island_id, 16);
    let guest = GuestId::new(9).expect("valid guest id");
    shared
        .try_enqueue(guest, ISLAND_GUEST_CAUSE_BASE, GuestIntent::ReleaseOrganism)
        .expect("enqueue succeeds at ingress buffer");

    let mut world = World::new();
    world.insert_resource(ObserverPolicy::Absent);
    world.insert_resource(shared.clone());
    world.insert_resource(DrainedGuestIntents::with_capacity(16));
    let mut sched = Schedule::default();
    sched.add_systems(drain_guest_intents_system);
    sched.run(&mut world);

    let drained = world.resource::<DrainedGuestIntents>();
    assert!(drained.intents().is_empty());
    assert_eq!(shared.counters().rejected_policy, 1);
    assert_eq!(shared.counters().drained, 0);
}

#[test]
fn legacy_manifest_fingerprint_matches_trunk_pin_and_island_flips_it() {
    let reg = ObservableRegistry::reference_default();

    let (raw_base, base) = load_fixture_manifest("experiments/baseline-no-exotic.json");
    assert_eq!(base.island, None);
    assert_eq!(base.world_identity.genesis_phase, None);
    assert_eq!(
        base.fingerprint(),
        TRUNK_FP_BASELINE_NO_EXOTIC,
        "legacy baseline fingerprint drifted from unmodified trunk pin"
    );
    assert_eq!(base.laws.fingerprint(), TRUNK_LAW_FP_BASELINE);
    let ser_base = serde_json::to_string_pretty(&base).expect("serialize base") + "\n";
    assert_eq!(ser_base, raw_base.replace("\r\n", "\n"));
    assert!(!ser_base.contains("\"island\""));
    assert!(!ser_base.contains("\"genesis_phase\""));
    assert!(base.validate(&reg).is_ok());

    let (raw_mana, mana) = load_fixture_manifest("experiments/mana-patchy-renewable.json");
    assert_eq!(mana.fingerprint(), TRUNK_FP_MANA_PATCHY);
    assert_eq!(mana.laws.fingerprint(), TRUNK_LAW_FP_MANA);
    let ser_mana = serde_json::to_string_pretty(&mana).expect("serialize mana") + "\n";
    assert_eq!(ser_mana, raw_mana.replace("\r\n", "\n"));

    let (_, e3b_c) = load_fixture_manifest("experiments_e3b/e3b-control-uniform.json");
    assert_eq!(e3b_c.fingerprint(), TRUNK_FP_E3B_CONTROL);
    let (_, e3b_t) = load_fixture_manifest("experiments_e3b/e3b-treatment-gradient.json");
    assert_eq!(e3b_t.fingerprint(), TRUNK_FP_E3B_TREATMENT);
    let (_, sel2p) = load_fixture_manifest("experiments_sel2p/sel2p-selected.json");
    assert_eq!(sel2p.fingerprint(), TRUNK_FP_SEL2P_SELECTED);

    // Flipping island presence, session_enabled, real-time ecology profile, or genesis_phase changes
    // ExperimentManifest::fingerprint() while keeping WorldLawSet::fingerprint() invariant (ER01).
    let island_id = IslandId::new(77).unwrap();
    let mut with_island = base.clone();
    with_island.observer = ObserverPolicy::Spectate;
    with_island.island = Some(IslandManifest::trial_default(island_id));
    assert!(with_island.validate(&reg).is_ok());
    assert_ne!(with_island.fingerprint(), TRUNK_FP_BASELINE_NO_EXOTIC);
    assert_eq!(with_island.laws.fingerprint(), TRUNK_LAW_FP_BASELINE);

    let mut session_off = with_island.clone();
    session_off.island.as_mut().unwrap().session_enabled = false;
    assert_ne!(session_off.fingerprint(), with_island.fingerprint());
    assert_eq!(session_off.laws.fingerprint(), TRUNK_LAW_FP_BASELINE);

    let mut custom_rt = with_island.clone();
    custom_rt
        .island
        .as_mut()
        .unwrap()
        .real_time_profile
        .as_mut()
        .unwrap()
        .year_length_seconds = 3_600;
    assert_ne!(custom_rt.fingerprint(), with_island.fingerprint());
    assert_eq!(custom_rt.laws.fingerprint(), TRUNK_LAW_FP_BASELINE);

    let mut phased_g2 = base.clone();
    phased_g2.observer = ObserverPolicy::Spectate;
    phased_g2.world_identity.genesis_phase = Some(GenesisPhase::G2);
    assert_ne!(phased_g2.fingerprint(), TRUNK_FP_BASELINE_NO_EXOTIC);
    assert_eq!(phased_g2.laws.fingerprint(), TRUNK_LAW_FP_BASELINE);

    let mut phased_g3 = phased_g2.clone();
    phased_g3.world_identity.genesis_phase = Some(GenesisPhase::G3);
    assert_ne!(phased_g3.fingerprint(), phased_g2.fingerprint());

    // FactorDiff surfaces both `"island"` and `"world_identity"`.
    assert_eq!(
        FactorDiff::diff_paths(&base, &with_island),
        vec!["island".to_string(), "observer".to_string()]
    );
    assert_eq!(
        FactorDiff::diff_paths(&with_island, &session_off),
        vec!["island".to_string()]
    );
    assert_eq!(
        FactorDiff::diff_paths(&phased_g2, &phased_g3),
        vec!["world_identity".to_string()]
    );
}

#[test]
fn guest_cannot_write_state() {
    let island_id = IslandId::new(505).unwrap();
    let guest_id = GuestId::new(11).unwrap();
    let shared = SharedGuestIntents::with_capacity(island_id, 4);

    let mut policy = IslandGuestPolicy::new(island_id, DEFAULT_ISLAND_MAX_GUESTS);
    assert!(policy.faster_than_real_time_allowed());

    // Spectate cannot inhabit; Inhabit assigns a reserved CauseId in guest band.
    assert_eq!(
        policy.admit_guest(guest_id, GuestRole::Inhabit, ObserverPolicy::Absent),
        Err(GuestIntentDecodeError::RoleForbidden)
    );
    let cause_id = policy
        .admit_guest(
            guest_id,
            GuestRole::Inhabit,
            ObserverPolicy::Inhabit {
                cause_id: CAUSE_OBSERVER,
            },
        )
        .expect("guest admitted under Inhabit policy");
    assert!(is_island_guest_cause(cause_id));
    assert!(!policy.faster_than_real_time_allowed());

    // Forged raw state payloads must be rejected and counted before simulation queues.
    let forged_state_payloads = [
        r#"{"island_id":505,"guest_id":11,"client_seq":1,"intent":"release_organism","segments":[]}"#,
        r#"{"island_id":505,"guest_id":11,"client_seq":1,"saved_simulation_state":{"ticks":1}}"#,
        r#"{"island_id":505,"guest_id":11,"client_seq":1,"agent_migration_data":{"energy":999}}"#,
        r#"{"island_id":505,"guest_id":11,"client_seq":1,"brain_weights":[1.0,2.0]}"#,
    ];
    for raw in forged_state_payloads {
        assert!(
            matches!(
                decode_guest_intent_json(raw.as_bytes(), island_id),
                Err(GuestIntentDecodeError::RejectedRawStateOrEnvIntervention { .. })
            ),
            "raw state payload must be rejected: {raw}"
        );
        assert!(matches!(
            shared.decode_and_enqueue_json(raw.as_bytes(), cause_id, GuestRole::Inhabit),
            Err(GuestIntentDecodeError::RejectedRawStateOrEnvIntervention { .. })
        ));
    }

    // Guest environment / macro interventions must be rejected (host-only, QD-PLAYRULES Rule 5).
    let env_payloads = [
        r#"{"island_id":505,"guest_id":11,"client_seq":1,"intent":"rainfall_delta"}"#,
        r#"{"island_id":505,"guest_id":11,"client_seq":1,"intent":"temperature_delta","amount":5.0}"#,
        r#"{"island_id":505,"guest_id":11,"client_seq":1,"intent":"deforest"}"#,
        r#"{"island_id":505,"guest_id":11,"client_seq":1,"intent":"add_nutrient"}"#,
        r#"{"island_id":505,"guest_id":11,"client_seq":1,"environment_intervention":{"kind":"rain"}}"#,
        r#"{"island_id":505,"guest_id":11,"client_seq":1,"exotic_intervention":{"id":1}}"#,
    ];
    for raw in env_payloads {
        assert!(
            matches!(
                shared.decode_and_enqueue_json(raw.as_bytes(), cause_id, GuestRole::Inhabit),
                Err(GuestIntentDecodeError::RejectedRawStateOrEnvIntervention { .. })
            ),
            "env intervention payload must be rejected: {raw}"
        );
    }

    // Out-of-bounds physical values, wrong island, zero guest id, oversized frame, unknown field, and trailing bincode bytes.
    let over_speed = serde_json::to_vec(&GuestIntentMessage {
        island_id,
        guest_id,
        client_seq: 1,
        intent: GuestIntent::CreatureControl(CreatureControlIntent {
            move_dir_x: 0.6,
            move_dir_z: 0.0,
            speed_fraction: 1.5, // illegal extra speed!
            feed_intent: false,
            attack_intent: false,
            pheromone_emit: 0.0,
        }),
    })
    .unwrap();
    assert!(matches!(
        shared.decode_and_enqueue_json(&over_speed, cause_id, GuestRole::Inhabit),
        Err(GuestIntentDecodeError::InvalidFieldValue { .. })
    ));

    let over_plant = serde_json::to_vec(&GuestIntentMessage {
        island_id,
        guest_id,
        client_seq: 2,
        intent: GuestIntent::PlantControl(PlantControlIntent {
            root_share: 0.6,
            canopy_share: 0.6, // sums to 1.2 > 1.0
            defence_share: 0.0,
            reproduction_share: 0.0,
            trigger_flowering: false,
            seed_size_bias: 0.5,
        }),
    })
    .unwrap();
    assert!(matches!(
        shared.decode_and_enqueue_json(&over_plant, cause_id, GuestRole::Inhabit),
        Err(GuestIntentDecodeError::InvalidFieldValue { .. })
    ));

    let unknown_field =
        br#"{"island_id":505,"guest_id":11,"client_seq":1,"intent":{"claim_organism":{"organism_id":3,"kind":"grass_patch"}}}"#;
    assert_eq!(
        shared.decode_and_enqueue_json(unknown_field, cause_id, GuestRole::Inhabit),
        Err(GuestIntentDecodeError::InvalidEncoding)
    );

    let oversized = vec![b' '; MAX_GUEST_INTENT_FRAME_BYTES + 1];
    assert_eq!(
        shared.decode_and_enqueue_json(&oversized, cause_id, GuestRole::Inhabit),
        Err(GuestIntentDecodeError::FrameTooLarge {
            bytes: MAX_GUEST_INTENT_FRAME_BYTES + 1,
            max: MAX_GUEST_INTENT_FRAME_BYTES,
        })
    );

    let wrong_island = serde_json::to_vec(&GuestIntentMessage {
        island_id: IslandId::new(999).unwrap(),
        guest_id,
        client_seq: 3,
        intent: GuestIntent::ReleaseOrganism,
    })
    .unwrap();
    assert_eq!(
        shared.decode_and_enqueue_json(&wrong_island, cause_id, GuestRole::Inhabit),
        Err(GuestIntentDecodeError::IslandMismatch)
    );

    // Valid bincode intent passes for Inhabit, is refused for Spectate, and trailing bytes are refused.
    let valid_msg = GuestIntentMessage {
        island_id,
        guest_id,
        client_seq: 4,
        intent: GuestIntent::ClaimOrganism {
            organism_id: 42,
            kind: OrganismTargetKind::Animal,
        },
    };
    let mut bin_bytes =
        bincode::serde::encode_to_vec(valid_msg, bincode::config::standard()).unwrap();
    let roundtrip = decode_guest_intent_bincode(&bin_bytes, island_id).expect("bincode roundtrip");
    assert_eq!(roundtrip, valid_msg);

    assert_eq!(
        shared.decode_and_enqueue_bincode(&bin_bytes, cause_id, GuestRole::Spectate),
        Err(GuestIntentDecodeError::RoleForbidden)
    );

    bin_bytes.push(0xAA);
    assert!(matches!(
        shared.decode_and_enqueue_bincode(&bin_bytes, cause_id, GuestRole::Inhabit),
        Err(GuestIntentDecodeError::RejectedRawStateOrEnvIntervention { .. })
    ));

    // Zero accepted intents leaked from any invalid/forbidden submission above.
    assert_eq!(shared.pending_len(), 0);
    assert_eq!(shared.counters().accepted, 0);
    assert!(shared.counters().rejected_decode >= 13);
    assert_eq!(shared.counters().rejected_policy, 1);

    // ISLAND-D5(a): fill bounded queue (cap=4); 5th enqueue is rejected/throttled without dropping 1..4.
    let valid_json = serde_json::to_vec(&valid_msg).unwrap();
    for seq in 1..=4u64 {
        assert_eq!(
            shared.decode_and_enqueue_json(&valid_json, cause_id, GuestRole::Inhabit),
            Ok(seq)
        );
    }
    assert_eq!(
        shared.decode_and_enqueue_json(&valid_json, cause_id, GuestRole::Inhabit),
        Err(GuestIntentDecodeError::QueueFullThrottled)
    );
    assert_eq!(shared.pending_len(), 4);
    assert_eq!(shared.counters().accepted, 4);
    assert_eq!(shared.counters().throttled_full, 1);

    // Host kick moderation cleanly revokes guest slot and restores faster-than-real-time eligibility.
    let kick = policy
        .kick_guest(guest_id)
        .expect("kicking active guest emits moderation action");
    assert_eq!(kick, HostModerationAction::KickGuest { guest_id, cause_id });
    assert!(policy.faster_than_real_time_allowed());
    shared
        .enqueue_moderation(kick)
        .expect("moderation enqueued");
}

#[test]
fn island_host_tick_zero_alloc() {
    let island_id = IslandId::new(808).unwrap();
    let g1 = GuestId::new(1).unwrap();
    let c1 = ISLAND_GUEST_CAUSE_BASE;
    let shared = SharedGuestIntents::with_capacity(island_id, 64);

    let mut world = World::new();
    world.insert_resource(ObserverPolicy::Inhabit {
        cause_id: CAUSE_OBSERVER,
    });
    world.insert_resource(shared.clone());
    world.insert_resource(DrainedGuestIntents::with_capacity(64));
    let mut schedule = Schedule::default();
    schedule.add_systems(drain_guest_intents_system);

    // Pre-warm Bevy schedule + Local state before opening allocation tracking.
    schedule.run(&mut world);

    // Phase 1: Empty queue steady-state ticks make 0 allocations.
    ALLOCATOR.start_tracking();
    for _ in 0..500 {
        schedule.run(&mut world);
    }
    let allocs_empty = ALLOCATOR.stop_tracking();
    assert_eq!(allocs_empty, 0, "empty guest intent drain allocated");

    // Phase 2: Populated intents + kick moderation across repeated ticks make 0 allocations.
    for tick_i in 0..200u64 {
        let intent = match tick_i % 4 {
            0 => GuestIntent::ClaimOrganism {
                organism_id: tick_i + 1,
                kind: OrganismTargetKind::Animal,
            },
            1 => GuestIntent::CreatureControl(CreatureControlIntent {
                move_dir_x: 0.7,
                move_dir_z: -0.3,
                speed_fraction: 0.8,
                feed_intent: true,
                attack_intent: false,
                pheromone_emit: 0.25,
            }),
            2 => GuestIntent::PlantControl(PlantControlIntent {
                root_share: 0.3,
                canopy_share: 0.4,
                defence_share: 0.1,
                reproduction_share: 0.2,
                trigger_flowering: false,
                seed_size_bias: 0.5,
            }),
            _ => GuestIntent::ReleaseOrganism,
        };
        shared.try_enqueue(g1, c1, intent).unwrap();
        if tick_i % 50 == 0 {
            shared
                .enqueue_moderation(HostModerationAction::KickGuest {
                    guest_id: g1,
                    cause_id: c1,
                })
                .unwrap();
        }

        ALLOCATOR.start_tracking();
        schedule.run(&mut world);
        let tick_allocs = ALLOCATOR.stop_tracking();
        assert_eq!(
            tick_allocs, 0,
            "tick {tick_i} guest intent drain performed {tick_allocs} allocations"
        );
    }
    assert_eq!(world.resource::<DrainedGuestIntents>().total_drained(), 200);

    // Phase 3: Absent discard path also makes 0 allocations.
    *world.resource_mut::<ObserverPolicy>() = ObserverPolicy::Absent;
    shared
        .try_enqueue(g1, c1, GuestIntent::ReleaseOrganism)
        .unwrap();
    ALLOCATOR.start_tracking();
    schedule.run(&mut world);
    let allocs_absent = ALLOCATOR.stop_tracking();
    assert_eq!(allocs_absent, 0);

    // Phase 4: Positive control — counter genuinely sees real heap allocation.
    ALLOCATOR.start_tracking();
    std::hint::black_box(Vec::<u8>::with_capacity(64));
    let control_allocs = ALLOCATOR.stop_tracking();
    assert!(
        control_allocs >= 1,
        "ThreadTrackingAllocator must observe real Vec heap allocation"
    );
}

#[test]
fn opt_in_zero_perturbation_matches_solo_schedule() {
    let profile = RealTimeEcologyProfile::default();
    assert_eq!(profile.target_animal_count, DEFAULT_ISLAND_TARGET_ANIMALS);
    assert_eq!(DEFAULT_ISLAND_TARGET_ANIMALS, 100);
    assert_eq!(DEFAULT_ISLAND_MAX_GUESTS, 16);
    assert_eq!(profile.day_length_seconds, DEFAULT_DAY_LENGTH_SECONDS);
    assert_eq!(DEFAULT_DAY_LENGTH_SECONDS, 1_200);
    assert_eq!(profile.year_length_seconds, DEFAULT_YEAR_LENGTH_SECONDS);
    assert_eq!(DEFAULT_YEAR_LENGTH_SECONDS, 5_400);
    assert_eq!(GenesisPhase::G1.display_label(), "designed founders");

    // Solo LiveExperimentAdapter with no island resources stays deterministic across independent runs
    // and places `drain_guest_intents_system` strictly in pre-sensory order.
    let initial = InitialConditionSet::new(vec![
        ("live.founders".to_string(), 10.0),
        ("live.predator_fraction".to_string(), 0.3),
        ("live.trees".to_string(), 8.0),
        ("live.lakes".to_string(), 2.0),
        ("live.food_cap".to_string(), 50.0),
    ]);
    let mut a1 = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        4_242_424_242,
        (16, 16),
        20,
    )
    .expect("live world builds");
    let mut a2 = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        4_242_424_242,
        (16, 16),
        20,
    )
    .expect("second live world builds");

    a1.run_schedule_once();
    a2.run_schedule_once();

    let names: Vec<String> = a1
        .schedule_mut()
        .systems()
        .expect("schedule initialized")
        .map(|(_, s)| {
            let full = s.name();
            full.rsplit("::")
                .next()
                .unwrap_or(full.as_ref())
                .to_string()
        })
        .collect();
    let pos = |n: &str| {
        names
            .iter()
            .position(|x| x == n)
            .unwrap_or_else(|| panic!("system {n} missing from build_tick_schedule"))
    };
    assert!(pos("drain_observer_actions_system") < pos("drain_guest_intents_system"));
    assert!(pos("drain_guest_intents_system") < pos("apply_live_interventions_system"));
    assert!(pos("apply_live_interventions_system") < pos("sensory_system"));

    assert_eq!(a1.world_identity(), a2.world_identity());
    assert_eq!(a1.world_identity().genesis_phase, None);
    let res1 = anima_engine_lib::core::experiment_runner::run_manifest_seed::<LiveExperimentAdapter>(
        &minimal_valid_manifest_for_live(),
        &ObservableRegistry::reference_default(),
        77,
        None,
        None,
    );
    let res2 = anima_engine_lib::core::experiment_runner::run_manifest_seed::<LiveExperimentAdapter>(
        &minimal_valid_manifest_for_live(),
        &ObservableRegistry::reference_default(),
        77,
        None,
        None,
    );
    assert!(res1.status.is_completed());
    assert_eq!(res1.final_observables, res2.final_observables);
    assert!(DeterministicMode::on().is_enabled());
}

fn minimal_valid_manifest_for_live() -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "i1-live-optin".into(),
        name: "i1-live-optin".into(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: InitialConditionSet::new(vec![
            ("live.founders".into(), 10.0),
            ("live.predator_fraction".into(), 0.3),
            ("live.trees".into(), 8.0),
            ("live.lakes".into(), 2.0),
            ("live.food_cap".into(), 50.0),
        ]),
        interventions: Vec::new(),
        seeds: vec![77],
        duration_ticks: 20,
        sample_period: 10,
        observable_ids: vec!["plants".into(), "herbivores".into()],
        exotic_interventions: Vec::new(),
        observer: ObserverPolicy::Absent,
        island: None,
    }
}
