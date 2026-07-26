//! OSS-071 on the live path — `LineageTracker::compact`.
//!
//! `simplify` was a pure function returning a value; nothing shrank. This is the part where the
//! tracker's own storage is replaced and memory actually drops.
//!
//! The dangerous half is not the pruning, it is **the sample set**. The tracker never learns who is
//! alive; a caller tells it. And "alive" is the wrong answer: every `lineage_id` in the MAP-Elites
//! archive can be selected as a parent by a later epoch, and an elite need not be an ancestor of
//! anyone currently alive. Prune by liveness alone and the very next reproduction names a node that
//! is gone.
//!
//! That used to be silent corruption — `add_reproduction` wrote the edge anyway, producing an orphan
//! edge, and an orphan edge makes the WHOLE graph unusable because both `to_newick` and `simplify`
//! refuse to process one. So a single missed sample would have poisoned export and every future
//! compaction. `add_reproduction` now refuses the edge instead, which is what makes compaction safe
//! to switch on at all; `a_missed_future_parent_costs_a_link_not_the_graph` is that guarantee.

use anima_engine_lib::evolution::genotype::{MorphologyGenotype, MorphologyNode};
use anima_engine_lib::evolution::lineage::{InMemoryLineageTracker, LineageTracker, RelationType};
use anima_engine_lib::evolution::newick::to_newick;

/// A genotype with real content, so a dropped node frees something worth freeing.
fn genotype(size: usize) -> MorphologyGenotype {
    MorphologyGenotype {
        nodes: (0..size)
            .map(|i| MorphologyNode {
                id: i as u32,
                length: 1.0,
                radius: 0.5,
                mass: 1.0,
            })
            .collect(),
        edges: Vec::new(),
    }
}

fn tracker_with_two_branches() -> InMemoryLineageTracker {
    let tracker = InMemoryLineageTracker::new();
    tracker
        .add_root("founder".into(), genotype(4))
        .expect("root recorded");

    // The surviving line.
    let mut previous = "founder".to_string();
    for g in 1..=4u32 {
        let id = format!("alive{g}");
        tracker
            .add_reproduction(
                id.clone(),
                g,
                genotype(4),
                vec![previous.clone()],
                RelationType::Mutate,
            )
            .expect("reproduction recorded");
        previous = id;
    }

    // A line that died out.
    let mut previous = "founder".to_string();
    for g in 1..=4u32 {
        let id = format!("extinct{g}");
        tracker
            .add_reproduction(
                id.clone(),
                g,
                genotype(4),
                vec![previous.clone()],
                RelationType::Clone,
            )
            .expect("reproduction recorded");
        previous = id;
    }
    tracker
}

#[test]
fn compaction_drops_extinct_branches() {
    let tracker = tracker_with_two_branches();
    let (before, _) = tracker.get_lineage_graph().expect("readable");
    assert_eq!(before.len(), 9);

    let report = tracker
        .compact(&["alive4".to_string()])
        .expect("a well-formed graph compacts");

    assert_eq!(report.nodes_before, 9);
    assert_eq!(report.nodes_after, 5, "founder + the four survivors");
    assert_eq!(report.nodes_removed(), 4);

    let (after, relations) = tracker.get_lineage_graph().expect("readable");
    let ids: Vec<&str> = after.iter().map(|n| n.id.as_str()).collect();
    assert!(!ids.iter().any(|id| id.starts_with("extinct")));
    assert!(
        relations
            .iter()
            .all(|r| !r.source_id.starts_with("extinct") && !r.target_id.starts_with("extinct")),
        "edges into a removed branch would be orphan edges"
    );
}

#[test]
fn the_surviving_trunk_is_kept_whole_because_compaction_does_not_compress() {
    // The live path runs `simplify` with compression OFF. Every intermediate ancestor stays, so the
    // lineage graph the UI draws and `get_mutations_count` keep working unchanged. This is the
    // deliberate limit on how far live compaction goes — see LineageTracker::compact.
    let tracker = tracker_with_two_branches();
    tracker.compact(&["alive4".to_string()]).expect("compacts");

    let (after, relations) = tracker.get_lineage_graph().expect("readable");
    let ids: Vec<&str> = after.iter().map(|n| n.id.as_str()).collect();
    for expected in ["founder", "alive1", "alive2", "alive3", "alive4"] {
        assert!(
            ids.contains(&expected),
            "{expected} should still be present"
        );
    }
    assert_eq!(relations.len(), 4, "the chain is intact, not collapsed");
    assert!(
        relations
            .iter()
            .all(|r| r.relation_type == RelationType::Mutate),
        "relation types survive exactly -- the mutation count depends on them"
    );
}

// ---- the archive hazard ---------------------------------------------------------------------------

#[test]
fn an_archive_elite_who_is_nobody_s_ancestor_survives_when_it_is_sampled() {
    // `extinct2` stands for a MAP-Elites elite: no living descendant, but still selectable as a
    // parent. Sampling it is what the live caller does, and it is why the sample set is not simply
    // "the living".
    let tracker = tracker_with_two_branches();
    let report = tracker
        .compact(&["alive4".to_string(), "extinct2".to_string()])
        .expect("compacts");

    let (after, _) = tracker.get_lineage_graph().expect("readable");
    let ids: Vec<&str> = after.iter().map(|n| n.id.as_str()).collect();
    assert!(ids.contains(&"extinct2"), "a sampled elite must survive");
    assert!(
        ids.contains(&"extinct1"),
        "and so must its ancestors, or its own lineage would dangle"
    );
    assert!(
        !ids.contains(&"extinct3"),
        "its descendants are still extinct and should go"
    );
    assert_eq!(report.nodes_after, 7);
}

#[test]
fn a_missed_future_parent_costs_a_link_not_the_graph() {
    // The failure this guard exists for. `extinct2` is pruned because the caller forgot to sample
    // it, and then a later epoch selects it as a parent anyway.
    let tracker = tracker_with_two_branches();
    tracker.compact(&["alive4".to_string()]).expect("compacts");

    let err = tracker
        .add_reproduction(
            "orphan-child".into(),
            5,
            genotype(4),
            vec!["extinct2".into()],
            RelationType::Mutate,
        )
        .expect_err("naming a pruned parent must be refused");
    assert!(
        err.contains("extinct2"),
        "the error names the parent: {err}"
    );

    // The point of refusing: the graph is still exportable. Had the edge been written, `to_newick`
    // and every future `compact` would reject the whole lineage from then on.
    let (nodes, relations) = tracker.get_lineage_graph().expect("readable");
    to_newick(&nodes, &relations).expect("the graph must remain valid after a refused edge");

    // The offspring itself is kept, as a new root. Losing the ancestry link is the cost; losing the
    // individual would be a second, larger error.
    assert!(nodes.iter().any(|n| n.id == "orphan-child"));
    assert!(
        !relations.iter().any(|r| r.target_id == "orphan-child"),
        "no edge should have been written"
    );
}

#[test]
fn a_partially_known_parent_list_keeps_the_edges_it_can() {
    // Crossover names two parents. One known, one pruned: the known edge is worth keeping, and
    // dropping both would lose ancestry the graph still has every right to record.
    let tracker = tracker_with_two_branches();
    tracker.compact(&["alive4".to_string()]).expect("compacts");

    let err = tracker
        .add_reproduction(
            "hybrid".into(),
            5,
            genotype(4),
            vec!["alive4".into(), "extinct2".into()],
            RelationType::Crossover,
        )
        .expect_err("the unknown parent is still reported");
    assert!(err.contains("extinct2"));
    assert!(!err.contains("alive4"), "the known parent is not at fault");

    let (_, relations) = tracker.get_lineage_graph().expect("readable");
    let parents: Vec<&str> = relations
        .iter()
        .filter(|r| r.target_id == "hybrid")
        .map(|r| r.source_id.as_str())
        .collect();
    assert_eq!(parents, vec!["alive4"], "the known edge survives");
}

// ---- shape and stability --------------------------------------------------------------------------

#[test]
fn compacting_twice_changes_nothing_the_second_time() {
    let tracker = tracker_with_two_branches();
    let first = tracker.compact(&["alive4".to_string()]).expect("compacts");
    let second = tracker.compact(&["alive4".to_string()]).expect("compacts");

    assert_eq!(second.nodes_before, first.nodes_after);
    assert_eq!(second.nodes_removed(), 0, "compaction is idempotent");
    assert_eq!(second.relations_before, second.relations_after);
}

#[test]
fn a_compacted_graph_is_still_exportable() {
    // Cheap structural validation for free: `to_newick` refuses cycles, orphan edges, duplicate ids
    // and generation inversions, so a successful export says the rewrite left the graph well formed.
    let tracker = tracker_with_two_branches();
    tracker.compact(&["alive4".to_string()]).expect("compacts");

    let (nodes, relations) = tracker.get_lineage_graph().expect("readable");
    let exported = to_newick(&nodes, &relations).expect("compaction must leave a valid graph");
    assert_eq!(exported.roots, 1);
    assert_eq!(
        exported.trees,
        vec!["((((alive4:1)alive3:1)alive2:1)alive1:1)founder;".to_string()]
    );
}

#[test]
fn compacting_against_every_node_removes_nothing() {
    // Negative control: if `compact` removed things regardless of the sample set, every assertion
    // above would still pass while the method quietly destroyed data.
    let tracker = tracker_with_two_branches();
    let (before, _) = tracker.get_lineage_graph().expect("readable");
    let all: Vec<String> = before.iter().map(|n| n.id.clone()).collect();

    let report = tracker.compact(&all).expect("compacts");
    assert_eq!(report.nodes_removed(), 0);
    assert_eq!(report.relations_before, report.relations_after);
}

#[test]
fn compaction_refuses_a_malformed_graph_rather_than_rewriting_it() {
    // A graph with a cycle cannot be compacted meaningfully, and silently rewriting one would
    // destroy the evidence of how it got that way. The live caller logs and carries on.
    let tracker = InMemoryLineageTracker::new();
    tracker.add_root("a".into(), genotype(2)).expect("root");
    tracker
        .add_reproduction(
            "b".into(),
            1,
            genotype(2),
            vec!["a".into()],
            RelationType::Clone,
        )
        .expect("recorded");
    // Re-parenting `a` onto `b` closes a loop. Written through the public API, so this is a shape
    // the tracker can genuinely end up in.
    tracker
        .add_reproduction(
            "a".into(),
            0,
            genotype(2),
            vec!["b".into()],
            RelationType::Clone,
        )
        .expect("recorded");

    let err = tracker
        .compact(&["b".to_string()])
        .expect_err("a malformed graph must not be silently rewritten");
    assert!(
        err.contains("duplicate") || err.contains("cycle") || err.contains("id"),
        "the error should name the defect: {err}"
    );
}
