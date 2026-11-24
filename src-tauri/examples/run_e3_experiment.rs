//! The E3-X experiment runner (`CARD E3-X-impl`): spatial productivity gradient vs matched
//! uniform scarcity on body-mass dispersion (`live.trait_mass_var`).
//!
//! Follows the frozen E3 preregistration verbatim:
//! - `src-tauri/tests/fixtures/experiments_e3/e3-preregistration.json`
//! - `docs/ai/planning/2026-08-26-experiment-e3-productivity-gradient-body-mass.md`
//!
//! # Build first in `--release`, then execute the binary -- never `cargo run`
//!
//! ```text
//! cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop --example run_e3_experiment
//! src-tauri/target/release/examples/run_e3_experiment --ensemble \
//!   --manifest-dir src-tauri/tests/fixtures/experiments_e3 \
//!   --smoke-dir artifacts/experiments/e3-productivity-gradient-body-mass/host-smoke \
//!   --out artifacts/experiments/e3-productivity-gradient-body-mass/confirmation
//! ```
//!
//! # Modes
//!
//! - `--ensemble`: runs the 12 preregistered seeds in `execution_order` across all three committed
//!   arms (`control_uniform`, `control_uniform_scarcity`, `treatment_gradient`). Release-only.
//! - `--replay`: re-runs one preregistered seed of one committed arm for deterministic checksum
//!   verification. Release-only.
//! - `--self-test`: exercises thresholds, per-contrast exclusions, pre-run refusals, short
//!   disjoint-seed simulation parity, and stop-on-world-identity-mismatch in a temporary folder.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anima_engine_lib::core::experiment::{
    ExperimentManifest, FactorDiff, ObservableRegistry, MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{
    paired_effect, run_manifest_seed, run_paired_ensemble_with_control, PairedEffect, RunResult,
    RunStatus,
};
use anima_engine_lib::core::live_experiment::{LiveExperimentAdapter, LIVE_MODEL_VERSION};
use anima_engine_lib::core::scenario::StateSample;
use anima_engine_lib::core::sha256::{sha256_file, sha256_hex};
use anima_engine_lib::core::world_artifact::WorldIdentity;

const DEFAULT_MANIFEST_DIR: &str = "src-tauri/tests/fixtures/experiments_e3";
const DEFAULT_CONFIRM_OUT: &str =
    "artifacts/experiments/e3-productivity-gradient-body-mass/confirmation";
const PREREG_FILE: &str = "e3-preregistration.json";
const TEMP_ARTIFACT_NAME: &str = "anima_world_artifact.anmw";
const ALLOWED_TREATMENT_DIFF: [&str; 1] = ["interventions"];
const ALLOWED_SCARCITY_DIFF: [&str; 2] = ["initial_conditions", "interventions"];

const ARM_ORDER: [&str; 3] = [
    "control_uniform",
    "control_uniform_scarcity",
    "treatment_gradient",
];

const SERIES_OBSERVABLES: [&str; 8] = [
    "live.agent_count",
    "live.trait_mass_var",
    "live.standing_crop",
    "live.births",
    "live.deaths",
    "live.closed_eu_total",
    "live.shannon",
    "live.archive_coverage",
];

pub type WorldIdentityHook = fn(usize, &str, u64, WorldIdentity) -> WorldIdentity;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Ensemble,
    Replay,
    SelfTest,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Mode::Ensemble => "ensemble",
            Mode::Replay => "replay",
            Mode::SelfTest => "self_test",
        }
    }
}

#[derive(Clone)]
pub struct RuntimeEnv {
    pub anima_world_artifact: Option<String>,
    pub temp_dir: PathBuf,
    pub is_debug_build: bool,
    pub host_os: String,
    pub host_arch: String,
    pub git_commit_override: Option<String>,
    pub git_dirty_paths_override: Option<Vec<String>>,
    pub world_identity_override: Option<WorldIdentityHook>,
}

impl RuntimeEnv {
    pub fn from_process() -> Self {
        Self {
            anima_world_artifact: std::env::var_os("ANIMA_WORLD_ARTIFACT")
                .map(os_string_lossy)
                .filter(|s| !s.is_empty()),
            temp_dir: std::env::temp_dir(),
            is_debug_build: cfg!(debug_assertions),
            host_os: std::env::consts::OS.to_string(),
            host_arch: std::env::consts::ARCH.to_string(),
            git_commit_override: None,
            git_dirty_paths_override: None,
            world_identity_override: None,
        }
    }
}

struct Args {
    mode: Mode,
    manifest_dir: PathBuf,
    out: PathBuf,
    smoke_dir: Option<PathBuf>,
    arm: Option<String>,
    seed: Option<u64>,
}

pub struct ThresholdsSpec {
    pub min_valid_pairs: usize,
    pub dz_min: f64,
    pub relative_delta_min: f64,
    pub ci_excludes_zero_required: bool,
    pub decision_enum: Vec<String>,
}

struct PreregSpec {
    raw: serde_json::Value,
    status: String,
    control_file: String,
    scarcity_file: String,
    treatment_file: String,
    execution_order: Vec<u64>,
    smoke_seed: u64,
    duration_ticks: u64,
    sample_period: u64,
    warmup_ticks: u64,
    tail_window: [u64; 2],
    min_agent_count_at_t: usize,
    smoke_host_observed: String,
    smoke_parent_commit: String,
    smoke_artifact_record: String,
    thresholds: ThresholdsSpec,
}

#[derive(Clone)]
struct SmokeVerification {
    dir: PathBuf,
    provenance_sha256: String,
    smoke_report_sha256: String,
    smoke_git_commit: Option<String>,
    smoke_host_os: Option<String>,
    smoke_host_arch: Option<String>,
    smoke_profile: Option<String>,
    smoke_world_identity: Option<WorldIdentity>,
    wp2_passed: bool,
}

#[derive(Clone)]
struct ArmRunRecord {
    seed: u64,
    arm: String,
    result: RunResult,
    terrain_source: String,
    world_identity: WorldIdentity,
    elapsed_secs: f64,
}

struct PartialStopInput<'a> {
    mode: Mode,
    out_dir: &'a Path,
    env: &'a RuntimeEnv,
    git_commit: &'a Option<String>,
    git_describe: &'a Option<String>,
    git_dirty_paths: &'a [String],
    prerun_check: &'a serde_json::Value,
    smoke_ref: Option<&'a SmokeVerification>,
    runs: &'a [ArmRunRecord],
    stop_reason: &'a str,
}

struct WriteEnsembleInput<'a> {
    out_dir: &'a Path,
    manifest_dir: &'a Path,
    prereg: &'a PreregSpec,
    manifests: &'a [(&'static str, &'a str, &'a ExperimentManifest); 3],
    manifest_hashes: &'a BTreeMap<String, String>,
    registry: &'a ObservableRegistry,
    env: &'a RuntimeEnv,
    git_commit: &'a Option<String>,
    git_describe: &'a Option<String>,
    git_dirty_paths: &'a [String],
    prerun_check: &'a serde_json::Value,
    smoke_ref: Option<&'a SmokeVerification>,
    runs: &'a [ArmRunRecord],
    expected_world_identity: WorldIdentity,
    analysis: &'a EnsembleAnalysis,
    started: SystemTime,
    ended: SystemTime,
    total_elapsed_secs: f64,
}

struct ContrastSpec {
    name: &'static str,
    control_arm: &'static str,
    treatment_arm: &'static str,
}

const CONTRASTS: [ContrastSpec; 3] = [
    ContrastSpec {
        name: "treatment_gradient - control_uniform_scarcity",
        control_arm: "control_uniform_scarcity",
        treatment_arm: "treatment_gradient",
    },
    ContrastSpec {
        name: "treatment_gradient - control_uniform",
        control_arm: "control_uniform",
        treatment_arm: "treatment_gradient",
    },
    ContrastSpec {
        name: "control_uniform_scarcity - control_uniform",
        control_arm: "control_uniform",
        treatment_arm: "control_uniform_scarcity",
    },
];

struct ExcludedSeedRecord {
    seed: u64,
    arm: String,
    contrast: String,
    detail: String,
    tick: u64,
}

pub struct SeedDeltaRow {
    pub seed: u64,
    pub control: f64,
    pub treatment: f64,
    pub delta: f64,
}

pub struct ObservableContrastReport {
    pub effect: PairedEffect,
    pub n_extinct_pairs_dropped: usize,
    pub n_singleton_pairs_dropped: usize,
    pub n_empty_half_pairs_dropped_s3: usize,
    pub median_of_per_seed_deltas: Option<f64>,
    pub between_seed_variance_of_deltas: Option<f64>,
    pub per_seed_delta_table: Vec<SeedDeltaRow>,
    pub decision: Option<String>,
    pub zero_control_mean_fallback_used: bool,
}

struct ContrastAnalysis {
    name: String,
    control_arm: String,
    treatment_arm: String,
    h1: ObservableContrastReport,
    h1_bessel_sensitivity: ObservableContrastReport,
    h1_tail_window: ObservableContrastReport,
    s1: ObservableContrastReport,
    s2: ObservableContrastReport,
    s3: ObservableContrastReport,
    s4: ObservableContrastReport,
    s5: ObservableContrastReport,
    s6_mean_lifespan: ObservableContrastReport,
    s6_births_sum: ObservableContrastReport,
    s6_deaths_sum: ObservableContrastReport,
    i1_closed_eu_total: ObservableContrastReport,
    m1_standing_crop_final: ObservableContrastReport,
    m1_standing_crop_post_warmup: ObservableContrastReport,
}

struct AdditiveDecompositionRow {
    seed: u64,
    control_uniform_var: f64,
    control_uniform_scarcity_var: f64,
    treatment_gradient_var: f64,
    delta_t_minus_u: f64,
    delta_t_minus_s: f64,
    delta_s_minus_u: f64,
    residual: f64,
}

struct FailedRunRecord {
    seed: u64,
    arm: String,
    tick: u64,
    reason: String,
}

struct ExtinctionRecord {
    seed: u64,
    arm: String,
    first_extinct_tick: u64,
}

struct SingletonRecord {
    seed: u64,
    arm: String,
    tick: u64,
    agent_count: f64,
    scope: String,
}

struct EmptyHalfRecord {
    seed: u64,
    arm: String,
    tick: u64,
    empty_half: String,
}

struct EnsembleAnalysis {
    contrasts: Vec<ContrastAnalysis>,
    agent_count_at_t_by_arm: BTreeMap<String, Vec<(u64, f64)>>,
    additive_decomposition_table: Vec<AdditiveDecompositionRow>,
    extinction_table_by_arm: Vec<ExtinctionRecord>,
    singleton_table_by_arm: Vec<SingletonRecord>,
    empty_half_table_by_arm: Vec<EmptyHalfRecord>,
    per_arm_empty_half_counts: BTreeMap<String, usize>,
    failed_runs: Vec<FailedRunRecord>,
    excluded_by_contrast_log: Vec<ExcludedSeedRecord>,
}

fn os_string_lossy(s: OsString) -> String {
    s.to_string_lossy().into_owned()
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() {
        eprintln!(
            "usage: run_e3_experiment (--ensemble | --replay --arm <arm> --seed <seed> | --self-test) [--manifest-dir <dir>] [--out <dir>] [--smoke-dir <dir>]"
        );
        std::process::exit(2);
    }
    if let Err(err) = run_with_args(argv) {
        eprintln!("E3-X runner error: {err}");
        std::process::exit(1);
    }
}

pub fn run_with_args(argv: Vec<String>) -> Result<(), String> {
    run_with_config(argv, RuntimeEnv::from_process())
}

pub fn run_with_config(argv: Vec<String>, env: RuntimeEnv) -> Result<(), String> {
    let args = parse_args(argv)?;
    match args.mode {
        Mode::SelfTest => run_self_test(&env),
        Mode::Replay => run_replay(&args, &env),
        Mode::Ensemble => run_ensemble(&args, &env),
    }
}

fn resolve_default_manifest_dir() -> PathBuf {
    let direct = Path::new(DEFAULT_MANIFEST_DIR);
    if direct.join(PREREG_FILE).exists() {
        return direct.to_path_buf();
    }
    let from_crate = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/experiments_e3");
    if from_crate.join(PREREG_FILE).exists() {
        return from_crate;
    }
    direct.to_path_buf()
}

fn parse_args(argv: Vec<String>) -> Result<Args, String> {
    let mut mode: Option<Mode> = None;
    let mut manifest_dir = resolve_default_manifest_dir();
    let mut out = PathBuf::from(DEFAULT_CONFIRM_OUT);
    let mut smoke_dir: Option<PathBuf> = None;
    let mut arm: Option<String> = None;
    let mut seed: Option<u64> = None;

    let mut it = argv.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--ensemble" => set_mode(&mut mode, Mode::Ensemble)?,
            "--replay" => set_mode(&mut mode, Mode::Replay)?,
            "--self-test" => set_mode(&mut mode, Mode::SelfTest)?,
            "--manifest-dir" => {
                manifest_dir = PathBuf::from(need_val(&mut it, "--manifest-dir")?);
            }
            "--out" => {
                out = PathBuf::from(need_val(&mut it, "--out")?);
            }
            "--smoke-dir" => {
                smoke_dir = Some(PathBuf::from(need_val(&mut it, "--smoke-dir")?));
            }
            "--arm" => {
                arm = Some(need_val(&mut it, "--arm")?);
            }
            "--seed" => {
                let raw = need_val(&mut it, "--seed")?;
                let parsed: u64 = raw
                    .parse()
                    .map_err(|_| format!("--seed must be a positive integer, got '{raw}'"))?;
                seed = Some(parsed);
            }
            other => return Err(format!("unknown flag '{other}'")),
        }
    }

    let mode = mode.ok_or("must pass exactly one mode: --ensemble, --replay, or --self-test")?;
    match mode {
        Mode::Replay => {
            if arm.is_none() || seed.is_none() {
                return Err("--replay requires both --arm <arm> and --seed <u64>".into());
            }
        }
        Mode::Ensemble | Mode::SelfTest => {
            if arm.is_some() || seed.is_some() {
                return Err("--arm and --seed are only valid in --replay mode".into());
            }
        }
    }

    Ok(Args {
        mode,
        manifest_dir,
        out,
        smoke_dir,
        arm,
        seed,
    })
}

fn set_mode(slot: &mut Option<Mode>, next: Mode) -> Result<(), String> {
    if let Some(prev) = slot {
        return Err(format!(
            "conflicting mode flags: --{} and --{}",
            prev.as_str(),
            next.as_str()
        ));
    }
    *slot = Some(next);
    Ok(())
}

fn need_val(it: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    it.next()
        .ok_or_else(|| format!("{flag} requires a value argument"))
}

// ---- Pre-run gates -----------------------------------------------------------------------------

fn refuse_debug(env: &RuntimeEnv, mode: Mode) -> Result<(), String> {
    if env.is_debug_build {
        return Err(format!(
            "refuse --{}: E3-X confirmation requires a --release build (debug_assertions = true)",
            mode.as_str()
        ));
    }
    Ok(())
}

fn ensure_empty_out_dir(out_dir: &Path) -> Result<(), String> {
    if out_dir.exists() {
        let mut entries = std::fs::read_dir(out_dir)
            .map_err(|e| format!("read output dir {}: {e}", out_dir.display()))?;
        if entries.next().is_some() {
            return Err(format!(
                "refuse: output directory {} is not empty",
                out_dir.display()
            ));
        }
    }
    Ok(())
}

fn check_world_artifact_prerun(env: &RuntimeEnv) -> Result<serde_json::Value, String> {
    if let Some(val) = &env.anima_world_artifact {
        return Err(format!(
            "refuse: ANIMA_WORLD_ARTIFACT is set ('{val}'); E3-X requires it unset before starting"
        ));
    }
    let temp_artifact = env.temp_dir.join(TEMP_ARTIFACT_NAME);
    if temp_artifact.exists() {
        return Err(format!(
            "refuse: desktop world artifact {} exists; E3-X requires it absent before starting",
            temp_artifact.display()
        ));
    }
    Ok(serde_json::json!({
        "anima_world_artifact_unset": true,
        "temp_artifact_filename": TEMP_ARTIFACT_NAME,
        "temp_artifact_path": temp_artifact.display().to_string().replace('\\', "/"),
        "temp_artifact_absent": true,
    }))
}

// ---- Preregistration & Manifest loading --------------------------------------------------------

fn load_prereg(manifest_dir: &Path) -> Result<PreregSpec, String> {
    let path = manifest_dir.join(PREREG_FILE);
    let raw = read_json(&path)?;

    let status = raw["status"]
        .as_str()
        .ok_or("preregistration missing status")?
        .to_string();
    if status != "FROZEN" {
        return Err(format!(
            "refuse: preregistration status must be FROZEN, got '{status}'"
        ));
    }

    let control_file = raw["manifests"]["control"]
        .as_str()
        .ok_or("preregistration missing manifests.control")?
        .to_string();
    let scarcity_file = raw["manifests"]["control_uniform_scarcity"]
        .as_str()
        .ok_or("preregistration missing manifests.control_uniform_scarcity")?
        .to_string();
    let treatment_file = raw["manifests"]["treatment"]
        .as_str()
        .ok_or("preregistration missing manifests.treatment")?
        .to_string();

    let execution_order = raw["seeds"]["execution_order"]
        .as_array()
        .ok_or("preregistration missing seeds.execution_order")?
        .iter()
        .map(|v| v.as_u64().ok_or("non-u64 entry in seeds.execution_order"))
        .collect::<Result<Vec<_>, _>>()?;
    if execution_order.is_empty() {
        return Err("preregistration seeds.execution_order is empty".into());
    }
    let smoke_seed = raw["seeds"]["smoke_seed"]
        .as_u64()
        .ok_or("preregistration missing seeds.smoke_seed")?;

    let duration_ticks = raw["duration"]["duration_ticks"]
        .as_u64()
        .ok_or("preregistration missing duration.duration_ticks")?;
    let sample_period = raw["duration"]["sample_period"]
        .as_u64()
        .ok_or("preregistration missing duration.sample_period")?;
    let warmup_ticks = raw["duration"]["warmup_ticks"]
        .as_u64()
        .ok_or("preregistration missing duration.warmup_ticks")?;
    let tail_arr = raw["duration"]["tail_window_ticks"]
        .as_array()
        .ok_or("preregistration missing duration.tail_window_ticks")?;
    if tail_arr.len() != 2 {
        return Err("preregistration tail_window_ticks must have length 2".into());
    }
    let tail_window = [
        tail_arr[0]
            .as_u64()
            .ok_or("tail_window_ticks[0] must be u64")?,
        tail_arr[1]
            .as_u64()
            .ok_or("tail_window_ticks[1] must be u64")?,
    ];
    let min_agent_count_at_t = raw["hypotheses"]
        .as_array()
        .and_then(|hs| hs.iter().find(|h| h["id"] == "H1"))
        .and_then(|h| h["requires_min_agent_count_at_T"].as_u64())
        .ok_or("preregistration H1 missing requires_min_agent_count_at_T")?
        as usize;

    let smoke_host_observed = raw["execution_host_rule"]["smoke_host_observed"]
        .as_str()
        .ok_or("preregistration missing execution_host_rule.smoke_host_observed")?
        .to_string();
    let smoke_parent_commit = raw["wp2_smoke_calibration"]["parent_commit"]
        .as_str()
        .ok_or("preregistration missing wp2_smoke_calibration.parent_commit")?
        .to_string();
    let smoke_artifact_record = raw["wp2_smoke_calibration"]["artifact_record"]
        .as_str()
        .unwrap_or("artifacts/experiments/e3-productivity-gradient-body-mass/smoke/")
        .to_string();

    let t = &raw["thresholds"];
    let min_valid_pairs = t["min_valid_pairs_for_decision"]
        .as_u64()
        .ok_or("thresholds missing min_valid_pairs_for_decision")?
        as usize;
    let dz_min = t["dz_min"].as_f64().ok_or("thresholds missing dz_min")?;
    let relative_delta_min = t["relative_delta_min"]
        .as_f64()
        .ok_or("thresholds missing relative_delta_min")?;
    let ci_excludes_zero_required = t["ci_excludes_zero_required"]
        .as_bool()
        .ok_or("thresholds missing ci_excludes_zero_required")?;
    let decision_enum = t["decision_enum"]
        .as_array()
        .ok_or("thresholds missing decision_enum")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or("non-string entry in thresholds.decision_enum")
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(PreregSpec {
        raw,
        status,
        control_file,
        scarcity_file,
        treatment_file,
        execution_order,
        smoke_seed,
        duration_ticks,
        sample_period,
        warmup_ticks,
        tail_window,
        min_agent_count_at_t,
        smoke_host_observed,
        smoke_parent_commit,
        smoke_artifact_record,
        thresholds: ThresholdsSpec {
            min_valid_pairs,
            dz_min,
            relative_delta_min,
            ci_excludes_zero_required,
            decision_enum,
        },
    })
}

fn load_and_validate_manifests(
    manifest_dir: &Path,
    prereg: &PreregSpec,
    registry: &ObservableRegistry,
) -> Result<(ExperimentManifest, ExperimentManifest, ExperimentManifest), String> {
    registry
        .validate()
        .map_err(|e| format!("invalid live registry: {e}"))?;

    let ctrl = load_manifest(&manifest_dir.join(&prereg.control_file))?;
    let scarce = load_manifest(&manifest_dir.join(&prereg.scarcity_file))?;
    let treat = load_manifest(&manifest_dir.join(&prereg.treatment_file))?;

    for (label, m) in [
        ("control_uniform", &ctrl),
        ("control_uniform_scarcity", &scarce),
        ("treatment_gradient", &treat),
    ] {
        if m.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(format!(
                "{label}: unexpected schema_version {}",
                m.schema_version
            ));
        }
        m.validate(registry)
            .map_err(|e| format!("{label} validation failed: {e}"))?;
        if m.seeds != prereg.execution_order {
            return Err(format!(
                "{label}: seeds {:?} disagree with preregistration execution_order {:?}",
                m.seeds, prereg.execution_order
            ));
        }
        if m.seeds.contains(&prereg.smoke_seed) {
            return Err(format!(
                "{label}: declared seeds must exclude smoke_seed {}",
                prereg.smoke_seed
            ));
        }
        if m.duration_ticks != prereg.duration_ticks {
            return Err(format!(
                "{label}: duration_ticks {} != preregistration {}",
                m.duration_ticks, prereg.duration_ticks
            ));
        }
        if m.sample_period != prereg.sample_period {
            return Err(format!(
                "{label}: sample_period {} != preregistration {}",
                m.sample_period, prereg.sample_period
            ));
        }
    }

    FactorDiff {
        allowed_paths: ALLOWED_TREATMENT_DIFF
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    }
    .validate(&ctrl, &treat)
    .map_err(|e| format!("FactorDiff(control_uniform, treatment_gradient) failed: {e}"))?;

    let scarcity_diff = FactorDiff {
        allowed_paths: ALLOWED_SCARCITY_DIFF
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    };
    scarcity_diff.validate(&ctrl, &scarce).map_err(|e| {
        format!("FactorDiff(control_uniform, control_uniform_scarcity) failed: {e}")
    })?;
    scarcity_diff.validate(&scarce, &treat).map_err(|e| {
        format!("FactorDiff(control_uniform_scarcity, treatment_gradient) failed: {e}")
    })?;

    Ok((ctrl, scarce, treat))
}

// ---- Smoke record validation -------------------------------------------------------------------

fn resolve_smoke_dir(
    smoke_arg: Option<&Path>,
    out_dir: &Path,
    default_record: &str,
) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(p) = smoke_arg {
        candidates.push(p.to_path_buf());
        candidates.push(p.join("smoke"));
    } else {
        if let Some(parent) = out_dir.parent() {
            candidates.push(parent.join("host-smoke"));
            candidates.push(parent.join("host-smoke").join("smoke"));
            candidates.push(parent.join("smoke"));
        }
        candidates.push(PathBuf::from(default_record));
    }
    candidates.into_iter().find(|cand| {
        cand.join("provenance.json").exists() && cand.join("smoke-report.json").exists()
    })
}

fn is_lower_hex_64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn verify_smoke_checksums(dir: &Path) -> Result<(), String> {
    let checksum_path = dir.join("checksums.sha256");
    if !checksum_path.exists() {
        return Err(format!(
            "refuse: smoke directory {} is missing checksums.sha256",
            dir.display()
        ));
    }
    let raw = std::fs::read_to_string(&checksum_path)
        .map_err(|e| format!("read {}: {e}", checksum_path.display()))?;
    let mut covered: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut line_count = 0usize;
    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
        line_count += 1;
        let Some((hex, rel)) = line.split_once("  ") else {
            return Err(format!(
                "refuse: malformed line in {}: '{line}'",
                checksum_path.display()
            ));
        };
        if !is_lower_hex_64(hex) || rel.is_empty() {
            return Err(format!(
                "refuse: malformed digest/path in {}: '{line}'",
                checksum_path.display()
            ));
        }
        let target = dir.join(rel);
        if !target.exists() {
            return Err(format!(
                "refuse: {} references missing smoke file '{rel}'",
                checksum_path.display()
            ));
        }
        let actual = sha256_file(&target)
            .map_err(|e| format!("hash smoke file {}: {e}", target.display()))?;
        if actual != hex {
            return Err(format!(
                "refuse: smoke checksum mismatch for {rel}: expected {hex}, got {actual}"
            ));
        }
        covered.insert(rel.to_string());
    }
    if line_count == 0 {
        return Err(format!(
            "refuse: smoke checksum file {} is empty",
            checksum_path.display()
        ));
    }
    for req in ["provenance.json", "smoke-report.json"] {
        if !covered.contains(req) {
            return Err(format!(
                "refuse: {} does not cover required smoke file '{req}'",
                checksum_path.display()
            ));
        }
    }
    Ok(())
}

fn validate_smoke_dir(
    dir: &Path,
    env: &RuntimeEnv,
    current_commit: Option<&str>,
    require_exact_commit_and_host: bool,
) -> Result<SmokeVerification, String> {
    let prov_path = dir.join("provenance.json");
    let report_path = dir.join("smoke-report.json");
    if !prov_path.exists() || !report_path.exists() {
        return Err(format!(
            "smoke directory {} must contain provenance.json and smoke-report.json",
            dir.display()
        ));
    }
    verify_smoke_checksums(dir)?;
    let prov = read_json(&prov_path)?;
    let report = read_json(&report_path)?;

    let smoke_host_os = prov["host"]["os"].as_str().map(str::to_string);
    let smoke_host_arch = prov["host"]["arch"].as_str().map(str::to_string);
    let smoke_profile = prov["build"]["profile"].as_str().map(str::to_string);
    let smoke_git_commit = prov["build"]["git_commit"].as_str().map(str::to_string);

    let wp2_a = report["wp2_a_resource_field"]["passed"].as_bool() == Some(true);
    let wp2_b = report["wp2_b_standing_crop_intervention"]["passed"].as_bool() == Some(true);
    let wp2_c = report["wp2_c_detritus_before_3000"]["passed"].as_bool() == Some(true);
    let finite_ok = report["all_observables_emitted_and_finite"].as_bool() == Some(true);
    let wid_ok = report["integrity"]["world_identity_identical"].as_bool() == Some(true);
    let budget_ok = report["wp3_schedule_lock"]["within_budget"].as_bool() == Some(true);
    let wp2_passed = wp2_a && wp2_b && wp2_c && finite_ok && wid_ok && budget_ok;

    if require_exact_commit_and_host {
        if smoke_host_os.as_deref() != Some(env.host_os.as_str())
            || smoke_host_arch.as_deref() != Some(env.host_arch.as_str())
        {
            return Err(format!(
                "refuse: smoke record {} host ({:?}/{:?}) != current host ({}/{})",
                dir.display(),
                smoke_host_os,
                smoke_host_arch,
                env.host_os,
                env.host_arch
            ));
        }
        if smoke_profile.as_deref() != Some("release") {
            return Err(format!(
                "refuse: smoke record {} build.profile must be 'release', got {:?}",
                dir.display(),
                smoke_profile
            ));
        }
        let expected_commit = current_commit
            .filter(|s| !s.is_empty())
            .ok_or("refuse: cannot verify smoke commit because current git HEAD is unknown")?;
        if smoke_git_commit.as_deref() != Some(expected_commit) {
            return Err(format!(
                "refuse: smoke record {} git_commit ({:?}) != current HEAD ({})",
                dir.display(),
                smoke_git_commit,
                expected_commit
            ));
        }
    }
    if !wp2_passed {
        return Err(format!(
            "refuse: smoke record {} did not pass all WP-2 / integrity / budget gates (a={wp2_a}, b={wp2_b}, c={wp2_c}, finite={finite_ok}, world_identity={wid_ok}, budget={budget_ok})",
            dir.display()
        ));
    }

    let wid_obj = &prov["world_identity_observed"];
    let smoke_world_identity = if wid_obj.is_object() {
        let sim_world_sha256 =
            match wid_obj.get("sim_world_sha256") {
                Some(v) if !v.is_null() => {
                    let hex = v
                        .as_str()
                        .ok_or("smoke world_identity.sim_world_sha256 must be a hex string")?;
                    Some(anima_worldgen::anmw_v3::sha256_from_hex(hex).map_err(|e| {
                        format!("smoke world_identity.sim_world_sha256 invalid: {e}")
                    })?)
                }
                _ => None,
            };
        Some(WorldIdentity {
            seed: wid_obj["seed"]
                .as_u64()
                .ok_or("smoke world_identity.seed missing")? as u32,
            generator_version: wid_obj["generator_version"]
                .as_u64()
                .ok_or("smoke world_identity.generator_version missing")?
                as u32,
            width: wid_obj["width"]
                .as_u64()
                .ok_or("smoke world_identity.width missing")? as u32,
            height: wid_obj["height"]
                .as_u64()
                .ok_or("smoke world_identity.height missing")? as u32,
            checksum: wid_obj["checksum"]
                .as_u64()
                .ok_or("smoke world_identity.checksum missing")? as u32,
            sim_world_sha256,
        })
    } else {
        None
    };

    Ok(SmokeVerification {
        dir: dir.to_path_buf(),
        provenance_sha256: sha256_file(&prov_path).map_err(|e| format!("hash smoke prov: {e}"))?,
        smoke_report_sha256: sha256_file(&report_path)
            .map_err(|e| format!("hash smoke report: {e}"))?,
        smoke_git_commit,
        smoke_host_os,
        smoke_host_arch,
        smoke_profile,
        smoke_world_identity,
        wp2_passed,
    })
}

fn verify_smoke_requirement(
    args: &Args,
    prereg: &PreregSpec,
    env: &RuntimeEnv,
    current_commit: Option<&str>,
) -> Result<Option<SmokeVerification>, String> {
    let current_host = format!("{}/{}", env.host_os, env.host_arch);
    let host_differs = current_host != prereg.smoke_host_observed;
    let commit_differs = current_commit != Some(prereg.smoke_parent_commit.as_str());
    let need_host_smoke = host_differs || commit_differs;

    if let Some(dir) = resolve_smoke_dir(
        args.smoke_dir.as_deref(),
        &args.out,
        &prereg.smoke_artifact_record,
    ) {
        let verified = validate_smoke_dir(&dir, env, current_commit, need_host_smoke)?;
        return Ok(Some(verified));
    }

    if need_host_smoke || args.smoke_dir.is_some() {
        return Err(format!(
            "refuse: missing valid host smoke re-run record for commit {:?} on {} (recorded baseline was {} at {}); run e3_smoke in --release into a separate directory first and pass --smoke-dir",
            current_commit, current_host, prereg.smoke_host_observed, prereg.smoke_parent_commit
        ));
    }

    Ok(None)
}

// ---- Runner modes ------------------------------------------------------------------------------

fn run_replay(args: &Args, env: &RuntimeEnv) -> Result<(), String> {
    refuse_debug(env, Mode::Replay)?;
    ensure_empty_out_dir(&args.out)?;
    let prerun_check = check_world_artifact_prerun(env)?;

    let registry = ObservableRegistry::live_default();
    let prereg = load_prereg(&args.manifest_dir)?;
    let (ctrl, scarce, treat) =
        load_and_validate_manifests(&args.manifest_dir, &prereg, &registry)?;
    let arm_name = args.arm.as_deref().unwrap_or_default();
    let seed = args.seed.ok_or("--replay requires --seed")?;
    let manifest = match arm_name {
        "control_uniform" => &ctrl,
        "control_uniform_scarcity" => &scarce,
        "treatment_gradient" => &treat,
        other => return Err(format!("--arm must be one of {ARM_ORDER:?}, got '{other}'")),
    };
    if !manifest.seeds.contains(&seed) {
        return Err(format!(
            "seed {seed} is not in declared execution_order {:?} for {arm_name}",
            manifest.seeds
        ));
    }

    std::fs::create_dir_all(&args.out)
        .map_err(|e| format!("create output dir {}: {e}", args.out.display()))?;

    let started = SystemTime::now();
    let clock = Instant::now();
    let result = run_manifest_seed::<LiveExperimentAdapter>(manifest, &registry, seed, None, None);
    let elapsed_secs = clock.elapsed().as_secs_f64();
    let ended = SystemTime::now();

    let terrain_source = result
        .provenance
        .terrain_source
        .clone()
        .ok_or("replay result missing provenance.terrain_source")?;
    let mut wid = result
        .provenance
        .world_identity
        .ok_or("replay result missing provenance.world_identity")?;
    if let Some(hook) = env.world_identity_override {
        wid = hook(0, arm_name, seed, wid);
    }

    let git_commit = resolve_git_commit(env);
    let git_dirty_paths = resolve_git_dirty_paths(env);
    let final_map: BTreeMap<String, f64> = result.final_observables.iter().cloned().collect();
    let doc = serde_json::json!({
        "kind": "e3-replay-RECORD",
        "mode": "replay",
        "note": "Deterministic replay check; excluded from confirmation analysis.",
        "arm": arm_name,
        "seed": seed,
        "duration_ticks": prereg.duration_ticks,
        "experiment_id": manifest.experiment_id,
        "manifest_fingerprint": manifest.fingerprint(),
        "status": status_label(&result.status),
        "final_checksum": result.final_checksum,
        "terrain_source": terrain_source,
        "world_identity": wid,
        "world_artifact_prerun_check": prerun_check,
        "build": {
            "git_commit": git_commit,
            "git_dirty_paths": git_dirty_paths,
            "git_tree_dirty": !git_dirty_paths.is_empty(),
            "profile": if env.is_debug_build { "debug" } else { "release" },
            "rustc": tool_version("rustc"),
            "binary_sha256": current_binary_sha256(),
        },
        "host": { "os": env.host_os, "arch": env.host_arch },
        "final_observables": final_map,
        "warnings": result.warnings,
        "model_version": LIVE_MODEL_VERSION,
    });
    let file_name = format!(
        "replay-{arm_name}-seed-{seed}-t{}.json",
        prereg.duration_ticks
    );
    write_text(&args.out.join(&file_name), &(pretty(&doc)? + "\n"))?;
    let timing_doc = serde_json::json!({
        "started_utc": iso8601_utc(unix_seconds(started)),
        "ended_utc": iso8601_utc(unix_seconds(ended)),
        "wall_clock_seconds": elapsed_secs,
    });
    write_text(
        &args.out.join("timing.json"),
        &(pretty(&timing_doc)? + "\n"),
    )?;
    write_checksums(&args.out, &[file_name.as_str()])?;
    println!(
        "E3 replay {arm_name} seed {seed}: checksum {} ({} in {:.2}s) -> {}",
        result.final_checksum,
        status_label(&result.status),
        elapsed_secs,
        args.out.join(&file_name).display()
    );
    Ok(())
}

fn run_ensemble(args: &Args, env: &RuntimeEnv) -> Result<(), String> {
    refuse_debug(env, Mode::Ensemble)?;
    ensure_empty_out_dir(&args.out)?;
    let prerun_check = check_world_artifact_prerun(env)?;

    let registry = ObservableRegistry::live_default();
    let prereg = load_prereg(&args.manifest_dir)?;
    let (ctrl, scarce, treat) =
        load_and_validate_manifests(&args.manifest_dir, &prereg, &registry)?;

    let git_commit = resolve_git_commit(env);
    let git_describe = git(&["describe", "--always", "--dirty"]);
    let git_dirty_paths = resolve_git_dirty_paths(env);
    let smoke_ref = verify_smoke_requirement(args, &prereg, env, git_commit.as_deref())?;

    let mut manifest_hashes: BTreeMap<String, String> = BTreeMap::new();
    for file in [
        PREREG_FILE,
        prereg.control_file.as_str(),
        prereg.scarcity_file.as_str(),
        prereg.treatment_file.as_str(),
    ] {
        let raw_text = std::fs::read_to_string(args.manifest_dir.join(file))
            .map_err(|e| format!("read {file}: {e}"))?;
        let normalized_lf = raw_text.replace("\r\n", "\n");
        manifest_hashes.insert(file.to_string(), sha256_hex(normalized_lf.as_bytes()));
    }

    let manifests_ordered: [(&'static str, &str, &ExperimentManifest); 3] = [
        ("control_uniform", &prereg.control_file, &ctrl),
        ("control_uniform_scarcity", &prereg.scarcity_file, &scarce),
        ("treatment_gradient", &prereg.treatment_file, &treat),
    ];

    let started = SystemTime::now();
    let total_timer = Instant::now();
    let mut records: Vec<ArmRunRecord> =
        Vec::with_capacity(prereg.execution_order.len() * manifests_ordered.len());
    let mut expected_wid: Option<WorldIdentity> =
        smoke_ref.as_ref().and_then(|s| s.smoke_world_identity);

    for &seed in &prereg.execution_order {
        for (arm_name, _, manifest) in &manifests_ordered {
            let run_idx = records.len();
            let timer = Instant::now();
            let result =
                run_manifest_seed::<LiveExperimentAdapter>(manifest, &registry, seed, None, None);
            let elapsed_secs = timer.elapsed().as_secs_f64();

            let terrain_source = match result.provenance.terrain_source.clone() {
                Some(s) => s,
                None => {
                    let reason = format!(
                        "missing_terrain_source at run_index={run_idx} arm={arm_name} seed={seed}"
                    );
                    write_partial_stop(PartialStopInput {
                        mode: args.mode,
                        out_dir: &args.out,
                        env,
                        git_commit: &git_commit,
                        git_describe: &git_describe,
                        git_dirty_paths: &git_dirty_paths,
                        prerun_check: &prerun_check,
                        smoke_ref: smoke_ref.as_ref(),
                        runs: &records,
                        stop_reason: &reason,
                    })?;
                    return Err(reason);
                }
            };
            let mut wid = match result.provenance.world_identity {
                Some(id) => id,
                None => {
                    let reason = format!(
                        "missing_world_identity at run_index={run_idx} arm={arm_name} seed={seed}"
                    );
                    write_partial_stop(PartialStopInput {
                        mode: args.mode,
                        out_dir: &args.out,
                        env,
                        git_commit: &git_commit,
                        git_describe: &git_describe,
                        git_dirty_paths: &git_dirty_paths,
                        prerun_check: &prerun_check,
                        smoke_ref: smoke_ref.as_ref(),
                        runs: &records,
                        stop_reason: &reason,
                    })?;
                    return Err(reason);
                }
            };
            if let Some(hook) = env.world_identity_override {
                wid = hook(run_idx, arm_name, seed, wid);
            }

            records.push(ArmRunRecord {
                seed,
                arm: (*arm_name).to_string(),
                result,
                terrain_source,
                world_identity: wid,
                elapsed_secs,
            });

            match expected_wid {
                None => expected_wid = Some(wid),
                Some(exp) if exp != wid => {
                    let reason = format!(
                        "world_identity_mismatch at run_index={run_idx} arm={arm_name} seed={seed}: expected {:?}, observed {:?}",
                        exp, wid
                    );
                    write_partial_stop(PartialStopInput {
                        mode: args.mode,
                        out_dir: &args.out,
                        env,
                        git_commit: &git_commit,
                        git_describe: &git_describe,
                        git_dirty_paths: &git_dirty_paths,
                        prerun_check: &prerun_check,
                        smoke_ref: smoke_ref.as_ref(),
                        runs: &records,
                        stop_reason: &reason,
                    })?;
                    return Err(reason);
                }
                Some(_) => {}
            }
        }
    }

    let ended = SystemTime::now();
    let total_elapsed_secs = total_timer.elapsed().as_secs_f64();
    let expected_wid = expected_wid.ok_or("ensemble executed zero runs")?;
    let analysis = analyse_ensemble(&prereg, &records)?;

    write_ensemble_artifacts(WriteEnsembleInput {
        out_dir: &args.out,
        manifest_dir: &args.manifest_dir,
        prereg: &prereg,
        manifests: &manifests_ordered,
        manifest_hashes: &manifest_hashes,
        registry: &registry,
        env,
        git_commit: &git_commit,
        git_describe: &git_describe,
        git_dirty_paths: &git_dirty_paths,
        prerun_check: &prerun_check,
        smoke_ref: smoke_ref.as_ref(),
        runs: &records,
        expected_world_identity: expected_wid,
        analysis: &analysis,
        started,
        ended,
        total_elapsed_secs,
    })?;

    println!(
        "E3 ensemble complete: {} runs ({} seeds x 3 arms x {} ticks) in {:.2}s; H1 decision={} -> {}",
        records.len(),
        prereg.execution_order.len(),
        prereg.duration_ticks,
        total_elapsed_secs,
        analysis.contrasts[0].h1.decision.as_deref().unwrap_or("none"),
        args.out.display(),
    );
    Ok(())
}

// ---- Per-contrast exclusions & statistical evaluation ------------------------------------------

fn run_series_obs(series: &[StateSample], tick: u64, obs: &str) -> Option<f64> {
    series
        .iter()
        .find(|s| s.tick == tick)
        .and_then(|s| s.observables.iter().find(|(k, _)| k == obs))
        .map(|(_, v)| *v)
}

fn first_extinct_tick(r: &RunResult, duration_ticks: u64) -> Option<u64> {
    for sample in &r.series {
        if let Some((_, count)) = sample
            .observables
            .iter()
            .find(|(k, _)| k == "live.agent_count")
        {
            if *count == 0.0 {
                return Some(sample.tick);
            }
        }
    }
    if let Some(count) = r.observable("live.agent_count") {
        if count == 0.0 {
            return Some(duration_ticks);
        }
    }
    None
}

fn min_tail_agents(r: &RunResult, tail_window: [u64; 2]) -> Option<(u64, f64)> {
    let [lo, hi] = tail_window;
    let mut min_entry: Option<(u64, f64)> = None;
    for sample in &r.series {
        if sample.tick >= lo && sample.tick <= hi {
            if let Some((_, count)) = sample
                .observables
                .iter()
                .find(|(k, _)| k == "live.agent_count")
            {
                if min_entry.is_none_or(|(_, m)| *count < m) {
                    min_entry = Some((sample.tick, *count));
                }
            }
        }
    }
    if let Some(final_n) = r.observable("live.agent_count") {
        if min_entry.is_none_or(|(_, m)| final_n < m) {
            min_entry = Some((hi, final_n));
        }
    }
    min_entry
}

fn empty_half_label(r: &RunResult) -> Option<&'static str> {
    let w = r.observable("live.trait_mass_mean_west").unwrap_or(0.0);
    let e = r.observable("live.trait_mass_mean_east").unwrap_or(0.0);
    match (w == 0.0, e == 0.0) {
        (true, true) => Some("both"),
        (true, false) => Some("west"),
        (false, true) => Some("east"),
        (false, false) => None,
    }
}

fn tail_window_mean(r: &RunResult, obs: &str, tail_window: [u64; 2]) -> Result<f64, String> {
    let [lo, hi] = tail_window;
    let mut vals = Vec::new();
    for sample in &r.series {
        if sample.tick >= lo && sample.tick <= hi {
            if let Some((_, v)) = sample.observables.iter().find(|(k, _)| k == obs) {
                vals.push(*v);
            }
        }
    }
    if vals.is_empty() {
        return Err(format!(
            "run missing tail-window samples for {obs} in [{lo}, {hi}]"
        ));
    }
    Ok(vals.iter().sum::<f64>() / vals.len() as f64)
}

fn post_warmup_mean(r: &RunResult, obs: &str, warmup_ticks: u64) -> Result<f64, String> {
    let mut vals = Vec::new();
    for sample in &r.series {
        if sample.tick > warmup_ticks {
            if let Some((_, v)) = sample.observables.iter().find(|(k, _)| k == obs) {
                vals.push(*v);
            }
        }
    }
    if vals.is_empty() {
        return Err(format!(
            "run missing post-warmup samples for {obs} (> {warmup_ticks})"
        ));
    }
    Ok(vals.iter().sum::<f64>() / vals.len() as f64)
}

fn cumulative_series_sum(r: &RunResult, obs: &str) -> Result<f64, String> {
    let mut total = 0.0;
    let mut found = false;
    for sample in &r.series {
        if let Some((_, v)) = sample.observables.iter().find(|(k, _)| k == obs) {
            total += *v;
            found = true;
        }
    }
    if !found {
        return Err(format!("run missing series values for {obs}"));
    }
    Ok(total)
}

fn bessel_var(r: &RunResult) -> Result<f64, String> {
    let var = r
        .observable("live.trait_mass_var")
        .ok_or("missing live.trait_mass_var")?;
    let n = r
        .observable("live.agent_count")
        .ok_or("missing live.agent_count")?;
    if n < 2.0 {
        return Err("bessel variance requires agent_count >= 2".into());
    }
    Ok(var * n / (n - 1.0))
}

fn classify_decision(
    effect: &PairedEffect,
    t: &ThresholdsSpec,
    allow_zero_control_fallback: bool,
) -> (String, bool) {
    if effect.n_complete_pairs < t.min_valid_pairs {
        return ("insufficient_evidence".to_string(), false);
    }
    let (Some(lo), Some(hi), Some(delta), Some(ctrl_mean)) = (
        effect.ci95_low,
        effect.ci95_high,
        effect.paired_mean_delta,
        effect.control_mean,
    ) else {
        return ("null".to_string(), false);
    };
    let ci_ok = if t.ci_excludes_zero_required {
        lo > 0.0 || hi < 0.0
    } else {
        true
    };
    if !ci_ok {
        return ("null".to_string(), false);
    }
    let dz_ok = effect.paired_dz.is_some_and(|dz| dz.abs() >= t.dz_min);
    let mut used_fallback = false;
    let rel_ok = if ctrl_mean == 0.0 && allow_zero_control_fallback {
        used_fallback = true;
        delta.abs() > 0.0
    } else {
        delta.abs() >= t.relative_delta_min * ctrl_mean.abs()
    };
    if !dz_ok || !rel_ok {
        return (
            "between_detectable_below_materiality".to_string(),
            used_fallback,
        );
    }
    let label = if delta > 0.0 {
        "material_positive"
    } else {
        "material_negative_opposite"
    };
    (label.to_string(), used_fallback)
}

fn median(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut sorted = xs.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        Some(sorted[mid])
    } else {
        Some(0.5 * (sorted[mid - 1] + sorted[mid]))
    }
}

fn sample_variance(xs: &[f64]) -> Option<f64> {
    if xs.len() < 2 {
        return None;
    }
    let mean = xs.iter().sum::<f64>() / xs.len() as f64;
    let ss: f64 = xs.iter().map(|x| (x - mean) * (x - mean)).sum();
    Some(ss / (xs.len() - 1) as f64)
}

const ZERO_SD_TOL: f64 = 1e-12;

#[allow(dead_code)]
pub fn load_thresholds(manifest_dir: &Path) -> Result<ThresholdsSpec, String> {
    Ok(load_prereg(manifest_dir)?.thresholds)
}

#[allow(clippy::too_many_arguments)]
pub fn build_observable_report(
    name: &str,
    n_requested: usize,
    rows: &[SeedDeltaRow],
    n_extinct_pairs_dropped: usize,
    n_singleton_pairs_dropped: usize,
    n_empty_half_pairs_dropped_s3: usize,
    decided: bool,
    thresholds: &ThresholdsSpec,
    allow_zero_control_fallback: bool,
) -> Result<ObservableContrastReport, String> {
    let cs: Vec<f64> = rows.iter().map(|r| r.control).collect();
    let ts: Vec<f64> = rows.iter().map(|r| r.treatment).collect();
    let ds: Vec<f64> = rows.iter().map(|r| r.delta).collect();
    let mut effect = paired_effect(name, n_requested, &cs, &ts, &ds)
        .map_err(|e| format!("paired_effect({name}): {e}"))?;
    let mut between_var = sample_variance(&ds);
    if let (Some(mean), Some(sd)) = (effect.paired_mean_delta, effect.paired_sd) {
        if sd.abs() < ZERO_SD_TOL * mean.abs().max(1.0) {
            effect.paired_sd = Some(0.0);
            effect.paired_se = Some(0.0);
            effect.ci95_low = Some(mean);
            effect.ci95_high = Some(mean);
            effect.paired_dz = None;
            between_var = Some(0.0);
        }
    }
    let (decision, fallback_used) = if decided {
        let (lbl, fb) = classify_decision(&effect, thresholds, allow_zero_control_fallback);
        (Some(lbl), fb)
    } else {
        (None, false)
    };
    Ok(ObservableContrastReport {
        effect,
        n_extinct_pairs_dropped,
        n_singleton_pairs_dropped,
        n_empty_half_pairs_dropped_s3,
        median_of_per_seed_deltas: median(&ds),
        between_seed_variance_of_deltas: between_var,
        per_seed_delta_table: rows
            .iter()
            .map(|r| SeedDeltaRow {
                seed: r.seed,
                control: r.control,
                treatment: r.treatment,
                delta: r.delta,
            })
            .collect(),
        decision,
        zero_control_mean_fallback_used: fallback_used,
    })
}

fn analyse_ensemble(
    prereg: &PreregSpec,
    records: &[ArmRunRecord],
) -> Result<EnsembleAnalysis, String> {
    let mut by_seed_arm: BTreeMap<(u64, &str), &ArmRunRecord> = BTreeMap::new();
    for rec in records {
        by_seed_arm.insert((rec.seed, rec.arm.as_str()), rec);
    }

    let mut agent_count_at_t_by_arm: BTreeMap<String, Vec<(u64, f64)>> = BTreeMap::new();
    let mut extinction_table_by_arm: Vec<ExtinctionRecord> = Vec::new();
    let mut singleton_table_by_arm: Vec<SingletonRecord> = Vec::new();
    let mut empty_half_table_by_arm: Vec<EmptyHalfRecord> = Vec::new();
    let mut per_arm_empty_half_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut failed_runs: Vec<FailedRunRecord> = Vec::new();
    for arm in ARM_ORDER {
        agent_count_at_t_by_arm.insert(arm.to_string(), Vec::new());
        per_arm_empty_half_counts.insert(arm.to_string(), 0);
    }

    for &seed in &prereg.execution_order {
        for arm in ARM_ORDER {
            let rec = by_seed_arm
                .get(&(seed, arm))
                .ok_or_else(|| format!("missing run record for seed={seed} arm={arm}"))?;
            if let RunStatus::Failed { tick, reason, .. } = &rec.result.status {
                failed_runs.push(FailedRunRecord {
                    seed,
                    arm: arm.to_string(),
                    tick: *tick,
                    reason: reason.clone(),
                });
            }
            let final_n = rec.result.observable("live.agent_count").unwrap_or(0.0);
            agent_count_at_t_by_arm
                .get_mut(arm)
                .expect("initialized arm")
                .push((seed, final_n));

            if let Some(ext_tick) = first_extinct_tick(&rec.result, prereg.duration_ticks) {
                extinction_table_by_arm.push(ExtinctionRecord {
                    seed,
                    arm: arm.to_string(),
                    first_extinct_tick: ext_tick,
                });
            } else {
                if (final_n as usize) < prereg.min_agent_count_at_t {
                    singleton_table_by_arm.push(SingletonRecord {
                        seed,
                        arm: arm.to_string(),
                        tick: prereg.duration_ticks,
                        agent_count: final_n,
                        scope: "final_T".to_string(),
                    });
                } else if let Some((t_tick, tail_min)) =
                    min_tail_agents(&rec.result, prereg.tail_window)
                {
                    if (tail_min as usize) < prereg.min_agent_count_at_t {
                        singleton_table_by_arm.push(SingletonRecord {
                            seed,
                            arm: arm.to_string(),
                            tick: t_tick,
                            agent_count: tail_min,
                            scope: "tail_window".to_string(),
                        });
                    }
                }
                if let Some(half) = empty_half_label(&rec.result) {
                    empty_half_table_by_arm.push(EmptyHalfRecord {
                        seed,
                        arm: arm.to_string(),
                        tick: prereg.duration_ticks,
                        empty_half: half.to_string(),
                    });
                    *per_arm_empty_half_counts.get_mut(arm).expect("arm") += 1;
                }
            }
        }
    }

    let mut excluded_by_contrast_log: Vec<ExcludedSeedRecord> = Vec::new();
    let n_req = prereg.execution_order.len();
    let mut contrasts: Vec<ContrastAnalysis> = Vec::new();

    for c in &CONTRASTS {
        let mut ext_dropped = 0usize;
        let mut single_t_dropped = 0usize;
        let mut single_tail_dropped = 0usize;
        let mut empty_half_dropped = 0usize;

        let mut h1_rows = Vec::new();
        let mut h1_bessel_rows = Vec::new();
        let mut h1_tail_rows = Vec::new();
        let mut s1_rows = Vec::new();
        let mut s2_rows = Vec::new();
        let mut s3_rows = Vec::new();
        let mut s4_rows = Vec::new();
        let mut s5_rows = Vec::new();
        let mut s6_life_rows = Vec::new();
        let mut s6_births_rows = Vec::new();
        let mut s6_deaths_rows = Vec::new();
        let mut i1_rows = Vec::new();
        let mut m1_final_rows = Vec::new();
        let mut m1_pw_rows = Vec::new();

        for &seed in &prereg.execution_order {
            let ctrl = by_seed_arm[&(seed, c.control_arm)];
            let treat = by_seed_arm[&(seed, c.treatment_arm)];

            if !ctrl.result.status.is_completed() || !treat.result.status.is_completed() {
                let fail_arm = if !ctrl.result.status.is_completed() {
                    c.control_arm
                } else {
                    c.treatment_arm
                };
                excluded_by_contrast_log.push(ExcludedSeedRecord {
                    seed,
                    arm: fail_arm.to_string(),
                    contrast: c.name.to_string(),
                    detail: "run_failed".to_string(),
                    tick: prereg.duration_ticks,
                });
                continue;
            }

            // Completed runs: I1, M1, S4, S5, S6 demographic summaries
            let push_pair = |vec: &mut Vec<SeedDeltaRow>, cv: f64, tv: f64| {
                vec.push(SeedDeltaRow {
                    seed,
                    control: cv,
                    treatment: tv,
                    delta: tv - cv,
                });
            };
            push_pair(
                &mut s4_rows,
                ctrl.result.observable("live.shannon").unwrap_or(0.0),
                treat.result.observable("live.shannon").unwrap_or(0.0),
            );
            push_pair(
                &mut s5_rows,
                ctrl.result
                    .observable("live.archive_coverage")
                    .unwrap_or(0.0),
                treat
                    .result
                    .observable("live.archive_coverage")
                    .unwrap_or(0.0),
            );
            push_pair(
                &mut s6_life_rows,
                ctrl.result.observable("live.mean_lifespan").unwrap_or(0.0),
                treat.result.observable("live.mean_lifespan").unwrap_or(0.0),
            );
            push_pair(
                &mut s6_births_rows,
                cumulative_series_sum(&ctrl.result, "live.births")?,
                cumulative_series_sum(&treat.result, "live.births")?,
            );
            push_pair(
                &mut s6_deaths_rows,
                cumulative_series_sum(&ctrl.result, "live.deaths")?,
                cumulative_series_sum(&treat.result, "live.deaths")?,
            );
            push_pair(
                &mut i1_rows,
                ctrl.result
                    .observable("live.closed_eu_total")
                    .unwrap_or(0.0),
                treat
                    .result
                    .observable("live.closed_eu_total")
                    .unwrap_or(0.0),
            );
            push_pair(
                &mut m1_final_rows,
                ctrl.result.observable("live.standing_crop").unwrap_or(0.0),
                treat.result.observable("live.standing_crop").unwrap_or(0.0),
            );
            push_pair(
                &mut m1_pw_rows,
                post_warmup_mean(&ctrl.result, "live.standing_crop", prereg.warmup_ticks)?,
                post_warmup_mean(&treat.result, "live.standing_crop", prereg.warmup_ticks)?,
            );

            // Check extinction for all live.trait_mass_* statistics (H1, H1_tail_window, S1, S2, S3)
            let ext_c = first_extinct_tick(&ctrl.result, prereg.duration_ticks);
            let ext_t = first_extinct_tick(&treat.result, prereg.duration_ticks);
            if ext_c.is_some() || ext_t.is_some() {
                ext_dropped += 1;
                if let Some(tick) = ext_c {
                    excluded_by_contrast_log.push(ExcludedSeedRecord {
                        seed,
                        arm: c.control_arm.to_string(),
                        contrast: c.name.to_string(),
                        detail: "extinction".to_string(),
                        tick,
                    });
                }
                if let Some(tick) = ext_t {
                    excluded_by_contrast_log.push(ExcludedSeedRecord {
                        seed,
                        arm: c.treatment_arm.to_string(),
                        contrast: c.name.to_string(),
                        detail: "extinction".to_string(),
                        tick,
                    });
                }
                continue;
            }

            // S2 (mean mass) needs >= 1 survivor
            push_pair(
                &mut s2_rows,
                ctrl.result
                    .observable("live.trait_mass_mean")
                    .ok_or("missing live.trait_mass_mean")?,
                treat
                    .result
                    .observable("live.trait_mass_mean")
                    .ok_or("missing live.trait_mass_mean")?,
            );

            // H1, H1 Bessel, S1 require agent_count >= min_agent_count_at_T (2) at T
            let c_n = ctrl.result.observable("live.agent_count").unwrap_or(0.0) as usize;
            let t_n = treat.result.observable("live.agent_count").unwrap_or(0.0) as usize;
            if c_n < prereg.min_agent_count_at_t || t_n < prereg.min_agent_count_at_t {
                single_t_dropped += 1;
                single_tail_dropped += 1;
                let s_arm = if c_n < prereg.min_agent_count_at_t {
                    c.control_arm
                } else {
                    c.treatment_arm
                };
                excluded_by_contrast_log.push(ExcludedSeedRecord {
                    seed,
                    arm: s_arm.to_string(),
                    contrast: c.name.to_string(),
                    detail: "singleton_at_T".to_string(),
                    tick: prereg.duration_ticks,
                });
            } else {
                push_pair(
                    &mut h1_rows,
                    ctrl.result
                        .observable("live.trait_mass_var")
                        .ok_or("missing live.trait_mass_var")?,
                    treat
                        .result
                        .observable("live.trait_mass_var")
                        .ok_or("missing live.trait_mass_var")?,
                );
                push_pair(
                    &mut h1_bessel_rows,
                    bessel_var(&ctrl.result)?,
                    bessel_var(&treat.result)?,
                );
                let c_iqr = ctrl.result.observable("live.trait_mass_p90").unwrap_or(0.0)
                    - ctrl.result.observable("live.trait_mass_p10").unwrap_or(0.0);
                let t_iqr = treat
                    .result
                    .observable("live.trait_mass_p90")
                    .unwrap_or(0.0)
                    - treat
                        .result
                        .observable("live.trait_mass_p10")
                        .unwrap_or(0.0);
                push_pair(&mut s1_rows, c_iqr, t_iqr);

                let c_tail_min = min_tail_agents(&ctrl.result, prereg.tail_window);
                let t_tail_min = min_tail_agents(&treat.result, prereg.tail_window);
                let c_tail_ok =
                    c_tail_min.is_none_or(|(_, n)| (n as usize) >= prereg.min_agent_count_at_t);
                let t_tail_ok =
                    t_tail_min.is_none_or(|(_, n)| (n as usize) >= prereg.min_agent_count_at_t);
                if !c_tail_ok || !t_tail_ok {
                    single_tail_dropped += 1;
                    let (bad_arm, tick) = if !c_tail_ok {
                        (
                            c.control_arm,
                            c_tail_min.map(|(t, _)| t).unwrap_or(prereg.duration_ticks),
                        )
                    } else {
                        (
                            c.treatment_arm,
                            t_tail_min.map(|(t, _)| t).unwrap_or(prereg.duration_ticks),
                        )
                    };
                    excluded_by_contrast_log.push(ExcludedSeedRecord {
                        seed,
                        arm: bad_arm.to_string(),
                        contrast: c.name.to_string(),
                        detail: "singleton_in_tail_window".to_string(),
                        tick,
                    });
                } else {
                    push_pair(
                        &mut h1_tail_rows,
                        tail_window_mean(&ctrl.result, "live.trait_mass_var", prereg.tail_window)?,
                        tail_window_mean(&treat.result, "live.trait_mass_var", prereg.tail_window)?,
                    );
                }
            }

            // S3 (east - west) applies empty_half_rule independently of singleton_at_T unless empty half
            let eh_c = empty_half_label(&ctrl.result);
            let eh_t = empty_half_label(&treat.result);
            if eh_c.is_some() || eh_t.is_some() {
                empty_half_dropped += 1;
                if let Some(side) = eh_c {
                    excluded_by_contrast_log.push(ExcludedSeedRecord {
                        seed,
                        arm: c.control_arm.to_string(),
                        contrast: c.name.to_string(),
                        detail: format!("empty_half:{side}"),
                        tick: prereg.duration_ticks,
                    });
                }
                if let Some(side) = eh_t {
                    excluded_by_contrast_log.push(ExcludedSeedRecord {
                        seed,
                        arm: c.treatment_arm.to_string(),
                        contrast: c.name.to_string(),
                        detail: format!("empty_half:{side}"),
                        tick: prereg.duration_ticks,
                    });
                }
            } else {
                let cs3 = ctrl
                    .result
                    .observable("live.trait_mass_mean_east")
                    .unwrap_or(0.0)
                    - ctrl
                        .result
                        .observable("live.trait_mass_mean_west")
                        .unwrap_or(0.0);
                let ts3 = treat
                    .result
                    .observable("live.trait_mass_mean_east")
                    .unwrap_or(0.0)
                    - treat
                        .result
                        .observable("live.trait_mass_mean_west")
                        .unwrap_or(0.0);
                push_pair(&mut s3_rows, cs3, ts3);
            }
        }

        let th = &prereg.thresholds;
        contrasts.push(ContrastAnalysis {
            name: c.name.to_string(),
            control_arm: c.control_arm.to_string(),
            treatment_arm: c.treatment_arm.to_string(),
            h1: build_observable_report(
                "live.trait_mass_var",
                n_req,
                &h1_rows,
                ext_dropped,
                single_t_dropped,
                0,
                true,
                th,
                false,
            )?,
            h1_bessel_sensitivity: build_observable_report(
                "live.trait_mass_var_bessel",
                n_req,
                &h1_bessel_rows,
                ext_dropped,
                single_t_dropped,
                0,
                true,
                th,
                false,
            )?,
            h1_tail_window: build_observable_report(
                "live.trait_mass_var@tail_window",
                n_req,
                &h1_tail_rows,
                ext_dropped,
                single_tail_dropped,
                0,
                true,
                th,
                false,
            )?,
            s1: build_observable_report(
                "live.trait_mass_p90 - live.trait_mass_p10",
                n_req,
                &s1_rows,
                ext_dropped,
                single_t_dropped,
                0,
                true,
                th,
                false,
            )?,
            s2: build_observable_report(
                "live.trait_mass_mean",
                n_req,
                &s2_rows,
                ext_dropped,
                0,
                0,
                true,
                th,
                false,
            )?,
            s3: build_observable_report(
                "live.trait_mass_mean_east - live.trait_mass_mean_west",
                n_req,
                &s3_rows,
                ext_dropped,
                0,
                empty_half_dropped,
                true,
                th,
                true,
            )?,
            s4: build_observable_report(
                "live.shannon",
                n_req,
                &s4_rows,
                0,
                0,
                0,
                false,
                th,
                false,
            )?,
            s5: build_observable_report(
                "live.archive_coverage",
                n_req,
                &s5_rows,
                0,
                0,
                0,
                false,
                th,
                false,
            )?,
            s6_mean_lifespan: build_observable_report(
                "live.mean_lifespan",
                n_req,
                &s6_life_rows,
                0,
                0,
                0,
                true,
                th,
                false,
            )?,
            s6_births_sum: build_observable_report(
                "live.births_sum",
                n_req,
                &s6_births_rows,
                0,
                0,
                0,
                true,
                th,
                false,
            )?,
            s6_deaths_sum: build_observable_report(
                "live.deaths_sum",
                n_req,
                &s6_deaths_rows,
                0,
                0,
                0,
                true,
                th,
                false,
            )?,
            i1_closed_eu_total: build_observable_report(
                "live.closed_eu_total",
                n_req,
                &i1_rows,
                0,
                0,
                0,
                false,
                th,
                false,
            )?,
            m1_standing_crop_final: build_observable_report(
                "live.standing_crop@T",
                n_req,
                &m1_final_rows,
                0,
                0,
                0,
                false,
                th,
                false,
            )?,
            m1_standing_crop_post_warmup: build_observable_report(
                "live.standing_crop@post_warmup_mean",
                n_req,
                &m1_pw_rows,
                0,
                0,
                0,
                false,
                th,
                false,
            )?,
        });
    }

    // Additive decomposition (T - U) == (T - S) + (S - U) on seeds valid for H1 in all 3 arms.
    let mut additive_decomposition_table = Vec::new();
    for &seed in &prereg.execution_order {
        let u = by_seed_arm[&(seed, "control_uniform")];
        let s = by_seed_arm[&(seed, "control_uniform_scarcity")];
        let t = by_seed_arm[&(seed, "treatment_gradient")];
        let valid_all = [u, s, t].into_iter().all(|rec| {
            rec.result.status.is_completed()
                && first_extinct_tick(&rec.result, prereg.duration_ticks).is_none()
                && (rec.result.observable("live.agent_count").unwrap_or(0.0) as usize)
                    >= prereg.min_agent_count_at_t
        });
        if valid_all {
            let u_var = u.result.observable("live.trait_mass_var").unwrap_or(0.0);
            let s_var = s.result.observable("live.trait_mass_var").unwrap_or(0.0);
            let t_var = t.result.observable("live.trait_mass_var").unwrap_or(0.0);
            let d_tu = t_var - u_var;
            let d_ts = t_var - s_var;
            let d_su = s_var - u_var;
            additive_decomposition_table.push(AdditiveDecompositionRow {
                seed,
                control_uniform_var: u_var,
                control_uniform_scarcity_var: s_var,
                treatment_gradient_var: t_var,
                delta_t_minus_u: d_tu,
                delta_t_minus_s: d_ts,
                delta_s_minus_u: d_su,
                residual: d_tu - (d_ts + d_su),
            });
        }
    }

    Ok(EnsembleAnalysis {
        contrasts,
        agent_count_at_t_by_arm,
        additive_decomposition_table,
        extinction_table_by_arm,
        singleton_table_by_arm,
        empty_half_table_by_arm,
        per_arm_empty_half_counts,
        failed_runs,
        excluded_by_contrast_log,
    })
}

// ---- Artifact writers --------------------------------------------------------------------------

fn observable_report_to_json(r: &ObservableContrastReport) -> serde_json::Value {
    let e = &r.effect;
    serde_json::json!({
        "observable": e.observable,
        "n_requested": e.n_requested,
        "n_complete_pairs": e.n_complete_pairs,
        "n_extinct_pairs_dropped": r.n_extinct_pairs_dropped,
        "n_singleton_pairs_dropped": r.n_singleton_pairs_dropped,
        "n_empty_half_pairs_dropped_s3": r.n_empty_half_pairs_dropped_s3,
        "control_mean": e.control_mean,
        "treatment_mean": e.treatment_mean,
        "paired_mean_delta": e.paired_mean_delta,
        "paired_sd": e.paired_sd,
        "paired_se": e.paired_se,
        "ci95_low": e.ci95_low,
        "ci95_high": e.ci95_high,
        "ci95_method": e.ci95_method,
        "paired_dz": e.paired_dz,
        "median_of_per_seed_deltas": r.median_of_per_seed_deltas,
        "between_seed_variance_of_deltas": r.between_seed_variance_of_deltas,
        "decision": r.decision,
        "zero_control_mean_fallback_used": r.zero_control_mean_fallback_used,
        "per_seed_delta_table": r.per_seed_delta_table.iter().map(|row| serde_json::json!({
            "seed": row.seed,
            "control": row.control,
            "treatment": row.treatment,
            "delta": row.delta,
        })).collect::<Vec<_>>(),
    })
}

fn runs_csv_text(runs: &[ArmRunRecord], duration_ticks: u64) -> String {
    let mut s = String::from(
        "seed,arm,experiment_id,status,failed_tick,failed_reason,final_checksum,duration_ticks,sample_period,sample_count,terrain_source,world_seed,world_gen_version,world_width,world_height,world_checksum,extinct_tick,final_agent_count,empty_half\n",
    );
    for rec in runs {
        let (st, ft, fr) = match &rec.result.status {
            RunStatus::Completed => ("completed", String::new(), String::new()),
            RunStatus::Failed { tick, reason, .. } => {
                ("failed", tick.to_string(), csv_field(reason))
            }
        };
        let ext = first_extinct_tick(&rec.result, duration_ticks)
            .map(|v| v.to_string())
            .unwrap_or_default();
        let final_n = rec
            .result
            .observable("live.agent_count")
            .map(|v| v.to_string())
            .unwrap_or_default();
        let eh = empty_half_label(&rec.result).unwrap_or("");
        s.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            rec.seed,
            rec.arm,
            csv_field(&rec.result.provenance.experiment_id),
            st,
            ft,
            fr,
            rec.result.final_checksum,
            duration_ticks,
            rec.result.series.first().map(|x| x.tick).unwrap_or(0),
            rec.result.series.len(),
            csv_field(&rec.terrain_source),
            rec.world_identity.seed,
            rec.world_identity.generator_version,
            rec.world_identity.width,
            rec.world_identity.height,
            rec.world_identity.checksum,
            ext,
            final_n,
            eh
        ));
    }
    s
}

fn series_csv_text(runs: &[ArmRunRecord]) -> String {
    let mut s = String::from("seed,arm,tick");
    for col in SERIES_OBSERVABLES {
        s.push(',');
        s.push_str(col);
    }
    s.push('\n');
    for rec in runs {
        for sample in &rec.result.series {
            s.push_str(&format!("{},{},{}", rec.seed, rec.arm, sample.tick));
            for col in SERIES_OBSERVABLES {
                let v = run_series_obs(&rec.result.series, sample.tick, col)
                    .map(|x| x.to_string())
                    .unwrap_or_default();
                s.push(',');
                s.push_str(&v);
            }
            s.push('\n');
        }
    }
    s
}

fn final_observables_json_text(runs: &[ArmRunRecord]) -> Result<String, String> {
    let arr: Vec<serde_json::Value> = runs
        .iter()
        .map(|rec| {
            let obs_map: BTreeMap<String, f64> =
                rec.result.final_observables.iter().cloned().collect();
            serde_json::json!({
                "seed": rec.seed,
                "arm": rec.arm,
                "status": status_label(&rec.result.status),
                "final_checksum": rec.result.final_checksum,
                "terrain_source": rec.terrain_source,
                "world_identity": rec.world_identity,
                "experiment_id": rec.result.provenance.experiment_id,
                "manifest_fingerprint": rec.result.provenance.manifest_fingerprint,
                "law_fingerprint": rec.result.provenance.law_fingerprint,
                "registry_fingerprint": rec.result.provenance.registry_fingerprint,
                "model_version": rec.result.provenance.model_version,
                "fitness_rule": rec.result.provenance.fitness_rule,
                "final_observables": obs_map,
                "warnings": rec.result.warnings,
            })
        })
        .collect();
    Ok(pretty(&serde_json::Value::Array(arr))? + "\n")
}

fn effects_json_text(prereg: &PreregSpec, analysis: &EnsembleAnalysis) -> Result<String, String> {
    let contrasts_json: BTreeMap<String, serde_json::Value> = analysis
        .contrasts
        .iter()
        .map(|c| {
            (
                c.name.clone(),
                serde_json::json!({
                    "contrast": c.name,
                    "control_arm": c.control_arm,
                    "treatment_arm": c.treatment_arm,
                    "H1": observable_report_to_json(&c.h1),
                    "H1_bessel_sensitivity": observable_report_to_json(&c.h1_bessel_sensitivity),
                    "H1_tail_window": observable_report_to_json(&c.h1_tail_window),
                    "S1": observable_report_to_json(&c.s1),
                    "S2": observable_report_to_json(&c.s2),
                    "S3": observable_report_to_json(&c.s3),
                    "S4_structural_zero": observable_report_to_json(&c.s4),
                    "S5_structural_zero": observable_report_to_json(&c.s5),
                    "S6": {
                        "mean_lifespan": observable_report_to_json(&c.s6_mean_lifespan),
                        "births_sum": observable_report_to_json(&c.s6_births_sum),
                        "deaths_sum": observable_report_to_json(&c.s6_deaths_sum),
                    },
                    "I1_closed_eu_total": observable_report_to_json(&c.i1_closed_eu_total),
                    "M1_standing_crop_final": observable_report_to_json(&c.m1_standing_crop_final),
                    "M1_standing_crop_post_warmup": observable_report_to_json(&c.m1_standing_crop_post_warmup),
                }),
            )
        })
        .collect();

    let agent_counts_json: BTreeMap<String, Vec<serde_json::Value>> = analysis
        .agent_count_at_t_by_arm
        .iter()
        .map(|(arm, entries)| {
            (
                arm.clone(),
                entries
                    .iter()
                    .map(|(seed, count)| serde_json::json!({ "seed": seed, "agent_count": count }))
                    .collect(),
            )
        })
        .collect();

    let doc = serde_json::json!({
        "kind": "e3-effects-and-decision",
        "preregistration_id": prereg.raw["preregistration_id"],
        "primary_contrast": "treatment_gradient - control_uniform_scarcity",
        "primary_decision": analysis.contrasts[0].h1.decision,
        "thresholds_applied": {
            "min_valid_pairs_for_decision": prereg.thresholds.min_valid_pairs,
            "dz_min": prereg.thresholds.dz_min,
            "relative_delta_min": prereg.thresholds.relative_delta_min,
            "ci_excludes_zero_required": prereg.thresholds.ci_excludes_zero_required,
            "decision_enum": prereg.thresholds.decision_enum,
        },
        "schedule": {
            "duration_ticks": prereg.duration_ticks,
            "warmup_ticks": prereg.warmup_ticks,
            "sample_period": prereg.sample_period,
            "tail_window_ticks": prereg.tail_window,
        },
        "contrasts": contrasts_json,
        "agent_count_at_T_by_arm": agent_counts_json,
        "additive_decomposition_table_on_all_three_valid_seeds": analysis.additive_decomposition_table.iter().map(|r| serde_json::json!({
            "seed": r.seed,
            "control_uniform": r.control_uniform_var,
            "control_uniform_scarcity": r.control_uniform_scarcity_var,
            "treatment_gradient": r.treatment_gradient_var,
            "delta_t_minus_u": r.delta_t_minus_u,
            "delta_t_minus_s": r.delta_t_minus_s,
            "delta_s_minus_u": r.delta_s_minus_u,
            "residual": r.residual,
        })).collect::<Vec<_>>(),
        "extinction_table_by_arm": analysis.extinction_table_by_arm.iter().map(|r| serde_json::json!({
            "seed": r.seed, "arm": r.arm, "first_extinct_tick": r.first_extinct_tick
        })).collect::<Vec<_>>(),
        "singleton_table_by_arm": analysis.singleton_table_by_arm.iter().map(|r| serde_json::json!({
            "seed": r.seed, "arm": r.arm, "tick": r.tick, "agent_count": r.agent_count, "scope": r.scope
        })).collect::<Vec<_>>(),
        "empty_half_table_by_arm": analysis.empty_half_table_by_arm.iter().map(|r| serde_json::json!({
            "seed": r.seed, "arm": r.arm, "tick": r.tick, "empty_half": r.empty_half
        })).collect::<Vec<_>>(),
        "per_arm_empty_half_counts": analysis.per_arm_empty_half_counts,
        "failed_runs": analysis.failed_runs.iter().map(|r| serde_json::json!({
            "seed": r.seed, "arm": r.arm, "tick": r.tick, "reason": r.reason
        })).collect::<Vec<_>>(),
        "excluded_by_contrast_log": analysis.excluded_by_contrast_log.iter().map(|r| serde_json::json!({
            "seed": r.seed, "arm": r.arm, "contrast": r.contrast, "detail": r.detail, "tick": r.tick
        })).collect::<Vec<_>>(),
    });
    Ok(pretty(&doc)? + "\n")
}

fn portable_smoke_dir(dir: &Path) -> String {
    let repo_root = git(&["rev-parse", "--show-toplevel"])
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok());
    if let (Some(root), Ok(canon_dir)) = (repo_root, std::fs::canonicalize(dir)) {
        if let Ok(canon_root) = std::fs::canonicalize(root) {
            if let Ok(rel) = canon_dir.strip_prefix(&canon_root) {
                return rel.display().to_string().replace('\\', "/");
            }
        }
    }
    dir.display().to_string().replace('\\', "/")
}

fn smoke_ref_json(smoke: Option<&SmokeVerification>) -> serde_json::Value {
    match smoke {
        None => serde_json::Value::Null,
        Some(s) => serde_json::json!({
            "smoke_dir": portable_smoke_dir(&s.dir),
            "provenance_sha256": s.provenance_sha256,
            "smoke_report_sha256": s.smoke_report_sha256,
            "git_commit": s.smoke_git_commit,
            "host_os": s.smoke_host_os,
            "host_arch": s.smoke_host_arch,
            "profile": s.smoke_profile,
            "world_identity_observed": s.smoke_world_identity,
            "wp2_passed": s.wp2_passed,
        }),
    }
}

fn write_partial_stop(p: PartialStopInput<'_>) -> Result<(), String> {
    std::fs::create_dir_all(p.out_dir)
        .map_err(|e| format!("create stop output dir {}: {e}", p.out_dir.display()))?;
    write_text(&p.out_dir.join("runs.csv"), &runs_csv_text(p.runs, 0))?;
    let per_run: Vec<serde_json::Value> = p
        .runs
        .iter()
        .enumerate()
        .map(|(idx, r)| {
            serde_json::json!({
                "run_index": idx,
                "seed": r.seed,
                "arm": r.arm,
                "terrain_source": r.terrain_source,
                "world_identity": r.world_identity,
            })
        })
        .collect();
    let prov = serde_json::json!({
        "kind": "e3-confirmation-provenance-STOPPED",
        "mode": p.mode.as_str(),
        "stopped_early": true,
        "stop_reason": p.stop_reason,
        "runs_completed_before_stop": p.runs.len(),
        "build": {
            "git_commit": p.git_commit,
            "git_describe": p.git_describe,
            "git_dirty_paths": p.git_dirty_paths,
            "git_tree_dirty": !p.git_dirty_paths.is_empty(),
            "profile": if p.env.is_debug_build { "debug" } else { "release" },
            "rustc": tool_version("rustc"),
            "package_version": env!("CARGO_PKG_VERSION"),
            "binary_sha256": current_binary_sha256(),
        },
        "host": { "os": p.env.host_os, "arch": p.env.host_arch },
        "world_artifact_prerun_check": p.prerun_check,
        "smoke_reference": smoke_ref_json(p.smoke_ref),
        "per_run_world_provenance": per_run,
    });
    write_text(&p.out_dir.join("provenance.json"), &(pretty(&prov)? + "\n"))?;
    write_checksums(p.out_dir, &["provenance.json", "runs.csv"])?;
    Ok(())
}

fn write_ensemble_artifacts(w: WriteEnsembleInput<'_>) -> Result<(), String> {
    let manifests_dir = w.out_dir.join("manifests");
    std::fs::create_dir_all(&manifests_dir)
        .map_err(|e| format!("create {}: {e}", manifests_dir.display()))?;

    let mut manifest_names: Vec<String> = vec![PREREG_FILE.to_string()];
    manifest_names.push(w.prereg.control_file.clone());
    manifest_names.push(w.prereg.scarcity_file.clone());
    manifest_names.push(w.prereg.treatment_file.clone());
    for name in &manifest_names {
        let src = w.manifest_dir.join(name);
        let raw =
            std::fs::read_to_string(&src).map_err(|e| format!("read {}: {e}", src.display()))?;
        write_text(&manifests_dir.join(name), &raw)?;
    }

    write_text(
        &w.out_dir.join("runs.csv"),
        &runs_csv_text(w.runs, w.prereg.duration_ticks),
    )?;
    write_text(&w.out_dir.join("series.csv"), &series_csv_text(w.runs))?;
    write_text(
        &w.out_dir.join("final-observables.json"),
        &final_observables_json_text(w.runs)?,
    )?;
    write_text(
        &w.out_dir.join("effects.json"),
        &effects_json_text(w.prereg, w.analysis)?,
    )?;

    let per_run_world: Vec<serde_json::Value> = w
        .runs
        .iter()
        .enumerate()
        .map(|(idx, r)| {
            serde_json::json!({
                "run_index": idx,
                "seed": r.seed,
                "arm": r.arm,
                "terrain_source": r.terrain_source,
                "world_identity": r.world_identity,
            })
        })
        .collect();

    let arm_fps: BTreeMap<String, u64> = w
        .manifests
        .iter()
        .map(|(arm, _, m)| ((*arm).to_string(), m.fingerprint()))
        .collect();

    let prov = serde_json::json!({
        "kind": "e3-confirmation-provenance",
        "mode": "ensemble",
        "stopped_early": false,
        "stop_reason": null,
        "preregistration_id": w.prereg.raw["preregistration_id"],
        "preregistration_status": w.prereg.status,
        "model": "core::live_experiment::LiveExperimentAdapter",
        "model_version": LIVE_MODEL_VERSION,
        "model_status": "headless adapter verified; NOT a live-world experiment-ready claim",
        "harness": "core::experiment_runner::{run_paired_ensemble_with_control, run_manifest_seed}",
        "registry": "core::experiment::ObservableRegistry::live_default",
        "registry_fingerprint": w.registry.fingerprint(),
        "terrain_source": "generated",
        "world_identity": w.expected_world_identity,
        "world_identity_all_runs_identical": true,
        "per_run_world_provenance": per_run_world,
        "world_artifact_prerun_check": w.prerun_check,
        "smoke_reference": smoke_ref_json(w.smoke_ref),
        "build": {
            "git_commit": w.git_commit,
            "git_describe": w.git_describe,
            "git_dirty_paths": w.git_dirty_paths,
            "git_tree_dirty": !w.git_dirty_paths.is_empty(),
            "profile": if w.env.is_debug_build { "debug" } else { "release" },
            "rustc": tool_version("rustc"),
            "package_version": env!("CARGO_PKG_VERSION"),
            "binary_sha256": current_binary_sha256(),
        },
        "host": {
            "os": w.env.host_os,
            "arch": w.env.host_arch,
        },
        "manifests": {
            "sha256_as_committed": w.manifest_hashes,
            "manifest_fingerprints_as_run": arm_fps,
        },
        "seeds": {
            "execution_order": w.prereg.execution_order,
            "smoke_seed": w.prereg.smoke_seed,
            "n_requested": w.prereg.execution_order.len(),
            "total_runs_executed": w.runs.len(),
            "min_complete_pairs_for_a_decision": w.prereg.thresholds.min_valid_pairs,
        },
        "duration": {
            "duration_ticks": w.prereg.duration_ticks,
            "warmup_ticks": w.prereg.warmup_ticks,
            "sample_period": w.prereg.sample_period,
            "tail_window_ticks": w.prereg.tail_window,
        },
    });
    write_text(&w.out_dir.join("provenance.json"), &(pretty(&prov)? + "\n"))?;

    let h1 = &w.analysis.contrasts[0].h1;
    let bessel = &w.analysis.contrasts[0].h1_bessel_sensitivity;
    let summary = format!(
        "# E3-X Confirmation Summary (`{}`)\n\n\
         - **Host**: `{}/{}` | **Profile**: `{}` | **World identity checksum**: `{}` (`terrain_source`: `generated`)\n\
         - **Runs**: `{}` (`{}` seeds × 3 arms × `{}` ticks)\n\
         - **Primary H1 (`treatment_gradient - control_uniform_scarcity`, `live.trait_mass_var` @ T)**:\n\
           - Decision: **`{}`** (`n_complete_pairs = {}`, `extinct_dropped = {}`, `singleton_dropped = {}`)\n\
           - `paired_mean_delta = {}`, `ci95 = [{}, {}]`, `paired_dz = {}`\n\
         - **H1 Bessel sensitivity (`var * n / (n - 1)`)**: `{}` (`delta = {}`, `dz = {}`)\n",
        w.prereg.raw["preregistration_id"].as_str().unwrap_or("e3"),
        w.env.host_os,
        w.env.host_arch,
        if w.env.is_debug_build { "debug" } else { "release" },
        w.expected_world_identity.checksum,
        w.runs.len(),
        w.prereg.execution_order.len(),
        w.prereg.duration_ticks,
        h1.decision.as_deref().unwrap_or("none"),
        h1.effect.n_complete_pairs,
        h1.n_extinct_pairs_dropped,
        h1.n_singleton_pairs_dropped,
        opt_f64(h1.effect.paired_mean_delta),
        opt_f64(h1.effect.ci95_low),
        opt_f64(h1.effect.ci95_high),
        opt_f64(h1.effect.paired_dz),
        bessel.decision.as_deref().unwrap_or("none"),
        opt_f64(bessel.effect.paired_mean_delta),
        opt_f64(bessel.effect.paired_dz),
    );
    write_text(&w.out_dir.join("summary.md"), &summary)?;

    let timing_doc = serde_json::json!({
        "started_utc": iso8601_utc(unix_seconds(w.started)),
        "ended_utc": iso8601_utc(unix_seconds(w.ended)),
        "wall_clock_seconds": w.total_elapsed_secs,
        "per_run_seconds": w.runs.iter().map(|r| serde_json::json!({
            "seed": r.seed, "arm": r.arm, "elapsed_seconds": r.elapsed_secs
        })).collect::<Vec<_>>(),
    });
    write_text(
        &w.out_dir.join("timing.json"),
        &(pretty(&timing_doc)? + "\n"),
    )?;

    let mut hashed_names: Vec<String> = vec![
        "effects.json".into(),
        "final-observables.json".into(),
        "provenance.json".into(),
        "runs.csv".into(),
        "series.csv".into(),
        "summary.md".into(),
    ];
    for name in &manifest_names {
        hashed_names.push(format!("manifests/{name}"));
    }
    hashed_names.sort();
    let refs: Vec<&str> = hashed_names.iter().map(String::as_str).collect();
    write_checksums(w.out_dir, &refs)?;
    Ok(())
}

// ---- `--self-test` -----------------------------------------------------------------------------

fn hook_mismatch_second(
    idx: usize,
    _arm: &str,
    _seed: u64,
    mut id: WorldIdentity,
) -> WorldIdentity {
    if idx == 1 {
        id.checksum = id.checksum.wrapping_add(1);
    }
    id
}

fn run_self_test(base_env: &RuntimeEnv) -> Result<(), String> {
    use anima_engine_lib::core::experiment::InitialConditionSet;
    use anima_engine_lib::core::experiment::WorldLawSet;
    use anima_engine_lib::core::intervention::{
        Curve, InterventionCommand, InterventionKind, Region,
    };
    use anima_engine_lib::core::live_experiment::{
        all_live_observable_ids, LIVE_KEY_RESOURCE_CAPACITY_SCALE,
    };

    let default_mdir = resolve_default_manifest_dir();
    let prereg_path = default_mdir.join(PREREG_FILE);
    let prereg = load_prereg(&default_mdir)?;
    assert_eq_check(prereg.thresholds.min_valid_pairs, 10, "min_valid_pairs")?;

    // 1. Threshold classification boundaries read from frozen preregistration JSON.
    let t = &prereg.thresholds;
    let mk_eff =
        |n: usize, delta: f64, lo: Option<f64>, hi: Option<f64>, dz: Option<f64>| PairedEffect {
            observable: "live.trait_mass_var".into(),
            n_requested: 12,
            n_complete_pairs: n,
            control_mean: Some(10.0),
            treatment_mean: Some(10.0 + delta),
            paired_mean_delta: Some(delta),
            paired_sd: Some(1.0),
            paired_se: Some(0.3),
            ci95_low: lo,
            ci95_high: hi,
            ci95_method: None,
            paired_dz: dz,
        };
    assert_str_check(
        classify_decision(&mk_eff(9, 2.0, Some(1.0), Some(3.0), Some(2.0)), t, false).0,
        "insufficient_evidence",
        "n=9 insufficient",
    )?;
    assert_str_check(
        classify_decision(&mk_eff(10, 2.0, Some(-0.1), Some(4.1), Some(2.0)), t, false).0,
        "null",
        "ci includes 0 -> null",
    )?;
    assert_str_check(
        classify_decision(&mk_eff(10, 0.4, Some(0.1), Some(0.7), Some(1.0)), t, false).0,
        "between_detectable_below_materiality",
        "below 5% relative shift",
    )?;
    assert_str_check(
        classify_decision(&mk_eff(10, 1.0, Some(0.2), Some(1.8), Some(0.79)), t, false).0,
        "between_detectable_below_materiality",
        "below 0.8 dz",
    )?;
    assert_str_check(
        classify_decision(&mk_eff(10, 1.0, Some(0.2), Some(1.8), Some(0.8)), t, false).0,
        "material_positive",
        "material_positive boundary",
    )?;
    assert_str_check(
        classify_decision(
            &mk_eff(10, -1.0, Some(-1.8), Some(-0.2), Some(-0.8)),
            t,
            false,
        )
        .0,
        "material_negative_opposite",
        "material_negative_opposite boundary",
    )?;

    // 2. Synthetic short 60-tick run using disjoint seed 612001 (never touches 730001..741001).
    let root = base_env
        .temp_dir
        .join(format!("e3x-runner-selftest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let mdir = root.join("manifests");
    let smoke_dir = root.join("smoke");
    let out_ok = root.join("out_ok");
    let out_stop = root.join("out_stop");
    let fake_temp = root.join("temp_empty");
    std::fs::create_dir_all(&mdir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&smoke_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&fake_temp).map_err(|e| e.to_string())?;

    let mut prereg_val = read_json(&prereg_path)?;
    prereg_val["seeds"]["execution_order"] = serde_json::json!([612_001u64]);
    prereg_val["duration"]["duration_ticks"] = serde_json::json!(60u64);
    prereg_val["duration"]["sample_period"] = serde_json::json!(20u64);
    prereg_val["duration"]["warmup_ticks"] = serde_json::json!(20u64);
    prereg_val["duration"]["tail_window_ticks"] = serde_json::json!([40u64, 60u64]);
    prereg_val["wp2_smoke_calibration"]["parent_commit"] = serde_json::json!("test-commit-sha");
    write_text(&mdir.join(PREREG_FILE), &(pretty(&prereg_val)? + "\n"))?;

    let reg = ObservableRegistry::live_default();
    let all_obs: Vec<String> = all_live_observable_ids()
        .into_iter()
        .map(str::to_string)
        .collect();
    let base_vals = |scale: f64| -> Vec<(String, f64)> {
        vec![
            ("live.ecological_fitness".into(), 1.0),
            ("live.mortality".into(), 1.0),
            ("live.reproduction".into(), 1.0),
            ("live.locomotion".into(), 1.0),
            ("live.drive_floor".into(), 0.5),
            ("live.thirst_steering".into(), 1.0),
            ("live.evolved_brains".into(), 0.0),
            ("live.founders".into(), 6.0),
            ("live.predator_fraction".into(), 0.3),
            ("live.trees".into(), 8.0),
            ("live.lakes".into(), 2.0),
            ("live.food_cap".into(), 50.0),
            (LIVE_KEY_RESOURCE_CAPACITY_SCALE.into(), scale),
        ]
    };
    let mk_manifest =
        |id: &str, scale: f64, deforest: f32, nut: f32, min_x: u32| ExperimentManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            experiment_id: id.into(),
            name: id.into(),
            world_identity: WorldIdentity::default(),
            laws: WorldLawSet::baseline(),
            initial_conditions: InitialConditionSet::new(base_vals(scale)),
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
                    duration_ticks: 60,
                    curve: Curve::Step,
                    reversible: false,
                    signed_negative: false,
                },
                InterventionCommand {
                    id: 2,
                    cause_id: 2,
                    kind: InterventionKind::AddNutrient,
                    region: Region::Rect {
                        min_x,
                        min_y: 0,
                        max_x: 255,
                        max_y: 255,
                    },
                    intensity: nut,
                    start_tick: 1,
                    duration_ticks: 60,
                    curve: Curve::Step,
                    reversible: false,
                    signed_negative: false,
                },
            ],
            seeds: vec![612_001],
            duration_ticks: 60,
            sample_period: 20,
            observable_ids: all_obs.clone(),
            exotic_interventions: Vec::new(),
            observer: Default::default(),
        };
    let m_ctrl = mk_manifest("e3::selftest_ctrl", 1.0, 0.0, 0.0, 128);
    let m_scarcity = mk_manifest("e3::selftest_scarcity", 0.42, 0.0, 1.0, 0);
    let m_treat = mk_manifest("e3::selftest_treat", 1.0, 0.3, 1.0, 128);

    // Exercise `run_paired_ensemble_with_control` directly on synthetic seed 612001 as well.
    let paired_check = run_paired_ensemble_with_control::<LiveExperimentAdapter>(
        &m_ctrl,
        &m_treat,
        &reg,
        &FactorDiff {
            allowed_paths: vec!["interventions".into()],
        },
    )
    .map_err(|e| format!("self-test run_paired_ensemble_with_control failed: {e}"))?;
    let expected_wid = paired_check.pairs[0]
        .control
        .provenance
        .world_identity
        .ok_or("self-test missing world_identity")?;

    for (fname, m) in [
        (&prereg.control_file, &m_ctrl),
        (&prereg.scarcity_file, &m_scarcity),
        (&prereg.treatment_file, &m_treat),
    ] {
        write_text(
            &mdir.join(fname),
            &(pretty(&serde_json::to_value(m).map_err(|e| e.to_string())?)? + "\n"),
        )?;
    }

    write_text(
        &smoke_dir.join("provenance.json"),
        &(pretty(&serde_json::json!({
            "build": { "git_commit": "test-commit-sha", "profile": "release" },
            "host": { "os": base_env.host_os, "arch": base_env.host_arch },
            "world_identity_observed": expected_wid,
        }))? + "\n"),
    )?;
    write_text(
        &smoke_dir.join("smoke-report.json"),
        &(pretty(&serde_json::json!({
            "all_observables_emitted_and_finite": true,
            "wp2_a_resource_field": { "passed": true },
            "wp2_b_standing_crop_intervention": { "passed": true },
            "wp2_c_detritus_before_3000": { "passed": true },
            "integrity": { "world_identity_identical": true },
            "wp3_schedule_lock": { "within_budget": true },
        }))? + "\n"),
    )?;
    write_checksums(&smoke_dir, &["provenance.json", "smoke-report.json"])?;

    let ok_env = RuntimeEnv {
        anima_world_artifact: None,
        temp_dir: fake_temp.clone(),
        is_debug_build: false,
        host_os: base_env.host_os.clone(),
        host_arch: base_env.host_arch.clone(),
        git_commit_override: Some("test-commit-sha".into()),
        git_dirty_paths_override: Some(Vec::new()),
        world_identity_override: None,
    };
    run_with_config(
        vec![
            "--ensemble".into(),
            "--manifest-dir".into(),
            mdir.display().to_string(),
            "--smoke-dir".into(),
            smoke_dir.display().to_string(),
            "--out".into(),
            out_ok.display().to_string(),
        ],
        ok_env.clone(),
    )?;

    // Check refusal in debug build & world artifact gate
    let mut debug_env = ok_env.clone();
    debug_env.is_debug_build = true;
    if run_with_config(
        vec![
            "--ensemble".into(),
            "--manifest-dir".into(),
            mdir.display().to_string(),
            "--smoke-dir".into(),
            smoke_dir.display().to_string(),
            "--out".into(),
            root.join("out_debug").display().to_string(),
        ],
        debug_env,
    )
    .is_ok()
    {
        return Err("self-test failed: debug build was not refused".into());
    }

    // Check immediate stop on world_identity mismatch
    let mut stop_env = ok_env;
    stop_env.world_identity_override = Some(hook_mismatch_second);
    if run_with_config(
        vec![
            "--ensemble".into(),
            "--manifest-dir".into(),
            mdir.display().to_string(),
            "--smoke-dir".into(),
            smoke_dir.display().to_string(),
            "--out".into(),
            out_stop.display().to_string(),
        ],
        stop_env,
    )
    .is_ok()
    {
        return Err("self-test failed: world_identity mismatch did not stop run".into());
    }
    let stop_prov = read_json(&out_stop.join("provenance.json"))?;
    if stop_prov["stopped_early"].as_bool() != Some(true)
        || stop_prov["runs_completed_before_stop"].as_u64() != Some(2)
    {
        return Err("self-test failed: partial stop did not halt after 2nd run".into());
    }

    let _ = std::fs::remove_dir_all(&root);
    println!("run_e3_experiment --self-test PASSED");
    Ok(())
}

fn assert_eq_check(a: usize, b: usize, label: &str) -> Result<(), String> {
    if a != b {
        return Err(format!("self-test {label}: expected {b}, got {a}"));
    }
    Ok(())
}

fn assert_str_check(a: String, b: &str, label: &str) -> Result<(), String> {
    if a != b {
        return Err(format!("self-test {label}: expected '{b}', got '{a}'"));
    }
    Ok(())
}

// ---- Low-level utilities -----------------------------------------------------------------------

fn current_binary_sha256() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    sha256_file(&exe).ok()
}

fn resolve_git_commit(env: &RuntimeEnv) -> Option<String> {
    env.git_commit_override
        .clone()
        .or_else(|| git(&["rev-parse", "HEAD"]))
}

fn resolve_git_dirty_paths(env: &RuntimeEnv) -> Vec<String> {
    if let Some(paths) = &env.git_dirty_paths_override {
        return paths.clone();
    }
    let out = match std::process::Command::new("git")
        .args(["-c", "status.relativePaths=false", "status", "--porcelain"])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => return Vec::new(),
    };
    out.lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .map(|line| {
            if line.len() > 3 {
                line[3..].trim().to_string()
            } else {
                line.to_string()
            }
        })
        .collect()
}

pub fn write_checksums(dir: &Path, names: &[&str]) -> Result<(), String> {
    let mut sorted: Vec<&str> = names.to_vec();
    sorted.sort_unstable();
    let mut lines = String::new();
    for name in sorted {
        let digest = sha256_file(&dir.join(name)).map_err(|e| format!("hash {name}: {e}"))?;
        lines.push_str(&format!("{digest}  {name}\n"));
    }
    write_text(&dir.join("checksums.sha256"), &lines)
}

fn opt_f64(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.6}"))
        .unwrap_or_else(|| "null".into())
}

fn status_label(s: &RunStatus) -> &'static str {
    match s {
        RunStatus::Completed => "completed",
        RunStatus::Failed { .. } => "failed",
    }
}

fn csv_field(s: &str) -> String {
    s.replace([',', '\n', '\r', '"'], " ")
}

fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))
}

fn load_manifest(path: &Path) -> Result<ExperimentManifest, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("{} is not an ExperimentManifest: {e}", path.display()))
}

fn pretty(v: &serde_json::Value) -> Result<String, String> {
    serde_json::to_string_pretty(v).map_err(|e| e.to_string())
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text.replace("\r\n", "\n").as_bytes())
        .map_err(|e| format!("write {}: {e}", path.display()))
}

fn git(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn tool_version(tool: &str) -> Option<String> {
    let out = std::process::Command::new(tool)
        .arg("--version")
        .output()
        .ok()?;
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
