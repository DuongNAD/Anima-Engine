//! Integration tests for `src-tauri/examples/run_e3b_experiment.rs` (`CARD C1. E3B-RUNNER`).
//!
//! Uses only disjoint synthetic test seeds (`613_001`, `613_002`, smoke `613_999`) verified absent
//! from every committed fixture -- NEVER runs any E3b confirmation seed (`810001..=873001`) or
//! `smoke_seed` (`999953`), and never writes under `artifacts/`.

use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, WorldLawSet,
    MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::intervention::{Curve, InterventionCommand, InterventionKind, Region};
use anima_engine_lib::core::live_experiment::{
    all_live_observable_ids, LIVE_KEY_RESOURCE_CAPACITY_SCALE,
};
use anima_engine_lib::core::world_artifact::WorldIdentity;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

#[allow(dead_code)]
#[path = "../examples/run_e3b_experiment.rs"]
mod runner;

const TEST_SEEDS: [u64; 2] = [613_001, 613_002];
const TEST_SMOKE_SEED: u64 = 613_999;

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

fn manifest_template(
    id: &str,
    scale: f64,
    deforest: f32,
    nutrient: f32,
    nut_min_x: u32,
    seeds: Vec<u64>,
    blind: bool,
) -> ExperimentManifest {
    let obs: Vec<String> = all_live_observable_ids()
        .into_iter()
        .filter(|name| !blind || !name.to_ascii_lowercase().contains("trait_mass"))
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
            ("live.founders".into(), 12.0),
            ("live.founder_layout".into(), 1.0),
            ("live.founder_genotype".into(), 1.0),
            ("live.predator_fraction".into(), 0.3),
            ("live.trees".into(), 8.0),
            ("live.lakes".into(), 4.0),
            ("live.lake_layout".into(), 1.0),
            ("live.food_cap".into(), 50.0),
            ("live.prey_cap".into(), 60.0),
            ("live.strict_prey_cap".into(), 1.0),
            ("live.dehydration_ticks".into(), 960.0),
            ("live.mutation_rate".into(), 0.05),
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
                duration_ticks: 1800,
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
                duration_ticks: 1800,
                curve: Curve::Step,
                reversible: false,
                signed_negative: false,
            },
        ],
        seeds,
        duration_ticks: 1800,
        sample_period: 300,
        observable_ids: obs,
        exotic_interventions: Vec::new(),
        observer: Default::default(),
        island: None,
    }
}

struct SyntheticWorkspace {
    root: PathBuf,
    manifest_dir: PathBuf,
    temp_dir: PathBuf,
}

impl SyntheticWorkspace {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "anima-e3b-runner-test-{tag}-{}-{nanos}",
            std::process::id()
        ));
        let manifest_dir = root.join("manifests");
        let temp_dir = root.join("proc_tmp");
        std::fs::create_dir_all(&manifest_dir).unwrap();
        std::fs::create_dir_all(&temp_dir).unwrap();

        let prereg = serde_json::json!({
            "schema_version": 1,
            "preregistration_id": "e3b-synthetic-test",
            "status": "FROZEN",
            "manifests": {
                "control": "ctrl.json",
                "control_uniform_scarcity": "scarce.json",
                "treatment": "treat.json",
                "smoke_control": "smoke-ctrl.json",
                "smoke_control_scarcity": "smoke-scarce.json",
                "smoke_treatment": "smoke-treat.json"
            },
            "seeds": {
                "execution_order": TEST_SEEDS,
                "smoke_seed": TEST_SMOKE_SEED
            },
            "duration": {
                "duration_ticks": 1800,
                "sample_period": 300,
                "warmup_ticks": 1200,
                "tail_window_ticks": [1500, 1800]
            },
            "thresholds": {
                "eps_primary": 0.0005,
                "eps_sensitivity": 0.00005,
                "sesoi_log_var_ratio": (1.5_f64).ln(),
                "fwer_alpha": 0.05,
                "min_pi_ll": 0.85,
                "min_valid_pairs_for_decision": 2,
                "min_non_zero_pairs_r7": 2,
                "max_abs_eu_drift": 1e-6
            }
        });
        std::fs::write(
            manifest_dir.join("e3b-preregistration.json"),
            serde_json::to_string_pretty(&prereg).unwrap(),
        )
        .unwrap();

        for (file, scale, deforest, nut, min_x, blind, seeds) in [
            (
                "ctrl.json",
                1.0,
                0.0_f32,
                0.0_f32,
                128_u32,
                false,
                TEST_SEEDS.to_vec(),
            ),
            ("scarce.json", 0.42, 0.0, 1.0, 0, false, TEST_SEEDS.to_vec()),
            ("treat.json", 1.0, 0.3, 1.0, 128, false, TEST_SEEDS.to_vec()),
            (
                "smoke-ctrl.json",
                1.0,
                0.0,
                0.0,
                128,
                true,
                vec![TEST_SMOKE_SEED],
            ),
            (
                "smoke-scarce.json",
                0.42,
                0.0,
                1.0,
                0,
                true,
                vec![TEST_SMOKE_SEED],
            ),
            (
                "smoke-treat.json",
                1.0,
                0.3,
                1.0,
                128,
                true,
                vec![TEST_SMOKE_SEED],
            ),
        ] {
            let m = manifest_template(file, scale, deforest, nut, min_x, seeds, blind);
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

    fn release_env(&self) -> runner::RuntimeEnv {
        runner::RuntimeEnv {
            anima_world_artifact: None,
            temp_dir: self.temp_dir.clone(),
            is_debug_build: false,
            host_os: "windows".to_string(),
            host_arch: "x86_64".to_string(),
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

    let reserved: BTreeSet<String> = TEST_SEEDS
        .iter()
        .copied()
        .chain(std::iter::once(TEST_SMOKE_SEED))
        .map(|s| s.to_string())
        .collect();
    for file in files {
        let raw = std::fs::read_to_string(&file).unwrap();
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
fn statistical_primitives_and_r10_48_partition_match_verifier_contract() {
    assert!((runner::student_t_inv(0.975, 63) - 1.99834054).abs() < 1e-6);
    assert!((runner::student_t_inv(0.975, 11) - 2.20098516).abs() < 1e-6);

    let p_holm = runner::holm_adjust(&[Some(0.01), Some(0.04), Some(0.03)]);
    assert!((p_holm[0].unwrap() - 0.03).abs() < 1e-12);
    assert!((p_holm[2].unwrap() - 0.06).abs() < 1e-12);
    assert!((p_holm[1].unwrap() - 0.06).abs() < 1e-12);

    let lee_bal = runner::lee_trimming_bounds(&[0.01, 0.02, 0.03], &[0.02, 0.04, 0.06], 5e-4);
    assert!(lee_bal.width.unwrap().abs() < 1e-12);
    let lee_unbal = runner::lee_trimming_bounds(&[0.01, 0.02], &[0.02, 0.04, 0.06], 5e-4);
    assert_eq!(lee_unbal.k, 2);
    assert!(lee_unbal.lee_hi.unwrap() > lee_unbal.lee_lo.unwrap());

    let mut counts = [0usize; 4]; // supports, opposite, equivalent_to_zero, insufficient_evidence
    let sesoi = 1.5_f64.ln();
    for wp2 in [true, false] {
        for strat in [true, false] {
            for holm in [true, false] {
                for mean_l in [0.5_f64, -0.5, 0.1] {
                    for tost in [true, false] {
                        let dec = runner::classify_e3b_decision(
                            wp2,
                            strat,
                            if strat { 64 } else { 50 },
                            64,
                            0.85,
                            7,
                            Some(if holm { 0.01 } else { 0.20 }),
                            0.05,
                            Some(mean_l),
                            tost,
                            sesoi,
                            true,
                        );
                        match dec.label {
                            "supports" => counts[0] += 1,
                            "opposite" => counts[1] += 1,
                            "equivalent_to_zero" => counts[2] += 1,
                            "insufficient_evidence" => counts[3] += 1,
                            other => panic!("unexpected label {other}"),
                        }
                    }
                }
            }
        }
    }
    assert_eq!(counts, [1, 1, 2, 44], "R10 48/48 discrete partition");
}

#[test]
fn refuses_debug_env_artifact_temp_file_and_unblinded_smoke() {
    let ws = SyntheticWorkspace::new("refusals");
    let mut dbg_env = ws.release_env();
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
    assert!(err_dbg.contains("--release"));

    let mut art_env = ws.release_env();
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
    assert!(err_art.contains("ANIMA_WORLD_ARTIFACT"));

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
        ws.release_env(),
    )
    .unwrap_err();
    assert!(err_tmp.contains("anima_world_artifact.anmw"));
    std::fs::remove_file(&tmp_anmw).unwrap();

    // Unblinded smoke manifest containing trait_mass must be refused before running.
    let bad_smoke_path = ws.manifest_dir.join("smoke-ctrl.json");
    let orig_smoke = std::fs::read_to_string(&bad_smoke_path).unwrap();
    let mut bad_m: ExperimentManifest = serde_json::from_str(&orig_smoke).unwrap();
    bad_m.observable_ids.push("live.trait_mass_var".into());
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
        ws.release_env(),
    )
    .unwrap_err();
    assert!(err_blind.contains("blinding violation"));
    std::fs::write(&bad_smoke_path, orig_smoke).unwrap();
}

#[test]
fn tiny_tick_smoke_and_ensemble_pass_node_verifier_and_checkpoint_resume_is_identical() {
    let ws = SyntheticWorkspace::new("e2e");
    let env = ws.release_env();
    let smoke_dir = ws.root.join("smoke");
    let confirm_seq = ws.root.join("confirm_seq");
    let confirm_par = ws.root.join("confirm_par");
    let ckpt_dir = ws.root.join("ckpt");

    // 1. Run blind smoke into temp dir.
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
    .expect("tiny-tick smoke succeeds");

    // Verify zero `trait_mass` substrings anywhere inside smoke output.
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

    // 2. Run ensemble sequentially with checkpoint dir (writes checkpoints).
    runner::run_with_config(
        vec![
            "--ensemble".into(),
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
    .expect("tiny-tick sequential ensemble succeeds");

    // 3. Independent Node verifier must exit 0 on confirm_seq.
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let verifier = repo_root.join("scripts/verify_e3b_artifacts.mjs");
    let prereg_path = ws.manifest_dir.join("e3b-preregistration.json");
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
        .expect("invoke verify_e3b_artifacts.mjs");
    assert!(
        out.status.success(),
        "verify_e3b_artifacts.mjs failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // 4. Delete 1 checkpoint and re-run with --jobs 2; resume + fresh run must agree on data files.
    let del_ckpt = ckpt_dir.join(format!("{}__control_uniform.json", TEST_SEEDS[0]));
    assert!(
        del_ckpt.exists(),
        "expected checkpoint {} to exist before deletion",
        del_ckpt.display()
    );
    std::fs::remove_file(&del_ckpt).unwrap();
    runner::run_with_config(
        vec![
            "--ensemble".into(),
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
        env.clone(),
    )
    .expect("tiny-tick resumed parallel ensemble succeeds");
    assert!(
        del_ckpt.exists(),
        "deleted checkpoint {} must be regenerated on resume",
        del_ckpt.display()
    );

    for fname in ["series.csv", "final-observables.json", "effects.json"] {
        let a = std::fs::read_to_string(confirm_seq.join(fname)).unwrap();
        let b = std::fs::read_to_string(confirm_par.join(fname)).unwrap();
        assert_eq!(
            a, b,
            "{fname} differed between seq and resumed parallel run"
        );
    }

    // 5. Verify --replay matches final_checksum recorded in runs.csv.
    let replay_dir = ws.root.join("replay_out");
    runner::run_with_config(
        vec![
            "--replay".into(),
            "--arm".into(),
            "treatment_gradient".into(),
            "--seed".into(),
            TEST_SEEDS[0].to_string(),
            "--manifest-dir".into(),
            ws.manifest_dir.display().to_string(),
            "--out".into(),
            replay_dir.display().to_string(),
        ],
        env,
    )
    .expect("replay succeeds");
    let replay_file = replay_dir.join(format!(
        "replay-treatment_gradient-seed-{}.json",
        TEST_SEEDS[0]
    ));
    let replay_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(replay_file).unwrap()).unwrap();
    let replay_csum = replay_json["final_checksum"].as_u64().unwrap();
    let final_obs: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(confirm_seq.join("final-observables.json")).unwrap(),
    )
    .unwrap();
    let expected_csum = final_obs
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            row["seed"].as_u64() == Some(TEST_SEEDS[0])
                && row["arm"].as_str() == Some("treatment_gradient")
        })
        .unwrap()["final_checksum"]
        .as_u64()
        .unwrap();
    assert_eq!(replay_csum, expected_csum);
}

#[test]
fn frozen_e3b_fixtures_validate_against_runner_without_running_seeds() {
    let registry = ObservableRegistry::live_default();
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let e3b_dir = repo_root.join("tests/fixtures/experiments_e3b");
    let prereg = runner::load_prereg(&e3b_dir).expect("loads frozen e3b preregistration");
    assert_eq!(prereg.execution_order.len(), 64);
    assert_eq!(prereg.smoke_seed, 999_953);
    assert!(runner::load_and_validate_confirmation_manifests(&e3b_dir, &prereg, &registry).is_ok());
    assert!(runner::load_and_validate_smoke_manifests(&e3b_dir, &prereg, &registry).is_ok());
}
