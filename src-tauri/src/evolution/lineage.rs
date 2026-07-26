use crate::evolution::genotype::MorphologyGenotype;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageNode {
    pub id: String,
    pub generation: u32,
    pub genotype: Option<MorphologyGenotype>,
    /// Mutation events on the path from this node's root, inclusive of its own arrival.
    ///
    /// # Why `Option<u32>` and not `u32`
    ///
    /// `LineageNode` is serialized inside `SavedSimulationState`. A bare `u32` would default to `0`
    /// on every save written before this field existed, and `0` does not read as "unknown" — it
    /// reads as **"this lineage never mutated"**. That is finite, plausible, and wrong, which is the
    /// worst shape a wrong number can have. `None` means *not recorded; derive it from the edges as
    /// before*, and [`cumulative_mutations_from_edges`] is that derivation.
    ///
    /// # Why it has to exist at all
    ///
    /// Compaction reaches its O(alive) bound only by splicing out unary paths, and a spliced edge
    /// represents a *path* rather than an event — so it cannot carry the per-edge [`RelationType`]
    /// that the old count walked. Storing the total per node makes the walk unnecessary instead of
    /// approximating it: five `Mutate` edges compressed into one still read as five.
    #[serde(default)]
    pub cumulative_mutations: Option<u32>,
}

/// Cumulative mutation count per node id, derived from edges alone.
///
/// This is the **one** implementation of the rule. It was previously inlined in
/// `commands::evolution::get_lineage_graph`; keeping a second copy next to the stored field is how
/// a stored value and a derived value drift into disagreeing, and the whole point of the field is
/// that the two agree.
///
/// The rule: a node's count is the greatest count among its parents, plus one when any edge into
/// it is a [`RelationType::Mutate`]. `max` rather than a sum because the count describes *one*
/// line of descent — a crossover child inherits the longer history, it does not add the two.
/// Turn a simplify plan's edges back into storable [`LineageRelation`]s.
///
/// Extracted from `compact` so the impossible case below is reachable from a test. It used to be
/// inline, and it handled that case by inventing an observation:
///
/// ```text
/// original_type.get(&key).copied().unwrap_or(RelationType::Clone)
/// ```
///
/// An edge with `events <= 1` is a single original relation carried through unchanged, so its
/// `(parent, child)` pair is necessarily in `relations`. If it is not, the plan and the graph
/// disagree — a broken invariant — and `Clone` is not a conservative default for that. It is a
/// specific claim about what happened: *this child is an unmutated copy of this parent*. It would
/// be written to the lineage store, read back by the diagnostics, and counted as a reproduction
/// event that never occurred. A lineage that quietly makes up ancestry is worse than one that
/// stops.
pub fn rebuild_relations_from_plan(
    edges: &[crate::evolution::simplify::SimplifiedEdge],
    relations: &[LineageRelation],
) -> Result<Vec<LineageRelation>, String> {
    let original_type: std::collections::HashMap<(&str, &str), RelationType> = relations
        .iter()
        .map(|r| {
            (
                (r.source_id.as_str(), r.target_id.as_str()),
                r.relation_type,
            )
        })
        .collect();

    edges
        .iter()
        .map(|e| {
            let key = (e.parent_id.as_str(), e.child_id.as_str());
            // An uncompressed edge keeps the type exactly as it was recorded. Only a spliced path
            // gets a summary, and `path_events` marks it as one so no reader mistakes the summary
            // for an observation.
            let (relation_type, path_events) = if e.events <= 1 {
                let recorded = original_type.get(&key).copied().ok_or_else(|| {
                    format!(
                        "simplify plan carries an uncompressed edge {} -> {} that is not in the \
                         lineage relations. This is a broken plan/graph invariant, not a clone: \
                         defaulting it to RelationType::Clone would record a reproduction event \
                         that was never observed.",
                        e.parent_id, e.child_id
                    )
                })?;
                (recorded, None)
            } else if e.crossovers > 0 {
                (RelationType::Crossover, Some(e.events))
            } else if e.mutations > 0 {
                (RelationType::Mutate, Some(e.events))
            } else {
                (RelationType::Clone, Some(e.events))
            };
            Ok(LineageRelation {
                source_id: e.parent_id.clone(),
                target_id: e.child_id.clone(),
                relation_type,
                path_events,
            })
        })
        .collect()
}

pub fn cumulative_mutations_from_edges(
    nodes: &[LineageNode],
    relations: &[LineageRelation],
) -> std::collections::HashMap<String, u32> {
    let mut parents: std::collections::HashMap<&str, Vec<(&str, RelationType)>> =
        std::collections::HashMap::new();
    for rel in relations {
        parents
            .entry(rel.target_id.as_str())
            .or_default()
            .push((rel.source_id.as_str(), rel.relation_type));
    }

    // Iterative rather than recursive: a long trunk is exactly the shape this walks, and a deep
    // lineage would blow the stack on the machine it matters on. `visiting` also makes a cyclic
    // graph terminate with a finite answer instead of hanging.
    let mut memo: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut visiting: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for node in nodes {
        if memo.contains_key(&node.id) {
            continue;
        }
        let mut stack: Vec<(&str, bool)> = vec![(node.id.as_str(), false)];
        while let Some((id, expanded)) = stack.pop() {
            if memo.contains_key(id) {
                visiting.remove(id);
                continue;
            }
            if expanded {
                let mut count = 0;
                if let Some(ps) = parents.get(id) {
                    let mut max_parent = 0;
                    let mut mutated = false;
                    for (pid, rel) in ps {
                        max_parent = max_parent.max(memo.get(*pid).copied().unwrap_or(0));
                        if *rel == RelationType::Mutate {
                            mutated = true;
                        }
                    }
                    count = max_parent + u32::from(mutated);
                }
                memo.insert(id.to_string(), count);
                visiting.remove(id);
                continue;
            }
            visiting.insert(id);
            stack.push((id, true));
            if let Some(ps) = parents.get(id) {
                for (pid, _) in ps {
                    if !memo.contains_key(*pid) && !visiting.contains(*pid) {
                        stack.push((*pid, false));
                    }
                }
            }
        }
    }
    memo
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RelationType {
    Clone,
    Mutate,
    Crossover,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageRelation {
    pub source_id: String,
    pub target_id: String,
    /// The reproduction event that produced `target_id` from `source_id`.
    ///
    /// When [`Self::path_events`] is `Some(n)` with `n > 1` this edge stands for a *path*, and the
    /// type is the strongest event on it (`Crossover` > `Mutate` > `Clone`) — a summary, not an
    /// event that happened. Read [`Self::path_events`] before drawing a conclusion from it, and
    /// take mutation totals from [`LineageNode::cumulative_mutations`], which stays exact.
    pub relation_type: RelationType,
    /// Reproduction events this edge represents, or `None` for a single recorded event.
    ///
    /// Present only on edges produced by compaction's unary-path splicing. `None` rather than `1`
    /// so that every pre-existing save deserializes to "a plain edge" without a migration, and so
    /// that "summarised" is visibly different from "recorded".
    #[serde(default)]
    pub path_events: Option<u32>,
}

/// What one [`LineageTracker::compact`] call removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompactionReport {
    pub nodes_before: usize,
    pub nodes_after: usize,
    pub relations_before: usize,
    pub relations_after: usize,
}

impl CompactionReport {
    /// Nodes removed. Each took a full `MorphologyGenotype` clone with it, which is the memory this
    /// exists to reclaim.
    pub fn nodes_removed(&self) -> usize {
        self.nodes_before.saturating_sub(self.nodes_after)
    }
}

pub trait LineageTracker: Send + Sync {
    fn add_root(&self, id: String, genotype: MorphologyGenotype) -> Result<(), String>;
    fn add_reproduction(
        &self,
        offspring_id: String,
        generation: u32,
        genotype: MorphologyGenotype,
        parents: Vec<String>,
        relation_type: RelationType,
    ) -> Result<(), String>;
    fn get_lineage_graph(&self) -> Result<(Vec<LineageNode>, Vec<LineageRelation>), String>;

    /// Drop every node with no `samples` descendant, freeing its genotype.
    ///
    /// # Why the caller supplies `samples`, and what must be in it
    ///
    /// The tracker does not know who is alive — it only ever sees writes. `samples` must contain
    /// **every id that can still be named as a parent**, which is more than "the living":
    ///
    /// - living agents' lineage ids;
    /// - **every `EliteIndividual::lineage_id` in the MAP-Elites archive.** An elite is selected as
    ///   a parent for future offspring and need not be an ancestor of anyone currently alive, so
    ///   pruning by liveness alone removes nodes the very next reproduction will name.
    ///
    /// Getting that wrong used to corrupt the graph silently. It no longer can:
    /// [`Self::add_reproduction`] refuses to write an edge whose parent is unknown, so a missed
    /// sample costs an ancestry link rather than producing an orphan edge that
    /// [`to_newick`](super::newick::to_newick) and [`simplify`](super::simplify::simplify) would
    /// later reject.
    ///
    /// # What it does, and what that costs
    ///
    /// **Unary-path compression is ON**, which is what takes the store to its O(alive) bound —
    /// pruning alone keeps every trunk back to genesis. It became safe once
    /// [`LineageNode::cumulative_mutations`] existed: the count the UI shows no longer depends on
    /// walking per-edge [`RelationType`]s that a spliced edge cannot carry.
    ///
    /// The price is paid in resolution, not correctness. A spliced node's **genotype is gone**, and
    /// the edge that replaced it carries [`LineageRelation::path_events`] to say so. Totals stay
    /// exact; the individual events between two surviving nodes do not survive.
    ///
    /// Ordering inside this method is load-bearing: every node's cumulative count is backfilled
    /// against the **full** graph before anything is spliced. Deriving it afterwards would read the
    /// compacted graph and return a smaller, entirely plausible, wrong number.
    ///
    /// **Neo4j is untouched.** Only the in-memory store shrinks. Deleting from the graph database
    /// is a destructive remote operation and needs its own decision. A consequence worth knowing:
    /// while Neo4j is online `get_lineage_graph` reads from it and returns the **full** graph, so
    /// the compacted store is not what the UI sees.
    fn compact(&self, samples: &[String]) -> Result<CompactionReport, String>;
}

pub struct InMemoryLineageTracker {
    nodes: RwLock<Vec<LineageNode>>,
    relations: RwLock<Vec<LineageRelation>>,
}

impl Default for InMemoryLineageTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryLineageTracker {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(Vec::new()),
            relations: RwLock::new(Vec::new()),
        }
    }

    /// Replace the whole graph — the restore path, used when a save is loaded.
    ///
    /// Nodes arrive exactly as they were serialized, which for any save written before
    /// [`LineageNode::cumulative_mutations`] existed means `None` everywhere. That is deliberate
    /// and must not be "fixed" by backfilling here: the derivation belongs in
    /// [`LineageTracker::compact`], where it runs against the full graph at the one moment it
    /// matters. Backfilling on load would also mean paying for a whole-graph walk on every restore.
    pub fn load_state(&self, nodes: Vec<LineageNode>, relations: Vec<LineageRelation>) {
        if let Ok(mut n) = self.nodes.write() {
            *n = nodes;
        }
        if let Ok(mut r) = self.relations.write() {
            *r = relations;
        }
    }
}

impl LineageTracker for InMemoryLineageTracker {
    fn add_root(&self, id: String, genotype: MorphologyGenotype) -> Result<(), String> {
        let node = LineageNode {
            id,
            generation: 0,
            genotype: Some(genotype),
            // A root has no ancestry, so zero is a recorded fact here, not a default.
            cumulative_mutations: Some(0),
        };
        self.nodes.write().map_err(|e| e.to_string())?.push(node);
        Ok(())
    }

    fn add_reproduction(
        &self,
        offspring_id: String,
        generation: u32,
        genotype: MorphologyGenotype,
        parents: Vec<String>,
        relation_type: RelationType,
    ) -> Result<(), String> {
        let mut nodes = self.nodes.write().map_err(|e| e.to_string())?;

        // Derived from the parents already in the store, before the offspring is pushed. Only
        // parents that exist contribute — an unknown parent has its edge refused below, so it is
        // not part of this individual's ancestry and must not be part of its count either.
        //
        // `None` is contagious on purpose: if any real parent's count was never recorded (a node
        // restored from a pre-field save), this node's total is not knowable from the store alone,
        // and claiming a number derived from a partial ancestry would be worse than saying
        // "unknown" and letting the edge walk answer.
        let cumulative_mutations = {
            let mut max_parent: Option<u32> = None;
            let mut saw_unrecorded = false;
            for parent in &parents {
                if let Some(p) = nodes.iter().find(|n| &n.id == parent) {
                    match p.cumulative_mutations {
                        Some(v) => max_parent = Some(max_parent.map_or(v, |m: u32| m.max(v))),
                        None => saw_unrecorded = true,
                    }
                }
            }
            if saw_unrecorded {
                None
            } else {
                // No known parent at all ⇒ this offspring is effectively a root.
                let base = max_parent.unwrap_or(0);
                Some(base.saturating_add(u32::from(relation_type == RelationType::Mutate)))
            }
        };

        let node = LineageNode {
            id: offspring_id.clone(),
            generation,
            genotype: Some(genotype),
            cumulative_mutations,
        };
        nodes.push(node);
        // Known ids are read from the node list rather than kept as a side index: the list is
        // already behind this lock, and a second structure would be one more thing compaction has
        // to remember to rewrite.
        let known: std::collections::HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();

        let mut relations = self.relations.write().map_err(|e| e.to_string())?;
        let mut unknown: Vec<String> = Vec::new();
        for parent in parents {
            // An edge to a node that does not exist is an ORPHAN EDGE, and it is the one defect
            // that makes the whole graph unusable: `to_newick` and `simplify` both refuse a graph
            // containing one, so a single bad write poisons export and compaction from then on.
            //
            // Before compaction existed this could not happen, because nothing was ever removed.
            // Now it can — a caller whose `samples` set missed a future parent prunes a node the
            // next reproduction names. Refusing the edge costs one ancestry link and leaves the
            // offspring as a new root; writing it would cost the graph.
            if !known.contains(parent.as_str()) {
                unknown.push(parent);
                continue;
            }
            relations.push(LineageRelation {
                source_id: parent,
                target_id: offspring_id.clone(),
                relation_type,
                path_events: None,
            });
        }

        if unknown.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "lineage: offspring {offspring_id:?} names {} unknown parent(s) {unknown:?}; the \
                 edges were not written. A compaction whose sample set missed a future parent is \
                 the usual cause — see LineageTracker::compact.",
                unknown.len()
            ))
        }
    }

    fn compact(&self, samples: &[String]) -> Result<CompactionReport, String> {
        use crate::evolution::simplify::{simplify_with, SimplifyOptions};

        let mut nodes = self.nodes.write().map_err(|e| e.to_string())?;
        let mut relations = self.relations.write().map_err(|e| e.to_string())?;

        let nodes_before = nodes.len();
        let relations_before = relations.len();

        // ---- backfill before anything is removed ------------------------------------------------
        //
        // Compression is only safe once every surviving node carries its own total, because after
        // the splice the intermediates that the edge walk counted are gone — walking the compacted
        // graph would return a smaller, entirely plausible, wrong number.
        //
        // A store restored from a pre-field save has `None` everywhere, so the backfill has to
        // happen here, against the FULL graph, while the evidence for it still exists. Doing it
        // after `simplify_with` would compute the same wrong number the field exists to prevent.
        let derived = cumulative_mutations_from_edges(&nodes, &relations);
        for node in nodes.iter_mut() {
            if node.cumulative_mutations.is_none() {
                node.cumulative_mutations = derived.get(&node.id).copied();
            }
        }

        // `simplify` decides what survives; the rewrite below is what applies it. Unary-path
        // splicing is now ON — it is the step that reaches the O(alive) bound, and it is what the
        // per-node count above was added to make safe.
        let plan = simplify_with(
            &nodes,
            &relations,
            samples,
            SimplifyOptions {
                compress_unary_paths: true,
            },
        )
        .map_err(|e| e.to_string())?;

        let keep: std::collections::HashSet<&str> =
            plan.nodes.iter().map(|n| n.id.as_str()).collect();

        // Relations must be rebuilt from the PLAN, not filtered from the originals. Filtering was
        // correct while nothing was spliced; with compression on it silently disconnects the graph,
        // because both original edges of an `A → B → C` path reference the removed `B` and would
        // both be dropped, orphaning `C`. The plan's edge is the one that spans the splice.
        let kept_relations = rebuild_relations_from_plan(&plan.edges, &relations)?;

        let kept_nodes: Vec<LineageNode> = nodes
            .iter()
            .filter(|n| keep.contains(n.id.as_str()))
            .cloned()
            .collect();

        let report = CompactionReport {
            nodes_before,
            nodes_after: kept_nodes.len(),
            relations_before,
            relations_after: kept_relations.len(),
        };

        *nodes = kept_nodes;
        *relations = kept_relations;
        Ok(report)
    }

    fn get_lineage_graph(&self) -> Result<(Vec<LineageNode>, Vec<LineageRelation>), String> {
        let nodes = self.nodes.read().map_err(|e| e.to_string())?.clone();
        let relations = self.relations.read().map_err(|e| e.to_string())?.clone();
        Ok((nodes, relations))
    }
}

// Only the Neo4j paths drive this runtime; without the `neo4j` feature nothing here is async.
#[cfg(feature = "neo4j")]
static TOKIO_RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

#[cfg(feature = "neo4j")]
fn get_tokio_runtime() -> &'static tokio::runtime::Runtime {
    TOKIO_RUNTIME.get_or_init(|| {
        // A `OnceLock` initialiser cannot return an error, and every Neo4j call below needs this
        // runtime, so failure here is genuinely unrecoverable for the `neo4j` feature. What was
        // missing was any indication of what failed: `.expect` names it.
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("lineage tracker could not start a tokio runtime for the Neo4j driver")
    })
}

/// The Neo4j driver handle, or a type that can never be constructed when the `neo4j` feature is
/// off (G2). Keeping the field present either way means `is_online` stays the single switch the
/// rest of this file already branches on, instead of every method growing a cfg.
#[cfg(feature = "neo4j")]
type Neo4jGraph = neo4rs::Graph;
#[cfg(not(feature = "neo4j"))]
type Neo4jGraph = std::convert::Infallible;

pub struct FallbackLineageTracker {
    in_memory: InMemoryLineageTracker,
    neo4j_graph: RwLock<Option<Neo4jGraph>>,
    is_online: AtomicBool,
}

impl FallbackLineageTracker {
    pub fn new(uri: &str, user: &str, pass: &str) -> Self {
        let in_memory = InMemoryLineageTracker::new();
        let is_online = AtomicBool::new(false);
        let neo4j_graph = RwLock::new(None);

        // Without the `neo4j` feature there is no driver to connect with, so the tracker simply stays
        // offline — which is the same state a failed connection produces, and a path this type was
        // built to handle from the start.
        #[cfg(feature = "neo4j")]
        {
            let config = neo4rs::ConfigBuilder::new()
                .uri(uri)
                .user(user)
                .password(pass)
                .build();

            let graph = if let Ok(config) = config {
                let connect_fut = neo4rs::Graph::connect(config);
                let rt = get_tokio_runtime();
                let (tx, rx) = crossbeam_channel::bounded(1);
                rt.spawn(async move {
                    let res = async {
                        let g = tokio::time::timeout(
                            std::time::Duration::from_millis(500),
                            connect_fut,
                        )
                        .await
                        .ok()?
                        .ok()?;
                        let ping = neo4rs::query("RETURN 1");
                        tokio::time::timeout(std::time::Duration::from_millis(500), g.run(ping))
                            .await
                            .ok()?
                            .ok()?;
                        Some(g)
                    }
                    .await;
                    let _ = tx.send(res);
                });
                rx.recv().unwrap_or(None)
            } else {
                None
            };

            if graph.is_some() {
                is_online.store(true, Ordering::SeqCst);
                *neo4j_graph.write().unwrap_or_else(|e| e.into_inner()) = graph;
            }
        }
        #[cfg(not(feature = "neo4j"))]
        let _ = (uri, user, pass);

        Self {
            in_memory,
            neo4j_graph,
            is_online,
        }
    }

    #[cfg(feature = "neo4j")]
    fn run_neo4j_async<F, T>(&self, fut: F) -> Result<T, String>
    where
        F: std::future::Future<Output = Result<T, String>> + Send + 'static,
        T: Send + 'static,
    {
        let rt = get_tokio_runtime();
        let (tx, rx) = crossbeam_channel::bounded(1);
        rt.spawn(async move {
            let res = tokio::time::timeout(std::time::Duration::from_millis(1000), fut)
                .await
                .map_err(|_| "Timeout waiting for Neo4j".to_string())
                .and_then(|r| r);
            let _ = tx.send(res);
        });
        rx.recv().map_err(|e| e.to_string())?
    }

    pub fn is_online(&self) -> bool {
        self.is_online.load(Ordering::SeqCst)
    }

    pub fn mark_offline(&self) {
        self.is_online.store(false, Ordering::SeqCst);
        if let Ok(mut g) = self.neo4j_graph.write() {
            *g = None;
        }
    }

    pub fn load_state(&self, nodes: Vec<LineageNode>, relations: Vec<LineageRelation>) {
        self.in_memory.load_state(nodes, relations);
    }
}

impl LineageTracker for FallbackLineageTracker {
    fn add_root(&self, id: String, genotype: MorphologyGenotype) -> Result<(), String> {
        self.in_memory.add_root(id.clone(), genotype.clone())?;

        // Compiled out entirely without the `neo4j` feature: `is_online` can only become true after a
        // successful connect, which cannot happen when there is no driver.
        #[cfg(feature = "neo4j")]
        if self.is_online() {
            let graph_opt = self.neo4j_graph.read().map_err(|e| e.to_string())?.clone();
            if let Some(graph) = graph_opt {
                let genotype_str = serde_json::to_string(&genotype).unwrap_or_default();
                let id_clone = id.clone();
                let fut = async move {
                    let q = neo4rs::query(
                        "MERGE (n:LineageNode {id: $id}) \
                         ON CREATE SET n.generation = $generation, n.genotype = $genotype",
                    )
                    .param("id", id_clone)
                    .param("generation", 0)
                    .param("genotype", genotype_str);
                    graph.run(q).await.map_err(|e| e.to_string())
                };

                if let Err(e) = self.run_neo4j_async(fut) {
                    eprintln!("Neo4j write failed: {}. Falling back to offline mode.", e);
                    self.mark_offline();
                }
            }
        }
        Ok(())
    }

    fn add_reproduction(
        &self,
        offspring_id: String,
        generation: u32,
        genotype: MorphologyGenotype,
        parents: Vec<String>,
        relation_type: RelationType,
    ) -> Result<(), String> {
        self.in_memory.add_reproduction(
            offspring_id.clone(),
            generation,
            genotype.clone(),
            parents.clone(),
            relation_type,
        )?;

        // Compiled out entirely without the `neo4j` feature: `is_online` can only become true after a
        // successful connect, which cannot happen when there is no driver.
        #[cfg(feature = "neo4j")]
        if self.is_online() {
            let graph_opt = self.neo4j_graph.read().map_err(|e| e.to_string())?.clone();
            if let Some(graph) = graph_opt {
                let genotype_str = serde_json::to_string(&genotype).unwrap_or_default();
                let offspring_id_clone = offspring_id.clone();
                let parents_clone = parents.clone();
                let rel_type_str = match relation_type {
                    RelationType::Clone => "Clone",
                    RelationType::Mutate => "Mutate",
                    RelationType::Crossover => "Crossover",
                };

                let fut = async move {
                    // 1. Merge the offspring node
                    let q_node = neo4rs::query(
                        "MERGE (n:LineageNode {id: $id}) \
                         ON CREATE SET n.generation = $generation, n.genotype = $genotype",
                    )
                    .param("id", offspring_id_clone.clone())
                    .param("generation", generation as i64)
                    .param("genotype", genotype_str);
                    graph.run(q_node).await.map_err(|e| e.to_string())?;

                    // 2. Merge parent relationships
                    for parent_id in parents_clone {
                        let q_rel = neo4rs::query(
                            "MATCH (p:LineageNode {id: $parent_id}), (c:LineageNode {id: $child_id}) \
                             MERGE (p)-[r:PARENT_OF {type: $relation_type}]->(c)"
                        )
                        .param("parent_id", parent_id)
                        .param("child_id", offspring_id_clone.clone())
                        .param("relation_type", rel_type_str);
                        graph.run(q_rel).await.map_err(|e| e.to_string())?;
                    }
                    Ok(())
                };

                if let Err(e) = self.run_neo4j_async(fut) {
                    eprintln!(
                        "Neo4j reproduction write failed: {}. Falling back to offline mode.",
                        e
                    );
                    self.mark_offline();
                }
            }
        }
        Ok(())
    }

    fn get_lineage_graph(&self) -> Result<(Vec<LineageNode>, Vec<LineageRelation>), String> {
        // Compiled out entirely without the `neo4j` feature: `is_online` can only become true after a
        // successful connect, which cannot happen when there is no driver.
        #[cfg(feature = "neo4j")]
        if self.is_online() {
            let graph_opt = self.neo4j_graph.read().map_err(|e| e.to_string())?.clone();
            if let Some(graph) = graph_opt {
                let fut = async move {
                    // Query nodes
                    let q_nodes = neo4rs::query(
                        "MATCH (n:LineageNode) RETURN n.id AS id, n.generation AS generation, n.genotype AS genotype"
                    );
                    let mut result_nodes =
                        graph.execute(q_nodes).await.map_err(|e| e.to_string())?;
                    let mut nodes = Vec::new();
                    while let Some(row) = result_nodes.next().await.map_err(|e| e.to_string())? {
                        let id: String = row.get("id").map_err(|e| e.to_string())?;
                        let gen_val: i64 = row.get("generation").map_err(|e| e.to_string())?;
                        let genotype_str: Option<String> =
                            row.get("genotype").map_err(|e| e.to_string())?;
                        let genotype = genotype_str.and_then(|s| serde_json::from_str(&s).ok());
                        nodes.push(LineageNode {
                            id,
                            generation: gen_val as u32,
                            genotype,
                            // Not a column in the graph database. `None` sends the reader to the
                            // edge walk, which is exact here because Neo4j keeps the full,
                            // uncompacted graph.
                            cumulative_mutations: None,
                        });
                    }

                    // Query relations
                    let q_rels = neo4rs::query(
                        "MATCH (p:LineageNode)-[r:PARENT_OF]->(c:LineageNode) RETURN p.id AS parent_id, c.id AS child_id, r.type AS rel_type"
                    );
                    let mut result_rels = graph.execute(q_rels).await.map_err(|e| e.to_string())?;
                    let mut relations = Vec::new();
                    while let Some(row) = result_rels.next().await.map_err(|e| e.to_string())? {
                        let parent_id: String = row.get("parent_id").map_err(|e| e.to_string())?;
                        let child_id: String = row.get("child_id").map_err(|e| e.to_string())?;
                        let rel_type_str: String =
                            row.get("rel_type").map_err(|e| e.to_string())?;
                        let relation_type = match rel_type_str.as_str() {
                            "Clone" => RelationType::Clone,
                            "Mutate" => RelationType::Mutate,
                            "Crossover" => RelationType::Crossover,
                            _ => RelationType::Clone,
                        };
                        relations.push(LineageRelation {
                            source_id: parent_id,
                            target_id: child_id,
                            relation_type,
                            // Neo4j holds the uncompacted graph — `compact` only ever shrinks the
                            // in-memory store — so every edge read back is a single event.
                            path_events: None,
                        });
                    }

                    Ok((nodes, relations))
                };

                match self.run_neo4j_async(fut) {
                    Ok(graph_data) => return Ok(graph_data),
                    Err(e) => {
                        eprintln!("Neo4j read failed: {}. Falling back to offline mode.", e);
                        self.mark_offline();
                    }
                }
            }
        }

        // Fallback to in-memory graph
        self.in_memory.get_lineage_graph()
    }

    /// Compacts the in-memory store only.
    ///
    /// Neo4j keeps everything. That is deliberate: the database is the durable record and deleting
    /// from it is a destructive remote operation with its own failure modes, while this method
    /// exists to bound the memory of a process that must not grow without limit. The consequence
    /// worth knowing is that with Neo4j online, `get_lineage_graph` reads from the database and so
    /// still returns the FULL graph — compaction changes what the fallback holds, not what an
    /// online run reports.
    fn compact(&self, samples: &[String]) -> Result<CompactionReport, String> {
        self.in_memory.compact(samples)
    }
}
