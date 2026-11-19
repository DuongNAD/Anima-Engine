//! The SEL2-P experiment runner: does ecological selection shift post-warmup time-integrated
//! genotypic body mass (`live.trait_mass_mean`) relative to lockstep neutral drift (`SEL2-D`)?
//!
//! Mirrors `src-tauri/examples/run_esel1_experiment.rs`, driving [`LiveExperimentAdapter`] through
//! [`run_live_drift_pair`], [`run_live_drift_ensemble`], and [`run_manifest_seed`], and pairing
//! outcomes through [`paired_effect`].
//!
//! # Build it, then run the binary — never `cargo run`
//!
//! ```text
//! cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop \
//!   --example run_sel2p_experiment
//!
//! src-tauri/target/release/examples/run_sel2p_experiment --pipeline-check \
//!   --manifest-dir src-tauri/tests/fixtures/experiments_sel2p \
//!   --out /tmp/sel2p-pipeline-check
//! ```
//!
//! No Tauri handle, window, GPU, learner, or evolution thread exists in this process; the status
//! stays **headless adapter verified**.
//!
//! # Modes
//!
//! | mode | runs | writes | status gate |
//! |---|---|---|---|
//! | `--pipeline-check` | 2,000-tick end-to-end check (disjoint seeds `999801..999822`) | `<out>/smoke/` + `<out>/` | works on `PROPOSED` |
//! | `--smoke` | 200,000-tick pooled baseline (`799501..=799506`) + smoke (`999961`), blind | `<out>/smoke/` | works on `PROPOSED` |
//! | `--confirm` | 200,000-tick 20-seed x 3-arm confirmation ensemble (`780001..799001`) | `<out>/` | requires `ACCEPTED`/`FROZEN` + `execution-lock.json` |

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anima_engine_lib::core::drift_control::DriftDiagnostics;
use anima_engine_lib::core::experiment::{ExperimentManifest, FactorDiff, ObservableRegistry};
use anima_engine_lib::core::experiment_runner::{
    paired_effect, run_manifest_seed, PairedEffect, PairedEnsembleReport, RunResult, RunStatus,
};
use anima_engine_lib::core::live_experiment::{
    all_live_observable_ids, run_live_drift_ensemble, run_live_drift_pair, DriftEnsembleReport,
    LiveExperimentAdapter, LiveWorldConfig, LIVE_KEY_DRIFT_CONTROL, LIVE_KEY_MUTATION_RATE,
    LIVE_KEY_PREY_CAP, LIVE_KEY_RESOURCE_CAPACITY_SCALE, LIVE_MODEL_VERSION,
};
use anima_engine_lib::core::scenario::StateSample;
use anima_engine_lib::core::sha256::sha256_file;

const PREREG_FILE: &str = "sel2p-preregistration.json";
const LOCK_FILE: &str = "execution-lock.json";
const ALLOWED_FACTOR_PATH: &str = "initial_conditions";

const PRIMARY_OBSERVABLE: &str = "live.trait_mass_mean";
const MASS_VAR_OBSERVABLE: &str = "live.trait_mass_var";
const ENERGY_INTEGRAL_OBSERVABLE: &str = "live.mean_energy_integral";
const COUNT_OBSERVABLE: &str = "live.agent_count";
const CROP_OBSERVABLE: &str = "live.standing_crop";
const INTEGRITY_OBSERVABLE: &str = "live.closed_eu_total";
const BIRTHS_OBSERVABLE: &str = "live.births";
const DEATHS_OBSERVABLE: &str = "live.deaths";
const LIFESPAN_OBSERVABLE: &str = "live.mean_lifespan";
const STARVATION_OBSERVABLE: &str = "live.deaths_starvation";
const DEHYDRATION_OBSERVABLE: &str = "live.deaths_dehydration";

const POST_WARMUP_STAT: &str = "live.trait_mass_mean@post_warmup_int";
const ENERGY_POST_WARMUP_STAT: &str = "live.mean_energy_integral@post_warmup_mean";
const TAIL20_STAT: &str = "live.trait_mass_mean@tail20";
const WINDOW_NET_STAT: &str = "window_net_selection_deltas";

const DESCRIPTIVE_OBSERVABLES: [&str; 10] = [
    "live.agent_count",
    "live.standing_crop",
    "live.food_items",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
    "live.mean_energy_integral",
    "live.trait_mass_var",
    "live.trait_mass_p10",
    "live.trait_mass_p90",
];

/// Non-drift smoke allow-list (8 observables, zero `trait_mass_*`).
const SMOKE_ALLOWED_NON_DRIFT: [&str; 8] = [
    INTEGRITY_OBSERVABLE,
    COUNT_OBSERVABLE,
    CROP_OBSERVABLE,
    BIRTHS_OBSERVABLE,
    DEATHS_OBSERVABLE,
    LIFESPAN_OBSERVABLE,
    STARVATION_OBSERVABLE,
    DEHYDRATION_OBSERVABLE,
];

/// Drift smoke allow-list (6 observables, zero `trait_mass_*`, zero death-cause split).
const SMOKE_ALLOWED_DRIFT: [&str; 6] = [
    INTEGRITY_OBSERVABLE,
    COUNT_OBSERVABLE,
    CROP_OBSERVABLE,
    BIRTHS_OBSERVABLE,
    DEATHS_OBSERVABLE,
    LIFESPAN_OBSERVABLE,
];

const C1_MIN_AGENTS: f64 = 5.0;
const C2_G_LOW: f64 = 0.4;
const C2_G_HIGH: f64 = 0.6;
const C3_MIN_GENERATIONS: f64 = 30.0;
const C4_MIN_STARVATION_SHARE: f64 = 0.50;

const MATERIALITY_DZ: f64 = 0.66;
const MATERIALITY_RELATIVE_SHIFT: f64 = 0.05;
const CLOSED_EU_TOLERANCE: f64 = 1e-6;
const CAUSE_PARTITION_TOLERANCE: f64 = 1e-6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    PipelineCheck,
    Smoke,
    Confirm,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Mode::PipelineCheck => "pipeline-check",
            Mode::Smoke => "smoke",
            Mode::Confirm => "confirm",
        }
    }
}

struct Args {
    mode: Mode,
    manifest_dir: PathBuf,
    out: PathBuf,
}

#[allow(dead_code)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run_with_args(args) {
        eprintln!("SEL2-P runner refused to proceed: {e}");
        std::process::exit(2);
    }
}

// ---- Public math & gate evaluators (also tested in prereg_sel2p_manifest_tests.rs) ------------

/// One `prey_cap = 80, scale = 1.0` baseline run outcome used for QD-SEL2g pooled `g`.
#[derive(Clone, Debug, PartialEq)]
pub struct BaselineObservation {
    pub seed: u64,
    pub completed: bool,
    pub extinct_tick: Option<u64>,
    pub crop_second_half: Option<f64>,
}

/// Pooled reference across surviving baseline runs (QD-SEL2g option (b)).
#[derive(Clone, Debug, PartialEq)]
pub struct PooledBaseline {
    pub n_requested: usize,
    pub n_surviving: usize,
    pub min_required: usize,
    pub surviving_seeds: Vec<u64>,
    pub pooled_crop_second_half: Option<f64>,
    pub sufficient: bool,
}

/// Compute QD-SEL2g pooled reference strictly over surviving completed baseline runs.
///
/// Requires `n_surviving >= min_required` (preregistered `3`). When fewer survive,
/// `sufficient = false` and `pooled_crop_second_half = None` so `C2` fails.
pub fn compute_pooled_baseline(
    runs: &[BaselineObservation],
    min_required: usize,
) -> PooledBaseline {
    let mut surviving_seeds = Vec::new();
    let mut crops = Vec::new();
    for r in runs {
        if r.completed && r.extinct_tick.is_none() {
            if let Some(crop) = r.crop_second_half {
                if crop.is_finite() {
                    surviving_seeds.push(r.seed);
                    crops.push(crop);
                }
            }
        }
    }
    let n_surviving = surviving_seeds.len();
    let sufficient = n_surviving >= min_required;
    let pooled_crop_second_half = if sufficient && !crops.is_empty() {
        Some(crops.iter().sum::<f64>() / crops.len() as f64)
    } else {
        None
    };
    PooledBaseline {
        n_requested: runs.len(),
        n_surviving,
        min_required,
        surviving_seeds,
        pooled_crop_second_half,
        sufficient,
    }
}

/// Smoke-gate metrics for one scarce run (blind to `trait_mass_*`).
#[derive(Clone, Debug, PartialEq)]
pub struct ScarceGateInput {
    pub arm: &'static str,
    pub completed: bool,
    pub extinct_tick: Option<u64>,
    pub min_agents: Option<f64>,
    pub mean_agents: Option<f64>,
    pub crop_second_half: Option<f64>,
    pub total_births: f64,
    pub total_deaths: f64,
    pub mean_lifespan: Option<f64>,
    pub total_starvation: Option<f64>,
    pub total_dehydration: Option<f64>,
    pub cause_partition_ok: Option<bool>,
    pub births_last_quarter: f64,
    pub deaths_last_quarter: f64,
}

/// Evaluation of smoke gate `C1..C5` + `drift_valid` (Draft section 6 + QD-SEL2g).
#[derive(Clone, Debug, PartialEq)]
pub struct GateReport {
    pub c1: bool,
    pub c2: bool,
    pub c3: bool,
    pub c4: bool,
    pub c5: bool,
    pub drift_valid: bool,
    pub gate_pass: bool,
    pub g_selected: Option<f64>,
    pub g_scarce_mut0: Option<f64>,
    pub g_scarce_drift: Option<f64>,
    pub generations_selected: Option<f64>,
    pub generations_scarce_mut0: Option<f64>,
    pub starvation_share_selected: Option<f64>,
    pub starvation_share_scarce_mut0: Option<f64>,
}

/// Evaluate `C1..C5` against `baseline` (QD-SEL2g pooled reference, `plant_reserve = 0.0`).
///
/// `C4` applies only to `selected` and `scarce_mut0`; `scarce_drift` is checked for `drift_valid`
/// (`completed && extinct_tick.is_none()`) and its `g` ratio is recorded descriptively.
pub fn evaluate_smoke_gate(
    duration_ticks: u64,
    baseline: &PooledBaseline,
    selected: &ScarceGateInput,
    scarce_mut0: &ScarceGateInput,
    scarce_drift: &ScarceGateInput,
) -> GateReport {
    let survived = |x: &ScarceGateInput| x.completed && x.extinct_tick.is_none();

    let c1_arm = |x: &ScarceGateInput| {
        survived(x)
            && x.min_agents
                .is_some_and(|m| m.is_finite() && m >= C1_MIN_AGENTS)
    };
    let c1 = c1_arm(selected) && c1_arm(scarce_mut0);

    let ref_crop = baseline
        .pooled_crop_second_half
        .filter(|c| baseline.sufficient && c.is_finite() && *c > 0.0);
    let g_ratio = |x: &ScarceGateInput| -> Option<f64> {
        let rc = ref_crop?;
        let crop = x.crop_second_half.filter(|c| c.is_finite())?;
        Some(crop / rc)
    };
    let g_selected = g_ratio(selected);
    let g_scarce_mut0 = g_ratio(scarce_mut0);
    let g_scarce_drift = g_ratio(scarce_drift);
    let g_in_band = |g: Option<f64>| g.is_some_and(|v| (C2_G_LOW..=C2_G_HIGH).contains(&v));
    let c2 = baseline.sufficient && g_in_band(g_selected) && g_in_band(g_scarce_mut0);

    let gens = |x: &ScarceGateInput| -> Option<f64> {
        if x.total_deaths <= 0.0 {
            return None;
        }
        let lifespan = x.mean_lifespan.filter(|l| l.is_finite() && *l > 0.0)?;
        let mean_n = x.mean_agents.filter(|n| n.is_finite() && *n > 0.0)?;
        let by_lifespan = (duration_ticks as f64) / lifespan;
        let by_turnover = x.total_births / mean_n.max(1.0);
        Some(by_lifespan.min(by_turnover))
    };
    let generations_selected = gens(selected);
    let generations_scarce_mut0 = gens(scarce_mut0);
    let c3 = generations_selected.is_some_and(|g| g >= C3_MIN_GENERATIONS)
        && generations_scarce_mut0.is_some_and(|g| g >= C3_MIN_GENERATIONS);

    let starv_share = |x: &ScarceGateInput| -> Option<f64> {
        if x.total_deaths <= 0.0 || x.cause_partition_ok != Some(true) {
            return None;
        }
        let s = x.total_starvation.filter(|v| v.is_finite())?;
        let _h = x.total_dehydration.filter(|v| v.is_finite())?;
        Some(s / x.total_deaths)
    };
    let starvation_share_selected = starv_share(selected);
    let starvation_share_scarce_mut0 = starv_share(scarce_mut0);
    let c4 = starvation_share_selected.is_some_and(|s| s > C4_MIN_STARVATION_SHARE)
        && starvation_share_scarce_mut0.is_some_and(|s| s > C4_MIN_STARVATION_SHARE);

    let c5_arm = |x: &ScarceGateInput| x.births_last_quarter > 0.0 && x.deaths_last_quarter > 0.0;
    let c5 = c5_arm(selected) && c5_arm(scarce_mut0);

    let drift_valid = survived(scarce_drift);
    let gate_pass = c1 && c2 && c3 && c4 && c5 && drift_valid;

    GateReport {
        c1,
        c2,
        c3,
        c4,
        c5,
        drift_valid,
        gate_pass,
        g_selected,
        g_scarce_mut0,
        g_scarce_drift,
        generations_selected,
        generations_scarce_mut0,
        starvation_share_selected,
        starvation_share_scarce_mut0,
    }
}

/// Arithmetic mean over post-warmup samples (`tick > warmup_ticks`).
pub fn post_warmup_mean(series: &[(u64, f64)], warmup_ticks: u64) -> Option<f64> {
    let vals: Vec<f64> = series
        .iter()
        .filter(|(t, v)| *t > warmup_ticks && v.is_finite())
        .map(|(_, v)| *v)
        .collect();
    if vals.is_empty() {
        None
    } else {
        Some(vals.iter().sum::<f64>() / vals.len() as f64)
    }
}

/// S2 post-warmup mean agent energy `(I(T) - I(T_warm)) / (T - T_warm)`.
pub fn post_warmup_energy_mean(
    series: &[(u64, f64)],
    warmup_ticks: u64,
    duration_ticks: u64,
) -> Option<f64> {
    if duration_ticks <= warmup_ticks {
        return None;
    }
    let i_warm = if warmup_ticks == 0 {
        Some(0.0)
    } else {
        series
            .iter()
            .find(|(t, _)| *t == warmup_ticks)
            .map(|(_, v)| *v)
    }?;
    let i_end = series
        .iter()
        .find(|(t, _)| *t == duration_ticks)
        .map(|(_, v)| *v)?;
    if !i_warm.is_finite() || !i_end.is_finite() {
        return None;
    }
    Some((i_end - i_warm) / (duration_ticks - warmup_ticks) as f64)
}

/// Arithmetic mean over final-20% samples (`tick > 0.8 * T`).
pub fn tail20_mean(series: &[(u64, f64)], duration_ticks: u64) -> Option<f64> {
    let vals: Vec<f64> = series
        .iter()
        .filter(|(t, v)| (*t).saturating_mul(5) > duration_ticks.saturating_mul(4) && v.is_finite())
        .map(|(_, v)| *v)
        .collect();
    if vals.is_empty() {
        None
    } else {
        Some(vals.iter().sum::<f64>() / vals.len() as f64)
    }
}

// ---- Entrypoint --------------------------------------------------------------------------------

pub fn run_with_args(argv: Vec<String>) -> Result<(), String> {
    let args = parse_args(argv)?;
    let registry = ObservableRegistry::live_default();
    registry
        .validate()
        .map_err(|e| format!("the live registry is invalid: {e}"))?;

    let prereg_path = args.manifest_dir.join(PREREG_FILE);
    let prereg = read_json(&prereg_path)?;
    let plan = Plan::from_prereg(&prereg)?;

    let selected = load_manifest(&args.manifest_dir.join(&plan.selected_file))?;
    let scarce_mut0 = load_manifest(&args.manifest_dir.join(&plan.scarce_mut0_file))?;
    let scarce_drift = load_manifest(&args.manifest_dir.join(&plan.scarce_drift_file))?;
    let baseline_ref = load_manifest(&args.manifest_dir.join(&plan.baseline_ref_file))?;
    let smoke_selected = load_manifest(&args.manifest_dir.join(&plan.smoke_selected_file))?;
    let smoke_mut0 = load_manifest(&args.manifest_dir.join(&plan.smoke_mut0_file))?;
    let smoke_drift = load_manifest(&args.manifest_dir.join(&plan.smoke_drift_file))?;

    let mut file_names = vec![
        plan.selected_file.clone(),
        plan.scarce_mut0_file.clone(),
        plan.scarce_drift_file.clone(),
        plan.baseline_ref_file.clone(),
        plan.smoke_selected_file.clone(),
        plan.smoke_mut0_file.clone(),
        plan.smoke_drift_file.clone(),
        PREREG_FILE.to_string(),
    ];
    file_names.sort();
    let mut manifest_hashes = BTreeMap::new();
    for name in &file_names {
        let digest =
            sha256_file(&args.manifest_dir.join(name)).map_err(|e| format!("hash {name}: {e}"))?;
        manifest_hashes.insert(name.clone(), digest);
    }

    validate_all_manifests(
        &registry,
        &plan,
        &selected,
        &scarce_mut0,
        &scarce_drift,
        &baseline_ref,
        &smoke_selected,
        &smoke_mut0,
        &smoke_drift,
    )?;

    if args.mode == Mode::Confirm {
        if plan.status != "FROZEN" && plan.status != "ACCEPTED" {
            return Err(format!(
                "--confirm refused: {PREREG_FILE} status is '{}'; confirmation seeds may not run \
                 until the preregistration is ACCEPTED or FROZEN",
                plan.status
            ));
        }
        let lock_path = args.out.join(LOCK_FILE);
        if !lock_path.is_file() {
            return Err(format!(
                "--confirm needs {} (execution lock after a passing smoke)",
                lock_path.display()
            ));
        }
        let lock = read_json(&lock_path)?;
        validate_confirm_lock(&lock, &plan, &manifest_hashes)?;
    }

    std::fs::create_dir_all(&args.out)
        .map_err(|e| format!("create {}: {e}", args.out.display()))?;

    match args.mode {
        Mode::Smoke => {
            let smoke_dir = args.out.join("smoke");
            std::fs::create_dir_all(&smoke_dir)
                .map_err(|e| format!("create {}: {e}", smoke_dir.display()))?;
            let _gate = run_smoke_stage(
                Mode::Smoke,
                &baseline_ref,
                &smoke_selected,
                &smoke_mut0,
                &smoke_drift,
                &registry,
                &smoke_dir,
                &file_names,
                &manifest_hashes,
                &plan,
                plan.smoke_duration_ticks,
            )?;
            Ok(())
        }
        Mode::Confirm => {
            let lock_path = args.out.join(LOCK_FILE);
            let lock = read_json(&lock_path)?;
            let lock_sha256 =
                sha256_file(&lock_path).map_err(|e| format!("hash {LOCK_FILE}: {e}"))?;
            run_analysis_stage(AnalysisInput {
                mode: Mode::Confirm,
                out_dir: &args.out,
                manifest_dir: &args.manifest_dir,
                file_names: &file_names,
                manifest_hashes: &manifest_hashes,
                selected: &selected,
                scarce_mut0: &scarce_mut0,
                scarce_drift: &scarce_drift,
                plan: &plan,
                duration_ticks: plan.duration_ticks,
                warmup_ticks: plan.warmup_ticks,
                registry: &registry,
                lock: Some(&lock),
                lock_sha256: Some(&lock_sha256),
            })
        }
        Mode::PipelineCheck => {
            // Build in-memory 2,000-tick pipeline-check manifests with disjoint seeds.
            let pc_duration = plan.pipeline_duration_ticks;
            let pc_warmup = plan.pipeline_warmup_ticks;
            let mut pc_baseline = baseline_ref.clone();
            pc_baseline.seeds = plan.pipeline_baseline_seeds.clone();
            pc_baseline.duration_ticks = pc_duration;
            pc_baseline.sample_period = plan.pipeline_smoke_sample_period;

            let mut pc_smoke_sel = smoke_selected.clone();
            pc_smoke_sel.seeds = vec![plan.pipeline_smoke_seed];
            pc_smoke_sel.duration_ticks = pc_duration;
            pc_smoke_sel.sample_period = plan.pipeline_smoke_sample_period;

            let mut pc_smoke_mut0 = smoke_mut0.clone();
            pc_smoke_mut0.seeds = vec![plan.pipeline_smoke_seed];
            pc_smoke_mut0.duration_ticks = pc_duration;
            pc_smoke_mut0.sample_period = plan.pipeline_smoke_sample_period;

            let mut pc_smoke_drift = smoke_drift.clone();
            pc_smoke_drift.seeds = vec![plan.pipeline_smoke_seed];
            pc_smoke_drift.duration_ticks = pc_duration;
            pc_smoke_drift.sample_period = plan.pipeline_smoke_sample_period;

            let smoke_dir = args.out.join("smoke");
            std::fs::create_dir_all(&smoke_dir)
                .map_err(|e| format!("create {}: {e}", smoke_dir.display()))?;
            let _gate = run_smoke_stage(
                Mode::PipelineCheck,
                &pc_baseline,
                &pc_smoke_sel,
                &pc_smoke_mut0,
                &pc_smoke_drift,
                &registry,
                &smoke_dir,
                &file_names,
                &manifest_hashes,
                &plan,
                pc_duration,
            )?;

            let mut pc_sel = selected.clone();
            pc_sel.seeds = plan.pipeline_pair_seeds.clone();
            pc_sel.duration_ticks = pc_duration;
            pc_sel.sample_period = plan.pipeline_pair_sample_period;

            let mut pc_mut0 = scarce_mut0.clone();
            pc_mut0.seeds = plan.pipeline_pair_seeds.clone();
            pc_mut0.duration_ticks = pc_duration;
            pc_mut0.sample_period = plan.pipeline_pair_sample_period;

            let mut pc_drift = scarce_drift.clone();
            pc_drift.seeds = plan.pipeline_pair_seeds.clone();
            pc_drift.duration_ticks = pc_duration;
            pc_drift.sample_period = plan.pipeline_pair_sample_period;

            run_analysis_stage(AnalysisInput {
                mode: Mode::PipelineCheck,
                out_dir: &args.out,
                manifest_dir: &args.manifest_dir,
                file_names: &file_names,
                manifest_hashes: &manifest_hashes,
                selected: &pc_sel,
                scarce_mut0: &pc_mut0,
                scarce_drift: &pc_drift,
                plan: &plan,
                duration_ticks: pc_duration,
                warmup_ticks: pc_warmup,
                registry: &registry,
                lock: None,
                lock_sha256: None,
            })
        }
    }
}

// ---- Smoke stage (blind to trait_mass_*) -------------------------------------------------------

/// Blind representation of one smoke or baseline run. Contains ZERO `trait_mass_*` data.
struct BlindRun {
    arm: &'static str,
    seed: u64,
    status: RunStatus,
    checksum: u32,
    elapsed_secs: f64,
    extinct_tick: Option<u64>,
    min_agents: Option<f64>,
    mean_agents: Option<f64>,
    crop_second_half: Option<f64>,
    total_births: f64,
    total_deaths: f64,
    mean_lifespan: Option<f64>,
    total_starvation: Option<f64>,
    total_dehydration: Option<f64>,
    cause_partition_ok: Option<bool>,
    births_last_quarter: f64,
    deaths_last_quarter: f64,
    /// `(tick, [SMOKE_ALLOWED_NON_DRIFT values])` — columns 6..8 are `None` for `scarce_drift`.
    series: Vec<(u64, [Option<f64>; 8])>,
}

fn guard_no_body_mass(what: &str, text: &str) -> Result<(), String> {
    if text.contains("trait_mass") {
        Err(format!(
            "smoke blindness violation: {what} names a body-mass observable"
        ))
    } else {
        Ok(())
    }
}

fn say_blind(line: &str) -> Result<(), String> {
    guard_no_body_mass("stdout", line)?;
    println!("{line}");
    Ok(())
}

fn scrub_reason(text: &str) -> String {
    if text.contains("trait_mass") {
        "reason withheld in smoke (names body-mass observable)".to_string()
    } else {
        csv_field(text)
    }
}

fn strip_to_blind_run(
    arm: &'static str,
    seed: u64,
    run: RunResult,
    allow_cause_split: bool,
    duration_ticks: u64,
    elapsed_secs: f64,
) -> BlindRun {
    let allowed: &[&str] = if allow_cause_split {
        &SMOKE_ALLOWED_NON_DRIFT
    } else {
        &SMOKE_ALLOWED_DRIFT
    };

    let mut series = Vec::with_capacity(run.series.len());
    let mut min_agents: Option<f64> = None;
    let mut agents_sum = 0.0;
    let mut agents_n = 0u64;
    let mut extinct_tick: Option<u64> = None;
    let mut crop_sum = 0.0;
    let mut crop_n = 0u64;
    let mut total_births = 0.0;
    let mut total_deaths = 0.0;
    let mut lifespan_weighted = 0.0;
    let mut total_starvation = 0.0;
    let mut total_dehydration = 0.0;
    let mut per_sample_partition_ok = true;
    let mut births_last_quarter = 0.0;
    let mut deaths_last_quarter = 0.0;

    for sample in &run.series {
        let mut row = [None; 8];
        for (idx, id) in SMOKE_ALLOWED_NON_DRIFT.iter().enumerate() {
            if allowed.contains(id) {
                row[idx] = sample
                    .observables
                    .iter()
                    .find(|(k, _)| k == id)
                    .map(|(_, v)| *v);
            }
        }
        if let Some(n) = row[1] {
            min_agents = Some(min_agents.map_or(n, |m| m.min(n)));
            agents_sum += n;
            agents_n += 1;
            if n == 0.0 && extinct_tick.is_none() {
                extinct_tick = Some(sample.tick);
            }
        }
        if sample.tick.saturating_mul(2) > duration_ticks {
            if let Some(c) = row[2] {
                crop_sum += c;
                crop_n += 1;
            }
        }
        let b = row[3].unwrap_or(0.0);
        let d = row[4].unwrap_or(0.0);
        let ls = row[5].unwrap_or(0.0);
        total_births += b;
        total_deaths += d;
        lifespan_weighted += ls * d;
        if sample.tick.saturating_mul(4) > duration_ticks.saturating_mul(3) {
            births_last_quarter += b;
            deaths_last_quarter += d;
        }
        if allow_cause_split {
            let s = row[6].unwrap_or(0.0);
            let h = row[7].unwrap_or(0.0);
            total_starvation += s;
            total_dehydration += h;
            if ((s + h) - d).abs() > CAUSE_PARTITION_TOLERANCE {
                per_sample_partition_ok = false;
            }
        }
        series.push((sample.tick, row));
    }

    if extinct_tick.is_none() && run.observable(COUNT_OBSERVABLE) == Some(0.0) {
        extinct_tick = Some(duration_ticks);
    }

    let mean_agents = if agents_n > 0 {
        Some(agents_sum / agents_n as f64)
    } else {
        None
    };
    let crop_second_half = if crop_n > 0 {
        Some(crop_sum / crop_n as f64)
    } else {
        None
    };
    let mean_lifespan = if total_deaths > 0.0 {
        Some(lifespan_weighted / total_deaths)
    } else {
        None
    };
    let (total_starvation, total_dehydration, cause_partition_ok) = if allow_cause_split {
        let total_ok = per_sample_partition_ok
            && ((total_starvation + total_dehydration) - total_deaths).abs()
                <= CAUSE_PARTITION_TOLERANCE;
        (
            Some(total_starvation),
            Some(total_dehydration),
            Some(total_ok),
        )
    } else {
        (None, None, None)
    };

    BlindRun {
        arm,
        seed,
        status: run.status,
        checksum: run.final_checksum,
        elapsed_secs,
        extinct_tick,
        min_agents,
        mean_agents,
        crop_second_half,
        total_births,
        total_deaths,
        mean_lifespan,
        total_starvation,
        total_dehydration,
        cause_partition_ok,
        births_last_quarter,
        deaths_last_quarter,
        series,
    }
}

fn to_scarce_gate_input(r: &BlindRun) -> ScarceGateInput {
    ScarceGateInput {
        arm: r.arm,
        completed: r.status.is_completed(),
        extinct_tick: r.extinct_tick,
        min_agents: r.min_agents,
        mean_agents: r.mean_agents,
        crop_second_half: r.crop_second_half,
        total_births: r.total_births,
        total_deaths: r.total_deaths,
        mean_lifespan: r.mean_lifespan,
        total_starvation: r.total_starvation,
        total_dehydration: r.total_dehydration,
        cause_partition_ok: r.cause_partition_ok,
        births_last_quarter: r.births_last_quarter,
        deaths_last_quarter: r.deaths_last_quarter,
    }
}

#[allow(clippy::too_many_arguments)]
fn run_smoke_stage(
    mode: Mode,
    baseline_manifest: &ExperimentManifest,
    smoke_selected: &ExperimentManifest,
    smoke_mut0: &ExperimentManifest,
    smoke_drift: &ExperimentManifest,
    registry: &ObservableRegistry,
    smoke_dir: &Path,
    file_names: &[String],
    manifest_hashes: &BTreeMap<String, String>,
    plan: &Plan,
    duration_ticks: u64,
) -> Result<GateReport, String> {
    // QD-SEL2g constraint #4: compute pooled baseline strictly BEFORE running/reading any scarce run.
    let baseline_started = SystemTime::now();
    let mut baseline_runs = Vec::with_capacity(baseline_manifest.seeds.len());
    for &seed in &baseline_manifest.seeds {
        let clock = Instant::now();
        let res = run_manifest_seed::<LiveExperimentAdapter>(
            baseline_manifest,
            registry,
            seed,
            None,
            None,
        );
        let elapsed_secs = clock.elapsed().as_secs_f64();
        let blind = strip_to_blind_run(
            "baseline_ref",
            seed,
            res,
            true,
            duration_ticks,
            elapsed_secs,
        );
        if let RunStatus::Failed { tick, reason, .. } = &blind.status {
            say_blind(&format!(
                "SEL2-P smoke baseline_ref seed {seed} FAILED at tick {tick}: {}",
                scrub_reason(reason)
            ))?;
        }
        baseline_runs.push(blind);
    }
    let baseline_ended = SystemTime::now();

    let baseline_obs: Vec<BaselineObservation> = baseline_runs
        .iter()
        .map(|r| BaselineObservation {
            seed: r.seed,
            completed: r.status.is_completed(),
            extinct_tick: r.extinct_tick,
            crop_second_half: r.crop_second_half,
        })
        .collect();
    let pooled = compute_pooled_baseline(&baseline_obs, plan.min_surviving_baseline_seeds);

    // Scarce smoke runs happen AFTER `pooled` is locked.
    let scarce_started = SystemTime::now();
    let smoke_seed = smoke_selected.seeds[0];
    let mut sel_for_drift = smoke_selected.clone();
    sel_for_drift.observable_ids = smoke_drift.observable_ids.clone();

    let drift_clock = Instant::now();
    let pair = run_live_drift_pair(&sel_for_drift, smoke_drift, registry, smoke_seed)
        .map_err(|e| format!("smoke drift pair failed: {e}"))?;
    let pair_elapsed = drift_clock.elapsed().as_secs_f64();
    let drift_diag = pair.diagnostics.clone();
    let sel_blind = strip_to_blind_run(
        "selected",
        smoke_seed,
        pair.selected,
        true,
        duration_ticks,
        pair_elapsed * 0.5,
    );
    let drift_blind = strip_to_blind_run(
        "scarce_drift",
        smoke_seed,
        pair.drift,
        false,
        duration_ticks,
        pair_elapsed * 0.5,
    );

    let mut0_clock = Instant::now();
    let mut0_res =
        run_manifest_seed::<LiveExperimentAdapter>(smoke_mut0, registry, smoke_seed, None, None);
    let mut0_elapsed = mut0_clock.elapsed().as_secs_f64();
    let mut0_blind = strip_to_blind_run(
        "scarce_mut0",
        smoke_seed,
        mut0_res,
        true,
        duration_ticks,
        mut0_elapsed,
    );
    let scarce_ended = SystemTime::now();

    let gate = evaluate_smoke_gate(
        duration_ticks,
        &pooled,
        &to_scarce_gate_input(&sel_blind),
        &to_scarce_gate_input(&mut0_blind),
        &to_scarce_gate_input(&drift_blind),
    );

    let all_runs: Vec<&BlindRun> = baseline_runs
        .iter()
        .chain([&sel_blind, &mut0_blind, &drift_blind])
        .collect();

    say_blind(&format!(
        "SEL2-P {} smoke: {} baseline + 3 scarce runs x {duration_ticks} ticks; \
         baseline surviving {}/{} (min {}), pooled_crop={:?}; \
         gate C1={} C2={} C3={} C4={} C5={} drift_valid={} -> pass={}",
        mode.as_str(),
        baseline_runs.len(),
        pooled.n_surviving,
        pooled.n_requested,
        pooled.min_required,
        pooled.pooled_crop_second_half,
        gate.c1,
        gate.c2,
        gate.c3,
        gate.c4,
        gate.c5,
        gate.drift_valid,
        gate.gate_pass,
    ))?;

    let mut runs_csv = String::from(
        "# SMOKE: allow-listed observables only; zero body-mass observables read or written\n\
         seed,arm,status,extinct_tick,min_agent_count,mean_agent_count,crop_second_half,\
         total_births,total_deaths,mean_lifespan,total_starvation,total_dehydration,\
         cause_partition_ok,births_last_quarter,deaths_last_quarter,us_per_tick,final_checksum\n",
    );
    for r in &all_runs {
        runs_csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{}\n",
            r.seed,
            r.arm,
            status_label(&r.status),
            opt_u64(r.extinct_tick),
            opt_f64(r.min_agents),
            opt_f64(r.mean_agents),
            opt_f64(r.crop_second_half),
            r.total_births,
            r.total_deaths,
            opt_f64(r.mean_lifespan),
            opt_f64(r.total_starvation),
            opt_f64(r.total_dehydration),
            r.cause_partition_ok
                .map(|b| b.to_string())
                .unwrap_or_default(),
            r.births_last_quarter,
            r.deaths_last_quarter,
            r.elapsed_secs * 1e6 / duration_ticks as f64,
            r.checksum,
        ));
    }

    let mut series_csv = String::from("seed,arm,tick");
    for id in SMOKE_ALLOWED_NON_DRIFT {
        series_csv.push(',');
        series_csv.push_str(id);
    }
    series_csv.push('\n');
    for r in &all_runs {
        for (tick, vals) in &r.series {
            series_csv.push_str(&format!("{},{},{tick}", r.seed, r.arm));
            for v in vals {
                series_csv.push(',');
                if let Some(x) = v {
                    series_csv.push_str(&x.to_string());
                }
            }
            series_csv.push('\n');
        }
    }

    let run_json = |r: &BlindRun| {
        serde_json::json!({
            "arm": r.arm,
            "seed": r.seed,
            "status": status_label(&r.status),
            "extinct_tick": r.extinct_tick,
            "min_agents": r.min_agents,
            "mean_agents": r.mean_agents,
            "crop_second_half": r.crop_second_half,
            "total_births": r.total_births,
            "total_deaths": r.total_deaths,
            "mean_lifespan": r.mean_lifespan,
            "total_starvation": r.total_starvation,
            "total_dehydration": r.total_dehydration,
            "cause_partition_ok": r.cause_partition_ok,
            "births_last_quarter": r.births_last_quarter,
            "deaths_last_quarter": r.deaths_last_quarter,
            "us_per_tick": r.elapsed_secs * 1e6 / duration_ticks as f64,
            "final_checksum": r.checksum,
        })
    };

    let report_json = serde_json::json!({
        "kind": if mode == Mode::Smoke {
            "sel2p-smoke-report-EXCLUDED-FROM-CONFIRMATION"
        } else {
            "sel2p-pipeline-check-smoke-report-EXCLUDED-FROM-CONFIRMATION"
        },
        "mode": mode.as_str(),
        "contains_body_mass": false,
        "baseline_computed_before_scarce": true,
        "duration_ticks": duration_ticks,
        "sample_period": smoke_selected.sample_period,
        "observables_written_non_drift": SMOKE_ALLOWED_NON_DRIFT,
        "observables_written_drift": SMOKE_ALLOWED_DRIFT,
        "pooled_baseline": {
            "n_requested": pooled.n_requested,
            "n_surviving": pooled.n_surviving,
            "min_required": pooled.min_required,
            "surviving_seeds": pooled.surviving_seeds,
            "pooled_crop_second_half": pooled.pooled_crop_second_half,
            "sufficient": pooled.sufficient,
        },
        "gate": {
            "c1_min_agents_ge_5": gate.c1,
            "c2_g_in_0_4_to_0_6": gate.c2,
            "c3_generations_ge_30": gate.c3,
            "c4_starvation_share_gt_0_5_non_drift_only": gate.c4,
            "c5_last_quarter_active": gate.c5,
            "drift_valid": gate.drift_valid,
            "gate_pass": gate.gate_pass,
            "g_selected": gate.g_selected,
            "g_scarce_mut0": gate.g_scarce_mut0,
            "g_scarce_drift_diagnostic": gate.g_scarce_drift,
            "generations_selected": gate.generations_selected,
            "generations_scarce_mut0": gate.generations_scarce_mut0,
            "starvation_share_selected": gate.starvation_share_selected,
            "starvation_share_scarce_mut0": gate.starvation_share_scarce_mut0,
        },
        "drift_diagnostics": drift_diag_json(&drift_diag),
        "runs": all_runs.iter().map(|r| run_json(r)).collect::<Vec<_>>(),
    });

    let exe = std::env::current_exe().ok();
    let prov_json = serde_json::json!({
        "kind": if mode == Mode::Smoke {
            "sel2p-smoke-provenance-EXCLUDED-FROM-CONFIRMATION"
        } else {
            "sel2p-pipeline-check-smoke-provenance-EXCLUDED-FROM-CONFIRMATION"
        },
        "mode": mode.as_str(),
        "contains_body_mass": false,
        "baseline_computed_before_scarce": true,
        "model": "core::live_experiment::LiveExperimentAdapter",
        "model_version": LIVE_MODEL_VERSION,
        "model_status": "headless adapter verified; NOT a live-world experiment-ready claim",
        "build": {
            "git_commit": git(&["rev-parse", "HEAD"]),
            "git_describe": git(&["describe", "--always", "--dirty"]),
            "git_tree_dirty": git(&["status", "--porcelain"]).map(|s| !s.trim().is_empty()),
            "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "rustc": tool_version("rustc"),
            "package_version": env!("CARGO_PKG_VERSION"),
            "binary_sha256": exe.as_ref().and_then(|p| sha256_file(p).ok()),
        },
        "host": { "os": std::env::consts::OS, "arch": std::env::consts::ARCH },
        "timing": {
            "baseline_started_utc": iso8601_utc(unix_seconds(baseline_started)),
            "baseline_ended_utc": iso8601_utc(unix_seconds(baseline_ended)),
            "scarce_started_utc": iso8601_utc(unix_seconds(scarce_started)),
            "scarce_ended_utc": iso8601_utc(unix_seconds(scarce_ended)),
        },
        "duration": {
            "duration_ticks_as_run": duration_ticks,
            "sample_period": smoke_selected.sample_period,
        },
        "manifests": {
            "files": file_names,
            "sha256_as_read": manifest_hashes,
            "note": "manifests are not copied into smoke/ because confirmation manifests name body-mass observables",
        },
        "seeds": {
            "baseline_seeds": baseline_manifest.seeds,
            "scarce_smoke_seed": smoke_seed,
        },
        "observables_written_non_drift": SMOKE_ALLOWED_NON_DRIFT,
        "observables_written_drift": SMOKE_ALLOWED_DRIFT,
    });

    let summary_md = format!(
        "# SEL2-P smoke ({})\n\n\
         > Blind to body mass: zero body-mass observables were read or written.\n\n\
         - duration: **{duration_ticks} ticks**, sample period `{}`\n\
         - baseline (`prey_cap = 80, scale = 1.0`): surviving **{}/{}** (min `{}`), pooled crop `{:?}`\n\
         - gates: `C1={}`, `C2={}`, `C3={}`, `C4={}`, `C5={}`, `drift_valid={}` -> **`gate_pass={}`**\n\
         - `g`: selected `{:?}`, scarce_mut0 `{:?}`, scarce_drift (diagnostic) `{:?}`\n\
         - `generations`: selected `{:?}`, scarce_mut0 `{:?}`\n\
         - `starvation_share` (C4 non-drift only): selected `{:?}`, scarce_mut0 `{:?}`\n",
        mode.as_str(),
        smoke_selected.sample_period,
        pooled.n_surviving,
        pooled.n_requested,
        pooled.min_required,
        pooled.pooled_crop_second_half,
        gate.c1,
        gate.c2,
        gate.c3,
        gate.c4,
        gate.c5,
        gate.drift_valid,
        gate.gate_pass,
        gate.g_selected,
        gate.g_scarce_mut0,
        gate.g_scarce_drift,
        gate.generations_selected,
        gate.generations_scarce_mut0,
        gate.starvation_share_selected,
        gate.starvation_share_scarce_mut0,
    );

    let report_text = pretty(&report_json)? + "\n";
    let prov_text = pretty(&prov_json)? + "\n";
    for (name, text) in [
        ("runs.csv", &runs_csv),
        ("smoke-series.csv", &series_csv),
        ("smoke-report.json", &report_text),
        ("provenance.json", &prov_text),
        ("summary.md", &summary_md),
    ] {
        guard_no_body_mass(name, text)?;
    }

    write_text(&smoke_dir.join("runs.csv"), &runs_csv)?;
    write_text(&smoke_dir.join("smoke-series.csv"), &series_csv)?;
    write_text(&smoke_dir.join("smoke-report.json"), &report_text)?;
    write_text(&smoke_dir.join("provenance.json"), &prov_text)?;
    write_text(&smoke_dir.join("summary.md"), &summary_md)?;
    write_checksums(
        smoke_dir,
        &[
            "provenance.json",
            "runs.csv",
            "smoke-report.json",
            "smoke-series.csv",
            "summary.md",
        ],
    )?;

    Ok(gate)
}

// ---- Analysis stage (confirm / pipeline-check paired ensemble) ---------------------------------

struct AnalysisInput<'a> {
    mode: Mode,
    out_dir: &'a Path,
    manifest_dir: &'a Path,
    file_names: &'a [String],
    manifest_hashes: &'a BTreeMap<String, String>,
    selected: &'a ExperimentManifest,
    scarce_mut0: &'a ExperimentManifest,
    scarce_drift: &'a ExperimentManifest,
    plan: &'a Plan,
    duration_ticks: u64,
    warmup_ticks: u64,
    registry: &'a ObservableRegistry,
    lock: Option<&'a serde_json::Value>,
    lock_sha256: Option<&'a str>,
}

struct ArmRunSummary {
    seed: u64,
    arm: &'static str,
    completed: bool,
    extinct_tick: Option<u64>,
    post_warmup_mass_mean: Option<f64>,
    post_warmup_energy_mean: Option<f64>,
    tail20_mass_mean: Option<f64>,
    closed_eu_relative_spread: Option<f64>,
    births_total: f64,
    min_agents: Option<f64>,
    max_agents: Option<f64>,
    mut0_mass_var_max: Option<f64>,
    window_net_mass_delta: Option<f64>,
}

struct DroppedRun {
    seed: u64,
    arm: &'static str,
    tick: Option<u64>,
    reason: String,
}

struct ContrastEffect {
    id: &'static str,
    contrast: &'static str,
    statistic: &'static str,
    arm_order: &'static str,
    effect: PairedEffect,
    rows: Vec<(u64, f64, f64, f64)>,
    median_delta: Option<f64>,
}

impl ContrastEffect {
    fn materiality(&self) -> (bool, bool, bool) {
        let e = &self.effect;
        let ci = match (e.ci95_low, e.ci95_high) {
            (Some(lo), Some(hi)) => lo > 0.0 || hi < 0.0,
            _ => false,
        };
        let dz = e.paired_dz.is_some_and(|d| d.abs() >= MATERIALITY_DZ);
        let rel = match (e.paired_mean_delta, e.control_mean) {
            (Some(d), Some(c)) => d.abs() >= MATERIALITY_RELATIVE_SHIFT * c.abs(),
            _ => false,
        };
        (ci, dz, rel)
    }

    fn is_material(&self) -> bool {
        let (ci, dz, rel) = self.materiality();
        ci && dz && rel
    }

    fn relative_shift(&self) -> Option<f64> {
        match (self.effect.paired_mean_delta, self.effect.control_mean) {
            (Some(d), Some(c)) if c != 0.0 => Some(d / c.abs()),
            _ => None,
        }
    }
}

fn series_of(series: &[StateSample], id: &str) -> Vec<(u64, f64)> {
    series
        .iter()
        .filter_map(|s| {
            s.observables
                .iter()
                .find(|(k, _)| k == id)
                .map(|(_, v)| (s.tick, *v))
        })
        .collect()
}

fn summarise_analysis_run(
    arm: &'static str,
    seed: u64,
    run: &RunResult,
    duration_ticks: u64,
    warmup_ticks: u64,
    window_net_mass_delta: Option<f64>,
) -> ArmRunSummary {
    let counts = series_of(&run.series, COUNT_OBSERVABLE);
    let mut min_agents: Option<f64> = None;
    let mut max_agents: Option<f64> = None;
    let mut extinct_tick: Option<u64> = None;
    for &(tick, n) in &counts {
        min_agents = Some(min_agents.map_or(n, |m| m.min(n)));
        max_agents = Some(max_agents.map_or(n, |m| m.max(n)));
        if n == 0.0 && extinct_tick.is_none() {
            extinct_tick = Some(tick);
        }
    }
    if extinct_tick.is_none() && run.observable(COUNT_OBSERVABLE) == Some(0.0) {
        extinct_tick = Some(duration_ticks);
    }

    let mass_series = series_of(&run.series, PRIMARY_OBSERVABLE);
    let energy_series = series_of(&run.series, ENERGY_INTEGRAL_OBSERVABLE);
    let eu_series = series_of(&run.series, INTEGRITY_OBSERVABLE);
    let births_total = series_of(&run.series, BIRTHS_OBSERVABLE)
        .iter()
        .map(|(_, b)| *b)
        .sum::<f64>();
    let mut0_mass_var_max = series_of(&run.series, MASS_VAR_OBSERVABLE)
        .iter()
        .map(|(_, v)| *v)
        .reduce(f64::max);

    let closed_eu_relative_spread = if eu_series.is_empty() {
        None
    } else {
        let hi = eu_series.iter().map(|(_, v)| *v).fold(f64::MIN, f64::max);
        let lo = eu_series.iter().map(|(_, v)| *v).fold(f64::MAX, f64::min);
        Some((hi - lo) / hi.abs().max(1.0))
    };

    ArmRunSummary {
        seed,
        arm,
        completed: run.status.is_completed(),
        extinct_tick,
        post_warmup_mass_mean: post_warmup_mean(&mass_series, warmup_ticks),
        post_warmup_energy_mean: post_warmup_energy_mean(
            &energy_series,
            warmup_ticks,
            duration_ticks,
        ),
        tail20_mass_mean: tail20_mean(&mass_series, duration_ticks),
        closed_eu_relative_spread,
        births_total,
        min_agents,
        max_agents,
        mut0_mass_var_max,
        window_net_mass_delta,
    }
}

struct ContrastSpec {
    id: &'static str,
    contrast: &'static str,
    statistic: &'static str,
    arm_order: &'static str,
    n_requested: usize,
}

fn build_contrast<F>(
    spec: ContrastSpec,
    left: &[ArmRunSummary],
    right: &[ArmRunSummary],
    metric: F,
) -> Result<ContrastEffect, String>
where
    F: Fn(&ArmRunSummary) -> Option<f64>,
{
    let mut seeds = Vec::new();
    let mut controls = Vec::new();
    let mut treatments = Vec::new();
    let mut deltas = Vec::new();
    for (c_run, t_run) in left.iter().zip(right.iter()) {
        debug_assert_eq!(c_run.seed, t_run.seed);
        let usable = |r: &ArmRunSummary| r.completed && r.extinct_tick.is_none();
        if usable(c_run) && usable(t_run) {
            if let (Some(cv), Some(tv)) = (metric(c_run), metric(t_run)) {
                seeds.push(c_run.seed);
                controls.push(cv);
                treatments.push(tv);
                deltas.push(tv - cv);
            }
        }
    }
    let effect = paired_effect(
        spec.statistic,
        spec.n_requested,
        &controls,
        &treatments,
        &deltas,
    )
    .map_err(|e| format!("{}: paired_effect refused: {e}", spec.contrast))?;
    let rows = seeds
        .iter()
        .zip(controls.iter().zip(treatments.iter().zip(deltas.iter())))
        .map(|(&s, (&c, (&t, &d)))| (s, c, t, d))
        .collect();
    Ok(ContrastEffect {
        id: spec.id,
        contrast: spec.contrast,
        statistic: spec.statistic,
        arm_order: spec.arm_order,
        effect,
        rows,
        median_delta: median(&deltas),
    })
}

fn run_analysis_stage(input: AnalysisInput<'_>) -> Result<(), String> {
    let started = SystemTime::now();
    let clock = Instant::now();
    let drift_ensemble: DriftEnsembleReport =
        run_live_drift_ensemble(input.selected, input.scarce_drift, input.registry)
            .map_err(|e| format!("run_live_drift_ensemble refused to run: {e}"))?;

    let mut mut0_runs = Vec::with_capacity(input.scarce_mut0.seeds.len());
    for &seed in &input.scarce_mut0.seeds {
        mut0_runs.push(run_manifest_seed::<LiveExperimentAdapter>(
            input.scarce_mut0,
            input.registry,
            seed,
            None,
            None,
        ));
    }
    let elapsed_secs = clock.elapsed().as_secs_f64();
    let ended = SystemTime::now();

    let report: &PairedEnsembleReport = &drift_ensemble.ensemble;
    let n_requested = report.pairs.len();

    let mut sel_summaries = Vec::with_capacity(n_requested);
    let mut drift_summaries = Vec::with_capacity(n_requested);
    let mut mut0_summaries = Vec::with_capacity(n_requested);

    for (i, pair) in report.pairs.iter().enumerate() {
        let win_delta = drift_ensemble
            .window_net_selection_by_seed
            .get(i)
            .and_then(|(_, map)| map.get(PRIMARY_OBSERVABLE).copied());
        sel_summaries.push(summarise_analysis_run(
            "selected",
            pair.seed,
            &pair.treatment,
            input.duration_ticks,
            input.warmup_ticks,
            win_delta,
        ));
        drift_summaries.push(summarise_analysis_run(
            "scarce_drift",
            pair.seed,
            &pair.control,
            input.duration_ticks,
            input.warmup_ticks,
            None,
        ));
        mut0_summaries.push(summarise_analysis_run(
            "scarce_mut0",
            pair.seed,
            &mut0_runs[i],
            input.duration_ticks,
            input.warmup_ticks,
            None,
        ));
    }

    let mut dropped_extinct = Vec::new();
    let mut dropped_failed = Vec::new();
    let mut extinct_by_arm: BTreeMap<&'static str, usize> =
        BTreeMap::from([("selected", 0), ("scarce_drift", 0), ("scarce_mut0", 0)]);

    let mut classify = |arm: &'static str, seed: u64, run: &RunResult, sum: &ArmRunSummary| {
        if let RunStatus::Failed { tick, reason, .. } = &run.status {
            dropped_failed.push(DroppedRun {
                seed,
                arm,
                tick: Some(*tick),
                reason: reason.clone(),
            });
        } else if let Some(t) = sum.extinct_tick {
            *extinct_by_arm.entry(arm).or_insert(0) += 1;
            dropped_extinct.push(DroppedRun {
                seed,
                arm,
                tick: Some(t),
                reason: "live.agent_count reached 0".into(),
            });
        }
    };

    for (i, pair) in report.pairs.iter().enumerate() {
        classify("selected", pair.seed, &pair.treatment, &sel_summaries[i]);
        classify(
            "scarce_drift",
            pair.seed,
            &pair.control,
            &drift_summaries[i],
        );
        classify("scarce_mut0", pair.seed, &mut0_runs[i], &mut0_summaries[i]);
    }

    let h1 = build_contrast(
        ContrastSpec {
            id: "H1",
            contrast: "H1_selected_vs_scarce_drift",
            statistic: POST_WARMUP_STAT,
            arm_order: "selected - scarce_drift",
            n_requested,
        },
        &drift_summaries,
        &sel_summaries,
        |s| s.post_warmup_mass_mean,
    )?;
    let h2a = build_contrast(
        ContrastSpec {
            id: "H2a",
            contrast: "H2a_selected_vs_scarce_mut0",
            statistic: POST_WARMUP_STAT,
            arm_order: "selected - scarce_mut0",
            n_requested,
        },
        &mut0_summaries,
        &sel_summaries,
        |s| s.post_warmup_mass_mean,
    )?;
    let h2b = build_contrast(
        ContrastSpec {
            id: "H2b",
            contrast: "H2b_scarce_drift_vs_scarce_mut0",
            statistic: POST_WARMUP_STAT,
            arm_order: "scarce_drift - scarce_mut0",
            n_requested,
        },
        &mut0_summaries,
        &drift_summaries,
        |s| s.post_warmup_mass_mean,
    )?;
    let r1_tail20 = build_contrast(
        ContrastSpec {
            id: "R1_tail20",
            contrast: "R1_tail20_selected_vs_scarce_drift",
            statistic: TAIL20_STAT,
            arm_order: "selected - scarce_drift",
            n_requested,
        },
        &drift_summaries,
        &sel_summaries,
        |s| s.tail20_mass_mean,
    )?;
    let s2_energy = build_contrast(
        ContrastSpec {
            id: "S2_energy",
            contrast: "S2_energy_selected_vs_scarce_drift",
            statistic: ENERGY_POST_WARMUP_STAT,
            arm_order: "selected - scarce_drift",
            n_requested,
        },
        &drift_summaries,
        &sel_summaries,
        |s| s.post_warmup_energy_mean,
    )?;

    // R2: window_net_selection_deltas["live.trait_mass_mean"] over complete H1 pairs.
    let mut r2_seeds = Vec::new();
    let mut r2_ctrl = Vec::new();
    let mut r2_trt = Vec::new();
    let mut r2_deltas = Vec::new();
    for (s_run, d_run) in sel_summaries.iter().zip(drift_summaries.iter()) {
        if s_run.completed
            && d_run.completed
            && s_run.extinct_tick.is_none()
            && d_run.extinct_tick.is_none()
        {
            if let Some(delta) = s_run.window_net_mass_delta {
                r2_seeds.push(s_run.seed);
                r2_ctrl.push(0.0);
                r2_trt.push(delta);
                r2_deltas.push(delta);
            }
        }
    }
    let r2_effect = paired_effect(WINDOW_NET_STAT, n_requested, &r2_ctrl, &r2_trt, &r2_deltas)
        .map_err(|e| format!("R2_window_net: paired_effect refused: {e}"))?;
    let r2_window_net = ContrastEffect {
        id: "R2_window_net",
        contrast: "R2_window_net_selected_vs_scarce_drift",
        statistic: WINDOW_NET_STAT,
        arm_order: "window_net_selection_delta",
        effect: r2_effect,
        rows: r2_seeds
            .iter()
            .zip(r2_deltas.iter())
            .map(|(&s, &d)| (s, 0.0, d, d))
            .collect(),
        median_delta: median(&r2_deltas),
    };

    let mut integrity_findings = Vec::new();
    for sum in sel_summaries
        .iter()
        .chain(drift_summaries.iter())
        .chain(mut0_summaries.iter())
    {
        if let Some(sp) = sum.closed_eu_relative_spread {
            if sp > CLOSED_EU_TOLERANCE {
                integrity_findings.push(format!(
                    "{} seed {}: live.closed_eu_total spread {sp:.3e} exceeds {CLOSED_EU_TOLERANCE:.0e}",
                    sum.arm, sum.seed
                ));
            }
        }
        if let (Some(max_n), Some(min_n)) = (sum.max_agents, sum.min_agents) {
            if sum.births_total == 0.0 && max_n > min_n {
                integrity_findings.push(format!(
                    "{} seed {}: population grew without live.births > 0",
                    sum.arm, sum.seed
                ));
            }
        }
        if sum.arm == "scarce_mut0" {
            if let Some(v) = sum.mut0_mass_var_max {
                if v > 1e-12 {
                    integrity_findings.push(format!(
                        "scarce_mut0 seed {}: live.trait_mass_var max {v:.3e} != 0",
                        sum.seed
                    ));
                }
            }
        }
    }

    let h1_pairs = h1.effect.n_complete_pairs;
    let sufficient_pairs = h1_pairs >= input.plan.min_complete_pairs;
    let decision = if input.mode == Mode::PipelineCheck {
        "pipeline_check_only"
    } else if !sufficient_pairs {
        "insufficient_evidence"
    } else if h1.is_material() && integrity_findings.is_empty() {
        "shift_in_model"
    } else {
        "null"
    };

    // Copy committed manifests verbatim into `<out>/manifests/` so the verifier checks byte identity.
    let manifests_dir = input.out_dir.join("manifests");
    std::fs::create_dir_all(&manifests_dir)
        .map_err(|e| format!("create {}: {e}", manifests_dir.display()))?;
    for name in input.file_names {
        let bytes = std::fs::read(input.manifest_dir.join(name))
            .map_err(|e| format!("read {name}: {e}"))?;
        std::fs::write(manifests_dir.join(name), bytes).map_err(|e| format!("copy {name}: {e}"))?;
    }

    write_text(
        &input.out_dir.join("paired-report.json"),
        &(pretty(&serde_json::to_value(report).map_err(|e| e.to_string())?)? + "\n"),
    )?;

    let mut per_seed_csv =
        String::from("seed,contrast,statistic,control_value,treatment_value,delta\n");
    for c in [&h1, &h2a, &h2b, &r1_tail20, &r2_window_net, &s2_energy] {
        for &(seed, cv, tv, d) in &c.rows {
            per_seed_csv.push_str(&format!(
                "{seed},{},{},{cv},{tv},{d}\n",
                c.contrast, c.statistic
            ));
        }
    }
    write_text(&input.out_dir.join("per-seed-deltas.csv"), &per_seed_csv)?;

    let all_ids: Vec<&'static str> = all_live_observable_ids();
    let mut runs_csv = String::from(
        "seed,arm,status,failed_tick,failure_reason,extinct,extinct_tick,\
         post_warmup_trait_mass_mean,post_warmup_mean_energy_integral,tail20_trait_mass_mean,\
         window_net_selection_trait_mass,final_checksum,warnings",
    );
    for id in &all_ids {
        runs_csv.push(',');
        runs_csv.push_str(id);
    }
    runs_csv.push('\n');
    let mut push_run_row = |arm: &str, seed: u64, run: &RunResult, sum: &ArmRunSummary| {
        let (tick, reason) = match &run.status {
            RunStatus::Completed => (String::new(), String::new()),
            RunStatus::Failed { tick, reason, .. } => (tick.to_string(), csv_field(reason)),
        };
        runs_csv.push_str(&format!(
            "{seed},{arm},{},{tick},{reason},{},{},{},{},{},{},{},{}",
            status_label(&run.status),
            sum.extinct_tick.is_some(),
            opt_u64(sum.extinct_tick),
            opt_f64(sum.post_warmup_mass_mean),
            opt_f64(sum.post_warmup_energy_mean),
            opt_f64(sum.tail20_mass_mean),
            opt_f64(sum.window_net_mass_delta),
            run.final_checksum,
            csv_field(&run.warnings.join(" | ")),
        ));
        for id in &all_ids {
            runs_csv.push(',');
            if let Some(v) = run.observable(id) {
                runs_csv.push_str(&v.to_string());
            }
        }
        runs_csv.push('\n');
    };
    for (i, pair) in report.pairs.iter().enumerate() {
        push_run_row("selected", pair.seed, &pair.treatment, &sel_summaries[i]);
        push_run_row(
            "scarce_drift",
            pair.seed,
            &pair.control,
            &drift_summaries[i],
        );
        push_run_row("scarce_mut0", pair.seed, &mut0_runs[i], &mut0_summaries[i]);
    }
    write_text(&input.out_dir.join("runs.csv"), &runs_csv)?;

    let descriptive: Vec<serde_json::Value> = report
        .effects
        .iter()
        .filter(|e| {
            DESCRIPTIVE_OBSERVABLES.contains(&e.observable.as_str())
                || e.observable == INTEGRITY_OBSERVABLE
        })
        .map(|e| serde_json::to_value(e).map_err(|x| x.to_string()))
        .collect::<Result<_, _>>()?;
    let drift_diags: Vec<serde_json::Value> = report
        .seed_order
        .iter()
        .zip(drift_ensemble.diagnostics.iter())
        .map(|(&seed, d)| {
            serde_json::json!({
                "seed": seed,
                "diagnostics": drift_diag_json(d),
            })
        })
        .collect();

    let dropped_json = |items: &[DroppedRun]| -> Vec<serde_json::Value> {
        items
            .iter()
            .map(|d| {
                serde_json::json!({
                    "seed": d.seed,
                    "arm": d.arm,
                    "tick": d.tick,
                    "reason": d.reason,
                })
            })
            .collect()
    };

    let effects_doc = serde_json::json!({
        "kind": if input.mode == Mode::Confirm {
            "sel2p-effects"
        } else {
            "sel2p-pipeline-check-effects-NOT-CONFIRMATION"
        },
        "mode": input.mode.as_str(),
        "contains_confirmation_results": input.mode == Mode::Confirm,
        "decision": decision,
        "primary_observable": PRIMARY_OBSERVABLE,
        "primary_statistic": POST_WARMUP_STAT,
        "materiality_rule": "all_of: ci95 excludes 0; |paired_dz| >= 0.66; |paired_mean_delta| >= 0.05 * |control_mean|",
        "min_complete_pairs_for_a_decision": input.plan.min_complete_pairs,
        "n_pairs_requested": n_requested,
        "n_complete_pairs_used_by_h1": h1_pairs,
        "sufficient_pairs": sufficient_pairs,
        "extinction": {
            "rule": "live.agent_count == 0 at any sample (or finally) drops the pair and records it here",
            "n_extinct_runs_by_arm": extinct_by_arm,
            "n_extinct_runs_total": dropped_extinct.len(),
            "extinct_runs": dropped_json(&dropped_extinct),
        },
        "failed_runs": dropped_json(&dropped_failed),
        "h1": contrast_json(&h1),
        "h2a_selected_vs_scarce_mut0": contrast_json(&h2a),
        "h2b_scarce_drift_vs_scarce_mut0": contrast_json(&h2b),
        "r1_tail20": contrast_json(&r1_tail20),
        "r2_window_net_selection": contrast_json(&r2_window_net),
        "s2_energy_post_warmup": contrast_json(&s2_energy),
        "drift_diagnostics_by_seed": drift_diags,
        "descriptive_final_value_effects_selected_vs_drift": descriptive,
        "integrity_findings": integrity_findings,
    });
    write_text(
        &input.out_dir.join("effects.json"),
        &(pretty(&effects_doc)? + "\n"),
    )?;

    let exe = std::env::current_exe().ok();
    let prov_doc = serde_json::json!({
        "kind": if input.mode == Mode::Confirm {
            "sel2p-provenance"
        } else {
            "sel2p-pipeline-check-provenance-NOT-CONFIRMATION"
        },
        "mode": input.mode.as_str(),
        "contains_confirmation_results": input.mode == Mode::Confirm,
        "model": "core::live_experiment::LiveExperimentAdapter",
        "model_version": LIVE_MODEL_VERSION,
        "model_status": "headless adapter verified; NOT a live-world experiment-ready claim",
        "harness": "core::live_experiment::run_live_drift_ensemble + core::experiment_runner::{run_manifest_seed, paired_effect}",
        "registry": "core::experiment::ObservableRegistry::live_default",
        "build": {
            "git_commit": git(&["rev-parse", "HEAD"]),
            "git_describe": git(&["describe", "--always", "--dirty"]),
            "git_tree_dirty": git(&["status", "--porcelain"]).map(|s| !s.trim().is_empty()),
            "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "rustc": tool_version("rustc"),
            "package_version": env!("CARGO_PKG_VERSION"),
            "binary_sha256": exe.as_ref().and_then(|p| sha256_file(p).ok()),
        },
        "host": { "os": std::env::consts::OS, "arch": std::env::consts::ARCH },
        "timing": {
            "started_utc": iso8601_utc(unix_seconds(started)),
            "ended_utc": iso8601_utc(unix_seconds(ended)),
            "wall_clock_seconds": elapsed_secs,
        },
        "duration": {
            "duration_ticks_as_run": input.duration_ticks,
            "warmup_ticks_as_run": input.warmup_ticks,
            "sample_period": input.selected.sample_period,
        },
        "execution_lock": input.lock,
        "execution_lock_sha256": input.lock_sha256,
        "manifests": {
            "sha256_as_committed": input.manifest_hashes,
            "selected_fingerprint_as_run": report.treatment_manifest_fingerprint,
            "scarce_drift_fingerprint_as_run": report.control_manifest_fingerprint,
            "scarce_mut0_fingerprint_as_run": input.scarce_mut0.fingerprint(),
            "registry_fingerprint": report.registry_fingerprint,
            "declared_factors": report.declared_factors,
            "selected_initial_conditions": input.selected.initial_conditions.values,
            "scarce_drift_initial_conditions": input.scarce_drift.initial_conditions.values,
            "scarce_mut0_initial_conditions": input.scarce_mut0.initial_conditions.values,
        },
        "seeds": {
            "execution_order": report.seed_order,
            "registered_execution_order": input.plan.seeds,
            "baseline_seeds": input.plan.baseline_seeds,
            "smoke_seed": input.plan.smoke_seed,
            "n_requested": n_requested,
            "n_complete_pairs_used_by_h1": h1_pairs,
            "min_complete_pairs_for_a_decision": input.plan.min_complete_pairs,
        },
        "observables": {
            "requested_confirmation_arms": input.selected.observable_ids,
            "registry_ids": all_ids,
            "specs": input.registry.fingerprint(),
        },
    });
    write_text(
        &input.out_dir.join("provenance.json"),
        &(pretty(&prov_doc)? + "\n"),
    )?;

    let summary_md = format!(
        "# SEL2-P ({}) - 16 spread lakes food scarcity and genotypic body mass\n\n\
         Model: `LiveExperimentAdapter`, **headless adapter verified**.\n\n\
         - mode: **{}**, seeds: `{:?}`\n\
         - duration: **{} ticks**, warmup **{} ticks**, sample period `{}`\n\
         - decision: **`{decision}`** (`{h1_pairs}` complete H1 pairs of `{n_requested}` requested; minimum `{}`)\n\
         - H1 (`selected - scarce_drift`, `{POST_WARMUP_STAT}`): delta `{}`, SD `{}`, CI `[{}, {}]`, d_z `{}`, material `{}`\n\
         - S2 energy (`{ENERGY_POST_WARMUP_STAT}`): delta `{}`, CI `[{}, {}]`\n\
         - wall clock: `{:.1}s`\n",
        input.mode.as_str(),
        input.mode.as_str(),
        report.seed_order,
        input.duration_ticks,
        input.warmup_ticks,
        input.selected.sample_period,
        input.plan.min_complete_pairs,
        opt_f64(h1.effect.paired_mean_delta),
        opt_f64(h1.effect.paired_sd),
        opt_f64(h1.effect.ci95_low),
        opt_f64(h1.effect.ci95_high),
        opt_f64(h1.effect.paired_dz),
        h1.is_material(),
        opt_f64(s2_energy.effect.paired_mean_delta),
        opt_f64(s2_energy.effect.ci95_low),
        opt_f64(s2_energy.effect.ci95_high),
        elapsed_secs,
    );
    write_text(&input.out_dir.join("summary.md"), &summary_md)?;

    let mut hashed_names: Vec<String> = vec![
        "effects.json".into(),
        "paired-report.json".into(),
        "per-seed-deltas.csv".into(),
        "provenance.json".into(),
        "runs.csv".into(),
        "summary.md".into(),
    ];
    for name in input.file_names {
        hashed_names.push(format!("manifests/{name}"));
    }
    if input.lock.is_some() {
        hashed_names.push(LOCK_FILE.to_string());
    }
    hashed_names.sort();
    let refs: Vec<&str> = hashed_names.iter().map(String::as_str).collect();
    write_checksums(input.out_dir, &refs)?;

    println!(
        "SEL2-P {}: {} seeds x 3 arms x {} ticks in {:.1}s -> {} (decision={decision})",
        input.mode.as_str(),
        n_requested,
        input.duration_ticks,
        elapsed_secs,
        input.out_dir.display(),
    );
    Ok(())
}

// ---- Manifest & plan validation ----------------------------------------------------------------

struct Plan {
    status: String,
    seeds: Vec<u64>,
    min_complete_pairs: usize,
    baseline_seeds: Vec<u64>,
    min_surviving_baseline_seeds: usize,
    smoke_seed: u64,
    excluded_exploration_seeds: Vec<u64>,
    pipeline_baseline_seeds: Vec<u64>,
    pipeline_smoke_seed: u64,
    pipeline_pair_seeds: Vec<u64>,
    duration_ticks: u64,
    warmup_ticks: u64,
    confirm_sample_period: u64,
    smoke_duration_ticks: u64,
    smoke_sample_period: u64,
    pipeline_duration_ticks: u64,
    pipeline_warmup_ticks: u64,
    pipeline_smoke_sample_period: u64,
    pipeline_pair_sample_period: u64,
    selected_file: String,
    scarce_mut0_file: String,
    scarce_drift_file: String,
    baseline_ref_file: String,
    smoke_selected_file: String,
    smoke_mut0_file: String,
    smoke_drift_file: String,
}

impl Plan {
    fn from_prereg(p: &serde_json::Value) -> Result<Self, String> {
        let u64_list = |v: &serde_json::Value, what: &str| -> Result<Vec<u64>, String> {
            v.as_array()
                .ok_or_else(|| format!("{what} must be a list"))?
                .iter()
                .map(|x| {
                    x.as_u64()
                        .ok_or_else(|| format!("{what} holds non-integer"))
                })
                .collect()
        };
        let need_u64 = |v: &serde_json::Value, what: &str| -> Result<u64, String> {
            v.as_u64()
                .ok_or_else(|| format!("{what} must be an integer"))
        };
        let need_str = |v: &serde_json::Value, what: &str| -> Result<String, String> {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("{what} must be a string"))
        };
        Ok(Plan {
            status: need_str(&p["status"], "status")?,
            seeds: u64_list(&p["seeds"]["execution_order"], "seeds.execution_order")?,
            min_complete_pairs: need_u64(
                &p["seeds"]["min_complete_pairs_for_a_decision"],
                "seeds.min_complete_pairs_for_a_decision",
            )? as usize,
            baseline_seeds: u64_list(&p["seeds"]["baseline_seeds"], "seeds.baseline_seeds")?,
            min_surviving_baseline_seeds: need_u64(
                &p["seeds"]["min_surviving_baseline_seeds"],
                "seeds.min_surviving_baseline_seeds",
            )? as usize,
            smoke_seed: need_u64(&p["seeds"]["smoke_seed"], "seeds.smoke_seed")?,
            excluded_exploration_seeds: u64_list(
                &p["seeds"]["excluded_exploration_seeds"],
                "seeds.excluded_exploration_seeds",
            )?,
            pipeline_baseline_seeds: u64_list(
                &p["seeds"]["pipeline_check"]["baseline_seeds"],
                "seeds.pipeline_check.baseline_seeds",
            )?,
            pipeline_smoke_seed: need_u64(
                &p["seeds"]["pipeline_check"]["smoke_seed"],
                "seeds.pipeline_check.smoke_seed",
            )?,
            pipeline_pair_seeds: u64_list(
                &p["seeds"]["pipeline_check"]["pair_seeds"],
                "seeds.pipeline_check.pair_seeds",
            )?,
            duration_ticks: need_u64(&p["duration"]["duration_ticks"], "duration.duration_ticks")?,
            warmup_ticks: need_u64(&p["duration"]["warmup_ticks"], "duration.warmup_ticks")?,
            confirm_sample_period: need_u64(
                &p["duration"]["confirmation_sample_period"],
                "duration.confirmation_sample_period",
            )?,
            smoke_duration_ticks: need_u64(
                &p["duration"]["duration_ticks"],
                "duration.duration_ticks",
            )?,
            smoke_sample_period: need_u64(
                &p["duration"]["smoke_sample_period"],
                "duration.smoke_sample_period",
            )?,
            pipeline_duration_ticks: need_u64(
                &p["duration"]["pipeline_check_duration_ticks"],
                "duration.pipeline_check_duration_ticks",
            )?,
            pipeline_warmup_ticks: need_u64(
                &p["duration"]["pipeline_check_warmup_ticks"],
                "duration.pipeline_check_warmup_ticks",
            )?,
            pipeline_smoke_sample_period: need_u64(
                &p["duration"]["pipeline_check_smoke_sample_period"],
                "duration.pipeline_check_smoke_sample_period",
            )?,
            pipeline_pair_sample_period: need_u64(
                &p["duration"]["pipeline_check_pair_sample_period"],
                "duration.pipeline_check_pair_sample_period",
            )?,
            selected_file: need_str(&p["manifests"]["selected"], "manifests.selected")?,
            scarce_mut0_file: need_str(&p["manifests"]["scarce_mut0"], "manifests.scarce_mut0")?,
            scarce_drift_file: need_str(&p["manifests"]["scarce_drift"], "manifests.scarce_drift")?,
            baseline_ref_file: need_str(&p["manifests"]["baseline_ref"], "manifests.baseline_ref")?,
            smoke_selected_file: need_str(
                &p["manifests"]["smoke_selected"],
                "manifests.smoke_selected",
            )?,
            smoke_mut0_file: need_str(
                &p["manifests"]["smoke_scarce_mut0"],
                "manifests.smoke_scarce_mut0",
            )?,
            smoke_drift_file: need_str(
                &p["manifests"]["smoke_scarce_drift"],
                "manifests.smoke_scarce_drift",
            )?,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_all_manifests(
    registry: &ObservableRegistry,
    plan: &Plan,
    selected: &ExperimentManifest,
    scarce_mut0: &ExperimentManifest,
    scarce_drift: &ExperimentManifest,
    baseline_ref: &ExperimentManifest,
    smoke_selected: &ExperimentManifest,
    smoke_mut0: &ExperimentManifest,
    smoke_drift: &ExperimentManifest,
) -> Result<(), String> {
    for m in [
        selected,
        scarce_mut0,
        scarce_drift,
        baseline_ref,
        smoke_selected,
        smoke_mut0,
        smoke_drift,
    ] {
        m.validate(registry)
            .map_err(|e| format!("manifest '{}' invalid: {e}", m.experiment_id))?;
        let cfg = LiveWorldConfig::from_initial_conditions(&m.initial_conditions)
            .map_err(|e| format!("manifest '{}' config invalid: {e}", m.experiment_id))?;
        cfg.ensure_observables_supported(&m.observable_ids)
            .map_err(|e| {
                format!(
                    "manifest '{}' observables unsupported: {e}",
                    m.experiment_id
                )
            })?;
    }

    // Factor checks across confirmation arms.
    FactorDiff {
        allowed_paths: vec![ALLOWED_FACTOR_PATH.to_string()],
    }
    .validate(selected, scarce_drift)
    .map_err(|e| format!("selected vs scarce_drift FactorDiff: {e}"))?;
    if key_differences(selected, scarce_drift) != [LIVE_KEY_DRIFT_CONTROL.to_string()] {
        return Err("selected vs scarce_drift must differ only in live.drift_control".into());
    }

    FactorDiff {
        allowed_paths: vec![ALLOWED_FACTOR_PATH.to_string()],
    }
    .validate(selected, scarce_mut0)
    .map_err(|e| format!("selected vs scarce_mut0 FactorDiff: {e}"))?;
    if key_differences(selected, scarce_mut0) != [LIVE_KEY_MUTATION_RATE.to_string()] {
        return Err("selected vs scarce_mut0 must differ only in live.mutation_rate".into());
    }

    FactorDiff {
        allowed_paths: vec![ALLOWED_FACTOR_PATH.to_string()],
    }
    .validate(smoke_selected, smoke_mut0)
    .map_err(|e| format!("smoke_selected vs smoke_scarce_mut0 FactorDiff: {e}"))?;
    if key_differences(smoke_selected, smoke_mut0) != [LIVE_KEY_MUTATION_RATE.to_string()] {
        return Err(
            "smoke_selected vs smoke_scarce_mut0 must differ only in live.mutation_rate".into(),
        );
    }
    if key_differences(smoke_selected, smoke_drift) != [LIVE_KEY_DRIFT_CONTROL.to_string()] {
        return Err(
            "smoke_selected vs smoke_scarce_drift must differ only in live.drift_control".into(),
        );
    }

    // Factor checks across baseline and smoke arms.
    let mut base_diff = key_differences(baseline_ref, smoke_selected);
    base_diff.sort();
    if base_diff
        != [
            LIVE_KEY_PREY_CAP.to_string(),
            LIVE_KEY_RESOURCE_CAPACITY_SCALE.to_string(),
        ]
    {
        return Err(format!(
            "baseline_ref vs smoke_selected must differ only in prey_cap and scale, got {base_diff:?}"
        ));
    }

    // Blindness checks on smoke & baseline manifests.
    for m in [baseline_ref, smoke_selected, smoke_mut0, smoke_drift] {
        if m.observable_ids.iter().any(|id| id.contains("trait_mass")) {
            return Err(format!(
                "smoke/baseline manifest '{}' must not request any trait_mass_* observable",
                m.experiment_id
            ));
        }
    }
    for obs in [STARVATION_OBSERVABLE, DEHYDRATION_OBSERVABLE] {
        if scarce_drift.observable_ids.iter().any(|x| x == obs)
            || smoke_drift.observable_ids.iter().any(|x| x == obs)
        {
            return Err(format!("drift manifest must not request {obs}"));
        }
    }

    // Seed disjointness across all declared sets.
    let mut seen = BTreeSet::new();
    let groups: [(&str, &[u64]); 5] = [
        ("exploration", &plan.excluded_exploration_seeds),
        ("baseline", &plan.baseline_seeds),
        ("confirmation", &plan.seeds),
        ("smoke", &[plan.smoke_seed]),
        ("pipeline_baseline", &plan.pipeline_baseline_seeds),
    ];
    for (label, seeds) in groups {
        for &s in seeds {
            if !seen.insert(s) {
                return Err(format!("seed {s} appears in multiple seed sets ({label})"));
            }
        }
    }
    for s in
        std::iter::once(plan.pipeline_smoke_seed).chain(plan.pipeline_pair_seeds.iter().copied())
    {
        if !seen.insert(s) {
            return Err(format!(
                "pipeline_check seed {s} collides with another seed set"
            ));
        }
    }

    if selected.seeds != plan.seeds
        || scarce_mut0.seeds != plan.seeds
        || scarce_drift.seeds != plan.seeds
    {
        return Err("confirmation manifests do not match preregistered execution_order".into());
    }
    if baseline_ref.seeds != plan.baseline_seeds {
        return Err("baseline_ref seeds do not match preregistered baseline_seeds".into());
    }
    if smoke_selected.seeds != [plan.smoke_seed]
        || smoke_mut0.seeds != [plan.smoke_seed]
        || smoke_drift.seeds != [plan.smoke_seed]
    {
        return Err("smoke manifests do not match preregistered smoke_seed".into());
    }
    if selected.sample_period != plan.confirm_sample_period
        || selected.duration_ticks != plan.duration_ticks
    {
        return Err(
            "confirmation manifest duration or sample_period disagrees with preregistration".into(),
        );
    }
    if smoke_selected.sample_period != plan.smoke_sample_period
        || baseline_ref.sample_period != plan.smoke_sample_period
    {
        return Err("smoke/baseline sample_period disagrees with preregistration".into());
    }

    Ok(())
}

fn validate_confirm_lock(
    lock: &serde_json::Value,
    plan: &Plan,
    manifest_hashes: &BTreeMap<String, String>,
) -> Result<(), String> {
    if lock.get("smoke_gate_passed").and_then(|v| v.as_bool()) != Some(true) {
        return Err(format!("{LOCK_FILE}: smoke_gate_passed must be true"));
    }
    if lock.get("duration_ticks").and_then(|v| v.as_u64()) != Some(plan.duration_ticks) {
        return Err(format!("{LOCK_FILE}: duration_ticks mismatch"));
    }
    if lock.get("warmup_ticks").and_then(|v| v.as_u64()) != Some(plan.warmup_ticks) {
        return Err(format!("{LOCK_FILE}: warmup_ticks mismatch"));
    }
    if lock.get("sample_period").and_then(|v| v.as_u64()) != Some(plan.confirm_sample_period) {
        return Err(format!("{LOCK_FILE}: sample_period mismatch"));
    }
    if lock
        .get("baseline_surviving_count")
        .and_then(|v| v.as_u64())
        .is_none_or(|n| (n as usize) < plan.min_surviving_baseline_seeds)
    {
        return Err(format!(
            "{LOCK_FILE}: baseline_surviving_count must be >= {}",
            plan.min_surviving_baseline_seeds
        ));
    }
    if !lock
        .get("pooled_baseline_crop")
        .and_then(|v| v.as_f64())
        .is_some_and(|c| c.is_finite() && c > 0.0)
    {
        return Err(format!(
            "{LOCK_FILE}: pooled_baseline_crop must be positive"
        ));
    }
    if let Some(obj) = lock.get("manifest_sha256").and_then(|v| v.as_object()) {
        for (name, val) in obj {
            let locked = val
                .as_str()
                .ok_or_else(|| format!("{LOCK_FILE}: bad hash for {name}"))?;
            let actual = manifest_hashes
                .get(name)
                .ok_or_else(|| format!("{LOCK_FILE}: unknown manifest {name}"))?;
            if locked != actual {
                return Err(format!(
                    "{LOCK_FILE}: manifest {name} sha256 {locked} != committed {actual}"
                ));
            }
        }
    }
    Ok(())
}

// ---- Small helpers -----------------------------------------------------------------------------

fn key_differences(a: &ExperimentManifest, b: &ExperimentManifest) -> Vec<String> {
    let to_map = |m: &ExperimentManifest| -> BTreeMap<String, f64> {
        m.initial_conditions.values.iter().cloned().collect()
    };
    let (ma, mb) = (to_map(a), to_map(b));
    let mut keys: Vec<String> = ma.keys().chain(mb.keys()).cloned().collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter(|k| ma.get(k) != mb.get(k))
        .collect()
}

fn contrast_json(c: &ContrastEffect) -> serde_json::Value {
    let (ci, dz, rel) = c.materiality();
    serde_json::json!({
        "id": c.id,
        "contrast": c.contrast,
        "statistic": c.statistic,
        "arm_order": c.arm_order,
        "n_requested": c.effect.n_requested,
        "n_complete_pairs": c.effect.n_complete_pairs,
        "control_mean": c.effect.control_mean,
        "treatment_mean": c.effect.treatment_mean,
        "paired_mean_delta": c.effect.paired_mean_delta,
        "paired_sd": c.effect.paired_sd,
        "paired_se": c.effect.paired_se,
        "ci95_low": c.effect.ci95_low,
        "ci95_high": c.effect.ci95_high,
        "paired_dz": c.effect.paired_dz,
        "median_of_per_seed_deltas": c.median_delta,
        "relative_shift_vs_control_mean": c.relative_shift(),
        "materiality": {
            "ci95_excludes_zero": ci,
            "abs_dz_at_least_0_66": dz,
            "abs_delta_at_least_5pc_of_control_mean": rel,
            "material": c.is_material(),
        },
    })
}

fn drift_diag_json(d: &DriftDiagnostics) -> serde_json::Value {
    serde_json::json!({
        "scheduled_deaths": d.scheduled_deaths,
        "actual_deaths": d.actual_deaths,
        "missed_deaths": d.missed_deaths,
        "scheduled_births": d.scheduled_births,
        "actual_births": d.actual_births,
        "missed_births_extinct": d.missed_births_extinct,
        "missed_births_insolvent": d.missed_births_insolvent,
        "ticks_with_missed_deaths": d.ticks_with_missed_deaths,
        "ticks_with_missed_births": d.ticks_with_missed_births,
        "first_divergence_tick": d.first_divergence_tick,
        "has_shortfall": d.has_shortfall(),
    })
}

fn write_checksums(dir: &Path, names: &[&str]) -> Result<(), String> {
    let mut sorted = names.to_vec();
    sorted.sort_unstable();
    let mut lines = String::new();
    for name in sorted {
        let digest = sha256_file(&dir.join(name)).map_err(|e| format!("hash {name}: {e}"))?;
        lines.push_str(&format!("{digest}  {name}\n"));
    }
    write_text(&dir.join("checksums.sha256"), &lines)
}

fn median(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    })
}

fn status_label(s: &RunStatus) -> &'static str {
    match s {
        RunStatus::Completed => "completed",
        RunStatus::Failed { .. } => "failed",
    }
}

fn opt_u64(v: Option<u64>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

fn opt_f64(v: Option<f64>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

fn csv_field(s: &str) -> String {
    s.replace([',', '\n', '\r'], " ")
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

fn parse_args(argv: Vec<String>) -> Result<Args, String> {
    let mut mode: Option<Mode> = None;
    let mut manifest_dir: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut it = argv.into_iter();
    while let Some(arg) = it.next() {
        let mut set_mode = |m: Mode| -> Result<(), String> {
            if let Some(prev) = mode {
                return Err(format!(
                    "--pipeline-check, --smoke and --confirm are mutually exclusive (already set {})",
                    prev.as_str()
                ));
            }
            mode = Some(m);
            Ok(())
        };
        match arg.as_str() {
            "--pipeline-check" => set_mode(Mode::PipelineCheck)?,
            "--smoke" => set_mode(Mode::Smoke)?,
            "--confirm" => set_mode(Mode::Confirm)?,
            "--manifest-dir" => {
                manifest_dir = Some(PathBuf::from(
                    it.next().ok_or("--manifest-dir needs a value")?,
                ))
            }
            "--out" => out = Some(PathBuf::from(it.next().ok_or("--out needs a value")?)),
            "--help" | "-h" => {
                println!(
                    "run_sel2p_experiment --pipeline-check|--smoke|--confirm \
                     --manifest-dir <dir> --out <dir>"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    let mode = mode.ok_or("exactly one of --pipeline-check, --smoke or --confirm is required")?;
    let manifest_dir = manifest_dir.ok_or("--manifest-dir is required")?;
    let out = out.ok_or("--out is required")?;
    if !manifest_dir.is_dir() {
        return Err(format!("{} is not a directory", manifest_dir.display()));
    }
    Ok(Args {
        mode,
        manifest_dir,
        out,
    })
}
