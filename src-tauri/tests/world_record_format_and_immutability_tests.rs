//! WR1 format, canonical encoding, bounded overflow, and chain immutability tests (ADR-0008).

use anima_engine_lib::core::causal::CAUSE_OBSERVER;
use anima_engine_lib::core::intervention::{Curve, InterventionCommand, InterventionKind, Region};
use anima_engine_lib::core::live_experiment::{LIVE_KEYS, LIVE_MODEL_VERSION};
use anima_engine_lib::core::observer::ObserverAction;
use anima_engine_lib::core::snapshot::{BuildProvenance, SCHEMA_VERSION};
use anima_engine_lib::core::world_artifact::WorldIdentity;
use anima_engine_lib::core::world_record::{
    canonical_encode, encode_record_json, hex_encode, verify_record, verify_record_json,
    DeathCauseTag, InputKind, PlatformClass, RecordedEvent, RecordedInput, SegmentBody, WorldEvent,
    WorldKeyframe, WorldRecord, WorldRecordError, WorldRecordHeader, WORLD_RECORD_SCHEMA_VERSION,
};

fn sample_header(deterministic: bool) -> WorldRecordHeader {
    WorldRecordHeader {
        schema_version: WORLD_RECORD_SCHEMA_VERSION,
        run_id: "run-700001".to_string(),
        parent_run_id: None,
        fork_tick: None,
        fork_keyframe_hash: None,
        manifest_fingerprint: Some(0x1111_2222_3333_4444),
        law_fingerprint: Some(0x5555_6666_7777_8888),
        registry_fingerprint: Some(0x9999_AAAA_BBBB_CCCC),
        seed: 700_001,
        world_identity: WorldIdentity {
            seed: 42,
            generator_version: 1,
            width: 16,
            height: 16,
            checksum: 0xDEAD_BEEF,
            sim_world_sha256: None,
        },
        world_artifact_sha256: Some("a".repeat(64)),
        model_version: LIVE_MODEL_VERSION.to_string(),
        snapshot_schema_version: SCHEMA_VERSION,
        provenance: BuildProvenance::current(),
        platform_class: PlatformClass::current(),
        deterministic,
        initial_world_checksum: 0x1234_5678,
        segment_ticks: 60,
        initial_conditions: vec![
            ("live.mortality".to_string(), 1.0),
            ("live.founders".to_string(), 10.0),
        ],
        era_index: 0,
        prev_era_tail_hash: None,
        omissions: vec!["gpu_microstate".to_string()],
    }
}

fn sample_keyframe(tick: u64, births: u64, deaths: u64) -> WorldKeyframe {
    WorldKeyframe {
        tick,
        agent_count: 10,
        predator_count: 3,
        prey_count: 7,
        standing_crop_eu: 1200.5,
        detritus_eu: 45.25,
        total_agent_energy_eu: 750.0,
        total_births: births,
        total_deaths: deaths,
    }
}

fn build_six_segment_record() -> WorldRecord {
    let mut rec = WorldRecord::build_empty(sample_header(true)).expect("header builds");
    let causes = [
        DeathCauseTag::Starvation,
        DeathCauseTag::Dehydration,
        DeathCauseTag::Predation,
        DeathCauseTag::Reclaim,
        DeathCauseTag::EpochReplacement,
        DeathCauseTag::Starvation,
    ];
    for idx in 0u64..6 {
        let tick_start = idx * 60 + 1;
        let tick_end = (idx + 1) * 60;
        let event = match idx {
            0 => WorldEvent::Birth {
                agent_id: "birth-1".into(),
                parent_id: Some("founder-0".into()),
                generation: 1,
                energy_transferred: 32.5,
            },
            1 => WorldEvent::Death {
                agent_id: "agent-1".into(),
                cause: causes[idx as usize],
                age_ticks: 90,
            },
            2 => WorldEvent::Kill {
                predator_id: "pred-0".into(),
                prey_id: "prey-2".into(),
                damage: 18.0,
                energy_transferred: 7.2,
            },
            3 => WorldEvent::Migration {
                agent_id: "migrant-3".into(),
                from_shard: "127.0.0.1:7000".into(),
                to_shard: "127.0.0.1:7001".into(),
            },
            4 => WorldEvent::Chronicle {
                category: "Drought".into(),
                summary: "seasonal dry spell".into(),
            },
            _ => WorldEvent::EpochReplacement {
                epoch: 2,
                replaced_count: 3,
            },
        };
        let input = match idx % 3 {
            0 => RecordedInput {
                tick: tick_start,
                cause_id: 1,
                kind: InputKind::Intervention(InterventionCommand {
                    id: idx as u32 + 1,
                    cause_id: 1,
                    kind: InterventionKind::RainfallDelta,
                    region: Region::Global,
                    start_tick: tick_start,
                    duration_ticks: 10,
                    intensity: 0.25,
                    signed_negative: false,
                    curve: Curve::Step,
                    reversible: true,
                }),
            },
            1 => RecordedInput {
                tick: tick_start + 1,
                cause_id: CAUSE_OBSERVER,
                kind: InputKind::ObserverAction(ObserverAction::EvolutionSettingsChanged {
                    mutation_rate: 0.05,
                    selection_bias: 1.5,
                    grid_resolution: 50,
                }),
            },
            _ => RecordedInput {
                tick: tick_start + 2,
                cause_id: CAUSE_OBSERVER,
                kind: InputKind::ChronicleNote {
                    event_type: "OperatorMark".into(),
                    title: "Checkpoint note".into(),
                    description: format!("segment {idx}"),
                },
            },
        };
        let body = SegmentBody {
            index: idx,
            tick_start,
            tick_end,
            end_checksum: 0xAA00_0000 + idx as u32,
            keyframe: sample_keyframe(tick_end, idx + 1, idx),
            keyframe_snapshot_sha256: None,
            inputs: vec![input],
            events: vec![
                RecordedEvent {
                    tick: tick_start + 5,
                    kind: event,
                },
                RecordedEvent {
                    tick: tick_start + 6,
                    kind: WorldEvent::Death {
                        agent_id: format!("cause-sample-{idx}"),
                        cause: causes[idx as usize],
                        age_ticks: 120 + idx,
                    },
                },
            ],
            dropped_inputs_since_prev: 0,
            dropped_events_since_prev: 0,
        };
        rec.append_segment(body).expect("append segment");
    }
    rec
}

#[test]
fn wr1_does_not_add_any_live_experiment_keys() {
    assert_eq!(LIVE_KEYS.len(), 31);
}

#[test]
fn header_validates_platform_class_and_fork_links() {
    let hdr = sample_header(true);
    assert!(!hdr.platform_class.os.is_empty());
    assert!(!hdr.platform_class.arch.is_empty());

    let mut empty_run = hdr.clone();
    empty_run.run_id = "   ".to_string();
    assert!(matches!(
        WorldRecord::build_empty(empty_run),
        Err(WorldRecordError::EmptyRunId)
    ));

    let mut bad_fork = hdr.clone();
    bad_fork.parent_run_id = Some("run-parent".to_string());
    bad_fork.fork_tick = None;
    assert!(matches!(
        WorldRecord::build_empty(bad_fork),
        Err(WorldRecordError::InvalidForkLink)
    ));

    let mut bad_fork_tick = hdr;
    bad_fork_tick.parent_run_id = None;
    bad_fork_tick.fork_tick = Some(180);
    assert!(matches!(
        WorldRecord::build_empty(bad_fork_tick),
        Err(WorldRecordError::InvalidForkLink)
    ));
}

#[test]
fn round_trip_covers_all_v1_events_and_death_causes() {
    let record = build_six_segment_record();
    let json = encode_record_json(&record).expect("encode pretty json");
    let (decoded, report) = verify_record_json(&json).expect("verify round-trip json");
    assert_eq!(decoded, record);
    assert_eq!(report.segment_count, 6);
    assert_eq!(report.first_tick, Some(1));
    assert_eq!(report.last_tick, Some(360));
    assert_eq!(report.total_inputs, 6);
    assert_eq!(report.total_events, 12);
    assert!(report.is_complete());
    assert!(report.replayable);

    let text = String::from_utf8(json).expect("utf8 json");
    for cause_tag in [
        "starvation",
        "dehydration",
        "predation",
        "reclaim",
        "epoch_replacement",
    ] {
        assert!(
            text.contains(cause_tag),
            "missing death cause variant '{cause_tag}' in JSON"
        );
    }
}

#[test]
fn deterministic_false_remains_readable_but_non_replayable() {
    let mut rec = WorldRecord::build_empty(sample_header(false)).expect("non-det header");
    rec.append_segment(SegmentBody {
        index: 0,
        tick_start: 1,
        tick_end: 60,
        end_checksum: 1,
        keyframe: sample_keyframe(60, 0, 0),
        keyframe_snapshot_sha256: None,
        inputs: vec![],
        events: vec![],
        dropped_inputs_since_prev: 0,
        dropped_events_since_prev: 0,
    })
    .expect("append");
    let report = verify_record(&rec).expect("readable record verifies");
    assert!(!report.replayable);
    assert!(report.is_complete());
}

#[test]
fn canonical_encoding_normalizes_negative_zero_and_rejects_nonfinite_floats() {
    let mut k_neg_zero = sample_keyframe(1, 0, 0);
    k_neg_zero.detritus_eu = -0.0_f64;
    let mut k_pos_zero = sample_keyframe(1, 0, 0);
    k_pos_zero.detritus_eu = 0.0_f64;
    assert_eq!(
        canonical_encode(&k_neg_zero).unwrap(),
        canonical_encode(&k_pos_zero).unwrap()
    );

    let mut bad_header = sample_header(true);
    bad_header
        .initial_conditions
        .push(("live.nan".into(), f64::NAN));
    let err = WorldRecord::build_empty(bad_header).expect_err("NaN in header rejected");
    assert!(matches!(err, WorldRecordError::NonFiniteFloat { .. }));

    let mut rec = WorldRecord::build_empty(sample_header(true)).unwrap();
    let mut bad_body = SegmentBody {
        index: 0,
        tick_start: 1,
        tick_end: 60,
        end_checksum: 0,
        keyframe: sample_keyframe(60, 0, 0),
        keyframe_snapshot_sha256: None,
        inputs: vec![],
        events: vec![],
        dropped_inputs_since_prev: 0,
        dropped_events_since_prev: 0,
    };
    bad_body.keyframe.standing_crop_eu = f64::INFINITY;
    let err = rec
        .append_segment(bad_body)
        .expect_err("Infinity in body rejected");
    assert!(matches!(err, WorldRecordError::NonFiniteFloat { .. }));
}

#[test]
fn overflow_counters_flag_incomplete_record() {
    let mut rec = WorldRecord::build_empty(sample_header(true)).unwrap();
    rec.append_segment(SegmentBody {
        index: 0,
        tick_start: 1,
        tick_end: 60,
        end_checksum: 99,
        keyframe: sample_keyframe(60, 0, 0),
        keyframe_snapshot_sha256: None,
        inputs: vec![],
        events: vec![],
        dropped_inputs_since_prev: 2,
        dropped_events_since_prev: 7,
    })
    .unwrap();
    let report = verify_record(&rec).unwrap();
    assert_eq!(report.total_dropped_inputs, 2);
    assert_eq!(report.total_dropped_events, 7);
    assert!(!report.is_complete());
    assert!(!report.replayable);
}

#[test]
fn single_byte_flip_in_any_segment_breaks_exact_segment_and_reseal_breaks_next() {
    let baseline = build_six_segment_record();
    assert!(verify_record(&baseline).is_ok());

    for k in 0..baseline.segments.len() {
        // Case A: Flip 1 byte in segment `k`'s body JSON representation without re-sealing.
        let mut tampered_body = baseline.clone();
        tampered_body.segments[k].body.end_checksum ^= 0x01;
        let err = verify_record(&tampered_body).expect_err("body flip must break chain");
        assert_eq!(
            err.broken_segment_index(),
            Some(k),
            "segment {k} body flip reported wrong broken index: {err}"
        );
        assert!(matches!(
            err,
            WorldRecordError::SegmentHashMismatch { segment_index, .. } if segment_index == k
        ));

        // Case B: Flip 1 ASCII hex nibble inside the serialized JSON digest of segment `k`.
        let mut serialized = String::from_utf8(encode_record_json(&baseline).unwrap()).unwrap();
        let old_hex = hex_encode(&baseline.segments[k].hash);
        let mut flipped_bytes = old_hex.clone().into_bytes();
        flipped_bytes[0] = if flipped_bytes[0] == b'0' { b'1' } else { b'0' };
        let flipped_hex = String::from_utf8(flipped_bytes).unwrap();
        // Replace first occurrence matching segment `k`'s `"hash"` field specifically.
        let target = format!("\"hash\": \"{old_hex}\"");
        let replacement = format!("\"hash\": \"{flipped_hex}\"");
        assert!(serialized.contains(&target));
        serialized = serialized.replacen(&target, &replacement, 1);
        let err_json = verify_record_json(serialized.as_bytes())
            .expect_err("serialized byte flip in hash hex must fail");
        assert_eq!(err_json.broken_segment_index(), Some(k));

        // Case C: Flip segment `k` body AND re-seal ONLY `segments[k].hash` (broken at `k + 1`).
        if k + 1 < baseline.segments.len() {
            let mut partial_reseal = baseline.clone();
            partial_reseal.segments[k].body.end_checksum ^= 0x01;
            partial_reseal.segments[k].hash =
                anima_engine_lib::core::world_record::compute_segment_hash(
                    &partial_reseal.segments[k].prev_hash,
                    &partial_reseal.segments[k].body,
                )
                .unwrap();
            let err_next = verify_record(&partial_reseal)
                .expect_err("re-sealing k alone must fail verification at k + 1");
            assert_eq!(
                err_next.broken_segment_index(),
                Some(k + 1),
                "re-sealing {k} should break at {}, got {err_next}",
                k + 1
            );
            assert!(matches!(
                err_next,
                WorldRecordError::BrokenChain { segment_index, .. } if segment_index == k + 1
            ));
        }
    }

    // Header mutation checks.
    let mut bad_header = baseline.clone();
    bad_header.header.seed += 1;
    let err_hdr = verify_record(&bad_header).expect_err("header flip fails");
    assert!(matches!(
        err_hdr,
        WorldRecordError::HeaderHashMismatch { .. }
    ));

    // Re-sealing header_hash alone without rewriting segment 0 breaks at index 0.
    bad_header.header_hash =
        anima_engine_lib::core::world_record::header_digest(&bad_header.header).unwrap();
    let err_seg0 = verify_record(&bad_header).expect_err("re-sealed header fails segment 0 anchor");
    assert_eq!(err_seg0.broken_segment_index(), Some(0));
}
