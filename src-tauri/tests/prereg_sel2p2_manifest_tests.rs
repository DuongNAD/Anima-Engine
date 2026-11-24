//! Preregistration lock and preflight validation for `SEL2-P2`
//! (`docs/ai/planning/2026-11-24-sel2-p2-preregistration.md`).
//!
//! Static verification only: validates all 8 fixtures in `tests/fixtures/experiments_sel2p2/`,
//! checks the pinned freeze-time 31-key partition (`23` explicit + `8` default-absent) and
//! 28 live observables on trunk `f195a361`, enforces `QD-BIO-D2` absent-means-legacy defaults for
//! any post-freeze `LIVE_KEYS`, confirms 48 confirmation seeds (`900001..=947001`, capped at
//! `N <= 70`), 12 `QD-SEL2g` baseline seeds (`970501..=970512`), and smoke seed `999931` are
//! pairwise disjoint and disjoint from all prior and `VIAB-*` calibration series, enforces zero
//! `trait_mass` observables in smoke/baseline manifests, verifies `ensure_observables_supported`
//! rejects cause-split observables under `drift_control = 1.0`, and runs negative controls.
//! Never executes any simulation seed and never reads any empirical `trait_mass` output.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anima_engine_lib::core::experiment::{
    ExperimentError, ExperimentManifest, FactorDiff, InitialConditionSet, ObservableRegistry,
    ObserverPolicy, WorldLawSet, MANIFEST_SCHEMA_VERSION, MAX_ENSEMBLE_RESULT_BYTES,
};
use anima_engine_lib::core::live_experiment::{
    all_live_observable_ids, mass_statistics, LiveWorldConfig, LIVE_KEYS,
    LIVE_KEY_DEHYDRATION_TICKS, LIVE_KEY_DRIFT_CONTROL, LIVE_KEY_DRIFT_RESET_TICKS,
    LIVE_KEY_DRIVE_FLOOR, LIVE_KEY_ECOLOGICAL_FITNESS, LIVE_KEY_EVOLVED_BRAINS, LIVE_KEY_FOOD_CAP,
    LIVE_KEY_FOUNDERS, LIVE_KEY_FOUNDER_GENOTYPE, LIVE_KEY_FOUNDER_LAYOUT, LIVE_KEY_LAKES,
    LIVE_KEY_LAKE_LAYOUT, LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY, LIVE_KEY_MUTATION_RATE,
    LIVE_KEY_PLANT_RESERVE, LIVE_KEY_PREDATOR_FRACTION, LIVE_KEY_PREY_CAP, LIVE_KEY_REPRODUCTION,
    LIVE_KEY_RESOURCE_CAPACITY_SCALE, LIVE_KEY_STRICT_PREY_CAP, LIVE_KEY_THIRST_STEERING,
    LIVE_KEY_TREES,
};

const PREREG_DIR: &str = "tests/fixtures/experiments_sel2p2";
const PREREG_FILE: &str = "sel2p2-preregistration.json";

const SELECTED: &str = "sel2p2-selected.json";
const SCARCE_MUT0: &str = "sel2p2-scarce-mut0.json";
const SCARCE_DRIFT: &str = "sel2p2-scarce-drift.json";
const BASELINE_REF: &str = "sel2p2-baseline-ref.json";
const SMOKE_SELECTED: &str = "sel2p2-smoke-selected.json";
const SMOKE_MUT0: &str = "sel2p2-smoke-scarce-mut0.json";
const SMOKE_DRIFT: &str = "sel2p2-smoke-scarce-drift.json";

const ALL_MANIFESTS: [&str; 7] = [
    SELECTED,
    SCARCE_MUT0,
    SCARCE_DRIFT,
    BASELINE_REF,
    SMOKE_SELECTED,
    SMOKE_MUT0,
    SMOKE_DRIFT,
];

const SEED_BASE: u64 = 900_001;
const SEED_STEP: u64 = 1_000;
const SEED_COUNT: usize = 48;
const MAX_N_CAP: usize = 70;
const BASELINE_SEED_START: u64 = 970_501;
const BASELINE_SEED_COUNT: usize = 12;
const SMOKE_SEED: u64 = 999_931;
const EXPECTED_DURATION: u64 = 200_000;
const EXPECTED_CONFIRM_PERIOD: u64 = 500;
const EXPECTED_SMOKE_PERIOD: u64 = 50;
const PINNED_TRUNK_FULL: &str = "f195a361ec50473932520ad30306bd366d719a6d";

const REQUIRED_COMMON_ALL_ARMS: [(&str, f64); 18] = [
    (LIVE_KEY_ECOLOGICAL_FITNESS, 1.0),
    (LIVE_KEY_MORTALITY, 1.0),
    (LIVE_KEY_REPRODUCTION, 1.0),
    (LIVE_KEY_LOCOMOTION, 1.0),
    (LIVE_KEY_DRIVE_FLOOR, 0.5),
    (LIVE_KEY_THIRST_STEERING, 1.0),
    (LIVE_KEY_EVOLVED_BRAINS, 0.0),
    (LIVE_KEY_FOUNDERS, 100.0),
    (LIVE_KEY_PREDATOR_FRACTION, 0.0),
    (LIVE_KEY_TREES, 8.0),
    (LIVE_KEY_LAKES, 16.0),
    (LIVE_KEY_LAKE_LAYOUT, 1.0),
    (LIVE_KEY_PLANT_RESERVE, 0.0),
    (LIVE_KEY_FOOD_CAP, 50.0),
    (LIVE_KEY_DEHYDRATION_TICKS, 960.0),
    (LIVE_KEY_STRICT_PREY_CAP, 1.0),
    (LIVE_KEY_FOUNDER_GENOTYPE, 1.0),
    (LIVE_KEY_FOUNDER_LAYOUT, 0.0),
];

/// Exact 31 `LIVE_KEYS` present at `SEL2-P2` freeze (`f195a361`). Future opt-in keys may extend
/// `LIVE_KEYS` without breaking `SEL2-P2`'s frozen manifest partition provided every post-freeze
/// key preserves absent-means-legacy semantics (`0.0 == LiveWorldConfig::default()`, `QD-BIO-D2`).
const SEL2P2_FREEZE_LIVE_KEYS: [&str; 31] = [
    "live.founders",
    "live.founder_layout",
    "live.founder_genotype",
    "live.predator_fraction",
    "live.trees",
    "live.lakes",
    "live.lake_layout",
    "live.food_cap",
    "live.evolved_brains",
    "live.mortality",
    "live.reproduction",
    "live.locomotion",
    "live.drive_floor",
    "live.thirst_steering",
    "live.grass_steering",
    "live.mutation_rate",
    "live.starve_ticks",
    "live.dehydration_ticks",
    "live.ecological_fitness",
    "live.w_surplus",
    "live.kill_hydration",
    "live.kill_efficiency",
    "live.target_min_energy",
    "live.prey_cap",
    "live.resource_capacity_scale",
    "live.plant_reserve",
    "live.drift_control",
    "live.drift_reset_ticks",
    "live.land_mask",
    "live.strict_prey_cap",
    "live.lineage_tracking",
];

/// Exact 28 live observables (in emission order) present at `SEL2-P2` freeze (`f195a361`).
/// Additive post-freeze observables may be appended after these 28 without breaking frozen manifests.
const SEL2P2_FREEZE_OBSERVABLE_IDS: [&str; 28] = [
    "plants",
    "detritus",
    "live.animals_eu",
    "live.dormant_eu",
    "live.closed_eu_total",
    "live.agent_count",
    "live.herbivore_count",
    "live.predator_count",
    "live.food_items",
    "live.standing_crop",
    "live.mean_agent_energy",
    "live.season_phase",
    "live.trait_mass_mean",
    "live.trait_mass_var",
    "live.trait_mass_p10",
    "live.trait_mass_p90",
    "live.shannon",
    "live.simpson",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
    "live.archive_coverage",
    "live.deaths_starvation",
    "live.deaths_dehydration",
    "live.mean_energy_integral",
    "live.trait_mass_mean_west",
    "live.trait_mass_mean_east",
    "live.cumulative_mean_lifespan",
];

const BLIND_NON_DRIFT_OBSERVABLES: [&str; 9] = [
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

const BLIND_DRIFT_OBSERVABLES: [&str; 7] = [
    "live.closed_eu_total",
    "live.agent_count",
    "live.standing_crop",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
    "live.cumulative_mean_lifespan",
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

fn expected_manifest_params(name: &str) -> (f64, f64, f64, f64, f64, u64) {
    match name {
        SELECTED => (320.0, 0.75, 0.05, 0.0, 0.0, EXPECTED_CONFIRM_PERIOD),
        SCARCE_MUT0 => (320.0, 0.75, 0.0, 0.0, 0.0, EXPECTED_CONFIRM_PERIOD),
        SCARCE_DRIFT => (320.0, 0.75, 0.05, 1.0, 2000.0, EXPECTED_CONFIRM_PERIOD),
        BASELINE_REF => (80.0, 1.0, 0.05, 0.0, 0.0, EXPECTED_SMOKE_PERIOD),
        SMOKE_SELECTED => (320.0, 0.75, 0.05, 0.0, 0.0, EXPECTED_SMOKE_PERIOD),
        SMOKE_MUT0 => (320.0, 0.75, 0.0, 0.0, 0.0, EXPECTED_SMOKE_PERIOD),
        SMOKE_DRIFT => (320.0, 0.75, 0.05, 1.0, 2000.0, EXPECTED_SMOKE_PERIOD),
        other => panic!("unexpected SEL2-P2 manifest name '{other}'"),
    }
}

fn validate_sel2p2_required_keys(
    m: &ExperimentManifest,
    name: &str,
    expected_prey_cap: f64,
    expected_scale: f64,
    expected_mut: f64,
    expected_drift: f64,
    expected_reset: f64,
) -> Result<(), String> {
    let map = ic_map(m);
    for (key, expected) in REQUIRED_COMMON_ALL_ARMS {
        let Some(actual) = map.get(key) else {
            return Err(format!("{name}: missing required SEL2-P2 key '{key}'"));
        };
        if (*actual - expected).abs() > 1e-12 {
            return Err(format!(
                "{name}: key '{key}' expected {expected}, found {actual}"
            ));
        }
    }
    for (key, expected) in [
        (LIVE_KEY_PREY_CAP, expected_prey_cap),
        (LIVE_KEY_RESOURCE_CAPACITY_SCALE, expected_scale),
        (LIVE_KEY_MUTATION_RATE, expected_mut),
        (LIVE_KEY_DRIFT_CONTROL, expected_drift),
        (LIVE_KEY_DRIFT_RESET_TICKS, expected_reset),
    ] {
        let Some(actual) = map.get(key) else {
            return Err(format!("{name}: missing per-arm SEL2-P2 key '{key}'"));
        };
        if (*actual - expected).abs() > 1e-12 {
            return Err(format!(
                "{name}: key '{key}' expected {expected}, found {actual}"
            ));
        }
    }
    if map.len() != 23 {
        return Err(format!(
            "{name}: expected 23 explicit keys, found {}",
            map.len()
        ));
    }
    Ok(())
}

fn validate_blind_manifest(
    m: &ExperimentManifest,
    name: &str,
    is_drift: bool,
) -> Result<(), String> {
    for obs in &m.observable_ids {
        if obs.to_ascii_lowercase().contains("trait_mass") {
            return Err(format!(
                "{name}: blind manifest contains forbidden trait_mass observable '{obs}'"
            ));
        }
    }
    if is_drift {
        let expected: Vec<String> = BLIND_DRIFT_OBSERVABLES
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        if m.observable_ids != expected {
            return Err(format!(
                "{name}: drift blind observables drifted from locked 7-item list"
            ));
        }
    } else {
        let expected: Vec<String> = BLIND_NON_DRIFT_OBSERVABLES
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        if m.observable_ids != expected {
            return Err(format!(
                "{name}: non-drift blind observables drifted from locked 9-item list"
            ));
        }
    }
    Ok(())
}

fn validate_freeze_observables(freeze_obs: &[&str], emitted: &[&str]) -> Result<(), String> {
    let unique: BTreeSet<&str> = freeze_obs.iter().copied().collect();
    if unique.len() != freeze_obs.len() {
        return Err("freeze observable list contains duplicates".to_string());
    }
    if emitted.len() < freeze_obs.len() {
        return Err(format!(
            "emitted observable count {} is less than pinned {} SEL2-P2 freeze observables",
            emitted.len(),
            freeze_obs.len()
        ));
    }
    if &emitted[..freeze_obs.len()] != freeze_obs {
        return Err(
            "first 28 emitted live observables drifted from the SEL2-P2 freeze order".to_string(),
        );
    }
    Ok(())
}

fn validate_key_partition(
    manifests: &[(&str, &ExperimentManifest)],
    defaults_obj: &serde_json::Map<String, serde_json::Value>,
    freeze_keys: &[&str],
    current_live_keys: &[&str],
) -> Result<(), String> {
    let freeze_set: BTreeSet<String> = freeze_keys.iter().map(|s| (*s).to_string()).collect();
    if freeze_set.len() != freeze_keys.len() {
        return Err("freeze key list contains duplicates".to_string());
    }
    let current_set: BTreeSet<String> =
        current_live_keys.iter().map(|s| (*s).to_string()).collect();
    if current_set.len() != current_live_keys.len() {
        return Err("current LIVE_KEYS contains duplicates".to_string());
    }
    if !freeze_set.is_subset(&current_set) {
        let removed: Vec<String> = freeze_set.difference(&current_set).cloned().collect();
        return Err(format!(
            "current LIVE_KEYS dropped SEL2-P2 freeze-time keys: {removed:?}"
        ));
    }

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
        if combined != freeze_set {
            let missing: Vec<String> = freeze_set.difference(&combined).cloned().collect();
            let extra: Vec<String> = combined.difference(&freeze_set).cloned().collect();
            return Err(format!(
                "{name}: explicit U default_keys_left_absent != freeze keys \
                 (missing={missing:?}, extra={extra:?})"
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

fn validate_post_freeze_keys_default_to_legacy(
    freeze_keys: &[&str],
    current_live_keys: &[&str],
    sel2p2_base_ic: &InitialConditionSet,
) -> Result<(), String> {
    let freeze_set: BTreeSet<&str> = freeze_keys.iter().copied().collect();
    let engine_default = LiveWorldConfig::default();
    let base_cfg = LiveWorldConfig::from_initial_conditions(sel2p2_base_ic)
        .map_err(|e| format!("failed to parse SEL2-P2 baseline initial conditions: {e}"))?;

    for &key in current_live_keys {
        if freeze_set.contains(key) {
            continue;
        }
        let parsed = LiveWorldConfig::from_initial_conditions(&InitialConditionSet::new(vec![(
            key.to_string(),
            0.0,
        )]))
        .map_err(|e| {
            format!("post-freeze key '{key}' does not accept absent-means-legacy value 0.0: {e}")
        })?;
        if parsed != engine_default {
            return Err(format!(
                "post-freeze key '{key}' = 0.0 differs from LiveWorldConfig::default() \
                 (missing absent-means-legacy default)"
            ));
        }

        let mut with_key = sel2p2_base_ic.clone();
        with_key.values.push((key.to_string(), 0.0));
        let parsed_with_key = LiveWorldConfig::from_initial_conditions(&with_key).map_err(|e| {
            format!("post-freeze key '{key}' = 0.0 rejected alongside SEL2-P2 baseline: {e}")
        })?;
        if parsed_with_key != base_cfg {
            return Err(format!(
                "post-freeze key '{key}' = 0.0 perturbs SEL2-P2 baseline LiveWorldConfig"
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
                "{name}: seeds mismatch expected formula 900001 + 1000*k (len {})",
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

fn forbidden_seed_set() -> BTreeSet<u64> {
    let mut forbidden = BTreeSet::new();
    // VIAB-1 / VIAB-1b / PR #216 / VIAB-2 calibration probes.
    forbidden.extend(880_001..=880_012);
    // Exploratory toys + E3b runner unit suite + prior smoke seeds + prior experimental blocks.
    forbidden.extend(1..=6);
    forbidden.extend([613_001, 613_002, 613_999]);
    forbidden.extend(700_001..=799_506);
    forbidden.extend((0..70).map(|k| 810_001 + 1_000 * k));
    forbidden.extend([999_983, 999_979, 999_971, 999_961, 999_953, SMOKE_SEED]);
    forbidden.extend(999_801..=999_822);
    forbidden.extend(999_701..=999_722);

    for rel in [
        "tests/fixtures/experiments_e2/e2-control-shared-brain.json",
        "tests/fixtures/experiments_e2/e2-smoke-control-shared-brain.json",
        "tests/fixtures/experiments_e3/e3-control-uniform.json",
        "tests/fixtures/experiments_e3/e3-smoke-control-uniform.json",
        "tests/fixtures/experiments_e3b/e3b-control-uniform.json",
        "tests/fixtures/experiments_e3b/e3b-smoke-control-uniform.json",
        "tests/fixtures/experiments_esel1/esel1-control-abundant.json",
        "tests/fixtures/experiments_esel1/esel1-smoke-abundant.json",
        "tests/fixtures/experiments_sel2p/sel2p-selected.json",
        "tests/fixtures/experiments_sel2p/sel2p-baseline-ref.json",
        "tests/fixtures/experiments_sel2p/sel2p-smoke-selected.json",
    ] {
        forbidden.extend(collect_seeds_from_manifest(rel));
    }
    forbidden
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

#[test]
fn every_sel2p2_manifest_parses_validates_passes_preflight_and_fits_memory_ceiling() {
    let reg = ObservableRegistry::live_default();
    reg.validate().expect("live registry must validate");

    let mut fingerprints = BTreeSet::new();
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

        let (prey_cap, scale, mut_rate, drift_ctrl, drift_reset, period) =
            expected_manifest_params(name);
        validate_sel2p2_required_keys(&m, name, prey_cap, scale, mut_rate, drift_ctrl, drift_reset)
            .unwrap_or_else(|e| panic!("required key validation failed: {e}"));

        let cfg =
            LiveWorldConfig::from_initial_conditions(&m.initial_conditions).unwrap_or_else(|e| {
                panic!("{name} failed LiveWorldConfig::from_initial_conditions: {e}")
            });
        cfg.ensure_observables_supported(&m.observable_ids)
            .unwrap_or_else(|e| panic!("{name} failed ensure_observables_supported: {e}"));

        assert_eq!(m.duration_ticks, EXPECTED_DURATION, "{name}");
        assert_eq!(m.sample_period, period, "{name}");
        assert_eq!(m.observer, ObserverPolicy::Absent, "{name}");
        assert_eq!(m.laws, WorldLawSet::baseline(), "{name}");
        assert!(m.interventions.is_empty(), "{name}");
        assert!(m.exotic_interventions.is_empty(), "{name}");

        let bytes = m.estimated_result_bytes(&reg);
        assert!(
            bytes < MAX_ENSEMBLE_RESULT_BYTES,
            "{name}: estimated {bytes} bytes exceeds {MAX_ENSEMBLE_RESULT_BYTES}"
        );

        let fp1 = m.fingerprint();
        let fp2 = load_manifest(name).fingerprint();
        assert_eq!(fp1, fp2, "{name}: fingerprint must be deterministic");
        assert!(
            fingerprints.insert(fp1),
            "{name}: fingerprint collided with another SEL2-P2 manifest"
        );
    }
}

#[test]
fn explicit_and_default_keys_partition_all_31_live_keys_and_isolate_arms() {
    let p = prereg();
    let defaults_obj = p["default_keys_left_absent"]
        .as_object()
        .expect("default_keys_left_absent must be an object");
    assert_eq!(SEL2P2_FREEZE_LIVE_KEYS.len(), 31);
    assert!(
        LIVE_KEYS.len() >= SEL2P2_FREEZE_LIVE_KEYS.len(),
        "LIVE_KEYS must contain at least the 31 SEL2-P2 freeze-time keys"
    );
    assert_eq!(
        defaults_obj.len(),
        8,
        "SEL2-P2 declares 23 explicit keys + 8 default-absent keys = 31 freeze-time keys"
    );

    let loaded: Vec<(&str, ExperimentManifest)> = ALL_MANIFESTS
        .iter()
        .map(|&n| (n, load_manifest(n)))
        .collect();
    let refs: Vec<(&str, &ExperimentManifest)> = loaded.iter().map(|(n, m)| (*n, m)).collect();
    validate_key_partition(&refs, defaults_obj, &SEL2P2_FREEZE_LIVE_KEYS, &LIVE_KEYS)
        .unwrap_or_else(|e| panic!("key partition check failed: {e}"));
    validate_post_freeze_keys_default_to_legacy(
        &SEL2P2_FREEZE_LIVE_KEYS,
        &LIVE_KEYS,
        &loaded[0].1.initial_conditions,
    )
    .unwrap_or_else(|e| panic!("post-freeze legacy-default check failed: {e}"));

    let sel = load_manifest(SELECTED);
    let mut0 = load_manifest(SCARCE_MUT0);
    let drift = load_manifest(SCARCE_DRIFT);
    let base = load_manifest(BASELINE_REF);
    let s_sel = load_manifest(SMOKE_SELECTED);
    let s_mut0 = load_manifest(SMOKE_MUT0);
    let s_drift = load_manifest(SMOKE_DRIFT);

    let allow_ic = FactorDiff {
        allowed_paths: vec!["initial_conditions".to_string()],
    };
    assert_eq!(
        allow_ic.validate(&sel, &mut0).unwrap(),
        vec!["initial_conditions".to_string()]
    );
    assert_eq!(
        diff_ic_keys(&sel, &mut0),
        vec![LIVE_KEY_MUTATION_RATE.to_string()]
    );

    assert_eq!(
        allow_ic.validate(&sel, &drift).unwrap(),
        vec!["initial_conditions".to_string()]
    );
    assert_eq!(
        diff_ic_keys(&sel, &drift),
        vec![
            LIVE_KEY_DRIFT_CONTROL.to_string(),
            LIVE_KEY_DRIFT_RESET_TICKS.to_string(),
        ]
    );

    assert_eq!(
        allow_ic.validate(&drift, &mut0).unwrap(),
        vec!["initial_conditions".to_string()]
    );
    assert_eq!(
        diff_ic_keys(&drift, &mut0),
        vec![
            LIVE_KEY_DRIFT_CONTROL.to_string(),
            LIVE_KEY_DRIFT_RESET_TICKS.to_string(),
            LIVE_KEY_MUTATION_RATE.to_string(),
        ]
    );

    let allow_ic_seeds = FactorDiff {
        allowed_paths: vec!["initial_conditions".to_string(), "seeds".to_string()],
    };
    let mut base_diffs = allow_ic_seeds.validate(&base, &s_sel).unwrap();
    base_diffs.sort();
    assert_eq!(
        base_diffs,
        vec!["initial_conditions".to_string(), "seeds".to_string()]
    );
    assert_eq!(
        diff_ic_keys(&base, &s_sel),
        vec![
            LIVE_KEY_PREY_CAP.to_string(),
            LIVE_KEY_RESOURCE_CAPACITY_SCALE.to_string(),
        ]
    );

    assert_eq!(
        allow_ic.validate(&s_sel, &s_mut0).unwrap(),
        vec!["initial_conditions".to_string()]
    );
    assert_eq!(
        diff_ic_keys(&s_sel, &s_mut0),
        vec![LIVE_KEY_MUTATION_RATE.to_string()]
    );

    let allow_ic_obs = FactorDiff {
        allowed_paths: vec!["initial_conditions".to_string(), "observables".to_string()],
    };
    let mut s_drift_diffs = allow_ic_obs.validate(&s_sel, &s_drift).unwrap();
    s_drift_diffs.sort();
    assert_eq!(
        s_drift_diffs,
        vec!["initial_conditions".to_string(), "observables".to_string()]
    );
    assert_eq!(
        diff_ic_keys(&s_sel, &s_drift),
        vec![
            LIVE_KEY_DRIFT_CONTROL.to_string(),
            LIVE_KEY_DRIFT_RESET_TICKS.to_string(),
        ]
    );
}

#[test]
fn forty_eight_seeds_baseline_smoke_and_pipeline_are_pairwise_and_repo_disjoint() {
    let p = prereg();
    let sel = load_manifest(SELECTED);
    let mut0 = load_manifest(SCARCE_MUT0);
    let drift = load_manifest(SCARCE_DRIFT);
    let base = load_manifest(BASELINE_REF);
    let s_sel = load_manifest(SMOKE_SELECTED);
    let s_mut0 = load_manifest(SMOKE_MUT0);
    let s_drift = load_manifest(SMOKE_DRIFT);

    let forbidden = forbidden_seed_set();
    validate_seed_schedule(
        &[
            (SELECTED, &sel),
            (SCARCE_MUT0, &mut0),
            (SCARCE_DRIFT, &drift),
        ],
        &forbidden,
    )
    .unwrap_or_else(|e| panic!("seed schedule invalid: {e}"));

    assert_eq!(sel.seeds.len(), 48);
    assert_eq!(sel.seeds.first(), Some(&900_001));
    assert_eq!(sel.seeds.last(), Some(&947_001));
    let expected_cap_max = SEED_BASE + SEED_STEP * (MAX_N_CAP as u64 - 1);
    assert_eq!(expected_cap_max, 969_001);
    assert!(expected_cap_max < BASELINE_SEED_START);

    let expected_baseline: Vec<u64> =
        (BASELINE_SEED_START..BASELINE_SEED_START + BASELINE_SEED_COUNT as u64).collect();
    assert_eq!(base.seeds, expected_baseline);
    for (name, sm) in [
        (SMOKE_SELECTED, &s_sel),
        (SMOKE_MUT0, &s_mut0),
        (SMOKE_DRIFT, &s_drift),
    ] {
        assert_eq!(sm.seeds, vec![SMOKE_SEED], "{name}");
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
    let unit_seeds: Vec<u64> = p["seeds"]["unit_test_seeds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert_eq!(pc_base.len(), 6);
    assert_eq!(pc_pair.len(), 2);
    assert_eq!(unit_seeds, vec![614_001, 614_002, 614_999]);

    let cap70: Vec<u64> = (0..MAX_N_CAP as u64)
        .map(|k| SEED_BASE + SEED_STEP * k)
        .collect();
    let mut union = BTreeSet::new();
    for (label, list) in [
        ("confirm_cap70", cap70.as_slice()),
        ("baseline_ref", expected_baseline.as_slice()),
        ("smoke", &[SMOKE_SEED]),
        ("pipeline_base", pc_base.as_slice()),
        ("pipeline_smoke", &[pc_smoke]),
        ("pipeline_pair", pc_pair.as_slice()),
        ("unit_tests", unit_seeds.as_slice()),
    ] {
        for &s in list {
            assert!(
                union.insert(s),
                "seed {s} duplicated inside SEL2-P2 ({label})"
            );
        }
    }

    let mut prior_without_sel2p2_smoke = forbidden_seed_set();
    prior_without_sel2p2_smoke.remove(&SMOKE_SEED);
    for (label, list) in [
        ("baseline_ref", expected_baseline.as_slice()),
        ("unit_tests", unit_seeds.as_slice()),
    ] {
        for &s in list {
            assert!(
                !prior_without_sel2p2_smoke.contains(&s),
                "SEL2-P2 {label} seed {s} overlaps prior repo seed sets"
            );
        }
    }
}

#[test]
fn drift_arm_omits_cause_split_and_smoke_and_baseline_are_blind_to_body_mass() {
    let reg = ObservableRegistry::live_default();
    let all_emitted = all_live_observable_ids();
    assert_eq!(SEL2P2_FREEZE_OBSERVABLE_IDS.len(), 28);
    validate_freeze_observables(&SEL2P2_FREEZE_OBSERVABLE_IDS, &all_emitted)
        .unwrap_or_else(|e| panic!("freeze observable pin failed: {e}"));

    let expected_confirm: Vec<String> = SEL2P2_FREEZE_OBSERVABLE_IDS
        .iter()
        .copied()
        .filter(|&id| id != "live.deaths_starvation" && id != "live.deaths_dehydration")
        .map(str::to_string)
        .collect();
    assert_eq!(expected_confirm.len(), 26);
    assert!(expected_confirm.contains(&"live.trait_mass_mean".to_string()));
    assert!(expected_confirm.contains(&"live.cumulative_mean_lifespan".to_string()));

    for name in [SELECTED, SCARCE_MUT0, SCARCE_DRIFT] {
        let m = load_manifest(name);
        assert_eq!(m.observable_ids, expected_confirm, "{name}");
        for id in &m.observable_ids {
            assert!(reg.get(id).is_some(), "{name} missing '{id}' in registry");
        }
    }

    // Engine preflight must reject cause-split observables when drift_control = 1.0.
    for drift_name in [SCARCE_DRIFT, SMOKE_DRIFT] {
        let mut bad = load_manifest(drift_name);
        let cfg = LiveWorldConfig::from_initial_conditions(&bad.initial_conditions)
            .expect("drift config");
        for cause_id in ["live.deaths_starvation", "live.deaths_dehydration"] {
            bad.observable_ids.push(cause_id.to_string());
            let err = cfg
                .ensure_observables_supported(&bad.observable_ids)
                .expect_err("drift config must reject cause-split observable");
            assert!(
                matches!(err, ExperimentError::InvalidObservableRequest { .. }),
                "{drift_name} with {cause_id}: {err:?}"
            );
            bad.observable_ids.pop();
        }
    }

    for (name, is_drift) in [
        (BASELINE_REF, false),
        (SMOKE_SELECTED, false),
        (SMOKE_MUT0, false),
        (SMOKE_DRIFT, true),
    ] {
        let raw = std::fs::read_to_string(fixture_path(name)).expect("read raw blind fixture");
        assert!(
            !raw.to_ascii_lowercase().contains("trait_mass"),
            "{name} raw JSON text must contain zero 'trait_mass' substrings"
        );
        let m = load_manifest(name);
        validate_blind_manifest(&m, name, is_drift)
            .unwrap_or_else(|e| panic!("blindness check failed: {e}"));
    }

    // Pre-run smoke & baseline artifacts (excluding confirmation/ or viab2/) must carry zero trait_mass.
    let art_root = repo_root().join("artifacts/experiments/sel2-p2");
    for sub in ["smoke", "viab2"] {
        let dir = art_root.join(sub);
        if !dir.exists() {
            continue;
        }
        let mut files = Vec::new();
        collect_files_recursive(&dir, &mut files);
        for f in files {
            let content = std::fs::read_to_string(&f)
                .unwrap_or_else(|e| panic!("read blind artifact {}: {e}", f.display()));
            assert!(
                !content.to_ascii_lowercase().contains("trait_mass"),
                "blind artifact {} contains forbidden 'trait_mass' substring",
                f.display()
            );
        }
    }
}

#[test]
fn missing_required_key_fails_preflight() {
    let base = load_manifest(SELECTED);

    let mut no_mortality = base.clone();
    no_mortality
        .initial_conditions
        .values
        .retain(|(k, _)| k != LIVE_KEY_MORTALITY);
    let cfg_no_mort = LiveWorldConfig::from_initial_conditions(&no_mortality.initial_conditions)
        .expect("LiveWorldConfig parses without mortality");
    assert!(matches!(
        cfg_no_mort.ensure_observables_supported(&no_mortality.observable_ids),
        Err(ExperimentError::InvalidObservableRequest { .. })
    ));

    let mut no_repro = base.clone();
    no_repro
        .initial_conditions
        .values
        .retain(|(k, _)| k != LIVE_KEY_REPRODUCTION);
    let cfg_no_repro = LiveWorldConfig::from_initial_conditions(&no_repro.initial_conditions)
        .expect("LiveWorldConfig parses without reproduction");
    assert!(matches!(
        cfg_no_repro.ensure_observables_supported(&no_repro.observable_ids),
        Err(ExperimentError::InvalidObservableRequest { .. })
    ));

    for (req_key, _) in REQUIRED_COMMON_ALL_ARMS {
        let mut dropped = base.clone();
        dropped
            .initial_conditions
            .values
            .retain(|(k, _)| k != req_key);
        let err = validate_sel2p2_required_keys(&dropped, "mutated", 320.0, 0.75, 0.05, 0.0, 0.0)
            .expect_err("dropping required key must fail SEL2-P2 preflight");
        assert!(
            err.contains(req_key),
            "error should name '{req_key}', got: {err}"
        );
    }
}

#[test]
fn negative_controls_fail_when_manifest_or_rule_is_wrong() {
    let sel = load_manifest(SELECTED);
    let mut0 = load_manifest(SCARCE_MUT0);
    let s_sel = load_manifest(SMOKE_SELECTED);
    let forbidden = forbidden_seed_set();

    // Control 1: Undeclared factor change between selected and scarce_mut0 must fail FactorDiff.
    let mut bad_mut0 = mut0.clone();
    for (k, v) in &mut bad_mut0.initial_conditions.values {
        if k == LIVE_KEY_FOUNDERS {
            *v = 99.0;
        }
    }
    let allow_ic = FactorDiff {
        allowed_paths: vec!["initial_conditions".to_string()],
    };
    assert!(allow_ic.validate(&sel, &bad_mut0).is_ok());
    assert_ne!(
        diff_ic_keys(&sel, &bad_mut0),
        vec![LIVE_KEY_MUTATION_RATE.to_string()],
        "undeclared factor diff must change IC diff key set"
    );
    let mut bad_period = mut0.clone();
    bad_period.sample_period = 250;
    assert!(
        allow_ic.validate(&sel, &bad_period).is_err(),
        "undeclared sample_period diff must fail FactorDiff"
    );

    // Control 2: Leaking trait_mass into smoke manifest fails blindness.
    let mut bad_smoke = s_sel.clone();
    bad_smoke
        .observable_ids
        .push("live.trait_mass_mean".to_string());
    assert!(
        validate_blind_manifest(&bad_smoke, "neg-smoke", false).is_err(),
        "trait_mass leak into smoke must fail blindness"
    );

    // Control 3: Calibration 880001 or smoke seed 999931 in confirmation manifest fails seed check.
    let mut bad_cal = sel.clone();
    bad_cal.seeds[0] = 880_001;
    assert!(
        validate_seed_schedule(&[("neg-cal", &bad_cal)], &forbidden).is_err(),
        "calibration seed 880001 in confirmation must fail"
    );
    let mut bad_smoke_seed = sel.clone();
    bad_smoke_seed.seeds[47] = SMOKE_SEED;
    assert!(
        validate_seed_schedule(&[("neg-smoke", &bad_smoke_seed)], &forbidden).is_err(),
        "smoke seed 999931 in confirmation must fail"
    );

    // Control 4: Unknown live key fails LiveWorldConfig preflight.
    let mut bad_unknown = sel.clone();
    bad_unknown
        .initial_conditions
        .values
        .push(("live.undeclared_knob".to_string(), 1.0));
    assert!(LiveWorldConfig::from_initial_conditions(&bad_unknown.initial_conditions).is_err());

    // Control 5-8 (QD-BIO-D2): Missing absent key, removed freeze key, non-zero legacy default, obs drift.
    let p = prereg();
    let defaults_obj = p["default_keys_left_absent"].as_object().unwrap();
    let mut broken_defaults = defaults_obj.clone();
    broken_defaults.remove("live.lineage_tracking");
    assert!(validate_key_partition(
        &[(SELECTED, &sel)],
        &broken_defaults,
        &SEL2P2_FREEZE_LIVE_KEYS,
        &LIVE_KEYS,
    )
    .is_err());

    let without_founders: Vec<&str> = SEL2P2_FREEZE_LIVE_KEYS
        .iter()
        .copied()
        .filter(|&k| k != LIVE_KEY_FOUNDERS)
        .collect();
    assert!(validate_key_partition(
        &[(SELECTED, &sel)],
        defaults_obj,
        &without_founders,
        &LIVE_KEYS,
    )
    .is_err());

    let without_w_surplus: Vec<&str> = SEL2P2_FREEZE_LIVE_KEYS
        .iter()
        .copied()
        .filter(|&k| k != "live.w_surplus")
        .collect();
    let err_ws = validate_post_freeze_keys_default_to_legacy(
        &without_w_surplus,
        &LIVE_KEYS,
        &sel.initial_conditions,
    )
    .expect_err("live.w_surplus default != 0.0 must fail post-freeze check");
    assert!(err_ws.contains("live.w_surplus"));

    let mut extended_unknown = LIVE_KEYS.to_vec();
    extended_unknown.push("live.post_freeze_no_default");
    assert!(validate_post_freeze_keys_default_to_legacy(
        &SEL2P2_FREEZE_LIVE_KEYS,
        &extended_unknown,
        &sel.initial_conditions,
    )
    .is_err());

    let without_lineage: Vec<&str> = SEL2P2_FREEZE_LIVE_KEYS
        .iter()
        .copied()
        .filter(|&k| k != "live.lineage_tracking")
        .collect();
    validate_post_freeze_keys_default_to_legacy(
        &without_lineage,
        &LIVE_KEYS,
        &sel.initial_conditions,
    )
    .expect("0.0 == default() opt-in key must pass post-freeze check");

    let mut extended_obs = SEL2P2_FREEZE_OBSERVABLE_IDS.to_vec();
    extended_obs.push("live.future_observable");
    validate_freeze_observables(&SEL2P2_FREEZE_OBSERVABLE_IDS, &extended_obs)
        .expect("appending post-freeze observable keeps pin green");
    assert!(validate_freeze_observables(
        &SEL2P2_FREEZE_OBSERVABLE_IDS,
        &SEL2P2_FREEZE_OBSERVABLE_IDS[..27]
    )
    .is_err());
    let mut swapped_obs = SEL2P2_FREEZE_OBSERVABLE_IDS;
    swapped_obs.swap(0, 1);
    assert!(validate_freeze_observables(&SEL2P2_FREEZE_OBSERVABLE_IDS, &swapped_obs).is_err());
}

#[test]
fn preregistration_json_agrees_with_frozen_doc_and_draft_pointer() {
    let p = prereg();
    assert_eq!(p["status"].as_str(), Some("FROZEN"));
    assert_eq!(p["contains_results"].as_bool(), Some(false));
    assert_eq!(p["experimental_seeds_run"].as_u64(), Some(0));
    assert_eq!(p["smoke_seeds_run"].as_u64(), Some(0));
    assert_eq!(p["body_mass_consulted"].as_bool(), Some(false));
    assert_eq!(p["h1_direction"].as_str(), Some("two_sided"));
    assert_eq!(
        p["registered_at_parent_commit_full"].as_str(),
        Some(PINNED_TRUNK_FULL)
    );

    assert_eq!(p["seeds"]["n_requested"].as_u64(), Some(48));
    assert_eq!(p["seeds"]["max_n_cap"].as_u64(), Some(70));
    assert_eq!(p["seeds"]["min_surviving_baseline_seeds"].as_u64(), Some(6));
    assert_eq!(p["seeds"]["smoke_seed"].as_u64(), Some(SMOKE_SEED));

    let th = &p["thresholds"];
    assert!((th["sesoi_log_mean_ratio"].as_f64().unwrap() - 1.10_f64.ln()).abs() < 1e-12);
    assert!(
        (th["max_arm_extinction_rate_for_complete_pairs"]
            .as_f64()
            .unwrap()
            - 0.05)
            .abs()
            < 1e-12
    );
    assert!((th["min_pi_ll"].as_f64().unwrap() - 0.85).abs() < 1e-12);
    assert_eq!(th["min_valid_pairs_for_decision"].as_u64(), Some(41));
    assert_eq!(th["min_non_zero_pairs_r7"].as_u64(), Some(7));
    assert_eq!(
        th["r10_labels"],
        serde_json::json!(["supports", "equivalent_to_zero", "insufficient_evidence"])
    );

    for (k, v) in p["commands"].as_object().unwrap() {
        let cmd = v.as_str().unwrap();
        assert!(
            !cmd.contains("cargo run"),
            "command {k} must not use cargo run"
        );
    }

    let doc_path = repo_root().join(p["plan_document"].as_str().unwrap());
    let doc = std::fs::read_to_string(&doc_path)
        .unwrap_or_else(|e| panic!("read frozen doc {}: {e}", doc_path.display()));
    assert!(
        doc.contains("status: active"),
        "frozen doc must use status: active"
    );
    assert!(
        !doc.contains("status: frozen"),
        "status: frozen is disallowed by metadata gate"
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
    for s in BASELINE_SEED_START..BASELINE_SEED_START + BASELINE_SEED_COUNT as u64 {
        assert!(
            doc.contains(&s.to_string()),
            "frozen doc missing baseline seed {s}"
        );
    }

    let draft_path = repo_root().join(p["draft_document"].as_str().unwrap());
    let draft = std::fs::read_to_string(&draft_path)
        .unwrap_or_else(|e| panic!("read draft doc {}: {e}", draft_path.display()));
    assert!(
        draft.contains("2026-11-24-sel2-p2-preregistration.md"),
        "draft must contain pointer to frozen preregistration"
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum R10Label {
    Supports,
    EquivalentToZero,
    InsufficientEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EffectBucket {
    PosGeSesoi,
    NegLeNegSesoi,
    WithinBand,
}

fn classify_sel2p2_r10_reference(
    smoke_and_host_valid: bool,
    attrition_and_drift_valid: bool,
    holm_sig: bool,
    eff: EffectBucket,
    tost_eq: bool,
) -> (R10Label, &'static str) {
    if !smoke_and_host_valid {
        return (R10Label::InsufficientEvidence, "harness_invalid");
    }
    if !attrition_and_drift_valid {
        return (
            R10Label::InsufficientEvidence,
            "attrition_below_floor_or_drift_invalid",
        );
    }
    if eff != EffectBucket::WithinBand && tost_eq {
        return (R10Label::InsufficientEvidence, "ci_inconsistent_impossible");
    }
    if holm_sig {
        match eff {
            EffectBucket::PosGeSesoi => {
                return (R10Label::Supports, "two_sided_positive");
            }
            EffectBucket::NegLeNegSesoi => {
                return (R10Label::Supports, "two_sided_negative");
            }
            EffectBucket::WithinBand => {
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
fn log_mean_ratio_additive_identity_and_closed_r10_3_label_partition() {
    assert_eq!(mass_statistics(&[]), [0.0; 4]);

    let mut0_mean = 2.50_f64;
    let drift_mean = 2.54_f64;
    let sel_mean = 2.82_f64;
    let d_sd = (sel_mean / drift_mean).ln();
    let d_dm = (drift_mean / mut0_mean).ln();
    let d_sm = (sel_mean / mut0_mean).ln();
    assert!((d_sm - (d_sd + d_dm)).abs() < 1e-12);

    let sesoi = 1.10_f64.ln();
    assert!((sesoi - 0.09531017980432493).abs() < 1e-15);

    let mut counts = BTreeMap::new();
    for valid_host in [true, false] {
        for valid_attr in [true, false] {
            for holm in [true, false] {
                for eff in [
                    EffectBucket::PosGeSesoi,
                    EffectBucket::NegLeNegSesoi,
                    EffectBucket::WithinBand,
                ] {
                    for tost in [true, false] {
                        let (lbl, _) =
                            classify_sel2p2_r10_reference(valid_host, valid_attr, holm, eff, tost);
                        *counts.entry(format!("{lbl:?}")).or_insert(0usize) += 1;
                    }
                }
            }
        }
    }
    assert_eq!(counts.values().sum::<usize>(), 48);
    assert_eq!(counts.get("Supports"), Some(&2));
    assert_eq!(counts.get("EquivalentToZero"), Some(&2));
    assert_eq!(counts.get("InsufficientEvidence"), Some(&44));
}
