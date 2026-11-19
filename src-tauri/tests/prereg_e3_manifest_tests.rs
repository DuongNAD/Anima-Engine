//! The E3 preregistration, checked by machine rather than trusted as prose.
//!
//! Pins the committed E3 manifests ("does a sustained regional productivity gradient widen
//! population body-mass dispersion?") and their agreement with
//! `docs/ai/planning/2026-08-26-experiment-e3-productivity-gradient-body-mass.md`.
//!
//! Never constructs a `LiveExperimentAdapter`, never steps a tick, and never touches the runner.

use anima_engine_lib::core::experiment::{
    ExperimentManifest, FactorDiff, ObservableRegistry, MANIFEST_SCHEMA_VERSION,
    MAX_ENSEMBLE_RESULT_BYTES,
};
use anima_engine_lib::core::intervention::{Curve, InterventionKind, Region};
use anima_engine_lib::core::live_experiment::{
    all_live_observable_ids, LiveWorldConfig, LIVE_KEYS, LIVE_KEY_DRIVE_FLOOR,
    LIVE_KEY_ECOLOGICAL_FITNESS, LIVE_KEY_EVOLVED_BRAINS, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS,
    LIVE_KEY_LAKES, LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY, LIVE_KEY_PREDATOR_FRACTION,
    LIVE_KEY_REPRODUCTION, LIVE_KEY_THIRST_STEERING, LIVE_KEY_TREES,
};
use std::collections::BTreeSet;

const PREREG_DIR: &str = "tests/fixtures/experiments_e3";
const SMOKE_SEED: u64 = 999_979;
const SEED_BASE: u64 = 730_001;
const SEED_STEP: u64 = 1_000;
const SEED_COUNT: usize = 12;

const CONTROL: &str = "e3-control-uniform.json";
const TREATMENT: &str = "e3-treatment-gradient.json";
const SMOKE_CONTROL: &str = "e3-smoke-control-uniform.json";
const SMOKE_TREATMENT: &str = "e3-smoke-treatment-gradient.json";

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(PREREG_DIR)
        .join(name)
}

fn load_manifest(name: &str) -> ExperimentManifest {
    let path = fixture_path(name);
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("{name} parses as ExperimentManifest: {e}"))
}

fn prereg() -> serde_json::Value {
    let path = fixture_path("e3-preregistration.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw).expect("e3-preregistration.json parses as JSON")
}

fn plan_doc_text() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs/ai/planning/2026-08-26-experiment-e3-productivity-gradient-body-mass.md");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn every_preregistered_manifest_parses_validates_and_supports_all_observables() {
    let reg = ObservableRegistry::live_default();
    reg.validate().expect("live registry must be valid");
    for name in [CONTROL, TREATMENT, SMOKE_CONTROL, SMOKE_TREATMENT] {
        let m = load_manifest(name);
        assert_eq!(m.schema_version, MANIFEST_SCHEMA_VERSION, "{name}");
        m.validate(&reg)
            .unwrap_or_else(|e| panic!("{name} must validate: {e}"));
        for (k, _) in &m.initial_conditions.values {
            assert!(
                LIVE_KEYS.contains(&k.as_str()),
                "{name} declares unknown live key '{k}'"
            );
        }
        let cfg = LiveWorldConfig::from_initial_conditions(&m.initial_conditions)
            .unwrap_or_else(|e| panic!("{name} LiveWorldConfig must parse: {e}"));
        cfg.ensure_observables_supported(&m.observable_ids)
            .unwrap_or_else(|e| panic!("{name} must support requested observables: {e}"));
    }
}

#[test]
fn the_declared_factor_is_exactly_the_two_regional_intervention_intensities() {
    for (ctrl_name, treat_name) in [(CONTROL, TREATMENT), (SMOKE_CONTROL, SMOKE_TREATMENT)] {
        let c = load_manifest(ctrl_name);
        let t = load_manifest(treat_name);

        let allowed = FactorDiff {
            allowed_paths: vec!["interventions".to_string()],
        };
        let diffs = allowed
            .validate(&c, &t)
            .unwrap_or_else(|e| panic!("{ctrl_name} vs {treat_name}: {e}"));
        assert_eq!(
            diffs,
            vec!["interventions".to_string()],
            "{ctrl_name} vs {treat_name} must differ only at interventions"
        );
        assert_ne!(
            c.fingerprint(),
            t.fingerprint(),
            "{ctrl_name} vs {treat_name} must have distinct fingerprints"
        );

        assert_eq!(c.interventions.len(), 2, "{ctrl_name} keeps both commands");
        assert_eq!(t.interventions.len(), 2, "{treat_name} keeps both commands");

        let (c0, c1) = (&c.interventions[0], &c.interventions[1]);
        let (t0, t1) = (&t.interventions[0], &t.interventions[1]);

        assert_eq!(c0.id, 1);
        assert_eq!(c0.cause_id, 1);
        assert_eq!(c0.kind, InterventionKind::Deforest);
        assert_eq!(
            c0.region,
            Region::Rect {
                min_x: 0,
                min_y: 0,
                max_x: 127,
                max_y: 255,
            }
        );
        assert_eq!(c0.start_tick, 1);
        assert_eq!(c0.duration_ticks, c.duration_ticks);
        assert_eq!(c0.curve, Curve::Step);
        assert!(!c0.reversible);
        assert!(!c0.signed_negative);
        assert_eq!(c0.intensity, 0.0);

        assert_eq!(c1.id, 2);
        assert_eq!(c1.cause_id, 2);
        assert_eq!(c1.kind, InterventionKind::AddNutrient);
        assert_eq!(
            c1.region,
            Region::Rect {
                min_x: 128,
                min_y: 0,
                max_x: 255,
                max_y: 255,
            }
        );
        assert_eq!(c1.start_tick, 1);
        assert_eq!(c1.duration_ticks, c.duration_ticks);
        assert_eq!(c1.curve, Curve::Step);
        assert!(!c1.reversible);
        assert!(!c1.signed_negative);
        assert_eq!(c1.intensity, 0.0);

        let mut t0_as_ctrl = t0.clone();
        t0_as_ctrl.intensity = 0.0;
        assert_eq!(t0_as_ctrl, *c0, "West Deforest differs only in intensity");
        assert_eq!(t0.intensity, 0.3f32);

        let mut t1_as_ctrl = t1.clone();
        t1_as_ctrl.intensity = 0.0;
        assert_eq!(
            t1_as_ctrl, *c1,
            "East AddNutrient differs only in intensity"
        );
        assert_eq!(t1.intensity, 1.0f32);
    }
}

#[test]
fn the_arms_and_smoke_manifests_share_everything_the_pairing_depends_on() {
    let c = load_manifest(CONTROL);
    let t = load_manifest(TREATMENT);
    let sc = load_manifest(SMOKE_CONTROL);
    let st = load_manifest(SMOKE_TREATMENT);

    assert_eq!(c.seeds, t.seeds);
    assert_eq!(c.duration_ticks, t.duration_ticks);
    assert_eq!(c.sample_period, t.sample_period);
    assert_eq!(c.world_identity, t.world_identity);
    assert_eq!(c.laws, t.laws);
    assert_eq!(c.initial_conditions, t.initial_conditions);
    assert_eq!(c.observable_ids, t.observable_ids);
    assert_eq!(c.exotic_interventions, t.exotic_interventions);
    assert_eq!(c.observer, t.observer);

    for (exp, smk, label) in [(&c, &sc, "control"), (&t, &st, "treatment")] {
        let allowed = FactorDiff {
            allowed_paths: vec!["seeds".to_string()],
        };
        let diffs = allowed
            .validate(exp, smk)
            .unwrap_or_else(|e| panic!("smoke {label} differs beyond seeds: {e}"));
        assert_eq!(diffs, vec!["seeds".to_string()]);
    }

    let expected_keys: BTreeSet<&str> = [
        LIVE_KEY_ECOLOGICAL_FITNESS,
        LIVE_KEY_MORTALITY,
        LIVE_KEY_REPRODUCTION,
        LIVE_KEY_LOCOMOTION,
        LIVE_KEY_DRIVE_FLOOR,
        LIVE_KEY_THIRST_STEERING,
        LIVE_KEY_EVOLVED_BRAINS,
        LIVE_KEY_FOUNDERS,
        LIVE_KEY_PREDATOR_FRACTION,
        LIVE_KEY_TREES,
        LIVE_KEY_LAKES,
        LIVE_KEY_FOOD_CAP,
    ]
    .into_iter()
    .collect();
    let actual_keys: BTreeSet<&str> = c
        .initial_conditions
        .values
        .iter()
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(actual_keys, expected_keys);
}

#[test]
fn twelve_experimental_seeds_follow_the_rule_and_smoke_seed_is_excluded() {
    let c = load_manifest(CONTROL);
    assert_eq!(c.seeds.len(), SEED_COUNT);
    let mut unique = c.seeds.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), SEED_COUNT);

    let expected: Vec<u64> = (0..SEED_COUNT as u64)
        .map(|k| SEED_BASE + SEED_STEP * k)
        .collect();
    assert_eq!(c.seeds, expected);

    for (name, m) in [
        (CONTROL, load_manifest(CONTROL)),
        (TREATMENT, load_manifest(TREATMENT)),
    ] {
        assert!(
            !m.seeds.contains(&SMOKE_SEED),
            "{name} must not declare smoke seed {SMOKE_SEED}"
        );
    }
    for name in [SMOKE_CONTROL, SMOKE_TREATMENT] {
        let m = load_manifest(name);
        assert_eq!(
            m.seeds,
            vec![SMOKE_SEED],
            "{name} must declare only smoke seed {SMOKE_SEED}"
        );
    }
}

#[test]
fn the_requested_observables_and_s2_regional_secondary_metrics_are_registered() {
    let reg = ObservableRegistry::live_default();
    let expected_ids = all_live_observable_ids();
    assert_eq!(expected_ids.len(), 27);

    let c = load_manifest(CONTROL);
    let declared: Vec<&str> = c.observable_ids.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        declared, expected_ids,
        "manifest must request all 27 live observables in emission order"
    );
    for id in &declared {
        let spec = reg
            .get(id)
            .unwrap_or_else(|| panic!("'{id}' missing from ObservableRegistry"));
        assert!(!spec.unit.is_empty(), "{id} needs a unit");
        assert!(spec.cadence_period > 0, "{id} needs positive cadence");
    }

    let p = prereg();
    let hypotheses = p["hypotheses"].as_array().expect("hypotheses is a list");
    let h1 = hypotheses
        .iter()
        .find(|h| h["id"].as_str() == Some("H1"))
        .expect("H1 hypothesis present");
    assert_eq!(h1["role"].as_str(), Some("primary"));
    assert_eq!(h1["observable"].as_str(), Some("live.trait_mass_var"));
    assert_eq!(h1["direction"].as_str(), Some("positive"));

    let mut secondary_observables = BTreeSet::new();
    for h in hypotheses {
        if let Some(obs) = h["observable"].as_str() {
            assert!(
                expected_ids.contains(&obs),
                "hypothesis {} names unknown observable '{obs}'",
                h["id"]
            );
            if h["role"].as_str() == Some("secondary") {
                secondary_observables.insert(obs);
            }
        }
        if let Some(arr) = h["observables"].as_array() {
            for item in arr {
                let obs = item.as_str().expect("observable id string");
                assert!(
                    expected_ids.contains(&obs),
                    "hypothesis {} names unknown observable '{obs}'",
                    h["id"]
                );
                if h["role"].as_str() == Some("secondary") {
                    secondary_observables.insert(obs);
                }
            }
        }
    }
    assert!(
        secondary_observables.contains("live.trait_mass_mean_west"),
        "S2 West observable must be registered as secondary"
    );
    assert!(
        secondary_observables.contains("live.trait_mass_mean_east"),
        "S2 East observable must be registered as secondary"
    );
}

#[test]
fn the_ensemble_fits_the_memory_ceiling_and_valid_sampling_grid() {
    let reg = ObservableRegistry::live_default();
    let c = load_manifest(CONTROL);
    assert_eq!(c.duration_ticks % c.sample_period, 0);
    assert_eq!(c.samples_per_run(), 30);

    let bytes = c.estimated_result_bytes(&reg);
    assert!(
        bytes < MAX_ENSEMBLE_RESULT_BYTES,
        "estimated bytes {bytes} must fit under {MAX_ENSEMBLE_RESULT_BYTES}"
    );

    let p = prereg();
    let ladder: Vec<u64> = p["duration"]["duration_ticks_ladder"]
        .as_array()
        .expect("ladder is a list")
        .iter()
        .map(|v| v.as_u64().expect("integer rung"))
        .collect();
    assert_eq!(ladder, vec![18_000, 12_000, 6_000]);
    assert_eq!(ladder.first(), Some(&c.duration_ticks));
    assert!(ladder.windows(2).all(|w| w[0] > w[1]));
}

#[test]
fn the_preregistration_json_agrees_with_manifests_and_plan_doc() {
    let p = prereg();
    let c = load_manifest(CONTROL);

    assert_eq!(p["contains_results"], serde_json::json!(false));
    assert_eq!(p["status"].as_str(), Some("FROZEN"));

    let planned: Vec<u64> = p["seeds"]["execution_order"]
        .as_array()
        .expect("execution_order")
        .iter()
        .map(|v| v.as_u64().expect("integer seed"))
        .collect();
    assert_eq!(planned, c.seeds);
    assert_eq!(p["seeds"]["smoke_seed"].as_u64(), Some(SMOKE_SEED));
    assert_eq!(p["seeds"]["n_requested"].as_u64(), Some(SEED_COUNT as u64));

    assert_eq!(
        p["duration"]["duration_ticks"].as_u64(),
        Some(c.duration_ticks)
    );
    assert_eq!(
        p["duration"]["sample_period"].as_u64(),
        Some(c.sample_period)
    );
    assert_eq!(
        p["duration"]["samples_per_run"].as_u64(),
        Some(c.samples_per_run())
    );
    assert_eq!(p["duration"]["warmup_ticks"].as_u64(), Some(3_000));

    let expected_ticks = (c.seeds.len() as u64) * 2 * c.duration_ticks;
    assert_eq!(p["cost"]["total_ticks"].as_u64(), Some(expected_ticks));

    let common = p["common_initial_conditions"]
        .as_object()
        .expect("common_initial_conditions is an object");
    assert_eq!(common.len(), c.initial_conditions.values.len());
    for (k, v) in &c.initial_conditions.values {
        assert_eq!(
            common.get(k).and_then(|x| x.as_f64()),
            Some(*v),
            "common_initial_conditions['{k}'] disagrees with control manifest"
        );
    }

    assert_eq!(p["manifests"]["control"].as_str(), Some(CONTROL));
    assert_eq!(p["manifests"]["treatment"].as_str(), Some(TREATMENT));
    assert_eq!(
        p["manifests"]["smoke_control"].as_str(),
        Some(SMOKE_CONTROL)
    );
    assert_eq!(
        p["manifests"]["smoke_treatment"].as_str(),
        Some(SMOKE_TREATMENT)
    );

    let smoke_cmd = p["smoke_command"].as_str().expect("smoke_command");
    assert!(!smoke_cmd.contains("cargo run"));
    assert!(smoke_cmd.contains("--release"));
    assert!(smoke_cmd.contains("e3_smoke"));

    assert_eq!(
        p["wp2_smoke_calibration"]["a_resource_field_present"]["passed"],
        serde_json::json!(true)
    );
    assert_eq!(
        p["wp2_smoke_calibration"]["b_standing_crop_change"]["passed"],
        serde_json::json!(true)
    );
    assert_eq!(
        p["wp2_smoke_calibration"]["c_detritus_before_3000"]["passed"],
        serde_json::json!(true)
    );
    assert_eq!(
        p["wp2_smoke_calibration"]["body_mass_consulted"],
        serde_json::json!(false)
    );
    assert_eq!(
        p["wp2_smoke_calibration"]["all_27_observables_emitted_and_finite"],
        serde_json::json!(true)
    );
    assert_eq!(
        p["wp3_locked_schedule"]["duration_ticks"].as_u64(),
        Some(c.duration_ticks)
    );
    assert_eq!(
        p["wp3_locked_schedule"]["warmup_ticks"].as_u64(),
        Some(3_000)
    );
    assert_eq!(
        p["wp3_locked_schedule"]["experimental_seeds_run"].as_u64(),
        Some(0)
    );

    let doc = plan_doc_text();
    let doc_norm = doc.replace("\r\n", "\n");
    let front_matter = doc_norm
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .expect("plan doc must start with YAML front matter")
        .0;
    let status_line = front_matter
        .lines()
        .find(|line| line.starts_with("status:"))
        .expect("plan doc front matter must declare status");
    assert_ne!(
        status_line.trim(),
        "status: frozen",
        "front-matter status must stay a policy-valid value, never 'frozen'"
    );
    assert_eq!(
        status_line.trim(),
        "status: active",
        "WP-4 frozen preregistration must declare policy-valid 'status: active'"
    );
    for needle in [
        "live.trait_mass_var",
        "live.trait_mass_mean_west",
        "live.trait_mass_mean_east",
        "999979",
        "730001",
        "2026-11-19",
        "91def40",
        "live.reproduction",
        "live.thirst_steering",
        "27 observable",
    ] {
        assert!(
            doc.contains(needle),
            "plan doc must name '{needle}' before any experimental run"
        );
    }
}
