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

pub const MAX_EVOLUTION_GRID_RESOLUTION: u32 = 10_000;
pub const MAX_SELECTION_BIAS: f64 = 1_024.0;

impl EvolutionSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !self.mutation_rate.is_finite()
            || !(0.0..=1.0).contains(&self.mutation_rate)
            || !self.selection_bias.is_finite()
            || self.selection_bias <= 0.0
            || self.selection_bias > MAX_SELECTION_BIAS
            || self.grid_resolution == 0
            || self.grid_resolution > MAX_EVOLUTION_GRID_RESOLUTION
        {
            return Err(format!(
                "invalid evolution settings: mutation_rate must be finite in [0, 1], \
                 selection_bias must be finite in (0, {MAX_SELECTION_BIAS}], and grid_resolution \
                 must be in [1, {MAX_EVOLUTION_GRID_RESOLUTION}]"
            ));
        }
        Ok(())
    }
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
    settings.validate()?;
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

pub use crate::core::telemetry_export::{MapElitesSummary, NewickExportRequest};

impl MapElitesGridState {
    pub fn compute_summary(&self) -> MapElitesSummary {
        crate::core::telemetry_export::compute_map_elites_summary(&self.grid, self.grid_resolution)
    }
}

#[tauri::command]
pub fn export_phylogenetic_tree(
    state: State<'_, AppState>,
    req: Option<NewickExportRequest>,
) -> Result<String, String> {
    crate::core::telemetry_export::export_lineage_newick(
        &state.engine.lineage_tracker,
        req.as_ref(),
    )
}

#[cfg(test)]
mod tests {
    use super::EvolutionSettings;

    #[test]
    fn evolution_settings_reject_values_that_can_panic_or_destabilize_the_worker() {
        let valid = EvolutionSettings {
            mutation_rate: 0.2,
            selection_bias: 1.2,
            grid_resolution: 30,
        };
        assert!(valid.validate().is_ok());

        for invalid in [
            EvolutionSettings {
                mutation_rate: f64::NAN,
                ..valid.clone()
            },
            EvolutionSettings {
                selection_bias: f64::INFINITY,
                ..valid.clone()
            },
            EvolutionSettings {
                selection_bias: 1_025.0,
                ..valid.clone()
            },
            EvolutionSettings {
                grid_resolution: 0,
                ..valid.clone()
            },
            EvolutionSettings {
                grid_resolution: 10_001,
                ..valid
            },
        ] {
            assert!(invalid.validate().is_err(), "{invalid:?}");
        }
    }
}
