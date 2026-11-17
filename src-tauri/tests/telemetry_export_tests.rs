//! Verification tests for Milestone 3 (Features 9-13):
//! 1. Demographic Metrics Pipeline (F9)
//! 2. Energy Flux Matrix & Sankey Data (F10)
//! 3. Spatial Density Heatmaps (F11)
//! 4. MAP-Elites QD Summary & Newick Export (F12)
//! 5. Scientific Dataset Exporter & Run Provenance (F13)

use std::collections::HashMap;

use anima_domain::energy::{EcosystemBiomass, EnergyFluxMatrix, EnergyLedger};
use anima_engine_lib::commands::evolution::EliteIndividualState;
use anima_engine_lib::core::components::{AgentSex, CombatEvent};
use anima_engine_lib::core::ecs::AgentType;
use anima_engine_lib::core::resources::MapBounds;
use anima_engine_lib::core::simulation_state::SegmentState;
use anima_engine_lib::core::telemetry_export::*;
use anima_engine_lib::evolution::genotype::MorphologyGenotype;
use anima_engine_lib::evolution::lineage::{FallbackLineageTracker, LineageTracker, RelationType};

// ===============================================================================================
// 1. FEATURE 9: DEMOGRAPHIC METRICS PIPELINE
// ===============================================================================================

#[test]
fn test_demographic_census_empty_population() {
    let agents = vec![];
    let tracker = DemographicTracker::default();
    let census = build_demographic_census(&agents, Some(&tracker));

    assert_eq!(census.prey_count, 0);
    assert_eq!(census.predator_count, 0);
    assert_eq!(census.male_count, 0);
    assert_eq!(census.female_count, 0);
    assert_eq!(census.sex_ratio, 0.0);
    assert!(!census.shannon.is_nan());
    assert!(!census.simpson.is_nan());
    assert_eq!(census.shannon, 0.0);
    assert_eq!(census.simpson, 0.0);
    assert_eq!(census.total_births, 0);
    assert_eq!(census.total_deaths, 0);

    for cohort in &census.age_cohorts {
        assert_eq!(cohort.count, 0);
    }
}

#[test]
fn test_demographic_census_unisex_population() {
    // 10 males, 0 females -> sex_ratio must be 0.0 (no division by zero panic)
    let agents: Vec<(u32, Option<AgentType>, Option<AgentSex>)> = (0..10)
        .map(|i| (i * 10, Some(AgentType::Prey), Some(AgentSex::Male)))
        .collect();
    let tracker = DemographicTracker::default();
    let census = build_demographic_census(&agents, Some(&tracker));

    assert_eq!(census.male_count, 10);
    assert_eq!(census.female_count, 0);
    assert_eq!(census.sex_ratio, 0.0);
    assert!(!census.sex_ratio.is_nan());
    assert_eq!(census.prey_count, 10);
    assert_eq!(census.predator_count, 0);
}

#[test]
fn test_demographic_census_cohorts_and_fecundity() {
    let mut tracker = DemographicTracker::default();
    tracker.record_birth(25); // young
    tracker.record_birth(100); // mature
    tracker.record_birth(200); // old
    tracker.record_death("Starvation");
    tracker.record_death("Predation");
    tracker.record_death("Senescence");

    let agents = vec![
        (10, Some(AgentType::Prey), Some(AgentSex::Male)), // cohort 0-50
        (45, Some(AgentType::Prey), Some(AgentSex::Female)), // cohort 0-50
        (80, Some(AgentType::Predator), Some(AgentSex::Male)), // cohort 51-150
        (200, Some(AgentType::Prey), Some(AgentSex::Female)), // cohort 151-300
        (500, Some(AgentType::Predator), Some(AgentSex::Female)), // cohort 301-1000
    ];

    let census = build_demographic_census(&agents, Some(&tracker));

    assert_eq!(census.age_cohorts[0].count, 2); // 0-50
    assert_eq!(census.age_cohorts[1].count, 1); // 51-150
    assert_eq!(census.age_cohorts[2].count, 1); // 151-300
    assert_eq!(census.age_cohorts[3].count, 1); // 301-1000

    assert_eq!(census.prey_count, 3);
    assert_eq!(census.predator_count, 2);
    assert_eq!(census.male_count, 2);
    assert_eq!(census.female_count, 3);
    assert!((census.sex_ratio - (2.0 / 3.0)).abs() < 1e-4);

    assert_eq!(census.total_births, 3);
    assert_eq!(census.total_deaths, 3);

    let starv = census
        .mortality_causes
        .iter()
        .find(|m| m.cause == "Starvation")
        .unwrap();
    assert_eq!(starv.count, 1);
    let pred = census
        .mortality_causes
        .iter()
        .find(|m| m.cause == "Predation")
        .unwrap();
    assert_eq!(pred.count, 1);
    let sen = census
        .mortality_causes
        .iter()
        .find(|m| m.cause == "Senescence")
        .unwrap();
    assert_eq!(sen.count, 1);

    let y_fec = census
        .fecundity_by_age
        .iter()
        .find(|f| f.age_group.contains("0-50"))
        .unwrap();
    assert_eq!(y_fec.births, 1);
    let m_fec = census
        .fecundity_by_age
        .iter()
        .find(|f| f.age_group.contains("51-150"))
        .unwrap();
    assert_eq!(m_fec.births, 1);
    let o_fec = census
        .fecundity_by_age
        .iter()
        .find(|f| f.age_group.contains("151+"))
        .unwrap();
    assert_eq!(o_fec.births, 1);
}

// ===============================================================================================
// 2. FEATURE 10: ENERGY FLUX MATRIX & SANKEY DATA
// ===============================================================================================

#[test]
fn test_energy_flux_report_generation() {
    let biomass = EcosystemBiomass {
        detritus: 1500.0,
        plants: 3000.0,
        animals: 1200.0,
        dormant: 0.0,
    };
    let mut ledger = EnergyLedger::default();
    ledger.lock_baseline(biomass.total());

    let flux = EnergyFluxMatrix {
        detritus_to_plants: 350.0,
        plants_to_herbivores: 120.0,
        herbivores_to_predators: 30.0,
        animals_to_detritus: 50.0,
        mortality_to_detritus: 40.0,
        reproduction_waste_to_detritus: 10.0,
        ..Default::default()
    };

    let report = generate_energy_flux_report(&biomass, Some(&ledger), Some(&flux));

    // The nodes are the three stocks the closed ledger holds. A "sun" node of 10000 EU and a
    // 70/30 split of the animals into herbivores and carnivores used to be added here, and nothing
    // in the simulation measures either: the ledger is closed, so there is no solar input, and it
    // holds one animal pool.
    let ids: Vec<&str> = report.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, vec!["plants", "animals", "detritus"]);
    assert_eq!(report.total_system_energy, 5700.0);
    assert!(report.energy_drift_percent < 0.01);

    let node = |id: &str| report.nodes.iter().find(|n| n.id == id).unwrap().biomass;
    assert_eq!(node("plants"), 3000.0);
    assert_eq!(node("animals"), 1200.0);
    assert_eq!(node("detritus"), 1500.0);
    assert_eq!(
        node("plants") + node("animals") + node("detritus"),
        report.total_system_energy
    );

    // Every link is a field of the matrix, summed where several fields share a source and a target,
    // and nothing else: no floor, no share of one field attributed to another.
    let link = |source: &str, target: &str| -> f64 {
        report
            .links
            .iter()
            .find(|l| l.source == source && l.target == target)
            .unwrap_or_else(|| panic!("no {source} -> {target} link"))
            .value
    };
    assert_eq!(report.links.len(), 4);
    assert_eq!(link("detritus", "plants"), 350.0);
    assert_eq!(link("plants", "animals"), 120.0);
    assert_eq!(link("animals", "detritus"), 50.0 + 40.0 + 10.0);
    assert_eq!(link("detritus", "animals"), 0.0);
    assert_eq!(report.respiration_loss, 50.0);
}

#[test]
fn test_energy_flux_without_a_matrix_is_reported_as_unmeasured() {
    let biomass = EcosystemBiomass {
        detritus: 1500.0,
        plants: 3000.0,
        animals: 1200.0,
        dormant: 0.0,
    };
    let mut ledger = EnergyLedger::default();
    ledger.lock_baseline(biomass.total());

    // Nothing writes the flux matrix, so in the running app this is the only case there is. It used to
    // return nine hard-coded flows (350, 120, 25, 80, 40, 15, 100, 30, 10) that looked like a
    // measurement. With no matrix there is no diagram, and the two numbers that are measured (the
    // closed total and its drift from the locked baseline) are still reported.
    let report = generate_energy_flux_report(&biomass, Some(&ledger), None);

    assert!(
        report.nodes.is_empty() && report.links.is_empty(),
        "an unmeasured flux reported {} nodes and {} links",
        report.nodes.len(),
        report.links.len()
    );
    assert_eq!(report.total_system_energy, 5700.0);
    assert_eq!(report.energy_drift_percent, 0.0);
}

#[test]
fn test_energy_flux_links_have_no_floors_and_no_invented_shares() {
    let biomass = EcosystemBiomass {
        detritus: 10.0,
        plants: 20.0,
        animals: 1000.0,
        dormant: 0.0,
    };
    let ledger = EnergyLedger::default();

    // A quiet world: the matrix is all zeros. A flow that measured zero is zero. The old code raised
    // each link to an invented minimum (1.0, 0.1, 0.05) and carved shares (0.35, 0.1, 0.7, 0.3, 0.75,
    // 0.25) out of one field to feed another.
    let report =
        generate_energy_flux_report(&biomass, Some(&ledger), Some(&EnergyFluxMatrix::default()));

    assert!(!report.links.is_empty());
    for l in &report.links {
        assert_eq!(
            l.value, 0.0,
            "{} -> {} reports {} for a flow the matrix recorded as zero",
            l.source, l.target, l.value
        );
    }
    assert!(report.nodes.iter().all(|n| n.id != "sun"));
    assert_eq!(report.respiration_loss, 0.0);

    // The animals are one pool, exactly as the ledger holds it: no 70/30 split.
    let animals = report.nodes.iter().find(|n| n.id == "animals").unwrap();
    assert_eq!(animals.biomass, 1000.0);
}

#[test]
fn test_trophic_biomass_counts_each_agent_once_by_role() {
    let seg =
        |agent_id: u32, segment_id: u32, agent_type: Option<AgentType>, energy: f32| SegmentState {
            agent_id,
            segment_id,
            agent_type,
            energy,
            ..Default::default()
        };
    let segments = vec![
        // A three-segment prey body holding 40 EU: every segment repeats the agent's whole reserve.
        seg(1, 0, Some(AgentType::Prey), 40.0),
        seg(1, 1, Some(AgentType::Prey), 40.0),
        seg(1, 2, Some(AgentType::Prey), 40.0),
        seg(2, 0, Some(AgentType::Prey), 10.0),
        seg(3, 0, Some(AgentType::Predator), 30.0),
        seg(3, 1, Some(AgentType::Predator), 30.0),
        // Neither role: in neither total.
        seg(4, 0, None, 500.0),
        // A reserve that is not a number, or is negative, counts as nothing.
        seg(5, 0, Some(AgentType::Prey), f32::NAN),
        seg(6, 0, Some(AgentType::Predator), -5.0),
    ];

    let (prey, predator) = trophic_biomass_from_segments(&segments);
    assert_eq!(prey, 50.0, "prey agents 1 and 2 hold 40 + 10 EU");
    assert_eq!(predator, 30.0, "predator agent 3 holds 30 EU, counted once");
    // Not a fixed share of the total: that is the 70/30 the export used to invent.
    assert_ne!(prey / (prey + predator), 0.7);

    assert_eq!(trophic_biomass_from_segments(&[]), (0.0, 0.0));
}

// ===============================================================================================
// 3. FEATURE 11: SPATIAL DENSITY HEATMAPS
// ===============================================================================================

#[test]
fn test_spatial_density_matrix_conservation() {
    let segments = vec![
        SegmentState {
            agent_id: 1,
            segment_id: 0,
            parent_segment_id: None,
            agent_type: Some(AgentType::Prey),
            x: -50.0,
            y: 0.0,
            z: -50.0,
            energy: 100.0,
            ..Default::default()
        },
        SegmentState {
            agent_id: 1,
            segment_id: 1,
            parent_segment_id: Some(0), // child segment (should not be double counted)
            agent_type: Some(AgentType::Prey),
            x: -50.0,
            y: 0.0,
            z: -50.0,
            energy: 100.0,
            ..Default::default()
        },
        SegmentState {
            agent_id: 2,
            segment_id: 0,
            parent_segment_id: None,
            agent_type: Some(AgentType::Prey),
            x: 20.0,
            y: 0.0,
            z: 30.0,
            energy: 80.0,
            ..Default::default()
        },
        SegmentState {
            agent_id: 3,
            segment_id: 0,
            parent_segment_id: None,
            agent_type: Some(AgentType::Predator),
            x: 80.0,
            y: 0.0,
            z: -80.0,
            energy: 120.0,
            ..Default::default()
        },
    ];

    let bounds = MapBounds {
        min: glam::Vec3::new(-100.0, 0.0, -100.0),
        max: glam::Vec3::new(100.0, 0.0, 100.0),
    };

    let combat_events = vec![CombatEvent {
        predator_id: 3,
        prey_id: 1,
        damage: 50.0,
        energy_transferred: 10.0,
    }];

    let report =
        generate_spatial_density_matrix(16, 16, &segments, Some(&bounds), None, &combat_events);

    assert_eq!(report.width, 16);
    assert_eq!(report.height, 16);
    assert_eq!(report.prey_density.len(), 256);
    assert_eq!(report.predator_density.len(), 256);

    // Conservation check: sum of densities equals distinct agent count
    let sum_prey: u32 = report.prey_density.iter().sum();
    let sum_pred: u32 = report.predator_density.iter().sum();
    let sum_hotspots: u32 = report.predation_hotspots.iter().sum();

    assert_eq!(sum_prey, 2);
    assert_eq!(sum_pred, 1);
    assert_eq!(sum_hotspots, 1);
}

// ===============================================================================================
// 4. FEATURE 12: MAP-ELITES QD SUMMARY & NEWICK EXPORT
// ===============================================================================================

#[test]
fn test_map_elites_summary_computation() {
    let mut grid = HashMap::new();
    grid.insert(
        "0,0".to_string(),
        EliteIndividualState {
            fitness: 120.0,
            features: vec![0.1, 0.2],
        },
    );
    grid.insert(
        "1,1".to_string(),
        EliteIndividualState {
            fitness: 80.0,
            features: vec![0.5, 0.6],
        },
    );

    let summary = compute_map_elites_summary(&grid, 10);

    assert_eq!(summary.occupied_cells, 2);
    assert_eq!(summary.max_cells, 100);
    assert_eq!(summary.total_fitness, 200.0);
    assert_eq!(summary.archive_coverage, 2.0);
}

#[test]
fn test_newick_lineage_export() {
    let tracker = FallbackLineageTracker::new("bolt://localhost:7687", "neo4j", "pass");

    // Empty tracker returns "();"
    let empty_tree = export_lineage_newick(&tracker, None).unwrap();
    assert_eq!(empty_tree, "();");

    // Record sample nodes and relationships
    tracker
        .add_root("root".to_string(), MorphologyGenotype::default())
        .unwrap();
    tracker
        .add_reproduction(
            "child_1".to_string(),
            1,
            MorphologyGenotype::default(),
            vec!["root".to_string()],
            RelationType::Mutate,
        )
        .unwrap();
    tracker
        .add_reproduction(
            "child_2".to_string(),
            1,
            MorphologyGenotype::default(),
            vec!["root".to_string()],
            RelationType::Mutate,
        )
        .unwrap();

    let tree = export_lineage_newick(&tracker, None).unwrap();
    assert!(tree.ends_with(';'));
    assert!(tree.contains("root") || tree.contains("child_1"));
}

// ===============================================================================================
// 5. FEATURE 13: SCIENTIFIC DATASET EXPORTER & RUN PROVENANCE
// ===============================================================================================

#[test]
fn test_run_provenance_and_iso8601() {
    let prov = RunProvenance::new(12345, 500);

    assert_eq!(prov.seed, 12345);
    assert_eq!(prov.duration_ticks, 500);
    assert!(!prov.build_id.is_empty());
    assert!(!prov.commit_hash.is_empty());
    assert!(!prov.engine_version.is_empty());
    assert!(prov.timestamp_utc.ends_with('Z'));
    assert!(prov.timestamp_utc.contains('T'));

    // Test ISO8601 converter
    let formatted = format_iso8601_utc(1700000000);
    assert_eq!(formatted, "2023-11-14T22:13:20Z");
}

#[test]
fn test_render_csv_time_series() {
    let samples = vec![
        TimeSeriesSample {
            tick: 0,
            plants_biomass: 3000.0,
            prey_biomass: 500.0,
            predator_biomass: 200.0,
            detritus_biomass: 1000.0,
            total_energy: 4700.0,
            prey_count: 50,
            predator_count: 10,
            shannon_diversity: 0.69,
            simpson_diversity: 0.50,
        },
        TimeSeriesSample {
            tick: 100,
            plants_biomass: 3200.0,
            prey_biomass: 600.0,
            predator_biomass: 180.0,
            detritus_biomass: 900.0,
            total_energy: 4880.0,
            prey_count: 60,
            predator_count: 9,
            shannon_diversity: 0.70,
            simpson_diversity: 0.52,
        },
    ];

    let csv = render_csv_time_series(&samples);
    assert!(csv.starts_with("tick,plants,prey,predators\n"));
    assert!(csv.contains("0,3000.00,50,10\n"));
    assert!(csv.contains("100,3200.00,60,9\n"));
}

#[test]
fn test_save_scientific_export_security_and_writing() {
    let tmp_dir = std::env::temp_dir().join(format!("anima_test_export_{}", std::process::id()));

    // Valid save
    let path = save_scientific_export(&tmp_dir, "run_experiment_1", b"{\"valid\": true}").unwrap();
    assert!(path.exists());
    assert_eq!(path.file_name().unwrap(), "run_experiment_1.json");

    // Path traversal attempt must fail
    assert!(save_scientific_export(&tmp_dir, "../../../evil", b"{}").is_err());
    assert!(save_scientific_export(&tmp_dir, "dir/name", b"{}").is_err());
    assert!(save_scientific_export(&tmp_dir, "CON", b"{}").is_err());

    let _ = std::fs::remove_dir_all(&tmp_dir);
}
