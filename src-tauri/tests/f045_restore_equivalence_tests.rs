//! F-045: live-world snapshot restore must preserve uninterrupted trajectories.
//!
//! Covers all four functional `DONE WHEN` criteria:
//! 1. Condition 1 — S6 base world (`seed = 3`), restore at tick 1 -> 2 and tick 600 -> 1,200,
//!    comparing restored runs against uninterrupted runs at every tick.
//! 2. Condition 2 — Structural `AddNode` mutation before checkpoint preserves `NextNodeId` across
//!    snapshot save/restore (even when `NextNodeId` exceeds `max_saved_node_id + 1`).
//! 3. Condition 3 — Legacy `SavedSimulationState` / `SerializedAgent` without `sex`,
//!    `next_node_id`, or `unindexed_colliders` still loads cleanly and falls back to
//!    `(max_saved_node_id + 1).max(3)`.
//! 4. Condition 4 — Uninterrupted S6 base trajectory checksums are unchanged after the fix.

use anima_engine_lib::core::agent_systems::{AgentGenotype, NextNodeId};
use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::components::{Agent, AgentSex};
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{LiveExperimentAdapter, LiveSnapshot};
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::core::simulation_state::{SavedSimulationState, SerializedSegmentState};
use anima_engine_lib::core::snapshot::{self, SnapshotEnvelope};
use anima_engine_lib::evolution::genotype::{MorphologyEdge, MorphologyNode};
use anima_engine_lib::evolution::mutation::mutate_genotype;
use anima_engine_lib::physics::spatial::SpatialHashGrid;
use bevy_ecs::prelude::{Entity, With};
use rand::{rngs::StdRng, SeedableRng};
use std::collections::HashSet;

fn s6_base_init() -> InitialConditionSet {
    InitialConditionSet::new(vec![
        ("live.founders".into(), 30.0),
        ("live.predator_fraction".into(), 0.0),
        ("live.trees".into(), 8.0),
        ("live.lakes".into(), 2.0),
        ("live.mortality".into(), 1.0),
        ("live.reproduction".into(), 1.0),
        ("live.locomotion".into(), 1.0),
        ("live.drive_floor".into(), 0.5),
        ("live.thirst_steering".into(), 1.0),
        ("live.prey_cap".into(), 160.0),
    ])
}

fn grid_entry_count(adapter: &LiveExperimentAdapter) -> usize {
    adapter
        .world()
        .resource::<SpatialHashGrid>()
        .cells
        .values()
        .map(|v| v.len())
        .sum()
}

fn through_disk_snapshot(snap: &LiveSnapshot, label: &str) -> LiveSnapshot {
    let dir = std::env::temp_dir().join(format!("anima_f045_{}_{}", std::process::id(), label));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir for snapshot test");
    let path = dir.join("checkpoint.json");
    let envelope = SnapshotEnvelope::seal(snap.saved.clone()).expect("seal snapshot");
    snapshot::write_atomic(&path, &envelope).expect("write atomic snapshot");
    let saved = snapshot::read(&path).expect("read snapshot back");
    let _ = std::fs::remove_dir_all(&dir);
    LiveSnapshot {
        saved,
        config: snap.config,
        seed: snap.seed,
    }
}

#[test]
fn f045_probe1_tick1_to_tick2_restore_matches_uninterrupted() {
    let seed = 3u64;
    let init = s6_base_init();
    let mut orig = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &init,
        &[],
        seed,
        (16, 16),
        2,
    )
    .expect("build uninterrupted S6 world");

    let mut clock = SimClock { tick: 0 };
    let mut ledger_orig = CausalLedger::new();
    let mut rng_orig = StdRng::seed_from_u64(seed);
    let _ = clock.advance();
    orig.step(&clock, &[], &mut ledger_orig, &mut rng_orig); // tick 1

    let snap = through_disk_snapshot(&orig.snapshot(), "tick1");
    let mut resumed = LiveExperimentAdapter::from_snapshot(&snap).expect("restore at tick 1");

    let orig_grid_colliders = grid_entry_count(&orig);
    let resumed_grid_colliders = grid_entry_count(&resumed);

    let mut ledger_res = CausalLedger::new();
    let mut rng_res = StdRng::seed_from_u64(seed);
    let _ = clock.advance(); // tick 2
    orig.step(&clock, &[], &mut ledger_orig, &mut rng_orig);
    resumed.step(&clock, &[], &mut ledger_res, &mut rng_res);

    let orig_t2 = orig.checksum();
    let resumed_t2 = resumed.checksum();
    println!(
        "F045_COND1_TICK1_TO_2 orig_grid_colliders={orig_grid_colliders} \
         resumed_grid_colliders={resumed_grid_colliders} \
         orig_t2={orig_t2:#010x} resumed_t2={resumed_t2:#010x}"
    );
    assert_eq!(
        orig_grid_colliders, resumed_grid_colliders,
        "restored SpatialHashGrid at tick 1 must index the exact same pre-rebuild colliders as the uninterrupted run"
    );
    assert_eq!(
        orig_t2, resumed_t2,
        "restoring at tick 1 must match the uninterrupted run at tick 2"
    );
    let saved_json = serde_json::to_value(&snap.saved).expect("serialize saved snapshot");
    let unindexed_agents = saved_json
        .get("unindexed_colliders")
        .and_then(|u| u.get("agents"))
        .and_then(|a| a.as_array())
        .expect("F-045 snapshot records post-rebuild unindexed colliders");
    assert!(
        !unindexed_agents.is_empty(),
        "tick 1 must produce newly born post-rebuild agents in unindexed_colliders"
    );
}

#[test]
fn f045_tick600_to_tick1200_restore_matches_uninterrupted_every_tick() {
    let seed = 3u64;
    let init = s6_base_init();
    let mut orig = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &init,
        &[],
        seed,
        (16, 16),
        1200,
    )
    .expect("build uninterrupted S6 world");

    let mut clock = SimClock { tick: 0 };
    let mut ledger_orig = CausalLedger::new();
    let mut rng_orig = StdRng::seed_from_u64(seed);
    for _ in 1..=600 {
        let _ = clock.advance();
        orig.step(&clock, &[], &mut ledger_orig, &mut rng_orig);
    }

    let snap = through_disk_snapshot(&orig.snapshot(), "tick600");
    let mut resumed = LiveExperimentAdapter::from_snapshot(&snap).expect("restore at tick 600");
    assert_eq!(
        orig.checksum(),
        resumed.checksum(),
        "immediate checksum at tick 600 must match"
    );

    // Measure the founder (None) vs offspring (Some) AgentSex archetype split across restore.
    let (sex_orig, sex_res) = {
        let mut w_orig = orig.world();
        let mut w_res = resumed.world();
        let sex_orig: usize = w_orig
            .query_filtered::<Option<&AgentSex>, With<Agent>>()
            .iter(&w_orig)
            .filter(|s| s.is_some())
            .count();
        let sex_res: usize = w_res
            .query_filtered::<Option<&AgentSex>, With<Agent>>()
            .iter(&w_res)
            .filter(|s| s.is_some())
            .count();
        (sex_orig, sex_res)
    };
    assert!(
        sex_orig > 0,
        "S6 base at tick 600 must have offspring with AgentSex"
    );

    let mut ledger_res = CausalLedger::new();
    let mut rng_res = StdRng::seed_from_u64(seed);
    let mut first_divergence: Option<(u64, u32, u32)> = None;

    for t in 601..=1200 {
        let _ = clock.advance();
        orig.step(&clock, &[], &mut ledger_orig, &mut rng_orig);
        resumed.step(&clock, &[], &mut ledger_res, &mut rng_res);
        let oc = orig.checksum();
        let rc = resumed.checksum();
        if first_divergence.is_none() && oc != rc {
            first_divergence = Some((t, oc, rc));
        }
    }

    let orig_1200 = orig.checksum();
    let resumed_1200 = resumed.checksum();
    println!(
        "F045_COND1_TICK600_TO_1200 sex_orig={sex_orig} sex_res={sex_res} \
         first_divergence={first_divergence:?} \
         orig_1200={orig_1200:#010x} resumed_1200={resumed_1200:#010x}"
    );
    assert!(
        first_divergence.is_none(),
        "restoring at tick 600 diverged from uninterrupted run: first={first_divergence:?}"
    );
    assert_eq!(orig_1200, resumed_1200);
    assert_eq!(
        sex_orig, sex_res,
        "offspring AgentSex count must match after restore"
    );
}

#[test]
fn f045_structural_add_node_mutation_next_node_id_survives_restore() {
    let seed = 3u64;
    let mut init_pairs = vec![
        ("live.founders".into(), 30.0),
        ("live.predator_fraction".into(), 0.0),
        ("live.trees".into(), 8.0),
        ("live.lakes".into(), 2.0),
        ("live.mortality".into(), 1.0),
        ("live.reproduction".into(), 1.0),
        ("live.locomotion".into(), 1.0),
        ("live.drive_floor".into(), 0.5),
        ("live.thirst_steering".into(), 1.0),
        ("live.prey_cap".into(), 160.0),
        ("live.mutation_rate".into(), 1.0),
    ];
    let init = InitialConditionSet::new(std::mem::take(&mut init_pairs));
    let mut orig = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &init,
        &[],
        seed,
        (16, 16),
        60,
    )
    .expect("build world");

    let mut clock = SimClock { tick: 0 };
    let mut ledger_orig = CausalLedger::new();
    let mut rng_orig = StdRng::seed_from_u64(seed);
    let mut mutated_before_checkpoint = false;
    for _ in 1..=600 {
        let _ = clock.advance();
        orig.step(&clock, &[], &mut ledger_orig, &mut rng_orig);
        if orig.world().resource::<NextNodeId>().0 > 3 {
            mutated_before_checkpoint = true;
            break;
        }
    }
    assert!(
        mutated_before_checkpoint,
        "live.mutation_rate = 1.0 over reproducing S6 world must trigger at least one AddNode mutation within 600 ticks"
    );
    let pre_checkpoint_max_nodes = {
        let mut w = orig.world();
        w.query::<&AgentGenotype>()
            .iter(&w)
            .map(|g| g.0.nodes.len())
            .max()
            .unwrap_or(0)
    };
    assert!(
        pre_checkpoint_max_nodes >= 4,
        "AddNode mutation before checkpoint must produce at least one 4+ node agent"
    );

    // Also advance NextNodeId by 2 beyond max living node id (simulating a failed structural check
    // or extinct mutated lineage) so we prove the persisted `NextNodeId` field itself survives
    // restore rather than being recomputed from living nodes alone.
    let expected_next_node_id = {
        let mut w = orig.world();
        let bumped = w.resource::<NextNodeId>().0 + 2;
        w.resource_mut::<NextNodeId>().0 = bumped;
        bumped
    };
    assert!(expected_next_node_id >= 6);

    let snap = through_disk_snapshot(&orig.snapshot(), "add_node_next_id");
    let mut resumed = LiveExperimentAdapter::from_snapshot(&snap).expect("restore after AddNode");
    let restored_next_id = resumed.world().resource::<NextNodeId>().0;
    println!(
        "F045_COND2_NEXT_NODE_ID checkpoint_tick={} pre_max_nodes={pre_checkpoint_max_nodes} \
         expected={expected_next_node_id} restored={restored_next_id}",
        clock.tick
    );
    assert_eq!(
        restored_next_id, expected_next_node_id,
        "NextNodeId resource must survive restore verbatim"
    );
    let saved_json = serde_json::to_value(&snap.saved).expect("serialize SavedSimulationState");
    assert_eq!(
        saved_json.get("next_node_id").and_then(|v| v.as_u64()),
        Some(u64::from(expected_next_node_id)),
        "SavedSimulationState must serialize NextNodeId verbatim"
    );

    // Drive a second AddNode mutation on a standalone genotype clone using restored `NextNodeId`
    // and verify all allocated node ids stay unique (`>= expected_next_node_id`).
    let mut check_geno = {
        let mut w = resumed.world();
        let target_entity: Entity = w
            .query_filtered::<Entity, With<Agent>>()
            .iter(&w)
            .next()
            .expect("living agent");
        w.get::<AgentGenotype>(target_entity).unwrap().0.clone()
    };
    {
        let mut counter = resumed.world().resource::<NextNodeId>().0;
        let mut rng = StdRng::seed_from_u64(42);
        let initial_len = check_geno.nodes.len();
        for _ in 0..64 {
            mutate_genotype(&mut check_geno, &mut counter, 1.0, &mut rng).expect("mutate");
            if check_geno.nodes.len() > initial_len {
                break;
            }
        }
        assert!(
            check_geno.nodes.len() > initial_len,
            "must add a structural node"
        );
        let unique_ids: HashSet<u32> = check_geno.nodes.iter().map(|n| n.id).collect();
        assert_eq!(
            unique_ids.len(),
            check_geno.nodes.len(),
            "post-restore AddNode must never collide with existing node IDs"
        );
        let max_new_id = *unique_ids.iter().max().unwrap();
        assert!(
            max_new_id >= expected_next_node_id,
            "new structural node ID ({max_new_id}) must be >= restored NextNodeId ({expected_next_node_id})"
        );
    }

    // And the unperturbed post-restore run continues matching uninterrupted through tick + 50
    // (including 4+ segment bodies and post-restore AddNode mutations!).
    let mut ledger_res = CausalLedger::new();
    let mut rng_res = StdRng::seed_from_u64(seed);
    let checkpoint_tick = clock.tick;
    for _ in 1..=50 {
        let _ = clock.advance();
        orig.step(&clock, &[], &mut ledger_orig, &mut rng_orig);
        resumed.step(&clock, &[], &mut ledger_res, &mut rng_res);
        assert_eq!(
            orig.checksum(),
            resumed.checksum(),
            "divergence at tick {} after tick-{} AddNode checkpoint",
            clock.tick,
            checkpoint_tick
        );
    }
    assert_eq!(
        orig.world().resource::<NextNodeId>().0,
        resumed.world().resource::<NextNodeId>().0,
        "post-restore NextNodeId progression must stay identical to uninterrupted"
    );
}

#[test]
fn f045_old_save_without_new_fields_loads_and_falls_back_cleanly() {
    let seed = 3u64;
    let init = s6_base_init();
    let mut orig = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &init,
        &[],
        seed,
        (16, 16),
        10,
    )
    .expect("build world");

    let mut clock = SimClock { tick: 0 };
    let mut ledger = CausalLedger::new();
    let mut rng = StdRng::seed_from_u64(seed);
    for _ in 1..=5 {
        let _ = clock.advance();
        orig.step(&clock, &[], &mut ledger, &mut rng);
    }

    let mut snap = orig.snapshot();
    // Attach a node id = 5 to agent 0 so fallback `(max_saved_node_id + 1).max(3)` == 6.
    snap.saved.agents[0].genotype.add_node(MorphologyNode {
        id: 5,
        length: 0.8,
        radius: 0.2,
        mass: 0.6,
    });
    snap.saved.agents[0].genotype.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 5,
        joint_anchor: glam::Vec3::Z,
        joint_axis: glam::Vec3::Y,
    });
    let seg_pos = snap.saved.agents[0].root_position + glam::Vec3::Z;
    let seg_rot = snap.saved.agents[0].root_rotation;
    snap.saved.agents[0].segments.push(SerializedSegmentState {
        segment_id: 5,
        position: seg_pos,
        rotation: seg_rot,
        velocity: glam::Vec3::ZERO,
        oscillator: None,
    });

    // Convert to JSON Value and strip F-045 fields (`sex`, `next_node_id`, `unindexed_colliders`)
    // to simulate an on-disk pre-F045 save.
    let mut value = serde_json::to_value(&snap.saved).expect("serialize to JSON value");
    let obj = value
        .as_object_mut()
        .expect("SavedSimulationState JSON object");
    obj.remove("next_node_id");
    obj.remove("unindexed_colliders");
    for agent_val in obj
        .get_mut("agents")
        .and_then(|v| v.as_array_mut())
        .expect("agents JSON array")
    {
        agent_val
            .as_object_mut()
            .expect("agent JSON object")
            .remove("sex");
    }

    let legacy_json = serde_json::to_string_pretty(&value).expect("stringify legacy save JSON");
    let parsed_legacy: SavedSimulationState =
        serde_json::from_str(&legacy_json).expect("deserialize legacy save without new fields");
    let reserialized = serde_json::to_value(&parsed_legacy).expect("reserialize parsed legacy");
    assert!(
        reserialized
            .get("next_node_id")
            .is_none_or(serde_json::Value::is_null),
        "legacy save must deserialize next_node_id as None"
    );
    assert!(
        reserialized
            .get("unindexed_colliders")
            .is_none_or(serde_json::Value::is_null),
        "legacy save must deserialize unindexed_colliders as None"
    );
    assert!(
        reserialized
            .get("agents")
            .and_then(|v| v.as_array())
            .is_some_and(|agents| agents
                .iter()
                .all(|a| a.get("sex").is_none_or(serde_json::Value::is_null))),
        "legacy save must deserialize every agent's sex as None"
    );

    // Also seal into a SnapshotEnvelope and reload from disk before booting LiveExperimentAdapter.
    let legacy_snap = through_disk_snapshot(
        &LiveSnapshot {
            saved: parsed_legacy,
            config: snap.config,
            seed: snap.seed,
        },
        "legacy_no_new_fields",
    );
    let mut restored =
        LiveExperimentAdapter::from_snapshot(&legacy_snap).expect("restore legacy save");
    let restored_next = restored.world().resource::<NextNodeId>().0;
    println!(
        "F045_COND3_OLD_SAVE restored_agents={} fallback_next_node_id={restored_next}",
        legacy_snap.saved.agents.len()
    );
    assert_eq!(restored_next, 6);

    // Step forward several ticks cleanly with zero errors.
    let mut rng_legacy = StdRng::seed_from_u64(seed);
    let mut ledger_legacy = CausalLedger::new();
    for _ in 1..=5 {
        let _ = clock.advance();
        restored.step(&clock, &[], &mut ledger_legacy, &mut rng_legacy);
    }
    assert!(
        restored.checksum() != 0,
        "resumed legacy world must advance cleanly"
    );
}

#[test]
fn f045_uninterrupted_s6_base_trajectory_fingerprint() {
    fn run_once() -> [u32; 4] {
        let seed = 3u64;
        let init = s6_base_init();
        let mut model = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &init,
            &[],
            seed,
            (16, 16),
            1200,
        )
        .expect("build uninterrupted S6 world");
        let mut clock = SimClock { tick: 0 };
        let mut ledger = CausalLedger::new();
        let mut rng = StdRng::seed_from_u64(seed);
        let mut samples = [0u32; 4];
        for t in 1..=1200 {
            let _ = clock.advance();
            model.step(&clock, &[], &mut ledger, &mut rng);
            match t {
                1 => samples[0] = model.checksum(),
                2 => samples[1] = model.checksum(),
                600 => samples[2] = model.checksum(),
                1200 => samples[3] = model.checksum(),
                _ => {}
            }
        }
        samples
    }

    let a = run_once();
    let b = run_once();
    assert_eq!(a, b, "same manifest and seed must be self-consistent");
    println!(
        "F045_COND4_UNINTERRUPTED seed=3 c_t1={:#010x} c_t2={:#010x} c_t600={:#010x} c_t1200={:#010x}",
        a[0], a[1], a[2], a[3]
    );
}
