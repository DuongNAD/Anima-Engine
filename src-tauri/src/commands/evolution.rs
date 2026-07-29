use crate::evolution::lineage::LineageTracker;
use crate::AppState;
use tauri::State;

// Every lineage-analysis command below takes a SINGLE-WORD parameter name on purpose.
//
// `#[tauri::command]` defaults to `ArgumentCase::Camel`, so a Rust parameter `file_path` arrives
// from JS as `filePath`, and a call site spelling it `file_path` silently never delivers the
// argument. That is the bug class `scripts/check_ipc_arg_case.mjs` exists for, and it had four
// commands broken in the real app while every mocked test stayed green. A name with no underscore
// has one spelling in both languages, so here the class is unreachable rather than merely caught.

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct EvolutionSettings {
    pub mutation_rate: f64,
    pub selection_bias: f64,
    pub grid_resolution: u32,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct EliteIndividualState {
    pub fitness: f64,
    pub features: Vec<f64>,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct MapElitesGridState {
    pub grid: std::collections::HashMap<String, EliteIndividualState>,
    pub grid_resolution: u32,
}

/// One agent in the lineage graph, as `get_lineage_graph` publishes it.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LineageNodePayload {
    pub id: String,
    pub generation: u32,
    pub parent_id: Option<String>,
    pub fitness: f64,
    /// Cumulative mutations along this agent's ancestry.
    pub mutations_count: u32,
}

/// A parent → child edge.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LineageLinkPayload {
    pub source: String,
    pub target: String,
}

/// The whole graph, plus whether it came from Neo4j or the in-memory fallback.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LineageGraphPayload {
    pub nodes: Vec<LineageNodePayload>,
    pub links: Vec<LineageLinkPayload>,
    /// False when the tracker is running offline in memory, so the UI can say which it is showing.
    pub db_connected: bool,
}

/// One maximal common ancestor, as `get_lineage_mrca` publishes it.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LineageMrcaAncestor {
    pub id: String,
    pub generation: u32,
    /// Graph edges down to the closest queried individual. **Edges, not reproduction events** — on
    /// a compacted graph one edge stands for a spliced path, so this is a lower bound. Generation
    /// deltas stay exact.
    pub nearest_edges: u32,
    /// The same measure for the farthest queried individual.
    pub farthest_edges: u32,
}

/// Where a set of individuals last shared an ancestor.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LineageMrcaPayload {
    /// Most recent first. **Empty is an answer**, not a failure: genesis creates one root per
    /// founder, so two individuals from different founding lines genuinely never coalesce.
    pub ancestors: Vec<LineageMrcaAncestor>,
    /// Nodes ancestral to every queried individual, maximal or not — the shared trunk above the
    /// coalescence point.
    pub common_ancestors: usize,
    /// **More than one answer.** Crossover gives an individual two parents, so the lineage is a DAG
    /// and its common ancestors can be incomparable — none of them "more recent" than the rest.
    /// A consumer rendering a single value must not just take `ancestors[0]`; see
    /// `evolution/mrca.rs` for the shape that produces this.
    pub ambiguous: bool,
}

/// The lineage as a Newick forest, ready to hand to any phylogenetics reader.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LineageNewickPayload {
    /// One `;`-terminated tree per root. A forest is the normal case here, not an edge case.
    pub trees: Vec<String>,
    /// Parent edges Newick could not represent, because it is a tree format and crossover gives a
    /// node two parents. **Non-zero means `trees` is a view of the lineage, not the lineage.**
    pub dropped_parent_edges: usize,
    pub roots: usize,
}

/// One node of a simplified lineage.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SimplifiedLineageNode {
    pub id: String,
    pub generation: u32,
    pub mutations_count: u32,
}

/// One edge of a simplified lineage. May stand for a whole path of reproductions, which is why it
/// carries counts instead of a relation type.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SimplifiedLineageEdge {
    pub parent_id: String,
    pub child_id: String,
    /// Reproductions this edge replaced. `1` when nothing was compressed.
    pub events: u32,
    pub mutations: u32,
    pub crossovers: u32,
}

/// The lineage reduced to the ancestry of a chosen set of individuals.
#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SimplifiedLineagePayload {
    pub nodes: Vec<SimplifiedLineageNode>,
    pub edges: Vec<SimplifiedLineageEdge>,
    /// Removed for having no queried descendant.
    pub dropped_nodes: usize,
    /// Spliced out of a unary path. Their ancestry survives in the edge that replaced them; their
    /// genotype does not.
    pub compressed_nodes: usize,
}

#[tauri::command]
pub fn get_map_elites_grid(state: State<'_, AppState>) -> Result<MapElitesGridState, String> {
    let grid = state
        .map_elites_grid
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    Ok(grid.clone())
}

#[tauri::command]
pub fn update_evolution_settings(
    state: State<'_, AppState>,
    settings: EvolutionSettings,
) -> Result<bool, String> {
    if settings.mutation_rate < 0.0
        || settings.mutation_rate > 1.0
        || settings.selection_bias <= 0.0
    {
        return Err("Invalid settings".to_string());
    }
    // ADR-0004 C3. One call that records and writes; this command no longer has a path that does one
    // without the other.
    state.seam.set_evolution_settings(settings)?;
    Ok(true)
}

#[tauri::command]
pub fn toggle_evolution(
    state: State<'_, AppState>,
    _app_handle: tauri::AppHandle,
) -> Result<bool, String> {
    // The seam reads the current value, records the flip and applies it as one operation — so the
    // recorded value cannot disagree with the value the world took.
    Ok(state.seam.toggle_evolution())
}

#[tauri::command]
pub fn get_lineage_graph(state: State<'_, AppState>) -> Result<LineageGraphPayload, String> {
    let (nodes, relations) = state.engine.lineage_tracker.get_lineage_graph()?;
    let db_connected = state.engine.lineage_tracker.is_online();

    let mut payload_nodes = Vec::with_capacity(nodes.len());
    let mut payload_links = Vec::with_capacity(relations.len());

    for rel in &relations {
        payload_links.push(LineageLinkPayload {
            source: rel.source_id.clone(),
            target: rel.target_id.clone(),
        });
    }

    let mut parent_map = std::collections::HashMap::new();
    for rel in &relations {
        parent_map
            .entry(rel.target_id.clone())
            .or_insert_with(Vec::new)
            .push(rel.source_id.clone());
    }

    // The edge walk is now the **fallback**, not the source of truth, and it lives in
    // `evolution::lineage` so exactly one implementation of the rule exists. It is still correct
    // for any graph that has not been compacted — which is every pre-field save, and everything
    // Neo4j returns.
    //
    // It is wrong for a compacted graph, and that is the whole reason the stored field exists: a
    // spliced edge stands for a path, so walking it counts one mutation where five happened. So a
    // node that recorded its own total is believed, and only a node that never did gets walked.
    let derived = crate::evolution::lineage::cumulative_mutations_from_edges(&nodes, &relations);

    for node in &nodes {
        let parent_id = parent_map
            .get(&node.id)
            .and_then(|parents| parents.first())
            .cloned();

        let mutations_count = node
            .cumulative_mutations
            .or_else(|| derived.get(&node.id).copied())
            .unwrap_or(0);
        let fitness = node
            .genotype
            .as_ref()
            .map(|g| g.nodes.len() as f64)
            .unwrap_or(0.0);

        payload_nodes.push(LineageNodePayload {
            id: node.id.clone(),
            generation: node.generation,
            parent_id,
            fitness,
            mutations_count,
        });
    }

    Ok(LineageGraphPayload {
        nodes: payload_nodes,
        links: payload_links,
        db_connected,
    })
}

/// Where `individuals` last shared an ancestor (OSS-072).
///
/// # Why the caller has to name them
///
/// There is no "the living population" default, and that is deliberate. The tracker only ever sees
/// writes, so it does not know who is alive — the same reason `LineageTracker::compact` makes the
/// caller supply its sample set. Defaulting to the graph's childless nodes would look like an
/// answer about the population and be an answer about the graph.
///
/// # Reading the result
///
/// [`LineageMrcaPayload::ancestors`] is a **set**: empty means no shared ancestor (ordinary in a
/// founder forest), and more than one means the lineage branched and rejoined through crossover.
/// See `evolution/mrca.rs` for the conventions, including that a node counts as its own ancestor.
#[tauri::command]
pub fn get_lineage_mrca(
    state: State<'_, AppState>,
    individuals: Vec<String>,
) -> Result<LineageMrcaPayload, String> {
    let (nodes, relations) = state.engine.lineage_tracker.get_lineage_graph()?;
    let result = crate::evolution::mrca::mrca(&nodes, &relations, &individuals)
        .map_err(|e| e.to_string())?;

    Ok(LineageMrcaPayload {
        ambiguous: result.is_ambiguous(),
        common_ancestors: result.common_ancestors,
        ancestors: result
            .ancestors
            .into_iter()
            .map(|a| LineageMrcaAncestor {
                id: a.id,
                generation: a.generation,
                nearest_edges: a.nearest_edges,
                farthest_edges: a.farthest_edges,
            })
            .collect(),
    })
}

/// The lineage as a Newick forest (OSS-070).
///
/// Returns the text rather than writing a file: the save-path name contract exists for state the
/// app owns, and an export the user asked for is the webview's to place. A caller that wants a file
/// joins [`LineageNewickPayload::trees`] with newlines — that is exactly the multi-tree file layout
/// `ape::read.tree` and `dendropy.TreeList.get` expect.
///
/// Fails on a malformed lineage — a cycle, an orphan edge, a duplicate id, or a generation that
/// disagrees with the edges — which is most of this command's value. Those defects can sit in an
/// in-memory graph indefinitely without anything else noticing.
#[tauri::command]
pub fn export_lineage_newick(state: State<'_, AppState>) -> Result<LineageNewickPayload, String> {
    let (nodes, relations) = state.engine.lineage_tracker.get_lineage_graph()?;
    let export =
        crate::evolution::newick::to_newick(&nodes, &relations).map_err(|e| e.to_string())?;
    Ok(LineageNewickPayload {
        trees: export.trees,
        dropped_parent_edges: export.dropped_parent_edges,
        roots: export.roots,
    })
}

/// The lineage reduced to the ancestry of `samples` (OSS-071).
///
/// **Read-only.** This shows what a compaction of the same sample set would keep; it does not
/// perform one. `LineageTracker::compact` is the mutating path, and the engine drives it on its own
/// schedule.
#[tauri::command]
pub fn get_simplified_lineage(
    state: State<'_, AppState>,
    samples: Vec<String>,
) -> Result<SimplifiedLineagePayload, String> {
    let (nodes, relations) = state.engine.lineage_tracker.get_lineage_graph()?;

    // Derived from the FULL graph, before anything is spliced. This ordering is the same one
    // `compact` documents and depends on: a compressed edge stands for a path, so deriving mutation
    // counts from the simplified graph would walk one edge where five reproductions happened and
    // return a smaller, entirely plausible, wrong number.
    let derived = crate::evolution::lineage::cumulative_mutations_from_edges(&nodes, &relations);

    let plan = crate::evolution::simplify::simplify(&nodes, &relations, &samples)
        .map_err(|e| e.to_string())?;

    Ok(SimplifiedLineagePayload {
        nodes: plan
            .nodes
            .iter()
            .map(|n| SimplifiedLineageNode {
                id: n.id.clone(),
                generation: n.generation,
                mutations_count: n
                    .cumulative_mutations
                    .or_else(|| derived.get(&n.id).copied())
                    .unwrap_or(0),
            })
            .collect(),
        edges: plan
            .edges
            .iter()
            .map(|e| SimplifiedLineageEdge {
                parent_id: e.parent_id.clone(),
                child_id: e.child_id.clone(),
                events: e.events,
                mutations: e.mutations,
                crossovers: e.crossovers,
            })
            .collect(),
        dropped_nodes: plan.dropped_nodes,
        compressed_nodes: plan.compressed_nodes,
    })
}
