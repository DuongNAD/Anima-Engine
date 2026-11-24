//! Experiment E3b runner (`CARD C1. E3B-RUNNER`): productivity gradient vs matched uniform
//! scarcity (`founder_genotype = 1.0`, `predator_fraction = 0.3`, `N = 64`).
//!
//! Follows the frozen E3b preregistration verbatim:
//! - `src-tauri/tests/fixtures/experiments_e3b/e3b-preregistration.json`
//! - `docs/ai/planning/2026-11-24-e3b-preregistration.md`
//! - `scripts/verify_e3b_artifacts.mjs`
//!
//! # Build first in `--release`, then execute the binary -- never `cargo run`
//!
//! ```text
//! cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop --example run_e3b_experiment
//! src-tauri/target/release/examples/run_e3b_experiment --smoke --out artifacts/experiments/e3b-productivity-gradient-body-mass/smoke
//! src-tauri/target/release/examples/run_e3b_experiment --ensemble \
//!   --smoke-dir artifacts/experiments/e3b-productivity-gradient-body-mass/smoke \
//!   --out artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation \
//!   --jobs 8 --checkpoint-dir <temp_dir>/e3b_ckpt
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::experiment::{
    ExperimentManifest, FactorDiff, ObservableRegistry, MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{
    run_manifest_seed, run_paired_ensemble_with_control, ExperimentModel, RunResult, RunStatus,
};
use anima_engine_lib::core::live_experiment::{LiveExperimentAdapter, LIVE_MODEL_VERSION};
use anima_engine_lib::core::resources::SimRng;
use anima_engine_lib::core::sha256::sha256_file;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use rand::RngCore;
use serde::{Deserialize, Serialize};

const DEFAULT_MANIFEST_DIR: &str = "src-tauri/tests/fixtures/experiments_e3b";
const DEFAULT_SMOKE_OUT: &str = "artifacts/experiments/e3b-productivity-gradient-body-mass/smoke";
const DEFAULT_CONFIRM_OUT: &str =
    "artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation";
const PREREG_FILE: &str = "e3b-preregistration.json";
const TEMP_ARTIFACT_NAME: &str = "anima_world_artifact.anmw";
const ALLOWED_TREATMENT_DIFF: [&str; 1] = ["interventions"];
const ALLOWED_SCARCITY_DIFF: [&str; 2] = ["initial_conditions", "interventions"];
const ARM_ORDER: [&str; 3] = [
    "control_uniform",
    "control_uniform_scarcity",
    "treatment_gradient",
];
const DEFOREST_CAUSE_ID: u32 = 1;
const ADD_NUTRIENT_CAUSE_ID: u32 = 2;
const WP2_B1_MIN_REL_SHIFT: f64 = 0.10;
const WP2_B2_MAX_REL_MISMATCH: f64 = 0.10;

const CONFIRM_SERIES_OBSERVABLES: [&str; 12] = [
    "detritus",
    "live.agent_count",
    "live.predator_count",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
    "live.standing_crop",
    "live.closed_eu_total",
    "live.trait_mass_var",
    "live.trait_mass_mean",
    "live.shannon",
    "live.archive_coverage",
];

const SMOKE_SERIES_OBSERVABLES: [&str; 22] = [
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
    "live.shannon",
    "live.simpson",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
    "live.archive_coverage",
    "live.deaths_starvation",
    "live.deaths_dehydration",
    "live.mean_energy_integral",
    "live.cumulative_mean_lifespan",
];

pub type WorldIdentityHook = fn(usize, &str, u64, WorldIdentity) -> WorldIdentity;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Help,
    Smoke,
    Ensemble,
    Replay,
    SelfTest,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Mode::Help => "help",
            Mode::Smoke => "smoke",
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
    out: Option<PathBuf>,
    smoke_dir: Option<PathBuf>,
    checkpoint_dir: Option<PathBuf>,
    jobs: usize,
    arm: Option<String>,
    seed: Option<u64>,
}

#[derive(Clone)]
pub struct ThresholdsSpec {
    pub eps_primary: f64,
    pub eps_sensitivity: f64,
    pub sesoi_log_var_ratio: f64,
    pub fwer_alpha: f64,
    pub min_pi_ll: f64,
    pub min_valid_pairs: usize,
    pub min_non_zero_pairs_r7: usize,
    pub max_abs_eu_drift: f64,
}

pub struct PreregSpec {
    raw: serde_json::Value,
    status: String,
    control_file: String,
    scarcity_file: String,
    treatment_file: String,
    smoke_control_file: String,
    smoke_scarcity_file: String,
    smoke_treatment_file: String,
    pub execution_order: Vec<u64>,
    pub smoke_seed: u64,
    duration_ticks: u64,
    sample_period: u64,
    warmup_ticks: u64,
    tail_window: [u64; 2],
    thresholds: ThresholdsSpec,
}

#[derive(Clone, Serialize, Deserialize)]
struct RunCheckpoint {
    seed: u64,
    arm: String,
    manifest_fingerprint: u64,
    terrain_source: String,
    world_identity: WorldIdentity,
    elapsed_secs: f64,
    result: RunResult,
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

#[derive(Clone)]
struct SmokeVerification {
    dir: PathBuf,
    provenance_sha256: String,
    smoke_report_sha256: String,
    git_commit: Option<String>,
    host_os: Option<String>,
    host_arch: Option<String>,
    profile: Option<String>,
    world_identity: Option<WorldIdentity>,
    wp2_passed: bool,
}

struct FieldProbe {
    world_identity: WorldIdentity,
    rng_seed: u64,
    rng_stream_pos: u128,
    rng_next_draws: Vec<u64>,
    resource_field_present: bool,
    width: usize,
    height: usize,
    initial_total_biomass: f64,
    initial_total_capacity: f64,
    ecosystem_plants: f64,
    ecosystem_detritus: f64,
    ecosystem_animals: f64,
    ecosystem_dormant: f64,
}

impl FieldProbe {
    fn total_ecosystem_eu(&self) -> f64 {
        self.ecosystem_plants
            + self.ecosystem_detritus
            + self.ecosystem_animals
            + self.ecosystem_dormant
    }
}

struct SmokeArmSummary {
    arm: &'static str,
    status: &'static str,
    final_checksum: u32,
    terrain_source: String,
    world_identity: WorldIdentity,
    standing_crop_final: f64,
    standing_crop_post_warmup_mean: f64,
    detritus_final: f64,
    first_positive_detritus_before_warmup: Option<(u64, f64)>,
    detritus_at_warmup: Option<f64>,
    max_abs_eu_drift: f64,
    agent_count_final: f64,
    predator_count_final: f64,
    emitted_count: usize,
    all_finite: bool,
    deforest_ledger_count: usize,
    add_nutrient_ledger_count: usize,
}

// ---- Exact E3b statistical engine (mirrors `scripts/verify_e3b_artifacts.mjs`) -----------------

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
    let mut lo = 0.0f64;
    let mut hi = 1.0f64;
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
    let r_c = if total_rank > 0.0 {
        Some((w_plus - w_minus) / total_rank)
    } else {
        Some(0.0)
    };

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
pub struct PairedLogRatioReport {
    pub n_requested: usize,
    pub n_complete_pairs: usize,
    pub pi_ll: f64,
    pub eps: f64,
    #[serde(rename = "mean_L")]
    pub mean_l: Option<f64>,
    pub geom_var_ratio: Option<f64>,
    #[serde(rename = "sd_L")]
    pub sd_l: Option<f64>,
    #[serde(rename = "se_L")]
    pub se_l: Option<f64>,
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
    pub concordance_pos: Option<f64>,
    pub concordance_neg: Option<f64>,
    #[serde(rename = "median_L")]
    pub median_l: Option<f64>,
    pub hl_pseudomedian: Option<f64>,
    pub wilcoxon: WilcoxonReport,
    pub diffs: Vec<f64>,
}

pub fn evaluate_paired_log_variance_ratio(
    var_a: &[f64],
    var_b: &[f64],
    eps: f64,
    sesoi: f64,
    min_non_zero_r7: usize,
    n_requested: usize,
) -> PairedLogRatioReport {
    assert_eq!(var_a.len(), var_b.len());
    let n = var_a.len();
    let diffs: Vec<f64> = var_a
        .iter()
        .zip(var_b)
        .map(|(a, b)| ((b + eps) / (a + eps)).ln())
        .collect();
    let wilcoxon = wilcoxon_signed_rank(&diffs, min_non_zero_r7);
    let mut out = PairedLogRatioReport {
        n_requested,
        n_complete_pairs: n,
        pi_ll: if n_requested > 0 {
            n as f64 / n_requested as f64
        } else {
            0.0
        },
        eps,
        mean_l: None,
        geom_var_ratio: None,
        sd_l: None,
        se_l: None,
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
        median_l: median_slice(&diffs),
        hl_pseudomedian: hodges_lehmann_pseudomedian(&diffs),
        wilcoxon,
        diffs: diffs.clone(),
    };
    let Some(m) = mean_slice(&diffs) else {
        return out;
    };
    out.mean_l = Some(m);
    out.geom_var_ratio = Some(m.exp());
    if n < 2 {
        return out;
    }
    let sd = sample_sd(&diffs).unwrap_or(0.0);
    let zero_tol = 1e-12 * m.abs().max(1.0);
    if sd < zero_tol {
        out.sd_l = Some(0.0);
        out.se_l = Some(0.0);
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
        return out;
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

    out.sd_l = Some(sd);
    out.se_l = Some(se);
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
    out
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
pub struct LeeBoundsReport {
    pub n_a: usize,
    pub n_b: usize,
    pub k: usize,
    pub lee_lo: Option<f64>,
    pub lee_hi: Option<f64>,
    pub width: Option<f64>,
}

pub fn lee_trimming_bounds(
    survived_var_a: &[f64],
    survived_var_b: &[f64],
    eps: f64,
) -> LeeBoundsReport {
    if survived_var_a.is_empty() || survived_var_b.is_empty() {
        return LeeBoundsReport {
            n_a: survived_var_a.len(),
            n_b: survived_var_b.len(),
            k: 0,
            lee_lo: None,
            lee_hi: None,
            width: None,
        };
    }
    let mut ya: Vec<f64> = survived_var_a.iter().map(|v| (v + eps).ln()).collect();
    let mut yb: Vec<f64> = survived_var_b.iter().map(|v| (v + eps).ln()).collect();
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

#[derive(Clone, Serialize, Deserialize)]
pub struct MediatorReport {
    pub n: usize,
    pub slope: Option<f64>,
    pub intercept: Option<f64>,
    pub pearson_r: Option<f64>,
    pub spearman_rho: Option<f64>,
    pub r2: Option<f64>,
    pub p_slope: Option<f64>,
}

pub fn paired_mediator_ols(births_a: &[f64], births_b: &[f64], diffs_l: &[f64]) -> MediatorReport {
    assert_eq!(births_a.len(), births_b.len());
    assert_eq!(births_a.len(), diffs_l.len());
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for i in 0..births_a.len() {
        if births_a[i] > 0.0 && births_b[i] > 0.0 && diffs_l[i].is_finite() {
            xs.push((births_b[i] / births_a[i]).ln());
            ys.push(diffs_l[i]);
        }
    }
    let n = xs.len();
    if n < 3 {
        return MediatorReport {
            n,
            slope: None,
            intercept: None,
            pearson_r: None,
            spearman_rho: None,
            r2: None,
            p_slope: None,
        };
    }
    let mx = mean_slice(&xs).unwrap_or(0.0);
    let my = mean_slice(&ys).unwrap_or(0.0);
    let mut sxx = 0.0;
    let mut syy = 0.0;
    let mut sxy = 0.0;
    for i in 0..n {
        let dx = xs[i] - mx;
        let dy = ys[i] - my;
        sxx += dx * dx;
        syy += dy * dy;
        sxy += dx * dy;
    }
    let (rx, _) = ranks_with_ties(&xs);
    let (ry, _) = ranks_with_ties(&ys);
    let mrx = mean_slice(&rx).unwrap_or(0.0);
    let mry = mean_slice(&ry).unwrap_or(0.0);
    let mut srxx = 0.0;
    let mut sryy = 0.0;
    let mut srxy = 0.0;
    for i in 0..n {
        let dx = rx[i] - mrx;
        let dy = ry[i] - mry;
        srxx += dx * dx;
        sryy += dy * dy;
        srxy += dx * dy;
    }
    let spearman_rho = if srxx > 0.0 && sryy > 0.0 {
        Some(srxy / (srxx * sryy).sqrt())
    } else {
        None
    };
    if sxx <= 0.0 {
        return MediatorReport {
            n,
            slope: None,
            intercept: Some(my),
            pearson_r: None,
            spearman_rho,
            r2: None,
            p_slope: None,
        };
    }
    let slope = sxy / sxx;
    let intercept = my - slope * mx;
    let r = if syy > 0.0 {
        sxy / (sxx * syy).sqrt()
    } else {
        0.0
    };
    let r2 = r * r;
    let df = n - 2;
    let rss = (syy - slope * sxy).max(0.0);
    let se_slope = (rss / (df as f64 * sxx)).sqrt();
    let p_slope = if se_slope > 0.0 {
        student_t_two_sided_p(slope / se_slope, df)
    } else if slope == 0.0 {
        1.0
    } else {
        0.0
    };
    MediatorReport {
        n,
        slope: Some(slope),
        intercept: Some(intercept),
        pearson_r: Some(r),
        spearman_rho,
        r2: Some(r2),
        p_slope: Some(p_slope),
    }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct R10Decision {
    pub label: &'static str,
    pub direction_tag: &'static str,
}

#[allow(clippy::too_many_arguments)]
pub fn classify_e3b_decision(
    wp2_and_host_valid: bool,
    failed_runs_zero: bool,
    n_complete_pairs: usize,
    n_requested: usize,
    min_pi_ll: f64,
    min_pairs_r7: usize,
    p_holm: Option<f64>,
    fwer_alpha: f64,
    mean_l: Option<f64>,
    tost_equivalent: bool,
    sesoi: f64,
    directional_positive: bool,
) -> R10Decision {
    if !wp2_and_host_valid {
        return R10Decision {
            label: "insufficient_evidence",
            direction_tag: "harness_invalid",
        };
    }
    let pi_ll = if n_requested > 0 {
        n_complete_pairs as f64 / n_requested as f64
    } else {
        0.0
    };
    let Some(m) = mean_l.filter(|v| v.is_finite()) else {
        return R10Decision {
            label: "insufficient_evidence",
            direction_tag: "attrition_below_pi85_floor",
        };
    };
    if !failed_runs_zero || n_complete_pairs < min_pairs_r7 || pi_ll < min_pi_ll {
        return R10Decision {
            label: "insufficient_evidence",
            direction_tag: "attrition_below_pi85_floor",
        };
    }
    let lt_sesoi = m.abs() < sesoi;
    if !lt_sesoi && tost_equivalent {
        return R10Decision {
            label: "insufficient_evidence",
            direction_tag: "ci_inconsistent_impossible",
        };
    }
    let holm_sig = p_holm.is_some_and(|p| p.is_finite() && p < fwer_alpha);
    if holm_sig {
        if m >= sesoi {
            return R10Decision {
                label: "supports",
                direction_tag: if directional_positive {
                    "predicted_positive_L"
                } else {
                    "non_zero_two_sided_L"
                },
            };
        }
        if m <= -sesoi {
            if directional_positive {
                return R10Decision {
                    label: "opposite",
                    direction_tag: "opposite_negative_L",
                };
            }
            return R10Decision {
                label: "supports",
                direction_tag: "non_zero_two_sided_L",
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

// ---- Entry points & CLI ------------------------------------------------------------------------

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run_with_config(argv, RuntimeEnv::from_process()) {
        eprintln!("run_e3b_experiment failed or refused: {e}");
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
                "usage: run_e3b_experiment (--smoke | --ensemble | --replay --arm <arm> --seed <seed> | --self-test) [--manifest-dir <dir>] [--smoke-dir <dir>] [--out <dir>] [--checkpoint-dir <dir>] [--jobs <n>]"
            );
            Ok(())
        }
        Mode::SelfTest => run_self_test(&env),
        Mode::Smoke => run_smoke(&args, &env),
        Mode::Ensemble => run_ensemble(&args, &env),
        Mode::Replay => run_replay(&args, &env),
    }
}

fn resolve_default_manifest_dir() -> PathBuf {
    let direct = Path::new(DEFAULT_MANIFEST_DIR);
    if direct.join(PREREG_FILE).exists() {
        return direct.to_path_buf();
    }
    let from_crate = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/experiments_e3b");
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
    let mut arm: Option<String> = None;
    let mut seed: Option<u64> = None;

    let mut it = argv.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--help" | "-h" => set_mode(&mut mode, Mode::Help)?,
            "--smoke" => set_mode(&mut mode, Mode::Smoke)?,
            "--ensemble" => set_mode(&mut mode, Mode::Ensemble)?,
            "--replay" => set_mode(&mut mode, Mode::Replay)?,
            "--self-test" => set_mode(&mut mode, Mode::SelfTest)?,
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

    let mode =
        mode.ok_or("must pass one mode: --smoke, --ensemble, --replay, --self-test, or --help")?;
    match mode {
        Mode::Replay => {
            if arm.is_none() || seed.is_none() {
                return Err("--replay requires both --arm <arm> and --seed <u64>".into());
            }
        }
        Mode::Help | Mode::Smoke | Mode::Ensemble | Mode::SelfTest => {
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
        checkpoint_dir,
        jobs,
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

// ---- Pre-run & manifest checks -----------------------------------------------------------------

fn refuse_debug(env: &RuntimeEnv, mode: Mode) -> Result<(), String> {
    if env.is_debug_build {
        return Err(format!(
            "refuse --{}: E3b execution requires a --release build (debug_assertions = true)",
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
            "refuse: ANIMA_WORLD_ARTIFACT is set ('{val}'); E3b requires it unset before starting"
        ));
    }
    let temp_artifact = env.temp_dir.join(TEMP_ARTIFACT_NAME);
    if temp_artifact.exists() {
        return Err(format!(
            "refuse: desktop world artifact {} exists; E3b requires it absent before starting",
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

    let mf = &raw["manifests"];
    let control_file = mf["control"]
        .as_str()
        .ok_or("preregistration missing manifests.control")?
        .to_string();
    let scarcity_file = mf["control_uniform_scarcity"]
        .as_str()
        .ok_or("preregistration missing manifests.control_uniform_scarcity")?
        .to_string();
    let treatment_file = mf["treatment"]
        .as_str()
        .ok_or("preregistration missing manifests.treatment")?
        .to_string();
    let smoke_control_file = mf["smoke_control"]
        .as_str()
        .ok_or("preregistration missing manifests.smoke_control")?
        .to_string();
    let smoke_scarcity_file = mf["smoke_control_scarcity"]
        .as_str()
        .ok_or("preregistration missing manifests.smoke_control_scarcity")?
        .to_string();
    let smoke_treatment_file = mf["smoke_treatment"]
        .as_str()
        .ok_or("preregistration missing manifests.smoke_treatment")?
        .to_string();

    let execution_order: Vec<u64> = raw["seeds"]["execution_order"]
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
    if execution_order.contains(&smoke_seed) {
        return Err(format!(
            "smoke_seed {smoke_seed} collides with seeds.execution_order"
        ));
    }

    let dur = &raw["duration"];
    let duration_ticks = dur["duration_ticks"]
        .as_u64()
        .ok_or("preregistration missing duration.duration_ticks")?;
    let sample_period = dur["sample_period"]
        .as_u64()
        .ok_or("preregistration missing duration.sample_period")?;
    let warmup_ticks = dur["warmup_ticks"]
        .as_u64()
        .ok_or("preregistration missing duration.warmup_ticks")?;
    let tw = dur["tail_window_ticks"]
        .as_array()
        .ok_or("preregistration missing duration.tail_window_ticks")?;
    if tw.len() != 2 {
        return Err("tail_window_ticks must have length 2".into());
    }
    let tail_window = [
        tw[0].as_u64().ok_or("tail_window_ticks[0] must be u64")?,
        tw[1].as_u64().ok_or("tail_window_ticks[1] must be u64")?,
    ];

    let th = &raw["thresholds"];
    let thresholds = ThresholdsSpec {
        eps_primary: th["eps_primary"]
            .as_f64()
            .ok_or("thresholds missing eps_primary")?,
        eps_sensitivity: th["eps_sensitivity"]
            .as_f64()
            .ok_or("thresholds missing eps_sensitivity")?,
        sesoi_log_var_ratio: th["sesoi_log_var_ratio"]
            .as_f64()
            .ok_or("thresholds missing sesoi_log_var_ratio")?,
        fwer_alpha: th["fwer_alpha"]
            .as_f64()
            .ok_or("thresholds missing fwer_alpha")?,
        min_pi_ll: th["min_pi_ll"]
            .as_f64()
            .ok_or("thresholds missing min_pi_ll")?,
        min_valid_pairs: th["min_valid_pairs_for_decision"]
            .as_u64()
            .ok_or("thresholds missing min_valid_pairs_for_decision")?
            as usize,
        min_non_zero_pairs_r7: th["min_non_zero_pairs_r7"]
            .as_u64()
            .ok_or("thresholds missing min_non_zero_pairs_r7")?
            as usize,
        max_abs_eu_drift: th["max_abs_eu_drift"]
            .as_f64()
            .ok_or("thresholds missing max_abs_eu_drift")?,
    };

    Ok(PreregSpec {
        raw,
        status,
        control_file,
        scarcity_file,
        treatment_file,
        smoke_control_file,
        smoke_scarcity_file,
        smoke_treatment_file,
        execution_order,
        smoke_seed,
        duration_ticks,
        sample_period,
        warmup_ticks,
        tail_window,
        thresholds,
    })
}

pub fn load_and_validate_confirmation_manifests(
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
        validate_common_manifest(label, m, prereg, registry)?;
        if m.seeds != prereg.execution_order {
            return Err(format!(
                "{label}: seeds {:?} disagree with execution_order {:?}",
                m.seeds, prereg.execution_order
            ));
        }
        if m.seeds.contains(&prereg.smoke_seed) {
            return Err(format!(
                "{label}: confirmation manifest contains smoke_seed {}",
                prereg.smoke_seed
            ));
        }
    }
    validate_arm_factor_diffs(&ctrl, &scarce, &treat)?;
    Ok((ctrl, scarce, treat))
}

pub fn load_and_validate_smoke_manifests(
    manifest_dir: &Path,
    prereg: &PreregSpec,
    registry: &ObservableRegistry,
) -> Result<(ExperimentManifest, ExperimentManifest, ExperimentManifest), String> {
    registry
        .validate()
        .map_err(|e| format!("invalid live registry: {e}"))?;
    let ctrl = load_manifest(&manifest_dir.join(&prereg.smoke_control_file))?;
    let scarce = load_manifest(&manifest_dir.join(&prereg.smoke_scarcity_file))?;
    let treat = load_manifest(&manifest_dir.join(&prereg.smoke_treatment_file))?;

    for (label, m) in [
        ("smoke_control_uniform", &ctrl),
        ("smoke_control_uniform_scarcity", &scarce),
        ("smoke_treatment_gradient", &treat),
    ] {
        validate_common_manifest(label, m, prereg, registry)?;
        if m.seeds != vec![prereg.smoke_seed] {
            return Err(format!(
                "{label}: expected seeds [{}], got {:?}",
                prereg.smoke_seed, m.seeds
            ));
        }
        if m.seeds.iter().any(|s| prereg.execution_order.contains(s)) {
            return Err(format!(
                "{label}: smoke manifest contains confirmation seed"
            ));
        }
        for obs in &m.observable_ids {
            if obs.to_ascii_lowercase().contains("trait_mass") {
                return Err(format!(
                    "blinding violation: {label} contains body-mass observable '{obs}'"
                ));
            }
        }
    }
    validate_arm_factor_diffs(&ctrl, &scarce, &treat)?;
    Ok((ctrl, scarce, treat))
}

fn validate_common_manifest(
    label: &str,
    m: &ExperimentManifest,
    prereg: &PreregSpec,
    registry: &ObservableRegistry,
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
    if m.sample_period != prereg.sample_period {
        return Err(format!(
            "{label}: sample_period {} != {}",
            m.sample_period, prereg.sample_period
        ));
    }
    Ok(())
}

fn validate_arm_factor_diffs(
    ctrl: &ExperimentManifest,
    scarce: &ExperimentManifest,
    treat: &ExperimentManifest,
) -> Result<(), String> {
    FactorDiff {
        allowed_paths: ALLOWED_TREATMENT_DIFF
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    }
    .validate(ctrl, treat)
    .map_err(|e| format!("FactorDiff(control_uniform, treatment_gradient) failed: {e}"))?;

    let scarcity_diff = FactorDiff {
        allowed_paths: ALLOWED_SCARCITY_DIFF
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    };
    scarcity_diff.validate(ctrl, scarce).map_err(|e| {
        format!("FactorDiff(control_uniform, control_uniform_scarcity) failed: {e}")
    })?;
    scarcity_diff.validate(scarce, treat).map_err(|e| {
        format!("FactorDiff(control_uniform_scarcity, treatment_gradient) failed: {e}")
    })?;
    Ok(())
}

// ---- `--smoke` (blind WP-2 runner) -------------------------------------------------------------

fn probe_genesis(manifest: &ExperimentManifest, seed: u64) -> Result<FieldProbe, String> {
    let adapter = LiveExperimentAdapter::from_manifest(
        &manifest.laws,
        &manifest.initial_conditions,
        &manifest.exotic_interventions,
        seed,
        (16, 16),
        manifest.duration_ticks,
    )
    .map_err(|e| format!("building '{}' at seed {seed}: {e}", manifest.experiment_id))?;

    let mut world = adapter.world();
    let world_identity = world
        .get_resource::<WorldIdentity>()
        .copied()
        .ok_or("live world missing WorldIdentity")?;

    let (rng_seed, rng_stream_pos, rng_next_draws) = {
        let mut rng = world
            .get_resource_mut::<SimRng>()
            .ok_or("live world missing SimRng")?;
        let s = rng.seed();
        let pos = rng.stream_pos();
        let draws: Vec<u64> = (0..8).map(|_| rng.rng().next_u64()).collect();
        (s, pos, draws)
    };

    let (resource_field_present, width, height, initial_total_biomass, initial_total_capacity) =
        match world.get_resource::<ResourceField>() {
            Some(f) => {
                let total_r: f64 = f.r.iter().map(|v| *v as f64).sum();
                let total_cap: f64 = f.r_max.iter().map(|v| *v as f64).sum();
                (true, f.width, f.height, total_r, total_cap)
            }
            None => (false, 0, 0, 0.0, 0.0),
        };

    let bio = world
        .get_resource::<EcosystemBiomass>()
        .copied()
        .unwrap_or(EcosystemBiomass {
            plants: 0.0,
            detritus: 0.0,
            animals: 0.0,
            dormant: 0.0,
        });

    Ok(FieldProbe {
        world_identity,
        rng_seed,
        rng_stream_pos,
        rng_next_draws,
        resource_field_present,
        width,
        height,
        initial_total_biomass,
        initial_total_capacity,
        ecosystem_plants: bio.plants,
        ecosystem_detritus: bio.detritus,
        ecosystem_animals: bio.animals,
        ecosystem_dormant: bio.dormant,
    })
}

fn summarize_smoke_arm(
    arm: &'static str,
    run: &RunResult,
    prereg: &PreregSpec,
) -> Result<SmokeArmSummary, String> {
    let terrain_source = run
        .provenance
        .terrain_source
        .clone()
        .unwrap_or_else(|| "missing".to_string());
    let world_identity = run.provenance.world_identity.unwrap_or_default();

    let status = match &run.status {
        RunStatus::Completed => "Completed",
        RunStatus::Failed { .. } => "Failed",
    };

    let obs_at = |sample: &[(String, f64)], key: &str| -> Result<f64, String> {
        sample
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| *v)
            .ok_or_else(|| format!("{arm}: missing observable '{key}'"))
    };

    let mut all_finite = run.final_observables.iter().all(|(_, v)| v.is_finite());
    let mut standing_crop_post = Vec::new();
    let mut first_positive_detritus_before_warmup = None;
    let mut detritus_at_warmup = None;
    let mut closed_eu_vals = Vec::with_capacity(run.series.len());

    for sample in &run.series {
        if !sample.observables.iter().all(|(_, v)| v.is_finite()) {
            all_finite = false;
        }
        let sc = obs_at(&sample.observables, "live.standing_crop")?;
        let det = obs_at(&sample.observables, "detritus")?;
        let eu = obs_at(&sample.observables, "live.closed_eu_total")?;
        closed_eu_vals.push(eu);
        if sample.tick > prereg.warmup_ticks {
            standing_crop_post.push(sc);
        }
        if sample.tick < prereg.warmup_ticks
            && det > 0.0
            && first_positive_detritus_before_warmup.is_none()
        {
            first_positive_detritus_before_warmup = Some((sample.tick, det));
        }
        if sample.tick == prereg.warmup_ticks {
            detritus_at_warmup = Some(det);
        }
    }

    let standing_crop_final = obs_at(&run.final_observables, "live.standing_crop")?;
    let detritus_final = obs_at(&run.final_observables, "detritus")?;
    let agent_count_final = obs_at(&run.final_observables, "live.agent_count")?;
    let predator_count_final = obs_at(&run.final_observables, "live.predator_count")?;

    let eu_base = closed_eu_vals.first().copied().unwrap_or(0.0);
    let max_abs_eu_drift = closed_eu_vals
        .iter()
        .map(|v| (*v - eu_base).abs())
        .fold(0.0_f64, f64::max);

    let mut deforest_ledger_count = 0usize;
    let mut add_nutrient_ledger_count = 0usize;
    for rec in run.ledger.all() {
        if rec.cause_id == DEFOREST_CAUSE_ID && rec.target == "plants" {
            deforest_ledger_count += 1;
        } else if rec.cause_id == ADD_NUTRIENT_CAUSE_ID && rec.target == "plants" {
            add_nutrient_ledger_count += 1;
        }
    }

    Ok(SmokeArmSummary {
        arm,
        status,
        final_checksum: run.final_checksum,
        terrain_source,
        world_identity,
        standing_crop_final,
        standing_crop_post_warmup_mean: mean_slice(&standing_crop_post).unwrap_or(0.0),
        detritus_final,
        first_positive_detritus_before_warmup,
        detritus_at_warmup,
        max_abs_eu_drift,
        agent_count_final,
        predator_count_final,
        emitted_count: run.final_observables.len(),
        all_finite,
        deforest_ledger_count,
        add_nutrient_ledger_count,
    })
}

fn assert_dir_has_no_trait_mass(dir: &Path) -> Result<(), String> {
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
                        "blinding check failed: forbidden substring found in {}",
                        path.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

fn run_smoke(args: &Args, env: &RuntimeEnv) -> Result<(), String> {
    refuse_debug(env, Mode::Smoke)?;
    let out_dir = args
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SMOKE_OUT));
    ensure_empty_out_dir(&out_dir)?;
    let prerun_check = check_world_artifact_prerun(env)?;

    let registry = ObservableRegistry::live_default();
    let prereg = load_prereg(&args.manifest_dir)?;
    let (ctrl, scarce, treat) =
        load_and_validate_smoke_manifests(&args.manifest_dir, &prereg, &registry)?;

    let probe_u = probe_genesis(&ctrl, prereg.smoke_seed)?;
    let probe_s = probe_genesis(&scarce, prereg.smoke_seed)?;
    let probe_t = probe_genesis(&treat, prereg.smoke_seed)?;

    let wid_equal = probe_u.world_identity == probe_t.world_identity
        && probe_u.world_identity == probe_s.world_identity;
    let rng_equal = probe_u.rng_seed == probe_t.rng_seed
        && probe_u.rng_stream_pos == probe_t.rng_stream_pos
        && probe_u.rng_next_draws == probe_t.rng_next_draws
        && probe_u.rng_seed == probe_s.rng_seed
        && probe_u.rng_stream_pos == probe_s.rng_stream_pos
        && probe_u.rng_next_draws == probe_s.rng_next_draws;
    let total_eu_u = probe_u.total_ecosystem_eu();
    let total_eu_s = probe_s.total_ecosystem_eu();
    let total_eu_t = probe_t.total_ecosystem_eu();
    let genesis_eu_equal =
        (total_eu_u - total_eu_s).abs() < 1e-6 && (total_eu_u - total_eu_t).abs() < 1e-6;

    let wp2_a = probe_u.resource_field_present
        && probe_s.resource_field_present
        && probe_t.resource_field_present
        && probe_u.width == 256
        && probe_u.height == 256
        && wid_equal
        && rng_equal
        && genesis_eu_equal
        && probe_u.initial_total_biomass > 0.0
        && probe_u.initial_total_capacity > 0.0
        && ((probe_s.initial_total_capacity / probe_u.initial_total_capacity) - 0.42).abs() < 1e-4;

    let started = SystemTime::now();
    let timer = Instant::now();
    let paired = run_paired_ensemble_with_control::<LiveExperimentAdapter>(
        &ctrl,
        &treat,
        &registry,
        &FactorDiff {
            allowed_paths: vec!["interventions".to_string()],
        },
    )
    .map_err(|e| format!("smoke paired run failed: {e}"))?;
    let scarcity_run = run_manifest_seed::<LiveExperimentAdapter>(
        &scarce,
        &registry,
        prereg.smoke_seed,
        None,
        None,
    );
    let elapsed_secs = timer.elapsed().as_secs_f64();
    let ended = SystemTime::now();

    let pair = paired.pairs.first().ok_or("smoke paired report empty")?;
    if !pair.is_complete() || !scarcity_run.status.is_completed() {
        return Err(format!(
            "smoke arm failed: ctrl={:?}, scarce={:?}, treat={:?}",
            pair.control.status, scarcity_run.status, pair.treatment.status
        ));
    }

    let sum_u = summarize_smoke_arm("control_uniform", &pair.control, &prereg)?;
    let sum_s = summarize_smoke_arm("control_uniform_scarcity", &scarcity_run, &prereg)?;
    let sum_t = summarize_smoke_arm("treatment_gradient", &pair.treatment, &prereg)?;

    let min_obs = ctrl.observable_ids.len();
    for s in [&sum_u, &sum_s, &sum_t] {
        if s.emitted_count < min_obs || !s.all_finite {
            return Err(format!(
                "{}: emitted_count={}, all_finite={}",
                s.arm, s.emitted_count, s.all_finite
            ));
        }
        if s.terrain_source != "generated" || s.world_identity != probe_u.world_identity {
            return Err(format!("{}: terrain/world_identity mismatch", s.arm));
        }
        if s.max_abs_eu_drift > prereg.thresholds.max_abs_eu_drift {
            return Err(format!(
                "{}: max_abs_eu_drift {} > {}",
                s.arm, s.max_abs_eu_drift, prereg.thresholds.max_abs_eu_drift
            ));
        }
    }

    let rel_shift_b1 = if sum_u.standing_crop_post_warmup_mean != 0.0 {
        (sum_t.standing_crop_post_warmup_mean - sum_u.standing_crop_post_warmup_mean)
            / sum_u.standing_crop_post_warmup_mean.abs()
    } else {
        0.0
    };
    let wp2_b1 = sum_u.final_checksum != sum_t.final_checksum
        && sum_t.deforest_ledger_count > 0
        && sum_t.add_nutrient_ledger_count > 0
        && rel_shift_b1.abs() >= WP2_B1_MIN_REL_SHIFT;

    let rel_mismatch_b2 = if sum_t.standing_crop_post_warmup_mean != 0.0 {
        (sum_s.standing_crop_post_warmup_mean - sum_t.standing_crop_post_warmup_mean)
            / sum_t.standing_crop_post_warmup_mean.abs()
    } else {
        1.0
    };
    let wp2_b2 = sum_s.final_checksum != sum_t.final_checksum
        && sum_s.deforest_ledger_count == 0
        && sum_s.add_nutrient_ledger_count > 0
        && rel_mismatch_b2.abs() <= WP2_B2_MAX_REL_MISMATCH;

    let wp2_c = [&sum_u, &sum_s, &sum_t].iter().all(|s| {
        s.first_positive_detritus_before_warmup.is_some()
            && s.detritus_at_warmup.is_some_and(|v| v > 0.0)
    });

    let wp2_passed = wp2_a && wp2_b1 && wp2_b2 && wp2_c;

    let manifests_dir = out_dir.join("manifests");
    std::fs::create_dir_all(&manifests_dir)
        .map_err(|e| format!("create {}: {e}", manifests_dir.display()))?;

    let mut hashed_names: Vec<String> = vec![
        "provenance.json".to_string(),
        "smoke-report.json".to_string(),
        "smoke-series.csv".to_string(),
    ];
    for file in [
        prereg.smoke_control_file.as_str(),
        prereg.smoke_scarcity_file.as_str(),
        prereg.smoke_treatment_file.as_str(),
    ] {
        let src = args.manifest_dir.join(file);
        let raw = std::fs::read_to_string(&src).map_err(|e| format!("read {file}: {e}"))?;
        write_text(&manifests_dir.join(file), &raw)?;
        hashed_names.push(format!("manifests/{file}"));
    }

    let smoke_series_csv = build_smoke_series_csv(&[
        ("control_uniform", &pair.control),
        ("control_uniform_scarcity", &scarcity_run),
        ("treatment_gradient", &pair.treatment),
    ]);
    write_text(&out_dir.join("smoke-series.csv"), &smoke_series_csv)?;

    let arm_rows: Vec<serde_json::Value> = [&sum_u, &sum_s, &sum_t]
        .iter()
        .map(|s| {
            serde_json::json!({
                "arm": s.arm,
                "status": s.status,
                "final_checksum": s.final_checksum,
                "standing_crop_final": s.standing_crop_final,
                "standing_crop_post_warmup_mean": s.standing_crop_post_warmup_mean,
                "detritus_final": s.detritus_final,
                "first_positive_detritus_before_warmup": s.first_positive_detritus_before_warmup,
                "detritus_at_warmup": s.detritus_at_warmup,
                "max_abs_eu_drift": s.max_abs_eu_drift,
                "agent_count_final": s.agent_count_final,
                "predator_count_final": s.predator_count_final,
            })
        })
        .collect();
    let smoke_report = serde_json::json!({
        "kind": "e3b-blind-smoke-report",
        "seed": prereg.smoke_seed,
        "duration_ticks": prereg.duration_ticks,
        "warmup_ticks": prereg.warmup_ticks,
        "sample_period": prereg.sample_period,
        "body_mass_consulted": false,
        "wp2_passed": wp2_passed,
        "wp2_a_resource_field": {
            "passed": wp2_a,
            "width": probe_u.width,
            "height": probe_u.height,
            "initial_closed_eu_uniform": total_eu_u,
            "initial_closed_eu_scarcity": total_eu_s,
            "initial_closed_eu_treatment": total_eu_t,
            "uniform_capacity_eu": probe_u.initial_total_capacity,
            "scarcity_capacity_eu": probe_s.initial_total_capacity,
            "capacity_ratio": probe_s.initial_total_capacity / probe_u.initial_total_capacity.max(1e-12),
            "world_identity_identical": wid_equal,
            "rng_stream_identical": rng_equal,
        },
        "wp2_b1_gradient_vs_uniform": {
            "passed": wp2_b1,
            "relative_post_warmup_shift": rel_shift_b1,
            "treatment_deforest_events": sum_t.deforest_ledger_count,
            "treatment_add_nutrient_events": sum_t.add_nutrient_ledger_count,
        },
        "wp2_b2_scarcity_vs_gradient": {
            "passed": wp2_b2,
            "relative_post_warmup_mismatch": rel_mismatch_b2,
            "scarcity_deforest_events": sum_s.deforest_ledger_count,
            "scarcity_add_nutrient_events": sum_s.add_nutrient_ledger_count,
        },
        "wp2_c_detritus_active": {
            "passed": wp2_c,
        },
        "arms": arm_rows,
        "elapsed_seconds": elapsed_secs,
        "started_utc": iso8601_utc(unix_seconds(started)),
        "ended_utc": iso8601_utc(unix_seconds(ended)),
    });
    write_text(
        &out_dir.join("smoke-report.json"),
        &(pretty(&smoke_report)? + "\n"),
    )?;

    let git_commit = resolve_git_commit(env);
    let git_dirty_paths = resolve_git_dirty_paths(env);
    let prov = serde_json::json!({
        "kind": "e3b-blind-smoke-provenance",
        "preregistration_id": prereg.raw["preregistration_id"],
        "preregistration_status": prereg.status,
        "body_mass_consulted": false,
        "terrain_source": "generated",
        "world_identity_observed": probe_u.world_identity,
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
        "smoke_seed": prereg.smoke_seed,
        "duration_ticks": prereg.duration_ticks,
        "warmup_ticks": prereg.warmup_ticks,
        "sample_period": prereg.sample_period,
    });
    write_text(&out_dir.join("provenance.json"), &(pretty(&prov)? + "\n"))?;

    let refs: Vec<&str> = hashed_names.iter().map(String::as_str).collect();
    write_checksums(&out_dir, &refs)?;
    assert_dir_has_no_trait_mass(&out_dir)?;

    println!(
        "E3b blind smoke seed {}: wp2_passed={wp2_passed} (a={wp2_a}, b1={wp2_b1}, b2={wp2_b2}, c={wp2_c}) -> {}",
        prereg.smoke_seed,
        out_dir.display()
    );
    Ok(())
}

fn build_smoke_series_csv(runs: &[(&str, &RunResult)]) -> String {
    let mut s = String::from("seed,arm,tick");
    for col in SMOKE_SERIES_OBSERVABLES {
        s.push(',');
        s.push_str(col);
    }
    s.push('\n');
    for (arm, run) in runs {
        for sample in &run.series {
            s.push_str(&format!("{},{},{}", run.provenance.seed, arm, sample.tick));
            for col in SMOKE_SERIES_OBSERVABLES {
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

// ---- `--ensemble` (resumable confirmation runner) ----------------------------------------------

fn validate_smoke_dir_for_ensemble(dir: &Path) -> Result<SmokeVerification, String> {
    let prov_path = dir.join("provenance.json");
    let report_path = dir.join("smoke-report.json");
    let checksum_path = dir.join("checksums.sha256");
    if !prov_path.exists() || !report_path.exists() || !checksum_path.exists() {
        return Err(format!(
            "smoke directory {} must contain provenance.json, smoke-report.json, and checksums.sha256",
            dir.display()
        ));
    }
    assert_dir_has_no_trait_mass(dir)?;
    verify_checksum_manifest(dir, &["provenance.json", "smoke-report.json"])?;

    let prov = read_json(&prov_path)?;
    let report = read_json(&report_path)?;
    let wp2_passed = report["wp2_passed"].as_bool() == Some(true);
    if !wp2_passed {
        return Err(format!(
            "refuse: smoke record {} has wp2_passed != true",
            dir.display()
        ));
    }
    if report["body_mass_consulted"].as_bool() == Some(true) {
        return Err("refuse: smoke record violated blinding".into());
    }

    let wid_obj = &prov["world_identity_observed"];
    let world_identity = if wid_obj.is_object() {
        let sim_world_sha256 = match wid_obj.get("sim_world_sha256") {
            Some(v) if !v.is_null() => {
                let hex = v
                    .as_str()
                    .ok_or("smoke wid sim_world_sha256 must be hex string")?;
                Some(
                    anima_worldgen::anmw_v3::sha256_from_hex(hex)
                        .map_err(|e| format!("smoke wid sim_world_sha256: {e}"))?,
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
        })
    } else {
        None
    };

    Ok(SmokeVerification {
        dir: dir.to_path_buf(),
        provenance_sha256: sha256_file(&prov_path).map_err(|e| e.to_string())?,
        smoke_report_sha256: sha256_file(&report_path).map_err(|e| e.to_string())?,
        git_commit: prov["build"]["git_commit"].as_str().map(str::to_string),
        host_os: prov["host"]["os"].as_str().map(str::to_string),
        host_arch: prov["host"]["arch"].as_str().map(str::to_string),
        profile: prov["build"]["profile"].as_str().map(str::to_string),
        world_identity,
        wp2_passed,
    })
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

struct TaskItem<'a> {
    seed: u64,
    arm: &'static str,
    manifest: &'a ExperimentManifest,
}

fn try_load_checkpoint(
    ckpt_dir: Option<&Path>,
    seed: u64,
    arm: &str,
    expected_fp: u64,
) -> Option<RunCheckpoint> {
    let dir = ckpt_dir?;
    let path = dir.join(format!("{seed}__{arm}.json"));
    if !path.exists() {
        return None;
    }
    let raw = std::fs::read_to_string(path).ok()?;
    let cp: RunCheckpoint = serde_json::from_str(&raw).ok()?;
    if cp.seed == seed && cp.arm == arm && cp.manifest_fingerprint == expected_fp {
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
    let path = dir.join(format!("{}__{}.json", cp.seed, cp.arm));
    let tmp = dir.join(format!("{}__{}.json.tmp", cp.seed, cp.arm));
    let json = serde_json::to_string_pretty(cp).map_err(|e| e.to_string())?;
    write_text(&tmp, &(json + "\n"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("rename checkpoint: {e}"))?;
    Ok(())
}

fn run_ensemble(args: &Args, env: &RuntimeEnv) -> Result<(), String> {
    refuse_debug(env, Mode::Ensemble)?;
    let out_dir = args
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIRM_OUT));
    ensure_empty_out_dir(&out_dir)?;
    let prerun_check = check_world_artifact_prerun(env)?;

    let smoke_dir = args
        .smoke_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SMOKE_OUT));
    let smoke_ref = validate_smoke_dir_for_ensemble(&smoke_dir)?;

    let registry = ObservableRegistry::live_default();
    let prereg = load_prereg(&args.manifest_dir)?;
    let (ctrl, scarce, treat) =
        load_and_validate_confirmation_manifests(&args.manifest_dir, &prereg, &registry)?;

    if let Some(ckpt) = &args.checkpoint_dir {
        let c_ckpt = abs_clean(ckpt);
        let c_out = abs_clean(&out_dir);
        if c_ckpt == c_out || c_ckpt.starts_with(&c_out) {
            return Err("refuse: --checkpoint-dir must not live inside --out".into());
        }
    }

    let started = SystemTime::now();
    let timer = Instant::now();
    let mut tasks = Vec::with_capacity(prereg.execution_order.len() * ARM_ORDER.len());
    for &seed in &prereg.execution_order {
        tasks.push(TaskItem {
            seed,
            arm: "control_uniform",
            manifest: &ctrl,
        });
        tasks.push(TaskItem {
            seed,
            arm: "control_uniform_scarcity",
            manifest: &scarce,
        });
        tasks.push(TaskItem {
            seed,
            arm: "treatment_gradient",
            manifest: &treat,
        });
    }

    let total_tasks = tasks.len();
    let slots: Vec<Mutex<Option<ArmRunRecord>>> =
        (0..total_tasks).map(|_| Mutex::new(None)).collect();
    let errors: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let next_idx = Mutex::new(0usize);

    let worker_count = args.jobs.min(total_tasks.max(1));
    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| loop {
                let idx = {
                    let mut guard = next_idx.lock().expect("task index mutex poisoned");
                    if *guard >= total_tasks {
                        return;
                    }
                    let cur = *guard;
                    *guard += 1;
                    cur
                };
                let t = &tasks[idx];
                let fp = t.manifest.fingerprint();
                if let Some(cp) =
                    try_load_checkpoint(args.checkpoint_dir.as_deref(), t.seed, t.arm, fp)
                {
                    let mut wid = cp.world_identity;
                    if let Some(hook) = env.world_identity_override {
                        wid = hook(idx, t.arm, t.seed, wid);
                    }
                    *slots[idx].lock().expect("slot lock") = Some(ArmRunRecord {
                        seed: cp.seed,
                        arm: cp.arm,
                        result: cp.result,
                        terrain_source: cp.terrain_source,
                        world_identity: wid,
                        elapsed_secs: cp.elapsed_secs,
                    });
                    continue;
                }

                let c = Instant::now();
                let res = run_manifest_seed::<LiveExperimentAdapter>(
                    t.manifest, &registry, t.seed, None, None,
                );
                let elapsed_secs = c.elapsed().as_secs_f64();

                let terrain_source = match res.provenance.terrain_source.clone() {
                    Some(ts) => ts,
                    None => {
                        errors
                            .lock()
                            .unwrap()
                            .push(format!("{}/{}: missing terrain_source", t.seed, t.arm));
                        return;
                    }
                };
                let mut wid = match res.provenance.world_identity {
                    Some(w) => w,
                    None => {
                        errors
                            .lock()
                            .unwrap()
                            .push(format!("{}/{}: missing world_identity", t.seed, t.arm));
                        return;
                    }
                };
                if let Some(hook) = env.world_identity_override {
                    wid = hook(idx, t.arm, t.seed, wid);
                }
                let cp = RunCheckpoint {
                    seed: t.seed,
                    arm: t.arm.to_string(),
                    manifest_fingerprint: fp,
                    terrain_source: terrain_source.clone(),
                    world_identity: wid,
                    elapsed_secs,
                    result: res.clone(),
                };
                if let Err(e) = save_checkpoint(args.checkpoint_dir.as_deref(), &cp) {
                    errors.lock().unwrap().push(e);
                    return;
                }
                *slots[idx].lock().expect("slot lock") = Some(ArmRunRecord {
                    seed: t.seed,
                    arm: t.arm.to_string(),
                    result: res,
                    terrain_source,
                    world_identity: wid,
                    elapsed_secs,
                });
            });
        }
    });

    let err_list = errors.into_inner().expect("errors mutex");
    if !err_list.is_empty() {
        return Err(err_list.join("; "));
    }

    let runs: Vec<ArmRunRecord> = slots
        .into_iter()
        .map(|s| s.into_inner().expect("slot mutex").expect("completed slot"))
        .collect();
    let total_elapsed_secs = timer.elapsed().as_secs_f64();
    let ended = SystemTime::now();

    let expected_wid = runs.first().ok_or("no runs executed")?.world_identity;
    if let Some(smoke_wid) = smoke_ref.world_identity {
        if smoke_wid != expected_wid {
            return Err(format!(
                "world_identity mismatch vs smoke: {:?} vs {:?}",
                smoke_wid, expected_wid
            ));
        }
    }
    for r in &runs {
        if r.terrain_source != "generated" || r.world_identity != expected_wid {
            return Err(format!(
                "terrain/world_identity mismatch at seed={} arm={} ({}, {:?})",
                r.seed, r.arm, r.terrain_source, r.world_identity
            ));
        }
    }

    write_ensemble_bundle(EnsembleWriteCtx {
        out_dir: &out_dir,
        manifest_dir: &args.manifest_dir,
        prereg: &prereg,
        manifests: &[
            ("control_uniform", &ctrl),
            ("control_uniform_scarcity", &scarce),
            ("treatment_gradient", &treat),
        ],
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
        "E3b confirmation ensemble completed: {} runs in {:.2}s -> {}",
        runs.len(),
        total_elapsed_secs,
        out_dir.display()
    );
    Ok(())
}

// ---- Analysis & Confirmation Bundle Writers ----------------------------------------------------

struct RunRuleSummary {
    seed: u64,
    arm: String,
    completed: bool,
    extinct: bool,
    first_extinct_tick: Option<u64>,
    survival_ticks: u64,
    singleton_final: bool,
    singleton_tail: bool,
    valid_final_var: bool,
    valid_tail_var: bool,
    empty_half: Option<&'static str>,
    var_final: f64,
    mean_mass_final: f64,
    iqr_mass_final: f64,
    s3_regional_diff: f64,
    tail_var_mean: Option<f64>,
    final_agents: f64,
    final_predators: f64,
    max_abs_eu_drift: f64,
    births_sum: f64,
    deaths_sum: f64,
    lifespan_at_t: Option<f64>,
    lifespan_post_warmup_pos: Option<f64>,
    cumulative_mean_lifespan: f64,
    post_warmup_crop: Option<f64>,
}

fn summarize_run_rules(rec: &ArmRunRecord, prereg: &PreregSpec) -> RunRuleSummary {
    let obs_val = |sample: &[(String, f64)], key: &str| -> Option<f64> {
        sample.iter().find(|(k, _)| k == key).map(|(_, v)| *v)
    };
    let mut first_extinct_tick = None;
    let mut tail_vars = Vec::new();
    let mut tail_min_agents = f64::INFINITY;
    let mut births_sum = 0.0;
    let mut deaths_sum = 0.0;
    let mut post_warmup_crop_vals = Vec::new();
    let mut lifespan_post_warmup_pos_vals = Vec::new();
    let mut lifespan_at_t = None;
    let mut eu_vals = Vec::with_capacity(rec.result.series.len());

    for s in &rec.result.series {
        if let Some(n) = obs_val(&s.observables, "live.agent_count") {
            if n == 0.0 && first_extinct_tick.is_none() {
                first_extinct_tick = Some(s.tick);
            }
            if s.tick >= prereg.tail_window[0]
                && s.tick <= prereg.tail_window[1]
                && n < tail_min_agents
            {
                tail_min_agents = n;
            }
        }
        if s.tick >= prereg.tail_window[0] && s.tick <= prereg.tail_window[1] {
            if let Some(v) = obs_val(&s.observables, "live.trait_mass_var") {
                tail_vars.push(v);
            }
        }
        births_sum += obs_val(&s.observables, "live.births").unwrap_or(0.0);
        let d = obs_val(&s.observables, "live.deaths").unwrap_or(0.0);
        deaths_sum += d;
        if s.tick > prereg.warmup_ticks {
            if let Some(crop) = obs_val(&s.observables, "live.standing_crop") {
                post_warmup_crop_vals.push(crop);
            }
            if d > 0.0 {
                if let Some(ls) = obs_val(&s.observables, "live.mean_lifespan") {
                    lifespan_post_warmup_pos_vals.push(ls);
                }
            }
        }
        if s.tick == prereg.duration_ticks {
            lifespan_at_t = obs_val(&s.observables, "live.mean_lifespan");
        }
        if let Some(eu) = obs_val(&s.observables, "live.closed_eu_total") {
            eu_vals.push(eu);
        }
    }

    let final_agents = rec.result.observable("live.agent_count").unwrap_or(0.0);
    if first_extinct_tick.is_none() && final_agents == 0.0 {
        first_extinct_tick = Some(prereg.duration_ticks);
    }
    let extinct = first_extinct_tick.is_some();
    let survival_ticks = first_extinct_tick.unwrap_or(prereg.duration_ticks);
    let singleton_final = !extinct && final_agents < 2.0;
    let singleton_tail = !extinct && (singleton_final || tail_min_agents < 2.0);
    let completed = rec.result.status.is_completed();
    let valid_final_var = completed && !extinct && !singleton_final;
    let valid_tail_var = completed && !extinct && !singleton_tail;

    let west_mean = rec
        .result
        .observable("live.trait_mass_mean_west")
        .unwrap_or(0.0);
    let east_mean = rec
        .result
        .observable("live.trait_mass_mean_east")
        .unwrap_or(0.0);
    let empty_half = if west_mean == 0.0 && east_mean == 0.0 {
        Some("both")
    } else if west_mean == 0.0 {
        Some("west")
    } else if east_mean == 0.0 {
        Some("east")
    } else {
        None
    };

    let eu_base = eu_vals.first().copied().unwrap_or(0.0);
    let max_abs_eu_drift = eu_vals
        .iter()
        .map(|v| (*v - eu_base).abs())
        .fold(0.0_f64, f64::max);

    RunRuleSummary {
        seed: rec.seed,
        arm: rec.arm.clone(),
        completed,
        extinct,
        first_extinct_tick,
        survival_ticks,
        singleton_final,
        singleton_tail,
        valid_final_var,
        valid_tail_var,
        empty_half,
        var_final: rec.result.observable("live.trait_mass_var").unwrap_or(0.0),
        mean_mass_final: rec.result.observable("live.trait_mass_mean").unwrap_or(0.0),
        iqr_mass_final: rec.result.observable("live.trait_mass_p90").unwrap_or(0.0)
            - rec.result.observable("live.trait_mass_p10").unwrap_or(0.0),
        s3_regional_diff: east_mean - west_mean,
        tail_var_mean: mean_slice(&tail_vars),
        final_agents,
        final_predators: rec.result.observable("live.predator_count").unwrap_or(0.0),
        max_abs_eu_drift,
        births_sum,
        deaths_sum,
        lifespan_at_t,
        lifespan_post_warmup_pos: mean_slice(&lifespan_post_warmup_pos_vals),
        cumulative_mean_lifespan: rec
            .result
            .observable("live.cumulative_mean_lifespan")
            .unwrap_or(0.0),
        post_warmup_crop: mean_slice(&post_warmup_crop_vals),
    }
}

struct ContrastDef {
    id: &'static str,
    name: &'static str,
    control_arm: &'static str,
    treatment_arm: &'static str,
    directional_positive: bool,
}

const CONTRAST_DEFS: [ContrastDef; 3] = [
    ContrastDef {
        id: "H1",
        name: "treatment_gradient / control_uniform_scarcity",
        control_arm: "control_uniform_scarcity",
        treatment_arm: "treatment_gradient",
        directional_positive: true,
    },
    ContrastDef {
        id: "H2",
        name: "treatment_gradient / control_uniform",
        control_arm: "control_uniform",
        treatment_arm: "treatment_gradient",
        directional_positive: false,
    },
    ContrastDef {
        id: "H3",
        name: "control_uniform_scarcity / control_uniform",
        control_arm: "control_uniform",
        treatment_arm: "control_uniform_scarcity",
        directional_positive: false,
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

    let mut raw = Vec::with_capacity(CONTRAST_DEFS.len());
    for c in &CONTRAST_DEFS {
        let mut var_a = Vec::new();
        let mut var_b = Vec::new();
        let mut births_a = Vec::new();
        let mut births_b = Vec::new();
        let mut surv_all_a = Vec::new();
        let mut surv_all_b = Vec::new();
        let mut surv_valid_var_a = Vec::new();
        let mut surv_valid_var_b = Vec::new();
        let mut s1_a = Vec::new();
        let mut s1_b = Vec::new();
        let mut s2_a = Vec::new();
        let mut s2_b = Vec::new();
        let mut s3_a = Vec::new();
        let mut s3_b = Vec::new();
        let mut tail_a = Vec::new();
        let mut tail_b = Vec::new();

        for &seed in &prereg.execution_order {
            let Some(sa) = summaries.get(&(seed, c.control_arm.to_string())) else {
                continue;
            };
            let Some(sb) = summaries.get(&(seed, c.treatment_arm.to_string())) else {
                continue;
            };
            surv_all_a.push(sa.survival_ticks as f64);
            surv_all_b.push(sb.survival_ticks as f64);
            if sa.valid_final_var {
                surv_valid_var_a.push(sa.var_final);
            }
            if sb.valid_final_var {
                surv_valid_var_b.push(sb.var_final);
            }
            if sa.valid_final_var && sb.valid_final_var {
                var_a.push(sa.var_final);
                var_b.push(sb.var_final);
                births_a.push(sa.births_sum);
                births_b.push(sb.births_sum);
                s1_a.push(sa.iqr_mass_final);
                s1_b.push(sb.iqr_mass_final);
                s2_a.push(sa.mean_mass_final);
                s2_b.push(sb.mean_mass_final);
                if sa.empty_half.is_none() && sb.empty_half.is_none() {
                    s3_a.push(sa.s3_regional_diff);
                    s3_b.push(sb.s3_regional_diff);
                }
            }
            if sa.valid_tail_var && sb.valid_tail_var {
                if let (Some(ta), Some(tb)) = (sa.tail_var_mean, sb.tail_var_mean) {
                    tail_a.push(ta);
                    tail_b.push(tb);
                }
            }
        }

        let primary = evaluate_paired_log_variance_ratio(
            &var_a,
            &var_b,
            th.eps_primary,
            th.sesoi_log_var_ratio,
            th.min_non_zero_pairs_r7,
            n_req,
        );
        let sens = evaluate_paired_log_variance_ratio(
            &var_a,
            &var_b,
            th.eps_sensitivity,
            th.sesoi_log_var_ratio,
            th.min_non_zero_pairs_r7,
            n_req,
        );
        let tail_window = evaluate_paired_log_variance_ratio(
            &tail_a,
            &tail_b,
            th.eps_primary,
            th.sesoi_log_var_ratio,
            th.min_non_zero_pairs_r7,
            n_req,
        );
        let lee = lee_trimming_bounds(&surv_valid_var_a, &surv_valid_var_b, th.eps_primary);
        let surv_diffs: Vec<f64> = surv_all_a
            .iter()
            .zip(&surv_all_b)
            .map(|(a, b)| b - a)
            .collect();
        let rmst_stats = paired_stats(&surv_all_a, &surv_all_b, n_req);
        let rmst_wilcoxon = wilcoxon_signed_rank(&surv_diffs, th.min_non_zero_pairs_r7);
        let mediator = paired_mediator_ols(&births_a, &births_b, &primary.diffs);

        raw.push((
            c,
            primary,
            sens,
            tail_window,
            lee,
            rmst_stats,
            rmst_wilcoxon,
            mediator,
            paired_stats(&s1_a, &s1_b, n_req),
            paired_stats(&s2_a, &s2_b, n_req),
            paired_stats(&s3_a, &s3_b, n_req),
        ));
    }

    let p_vec: Vec<Option<f64>> = raw.iter().map(|entry| entry.1.p_two_sided).collect();
    let holm_p = holm_adjust(&p_vec);

    let mut contrasts_map = serde_json::Map::new();
    let mut h1_decision = "insufficient_evidence";
    let mut h1_dir_tag = "attrition_below_pi85_floor";

    for (idx, entry) in raw.into_iter().enumerate() {
        let (c, primary, sens, tail_w, lee, rmst_stats, rmst_wilcox, mediator, s1, s2, s3) = entry;
        let p_holm = holm_p[idx];
        let dec = classify_e3b_decision(
            true,
            failed_runs_zero,
            primary.n_complete_pairs,
            n_req,
            th.min_pi_ll,
            th.min_non_zero_pairs_r7,
            p_holm,
            th.fwer_alpha,
            primary.mean_l,
            primary.tost_equivalent,
            th.sesoi_log_var_ratio,
            c.directional_positive,
        );
        if c.id == "H1" {
            h1_decision = dec.label;
            h1_dir_tag = dec.direction_tag;
        }
        let mut rmst_val = serde_json::to_value(&rmst_stats).map_err(|e| e.to_string())?;
        if let Some(obj) = rmst_val.as_object_mut() {
            obj.insert(
                "wilcoxon".to_string(),
                serde_json::to_value(&rmst_wilcox).map_err(|e| e.to_string())?,
            );
        }
        contrasts_map.insert(
            c.id.to_string(),
            serde_json::json!({
                "id": c.id,
                "name": c.name,
                "p_holm": p_holm,
                "decision": dec.label,
                "direction_tag": dec.direction_tag,
                "primary": primary,
                "sensitivity_eps_5e5": sens,
                "tail_window_L": tail_w,
                "lee_bounds": lee,
                "rmst": rmst_val,
                "mediator_ln_births": mediator,
                "s1_iqr": s1,
                "s2_mean": s2,
                "s3_east_minus_west": s3,
            }),
        );
    }

    let doc = serde_json::json!({
        "kind": "e3b-effects-and-decision",
        "preregistration_id": prereg.raw["preregistration_id"],
        "s6_lifespan_source": "series.csv",
        "primary_decision": h1_decision,
        "primary_direction_tag": h1_dir_tag,
        "failed_runs_count": failed_runs_count,
        "contrasts": contrasts_map,
        "arm_diagnostics": summaries.values().map(|s| serde_json::json!({
            "seed": s.seed,
            "arm": s.arm,
            "completed": s.completed,
            "extinct": s.extinct,
            "first_extinct_tick": s.first_extinct_tick,
            "singleton_final": s.singleton_final,
            "singleton_tail": s.singleton_tail,
            "empty_half": s.empty_half,
            "final_agents": s.final_agents,
            "final_predators": s.final_predators,
            "max_abs_eu_drift": s.max_abs_eu_drift,
            "births_sum": s.births_sum,
            "deaths_sum": s.deaths_sum,
            "lifespan_at_t": s.lifespan_at_t,
            "lifespan_post_warmup_pos": s.lifespan_post_warmup_pos,
            "cumulative_mean_lifespan": s.cumulative_mean_lifespan,
            "post_warmup_crop": s.post_warmup_crop,
        })).collect::<Vec<_>>(),
    });
    Ok(pretty(&doc)? + "\n")
}

struct EnsembleWriteCtx<'a> {
    out_dir: &'a Path,
    manifest_dir: &'a Path,
    prereg: &'a PreregSpec,
    manifests: &'a [(&'static str, &'a ExperimentManifest)],
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

fn write_ensemble_bundle(w: EnsembleWriteCtx<'_>) -> Result<(), String> {
    let manifests_dir = w.out_dir.join("manifests");
    std::fs::create_dir_all(&manifests_dir)
        .map_err(|e| format!("create {}: {e}", manifests_dir.display()))?;

    let mut manifest_hashes: BTreeMap<String, String> = BTreeMap::new();
    let manifest_files = [
        PREREG_FILE,
        w.prereg.control_file.as_str(),
        w.prereg.scarcity_file.as_str(),
        w.prereg.treatment_file.as_str(),
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

    let arm_fps: BTreeMap<String, u64> = w
        .manifests
        .iter()
        .map(|(arm, m)| ((*arm).to_string(), m.fingerprint()))
        .collect();
    let git_commit = resolve_git_commit(w.env);
    let git_dirty_paths = resolve_git_dirty_paths(w.env);
    let prov = serde_json::json!({
        "kind": "e3b-confirmation-provenance",
        "mode": "ensemble",
        "stopped_early": false,
        "stop_reason": serde_json::Value::Null,
        "preregistration_id": w.prereg.raw["preregistration_id"],
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
            "smoke_dir": w.smoke_ref.dir.display().to_string().replace('\\', "/"),
            "provenance_sha256": w.smoke_ref.provenance_sha256,
            "smoke_report_sha256": w.smoke_ref.smoke_report_sha256,
            "git_commit": w.smoke_ref.git_commit,
            "host_os": w.smoke_ref.host_os,
            "host_arch": w.smoke_ref.host_arch,
            "profile": w.smoke_ref.profile,
            "world_identity_observed": w.smoke_ref.world_identity,
            "wp2_passed": w.smoke_ref.wp2_passed,
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
    write_checksums(w.out_dir, &refs)?;
    Ok(())
}

fn build_runs_csv(runs: &[ArmRunRecord]) -> String {
    let mut s = String::from(
        "seed,arm,status,terrain_source,world_seed,world_gen_version,world_width,world_height,world_checksum,final_checksum,elapsed_secs\n",
    );
    for rec in runs {
        let status = match &rec.result.status {
            RunStatus::Completed => "Completed",
            RunStatus::Failed { .. } => "Failed",
        };
        s.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{:.4}\n",
            rec.seed,
            rec.arm,
            status,
            rec.terrain_source,
            rec.world_identity.seed,
            rec.world_identity.generator_version,
            rec.world_identity.width,
            rec.world_identity.height,
            rec.world_identity.checksum,
            rec.result.final_checksum,
            rec.elapsed_secs
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
            let status = match &rec.result.status {
                RunStatus::Completed => "Completed",
                RunStatus::Failed { .. } => "Failed",
            };
            serde_json::json!({
                "seed": rec.seed,
                "arm": rec.arm,
                "status": status,
                "final_checksum": rec.result.final_checksum,
                "terrain_source": rec.terrain_source,
                "world_identity": rec.world_identity,
                "observables": obs,
            })
        })
        .collect();
    Ok(pretty(&serde_json::Value::Array(arr))? + "\n")
}

// ---- `--replay` & `--self-test` ----------------------------------------------------------------

fn run_replay(args: &Args, env: &RuntimeEnv) -> Result<(), String> {
    refuse_debug(env, Mode::Replay)?;
    let _ = check_world_artifact_prerun(env)?;
    let registry = ObservableRegistry::live_default();
    let prereg = load_prereg(&args.manifest_dir)?;
    let (ctrl, scarce, treat) =
        load_and_validate_confirmation_manifests(&args.manifest_dir, &prereg, &registry)?;
    let arm = args.arm.as_deref().unwrap_or_default();
    let seed = args.seed.ok_or("--replay missing --seed")?;
    let manifest = match arm {
        "control_uniform" => &ctrl,
        "control_uniform_scarcity" => &scarce,
        "treatment_gradient" => &treat,
        other => return Err(format!("--arm must be one of {ARM_ORDER:?}, got '{other}'")),
    };
    if !manifest.seeds.contains(&seed) {
        return Err(format!("seed {seed} is not in declared execution_order"));
    }
    let timer = Instant::now();
    let res = run_manifest_seed::<LiveExperimentAdapter>(manifest, &registry, seed, None, None);
    let elapsed_secs = timer.elapsed().as_secs_f64();
    if let Some(out_dir) = &args.out {
        ensure_empty_out_dir(out_dir)?;
        std::fs::create_dir_all(out_dir)
            .map_err(|e| format!("create {}: {e}", out_dir.display()))?;
        let obs: BTreeMap<String, f64> = res.final_observables.iter().cloned().collect();
        let doc = serde_json::json!({
            "kind": "e3b-replay-record",
            "arm": arm,
            "seed": seed,
            "final_checksum": res.final_checksum,
            "terrain_source": res.provenance.terrain_source,
            "world_identity": res.provenance.world_identity,
            "elapsed_secs": elapsed_secs,
            "observables": obs,
        });
        let fname = format!("replay-{arm}-seed-{seed}.json");
        write_text(&out_dir.join(&fname), &(pretty(&doc)? + "\n"))?;
        write_checksums(out_dir, &[fname.as_str()])?;
    }
    println!(
        "E3b replay {arm} seed {seed}: checksum {} in {elapsed_secs:.2}s",
        res.final_checksum
    );
    Ok(())
}

fn run_self_test(_env: &RuntimeEnv) -> Result<(), String> {
    let t63 = student_t_inv(0.975, 63);
    if (t63 - 1.99834054).abs() > 1e-5 {
        return Err(format!("student_t_inv(0.975, 63) mismatch: {t63}"));
    }
    let p_holm = holm_adjust(&[Some(0.01), Some(0.04), Some(0.03)]);
    if (p_holm[0].unwrap() - 0.03).abs() > 1e-12
        || (p_holm[2].unwrap() - 0.06).abs() > 1e-12
        || (p_holm[1].unwrap() - 0.06).abs() > 1e-12
    {
        return Err("holm_adjust mismatch".into());
    }
    let prereg_dir = resolve_default_manifest_dir();
    let registry = ObservableRegistry::live_default();
    let prereg = load_prereg(&prereg_dir)?;
    let _ = load_and_validate_confirmation_manifests(&prereg_dir, &prereg, &registry)?;
    let _ = load_and_validate_smoke_manifests(&prereg_dir, &prereg, &registry)?;
    println!("run_e3b_experiment --self-test PASSED");
    Ok(())
}

// ---- Low-level utilities -----------------------------------------------------------------------

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
