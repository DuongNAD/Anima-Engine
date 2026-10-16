//! Adversarial Stress Tests for Feature 6 (N-ary Causal Branch Tree) and Feature 8 (Deterministic Replay & Parity).
//!
//! Evaluates:
//! 1. Deep branch DAGs (grandchild branches, multi-way forks at identical ticks, multi-interventions).
//! 2. Checkpoint memoization parity (memoized branch execution vs standalone freshly simulated runs).
//! 3. Trajectory divergence metrics (exact 0 for identical trajectories, strictly positive for interventions, correct time-to-divergence, DTW mathematical properties).
//! 4. State digest parity (`world_checksum`) across forks and seekable ChaCha12 stream restoration.
//! 5. Empirical exposure of bugs and boundary condition failures.

use anima_engine_lib::core::branch_tree::{
    dynamic_time_warping, run_branch_tree,
    BranchNodeSpecification, BranchSeedStrategy, BranchTreeManifest,
    BRANCH_TREE_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, ObserverPolicy, WorldIdentity,
    WorldLawSet, MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{run_manifest_seed, RunStatus};
use anima_engine_lib::core::intervention::{
    Curve, InterventionCommand, InterventionKind, Region, TraitMod,
};
use anima_engine_lib::core::reference_world::ReferenceEvolutionWorld;
use anima_engine_lib::core::resources::SimRng;
use anima_engine_lib::core::sim_clock::ECOLOGY_PERIOD;
use rand::RngCore;

fn standard_base_manifest(duration_ticks: u64) -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "adv_branch_base".into(),
        name: "Adversarial Branch Base Experiment".into(),
        observer: ObserverPolicy::default(),
        world_identity: WorldIdentity {
            seed: 424242,
            generator_version: 20,
            width: 128,
            height: 128,
            checksum: 0xCAFE_BABE,
        },
        laws: WorldLawSet::baseline(),
        initial_conditions: InitialConditionSet::new(vec![
            ("precip".into(), 1.2),
            ("temperature".into(), 0.6),
            ("plants".into(), 150.0),
            ("herbivores".into(), 50.0),
            ("predators".into(), 10.0),
            ("detritus".into(), 5.0),
        ]),
        interventions: Vec::new(),
        seeds: vec![777],
        duration_ticks,
        sample_period: ECOLOGY_PERIOD,
        observable_ids: vec![
            "plants".into(),
            "herbivores".into(),
            "predators".into(),
            "detritus".into(),
        ],
        exotic_interventions: Vec::new(),
    }
}

// =============================================================================
// TEST SUITE 1: Deep Branch DAGs & Complex Topologies
// =============================================================================

#[test]
fn adversarial_deep_dag_grandchild_multiway_forks_and_interventions() {
    let registry = ObservableRegistry::reference_default();
    let base = standard_base_manifest(1200);

    // Build a 4-tier deep DAG with 7 branches:
    // Root (0..1200)
    //  ├── Child A (fork 240..1200) [Deforest]
    //  │    ├── Grandchild A1 (fork 480..1200) [Pathogen]
    //  │    │    └── Great-Grandchild A1a (fork 720..1200) [GeneticInjection + ToxicityPlume]
    //  │    └── Grandchild A2 (fork 480..1200) [Simultaneous Multi-Intervention: Deforest + RemovePredators]
    //  ├── Child B (fork 240..1200) [ToxicityPlume, DerivedStream]
    //  └── Child C (fork 240..1200) [Control, InheritStream]
    let manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "deep_dag_adv_test".into(),
        name: "Deep DAG Adversarial Stress Test".into(),
        base_manifest: base,
        nodes: vec![
            // Node 1: Root Control (0..1200)
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 1200,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root Control".into(),
            },
            // Node 2: Child A (fork 240..1200)
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 240,
                duration_ticks: 1200,
                interventions: vec![InterventionCommand {
                    id: 201,
                    cause_id: 2001,
                    kind: InterventionKind::Deforest,
                    region: Region::Global,
                    start_tick: 241,
                    duration_ticks: 360,
                    intensity: 0.6,
                    signed_negative: true,
                    curve: Curve::RampUp,
                    reversible: false,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Child A (Deforest)".into(),
            },
            // Node 3: Grandchild A1 (fork 480..1200 from Node 2)
            BranchNodeSpecification {
                id: 3,
                parent_id: Some(2),
                fork_tick: 480,
                duration_ticks: 1200,
                interventions: vec![InterventionCommand {
                    id: 301,
                    cause_id: 3001,
                    kind: InterventionKind::PathogenTransmission {
                        pathogen_id: 99,
                        transmission_rate: 0.75,
                        mortality_multiplier: 1.8,
                    },
                    region: Region::Global,
                    start_tick: 481,
                    duration_ticks: 240,
                    intensity: 0.9,
                    signed_negative: true,
                    curve: Curve::Step,
                    reversible: false,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Grandchild A1 (Pathogen on Deforest)".into(),
            },
            // Node 4: Great-Grandchild A1a (fork 720..1200 from Node 3)
            BranchNodeSpecification {
                id: 4,
                parent_id: Some(3),
                fork_tick: 720,
                duration_ticks: 1200,
                interventions: vec![
                    InterventionCommand {
                        id: 401,
                        cause_id: 4001,
                        kind: InterventionKind::GeneticInjection {
                            target_species: Some(1),
                            trait_modifications: vec![
                                TraitMod::MetabolicEfficiencyDelta(0.25),
                                TraitMod::ScaleMorphology(1.1),
                            ],
                            count: 40,
                        },
                        region: Region::Global,
                        start_tick: 721,
                        duration_ticks: 240,
                        intensity: 1.0,
                        signed_negative: false,
                        curve: Curve::Step,
                        reversible: true,
                    },
                    InterventionCommand {
                        id: 402,
                        cause_id: 4002,
                        kind: InterventionKind::ToxicityPlume {
                            center_x: 64.0,
                            center_y: 64.0,
                            radius: 32.0,
                            toxicity_level: 0.7,
                            decay_rate: 0.02,
                        },
                        region: Region::Global,
                        start_tick: 721,
                        duration_ticks: 360,
                        intensity: 0.8,
                        signed_negative: true,
                        curve: Curve::Triangle,
                        reversible: false,
                    },
                ],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::DerivedStream { stream_id: 104 },
                label: "Great-Grandchild A1a (Genetic + Toxicity)".into(),
            },
            // Node 5: Grandchild A2 (fork 480..1200 from Node 2) - Multiple simultaneous interventions
            BranchNodeSpecification {
                id: 5,
                parent_id: Some(2),
                fork_tick: 480,
                duration_ticks: 1200,
                interventions: vec![
                    InterventionCommand {
                        id: 501,
                        cause_id: 5001,
                        kind: InterventionKind::Deforest,
                        region: Region::Global,
                        start_tick: 481,
                        duration_ticks: 240,
                        intensity: 0.4,
                        signed_negative: true,
                        curve: Curve::RampDown,
                        reversible: false,
                    },
                    InterventionCommand {
                        id: 502,
                        cause_id: 5002,
                        kind: InterventionKind::RemovePredators,
                        region: Region::Global,
                        start_tick: 481,
                        duration_ticks: 240,
                        intensity: 0.3,
                        signed_negative: true,
                        curve: Curve::Step,
                        reversible: false,
                    },
                ],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Grandchild A2 (Simultaneous Deforest + RemovePredators)".into(),
            },
            // Node 6: Child B (fork 240..1200 from Node 1) - Multi-way fork at tick 240
            BranchNodeSpecification {
                id: 6,
                parent_id: Some(1),
                fork_tick: 240,
                duration_ticks: 1200,
                interventions: vec![InterventionCommand {
                    id: 601,
                    cause_id: 6001,
                    kind: InterventionKind::ToxicityPlume {
                        center_x: 10.0,
                        center_y: 10.0,
                        radius: 20.0,
                        toxicity_level: 0.5,
                        decay_rate: 0.01,
                    },
                    region: Region::Global,
                    start_tick: 241,
                    duration_ticks: 480,
                    intensity: 1.0,
                    signed_negative: true,
                    curve: Curve::Step,
                    reversible: false,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::DerivedStream { stream_id: 999 },
                label: "Child B (ToxicityPlume at 240)".into(),
            },
            // Node 7: Child C (fork 240..1200 from Node 1) - Multi-way fork Control at tick 240
            BranchNodeSpecification {
                id: 7,
                parent_id: Some(1),
                fork_tick: 240,
                duration_ticks: 1200,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Child C (Control Fork at 240)".into(),
            },
        ],
        comparisons: vec![
            (1, 7), // Root vs Control Child C (should be exact 0 divergence)
            (1, 2), // Root vs Child A
            (2, 3), // Child A vs Grandchild A1
            (3, 4), // Grandchild A1 vs Great-Grandchild A1a
            (2, 5), // Child A vs Grandchild A2
            (1, 6), // Root vs Child B
            (2, 6), // Child A vs Child B (multi-way fork comparison)
        ],
    };

    let report = run_branch_tree::<ReferenceEvolutionWorld>(&manifest, &registry)
        .expect("deep DAG branch tree must execute successfully");

    assert_eq!(report.nodes.len(), 7, "All 7 nodes must be completed");
    assert_eq!(report.divergences.len(), 7, "All 7 comparisons must be computed");

    for (id, node) in &report.nodes {
        assert_eq!(node.status, anima_engine_lib::core::branch_tree::BranchExecutionStatus::Completed);
        let res = node.run_result.as_ref().expect("run_result must exist");
        assert_eq!(res.status, RunStatus::Completed);
        assert!(!res.series.is_empty(), "Series for node {} must not be empty", id);
    }

    // Verify Child C (Node 7) vs Root (Node 1) is exact zero divergence
    let div_root_control = report.divergences.iter().find(|d| (d.branch_a_id == 1 && d.branch_b_id == 7) || (d.branch_a_id == 7 && d.branch_b_id == 1)).unwrap();
    assert_eq!(div_root_control.phase_space.max_distance, 0.0);
    assert_eq!(div_root_control.phase_space.mean_distance, 0.0);
    assert_eq!(div_root_control.time_to_divergence, None);
}

#[test]
fn adversarial_wide_20_node_tree_execution() {
    let registry = ObservableRegistry::reference_default();
    let base = standard_base_manifest(600);

    let mut nodes = Vec::new();
    nodes.push(BranchNodeSpecification {
        id: 1,
        parent_id: None,
        fork_tick: 0,
        duration_ticks: 600,
        interventions: Vec::new(),
        exotic_interventions: Vec::new(),
        seed_strategy: BranchSeedStrategy::InheritStream,
        label: "Root Node".into(),
    });

    let mut comparisons = Vec::new();
    nodes.push(BranchNodeSpecification {
        id: 2,
        parent_id: Some(1),
        fork_tick: 120,
        duration_ticks: 600,
        interventions: Vec::new(),
        exotic_interventions: Vec::new(),
        seed_strategy: BranchSeedStrategy::InheritStream,
        label: "Branch Node 2".into(),
    });
    comparisons.push((1, 2));

    // Add 18 more children forking from either 1 or 2 at strictly later ticks
    for i in 3..=20 {
        let (parent_id, min_fork) = if i > 12 { (2, 120) } else { (1, 0) };
        let fork_tick = min_fork + match i % 4 {
            0 => 60,
            1 => 120,
            2 => 180,
            _ => 240,
        };
        // Ensure fork_tick > parent fork
        let effective_fork = if parent_id == 2 && fork_tick <= 120 { 240 } else { fork_tick };

        nodes.push(BranchNodeSpecification {
            id: i,
            parent_id: Some(parent_id),
            fork_tick: effective_fork,
            duration_ticks: 600,
            interventions: vec![InterventionCommand {
                id: (i * 10) as u32,
                cause_id: (i * 100) as u32,
                kind: InterventionKind::RainfallDelta,
                region: Region::Global,
                start_tick: effective_fork + 1,
                duration_ticks: 100,
                intensity: 0.1 * (i as f32 % 5.0 + 1.0),
                signed_negative: i % 2 == 0,
                curve: Curve::Step,
                reversible: true,
            }],
            exotic_interventions: Vec::new(),
            seed_strategy: if i % 3 == 0 {
                BranchSeedStrategy::DerivedStream { stream_id: i }
            } else {
                BranchSeedStrategy::InheritStream
            },
            label: format!("Branch Node {}", i),
        });

        comparisons.push((1, i));
    }

    let manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "wide_20_node_tree".into(),
        name: "Wide 20-Node Branch Tree".into(),
        base_manifest: base,
        nodes,
        comparisons,
    };

    let report = run_branch_tree::<ReferenceEvolutionWorld>(&manifest, &registry)
        .expect("20-node branch tree must execute successfully");

    assert_eq!(report.nodes.len(), 20);
    assert_eq!(report.divergences.len(), 19);
}

// =============================================================================
// TEST SUITE 2: Checkpoint Memoization Parity vs Standalone Scratch Run
// =============================================================================

#[test]
fn adversarial_checkpoint_memoization_parity_vs_standalone_scratch_run() {
    let registry = ObservableRegistry::reference_default();
    let base = standard_base_manifest(800);
    let seed = 777;

    let deforest_intervention_241 = InterventionCommand {
        id: 555,
        cause_id: 5555,
        kind: InterventionKind::Deforest,
        region: Region::Global,
        start_tick: 241,
        duration_ticks: 360,
        intensity: 0.7,
        signed_negative: true,
        curve: Curve::Step,
        reversible: false,
    };

    let branch_manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "parity_memoized_tree".into(),
        name: "Memoization Parity Tree".into(),
        base_manifest: base.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 800,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 240,
                duration_ticks: 800,
                interventions: vec![deforest_intervention_241.clone()],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Child Deforest".into(),
            },
        ],
        comparisons: vec![(1, 2)],
    };

    let tree_report = run_branch_tree::<ReferenceEvolutionWorld>(&branch_manifest, &registry)
        .expect("branch tree should execute");
    let memoized_child_run = tree_report.nodes.get(&2).unwrap().run_result.as_ref().unwrap();

    let mut standalone_manifest = base.clone();
    standalone_manifest.interventions = vec![deforest_intervention_241];
    standalone_manifest.duration_ticks = 800;

    let standalone_run = run_manifest_seed::<ReferenceEvolutionWorld>(&standalone_manifest, &registry, seed, None, None);

    assert_eq!(memoized_child_run.status, RunStatus::Completed);
    assert_eq!(standalone_run.status, RunStatus::Completed);
    assert_eq!(
        memoized_child_run.final_checksum, standalone_run.final_checksum,
        "Final checksum of memoized branch MUST be bit-exact identical to standalone run"
    );

    let memoized_samples_map: std::collections::HashMap<u64, &anima_engine_lib::core::scenario::StateSample> =
        memoized_child_run.series.iter().map(|s| (s.tick, s)).collect();

    for standalone_sample in &standalone_run.series {
        if standalone_sample.tick >= 240 {
            let memo_sample = memoized_samples_map.get(&standalone_sample.tick)
                .unwrap_or_else(|| panic!("Sample at tick {} must exist in memoized run", standalone_sample.tick));
            assert_eq!(
                memo_sample.observables, standalone_sample.observables,
                "Trajectory observables at tick {} must be bit-exact identical", standalone_sample.tick
            );
        }
    }
}

// =============================================================================
// TEST SUITE 3: Trajectory Divergence Metrics & Mathematical Properties
// =============================================================================

#[test]
fn adversarial_divergence_metrics_exact_zero_and_strictly_positive() {
    let registry = ObservableRegistry::reference_default();
    let base = standard_base_manifest(600);

    let manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "div_metrics_tree".into(),
        name: "Divergence Metrics Test".into(),
        base_manifest: base,
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 600,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root Control".into(),
            },
            // Identical Child (Control continuation)
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 120,
                duration_ticks: 600,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Identical Child".into(),
            },
            // Active Intervention Child
            BranchNodeSpecification {
                id: 3,
                parent_id: Some(1),
                fork_tick: 120,
                duration_ticks: 600,
                interventions: vec![InterventionCommand {
                    id: 301,
                    cause_id: 3001,
                    kind: InterventionKind::Deforest,
                    region: Region::Global,
                    start_tick: 121,
                    duration_ticks: 240,
                    intensity: 0.8,
                    signed_negative: true,
                    curve: Curve::Step,
                    reversible: false,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Deforest Intervention Child".into(),
            },
        ],
        comparisons: vec![(1, 2), (1, 3)],
    };

    let report = run_branch_tree::<ReferenceEvolutionWorld>(&manifest, &registry).unwrap();

    // 1. Check identical comparison (1 vs 2)
    let div_1_2 = report.divergences.iter().find(|d| d.branch_a_id == 1 && d.branch_b_id == 2).unwrap();
    assert_eq!(div_1_2.phase_space.mean_distance, 0.0, "Identical runs must have mean_distance = 0");
    assert_eq!(div_1_2.phase_space.max_distance, 0.0, "Identical runs must have max_distance = 0");
    assert_eq!(div_1_2.phase_space.final_distance, 0.0, "Identical runs must have final_distance = 0");
    assert_eq!(div_1_2.time_to_divergence, None, "Identical runs must have time_to_divergence = None");
    for obs in &div_1_2.observables {
        assert_eq!(obs.mean_delta, 0.0, "Observable {} mean_delta must be 0", obs.observable_id);
        assert_eq!(obs.max_abs_delta, 0.0, "Observable {} max_abs_delta must be 0", obs.observable_id);
        assert_eq!(obs.rmsd, 0.0, "Observable {} rmsd must be 0", obs.observable_id);
        assert_eq!(obs.relative_divergence, 0.0, "Observable {} relative_divergence must be 0", obs.observable_id);
        if let Some(dtw) = obs.dtw_distance {
            assert_eq!(dtw, 0.0, "Observable {} DTW distance must be 0", obs.observable_id);
        }
    }

    // 2. Check active intervention comparison (1 vs 3)
    let div_1_3 = report.divergences.iter().find(|d| d.branch_a_id == 1 && d.branch_b_id == 3).unwrap();
    assert!(div_1_3.phase_space.max_distance > 0.0, "Intervention must produce strictly positive max_distance");
    assert!(div_1_3.phase_space.mean_distance > 0.0, "Intervention must produce strictly positive mean_distance");
    assert!(div_1_3.time_to_divergence.is_some(), "Intervention must trigger time_to_divergence");
    let ttd = div_1_3.time_to_divergence.unwrap();
    assert!(ttd >= 120, "time_to_divergence {} must be >= fork_tick 120", ttd);

    let plant_obs = div_1_3.observables.iter().find(|o| o.observable_id == "plants").unwrap();
    assert!(plant_obs.max_abs_delta > 10.0, "Plants must be heavily impacted by deforest");
    assert!(plant_obs.rmsd > 0.0);
    assert!(plant_obs.relative_divergence > 0.0);
    assert!(plant_obs.dtw_distance.unwrap_or(0.0) > 0.0);
}

#[test]
fn adversarial_dynamic_time_warping_axioms() {
    let s1 = vec![10.0, 15.0, 20.0, 25.0, 30.0, 35.0];
    let s2 = vec![12.0, 14.0, 22.0, 24.0, 28.0, 38.0];
    let s3 = vec![100.0, 200.0, 300.0, 400.0, 500.0, 600.0];

    // 1. Identity: DTW(A, A) == 0
    assert_eq!(dynamic_time_warping(&s1, &s1), 0.0);
    assert_eq!(dynamic_time_warping(&s2, &s2), 0.0);

    // 2. Non-negativity: DTW(A, B) >= 0
    let d12 = dynamic_time_warping(&s1, &s2);
    let d13 = dynamic_time_warping(&s1, &s3);
    assert!(d12 > 0.0);
    assert!(d13 > 0.0);

    // 3. Symmetry: DTW(A, B) == DTW(B, A)
    let d21 = dynamic_time_warping(&s2, &s1);
    assert!((d12 - d21).abs() < 1e-12, "DTW must be symmetric");

    // 4. Sensitivity: Disrupted series s3 has strictly greater distance than s2
    assert!(d13 > d12);

    // 5. Empty inputs
    assert_eq!(dynamic_time_warping(&[], &s1), 0.0);
    assert_eq!(dynamic_time_warping(&s1, &[]), 0.0);
    assert_eq!(dynamic_time_warping(&[], &[]), 0.0);
}

// =============================================================================
// TEST SUITE 4: State Digest Parity & ChaCha12 Stream Restoration
// =============================================================================

#[test]
fn adversarial_chacha12_stream_pos_seekable_restoration_parity() {
    let seed = 0x1234_5678_9ABC_DEF0;
    let mut rng = SimRng::from_seed(seed);

    // Draw some random numbers to advance stream position
    for _ in 0..100 {
        let _ = rng.rng().next_u64();
    }
    let saved_pos = rng.stream_pos();
    assert!(saved_pos > 0, "Stream position must advance after draws");

    // Generate expected next 50 u64 numbers
    let mut expected_draws = Vec::with_capacity(50);
    for _ in 0..50 {
        expected_draws.push(rng.rng().next_u64());
    }

    // Now restore a new SimRng directly using seed and saved_pos (O(1) seek)
    let mut restored_rng = SimRng::restore(seed, saved_pos);
    assert_eq!(restored_rng.stream_pos(), saved_pos);

    let mut actual_draws = Vec::with_capacity(50);
    for _ in 0..50 {
        actual_draws.push(restored_rng.rng().next_u64());
    }

    assert_eq!(
        expected_draws, actual_draws,
        "Restored SimRng must produce bit-exact identical random draw sequence as uninterrupted stream"
    );
}

#[test]
fn adversarial_branch_tree_determinism_and_state_digest_checksum_repeatability() {
    let registry = ObservableRegistry::reference_default();
    let base = standard_base_manifest(600);

    let manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "digest_parity_tree".into(),
        name: "Digest Parity Repeatability Test".into(),
        base_manifest: base,
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 600,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 240,
                duration_ticks: 600,
                interventions: vec![InterventionCommand {
                    id: 201,
                    cause_id: 2001,
                    kind: InterventionKind::PathogenTransmission {
                        pathogen_id: 42,
                        transmission_rate: 0.6,
                        mortality_multiplier: 1.4,
                    },
                    region: Region::Global,
                    start_tick: 241,
                    duration_ticks: 240,
                    intensity: 1.0,
                    signed_negative: true,
                    curve: Curve::Step,
                    reversible: false,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Child Inherit".into(),
            },
            BranchNodeSpecification {
                id: 3,
                parent_id: Some(1),
                fork_tick: 240,
                duration_ticks: 600,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::DerivedStream { stream_id: 888 },
                label: "Child Derived Stream".into(),
            },
            BranchNodeSpecification {
                id: 4,
                parent_id: Some(1),
                fork_tick: 240,
                duration_ticks: 600,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::ExplicitSeed { seed: 99999 },
                label: "Child Explicit Seed".into(),
            },
        ],
        comparisons: vec![(1, 2), (1, 3), (1, 4)],
    };

    // Run 1
    let rep1 = run_branch_tree::<ReferenceEvolutionWorld>(&manifest, &registry).unwrap();
    // Run 2
    let rep2 = run_branch_tree::<ReferenceEvolutionWorld>(&manifest, &registry).unwrap();

    // Check that every node's final checksum and series are 100% bit-exact across independent runs
    for &id in &[1, 2, 3, 4] {
        let n1 = rep1.nodes.get(&id).unwrap();
        let n2 = rep2.nodes.get(&id).unwrap();
        let r1 = n1.run_result.as_ref().unwrap();
        let r2 = n2.run_result.as_ref().unwrap();

        assert_eq!(
            r1.final_checksum, r2.final_checksum,
            "Node {} final checksum must be bit-exact across runs", id
        );
        assert_eq!(r1.series.len(), r2.series.len());
        for (s1, s2) in r1.series.iter().zip(r2.series.iter()) {
            assert_eq!(s1.tick, s2.tick);
            assert_eq!(s1.observables, s2.observables);
        }
    }

    let r3 = rep1.nodes.get(&3).unwrap().run_result.as_ref().unwrap();
    let r4 = rep1.nodes.get(&4).unwrap().run_result.as_ref().unwrap();

    assert_eq!(r3.status, RunStatus::Completed);
    assert_eq!(r4.status, RunStatus::Completed);
}

// =============================================================================
// TEST SUITE 5: Empirical Discovery of Bugs & Boundary Defects
// =============================================================================

/// Tests genesis fork (`fork_tick == 0`) memoization and execution parity.
#[test]
fn adversarial_genesis_fork_tick_0_executes_successfully() {
    let registry = ObservableRegistry::reference_default();
    let base = standard_base_manifest(100);

    let manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "genesis_fork_0".into(),
        name: "Genesis Fork at 0 Parity Test".into(),
        base_manifest: base,
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 100,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 0,
                duration_ticks: 100,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Child at Genesis 0".into(),
            },
        ],
        comparisons: vec![(1, 2)],
    };

    assert!(manifest.validate(&registry).is_ok());
    let result = run_branch_tree::<ReferenceEvolutionWorld>(&manifest, &registry);
    assert!(result.is_ok(), "Genesis fork at tick 0 must execute successfully");
    let report = result.unwrap();
    let r1 = report.nodes.get(&1).unwrap().run_result.as_ref().unwrap();
    let r2 = report.nodes.get(&2).unwrap().run_result.as_ref().unwrap();
    assert_eq!(r1.status, RunStatus::Completed);
    assert_eq!(r2.status, RunStatus::Completed);
    assert_eq!(r1.final_checksum, r2.final_checksum, "Unmodified child at tick 0 must match root exactly");
}

/// Tests co-temporal fork (`fork_tick == parent.fork_tick`) memoization and execution parity.
#[test]
fn adversarial_cotemporal_fork_at_parent_fork_tick_executes_successfully() {
    let registry = ObservableRegistry::reference_default();
    let base = standard_base_manifest(600);

    // Node 1 (Root): 0..600
    // Node 2 (Child A): forks from Node 1 at tick 240..600
    // Node 3 (Child B): forks from Node 2 at tick 240..600 (counterfactual variant of Branch A at same fork tick)
    let manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "cotemporal_fork".into(),
        name: "Co-temporal Fork Parity Test".into(),
        base_manifest: base,
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 600,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 240,
                duration_ticks: 600,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Child A (fork 240)".into(),
            },
            BranchNodeSpecification {
                id: 3,
                parent_id: Some(2),
                fork_tick: 240,
                duration_ticks: 600,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::DerivedStream { stream_id: 77 },
                label: "Grandchild B (fork 240 from Child A)".into(),
            },
        ],
        comparisons: vec![(1, 2), (2, 3)],
    };

    assert!(manifest.validate(&registry).is_ok(), "Manifest validation permits fork_tick == parent.fork_tick");
    let result = run_branch_tree::<ReferenceEvolutionWorld>(&manifest, &registry);
    assert!(result.is_ok(), "Co-temporal fork at parent fork tick 240 must execute successfully");
    let report = result.unwrap();
    let r1 = report.nodes.get(&1).unwrap().run_result.as_ref().unwrap();
    let r2 = report.nodes.get(&2).unwrap().run_result.as_ref().unwrap();
    let r3 = report.nodes.get(&3).unwrap().run_result.as_ref().unwrap();
    assert_eq!(r1.status, RunStatus::Completed);
    assert_eq!(r2.status, RunStatus::Completed);
    assert_eq!(r3.status, RunStatus::Completed);
}

/// Tests intervention scheduling applicability enforcement and parity.
#[test]
fn adversarial_fork_tick_intervention_applicability_and_parity() {
    let registry = ObservableRegistry::reference_default();
    let base = standard_base_manifest(600);
    let seed = 777;

    // Single-tick impulse intervention at tick 240 (which has no active ticks in post-fork window (240, 600])
    let impulse_240 = InterventionCommand {
        id: 999,
        cause_id: 9999,
        kind: InterventionKind::Deforest,
        region: Region::Global,
        start_tick: 240,
        duration_ticks: 1, // Only active at tick 240
        intensity: 0.9,
        signed_negative: true,
        curve: Curve::Step,
        reversible: false,
    };

    let invalid_branch_manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "impulse_at_fork_tick_invalid".into(),
        name: "Impulse At Fork Tick Invalid Test".into(),
        base_manifest: base.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 600,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 240,
                duration_ticks: 600,
                interventions: vec![impulse_240],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Child Impulse at 240".into(),
            },
        ],
        comparisons: vec![(1, 2)],
    };

    // Manifest validation must reject this inapplicable intervention
    assert!(invalid_branch_manifest.validate(&registry).is_err(), "Inapplicable impulse intervention before branch execution window must be rejected");

    // Scheduling the impulse at tick 241 (the first simulated tick post-fork) is valid and produces bit-exact parity
    let impulse_241 = InterventionCommand {
        id: 1000,
        cause_id: 9999,
        kind: InterventionKind::Deforest,
        region: Region::Global,
        start_tick: 241,
        duration_ticks: 1,
        intensity: 0.9,
        signed_negative: true,
        curve: Curve::Step,
        reversible: false,
    };

    let valid_branch_manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "impulse_at_fork_tick_valid".into(),
        name: "Impulse At Fork Tick Valid Test".into(),
        base_manifest: base.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 600,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 240,
                duration_ticks: 600,
                interventions: vec![impulse_241.clone()],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Child Impulse at 241".into(),
            },
        ],
        comparisons: vec![(1, 2)],
    };

    assert!(valid_branch_manifest.validate(&registry).is_ok());
    let tree_report = run_branch_tree::<ReferenceEvolutionWorld>(&valid_branch_manifest, &registry)
        .expect("Valid branch tree must execute");
    let child_node = tree_report.nodes.get(&2).unwrap().run_result.as_ref().unwrap();

    let mut standalone_manifest = base;
    standalone_manifest.interventions = vec![impulse_241];
    standalone_manifest.duration_ticks = 600;
    let standalone_run = run_manifest_seed::<ReferenceEvolutionWorld>(&standalone_manifest, &registry, seed, None, None);

    assert_eq!(
        child_node.final_checksum, standalone_run.final_checksum,
        "Child branch with impulse at tick 241 must have bit-exact checksum parity with standalone run"
    );
}
