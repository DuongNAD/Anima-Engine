//! OSS-071 — tskit-style `simplify()` for the lineage graph.
//!
//! # The problem
//!
//! [`InMemoryLineageTracker`](super::lineage::InMemoryLineageTracker) records every reproduction as
//! a node carrying a **full clone** of its `MorphologyGenotype`, and never removes anything. Memory
//! therefore grows with *everyone who ever lived*, not with who is alive — an unbounded line for a
//! long run.
//!
//! # What simplify does, in two steps
//!
//! Given the ids of the individuals that still matter (the **samples** — normally the living
//! population), it:
//!
//! 1. **Prunes** every node with no sample among its descendants. Nothing on a path between two
//!    retained nodes is touched, so ancestry among what is kept cannot change.
//! 2. **Compresses** unary paths: a retained non-sample node with exactly one parent and exactly one
//!    child carries no branching information, so it is spliced out.
//!
//! Step 1 alone does **not** bound memory. The ancestors of a living population still reach back to
//! genesis, so pruning removes extinct side branches but leaves every trunk intact. Step 2 is what
//! turns the trunk into a single edge, and it is why the retained set collapses to the *branch
//! points* — for a tree that is at most `2·samples − 1` nodes plus the surviving roots, which is the
//! O(alive) bound this task exists to reach.
//!
//! # The part that would have been silently wrong
//!
//! Compression cannot simply re-label the spliced edge. [`get_mutations_count`] in
//! `commands/evolution.rs` **counts `Mutate` edges along the ancestry path** to produce the mutation
//! figure the UI shows. Collapsing five `Mutate` edges into one `Mutate` edge keeps the type honest
//! and makes the count read 1 instead of 5 — a number that is still finite, still plausible, and
//! wrong by a factor of five.
//!
//! So a [`SimplifiedEdge`] carries `events` and `mutations`: how many reproductions the edge stands
//! for, and how many of them mutated. Compression is then lossless for the thing anyone actually
//! counts.
//!
//! [`get_mutations_count`]: https://github.com/DuongNAD/Anima-Engine
//!
//! # Why this returns its own types
//!
//! [`LineageRelation`](super::lineage::LineageRelation) is a **persisted** contract — it is
//! serialised into save state and into Neo4j. A compressed edge is not a reproduction event and has
//! no single `RelationType`, so widening that struct to hold path counts would push an
//! analysis-only concept into a storage format. `simplify` produces a separate value instead.

use super::lineage::{LineageNode, LineageRelation, RelationType};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// One edge of a simplified lineage. May stand for a **path** of reproductions rather than a single
/// event, which is why it carries counts instead of a [`RelationType`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimplifiedEdge {
    pub parent_id: String,
    pub child_id: String,
    /// Reproduction events on the path this edge replaced. `1` when nothing was compressed.
    pub events: u32,
    /// How many of those events were [`RelationType::Mutate`] — the figure a mutation count needs.
    pub mutations: u32,
    /// How many were [`RelationType::Crossover`]. A crossover child has two parents and is
    /// therefore never spliced out, so this is `0` or `1` in practice; it is carried rather than
    /// asserted because a future writer could record a crossover differently.
    pub crossovers: u32,
}

/// A lineage reduced to the ancestry of a chosen set of individuals.
///
/// Deliberately not `PartialEq`: that would need `LineageNode` (and through it
/// `MorphologyGenotype`) to compare structurally, which is a change to a **persisted** type for the
/// benefit of test ergonomics. Comparing `edges` and the retained ids says what a test means anyway;
/// comparing whole genotypes only adds noise to the failure output.
#[derive(Debug, Clone)]
pub struct SimplifiedLineage {
    /// Retained nodes, in id order. Genotypes come across untouched; the memory that is freed is
    /// the genotypes of the nodes that were **dropped**.
    pub nodes: Vec<LineageNode>,
    /// Retained edges, in (parent, child) order.
    pub edges: Vec<SimplifiedEdge>,
    /// Nodes removed for having no sample descendant.
    pub dropped_nodes: usize,
    /// Nodes spliced out of unary paths. These were ancestors of a sample — their ancestry is
    /// preserved through the edge that replaced them, but their genotype is gone.
    pub compressed_nodes: usize,
}

/// Why a lineage could not be simplified.
///
/// The three structural variants mirror [`NewickError`](super::newick::NewickError) on purpose:
/// both operations walk the same graph, so both refuse the same defects. They are separate enums
/// because they are separate operations, and folding them together would make either one's error
/// surface depend on the other's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimplifyError {
    /// Two nodes share an id, so ancestry keyed by id is ambiguous.
    DuplicateNode { id: String },
    /// A relation names an id no node declares.
    UnknownEndpoint { source: String, target: String },
    /// A sample id that is not in the graph. Silently ignoring it would prune the lineage against a
    /// smaller sample set than the caller asked for, and return a plausible answer to a question
    /// nobody asked.
    UnknownSample { id: String },
    /// The graph contains a cycle, so "ancestors of the samples" is not a finite set to keep.
    Cycle { node: String },
}

impl std::fmt::Display for SimplifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateNode { id } => write!(f, "two lineage nodes share the id {id:?}"),
            Self::UnknownEndpoint { source, target } => write!(
                f,
                "relation {source:?} -> {target:?} names an id that no node declares"
            ),
            Self::UnknownSample { id } => {
                write!(f, "sample {id:?} is not a node in this lineage")
            }
            Self::Cycle { node } => write!(
                f,
                "the lineage contains a cycle through {node:?}; ancestry is not well defined"
            ),
        }
    }
}

impl std::error::Error for SimplifyError {}

/// Running totals for an edge, kept separately from the ids while the graph is being rewritten.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct EdgeStat {
    events: u32,
    mutations: u32,
    crossovers: u32,
}

impl EdgeStat {
    fn of(kind: RelationType) -> Self {
        Self {
            events: 1,
            mutations: u32::from(kind == RelationType::Mutate),
            crossovers: u32::from(kind == RelationType::Crossover),
        }
    }

    /// Concatenate two paths. Saturating because a lineage long enough to overflow `u32` would
    /// wrap into a small, believable number — the failure mode this whole module is about.
    fn concat(self, next: Self) -> Self {
        Self {
            events: self.events.saturating_add(next.events),
            mutations: self.mutations.saturating_add(next.mutations),
            crossovers: self.crossovers.saturating_add(next.crossovers),
        }
    }
}

/// How much of the graph a caller is willing to lose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimplifyOptions {
    /// Splice out retained non-sample nodes that have exactly one parent and one child.
    ///
    /// This is the step that reaches the O(alive) bound — pruning alone keeps every trunk back to
    /// genesis. It is also the step that removes nodes an existing consumer can still see: the
    /// lineage graph the UI draws comes straight from the tracker, and `get_mutations_count` in
    /// `commands/evolution.rs` walks per-edge `RelationType`s that a compressed edge no longer has.
    ///
    /// So the **live tracker compacts with this off** ([`super::lineage::LineageTracker::compact`])
    /// and analysis turns it on. Turning it on for storage needs a per-node cumulative mutation
    /// count to be persisted first — see that method's documentation.
    pub compress_unary_paths: bool,
}

impl Default for SimplifyOptions {
    fn default() -> Self {
        Self {
            compress_unary_paths: true,
        }
    }
}

/// Reduce `nodes`/`relations` to the ancestry of `samples`, compressing unary paths.
///
/// `samples` are the individuals whose ancestry must survive — normally the living population. They
/// are never pruned and never compressed away, so a caller can always find them in the result.
///
/// Accepts exactly what
/// [`LineageTracker::get_lineage_graph`](super::lineage::LineageTracker::get_lineage_graph)
/// returns.
pub fn simplify(
    nodes: &[LineageNode],
    relations: &[LineageRelation],
    samples: &[String],
) -> Result<SimplifiedLineage, SimplifyError> {
    simplify_with(nodes, relations, samples, SimplifyOptions::default())
}

/// [`simplify`] with the compression step under the caller's control.
pub fn simplify_with(
    nodes: &[LineageNode],
    relations: &[LineageRelation],
    samples: &[String],
    options: SimplifyOptions,
) -> Result<SimplifiedLineage, SimplifyError> {
    // ---- index, rejecting duplicates ---------------------------------------------------------
    let mut index: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, node) in nodes.iter().enumerate() {
        if index.insert(node.id.as_str(), i).is_some() {
            return Err(SimplifyError::DuplicateNode {
                id: node.id.clone(),
            });
        }
    }

    // ---- adjacency ----------------------------------------------------------------------------
    // Parallel edges between the same pair are folded together here: two recorded relations between
    // one pair describe one ancestral connection, and keeping both would double-count events.
    let mut children: Vec<BTreeMap<usize, EdgeStat>> = vec![BTreeMap::new(); nodes.len()];
    let mut parents: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); nodes.len()];

    for rel in relations {
        let &parent =
            index
                .get(rel.source_id.as_str())
                .ok_or_else(|| SimplifyError::UnknownEndpoint {
                    source: rel.source_id.clone(),
                    target: rel.target_id.clone(),
                })?;
        let &child =
            index
                .get(rel.target_id.as_str())
                .ok_or_else(|| SimplifyError::UnknownEndpoint {
                    source: rel.source_id.clone(),
                    target: rel.target_id.clone(),
                })?;

        let stat = EdgeStat::of(rel.relation_type);
        children[parent]
            .entry(child)
            .and_modify(|existing| {
                existing.events = existing.events.max(stat.events);
                existing.mutations = existing.mutations.max(stat.mutations);
                existing.crossovers = existing.crossovers.max(stat.crossovers);
            })
            .or_insert(stat);
        parents[child].insert(parent);
    }

    // ---- cycle check --------------------------------------------------------------------------
    // Done on the WHOLE graph, before anything is pruned. A cycle in a branch that is about to be
    // dropped is still a defect in the recorded lineage, and reporting it only when it happens to
    // survive pruning would make the check depend on which individuals are alive.
    if let Some(node) = find_cycle(&children, &parents) {
        return Err(SimplifyError::Cycle {
            node: nodes[node].id.clone(),
        });
    }

    // ---- samples ------------------------------------------------------------------------------
    let mut is_sample = vec![false; nodes.len()];
    for id in samples {
        let &i = index
            .get(id.as_str())
            .ok_or_else(|| SimplifyError::UnknownSample { id: id.clone() })?;
        is_sample[i] = true;
    }

    // ---- step 1: retain the samples and their ancestors ----------------------------------------
    let mut retained = vec![false; nodes.len()];
    let mut queue: VecDeque<usize> = VecDeque::new();
    for (i, &sample) in is_sample.iter().enumerate() {
        if sample {
            retained[i] = true;
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        for &p in &parents[i] {
            if !retained[p] {
                retained[p] = true;
                queue.push_back(p);
            }
        }
    }
    let dropped_nodes = retained.iter().filter(|&&r| !r).count();

    // Restrict the adjacency to the retained set. Edges to or from a dropped node cannot connect
    // two retained nodes — a dropped node has no sample descendant, so nothing retained descends
    // through it — which is why this is a plain filter and not a re-linking pass.
    for (i, kids) in children.iter_mut().enumerate() {
        if !retained[i] {
            kids.clear();
        } else {
            kids.retain(|&c, _| retained[c]);
        }
    }
    for (i, ps) in parents.iter_mut().enumerate() {
        if !retained[i] {
            ps.clear();
        } else {
            ps.retain(|&p| retained[p]);
        }
    }

    // ---- step 2: splice out unary non-sample nodes ---------------------------------------------
    let mut compressed = vec![false; nodes.len()];
    let mut work: VecDeque<usize> = if options.compress_unary_paths {
        (0..nodes.len())
            .filter(|&i| retained[i] && !is_sample[i])
            .collect()
    } else {
        VecDeque::new()
    };

    while let Some(v) = work.pop_front() {
        if !retained[v] || compressed[v] || is_sample[v] {
            continue;
        }
        if parents[v].len() != 1 || children[v].len() != 1 {
            continue;
        }
        let p = *parents[v].iter().next().unwrap_or(&usize::MAX);
        let (c, _) = children[v]
            .iter()
            .next()
            .map(|(&c, &s)| (c, s))
            .unwrap_or((usize::MAX, EdgeStat::default()));
        if p == usize::MAX || c == usize::MAX {
            continue;
        }
        // Splicing p -> v -> p would create a self-loop; the cycle check above already rules this
        // out, so the guard is a belt on a graph that has been proven acyclic rather than a path
        // anything reaches.
        if p == c {
            continue;
        }
        // A diamond: p already reaches c another way. Merging would fold two distinct ancestral
        // paths into one edge and silently add their event counts together, so `v` stays and the
        // shape is preserved.
        if children[p].contains_key(&c) {
            continue;
        }

        let up = children[p].remove(&v).unwrap_or_default();
        let down = children[v].remove(&c).unwrap_or_default();
        parents[v].remove(&p);
        parents[c].remove(&v);

        children[p].insert(c, up.concat(down));
        parents[c].insert(p);

        retained[v] = false;
        compressed[v] = true;

        // Both endpoints changed degree, so either may have become compressible.
        work.push_back(p);
        work.push_back(c);
    }
    let compressed_nodes = compressed.iter().filter(|&&c| c).count();

    // ---- emit, in id order ---------------------------------------------------------------------
    let mut kept: Vec<usize> = (0..nodes.len()).filter(|&i| retained[i]).collect();
    kept.sort_by(|&a, &b| nodes[a].id.cmp(&nodes[b].id));

    let out_nodes: Vec<LineageNode> = kept.iter().map(|&i| nodes[i].clone()).collect();

    let mut out_edges: Vec<SimplifiedEdge> = Vec::new();
    for &p in &kept {
        for (&c, stat) in &children[p] {
            out_edges.push(SimplifiedEdge {
                parent_id: nodes[p].id.clone(),
                child_id: nodes[c].id.clone(),
                events: stat.events,
                mutations: stat.mutations,
                crossovers: stat.crossovers,
            });
        }
    }
    out_edges.sort_by(|a, b| {
        a.parent_id
            .cmp(&b.parent_id)
            .then_with(|| a.child_id.cmp(&b.child_id))
    });

    Ok(SimplifiedLineage {
        nodes: out_nodes,
        edges: out_edges,
        dropped_nodes,
        compressed_nodes,
    })
}

/// Kahn's algorithm. Returns a node still holding an incoming edge when the queue drains — one that
/// is on or below a cycle.
fn find_cycle(
    children: &[BTreeMap<usize, EdgeStat>],
    parents: &[BTreeSet<usize>],
) -> Option<usize> {
    let mut indegree: Vec<usize> = parents.iter().map(|p| p.len()).collect();
    let mut queue: VecDeque<usize> = (0..children.len()).filter(|&i| indegree[i] == 0).collect();
    let mut settled = 0usize;

    while let Some(i) = queue.pop_front() {
        settled += 1;
        for &c in children[i].keys() {
            indegree[c] -= 1;
            if indegree[c] == 0 {
                queue.push_back(c);
            }
        }
    }

    if settled == children.len() {
        return None;
    }
    // Walk parents from an unsettled node until an id repeats, so the reported node is ON the loop
    // rather than merely downstream of it.
    let start = indegree.iter().position(|&d| d > 0)?;
    let mut seen = BTreeSet::new();
    let mut cur = start;
    loop {
        if !seen.insert(cur) {
            return Some(cur);
        }
        // Follow a parent that is itself unsettled; a settled parent cannot be on the loop.
        match parents[cur].iter().copied().find(|&p| indegree[p] > 0) {
            Some(p) => cur = p,
            None => return Some(cur),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn edge_stat_counts_the_kind_it_was_built_from() {
        assert_eq!(EdgeStat::of(RelationType::Mutate).mutations, 1);
        assert_eq!(EdgeStat::of(RelationType::Clone).mutations, 0);
        assert_eq!(EdgeStat::of(RelationType::Crossover).crossovers, 1);
        assert_eq!(EdgeStat::of(RelationType::Clone).events, 1);
    }

    #[test]
    fn concatenating_paths_adds_their_counts() {
        let a = EdgeStat::of(RelationType::Mutate);
        let b = EdgeStat::of(RelationType::Clone);
        let joined = a.concat(b);
        assert_eq!(joined.events, 2);
        assert_eq!(joined.mutations, 1);
    }

    #[test]
    fn concatenation_saturates_rather_than_wrapping() {
        let big = EdgeStat {
            events: u32::MAX,
            mutations: u32::MAX,
            crossovers: 0,
        };
        let joined = big.concat(EdgeStat::of(RelationType::Mutate));
        assert_eq!(joined.events, u32::MAX, "wrapping would report a tiny path");
        assert_eq!(joined.mutations, u32::MAX);
    }

    #[test]
    fn a_lone_sample_keeps_itself() {
        let out = simplify(&[node("a", 0)], &[], &["a".to_string()]).expect("valid");
        assert_eq!(out.nodes.len(), 1);
        assert_eq!(out.dropped_nodes, 0);
        assert_eq!(out.compressed_nodes, 0);
    }

    #[test]
    fn an_unknown_sample_is_refused_rather_than_ignored() {
        match simplify(&[node("a", 0)], &[], &["ghost".to_string()]) {
            Err(SimplifyError::UnknownSample { id }) => assert_eq!(id, "ghost"),
            other => panic!("expected UnknownSample, got {other:?}"),
        }
    }

    #[test]
    fn a_cycle_is_refused_even_when_no_sample_descends_from_it() {
        // The cycle sits in a branch that pruning would have removed. Checking after pruning would
        // make the diagnosis depend on who happens to be alive.
        let nodes = vec![node("s", 0), node("x", 1), node("y", 2)];
        let relations = vec![
            rel("x", "y", RelationType::Clone),
            rel("y", "x", RelationType::Clone),
        ];
        match simplify(&nodes, &relations, &["s".to_string()]) {
            Err(SimplifyError::Cycle { node }) => {
                assert!(["x", "y"].contains(&node.as_str()), "got {node}")
            }
            other => panic!("expected Cycle, got {other:?}"),
        }
    }
}
