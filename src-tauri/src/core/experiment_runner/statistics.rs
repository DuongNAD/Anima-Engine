//! The statistics behind ensemble summaries and effect sizes: Student-*t* critical values, paired
//! and independent effects, and the finite-sample guards that keep every aggregate JSON-safe.

use super::*;

/// Two-sided 95% Student-*t* critical value, selected conservatively.
///
/// The exact NIST 0.975 quantiles are used for integer degrees of freedom 1–30, which covers the
/// experiment contract's normal 5–30-seed range. For fractional Welch degrees of freedom, flooring
/// the value selects a wider interval than rounding upward. Above 30, the next-lowest tabulated
/// anchor is retained; this stays deterministic and conservative without adding a statistical
/// distribution dependency to the simulation runtime.
pub(super) fn conservative_student_t_critical_95(df: f64) -> f64 {
    const T_975_DF_1_TO_30: [f64; 30] = [
        12.706_204_736_432_095,
        4.302_652_729_696_142,
        3.182_446_305_284_264,
        2.776_445_105_197_798_7,
        2.570_581_835_636_314,
        2.446_911_848_791_681,
        2.364_624_251_010_299_3,
        2.306_004_135_204_166,
        2.262_157_162_854_099_3,
        2.228_138_851_964_938_5,
        2.200_985_160_082_949,
        2.178_812_829_663_417_7,
        2.160_368_656_461_012_7,
        2.144_786_687_916_927_3,
        2.131_449_545_559_323,
        2.119_905_299_221_011_2,
        2.109_815_577_833_181,
        2.100_922_040_240_96,
        2.093_024_054_408_263,
        2.085_963_447_265_836_4,
        2.079_613_844_727_662,
        2.073_873_067_904_015,
        2.068_657_610_419_041,
        2.063_898_561_628_021,
        2.059_538_552_753_294,
        2.055_529_438_642_871,
        2.051_830_516_480_289,
        2.048_407_141_795_244,
        2.045_229_642_132_703,
        2.042_272_456_301_238,
    ];

    if !df.is_finite() || df < 1.0 {
        return f64::NAN;
    }
    let lower_df = df.floor();
    if lower_df <= 30.0 {
        return T_975_DF_1_TO_30[lower_df as usize - 1];
    }
    if lower_df < 40.0 {
        return T_975_DF_1_TO_30[29];
    }
    if lower_df < 60.0 {
        return 2.021_075_390_306_273_3;
    }
    if lower_df < 80.0 {
        return 2.000_297_821_058_262;
    }
    if lower_df < 100.0 {
        return 1.990_063_421_071_4;
    }
    if lower_df < 120.0 {
        return 1.983_971_518_449_633_4;
    }
    1.979_930_405_052_777
}

/// Build a [`PairedEffect`], honouring the defined-ness contract documented on that type.
pub fn paired_effect(
    name: &str,
    n_requested: usize,
    control: &[f64],
    treatment: &[f64],
    deltas: &[f64],
) -> Result<PairedEffect, ExperimentError> {
    let n = deltas.len();
    let mut e = PairedEffect {
        observable: name.to_string(),
        n_requested,
        n_complete_pairs: n,
        control_mean: None,
        treatment_mean: None,
        paired_mean_delta: None,
        paired_sd: None,
        paired_se: None,
        ci95_low: None,
        ci95_high: None,
        ci95_method: None,
        paired_dz: None,
    };
    if n == 0 {
        return Ok(e);
    }
    let nf = n as f64;
    let cm = finite_sample_stats(name, control)?.mean;
    let tm = finite_sample_stats(name, treatment)?.mean;
    let delta_stats = finite_sample_stats(name, deltas)?;
    let dm = delta_stats.mean;
    e.control_mean = Some(cm);
    e.treatment_mean = Some(tm);
    e.paired_mean_delta = Some(dm);
    if n < 2 {
        // One pair: the delta is known, but no spread is estimable.
        return Ok(e);
    }
    let sd = delta_stats.sample_sd.expect("n >= 2 has a sample SD");
    let se = require_finite_aggregate(name, "paired standard error", sd / nf.sqrt())?;
    let critical = require_finite_aggregate(
        name,
        "paired 95% Student-t critical value",
        conservative_student_t_critical_95(nf - 1.0),
    )?;
    let half = require_finite_aggregate(name, "paired 95% confidence half-width", critical * se)?;
    e.paired_sd = Some(sd);
    e.paired_se = Some(se);
    e.ci95_low = Some(require_finite_aggregate(
        name,
        "paired 95% confidence lower bound",
        dm - half,
    )?);
    e.ci95_high = Some(require_finite_aggregate(
        name,
        "paired 95% confidence upper bound",
        dm + half,
    )?);
    e.ci95_method = Some(ConfidenceIntervalMethod::PairedStudentTConservativeTable);
    // Zero paired variance ⇒ d_z is undefined (division by zero), never ±inf.
    if sd > 0.0 {
        e.paired_dz = Some(require_finite_aggregate(
            name,
            "paired Cohen's dz",
            dm / sd,
        )?);
    }
    Ok(e)
}

// ---- Independent-sample effect size (descriptive helper; NOT the AE-S14 gate) ------------------

/// The comparative effect of a treatment ensemble against a control ensemble, for one observable.
///
/// Reports the raw **mean difference** (treatment − control, in the observable's own unit), a
/// **standardized** effect size (Hedges' *g* — Cohen's *d* with the small-sample correction), and a
/// 95% Welch–Student-*t* interval on the mean difference. Sample counts are the *completed* runs on
/// each side; failed runs are never silently folded in (they are listed on
/// [`EnsembleComparison`]).
///
/// This is a descriptive statistic over a seed ensemble, **not** a significance test and **not**
/// evidence of adaptation or selection — AE1–AE2.5 contain no pathway or reproduction, so no
/// evolutionary claim may be drawn from it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectSize {
    pub observable: String,
    pub n_control: usize,
    pub n_treatment: usize,
    pub control_mean: f64,
    pub treatment_mean: f64,
    /// `treatment_mean − control_mean`, in the observable's unit.
    pub mean_difference: f64,
    /// Pooled standard deviation; `None` unless both groups contain at least two samples.
    pub pooled_sd: Option<f64>,
    /// Hedges' *g*: the mean difference in pooled-SD units, small-sample corrected. `None` when
    /// either group has fewer than two samples or the pooled SD is zero.
    pub hedges_g: Option<f64>,
    /// 95% Welch–Student-*t* interval on the **mean difference**.
    pub ci95_low: Option<f64>,
    pub ci95_high: Option<f64>,
    /// Procedure used for the interval; `None` in legacy or non-estimable artifacts.
    pub ci95_method: Option<ConfidenceIntervalMethod>,
}

/// A control-vs-treatment ensemble comparison: per-observable effect sizes over the observables both
/// sides emitted, the preserved failures from each side, and the observables that exist on only one
/// side (reported explicitly rather than compared against a fabricated zero).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnsembleComparison {
    pub control_experiment_id: String,
    pub treatment_experiment_id: String,
    pub control_manifest_fingerprint: u64,
    pub treatment_manifest_fingerprint: u64,
    /// Completed-run counts (the N behind every effect size).
    pub control_n: usize,
    pub treatment_n: usize,
    /// `(seed, reason)` for every failed run — preserved, never dropped (ER11).
    pub control_failed: Vec<(u64, String)>,
    pub treatment_failed: Vec<(u64, String)>,
    pub effects: Vec<EffectSize>,
    /// Observables emitted only by the control side.
    pub control_only: Vec<String>,
    /// Observables emitted only by the treatment side.
    pub treatment_only: Vec<String>,
}

impl EnsembleComparison {
    /// The effect size for a named observable, if both sides emitted it.
    pub fn effect_of(&self, observable: &str) -> Option<&EffectSize> {
        self.effects.iter().find(|e| e.observable == observable)
    }
}

/// Compare two **already-run, independent** ensembles descriptively.
///
/// **This is NOT the AE-S14 gate.** It treats the two ensembles as independent samples (pooled
/// Hedges' *g*, independent-sample interval) and does **not** require — or verify — that the two
/// sides used the same seeds, the same seed order, or a validated control/treatment factor diff. Two
/// ensembles compared this way may differ in uncontrolled ways, so the result is descriptive only.
///
/// For the same-seed causal control/treatment design the gate requires, use
/// [`run_paired_ensemble`] and [`PairedEnsembleReport`], which derives and validates the control
/// variant, guarantees the identical ordered seed set by construction, and reports **paired** deltas.
///
/// Only **completed** runs contribute samples; failures from both sides are carried into the report.
/// An observable present on only one side is listed in `control_only` / `treatment_only` and is *not*
/// given a fabricated zero-valued counterpart.
pub fn compare_ensembles(
    control: &EnsembleSummary,
    treatment: &EnsembleSummary,
) -> Result<EnsembleComparison, ExperimentError> {
    let samples = |e: &EnsembleSummary, name: &str| -> Vec<f64> {
        e.runs
            .iter()
            .filter(|r| r.status.is_completed())
            .filter_map(|r| r.observable(name))
            .collect()
    };
    let names = |e: &EnsembleSummary| -> Vec<String> {
        let mut seen = std::collections::BTreeSet::new();
        for r in e.runs.iter().filter(|r| r.status.is_completed()) {
            for (k, _) in &r.final_observables {
                seen.insert(k.clone());
            }
        }
        seen.into_iter().collect()
    };

    let c_names = names(control);
    let t_names = names(treatment);
    let mut effects = Vec::new();
    for name in &c_names {
        if !t_names.contains(name) {
            continue;
        }
        let cs = samples(control, name);
        let ts = samples(treatment, name);
        if cs.is_empty() || ts.is_empty() {
            continue;
        }
        effects.push(effect_size(name, &cs, &ts)?);
    }
    let control_only: Vec<String> = c_names
        .iter()
        .filter(|n| !t_names.contains(*n))
        .cloned()
        .collect();
    let treatment_only: Vec<String> = t_names
        .iter()
        .filter(|n| !c_names.contains(*n))
        .cloned()
        .collect();

    Ok(EnsembleComparison {
        control_experiment_id: control.experiment_id.clone(),
        treatment_experiment_id: treatment.experiment_id.clone(),
        control_manifest_fingerprint: control.manifest_fingerprint,
        treatment_manifest_fingerprint: treatment.manifest_fingerprint,
        control_n: control.completed_runs,
        treatment_n: treatment.completed_runs,
        control_failed: control.failed.clone(),
        treatment_failed: treatment.failed.clone(),
        effects,
        control_only,
        treatment_only,
    })
}

pub(super) fn effect_size(
    name: &str,
    control: &[f64],
    treatment: &[f64],
) -> Result<EffectSize, ExperimentError> {
    let control_stats = finite_sample_stats(name, control)?;
    let treatment_stats = finite_sample_stats(name, treatment)?;
    let cm = control_stats.mean;
    let tm = treatment_stats.mean;
    let control_sd = control_stats.sample_sd.unwrap_or(0.0);
    let treatment_sd = treatment_stats.sample_sd.unwrap_or(0.0);
    let nc = control.len() as f64;
    let nt = treatment.len() as f64;
    let diff = require_finite_aggregate(name, "independent mean difference", tm - cm)?;

    // Pooled SD (Cohen). Both groups need their own estimable sample variance; treating an absent
    // variance as zero would falsely make a singleton arm look perfectly measured.
    let can_estimate_spread = control.len() >= 2 && treatment.len() >= 2;
    let df = (nc + nt - 2.0).max(1.0);
    let pooled_sd = if can_estimate_spread {
        let sd_scale = control_sd.max(treatment_sd);
        let value = if sd_scale > 0.0 {
            let control_scaled = control_sd / sd_scale;
            let treatment_scaled = treatment_sd / sd_scale;
            let pooled_scaled = (((nc - 1.0) * control_scaled * control_scaled
                + (nt - 1.0) * treatment_scaled * treatment_scaled)
                / df)
                .max(0.0)
                .sqrt();
            require_finite_aggregate(name, "pooled standard deviation", pooled_scaled * sd_scale)?
        } else {
            0.0
        };
        Some(value)
    } else {
        None
    };

    // Hedges' g = Cohen's d × small-sample correction J ≈ 1 − 3/(4·df − 1).
    let hedges_g = match pooled_sd {
        Some(sd) if sd > 0.0 => {
            let d = diff / sd;
            let j = 1.0 - 3.0 / (4.0 * df - 1.0);
            Some(require_finite_aggregate(name, "Hedges' g", d * j)?)
        }
        _ => None,
    };

    // 95% Welch-Student-t interval on the mean difference (independent samples). Scaling both
    // standard deviations before calculating SE and Satterthwaite df avoids overflow for otherwise
    // finite extreme-valued experiments.
    let (ci95_low, ci95_high) = if can_estimate_spread {
        let sd_scale = control_sd.max(treatment_sd);
        let half = if sd_scale > 0.0 {
            let control_scaled = control_sd / sd_scale;
            let treatment_scaled = treatment_sd / sd_scale;
            let control_se_variance = control_scaled * control_scaled / nc;
            let treatment_se_variance = treatment_scaled * treatment_scaled / nt;
            let se_scaled = control_se_variance
                .max(0.0)
                .sqrt()
                .hypot(treatment_se_variance.max(0.0).sqrt());
            let se =
                require_finite_aggregate(name, "independent standard error", se_scaled * sd_scale)?;
            let numerator = (control_se_variance + treatment_se_variance).powi(2);
            let denominator = control_se_variance.powi(2) / (nc - 1.0)
                + treatment_se_variance.powi(2) / (nt - 1.0);
            let welch_df = require_finite_aggregate(
                name,
                "Welch-Satterthwaite degrees of freedom",
                numerator / denominator,
            )?;
            let critical = require_finite_aggregate(
                name,
                "independent 95% Student-t critical value",
                conservative_student_t_critical_95(welch_df),
            )?;
            require_finite_aggregate(name, "independent 95% confidence half-width", critical * se)?
        } else {
            0.0
        };
        (
            Some(require_finite_aggregate(
                name,
                "independent 95% confidence lower bound",
                diff - half,
            )?),
            Some(require_finite_aggregate(
                name,
                "independent 95% confidence upper bound",
                diff + half,
            )?),
        )
    } else {
        (None, None)
    };
    Ok(EffectSize {
        observable: name.to_string(),
        n_control: control.len(),
        n_treatment: treatment.len(),
        control_mean: cm,
        treatment_mean: tm,
        mean_difference: diff,
        pooled_sd,
        hedges_g,
        ci95_low,
        ci95_high,
        ci95_method: can_estimate_spread
            .then_some(ConfidenceIntervalMethod::WelchStudentTConservativeTable),
    })
}

#[derive(Clone, Copy, Debug)]
pub(super) struct FiniteSampleStats {
    mean: f64,
    sample_sd: Option<f64>,
}

pub(super) fn aggregate_not_finite(observable: &str, statistic: &str) -> ExperimentError {
    ExperimentError::AggregateNotFinite {
        observable: observable.to_owned(),
        statistic: statistic.to_owned(),
    }
}

pub(super) fn require_finite_aggregate(
    observable: &str,
    statistic: &str,
    value: f64,
) -> Result<f64, ExperimentError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| aggregate_not_finite(observable, statistic))
}

/// Compute a finite mean and sample SD without overflowing intermediate sums.
///
/// Scaling by a power of two is exact for normal values and bounds every normalized sample below
/// 2.0 in magnitude. The online mean therefore cannot overflow, and the second pass uses compensated
/// accumulation for squared deviations. A genuinely unrepresentable final statistic is reported as
/// structured data instead of being allowed to reach JSON as `null`.
pub(super) fn finite_sample_stats(
    observable: &str,
    samples: &[f64],
) -> Result<FiniteSampleStats, ExperimentError> {
    debug_assert!(!samples.is_empty());
    if samples.is_empty() {
        return Err(aggregate_not_finite(observable, "empty sample"));
    }
    if samples.iter().any(|value| !value.is_finite()) {
        return Err(aggregate_not_finite(observable, "input sample"));
    }

    let max_abs = samples
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    if max_abs == 0.0 {
        return Ok(FiniteSampleStats {
            mean: 0.0,
            sample_sd: (samples.len() > 1).then_some(0.0),
        });
    }

    const EXPONENT_MASK: u64 = 0x7ff0_0000_0000_0000;
    let exponent = max_abs.to_bits() & EXPONENT_MASK;
    let scale = if exponent == 0 {
        f64::MIN_POSITIVE
    } else {
        f64::from_bits(exponent)
    };

    let mut mean_scaled = 0.0;
    for (index, value) in samples.iter().enumerate() {
        let normalized = value / scale;
        mean_scaled += (normalized - mean_scaled) / (index + 1) as f64;
    }
    let mean = require_finite_aggregate(observable, "mean", mean_scaled * scale)?;

    if samples.len() == 1 {
        return Ok(FiniteSampleStats {
            mean,
            sample_sd: None,
        });
    }

    let mut squared_sum = 0.0;
    let mut correction = 0.0;
    for value in samples {
        let deviation = value / scale - mean_scaled;
        let term = deviation * deviation;
        let adjusted = term - correction;
        let updated = squared_sum + adjusted;
        correction = (updated - squared_sum) - adjusted;
        squared_sum = updated;
    }
    let variance_scaled = require_finite_aggregate(
        observable,
        "sample variance",
        (squared_sum / (samples.len() - 1) as f64).max(0.0),
    )?;
    let sample_sd = require_finite_aggregate(
        observable,
        "sample standard deviation",
        variance_scaled.sqrt() * scale,
    )?;

    Ok(FiniteSampleStats {
        mean,
        sample_sd: Some(sample_sd),
    })
}

/// Summarize the deterministic union of observables across completed runs.
///
/// Names retain first-appearance order across the manifest's seed order. A model may legitimately
/// emit a seed-dependent final observable, so using only the first completed run as the name source
/// would silently discard later evidence.
pub(super) fn summarize_metrics(
    completed: &[&RunResult],
) -> Result<Vec<MetricSummary>, ExperimentError> {
    let mut seen = std::collections::BTreeSet::new();
    let mut names = Vec::new();
    for run in completed {
        for (name, _) in &run.final_observables {
            if seen.insert(name.as_str()) {
                names.push(name.as_str());
            }
        }
    }

    let mut out = Vec::new();
    for name in names {
        let samples: Vec<f64> = completed
            .iter()
            .filter_map(|r| r.observable(name))
            .collect();
        let n = samples.len();
        if n == 0 {
            continue;
        }
        let stats = finite_sample_stats(name, &samples)?;
        let mean = stats.mean;
        let std = stats.sample_sd;
        let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let (ci95_low, ci95_high) = if let Some(std) = std {
            let critical = require_finite_aggregate(
                name,
                "95% Student-t critical value",
                conservative_student_t_critical_95((n - 1) as f64),
            )?;
            let half = require_finite_aggregate(
                name,
                "95% confidence half-width",
                std / (n as f64).sqrt() * critical,
            )?;
            (
                Some(require_finite_aggregate(
                    name,
                    "95% confidence lower bound",
                    mean - half,
                )?),
                Some(require_finite_aggregate(
                    name,
                    "95% confidence upper bound",
                    mean + half,
                )?),
            )
        } else {
            (None, None)
        };
        let ci95_method = std.map(|_| ConfidenceIntervalMethod::StudentTConservativeTable);
        out.push(MetricSummary {
            observable: name.to_owned(),
            n,
            mean,
            std,
            min,
            max,
            ci95_low,
            ci95_high,
            ci95_method,
        });
    }
    Ok(out)
}
