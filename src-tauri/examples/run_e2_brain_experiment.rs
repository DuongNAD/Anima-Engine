//! The E2 runner: does a per-agent evolved brain change outcomes under the same world and seed?
//!
//! This is the binary the preregistration
//! (`docs/ai/{requirements,design,planning,testing}/2026-07-27-experiment-e2-evolved-brain-default.md`)
//! names, and it may only do what that package registered before any run existed. It loads the
//! committed manifests rather than reconstructing them, drives
//! [`run_paired_ensemble_with_control`] over [`LiveExperimentAdapter`], and writes the artifact set
//! of design §7.
//!
//! # Build it, then run the binary — never `cargo run`
//!
//! ```text
//! cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop \
//!   --example run_e2_brain_experiment
//!
//! src-tauri/target/release/examples/run_e2_brain_experiment.exe --smoke \
//!   --manifest-dir src-tauri/tests/fixtures/experiments_e2 \
//!   --out artifacts/experiments/e2-evolved-brain-default
//! ```
//!
//! `cargo run` is forbidden on the development machine by a standing owner rule that bars launching
//! the app or the full backend by any route. The rule is categorical rather than a judgement about
//! this program, and this program is on the safe side of it anyway: it constructs a Bevy `World` and
//! runs `simulation_schedule::build_tick_schedule` in-process. There is no Tauri handle, no window,
//! no GPU device, no renderer, no learner thread, no evolution thread and no websocket server —
//! `LiveExperimentAdapter` is the whole surface, and what it deliberately leaves out is documented
//! in `core::live_experiment`'s module docs. The status of the live world remains **headless adapter
//! verified**, and nothing this binary produces may be quoted as anything stronger.
//!
//! # Modes, and why the smoke seed cannot leak into the analysis
//!
//! | mode | what it runs | where it writes |
//! |---|---|---|
//! | `--smoke` | seed 999983 only, both arms, for calibration | `<out>/smoke/` |
//! | `--ensemble` | the twelve preregistered seeds, both arms, once | `<out>/` |
//! | `--replay` | one seed of one arm, for the checksum-identity check | `<out>/replay/` |
//!
//! The separation is mechanical rather than promised. `--smoke` reads only the two smoke manifests
//! and refuses to start if they declare anything but the smoke seed; `--ensemble` reads only the two
//! experimental manifests and refuses to start if their seed lists are not *exactly* the
//! preregistered execution order, or if the smoke seed appears in either. A smoke run cannot write
//! outside `smoke/`, so nothing downstream can read it by accident.
//!
//! # The duration ladder
//!
//! `--duration-rung` accepts only a value from the ladder the preregistration declares
//! (`[18000, 12000, 6000]`), applies it to **both** manifests identically, and records both the
//! committed and the effective duration and fingerprints in `provenance.json`. The rung is chosen
//! once, from the smoke calibration, before any experimental seed runs; this flag is how that choice
//! is expressed, not a knob to turn afterwards.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anima_engine_lib::core::components::{Agent, AgentBrain};
use anima_engine_lib::core::experiment::{ExperimentManifest, FactorDiff, ObservableRegistry};
use anima_engine_lib::core::experiment_runner::{
    run_manifest_seed, run_paired_ensemble_with_control, ExperimentModel, PairedEnsembleReport,
    RunResult, RunStatus,
};
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LIVE_MODEL_VERSION, LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::resources::SimRng;
use anima_engine_lib::core::sha256::sha256_file;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use bevy_ecs::prelude::*;
use rand::RngCore;

/// The declared factor's allowlist, from design §2.2. Path-granular, which is why
/// `prereg_e2_manifest_tests::the_declared_factor_is_exactly_one_initial_condition_key` also pins the
/// key — see the note there.
const ALLOWED_FACTOR_PATH: &str = "initial_conditions";

/// The primary metric (requirements §4). Named here so the summary cannot quietly promote another.
const PRIMARY_OBSERVABLE: &str = "live.mean_agent_energy";
const SECONDARY_OBSERVABLES: [&str; 3] =
    ["live.agent_count", "live.animals_eu", "live.predator_count"];
const INTEGRITY_OBSERVABLE: &str = "live.closed_eu_total";

/// Materiality, all three conditions, from planning §6.2 — evaluated by this program rather than by
/// a reader with the numbers in front of them.
const MATERIALITY_DZ: f64 = 0.8;
const MATERIALITY_RELATIVE_SHIFT: f64 = 0.05;

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

fn main() {
    if let Err(e) = run() {
        eprintln!("E2 runner refused to proceed: {e}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let registry = ObservableRegistry::live_default();
    registry
        .validate()
        .map_err(|e| format!("the live registry is invalid: {e}"))?;

    let prereg = read_json(&args.manifest_dir.join("e2-preregistration.json"))?;
    let plan = Plan::from_prereg(&prereg)?;

    let (control_name, treatment_name) = match args.mode {
        Mode::Smoke => (
            plan.smoke_control_file.clone(),
            plan.smoke_treatment_file.clone(),
        ),
        Mode::Ensemble | Mode::Replay => (plan.control_file.clone(), plan.treatment_file.clone()),
    };
    let mut control = load_manifest(&args.manifest_dir.join(&control_name))?;
    let mut treatment = load_manifest(&args.manifest_dir.join(&treatment_name))?;

    let committed = CommittedIdentity {
        control_fingerprint: control.fingerprint(),
        treatment_fingerprint: treatment.fingerprint(),
        duration_ticks: control.duration_ticks,
        control_sha256: sha256_file(&args.manifest_dir.join(&control_name))
            .map_err(|e| format!("hash {control_name}: {e}"))?,
        treatment_sha256: sha256_file(&args.manifest_dir.join(&treatment_name))
            .map_err(|e| format!("hash {treatment_name}: {e}"))?,
        prereg_sha256: sha256_file(&args.manifest_dir.join("e2-preregistration.json"))
            .map_err(|e| format!("hash e2-preregistration.json: {e}"))?,
    };

    plan.refuse_undeclared_seeds(args.mode, &control, &treatment)?;

    let rung = match args.duration_rung {
        Some(t) => {
            if !plan.ladder.contains(&t) {
                return Err(format!(
                    "--duration-rung {t} is not on the preregistered ladder {:?}; the rung must be \
                     one that was registered before any run, not one invented for this one",
                    plan.ladder
                ));
            }
            control.duration_ticks = t;
            treatment.duration_ticks = t;
            t
        }
        None => control.duration_ticks,
    };

    let out_dir = match args.mode {
        // A smoke run is calibration and never evidence. It writes under `smoke/` and cannot be
        // pointed anywhere else, so no later step can read it as a result by mistake.
        Mode::Smoke => args.out.join("smoke"),
        Mode::Ensemble => args.out.clone(),
        Mode::Replay => args.out.join("replay"),
    };
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;

    if args.mode == Mode::Replay {
        return replay(
            &args, &plan, &control, &treatment, &registry, &out_dir, rung,
        );
    }

    // Integrity, measured on the world this run will actually build rather than asserted. Each probe
    // constructs an adapter, reads it and drops it; `run_manifest_seed` builds its own world per
    // seed, so probing changes nothing downstream.
    let probe_seed = control.seeds[0];
    let control_probe = probe_arm(&control, probe_seed)?;
    let treatment_probe = probe_arm(&treatment, probe_seed)?;
    let integrity = Integrity::check(&control_probe, &treatment_probe)?;

    println!(
        "E2 {:?}: {} seeds x 2 arms x {rung} ticks, sample period {}",
        args.mode,
        control.seeds.len(),
        control.sample_period
    );
    println!(
        "  integrity: control brains {} / treatment brains {} of {} founders; ecology stream \
         identical: {}; world identity identical: {}",
        control_probe.brains,
        treatment_probe.brains,
        treatment_probe.agents,
        integrity.ecology_stream_identical,
        integrity.world_identity_identical
    );

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
    let elapsed = clock.elapsed();
    let ended = SystemTime::now();

    println!(
        "  finished in {:.1}s: {} complete pairs of {}",
        elapsed.as_secs_f64(),
        report.complete_pairs(),
        report.pairs.len()
    );

    let effects = derive_effects(&report);
    write_artifacts(WriteInput {
        mode: args.mode,
        out_dir: &out_dir,
        manifest_dir: &args.manifest_dir,
        control: &control,
        treatment: &treatment,
        control_name: &control_name,
        treatment_name: &treatment_name,
        committed: &committed,
        plan: &plan,
        rung,
        report: &report,
        effects: &effects,
        integrity: &integrity,
        control_probe: &control_probe,
        treatment_probe: &treatment_probe,
        registry: &registry,
        started,
        ended,
        elapsed_secs: elapsed.as_secs_f64(),
    })?;

    println!("  artifacts written to {}", out_dir.display());
    Ok(())
}

// ---- Arguments ---------------------------------------------------------------------------------

fn parse_args() -> Result<Args, String> {
    let mut mode: Option<Mode> = None;
    let mut manifest_dir: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut duration_rung: Option<u64> = None;
    let mut replay_seed: Option<u64> = None;
    let mut replay_arm: Option<String> = None;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut set_mode = |m: Mode| -> Result<(), String> {
            if let Some(existing) = mode {
                return Err(format!(
                    "--smoke, --ensemble and --replay are mutually exclusive; {existing:?} was \
                     already selected. An ambiguous mode is how a calibration run ends up filed as \
                     a result."
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
        return Err("--replay needs --replay-seed and --replay-arm control|treatment".into());
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
run_e2_brain_experiment --smoke|--ensemble|--replay --manifest-dir <dir> --out <dir>
                       [--duration-rung <ticks>]
                       [--replay-seed <seed> --replay-arm control|treatment]

Build with:
  cargo build --manifest-path src-tauri/Cargo.toml --release --features desktop \\
    --example run_e2_brain_experiment
then execute the compiled binary directly. Never `cargo run`.";

fn need(it: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    it.next().ok_or_else(|| format!("{flag} needs a value"))
}

// ---- The preregistered plan, read as data ------------------------------------------------------

/// The parts of `e2-preregistration.json` this runner is bound by. Read rather than retyped, so the
/// binary cannot drift away from the registered plan.
struct Plan {
    seeds: Vec<u64>,
    smoke_seed: u64,
    min_complete_pairs: usize,
    ladder: Vec<u64>,
    sample_period: u64,
    max_wall_clock_minutes: f64,
    control_file: String,
    treatment_file: String,
    smoke_control_file: String,
    smoke_treatment_file: String,
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
            sample_period: p["duration"]["sample_period"]
                .as_u64()
                .ok_or("duration.sample_period must be an integer")?,
            max_wall_clock_minutes: p["cost"]["max_wall_clock_minutes"]
                .as_f64()
                .ok_or("cost.max_wall_clock_minutes must be a number")?,
            control_file: s(&p["manifests"]["control"], "manifests.control")?,
            treatment_file: s(&p["manifests"]["treatment"], "manifests.treatment")?,
            smoke_control_file: s(&p["manifests"]["smoke_control"], "manifests.smoke_control")?,
            smoke_treatment_file: s(
                &p["manifests"]["smoke_treatment"],
                "manifests.smoke_treatment",
            )?,
        })
    }

    /// Refuse a mixed or undeclared seed set before anything is built.
    ///
    /// The preregistration's exclusion of the smoke seed is mechanical at the runner level too
    /// (`run_manifest_seed` returns `SeedNotInManifest`), but by then a run has been attempted. This
    /// is the earlier, louder refusal: an ensemble whose seeds are not *exactly* the registered
    /// execution order is not the registered experiment, and it stops here rather than producing a
    /// report that looks like one.
    fn refuse_undeclared_seeds(
        &self,
        mode: Mode,
        control: &ExperimentManifest,
        treatment: &ExperimentManifest,
    ) -> Result<(), String> {
        if control.seeds != treatment.seeds {
            return Err(format!(
                "the arms declare different seed lists ({:?} vs {:?}); pairing requires the same \
                 seeds in the same order",
                control.seeds, treatment.seeds
            ));
        }
        match mode {
            Mode::Ensemble | Mode::Replay => {
                if control.seeds != self.seeds {
                    return Err(format!(
                        "the manifests declare {:?}, but the preregistered execution order is {:?}. \
                         No substitution, reordering or top-up is permitted.",
                        control.seeds, self.seeds
                    ));
                }
                if control.seeds.contains(&self.smoke_seed) {
                    return Err(format!(
                        "the smoke seed {} appears in an experimental manifest; it is excluded from \
                         every analysis, table, statistic and claim",
                        self.smoke_seed
                    ));
                }
            }
            Mode::Smoke => {
                if control.seeds != vec![self.smoke_seed] {
                    return Err(format!(
                        "a smoke run may declare exactly the smoke seed [{}] and nothing else; the \
                         manifests declare {:?}",
                        self.smoke_seed, control.seeds
                    ));
                }
                if let Some(bad) = control.seeds.iter().find(|s| self.seeds.contains(s)) {
                    return Err(format!(
                        "seed {bad} is an experimental seed and must not be run as calibration"
                    ));
                }
            }
        }
        Ok(())
    }
}

// ---- Integrity, measured on the built worlds ---------------------------------------------------

/// What one arm's world looks like the instant after genesis, before any tick.
struct ArmProbe {
    agents: usize,
    brains: usize,
    world_identity: WorldIdentity,
    rng_seed: u64,
    rng_stream_pos: u128,
    /// The next draws the ecology stream would hand out, as a fingerprint of its exact state.
    rng_next_draws: Vec<u64>,
}

fn probe_arm(manifest: &ExperimentManifest, seed: u64) -> Result<ArmProbe, String> {
    let adapter = LiveExperimentAdapter::from_manifest(
        &manifest.laws,
        &manifest.initial_conditions,
        &manifest.exotic_interventions,
        seed,
        (16, 16),
        manifest.duration_ticks,
    )
    .map_err(|e| format!("building '{}' at seed {seed}: {e}", manifest.experiment_id))?;

    let world_identity = adapter
        .world()
        .get_resource::<WorldIdentity>()
        .copied()
        .ok_or("a live world always has a WorldIdentity")?;

    let (agents, brains) = {
        let mut world = adapter.world();
        let world = &mut *world;
        let agents = {
            let mut q = world.query_filtered::<(), With<Agent>>();
            q.iter(world).count()
        };
        let brains = {
            let mut q = world.query_filtered::<(), (With<Agent>, With<AgentBrain>)>();
            q.iter(world).count()
        };
        (agents, brains)
    };

    let (rng_seed, rng_stream_pos, rng_next_draws) = {
        let mut world = adapter.world();
        let mut rng = world
            .get_resource_mut::<SimRng>()
            .ok_or("a live world always has SimRng")?;
        let seed = rng.seed();
        let pos = rng.stream_pos();
        let draws: Vec<u64> = (0..8).map(|_| rng.rng().next_u64()).collect();
        (seed, pos, draws)
    };

    Ok(ArmProbe {
        agents,
        brains,
        world_identity,
        rng_seed,
        rng_stream_pos,
        rng_next_draws,
    })
}

/// The three properties a paired E2 run is void without.
struct Integrity {
    world_identity_identical: bool,
    ecology_stream_identical: bool,
    brains_present_only_in_treatment: bool,
}

impl Integrity {
    fn check(control: &ArmProbe, treatment: &ArmProbe) -> Result<Self, String> {
        let me = Integrity {
            world_identity_identical: control.world_identity == treatment.world_identity,
            ecology_stream_identical: control.rng_seed == treatment.rng_seed
                && control.rng_stream_pos == treatment.rng_stream_pos
                && control.rng_next_draws == treatment.rng_next_draws,
            brains_present_only_in_treatment: control.brains == 0
                && treatment.brains == treatment.agents
                && treatment.agents > 0,
        };
        // Each of these makes the comparison mean something other than what it claims, so the run is
        // refused rather than reported with a caveat.
        if !me.world_identity_identical {
            return Err(format!(
                "the arms built different worlds ({:?} vs {:?}); gate E2-G6 voids the run",
                control.world_identity, treatment.world_identity
            ));
        }
        if !me.ecology_stream_identical {
            return Err(
                "the arms' ecology streams are not in the same state after genesis, so they would \
                 differ in the brain AND in the realised random sequence, inseparably (design §4.3, \
                 gate E2-G3)"
                    .into(),
            );
        }
        if !me.brains_present_only_in_treatment {
            return Err(format!(
                "the arms are not the arms they claim: control has {} brains and treatment has {} \
                 of {} founders (gate E2-G1)",
                control.brains, treatment.brains, treatment.agents
            ));
        }
        Ok(me)
    }
}

// ---- Effects: the registered statistics, plus the two derived ones -----------------------------

/// One observable's paired effect, as the runner computed it plus the two derivations planning §6.1
/// registers: the median of the per-seed deltas and their between-seed variance.
struct Effect {
    observable: String,
    n_requested: usize,
    n_complete_pairs: usize,
    control_mean: Option<f64>,
    treatment_mean: Option<f64>,
    paired_mean_delta: Option<f64>,
    paired_sd: Option<f64>,
    paired_se: Option<f64>,
    ci95_low: Option<f64>,
    ci95_high: Option<f64>,
    paired_dz: Option<f64>,
    median_delta: Option<f64>,
    between_seed_variance: Option<f64>,
    /// The per-seed `(seed, control, treatment, delta)` rows, complete pairs only.
    rows: Vec<(u64, f64, f64, f64)>,
}

impl Effect {
    /// The three materiality conditions of planning §6.2. All must hold; `None` anywhere is a
    /// failure to meet the condition, never a pass by default.
    fn materiality(&self) -> (bool, bool, bool) {
        let ci_excludes_zero = match (self.ci95_low, self.ci95_high) {
            (Some(lo), Some(hi)) => lo > 0.0 || hi < 0.0,
            _ => false,
        };
        let large_dz = self
            .paired_dz
            .map(|d| d.abs() >= MATERIALITY_DZ)
            .unwrap_or(false);
        let relative = match (self.paired_mean_delta, self.control_mean) {
            (Some(d), Some(c)) => d.abs() >= MATERIALITY_RELATIVE_SHIFT * c.abs(),
            _ => false,
        };
        (ci_excludes_zero, large_dz, relative)
    }

    fn is_material(&self) -> bool {
        let (a, b, c) = self.materiality();
        a && b && c
    }

    fn relative_shift(&self) -> Option<f64> {
        match (self.paired_mean_delta, self.control_mean) {
            (Some(d), Some(c)) if c != 0.0 => Some(d / c.abs()),
            _ => None,
        }
    }
}

fn derive_effects(report: &PairedEnsembleReport) -> Vec<Effect> {
    report
        .effects
        .iter()
        .map(|e| {
            let mut rows = Vec::new();
            for pair in &report.pairs {
                if !pair.is_complete() {
                    continue;
                }
                if let (Some(c), Some(t)) = (
                    pair.control.observable(&e.observable),
                    pair.treatment.observable(&e.observable),
                ) {
                    rows.push((pair.seed, c, t, t - c));
                }
            }
            let deltas: Vec<f64> = rows.iter().map(|r| r.3).collect();
            Effect {
                observable: e.observable.clone(),
                n_requested: e.n_requested,
                n_complete_pairs: e.n_complete_pairs,
                control_mean: e.control_mean,
                treatment_mean: e.treatment_mean,
                paired_mean_delta: e.paired_mean_delta,
                paired_sd: e.paired_sd,
                paired_se: e.paired_se,
                ci95_low: e.ci95_low,
                ci95_high: e.ci95_high,
                paired_dz: e.paired_dz,
                median_delta: median(&deltas),
                // Between-seed variance of the deltas IS `paired_sd²` (planning §6.1), derived from
                // the runner's own SD rather than recomputed, so the two can never disagree.
                between_seed_variance: e.paired_sd.map(|sd| sd * sd),
                rows,
            }
        })
        .collect()
}

/// The median of a sample. Reported beside the mean because with n = 12 one outlying seed moves a
/// mean and not a median, and the two disagreeing is itself informative (planning §6.1).
fn median(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).expect("finite observables"));
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

// ---- Replay (gate E2-G5) -----------------------------------------------------------------------

/// Re-run one seed of one arm and record its final checksum, so "this run reproduces" is a measured
/// fact rather than a property of the design. Writes to `replay/`, never into the analysis.
fn replay(
    args: &Args,
    plan: &Plan,
    control: &ExperimentManifest,
    treatment: &ExperimentManifest,
    registry: &ObservableRegistry,
    out_dir: &Path,
    rung: u64,
) -> Result<(), String> {
    let arm = args.replay_arm.as_deref().unwrap_or_default();
    let manifest = match arm {
        "control" => control,
        "treatment" => treatment,
        other => {
            return Err(format!(
                "--replay-arm must be control or treatment, not '{other}'"
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
    let _ = plan;

    let clock = Instant::now();
    let result = run_manifest_seed::<LiveExperimentAdapter>(manifest, registry, seed, None, None);
    let elapsed = clock.elapsed().as_secs_f64();

    let doc = serde_json::json!({
        "kind": "e2-replay",
        "note": "A checksum-identity check (gate E2-G5). Never merged into the analysis.",
        "arm": arm,
        "seed": seed,
        "duration_ticks": rung,
        "experiment_id": manifest.experiment_id,
        "manifest_fingerprint": manifest.fingerprint(),
        "status": status_label(&result.status),
        "final_checksum": result.final_checksum,
        "final_observables": observables_map(&result),
        "warnings": result.warnings,
        "wall_clock_seconds": elapsed,
        "model_version_recorded_by_runner": result.provenance.model_version,
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

struct CommittedIdentity {
    control_fingerprint: u64,
    treatment_fingerprint: u64,
    duration_ticks: u64,
    control_sha256: String,
    treatment_sha256: String,
    prereg_sha256: String,
}

struct WriteInput<'a> {
    mode: Mode,
    out_dir: &'a Path,
    manifest_dir: &'a Path,
    control: &'a ExperimentManifest,
    treatment: &'a ExperimentManifest,
    control_name: &'a str,
    treatment_name: &'a str,
    committed: &'a CommittedIdentity,
    plan: &'a Plan,
    rung: u64,
    report: &'a PairedEnsembleReport,
    effects: &'a [Effect],
    integrity: &'a Integrity,
    control_probe: &'a ArmProbe,
    treatment_probe: &'a ArmProbe,
    registry: &'a ObservableRegistry,
    started: SystemTime,
    ended: SystemTime,
    elapsed_secs: f64,
}

fn write_artifacts(w: WriteInput<'_>) -> Result<(), String> {
    // The manifests as run, copied beside the results. A reader should never have to trust that the
    // committed file at some later commit is the one this run used.
    let manifests_dir = w.out_dir.join("manifests");
    std::fs::create_dir_all(&manifests_dir)
        .map_err(|e| format!("create {}: {e}", manifests_dir.display()))?;
    for name in [w.control_name, w.treatment_name, "e2-preregistration.json"] {
        let from = w.manifest_dir.join(name);
        let bytes = std::fs::read(&from).map_err(|e| format!("read {}: {e}", from.display()))?;
        std::fs::write(manifests_dir.join(name), &bytes)
            .map_err(|e| format!("copy {name}: {e}"))?;
    }

    write_text(
        &w.out_dir.join("paired-report.json"),
        &(pretty(&serde_json::to_value(w.report).map_err(|e| e.to_string())?)? + "\n"),
    )?;
    write_text(&w.out_dir.join("effects.json"), &effects_json(&w)?)?;
    write_text(
        &w.out_dir.join("per-seed-deltas.csv"),
        &per_seed_csv(w.effects),
    )?;
    write_text(&w.out_dir.join("runs.csv"), &runs_csv(w.report))?;
    write_text(&w.out_dir.join("provenance.json"), &provenance_json(&w)?)?;
    write_text(&w.out_dir.join("summary.md"), &summary_md(&w))?;

    // Checksums last: they cover every file above, in a stable order, in the format `sha256sum -c`
    // and `scripts/verify_e2_artifacts.mjs` both read.
    let mut names: Vec<String> = vec![
        "effects.json".into(),
        "paired-report.json".into(),
        "per-seed-deltas.csv".into(),
        "provenance.json".into(),
        "runs.csv".into(),
        "summary.md".into(),
    ];
    for name in [w.control_name, w.treatment_name, "e2-preregistration.json"] {
        names.push(format!("manifests/{name}"));
    }
    names.sort();
    let mut lines = String::new();
    for name in &names {
        let digest = sha256_file(&w.out_dir.join(name)).map_err(|e| format!("hash {name}: {e}"))?;
        lines.push_str(&format!("{digest}  {name}\n"));
    }
    write_text(&w.out_dir.join("checksums.sha256"), &lines)?;
    Ok(())
}

fn effects_json(w: &WriteInput<'_>) -> Result<String, String> {
    let rows: Vec<serde_json::Value> = w
        .effects
        .iter()
        .map(|e| {
            let (ci, dz, rel) = e.materiality();
            serde_json::json!({
                "observable": e.observable,
                "role": role_of(&e.observable),
                "n_requested": e.n_requested,
                "n_complete_pairs": e.n_complete_pairs,
                "control_mean": e.control_mean,
                "treatment_mean": e.treatment_mean,
                "paired_mean_delta": e.paired_mean_delta,
                "paired_sd": e.paired_sd,
                "paired_se": e.paired_se,
                "ci95_low": e.ci95_low,
                "ci95_high": e.ci95_high,
                "paired_dz": e.paired_dz,
                "median_of_per_seed_deltas": e.median_delta,
                "between_seed_variance_of_deltas": e.between_seed_variance,
                "relative_shift_vs_control_mean": e.relative_shift(),
                "materiality": {
                    "ci95_excludes_zero": ci,
                    "abs_dz_at_least_0_8": dz,
                    "abs_delta_at_least_5pc_of_control_mean": rel,
                    "material": e.is_material(),
                },
            })
        })
        .collect();
    let doc = serde_json::json!({
        "kind": if w.mode == Mode::Smoke { "e2-smoke-effects-EXCLUDED-FROM-ANALYSIS" } else { "e2-effects" },
        "primary_observable": PRIMARY_OBSERVABLE,
        "secondary_observables": SECONDARY_OBSERVABLES,
        "harness_integrity_observable": INTEGRITY_OBSERVABLE,
        "materiality_rule": "all_of: ci95 excludes 0; |paired_dz| >= 0.8; |paired_mean_delta| >= 0.05 * |control_mean|",
        "n_complete_pairs": w.report.complete_pairs(),
        "min_complete_pairs_for_a_decision": w.plan.min_complete_pairs,
        "effects": rows,
    });
    Ok(pretty(&doc)? + "\n")
}

fn per_seed_csv(effects: &[Effect]) -> String {
    // Exactly the columns design §7 declares. Complete pairs only — an incomplete pair has no
    // delta, and inventing a column to say so would change a schema registered before the run.
    // Every run, complete or not, appears in `runs.csv`.
    let mut s = String::from("seed,observable,control_final,treatment_final,delta\n");
    for e in effects {
        for (seed, c, t, d) in &e.rows {
            s.push_str(&format!("{seed},{},{c},{t},{d}\n", e.observable));
        }
    }
    s
}

fn runs_csv(report: &PairedEnsembleReport) -> String {
    let mut s = String::from("seed,arm,status,failed_tick,failure_reason,final_checksum,warnings");
    for id in LIVE_OBSERVABLE_IDS {
        s.push(',');
        s.push_str(id);
    }
    s.push('\n');
    for pair in &report.pairs {
        for (arm, run) in [("control", &pair.control), ("treatment", &pair.treatment)] {
            let (tick, reason) = match &run.status {
                RunStatus::Completed => (String::new(), String::new()),
                RunStatus::Failed { tick, reason, .. } => (tick.to_string(), csv_field(reason)),
            };
            s.push_str(&format!(
                "{},{arm},{},{tick},{reason},{},{}",
                pair.seed,
                status_label(&run.status),
                run.final_checksum,
                csv_field(&run.warnings.join(" | ")),
            ));
            for id in LIVE_OBSERVABLE_IDS {
                s.push(',');
                if let Some(v) = run.observable(id) {
                    s.push_str(&v.to_string());
                }
            }
            s.push('\n');
        }
    }
    s
}

fn provenance_json(w: &WriteInput<'_>) -> Result<String, String> {
    let started = unix_seconds(w.started);
    let ended = unix_seconds(w.ended);
    let exe = std::env::current_exe().ok();
    let doc = serde_json::json!({
        "kind": if w.mode == Mode::Smoke { "e2-smoke-provenance-EXCLUDED-FROM-ANALYSIS" } else { "e2-provenance" },
        "experiment_id": w.report.experiment_id,
        "mode": format!("{:?}", w.mode).to_lowercase(),
        "model": "core::live_experiment::LiveExperimentAdapter",
        "model_status": "headless adapter verified; NOT a live-world experiment-ready claim",
        "harness": "core::experiment_runner::run_paired_ensemble_with_control",
        "registry": "core::experiment::ObservableRegistry::live_default",

        "build": {
            "git_commit": git(&["rev-parse", "HEAD"]),
            "git_describe": git(&["describe", "--always", "--dirty"]),
            "git_tree_dirty": git(&["status", "--porcelain"]).map(|s| !s.trim().is_empty()),
            "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "rustc": tool_version("rustc"),
            "cargo": tool_version("cargo"),
            "features": "desktop",
            "package_version": env!("CARGO_PKG_VERSION"),
            "binary_path": exe.as_ref().map(|p| p.display().to_string()),
            "binary_sha256": exe.as_ref().and_then(|p| sha256_file(p).ok()),
        },
        "host": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "machine": std::env::var("COMPUTERNAME").ok(),
            "cpu": std::env::var("PROCESSOR_IDENTIFIER").ok(),
        },
        "timing": {
            "started_unix_seconds": started,
            "ended_unix_seconds": ended,
            "started_utc": iso8601_utc(started),
            "ended_utc": iso8601_utc(ended),
            "ensemble_wall_clock_seconds": w.elapsed_secs,
            "ensemble_wall_clock_minutes": w.elapsed_secs / 60.0,
            "max_wall_clock_minutes_registered": w.plan.max_wall_clock_minutes,
            "within_registered_budget": w.elapsed_secs / 60.0 <= w.plan.max_wall_clock_minutes,
        },
        "duration": {
            "ladder_registered": w.plan.ladder,
            "duration_ticks_as_committed": w.committed.duration_ticks,
            "duration_ticks_as_run": w.rung,
            "rung_stepped_down": w.rung != w.committed.duration_ticks,
            "sample_period": w.control.sample_period,
            "sample_period_registered": w.plan.sample_period,
            "total_ticks": (w.control.seeds.len() as u64) * 2 * w.rung,
        },
        "manifests": {
            "control_file": w.control_name,
            "treatment_file": w.treatment_name,
            "control_sha256_as_committed": w.committed.control_sha256,
            "treatment_sha256_as_committed": w.committed.treatment_sha256,
            "preregistration_sha256": w.committed.prereg_sha256,
            "control_fingerprint_as_committed": w.committed.control_fingerprint,
            "treatment_fingerprint_as_committed": w.committed.treatment_fingerprint,
            "control_fingerprint_as_run": w.report.control_manifest_fingerprint,
            "treatment_fingerprint_as_run": w.report.treatment_manifest_fingerprint,
            "control_law_fingerprint": w.report.control_law_fingerprint,
            "treatment_law_fingerprint": w.report.treatment_law_fingerprint,
            "registry_fingerprint": w.report.registry_fingerprint,
            "declared_factors": w.report.declared_factors,
            // The declared factor as data rather than as a claim: a reader can diff these two lists
            // and see for themselves that exactly one key differs.
            "control_initial_conditions": w.control.initial_conditions.values,
            "treatment_initial_conditions": w.treatment.initial_conditions.values,
        },
        "seeds": {
            "execution_order": w.report.seed_order,
            "registered_execution_order": w.plan.seeds,
            "smoke_seed": w.plan.smoke_seed,
            "smoke_seed_present_in_this_run": w.report.seed_order.contains(&w.plan.smoke_seed),
            "n_requested": w.report.pairs.len(),
            "n_complete_pairs": w.report.complete_pairs(),
            "n_incomplete_pairs": w.report.incomplete_pairs(),
            "min_complete_pairs_for_a_decision": w.plan.min_complete_pairs,
        },
        "world_identity_observed": {
            "note": "Finding E2-F2: LiveExperimentAdapter never checks a manifest's declared world_identity against the world init_world builds, so 'same world' is recorded from the world that was actually constructed (gate E2-G6).",
            "declared_in_manifest": w.control.world_identity,
            "control_observed": w.control_probe.world_identity,
            "treatment_observed": w.treatment_probe.world_identity,
            "identical_across_arms": w.integrity.world_identity_identical,
        },
        "model_version": {
            "note": "Finding E2-F1: RunProvenance::derive hard-codes the reference model's version for every run, so every RunResult in paired-report.json names the wrong model. The true version of the model that ran is recorded here. The fix is filed separately and is not part of this experiment.",
            "recorded_in_run_provenance": w.report.pairs.first().map(|p| p.control.provenance.model_version.clone()),
            "model_actually_run": LIVE_MODEL_VERSION,
        },
        "integrity": {
            "probe_seed": w.control.seeds[0],
            "control_agents": w.control_probe.agents,
            "control_brains": w.control_probe.brains,
            "treatment_agents": w.treatment_probe.agents,
            "treatment_brains": w.treatment_probe.brains,
            "brains_present_only_in_treatment": w.integrity.brains_present_only_in_treatment,
            "ecology_stream_identical_after_genesis": w.integrity.ecology_stream_identical,
            "control_rng": { "seed": w.control_probe.rng_seed, "stream_pos": w.control_probe.rng_stream_pos.to_string(), "next_draws": w.control_probe.rng_next_draws },
            "treatment_rng": { "seed": w.treatment_probe.rng_seed, "stream_pos": w.treatment_probe.rng_stream_pos.to_string(), "next_draws": w.treatment_probe.rng_next_draws },
        },
        "observables": {
            "requested": w.control.observable_ids,
            "registry_ids": LIVE_OBSERVABLE_IDS,
            "specs": w.registry.fingerprint(),
        },
        "what_this_run_does_not_claim": [
            "No full desktop-app run was made; no Tauri handle, window, GPU device, renderer, learner thread or evolution thread exists in this process.",
            "The adapter runs no evolutionary replacement, so nothing here is evidence about selection, adaptation or speciation.",
            "N seeds under one world identity are N stochastic samples of ONE world design, not N worlds.",
            "Numerical agreement with ReferenceEvolutionWorld is not claimed.",
        ],
    });
    Ok(pretty(&doc)? + "\n")
}

fn summary_md(w: &WriteInput<'_>) -> String {
    let mut s = String::new();
    let excluded = w.mode == Mode::Smoke;
    s.push_str("# E2 — per-agent evolved brain vs shared BrainModel\n\n");
    if excluded {
        s.push_str(
            "> ⛔ **Calibration only, seed 999983, EXCLUDED from every analysis, table, statistic \
             and claim.** These numbers exist to time the harness and prove it runs. They are not \
             evidence about brains and may not be quoted as any part of the E2 result.\n\n",
        );
    }
    s.push_str(&format!(
        "Model: `core::live_experiment::LiveExperimentAdapter` — **headless adapter verified**, \
         which is the only status claim permitted. Harness: \
         `core::experiment_runner::run_paired_ensemble_with_control`.\n\n\
         - seeds run: `{:?}`\n- duration: **{} ticks** (committed {}, ladder {:?}), sample period {}\n\
         - complete pairs: **{} of {}** (decision needs {})\n- wall clock: **{:.1} s** ({:.2} min) \
         against a registered ceiling of {} min\n- declared factors: `{:?}`\n\n",
        w.report.seed_order,
        w.rung,
        w.committed.duration_ticks,
        w.plan.ladder,
        w.control.sample_period,
        w.report.complete_pairs(),
        w.report.pairs.len(),
        w.plan.min_complete_pairs,
        w.elapsed_secs,
        w.elapsed_secs / 60.0,
        w.plan.max_wall_clock_minutes,
        w.report.declared_factors,
    ));

    s.push_str("## Integrity (gates E2-G1, E2-G3, E2-G6)\n\n");
    s.push_str(&format!(
        "| check | result |\n|---|---|\n\
         | control founders carrying `AgentBrain` | {} of {} |\n\
         | treatment founders carrying `AgentBrain` | {} of {} |\n\
         | ecology stream identical after genesis | {} |\n\
         | world identity identical across arms | {} |\n\n",
        w.control_probe.brains,
        w.control_probe.agents,
        w.treatment_probe.brains,
        w.treatment_probe.agents,
        yes_no(w.integrity.ecology_stream_identical),
        yes_no(w.integrity.world_identity_identical),
    ));

    s.push_str("## Every observable the run produced\n\n");
    s.push_str(
        "Roles were fixed before the numbers existed and cannot move afterwards. `delta` is \
         treatment − control in the observable's own unit.\n\n",
    );
    s.push_str(
        "| observable | role | n | control mean | treatment mean | delta | median delta | SD | SE | \
         95% CI | d_z | rel. shift | material |\n|---|---|--:|--:|--:|--:|--:|--:|--:|:--:|--:|--:|:--:|\n",
    );
    for e in w.effects {
        s.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            e.observable,
            role_of(&e.observable),
            e.n_complete_pairs,
            num(e.control_mean),
            num(e.treatment_mean),
            num(e.paired_mean_delta),
            num(e.median_delta),
            num(e.paired_sd),
            num(e.paired_se),
            match (e.ci95_low, e.ci95_high) {
                (Some(a), Some(b)) => format!("[{a:.6}, {b:.6}]"),
                _ => "n/a".into(),
            },
            num(e.paired_dz),
            e.relative_shift()
                .map(|r| format!("{:+.2}%", r * 100.0))
                .unwrap_or_else(|| "n/a".into()),
            yes_no(e.is_material()),
        ));
    }
    s.push('\n');

    if let Some(p) = w
        .effects
        .iter()
        .find(|e| e.observable == PRIMARY_OBSERVABLE)
    {
        let (ci, dz, rel) = p.materiality();
        s.push_str("## The primary metric, against the three registered conditions\n\n");
        s.push_str(&format!(
            "`{PRIMARY_OBSERVABLE}`, paired per seed, hypothesised **negative** (H1).\n\n\
             | condition | required | measured | met |\n|---|---|---|:--:|\n\
             | 95% CI excludes 0 | CI must not contain 0 | {} | {} |\n\
             | \\|d_z\\| ≥ 0.8 | large by convention | {} | {} |\n\
             | \\|delta\\| ≥ 5% of \\|control mean\\| | relative shift | {} | {} |\n\n\
             **Material: {}.**\n\n",
            match (p.ci95_low, p.ci95_high) {
                (Some(a), Some(b)) => format!("[{a:.6}, {b:.6}]"),
                _ => "undefined".into(),
            },
            yes_no(ci),
            num(p.paired_dz),
            yes_no(dz),
            p.relative_shift()
                .map(|r| format!("{:+.3}%", r * 100.0))
                .unwrap_or_else(|| "undefined".into()),
            yes_no(rel),
            yes_no(p.is_material()),
        ));
    }

    let failures: Vec<String> = w
        .report
        .pairs
        .iter()
        .flat_map(|pair| {
            [("control", &pair.control), ("treatment", &pair.treatment)]
                .into_iter()
                .filter_map(move |(arm, run)| match &run.status {
                    RunStatus::Completed => None,
                    RunStatus::Failed { tick, reason, .. } => {
                        Some(format!("| {} | {arm} | {tick} | {reason} |", pair.seed))
                    }
                })
        })
        .collect();
    s.push_str("## Failures and warnings\n\n");
    if failures.is_empty() {
        s.push_str("No run failed.\n\n");
    } else {
        s.push_str("| seed | arm | tick | reason |\n|---|---|--:|---|\n");
        for row in &failures {
            s.push_str(row);
            s.push('\n');
        }
        s.push('\n');
    }
    let warnings: Vec<String> = w
        .report
        .pairs
        .iter()
        .flat_map(|pair| {
            [("control", &pair.control), ("treatment", &pair.treatment)]
                .into_iter()
                .flat_map(move |(arm, run)| {
                    run.warnings
                        .iter()
                        .map(move |warn| format!("- seed {} {arm}: {warn}", pair.seed))
                })
        })
        .collect();
    if warnings.is_empty() {
        s.push_str("No run produced a warning.\n\n");
    } else {
        for line in &warnings {
            s.push_str(line);
            s.push('\n');
        }
        s.push('\n');
    }

    s.push_str("## What this cannot establish\n\n");
    s.push_str(
        "- **Nothing about evolution.** The adapter drains epoch statistics and applies no \
         replacement, so the founding brains are the only brains a run ever has and nothing is ever \
         selected.\n\
         - **Nothing about worlds.** These seeds are stochastic samples of ONE world design; the map \
         never varied, so between-world variance was never sampled.\n\
         - **Nothing about the desktop app.** No app was started; the status of the live world is \
         *headless adapter verified* and this run does not change it.\n\
         - **Nothing about behavioural diversity**, which is EB-S11's bespoke harness and not a \
         registry observable.\n\n\
         Every number above traces to a file in this directory; `checksums.sha256` covers all of \
         them.\n",
    );
    s
}

// ---- Small helpers -----------------------------------------------------------------------------

fn role_of(observable: &str) -> &'static str {
    if observable == PRIMARY_OBSERVABLE {
        "primary"
    } else if SECONDARY_OBSERVABLES.contains(&observable) {
        "secondary"
    } else if observable == INTEGRITY_OBSERVABLE {
        "harness integrity"
    } else {
        "reported"
    }
}

fn num(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.6}")).unwrap_or_else(|| "n/a".into())
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn status_label(s: &RunStatus) -> &'static str {
    match s {
        RunStatus::Completed => "completed",
        RunStatus::Failed { .. } => "failed",
    }
}

fn observables_map(run: &RunResult) -> BTreeMap<String, f64> {
    run.final_observables.iter().cloned().collect()
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

/// Every artifact is written with LF endings on every platform, because their bytes are hashed.
/// `.gitattributes` pins the same thing for the checked-in copies.
fn write_text(path: &Path, text: &str) -> Result<(), String> {
    let normalised = text.replace("\r\n", "\n");
    std::fs::write(path, normalised.as_bytes())
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

/// Civil date from a UNIX timestamp (Howard Hinnant's `civil_from_days`), so provenance carries a
/// date a human can read without adding a dependency for it.
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
