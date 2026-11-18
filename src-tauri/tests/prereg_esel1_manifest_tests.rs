//! The E-SEL1 preregistration, checked by machine rather than trusted as prose.
//!
//! Modelled on `prereg_e2_manifest_tests.rs`. These tests pin the committed manifests of E-SEL1 —
//! "does food scarcity shift the live population's mean body mass?" — so that the declared factors,
//! the seed set, the sampling plan, the metric list and the places where the smoke may still set a
//! value are fixed in git history *before* any smoke or experimental run exists.
//!
//! It never constructs a `LiveExperimentAdapter`, never steps a tick and never touches the runner.
//! Parsing a manifest is a schema check; running one is the experiment.
//!
//! # Placeholders
//!
//! The smoke may set exactly `N0` (`live.founders`), `F_hi` / `F_lo` (`live.food_cap`) and the
//! duration rung (plan section 6), each by a mechanical algorithm that reads only
//! `live.agent_count` and `live.standing_crop`. The H1 direction is two-sided, fixed before any run;
//! the smoke never sets it. A manifest holds numbers only, so the experimental
//! manifests carry NEGATIVE SENTINELS for the first three; nothing else may be a sentinel, and the
//! live adapter refuses a sentinel at model construction, so an unamended experimental manifest
//! cannot be run by accident. When the smoke values are locked, the sentinels are replaced in a
//! dated amendment and `PLACEHOLDERS_OPEN` below is flipped to `false` in that same commit, which
//! turns the "no sentinel" assertions on.

use anima_engine_lib::core::experiment::{
    ExperimentManifest, FactorDiff, ObservableRegistry, MANIFEST_SCHEMA_VERSION,
    MAX_ENSEMBLE_RESULT_BYTES,
};
use anima_engine_lib::core::live_experiment::{
    all_live_observable_ids, LiveWorldConfig, LIVE_KEYS, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS,
    LIVE_KEY_MUTATION_RATE,
};
use std::collections::BTreeMap;

const PREREG_DIR: &str = "tests/fixtures/experiments_esel1";
const SMOKE_SEED: u64 = 999971;

/// `true` while the smoke has not yet set N0, F_hi, F_lo. The amendment that sets them flips this.
/// Amendment 2 closed the registration without an experiment (the smoke stopped twice at step C),
/// so this stays `true` for good: the experimental manifests keep their sentinels and cannot run.
const PLACEHOLDERS_OPEN: bool = true;

const SENTINEL_FOUNDERS: f64 = -3.0;
const SENTINEL_F_HI: f64 = -1.0;
const SENTINEL_F_LO: f64 = -2.0;

const CONTROL: &str = "esel1-control-abundant.json";
const TREATMENT: &str = "esel1-treatment-scarce.json";
const DRIFT: &str = "esel1-drift-scarce-mutation-off.json";
const SMOKE_CONTROL: &str = "esel1-smoke-abundant.json";
const SMOKE_TREATMENT: &str = "esel1-smoke-scarce.json";
const SMOKE_DRIFT: &str = "esel1-smoke-drift-scarce-mutation-off.json";

fn path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(PREREG_DIR)
        .join(name)
}

fn load(name: &str) -> ExperimentManifest {
    let raw = std::fs::read_to_string(path(name)).unwrap_or_else(|e| panic!("read {name}: {e}"));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("{name} parses as ExperimentManifest: {e}"))
}

fn prereg() -> serde_json::Value {
    let raw = std::fs::read_to_string(path("esel1-preregistration.json")).expect("read prereg");
    serde_json::from_str(&raw).expect("esel1-preregistration.json parses as JSON")
}

fn ic(m: &ExperimentManifest) -> BTreeMap<String, f64> {
    m.initial_conditions.values.iter().cloned().collect()
}

/// Keys whose value differs (or whose presence differs) between two manifests.
fn diff_keys(a: &ExperimentManifest, b: &ExperimentManifest) -> Vec<String> {
    let (ma, mb) = (ic(a), ic(b));
    let mut keys: Vec<String> = ma.keys().chain(mb.keys()).cloned().collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter(|k| ma.get(k) != mb.get(k))
        .collect()
}

const ALL: [&str; 6] = [
    CONTROL,
    TREATMENT,
    DRIFT,
    SMOKE_CONTROL,
    SMOKE_TREATMENT,
    SMOKE_DRIFT,
];

// ---- Real manifests --------------------------------------------------------------------------

#[test]
fn every_preregistered_manifest_parses_validates_and_uses_only_live_keys() {
    let reg = ObservableRegistry::live_default();
    reg.validate().expect("the live registry is valid");
    for name in ALL {
        let m = load(name);
        assert_eq!(m.schema_version, MANIFEST_SCHEMA_VERSION, "{name}");
        m.validate(&reg)
            .unwrap_or_else(|e| panic!("{name} must validate: {e}"));
        for (k, _) in &m.initial_conditions.values {
            assert!(
                LIVE_KEYS.contains(&k.as_str()),
                "{name} declares '{k}', which the live adapter would refuse"
            );
        }
    }
}

// ---- The declared factors --------------------------------------------------------------------

#[test]
fn control_and_treatment_differ_in_exactly_live_food_cap() {
    let (c, t) = (load(CONTROL), load(TREATMENT));
    let allowed = FactorDiff {
        allowed_paths: vec!["initial_conditions".to_string()],
    };
    assert_eq!(
        allowed
            .validate(&c, &t)
            .expect("differ only at declared factors"),
        vec!["initial_conditions".to_string()]
    );
    assert_eq!(
        diff_keys(&c, &t),
        vec![LIVE_KEY_FOOD_CAP.to_string()],
        "the declared factor is exactly one initial-condition key"
    );
    assert_ne!(c.fingerprint(), t.fingerprint());
}

#[test]
fn drift_differs_from_treatment_in_exactly_live_mutation_rate_set_to_zero() {
    let (t, d) = (load(TREATMENT), load(DRIFT));
    assert_eq!(diff_keys(&t, &d), vec![LIVE_KEY_MUTATION_RATE.to_string()]);
    assert_eq!(ic(&d).get(LIVE_KEY_MUTATION_RATE), Some(&0.0));
    assert!(
        !ic(&t).contains_key(LIVE_KEY_MUTATION_RATE),
        "the treatment leaves the mutation rate absent (== 0.05), as the plan states"
    );
}

#[test]
fn the_common_configuration_is_the_one_the_lead_session_fixed() {
    let expected: [(&str, f64); 7] = [
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
        ("live.locomotion", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
        ("live.evolved_brains", 0.0),
        ("live.predator_fraction", 0.0),
    ];
    for name in ALL {
        let m = ic(&load(name));
        for (k, v) in expected {
            assert_eq!(m.get(k), Some(&v), "{name}: {k}");
        }
        for absent in ["live.starve_ticks", "live.dehydration_ticks"] {
            assert!(
                !m.contains_key(absent),
                "{name} must leave {absent} at its default"
            );
        }
    }
    let p = prereg();
    let common = &p["common_initial_conditions"];
    for (k, v) in expected {
        assert_eq!(
            common[k].as_f64(),
            Some(v),
            "preregistration disagrees on {k}"
        );
    }
}

#[test]
fn all_arms_share_everything_the_pairing_depends_on() {
    let c = load(CONTROL);
    for name in [TREATMENT, DRIFT] {
        let o = load(name);
        assert_eq!(c.seeds, o.seeds, "{name}");
        assert_eq!(c.duration_ticks, o.duration_ticks, "{name}");
        assert_eq!(c.sample_period, o.sample_period, "{name}");
        assert_eq!(c.world_identity, o.world_identity, "{name}");
        assert_eq!(c.laws, o.laws, "{name}");
        assert_eq!(c.observable_ids, o.observable_ids, "{name}");
        assert!(o.interventions.is_empty() && o.exotic_interventions.is_empty());
    }
}

// ---- The smoke manifests ---------------------------------------------------------------------

#[test]
fn the_smoke_manifests_differ_from_the_experiment_only_where_the_smoke_may_set() {
    // The smoke may set N0 and the food cap (and, by construction, the seed list); the smoke drift
    // manifest additionally carries live.mutation_rate, which the experimental drift manifest holds
    // at the same 0. Nothing else may differ: not the duration, sampling, laws, world or observables.
    let may_set = [LIVE_KEY_FOUNDERS, LIVE_KEY_FOOD_CAP, LIVE_KEY_MUTATION_RATE];
    for (smoke, exp) in [
        (SMOKE_CONTROL, CONTROL),
        (SMOKE_TREATMENT, TREATMENT),
        (SMOKE_DRIFT, DRIFT),
    ] {
        let (s, e) = (load(smoke), load(exp));
        for k in diff_keys(&s, &e) {
            assert!(
                may_set.contains(&k.as_str()),
                "{smoke} differs from {exp} in '{k}', which the smoke may not set"
            );
            // live.mutation_rate is allowed to differ only because the smoke drift manifest holds
            // it; it holds the same 0 as the experimental drift manifest, so it never differs.
            assert_ne!(
                k, LIVE_KEY_MUTATION_RATE,
                "{smoke} and {exp} must hold the same mutation rate"
            );
        }
        assert_ne!(
            s.seeds, e.seeds,
            "{smoke} must use the smoke seed, {exp} the registered ones"
        );
        assert_eq!(s.duration_ticks, e.duration_ticks, "{smoke} duration");
        assert_eq!(s.sample_period, e.sample_period, "{smoke} sample period");
        assert_eq!(s.observable_ids, e.observable_ids, "{smoke} observables");
        assert_eq!(s.laws, e.laws, "{smoke} laws");
        assert_eq!(s.world_identity, e.world_identity, "{smoke} world");
        assert_eq!(s.interventions, e.interventions, "{smoke} interventions");
        assert_eq!(s.exotic_interventions, e.exotic_interventions, "{smoke}");
        assert_eq!(s.observer, e.observer, "{smoke} observer");
        // Same key set on both sides.
        let keys = |m: &ExperimentManifest| ic(m).into_keys().collect::<Vec<_>>();
        assert_eq!(
            keys(&s),
            keys(&e),
            "{smoke} and {exp} must declare the same keys"
        );
    }
    // The smoke drift manifest holds live.mutation_rate = 0 exactly like the experimental one.
    assert_eq!(
        ic(&load(SMOKE_DRIFT)).get(LIVE_KEY_MUTATION_RATE),
        Some(&0.0)
    );
    // The two smoke arms differ in the food cap only, like the experiment.
    assert_eq!(
        diff_keys(&load(SMOKE_CONTROL), &load(SMOKE_TREATMENT)),
        vec![LIVE_KEY_FOOD_CAP.to_string()]
    );
    // The smoke drift arm is the smoke scarce arm with the mutation rate off, nothing else.
    assert_eq!(
        diff_keys(&load(SMOKE_TREATMENT), &load(SMOKE_DRIFT)),
        vec![LIVE_KEY_MUTATION_RATE.to_string()]
    );
    // Smoke candidates respect the written constraints they are chosen under.
    let (sc, st) = (ic(&load(SMOKE_CONTROL)), ic(&load(SMOKE_TREATMENT)));
    assert!(sc[LIVE_KEY_FOUNDERS] >= 30.0, "N0 >= 30");
    assert_eq!(
        sc[LIVE_KEY_FOUNDERS], st[LIVE_KEY_FOUNDERS],
        "N0 is shared by the smoke arms"
    );
    assert!(
        st[LIVE_KEY_FOOD_CAP] < sc[LIVE_KEY_FOOD_CAP],
        "scarce < abundant"
    );
    // The starting candidates are the algorithm's first ones: N0 = 30, F = 50, scarce = round(F/2).
    assert_eq!(sc[LIVE_KEY_FOUNDERS], 30.0);
    assert_eq!(sc[LIVE_KEY_FOOD_CAP], 50.0);
    assert_eq!(st[LIVE_KEY_FOOD_CAP], 25.0);
}

#[test]
fn the_smoke_algorithm_is_mechanical_and_never_reads_body_mass() {
    let p = prereg();
    let s = &p["smoke_may_set_only"];
    let consult: Vec<&str> = s["may_consult_only"]
        .as_array()
        .expect("may_consult_only")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(consult, vec!["live.agent_count", "live.standing_crop"]);
    assert!(s["must_not_consult"]
        .as_str()
        .unwrap()
        .contains("live.trait_mass_"));
    let alg = s["algorithm"].as_object().expect("algorithm");
    for step in ["step_A_F_hi", "step_B_N0", "step_C_F_lo", "duration_rung"] {
        let text = alg[step].as_str().unwrap_or_else(|| panic!("{step}"));
        assert!(!text.is_empty(), "{step}");
        assert!(
            !text.contains("trait_mass"),
            "{step} must not consult body mass"
        );
    }
    let a = alg["step_A_F_hi"].as_str().unwrap();
    assert!(a.contains("N0 = 30") && a.contains("food_cap F = 50") && a.contains("tick <= 1200"));
    assert!(a.contains("> 80") && a.contains("F - 10") && a.contains("floor 20"));
    let b = alg["step_B_N0"].as_str().unwrap();
    assert!(
        b.contains("30, 40, 50, 60, 70, 80") && b.contains(">= 5") && b.contains("ALL samples")
    );
    let c = alg["step_C_F_lo"].as_str().unwrap();
    assert!(
        c.contains("1/2, 1/3, 1/4, 1/8") && c.contains("0.4 <= r <= 0.6") && c.contains("FIRST")
    );
    let d = alg["duration_rung"].as_str().unwrap();
    assert!(d.contains("36 runs") && d.contains("<= 90") && d.contains("HIGHEST"));
    assert!(d.contains("no parallelism credit") && d.contains("N is never lowered"));
    assert!(s["T_smoke"].as_str().unwrap().starts_with("18000"));
    // The smoke has no say over the direction: not a key, not a rule.
    assert!(s.get("h1_direction").is_none());
    assert!(!s.to_string().contains("direction_fixed_after_smoke"));
    // The amendment block exists and states that no run had been made.
    let amend = &p["pre_run_tightening_2026-11-17"];
    assert!(amend["no_smoke_or_experimental_run_had_been_made"]
        .as_str()
        .unwrap()
        .contains("no smoke or experimental run had been made"));
    assert!(amend["changes"].as_array().unwrap().len() >= 7);
}

#[test]
fn h1_is_two_sided_and_no_smoke_output_can_change_that() {
    let p = prereg();
    let h1 = p["hypotheses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["id"] == "H1")
        .expect("H1");
    assert_eq!(h1["direction"].as_str(), Some("two_sided"));
    assert!(p.get("direction_fixed_after_smoke").is_none());
    assert!(p["h1_direction"].as_str().unwrap().starts_with("two_sided"));
}
// ---- Placeholders ----------------------------------------------------------------------------

#[test]
fn placeholders_exist_only_where_the_plan_allows_them() {
    let sentinel_value = |name: &str, key: &str| ic(&load(name)).get(key).copied();
    // Experimental manifests: sentinels (while open) in the three smoke-set values only.
    for name in [CONTROL, TREATMENT, DRIFT] {
        let m = ic(&load(name));
        for (k, v) in &m {
            let is_settable = k == LIVE_KEY_FOUNDERS || k == LIVE_KEY_FOOD_CAP;
            if !is_settable {
                assert!(
                    *v >= 0.0,
                    "{name}: '{k}' = {v} is a placeholder where none is allowed"
                );
            }
        }
    }
    // The three live-world sentinels are the registered ones.
    if PLACEHOLDERS_OPEN {
        assert_eq!(
            sentinel_value(CONTROL, LIVE_KEY_FOUNDERS),
            Some(SENTINEL_FOUNDERS)
        );
        assert_eq!(
            sentinel_value(TREATMENT, LIVE_KEY_FOUNDERS),
            Some(SENTINEL_FOUNDERS)
        );
        assert_eq!(
            sentinel_value(DRIFT, LIVE_KEY_FOUNDERS),
            Some(SENTINEL_FOUNDERS)
        );
        assert_eq!(
            sentinel_value(CONTROL, LIVE_KEY_FOOD_CAP),
            Some(SENTINEL_F_HI)
        );
        assert_eq!(
            sentinel_value(TREATMENT, LIVE_KEY_FOOD_CAP),
            Some(SENTINEL_F_LO)
        );
        assert_eq!(
            sentinel_value(DRIFT, LIVE_KEY_FOOD_CAP),
            Some(SENTINEL_F_LO)
        );
        // The sentinels are real values the live adapter cannot accept, and distinguish the arms.
        assert_ne!(SENTINEL_F_HI, SENTINEL_F_LO);
    } else {
        for name in [CONTROL, TREATMENT, DRIFT] {
            for (k, v) in ic(&load(name)) {
                assert!(
                    v >= 0.0,
                    "{name}: '{k}' still holds a sentinel after the amendment"
                );
            }
        }
        let c = ic(&load(CONTROL));
        assert!(c[LIVE_KEY_FOUNDERS] >= 30.0);
        assert!(ic(&load(TREATMENT))[LIVE_KEY_FOOD_CAP] < c[LIVE_KEY_FOOD_CAP]);
    }
    // Smoke manifests carry concrete starting candidates, never sentinels.
    for name in [SMOKE_CONTROL, SMOKE_TREATMENT, SMOKE_DRIFT] {
        for (k, v) in ic(&load(name)) {
            assert!(v >= 0.0, "{name}: '{k}' is a sentinel in a smoke manifest");
        }
    }

    // Placeholder strings in the JSON preregistration: only on the allowed paths.
    let p = prereg();
    let mut found: Vec<String> = Vec::new();
    collect_markers(&p, "", &mut found);
    found.sort();
    let allowed = [
        "common_initial_conditions.live.founders",
        "declared_factor.exact_initial_condition_difference.control_value",
        "declared_factor.exact_initial_condition_difference.treatment_value",
        "duration.duration_ticks",
    ];
    for f in &found {
        // `placeholders.*` and `smoke_may_set_only.*` describe the rule and may quote the marker.
        if f.starts_with("placeholders.") || f.starts_with("smoke_may_set_only.") {
            continue;
        }
        assert!(
            allowed.contains(&f.as_str()),
            "a [SMOKE] marker at '{f}' is outside what plan section 6 lets the smoke set"
        );
    }
    if PLACEHOLDERS_OPEN {
        for a in allowed {
            assert!(
                found.iter().any(|f| f == a),
                "expected an open placeholder at '{a}'"
            );
        }
    }
}

fn collect_markers(v: &serde_json::Value, at: &str, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s) if s.contains("[SMOKE") => out.push(at.to_string()),
        serde_json::Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                let p = if at.is_empty() {
                    i.to_string()
                } else {
                    format!("{at}.{i}")
                };
                collect_markers(x, &p, out);
            }
        }
        serde_json::Value::Object(o) => {
            for (k, x) in o {
                let p = if at.is_empty() {
                    k.clone()
                } else {
                    format!("{at}.{k}")
                };
                collect_markers(x, &p, out);
            }
        }
        _ => {}
    }
}

// ---- Seeds -----------------------------------------------------------------------------------

#[test]
fn twelve_seeds_follow_the_stated_rule_and_the_smoke_seed_is_excluded() {
    let expected: Vec<u64> = (0..12).map(|k| 760_001 + 1_000 * k).collect();
    for name in [CONTROL, TREATMENT, DRIFT] {
        let m = load(name);
        assert_eq!(
            m.seeds, expected,
            "{name}: seed_k = 760001 + 1000*k, in execution order"
        );
        assert!(
            !m.seeds.contains(&SMOKE_SEED),
            "{name} must not declare the smoke seed"
        );
    }
    for name in [SMOKE_CONTROL, SMOKE_TREATMENT, SMOKE_DRIFT] {
        assert_eq!(load(name).seeds, vec![SMOKE_SEED], "{name}");
    }
    // Disjoint from E2 (700001..), E3 draft (730001..), E2R (750001..): all of those are < 760001
    // by construction, and 999971 is far above the last E-SEL1 seed.
    assert!(expected.iter().all(|s| *s >= 760_001 && *s <= 771_001));
}

// ---- Metrics and sampling --------------------------------------------------------------------

#[test]
fn the_requested_observables_are_exactly_what_the_live_adapter_emits() {
    let reg = ObservableRegistry::live_default();
    // The manifests are frozen at the 22 ids the adapter emitted when E-SEL1 registered. Ids appended
    // since (card S9's two death-cause counts, owner-approved 2026-11-18) go after them and are not
    // requested here: the preregistered manifests are never edited to follow the adapter (QD-E2M).
    const EMITTED_AT_REGISTRATION: usize = 22;
    let emitted = all_live_observable_ids();
    let expected = &emitted[..EMITTED_AT_REGISTRATION];
    for name in ALL {
        let m = load(name);
        let declared: Vec<&str> = m.observable_ids.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            declared, expected,
            "{name}: emission order, nothing the adapter does not emit"
        );
        for id in &declared {
            let spec = reg
                .get(id)
                .unwrap_or_else(|| panic!("'{id}' has no registry spec"));
            assert!(spec.cadence_period > 0, "{id}");
        }
    }
    let p = prereg();
    for h in p["hypotheses"].as_array().expect("hypotheses") {
        let names: Vec<&str> = match (h["observable"].as_str(), h["observables"].as_array()) {
            (Some(o), _) => vec![o],
            (None, Some(a)) => a.iter().filter_map(|x| x.as_str()).collect(),
            _ => panic!("hypothesis {} names no observable", h["id"]),
        };
        for o in names {
            assert!(
                expected.contains(&o),
                "hypothesis {} names '{o}', which is not emitted",
                h["id"]
            );
        }
    }
}

#[test]
fn sampling_gives_a_tail_window_at_every_rung_and_fits_the_memory_ceiling() {
    let reg = ObservableRegistry::live_default();
    let p = prereg();
    let ladder: Vec<u64> = p["duration"]["duration_ticks_ladder"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert_eq!(ladder, vec![18_000, 12_000, 6_000]);
    let c = load(CONTROL);
    assert_eq!(
        ladder.first(),
        Some(&c.duration_ticks),
        "the manifests carry the top rung"
    );
    assert_eq!(
        p["duration"]["sample_period"].as_u64(),
        Some(c.sample_period)
    );
    assert_eq!(c.sample_period, 600);
    for t in &ladder {
        let tail = (1..=t / c.sample_period)
            .filter(|k| k * c.sample_period * 5 > 4 * t)
            .count();
        assert!(tail >= 1, "rung {t} has no sample with tick > 0.8 T");
        assert_eq!(t % c.sample_period, 0);
    }
    for name in ALL {
        let m = load(name);
        assert!(
            m.estimated_result_bytes(&reg) < MAX_ENSEMBLE_RESULT_BYTES,
            "{name}"
        );
    }
}

// ---- Plan and manifests cannot drift apart ---------------------------------------------------

#[test]
fn the_preregistration_agrees_with_the_manifests_it_describes() {
    let p = prereg();
    assert_eq!(p["contains_results"], serde_json::json!(false));
    assert_eq!(p["registered_on"].as_str(), Some("2026-11-17"));
    assert!(p["registered_at_parent_commit"]
        .as_str()
        .is_some_and(|s| s.len() >= 7));
    assert_eq!(p["status"].as_str(), Some("FROZEN"));

    let c = load(CONTROL);
    let planned: Vec<u64> = p["seeds"]["execution_order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert_eq!(planned, c.seeds);
    assert_eq!(p["seeds"]["smoke_seed"].as_u64(), Some(SMOKE_SEED));
    assert_eq!(p["seeds"]["n_requested"].as_u64(), Some(12));
    assert_eq!(
        p["seeds"]["min_complete_pairs_for_a_decision"].as_u64(),
        Some(10)
    );
    assert_eq!(p["cost"]["max_wall_clock_minutes"].as_u64(), Some(90));

    assert_eq!(
        p["declared_factor"]["exact_initial_condition_difference"]["key"].as_str(),
        Some(LIVE_KEY_FOOD_CAP)
    );
    assert_eq!(
        p["declared_factor"]["drift_contrast_exact_initial_condition_difference"]["key"].as_str(),
        Some(LIVE_KEY_MUTATION_RATE)
    );
    let files = &p["manifests"];
    assert_eq!(files["control"].as_str(), Some(CONTROL));
    assert_eq!(files["treatment"].as_str(), Some(TREATMENT));
    assert_eq!(
        files["drift_control_optional_requires_P4"].as_str(),
        Some(DRIFT)
    );
    assert_eq!(files["smoke_control"].as_str(), Some(SMOKE_CONTROL));
    assert_eq!(files["smoke_treatment"].as_str(), Some(SMOKE_TREATMENT));
    assert_eq!(files["smoke_drift"].as_str(), Some(SMOKE_DRIFT));

    // The thresholds the runner hard-codes are the registered ones.
    let conds = p["thresholds"]["conditions"].as_array().unwrap();
    assert_eq!(conds.len(), 3);
    assert!(conds[1].as_str().unwrap().contains("0.8"));
    assert!(conds[2].as_str().unwrap().contains("0.05"));

    // All five preconditions are recorded as satisfied, each with evidence.
    let pre = p["blocking_preconditions"].as_array().unwrap();
    let ids: Vec<&str> = pre.iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["P1", "P2", "P3", "P4", "P5"]);
    for x in pre {
        assert_eq!(x["status"].as_str(), Some("satisfied"), "{x}");
        assert!(x["evidence"].as_str().is_some_and(|e| !e.is_empty()));
    }
}

#[test]
fn the_reproduction_commands_never_launch_the_app() {
    let p = prereg();
    for k in ["reproduction_command", "smoke_command"] {
        let s = p[k].as_str().expect(k);
        assert!(!s.contains("cargo run"), "{k} must not use cargo run");
        assert!(s.contains("--release"), "{k} must be a release build");
        assert!(s.contains("run_esel1_experiment"), "{k}");
    }
    assert!(p["smoke_command"].as_str().unwrap().contains("--smoke"));
    assert!(p["reproduction_command"]
        .as_str()
        .unwrap()
        .contains("--ensemble"));
}

#[test]
fn an_unamended_experimental_manifest_is_refused_by_the_live_config_parser() {
    // Pure parsing of initial conditions: no adapter is built and no tick is stepped.
    for name in [CONTROL, TREATMENT, DRIFT] {
        let parsed = LiveWorldConfig::from_initial_conditions(&load(name).initial_conditions);
        if PLACEHOLDERS_OPEN {
            assert!(
                parsed.is_err(),
                "{name} still holds sentinels and must not be runnable"
            );
        } else {
            assert!(parsed.is_ok(), "{name}: {:?}", parsed.err());
        }
    }
    for name in [SMOKE_CONTROL, SMOKE_TREATMENT, SMOKE_DRIFT] {
        let parsed = LiveWorldConfig::from_initial_conditions(&load(name).initial_conditions);
        assert!(
            parsed.is_ok(),
            "{name} must be runnable: {:?}",
            parsed.err()
        );
    }
}

#[test]
fn a_closed_registration_keeps_its_experimental_manifests_unrunnable() {
    // Amendment 2 (2026-11-17) closed E-SEL1 at the smoke. A closed registration must never be
    // run by accident, so it may not fill its sentinels (`PLACEHOLDERS_OPEN` stays `true`): the
    // live config parser keeps refusing the three experimental manifests, and no execution lock
    // may exist.
    let p = prereg();
    let outcome = p["registration_outcome"].as_str().unwrap_or("");
    if outcome.starts_with("CLOSED_WITHOUT_EXPERIMENT") {
        for name in [CONTROL, TREATMENT, DRIFT] {
            let parsed = LiveWorldConfig::from_initial_conditions(&load(name).initial_conditions);
            assert!(
                parsed.is_err(),
                "{name}: a closed registration's experimental manifest became runnable"
            );
        }
        let amendment = &p["amendment_2_2026-11-17_smoke_run_2_stopped_registration_closed"];
        assert_eq!(
            amendment["smoke_run_2"]["body_mass_consulted"],
            serde_json::json!(false)
        );
        let lock = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../artifacts/experiments/e-sel1-food-scarcity/execution-lock.json");
        assert!(
            !lock.exists(),
            "a closed registration has an execution lock at {}",
            lock.display()
        );
    }
}
