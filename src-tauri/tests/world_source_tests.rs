//! Regression tests for F051 (`WORLD-SRC`):
//! - Headless live experiments (`LiveExperimentAdapter` / `build_live_world`) always generate their
//!   own terrain via `TerrainSource::Generated` (`MapSettings::default()`) and record both
//!   `terrain_source = "generated"` and the observed `WorldIdentity` in provenance.
//! - Machine environment variables (`ANIMA_WORLD_ARTIFACT`, `ANIMA_CACHE_DIR`) and desktop-app
//!   temp artifacts (`<temp_dir>/anima_world_artifact.anmw`) never alter a headless run.
//! - The desktop app's `ecs::init_world()` (`TerrainSource::AppArtifactOrGenerated`) retains its
//!   existing artifact-loading behaviour (positive control).
//!
//! All environment-variable mutations live inside ONE `#[test]` function so parallel test threads
//! cannot race on process-global environment variables.

use anima_engine_lib::core::branch_tree::{
    run_branch_tree, BranchNodeSpecification, BranchSeedStrategy, BranchTreeManifest,
    BRANCH_TREE_SCHEMA_VERSION,
};
use anima_engine_lib::core::ecs::init_world;
use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, WorldLawSet,
    MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{checkpoint_fork, run_manifest_seed};
use anima_engine_lib::core::live_experiment::{LiveExperimentAdapter, LIVE_OBSERVABLE_IDS};
use anima_engine_lib::core::terrain::TerrainMap;
use anima_engine_lib::core::world_artifact::{default_artifact_path, WorldIdentity};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const ENV_VARS: [&str; 5] = [
    "ANIMA_WORLD_ARTIFACT",
    "ANIMA_CACHE_DIR",
    "TMPDIR",
    "TEMP",
    "TMP",
];

struct EnvGuard {
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl EnvGuard {
    fn capture() -> Self {
        let saved = ENV_VARS
            .iter()
            .map(|&name| (name, std::env::var_os(name)))
            .collect();
        Self { saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (name, val) in &self.saved {
            match val {
                Some(v) => std::env::set_var(name, v),
                None => std::env::remove_var(name),
            }
        }
    }
}

fn live_manifest(duration_ticks: u64) -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "f051::world_source".to_string(),
        name: "F051 World Source Check".to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: InitialConditionSet::new(vec![
            ("live.mortality".to_string(), 1.0),
            ("live.reproduction".to_string(), 1.0),
            ("live.predator_fraction".to_string(), 0.2),
            ("live.founders".to_string(), 8.0),
            ("live.food_cap".to_string(), 40.0),
            ("live.land_mask".to_string(), 1.0),
        ]),
        interventions: Vec::new(),
        seeds: vec![100_001],
        duration_ticks,
        sample_period: 60,
        observable_ids: LIVE_OBSERVABLE_IDS
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        exotic_interventions: Vec::new(),
        observer: Default::default(),
    }
}

fn set_temp_root(dir: &Path) {
    std::env::set_var("TMPDIR", dir);
    std::env::set_var("TEMP", dir);
    std::env::set_var("TMP", dir);
}

fn app_world_fingerprint() -> (WorldIdentity, TerrainMap) {
    let world = init_world();
    let identity = *world.resource::<WorldIdentity>();
    let terrain = world.resource::<TerrainMap>().clone();
    (identity, terrain)
}

#[test]
fn headless_live_world_ignores_env_and_temp_artifacts_while_app_init_world_reads_them() {
    let _env_guard = EnvGuard::capture();

    let repo_artifact = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR has repo parent")
        .join("artifacts/world_256.anmw");
    assert!(
        repo_artifact.is_file(),
        "expected fixture artifact at {}",
        repo_artifact.display()
    );

    let scratch_root =
        std::env::temp_dir().join(format!("anima_world_source_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch_root);
    let empty_tmp: PathBuf = scratch_root.join("empty_tmp");
    let empty_cache: PathBuf = scratch_root.join("empty_cache");
    let artifact_tmp: PathBuf = scratch_root.join("artifact_tmp");
    std::fs::create_dir_all(&empty_tmp).expect("mkdir empty_tmp");
    std::fs::create_dir_all(&empty_cache).expect("mkdir empty_cache");
    std::fs::create_dir_all(&artifact_tmp).expect("mkdir artifact_tmp");

    let temp_artifact_copy = artifact_tmp.join("anima_world_artifact.anmw");
    std::fs::copy(&repo_artifact, &temp_artifact_copy).expect("copy world_256.anmw");

    let registry = ObservableRegistry::live_default();
    let manifest = live_manifest(180);
    let seed = manifest.seeds[0];

    // Setting 1: ANIMA_WORLD_ARTIFACT unset and empty temp dir (no anima_world_artifact.anmw).
    std::env::remove_var("ANIMA_WORLD_ARTIFACT");
    std::env::set_var("ANIMA_CACHE_DIR", &empty_cache);
    set_temp_root(&empty_tmp);
    assert!(
        !default_artifact_path().exists(),
        "clean temp setting must have no artifact at {}",
        default_artifact_path().display()
    );
    let (app_clean_id, app_clean_terrain) = app_world_fingerprint();
    let run_clean =
        run_manifest_seed::<LiveExperimentAdapter>(&manifest, &registry, seed, None, None);
    assert!(run_clean.status.is_completed(), "{:?}", run_clean.status);

    // Setting 2: ANIMA_WORLD_ARTIFACT points at artifacts/world_256.anmw.
    std::env::set_var("ANIMA_WORLD_ARTIFACT", &repo_artifact);
    set_temp_root(&empty_tmp);
    assert_eq!(default_artifact_path(), repo_artifact);
    let (app_env_id, app_env_terrain) = app_world_fingerprint();
    let run_env_artifact =
        run_manifest_seed::<LiveExperimentAdapter>(&manifest, &registry, seed, None, None);
    assert!(
        run_env_artifact.status.is_completed(),
        "{:?}",
        run_env_artifact.status
    );

    // Setting 3: ANIMA_WORLD_ARTIFACT unset, temp dir holds anima_world_artifact.anmw.
    std::env::remove_var("ANIMA_WORLD_ARTIFACT");
    set_temp_root(&artifact_tmp);
    assert!(
        default_artifact_path().is_file(),
        "temp setting should resolve an existing artifact at {}",
        default_artifact_path().display()
    );
    let (app_temp_id, app_temp_terrain) = app_world_fingerprint();
    let run_temp_artifact =
        run_manifest_seed::<LiveExperimentAdapter>(&manifest, &registry, seed, None, None);
    assert!(
        run_temp_artifact.status.is_completed(),
        "{:?}",
        run_temp_artifact.status
    );

    // (a) All three headless runs match bit-for-bit and record terrain_source = "generated".
    assert_eq!(
        run_clean.final_checksum, run_env_artifact.final_checksum,
        "headless run must ignore ANIMA_WORLD_ARTIFACT"
    );
    assert_eq!(
        run_clean.final_checksum, run_temp_artifact.final_checksum,
        "headless run must ignore <temp_dir>/anima_world_artifact.anmw"
    );
    assert_eq!(
        run_clean.final_observables,
        run_env_artifact.final_observables
    );
    assert_eq!(
        run_clean.final_observables,
        run_temp_artifact.final_observables
    );

    for (label, res) in [
        ("clean", &run_clean),
        ("ANIMA_WORLD_ARTIFACT", &run_env_artifact),
        ("temp artifact", &run_temp_artifact),
    ] {
        assert_eq!(
            res.provenance.terrain_source.as_deref(),
            Some("generated"),
            "{label}: expected generated terrain_source"
        );
        let id = res
            .provenance
            .world_identity
            .unwrap_or_else(|| panic!("{label}: missing provenance.world_identity"));
        assert_ne!(id.checksum, 0, "{label}: terrain checksum must be non-zero");
        assert_eq!(id.width, 256, "{label}: width");
        assert_eq!(id.height, 256, "{label}: height");
    }
    assert_eq!(
        run_clean.provenance.world_identity,
        run_env_artifact.provenance.world_identity
    );
    assert_eq!(
        run_clean.provenance.world_identity,
        run_temp_artifact.provenance.world_identity
    );

    // (b) Positive control: the app's init_world() DOES switch terrain when an artifact is present.
    assert_ne!(
        app_env_id, app_clean_id,
        "positive control: init_world() must read ANIMA_WORLD_ARTIFACT when set"
    );
    assert_ne!(
        app_env_terrain.biomes, app_clean_terrain.biomes,
        "positive control: artifact terrain biomes must differ from MapSettings::default()"
    );
    assert_eq!(
        app_env_id, app_temp_id,
        "temp-dir anima_world_artifact.anmw and ANIMA_WORLD_ARTIFACT must load the same world"
    );
    assert_eq!(app_env_terrain.biomes, app_temp_terrain.biomes);
    assert_eq!(
        run_clean.provenance.world_identity,
        Some(app_clean_id),
        "clean headless run must match MapSettings::default() clean init_world()"
    );

    let _ = std::fs::remove_dir_all(&scratch_root);
}

#[test]
fn checkpoint_fork_and_branch_tree_record_generated_world_provenance() {
    let registry = ObservableRegistry::live_default();
    let manifest = live_manifest(120);
    let seed = manifest.seeds[0];

    let fork = checkpoint_fork::<LiveExperimentAdapter>(&manifest, &registry, seed, 60, &[])
        .expect("checkpoint_fork succeeds");
    assert_eq!(
        fork.prefix.provenance.terrain_source.as_deref(),
        Some("generated")
    );
    assert_eq!(
        fork.control.provenance.terrain_source.as_deref(),
        Some("generated")
    );
    assert_eq!(
        fork.treatment.provenance.terrain_source.as_deref(),
        Some("generated")
    );
    let fork_id = fork
        .prefix
        .provenance
        .world_identity
        .expect("prefix records world_identity");
    assert_ne!(fork_id.checksum, 0);
    assert_eq!(fork.control.provenance.world_identity, Some(fork_id));
    assert_eq!(fork.treatment.provenance.world_identity, Some(fork_id));

    let tree_manifest = BranchTreeManifest {
        schema_version: BRANCH_TREE_SCHEMA_VERSION,
        tree_id: "f051::tree".to_string(),
        name: "F051 Branch Tree World Provenance".to_string(),
        base_manifest: manifest,
        nodes: vec![
            BranchNodeSpecification {
                id: 0,
                parent_id: None,
                fork_tick: 0,
                duration_ticks: 120,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::ExplicitSeed { seed },
                label: "root".to_string(),
            },
            BranchNodeSpecification {
                id: 1,
                parent_id: Some(0),
                fork_tick: 60,
                duration_ticks: 120,
                interventions: Vec::new(),
                exotic_interventions: Vec::new(),
                seed_strategy: BranchSeedStrategy::InheritStream,
                label: "child".to_string(),
            },
        ],
        comparisons: Vec::new(),
    };

    let tree = run_branch_tree::<LiveExperimentAdapter>(&tree_manifest, &registry)
        .expect("run_branch_tree succeeds");
    let root_prov = &tree.nodes[&0].run_result.as_ref().unwrap().provenance;
    let child_prov = &tree.nodes[&1].run_result.as_ref().unwrap().provenance;
    assert_eq!(root_prov.terrain_source.as_deref(), Some("generated"));
    assert_eq!(child_prov.terrain_source.as_deref(), Some("generated"));
    assert_eq!(root_prov.world_identity, Some(fork_id));
    assert_eq!(child_prov.world_identity, Some(fork_id));
}
