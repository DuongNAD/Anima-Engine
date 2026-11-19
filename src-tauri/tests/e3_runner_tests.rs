//! Integration tests for `src-tauri/examples/run_e3_experiment.rs` (`CARD E3-X-impl`).
//!
//! Uses only disjoint synthetic test seeds (`612_001`, `612_002`) checked absent from every
//! committed fixture — NEVER runs any E3 preregistered seed (`730001..741001`) or `smoke_seed`
//! (`999979`), and never mutates process-global environment variables.

use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, WorldLawSet,
    MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::intervention::{Curve, InterventionCommand, InterventionKind, Region};
use anima_engine_lib::core::live_experiment::{
    all_live_observable_ids, LIVE_KEY_RESOURCE_CAPACITY_SCALE,
};
use anima_engine_lib::core::sha256::sha256_file;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[path = "../examples/run_e3_experiment.rs"]
mod runner;

const TEST_SEEDS: [u64; 2] = [612_001, 612_002];
const PREREG_REL: &str = "tests/fixtures/experiments_e3/e3-preregistration.json";

fn collect_json_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_json_files(&p, out);
            } else if p.extension().and_then(|s| s.to_str()) == Some("json") {
                out.push(p);
            }
        }
    }
}

fn synthetic_manifest(
    id: &str,
    scale: f64,
    deforest: f32,
    nutrient: f32,
    nut_min_x: u32,
) -> ExperimentManifest {
    let all_obs: Vec<String> = all_live_observable_ids()
        .into_iter()
        .map(str::to_string)
        .collect();
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: id.into(),
        name: id.into(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: InitialConditionSet::new(vec![
            ("live.ecological_fitness".into(), 1.0),
            ("live.mortality".into(), 1.0),
            ("live.reproduction".into(), 1.0),
            ("live.locomotion".into(), 1.0),
            ("live.drive_floor".into(), 0.5),
            ("live.thirst_steering".into(), 1.0),
            ("live.evolved_brains".into(), 0.0),
            ("live.founders".into(), 8.0),
            ("live.predator_fraction".into(), 0.3),
            ("live.trees".into(), 8.0),
            ("live.lakes".into(), 2.0),
            ("live.food_cap".into(), 50.0),
            (LIVE_KEY_RESOURCE_CAPACITY_SCALE.into(), scale),
        ]),
        interventions: vec![
            InterventionCommand {
                id: 1,
                cause_id: 1,
                kind: InterventionKind::Deforest,
                region: Region::Rect {
                    min_x: 0,
                    min_y: 0,
                    max_x: 127,
                    max_y: 255,
                },
                intensity: deforest,
                start_tick: 1,
                duration_ticks: 60,
                curve: Curve::Step,
                reversible: false,
                signed_negative: false,
            },
            InterventionCommand {
                id: 2,
                cause_id: 2,
                kind: InterventionKind::AddNutrient,
                region: Region::Rect {
                    min_x: nut_min_x,
                    min_y: 0,
                    max_x: 255,
                    max_y: 255,
                },
                intensity: nutrient,
                start_tick: 1,
                duration_ticks: 60,
                curve: Curve::Step,
                reversible: false,
                signed_negative: false,
            },
        ],
        seeds: TEST_SEEDS.to_vec(),
        duration_ticks: 60,
        sample_period: 20,
        observable_ids: all_obs,
        exotic_interventions: Vec::new(),
        observer: Default::default(),
    }
}

struct TestWorkspace {
    root: PathBuf,
    manifest_dir: PathBuf,
    smoke_dir: PathBuf,
    temp_clean: PathBuf,
}

impl TestWorkspace {
    fn new(tag: &str, commit: &str, host_os: &str, host_arch: &str) -> Self {
        let root = std::env::temp_dir().join(format!("e3x-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let manifest_dir = root.join("manifests");
        let smoke_dir = root.join("smoke");
        let temp_clean = root.join("clean_tmp");
        std::fs::create_dir_all(&manifest_dir).unwrap();
        std::fs::create_dir_all(&smoke_dir).unwrap();
        std::fs::create_dir_all(&temp_clean).unwrap();

        let prereg_src = Path::new(env!("CARGO_MANIFEST_DIR")).join(PREREG_REL);
        let mut prereg: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&prereg_src).unwrap()).unwrap();
        prereg["seeds"]["execution_order"] = serde_json::json!(TEST_SEEDS);
        prereg["duration"]["duration_ticks"] = serde_json::json!(60u64);
        prereg["duration"]["sample_period"] = serde_json::json!(20u64);
        prereg["duration"]["warmup_ticks"] = serde_json::json!(20u64);
        prereg["duration"]["tail_window_ticks"] = serde_json::json!([40u64, 60u64]);
        prereg["wp2_smoke_calibration"]["parent_commit"] = serde_json::json!(commit);
        std::fs::write(
            manifest_dir.join("e3-preregistration.json"),
            serde_json::to_string_pretty(&prereg).unwrap(),
        )
        .unwrap();

        let c_file = prereg["manifests"]["control"].as_str().unwrap().to_string();
        let s_file = prereg["manifests"]["control_uniform_scarcity"]
            .as_str()
            .unwrap()
            .to_string();
        let t_file = prereg["manifests"]["treatment"]
            .as_str()
            .unwrap()
            .to_string();

        let m_c = synthetic_manifest("e3::test_ctrl", 1.0, 0.0, 0.0, 128);
        let m_s = synthetic_manifest("e3::test_scarce", 0.42, 0.0, 1.0, 0);
        let m_t = synthetic_manifest("e3::test_treat", 1.0, 0.3, 1.0, 128);
        for (fname, m) in [(&c_file, &m_c), (&s_file, &m_s), (&t_file, &m_t)] {
            std::fs::write(
                manifest_dir.join(fname),
                serde_json::to_string_pretty(m).unwrap(),
            )
            .unwrap();
        }

        // Determine actual observed WorldIdentity from one seed of the real LiveExperimentAdapter
        // terrain without hard-coding a platform checksum.
        let reg = ObservableRegistry::live_default();
        let probe = anima_engine_lib::core::experiment_runner::run_manifest_seed::<
            anima_engine_lib::core::live_experiment::LiveExperimentAdapter,
        >(&m_c, &reg, TEST_SEEDS[0], None, None);
        let wid = probe.provenance.world_identity.unwrap();

        std::fs::write(
            smoke_dir.join("provenance.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "build": { "git_commit": commit, "profile": "release" },
                "host": { "os": host_os, "arch": host_arch },
                "world_identity_observed": wid,
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            smoke_dir.join("smoke-report.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "all_observables_emitted_and_finite": true,
                "wp2_a_resource_field": { "passed": true },
                "wp2_b_standing_crop_intervention": { "passed": true },
                "wp2_c_detritus_before_3000": { "passed": true },
                "integrity": { "world_identity_identical": true },
                "wp3_schedule_lock": { "within_budget": true },
            }))
            .unwrap(),
        )
        .unwrap();
        runner::write_checksums(&smoke_dir, &["provenance.json", "smoke-report.json"]).unwrap();

        Self {
            root,
            manifest_dir,
            smoke_dir,
            temp_clean,
        }
    }

    fn base_env(&self, commit: &str, host_os: &str, host_arch: &str) -> runner::RuntimeEnv {
        runner::RuntimeEnv {
            anima_world_artifact: None,
            temp_dir: self.temp_clean.clone(),
            is_debug_build: false,
            host_os: host_os.to_string(),
            host_arch: host_arch.to_string(),
            git_commit_override: Some(commit.to_string()),
            git_dirty_paths_override: Some(Vec::new()),
            world_identity_override: None,
        }
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn mismatch_second_run(idx: usize, _arm: &str, _seed: u64, mut id: WorldIdentity) -> WorldIdentity {
    if idx == 1 {
        id.checksum = id.checksum.wrapping_add(7);
    }
    id
}

#[test]
fn test_seeds_are_disjoint_from_every_committed_fixture() {
    let fixtures_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut files = Vec::new();
    collect_json_files(&fixtures_root, &mut files);
    assert!(!files.is_empty());

    let reserved: BTreeSet<String> = TEST_SEEDS.iter().map(ToString::to_string).collect();
    for file in files {
        let raw = std::fs::read_to_string(&file).unwrap();
        for tok in &reserved {
            assert!(
                !raw.contains(tok),
                "test seed {tok} collides with committed fixture {}",
                file.display()
            );
        }
    }
}

#[test]
fn refuses_debug_build_and_invalid_cli_arguments() {
    let ws = TestWorkspace::new("refuse-cli", "commit-a", "windows", "x86_64");
    let env = ws.base_env("commit-a", "windows", "x86_64");

    assert!(runner::run_with_config(vec![], env.clone()).is_err());
    assert!(
        runner::run_with_config(vec!["--ensemble".into(), "--replay".into()], env.clone()).is_err()
    );
    assert!(
        runner::run_with_config(vec!["--ensemble".into(), "--unknown".into()], env.clone())
            .is_err()
    );
    assert!(runner::run_with_config(vec!["--replay".into()], env.clone()).is_err());
    assert!(runner::run_with_config(
        vec![
            "--ensemble".into(),
            "--arm".into(),
            "control_uniform".into(),
        ],
        env.clone()
    )
    .is_err());

    let mut debug_env = env;
    debug_env.is_debug_build = true;
    let err_ens = runner::run_with_config(
        vec![
            "--ensemble".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--smoke-dir".into(),
            ws.smoke_dir.display().to_string(),
            "--out".into(),
            ws.root.join("out_dbg_ens").display().to_string(),
        ],
        debug_env.clone(),
    )
    .unwrap_err();
    assert!(err_ens.contains("--release"));

    let err_rep = runner::run_with_config(
        vec![
            "--replay".into(),
            "--arm".into(),
            "control_uniform".into(),
            "--seed".into(),
            TEST_SEEDS[0].to_string(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--out".into(),
            ws.root.join("out_dbg_rep").display().to_string(),
        ],
        debug_env,
    )
    .unwrap_err();
    assert!(err_rep.contains("--release"));
}

#[test]
fn refuses_world_artifact_prerun_nonempty_out_and_missing_or_mismatched_host_smoke() {
    let ws = TestWorkspace::new("refuse-pre", "commit-cur", "windows", "x86_64");
    let base = ws.base_env("commit-cur", "windows", "x86_64");

    // 1. ANIMA_WORLD_ARTIFACT env set -> refused.
    let mut env_art = base.clone();
    env_art.anima_world_artifact = Some("C:/fake/anima_world_artifact.anmw".into());
    let err1 = runner::run_with_config(
        vec![
            "--ensemble".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--smoke-dir".into(),
            ws.smoke_dir.display().to_string(),
            "--out".into(),
            ws.root.join("out_art_env").display().to_string(),
        ],
        env_art,
    )
    .unwrap_err();
    assert!(err1.contains("ANIMA_WORLD_ARTIFACT"));

    // 2. Temp file <temp_dir>/anima_world_artifact.anmw present -> refused.
    let bad_tmp = ws.root.join("bad_tmp");
    std::fs::create_dir_all(&bad_tmp).unwrap();
    std::fs::write(bad_tmp.join("anima_world_artifact.anmw"), b"fake").unwrap();
    let mut env_tmp = base.clone();
    env_tmp.temp_dir = bad_tmp;
    let err2 = runner::run_with_config(
        vec![
            "--ensemble".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--smoke-dir".into(),
            ws.smoke_dir.display().to_string(),
            "--out".into(),
            ws.root.join("out_art_file").display().to_string(),
        ],
        env_tmp,
    )
    .unwrap_err();
    assert!(err2.contains("anima_world_artifact.anmw"));

    // 3. Non-empty --out -> refused.
    let non_empty = ws.root.join("non_empty");
    std::fs::create_dir_all(&non_empty).unwrap();
    std::fs::write(non_empty.join("stale.txt"), b"stale").unwrap();
    let err3 = runner::run_with_config(
        vec![
            "--ensemble".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--smoke-dir".into(),
            ws.smoke_dir.display().to_string(),
            "--out".into(),
            non_empty.display().to_string(),
        ],
        base.clone(),
    )
    .unwrap_err();
    assert!(err3.contains("not empty"));

    // 4. Host OS/arch differs from recorded smoke host with no matching smoke -> refused.
    let err4 = runner::run_with_config(
        vec![
            "--ensemble".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--smoke-dir".into(),
            ws.smoke_dir.display().to_string(),
            "--out".into(),
            ws.root.join("out_bad_host").display().to_string(),
        ],
        ws.base_env("commit-cur", "macos", "aarch64"),
    )
    .unwrap_err();
    assert!(err4.contains("host"));

    // 5. Commit differs from smoke provenance commit -> refused.
    let err5 = runner::run_with_config(
        vec![
            "--ensemble".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--smoke-dir".into(),
            ws.smoke_dir.display().to_string(),
            "--out".into(),
            ws.root.join("out_bad_commit").display().to_string(),
        ],
        ws.base_env("different-commit", "windows", "x86_64"),
    )
    .unwrap_err();
    assert!(err5.contains("git_commit"));

    // 6. --replay rejects undeclared seed and invalid arm.
    let err6 = runner::run_with_config(
        vec![
            "--replay".into(),
            "--arm".into(),
            "control_uniform".into(),
            "--seed".into(),
            "999979".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--out".into(),
            ws.root.join("out_bad_replay_seed").display().to_string(),
        ],
        base.clone(),
    )
    .unwrap_err();
    assert!(err6.contains("execution_order"));
}

#[test]
fn stops_immediately_and_records_stop_when_world_identity_differs() {
    let ws = TestWorkspace::new("wid-stop", "commit-stop", "windows", "x86_64");
    let mut env = ws.base_env("commit-stop", "windows", "x86_64");
    env.world_identity_override = Some(mismatch_second_run);

    let out = ws.root.join("out_stopped");
    let err = runner::run_with_config(
        vec![
            "--ensemble".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--smoke-dir".into(),
            ws.smoke_dir.display().to_string(),
            "--out".into(),
            out.display().to_string(),
        ],
        env,
    )
    .unwrap_err();
    assert!(
        err.contains("world_identity_mismatch"),
        "unexpected err: {err}"
    );

    let prov: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("provenance.json")).unwrap())
            .unwrap();
    assert_eq!(prov["stopped_early"].as_bool(), Some(true));
    assert_eq!(prov["runs_completed_before_stop"].as_u64(), Some(2));
    assert!(prov["stop_reason"]
        .as_str()
        .unwrap()
        .contains("world_identity_mismatch"));

    let runs_lines: Vec<String> = std::fs::read_to_string(out.join("runs.csv"))
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    // Header + 2 runs (halted at run index 1, never executed the remaining 4 runs of the 2x3 ensemble).
    assert_eq!(runs_lines.len(), 3);
}

#[test]
fn short_synthetic_ensemble_and_replay_write_complete_verified_artifacts() {
    let ws = TestWorkspace::new("short-e2e", "commit-e2e", "windows", "x86_64");
    let mut env = ws.base_env("commit-e2e", "windows", "x86_64");
    env.git_dirty_paths_override = Some(vec![
        "artifacts/experiments/e3-productivity-gradient-body-mass/host-smoke/".into(),
    ]);

    let out = ws.root.join("confirmation");
    runner::run_with_config(
        vec![
            "--ensemble".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--smoke-dir".into(),
            ws.smoke_dir.display().to_string(),
            "--out".into(),
            out.display().to_string(),
        ],
        env.clone(),
    )
    .unwrap();

    for req in [
        "runs.csv",
        "series.csv",
        "final-observables.json",
        "effects.json",
        "provenance.json",
        "summary.md",
        "timing.json",
        "checksums.sha256",
    ] {
        assert!(out.join(req).exists(), "missing output file {req}");
    }

    let prov: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("provenance.json")).unwrap())
            .unwrap();
    assert_eq!(prov["stopped_early"].as_bool(), Some(false));
    assert_eq!(prov["terrain_source"].as_str(), Some("generated"));
    assert_eq!(
        prov["world_identity_all_runs_identical"].as_bool(),
        Some(true)
    );
    assert_eq!(
        prov["build"]["git_dirty_paths"].as_array().unwrap().len(),
        1
    );
    assert_eq!(prov["build"]["profile"].as_str(), Some("release"));
    assert_eq!(
        prov["world_artifact_prerun_check"]["anima_world_artifact_unset"].as_bool(),
        Some(true)
    );
    assert_eq!(
        prov["world_artifact_prerun_check"]["temp_artifact_absent"].as_bool(),
        Some(true)
    );
    assert_eq!(
        prov["per_run_world_provenance"].as_array().unwrap().len(),
        6
    );
    assert!(prov["smoke_reference"]["wp2_passed"].as_bool() == Some(true));

    // Independently check checksums.sha256 covers every deterministic file and excludes timing.json.
    let csum_text = std::fs::read_to_string(out.join("checksums.sha256")).unwrap();
    assert!(!csum_text.contains("timing.json"));
    for line in csum_text.lines() {
        let mut parts = line.split("  ");
        let hex = parts.next().unwrap();
        let rel = parts.next().unwrap();
        let actual = sha256_file(&out.join(rel)).unwrap();
        assert_eq!(actual, hex, "digest mismatch for {rel}");
    }

    // Verify effects.json has < 10 valid pairs on the 2-seed fixture -> insufficient_evidence.
    let effects: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("effects.json")).unwrap()).unwrap();
    assert_eq!(
        effects["primary_decision"].as_str(),
        Some("insufficient_evidence")
    );
    assert_eq!(
        effects["contrasts"]["treatment_gradient - control_uniform_scarcity"]["H1"]
            ["n_complete_pairs"]
            .as_u64(),
        Some(2)
    );

    // Cross-language check: execute the independent Node verifier against the synthetic output.
    let verifier_script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("scripts/verify_e3_artifacts.mjs");
    let verifier_out = std::process::Command::new("node")
        .arg(&verifier_script)
        .arg(&out)
        .arg("--prereg")
        .arg(ws.manifest_dir.join("e3-preregistration.json"))
        .arg("--skip-committed-manifests")
        .output()
        .expect("invoke node verify_e3_artifacts.mjs");
    assert!(
        verifier_out.status.success(),
        "verify_e3_artifacts.mjs rejected runner output:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&verifier_out.stdout),
        String::from_utf8_lossy(&verifier_out.stderr)
    );

    // Verify --replay matches checksum of (seed 612002, treatment_gradient) in runs.csv.
    let replay_out = ws.root.join("replay");
    runner::run_with_config(
        vec![
            "--replay".into(),
            "--arm".into(),
            "treatment_gradient".into(),
            "--seed".into(),
            TEST_SEEDS[1].to_string(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--out".into(),
            replay_out.display().to_string(),
        ],
        env,
    )
    .unwrap();
    let replay_doc: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(replay_out.join("replay-treatment_gradient-seed-612002-t60.json"))
            .unwrap(),
    )
    .unwrap();
    let replay_cksum = replay_doc["final_checksum"].as_u64().unwrap().to_string();
    let runs_csv = std::fs::read_to_string(out.join("runs.csv")).unwrap();
    assert!(
        runs_csv.contains(&format!(
            "612002,treatment_gradient,e3::test_treat,completed,,,{replay_cksum},"
        )),
        "replay checksum did not match ensemble runs.csv"
    );
}

#[test]
fn runner_self_test_mode_passes() {
    let env = runner::RuntimeEnv::from_process();
    runner::run_with_config(vec!["--self-test".into()], env).unwrap();
}

#[test]
fn refuses_host_smoke_with_missing_or_tampered_checksums_sha256() {
    let ws = TestWorkspace::new("smoke-cksum", "commit-ck", "windows", "x86_64");
    let env = ws.base_env("commit-ck", "windows", "x86_64");

    // 1. Missing smoke checksums.sha256 -> refused before any tick runs.
    let cksum_path = ws.smoke_dir.join("checksums.sha256");
    std::fs::remove_file(&cksum_path).unwrap();
    let err_missing = runner::run_with_config(
        vec![
            "--ensemble".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--smoke-dir".into(),
            ws.smoke_dir.display().to_string(),
            "--out".into(),
            ws.root.join("out_no_cksum").display().to_string(),
        ],
        env.clone(),
    )
    .unwrap_err();
    assert!(
        err_missing.contains("checksums.sha256"),
        "unexpected err: {err_missing}"
    );

    // 2. Tampered digest inside smoke checksums.sha256 -> refused.
    std::fs::write(
        &cksum_path,
        format!(
            "{0}  provenance.json\n{0}  smoke-report.json\n",
            "0".repeat(64)
        ),
    )
    .unwrap();
    let err_bad = runner::run_with_config(
        vec![
            "--ensemble".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--smoke-dir".into(),
            ws.smoke_dir.display().to_string(),
            "--out".into(),
            ws.root.join("out_bad_cksum").display().to_string(),
        ],
        env,
    )
    .unwrap_err();
    assert!(
        err_bad.contains("checksum mismatch"),
        "unexpected err: {err_bad}"
    );
}

#[test]
fn identical_non_integer_deltas_clamp_sd_to_zero_and_classify_below_materiality() {
    let prereg_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/experiments_e3");
    let thresholds = runner::load_thresholds(&prereg_dir).unwrap();

    for (ctrl, treat) in [(3.0_f64, 1.1_f64), (1.0_f64, 1.1_f64)] {
        let rows: Vec<runner::SeedDeltaRow> = (0..12_u64)
            .map(|i| runner::SeedDeltaRow {
                seed: 612_001 + i,
                control: ctrl,
                treatment: treat,
                delta: treat - ctrl,
            })
            .collect();
        let report = runner::build_observable_report(
            "live.trait_mass_var",
            12,
            &rows,
            0,
            0,
            0,
            true,
            &thresholds,
            false,
        )
        .unwrap();
        assert_eq!(report.effect.paired_sd, Some(0.0));
        assert_eq!(report.effect.paired_se, Some(0.0));
        assert_eq!(report.effect.paired_dz, None);
        assert_eq!(report.between_seed_variance_of_deltas, Some(0.0));
        assert_eq!(report.effect.ci95_low, report.effect.ci95_high);
        assert_eq!(
            report.decision.as_deref(),
            Some("between_detectable_below_materiality")
        );
    }
}
