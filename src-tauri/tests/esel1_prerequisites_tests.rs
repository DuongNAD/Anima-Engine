//! E-SEL1 preconditions P3 and P4 (draft: `docs/ai/planning/2026-11-16-experiment-e-sel1-...`).
//!
//! * P4, `live.mutation_rate`: absent == the historical 0.05 (no absolute pin, only equality between
//!   two runs of this build), and `0.0` makes offspring exact copies of their lineage's founder.
//! * P3, the compiled runner `examples/run_esel1_experiment.rs`: it is `#[path]`-included here and
//!   driven in-process through `run_with_args`, with tiny manifests generated into a temp dir. That
//!   exercises the program's own code without `cargo run` and without depending on the example
//!   binary having been built. No E-SEL1 fixture directory is committed yet (freezing is later).

use anima_engine_lib::core::agent_systems::{AgentGenotype, AgentLineageId};
use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::components::{Agent, AgentBrain};
use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, WorldLawSet, MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::intervention::{InterventionCommand, InterventionQueue};
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LiveWorldConfig, LIVE_EXTENDED_OBSERVABLE_IDS, LIVE_KEY_MUTATION_RATE,
    LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use bevy_ecs::prelude::*;
use rand::SeedableRng;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[path = "../examples/run_esel1_experiment.rs"]
mod runner;

const SEED: u64 = 1;
const TICKS: u64 = 1_500;

fn initial(extra: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(extra.iter().map(|(k, v)| (k.to_string(), *v)).collect())
}

fn base_keys() -> Vec<(&'static str, f64)> {
    vec![
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
        ("live.predator_fraction", 0.0),
    ]
}

struct Driver {
    clock: SimClock,
    ledger: CausalLedger,
    rng: rand::rngs::StdRng,
    queue: InterventionQueue,
}

impl Driver {
    fn new() -> Self {
        Self {
            clock: SimClock { tick: 0 },
            ledger: CausalLedger::new(),
            rng: rand::rngs::StdRng::seed_from_u64(SEED),
            queue: InterventionQueue::new(vec![]),
        }
    }
    fn step(&mut self, a: &mut LiveExperimentAdapter) {
        let tick = self.clock.advance();
        let active: Vec<&InterventionCommand> = self.queue.active_at(tick).collect();
        a.step(&self.clock, &active, &mut self.ledger, &mut self.rng);
    }
}

fn build(init: &InitialConditionSet) -> LiveExperimentAdapter {
    LiveExperimentAdapter::from_manifest(&WorldLawSet::baseline(), init, &[], SEED, (16, 16), TICKS)
        .expect("build")
}

fn checksum_after(init: &InitialConditionSet, ticks: u64) -> u32 {
    let mut a = build(init);
    let mut d = Driver::new();
    for _ in 0..ticks {
        d.step(&mut a);
    }
    a.checksum()
}

/// `(lineage id, serialized body genotype, brain genotype if any)` for every living agent.
fn population(a: &LiveExperimentAdapter) -> Vec<(String, String, Option<Vec<f32>>)> {
    let mut world = a.world();
    let world = &mut *world;
    let mut q = world
        .query_filtered::<(&AgentLineageId, &AgentGenotype, Option<&AgentBrain>), With<Agent>>();
    q.iter(world)
        .map(|(l, g, b)| {
            (
                l.0.clone(),
                serde_json::to_string(&g.0).expect("genotype serializes"),
                b.map(|b| b.genotype.weights.clone()),
            )
        })
        .collect()
}

// ---- P4: live.mutation_rate -------------------------------------------------------------------

#[test]
fn mutation_rate_absent_is_exactly_the_historical_five_percent() {
    let absent = initial(&base_keys());
    let mut explicit = base_keys();
    explicit.push((LIVE_KEY_MUTATION_RATE, 0.05));
    let explicit = initial(&explicit);
    assert_eq!(
        LiveWorldConfig::from_initial_conditions(&absent)
            .unwrap()
            .mutation_rate,
        0.05
    );
    let a = checksum_after(&absent, TICKS);
    let b = checksum_after(&explicit, TICKS);
    assert_eq!(
        a, b,
        "absent must equal an explicit live.mutation_rate = 0.05"
    );

    // Non-vacuous: the key is wired. A different rate changes the trajectory.
    let mut high = base_keys();
    high.push((LIVE_KEY_MUTATION_RATE, 1.0));
    assert_ne!(
        a,
        checksum_after(&initial(&high), TICKS),
        "live.mutation_rate = 1.0 left the trajectory unchanged: the key is not reaching the system"
    );
}

#[test]
fn mutation_rate_out_of_range_is_refused() {
    for bad in [-0.1, 1.5, f64::NAN] {
        let r =
            LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_MUTATION_RATE, bad)]));
        assert!(r.is_err(), "{bad} must be refused");
    }
    assert!(
        LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_MUTATION_RATE, 0.0)]))
            .is_ok()
    );
}

#[test]
fn mutation_rate_zero_makes_offspring_exact_copies_of_their_founder() {
    let mut keys = base_keys();
    keys.push(("live.evolved_brains", 1.0));
    let mut with_zero = keys.clone();
    with_zero.push((LIVE_KEY_MUTATION_RATE, 0.0));

    let founders: BTreeMap<String, (String, Option<Vec<f32>>)> =
        population(&build(&initial(&with_zero)))
            .into_iter()
            .map(|(l, g, b)| (l, (g, b)))
            .collect();
    assert!(!founders.is_empty());

    let run = |init: &InitialConditionSet| {
        let mut a = build(init);
        let mut d = Driver::new();
        for _ in 0..TICKS {
            d.step(&mut a);
        }
        population(&a)
    };

    let after = run(&initial(&with_zero));
    let offspring: Vec<_> = after.iter().filter(|(l, _, _)| l.contains('.')).collect();
    assert!(
        !offspring.is_empty(),
        "no offspring were born in {TICKS} ticks; the test would be vacuous"
    );
    for (lineage, genotype, brain) in &offspring {
        let root = lineage.split('.').next().unwrap();
        let (fg, fb) = founders
            .get(root)
            .unwrap_or_else(|| panic!("no founder named {root} for {lineage}"));
        assert_eq!(
            genotype, fg,
            "{lineage}: body genotype differs from founder {root}"
        );
        assert_eq!(
            brain, fb,
            "{lineage}: brain genotype differs from founder {root}"
        );
    }

    // Contrast: at the default rate the same world does produce a mutated descendant, so the
    // equality above is the key's doing and not an artefact of the harness.
    let default_after = run(&initial(&keys));
    let any_changed = default_after.iter().any(|(l, g, b)| {
        l.contains('.')
            && founders
                .get(l.split('.').next().unwrap())
                .is_some_and(|(fg, fb)| g != fg || b != fb)
    });
    assert!(
        any_changed,
        "at the default rate no descendant differed from its founder; the contrast is vacuous"
    );
}

// ---- P3: the runner ---------------------------------------------------------------------------

const SMOKE_SEED: u64 = 999_971;
const EXP_SEEDS: [u64; 2] = [11, 12];
const RUNG: u64 = 100;

fn manifest(id: &str, seeds: &[u64], cond: &[(&str, f64)]) -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: id.to_string(),
        name: id.to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: initial(cond),
        interventions: vec![],
        seeds: seeds.to_vec(),
        duration_ticks: RUNG,
        sample_period: 10,
        observable_ids: LIVE_OBSERVABLE_IDS
            .iter()
            .chain(LIVE_EXTENDED_OBSERVABLE_IDS.iter())
            .map(|s| s.to_string())
            .collect(),
        exotic_interventions: Vec::new(),
        observer: Default::default(),
    }
}

fn arm(food: f64, founders: f64, mutation: Option<f64>) -> Vec<(&'static str, f64)> {
    let mut v = base_keys();
    v.push(("live.founders", founders));
    v.push(("live.food_cap", food));
    if let Some(m) = mutation {
        v.push((LIVE_KEY_MUTATION_RATE, m));
    }
    v
}

fn put(dir: &Path, name: &str, m: &ExperimentManifest) {
    std::fs::write(dir.join(name), serde_json::to_string_pretty(m).unwrap()).unwrap();
}

/// Writes a complete plan into a fresh temp dir. `founders = 0` makes every run extinct at once.
fn fixture(tag: &str, founders: f64, with_drift: bool, seeds: &[u64]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("esel1-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    put(
        &dir,
        "control.json",
        &manifest("t::control", seeds, &arm(60.0, founders, None)),
    );
    put(
        &dir,
        "treatment.json",
        &manifest("t::treatment", seeds, &arm(20.0, founders, None)),
    );
    put(
        &dir,
        "drift.json",
        &manifest("t::drift", seeds, &arm(20.0, founders, Some(0.0))),
    );
    let smoke = [SMOKE_SEED];
    put(
        &dir,
        "smoke-control.json",
        &manifest("t::sc", &smoke, &arm(60.0, founders, None)),
    );
    put(
        &dir,
        "smoke-treatment.json",
        &manifest("t::st", &smoke, &arm(20.0, founders, None)),
    );
    let prereg = serde_json::json!({
        "seeds": { "execution_order": EXP_SEEDS, "smoke_seed": SMOKE_SEED,
                   "min_complete_pairs_for_a_decision": 2 },
        "duration": { "duration_ticks_ladder": [100, 60, 40] },
        "cost": { "max_wall_clock_minutes": 90 },
        "manifests": {
            "control": "control.json", "treatment": "treatment.json",
            "drift_control_optional_requires_P4": if with_drift { serde_json::json!("drift.json") } else { serde_json::Value::Null },
            "smoke_control": "smoke-control.json", "smoke_treatment": "smoke-treatment.json"
        }
    });
    std::fs::write(dir.join("esel1-preregistration.json"), prereg.to_string()).unwrap();
    dir
}

fn args(parts: &[&str], dir: &Path, out: &Path) -> Vec<String> {
    let mut v: Vec<String> = parts.iter().map(|s| s.to_string()).collect();
    v.push("--manifest-dir".into());
    v.push(dir.display().to_string());
    v.push("--out".into());
    v.push(out.display().to_string());
    v
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn the_runner_writes_a_parseable_artifact_set_with_a_drift_arm() {
    let dir = fixture("full", 12.0, true, &EXP_SEEDS);
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("execution-lock.json"), r#"{"duration_ticks":100}"#).unwrap();
    runner::run_with_args(args(&["--ensemble"], &dir, &out)).expect("ensemble runs");

    for f in [
        "runs.csv",
        "per-seed-deltas.csv",
        "paired-report.json",
        "effects.json",
        "provenance.json",
        "checksums.sha256",
        "summary.md",
    ] {
        assert!(out.join(f).is_file(), "{f} missing");
    }
    for f in ["paired-report.json", "effects.json", "provenance.json"] {
        serde_json::from_str::<serde_json::Value>(&read(&out.join(f)))
            .unwrap_or_else(|e| panic!("{f} unparseable: {e}"));
    }
    let effects: serde_json::Value =
        serde_json::from_str(&read(&out.join("effects.json"))).unwrap();
    assert_eq!(effects["primary_observable"], "live.trait_mass_mean");
    assert!(effects["h2_descriptive_drift_control"].is_object());
    assert_eq!(effects["extinction"]["n_extinct_runs_total"], 0);
    // 2 seeds x 3 arms + header.
    assert_eq!(read(&out.join("runs.csv")).lines().count(), 1 + 6);
    // Every file listed in checksums.sha256 hashes to its line.
    for line in read(&out.join("checksums.sha256")).lines() {
        let (digest, name) = line.split_once("  ").unwrap();
        let actual = anima_engine_lib::core::sha256::sha256_file(&out.join(name)).unwrap();
        assert_eq!(digest, actual, "{name}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_extinct_population_drops_its_pair_and_is_counted() {
    // Zero founders: every run has live.agent_count == 0 at every sample.
    let dir = fixture("extinct", 0.0, false, &EXP_SEEDS);
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("execution-lock.json"), "{}").unwrap();
    runner::run_with_args(args(&["--ensemble"], &dir, &out)).expect("ensemble runs");
    let effects: serde_json::Value =
        serde_json::from_str(&read(&out.join("effects.json"))).unwrap();
    assert_eq!(
        effects["extinction"]["n_extinct_runs_total"], 4,
        "2 seeds x 2 arms"
    );
    assert_eq!(effects["extinction"]["n_extinct_runs_by_arm"]["control"], 2);
    assert_eq!(effects["n_complete_pairs_used_by_h1"], 0);
    assert_eq!(effects["sufficient_pairs"], false);
    assert_eq!(
        effects["extinction"]["extinct_runs"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert!(read(&out.join("summary.md")).contains("Dropped runs"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ensemble_is_refused_without_an_execution_lock() {
    let dir = fixture("nolock", 8.0, false, &EXP_SEEDS);
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let e = runner::run_with_args(args(&["--ensemble"], &dir, &out)).unwrap_err();
    assert!(e.contains("execution-lock.json"), "{e}");
    assert!(
        !out.join("runs.csv").exists(),
        "nothing may run before the lock check"
    );
    // A lock for a different rung is refused too.
    std::fs::write(out.join("execution-lock.json"), r#"{"duration_ticks":60}"#).unwrap();
    let e = runner::run_with_args(args(&["--ensemble"], &dir, &out)).unwrap_err();
    assert!(e.contains("locks duration_ticks"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undeclared_seeds_are_refused() {
    // Manifests declare a seed the preregistration does not.
    let dir = fixture("badseed", 8.0, false, &[11, 99]);
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("execution-lock.json"), "{}").unwrap();
    let e = runner::run_with_args(args(&["--ensemble"], &dir, &out)).unwrap_err();
    assert!(e.contains("preregistered execution order"), "{e}");

    // The smoke seed may not appear in an experimental manifest either.
    let dir2 = fixture("smokeleak", 8.0, false, &EXP_SEEDS);
    let out2 = dir2.join("out");
    std::fs::create_dir_all(&out2).unwrap();
    let e = runner::run_with_args(args(
        &["--replay", "--replay-arm", "control", "--replay-seed", "99"],
        &dir2,
        &out2,
    ))
    .unwrap_err();
    assert!(e.contains("not declared"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}

#[test]
fn the_runner_refuses_a_second_undeclared_factor() {
    let dir = fixture("factor", 8.0, false, &EXP_SEEDS);
    // Treatment also changes the tree count: a factor nobody registered.
    let mut t = arm(20.0, 8.0, None);
    t.push(("live.trees", 3.0));
    put(
        &dir,
        "treatment.json",
        &manifest("t::treatment", &EXP_SEEDS, &t),
    );
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("execution-lock.json"), "{}").unwrap();
    let e = runner::run_with_args(args(&["--ensemble"], &dir, &out)).unwrap_err();
    assert!(
        e.contains("exactly the initial condition live.food_cap"),
        "{e}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn smoke_and_replay_write_only_under_their_own_directories() {
    let dir = fixture("smoke", 8.0, false, &EXP_SEEDS);
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    runner::run_with_args(args(&["--smoke"], &dir, &out)).expect("smoke runs without a lock");
    assert!(out.join("smoke").join("runs.csv").is_file());
    assert!(!out.join("runs.csv").exists());
    runner::run_with_args(args(
        &[
            "--replay",
            "--replay-arm",
            "treatment",
            "--replay-seed",
            "11",
        ],
        &dir,
        &out,
    ))
    .expect("replay runs");
    assert!(out
        .join("replay")
        .join("replay-treatment-seed-11-t100.json")
        .is_file());
    let _ = std::fs::remove_dir_all(&dir);
}
