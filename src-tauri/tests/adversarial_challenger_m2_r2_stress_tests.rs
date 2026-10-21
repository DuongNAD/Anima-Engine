//! Empirical Adversarial Challenger Test Suite for Milestone 2 Iteration 2.
//!
//! Evaluates and stress-tests:
//! 1. Defect 1: Live Simulation Checkpoint Parity
//!    - Multi-stage save/load disk roundtrip and uninterrupted replay parity.
//!    - Checkpoint fork control vs uninterrupted live run state checksum equality.
//!    - Multi-band ecology interval replay parity.
//! 2. Defect 2: Genesis Forks, Co-Temporal Checkpoints, and Fork-Tick Interventions
//!    - Deep multi-tier genesis chains (Root -> Gen1 at 0 -> Gen2 at 0 -> Gen3 at 0).
//!    - Massive co-temporal branch DAGs (sibling and descendant branches forking at identical ticks).
//!    - Strict boundary enforcement on fork-tick intervention windows and applicability.
//!    - 50-node wide/deep DAG stress trees without panic or divergence.
//!    - Exact zero trajectory divergence on control branches and positive divergence on active interventions.

use anima_engine_lib::core::branch_tree::{
    run_branch_tree, BranchNodeSpecification, BranchSeedStrategy, BranchTreeManifest,
    BRANCH_TREE_SCHEMA_VERSION,
};
use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, ObserverPolicy,
    WorldIdentity, WorldLawSet, MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{
    checkpoint_fork, run_manifest_seed, ExperimentModel, RunStatus,
};
use anima_engine_lib::core::intervention::{
    Curve, InterventionCommand, InterventionKind, InterventionQueue, Region,
};
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LiveSnapshot, LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::reference_world::ReferenceEvolutionWorld;
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::core::snapshot::{self, SnapshotEnvelope};
use rand::SeedableRng;

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn reference_registry() -> ObservableRegistry {
    ObservableRegistry::reference_default()
}

fn live_registry() -> ObservableRegistry {
    ObservableRegistry::live_default()
}

fn reference_base_manifest(duration_ticks: u64, seed: u64) -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "adv_m2_r2_ref_base".into(),
        name: "Adversarial Ref Base Manifest".into(),
        observer: ObserverPolicy::default(),
        world_identity: WorldIdentity {
            seed: seed as u32,
            generator_version: 1,
            width: 64,
            height: 64,
            checksum: 0xBEEF_CAFE,
        },
        laws: WorldLawSet::baseline(),
        initial_conditions: InitialConditionSet::new(vec![
            ("precip".into(), 1.0),
            ("temperature".into(), 0.5),
            ("plants".into(), 100.0),
            ("herbivores".into(), 30.0),
            ("predators".into(), 5.0),
            ("detritus".into(), 10.0),
        ]),
        interventions: Vec::new(),
        seeds: vec![seed],
        duration_ticks,
        sample_period: 30,
        observable_ids: vec![
            "plants".into(),
            "herbivores".into(),
            "predators".into(),
            "detritus".into(),
        ],
        exotic_interventions: Vec::new(),
    }
}

fn live_base_manifest(duration_ticks: u64, seed: u64) -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "adv_m2_r2_live_base".into(),
        name: "Adversarial Live Base Manifest".into(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: InitialConditionSet::new(vec![
            ("live.founders".to_string(), 6.0),
            ("live.predator_fraction".to_string(), 0.5),
            ("live.trees".to_string(), 4.0),
            ("live.lakes".to_string(), 2.0),
            ("live.food_cap".to_string(), 50.0),
        ]),
        interventions: Vec::new(),
        seeds: vec![seed],
        duration_ticks,
        sample_period: 60,
        observable_ids: LIVE_OBSERVABLE_IDS.iter().map(|s| s.to_string()).collect(),
        exotic_interventions: Vec::new(),
        observer: Default::default(),
    }
}

fn through_disk_temp(snapshot: &LiveSnapshot, label: &str) -> LiveSnapshot {
    static DISK_CTR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(5000);
    let id = DISK_CTR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("anima_challenger_r2_{}_{}_{}", std::process::id(), id, label));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("checkpoint.json");
    let envelope = SnapshotEnvelope::seal(snapshot.saved.clone()).expect("seal");
    snapshot::write_atomic(&path, &envelope).expect("write");
    let back = snapshot::read(&path).expect("read");
    let _ = std::fs::remove_dir_all(&dir);
    LiveSnapshot {
        saved: back,
        config: snapshot.config,
        seed: snapshot.seed,
    }
}

// =============================================================================
// SUITE 1: DEFECT 1 - Live Simulation Checkpoint Parity & Fork Stress
// =============================================================================

/// Stress-test: Save/load live simulation state across ecology boundaries (K = 60).
/// Verifies bit-exact checksum equality between resumed and uninterrupted runs.
#[test]
fn stress_test_live_sim_checkpoint_disk_roundtrip_parity() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let seed = 4242;
    let duration = 180;
    let laws = WorldLawSet::baseline();
    let initial = InitialConditionSet::new(vec![
        ("live.founders".to_string(), 6.0),
        ("live.predator_fraction".to_string(), 0.5),
        ("live.trees".to_string(), 4.0),
        ("live.lakes".to_string(), 2.0),
        ("live.food_cap".to_string(), 50.0),
    ]);
    let queue = InterventionQueue::new(vec![]);

    // 1. Uninterrupted reference execution
    let mut uninterrupted = LiveExperimentAdapter::from_manifest(&laws, &initial, &[], seed, (16, 16), duration)
        .expect("build uninterrupted");
    let mut rng_ref = rand::rngs::StdRng::seed_from_u64(seed);
    let mut clock_ref = SimClock::new();
    let mut ledger_ref = CausalLedger::new();

    for _ in 0..duration {
        let tick = clock_ref.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        uninterrupted.step(&clock_ref, &active, &mut ledger_ref, &mut rng_ref);
        let _ = uninterrupted.observables();
    }
    let expected_checksum = uninterrupted.checksum();
    let expected_obs = uninterrupted.observables();

    // 2. Interrupted execution at K = 60
    let k = 60;
    let mut interrupted = LiveExperimentAdapter::from_manifest(&laws, &initial, &[], seed, (16, 16), duration)
        .expect("build interrupted");
    let mut rng_b = rand::rngs::StdRng::seed_from_u64(seed);
    let mut clock_b = SimClock::new();
    let mut ledger_b = CausalLedger::new();
    for _ in 0..k {
        let tick = clock_b.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        interrupted.step(&clock_b, &active, &mut ledger_b, &mut rng_b);
        let _ = interrupted.observables();
    }

    let snap = through_disk_temp(&interrupted.snapshot(), "k60");
    let mut resumed = LiveExperimentAdapter::from_snapshot(&snap).expect("restore");
    let mut clock_c = SimClock { tick: k };
    for _ in 0..(duration - k) {
        let tick = clock_c.advance();
        let active: Vec<&InterventionCommand> = queue.active_at(tick).collect();
        resumed.step(&clock_c, &active, &mut ledger_b, &mut rng_b);
        let _ = resumed.observables();
    }

    assert_eq!(
        expected_checksum,
        resumed.checksum(),
        "Live simulation resumed from tick 60 must match uninterrupted baseline bit-exactly"
    );
    assert_eq!(expected_obs, resumed.observables());
}

/// Stress-test: Checkpoint fork with LiveExperimentAdapter.
/// Asserts that control branch reproduced bit-exact checksum equality with uninterrupted run.
#[test]
fn stress_test_live_checkpoint_fork_control_parity() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let reg = live_registry();
    let seed = 4242;
    let m = live_base_manifest(180, seed);
    let report = checkpoint_fork::<LiveExperimentAdapter>(&m, &reg, seed, 60, &[
        InterventionCommand {
            id: 10,
            cause_id: 100,
            kind: InterventionKind::Deforest,
            region: Region::Global,
            start_tick: 61,
            duration_ticks: 120,
            intensity: 0.5,
            signed_negative: true,
            curve: Curve::Step,
            reversible: false,
        },
    ]).expect("checkpoint_fork must succeed");

    let uninterrupted = run_manifest_seed::<LiveExperimentAdapter>(&m, &reg, seed, None, None);
    assert_eq!(
        report.control.final_checksum, uninterrupted.final_checksum,
        "Control branch from tick 60 diverged from uninterrupted run"
    );
    assert_ne!(
        report.control.final_checksum, report.treatment.final_checksum,
        "Treatment branch did not diverge from control"
    );
}

// =============================================================================
// SUITE 2: DEFECT 2 - Genesis Forks, Co-Temporal Forks, and Multi-Level DAGs
// =============================================================================

/// Stress-test: Deep multi-tier Genesis Chain where all branches fork at tick 0:
/// Root (0..200) -> Genesis Child (fork 0..200) -> Genesis Grandchild (fork 0..200) -> Genesis Great-Grandchild (fork 0..200)
/// Tests memoization cache at `start_tick = 0` across 4 levels of tree hierarchy.
#[test]
fn stress_test_deep_multi_tier_genesis_fork_chain_at_tick_0() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let registry = reference_registry();
    let base = reference_base_manifest(200, 112233);

    let manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "genesis_multi_tier_chain".into(),
        name: "Genesis Multi-Tier Chain Test".into(),
        base_manifest: base,
        nodes: vec![
            // Node 1: Root (0..200)
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 200,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root Control".into(),
            },
            // Node 2: Genesis Child (fork 0 from 1)
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 0,
                duration_ticks: 200,
                interventions: vec![InterventionCommand {
                    id: 201,
                    cause_id: 2001,
                    kind: InterventionKind::Deforest,
                    region: Region::Global,
                    start_tick: 1,
                    duration_ticks: 100,
                    intensity: 0.5,
                    signed_negative: true,
                    curve: Curve::Step,
                    reversible: false,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Genesis Child (fork 0 from 1)".into(),
            },
            // Node 3: Genesis Grandchild (fork 0 from 2)
            BranchNodeSpecification {
                id: 3,
                parent_id: Some(2),
                fork_tick: 0,
                duration_ticks: 200,
                interventions: vec![InterventionCommand {
                    id: 301,
                    cause_id: 3001,
                    kind: InterventionKind::RemovePredators,
                    region: Region::Global,
                    start_tick: 1,
                    duration_ticks: 100,
                    intensity: 1.0,
                    signed_negative: false,
                    curve: Curve::Step,
                    reversible: false,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Genesis Grandchild (fork 0 from 2)".into(),
            },
            // Node 4: Genesis Great-Grandchild (fork 0 from 3)
            BranchNodeSpecification {
                id: 4,
                parent_id: Some(3),
                fork_tick: 0,
                duration_ticks: 200,
                interventions: vec![InterventionCommand {
                    id: 401,
                    cause_id: 4001,
                    kind: InterventionKind::TemperatureDelta,
                    region: Region::Global,
                    start_tick: 1,
                    duration_ticks: 100,
                    intensity: 0.3,
                    signed_negative: false,
                    curve: Curve::Step,
                    reversible: true,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Genesis Great-Grandchild (fork 0 from 3)".into(),
            },
        ],
        comparisons: vec![(1, 2), (2, 3), (3, 4)],
    };

    let report = run_branch_tree::<ReferenceEvolutionWorld>(&manifest, &registry)
        .expect("multi-tier genesis chain must execute successfully");

    assert_eq!(report.nodes.len(), 4);
    for (id, node) in &report.nodes {
        let res = node.run_result.as_ref().unwrap();
        assert_eq!(res.status, RunStatus::Completed, "Node {} must complete", id);
    }
}

/// Stress-test: Massive Co-Temporal Sibling & Descendant DAG
/// Multiple branches forking at the EXACT same tick (tick 120) across 3 parent generations:
/// - Node 1: Root (0..400)
/// - Node 2: Child A (fork 120 from 1) with Deforest at 121
/// - Node 3: Child B (fork 120 from 1) with RainfallDelta at 121, DerivedStream
/// - Node 4: Grandchild A1 (fork 120 from 2 - co-temporal with 2's own fork!) with RemovePredators
/// - Node 5: Control Grandchild (fork 120 from 1 - control continuation at 120)
/// - Node 6: Great-Grandchild A1a (fork 120 from 4 - co-temporal across 3 tiers!)
#[test]
fn stress_test_massive_cotemporal_multi_tier_dag() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let registry = reference_registry();
    let base = reference_base_manifest(400, 445566);

    let manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "massive_cotemporal_dag".into(),
        name: "Massive Co-Temporal Multi-Tier DAG".into(),
        base_manifest: base,
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 400,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 120,
                duration_ticks: 400,
                interventions: vec![InterventionCommand {
                    id: 201,
                    cause_id: 2001,
                    kind: InterventionKind::Deforest,
                    region: Region::Global,
                    start_tick: 121,
                    duration_ticks: 100,
                    intensity: 0.4,
                    signed_negative: true,
                    curve: Curve::Step,
                    reversible: false,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Child A (fork 120)".into(),
            },
            BranchNodeSpecification {
                id: 3,
                parent_id: Some(1),
                fork_tick: 120,
                duration_ticks: 400,
                interventions: vec![InterventionCommand {
                    id: 301,
                    cause_id: 3001,
                    kind: InterventionKind::RainfallDelta,
                    region: Region::Global,
                    start_tick: 121,
                    duration_ticks: 100,
                    intensity: 0.5,
                    signed_negative: false,
                    curve: Curve::Step,
                    reversible: true,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::DerivedStream { stream_id: 33 },
                label: "Child B (fork 120, DerivedStream)".into(),
            },
            BranchNodeSpecification {
                id: 4,
                parent_id: Some(2),
                fork_tick: 120,
                duration_ticks: 400,
                interventions: vec![InterventionCommand {
                    id: 401,
                    cause_id: 4001,
                    kind: InterventionKind::RemovePredators,
                    region: Region::Global,
                    start_tick: 121,
                    duration_ticks: 100,
                    intensity: 1.0,
                    signed_negative: false,
                    curve: Curve::Step,
                    reversible: false,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Grandchild A1 (co-temporal fork 120 from 2)".into(),
            },
            BranchNodeSpecification {
                id: 5,
                parent_id: Some(1),
                fork_tick: 120,
                duration_ticks: 400,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Control Child at 120".into(),
            },
            BranchNodeSpecification {
                id: 6,
                parent_id: Some(4),
                fork_tick: 120,
                duration_ticks: 400,
                interventions: vec![InterventionCommand {
                    id: 601,
                    cause_id: 6001,
                    kind: InterventionKind::TemperatureDelta,
                    region: Region::Global,
                    start_tick: 121,
                    duration_ticks: 100,
                    intensity: 0.2,
                    signed_negative: true,
                    curve: Curve::Step,
                    reversible: true,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Great-Grandchild A1a (3-tier co-temporal fork 120 from 4)".into(),
            },
        ],
        comparisons: vec![(1, 5), (1, 2), (2, 4), (4, 6)],
    };

    let report = run_branch_tree::<ReferenceEvolutionWorld>(&manifest, &registry)
        .expect("massive co-temporal multi-tier DAG must execute without panics");

    assert_eq!(report.nodes.len(), 6);
    assert_eq!(report.divergences.len(), 4);

    // Node 5 (Control fork from Node 1 at 120) MUST match Node 1 bit-exactly
    let r1 = report.nodes.get(&1).unwrap().run_result.as_ref().unwrap();
    let r5 = report.nodes.get(&5).unwrap().run_result.as_ref().unwrap();
    assert_eq!(
        r1.final_checksum, r5.final_checksum,
        "Control fork at tick 120 must match root bit-exactly"
    );

    // Control comparison 1 vs 5 must have exactly 0 distance
    let div_1_5 = report.divergences.iter().find(|d| (d.branch_a_id == 1 && d.branch_b_id == 5) || (d.branch_a_id == 5 && d.branch_b_id == 1)).unwrap();
    assert_eq!(div_1_5.phase_space.max_distance, 0.0);
    assert_eq!(div_1_5.time_to_divergence, None);
}

/// Stress-test: Large 50-Node Random DAG Execution
/// Generates and executes a massive 50-node randomized branching tree with multi-level forks and interventions.
#[test]
fn stress_test_large_50_node_tree_execution() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let registry = reference_registry();
    let base = reference_base_manifest(500, 998877);

    let mut nodes = Vec::new();
    nodes.push(BranchNodeSpecification {
        id: 1,
        parent_id: None,
        fork_tick: 0,
        duration_ticks: 500,
        interventions: Vec::new(),
        exotic_interventions: Vec::new(),
        seed_strategy: BranchSeedStrategy::InheritStream,
        label: "Root Node 1".into(),
    });

    let mut comparisons = Vec::new();
    let mut node_forks = std::collections::HashMap::new();
    node_forks.insert(1u64, 0u64);

    for id in 2..=50u64 {
        // Parent is any previous node in the tree
        let parent_id = ((id * 7 + 3) % (id - 1)) + 1;
        let parent_fork = node_forks[&parent_id];

        // Fork tick must be >= parent_fork and < duration_ticks (500)
        let step = (id * 17) % 60;
        let fork_tick = (parent_fork + step).min(450);
        node_forks.insert(id, fork_tick);

        nodes.push(BranchNodeSpecification {
            id,
            parent_id: Some(parent_id),
            fork_tick,
            duration_ticks: 500,
            interventions: vec![InterventionCommand {
                id: (id * 10) as u32,
                cause_id: (id * 100) as u32,
                kind: match id % 4 {
                    0 => InterventionKind::Deforest,
                    1 => InterventionKind::RainfallDelta,
                    2 => InterventionKind::RemovePredators,
                    _ => InterventionKind::TemperatureDelta,
                },
                region: Region::Global,
                start_tick: fork_tick + 1,
                duration_ticks: 40,
                intensity: 0.2 + (id as f32 % 5.0) * 0.1,
                signed_negative: id % 2 == 0,
                curve: Curve::Step,
                reversible: true,
            }],
            exotic_interventions: Vec::new(),
            seed_strategy: if id % 4 == 0 {
                BranchSeedStrategy::DerivedStream { stream_id: id }
            } else {
                BranchSeedStrategy::InheritStream
            },
            label: format!("Node {}", id),
        });

        comparisons.push((parent_id, id));
    }

    let manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "stress_50_node_tree".into(),
        name: "Stress 50 Node Tree".into(),
        base_manifest: base,
        nodes,
        comparisons,
    };

    let report = run_branch_tree::<ReferenceEvolutionWorld>(&manifest, &registry)
        .expect("50-node stress branch tree must execute successfully");

    assert_eq!(report.nodes.len(), 50);
    assert_eq!(report.divergences.len(), 49);

    for (id, node) in &report.nodes {
        assert_eq!(node.status, anima_engine_lib::core::branch_tree::BranchExecutionStatus::Completed);
        let res = node.run_result.as_ref().unwrap();
        assert_eq!(res.status, RunStatus::Completed, "Node {} status must be completed", id);
    }
}

/// Stress-test: Adversarial Validation and Error Rejection
/// Verifies strict and exhaustive rejection of malformed branch trees:
/// 1. Cycles in branch parent references (1 -> 2 -> 3 -> 1)
/// 2. Disconnected components (Node 2 parent_id = 999)
/// 3. Inapplicable intervention: start_tick + duration <= fork_tick + 1
/// 4. Inapplicable intervention: start_tick > duration_ticks
/// 5. Invalid fork tick: fork_tick < parent.fork_tick
/// 6. Invalid fork tick: fork_tick > parent.duration_ticks
/// 7. Multiple root nodes (two nodes with parent_id = None)
/// 8. Empty nodes list
#[test]
fn stress_test_adversarial_validation_and_error_rejection() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let registry = reference_registry();
    let base = reference_base_manifest(300, 12345);

    // 1. Cycle (1 -> 2 -> 3 -> 2)
    let cycle_manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "cycle_tree".into(),
        name: "Cycle Tree".into(),
        base_manifest: base.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 300,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(3),
                fork_tick: 100,
                duration_ticks: 300,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Node 2".into(),
            },
            BranchNodeSpecification {
                id: 3,
                parent_id: Some(2),
                fork_tick: 100,
                duration_ticks: 300,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Node 3".into(),
            },
        ],
        comparisons: Vec::new(),
    };
    assert!(cycle_manifest.validate(&registry).is_err());

    // 2. Disconnected component / unknown parent
    let disconnected_manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "disconnected_tree".into(),
        name: "Disconnected Tree".into(),
        base_manifest: base.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 300,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(999),
                fork_tick: 100,
                duration_ticks: 300,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Node 2".into(),
            },
        ],
        comparisons: Vec::new(),
    };
    assert!(disconnected_manifest.validate(&registry).is_err());

    // 3. Inapplicable intervention: start_tick = 50, duration = 1 on child forking at 100
    let early_iv_manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "early_iv_tree".into(),
        name: "Early Intervention Tree".into(),
        base_manifest: base.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 300,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 100,
                duration_ticks: 300,
                interventions: vec![InterventionCommand {
                    id: 1,
                    cause_id: 10,
                    kind: InterventionKind::Deforest,
                    region: Region::Global,
                    start_tick: 50,
                    duration_ticks: 10,
                    intensity: 0.5,
                    signed_negative: true,
                    curve: Curve::Step,
                    reversible: false,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Node 2".into(),
            },
        ],
        comparisons: Vec::new(),
    };
    assert!(early_iv_manifest.validate(&registry).is_err());

    // 4. Inapplicable intervention: start_tick = 301 on branch with duration 300
    let late_iv_manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "late_iv_tree".into(),
        name: "Late Intervention Tree".into(),
        base_manifest: base.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 300,
                interventions: vec![InterventionCommand {
                    id: 1,
                    cause_id: 10,
                    kind: InterventionKind::Deforest,
                    region: Region::Global,
                    start_tick: 301,
                    duration_ticks: 10,
                    intensity: 0.5,
                    signed_negative: true,
                    curve: Curve::Step,
                    reversible: false,
                }],
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
        ],
        comparisons: Vec::new(),
    };
    assert!(late_iv_manifest.validate(&registry).is_err());

    // 5. Invalid fork tick: child fork_tick 50 < parent fork_tick 100
    let backward_fork_manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "backward_fork_tree".into(),
        name: "Backward Fork Tree".into(),
        base_manifest: base.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 300,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 100,
                duration_ticks: 300,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Node 2".into(),
            },
            BranchNodeSpecification {
                id: 3,
                parent_id: Some(2),
                fork_tick: 50,
                duration_ticks: 300,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Node 3".into(),
            },
        ],
        comparisons: Vec::new(),
    };
    assert!(backward_fork_manifest.validate(&registry).is_err());

    // 6. Invalid fork tick: child fork_tick 350 > parent duration 300
    let overshooting_fork_manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "overshooting_fork_tree".into(),
        name: "Overshooting Fork Tree".into(),
        base_manifest: base.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 300,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: Some(1),
                fork_tick: 350,
                duration_ticks: 400,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Node 2".into(),
            },
        ],
        comparisons: Vec::new(),
    };
    assert!(overshooting_fork_manifest.validate(&registry).is_err());

    // 7. Multiple roots
    let multi_root_manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "multi_root_tree".into(),
        name: "Multi Root Tree".into(),
        base_manifest: base.clone(),
        nodes: vec![
            BranchNodeSpecification {
                id: 1,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 300,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root 1".into(),
            },
            BranchNodeSpecification {
                id: 2,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 300,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "Root 2".into(),
            },
        ],
        comparisons: Vec::new(),
    };
    assert!(multi_root_manifest.validate(&registry).is_err());

    // 8. Empty nodes
    let empty_manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "empty_tree".into(),
        name: "Empty Tree".into(),
        base_manifest: base,
        nodes: Vec::new(),
        comparisons: Vec::new(),
    };
    assert!(empty_manifest.validate(&registry).is_err());
}
