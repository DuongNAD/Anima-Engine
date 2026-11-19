//! The E3 smoke calibration binary (WP-2 / WP-3 of `CARD E3-P`).
//!
//! Runs ONLY smoke seed `999979` in `--release` on `e3-smoke-control-uniform.json` and
//! `e3-smoke-treatment-gradient.json`, proves WP-2 (a) headless `ResourceField` presence, (b) that
//! West `Deforest` + East `AddNutrient` genuinely change `live.standing_crop` between arms, and (c)
//! that `detritus > 0` before tick `3000`, and records WP-3's `T` and warm-up timing calibration.
//!
//! # Blinding
//!
//! No `live.trait_mass_*` observable value or string is ever read, printed, or written by this
//! binary (`guard_no_body_mass`), keeping WP-2 and WP-3 strictly blind to body-mass outcomes.
//!
//! # Build it, then execute the compiled binary — never `cargo run`
//!
//! ```text
//! cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop --example e3_smoke -j 6
//! src-tauri/target/release/examples/e3_smoke.exe \
//!   --manifest-dir src-tauri/tests/fixtures/experiments_e3 \
//!   --out artifacts/experiments/e3-productivity-gradient-body-mass
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::experiment::{ExperimentManifest, FactorDiff, ObservableRegistry};
use anima_engine_lib::core::experiment_runner::{
    run_paired_ensemble_with_control, ExperimentModel, PairedEnsembleReport, RunResult, RunStatus,
};
use anima_engine_lib::core::live_experiment::{
    all_live_observable_ids, LiveExperimentAdapter, LiveWorldConfig, LIVE_MODEL_VERSION,
};
use anima_engine_lib::core::resources::SimRng;
use anima_engine_lib::core::sha256::sha256_file;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use rand::RngCore;

const PREREG_FILE: &str = "e3-preregistration.json";
const ALLOWED_FACTOR_PATH: &str = "interventions";
const DEFOREST_CAUSE_ID: u32 = 1;
const ADD_NUTRIENT_CAUSE_ID: u32 = 2;

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
    smoke_control_file: String,
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
    let mut treatment = load_manifest(&args.manifest_dir.join(&plan.smoke_treatment_file))?;

    validate_smoke_manifests(&plan, &registry, &control, &treatment)?;

    let rung = match args.duration_rung {
        Some(t) => {
            if !plan.ladder.contains(&t) {
                return Err(format!(
                    "--duration-rung {t} is not on preregistered ladder {:?}",
                    plan.ladder
                ));
            }
            control.duration_ticks = t;
            treatment.duration_ticks = t;
            for cmd in &mut control.interventions {
                cmd.duration_ticks = t;
            }
            for cmd in &mut treatment.interventions {
                cmd.duration_ticks = t;
            }
            t
        }
        None => control.duration_ticks,
    };

    let mut input_hashes: BTreeMap<String, String> = BTreeMap::new();
    for file in [
        plan.smoke_control_file.as_str(),
        plan.smoke_treatment_file.as_str(),
        PREREG_FILE,
    ] {
        let digest =
            sha256_file(&args.manifest_dir.join(file)).map_err(|e| format!("hash {file}: {e}"))?;
        input_hashes.insert(file.to_string(), digest);
    }

    let out_dir = args.out.join("smoke");
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;

    // WP-2 (a): direct headless genesis probe before any ticks step.
    let probe_control = probe_genesis(&control, plan.smoke_seed)?;
    let probe_treatment = probe_genesis(&treatment, plan.smoke_seed)?;
    let world_identity_identical = probe_control.world_identity == probe_treatment.world_identity;
    let ecology_stream_identical = probe_control.rng_seed == probe_treatment.rng_seed
        && probe_control.rng_stream_pos == probe_treatment.rng_stream_pos
        && probe_control.rng_next_draws == probe_treatment.rng_next_draws;

    if !world_identity_identical {
        return Err(format!(
            "WP-2 integrity failed: world_identity differs between smoke arms ({:?} vs {:?})",
            probe_control.world_identity, probe_treatment.world_identity
        ));
    }
    if !ecology_stream_identical {
        return Err(
            "WP-2 integrity failed: SimRng state differs between smoke arms at genesis".into(),
        );
    }

    let wp2_a_passed = probe_control.resource_field_present
        && probe_treatment.resource_field_present
        && probe_control.width == 256
        && probe_control.height == 256
        && probe_control.initial_total_biomass > 0.0
        && probe_control.initial_total_capacity > 0.0;

    say(&format!(
        "E3 smoke: seed {}, 2 arms x {} ticks (sample period {}), profile {}",
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
        "  WP-2 (a) ResourceField present: {} ({}x{}, initial biomass {:.3} EU, capacity {:.3} EU)",
        wp2_a_passed,
        probe_control.width,
        probe_control.height,
        probe_control.initial_total_biomass,
        probe_control.initial_total_capacity
    ))?;

    // WP-2 (b) & (c): run the paired smoke arms through the shared runner.
    let started = SystemTime::now();
    let timer = Instant::now();
    let report = run_paired_ensemble_with_control::<LiveExperimentAdapter>(
        &control,
        &treatment,
        &registry,
        &FactorDiff {
            allowed_paths: vec![ALLOWED_FACTOR_PATH.to_string()],
        },
    )
    .map_err(|e| format!("run_paired_ensemble_with_control failed: {e}"))?;
    let elapsed_secs = timer.elapsed().as_secs_f64();
    let ended = SystemTime::now();

    let pair = report
        .pairs
        .first()
        .ok_or_else(|| "paired report contains no pairs".to_string())?;
    let ctrl_summary = summarize_arm("control", &pair.control, &plan)?;
    let treat_summary = summarize_arm("treatment", &pair.treatment, &plan)?;

    let expected_emitted = all_live_observable_ids().len();
    if ctrl_summary.emitted_count != expected_emitted
        || treat_summary.emitted_count != expected_emitted
    {
        return Err(format!(
            "expected {expected_emitted} emitted observables, got control={} treatment={}",
            ctrl_summary.emitted_count, treat_summary.emitted_count
        ));
    }
    if !ctrl_summary.all_finite || !treat_summary.all_finite {
        return Err("non-finite observable encountered in smoke run".into());
    }

    let max_abs_standing_crop_diff = ctrl_summary
        .standing_crop_series
        .iter()
        .zip(&treat_summary.standing_crop_series)
        .map(|((tc, vc), (tt, vt))| {
            assert_eq!(tc, tt);
            (vt - vc).abs()
        })
        .fold(0.0f64, f64::max);
    let final_standing_crop_diff =
        treat_summary.standing_crop_final - ctrl_summary.standing_crop_final;
    let post_warmup_standing_crop_diff =
        treat_summary.standing_crop_post_warmup_mean - ctrl_summary.standing_crop_post_warmup_mean;
    let tail_standing_crop_diff =
        treat_summary.standing_crop_tail_mean - ctrl_summary.standing_crop_tail_mean;

    let wp2_b_passed = pair.is_complete()
        && ctrl_summary.final_checksum != treat_summary.final_checksum
        && treat_summary.deforest_ledger_count > 0
        && treat_summary.add_nutrient_ledger_count > 0
        && (final_standing_crop_diff.abs() > 1e-3 || max_abs_standing_crop_diff > 1e-3);

    let wp2_c_passed = ctrl_summary.first_positive_detritus_before_3000.is_some()
        && treat_summary.first_positive_detritus_before_3000.is_some()
        && ctrl_summary.detritus_at_3000.is_some_and(|v| v > 0.0)
        && treat_summary.detritus_at_3000.is_some_and(|v| v > 0.0);

    let us_per_tick = elapsed_secs * 1e6 / (2.0 * rung as f64);
    let projected_by_rung: Vec<(u64, f64)> = plan
        .ladder
        .iter()
        .map(|&t| {
            let mins = 24.0 * (t as f64) * us_per_tick / 60_000_000.0;
            (t, mins)
        })
        .collect();
    let recommended_rung = projected_by_rung
        .iter()
        .find(|(_, mins)| *mins <= plan.max_wall_clock_minutes)
        .map(|(t, _)| *t);

    say(&format!(
        "  WP-2 (b) standing_crop change: {} (final delta {:+.3} EU, post-warmup delta {:+.3} EU, max sample |delta| {:.3} EU; ledger Deforest {} / AddNutrient {})",
        wp2_b_passed,
        final_standing_crop_diff,
        post_warmup_standing_crop_diff,
        max_abs_standing_crop_diff,
        treat_summary.deforest_ledger_count,
        treat_summary.add_nutrient_ledger_count
    ))?;
    say(&format!(
        "  WP-2 (c) detritus > 0 before tick 3000: {} (control first {:?}, treatment first {:?})",
        wp2_c_passed,
        ctrl_summary.first_positive_detritus_before_3000,
        treat_summary.first_positive_detritus_before_3000
    ))?;
    say(&format!(
        "  WP-3 timing: {:.2}s ({:.1} us/tick), 24x18000 projection {:.2} min (recommended rung {:?})",
        elapsed_secs,
        us_per_tick,
        projected_by_rung
            .iter()
            .find(|(t, _)| *t == 18_000)
            .map(|(_, m)| *m)
            .unwrap_or(0.0),
        recommended_rung
    ))?;

    let runs_csv = build_runs_csv(&ctrl_summary, &treat_summary, plan.smoke_seed, us_per_tick);
    let series_csv = build_series_csv(&pair.control, &pair.treatment, plan.smoke_seed)?;
    let report_json = pretty(&serde_json::json!({
        "kind": "e3-smoke-report-EXCLUDED-FROM-ANALYSIS",
        "smoke_seed": plan.smoke_seed,
        "duration_ticks": rung,
        "sample_period": control.sample_period,
        "warmup_ticks": plan.warmup_ticks,
        "tail_window_ticks": plan.tail_window,
        "body_mass_consulted": false,
        "all_27_observables_emitted_and_finite": true,
        "integrity": {
            "world_identity_identical": world_identity_identical,
            "ecology_stream_identical": ecology_stream_identical,
            "control_final_checksum": ctrl_summary.final_checksum,
            "treatment_final_checksum": treat_summary.final_checksum,
            "checksums_differ": ctrl_summary.final_checksum != treat_summary.final_checksum,
        },
        "wp2_a_resource_field": {
            "passed": wp2_a_passed,
            "width": probe_control.width,
            "height": probe_control.height,
            "bounds_x": [probe_control.min_x, probe_control.max_x],
            "bounds_z": [probe_control.min_z, probe_control.max_z],
            "initial_total_biomass_eu": probe_control.initial_total_biomass,
            "initial_total_capacity_eu": probe_control.initial_total_capacity,
            "initial_west_biomass_eu": probe_control.initial_west_biomass,
            "initial_west_capacity_eu": probe_control.initial_west_capacity,
            "initial_east_biomass_eu": probe_control.initial_east_biomass,
            "initial_east_capacity_eu": probe_control.initial_east_capacity,
            "initial_ecosystem_biomass": {
                "plants": probe_control.ecosystem_plants,
                "detritus": probe_control.ecosystem_detritus,
                "animals": probe_control.ecosystem_animals,
                "dormant": probe_control.ecosystem_dormant,
            }
        },
        "wp2_b_standing_crop_intervention": {
            "passed": wp2_b_passed,
            "control_standing_crop_final": ctrl_summary.standing_crop_final,
            "treatment_standing_crop_final": treat_summary.standing_crop_final,
            "final_delta_treatment_minus_control": final_standing_crop_diff,
            "control_standing_crop_post_warmup_mean": ctrl_summary.standing_crop_post_warmup_mean,
            "treatment_standing_crop_post_warmup_mean": treat_summary.standing_crop_post_warmup_mean,
            "post_warmup_mean_delta": post_warmup_standing_crop_diff,
            "post_warmup_relative_shift": if ctrl_summary.standing_crop_post_warmup_mean != 0.0 {
                Some(post_warmup_standing_crop_diff / ctrl_summary.standing_crop_post_warmup_mean.abs())
            } else {
                None
            },
            "control_standing_crop_tail_mean": ctrl_summary.standing_crop_tail_mean,
            "treatment_standing_crop_tail_mean": treat_summary.standing_crop_tail_mean,
            "tail_mean_delta": tail_standing_crop_diff,
            "max_abs_sample_diff": max_abs_standing_crop_diff,
            "control_deforest_ledger_count": ctrl_summary.deforest_ledger_count,
            "control_deforest_eu_moved": ctrl_summary.deforest_eu_moved,
            "control_add_nutrient_ledger_count": ctrl_summary.add_nutrient_ledger_count,
            "control_add_nutrient_eu_added": ctrl_summary.add_nutrient_eu_added,
            "treatment_deforest_ledger_count": treat_summary.deforest_ledger_count,
            "treatment_deforest_eu_moved": treat_summary.deforest_eu_moved,
            "treatment_add_nutrient_ledger_count": treat_summary.add_nutrient_ledger_count,
            "treatment_add_nutrient_eu_added": treat_summary.add_nutrient_eu_added,
        },
        "wp2_c_detritus_before_3000": {
            "passed": wp2_c_passed,
            "control_first_positive_before_3000": ctrl_summary.first_positive_detritus_before_3000,
            "treatment_first_positive_before_3000": treat_summary.first_positive_detritus_before_3000,
            "control_detritus_at_3000": ctrl_summary.detritus_at_3000,
            "treatment_detritus_at_3000": treat_summary.detritus_at_3000,
            "control_early_samples": ctrl_summary.detritus_early_samples,
            "treatment_early_samples": treat_summary.detritus_early_samples,
            "control_detritus_final": ctrl_summary.detritus_final,
            "treatment_detritus_final": treat_summary.detritus_final,
        },
        "demographics_and_conservation": {
            "control": arm_diag_json(&ctrl_summary),
            "treatment": arm_diag_json(&treat_summary),
        },
        "wp3_schedule_lock": {
            "wall_clock_seconds": elapsed_secs,
            "us_per_tick": us_per_tick,
            "projected_24_runs_minutes_by_rung": projected_by_rung
                .iter()
                .map(|(t, mins)| serde_json::json!({ "duration_ticks": t, "projected_minutes": mins }))
                .collect::<Vec<_>>(),
            "recommended_duration_ticks": recommended_rung,
            "warmup_ticks": plan.warmup_ticks,
            "sample_period": control.sample_period,
            "tail_window_ticks": plan.tail_window,
            "within_budget": recommended_rung == Some(rung),
        }
    }))? + "\n";

    let provenance_json = pretty(&build_provenance(
        &args,
        &plan,
        &control,
        &treatment,
        &registry,
        &report,
        &probe_control,
        &input_hashes,
        rung,
        started,
        ended,
        elapsed_secs,
        us_per_tick,
        &projected_by_rung,
        recommended_rung,
    ))? + "\n";

    let summary_md = build_summary_md(
        &plan,
        rung,
        control.sample_period,
        elapsed_secs,
        us_per_tick,
        &projected_by_rung,
        recommended_rung,
        &probe_control,
        &ctrl_summary,
        &treat_summary,
        wp2_a_passed,
        wp2_b_passed,
        wp2_c_passed,
        max_abs_standing_crop_diff,
        final_standing_crop_diff,
        post_warmup_standing_crop_diff,
        tail_standing_crop_diff,
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

fn validate_smoke_manifests(
    plan: &Plan,
    registry: &ObservableRegistry,
    control: &ExperimentManifest,
    treatment: &ExperimentManifest,
) -> Result<(), String> {
    control
        .validate(registry)
        .map_err(|e| format!("control manifest invalid: {e}"))?;
    treatment
        .validate(registry)
        .map_err(|e| format!("treatment manifest invalid: {e}"))?;

    let ctrl_cfg = LiveWorldConfig::from_initial_conditions(&control.initial_conditions)
        .map_err(|e| format!("control LiveWorldConfig invalid: {e}"))?;
    ctrl_cfg
        .ensure_observables_supported(&control.observable_ids)
        .map_err(|e| format!("control observables unsupported: {e}"))?;

    let treat_cfg = LiveWorldConfig::from_initial_conditions(&treatment.initial_conditions)
        .map_err(|e| format!("treatment LiveWorldConfig invalid: {e}"))?;
    treat_cfg
        .ensure_observables_supported(&treatment.observable_ids)
        .map_err(|e| format!("treatment observables unsupported: {e}"))?;

    if control.sample_period != plan.sample_period || treatment.sample_period != plan.sample_period
    {
        return Err(format!(
            "smoke manifests must declare sample_period={}, got control={} treatment={}",
            plan.sample_period, control.sample_period, treatment.sample_period
        ));
    }
    if control.seeds != vec![plan.smoke_seed] || treatment.seeds != vec![plan.smoke_seed] {
        return Err(format!(
            "smoke manifests must declare only [{}], got control={:?} treatment={:?}",
            plan.smoke_seed, control.seeds, treatment.seeds
        ));
    }
    if control.seeds.iter().any(|s| plan.seeds.contains(s)) {
        return Err("smoke manifest contains an experimental seed".into());
    }
    FactorDiff {
        allowed_paths: vec![ALLOWED_FACTOR_PATH.to_string()],
    }
    .validate(control, treatment)
    .map_err(|e| format!("FactorDiff failed: {e}"))?;
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

fn arm_diag_json(a: &ArmSummary) -> serde_json::Value {
    serde_json::json!({
        "status": a.status,
        "final_checksum": a.final_checksum,
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

fn build_runs_csv(ctrl: &ArmSummary, treat: &ArmSummary, seed: u64, us_per_tick: f64) -> String {
    let mut s = String::from(
        "# SMOKE CALIBRATION ONLY (seed 999979); blind to body-mass observables\n\
         seed,arm,status,final_checksum,us_per_tick,deforest_ledger_count,deforest_eu_moved,\
         add_nutrient_ledger_count,add_nutrient_eu_added,standing_crop_final,\
         standing_crop_post_warmup_mean,detritus_at_3000,detritus_final,agent_count_min,\
         agent_count_final,births_sum,deaths_sum,closed_eu_max_abs_drift\n",
    );
    for a in [ctrl, treat] {
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

fn build_series_csv(ctrl: &RunResult, treat: &RunResult, seed: u64) -> Result<String, String> {
    let mut s = String::from("seed,arm,tick");
    for id in SMOKE_SERIES_OBSERVABLES {
        s.push(',');
        s.push_str(id);
    }
    s.push('\n');

    for (arm, run) in [("control", ctrl), ("treatment", treat)] {
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

#[allow(clippy::too_many_arguments)]
fn build_provenance(
    args: &Args,
    plan: &Plan,
    control: &ExperimentManifest,
    treatment: &ExperimentManifest,
    registry: &ObservableRegistry,
    report: &PairedEnsembleReport,
    probe: &FieldProbe,
    input_hashes: &BTreeMap<String, String>,
    rung: u64,
    started: SystemTime,
    ended: SystemTime,
    elapsed_secs: f64,
    us_per_tick: f64,
    projected_by_rung: &[(u64, f64)],
    recommended_rung: Option<u64>,
) -> serde_json::Value {
    let exe = std::env::current_exe().ok();
    serde_json::json!({
        "kind": "e3-smoke-provenance-EXCLUDED-FROM-ANALYSIS",
        "mode": "smoke",
        "model": "core::live_experiment::LiveExperimentAdapter",
        "model_version": LIVE_MODEL_VERSION,
        "model_status": "headless adapter verified; NOT a live-world experiment-ready claim",
        "harness": "core::experiment_runner::run_paired_ensemble_with_control",
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
            "started_utc": iso8601_utc(unix_seconds(started)),
            "ended_utc": iso8601_utc(unix_seconds(ended)),
            "wall_clock_seconds": elapsed_secs,
            "us_per_tick": us_per_tick,
            "max_wall_clock_minutes_registered": plan.max_wall_clock_minutes,
            "projected_24_runs_minutes_by_rung": projected_by_rung
                .iter()
                .map(|(t, mins)| serde_json::json!({ "duration_ticks": t, "projected_minutes": mins }))
                .collect::<Vec<_>>(),
        },
        "wp3_schedule_lock": {
            "duration_ticks_locked": recommended_rung,
            "duration_ticks_as_run": rung,
            "duration_ticks_ladder": plan.ladder,
            "sample_period": control.sample_period,
            "warmup_ticks": plan.warmup_ticks,
            "warmup_samples_excluded": plan.warmup_ticks / control.sample_period.max(1),
            "tail_window_ticks": plan.tail_window,
        },
        "manifests": {
            "manifest_dir": args.manifest_dir.display().to_string().replace('\\', "/"),
            "sha256_as_committed": input_hashes,
            "control_fingerprint_as_run": report.control_manifest_fingerprint,
            "treatment_fingerprint_as_run": report.treatment_manifest_fingerprint,
            "control_law_fingerprint": report.control_law_fingerprint,
            "treatment_law_fingerprint": report.treatment_law_fingerprint,
            "registry_fingerprint": report.registry_fingerprint,
            "declared_factors": report.declared_factors,
            "control_initial_conditions": control.initial_conditions.values,
            "treatment_initial_conditions": treatment.initial_conditions.values,
            "control_interventions": control.interventions,
            "treatment_interventions": treatment.interventions,
            "note": "manifests are referenced by SHA-256 rather than copied here so body-mass observable names stay out of the smoke directory",
        },
        "observables": {
            "manifests_requested_observable_count": control.observable_ids.len(),
            "registry_total_count": registry.specs().len(),
            "observables_written_to_smoke_series": SMOKE_SERIES_OBSERVABLES,
            "body_mass_consulted": false,
        },
        "seeds": {
            "smoke_seed": plan.smoke_seed,
            "executed": [plan.smoke_seed],
            "registered_experimental_seeds": plan.seeds,
            "experimental_seeds_run": 0,
        },
        "world_identity_observed": {
            "seed": probe.world_identity.seed,
            "generator_version": probe.world_identity.generator_version,
            "width": probe.world_identity.width,
            "height": probe.world_identity.height,
            "checksum": probe.world_identity.checksum,
        },
        "what_this_run_does_not_claim": [
            "No desktop-app run was made; no Tauri handle, window, GPU, learner or evolution thread exists in this process.",
            "Smoke seed 999979 is calibration only and is mechanically excluded from all experimental manifests.",
            "No body-mass observable was read or written during WP-2/WP-3.",
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
    projected_by_rung: &[(u64, f64)],
    recommended_rung: Option<u64>,
    probe: &FieldProbe,
    ctrl: &ArmSummary,
    treat: &ArmSummary,
    wp2_a_passed: bool,
    wp2_b_passed: bool,
    wp2_c_passed: bool,
    max_abs_standing_crop_diff: f64,
    final_standing_crop_diff: f64,
    post_warmup_standing_crop_diff: f64,
    tail_standing_crop_diff: f64,
) -> String {
    let mut s = String::from("# E3 WP-2 / WP-3 Smoke Calibration Report\n\n");
    s.push_str(
        "> Calibration only, smoke seed `999979`, EXCLUDED from every analysis, table, statistic, \
         and claim. Body-mass observables were never read or written (`body_mass_consulted = false`).\n\n",
    );
    s.push_str(&format!(
        "Model: `core::live_experiment::LiveExperimentAdapter` (`{LIVE_MODEL_VERSION}`) — \
         **headless adapter verified**.\n\n\
         - Smoke seed: `{}` (`2` arms x `{rung}` ticks, `sample_period = {sample_period}`)\n\
         - Profile: `{}`\n\
         - Wall clock: **{elapsed_secs:.3} s** (`{us_per_tick:.2} us/tick`)\n\
         - Checksums: control `{}`, treatment `{}` (differ: `{}`)\n\n",
        plan.smoke_seed,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        ctrl.final_checksum,
        treat.final_checksum,
        ctrl.final_checksum != treat.final_checksum,
    ));

    s.push_str("## WP-2 (a) ResourceField in headless LiveExperimentAdapter\n\n");
    s.push_str(&format!(
        "- Gate result: **{}**\n\
         - Dimensions: `{}x{}` (`x in [{:.1}, {:.1}]`, `z in [{:.1}, {:.1}]`)\n\
         - Initial field biomass / capacity: `{:.3}` / `{:.3}` EU\n\
         - Initial West (`col < 128`) biomass / capacity: `{:.3}` / `{:.3}` EU\n\
         - Initial East (`col >= 128`) biomass / capacity: `{:.3}` / `{:.3}` EU\n\
         - Initial `EcosystemBiomass`: plants `{:.3}`, detritus `{:.3}`, animals `{:.3}`, dormant `{:.3}`\n\n",
        if wp2_a_passed { "PASS" } else { "FAIL" },
        probe.width,
        probe.height,
        probe.min_x,
        probe.max_x,
        probe.min_z,
        probe.max_z,
        probe.initial_total_biomass,
        probe.initial_total_capacity,
        probe.initial_west_biomass,
        probe.initial_west_capacity,
        probe.initial_east_biomass,
        probe.initial_east_capacity,
        probe.ecosystem_plants,
        probe.ecosystem_detritus,
        probe.ecosystem_animals,
        probe.ecosystem_dormant,
    ));

    s.push_str("## WP-2 (b) Regional interventions change `live.standing_crop`\n\n");
    s.push_str(&format!(
        "- Gate result: **{}**\n\n\
         | Metric | Control | Treatment | Treatment - Control |\n\
         |---|---:|---:|---:|\n\
         | `live.standing_crop` at T={rung} | {:.3} | {:.3} | {:+.3} |\n\
         | Post-warmup mean (`tick > {}`) | {:.3} | {:.3} | {:+.3} |\n\
         | Tail window mean (`{}..={}`) | {:.3} | {:.3} | {:+.3} |\n\
         | Max abs sample difference | — | — | {:.3} |\n\
         | `Deforest` ledger firings / EU moved (`cause_id=1`) | {} / {:.3} | {} / {:.3} | — |\n\
         | `AddNutrient` ledger firings / EU added (`cause_id=2`) | {} / {:.3} | {} / {:.3} | — |\n\n",
        if wp2_b_passed { "PASS" } else { "FAIL" },
        ctrl.standing_crop_final,
        treat.standing_crop_final,
        final_standing_crop_diff,
        plan.warmup_ticks,
        ctrl.standing_crop_post_warmup_mean,
        treat.standing_crop_post_warmup_mean,
        post_warmup_standing_crop_diff,
        plan.tail_window[0],
        plan.tail_window[1],
        ctrl.standing_crop_tail_mean,
        treat.standing_crop_tail_mean,
        tail_standing_crop_diff,
        max_abs_standing_crop_diff,
        ctrl.deforest_ledger_count,
        ctrl.deforest_eu_moved,
        treat.deforest_ledger_count,
        treat.deforest_eu_moved,
        ctrl.add_nutrient_ledger_count,
        ctrl.add_nutrient_eu_added,
        treat.add_nutrient_ledger_count,
        treat.add_nutrient_eu_added,
    ));

    s.push_str("## WP-2 (c) `detritus > 0` before tick 3000\n\n");
    s.push_str(&format!(
        "- Gate result: **{}**\n\
         - First positive sample `< 3000`: control `{:?}`, treatment `{:?}`\n\n\
         | Tick | Control `detritus` (EU) | Treatment `detritus` (EU) |\n\
         |---:|---:|---:|\n",
        if wp2_c_passed { "PASS" } else { "FAIL" },
        ctrl.first_positive_detritus_before_3000,
        treat.first_positive_detritus_before_3000,
    ));
    for ((tc, vc), (tt, vt)) in ctrl
        .detritus_early_samples
        .iter()
        .zip(&treat.detritus_early_samples)
    {
        assert_eq!(tc, tt);
        s.push_str(&format!("| {tc} | {vc:.3} | {vt:.3} |\n"));
    }
    s.push('\n');

    s.push_str("## WP-3 Schedule & Budget Calibration\n\n");
    s.push_str(
        "| Ladder rung (`T`) | Projected 24-run minutes | <= 90 min budget |\n|---:|---:|---|\n",
    );
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
    s.push_str(&format!(
        "| Arm | Status | `agent_count` min / max / final | Sum `births` | Sum `deaths` (starve / dehydrate) | Max `closed_eu_total` abs drift |\n\
         |---|---|---:|---:|---:|---:|\n\
         | control | {} | {} / {} / {} | {} | {} ({} / {}) | {:.6e} |\n\
         | treatment | {} | {} / {} / {} | {} | {} ({} / {}) | {:.6e} |\n",
        ctrl.status,
        ctrl.agent_count_min,
        ctrl.agent_count_max,
        ctrl.agent_count_final,
        ctrl.births_sum,
        ctrl.deaths_sum,
        ctrl.deaths_starvation_sum,
        ctrl.deaths_dehydration_sum,
        ctrl.closed_eu_max_abs_drift,
        treat.status,
        treat.agent_count_min,
        treat.agent_count_max,
        treat.agent_count_final,
        treat.births_sum,
        treat.deaths_sum,
        treat.deaths_starvation_sum,
        treat.deaths_dehydration_sum,
        treat.closed_eu_max_abs_drift,
    ));

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
            smoke_control_file: s(&p["manifests"]["smoke_control"], "manifests.smoke_control")?,
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
                     Build with: cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop --example e3_smoke -j 6"
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
