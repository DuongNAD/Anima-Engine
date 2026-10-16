//! Adversarial Stress Tests for Feature 5 (Multidimensional Parameter Sweeps) & `anima-lab sweep` CLI.

use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anima_engine_lib::core::experiment::{
    ExperimentError, ExperimentManifest, InitialConditionSet, ObservableRegistry, ObserverPolicy,
    WorldIdentity, WorldLawSet, MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::reference_world::ReferenceEvolutionWorld;
use anima_engine_lib::core::sweeps::{
    pearson_correlation, run_sweep, DimensionSpec, ParameterDim, SamplingStrategy,
    SobolSequence, SweepManifest, SweepResult, SweepTarget, MAX_SWEEP_DIMENSIONS,
    SWEEP_SCHEMA_VERSION,
};

fn base_reference_manifest() -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "adv_sweep_base".into(),
        name: "Adversarial Sweep Base".into(),
        observer: ObserverPolicy::default(),
        world_identity: WorldIdentity {
            seed: 4242,
            generator_version: 20,
            width: 128,
            height: 128,
            checksum: 0xCAFE_BABE,
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
        seeds: vec![101, 102],
        duration_ticks: 60,
        sample_period: 20,
        observable_ids: vec!["plants".into(), "herbivores".into(), "predators".into()],
        exotic_interventions: Vec::new(),
    }
}

// =============================================================================
// 1. SOBOL LOW-DISCREPANCY SEQUENCE ADVERSARIAL TESTS
// =============================================================================

#[test]
fn sobol_sequence_covers_all_32_dimensions_with_unit_bounds() {
    let mut sobol = SobolSequence::new(MAX_SWEEP_DIMENSIONS);
    let n_samples = 512;

    let mut all_points = Vec::with_capacity(n_samples);

    for idx in 0..n_samples {
        let pt = sobol.next_point();
        assert_eq!(
            pt.len(),
            MAX_SWEEP_DIMENSIONS,
            "Point {idx} must have 32 dimensions"
        );

        for (d, &coord) in pt.iter().enumerate() {
            assert!(
                coord.is_finite(),
                "Sobol dim {d} produced non-finite value at sample {idx}: {coord}"
            );
            assert!(
                coord >= 0.0 && coord < 1.0,
                "Sobol dim {d} coordinate out of [0, 1) at sample {idx}: {coord}"
            );
        }
        all_points.push(pt);
    }

    // 1D Uniformity / Stratification test across 16 bins for every single dimension
    let num_bins = 16;
    for d in 0..MAX_SWEEP_DIMENSIONS {
        let mut bin_counts = vec![0usize; num_bins];
        for pt in &all_points {
            let bin = ((pt[d] * num_bins as f64).floor() as usize).min(num_bins - 1);
            bin_counts[bin] += 1;
        }

        // Each bin expected to have ~32 samples. Verify no empty bins and reasonable distribution.
        for (bin_idx, &count) in bin_counts.iter().enumerate() {
            assert!(
                count > 0,
                "Dimension {d} has empty bin {bin_idx} after {n_samples} Sobol samples!"
            );
        }
    }
}

#[test]
fn sobol_sequence_pairwise_low_correlation() {
    let mut sobol = SobolSequence::new(8);
    let n_samples = 1024;
    let mut dims = vec![Vec::with_capacity(n_samples); 8];

    for _ in 0..n_samples {
        let pt = sobol.next_point();
        for d in 0..8 {
            dims[d].push(pt[d]);
        }
    }

    // Check pairwise Pearson correlations between different dimensions
    for i in 0..8 {
        for j in (i + 1)..8 {
            let r = pearson_correlation(&dims[i], &dims[j]).expect("correlation must exist");
            assert!(
                r.abs() < 0.15,
                "Sobol dimensions {i} and {j} have unexpectedly high correlation: {r}"
            );
        }
    }
}

// =============================================================================
// 2. LATIN HYPERCUBE SAMPLING (LHS) LATIN PROPERTY RIGOROUS VERIFICATION
// =============================================================================

#[test]
fn lhs_strictly_satisfies_latin_property_across_dimensions_and_sample_sizes() {
    let _registry = ObservableRegistry::reference_default();
    let base = base_reference_manifest();

    let sample_sizes = [1, 2, 7, 25, 100];
    let dimensions = [1, 3, 10, 20];

    for &n_samples in &sample_sizes {
        for &num_dims in &dimensions {
            for centered in [false, true] {
                let params: Vec<ParameterDim> = (0..num_dims)
                    .map(|d| ParameterDim {
                        name: format!("param_{d}"),
                        target: SweepTarget::InitialCondition(format!("init_{d}")),
                        spec: DimensionSpec::Continuous {
                            min: 0.0,
                            max: 100.0,
                        },
                    })
                    .collect();

                let manifest = SweepManifest {
                    schema_version: SWEEP_SCHEMA_VERSION,
                    sweep_id: format!("lhs_stress_{n_samples}_{num_dims}"),
                    name: "LHS Stress Test".into(),
                    base_manifest: base.clone(),
                    parameters: params,
                    sampling: SamplingStrategy::LatinHypercube {
                        num_samples: n_samples,
                        seed: Some(0xDEAD_1234 ^ (n_samples as u64) ^ (num_dims as u64)),
                        centered,
                    },
                    max_threads: Some(1),
                };

                let (_, matrix) = manifest
                    .generate_sample_matrix()
                    .expect("matrix generation must succeed");
                assert_eq!(matrix.len(), n_samples);

                // Check Latin property: for EACH dimension, each of the N bins [0, 100/N), [100/N, 200/N), ...
                // must contain EXACTLY 1 sample!
                for d in 0..num_dims {
                    let mut bin_counts = vec![0usize; n_samples];
                    for row in &matrix {
                        let val = row[d];
                        assert!(val >= 0.0 && val <= 100.0, "LHS value {val} out of bounds");
                        let bin = ((val / 100.0 * n_samples as f64).floor() as usize)
                            .min(n_samples - 1);
                        bin_counts[bin] += 1;
                    }

                    for (b, &cnt) in bin_counts.iter().enumerate() {
                        assert_eq!(
                            cnt, 1,
                            "LHS Latin property violated! Dim {d}, bin {b}/{n_samples} has count {cnt} (expected exactly 1). centered={centered}"
                        );
                    }
                }
            }
        }
    }
}

// =============================================================================
// 3. EXTREME PARAMETER SPACES (1D, 16D, 32D, LOG, DISCRETE, INTEGER)
// =============================================================================

#[test]
fn single_dimension_sweeps_all_dimension_spec_types() {
    let registry = ObservableRegistry::reference_default();
    let base = base_reference_manifest();

    // 1D Continuous
    let sweep_cont = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "1d_cont".into(),
        name: "1D Continuous".into(),
        base_manifest: base.clone(),
        parameters: vec![ParameterDim {
            name: "plants".into(),
            target: SweepTarget::InitialCondition("plants".into()),
            spec: DimensionSpec::Continuous {
                min: 50.0,
                max: 150.0,
            },
        }],
        sampling: SamplingStrategy::Grid {
            points_per_dim: Some(5),
        },
        max_threads: Some(1),
    };
    let res_cont = run_sweep::<ReferenceEvolutionWorld>(&sweep_cont, &registry).unwrap();
    assert_eq!(res_cont.total_points, 5);
    assert_eq!(res_cont.completed_points, 5);

    // 1D Logarithmic with wide dynamic range
    let sweep_log = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "1d_log".into(),
        name: "1D Logarithmic".into(),
        base_manifest: base.clone(),
        parameters: vec![ParameterDim {
            name: "plants".into(),
            target: SweepTarget::InitialCondition("plants".into()),
            spec: DimensionSpec::Logarithmic {
                min: 1e-3,
                max: 1e3,
            },
        }],
        sampling: SamplingStrategy::Sobol {
            num_samples: 8,
            offset: 0,
        },
        max_threads: Some(1),
    };
    let res_log = run_sweep::<ReferenceEvolutionWorld>(&sweep_log, &registry).unwrap();
    assert_eq!(res_log.total_points, 8);
    for pt in &res_log.points {
        assert!(pt.parameters[0] >= 1e-3 && pt.parameters[0] <= 1e3);
    }

    // 1D Discrete with negative and fractional values
    let sweep_disc = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "1d_disc".into(),
        name: "1D Discrete".into(),
        base_manifest: base.clone(),
        parameters: vec![ParameterDim {
            name: "plants".into(),
            target: SweepTarget::InitialCondition("plants".into()),
            spec: DimensionSpec::Discrete {
                values: vec![-10.5, 0.0, 42.125, 100.0],
            },
        }],
        sampling: SamplingStrategy::Grid {
            points_per_dim: None,
        },
        max_threads: Some(1),
    };
    let res_disc = run_sweep::<ReferenceEvolutionWorld>(&sweep_disc, &registry).unwrap();
    assert_eq!(res_disc.total_points, 4);

    // 1D IntegerRange
    let sweep_int = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "1d_int".into(),
        name: "1D IntegerRange".into(),
        base_manifest: base.clone(),
        parameters: vec![ParameterDim {
            name: "plants".into(),
            target: SweepTarget::InitialCondition("plants".into()),
            spec: DimensionSpec::IntegerRange {
                min: -10,
                max: 10,
                step: 5,
            },
        }],
        sampling: SamplingStrategy::Grid {
            points_per_dim: None,
        },
        max_threads: Some(1),
    };
    let res_int = run_sweep::<ReferenceEvolutionWorld>(&sweep_int, &registry).unwrap();
    assert_eq!(res_int.total_points, 5); // -10, -5, 0, 5, 10
}

#[test]
fn extreme_high_dimensional_32d_sweep_execution() {
    let registry = ObservableRegistry::reference_default();
    let base = base_reference_manifest();

    // Construct 32 distinct dimensions with mixed types
    let mut params = Vec::with_capacity(32);
    for d in 0..32 {
        let spec = match d % 4 {
            0 => DimensionSpec::Continuous {
                min: 10.0 + (d as f64),
                max: 100.0 + (d as f64),
            },
            1 => DimensionSpec::Logarithmic {
                min: 0.1 * ((d + 1) as f64),
                max: 10.0 * ((d + 1) as f64),
            },
            2 => DimensionSpec::IntegerRange {
                min: 1,
                max: 50,
                step: 10,
            },
            _ => DimensionSpec::Discrete {
                values: vec![1.0, 5.0, 10.0, 20.0],
            },
        };
        params.push(ParameterDim {
            name: format!("dim_{d:02}"),
            target: SweepTarget::InitialCondition(format!("init_dim_{d}")),
            spec,
        });
    }

    let sweep_32d = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "32d_extreme_sobol".into(),
        name: "32D Extreme Sobol".into(),
        base_manifest: base.clone(),
        parameters: params,
        sampling: SamplingStrategy::Sobol {
            num_samples: 16,
            offset: 0,
        },
        max_threads: Some(4),
    };

    let result = run_sweep::<ReferenceEvolutionWorld>(&sweep_32d, &registry)
        .expect("32-dimensional Sobol sweep must succeed");
    assert_eq!(result.total_points, 16);
    assert_eq!(result.parameter_names.len(), 32);
    for pt in &result.points {
        assert_eq!(pt.parameters.len(), 32);
    }
}

// =============================================================================
// 4. INVALID MANIFESTS & ERROR HANDLING (NO PANICS)
// =============================================================================

#[test]
fn invalid_manifests_gracefully_rejected_with_specific_errors() {
    let registry = ObservableRegistry::reference_default();
    let base = base_reference_manifest();

    // 1. Dimension count exceeds limit (> 32)
    let over_32_params: Vec<ParameterDim> = (0..33)
        .map(|d| ParameterDim {
            name: format!("p_{d}"),
            target: SweepTarget::InitialCondition(format!("init_{d}")),
            spec: DimensionSpec::Continuous {
                min: 0.0,
                max: 1.0,
            },
        })
        .collect();
    let sweep_over_32 = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "over_32".into(),
        name: "Over 32 Dims".into(),
        base_manifest: base.clone(),
        parameters: over_32_params,
        sampling: SamplingStrategy::Sobol {
            num_samples: 5,
            offset: 0,
        },
        max_threads: None,
    };
    match sweep_over_32.validate(&registry) {
        Err(ExperimentError::ResourceLimit { field, limit, found }) => {
            assert!(field.contains("sweep parameters"));
            assert_eq!(limit, 32);
            assert_eq!(found, 33);
        }
        other => panic!("Expected ResourceLimit error, got {other:?}"),
    }

    // 2. Empty parameters
    let sweep_empty = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "empty_p".into(),
        name: "Empty Params".into(),
        base_manifest: base.clone(),
        parameters: Vec::new(),
        sampling: SamplingStrategy::Sobol {
            num_samples: 5,
            offset: 0,
        },
        max_threads: None,
    };
    assert!(matches!(
        sweep_empty.validate(&registry),
        Err(ExperimentError::EmptyField { .. })
    ));

    // 3. Duplicate parameter names
    let sweep_dup = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "dup_p".into(),
        name: "Duplicate Params".into(),
        base_manifest: base.clone(),
        parameters: vec![
            ParameterDim {
                name: "dup_name".into(),
                target: SweepTarget::InitialCondition("plants".into()),
                spec: DimensionSpec::Continuous { min: 0.0, max: 1.0 },
            },
            ParameterDim {
                name: "dup_name".into(),
                target: SweepTarget::InitialCondition("herbivores".into()),
                spec: DimensionSpec::Continuous { min: 0.0, max: 1.0 },
            },
        ],
        sampling: SamplingStrategy::Sobol {
            num_samples: 5,
            offset: 0,
        },
        max_threads: None,
    };
    assert!(matches!(
        sweep_dup.validate(&registry),
        Err(ExperimentError::DuplicateId { .. })
    ));

    // 4. Inverted Continuous bounds (min > max)
    let p_inv_cont = ParameterDim {
        name: "bad_cont".into(),
        target: SweepTarget::InitialCondition("plants".into()),
        spec: DimensionSpec::Continuous { min: 10.0, max: 5.0 },
    };
    assert!(matches!(
        p_inv_cont.validate(),
        Err(ExperimentError::OutOfRange { .. })
    ));

    // 5. Inverted Logarithmic bounds (min > max)
    let p_inv_log = ParameterDim {
        name: "bad_log".into(),
        target: SweepTarget::InitialCondition("plants".into()),
        spec: DimensionSpec::Logarithmic { min: 10.0, max: 1.0 },
    };
    assert!(matches!(
        p_inv_log.validate(),
        Err(ExperimentError::OutOfRange { .. })
    ));

    // 6. Logarithmic non-positive min (min <= 0.0)
    let p_zero_log = ParameterDim {
        name: "bad_log_zero".into(),
        target: SweepTarget::InitialCondition("plants".into()),
        spec: DimensionSpec::Logarithmic { min: 0.0, max: 10.0 },
    };
    assert!(matches!(
        p_zero_log.validate(),
        Err(ExperimentError::OutOfRange { .. })
    ));

    let p_neg_log = ParameterDim {
        name: "bad_log_neg".into(),
        target: SweepTarget::InitialCondition("plants".into()),
        spec: DimensionSpec::Logarithmic { min: -5.0, max: 10.0 },
    };
    assert!(matches!(
        p_neg_log.validate(),
        Err(ExperimentError::OutOfRange { .. })
    ));

    // 7. IntegerRange invalid step (step <= 0)
    let p_bad_step = ParameterDim {
        name: "bad_step".into(),
        target: SweepTarget::InitialCondition("plants".into()),
        spec: DimensionSpec::IntegerRange { min: 1, max: 10, step: 0 },
    };
    assert!(matches!(
        p_bad_step.validate(),
        Err(ExperimentError::OutOfRange { .. })
    ));

    // 8. Discrete empty values
    let p_empty_disc = ParameterDim {
        name: "bad_disc".into(),
        target: SweepTarget::InitialCondition("plants".into()),
        spec: DimensionSpec::Discrete { values: vec![] },
    };
    assert!(matches!(
        p_empty_disc.validate(),
        Err(ExperimentError::EmptyField { .. })
    ));

    // 9. Discrete non-finite values (NaN / Inf)
    let p_nan_disc = ParameterDim {
        name: "nan_disc".into(),
        target: SweepTarget::InitialCondition("plants".into()),
        spec: DimensionSpec::Discrete { values: vec![1.0, f64::NAN] },
    };
    assert!(matches!(
        p_nan_disc.validate(),
        Err(ExperimentError::NotFinite { .. })
    ));

    // 10. Sobol zero samples
    let sweep_sobol_zero = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "sobol_zero".into(),
        name: "Sobol Zero".into(),
        base_manifest: base.clone(),
        parameters: vec![ParameterDim {
            name: "p".into(),
            target: SweepTarget::InitialCondition("plants".into()),
            spec: DimensionSpec::Continuous { min: 0.0, max: 1.0 },
        }],
        sampling: SamplingStrategy::Sobol {
            num_samples: 0,
            offset: 0,
        },
        max_threads: None,
    };
    assert!(matches!(
        sweep_sobol_zero.generate_sample_matrix(),
        Err(ExperimentError::OutOfRange { .. })
    ));

    // 11. LHS zero samples
    let sweep_lhs_zero = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "lhs_zero".into(),
        name: "LHS Zero".into(),
        base_manifest: base.clone(),
        parameters: vec![ParameterDim {
            name: "p".into(),
            target: SweepTarget::InitialCondition("plants".into()),
            spec: DimensionSpec::Continuous { min: 0.0, max: 1.0 },
        }],
        sampling: SamplingStrategy::LatinHypercube {
            num_samples: 0,
            seed: None,
            centered: false,
        },
        max_threads: None,
    };
    assert!(matches!(
        sweep_lhs_zero.generate_sample_matrix(),
        Err(ExperimentError::OutOfRange { .. })
    ));

    // 12. Grid explosion (> 100,000 points)
    let sweep_grid_huge = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "grid_huge".into(),
        name: "Grid Huge".into(),
        base_manifest: base.clone(),
        parameters: vec![
            ParameterDim {
                name: "p1".into(),
                target: SweepTarget::InitialCondition("plants".into()),
                spec: DimensionSpec::Continuous { min: 0.0, max: 1.0 },
            },
            ParameterDim {
                name: "p2".into(),
                target: SweepTarget::InitialCondition("plants".into()),
                spec: DimensionSpec::Continuous { min: 0.0, max: 1.0 },
            },
            ParameterDim {
                name: "p3".into(),
                target: SweepTarget::InitialCondition("plants".into()),
                spec: DimensionSpec::Continuous { min: 0.0, max: 1.0 },
            },
        ],
        sampling: SamplingStrategy::Grid {
            points_per_dim: Some(100), // 100^3 = 1,000,000 > 100,000
        },
        max_threads: None,
    };
    assert!(matches!(
        sweep_grid_huge.generate_sample_matrix(),
        Err(ExperimentError::ResourceLimit { .. })
    ));
}

// =============================================================================
// 5. PEARSON CORRELATION ADVERSARIAL STRESS & NUMERICAL STABILITY
// =============================================================================

#[test]
fn pearson_correlation_adversarial_properties_and_bounds() {
    // 1. Length mismatch
    assert!(pearson_correlation(&[1.0, 2.0], &[1.0, 2.0, 3.0]).is_none());

    // 2. Sample count < 2
    assert!(pearson_correlation(&[1.0], &[2.0]).is_none());
    assert!(pearson_correlation(&[], &[]).is_none());

    // 3. Zero variance in X
    assert!(pearson_correlation(&[4.0, 4.0, 4.0, 4.0], &[1.0, 2.0, 3.0, 4.0]).is_none());

    // 4. Zero variance in Y
    assert!(pearson_correlation(&[1.0, 2.0, 3.0, 4.0], &[9.0, 9.0, 9.0, 9.0]).is_none());

    // 5. Perfect positive correlation (+1.0)
    let x = vec![10.0, 20.0, 30.0, 40.0, 50.0];
    let y = vec![105.0, 110.0, 115.0, 120.0, 125.0];
    let r = pearson_correlation(&x, &y).unwrap();
    assert!((r - 1.0).abs() < 1e-12, "Expected 1.0, got {r}");

    // 6. Perfect negative correlation (-1.0)
    let y_neg = vec![500.0, 400.0, 300.0, 200.0, 100.0];
    let r_neg = pearson_correlation(&x, &y_neg).unwrap();
    assert!((r_neg - (-1.0)).abs() < 1e-12, "Expected -1.0, got {r_neg}");

    // 7. Numerical precision stress: large offsets with tiny variations
    let x_offset = vec![1e14 + 1.0, 1e14 + 2.0, 1e14 + 3.0, 1e14 + 4.0];
    let y_offset = vec![1e14 + 2.0, 1e14 + 4.0, 1e14 + 6.0, 1e14 + 8.0];
    let r_offset = pearson_correlation(&x_offset, &y_offset).unwrap();
    assert!(r_offset >= -1.0 && r_offset <= 1.0);
    assert!((r_offset - 1.0).abs() < 1e-5);
}

// =============================================================================
// 6. CSV AND JSON SERIALIZATION INTEGRITY & PARITY
// =============================================================================

#[test]
fn sweep_result_csv_and_json_format_integrity() {
    let registry = ObservableRegistry::reference_default();
    let base = base_reference_manifest();

    let sweep = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "export_integrity_test".into(),
        name: "Export Integrity Test".into(),
        base_manifest: base,
        parameters: vec![
            ParameterDim {
                name: "plants".into(),
                target: SweepTarget::InitialCondition("plants".into()),
                spec: DimensionSpec::Continuous { min: 60.0, max: 120.0 },
            },
            ParameterDim {
                name: "herbivores".into(),
                target: SweepTarget::InitialCondition("herbivores".into()),
                spec: DimensionSpec::Discrete { values: vec![20.0, 40.0] },
            },
        ],
        sampling: SamplingStrategy::Grid { points_per_dim: Some(2) },
        max_threads: Some(2),
    };

    let result = run_sweep::<ReferenceEvolutionWorld>(&sweep, &registry).unwrap();
    assert_eq!(result.total_points, 4);

    // Verify JSON round-trip
    let json_str = result.to_json().expect("JSON serialization must succeed");
    let deserialized: SweepResult =
        serde_json::from_str(&json_str).expect("JSON must deserialize back to SweepResult");
    assert_eq!(deserialized.sweep_id, result.sweep_id);
    assert_eq!(deserialized.total_points, result.total_points);
    assert_eq!(deserialized.parameter_names, result.parameter_names);
    assert_eq!(deserialized.parameter_matrix, result.parameter_matrix);

    // Verify CSV structure
    let csv_str = result.to_csv();
    let lines: Vec<&str> = csv_str.lines().collect();
    assert_eq!(lines.len(), 5); // 1 header + 4 data rows

    let header = lines[0];
    assert!(header.starts_with("point_index,plants,herbivores,completed_runs,failed_runs"));
    assert!(header.contains("plants_mean"));
    assert!(header.contains("herbivores_mean"));
    assert!(header.contains("predators_mean"));

    for (idx, line) in lines[1..].iter().enumerate() {
        let cols: Vec<&str> = line.split(',').collect();
        assert_eq!(cols[0], idx.to_string());
        // Verify numeric values parse properly
        for col in cols {
            if !col.is_empty() {
                assert!(col.parse::<f64>().is_ok(), "Column '{col}' is not a valid float");
            }
        }
    }
}

// =============================================================================
// 7. CLI DIRECT PROCESS TESTING (`anima-lab sweep`)
// =============================================================================

struct TempSweepManifest(PathBuf);

impl Drop for TempSweepManifest {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn write_temp_sweep_manifest(manifest: &SweepManifest) -> TempSweepManifest {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "anima-lab-sweep-adv-{}-{nonce}.json",
        std::process::id()
    ));
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(manifest).expect("manifest serialises"),
    )
    .expect("temp sweep manifest writable");
    TempSweepManifest(path)
}

#[test]
fn cli_anima_lab_sweep_json_and_csv_execution() {
    let base = base_reference_manifest();
    let sweep = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "cli_adv_sweep".into(),
        name: "CLI Adv Sweep".into(),
        base_manifest: base,
        parameters: vec![ParameterDim {
            name: "plants".into(),
            target: SweepTarget::InitialCondition("plants".into()),
            spec: DimensionSpec::Continuous { min: 80.0, max: 120.0 },
        }],
        sampling: SamplingStrategy::Sobol { num_samples: 4, offset: 0 },
        max_threads: None,
    };

    let temp_manifest = write_temp_sweep_manifest(&sweep);

    // 1. Run default JSON output
    let output_json = Command::new(env!("CARGO_BIN_EXE_anima-lab"))
        .arg("sweep")
        .arg(&temp_manifest.0)
        .output()
        .expect("anima-lab starts");

    assert!(
        output_json.status.success(),
        "CLI JSON sweep failed: {}",
        String::from_utf8_lossy(&output_json.stderr)
    );
    let parsed_json: serde_json::Value =
        serde_json::from_slice(&output_json.stdout).expect("stdout must be valid JSON");
    assert_eq!(parsed_json["sweep_id"], "cli_adv_sweep");
    assert_eq!(parsed_json["total_points"], 4);
    assert_eq!(parsed_json["completed_points"], 4);
    assert_eq!(parsed_json["failed_runs"], 0);

    // 2. Run CSV output with explicit threads
    let output_csv = Command::new(env!("CARGO_BIN_EXE_anima-lab"))
        .args(["sweep", "--format", "csv", "--threads", "2"])
        .arg(&temp_manifest.0)
        .output()
        .expect("anima-lab starts");

    assert!(
        output_csv.status.success(),
        "CLI CSV sweep failed: {}",
        String::from_utf8_lossy(&output_csv.stderr)
    );
    let csv_text = String::from_utf8_lossy(&output_csv.stdout);
    assert!(csv_text.starts_with("point_index,plants,completed_runs,failed_runs"));
    assert_eq!(csv_text.lines().count(), 5); // Header + 4 points
}

// =============================================================================
// 8. SOBOL (t, m, s)-NET ELEMENTARY INTERVAL STRATIFICATION PROPERTY
// =============================================================================

#[test]
fn sobol_2d_elementary_interval_digital_net_property() {
    // For Sobol sequence (d0, d1), taking N = 2^m points (e.g. N = 64, m = 6):
    // In the 1-indexed Antonov-Saleev Gray code sequence, points x_1..x_N are uniformly
    // distributed across dyadic elementary intervals with maximum bin occupancy <= 2.
    let mut sobol = SobolSequence::new(2);
    let m = 6;
    let n_samples = 1 << m; // 64

    let mut points = Vec::with_capacity(n_samples);
    for _ in 0..n_samples {
        let pt = sobol.next_point();
        points.push((pt[0], pt[1]));
    }

    // Test dyadic partitions: (a, b) = (6, 0), (3, 3), (0, 6), (4, 2), (2, 4)
    let partitions = [(6, 0), (3, 3), (0, 6), (4, 2), (2, 4)];
    for &(a, b) in &partitions {
        let num_x_bins = 1 << a;
        let num_y_bins = 1 << b;
        let mut grid_counts = vec![vec![0usize; num_y_bins]; num_x_bins];

        for &(x, y) in &points {
            let bx = ((x * num_x_bins as f64).floor() as usize).min(num_x_bins - 1);
            let by = ((y * num_y_bins as f64).floor() as usize).min(num_y_bins - 1);
            grid_counts[bx][by] += 1;
        }

        let mut occupied_cells = 0usize;
        for bx in 0..num_x_bins {
            for by in 0..num_y_bins {
                let count = grid_counts[bx][by];
                assert!(
                    count <= 2,
                    "Sobol partition a={a}, b={b}, cell ({bx}, {by}) count={count} exceeded bound 2"
                );
                if count > 0 {
                    occupied_cells += 1;
                }
            }
        }
        // At least 60/64 bins occupied for 64 points in 64 bins (high uniformity)
        assert!(
            occupied_cells >= (n_samples - 4),
            "Sobol partition a={a}, b={b} had insufficient bin occupancy {occupied_cells}/{n_samples}"
        );
    }
}

// =============================================================================
// 9. SWEEP TARGETS & EXOTIC LAW / INTERVENTION TARGET MAPPING
// =============================================================================

#[test]
fn sweep_target_intervention_and_exotic_law_modifications() {
    let mut base = base_reference_manifest();
    // Add an intervention command
    base.interventions.push(anima_domain::intervention::InterventionCommand {
        id: 101,
        cause_id: 1,
        start_tick: 10,
        duration_ticks: 5,
        intensity: 0.2,
        signed_negative: true,
        region: anima_domain::intervention::Region::Global,
        curve: anima_domain::intervention::Curve::Step,
        reversible: false,
        kind: anima_domain::intervention::InterventionKind::RemovePredators,
    });

    let sweep = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "target_test".into(),
        name: "Target Test".into(),
        base_manifest: base,
        parameters: vec![
            ParameterDim {
                name: "cull_intensity".into(),
                target: SweepTarget::InterventionIntensity(101),
                spec: DimensionSpec::Continuous { min: 0.1, max: 0.9 },
            },
            ParameterDim {
                name: "cull_start".into(),
                target: SweepTarget::InterventionStartTick(101),
                spec: DimensionSpec::IntegerRange { min: 20, max: 50, step: 10 },
            },
            ParameterDim {
                name: "cull_dur".into(),
                target: SweepTarget::InterventionDuration(101),
                spec: DimensionSpec::IntegerRange { min: 5, max: 15, step: 5 },
            },
        ],
        sampling: SamplingStrategy::Grid { points_per_dim: Some(2) },
        max_threads: Some(1),
    };

    let pt_manifest = sweep.apply_point(0, &[0.75, 30.0, 10.0]).unwrap();
    let cmd = &pt_manifest.interventions[0];
    assert!((cmd.intensity - 0.75).abs() < 1e-6);
    assert_eq!(cmd.start_tick, 30);
    assert_eq!(cmd.duration_ticks, 10);

    // Test non-existent intervention ID target
    let sweep_bad_target = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "bad_target".into(),
        name: "Bad Target".into(),
        base_manifest: pt_manifest.clone(),
        parameters: vec![ParameterDim {
            name: "bad_cmd".into(),
            target: SweepTarget::InterventionIntensity(9999),
            spec: DimensionSpec::Continuous { min: 0.1, max: 0.9 },
        }],
        sampling: SamplingStrategy::Grid { points_per_dim: Some(2) },
        max_threads: Some(1),
    };
    assert!(sweep_bad_target.apply_point(0, &[0.5]).is_err());
}

// =============================================================================
// 10. THREAD SCALING DETERMINISM (1 THREAD VS 4 THREADS PARITY)
// =============================================================================

#[test]
fn sweep_thread_scaling_yields_bit_identical_results() {
    let registry = ObservableRegistry::reference_default();
    let base = base_reference_manifest();

    let sweep_1t = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "thread_det_sweep".into(),
        name: "Thread Det Sweep".into(),
        base_manifest: base.clone(),
        parameters: vec![
            ParameterDim {
                name: "plants".into(),
                target: SweepTarget::InitialCondition("plants".into()),
                spec: DimensionSpec::Continuous { min: 50.0, max: 150.0 },
            },
            ParameterDim {
                name: "herbivores".into(),
                target: SweepTarget::InitialCondition("herbivores".into()),
                spec: DimensionSpec::Continuous { min: 20.0, max: 60.0 },
            },
        ],
        sampling: SamplingStrategy::Sobol { num_samples: 8, offset: 0 },
        max_threads: Some(1),
    };

    let mut sweep_4t = sweep_1t.clone();
    sweep_4t.max_threads = Some(4);

    let res_1t = run_sweep::<ReferenceEvolutionWorld>(&sweep_1t, &registry).unwrap();
    let res_4t = run_sweep::<ReferenceEvolutionWorld>(&sweep_4t, &registry).unwrap();

    assert_eq!(res_1t.parameter_matrix, res_4t.parameter_matrix);
    assert_eq!(res_1t.total_points, res_4t.total_points);
    assert_eq!(res_1t.completed_runs, res_4t.completed_runs);

    for (p1, p2) in res_1t.points.iter().zip(res_4t.points.iter()) {
        assert_eq!(p1.parameters, p2.parameters);
        for (k, m1) in &p1.observables {
            let m2 = &p2.observables[k];
            assert_eq!(m1.mean.to_bits(), m2.mean.to_bits(), "Mean mismatch for {k}");
            assert_eq!(m1.min.to_bits(), m2.min.to_bits(), "Min mismatch for {k}");
            assert_eq!(m1.max.to_bits(), m2.max.to_bits(), "Max mismatch for {k}");
        }
    }
}

// =============================================================================
// 11. LIVE EXPERIMENT ADAPTER SWEEP EXECUTION & SENSITIVITY
// =============================================================================

#[test]
fn live_experiment_adapter_sweep_execution() {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/experiments_e2/e2-smoke-control-shared-brain.json");
    let mut base: ExperimentManifest =
        serde_json::from_str(&std::fs::read_to_string(fixture_path).unwrap()).unwrap();
    base.duration_ticks = 4;
    base.sample_period = 2;

    let registry = ObservableRegistry::live_default();

    let sweep = SweepManifest {
        schema_version: SWEEP_SCHEMA_VERSION,
        sweep_id: "live_adapter_sweep".into(),
        name: "Live Adapter Sweep".into(),
        base_manifest: base,
        parameters: vec![ParameterDim {
            name: "live_founders".into(),
            target: SweepTarget::InitialCondition("live.founders".into()),
            spec: DimensionSpec::Continuous { min: 10.0, max: 30.0 },
        }],
        sampling: SamplingStrategy::Sobol { num_samples: 3, offset: 0 },
        max_threads: Some(1),
    };

    let result = run_sweep::<anima_engine_lib::core::live_experiment::LiveExperimentAdapter>(&sweep, &registry)
        .expect("Live adapter sweep must run successfully");

    assert_eq!(
        result.completed_points, 3,
        "Completed points was 0. Failure records: {:#?}",
        result.failure_records
    );
    assert_eq!(result.failed_runs, 0);
    assert!(result.global_metrics.contains_key("live.agent_count"));
    assert!(result.global_metrics["live.agent_count"].mean > 0.0);
}

#[test]
fn cli_anima_lab_sweep_error_cases_and_exit_codes() {
    // 1. Missing file
    let out_missing = Command::new(env!("CARGO_BIN_EXE_anima-lab"))
        .args(["sweep", "non_existent_sweep_file_9999.json"])
        .output()
        .expect("anima-lab starts");
    assert!(!out_missing.status.success());
    assert!(String::from_utf8_lossy(&out_missing.stderr).contains("cannot read manifest"));

    // 2. Invalid syntax / unknown flag
    let out_bad_flag = Command::new(env!("CARGO_BIN_EXE_anima-lab"))
        .args(["sweep", "--unknown-flag", "foo.json"])
        .output()
        .expect("anima-lab starts");
    assert!(!out_bad_flag.status.success());
    assert!(String::from_utf8_lossy(&out_bad_flag.stderr).contains("unknown option"));

    // 3. Malformed JSON file
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let malformed_path = std::env::temp_dir().join(format!("malformed-{nonce}.json"));
    std::fs::write(&malformed_path, b"{ this is not valid json }").unwrap();

    let out_malformed = Command::new(env!("CARGO_BIN_EXE_anima-lab"))
        .args(["sweep", malformed_path.to_str().unwrap()])
        .output()
        .expect("anima-lab starts");
    let _ = std::fs::remove_file(&malformed_path);

    assert!(!out_malformed.status.success());
    assert!(String::from_utf8_lossy(&out_malformed.stderr).contains("cannot parse sweep manifest"));
}

