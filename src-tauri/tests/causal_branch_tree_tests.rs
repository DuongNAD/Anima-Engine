//! Integration tests for N-ary Causal Branch Tree and Trajectory Divergence (Feature 6).

use anima_engine_lib::core::branch_tree::{
    dynamic_time_warping, run_branch_tree, BranchNodeSpecification,
    BranchSeedStrategy, BranchTreeManifest, BRANCH_TREE_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, ObserverPolicy, WorldIdentity,
    WorldLawSet, MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::intervention::{
    Curve, InterventionCommand, InterventionKind, Region, TraitMod,
};
use anima_engine_lib::core::reference_world::ReferenceEvolutionWorld;
use anima_engine_lib::core::sim_clock::ECOLOGY_PERIOD;

fn base_manifest() -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "branch_base_exp".into(),
        name: "Branch Base Experiment".into(),
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
        seeds: vec![101],
        duration_ticks: 1200,
        sample_period: ECOLOGY_PERIOD,
        observable_ids: vec!["plants".into(), "herbivores".into(), "predators".into()],
        exotic_interventions: Vec::new(),
    }
}

#[test]
fn dynamic_time_warping_identical_and_shifted_series() {
    let s1 = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let s2 = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    assert_eq!(dynamic_time_warping(&s1, &s2), 0.0);

    let s3 = vec![2.0, 3.0, 4.0, 5.0, 6.0];
    let dtw = dynamic_time_warping(&s1, &s3);
    assert!(dtw > 0.0);
}

#[test]
fn branch_tree_n_ary_execution_with_checkpoint_memoization() {
    let registry = ObservableRegistry::reference_default();
    let base = base_manifest();

    let manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "n_ary_branch_test".into(),
        name: "N-ary Branch Test".into(),
        base_manifest: base,
        nodes: vec![
            // Root branch: runs ticks 0..=1200
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
            // Child branch A: forks at tick 240 with Deforest intervention
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 240,
                duration_ticks: 1200,
                interventions: vec![InterventionCommand {
                    id: 101,
                    cause_id: 1001,
                    kind: InterventionKind::Deforest,
                    region: Region::Global,
                    start_tick: 240,
                    duration_ticks: 360,
                    intensity: 0.5,
                    signed_negative: true,
                    curve: Curve::Step,
                    reversible: false,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Deforestation Branch".into(),
            },
            // Child branch B: forks at tick 240 with Pathogen transmission
            BranchNodeSpecification {
                id: 3,
                parent_id: Some(1),
                fork_tick: 240,
                duration_ticks: 1200,
                interventions: vec![InterventionCommand {
                    id: 102,
                    cause_id: 1002,
                    kind: InterventionKind::PathogenTransmission {
                        pathogen_id: 1,
                        transmission_rate: 0.8,
                        mortality_multiplier: 1.5,
                    },
                    region: Region::Global,
                    start_tick: 240,
                    duration_ticks: 360,
                    intensity: 1.0,
                    signed_negative: true,
                    curve: Curve::Step,
                    reversible: false,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Pathogen Epidemic Branch".into(),
            },
            // Grandchild branch C: forks from Branch A at tick 600 with Genetic Injection
            BranchNodeSpecification {
                id: 4,
                parent_id: Some(2),
                fork_tick: 600,
                duration_ticks: 1200,
                interventions: vec![InterventionCommand {
                    id: 103,
                    cause_id: 1003,
                    kind: InterventionKind::GeneticInjection {
                        target_species: Some(1),
                        trait_modifications: vec![TraitMod::MetabolicEfficiencyDelta(0.2)],
                        count: 50,
                    },
                    region: Region::Global,
                    start_tick: 600,
                    duration_ticks: 240,
                    intensity: 1.0,
                    signed_negative: false,
                    curve: Curve::Step,
                    reversible: true,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::DerivedStream { stream_id: 42 },
                label: "Genetic Rescue Sub-branch".into(),
            },
        ],
        comparisons: vec![(1, 2), (1, 3), (2, 4)],
    };

    let report = run_branch_tree::<ReferenceEvolutionWorld>(&manifest, &registry)
        .expect("branch tree must execute successfully");

    assert_eq!(report.nodes.len(), 4);
    assert_eq!(report.divergences.len(), 3);

    // Verify root vs deforest divergence
    let div_1_2 = report.divergences.iter().find(|d| d.branch_a_id == 1 && d.branch_b_id == 2).unwrap();
    assert_eq!(div_1_2.common_start_tick, 240);
    assert_eq!(div_1_2.common_end_tick, 1200);
    assert!(div_1_2.phase_space.max_distance > 0.0);

    let plant_div = div_1_2.observables.iter().find(|o| o.observable_id == "plants").unwrap();
    assert!(plant_div.max_abs_delta > 0.0);

    // Verify serialization roundtrip
    let json = report.to_json().expect("report serializes to json");
    assert!(json.contains("n_ary_branch_test"));
    assert!(json.contains("Deforestation Branch"));
}

#[test]
fn branch_tree_topology_validation_rejects_cycles_and_invalid_fork_ticks() {
    let registry = ObservableRegistry::reference_default();
    let base = base_manifest();

    // Cyclic tree
    let cyclic_manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "cyclic_tree".into(),
        name: "Cyclic Tree".into(),
        base_manifest: base.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: Some(2), // Cycle: 1 -> 2 -> 1
                fork_tick: 0,
                duration_ticks: 100,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Node 1".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 20,
                duration_ticks: 100,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Node 2".into(),
            },
        ],
        comparisons: Vec::new(),
    };

    assert!(run_branch_tree::<ReferenceEvolutionWorld>(&cyclic_manifest, &registry).is_err());

    // Invalid fork tick beyond parent duration
    let bad_fork_manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "bad_fork_tree".into(),
        name: "Bad Fork Tree".into(),
        base_manifest: base,
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 50,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 60, // > parent duration (50)
                duration_ticks: 100,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Invalid Fork Child".into(),
            },
        ],
        comparisons: Vec::new(),
    };

    assert!(run_branch_tree::<ReferenceEvolutionWorld>(&bad_fork_manifest, &registry).is_err());
}
