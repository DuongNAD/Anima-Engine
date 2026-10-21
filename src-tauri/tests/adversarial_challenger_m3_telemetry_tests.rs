//! Empirical Adversarial Challenger Test Suite for Milestone 3 Telemetry:
//! 1. Spatial density downsampling invariance & coordinate clamping under extreme resolutions and boundary conditions
//! 2. Newick tree syntax validity, balanced parentheses, deep chains (1000 generations), wide branches, and DAG crossover resolution
//! 3. Dataset exporter RFC 4180 CSV compliance, JSON schema roundtrip, and RunProvenance completeness
//! 4. Path traversal injection payload security & Windows reserved device name rejections

use anima_engine_lib::commands::save_paths::{sanitize_save_name, SaveNameError, MAX_LEN};
use anima_engine_lib::core::ecs::AgentType;
use anima_engine_lib::core::resources::MapBounds;
use anima_engine_lib::core::simulation_state::SegmentState;
use anima_engine_lib::core::telemetry_export::*;
use anima_engine_lib::evolution::genotype::MorphologyGenotype;
use anima_engine_lib::evolution::lineage::{FallbackLineageTracker, LineageNode, LineageRelation, LineageTracker, RelationType};
use anima_engine_lib::evolution::newick::{to_newick, NewickError};

// ===============================================================================================
// 1. SPATIAL DENSITY DOWNSAMPLING INVARIANCE & EXTREME RESOLUTIONS
// ===============================================================================================

#[test]
fn test_adversarial_spatial_density_invariance_extreme_resolutions() {
    let mut segments = Vec::new();
    let num_prey = 350;
    let num_predators = 150;

    // Create 350 prey and 150 predators, with root segments and multiple child limb segments
    for i in 0..num_prey {
        // Root segment
        segments.push(SegmentState {
            agent_id: i,
            segment_id: 0,
            parent_segment_id: None,
            agent_type: Some(AgentType::Prey),
            x: ((i as f32 * 17.0) % 190.0) - 95.0,
            y: 0.0,
            z: ((i as f32 * 31.0) % 190.0) - 95.0,
            energy: 100.0,
            ..Default::default()
        });
        // 2 child limb segments per agent (must NOT be double counted)
        segments.push(SegmentState {
            agent_id: i,
            segment_id: 1,
            parent_segment_id: Some(0),
            agent_type: Some(AgentType::Prey),
            x: ((i as f32 * 17.0) % 190.0) - 95.0,
            y: 0.0,
            z: ((i as f32 * 31.0) % 190.0) - 95.0,
            energy: 50.0,
            ..Default::default()
        });
        segments.push(SegmentState {
            agent_id: i,
            segment_id: 2,
            parent_segment_id: Some(1),
            agent_type: Some(AgentType::Prey),
            x: ((i as f32 * 17.0) % 190.0) - 95.0,
            y: 0.0,
            z: ((i as f32 * 31.0) % 190.0) - 95.0,
            energy: 25.0,
            ..Default::default()
        });
    }

    for j in 0..num_predators {
        let agent_id = num_prey + j;
        // Root segment
        segments.push(SegmentState {
            agent_id,
            segment_id: 0,
            parent_segment_id: None,
            agent_type: Some(AgentType::Predator),
            x: ((j as f32 * 23.0) % 180.0) - 90.0,
            y: 0.0,
            z: ((j as f32 * 41.0) % 180.0) - 90.0,
            energy: 150.0,
            ..Default::default()
        });
        // Child segment
        segments.push(SegmentState {
            agent_id,
            segment_id: 1,
            parent_segment_id: Some(0),
            agent_type: Some(AgentType::Predator),
            x: ((j as f32 * 23.0) % 180.0) - 90.0,
            y: 0.0,
            z: ((j as f32 * 41.0) % 180.0) - 90.0,
            energy: 75.0,
            ..Default::default()
        });
    }

    let bounds = MapBounds {
        min: glam::Vec3::new(-100.0, 0.0, -100.0),
        max: glam::Vec3::new(100.0, 0.0, 100.0),
    };

    // Test across non-square, extreme, and standard resolutions
    let resolutions = [
        (1, 1),
        (1, 10),
        (10, 1),
        (3, 7),
        (7, 3),
        (16, 16),
        (32, 32),
        (64, 64),
        (128, 128),
        (256, 128),
        (0, 0), // Should clamp to at least 1x1 without division by zero or panic
    ];

    for (w, h) in resolutions {
        let report = generate_spatial_density_matrix(w, h, &segments, Some(&bounds), None, &[]);

        let expected_w = w.max(1);
        let expected_h = h.max(1);
        assert_eq!(report.width, expected_w, "Width mismatch for ({w}, {h})");
        assert_eq!(report.height, expected_h, "Height mismatch for ({w}, {h})");
        assert_eq!(report.prey_density.len(), (expected_w * expected_h) as usize);
        assert_eq!(report.predator_density.len(), (expected_w * expected_h) as usize);
        assert_eq!(report.vegetation_density.len(), (expected_w * expected_h) as usize);
        assert_eq!(report.predation_hotspots.len(), (expected_w * expected_h) as usize);

        let sum_prey: u32 = report.prey_density.iter().sum();
        let sum_pred: u32 = report.predator_density.iter().sum();

        assert_eq!(
            sum_prey, num_prey,
            "Spatial density prey conservation violated for resolution ({w}, {h}): sum={sum_prey} != expected={num_prey}"
        );
        assert_eq!(
            sum_pred, num_predators,
            "Spatial density predator conservation violated for resolution ({w}, {h}): sum={sum_pred} != expected={num_predators}"
        );
    }
}

#[test]
fn test_adversarial_spatial_density_out_of_bounds_clamping_and_nan() {
    let bounds = MapBounds {
        min: glam::Vec3::new(-100.0, 0.0, -100.0),
        max: glam::Vec3::new(100.0, 0.0, 100.0),
    };

    // Adversarial agent coordinates: extreme out-of-bounds, infinite, NaN, and exact boundaries
    let segments = vec![
        // Extreme negative coordinates
        SegmentState {
            agent_id: 1,
            segment_id: 0,
            parent_segment_id: None,
            agent_type: Some(AgentType::Prey),
            x: -999999.0,
            y: 0.0,
            z: -999999.0,
            energy: 100.0,
            ..Default::default()
        },
        // Extreme positive coordinates
        SegmentState {
            agent_id: 2,
            segment_id: 0,
            parent_segment_id: None,
            agent_type: Some(AgentType::Predator),
            x: 999999.0,
            y: 0.0,
            z: 999999.0,
            energy: 100.0,
            ..Default::default()
        },
        // NaN coordinates
        SegmentState {
            agent_id: 3,
            segment_id: 0,
            parent_segment_id: None,
            agent_type: Some(AgentType::Prey),
            x: f32::NAN,
            y: 0.0,
            z: f32::NAN,
            energy: 100.0,
            ..Default::default()
        },
        // Infinity coordinates
        SegmentState {
            agent_id: 4,
            segment_id: 0,
            parent_segment_id: None,
            agent_type: Some(AgentType::Predator),
            x: f32::INFINITY,
            y: 0.0,
            z: f32::NEG_INFINITY,
            energy: 100.0,
            ..Default::default()
        },
        // Exact boundary corners
        SegmentState {
            agent_id: 5,
            segment_id: 0,
            parent_segment_id: None,
            agent_type: Some(AgentType::Prey),
            x: -100.0,
            y: 0.0,
            z: -100.0,
            energy: 100.0,
            ..Default::default()
        },
        SegmentState {
            agent_id: 6,
            segment_id: 0,
            parent_segment_id: None,
            agent_type: Some(AgentType::Prey),
            x: 100.0,
            y: 0.0,
            z: 100.0,
            energy: 100.0,
            ..Default::default()
        },
    ];

    let report = generate_spatial_density_matrix(16, 16, &segments, Some(&bounds), None, &[]);

    let sum_prey: u32 = report.prey_density.iter().sum();
    let sum_pred: u32 = report.predator_density.iter().sum();

    // 4 prey, 2 predators — all must be safely clamped into valid grid cells without panicking
    assert_eq!(sum_prey, 4, "Prey count conservation violated with out-of-bounds coords");
    assert_eq!(sum_pred, 2, "Predator count conservation violated with out-of-bounds coords");
}

// ===============================================================================================
// 2. NEWICK TREE SYNTAX VALIDITY & STRESS HARNESS
// ===============================================================================================

fn verify_newick_syntax(newick_str: &str) {
    let trimmed = newick_str.trim();
    assert!(!trimmed.is_empty(), "Newick string is empty");
    assert!(trimmed.ends_with(';'), "Newick string must end with ';', got: {trimmed}");

    // Verify balanced parentheses
    let mut depth: i32 = 0;
    let mut in_quotes = false;
    let mut prev_char = ' ';

    for c in trimmed.chars() {
        if c == '\'' && prev_char != '\\' {
            in_quotes = !in_quotes;
        } else if !in_quotes {
            if c == '(' {
                depth += 1;
            } else if c == ')' {
                depth -= 1;
                assert!(depth >= 0, "Unmatched closing parenthesis in Newick: {trimmed}");
            }
        }
        prev_char = c;
    }
    assert_eq!(depth, 0, "Unclosed open parenthesis in Newick: {trimmed}");
    assert!(!in_quotes, "Unclosed single quote in Newick: {trimmed}");
}

#[test]
fn test_adversarial_newick_empty_and_single_node() {
    let tracker = FallbackLineageTracker::new("bolt://localhost:7687", "neo4j", "pass");

    // Empty tracker returns "();"
    let empty_tree = export_lineage_newick(&tracker, None).unwrap();
    assert_eq!(empty_tree, "();");
    verify_newick_syntax(&empty_tree);

    // Single founder root node with hyphen (unquoted)
    let tracker_unquoted = FallbackLineageTracker::new("bolt://localhost:7687", "neo4j", "pass");
    tracker_unquoted
        .add_root("founder-root".to_string(), MorphologyGenotype::default())
        .unwrap();
    let unquoted_tree = export_lineage_newick(&tracker_unquoted, None).unwrap();
    assert_eq!(unquoted_tree, "founder-root;");
    verify_newick_syntax(&unquoted_tree);

    // Single founder root node with underscore (quoted per Newick standard)
    tracker
        .add_root("founder_root".to_string(), MorphologyGenotype::default())
        .unwrap();
    let single_tree = export_lineage_newick(&tracker, None).unwrap();
    assert_eq!(single_tree, "'founder_root';");
    verify_newick_syntax(&single_tree);
}

#[test]
fn test_adversarial_newick_deep_linear_tree_1000_generations() {
    let tracker = FallbackLineageTracker::new("bolt://localhost:7687", "neo4j", "pass");

    let root_id = "gen-0".to_string();
    tracker.add_root(root_id.clone(), MorphologyGenotype::default()).unwrap();

    let mut parent = root_id;
    for gen in 1..=1000 {
        let child = format!("gen-{gen}");
        tracker
            .add_reproduction(
                child.clone(),
                gen,
                MorphologyGenotype::default(),
                vec![parent],
                RelationType::Mutate,
            )
            .unwrap();
        parent = child;
    }

    // Must execute iteratively without stack overflow
    let tree = export_lineage_newick(&tracker, None).unwrap();
    verify_newick_syntax(&tree);
    assert!(tree.contains("gen-1000"));
    assert!(tree.contains("gen-0"));
}

#[test]
fn test_adversarial_newick_wide_branching_tree() {
    let tracker = FallbackLineageTracker::new("bolt://localhost:7687", "neo4j", "pass");

    let root_id = "broad-root".to_string();
    tracker.add_root(root_id.clone(), MorphologyGenotype::default()).unwrap();

    for i in 0..1000 {
        let child = format!("child-{i:04}");
        tracker
            .add_reproduction(
                child,
                1,
                MorphologyGenotype::default(),
                vec![root_id.clone()],
                RelationType::Mutate,
            )
            .unwrap();
    }

    let tree = export_lineage_newick(&tracker, None).unwrap();
    verify_newick_syntax(&tree);
    assert!(tree.starts_with('('));
    assert!(tree.contains("child-0000:1"));
    assert!(tree.contains("child-0999:1"));
    assert!(tree.ends_with("broad-root;"));
}

#[test]
fn test_adversarial_newick_crossover_dag_and_dropped_edges() {
    let tracker = FallbackLineageTracker::new("bolt://localhost:7687", "neo4j", "pass");

    tracker.add_root("parent_a".to_string(), MorphologyGenotype::default()).unwrap();
    tracker.add_root("parent_b".to_string(), MorphologyGenotype::default()).unwrap();

    // Crossover child with two parents
    tracker
        .add_reproduction(
            "hybrid_child".to_string(),
            1,
            MorphologyGenotype::default(),
            vec!["parent_a".to_string(), "parent_b".to_string()],
            RelationType::Crossover,
        )
        .unwrap();

    let (nodes, relations) = tracker.get_lineage_graph().unwrap();
    let export_result = to_newick(&nodes, &relations).expect("Crossover DAG must be exportable as tree view");

    assert_eq!(export_result.dropped_parent_edges, 1, "Crossover must record exactly 1 dropped parent edge");
    assert_eq!(export_result.roots, 2, "Forest must have 2 roots");

    let file_content = export_result.to_file_contents();
    for line in file_content.lines() {
        if !line.trim().is_empty() {
            verify_newick_syntax(line);
        }
    }
}

#[test]
fn test_adversarial_newick_cycles_and_generation_inversions_rejected() {
    // 1. Cycle rejection
    let nodes = vec![
        LineageNode {
            id: "loop_a".to_string(),
            generation: 1,
            genotype: None,
            cumulative_mutations: None,
        },
        LineageNode {
            id: "loop_b".to_string(),
            generation: 2,
            genotype: None,
            cumulative_mutations: None,
        },
    ];
    let cyclic_relations = vec![
        LineageRelation {
            source_id: "loop_a".to_string(),
            target_id: "loop_b".to_string(),
            relation_type: RelationType::Mutate,
            path_events: None,
        },
        LineageRelation {
            source_id: "loop_b".to_string(),
            target_id: "loop_a".to_string(),
            relation_type: RelationType::Mutate,
            path_events: None,
        },
    ];

    let result = to_newick(&nodes, &cyclic_relations);
    assert!(matches!(result, Err(NewickError::Cycle { .. })), "Cycles must be rejected with NewickError::Cycle");

    // 2. Generation Inversion rejection
    let inv_nodes = vec![
        LineageNode {
            id: "old_parent".to_string(),
            generation: 50,
            genotype: None,
            cumulative_mutations: None,
        },
        LineageNode {
            id: "impossible_child".to_string(),
            generation: 10, // Inverted! Child generation < parent generation
            genotype: None,
            cumulative_mutations: None,
        },
    ];
    let inv_relations = vec![LineageRelation {
        source_id: "old_parent".to_string(),
        target_id: "impossible_child".to_string(),
        relation_type: RelationType::Mutate,
        path_events: None,
    }];

    let inv_result = to_newick(&inv_nodes, &inv_relations);
    assert!(
        matches!(inv_result, Err(NewickError::GenerationInversion { .. })),
        "Generation inversion must be rejected with NewickError::GenerationInversion"
    );
}

// ===============================================================================================
// 3. DATASET EXPORTER: RFC 4180 CSV COMPLIANCE & JSON SCHEMA ROUNDTRIP
// ===============================================================================================

#[test]
fn test_adversarial_dataset_exporter_rfc4180_csv_compliance() {
    let samples = vec![
        TimeSeriesSample {
            tick: 0,
            plants_biomass: 0.0,
            prey_biomass: 0.0,
            predator_biomass: 0.0,
            detritus_biomass: 0.0,
            total_energy: 0.0,
            energy_residual: 0.0,
            prey_count: 0,
            predator_count: 0,
            shannon_diversity: 0.0,
            simpson_diversity: 0.0,
            avg_prey_metabolic_rate: 0.0,
            avg_predator_metabolic_rate: 0.0,
            ambient_temperature_c: -10.5,
        },
        TimeSeriesSample {
            tick: 10000,
            plants_biomass: 123456.789,
            prey_biomass: 9876.543,
            predator_biomass: 1234.567,
            detritus_biomass: 4321.0,
            total_energy: 9999999.99,
            energy_residual: 0.0000001,
            prey_count: 5000,
            predator_count: 200,
            shannon_diversity: 0.999,
            simpson_diversity: 0.888,
            avg_prey_metabolic_rate: 0.0123,
            avg_predator_metabolic_rate: 0.0456,
            ambient_temperature_c: 35.2,
        },
    ];

    let csv = render_csv_time_series(&samples);
    let lines: Vec<&str> = csv.lines().collect();

    // RFC 4180 validation
    assert!(lines.len() >= 3, "CSV must contain header + 2 data rows");
    assert_eq!(lines[0], "tick,plants,prey,predators", "Header format violation");

    for (i, line) in lines.iter().skip(1).enumerate() {
        let fields: Vec<&str> = line.split(',').collect();
        assert_eq!(fields.len(), 4, "Row {i} does not have exactly 4 columns: {line}");

        // Validate each column parses into its expected numeric type
        let tick: u64 = fields[0].parse().expect("tick must be u64");
        let plants: f64 = fields[1].parse().expect("plants must be f64");
        let prey: usize = fields[2].parse().expect("prey must be usize");
        let predators: usize = fields[3].parse().expect("predators must be usize");

        assert_eq!(tick, samples[i].tick);
        assert!((plants - samples[i].plants_biomass).abs() < 0.01);
        assert_eq!(prey, samples[i].prey_count);
        assert_eq!(predators, samples[i].predator_count);
    }
}

#[test]
fn test_adversarial_dataset_exporter_json_and_provenance_roundtrip() {
    let prov = RunProvenance::new(424242, 10000);
    assert_eq!(prov.seed, 424242);
    assert_eq!(prov.duration_ticks, 10000);
    assert!(!prov.commit_hash.is_empty());
    assert!(!prov.build_id.is_empty());
    assert!(!prov.engine_version.is_empty());
    assert!(!prov.architecture.is_empty());
    assert!(!prov.os.is_empty());

    let package = ScientificDatasetPackage {
        schema_version: 1,
        provenance: prov.clone(),
        time_series: vec![TimeSeriesSample {
            tick: 1,
            plants_biomass: 1000.0,
            prey_biomass: 200.0,
            predator_biomass: 50.0,
            detritus_biomass: 300.0,
            total_energy: 1550.0,
            energy_residual: 0.0,
            prey_count: 20,
            predator_count: 5,
            shannon_diversity: 0.5,
            simpson_diversity: 0.4,
            avg_prey_metabolic_rate: 0.02,
            avg_predator_metabolic_rate: 0.05,
            ambient_temperature_c: 20.0,
        }],
        demographics: Some(DemographicCensusState {
            prey_count: 20,
            predator_count: 5,
            male_count: 12,
            female_count: 13,
            sex_ratio: 12.0 / 13.0,
            total_births: 50,
            total_deaths: 25,
            shannon: 0.5,
            simpson: 0.4,
            age_cohorts: vec![],
            mortality_causes: vec![],
            fecundity_by_age: vec![],
        }),
        energy_flux: Some(EnergyFluxReport {
            nodes: vec![],
            links: vec![],
            respiration_loss: 10.0,
            total_system_energy: 1550.0,
            energy_drift_percent: 0.001,
        }),
        spatial_density: Some(SpatialDensityReport {
            width: 16,
            height: 16,
            prey_density: vec![0; 256],
            predator_density: vec![0; 256],
            vegetation_density: vec![0.5; 256],
            predation_hotspots: vec![0; 256],
        }),
        map_elites_summary: Some(MapElitesSummary {
            total_fitness: 500.0,
            archive_coverage: 15.5,
            occupied_cells: 31,
            max_cells: 200,
        }),
        newick_tree: Some("(a:1,b:1)root;".to_string()),
    };

    // Serialize to JSON
    let json_str = serde_json::to_string_pretty(&package).expect("Dataset package must serialize to JSON");
    assert!(json_str.contains("\"schemaVersion\": 1"));
    assert!(json_str.contains("\"provenance\""));
    assert!(json_str.contains("\"timeSeries\""));

    // Deserialize back and assert full parity
    let deserialized: ScientificDatasetPackage = serde_json::from_str(&json_str)
        .expect("Dataset package must deserialize from valid JSON");
    assert_eq!(deserialized, package);
}

#[test]
fn test_adversarial_iso8601_date_time_robustness() {
    // Epoch start: 1970-01-01T00:00:00Z
    assert_eq!(format_iso8601_utc(0), "1970-01-01T00:00:00Z");

    // Known Leap Year: 2000-02-29T12:34:56Z -> epoch: 951827696
    assert_eq!(format_iso8601_utc(951827696), "2000-02-29T12:34:56Z");

    // Year 2026: 2026-08-16T03:00:00Z -> epoch: 1786849200
    assert_eq!(format_iso8601_utc(1786849200), "2026-08-16T03:00:00Z");

    // Year 2038 boundary (32-bit Unix overflow safety in u64): 2038-01-19T03:14:08Z -> epoch: 2147483648
    assert_eq!(format_iso8601_utc(2147483648), "2038-01-19T03:14:08Z");
}

// ===============================================================================================
// 4. PATH TRAVERSAL SECURITY & FILENAME SANITIZATION
// ===============================================================================================

#[test]
fn test_adversarial_path_traversal_payload_rejections() {
    let evil_payloads = [
        // Classic traversal
        "../../etc/passwd",
        "..\\..\\Windows\\System32\\cmd.exe",
        "../foo.json",
        "..\\foo.json",
        "/etc/shadow",
        "\\Windows\\explorer.exe",
        "~/.ssh/id_rsa",
        "//192.168.1.1/share/evil",
        "\\\\192.168.1.1\\share\\evil",
        "C:\\Windows\\System32",
        "D:/games/mod.dll",
        // Windows reserved devices
        "CON",
        "con.json",
        "PRN",
        "prn.json",
        "AUX",
        "aux.json",
        "NUL",
        "nul.json",
        "COM1",
        "com1.json",
        "COM9",
        "com9.json",
        "LPT1",
        "lpt1.json",
        "LPT9",
        "lpt9.json",
        "CoN.JSON",
        "NuL.JsOn",
        // Nested subdirectories
        "foo/bar/baz.json",
        "foo\\bar\\baz.json",
        "a/b",
        "a\\b",
        // Pure dots
        ".",
        "..",
        "...",
        "....",
        "....json",
        "...json",
        "..json",
        ".json",
        // Dangerous characters & streams
        "save\0evil.json",
        "save:stream:$DATA",
        "save*wildcard.json",
        "save?wildcard.json",
        "save|pipe.json",
        "save<redirect.json",
        "save>redirect.json",
        "save\"quote.json",
        "save%20escape.json",
        "save'single.json",
        "save;semicolon.json",
        "save space inside.json",
        // Empty / Whitespace
        "",
        "   ",
        "\t\r\n",
    ];

    let tmp_dir = std::env::temp_dir().join(format!("adversarial_security_test_{}", std::process::id()));

    for evil in evil_payloads {
        let san_res = sanitize_save_name(evil);
        assert!(
            san_res.is_err(),
            "sanitize_save_name unexpectedly ACCEPTED hazardous payload: {evil:?} -> {san_res:?}"
        );

        let export_res = save_scientific_export(&tmp_dir, evil, b"{}");
        assert!(
            export_res.is_err(),
            "save_scientific_export unexpectedly ACCEPTED hazardous payload: {evil:?} -> {export_res:?}"
        );
    }

    // Test max length boundary
    let too_long = "a".repeat(MAX_LEN + 1);
    assert_eq!(sanitize_save_name(&too_long), Err(SaveNameError::TooLong(MAX_LEN + 1)));

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_adversarial_path_traversal_sanitizes_valid_inputs_safely() {
    let tmp_dir = std::env::temp_dir().join(format!("adversarial_valid_test_{}", std::process::id()));

    let valid_inputs = [
        ("experiment_run_1", "experiment_run_1.json"),
        ("experiment_run_1.json", "experiment_run_1.json"),
        ("sweep-2026-08-16", "sweep-2026-08-16.json"),
        ("alpha.beta.gamma", "alpha.beta.gamma.json"),
        ("alpha.beta.gamma.json", "alpha.beta.gamma.json"),
        ("   padded_name   ", "padded_name.json"),
        ("a", "a.json"),
        ("0", "0.json"),
    ];

    for (raw, expected) in valid_inputs {
        let sanitized = sanitize_save_name(raw).unwrap_or_else(|e| panic!("Failed to sanitize valid input '{raw}': {e}"));
        assert_eq!(sanitized, expected);

        let path = save_scientific_export(&tmp_dir, raw, b"{\"test\": true}").unwrap();
        assert!(path.exists());
        assert_eq!(path.file_name().unwrap().to_str().unwrap(), expected);
    }

    let _ = std::fs::remove_dir_all(&tmp_dir);
}
