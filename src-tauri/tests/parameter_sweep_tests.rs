//! Integration tests for Multidimensional Parameter Sweeps (Feature 5).

use std::collections::HashSet;
use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, ObserverPolicy, WorldIdentity,
    WorldLawSet, MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::reference_world::ReferenceEvolutionWorld;
use anima_engine_lib::core::sweeps::{
    pearson_correlation, run_sweep, DimensionSpec, ParameterDim, SamplingStrategy,
    SobolSequence, SweepManifest, SweepTarget, SWEEP_SCHEMA_VERSION,
};

fn base_manifest() -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "sweep_base_exp".into(),
        name: "Sweep Base Experiment".into(),
        observer: ObserverPolicy::default(),
        world_identity: WorldIdentity {
            seed: 1337,
            generator_version: 20,
            width: 128,
            height: 128,
            checksum: 0xDEAD_BEEF,
        },
        laws: WorldLawSet::baseline(),
        initial_conditions: InitialConditionSet::new(vec![
            ("precip".into(), 1.0),
            ("temperature".into(), 0.5),
            ("plants".into(), 100.0),
            ("herbivores".into(), 40.0),
            ("predators".into(), 8.0),
            ("detritus".into(), 0.0),
        ]),
        interventions: Vec::new(),
        seeds: vec![42, 43],
        duration_ticks: 100,
        sample_period: 20,
        observable_ids: vec!["plants".into(), "herbivores".into(), "predators".into()],
        exotic_interventions: Vec::new(),
    }
}

#[test]
fn sobol_sequence_generates_bounded_uniform_points() {
    let mut sobol = SobolSequence::new(4);
    let n = 256;
    let mut points = Vec::with_capacity(n);

    for _ in 0..n {
        let pt = sobol.next_point();
        assert_eq!(pt.len(), 4);
        for &u in &pt {
            assert!(u >= 0.0 && u < 1.0, "Sobol coordinate {u} out of [0, 1)");
        }
        points.push(pt);
    }

    // Verify all points are distinct
    let mut set = HashSet::new();
    for p in &points {
        let key = format!("{:.6}_{:.6}_{:.6}_{:.6}", p[0], p[1], p[2], p[3]);
        assert!(set.insert(key), "Sobol produced duplicate point");
    }
}

#[test]
fn dimension_spec_mapping_and_grid_generation() {
    let cont = DimensionSpec::Continuous { min: 10.0, max: 20.0 };
    assert_eq!(cont.map_normalized(0.0), 10.0);
    assert_eq!(cont.map_normalized(0.5), 15.0);
    assert_eq!(cont.grid_values(5), vec![10.0, 12.5, 15.0, 17.5, 20.0]);

    let log = DimensionSpec::Logarithmic { min: 1.0, max: 100.0 };
    assert_eq!(log.map_normalized(0.0), 1.0);
    assert!((log.map_normalized(0.5) - 10.0).abs() < 1e-10);
    assert_eq!(log.grid_values(3), vec![1.0, 10.0, 100.0]);

    let int_range = DimensionSpec::IntegerRange { min: 5, max: 9, step: 2 };
    assert_eq!(int_range.grid_values(0), vec![5.0, 7.0, 9.0]);

    let disc = DimensionSpec::Discrete { values: vec![1.1, 2.2, 3.3] };
    assert_eq!(disc.map_normalized(0.0), 1.1);
    assert_eq!(disc.map_normalized(0.4), 2.2);
    assert_eq!(disc.map_normalized(0.9), 3.3);
}

#[test]
fn pearson_correlation_accuracy() {
    let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let y_pos = vec![2.0, 4.0, 6.0, 8.0, 10.0];
    let r_pos = pearson_correlation(&x, &y_pos).expect("positive correlation");
    assert!((r_pos - 1.0).abs() < 1e-10);

    let y_neg = vec![10.0, 8.0, 6.0, 4.0, 2.0];
    let r_neg = pearson_correlation(&x, &y_neg).expect("negative correlation");
    assert!((r_neg - (-1.0)).abs() < 1e-10);

    let y_flat = vec![5.0, 5.0, 5.0, 5.0, 5.0];
    assert!(pearson_correlation(&x, &y_flat).is_none());
}

#[test]
fn sweep_grid_execution_and_csv_generation() {
    let registry = ObservableRegistry::reference_default();
    let base = base_manifest();

    let sweep = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "grid_test_sweep".into(),
        name: "Grid Test".into(),
        base_manifest: base,
        parameters: vec![
            ParameterDim {
                name: "plant_biomass".into(),
                target: SweepTarget::InitialCondition("plants".into()),
                spec: DimensionSpec::Continuous { min: 50.0, max: 150.0 },
            },
            ParameterDim {
                name: "herbivore_biomass".into(),
                target: SweepTarget::InitialCondition("herbivores".into()),
                spec: DimensionSpec::Discrete { values: vec![20.0, 40.0] },
            },
        ],
        sampling: SamplingStrategy::Grid { points_per_dim: Some(3) },
        max_threads: Some(2),
    };

    let result = run_sweep::<ReferenceEvolutionWorld>(&sweep, &registry)
        .expect("sweep must run successfully");

    assert_eq!(result.total_points, 6); // 3 * 2
    assert_eq!(result.completed_points, 6);
    assert_eq!(result.total_runs, 12); // 6 points * 2 seeds
    assert_eq!(result.completed_runs, 12);
    assert_eq!(result.failed_runs, 0);

    let csv = result.to_csv();
    assert!(csv.contains("point_index,plant_biomass,herbivore_biomass,completed_runs,failed_runs"));
    assert_eq!(csv.lines().count(), 7); // Header + 6 rows
}

#[test]
fn sweep_sobol_sensitivity_analysis() {
    let registry = ObservableRegistry::reference_default();
    let base = base_manifest();

    let sweep = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "sobol_sensitivity_sweep".into(),
        name: "Sobol Sensitivity Test".into(),
        base_manifest: base,
        parameters: vec![
            ParameterDim {
                name: "plants".into(),
                target: SweepTarget::InitialCondition("plants".into()),
                spec: DimensionSpec::Continuous { min: 80.0, max: 120.0 },
            },
            ParameterDim {
                name: "herbivores".into(),
                target: SweepTarget::InitialCondition("herbivores".into()),
                spec: DimensionSpec::Continuous { min: 30.0, max: 50.0 },
            },
        ],
        sampling: SamplingStrategy::Sobol { num_samples: 8, offset: 0 },
        max_threads: Some(4),
    };

    let result = run_sweep::<ReferenceEvolutionWorld>(&sweep, &registry)
        .expect("sobol sweep must succeed");

    assert_eq!(result.total_points, 8);
    assert_eq!(result.completed_points, 8);
    assert!(result.global_metrics.contains_key("plants"));

    let plant_stats = &result.global_metrics["plants"];
    assert_eq!(plant_stats.parameter_correlations.len(), 2);
    assert!(plant_stats.mean > 0.0);
    assert!(plant_stats.min <= plant_stats.max);
}

#[test]
fn sweep_latin_hypercube_reproducibility() {
    let registry = ObservableRegistry::reference_default();
    let base = base_manifest();

    let make_sweep = |seed: u64| SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "lhs_repro_sweep".into(),
        name: "LHS Repro Test".into(),
        base_manifest: base.clone(),
        parameters: vec![
            ParameterDim {
                name: "predators".into(),
                target: SweepTarget::InitialCondition("predators".into()),
                spec: DimensionSpec::Continuous { min: 2.0, max: 12.0 },
            },
        ],
        sampling: SamplingStrategy::LatinHypercube {
            num_samples: 5,
            seed: Some(seed),
            centered: false,
        },
        max_threads: Some(1),
    };

    let res1 = run_sweep::<ReferenceEvolutionWorld>(&make_sweep(999), &registry).unwrap();
    let res2 = run_sweep::<ReferenceEvolutionWorld>(&make_sweep(999), &registry).unwrap();

    assert_eq!(res1.parameter_matrix, res2.parameter_matrix);
    assert_eq!(
        res1.points[0].observables["plants"].mean,
        res2.points[0].observables["plants"].mean
    );
}
