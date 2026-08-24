//! OSS-070 — Newick export for the lineage graph.
//!
//! # Why a format and not a library
//!
//! Newick is the interchange format the whole phylogenetics toolchain reads (`ape`, `ggtree`,
//! DendroPy, Biopython, iTOL). It is a *format*, so nothing is imported and the licences of those
//! projects do not enter into it — which matters, because Anima Engine is proprietary.
//!
//! The larger reason is not interop. A third-party parser is an **independent check on whether the
//! lineage graph is actually well-formed**. A cycle, an orphan edge or a duplicate id can sit in an
//! in-memory graph for a long time without anything failing; export refuses to serialise them, and a
//! foreign parser refuses to read a malformed tree. That turns a class of silent corruption into a
//! loud one.
//!
//! # The part that is not a detail: the graph is a DAG, Newick is a tree
//!
//! [`RelationType::Crossover`](super::lineage::RelationType) gives an individual **two** parents, so
//! [`LineageRelation`](super::lineage::LineageRelation) describes a directed acyclic *graph*. Newick
//! cannot express that; it has exactly one path from a node to the root.
//!
//! The choice here is to keep **one** parent per node and **count** the edges that could not be
//! represented, returned as [`NewickExport::dropped_parent_edges`]. Counting rather than dropping
//! silently is the whole point: an export that quietly halves a crossover-heavy lineage would still
//! parse, still draw, and still be wrong. A caller that sees a non-zero count knows the tree is a
//! *view* of the graph, not the graph.
//!
//! Which parent survives is decided by **smallest id**, not by input order. Input order differs
//! between the in-memory tracker (push order) and a Neo4j restore (query order), and an export that
//! changes shape depending on where the data came from is not reproducible.
//!
//! # What is deliberately not encoded
//!
//! `RelationType` itself. There is an NHX comment convention that could carry it, but plain Newick
//! is what every parser accepts, and the relation type is not needed for either purpose above. The
//! count of dropped edges is the one piece of crossover information that would otherwise be lost.

use super::lineage::{LineageNode, LineageRelation};
use std::collections::{BTreeMap, HashSet};

/// A lineage exported as Newick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewickExport {
    /// One Newick string per root, each already terminated with `;`.
    ///
    /// A lineage has as many roots as `add_root` was called, and genesis calls it once per founder,
    /// so a forest is the normal case rather than an edge case. Every phylogenetics reader takes a
    /// multi-tree file, one tree per line — see [`Self::to_file_contents`].
    pub trees: Vec<String>,
    /// Parent edges that could not be represented, because Newick is a tree format and crossover
    /// gives a node more than one parent. Non-zero means `trees` is a **view** of the lineage.
    pub dropped_parent_edges: usize,
    /// Nodes that had no parent edge.
    pub roots: usize,
}

impl NewickExport {
    /// The forest as a file: one tree per line. This is what `ape::read.tree` and
    /// `dendropy.TreeList.get` expect from a multi-tree file.
    pub fn to_file_contents(&self) -> String {
        let mut out = String::new();
        for tree in &self.trees {
            out.push_str(tree);
            out.push('\n');
        }
        out
    }
}

/// Why a lineage could not be written as Newick.
///
/// Every variant is a defect in the graph, not a limitation of the format — the one real limitation
/// (multiple parents) is counted in [`NewickExport::dropped_parent_edges`] instead of failing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewickError {
    /// Two nodes share an id. Ancestry is keyed by id, so this makes the graph ambiguous rather
    /// than merely untidy.
    DuplicateNode { id: String },
    /// An edge names an id that no node declares. Left alone this silently detaches a subtree.
    UnknownEndpoint { source: String, target: String },
    /// Following parents from this node returns to it. A self-relation counts.
    Cycle { node: String },
    /// A child is recorded as belonging to an EARLIER generation than its parent. Not a formatting
    /// problem: it means the generation counter and the parent edges disagree about time, and the
    /// branch length Newick asks for cannot be computed without inventing a number.
    GenerationInversion {
        parent: String,
        child: String,
        parent_generation: u32,
        child_generation: u32,
    },
}

impl std::fmt::Display for NewickError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateNode { id } => {
                write!(f, "two lineage nodes share the id {id:?}")
            }
            Self::UnknownEndpoint { source, target } => write!(
                f,
                "relation {source:?} -> {target:?} names an id that no node declares"
            ),
            Self::Cycle { node } => write!(
                f,
                "following parents from {node:?} returns to it; a lineage cannot contain a cycle"
            ),
            Self::GenerationInversion {
                parent,
                child,
                parent_generation,
                child_generation,
            } => write!(
                f,
                "child {child:?} is generation {child_generation} but its parent {parent:?} is \
                 generation {parent_generation}; the generation counter and the parent edges \
                 disagree"
            ),
        }
    }
}

impl std::error::Error for NewickError {}

/// Characters Newick gives a meaning to. A label containing any of them, or leading/trailing
/// whitespace, has to be quoted or the tree reads back with a different shape.
const RESERVED: [char; 8] = ['(', ')', ',', ':', ';', '\'', '[', ']'];

/// Render one label, quoting only when the content forces it.
///
/// Unquoted Newick treats `_` as a space, so a label containing `_` must be quoted too or it comes
/// back different. Ids here are UUIDs today and would survive unquoted, but restore and migration
/// carry whatever ids they were given, and a label is exactly the place where an unexpected string
/// stops being cosmetic.
fn label(raw: &str) -> String {
    let needs_quoting = raw.is_empty()
        || raw.contains(RESERVED)
        || raw.contains(char::is_whitespace)
        || raw.contains('_');
    if !needs_quoting {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len() + 2);
    out.push('\'');
    for c in raw.chars() {
        // A single quote inside a quoted label is escaped by doubling it.
        if c == '\'' {
            out.push('\'');
        }
        out.push(c);
    }
    out.push('\'');
    out
}

/// Write `nodes`/`relations` as a Newick forest.
///
/// Accepts exactly what [`LineageTracker::get_lineage_graph`](super::lineage::LineageTracker::get_lineage_graph)
/// returns, so it works against the in-memory tracker and a Neo4j restore alike.
///
/// Branch lengths are the **generation delta** across each edge, which is real information the
/// format carries and which lets a reader compute depth. It is also why
/// [`NewickError::GenerationInversion`] exists: a negative delta is not a number to clamp, it is two
/// sources disagreeing.
pub fn to_newick(
    nodes: &[LineageNode],
    relations: &[LineageRelation],
) -> Result<NewickExport, NewickError> {
    to_newick_from(
        nodes,
        relations
            .iter()
            .map(|r| (r.source_id.as_str(), r.target_id.as_str())),
    )
}

/// Write a forest from bare `(parent, child)` pairs.
///
/// Exists because a **simplified** lineage
/// ([`simplify`](super::simplify::simplify)) has no `RelationType` to offer: a compressed edge
/// stands for a path of reproductions rather than one event, and inventing a type for it is exactly
/// the fabrication that module refuses to make. Newick does not encode the type anyway, so taking
/// pairs lets a simplified lineage be exported without anyone having to make one up.
///
/// [`to_newick`] is a thin wrapper over this, so both paths share one implementation and cannot
/// drift.
pub fn to_newick_from<'a, I>(nodes: &[LineageNode], edges: I) -> Result<NewickExport, NewickError>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    // ---- index the nodes, rejecting duplicate ids ------------------------------------------
    let mut index: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, node) in nodes.iter().enumerate() {
        if index.insert(node.id.as_str(), i).is_some() {
            return Err(NewickError::DuplicateNode {
                id: node.id.clone(),
            });
        }
    }

    // ---- choose one parent per node ---------------------------------------------------------
    // Smallest id wins, so the result does not depend on the order relations arrived in. Every
    // further candidate is counted, not discarded quietly.
    let mut parent_of: Vec<Option<usize>> = vec![None; nodes.len()];
    let mut dropped_parent_edges = 0usize;

    for (source_id, target_id) in edges {
        let &parent = index
            .get(source_id)
            .ok_or_else(|| NewickError::UnknownEndpoint {
                source: source_id.to_string(),
                target: target_id.to_string(),
            })?;
        let &child = index
            .get(target_id)
            .ok_or_else(|| NewickError::UnknownEndpoint {
                source: source_id.to_string(),
                target: target_id.to_string(),
            })?;

        match parent_of[child] {
            None => parent_of[child] = Some(parent),
            Some(existing) => {
                dropped_parent_edges += 1;
                if nodes[parent].id < nodes[existing].id {
                    parent_of[child] = Some(parent);
                }
            }
        }
    }

    // ---- children lists, sorted by id so output is deterministic ----------------------------
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (child, slot) in parent_of.iter().enumerate() {
        if let Some(parent) = *slot {
            children[parent].push(child);
        }
    }
    for kids in children.iter_mut() {
        kids.sort_by(|&a, &b| nodes[a].id.cmp(&nodes[b].id));
    }

    // ---- roots, in id order ------------------------------------------------------------------
    let mut roots: Vec<usize> = (0..nodes.len())
        .filter(|&i| parent_of[i].is_none())
        .collect();
    roots.sort_by(|&a, &b| nodes[a].id.cmp(&nodes[b].id));

    // ---- emit ---------------------------------------------------------------------------------
    let mut visited = vec![false; nodes.len()];
    let mut trees = Vec::with_capacity(roots.len());
    for &root in &roots {
        trees.push(emit_tree(root, &children, &parent_of, nodes, &mut visited));
    }

    // ---- cycle check, BEFORE the generation check ------------------------------------------
    //
    // Every node has at most one parent now, so a node the roots cannot reach is a node whose
    // ancestor chain loops. Reachability IS cycle detection here, which is why there is no separate
    // colouring pass.
    //
    // The ordering is not cosmetic. Generations cannot increase monotonically all the way around a
    // loop, so **every cycle also contains a generation inversion** — and checking generations first
    // reports the symptom instead of the cause, sending whoever reads the error to a node whose own
    // record is fine. `a_cycle_is_refused` and `a_cycle_beside_a_healthy_tree_is_still_refused`
    // caught exactly that; both failed with a GenerationInversion until this moved.
    if let Some(unreached) = visited.iter().position(|&v| !v) {
        return Err(NewickError::Cycle {
            node: nodes[first_node_on_cycle_from(unreached, &parent_of)]
                .id
                .clone(),
        });
    }

    // ---- generation sanity, on the edges that survived --------------------------------------
    //
    // Reached only once the graph is known to be acyclic, so an inversion here is a genuine
    // disagreement between the generation counter and the parent edges rather than a side effect of
    // a loop. `emit_tree` uses `saturating_sub`, so a tree was already built above with a clamped
    // branch length; it is discarded rather than returned, because a clamped length is precisely the
    // valid-looking output this check exists to withhold.
    for (child, slot) in parent_of.iter().enumerate() {
        if let Some(parent) = *slot {
            if nodes[child].generation < nodes[parent].generation {
                return Err(NewickError::GenerationInversion {
                    parent: nodes[parent].id.clone(),
                    child: nodes[child].id.clone(),
                    parent_generation: nodes[parent].generation,
                    child_generation: nodes[child].generation,
                });
            }
        }
    }

    Ok(NewickExport {
        trees,
        dropped_parent_edges,
        roots: roots.len(),
    })
}

/// Walk parents from `start` until an id repeats, and return that repeated node — the one actually
/// on the loop, rather than whichever unreached node happened to be found first. Reporting a node
/// that merely *hangs off* a cycle sends the reader to the wrong place.
fn first_node_on_cycle_from(start: usize, parent_of: &[Option<usize>]) -> usize {
    let mut seen = HashSet::new();
    let mut cur = start;
    loop {
        if !seen.insert(cur) {
            return cur;
        }
        match parent_of[cur] {
            Some(p) => cur = p,
            // Unreachable in practice: a chain that ends at a parentless node would have been
            // walked from that root. Returning `cur` keeps this total rather than panicking.
            None => return cur,
        }
    }
}

/// Emit one tree, iteratively.
///
/// Deliberately not recursive: a lineage is as deep as the run is long, and `lineage_stress_tests`
/// already builds deep chains. A recursive emitter would work on every test that fits in a screen
/// and overflow the stack on the run that matters.
fn emit_tree(
    root: usize,
    children: &[Vec<usize>],
    parent_of: &[Option<usize>],
    nodes: &[LineageNode],
    visited: &mut [bool],
) -> String {
    let mut out = String::new();
    // (node, index of the next child to descend into)
    let mut stack: Vec<(usize, usize)> = vec![(root, 0)];

    while let Some(&(idx, pos)) = stack.last() {
        visited[idx] = true;
        let kids = &children[idx];

        if pos == 0 && !kids.is_empty() {
            out.push('(');
        }

        if pos < kids.len() {
            if pos > 0 {
                out.push(',');
            }
            if let Some(top) = stack.last_mut() {
                top.1 = pos + 1;
            }
            stack.push((kids[pos], 0));
            continue;
        }

        if !kids.is_empty() {
            out.push(')');
        }
        out.push_str(&label(&nodes[idx].id));
        if let Some(parent) = parent_of[idx] {
            let delta = nodes[idx]
                .generation
                .saturating_sub(nodes[parent].generation);
            out.push(':');
            out.push_str(&delta.to_string());
        }
        stack.pop();
    }

    out.push(';');
    out
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

    fn edge(source: &str, target: &str) -> LineageRelation {
        LineageRelation {
            source_id: source.to_string(),
            target_id: target.to_string(),
            relation_type: RelationType::Mutate,
            path_events: None,
        }
    }

    #[test]
    fn a_bare_label_is_left_unquoted() {
        assert_eq!(label("abc-123"), "abc-123");
    }

    #[test]
    fn a_label_with_a_reserved_character_is_quoted() {
        assert_eq!(label("a,b"), "'a,b'");
        assert_eq!(label("(x)"), "'(x)'");
    }

    #[test]
    fn an_underscore_is_quoted_because_unquoted_newick_reads_it_as_a_space() {
        assert_eq!(label("a_b"), "'a_b'");
    }

    #[test]
    fn an_inner_single_quote_is_doubled() {
        assert_eq!(label("it's"), "'it''s'");
    }

    #[test]
    fn an_empty_label_is_quoted_rather_than_emitted_as_nothing() {
        assert_eq!(label(""), "''");
    }

    #[test]
    fn a_single_node_is_a_one_leaf_tree() {
        let out = to_newick(&[node("a", 0)], &[]).expect("valid");
        assert_eq!(out.trees, vec!["a;".to_string()]);
        assert_eq!(out.roots, 1);
        assert_eq!(out.dropped_parent_edges, 0);
    }

    #[test]
    fn children_are_emitted_in_id_order_whatever_order_the_edges_arrived_in() {
        let nodes = vec![node("root", 0), node("b", 1), node("a", 1)];
        let forward = to_newick(&nodes, &[edge("root", "b"), edge("root", "a")]).expect("valid");
        let reversed = to_newick(&nodes, &[edge("root", "a"), edge("root", "b")]).expect("valid");
        assert_eq!(forward.trees, vec!["(a:1,b:1)root;".to_string()]);
        assert_eq!(forward, reversed);
    }

    #[test]
    fn branch_lengths_are_the_generation_delta() {
        let nodes = vec![node("r", 0), node("c", 3)];
        let out = to_newick(&nodes, &[edge("r", "c")]).expect("valid");
        assert_eq!(out.trees, vec!["(c:3)r;".to_string()]);
    }
}
