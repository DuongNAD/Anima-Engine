//! OSS-072 — the most recent common ancestor of a set of individuals.
//!
//! # Why this is not a one-liner
//!
//! In a rooted tree the MRCA of two individuals is a single node, and every phylogenetics library
//! returns one. This lineage is **not a tree**: [`RelationType::Crossover`](super::lineage::RelationType)
//! gives an individual two parents, so [`LineageRelation`](super::lineage::LineageRelation)
//! describes a directed acyclic *graph*, and a DAG can have several common ancestors that are
//! incomparable — none of them an ancestor of another, so none of them "more recent" than the rest.
//!
//! The smallest case is one this engine produces on purpose — two siblings, each a crossover of the
//! same pair:
//!
//! ```text
//!       r
//!      / \
//!     a   b
//!     |\ /|
//!     | X |
//!     |/ \|
//!     x   y
//! ```
//!
//! `a` and `b` are both common ancestors of `x` and `y`, and neither descends from the other. `r` is
//! a common ancestor too, but it is *less* recent than both. So the honest answer to
//! `mrca(x, y)` here is the **set** `{a, b}`, and that is what [`mrca`] returns.
//!
//! Returning one of them would be the failure mode this whole subsystem keeps running into: a
//! finite, plausible, single answer to a question that does not have one. A caller that gets two
//! back knows its lineage branched and rejoined; a caller handed `a` alone would never find out.
//!
//! # The three conventions, stated rather than implied
//!
//! **1. Ancestry is reflexive: a node is its own ancestor.** So `mrca(x, x) == {x}`, and if `x` is
//! an ancestor of `y` then `mrca(x, y) == {x}`. This matches what `dendropy` and `ape` do, and the
//! alternative is worse: under strict ancestry `mrca(x, x)` would be `x`'s parent, which reads as a
//! bug at every call site.
//!
//! **2. No common ancestor is an answer, not an error.** [`Mrca::ancestors`] comes back empty.
//! Genesis calls `add_root` once per founder, so a lineage here is normally a **forest** and two
//! individuals from different founding lines genuinely never coalesce. Returning `Err` would make
//! the ordinary case an error path and push every caller into treating "unrelated" as a failure.
//!
//! **3. An empty query is refused.** Every node is vacuously a common ancestor of no-one, so the
//! mathematically correct answer is "the whole graph" — which is useless and looks like a result.
//! [`MrcaError::NoIndividuals`] says what happened instead.
//!
//! # Which one is "most recent"
//!
//! Strictly speaking, nothing is: every entry in [`Mrca::ancestors`] is *incomparable* with every
//! other, which is what "maximal" means, so no ordering among them is more correct than another. The
//! order is a presentation, and the set is the answer.
//!
//! Given that, the order is **generation descending**, then id. Generation is the best available
//! clock because it survives compaction exactly — splicing a unary path removes nodes but never
//! rewrites [`LineageNode::generation`](super::lineage::LineageNode). It is not verified *here*:
//! [`to_newick`](super::newick::to_newick) is what refuses a graph whose generations disagree with
//! its edges, and this function does not run that check, because a disagreement about generations
//! does not make the ancestor set wrong — only the order it is presented in.
//!
//! [`MrcaAncestor::nearest_edges`] and [`MrcaAncestor::farthest_edges`] are offered as a secondary
//! tiebreak with a caveat that has to be read: **they count graph edges, not reproduction events.**
//! On a compacted graph one edge can stand for a whole spliced path — that is exactly what
//! [`LineageRelation::path_events`](super::lineage::LineageRelation) records — so an edge count
//! there is a lower bound on how many reproductions actually separate the two individuals.
//! Generation deltas stay exact; edge counts do not.
//!
//! # Cost
//!
//! `O(k · (N + E))` for `k` individuals over `N` nodes and `E` edges: one upward breadth-first walk
//! per individual, then one linear pass to discard the common ancestors that are not maximal. This
//! is a query, not a tick-path function — nothing here belongs inside the zero-allocation hot loop.

use super::lineage::{LineageNode, LineageRelation};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// One maximal common ancestor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MrcaAncestor {
    pub id: String,
    /// The recency measure. Higher is more recent, and this is the field
    /// [`Mrca::ancestors`] is sorted on.
    pub generation: u32,
    /// Fewest edges from this ancestor down to whichever queried individual is closest — shortest
    /// path per individual, then the minimum across them.
    ///
    /// **Edges, not reproduction events.** See the module docs: a compacted edge summarises a path.
    pub nearest_edges: u32,
    /// The same measure for whichever queried individual is farthest.
    pub farthest_edges: u32,
}

/// The answer to "where did these individuals last share an ancestor".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mrca {
    /// The maximal common ancestors, most recent first (generation descending, then id ascending).
    ///
    /// Empty when the individuals share no ancestor at all, which is ordinary in a founder forest.
    /// More than one entry means the lineage branched and rejoined — see the module docs.
    pub ancestors: Vec<MrcaAncestor>,
    /// How many nodes are ancestors of **every** queried individual, maximal or not.
    ///
    /// Always `>= ancestors.len()`. The difference is the shared trunk above the coalescence point,
    /// and a caller can use it to tell "these two just met" from "these two share a long history".
    pub common_ancestors: usize,
}

impl Mrca {
    /// Whether the DAG gave more than one incomparable answer.
    ///
    /// A caller that renders a single "common ancestor" field should check this rather than take
    /// `ancestors[0]`, which is the most recent but not the only one.
    pub fn is_ambiguous(&self) -> bool {
        self.ancestors.len() > 1
    }
}

/// Why an MRCA could not be computed.
///
/// The four structural variants mirror [`SimplifyError`](super::simplify::SimplifyError): all three
/// of these operations walk the same graph, so all three refuse the same defects. They stay separate
/// enums for the reason `simplify` already records — folding them together would make one
/// operation's error surface depend on another's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MrcaError {
    /// Two nodes share an id, so ancestry keyed by id is ambiguous.
    DuplicateNode { id: String },
    /// A relation names an id no node declares.
    UnknownEndpoint { source: String, target: String },
    /// A queried id that is not in the graph. Skipping it would answer for a smaller set than the
    /// caller asked about and return a plausible number for the wrong question.
    UnknownIndividual { id: String },
    /// The graph contains a cycle, so "is an ancestor of" is not an order and "most recent" has no
    /// meaning.
    Cycle { node: String },
    /// No individuals were named. Every node is vacuously a common ancestor of the empty set, so
    /// the correct answer is the whole graph — which no caller wants and which looks like a result.
    NoIndividuals,
}

impl std::fmt::Display for MrcaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateNode { id } => write!(f, "two lineage nodes share the id {id:?}"),
            Self::UnknownEndpoint { source, target } => write!(
                f,
                "relation {source:?} -> {target:?} names an id that no node declares"
            ),
            Self::UnknownIndividual { id } => {
                write!(f, "individual {id:?} is not a node in this lineage")
            }
            Self::Cycle { node } => write!(
                f,
                "the lineage contains a cycle through {node:?}; ancestry is not well defined"
            ),
            Self::NoIndividuals => write!(
                f,
                "no individuals were named; the most recent common ancestor of an empty set is \
                 every node in the graph, which is not an answer"
            ),
        }
    }
}

impl std::error::Error for MrcaError {}

/// The most recent common ancestors of `individuals`.
///
/// Accepts exactly what
/// [`LineageTracker::get_lineage_graph`](super::lineage::LineageTracker::get_lineage_graph)
/// returns, so it works against the in-memory tracker and a Neo4j restore alike.
///
/// Read the module docs before using the result: the answer is a **set**, ancestry is **reflexive**,
/// and an empty set means "no shared ancestor", which is normal in a founder forest.
///
/// Duplicate ids in `individuals` are collapsed, so asking about `[x, x]` is asking about `[x]`.
pub fn mrca(
    nodes: &[LineageNode],
    relations: &[LineageRelation],
    individuals: &[String],
) -> Result<Mrca, MrcaError> {
    // ---- index, rejecting duplicates ---------------------------------------------------------
    let mut index: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, node) in nodes.iter().enumerate() {
        if index.insert(node.id.as_str(), i).is_some() {
            return Err(MrcaError::DuplicateNode {
                id: node.id.clone(),
            });
        }
    }

    // ---- adjacency ----------------------------------------------------------------------------
    // Sets rather than lists: a parallel edge between the same pair is one ancestral connection, and
    // counting it twice would change the in-degree that the cycle check below reads.
    let mut children: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); nodes.len()];
    let mut parents: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); nodes.len()];

    for rel in relations {
        let &parent =
            index
                .get(rel.source_id.as_str())
                .ok_or_else(|| MrcaError::UnknownEndpoint {
                    source: rel.source_id.clone(),
                    target: rel.target_id.clone(),
                })?;
        let &child =
            index
                .get(rel.target_id.as_str())
                .ok_or_else(|| MrcaError::UnknownEndpoint {
                    source: rel.source_id.clone(),
                    target: rel.target_id.clone(),
                })?;
        children[parent].insert(child);
        parents[child].insert(parent);
    }

    // ---- cycle check, on the WHOLE graph ------------------------------------------------------
    //
    // Before the individuals are even resolved, for the reason `simplify` gives: a cycle in a branch
    // nobody asked about is still a defect in the recorded lineage, and checking only the queried
    // ancestry would make the diagnosis depend on who you happened to ask about. It is also load
    // bearing here rather than merely tidy — "maximal common ancestor" is defined against a partial
    // order, and a cycle means there is no order to be maximal in.
    let topo = topological_order(&children, &parents).map_err(|node| MrcaError::Cycle {
        node: nodes[node].id.clone(),
    })?;

    // ---- resolve the individuals ---------------------------------------------------------------
    if individuals.is_empty() {
        return Err(MrcaError::NoIndividuals);
    }
    let mut queried: BTreeSet<usize> = BTreeSet::new();
    for id in individuals {
        let &i = index
            .get(id.as_str())
            .ok_or_else(|| MrcaError::UnknownIndividual { id: id.clone() })?;
        queried.insert(i);
    }
    let wanted = queried.len();

    // ---- one upward walk per individual --------------------------------------------------------
    //
    // Breadth-first, so `depth` is the shortest ancestral path. `hits` counts how many *distinct*
    // individuals reached each node — a BFS visits a node at most once, so a node reached by all of
    // them ends at exactly `wanted`.
    //
    // Iterative on purpose. A lineage is as deep as the run is long, and `lineage_stress_tests`
    // already builds chains deep enough that a recursive walk would overflow the stack on the run
    // that matters rather than on any test that fits on a screen.
    let mut hits: Vec<u32> = vec![0; nodes.len()];
    let mut nearest: Vec<u32> = vec![u32::MAX; nodes.len()];
    let mut farthest: Vec<u32> = vec![0; nodes.len()];
    let mut seen: Vec<bool> = vec![false; nodes.len()];
    let mut queue: VecDeque<(usize, u32)> = VecDeque::new();

    for &start in &queried {
        seen.iter_mut().for_each(|s| *s = false);
        queue.clear();
        // Depth 0, because ancestry is reflexive — see convention 1 in the module docs.
        seen[start] = true;
        queue.push_back((start, 0));

        while let Some((v, depth)) = queue.pop_front() {
            hits[v] += 1;
            nearest[v] = nearest[v].min(depth);
            farthest[v] = farthest[v].max(depth);
            for &p in &parents[v] {
                if !seen[p] {
                    seen[p] = true;
                    queue.push_back((p, depth.saturating_add(1)));
                }
            }
        }
    }

    let is_common: Vec<bool> = hits.iter().map(|&h| h as usize == wanted).collect();
    let common_ancestors = is_common.iter().filter(|&&c| c).count();

    // ---- discard the common ancestors that are not maximal --------------------------------------
    //
    // A common ancestor `c` is *not* the most recent one when some other common ancestor sits below
    // it, i.e. when descending from `c` reaches the common set again. `reaches_common[v]` says
    // exactly that, and computing it in reverse topological order settles every child before its
    // parent, so the whole thing is one linear pass instead of a reachability query per pair.
    //
    // Restricting the walk to the common set would also be correct — any node between two common
    // ancestors is itself common — but the whole-graph form needs no such argument to be right.
    let mut reaches_common: Vec<bool> = vec![false; nodes.len()];
    for &v in topo.iter().rev() {
        reaches_common[v] = children[v]
            .iter()
            .any(|&c| is_common[c] || reaches_common[c]);
    }

    let mut ancestors: Vec<MrcaAncestor> = (0..nodes.len())
        .filter(|&i| is_common[i] && !reaches_common[i])
        .map(|i| MrcaAncestor {
            id: nodes[i].id.clone(),
            generation: nodes[i].generation,
            nearest_edges: nearest[i],
            farthest_edges: farthest[i],
        })
        .collect();

    // Most recent first. The id tiebreak is what makes the result independent of the order the
    // relations arrived in — which differs between the in-memory tracker (push order) and a Neo4j
    // restore (query order), and a query whose answer depends on that is not reproducible.
    ancestors.sort_by(|a, b| {
        b.generation
            .cmp(&a.generation)
            .then_with(|| a.id.cmp(&b.id))
    });

    Ok(Mrca {
        ancestors,
        common_ancestors,
    })
}

/// Kahn's algorithm: a parents-before-children ordering, or the id of a node on a cycle.
///
/// Written here rather than shared with [`simplify`](super::simplify) because that module's copy is
/// coupled to its edge-statistics adjacency, and widening it to serve both would put an
/// analysis-only concern into the type compaction depends on.
fn topological_order(
    children: &[BTreeSet<usize>],
    parents: &[BTreeSet<usize>],
) -> Result<Vec<usize>, usize> {
    let mut indegree: Vec<usize> = parents.iter().map(|p| p.len()).collect();
    let mut queue: VecDeque<usize> = (0..children.len()).filter(|&i| indegree[i] == 0).collect();
    let mut order: Vec<usize> = Vec::with_capacity(children.len());

    while let Some(i) = queue.pop_front() {
        order.push(i);
        for &c in &children[i] {
            indegree[c] -= 1;
            if indegree[c] == 0 {
                queue.push_back(c);
            }
        }
    }

    if order.len() == children.len() {
        return Ok(order);
    }

    // Walk parents from an unsettled node until an id repeats, so the node reported is ON the loop
    // rather than merely hanging below it — the same distinction `newick` makes, and for the same
    // reason: naming a downstream node sends whoever reads the error to a record that is fine.
    let start = indegree.iter().position(|&d| d > 0).unwrap_or_default();
    let mut visited = BTreeSet::new();
    let mut cur = start;
    loop {
        if !visited.insert(cur) {
            return Err(cur);
        }
        match parents[cur].iter().copied().find(|&p| indegree[p] > 0) {
            Some(p) => cur = p,
            None => return Err(cur),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evolution::lineage::RelationType;

    fn node(id: &str, generation: u32) -> LineageNode {
        LineageNode {
            id: id.to_string(),
            generation,
            genotype: None,
            cumulative_mutations: None,
        }
    }

    fn rel(source: &str, target: &str) -> LineageRelation {
        LineageRelation {
            source_id: source.to_string(),
            target_id: target.to_string(),
            relation_type: RelationType::Mutate,
            path_events: None,
        }
    }

    fn ids(m: &Mrca) -> Vec<&str> {
        m.ancestors.iter().map(|a| a.id.as_str()).collect()
    }

    #[test]
    fn an_individual_is_its_own_most_recent_common_ancestor() {
        let out = mrca(&[node("a", 0)], &[], &["a".to_string()]).expect("valid");
        assert_eq!(ids(&out), vec!["a"], "ancestry is reflexive by convention");
    }

    #[test]
    fn asking_twice_about_one_individual_is_asking_once() {
        let nodes = vec![node("r", 0), node("x", 1)];
        let relations = vec![rel("r", "x")];
        let once = mrca(&nodes, &relations, &["x".to_string()]).expect("valid");
        let twice = mrca(&nodes, &relations, &["x".to_string(), "x".to_string()]).expect("valid");
        assert_eq!(once, twice);
    }

    #[test]
    fn an_ancestor_paired_with_its_descendant_is_the_answer() {
        let nodes = vec![node("r", 0), node("m", 1), node("x", 2)];
        let relations = vec![rel("r", "m"), rel("m", "x")];
        let out = mrca(&nodes, &relations, &["m".to_string(), "x".to_string()]).expect("valid");
        assert_eq!(ids(&out), vec!["m"]);
    }

    #[test]
    fn an_empty_query_is_refused_rather_than_answered_with_the_whole_graph() {
        assert_eq!(
            mrca(&[node("a", 0)], &[], &[]),
            Err(MrcaError::NoIndividuals)
        );
    }

    #[test]
    fn an_unknown_individual_is_refused_rather_than_ignored() {
        match mrca(&[node("a", 0)], &[], &["ghost".to_string()]) {
            Err(MrcaError::UnknownIndividual { id }) => assert_eq!(id, "ghost"),
            other => panic!("expected UnknownIndividual, got {other:?}"),
        }
    }

    #[test]
    fn a_cycle_is_refused_even_when_it_is_nowhere_near_the_query() {
        let nodes = vec![node("s", 0), node("x", 1), node("y", 2)];
        let relations = vec![rel("x", "y"), rel("y", "x")];
        match mrca(&nodes, &relations, &["s".to_string()]) {
            Err(MrcaError::Cycle { node }) => {
                assert!(["x", "y"].contains(&node.as_str()), "got {node}")
            }
            other => panic!("expected Cycle, got {other:?}"),
        }
    }

    #[test]
    fn edge_counts_are_the_shortest_path_to_the_nearest_and_farthest_of_the_query() {
        //   r -> a -> b -> deep
        //   r -> shallow
        let nodes = vec![
            node("r", 0),
            node("a", 1),
            node("b", 2),
            node("deep", 3),
            node("shallow", 1),
        ];
        let relations = vec![
            rel("r", "a"),
            rel("a", "b"),
            rel("b", "deep"),
            rel("r", "shallow"),
        ];
        let out = mrca(
            &nodes,
            &relations,
            &["deep".to_string(), "shallow".to_string()],
        )
        .expect("valid");
        assert_eq!(ids(&out), vec!["r"]);
        assert_eq!(
            out.ancestors[0].nearest_edges, 1,
            "shallow is one edge away"
        );
        assert_eq!(out.ancestors[0].farthest_edges, 3, "deep is three");
    }
}
