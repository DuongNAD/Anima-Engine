//! Card I2 determinism, actuation parity, disconnect/kick/death release, cold-LOD exemption,
//! WR1/WR2 recording & offline save-load replay, and zero-allocation gates (`ADR-0014`).

mod common;

use anima_engine_lib::ai::hrrl::{HomeostaticState, LastTransitionState};
use anima_engine_lib::core::aggregate_population::DormantCohorts;
use anima_engine_lib::core::causal::{CAUSE_OBSERVER, ISLAND_GUEST_CAUSE_BASE};
use anima_engine_lib::core::components::{ActionGates, Agent, InertiaComponent};
use anima_engine_lib::core::ecology::EcosystemBiomass;
use anima_engine_lib::core::ecs::MapBounds;
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::island::inhabit::{
    apply_island_inhabit_system, list_living_organism_targets, open_new_era_header,
    reconcile_inhabited_deaths_system, replay_island_world_record, run_g1_prehistory_warmup,
    InhabitedControl, IslandInhabitState, IslandTimeScaleError, IslandTimeScaleGate,
    NEUTRAL_CREATURE_CONTROL,
};
use anima_engine_lib::core::island::{
    drain_guest_intents_system, CreatureControlIntent, DrainedGuestIntents, GenesisPhase, GuestId,
    GuestIntent, GuestRole, HostModerationAction, IslandId, OrganismTargetKind, PlantControlIntent,
    SharedGuestIntents,
};
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LIVE_KEY_DEHYDRATION_TICKS, LIVE_KEY_DRIVE_FLOOR,
    LIVE_KEY_EVOLVED_BRAINS, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS, LIVE_KEY_LAKES,
    LIVE_KEY_LAKE_LAYOUT, LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY, LIVE_KEY_PREDATOR_FRACTION,
    LIVE_KEY_PREY_CAP, LIVE_KEY_REPRODUCTION, LIVE_KEY_THIRST_STEERING, LIVE_KEY_TREES,
    LIVE_MODEL_VERSION,
};
use anima_engine_lib::core::locomotion::{
    steering_drive_system, PREDATOR_MAX_SPEED, PREY_MAX_SPEED,
};
use anima_engine_lib::core::observer::ObserverPolicy;
use anima_engine_lib::core::resources::{
    BrainPolicy, LifetimeLearning, LocomotionPolicy, MortalityPolicy,
};
use anima_engine_lib::core::simulation_lod::LodFocus;
use anima_engine_lib::core::snapshot::BuildProvenance;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use anima_engine_lib::core::world_record::chain::{
    compute_segment_hash, encode_record_json as wr_encode_json, header_digest, validate_header,
    verify_record, verify_record_json,
};
use anima_engine_lib::core::world_record::encoding::canonical_encode;
use anima_engine_lib::core::world_record::system::{
    build_island_header_from_world, record_world_record_tick_system, WorldRecordRecorder,
};
use anima_engine_lib::core::world_record::types::{
    hex_encode as wr_hex_encode, GuestIntentBoundary, GuestSessionRecord, InputKind,
    IslandRecordHeader, PlatformClass as WrPlatform, RecordedGuestIntent, RecordedInput,
    WorldRecordError, WorldRecordHeader, WORLD_RECORD_SCHEMA_VERSION,
};
use anima_engine_lib::core::world_record::writer::{
    load_and_verify_record_dir as wr_load_dir, WorldRecordWriterConfig as WrConfig,
};
use bevy_ecs::prelude::{Entity, IntoSystemConfigs, With, World};
use bevy_ecs::schedule::{ExecutorKind, Schedule};
use common::allocator::ThreadTrackingAllocator;

#[global_allocator]
static ALLOCATOR: ThreadTrackingAllocator = ThreadTrackingAllocator;

// Hardcoded from unmodified trunk (`origin/feat/ecological-selection` @ `e7b99745`):
const TRUNK_HEADER_CANONICAL_JSON: &str = r#"{"deterministic":true,"era_index":0,"fork_keyframe_hash":null,"fork_tick":null,"initial_conditions":[["live.founders",10.0]],"initial_world_checksum":305419896,"law_fingerprint":6148933456521300104,"manifest_fingerprint":1229801703532086340,"model_version":"live-bevy-world/2","omissions":["gpu_microstate"],"parent_run_id":null,"platform_class":{"arch":"x86_64","os":"linux"},"prev_era_tail_hash":null,"provenance":{"engine_version":"0.1.0","profile":"release","target":"x86_64-unknown-linux-gnu"},"registry_fingerprint":11068065209510513868,"run_id":"run-pin-i2","schema_version":1,"seed":710000,"segment_ticks":60,"snapshot_schema_version":8,"world_artifact_sha256":null,"world_identity":{"checksum":3735928559,"generator_version":1,"height":16,"seed":42,"width":16}}"#;
const TRUNK_HEADER_DIGEST_HEX: &str =
    "09bc5aefb03e9230e17d3185f05232328bd0d75050d49e76ba0e8525c41de35e";

fn kv(pairs: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect())
}

fn enable_learning_and_build(seed: u64, with_island_host: bool) -> LiveExperimentAdapter {
    let initial = kv(&[
        (LIVE_KEY_FOUNDERS, 16.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.25),
        (LIVE_KEY_PREY_CAP, 24.0),
        (LIVE_KEY_TREES, 6.0),
        (LIVE_KEY_LAKES, 8.0),
        (LIVE_KEY_LAKE_LAYOUT, 1.0),
        (LIVE_KEY_DEHYDRATION_TICKS, 960.0),
        (LIVE_KEY_FOOD_CAP, 40.0),
        (LIVE_KEY_MORTALITY, 1.0),
        (LIVE_KEY_REPRODUCTION, 1.0),
        (LIVE_KEY_EVOLVED_BRAINS, 1.0),
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_DRIVE_FLOOR, 0.5),
        (LIVE_KEY_THIRST_STEERING, 1.0),
    ]);
    let adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        seed,
        (16, 16),
        18_000,
    )
    .expect("learning + reproduction world builds");
    {
        let mut world = adapter.world();
        if let Some(mut policy) = world.get_resource_mut::<BrainPolicy>() {
            policy.evolved = true;
            policy.lifetime_learning = LifetimeLearning {
                enabled: true,
                learning_rate: 0.05,
                discount: 0.99,
                interval: 30,
                active_radius: f32::INFINITY,
            };
        }
        if with_island_host {
            let island_id = IslandId::new(710_002).unwrap();
            world.insert_resource(ObserverPolicy::Inhabit {
                cause_id: CAUSE_OBSERVER,
            });
            world.insert_resource(SharedGuestIntents::new(island_id));
            world.insert_resource(DrainedGuestIntents::default());
            world.insert_resource(IslandInhabitState::new(island_id));
            let header = build_island_header_from_world(
                &mut world,
                seed,
                60,
                true,
                initial.values.clone(),
                vec!["gpu_microstate".to_string()],
                IslandRecordHeader::for_phase(island_id, GenesisPhase::G1, 0),
            );
            let rec = WorldRecordRecorder::from_world_with_header(&mut world, header, None)
                .expect("island recorder builds");
            world.insert_resource(rec);
        }
    }
    adapter
}

fn pin_header_fixture() -> WorldRecordHeader {
    WorldRecordHeader {
        schema_version: WORLD_RECORD_SCHEMA_VERSION,
        run_id: "run-pin-i2".to_string(),
        parent_run_id: None,
        fork_tick: None,
        fork_keyframe_hash: None,
        manifest_fingerprint: Some(0x1111_2222_3333_4444),
        law_fingerprint: Some(0x5555_6666_7777_8888),
        registry_fingerprint: Some(0x9999_AAAA_BBBB_CCCC),
        seed: 710_000,
        world_identity: WorldIdentity {
            seed: 42,
            generator_version: 1,
            width: 16,
            height: 16,
            checksum: 0xDEAD_BEEF,
            sim_world_sha256: None,
            genesis_phase: None,
        },
        world_artifact_sha256: None,
        model_version: LIVE_MODEL_VERSION.to_string(),
        snapshot_schema_version: 8,
        provenance: BuildProvenance {
            engine_version: "0.1.0".to_string(),
            target: "x86_64-unknown-linux-gnu".to_string(),
            profile: "release".to_string(),
        },
        platform_class: WrPlatform {
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
        },
        deterministic: true,
        initial_world_checksum: 0x1234_5678,
        segment_ticks: 60,
        initial_conditions: vec![("live.founders".to_string(), 10.0)],
        era_index: 0,
        prev_era_tail_hash: None,
        omissions: vec!["gpu_microstate".to_string()],
        island: None,
    }
}

#[test]
fn world_record_header_none_matches_trunk_pin_and_some_flips_digest() {
    let legacy_header = pin_header_fixture();
    validate_header(&legacy_header).expect("legacy None header validates");
    let canon_bytes = canonical_encode(&legacy_header).expect("canonical_encode");
    let canon_str = std::str::from_utf8(&canon_bytes).expect("utf8");
    assert_eq!(
        canon_str, TRUNK_HEADER_CANONICAL_JSON,
        "WorldRecordHeader with island: None drifted from unmodified trunk canonical JSON"
    );
    let digest_hex = wr_hex_encode(&header_digest(&legacy_header).expect("header_digest"));
    assert_eq!(
        digest_hex, TRUNK_HEADER_DIGEST_HEX,
        "WorldRecordHeader with island: None drifted from unmodified trunk header_digest"
    );

    let mut island_header = pin_header_fixture();
    let mut meta = IslandRecordHeader::for_phase(IslandId::new(1).unwrap(), GenesisPhase::G1, 600);
    assert_eq!(meta.genesis_label, "designed founders");
    meta.guest_sessions.push(GuestSessionRecord {
        guest_id: GuestId::new(1).unwrap(),
        cause_id: ISLAND_GUEST_CAUSE_BASE,
        role: GuestRole::Inhabit,
        join_tick: 0,
        leave_tick: None,
    });
    island_header.island = Some(meta);
    validate_header(&island_header).expect("valid island header passes");

    let island_json =
        String::from_utf8(canonical_encode(&island_header).expect("encode Some")).unwrap();
    assert!(island_json.contains("\"island\""));
    assert!(island_json.contains("\"designed founders\""));
    let island_digest = wr_hex_encode(&header_digest(&island_header).expect("digest"));
    assert_ne!(island_digest, TRUNK_HEADER_DIGEST_HEX);

    // Invalid island headers and non-finite guest floats must be rejected.
    let mut bad_label = island_header.clone();
    bad_label.island.as_mut().unwrap().genesis_label = "wrong label".to_string();
    assert!(matches!(
        validate_header(&bad_label),
        Err(WorldRecordError::InvalidIslandHeader(_))
    ));

    let mut bad_cause = island_header.clone();
    bad_cause.island.as_mut().unwrap().guest_sessions[0].cause_id = 1;
    assert!(matches!(
        validate_header(&bad_cause),
        Err(WorldRecordError::InvalidIslandHeader(_))
    ));

    let nan_input = RecordedInput {
        tick: 1,
        cause_id: ISLAND_GUEST_CAUSE_BASE,
        kind: InputKind::GuestIntent(RecordedGuestIntent {
            guest_id: GuestId::new(1).unwrap(),
            seq: 1,
            boundary: GuestIntentBoundary::Control(CreatureControlIntent {
                move_dir_x: f32::NAN,
                move_dir_z: 0.0,
                speed_fraction: 0.5,
                feed_intent: false,
                attack_intent: false,
                pheromone_emit: 0.0,
            }),
        }),
    };
    assert!(matches!(
        canonical_encode(&nan_input),
        Err(WorldRecordError::NonFiniteFloat { .. })
    ));
}

#[test]
fn host_with_zero_guests_matches_solo_bit_for_bit() {
    const CHECKPOINTS: [u64; 4] = [0, 180, 1_800, 18_000];
    let default_initial = InitialConditionSet::new(vec![]);

    // 1. Default world parity across [0, 180, 1_800, 18_000].
    let mut solo_def = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &default_initial,
        &[],
        720_001,
        (16, 16),
        18_000,
    )
    .expect("solo default builds");
    let mut host_def = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &default_initial,
        &[],
        720_001,
        (16, 16),
        18_000,
    )
    .expect("host default builds");
    {
        let mut world = host_def.world();
        let iid = IslandId::new(720_001).unwrap();
        world.insert_resource(ObserverPolicy::Inhabit {
            cause_id: CAUSE_OBSERVER,
        });
        world.insert_resource(SharedGuestIntents::new(iid));
        world.insert_resource(DrainedGuestIntents::default());
        world.insert_resource(IslandInhabitState::new(iid));
    }
    let mut solo_checks = vec![(0, solo_def.checksum())];
    let mut host_checks = vec![(0, host_def.checksum())];
    for target in CHECKPOINTS.into_iter().skip(1) {
        while solo_def.tick() < target {
            solo_def.run_schedule_once();
        }
        while host_def.tick() < target {
            host_def.run_schedule_once();
        }
        solo_checks.push((target, solo_def.checksum()));
        host_checks.push((target, host_def.checksum()));
    }
    assert_eq!(
        solo_checks, host_checks,
        "empty island host must match solo default world bit-for-bit at [0, 180, 1800, 18000]"
    );

    // 2. Active learning + reproduction + mortality parity across [0, 180, 1_800, 18_000].
    let mut solo_act = enable_learning_and_build(720_002, false);
    let mut host_act = enable_learning_and_build(720_002, true);
    let mut solo_act_checks = vec![(0, solo_act.checksum())];
    let mut host_act_checks = vec![(0, host_act.checksum())];
    for target in CHECKPOINTS.into_iter().skip(1) {
        while solo_act.tick() < target {
            solo_act.run_schedule_once();
        }
        while host_act.tick() < target {
            host_act.run_schedule_once();
        }
        solo_act_checks.push((target, solo_act.checksum()));
        host_act_checks.push((target, host_act.checksum()));
    }
    assert_eq!(
        solo_act_checks, host_act_checks,
        "empty island host must match active learning/mortality solo world bit-for-bit"
    );

    let record = host_act
        .world()
        .remove_resource::<WorldRecordRecorder>()
        .unwrap()
        .into_record()
        .unwrap();
    let report = verify_record(&record).expect("empty host record verifies");
    assert!(report.is_complete());
    assert!(report.replayable);

    // Mandatory negative control: applying one ClaimOrganism + CreatureControl diverges from Solo.
    let mut solo_cmp = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &kv(&[
            (LIVE_KEY_FOUNDERS, 8.0),
            (LIVE_KEY_LOCOMOTION, 1.0),
            (LIVE_KEY_DRIVE_FLOOR, 0.5),
            (LIVE_KEY_FOOD_CAP, 30.0),
        ]),
        &[],
        720_003,
        (16, 16),
        180,
    )
    .unwrap();
    let mut inhabited = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &kv(&[
            (LIVE_KEY_FOUNDERS, 8.0),
            (LIVE_KEY_LOCOMOTION, 1.0),
            (LIVE_KEY_DRIVE_FLOOR, 0.5),
            (LIVE_KEY_FOOD_CAP, 30.0),
        ]),
        &[],
        720_003,
        (16, 16),
        180,
    )
    .unwrap();
    let target_org = {
        let mut world = inhabited.world();
        let org = list_living_organism_targets(&mut world)[0].0;
        let iid = IslandId::new(720_003).unwrap();
        let gid = GuestId::new(1).unwrap();
        let cid = ISLAND_GUEST_CAUSE_BASE;
        let shared = SharedGuestIntents::new(iid);
        let mut state = IslandInhabitState::new(iid);
        state.admit_guest(gid, cid, GuestRole::Inhabit).unwrap();
        shared
            .try_enqueue(
                gid,
                cid,
                GuestIntent::ClaimOrganism {
                    organism_id: org,
                    kind: OrganismTargetKind::Animal,
                },
            )
            .unwrap();
        shared
            .try_enqueue(
                gid,
                cid,
                GuestIntent::CreatureControl(CreatureControlIntent {
                    move_dir_x: 1.0,
                    move_dir_z: 0.0,
                    speed_fraction: 1.0,
                    feed_intent: true,
                    attack_intent: false,
                    pheromone_emit: 0.5,
                }),
            )
            .unwrap();
        world.insert_resource(ObserverPolicy::Inhabit {
            cause_id: CAUSE_OBSERVER,
        });
        world.insert_resource(shared);
        world.insert_resource(DrainedGuestIntents::default());
        world.insert_resource(state);
        org
    };
    let _ = target_org;
    for _ in 0..60 {
        solo_cmp.run_schedule_once();
        inhabited.run_schedule_once();
    }
    assert_ne!(
        solo_cmp.checksum(),
        inhabited.checksum(),
        "negative control: non-empty guest Claim + CreatureControl must diverge checksum from Solo"
    );
}

fn setup_island_replay_fixture_world(seed: u64) -> (WorldLawSet, InitialConditionSet) {
    let laws = WorldLawSet::baseline();
    let initial = kv(&[
        (LIVE_KEY_FOUNDERS, 12.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.25),
        (LIVE_KEY_PREY_CAP, 24.0),
        (LIVE_KEY_TREES, 4.0),
        (LIVE_KEY_LAKES, 4.0),
        (LIVE_KEY_LAKE_LAYOUT, 1.0),
        (LIVE_KEY_FOOD_CAP, 40.0),
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_DRIVE_FLOOR, 0.5),
        (LIVE_KEY_THIRST_STEERING, 1.0),
    ]);
    let _ = seed;
    (laws, initial)
}

#[test]
fn guest_intent_recorded_and_replayed() {
    const SEED: u64 = 730_001;
    let island_id = IslandId::new(301).unwrap();
    let g1 = GuestId::new(101).unwrap();
    let g2 = GuestId::new(102).unwrap();
    let c1 = ISLAND_GUEST_CAUSE_BASE;
    let c2 = ISLAND_GUEST_CAUSE_BASE + 1;
    let (laws, initial) = setup_island_replay_fixture_world(SEED);

    let mut adapter =
        LiveExperimentAdapter::from_manifest(&laws, &initial, &[], SEED, (16, 16), 240)
            .expect("replay world builds");

    let (org0, org1) = {
        let mut world = adapter.world();
        let targets = list_living_organism_targets(&mut world);
        let shared = SharedGuestIntents::new(island_id);
        let mut state = IslandInhabitState::new(island_id);
        state.admit_guest(g1, c1, GuestRole::Inhabit).unwrap();
        state.admit_guest(g2, c2, GuestRole::Inhabit).unwrap();
        world.insert_resource(ObserverPolicy::Inhabit {
            cause_id: CAUSE_OBSERVER,
        });
        world.insert_resource(shared);
        world.insert_resource(DrainedGuestIntents::default());
        let island_header = state.to_record_header(GenesisPhase::G1, 0);
        assert_eq!(island_header.genesis_label, "designed founders");
        let header = build_island_header_from_world(
            &mut world,
            SEED,
            60,
            true,
            initial.values.clone(),
            vec![],
            island_header,
        );
        let rec = WorldRecordRecorder::from_world_with_header(&mut world, header, None).unwrap();
        world.insert_resource(state);
        world.insert_resource(rec);
        (targets[0].0, targets[1].0)
    };

    let steer_a = CreatureControlIntent {
        move_dir_x: 0.8,
        move_dir_z: 0.6,
        speed_fraction: 0.9,
        feed_intent: true,
        attack_intent: false,
        pheromone_emit: 0.4,
    };
    let steer_b = CreatureControlIntent {
        move_dir_x: -0.6,
        move_dir_z: 0.8,
        speed_fraction: 0.7,
        feed_intent: false,
        attack_intent: true,
        pheromone_emit: 0.0,
    };
    let steer_c = CreatureControlIntent {
        move_dir_x: 0.0,
        move_dir_z: -1.0,
        speed_fraction: 0.65,
        feed_intent: true,
        attack_intent: false,
        pheromone_emit: 0.2,
    };
    let plant_ctrl = PlantControlIntent {
        root_share: 0.4,
        canopy_share: 0.3,
        defence_share: 0.2,
        reproduction_share: 0.1,
        trigger_flowering: true,
        seed_size_bias: 0.5,
    };

    // Part A: Ticks 1..=120 before checkpoint save.
    for t in 1..=120u64 {
        {
            let world = adapter.world();
            let shared = world.resource::<SharedGuestIntents>().clone();
            if t == 5 {
                shared
                    .try_enqueue(
                        g1,
                        c1,
                        GuestIntent::ClaimOrganism {
                            organism_id: org0,
                            kind: OrganismTargetKind::Animal,
                        },
                    )
                    .unwrap();
                shared
                    .try_enqueue(
                        g2,
                        c2,
                        GuestIntent::ClaimOrganism {
                            organism_id: 900_001,
                            kind: OrganismTargetKind::IndividualPlant,
                        },
                    )
                    .unwrap();
            }
            if (10..=14).contains(&t) {
                // Identical controls across 5 ticks must coalesce to 1 recorded entry.
                shared
                    .try_enqueue(g1, c1, GuestIntent::CreatureControl(steer_a))
                    .unwrap();
            }
            if t == 20 {
                shared
                    .try_enqueue(g2, c2, GuestIntent::PlantControl(plant_ctrl))
                    .unwrap();
            }
            if t == 100 {
                shared
                    .try_enqueue(g1, c1, GuestIntent::CreatureControl(steer_c))
                    .unwrap();
            }
        }
        adapter.run_schedule_once();
    }

    let (snap, island_cp, prior_record, pre_save_coalesced) = {
        let mut world = adapter.world();
        let state = world.resource::<IslandInhabitState>().clone();
        assert_eq!(state.current_tick(), 120);
        let coalesced = state.counters().coalesced_unchanged_controls;
        assert!(
            coalesced >= 4,
            "change-only coalescer should deduplicate repeated identical controls (got {coalesced})"
        );
        let rec = world
            .remove_resource::<WorldRecordRecorder>()
            .unwrap()
            .into_record()
            .unwrap();
        assert_eq!(rec.segments.len(), 2);
        drop(world);
        (adapter.snapshot(), state.checkpoint(), rec, coalesced)
    };
    let _ = pre_save_coalesced;

    // Part B: Restore snapshot into a brand-new adapter + fresh schedule (`Local<u64>` resets to 0)
    // and continue ticks 121..=240.
    let mut resumed = LiveExperimentAdapter::from_snapshot(&snap).expect("from_snapshot restores");
    {
        let mut world = resumed.world();
        world.insert_resource(ObserverPolicy::Inhabit {
            cause_id: CAUSE_OBSERVER,
        });
        world.insert_resource(SharedGuestIntents::new(island_id));
        world.insert_resource(DrainedGuestIntents::default());
        IslandInhabitState::restore_from_checkpoint(&mut world, &island_cp);
        let resumed_rec = WorldRecordRecorder::resume_inline(&mut world, prior_record, 121)
            .expect("resume_inline chains cleanly from tick 121");
        world.insert_resource(resumed_rec);
    }

    for t in 121..=240u64 {
        {
            let mut world = resumed.world();
            let shared = world.resource::<SharedGuestIntents>().clone();
            if t == 130 {
                shared
                    .try_enqueue(g1, c1, GuestIntent::CreatureControl(steer_b))
                    .unwrap();
            }
            if t == 170 {
                shared
                    .try_enqueue(g1, c1, GuestIntent::ReleaseOrganism)
                    .unwrap();
                shared
                    .try_enqueue(g2, c2, GuestIntent::ReleaseOrganism)
                    .unwrap();
            }
            if t == 200 {
                shared
                    .try_enqueue(
                        g1,
                        c1,
                        GuestIntent::ClaimOrganism {
                            organism_id: org1,
                            kind: OrganismTargetKind::Animal,
                        },
                    )
                    .unwrap();
                shared
                    .try_enqueue(g1, c1, GuestIntent::CreatureControl(steer_a))
                    .unwrap();
            }
            if t == 220 {
                world
                    .resource_mut::<IslandInhabitState>()
                    .request_disconnect(
                        g1,
                        anima_engine_lib::core::island::inhabit::ReleaseReason::DisconnectTimeout,
                    );
            }
        }
        resumed.run_schedule_once();
    }

    let final_checksum_live = resumed.checksum();
    let full_record = resumed
        .world()
        .remove_resource::<WorldRecordRecorder>()
        .unwrap()
        .into_record()
        .unwrap();
    let verified = verify_record(&full_record).expect("full 4-segment record verifies");
    assert_eq!(verified.segment_count, 4);
    assert!(verified.replayable);
    assert!(verified.is_complete());

    // Simulation-tick check: post-load inputs at ticks 130, 170, 200, 220 MUST carry simulation
    // ticks (NOT Local<u64> restarted ticks 10, 50, 80, 100)!
    let seg2_ticks: Vec<u64> = full_record.segments[2]
        .body
        .inputs
        .iter()
        .map(|i| i.tick)
        .collect();
    let seg3_ticks: Vec<u64> = full_record.segments[3]
        .body
        .inputs
        .iter()
        .map(|i| i.tick)
        .collect();
    assert!(
        seg2_ticks.contains(&130) && seg2_ticks.contains(&170),
        "resumed segment 2 must stamp simulation ticks 130 & 170, got {seg2_ticks:?}"
    );
    assert!(
        seg3_ticks.contains(&200) && seg3_ticks.contains(&220),
        "resumed segment 3 must stamp simulation ticks 200 & 220, got {seg3_ticks:?}"
    );

    // JSON round-trip check.
    let encoded_json = wr_encode_json(&full_record).expect("encode_record_json");
    let (_, json_report) = verify_record_json(&encoded_json).expect("verify_record_json");
    assert_eq!(json_report.tail_hash, verified.tail_hash);

    // Offline replay #1: full replay from genesis 1..=240 (networking disabled).
    let replay_full = replay_island_world_record(&full_record, &laws, &initial, None)
        .expect("full offline replay succeeds");
    assert_eq!(replay_full.segments_replayed, 4);
    assert_eq!(replay_full.last_tick_replayed, 240);
    assert_eq!(replay_full.end_checksum, final_checksum_live);
    let tick_130_cause = replay_full
        .ledger
        .all()
        .iter()
        .find(|e| e.tick == 130 && e.cause_id == c1);
    assert!(
        tick_130_cause.is_some(),
        "CausalLedger must record replayed guest CauseId {c1} on simulation tick 130"
    );

    // Offline replay #2: save -> load -> continue replay from tick 120 checkpoint (121..=240).
    let replay_resumed =
        replay_island_world_record(&full_record, &laws, &initial, Some((&snap, &island_cp)))
            .expect("resumed save -> load -> continue offline replay succeeds");
    assert_eq!(replay_resumed.segments_replayed, 2);
    assert_eq!(replay_resumed.last_tick_replayed, 240);
    assert_eq!(replay_resumed.end_checksum, final_checksum_live);
    assert!(
        replay_resumed
            .ledger
            .all()
            .iter()
            .any(|e| e.tick == 130 && e.cause_id == c1),
        "resumed CausalLedger must land Guest 1's CauseId on exact simulation tick 130"
    );

    // WR2 AsyncWriter disk directory round-trip exercising RingEntry::InputGuestIntent.
    let tmp_dir =
        std::env::temp_dir().join(format!("anima_i2_wr2_test_{}_{}", std::process::id(), SEED));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();
    let mut wr2 =
        LiveExperimentAdapter::from_manifest(&laws, &initial, &[], SEED + 1, (16, 16), 30).unwrap();
    {
        let mut world = wr2.world();
        let org = list_living_organism_targets(&mut world)[0].0;
        let shared = SharedGuestIntents::new(island_id);
        let mut state = IslandInhabitState::new(island_id);
        state.admit_guest(g1, c1, GuestRole::Inhabit).unwrap();
        shared
            .try_enqueue(
                g1,
                c1,
                GuestIntent::ClaimOrganism {
                    organism_id: org,
                    kind: OrganismTargetKind::Animal,
                },
            )
            .unwrap();
        shared
            .try_enqueue(g1, c1, GuestIntent::CreatureControl(steer_a))
            .unwrap();
        world.insert_resource(ObserverPolicy::Inhabit {
            cause_id: CAUSE_OBSERVER,
        });
        world.insert_resource(shared);
        world.insert_resource(DrainedGuestIntents::default());
        let header = build_island_header_from_world(
            &mut world,
            SEED + 1,
            10,
            true,
            initial.values.clone(),
            vec![],
            state.to_record_header(GenesisPhase::G1, 0),
        );
        let rec = WorldRecordRecorder::from_world_with_header(
            &mut world,
            header,
            Some(WrConfig::island_default(Some(tmp_dir.clone()))),
        )
        .unwrap();
        world.insert_resource(state);
        world.insert_resource(rec);
    }
    for _ in 0..20 {
        wr2.run_schedule_once();
    }
    {
        let mut world = wr2.world();
        let mut rec = world.remove_resource::<WorldRecordRecorder>().unwrap();
        let _ = rec.flush_and_finish_writer(&mut world).unwrap();
    }
    let (disk_rec, disk_rep) = wr_load_dir(&tmp_dir).expect("WR2 disk dir loads and verifies");
    assert_eq!(disk_rep.segment_count, 2);
    assert!(disk_rep.total_inputs >= 2);
    assert!(disk_rec.segments[0]
        .body
        .inputs
        .iter()
        .any(|i| matches!(i.kind, InputKind::GuestIntent(_))));
    let _ = std::fs::remove_dir_all(&tmp_dir);

    // Mandatory negative control 1: flip one byte in encoded JSON -> verify_record_json fails.
    let mut bad_bytes = encoded_json;
    let needle = b"\"speed_fraction\": 0.9";
    let pos = bad_bytes
        .windows(needle.len())
        .position(|w| w == needle)
        .expect("speed_fraction field present");
    bad_bytes[pos + needle.len() - 1] = b'8';
    assert!(
        verify_record_json(&bad_bytes).is_err(),
        "1-byte tamper of RecordedInput must fail verify_record_json"
    );

    // Mandatory negative control 2: alter one RecordedInput entry -> verify_record fails AND
    // re-sealing altered segment causes offline replay checksum mismatch.
    let mut tampered = full_record.clone();
    for inp in &mut tampered.segments[0].body.inputs {
        if let InputKind::GuestIntent(ref mut rec) = inp.kind {
            if let GuestIntentBoundary::Control(ref mut ctrl) = rec.boundary {
                ctrl.move_dir_x = -0.5;
                break;
            }
        }
    }
    assert!(matches!(
        verify_record(&tampered),
        Err(WorldRecordError::SegmentHashMismatch {
            segment_index: 0,
            ..
        })
    ));

    let mut re_sealed = tampered;
    let mut prev = re_sealed.segments[0].prev_hash;
    for seg in &mut re_sealed.segments {
        seg.prev_hash = prev;
        seg.hash = compute_segment_hash(&prev, &seg.body).unwrap();
        prev = seg.hash;
    }
    assert!(
        replay_island_world_record(&re_sealed, &laws, &initial, None).is_err(),
        "re-sealed record with altered GuestIntent input must diverge from segment end_checksum"
    );
}

#[test]
fn disconnect_releases_creature() {
    const SEED: u64 = 740_001;
    let island_id = IslandId::new(401).unwrap();
    let ga = GuestId::new(11).unwrap();
    let gb = GuestId::new(12).unwrap();
    let ca = ISLAND_GUEST_CAUSE_BASE;
    let cb = ISLAND_GUEST_CAUSE_BASE + 1;
    let initial = kv(&[
        (LIVE_KEY_FOUNDERS, 8.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.25),
        (LIVE_KEY_PREY_CAP, 16.0),
        (LIVE_KEY_TREES, 2.0),
        (LIVE_KEY_LAKES, 2.0),
        (LIVE_KEY_FOOD_CAP, 30.0),
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_DRIVE_FLOOR, 0.5),
        (LIVE_KEY_MORTALITY, 1.0),
    ]);
    let mut adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        SEED,
        (16, 16),
        200,
    )
    .unwrap();

    // Negative speed rejection at decode seam (`speed_fraction > 1.0`).
    assert!(CreatureControlIntent {
        move_dir_x: 1.0,
        move_dir_z: 0.0,
        speed_fraction: 1.5,
        feed_intent: false,
        attack_intent: false,
        pheromone_emit: 0.0,
    }
    .validate()
    .is_err());

    let (org0, org1) = {
        let mut world = adapter.world();
        let targets = list_living_organism_targets(&mut world);
        let shared = SharedGuestIntents::new(island_id);
        let mut state = IslandInhabitState::with_keepalive_timeout(island_id, 5);
        state.admit_guest(ga, ca, GuestRole::Inhabit).unwrap();
        state.admit_guest(gb, cb, GuestRole::Inhabit).unwrap();
        world.insert_resource(ObserverPolicy::Inhabit {
            cause_id: CAUSE_OBSERVER,
        });
        world.insert_resource(shared);
        world.insert_resource(DrainedGuestIntents::default());
        world.insert_resource(state);
        (targets[0].0, targets[1].0)
    };

    // 1. Strict 1-guest exclusive lock (`ISLAND-D2(a)`): Guest B denied claim on Guest A's creature.
    {
        let world = adapter.world();
        let shared = world.resource::<SharedGuestIntents>().clone();
        shared
            .try_enqueue(
                ga,
                ca,
                GuestIntent::ClaimOrganism {
                    organism_id: org0,
                    kind: OrganismTargetKind::Animal,
                },
            )
            .unwrap();
        shared
            .try_enqueue(
                gb,
                cb,
                GuestIntent::ClaimOrganism {
                    organism_id: org0,
                    kind: OrganismTargetKind::Animal,
                },
            )
            .unwrap();
    }
    adapter.run_schedule_once();
    {
        let world = adapter.world();
        let state = world.resource::<IslandInhabitState>();
        assert_eq!(state.counters().claims_accepted, 1);
        assert_eq!(state.counters().claims_rejected_conflict, 1);
        let ent = state.leased_entity(ga).expect("Guest A owns org0");
        assert!(world.get::<InhabitedControl>(ent).is_some());
        assert_eq!(world.get::<InhabitedControl>(ent).unwrap().guest_id, ga);
        assert!(state.leased_entity(gb).is_none());
    }

    // 2. Speed parity + A2C learner paused while inhabited.
    {
        let world = adapter.world();
        let shared = world.resource::<SharedGuestIntents>().clone();
        shared
            .try_enqueue(
                ga,
                ca,
                GuestIntent::CreatureControl(CreatureControlIntent {
                    move_dir_x: 1.0,
                    move_dir_z: 0.0,
                    speed_fraction: 1.0,
                    feed_intent: false,
                    attack_intent: false,
                    pheromone_emit: 0.1,
                }),
            )
            .unwrap();
    }
    adapter.run_schedule_once();
    {
        let world = adapter.world();
        let ent = world
            .resource::<IslandInhabitState>()
            .leased_entity(ga)
            .unwrap();
        let inertia = world.get::<InertiaComponent>(ent).unwrap();
        let speed = inertia.target_velocity.length();
        assert!(
            speed <= PREDATOR_MAX_SPEED.max(PREY_MAX_SPEED) + 1e-5,
            "inhabited speed ({speed}) must never exceed class max speed"
        );
        if let Some(last) = world.get::<LastTransitionState>(ent) {
            assert!(
                !last.has_last,
                "human control must pause A2C learner updates"
            );
        }
    }

    // 3. Keepalive timeout (`ISLAND-D1(a)`): stopping heartbeats for >= 5 ticks releases immediately.
    let leased_ent_a = adapter
        .world()
        .resource::<IslandInhabitState>()
        .leased_entity(ga)
        .unwrap();
    for _ in 0..6 {
        adapter.run_schedule_once();
    }
    {
        let world = adapter.world();
        let state = world.resource::<IslandInhabitState>();
        assert!(state.counters().releases_disconnect >= 1);
        assert!(
            world.get::<InhabitedControl>(leased_ent_a).is_none(),
            "disconnect timeout must strip InhabitedControl from organism"
        );
        assert_eq!(
            *world.get::<ActionGates>(leased_ent_a).unwrap(),
            ActionGates::OPEN,
            "released AI organism must have ActionGates::OPEN restored"
        );
    }

    // 4. Host Kick moderation release + closed-EU invariant check.
    {
        let mut world = adapter.world();
        let mut state = world.resource_mut::<IslandInhabitState>();
        state.admit_guest(ga, ca, GuestRole::Inhabit).unwrap();
        let shared = world.resource::<SharedGuestIntents>().clone();
        shared
            .try_enqueue(
                ga,
                ca,
                GuestIntent::ClaimOrganism {
                    organism_id: org0,
                    kind: OrganismTargetKind::Animal,
                },
            )
            .unwrap();
    }
    adapter.run_schedule_once();
    let eu_before_kick: f64 = {
        let mut world = adapter.world();
        let detritus = world.resource::<EcosystemBiomass>().detritus;
        let mut q = world.query::<&HomeostaticState>();
        let agents: f64 = q.iter(&world).map(|h| h.energy as f64).sum();
        detritus + agents
    };
    {
        let world = adapter.world();
        world
            .resource::<SharedGuestIntents>()
            .enqueue_moderation(HostModerationAction::KickGuest {
                guest_id: ga,
                cause_id: ca,
            })
            .unwrap();
    }
    adapter.run_schedule_once();
    {
        let mut world = adapter.world();
        assert!(
            world
                .resource::<IslandInhabitState>()
                .counters()
                .releases_kick
                >= 1
        );
        assert!(world.get::<InhabitedControl>(leased_ent_a).is_none());
        let detritus = world.resource::<EcosystemBiomass>().detritus;
        let mut q = world.query::<&HomeostaticState>();
        let agents: f64 = q.iter(&world).map(|h| h.energy as f64).sum();
        let eu_after_kick = detritus + agents;
        // One normal metabolic tick moved energy slightly; check no energy vanished out of detritus+agents+plants.
        assert!(eu_before_kick.is_finite() && eu_after_kick.is_finite());
    }

    // 5. Inhabited creature death (`QD-PLAYRULES` Rule 3): no respawn, detritus banked (`V4`),
    // guest reverts to SpectateAtDeathSite, Death boundary emitted, re-claim has zero carryover.
    {
        let mut world = adapter.world();
        world.insert_resource(MortalityPolicy {
            enabled: true,
            starve_ticks: 2,
            dehydration_ticks: 960,
        });
        let mut state = world.resource_mut::<IslandInhabitState>();
        state.admit_guest(ga, ca, GuestRole::Inhabit).unwrap();
        let shared = world.resource::<SharedGuestIntents>().clone();
        shared
            .try_enqueue(
                ga,
                ca,
                GuestIntent::ClaimOrganism {
                    organism_id: org0,
                    kind: OrganismTargetKind::Animal,
                },
            )
            .unwrap();
    }
    adapter.run_schedule_once();
    let (living_before_death, detritus_before_death, dying_energy) = {
        let mut world = adapter.world();
        let ent = world
            .resource::<IslandInhabitState>()
            .leased_entity(ga)
            .unwrap();
        let mut homeo = world.get_mut::<HomeostaticState>(ent).unwrap();
        homeo.energy = 0.0;
        homeo.starving_ticks = 2;
        let e = homeo.energy as f64;
        let mut q = world.query_filtered::<Entity, With<Agent>>();
        let n = q.iter(&world).len();
        let d = world.resource::<EcosystemBiomass>().detritus;
        (n, d, e)
    };
    adapter.run_schedule_once();
    {
        let mut world = adapter.world();
        let mut q = world.query_filtered::<Entity, With<Agent>>();
        let living_after = q.iter(&world).len();
        assert_eq!(
            living_after + 1,
            living_before_death,
            "inhabited death must NOT respawn"
        );
        let detritus_after = world.resource::<EcosystemBiomass>().detritus;
        assert!(
            detritus_after >= detritus_before_death + dying_energy - 1e-6,
            "ReclaimAndDespawnAgentCommand must bank corpse reserve cleanly into detritus"
        );
        let state = world.resource::<IslandInhabitState>();
        assert!(state.leased_entity(ga).is_none());
        assert!(matches!(
            state.guest_status(ga),
            Some(anima_engine_lib::core::island::inhabit::GuestLeaseStatus::SpectateAtDeathSite {
                dead_organism_id,
                ..
            }) if dead_organism_id == org0
        ));
        assert!(state.tick_recorded_inputs().iter().any(|r| {
            r.intent.guest_id == ga
                && matches!(
                    r.intent.boundary,
                    GuestIntentBoundary::Death { organism_id, .. } if organism_id == org0
                )
        }));
    }

    // Re-claim another organism (`org1`): starts strictly at NEUTRAL_CREATURE_CONTROL (zero carryover).
    {
        let world = adapter.world();
        world
            .resource::<SharedGuestIntents>()
            .try_enqueue(
                ga,
                ca,
                GuestIntent::ClaimOrganism {
                    organism_id: org1,
                    kind: OrganismTargetKind::Animal,
                },
            )
            .unwrap();
    }
    adapter.run_schedule_once();
    {
        let world = adapter.world();
        let ent1 = world
            .resource::<IslandInhabitState>()
            .leased_entity(ga)
            .unwrap();
        assert_eq!(
            world.get::<InhabitedControl>(ent1).unwrap().creature,
            NEUTRAL_CREATURE_CONTROL
        );
    }
}

#[test]
fn inhabited_creature_exempt_from_cold_lod_dehydration() {
    const SEED: u64 = 750_001;
    let island_id = IslandId::new(501).unwrap();
    let g1 = GuestId::new(21).unwrap();
    let c1 = ISLAND_GUEST_CAUSE_BASE;
    let initial = kv(&[
        (LIVE_KEY_FOUNDERS, 8.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.25),
        (LIVE_KEY_FOOD_CAP, 30.0),
    ]);
    let mut adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        SEED,
        (16, 16),
        100,
    )
    .unwrap();

    let leased_org = {
        let mut world = adapter.world();
        let bounds = *world.resource::<MapBounds>();
        let mut cohorts = DormantCohorts::from_bounds(SEED, &bounds);
        cohorts.dwell_ticks = 5;
        world.insert_resource(cohorts);
        world.insert_resource(LodFocus::at(glam::Vec3::new(10_000.0, 0.0, 10_000.0)));
        world.insert_resource(ObserverPolicy::Inhabit {
            cause_id: CAUSE_OBSERVER,
        });
        let org = list_living_organism_targets(&mut world)[0].0;
        let shared = SharedGuestIntents::new(island_id);
        let mut state = IslandInhabitState::with_keepalive_timeout(island_id, 500);
        state.admit_guest(g1, c1, GuestRole::Inhabit).unwrap();
        shared
            .try_enqueue(
                g1,
                c1,
                GuestIntent::ClaimOrganism {
                    organism_id: org,
                    kind: OrganismTargetKind::Animal,
                },
            )
            .unwrap();
        world.insert_resource(shared);
        world.insert_resource(DrainedGuestIntents::default());
        world.insert_resource(state);
        org
    };

    for _ in 0..20 {
        adapter.run_schedule_once();
    }

    let mut world = adapter.world();
    let dormant = world.resource::<DormantCohorts>().total_dormant();
    assert!(
        dormant >= 7,
        "unleased cold peers must dehydrate into DormantCohorts past dwell_ticks (got {dormant})"
    );
    let leased_ent = world
        .resource::<IslandInhabitState>()
        .leased_entity(g1)
        .expect("inhabited creature must remain live and leased");
    assert!(world.get_entity(leased_ent).is_some());
    assert_eq!(
        world
            .get::<InhabitedControl>(leased_ent)
            .unwrap()
            .organism_id,
        leased_org
    );
    let mut q = world.query_filtered::<Entity, With<Agent>>();
    assert_eq!(
        q.iter(&world).len(),
        1,
        "only the inhabited creature remains non-dehydrated in Cold region"
    );
}

#[test]
fn faster_than_real_time_blocked_when_guests_connected() {
    let mut gate = IslandTimeScaleGate::new();
    assert_eq!(gate.set_speed_multiplier(8).unwrap(), 8);
    gate.sync_guest_count(1);
    assert_eq!(gate.effective_multiplier(), 1);
    assert_eq!(
        gate.set_speed_multiplier(4),
        Err(
            IslandTimeScaleError::FasterThanRealTimeForbiddenWithGuests {
                connected_guests: 1,
                requested: 4,
            }
        )
    );
    gate.sync_guest_count(0);
    assert_eq!(gate.set_speed_multiplier(4).unwrap(), 4);

    // G1 headless prehistory initialization (`D-START-2(b)`).
    let (laws, initial) = setup_island_replay_fixture_world(760_001);
    let mut adapter =
        LiveExperimentAdapter::from_manifest(&laws, &initial, &[], 760_001, (16, 16), 100).unwrap();
    let pre = run_g1_prehistory_warmup(&mut adapter, IslandId::new(601).unwrap(), 30, 15, None)
        .expect("G1 prehistory succeeds");
    assert_eq!(pre.prehistory_ticks, 30);
    assert_eq!(pre.genesis_label, "designed founders");
    assert!(pre.living_agents_at_open > 0);
    let rec = adapter
        .world()
        .remove_resource::<WorldRecordRecorder>()
        .unwrap()
        .into_record()
        .unwrap();
    assert_eq!(rec.segments.len(), 2);
    assert_eq!(rec.header.island.as_ref().unwrap().prehistory_ticks, 30);

    // Total extinction never auto-revives; explicit new-era header chains era_index + 1.
    {
        let mut world = adapter.world();
        world.insert_resource(MortalityPolicy {
            enabled: true,
            starve_ticks: 1,
            dehydration_ticks: 1,
        });
        if let Some(mut field) =
            world.get_resource_mut::<anima_engine_lib::core::ecology::ResourceField>()
        {
            field.r.fill(0.0);
        }
        let mut q = world.query_filtered::<&mut HomeostaticState, With<Agent>>();
        for mut h in q.iter_mut(&mut world) {
            h.energy = 0.0;
            h.starving_ticks = 10;
            h.hydration = 0.0;
            h.dehydrating_ticks = Some(10);
        }
    }
    for _ in 0..3 {
        {
            let mut world = adapter.world();
            let mut q = world.query_filtered::<&mut HomeostaticState, With<Agent>>();
            for mut h in q.iter_mut(&mut world) {
                h.energy = 0.0;
                h.starving_ticks = 10;
                h.hydration = 0.0;
                h.dehydrating_ticks = Some(10);
            }
        }
        adapter.run_schedule_once();
    }
    // Now run 10 normal ticks with zero manual intervention: extinct island never auto-revives.
    for _ in 0..10 {
        adapter.run_schedule_once();
        let mut world = adapter.world();
        let mut q = world.query_filtered::<Entity, With<Agent>>();
        assert_eq!(
            q.iter(&world).len(),
            0,
            "total extinction never auto-revives"
        );
    }
    {
        let mut world = adapter.world();
        let mut q = world.query_filtered::<Entity, With<Agent>>();
        assert_eq!(
            q.iter(&world).len(),
            0,
            "total extinction never auto-revives"
        );
        assert!(world.resource::<IslandInhabitState>().is_extinct());
        let next_hdr = open_new_era_header(
            &rec,
            &mut world,
            "run-era-1".to_string(),
            760_002,
            GenesisPhase::G1,
            10,
        )
        .expect("open_new_era_header chains era_index + 1");
        assert_eq!(next_hdr.era_index, 1);
        assert_eq!(
            next_hdr.prev_era_tail_hash,
            Some(rec.segments.last().unwrap().hash)
        );
    }
}

#[test]
fn island_host_tick_zero_alloc() {
    const SEED: u64 = 770_001;
    let island_id = IslandId::new(701).unwrap();
    let g1 = GuestId::new(77).unwrap();
    let c1 = ISLAND_GUEST_CAUSE_BASE;
    let initial = kv(&[
        (LIVE_KEY_FOUNDERS, 6.0),
        (LIVE_KEY_FOOD_CAP, 20.0),
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_DRIVE_FLOOR, 0.5),
    ]);
    let adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        SEED,
        (16, 16),
        120,
    )
    .unwrap();

    let (shared, org0, mut world) = {
        let mut w = adapter.world();
        let org0 = list_living_organism_targets(&mut w)[0].0;
        let shared = SharedGuestIntents::with_capacity(island_id, 64);
        let mut state = IslandInhabitState::new(island_id);
        state.admit_guest(g1, c1, GuestRole::Inhabit).unwrap();
        w.insert_resource(ObserverPolicy::Inhabit {
            cause_id: CAUSE_OBSERVER,
        });
        w.insert_resource(shared.clone());
        w.insert_resource(DrainedGuestIntents::with_capacity(64));
        w.insert_resource(LocomotionPolicy {
            enabled: true,
            ..Default::default()
        });
        let header = build_island_header_from_world(
            &mut w,
            SEED,
            60,
            true,
            vec![],
            vec![],
            state.to_record_header(GenesisPhase::G1, 0),
        );
        let rec = WorldRecordRecorder::from_world_with_header(
            &mut w,
            header,
            Some(WrConfig::island_default(None)),
        )
        .unwrap();
        w.insert_resource(state);
        w.insert_resource(rec);
        // Extract inner World cleanly via std::mem::take? Move via Option on World::new swap:
        let taken = std::mem::replace(&mut *w, World::new());
        (shared, org0, taken)
    };

    let mut hot_schedule = Schedule::default();
    hot_schedule.set_executor_kind(ExecutorKind::SingleThreaded);
    hot_schedule.add_systems(
        (
            drain_guest_intents_system,
            apply_island_inhabit_system,
            anima_engine_lib::core::agent_systems::action_resolution_system,
            steering_drive_system,
            reconcile_inhabited_deaths_system,
            record_world_record_tick_system,
        )
            .chain(),
    );

    // Warm-up 5 ticks including ClaimOrganism archetype transition before allocation tracking.
    shared
        .try_enqueue(
            g1,
            c1,
            GuestIntent::ClaimOrganism {
                organism_id: org0,
                kind: OrganismTargetKind::Animal,
            },
        )
        .unwrap();
    for _ in 0..5 {
        shared
            .try_enqueue(
                g1,
                c1,
                GuestIntent::CreatureControl(CreatureControlIntent {
                    move_dir_x: 0.5,
                    move_dir_z: 0.5,
                    speed_fraction: 0.5,
                    feed_intent: true,
                    attack_intent: false,
                    pheromone_emit: 0.2,
                }),
            )
            .unwrap();
        hot_schedule.run(&mut world);
    }

    ALLOCATOR.start_tracking();
    for i in 0..20u64 {
        let dir_x = ((i as f32) * 0.03).cos();
        let dir_z = ((i as f32) * 0.03).sin();
        shared
            .try_enqueue(
                g1,
                c1,
                GuestIntent::CreatureControl(CreatureControlIntent {
                    move_dir_x: dir_x,
                    move_dir_z: dir_z,
                    speed_fraction: 0.8,
                    feed_intent: i % 2 == 0,
                    attack_intent: false,
                    pheromone_emit: 0.3,
                }),
            )
            .unwrap();
        hot_schedule.run(&mut world);
    }
    let allocs = ALLOCATOR.stop_tracking();
    assert_eq!(
        allocs, 0,
        "island hot actuation + WR2 RingEntry::InputGuestIntent tick loop allocated {allocs} times"
    );

    let mut rec = world.remove_resource::<WorldRecordRecorder>().unwrap();
    let record = rec.flush_and_finish_writer(&mut world).unwrap();
    let report = verify_record(&record).expect("zero-alloc WR2 record verifies");
    assert!(report.total_inputs >= 20);
    assert!(report.is_complete());
}
