//! Integration tests for `src-tauri/examples/run_sel2p2_experiment.rs` (`Card SEL2-P2-RUNNER`).
//!
//! Uses only disjoint synthetic test seeds (`614_001`, `614_002`, baseline `614_101..=614_102`,
//! smoke `614_999`) verified absent from every committed fixture -- NEVER runs any preregistered
//! confirmation (`900001..=947001`), baseline (`970501..=970512`), or smoke (`999931`) seed, and
//! never writes under `artifacts/`.

use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, ObserverPolicy, WorldLawSet,
    MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::live_experiment::{
    all_live_observable_ids, LIVE_KEY_DEHYDRATION_TICKS, LIVE_KEY_DRIFT_CONTROL,
    LIVE_KEY_DRIFT_RESET_TICKS, LIVE_KEY_DRIVE_FLOOR, LIVE_KEY_ECOLOGICAL_FITNESS,
    LIVE_KEY_EVOLVED_BRAINS, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS, LIVE_KEY_FOUNDER_GENOTYPE,
    LIVE_KEY_FOUNDER_LAYOUT, LIVE_KEY_LAKES, LIVE_KEY_LAKE_LAYOUT, LIVE_KEY_LOCOMOTION,
    LIVE_KEY_MORTALITY, LIVE_KEY_MUTATION_RATE, LIVE_KEY_PLANT_RESERVE, LIVE_KEY_PREDATOR_FRACTION,
    LIVE_KEY_PREY_CAP, LIVE_KEY_REPRODUCTION, LIVE_KEY_RESOURCE_CAPACITY_SCALE,
    LIVE_KEY_STRICT_PREY_CAP, LIVE_KEY_THIRST_STEERING, LIVE_KEY_TREES,
};
use anima_engine_lib::core::world_artifact::WorldIdentity;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

#[allow(dead_code)]
#[path = "../examples/run_sel2p2_experiment.rs"]
mod runner;

const TEST_SEEDS: [u64; 2] = [614_001, 614_002];
const TEST_BASELINE_SEEDS: [u64; 2] = [614_101, 614_102];
const TEST_SMOKE_SEED: u64 = 614_999;

const SMOKE_NON_DRIFT_OBS: [&str; 9] = [
    "live.closed_eu_total",
    "live.agent_count",
    "live.standing_crop",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
    "live.deaths_starvation",
    "live.deaths_dehydration",
    "live.cumulative_mean_lifespan",
];

const SMOKE_DRIFT_OBS: [&str; 7] = [
    "live.closed_eu_total",
    "live.agent_count",
    "live.standing_crop",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
    "live.cumulative_mean_lifespan",
];

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

struct ManifestArgs {
    mut_rate: f64,
    drift_control: f64,
    drift_reset: f64,
    prey_cap: f64,
    scale: f64,
}

fn manifest_template(
    id: &str,
    cfg: ManifestArgs,
    seeds: Vec<u64>,
    observable_ids: Vec<String>,
) -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: id.into(),
        name: id.into(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: InitialConditionSet::new(vec![
            (LIVE_KEY_ECOLOGICAL_FITNESS.into(), 1.0),
            (LIVE_KEY_MORTALITY.into(), 1.0),
            (LIVE_KEY_REPRODUCTION.into(), 1.0),
            (LIVE_KEY_LOCOMOTION.into(), 1.0),
            (LIVE_KEY_DRIVE_FLOOR.into(), 0.5),
            (LIVE_KEY_THIRST_STEERING.into(), 1.0),
            (LIVE_KEY_EVOLVED_BRAINS.into(), 0.0),
            (LIVE_KEY_FOUNDERS.into(), 40.0),
            (LIVE_KEY_PREDATOR_FRACTION.into(), 0.0),
            (LIVE_KEY_TREES.into(), 8.0),
            (LIVE_KEY_LAKES.into(), 16.0),
            (LIVE_KEY_LAKE_LAYOUT.into(), 1.0),
            (LIVE_KEY_PLANT_RESERVE.into(), 0.0),
            (LIVE_KEY_FOOD_CAP.into(), 50.0),
            (LIVE_KEY_DEHYDRATION_TICKS.into(), 960.0),
            (LIVE_KEY_PREY_CAP.into(), cfg.prey_cap),
            (LIVE_KEY_STRICT_PREY_CAP.into(), 1.0),
            (LIVE_KEY_FOUNDER_GENOTYPE.into(), 1.0),
            (LIVE_KEY_FOUNDER_LAYOUT.into(), 0.0),
            (LIVE_KEY_RESOURCE_CAPACITY_SCALE.into(), cfg.scale),
            (LIVE_KEY_MUTATION_RATE.into(), cfg.mut_rate),
            (LIVE_KEY_DRIFT_CONTROL.into(), cfg.drift_control),
            (LIVE_KEY_DRIFT_RESET_TICKS.into(), cfg.drift_reset),
        ]),
        interventions: Vec::new(),
        seeds,
        duration_ticks: 2000,
        sample_period: 200,
        observable_ids,
        exotic_interventions: Vec::new(),
        observer: ObserverPolicy::Absent,
        island: None,
    }
}

struct SyntheticWorkspace {
    root: PathBuf,
    manifest_dir: PathBuf,
    temp_dir: PathBuf,
}

impl SyntheticWorkspace {
    fn new(tag: &str, c3_min_generations: f64) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "anima-sel2p2-runner-test-{tag}-{}-{nanos}",
            std::process::id()
        ));
        let manifest_dir = root.join("manifests");
        let temp_dir = root.join("proc_tmp");
        std::fs::create_dir_all(&manifest_dir).unwrap();
        std::fs::create_dir_all(&temp_dir).unwrap();

        let confirm_obs: Vec<String> = all_live_observable_ids()
            .into_iter()
            .filter(|id| !id.starts_with("live.deaths_"))
            .map(str::to_string)
            .collect();
        let smoke_non_drift: Vec<String> = SMOKE_NON_DRIFT_OBS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let smoke_drift: Vec<String> = SMOKE_DRIFT_OBS.iter().map(|s| (*s).to_string()).collect();

        let prereg = serde_json::json!({
            "schema_version": 1,
            "preregistration_id": "sel2p2-synthetic-test",
            "status": "FROZEN",
            "manifests": {
                "selected": "sel2p2-selected.json",
                "scarce_mut0": "sel2p2-scarce-mut0.json",
                "scarce_drift": "sel2p2-scarce-drift.json",
                "baseline_ref": "sel2p2-baseline-ref.json",
                "smoke_selected": "sel2p2-smoke-selected.json",
                "smoke_scarce_mut0": "sel2p2-smoke-scarce-mut0.json",
                "smoke_scarce_drift": "sel2p2-smoke-scarce-drift.json"
            },
            "seeds": {
                "execution_order": TEST_SEEDS,
                "baseline_ref_seeds": TEST_BASELINE_SEEDS,
                "min_surviving_baseline_seeds": 1,
                "smoke_seed": TEST_SMOKE_SEED
            },
            "duration": {
                "duration_ticks": 2000,
                "warmup_ticks": 200,
                "confirmation_sample_period": 200,
                "smoke_sample_period": 200,
                "tail20_threshold_tick": 1600
            },
            "thresholds": {
                "fwer_alpha": 0.05,
                "sesoi_log_mean_ratio": 1.10_f64.ln(),
                "max_arm_extinction_rate_for_complete_pairs": 0.05,
                "min_pi_ll": 0.85,
                "min_valid_pairs_for_decision": 2,
                "min_non_zero_pairs_r7": 2,
                "max_abs_eu_drift": 1e-6,
                "rmst_tau_ticks": 2000,
                "bca_bootstrap_resamples": 100
            },
            "smoke_gate": {
                "min_surviving_baseline_seeds": 1,
                "c1_min_agents": 1.0,
                "c2_g_band": [0.0, 100.0],
                "c3_min_generations": c3_min_generations,
                "c4_min_starvation_share": -1.0
            },
            "artifacts": {
                "root": "artifacts/experiments/sel2-p2/",
                "smoke_dir": "artifacts/experiments/sel2-p2/smoke",
                "confirmation_dir": "artifacts/experiments/sel2-p2/confirmation"
            }
        });
        std::fs::write(
            manifest_dir.join("sel2p2-preregistration.json"),
            serde_json::to_string_pretty(&prereg).unwrap(),
        )
        .unwrap();

        for (file, mut_rate, drift_ctrl, drift_reset, prey_cap, scale, seeds, obs) in [
            (
                "sel2p2-selected.json",
                0.05,
                0.0,
                0.0,
                320.0,
                0.75,
                TEST_SEEDS.to_vec(),
                confirm_obs.clone(),
            ),
            (
                "sel2p2-scarce-mut0.json",
                0.0,
                0.0,
                0.0,
                320.0,
                0.75,
                TEST_SEEDS.to_vec(),
                confirm_obs.clone(),
            ),
            (
                "sel2p2-scarce-drift.json",
                0.05,
                1.0,
                500.0,
                320.0,
                0.75,
                TEST_SEEDS.to_vec(),
                confirm_obs,
            ),
            (
                "sel2p2-baseline-ref.json",
                0.05,
                0.0,
                0.0,
                80.0,
                1.0,
                TEST_BASELINE_SEEDS.to_vec(),
                smoke_non_drift.clone(),
            ),
            (
                "sel2p2-smoke-selected.json",
                0.05,
                0.0,
                0.0,
                320.0,
                0.75,
                vec![TEST_SMOKE_SEED],
                smoke_non_drift.clone(),
            ),
            (
                "sel2p2-smoke-scarce-mut0.json",
                0.0,
                0.0,
                0.0,
                320.0,
                0.75,
                vec![TEST_SMOKE_SEED],
                smoke_non_drift,
            ),
            (
                "sel2p2-smoke-scarce-drift.json",
                0.05,
                1.0,
                500.0,
                320.0,
                0.75,
                vec![TEST_SMOKE_SEED],
                smoke_drift,
            ),
        ] {
            let m = manifest_template(
                file,
                ManifestArgs {
                    mut_rate,
                    drift_control: drift_ctrl,
                    drift_reset,
                    prey_cap,
                    scale,
                },
                seeds,
                obs,
            );
            std::fs::write(
                manifest_dir.join(file),
                serde_json::to_string_pretty(&m).unwrap(),
            )
            .unwrap();
        }

        Self {
            root,
            manifest_dir,
            temp_dir,
        }
    }

    fn release_env(&self, allow_arbitrary_out: bool) -> runner::RuntimeEnv {
        runner::RuntimeEnv {
            anima_world_artifact: None,
            temp_dir: self.temp_dir.clone(),
            is_debug_build: false,
            host_os: "windows".to_string(),
            host_arch: "x86_64".to_string(),
            allow_arbitrary_out,
            git_commit_override: Some("deadbeef".to_string()),
            git_dirty_paths_override: Some(Vec::new()),
            world_identity_override: None,
        }
    }
}

impl Drop for SyntheticWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn synthetic_test_seeds_never_collide_with_committed_fixtures() {
    let fixtures_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut files = Vec::new();
    collect_json_files(&fixtures_root, &mut files);
    assert!(!files.is_empty());

    let reserved_nums: BTreeSet<u64> = TEST_SEEDS
        .iter()
        .copied()
        .chain(TEST_BASELINE_SEEDS.iter().copied())
        .chain(std::iter::once(TEST_SMOKE_SEED))
        .collect();
    let reserved: BTreeSet<String> = reserved_nums.iter().map(|s| s.to_string()).collect();

    for file in files {
        let raw = std::fs::read_to_string(&file).unwrap();
        if file.file_name().and_then(|s| s.to_str()) == Some("sel2p2-preregistration.json") {
            let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
            for block in [
                &v["seeds"]["execution_order"],
                &v["seeds"]["baseline_ref_seeds"],
                &v["seeds"]["smoke_seed"],
                &v["seeds"]["pipeline_check"],
            ] {
                let s = serde_json::to_string(block).unwrap();
                for tok in &reserved {
                    assert!(
                        !s.contains(tok),
                        "synthetic seed {tok} collided with non-test seed block in {}",
                        file.display()
                    );
                }
            }
            continue;
        }
        for tok in &reserved {
            assert!(
                !raw.contains(tok),
                "synthetic seed {tok} collided with fixture {}",
                file.display()
            );
        }
    }
}

#[test]
fn statistical_primitives_and_r10_closed_3_label_partition_match_verifier() {
    assert!((runner::student_t_inv(0.975, 47) - 2.01174051).abs() < 1e-6);
    assert!((runner::student_t_inv(0.95, 47) - 1.67792672).abs() < 1e-6);

    let p_holm = runner::holm_adjust(&[Some(0.01), Some(0.04), Some(0.03)]);
    assert!((p_holm[0].unwrap() - 0.03).abs() < 1e-12);
    assert!((p_holm[1].unwrap() - 0.06).abs() < 1e-12);
    assert!((p_holm[2].unwrap() - 0.06).abs() < 1e-12);

    let sesoi = 1.10_f64.ln();
    let d_pos = runner::evaluate_paired_log_mean_ratio(
        &[2.5, 2.51, 2.52, 2.5],
        &[2.9, 2.95, 2.91, 2.92],
        sesoi,
        2,
        4,
        200,
    )
    .unwrap();
    assert!(d_pos.mean_d.unwrap() > sesoi);
    assert!(d_pos.p_two_sided.unwrap() < 0.01);
    assert!(d_pos.hl_bca_95_ci.ci_low.unwrap() < d_pos.hl_bca_95_ci.ci_high.unwrap());

    let lee = runner::lee_trimming_bounds_log_mean(&[2.5, 2.52], &[2.8, 2.9, 3.0]);
    assert_eq!(lee.k, 2);
    assert!(lee.lee_hi.unwrap() > lee.lee_lo.unwrap());

    let mut counts = [0usize; 3]; // supports, equivalent_to_zero, insufficient_evidence
    for host in [true, false] {
        for drift_ok in [true, false] {
            for holm in [true, false] {
                for mean_d in [0.15_f64, -0.15, 0.02] {
                    for tost in [true, false] {
                        let dec = runner::classify_sel2p2_decision(
                            host,
                            drift_ok,
                            drift_ok,
                            "complete_pairs",
                            48,
                            48,
                            0.85,
                            7,
                            Some(if holm { 0.01 } else { 0.20 }),
                            0.05,
                            Some(mean_d),
                            tost,
                            sesoi,
                            None,
                            None,
                            None,
                        );
                        match dec.label {
                            "supports" => counts[0] += 1,
                            "equivalent_to_zero" => counts[1] += 1,
                            "insufficient_evidence" => counts[2] += 1,
                            other => panic!("unexpected label {other}"),
                        }
                    }
                }
            }
        }
    }
    assert_eq!(counts, [2, 2, 44], "R10 48/48 closed 3-label partition");

    let lachin_pos = runner::classify_sel2p2_decision(
        true,
        true,
        true,
        "lachin_worst_rank",
        38,
        48,
        0.85,
        7,
        Some(0.005),
        0.05,
        None,
        false,
        sesoi,
        Some(18.5),
        Some(0.11),
        Some(0.16),
    );
    assert_eq!(lachin_pos.label, "supports");
    assert_eq!(lachin_pos.direction_tag, "lachin_two_sided_positive");
}

#[test]
fn dry_run_validates_frozen_preregistration_lists_all_runs_and_writes_nothing() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest_dir = repo_root.join("tests/fixtures/experiments_sel2p2");
    let art_confirm = repo_root
        .parent()
        .unwrap()
        .join("artifacts/experiments/sel2-p2/confirmation");
    let existed_before = art_confirm.exists();

    runner::run_with_config(
        vec![
            "--dry-run".into(),
            "--manifest-dir".into(),
            manifest_dir.display().to_string(),
        ],
        runner::RuntimeEnv::from_process(),
    )
    .expect("--dry-run succeeds cleanly without running any seed");

    assert_eq!(
        art_confirm.exists(),
        existed_before,
        "--dry-run must not create confirmation artifacts"
    );
}

#[test]
fn refuses_debug_env_artifact_temp_file_outside_out_and_unblinded_smoke() {
    let ws = SyntheticWorkspace::new("refusals", 0.0);

    let mut dbg_env = ws.release_env(true);
    dbg_env.is_debug_build = true;
    let err_dbg = runner::run_with_config(
        vec![
            "--smoke".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--out".into(),
            ws.root.join("out_dbg").display().to_string(),
        ],
        dbg_env,
    )
    .unwrap_err();
    assert!(err_dbg.contains("--release"), "got: {err_dbg}");

    let mut art_env = ws.release_env(true);
    art_env.anima_world_artifact = Some("some_file.anmw".into());
    let err_art = runner::run_with_config(
        vec![
            "--smoke".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--out".into(),
            ws.root.join("out_art").display().to_string(),
        ],
        art_env,
    )
    .unwrap_err();
    assert!(err_art.contains("ANIMA_WORLD_ARTIFACT"), "got: {err_art}");

    let tmp_anmw = ws.temp_dir.join("anima_world_artifact.anmw");
    std::fs::write(&tmp_anmw, b"desktop-world").unwrap();
    let err_tmp = runner::run_with_config(
        vec![
            "--smoke".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--out".into(),
            ws.root.join("out_tmp").display().to_string(),
        ],
        ws.release_env(true),
    )
    .unwrap_err();
    assert!(
        err_tmp.contains("anima_world_artifact.anmw"),
        "got: {err_tmp}"
    );
    std::fs::remove_file(&tmp_anmw).unwrap();

    // Production process (`allow_arbitrary_out = false`) refuses writing outside preregistered root.
    let err_out = runner::run_with_config(
        vec![
            "--smoke".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--out".into(),
            ws.root.join("out_disallowed").display().to_string(),
        ],
        ws.release_env(false),
    )
    .unwrap_err();
    assert!(
        err_out.contains("outside preregistered experiment directory"),
        "got: {err_out}"
    );

    // Unblinded smoke manifest containing trait_mass must be refused before running any seed.
    let bad_smoke_path = ws.manifest_dir.join("sel2p2-smoke-selected.json");
    let orig_smoke = std::fs::read_to_string(&bad_smoke_path).unwrap();
    let mut bad_m: ExperimentManifest = serde_json::from_str(&orig_smoke).unwrap();
    bad_m.observable_ids.push("live.trait_mass_mean".into());
    std::fs::write(
        &bad_smoke_path,
        serde_json::to_string_pretty(&bad_m).unwrap(),
    )
    .unwrap();
    let err_blind = runner::run_with_config(
        vec![
            "--smoke".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--out".into(),
            ws.root.join("out_blind").display().to_string(),
        ],
        ws.release_env(true),
    )
    .unwrap_err();
    assert!(err_blind.contains("blinding violation"), "got: {err_blind}");
    std::fs::write(&bad_smoke_path, orig_smoke).unwrap();
}

#[test]
fn negative_control_failed_smoke_hard_stops_confirmation() {
    // 1. Live `--run-all` with an impossible smoke threshold (`c3_min_generations = 999999`)
    //    must stop after smoke, write a blind failure report, and never create `confirmation/`.
    let ws = SyntheticWorkspace::new("neg_hardstop", 999_999.0);
    let env = ws.release_env(true);
    let run_all_root = ws.root.join("run_all");
    let err = runner::run_with_config(
        vec![
            "--run-all".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--out".into(),
            run_all_root.display().to_string(),
        ],
        env.clone(),
    )
    .unwrap_err();
    assert!(
        err.contains("smoke gate FAILED (hard stop)"),
        "expected hard-stop refusal, got: {err}"
    );
    let smoke_report_raw =
        std::fs::read_to_string(run_all_root.join("smoke/smoke-report.json")).unwrap();
    let smoke_report: serde_json::Value = serde_json::from_str(&smoke_report_raw).unwrap();
    assert_eq!(smoke_report["gate_pass"], serde_json::Value::Bool(false));
    assert!(!smoke_report_raw.to_ascii_lowercase().contains("trait_mass"));
    assert!(
        !run_all_root.join("confirmation").exists(),
        "confirmation dir must not be created after failed smoke"
    );

    // 2. Direct `--confirm` pointing at that failed smoke dir must also refuse.
    let err_confirm = runner::run_with_config(
        vec![
            "--confirm".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--smoke-dir".into(),
            run_all_root.join("smoke").display().to_string(),
            "--out".into(),
            ws.root.join("confirm_blocked").display().to_string(),
        ],
        env,
    )
    .unwrap_err();
    assert!(
        err_confirm.contains("gate_pass != true"),
        "expected gate_pass refusal, got: {err_confirm}"
    );
    assert!(!ws.root.join("confirm_blocked").exists());
}

#[test]
fn tiny_tick_smoke_and_confirm_pass_node_verifier_and_checkpoint_resume_is_identical() {
    let ws = SyntheticWorkspace::new("e2e", 0.0);
    let env = ws.release_env(true);
    let smoke_dir = ws.root.join("smoke");
    let confirm_seq = ws.root.join("confirm_seq");
    let confirm_par = ws.root.join("confirm_par");
    let ckpt_dir = ws.root.join("ckpt");

    // 1. Run blind smoke into temp dir and confirm zero `trait_mass` substrings anywhere.
    runner::run_with_config(
        vec![
            "--smoke".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--out".into(),
            smoke_dir.display().to_string(),
        ],
        env.clone(),
    )
    .expect("tiny-tick blind smoke succeeds");

    let mut stack = vec![smoke_dir.clone()];
    while let Some(dir) = stack.pop() {
        for ent in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = ent.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let text = std::fs::read_to_string(&p).unwrap();
                assert!(
                    !text.to_ascii_lowercase().contains("trait_mass"),
                    "forbidden trait_mass substring found in {}",
                    p.display()
                );
            }
        }
    }

    // 2. Run confirmation sequentially with checkpoint dir.
    runner::run_with_config(
        vec![
            "--confirm".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--smoke-dir".into(),
            smoke_dir.display().to_string(),
            "--checkpoint-dir".into(),
            ckpt_dir.display().to_string(),
            "--jobs".into(),
            "1".into(),
            "--out".into(),
            confirm_seq.display().to_string(),
        ],
        env.clone(),
    )
    .expect("tiny-tick sequential confirmation succeeds");

    // 3. Independent Node verifier must exit 0 on confirm_seq.
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let verifier = repo_root.join("scripts/verify_sel2p2_artifacts.mjs");
    let prereg_path = ws.manifest_dir.join("sel2p2-preregistration.json");
    let out = Command::new("node")
        .current_dir(repo_root)
        .args([
            verifier.to_string_lossy().as_ref(),
            confirm_seq.to_string_lossy().as_ref(),
            "--prereg",
            prereg_path.to_string_lossy().as_ref(),
            "--skip-committed-manifests",
        ])
        .output()
        .expect("invoke verify_sel2p2_artifacts.mjs");
    assert!(
        out.status.success(),
        "verify_sel2p2_artifacts.mjs failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // 4. Delete 1 checkpoint and re-run with --jobs 2; resume + fresh run must agree byte-for-byte.
    let del_ckpt = ckpt_dir.join(format!("seed_{}.json", TEST_SEEDS[0]));
    assert!(del_ckpt.exists());
    std::fs::remove_file(&del_ckpt).unwrap();

    runner::run_with_config(
        vec![
            "--confirm".into(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--smoke-dir".into(),
            smoke_dir.display().to_string(),
            "--checkpoint-dir".into(),
            ckpt_dir.display().to_string(),
            "--jobs".into(),
            "2".into(),
            "--out".into(),
            confirm_par.display().to_string(),
        ],
        env,
    )
    .expect("tiny-tick resumed parallel confirmation succeeds");
    assert!(del_ckpt.exists(), "deleted checkpoint must be regenerated");

    for fname in ["series.csv", "final-observables.json", "effects.json"] {
        let a = std::fs::read_to_string(confirm_seq.join(fname)).unwrap();
        let b = std::fs::read_to_string(confirm_par.join(fname)).unwrap();
        assert_eq!(
            a, b,
            "{fname} differed between seq and resumed parallel run"
        );
    }
}

#[test]
fn frozen_sel2p2_fixtures_validate_against_runner_without_running_seeds() {
    let registry = ObservableRegistry::live_default();
    let prereg_dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/experiments_sel2p2");
    let prereg = runner::load_prereg(&prereg_dir).expect("loads frozen sel2p2 preregistration");
    assert_eq!(prereg.execution_order.len(), 48);
    assert_eq!(prereg.baseline_ref_seeds.len(), 12);
    assert_eq!(prereg.smoke_seed, 999_931);
    assert!(runner::load_and_validate_all_manifests(&prereg_dir, &prereg, &registry).is_ok());
}
