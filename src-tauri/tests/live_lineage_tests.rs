//! Live-world births in the lineage tracker (`live.lineage_tracking`, FINDINGS F-066).
//!
//! Before this key existed, a birth in the live Bevy world never reached a `LineageTracker`: the
//! only caller of `add_reproduction` was the MAP-Elites epoch on the evolution thread, and
//! `SpawnOffspringCommand` carried the parent ids without writing an edge. Worse, the child id was
//! `format!("{parent}.{generation + 1}")`, so every child of one parent wore the same id. Together
//! those made every line-of-descent measurement on the live population (OSS-073, the S43 Red-Queen
//! gate, AE-S11 species candidates) impossible: there was no graph, and the ids could not have
//! named its nodes anyway.
//!
//! What these tests pin:
//!
//! 1. **Opt-in births reach the graph.** With `live.lineage_tracking = 1` every live birth is one
//!    node with exactly one edge from the parent its `AgentParentLineageIds` names, and every living
//!    agent is a node.
//! 2. **Ids are unique**, siblings included — the defect F-047 worked around in inference pairing.
//! 3. **Deaths carry a tick**, so an ALife-standard `destruction_time` can be exported: no living
//!    agent has one, and every node that has one is no longer alive.
//! 4. **Save/restore round-trips the graph and the id cursor**: `run N` equals
//!    `run K → snapshot → restore → run N − K` in both checksum and lineage.
//! 5. **The legacy path is untouched.** With the key absent nothing is recorded and children keep
//!    the historical `parent.generation` id — the trajectory every calibrated manifest depends on.
//! 6. **Recording does not feed back** (ADR-0008 driver 4): the same seed and manifest with and
//!    without the key are the same world at every checkpoint. The unique id therefore lives in
//!    `LiveLineage` and `SerializedAgent::live_id`; `AgentLineageId` keeps `parent.generation` in
//!    both runs, because `autonomous_reproduction_system` sorts parents by it.
//! 7. **Compaction survives a restore**, on both sides of the restore point, and never removes a
//!    living agent's node.
//! 8. **`Clone` means an exact copy**: the edge label follows what the birth did to the genome.

use anima_engine_lib::core::experiment::{ExperimentError, InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::LiveSnapshot;
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LiveWorldConfig, LIVE_KEYS, LIVE_KEY_DRIVE_FLOOR, LIVE_KEY_FOOD_CAP,
    LIVE_KEY_FOUNDERS, LIVE_KEY_LAKES, LIVE_KEY_LAKE_LAYOUT, LIVE_KEY_LINEAGE_TRACKING,
    LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY, LIVE_KEY_MUTATION_RATE, LIVE_KEY_PREDATOR_FRACTION,
    LIVE_KEY_PREY_CAP, LIVE_KEY_REPRODUCTION, LIVE_KEY_THIRST_STEERING, LIVE_KEY_TREES,
};
use anima_engine_lib::core::live_lineage::{
    LiveLineage, FIRST_COMPACTION_AT, LIVE_BIRTH_ID_PREFIX,
};
use anima_engine_lib::core::resources::VitalStatistics;
use anima_engine_lib::core::simulation_state::{SavedSimulationState, SerializedAgent};
use anima_engine_lib::core::snapshot::{self, SnapshotEnvelope};
use anima_engine_lib::evolution::genotype::MorphologyGenotype;
use anima_engine_lib::evolution::lineage::{LineageNode, LineageRelation, RelationType};
use std::collections::{BTreeMap, BTreeSet};

fn initial(pairs: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect())
}

/// A small prey-only world that reproduces on tick 1 and starts dying about 1,700 ticks in.
fn breeding_world_with(lineage: Option<f64>, extra: &[(&str, f64)]) -> InitialConditionSet {
    let mut pairs = vec![
        (LIVE_KEY_FOUNDERS, 12.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.0),
        (LIVE_KEY_PREY_CAP, 40.0),
        (LIVE_KEY_TREES, 4.0),
        (LIVE_KEY_LAKES, 4.0),
        (LIVE_KEY_LAKE_LAYOUT, 1.0),
        (LIVE_KEY_FOOD_CAP, 30.0),
        (LIVE_KEY_MORTALITY, 1.0),
        (LIVE_KEY_REPRODUCTION, 1.0),
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_DRIVE_FLOOR, 0.5),
        (LIVE_KEY_THIRST_STEERING, 1.0),
    ];
    if let Some(v) = lineage {
        pairs.push((LIVE_KEY_LINEAGE_TRACKING, v));
    }
    pairs.extend_from_slice(extra);
    initial(&pairs)
}

const SEED: u64 = 0x1a9e_a9e5;

fn build(lineage: Option<f64>) -> LiveExperimentAdapter {
    build_with(lineage, &[])
}

fn build_with(lineage: Option<f64>, extra: &[(&str, f64)]) -> LiveExperimentAdapter {
    LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &breeding_world_with(lineage, extra),
        &[],
        SEED,
        (16, 16),
        1_000,
    )
    .expect("breeding world builds")
}

/// A living agent's unique id. Every agent of a recording run has one.
fn live_id(agent: &SerializedAgent) -> &str {
    agent
        .live_id
        .as_deref()
        .unwrap_or_else(|| panic!("agent {} has no live id", agent.lineage_id))
}

fn run(adapter: &mut LiveExperimentAdapter, ticks: u64) {
    for _ in 0..ticks {
        adapter.run_schedule_once();
    }
}

fn total_births(adapter: &LiveExperimentAdapter) -> u64 {
    adapter.world().resource::<VitalStatistics>().total_births
}

fn saved(adapter: &LiveExperimentAdapter) -> SavedSimulationState {
    adapter.snapshot().saved
}

fn incoming(relations: &[LineageRelation]) -> BTreeMap<&str, Vec<&str>> {
    let mut map: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for r in relations {
        map.entry(r.target_id.as_str())
            .or_default()
            .push(r.source_id.as_str());
    }
    map
}

fn node_map(nodes: &[LineageNode]) -> BTreeMap<&str, &LineageNode> {
    nodes.iter().map(|n| (n.id.as_str(), n)).collect()
}

/// A graph flattened to comparable plain data: genotypes are left out on purpose (they are not
/// `PartialEq`, and the checksum already covers the bodies of the living).
type FlatGraph = (
    Vec<(String, u32, Option<u32>, Option<u64>, Option<u64>)>,
    Vec<(String, String, String, Option<u32>)>,
);

fn flatten(state: &SavedSimulationState) -> FlatGraph {
    let mut nodes: Vec<_> = state
        .lineage_nodes
        .iter()
        .map(|n| {
            (
                n.id.clone(),
                n.generation,
                n.cumulative_mutations,
                n.origin_tick,
                n.destruction_tick,
            )
        })
        .collect();
    nodes.sort();
    let mut rels: Vec<_> = state
        .lineage_relations
        .iter()
        .map(|r| {
            (
                r.source_id.clone(),
                r.target_id.clone(),
                format!("{:?}", r.relation_type),
                r.path_events,
            )
        })
        .collect();
    rels.sort();
    (nodes, rels)
}

// ---- key registration ---------------------------------------------------------------------------

#[test]
fn the_key_is_registered_exact_boolean_and_off_by_default() {
    assert_eq!(LIVE_KEY_LINEAGE_TRACKING, "live.lineage_tracking");
    assert!(LIVE_KEYS.contains(&LIVE_KEY_LINEAGE_TRACKING));
    assert!(!LiveWorldConfig::default().lineage_tracking);
    let on =
        LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_LINEAGE_TRACKING, 1.0)]))
            .expect("1.0 is on");
    assert!(on.lineage_tracking);
    let off =
        LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_LINEAGE_TRACKING, 0.0)]))
            .expect("0.0 is off");
    assert!(!off.lineage_tracking);
    for bad in [0.5, 2.0, -1.0, f64::NAN] {
        let err =
            LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_LINEAGE_TRACKING, bad)]))
                .expect_err("a non-boolean lineage_tracking must be refused");
        assert!(
            matches!(err, ExperimentError::OutOfRange { .. }) || bad.is_nan(),
            "{bad}: {err:?}"
        );
    }
    // A config serialized before the field existed reads back as off.
    let mut json = serde_json::to_value(LiveWorldConfig::default()).unwrap();
    json.as_object_mut().unwrap().remove("lineage_tracking");
    let back: LiveWorldConfig = serde_json::from_value(json).unwrap();
    assert!(!back.lineage_tracking);
}

// ---- 1 + 2: births reach the graph, ids are unique ------------------------------------------------

#[test]
fn live_births_reach_the_tracker_as_edges_from_the_recorded_parent() {
    let mut adapter = build(Some(1.0));
    run(&mut adapter, 150);
    let births = total_births(&adapter);
    assert!(
        births >= 12,
        "non-vacuity: the world must breed (births={births})"
    );

    let state = saved(&adapter);
    let nodes = node_map(&state.lineage_nodes);
    let parents_of = incoming(&state.lineage_relations);

    // No compaction can have run yet at this size, so the graph is the whole history: twelve roots
    // plus one node and one edge per birth.
    assert_eq!(state.lineage_nodes.len() as u64, 12 + births);
    assert_eq!(state.lineage_relations.len() as u64, births);

    let mut minted = 0u64;
    for node in &state.lineage_nodes {
        if node.id.starts_with("live-founder-") {
            assert_eq!(node.generation, 0);
            assert_eq!(node.origin_tick, Some(0), "a founder is born at genesis");
            assert!(!parents_of.contains_key(node.id.as_str()));
        } else {
            minted += 1;
            assert!(
                node.id.starts_with(&format!("{LIVE_BIRTH_ID_PREFIX}-")),
                "a live birth carries a minted id, not `parent.generation`: {}",
                node.id
            );
            let parents = &parents_of[node.id.as_str()];
            assert_eq!(parents.len(), 1, "asexual birth: one parent edge");
            let parent = nodes[parents[0]];
            assert_eq!(node.generation, parent.generation + 1);
            let origin = node.origin_tick.expect("a live birth records its tick");
            assert!((1..=150).contains(&origin));
        }
    }
    assert_eq!(minted, births);
    assert_eq!(adapter.world().resource::<LiveLineage>().faults(), 0);

    // Every living agent is a node, and the edge into it comes from its own parent. The edge names
    // the parent's unique id; the agent still carries the legacy `parent.generation` strings
    // (`AgentLineageId`, `AgentParentLineageIds`), which recording never rewrites. So the check
    // goes through the parent: the agent whose live id the edge names must be the one whose legacy
    // id this child's `parent_ids` holds. No one has died by tick 150 (the first deaths come at
    // about tick 1,700) and nothing has been compacted, so every parent is alive and every edge is
    // still the direct one. (After a compaction an edge may skip a spliced, dead parent and name an
    // older ancestor; `compaction_on_both_sides_of_a_restore_point_keeps_the_living` checks what
    // holds then.)
    let legacy_of: BTreeMap<&str, &str> = state
        .agents
        .iter()
        .map(|a| (live_id(a), a.lineage_id.as_str()))
        .collect();
    for agent in &state.agents {
        let id = live_id(agent);
        assert!(
            nodes.contains_key(id),
            "living agent {id} has no lineage node"
        );
        let Some(parent_legacy) = agent.parent_ids.first() else {
            assert!(!parents_of.contains_key(id), "a founder has no parent edge");
            continue;
        };
        let parents = &parents_of[id];
        assert_eq!(parents.len(), 1);
        let parent_live = parents[0];
        let parent_legacy_seen = legacy_of
            .get(parent_live)
            .copied()
            .unwrap_or_else(|| panic!("parent {parent_live} of {id} is not alive at tick 150"));
        assert_eq!(parent_legacy_seen, parent_legacy.as_str());
        assert_eq!(
            agent.lineage_id,
            format!("{parent_legacy}.{}", agent.generation),
            "AgentLineageId keeps its legacy value in a recording run"
        );
    }
}

#[test]
fn siblings_get_distinct_ids() {
    let mut adapter = build(Some(1.0));
    run(&mut adapter, 400);
    let state = saved(&adapter);

    let ids: Vec<&str> = state.lineage_nodes.iter().map(|n| n.id.as_str()).collect();
    let unique: BTreeSet<&str> = ids.iter().copied().collect();
    assert_eq!(ids.len(), unique.len(), "lineage node ids must be unique");

    let living: Vec<&str> = state.agents.iter().map(live_id).collect();
    let living_unique: BTreeSet<&str> = living.iter().copied().collect();
    assert_eq!(
        living.len(),
        living_unique.len(),
        "living ids must be unique"
    );

    // Non-vacuity: at least one parent had two or more children, which is exactly the case that
    // used to collide.
    let mut children: BTreeMap<&str, usize> = BTreeMap::new();
    for r in &state.lineage_relations {
        *children.entry(r.source_id.as_str()).or_default() += 1;
    }
    assert!(
        children.values().any(|&n| n >= 2),
        "the run must contain siblings for this test to mean anything"
    );
}

#[test]
fn the_legacy_path_records_nothing_and_keeps_parent_generation_ids() {
    let mut adapter = build(None);
    assert!(adapter.world().get_resource::<LiveLineage>().is_none());
    run(&mut adapter, 400);
    let state = saved(&adapter);
    assert!(state.lineage_nodes.is_empty());
    assert!(state.lineage_relations.is_empty());
    assert!(state.live_lineage.is_none());
    assert!(state.agents.iter().all(|a| a.live_id.is_none()));
    // Nothing new reaches the bytes of a save without the key.
    let json = serde_json::to_string(&state).unwrap();
    assert!(!json.contains("\"live_id\""));
    assert!(!json.contains("\"origin_tick\""));

    // The historical id scheme, siblings colliding: this is the trajectory every calibrated
    // manifest was measured on, and it must stay reachable.
    let mut seen = BTreeSet::new();
    let mut collision = false;
    for agent in &state.agents {
        if let Some(parent) = agent.parent_ids.first() {
            assert_eq!(
                agent.lineage_id,
                format!("{parent}.{}", agent.generation),
                "legacy child ids are `parent.generation`"
            );
        }
        collision |= !seen.insert(agent.lineage_id.clone());
    }
    assert!(
        collision,
        "positive control: the legacy scheme must still show the sibling collision"
    );
}

// ---- 3: death time ----------------------------------------------------------------------------------

#[test]
fn deaths_are_recorded_with_their_tick_and_never_on_the_living() {
    // Measured on this seed: the first deaths (dehydration) come at about tick 1,700.
    let mut adapter = build(Some(1.0));
    run(&mut adapter, 2_200);
    let state = saved(&adapter);
    let tick = adapter.tick();
    let total_deaths = adapter.world().resource::<VitalStatistics>().total_deaths;
    assert_eq!(adapter.world().resource::<LiveLineage>().faults(), 0);

    let living: BTreeSet<&str> = state.agents.iter().map(live_id).collect();
    let mut dead = 0usize;
    for node in &state.lineage_nodes {
        if let Some(t) = node.destruction_tick {
            dead += 1;
            assert!(!living.contains(node.id.as_str()), "{} is alive", node.id);
            assert!(t <= tick);
            if let Some(origin) = node.origin_tick {
                assert!(origin <= t, "{} died before it was born", node.id);
            }
        }
    }
    for id in &living {
        let node = state
            .lineage_nodes
            .iter()
            .find(|n| n.id == *id)
            .expect("every living agent is a node");
        assert_eq!(node.destruction_tick, None);
    }
    assert!(dead > 0, "non-vacuity: the run must contain deaths");
    // Below 4,096 nodes nothing has been compacted, so every death is still in the graph.
    assert!(state.lineage_nodes.len() < 4_096);
    assert_eq!(
        dead as u64, total_deaths,
        "every death is recorded exactly once"
    );
}

// ---- 4: save/restore --------------------------------------------------------------------------------

#[test]
fn run_n_equals_run_k_snapshot_restore_run_n_minus_k_with_lineage() {
    // K falls after the first deaths (about tick 1,700 on this seed) and N after more of them, so
    // both halves of the run record births and deaths.
    const N: u64 = 2_100;
    const K: u64 = 1_850;

    let mut straight = build(Some(1.0));
    run(&mut straight, N);

    let mut first = build(Some(1.0));
    run(&mut first, K);
    let snap = first.snapshot();
    assert!(snap.saved.live_lineage.is_some(), "the id cursor is saved");
    // Through the bytes, so the serde shape is exercised and not only the in-memory clone.
    let bytes = serde_json::to_vec(&snap.saved).unwrap();
    let mut snap2 = snap.clone();
    snap2.saved = serde_json::from_slice(&bytes).unwrap();
    let mut resumed = LiveExperimentAdapter::from_snapshot(&snap2).expect("restores");
    run(&mut resumed, N - K);

    assert_eq!(straight.checksum(), resumed.checksum());
    let straight_state = saved(&straight);
    assert!(
        straight_state
            .lineage_nodes
            .iter()
            .any(|n| n.destruction_tick.is_some_and(|t| t > K)),
        "non-vacuity: a death after the restore point"
    );
    assert_eq!(flatten(&saved(&straight)), flatten(&saved(&resumed)));
    let living_ids = |state: &SavedSimulationState| {
        let mut ids: Vec<String> = state.agents.iter().map(|a| live_id(a).to_owned()).collect();
        ids.sort();
        ids
    };
    assert_eq!(
        living_ids(&saved(&straight)),
        living_ids(&saved(&resumed)),
        "every living agent carries the same id through the restore, never a re-minted one"
    );
    assert_eq!(
        saved(&straight).live_lineage,
        saved(&resumed).live_lineage,
        "the id cursor continues, it does not restart"
    );
}

#[test]
fn a_save_without_the_new_fields_still_loads() {
    let mut adapter = build(Some(1.0));
    run(&mut adapter, 60);
    let snap = adapter.snapshot();

    // Strip every field this change added, as a save written before it would lack them.
    let mut json = serde_json::to_value(&snap.saved).unwrap();
    let obj = json.as_object_mut().unwrap();
    obj.remove("live_lineage");
    for node in obj["lineage_nodes"].as_array_mut().unwrap() {
        let n = node.as_object_mut().unwrap();
        n.remove("origin_tick");
        n.remove("destruction_tick");
    }
    for agent in obj["agents"].as_array_mut().unwrap() {
        agent.as_object_mut().unwrap().remove("live_id");
    }
    let old: SavedSimulationState = serde_json::from_value(json).expect("old shape deserializes");
    assert!(old.live_lineage.is_none());
    assert!(old.lineage_nodes.iter().all(|n| n.origin_tick.is_none()));
    assert!(old.agents.iter().all(|a| a.live_id.is_none()));

    // The run still resumes; the cursor is recovered from the ids already in the save, so the next
    // minted id cannot collide with one that exists. A founder keeps its node; a child, whose
    // legacy id names no node, is entered as a new root under a fresh id.
    let mut snap_old = snap.clone();
    snap_old.saved = old;
    let mut resumed = LiveExperimentAdapter::from_snapshot(&snap_old).expect("old save restores");
    run(&mut resumed, 60);
    assert_eq!(
        resumed.world().resource::<LiveLineage>().faults(),
        0,
        "every agent of the resumed run has a node, so no birth or death is refused"
    );
    let state = saved(&resumed);
    let ids: Vec<&str> = state.lineage_nodes.iter().map(|n| n.id.as_str()).collect();
    let unique: BTreeSet<&str> = ids.iter().copied().collect();
    assert_eq!(ids.len(), unique.len());
    let living: Vec<&str> = state.agents.iter().map(live_id).collect();
    let living_unique: BTreeSet<&str> = living.iter().copied().collect();
    assert_eq!(living.len(), living_unique.len());
    assert!(living.iter().all(|id| unique.contains(id)));
    assert_eq!(state.live_lineage.map(|l| l.faults), Some(0));
}

// ---- 6: recording does not feed back (ADR-0008 driver 4) ---------------------------------------------

/// ADR-0008 driver 4 ("Không phản hồi"): recording the past must not change the future. The same
/// seed and manifest, run once without `live.lineage_tracking` and once with it, must be the same
/// world at every checkpoint — same `world_checksum`, same births, same deaths.
///
/// Run against run, never against a pinned number: checksums are platform-dependent.
#[test]
fn recording_lineage_does_not_change_the_trajectory() {
    let vitals = |adapter: &LiveExperimentAdapter| {
        let w = adapter.world();
        let v = w.resource::<VitalStatistics>();
        (v.total_births, v.total_deaths)
    };
    let mut off = build(None);
    let mut on = build(Some(1.0));
    // Recording with compaction running too (floor 30 compacts twice by tick 2,500 on this seed):
    // pruning the graph must not feed back either.
    let mut compacting = build(Some(1.0));
    compacting
        .world()
        .resource_mut::<LiveLineage>()
        .set_compaction_floor(30);
    let mut at = 0u64;
    for checkpoint in [0u64, 180, 1_800, 2_500] {
        for adapter in [&mut off, &mut on, &mut compacting] {
            run(adapter, checkpoint - at);
        }
        at = checkpoint;
        for (label, recording) in [("on", &on), ("compacting", &compacting)] {
            assert_eq!(
                vitals(&off),
                vitals(recording),
                "tick {checkpoint}: (births, deaths) without recording vs {label}"
            );
            assert_eq!(
                off.checksum(),
                recording.checksum(),
                "tick {checkpoint}: recording lineage ({label}) changed the world"
            );
        }
    }
    let (births, deaths) = vitals(&on);
    assert!(
        births > 0 && deaths > 0,
        "non-vacuity: the run must breed and die (births={births}, deaths={deaths})"
    );
    assert!(compacting.world().resource::<LiveLineage>().compactions() >= 2);
    for recording in [&on, &compacting] {
        assert_eq!(recording.world().resource::<LiveLineage>().faults(), 0);
    }
}

// ---- 7: compaction across a restore ----------------------------------------------------------------

/// Round-trip a snapshot through the real on-disk format (envelope, checksum, atomic write, read).
fn through_disk(snapshot: &LiveSnapshot, label: &str) -> LiveSnapshot {
    let dir =
        std::env::temp_dir().join(format!("anima_live_lineage_{}_{label}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("checkpoint.json");
    let envelope = SnapshotEnvelope::seal(snapshot.saved.clone()).expect("seal");
    snapshot::write_atomic(&path, &envelope).expect("write");
    let back = snapshot::read(&path).expect("read");
    let _ = std::fs::remove_dir_all(&dir);
    LiveSnapshot {
        saved: back,
        config: snapshot.config,
        seed: snapshot.seed,
    }
}

#[test]
fn compaction_on_both_sides_of_a_restore_point_keeps_the_living() {
    // A floor of 30 instead of the default 4,096, so compaction runs inside a short run. Measured on
    // this seed: the first compaction comes at about tick 430 (30 nodes, nobody dead yet), the
    // second at about tick 2,265 (62 nodes, about 20 dead), which prunes. K sits between them, so
    // one compaction runs before the restore point and one after it, in the resumed half.
    const FLOOR: u64 = 30;
    const K: u64 = 1_000;
    const N: u64 = 2_400;
    const { assert!(FLOOR < FIRST_COMPACTION_AT) };
    let with_floor = || {
        let adapter = build(Some(1.0));
        adapter
            .world()
            .resource_mut::<LiveLineage>()
            .set_compaction_floor(FLOOR);
        adapter
    };

    let mut straight = with_floor();
    run(&mut straight, N);

    let mut first = with_floor();
    run(&mut first, K);
    assert!(
        first.world().resource::<LiveLineage>().compactions() >= 1,
        "non-vacuity: a compaction before the restore point"
    );
    let snap = first.snapshot();
    let saved_cursor = snap.saved.live_lineage.expect("the cursor is saved");
    assert_eq!(saved_cursor.compaction_floor, FLOOR, "the floor travels");
    let mut resumed =
        LiveExperimentAdapter::from_snapshot(&through_disk(&snap, "compaction")).expect("restores");
    run(&mut resumed, N - K);
    assert!(
        resumed.world().resource::<LiveLineage>().compactions() >= 1,
        "non-vacuity: a compaction after the restore point, in the resumed half"
    );

    assert_eq!(straight.checksum(), resumed.checksum());
    {
        let (s, r) = (straight.world(), resumed.world());
        let (s, r) = (s.resource::<LiveLineage>(), r.resource::<LiveLineage>());
        assert_eq!(s.node_count(), r.node_count());
        assert_eq!(s.next_compaction_at(), r.next_compaction_at());
        assert_eq!((s.faults(), r.faults()), (0, 0));
    }
    let (straight_state, resumed_state) = (saved(&straight), saved(&resumed));
    assert_eq!(flatten(&straight_state), flatten(&resumed_state));
    assert_eq!(straight_state.live_lineage, resumed_state.live_lineage);

    // Compaction did remove something: dead leaves are gone.
    let births = total_births(&straight);
    assert!(
        (straight_state.lineage_nodes.len() as u64) < 12 + births,
        "non-vacuity: compaction pruned dead nodes ({} nodes, {} births)",
        straight_state.lineage_nodes.len(),
        births
    );

    for state in [&straight_state, &resumed_state] {
        let nodes = node_map(&state.lineage_nodes);
        let parents_of = incoming(&state.lineage_relations);
        // Every living agent's node survived every compaction, still open.
        for agent in &state.agents {
            let id = live_id(agent);
            let node = nodes
                .get(id)
                .unwrap_or_else(|| panic!("compaction removed living agent {id}"));
            assert!(node.is_open(), "living agent {id} is not an open node");
        }
        // What does hold after a splice. A dead parent with a single surviving line of descent is
        // spliced out, so the edge into a living child may name an older ancestor than the one its
        // (legacy) `AgentParentLineageIds` names, and `path_events` counts the edges it stands
        // for. The graph stays well formed: every edge's ends are nodes, each non-founder has
        // exactly one parent edge, and an edge always runs to a later generation.
        for r in &state.lineage_relations {
            let (source, target) = (nodes[r.source_id.as_str()], nodes[r.target_id.as_str()]);
            assert!(source.generation < target.generation);
            if let Some(path) = r.path_events {
                assert!(path >= 1);
            }
        }
        for node in &state.lineage_nodes {
            let n = parents_of.get(node.id.as_str()).map_or(0, Vec::len);
            if node.id.starts_with("live-founder-") {
                assert_eq!(n, 0);
            } else {
                assert_eq!(n, 1, "{} has {n} parent edges", node.id);
            }
        }
    }
}

// ---- 8: Clone means an exact copy -------------------------------------------------------------------

fn genotype_of<'a>(nodes: &BTreeMap<&str, &'a LineageNode>, id: &str) -> &'a MorphologyGenotype {
    nodes[id]
        .genotype
        .as_ref()
        .unwrap_or_else(|| panic!("node {id} keeps no genotype"))
}

#[test]
fn with_mutation_rate_zero_every_birth_is_an_exact_clone() {
    let mut adapter = build_with(Some(1.0), &[(LIVE_KEY_MUTATION_RATE, 0.0)]);
    run(&mut adapter, 400);
    let state = saved(&adapter);
    assert!(
        !state.lineage_relations.is_empty(),
        "non-vacuity: the run must breed"
    );
    let nodes = node_map(&state.lineage_nodes);
    for r in &state.lineage_relations {
        assert_eq!(
            r.relation_type,
            RelationType::Clone,
            "{} -> {}: nothing can change at rate zero",
            r.source_id,
            r.target_id
        );
        assert!(
            genotype_of(&nodes, &r.target_id).is_bit_identical(genotype_of(&nodes, &r.source_id)),
            "a Clone child's genotype equals its parent's ({} -> {})",
            r.source_id,
            r.target_id
        );
    }
}

#[test]
fn the_edge_label_says_whether_the_genome_actually_changed() {
    // No evolved brains here, so the body is the whole genome and the label must be exactly
    // "the child's body differs from its parent's". A high rate gives both labels in a short run.
    let mut adapter = build_with(Some(1.0), &[(LIVE_KEY_MUTATION_RATE, 0.5)]);
    run(&mut adapter, 400);
    let state = saved(&adapter);
    let nodes = node_map(&state.lineage_nodes);
    let (mut clones, mut mutants) = (0, 0);
    for r in &state.lineage_relations {
        let changed =
            !genotype_of(&nodes, &r.target_id).is_bit_identical(genotype_of(&nodes, &r.source_id));
        match r.relation_type {
            RelationType::Mutate => {
                mutants += 1;
                assert!(changed, "{} is labelled Mutate but is a copy", r.target_id);
            }
            RelationType::Clone => {
                clones += 1;
                assert!(!changed, "{} is labelled Clone but differs", r.target_id);
            }
            other => panic!("unexpected live birth relation {other:?}"),
        }
    }
    assert!(
        clones > 0 && mutants > 0,
        "non-vacuity: both labels must occur (clones={clones}, mutants={mutants})"
    );
}
