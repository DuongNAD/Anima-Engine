//! Preregistration lock and preflight validation for E3b (`2026-11-24-e3b-preregistration.md`).
//!
//! Static verification only: validates all 7 fixtures in `tests/fixtures/experiments_e3b/`,
//! checks the 31-key partition (`20` explicit + `11` default-absent), confirms 64 R1 seeds
//! (`810001..=873001`, capped at `N <= 70`), enforces disjointness from `880001..=880012` and
//! all prior experiments, enforces zero `trait_mass` observables in smoke/pre-run paths, verifies
//! engine preflight failure on missing required keys, and includes negative-control failure tests.
//! Never executes any simulation seed and never reads any empirical `trait_mass` output.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anima_engine_lib::core::experiment::{
    ExperimentError, ExperimentManifest, FactorDiff, InitialConditionSet, ObservableRegistry,
    ObserverPolicy, MANIFEST_SCHEMA_VERSION, MAX_ENSEMBLE_RESULT_BYTES,
};
use anima_engine_lib::core::intervention::{Curve, InterventionKind, Region};
use anima_engine_lib::core::live_experiment::{
    all_live_observable_ids, mass_statistics, LiveWorldConfig, LIVE_KEYS,
    LIVE_KEY_DEHYDRATION_TICKS, LIVE_KEY_DRIVE_FLOOR, LIVE_KEY_ECOLOGICAL_FITNESS,
    LIVE_KEY_EVOLVED_BRAINS, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS, LIVE_KEY_FOUNDER_GENOTYPE,
    LIVE_KEY_FOUNDER_LAYOUT, LIVE_KEY_LAKES, LIVE_KEY_LAKE_LAYOUT, LIVE_KEY_LOCOMOTION,
    LIVE_KEY_MORTALITY, LIVE_KEY_MUTATION_RATE, LIVE_KEY_PREDATOR_FRACTION, LIVE_KEY_PREY_CAP,
    LIVE_KEY_REPRODUCTION, LIVE_KEY_RESOURCE_CAPACITY_SCALE, LIVE_KEY_STRICT_PREY_CAP,
    LIVE_KEY_THIRST_STEERING, LIVE_KEY_TREES,
};

const PREREG_DIR: &str = "tests/fixtures/experiments_e3b";
const PREREG_FILE: &str = "e3b-preregistration.json";

const CONTROL: &str = "e3b-control-uniform.json";
const CONTROL_SCARCITY: &str = "e3b-control-uniform-scarcity.json";
const TREATMENT: &str = "e3b-treatment-gradient.json";
const SMOKE_CONTROL: &str = "e3b-smoke-control-uniform.json";
const SMOKE_CONTROL_SCARCITY: &str = "e3b-smoke-control-uniform-scarcity.json";
const SMOKE_TREATMENT: &str = "e3b-smoke-treatment-gradient.json";

const ALL_MANIFESTS: [&str; 6] = [
    CONTROL,
    CONTROL_SCARCITY,
    TREATMENT,
    SMOKE_CONTROL,
    SMOKE_CONTROL_SCARCITY,
    SMOKE_TREATMENT,
];

const SEED_BASE: u64 = 810_001;
const SEED_STEP: u64 = 1_000;
const SEED_COUNT: usize = 64;
const MAX_N_CAP: usize = 70;
const SMOKE_SEED: u64 = 999_953;
const EXPECTED_DURATION: u64 = 18_000;
const EXPECTED_PERIOD: u64 = 600;
const PINNED_TRUNK_FULL: &str = "b0413f2eeecbab8e5ee045c22c72d5c00c16b87a";

const REQUIRED_SHARED_KEYS: [(&str, f64); 19] = [
    (LIVE_KEY_ECOLOGICAL_FITNESS, 1.0),
    (LIVE_KEY_MORTALITY, 1.0),
    (LIVE_KEY_REPRODUCTION, 1.0),
    (LIVE_KEY_LOCOMOTION, 1.0),
    (LIVE_KEY_DRIVE_FLOOR, 0.5),
    (LIVE_KEY_THIRST_STEERING, 1.0),
    (LIVE_KEY_EVOLVED_BRAINS, 0.0),
    (LIVE_KEY_FOUNDERS, 160.0),
    (LIVE_KEY_FOUNDER_LAYOUT, 1.0),
    (LIVE_KEY_FOUNDER_GENOTYPE, 1.0),
    (LIVE_KEY_PREDATOR_FRACTION, 0.3),
    (LIVE_KEY_TREES, 8.0),
    (LIVE_KEY_LAKES, 16.0),
    (LIVE_KEY_LAKE_LAYOUT, 1.0),
    (LIVE_KEY_FOOD_CAP, 50.0),
    (LIVE_KEY_PREY_CAP, 160.0),
    (LIVE_KEY_STRICT_PREY_CAP, 1.0),
    (LIVE_KEY_DEHYDRATION_TICKS, 960.0),
    (LIVE_KEY_MUTATION_RATE, 0.05),
];

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    crate_root()
        .parent()
        .expect("src-tauri must live under repo root")
        .to_path_buf()
}

fn fixture_path(name: &str) -> PathBuf {
    crate_root().join(PREREG_DIR).join(name)
}

fn load_json(rel_from_crate: &str) -> serde_json::Value {
    let raw = std::fs::read_to_string(crate_root().join(rel_from_crate))
        .unwrap_or_else(|e| panic!("read {rel_from_crate}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {rel_from_crate}: {e}"))
}

fn load_manifest(name: &str) -> ExperimentManifest {
    let raw = std::fs::read_to_string(fixture_path(name))
        .unwrap_or_else(|e| panic!("read manifest {name}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse manifest {name}: {e}"))
}

fn prereg() -> serde_json::Value {
    load_json(&format!("{PREREG_DIR}/{PREREG_FILE}"))
}

fn ic_map(m: &ExperimentManifest) -> BTreeMap<String, f64> {
    m.initial_conditions.values.iter().cloned().collect()
}

fn diff_ic_keys(a: &ExperimentManifest, b: &ExperimentManifest) -> Vec<String> {
    let (ma, mb) = (ic_map(a), ic_map(b));
    let keys: BTreeSet<String> = ma.keys().chain(mb.keys()).cloned().collect();
    keys.into_iter()
        .filter(|k| ma.get(k) != mb.get(k))
        .collect()
}

fn collect_seeds_from_manifest(rel: &str) -> BTreeSet<u64> {
    let v = load_json(rel);
    v["seeds"]
        .as_array()
        .unwrap_or_else(|| panic!("{rel} must contain seeds array"))
        .iter()
        .map(|s| {
            s.as_u64()
                .unwrap_or_else(|| panic!("{rel} seed must be u64"))
        })
        .collect()
}

fn validate_e3b_required_keys(m: &ExperimentManifest, name: &str) -> Result<(), String> {
    let map = ic_map(m);
    for (key, expected) in REQUIRED_SHARED_KEYS {
        let Some(actual) = map.get(key) else {
            return Err(format!("{name}: missing required E3b key '{key}'"));
        };
        if (*actual - expected).abs() > 1e-12 {
            return Err(format!(
                "{name}: key '{key}' expected {expected}, found {actual}"
            ));
        }
    }
    if !map.contains_key(LIVE_KEY_RESOURCE_CAPACITY_SCALE) {
        return Err(format!(
            "{name}: missing required per-arm key '{LIVE_KEY_RESOURCE_CAPACITY_SCALE}'"
        ));
    }
    if map.len() != REQUIRED_SHARED_KEYS.len() + 1 {
        return Err(format!(
            "{name}: expected {} explicit keys, found {}",
            REQUIRED_SHARED_KEYS.len() + 1,
            map.len()
        ));
    }
    Ok(())
}

fn validate_smoke_blindness(m: &ExperimentManifest, name: &str) -> Result<(), String> {
    for obs in &m.observable_ids {
        if obs.to_ascii_lowercase().contains("trait_mass") {
            return Err(format!(
                "{name}: smoke manifest contains forbidden trait_mass observable '{obs}'"
            ));
        }
    }
    let expected_smoke: Vec<String> = all_live_observable_ids()
        .into_iter()
        .filter(|id| !id.contains("trait_mass"))
        .map(str::to_string)
        .collect();
    if m.observable_ids != expected_smoke {
        return Err(format!(
            "{name}: smoke observable_ids drifted from the 22 non-mass live observables"
        ));
    }
    Ok(())
}

fn validate_key_partition(
    manifests: &[(&str, &ExperimentManifest)],
    defaults_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let live_keys: BTreeSet<String> = LIVE_KEYS.iter().map(|s| (*s).to_string()).collect();
    let mut defaults = BTreeMap::new();
    for (k, v) in defaults_obj {
        let val = v
            .as_f64()
            .ok_or_else(|| format!("default_keys_left_absent['{k}'] must be numeric"))?;
        defaults.insert(k.clone(), val);
    }
    let default_keys: BTreeSet<String> = defaults.keys().cloned().collect();

    for (name, m) in manifests {
        let explicit: BTreeSet<String> = m
            .initial_conditions
            .values
            .iter()
            .map(|(k, _)| k.clone())
            .collect();
        if explicit.len() != m.initial_conditions.values.len() {
            return Err(format!("{name}: duplicate key in initial_conditions"));
        }
        if !explicit.is_disjoint(&default_keys) {
            let overlap: Vec<String> = explicit.intersection(&default_keys).cloned().collect();
            return Err(format!(
                "{name}: explicit keys overlap default_keys_left_absent: {overlap:?}"
            ));
        }
        let combined: BTreeSet<String> = explicit.union(&default_keys).cloned().collect();
        if combined != live_keys {
            return Err(format!(
                "{name}: explicit U default_keys_left_absent != LIVE_KEYS"
            ));
        }
    }

    let engine_default = LiveWorldConfig::default();
    for (k, v) in &defaults {
        let parsed = LiveWorldConfig::from_initial_conditions(&InitialConditionSet::new(vec![(
            k.clone(),
            *v,
        )]))
        .map_err(|e| format!("default '{k}' = {v} rejected: {e}"))?;
        if parsed != engine_default {
            return Err(format!(
                "default_keys_left_absent '{k}' = {v} differs from LiveWorldConfig::default()"
            ));
        }
    }
    Ok(())
}

fn validate_seed_schedule(
    manifests: &[(&str, &ExperimentManifest)],
    forbidden: &BTreeSet<u64>,
) -> Result<(), String> {
    let expected: Vec<u64> = (0..SEED_COUNT as u64)
        .map(|k| SEED_BASE + SEED_STEP * k)
        .collect();
    if expected.len() > MAX_N_CAP {
        return Err(format!("N={} exceeds cap {}", expected.len(), MAX_N_CAP));
    }
    for (name, m) in manifests {
        if m.seeds != expected {
            return Err(format!(
                "{name}: seeds mismatch expected formula 810001 + 1000*k (len {})",
                m.seeds.len()
            ));
        }
        for s in &m.seeds {
            if forbidden.contains(s) {
                return Err(format!(
                    "{name}: confirmation seed {s} collides with forbidden range"
                ));
            }
        }
    }
    Ok(())
}

fn validate_arm_contrast_and_factors(
    cu: &ExperimentManifest,
    cs: &ExperimentManifest,
    tr: &ExperimentManifest,
    label: &str,
) -> Result<(), String> {
    let allow_ut = FactorDiff {
        allowed_paths: vec!["interventions".to_string()],
    };
    let ut_diff = allow_ut
        .validate(cu, tr)
        .map_err(|e| format!("{label} U vs T FactorDiff failed: {e}"))?;
    if ut_diff != vec!["interventions".to_string()] {
        return Err(format!("{label} U vs T diff mismatch: {ut_diff:?}"));
    }

    let allow_scarcity = FactorDiff {
        allowed_paths: vec![
            "initial_conditions".to_string(),
            "interventions".to_string(),
        ],
    };
    let mut us_diff = allow_scarcity
        .validate(cu, cs)
        .map_err(|e| format!("{label} U vs S FactorDiff failed: {e}"))?;
    us_diff.sort();
    if us_diff
        != vec![
            "initial_conditions".to_string(),
            "interventions".to_string(),
        ]
    {
        return Err(format!("{label} U vs S diff mismatch: {us_diff:?}"));
    }

    let mut st_diff = allow_scarcity
        .validate(cs, tr)
        .map_err(|e| format!("{label} S vs T FactorDiff failed: {e}"))?;
    st_diff.sort();
    if st_diff
        != vec![
            "initial_conditions".to_string(),
            "interventions".to_string(),
        ]
    {
        return Err(format!("{label} S vs T diff mismatch: {st_diff:?}"));
    }

    if diff_ic_keys(cu, cs) != vec![LIVE_KEY_RESOURCE_CAPACITY_SCALE.to_string()] {
        return Err(format!(
            "{label} U vs S IC diff must be solely live.resource_capacity_scale"
        ));
    }
    if diff_ic_keys(cs, tr) != vec![LIVE_KEY_RESOURCE_CAPACITY_SCALE.to_string()] {
        return Err(format!(
            "{label} S vs T IC diff must be solely live.resource_capacity_scale"
        ));
    }
    if !diff_ic_keys(cu, tr).is_empty() {
        return Err(format!("{label} U vs T IC diff must be empty"));
    }
    Ok(())
}

fn collect_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_files_recursive(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn forbidden_seed_set() -> BTreeSet<u64> {
    let mut forbidden = BTreeSet::new();
    // Calibration seeds (VIAB-1 / VIAB-1b / PR #216 probes): 880001..=880012 (including k=70 collision)
    forbidden.extend(880_001..=880_012);
    // AE toy baseline seeds
    forbidden.extend(1..=3);
    // E2R exploratory seeds
    forbidden.extend((0..12).map(|k| 750_001 + 1_000 * k));
    // All smoke seeds across campaigns
    forbidden.extend([999_983, 999_979, 999_971, 999_961, SMOKE_SEED]);
    // SEL2-P extra exploratory range
    forbidden.extend(999_801..=999_822);

    for rel in [
        "tests/fixtures/experiments_e2/e2-control-shared-brain.json",
        "tests/fixtures/experiments_e2/e2-smoke-control-shared-brain.json",
        "tests/fixtures/experiments_e3/e3-control-uniform.json",
        "tests/fixtures/experiments_e3/e3-smoke-control-uniform.json",
        "tests/fixtures/experiments_esel1/esel1-control-abundant.json",
        "tests/fixtures/experiments_esel1/esel1-smoke-abundant.json",
        "tests/fixtures/experiments_sel2p/sel2p-selected.json",
        "tests/fixtures/experiments_sel2p/sel2p-smoke-selected.json",
    ] {
        forbidden.extend(collect_seeds_from_manifest(rel));
    }
    forbidden
}

#[test]
fn every_e3b_manifest_parses_validates_passes_preflight_and_fits_memory_ceiling() {
    let reg = ObservableRegistry::live_default();
    reg.validate().expect("live registry must validate");

    for name in ALL_MANIFESTS {
        let m = load_manifest(name);
        assert_eq!(m.schema_version, MANIFEST_SCHEMA_VERSION, "{name}");
        m.validate(&reg)
            .unwrap_or_else(|e| panic!("{name} failed ExperimentManifest::validate: {e}"));

        for (k, _) in &m.initial_conditions.values {
            assert!(
                LIVE_KEYS.contains(&k.as_str()),
                "{name} declares unknown key '{k}'"
            );
        }

        validate_e3b_required_keys(&m, name)
            .unwrap_or_else(|e| panic!("required key validation failed: {e}"));

        let cfg =
            LiveWorldConfig::from_initial_conditions(&m.initial_conditions).unwrap_or_else(|e| {
                panic!("{name} failed LiveWorldConfig::from_initial_conditions: {e}")
            });
        cfg.ensure_observables_supported(&m.observable_ids)
            .unwrap_or_else(|e| panic!("{name} failed ensure_observables_supported: {e}"));

        assert_eq!(m.duration_ticks, EXPECTED_DURATION, "{name}");
        assert_eq!(m.sample_period, EXPECTED_PERIOD, "{name}");
        assert_eq!(m.observer, ObserverPolicy::Absent, "{name}");
        assert!(m.exotic_interventions.is_empty(), "{name}");

        let bytes = m.estimated_result_bytes(&reg);
        assert!(
            bytes < MAX_ENSEMBLE_RESULT_BYTES,
            "{name}: estimated {bytes} bytes exceeds {MAX_ENSEMBLE_RESULT_BYTES}"
        );
    }
}

#[test]
fn explicit_and_default_keys_partition_all_31_live_keys_and_match_engine_defaults() {
    let p = prereg();
    let defaults_obj = p["default_keys_left_absent"]
        .as_object()
        .expect("default_keys_left_absent must be an object");
    assert_eq!(
        LIVE_KEYS.len(),
        31,
        "expected 31 LIVE_KEYS on trunk b0413f2"
    );
    assert_eq!(
        defaults_obj.len(),
        11,
        "E3b declares 20 explicit keys + 11 default-absent keys = 31 LIVE_KEYS"
    );

    let loaded: Vec<(&str, ExperimentManifest)> = ALL_MANIFESTS
        .iter()
        .map(|&n| (n, load_manifest(n)))
        .collect();
    let refs: Vec<(&str, &ExperimentManifest)> = loaded.iter().map(|(n, m)| (*n, m)).collect();
    validate_key_partition(&refs, defaults_obj)
        .unwrap_or_else(|e| panic!("key partition check failed: {e}"));

    // Spot-check all section 1.1 overrides explicitly demanded by E3B-FREEZE.
    for (name, m) in &refs {
        let map = ic_map(m);
        assert_eq!(map.get(LIVE_KEY_FOUNDER_GENOTYPE), Some(&1.0), "{name}");
        assert_eq!(map.get(LIVE_KEY_PREDATOR_FRACTION), Some(&0.3), "{name}");
        assert_eq!(map.get(LIVE_KEY_MUTATION_RATE), Some(&0.05), "{name}");
        assert_eq!(map.get(LIVE_KEY_FOUNDERS), Some(&160.0), "{name}");
        assert_eq!(map.get(LIVE_KEY_PREY_CAP), Some(&160.0), "{name}");
        assert_eq!(map.get(LIVE_KEY_STRICT_PREY_CAP), Some(&1.0), "{name}");
        assert_eq!(map.get(LIVE_KEY_LAKES), Some(&16.0), "{name}");
        assert_eq!(map.get(LIVE_KEY_LAKE_LAYOUT), Some(&1.0), "{name}");
        assert_eq!(map.get(LIVE_KEY_DEHYDRATION_TICKS), Some(&960.0), "{name}");
        assert_eq!(map.get(LIVE_KEY_FOUNDER_LAYOUT), Some(&1.0), "{name}");
    }
}

#[test]
fn arms_differ_only_in_declared_arm_keys_and_interventions() {
    let cu = load_manifest(CONTROL);
    let cs = load_manifest(CONTROL_SCARCITY);
    let tr = load_manifest(TREATMENT);
    let scu = load_manifest(SMOKE_CONTROL);
    let scs = load_manifest(SMOKE_CONTROL_SCARCITY);
    let str_m = load_manifest(SMOKE_TREATMENT);

    validate_arm_contrast_and_factors(&cu, &cs, &tr, "confirmation").expect("confirmation factors");
    validate_arm_contrast_and_factors(&scu, &scs, &str_m, "smoke").expect("smoke factors");

    for (u, s, t) in [(&cu, &cs, &tr), (&scu, &scs, &str_m)] {
        assert_eq!(ic_map(u).get(LIVE_KEY_RESOURCE_CAPACITY_SCALE), Some(&1.0));
        assert_eq!(ic_map(s).get(LIVE_KEY_RESOURCE_CAPACITY_SCALE), Some(&0.42));
        assert_eq!(ic_map(t).get(LIVE_KEY_RESOURCE_CAPACITY_SCALE), Some(&1.0));

        assert_ne!(u.fingerprint(), s.fingerprint());
        assert_ne!(u.fingerprint(), t.fingerprint());
        assert_ne!(s.fingerprint(), t.fingerprint());

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

        assert_eq!(u.interventions.len(), 2);
        assert_eq!(s.interventions.len(), 2);
        assert_eq!(t.interventions.len(), 2);

        for iv in u
            .interventions
            .iter()
            .chain(s.interventions.iter())
            .chain(t.interventions.iter())
        {
            assert_eq!(iv.start_tick, 1);
            assert_eq!(iv.duration_ticks, EXPECTED_DURATION);
            assert_eq!(iv.curve, Curve::Step);
            assert!(!iv.reversible);
            assert!(!iv.signed_negative);
        }

        assert_eq!(u.interventions[0].kind, InterventionKind::Deforest);
        assert_eq!(u.interventions[0].region, west_half);
        assert_eq!(u.interventions[0].intensity, 0.0);
        assert_eq!(u.interventions[1].kind, InterventionKind::AddNutrient);
        assert_eq!(u.interventions[1].region, east_half);
        assert_eq!(u.interventions[1].intensity, 0.0);

        assert_eq!(s.interventions[0].kind, InterventionKind::Deforest);
        assert_eq!(s.interventions[0].region, west_half);
        assert_eq!(s.interventions[0].intensity, 0.0);
        assert_eq!(s.interventions[1].kind, InterventionKind::AddNutrient);
        assert_eq!(s.interventions[1].region, whole_map);
        assert_eq!(s.interventions[1].intensity, 1.0);

        assert_eq!(t.interventions[0].kind, InterventionKind::Deforest);
        assert_eq!(t.interventions[0].region, west_half);
        assert_eq!(t.interventions[0].intensity, 0.3);
        assert_eq!(t.interventions[1].kind, InterventionKind::AddNutrient);
        assert_eq!(t.interventions[1].region, east_half);
        assert_eq!(t.interventions[1].intensity, 1.0);
    }

    // Smoke vs confirmation manifests differ only in identity/name, seeds, and stripped trait_mass ids.
    for (conf, smoke) in [(&cu, &scu), (&cs, &scs), (&tr, &str_m)] {
        assert_eq!(conf.laws, smoke.laws);
        assert_eq!(conf.initial_conditions, smoke.initial_conditions);
        assert_eq!(conf.interventions, smoke.interventions);
        assert_eq!(conf.duration_ticks, smoke.duration_ticks);
        assert_eq!(conf.sample_period, smoke.sample_period);
        assert_eq!(conf.observer, smoke.observer);
    }
}

#[test]
fn sixty_four_seeds_match_r1_formula_cap_and_are_disjoint_from_all_calibration_and_prior_seeds() {
    assert_eq!(
        SEED_BASE + SEED_STEP * 70,
        880_001,
        "R1 collision proof: k=70 hits calibration seed 880001"
    );
    let cu = load_manifest(CONTROL);
    let cs = load_manifest(CONTROL_SCARCITY);
    let tr = load_manifest(TREATMENT);
    let forbidden = forbidden_seed_set();

    validate_seed_schedule(
        &[(CONTROL, &cu), (CONTROL_SCARCITY, &cs), (TREATMENT, &tr)],
        &forbidden,
    )
    .unwrap_or_else(|e| panic!("seed schedule invalid: {e}"));

    assert_eq!(cu.seeds.first(), Some(&810_001));
    assert_eq!(cu.seeds.last(), Some(&873_001));
    assert!(*cu.seeds.last().unwrap() < 880_001);

    for (name, sm) in [
        (SMOKE_CONTROL, load_manifest(SMOKE_CONTROL)),
        (
            SMOKE_CONTROL_SCARCITY,
            load_manifest(SMOKE_CONTROL_SCARCITY),
        ),
        (SMOKE_TREATMENT, load_manifest(SMOKE_TREATMENT)),
    ] {
        assert_eq!(sm.seeds, vec![SMOKE_SEED], "{name}");
        assert!(!cu.seeds.contains(&SMOKE_SEED));
    }
}

#[test]
fn no_trait_mass_observable_in_smoke_manifests_or_pre_run_artifact_paths() {
    let reg = ObservableRegistry::live_default();
    let all_emitted: Vec<String> = all_live_observable_ids()
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(
        all_emitted.len(),
        28,
        "expected 28 live observables on b0413f2"
    );

    for name in [CONTROL, CONTROL_SCARCITY, TREATMENT] {
        let m = load_manifest(name);
        assert_eq!(m.observable_ids, all_emitted, "{name}");
        for id in &m.observable_ids {
            assert!(reg.get(id).is_some(), "{name} missing '{id}' in registry");
        }
    }

    for name in [SMOKE_CONTROL, SMOKE_CONTROL_SCARCITY, SMOKE_TREATMENT] {
        let m = load_manifest(name);
        validate_smoke_blindness(&m, name)
            .unwrap_or_else(|e| panic!("smoke blindness failed: {e}"));
        let raw = std::fs::read_to_string(fixture_path(name)).expect("read raw smoke fixture");
        assert!(
            !raw.to_ascii_lowercase().contains("trait_mass"),
            "{name} raw JSON text must contain zero 'trait_mass' substrings"
        );
    }

    // Freeze-time state (no confirmation/) is recorded by the freeze commit `ebe636d` (#231).
    // Rescoped for E3B-RUN with the owner's authorization of 2026-11-24: keep the recursive
    // `trait_mass` scan across artifacts/experiments/e3b-productivity-gradient-body-mass/
    // excluding only `confirmation/` and top-level `RESULT.md`, and require any `confirmation/`
    // bundle to carry a parsed `confirmation/provenance.json` with `smoke_reference.wp2_passed == true`.
    let art_root = repo_root().join("artifacts/experiments/e3b-productivity-gradient-body-mass");
    let confirm_dir = art_root.join("confirmation");
    if confirm_dir.exists() {
        let prov_path = confirm_dir.join("provenance.json");
        let prov_raw = std::fs::read_to_string(&prov_path).unwrap_or_else(|e| {
            panic!("read confirmation provenance {}: {e}", prov_path.display())
        });
        let prov: serde_json::Value = serde_json::from_str(&prov_raw).unwrap_or_else(|e| {
            panic!("parse confirmation provenance {}: {e}", prov_path.display())
        });
        assert_eq!(
            prov.pointer("/smoke_reference/wp2_passed"),
            Some(&serde_json::Value::Bool(true)),
            "confirmation bundle {} must carry smoke_reference.wp2_passed == true",
            prov_path.display()
        );
    }
    if art_root.exists() {
        let mut files = Vec::new();
        collect_files_recursive(&art_root, &mut files);
        for f in files {
            let rel = f
                .strip_prefix(&art_root)
                .unwrap_or_else(|e| panic!("strip art_root prefix from {}: {e}", f.display()));
            let in_confirmation = rel.components().next()
                == Some(std::path::Component::Normal(std::ffi::OsStr::new(
                    "confirmation",
                )));
            let is_top_level_result = rel == Path::new("RESULT.md");
            if in_confirmation || is_top_level_result {
                continue;
            }
            let content = std::fs::read_to_string(&f)
                .unwrap_or_else(|e| panic!("read pre-run/smoke artifact {}: {e}", f.display()));
            assert!(
                !content.to_ascii_lowercase().contains("trait_mass"),
                "pre-run/smoke artifact {} contains forbidden 'trait_mass' substring",
                f.display()
            );
        }
    }
}

#[test]
fn missing_required_key_fails_preflight() {
    let base = load_manifest(CONTROL);

    // 1. Engine preflight refuses death observables if live.mortality is absent or 0.0.
    let mut no_mortality = base.clone();
    no_mortality
        .initial_conditions
        .values
        .retain(|(k, _)| k != LIVE_KEY_MORTALITY);
    let cfg_no_mort = LiveWorldConfig::from_initial_conditions(&no_mortality.initial_conditions)
        .expect("LiveWorldConfig parses without mortality");
    let err_mort = cfg_no_mort
        .ensure_observables_supported(&no_mortality.observable_ids)
        .expect_err("dropping live.mortality must fail observable preflight");
    assert!(
        matches!(err_mort, ExperimentError::InvalidObservableRequest { .. }),
        "unexpected error: {err_mort:?}"
    );

    // 2. Engine preflight refuses birth observables if live.reproduction is absent or 0.0.
    let mut no_repro = base.clone();
    no_repro
        .initial_conditions
        .values
        .retain(|(k, _)| k != LIVE_KEY_REPRODUCTION);
    let cfg_no_repro = LiveWorldConfig::from_initial_conditions(&no_repro.initial_conditions)
        .expect("LiveWorldConfig parses without reproduction");
    let err_repro = cfg_no_repro
        .ensure_observables_supported(&no_repro.observable_ids)
        .expect_err("dropping live.reproduction must fail observable preflight");
    assert!(
        matches!(err_repro, ExperimentError::InvalidObservableRequest { .. }),
        "unexpected error: {err_repro:?}"
    );

    // 3. Engine preflight refuses live.lakes = 0.0 when live.lake_layout = 1.0.
    let mut no_lakes = base.clone();
    for (k, v) in &mut no_lakes.initial_conditions.values {
        if k == LIVE_KEY_LAKES {
            *v = 0.0;
        }
    }
    let err_lakes = LiveWorldConfig::from_initial_conditions(&no_lakes.initial_conditions)
        .expect_err("live.lake_layout = 1.0 with live.lakes = 0.0 must fail");
    assert!(
        matches!(err_lakes, ExperimentError::InvalidLaw { .. }),
        "unexpected error: {err_lakes:?}"
    );

    // 4. E3b preflight rejects manifests missing any required section 1.1 key.
    for (req_key, _) in REQUIRED_SHARED_KEYS {
        let mut dropped = base.clone();
        dropped
            .initial_conditions
            .values
            .retain(|(k, _)| k != req_key);
        let err = validate_e3b_required_keys(&dropped, "mutated")
            .expect_err("dropping required key must fail E3b preflight");
        assert!(
            err.contains(req_key),
            "error should name '{req_key}', got: {err}"
        );
    }
}

#[test]
fn negative_controls_fail_when_a_manifest_or_rule_is_wrong() {
    let cu = load_manifest(CONTROL);
    let cs = load_manifest(CONTROL_SCARCITY);
    let tr = load_manifest(TREATMENT);
    let scu = load_manifest(SMOKE_CONTROL);
    let forbidden = forbidden_seed_set();

    // Control 1: Undeclared factor change between U and T must fail FactorDiff.
    let mut bad_tr = tr.clone();
    for (k, v) in &mut bad_tr.initial_conditions.values {
        if k == LIVE_KEY_FOUNDERS {
            *v = 159.0;
        }
    }
    assert!(
        validate_arm_contrast_and_factors(&cu, &cs, &bad_tr, "neg-factor").is_err(),
        "undeclared factor diff must fail"
    );

    // Control 2: Leaking a trait_mass observable into a smoke manifest must fail blindness.
    let mut bad_smoke = scu.clone();
    bad_smoke
        .observable_ids
        .push("live.trait_mass_var".to_string());
    assert!(
        validate_smoke_blindness(&bad_smoke, "neg-smoke").is_err(),
        "trait_mass leak into smoke must fail"
    );

    // Control 3: Calibration or smoke seed inside confirmation seeds must fail seed check.
    let mut bad_cal_seed = cu.clone();
    bad_cal_seed.seeds[63] = 880_001;
    assert!(
        validate_seed_schedule(&[("neg-cal", &bad_cal_seed)], &forbidden).is_err(),
        "calibration seed 880001 in confirmation manifest must fail"
    );
    let mut bad_smoke_seed = cu.clone();
    bad_smoke_seed.seeds[0] = SMOKE_SEED;
    assert!(
        validate_seed_schedule(&[("neg-smoke-seed", &bad_smoke_seed)], &forbidden).is_err(),
        "smoke seed 999953 in confirmation manifest must fail"
    );

    // Control 4: Unknown live key fails LiveWorldConfig preflight.
    let mut bad_unknown_key = cu.clone();
    bad_unknown_key
        .initial_conditions
        .values
        .push(("live.undeclared_knob".to_string(), 1.0));
    assert!(
        LiveWorldConfig::from_initial_conditions(&bad_unknown_key.initial_conditions).is_err(),
        "unknown live key must fail preflight"
    );

    // Control 5: Missing a key from default_keys_left_absent breaks the 31-key partition.
    let p = prereg();
    let mut broken_defaults = p["default_keys_left_absent"]
        .as_object()
        .expect("defaults object")
        .clone();
    broken_defaults.remove("live.lineage_tracking");
    assert!(
        validate_key_partition(&[(CONTROL, &cu)], &broken_defaults).is_err(),
        "broken 31-key partition must fail"
    );
}

#[test]
fn preregistration_json_agrees_with_frozen_doc_and_draft_pointer() {
    let p = prereg();
    assert_eq!(p["status"].as_str(), Some("FROZEN"));
    assert_eq!(p["contains_results"].as_bool(), Some(false));
    assert_eq!(p["experimental_seeds_run"].as_u64(), Some(0));
    assert_eq!(p["smoke_seeds_run"].as_u64(), Some(0));
    assert_eq!(p["body_mass_consulted"].as_bool(), Some(false));
    assert_eq!(
        p["registered_at_parent_commit_full"].as_str(),
        Some(PINNED_TRUNK_FULL)
    );

    let th = &p["thresholds"];
    assert!((th["eps_primary"].as_f64().unwrap() - 5e-4).abs() < 1e-12);
    assert!((th["eps_sensitivity"].as_f64().unwrap() - 5e-5).abs() < 1e-12);
    assert!((th["sesoi_log_var_ratio"].as_f64().unwrap() - 1.5_f64.ln()).abs() < 1e-12);
    assert!((th["min_pi_ll"].as_f64().unwrap() - 0.85).abs() < 1e-12);
    assert_eq!(th["min_valid_pairs_for_decision"].as_u64(), Some(55));
    assert_eq!(th["min_non_zero_pairs_r7"].as_u64(), Some(7));
    assert!((th["kerby_rc_benchmarks"]["dz_0_50"].as_f64().unwrap() - 0.5205).abs() < 1e-6);
    assert!((th["kerby_rc_benchmarks"]["dz_0_80"].as_f64().unwrap() - 0.7421).abs() < 1e-6);

    let doc_path = repo_root().join(p["plan_document"].as_str().unwrap());
    let doc = std::fs::read_to_string(&doc_path)
        .unwrap_or_else(|e| panic!("read frozen doc {}: {e}", doc_path.display()));
    assert!(
        doc.contains("status: active"),
        "frozen doc must use status: active"
    );
    assert!(
        !doc.contains("status: frozen"),
        "status: frozen is rejected by metadata check"
    );
    assert!(
        doc.contains(PINNED_TRUNK_FULL),
        "frozen doc must record pinned trunk SHA"
    );
    assert!(
        doc.contains(&SMOKE_SEED.to_string()),
        "frozen doc must record smoke seed"
    );
    for k in 0..SEED_COUNT as u64 {
        let seed = SEED_BASE + SEED_STEP * k;
        assert!(
            doc.contains(&seed.to_string()),
            "frozen doc missing confirmation seed {seed}"
        );
    }

    let draft_path = repo_root().join(p["draft_document"].as_str().unwrap());
    let draft = std::fs::read_to_string(&draft_path)
        .unwrap_or_else(|e| panic!("read draft doc {}: {e}", draft_path.display()));
    assert!(
        draft.contains("2026-11-24-e3b-preregistration.md"),
        "draft must contain one-line pointer to frozen preregistration"
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum R10Label {
    Supports,
    Opposite,
    EquivalentToZero,
    InsufficientEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EffectBucket {
    AtLeastPositiveSesoi,
    AtMostNegativeSesoi,
    WithinEquivalentBand,
}

fn classify_r10_reference(
    wp2_valid: bool,
    strat_valid: bool,
    holm_sig: bool,
    eff: EffectBucket,
    tost_eq: bool,
    directional_pos: bool,
) -> (R10Label, &'static str) {
    if !wp2_valid {
        return (R10Label::InsufficientEvidence, "harness_invalid");
    }
    if !strat_valid {
        return (R10Label::InsufficientEvidence, "attrition_below_pi85_floor");
    }
    if eff != EffectBucket::WithinEquivalentBand && tost_eq {
        return (R10Label::InsufficientEvidence, "ci_inconsistent_impossible");
    }
    if holm_sig {
        match eff {
            EffectBucket::AtLeastPositiveSesoi => {
                return (R10Label::Supports, "predicted_positive_L")
            }
            EffectBucket::AtMostNegativeSesoi => {
                if directional_pos {
                    return (R10Label::Opposite, "opposite_negative_L");
                }
                return (R10Label::Supports, "non_zero_two_sided_L");
            }
            EffectBucket::WithinEquivalentBand => {
                if tost_eq {
                    return (R10Label::EquivalentToZero, "trivial_below_sesoi_equivalent");
                }
                return (
                    R10Label::InsufficientEvidence,
                    "inconclusive_straddles_sesoi",
                );
            }
        }
    }
    if tost_eq {
        return (
            R10Label::EquivalentToZero,
            "equivalent_to_zero_powered_null",
        );
    }
    (R10Label::InsufficientEvidence, "inconclusive_wide_ci")
}

#[test]
fn r10_exhaustive_48_partition_and_li_tost_lee_math_boundaries() {
    let empty_stats = mass_statistics(&[]);
    assert_eq!(empty_stats, [0.0; 4]);

    let eps = 5e-4_f64;
    let vu = 0.02_f64;
    let vs = 0.01_f64;
    let vt = 0.04_f64;
    let l_ts = ((vt + eps) / (vs + eps)).ln();
    let l_su = ((vs + eps) / (vu + eps)).ln();
    let l_tu = ((vt + eps) / (vu + eps)).ln();
    assert!(
        (l_tu - (l_ts + l_su)).abs() < 1e-12,
        "additive identity must hold"
    );

    // Exhaustive 48/48 check for directional H1 (supports=1, opposite=1, equiv=2, insuff=44).
    let mut h1_counts = BTreeMap::new();
    let mut h2_counts = BTreeMap::new();
    for wp2 in [true, false] {
        for strat in [true, false] {
            for holm in [true, false] {
                for eff in [
                    EffectBucket::AtLeastPositiveSesoi,
                    EffectBucket::AtMostNegativeSesoi,
                    EffectBucket::WithinEquivalentBand,
                ] {
                    for tost in [true, false] {
                        let (lbl1, _) = classify_r10_reference(wp2, strat, holm, eff, tost, true);
                        *h1_counts.entry(format!("{lbl1:?}")).or_insert(0usize) += 1;
                        let (lbl2, _) = classify_r10_reference(wp2, strat, holm, eff, tost, false);
                        *h2_counts.entry(format!("{lbl2:?}")).or_insert(0usize) += 1;
                    }
                }
            }
        }
    }
    assert_eq!(h1_counts.values().sum::<usize>(), 48);
    assert_eq!(h1_counts.get("Supports"), Some(&1));
    assert_eq!(h1_counts.get("Opposite"), Some(&1));
    assert_eq!(h1_counts.get("EquivalentToZero"), Some(&2));
    assert_eq!(h1_counts.get("InsufficientEvidence"), Some(&44));

    assert_eq!(h2_counts.values().sum::<usize>(), 48);
    assert_eq!(h2_counts.get("Supports"), Some(&2));
    assert_eq!(h2_counts.get("Opposite"), None);
    assert_eq!(h2_counts.get("EquivalentToZero"), Some(&2));
    assert_eq!(h2_counts.get("InsufficientEvidence"), Some(&44));
}
