//! Round 3 hardening & real-bug characterization tests (`HD_SEED = 0x5EED`).
//!
//! Covers four independent defects across the scientific simulation pipeline:
//! - `F-062`: `run_branch_tree` drops parent/ancestor `interventions` when executing descendant forks.
//! - `F-063`: `to_newick` / `to_newick_from` silently discards self-loops and cycles when a smaller
//!   `parent_id` appears first on the target node.
//! - `F-064`: `SobolSequence` `SOBOL_POLY_TABLE` contains invalid `init_m >= 2^(i+1)` and non-primitive/
//!   reducible polynomials, misaligning high-dimensional Sobol direction numbers and inducing strong correlation.
//! - `F-065`: `pearson_correlation` and `DimensionSpec::Logarithmic::grid_values` use unscaled
//!   `<= 1e-15` thresholds, misclassifying valid small-scale positive series as zero-variance / single-point.

use anima_engine_lib::core::branch_tree::{
    run_branch_tree, BranchNodeSpecification, BranchSeedStrategy, BranchTreeManifest,
    BRANCH_TREE_SCHEMA_VERSION,
};
use anima_engine_lib::core::exotic_energy::{
    ExoticEnergyLaw, ExoticIntervention, ExoticInterventionKind,
};
use anima_engine_lib::core::experiment::{
    ExperimentError, ExperimentManifest, InitialConditionSet, ObservableRegistry, ObserverPolicy,
    WorldIdentity, WorldLawSet, MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::intervention::{Curve, InterventionCommand, InterventionKind, Region};
use anima_engine_lib::core::reference_world::ReferenceEvolutionWorld;
use anima_engine_lib::core::sim_clock::ECOLOGY_PERIOD;
use anima_engine_lib::core::sweeps::{
    pearson_correlation, DimensionSpec, ParameterDim, SamplingStrategy, SobolSequence,
    SweepManifest, SweepTarget, SWEEP_SCHEMA_VERSION,
};
use anima_engine_lib::evolution::lineage::{LineageNode, LineageRelation, RelationType};
use anima_engine_lib::evolution::newick::{to_newick, NewickError};

const HD_SEED: u64 = 0x5EED;

fn ref_base_manifest() -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "round3-branch-base".into(),
        name: "Round 3 Branch Base".into(),
        observer: ObserverPolicy::default(),
        world_identity: WorldIdentity {
            seed: HD_SEED as u32,
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
        seeds: vec![HD_SEED],
        duration_ticks: 200,
        sample_period: ECOLOGY_PERIOD,
        observable_ids: vec!["plants".into(), "herbivores".into(), "predators".into()],
        exotic_interventions: Vec::new(),
    }
}

fn lineage_node(id: &str, generation: u32) -> LineageNode {
    LineageNode {
        id: id.to_string(),
        generation,
        genotype: None,
        cumulative_mutations: None,
    }
}

fn lineage_edge(source: &str, target: &str) -> LineageRelation {
    LineageRelation {
        source_id: source.to_string(),
        target_id: target.to_string(),
        relation_type: RelationType::Mutate,
        path_events: None,
    }
}

fn obs_val(o: &[(String, f64)], key: &str) -> f64 {
    o.iter()
        .find(|(k, _)| k == key)
        .unwrap_or_else(|| panic!("missing observable {key}"))
        .1
}

#[test]
fn f062_branch_tree_inherits_ancestor_interventions_and_exotic_forcings() {
    let registry = ObservableRegistry::reference_default();
    let base = ref_base_manifest();

    let drought = InterventionCommand {
        id: 1,
        cause_id: 1,
        kind: InterventionKind::RainfallDelta,
        region: Region::Global,
        start_tick: 20,
        duration_ticks: 180,
        intensity: 0.8,
        signed_negative: true,
        curve: Curve::Step,
        reversible: true,
    };

    // Node 0: baseline root; Node 1: applies drought from tick 20..200; Node 2: forks from Node 1
    // at tick 50 with InheritStream and no extra interventions (must continue the exact drought).
    let manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "f062-parent-intervention-inherit".into(),
        name: "F-062 Parent Intervention Inherit".into(),
        base_manifest: base.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 0,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 200,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
            BranchNodeSpecification {
                id: 1,
                parent_id: Some(0),
                fork_tick: 20,
                duration_ticks: 200,
                interventions: vec![drought.clone()],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Child-Drought".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 50,
                duration_ticks: 200,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Grandchild-Inherit".into(),
            },
        ],
        comparisons: vec![(1, 2)],
    };

    let report = run_branch_tree::<ReferenceEvolutionWorld>(&manifest, &registry)
        .expect("branch tree runs cleanly");
    let root_run = report.nodes[&0].run_result.as_ref().expect("root run");
    let child_run = report.nodes[&1].run_result.as_ref().expect("child run");
    let grand_run = report.nodes[&2]
        .run_result
        .as_ref()
        .expect("grandchild run");

    // Non-vacuity: the drought applied on Child (Node 1) genuinely lowers plants vs Root (Node 0).
    let child_plants = obs_val(&child_run.final_observables, "plants");
    let root_plants = obs_val(&root_run.final_observables, "plants");
    assert_ne!(child_run.final_checksum, root_run.final_checksum);
    assert!(
        child_plants < root_plants,
        "drought must reduce plants on Node 1: child={child_plants}, root={root_plants}"
    );

    // F-062 regression lock (`branch_tree.rs`): Grandchild (Node 2) inherits Node 1's active
    // drought and produces the exact same trajectory and checksum as its parent.
    let grand_plants = obs_val(&grand_run.final_observables, "plants");
    assert_eq!(
        grand_run.final_checksum, child_run.final_checksum,
        "F-062: grandchild must match parent checksum when no new intervention is added"
    );
    assert_eq!(grand_plants, child_plants);
    assert_eq!(report.divergences.len(), 1);
    assert_eq!(report.divergences[0].phase_space.final_distance, 0.0);

    // 2-node case where Root holds a node-level intervention and Child inherits it.
    let root_intervention = InterventionCommand {
        start_tick: 10,
        duration_ticks: 190,
        ..drought
    };
    let two_node = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "f062-root-intervention-inherit".into(),
        name: "F-062 Root Intervention Inherit".into(),
        base_manifest: base.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 0,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 200,
                interventions: vec![root_intervention],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root-With-Drought".into(),
            },
            BranchNodeSpecification {
                id: 1,
                parent_id: Some(0),
                fork_tick: 40,
                duration_ticks: 200,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Child-No-Own-Intervention".into(),
            },
        ],
        comparisons: vec![],
    };
    let report2 = run_branch_tree::<ReferenceEvolutionWorld>(&two_node, &registry)
        .expect("two-node tree runs");
    assert_eq!(
        report2.nodes[&0]
            .run_result
            .as_ref()
            .unwrap()
            .final_checksum,
        report2.nodes[&1]
            .run_result
            .as_ref()
            .unwrap()
            .final_checksum
    );

    // 3-node exotic_interventions inheritance across Root -> Child -> Grandchild.
    let mut exotic_base = base;
    exotic_base.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_uniform(50.0));
    exotic_base.observable_ids.push("exotic.sourced".into());

    let mk_exotic = |id: u32, start_tick: u64| ExoticIntervention {
        id,
        cause_id: 900 + id,
        kind: ExoticInterventionKind::AddSource,
        region: Region::Global,
        start_tick,
        duration_ticks: 200,
        amount: 0.05,
        curve: Curve::Step,
    };
    let f_root = mk_exotic(10, 1);
    let f_child = mk_exotic(11, 30);
    let f_grand = mk_exotic(12, 90);

    let exotic_tree = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "f062-exotic-ancestor-inherit".into(),
        name: "F-062 Exotic Ancestor Inherit".into(),
        base_manifest: exotic_base.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 0,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 180,
                interventions: Vec::new(),
                exotic_interventions: vec![f_root.clone()],
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root-Exotic".into(),
            },
            BranchNodeSpecification {
                id: 1,
                parent_id: Some(0),
                fork_tick: 30,
                duration_ticks: 180,
                interventions: Vec::new(),
                exotic_interventions: vec![f_child.clone()],
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Child-Exotic".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 90,
                duration_ticks: 180,
                interventions: Vec::new(),
                exotic_interventions: vec![f_grand.clone()],
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Grandchild-Exotic".into(),
            },
        ],
        comparisons: vec![],
    };
    let exotic_report = run_branch_tree::<ReferenceEvolutionWorld>(&exotic_tree, &registry)
        .expect("exotic branch tree runs");

    // Equivalent flat manifest carrying all three forcings from t=0 must match Grandchild bit-for-bit.
    exotic_base.exotic_interventions = vec![f_root, f_child, f_grand];
    let flat_tree = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "f062-exotic-flat-ref".into(),
        name: "F-062 Exotic Flat Ref".into(),
        base_manifest: exotic_base,
        nodes: vec![BranchNodeSpecification {
            id: 0,
            parent_id: None,
            fork_tick: 0,
            duration_ticks: 180,
            interventions: Vec::new(),
            exotic_interventions: Vec::new(),
            seed_strategy: BranchSeedStrategy::InheritStream,
            label: "Flat-All-Three".into(),
        }],
        comparisons: vec![],
    };
    let flat_report = run_branch_tree::<ReferenceEvolutionWorld>(&flat_tree, &registry)
        .expect("flat exotic reference runs");

    let grand_exotic = exotic_report.nodes[&2].run_result.as_ref().unwrap();
    let flat_exotic = flat_report.nodes[&0].run_result.as_ref().unwrap();
    assert_eq!(
        grand_exotic.final_checksum, flat_exotic.final_checksum,
        "Grandchild accumulating Root+Child+Grandchild exotic forcings must match flat run"
    );
    assert_eq!(
        obs_val(&grand_exotic.final_observables, "exotic.sourced"),
        obs_val(&flat_exotic.final_observables, "exotic.sourced")
    );
}

#[test]
fn f063_newick_detects_cycles_and_self_loops_across_all_edges_and_dedups_duplicates() {
    // Case A: Self-loop B -> B masked by smaller parent id A -> B must still return Cycle { node: "B" }.
    let nodes_a = vec![lineage_node("A", 0), lineage_node("B", 1)];
    let rels_a = vec![lineage_edge("A", "B"), lineage_edge("B", "B")];
    let res_a = to_newick(&nodes_a, &rels_a);
    assert_eq!(
        res_a,
        Err(NewickError::Cycle { node: "B".into() }),
        "F-063: self-loop B -> B behind A -> B must be rejected as Cycle"
    );

    // Case B: Directed 2-cycle B -> C -> B (+ generation inversion C->B) masked because A < C on B.
    let nodes_b = vec![
        lineage_node("A", 0),
        lineage_node("B", 1),
        lineage_node("C", 2),
    ];
    let rels_b = vec![
        lineage_edge("A", "B"),
        lineage_edge("B", "C"),
        lineage_edge("C", "B"),
    ];
    match to_newick(&nodes_b, &rels_b) {
        Err(NewickError::Cycle { node }) => {
            assert!(
                node == "B" || node == "C",
                "expected cycle node on loop (B or C), got {node}"
            );
        }
        other => panic!("expected Cycle error for A->B->C->B, got {other:?}"),
    }

    // Case B2: Multi-parent cycle with a hanging tail (`A->B, B->C, C->B, C->tail`) must report a
    // node strictly on the loop (`B` or `C`), never `A` or `tail`.
    let nodes_tail = vec![
        lineage_node("A", 0),
        lineage_node("B", 1),
        lineage_node("C", 2),
        lineage_node("tail", 3),
    ];
    let rels_tail = vec![
        lineage_edge("A", "B"),
        lineage_edge("B", "C"),
        lineage_edge("C", "B"),
        lineage_edge("C", "tail"),
    ];
    match to_newick(&nodes_tail, &rels_tail) {
        Err(NewickError::Cycle { node }) => {
            assert!(
                node == "B" || node == "C",
                "reported cycle node must be on the loop, got {node}"
            );
        }
        other => panic!("expected Cycle error for tailed multi-parent cycle, got {other:?}"),
    }

    // Case C: Duplicate identical edge must not be counted as a dropped crossover edge.
    let dup_rels = vec![lineage_edge("A", "B"), lineage_edge("A", "B")];
    let export_dup = to_newick(&nodes_a, &dup_rels).expect("duplicate edge returns Ok");
    assert_eq!(export_dup.dropped_parent_edges, 0);
    assert_eq!(export_dup.trees, vec!["(B:1)A;".to_string()]);

    // Positive control: unmasked self-loop without smaller parent id IS rejected with Cycle.
    let pure_self = vec![lineage_node("B", 0)];
    let pure_rels = vec![lineage_edge("B", "B")];
    let err = to_newick(&pure_self, &pure_rels).expect_err("pure self-loop must fail");
    assert_eq!(err, NewickError::Cycle { node: "B".into() });
}

#[test]
fn f064_sobol_matches_joe_kuo_reference_points_and_decorrelates_high_dims() {
    let mut sobol = SobolSequence::new(32);
    let mut pts = Vec::with_capacity(1024);
    for _ in 0..1024 {
        pts.push(sobol.next_point());
    }

    // 1. All 1024 x 32 coordinates lie in [0.0, 1.0) and every dimension mean at N=1024 is centered.
    for (row_idx, p) in pts.iter().enumerate() {
        assert_eq!(p.len(), 32);
        for (d, &u) in p.iter().enumerate() {
            assert!(
                u.is_finite() && (0.0..1.0).contains(&u),
                "Sobol coord out of [0, 1) at row={row_idx}, dim={d}: {u}"
            );
        }
    }
    for d in 0..32 {
        let mean: f64 = pts.iter().map(|p| p[d]).sum::<f64>() / (pts.len() as f64);
        assert!(
            (0.48..=0.52).contains(&mean),
            "dim {d} mean at N=1024 drifted: {mean}"
        );
    }

    // 2. Exact u32 numerator parity against `scipy.stats.qmc.Sobol(d=32, scramble=False)`
    //    (Joe & Kuo `new-joe-kuo-6.21201`) across low, middle, and dims 23..=31.
    const REF_DIMS: [usize; 9] = [0, 1, 6, 7, 15, 23, 24, 25, 31];
    const REF_POINTS: [(usize, [u32; 9]); 7] = [
        (
            1,
            [
                2_147_483_648,
                2_147_483_648,
                2_147_483_648,
                2_147_483_648,
                2_147_483_648,
                2_147_483_648,
                2_147_483_648,
                2_147_483_648,
                2_147_483_648,
            ],
        ),
        (
            2,
            [
                3_221_225_472,
                1_073_741_824,
                1_073_741_824,
                3_221_225_472,
                1_073_741_824,
                1_073_741_824,
                1_073_741_824,
                3_221_225_472,
                1_073_741_824,
            ],
        ),
        (
            3,
            [
                1_073_741_824,
                3_221_225_472,
                3_221_225_472,
                1_073_741_824,
                3_221_225_472,
                3_221_225_472,
                3_221_225_472,
                1_073_741_824,
                3_221_225_472,
            ],
        ),
        (
            8,
            [
                805_306_368,
                1_342_177_280,
                1_879_048_192,
                4_026_531_840,
                4_026_531_840,
                4_026_531_840,
                1_879_048_192,
                268_435_456,
                3_489_660_928,
            ],
        ),
        (
            16,
            [
                402_653_184,
                2_013_265_920,
                2_281_701_376,
                3_623_878_656,
                134_217_728,
                2_550_136_832,
                2_281_701_376,
                671_088_640,
                1_476_395_008,
            ],
        ),
        (
            64,
            [
                100_663_296,
                1_711_276_032,
                2_852_126_720,
                905_969_664,
                2_181_038_080,
                1_509_949_440,
                3_388_997_632,
                3_724_541_952,
                3_456_106_496,
            ],
        ),
        (
            256,
            [
                25_165_824,
                2_139_095_040,
                1_568_669_696,
                1_535_115_264,
                545_259_520,
                3_967_811_584,
                444_596_224,
                3_615_490_048,
                494_927_872,
            ],
        ),
    ];
    for (pt_one_based, expected_nums) in REF_POINTS {
        let row = &pts[pt_one_based - 1];
        for (&dim, &expected_num) in REF_DIMS.iter().zip(expected_nums.iter()) {
            let expected_u = (expected_num as f64) * (1.0 / 4_294_967_296.0);
            assert_eq!(
                row[dim].to_bits(),
                expected_u.to_bits(),
                "Joe-Kuo parity mismatch at point {pt_one_based}, dim {dim}: got {} vs {expected_u}",
                row[dim]
            );
        }
    }

    // 3. Previously correlated high dims (23, 24) and (24, 25) must stay decorrelated at N=256
    //    (was |r| ~ 0.74 before using verbatim `new-joe-kuo-6.21201`).
    for (da, db) in [(23usize, 24usize), (24usize, 25usize)] {
        let xa: Vec<f64> = pts[..256].iter().map(|p| p[da]).collect();
        let xb: Vec<f64> = pts[..256].iter().map(|p| p[db]).collect();
        let r = pearson_correlation(&xa, &xb).expect("Sobol projections have non-zero variance");
        assert!(
            r.abs() < 0.25,
            "F-064: pair ({da}, {db}) correlation at N=256 too high: |r|={} >= 0.25",
            r.abs()
        );
    }

    // 4. Exact 1D elementary-interval stratification over the first 255 non-zero points
    //    (Sobol indices 1..=255): every dimension splits [0, 1) into 16 bins with [15, 16, ..., 16].
    for d in 0..32 {
        let mut bins = [0usize; 16];
        for p in &pts[..255] {
            let gx = ((p[d] * 16.0).floor() as usize).min(15);
            bins[gx] += 1;
        }
        assert_eq!(
            bins[0], 15,
            "F-064: dim {d} bin 0 over 255 points expected 15, got {}",
            bins[0]
        );
        assert!(
            bins[1..].iter().all(|&c| c == 16),
            "F-064: dim {d} bins 1..16 over 255 points expected 16 each, got {:?}",
            &bins[1..]
        );
    }
}

#[test]
fn f065_sweeps_pearson_and_grid_values_are_scale_invariant() {
    // Part A: exact linear series y = 2x at 1e-9 scale yields r = +1.0.
    let x_small = [1e-9, 2e-9, 3e-9, 4e-9];
    let y_small = [2e-9, 4e-9, 6e-9, 8e-9];
    let r_small = pearson_correlation(&x_small, &y_small).expect("small-scale series correlates");
    assert!((r_small - 1.0).abs() < 1e-12, "got {r_small}");

    // Constant series must still return None even under floating-point mean rounding (e.g. 0.1).
    assert_eq!(
        pearson_correlation(&[0.1, 0.1, 0.1], &[1.0, 2.0, 3.0]),
        None
    );

    // Positive control: identical shape scaled to unit magnitude gives r = +1.0.
    let x_unit = [1.0, 2.0, 3.0, 4.0];
    let y_unit = [2.0, 4.0, 6.0, 8.0];
    let r_unit = pearson_correlation(&x_unit, &y_unit).expect("unit scale correlates");
    assert!((r_unit - 1.0).abs() < 1e-12);

    // Part B: Logarithmic { min: 1e-18, max: 1e-16 } and Continuous { min: 1e-18, max: 1e-16 }
    // preserve all 5 points.
    let tiny_log = DimensionSpec::Logarithmic {
        min: 1e-18,
        max: 1e-16,
    };
    assert!(tiny_log.validate("tiny_rate").is_ok());
    let log_vals = tiny_log.grid_values(5);
    assert_eq!(log_vals.len(), 5);
    assert!((log_vals[0] - 1e-18).abs() <= 1e-30);
    assert!((log_vals[4] - 1e-16).abs() <= 1e-28);

    let tiny_cont = DimensionSpec::Continuous {
        min: 1e-18,
        max: 1e-16,
    };
    assert_eq!(tiny_cont.grid_values(5).len(), 5);

    // Extreme ratio [1e-200, 1e200] avoids max/min overflow in Logarithmic::grid_values.
    let wide_log = DimensionSpec::Logarithmic {
        min: 1e-200,
        max: 1e200,
    };
    let wide_vals = wide_log.grid_values(5);
    assert_eq!(wide_vals.len(), 5);
    assert!(wide_vals.iter().all(|v| v.is_finite() && *v > 0.0));

    // Degenerate min == max still collapses cleanly to 1 point.
    let deg_log = DimensionSpec::Logarithmic {
        min: 1e-6,
        max: 1e-6,
    };
    assert_eq!(deg_log.grid_values(5), vec![1e-6]);

    // Positive control: normal-scale logarithmic grid preserves all 5 points.
    let mid_log = DimensionSpec::Logarithmic {
        min: 1e-6,
        max: 1e-4,
    };
    assert_eq!(mid_log.grid_values(5).len(), 5);

    // Extreme-ratio Logarithmic endpoints are pinned exactly and map_normalized/Sobol/LHS stay finite.
    assert_eq!(wide_vals[0], 1e-200);
    assert_eq!(wide_vals[4], 1e200);
    assert!(wide_vals.windows(2).all(|w| w[0] < w[1]));
    for u in [0.0, 0.1, 0.5, 0.999, 1.0] {
        let mapped = wide_log.map_normalized(u);
        assert!(
            mapped.is_finite() && (1e-200..=1e200).contains(&mapped),
            "wide_log.map_normalized({u}) produced {mapped}"
        );
    }

    // Wide symmetric f64 Continuous span [-1e308, 1e308] and pearson_correlation avoid overflow.
    let wide_cont = DimensionSpec::Continuous {
        min: -1e308,
        max: 1e308,
    };
    let cont_vals = wide_cont.grid_values(5);
    assert_eq!(cont_vals.len(), 5);
    assert!(cont_vals.iter().all(|v| v.is_finite()));
    assert!(cont_vals.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(wide_cont.map_normalized(0.5), 0.0);

    let r_sym = pearson_correlation(&[-1e308, 0.0, 1e308], &[-1e308, 0.0, 1e308])
        .expect("symmetric 1e308 span correlates");
    assert!((r_sym - 1.0).abs() < 1e-12, "got {r_sym}");

    let sweep_base = ref_base_manifest();
    for sampling in [
        SamplingStrategy::Sobol {
            num_samples: 16,
            offset: 0,
        },
        SamplingStrategy::LatinHypercube {
            num_samples: 16,
            seed: Some(HD_SEED),
            centered: false,
        },
    ] {
        let manifest = SweepManifest {
            schema_version: SWEEP_SCHEMA_VERSION,
            sweep_id: "f065-extreme-log-sweep".into(),
            name: "F-065 Extreme Log Sweep".into(),
            base_manifest: sweep_base.clone(),
            parameters: vec![
                ParameterDim {
                    name: "wide_log".into(),
                    target: SweepTarget::InitialCondition("plants".into()),
                    spec: wide_log.clone(),
                },
                ParameterDim {
                    name: "wide_cont".into(),
                    target: SweepTarget::InitialCondition("precip".into()),
                    spec: wide_cont.clone(),
                },
            ],
            sampling,
            max_threads: None,
        };
        let (_names, matrix) = manifest
            .generate_sample_matrix()
            .expect("Sobol/LHS matrix generates cleanly for extreme bounds");
        assert_eq!(matrix.len(), 16);
        for row in &matrix {
            assert!(row[0].is_finite() && (1e-200..=1e200).contains(&row[0]));
            assert!(row[1].is_finite() && (-1e308..=1e308).contains(&row[1]));
        }
    }
}

#[test]
fn f062_shorter_child_with_exotic_interventions_filters_future_ancestor_forcings_and_rejects_invalid_own(
) {
    let registry = ObservableRegistry::reference_default();
    let mut exotic_base = ref_base_manifest();
    exotic_base.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_uniform(50.0));
    exotic_base.observable_ids.push("exotic.sourced".into());

    let mk_exotic =
        |id: u32, start_tick: u64, duration_ticks: u64, amount: f32| ExoticIntervention {
            id,
            cause_id: 950 + id,
            kind: ExoticInterventionKind::AddSource,
            region: Region::Global,
            start_tick,
            duration_ticks,
            amount,
            curve: Curve::Step,
        };

    // Root (duration 200) schedules a future forcing at tick 150..200 (fires at tick 180);
    // Shorter Child (duration 100, fork 20) adds its own valid forcing at tick 30..70 (fires at tick 60).
    let f_root_future = mk_exotic(20, 150, 50, 0.1);
    let f_child_valid = mk_exotic(21, 30, 40, 0.2);

    let valid_shorter_tree = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "f062-shorter-child-exotic-ok".into(),
        name: "F-062 Shorter Child Exotic OK".into(),
        base_manifest: exotic_base.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 0,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 200,
                interventions: Vec::new(),
                exotic_interventions: vec![f_root_future.clone()],
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root-Long".into(),
            },
            BranchNodeSpecification {
                id: 1,
                parent_id: Some(0),
                fork_tick: 20,
                duration_ticks: 100,
                interventions: Vec::new(),
                exotic_interventions: vec![f_child_valid],
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Child-Short".into(),
            },
        ],
        comparisons: vec![],
    };

    let ok_report = run_branch_tree::<ReferenceEvolutionWorld>(&valid_shorter_tree, &registry)
        .expect("shorter child with valid local exotic forcing must succeed");
    let child_run = ok_report.nodes[&1].run_result.as_ref().unwrap();
    assert!(
        obs_val(&child_run.final_observables, "exotic.sourced") > 0.0,
        "child's own exotic forcing must be applied within ticks 30..=70"
    );

    // Control: child with an out-of-horizon own forcing (start_tick: 110 on duration_ticks: 100)
    // must still be strictly rejected.
    let f_child_ooh = mk_exotic(22, 110, 30, 0.2);
    let invalid_child_tree = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "f062-shorter-child-exotic-ooh".into(),
        name: "F-062 Shorter Child Exotic Out-Of-Horizon".into(),
        base_manifest: exotic_base,
        nodes: vec![
            BranchNodeSpecification {
                id: 0,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 200,
                interventions: Vec::new(),
                exotic_interventions: vec![f_root_future],
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root-Long".into(),
            },
            BranchNodeSpecification {
                id: 1,
                parent_id: Some(0),
                fork_tick: 20,
                duration_ticks: 100,
                interventions: Vec::new(),
                exotic_interventions: vec![f_child_ooh],
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Child-Invalid-OOH".into(),
            },
        ],
        comparisons: vec![],
    };

    let err = run_branch_tree::<ReferenceEvolutionWorld>(&invalid_child_tree, &registry)
        .expect_err("child with out-of-horizon own exotic forcing must be rejected");
    match err {
        ExperimentError::InvalidExoticIntervention { id, .. } => assert_eq!(id, 22),
        other => panic!("expected InvalidExoticIntervention for child id 22, got {other:?}"),
    }
}

#[test]
fn f062_three_level_shorter_child_to_longer_empty_grandchild_restores_ancestor_forcings() {
    use anima_engine_lib::core::experiment_runner::{run_manifest_seed, RunStatus};

    let registry = ObservableRegistry::reference_default();
    let mut exotic_base = ref_base_manifest();
    exotic_base.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_uniform(50.0));
    exotic_base.observable_ids.push("exotic.sourced".into());

    let mk_exotic =
        |id: u32, start_tick: u64, duration_ticks: u64, amount: f32| ExoticIntervention {
            id,
            cause_id: 960 + id,
            kind: ExoticInterventionKind::AddSource,
            region: Region::Global,
            start_tick,
            duration_ticks,
            amount,
            curve: Curve::Step,
        };

    // Root(dur=200, id:20@150..200) -> ShorterChild(dur=100, id:21@30..70) -> LongerGrandchild(dur=200, exotic_interventions=[]).
    // Child filters out id:20 for its own duration=100 run, so its checkpoint snapshot does not hold id:20.
    // LongerGrandchild must still restore id:20 from effective_exotic and match flat run_manifest_seed bit-for-bit.
    let f_root_future = mk_exotic(20, 150, 50, 0.25);
    let f_child_valid = mk_exotic(21, 30, 40, 0.20);

    let three_level_tree = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "f062-3lvl-empty-grandchild".into(),
        name: "F-062 3-Level Empty Grandchild".into(),
        base_manifest: exotic_base.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 0,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 200,
                interventions: Vec::new(),
                exotic_interventions: vec![f_root_future.clone()],
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root-Long".into(),
            },
            BranchNodeSpecification {
                id: 1,
                parent_id: Some(0),
                fork_tick: 20,
                duration_ticks: 100,
                interventions: Vec::new(),
                exotic_interventions: vec![f_child_valid.clone()],
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Shorter-Child".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 50,
                duration_ticks: 200,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Longer-Empty-Grandchild".into(),
            },
        ],
        comparisons: vec![],
    };

    let tree_report = run_branch_tree::<ReferenceEvolutionWorld>(&three_level_tree, &registry)
        .expect("3-level shorter-child to longer-empty-grandchild tree must succeed");
    let grand_run = tree_report.nodes[&2].run_result.as_ref().unwrap();

    let mut flat_manifest = exotic_base;
    flat_manifest.duration_ticks = 200;
    flat_manifest.exotic_interventions = vec![f_root_future, f_child_valid];
    let flat_run = run_manifest_seed::<ReferenceEvolutionWorld>(
        &flat_manifest,
        &registry,
        HD_SEED,
        None,
        None,
    );
    assert!(
        matches!(flat_run.status, RunStatus::Completed),
        "flat counterfactual reference run must complete: {:?}",
        flat_run.status
    );

    assert_eq!(
        grand_run.final_checksum, flat_run.final_checksum,
        "longer grandchild with empty exotic_interventions must restore Root's future forcing and match flat run bit-for-bit"
    );
    assert_eq!(
        obs_val(&grand_run.final_observables, "exotic.sourced").to_bits(),
        obs_val(&flat_run.final_observables, "exotic.sourced").to_bits(),
        "exotic.sourced must match flat counterfactual bit-for-bit"
    );
}
