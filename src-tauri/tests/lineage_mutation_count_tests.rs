//! OSS-071b part 2 — the per-node cumulative mutation count, and the compression it unlocks.
//!
//! # What this file is defending
//!
//! Compaction reaches its O(alive) bound only by splicing unary paths out. A spliced edge stands
//! for a *path*, so it cannot carry the per-edge `RelationType` that the UI's mutation figure used
//! to be derived from — five `Mutate` edges compressed into one edge read back as **one** mutation.
//!
//! That is the worst shape a wrong number can have: finite, plausible, monotonic, and silently
//! smaller than the truth. Nobody would look at "3 mutations" and think the graph was compacted.
//!
//! So the count moved onto the node, and these tests pin the three things that makes it depend on:
//!
//! 1. the number the UI would show is **identical** before and after compaction (`survives_*`);
//! 2. a save written before the field existed still reads, and still gets the right number
//!    (`a_pre_field_graph_*`);
//! 3. compaction actually reaches the bound it exists for (`compaction_reaches_*`).
//!
//! Plus the structural trap that the first implementation walked straight into: filtering the
//! ORIGINAL relations by the surviving node set disconnects the graph the moment anything is
//! spliced, because both edges of `A → B → C` mention the removed `B`. See
//! `compaction_leaves_no_orphans`.

use anima_engine_lib::evolution::genotype::{MorphologyGenotype, MorphologyNode};
use anima_engine_lib::evolution::lineage::{
    cumulative_mutations_from_edges, InMemoryLineageTracker, LineageNode, LineageRelation,
    LineageTracker, RelationType,
};
use anima_engine_lib::evolution::newick::to_newick;

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

/// A trunk of `mutations` mutation events, then `clones` clone events, then a dead side branch.
/// The side branch is what compaction prunes; the trunk is what it compresses.
fn tracker_with_trunk(mutations: u32, clones: u32) -> (InMemoryLineageTracker, String) {
    let tracker = InMemoryLineageTracker::new();
    tracker.add_root("founder".into(), genotype(4)).unwrap();

    let mut previous = "founder".to_string();
    let mut g = 0u32;
    for _ in 0..mutations {
        g += 1;
        let id = format!("m{g}");
        tracker
            .add_reproduction(
                id.clone(),
                g,
                genotype(4),
                vec![previous.clone()],
                RelationType::Mutate,
            )
            .unwrap();
        previous = id;
    }
    for _ in 0..clones {
        g += 1;
        let id = format!("c{g}");
        tracker
            .add_reproduction(
                id.clone(),
                g,
                genotype(4),
                vec![previous.clone()],
                RelationType::Clone,
            )
            .unwrap();
        previous = id;
    }

    // A branch that dies out, so compaction has something to prune as well as compress.
    tracker
        .add_reproduction(
            "dead1".into(),
            1,
            genotype(4),
            vec!["founder".to_string()],
            RelationType::Mutate,
        )
        .unwrap();
    tracker
        .add_reproduction(
            "dead2".into(),
            2,
            genotype(4),
            vec!["dead1".to_string()],
            RelationType::Mutate,
        )
        .unwrap();

    (tracker, previous)
}

/// The figure `commands::evolution::get_lineage_graph` reports, reproduced through the same public
/// rule it uses: the stored count when recorded, the edge walk otherwise.
fn ui_mutation_count(nodes: &[LineageNode], relations: &[LineageRelation], id: &str) -> u32 {
    let derived = cumulative_mutations_from_edges(nodes, relations);
    let node = nodes
        .iter()
        .find(|n| n.id == id)
        .unwrap_or_else(|| panic!("{id} is not in the graph"));
    node.cumulative_mutations
        .or_else(|| derived.get(id).copied())
        .unwrap_or(0)
}

#[test]
fn the_live_tracker_records_a_count_that_matches_the_edge_walk() {
    // Before compaction the two must agree exactly, or the stored value is not a substitute for the
    // walk and every test below is comparing a number to itself.
    let (tracker, tip) = tracker_with_trunk(5, 3);
    let (nodes, relations) = tracker.get_lineage_graph().unwrap();

    let derived = cumulative_mutations_from_edges(&nodes, &relations);
    for node in &nodes {
        assert_eq!(
            node.cumulative_mutations,
            Some(derived[&node.id]),
            "stored and derived disagree at {}",
            node.id
        );
    }
    assert_eq!(ui_mutation_count(&nodes, &relations, &tip), 5);
    assert_eq!(ui_mutation_count(&nodes, &relations, "founder"), 0);
    // Clone events must not count. If they did, the number would be 8 and would still look sane.
    assert_eq!(ui_mutation_count(&nodes, &relations, "c8"), 5);
}

#[test]
fn survives_compaction_unchanged() {
    // The definition of done for OSS-071b: the number on the UI does not move when storage shrinks.
    let (tracker, tip) = tracker_with_trunk(5, 3);
    let (before_nodes, before_rels) = tracker.get_lineage_graph().unwrap();
    let before = ui_mutation_count(&before_nodes, &before_rels, &tip);

    let report = tracker.compact(std::slice::from_ref(&tip)).unwrap();
    assert!(
        report.nodes_removed() > 0,
        "nothing was removed, so this proves nothing"
    );

    let (after_nodes, after_rels) = tracker.get_lineage_graph().unwrap();
    let after = ui_mutation_count(&after_nodes, &after_rels, &tip);

    assert_eq!(
        before, after,
        "the mutation count moved across compaction ({before} -> {after})"
    );
    assert_eq!(before, 5);

    // And the negative control that gives the assertion above its teeth: the edge walk ALONE, on
    // the compacted graph, is now wrong. If this ever equals 5, compression stopped happening and
    // the test above went green for the wrong reason.
    let walked = cumulative_mutations_from_edges(&after_nodes, &after_rels)[&tip];
    assert!(
        walked < before,
        "expected the post-compaction edge walk to under-count (got {walked}, stored {before}); \
         if they now agree, unary-path compression is no longer running"
    );
}

#[test]
fn a_pre_field_graph_still_reads_and_still_counts() {
    // Every save written before `cumulative_mutations` existed deserializes with `None`. `None` has
    // to mean "not recorded, walk the edges" — the reason the field is `Option<u32>` and not `u32`,
    // because a defaulted `0` would read as "never mutated" for every historical save.
    let json = r#"{
        "id": "old3",
        "generation": 3,
        "genotype": null
    }"#;
    let node: LineageNode =
        serde_json::from_str(json).expect("a pre-field node still deserializes");
    assert_eq!(node.id, "old3");
    assert_eq!(
        node.cumulative_mutations, None,
        "an absent field must read as unknown, never as zero"
    );

    // A whole graph in the old shape still produces the right number through the fallback.
    let nodes: Vec<LineageNode> = (0..4)
        .map(|g| LineageNode {
            id: format!("old{g}"),
            generation: g,
            genotype: None,
            cumulative_mutations: None,
        })
        .collect();
    let relations: Vec<LineageRelation> = (1..4)
        .map(|g| LineageRelation {
            source_id: format!("old{}", g - 1),
            target_id: format!("old{g}"),
            // Two mutations and one clone, so a wrong rule that counts every edge would say 3.
            relation_type: if g == 2 {
                RelationType::Clone
            } else {
                RelationType::Mutate
            },
            path_events: None,
        })
        .collect();

    assert_eq!(ui_mutation_count(&nodes, &relations, "old3"), 2);
    assert_eq!(ui_mutation_count(&nodes, &relations, "old0"), 0);
}

#[test]
fn compaction_backfills_a_pre_field_graph_before_it_compresses() {
    // The ordering trap. A store restored from an old save has `None` everywhere; if compaction
    // compressed first and derived afterwards, it would derive from the ALREADY compressed graph
    // and bake in the under-count permanently. The backfill has to happen against the full graph.
    let (tracker, tip) = tracker_with_trunk(6, 0);
    let (mut nodes, relations) = tracker.get_lineage_graph().unwrap();

    // Simulate the restore: wipe every recorded count, exactly as an old save would arrive.
    for n in nodes.iter_mut() {
        n.cumulative_mutations = None;
    }
    let restored = InMemoryLineageTracker::new();
    restored.load_state(nodes, relations);

    let expected = 6;
    let (pre_nodes, pre_rels) = restored.get_lineage_graph().unwrap();
    assert_eq!(ui_mutation_count(&pre_nodes, &pre_rels, &tip), expected);

    restored.compact(std::slice::from_ref(&tip)).unwrap();

    let (post_nodes, post_rels) = restored.get_lineage_graph().unwrap();
    assert_eq!(
        ui_mutation_count(&post_nodes, &post_rels, &tip),
        expected,
        "an old save's count was lost across its first compaction"
    );
    assert_eq!(
        post_nodes
            .iter()
            .find(|n| n.id == tip)
            .unwrap()
            .cumulative_mutations,
        Some(expected),
        "compaction must leave every survivor with a recorded count"
    );
}

#[test]
fn compaction_reaches_the_two_samples_bound() {
    // The point of switching compression on. A long trunk must collapse to the sample plus its
    // retained branch points, not stay proportional to the number of generations ever lived.
    let (tracker, tip) = tracker_with_trunk(40, 40);
    let (before, _) = tracker.get_lineage_graph().unwrap();
    assert!(before.len() > 80, "fixture is too small to be a test");

    let samples = vec![tip];
    let report = tracker.compact(&samples).unwrap();

    let bound = 2 * samples.len();
    assert!(
        report.nodes_after <= bound,
        "compaction left {} nodes for {} sample(s); the 2*samples bound is {}",
        report.nodes_after,
        samples.len(),
        bound
    );
}

#[test]
fn compaction_leaves_no_orphans() {
    // The structural trap, and the one that fails loudest if the rewrite is done wrong: relations
    // must be rebuilt from the simplify PLAN, not filtered from the originals. Filtering keeps only
    // edges whose BOTH endpoints survived, and a spliced `A -> B -> C` has no such edge — so `C`
    // silently becomes a root and the ancestry the compaction was supposed to preserve is gone.
    //
    // `to_newick` is the detector: it refuses a graph with an edge to a node that does not exist,
    // and it reports how many roots it found.
    let (tracker, tip) = tracker_with_trunk(10, 5);
    tracker.compact(std::slice::from_ref(&tip)).unwrap();
    let (nodes, relations) = tracker.get_lineage_graph().unwrap();

    let ids: std::collections::HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    for r in &relations {
        assert!(
            ids.contains(r.source_id.as_str()),
            "edge {} -> {} has a source that no longer exists",
            r.source_id,
            r.target_id
        );
        assert!(
            ids.contains(r.target_id.as_str()),
            "edge {} -> {} has a target that no longer exists",
            r.source_id,
            r.target_id
        );
    }

    let out = to_newick(&nodes, &relations).expect("a compacted graph is still exportable");
    assert_eq!(
        out.roots, 1,
        "compaction split one lineage into {} roots — the survivors were disconnected",
        out.roots
    );
}

#[test]
fn a_compressed_edge_says_that_it_is_a_summary() {
    // `relation_type` on a spliced edge is the strongest event on the path, not an event that
    // happened. `path_events` is what lets a reader tell the two apart — without it the summary is
    // indistinguishable from an observation, which is the fabrication this avoids.
    let (tracker, tip) = tracker_with_trunk(8, 0);
    tracker.compact(std::slice::from_ref(&tip)).unwrap();
    let (_, relations) = tracker.get_lineage_graph().unwrap();

    let summarised: Vec<_> = relations
        .iter()
        .filter(|r| r.path_events.is_some_and(|n| n > 1))
        .collect();
    assert!(
        !summarised.is_empty(),
        "nothing was compressed, so there is no summary to check"
    );
    for r in summarised {
        assert_eq!(
            r.relation_type,
            RelationType::Mutate,
            "a path of mutations should summarise as Mutate"
        );
    }
}
