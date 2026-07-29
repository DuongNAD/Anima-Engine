//! OSS-072 — the MRCA gate.
//!
//! Three claims are being pinned, and only the first is the obvious one.
//!
//! 1. **The answer is right on graphs whose answer is known.** Hand-drawn trees with the MRCA
//!    written into the test name.
//! 2. **The answer is right on graphs whose answer nobody worked out by hand.** A brute-force
//!    oracle recomputes it by set intersection and pairwise reachability — no topological order, no
//!    shared helper, nothing borrowed from the implementation — and the two are compared over a
//!    deterministically generated DAG with crossovers in it. `the_oracle_can_actually_disagree` is
//!    the negative control that stops this from being a comparison of a function with itself.
//! 3. **Compaction does not move the MRCA.** This is the cross-subsystem invariant, and it is not
//!    an accident: the MRCA of a sample set has at least two retained children (otherwise its child
//!    would be a more recent common ancestor), so unary-path splicing can never remove it. If that
//!    stops holding, the science built on top of the lineage silently changes answer after epoch 50.
//!
//! The DAG cases are the point of the file. A tree has one MRCA and every phylogenetics library
//! returns it; this lineage has crossover, so it does not, and a version of this code that returned
//! one node would pass every tree test here.

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

use anima_engine_lib::evolution::genotype::MorphologyGenotype;
use anima_engine_lib::evolution::lineage::{
    InMemoryLineageTracker, LineageNode, LineageRelation, LineageTracker, RelationType,
};
use anima_engine_lib::evolution::mrca::{mrca, MrcaError};
use anima_engine_lib::evolution::newick::to_newick;

fn node(id: &str, generation: u32) -> LineageNode {
    LineageNode {
        id: id.to_string(),
        generation,
        genotype: None,
        cumulative_mutations: None,
    }
}

fn rel(source: &str, target: &str, kind: RelationType) -> LineageRelation {
    LineageRelation {
        source_id: source.to_string(),
        target_id: target.to_string(),
        relation_type: kind,
        path_events: None,
    }
}

fn empty_genotype() -> MorphologyGenotype {
    MorphologyGenotype {
        nodes: Vec::new(),
        edges: Vec::new(),
    }
}

fn who(nodes: &[LineageNode], relations: &[LineageRelation], individuals: &[&str]) -> Vec<String> {
    let query: Vec<String> = individuals.iter().map(|s| s.to_string()).collect();
    mrca(nodes, relations, &query)
        .expect("valid lineage")
        .ancestors
        .into_iter()
        .map(|a| a.id)
        .collect()
}

// ---- the brute-force oracle ----------------------------------------------------------------------
//
// Deliberately naive and deliberately independent. It shares no function with the implementation:
// where `mrca` computes one topological order and one linear "does this reach the common set" pass,
// this walks reachability separately for every candidate pair. It is O(|C|^2 * E) and would be a
// poor implementation, which is exactly what makes it a usable second opinion.

fn parent_map(relations: &[LineageRelation]) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in relations {
        map.entry(r.target_id.clone())
            .or_default()
            .push(r.source_id.clone());
    }
    map
}

fn child_map(relations: &[LineageRelation]) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in relations {
        map.entry(r.source_id.clone())
            .or_default()
            .push(r.target_id.clone());
    }
    map
}

/// Ancestors of `start`, **including `start`** — the reflexive convention the module declares.
fn ancestors_of(start: &str, parents: &BTreeMap<String, Vec<String>>) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    out.insert(start.to_string());
    queue.push_back(start.to_string());
    while let Some(cur) = queue.pop_front() {
        for p in parents.get(&cur).into_iter().flatten() {
            if out.insert(p.clone()) {
                queue.push_back(p.clone());
            }
        }
    }
    out
}

/// Can `from` reach `to` by descending? Used to strip the common ancestors that are not maximal.
fn reaches(from: &str, to: &str, children: &BTreeMap<String, Vec<String>>) -> bool {
    let mut seen: HashSet<String> = HashSet::new();
    let mut stack: Vec<String> = vec![from.to_string()];
    while let Some(cur) = stack.pop() {
        for c in children.get(&cur).into_iter().flatten() {
            if c == to {
                return true;
            }
            if seen.insert(c.clone()) {
                stack.push(c.clone());
            }
        }
    }
    false
}

fn oracle_mrca(relations: &[LineageRelation], individuals: &[String]) -> BTreeSet<String> {
    let parents = parent_map(relations);
    let children = child_map(relations);

    let mut common: Option<BTreeSet<String>> = None;
    for id in individuals {
        let a = ancestors_of(id, &parents);
        common = Some(match common {
            None => a,
            Some(prev) => prev.intersection(&a).cloned().collect(),
        });
    }
    let common = common.unwrap_or_default();

    common
        .iter()
        .filter(|c| {
            // Maximal: no *other* common ancestor sits below this one.
            !common
                .iter()
                .any(|other| other != *c && reaches(c, other, &children))
        })
        .cloned()
        .collect()
}

// ---- known answers -------------------------------------------------------------------------------

/// ```text
///        root
///        /  \
///       a    b
///      / \    \
///    x1  x2    y
/// ```
fn small_tree() -> (Vec<LineageNode>, Vec<LineageRelation>) {
    let nodes = vec![
        node("root", 0),
        node("a", 1),
        node("b", 1),
        node("x1", 2),
        node("x2", 2),
        node("y", 2),
    ];
    let relations = vec![
        rel("root", "a", RelationType::Mutate),
        rel("root", "b", RelationType::Mutate),
        rel("a", "x1", RelationType::Mutate),
        rel("a", "x2", RelationType::Clone),
        rel("b", "y", RelationType::Mutate),
    ];
    (nodes, relations)
}

#[test]
fn two_siblings_coalesce_at_their_parent() {
    let (nodes, relations) = small_tree();
    assert_eq!(who(&nodes, &relations, &["x1", "x2"]), vec!["a"]);
}

#[test]
fn two_cousins_coalesce_at_the_root() {
    let (nodes, relations) = small_tree();
    assert_eq!(who(&nodes, &relations, &["x1", "y"]), vec!["root"]);
}

#[test]
fn a_whole_population_coalesces_at_the_root() {
    let (nodes, relations) = small_tree();
    assert_eq!(who(&nodes, &relations, &["x1", "x2", "y"]), vec!["root"]);
}

#[test]
fn the_shared_trunk_is_reported_separately_from_the_answer() {
    // `common_ancestors` counts everything shared, `ancestors` only the most recent. For two
    // siblings under `a` under `root`, that is 2 and 1 — and the difference is what tells a caller
    // these two have a history rather than having just met.
    let (nodes, relations) = small_tree();
    let out = mrca(&nodes, &relations, &["x1".to_string(), "x2".to_string()]).expect("valid");
    assert_eq!(out.ancestors.len(), 1);
    assert_eq!(out.common_ancestors, 2, "a and root");
    assert!(!out.is_ambiguous());
}

// ---- the DAG case, which is why this module returns a set -----------------------------------------

/// Two siblings, each a crossover of the same pair. `a` and `b` are both common ancestors of `x`
/// and `y`, and neither descends from the other.
///
/// ```text
///       r
///      / \
///     a   b
///     |\ /|
///     | X |
///     |/ \|
///     x   y
/// ```
fn crossover_diamond() -> (Vec<LineageNode>, Vec<LineageRelation>) {
    let nodes = vec![
        node("r", 0),
        node("a", 1),
        node("b", 1),
        node("x", 2),
        node("y", 2),
    ];
    let relations = vec![
        rel("r", "a", RelationType::Mutate),
        rel("r", "b", RelationType::Mutate),
        rel("a", "x", RelationType::Crossover),
        rel("b", "x", RelationType::Crossover),
        rel("a", "y", RelationType::Crossover),
        rel("b", "y", RelationType::Crossover),
    ];
    (nodes, relations)
}

#[test]
fn a_crossover_diamond_has_two_answers_and_both_are_returned() {
    // The whole reason `ancestors` is a Vec. A version of this that returned one node would pass
    // every tree test in this file and be wrong here — and wrong in the direction that looks right,
    // because one plausible ancestor is exactly what a caller expects to see.
    let (nodes, relations) = crossover_diamond();
    assert_eq!(who(&nodes, &relations, &["x", "y"]), vec!["a", "b"]);

    let out = mrca(&nodes, &relations, &["x".to_string(), "y".to_string()]).expect("valid");
    assert!(out.is_ambiguous());
    assert_eq!(out.common_ancestors, 3, "a, b and r");
}

#[test]
fn the_less_recent_common_ancestor_is_not_reported_as_an_answer() {
    // `r` is a common ancestor of `x` and `y` too. It is not *the most recent* one, and reporting it
    // alongside `a` and `b` would make "most recent" meaningless.
    let (nodes, relations) = crossover_diamond();
    assert!(!who(&nodes, &relations, &["x", "y"]).contains(&"r".to_string()));
}

// ---- the declared conventions ---------------------------------------------------------------------

#[test]
fn individuals_from_different_founding_lines_have_no_common_ancestor_and_that_is_not_an_error() {
    // Genesis calls `add_root` once per founder, so a forest is the normal shape here. An `Err`
    // would make the ordinary case an error path.
    let nodes = vec![node("f1", 0), node("f2", 0), node("c1", 1), node("c2", 1)];
    let relations = vec![
        rel("f1", "c1", RelationType::Mutate),
        rel("f2", "c2", RelationType::Mutate),
    ];
    let out = mrca(&nodes, &relations, &["c1".to_string(), "c2".to_string()]).expect("valid");
    assert!(out.ancestors.is_empty());
    assert_eq!(out.common_ancestors, 0);
}

#[test]
fn an_individual_is_its_own_ancestor() {
    let (nodes, relations) = small_tree();
    assert_eq!(who(&nodes, &relations, &["x1"]), vec!["x1"]);
    assert_eq!(who(&nodes, &relations, &["a", "x1"]), vec!["a"]);
}

#[test]
fn an_empty_query_is_refused() {
    let (nodes, relations) = small_tree();
    assert_eq!(mrca(&nodes, &relations, &[]), Err(MrcaError::NoIndividuals));
}

#[test]
fn an_unknown_individual_is_refused_rather_than_dropped_from_the_query() {
    let (nodes, relations) = small_tree();
    match mrca(&nodes, &relations, &["x1".to_string(), "ghost".to_string()]) {
        Err(MrcaError::UnknownIndividual { id }) => assert_eq!(id, "ghost"),
        other => panic!("expected UnknownIndividual, got {other:?}"),
    }
}

#[test]
fn an_orphan_edge_is_refused_the_same_way_export_and_compaction_refuse_it() {
    let (nodes, mut relations) = small_tree();
    relations.push(rel("nowhere", "x1", RelationType::Mutate));
    match mrca(&nodes, &relations, &["x1".to_string()]) {
        Err(MrcaError::UnknownEndpoint { source, .. }) => assert_eq!(source, "nowhere"),
        other => panic!("expected UnknownEndpoint, got {other:?}"),
    }
}

#[test]
fn a_duplicate_id_is_refused_because_ancestry_is_keyed_by_id() {
    let nodes = vec![node("a", 0), node("a", 1)];
    match mrca(&nodes, &[], &["a".to_string()]) {
        Err(MrcaError::DuplicateNode { id }) => assert_eq!(id, "a"),
        other => panic!("expected DuplicateNode, got {other:?}"),
    }
}

// ---- ordering and determinism ----------------------------------------------------------------------

#[test]
fn answers_come_back_most_recent_first() {
    // Two maximal ancestors at different generations. Ordering by generation descending is what
    // makes `ancestors[0]` mean something to a caller that wants one.
    //
    //   old ------------> x        (old is a direct parent of x)
    //   old -> mid -----> y
    //   recent ---------> x
    //   recent ---------> y
    let nodes = vec![
        node("old", 0),
        node("mid", 1),
        node("recent", 1),
        node("x", 2),
        node("y", 2),
    ];
    let relations = vec![
        rel("old", "x", RelationType::Crossover),
        rel("old", "mid", RelationType::Mutate),
        rel("mid", "y", RelationType::Crossover),
        rel("recent", "x", RelationType::Crossover),
        rel("recent", "y", RelationType::Crossover),
    ];
    let out = mrca(&nodes, &relations, &["x".to_string(), "y".to_string()]).expect("valid");
    let listed: Vec<&str> = out.ancestors.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(
        listed,
        vec!["recent", "old"],
        "generation 1 before generation 0"
    );
}

#[test]
fn the_answer_does_not_depend_on_the_order_the_relations_arrived_in() {
    // The in-memory tracker pushes in reproduction order; a Neo4j restore returns query order. A
    // query whose answer depends on which one it got is not reproducible.
    let (nodes, relations) = crossover_diamond();
    let query = vec!["x".to_string(), "y".to_string()];

    let forward = mrca(&nodes, &relations, &query).expect("valid");
    let mut reversed = relations.clone();
    reversed.reverse();
    let backward = mrca(&nodes, &reversed, &query).expect("valid");
    assert_eq!(forward, backward);

    // And not on the order the individuals were named in either.
    let swapped = mrca(&nodes, &relations, &["y".to_string(), "x".to_string()]).expect("valid");
    assert_eq!(forward, swapped);
}

// ---- against the oracle ----------------------------------------------------------------------------

/// A layered DAG with crossovers, built from a fixed LCG so it is the same graph on every machine.
///
/// No `rand`: `sim_determinism_tests` scans the source for `thread_rng()`, and more to the point a
/// gate whose fixture differs per run reports a different failure every time it fails.
fn layered_dag(generations: u32, width: usize) -> (Vec<LineageNode>, Vec<LineageRelation>) {
    let mut nodes = Vec::new();
    let mut relations = Vec::new();
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as usize
    };

    for g in 0..=generations {
        for i in 0..width {
            let id = format!("g{g:02}-{i:03}");
            nodes.push(node(&id, g));
            if g == 0 {
                continue;
            }
            let p1 = next() % width;
            let parent1 = format!("g{:02}-{:03}", g - 1, p1);
            // Every third child is a crossover, which is what makes this a DAG rather than a tree.
            if next() % 3 == 0 {
                let p2 = (p1 + 1 + next() % (width - 1)) % width;
                let parent2 = format!("g{:02}-{:03}", g - 1, p2);
                relations.push(rel(&parent1, &id, RelationType::Crossover));
                relations.push(rel(&parent2, &id, RelationType::Crossover));
            } else {
                relations.push(rel(&parent1, &id, RelationType::Mutate));
            }
        }
    }
    (nodes, relations)
}

#[test]
fn the_implementation_agrees_with_a_brute_force_oracle_across_a_dag() {
    let (nodes, relations) = layered_dag(6, 8);
    // A well-formed fixture, checked by a third party rather than assumed: Newick export refuses
    // cycles, orphan edges and generation inversions.
    to_newick(&nodes, &relations).expect("the fixture itself must be a valid lineage");

    let leaves: Vec<String> = nodes
        .iter()
        .filter(|n| n.generation == 6)
        .map(|n| n.id.clone())
        .collect();

    let mut compared = 0usize;
    let mut saw_ambiguous = false;
    for i in 0..leaves.len() {
        for j in i..leaves.len() {
            let query = vec![leaves[i].clone(), leaves[j].clone()];
            let got: BTreeSet<String> = mrca(&nodes, &relations, &query)
                .expect("valid")
                .ancestors
                .into_iter()
                .map(|a| a.id)
                .collect();
            let want = oracle_mrca(&relations, &query);
            assert_eq!(got, want, "disagreed on {query:?}");
            saw_ambiguous |= got.len() > 1;
            compared += 1;
        }
    }
    assert_eq!(
        compared, 36,
        "8 leaves, every unordered pair including self"
    );
    assert!(
        saw_ambiguous,
        "the fixture never produced a multi-answer MRCA, so this run did not exercise the DAG case \
         at all — the crossover rate or the width needs to change, not this assertion"
    );

    // The whole leaf row at once, which is the query OSS-073 will actually make.
    let got: BTreeSet<String> = mrca(&nodes, &relations, &leaves)
        .expect("valid")
        .ancestors
        .into_iter()
        .map(|a| a.id)
        .collect();
    assert_eq!(got, oracle_mrca(&relations, &leaves));
}

#[test]
fn the_oracle_can_actually_disagree() {
    // Negative control. Without it, the comparison above could be passing because both sides return
    // the same empty set for every input.
    let (_, relations) = crossover_diamond();
    let query = vec!["x".to_string(), "y".to_string()];
    let truth = oracle_mrca(&relations, &query);
    assert_eq!(truth.len(), 2);

    // Sever one crossover edge, exactly as a graph that lost a parent link would look.
    let damaged: Vec<LineageRelation> = relations
        .iter()
        .filter(|r| !(r.source_id == "b" && r.target_id == "y"))
        .cloned()
        .collect();
    assert_ne!(
        oracle_mrca(&damaged, &query),
        truth,
        "the oracle returns the same answer for a graph missing an edge, so it proves nothing"
    );
}

// ---- compaction does not move the MRCA ---------------------------------------------------------------

#[test]
fn compaction_leaves_the_mrca_where_it_was() {
    // The cross-subsystem invariant, and it holds for a structural reason rather than by luck: the
    // MRCA of a sample set has at least two retained children — if it had one, that child would be
    // a more recent common ancestor — so unary-path splicing can never reach it.
    //
    // What compaction *does* change is the trunk above it, which is why only the answer is compared
    // and `common_ancestors` is asserted to have shrunk.
    let tracker = InMemoryLineageTracker::new();
    tracker
        .add_root("founder".into(), empty_genotype())
        .expect("root recorded");

    // A long unary trunk, then a split, then two long unary tails. Everything on the trunk and the
    // tails is spliceable; the split point is not.
    let mut previous = "founder".to_string();
    for g in 1..=8u32 {
        let id = format!("trunk{g}");
        tracker
            .add_reproduction(
                id.clone(),
                g,
                empty_genotype(),
                vec![previous.clone()],
                RelationType::Mutate,
            )
            .expect("recorded");
        previous = id;
    }
    let split = previous.clone();
    let mut tips = Vec::new();
    for side in ["left", "right"] {
        let mut prev = split.clone();
        for g in 9..=14u32 {
            let id = format!("{side}{g}");
            tracker
                .add_reproduction(
                    id.clone(),
                    g,
                    empty_genotype(),
                    vec![prev.clone()],
                    RelationType::Mutate,
                )
                .expect("recorded");
            prev = id;
        }
        tips.push(prev);
    }

    let (before_nodes, before_rels) = tracker.get_lineage_graph().expect("graph readable");
    let before = mrca(&before_nodes, &before_rels, &tips).expect("valid");
    assert_eq!(
        before
            .ancestors
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        vec![split.as_str()],
    );

    let report = tracker.compact(&tips).expect("compaction succeeded");
    assert!(
        report.nodes_removed() > 0,
        "nothing was compacted, so this test would pass trivially"
    );

    let (after_nodes, after_rels) = tracker.get_lineage_graph().expect("graph readable");
    let after = mrca(&after_nodes, &after_rels, &tips).expect("valid after compaction");

    assert_eq!(
        after
            .ancestors
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        vec![split.as_str()],
        "compaction moved the coalescence point"
    );
    assert_eq!(
        after.ancestors[0].generation, before.ancestors[0].generation,
        "generation survives splicing exactly; that is why it is the recency measure"
    );
    assert!(
        after.common_ancestors < before.common_ancestors,
        "the trunk above the MRCA should have been spliced away ({} -> {})",
        before.common_ancestors,
        after.common_ancestors
    );
}

// ---- scale ------------------------------------------------------------------------------------------

#[test]
fn a_lineage_deeper_than_the_stack_still_answers() {
    // A lineage is as deep as the run is long. A recursive walk would pass every test above and
    // overflow on the run that matters, so the depth here is chosen to be well past what a default
    // stack survives recursively.
    const DEPTH: u32 = 60_000;
    let mut nodes = Vec::with_capacity(DEPTH as usize + 3);
    let mut relations = Vec::with_capacity(DEPTH as usize + 2);
    nodes.push(node("root", 0));
    for g in 1..=DEPTH {
        let id = format!("n{g}");
        nodes.push(node(&id, g));
        let parent = if g == 1 {
            "root".to_string()
        } else {
            format!("n{}", g - 1)
        };
        relations.push(rel(&parent, &id, RelationType::Mutate));
    }
    // Two tips hanging off the very bottom, so the answer is the deepest node rather than the root.
    let last = format!("n{DEPTH}");
    nodes.push(node("tipA", DEPTH + 1));
    nodes.push(node("tipB", DEPTH + 1));
    relations.push(rel(&last, "tipA", RelationType::Mutate));
    relations.push(rel(&last, "tipB", RelationType::Mutate));

    let out = mrca(
        &nodes,
        &relations,
        &["tipA".to_string(), "tipB".to_string()],
    )
    .expect("valid");
    assert_eq!(out.ancestors.len(), 1);
    assert_eq!(out.ancestors[0].id, last);
    assert_eq!(out.ancestors[0].nearest_edges, 1);
    assert_eq!(out.ancestors[0].farthest_edges, 1);
    assert_eq!(out.common_ancestors as u32, DEPTH + 1, "the whole trunk");
}

// ---- against the real tracker -------------------------------------------------------------------------

#[test]
fn it_answers_for_a_lineage_the_tracker_actually_wrote() {
    let tracker = InMemoryLineageTracker::new();
    tracker
        .add_root("adam".into(), empty_genotype())
        .expect("root recorded");
    tracker
        .add_root("eve".into(), empty_genotype())
        .expect("root recorded");

    for child in ["kid-a", "kid-b"] {
        tracker
            .add_reproduction(
                child.into(),
                1,
                empty_genotype(),
                vec!["adam".into(), "eve".into()],
                RelationType::Crossover,
            )
            .expect("recorded");
    }

    let (nodes, relations) = tracker.get_lineage_graph().expect("graph readable");
    let out = mrca(
        &nodes,
        &relations,
        &["kid-a".to_string(), "kid-b".to_string()],
    )
    .expect("valid");

    // Two founders, two crossover children of both: the real engine's own way of producing a
    // question with two right answers.
    assert_eq!(
        out.ancestors
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        vec!["adam", "eve"],
    );
    assert!(out.is_ambiguous());
}
