//! OSS-070 — the Newick export gate.
//!
//! The export exists for two reasons, and only the second one needs a test file this size.
//!
//! The first is interop: `ape`, `ggtree`, DendroPy and Biopython all read Newick, so a lineage
//! becomes analysable without importing anything (Newick is a format, so no third-party library
//! licence enters into the engine).
//!
//! The second is that **a foreign parser is an independent check on whether the lineage graph is
//! well-formed at all**. A cycle, an orphan edge or a duplicate id can sit in the in-memory tracker
//! indefinitely without anything failing: nothing walks ancestry today, so nothing trips over them.
//! The export does walk it, so it is the first thing in the codebase that can notice. These tests
//! pin that it *does* notice, rather than emitting a string that happens to parse.
//!
//! Two properties here are easy to get wrong in a way that still looks right:
//!
//! - **Crossover makes the lineage a DAG, and Newick is a tree format.** The export keeps one parent
//!   and COUNTS the rest. A version that silently dropped them would pass every structural test in
//!   this file except `crossover_keeps_one_parent_and_reports_the_other`.
//! - **Which parent survives must not depend on edge order.** The in-memory tracker pushes in
//!   reproduction order; a Neo4j restore returns query order. An export whose shape depends on where
//!   the data came from is not reproducible, and nothing would report it.

use anima_engine_lib::evolution::genotype::MorphologyGenotype;
use anima_engine_lib::evolution::lineage::{
    InMemoryLineageTracker, LineageNode, LineageRelation, LineageTracker, RelationType,
};
use anima_engine_lib::evolution::newick::{to_newick, NewickError};

fn node(id: &str, generation: u32) -> LineageNode {
    LineageNode {
        id: id.to_string(),
        generation,
        genotype: None,
        cumulative_mutations: None,
    }
}

fn edge(source: &str, target: &str) -> LineageRelation {
    LineageRelation {
        source_id: source.to_string(),
        target_id: target.to_string(),
        relation_type: RelationType::Mutate,
        path_events: None,
    }
}

fn empty_genotype() -> MorphologyGenotype {
    MorphologyGenotype {
        nodes: Vec::new(),
        edges: Vec::new(),
    }
}

/// The lineage behind the committed fixture.
///
/// Deliberately awkward: two roots, a crossover whose second parent cannot be represented, a label
/// containing a space and one containing a colon. A fixture built from a tidy chain would prove the
/// export works on the case that was never in doubt.
fn fixture_lineage() -> (Vec<LineageNode>, Vec<LineageRelation>) {
    let nodes = vec![
        node("f-alpha", 0),
        node("f-beta", 0),
        node("child one", 1),
        node("child:two", 2),
        node("hybrid", 3),
    ];
    let relations = vec![
        edge("f-alpha", "child one"),
        edge("child one", "child:two"),
        edge("child:two", "hybrid"),
        // Second parent: a crossover. "child:two" < "f-beta", so this is the edge that is dropped.
        edge("f-beta", "hybrid"),
    ];
    (nodes, relations)
}

#[test]
fn the_committed_fixture_still_matches_what_the_export_produces() {
    // Two-sided gate. This half pins the Rust output against the file; `scripts/verify_newick.py`
    // makes DendroPy read the same file. Neither half alone is worth much: a Rust round-trip proves
    // the serialiser agrees with itself, and a parser reading a stale file proves nothing about the
    // current code.
    let (nodes, relations) = fixture_lineage();
    let out = to_newick(&nodes, &relations).expect("the fixture lineage is valid");

    assert_eq!(out.roots, 2);
    assert_eq!(out.dropped_parent_edges, 1);
    assert_eq!(
        out.to_file_contents(),
        include_str!("fixtures/newick/lineage_forest.nwk").replace("\r\n", "\n"),
        "the export no longer matches tests/fixtures/newick/lineage_forest.nwk -- if the change is \
         intended, regenerate the fixture AND re-run scripts/verify_newick.py"
    );
}

// ---- shape ------------------------------------------------------------------------------------

#[test]
fn a_chain_exports_as_nested_parentheses() {
    let nodes = vec![node("r", 0), node("c", 1), node("g", 2)];
    let out = to_newick(&nodes, &[edge("r", "c"), edge("c", "g")]).expect("valid lineage");
    assert_eq!(out.trees, vec!["((g:1)c:1)r;".to_string()]);
    assert_eq!(out.roots, 1);
    assert_eq!(out.dropped_parent_edges, 0);
}

#[test]
fn a_forest_comes_out_as_one_tree_per_root() {
    // Genesis calls `add_root` once per founder, so more than one root is the NORMAL case here,
    // not a malformed graph. Failing on it would have made the export useless for a real run.
    let nodes = vec![node("a", 0), node("b", 0), node("a-kid", 1)];
    let out = to_newick(&nodes, &[edge("a", "a-kid")]).expect("valid lineage");
    assert_eq!(out.roots, 2);
    // A hyphen is NOT reserved in Newick, so `a-kid` stays unquoted. Quoting it would still parse,
    // but it would mean the quoting rule fires on ordinary ids — and every UUID contains hyphens,
    // so the whole export would end up quoted for no reason.
    assert_eq!(out.trees, vec!["(a-kid:1)a;".to_string(), "b;".to_string()]);
    assert_eq!(out.to_file_contents(), "(a-kid:1)a;\nb;\n");
}

// ---- the DAG problem ---------------------------------------------------------------------------

#[test]
fn crossover_keeps_one_parent_and_reports_the_other() {
    // The whole reason this counter exists. An export that silently kept one parent would produce
    // a tree that parses, draws, and understates the lineage by exactly the crossover edges.
    let nodes = vec![node("mother", 0), node("father", 0), node("kid", 1)];
    let relations = vec![edge("father", "kid"), edge("mother", "kid")];

    let out = to_newick(&nodes, &relations).expect("a DAG is exportable as a view");

    assert_eq!(
        out.dropped_parent_edges, 1,
        "one of the two parent edges cannot be represented and must be reported"
    );
    // "father" < "mother" lexicographically, so father is the surviving parent.
    assert_eq!(
        out.trees,
        vec!["(kid:1)father;".to_string(), "mother;".to_string()]
    );
}

#[test]
fn which_parent_survives_does_not_depend_on_edge_order() {
    let nodes = vec![node("mother", 0), node("father", 0), node("kid", 1)];
    let one = to_newick(&nodes, &[edge("father", "kid"), edge("mother", "kid")]).expect("valid");
    let other = to_newick(&nodes, &[edge("mother", "kid"), edge("father", "kid")]).expect("valid");
    assert_eq!(
        one, other,
        "in-memory push order and a Neo4j restore's query order must give the same tree"
    );
}

// ---- defects the export is expected to catch ---------------------------------------------------

#[test]
fn a_cycle_is_refused() {
    let nodes = vec![node("a", 0), node("b", 1), node("c", 2)];
    let relations = vec![edge("a", "b"), edge("b", "c"), edge("c", "a")];
    match to_newick(&nodes, &relations) {
        Err(NewickError::Cycle { node }) => {
            assert!(["a", "b", "c"].contains(&node.as_str()), "got {node}");
        }
        other => panic!("a cycle must be refused, got {other:?}"),
    }
}

#[test]
fn a_self_relation_is_refused() {
    let nodes = vec![node("a", 0)];
    match to_newick(&nodes, &[edge("a", "a")]) {
        Err(NewickError::Cycle { node }) => assert_eq!(node, "a"),
        other => panic!("a node parenting itself must be refused, got {other:?}"),
    }
}

#[test]
fn a_cycle_beside_a_healthy_tree_is_still_refused() {
    // Negative control for the cheap implementation of cycle detection: "there are no roots" is NOT
    // the same test as "there is a cycle". This graph HAS a valid root and a valid tree hanging off
    // it, so an export that only checked for the absence of roots would return a happy answer and
    // silently omit the looping half.
    let nodes = vec![node("root", 0), node("kid", 1), node("x", 5), node("y", 6)];
    let relations = vec![edge("root", "kid"), edge("x", "y"), edge("y", "x")];
    match to_newick(&nodes, &relations) {
        Err(NewickError::Cycle { node }) => {
            assert!(["x", "y"].contains(&node.as_str()), "got {node}");
        }
        other => panic!("a cycle must be refused even next to a healthy tree, got {other:?}"),
    }
}

#[test]
fn the_reported_cycle_node_is_on_the_loop_not_merely_hanging_off_it() {
    // `tail` is not on the loop; it descends from it. Reporting `tail` would send whoever reads the
    // error to a node whose own record is fine.
    let nodes = vec![node("x", 0), node("y", 1), node("tail", 2)];
    let relations = vec![edge("x", "y"), edge("y", "x"), edge("y", "tail")];
    match to_newick(&nodes, &relations) {
        Err(NewickError::Cycle { node }) => assert!(
            node == "x" || node == "y",
            "expected a node on the loop, got {node}"
        ),
        other => panic!("expected a cycle error, got {other:?}"),
    }
}

#[test]
fn an_edge_naming_an_unknown_node_is_refused() {
    let nodes = vec![node("a", 0)];
    match to_newick(&nodes, &[edge("a", "ghost")]) {
        Err(NewickError::UnknownEndpoint { source, target }) => {
            assert_eq!(source, "a");
            assert_eq!(target, "ghost");
        }
        other => panic!("an orphan edge must be refused, got {other:?}"),
    }
}

#[test]
fn a_duplicate_id_is_refused() {
    let nodes = vec![node("dup", 0), node("dup", 1)];
    match to_newick(&nodes, &[]) {
        Err(NewickError::DuplicateNode { id }) => assert_eq!(id, "dup"),
        other => panic!("a duplicate id makes ancestry ambiguous, got {other:?}"),
    }
}

#[test]
fn a_child_recorded_as_older_than_its_parent_is_refused() {
    // Not a formatting problem: the generation counter and the parent edges disagree about time.
    // Clamping the branch length to zero would hide a real inconsistency behind a valid-looking
    // tree, which is exactly what this export exists to stop.
    let nodes = vec![node("parent", 7), node("child", 3)];
    match to_newick(&nodes, &[edge("parent", "child")]) {
        Err(NewickError::GenerationInversion {
            parent,
            child,
            parent_generation,
            child_generation,
        }) => {
            assert_eq!(parent, "parent");
            assert_eq!(child, "child");
            assert_eq!(parent_generation, 7);
            assert_eq!(child_generation, 3);
        }
        other => panic!("a generation inversion must be refused, got {other:?}"),
    }
}

// ---- scale -------------------------------------------------------------------------------------

#[test]
fn a_deep_chain_does_not_overflow_the_stack() {
    // A lineage is as deep as the run is long. A recursive emitter passes every test above and dies
    // on the run that matters, so depth is pinned rather than assumed.
    const DEPTH: u32 = 50_000;
    let nodes: Vec<LineageNode> = (0..DEPTH).map(|g| node(&format!("n{g:07}"), g)).collect();
    let relations: Vec<LineageRelation> = (1..DEPTH)
        .map(|g| LineageRelation {
            source_id: format!("n{:07}", g - 1),
            target_id: format!("n{g:07}"),
            relation_type: RelationType::Clone,
            path_events: None,
        })
        .collect();

    let out = to_newick(&nodes, &relations).expect("a deep chain is still a valid lineage");
    assert_eq!(out.roots, 1);
    assert_eq!(out.dropped_parent_edges, 0);
    let tree = &out.trees[0];
    assert_eq!(
        tree.matches('(').count(),
        (DEPTH - 1) as usize,
        "one nesting level per edge"
    );
    assert!(tree.ends_with("n0000000;"), "the root is emitted outermost");
}

// ---- against the real tracker ------------------------------------------------------------------

#[test]
fn it_exports_what_the_tracker_actually_produced() {
    // The tests above build `LineageNode`/`LineageRelation` by hand. This one goes through the
    // tracker's own API, so a change to how it records parents shows up here instead of leaving the
    // export passing against a shape the tracker no longer produces.
    let tracker = InMemoryLineageTracker::new();
    tracker
        .add_root("founder".into(), empty_genotype())
        .expect("root recorded");
    tracker
        .add_reproduction(
            "child".into(),
            1,
            empty_genotype(),
            vec!["founder".into()],
            RelationType::Mutate,
        )
        .expect("reproduction recorded");
    tracker
        .add_reproduction(
            "grandchild".into(),
            2,
            empty_genotype(),
            vec!["child".into()],
            RelationType::Clone,
        )
        .expect("reproduction recorded");

    let (nodes, relations) = tracker.get_lineage_graph().expect("graph readable");
    let out = to_newick(&nodes, &relations).expect("the tracker produces a valid lineage");

    assert_eq!(out.roots, 1);
    assert_eq!(out.dropped_parent_edges, 0);
    assert_eq!(
        out.trees,
        vec!["((grandchild:1)child:1)founder;".to_string()]
    );
}

#[test]
fn a_crossover_recorded_by_the_tracker_is_reported_as_a_dropped_edge() {
    // `add_reproduction` takes a Vec of parents and writes one relation each, so a crossover really
    // does produce the two-parent shape — this is not a hypothetical the export guards against.
    let tracker = InMemoryLineageTracker::new();
    tracker
        .add_root("alpha".into(), empty_genotype())
        .expect("root recorded");
    tracker
        .add_root("beta".into(), empty_genotype())
        .expect("root recorded");
    tracker
        .add_reproduction(
            "hybrid".into(),
            1,
            empty_genotype(),
            vec!["alpha".into(), "beta".into()],
            RelationType::Crossover,
        )
        .expect("reproduction recorded");

    let (nodes, relations) = tracker.get_lineage_graph().expect("graph readable");
    assert_eq!(
        relations.len(),
        2,
        "a crossover writes one relation per parent"
    );

    let out = to_newick(&nodes, &relations).expect("a DAG is exportable as a view");
    assert_eq!(
        out.dropped_parent_edges, 1,
        "the second parent is not representable in Newick and must be counted"
    );
}
