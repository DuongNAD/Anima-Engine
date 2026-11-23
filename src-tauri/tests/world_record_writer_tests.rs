//! WR2 writer thread, tiny-buffer overflow, atomic disk persistence, and async no-feedback tests (ADR-0008 H2).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anima_engine_lib::core::causal::CAUSE_OBSERVER;
use anima_engine_lib::core::components::AgentBrain;
use anima_engine_lib::core::ecs::Agent;
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LIVE_KEY_DEHYDRATION_TICKS, LIVE_KEY_DRIVE_FLOOR,
    LIVE_KEY_EVOLVED_BRAINS, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS, LIVE_KEY_LAKES,
    LIVE_KEY_LAKE_LAYOUT, LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY, LIVE_KEY_PREDATOR_FRACTION,
    LIVE_KEY_PREY_CAP, LIVE_KEY_REPRODUCTION, LIVE_KEY_THIRST_STEERING, LIVE_KEY_TREES,
};
use anima_engine_lib::core::observer::ObserverAction;
use anima_engine_lib::core::resources::{BrainPolicy, LifetimeLearning, VitalStatistics};
use anima_engine_lib::core::snapshot::BuildProvenance;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use anima_engine_lib::core::world_record::{
    load_and_verify_record_dir, verify_record, DeathCauseTag, InputKind, PlatformClass,
    RecordedInput, WorldEvent, WorldRecordError, WorldRecordHeader, WorldRecordRecorder,
    WorldRecordWriterConfig, WORLD_RECORD_SCHEMA_VERSION,
};
use bevy_ecs::prelude::With;

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(label: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "anima-wr2-test-{label}-{}-{nanos}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp record dir");
        Self { path }
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn sample_header(seed: u64, segment_ticks: u64) -> WorldRecordHeader {
    WorldRecordHeader {
        schema_version: WORLD_RECORD_SCHEMA_VERSION,
        run_id: format!("run-{seed}"),
        parent_run_id: None,
        fork_tick: None,
        fork_keyframe_hash: None,
        manifest_fingerprint: Some(0x1111_2222_3333_4444),
        law_fingerprint: Some(0x5555_6666_7777_8888),
        registry_fingerprint: Some(0x9999_AAAA_BBBB_CCCC),
        seed,
        world_identity: WorldIdentity {
            seed: 42,
            generator_version: 1,
            width: 16,
            height: 16,
            checksum: 0xDEAD_BEEF,
        },
        world_artifact_sha256: None,
        model_version: "1".to_string(),
        snapshot_schema_version: 7,
        provenance: BuildProvenance::current(),
        platform_class: PlatformClass::current(),
        deterministic: true,
        initial_world_checksum: 0x1234_5678,
        segment_ticks,
        initial_conditions: vec![],
        era_index: 0,
        prev_era_tail_hash: None,
        omissions: vec![],
    }
}

#[test]
fn tiny_ring_buffer_overflow_reports_drops_never_blocks_and_preserves_chain() {
    let paused = Arc::new(AtomicBool::new(true));
    let mut recorder = WorldRecordRecorder::with_writer(
        sample_header(900_001, 60),
        WorldRecordWriterConfig {
            ring_capacity: 4,
            output_dir: None,
            paused: Some(Arc::clone(&paused)),
            ..Default::default()
        },
    )
    .expect("tiny writer spawns");

    for tick in 1u64..=20 {
        recorder.push_input(RecordedInput {
            tick,
            cause_id: CAUSE_OBSERVER,
            kind: InputKind::ObserverAction(ObserverAction::EvolutionToggled {
                running: tick % 2 == 0,
            }),
        });
        recorder.push_event(
            tick,
            WorldEvent::Death {
                agent_id: format!("agent-{tick}"),
                cause: DeathCauseTag::Starvation,
                age_ticks: tick * 10,
            },
        );
    }

    let dropped_in = recorder.dropped_inputs();
    let dropped_ev = recorder.dropped_events();
    assert!(dropped_in > 0, "tiny paused ring must count dropped inputs");
    assert!(dropped_ev > 0, "tiny paused ring must count dropped events");

    paused.store(false, Ordering::Release);
    recorder.unpark_writer();
    let record = recorder.finish_writer().expect("writer drains");
    let _ = record;

    // Also test a seal with overflow counters persisted and loaded from disk.
    let tmp_overflow = TempDirGuard::new("tiny-overflow-disk");
    let paused2 = Arc::new(AtomicBool::new(true));
    let mut world = bevy_ecs::prelude::World::new();
    let rec2 = WorldRecordRecorder::from_world_with_writer(
        &mut world,
        900_002,
        60,
        true,
        vec![],
        vec![],
        WorldRecordWriterConfig {
            ring_capacity: 4,
            output_dir: Some(tmp_overflow.path.clone()),
            paused: Some(Arc::clone(&paused2)),
            ..Default::default()
        },
    )
    .expect("tiny writer 2 spawns");
    world.insert_resource(rec2);
    let mut sched = bevy_ecs::prelude::Schedule::default();
    sched.add_systems(anima_engine_lib::core::world_record::record_world_record_tick_system);

    {
        let mut rec2 = world.resource_mut::<WorldRecordRecorder>();
        for tick in 1u64..=25 {
            rec2.push_input(RecordedInput {
                tick,
                cause_id: CAUSE_OBSERVER,
                kind: InputKind::ObserverAction(ObserverAction::EvolutionToggled { running: true }),
            });
            rec2.push_event(
                tick,
                WorldEvent::Birth {
                    agent_id: format!("child-{tick}"),
                    parent_id: None,
                    generation: 1,
                    energy_transferred: 25.0,
                },
            );
        }
    }
    for _ in 1u64..=25 {
        sched.run(&mut world); // Advances `last_recorded_tick` to 25 cleanly (`segment_ticks = 60`).
    }
    let (exp_in, exp_ev) = {
        let rec2 = world.resource::<WorldRecordRecorder>();
        (rec2.dropped_inputs(), rec2.dropped_events())
    };
    assert!(exp_in > 0 && exp_ev > 0);
    paused2.store(false, Ordering::Release);
    let _finished2 = world
        .resource_scope::<WorldRecordRecorder, _>(|world, mut rec| {
            rec.flush_and_finish_writer(world)
        })
        .expect("finish 2");
    let (disk2, rep2) =
        load_and_verify_record_dir(&tmp_overflow.path).expect("overflowed disk dir verifies");
    assert_eq!(disk2.segments.len(), 1);
    assert_eq!(rep2.total_dropped_inputs, exp_in);
    assert_eq!(rep2.total_dropped_events, exp_ev);
    assert!(!rep2.is_complete());
    assert!(!rep2.replayable);
}

#[test]
fn atomic_disk_writer_persists_verifiable_segments_and_tolerates_tmp_crash_residue() {
    let tmp = TempDirGuard::new("atomic-disk");
    let mut off = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &InitialConditionSet::new(vec![]),
        &[],
        700_010,
        (16, 16),
        360,
    )
    .expect("off builds");
    let mut on = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &InitialConditionSet::new(vec![]),
        &[],
        700_010,
        (16, 16),
        360,
    )
    .expect("on builds");
    {
        let mut world = on.world();
        let recorder = WorldRecordRecorder::from_world_with_writer(
            &mut world,
            700_010,
            60,
            true,
            vec![],
            vec![],
            WorldRecordWriterConfig {
                ring_capacity: 512,
                output_dir: Some(tmp.path.clone()),
                ..Default::default()
            },
        )
        .expect("async disk recorder builds");
        world.insert_resource(recorder);
    }

    for tick in 1u64..=360 {
        off.run_schedule_once();
        on.run_schedule_once();
        if tick % 60 == 0 {
            assert_eq!(off.checksum(), on.checksum(), "diverged at tick {tick}");
        }
    }

    let final_csum = on.checksum();
    let mem_record = {
        let mut world = on.world();
        let mut rec = world
            .get_resource_mut::<WorldRecordRecorder>()
            .expect("recorder present");
        rec.finish_writer().expect("async disk writer finishes")
    };

    // Simulate a truncated `.tmp` leftover from a killed process before loading the directory.
    std::fs::write(
        tmp.path.join(".segment_000006.json.999999.tmp"),
        b"{\"partial_corrupt_json\":",
    )
    .expect("write simulated crash tmp file");

    let entries: Vec<String> = std::fs::read_dir(&tmp.path)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    assert!(entries.iter().any(|n| n == "header.json"));
    assert!(entries.iter().any(|n| n == "segment_000000.json"));
    assert!(entries.iter().any(|n| n == "segment_000005.json"));

    let (disk_record, disk_report) =
        load_and_verify_record_dir(&tmp.path).expect("persisted record dir verifies");
    assert_eq!(disk_record, mem_record);
    assert_eq!(disk_report.segment_count, 6);
    assert_eq!(disk_report.first_tick, Some(1));
    assert_eq!(disk_report.last_tick, Some(360));
    assert!(disk_report.is_complete());
    assert!(disk_report.replayable);
    assert_eq!(disk_record.segments[5].body.end_checksum, final_csum);

    // Tamper 1 byte inside persisted segment 2 (`segment_000002.json`) -> breaks at index 2.
    let seg2_path = tmp.path.join("segment_000002.json");
    let seg2_raw = std::fs::read_to_string(&seg2_path).unwrap();
    let needle = format!(
        "\"end_checksum\": {}",
        disk_record.segments[2].body.end_checksum
    );
    assert!(seg2_raw.contains(&needle));
    let tampered = seg2_raw.replacen(
        &needle,
        &format!(
            "\"end_checksum\": {}",
            disk_record.segments[2].body.end_checksum ^ 1
        ),
        1,
    );
    std::fs::write(&seg2_path, tampered).unwrap();
    let err = load_and_verify_record_dir(&tmp.path).expect_err("tampered segment must fail");
    assert_eq!(err.broken_segment_index(), Some(2));
    assert!(matches!(
        err,
        WorldRecordError::SegmentHashMismatch {
            segment_index: 2,
            ..
        }
    ));
}

fn kv(pairs: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect())
}

#[test]
fn wr2_async_writer_preserves_zero_feedback_on_default_and_learning_worlds() {
    const CHECKPOINTS: [u64; 4] = [0, 180, 1_800, 18_000];
    const SEGMENT_TICKS: u64 = 180;

    // 1. Default world ON (WR2 async writer) vs OFF across 18,000 ticks.
    let tmp_default = TempDirGuard::new("no-feedback-default");
    let initial_default = kv(&[]);
    let mut def_off = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial_default,
        &[],
        700_011,
        (16, 16),
        CHECKPOINTS[3],
    )
    .unwrap();
    let mut def_on = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial_default,
        &[],
        700_011,
        (16, 16),
        CHECKPOINTS[3],
    )
    .unwrap();
    {
        let mut world = def_on.world();
        let rec = WorldRecordRecorder::from_world_with_writer(
            &mut world,
            700_011,
            SEGMENT_TICKS,
            true,
            vec![],
            vec![],
            WorldRecordWriterConfig {
                output_dir: Some(tmp_default.path.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        world.insert_resource(rec);
    }

    let run_checks = |a: &mut LiveExperimentAdapter| -> Vec<(u64, u32)> {
        let mut out = vec![(0, a.checksum())];
        for target in CHECKPOINTS.into_iter().skip(1) {
            while a.tick() < target {
                a.run_schedule_once();
            }
            out.push((target, a.checksum()));
        }
        out
    };
    let off_checks = run_checks(&mut def_off);
    let on_checks = run_checks(&mut def_on);
    assert_eq!(on_checks, off_checks);
    {
        let mut world = def_on.world();
        world
            .get_resource_mut::<WorldRecordRecorder>()
            .unwrap()
            .finish_writer()
            .unwrap();
    }
    let (def_disk, def_rep) = load_and_verify_record_dir(&tmp_default.path).unwrap();
    assert_eq!(def_rep.segment_count, 100);
    assert!(def_rep.is_complete());
    assert!(def_rep.replayable);
    let _ = &def_disk;

    // 2. Learning + reproduction + mortality world ON (WR2 async writer) vs OFF across 18,000 ticks.
    let tmp_active = TempDirGuard::new("no-feedback-active");
    let build_active = |with_writer: bool| -> LiveExperimentAdapter {
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
            700_012,
            (16, 16),
            CHECKPOINTS[3],
        )
        .unwrap();
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
            if with_writer {
                let rec = WorldRecordRecorder::from_world_with_writer(
                    &mut world,
                    700_012,
                    SEGMENT_TICKS,
                    true,
                    initial.values.clone(),
                    vec!["gpu_microstate".to_string()],
                    WorldRecordWriterConfig {
                        output_dir: Some(tmp_active.path.clone()),
                        ..Default::default()
                    },
                )
                .unwrap();
                world.insert_resource(rec);
            }
        }
        adapter
    };

    let mut act_off = build_active(false);
    let mut act_on = build_active(true);
    let act_off_checks = run_checks(&mut act_off);
    let act_on_checks = run_checks(&mut act_on);
    assert_eq!(act_on_checks, act_off_checks);

    {
        let mut world = act_on.world();
        let vitals = world.resource::<VitalStatistics>().clone();
        assert!(vitals.total_births > 0 && vitals.total_deaths > 0);
        let learned_count = world
            .query_filtered::<&AgentBrain, With<Agent>>()
            .iter(&world)
            .filter(|b| b.learned.is_some())
            .count();
        assert!(learned_count > 0);
        let mem_rec = world
            .get_resource_mut::<WorldRecordRecorder>()
            .unwrap()
            .finish_writer()
            .unwrap();
        assert!(verify_record(&mem_rec).unwrap().is_complete());
    }
    let (disk_rec, disk_rep) = load_and_verify_record_dir(&tmp_active.path).unwrap();
    assert_eq!(disk_rep.segment_count, 100);
    assert!(disk_rep.total_events > 0);
    assert!(disk_rep.is_complete());
    assert!(disk_rep.replayable);
    assert_eq!(disk_rec.segments[99].body.end_checksum, act_on_checks[3].1);
    // Confirm tracked age_ticks on Death events reflects elapsed simulation ticks (non-zero when older agents die).
    let max_death_age = disk_rec
        .segments
        .iter()
        .flat_map(|s| s.body.events.iter())
        .filter_map(|e| match e.kind {
            WorldEvent::Death { age_ticks, .. } => Some(age_ticks),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    assert!(
        max_death_age > 0,
        "WR2 age tracking must record positive death ages"
    );
}
