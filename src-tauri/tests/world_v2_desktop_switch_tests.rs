//! Card WV2-7 integration tests: desktop switchover onto bundled `artifacts/world-v2/sim_256.anmw3`,
//! SHA-256 verification and loud error paths, legacy v2 fixture decoding, mismatched save refusal
//! (`WorldIdentity::mismatch_against` precheck and `SimulationEngine::start` backstop), v3-only
//! lake-threshold audit, and `TerrainSource::Generated` isolation.

use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};
use anima_engine_lib::core::ecs::{
    init_world, init_world_with, try_init_world_with_bundled_path, TerrainSource, WorldV3SeaLevel,
};
use anima_engine_lib::core::simulation_loop::{is_non_land_nav_lake_candidate, SimulationEngine};
use anima_engine_lib::core::snapshot::{read_with_warnings, SnapshotEnvelope};
use anima_engine_lib::core::terrain::MapSettings;
use anima_engine_lib::core::world_artifact::{
    active_desktop_world_identity, load_desktop_world_with_bundled_path,
    load_verified_v3_sim_from_paths, terrain_map_from_v3_sim, try_world_seed_from_disk,
    try_world_seed_from_disk_with_bundled_path, WorldIdentity,
};
use anima_worldgen::anmw_v3::AnmwV3Sim;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

static ENV_LOCK: Mutex<()> = Mutex::new(());

const ENV_VARS: [&str; 5] = [
    "ANIMA_WORLD_ARTIFACT",
    "ANIMA_USE_GPU",
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

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR has repo parent")
        .to_path_buf()
}

fn expected_c01_sha256() -> [u8; 32] {
    let idx_path = repo_root().join("artifacts/world-v2/index.json");
    let raw = std::fs::read_to_string(&idx_path).expect("read artifacts/world-v2/index.json");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse index.json");
    let hex = parsed["sim_world_identity"]
        .as_str()
        .expect("sim_world_identity string");
    anima_worldgen::anmw_v3::sha256_from_hex(hex).expect("valid sha256 hex")
}

fn set_temp_root(dir: &Path) {
    std::env::set_var("TMPDIR", dir);
    std::env::set_var("TEMP", dir);
    std::env::set_var("TMP", dir);
}

fn shared_handles() -> (
    Arc<Mutex<EvolutionSettings>>,
    Arc<AtomicBool>,
    Arc<Mutex<MapElitesGridState>>,
) {
    (
        Arc::new(Mutex::new(EvolutionSettings {
            mutation_rate: 0.15,
            selection_bias: 1.5,
            grid_resolution: 50,
        })),
        Arc::new(AtomicBool::new(false)),
        Arc::new(Mutex::new(MapElitesGridState {
            grid: std::collections::HashMap::new(),
            grid_resolution: 50,
        })),
    )
}

#[test]
fn wv2_7_desktop_boot_precedence_uses_bundled_then_dev_and_retires_v1_temp_artifact() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _guard = EnvGuard::capture();
    std::env::remove_var("ANIMA_WORLD_ARTIFACT");

    let scratch =
        std::env::temp_dir().join(format!("anima_wv2_7_precedence_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let temp_root = scratch.join("tmp");
    let bundle_dir = scratch.join("bundle/world-v2");
    std::fs::create_dir_all(&temp_root).expect("mkdir tmp");
    std::fs::create_dir_all(&bundle_dir).expect("mkdir bundle/world-v2");

    // Place a valid v2 artifact at the retired <temp_dir>/anima_world_artifact.anmw location.
    let v2_legacy = repo_root().join("artifacts/world_256.anmw");
    std::fs::copy(&v2_legacy, temp_root.join("anima_world_artifact.anmw"))
        .expect("copy retired v1 temp artifact");
    set_temp_root(&temp_root);

    let c01_sha = expected_c01_sha256();
    let dev_world = init_world();
    let dev_id = *dev_world.resource::<WorldIdentity>();
    assert_eq!(dev_id.sim_world_sha256, Some(c01_sha));
    let sea = dev_world
        .get_resource::<WorldV3SeaLevel>()
        .expect("desktop ANMW v3 boot inserts WorldV3SeaLevel");
    assert!((sea.0 - anima_worldgen::SEA_LEVEL_NORM).abs() < 1e-6);
    assert_eq!(
        try_world_seed_from_disk().expect("seed resolves from C01"),
        dev_id.seed
    );

    // Bundled resource path takes precedence over dev artifacts when present on disk.
    let sim_src = repo_root().join("artifacts/world-v2/sim_256.anmw3");
    let idx_src = repo_root().join("artifacts/world-v2/index.json");
    let bundled_sim = bundle_dir.join("sim_256.anmw3");
    std::fs::copy(&sim_src, &bundled_sim).expect("copy sim_256.anmw3");
    std::fs::copy(&idx_src, bundle_dir.join("index.json")).expect("copy index.json");

    let bundled_world =
        try_init_world_with_bundled_path(TerrainSource::AppArtifactOrGenerated, Some(&bundled_sim))
            .expect("bundled sim_256.anmw3 boots cleanly");
    assert_eq!(*bundled_world.resource::<WorldIdentity>(), dev_id);
    assert_eq!(
        try_world_seed_from_disk_with_bundled_path(Some(&bundled_sim)).expect("bundled seed"),
        dev_id.seed
    );

    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn wv2_7_negative_control_a_missing_corrupt_or_tampered_v3_fails_loudly_without_fallback() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _guard = EnvGuard::capture();

    let scratch = std::env::temp_dir().join(format!("anima_wv2_7_neg_a_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).expect("mkdir scratch");

    // 1. Missing v3 file fails loudly.
    let missing_sim = scratch.join("missing.anmw3");
    let real_idx = repo_root().join("artifacts/world-v2/index.json");
    let missing_err =
        load_verified_v3_sim_from_paths(&missing_sim, &real_idx).expect_err("missing must fail");
    assert!(
        missing_err.contains("ANMW v3"),
        "missing error must name ANMW v3: {missing_err}"
    );

    // 2. One-byte tamper in bundled sim_256.anmw3 fails SHA-256 verification without dev fallback.
    let real_sim = repo_root().join("artifacts/world-v2/sim_256.anmw3");
    let mut tampered = std::fs::read(&real_sim).expect("read sim_256.anmw3");
    let mid = tampered.len() / 2;
    tampered[mid] ^= 0x55;
    let tampered_sim = scratch.join("sim_256.anmw3");
    let copied_idx = scratch.join("index.json");
    std::fs::write(&tampered_sim, &tampered).expect("write tampered sim");
    std::fs::copy(&real_idx, &copied_idx).expect("copy index.json");

    std::env::remove_var("ANIMA_WORLD_ARTIFACT");
    let bundled_err =
        load_desktop_world_with_bundled_path(Some(&tampered_sim), &MapSettings::default())
            .expect_err("tampered bundled sim must fail SHA-256 check");
    assert!(
        bundled_err.contains("ANMW v3") && bundled_err.contains("mismatch"),
        "unexpected tampered bundled error: {bundled_err}"
    );
    let init_err = try_init_world_with_bundled_path(
        TerrainSource::AppArtifactOrGenerated,
        Some(&tampered_sim),
    )
    .expect_err("try_init_world must not fall back on tampered bundled world");
    assert!(init_err.contains("ANMW v3"));

    // 3. Explicit ANIMA_WORLD_ARTIFACT pointing at missing, corrupt, or tampered v3 fails loudly,
    //    and SimulationEngine::start spawns zero workers with running=false.
    std::env::set_var("ANIMA_WORLD_ARTIFACT", &missing_sim);
    assert!(load_desktop_world_with_bundled_path(None, &MapSettings::default()).is_err());

    let corrupt_path = scratch.join("corrupt.anmw");
    std::fs::write(&corrupt_path, b"not-anmw-corrupt").expect("write corrupt");
    std::env::set_var("ANIMA_WORLD_ARTIFACT", &corrupt_path);
    assert!(load_desktop_world_with_bundled_path(None, &MapSettings::default()).is_err());

    std::env::set_var("ANIMA_WORLD_ARTIFACT", &tampered_sim);
    let seed_err = try_world_seed_from_disk().expect_err("tampered env v3 seed fails loudly");
    assert!(seed_err.contains("ANMW v3"));
    let engine = SimulationEngine::new();
    let (evo_settings, evo_running, map_grid) = shared_handles();
    engine.start::<tauri::test::MockRuntime>(None, evo_settings, evo_running, map_grid);
    assert!(
        !engine.running.load(Ordering::SeqCst),
        "aborted start on tampered ANMW v3 must reset running=false"
    );
    assert!(
        engine.threads.lock().unwrap().is_none(),
        "aborted start on tampered ANMW v3 must spawn 0 workers"
    );

    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn wv2_7_negative_control_b_legacy_v2_fixtures_continue_decoding() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _guard = EnvGuard::capture();

    for rel in [
        "src-tauri/tests/fixtures/world_real_128.anmw",
        "src-tauri/tests/fixtures/world_4x4.anmw",
    ] {
        let fixture = repo_root().join(rel);
        std::env::set_var("ANIMA_WORLD_ARTIFACT", &fixture);
        let world = init_world();
        let identity = *world.resource::<WorldIdentity>();
        assert_eq!(
            identity.sim_world_sha256, None,
            "{rel} keeps sim_world_sha256=None"
        );
        assert!(
            world.get_resource::<WorldV3SeaLevel>().is_none(),
            "{rel} must not insert WorldV3SeaLevel"
        );
        assert!(try_world_seed_from_disk().is_ok(), "{rel} seed decodes");
    }
}

#[test]
fn wv2_7_old_saves_refused_by_mismatch_precheck_and_engine_start_backstop() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _guard = EnvGuard::capture();
    std::env::remove_var("ANIMA_WORLD_ARTIFACT");
    std::env::set_var("ANIMA_USE_GPU", "0");

    let fixture_path =
        repo_root().join("src-tauri/tests/fixtures/snapshots/schema7_burn013_shared_learner.json");
    let raw_bytes = std::fs::read(&fixture_path).expect("schema-7 fixture exists");
    let envelope: SnapshotEnvelope = serde_json::from_slice(&raw_bytes).expect("parse envelope");
    envelope.verify().expect("fixture checksum verifies");
    let (migrated, _) = read_with_warnings(&fixture_path).expect("read migrated fixture");

    // 1. Pre-check via `WorldIdentity::mismatch_against` (`load_simulation_state` contract).
    let active_id = active_desktop_world_identity(None).expect("active C01 desktop world identity");
    let mismatch = migrated
        .world_identity
        .mismatch_against(&active_id)
        .expect("old v1 schema-7 save must mismatch C01 v3 desktop identity");
    assert!(
        mismatch.contains("sim_world_sha256"),
        "expected sim_world_sha256 in mismatch reason: {mismatch}"
    );

    // 2. Engine backstop: queueing a mismatched save into SimulationEngine::start refuses before
    //    spawning workers, clears the incompatible pending save, and keeps running=false.
    let engine = SimulationEngine::new();
    *engine.pending_load_state.lock().unwrap() = Some(migrated);
    let (evo_settings, evo_running, map_grid) = shared_handles();
    engine.start::<tauri::test::MockRuntime>(
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_grid),
    );
    assert!(
        !engine.running.load(Ordering::SeqCst),
        "engine backstop must refuse mismatched save with running=false"
    );
    assert!(
        engine.threads.lock().unwrap().is_none(),
        "engine backstop must spawn 0 worker threads on identity mismatch"
    );
    assert_eq!(engine.get_status().tick_count, 0);
    assert!(engine.run_seed.read().unwrap().is_none());
    assert!(engine.pending_load_state.lock().unwrap().is_none());

    // 3. Matching C01 v3 save round-trips and resumes cleanly on the switched-over desktop engine.
    engine.start::<tauri::test::MockRuntime>(
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_grid),
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    while (!engine.get_status().running || engine.get_status().tick_count < 2)
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(engine.get_status().tick_count >= 2);
    let (tx, rx) = std::sync::mpsc::channel();
    engine.save_request_tx.send(tx).expect("send save");
    let saved_v2 = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("save reply")
        .expect("C01 save succeeds");
    engine.stop();

    assert_eq!(saved_v2.world_identity, active_id);
    assert!(saved_v2
        .world_identity
        .mismatch_against(&active_id)
        .is_none());

    let resumed_floor = saved_v2.tick_count;
    let engine2 = SimulationEngine::new();
    *engine2.pending_load_state.lock().unwrap() = Some(saved_v2);
    engine2.start::<tauri::test::MockRuntime>(None, evo_settings, evo_running, map_grid);
    let resume_deadline = Instant::now() + Duration::from_secs(15);
    while (!engine2.get_status().running || engine2.get_status().tick_count <= resumed_floor)
        && Instant::now() < resume_deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    let resumed_ticks = engine2.get_status().tick_count;
    engine2.stop();
    assert!(
        resumed_ticks > resumed_floor,
        "matching C01 v3 save must resume cleanly past tick {resumed_floor}, got {resumed_ticks}"
    );
}

#[test]
fn wv2_7_lake_threshold_audit_on_c01_allows_alpine_lakes_and_keeps_v1_path_bit_identical() {
    let sim_path = repo_root().join("artifacts/world-v2/sim_256.anmw3");
    let sim_bytes = std::fs::read(&sim_path).expect("read sim_256.anmw3");
    let sim = AnmwV3Sim::from_bytes(&sim_bytes).expect("decode C01");
    let (tm, wid) = terrain_map_from_v3_sim(&sim, &sim_bytes).expect("project C01");
    assert_eq!(wid.sim_world_sha256, Some(expected_c01_sha256()));

    let mut v1_rule_count = 0usize;
    let mut v3_rule_count = 0usize;
    for i in 0..(tm.width * tm.height) {
        if is_non_land_nav_lake_candidate(tm.biomes[i], tm.elevations[i], None) {
            v1_rule_count += 1;
        }
        if is_non_land_nav_lake_candidate(tm.biomes[i], tm.elevations[i], Some(sim.sea_level_norm))
        {
            v3_rule_count += 1;
            // Every v3 candidate must be strictly above sea level and never DeepOcean.
            assert_ne!(tm.biomes[i], 0);
            assert!(tm.elevations[i] > sim.sea_level_norm + 0.001);
        }
    }
    // Before (v1 rule on MAP-D8 BIG_4096_v2): 31,038 cells matching `(Ocean/Sea/Marsh) && elev < 0.4`.
    // After (v3 rule on MAP-D8 BIG_4096_v2): 1,473 inland non-DeepOcean freshwater lake cells > sea_level_norm + 0.001.
    assert_eq!(v1_rule_count, 31_038);
    assert_eq!(v3_rule_count, 1_473);

    // High-elevation alpine lake (>= 0.4 / 1,380 m) is accepted under v3 and rejected under v1.
    assert!(is_non_land_nav_lake_candidate(
        1,
        0.72,
        Some(sim.sea_level_norm)
    ));
    assert!(!is_non_land_nav_lake_candidate(1, 0.72, None));
    assert!(!is_non_land_nav_lake_candidate(
        0,
        0.30,
        Some(sim.sea_level_norm)
    ));

    // Legacy / Generated (`None`) matches `(biome == 0 || biome == 1 || biome == 3) && elev < 0.4` identically.
    for biome in 0u8..=10 {
        for elev in [0.0_f32, 0.20, 0.3999, 0.4, 0.75, 1.0] {
            let expected_legacy = (biome == 0 || biome == 1 || biome == 3) && elev < 0.4;
            assert_eq!(
                is_non_land_nav_lake_candidate(biome, elev, None),
                expected_legacy,
                "legacy predicate drifted for biome={biome}, elev={elev}"
            );
        }
    }
}

#[test]
fn wv2_7_generated_source_stays_isolated_without_v3_sha_or_sea_level() {
    let world = init_world_with(TerrainSource::Generated);
    let identity = *world.resource::<WorldIdentity>();
    assert_eq!(identity.sim_world_sha256, None);
    assert_eq!(identity.seed, MapSettings::default().seed);
    assert!(world.get_resource::<WorldV3SeaLevel>().is_none());
}
