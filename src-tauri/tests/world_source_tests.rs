//! Regression tests for F051 (`WORLD-SRC`) & F051b:
//! - Headless live experiments (`LiveExperimentAdapter` / `build_live_world`) always generate their
//!   own terrain via `TerrainSource::Generated` (`MapSettings::default()`, `MapConfig::default()`)
//!   and record both `terrain_source = "generated"` and the observed `WorldIdentity` in provenance.
//! - Machine environment variables (`ANIMA_WORLD_ARTIFACT`, `ANIMA_CACHE_DIR`), desktop-app temp
//!   artifacts (`<temp_dir>/anima_world_artifact.anmw`), cached terrain files (`world_*.bin`), and
//!   working-directory `map_config.json` never alter a headless run and `Generated` never writes to
//!   the terrain cache.
//! - The desktop app's `ecs::init_world()` (`TerrainSource::AppArtifactOrGenerated`) retains its
//!   existing artifact, cache, and `map_config.json` loading behaviour (positive controls).

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
use anima_engine_lib::core::terrain::{
    BiomeType, MapConfig, MapSettings, TerrainMap, WORLD_CACHE_VERSION,
};
use anima_engine_lib::core::world_artifact::{default_artifact_path, WorldIdentity};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

const CHILD_MODE_ENV: &str = "ANIMA_WORLD_SRC_CHILD_MODE";
const CHILD_SCRATCH_CWD_ENV: &str = "ANIMA_WORLD_SRC_SCRATCH_CWD";
const CHILD_HEADLESS_MARKER: &str = "WORLD_SRC_CHILD_HEADLESS";
const CHILD_APP_MARKER: &str = "WORLD_SRC_CHILD_APP";

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
        island: None,
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
    let _env_lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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

    // (b) Positive control (WV2-7 desktop switchover):
    // - Clean init_world() (ANIMA_WORLD_ARTIFACT unset) boots C01 sim_256.anmw3 with
    //   sim_world_sha256 == Some(<index.json sim_world_identity>), distinct from Generated;
    // - Explicit ANIMA_WORLD_ARTIFACT loads that legacy .anmw file (sim_world_sha256 == None);
    // - <temp_dir>/anima_world_artifact.anmw is retired and ignored when ANIMA_WORLD_ARTIFACT is unset.
    let expected_c01_sha = {
        let idx_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("CARGO_MANIFEST_DIR has repo parent")
            .join("artifacts/world-v2/index.json");
        let raw_idx =
            std::fs::read_to_string(&idx_path).expect("read artifacts/world-v2/index.json");
        let parsed: serde_json::Value = serde_json::from_str(&raw_idx).expect("parse index.json");
        let hex = parsed["sim_world_identity"]
            .as_str()
            .expect("sim_world_identity hex");
        anima_worldgen::anmw_v3::sha256_from_hex(hex).expect("valid sha256 hex")
    };
    assert_eq!(
        app_clean_id.sim_world_sha256,
        Some(expected_c01_sha),
        "clean init_world() must boot bundled C01 sim_256.anmw3 with verified sim_world_sha256"
    );
    assert_ne!(
        app_env_id, app_clean_id,
        "positive control: init_world() must read explicit ANIMA_WORLD_ARTIFACT when set"
    );
    assert_eq!(
        app_env_id.sim_world_sha256, None,
        "legacy v2 ANIMA_WORLD_ARTIFACT keeps sim_world_sha256 = None"
    );
    assert_ne!(
        app_env_terrain.biomes, app_clean_terrain.biomes,
        "positive control: legacy ANIMA_WORLD_ARTIFACT biomes must differ from C01 sim_256.anmw3"
    );
    assert_eq!(
        app_temp_id, app_clean_id,
        "WV2-7 retires <temp_dir>/anima_world_artifact.anmw on the desktop path"
    );
    assert_eq!(
        app_temp_terrain.biomes, app_clean_terrain.biomes,
        "retired <temp_dir>/anima_world_artifact.anmw must not alter desktop terrain"
    );
    let default_settings = MapSettings::default();
    let default_generated =
        TerrainMap::generate_with_config(&default_settings, &MapConfig::default());
    let default_expected_id = WorldIdentity::from_terrain(
        &default_generated,
        default_settings.seed,
        WORLD_CACHE_VERSION,
    );
    assert_ne!(
        app_clean_id, default_expected_id,
        "switched-over C01 desktop world differs from procedural MapSettings::default()"
    );
    assert_eq!(
        run_clean.provenance.world_identity,
        Some(default_expected_id),
        "Generated world_identity must match TerrainMap::generate_with_config(default, default)"
    );

    let _ = std::fs::remove_dir_all(&scratch_root);
}

struct CwdGuard {
    original: PathBuf,
}

impl CwdGuard {
    fn enter(target: &Path) -> Self {
        let original = std::env::current_dir().expect("capture current_dir");
        std::env::set_current_dir(target).expect("set_current_dir to scratch dir");
        Self { original }
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.original);
    }
}

struct ChildProbeOutcome {
    headless_final_checksum: Option<u32>,
    headless_world_id_checksum: Option<u32>,
    app_world_id_checksum: Option<u32>,
}

fn run_child_probe(
    mode: &str,
    cwd: &Path,
    cache_dir: Option<&Path>,
    temp_root: &Path,
    scratch_cwd: Option<&Path>,
) -> ChildProbeOutcome {
    let mut cmd = Command::new(std::env::current_exe().expect("current_exe"));
    cmd.args(["--exact", "world_source_child_entry_point", "--nocapture"])
        .current_dir(cwd)
        .env(CHILD_MODE_ENV, mode)
        .env("ANIMA_USE_GPU", "0")
        .env("TMPDIR", temp_root)
        .env("TEMP", temp_root)
        .env("TMP", temp_root)
        .env_remove("ANIMA_WORLD_ARTIFACT");
    match cache_dir {
        Some(dir) => {
            cmd.env("ANIMA_CACHE_DIR", dir);
        }
        None => {
            cmd.env_remove("ANIMA_CACHE_DIR");
        }
    }
    match scratch_cwd {
        Some(dir) => {
            cmd.env(CHILD_SCRATCH_CWD_ENV, dir);
        }
        None => {
            cmd.env_remove(CHILD_SCRATCH_CWD_ENV);
        }
    }

    let output = cmd.output().expect("spawn world_source child");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "world_source child ({mode}) failed with {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status.code()
    );

    let mut headless_final_checksum = None;
    let mut headless_world_id_checksum = None;
    let mut app_world_id_checksum = None;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(CHILD_HEADLESS_MARKER) {
            let mut parts = rest.split_whitespace();
            headless_final_checksum = parts.next().and_then(|s| s.parse::<u32>().ok());
            headless_world_id_checksum = parts.next().and_then(|s| s.parse::<u32>().ok());
        } else if let Some(rest) = trimmed.strip_prefix(CHILD_APP_MARKER) {
            app_world_id_checksum = rest.trim().parse::<u32>().ok();
        }
    }
    ChildProbeOutcome {
        headless_final_checksum,
        headless_world_id_checksum,
        app_world_id_checksum,
    }
}

#[test]
fn world_source_child_entry_point() {
    let Ok(mode) = std::env::var(CHILD_MODE_ENV) else {
        return;
    };
    let _cwd_guard = std::env::var_os(CHILD_SCRATCH_CWD_ENV).map(|dir| {
        let guard = CwdGuard::enter(Path::new(&dir));
        assert!(
            Path::new("map_config.json").is_file(),
            "scratch CWD must hold map_config.json"
        );
        guard
    });

    match mode.as_str() {
        "seed_app_cache" => {
            let cache_dir = std::env::var_os("ANIMA_CACHE_DIR")
                .map(PathBuf::from)
                .expect("ANIMA_CACHE_DIR set for seed_app_cache");
            let _ = TerrainMap::load_or_generate(&MapSettings::default(), &cache_dir);
        }
        "headless_only" => {
            let registry = ObservableRegistry::live_default();
            let manifest = live_manifest(180);
            let res = run_manifest_seed::<LiveExperimentAdapter>(
                &manifest,
                &registry,
                manifest.seeds[0],
                None,
                None,
            );
            assert!(res.status.is_completed(), "{:?}", res.status);
            assert_eq!(res.provenance.terrain_source.as_deref(), Some("generated"));
            let id = res
                .provenance
                .world_identity
                .expect("headless run must record world_identity");
            println!(
                "{CHILD_HEADLESS_MARKER} {} {}",
                res.final_checksum, id.checksum
            );
        }
        "headless_then_app" => {
            // Headless MUST execute BEFORE init_world() while both GENERATED_DEFAULT_TERRAIN and
            // MAP_CONFIG OnceLocks are cold in this child process.
            let registry = ObservableRegistry::live_default();
            let manifest = live_manifest(180);
            let res = run_manifest_seed::<LiveExperimentAdapter>(
                &manifest,
                &registry,
                manifest.seeds[0],
                None,
                None,
            );
            assert!(res.status.is_completed(), "{:?}", res.status);
            assert_eq!(res.provenance.terrain_source.as_deref(), Some("generated"));
            let id = res
                .provenance
                .world_identity
                .expect("headless run must record world_identity");
            println!(
                "{CHILD_HEADLESS_MARKER} {} {}",
                res.final_checksum, id.checksum
            );

            let (app_id, _) = app_world_fingerprint();
            println!("{CHILD_APP_MARKER} {}", app_id.checksum);
        }
        other => panic!("unexpected {CHILD_MODE_ENV}={other}"),
    }
}

#[test]
fn generated_ignores_poisoned_cache_and_modified_map_config_in_fresh_processes() {
    let _env_lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let settings = MapSettings::default();
    let expected_terrain = TerrainMap::generate_with_config(&settings, &MapConfig::default());
    let expected_id =
        WorldIdentity::from_terrain(&expected_terrain, settings.seed, WORLD_CACHE_VERSION);

    let modified_config = MapConfig {
        sea_ratio: 0.65,
        sand_ratio: 0.72,
        snow_ratio: 0.92,
        noise_scale: 5.5,
    };
    let modified_terrain = TerrainMap::generate_with_config(&settings, &modified_config);
    let modified_id =
        WorldIdentity::from_terrain(&modified_terrain, settings.seed, WORLD_CACHE_VERSION);
    assert_ne!(
        modified_id, expected_id,
        "modified MapConfig must change WorldIdentity"
    );
    assert_ne!(
        modified_terrain.biomes, expected_terrain.biomes,
        "modified MapConfig must change biomes"
    );

    let scratch_root =
        std::env::temp_dir().join(format!("anima_f051b_subproc_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch_root);
    let empty_cwd = scratch_root.join("empty_cwd");
    let scratch_cwd = scratch_root.join("scratch_cwd");
    let seed_cache = scratch_root.join("seed_cache");
    let empty_tmp = scratch_root.join("empty_tmp");
    let clean_cache = scratch_root.join("clean_cache");
    let clean_tmp = scratch_root.join("clean_tmp");
    let poison_env_cache = scratch_root.join("poison_env_cache");
    let poison_tmp_root = scratch_root.join("poison_tmp_root");
    let poison_tmp_cache = poison_tmp_root.join("anima_world_cache");
    let map_cfg_cache = scratch_root.join("map_cfg_cache");
    let map_cfg_tmp = scratch_root.join("map_cfg_tmp");

    for dir in [
        &empty_cwd,
        &scratch_cwd,
        &seed_cache,
        &empty_tmp,
        &clean_cache,
        &clean_tmp,
        &poison_env_cache,
        &poison_tmp_cache,
        &map_cfg_cache,
        &map_cfg_tmp,
    ] {
        std::fs::create_dir_all(dir).expect("create scratch dir");
    }
    std::fs::write(
        scratch_cwd.join("map_config.json"),
        serde_json::to_string_pretty(&modified_config).expect("serialize MapConfig"),
    )
    .expect("write modified map_config.json");

    // 1. Let a clean app init_world() child write the real cache filename for MapConfig::default(),
    //    then build a valid decoded TerrainMap payload with poisoned biomes/elevations.
    run_child_probe(
        "seed_app_cache",
        &empty_cwd,
        Some(&seed_cache),
        &empty_tmp,
        None,
    );
    let cache_entries: Vec<_> = std::fs::read_dir(&seed_cache)
        .expect("read seed_cache")
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let s = name.to_string_lossy();
            s.starts_with("world_") && s.ends_with(".bin")
        })
        .collect();
    assert_eq!(
        cache_entries.len(),
        1,
        "seed_app_cache should produce exactly one world_*.bin cache file"
    );
    let cache_filename = cache_entries[0].file_name();
    let raw_cache = std::fs::read(cache_entries[0].path()).expect("read seeded cache");
    let (mut poisoned_map, _): (TerrainMap, usize) =
        bincode::serde::decode_from_slice(&raw_cache, bincode::config::standard())
            .expect("decode seeded cache");
    poisoned_map.elevations.fill(0.5);
    poisoned_map.biomes.fill(BiomeType::Desert as u8);
    let poisoned_id =
        WorldIdentity::from_terrain(&poisoned_map, settings.seed, WORLD_CACHE_VERSION);
    assert_ne!(poisoned_id, expected_id);
    assert_ne!(poisoned_id, modified_id);
    let poisoned_bytes = bincode::serde::encode_to_vec(&poisoned_map, bincode::config::standard())
        .expect("encode poisoned cache");
    std::fs::write(poison_env_cache.join(&cache_filename), &poisoned_bytes)
        .expect("write ANIMA_CACHE_DIR poison");
    std::fs::write(poison_tmp_cache.join(&cache_filename), &poisoned_bytes)
        .expect("write <temp_dir>/anima_world_cache poison");

    // 2. Clean headless-only child: must match expected_id and never write to ANIMA_CACHE_DIR or
    //    <temp_dir>/anima_world_cache.
    let clean = run_child_probe(
        "headless_only",
        &empty_cwd,
        Some(&clean_cache),
        &clean_tmp,
        None,
    );
    let clean_final = clean
        .headless_final_checksum
        .expect("clean child headless checksum");
    assert_eq!(
        clean.headless_world_id_checksum,
        Some(expected_id.checksum),
        "clean headless run must report default WorldIdentity"
    );
    assert!(
        std::fs::read_dir(&clean_cache)
            .expect("read clean_cache")
            .next()
            .is_none(),
        "Generated must never write into ANIMA_CACHE_DIR"
    );
    assert!(
        !clean_tmp.join("anima_world_cache").exists(),
        "Generated must never create <temp_dir>/anima_world_cache"
    );

    let expected_c01_checksum = app_world_fingerprint().0.checksum;

    // 3. Poisoned ANIMA_CACHE_DIR: headless ignores it; desktop init_world() boots C01 cleanly.
    let env_poison_res = run_child_probe(
        "headless_then_app",
        &empty_cwd,
        Some(&poison_env_cache),
        &empty_tmp,
        None,
    );
    assert_eq!(
        env_poison_res.headless_final_checksum,
        Some(clean_final),
        "headless run must ignore poisoned ANIMA_CACHE_DIR"
    );
    assert_eq!(
        env_poison_res.headless_world_id_checksum,
        Some(expected_id.checksum),
        "headless world_identity must ignore poisoned ANIMA_CACHE_DIR"
    );
    assert_eq!(
        env_poison_res.app_world_id_checksum,
        Some(expected_c01_checksum),
        "WV2-7 desktop init_world() must ignore poisoned ANIMA_CACHE_DIR and boot C01"
    );

    // 4. Poisoned <temp_dir>/anima_world_cache (with ANIMA_CACHE_DIR unset): headless ignores it;
    //    desktop init_world() boots C01 cleanly.
    let temp_poison_res = run_child_probe(
        "headless_then_app",
        &empty_cwd,
        None,
        &poison_tmp_root,
        None,
    );
    assert_eq!(
        temp_poison_res.headless_final_checksum,
        Some(clean_final),
        "headless run must ignore poisoned <temp_dir>/anima_world_cache"
    );
    assert_eq!(
        temp_poison_res.headless_world_id_checksum,
        Some(expected_id.checksum),
        "headless world_identity must ignore poisoned <temp_dir>/anima_world_cache"
    );
    assert_eq!(
        temp_poison_res.app_world_id_checksum,
        Some(expected_c01_checksum),
        "WV2-7 desktop init_world() must ignore poisoned <temp_dir>/anima_world_cache and boot C01"
    );

    // 5. Modified map_config.json in scratch working directory: headless ignores it; desktop
    //    init_world() boots C01 cleanly without reading CWD map_config.json.
    let map_cfg_res = run_child_probe(
        "headless_then_app",
        &empty_cwd,
        Some(&map_cfg_cache),
        &map_cfg_tmp,
        Some(&scratch_cwd),
    );
    assert_eq!(
        map_cfg_res.headless_final_checksum,
        Some(clean_final),
        "headless run must ignore CWD map_config.json"
    );
    assert_eq!(
        map_cfg_res.headless_world_id_checksum,
        Some(expected_id.checksum),
        "headless world_identity must ignore CWD map_config.json"
    );
    assert_eq!(
        map_cfg_res.app_world_id_checksum,
        Some(expected_c01_checksum),
        "WV2-7 desktop init_world() must ignore CWD map_config.json and boot C01"
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
