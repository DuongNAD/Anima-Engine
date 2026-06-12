use serde::{Serialize, Deserialize};
use crate::evolution::genotype::MorphologyGenotype;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

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
}

pub struct InMemoryLineageTracker {
    nodes: RwLock<Vec<LineageNode>>,
    relations: RwLock<Vec<LineageRelation>>,
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
        self.nodes.write().map_err(|e| e.to_string())?.push(node);

        let mut relations = self.relations.write().map_err(|e| e.to_string())?;
        for parent in parents {
            relations.push(LineageRelation {
                source_id: parent,
                target_id: offspring_id.clone(),
                relation_type,
            });
        }
        Ok(())
    }

    fn get_lineage_graph(&self) -> Result<(Vec<LineageNode>, Vec<LineageRelation>), String> {
        let nodes = self.nodes.read().map_err(|e| e.to_string())?.clone();
        let relations = self.relations.read().map_err(|e| e.to_string())?.clone();
        Ok((nodes, relations))
    }
}

static TOKIO_RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

fn get_tokio_runtime() -> &'static tokio::runtime::Runtime {
    TOKIO_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
    })
}

pub struct FallbackLineageTracker {
    in_memory: InMemoryLineageTracker,
    neo4j_graph: RwLock<Option<neo4rs::Graph>>,
    is_online: AtomicBool,
}

impl FallbackLineageTracker {
    pub fn new(uri: &str, user: &str, pass: &str) -> Self {
        let in_memory = InMemoryLineageTracker::new();
        let is_online = AtomicBool::new(false);
        let neo4j_graph = RwLock::new(None);

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
                    let g = tokio::time::timeout(std::time::Duration::from_millis(500), connect_fut)
                        .await
                        .ok()?
                        .ok()?;
                    let ping = neo4rs::query("RETURN 1");
                    tokio::time::timeout(std::time::Duration::from_millis(500), g.run(ping))
                        .await
                        .ok()?
                        .ok()?;
                    Some(g)
                }.await;
                let _ = tx.send(res);
            });
            rx.recv().unwrap_or(None)
        } else {
            None
        };

        if graph.is_some() {
            is_online.store(true, Ordering::SeqCst);
            *neo4j_graph.write().unwrap() = graph;
        }

        Self {
            in_memory,
            neo4j_graph,
            is_online,
        }
    }

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
}

impl LineageTracker for FallbackLineageTracker {
    fn add_root(&self, id: String, genotype: MorphologyGenotype) -> Result<(), String> {
        self.in_memory.add_root(id.clone(), genotype.clone())?;

        if self.is_online() {
            let graph_opt = self.neo4j_graph.read().map_err(|e| e.to_string())?.clone();
            if let Some(graph) = graph_opt {
                let genotype_str = serde_json::to_string(&genotype).unwrap_or_default();
                let id_clone = id.clone();
                let fut = async move {
                    let q = neo4rs::query(
                        "MERGE (n:LineageNode {id: $id}) \
                         ON CREATE SET n.generation = $generation, n.genotype = $genotype"
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
                         ON CREATE SET n.generation = $generation, n.genotype = $genotype"
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
                    eprintln!("Neo4j reproduction write failed: {}. Falling back to offline mode.", e);
                    self.mark_offline();
                }
            }
        }
        Ok(())
    }

    fn get_lineage_graph(&self) -> Result<(Vec<LineageNode>, Vec<LineageRelation>), String> {
        if self.is_online() {
            let graph_opt = self.neo4j_graph.read().map_err(|e| e.to_string())?.clone();
            if let Some(graph) = graph_opt {
                let fut = async move {
                    // Query nodes
                    let q_nodes = neo4rs::query(
                        "MATCH (n:LineageNode) RETURN n.id AS id, n.generation AS generation, n.genotype AS genotype"
                    );
                    let mut result_nodes = graph.execute(q_nodes).await.map_err(|e| e.to_string())?;
                    let mut nodes = Vec::new();
                    while let Some(row) = result_nodes.next().await.map_err(|e| e.to_string())? {
                        let id: String = row.get("id").map_err(|e| e.to_string())?;
                        let gen_val: i64 = row.get("generation").map_err(|e| e.to_string())?;
                        let genotype_str: Option<String> = row.get("genotype").map_err(|e| e.to_string())?;
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
                        let rel_type_str: String = row.get("rel_type").map_err(|e| e.to_string())?;
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
}
