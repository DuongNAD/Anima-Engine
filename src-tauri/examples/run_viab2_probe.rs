//! Blind `VIAB-2` pre-check probe for `SEL2-P2` (`CARD C5b. SEL2P2-VIAB2`).
//!
//! Executes the 200,000-tick viability check required by `QD-SEL2P2-D1(a)` (`live.founder_genotype
//! = 1.0`) and `QD-SEL2P2-D2(a)` (`scale = 0.75`, `founder_layout = 0`, `drift_reset_ticks = 2000`;
//! optional `-layout 2` fallback per `D2(b)`) on exploratory seeds `880001..=880012` using
//! [`run_live_drift_pair`] and [`run_manifest_seed`].
//!
//! **Blind by construction:** requests and retains strictly non-body-mass allow-listed observables,
//! never computes, prints, or serializes any `trait_mass*` value, and verifies all outputs under
//! `--out` against the blind schema check before returning.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, WorldLawSet,
    MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{run_manifest_seed, RunResult};
use anima_engine_lib::core::live_experiment::{
    run_live_drift_pair, DriftPairReport, LiveExperimentAdapter, LiveWorldConfig,
    LIVE_MODEL_VERSION,
};
use anima_engine_lib::core::sha256::sha256_file;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

pub const DEFAULT_DURATION_TICKS: u64 = 200_000;
pub const DEFAULT_SAMPLE_PERIOD: u64 = 50;
pub const DEFAULT_DRIFT_RESET_TICKS: u64 = 2_000;
pub const DEFAULT_MIN_SURVIVING_BASELINE: usize = 6;
pub const DEFAULT_EXPLORATORY_START: u64 = 880_001;
pub const DEFAULT_EXPLORATORY_END: u64 = 880_012;
pub const UNIT_TEST_ALLOWED_SEEDS: [u64; 3] = [614_001, 614_002, 614_999];
pub const UNIT_TEST_MAX_DURATION_TICKS: u64 = 2_000;

pub const C1_MIN_AGENTS: f64 = 5.0;
pub const C2_G_LOW: f64 = 0.4;
pub const C2_G_HIGH: f64 = 0.6;
pub const C3_MIN_GENERATIONS: f64 = 30.0;
pub const C4_MIN_STARVATION_SHARE: f64 = 0.50;
pub const MIN_SURVIVING_ARMS_D1A: usize = 11;
pub const EU_DRIFT_TOLERANCE: f64 = 1.0e-6;
pub const CAUSE_PARTITION_TOLERANCE: f64 = 1.0e-6;
pub const TEMP_ARTIFACT_NAME: &str = "anima_world_artifact.anmw";

/// Blind non-drift observable allow-list (matches `SEL2-P` smoke non-drift observables).
pub const NON_DRIFT_BLIND_OBS: [&str; 8] = [
    "live.closed_eu_total",
    "live.agent_count",
    "live.standing_crop",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
    "live.deaths_starvation",
    "live.deaths_dehydration",
];

/// Blind drift observable allow-list (starvation/dehydration cause split omitted on drift arm).
pub const DRIFT_BLIND_OBS: [&str; 6] = [
    "live.closed_eu_total",
    "live.agent_count",
    "live.standing_crop",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
];

#[derive(Clone, Debug)]
pub struct Args {
    pub out_dir: Option<PathBuf>,
    pub checkpoint_dir: Option<PathBuf>,
    pub layout: u8,
    pub seeds: Vec<u64>,
    pub duration_ticks: u64,
    pub sample_period: u64,
    pub drift_reset_ticks: u64,
    pub min_surviving_baseline: usize,
    pub jobs: usize,
    pub allow_unit_test_seeds: bool,
    pub self_test: bool,
}

#[derive(Clone, Debug)]
pub struct Viab2ManifestSet {
    pub cell_id: String,
    pub layout: u8,
    pub baseline_ref: ExperimentManifest,
    pub selected: ExperimentManifest,
    pub scarce_mut0: ExperimentManifest,
    pub scarce_drift: ExperimentManifest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArmMetrics {
    pub arm: String,
    pub seed: u64,
    pub completed: bool,
    pub survived: bool,
    pub extinct_tick: Option<u64>,
    pub final_checksum: u32,
    pub terrain_source: Option<String>,
    pub world_identity: Option<WorldIdentity>,
    pub min_n: f64,
    pub mean_n: f64,
    pub max_n: f64,
    pub final_n: f64,
    pub total_births: f64,
    pub total_deaths: f64,
    pub total_starvation: Option<f64>,
    pub total_dehydration: Option<f64>,
    pub starvation_share: Option<f64>,
    pub cause_partition_ok: Option<bool>,
    pub mean_lifespan_weighted: Option<f64>,
    pub generations_proxy: Option<f64>,
    pub births_last_quarter: f64,
    pub deaths_last_quarter: f64,
    pub crop_second_half: f64,
    pub max_abs_eu_drift: f64,
    pub elapsed_secs: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriftDiagSummary {
    pub scheduled_deaths: u64,
    pub actual_deaths: u64,
    pub missed_deaths: u64,
    pub scheduled_births: u64,
    pub actual_births: u64,
    pub missed_births_extinct: u64,
    pub missed_births_insolvent: u64,
    pub missed_births_capped: u64,
    pub ticks_with_missed_deaths: u64,
    pub ticks_with_missed_births: u64,
    pub first_divergence_tick: Option<u64>,
    pub window_count: usize,
    pub has_shortfall: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PooledBaselineSummary {
    pub n_requested: usize,
    pub n_surviving: usize,
    pub min_required: usize,
    pub surviving_seeds: Vec<u64>,
    pub pooled_crop_second_half: Option<f64>,
    pub sufficient: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeedGateRow {
    pub seed: u64,
    pub baseline_ref: ArmMetrics,
    pub selected: ArmMetrics,
    pub scarce_mut0: ArmMetrics,
    pub scarce_drift: ArmMetrics,
    pub drift_diagnostics: DriftDiagSummary,
    pub g_selected: Option<f64>,
    pub g_scarce_mut0: Option<f64>,
    pub g_scarce_drift: Option<f64>,
    pub c1_selected_min5: bool,
    pub c1_mut0_min5: bool,
    pub c1_all_scarce_min10: bool,
    pub c2_selected: bool,
    pub c2_mut0: bool,
    pub c2_both: bool,
    pub c3: bool,
    pub c4: bool,
    pub c5: bool,
    pub drift_valid: bool,
    pub gate_pass: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CellAggregateSummary {
    pub n_seeds: usize,
    pub baseline_survived: usize,
    pub selected_survived: usize,
    pub scarce_mut0_survived: usize,
    pub scarce_drift_survived: usize,
    pub selected_min_n_ge_5: usize,
    pub scarce_mut0_min_n_ge_5: usize,
    pub scarce_drift_min_n_ge_5: usize,
    pub selected_min_n_ge_10: usize,
    pub scarce_mut0_min_n_ge_10: usize,
    pub scarce_drift_min_n_ge_10: usize,
    pub c2_selected_pass: usize,
    pub c2_mut0_pass: usize,
    pub c2_both_pass: usize,
    pub c3_pass: usize,
    pub c4_pass: usize,
    pub c5_pass: usize,
    pub drift_valid_count: usize,
    pub gate_pass_count: usize,
    pub max_abs_eu_drift_overall: f64,
    pub all_terrain_generated: bool,
    pub d1a_survival_bar: usize,
    pub d1a_survival_pass: bool,
    pub min_n_shortfall: bool,
    pub c2_shortfall: bool,
    pub d2b_escalation_recommended: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Viab2CellReport {
    pub suite: String,
    pub cell_id: String,
    pub mass_observables_excluded: bool,
    pub blind_mode: String,
    pub layout: u8,
    pub duration_ticks: u64,
    pub sample_period: u64,
    pub drift_reset_ticks: u64,
    pub trunk_sha: Option<String>,
    pub binary_sha256: Option<String>,
    pub elapsed_secs: f64,
    pub seeds_evaluated: Vec<u64>,
    pub world_identities_observed: Vec<WorldIdentity>,
    pub all_terrain_generated: bool,
    pub pooled_baseline: PooledBaselineSummary,
    pub aggregate: CellAggregateSummary,
    pub per_seed: Vec<SeedGateRow>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct JobCheckpoint {
    cell_id: String,
    seed: u64,
    job_kind: String,
    manifest_fingerprint_a: u64,
    manifest_fingerprint_b: Option<u64>,
    duration_ticks: u64,
    sample_period: u64,
    drift_reset_ticks: u64,
    arm_a: ArmMetrics,
    arm_b: Option<ArmMetrics>,
    drift_diag: Option<DriftDiagSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProvenanceCellEntry {
    pub cell_id: String,
    pub file: String,
    pub sha256: String,
    pub layout: u8,
    pub d1a_survival_pass: bool,
    pub d2b_escalation_recommended: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Viab2Provenance {
    pub experiment: String,
    pub card: String,
    pub mass_observables_excluded: bool,
    pub blind_mode: String,
    pub allowed_non_drift_observables: Vec<String>,
    pub allowed_drift_observables: Vec<String>,
    pub model: String,
    pub model_version: String,
    pub generated_utc: String,
    pub command_line: Vec<String>,
    pub git_commit: Option<String>,
    pub git_tree_dirty: Option<bool>,
    pub rustc_version: Option<String>,
    pub binary_path: Option<String>,
    pub binary_sha256: Option<String>,
    pub host_os: String,
    pub host_arch: String,
    pub preflight_world_artifact_unset: bool,
    pub preflight_gpu_disabled: bool,
    pub preflight_temp_artifact_absent: bool,
    pub cells: Vec<ProvenanceCellEntry>,
}

#[allow(dead_code)]
fn main() {
    if let Err(e) = run_cli() {
        eprintln!("run_viab2_probe failed: {e}");
        std::process::exit(1);
    }
}

fn run_cli() -> Result<(), String> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let args = parse_args(&raw)?;
    if args.self_test {
        return run_self_test();
    }
    execute_probe(&args, &raw)
}

pub fn parse_seeds(spec: &str) -> Result<Vec<u64>, String> {
    if let Some((start_s, end_s)) = spec.split_once("..=") {
        let start: u64 = start_s
            .parse()
            .map_err(|e| format!("bad seed start: {e}"))?;
        let end: u64 = end_s.parse().map_err(|e| format!("bad seed end: {e}"))?;
        if start > end {
            return Err(format!("empty seed range {start}..={end}"));
        }
        Ok((start..=end).collect())
    } else {
        let mut out = Vec::new();
        for tok in spec.split(',') {
            let trimmed = tok.trim();
            if trimmed.is_empty() {
                continue;
            }
            let s: u64 = trimmed
                .parse()
                .map_err(|e| format!("bad seed '{trimmed}': {e}"))?;
            out.push(s);
        }
        Ok(out)
    }
}

pub fn parse_args(argv: &[String]) -> Result<Args, String> {
    let mut out_dir: Option<PathBuf> = None;
    let mut checkpoint_dir: Option<PathBuf> = None;
    let mut layout: u8 = 0;
    let mut seeds: Vec<u64> = (DEFAULT_EXPLORATORY_START..=DEFAULT_EXPLORATORY_END).collect();
    let mut duration_ticks: u64 = DEFAULT_DURATION_TICKS;
    let mut sample_period: u64 = DEFAULT_SAMPLE_PERIOD;
    let mut drift_reset_ticks: u64 = DEFAULT_DRIFT_RESET_TICKS;
    let mut min_surviving_baseline: usize = DEFAULT_MIN_SURVIVING_BASELINE;
    let mut jobs: usize = 8;
    let mut allow_unit_test_seeds = false;
    let mut self_test = false;

    let mut i = 0;
    while i < argv.len() {
        let flag = argv[i].as_str();
        match flag {
            "--help" | "-h" => {
                println!(
                    "run_viab2_probe [--layout 0|2] [--seeds 880001..=880012] \
                     [--duration-ticks 200000] [--sample-period 50] [--drift-reset-ticks 2000] \
                     [--min-surviving-baseline 6] [--jobs 8] --checkpoint-dir <dir> --out <dir> | --self-test"
                );
                std::process::exit(0);
            }
            "--self-test" => {
                self_test = true;
                i += 1;
                continue;
            }
            "--allow-unit-test-seeds" => {
                allow_unit_test_seeds = true;
                i += 1;
                continue;
            }
            _ => {}
        }

        let next = argv
            .get(i + 1)
            .ok_or_else(|| format!("missing value for {flag}"))?
            .clone();
        match flag {
            "--out" => out_dir = Some(PathBuf::from(next)),
            "--checkpoint-dir" => checkpoint_dir = Some(PathBuf::from(next)),
            "--layout" => {
                let v: u8 = next.parse().map_err(|e| format!("bad --layout: {e}"))?;
                if v != 0 && v != 2 {
                    return Err(format!("--layout must be 0 or 2, got {v}"));
                }
                layout = v;
            }
            "--seeds" => seeds = parse_seeds(&next)?,
            "--duration-ticks" => {
                duration_ticks = next
                    .parse()
                    .map_err(|e| format!("bad --duration-ticks: {e}"))?
            }
            "--sample-period" => {
                sample_period = next
                    .parse()
                    .map_err(|e| format!("bad --sample-period: {e}"))?
            }
            "--drift-reset-ticks" => {
                drift_reset_ticks = next
                    .parse()
                    .map_err(|e| format!("bad --drift-reset-ticks: {e}"))?
            }
            "--min-surviving-baseline" => {
                min_surviving_baseline = next
                    .parse()
                    .map_err(|e| format!("bad --min-surviving-baseline: {e}"))?
            }
            "--jobs" => {
                jobs = next
                    .parse::<usize>()
                    .map_err(|e| format!("bad --jobs: {e}"))?
                    .max(1)
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
        i += 2;
    }

    if !self_test {
        if out_dir.is_none() {
            return Err("--out is required unless --self-test".into());
        }
        if checkpoint_dir.is_none() {
            return Err("--checkpoint-dir is required unless --self-test".into());
        }
        if let (Some(o), Some(c)) = (&out_dir, &checkpoint_dir) {
            if c.starts_with(o) {
                return Err("--checkpoint-dir must sit outside --out".into());
            }
        }
        validate_seed_policy(&seeds, allow_unit_test_seeds, duration_ticks)?;
        if sample_period == 0 || !duration_ticks.is_multiple_of(sample_period) {
            return Err(format!(
                "--sample-period ({sample_period}) must be > 0 and divide duration_ticks ({duration_ticks})"
            ));
        }
        if drift_reset_ticks > 0 && !duration_ticks.is_multiple_of(drift_reset_ticks) {
            return Err(format!(
                "--drift-reset-ticks ({drift_reset_ticks}) must divide duration_ticks ({duration_ticks})"
            ));
        }
    }

    Ok(Args {
        out_dir,
        checkpoint_dir,
        layout,
        seeds,
        duration_ticks,
        sample_period,
        drift_reset_ticks,
        min_surviving_baseline,
        jobs,
        allow_unit_test_seeds,
        self_test,
    })
}

pub fn validate_seed_policy(
    seeds: &[u64],
    allow_unit_test_seeds: bool,
    duration_ticks: u64,
) -> Result<(), String> {
    if seeds.is_empty() {
        return Err("seed list is empty".into());
    }
    let mut uniq = BTreeSet::new();
    for &s in seeds {
        if !uniq.insert(s) {
            return Err(format!("duplicate seed {s}"));
        }
        if allow_unit_test_seeds {
            if !UNIT_TEST_ALLOWED_SEEDS.contains(&s) {
                return Err(format!(
                    "unit-test mode refuses seed {s}; allowed unit-test seeds: {:?}",
                    UNIT_TEST_ALLOWED_SEEDS
                ));
            }
            if duration_ticks > UNIT_TEST_MAX_DURATION_TICKS {
                return Err(format!(
                    "unit-test mode refuses duration_ticks {duration_ticks} > {UNIT_TEST_MAX_DURATION_TICKS}"
                ));
            }
        } else if !(DEFAULT_EXPLORATORY_START..=DEFAULT_EXPLORATORY_END).contains(&s) {
            return Err(format!(
                "refusing non-VIAB-2 exploratory seed {s} (allowed: {}..={})",
                DEFAULT_EXPLORATORY_START, DEFAULT_EXPLORATORY_END
            ));
        }
    }
    Ok(())
}

pub fn check_host_preflight() -> Result<(bool, bool, bool), String> {
    if let Some(val) = std::env::var_os("ANIMA_WORLD_ARTIFACT") {
        return Err(format!(
            "host preflight failed: ANIMA_WORLD_ARTIFACT must be unset (found {:?})",
            val
        ));
    }
    if let Some(gpu) = std::env::var_os("ANIMA_USE_GPU") {
        if gpu.to_string_lossy() != "0" {
            return Err(format!(
                "host preflight failed: ANIMA_USE_GPU must be unset or '0' (found {:?})",
                gpu
            ));
        }
    }
    let tmp_artifact = std::env::temp_dir().join(TEMP_ARTIFACT_NAME);
    if tmp_artifact.exists() {
        return Err(format!(
            "host preflight failed: desktop world artifact present at {} (remove or stop app first; probe never deletes it)",
            tmp_artifact.display()
        ));
    }
    Ok((true, true, true))
}

pub fn build_viab2_manifests(
    layout: u8,
    seeds: &[u64],
    duration_ticks: u64,
    sample_period: u64,
    drift_reset_ticks: u64,
) -> Result<Viab2ManifestSet, String> {
    if layout != 0 && layout != 2 {
        return Err(format!(
            "unsupported founder_layout {layout}; expected 0 or 2"
        ));
    }
    let registry = ObservableRegistry::live_default();
    registry
        .validate()
        .map_err(|e| format!("live registry invalid: {e}"))?;

    let non_drift_obs: Vec<String> = NON_DRIFT_BLIND_OBS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let drift_obs: Vec<String> = DRIFT_BLIND_OBS.iter().map(|s| (*s).to_string()).collect();

    let make_ic = |scale: f64, prey_cap: f64, mut_rate: f64, drift_ctrl: f64, reset: f64| {
        InitialConditionSet::new(vec![
            ("live.founders".into(), 100.0),
            ("live.founder_layout".into(), f64::from(layout)),
            ("live.founder_genotype".into(), 1.0),
            ("live.predator_fraction".into(), 0.0),
            ("live.trees".into(), 8.0),
            ("live.lakes".into(), 16.0),
            ("live.lake_layout".into(), 1.0),
            ("live.resource_capacity_scale".into(), scale),
            ("live.plant_reserve".into(), 0.0),
            ("live.prey_cap".into(), prey_cap),
            ("live.strict_prey_cap".into(), 1.0),
            ("live.dehydration_ticks".into(), 960.0),
            ("live.mortality".into(), 1.0),
            ("live.reproduction".into(), 1.0),
            ("live.locomotion".into(), 1.0),
            ("live.drive_floor".into(), 0.5),
            ("live.thirst_steering".into(), 1.0),
            ("live.evolved_brains".into(), 0.0),
            ("live.mutation_rate".into(), mut_rate),
            ("live.drift_control".into(), drift_ctrl),
            ("live.drift_reset_ticks".into(), reset),
        ])
    };

    let cell_id = format!("viab2_fg1_s075_k2k_fl{layout}");
    let make_manifest = |arm_suffix: &str,
                         scale: f64,
                         prey_cap: f64,
                         mut_rate: f64,
                         drift_ctrl: f64,
                         reset: f64,
                         obs: &[String]|
     -> Result<ExperimentManifest, String> {
        let m = ExperimentManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            experiment_id: format!("sel2-p2-viab2::{cell_id}::{arm_suffix}"),
            name: format!("VIAB-2 {cell_id} {arm_suffix}"),
            observer: Default::default(),
            world_identity: WorldIdentity::default(),
            laws: WorldLawSet::baseline(),
            initial_conditions: make_ic(scale, prey_cap, mut_rate, drift_ctrl, reset),
            interventions: Vec::new(),
            seeds: seeds.to_vec(),
            duration_ticks,
            sample_period,
            observable_ids: obs.to_vec(),
            exotic_interventions: Vec::new(),
        };
        m.validate(&registry)
            .map_err(|e| format!("manifest {arm_suffix} invalid: {e}"))?;
        let cfg = LiveWorldConfig::from_initial_conditions(&m.initial_conditions)
            .map_err(|e| format!("config {arm_suffix} invalid: {e}"))?;
        cfg.ensure_observables_supported(&m.observable_ids)
            .map_err(|e| format!("observables {arm_suffix} invalid: {e}"))?;
        Ok(m)
    };

    let baseline_ref = make_manifest("baseline_ref", 1.0, 80.0, 0.05, 0.0, 0.0, &non_drift_obs)?;
    let selected = make_manifest("selected", 0.75, 320.0, 0.05, 0.0, 0.0, &non_drift_obs)?;
    let scarce_mut0 = make_manifest("scarce_mut0", 0.75, 320.0, 0.0, 0.0, 0.0, &non_drift_obs)?;
    let scarce_drift = make_manifest(
        "scarce_drift",
        0.75,
        320.0,
        0.05,
        1.0,
        drift_reset_ticks as f64,
        &drift_obs,
    )?;

    Ok(Viab2ManifestSet {
        cell_id,
        layout,
        baseline_ref,
        selected,
        scarce_mut0,
        scarce_drift,
    })
}

fn contains_forbidden_mass_token(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.contains("trait_mass") || lower.contains("body_mass")
}

fn check_json_value_blind(val: &serde_json::Value, path: &str) -> Result<(), String> {
    match val {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                if contains_forbidden_mass_token(k) {
                    return Err(format!(
                        "blindness violation: forbidden mass key '{k}' at {path}.{k}"
                    ));
                }
                check_json_value_blind(v, &format!("{path}.{k}"))?;
            }
        }
        serde_json::Value::Array(arr) => {
            for (idx, item) in arr.iter().enumerate() {
                check_json_value_blind(item, &format!("{path}[{idx}]"))?;
            }
        }
        serde_json::Value::String(s) if contains_forbidden_mass_token(s) => {
            return Err(format!(
                "blindness violation: forbidden mass string '{s}' at {path}"
            ));
        }
        _ => {}
    }
    Ok(())
}

/// Assert zero `trait_mass*` / `body_mass*` keys, strings, or substrings exist in any output file.
pub fn assert_blind_output_dir(dir: &Path) -> Result<(), String> {
    if !dir.is_dir() {
        return Err(format!("output dir missing: {}", dir.display()));
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(cur) = stack.pop() {
        let rd = fs::read_dir(&cur).map_err(|e| format!("read_dir {}: {e}", cur.display()))?;
        for entry in rd {
            let entry = entry.map_err(|e| format!("dir entry error: {e}"))?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let text =
                fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
            if contains_forbidden_mass_token(&text) {
                return Err(format!(
                    "blindness violation: forbidden mass substring found in {}",
                    path.display()
                ));
            }
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let parsed: serde_json::Value = serde_json::from_str(&text)
                    .map_err(|e| format!("invalid JSON in {}: {e}", path.display()))?;
                check_json_value_blind(&parsed, path.to_string_lossy().as_ref())?;
            }
        }
    }
    Ok(())
}

fn blind_sample_value(sample: &[(String, f64)], allowed_id: &str) -> Result<f64, String> {
    if !NON_DRIFT_BLIND_OBS.contains(&allowed_id) {
        return Err(format!("refusing non-allowlisted observable {allowed_id}"));
    }
    sample
        .iter()
        .find(|(k, _)| k == allowed_id)
        .map(|(_, v)| *v)
        .ok_or_else(|| format!("missing observable {allowed_id}"))
}

fn summarize_blind_arm(
    arm: &str,
    seed: u64,
    duration_ticks: u64,
    allow_cause_split: bool,
    elapsed_secs: f64,
    run: &RunResult,
) -> Result<ArmMetrics, String> {
    let completed = run.status.is_completed();
    let final_n = blind_sample_value(&run.final_observables, "live.agent_count")?;
    let eu_final = blind_sample_value(&run.final_observables, "live.closed_eu_total")?;

    let mut min_n = f64::INFINITY;
    let mut max_n = f64::NEG_INFINITY;
    let mut sum_n = 0.0;
    let mut count_n = 0usize;
    let mut extinct_tick: Option<u64> = None;

    let mut total_births = 0.0;
    let mut total_deaths = 0.0;
    let mut lifespan_weighted = 0.0;
    let mut total_starvation = 0.0;
    let mut total_dehydration = 0.0;
    let mut per_sample_part_ok = true;
    let mut births_last_quarter = 0.0;
    let mut deaths_last_quarter = 0.0;
    let mut crop_2nd_sum = 0.0;
    let mut crop_2nd_n = 0usize;
    let mut eu_vals = Vec::with_capacity(run.series.len() + 1);

    for s in &run.series {
        let n = blind_sample_value(&s.observables, "live.agent_count")?;
        let sc = blind_sample_value(&s.observables, "live.standing_crop")?;
        let eu = blind_sample_value(&s.observables, "live.closed_eu_total")?;
        let b = blind_sample_value(&s.observables, "live.births")?;
        let d = blind_sample_value(&s.observables, "live.deaths")?;
        let ls = blind_sample_value(&s.observables, "live.mean_lifespan")?;

        min_n = min_n.min(n);
        max_n = max_n.max(n);
        sum_n += n;
        count_n += 1;
        if n == 0.0 && extinct_tick.is_none() {
            extinct_tick = Some(s.tick);
        }
        if s.tick.saturating_mul(2) > duration_ticks {
            crop_2nd_sum += sc;
            crop_2nd_n += 1;
        }
        if s.tick.saturating_mul(4) > duration_ticks.saturating_mul(3) {
            births_last_quarter += b;
            deaths_last_quarter += d;
        }
        total_births += b;
        total_deaths += d;
        lifespan_weighted += ls * d;

        if allow_cause_split {
            let ds = blind_sample_value(&s.observables, "live.deaths_starvation")?;
            let dh = blind_sample_value(&s.observables, "live.deaths_dehydration")?;
            total_starvation += ds;
            total_dehydration += dh;
            if ((ds + dh) - d).abs() > CAUSE_PARTITION_TOLERANCE {
                per_sample_part_ok = false;
            }
        }
        eu_vals.push(eu);
    }
    eu_vals.push(eu_final);

    if extinct_tick.is_none() && final_n == 0.0 {
        extinct_tick = Some(duration_ticks);
    }
    let survived = completed && extinct_tick.is_none() && final_n > 0.0;
    let mean_n = if count_n > 0 {
        sum_n / count_n as f64
    } else {
        0.0
    };
    let crop_second_half = if crop_2nd_n > 0 {
        crop_2nd_sum / crop_2nd_n as f64
    } else {
        0.0
    };
    let mean_lifespan_weighted = if total_deaths > 0.0 {
        Some(lifespan_weighted / total_deaths)
    } else {
        None
    };
    let generations_proxy = match mean_lifespan_weighted {
        Some(ls) if ls.is_finite() && ls > 0.0 && mean_n > 0.0 => {
            let by_life = (duration_ticks as f64) / ls;
            let by_turn = total_births / mean_n.max(1.0);
            Some(by_life.min(by_turn))
        }
        _ => None,
    };

    let (starv_opt, dehyd_opt, starv_share, part_ok_opt) = if allow_cause_split {
        let total_ok = per_sample_part_ok
            && ((total_starvation + total_dehydration) - total_deaths).abs()
                <= CAUSE_PARTITION_TOLERANCE;
        let share = if total_ok && total_deaths > 0.0 {
            Some(total_starvation / total_deaths)
        } else {
            None
        };
        (
            Some(total_starvation),
            Some(total_dehydration),
            share,
            Some(total_ok),
        )
    } else {
        (None, None, None, None)
    };

    let eu_first = eu_vals.first().copied().unwrap_or(0.0);
    let max_abs_eu_drift = eu_vals
        .iter()
        .map(|v| (*v - eu_first).abs())
        .fold(0.0, f64::max);

    Ok(ArmMetrics {
        arm: arm.to_string(),
        seed,
        completed,
        survived,
        extinct_tick,
        final_checksum: run.final_checksum,
        terrain_source: run.provenance.terrain_source.clone(),
        world_identity: run.provenance.world_identity,
        min_n: min_n.min(final_n),
        mean_n,
        max_n: max_n.max(final_n),
        final_n,
        total_births,
        total_deaths,
        total_starvation: starv_opt,
        total_dehydration: dehyd_opt,
        starvation_share: starv_share,
        cause_partition_ok: part_ok_opt,
        mean_lifespan_weighted,
        generations_proxy,
        births_last_quarter,
        deaths_last_quarter,
        crop_second_half,
        max_abs_eu_drift,
        elapsed_secs,
    })
}

fn summarize_drift_diag(pair: &DriftPairReport) -> DriftDiagSummary {
    let d = &pair.diagnostics;
    DriftDiagSummary {
        scheduled_deaths: d.scheduled_deaths,
        actual_deaths: d.actual_deaths,
        missed_deaths: d.missed_deaths,
        scheduled_births: d.scheduled_births,
        actual_births: d.actual_births,
        missed_births_extinct: d.missed_births_extinct,
        missed_births_insolvent: d.missed_births_insolvent,
        missed_births_capped: d.missed_births_capped,
        ticks_with_missed_deaths: d.ticks_with_missed_deaths,
        ticks_with_missed_births: d.ticks_with_missed_births,
        first_divergence_tick: d.first_divergence_tick,
        window_count: pair.window_count,
        has_shortfall: d.has_shortfall(),
    }
}

#[derive(Clone, Debug)]
pub struct CellEvaluationOutcome {
    pub pooled_baseline: PooledBaselineSummary,
    pub aggregate: CellAggregateSummary,
    pub per_seed: Vec<SeedGateRow>,
    pub world_identities_observed: Vec<WorldIdentity>,
    pub all_terrain_generated: bool,
}

pub fn evaluate_cell_outcomes(
    base_map: &BTreeMap<u64, ArmMetrics>,
    mut0_map: &BTreeMap<u64, ArmMetrics>,
    pair_map: &BTreeMap<u64, (ArmMetrics, ArmMetrics, DriftDiagSummary)>,
    seeds: &[u64],
    min_surviving_baseline: usize,
) -> Result<CellEvaluationOutcome, String> {
    let mut surviving_base_seeds = Vec::new();
    let mut base_crops = Vec::new();
    for &seed in seeds {
        let b = base_map
            .get(&seed)
            .ok_or_else(|| format!("missing baseline_ref for seed {seed}"))?;
        if b.survived && b.crop_second_half.is_finite() {
            surviving_base_seeds.push(seed);
            base_crops.push(b.crop_second_half);
        }
    }
    let sufficient = surviving_base_seeds.len() >= min_surviving_baseline;
    let pooled_crop = if sufficient && !base_crops.is_empty() {
        Some(base_crops.iter().sum::<f64>() / base_crops.len() as f64)
    } else {
        None
    };
    let pooled_baseline = PooledBaselineSummary {
        n_requested: seeds.len(),
        n_surviving: surviving_base_seeds.len(),
        min_required: min_surviving_baseline,
        surviving_seeds: surviving_base_seeds,
        pooled_crop_second_half: pooled_crop,
        sufficient,
    };

    let mut wid_set: BTreeSet<String> = BTreeSet::new();
    let mut wid_vec: Vec<WorldIdentity> = Vec::new();
    let mut all_terrain_generated = true;
    let mut per_seed = Vec::with_capacity(seeds.len());
    let mut max_drift = 0.0f64;

    for &seed in seeds {
        let base = base_map
            .get(&seed)
            .ok_or_else(|| format!("missing baseline_ref for seed {seed}"))?
            .clone();
        let mut0 = mut0_map
            .get(&seed)
            .ok_or_else(|| format!("missing scarce_mut0 for seed {seed}"))?
            .clone();
        let (sel, drf, diag) = pair_map
            .get(&seed)
            .ok_or_else(|| format!("missing drift pair for seed {seed}"))?
            .clone();

        for arm in [&base, &sel, &mut0, &drf] {
            if arm.terrain_source.as_deref() != Some("generated") {
                all_terrain_generated = false;
            }
            max_drift = max_drift.max(arm.max_abs_eu_drift);
            if let Some(wid) = arm.world_identity {
                let key = format!(
                    "{}:{}:{}x{}:{}",
                    wid.seed, wid.generator_version, wid.width, wid.height, wid.checksum
                );
                if wid_set.insert(key) {
                    wid_vec.push(wid);
                }
            }
        }

        let c1_selected_min5 = sel.survived && sel.min_n >= C1_MIN_AGENTS;
        let c1_mut0_min5 = mut0.survived && mut0.min_n >= C1_MIN_AGENTS;
        let c1_all_scarce_min10 = sel.survived
            && sel.min_n >= 10.0
            && mut0.survived
            && mut0.min_n >= 10.0
            && drf.survived
            && drf.min_n >= 10.0;

        let ref_crop = pooled_crop.filter(|c| sufficient && c.is_finite() && *c > 0.0);
        let g_of = |a: &ArmMetrics| -> Option<f64> {
            let rc = ref_crop?;
            if a.crop_second_half.is_finite() {
                Some(a.crop_second_half / rc)
            } else {
                None
            }
        };
        let g_selected = g_of(&sel);
        let g_scarce_mut0 = g_of(&mut0);
        let g_scarce_drift = g_of(&drf);
        let g_ok = |g: Option<f64>| g.is_some_and(|v| (C2_G_LOW..=C2_G_HIGH).contains(&v));
        let c2_selected = sufficient && g_ok(g_selected);
        let c2_mut0 = sufficient && g_ok(g_scarce_mut0);
        let c2_both = c2_selected && c2_mut0;

        let c3 = sel
            .generations_proxy
            .is_some_and(|g| g >= C3_MIN_GENERATIONS)
            && mut0
                .generations_proxy
                .is_some_and(|g| g >= C3_MIN_GENERATIONS);

        let c4 = sel
            .starvation_share
            .is_some_and(|s| s > C4_MIN_STARVATION_SHARE)
            && mut0
                .starvation_share
                .is_some_and(|s| s > C4_MIN_STARVATION_SHARE);

        let c5_arm = |a: &ArmMetrics| a.births_last_quarter > 0.0 && a.deaths_last_quarter > 0.0;
        let c5 = c5_arm(&sel) && c5_arm(&mut0);
        let drift_valid = drf.survived;
        let gate_pass =
            c1_selected_min5 && c1_mut0_min5 && c2_both && c3 && c4 && c5 && drift_valid;

        per_seed.push(SeedGateRow {
            seed,
            baseline_ref: base,
            selected: sel,
            scarce_mut0: mut0,
            scarce_drift: drf,
            drift_diagnostics: diag,
            g_selected,
            g_scarce_mut0,
            g_scarce_drift,
            c1_selected_min5,
            c1_mut0_min5,
            c1_all_scarce_min10,
            c2_selected,
            c2_mut0,
            c2_both,
            c3,
            c4,
            c5,
            drift_valid,
            gate_pass,
        });
    }

    let n_seeds = per_seed.len();
    let baseline_survived = per_seed.iter().filter(|r| r.baseline_ref.survived).count();
    let selected_survived = per_seed.iter().filter(|r| r.selected.survived).count();
    let scarce_mut0_survived = per_seed.iter().filter(|r| r.scarce_mut0.survived).count();
    let scarce_drift_survived = per_seed.iter().filter(|r| r.scarce_drift.survived).count();
    let selected_min_n_ge_5 = per_seed.iter().filter(|r| r.c1_selected_min5).count();
    let scarce_mut0_min_n_ge_5 = per_seed.iter().filter(|r| r.c1_mut0_min5).count();
    let scarce_drift_min_n_ge_5 = per_seed
        .iter()
        .filter(|r| r.scarce_drift.survived && r.scarce_drift.min_n >= C1_MIN_AGENTS)
        .count();
    let selected_min_n_ge_10 = per_seed
        .iter()
        .filter(|r| r.selected.survived && r.selected.min_n >= 10.0)
        .count();
    let scarce_mut0_min_n_ge_10 = per_seed
        .iter()
        .filter(|r| r.scarce_mut0.survived && r.scarce_mut0.min_n >= 10.0)
        .count();
    let scarce_drift_min_n_ge_10 = per_seed
        .iter()
        .filter(|r| r.scarce_drift.survived && r.scarce_drift.min_n >= 10.0)
        .count();
    let c2_selected_pass = per_seed.iter().filter(|r| r.c2_selected).count();
    let c2_mut0_pass = per_seed.iter().filter(|r| r.c2_mut0).count();
    let c2_both_pass = per_seed.iter().filter(|r| r.c2_both).count();
    let c3_pass = per_seed.iter().filter(|r| r.c3).count();
    let c4_pass = per_seed.iter().filter(|r| r.c4).count();
    let c5_pass = per_seed.iter().filter(|r| r.c5).count();
    let drift_valid_count = per_seed.iter().filter(|r| r.drift_valid).count();
    let gate_pass_count = per_seed.iter().filter(|r| r.gate_pass).count();

    let bar = if n_seeds == 12 {
        MIN_SURVIVING_ARMS_D1A
    } else {
        n_seeds
    };
    let d1a_survival_pass = selected_survived >= bar
        && scarce_mut0_survived >= bar
        && scarce_drift_survived >= bar
        && all_terrain_generated
        && max_drift <= EU_DRIFT_TOLERANCE;
    let min_n_shortfall =
        selected_min_n_ge_5 < bar || scarce_mut0_min_n_ge_5 < bar || scarce_drift_min_n_ge_5 < bar;
    let c2_shortfall = !sufficient || c2_both_pass < bar.saturating_sub(1);
    let d2b_escalation_recommended = min_n_shortfall || c2_shortfall;

    let agg = CellAggregateSummary {
        n_seeds,
        baseline_survived,
        selected_survived,
        scarce_mut0_survived,
        scarce_drift_survived,
        selected_min_n_ge_5,
        scarce_mut0_min_n_ge_5,
        scarce_drift_min_n_ge_5,
        selected_min_n_ge_10,
        scarce_mut0_min_n_ge_10,
        scarce_drift_min_n_ge_10,
        c2_selected_pass,
        c2_mut0_pass,
        c2_both_pass,
        c3_pass,
        c4_pass,
        c5_pass,
        drift_valid_count,
        gate_pass_count,
        max_abs_eu_drift_overall: max_drift,
        all_terrain_generated,
        d1a_survival_bar: bar,
        d1a_survival_pass,
        min_n_shortfall,
        c2_shortfall,
        d2b_escalation_recommended,
    };

    Ok(CellEvaluationOutcome {
        pooled_baseline,
        aggregate: agg,
        per_seed,
        world_identities_observed: wid_vec,
        all_terrain_generated,
    })
}

struct CheckpointKey<'a> {
    cell_id: &'a str,
    seed: u64,
    job_kind: &'a str,
    fp_a: u64,
    fp_b: Option<u64>,
    duration_ticks: u64,
    sample_period: u64,
    drift_reset_ticks: u64,
}

fn load_valid_checkpoint(path: &Path, key: &CheckpointKey<'_>) -> Option<JobCheckpoint> {
    let raw = fs::read_to_string(path).ok()?;
    if contains_forbidden_mass_token(&raw) {
        return None;
    }
    let ckpt: JobCheckpoint = serde_json::from_str(&raw).ok()?;
    if ckpt.cell_id == key.cell_id
        && ckpt.seed == key.seed
        && ckpt.job_kind == key.job_kind
        && ckpt.manifest_fingerprint_a == key.fp_a
        && ckpt.manifest_fingerprint_b == key.fp_b
        && ckpt.duration_ticks == key.duration_ticks
        && ckpt.sample_period == key.sample_period
        && ckpt.drift_reset_ticks == key.drift_reset_ticks
    {
        Some(ckpt)
    } else {
        None
    }
}

fn write_atomic_json<T: Serialize>(path: &Path, value: &T) -> Result<String, String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| format!("json serialize error: {e}"))?
        + "\n";
    let normalized = json.replace("\r\n", "\n");
    if contains_forbidden_mass_token(&normalized) {
        return Err(format!(
            "blindness violation before write: {}",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create_dir_all {}: {e}", parent.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, normalized.as_bytes()).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("rename {}: {e}", path.display()))?;
    Ok(normalized)
}

fn opt_u64(v: Option<u64>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

fn opt_f64(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.6}")).unwrap_or_default()
}

pub fn execute_probe(args: &Args, raw_cli: &[String]) -> Result<(), String> {
    let (wf_unset, gpu_off, tmp_absent) = check_host_preflight()?;
    let out_dir = args
        .out_dir
        .as_ref()
        .ok_or_else(|| "--out missing".to_string())?;
    let ckpt_dir = args
        .checkpoint_dir
        .as_ref()
        .ok_or_else(|| "--checkpoint-dir missing".to_string())?;

    fs::create_dir_all(out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;
    fs::create_dir_all(ckpt_dir).map_err(|e| format!("create {}: {e}", ckpt_dir.display()))?;

    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(args.jobs)
        .build_global();

    let manifests = build_viab2_manifests(
        args.layout,
        &args.seeds,
        args.duration_ticks,
        args.sample_period,
        args.drift_reset_ticks,
    )?;
    let registry = ObservableRegistry::live_default();
    let started = Instant::now();

    let fp_base = manifests.baseline_ref.fingerprint();
    let fp_mut0 = manifests.scarce_mut0.fingerprint();
    let mut sel_for_drift = manifests.selected.clone();
    sel_for_drift.observable_ids = manifests.scarce_drift.observable_ids.clone();
    let fp_sel_drift = sel_for_drift.fingerprint();
    let fp_drift = manifests.scarce_drift.fingerprint();

    let mut jobs: Vec<(&'static str, u64)> = Vec::with_capacity(args.seeds.len() * 3);
    for &seed in &args.seeds {
        jobs.push(("drift_pair", seed));
        jobs.push(("baseline_ref", seed));
        jobs.push(("scarce_mut0", seed));
    }

    let results: Vec<Result<JobCheckpoint, String>> = jobs
        .par_iter()
        .map(|&(kind, seed)| {
            let ckpt_path = ckpt_dir.join(format!("{}__{}__{seed}.json", manifests.cell_id, kind));
            let (fp_a, fp_b) = match kind {
                "baseline_ref" => (fp_base, None),
                "scarce_mut0" => (fp_mut0, None),
                _ => (fp_sel_drift, Some(fp_drift)),
            };
            let ckpt_key = CheckpointKey {
                cell_id: &manifests.cell_id,
                seed,
                job_kind: kind,
                fp_a,
                fp_b,
                duration_ticks: args.duration_ticks,
                sample_period: args.sample_period,
                drift_reset_ticks: args.drift_reset_ticks,
            };
            if let Some(cached) = load_valid_checkpoint(&ckpt_path, &ckpt_key) {
                return Ok(cached);
            }

            let ckpt = match kind {
                "baseline_ref" => {
                    let t0 = Instant::now();
                    let run = run_manifest_seed::<LiveExperimentAdapter>(
                        &manifests.baseline_ref,
                        &registry,
                        seed,
                        None,
                        None,
                    );
                    let m = summarize_blind_arm(
                        "baseline_ref",
                        seed,
                        args.duration_ticks,
                        true,
                        t0.elapsed().as_secs_f64(),
                        &run,
                    )?;
                    JobCheckpoint {
                        cell_id: manifests.cell_id.clone(),
                        seed,
                        job_kind: kind.to_string(),
                        manifest_fingerprint_a: fp_a,
                        manifest_fingerprint_b: fp_b,
                        duration_ticks: args.duration_ticks,
                        sample_period: args.sample_period,
                        drift_reset_ticks: args.drift_reset_ticks,
                        arm_a: m,
                        arm_b: None,
                        drift_diag: None,
                    }
                }
                "scarce_mut0" => {
                    let t0 = Instant::now();
                    let run = run_manifest_seed::<LiveExperimentAdapter>(
                        &manifests.scarce_mut0,
                        &registry,
                        seed,
                        None,
                        None,
                    );
                    let m = summarize_blind_arm(
                        "scarce_mut0",
                        seed,
                        args.duration_ticks,
                        true,
                        t0.elapsed().as_secs_f64(),
                        &run,
                    )?;
                    JobCheckpoint {
                        cell_id: manifests.cell_id.clone(),
                        seed,
                        job_kind: kind.to_string(),
                        manifest_fingerprint_a: fp_a,
                        manifest_fingerprint_b: fp_b,
                        duration_ticks: args.duration_ticks,
                        sample_period: args.sample_period,
                        drift_reset_ticks: args.drift_reset_ticks,
                        arm_a: m,
                        arm_b: None,
                        drift_diag: None,
                    }
                }
                _ => {
                    let t0 = Instant::now();
                    let pair = run_live_drift_pair(
                        &sel_for_drift,
                        &manifests.scarce_drift,
                        &registry,
                        seed,
                    )
                    .map_err(|e| format!("run_live_drift_pair failed (seed {seed}): {e}"))?;
                    let half_secs = t0.elapsed().as_secs_f64() * 0.5;
                    let sel_m = summarize_blind_arm(
                        "selected",
                        seed,
                        args.duration_ticks,
                        true,
                        half_secs,
                        &pair.selected,
                    )?;
                    let drf_m = summarize_blind_arm(
                        "scarce_drift",
                        seed,
                        args.duration_ticks,
                        false,
                        half_secs,
                        &pair.drift,
                    )?;
                    let diag = summarize_drift_diag(&pair);
                    JobCheckpoint {
                        cell_id: manifests.cell_id.clone(),
                        seed,
                        job_kind: kind.to_string(),
                        manifest_fingerprint_a: fp_a,
                        manifest_fingerprint_b: fp_b,
                        duration_ticks: args.duration_ticks,
                        sample_period: args.sample_period,
                        drift_reset_ticks: args.drift_reset_ticks,
                        arm_a: sel_m,
                        arm_b: Some(drf_m),
                        drift_diag: Some(diag),
                    }
                }
            };
            write_atomic_json(&ckpt_path, &ckpt)?;
            Ok(ckpt)
        })
        .collect();

    let mut base_map = BTreeMap::new();
    let mut mut0_map = BTreeMap::new();
    let mut pair_map = BTreeMap::new();
    for res in results {
        let item = res?;
        match item.job_kind.as_str() {
            "baseline_ref" => {
                base_map.insert(item.seed, item.arm_a);
            }
            "scarce_mut0" => {
                mut0_map.insert(item.seed, item.arm_a);
            }
            _ => {
                let b = item
                    .arm_b
                    .ok_or_else(|| "missing scarce_drift in drift_pair".to_string())?;
                let d = item
                    .drift_diag
                    .ok_or_else(|| "missing drift_diag in drift_pair".to_string())?;
                pair_map.insert(item.seed, (item.arm_a, b, d));
            }
        }
    }

    let CellEvaluationOutcome {
        pooled_baseline,
        aggregate,
        per_seed,
        world_identities_observed: wid_vec,
        all_terrain_generated,
    } = evaluate_cell_outcomes(
        &base_map,
        &mut0_map,
        &pair_map,
        &args.seeds,
        args.min_surviving_baseline,
    )?;

    let git_commit = cmd_stdout("git", &["rev-parse", "HEAD"]);
    let git_dirty = cmd_stdout("git", &["status", "--porcelain"]).map(|s| !s.trim().is_empty());
    let rustc_version = cmd_stdout("rustc", &["--version"]);
    let exe_path = std::env::current_exe().ok();
    let exe_sha = exe_path.as_ref().and_then(|p| sha256_file(p).ok());

    let report = Viab2CellReport {
        suite: "sel2-p2-viab2".into(),
        cell_id: manifests.cell_id.clone(),
        mass_observables_excluded: true,
        blind_mode: "survival_and_gate_metrics_only".into(),
        layout: args.layout,
        duration_ticks: args.duration_ticks,
        sample_period: args.sample_period,
        drift_reset_ticks: args.drift_reset_ticks,
        trunk_sha: git_commit.clone(),
        binary_sha256: exe_sha.clone(),
        elapsed_secs: started.elapsed().as_secs_f64(),
        seeds_evaluated: args.seeds.clone(),
        world_identities_observed: wid_vec,
        all_terrain_generated,
        pooled_baseline,
        aggregate: aggregate.clone(),
        per_seed,
    };

    let cell_json_name = format!("{}.json", manifests.cell_id);
    write_atomic_json(&out_dir.join(&cell_json_name), &report)?;

    // Rebuild summary.csv, provenance.json, and checksums.sha256 across all viab2_*.json in out_dir.
    let mut cell_files: Vec<PathBuf> = fs::read_dir(out_dir)
        .map_err(|e| format!("read_dir {}: {e}", out_dir.display()))?
        .filter_map(|e| e.ok().map(|x| x.path()))
        .filter(|p| {
            p.extension().and_then(|s| s.to_str()) == Some("json")
                && p.file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|n| n.starts_with("viab2_"))
        })
        .collect();
    cell_files.sort();

    let mut csv = String::from(
        "cell_id,layout,seed,arm,completed,survived,extinct_tick,min_n,mean_n,final_n,\
         crop_second_half,g_ratio,generations_proxy,starvation_share,c1_min5,c2_pass,c3_pass,\
         c4_pass,c5_pass,drift_valid,drift_has_shortfall,max_abs_eu_drift,final_checksum\n",
    );
    let mut prov_cells = Vec::new();
    for cf in &cell_files {
        let raw = fs::read_to_string(cf).map_err(|e| format!("read {}: {e}", cf.display()))?;
        let parsed: Viab2CellReport =
            serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", cf.display()))?;
        let fname = cf
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| "bad cell filename".to_string())?
            .to_string();
        let sha = sha256_file(cf).map_err(|e| format!("hash {fname}: {e}"))?;
        prov_cells.push(ProvenanceCellEntry {
            cell_id: parsed.cell_id.clone(),
            file: fname,
            sha256: sha,
            layout: parsed.layout,
            d1a_survival_pass: parsed.aggregate.d1a_survival_pass,
            d2b_escalation_recommended: parsed.aggregate.d2b_escalation_recommended,
        });

        for row in &parsed.per_seed {
            let arms: [(&ArmMetrics, Option<f64>, bool, bool); 4] = [
                (&row.baseline_ref, None, false, false),
                (
                    &row.selected,
                    row.g_selected,
                    row.c1_selected_min5,
                    row.c2_selected,
                ),
                (
                    &row.scarce_mut0,
                    row.g_scarce_mut0,
                    row.c1_mut0_min5,
                    row.c2_mut0,
                ),
                (
                    &row.scarce_drift,
                    row.g_scarce_drift,
                    row.scarce_drift.min_n >= C1_MIN_AGENTS && row.scarce_drift.survived,
                    false,
                ),
            ];
            for (arm_m, g_opt, c1_ok, c2_ok) in arms {
                let is_scarce_non_drift = arm_m.arm == "selected" || arm_m.arm == "scarce_mut0";
                let is_drift = arm_m.arm == "scarce_drift";
                csv.push_str(&format!(
                    "{},{},{},{},{},{},{},{:.0},{:.3},{:.0},{:.6},{},{},{},{},{},{},{},{},{},{},{:.6e},{}\n",
                    parsed.cell_id,
                    parsed.layout,
                    row.seed,
                    arm_m.arm,
                    arm_m.completed,
                    arm_m.survived,
                    opt_u64(arm_m.extinct_tick),
                    arm_m.min_n,
                    arm_m.mean_n,
                    arm_m.final_n,
                    arm_m.crop_second_half,
                    opt_f64(g_opt),
                    opt_f64(arm_m.generations_proxy),
                    opt_f64(arm_m.starvation_share),
                    c1_ok,
                    c2_ok,
                    if is_scarce_non_drift { row.c3.to_string() } else { String::new() },
                    if is_scarce_non_drift { row.c4.to_string() } else { String::new() },
                    if is_scarce_non_drift { row.c5.to_string() } else { String::new() },
                    if is_drift { row.drift_valid.to_string() } else { String::new() },
                    if is_drift { row.drift_diagnostics.has_shortfall.to_string() } else { String::new() },
                    arm_m.max_abs_eu_drift,
                    arm_m.final_checksum,
                ));
            }
        }
    }
    if contains_forbidden_mass_token(&csv) {
        return Err("blindness violation in summary.csv".into());
    }
    fs::write(out_dir.join("summary.csv"), csv.as_bytes())
        .map_err(|e| format!("write summary.csv: {e}"))?;

    let prov = Viab2Provenance {
        experiment: "sel2-p2-viab2".into(),
        card: "C5b. SEL2P2-VIAB2".into(),
        mass_observables_excluded: true,
        blind_mode: "survival_and_gate_metrics_only".into(),
        allowed_non_drift_observables: NON_DRIFT_BLIND_OBS
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        allowed_drift_observables: DRIFT_BLIND_OBS.iter().map(|s| (*s).to_string()).collect(),
        model: "core::live_experiment::LiveExperimentAdapter".into(),
        model_version: LIVE_MODEL_VERSION.into(),
        generated_utc: iso8601_utc(unix_seconds(SystemTime::now())),
        command_line: raw_cli.to_vec(),
        git_commit,
        git_tree_dirty: git_dirty,
        rustc_version,
        binary_path: exe_path.map(|p| p.display().to_string()),
        binary_sha256: exe_sha,
        host_os: std::env::consts::OS.into(),
        host_arch: std::env::consts::ARCH.into(),
        preflight_world_artifact_unset: wf_unset,
        preflight_gpu_disabled: gpu_off,
        preflight_temp_artifact_absent: tmp_absent,
        cells: prov_cells,
    };
    write_atomic_json(&out_dir.join("provenance.json"), &prov)?;

    let mut hashed_names: Vec<String> = fs::read_dir(out_dir)
        .map_err(|e| format!("read_dir {}: {e}", out_dir.display()))?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path().is_file() && name != "checksums.sha256" && !name.ends_with(".tmp") {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    hashed_names.sort();
    let mut csum_buf = String::new();
    for name in &hashed_names {
        let h = sha256_file(&out_dir.join(name)).map_err(|e| format!("sha256 {name}: {e}"))?;
        csum_buf.push_str(&format!("{h}  {name}\n"));
    }
    fs::write(out_dir.join("checksums.sha256"), csum_buf.as_bytes())
        .map_err(|e| format!("write checksums.sha256: {e}"))?;

    assert_blind_output_dir(out_dir)?;

    println!(
        "[VIAB2] {} ({:.1}s): surv B={}/{} S={}/{} M={}/{} D={}/{} | d1a_pass={} (bar={}) | minN_short={} c2_short={} d2b_escalate={} | max_eu_drift={:.3e}",
        manifests.cell_id,
        report.elapsed_secs,
        aggregate.baseline_survived,
        aggregate.n_seeds,
        aggregate.selected_survived,
        aggregate.n_seeds,
        aggregate.scarce_mut0_survived,
        aggregate.n_seeds,
        aggregate.scarce_drift_survived,
        aggregate.n_seeds,
        aggregate.d1a_survival_pass,
        aggregate.d1a_survival_bar,
        aggregate.min_n_shortfall,
        aggregate.c2_shortfall,
        aggregate.d2b_escalation_recommended,
        aggregate.max_abs_eu_drift_overall,
    );
    Ok(())
}

pub fn run_self_test() -> Result<(), String> {
    let m0 = build_viab2_manifests(0, &[614_001], 200, 50, 100)?;
    if m0
        .selected
        .observable_ids
        .iter()
        .any(|x| contains_forbidden_mass_token(x))
    {
        return Err("self-test: forbidden observable in selected".into());
    }
    if validate_seed_policy(&[900_001], false, 200_000).is_ok() {
        return Err("self-test: non-VIAB-2 seed should fail".into());
    }
    let tmp = std::env::temp_dir().join(format!("viab2_selftest_{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(|e| format!("create tmp: {e}"))?;
    write_atomic_json(&tmp.join("ok.json"), &serde_json::json!({"ok": true}))?;
    assert_blind_output_dir(&tmp)?;
    fs::write(tmp.join("tamper.json"), b"{\"trait_mass_mean\": 2.5}\n")
        .map_err(|e| format!("write tamper: {e}"))?;
    if assert_blind_output_dir(&tmp).is_ok() {
        let _ = fs::remove_dir_all(&tmp);
        return Err("self-test: tamper negative control must fail".into());
    }
    let _ = fs::remove_dir_all(&tmp);
    println!("run_viab2_probe --self-test PASSED");
    Ok(())
}

fn cmd_stdout(program: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn unix_seconds(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn iso8601_utc(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let sod = unix_secs % 86_400;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        sod / 3_600,
        (sod / 60) % 60,
        sod % 60
    )
}
