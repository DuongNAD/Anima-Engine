//! The E-SEL1 runner: does food scarcity shift the live population's mean body mass?
//!
//! The binary behind precondition P3 of the E-SEL1 draft
//! (`docs/ai/planning/2026-11-16-experiment-e-sel1-food-scarcity-body-mass.md`, on its own branch).
//! Modelled on `run_e2_brain_experiment`: it loads committed manifests rather than rebuilding them,
//! drives the shared [`experiment_runner`](anima_engine_lib::core::experiment_runner) over
//! [`LiveExperimentAdapter`], and writes the E2 artifact set. It does **not** fork the statistics:
//! the per-seed paired statistic is `experiment_runner::paired_effect` (the function the shared
//! paired ensemble itself uses), applied to the tail-window mean the preregistration names.
//!
//! # Build it, then run the binary — never `cargo run`
//!
//! ```text
//! cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop \
//!   --example run_esel1_experiment
//!
//! src-tauri/target/release/examples/run_esel1_experiment --smoke \
//!   --manifest-dir src-tauri/tests/fixtures/experiments_esel1 \
//!   --out artifacts/experiments/e-sel1-food-scarcity
//! ```
//!
//! No Tauri handle, window, GPU, learner or evolution thread exists in this process; the status of
//! the live world stays **headless adapter verified**.
//!
//! # Modes
//!
//! | mode | runs | writes | needs |
//! |---|---|---|---|
//! | `--smoke` | the smoke seed only, all arms of the smoke manifests | `<out>/smoke/` | nothing |
//! | `--ensemble` | the preregistered seeds, all arms | `<out>/` | `<out>/execution-lock.json` |
//! | `--replay` | one seed of one arm | `<out>/replay/` | nothing |
//!
//! `execution-lock.json` is written **by hand** after the smoke (draft section 7). `--ensemble`
//! refuses to start without it, refuses a lock that lacks `duration_ticks`, `N0`, `F_hi` or `F_lo`,
//! and refuses one that disagrees with the effective duration or with the founders and food caps of
//! the manifests in `--manifest-dir`, so nothing can drift after the lock. The lock is hashed into
//! `checksums.sha256` and its sha256 is recorded in `provenance.json`. Seeds are bound to the
//! preregistration: an ensemble whose manifests declare anything but the registered execution order
//! (or the smoke seed) is refused before a tick runs, and `--replay` refuses a seed its manifest does
//! not declare.
//!
//! # The statistic and the extinction rule
//!
//! H1 compares `live.trait_mass_mean`, averaged per run over the samples with `tick > 0.8 * T`
//! (the last 20% of samples), as **treatment minus control** (scarce minus
//! abundant). A run whose `live.agent_count` is 0 at any sample, or finally, is *extinct*. The pair
//! containing it is dropped from every paired statistic and **counted** (seed, arm, tick) in
//! `effects.json`, `paired-report.json` stays complete, and `runs.csv` lists every run. A failed run
//! drops its pair the same way and is listed too. Nothing is dropped silently.
//!
//! The optional third arm (manifest key `manifests.drift_control_optional_requires_P4`) is the
//! treatment with `live.mutation_rate = 0`; it is compared with the treatment as H2 (descriptive).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anima_engine_lib::core::experiment::{ExperimentManifest, FactorDiff, ObservableRegistry};
use anima_engine_lib::core::experiment_runner::{
    paired_effect, run_manifest_seed, run_paired_ensemble_with_control, PairedEffect,
    PairedEnsembleReport, RunResult, RunStatus,
};
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LIVE_EXTENDED_OBSERVABLE_IDS, LIVE_MODEL_VERSION, LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::sha256::sha256_file;

/// Every observable id the live registry knows, legacy then extended.
fn live_all_ids() -> impl Iterator<Item = &'static str> {
    LIVE_OBSERVABLE_IDS
        .iter()
        .chain(LIVE_EXTENDED_OBSERVABLE_IDS.iter())
        .copied()
}

const PREREG_FILE: &str = "esel1-preregistration.json";
const LOCK_FILE: &str = "execution-lock.json";
const ALLOWED_FACTOR_PATH: &str = "initial_conditions";
const FOOD_KEY: &str = "live.food_cap";
const FOUNDERS_KEY: &str = "live.founders";
const MUTATION_KEY: &str = "live.mutation_rate";

const PRIMARY_OBSERVABLE: &str = "live.trait_mass_mean";
const COUNT_OBSERVABLE: &str = "live.agent_count";
const INTEGRITY_OBSERVABLE: &str = "live.closed_eu_total";
const BIRTHS_OBSERVABLE: &str = "live.births";
/// The name under which the tail-window statistic is reported.
const TAIL_STAT: &str = "live.trait_mass_mean@tail20";
const DESCRIPTIVE_OBSERVABLES: [&str; 9] = [
    "live.agent_count",
    "live.standing_crop",
    "live.food_items",
    "live.births",
    "live.deaths",
    "live.mean_lifespan",
    "live.trait_mass_var",
    "live.trait_mass_p10",
    "live.trait_mass_p90",
];

/// The only observables a smoke run may emit or write. The first two are what the smoke algorithm
/// consults; the rest are harness integrity and descriptive counts. No `live.trait_mass_*` value, and
/// no effect computed from one, may reach any smoke output: the rule is enforced here, not by
/// discipline.
const SMOKE_ALLOWED_OBSERVABLES: [&str; 5] = [
    "live.agent_count",
    "live.standing_crop",
    "live.closed_eu_total",
    "live.births",
    "live.deaths",
];
/// Step A of the smoke algorithm reads samples with `tick <= SMOKE_STEP_A_TICKS`.
const SMOKE_STEP_A_TICKS: u64 = 1200;

const MATERIALITY_DZ: f64 = 0.8;
const MATERIALITY_RELATIVE_SHIFT: f64 = 0.05;
/// Relative spread of `live.closed_eu_total` within one run above which it is filed as a defect.
const CLOSED_EU_TOLERANCE: f64 = 1e-6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Smoke,
    Ensemble,
    Replay,
}

struct Args {
    mode: Mode,
    manifest_dir: PathBuf,
    out: PathBuf,
    duration_rung: Option<u64>,
    replay_seed: Option<u64>,
    replay_arm: Option<String>,
}

// Unused when this file is `#[path]`-included by a test; that is the only reason for the allow.
#[allow(dead_code)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run_with_args(args) {
        eprintln!("E-SEL1 runner refused to proceed: {e}");
        std::process::exit(2);
    }
}

/// The whole program, taking its arguments explicitly so a test can drive it in-process.
pub fn run_with_args(argv: Vec<String>) -> Result<(), String> {
    let args = parse_args(argv)?;
    let registry = ObservableRegistry::live_default();
    registry
        .validate()
        .map_err(|e| format!("the live registry is invalid: {e}"))?;

    let prereg_path = args.manifest_dir.join(PREREG_FILE);
    let prereg = read_json(&prereg_path)?;
    let plan = Plan::from_prereg(&prereg)?;

    let smoke = args.mode == Mode::Smoke;
    let (control_name, treatment_name, drift_name) = if smoke {
        (
            plan.smoke_control_file.clone(),
            plan.smoke_treatment_file.clone(),
            plan.smoke_drift_file.clone(),
        )
    } else {
        (
            plan.control_file.clone(),
            plan.treatment_file.clone(),
            plan.drift_file.clone(),
        )
    };
    let mut control = load_manifest(&args.manifest_dir.join(&control_name))?;
    let mut treatment = load_manifest(&args.manifest_dir.join(&treatment_name))?;
    let mut drift = match &drift_name {
        Some(n) => Some(load_manifest(&args.manifest_dir.join(n))?),
        None => None,
    };

    let committed_duration = control.duration_ticks;
    let mut manifest_hashes: BTreeMap<String, String> = BTreeMap::new();
    let mut file_names: Vec<String> = vec![control_name.clone(), treatment_name.clone()];
    if let Some(n) = &drift_name {
        file_names.push(n.clone());
    }
    file_names.push(PREREG_FILE.to_string());
    for n in &file_names {
        manifest_hashes.insert(
            n.clone(),
            sha256_file(&args.manifest_dir.join(n)).map_err(|e| format!("hash {n}: {e}"))?,
        );
    }

    plan.refuse_undeclared_seeds(args.mode, &control, &treatment, drift.as_ref())?;
    check_factors(&control, &treatment, drift.as_ref())?;

    let rung = match args.duration_rung {
        Some(t) => {
            if !plan.ladder.contains(&t) {
                return Err(format!(
                    "--duration-rung {t} is not on the preregistered ladder {:?}",
                    plan.ladder
                ));
            }
            control.duration_ticks = t;
            treatment.duration_ticks = t;
            if let Some(d) = drift.as_mut() {
                d.duration_ticks = t;
            }
            t
        }
        None => control.duration_ticks,
    };
    for m in [Some(&control), Some(&treatment), drift.as_ref()]
        .into_iter()
        .flatten()
    {
        if m.duration_ticks != rung {
            return Err(format!(
                "arm '{}' has duration {} but the run uses {rung}; arms must share one duration",
                m.experiment_id, m.duration_ticks
            ));
        }
        if smoke {
            // The smoke consults only the allow-listed observables (draft section 6).
            if !SMOKE_ALLOWED_OBSERVABLES
                .iter()
                .take(2)
                .all(|o| m.observable_ids.iter().any(|x| x == o))
            {
                return Err(format!(
                    "smoke arm '{}' must request live.agent_count and live.standing_crop",
                    m.experiment_id
                ));
            }
            continue;
        }
        if !m.observable_ids.iter().any(|o| o == PRIMARY_OBSERVABLE)
            || !m.observable_ids.iter().any(|o| o == COUNT_OBSERVABLE)
        {
            return Err(format!(
                "arm '{}' must request {PRIMARY_OBSERVABLE} and {COUNT_OBSERVABLE}",
                m.experiment_id
            ));
        }
        if tail_samples_possible(rung, m.sample_period) == 0 {
            return Err(format!(
                "arm '{}': no sample falls in the tail window (tick > 0.8 * {rung}) at sample \
                 period {}; the primary statistic would be undefined",
                m.experiment_id, m.sample_period
            ));
        }
    }

    // The execution lock gates the ensemble (draft section 7): written by hand after the smoke.
    let lock = if args.mode == Mode::Ensemble {
        let lock_path = args.out.join(LOCK_FILE);
        if !lock_path.is_file() {
            return Err(format!(
                "--ensemble needs {} (written by hand after the smoke, draft section 7); it is \
                 absent, so the duration and arm parameters were never locked",
                lock_path.display()
            ));
        }
        let lock = read_json(&lock_path)?;
        validate_lock(&lock, rung, &control, &treatment, drift.as_ref())?;
        let digest = sha256_file(&lock_path).map_err(|e| format!("hash {LOCK_FILE}: {e}"))?;
        Some((lock, digest))
    } else {
        None
    };
    let (lock, lock_sha256) = match lock {
        Some((l, d)) => (Some(l), Some(d)),
        None => (None, None),
    };

    let out_dir = match args.mode {
        Mode::Smoke => args.out.join("smoke"),
        Mode::Ensemble => args.out.clone(),
        Mode::Replay => args.out.join("replay"),
    };
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;

    if args.mode == Mode::Replay {
        return replay(
            &args,
            &control,
            &treatment,
            drift.as_ref(),
            &registry,
            &out_dir,
            rung,
        );
    }

    if smoke {
        return smoke_run(
            &control,
            &treatment,
            drift.as_ref(),
            &registry,
            &out_dir,
            &file_names,
            &manifest_hashes,
            &plan,
            rung,
        );
    }

    let started = SystemTime::now();
    let clock = Instant::now();
    let report = run_paired_ensemble_with_control::<LiveExperimentAdapter>(
        &control,
        &treatment,
        &registry,
        &FactorDiff {
            allowed_paths: vec![ALLOWED_FACTOR_PATH.to_string()],
        },
    )
    .map_err(|e| format!("the paired ensemble refused to run: {e}"))?;
    let drift_runs: Option<Vec<RunResult>> = drift.as_ref().map(|d| {
        d.seeds
            .iter()
            .map(|&s| run_manifest_seed::<LiveExperimentAdapter>(d, &registry, s, None, None))
            .collect()
    });
    let elapsed = clock.elapsed().as_secs_f64();
    let ended = SystemTime::now();

    let analysis = analyse(&report, drift_runs.as_deref(), &control)?;
    println!(
        "E-SEL1 {:?}: {} seeds x {} arms x {rung} ticks in {:.1}s; complete pairs {} of {} \
         ({} extinct, {} failed, dropped)",
        args.mode,
        control.seeds.len(),
        if drift.is_some() { 3 } else { 2 },
        elapsed,
        analysis.h1_pairs_used,
        report.pairs.len(),
        analysis.dropped_extinct.len(),
        analysis.dropped_failed.len(),
    );

    write_artifacts(&WriteInput {
        mode: args.mode,
        out_dir: &out_dir,
        manifest_dir: &args.manifest_dir,
        file_names: &file_names,
        manifest_hashes: &manifest_hashes,
        control: &control,
        treatment: &treatment,
        drift: drift.as_ref(),
        committed_duration,
        plan: &plan,
        rung,
        report: &report,
        drift_runs: drift_runs.as_deref(),
        analysis: &analysis,
        lock: lock.as_ref(),
        lock_sha256: lock_sha256.as_deref(),
        registry: &registry,
        started,
        ended,
        elapsed_secs: elapsed,
    })?;
    println!("  artifacts written to {}", out_dir.display());
    Ok(())
}

// ---- The smoke: allow-listed outputs only ------------------------------------------------------

/// What the smoke algorithm consults, per run. Nothing here is derived from body mass.
struct SmokeRun {
    arm: &'static str,
    seed: u64,
    status: RunStatus,
    checksum: u32,
    elapsed_secs: f64,
    /// `(tick, [value of each SMOKE_ALLOWED_OBSERVABLES])`.
    series: Vec<(u64, [Option<f64>; 5])>,
}

impl SmokeRun {
    fn column(&self, i: usize) -> impl Iterator<Item = (u64, f64)> + '_ {
        self.series
            .iter()
            .filter_map(move |(t, v)| v[i].map(|x| (*t, x)))
    }
    /// Step A: max live.agent_count over samples with tick <= 1200.
    fn max_agents_step_a(&self) -> Option<f64> {
        self.column(0)
            .filter(|(t, _)| *t <= SMOKE_STEP_A_TICKS)
            .map(|(_, v)| v)
            .reduce(f64::max)
    }
    /// Steps B and C: min live.agent_count over all samples.
    fn min_agents(&self) -> Option<f64> {
        self.column(0).map(|(_, v)| v).reduce(f64::min)
    }
    fn max_agents(&self) -> Option<f64> {
        self.column(0).map(|(_, v)| v).reduce(f64::max)
    }
    /// Step C: mean live.standing_crop over samples with tick > T/2.
    fn standing_crop_tail_mean(&self, duration: u64) -> Option<f64> {
        let xs: Vec<f64> = self
            .column(1)
            .filter(|(t, _)| *t * 2 > duration)
            .map(|(_, v)| v)
            .collect();
        if xs.is_empty() {
            None
        } else {
            Some(xs.iter().sum::<f64>() / xs.len() as f64)
        }
    }
    fn extinct_tick(&self) -> Option<u64> {
        self.column(0).find(|(_, v)| *v == 0.0).map(|(t, _)| t)
    }
    fn us_per_tick(&self, duration: u64) -> f64 {
        self.elapsed_secs * 1e6 / duration as f64
    }
}

/// Refuse to emit text that names a body-mass observable in a smoke run.
fn guard_no_body_mass(what: &str, text: &str) -> Result<(), String> {
    if text.contains("trait_mass") {
        Err(format!(
            "the smoke may not write a body-mass observable, but {what} would"
        ))
    } else {
        Ok(())
    }
}

/// Smoke stdout: the same guard as for files.
fn say(line: &str) -> Result<(), String> {
    guard_no_body_mass("stdout", line)?;
    println!("{line}");
    Ok(())
}

/// A failure reason is free text from the model; withhold it if it would name a body-mass observable.
fn scrub(text: &str) -> String {
    if text.contains("trait_mass") {
        "reason withheld in the smoke (it names a body-mass observable)".to_string()
    } else {
        csv_field(text)
    }
}

#[allow(clippy::too_many_arguments)]
fn smoke_run(
    control: &ExperimentManifest,
    treatment: &ExperimentManifest,
    drift: Option<&ExperimentManifest>,
    registry: &ObservableRegistry,
    out_dir: &Path,
    file_names: &[String],
    manifest_hashes: &BTreeMap<String, String>,
    plan: &Plan,
    rung: u64,
) -> Result<(), String> {
    let started = SystemTime::now();
    let mut runs: Vec<SmokeRun> = Vec::new();
    let arms: [(&'static str, Option<&ExperimentManifest>); 3] = [
        ("control", Some(control)),
        ("treatment", Some(treatment)),
        ("drift", drift),
    ];
    for (arm, manifest) in arms {
        let Some(m) = manifest else { continue };
        // Never even request a body-mass observable in the smoke.
        let mut restricted = m.clone();
        restricted
            .observable_ids
            .retain(|o| SMOKE_ALLOWED_OBSERVABLES.contains(&o.as_str()));
        for &seed in &m.seeds {
            let clock = Instant::now();
            let run =
                run_manifest_seed::<LiveExperimentAdapter>(&restricted, registry, seed, None, None);
            let elapsed_secs = clock.elapsed().as_secs_f64();
            let series = run
                .series
                .iter()
                .map(|sample| {
                    let mut v = [None; 5];
                    for (i, id) in SMOKE_ALLOWED_OBSERVABLES.iter().enumerate() {
                        v[i] = sample
                            .observables
                            .iter()
                            .find(|(k, _)| k == id)
                            .map(|(_, x)| *x);
                    }
                    (sample.tick, v)
                })
                .collect();
            let failure = match &run.status {
                RunStatus::Completed => None,
                RunStatus::Failed { tick, reason, .. } => Some((*tick, scrub(reason))),
            };
            runs.push(SmokeRun {
                arm,
                seed,
                status: run.status.clone(),
                checksum: run.final_checksum,
                elapsed_secs,
                series,
            });
            if let Some((tick, reason)) = failure {
                say(&format!(
                    "E-SEL1 smoke: {arm} seed {seed} FAILED at tick {tick}: {reason}"
                ))?;
            }
        }
    }
    let ended = SystemTime::now();
    let max_us = runs
        .iter()
        .map(|r| r.us_per_tick(rung))
        .fold(0.0_f64, f64::max);
    say(&format!(
        "E-SEL1 smoke: {} runs x {rung} ticks; max {:.1} us/tick; only {:?} were written",
        runs.len(),
        max_us,
        SMOKE_ALLOWED_OBSERVABLES
    ))?;

    // runs.csv: one row per run, allow-listed columns only.
    let mut csv = String::from(
        "# SMOKE: allow-listed observables only (live.agent_count, live.standing_crop, live.closed_eu_total, live.births, live.deaths); no body-mass value is written (plan section 6)\n\
         seed,arm,status,extinct_tick,min_agent_count,max_agent_count,max_agent_count_tick_le_1200,standing_crop_tail_mean_tick_gt_half_T,us_per_tick,final_checksum\n",
    );
    for r in &runs {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{:.3},{}\n",
            r.seed,
            r.arm,
            status_label(&r.status),
            r.extinct_tick().map(|t| t.to_string()).unwrap_or_default(),
            r.min_agents().map(|v| v.to_string()).unwrap_or_default(),
            r.max_agents().map(|v| v.to_string()).unwrap_or_default(),
            r.max_agents_step_a()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            r.standing_crop_tail_mean(rung)
                .map(|v| v.to_string())
                .unwrap_or_default(),
            r.us_per_tick(rung),
            r.checksum,
        ));
    }
    // smoke-series.csv: the allow-listed sample series.
    let mut series = String::from("seed,arm,tick");
    for id in SMOKE_ALLOWED_OBSERVABLES {
        series.push(',');
        series.push_str(id);
    }
    series.push('\n');
    for r in &runs {
        for (tick, v) in &r.series {
            series.push_str(&format!("{},{},{tick}", r.seed, r.arm));
            for x in v {
                series.push(',');
                if let Some(x) = x {
                    series.push_str(&x.to_string());
                }
            }
            series.push('\n');
        }
    }
    // smoke-report.json: exactly the quantities the algorithm reads.
    let per_run: Vec<serde_json::Value> = runs
        .iter()
        .map(|r| {
            serde_json::json!({
                "arm": r.arm,
                "seed": r.seed,
                "status": status_label(&r.status),
                "extinct_tick": r.extinct_tick(),
                "min_live_agent_count_all_samples": r.min_agents(),
                "max_live_agent_count_all_samples": r.max_agents(),
                "max_live_agent_count_tick_le_1200": r.max_agents_step_a(),
                "mean_live_standing_crop_tick_gt_half_T": r.standing_crop_tail_mean(rung),
                "us_per_tick": r.us_per_tick(rung),
                "final_checksum": r.checksum,
            })
        })
        .collect();
    let report = serde_json::json!({
        "kind": "e-sel1-smoke-report-EXCLUDED-FROM-ANALYSIS",
        "contains_body_mass": false,
        "observables_written": SMOKE_ALLOWED_OBSERVABLES,
        "T_smoke": rung,
        "step_A_window_max_tick": SMOKE_STEP_A_TICKS,
        "runs": per_run,
        "max_us_per_tick": max_us,
    });
    let exe = std::env::current_exe().ok();
    let prov = serde_json::json!({
        "kind": "e-sel1-smoke-provenance-EXCLUDED-FROM-ANALYSIS",
        "mode": "smoke",
        "model": "core::live_experiment::LiveExperimentAdapter",
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
            "started_utc": iso8601_utc(unix_seconds(started)),
            "ended_utc": iso8601_utc(unix_seconds(ended)),
            "max_us_per_tick": max_us,
            "us_per_tick_by_run": runs.iter().map(|r| serde_json::json!({
                "arm": r.arm, "seed": r.seed, "us_per_tick": r.us_per_tick(rung) })).collect::<Vec<_>>(),
        },
        "duration": { "duration_ticks_as_run": rung, "ladder_registered": plan.ladder },
        "manifests": {
            "files": file_names,
            "sha256_as_read": manifest_hashes,
            "initial_conditions_by_arm": {
                "control": control.initial_conditions.values,
                "treatment": treatment.initial_conditions.values,
                "drift": drift.map(|d| d.initial_conditions.values.clone()),
            },
            "note": "manifests are not copied into smoke/ because they name every observable; their sha256 is recorded here",
        },
        "seeds": { "executed": runs.iter().map(|r| r.seed).collect::<Vec<_>>(), "smoke_seed": plan.smoke_seed },
        "observables_written": SMOKE_ALLOWED_OBSERVABLES,
    });
    let md = format!(
        "# E-SEL1 smoke (EXCLUDED from every analysis and from every choice but N0, F_hi, F_lo and the rung)\n\n\
         Only live.agent_count, live.standing_crop, live.closed_eu_total, live.births and live.deaths are \
         written; no body-mass value or effect exists in this directory (plan section 6).\n\n\
         - runs: {}\n- T_smoke: {rung} ticks\n- max us/tick: {:.1}\n\n\
         See `smoke-report.json` for the quantities the smoke algorithm reads.\n",
        runs.len(),
        max_us
    );
    let report_text = pretty(&report)? + "\n";
    let prov_text = pretty(&prov)? + "\n";
    // The rule is enforced here: nothing naming a body-mass observable reaches any smoke output.
    for (name, text) in [
        ("runs.csv", &csv),
        ("smoke-series.csv", &series),
        ("smoke-report.json", &report_text),
        ("provenance.json", &prov_text),
        ("summary.md", &md),
    ] {
        guard_no_body_mass(name, text)?;
    }
    write_text(&out_dir.join("runs.csv"), &csv)?;
    write_text(&out_dir.join("smoke-series.csv"), &series)?;
    write_text(&out_dir.join("smoke-report.json"), &report_text)?;
    write_text(&out_dir.join("provenance.json"), &prov_text)?;
    write_text(&out_dir.join("summary.md"), &md)?;
    let mut lines = String::new();
    for name in [
        "provenance.json",
        "runs.csv",
        "smoke-report.json",
        "smoke-series.csv",
        "summary.md",
    ] {
        let digest = sha256_file(&out_dir.join(name)).map_err(|e| format!("hash {name}: {e}"))?;
        lines.push_str(&format!("{digest}  {name}\n"));
    }
    write_text(&out_dir.join("checksums.sha256"), &lines)?;
    say(&format!(
        "  smoke artifacts written to {}",
        out_dir.display()
    ))?;
    Ok(())
}

// ---- Arguments ---------------------------------------------------------------------------------

fn parse_args(argv: Vec<String>) -> Result<Args, String> {
    let mut mode: Option<Mode> = None;
    let mut manifest_dir: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut duration_rung: Option<u64> = None;
    let mut replay_seed: Option<u64> = None;
    let mut replay_arm: Option<String> = None;

    let mut it = argv.into_iter();
    while let Some(arg) = it.next() {
        let mut set_mode = |m: Mode| -> Result<(), String> {
            if let Some(existing) = mode {
                return Err(format!(
                    "--smoke, --ensemble and --replay are mutually exclusive; {existing:?} was \
                     already selected"
                ));
            }
            mode = Some(m);
            Ok(())
        };
        match arg.as_str() {
            "--smoke" => set_mode(Mode::Smoke)?,
            "--ensemble" => set_mode(Mode::Ensemble)?,
            "--replay" => set_mode(Mode::Replay)?,
            "--manifest-dir" => manifest_dir = Some(PathBuf::from(need(&mut it, &arg)?)),
            "--out" => out = Some(PathBuf::from(need(&mut it, &arg)?)),
            "--duration-rung" => {
                duration_rung = Some(
                    need(&mut it, &arg)?
                        .parse()
                        .map_err(|e| format!("--duration-rung must be an integer: {e}"))?,
                )
            }
            "--replay-seed" => {
                replay_seed = Some(
                    need(&mut it, &arg)?
                        .parse()
                        .map_err(|e| format!("--replay-seed must be an integer: {e}"))?,
                )
            }
            "--replay-arm" => replay_arm = Some(need(&mut it, &arg)?),
            "--help" | "-h" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument '{other}'\n\n{USAGE}")),
        }
    }

    let mode = mode.ok_or_else(|| {
        format!("exactly one of --smoke, --ensemble or --replay is required\n\n{USAGE}")
    })?;
    let manifest_dir = manifest_dir.ok_or("--manifest-dir is required")?;
    let out = out.ok_or("--out is required")?;
    if !manifest_dir.is_dir() {
        return Err(format!("{} is not a directory", manifest_dir.display()));
    }
    if mode == Mode::Replay && (replay_seed.is_none() || replay_arm.is_none()) {
        return Err("--replay needs --replay-seed and --replay-arm control|treatment|drift".into());
    }
    if mode != Mode::Replay && (replay_seed.is_some() || replay_arm.is_some()) {
        return Err("--replay-seed / --replay-arm are only meaningful with --replay".into());
    }
    Ok(Args {
        mode,
        manifest_dir,
        out,
        duration_rung,
        replay_seed,
        replay_arm,
    })
}

const USAGE: &str = "\
run_esel1_experiment --smoke|--ensemble|--replay --manifest-dir <dir> --out <dir>
                     [--duration-rung <ticks>]
                     [--replay-seed <seed> --replay-arm control|treatment|drift]

Build with:
  cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop \\
    --example run_esel1_experiment
then execute the compiled binary directly. Never `cargo run`.
--ensemble needs <out>/execution-lock.json (written by hand after the smoke).";

fn need(it: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    it.next().ok_or_else(|| format!("{flag} needs a value"))
}

// ---- The preregistered plan, read as data ------------------------------------------------------

struct Plan {
    seeds: Vec<u64>,
    smoke_seed: u64,
    min_complete_pairs: usize,
    ladder: Vec<u64>,
    max_wall_clock_minutes: f64,
    control_file: String,
    treatment_file: String,
    drift_file: Option<String>,
    smoke_control_file: String,
    smoke_treatment_file: String,
    smoke_drift_file: Option<String>,
}

impl Plan {
    fn from_prereg(p: &serde_json::Value) -> Result<Self, String> {
        let arr = |v: &serde_json::Value, what: &str| -> Result<Vec<u64>, String> {
            v.as_array()
                .ok_or_else(|| format!("{what} must be a list"))?
                .iter()
                .map(|x| {
                    x.as_u64()
                        .ok_or_else(|| format!("{what} holds a non-integer"))
                })
                .collect()
        };
        let s = |v: &serde_json::Value, what: &str| -> Result<String, String> {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("{what} must be a string"))
        };
        let opt = |v: &serde_json::Value| v.as_str().map(str::to_string);
        Ok(Plan {
            seeds: arr(&p["seeds"]["execution_order"], "seeds.execution_order")?,
            smoke_seed: p["seeds"]["smoke_seed"]
                .as_u64()
                .ok_or("seeds.smoke_seed must be an integer")?,
            min_complete_pairs: p["seeds"]["min_complete_pairs_for_a_decision"]
                .as_u64()
                .ok_or("seeds.min_complete_pairs_for_a_decision must be an integer")?
                as usize,
            ladder: arr(&p["duration"]["duration_ticks_ladder"], "the ladder")?,
            max_wall_clock_minutes: p["cost"]["max_wall_clock_minutes"]
                .as_f64()
                .ok_or("cost.max_wall_clock_minutes must be a number")?,
            control_file: s(&p["manifests"]["control"], "manifests.control")?,
            treatment_file: s(&p["manifests"]["treatment"], "manifests.treatment")?,
            drift_file: opt(&p["manifests"]["drift_control_optional_requires_P4"]),
            smoke_control_file: s(&p["manifests"]["smoke_control"], "manifests.smoke_control")?,
            smoke_treatment_file: s(
                &p["manifests"]["smoke_treatment"],
                "manifests.smoke_treatment",
            )?,
            smoke_drift_file: opt(&p["manifests"]["smoke_drift"]),
        })
    }

    /// Refuse a mixed or undeclared seed set before anything is built.
    fn refuse_undeclared_seeds(
        &self,
        mode: Mode,
        control: &ExperimentManifest,
        treatment: &ExperimentManifest,
        drift: Option<&ExperimentManifest>,
    ) -> Result<(), String> {
        for other in std::iter::once(treatment).chain(drift) {
            if control.seeds != other.seeds {
                return Err(format!(
                    "the arms declare different seed lists ({:?} vs {:?}); pairing requires the \
                     same seeds in the same order",
                    control.seeds, other.seeds
                ));
            }
        }
        match mode {
            Mode::Ensemble | Mode::Replay => {
                if control.seeds != self.seeds {
                    return Err(format!(
                        "the manifests declare {:?}, but the preregistered execution order is \
                         {:?}; no substitution, reordering or top-up is permitted",
                        control.seeds, self.seeds
                    ));
                }
                if control.seeds.contains(&self.smoke_seed) {
                    return Err(format!(
                        "the smoke seed {} appears in an experimental manifest",
                        self.smoke_seed
                    ));
                }
            }
            Mode::Smoke => {
                if control.seeds != vec![self.smoke_seed] {
                    return Err(format!(
                        "a smoke run may declare exactly the smoke seed [{}]; the manifests \
                         declare {:?}",
                        self.smoke_seed, control.seeds
                    ));
                }
            }
        }
        Ok(())
    }
}

/// The value of one initial condition of a manifest, if declared.
fn ic_value(m: &ExperimentManifest, key: &str) -> Option<f64> {
    m.initial_conditions
        .values
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| *v)
}

/// The execution lock is written by hand after the smoke and gates the ensemble. It must state
/// `duration_ticks`, `N0`, `F_hi` and `F_lo`, and each must agree with what this run would use: the
/// effective duration, and the founders and food caps of the manifests in `--manifest-dir`. A lock
/// that is silent on a value, or disagrees with the manifests, was not written against them.
fn validate_lock(
    lock: &serde_json::Value,
    rung: u64,
    control: &ExperimentManifest,
    treatment: &ExperimentManifest,
    drift: Option<&ExperimentManifest>,
) -> Result<(), String> {
    let need_num = |field: &str| -> Result<f64, String> {
        lock.get(field).and_then(|v| v.as_f64()).ok_or_else(|| {
            format!("{LOCK_FILE} lacks {field} (a number); the run was never fully locked")
        })
    };
    let t = lock
        .get("duration_ticks")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            format!("{LOCK_FILE} lacks duration_ticks (an integer); the run was never fully locked")
        })?;
    if t != rung {
        return Err(format!(
            "{LOCK_FILE} locks duration_ticks = {t} but this run would use {rung}; the rung is \
             chosen once and never changed"
        ));
    }
    let (n0, f_hi, f_lo) = (need_num("N0")?, need_num("F_hi")?, need_num("F_lo")?);
    let check = |what: &str, locked: f64, m: &ExperimentManifest, key: &str| {
        let held = ic_value(m, key);
        if held == Some(locked) {
            Ok(())
        } else {
            Err(format!(
                "{LOCK_FILE} locks {what} = {locked}, but '{}' holds {key} = {held:?}",
                m.experiment_id
            ))
        }
    };
    for m in [Some(control), Some(treatment), drift]
        .into_iter()
        .flatten()
    {
        check("N0", n0, m, FOUNDERS_KEY)?;
    }
    check("F_hi", f_hi, control, FOOD_KEY)?;
    check("F_lo", f_lo, treatment, FOOD_KEY)?;
    if let Some(d) = drift {
        check("F_lo", f_lo, d, FOOD_KEY)?;
    }
    Ok(())
}

/// Exactly one initial condition differs per contrast: `live.food_cap` between control and
/// treatment, `live.mutation_rate` between treatment and the drift arm. Anything else is a factor
/// nobody registered.
fn check_factors(
    control: &ExperimentManifest,
    treatment: &ExperimentManifest,
    drift: Option<&ExperimentManifest>,
) -> Result<(), String> {
    let one_key_diff = |a: &ExperimentManifest, b: &ExperimentManifest, key: &str| {
        let diff = key_differences(a, b);
        if diff == [key.to_string()] {
            Ok(())
        } else {
            Err(format!(
                "'{}' vs '{}' must differ in exactly the initial condition {key}; they differ in \
                 {diff:?}",
                a.experiment_id, b.experiment_id
            ))
        }
    };
    one_key_diff(control, treatment, FOOD_KEY)?;
    if let Some(d) = drift {
        one_key_diff(treatment, d, MUTATION_KEY)?;
    }
    Ok(())
}

/// The sorted set of initial-condition keys whose presence or value differs.
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

// ---- Per-run reduction -------------------------------------------------------------------------

/// What the preregistration reads out of one run.
struct RunSummary {
    completed: bool,
    /// First sampled tick at which `live.agent_count` was 0 (or the final tick if only the final
    /// observable shows it).
    extinct_tick: Option<u64>,
    /// Mean of `live.trait_mass_mean` over samples with `tick > 0.8 * T`.
    tail_mean: Option<f64>,
    min_agents: Option<f64>,
    max_agents: Option<f64>,
    closed_eu_relative_spread: Option<f64>,
    births_max: Option<f64>,
}

fn tail_samples_possible(duration: u64, period: u64) -> u64 {
    if period == 0 {
        return 0;
    }
    (1..=duration / period)
        .filter(|k| k * period * 5 > 4 * duration)
        .count() as u64
}

fn summarise(run: &RunResult, duration: u64) -> RunSummary {
    let value = |sample: &anima_engine_lib::core::scenario::StateSample, id: &str| {
        sample
            .observables
            .iter()
            .find(|(k, _)| k == id)
            .map(|(_, v)| *v)
    };
    let mut tail = Vec::new();
    let mut extinct_tick = None;
    let mut min_agents: Option<f64> = None;
    let mut max_agents: Option<f64> = None;
    let mut eu: Vec<f64> = Vec::new();
    let mut births_max: Option<f64> = None;
    for sample in &run.series {
        if let Some(n) = value(sample, COUNT_OBSERVABLE) {
            min_agents = Some(min_agents.map_or(n, |m| m.min(n)));
            max_agents = Some(max_agents.map_or(n, |m| m.max(n)));
            if n == 0.0 && extinct_tick.is_none() {
                extinct_tick = Some(sample.tick);
            }
        }
        if sample.tick * 5 > 4 * duration {
            if let Some(m) = value(sample, PRIMARY_OBSERVABLE) {
                tail.push(m);
            }
        }
        if let Some(v) = value(sample, INTEGRITY_OBSERVABLE) {
            eu.push(v);
        }
        if let Some(b) = value(sample, BIRTHS_OBSERVABLE) {
            births_max = Some(births_max.map_or(b, |m| m.max(b)));
        }
    }
    if extinct_tick.is_none() && run.observable(COUNT_OBSERVABLE) == Some(0.0) {
        extinct_tick = Some(duration);
    }
    let closed_eu_relative_spread = if eu.is_empty() {
        None
    } else {
        let hi = eu.iter().cloned().fold(f64::MIN, f64::max);
        let lo = eu.iter().cloned().fold(f64::MAX, f64::min);
        Some((hi - lo) / hi.abs().max(1.0))
    };
    RunSummary {
        completed: run.status.is_completed(),
        extinct_tick,
        tail_mean: if tail.is_empty() {
            None
        } else {
            Some(tail.iter().sum::<f64>() / tail.len() as f64)
        },
        min_agents,
        max_agents,
        closed_eu_relative_spread,
        births_max,
    }
}

// ---- Analysis ----------------------------------------------------------------------------------

struct Dropped {
    seed: u64,
    arm: &'static str,
    tick: Option<u64>,
    reason: String,
}

struct Effect {
    contrast: &'static str,
    effect: PairedEffect,
    /// `(seed, control_side, treatment_side, delta)`.
    rows: Vec<(u64, f64, f64, f64)>,
    median_delta: Option<f64>,
}

impl Effect {
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
        let (a, b, c) = self.materiality();
        a && b && c
    }
    fn relative_shift(&self) -> Option<f64> {
        match (self.effect.paired_mean_delta, self.effect.control_mean) {
            (Some(d), Some(c)) if c != 0.0 => Some(d / c.abs()),
            _ => None,
        }
    }
}

struct Analysis {
    control_summaries: Vec<(u64, RunSummary)>,
    treatment_summaries: Vec<(u64, RunSummary)>,
    drift_summaries: Vec<(u64, RunSummary)>,
    dropped_extinct: Vec<Dropped>,
    dropped_failed: Vec<Dropped>,
    h1: Effect,
    h1_pairs_used: usize,
    h2: Option<Effect>,
    integrity_findings: Vec<String>,
    extinct_by_arm: BTreeMap<&'static str, usize>,
}

fn analyse(
    report: &PairedEnsembleReport,
    drift_runs: Option<&[RunResult]>,
    control: &ExperimentManifest,
) -> Result<Analysis, String> {
    let duration = control.duration_ticks;
    let mut control_summaries = Vec::new();
    let mut treatment_summaries = Vec::new();
    for pair in &report.pairs {
        control_summaries.push((pair.seed, summarise(&pair.control, duration)));
        treatment_summaries.push((pair.seed, summarise(&pair.treatment, duration)));
    }
    let drift_summaries: Vec<(u64, RunSummary)> = match drift_runs {
        Some(runs) => runs
            .iter()
            .zip(&control.seeds)
            .map(|(r, &s)| (s, summarise(r, duration)))
            .collect(),
        None => Vec::new(),
    };

    let mut dropped_extinct = Vec::new();
    let mut dropped_failed = Vec::new();
    let mut extinct_by_arm: BTreeMap<&'static str, usize> = BTreeMap::new();
    extinct_by_arm.insert("control", 0);
    extinct_by_arm.insert("treatment", 0);
    if drift_runs.is_some() {
        extinct_by_arm.insert("drift", 0);
    }

    // Classify every run once, so each drop is recorded with its arm.
    let classify = |arm: &'static str,
                    seed: u64,
                    run: &RunResult,
                    s: &RunSummary,
                    extinct_by_arm: &mut BTreeMap<&'static str, usize>,
                    dropped_extinct: &mut Vec<Dropped>,
                    dropped_failed: &mut Vec<Dropped>| {
        if let RunStatus::Failed { tick, reason, .. } = &run.status {
            dropped_failed.push(Dropped {
                seed,
                arm,
                tick: Some(*tick),
                reason: reason.clone(),
            });
        } else if let Some(t) = s.extinct_tick {
            *extinct_by_arm.entry(arm).or_insert(0) += 1;
            dropped_extinct.push(Dropped {
                seed,
                arm,
                tick: Some(t),
                reason: "live.agent_count reached 0".into(),
            });
        }
    };
    for (i, pair) in report.pairs.iter().enumerate() {
        classify(
            "control",
            pair.seed,
            &pair.control,
            &control_summaries[i].1,
            &mut extinct_by_arm,
            &mut dropped_extinct,
            &mut dropped_failed,
        );
        classify(
            "treatment",
            pair.seed,
            &pair.treatment,
            &treatment_summaries[i].1,
            &mut extinct_by_arm,
            &mut dropped_extinct,
            &mut dropped_failed,
        );
    }
    if let Some(runs) = drift_runs {
        for (i, run) in runs.iter().enumerate() {
            classify(
                "drift",
                drift_summaries[i].0,
                run,
                &drift_summaries[i].1,
                &mut extinct_by_arm,
                &mut dropped_extinct,
                &mut dropped_failed,
            );
        }
    }

    let usable = |s: &RunSummary| s.completed && s.extinct_tick.is_none() && s.tail_mean.is_some();

    // H1: treatment (scarce) minus control (abundant), complete non-extinct pairs only.
    let (mut cs, mut ts, mut deltas, mut seeds1) = (vec![], vec![], vec![], vec![]);
    for (i, pair) in report.pairs.iter().enumerate() {
        let (c, t) = (&control_summaries[i].1, &treatment_summaries[i].1);
        if usable(c) && usable(t) {
            let (cv, tv) = (c.tail_mean.unwrap_or(0.0), t.tail_mean.unwrap_or(0.0));
            cs.push(cv);
            ts.push(tv);
            deltas.push(tv - cv);
            seeds1.push(pair.seed);
        }
    }
    let h1_pairs_used = deltas.len();
    let h1 = make_effect(
        "H1 scarce_vs_abundant",
        report.pairs.len(),
        &seeds1,
        &cs,
        &ts,
        &deltas,
    )?;

    // H2: drift arm (mutation off) vs the treatment it was derived from, same seeds.
    let h2 = if drift_runs.is_some() {
        let (mut cs, mut ts, mut deltas, mut seeds2) = (vec![], vec![], vec![], vec![]);
        for (i, (seed, d)) in drift_summaries.iter().enumerate() {
            let t = &treatment_summaries[i].1;
            if usable(t) && usable(d) {
                // "control side" of H2 is the treatment arm (mutation on), delta = drift - scarce.
                let (tv, dv) = (t.tail_mean.unwrap_or(0.0), d.tail_mean.unwrap_or(0.0));
                cs.push(tv);
                ts.push(dv);
                deltas.push(dv - tv);
                seeds2.push(*seed);
            }
        }
        Some(make_effect(
            "H2 scarce_mut0_vs_scarce",
            report.pairs.len(),
            &seeds2,
            &cs,
            &ts,
            &deltas,
        )?)
    } else {
        None
    };

    // Harness integrity (H4): closed EU constant in-run, and births observed where the population
    // grew. Findings are listed, never silently absorbed.
    let mut integrity_findings = Vec::new();
    let arms: [(&str, &Vec<(u64, RunSummary)>); 3] = [
        ("control", &control_summaries),
        ("treatment", &treatment_summaries),
        ("drift", &drift_summaries),
    ];
    for (arm, list) in arms {
        for (seed, s) in list {
            if let Some(sp) = s.closed_eu_relative_spread {
                if sp > CLOSED_EU_TOLERANCE {
                    integrity_findings.push(format!(
                        "{arm} seed {seed}: live.closed_eu_total moved by a relative {sp:.3e} \
                         within one run (tolerance {CLOSED_EU_TOLERANCE:.0e}); a defect, not biology"
                    ));
                }
            }
            if let (Some(b), Some(first_max)) = (s.births_max, s.max_agents) {
                if b == 0.0 && s.min_agents.is_some_and(|m| first_max > m) {
                    integrity_findings.push(format!(
                        "{arm} seed {seed}: population grew but live.births never exceeded 0"
                    ));
                }
            }
        }
    }

    Ok(Analysis {
        control_summaries,
        treatment_summaries,
        drift_summaries,
        dropped_extinct,
        dropped_failed,
        h1,
        h1_pairs_used,
        h2,
        integrity_findings,
        extinct_by_arm,
    })
}

fn make_effect(
    contrast: &'static str,
    n_requested: usize,
    seeds: &[u64],
    cs: &[f64],
    ts: &[f64],
    deltas: &[f64],
) -> Result<Effect, String> {
    let effect = paired_effect(TAIL_STAT, n_requested, cs, ts, deltas)
        .map_err(|e| format!("{contrast}: the shared paired statistic refused: {e}"))?;
    let rows = seeds
        .iter()
        .zip(cs.iter().zip(ts.iter().zip(deltas.iter())))
        .map(|(&s, (&c, (&t, &d)))| (s, c, t, d))
        .collect();
    Ok(Effect {
        contrast,
        effect,
        rows,
        median_delta: median(deltas),
    })
}

fn median(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).expect("finite deltas"));
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

// ---- Replay ------------------------------------------------------------------------------------

fn replay(
    args: &Args,
    control: &ExperimentManifest,
    treatment: &ExperimentManifest,
    drift: Option<&ExperimentManifest>,
    registry: &ObservableRegistry,
    out_dir: &Path,
    rung: u64,
) -> Result<(), String> {
    let arm = args.replay_arm.as_deref().unwrap_or_default();
    let manifest = match arm {
        "control" => control,
        "treatment" => treatment,
        "drift" => drift.ok_or("--replay-arm drift, but this plan has no drift arm")?,
        other => {
            return Err(format!(
                "--replay-arm must be control, treatment or drift, not '{other}'"
            ))
        }
    };
    let seed = args.replay_seed.expect("checked in parse_args");
    if !manifest.seeds.contains(&seed) {
        return Err(format!(
            "seed {seed} is not declared by the {arm} manifest, which declares {:?}",
            manifest.seeds
        ));
    }
    let clock = Instant::now();
    let result = run_manifest_seed::<LiveExperimentAdapter>(manifest, registry, seed, None, None);
    let elapsed = clock.elapsed().as_secs_f64();
    let doc = serde_json::json!({
        "kind": "e-sel1-replay",
        "note": "A checksum-identity check. Never merged into the analysis.",
        "arm": arm,
        "seed": seed,
        "duration_ticks": rung,
        "experiment_id": manifest.experiment_id,
        "manifest_fingerprint": manifest.fingerprint(),
        "status": status_label(&result.status),
        "final_checksum": result.final_checksum,
        "final_observables": result.final_observables.iter().cloned().collect::<BTreeMap<String, f64>>(),
        "warnings": result.warnings,
        "wall_clock_seconds": elapsed,
        "model_version_of_the_model_actually_run": LIVE_MODEL_VERSION,
    });
    let name = format!("replay-{arm}-seed-{seed}-t{rung}.json");
    write_text(&out_dir.join(&name), &(pretty(&doc)? + "\n"))?;
    println!(
        "replay {arm} seed {seed}: checksum {} ({} in {:.1}s) -> {}",
        result.final_checksum,
        status_label(&result.status),
        elapsed,
        out_dir.join(&name).display()
    );
    Ok(())
}

// ---- Artifacts ---------------------------------------------------------------------------------

struct WriteInput<'a> {
    mode: Mode,
    out_dir: &'a Path,
    manifest_dir: &'a Path,
    file_names: &'a [String],
    manifest_hashes: &'a BTreeMap<String, String>,
    control: &'a ExperimentManifest,
    treatment: &'a ExperimentManifest,
    drift: Option<&'a ExperimentManifest>,
    committed_duration: u64,
    plan: &'a Plan,
    rung: u64,
    report: &'a PairedEnsembleReport,
    drift_runs: Option<&'a [RunResult]>,
    analysis: &'a Analysis,
    lock: Option<&'a serde_json::Value>,
    /// sha256 of `execution-lock.json` exactly as read; `Some` iff the lock exists (ensemble).
    lock_sha256: Option<&'a str>,
    registry: &'a ObservableRegistry,
    started: SystemTime,
    ended: SystemTime,
    elapsed_secs: f64,
}

fn write_artifacts(w: &WriteInput<'_>) -> Result<(), String> {
    let manifests_dir = w.out_dir.join("manifests");
    std::fs::create_dir_all(&manifests_dir)
        .map_err(|e| format!("create {}: {e}", manifests_dir.display()))?;
    for name in w.file_names {
        let from = w.manifest_dir.join(name);
        let bytes = std::fs::read(&from).map_err(|e| format!("read {}: {e}", from.display()))?;
        std::fs::write(manifests_dir.join(name), &bytes)
            .map_err(|e| format!("copy {name}: {e}"))?;
    }
    write_text(
        &w.out_dir.join("paired-report.json"),
        &(pretty(&serde_json::to_value(w.report).map_err(|e| e.to_string())?)? + "\n"),
    )?;
    write_text(&w.out_dir.join("effects.json"), &effects_json(w)?)?;
    write_text(
        &w.out_dir.join("per-seed-deltas.csv"),
        &per_seed_csv(w.analysis),
    )?;
    write_text(&w.out_dir.join("runs.csv"), &runs_csv(w))?;
    write_text(&w.out_dir.join("provenance.json"), &provenance_json(w)?)?;
    write_text(&w.out_dir.join("summary.md"), &summary_md(w))?;

    let mut names: Vec<String> = vec![
        "effects.json".into(),
        "paired-report.json".into(),
        "per-seed-deltas.csv".into(),
        "provenance.json".into(),
        "runs.csv".into(),
        "summary.md".into(),
    ];
    for name in w.file_names {
        names.push(format!("manifests/{name}"));
    }
    if w.lock.is_some() {
        // The hand-written lock is part of the evidence: hash it with the rest.
        names.push(LOCK_FILE.to_string());
    }
    names.sort();
    let mut lines = String::new();
    for name in &names {
        let digest = sha256_file(&w.out_dir.join(name)).map_err(|e| format!("hash {name}: {e}"))?;
        lines.push_str(&format!("{digest}  {name}\n"));
    }
    write_text(&w.out_dir.join("checksums.sha256"), &lines)
}

fn effect_json(e: &Effect) -> serde_json::Value {
    let (ci, dz, rel) = e.materiality();
    serde_json::json!({
        "contrast": e.contrast,
        "statistic": TAIL_STAT,
        "arm_order": if e.contrast.starts_with("H2") { "delta = scarce_mut0 - scarce" } else { "delta = scarce - abundant" },
        "n_requested": e.effect.n_requested,
        "n_complete_pairs": e.effect.n_complete_pairs,
        "control_mean": e.effect.control_mean,
        "treatment_mean": e.effect.treatment_mean,
        "paired_mean_delta": e.effect.paired_mean_delta,
        "paired_sd": e.effect.paired_sd,
        "paired_se": e.effect.paired_se,
        "ci95_low": e.effect.ci95_low,
        "ci95_high": e.effect.ci95_high,
        "paired_dz": e.effect.paired_dz,
        "median_of_per_seed_deltas": e.median_delta,
        "relative_shift_vs_control_mean": e.relative_shift(),
        "materiality": {
            "ci95_excludes_zero": ci,
            "abs_dz_at_least_0_8": dz,
            "abs_delta_at_least_5pc_of_control_mean": rel,
            "material": e.is_material(),
        },
    })
}

fn dropped_json(d: &[Dropped]) -> Vec<serde_json::Value> {
    d.iter()
        .map(|x| {
            serde_json::json!({
                "seed": x.seed, "arm": x.arm, "tick": x.tick, "reason": x.reason,
            })
        })
        .collect()
}

fn effects_json(w: &WriteInput<'_>) -> Result<String, String> {
    let a = w.analysis;
    let descriptive: Vec<serde_json::Value> = w
        .report
        .effects
        .iter()
        .filter(|e| {
            DESCRIPTIVE_OBSERVABLES.contains(&e.observable.as_str())
                || e.observable == INTEGRITY_OBSERVABLE
        })
        .map(|e| serde_json::to_value(e).map_err(|x| x.to_string()))
        .collect::<Result<_, _>>()?;
    let doc = serde_json::json!({
        "kind": if w.mode == Mode::Smoke { "e-sel1-smoke-effects-EXCLUDED-FROM-ANALYSIS" } else { "e-sel1-effects" },
        "primary_observable": PRIMARY_OBSERVABLE,
        "primary_statistic": "paired mean of the per-run tail-window mean (tick > 0.8 * duration); treatment minus control",
        "materiality_rule": "all_of: ci95 excludes 0; |paired_dz| >= 0.8; |paired_mean_delta| >= 0.05 * |control_mean|",
        "min_complete_pairs_for_a_decision": w.plan.min_complete_pairs,
        "n_pairs_requested": w.report.pairs.len(),
        "n_complete_pairs_used_by_h1": a.h1_pairs_used,
        "sufficient_pairs": a.h1_pairs_used >= w.plan.min_complete_pairs,
        "extinction": {
            "rule": "live.agent_count == 0 at any sample (or finally) makes the run extinct; its pair is dropped from the paired statistic and counted here",
            "n_extinct_runs_by_arm": a.extinct_by_arm,
            "n_extinct_runs_total": a.dropped_extinct.len(),
            "extinct_runs": dropped_json(&a.dropped_extinct),
        },
        "failed_runs": dropped_json(&a.dropped_failed),
        "h1": effect_json(&a.h1),
        "h2_descriptive_drift_control": a.h2.as_ref().map(effect_json),
        "descriptive_final_value_effects_complete_pairs": descriptive,
        "integrity_findings": a.integrity_findings,
    });
    Ok(pretty(&doc)? + "\n")
}

fn per_seed_csv(a: &Analysis) -> String {
    let mut s = String::from("seed,contrast,observable,control_final,treatment_final,delta\n");
    for e in std::iter::once(&a.h1).chain(a.h2.as_ref()) {
        for (seed, c, t, d) in &e.rows {
            s.push_str(&format!("{seed},{},{TAIL_STAT},{c},{t},{d}\n", e.contrast));
        }
    }
    s
}

fn runs_csv(w: &WriteInput<'_>) -> String {
    let mut s = String::from(
        "seed,arm,status,failed_tick,failure_reason,extinct,extinct_tick,tail_mean_trait_mass,final_checksum,warnings",
    );
    for id in live_all_ids() {
        s.push(',');
        s.push_str(id);
    }
    s.push('\n');
    let mut row = |seed: u64, arm: &str, run: &RunResult, sum: &RunSummary| {
        let (tick, reason) = match &run.status {
            RunStatus::Completed => (String::new(), String::new()),
            RunStatus::Failed { tick, reason, .. } => (tick.to_string(), csv_field(reason)),
        };
        s.push_str(&format!(
            "{seed},{arm},{},{tick},{reason},{},{},{},{},{}",
            status_label(&run.status),
            sum.extinct_tick.is_some(),
            sum.extinct_tick.map(|t| t.to_string()).unwrap_or_default(),
            sum.tail_mean.map(|v| v.to_string()).unwrap_or_default(),
            run.final_checksum,
            csv_field(&run.warnings.join(" | ")),
        ));
        for id in live_all_ids() {
            s.push(',');
            if let Some(v) = run.observable(id) {
                s.push_str(&v.to_string());
            }
        }
        s.push('\n');
    };
    for (i, pair) in w.report.pairs.iter().enumerate() {
        row(
            pair.seed,
            "control",
            &pair.control,
            &w.analysis.control_summaries[i].1,
        );
        row(
            pair.seed,
            "treatment",
            &pair.treatment,
            &w.analysis.treatment_summaries[i].1,
        );
        if let Some(runs) = w.drift_runs {
            row(
                pair.seed,
                "drift",
                &runs[i],
                &w.analysis.drift_summaries[i].1,
            );
        }
    }
    s
}

fn provenance_json(w: &WriteInput<'_>) -> Result<String, String> {
    let (started, ended) = (unix_seconds(w.started), unix_seconds(w.ended));
    let exe = std::env::current_exe().ok();
    let doc = serde_json::json!({
        "kind": if w.mode == Mode::Smoke { "e-sel1-smoke-provenance-EXCLUDED-FROM-ANALYSIS" } else { "e-sel1-provenance" },
        "experiment_id": w.report.experiment_id,
        "mode": format!("{:?}", w.mode).to_lowercase(),
        "model": "core::live_experiment::LiveExperimentAdapter",
        "model_status": "headless adapter verified; NOT a live-world experiment-ready claim",
        "harness": "core::experiment_runner::run_paired_ensemble_with_control + paired_effect",
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
            "started_utc": iso8601_utc(started),
            "ended_utc": iso8601_utc(ended),
            "wall_clock_seconds": w.elapsed_secs,
            "max_wall_clock_minutes_registered": w.plan.max_wall_clock_minutes,
            "within_registered_budget": w.elapsed_secs / 60.0 <= w.plan.max_wall_clock_minutes,
            "us_per_tick": w.elapsed_secs * 1e6 / ((w.control.seeds.len() as f64) * (if w.drift.is_some() { 3.0 } else { 2.0 }) * (w.rung as f64)),
        },
        "duration": {
            "ladder_registered": w.plan.ladder,
            "duration_ticks_as_committed": w.committed_duration,
            "duration_ticks_as_run": w.rung,
            "sample_period": w.control.sample_period,
        },
        "execution_lock": w.lock,
        "execution_lock_sha256": w.lock_sha256,
        "manifests": {
            "sha256_as_committed": w.manifest_hashes,
            "control_fingerprint_as_run": w.report.control_manifest_fingerprint,
            "treatment_fingerprint_as_run": w.report.treatment_manifest_fingerprint,
            "drift_fingerprint_as_run": w.drift.map(|d| d.fingerprint()),
            "registry_fingerprint": w.report.registry_fingerprint,
            "declared_factors": w.report.declared_factors,
            "control_initial_conditions": w.control.initial_conditions.values,
            "treatment_initial_conditions": w.treatment.initial_conditions.values,
            "drift_initial_conditions": w.drift.map(|d| d.initial_conditions.values.clone()),
        },
        "seeds": {
            "execution_order": w.report.seed_order,
            "registered_execution_order": w.plan.seeds,
            "smoke_seed": w.plan.smoke_seed,
            "smoke_seed_present_in_this_run": w.report.seed_order.contains(&w.plan.smoke_seed),
            "n_requested": w.report.pairs.len(),
            "n_complete_pairs_used_by_h1": w.analysis.h1_pairs_used,
            "n_extinct_runs": w.analysis.dropped_extinct.len(),
            "n_failed_runs": w.analysis.dropped_failed.len(),
            "min_complete_pairs_for_a_decision": w.plan.min_complete_pairs,
        },
        "observables": {
            "requested": w.control.observable_ids,
            "registry_ids": live_all_ids().collect::<Vec<_>>(),
            "specs": w.registry.fingerprint(),
        },
        "what_this_run_does_not_claim": [
            "No desktop-app run was made; no Tauri handle, window, GPU, learner or evolution thread exists in this process.",
            "Not adaptation, local adaptation (CM-S11 not run), speciation, predators, Red-Queen coevolution or brains.",
            "One terrain seed; no generality beyond the headless adapter.",
        ],
    });
    Ok(pretty(&doc)? + "\n")
}

fn summary_md(w: &WriteInput<'_>) -> String {
    let a = w.analysis;
    let mut s = String::from("# E-SEL1 - food scarcity and body mass\n\n");
    if w.mode == Mode::Smoke {
        s.push_str(
            "> Calibration only, the smoke seed, EXCLUDED from every analysis, table, statistic \
             and claim.\n\n",
        );
    }
    s.push_str(&format!(
        "Model: `LiveExperimentAdapter`, **headless adapter verified**.\n\n\
         - seeds: `{:?}`\n- duration: **{} ticks** (committed {}), sample period {}\n\
         - pairs requested {}, complete non-extinct pairs used by H1: **{}** (a decision needs {})\n\
         - extinct runs: {} (by arm {:?}); failed runs: {}\n\
         - wall clock {:.1} s\n\n",
        w.report.seed_order,
        w.rung,
        w.committed_duration,
        w.control.sample_period,
        w.report.pairs.len(),
        a.h1_pairs_used,
        w.plan.min_complete_pairs,
        a.dropped_extinct.len(),
        a.extinct_by_arm,
        a.dropped_failed.len(),
        w.elapsed_secs,
    ));
    for e in std::iter::once(&a.h1).chain(a.h2.as_ref()) {
        let (ci, dz, rel) = e.materiality();
        s.push_str(&format!(
            "## {}\n\n`{TAIL_STAT}`: n = {}, delta {}, SD {}, CI [{}, {}], d_z {}, relative shift {}.\n\
             Conditions: CI excludes 0 = {}, |d_z| >= 0.8 = {}, |delta| >= 5% = {}; **material: {}**.\n\n",
            e.contrast,
            e.effect.n_complete_pairs,
            num(e.effect.paired_mean_delta),
            num(e.effect.paired_sd),
            num(e.effect.ci95_low),
            num(e.effect.ci95_high),
            num(e.effect.paired_dz),
            e.relative_shift()
                .map(|r| format!("{:+.2}%", r * 100.0))
                .unwrap_or_else(|| "n/a".into()),
            ci,
            dz,
            rel,
            e.is_material(),
        ));
    }
    if !a.dropped_extinct.is_empty() || !a.dropped_failed.is_empty() {
        s.push_str("## Dropped runs\n\n| seed | arm | tick | reason |\n|---|---|--:|---|\n");
        for d in a.dropped_extinct.iter().chain(a.dropped_failed.iter()) {
            s.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                d.seed,
                d.arm,
                d.tick.map(|t| t.to_string()).unwrap_or_default(),
                d.reason
            ));
        }
        s.push('\n');
    }
    if !a.integrity_findings.is_empty() {
        s.push_str("## Integrity findings (defects)\n\n");
        for f in &a.integrity_findings {
            s.push_str(&format!("- {f}\n"));
        }
        s.push('\n');
    }
    s.push_str(
        "Every number traces to a file in this directory; `checksums.sha256` covers them.\n",
    );
    s
}

// ---- Small helpers -----------------------------------------------------------------------------

fn num(v: Option<f64>) -> String {
    v.map(|x| {
        if x != 0.0 && x.abs() < 1e-4 {
            format!("{x:.3e}")
        } else {
            format!("{x:.6}")
        }
    })
    .unwrap_or_else(|| "n/a".into())
}

fn status_label(s: &RunStatus) -> &'static str {
    match s {
        RunStatus::Completed => "completed",
        RunStatus::Failed { .. } => "failed",
    }
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

/// LF endings on every platform, because the bytes are hashed.
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

/// Civil date from a UNIX timestamp (Howard Hinnant's `civil_from_days`).
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
