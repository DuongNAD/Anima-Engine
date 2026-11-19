//! Machine-checked validation of the SEL2-P preregistration, manifests, seed disjointness,
//! drift-observable compatibility (draft dependency #4), and QD-SEL2g pooled-reference gate math.
//!
//! Mirrors `prereg_esel1_manifest_tests.rs`. Never constructs a `LiveExperimentAdapter` or steps
//! simulation ticks.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use anima_engine_lib::core::experiment::{
    ExperimentManifest, FactorDiff, ObservableRegistry, MANIFEST_SCHEMA_VERSION,
    MAX_ENSEMBLE_RESULT_BYTES,
};
use anima_engine_lib::core::live_experiment::{
    all_live_observable_ids, LiveWorldConfig, LIVE_KEYS, LIVE_KEY_DRIFT_CONTROL,
    LIVE_KEY_MUTATION_RATE, LIVE_KEY_PREY_CAP, LIVE_KEY_RESOURCE_CAPACITY_SCALE,
};

#[allow(dead_code)]
#[path = "../examples/run_sel2p_experiment.rs"]
mod runner;

const PREREG_DIR: &str = "tests/fixtures/experiments_sel2p";
const PREREG_FILE: &str = "sel2p-preregistration.json";

const SELECTED: &str = "sel2p-selected.json";
const SCARCE_MUT0: &str = "sel2p-scarce-mut0.json";
const SCARCE_DRIFT: &str = "sel2p-scarce-drift.json";
const BASELINE_REF: &str = "sel2p-baseline-ref.json";
const SMOKE_SELECTED: &str = "sel2p-smoke-selected.json";
const SMOKE_MUT0: &str = "sel2p-smoke-scarce-mut0.json";
const SMOKE_DRIFT: &str = "sel2p-smoke-scarce-drift.json";

const ALL_MANIFESTS: [&str; 7] = [
    SELECTED,
    SCARCE_MUT0,
    SCARCE_DRIFT,
    BASELINE_REF,
    SMOKE_SELECTED,
    SMOKE_MUT0,
    SMOKE_DRIFT,
];

fn dir_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PREREG_DIR)
}

fn load_manifest(name: &str) -> ExperimentManifest {
    let raw = std::fs::read_to_string(dir_path().join(name))
        .unwrap_or_else(|e| panic!("read {name}: {e}"));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("{name} parses as ExperimentManifest: {e}"))
}

fn load_json(rel: &str) -> serde_json::Value {
    let raw = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|e| panic!("read {rel}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{rel} parses as JSON: {e}"))
}

fn prereg() -> serde_json::Value {
    load_json(&format!("{PREREG_DIR}/{PREREG_FILE}"))
}

fn ic_map(m: &ExperimentManifest) -> BTreeMap<String, f64> {
    m.initial_conditions.values.iter().cloned().collect()
}

fn diff_keys(a: &ExperimentManifest, b: &ExperimentManifest) -> Vec<String> {
    let (ma, mb) = (ic_map(a), ic_map(b));
    let mut keys: Vec<String> = ma.keys().chain(mb.keys()).cloned().collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter(|k| ma.get(k) != mb.get(k))
        .collect()
}

#[test]
fn every_sel2p_manifest_parses_validates_uses_only_live_keys_and_fits_memory_ceiling() {
    let reg = ObservableRegistry::live_default();
    reg.validate().expect("live registry must validate");

    for name in ALL_MANIFESTS {
        let m = load_manifest(name);
        assert_eq!(m.schema_version, MANIFEST_SCHEMA_VERSION, "{name}");
        m.validate(&reg)
            .unwrap_or_else(|e| panic!("{name} failed manifest validate: {e}"));

        for (k, _) in &m.initial_conditions.values {
            assert!(
                LIVE_KEYS.contains(&k.as_str()),
                "{name} declares unknown live key '{k}'"
            );
        }

        let cfg = LiveWorldConfig::from_initial_conditions(&m.initial_conditions)
            .unwrap_or_else(|e| panic!("{name} failed LiveWorldConfig parse: {e}"));
        cfg.ensure_observables_supported(&m.observable_ids)
            .unwrap_or_else(|e| panic!("{name} failed ensure_observables_supported: {e}"));

        assert!(
            m.estimated_result_bytes(&reg) < MAX_ENSEMBLE_RESULT_BYTES,
            "{name} exceeds MAX_ENSEMBLE_RESULT_BYTES"
        );
    }
}

#[test]
fn confirmation_arms_differ_only_in_declared_keys_and_share_pairing_structure() {
    let sel = load_manifest(SELECTED);
    let mut0 = load_manifest(SCARCE_MUT0);
    let drift = load_manifest(SCARCE_DRIFT);

    let allowed = FactorDiff {
        allowed_paths: vec!["initial_conditions".to_string()],
    };
    assert_eq!(
        allowed
            .validate(&sel, &drift)
            .expect("selected vs scarce_drift FactorDiff"),
        vec!["initial_conditions".to_string()]
    );
    assert_eq!(
        diff_keys(&sel, &drift),
        vec![LIVE_KEY_DRIFT_CONTROL.to_string()]
    );

    assert_eq!(
        allowed
            .validate(&sel, &mut0)
            .expect("selected vs scarce_mut0 FactorDiff"),
        vec!["initial_conditions".to_string()]
    );
    assert_eq!(
        diff_keys(&sel, &mut0),
        vec![LIVE_KEY_MUTATION_RATE.to_string()]
    );

    let sel_ic = ic_map(&sel);
    let mut0_ic = ic_map(&mut0);
    let drift_ic = ic_map(&drift);
    assert_eq!(sel_ic.get(LIVE_KEY_MUTATION_RATE), Some(&0.05));
    assert_eq!(mut0_ic.get(LIVE_KEY_MUTATION_RATE), Some(&0.0));
    assert_eq!(drift_ic.get(LIVE_KEY_MUTATION_RATE), Some(&0.05));
    assert_eq!(sel_ic.get(LIVE_KEY_DRIFT_CONTROL), Some(&0.0));
    assert_eq!(mut0_ic.get(LIVE_KEY_DRIFT_CONTROL), Some(&0.0));
    assert_eq!(drift_ic.get(LIVE_KEY_DRIFT_CONTROL), Some(&1.0));
    assert_eq!(drift_ic.get("live.drift_reset_ticks"), Some(&0.0));

    let fixed_common: [(&str, f64); 15] = [
        ("live.founders", 100.0),
        ("live.predator_fraction", 0.0),
        ("live.trees", 8.0),
        ("live.lakes", 16.0),
        ("live.lake_layout", 1.0),
        ("live.resource_capacity_scale", 0.63),
        ("live.plant_reserve", 0.0),
        ("live.prey_cap", 320.0),
        ("live.dehydration_ticks", 960.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
        ("live.locomotion", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
        ("live.evolved_brains", 0.0),
    ];
    for name in [SELECTED, SCARCE_MUT0, SCARCE_DRIFT] {
        let ic = ic_map(&load_manifest(name));
        for (k, v) in fixed_common {
            assert_eq!(ic.get(k), Some(&v), "{name} missing/mismatched {k}");
        }
        assert!(
            !ic.contains_key("live.starve_ticks") && !ic.contains_key("live.food_cap"),
            "{name} keeps default starve_ticks and food_cap"
        );
    }

    for other in [&mut0, &drift] {
        assert_eq!(sel.seeds, other.seeds);
        assert_eq!(sel.duration_ticks, other.duration_ticks);
        assert_eq!(sel.sample_period, other.sample_period);
        assert_eq!(sel.world_identity, other.world_identity);
        assert_eq!(sel.laws, other.laws);
        assert_eq!(sel.observable_ids, other.observable_ids);
        assert!(other.interventions.is_empty() && other.exotic_interventions.is_empty());
    }
    assert_eq!(sel.duration_ticks, 200_000);
    assert_eq!(sel.sample_period, 500);
}

#[test]
fn baseline_ref_differs_from_smoke_selected_only_in_prey_cap_scale_and_seeds() {
    let base = load_manifest(BASELINE_REF);
    let smoke_sel = load_manifest(SMOKE_SELECTED);

    let allowed = FactorDiff {
        allowed_paths: vec!["initial_conditions".to_string(), "seeds".to_string()],
    };
    let mut diffs = allowed
        .validate(&base, &smoke_sel)
        .expect("baseline_ref vs smoke_selected FactorDiff");
    diffs.sort();
    assert_eq!(
        diffs,
        vec!["initial_conditions".to_string(), "seeds".to_string()]
    );

    assert_eq!(
        diff_keys(&base, &smoke_sel),
        vec![
            LIVE_KEY_PREY_CAP.to_string(),
            LIVE_KEY_RESOURCE_CAPACITY_SCALE.to_string(),
        ]
    );
    let base_ic = ic_map(&base);
    assert_eq!(base_ic.get(LIVE_KEY_PREY_CAP), Some(&80.0));
    assert_eq!(base_ic.get(LIVE_KEY_RESOURCE_CAPACITY_SCALE), Some(&1.0));
    assert_eq!(base.duration_ticks, 200_000);
    assert_eq!(base.sample_period, 50);
    assert_eq!(smoke_sel.duration_ticks, 200_000);
    assert_eq!(smoke_sel.sample_period, 50);
}

#[test]
fn drift_arm_omits_cause_split_observables_and_smoke_manifests_are_blind_to_body_mass() {
    let all_emitted = all_live_observable_ids();
    let expected_confirm: Vec<&str> = all_emitted
        .into_iter()
        .filter(|id| *id != "live.deaths_starvation" && *id != "live.deaths_dehydration")
        .collect();
    assert!(expected_confirm.contains(&"live.mean_energy_integral"));
    assert!(expected_confirm.contains(&"live.trait_mass_mean"));
    assert!(expected_confirm.contains(&"live.trait_mass_var"));

    for name in [SELECTED, SCARCE_MUT0, SCARCE_DRIFT] {
        let m = load_manifest(name);
        let declared: Vec<&str> = m.observable_ids.iter().map(String::as_str).collect();
        assert_eq!(
            declared, expected_confirm,
            "{name} must declare the 25 drift-compatible live observables (draft dependency #4)"
        );
    }

    // Prove ensure_observables_supported rejects adding cause-split observables onto scarce_drift.
    let mut bad_drift = load_manifest(SCARCE_DRIFT);
    let drf_cfg = LiveWorldConfig::from_initial_conditions(&bad_drift.initial_conditions)
        .expect("drift config");
    for cause_id in ["live.deaths_starvation", "live.deaths_dehydration"] {
        bad_drift.observable_ids.push(cause_id.to_string());
        assert!(
            drf_cfg
                .ensure_observables_supported(&bad_drift.observable_ids)
                .is_err(),
            "drift config must reject {cause_id}"
        );
        bad_drift.observable_ids.pop();
    }

    // Smoke non-drift arms carry cause-split observables for C4; smoke drift omits them; NONE carry trait_mass_*.
    for name in [BASELINE_REF, SMOKE_SELECTED, SMOKE_MUT0] {
        let raw = std::fs::read_to_string(dir_path().join(name)).expect("read smoke manifest");
        assert!(
            !raw.contains("trait_mass"),
            "{name} raw JSON must not contain 'trait_mass'"
        );
        let m = load_manifest(name);
        assert!(
            m.observable_ids
                .iter()
                .any(|x| x == "live.deaths_starvation"),
            "{name} needs live.deaths_starvation for C4"
        );
        assert!(
            m.observable_ids
                .iter()
                .any(|x| x == "live.deaths_dehydration"),
            "{name} needs live.deaths_dehydration for C4"
        );
        assert!(
            !m.observable_ids.iter().any(|x| x.contains("trait_mass")),
            "{name} must remain blind to body mass"
        );
    }
    let smoke_drift_raw =
        std::fs::read_to_string(dir_path().join(SMOKE_DRIFT)).expect("read smoke drift manifest");
    assert!(
        !smoke_drift_raw.contains("trait_mass"),
        "{SMOKE_DRIFT} raw JSON must not contain 'trait_mass'"
    );
    let smoke_drift = load_manifest(SMOKE_DRIFT);
    assert!(
        !smoke_drift
            .observable_ids
            .iter()
            .any(|x| x == "live.deaths_starvation" || x == "live.deaths_dehydration"),
        "smoke drift arm must omit cause-split observables"
    );
    assert!(
        !smoke_drift
            .observable_ids
            .iter()
            .any(|x| x.contains("trait_mass")),
        "smoke drift arm must remain blind to body mass"
    );
}

#[test]
fn all_seed_sets_are_pairwise_disjoint_and_disjoint_from_prior_experiments() {
    let p = prereg();
    let expected_confirm: Vec<u64> = (0..20).map(|k| 780_001 + 1_000 * k).collect();
    let expected_baseline: Vec<u64> = (799_501..=799_506).collect();
    let smoke_seed = 999_961_u64;

    for name in [SELECTED, SCARCE_MUT0, SCARCE_DRIFT] {
        assert_eq!(load_manifest(name).seeds, expected_confirm, "{name}");
    }
    assert_eq!(load_manifest(BASELINE_REF).seeds, expected_baseline);
    for name in [SMOKE_SELECTED, SMOKE_MUT0, SMOKE_DRIFT] {
        assert_eq!(load_manifest(name).seeds, vec![smoke_seed], "{name}");
    }

    let pc_base: Vec<u64> = p["seeds"]["pipeline_check"]["baseline_seeds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    let pc_smoke = p["seeds"]["pipeline_check"]["smoke_seed"].as_u64().unwrap();
    let pc_pair: Vec<u64> = p["seeds"]["pipeline_check"]["pair_seeds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert_eq!(pc_base.len(), 6);
    assert_eq!(pc_pair.len(), 2);

    let exploration: Vec<u64> = (1..=6).collect();
    let mut sel2p_all = BTreeSet::new();
    for (label, list) in [
        ("exploration", exploration.as_slice()),
        ("baseline", expected_baseline.as_slice()),
        ("confirmation", expected_confirm.as_slice()),
        ("smoke", &[smoke_seed]),
        ("pipeline_baseline", pc_base.as_slice()),
        ("pipeline_smoke", &[pc_smoke]),
        ("pipeline_pair", pc_pair.as_slice()),
    ] {
        for &s in list {
            assert!(
                sel2p_all.insert(s),
                "seed {s} duplicated across SEL2-P sets (caught at {label})"
            );
        }
    }

    // Disjoint from committed E2 and E-SEL1 preregistrations plus E3 (730001..750000) and E2R (750001..760000).
    let e2 = load_json("tests/fixtures/experiments_e2/e2-preregistration.json");
    let esel1 = load_json("tests/fixtures/experiments_esel1/esel1-preregistration.json");
    let mut prior_seeds: BTreeSet<u64> = BTreeSet::new();
    for arr in [
        e2["seeds"]["execution_order"].as_array().unwrap(),
        esel1["seeds"]["execution_order"].as_array().unwrap(),
    ] {
        for v in arr {
            prior_seeds.insert(v.as_u64().unwrap());
        }
    }
    if let Some(s) = e2["seeds"]["smoke_seed"].as_u64() {
        prior_seeds.insert(s);
    }
    if let Some(s) = esel1["seeds"]["smoke_seed"].as_u64() {
        prior_seeds.insert(s);
    }
    for s in 700_001..=779_999 {
        prior_seeds.insert(s);
    }

    for s in &sel2p_all {
        if !exploration.contains(s) {
            assert!(
                !prior_seeds.contains(s),
                "SEL2-P seed {s} overlaps prior E2/E3/E2R/E-SEL1 ranges"
            );
        }
    }
}

#[test]
fn preregistration_json_matches_draft_and_manifests() {
    let p = prereg();
    assert_eq!(p["status"].as_str(), Some("PROPOSED"));
    assert_eq!(p["contains_results"], serde_json::json!(false));
    assert_eq!(p["model_version"].as_str(), Some("live-bevy-world/2"));
    assert_eq!(p["snapshot_schema_version"].as_u64(), Some(8));
    assert_eq!(p["h1_direction"].as_str(), Some("two_sided"));

    assert_eq!(p["seeds"]["n_requested"].as_u64(), Some(20));
    assert_eq!(
        p["seeds"]["min_complete_pairs_for_a_decision"].as_u64(),
        Some(16)
    );
    assert_eq!(p["seeds"]["min_surviving_baseline_seeds"].as_u64(), Some(3));
    assert_eq!(p["seeds"]["smoke_seed"].as_u64(), Some(999_961));

    assert_eq!(p["smoke_gate"]["decision_id"].as_str(), Some("QD-SEL2g"));
    assert_eq!(
        p["smoke_gate"]["reference_computed_before_scarce_observables"],
        serde_json::json!(true)
    );
    assert_eq!(
        p["smoke_gate"]["c4_applies_to_arms"],
        serde_json::json!(["selected", "scarce_mut0"])
    );
    assert_eq!(
        p["smoke_gate"]["c4_excluded_arms"],
        serde_json::json!(["scarce_drift"])
    );

    assert_eq!(p["thresholds"]["min_abs_paired_dz"].as_f64(), Some(0.66));
    assert_eq!(
        p["thresholds"]["min_relative_shift_vs_control_mean"].as_f64(),
        Some(0.05)
    );

    for (k, v) in p["commands"].as_object().unwrap() {
        let cmd = v.as_str().unwrap();
        assert!(
            !cmd.contains("cargo run"),
            "command {k} must not use cargo run"
        );
    }
}

#[test]
fn pooled_reference_gate_arithmetic_on_hand_built_examples() {
    use runner::{
        compute_pooled_baseline, evaluate_smoke_gate, post_warmup_energy_mean, post_warmup_mean,
        tail20_mean, BaselineObservation, ScarceGateInput,
    };

    // 4 surviving of 6 baseline seeds (two extinct excluded from denominator).
    let baseline_runs = vec![
        BaselineObservation {
            seed: 799_501,
            completed: true,
            extinct_tick: Some(8_545),
            crop_second_half: Some(153_586.9),
        },
        BaselineObservation {
            seed: 799_502,
            completed: true,
            extinct_tick: None,
            crop_second_half: Some(113_000.0),
        },
        BaselineObservation {
            seed: 799_503,
            completed: true,
            extinct_tick: Some(20_787),
            crop_second_half: Some(153_586.9),
        },
        BaselineObservation {
            seed: 799_504,
            completed: true,
            extinct_tick: None,
            crop_second_half: Some(113_400.0),
        },
        BaselineObservation {
            seed: 799_505,
            completed: true,
            extinct_tick: None,
            crop_second_half: Some(113_800.0),
        },
        BaselineObservation {
            seed: 799_506,
            completed: true,
            extinct_tick: None,
            crop_second_half: Some(114_200.0),
        },
    ];
    let pool = compute_pooled_baseline(&baseline_runs, 3);
    assert!(pool.sufficient);
    assert_eq!(pool.n_surviving, 4);
    assert_eq!(
        pool.surviving_seeds,
        vec![799_502, 799_504, 799_505, 799_506]
    );
    let expected_pool = (113_000.0 + 113_400.0 + 113_800.0 + 114_200.0) / 4.0;
    assert!((pool.pooled_crop_second_half.unwrap() - expected_pool).abs() < 1e-9);

    let arm = |name: &'static str, crop: f64| ScarceGateInput {
        arm: name,
        completed: true,
        extinct_tick: None,
        min_agents: Some(12.0),
        mean_agents: Some(100.0),
        crop_second_half: Some(crop),
        total_births: 3_500.0,
        total_deaths: 3_500.0,
        mean_lifespan: Some(5_000.0),
        total_starvation: Some(2_500.0),
        total_dehydration: Some(1_000.0),
        cause_partition_ok: Some(true),
        births_last_quarter: 800.0,
        deaths_last_quarter: 800.0,
    };
    let drift_arm = ScarceGateInput {
        arm: "scarce_drift",
        total_starvation: None,
        total_dehydration: None,
        cause_partition_ok: None,
        ..arm("scarce_drift", expected_pool * 0.5)
    };

    // Passing case (g = 0.5 in [0.4, 0.6], C4 ignores drift_arm having None cause counts).
    let pass = evaluate_smoke_gate(
        200_000,
        &pool,
        &arm("selected", expected_pool * 0.5),
        &arm("scarce_mut0", expected_pool * 0.52),
        &drift_arm,
    );
    assert!(pass.gate_pass);
    assert!((pass.g_selected.unwrap() - 0.5).abs() < 1e-12);
    assert!((pass.g_scarce_mut0.unwrap() - 0.52).abs() < 1e-12);

    // Fewer than 3 surviving baseline seeds -> C2 fails.
    let small_pool = compute_pooled_baseline(&baseline_runs[..3], 3);
    assert!(!small_pool.sufficient);
    let fail_pool = evaluate_smoke_gate(
        200_000,
        &small_pool,
        &arm("selected", expected_pool * 0.5),
        &arm("scarce_mut0", expected_pool * 0.5),
        &drift_arm,
    );
    assert!(!fail_pool.c2 && !fail_pool.gate_pass);

    // Out-of-band g -> C2 fails.
    let fail_g = evaluate_smoke_gate(
        200_000,
        &pool,
        &arm("selected", expected_pool * 0.61),
        &arm("scarce_mut0", expected_pool * 0.5),
        &drift_arm,
    );
    assert!(!fail_g.c2 && !fail_g.gate_pass);

    // Starvation share == 0.50 (not strictly >) -> C4 fails.
    let mut bad_c4 = arm("selected", expected_pool * 0.5);
    bad_c4.total_starvation = Some(1_750.0);
    bad_c4.total_dehydration = Some(1_750.0);
    let fail_c4 = evaluate_smoke_gate(
        200_000,
        &pool,
        &bad_c4,
        &arm("scarce_mut0", expected_pool * 0.5),
        &drift_arm,
    );
    assert!(!fail_c4.c4 && !fail_c4.gate_pass);

    // Post-warmup mean, S2 energy integral difference, and tail20 math.
    let mass_series = [(20_000, 3.3), (50_000, 3.5), (170_000, 3.7), (200_000, 3.9)];
    assert!((post_warmup_mean(&mass_series, 20_000).unwrap() - 3.7).abs() < 1e-12);
    assert!((tail20_mean(&mass_series, 200_000).unwrap() - 3.8).abs() < 1e-12);
    let energy_series = [(20_000, 10_000.0), (200_000, 100_000.0)];
    assert!(
        (post_warmup_energy_mean(&energy_series, 20_000, 200_000).unwrap() - 0.5).abs() < 1e-12
    );
}

#[test]
fn confirm_mode_refuses_to_run_while_preregistration_is_proposed() {
    let tmp = std::env::temp_dir().join(format!(
        "sel2p_confirm_refuse_{}_{}",
        std::process::id(),
        file!().len()
    ));
    let err = runner::run_with_args(vec![
        "--confirm".to_string(),
        "--manifest-dir".to_string(),
        dir_path().to_string_lossy().into_owned(),
        "--out".to_string(),
        tmp.to_string_lossy().into_owned(),
    ])
    .expect_err("PROPOSED preregistration must refuse --confirm");
    assert!(
        err.contains("PROPOSED"),
        "unexpected refusal message: {err}"
    );
}

#[test]
fn sel2p_thresholds_and_smoke_arm_scope_constants_match_preregistration() {
    let p = prereg();
    let sg = &p["smoke_gate"];
    let th = &p["thresholds"];

    assert_eq!(sg["c1_min_agents"].as_f64(), Some(runner::C1_MIN_AGENTS));
    assert_eq!(
        sg["c2_g_band"]
            .as_array()
            .expect("c2_g_band")
            .iter()
            .filter_map(|v| v.as_f64())
            .collect::<Vec<_>>(),
        vec![runner::C2_G_LOW, runner::C2_G_HIGH]
    );
    assert_eq!(
        sg["c3_min_generations"].as_f64(),
        Some(runner::C3_MIN_GENERATIONS)
    );
    assert_eq!(
        sg["c4_min_starvation_share"].as_f64(),
        Some(runner::C4_MIN_STARVATION_SHARE)
    );
    assert_eq!(
        th["min_abs_paired_dz"].as_f64(),
        Some(runner::MATERIALITY_DZ)
    );
    assert_eq!(
        th["min_relative_shift_vs_control_mean"].as_f64(),
        Some(runner::MATERIALITY_RELATIVE_SHIFT)
    );

    let as_strs = |key: &str| -> Vec<String> {
        sg[key]
            .as_array()
            .unwrap_or_else(|| panic!("missing smoke_gate.{key}"))
            .iter()
            .map(|v| v.as_str().expect("string arm").to_string())
            .collect()
    };
    let expected_arms =
        |arr: &[&str]| -> Vec<String> { arr.iter().map(|s| (*s).to_string()).collect() };
    assert_eq!(
        as_strs("c1_applies_to_arms"),
        expected_arms(&runner::C1_APPLIES_TO_ARMS)
    );
    assert_eq!(
        as_strs("c2_applies_to_arms"),
        expected_arms(&runner::C2_APPLIES_TO_ARMS)
    );
    assert_eq!(
        as_strs("c2_diagnostic_only_arms"),
        vec!["scarce_drift".to_string()]
    );
    assert_eq!(
        as_strs("c3_applies_to_arms"),
        expected_arms(&runner::C3_APPLIES_TO_ARMS)
    );
    assert_eq!(
        as_strs("c4_applies_to_arms"),
        expected_arms(&runner::C4_APPLIES_TO_ARMS)
    );
    assert_eq!(
        as_strs("c5_applies_to_arms"),
        expected_arms(&runner::C5_APPLIES_TO_ARMS)
    );
}

#[test]
fn sel2p_smoke_and_confirmation_manifest_pairs_share_world_laws_ic_and_durations() {
    let pairs = [
        ("sel2p-smoke-selected.json", "sel2p-selected.json"),
        ("sel2p-smoke-scarce-mut0.json", "sel2p-scarce-mut0.json"),
        ("sel2p-smoke-scarce-drift.json", "sel2p-scarce-drift.json"),
    ];
    for (smoke_name, confirm_name) in pairs {
        let sm = load_manifest(smoke_name);
        let cm = load_manifest(confirm_name);
        assert_eq!(sm.world_identity, cm.world_identity, "{smoke_name}");
        assert_eq!(sm.laws, cm.laws, "{smoke_name}");
        assert_eq!(sm.initial_conditions, cm.initial_conditions, "{smoke_name}");
        assert_eq!(sm.interventions, cm.interventions, "{smoke_name}");
        assert_eq!(
            sm.exotic_interventions, cm.exotic_interventions,
            "{smoke_name}"
        );
        assert_eq!(sm.observer, cm.observer, "{smoke_name}");
        assert_eq!(sm.duration_ticks, cm.duration_ticks, "{smoke_name}");
    }

    for name in [
        "sel2p-selected.json",
        "sel2p-scarce-mut0.json",
        "sel2p-scarce-drift.json",
    ] {
        let m = load_manifest(name);
        assert_eq!(m.duration_ticks, 200_000, "{name}");
        assert_eq!(m.sample_period, 500, "{name}");
    }
    for name in [
        "sel2p-baseline-ref.json",
        "sel2p-smoke-selected.json",
        "sel2p-smoke-scarce-mut0.json",
        "sel2p-smoke-scarce-drift.json",
    ] {
        let m = load_manifest(name);
        assert_eq!(m.duration_ticks, 200_000, "{name}");
        assert_eq!(m.sample_period, 50, "{name}");
    }

    // D6 runner rejection test: mutating a smoke manifest's initial condition or duration
    // must be rejected before any simulation starts.
    let tmp = std::env::temp_dir().join(format!(
        "sel2p_d6_mismatch_{}_{}",
        std::process::id(),
        line!()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    let mdir = tmp.join("manifests");
    let odir = tmp.join("out");
    std::fs::create_dir_all(&mdir).expect("create temp manifest dir");
    for name in [
        "sel2p-preregistration.json",
        "sel2p-selected.json",
        "sel2p-scarce-mut0.json",
        "sel2p-scarce-drift.json",
        "sel2p-baseline-ref.json",
        "sel2p-smoke-selected.json",
        "sel2p-smoke-scarce-mut0.json",
        "sel2p-smoke-scarce-drift.json",
    ] {
        std::fs::copy(dir_path().join(name), mdir.join(name)).expect("copy fixture");
    }
    let mut bad_smoke: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(mdir.join("sel2p-smoke-selected.json")).expect("read smoke"),
    )
    .expect("parse smoke");
    bad_smoke["duration_ticks"] = serde_json::json!(150_000);
    std::fs::write(
        mdir.join("sel2p-smoke-selected.json"),
        serde_json::to_string_pretty(&bad_smoke).unwrap(),
    )
    .expect("write drifted smoke");

    let err = runner::run_with_args(vec![
        "--pipeline-check".to_string(),
        "--manifest-dir".to_string(),
        mdir.to_string_lossy().into_owned(),
        "--out".to_string(),
        odir.to_string_lossy().into_owned(),
    ])
    .expect_err("drifted smoke duration_ticks must fail D6 manifest validation");
    assert!(err.contains("duration_ticks"), "unexpected D6 error: {err}");

    for smoke_file in [
        "sel2p-smoke-selected.json",
        "sel2p-smoke-scarce-mut0.json",
        "sel2p-smoke-scarce-drift.json",
    ] {
        let mut bad_ic: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir_path().join(smoke_file)).expect("read smoke manifest"),
        )
        .expect("parse smoke manifest");
        let mut mutated = false;
        for entry in bad_ic["initial_conditions"]["values"]
            .as_array_mut()
            .expect("initial_conditions.values array")
        {
            if entry[0].as_str() == Some("live.prey_cap") {
                entry[1] = serde_json::json!(321);
                mutated = true;
            }
        }
        assert!(mutated, "{smoke_file} must contain live.prey_cap entry");
        std::fs::write(
            mdir.join(smoke_file),
            serde_json::to_string_pretty(&bad_ic).unwrap(),
        )
        .expect("write drifted smoke ic");
    }
    let err_ic = runner::run_with_args(vec![
        "--pipeline-check".to_string(),
        "--manifest-dir".to_string(),
        mdir.to_string_lossy().into_owned(),
        "--out".to_string(),
        odir.to_string_lossy().into_owned(),
    ])
    .expect_err("drifted smoke initial_conditions must fail D6 manifest validation");
    assert!(
        err_ic.contains("D6:") && err_ic.contains("initial_conditions"),
        "unexpected D6 ic error: {err_ic}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn sel2p_confirm_prerequisite_gate_rejects_missing_or_mismatched_smoke_and_hashes() {
    let tmp =
        std::env::temp_dir().join(format!("sel2p_d3_gate_{}_{}", std::process::id(), line!()));
    let _ = std::fs::remove_dir_all(&tmp);
    let smoke_dir = tmp.join("smoke");
    std::fs::create_dir_all(&smoke_dir).expect("create temp smoke dir");

    let names = [
        "sel2p-preregistration.json",
        "sel2p-selected.json",
        "sel2p-scarce-mut0.json",
        "sel2p-scarce-drift.json",
        "sel2p-baseline-ref.json",
        "sel2p-smoke-selected.json",
        "sel2p-smoke-scarce-mut0.json",
        "sel2p-smoke-scarce-drift.json",
    ];
    let mut hashes = BTreeMap::new();
    for (idx, name) in names.iter().enumerate() {
        hashes.insert((*name).to_string(), format!("{idx:064x}"));
    }

    let p = prereg();
    let base_lock = || {
        serde_json::json!({
            "smoke_gate_passed": true,
            "duration_ticks": 200_000,
            "warmup_ticks": 20_000,
            "sample_period": 500,
            "baseline_surviving_count": 4,
            "pooled_baseline_crop": 150_000.0,
            "manifest_sha256": hashes,
        })
    };
    let write_smoke =
        |mode: &str, duration: u64, period: u64, gate_pass: bool, suf: bool, n: u64, crop: f64| {
            let rep = serde_json::json!({
                "mode": mode,
                "duration_ticks": duration,
                "sample_period": period,
                "gate": { "gate_pass": gate_pass },
                "pooled_baseline": {
                    "sufficient": suf,
                    "n_surviving": n,
                    "pooled_crop_second_half": crop,
                }
            });
            std::fs::write(
                smoke_dir.join("smoke-report.json"),
                serde_json::to_string_pretty(&rep).unwrap(),
            )
            .expect("write smoke-report.json");
        };

    // 1. Missing smoke-report.json fails even with complete lock + hashes.
    let err_no_smoke = runner::validate_confirm_prerequisites_json(&tmp, &base_lock(), &p, &hashes)
        .expect_err("missing smoke-report.json must fail");
    assert!(err_no_smoke.contains("smoke-report.json"), "{err_no_smoke}");

    // 2. Write valid smoke report: full gate must pass.
    write_smoke("smoke", 200_000, 50, true, true, 4, 150_000.0);
    runner::validate_confirm_prerequisites_json(&tmp, &base_lock(), &p, &hashes)
        .expect("valid smoke report and 8 manifest hashes must pass");

    // 3. Missing manifest_sha256 fails.
    let mut lock_no_hashes = base_lock();
    lock_no_hashes
        .as_object_mut()
        .unwrap()
        .remove("manifest_sha256");
    let err = runner::validate_confirm_prerequisites_json(&tmp, &lock_no_hashes, &p, &hashes)
        .expect_err("missing manifest_sha256 must fail");
    assert!(err.contains("manifest_sha256"), "{err}");

    // 4. Incomplete 7-of-8 manifest_sha256 fails.
    let mut lock_partial = base_lock();
    lock_partial["manifest_sha256"]
        .as_object_mut()
        .unwrap()
        .remove("sel2p-scarce-drift.json");
    let err = runner::validate_confirm_prerequisites_json(&tmp, &lock_partial, &p, &hashes)
        .expect_err("incomplete manifest_sha256 must fail");
    assert!(err.contains("manifest_sha256"), "{err}");

    // 5. Wrong manifest hash fails.
    let mut lock_bad_hash = base_lock();
    lock_bad_hash["manifest_sha256"]["sel2p-selected.json"] = serde_json::json!("f".repeat(64));
    let err = runner::validate_confirm_prerequisites_json(&tmp, &lock_bad_hash, &p, &hashes)
        .expect_err("wrong manifest sha256 must fail");
    assert!(err.contains("sha256"), "{err}");

    // 6. pipeline-check smoke report fails.
    write_smoke("pipeline-check", 200_000, 50, true, true, 4, 150_000.0);
    let err = runner::validate_confirm_prerequisites_json(&tmp, &base_lock(), &p, &hashes)
        .expect_err("pipeline-check smoke report must fail");
    assert!(err.contains("mode"), "{err}");

    // 7. Failing gate_pass fails.
    write_smoke("smoke", 200_000, 50, false, true, 4, 150_000.0);
    let err = runner::validate_confirm_prerequisites_json(&tmp, &base_lock(), &p, &hashes)
        .expect_err("gate_pass=false must fail");
    assert!(err.contains("gate_pass"), "{err}");

    // 8. Mismatched pooled crop and surviving count fail.
    write_smoke("smoke", 200_000, 50, true, true, 4, 151_000.0);
    let err = runner::validate_confirm_prerequisites_json(&tmp, &base_lock(), &p, &hashes)
        .expect_err("mismatched pooled_crop_second_half must fail");
    assert!(err.contains("pooled_crop_second_half"), "{err}");

    write_smoke("smoke", 200_000, 50, true, true, 5, 150_000.0);
    let err = runner::validate_confirm_prerequisites_json(&tmp, &base_lock(), &p, &hashes)
        .expect_err("mismatched n_surviving must fail");
    assert!(err.contains("n_surviving"), "{err}");

    let _ = std::fs::remove_dir_all(&tmp);
}
