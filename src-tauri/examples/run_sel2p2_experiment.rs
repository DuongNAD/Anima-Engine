//! The `SEL2-P2` experiment runner (`Card SEL2-P2-RUNNER`, Option `(a)` on pinned `f195a361`).
//!
//! Reads the frozen preregistration (`sel2p2-preregistration.json`) and its 7 manifests under
//! `src-tauri/tests/fixtures/experiments_sel2p2/`, validates every invariant, and executes the
//! preregistered stages in strict order:
//! 1. Pre-smoke `QD-SEL2g` pooled `baseline_ref` (`12` seeds `970501..=970512`, `prey_cap = 80`, `scale = 1.0`);
//! 2. Blind smoke (`seed = 999931` across `selected`, `scarce_drift`, `scarce_mut0`) with `C1..C5` +
//!    `drift_valid` hard stop and zero `trait_mass` observables anywhere in `smoke/` or logs;
//! 3. Confirmation blocks (`N = 48` paired seeds `900001..=947001` x `3` arms = `144` runs) only after
//!    passing the blind smoke hard stop, staying strictly blind to `trait_mass` until all runs finish.
//!
//! # Build first in `--release --features desktop`, then run the binary (never `cargo run`)
//!
//! ```text
//! cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop \
//!   --example run_sel2p2_experiment
//!
//! run_sel2p2_experiment --dry-run
//! run_sel2p2_experiment --smoke --out artifacts/experiments/sel2-p2/smoke
//! run_sel2p2_experiment --confirm --smoke-dir artifacts/experiments/sel2-p2/smoke \
//!   --out artifacts/experiments/sel2-p2/confirmation --checkpoint-dir <external-tmp> --jobs 4
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anima_engine_lib::core::drift_control::DriftDiagnostics;
use anima_engine_lib::core::experiment::{
    ExperimentManifest, FactorDiff, ObservableRegistry, MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{run_manifest_seed, RunResult, RunStatus};
use anima_engine_lib::core::live_experiment::{
    run_live_drift_pair, LiveExperimentAdapter, LIVE_MODEL_VERSION,
};
use anima_engine_lib::core::sha256::sha256_file;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use serde::{Deserialize, Serialize};

const DEFAULT_MANIFEST_DIR: &str = "src-tauri/tests/fixtures/experiments_sel2p2";
const DEFAULT_SMOKE_OUT: &str = "artifacts/experiments/sel2-p2/smoke";
const DEFAULT_CONFIRM_OUT: &str = "artifacts/experiments/sel2-p2/confirmation";
const PREREG_FILE: &str = "sel2p2-preregistration.json";
const TEMP_ARTIFACT_NAME: &str = "anima_world_artifact.anmw";
const ARM_ORDER: [&str; 3] = ["selected", "scarce_mut0", "scarce_drift"];

const SMOKE_ALLOWED_NON_DRIFT: [&str; 9] = [
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

const SMOKE_ALLOWED_DRIFT: [&str; 7] = [
    "live.closed_eu_total",
    "live.agent_count",
    "live.standing_crop",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
    "live.cumulative_mean_lifespan",
];

const CONFIRM_SERIES_OBSERVABLES: [&str; 9] = [
    "live.agent_count",
    "live.deaths",
    "live.births",
    "live.mean_lifespan",
    "live.mean_energy_integral",
    "live.trait_mass_mean",
    "live.trait_mass_var",
    "live.standing_crop",
    "live.closed_eu_total",
];

const CAUSE_PARTITION_TOLERANCE: f64 = 1e-6;

pub type WorldIdentityHook = fn(usize, &str, u64, WorldIdentity) -> WorldIdentity;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Help,
    DryRun,
    SelfTest,
    Smoke,
    Confirm,
    RunAll,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Mode::Help => "help",
            Mode::DryRun => "dry-run",
            Mode::SelfTest => "self-test",
            Mode::Smoke => "smoke",
            Mode::Confirm => "confirm",
            Mode::RunAll => "run-all",
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
    pub allow_arbitrary_out: bool,
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
            allow_arbitrary_out: false,
            git_commit_override: None,
            git_dirty_paths_override: None,
            world_identity_override: None,
        }
    }
}

struct Args {
    mode: Mode,
    manifest_dir: PathBuf,
    out: Option<PathBuf>,
    smoke_dir: Option<PathBuf>,
    checkpoint_dir: Option<PathBuf>,
    jobs: usize,
}

#[derive(Clone)]
pub struct ThresholdsSpec {
    pub fwer_alpha: f64,
    pub sesoi_log_mean_ratio: f64,
    pub max_ext_rate: f64,
    pub min_pi_ll: f64,
    pub min_valid_pairs: usize,
    pub min_non_zero_pairs_r7: usize,
    pub max_abs_eu_drift: f64,
    pub rmst_tau_ticks: u64,
    pub bca_resamples: usize,
}

#[derive(Clone)]
pub struct SmokeGateSpec {
    pub min_surviving_baseline_seeds: usize,
    pub c1_min_agents: f64,
    pub c2_g_band: [f64; 2],
    pub c3_min_generations: f64,
    pub c4_min_starvation_share: f64,
}

pub struct PreregSpec {
    status: String,
    prereg_id: String,
    selected_file: String,
    scarce_mut0_file: String,
    scarce_drift_file: String,
    baseline_ref_file: String,
    smoke_selected_file: String,
    smoke_mut0_file: String,
    smoke_drift_file: String,
    pub execution_order: Vec<u64>,
    pub baseline_ref_seeds: Vec<u64>,
    pub smoke_seed: u64,
    duration_ticks: u64,
    warmup_ticks: u64,
    confirmation_sample_period: u64,
    smoke_sample_period: u64,
    tail20_threshold_tick: u64,
    artifacts_root: String,
    smoke_dir_default: String,
    confirmation_dir_default: String,
    smoke_gate: SmokeGateSpec,
    thresholds: ThresholdsSpec,
}

#[derive(Clone, Serialize, Deserialize)]
struct RunCheckpoint {
    seed: u64,
    pair_elapsed_secs: f64,
    mut0_elapsed_secs: f64,
    selected: RunResult,
    scarce_drift: RunResult,
    scarce_mut0: RunResult,
    drift_diagnostics: DriftDiagnostics,
    window_net_selection_deltas: BTreeMap<String, f64>,
}

#[derive(Clone)]
struct ArmRunRecord {
    seed: u64,
    arm: String,
    result: RunResult,
    drift_valid: bool,
    has_shortfall: bool,
    terrain_source: String,
    world_identity: WorldIdentity,
    elapsed_secs: f64,
    window_net_selection_deltas: Option<BTreeMap<String, f64>>,
}

#[derive(Clone)]
struct SmokeVerification {
    dir_display: String,
    provenance_sha256: String,
    smoke_report_sha256: String,
    git_commit: Option<String>,
    host_os: Option<String>,
    host_arch: Option<String>,
    profile: Option<String>,
    world_identity: Option<WorldIdentity>,
    gate_pass: bool,
}

// ---- Exact SEL2-P2 statistical engine (mirrors `scripts/verify_sel2p2_artifacts.mjs`) ----------

pub fn log_gamma(z: f64) -> f64 {
    #[allow(clippy::excessive_precision)]
    let p = [
        0.99999999999980993,
        676.5203681218851,
        -1259.1392167224028,
        771.32342877765313,
        -176.61502916214059,
        12.507343278686905,
        -0.13857109526572012,
        9.9843695780195716e-6,
        1.5056327351493116e-7,
    ];
    if z < 0.5 {
        return std::f64::consts::PI.ln()
            - (std::f64::consts::PI * z).sin().ln()
            - log_gamma(1.0 - z);
    }
    let zm1 = z - 1.0;
    let mut x = p[0];
    for (idx, coef) in p.iter().enumerate().skip(1) {
        x += coef / (zm1 + idx as f64);
    }
    let t = zm1 + 7.5;
    0.5 * (2.0 * std::f64::consts::PI).ln() + (zm1 + 0.5) * t.ln() - t + x.ln()
}

fn betacf(a: f64, b: f64, x: f64) -> f64 {
    const MAXIT: usize = 300;
    const EPS: f64 = 3e-15;
    const FPMIN: f64 = 1e-300;
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - (qab * x) / qap;
    if d.abs() < FPMIN {
        d = FPMIN;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..=MAXIT {
        let mf = m as f64;
        let m2 = 2.0 * mf;
        let mut aa = (mf * (b - mf) * x) / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        h *= d * c;
        aa = (-(a + mf) * (qab + mf) * x) / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < FPMIN {
            d = FPMIN;
        }
        c = 1.0 + aa / c;
        if c.abs() < FPMIN {
            c = FPMIN;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() <= EPS {
            break;
        }
    }
    h
}

pub fn beta_inc(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let bt =
        (log_gamma(a + b) - log_gamma(a) - log_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        (bt * betacf(a, b, x)) / a
    } else {
        1.0 - (bt * betacf(b, a, 1.0 - x)) / b
    }
}

pub fn student_t_cdf(t: f64, df: usize) -> f64 {
    if df < 1 {
        return f64::NAN;
    }
    if t == 0.0 {
        return 0.5;
    }
    if t.is_infinite() {
        return if t.is_sign_positive() { 1.0 } else { 0.0 };
    }
    let dff = df as f64;
    let x = dff / (dff + t * t);
    let tail = 0.5 * beta_inc(0.5 * dff, 0.5, x);
    if t >= 0.0 {
        1.0 - tail
    } else {
        tail
    }
}

pub fn student_t_two_sided_p(t: f64, df: usize) -> f64 {
    if df < 1 {
        return f64::NAN;
    }
    if !t.is_finite() {
        return if t.is_infinite() { 0.0 } else { f64::NAN };
    }
    if t == 0.0 {
        return 1.0;
    }
    let dff = df as f64;
    let x = dff / (dff + t * t);
    beta_inc(0.5 * dff, 0.5, x).clamp(0.0, 1.0)
}

pub fn student_t_inv(prob: f64, df: usize) -> f64 {
    if df < 1 || !(prob > 0.0 && prob < 1.0) {
        return f64::NAN;
    }
    if (prob - 0.5).abs() == 0.0 {
        return 0.0;
    }
    if prob < 0.5 {
        return -student_t_inv(1.0 - prob, df);
    }
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;
    while student_t_cdf(hi, df) < prob && hi < 1e9 {
        hi *= 2.0;
    }
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        if student_t_cdf(mid, df) < prob {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo <= 1e-13 * mid.abs().max(1.0) {
            break;
        }
    }
    0.5 * (lo + hi)
}

fn erfc_approx(x: f64) -> f64 {
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let r = t
        * (-z * z - 1.26551223
            + t * (1.00002368
                + t * (0.37409196
                    + t * (0.09678418
                        + t * (-0.18628806
                            + t * (0.27886807
                                + t * (-1.13520398
                                    + t * (1.48851587 + t * (-0.82215223 + t * 0.17087277)))))))))
            .exp();
    if x >= 0.0 {
        r
    } else {
        2.0 - r
    }
}

pub fn norm_cdf(z: f64) -> f64 {
    0.5 * erfc_approx(-z / std::f64::consts::SQRT_2)
}

#[allow(clippy::excessive_precision)]
pub fn norm_inv(p: f64) -> f64 {
    if !(p > 0.0 && p < 1.0) {
        if p == 0.0 {
            return f64::NEG_INFINITY;
        }
        if p == 1.0 {
            return f64::INFINITY;
        }
        return f64::NAN;
    }
    let a = [
        -3.969683028665376e1,
        2.209460984245205e2,
        -2.759285104469687e2,
        1.38357751867269e2,
        -3.066479806614716e1,
        2.506628277459239,
    ];
    let b = [
        -5.447609879822406e1,
        1.615858368580409e2,
        -1.556989798598866e2,
        6.680131188771972e1,
        -1.328068155288572e1,
    ];
    let c = [
        -7.784894002430293e-3,
        -3.223964580411365e-1,
        -2.400758277161838,
        -2.549732539343734,
        4.374664141464968,
        2.938163982698783,
    ];
    let d = [
        7.784695709041462e-3,
        3.224671290700398e-1,
        2.445134137142996,
        3.754408661907416,
    ];
    let plow = 0.02425;
    let phigh = 1.0 - plow;
    let mut x = if p < plow {
        let q = (-2.0 * p.ln()).sqrt();
        (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
            / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    } else if p <= phigh {
        let q = p - 0.5;
        let r = q * q;
        (((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q
            / (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
            / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    };
    let inv_sqrt_2pi = 1.0 / (2.0 * std::f64::consts::PI).sqrt();
    for _ in 0..2 {
        let err = norm_cdf(x) - p;
        let pdf = inv_sqrt_2pi * (-0.5 * x * x).exp();
        if pdf > 0.0 {
            x -= err / pdf;
        }
    }
    x
}

pub fn mean_slice(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        None
    } else {
        Some(xs.iter().sum::<f64>() / xs.len() as f64)
    }
}

pub fn median_slice(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut sorted = xs.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        Some(sorted[mid])
    } else {
        Some(0.5 * (sorted[mid - 1] + sorted[mid]))
    }
}

pub fn sample_sd(xs: &[f64]) -> Option<f64> {
    if xs.len() < 2 {
        return None;
    }
    let m = mean_slice(xs)?;
    let ss: f64 = xs.iter().map(|x| (x - m) * (x - m)).sum();
    Some((ss / (xs.len() - 1) as f64).sqrt())
}

pub fn ranks_with_ties(xs: &[f64]) -> (Vec<f64>, Vec<usize>) {
    let mut indexed: Vec<(f64, usize)> = xs
        .iter()
        .copied()
        .enumerate()
        .map(|(i, v)| (v, i))
        .collect();
    indexed.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut ranks = vec![0.0; xs.len()];
    let mut tie_counts = Vec::new();
    let mut i = 0;
    while i < indexed.len() {
        let mut j = i + 1;
        while j < indexed.len() && indexed[j].0 == indexed[i].0 {
            j += 1;
        }
        let avg_rank = 0.5 * ((i + 1 + j) as f64);
        for k in i..j {
            ranks[indexed[k].1] = avg_rank;
        }
        if j - i > 1 {
            tie_counts.push(j - i);
        }
        i = j;
    }
    (ranks, tie_counts)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct WilcoxonReport {
    pub n_non_zero: usize,
    pub w_plus: f64,
    pub w_minus: f64,
    pub r_c: Option<f64>,
    pub p_value: f64,
    pub p_min: f64,
    pub meets_r7: bool,
    pub method: String,
}

pub fn wilcoxon_signed_rank(diffs: &[f64], min_non_zero_r7: usize) -> WilcoxonReport {
    let nz: Vec<f64> = diffs
        .iter()
        .copied()
        .filter(|d| d.is_finite() && *d != 0.0)
        .collect();
    let n_nz = nz.len();
    let p_min = if n_nz > 0 {
        (2.0 / 2.0_f64.powi(n_nz as i32)).min(1.0)
    } else {
        1.0
    };
    if n_nz == 0 {
        return WilcoxonReport {
            n_non_zero: 0,
            w_plus: 0.0,
            w_minus: 0.0,
            r_c: None,
            p_value: 1.0,
            p_min: 1.0,
            meets_r7: false,
            method: "degenerate_all_zero".to_string(),
        };
    }
    let abs_vals: Vec<f64> = nz.iter().map(|d| d.abs()).collect();
    let (ranks, tie_counts) = ranks_with_ties(&abs_vals);
    let mut w_plus = 0.0;
    let mut w_minus = 0.0;
    for (idx, &d) in nz.iter().enumerate() {
        if d > 0.0 {
            w_plus += ranks[idx];
        } else {
            w_minus += ranks[idx];
        }
    }
    let total_rank = w_plus + w_minus;
    let r_c = Some(if total_rank > 0.0 {
        (w_plus - w_minus) / total_rank
    } else {
        0.0
    });

    let (p_value, method) = if n_nz <= 20 && tie_counts.is_empty() {
        let max_sum = n_nz * (n_nz + 1) / 2;
        let mut dp = vec![0.0_f64; max_sum + 1];
        dp[0] = 1.0;
        for k in 1..=n_nz {
            let mut next = vec![0.0_f64; max_sum + 1];
            for s in 0..=max_sum {
                if dp[s] > 0.0 {
                    next[s] += dp[s];
                    if s + k <= max_sum {
                        next[s + k] += dp[s];
                    }
                }
            }
            dp = next;
        }
        let w_obs = w_plus.min(w_minus).round() as usize;
        let tail_count: f64 = dp[..=w_obs.min(max_sum)].iter().sum();
        (
            ((2.0 * tail_count) / 2.0_f64.powi(n_nz as i32)).min(1.0),
            "exact_dp".to_string(),
        )
    } else {
        let nf = n_nz as f64;
        let mu = nf * (nf + 1.0) / 4.0;
        let tie_adj: f64 = tie_counts
            .iter()
            .map(|&t| (t * (t * t - 1)) as f64)
            .sum::<f64>()
            / 48.0;
        let variance = (nf * (nf + 1.0) * (2.0 * nf + 1.0)) / 24.0 - tie_adj;
        let p = if variance <= 0.0 {
            1.0
        } else {
            let z = (w_plus.min(w_minus) - mu).abs() / variance.sqrt();
            (2.0 * norm_cdf(-z)).min(1.0)
        };
        (p, "normal_approx".to_string())
    };

    WilcoxonReport {
        n_non_zero: n_nz,
        w_plus,
        w_minus,
        r_c,
        p_value,
        p_min,
        meets_r7: n_nz >= min_non_zero_r7,
        method,
    }
}

pub fn hodges_lehmann_pseudomedian(xs: &[f64]) -> Option<f64> {
    let n = xs.len();
    if n == 0 {
        return None;
    }
    if n == 1 {
        return Some(xs[0]);
    }
    let mut walsh = Vec::with_capacity(n * (n + 1) / 2);
    for i in 0..n {
        for j in i..n {
            walsh.push(0.5 * (xs[i] + xs[j]));
        }
    }
    walsh.sort_by(f64::total_cmp);
    let mid = walsh.len() / 2;
    if walsh.len() % 2 == 1 {
        Some(walsh[mid])
    } else {
        Some(0.5 * (walsh[mid - 1] + walsh[mid]))
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HlBcaCiReport {
    pub estimate: Option<f64>,
    pub ci_low: Option<f64>,
    pub ci_high: Option<f64>,
    pub n_boot: usize,
}

fn splitmix64_unit(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^= z >> 31;
    ((z & 0x001f_ffff_ffff_ffff) as f64) / 9_007_199_254_740_992.0
}

pub fn hodges_lehmann_bca_ci(xs: &[f64], n_boot: usize, alpha: f64, seed: u64) -> HlBcaCiReport {
    let n = xs.len();
    let est = hodges_lehmann_pseudomedian(xs);
    let Some(est_val) = est else {
        return HlBcaCiReport {
            estimate: None,
            ci_low: None,
            ci_high: None,
            n_boot: 0,
        };
    };
    if n < 2 || n_boot == 0 {
        return HlBcaCiReport {
            estimate: est,
            ci_low: None,
            ci_high: None,
            n_boot: 0,
        };
    }

    let mut state = seed;
    let mut boots = Vec::with_capacity(n_boot);
    let mut sample = vec![0.0_f64; n];
    let mut lt_count = 0usize;
    let mut eq_count = 0usize;
    for _ in 0..n_boot {
        for slot in &mut sample {
            let idx = (splitmix64_unit(&mut state) * n as f64).floor() as usize;
            *slot = xs[idx.min(n - 1)];
        }
        let hl = hodges_lehmann_pseudomedian(&sample).unwrap_or(est_val);
        boots.push(hl);
        if hl < est_val {
            lt_count += 1;
        } else if hl == est_val {
            eq_count += 1;
        }
    }
    boots.sort_by(f64::total_cmp);

    let mut jack = Vec::with_capacity(n);
    for i in 0..n {
        let leave_one: Vec<f64> = xs
            .iter()
            .enumerate()
            .filter_map(|(j, &v)| if j != i { Some(v) } else { None })
            .collect();
        jack.push(hodges_lehmann_pseudomedian(&leave_one).unwrap_or(est_val));
    }
    let jack_mean = mean_slice(&jack).unwrap_or(est_val);
    let mut num = 0.0;
    let mut den = 0.0;
    for &jv in &jack {
        let d = jack_mean - jv;
        num += d * d * d;
        den += d * d;
    }
    let a = if den > 0.0 {
        num / (6.0 * den.powf(1.5))
    } else {
        0.0
    };
    let prop = ((lt_count as f64 + 0.5 * eq_count as f64) / n_boot as f64).clamp(
        1.0 / (2.0 * n_boot as f64),
        1.0 - 1.0 / (2.0 * n_boot as f64),
    );
    let z0 = norm_inv(prop);

    let bca_quantile = |alpha_tail: f64| -> f64 {
        let z_alpha = norm_inv(alpha_tail);
        let denom = 1.0 - a * (z0 + z_alpha);
        let u = if denom == 0.0 {
            if z0 + z_alpha >= 0.0 {
                1.0
            } else {
                0.0
            }
        } else {
            norm_cdf(z0 + (z0 + z_alpha) / denom)
        }
        .clamp(0.0, 1.0);
        let idx = ((u * (n_boot - 1) as f64).round() as usize).min(n_boot - 1);
        boots[idx]
    };

    HlBcaCiReport {
        estimate: est,
        ci_low: Some(bca_quantile(alpha * 0.5)),
        ci_high: Some(bca_quantile(1.0 - alpha * 0.5)),
        n_boot,
    }
}

pub fn holm_adjust(p_values: &[Option<f64>]) -> Vec<Option<f64>> {
    let m = p_values.len();
    let mut valid: Vec<(f64, usize)> = p_values
        .iter()
        .enumerate()
        .filter_map(|(i, p)| p.filter(|v| v.is_finite()).map(|v| (v, i)))
        .collect();
    valid.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut adjusted = vec![None; m];
    let mut running_max = 0.0_f64;
    for (rank, (p, idx)) in valid.into_iter().enumerate() {
        let raw_adj = (m - rank) as f64 * p;
        running_max = running_max.max(raw_adj).min(1.0);
        adjusted[idx] = Some(running_max);
    }
    adjusted
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PairedStatsReport {
    pub n_requested: usize,
    pub n_complete_pairs: usize,
    pub control_mean: Option<f64>,
    pub treatment_mean: Option<f64>,
    pub paired_mean_delta: Option<f64>,
    pub paired_sd: Option<f64>,
    pub paired_se: Option<f64>,
    pub ci95_low: Option<f64>,
    pub ci95_high: Option<f64>,
    pub paired_dz: Option<f64>,
    pub median_of_per_seed_deltas: Option<f64>,
}

pub fn paired_stats(control: &[f64], treatment: &[f64], n_requested: usize) -> PairedStatsReport {
    assert_eq!(control.len(), treatment.len());
    let n = control.len();
    let deltas: Vec<f64> = control.iter().zip(treatment).map(|(c, t)| t - c).collect();
    let mut out = PairedStatsReport {
        n_requested,
        n_complete_pairs: n,
        control_mean: mean_slice(control),
        treatment_mean: mean_slice(treatment),
        paired_mean_delta: mean_slice(&deltas),
        paired_sd: None,
        paired_se: None,
        ci95_low: None,
        ci95_high: None,
        paired_dz: None,
        median_of_per_seed_deltas: median_slice(&deltas),
    };
    if n < 2 {
        return out;
    }
    let m = out.paired_mean_delta.unwrap_or(0.0);
    let sd = sample_sd(&deltas).unwrap_or(0.0);
    if sd < 1e-12 * m.abs().max(1.0) {
        out.paired_sd = Some(0.0);
        out.paired_se = Some(0.0);
        out.ci95_low = Some(m);
        out.ci95_high = Some(m);
        return out;
    }
    let se = sd / (n as f64).sqrt();
    let half = student_t_inv(0.975, n - 1) * se;
    out.paired_sd = Some(sd);
    out.paired_se = Some(se);
    out.ci95_low = Some(m - half);
    out.ci95_high = Some(m + half);
    out.paired_dz = Some(m / sd);
    out
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PairedLogMeanRatioReport {
    pub n_requested: usize,
    pub n_complete_pairs: usize,
    pub pi_ll: f64,
    #[serde(rename = "mean_D")]
    pub mean_d: Option<f64>,
    pub geom_mean_ratio: Option<f64>,
    #[serde(rename = "sd_D")]
    pub sd_d: Option<f64>,
    #[serde(rename = "se_D")]
    pub se_d: Option<f64>,
    pub df: Option<usize>,
    pub t_stat: Option<f64>,
    pub p_two_sided: Option<f64>,
    pub ci95_low: Option<f64>,
    pub ci95_high: Option<f64>,
    pub ci90_tost_low: Option<f64>,
    pub ci90_tost_high: Option<f64>,
    pub tost_p1: Option<f64>,
    pub tost_p2: Option<f64>,
    pub tost_equivalent: bool,
    pub paired_dz: Option<f64>,
    pub mde_0_5_sd: Option<f64>,
    pub mde_holm1_80: Option<f64>,
    pub concordance_pos: Option<f64>,
    pub concordance_neg: Option<f64>,
    #[serde(rename = "median_D")]
    pub median_d: Option<f64>,
    pub hl_pseudomedian: Option<f64>,
    pub hl_bca_95_ci: HlBcaCiReport,
    pub wilcoxon: WilcoxonReport,
    pub diffs: Vec<f64>,
}

pub fn evaluate_paired_log_mean_ratio(
    mean_a: &[f64],
    mean_b: &[f64],
    sesoi: f64,
    min_non_zero_r7: usize,
    n_requested: usize,
    bca_resamples: usize,
) -> Result<PairedLogMeanRatioReport, String> {
    if mean_a.len() != mean_b.len() {
        return Err("mean_a and mean_b must have equal length".into());
    }
    let n = mean_a.len();
    let mut diffs = Vec::with_capacity(n);
    for i in 0..n {
        if !(mean_a[i] > 0.0 && mean_b[i] > 0.0 && mean_a[i].is_finite() && mean_b[i].is_finite()) {
            return Err(format!(
                "mean_a/mean_b must be strictly positive finite numbers at index {i}"
            ));
        }
        diffs.push((mean_b[i] / mean_a[i]).ln());
    }
    let wilcoxon = wilcoxon_signed_rank(&diffs, min_non_zero_r7);
    let hl_pseudomedian = hodges_lehmann_pseudomedian(&diffs);
    let hl_bca_95_ci = hodges_lehmann_bca_ci(&diffs, bca_resamples, 0.05, 0x5e12_0002);
    let mut out = PairedLogMeanRatioReport {
        n_requested,
        n_complete_pairs: n,
        pi_ll: if n_requested > 0 {
            n as f64 / n_requested as f64
        } else {
            0.0
        },
        mean_d: None,
        geom_mean_ratio: None,
        sd_d: None,
        se_d: None,
        df: if n >= 2 { Some(n - 1) } else { None },
        t_stat: None,
        p_two_sided: None,
        ci95_low: None,
        ci95_high: None,
        ci90_tost_low: None,
        ci90_tost_high: None,
        tost_p1: None,
        tost_p2: None,
        tost_equivalent: false,
        paired_dz: None,
        mde_0_5_sd: None,
        mde_holm1_80: None,
        concordance_pos: if n > 0 {
            Some(diffs.iter().filter(|d| **d > 0.0).count() as f64 / n as f64)
        } else {
            None
        },
        concordance_neg: if n > 0 {
            Some(diffs.iter().filter(|d| **d < 0.0).count() as f64 / n as f64)
        } else {
            None
        },
        median_d: median_slice(&diffs),
        hl_pseudomedian,
        hl_bca_95_ci,
        wilcoxon,
        diffs: diffs.clone(),
    };
    let Some(m) = mean_slice(&diffs) else {
        return Ok(out);
    };
    out.mean_d = Some(m);
    out.geom_mean_ratio = Some(m.exp());
    if n < 2 {
        return Ok(out);
    }

    let sd = sample_sd(&diffs).unwrap_or(0.0);
    let zero_tol = 1e-12 * m.abs().max(1.0);
    if sd < zero_tol {
        out.sd_d = Some(0.0);
        out.se_d = Some(0.0);
        out.t_stat = if m == 0.0 { Some(0.0) } else { None };
        out.p_two_sided = Some(if m == 0.0 { 1.0 } else { 0.0 });
        out.ci95_low = Some(m);
        out.ci95_high = Some(m);
        out.ci90_tost_low = Some(m);
        out.ci90_tost_high = Some(m);
        out.tost_p1 = Some(if m > -sesoi { 0.0 } else { 1.0 });
        out.tost_p2 = Some(if m < sesoi { 0.0 } else { 1.0 });
        out.tost_equivalent = m.abs() < sesoi;
        out.mde_0_5_sd = Some(0.0);
        out.mde_holm1_80 = Some(0.0);
        return Ok(out);
    }

    let df = n - 1;
    let se = sd / (n as f64).sqrt();
    let t_stat = m / se;
    let t_crit_95 = student_t_inv(0.975, df);
    let t_crit_90 = student_t_inv(0.95, df);
    let p_two = student_t_two_sided_p(t_stat, df);
    let ci95_lo = m - t_crit_95 * se;
    let ci95_hi = m + t_crit_95 * se;
    let ci90_lo = m - t_crit_90 * se;
    let ci90_hi = m + t_crit_90 * se;
    let tost_p1 = 1.0 - student_t_cdf((m + sesoi) / se, df);
    let tost_p2 = 1.0 - student_t_cdf((sesoi - m) / se, df);

    out.sd_d = Some(sd);
    out.se_d = Some(se);
    out.t_stat = Some(t_stat);
    out.p_two_sided = Some(p_two);
    out.ci95_low = Some(ci95_lo);
    out.ci95_high = Some(ci95_hi);
    out.ci90_tost_low = Some(ci90_lo);
    out.ci90_tost_high = Some(ci90_hi);
    out.tost_p1 = Some(tost_p1);
    out.tost_p2 = Some(tost_p2);
    out.tost_equivalent = ci90_lo > -sesoi && ci90_hi < sesoi;
    out.paired_dz = Some(m / sd);
    out.mde_0_5_sd = Some(0.5 * sd);
    out.mde_holm1_80 = Some(0.4818 * sd);
    Ok(out)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LeeBoundsReport {
    pub n_a: usize,
    pub n_b: usize,
    pub k: usize,
    pub lee_lo: Option<f64>,
    pub lee_hi: Option<f64>,
    pub width: Option<f64>,
}

pub fn lee_trimming_bounds_log_mean(
    survived_mean_a: &[f64],
    survived_mean_b: &[f64],
) -> LeeBoundsReport {
    if survived_mean_a.is_empty() || survived_mean_b.is_empty() {
        return LeeBoundsReport {
            n_a: survived_mean_a.len(),
            n_b: survived_mean_b.len(),
            k: 0,
            lee_lo: None,
            lee_hi: None,
            width: None,
        };
    }
    let mut ya: Vec<f64> = survived_mean_a.iter().map(|v| v.ln()).collect();
    let mut yb: Vec<f64> = survived_mean_b.iter().map(|v| v.ln()).collect();
    ya.sort_by(f64::total_cmp);
    yb.sort_by(f64::total_cmp);
    let na = ya.len();
    let nb = yb.len();
    let k = na.min(nb);
    let lee_lo = mean_slice(&yb[..k]).unwrap_or(0.0) - mean_slice(&ya[na - k..]).unwrap_or(0.0);
    let lee_hi = mean_slice(&yb[nb - k..]).unwrap_or(0.0) - mean_slice(&ya[..k]).unwrap_or(0.0);
    LeeBoundsReport {
        n_a: na,
        n_b: nb,
        k,
        lee_lo: Some(lee_lo),
        lee_hi: Some(lee_hi),
        width: Some(lee_hi - lee_lo),
    }
}

#[derive(Clone, Debug)]
pub struct LachinObs {
    pub valid_mass: bool,
    pub survival_ticks: u64,
    pub mean_int: Option<f64>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LachinReport {
    pub n_pairs: usize,
    pub mean_rank_diff: Option<f64>,
    pub median_rank_diff: Option<f64>,
    pub p_two_sided: f64,
    pub r_c: Option<f64>,
    pub wilcoxon: WilcoxonReport,
}

pub fn lachin_worst_rank_paired(
    obs_a: &[LachinObs],
    obs_b: &[LachinObs],
    min_non_zero_r7: usize,
) -> Result<LachinReport, String> {
    if obs_a.len() != obs_b.len() {
        return Err("lachin_worst_rank_paired requires equal-length paired arrays".into());
    }
    let n = obs_a.len();
    #[derive(Clone, Copy)]
    struct Entry {
        tier: u8,
        key: f64,
        side: u8,
        idx: usize,
    }
    let mut pooled = Vec::with_capacity(2 * n);
    let push_obs = |pooled: &mut Vec<Entry>, o: &LachinObs, side: u8, idx: usize| {
        let valid_surv = o.valid_mass && o.mean_int.is_some_and(|v| v.is_finite() && v > 0.0);
        let (tier, key) = if valid_surv {
            (1_u8, o.mean_int.unwrap_or(0.0))
        } else {
            (0_u8, o.survival_ticks as f64)
        };
        pooled.push(Entry {
            tier,
            key,
            side,
            idx,
        });
    };
    for i in 0..n {
        push_obs(&mut pooled, &obs_a[i], 0, i);
        push_obs(&mut pooled, &obs_b[i], 1, i);
    }
    pooled.sort_by(|x, y| x.tier.cmp(&y.tier).then_with(|| x.key.total_cmp(&y.key)));

    let mut rank_a = vec![0.0_f64; n];
    let mut rank_b = vec![0.0_f64; n];
    let mut i = 0;
    while i < pooled.len() {
        let mut j = i + 1;
        while j < pooled.len() && pooled[j].tier == pooled[i].tier && pooled[j].key == pooled[i].key
        {
            j += 1;
        }
        let mid_rank = 0.5 * ((i + 1 + j) as f64);
        for k in i..j {
            if pooled[k].side == 0 {
                rank_a[pooled[k].idx] = mid_rank;
            } else {
                rank_b[pooled[k].idx] = mid_rank;
            }
        }
        i = j;
    }

    let rank_diffs: Vec<f64> = rank_a.iter().zip(&rank_b).map(|(ra, rb)| rb - ra).collect();
    let wilcoxon = wilcoxon_signed_rank(&rank_diffs, min_non_zero_r7);
    Ok(LachinReport {
        n_pairs: n,
        mean_rank_diff: mean_slice(&rank_diffs),
        median_rank_diff: median_slice(&rank_diffs),
        p_two_sided: wilcoxon.p_value,
        r_c: wilcoxon.r_c,
        wilcoxon,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct R10Decision {
    pub label: &'static str,
    pub direction_tag: &'static str,
}

#[allow(clippy::too_many_arguments)]
pub fn classify_sel2p2_decision(
    smoke_and_host_valid: bool,
    failed_runs_zero: bool,
    drift_and_shortfall_valid: bool,
    primary_mode: &str,
    n_complete_pairs: usize,
    n_requested: usize,
    min_pi_ll: f64,
    min_pairs_r7: usize,
    p_holm: Option<f64>,
    fwer_alpha: f64,
    mean_d: Option<f64>,
    tost_equivalent: bool,
    sesoi: f64,
    lachin_mean_rank_diff: Option<f64>,
    sace_lo: Option<f64>,
    sace_hi: Option<f64>,
) -> R10Decision {
    if !smoke_and_host_valid {
        return R10Decision {
            label: "insufficient_evidence",
            direction_tag: "harness_invalid",
        };
    }
    if !failed_runs_zero || !drift_and_shortfall_valid {
        return R10Decision {
            label: "insufficient_evidence",
            direction_tag: "attrition_below_floor_or_drift_invalid",
        };
    }

    let holm_sig = p_holm.is_some_and(|p| p.is_finite() && p < fwer_alpha);

    if primary_mode == "lachin_worst_rank" {
        if !holm_sig {
            return R10Decision {
                label: "insufficient_evidence",
                direction_tag: "inconclusive_wide_ci",
            };
        }
        let sign_pos =
            lachin_mean_rank_diff.is_some_and(|r| r > 0.0) && sace_lo.is_some_and(|lo| lo >= sesoi);
        let sign_neg = lachin_mean_rank_diff.is_some_and(|r| r < 0.0)
            && sace_hi.is_some_and(|hi| hi <= -sesoi);
        if sign_pos {
            return R10Decision {
                label: "supports",
                direction_tag: "lachin_two_sided_positive",
            };
        }
        if sign_neg {
            return R10Decision {
                label: "supports",
                direction_tag: "lachin_two_sided_negative",
            };
        }
        return R10Decision {
            label: "insufficient_evidence",
            direction_tag: "attrition_sace_straddles_or_inconclusive",
        };
    }

    let pi_ll = if n_requested > 0 {
        n_complete_pairs as f64 / n_requested as f64
    } else {
        0.0
    };
    let Some(m) = mean_d.filter(|v| v.is_finite()) else {
        return R10Decision {
            label: "insufficient_evidence",
            direction_tag: "attrition_below_floor_or_drift_invalid",
        };
    };
    if n_complete_pairs < min_pairs_r7 || pi_ll < min_pi_ll {
        return R10Decision {
            label: "insufficient_evidence",
            direction_tag: "attrition_below_floor_or_drift_invalid",
        };
    }

    let lt_sesoi = m.abs() < sesoi;
    if !lt_sesoi && tost_equivalent {
        return R10Decision {
            label: "insufficient_evidence",
            direction_tag: "ci_inconsistent_impossible",
        };
    }

    if holm_sig {
        if m >= sesoi {
            return R10Decision {
                label: "supports",
                direction_tag: "two_sided_positive",
            };
        }
        if m <= -sesoi {
            return R10Decision {
                label: "supports",
                direction_tag: "two_sided_negative",
            };
        }
        if tost_equivalent {
            return R10Decision {
                label: "equivalent_to_zero",
                direction_tag: "trivial_below_sesoi_equivalent",
            };
        }
        return R10Decision {
            label: "insufficient_evidence",
            direction_tag: "inconclusive_straddles_sesoi",
        };
    }

    if tost_equivalent && lt_sesoi {
        return R10Decision {
            label: "equivalent_to_zero",
            direction_tag: "equivalent_to_zero_powered_null",
        };
    }
    R10Decision {
        label: "insufficient_evidence",
        direction_tag: "inconclusive_wide_ci",
    }
}

// ---- Smoke Gate Types & Evaluators -------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct BaselineObservation {
    pub seed: u64,
    pub completed: bool,
    pub extinct_tick: Option<u64>,
    pub crop_second_half: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PooledBaseline {
    pub n_requested: usize,
    pub n_surviving: usize,
    pub min_required: usize,
    pub surviving_seeds: Vec<u64>,
    pub pooled_crop_second_half: Option<f64>,
    pub sufficient: bool,
}

pub fn evaluate_pooled_baseline(
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
    pub drift_valid: bool,
    pub has_shortfall: bool,
}

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

pub fn evaluate_smoke_gate(
    duration_ticks: u64,
    baseline: &PooledBaseline,
    selected: &ScarceGateInput,
    scarce_mut0: &ScarceGateInput,
    scarce_drift: &ScarceGateInput,
    spec: &SmokeGateSpec,
) -> GateReport {
    let survived = |x: &ScarceGateInput| x.completed && x.extinct_tick.is_none();

    let c1_arm = |x: &ScarceGateInput| {
        survived(x)
            && x.min_agents
                .is_some_and(|m| m.is_finite() && m >= spec.c1_min_agents)
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
    let g_in_band =
        |g: Option<f64>| g.is_some_and(|v| v >= spec.c2_g_band[0] && v <= spec.c2_g_band[1]);
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
    let c3 = generations_selected.is_some_and(|g| g >= spec.c3_min_generations)
        && generations_scarce_mut0.is_some_and(|g| g >= spec.c3_min_generations);

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
    let c4 = starvation_share_selected.is_some_and(|s| s > spec.c4_min_starvation_share)
        && starvation_share_scarce_mut0.is_some_and(|s| s > spec.c4_min_starvation_share);

    let c5_arm = |x: &ScarceGateInput| x.births_last_quarter > 0.0 && x.deaths_last_quarter > 0.0;
    let c5 = c5_arm(selected) && c5_arm(scarce_mut0);

    let drift_valid =
        survived(scarce_drift) && scarce_drift.drift_valid && !scarce_drift.has_shortfall;
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

// ---- Entrypoint & CLI --------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run_with_config(args, RuntimeEnv::from_process()) {
        eprintln!("run_sel2p2_experiment refused or failed: {e}");
        std::process::exit(2);
    }
}

pub fn run_with_args(argv: Vec<String>) -> Result<(), String> {
    run_with_config(argv, RuntimeEnv::from_process())
}

pub fn run_with_config(argv: Vec<String>, env: RuntimeEnv) -> Result<(), String> {
    let args = parse_args(argv)?;
    match args.mode {
        Mode::Help => {
            println!(
                "usage: run_sel2p2_experiment (--dry-run | --smoke | --confirm | --run-all | --self-test) \
                 [--manifest-dir <dir>] [--smoke-dir <dir>] [--out <dir>] [--checkpoint-dir <dir>] [--jobs <n>]"
            );
            Ok(())
        }
        Mode::DryRun => run_dry_run(&args),
        Mode::SelfTest => run_self_test(&args),
        Mode::Smoke => run_smoke_mode(&args, &env),
        Mode::Confirm => run_confirm_mode(&args, &env),
        Mode::RunAll => run_all_mode(&args, &env),
    }
}

fn resolve_default_manifest_dir() -> PathBuf {
    let direct = Path::new(DEFAULT_MANIFEST_DIR);
    if direct.join(PREREG_FILE).exists() {
        return direct.to_path_buf();
    }
    let from_crate =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/experiments_sel2p2");
    if from_crate.join(PREREG_FILE).exists() {
        return from_crate;
    }
    direct.to_path_buf()
}

fn parse_args(argv: Vec<String>) -> Result<Args, String> {
    let mut mode: Option<Mode> = None;
    let mut manifest_dir = resolve_default_manifest_dir();
    let mut out: Option<PathBuf> = None;
    let mut smoke_dir: Option<PathBuf> = None;
    let mut checkpoint_dir: Option<PathBuf> = None;
    let mut jobs: usize = 1;

    let mut it = argv.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--help" | "-h" => set_mode(&mut mode, Mode::Help)?,
            "--dry-run" => set_mode(&mut mode, Mode::DryRun)?,
            "--self-test" => set_mode(&mut mode, Mode::SelfTest)?,
            "--smoke" => set_mode(&mut mode, Mode::Smoke)?,
            "--confirm" => set_mode(&mut mode, Mode::Confirm)?,
            "--run-all" => set_mode(&mut mode, Mode::RunAll)?,
            "--manifest-dir" => {
                manifest_dir = PathBuf::from(need_val(&mut it, "--manifest-dir")?);
            }
            "--out" => {
                out = Some(PathBuf::from(need_val(&mut it, "--out")?));
            }
            "--smoke-dir" => {
                smoke_dir = Some(PathBuf::from(need_val(&mut it, "--smoke-dir")?));
            }
            "--checkpoint-dir" => {
                checkpoint_dir = Some(PathBuf::from(need_val(&mut it, "--checkpoint-dir")?));
            }
            "--jobs" => {
                let raw = need_val(&mut it, "--jobs")?;
                let parsed: usize = raw
                    .parse()
                    .map_err(|_| format!("--jobs must be a positive integer, got '{raw}'"))?;
                if parsed == 0 {
                    return Err("--jobs must be >= 1".into());
                }
                jobs = parsed;
            }
            other => return Err(format!("unknown flag '{other}'")),
        }
    }

    let mode = mode.ok_or(
        "must pass one mode: --dry-run, --smoke, --confirm, --run-all, --self-test, or --help",
    )?;
    Ok(Args {
        mode,
        manifest_dir,
        out,
        smoke_dir,
        checkpoint_dir,
        jobs,
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

// ---- Preregistration & Manifest Validation -----------------------------------------------------

pub fn load_prereg(manifest_dir: &Path) -> Result<PreregSpec, String> {
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
    let prereg_id = raw["preregistration_id"]
        .as_str()
        .ok_or("preregistration missing preregistration_id")?
        .to_string();

    let mf = &raw["manifests"];
    let str_field = |v: &serde_json::Value, name: &str| -> Result<String, String> {
        v[name]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("preregistration missing manifests.{name}"))
    };
    let selected_file = str_field(mf, "selected")?;
    let scarce_mut0_file = str_field(mf, "scarce_mut0")?;
    let scarce_drift_file = str_field(mf, "scarce_drift")?;
    let baseline_ref_file = str_field(mf, "baseline_ref")?;
    let smoke_selected_file = str_field(mf, "smoke_selected")?;
    let smoke_mut0_file = str_field(mf, "smoke_scarce_mut0")?;
    let smoke_drift_file = str_field(mf, "smoke_scarce_drift")?;

    let u64_vec = |v: &serde_json::Value, name: &str| -> Result<Vec<u64>, String> {
        let arr = v
            .as_array()
            .ok_or_else(|| format!("preregistration missing {name}"))?;
        arr.iter()
            .map(|x| x.as_u64().ok_or_else(|| format!("non-u64 entry in {name}")))
            .collect()
    };
    let execution_order = u64_vec(&raw["seeds"]["execution_order"], "seeds.execution_order")?;
    if execution_order.is_empty() {
        return Err("preregistration seeds.execution_order is empty".into());
    }
    let baseline_ref_seeds = u64_vec(
        &raw["seeds"]["baseline_ref_seeds"],
        "seeds.baseline_ref_seeds",
    )?;
    if baseline_ref_seeds.is_empty() {
        return Err("preregistration seeds.baseline_ref_seeds is empty".into());
    }
    let smoke_seed = raw["seeds"]["smoke_seed"]
        .as_u64()
        .ok_or("preregistration missing seeds.smoke_seed")?;

    let confirm_set: BTreeSet<u64> = execution_order.iter().copied().collect();
    let baseline_set: BTreeSet<u64> = baseline_ref_seeds.iter().copied().collect();
    if confirm_set.len() != execution_order.len() {
        return Err("duplicate seed in seeds.execution_order".into());
    }
    if baseline_set.len() != baseline_ref_seeds.len() {
        return Err("duplicate seed in seeds.baseline_ref_seeds".into());
    }
    if confirm_set.contains(&smoke_seed) || baseline_set.contains(&smoke_seed) {
        return Err(format!(
            "smoke_seed {smoke_seed} collides with confirmation/baseline"
        ));
    }
    if !confirm_set.is_disjoint(&baseline_set) {
        return Err("confirmation and baseline_ref seeds must be disjoint".into());
    }

    let dur = &raw["duration"];
    let duration_ticks = dur["duration_ticks"]
        .as_u64()
        .ok_or("preregistration missing duration.duration_ticks")?;
    let warmup_ticks = dur["warmup_ticks"]
        .as_u64()
        .ok_or("preregistration missing duration.warmup_ticks")?;
    let confirmation_sample_period = dur["confirmation_sample_period"]
        .as_u64()
        .ok_or("preregistration missing duration.confirmation_sample_period")?;
    let smoke_sample_period = dur["smoke_sample_period"]
        .as_u64()
        .ok_or("preregistration missing duration.smoke_sample_period")?;
    let tail20_threshold_tick = dur["tail20_threshold_tick"]
        .as_u64()
        .ok_or("preregistration missing duration.tail20_threshold_tick")?;

    let art = &raw["artifacts"];
    let artifacts_root = art["root"]
        .as_str()
        .unwrap_or("artifacts/experiments/sel2-p2/")
        .to_string();
    let smoke_dir_default = art["smoke_dir"]
        .as_str()
        .unwrap_or(DEFAULT_SMOKE_OUT)
        .to_string();
    let confirmation_dir_default = art["confirmation_dir"]
        .as_str()
        .unwrap_or(DEFAULT_CONFIRM_OUT)
        .to_string();

    let sg = &raw["smoke_gate"];
    let c2_arr = sg["c2_g_band"]
        .as_array()
        .ok_or("smoke_gate missing c2_g_band")?;
    if c2_arr.len() != 2 {
        return Err("smoke_gate.c2_g_band must have length 2".into());
    }
    let smoke_gate = SmokeGateSpec {
        min_surviving_baseline_seeds: raw["seeds"]["min_surviving_baseline_seeds"]
            .as_u64()
            .or_else(|| sg["min_surviving_baseline_seeds"].as_u64())
            .ok_or("missing min_surviving_baseline_seeds")?
            as usize,
        c1_min_agents: sg["c1_min_agents"]
            .as_f64()
            .ok_or("missing c1_min_agents")?,
        c2_g_band: [
            c2_arr[0].as_f64().ok_or("c2_g_band[0]")?,
            c2_arr[1].as_f64().ok_or("c2_g_band[1]")?,
        ],
        c3_min_generations: sg["c3_min_generations"]
            .as_f64()
            .ok_or("missing c3_min_generations")?,
        c4_min_starvation_share: sg["c4_min_starvation_share"]
            .as_f64()
            .ok_or("missing c4_min_starvation_share")?,
    };

    let th = &raw["thresholds"];
    let thresholds = ThresholdsSpec {
        fwer_alpha: th["fwer_alpha"].as_f64().ok_or("missing fwer_alpha")?,
        sesoi_log_mean_ratio: th["sesoi_log_mean_ratio"]
            .as_f64()
            .ok_or("missing sesoi_log_mean_ratio")?,
        max_ext_rate: th["max_arm_extinction_rate_for_complete_pairs"]
            .as_f64()
            .ok_or("missing max_arm_extinction_rate_for_complete_pairs")?,
        min_pi_ll: th["min_pi_ll"].as_f64().ok_or("missing min_pi_ll")?,
        min_valid_pairs: th["min_valid_pairs_for_decision"]
            .as_u64()
            .ok_or("missing min_valid_pairs_for_decision")? as usize,
        min_non_zero_pairs_r7: th["min_non_zero_pairs_r7"]
            .as_u64()
            .ok_or("missing min_non_zero_pairs_r7")? as usize,
        max_abs_eu_drift: th["max_abs_eu_drift"]
            .as_f64()
            .ok_or("missing max_abs_eu_drift")?,
        rmst_tau_ticks: th["rmst_tau_ticks"].as_u64().unwrap_or(duration_ticks),
        bca_resamples: th["bca_bootstrap_resamples"].as_u64().unwrap_or(10_000) as usize,
    };

    Ok(PreregSpec {
        status,
        prereg_id,
        selected_file,
        scarce_mut0_file,
        scarce_drift_file,
        baseline_ref_file,
        smoke_selected_file,
        smoke_mut0_file,
        smoke_drift_file,
        execution_order,
        baseline_ref_seeds,
        smoke_seed,
        duration_ticks,
        warmup_ticks,
        confirmation_sample_period,
        smoke_sample_period,
        tail20_threshold_tick,
        artifacts_root,
        smoke_dir_default,
        confirmation_dir_default,
        smoke_gate,
        thresholds,
    })
}

pub struct ManifestBundle {
    pub selected: ExperimentManifest,
    pub scarce_mut0: ExperimentManifest,
    pub scarce_drift: ExperimentManifest,
    pub baseline_ref: ExperimentManifest,
    pub smoke_selected: ExperimentManifest,
    pub smoke_mut0: ExperimentManifest,
    pub smoke_drift: ExperimentManifest,
}

pub fn load_and_validate_all_manifests(
    manifest_dir: &Path,
    prereg: &PreregSpec,
    registry: &ObservableRegistry,
) -> Result<ManifestBundle, String> {
    registry
        .validate()
        .map_err(|e| format!("invalid live registry: {e}"))?;
    let selected = load_manifest(&manifest_dir.join(&prereg.selected_file))?;
    let scarce_mut0 = load_manifest(&manifest_dir.join(&prereg.scarce_mut0_file))?;
    let scarce_drift = load_manifest(&manifest_dir.join(&prereg.scarce_drift_file))?;
    let baseline_ref = load_manifest(&manifest_dir.join(&prereg.baseline_ref_file))?;
    let smoke_selected = load_manifest(&manifest_dir.join(&prereg.smoke_selected_file))?;
    let smoke_mut0 = load_manifest(&manifest_dir.join(&prereg.smoke_mut0_file))?;
    let smoke_drift = load_manifest(&manifest_dir.join(&prereg.smoke_drift_file))?;

    for (label, m, expected_seeds, expected_period, must_be_blind) in [
        (
            "selected",
            &selected,
            &prereg.execution_order,
            prereg.confirmation_sample_period,
            false,
        ),
        (
            "scarce_mut0",
            &scarce_mut0,
            &prereg.execution_order,
            prereg.confirmation_sample_period,
            false,
        ),
        (
            "scarce_drift",
            &scarce_drift,
            &prereg.execution_order,
            prereg.confirmation_sample_period,
            false,
        ),
        (
            "baseline_ref",
            &baseline_ref,
            &prereg.baseline_ref_seeds,
            prereg.smoke_sample_period,
            true,
        ),
    ] {
        validate_manifest_common(label, m, prereg, registry, expected_period)?;
        if &m.seeds != expected_seeds {
            return Err(format!("{label}: seeds disagree with preregistration"));
        }
        if must_be_blind {
            ensure_manifest_blind(label, m)?;
        }
    }

    let smoke_expected = vec![prereg.smoke_seed];
    for (label, m) in [
        ("smoke_selected", &smoke_selected),
        ("smoke_scarce_mut0", &smoke_mut0),
        ("smoke_scarce_drift", &smoke_drift),
    ] {
        validate_manifest_common(label, m, prereg, registry, prereg.smoke_sample_period)?;
        if m.seeds != smoke_expected {
            return Err(format!("{label}: seeds disagree with smoke_seed"));
        }
        ensure_manifest_blind(label, m)?;
    }

    let ic_only = FactorDiff {
        allowed_paths: vec!["initial_conditions".into()],
    };
    ic_only
        .validate(&selected, &scarce_drift)
        .map_err(|e| format!("FactorDiff(selected, scarce_drift): {e}"))?;
    ic_only
        .validate(&selected, &scarce_mut0)
        .map_err(|e| format!("FactorDiff(selected, scarce_mut0): {e}"))?;
    ic_only
        .validate(&scarce_drift, &scarce_mut0)
        .map_err(|e| format!("FactorDiff(scarce_drift, scarce_mut0): {e}"))?;

    let ic_and_obs = FactorDiff {
        allowed_paths: vec!["initial_conditions".into(), "observables".into()],
    };
    ic_and_obs
        .validate(&smoke_selected, &smoke_drift)
        .map_err(|e| format!("FactorDiff(smoke_selected, smoke_drift): {e}"))?;
    ic_and_obs
        .validate(&smoke_selected, &smoke_mut0)
        .map_err(|e| format!("FactorDiff(smoke_selected, smoke_mut0): {e}"))?;

    FactorDiff {
        allowed_paths: vec!["initial_conditions".into(), "seeds".into()],
    }
    .validate(&baseline_ref, &smoke_selected)
    .map_err(|e| format!("FactorDiff(baseline_ref, smoke_selected): {e}"))?;

    Ok(ManifestBundle {
        selected,
        scarce_mut0,
        scarce_drift,
        baseline_ref,
        smoke_selected,
        smoke_mut0,
        smoke_drift,
    })
}

fn validate_manifest_common(
    label: &str,
    m: &ExperimentManifest,
    prereg: &PreregSpec,
    registry: &ObservableRegistry,
    expected_sample_period: u64,
) -> Result<(), String> {
    if m.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(format!(
            "{label}: unexpected schema_version {}",
            m.schema_version
        ));
    }
    m.validate(registry)
        .map_err(|e| format!("{label} validation failed: {e}"))?;
    if m.duration_ticks != prereg.duration_ticks {
        return Err(format!(
            "{label}: duration_ticks {} != {}",
            m.duration_ticks, prereg.duration_ticks
        ));
    }
    if m.sample_period != expected_sample_period {
        return Err(format!(
            "{label}: sample_period {} != {}",
            m.sample_period, expected_sample_period
        ));
    }
    Ok(())
}

fn ensure_manifest_blind(label: &str, m: &ExperimentManifest) -> Result<(), String> {
    for obs in &m.observable_ids {
        if obs.to_ascii_lowercase().contains("trait_mass") {
            return Err(format!(
                "blinding violation: {label} contains body-mass observable '{obs}'"
            ));
        }
    }
    Ok(())
}

// ---- Pre-run & Path Guards ---------------------------------------------------------------------

fn refuse_debug(env: &RuntimeEnv, mode: Mode) -> Result<(), String> {
    if env.is_debug_build {
        return Err(format!(
            "refuse --{}: SEL2-P2 execution requires a --release build (debug_assertions = true)",
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

fn ensure_out_inside_prereg_root(
    out_dir: &Path,
    prereg: &PreregSpec,
    env: &RuntimeEnv,
) -> Result<(), String> {
    if env.allow_arbitrary_out {
        return Ok(());
    }
    let allowed_root = abs_clean(Path::new(&prereg.artifacts_root));
    let target = abs_clean(out_dir);
    if !target.starts_with(&allowed_root) {
        return Err(format!(
            "refuse: --out '{}' is outside preregistered experiment directory '{}'",
            out_dir.display(),
            prereg.artifacts_root
        ));
    }
    Ok(())
}

fn check_world_artifact_prerun(env: &RuntimeEnv) -> Result<serde_json::Value, String> {
    if let Some(val) = &env.anima_world_artifact {
        return Err(format!(
            "refuse: ANIMA_WORLD_ARTIFACT is set ('{val}'); SEL2-P2 requires it unset"
        ));
    }
    let temp_artifact = env.temp_dir.join(TEMP_ARTIFACT_NAME);
    if temp_artifact.exists() {
        return Err(format!(
            "refuse: desktop world artifact {} exists; SEL2-P2 requires it absent",
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

// ---- `--dry-run` & `--self-test` ---------------------------------------------------------------

fn run_dry_run(args: &Args) -> Result<(), String> {
    let registry = ObservableRegistry::live_default();
    let prereg = load_prereg(&args.manifest_dir)?;
    let _bundle = load_and_validate_all_manifests(&args.manifest_dir, &prereg, &registry)?;

    println!(
        "SEL2-P2 DRY RUN (validated FROZEN preregistration; 0 seeds executed, 0 files written)"
    );
    println!("preregistration_id: {}", prereg.prereg_id);
    println!("preregistration_status: {}", prereg.status);
    println!("model_version: {LIVE_MODEL_VERSION}");
    println!("experiment_root: {}", prereg.artifacts_root);
    println!("smoke_out_default: {}", prereg.smoke_dir_default);
    println!(
        "confirmation_out_default: {}",
        prereg.confirmation_dir_default
    );
    println!(
        "duration_ticks: {} (warmup: {}, confirm_sample_period: {}, smoke_sample_period: {})",
        prereg.duration_ticks,
        prereg.warmup_ticks,
        prereg.confirmation_sample_period,
        prereg.smoke_sample_period
    );
    println!();
    println!(
        "Step 1 — QD-SEL2g pooled baseline_ref ({} seeds, min_surviving = {}, blind non-mass):",
        prereg.baseline_ref_seeds.len(),
        prereg.smoke_gate.min_surviving_baseline_seeds
    );
    for (idx, seed) in prereg.baseline_ref_seeds.iter().enumerate() {
        println!(
            "  [{:03}] stage=baseline_ref arm=baseline_ref seed={seed} manifest={}",
            idx + 1,
            prereg.baseline_ref_file
        );
    }
    println!();
    println!(
        "Step 2 — Blind smoke seed {} (C1..C5 + drift_valid hard stop; 0 trait_mass allowed):",
        prereg.smoke_seed
    );
    for (idx, (arm, mf)) in [
        ("selected", &prereg.smoke_selected_file),
        ("scarce_drift", &prereg.smoke_drift_file),
        ("scarce_mut0", &prereg.smoke_mut0_file),
    ]
    .iter()
    .enumerate()
    {
        println!(
            "  [{:03}] stage=smoke arm={arm} seed={} manifest={mf}",
            idx + 1,
            prereg.smoke_seed
        );
    }
    println!();
    println!(
        "Step 3 — Confirmation ensemble ({} pairs x 3 arms = {} runs, paired CRN):",
        prereg.execution_order.len(),
        prereg.execution_order.len() * ARM_ORDER.len()
    );
    let mut seq = 1usize;
    for seed in &prereg.execution_order {
        for (arm, mf) in [
            ("selected", &prereg.selected_file),
            ("scarce_drift", &prereg.scarce_drift_file),
            ("scarce_mut0", &prereg.scarce_mut0_file),
        ] {
            println!("  [{seq:03}] stage=confirmation arm={arm} seed={seed} manifest={mf}");
            seq += 1;
        }
    }
    Ok(())
}

fn run_self_test(args: &Args) -> Result<(), String> {
    let t975 = student_t_inv(0.975, 47);
    let t950 = student_t_inv(0.95, 47);
    if (t975 - 2.01174051).abs() > 1e-6 || (t950 - 1.67792672).abs() > 1e-6 {
        return Err(format!("student_t_inv(df=47) mismatch: {t975}, {t950}"));
    }
    let p_holm = holm_adjust(&[Some(0.01), Some(0.04), Some(0.03)]);
    if (p_holm[0].unwrap_or(1.0) - 0.03).abs() > 1e-12
        || (p_holm[1].unwrap_or(1.0) - 0.06).abs() > 1e-12
        || (p_holm[2].unwrap_or(1.0) - 0.06).abs() > 1e-12
    {
        return Err("holm_adjust mismatch".into());
    }
    let registry = ObservableRegistry::live_default();
    let prereg = load_prereg(&args.manifest_dir)?;
    let _ = load_and_validate_all_manifests(&args.manifest_dir, &prereg, &registry)?;
    println!("run_sel2p2_experiment --self-test PASSED");
    Ok(())
}

// ---- `--smoke` (Step 1 baseline_ref + Step 2 blind smoke hard stop) ----------------------------

struct BlindSmokeRun {
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
    drift_valid: bool,
    has_shortfall: bool,
    terrain_source: Option<String>,
    world_identity: Option<WorldIdentity>,
    series: Vec<(u64, [Option<f64>; 9])>,
}

fn guard_no_body_mass(what: &str, text: &str) -> Result<(), String> {
    if text.to_ascii_lowercase().contains("trait_mass") {
        Err(format!(
            "smoke blindness violation: {what} contains forbidden body-mass substring"
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

fn strip_to_blind_run(
    arm: &'static str,
    seed: u64,
    run: RunResult,
    allow_cause_split: bool,
    duration_ticks: u64,
    elapsed_secs: f64,
    has_shortfall: bool,
) -> BlindSmokeRun {
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
        let mut row = [None; 9];
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

    if extinct_tick.is_none() && run.observable("live.agent_count") == Some(0.0) {
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
    let drift_valid = run.status.is_completed() && extinct_tick.is_none() && !has_shortfall;

    BlindSmokeRun {
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
        drift_valid,
        has_shortfall,
        terrain_source: run.provenance.terrain_source,
        world_identity: run.provenance.world_identity,
        series,
    }
}

fn to_scarce_gate_input(r: &BlindSmokeRun) -> ScarceGateInput {
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
        drift_valid: r.drift_valid,
        has_shortfall: r.has_shortfall,
    }
}

fn run_smoke_mode(args: &Args, env: &RuntimeEnv) -> Result<(), String> {
    refuse_debug(env, Mode::Smoke)?;
    let prereg = load_prereg(&args.manifest_dir)?;
    let out_dir = args
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from(&prereg.smoke_dir_default));
    ensure_out_inside_prereg_root(&out_dir, &prereg, env)?;
    ensure_empty_out_dir(&out_dir)?;
    let prerun_check = check_world_artifact_prerun(env)?;

    let registry = ObservableRegistry::live_default();
    let bundle = load_and_validate_all_manifests(&args.manifest_dir, &prereg, &registry)?;
    let duration_ticks = prereg.duration_ticks;

    // Step 1: QD-SEL2g pooled baseline BEFORE any scarce run executes.
    let baseline_started = SystemTime::now();
    let mut baseline_runs = Vec::with_capacity(bundle.baseline_ref.seeds.len());
    for &seed in &bundle.baseline_ref.seeds {
        let clock = Instant::now();
        let res = run_manifest_seed::<LiveExperimentAdapter>(
            &bundle.baseline_ref,
            &registry,
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
            false,
        );
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
    let pooled = evaluate_pooled_baseline(
        &baseline_obs,
        prereg.smoke_gate.min_surviving_baseline_seeds,
    );

    // Step 2: Blind scarce smoke runs after `pooled` is finalized.
    let scarce_started = SystemTime::now();
    let smoke_seed = prereg.smoke_seed;
    let mut sel_for_drift = bundle.smoke_selected.clone();
    sel_for_drift.observable_ids = bundle.smoke_drift.observable_ids.clone();

    let drift_clock = Instant::now();
    let pair = run_live_drift_pair(&sel_for_drift, &bundle.smoke_drift, &registry, smoke_seed)
        .map_err(|e| format!("smoke drift pair failed: {e}"))?;
    let pair_elapsed = drift_clock.elapsed().as_secs_f64();
    let drift_diag = pair.diagnostics.clone();
    let has_shortfall = drift_diag.has_shortfall();

    let sel_blind = strip_to_blind_run(
        "selected",
        smoke_seed,
        pair.selected,
        true,
        duration_ticks,
        pair_elapsed * 0.5,
        false,
    );
    let drift_blind = strip_to_blind_run(
        "scarce_drift",
        smoke_seed,
        pair.drift,
        false,
        duration_ticks,
        pair_elapsed * 0.5,
        has_shortfall,
    );

    let mut0_clock = Instant::now();
    let mut0_res = run_manifest_seed::<LiveExperimentAdapter>(
        &bundle.smoke_mut0,
        &registry,
        smoke_seed,
        None,
        None,
    );
    let mut0_elapsed = mut0_clock.elapsed().as_secs_f64();
    let mut0_blind = strip_to_blind_run(
        "scarce_mut0",
        smoke_seed,
        mut0_res,
        true,
        duration_ticks,
        mut0_elapsed,
        false,
    );
    let scarce_ended = SystemTime::now();

    let gate = evaluate_smoke_gate(
        duration_ticks,
        &pooled,
        &to_scarce_gate_input(&sel_blind),
        &to_scarce_gate_input(&mut0_blind),
        &to_scarce_gate_input(&drift_blind),
        &prereg.smoke_gate,
    );

    let all_runs: Vec<&BlindSmokeRun> = baseline_runs
        .iter()
        .chain([&sel_blind, &mut0_blind, &drift_blind])
        .collect();

    let mut expected_wid: Option<(String, WorldIdentity)> = None;
    for r in &all_runs {
        let src = r
            .terrain_source
            .as_deref()
            .ok_or_else(|| format!("smoke {}: missing terrain_source", r.arm))?;
        let wid = r
            .world_identity
            .ok_or_else(|| format!("smoke {}: missing world_identity", r.arm))?;
        match &expected_wid {
            None => expected_wid = Some((src.to_string(), wid)),
            Some((es, ei)) if es == src && *ei == wid => {}
            Some((es, ei)) => {
                return Err(format!(
                    "smoke {}: mixed terrain/world_identity: ({es}, {ei:?}) vs ({src}, {wid:?})",
                    r.arm
                ));
            }
        }
    }
    let (terrain_source, mut world_identity) = expected_wid.ok_or("no smoke runs executed")?;
    if let Some(hook) = env.world_identity_override {
        world_identity = hook(0, "smoke", smoke_seed, world_identity);
    }

    std::fs::create_dir_all(&out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;

    let mut runs_csv = String::from(
        "# BLIND SMOKE: allow-listed non-mass observables only\n\
         seed,arm,status,extinct_tick,min_agent_count,mean_agent_count,crop_second_half,\
         total_births,total_deaths,mean_lifespan,total_starvation,total_dehydration,\
         cause_partition_ok,births_last_quarter,deaths_last_quarter,drift_valid,has_shortfall,us_per_tick,final_checksum\n",
    );
    for r in &all_runs {
        runs_csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{}\n",
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
            r.drift_valid,
            r.has_shortfall,
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

    let run_json = |r: &BlindSmokeRun| {
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
            "drift_valid": r.drift_valid,
            "has_shortfall": r.has_shortfall,
            "final_checksum": r.checksum,
        })
    };

    let smoke_report = serde_json::json!({
        "kind": "sel2p2-blind-smoke-report",
        "preregistration_id": prereg.prereg_id,
        "seed": smoke_seed,
        "duration_ticks": duration_ticks,
        "warmup_ticks": prereg.warmup_ticks,
        "sample_period": prereg.smoke_sample_period,
        "body_mass_consulted": false,
        "gate_pass": gate.gate_pass,
        "pooled_baseline": {
            "n_requested": pooled.n_requested,
            "n_surviving": pooled.n_surviving,
            "min_required": pooled.min_required,
            "surviving_seeds": pooled.surviving_seeds,
            "pooled_crop_second_half": pooled.pooled_crop_second_half,
            "sufficient": pooled.sufficient,
        },
        "gate": {
            "c1_min_agents": gate.c1,
            "c2_g_band": gate.c2,
            "c3_generations": gate.c3,
            "c4_starvation_share": gate.c4,
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
        "drift_diagnostics": serde_json::to_value(&drift_diag).map_err(|e| e.to_string())?,
        "runs": all_runs.iter().map(|r| run_json(r)).collect::<Vec<_>>(),
    });

    let git_commit = resolve_git_commit(env);
    let git_dirty_paths = resolve_git_dirty_paths(env);
    let prov = serde_json::json!({
        "kind": "sel2p2-blind-smoke-provenance",
        "preregistration_id": prereg.prereg_id,
        "preregistration_status": prereg.status,
        "body_mass_consulted": false,
        "baseline_computed_before_scarce": true,
        "model": "core::live_experiment::LiveExperimentAdapter",
        "model_version": LIVE_MODEL_VERSION,
        "terrain_source": terrain_source,
        "world_identity_observed": world_identity,
        "world_artifact_prerun_check": prerun_check,
        "build": {
            "git_commit": git_commit,
            "git_describe": git(&["describe", "--always", "--dirty"]),
            "git_dirty_paths": git_dirty_paths,
            "git_tree_dirty": !git_dirty_paths.is_empty(),
            "profile": if env.is_debug_build { "debug" } else { "release" },
            "rustc": tool_version("rustc"),
            "package_version": env!("CARGO_PKG_VERSION"),
            "binary_sha256": current_binary_sha256(),
        },
        "host": {
            "os": env.host_os,
            "arch": env.host_arch,
        },
        "timing": {
            "baseline_started_utc": iso8601_utc(unix_seconds(baseline_started)),
            "baseline_ended_utc": iso8601_utc(unix_seconds(baseline_ended)),
            "scarce_started_utc": iso8601_utc(unix_seconds(scarce_started)),
            "scarce_ended_utc": iso8601_utc(unix_seconds(scarce_ended)),
        },
        "smoke_seed": smoke_seed,
        "baseline_seeds": prereg.baseline_ref_seeds,
        "duration_ticks": duration_ticks,
        "warmup_ticks": prereg.warmup_ticks,
        "sample_period": prereg.smoke_sample_period,
    });

    let summary_md = format!(
        "# SEL2-P2 Blind Smoke Report\n\n\
         > Blind to body mass (`body_mass_consulted = false`).\n\n\
         - `smoke_seed`: `{smoke_seed}`\n\
         - `baseline_ref` (`{}..={}`): surviving **{}/{}** (min `{}`)\n\
         - gates: `C1={}`, `C2={}`, `C3={}`, `C4={}`, `C5={}`, `drift_valid={}` -> **`gate_pass={}`**\n\
         - `g`: `selected={:?}`, `scarce_mut0={:?}`, `scarce_drift={:?}`\n\
         - `generations`: `selected={:?}`, `scarce_mut0={:?}`\n\
         - `starvation_share`: `selected={:?}`, `scarce_mut0={:?}`\n",
        prereg.baseline_ref_seeds.first().copied().unwrap_or(0),
        prereg.baseline_ref_seeds.last().copied().unwrap_or(0),
        pooled.n_surviving,
        pooled.n_requested,
        pooled.min_required,
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

    let report_text = pretty(&smoke_report)? + "\n";
    let prov_text = pretty(&prov)? + "\n";
    for (name, text) in [
        ("smoke-runs.csv", &runs_csv),
        ("smoke-series.csv", &series_csv),
        ("smoke-report.json", &report_text),
        ("provenance.json", &prov_text),
        ("summary.md", &summary_md),
    ] {
        guard_no_body_mass(name, text)?;
    }

    write_text(&out_dir.join("smoke-runs.csv"), &runs_csv)?;
    write_text(&out_dir.join("smoke-series.csv"), &series_csv)?;
    write_text(&out_dir.join("smoke-report.json"), &report_text)?;
    write_text(&out_dir.join("provenance.json"), &prov_text)?;
    write_text(&out_dir.join("summary.md"), &summary_md)?;
    write_checksums(
        &out_dir,
        &[
            "provenance.json",
            "smoke-report.json",
            "smoke-runs.csv",
            "smoke-series.csv",
            "summary.md",
        ],
    )?;
    assert_dir_has_no_body_mass(&out_dir)?;

    say_blind(&format!(
        "SEL2-P2 blind smoke seed {smoke_seed}: baseline {}/{} (min {}), C1={} C2={} C3={} C4={} C5={} drift_valid={} -> gate_pass={} ({})",
        pooled.n_surviving,
        pooled.n_requested,
        pooled.min_required,
        gate.c1,
        gate.c2,
        gate.c3,
        gate.c4,
        gate.c5,
        gate.drift_valid,
        gate.gate_pass,
        out_dir.display()
    ))?;

    if !gate.gate_pass {
        return Err(format!(
            "smoke gate FAILED (hard stop): C1={} C2={} C3={} C4={} C5={} drift_valid={} -> refusing confirmation",
            gate.c1, gate.c2, gate.c3, gate.c4, gate.c5, gate.drift_valid
        ));
    }
    Ok(())
}

// ---- `--confirm` (Step 3 confirmation ensemble) ------------------------------------------------

fn validate_smoke_dir_for_confirm(dir: &Path) -> Result<SmokeVerification, String> {
    let prov_path = dir.join("provenance.json");
    let report_path = dir.join("smoke-report.json");
    let checksum_path = dir.join("checksums.sha256");
    if !prov_path.exists() || !report_path.exists() || !checksum_path.exists() {
        return Err(format!(
            "smoke hard stop: {} must contain provenance.json, smoke-report.json, and checksums.sha256",
            dir.display()
        ));
    }
    assert_dir_has_no_body_mass(dir)?;
    verify_checksum_manifest(dir, &["provenance.json", "smoke-report.json"])?;

    let prov = read_json(&prov_path)?;
    let report = read_json(&report_path)?;
    if report["body_mass_consulted"].as_bool() == Some(true) {
        return Err("smoke hard stop: smoke record violated body-mass blinding".into());
    }
    let gate_pass = report["gate_pass"].as_bool() == Some(true);
    if !gate_pass {
        return Err(format!(
            "smoke hard stop: {} has gate_pass != true; refusing confirmation",
            dir.display()
        ));
    }

    let wid_obj = &prov["world_identity_observed"];
    let world_identity = if wid_obj.is_object() {
        let sim_world_sha256 = match wid_obj.get("sim_world_sha256") {
            Some(v) if !v.is_null() => {
                let hex = v
                    .as_str()
                    .ok_or("smoke world_identity.sim_world_sha256 must be hex string")?;
                Some(
                    anima_worldgen::anmw_v3::sha256_from_hex(hex)
                        .map_err(|e| format!("smoke sim_world_sha256: {e}"))?,
                )
            }
            _ => None,
        };
        Some(WorldIdentity {
            seed: wid_obj["seed"].as_u64().ok_or("smoke wid seed")? as u32,
            generator_version: wid_obj["generator_version"]
                .as_u64()
                .ok_or("smoke wid generator_version")? as u32,
            width: wid_obj["width"].as_u64().ok_or("smoke wid width")? as u32,
            height: wid_obj["height"].as_u64().ok_or("smoke wid height")? as u32,
            checksum: wid_obj["checksum"].as_u64().ok_or("smoke wid checksum")? as u32,
            sim_world_sha256,
            genesis_phase: None,
        })
    } else {
        None
    };

    Ok(SmokeVerification {
        dir_display: dir.display().to_string().replace('\\', "/"),
        provenance_sha256: sha256_file(&prov_path).map_err(|e| e.to_string())?,
        smoke_report_sha256: sha256_file(&report_path).map_err(|e| e.to_string())?,
        git_commit: prov["build"]["git_commit"].as_str().map(str::to_string),
        host_os: prov["host"]["os"].as_str().map(str::to_string),
        host_arch: prov["host"]["arch"].as_str().map(str::to_string),
        profile: prov["build"]["profile"].as_str().map(str::to_string),
        world_identity,
        gate_pass,
    })
}

fn try_load_checkpoint(ckpt_dir: Option<&Path>, seed: u64) -> Option<RunCheckpoint> {
    let path = ckpt_dir?.join(format!("seed_{seed}.json"));
    let raw = std::fs::read_to_string(path).ok()?;
    let cp: RunCheckpoint = serde_json::from_str(&raw).ok()?;
    if cp.seed == seed {
        Some(cp)
    } else {
        None
    }
}

fn save_checkpoint(ckpt_dir: Option<&Path>, cp: &RunCheckpoint) -> Result<(), String> {
    let Some(dir) = ckpt_dir else {
        return Ok(());
    };
    std::fs::create_dir_all(dir).map_err(|e| format!("create checkpoint dir: {e}"))?;
    let path = dir.join(format!("seed_{}.json", cp.seed));
    let tmp = dir.join(format!("seed_{}.json.tmp", cp.seed));
    let json = serde_json::to_string_pretty(cp).map_err(|e| e.to_string())?;
    write_text(&tmp, &(json + "\n"))?;
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::rename(&tmp, &path).map_err(|e| format!("rename checkpoint: {e}"))?;
    Ok(())
}

fn run_confirm_mode(args: &Args, env: &RuntimeEnv) -> Result<(), String> {
    refuse_debug(env, Mode::Confirm)?;
    let prereg = load_prereg(&args.manifest_dir)?;
    let out_dir = args
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from(&prereg.confirmation_dir_default));
    ensure_out_inside_prereg_root(&out_dir, &prereg, env)?;
    ensure_empty_out_dir(&out_dir)?;
    let prerun_check = check_world_artifact_prerun(env)?;

    let smoke_dir = args
        .smoke_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(&prereg.smoke_dir_default));
    let smoke_ref = validate_smoke_dir_for_confirm(&smoke_dir)?;

    if let Some(ckpt) = &args.checkpoint_dir {
        let c_ckpt = abs_clean(ckpt);
        let c_out = abs_clean(&out_dir);
        if c_ckpt == c_out || c_ckpt.starts_with(&c_out) {
            return Err("refuse: --checkpoint-dir must not live inside --out".into());
        }
    }

    let registry = ObservableRegistry::live_default();
    let bundle = load_and_validate_all_manifests(&args.manifest_dir, &prereg, &registry)?;

    let started = SystemTime::now();
    let timer = Instant::now();
    let total_seeds = prereg.execution_order.len();
    let seed_slots: Vec<Mutex<Option<RunCheckpoint>>> =
        (0..total_seeds).map(|_| Mutex::new(None)).collect();
    let errors: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let next_idx = Mutex::new(0usize);
    let done_count = Mutex::new(0usize);

    let workers = args.jobs.min(total_seeds.max(1));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let idx = {
                    let mut guard = next_idx.lock().expect("seed index mutex poisoned");
                    if *guard >= total_seeds {
                        return;
                    }
                    let cur = *guard;
                    *guard += 1;
                    cur
                };
                let seed = prereg.execution_order[idx];
                let cp = if let Some(loaded) =
                    try_load_checkpoint(args.checkpoint_dir.as_deref(), seed)
                {
                    loaded
                } else {
                    let pair_clk = Instant::now();
                    let pair = match run_live_drift_pair(
                        &bundle.selected,
                        &bundle.scarce_drift,
                        &registry,
                        seed,
                    ) {
                        Ok(p) => p,
                        Err(e) => {
                            errors
                                .lock()
                                .unwrap()
                                .push(format!("seed {seed} drift pair failed: {e}"));
                            return;
                        }
                    };
                    let pair_elapsed = pair_clk.elapsed().as_secs_f64();

                    let mut0_clk = Instant::now();
                    let scarce_mut0 = run_manifest_seed::<LiveExperimentAdapter>(
                        &bundle.scarce_mut0,
                        &registry,
                        seed,
                        None,
                        None,
                    );
                    let mut0_elapsed = mut0_clk.elapsed().as_secs_f64();

                    let built = RunCheckpoint {
                        seed,
                        pair_elapsed_secs: pair_elapsed,
                        mut0_elapsed_secs: mut0_elapsed,
                        selected: pair.selected,
                        scarce_drift: pair.drift,
                        scarce_mut0,
                        drift_diagnostics: pair.diagnostics,
                        window_net_selection_deltas: pair.window_net_selection_deltas,
                    };
                    if let Err(e) = save_checkpoint(args.checkpoint_dir.as_deref(), &built) {
                        errors.lock().unwrap().push(e);
                        return;
                    }
                    built
                };

                // Blind progress output only: never inspect or print `trait_mass_*` here.
                let finished = {
                    let mut d = done_count.lock().expect("done mutex poisoned");
                    *d += 1;
                    *d
                };
                println!(
                    "SEL2-P2 progress [{finished}/{total_seeds}] seed={seed} shortfall={} pair_s={:.2} mut0_s={:.2}",
                    cp.drift_diagnostics.has_shortfall(),
                    cp.pair_elapsed_secs,
                    cp.mut0_elapsed_secs
                );

                *seed_slots[idx].lock().expect("slot lock") = Some(cp);
            });
        }
    });

    let err_list = errors.into_inner().expect("errors mutex");
    if !err_list.is_empty() {
        return Err(err_list.join("; "));
    }

    let checkpoints: Vec<RunCheckpoint> = seed_slots
        .into_iter()
        .map(|s| s.into_inner().expect("slot mutex").expect("completed seed"))
        .collect();
    let total_elapsed_secs = timer.elapsed().as_secs_f64();
    let ended = SystemTime::now();

    let mut runs = Vec::with_capacity(checkpoints.len() * ARM_ORDER.len());
    let mut seq = 0usize;
    for cp in checkpoints {
        let diag_shortfall = cp.drift_diagnostics.has_shortfall();
        for (arm, result, is_drift, win_deltas, elapsed) in [
            (
                "selected",
                cp.selected,
                false,
                Some(cp.window_net_selection_deltas),
                cp.pair_elapsed_secs * 0.5,
            ),
            (
                "scarce_mut0",
                cp.scarce_mut0,
                false,
                None,
                cp.mut0_elapsed_secs,
            ),
            (
                "scarce_drift",
                cp.scarce_drift,
                true,
                None,
                cp.pair_elapsed_secs * 0.5,
            ),
        ] {
            let terrain_source = result
                .provenance
                .terrain_source
                .clone()
                .ok_or_else(|| format!("seed {} {arm}: missing terrain_source", cp.seed))?;
            let mut wid = result
                .provenance
                .world_identity
                .ok_or_else(|| format!("seed {} {arm}: missing world_identity", cp.seed))?;
            if let Some(hook) = env.world_identity_override {
                wid = hook(seq, arm, cp.seed, wid);
            }
            seq += 1;
            let extinct = result.observable("live.agent_count") == Some(0.0)
                || result.series.iter().any(|s| {
                    s.observables
                        .iter()
                        .any(|(k, v)| k == "live.agent_count" && *v == 0.0)
                });
            let has_shortfall = is_drift && diag_shortfall;
            let drift_valid = if is_drift {
                result.status.is_completed() && !extinct && !has_shortfall
            } else {
                true
            };
            runs.push(ArmRunRecord {
                seed: cp.seed,
                arm: arm.to_string(),
                result,
                drift_valid,
                has_shortfall,
                terrain_source,
                world_identity: wid,
                elapsed_secs: elapsed,
                window_net_selection_deltas: win_deltas,
            });
        }
    }

    let expected_wid = runs.first().ok_or("no confirmation runs")?.world_identity;
    if let Some(smoke_wid) = smoke_ref.world_identity {
        if smoke_wid != expected_wid {
            return Err(format!(
                "world_identity mismatch vs smoke: {smoke_wid:?} vs {expected_wid:?}"
            ));
        }
    }
    for r in &runs {
        if r.terrain_source != "generated" || r.world_identity != expected_wid {
            return Err(format!(
                "terrain/world_identity mismatch at seed={} arm={}",
                r.seed, r.arm
            ));
        }
    }

    write_confirmation_bundle(ConfirmWriteCtx {
        out_dir: &out_dir,
        manifest_dir: &args.manifest_dir,
        prereg: &prereg,
        bundle: &bundle,
        registry: &registry,
        env,
        prerun_check: &prerun_check,
        smoke_ref: &smoke_ref,
        runs: &runs,
        expected_wid,
        started,
        ended,
        total_elapsed_secs,
    })?;

    println!(
        "SEL2-P2 confirmation completed: {} runs in {:.2}s -> {}",
        runs.len(),
        total_elapsed_secs,
        out_dir.display()
    );
    Ok(())
}

fn run_all_mode(args: &Args, env: &RuntimeEnv) -> Result<(), String> {
    let prereg = load_prereg(&args.manifest_dir)?;
    let (smoke_dir, confirm_dir) = if let Some(base) = &args.out {
        (base.join("smoke"), base.join("confirmation"))
    } else {
        (
            PathBuf::from(&prereg.smoke_dir_default),
            PathBuf::from(&prereg.confirmation_dir_default),
        )
    };
    let smoke_args = Args {
        mode: Mode::Smoke,
        manifest_dir: args.manifest_dir.clone(),
        out: Some(smoke_dir.clone()),
        smoke_dir: None,
        checkpoint_dir: None,
        jobs: args.jobs,
    };
    run_smoke_mode(&smoke_args, env)?;

    let confirm_args = Args {
        mode: Mode::Confirm,
        manifest_dir: args.manifest_dir.clone(),
        out: Some(confirm_dir),
        smoke_dir: Some(smoke_dir),
        checkpoint_dir: args.checkpoint_dir.clone(),
        jobs: args.jobs,
    };
    run_confirm_mode(&confirm_args, env)
}

// ---- Confirmation Analysis & Bundle Writers ----------------------------------------------------

struct RunRuleSummary {
    seed: u64,
    arm: String,
    completed: bool,
    extinct: bool,
    first_extinct_tick: Option<u64>,
    survival_ticks: u64,
    valid_mass: bool,
    mean_int: Option<f64>,
    mass_tail20_mean: Option<f64>,
    energy_post_mean: Option<f64>,
    var_final: f64,
    final_agents: f64,
    closed_eu_total: f64,
    births_sum: f64,
    deaths_sum: f64,
    lifespan_at_t: Option<f64>,
    cumulative_mean_lifespan: f64,
    drift_valid: bool,
    has_shortfall: bool,
    window_net_selection_deltas: Option<BTreeMap<String, f64>>,
}

fn summarize_run_rules(rec: &ArmRunRecord, prereg: &PreregSpec) -> RunRuleSummary {
    let obs_val = |sample: &[(String, f64)], key: &str| -> Option<f64> {
        sample.iter().find(|(k, _)| k == key).map(|(_, v)| *v)
    };
    let mut ticks: Vec<_> = rec.result.series.iter().collect();
    ticks.sort_by_key(|s| s.tick);

    let mut first_extinct_tick = None;
    for s in &ticks {
        if obs_val(&s.observables, "live.agent_count") == Some(0.0) {
            first_extinct_tick = Some(s.tick);
            break;
        }
    }
    let final_agents = rec.result.observable("live.agent_count").unwrap_or(0.0);
    if first_extinct_tick.is_none() && final_agents == 0.0 {
        first_extinct_tick = Some(prereg.duration_ticks);
    }
    let extinct = first_extinct_tick.is_some();
    let survival_ticks = first_extinct_tick.unwrap_or(prereg.duration_ticks);

    let post_warmup: Vec<f64> = ticks
        .iter()
        .filter(|s| s.tick > prereg.warmup_ticks && s.tick <= prereg.duration_ticks)
        .filter_map(|s| {
            let n = obs_val(&s.observables, "live.agent_count")?;
            let m = obs_val(&s.observables, "live.trait_mass_mean")?;
            if n >= 1.0 && m.is_finite() {
                Some(m)
            } else {
                None
            }
        })
        .collect();
    let post_warmup_expected = ticks
        .iter()
        .filter(|s| s.tick > prereg.warmup_ticks && s.tick <= prereg.duration_ticks)
        .count();
    let completed = rec.result.status.is_completed();
    let valid_post_warmup_mass = !extinct
        && completed
        && post_warmup_expected > 0
        && post_warmup.len() == post_warmup_expected;
    let mean_int = if valid_post_warmup_mass {
        mean_slice(&post_warmup)
    } else {
        None
    };

    let tail_vals: Vec<f64> = ticks
        .iter()
        .filter(|s| s.tick > prereg.tail20_threshold_tick)
        .filter_map(|s| {
            let n = obs_val(&s.observables, "live.agent_count")?;
            let m = obs_val(&s.observables, "live.trait_mass_mean")?;
            if n >= 1.0 && m.is_finite() {
                Some(m)
            } else {
                None
            }
        })
        .collect();
    let mass_tail20_mean = if !extinct && !tail_vals.is_empty() {
        mean_slice(&tail_vals)
    } else {
        None
    };

    let final_tick_sample = ticks.iter().find(|s| s.tick == prereg.duration_ticks);
    let warm_tick_sample = ticks.iter().find(|s| s.tick == prereg.warmup_ticks);
    let lifespan_at_t =
        final_tick_sample.and_then(|s| obs_val(&s.observables, "live.mean_lifespan"));
    let births_sum: f64 = ticks
        .iter()
        .map(|s| obs_val(&s.observables, "live.births").unwrap_or(0.0))
        .sum();
    let deaths_sum: f64 = ticks
        .iter()
        .map(|s| obs_val(&s.observables, "live.deaths").unwrap_or(0.0))
        .sum();

    let energy_post_mean = match (
        final_tick_sample.and_then(|s| obs_val(&s.observables, "live.mean_energy_integral")),
        warm_tick_sample.and_then(|s| obs_val(&s.observables, "live.mean_energy_integral")),
    ) {
        (Some(et), Some(ew))
            if et.is_finite() && ew.is_finite() && prereg.duration_ticks > prereg.warmup_ticks =>
        {
            Some((et - ew) / (prereg.duration_ticks - prereg.warmup_ticks) as f64)
        }
        _ => None,
    };

    let valid_mass = valid_post_warmup_mass && mean_int.is_some_and(|v| v.is_finite() && v > 0.0);

    RunRuleSummary {
        seed: rec.seed,
        arm: rec.arm.clone(),
        completed,
        extinct,
        first_extinct_tick,
        survival_ticks,
        valid_mass,
        mean_int,
        mass_tail20_mean,
        energy_post_mean,
        var_final: rec.result.observable("live.trait_mass_var").unwrap_or(0.0),
        final_agents,
        closed_eu_total: rec.result.observable("live.closed_eu_total").unwrap_or(0.0),
        births_sum,
        deaths_sum,
        lifespan_at_t,
        cumulative_mean_lifespan: rec
            .result
            .observable("live.cumulative_mean_lifespan")
            .unwrap_or(0.0),
        drift_valid: rec.drift_valid,
        has_shortfall: rec.has_shortfall,
        window_net_selection_deltas: rec.window_net_selection_deltas.clone(),
    }
}

struct ContrastDef {
    id: &'static str,
    name: &'static str,
    control_arm: &'static str,
    treatment_arm: &'static str,
}

const CONTRAST_DEFS: [ContrastDef; 3] = [
    ContrastDef {
        id: "H1",
        name: "selected / scarce_drift",
        control_arm: "scarce_drift",
        treatment_arm: "selected",
    },
    ContrastDef {
        id: "H2",
        name: "selected / scarce_mut0",
        control_arm: "scarce_mut0",
        treatment_arm: "selected",
    },
    ContrastDef {
        id: "H3",
        name: "scarce_drift / scarce_mut0",
        control_arm: "scarce_mut0",
        treatment_arm: "scarce_drift",
    },
];

fn build_effects_json(runs: &[ArmRunRecord], prereg: &PreregSpec) -> Result<String, String> {
    let summaries: BTreeMap<(u64, String), RunRuleSummary> = runs
        .iter()
        .map(|r| ((r.seed, r.arm.clone()), summarize_run_rules(r, prereg)))
        .collect();
    let failed_runs_count = runs
        .iter()
        .filter(|r| !r.result.status.is_completed())
        .count();
    let failed_runs_zero =
        failed_runs_count == 0 && runs.len() == prereg.execution_order.len() * ARM_ORDER.len();
    let n_req = prereg.execution_order.len();
    let th = &prereg.thresholds;

    struct RawContrast {
        def: &'static ContrastDef,
        primary_mode: &'static str,
        raw_primary_p: Option<f64>,
        p_ext_a: f64,
        p_ext_b: f64,
        drift_shortfall_ok: bool,
        complete_pairs: PairedLogMeanRatioReport,
        lachin: LachinReport,
        lee: LeeBoundsReport,
        rmst_stats: PairedStatsReport,
        rmst_wilcox: WilcoxonReport,
        tail20: Option<PairedLogMeanRatioReport>,
        energy_integral: PairedStatsReport,
        s1_var_final: PairedStatsReport,
        s3_births: PairedStatsReport,
        s3_deaths: PairedStatsReport,
        s3_cum_lifespan: PairedStatsReport,
    }

    let mut raw = Vec::with_capacity(CONTRAST_DEFS.len());
    for c in &CONTRAST_DEFS {
        let mut mean_a = Vec::new();
        let mut mean_b = Vec::new();
        let mut obs_a = Vec::new();
        let mut obs_b = Vec::new();
        let mut surv_valid_a = Vec::new();
        let mut surv_valid_b = Vec::new();
        let mut surv_all_a = Vec::new();
        let mut surv_all_b = Vec::new();
        let mut tail_a = Vec::new();
        let mut tail_b = Vec::new();
        let mut energy_a = Vec::new();
        let mut energy_b = Vec::new();
        let mut var_a = Vec::new();
        let mut var_b = Vec::new();
        let mut births_a = Vec::new();
        let mut births_b = Vec::new();
        let mut deaths_a = Vec::new();
        let mut deaths_b = Vec::new();
        let mut cum_ls_a = Vec::new();
        let mut cum_ls_b = Vec::new();
        let mut extinct_a = 0usize;
        let mut extinct_b = 0usize;
        let mut drift_shortfall_ok = true;

        for &seed in &prereg.execution_order {
            let Some(sa) = summaries.get(&(seed, c.control_arm.to_string())) else {
                continue;
            };
            let Some(sb) = summaries.get(&(seed, c.treatment_arm.to_string())) else {
                continue;
            };
            if sa.extinct {
                extinct_a += 1;
            }
            if sb.extinct {
                extinct_b += 1;
            }
            if !sa.drift_valid || sa.has_shortfall || !sb.drift_valid || sb.has_shortfall {
                drift_shortfall_ok = false;
            }
            obs_a.push(LachinObs {
                valid_mass: sa.valid_mass,
                survival_ticks: sa.survival_ticks,
                mean_int: sa.mean_int,
            });
            obs_b.push(LachinObs {
                valid_mass: sb.valid_mass,
                survival_ticks: sb.survival_ticks,
                mean_int: sb.mean_int,
            });
            surv_all_a.push(sa.survival_ticks as f64);
            surv_all_b.push(sb.survival_ticks as f64);
            if sa.valid_mass {
                if let Some(va) = sa.mean_int {
                    surv_valid_a.push(va);
                }
            }
            if sb.valid_mass {
                if let Some(vb) = sb.mean_int {
                    surv_valid_b.push(vb);
                }
            }
            if sa.valid_mass && sb.valid_mass {
                if let (Some(va), Some(vb)) = (sa.mean_int, sb.mean_int) {
                    mean_a.push(va);
                    mean_b.push(vb);
                }
                var_a.push(sa.var_final);
                var_b.push(sb.var_final);
                births_a.push(sa.births_sum);
                births_b.push(sb.births_sum);
                deaths_a.push(sa.deaths_sum);
                deaths_b.push(sb.deaths_sum);
                cum_ls_a.push(sa.cumulative_mean_lifespan);
                cum_ls_b.push(sb.cumulative_mean_lifespan);
            }
            if let (Some(ta), Some(tb)) = (sa.mass_tail20_mean, sb.mass_tail20_mean) {
                tail_a.push(ta);
                tail_b.push(tb);
            }
            if let (Some(ea), Some(eb)) = (sa.energy_post_mean, sb.energy_post_mean) {
                energy_a.push(ea);
                energy_b.push(eb);
            }
        }

        let p_ext_a = if n_req > 0 {
            extinct_a as f64 / n_req as f64
        } else {
            1.0
        };
        let p_ext_b = if n_req > 0 {
            extinct_b as f64 / n_req as f64
        } else {
            1.0
        };
        let complete_pairs = evaluate_paired_log_mean_ratio(
            &mean_a,
            &mean_b,
            th.sesoi_log_mean_ratio,
            th.min_non_zero_pairs_r7,
            n_req,
            th.bca_resamples,
        )?;
        let lachin = lachin_worst_rank_paired(&obs_a, &obs_b, th.min_non_zero_pairs_r7)?;
        let lee = lee_trimming_bounds_log_mean(&surv_valid_a, &surv_valid_b);
        let surv_diffs: Vec<f64> = surv_all_a
            .iter()
            .zip(&surv_all_b)
            .map(|(a, b)| b - a)
            .collect();
        let rmst_stats = paired_stats(&surv_all_a, &surv_all_b, n_req);
        let rmst_wilcox = wilcoxon_signed_rank(&surv_diffs, th.min_non_zero_pairs_r7);
        let tail20 = if !tail_a.is_empty() && tail_a.len() == tail_b.len() {
            Some(evaluate_paired_log_mean_ratio(
                &tail_a,
                &tail_b,
                th.sesoi_log_mean_ratio,
                th.min_non_zero_pairs_r7,
                n_req,
                th.bca_resamples,
            )?)
        } else {
            None
        };
        let energy_integral = paired_stats(&energy_a, &energy_b, n_req);

        let complete_pair_eligible = p_ext_a <= th.max_ext_rate
            && p_ext_b <= th.max_ext_rate
            && complete_pairs.pi_ll >= th.min_pi_ll
            && complete_pairs.n_complete_pairs >= th.min_non_zero_pairs_r7;
        let primary_mode = if complete_pair_eligible {
            "complete_pairs"
        } else {
            "lachin_worst_rank"
        };
        let raw_primary_p = if primary_mode == "complete_pairs" {
            complete_pairs.p_two_sided
        } else {
            Some(lachin.p_two_sided)
        };

        raw.push(RawContrast {
            def: c,
            primary_mode,
            raw_primary_p,
            p_ext_a,
            p_ext_b,
            drift_shortfall_ok,
            complete_pairs,
            lachin,
            lee,
            rmst_stats,
            rmst_wilcox,
            tail20,
            energy_integral,
            s1_var_final: paired_stats(&var_a, &var_b, n_req),
            s3_births: paired_stats(&births_a, &births_b, n_req),
            s3_deaths: paired_stats(&deaths_a, &deaths_b, n_req),
            s3_cum_lifespan: paired_stats(&cum_ls_a, &cum_ls_b, n_req),
        });
    }

    let p_vec: Vec<Option<f64>> = raw.iter().map(|e| e.raw_primary_p).collect();
    let holm_p = holm_adjust(&p_vec);

    let mut contrasts_map = serde_json::Map::new();
    let mut h1_decision = "insufficient_evidence";
    let mut h1_dir_tag = "attrition_below_floor_or_drift_invalid";
    let mut h1_mode = "complete_pairs";

    for (idx, e) in raw.into_iter().enumerate() {
        let p_holm = holm_p[idx];
        let dec = classify_sel2p2_decision(
            true,
            failed_runs_zero,
            e.drift_shortfall_ok,
            e.primary_mode,
            e.complete_pairs.n_complete_pairs,
            n_req,
            th.min_pi_ll,
            th.min_non_zero_pairs_r7,
            p_holm,
            th.fwer_alpha,
            e.complete_pairs.mean_d,
            e.complete_pairs.tost_equivalent,
            th.sesoi_log_mean_ratio,
            e.lachin.mean_rank_diff,
            e.lee.lee_lo,
            e.lee.lee_hi,
        );
        if e.def.id == "H1" {
            h1_decision = dec.label;
            h1_dir_tag = dec.direction_tag;
            h1_mode = e.primary_mode;
        }
        let mut rmst_val = serde_json::to_value(&e.rmst_stats).map_err(|err| err.to_string())?;
        if let Some(obj) = rmst_val.as_object_mut() {
            obj.insert(
                "wilcoxon".to_string(),
                serde_json::to_value(&e.rmst_wilcox).map_err(|err| err.to_string())?,
            );
        }
        contrasts_map.insert(
            e.def.id.to_string(),
            serde_json::json!({
                "id": e.def.id,
                "name": e.def.name,
                "primary_mode": e.primary_mode,
                "p_ext_control": e.p_ext_a,
                "p_ext_treatment": e.p_ext_b,
                "drift_and_shortfall_valid": e.drift_shortfall_ok,
                "p_holm": p_holm,
                "decision": dec.label,
                "direction_tag": dec.direction_tag,
                "complete_pairs": e.complete_pairs,
                "lachin_worst_rank": e.lachin,
                "lee_bounds": e.lee,
                "rmst": rmst_val,
                "tail20": e.tail20,
                "energy_integral": e.energy_integral,
                "s1_trait_mass_var": e.s1_var_final,
                "s3_births": e.s3_births,
                "s3_deaths": e.s3_deaths,
                "s3_cumulative_mean_lifespan": e.s3_cum_lifespan,
            }),
        );
    }

    let arm_diag: Vec<serde_json::Value> = summaries
        .values()
        .map(|s| {
            serde_json::json!({
                "seed": s.seed,
                "arm": s.arm,
                "completed": s.completed,
                "extinct": s.extinct,
                "first_extinct_tick": s.first_extinct_tick,
                "survival_ticks": s.survival_ticks,
                "valid_mass": s.valid_mass,
                "mean_int": s.mean_int,
                "mass_tail20_mean": s.mass_tail20_mean,
                "energy_post_mean": s.energy_post_mean,
                "var_final": s.var_final,
                "final_agents": s.final_agents,
                "closed_eu_total": s.closed_eu_total,
                "births_sum": s.births_sum,
                "deaths_sum": s.deaths_sum,
                "lifespan_at_t": s.lifespan_at_t,
                "cumulative_mean_lifespan": s.cumulative_mean_lifespan,
                "drift_valid": s.drift_valid,
                "has_shortfall": s.has_shortfall,
                "window_net_selection_deltas": s.window_net_selection_deltas,
            })
        })
        .collect();

    let doc = serde_json::json!({
        "kind": "sel2p2-effects-and-decision",
        "preregistration_id": prereg.prereg_id,
        "s3_lifespan_source": "series.csv",
        "primary_decision": h1_decision,
        "primary_direction_tag": h1_dir_tag,
        "primary_mode": h1_mode,
        "failed_runs_count": failed_runs_count,
        "min_valid_pairs_for_decision": th.min_valid_pairs,
        "rmst_tau_ticks": th.rmst_tau_ticks,
        "max_abs_eu_drift_threshold": th.max_abs_eu_drift,
        "contrasts": contrasts_map,
        "arm_diagnostics": arm_diag,
    });
    Ok(pretty(&doc)? + "\n")
}

struct ConfirmWriteCtx<'a> {
    out_dir: &'a Path,
    manifest_dir: &'a Path,
    prereg: &'a PreregSpec,
    bundle: &'a ManifestBundle,
    registry: &'a ObservableRegistry,
    env: &'a RuntimeEnv,
    prerun_check: &'a serde_json::Value,
    smoke_ref: &'a SmokeVerification,
    runs: &'a [ArmRunRecord],
    expected_wid: WorldIdentity,
    started: SystemTime,
    ended: SystemTime,
    total_elapsed_secs: f64,
}

fn write_confirmation_bundle(w: ConfirmWriteCtx<'_>) -> Result<(), String> {
    let manifests_dir = w.out_dir.join("manifests");
    std::fs::create_dir_all(&manifests_dir)
        .map_err(|e| format!("create {}: {e}", manifests_dir.display()))?;

    let mut manifest_hashes: BTreeMap<String, String> = BTreeMap::new();
    let manifest_files = [
        PREREG_FILE,
        w.prereg.selected_file.as_str(),
        w.prereg.scarce_mut0_file.as_str(),
        w.prereg.scarce_drift_file.as_str(),
    ];
    for name in manifest_files {
        let src = w.manifest_dir.join(name);
        let raw = std::fs::read_to_string(&src).map_err(|e| format!("read {name}: {e}"))?;
        let dst = manifests_dir.join(name);
        write_text(&dst, &raw)?;
        manifest_hashes.insert(
            name.to_string(),
            sha256_file(&dst).map_err(|e| e.to_string())?,
        );
    }

    write_text(&w.out_dir.join("runs.csv"), &build_runs_csv(w.runs))?;
    write_text(
        &w.out_dir.join("series.csv"),
        &build_confirm_series_csv(w.runs),
    )?;
    write_text(
        &w.out_dir.join("final-observables.json"),
        &build_final_observables_json(w.runs)?,
    )?;
    write_text(
        &w.out_dir.join("effects.json"),
        &build_effects_json(w.runs, w.prereg)?,
    )?;

    let arm_fps = BTreeMap::from([
        ("selected".to_string(), w.bundle.selected.fingerprint()),
        (
            "scarce_mut0".to_string(),
            w.bundle.scarce_mut0.fingerprint(),
        ),
        (
            "scarce_drift".to_string(),
            w.bundle.scarce_drift.fingerprint(),
        ),
    ]);
    let git_commit = resolve_git_commit(w.env);
    let git_dirty_paths = resolve_git_dirty_paths(w.env);
    let prov = serde_json::json!({
        "kind": "sel2p2-confirmation-provenance",
        "mode": "confirm",
        "stopped_early": false,
        "stop_reason": serde_json::Value::Null,
        "preregistration_id": w.prereg.prereg_id,
        "preregistration_status": w.prereg.status,
        "model": "core::live_experiment::LiveExperimentAdapter",
        "model_version": LIVE_MODEL_VERSION,
        "registry": "core::experiment::ObservableRegistry::live_default",
        "registry_fingerprint": w.registry.fingerprint(),
        "terrain_source": "generated",
        "world_identity": w.expected_wid,
        "world_identity_all_runs_identical": true,
        "world_artifact_prerun_check": w.prerun_check,
        "smoke_reference": {
            "smoke_dir": w.smoke_ref.dir_display,
            "provenance_sha256": w.smoke_ref.provenance_sha256,
            "smoke_report_sha256": w.smoke_ref.smoke_report_sha256,
            "git_commit": w.smoke_ref.git_commit,
            "host_os": w.smoke_ref.host_os,
            "host_arch": w.smoke_ref.host_arch,
            "profile": w.smoke_ref.profile,
            "world_identity_observed": w.smoke_ref.world_identity,
            "gate_pass": w.smoke_ref.gate_pass,
        },
        "build": {
            "git_commit": git_commit,
            "git_describe": git(&["describe", "--always", "--dirty"]),
            "git_dirty_paths": git_dirty_paths,
            "git_tree_dirty": !git_dirty_paths.is_empty(),
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
            "sha256_as_committed": manifest_hashes,
            "manifest_fingerprints_as_run": arm_fps,
        },
        "seeds": {
            "execution_order": w.prereg.execution_order,
            "smoke_seed": w.prereg.smoke_seed,
            "n_requested": w.prereg.execution_order.len(),
            "total_runs_executed": w.runs.len(),
        },
        "timing": {
            "started_utc": iso8601_utc(unix_seconds(w.started)),
            "ended_utc": iso8601_utc(unix_seconds(w.ended)),
            "wall_clock_seconds": w.total_elapsed_secs,
        },
    });
    write_text(&w.out_dir.join("provenance.json"), &(pretty(&prov)? + "\n"))?;

    let mut hashed = vec![
        "effects.json".to_string(),
        "final-observables.json".to_string(),
        "provenance.json".to_string(),
        "runs.csv".to_string(),
        "series.csv".to_string(),
    ];
    for name in manifest_files {
        hashed.push(format!("manifests/{name}"));
    }
    let refs: Vec<&str> = hashed.iter().map(String::as_str).collect();
    write_checksums(w.out_dir, &refs)
}

fn build_runs_csv(runs: &[ArmRunRecord]) -> String {
    let mut s = String::from(
        "seed,arm,status,drift_valid,has_shortfall,terrain_source,world_seed,world_gen_version,world_width,world_height,world_checksum,final_checksum,elapsed_secs\n",
    );
    for rec in runs {
        s.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{:.4}\n",
            rec.seed,
            rec.arm,
            status_label(&rec.result.status),
            rec.drift_valid,
            rec.has_shortfall,
            rec.terrain_source,
            rec.world_identity.seed,
            rec.world_identity.generator_version,
            rec.world_identity.width,
            rec.world_identity.height,
            rec.world_identity.checksum,
            rec.result.final_checksum,
            rec.elapsed_secs,
        ));
    }
    s
}

fn build_confirm_series_csv(runs: &[ArmRunRecord]) -> String {
    let mut s = String::from("seed,arm,tick");
    for col in CONFIRM_SERIES_OBSERVABLES {
        s.push(',');
        s.push_str(col);
    }
    s.push('\n');
    for rec in runs {
        for sample in &rec.result.series {
            s.push_str(&format!("{},{},{}", rec.seed, rec.arm, sample.tick));
            for col in CONFIRM_SERIES_OBSERVABLES {
                let val = sample
                    .observables
                    .iter()
                    .find(|(k, _)| k == col)
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_default();
                s.push(',');
                s.push_str(&val);
            }
            s.push('\n');
        }
    }
    s
}

fn build_final_observables_json(runs: &[ArmRunRecord]) -> Result<String, String> {
    let arr: Vec<serde_json::Value> = runs
        .iter()
        .map(|rec| {
            let obs: BTreeMap<String, f64> = rec.result.final_observables.iter().cloned().collect();
            serde_json::json!({
                "seed": rec.seed,
                "arm": rec.arm,
                "status": status_label(&rec.result.status),
                "final_checksum": rec.result.final_checksum,
                "terrain_source": rec.terrain_source,
                "world_identity": rec.world_identity,
                "observables": obs,
            })
        })
        .collect();
    Ok(pretty(&serde_json::Value::Array(arr))? + "\n")
}

// ---- Low-level Helpers -------------------------------------------------------------------------

fn assert_dir_has_no_body_mass(dir: &Path) -> Result<(), String> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(cur) = stack.pop() {
        let entries =
            std::fs::read_dir(&cur).map_err(|e| format!("read {}: {e}", cur.display()))?;
        for ent in entries {
            let path = ent.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file() {
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| format!("scan {}: {e}", path.display()))?;
                if text.to_ascii_lowercase().contains("trait_mass") {
                    return Err(format!(
                        "blinding check failed: forbidden body-mass substring found in {}",
                        path.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

fn verify_checksum_manifest(dir: &Path, required: &[&str]) -> Result<(), String> {
    let path = dir.join("checksums.sha256");
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("read checksums: {e}"))?;
    let mut covered = BTreeSet::new();
    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
        let Some((hex, rel)) = line.split_once("  ") else {
            return Err(format!("malformed checksums line: '{line}'"));
        };
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("malformed hex digest in checksums: '{line}'"));
        }
        let target = dir.join(rel);
        let actual = sha256_file(&target).map_err(|e| format!("hash {rel}: {e}"))?;
        if actual != hex {
            return Err(format!("checksum mismatch for '{rel}'"));
        }
        covered.insert(rel.to_string());
    }
    for req in required {
        if !covered.contains(*req) {
            return Err(format!("checksums.sha256 omits '{req}'"));
        }
    }
    Ok(())
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

fn abs_clean(p: &Path) -> PathBuf {
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    };
    let mut out = PathBuf::new();
    for comp in joined.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

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

fn status_label(status: &RunStatus) -> &'static str {
    match status {
        RunStatus::Completed => "Completed",
        RunStatus::Failed { .. } => "Failed",
    }
}

fn opt_u64(v: Option<u64>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

fn opt_f64(v: Option<f64>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
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

fn os_string_lossy(s: OsString) -> String {
    s.to_string_lossy().into_owned()
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
