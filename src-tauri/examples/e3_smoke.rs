//! The E3 smoke calibration binary (WP-2 / WP-3 of `CARD E3-P`).
//!
//! Runs ONLY smoke seed `999979` in `--release` on all three E3 smoke arms:
//! - `e3-smoke-control-uniform.json` (`control_uniform`)
//! - `e3-smoke-control-uniform-scarcity.json` (`control_uniform_scarcity`)
//! - `e3-smoke-treatment-gradient.json` (`treatment_gradient`)
//!
//! Proves WP-2:
//! - (a) headless `ResourceField` presence and identical closed-EU / RNG / `WorldIdentity` at
//!   genesis (with `control_uniform_scarcity` scaled by `0.42` into detritus);
//! - (b1) West `Deforest 0.30` + East `AddNutrient 1.00` genuinely shift `live.standing_crop` in
//!   `treatment_gradient` relative to `control_uniform` (`|relative_shift| >= 0.10`, ledger > 0);
//! - (b2) `control_uniform_scarcity` (`scale = 0.42` + whole-map `AddNutrient 1.00`) matches
//!   `treatment_gradient`'s post-warmup mean `live.standing_crop` within `+/- 10%` with zero
//!   `Deforest` firings;
//! - (c) `detritus > 0` before tick `3000` and at tick `3000` in every arm.
//!
//! Also records WP-3's `T` and warm-up timing calibration projected for `36` runs (`3` arms x `12`
//! seeds).
//!
//! # Blinding
//!
//! No `live.trait_mass_*` observable value is ever written, printed, or used in any gate except
//! the uniform finiteness check (`is_finite()` over every emitted observable), keeping WP-2 and
//! WP-3 blind to body-mass outcomes (`body_mass_consulted: false`).
//!
//! # Build first, then execute the compiled binary -- never `cargo run`
//!
//! ```text
//! cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop --example e3_smoke -j 4
//! src-tauri/target/release/examples/e3_smoke \
//!   --manifest-dir src-tauri/tests/fixtures/experiments_e3 \
//!   --out artifacts/experiments/e3-productivity-gradient-body-mass
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::experiment::{ExperimentManifest, FactorDiff, ObservableRegistry};
use anima_engine_lib::core::experiment_runner::{
    run_manifest_seed, run_paired_ensemble_with_control, ExperimentModel, PairedEnsembleReport,
    RunResult, RunStatus,
};
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LiveWorldConfig, LIVE_MODEL_VERSION,
};
use anima_engine_lib::core::resources::SimRng;
use anima_engine_lib::core::sha256::sha256_file;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use rand::RngCore;

const PREREG_FILE: &str = "e3-preregistration.json";
const ALLOWED_INTERVENTION_PATH: &str = "interventions";
const ALLOWED_SCARCITY_FACTOR_PATHS: [&str; 2] = ["initial_conditions", "interventions"];
const DEFOREST_CAUSE_ID: u32 = 1;
const ADD_NUTRIENT_CAUSE_ID: u32 = 2;
const NUM_ARMS: usize = 3;
/// WP-2 (b1): minimum post-warmup relative shift in `live.standing_crop` vs uniform control.
const WP2_B1_MIN_REL_SHIFT: f64 = 0.10;
/// WP-2 (b2): maximum post-warmup relative total standing-crop mismatch between scarcity control
/// and gradient treatment.
const WP2_B2_MAX_REL_MISMATCH: f64 = 0.10;

/// Non-mass observables written to `smoke-series.csv` (strictly blind to `trait_mass_*`).
const SMOKE_SERIES_OBSERVABLES: [&str; 21] = [
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
];

struct Args {
    manifest_dir: PathBuf,
    out: PathBuf,
    duration_rung: Option<u64>,
}

struct Plan {
    seeds: Vec<u64>,
    smoke_seed: u64,
    ladder: Vec<u64>,
    sample_period: u64,
    warmup_ticks: u64,
    tail_window: [u64; 2],
    max_wall_clock_minutes: f64,
    control_file: String,
    control_scarcity_file: String,
    treatment_file: String,
    smoke_control_file: String,
    smoke_control_scarcity_file: String,
    smoke_treatment_file: String,
}

struct FieldProbe {
    world_identity: WorldIdentity,
    rng_seed: u64,
    rng_stream_pos: u128,
    rng_next_draws: Vec<u64>,
    resource_field_present: bool,
    width: usize,
    height: usize,
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
    initial_total_biomass: f64,
    initial_total_capacity: f64,
    initial_west_biomass: f64,
    initial_west_capacity: f64,
    initial_east_biomass: f64,
    initial_east_capacity: f64,
    ecosystem_plants: f64,
    ecosystem_detritus: f64,
    ecosystem_animals: f64,
    ecosystem_dormant: f64,
}

struct ArmSummary {
    arm: &'static str,
    status: &'static str,
    final_checksum: u32,
    standing_crop_final: f64,
    standing_crop_post_warmup_mean: f64,
    standing_crop_tail_mean: f64,
    standing_crop_series: Vec<(u64, f64)>,
    detritus_early_samples: Vec<(u64, f64)>,
    first_positive_detritus_before_3000: Option<(u64, f64)>,
    detritus_at_3000: Option<f64>,
    detritus_final: f64,
    closed_eu_first: f64,
    closed_eu_min: f64,
    closed_eu_max: f64,
    closed_eu_max_abs_drift: f64,
    agent_count_min: f64,
    agent_count_max: f64,
    agent_count_final: f64,
    herbivore_count_final: f64,
    predator_count_final: f64,
    births_sum: f64,
    deaths_sum: f64,
    deaths_starvation_sum: f64,
    deaths_dehydration_sum: f64,
    deforest_ledger_count: usize,
    deforest_eu_moved: f64,
    add_nutrient_ledger_count: usize,
    add_nutrient_eu_added: f64,
    emitted_count: usize,
    all_finite: bool,
}

struct SmokeCalibMetrics {
    wp2_a_passed: bool,
    wp2_b1_passed: bool,
    wp2_b2_passed: bool,
    wp2_b_passed: bool,
    wp2_c_passed: bool,
    final_delta_treat_vs_ctrl: f64,
    post_warmup_delta_treat_vs_ctrl: f64,
    post_warmup_rel_shift_treat_vs_ctrl: Option<f64>,
    tail_delta_treat_vs_ctrl: f64,
    max_abs_sample_diff_treat_vs_ctrl: f64,
    final_delta_treat_vs_scarcity: f64,
    post_warmup_delta_treat_vs_scarcity: f64,
    post_warmup_rel_mismatch_scarcity_vs_treat: Option<f64>,
    tail_delta_treat_vs_scarcity: f64,
    max_abs_sample_diff_treat_vs_scarcity: f64,
}

struct ProvenanceContext<'a> {
    args: &'a Args,
    plan: &'a Plan,
    control: &'a ExperimentManifest,
    scarcity: &'a ExperimentManifest,
    treatment: &'a ExperimentManifest,
    registry: &'a ObservableRegistry,
    paired_report: &'a PairedEnsembleReport,
    scarcity_fingerprint: u64,
    probe: &'a FieldProbe,
    input_hashes: &'a BTreeMap<String, String>,
    prereg_hash_at_run: &'a str,
    rung: u64,
    started: SystemTime,
    ended: SystemTime,
    elapsed_secs: f64,
    us_per_tick: f64,
    projected_by_rung: &'a [(u64, f64)],
    recommended_rung: Option<u64>,
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run_with_args(argv) {
        eprintln!("E3 smoke refused or failed: {e}");
        std::process::exit(2);
    }
}

fn run_with_args(argv: Vec<String>) -> Result<(), String> {
    let args = parse_args(argv)?;
    let registry = ObservableRegistry::live_default();
    registry
        .validate()
        .map_err(|e| format!("live registry invalid: {e}"))?;

    let prereg_path = args.manifest_dir.join(PREREG_FILE);
    let prereg = read_json(&prereg_path)?;
    let plan = Plan::from_prereg(&prereg)?;

    let mut control = load_manifest(&args.manifest_dir.join(&plan.smoke_control_file))?;
    let mut scarcity = load_manifest(&args.manifest_dir.join(&plan.smoke_control_scarcity_file))?;
    let mut treatment = load_manifest(&args.manifest_dir.join(&plan.smoke_treatment_file))?;

    validate_smoke_manifests(&plan, &registry, &control, &scarcity, &treatment)?;

    let rung = match args.duration_rung {
        Some(t) => {
            if !plan.ladder.contains(&t) {
                return Err(format!(
                    "--duration-rung {t} is not on preregistered ladder {:?}",
                    plan.ladder
                ));
            }
            for m in [&mut control, &mut scarcity, &mut treatment] {
                m.duration_ticks = t;
                for cmd in &mut m.interventions {
                    cmd.duration_ticks = t;
                }
            }
            t
        }
        None => control.duration_ticks,
    };

    // Hash the six immutable manifest inputs and this smoke source file (NOT
    // `e3-preregistration.json`, which records `wp2_smoke_calibration` after this run, avoiding
    // any circular self-hash).
    let mut input_hashes: BTreeMap<String, String> = BTreeMap::new();
    for file in [
        plan.control_file.as_str(),
        plan.control_scarcity_file.as_str(),
        plan.treatment_file.as_str(),
        plan.smoke_control_file.as_str(),
        plan.smoke_control_scarcity_file.as_str(),
        plan.smoke_treatment_file.as_str(),
    ] {
        let digest =
            sha256_file(&args.manifest_dir.join(file)).map_err(|e| format!("hash {file}: {e}"))?;
        input_hashes.insert(file.to_string(), digest);
    }
    let smoke_src_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/e3_smoke.rs");
    let smoke_src_digest = sha256_file(&smoke_src_path)
        .map_err(|e| format!("hash {}: {e}", smoke_src_path.display()))?;
    input_hashes.insert(
        "src-tauri/examples/e3_smoke.rs".to_string(),
        smoke_src_digest,
    );
    let prereg_hash_at_run =
        sha256_file(&prereg_path).map_err(|e| format!("hash {PREREG_FILE}: {e}"))?;

    let out_dir = args.out.join("smoke");
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;

    // WP-2 (a): direct headless genesis probe before any ticks step, across all three arms.
    let probe_control = probe_genesis(&control, plan.smoke_seed)?;
    let probe_scarcity = probe_genesis(&scarcity, plan.smoke_seed)?;
    let probe_treatment = probe_genesis(&treatment, plan.smoke_seed)?;

    let world_identity_identical = probe_control.world_identity == probe_treatment.world_identity
        && probe_control.world_identity == probe_scarcity.world_identity;
    let ecology_stream_identical = probe_control.rng_seed == probe_treatment.rng_seed
        && probe_control.rng_stream_pos == probe_treatment.rng_stream_pos
        && probe_control.rng_next_draws == probe_treatment.rng_next_draws
        && probe_control.rng_seed == probe_scarcity.rng_seed
        && probe_control.rng_stream_pos == probe_scarcity.rng_stream_pos
        && probe_control.rng_next_draws == probe_scarcity.rng_next_draws;
    let total_eu_ctrl = probe_control.total_ecosystem_eu();
    let total_eu_scarcity = probe_scarcity.total_ecosystem_eu();
    let total_eu_treat = probe_treatment.total_ecosystem_eu();
    let genesis_closed_eu_identical = (total_eu_ctrl - total_eu_scarcity).abs() < 1e-6
        && (total_eu_ctrl - total_eu_treat).abs() < 1e-6;

    if !world_identity_identical {
        return Err("WP-2 integrity failed: world_identity differs among smoke arms".into());
    }
    if !ecology_stream_identical {
        return Err(
            "WP-2 integrity failed: SimRng state differs among smoke arms at genesis".into(),
        );
    }
    if !genesis_closed_eu_identical {
        return Err(format!(
            "WP-2 integrity failed: initial closed EU differs across arms (ctrl={total_eu_ctrl}, scarcity={total_eu_scarcity}, treat={total_eu_treat})"
        ));
    }

    let wp2_a_passed = probe_control.resource_field_present
        && probe_scarcity.resource_field_present
        && probe_treatment.resource_field_present
        && probe_control.width == 256
        && probe_control.height == 256
        && probe_control.initial_total_biomass > 0.0
        && probe_control.initial_total_capacity > 0.0
        && (probe_scarcity.initial_total_capacity / probe_control.initial_total_capacity - 0.42)
            .abs()
            < 1e-4;

    say(&format!(
        "E3 smoke: seed {}, {NUM_ARMS} arms x {} ticks (sample period {}), profile {}",
        plan.smoke_seed,
        rung,
        control.sample_period,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    ))?;
    say(&format!(
        "  WP-2 (a) ResourceField present: {} ({}x{}, abundant cap {:.3} EU, scarcity cap {:.3} EU, detritus_0 {:.3} EU)",
        wp2_a_passed,
        probe_control.width,
        probe_control.height,
        probe_control.initial_total_capacity,
        probe_scarcity.initial_total_capacity,
        probe_scarcity.ecosystem_detritus
    ))?;

    // WP-2 (b) & (c): run all three arms under the shared runner/single-seed entry points.
    let started = SystemTime::now();
    let timer = Instant::now();
    let paired_report = run_paired_ensemble_with_control::<LiveExperimentAdapter>(
        &control,
        &treatment,
        &registry,
        &FactorDiff {
            allowed_paths: vec![ALLOWED_INTERVENTION_PATH.to_string()],
        },
    )
    .map_err(|e| format!("run_paired_ensemble_with_control failed: {e}"))?;
    let scarcity_run = run_manifest_seed::<LiveExperimentAdapter>(
        &scarcity,
        &registry,
        plan.smoke_seed,
        None,
        None,
    );
    let elapsed_secs = timer.elapsed().as_secs_f64();
    let ended = SystemTime::now();

    let pair = paired_report
        .pairs
        .first()
        .ok_or_else(|| "paired report contains no pairs".to_string())?;
    if !pair.is_complete() || !scarcity_run.status.is_completed() {
        return Err(format!(
            "one or more smoke arms failed: control={:?}, scarcity={:?}, treatment={:?}",
            pair.control.status, scarcity_run.status, pair.treatment.status
        ));
    }

    let ctrl_summary = summarize_arm("control_uniform", &pair.control, &plan)?;
    let scarcity_summary = summarize_arm("control_uniform_scarcity", &scarcity_run, &plan)?;
    let treat_summary = summarize_arm("treatment_gradient", &pair.treatment, &plan)?;

    let required_min_emitted = control.observable_ids.len();
    for a in [&ctrl_summary, &scarcity_summary, &treat_summary] {
        if a.emitted_count < required_min_emitted {
            return Err(format!(
                "{}: expected at least {required_min_emitted} emitted observables, got {}",
                a.arm, a.emitted_count
            ));
        }
        if !a.all_finite {
            return Err(format!("{}: non-finite observable in smoke run", a.arm));
        }
    }

    let (max_abs_diff_uc, final_delta_uc, post_warmup_delta_uc, tail_delta_uc) =
        standing_crop_diffs(&ctrl_summary, &treat_summary);
    let (max_abs_diff_sc, final_delta_sc, post_warmup_delta_sc, tail_delta_sc) =
        standing_crop_diffs(&scarcity_summary, &treat_summary);

    let post_warmup_rel_shift_treat_vs_ctrl = rel_ratio(
        post_warmup_delta_uc,
        ctrl_summary.standing_crop_post_warmup_mean,
    );
    let post_warmup_rel_mismatch_scarcity_vs_treat = rel_ratio(
        post_warmup_delta_sc,
        treat_summary.standing_crop_post_warmup_mean,
    );

    let wp2_b1_passed = ctrl_summary.final_checksum != treat_summary.final_checksum
        && treat_summary.deforest_ledger_count > 0
        && treat_summary.add_nutrient_ledger_count > 0
        && post_warmup_rel_shift_treat_vs_ctrl.is_some_and(|r| r.abs() >= WP2_B1_MIN_REL_SHIFT);

    let wp2_b2_passed = scarcity_summary.final_checksum != treat_summary.final_checksum
        && scarcity_summary.deforest_ledger_count == 0
        && scarcity_summary.add_nutrient_ledger_count > 0
        && post_warmup_rel_mismatch_scarcity_vs_treat
            .is_some_and(|r| r.abs() <= WP2_B2_MAX_REL_MISMATCH);

    let wp2_b_passed = wp2_b1_passed && wp2_b2_passed;

    let wp2_c_passed = [&ctrl_summary, &scarcity_summary, &treat_summary]
        .iter()
        .all(|a| {
            a.first_positive_detritus_before_3000.is_some()
                && a.detritus_at_3000.is_some_and(|v| v > 0.0)
        });

    let metrics = SmokeCalibMetrics {
        wp2_a_passed,
        wp2_b1_passed,
        wp2_b2_passed,
        wp2_b_passed,
        wp2_c_passed,
        final_delta_treat_vs_ctrl: final_delta_uc,
        post_warmup_delta_treat_vs_ctrl: post_warmup_delta_uc,
        post_warmup_rel_shift_treat_vs_ctrl,
        tail_delta_treat_vs_ctrl: tail_delta_uc,
        max_abs_sample_diff_treat_vs_ctrl: max_abs_diff_uc,
        final_delta_treat_vs_scarcity: final_delta_sc,
        post_warmup_delta_treat_vs_scarcity: post_warmup_delta_sc,
        post_warmup_rel_mismatch_scarcity_vs_treat,
        tail_delta_treat_vs_scarcity: tail_delta_sc,
        max_abs_sample_diff_treat_vs_scarcity: max_abs_diff_sc,
    };

    let planned_ensemble_runs = (NUM_ARMS * plan.seeds.len()) as f64;
    let us_per_tick = elapsed_secs * 1e6 / ((NUM_ARMS as f64) * (rung as f64));
    let projected_by_rung: Vec<(u64, f64)> = plan
        .ladder
        .iter()
        .map(|&t| {
            let mins = planned_ensemble_runs * (t as f64) * us_per_tick / 60_000_000.0;
            (t, mins)
        })
        .collect();
    let recommended_rung = projected_by_rung
        .iter()
        .find(|(_, mins)| *mins <= plan.max_wall_clock_minutes)
        .map(|(t, _)| *t);

    say(&format!(
        "  WP-2 (b1) gradient vs abundant standing_crop: {} (post-warmup delta {:+.3} EU / {:+.2}%)",
        wp2_b1_passed,
        post_warmup_delta_uc,
        post_warmup_rel_shift_treat_vs_ctrl.unwrap_or(0.0) * 100.0
    ))?;
    say(&format!(
        "  WP-2 (b2) scarcity control vs treatment total match: {} (post-warmup delta {:+.3} EU / {:+.2}%, agents min/final ctrl_s={}/{})",
        wp2_b2_passed,
        post_warmup_delta_sc,
        post_warmup_rel_mismatch_scarcity_vs_treat.unwrap_or(0.0) * 100.0,
        scarcity_summary.agent_count_min,
        scarcity_summary.agent_count_final
    ))?;
    say(&format!(
        "  WP-2 (c) detritus > 0 before tick 3000 (all 3 arms): {}",
        wp2_c_passed
    ))?;
    say(&format!(
        "  WP-3 timing: {:.2}s ({:.1} us/tick), {}x18000 projection {:.2} min (recommended rung {:?})",
        elapsed_secs,
        us_per_tick,
        NUM_ARMS * plan.seeds.len(),
        projected_by_rung
            .iter()
            .find(|(t, _)| *t == 18_000)
            .map(|(_, m)| *m)
            .unwrap_or(0.0),
        recommended_rung
    ))?;

    let arm_summaries = [&ctrl_summary, &scarcity_summary, &treat_summary];
    let runs_csv = build_runs_csv(&arm_summaries, plan.smoke_seed, us_per_tick);
    let series_csv = build_series_csv(
        &[
            ("control_uniform", &pair.control),
            ("control_uniform_scarcity", &scarcity_run),
            ("treatment_gradient", &pair.treatment),
        ],
        plan.smoke_seed,
    )?;

    let report_json = pretty(&build_report_json(
        &plan,
        rung,
        control.sample_period,
        elapsed_secs,
        us_per_tick,
        planned_ensemble_runs as usize,
        &probe_control,
        &probe_scarcity,
        &probe_treatment,
        &ctrl_summary,
        &scarcity_summary,
        &treat_summary,
        &metrics,
        &projected_by_rung,
        recommended_rung,
        world_identity_identical,
        ecology_stream_identical,
        genesis_closed_eu_identical,
    ))? + "\n";

    let prov_ctx = ProvenanceContext {
        args: &args,
        plan: &plan,
        control: &control,
        scarcity: &scarcity,
        treatment: &treatment,
        registry: &registry,
        paired_report: &paired_report,
        scarcity_fingerprint: scarcity.fingerprint(),
        probe: &probe_control,
        input_hashes: &input_hashes,
        prereg_hash_at_run: &prereg_hash_at_run,
        rung,
        started,
        ended,
        elapsed_secs,
        us_per_tick,
        projected_by_rung: &projected_by_rung,
        recommended_rung,
    };
    let provenance_json = pretty(&build_provenance(&prov_ctx))? + "\n";

    let summary_md = build_summary_md(
        &plan,
        rung,
        control.sample_period,
        elapsed_secs,
        us_per_tick,
        planned_ensemble_runs as usize,
        &projected_by_rung,
        recommended_rung,
        &probe_control,
        &probe_scarcity,
        &arm_summaries,
        &metrics,
    );

    for (name, content) in [
        ("provenance.json", &provenance_json),
        ("runs.csv", &runs_csv),
        ("smoke-report.json", &report_json),
        ("smoke-series.csv", &series_csv),
        ("summary.md", &summary_md),
    ] {
        guard_no_body_mass(name, content)?;
    }

    write_text(&out_dir.join("provenance.json"), &provenance_json)?;
    write_text(&out_dir.join("runs.csv"), &runs_csv)?;
    write_text(&out_dir.join("smoke-report.json"), &report_json)?;
    write_text(&out_dir.join("smoke-series.csv"), &series_csv)?;
    write_text(&out_dir.join("summary.md"), &summary_md)?;

    let mut checksums = String::new();
    for name in [
        "provenance.json",
        "runs.csv",
        "smoke-report.json",
        "smoke-series.csv",
        "summary.md",
    ] {
        let digest = sha256_file(&out_dir.join(name)).map_err(|e| format!("hash {name}: {e}"))?;
        checksums.push_str(&format!("{digest}  {name}\n"));
    }
    write_text(&out_dir.join("checksums.sha256"), &checksums)?;
    say(&format!(
        "  smoke artifacts written to {}",
        out_dir.display()
    ))?;

    Ok(())
}

impl FieldProbe {
    fn total_ecosystem_eu(&self) -> f64 {
        self.ecosystem_plants
            + self.ecosystem_detritus
            + self.ecosystem_animals
            + self.ecosystem_dormant
    }
}

fn rel_ratio(delta: f64, denom: f64) -> Option<f64> {
    if denom != 0.0 {
        Some(delta / denom.abs())
    } else {
        None
    }
}

fn standing_crop_diffs(base: &ArmSummary, other: &ArmSummary) -> (f64, f64, f64, f64) {
    let max_abs = base
        .standing_crop_series
        .iter()
        .zip(&other.standing_crop_series)
        .map(|((tb, vb), (to, vo))| {
            assert_eq!(tb, to);
            (vo - vb).abs()
        })
        .fold(0.0f64, f64::max);
    let final_delta = other.standing_crop_final - base.standing_crop_final;
    let post_warmup_delta =
        other.standing_crop_post_warmup_mean - base.standing_crop_post_warmup_mean;
    let tail_delta = other.standing_crop_tail_mean - base.standing_crop_tail_mean;
    (max_abs, final_delta, post_warmup_delta, tail_delta)
}

fn validate_smoke_manifests(
    plan: &Plan,
    registry: &ObservableRegistry,
    control: &ExperimentManifest,
    scarcity: &ExperimentManifest,
    treatment: &ExperimentManifest,
) -> Result<(), String> {
    for (label, m) in [
        ("control_uniform", control),
        ("control_uniform_scarcity", scarcity),
        ("treatment_gradient", treatment),
    ] {
        m.validate(registry)
            .map_err(|e| format!("{label} manifest invalid: {e}"))?;
        let cfg = LiveWorldConfig::from_initial_conditions(&m.initial_conditions)
            .map_err(|e| format!("{label} LiveWorldConfig invalid: {e}"))?;
        cfg.ensure_observables_supported(&m.observable_ids)
            .map_err(|e| format!("{label} observables unsupported: {e}"))?;
        if m.sample_period != plan.sample_period {
            return Err(format!(
                "{label} must declare sample_period={}, got {}",
                plan.sample_period, m.sample_period
            ));
        }
        if m.seeds != vec![plan.smoke_seed] {
            return Err(format!(
                "{label} must declare only [{}], got {:?}",
                plan.smoke_seed, m.seeds
            ));
        }
        if m.seeds.iter().any(|s| plan.seeds.contains(s)) {
            return Err(format!("{label} contains an experimental seed"));
        }
    }

    FactorDiff {
        allowed_paths: vec![ALLOWED_INTERVENTION_PATH.to_string()],
    }
    .validate(control, treatment)
    .map_err(|e| format!("FactorDiff (control_uniform vs treatment) failed: {e}"))?;

    let scarcity_allowed = FactorDiff {
        allowed_paths: ALLOWED_SCARCITY_FACTOR_PATHS
            .iter()
            .map(|s| s.to_string())
            .collect(),
    };
    scarcity_allowed
        .validate(control, scarcity)
        .map_err(|e| format!("FactorDiff (control_uniform vs control_scarcity) failed: {e}"))?;
    scarcity_allowed
        .validate(scarcity, treatment)
        .map_err(|e| format!("FactorDiff (control_scarcity vs treatment) failed: {e}"))?;
    Ok(())
}

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

    let (
        resource_field_present,
        width,
        height,
        min_x,
        max_x,
        min_z,
        max_z,
        initial_total_biomass,
        initial_total_capacity,
        initial_west_biomass,
        initial_west_capacity,
        initial_east_biomass,
        initial_east_capacity,
    ) = match world.get_resource::<ResourceField>() {
        Some(f) => {
            let mut total_r = 0.0f64;
            let mut total_cap = 0.0f64;
            let mut west_r = 0.0f64;
            let mut west_cap = 0.0f64;
            let mut east_r = 0.0f64;
            let mut east_cap = 0.0f64;
            for y in 0..f.height {
                for x in 0..f.width {
                    let i = y * f.width + x;
                    let r = f.r[i] as f64;
                    let cap = f.r_max[i] as f64;
                    total_r += r;
                    total_cap += cap;
                    if x < f.width / 2 {
                        west_r += r;
                        west_cap += cap;
                    } else {
                        east_r += r;
                        east_cap += cap;
                    }
                }
            }
            (
                true, f.width, f.height, f.min_x, f.max_x, f.min_z, f.max_z, total_r, total_cap,
                west_r, west_cap, east_r, east_cap,
            )
        }
        None => (
            false, 0, 0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ),
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
        min_x,
        max_x,
        min_z,
        max_z,
        initial_total_biomass,
        initial_total_capacity,
        initial_west_biomass,
        initial_west_capacity,
        initial_east_biomass,
        initial_east_capacity,
        ecosystem_plants: bio.plants,
        ecosystem_detritus: bio.detritus,
        ecosystem_animals: bio.animals,
        ecosystem_dormant: bio.dormant,
    })
}

fn summarize_arm(arm: &'static str, run: &RunResult, plan: &Plan) -> Result<ArmSummary, String> {
    let status = match &run.status {
        RunStatus::Completed => "completed",
        RunStatus::Failed { .. } => "failed",
    };

    let all_finite = run
        .series
        .iter()
        .all(|s| s.observables.iter().all(|(_, v)| v.is_finite()))
        && run.final_observables.iter().all(|(_, v)| v.is_finite());

    let emitted_count = run.final_observables.len();

    let obs_at = |sample: &[(String, f64)], key: &str| -> Result<f64, String> {
        sample
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| *v)
            .ok_or_else(|| format!("{arm}: missing observable '{key}'"))
    };

    let standing_crop_final = obs_at(&run.final_observables, "live.standing_crop")?;
    let detritus_final = obs_at(&run.final_observables, "detritus")?;

    let mut standing_crop_series = Vec::with_capacity(run.series.len());
    let mut detritus_early_samples = Vec::new();
    let mut first_positive_detritus_before_3000 = None;
    let mut detritus_at_3000 = None;
    let mut closed_eu_vals = Vec::with_capacity(run.series.len());
    let mut agent_counts = Vec::with_capacity(run.series.len());
    let mut births_sum = 0.0f64;
    let mut deaths_sum = 0.0f64;
    let mut deaths_starvation_sum = 0.0f64;
    let mut deaths_dehydration_sum = 0.0f64;

    for sample in &run.series {
        let sc = obs_at(&sample.observables, "live.standing_crop")?;
        let det = obs_at(&sample.observables, "detritus")?;
        let eu = obs_at(&sample.observables, "live.closed_eu_total")?;
        let ag = obs_at(&sample.observables, "live.agent_count")?;
        births_sum += obs_at(&sample.observables, "live.births")?;
        deaths_sum += obs_at(&sample.observables, "live.deaths")?;
        deaths_starvation_sum += obs_at(&sample.observables, "live.deaths_starvation")?;
        deaths_dehydration_sum += obs_at(&sample.observables, "live.deaths_dehydration")?;

        standing_crop_series.push((sample.tick, sc));
        if sample.tick <= 3_000 {
            detritus_early_samples.push((sample.tick, det));
        }
        if sample.tick < 3_000 && det > 0.0 && first_positive_detritus_before_3000.is_none() {
            first_positive_detritus_before_3000 = Some((sample.tick, det));
        }
        if sample.tick == 3_000 {
            detritus_at_3000 = Some(det);
        }
        closed_eu_vals.push(eu);
        agent_counts.push(ag);
    }

    let post_warmup: Vec<f64> = standing_crop_series
        .iter()
        .filter(|(t, _)| *t > plan.warmup_ticks)
        .map(|(_, v)| *v)
        .collect();
    let standing_crop_post_warmup_mean = if post_warmup.is_empty() {
        0.0
    } else {
        post_warmup.iter().sum::<f64>() / post_warmup.len() as f64
    };

    let tail: Vec<f64> = standing_crop_series
        .iter()
        .filter(|(t, _)| *t >= plan.tail_window[0] && *t <= plan.tail_window[1])
        .map(|(_, v)| *v)
        .collect();
    let standing_crop_tail_mean = if tail.is_empty() {
        0.0
    } else {
        tail.iter().sum::<f64>() / tail.len() as f64
    };

    let closed_eu_first = closed_eu_vals.first().copied().unwrap_or(0.0);
    let closed_eu_min = closed_eu_vals.iter().copied().fold(f64::INFINITY, f64::min);
    let closed_eu_max = closed_eu_vals
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let closed_eu_max_abs_drift = closed_eu_vals
        .iter()
        .map(|v| (*v - closed_eu_first).abs())
        .fold(0.0f64, f64::max);

    let agent_count_min = agent_counts.iter().copied().fold(f64::INFINITY, f64::min);
    let agent_count_max = agent_counts
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let agent_count_final = obs_at(&run.final_observables, "live.agent_count")?;
    let herbivore_count_final = obs_at(&run.final_observables, "live.herbivore_count")?;
    let predator_count_final = obs_at(&run.final_observables, "live.predator_count")?;

    let mut deforest_ledger_count = 0usize;
    let mut deforest_eu_moved = 0.0f64;
    let mut add_nutrient_ledger_count = 0usize;
    let mut add_nutrient_eu_added = 0.0f64;
    for rec in run.ledger.all() {
        if rec.cause_id == DEFOREST_CAUSE_ID && rec.target == "plants" {
            deforest_ledger_count += 1;
            deforest_eu_moved += rec.delta.abs();
        } else if rec.cause_id == ADD_NUTRIENT_CAUSE_ID && rec.target == "plants" {
            add_nutrient_ledger_count += 1;
            add_nutrient_eu_added += rec.delta;
        }
    }

    Ok(ArmSummary {
        arm,
        status,
        final_checksum: run.final_checksum,
        standing_crop_final,
        standing_crop_post_warmup_mean,
        standing_crop_tail_mean,
        standing_crop_series,
        detritus_early_samples,
        first_positive_detritus_before_3000,
        detritus_at_3000,
        detritus_final,
        closed_eu_first,
        closed_eu_min,
        closed_eu_max,
        closed_eu_max_abs_drift,
        agent_count_min,
        agent_count_max,
        agent_count_final,
        herbivore_count_final,
        predator_count_final,
        births_sum,
        deaths_sum,
        deaths_starvation_sum,
        deaths_dehydration_sum,
        deforest_ledger_count,
        deforest_eu_moved,
        add_nutrient_ledger_count,
        add_nutrient_eu_added,
        emitted_count,
        all_finite,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_report_json(
    plan: &Plan,
    rung: u64,
    sample_period: u64,
    elapsed_secs: f64,
    us_per_tick: f64,
    planned_ensemble_runs: usize,
    probe_control: &FieldProbe,
    probe_scarcity: &FieldProbe,
    probe_treatment: &FieldProbe,
    ctrl: &ArmSummary,
    scarcity: &ArmSummary,
    treat: &ArmSummary,
    metrics: &SmokeCalibMetrics,
    projected_by_rung: &[(u64, f64)],
    recommended_rung: Option<u64>,
    world_identity_identical: bool,
    ecology_stream_identical: bool,
    genesis_closed_eu_identical: bool,
) -> serde_json::Value {
    let mut root = serde_json::Map::new();
    root.insert(
        "kind".into(),
        serde_json::json!("e3-smoke-report-EXCLUDED-FROM-ANALYSIS"),
    );
    root.insert("smoke_seed".into(), serde_json::json!(plan.smoke_seed));
    root.insert(
        "arms".into(),
        serde_json::json!([ctrl.arm, scarcity.arm, treat.arm]),
    );
    root.insert("duration_ticks".into(), serde_json::json!(rung));
    root.insert("sample_period".into(), serde_json::json!(sample_period));
    root.insert("warmup_ticks".into(), serde_json::json!(plan.warmup_ticks));
    root.insert(
        "tail_window_ticks".into(),
        serde_json::json!(plan.tail_window),
    );
    root.insert("body_mass_consulted".into(), serde_json::json!(false));
    root.insert(
        "all_observables_emitted_and_finite".into(),
        serde_json::json!(true),
    );
    root.insert(
        "integrity".into(),
        serde_json::json!({
            "world_identity_identical": world_identity_identical,
            "ecology_stream_identical": ecology_stream_identical,
            "genesis_closed_eu_identical": genesis_closed_eu_identical,
            "control_uniform_final_checksum": ctrl.final_checksum,
            "control_uniform_scarcity_final_checksum": scarcity.final_checksum,
            "treatment_gradient_final_checksum": treat.final_checksum,
            "all_three_checksums_distinct": ctrl.final_checksum != treat.final_checksum
                && scarcity.final_checksum != treat.final_checksum
                && ctrl.final_checksum != scarcity.final_checksum,
        }),
    );
    root.insert(
        "wp2_a_resource_field".into(),
        wp2_a_json(
            metrics.wp2_a_passed,
            probe_control,
            probe_scarcity,
            probe_treatment,
        ),
    );
    root.insert(
        "wp2_b_standing_crop_intervention".into(),
        wp2_b_json(ctrl, scarcity, treat, metrics),
    );
    root.insert(
        "wp2_c_detritus_before_3000".into(),
        wp2_c_json(metrics.wp2_c_passed, ctrl, scarcity, treat),
    );
    root.insert(
        "demographics_and_conservation".into(),
        serde_json::json!({
            "control_uniform": arm_diag_json(ctrl),
            "control_uniform_scarcity": arm_diag_json(scarcity),
            "treatment_gradient": arm_diag_json(treat),
        }),
    );
    root.insert(
        "wp3_schedule_lock".into(),
        serde_json::json!({
            "wall_clock_seconds": elapsed_secs,
            "us_per_tick": us_per_tick,
            "planned_ensemble_runs": planned_ensemble_runs,
            "projected_ensemble_minutes_by_rung": projected_by_rung
                .iter()
                .map(|(t, mins)| serde_json::json!({ "duration_ticks": t, "projected_minutes": mins }))
                .collect::<Vec<_>>(),
            "recommended_duration_ticks": recommended_rung,
            "warmup_ticks": plan.warmup_ticks,
            "sample_period": sample_period,
            "tail_window_ticks": plan.tail_window,
            "within_budget": recommended_rung == Some(rung),
        }),
    );
    serde_json::Value::Object(root)
}

fn wp2_a_json(
    passed: bool,
    probe_control: &FieldProbe,
    probe_scarcity: &FieldProbe,
    probe_treatment: &FieldProbe,
) -> serde_json::Value {
    serde_json::json!({
        "passed": passed,
        "width": probe_control.width,
        "height": probe_control.height,
        "bounds_x": [probe_control.min_x, probe_control.max_x],
        "bounds_z": [probe_control.min_z, probe_control.max_z],
        "abundant_initial_total_biomass_eu": probe_control.initial_total_biomass,
        "abundant_initial_total_capacity_eu": probe_control.initial_total_capacity,
        "abundant_initial_west_biomass_eu": probe_control.initial_west_biomass,
        "abundant_initial_west_capacity_eu": probe_control.initial_west_capacity,
        "abundant_initial_east_biomass_eu": probe_control.initial_east_biomass,
        "abundant_initial_east_capacity_eu": probe_control.initial_east_capacity,
        "scarcity_initial_total_biomass_eu": probe_scarcity.initial_total_biomass,
        "scarcity_initial_total_capacity_eu": probe_scarcity.initial_total_capacity,
        "scarcity_initial_west_biomass_eu": probe_scarcity.initial_west_biomass,
        "scarcity_initial_west_capacity_eu": probe_scarcity.initial_west_capacity,
        "scarcity_initial_east_biomass_eu": probe_scarcity.initial_east_biomass,
        "scarcity_initial_east_capacity_eu": probe_scarcity.initial_east_capacity,
        "scarcity_initial_detritus_eu": probe_scarcity.ecosystem_detritus,
        "treatment_initial_total_biomass_eu": probe_treatment.initial_total_biomass,
        "abundant_initial_ecosystem_biomass": {
            "plants": probe_control.ecosystem_plants,
            "detritus": probe_control.ecosystem_detritus,
            "animals": probe_control.ecosystem_animals,
            "dormant": probe_control.ecosystem_dormant,
        },
        "scarcity_initial_ecosystem_biomass": {
            "plants": probe_scarcity.ecosystem_plants,
            "detritus": probe_scarcity.ecosystem_detritus,
            "animals": probe_scarcity.ecosystem_animals,
            "dormant": probe_scarcity.ecosystem_dormant,
        }
    })
}

fn wp2_b_json(
    ctrl: &ArmSummary,
    scarcity: &ArmSummary,
    treat: &ArmSummary,
    metrics: &SmokeCalibMetrics,
) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    m.insert("passed".into(), serde_json::json!(metrics.wp2_b_passed));
    m.insert(
        "b1_gradient_shifts_vs_abundant_passed".into(),
        serde_json::json!(metrics.wp2_b1_passed),
    );
    m.insert(
        "b2_scarcity_matches_treatment_total_passed".into(),
        serde_json::json!(metrics.wp2_b2_passed),
    );
    m.insert(
        "b1_min_rel_shift_threshold".into(),
        serde_json::json!(WP2_B1_MIN_REL_SHIFT),
    );
    m.insert(
        "b2_max_rel_mismatch_threshold".into(),
        serde_json::json!(WP2_B2_MAX_REL_MISMATCH),
    );
    m.insert(
        "control_uniform_standing_crop_final".into(),
        serde_json::json!(ctrl.standing_crop_final),
    );
    m.insert(
        "control_uniform_scarcity_standing_crop_final".into(),
        serde_json::json!(scarcity.standing_crop_final),
    );
    m.insert(
        "treatment_gradient_standing_crop_final".into(),
        serde_json::json!(treat.standing_crop_final),
    );
    m.insert(
        "control_uniform_standing_crop_post_warmup_mean".into(),
        serde_json::json!(ctrl.standing_crop_post_warmup_mean),
    );
    m.insert(
        "control_uniform_scarcity_standing_crop_post_warmup_mean".into(),
        serde_json::json!(scarcity.standing_crop_post_warmup_mean),
    );
    m.insert(
        "treatment_gradient_standing_crop_post_warmup_mean".into(),
        serde_json::json!(treat.standing_crop_post_warmup_mean),
    );
    m.insert(
        "control_uniform_standing_crop_tail_mean".into(),
        serde_json::json!(ctrl.standing_crop_tail_mean),
    );
    m.insert(
        "control_uniform_scarcity_standing_crop_tail_mean".into(),
        serde_json::json!(scarcity.standing_crop_tail_mean),
    );
    m.insert(
        "treatment_gradient_standing_crop_tail_mean".into(),
        serde_json::json!(treat.standing_crop_tail_mean),
    );
    m.insert(
        "treat_vs_ctrl_final_delta".into(),
        serde_json::json!(metrics.final_delta_treat_vs_ctrl),
    );
    m.insert(
        "treat_vs_ctrl_post_warmup_mean_delta".into(),
        serde_json::json!(metrics.post_warmup_delta_treat_vs_ctrl),
    );
    m.insert(
        "treat_vs_ctrl_post_warmup_relative_shift".into(),
        serde_json::json!(metrics.post_warmup_rel_shift_treat_vs_ctrl),
    );
    m.insert(
        "treat_vs_ctrl_tail_mean_delta".into(),
        serde_json::json!(metrics.tail_delta_treat_vs_ctrl),
    );
    m.insert(
        "treat_vs_ctrl_max_abs_sample_diff".into(),
        serde_json::json!(metrics.max_abs_sample_diff_treat_vs_ctrl),
    );
    m.insert(
        "treat_vs_scarcity_final_delta".into(),
        serde_json::json!(metrics.final_delta_treat_vs_scarcity),
    );
    m.insert(
        "treat_vs_scarcity_post_warmup_mean_delta".into(),
        serde_json::json!(metrics.post_warmup_delta_treat_vs_scarcity),
    );
    m.insert(
        "scarcity_vs_treat_post_warmup_relative_mismatch".into(),
        serde_json::json!(metrics.post_warmup_rel_mismatch_scarcity_vs_treat),
    );
    m.insert(
        "treat_vs_scarcity_tail_mean_delta".into(),
        serde_json::json!(metrics.tail_delta_treat_vs_scarcity),
    );
    m.insert(
        "treat_vs_scarcity_max_abs_sample_diff".into(),
        serde_json::json!(metrics.max_abs_sample_diff_treat_vs_scarcity),
    );
    m.insert(
        "ledger_by_arm".into(),
        serde_json::json!({
            "control_uniform": arm_ledger_json(ctrl),
            "control_uniform_scarcity": arm_ledger_json(scarcity),
            "treatment_gradient": arm_ledger_json(treat),
        }),
    );
    serde_json::Value::Object(m)
}

fn arm_ledger_json(a: &ArmSummary) -> serde_json::Value {
    serde_json::json!({
        "deforest_count": a.deforest_ledger_count,
        "deforest_eu_moved": a.deforest_eu_moved,
        "add_nutrient_count": a.add_nutrient_ledger_count,
        "add_nutrient_eu_added": a.add_nutrient_eu_added,
    })
}

fn wp2_c_json(
    passed: bool,
    ctrl: &ArmSummary,
    scarcity: &ArmSummary,
    treat: &ArmSummary,
) -> serde_json::Value {
    serde_json::json!({
        "passed": passed,
        "control_uniform_first_positive_before_3000": ctrl.first_positive_detritus_before_3000,
        "control_uniform_scarcity_first_positive_before_3000": scarcity.first_positive_detritus_before_3000,
        "treatment_gradient_first_positive_before_3000": treat.first_positive_detritus_before_3000,
        "control_uniform_detritus_at_3000": ctrl.detritus_at_3000,
        "control_uniform_scarcity_detritus_at_3000": scarcity.detritus_at_3000,
        "treatment_gradient_detritus_at_3000": treat.detritus_at_3000,
        "control_uniform_early_samples": ctrl.detritus_early_samples,
        "control_uniform_scarcity_early_samples": scarcity.detritus_early_samples,
        "treatment_gradient_early_samples": treat.detritus_early_samples,
        "control_uniform_detritus_final": ctrl.detritus_final,
        "control_uniform_scarcity_detritus_final": scarcity.detritus_final,
        "treatment_gradient_detritus_final": treat.detritus_final,
    })
}

fn arm_diag_json(a: &ArmSummary) -> serde_json::Value {
    serde_json::json!({
        "status": a.status,
        "final_checksum": a.final_checksum,
        "emitted_observable_count": a.emitted_count,
        "closed_eu_first": a.closed_eu_first,
        "closed_eu_min": a.closed_eu_min,
        "closed_eu_max": a.closed_eu_max,
        "closed_eu_max_abs_drift": a.closed_eu_max_abs_drift,
        "agent_count_min": a.agent_count_min,
        "agent_count_max": a.agent_count_max,
        "agent_count_final": a.agent_count_final,
        "herbivore_count_final": a.herbivore_count_final,
        "predator_count_final": a.predator_count_final,
        "births_sum": a.births_sum,
        "deaths_sum": a.deaths_sum,
        "deaths_starvation_sum": a.deaths_starvation_sum,
        "deaths_dehydration_sum": a.deaths_dehydration_sum,
    })
}

fn build_runs_csv(arms: &[&ArmSummary], seed: u64, us_per_tick: f64) -> String {
    let mut s = String::from(
        "# SMOKE CALIBRATION ONLY (seed 999979); blind to body-mass values (no body-mass value written or gated beyond finiteness check)\n\
         seed,arm,status,final_checksum,us_per_tick,deforest_ledger_count,deforest_eu_moved,\
         add_nutrient_ledger_count,add_nutrient_eu_added,standing_crop_final,\
         standing_crop_post_warmup_mean,detritus_at_3000,detritus_final,agent_count_min,\
         agent_count_final,births_sum,deaths_sum,closed_eu_max_abs_drift\n",
    );
    for a in arms {
        s.push_str(&format!(
            "{seed},{},{},{},{us_per_tick:.3},{},{:.6},{},{:.6},{:.6},{:.6},{},{:.6},{},{},{},{},{:.6e}\n",
            a.arm,
            a.status,
            a.final_checksum,
            a.deforest_ledger_count,
            a.deforest_eu_moved,
            a.add_nutrient_ledger_count,
            a.add_nutrient_eu_added,
            a.standing_crop_final,
            a.standing_crop_post_warmup_mean,
            a.detritus_at_3000
                .map(|v| format!("{v:.6}"))
                .unwrap_or_default(),
            a.detritus_final,
            a.agent_count_min,
            a.agent_count_final,
            a.births_sum,
            a.deaths_sum,
            a.closed_eu_max_abs_drift,
        ));
    }
    s
}

fn build_series_csv(arms: &[(&'static str, &RunResult)], seed: u64) -> Result<String, String> {
    let mut s = String::from("seed,arm,tick");
    for id in SMOKE_SERIES_OBSERVABLES {
        s.push(',');
        s.push_str(id);
    }
    s.push('\n');

    for (arm, run) in arms {
        for sample in &run.series {
            s.push_str(&format!("{seed},{arm},{}", sample.tick));
            for id in SMOKE_SERIES_OBSERVABLES {
                let val = sample
                    .observables
                    .iter()
                    .find(|(k, _)| k == id)
                    .map(|(_, v)| *v)
                    .ok_or_else(|| format!("{arm} tick {} missing '{id}'", sample.tick))?;
                s.push(',');
                s.push_str(&val.to_string());
            }
            s.push('\n');
        }
    }
    Ok(s)
}

fn build_provenance(ctx: &ProvenanceContext<'_>) -> serde_json::Value {
    let exe = std::env::current_exe().ok();
    let planned_ensemble_runs = NUM_ARMS * ctx.plan.seeds.len();
    serde_json::json!({
        "kind": "e3-smoke-provenance-EXCLUDED-FROM-ANALYSIS",
        "mode": "smoke",
        "model": "core::live_experiment::LiveExperimentAdapter",
        "model_version": LIVE_MODEL_VERSION,
        "model_status": "headless adapter verified; NOT a live-world experiment-ready claim",
        "harness": "core::experiment_runner::{run_paired_ensemble_with_control, run_manifest_seed}",
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
        "host": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
        },
        "timing": {
            "started_utc": iso8601_utc(unix_seconds(ctx.started)),
            "ended_utc": iso8601_utc(unix_seconds(ctx.ended)),
            "wall_clock_seconds": ctx.elapsed_secs,
            "us_per_tick": ctx.us_per_tick,
            "planned_ensemble_runs": planned_ensemble_runs,
            "max_wall_clock_minutes_registered": ctx.plan.max_wall_clock_minutes,
            "projected_ensemble_minutes_by_rung": ctx.projected_by_rung
                .iter()
                .map(|(t, mins)| serde_json::json!({ "duration_ticks": t, "projected_minutes": mins }))
                .collect::<Vec<_>>(),
        },
        "wp3_schedule_lock": {
            "duration_ticks_locked": ctx.recommended_rung,
            "duration_ticks_as_run": ctx.rung,
            "duration_ticks_ladder": ctx.plan.ladder,
            "sample_period": ctx.control.sample_period,
            "warmup_ticks": ctx.plan.warmup_ticks,
            "warmup_samples_excluded": ctx.plan.warmup_ticks / ctx.control.sample_period.max(1),
            "tail_window_ticks": ctx.plan.tail_window,
        },
        "manifests": {
            "manifest_dir": ctx.args.manifest_dir.display().to_string().replace('\\', "/"),
            "sha256_as_committed": ctx.input_hashes,
            "preregistration_sha256_at_run": ctx.prereg_hash_at_run,
            "preregistration_hash_note": "e3-preregistration.json is hashed separately at run time rather than inside sha256_as_committed because wp2_smoke_calibration is copied into it after this single deterministic run.",
            "control_uniform_fingerprint_as_run": ctx.paired_report.control_manifest_fingerprint,
            "control_uniform_scarcity_fingerprint_as_run": ctx.scarcity_fingerprint,
            "treatment_gradient_fingerprint_as_run": ctx.paired_report.treatment_manifest_fingerprint,
            "control_law_fingerprint": ctx.paired_report.control_law_fingerprint,
            "treatment_law_fingerprint": ctx.paired_report.treatment_law_fingerprint,
            "registry_fingerprint": ctx.paired_report.registry_fingerprint,
            "declared_factors": ["initial_conditions.live.resource_capacity_scale", "interventions"],
            "control_uniform_initial_conditions": ctx.control.initial_conditions.values,
            "control_uniform_scarcity_initial_conditions": ctx.scarcity.initial_conditions.values,
            "treatment_gradient_initial_conditions": ctx.treatment.initial_conditions.values,
            "control_uniform_interventions": ctx.control.interventions,
            "control_uniform_scarcity_interventions": ctx.scarcity.interventions,
            "treatment_gradient_interventions": ctx.treatment.interventions,
            "note": "manifest IDs and observable lists are verified by SHA-256 above so body-mass observable names stay out of the smoke directory"
        },
        "observables": {
            "manifests_requested_observable_count": ctx.control.observable_ids.len(),
            "registry_total_count": ctx.registry.specs().len(),
            "observables_written_to_smoke_series": SMOKE_SERIES_OBSERVABLES,
            "body_mass_consulted": false,
        },
        "seeds": {
            "smoke_seed": ctx.plan.smoke_seed,
            "executed": [ctx.plan.smoke_seed],
            "registered_experimental_seeds": ctx.plan.seeds,
            "experimental_seeds_run": 0,
        },
        "world_identity_observed": {
            "seed": ctx.probe.world_identity.seed,
            "generator_version": ctx.probe.world_identity.generator_version,
            "width": ctx.probe.world_identity.width,
            "height": ctx.probe.world_identity.height,
            "checksum": ctx.probe.world_identity.checksum,
        },
        "what_this_run_does_not_claim": [
            "No desktop-app run was made; no Tauri handle, window, GPU, learner or evolution thread exists in this process.",
            "Smoke seed 999979 is calibration only and is mechanically excluded from all experimental manifests.",
            "No body-mass observable value was written, printed, or used in any gate except a finiteness check (is_finite() over every emitted observable).",
        ],
    })
}

#[allow(clippy::too_many_arguments)]
fn build_summary_md(
    plan: &Plan,
    rung: u64,
    sample_period: u64,
    elapsed_secs: f64,
    us_per_tick: f64,
    planned_ensemble_runs: usize,
    projected_by_rung: &[(u64, f64)],
    recommended_rung: Option<u64>,
    probe_control: &FieldProbe,
    probe_scarcity: &FieldProbe,
    arms: &[&ArmSummary],
    metrics: &SmokeCalibMetrics,
) -> String {
    let [ctrl, scarcity, treat] = [arms[0], arms[1], arms[2]];
    let mut s = String::from("# E3 WP-2 / WP-3 Smoke Calibration Report (3-Arm Design)\n\n");
    s.push_str(
        "> Calibration only, smoke seed `999979`, EXCLUDED from every analysis, table, statistic, \
         and claim. No body-mass observable value was written, printed, or used in any gate except \
         a finiteness check (`is_finite()` over every emitted observable; `body_mass_consulted = false`).\n\n",
    );
    s.push_str(&format!(
        "Model: `core::live_experiment::LiveExperimentAdapter` (`{LIVE_MODEL_VERSION}`) -- \
         **headless adapter verified**.\n\n\
         - Smoke seed: `{}` (`{NUM_ARMS}` arms x `{rung}` ticks, `sample_period = {sample_period}`)\n\
         - Profile: `{}`\n\
         - Wall clock: **{elapsed_secs:.3} s** (`{us_per_tick:.2} us/tick`)\n\
         - Checksums: `control_uniform` `{}`, `control_uniform_scarcity` `{}`, `treatment_gradient` `{}`\n\n",
        plan.smoke_seed,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        ctrl.final_checksum,
        scarcity.final_checksum,
        treat.final_checksum,
    ));

    s.push_str("## WP-2 (a) ResourceField in headless LiveExperimentAdapter\n\n");
    s.push_str(&format!(
        "- Gate result: **{}**\n\
         - Dimensions: `{}x{}` (`x in [{:.1}, {:.1}]`, `z in [{:.1}, {:.1}]`)\n\
         - Abundant arms (`scale=1.0`) initial plants / capacity: `{:.3}` / `{:.3}` EU (West `{:.3}`, East `{:.3}`, detritus `{:.3}`)\n\
         - Uniform-scarcity arm (`scale=0.42`) initial plants / capacity: `{:.3}` / `{:.3}` EU (West `{:.3}`, East `{:.3}`, detritus `{:.3}`)\n\
         - Identical initial closed EU across all 3 arms: `{:.3}` EU\n\n",
        if metrics.wp2_a_passed { "PASS" } else { "FAIL" },
        probe_control.width,
        probe_control.height,
        probe_control.min_x,
        probe_control.max_x,
        probe_control.min_z,
        probe_control.max_z,
        probe_control.initial_total_biomass,
        probe_control.initial_total_capacity,
        probe_control.initial_west_capacity,
        probe_control.initial_east_capacity,
        probe_control.ecosystem_detritus,
        probe_scarcity.initial_total_biomass,
        probe_scarcity.initial_total_capacity,
        probe_scarcity.initial_west_capacity,
        probe_scarcity.initial_east_capacity,
        probe_scarcity.ecosystem_detritus,
        probe_control.total_ecosystem_eu(),
    ));

    s.push_str("## WP-2 (b1 & b2) Standing Crop Manipulation & Total-Scarcity Control Match\n\n");
    s.push_str(&format!(
        "- Overall WP-2 (b) gate result: **{}** (`b1`: `{}`, `b2`: `{}`)\n\n\
         | Metric | `control_uniform` | `control_uniform_scarcity` | `treatment_gradient` | `treat - ctrl_u` | `treat - ctrl_s` |\n\
         |---|---:|---:|---:|---:|---:|\n\
         | `live.standing_crop` at T={rung} | {:.3} | {:.3} | {:.3} | {:+.3} | {:+.3} |\n\
         | Post-warmup mean (`tick > {}`) | {:.3} | {:.3} | {:.3} | {:+.3} ({:+.2}%) | {:+.3} ({:+.2}%) |\n\
         | Tail window mean (`{}..={}`) | {:.3} | {:.3} | {:.3} | {:+.3} | {:+.3} |\n\
         | Max abs sample difference | -- | -- | -- | {:.3} | {:.3} |\n\
         | `Deforest` ledger (`cause_id=1`) | {} ({:.1} EU) | {} ({:.1} EU) | {} ({:.1} EU) | -- | -- |\n\
         | `AddNutrient` ledger (`cause_id=2`) | {} ({:.1} EU) | {} ({:.1} EU) | {} ({:.1} EU) | -- | -- |\n\n",
        if metrics.wp2_b_passed { "PASS" } else { "FAIL" },
        if metrics.wp2_b1_passed { "PASS" } else { "FAIL" },
        if metrics.wp2_b2_passed { "PASS" } else { "FAIL" },
        ctrl.standing_crop_final,
        scarcity.standing_crop_final,
        treat.standing_crop_final,
        metrics.final_delta_treat_vs_ctrl,
        metrics.final_delta_treat_vs_scarcity,
        plan.warmup_ticks,
        ctrl.standing_crop_post_warmup_mean,
        scarcity.standing_crop_post_warmup_mean,
        treat.standing_crop_post_warmup_mean,
        metrics.post_warmup_delta_treat_vs_ctrl,
        metrics.post_warmup_rel_shift_treat_vs_ctrl.unwrap_or(0.0) * 100.0,
        metrics.post_warmup_delta_treat_vs_scarcity,
        metrics.post_warmup_rel_mismatch_scarcity_vs_treat.unwrap_or(0.0) * 100.0,
        plan.tail_window[0],
        plan.tail_window[1],
        ctrl.standing_crop_tail_mean,
        scarcity.standing_crop_tail_mean,
        treat.standing_crop_tail_mean,
        metrics.tail_delta_treat_vs_ctrl,
        metrics.tail_delta_treat_vs_scarcity,
        metrics.max_abs_sample_diff_treat_vs_ctrl,
        metrics.max_abs_sample_diff_treat_vs_scarcity,
        ctrl.deforest_ledger_count,
        ctrl.deforest_eu_moved,
        scarcity.deforest_ledger_count,
        scarcity.deforest_eu_moved,
        treat.deforest_ledger_count,
        treat.deforest_eu_moved,
        ctrl.add_nutrient_ledger_count,
        ctrl.add_nutrient_eu_added,
        scarcity.add_nutrient_ledger_count,
        scarcity.add_nutrient_eu_added,
        treat.add_nutrient_ledger_count,
        treat.add_nutrient_eu_added,
    ));

    s.push_str("## WP-2 (c) `detritus > 0` before tick 3000 (all 3 arms)\n\n");
    s.push_str(&format!(
        "- Gate result: **{}**\n\
         - First positive `< 3000`: `control_uniform` `{:?}`, `control_uniform_scarcity` `{:?}`, `treatment_gradient` `{:?}`\n\n\
         | Tick | `control_uniform` detritus | `control_uniform_scarcity` detritus | `treatment_gradient` detritus |\n\
         |---:|---:|---:|---:|\n",
        if metrics.wp2_c_passed { "PASS" } else { "FAIL" },
        ctrl.first_positive_detritus_before_3000,
        scarcity.first_positive_detritus_before_3000,
        treat.first_positive_detritus_before_3000,
    ));
    for ((tc, vc), ((_, vs), (_, vt))) in ctrl.detritus_early_samples.iter().zip(
        scarcity
            .detritus_early_samples
            .iter()
            .zip(&treat.detritus_early_samples),
    ) {
        s.push_str(&format!("| {tc} | {vc:.3} | {vs:.3} | {vt:.3} |\n"));
    }
    s.push('\n');

    s.push_str("## WP-3 Schedule & Budget Calibration (36 Planned Runs = 3 Arms x 12 Seeds)\n\n");
    s.push_str(&format!(
        "| Ladder rung (`T`) | Projected {planned_ensemble_runs}-run minutes | <= 90 min budget |\n|---:|---:|---|\n"
    ));
    for &(t, mins) in projected_by_rung {
        s.push_str(&format!(
            "| {t} | {mins:.3} | {} |\n",
            if mins <= plan.max_wall_clock_minutes {
                "yes"
            } else {
                "no"
            }
        ));
    }
    s.push_str(&format!(
        "\n- Recommended `T`: `{:?}`\n- Warm-up window: `{}` ticks (`{}` samples excluded)\n- `sample_period`: `{sample_period}`\n- Tail window: `{:?}`\n\n",
        recommended_rung,
        plan.warmup_ticks,
        plan.warmup_ticks / sample_period.max(1),
        plan.tail_window,
    ));

    s.push_str("## Demographics & Closed-EU Diagnostics (Smoke Seed Only)\n\n");
    s.push_str(
        "| Arm | Status | `agent_count` min / max / final | Sum `births` | Sum `deaths` (starve / dehydrate) | Max `closed_eu_total` abs drift |\n\
         |---|---|---:|---:|---:|---:|\n",
    );
    for a in arms {
        s.push_str(&format!(
            "| {} | {} | {} / {} / {} | {} | {} ({} / {}) | {:.6e} |\n",
            a.arm,
            a.status,
            a.agent_count_min,
            a.agent_count_max,
            a.agent_count_final,
            a.births_sum,
            a.deaths_sum,
            a.deaths_starvation_sum,
            a.deaths_dehydration_sum,
            a.closed_eu_max_abs_drift,
        ));
    }

    s
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
        let tail_arr = arr(
            &p["duration"]["tail_window_ticks"],
            "duration.tail_window_ticks",
        )?;
        if tail_arr.len() != 2 {
            return Err("duration.tail_window_ticks must have length 2".into());
        }
        Ok(Plan {
            seeds: arr(&p["seeds"]["execution_order"], "seeds.execution_order")?,
            smoke_seed: p["seeds"]["smoke_seed"]
                .as_u64()
                .ok_or("seeds.smoke_seed must be an integer")?,
            ladder: arr(
                &p["duration"]["duration_ticks_ladder"],
                "duration.duration_ticks_ladder",
            )?,
            sample_period: p["duration"]["sample_period"]
                .as_u64()
                .ok_or("duration.sample_period must be an integer")?,
            warmup_ticks: p["duration"]["warmup_ticks"]
                .as_u64()
                .ok_or("duration.warmup_ticks must be an integer")?,
            tail_window: [tail_arr[0], tail_arr[1]],
            max_wall_clock_minutes: p["cost"]["max_wall_clock_minutes"]
                .as_f64()
                .ok_or("cost.max_wall_clock_minutes must be a number")?,
            control_file: s(&p["manifests"]["control"], "manifests.control")?,
            control_scarcity_file: s(
                &p["manifests"]["control_uniform_scarcity"],
                "manifests.control_uniform_scarcity",
            )?,
            treatment_file: s(&p["manifests"]["treatment"], "manifests.treatment")?,
            smoke_control_file: s(&p["manifests"]["smoke_control"], "manifests.smoke_control")?,
            smoke_control_scarcity_file: s(
                &p["manifests"]["smoke_control_scarcity"],
                "manifests.smoke_control_scarcity",
            )?,
            smoke_treatment_file: s(
                &p["manifests"]["smoke_treatment"],
                "manifests.smoke_treatment",
            )?,
        })
    }
}

fn parse_args(argv: Vec<String>) -> Result<Args, String> {
    let mut manifest_dir: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut duration_rung: Option<u64> = None;
    let mut it = argv.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--manifest-dir" => {
                manifest_dir = Some(PathBuf::from(
                    it.next().ok_or("--manifest-dir needs a value")?,
                ))
            }
            "--out" => out = Some(PathBuf::from(it.next().ok_or("--out needs a value")?)),
            "--duration-rung" => {
                let raw = it.next().ok_or("--duration-rung needs a value")?;
                duration_rung = Some(
                    raw.parse()
                        .map_err(|e| format!("--duration-rung must be an integer: {e}"))?,
                );
            }
            "--help" | "-h" => {
                println!(
                    "e3_smoke --manifest-dir <dir> --out <dir> [--duration-rung <ticks>]\n\
                     Build with: cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop --example e3_smoke -j 4"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    let manifest_dir = manifest_dir.ok_or("--manifest-dir is required")?;
    let out = out.ok_or("--out is required")?;
    if !manifest_dir.is_dir() {
        return Err(format!("{} is not a directory", manifest_dir.display()));
    }
    Ok(Args {
        manifest_dir,
        out,
        duration_rung,
    })
}

fn guard_no_body_mass(what: &str, text: &str) -> Result<(), String> {
    if text.contains("trait_mass") {
        Err(format!(
            "the smoke may not write a body-mass observable, but {what} would"
        ))
    } else {
        Ok(())
    }
}

fn say(line: &str) -> Result<(), String> {
    guard_no_body_mass("stdout", line)?;
    println!("{line}");
    Ok(())
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
