//! The E3 preregistration, checked by machine rather than trusted as prose.
//!
//! Pins the committed E3 three-arm manifests, agreement with
//! `docs/ai/planning/2026-08-26-experiment-e3-productivity-gradient-body-mass.md`, agreement with
//! the blind WP-2/WP-3 smoke calibration artifacts, pairwise seed disjointness, empty-half and
//! extinction exclusion rules (D1), S4/S5 structural-zero demotions (D2), and materiality
//! threshold decision boundaries.
//!
//! Never constructs a `LiveExperimentAdapter`, never steps a tick, and never runs experimental
//! seeds.

use anima_engine_lib::core::experiment::{
    ExperimentManifest, FactorDiff, ObservableRegistry, MANIFEST_SCHEMA_VERSION,
    MAX_ENSEMBLE_RESULT_BYTES,
};
use anima_engine_lib::core::experiment_runner::{paired_effect, PairedEffect};
use anima_engine_lib::core::intervention::{Curve, InterventionKind, Region};
use anima_engine_lib::core::live_experiment::{
    all_live_observable_ids, mass_statistics, LiveWorldConfig, LIVE_KEYS, LIVE_KEY_DRIVE_FLOOR,
    LIVE_KEY_ECOLOGICAL_FITNESS, LIVE_KEY_EVOLVED_BRAINS, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS,
    LIVE_KEY_LAKES, LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY, LIVE_KEY_PREDATOR_FRACTION,
    LIVE_KEY_REPRODUCTION, LIVE_KEY_RESOURCE_CAPACITY_SCALE, LIVE_KEY_THIRST_STEERING,
    LIVE_KEY_TREES,
};
use std::collections::{BTreeMap, BTreeSet};

const PREREG_DIR: &str = "tests/fixtures/experiments_e3";
const SMOKE_SEED: u64 = 999_979;
const SEED_BASE: u64 = 730_001;
const SEED_STEP: u64 = 1_000;
const SEED_COUNT: usize = 12;
const EMITTED_AT_REGISTRATION: usize = 27;

const CONTROL: &str = "e3-control-uniform.json";
const CONTROL_SCARCITY: &str = "e3-control-uniform-scarcity.json";
const TREATMENT: &str = "e3-treatment-gradient.json";
const SMOKE_CONTROL: &str = "e3-smoke-control-uniform.json";
const SMOKE_CONTROL_SCARCITY: &str = "e3-smoke-control-uniform-scarcity.json";
const SMOKE_TREATMENT: &str = "e3-smoke-treatment-gradient.json";

const ALL_MANIFESTS: [&str; 6] = [
    CONTROL,
    CONTROL_SCARCITY,
    TREATMENT,
    SMOKE_CONTROL,
    SMOKE_CONTROL_SCARCITY,
    SMOKE_TREATMENT,
];

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(PREREG_DIR)
        .join(name)
}

fn load_json(rel_from_crate: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_from_crate);
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{} parses as JSON: {e}", path.display()))
}

fn load_manifest(name: &str) -> ExperimentManifest {
    let path = fixture_path(name);
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("{name} parses as ExperimentManifest: {e}"))
}

fn prereg() -> serde_json::Value {
    load_json(&format!("{PREREG_DIR}/e3-preregistration.json"))
}

fn plan_doc_text() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs/ai/planning/2026-08-26-experiment-e3-productivity-gradient-body-mass.md");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn ic_map(m: &ExperimentManifest) -> BTreeMap<String, f64> {
    m.initial_conditions.values.iter().cloned().collect()
}

#[test]
fn every_preregistered_manifest_parses_validates_and_supports_all_observables() {
    let reg = ObservableRegistry::live_default();
    reg.validate().expect("live registry must be valid");
    for name in ALL_MANIFESTS {
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
fn the_declared_factors_match_the_three_arm_confound_free_design() {
    for (ctrl_u_name, ctrl_s_name, treat_name) in [
        (CONTROL, CONTROL_SCARCITY, TREATMENT),
        (SMOKE_CONTROL, SMOKE_CONTROL_SCARCITY, SMOKE_TREATMENT),
    ] {
        let cu = load_manifest(ctrl_u_name);
        let cs = load_manifest(ctrl_s_name);
        let tr = load_manifest(treat_name);

        let allow_interventions = FactorDiff {
            allowed_paths: vec!["interventions".to_string()],
        };
        let diffs_ut = allow_interventions
            .validate(&cu, &tr)
            .unwrap_or_else(|e| panic!("{ctrl_u_name} vs {treat_name}: {e}"));
        assert_eq!(diffs_ut, vec!["interventions".to_string()]);

        let allow_scarcity = FactorDiff {
            allowed_paths: vec![
                "initial_conditions".to_string(),
                "interventions".to_string(),
            ],
        };
        let mut diffs_us = allow_scarcity
            .validate(&cu, &cs)
            .unwrap_or_else(|e| panic!("{ctrl_u_name} vs {ctrl_s_name}: {e}"));
        diffs_us.sort();
        assert_eq!(
            diffs_us,
            vec![
                "initial_conditions".to_string(),
                "interventions".to_string()
            ]
        );
        let mut diffs_st = allow_scarcity
            .validate(&cs, &tr)
            .unwrap_or_else(|e| panic!("{ctrl_s_name} vs {treat_name}: {e}"));
        diffs_st.sort();
        assert_eq!(
            diffs_st,
            vec![
                "initial_conditions".to_string(),
                "interventions".to_string()
            ]
        );

        assert_ne!(cu.fingerprint(), cs.fingerprint());
        assert_ne!(cu.fingerprint(), tr.fingerprint());
        assert_ne!(cs.fingerprint(), tr.fingerprint());

        // Check exact initial_conditions difference on live.resource_capacity_scale only.
        let cu_ic = ic_map(&cu);
        let cs_ic = ic_map(&cs);
        let tr_ic = ic_map(&tr);
        assert_eq!(cu_ic, tr_ic, "abundant control and treatment share all ICs");
        assert_eq!(cu_ic.get(LIVE_KEY_RESOURCE_CAPACITY_SCALE), Some(&1.0));
        assert_eq!(cs_ic.get(LIVE_KEY_RESOURCE_CAPACITY_SCALE), Some(&0.42));
        assert_eq!(tr_ic.get(LIVE_KEY_RESOURCE_CAPACITY_SCALE), Some(&1.0));
        for k in cu_ic.keys() {
            if k != LIVE_KEY_RESOURCE_CAPACITY_SCALE {
                assert_eq!(cu_ic.get(k), cs_ic.get(k), "{k} must match scarcity arm");
            }
        }

        // Check exact intervention parameters across all 3 arms.
        assert_eq!(cu.interventions.len(), 2);
        assert_eq!(cs.interventions.len(), 2);
        assert_eq!(tr.interventions.len(), 2);

        let (cu0, cu1) = (&cu.interventions[0], &cu.interventions[1]);
        let (cs0, cs1) = (&cs.interventions[0], &cs.interventions[1]);
        let (tr0, tr1) = (&tr.interventions[0], &tr.interventions[1]);

        let west_half = Region::Rect {
            min_x: 0,
            min_y: 0,
            max_x: 127,
            max_y: 255,
        };
        let east_half = Region::Rect {
            min_x: 128,
            min_y: 0,
            max_x: 255,
            max_y: 255,
        };
        let whole_map = Region::Rect {
            min_x: 0,
            min_y: 0,
            max_x: 255,
            max_y: 255,
        };

        assert_eq!(cu0.id, 1);
        assert_eq!(cu0.cause_id, 1);
        assert_eq!(cu0.kind, InterventionKind::Deforest);
        assert_eq!(cu0.region, west_half);
        assert_eq!(cu0.start_tick, 1);
        assert_eq!(cu0.duration_ticks, cu.duration_ticks);
        assert_eq!(cu0.curve, Curve::Step);
        assert!(!cu0.reversible);
        assert!(!cu0.signed_negative);
        assert_eq!(cu0.intensity, 0.0);
        assert_eq!(cs0, cu0, "scarcity arm keeps zero West Deforest");

        let mut tr0_as_ctrl = tr0.clone();
        tr0_as_ctrl.intensity = 0.0;
        assert_eq!(tr0_as_ctrl, *cu0);
        assert_eq!(tr0.intensity, 0.3f32);

        assert_eq!(cu1.id, 2);
        assert_eq!(cu1.cause_id, 2);
        assert_eq!(cu1.kind, InterventionKind::AddNutrient);
        assert_eq!(cu1.region, east_half);
        assert_eq!(cu1.start_tick, 1);
        assert_eq!(cu1.duration_ticks, cu.duration_ticks);
        assert_eq!(cu1.curve, Curve::Step);
        assert!(!cu1.reversible);
        assert!(!cu1.signed_negative);
        assert_eq!(cu1.intensity, 0.0);

        let mut tr1_as_ctrl = tr1.clone();
        tr1_as_ctrl.intensity = 0.0;
        assert_eq!(tr1_as_ctrl, *cu1);
        assert_eq!(tr1.intensity, 1.0f32);

        assert_eq!(cs1.id, 2);
        assert_eq!(cs1.cause_id, 2);
        assert_eq!(cs1.kind, InterventionKind::AddNutrient);
        assert_eq!(cs1.region, whole_map);
        assert_eq!(cs1.start_tick, 1);
        assert_eq!(cs1.duration_ticks, cs.duration_ticks);
        assert_eq!(cs1.curve, Curve::Step);
        assert!(!cs1.reversible);
        assert!(!cs1.signed_negative);
        assert_eq!(cs1.intensity, 1.0f32);
    }
}

#[test]
fn the_arms_and_smoke_manifests_share_everything_the_pairing_depends_on() {
    let cu = load_manifest(CONTROL);
    let cs = load_manifest(CONTROL_SCARCITY);
    let tr = load_manifest(TREATMENT);
    let scu = load_manifest(SMOKE_CONTROL);
    let scs = load_manifest(SMOKE_CONTROL_SCARCITY);
    let str_m = load_manifest(SMOKE_TREATMENT);

    for other in [&cs, &tr] {
        assert_eq!(cu.seeds, other.seeds);
        assert_eq!(cu.duration_ticks, other.duration_ticks);
        assert_eq!(cu.sample_period, other.sample_period);
        assert_eq!(cu.world_identity, other.world_identity);
        assert_eq!(cu.laws, other.laws);
        assert_eq!(cu.observable_ids, other.observable_ids);
        assert_eq!(cu.exotic_interventions, other.exotic_interventions);
        assert_eq!(cu.observer, other.observer);
    }
    assert_eq!(cu.initial_conditions, tr.initial_conditions);

    for (exp, smk, label) in [
        (&cu, &scu, "control_uniform"),
        (&cs, &scs, "control_uniform_scarcity"),
        (&tr, &str_m, "treatment_gradient"),
    ] {
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
        LIVE_KEY_RESOURCE_CAPACITY_SCALE,
    ]
    .into_iter()
    .collect();
    for name in [CONTROL, CONTROL_SCARCITY, TREATMENT] {
        let m = load_manifest(name);
        let actual_keys: BTreeSet<&str> = m
            .initial_conditions
            .values
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(
            actual_keys, expected_keys,
            "{name} must declare all 13 explicit initial_conditions keys"
        );
    }
}

#[test]
fn twelve_experimental_seeds_follow_the_rule_and_smoke_seed_is_excluded() {
    let expected: Vec<u64> = (0..SEED_COUNT as u64)
        .map(|k| SEED_BASE + SEED_STEP * k)
        .collect();

    for name in [CONTROL, CONTROL_SCARCITY, TREATMENT] {
        let m = load_manifest(name);
        assert_eq!(m.seeds.len(), SEED_COUNT, "{name}");
        let mut unique = m.seeds.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), SEED_COUNT, "{name}");
        assert_eq!(m.seeds, expected, "{name}");
        assert!(
            !m.seeds.contains(&SMOKE_SEED),
            "{name} must not declare smoke seed {SMOKE_SEED}"
        );
    }
    for name in [SMOKE_CONTROL, SMOKE_CONTROL_SCARCITY, SMOKE_TREATMENT] {
        let m = load_manifest(name);
        assert_eq!(
            m.seeds,
            vec![SMOKE_SEED],
            "{name} must declare only smoke seed {SMOKE_SEED}"
        );
    }
}

#[test]
fn all_e3_seeds_are_disjoint_from_other_experiments() {
    let e2 = load_json("tests/fixtures/experiments_e2/e2-preregistration.json");
    let esel1 = load_json("tests/fixtures/experiments_esel1/esel1-preregistration.json");
    let sel2p = load_json("tests/fixtures/experiments_sel2p/sel2p-preregistration.json");

    let mut other_seeds = BTreeSet::new();
    for arr in [
        e2["seeds"]["execution_order"].as_array().unwrap(),
        esel1["seeds"]["execution_order"].as_array().unwrap(),
        sel2p["seeds"]["execution_order"].as_array().unwrap(),
        sel2p["seeds"]["baseline_seeds"].as_array().unwrap(),
        sel2p["seeds"]["excluded_exploration_seeds"]
            .as_array()
            .unwrap(),
        sel2p["seeds"]["pipeline_check"]["baseline_seeds"]
            .as_array()
            .unwrap(),
        sel2p["seeds"]["pipeline_check"]["pair_seeds"]
            .as_array()
            .unwrap(),
    ] {
        for v in arr {
            other_seeds.insert(v.as_u64().unwrap());
        }
    }
    for s in [
        e2["seeds"]["smoke_seed"].as_u64(),
        esel1["seeds"]["smoke_seed"].as_u64(),
        sel2p["seeds"]["smoke_seed"].as_u64(),
        sel2p["seeds"]["pipeline_check"]["smoke_seed"].as_u64(),
    ]
    .into_iter()
    .flatten()
    {
        other_seeds.insert(s);
    }
    // Reserved E2R range (750001..=760000) and AE integration fixture seeds (1..=3).
    other_seeds.extend(750_001..=760_000);
    other_seeds.extend(1..=3);

    let e3_exp = load_manifest(CONTROL).seeds;
    for s in e3_exp.iter().copied().chain(std::iter::once(SMOKE_SEED)) {
        assert!(
            !other_seeds.contains(&s),
            "E3 seed {s} collides with another experiment's registered or reserved seed set"
        );
    }
}

#[test]
fn the_requested_observables_s2_regional_metrics_and_structural_zero_demotions_are_registered() {
    let reg = ObservableRegistry::live_default();
    let all_emitted = all_live_observable_ids();
    assert!(
        all_emitted.len() >= EMITTED_AT_REGISTRATION,
        "live adapter must emit at least the {EMITTED_AT_REGISTRATION} observables frozen at E3 registration"
    );
    // D3 fix: compare against the frozen registration prefix so future observables appended after
    // position 27 do not spuriously break E3's frozen manifests (matching E-SEL1 QD-E2M).
    let expected_prefix = &all_emitted[..EMITTED_AT_REGISTRATION];

    for name in [CONTROL, CONTROL_SCARCITY, TREATMENT] {
        let m = load_manifest(name);
        let declared: Vec<&str> = m.observable_ids.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            declared, expected_prefix,
            "{name} must request the {EMITTED_AT_REGISTRATION} frozen registration observables in emission order"
        );
        for required_named in [
            "live.trait_mass_var",
            "live.trait_mass_p10",
            "live.trait_mass_p90",
            "live.trait_mass_mean",
            "live.trait_mass_mean_west",
            "live.trait_mass_mean_east",
            "live.standing_crop",
            "live.closed_eu_total",
            "live.agent_count",
            "live.births",
            "live.deaths",
            "live.mean_lifespan",
            "live.shannon",
            "live.archive_coverage",
            "plants",
            "detritus",
        ] {
            assert!(
                declared.contains(&required_named),
                "{name} must name registered observable '{required_named}'"
            );
            let spec = reg
                .get(required_named)
                .unwrap_or_else(|| panic!("'{required_named}' missing from ObservableRegistry"));
            assert!(!spec.unit.is_empty(), "{required_named} needs a unit");
            assert!(
                spec.cadence_period > 0,
                "{required_named} needs positive cadence"
            );
        }
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
    assert_eq!(
        h1["primary_contrast"].as_str(),
        Some("treatment_gradient - control_uniform_scarcity")
    );

    let mut secondary_observables = BTreeSet::new();
    let mut roles_by_id = BTreeMap::new();
    for h in hypotheses {
        let id = h["id"].as_str().expect("hypothesis id");
        let role = h["role"].as_str().expect("hypothesis role");
        roles_by_id.insert(id, role);
        if let Some(obs) = h["observable"].as_str() {
            assert!(
                expected_prefix.contains(&obs),
                "hypothesis {id} names unknown observable '{obs}'"
            );
            if role == "secondary" {
                secondary_observables.insert(obs);
            }
        }
        if let Some(arr) = h["observables"].as_array() {
            for item in arr {
                let obs = item.as_str().expect("observable id string");
                assert!(
                    expected_prefix.contains(&obs),
                    "hypothesis {id} names unknown observable '{obs}'"
                );
                if role == "secondary" {
                    secondary_observables.insert(obs);
                }
            }
        }
    }

    for sec_id in ["S1", "S2", "S3", "S6"] {
        assert_eq!(
            roles_by_id.get(sec_id).copied(),
            Some("secondary"),
            "{sec_id} must remain an interpreted secondary hypothesis"
        );
    }
    // D2 fix: S4 and S5 must be explicitly demoted before experimental data.
    for demoted_id in ["S4", "S5"] {
        let h = hypotheses
            .iter()
            .find(|h| h["id"].as_str() == Some(demoted_id))
            .unwrap_or_else(|| panic!("{demoted_id} present"));
        assert_eq!(
            h["role"].as_str(),
            Some("reported_not_interpreted_structural_zero"),
            "{demoted_id} must be demoted from interpreted secondary status (D2)"
        );
        assert_eq!(h["decision_effect"].as_str(), Some("none"));
        assert!(!h["structural_status"].as_str().unwrap_or("").is_empty());
    }
    assert!(
        secondary_observables.contains("live.trait_mass_mean_west")
            && secondary_observables.contains("live.trait_mass_mean_east"),
        "S2 West/East observables must remain registered as active secondary S3"
    );
}

#[test]
fn the_ensemble_fits_the_memory_ceiling_and_valid_sampling_grid() {
    let reg = ObservableRegistry::live_default();
    for name in [CONTROL, CONTROL_SCARCITY, TREATMENT] {
        let m = load_manifest(name);
        assert_eq!(m.duration_ticks % m.sample_period, 0);
        assert_eq!(m.samples_per_run(), 30);

        let bytes = m.estimated_result_bytes(&reg);
        assert!(
            bytes < MAX_ENSEMBLE_RESULT_BYTES,
            "{name}: estimated bytes {bytes} must fit under {MAX_ENSEMBLE_RESULT_BYTES}"
        );
    }

    let p = prereg();
    let ladder: Vec<u64> = p["duration"]["duration_ticks_ladder"]
        .as_array()
        .expect("ladder is a list")
        .iter()
        .map(|v| v.as_u64().expect("integer rung"))
        .collect();
    assert_eq!(ladder, vec![18_000, 12_000, 6_000]);
    assert_eq!(ladder.first(), Some(&load_manifest(CONTROL).duration_ticks));
    assert!(ladder.windows(2).all(|w| w[0] > w[1]));
}

fn parse_leading_localized_number(slice: &str) -> Option<f64> {
    let trimmed = slice.trim_start();
    let end = trimmed
        .find(|c: char| !c.is_ascii_digit() && c != ',' && c != '.')
        .unwrap_or(trimmed.len());
    let token = trimmed[..end].trim_end_matches([',', '.']);
    if token.is_empty() {
        return None;
    }
    token.replace(',', ".").parse::<f64>().ok()
}

fn collect_numbers_after(text: &str, marker: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(pos) = rest.find(marker) {
        let after = &rest[pos + marker.len()..];
        if let Some(v) = parse_leading_localized_number(after) {
            out.push(v);
        }
        rest = after;
    }
    out
}

#[test]
fn the_preregistration_json_agrees_with_manifests_plan_doc_and_smoke_artifacts() {
    let p = prereg();
    let cu = load_manifest(CONTROL);

    assert_eq!(p["contains_results"], serde_json::json!(false));
    assert_eq!(p["status"].as_str(), Some("FROZEN"));
    assert_eq!(
        p["design_confound_resolution"]["chosen_option"].as_str(),
        Some("add_uniform_scarcity_control_arm")
    );
    // R2 & N4 pins.
    let sec_contrasts = p["design_confound_resolution"]["secondary_contrasts"]
        .as_array()
        .expect("secondary_contrasts");
    let su_label = sec_contrasts
        .get(1)
        .and_then(|v| v.as_str())
        .expect("S-U contrast label");
    assert!(
        su_label.contains("whole-map AddNutrient refill; control_uniform has none"),
        "R2 mismatch in secondary_contrasts[1]: {su_label}"
    );
    assert!(
        !su_label.contains("matched AddNutrient"),
        "R2 forbids false 'matched AddNutrient' label"
    );
    let scarcity_deriv = p["design_confound_resolution"]["scarcity_arm_analytic_derivation"]
        .as_str()
        .expect("scarcity_arm_analytic_derivation");
    assert!(
        !scarcity_deriv.contains("BEFORE seeing any outcomes"),
        "N4 forbids overclaiming 'BEFORE seeing any outcomes'"
    );
    let nutrient_note = p["design_confound_resolution"]["why_uniform_whole_map_add_nutrient"]
        .as_str()
        .expect("why_uniform_whole_map_add_nutrient");
    assert!(
        nutrient_note.contains("-0.44%")
            && nutrient_note.contains("-1.86%")
            && nutrient_note.contains("-1.68%"),
        "N3 requires post-warmup (-0.44%), final (-1.86%), and tail (-1.68%) mismatches"
    );

    let planned: Vec<u64> = p["seeds"]["execution_order"]
        .as_array()
        .expect("execution_order")
        .iter()
        .map(|v| v.as_u64().expect("integer seed"))
        .collect();
    assert_eq!(planned, cu.seeds);
    assert_eq!(p["seeds"]["smoke_seed"].as_u64(), Some(SMOKE_SEED));
    assert_eq!(p["seeds"]["n_requested"].as_u64(), Some(SEED_COUNT as u64));
    assert_eq!(
        p["seeds"]["min_complete_pairs_for_a_decision"].as_u64(),
        Some(10)
    );

    assert_eq!(
        p["duration"]["duration_ticks"].as_u64(),
        Some(cu.duration_ticks)
    );
    assert_eq!(
        p["duration"]["sample_period"].as_u64(),
        Some(cu.sample_period)
    );
    assert_eq!(
        p["duration"]["samples_per_run"].as_u64(),
        Some(cu.samples_per_run())
    );
    assert_eq!(p["duration"]["warmup_ticks"].as_u64(), Some(3_000));

    let expected_ticks = (cu.seeds.len() as u64) * 3 * cu.duration_ticks;
    assert_eq!(p["cost"]["arms"].as_u64(), Some(3));
    assert_eq!(p["cost"]["runs"].as_u64(), Some(36));
    assert_eq!(p["cost"]["total_ticks"].as_u64(), Some(expected_ticks));

    let shared = p["common_initial_conditions"]["shared_across_all_arms"]
        .as_object()
        .expect("shared_across_all_arms is an object");
    assert_eq!(shared.len(), 12);
    let cu_ic = ic_map(&cu);
    for (k, v) in shared {
        assert_eq!(cu_ic.get(k), Some(&v.as_f64().unwrap()));
    }
    assert_eq!(
        p["common_initial_conditions"]["per_arm_values"]["control_uniform"].as_f64(),
        Some(1.0)
    );
    assert_eq!(
        p["common_initial_conditions"]["per_arm_values"]["control_uniform_scarcity"].as_f64(),
        Some(0.42)
    );
    assert_eq!(
        p["common_initial_conditions"]["per_arm_values"]["treatment_gradient"].as_f64(),
        Some(1.0)
    );

    assert_eq!(p["manifests"]["control"].as_str(), Some(CONTROL));
    assert_eq!(
        p["manifests"]["control_uniform_scarcity"].as_str(),
        Some(CONTROL_SCARCITY)
    );
    assert_eq!(p["manifests"]["treatment"].as_str(), Some(TREATMENT));
    assert_eq!(
        p["manifests"]["smoke_control"].as_str(),
        Some(SMOKE_CONTROL)
    );
    assert_eq!(
        p["manifests"]["smoke_control_scarcity"].as_str(),
        Some(SMOKE_CONTROL_SCARCITY)
    );
    assert_eq!(
        p["manifests"]["smoke_treatment"].as_str(),
        Some(SMOKE_TREATMENT)
    );

    // D1 / R1 per-contrast exclusion rules & N5 exact pins.
    for rule_key in ["extinction_rule", "min_agent_count_rule", "empty_half_rule"] {
        assert_eq!(
            p[rule_key]["exclusion_scope"].as_str(),
            Some("per_contrast"),
            "R1: {rule_key}.exclusion_scope must be 'per_contrast'"
        );
    }
    let ext_def = p["extinction_rule"]["definition"]
        .as_str()
        .expect("extinction_rule.definition");
    assert!(
        ext_def.contains("live.agent_count == 0")
            && ext_def.contains("any sampled tick")
            && ext_def.contains("final_observables at T = 18000"),
        "N5: extinction_rule.definition weakened: {ext_def}"
    );
    let ext_hand = p["extinction_rule"]["handling"]
        .as_str()
        .expect("extinction_rule.handling");
    assert!(
        ext_hand.contains("per contrast")
            && ext_hand.contains("counts only S and T runs")
            && ext_hand.contains("valid in all three arms"),
        "R1: extinction_rule.handling must specify per-contrast scope and all-three-arms additive table"
    );
    assert!(!p["empty_half_rule"]["definition"]
        .as_str()
        .unwrap_or("")
        .is_empty());
    assert_eq!(
        p["min_agent_count_rule"]["definition"].as_str(),
        Some("live.agent_count < 2 in final_observables at T = 18000 (or at any sample in 14400..=18000 for H1_tail_window)")
    );
    assert_eq!(
        p["min_agent_count_rule"]["applicable_endpoints"],
        serde_json::json!(["H1", "H1_tail_window", "S1"])
    );
    let report_fields: Vec<&str> = p["reporting"]["per_observable_fields"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for req_field in [
        "n_extinct_pairs_dropped",
        "n_singleton_pairs_dropped",
        "n_empty_half_pairs_dropped_s3",
    ] {
        assert!(
            report_fields.contains(&req_field),
            "reporting.per_observable_fields missing '{req_field}'"
        );
    }
    let derived_fields: Vec<&str> = p["reporting"]["additional_derived"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for req_derived in [
        "h1_bessel_corrected_sensitivity_var_n_over_n_minus_1",
        "agent_count_at_T_by_arm",
        "additive_decomposition_table_on_all_three_valid_seeds",
    ] {
        assert!(
            derived_fields.contains(&req_derived),
            "reporting.additional_derived missing '{req_derived}'"
        );
    }

    // N1, N6, N7, N8, N10 pins.
    let hypotheses = p["hypotheses"].as_array().unwrap();
    let h1 = hypotheses.iter().find(|h| h["id"] == "H1").unwrap();
    let bessel_text = h1["bessel_sensitivity_analysis"].as_str().unwrap_or("");
    assert!(bessel_text.contains("n / (n - 1)") && bessel_text.contains("live.agent_count"));
    let s6 = hypotheses.iter().find(|h| h["id"] == "S6").unwrap();
    let s6_eval = s6["evaluated_at"].as_str().unwrap();
    assert!(
        !s6_eval.contains("_or_")
            && !s6_eval.contains(" or ")
            && s6_eval.contains("final_window_at_T")
    );
    let s6_formula = p["analysis_details"]["s6_formula"].as_str().unwrap();
    assert!(!s6_formula.contains(" or ") && s6_formula.contains("final-window"));

    assert_eq!(
        p["execution_host_rule"]["single_host_required"],
        serde_json::json!(true)
    );
    assert_eq!(
        p["execution_host_rule"]["smoke_host_observed"].as_str(),
        Some("windows/x86_64")
    );
    let host_rule = p["execution_host_rule"]["rule"].as_str().unwrap();
    assert!(host_rule.contains("36 E3-X runs") && host_rule.contains("re-run e3_smoke"));

    // Machine-readable materiality thresholds, decision_definitions, and secondary_endpoints_rule (N5).
    let dz_min = p["thresholds"]["dz_min"].as_f64().unwrap();
    let rel_min = p["thresholds"]["relative_delta_min"].as_f64().unwrap();
    let min_pairs = p["thresholds"]["min_valid_pairs_for_decision"]
        .as_u64()
        .unwrap();
    assert_eq!(dz_min, 0.8);
    assert_eq!(rel_min, 0.05);
    assert_eq!(min_pairs, 10);
    assert_eq!(
        p["thresholds"]["secondary_endpoints_rule"]["dz_min"].as_f64(),
        Some(dz_min),
        "N5: secondary_endpoints_rule.dz_min must equal thresholds.dz_min"
    );
    assert_eq!(
        p["thresholds"]["secondary_endpoints_rule"]["relative_delta_min"].as_f64(),
        Some(rel_min),
        "N5: secondary_endpoints_rule.relative_delta_min must equal thresholds.relative_delta_min"
    );
    assert_eq!(
        p["thresholds"]["secondary_endpoints_rule"]["applies_to"],
        serde_json::json!(["S1", "S2", "S3", "S6"])
    );

    let defs = &p["thresholds"]["decision_definitions"];
    let min_lt = format!("< {min_pairs}");
    let min_ge = format!(">= {min_pairs}");
    let dz_lt = format!("< {dz_min}");
    let dz_ge = format!(">= {dz_min}");
    let rel_lt = format!("< {rel_min}");
    let rel_ge = format!(">= {rel_min}");
    assert!(defs["insufficient_evidence"]
        .as_str()
        .unwrap()
        .contains(&min_lt));
    assert!(defs["null"].as_str().unwrap().contains(&min_ge));
    let between_def = defs["between_detectable_below_materiality"]
        .as_str()
        .unwrap();
    assert!(
        between_def.contains(&min_ge)
            && between_def.contains(&dz_lt)
            && between_def.contains(&rel_lt),
        "N5: decision_definitions.between_detectable_below_materiality mismatch: {between_def}"
    );
    for key in ["material_positive", "material_negative_opposite"] {
        let d = defs[key].as_str().unwrap();
        assert!(
            d.contains(&min_ge) && d.contains(&dz_ge) && d.contains(&rel_ge),
            "N5: decision_definitions.{key} disagrees with thresholds: {d}"
        );
    }

    let conditions_joined = p["thresholds"]["conditions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(conditions_joined.contains(&dz_min.to_string()));
    assert!(conditions_joined.contains(&rel_min.to_string()));

    let smoke_cmd = p["smoke_command"].as_str().expect("smoke_command");
    assert!(!smoke_cmd.contains("cargo run"));
    assert!(smoke_cmd.contains("--release"));
    assert!(smoke_cmd.contains("e3_smoke"));

    // Pin JSON calibration numbers directly against committed smoke/smoke-report.json AND ensure
    // zero trait_mass substrings exist anywhere under smoke/.
    let smoke_report = load_json(
        "../artifacts/experiments/e3-productivity-gradient-body-mass/smoke/smoke-report.json",
    );
    let calib = &p["wp2_smoke_calibration"];
    let newer_rule = calib["e3x_newer_commit_rule"].as_str().unwrap();
    assert!(
        newer_rule.contains("stop and report") && newer_rule.contains("never tune"),
        "N6: e3x_newer_commit_rule must mandate stop-and-report without tuning"
    );
    assert_eq!(
        calib["smoke_build_git_commit"].as_str(),
        Some("a1229a6b59f1849091d0f2e5c0028769f892c553")
    );
    assert_eq!(
        calib["a_resource_field_present"]["passed"],
        smoke_report["wp2_a_resource_field"]["passed"]
    );
    assert_eq!(
        calib["b_standing_crop_change"]["passed"],
        smoke_report["wp2_b_standing_crop_intervention"]["passed"]
    );
    assert_eq!(
        calib["c_detritus_before_3000"]["passed"],
        smoke_report["wp2_c_detritus_before_3000"]["passed"]
    );
    assert_eq!(calib["body_mass_consulted"], serde_json::json!(false));
    assert_eq!(
        calib["all_observables_emitted_and_finite"],
        smoke_report["all_observables_emitted_and_finite"]
    );
    for (field_key, sr_key) in [
        (
            "control_uniform_standing_crop_final",
            "control_uniform_standing_crop_final",
        ),
        (
            "control_uniform_scarcity_standing_crop_final",
            "control_uniform_scarcity_standing_crop_final",
        ),
        (
            "treatment_gradient_standing_crop_final",
            "treatment_gradient_standing_crop_final",
        ),
        (
            "control_uniform_standing_crop_post_warmup_mean",
            "control_uniform_standing_crop_post_warmup_mean",
        ),
        (
            "control_uniform_scarcity_standing_crop_post_warmup_mean",
            "control_uniform_scarcity_standing_crop_post_warmup_mean",
        ),
        (
            "treatment_gradient_standing_crop_post_warmup_mean",
            "treatment_gradient_standing_crop_post_warmup_mean",
        ),
        (
            "control_uniform_scarcity_standing_crop_tail_mean",
            "control_uniform_scarcity_standing_crop_tail_mean",
        ),
        (
            "treatment_gradient_standing_crop_tail_mean",
            "treatment_gradient_standing_crop_tail_mean",
        ),
        (
            "treat_vs_ctrl_post_warmup_relative_shift",
            "treat_vs_ctrl_post_warmup_relative_shift",
        ),
        (
            "scarcity_vs_treat_post_warmup_relative_mismatch",
            "scarcity_vs_treat_post_warmup_relative_mismatch",
        ),
        (
            "treat_vs_scarcity_final_delta",
            "treat_vs_scarcity_final_delta",
        ),
        (
            "treat_vs_scarcity_tail_mean_delta",
            "treat_vs_scarcity_tail_mean_delta",
        ),
    ] {
        let a = calib["b_standing_crop_change"][field_key]
            .as_f64()
            .expect(field_key);
        let b = smoke_report["wp2_b_standing_crop_intervention"][sr_key]
            .as_f64()
            .expect(sr_key);
        assert!(
            (a - b).abs() < 1e-12,
            "calibration {field_key} ({a}) disagrees with smoke-report.json ({b})"
        );
    }

    let smoke_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts/experiments/e3-productivity-gradient-body-mass/smoke");
    for entry in std::fs::read_dir(&smoke_dir).expect("read smoke dir") {
        let path = entry.expect("smoke entry").path();
        if path.is_file() {
            let content = std::fs::read_to_string(&path).expect("read smoke artifact");
            assert!(
                !content.contains("trait_mass"),
                "smoke artifact {} must remain blind (zero 'trait_mass' occurrences)",
                path.display()
            );
        }
    }

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
        .expect("plan doc front matter must declare status")
        .trim();
    assert_ne!(
        status_line, "status: frozen",
        "front-matter status must stay a policy-valid value, never 'frozen'"
    );
    assert!(
        matches!(status_line, "status: active" | "status: completed"),
        "front-matter status must be 'status: active' or 'status: completed', got '{status_line}'"
    );

    // N5: numerically parse §4 and §7 threshold numbers in the plan doc and compare to JSON.
    let sec4 = doc_norm
        .split("\n## 4.")
        .nth(1)
        .and_then(|s| s.split("\n## 5.").next())
        .expect("plan doc section 4");
    let sec4_min_vals = [
        collect_numbers_after(sec4, "Tối thiểu để ra quyết định:"),
        collect_numbers_after(sec4, "Dưới "),
    ];
    assert!(
        sec4_min_vals[0].len() == 1 && sec4_min_vals[1].len() == 1,
        "N5: §4 must state minimum valid pairs twice: {sec4_min_vals:?}"
    );
    for v in sec4_min_vals.into_iter().flatten() {
        assert!(
            (v - (min_pairs as f64)).abs() < 1e-12,
            "N5: §4 min_pairs={v} != {min_pairs}"
        );
    }

    let sec7 = doc_norm
        .split("\n## 7.")
        .nth(1)
        .and_then(|s| s.split("\n## 8.").next())
        .expect("plan doc section 7");
    let sec7_plain = sec7.replace('`', "");
    let sec7_dz: Vec<f64> = ["dz_min = ", "|paired_dz| ≥ ", "|dz| ≥ "]
        .into_iter()
        .flat_map(|m| collect_numbers_after(&sec7_plain, m))
        .collect();
    assert!(
        sec7_dz.len() >= 5,
        "N5: expected >= 5 dz_min occurrences in §7, got {}",
        sec7_dz.len()
    );
    for v in &sec7_dz {
        assert!((v - dz_min).abs() < 1e-12, "N5: §7 dz_min={v} != {dz_min}");
    }
    let sec7_rel: Vec<f64> = [
        "relative_delta_min = ",
        "|paired_mean_delta| ≥ ",
        "|delta| ≥ ",
    ]
    .into_iter()
    .flat_map(|m| collect_numbers_after(&sec7_plain, m))
    .collect();
    assert!(
        sec7_rel.len() >= 5,
        "N5: expected >= 5 relative_delta_min occurrences in §7, got {}",
        sec7_rel.len()
    );
    for v in &sec7_rel {
        assert!(
            (v - rel_min).abs() < 1e-12,
            "N5: §7 relative_delta_min={v} != {rel_min}"
        );
    }
    let sec7_min_pairs: Vec<f64> = ["min_valid_pairs = ", "số cặp hợp lệ ≥ ", "số cặp hợp lệ < "]
        .into_iter()
        .flat_map(|m| collect_numbers_after(&sec7_plain, m))
        .collect();
    assert!(
        sec7_min_pairs.len() >= 6,
        "N5: expected >= 6 min_valid_pairs occurrences in §7, got {}",
        sec7_min_pairs.len()
    );
    for v in &sec7_min_pairs {
        assert!(
            (v - (min_pairs as f64)).abs() < 1e-12,
            "N5: §7 min_pairs={v} != {min_pairs}"
        );
    }

    for needle in [
        "live.trait_mass_var",
        "live.trait_mass_mean_west",
        "live.trait_mass_mean_east",
        "live.resource_capacity_scale",
        "control_uniform_scarcity",
        "999979",
        "730001",
        "2026-11-19",
        "9958f27",
        "a1229a6",
        "91def40",
        "live.reproduction",
        "live.thirst_steering",
        "27 observable",
        "empty_half_rule",
        "extinction_rule",
        "per_contrast",
        "−1,86 %",
        "−1,68 %",
        "var · n/(n − 1)",
    ] {
        assert!(
            doc.contains(needle),
            "plan doc must contain '{needle}' before any experimental run"
        );
    }
}

// ---- Reference evaluator matching e3-preregistration.json rules (D1 & materiality tests) ----

#[derive(Clone, Debug, PartialEq, Eq)]
enum E3Decision {
    MaterialPositive,
    MaterialNegativeOpposite,
    BetweenDetectableBelowMateriality,
    Null,
    InsufficientEvidence,
}

#[derive(Clone, Debug)]
struct SyntheticArmRun {
    seed: u64,
    agent_counts_by_sample: Vec<(u64, f64)>,
    final_agent_count: f64,
    trait_mass_var: f64,
    trait_mass_p10: f64,
    trait_mass_p90: f64,
    trait_mass_mean: f64,
    trait_mass_mean_west: f64,
    trait_mass_mean_east: f64,
}

struct E3EvalReport {
    n_extinct_pairs_dropped: usize,
    n_singleton_pairs_dropped: usize,
    n_empty_half_pairs_dropped_s3: usize,
    empty_half_records: Vec<(u64, &'static str, &'static str)>,
    h1_effect: PairedEffect,
    s1_effect: PairedEffect,
    s2_effect: PairedEffect,
    s3_effect: PairedEffect,
}

fn first_extinct_tick(r: &SyntheticArmRun) -> Option<u64> {
    if let Some(&(t, _)) = r.agent_counts_by_sample.iter().find(|&&(_, n)| n == 0.0) {
        return Some(t);
    }
    if r.final_agent_count == 0.0 {
        return Some(18_000);
    }
    None
}

fn empty_half_label(r: &SyntheticArmRun) -> Option<&'static str> {
    match (r.trait_mass_mean_west == 0.0, r.trait_mass_mean_east == 0.0) {
        (true, true) => Some("both"),
        (true, false) => Some("west"),
        (false, true) => Some("east"),
        (false, false) => None,
    }
}

fn evaluate_e3_pairs(pairs: &[(SyntheticArmRun, SyntheticArmRun)]) -> E3EvalReport {
    let n_req = pairs.len();
    let mut n_extinct_pairs_dropped = 0usize;
    let mut n_singleton_pairs_dropped = 0usize;
    let mut n_empty_half_pairs_dropped_s3 = 0usize;
    let mut empty_half_records = Vec::new();

    let (mut h1_c, mut h1_t, mut h1_d) = (Vec::new(), Vec::new(), Vec::new());
    let (mut s1_c, mut s1_t, mut s1_d) = (Vec::new(), Vec::new(), Vec::new());
    let (mut s2_c, mut s2_t, mut s2_d) = (Vec::new(), Vec::new(), Vec::new());
    let (mut s3_c, mut s3_t, mut s3_d) = (Vec::new(), Vec::new(), Vec::new());

    for (ctrl, treat) in pairs {
        assert_eq!(ctrl.seed, treat.seed);
        if first_extinct_tick(ctrl).is_some() || first_extinct_tick(treat).is_some() {
            n_extinct_pairs_dropped += 1;
            continue;
        }
        s2_c.push(ctrl.trait_mass_mean);
        s2_t.push(treat.trait_mass_mean);
        s2_d.push(treat.trait_mass_mean - ctrl.trait_mass_mean);

        if ctrl.final_agent_count < 2.0 || treat.final_agent_count < 2.0 {
            n_singleton_pairs_dropped += 1;
        } else {
            h1_c.push(ctrl.trait_mass_var);
            h1_t.push(treat.trait_mass_var);
            h1_d.push(treat.trait_mass_var - ctrl.trait_mass_var);

            let c_iqr = ctrl.trait_mass_p90 - ctrl.trait_mass_p10;
            let t_iqr = treat.trait_mass_p90 - treat.trait_mass_p10;
            s1_c.push(c_iqr);
            s1_t.push(t_iqr);
            s1_d.push(t_iqr - c_iqr);
        }

        let ctrl_empty = empty_half_label(ctrl);
        let treat_empty = empty_half_label(treat);
        if ctrl_empty.is_some() || treat_empty.is_some() {
            n_empty_half_pairs_dropped_s3 += 1;
            if let Some(side) = ctrl_empty {
                empty_half_records.push((ctrl.seed, "control", side));
            }
            if let Some(side) = treat_empty {
                empty_half_records.push((treat.seed, "treatment", side));
            }
        } else {
            let cs3 = ctrl.trait_mass_mean_east - ctrl.trait_mass_mean_west;
            let ts3 = treat.trait_mass_mean_east - treat.trait_mass_mean_west;
            s3_c.push(cs3);
            s3_t.push(ts3);
            s3_d.push(ts3 - cs3);
        }
    }

    E3EvalReport {
        n_extinct_pairs_dropped,
        n_singleton_pairs_dropped,
        n_empty_half_pairs_dropped_s3,
        empty_half_records,
        h1_effect: paired_effect("live.trait_mass_var", n_req, &h1_c, &h1_t, &h1_d).unwrap(),
        s1_effect: paired_effect("S1", n_req, &s1_c, &s1_t, &s1_d).unwrap(),
        s2_effect: paired_effect("live.trait_mass_mean", n_req, &s2_c, &s2_t, &s2_d).unwrap(),
        s3_effect: paired_effect("S3", n_req, &s3_c, &s3_t, &s3_d).unwrap(),
    }
}

fn classify_e3_decision(
    effect: &PairedEffect,
    min_valid_pairs: usize,
    dz_min: f64,
    relative_delta_min: f64,
) -> E3Decision {
    if effect.n_complete_pairs < min_valid_pairs {
        return E3Decision::InsufficientEvidence;
    }
    let (Some(lo), Some(hi), Some(delta), Some(ctrl_mean)) = (
        effect.ci95_low,
        effect.ci95_high,
        effect.paired_mean_delta,
        effect.control_mean,
    ) else {
        return E3Decision::Null;
    };
    let ci_excludes_zero = lo > 0.0 || hi < 0.0;
    if !ci_excludes_zero {
        return E3Decision::Null;
    }
    let dz_ok = effect.paired_dz.is_some_and(|d| d.abs() >= dz_min);
    let rel_ok = delta.abs() >= relative_delta_min * ctrl_mean.abs();
    if !dz_ok || !rel_ok {
        return E3Decision::BetweenDetectableBelowMateriality;
    }
    if delta > 0.0 {
        E3Decision::MaterialPositive
    } else {
        E3Decision::MaterialNegativeOpposite
    }
}

fn make_clean_run(seed: u64, var: f64, mean: f64, west: f64, east: f64) -> SyntheticArmRun {
    SyntheticArmRun {
        seed,
        agent_counts_by_sample: vec![(600, 120.0), (12_000, 40.0), (18_000, 25.0)],
        final_agent_count: 25.0,
        trait_mass_var: var,
        trait_mass_p10: mean - 1.0,
        trait_mass_p90: mean + 1.0,
        trait_mass_mean: mean,
        trait_mass_mean_west: west,
        trait_mass_mean_east: east,
    }
}

#[test]
fn empty_half_and_extinction_yield_excluded_pairs_never_zero_sentinels() {
    // Ground-truth check on real mass_statistics() sentinel outputs (D1 motivation).
    let empty_stats = mass_statistics(&[]);
    assert_eq!(empty_stats, [0.0; 4]);
    let single_stats = mass_statistics(&[5.0]);
    assert_eq!(single_stats[0], 5.0);
    assert_eq!(single_stats[1], 0.0);

    let p = prereg();
    let min_pairs = p["thresholds"]["min_valid_pairs_for_decision"]
        .as_u64()
        .unwrap() as usize;
    let dz_min = p["thresholds"]["dz_min"].as_f64().unwrap();
    let rel_min = p["thresholds"]["relative_delta_min"].as_f64().unwrap();

    // Case A: 12 pairs where 1 treatment seed has an empty West half at T (0.0 via mass_statistics(&[])),
    // but 15 surviving agents in East, and all clean seeds have exact S3 delta = +1.5.
    // Without D1 empty_half_rule, treat S3 for that seed would be 20.0 - 0.0 = +20.0, skewing S3.
    let mut pairs_empty_west = Vec::new();
    for i in 0..12u64 {
        let seed = SEED_BASE + SEED_STEP * i;
        let ctrl = make_clean_run(seed, 4.0, 10.0, 10.0, 10.0);
        let mut treat = make_clean_run(seed, 6.0, 10.5, 9.5, 11.0); // treat S3 = +1.5
        if i == 0 {
            treat.final_agent_count = 15.0;
            treat.trait_mass_mean_west = empty_stats[0]; // 0.0 sentinel from empty half
            treat.trait_mass_mean_east = 20.0;
        }
        pairs_empty_west.push((ctrl, treat));
    }
    let rep_w = evaluate_e3_pairs(&pairs_empty_west);
    assert_eq!(rep_w.n_extinct_pairs_dropped, 0);
    assert_eq!(rep_w.n_empty_half_pairs_dropped_s3, 1);
    assert_eq!(
        rep_w.empty_half_records,
        vec![(730_001, "treatment", "west")]
    );
    assert_eq!(rep_w.s3_effect.n_complete_pairs, 11);
    assert!(
        (rep_w.s3_effect.paired_mean_delta.unwrap() - 1.5).abs() < 1e-12,
        "empty West 0.0 sentinel must be excluded so clean +1.5 delta enters S3, not +20.0"
    );
    assert_eq!(
        rep_w.h1_effect.n_complete_pairs, 12,
        "global H1 retains the non-extinct, n>=2 pair even when one grid half is empty"
    );

    // Case B: empty East half in control arm, plus 1 extinct arm at tick 12000, plus 1 singleton at T.
    let mut pairs_mixed = Vec::new();
    for i in 0..12u64 {
        let seed = SEED_BASE + SEED_STEP * i;
        let mut ctrl = make_clean_run(seed, 4.0, 10.0, 10.0, 10.0);
        let mut treat = make_clean_run(seed, 6.0 + (i as f64) * 0.1, 10.5, 9.5, 11.0);
        if i == 1 {
            // Extinct in control mid-run (0 agents -> mass_statistics(&[]) zeros): dropped from ALL trait_mass metrics.
            ctrl.agent_counts_by_sample = vec![(600, 120.0), (12_000, 0.0), (18_000, 0.0)];
            ctrl.final_agent_count = 0.0;
            ctrl.trait_mass_var = empty_stats[1]; // 0.0 sentinel
            ctrl.trait_mass_mean = empty_stats[0];
            ctrl.trait_mass_mean_west = empty_stats[0];
            ctrl.trait_mass_mean_east = empty_stats[0];
        } else if i == 2 {
            // Empty East in control: dropped from S3 only.
            ctrl.trait_mass_mean_east = empty_stats[0];
        } else if i == 3 {
            // Singleton in treatment at T (var == 0.0): dropped from H1 & S1, retained for S2 & S3.
            treat.final_agent_count = 1.0;
            treat.trait_mass_var = single_stats[1];
            treat.trait_mass_p10 = single_stats[2];
            treat.trait_mass_p90 = single_stats[3];
        }
        pairs_mixed.push((ctrl, treat));
    }
    let rep_m = evaluate_e3_pairs(&pairs_mixed);
    assert_eq!(rep_m.n_extinct_pairs_dropped, 1);
    assert_eq!(rep_m.n_singleton_pairs_dropped, 1);
    assert_eq!(rep_m.n_empty_half_pairs_dropped_s3, 1);
    assert_eq!(rep_m.s2_effect.n_complete_pairs, 11);
    assert_eq!(rep_m.h1_effect.n_complete_pairs, 10);
    assert_eq!(rep_m.s1_effect.n_complete_pairs, 10);
    assert_eq!(rep_m.s3_effect.n_complete_pairs, 10);
    assert!(
        (rep_m.h1_effect.control_mean.unwrap() - 4.0).abs() < 1e-12,
        "extinct 0.0 var must never pull control_mean down from 4.0"
    );
    assert_eq!(
        classify_e3_decision(&rep_m.h1_effect, min_pairs, dz_min, rel_min),
        E3Decision::MaterialPositive
    );

    // Case C: 3 excluded pairs out of 12 (2 extinct + 1 singleton -> 9 < 10) MUST be InsufficientEvidence.
    let mut pairs_too_few = pairs_mixed;
    pairs_too_few[4].1.final_agent_count = 0.0;
    let rep_low = evaluate_e3_pairs(&pairs_too_few);
    assert_eq!(rep_low.h1_effect.n_complete_pairs, 9);
    assert_eq!(
        classify_e3_decision(&rep_low.h1_effect, min_pairs, dz_min, rel_min),
        E3Decision::InsufficientEvidence
    );
}

#[test]
fn materiality_threshold_decision_rules_hand_worked_boundaries() {
    let p = prereg();
    let min_pairs = p["thresholds"]["min_valid_pairs_for_decision"]
        .as_u64()
        .unwrap() as usize;
    let dz_min = p["thresholds"]["dz_min"].as_f64().unwrap();
    let rel_min = p["thresholds"]["relative_delta_min"].as_f64().unwrap();
    assert_eq!(min_pairs, 10);
    assert_eq!(dz_min, 0.8);
    assert_eq!(rel_min, 0.05);

    let enum_vals: BTreeSet<&str> = p["thresholds"]["decision_enum"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(
        enum_vals,
        [
            "material_positive",
            "material_negative_opposite",
            "between_detectable_below_materiality",
            "null",
            "insufficient_evidence",
        ]
        .into_iter()
        .collect()
    );

    // Baseline hand-worked passing effect: control_mean = 10.0, 5% threshold = 0.50.
    let hand_effect =
        |n: usize, delta: f64, lo: Option<f64>, hi: Option<f64>, dz: Option<f64>| PairedEffect {
            observable: "live.trait_mass_var".to_string(),
            n_requested: 12,
            n_complete_pairs: n,
            control_mean: Some(10.0),
            treatment_mean: Some(10.0 + delta),
            paired_mean_delta: Some(delta),
            paired_sd: Some(delta.abs() / dz.unwrap_or(1.0).abs().max(1e-9)),
            paired_se: Some(0.1),
            ci95_low: lo,
            ci95_high: hi,
            ci95_method: None,
            paired_dz: dz,
        };

    // 1. Minimum valid pairs boundary: 9 vs 10.
    assert_eq!(
        classify_e3_decision(
            &hand_effect(9, 2.0, Some(1.5), Some(2.5), Some(2.0)),
            min_pairs,
            dz_min,
            rel_min
        ),
        E3Decision::InsufficientEvidence
    );
    assert_eq!(
        classify_e3_decision(
            &hand_effect(10, 2.0, Some(1.5), Some(2.5), Some(2.0)),
            min_pairs,
            dz_min,
            rel_min
        ),
        E3Decision::MaterialPositive
    );

    // 2. CI95 around 0 boundary: includes vs strictly excludes 0 (and None).
    assert_eq!(
        classify_e3_decision(
            &hand_effect(12, 1.0, Some(-0.01), Some(2.01), Some(0.9)),
            min_pairs,
            dz_min,
            rel_min
        ),
        E3Decision::Null
    );
    assert_eq!(
        classify_e3_decision(
            &hand_effect(12, 1.0, Some(0.0), Some(2.0), Some(0.9)),
            min_pairs,
            dz_min,
            rel_min
        ),
        E3Decision::Null
    );
    assert_eq!(
        classify_e3_decision(
            &hand_effect(12, 1.0, None, None, Some(0.9)),
            min_pairs,
            dz_min,
            rel_min
        ),
        E3Decision::Null
    );
    assert_eq!(
        classify_e3_decision(
            &hand_effect(12, 1.0, Some(0.01), Some(1.99), Some(0.9)),
            min_pairs,
            dz_min,
            rel_min
        ),
        E3Decision::MaterialPositive
    );
    assert_eq!(
        classify_e3_decision(
            &hand_effect(12, -1.0, Some(-1.99), Some(-0.01), Some(-0.9)),
            min_pairs,
            dz_min,
            rel_min
        ),
        E3Decision::MaterialNegativeOpposite
    );

    // 3. Cohen's dz boundary around |0.8|: 0.7999 vs 0.80, -0.7999 vs -0.80, None.
    assert_eq!(
        classify_e3_decision(
            &hand_effect(12, 1.0, Some(0.2), Some(1.8), Some(0.7999)),
            min_pairs,
            dz_min,
            rel_min
        ),
        E3Decision::BetweenDetectableBelowMateriality
    );
    assert_eq!(
        classify_e3_decision(
            &hand_effect(12, 1.0, Some(0.2), Some(1.8), Some(0.8)),
            min_pairs,
            dz_min,
            rel_min
        ),
        E3Decision::MaterialPositive
    );
    assert_eq!(
        classify_e3_decision(
            &hand_effect(12, -1.0, Some(-1.8), Some(-0.2), Some(-0.7999)),
            min_pairs,
            dz_min,
            rel_min
        ),
        E3Decision::BetweenDetectableBelowMateriality
    );
    assert_eq!(
        classify_e3_decision(
            &hand_effect(12, -1.0, Some(-1.8), Some(-0.2), Some(-0.8)),
            min_pairs,
            dz_min,
            rel_min
        ),
        E3Decision::MaterialNegativeOpposite
    );
    assert_eq!(
        classify_e3_decision(
            &hand_effect(12, 1.0, Some(0.5), Some(1.5), None),
            min_pairs,
            dz_min,
            rel_min
        ),
        E3Decision::BetweenDetectableBelowMateriality
    );

    // 4. Relative shift boundary around 0.05 * |control_mean| (0.05 * 10.0 = 0.50).
    assert_eq!(
        classify_e3_decision(
            &hand_effect(12, 0.4999, Some(0.2), Some(0.8), Some(1.2)),
            min_pairs,
            dz_min,
            rel_min
        ),
        E3Decision::BetweenDetectableBelowMateriality
    );
    assert_eq!(
        classify_e3_decision(
            &hand_effect(12, 0.50, Some(0.2), Some(0.8), Some(1.2)),
            min_pairs,
            dz_min,
            rel_min
        ),
        E3Decision::MaterialPositive
    );
    assert_eq!(
        classify_e3_decision(
            &hand_effect(12, -0.4999, Some(-0.8), Some(-0.2), Some(-1.2)),
            min_pairs,
            dz_min,
            rel_min
        ),
        E3Decision::BetweenDetectableBelowMateriality
    );
    assert_eq!(
        classify_e3_decision(
            &hand_effect(12, -0.50, Some(-0.8), Some(-0.2), Some(-1.2)),
            min_pairs,
            dz_min,
            rel_min
        ),
        E3Decision::MaterialNegativeOpposite
    );

    // 5. Real paired_effect() round-trip check across three 10-pair vectors.
    let ctrl = vec![10.0; 10];
    // Below 5% relative shift (+0.40 mean delta) with non-zero variance across seeds -> between.
    let d_small = vec![0.35, 0.45, 0.35, 0.45, 0.35, 0.45, 0.35, 0.45, 0.35, 0.45];
    let t_small: Vec<f64> = ctrl.iter().zip(&d_small).map(|(c, d)| c + d).collect();
    let e_small = paired_effect("live.trait_mass_var", 12, &ctrl, &t_small, &d_small).unwrap();
    assert_eq!(
        classify_e3_decision(&e_small, min_pairs, dz_min, rel_min),
        E3Decision::BetweenDetectableBelowMateriality
    );

    // Above all thresholds (+0.60 mean delta >= 0.50, tight variance so dz > 0.8) -> material_positive.
    let d_mat = vec![0.55, 0.65, 0.55, 0.65, 0.55, 0.65, 0.55, 0.65, 0.55, 0.65];
    let t_mat: Vec<f64> = ctrl.iter().zip(&d_mat).map(|(c, d)| c + d).collect();
    let e_mat = paired_effect("live.trait_mass_var", 12, &ctrl, &t_mat, &d_mat).unwrap();
    assert_eq!(
        classify_e3_decision(&e_mat, min_pairs, dz_min, rel_min),
        E3Decision::MaterialPositive
    );
}

struct E3TripletReport {
    t_minus_s: E3EvalReport,
    t_minus_u: E3EvalReport,
    s_minus_u: E3EvalReport,
    additive_h1_all_three_valid: Vec<(u64, f64, f64, f64)>,
}

fn evaluate_e3_triplets(
    triplets: &[(SyntheticArmRun, SyntheticArmRun, SyntheticArmRun)],
) -> E3TripletReport {
    let pairs_ts: Vec<(SyntheticArmRun, SyntheticArmRun)> = triplets
        .iter()
        .map(|(_, s, t)| (s.clone(), t.clone()))
        .collect();
    let pairs_tu: Vec<(SyntheticArmRun, SyntheticArmRun)> = triplets
        .iter()
        .map(|(u, _, t)| (u.clone(), t.clone()))
        .collect();
    let pairs_su: Vec<(SyntheticArmRun, SyntheticArmRun)> = triplets
        .iter()
        .map(|(u, s, _)| (u.clone(), s.clone()))
        .collect();

    let additive_h1_all_three_valid = triplets
        .iter()
        .filter(|(u, s, t)| {
            [u, s, t]
                .into_iter()
                .all(|arm| first_extinct_tick(arm).is_none() && arm.final_agent_count >= 2.0)
        })
        .map(|(u, s, t)| {
            (
                u.seed,
                t.trait_mass_var - u.trait_mass_var,
                t.trait_mass_var - s.trait_mass_var,
                s.trait_mass_var - u.trait_mass_var,
            )
        })
        .collect();

    E3TripletReport {
        t_minus_s: evaluate_e3_pairs(&pairs_ts),
        t_minus_u: evaluate_e3_pairs(&pairs_tu),
        s_minus_u: evaluate_e3_pairs(&pairs_su),
        additive_h1_all_three_valid,
    }
}

#[test]
fn three_arm_per_contrast_exclusion_keeps_h1_when_control_uniform_extinct() {
    let p = prereg();
    let min_pairs = p["thresholds"]["min_valid_pairs_for_decision"]
        .as_u64()
        .unwrap() as usize;
    let dz_min = p["thresholds"]["dz_min"].as_f64().unwrap();
    let rel_min = p["thresholds"]["relative_delta_min"].as_f64().unwrap();

    // R1 scenario: control_uniform (U) goes extinct in 3 of 12 seeds while S and T stay healthy.
    // Per contrast: H1 (T - S) retains all 12 pairs -> MaterialPositive; T - U and S - U drop
    // to 9 pairs -> InsufficientEvidence; additive decomposition uses the 9 seeds valid in all 3.
    let mut triplets = Vec::new();
    for i in 0..12u64 {
        let seed = SEED_BASE + SEED_STEP * i;
        let mut u = make_clean_run(seed, 3.0, 9.5, 9.5, 9.5);
        let s = make_clean_run(seed, 4.0 + (i as f64) * 0.03, 10.0, 10.0, 10.0);
        let t = make_clean_run(seed, 5.5 + (i as f64) * 0.08, 10.5, 9.5, 11.0);
        if i < 3 {
            u.agent_counts_by_sample = vec![(600, 120.0), (15_000, 0.0), (18_000, 0.0)];
            u.final_agent_count = 0.0;
            u.trait_mass_var = 0.0;
            u.trait_mass_mean = 0.0;
            u.trait_mass_mean_west = 0.0;
            u.trait_mass_mean_east = 0.0;
        }
        triplets.push((u, s, t));
    }

    let rep = evaluate_e3_triplets(&triplets);
    assert_eq!(rep.t_minus_s.n_extinct_pairs_dropped, 0);
    assert_eq!(rep.t_minus_s.h1_effect.n_complete_pairs, 12);
    assert_eq!(
        classify_e3_decision(&rep.t_minus_s.h1_effect, min_pairs, dz_min, rel_min),
        E3Decision::MaterialPositive
    );

    assert_eq!(rep.t_minus_u.n_extinct_pairs_dropped, 3);
    assert_eq!(rep.t_minus_u.h1_effect.n_complete_pairs, 9);
    assert_eq!(
        classify_e3_decision(&rep.t_minus_u.h1_effect, min_pairs, dz_min, rel_min),
        E3Decision::InsufficientEvidence
    );

    assert_eq!(rep.s_minus_u.n_extinct_pairs_dropped, 3);
    assert_eq!(rep.s_minus_u.h1_effect.n_complete_pairs, 9);
    assert_eq!(
        classify_e3_decision(&rep.s_minus_u.h1_effect, min_pairs, dz_min, rel_min),
        E3Decision::InsufficientEvidence
    );

    assert_eq!(rep.additive_h1_all_three_valid.len(), 9);
    for (seed, d_tu, d_ts, d_su) in rep.additive_h1_all_three_valid {
        assert!(
            (d_tu - (d_ts + d_su)).abs() < 1e-12,
            "seed {seed}: (T - U)={d_tu} must equal (T - S)={d_ts} + (S - U)={d_su}"
        );
    }
}
