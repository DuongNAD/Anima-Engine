//! Reproducibility gates for the *live* simulation's randomness.
//!
//! The headless experiment slice already forbids `thread_rng()` so a manifest replays exactly
//! (`core::exotic_energy`, `core::experiment_runner`). These tests hold the same line for the live
//! path: every stochastic decision draws from a seeded stream, and equal seeds produce equal runs.
//!
//! They are regression guards, not smoke tests. Each one fails if a future change reintroduces a
//! process-random source — `thread_rng()`, or a container whose iteration order is not defined.

use anima_engine_lib::core::resources::{
    derived_rng, derived_sim_rng, resolve_run_seed, sim_seed_override_from_env, sim_stream, SimRng,
    DEFAULT_SIM_SEED,
};
use anima_engine_lib::core::simulation_state::{
    empty_saved_state_for_tests, evolution_worker_resume_state, startup_run_seed, ChronicleEvent,
    SavedEvolutionWorkerState,
};
use anima_engine_lib::evolution::crossover::crossover_genotypes;
use anima_engine_lib::evolution::genotype::{MorphologyEdge, MorphologyGenotype, MorphologyNode};
use anima_engine_lib::evolution::map_elites::{
    EliteIndividual, MapElitesArchive, SavedMapElitesArchive,
};
use anima_engine_lib::evolution::mutation::mutate_genotype;
use glam::Vec3;
use rand::Rng;

/// Serialises every test that reads or writes `ANIMA_SIM_SEED`.
///
/// The environment is process-global while `cargo test` runs test functions on parallel threads, so
/// one test's `set_var` is visible to another's `resolve_run_seed`. Without this the suite passes
/// when a file is run alone and fails when the whole suite runs — the same interference pattern that
/// makes the terrain allocation test flaky. Poison is recovered rather than propagated so a genuine
/// assertion failure is reported as itself instead of cascading into the other tests.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn draws(rng: &mut impl Rng, n: usize) -> Vec<u64> {
    (0..n).map(|_| rng.gen()).collect()
}

fn node(id: u32, length: f32) -> MorphologyNode {
    MorphologyNode {
        id,
        length,
        radius: 0.5,
        mass: 1.0,
    }
}

fn chain_genotype(len: u32) -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    for i in 0..len {
        g.add_node(node(i, 1.0 + i as f32));
    }
    for i in 1..len {
        g.add_edge(MorphologyEdge {
            source_node: i - 1,
            target_node: i,
            joint_anchor: Vec3::new(0.5, 0.0, 0.0),
            joint_axis: Vec3::Y,
        });
    }
    g
}

fn populated_archive() -> MapElitesArchive {
    let mut archive = MapElitesArchive::new(0.5);
    // Deliberately inserted out of coordinate order: a container that preserved insertion order
    // rather than key order would still pass a same-process check, so the archive is filled in an
    // order that does not match the key order it must iterate in.
    for (i, fitness) in [(7i32, 3.0f32), (1, 9.0), (5, 1.0), (3, 20.0), (2, 7.5)] {
        archive.add_individual(EliteIndividual {
            genotype: chain_genotype(2),
            fitness,
            features: vec![i as f32 * 0.5, 0.0],
            lineage_id: format!("lin-{i}"),
            generation: 1,
        });
    }
    archive
}

#[test]
fn same_seed_reproduces_the_same_stream() {
    let mut a = SimRng::from_seed(4242);
    let mut b = SimRng::from_seed(4242);
    assert_eq!(draws(a.rng(), 64), draws(b.rng(), 64));
}

#[test]
fn different_seeds_diverge() {
    let mut a = SimRng::from_seed(4242);
    let mut b = SimRng::from_seed(4243);
    assert_ne!(
        draws(a.rng(), 64),
        draws(b.rng(), 64),
        "neighbouring seeds must not alias onto one stream"
    );
}

#[test]
fn reseed_rewinds_to_a_fresh_stream() {
    let mut rng = SimRng::from_seed(99);
    let first = draws(rng.rng(), 16);
    let _advanced = draws(rng.rng(), 16);

    rng.reseed(99);
    assert_eq!(first, draws(rng.rng(), 16));
    assert_eq!(rng.seed(), 99);
}

#[test]
fn default_seed_is_the_declared_constant() {
    assert_eq!(SimRng::default().seed(), DEFAULT_SIM_SEED);
}

#[test]
fn named_substreams_are_independent_and_reproducible() {
    const RUN_SEED: u64 = 9001;
    let world_a = draws(&mut derived_rng(RUN_SEED, sim_stream::WORLD_INIT), 32);
    let world_b = draws(&mut derived_rng(RUN_SEED, sim_stream::WORLD_INIT), 32);
    let evolution = draws(&mut derived_rng(RUN_SEED, sim_stream::EVOLUTION), 32);

    assert_eq!(world_a, world_b, "a named substream must be reproducible");
    assert_ne!(
        world_a, evolution,
        "concurrent substreams must not draw the same numbers"
    );
}

#[test]
fn checkpointable_substream_matches_the_legacy_derived_stream() {
    for (run_seed, stream) in [
        (0, sim_stream::EVOLUTION),
        (1, sim_stream::WORLD_INIT),
        (9_001, sim_stream::EVOLUTION),
        (u64::MAX, u64::MAX),
    ] {
        let mut legacy = derived_rng(run_seed, stream);
        let mut checkpointable = derived_sim_rng(run_seed, stream);
        assert_eq!(
            draws(&mut legacy, 256),
            draws(checkpointable.rng(), 256),
            "changing the RNG wrapper must not change the established substream"
        );
    }
}

#[test]
fn checkpointable_substream_resumes_at_its_exact_position() {
    let mut original = derived_sim_rng(9_001, sim_stream::EVOLUTION);
    let _ = draws(original.rng(), 137);
    let mut resumed = SimRng::restore(original.seed(), original.stream_pos());
    assert_eq!(draws(original.rng(), 256), draws(resumed.rng(), 256));
}

#[test]
fn exact_evolution_worker_checkpoint_takes_precedence_over_legacy_reconstruction() {
    let mut state = empty_saved_state_for_tests();
    state.chronicle_history.push(ChronicleEvent {
        id: "legacy-stable-event".into(),
        event_type: "Abundance".into(),
        timestamp: 0,
        title: "Stable Climate".into(),
        description: String::new(),
        parameter_delta: Default::default(),
    });
    let rng = derived_sim_rng(9_001, sim_stream::EVOLUTION);
    state.evolution_worker = Some(SavedEvolutionWorkerState {
        rng_seed: rng.seed(),
        rng_pos: 42,
        node_id_counter: 77,
        meta_ai_epoch: 1,
        meta_ai_history: vec![anima_engine_lib::evolution::meta_ai::EnvironmentalEvent::Stable],
        chronicle_ids_issued: 12,
        offspring_ids_issued: 34,
        archive: SavedMapElitesArchive {
            grid_resolution: 0.25,
            elites: Vec::new(),
        },
    });

    let resumed = evolution_worker_resume_state(Some(&state), 9_001).expect("checkpoint is valid");
    assert_eq!(resumed.rng_seed, rng.seed());
    assert_eq!(resumed.rng_pos, 42);
    assert_eq!(resumed.node_id_counter, 77);
    assert_eq!(resumed.chronicle_ids_issued, 12);
    assert_eq!(resumed.offspring_ids_issued, 34);
    assert_eq!(resumed.archive.expect("full archive").grid_resolution, 0.25);
}

#[test]
fn exact_evolution_checkpoint_rejects_an_epoch_cursor_ahead_of_its_history() {
    let mut state = empty_saved_state_for_tests();
    let rng = derived_sim_rng(9_001, sim_stream::EVOLUTION);
    state.evolution_worker = Some(SavedEvolutionWorkerState {
        rng_seed: rng.seed(),
        rng_pos: 42,
        node_id_counter: 3,
        meta_ai_epoch: 2,
        meta_ai_history: vec![anima_engine_lib::evolution::meta_ai::EnvironmentalEvent::Stable],
        chronicle_ids_issued: 0,
        offspring_ids_issued: 0,
        archive: SavedMapElitesArchive {
            grid_resolution: 0.25,
            elites: Vec::new(),
        },
    });

    let error = evolution_worker_resume_state(Some(&state), 9_001)
        .expect_err("an exact checkpoint may not skip an unrecorded Meta-AI epoch");
    assert!(
        error.contains("epoch") && error.contains("history"),
        "{error}"
    );
}

#[test]
fn exact_evolution_checkpoint_rejects_history_that_contradicts_the_chronicle() {
    let mut state = empty_saved_state_for_tests();
    state.chronicle_history.push(ChronicleEvent {
        id: "legacy-event".into(),
        event_type: "Drought".into(),
        timestamp: 0,
        title: "Resource Drought".into(),
        description: String::new(),
        parameter_delta: Default::default(),
    });
    let rng = derived_sim_rng(9_001, sim_stream::EVOLUTION);
    state.evolution_worker = Some(SavedEvolutionWorkerState {
        rng_seed: rng.seed(),
        rng_pos: 42,
        node_id_counter: 3,
        meta_ai_epoch: 1,
        meta_ai_history: vec![anima_engine_lib::evolution::meta_ai::EnvironmentalEvent::Stable],
        chronicle_ids_issued: 0,
        offspring_ids_issued: 0,
        archive: SavedMapElitesArchive {
            grid_resolution: 0.25,
            elites: Vec::new(),
        },
    });

    let error = evolution_worker_resume_state(Some(&state), 9_001)
        .expect_err("hidden worker history may not contradict the public Chronicle");
    assert!(error.contains("Chronicle"), "{error}");
}

#[test]
fn evolution_worker_checkpoint_rejects_a_stream_from_another_run() {
    let mut state = empty_saved_state_for_tests();
    state.evolution_worker = Some(SavedEvolutionWorkerState {
        rng_seed: 123,
        rng_pos: 0,
        node_id_counter: 3,
        meta_ai_epoch: 0,
        meta_ai_history: Vec::new(),
        chronicle_ids_issued: 0,
        offspring_ids_issued: 0,
        archive: SavedMapElitesArchive {
            grid_resolution: 0.25,
            elites: Vec::new(),
        },
    });

    assert!(
        evolution_worker_resume_state(Some(&state), 9_001).is_err(),
        "a foreign evolution stream must not silently join this run"
    );
}

#[test]
fn substreams_track_the_run_seed() {
    let under_one = draws(&mut derived_rng(1, sim_stream::EVOLUTION), 32);
    let under_two = draws(&mut derived_rng(2, sim_stream::EVOLUTION), 32);
    assert_ne!(
        under_one, under_two,
        "a substream must follow the run seed, not just the stream constant"
    );
}

#[test]
fn parent_selection_replays_under_one_seed() {
    let archive = populated_archive();

    let pick = |seed: u64, bias: f64| -> Vec<String> {
        let mut rng = SimRng::from_seed(seed);
        (0..40)
            .filter_map(|_| {
                archive
                    .select_parent(bias, rng.rng())
                    .map(|e| e.lineage_id.clone())
            })
            .collect()
    };

    // Uniform sampling and tournament sampling take different branches; both must replay.
    assert_eq!(pick(7, 1.0), pick(7, 1.0));
    assert_eq!(pick(7, 5.0), pick(7, 5.0));
    assert_ne!(
        pick(7, 1.0),
        pick(8, 1.0),
        "selection must actually consume the stream"
    );
}

#[test]
fn archive_iteration_follows_niche_coordinates_not_insertion() {
    // The archive is walked during parent selection, so its order is part of the reproducibility
    // contract. `HashMap` seeds its hasher per process and would break this across runs.
    let archive = populated_archive();
    let coords: Vec<(i32, i32)> = archive.grid.keys().copied().collect();

    let mut sorted = coords.clone();
    sorted.sort_unstable();
    assert_eq!(coords, sorted, "archive must iterate in niche-key order");
}

#[test]
fn mutation_replays_under_one_seed() {
    let base = chain_genotype(4);

    let run = |seed: u64| {
        let mut g = base.clone();
        let mut counter = 100u32;
        let mut rng = SimRng::from_seed(seed);
        for _ in 0..25 {
            mutate_genotype(&mut g, &mut counter, 1.0, rng.rng())
                .expect("test node cursor has headroom");
        }
        (
            g.nodes.iter().map(|n| (n.id, n.length)).collect::<Vec<_>>(),
            g.edges
                .iter()
                .map(|e| (e.source_node, e.target_node))
                .collect::<Vec<_>>(),
            counter,
        )
    };

    assert_eq!(run(11), run(11));
    assert_ne!(run(11), run(12), "mutation must consume the stream");
}

#[test]
fn crossover_replays_under_one_seed() {
    let parent_a = chain_genotype(4);
    let mut parent_b = chain_genotype(3);
    for n in parent_b.nodes.iter_mut() {
        n.id += 50;
        n.length += 10.0;
    }
    for e in parent_b.edges.iter_mut() {
        e.source_node += 50;
        e.target_node += 50;
    }

    let run = |seed: u64| {
        let mut counter = 200u32;
        let mut rng = SimRng::from_seed(seed);
        let children: Vec<Vec<(u32, f32)>> = (0..15)
            .map(|_| {
                let child = crossover_genotypes(&parent_a, &parent_b, &mut counter, rng.rng())
                    .expect("test node cursor has headroom");
                child.nodes.iter().map(|n| (n.id, n.length)).collect()
            })
            .collect();
        (children, counter)
    };

    assert_eq!(run(21), run(21));
}

/// The type system cannot stop someone reaching for `rand::thread_rng()` again, so this walks the
/// backend sources and fails if one reappears. Prose mentioning the ban is fine; a call is not.
#[test]
fn no_process_random_source_in_backend_sources() {
    fn scan(dir: &std::path::Path, hits: &mut Vec<String>) {
        let entries = std::fs::read_dir(dir).expect("readable source directory");
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan(&path, hits);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).expect("readable source file");
                for (i, line) in text.lines().enumerate() {
                    let trimmed = line.trim_start();
                    if trimmed.starts_with("//") {
                        continue; // documentation about the rule, not a use of it
                    }
                    if line.contains("thread_rng") {
                        hits.push(format!("{}:{}", path.display(), i + 1));
                    }
                }
            }
        }
    }

    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut hits = Vec::new();
    scan(&src, &mut hits);

    assert!(
        hits.is_empty(),
        "`thread_rng()` is process-random and breaks run reproducibility. Take `SimRng` \
         (systems) or `derived_rng` (setup/worker threads) instead. Found at: {hits:?}"
    );
}

#[test]
fn live_simulation_does_not_bypass_the_deterministic_id_gate() {
    let simulation_loop = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("core")
            .join("simulation_loop.rs"),
    )
    .expect("simulation loop source must be readable");

    assert!(
        !simulation_loop.contains("Uuid::new_v4"),
        "live founders and offspring must go through determinism::next_entity_id"
    );
}

#[test]
fn run_seed_comes_from_the_world_not_from_ambient_state() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Invariant D07: the world the agents live in is the authority for the run's randomness.
    assert_eq!(resolve_run_seed(4242), 4242);
    assert_eq!(SimRng::for_world(4242).seed(), 4242);
    assert_ne!(
        SimRng::for_world(4242).seed(),
        SimRng::for_world(4243).seed(),
        "two worlds must not share a trajectory"
    );
}

#[test]
fn env_seed_override_is_honoured() {
    // This test mutates the shared environment, so it holds the lock for its whole body and restores
    // the prior value before releasing it.
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let previous = std::env::var("ANIMA_SIM_SEED").ok();

    std::env::set_var("ANIMA_SIM_SEED", "24680");
    assert_eq!(sim_seed_override_from_env(), Some(24680));
    assert_eq!(
        resolve_run_seed(4242),
        24680,
        "an explicit override must beat the world seed"
    );

    // A malformed value is ignored rather than fatal, and the world seed still wins.
    std::env::set_var("ANIMA_SIM_SEED", "not-a-number");
    assert_eq!(sim_seed_override_from_env(), None);
    assert_eq!(resolve_run_seed(4242), 4242);

    match previous {
        Some(v) => std::env::set_var("ANIMA_SIM_SEED", v),
        None => std::env::remove_var("ANIMA_SIM_SEED"),
    }
}

#[test]
fn resumed_snapshot_seed_is_the_startup_authority() {
    let mut state = empty_saved_state_for_tests();
    state.sim_rng_seed = 98_765;
    state.sim_rng_pos = 42;

    assert_eq!(
        startup_run_seed(Some(&state), 12_345),
        98_765,
        "every worker started for a resumed run must use the snapshot's RNG seed"
    );
}

#[test]
fn legacy_snapshot_without_rng_state_uses_the_fallback_seed() {
    let state = empty_saved_state_for_tests();

    assert_eq!(
        startup_run_seed(Some(&state), 12_345),
        12_345,
        "a pre-G1.2 snapshot has no saved RNG authority and must preserve legacy startup behaviour"
    );
}

#[test]
fn zero_seed_with_a_nonzero_position_is_valid_saved_rng_state() {
    let mut state = empty_saved_state_for_tests();
    state.sim_rng_seed = 0;
    state.sim_rng_pos = 1;

    assert_eq!(
        startup_run_seed(Some(&state), 12_345),
        0,
        "seed zero is valid when the saved stream position proves RNG state was recorded"
    );
}

#[test]
fn resumed_identity_source_continues_after_existing_ids() {
    let existing = [
        "lineage-000000000000002a-00000000",
        "unrelated-000000000000002a-ffffffff",
        "lineage-000000000000002b-ffffffff",
        "lineage-000000000000002a-00000007",
    ];
    let issued =
        anima_engine_lib::core::determinism::issued_after_existing_ids(0x2a, "lineage", existing)
            .expect("valid deterministic ids");
    let resumed =
        anima_engine_lib::core::determinism::RunIdentity::with_issued(0x2a, "lineage", issued);

    assert_eq!(
        resumed.next_id(),
        "lineage-000000000000002a-00000008",
        "resume must not mint an id already present in the checkpoint"
    );
}

#[test]
fn evolution_worker_resume_state_recovers_counters_and_history() {
    let mut state = empty_saved_state_for_tests();
    state.chronicle_history = vec![
        ChronicleEvent {
            id: "chronicle-000000000000002a-00000000".into(),
            event_type: "Drought".into(),
            timestamp: 0,
            title: "Resource Drought".into(),
            description: String::new(),
            parameter_delta: Default::default(),
        },
        ChronicleEvent {
            id: "chronicle-000000000000002a-00000003".into(),
            event_type: "TemperatureSpike".into(),
            timestamp: 0,
            title: "Glacial Period".into(),
            description: String::new(),
            parameter_delta: Default::default(),
        },
    ];
    let mut high_node_genotype = chain_genotype(2);
    high_node_genotype.nodes[1].id = 91;
    high_node_genotype.edges[0].target_node = 91;
    state
        .lineage_nodes
        .push(anima_engine_lib::evolution::lineage::LineageNode {
            id: "lineage-000000000000002a-0000000b".into(),
            generation: 1,
            genotype: Some(high_node_genotype),
            cumulative_mutations: Some(0),
        });

    let resume = evolution_worker_resume_state(Some(&state), 0x2a)
        .expect("ordinary checkpoint counters must be recoverable");
    assert_eq!(resume.chronicle_ids_issued, 4);
    assert_eq!(resume.offspring_ids_issued, 12);
    assert_eq!(resume.node_id_counter, 92);
    assert_eq!(resume.meta_ai_epoch, 2);
    assert_eq!(
        resume.meta_ai_history,
        vec![
            anima_engine_lib::evolution::meta_ai::EnvironmentalEvent::ResourceDrought,
            anima_engine_lib::evolution::meta_ai::EnvironmentalEvent::GlacialPeriod,
        ]
    );
}

#[test]
fn legacy_resume_counts_only_chronicle_events_that_belong_to_meta_ai_history() {
    let mut state = empty_saved_state_for_tests();
    state.chronicle_history = vec![
        ChronicleEvent {
            id: "operator-note".into(),
            event_type: "OperatorNote".into(),
            timestamp: 0,
            title: "Calibration complete".into(),
            description: String::new(),
            parameter_delta: Default::default(),
        },
        ChronicleEvent {
            id: "legacy-drought".into(),
            event_type: "Drought".into(),
            timestamp: 1,
            title: "Resource Drought".into(),
            description: String::new(),
            parameter_delta: Default::default(),
        },
    ];

    let resume = evolution_worker_resume_state(Some(&state), 0x2a)
        .expect("an unrelated Chronicle entry must not corrupt legacy Meta-AI recovery");
    assert_eq!(resume.meta_ai_epoch, 1);
    assert_eq!(
        resume.meta_ai_history,
        vec![anima_engine_lib::evolution::meta_ai::EnvironmentalEvent::ResourceDrought]
    );
}

/// The evolution thread is spawned before the ECS world exists, so it receives the same startup
/// seed that the world uses. If those two paths ever disagree, the evolution stream silently stops
/// belonging to the world it is evolving in — which no other test would notice.
#[test]
fn evolution_thread_and_world_agree_on_seed() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let world = anima_engine_lib::core::ecs::init_world();

    let in_world = world
        .get_resource::<SimRng>()
        .expect("init_world must seed the run")
        .seed();
    let identity_seed = world
        .get_resource::<anima_engine_lib::core::world_artifact::WorldIdentity>()
        .expect("init_world must publish the world identity")
        .seed;

    let pre_world =
        resolve_run_seed(anima_engine_lib::core::world_artifact::world_seed_from_disk());

    assert_eq!(
        in_world,
        resolve_run_seed(identity_seed),
        "the world's RNG must be seeded from the world's own identity"
    );
    assert_eq!(
        pre_world, in_world,
        "the pre-world resolver and init_world must pick the same run seed"
    );
}

#[test]
fn peek_seed_matches_a_full_decode() {
    use anima_engine_lib::core::world_artifact::WorldArtifact;

    // The fixture is the frontend-encoded artifact the cross-language gate already relies on, so
    // this also pins `peek_seed` against a real file rather than a hand-built header.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("world_4x4.anmw");
    let bytes = std::fs::read(&path).expect("cross-language fixture must exist");

    let full = WorldArtifact::from_bytes(&bytes).expect("fixture must decode");
    assert_eq!(WorldArtifact::peek_seed(&bytes), Ok(full.seed));

    // A corrupt header must be rejected here exactly as a full decode rejects it.
    let mut bad_magic = bytes.clone();
    bad_magic[0] = b'X';
    assert!(WorldArtifact::peek_seed(&bad_magic).is_err());
    assert!(WorldArtifact::peek_seed(&bytes[..8]).is_err());
}

/// `SimRng` names `ChaCha12Rng` rather than `StdRng` so a snapshot can restore the draw position
/// (G1.2). In rand 0.8 `StdRng` IS a newtype over `ChaCha12Rng`, so that swap must be invisible —
/// this pins it instead of trusting the documentation. If rand ever repoints `StdRng` at a
/// different algorithm, this fails and tells you the trajectory of every existing run just moved.
#[test]
fn simrng_stream_matches_stdrng_exactly() {
    use rand::{Rng, SeedableRng};
    for seed in [0u64, 1, 1337, 0x5EED, u64::MAX] {
        let mut sim = anima_engine_lib::core::resources::SimRng::from_seed(seed);
        let mut std_rng = rand::rngs::StdRng::seed_from_u64(seed);
        for i in 0..256 {
            assert_eq!(
                sim.rng().gen::<u64>(),
                std_rng.gen::<u64>(),
                "seed {seed} diverged at draw {i}"
            );
        }
    }
}

/// The half of the stream state a checkpoint has to carry. Restoring seed alone is not enough:
/// the resumed stream must continue where the saved one left off, not restart.
#[test]
fn simrng_restores_its_exact_stream_position() {
    use rand::Rng;
    let mut original = anima_engine_lib::core::resources::SimRng::from_seed(1337);
    for _ in 0..1000 {
        let _: u64 = original.rng().gen();
    }
    let pos = original.stream_pos();

    let mut resumed = anima_engine_lib::core::resources::SimRng::restore(1337, pos);
    for i in 0..256 {
        assert_eq!(
            resumed.rng().gen::<u64>(),
            original.rng().gen::<u64>(),
            "resumed stream diverged at draw {i}"
        );
    }

    // And the naive "just reseed" path must NOT match, or this test proves nothing.
    let mut reseeded = anima_engine_lib::core::resources::SimRng::from_seed(1337);
    let mut fresh = anima_engine_lib::core::resources::SimRng::restore(1337, pos);
    assert_ne!(
        reseeded.rng().gen::<u64>(),
        fresh.rng().gen::<u64>(),
        "reseeding from the same seed must not accidentally land on the saved position"
    );
}
