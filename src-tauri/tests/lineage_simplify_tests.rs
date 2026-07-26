//! OSS-071 — the `simplify()` gate.
//!
//! Two claims are being pinned, and they pull against each other:
//!
//! 1. **Memory becomes O(alive), not O(everyone who ever lived.)** That needs unary-path
//!    compression, not just pruning: the ancestors of a living population still reach back to
//!    genesis, so pruning alone removes extinct side branches and leaves every trunk.
//! 2. **Ancestry among the retained nodes is unchanged.** Compression removes nodes, so this is the
//!    property that stops (1) from being achieved by simply deleting things.
//!
//! The subtle one is neither. Compression rewrites an edge to stand for a *path*, and
//! `get_mutations_count` in `commands/evolution.rs` counts `Mutate` edges along ancestry to produce
//! the mutation figure the UI shows. Collapsing five `Mutate` edges into one keeps the type honest
//! and makes that count read 1 instead of 5 — finite, plausible, wrong by a factor of five. So the
//! counts on a `SimplifiedEdge` are load-bearing, and `mutation_counts_survive_compression` is the
//! test that would fail if someone "simplified" them away.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use anima_engine_lib::evolution::genotype::MorphologyGenotype;
use anima_engine_lib::evolution::lineage::{
    InMemoryLineageTracker, LineageNode, LineageRelation, LineageTracker, RelationType,
};
use anima_engine_lib::evolution::newick::to_newick_from;
use anima_engine_lib::evolution::simplify::{simplify, SimplifiedLineage, SimplifyError};

fn node(id: &str, generation: u32) -> LineageNode {
    LineageNode {
        id: id.to_string(),
        generation,
        genotype: None,
    }
}

fn rel(source: &str, target: &str, kind: RelationType) -> LineageRelation {
    LineageRelation {
        source_id: source.to_string(),
        target_id: target.to_string(),
        relation_type: kind,
    }
}

fn empty_genotype() -> MorphologyGenotype {
    MorphologyGenotype {
        nodes: Vec::new(),
        edges: Vec::new(),
    }
}

// ---- ancestry comparison ------------------------------------------------------------------------

/// Ancestors of `start`, keeping only ids in `keep`.
///
/// Restricting to `keep` is the whole point: the simplified graph does not contain the spliced-out
/// nodes, so an unrestricted comparison would always differ. What must not change is the ancestry
/// relation among the nodes that survive.
fn ancestors_within(
    start: &str,
    parents: &BTreeMap<String, Vec<String>>,
    keep: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    queue.push_back(start.to_string());
    while let Some(cur) = queue.pop_front() {
        for p in parents.get(&cur).into_iter().flatten() {
            if !seen.insert(p.clone()) {
                continue;
            }
            if keep.contains(p) {
                out.insert(p.clone());
            }
            queue.push_back(p.clone());
        }
    }
    out
}

fn parents_of_relations(relations: &[LineageRelation]) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in relations {
        map.entry(r.target_id.clone())
            .or_default()
            .push(r.source_id.clone());
    }
    map
}

fn parents_of_simplified(out: &SimplifiedLineage) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for e in &out.edges {
        map.entry(e.child_id.clone())
            .or_default()
            .push(e.parent_id.clone());
    }
    map
}

/// Returns the ids whose ancestry changed. Empty means the invariant held.
fn ancestry_differences(
    relations: &[LineageRelation],
    out: &SimplifiedLineage,
) -> Vec<(String, BTreeSet<String>, BTreeSet<String>)> {
    let keep: BTreeSet<String> = out.nodes.iter().map(|n| n.id.clone()).collect();
    let before = parents_of_relations(relations);
    let after = parents_of_simplified(out);

    let mut diffs = Vec::new();
    for id in &keep {
        let a = ancestors_within(id, &before, &keep);
        let b = ancestors_within(id, &after, &keep);
        if a != b {
            diffs.push((id.clone(), a, b));
        }
    }
    diffs
}

// ---- the headline invariant ---------------------------------------------------------------------

/// A lineage with a live trunk, a dead side branch, and a crossover.
fn mixed_lineage() -> (Vec<LineageNode>, Vec<LineageRelation>) {
    let nodes = vec![
        node("g0-root", 0),
        node("g1-trunk", 1),
        node("g2-trunk", 2),
        node("g3-alive", 3),
        node("g1-dead", 1),
        node("g2-dead", 2),
        node("g0-other", 0),
        node("g3-hybrid", 3),
    ];
    let relations = vec![
        rel("g0-root", "g1-trunk", RelationType::Mutate),
        rel("g1-trunk", "g2-trunk", RelationType::Mutate),
        rel("g2-trunk", "g3-alive", RelationType::Mutate),
        // A branch nobody alive descends from.
        rel("g0-root", "g1-dead", RelationType::Clone),
        rel("g1-dead", "g2-dead", RelationType::Mutate),
        // A crossover: two parents, so this child can never be spliced out.
        rel("g2-trunk", "g3-hybrid", RelationType::Crossover),
        rel("g0-other", "g3-hybrid", RelationType::Crossover),
    ];
    (nodes, relations)
}

#[test]
fn ancestry_among_retained_nodes_is_unchanged() {
    let (nodes, relations) = mixed_lineage();
    let samples = vec!["g3-alive".to_string(), "g3-hybrid".to_string()];
    let out = simplify(&nodes, &relations, &samples).expect("valid lineage");

    let diffs = ancestry_differences(&relations, &out);
    assert!(
        diffs.is_empty(),
        "ancestry changed for {:?}",
        diffs.iter().map(|d| &d.0).collect::<Vec<_>>()
    );
}

#[test]
fn the_ancestry_check_can_actually_fail() {
    // Negative control. Without this, `ancestry_among_retained_nodes_is_unchanged` could be passing
    // because the comparison is vacuous rather than because the invariant holds.
    let (nodes, relations) = mixed_lineage();
    let samples = vec!["g3-alive".to_string(), "g3-hybrid".to_string()];
    let mut out = simplify(&nodes, &relations, &samples).expect("valid lineage");

    // Sever one edge, exactly as a buggy compression pass would.
    out.edges.remove(0);

    let diffs = ancestry_differences(&relations, &out);
    assert!(
        !diffs.is_empty(),
        "removing an edge must show up as an ancestry difference, or the check proves nothing"
    );
}

// ---- pruning ------------------------------------------------------------------------------------

#[test]
fn a_branch_with_no_living_descendant_is_dropped() {
    let (nodes, relations) = mixed_lineage();
    let samples = vec!["g3-alive".to_string(), "g3-hybrid".to_string()];
    let out = simplify(&nodes, &relations, &samples).expect("valid lineage");

    let kept: BTreeSet<&str> = out.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(!kept.contains("g1-dead"), "extinct branch should be gone");
    assert!(!kept.contains("g2-dead"), "extinct branch should be gone");
    assert_eq!(out.dropped_nodes, 2);

    // Both samples and both roots survive; the crossover parent `g0-other` is an ancestor of a
    // sample, so it is retained even though nothing else connects it.
    assert!(kept.contains("g3-alive"));
    assert!(kept.contains("g3-hybrid"));
    assert!(kept.contains("g0-other"));
    assert!(kept.contains("g0-root"));
}

// ---- compression --------------------------------------------------------------------------------

#[test]
fn a_unary_chain_collapses_into_one_edge() {
    // root -> a -> b -> c -> tip, with only `tip` alive. Nothing in between branches, so nothing in
    // between carries information the ancestry needs.
    let nodes = vec![
        node("root", 0),
        node("a", 1),
        node("b", 2),
        node("c", 3),
        node("tip", 4),
    ];
    let relations = vec![
        rel("root", "a", RelationType::Mutate),
        rel("a", "b", RelationType::Clone),
        rel("b", "c", RelationType::Mutate),
        rel("c", "tip", RelationType::Mutate),
    ];
    let out = simplify(&nodes, &relations, &["tip".to_string()]).expect("valid lineage");

    let kept: Vec<&str> = out.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(
        kept,
        vec!["root", "tip"],
        "only the root and the sample remain"
    );
    assert_eq!(out.compressed_nodes, 3);
    assert_eq!(
        out.dropped_nodes, 0,
        "nothing was extinct; everything was on the path"
    );
    assert_eq!(out.edges.len(), 1);
}

#[test]
fn mutation_counts_survive_compression() {
    // The reason SimplifiedEdge carries counts at all. Four reproductions, three of them mutations:
    // an edge that merely said "Mutate" would let a caller conclude one.
    let nodes = vec![
        node("root", 0),
        node("a", 1),
        node("b", 2),
        node("c", 3),
        node("tip", 4),
    ];
    let relations = vec![
        rel("root", "a", RelationType::Mutate),
        rel("a", "b", RelationType::Clone),
        rel("b", "c", RelationType::Mutate),
        rel("c", "tip", RelationType::Mutate),
    ];
    let out = simplify(&nodes, &relations, &["tip".to_string()]).expect("valid lineage");

    let edge = &out.edges[0];
    assert_eq!(edge.parent_id, "root");
    assert_eq!(edge.child_id, "tip");
    assert_eq!(edge.events, 4, "four reproductions were collapsed");
    assert_eq!(edge.mutations, 3, "three of them were mutations");
    assert_eq!(edge.crossovers, 0);
}

#[test]
fn a_crossover_child_is_never_compressed_away() {
    // It has two parents, so splicing it out would have to pick one and silently discard the other
    // — losing the fact that a crossover happened at all.
    let (nodes, relations) = mixed_lineage();
    let out = simplify(&nodes, &relations, &["g3-hybrid".to_string()]).expect("valid lineage");

    let kept: BTreeSet<&str> = out.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(kept.contains("g3-hybrid"));
    let parents: Vec<&str> = out
        .edges
        .iter()
        .filter(|e| e.child_id == "g3-hybrid")
        .map(|e| e.parent_id.as_str())
        .collect();
    assert_eq!(parents.len(), 2, "both parents of a crossover must survive");
}

#[test]
fn a_diamond_is_left_alone_rather_than_folded() {
    // root -> left -> join, root -> right -> join. `left` and `right` are each unary, but splicing
    // either one would put two edges between root and join -- two distinct ancestral paths merged
    // into one, with their event counts added together as if they were sequential.
    let nodes = vec![
        node("root", 0),
        node("left", 1),
        node("right", 1),
        node("join", 2),
    ];
    let relations = vec![
        rel("root", "left", RelationType::Mutate),
        rel("root", "right", RelationType::Mutate),
        rel("left", "join", RelationType::Crossover),
        rel("right", "join", RelationType::Crossover),
    ];
    let out = simplify(&nodes, &relations, &["join".to_string()]).expect("valid lineage");

    // One of the two sides may compress (whichever is processed first leaves the other blocked);
    // what must NOT happen is both collapsing into parallel root->join edges.
    let root_to_join = out
        .edges
        .iter()
        .filter(|e| e.parent_id == "root" && e.child_id == "join")
        .count();
    assert!(
        root_to_join <= 1,
        "two parallel root->join edges means two distinct paths were folded together"
    );
    let diffs = ancestry_differences(&relations, &out);
    assert!(diffs.is_empty(), "ancestry changed: {diffs:?}");
}

// ---- the memory bound ---------------------------------------------------------------------------

/// A perfect binary tree of the given depth. `depth = 10` is 2,047 nodes and 1,024 leaves.
fn binary_lineage(depth: u32) -> (Vec<LineageNode>, Vec<LineageRelation>, Vec<String>) {
    let mut nodes = Vec::new();
    let mut relations = Vec::new();
    let mut leaves = Vec::new();

    for g in 0..=depth {
        let count = 1u32 << g;
        for i in 0..count {
            let id = format!("g{g:02}-{i:05}");
            nodes.push(node(&id, g));
            if g > 0 {
                let parent = format!("g{:02}-{:05}", g - 1, i / 2);
                relations.push(rel(&parent, &id, RelationType::Mutate));
            }
            if g == depth {
                leaves.push(id);
            }
        }
    }
    (nodes, relations, leaves)
}

#[test]
fn the_retained_set_is_bounded_by_the_sample_count_not_the_history() {
    // This is the claim OSS-071 exists to make good on: memory O(alive), not O(ever lived).
    const DEPTH: u32 = 10;
    let (nodes, relations, leaves) = binary_lineage(DEPTH);
    assert_eq!(nodes.len(), 2047);

    // 16 living individuals, spread across the leaf row so their ancestry really does branch.
    let samples: Vec<String> = leaves.iter().step_by(leaves.len() / 16).cloned().collect();
    assert_eq!(samples.len(), 16);

    let out = simplify(&nodes, &relations, &samples).expect("valid lineage");

    // A tree simplified to its branch points has at most 2S-1 nodes; the root survives even when
    // unary, because a founder is not a splice point.
    assert!(
        out.nodes.len() <= 2 * samples.len(),
        "retained {} nodes for {} samples -- the bound is what makes this O(alive)",
        out.nodes.len(),
        samples.len()
    );
    assert!(
        out.nodes.len() < nodes.len() / 50,
        "retained {} of {} -- pruning alone would have kept every trunk",
        out.nodes.len(),
        nodes.len()
    );
    // Every sample is still present: they are never pruned and never spliced.
    let kept: BTreeSet<&str> = out.nodes.iter().map(|n| n.id.as_str()).collect();
    for s in &samples {
        assert!(kept.contains(s.as_str()), "sample {s} was removed");
    }

    let diffs = ancestry_differences(&relations, &out);
    assert!(
        diffs.is_empty(),
        "ancestry changed for {} nodes",
        diffs.len()
    );
}

#[test]
fn pruning_without_compression_would_not_have_been_enough() {
    // Documents WHY step 2 exists, as a measurement rather than a claim in a comment: pruning alone
    // retains every ancestor of every sample, which for this shape is most of the tree's depth.
    const DEPTH: u32 = 10;
    let (nodes, relations, leaves) = binary_lineage(DEPTH);
    let samples: Vec<String> = leaves.iter().step_by(leaves.len() / 16).cloned().collect();

    let keep: BTreeSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let parents = parents_of_relations(&relations);
    let mut pruned_only: BTreeSet<String> = samples.iter().cloned().collect();
    for s in &samples {
        pruned_only.extend(ancestors_within(s, &parents, &keep));
    }

    let out = simplify(&nodes, &relations, &samples).expect("valid lineage");
    assert!(
        pruned_only.len() > out.nodes.len() * 3,
        "pruning alone kept {}, compression got it to {} -- if these were close, step 2 would not \
         be earning its complexity",
        pruned_only.len(),
        out.nodes.len()
    );
}

// ---- determinism ---------------------------------------------------------------------------------

#[test]
fn the_result_does_not_depend_on_the_order_relations_arrived_in() {
    // The in-memory tracker pushes in reproduction order; a Neo4j restore returns query order.
    let (nodes, relations) = mixed_lineage();
    let samples = vec!["g3-alive".to_string(), "g3-hybrid".to_string()];

    let forward = simplify(&nodes, &relations, &samples).expect("valid");
    let mut reversed = relations.clone();
    reversed.reverse();
    let backward = simplify(&nodes, &reversed, &samples).expect("valid");

    let ids_f: Vec<&str> = forward.nodes.iter().map(|n| n.id.as_str()).collect();
    let ids_b: Vec<&str> = backward.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids_f, ids_b);
    assert_eq!(forward.edges, backward.edges);
}

// ---- cross-check through Newick ------------------------------------------------------------------

#[test]
fn the_simplified_lineage_is_still_a_tree_a_newick_parser_would_accept() {
    // OSS-070 was built partly to give this check something to stand on: a simplified lineage that
    // still serialises as a well-formed Newick forest has no cycle, no orphan edge and no
    // generation inversion. Structural validation the assertions above do not cover, for free.
    const DEPTH: u32 = 8;
    let (nodes, relations, leaves) = binary_lineage(DEPTH);
    let samples: Vec<String> = leaves.iter().step_by(16).cloned().collect();

    let out = simplify(&nodes, &relations, &samples).expect("valid lineage");
    let pairs: Vec<(&str, &str)> = out
        .edges
        .iter()
        .map(|e| (e.parent_id.as_str(), e.child_id.as_str()))
        .collect();

    let exported = to_newick_from(&out.nodes, pairs.iter().copied())
        .expect("a simplified lineage must still be exportable");
    assert_eq!(exported.roots, 1);
    assert_eq!(exported.dropped_parent_edges, 0, "a tree has no crossover");
    let tree = &exported.trees[0];
    assert!(
        tree.ends_with("g00-00000;"),
        "the founder is outermost: {tree}"
    );
    // With samples every 16th leaf, the ancestry branches only in the top four levels, so the
    // simplified tree is exactly 16 leaves + 15 branch points. Asserting the number rather than
    // "it got smaller" is what makes this a bound rather than an impression.
    assert_eq!(out.nodes.len(), 31, "16 samples + 15 branch points");

    // Branch lengths are generation deltas, so compression shows up as a length greater than one.
    // Read the numbers out instead of grepping for a literal: the first version of this assertion
    // looked for ":2".."::4" and failed on a correct tree whose collapsed edges span 5 generations
    // — the test was wrong about the shape, not the code.
    let max_branch = tree
        .split(':')
        .skip(1)
        .filter_map(|s| {
            s.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse::<u32>()
                .ok()
        })
        .max()
        .unwrap_or(0);
    assert!(
        max_branch > 1,
        "no edge spans more than one generation, so nothing was compressed: {tree}"
    );
}

// ---- against the real tracker ---------------------------------------------------------------------

#[test]
fn it_simplifies_what_the_tracker_actually_produced() {
    let tracker = InMemoryLineageTracker::new();
    tracker
        .add_root("founder".into(), empty_genotype())
        .expect("root recorded");
    let mut previous = "founder".to_string();
    for g in 1..=5u32 {
        let id = format!("gen{g}");
        tracker
            .add_reproduction(
                id.clone(),
                g,
                empty_genotype(),
                vec![previous.clone()],
                RelationType::Mutate,
            )
            .expect("reproduction recorded");
        previous = id;
    }

    let (nodes, relations) = tracker.get_lineage_graph().expect("graph readable");
    assert_eq!(nodes.len(), 6);

    let out = simplify(&nodes, &relations, &[previous.clone()]).expect("valid lineage");
    let kept: Vec<&str> = out.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(kept, vec!["founder", "gen5"]);
    assert_eq!(out.edges.len(), 1);
    assert_eq!(out.edges[0].events, 5);
    assert_eq!(out.edges[0].mutations, 5);
    assert_eq!(out.compressed_nodes, 4);
}

#[test]
fn an_unknown_sample_is_refused() {
    let (nodes, relations) = mixed_lineage();
    match simplify(&nodes, &relations, &["nobody".to_string()]) {
        Err(SimplifyError::UnknownSample { id }) => assert_eq!(id, "nobody"),
        other => panic!("expected UnknownSample, got {other:?}"),
    }
}
