use crate::evolution::genotype::MorphologyGenotype;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageNode {
    pub id: String,
    pub generation: u32,
    pub genotype: Option<MorphologyGenotype>,
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
    pub relation_type: RelationType,
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
    /// # What it deliberately does NOT do
    ///
    /// **No unary-path compression.** That is the step that reaches the O(alive) bound, and it
    /// removes nodes that existing consumers still read: the lineage graph the UI draws comes
    /// straight from here, and `get_mutations_count` in `commands/evolution.rs` walks per-edge
    /// `RelationType`s that a compressed edge cannot carry. Enabling it for storage requires a
    /// per-node cumulative mutation count to be persisted first — a change to `LineageNode`, which
    /// is written into save state. Until then compaction removes extinct branches, which is where
    /// the genotypes are, and leaves the trunk.
    ///
    /// **Neo4j is untouched.** Only the in-memory store shrinks. Deleting from the graph database
    /// is a destructive remote operation and needs its own decision.
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
}

impl LineageTracker for InMemoryLineageTracker {
    fn add_root(&self, id: String, genotype: MorphologyGenotype) -> Result<(), String> {
        let node = LineageNode {
            id,
            generation: 0,
            genotype: Some(genotype),
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
        let node = LineageNode {
            id: offspring_id.clone(),
            generation,
            genotype: Some(genotype),
        };
        let mut nodes = self.nodes.write().map_err(|e| e.to_string())?;
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

        // `simplify` decides what survives; the filtering below is what applies it. Deriving the
        // stored relations from `SimplifiedEdge` instead would mean inventing a `RelationType` for
        // each one — the fabrication that module exists to refuse. Filtering the ORIGINALS keeps
        // every type exactly as it was recorded.
        let plan = simplify_with(
            &nodes,
            &relations,
            samples,
            SimplifyOptions {
                compress_unary_paths: false,
            },
        )
        .map_err(|e| e.to_string())?;

        let keep: std::collections::HashSet<&str> =
            plan.nodes.iter().map(|n| n.id.as_str()).collect();

        let kept_relations: Vec<LineageRelation> = relations
            .iter()
            .filter(|r| keep.contains(r.source_id.as_str()) && keep.contains(r.target_id.as_str()))
            .cloned()
            .collect();
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
        if let Ok(mut mem_nodes) = self.in_memory.nodes.write() {
            *mem_nodes = nodes;
        }
        if let Ok(mut mem_relations) = self.in_memory.relations.write() {
            *mem_relations = relations;
        }
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
