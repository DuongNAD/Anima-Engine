//! Integration tests for `src-tauri/examples/run_viab2_probe.rs` (`CARD C5b. SEL2P2-VIAB2`).
//!
//! Uses only synthetic test seeds (`614001`, `614002`, `614999`) at tiny tick counts in `%TEMP%`
//! — never runs `880001..=880012` or `SEL2-P2` confirmation/baseline seeds, and never writes
//! under `artifacts/`.

#[allow(dead_code)]
#[path = "../examples/run_viab2_probe.rs"]
mod probe;

use anima_engine_lib::core::world_artifact::WorldIdentity;
use probe::{
    assert_blind_output_dir, build_viab2_manifests, evaluate_cell_outcomes, execute_probe,
    parse_args, run_self_test, validate_seed_policy, Args, ArmMetrics, DriftDiagSummary,
    DEFAULT_EXPLORATORY_END, DEFAULT_EXPLORATORY_START,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

struct TempDirGuard(PathBuf);

impl TempDirGuard {
    fn new(prefix: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nanos));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_dir_files(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("create dst dir");
    for entry in fs::read_dir(src).expect("read src dir").flatten() {
        let p = entry.path();
        if p.is_file() {
            fs::copy(&p, dst.join(entry.file_name())).expect("copy output file");
        }
    }
}

fn ic_val(
    manifest: &anima_engine_lib::core::experiment::ExperimentManifest,
    key: &str,
) -> Option<f64> {
    manifest
        .initial_conditions
        .values
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| *v)
}

#[test]
fn viab2_manifests_match_qd_sel2p2_d1_d2_and_stay_blind() {
    let seeds: Vec<u64> = (DEFAULT_EXPLORATORY_START..=DEFAULT_EXPLORATORY_END).collect();
    for layout in [0u8, 2u8] {
        let set = build_viab2_manifests(layout, &seeds, 200_000, 50, 2_000)
            .expect("build viab2 manifests");
        assert_eq!(set.layout, layout);
        for m in [
            &set.baseline_ref,
            &set.selected,
            &set.scarce_mut0,
            &set.scarce_drift,
        ] {
            assert_eq!(ic_val(m, "live.founder_genotype"), Some(1.0));
            assert_eq!(ic_val(m, "live.strict_prey_cap"), Some(1.0));
            assert_eq!(ic_val(m, "live.founder_layout"), Some(f64::from(layout)));
            assert!(
                !m.observable_ids
                    .iter()
                    .any(|o| o.to_ascii_lowercase().contains("trait_mass")),
                "manifest {} must never request trait_mass observables",
                m.experiment_id
            );
        }
        assert_eq!(ic_val(&set.baseline_ref, "live.prey_cap"), Some(80.0));
        assert_eq!(
            ic_val(&set.baseline_ref, "live.resource_capacity_scale"),
            Some(1.0)
        );
        assert_eq!(ic_val(&set.selected, "live.prey_cap"), Some(320.0));
        assert_eq!(
            ic_val(&set.selected, "live.resource_capacity_scale"),
            Some(0.75)
        );
        assert_eq!(ic_val(&set.selected, "live.mutation_rate"), Some(0.05));
        assert_eq!(ic_val(&set.scarce_mut0, "live.mutation_rate"), Some(0.0));
        assert_eq!(ic_val(&set.scarce_drift, "live.drift_control"), Some(1.0));
        assert_eq!(
            ic_val(&set.scarce_drift, "live.drift_reset_ticks"),
            Some(2_000.0)
        );
    }
}

#[test]
fn viab2_seed_guard_refuses_confirmation_and_smoke_seeds() {
    let valid: Vec<u64> = (DEFAULT_EXPLORATORY_START..=DEFAULT_EXPLORATORY_END).collect();
    assert!(validate_seed_policy(&valid, false, 200_000).is_ok());

    for bad in [900_001u64, 970_501, 999_931, 810_001, 614_001] {
        assert!(
            validate_seed_policy(&[bad], false, 200_000).is_err(),
            "production VIAB-2 mode must reject seed {bad}"
        );
    }

    assert!(validate_seed_policy(&[614_001, 614_002], true, 200).is_ok());
    assert!(validate_seed_policy(&[880_001], true, 200).is_err());
    assert!(validate_seed_policy(&[614_001], true, 5_000).is_err());
}

#[test]
fn viab2_end_to_end_resumable_and_trait_mass_negative_control() {
    if std::env::var_os("ANIMA_WORLD_ARTIFACT").is_some()
        || std::env::temp_dir()
            .join("anima_world_artifact.anmw")
            .exists()
    {
        panic!("host preflight environment polluted during test");
    }

    let tmp = TempDirGuard::new("viab2_probe_e2e");
    let out1 = tmp.path().join("out1");
    let out2 = tmp.path().join("out2");
    let ckpt = tmp.path().join("ckpt");

    let argv1 = vec![
        "--allow-unit-test-seeds".into(),
        "--layout".into(),
        "0".into(),
        "--seeds".into(),
        "614001,614002".into(),
        "--duration-ticks".into(),
        "200".into(),
        "--sample-period".into(),
        "50".into(),
        "--drift-reset-ticks".into(),
        "100".into(),
        "--min-surviving-baseline".into(),
        "1".into(),
        "--jobs".into(),
        "2".into(),
        "--checkpoint-dir".into(),
        ckpt.display().to_string(),
        "--out".into(),
        out1.display().to_string(),
    ];
    let args1: Args = parse_args(&argv1).expect("parse argv1");
    execute_probe(&args1, &argv1).expect("execute_probe run 1");

    for fname in [
        "viab2_fg1_s075_k2k_fl0.json",
        "summary.csv",
        "provenance.json",
        "checksums.sha256",
    ] {
        assert!(out1.join(fname).is_file(), "missing {fname}");
    }
    assert_blind_output_dir(&out1).expect("out1 must pass blind schema check");

    let ckpt_count = fs::read_dir(&ckpt)
        .expect("read ckpt")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .count();
    assert_eq!(ckpt_count, 6, "2 seeds x 3 job kinds = 6 checkpoints");

    let cell_raw = fs::read_to_string(out1.join("viab2_fg1_s075_k2k_fl0.json")).unwrap();
    let cell_json: serde_json::Value = serde_json::from_str(&cell_raw).unwrap();
    assert_eq!(
        cell_json["mass_observables_excluded"],
        serde_json::json!(true)
    );
    assert_eq!(cell_json["all_terrain_generated"], serde_json::json!(true));
    assert_eq!(
        cell_json["per_seed"][0]["drift_diagnostics"]["window_count"],
        serde_json::json!(2)
    );
    assert!(
        cell_json["per_seed"][0]["selected"]["starvation_share"].is_number()
            || cell_json["per_seed"][0]["selected"]["total_deaths"] == serde_json::json!(0.0)
    );

    // Resumability: rerun against the same checkpoint dir into out2 and compare final_checksums.
    let mut argv2 = argv1.clone();
    *argv2.last_mut().unwrap() = out2.display().to_string();
    let args2 = parse_args(&argv2).expect("parse argv2");
    execute_probe(&args2, &argv2).expect("execute_probe resumed run 2");
    let cell2_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(out2.join("viab2_fg1_s075_k2k_fl0.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(cell2_json["per_seed"], cell_json["per_seed"]);

    // Required negative control: inject `"trait_mass_mean"` into a temp copy of output -> must FAIL.
    let tampered_json_dir = tmp.path().join("tampered_json");
    copy_dir_files(&out1, &tampered_json_dir);
    let cell_copy_path = tampered_json_dir.join("viab2_fg1_s075_k2k_fl0.json");
    let mut poisoned_obj: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&cell_copy_path).unwrap()).unwrap();
    poisoned_obj["trait_mass_mean"] = serde_json::json!(2.5);
    fs::write(
        &cell_copy_path,
        serde_json::to_string_pretty(&poisoned_obj).unwrap(),
    )
    .unwrap();
    let json_err = assert_blind_output_dir(&tampered_json_dir)
        .expect_err("injected trait_mass_mean JSON field must fail");
    assert!(
        json_err.contains("blindness violation")
            && json_err.contains("viab2_fg1_s075_k2k_fl0.json"),
        "unexpected error message: {json_err}"
    );

    let tampered_csv_dir = tmp.path().join("tampered_csv");
    copy_dir_files(&out1, &tampered_csv_dir);
    let csv_copy_path = tampered_csv_dir.join("summary.csv");
    let mut csv_text = fs::read_to_string(&csv_copy_path).unwrap();
    csv_text.push_str("# leak live.trait_mass_var\n");
    fs::write(&csv_copy_path, csv_text).unwrap();
    assert!(
        assert_blind_output_dir(&tampered_csv_dir).is_err(),
        "injected trait_mass substring in summary.csv must fail"
    );
}

fn synthetic_arm(arm: &str, seed: u64, survived: bool, drift: f64) -> ArmMetrics {
    ArmMetrics {
        arm: arm.to_string(),
        seed,
        completed: true,
        survived,
        extinct_tick: if survived { None } else { Some(50_000) },
        final_checksum: 1,
        terrain_source: Some("generated".to_string()),
        world_identity: Some(WorldIdentity::default()),
        min_n: if survived { 15.0 } else { 0.0 },
        mean_n: 80.0,
        max_n: 100.0,
        final_n: if survived { 80.0 } else { 0.0 },
        total_births: 4_000.0,
        total_deaths: 3_980.0,
        total_starvation: Some(3_000.0),
        total_dehydration: Some(980.0),
        starvation_share: Some(0.75),
        cause_partition_ok: Some(true),
        mean_lifespan_weighted: Some(4_000.0),
        generations_proxy: Some(50.0),
        births_last_quarter: 500.0,
        deaths_last_quarter: 500.0,
        crop_second_half: if arm == "baseline_ref" { 100.0 } else { 50.0 },
        max_abs_eu_drift: drift,
        elapsed_secs: 1.0,
    }
}

fn synthetic_diag() -> DriftDiagSummary {
    DriftDiagSummary {
        scheduled_deaths: 100,
        actual_deaths: 100,
        missed_deaths: 0,
        scheduled_births: 100,
        actual_births: 100,
        missed_births_extinct: 0,
        missed_births_insolvent: 0,
        missed_births_capped: 0,
        ticks_with_missed_deaths: 0,
        ticks_with_missed_births: 0,
        first_divergence_tick: None,
        window_count: 100,
        has_shortfall: false,
    }
}

#[test]
fn viab2_d1a_gate_requires_at_least_11_of_12_in_every_arm() {
    let seeds: Vec<u64> = (DEFAULT_EXPLORATORY_START..=DEFAULT_EXPLORATORY_END).collect();
    let eval_case = |drop_selected: usize, drop_mut0: usize, drop_drift: usize, drift: f64| {
        let mut base = BTreeMap::new();
        let mut mut0 = BTreeMap::new();
        let mut pair = BTreeMap::new();
        for (idx, &s) in seeds.iter().enumerate() {
            base.insert(s, synthetic_arm("baseline_ref", s, true, 1e-8));
            mut0.insert(s, synthetic_arm("scarce_mut0", s, idx >= drop_mut0, 1e-8));
            pair.insert(
                s,
                (
                    synthetic_arm("selected", s, idx >= drop_selected, drift),
                    synthetic_arm("scarce_drift", s, idx >= drop_drift, 1e-8),
                    synthetic_diag(),
                ),
            );
        }
        evaluate_cell_outcomes(&base, &mut0, &pair, &seeds, 6)
            .unwrap()
            .aggregate
    };

    assert!(eval_case(1, 1, 1, 1e-8).d1a_survival_pass);
    assert!(!eval_case(2, 0, 0, 1e-8).d1a_survival_pass);
    assert!(!eval_case(0, 2, 0, 1e-8).d1a_survival_pass);
    assert!(!eval_case(0, 0, 2, 1e-8).d1a_survival_pass);
    assert!(!eval_case(0, 0, 0, 1e-5).d1a_survival_pass);
}

#[test]
fn viab2_self_test_passes_cleanly() {
    run_self_test().expect("self-test must pass");
}
