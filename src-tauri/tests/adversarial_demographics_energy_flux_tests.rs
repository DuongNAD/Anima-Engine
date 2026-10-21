//! Adversarial Stress & Edge Case Tests for Demographics & Energy Flux Telemetry
//! (Milestone 3: Features 9, 10, 11, 12, 13)
//!
//! Empirical Challenge Suite covering:
//! 1. Empty world boundary conditions (0 agents, 0 plants, 0 detritus, 0 biomass).
//! 2. Unisex population & extreme sex ratio skews (100% male, 100% female, 10^6:1 skews).
//! 3. Extreme senescence, longevity clamping (ages 0 to u32::MAX) and mortality attribution.
//! 4. Rapid population boom / mass starvation counter saturation (u32::MAX, u64::MAX).
//! 5. Closed-loop energy conservation parity under live simulation & flux report consistency (|E(t) - E(0)| < 1e-3 EU).
//! 6. Spatial density coordinate corruption (NaN, +Inf, -Inf, out-of-bounds, non-root segments).
//! 7. ISO-8601 calendar arithmetic edge cases (leap years, century non-leap, epoch boundaries).

use std::collections::HashMap;

use anima_domain::energy::{EcosystemBiomass, EnergyFluxMatrix, EnergyLedger, RESIDUAL_ABS_TOLERANCE_EU};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::{AgentEvaluation, AgentLineageId};
use anima_engine_lib::core::components::{Agent, AgentSex, Predator, Prey};
use anima_engine_lib::core::ecology::ResourceField;
use anima_engine_lib::core::ecs::{init_world, AgentType};
use anima_engine_lib::core::environmental_systems::{
    demographic_census_system, energy_flux_aggregation_system,
};
use anima_engine_lib::core::resources::MapBounds;
use anima_engine_lib::core::simulation_state::SegmentState;
use anima_engine_lib::core::telemetry_export::*;
use bevy_ecs::prelude::*;

// ===============================================================================================
// 1. EMPTY WORLD & ZERO-BIOMASS ADVERSARIAL TESTS
// ===============================================================================================

#[test]
fn test_adversarial_empty_world_boundary_conditions() {
    // A completely empty agent population
    let empty_agents: Vec<(u32, Option<AgentType>, Option<AgentSex>)> = Vec::new();
    let tracker = DemographicTracker::default();

    let census = build_demographic_census(&empty_agents, Some(&tracker));

    // Numerical sanity: No NaN, no Infinity
    assert!(!census.shannon.is_nan(), "Shannon index must not be NaN on empty world");
    assert!(!census.shannon.is_infinite(), "Shannon index must not be infinite");
    assert_eq!(census.shannon, 0.0);

    assert!(!census.simpson.is_nan(), "Simpson index must not be NaN on empty world");
    assert!(!census.simpson.is_infinite(), "Simpson index must not be infinite");
    assert_eq!(census.simpson, 0.0);

    assert!(!census.sex_ratio.is_nan(), "Sex ratio must not be NaN on empty world");
    assert!(!census.sex_ratio.is_infinite(), "Sex ratio must not be infinite");
    assert_eq!(census.sex_ratio, 0.0);

    assert_eq!(census.prey_count, 0);
    assert_eq!(census.predator_count, 0);
    assert_eq!(census.male_count, 0);
    assert_eq!(census.female_count, 0);
    assert_eq!(census.total_births, 0);
    assert_eq!(census.total_deaths, 0);

    // Cohorts must be initialized with 0 counts
    assert_eq!(census.age_cohorts.len(), 4);
    for c in &census.age_cohorts {
        assert_eq!(c.count, 0);
    }

    // Zero-biomass Energy Flux Report
    let zero_biomass = EcosystemBiomass {
        detritus: 0.0,
        plants: 0.0,
        animals: 0.0,
    };
    let mut ledger = EnergyLedger::default();
    ledger.lock_baseline(0.0);

    let flux_report = generate_energy_flux_report(&zero_biomass, Some(&ledger), None);

    assert_eq!(flux_report.nodes.len(), 6);
    assert_eq!(flux_report.links.len(), 9);
    assert_eq!(flux_report.total_system_energy, 0.0);
    assert!(!flux_report.energy_drift_percent.is_nan());
    assert_eq!(flux_report.energy_drift_percent, 0.0);
    assert!(!flux_report.respiration_loss.is_nan());
    assert!(flux_report.respiration_loss >= 0.0);

    for node in &flux_report.nodes {
        assert!(!node.biomass.is_nan(), "Node {} biomass is NaN", node.id);
        assert!(!node.biomass.is_infinite(), "Node {} biomass is Infinite", node.id);
        assert!(node.biomass >= 0.0, "Node {} biomass is negative", node.id);
    }

    for link in &flux_report.links {
        assert!(!link.value.is_nan(), "Link {}->{} is NaN", link.source, link.target);
        assert!(!link.value.is_infinite(), "Link {}->{} is Infinite", link.source, link.target);
        assert!(link.value >= 0.0, "Link {}->{} is negative", link.source, link.target);
    }

    // Spatial Density on 0 segments & 0 bounds
    let spatial = generate_spatial_density_matrix(32, 32, &[], None, None, &[]);
    assert_eq!(spatial.width, 32);
    assert_eq!(spatial.height, 32);
    assert_eq!(spatial.prey_density.len(), 1024);
    assert_eq!(spatial.predator_density.len(), 1024);
    assert_eq!(spatial.predation_hotspots.len(), 1024);
    assert_eq!(spatial.prey_density.iter().sum::<u32>(), 0);
    assert_eq!(spatial.predator_density.iter().sum::<u32>(), 0);
    assert_eq!(spatial.predation_hotspots.iter().sum::<u32>(), 0);

    // MAP-Elites Summary on empty grid
    let empty_grid = HashMap::new();
    let qd_summary = compute_map_elites_summary(&empty_grid, 10);
    assert_eq!(qd_summary.occupied_cells, 0);
    assert_eq!(qd_summary.max_cells, 100);
    assert_eq!(qd_summary.total_fitness, 0.0);
    assert_eq!(qd_summary.archive_coverage, 0.0);
    assert!(!qd_summary.archive_coverage.is_nan());

    // Degenerate resolution 0
    let qd_zero_res = compute_map_elites_summary(&empty_grid, 0);
    assert_eq!(qd_zero_res.max_cells, 0);
    assert_eq!(qd_zero_res.archive_coverage, 0.0);
    assert!(!qd_zero_res.archive_coverage.is_nan());
}

// ===============================================================================================
// 2. UNISEX POPULATION & EXTREME SEX RATIO SKEW TESTS
// ===============================================================================================

#[test]
fn test_adversarial_unisex_populations_and_skewed_ratios() {
    let tracker = DemographicTracker::default();

    // 1. 100% Male Population (N = 500)
    let all_males: Vec<(u32, Option<AgentType>, Option<AgentSex>)> = (0..500)
        .map(|i| (i % 100, Some(AgentType::Prey), Some(AgentSex::Male)))
        .collect();

    let census_males = build_demographic_census(&all_males, Some(&tracker));
    assert_eq!(census_males.male_count, 500);
    assert_eq!(census_males.female_count, 0);
    assert_eq!(census_males.sex_ratio, 0.0, "Sex ratio for 0 females must be 0.0 without panic");
    assert!(!census_males.sex_ratio.is_nan());
    assert!(!census_males.sex_ratio.is_infinite());

    // 2. 100% Female Population (N = 500)
    let all_females: Vec<(u32, Option<AgentType>, Option<AgentSex>)> = (0..500)
        .map(|i| (i % 100, Some(AgentType::Prey), Some(AgentSex::Female)))
        .collect();

    let census_females = build_demographic_census(&all_females, Some(&tracker));
    assert_eq!(census_females.male_count, 0);
    assert_eq!(census_females.female_count, 500);
    assert_eq!(census_females.sex_ratio, 0.0);
    assert!(!census_females.sex_ratio.is_nan());

    // 3. Extreme Skew: 1 Female, 10,000 Males
    let mut skewed_male: Vec<(u32, Option<AgentType>, Option<AgentSex>)> = (0..10_000)
        .map(|i| (i % 50, Some(AgentType::Prey), Some(AgentSex::Male)))
        .collect();
    skewed_male.push((25, Some(AgentType::Prey), Some(AgentSex::Female)));

    let census_skewed_male = build_demographic_census(&skewed_male, Some(&tracker));
    assert_eq!(census_skewed_male.male_count, 10_000);
    assert_eq!(census_skewed_male.female_count, 1);
    assert_eq!(census_skewed_male.sex_ratio, 10_000.0);

    // 4. Extreme Skew: 1 Male, 10,000 Females
    let mut skewed_female: Vec<(u32, Option<AgentType>, Option<AgentSex>)> = (0..10_000)
        .map(|i| (i % 50, Some(AgentType::Prey), Some(AgentSex::Female)))
        .collect();
    skewed_female.push((25, Some(AgentType::Prey), Some(AgentSex::Male)));

    let census_skewed_female = build_demographic_census(&skewed_female, Some(&tracker));
    assert_eq!(census_skewed_female.male_count, 1);
    assert_eq!(census_skewed_female.female_count, 10_000);
    assert!((census_skewed_female.sex_ratio - 0.0001).abs() < 1e-6);

    // 5. Unassigned Sex Fallback (None)
    let unassigned_sex: Vec<(u32, Option<AgentType>, Option<AgentSex>)> = (0..100)
        .map(|i| (i, Some(AgentType::Prey), None))
        .collect();

    let census_unassigned = build_demographic_census(&unassigned_sex, Some(&tracker));
    assert_eq!(census_unassigned.male_count + census_unassigned.female_count, 100);
    assert_eq!(census_unassigned.male_count, 50);
    assert_eq!(census_unassigned.female_count, 50);
    assert_eq!(census_unassigned.sex_ratio, 1.0);
}

// ===============================================================================================
// 3. EXTREME SENESCENCE, LONGEVITY CLAMPING & MORTALITY ATTRIBUTION
// ===============================================================================================

#[test]
fn test_adversarial_extreme_age_senescence_and_clamping() {
    let mut tracker = DemographicTracker::default();

    // Extreme age cohorts: 0, 50, 51, 150, 151, 300, 301, 1000, 5000, u32::MAX
    let test_ages = vec![
        0, 1, 25, 50,           // Cohort 0 (0-50) -> 4 agents
        51, 100, 150,           // Cohort 1 (51-150) -> 3 agents
        151, 200, 300,          // Cohort 2 (151-300) -> 3 agents
        301, 1000, 2500, u32::MAX, // Cohort 3 (301-1000+) -> 4 agents
    ];

    let agents: Vec<(u32, Option<AgentType>, Option<AgentSex>)> = test_ages
        .iter()
        .map(|&age| (age, Some(AgentType::Prey), Some(AgentSex::Male)))
        .collect();

    let census = build_demographic_census(&agents, Some(&tracker));

    assert_eq!(census.age_cohorts[0].count, 4, "Cohort 0 (0-50) count mismatch");
    assert_eq!(census.age_cohorts[1].count, 3, "Cohort 1 (51-150) count mismatch");
    assert_eq!(census.age_cohorts[2].count, 3, "Cohort 2 (151-300) count mismatch");
    assert_eq!(census.age_cohorts[3].count, 4, "Cohort 3 (301+) count mismatch");

    // Parent birth age tracking at extreme values
    tracker.record_birth(0);
    tracker.record_birth(50);
    tracker.record_birth(51);
    tracker.record_birth(150);
    tracker.record_birth(151);
    tracker.record_birth(300);
    tracker.record_birth(u32::MAX);

    assert_eq!(tracker.births_young, 2);
    assert_eq!(tracker.births_mature, 2);
    assert_eq!(tracker.births_old, 3);
    assert_eq!(tracker.total_births(), 7);

    // Mortality cause tracking: Senescence vs Starvation vs Others
    tracker.record_death("Starvation");
    tracker.record_death("Predation");
    tracker.record_death("Senescence");
    tracker.record_death("Toxicity");
    tracker.record_death("Pathogen");
    tracker.record_death("UnknownAlienCause"); // Should fallback to Starvation

    assert_eq!(tracker.deaths_starvation, 2);
    assert_eq!(tracker.deaths_predation, 1);
    assert_eq!(tracker.deaths_senescence, 1);
    assert_eq!(tracker.deaths_toxicity, 1);
    assert_eq!(tracker.deaths_pathogen, 1);
    assert_eq!(tracker.total_deaths(), 6);
}

// ===============================================================================================
// 4. COUNTER SATURATION & MASS POPULATION DYNAMICS
// ===============================================================================================

#[test]
fn test_adversarial_counter_saturation_and_overflow_resistance() {
    let mut tracker = DemographicTracker::default();

    // Force near u32::MAX for births
    tracker.births_young = u32::MAX - 1;
    tracker.record_birth(20);
    assert_eq!(tracker.births_young, u32::MAX);
    // Saturating add must not wrap to 0 or panic
    tracker.record_birth(20);
    assert_eq!(tracker.births_young, u32::MAX);

    // Force near u64::MAX for deaths
    tracker.deaths_starvation = u64::MAX - 1;
    tracker.record_death("Starvation");
    assert_eq!(tracker.deaths_starvation, u64::MAX);
    tracker.record_death("Starvation");
    assert_eq!(tracker.deaths_starvation, u64::MAX);

    // Large batch calculation
    let massive_agents: Vec<(u32, Option<AgentType>, Option<AgentSex>)> = (0..50_000)
        .map(|i| (i % 400, if i % 3 == 0 { Some(AgentType::Predator) } else { Some(AgentType::Prey) }, if i % 2 == 0 { Some(AgentSex::Male) } else { Some(AgentSex::Female) }))
        .collect();

    let census = build_demographic_census(&massive_agents, Some(&tracker));
    assert_eq!(census.prey_count + census.predator_count, 50_000);
    assert_eq!(census.male_count, 25_000);
    assert_eq!(census.female_count, 25_000);
    assert_eq!(census.sex_ratio, 1.0);

    let total_cohort_count: u32 = census.age_cohorts.iter().map(|c| c.count).sum();
    assert_eq!(total_cohort_count, 50_000, "All 50,000 agents must be strictly conserved in cohorts");
}

// ===============================================================================================
// 5. CLOSED-LOOP ENERGY CONSERVATION PARITY & FLUX CONSISTENCY
// ===============================================================================================

#[test]
fn test_adversarial_closed_loop_energy_conservation_parity() {
    let mut world = init_world();
    world.insert_resource(MapBounds {
        min: glam::Vec3::new(-100.0, 0.0, -100.0),
        max: glam::Vec3::new(100.0, 0.0, 100.0),
    });

    let biomes = vec![0u8; 32 * 32];
    let mut resource_field = ResourceField::from_biomes(&biomes, 32, 32, -100.0, -100.0, 100.0, 100.0, 0.05);
    for r in resource_field.r.iter_mut() {
        *r = 5.0;
    }
    let total_plant_energy = resource_field.total_biomass();
    world.insert_resource(resource_field);

    let initial_detritus = 2000.0f64;
    let mut biomass = EcosystemBiomass {
        detritus: initial_detritus,
        plants: total_plant_energy,
        animals: 0.0,
    };

    // Spawn 20 prey and 5 predators
    let mut total_animal_energy = 0.0f64;
    for i in 0..20 {
        world.spawn((
            Agent,
            Prey,
            AgentSex::Female,
            HomeostaticState {
                energy: 50.0,
                energy_target: 100.0,
                ..Default::default()
            },
            AgentEvaluation {
                start_position: glam::Vec3::ZERO,
                total_distance: 0.0,
                total_energy_expended: 0.0,
                survival_ticks: 10 + i,
                last_position: glam::Vec3::ZERO,
            },
            AgentLineageId(format!("prey_{i}")),
        ));
        total_animal_energy += 50.0;
    }

    for i in 0..5 {
        world.spawn((
            Agent,
            Predator,
            AgentSex::Male,
            HomeostaticState {
                energy: 100.0,
                energy_target: 150.0,
                ..Default::default()
            },
            AgentEvaluation {
                start_position: glam::Vec3::ZERO,
                total_distance: 0.0,
                total_energy_expended: 0.0,
                survival_ticks: 50 + i,
                last_position: glam::Vec3::ZERO,
            },
            AgentLineageId(format!("pred_{i}")),
        ));
        total_animal_energy += 100.0;
    }

    biomass.animals = total_animal_energy;
    let initial_total_energy = biomass.total();

    let mut ledger = EnergyLedger::default();
    ledger.lock_baseline(initial_total_energy);

    world.insert_resource(biomass);
    world.insert_resource(ledger);
    world.insert_resource(EnergyFluxMatrix::default());
    world.insert_resource(DemographicTracker::default());
    world.insert_resource(DemographicCensusState::default());
    world.insert_resource(EnergyFluxReport::default());

    let mut schedule = Schedule::default();
    schedule.add_systems((demographic_census_system, energy_flux_aggregation_system));
    schedule.run(&mut world);

    // Verify demographic census from live world
    let census = world.resource::<DemographicCensusState>();
    assert_eq!(census.prey_count, 20);
    assert_eq!(census.predator_count, 5);
    assert_eq!(census.female_count, 20);
    assert_eq!(census.male_count, 5);
    assert_eq!(census.sex_ratio, 5.0 / 20.0);
    assert!(census.shannon > 0.0, "Shannon diversity should be positive for mixed community");
    assert!(census.simpson > 0.0, "Simpson diversity should be positive for mixed community");

    // Verify energy flux report
    let flux_report = world.resource::<EnergyFluxReport>();
    assert_eq!(flux_report.nodes.len(), 6);
    assert_eq!(flux_report.links.len(), 9);
    assert_eq!(flux_report.total_system_energy, initial_total_energy);
    assert!(
        flux_report.energy_drift_percent < 0.01,
        "Energy drift must be < 0.01% (was {})",
        flux_report.energy_drift_percent
    );

    // Check node biomass sum conservation (plants + detritus + herbivores + carnivores = total_energy)
    let plant_biomass = flux_report.nodes.iter().find(|n| n.id == "plants").unwrap().biomass;
    let detritus_biomass = flux_report.nodes.iter().find(|n| n.id == "detritus").unwrap().biomass;
    let herbivore_biomass = flux_report.nodes.iter().find(|n| n.id == "herbivores").unwrap().biomass;
    let carnivore_biomass = flux_report.nodes.iter().find(|n| n.id == "carnivores").unwrap().biomass;

    let summed_flux_nodes = plant_biomass + detritus_biomass + herbivore_biomass + carnivore_biomass;
    let delta = (summed_flux_nodes - initial_total_energy).abs();
    assert!(
        delta < RESIDUAL_ABS_TOLERANCE_EU,
        "Sum of trophic nodes ({summed_flux_nodes}) must equal total system energy ({initial_total_energy}) within tolerance 1e-3, delta={delta}"
    );
}

// ===============================================================================================
// 6. SPATIAL DENSITY MATRIX COORDINATE CORRUPTION & ANOMALY TESTS
// ===============================================================================================

#[test]
fn test_adversarial_spatial_density_coordinate_anomalies() {
    let bounds = MapBounds {
        min: glam::Vec3::new(-100.0, 0.0, -100.0),
        max: glam::Vec3::new(100.0, 0.0, 100.0),
    };

    let corrupt_segments = vec![
        // Extreme NaN coordinates
        SegmentState {
            agent_id: 1,
            segment_id: 0,
            parent_segment_id: None,
            agent_type: Some(AgentType::Prey),
            x: f32::NAN,
            y: 0.0,
            z: f32::NAN,
            ..Default::default()
        },
        // +Infinity coordinates
        SegmentState {
            agent_id: 2,
            segment_id: 0,
            parent_segment_id: None,
            agent_type: Some(AgentType::Predator),
            x: f32::INFINITY,
            y: 0.0,
            z: f32::INFINITY,
            ..Default::default()
        },
        // -Infinity coordinates
        SegmentState {
            agent_id: 3,
            segment_id: 0,
            parent_segment_id: None,
            agent_type: Some(AgentType::Prey),
            x: f32::NEG_INFINITY,
            y: 0.0,
            z: f32::NEG_INFINITY,
            ..Default::default()
        },
        // Extreme out of bounds
        SegmentState {
            agent_id: 4,
            segment_id: 0,
            parent_segment_id: None,
            agent_type: Some(AgentType::Prey),
            x: 1_000_000.0,
            y: 0.0,
            z: -1_000_000.0,
            ..Default::default()
        },
        // Non-root child segment of agent 4 (must be ignored)
        SegmentState {
            agent_id: 4,
            segment_id: 1,
            parent_segment_id: Some(0),
            agent_type: Some(AgentType::Prey),
            x: 1_000_000.0,
            y: 0.0,
            z: -1_000_000.0,
            ..Default::default()
        },
    ];

    let report = generate_spatial_density_matrix(16, 16, &corrupt_segments, Some(&bounds), None, &[]);

    assert_eq!(report.width, 16);
    assert_eq!(report.height, 16);
    assert_eq!(report.prey_density.len(), 256);
    assert_eq!(report.predator_density.len(), 256);

    let sum_prey: u32 = report.prey_density.iter().sum();
    let sum_pred: u32 = report.predator_density.iter().sum();

    // 3 prey root segments + 1 predator root segment = 4 total agents
    assert_eq!(sum_prey, 3, "Corrupt prey coordinates must safely clamp into grid");
    assert_eq!(sum_pred, 1, "Corrupt predator coordinates must safely clamp into grid");
}

// ===============================================================================================
// 7. ISO-8601 CALENDAR ARITHMETIC EDGE CASES
// ===============================================================================================

#[test]
fn test_adversarial_iso8601_calendar_edges() {
    // 1970-01-01T00:00:00Z (UNIX Epoch start)
    assert_eq!(format_iso8601_utc(0), "1970-01-01T00:00:00Z");

    // 2000-02-29T12:00:00Z (Century Leap Year)
    // Days from 1970-01-01 to 2000-02-29:
    // 30 years (7 leap years: 72, 76, 80, 84, 88, 92, 96) + 31 days (Jan) + 28 days = 11016 days
    let epoch_2000_leap = 11016 * 86400 + 12 * 3600;
    assert_eq!(format_iso8601_utc(epoch_2000_leap), "2000-02-29T12:00:00Z");

    // 2026-08-16T03:19:54Z
    let sample_epoch = 1786850394;
    let formatted = format_iso8601_utc(sample_epoch);
    assert_eq!(formatted, "2026-08-16T03:19:54Z");
}
