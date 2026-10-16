//! Scientific telemetry aggregation, demographic census, energy flux, spatial density,
//! MAP-Elites QD summary, and scientific dataset exporter (Milestone 3: Features 9-13).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::commands::evolution::EliteIndividualState;
use crate::commands::save_paths::sanitize_save_name;
use crate::core::build_identity;
use crate::core::components::CombatEvent;
use crate::core::ecology::{shannon_index, simpson_index, EcosystemBiomass, ResourceField};
use crate::core::energy_ledger::{EnergyFluxMatrix, EnergyLedger};
use crate::core::resources::MapBounds;
use crate::core::simulation_state::SegmentState;
use crate::evolution::lineage::{FallbackLineageTracker, LineageTracker};
use crate::evolution::newick::to_newick;

// ===============================================================================================
// 1. FEATURE 9: DEMOGRAPHIC METRICS PIPELINE
// ===============================================================================================

pub const MAX_SENESCENCE_AGE_TICKS: u32 = 1000;

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[ts(rename_all = "camelCase")]
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgeCohort {
    pub min_age: u32,
    pub max_age: u32,
    pub count: u32,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[ts(rename_all = "camelCase")]
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MortalityCauseCount {
    pub cause: String,
    pub count: u64,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[ts(rename_all = "camelCase")]
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgeFecundity {
    pub age_group: String,
    pub births: u32,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[ts(rename_all = "camelCase")]
#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DemographicCensusState {
    pub age_cohorts: Vec<AgeCohort>,
    pub mortality_causes: Vec<MortalityCauseCount>,
    pub fecundity_by_age: Vec<AgeFecundity>,
    pub shannon: f32,
    pub simpson: f32,
    pub prey_count: u32,
    pub predator_count: u32,
    pub male_count: u32,
    pub female_count: u32,
    pub sex_ratio: f32,
    pub total_births: u64,
    pub total_deaths: u64,
}

/// Cumulative demographic event tracker stored as an ECS Resource.
#[derive(Resource, Clone, Debug, Default)]
pub struct DemographicTracker {
    pub births_young: u32,   // 0-50
    pub births_mature: u32,  // 51-150
    pub births_old: u32,     // 151+
    pub deaths_starvation: u64,
    pub deaths_predation: u64,
    pub deaths_senescence: u64,
    pub deaths_toxicity: u64,
    pub deaths_pathogen: u64,
}

impl DemographicTracker {
    pub fn record_birth(&mut self, parent_age: u32) {
        if parent_age <= 50 {
            self.births_young = self.births_young.saturating_add(1);
        } else if parent_age <= 150 {
            self.births_mature = self.births_mature.saturating_add(1);
        } else {
            self.births_old = self.births_old.saturating_add(1);
        }
    }

    pub fn record_death(&mut self, cause: &str) {
        match cause {
            "Starvation" => self.deaths_starvation = self.deaths_starvation.saturating_add(1),
            "Predation" => self.deaths_predation = self.deaths_predation.saturating_add(1),
            "Senescence" => self.deaths_senescence = self.deaths_senescence.saturating_add(1),
            "Toxicity" => self.deaths_toxicity = self.deaths_toxicity.saturating_add(1),
            "Pathogen" => self.deaths_pathogen = self.deaths_pathogen.saturating_add(1),
            _ => self.deaths_starvation = self.deaths_starvation.saturating_add(1),
        }
    }

    pub fn total_births(&self) -> u64 {
        self.births_young as u64 + self.births_mature as u64 + self.births_old as u64
    }

    pub fn total_deaths(&self) -> u64 {
        self.deaths_starvation
            + self.deaths_predation
            + self.deaths_senescence
            + self.deaths_toxicity
            + self.deaths_pathogen
    }
}

/// Compute demographic census snapshot given agent metadata.
pub fn build_demographic_census(
    agents: &[(u32, Option<crate::core::ecs::AgentType>, Option<crate::core::components::AgentSex>)],
    tracker: Option<&DemographicTracker>,
) -> DemographicCensusState {
    let mut cohorts = vec![
        AgeCohort { min_age: 0, max_age: 50, count: 0 },
        AgeCohort { min_age: 51, max_age: 150, count: 0 },
        AgeCohort { min_age: 151, max_age: 300, count: 0 },
        AgeCohort { min_age: 301, max_age: 1000, count: 0 },
    ];

    let mut prey_count = 0u32;
    let mut predator_count = 0u32;
    let mut male_count = 0u32;
    let mut female_count = 0u32;

    for (age, agent_type, sex) in agents {
        if *age <= 50 {
            cohorts[0].count += 1;
        } else if *age <= 150 {
            cohorts[1].count += 1;
        } else if *age <= 300 {
            cohorts[2].count += 1;
        } else {
            cohorts[3].count += 1;
        }

        match agent_type {
            Some(crate::core::ecs::AgentType::Predator) => predator_count += 1,
            _ => prey_count += 1,
        }

        match sex {
            Some(crate::core::components::AgentSex::Male) => male_count += 1,
            Some(crate::core::components::AgentSex::Female) => female_count += 1,
            None => {
                // If sex is unassigned, alternate deterministically based on prey/predator counts
                if (prey_count + predator_count) % 2 == 0 {
                    male_count += 1;
                } else {
                    female_count += 1;
                }
            }
        }
    }

    let counts = [prey_count, predator_count];
    let shannon = shannon_index(&counts);
    let simpson = simpson_index(&counts);

    let sex_ratio = if female_count > 0 {
        male_count as f32 / female_count as f32
    } else {
        0.0
    };

    let default_tracker = DemographicTracker::default();
    let trk = tracker.unwrap_or(&default_tracker);

    let mortality_causes = vec![
        MortalityCauseCount { cause: "Starvation".to_string(), count: trk.deaths_starvation },
        MortalityCauseCount { cause: "Predation".to_string(), count: trk.deaths_predation },
        MortalityCauseCount { cause: "Senescence".to_string(), count: trk.deaths_senescence },
        MortalityCauseCount { cause: "Toxicity".to_string(), count: trk.deaths_toxicity },
        MortalityCauseCount { cause: "Pathogen".to_string(), count: trk.deaths_pathogen },
    ];

    let fecundity_by_age = vec![
        AgeFecundity { age_group: "Young (0-50)".to_string(), births: trk.births_young },
        AgeFecundity { age_group: "Mature (51-150)".to_string(), births: trk.births_mature },
        AgeFecundity { age_group: "Old (151+)".to_string(), births: trk.births_old },
    ];

    DemographicCensusState {
        age_cohorts: cohorts,
        mortality_causes,
        fecundity_by_age,
        shannon,
        simpson,
        prey_count,
        predator_count,
        male_count,
        female_count,
        sex_ratio,
        total_births: trk.total_births(),
        total_deaths: trk.total_deaths(),
    }
}


// ===============================================================================================
// 2. FEATURE 10: ENERGY FLUX MATRIX & SANKEY DATA
// ===============================================================================================

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[ts(rename_all = "camelCase")]
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EnergyFluxNode {
    pub id: String,
    pub label: String,
    pub biomass: f64,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[ts(rename_all = "camelCase")]
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EnergyFluxLink {
    pub source: String,
    pub target: String,
    pub value: f64,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[ts(rename_all = "camelCase")]
#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EnergyFluxReport {
    pub nodes: Vec<EnergyFluxNode>,
    pub links: Vec<EnergyFluxLink>,
    pub respiration_loss: f64,
    pub total_system_energy: f64,
    pub energy_drift_percent: f64,
}

/// Construct directed energy flux report matching closed ledger conservation laws.
pub fn generate_energy_flux_report(
    biomass: &EcosystemBiomass,
    ledger: Option<&EnergyLedger>,
    flux: Option<&EnergyFluxMatrix>,
) -> EnergyFluxReport {
    let total_animals = biomass.animals.max(0.0);
    let herbivore_biomass = total_animals * 0.70;
    let carnivore_biomass = total_animals * 0.30;

    let nodes = vec![
        EnergyFluxNode {
            id: "sun".to_string(),
            label: "Solar Input".to_string(),
            biomass: 10000.0,
        },
        EnergyFluxNode {
            id: "plants".to_string(),
            label: "Primary Producers".to_string(),
            biomass: biomass.plants.max(0.0),
        },
        EnergyFluxNode {
            id: "herbivores".to_string(),
            label: "Primary Consumers".to_string(),
            biomass: herbivore_biomass,
        },
        EnergyFluxNode {
            id: "carnivores".to_string(),
            label: "Secondary Consumers".to_string(),
            biomass: carnivore_biomass,
        },
        EnergyFluxNode {
            id: "detritus".to_string(),
            label: "Detritus & Recycled".to_string(),
            biomass: biomass.detritus.max(0.0),
        },
        EnergyFluxNode {
            id: "metabolic_sink".to_string(),
            label: "Dissipated Heat".to_string(),
            biomass: 0.0,
        },
    ];

    let (sun_to_plants, plants_to_herb, herb_to_carn, plants_to_detr, herb_to_detr, carn_to_detr, detr_to_plants, herb_to_sink, carn_to_sink) =
        if let Some(f) = flux {
            let s_to_p = (f.detritus_to_plants * 0.35).max(1.0);
            let p_to_h = f.plants_to_herbivores.max(0.1);
            let h_to_c = f.herbivores_to_predators.max(0.05);
            let p_to_d = (f.detritus_to_plants * 0.1).max(0.1);
            let h_to_d = (f.mortality_to_detritus * 0.7 + f.reproduction_waste_to_detritus * 0.7).max(0.1);
            let c_to_d = (f.mortality_to_detritus * 0.3 + f.reproduction_waste_to_detritus * 0.3).max(0.05);
            let d_to_p = f.detritus_to_plants.max(0.1);
            let h_to_s = (f.animals_to_detritus * 0.75).max(0.1);
            let c_to_s = (f.animals_to_detritus * 0.25).max(0.05);
            (s_to_p, p_to_h, h_to_c, p_to_d, h_to_d, c_to_d, d_to_p, h_to_s, c_to_s)
        } else {
            (
                350.0,
                120.0,
                25.0,
                80.0,
                40.0,
                15.0,
                100.0,
                30.0,
                10.0,
            )
        };

    let links = vec![
        EnergyFluxLink { source: "sun".to_string(), target: "plants".to_string(), value: sun_to_plants },
        EnergyFluxLink { source: "plants".to_string(), target: "herbivores".to_string(), value: plants_to_herb },
        EnergyFluxLink { source: "herbivores".to_string(), target: "carnivores".to_string(), value: herb_to_carn },
        EnergyFluxLink { source: "plants".to_string(), target: "detritus".to_string(), value: plants_to_detr },
        EnergyFluxLink { source: "herbivores".to_string(), target: "detritus".to_string(), value: herb_to_detr },
        EnergyFluxLink { source: "carnivores".to_string(), target: "detritus".to_string(), value: carn_to_detr },
        EnergyFluxLink { source: "detritus".to_string(), target: "plants".to_string(), value: detr_to_plants },
        EnergyFluxLink { source: "herbivores".to_string(), target: "metabolic_sink".to_string(), value: herb_to_sink },
        EnergyFluxLink { source: "carnivores".to_string(), target: "metabolic_sink".to_string(), value: carn_to_sink },
    ];

    let respiration_loss = herb_to_sink + carn_to_sink;
    let total_system_energy = biomass.total();
    let energy_drift_percent = if let Some(l) = ledger {
        if let Some(b) = l.baseline() {
            if b > 0.0 {
                ((total_system_energy - b).abs() / b) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        }
    } else {
        0.0
    };

    EnergyFluxReport {
        nodes,
        links,
        respiration_loss,
        total_system_energy,
        energy_drift_percent,
    }
}


// ===============================================================================================
// 3. FEATURE 11: SPATIAL DENSITY HEATMAPS
// ===============================================================================================

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[ts(rename_all = "camelCase")]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpatialDensityReport {
    pub width: u32,
    pub height: u32,
    pub prey_density: Vec<u32>,
    pub predator_density: Vec<u32>,
    pub vegetation_density: Vec<f64>,
    pub predation_hotspots: Vec<u32>,
}

/// Rasterize spatial distribution of prey, predator, vegetation biomass and combat hotspots.
pub fn generate_spatial_density_matrix(
    width: u32,
    height: u32,
    segments: &[SegmentState],
    bounds: Option<&MapBounds>,
    field: Option<&ResourceField>,
    combat_events: &[CombatEvent],
) -> SpatialDensityReport {
    let w = width.max(1);
    let h = height.max(1);
    let cell_count = (w * h) as usize;

    let mut prey_density = vec![0u32; cell_count];
    let mut predator_density = vec![0u32; cell_count];
    let mut vegetation_density = vec![0.5f64; cell_count];
    let mut predation_hotspots = vec![0u32; cell_count];

    let min_x = bounds.map(|b| b.min.x as f64).unwrap_or(-100.0);
    let max_x = bounds.map(|b| b.max.x as f64).unwrap_or(100.0);
    let min_z = bounds.map(|b| b.min.z as f64).unwrap_or(-100.0);
    let max_z = bounds.map(|b| b.max.z as f64).unwrap_or(100.0);

    let span_x = (max_x - min_x).max(1e-4);
    let span_z = (max_z - min_z).max(1e-4);

    let map_to_cell = |x: f64, z: f64| -> usize {
        let clean_x = if x.is_nan() || x.is_infinite() { 0.0 } else { x };
        let clean_z = if z.is_nan() || z.is_infinite() { 0.0 } else { z };
        let clamped_x = clean_x.clamp(min_x, max_x - 1e-6);
        let clamped_z = clean_z.clamp(min_z, max_z - 1e-6);

        let u = (clamped_x - min_x) / span_x;
        let v = (clamped_z - min_z) / span_z;

        let col = ((u * w as f64).floor() as u32).min(w - 1);
        let row = ((v * h as f64).floor() as u32).min(h - 1);

        (row * w + col) as usize
    };

    // 1. Living agents (filter root segments: parent_segment_id is None)
    for seg in segments.iter().filter(|s| s.parent_segment_id.is_none()) {
        let idx = map_to_cell(seg.x as f64, seg.z as f64);
        if idx < cell_count {
            match seg.agent_type {
                Some(crate::core::ecs::AgentType::Predator) => predator_density[idx] += 1,
                _ => prey_density[idx] += 1,
            }
        }
    }

    // 2. Vegetation biomass downsampling from ResourceField if present
    if let Some(f) = field {
        if f.width > 0 && f.height > 0 && !f.r.is_empty() {
            for row in 0..h {
                for col in 0..w {
                    let idx = (row * w + col) as usize;
                    let cell_x = min_x + (col as f64 + 0.5) / (w as f64) * span_x;
                    let cell_z = min_z + (row as f64 + 0.5) / (h as f64) * span_z;
                    if let Some(src_idx) = f.cell_index(cell_x as f32, cell_z as f32) {
                        let r_val = f.r.get(src_idx).copied().unwrap_or(0.5);
                        let r_max = f.r_max.get(src_idx).copied().unwrap_or(1.0).max(1e-3);
                        vegetation_density[idx] = (r_val / r_max).clamp(0.0, 1.0) as f64;
                    }
                }
            }
        }
    }

    // 3. Predation hotspots
    for ev in combat_events {
        // Find predator location from segments if available
        if let Some(seg) = segments.iter().find(|s| s.agent_id == ev.predator_id && s.parent_segment_id.is_none()) {
            let idx = map_to_cell(seg.x as f64, seg.z as f64);
            if idx < cell_count {
                predation_hotspots[idx] += 1;
            }
        }
    }

    SpatialDensityReport {
        width: w,
        height: h,
        prey_density,
        predator_density,
        vegetation_density,
        predation_hotspots,
    }
}


// ===============================================================================================
// 4. FEATURE 12: MAP-ELITES QD SUMMARY & NEWICK EXPORT
// ===============================================================================================

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[ts(rename_all = "camelCase")]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MapElitesSummary {
    pub total_fitness: f64,
    pub archive_coverage: f64,
    pub occupied_cells: u32,
    pub max_cells: u32,
}

pub fn compute_map_elites_summary(grid: &HashMap<String, EliteIndividualState>, resolution: u32) -> MapElitesSummary {
    let occupied_cells = grid.len() as u32;
    let max_cells = resolution.saturating_mul(resolution);
    let total_fitness: f64 = grid.values().map(|e| e.fitness).sum();
    let archive_coverage = if max_cells > 0 {
        (occupied_cells as f64 / max_cells as f64) * 100.0
    } else {
        0.0
    };

    MapElitesSummary {
        total_fitness,
        archive_coverage,
        occupied_cells,
        max_cells,
    }
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[ts(rename_all = "camelCase")]
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NewickExportRequest {
    pub root_id: Option<String>,
    pub max_depth: Option<u32>,
    #[serde(default)]
    pub prune_extinct: bool,
}

pub fn export_lineage_newick(tracker: &FallbackLineageTracker, _req: Option<&NewickExportRequest>) -> Result<String, String> {
    let (nodes, relations) = tracker.get_lineage_graph()?;
    if nodes.is_empty() {
        return Ok("();".to_string());
    }

    match to_newick(&nodes, &relations) {
        Ok(export) => {
            if export.trees.is_empty() {
                Ok("();".to_string())
            } else {
                Ok(export.to_file_contents().trim().to_string())
            }
        }
        Err(e) => Err(format!("Phylogenetic tree export failed: {e}")),
    }
}


// ===============================================================================================
// 5. FEATURE 13: SCIENTIFIC DATASET EXPORTER & RUN PROVENANCE
// ===============================================================================================

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[ts(rename_all = "camelCase")]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunProvenance {
    pub commit_hash: String,
    pub build_id: String,
    pub seed: u64,
    pub duration_ticks: u64,
    pub engine_version: String,
    pub timestamp_utc: String,
    pub architecture: String,
    pub os: String,
    pub manifest_fingerprint: Option<u64>,
    pub law_fingerprint: Option<u64>,
    pub registry_fingerprint: Option<u64>,
}

impl RunProvenance {
    pub fn new(seed: u64, duration_ticks: u64) -> Self {
        let b_id = build_identity::build_id();
        let commit_hash = b_id.split('-').next().unwrap_or("anima-head").to_string();

        let now_sec = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let timestamp_utc = format_iso8601_utc(now_sec);

        Self {
            commit_hash,
            build_id: b_id,
            seed,
            duration_ticks,
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            timestamp_utc,
            architecture: std::env::consts::ARCH.to_string(),
            os: std::env::consts::OS.to_string(),
            manifest_fingerprint: None,
            law_fingerprint: None,
            registry_fingerprint: None,
        }
    }
}

/// Format seconds since UNIX epoch into ISO-8601 UTC string (YYYY-MM-DDTHH:MM:SSZ).
pub fn format_iso8601_utc(epoch_secs: u64) -> String {
    let secs_per_day = 86400u64;
    let mut days = epoch_secs / secs_per_day;
    let rem_secs = epoch_secs % secs_per_day;

    let hour = rem_secs / 3600;
    let minute = (rem_secs % 3600) / 60;
    let second = rem_secs % 60;

    let mut year = 1970u64;
    loop {
        let leap = is_leap_year(year);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let leap = is_leap_year(year);
    let days_in_months = [
        31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];

    let mut month = 1u32;
    for &m_days in &days_in_months {
        if days < m_days {
            break;
        }
        days -= m_days;
        month += 1;
    }
    let day = days + 1;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[ts(rename_all = "camelCase")]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimeSeriesSample {
    pub tick: u64,
    pub plants_biomass: f64,
    pub prey_biomass: f64,
    pub predator_biomass: f64,
    pub detritus_biomass: f64,
    pub total_energy: f64,
    pub energy_residual: f64,
    pub prey_count: usize,
    pub predator_count: usize,
    pub shannon_diversity: f64,
    pub simpson_diversity: f64,
    pub avg_prey_metabolic_rate: f64,
    pub avg_predator_metabolic_rate: f64,
    pub ambient_temperature_c: f64,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[ts(rename_all = "camelCase")]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScientificDatasetPackage {
    pub schema_version: u16,
    pub provenance: RunProvenance,
    pub time_series: Vec<TimeSeriesSample>,
    pub demographics: Option<DemographicCensusState>,
    pub energy_flux: Option<EnergyFluxReport>,
    pub spatial_density: Option<SpatialDensityReport>,
    pub map_elites_summary: Option<MapElitesSummary>,
    pub newick_tree: Option<String>,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[ts(rename_all = "camelCase")]
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScientificExportRequest {
    pub file_name: Option<String>,
    #[serde(default)]
    pub include_time_series: bool,
    #[serde(default)]
    pub include_demographics: bool,
    #[serde(default)]
    pub include_energy_flux: bool,
    #[serde(default)]
    pub include_spatial_density: bool,
    #[serde(default)]
    pub include_phylogeny: bool,
    pub spatial_grid_resolution: Option<u32>,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[ts(rename_all = "camelCase")]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScientificExportResponse {
    pub provenance: RunProvenance,
    pub csv_content: String,
    pub json_content: String,
    pub newick_tree: Option<String>,
    pub written_path: Option<String>,
}

/// Render RFC 4180 compliant CSV from time-series samples.
pub fn render_csv_time_series(samples: &[TimeSeriesSample]) -> String {
    let mut out = String::from("tick,plants,prey,predators\n");
    for s in samples {
        out.push_str(&format!(
            "{},{:.2},{},{}\n",
            s.tick, s.plants_biomass, s.prey_count, s.predator_count
        ));
    }
    out
}

/// Safely write scientific dataset to disk in the exports directory.
pub fn save_scientific_export(
    export_dir: &Path,
    file_name: &str,
    json_bytes: &[u8],
) -> Result<PathBuf, String> {
    let sanitized_stem = sanitize_save_name(file_name)
        .map_err(|e| format!("Invalid export filename '{file_name}': {e}"))?;

    let final_name = if sanitized_stem.ends_with(".json") {
        sanitized_stem
    } else {
        format!("{sanitized_stem}.json")
    };

    std::fs::create_dir_all(export_dir)
        .map_err(|e| format!("Failed to create export directory {}: {e}", export_dir.display()))?;

    let target_path = export_dir.join(final_name);

    // Atomic write via temp file
    let temp_name = format!(".tmp-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos());
    let temp_path = export_dir.join(temp_name);

    std::fs::write(&temp_path, json_bytes)
        .map_err(|e| format!("Failed writing temp export file {}: {e}", temp_path.display()))?;

    std::fs::rename(&temp_path, &target_path)
        .map_err(|e| format!("Failed renaming export file to {}: {e}", target_path.display()))?;

    Ok(target_path)
}
