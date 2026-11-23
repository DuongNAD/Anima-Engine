//! Card FOUNDER-2SEG (`live.founder_genotype`, D5 option (b)): opt-in declared 2-segment genesis
//! founder (`0.0`/absent = legacy 3-segment founder, bit-identical to pre-card runs; `1.0` =
//! 2-segment founder minus tail leaf: nodes `[0, 1]`, masses `[1.5, 1.0]`, edge `0 -> 1`).
//!
//! Covers every acceptance requirement without pinning any absolute world checksum:
//! 1. Key registration (`LIVE_KEY_FOUNDER_GENOTYPE = "live.founder_genotype"`, `LIVE_KEYS.len() = 31`),
//!    exact `{0.0, 1.0}` validation, `LiveWorldConfig::default().founder_genotype = 0`, serde
//!    backward compatibility, and `live.founders = 0` no-op under all modes.
//! 2. `absent` and `0.0` produce identical checksums, observables, and founder genotypes at every
//!    sampled tick (default baseline and active ecology).
//! 3. Value `1.0` spawns founders with exact 2-node morphology (`[1.5, 1.0]`, edge `0 -> 1` with
//!    unchanged anchor/axis), exact `serde_json` SHA-256
//!    `dfeb276205f818acf3f34f5d716cbf163598a03a23ffc6964ff85753e280c719`, two spawned ECS segments
//!    per founder, and tick-0 `live.trait_mass_mean = 2.5` / `live.trait_mass_var = 0.0`.
//! 4. Save and restore (via sealed on-disk snapshot round-trip) carry all genotypes unchanged without
//!    re-rolling at `genesis`, matching an uninterrupted run on checksums and observables.

use anima_engine_lib::core::agent_systems::{AgentGeneration, AgentGenotype, AgentLineageId};
use anima_engine_lib::core::ecs::{Agent, ParentAgent, Segment};
use anima_engine_lib::core::experiment::{ExperimentError, InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LiveSnapshot, LiveWorldConfig, LIVE_KEYS, LIVE_KEY_DRIVE_FLOOR,
    LIVE_KEY_ECOLOGICAL_FITNESS, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS, LIVE_KEY_FOUNDER_GENOTYPE,
    LIVE_KEY_FOUNDER_LAYOUT, LIVE_KEY_LAKES, LIVE_KEY_LAND_MASK, LIVE_KEY_LOCOMOTION,
    LIVE_KEY_MORTALITY, LIVE_KEY_MUTATION_RATE, LIVE_KEY_PREDATOR_FRACTION, LIVE_KEY_REPRODUCTION,
    LIVE_KEY_THIRST_STEERING, LIVE_KEY_TREES,
};
use anima_engine_lib::core::sha256::sha256_hex;
use anima_engine_lib::core::snapshot::{self, SnapshotEnvelope};
use anima_engine_lib::evolution::genotype::MorphologyGenotype;
use bevy_ecs::prelude::*;
use glam::Vec3;

const EXPECTED_2SEG_SHA256: &str =
    "dfeb276205f818acf3f34f5d716cbf163598a03a23ffc6964ff85753e280c719";
const EXPECTED_LEGACY_3SEG_SHA256: &str =
    "9067cbf53fd6dbb561e99c1b1f690b2a1633e56522499309793b788354f5c808";

fn initial(pairs: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(pairs.iter().map(|(k, v)| ((*k).to_string(), *v)).collect())
}

fn obs_value(obs: &[(String, f64)], id: &str) -> f64 {
    obs.iter()
        .find(|(k, _)| k == id)
        .unwrap_or_else(|| panic!("missing observable {id}"))
        .1
}

fn genotype_sha256(genotype: &MorphologyGenotype) -> String {
    let bytes = serde_json::to_vec(genotype).expect("MorphologyGenotype serializes to JSON");
    sha256_hex(&bytes)
}

fn through_disk(snapshot: &LiveSnapshot, label: &str) -> LiveSnapshot {
    let dir =
        std::env::temp_dir().join(format!("anima_founder_2seg_{}_{label}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("checkpoint.json");
    let envelope = SnapshotEnvelope::seal(snapshot.saved.clone()).expect("seal snapshot");
    snapshot::write_atomic(&path, &envelope).expect("write atomic snapshot");
    let saved = snapshot::read(&path).expect("read snapshot back");
    let _ = std::fs::remove_dir_all(&dir);
    LiveSnapshot {
        saved,
        config: snapshot.config,
        seed: snapshot.seed,
    }
}

#[test]
fn key_is_registered_validated_and_serde_backward_compatible() {
    assert_eq!(LIVE_KEY_FOUNDER_GENOTYPE, "live.founder_genotype");
    assert_eq!(LIVE_KEYS.len(), 31);
    assert!(LIVE_KEYS.contains(&LIVE_KEY_FOUNDER_GENOTYPE));
    assert_eq!(LiveWorldConfig::default().founder_genotype, 0);

    let cfg_absent =
        LiveWorldConfig::from_initial_conditions(&initial(&[])).expect("empty initial parses");
    assert_eq!(cfg_absent.founder_genotype, 0);

    let cfg_zero =
        LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_FOUNDER_GENOTYPE, 0.0)]))
            .expect("0.0 is valid");
    assert_eq!(cfg_zero.founder_genotype, 0);

    let cfg_one =
        LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_FOUNDER_GENOTYPE, 1.0)]))
            .expect("1.0 is valid");
    assert_eq!(cfg_one.founder_genotype, 1);

    for bad in [-1.0, -0.5, 0.5, 1.5, 2.0, 3.0] {
        let err =
            LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_FOUNDER_GENOTYPE, bad)]))
                .expect_err("out-of-range live.founder_genotype must be rejected");
        assert!(
            matches!(err, ExperimentError::OutOfRange { .. }),
            "{bad}: {err:?}"
        );
    }
    for non_finite in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let err = LiveWorldConfig::from_initial_conditions(&initial(&[(
            LIVE_KEY_FOUNDER_GENOTYPE,
            non_finite,
        )]))
        .expect_err("non-finite live.founder_genotype must be rejected");
        assert!(
            matches!(err, ExperimentError::NotFinite { .. }),
            "{non_finite}: {err:?}"
        );
    }

    // Config saved before `founder_genotype` existed must deserialize to `0` (legacy 3-segment).
    let mut json = serde_json::to_value(LiveWorldConfig::default()).expect("config to json");
    json.as_object_mut()
        .expect("object")
        .remove("founder_genotype");
    let restored: LiveWorldConfig =
        serde_json::from_value(json).expect("backward compatible serde");
    assert_eq!(restored.founder_genotype, 0);

    // Zero founders is a valid no-op across both modes and absent.
    for val in [None, Some(0.0), Some(1.0)] {
        let mut pairs = vec![(LIVE_KEY_FOUNDERS, 0.0)];
        if let Some(v) = val {
            pairs.push((LIVE_KEY_FOUNDER_GENOTYPE, v));
        }
        let adapter = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &initial(&pairs),
            &[],
            7,
            (16, 16),
            10,
        )
        .expect("zero founders builds cleanly");
        assert_eq!(adapter.snapshot().saved.agents.len(), 0);
    }
}

#[test]
fn absent_and_zero_are_bit_identical_across_ticks() {
    let run_case = |active_ecology: bool, val: Option<f64>| {
        let mut pairs = vec![
            (LIVE_KEY_FOUNDERS, 12.0),
            (LIVE_KEY_PREDATOR_FRACTION, 0.3),
            (LIVE_KEY_TREES, 8.0),
            (LIVE_KEY_LAKES, 2.0),
            (LIVE_KEY_FOOD_CAP, 50.0),
        ];
        if active_ecology {
            pairs.extend_from_slice(&[
                (LIVE_KEY_MORTALITY, 1.0),
                (LIVE_KEY_REPRODUCTION, 1.0),
                (LIVE_KEY_LOCOMOTION, 1.0),
                (LIVE_KEY_DRIVE_FLOOR, 0.5),
                (LIVE_KEY_THIRST_STEERING, 1.0),
                (LIVE_KEY_ECOLOGICAL_FITNESS, 1.0),
                (LIVE_KEY_LAND_MASK, 1.0),
                (LIVE_KEY_FOUNDER_LAYOUT, 1.0),
            ]);
        }
        if let Some(v) = val {
            pairs.push((LIVE_KEY_FOUNDER_GENOTYPE, v));
        }
        let ticks = 500;
        let mut adapter = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &initial(&pairs),
            &[],
            880001,
            (16, 16),
            ticks,
        )
        .expect("adapter builds");

        let mut samples = Vec::new();
        samples.push((0u64, adapter.checksum(), adapter.observables()));
        for t in 1..=ticks {
            adapter.run_schedule_once();
            if matches!(t, 1 | 50 | 200 | 500) {
                samples.push((t, adapter.checksum(), adapter.observables()));
            }
        }
        let founder_hashes: Vec<String> = adapter
            .snapshot()
            .saved
            .agents
            .iter()
            .map(|a| genotype_sha256(&a.genotype))
            .collect();
        (samples, founder_hashes)
    };

    for active_ecology in [false, true] {
        let (samples_absent, hashes_absent) = run_case(active_ecology, None);
        let (samples_zero, hashes_zero) = run_case(active_ecology, Some(0.0));
        assert_eq!(
            samples_absent, samples_zero,
            "absent and 0.0 must be bit-identical at every sampled tick (ecology={active_ecology})"
        );
        assert_eq!(hashes_absent, hashes_zero);
    }

    // Confirm legacy baseline still uses exact 3-segment founder SHA-256 at tick 0.
    let legacy = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial(&[(LIVE_KEY_FOUNDERS, 4.0)]),
        &[],
        42,
        (16, 16),
        10,
    )
    .expect("legacy builds");
    for agent in &legacy.snapshot().saved.agents {
        assert_eq!(agent.genotype.nodes.len(), 3);
        assert_eq!(
            genotype_sha256(&agent.genotype),
            EXPECTED_LEGACY_3SEG_SHA256
        );
    }
}

#[test]
fn value_one_spawns_exact_two_segment_founder_with_stated_sha256() {
    for (layout, land_mask) in [(0.0, 0.0), (2.0, 1.0)] {
        let adapter = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &initial(&[
                (LIVE_KEY_FOUNDERS, 12.0),
                (LIVE_KEY_PREDATOR_FRACTION, 0.3),
                (LIVE_KEY_TREES, 8.0),
                (LIVE_KEY_LAKES, 2.0),
                (LIVE_KEY_FOOD_CAP, 50.0),
                (LIVE_KEY_FOUNDER_LAYOUT, layout),
                (LIVE_KEY_LAND_MASK, land_mask),
                (LIVE_KEY_FOUNDER_GENOTYPE, 1.0),
            ]),
            &[],
            880001,
            (16, 16),
            10,
        )
        .expect("2-segment adapter builds");

        let obs = adapter.observables();
        assert_eq!(obs_value(&obs, "live.agent_count"), 12.0);
        assert!((obs_value(&obs, "live.trait_mass_mean") - 2.5).abs() < 1e-12);
        assert!(obs_value(&obs, "live.trait_mass_var").abs() < 1e-12);

        let mut world = adapter.world();
        let mut agent_q =
            world.query_filtered::<(Entity, &AgentGenotype, &AgentLineageId, &AgentGeneration), With<Agent>>();
        let founders: Vec<(Entity, MorphologyGenotype, String, u32)> = agent_q
            .iter(&world)
            .map(|(e, g, lid, gen)| (e, g.0.clone(), lid.0.clone(), gen.0))
            .collect();
        assert_eq!(founders.len(), 12);

        let mut seg_q = world.query::<(&ParentAgent, &Segment)>();
        let all_segments: Vec<(Entity, u32)> =
            seg_q.iter(&world).map(|(p, s)| (p.0, s.id)).collect();

        for (root, g, lid, gen) in &founders {
            assert_eq!(*gen, 0, "founder {lid} must be generation 0");
            assert_eq!(g.nodes.len(), 2, "founder {lid} must have 2 nodes");
            assert_eq!(g.nodes[0].id, 0);
            assert_eq!(g.nodes[0].length.to_bits(), 1.0f32.to_bits());
            assert_eq!(g.nodes[0].radius.to_bits(), 0.2f32.to_bits());
            assert_eq!(g.nodes[0].mass.to_bits(), 1.5f32.to_bits());
            assert_eq!(g.nodes[1].id, 1);
            assert_eq!(g.nodes[1].length.to_bits(), 1.0f32.to_bits());
            assert_eq!(g.nodes[1].radius.to_bits(), 0.2f32.to_bits());
            assert_eq!(g.nodes[1].mass.to_bits(), 1.0f32.to_bits());

            assert_eq!(g.edges.len(), 1, "founder {lid} must have 1 edge");
            assert_eq!(g.edges[0].source_node, 0);
            assert_eq!(g.edges[0].target_node, 1);
            assert_eq!(
                g.edges[0].joint_anchor,
                Vec3::new(1.0, 0.0, 0.0),
                "0->1 anchor must remain untouched"
            );
            assert_eq!(
                g.edges[0].joint_axis,
                Vec3::new(0.0, 0.0, 1.0),
                "0->1 axis must remain untouched"
            );
            assert_eq!(g.total_mass().to_bits(), 2.5f32.to_bits());
            assert_eq!(
                genotype_sha256(g),
                EXPECTED_2SEG_SHA256,
                "canonical MorphologyGenotype SHA-256 mismatch for {lid}"
            );

            let owned_segs: Vec<u32> = all_segments
                .iter()
                .filter(|(p, _)| p == root)
                .map(|(_, id)| *id)
                .collect();
            assert_eq!(
                owned_segs.len(),
                2,
                "decoded agent {lid} must spawn exactly 2 ECS segments"
            );
        }
    }
}

#[test]
fn save_and_restore_carry_two_segment_founders_without_rerolling() {
    let run_restore_check = |k_save: u64, n_total: u64, mutation_rate: f64, label: &str| {
        let initial_set = initial(&[
            (LIVE_KEY_FOUNDERS, 12.0),
            (LIVE_KEY_PREDATOR_FRACTION, 0.3),
            (LIVE_KEY_TREES, 8.0),
            (LIVE_KEY_LAKES, 2.0),
            (LIVE_KEY_FOOD_CAP, 50.0),
            (LIVE_KEY_MORTALITY, 1.0),
            (LIVE_KEY_REPRODUCTION, 1.0),
            (LIVE_KEY_LOCOMOTION, 1.0),
            (LIVE_KEY_DRIVE_FLOOR, 0.5),
            (LIVE_KEY_THIRST_STEERING, 1.0),
            (LIVE_KEY_ECOLOGICAL_FITNESS, 1.0),
            (LIVE_KEY_LAND_MASK, 1.0),
            (LIVE_KEY_FOUNDER_LAYOUT, 1.0),
            (LIVE_KEY_MUTATION_RATE, mutation_rate),
            (LIVE_KEY_FOUNDER_GENOTYPE, 1.0),
        ]);

        let mut straight = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &initial_set,
            &[],
            880002,
            (16, 16),
            n_total,
        )
        .expect("straight adapter builds");

        let mut first = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &initial_set,
            &[],
            880002,
            (16, 16),
            n_total,
        )
        .expect("split adapter builds");

        for _ in 0..k_save {
            straight.run_schedule_once();
            first.run_schedule_once();
        }
        assert_eq!(first.checksum(), straight.checksum());
        // Drain interval accumulators (e.g. `live.births`) equally before snapshot, since a
        // fresh world restored from disk starts its interval counters at zero.
        assert_eq!(first.observables(), straight.observables());

        let pre_snap = first.snapshot();
        assert!(
            !pre_snap.saved.agents.is_empty(),
            "{label}: population must still be alive at save tick {k_save}"
        );
        let pre_hashes: Vec<(String, String)> = pre_snap
            .saved
            .agents
            .iter()
            .map(|a| (a.lineage_id.clone(), genotype_sha256(&a.genotype)))
            .collect();
        if k_save == 0 {
            for (lid, h) in &pre_hashes {
                assert_eq!(h, EXPECTED_2SEG_SHA256, "tick 0 {lid}");
            }
        }

        let roundtripped = through_disk(&pre_snap, label);
        let mut resumed = LiveExperimentAdapter::from_snapshot(&roundtripped).expect("restores");
        let resumed_snap = resumed.snapshot();
        assert_eq!(resumed_snap.saved.agents.len(), pre_snap.saved.agents.len());
        for (before, after) in pre_snap.saved.agents.iter().zip(&resumed_snap.saved.agents) {
            assert!(
                before.genotype.is_bit_identical(&after.genotype),
                "{label}: restore altered genotype for {}",
                before.lineage_id
            );
            assert_eq!(
                genotype_sha256(&before.genotype),
                genotype_sha256(&after.genotype)
            );
            if mutation_rate == 0.0 {
                assert_eq!(after.genotype.nodes.len(), 2);
                assert_eq!(genotype_sha256(&after.genotype), EXPECTED_2SEG_SHA256);
            }
        }

        assert_eq!(resumed.checksum(), straight.checksum());
        assert_eq!(resumed.observables(), straight.observables());

        for t in (k_save + 1)..=n_total {
            straight.run_schedule_once();
            resumed.run_schedule_once();
            if t % 50 == 0 || t == n_total {
                assert_eq!(
                    resumed.checksum(),
                    straight.checksum(),
                    "{label}: resumed checksum diverged at tick {t}"
                );
                assert_eq!(
                    resumed.observables(),
                    straight.observables(),
                    "{label}: resumed observables diverged at tick {t}"
                );
            }
        }
    };

    run_restore_check(0, 120, 0.0, "tick0_mut0");
    run_restore_check(100, 250, 0.05, "midrun_mut005");
}
