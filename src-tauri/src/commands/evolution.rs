use crate::evolution::lineage::LineageTracker;
use crate::AppState;
use tauri::State;

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

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LineageNodePayload {
    pub id: String,
    pub generation: u32,
    pub parent_id: Option<String>,
    pub fitness: f64,
    pub mutations_count: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LineageLinkPayload {
    pub source: String,
    pub target: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LineageGraphPayload {
    pub nodes: Vec<LineageNodePayload>,
    pub links: Vec<LineageLinkPayload>,
    pub db_connected: bool,
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
            .push((rel.source_id.clone(), rel.relation_type));
    }

    let mut mutations_map = std::collections::HashMap::new();

    fn get_mutations_count(
        node_id: &str,
        parent_map: &std::collections::HashMap<
            String,
            Vec<(String, crate::evolution::lineage::RelationType)>,
        >,
        memo: &mut std::collections::HashMap<String, u32>,
    ) -> u32 {
        if let Some(&val) = memo.get(node_id) {
            return val;
        }
        let mut count = 0;
        if let Some(parents) = parent_map.get(node_id) {
            let mut max_parent_mutations = 0;
            let mut is_mutation = false;
            for (parent_id, rel_type) in parents {
                let parent_mut = get_mutations_count(parent_id, parent_map, memo);
                if parent_mut > max_parent_mutations {
                    max_parent_mutations = parent_mut;
                }
                if *rel_type == crate::evolution::lineage::RelationType::Mutate {
                    is_mutation = true;
                }
            }
            count = max_parent_mutations + if is_mutation { 1 } else { 0 };
        }
        memo.insert(node_id.to_string(), count);
        count
    }

    for node in &nodes {
        let parent_id = parent_map
            .get(&node.id)
            .and_then(|parents| parents.first())
            .map(|(p_id, _)| p_id.clone());

        let mutations_count = get_mutations_count(&node.id, &parent_map, &mut mutations_map);
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
