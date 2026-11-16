//! Card S1 / ADR-0005 option A: population turnover in the live world is a **declared** input.
//!
//! `live.reproduction = 1` turns on `autonomous_reproduction_system` (it is already on the live
//! schedule); absent or `0` keeps the adapter's historical `ReproductionPolicy::disabled()`. Every
//! test drives `LiveExperimentAdapter` through the real tick schedule
//! (`simulation_schedule::build_tick_schedule`).
//!
//! * (a) off is bit-identical to the base commit (a pinned checksum measured BEFORE the key existed)
//! * (b) on + mortality: real turnover, founders replaced
//! * (c) on: the checksum agrees across 6 separate OS processes
//! * (d) fork / save-load after the first death and first birth match the uninterrupted run
//! * (e) closed EU stays closed with turnover on
//!
//! Keys are spelled as strings on purpose: the same file measured the base commit, where the
//! constant did not exist yet.

use anima_engine_lib::core::agent_systems::AgentGeneration;
use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, ObservableRegistry, WorldLawSet,
    MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::{
    checkpoint_fork, run_manifest_seed, ExperimentModel, RunStatus,
};
use anima_engine_lib::core::intervention::{InterventionCommand, InterventionQueue};
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LiveSnapshot, LiveWorldConfig, LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::core::snapshot::{self, SnapshotEnvelope};
use anima_engine_lib::core::world_artifact::WorldIdentity;
use rand::SeedableRng;
use std::process::Command;

/// Seed and horizon from the ADR-0005 exploratory run (seed 1: founders gone well before 2000).
const SEED: u64 = 1;
const TICKS: u64 = 3_000;
const OFF_TICKS: u64 = 600;

/// The world checksum of the default (key absent) manifest after `OFF_TICKS` ticks, measured on
/// the base commit (origin/finding/f037-dual-mortality-grace) BEFORE `live.reproduction` existed.
const BASE_OFF_CHECKSUM: u32 = 0x09ed_e909;

const CHILD_ROLE: &str = "ANIMA_S1_TURNOVER_CHILD";
const MARKER: &str = "S1_TURNOVER_CHECKSUM";

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn initial(extra: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(extra.iter().map(|(k, v)| (k.to_string(), *v)).collect())
}

fn on_keys() -> Vec<(&'static str, f64)> {
    vec![("live.mortality", 1.0), ("live.reproduction", 1.0)]
}

fn manifest(initial: InitialConditionSet, ticks: u64) -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "s1-turnover".to_string(),
        name: "s1-turnover".to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: initial,
        interventions: vec![],
        seeds: vec![SEED],
        duration_ticks: ticks,
        sample_period: 60,
        observable_ids: LIVE_OBSERVABLE_IDS.iter().map(|s| s.to_string()).collect(),
        exotic_interventions: Vec::new(),
        observer: Default::default(),
    }
}

fn build(initial: &InitialConditionSet, ticks: u64) -> LiveExperimentAdapter {
    LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        initial,
        &[],
        SEED,
        (16, 16),
        ticks,
    )
    .expect("build")
}

struct Driver {
    clock: SimClock,
    ledger: CausalLedger,
    rng: rand::rngs::StdRng,
    queue: InterventionQueue,
}

impl Driver {
    fn new(at_tick: u64) -> Self {
        Self {
            clock: SimClock { tick: at_tick },
            ledger: CausalLedger::new(),
            rng: rand::rngs::StdRng::seed_from_u64(SEED),
            queue: InterventionQueue::new(vec![]),
        }
    }
    fn step(&mut self, a: &mut LiveExperimentAdapter) -> Vec<(String, f64)> {
        let tick = self.clock.advance();
        let active: Vec<&InterventionCommand> = self.queue.active_at(tick).collect();
        a.step(&self.clock, &active, &mut self.ledger, &mut self.rng);
        a.observables()
    }
}

fn obs(o: &[(String, f64)], id: &str) -> f64 {
    o.iter()
        .find(|(k, _)| k == id)
        .unwrap_or_else(|| panic!("{id}"))
        .1
}

fn checksum_after(initial: &InitialConditionSet, ticks: u64) -> u32 {
    let mut a = build(initial, ticks);
    let mut d = Driver::new(0);
    for _ in 0..ticks {
        d.step(&mut a);
    }
    a.checksum()
}

// ---- (a) ---------------------------------------------------------------------------------------

#[test]
fn a_default_world_and_reproduction_zero_are_bit_identical_to_the_base_commit() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let absent = checksum_after(&initial(&[]), OFF_TICKS);
    println!("S1_OFF_CHECKSUM absent {absent:#010x}");
    assert_eq!(
        absent, BASE_OFF_CHECKSUM,
        "the default trajectory changed: LIVE_MODEL_VERSION would have to be bumped"
    );
    let zero = checksum_after(&initial(&[("live.reproduction", 0.0)]), OFF_TICKS);
    assert_eq!(
        zero, BASE_OFF_CHECKSUM,
        "live.reproduction = 0 must equal the key being absent"
    );
}

// ---- (b) ---------------------------------------------------------------------------------------

#[test]
fn reproduction_and_mortality_replace_the_founders() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut a = build(&initial(&on_keys()), TICKS);
    let mut d = Driver::new(0);
    let (mut births, mut deaths) = (0.0, 0.0);
    let mut counts = std::collections::BTreeSet::new();
    let founders = obs(&a.observables(), "live.agent_count");
    for _ in 0..TICKS {
        let o = d.step(&mut a);
        births += obs(&o, "live.births");
        deaths += obs(&o, "live.deaths");
        counts.insert(obs(&o, "live.agent_count") as u64);
    }
    // The founders are exactly the generation-0 agents; "replaced" = none of them is alive.
    let mut w = a.world();
    let mut q = w.query::<&AgentGeneration>();
    let gens: Vec<u32> = q.iter(&w).map(|g| g.0).collect();
    let max_gen = gens.iter().copied().max().unwrap_or(0);
    let founders_left = gens.iter().filter(|g| **g == 0).count();
    println!(
        "S1_ON founders={founders} births={births} deaths={deaths} distinct_counts={} alive={} \
         max_gen={max_gen} founders_left={founders_left}",
        counts.len(),
        gens.len()
    );
    assert!(
        births > 0.0 && deaths > 0.0,
        "births {births} deaths {deaths}"
    );
    assert!(
        counts.len() > 1,
        "live.agent_count never changed in the run"
    );
    assert!(!gens.is_empty(), "the population went extinct");
    assert_eq!(founders_left, 0, "a generation-0 founder is still alive");
    assert!(max_gen >= 2, "max generation {max_gen}");
}

// ---- (c) ---------------------------------------------------------------------------------------

fn on_checksum() -> u32 {
    checksum_after(&initial(&on_keys()), TICKS)
}

/// Child entry point. A plain test that does nothing unless the parent set the role variable, so
/// it needs neither `#[ignore]` nor a target-policy entry.
#[test]
fn turnover_child_entry_point() {
    if std::env::var(CHILD_ROLE).is_err() {
        return;
    }
    println!("{MARKER} {:#010x}", on_checksum());
}

fn checksum_from_child() -> u32 {
    let output = Command::new(std::env::current_exe().expect("current_exe"))
        .args(["--exact", "turnover_child_entry_point", "--nocapture"])
        .env(CHILD_ROLE, "1")
        .env("ANIMA_USE_GPU", "0")
        .output()
        .expect("spawn child");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|l| l.trim_start().starts_with(MARKER))
        .unwrap_or_else(|| {
            panic!(
                "child printed no checksum\n{stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
    let hex = line.trim().rsplit(' ').next().expect("token");
    u32::from_str_radix(hex.trim_start_matches("0x"), 16).expect("hex checksum")
}

#[test]
fn the_turnover_checksum_agrees_across_six_separate_processes() {
    let sums: Vec<u32> = (0..6).map(|_| checksum_from_child()).collect();
    println!(
        "S1_ON_CHECKSUMS {}",
        sums.iter()
            .map(|s| format!("{s:#010x}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    assert!(
        sums.iter().all(|s| *s == sums[0]),
        "processes disagree: {sums:x?}"
    );
    assert_ne!(sums[0], 0);
}

// ---- (d) ---------------------------------------------------------------------------------------

/// A tick just after the first death and the first birth, from a scouting run.
fn tick_after_first_death_and_birth() -> u64 {
    let mut a = build(&initial(&on_keys()), TICKS);
    let mut d = Driver::new(0);
    let (mut b, mut dth) = (false, false);
    for _ in 0..TICKS {
        let o = d.step(&mut a);
        b |= obs(&o, "live.births") > 0.0;
        dth |= obs(&o, "live.deaths") > 0.0;
        if b && dth {
            return d.clock.tick + 5;
        }
    }
    panic!("no birth and death within {TICKS} ticks");
}

fn through_disk(s: &LiveSnapshot) -> LiveSnapshot {
    let dir = std::env::temp_dir().join(format!("anima_s1_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("checkpoint.json");
    snapshot::write_atomic(
        &path,
        &SnapshotEnvelope::seal(s.saved.clone()).expect("seal"),
    )
    .expect("write");
    let back = snapshot::read(&path).expect("read");
    let _ = std::fs::remove_dir_all(&dir);
    LiveSnapshot {
        saved: back,
        config: s.config,
        seed: s.seed,
    }
}

#[test]
fn a_fork_and_a_save_load_after_the_first_death_and_birth_match_the_uninterrupted_run() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let k = tick_after_first_death_and_birth();
    println!("S1_FORK_TICK {k}");
    let init = initial(&on_keys());
    let m = manifest(init.clone(), TICKS);
    let reg = ObservableRegistry::live_default();

    let uninterrupted = run_manifest_seed::<LiveExperimentAdapter>(&m, &reg, SEED, None, None);
    assert_eq!(uninterrupted.status, RunStatus::Completed);
    let report =
        checkpoint_fork::<LiveExperimentAdapter>(&m, &reg, SEED, k, &[]).expect("fork accepted");
    assert_eq!(
        report.control.final_checksum, uninterrupted.final_checksum,
        "fork diverged"
    );

    // Save at k, through disk, then run the rest.
    let mut reference = build(&init, TICKS);
    let mut dr = Driver::new(0);
    for _ in 0..TICKS {
        dr.step(&mut reference);
    }
    let mut first = build(&init, TICKS);
    let mut d1 = Driver::new(0);
    for _ in 0..k {
        d1.step(&mut first);
    }
    let snap = through_disk(&first.snapshot());
    let mut resumed = LiveExperimentAdapter::from_snapshot(&snap).expect("resume");
    let mut d2 = Driver {
        clock: SimClock { tick: k },
        ..d1
    };
    for _ in 0..(TICKS - k) {
        d2.step(&mut resumed);
    }
    assert_eq!(reference.checksum(), uninterrupted.final_checksum);
    assert_eq!(
        resumed.checksum(),
        reference.checksum(),
        "save at {k} -> load -> run the rest diverged"
    );
    assert_eq!(resumed.observables(), reference.observables());
}

// ---- (e) ---------------------------------------------------------------------------------------

#[test]
fn closed_eu_stays_closed_with_turnover_on() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut a = build(&initial(&on_keys()), TICKS);
    let mut d = Driver::new(0);
    // Tick 1 settles genesis (the world gains a constant 1000 EU of seeded regrowth on its first
    // tick, with or without turnover), so the closed total is baselined after it.
    let start = obs(&d.step(&mut a), "live.closed_eu_total");
    let mut worst = 0.0_f64;
    let mut births = 0.0;
    for _ in 1..TICKS {
        let o = d.step(&mut a);
        births += obs(&o, "live.births");
        let drift = (obs(&o, "live.closed_eu_total") - start).abs() / start.abs().max(1.0);
        worst = worst.max(drift);
    }
    println!("S1_EU start={start} worst_relative_drift={worst} births={births}");
    assert!(
        births > 0.0,
        "no turnover happened, the check would be vacuous"
    );
    assert!(worst < 1e-6, "closed EU drifted by {worst} (relative)");
}

// ---- the declared key itself -------------------------------------------------------------------

#[test]
fn live_births_is_refused_without_a_birth_rule_and_a_non_boolean_is_refused() {
    let off = LiveWorldConfig::default();
    assert!(off
        .ensure_observables_supported(&["live.births".to_string()])
        .is_err());
    let on = LiveWorldConfig::from_initial_conditions(&initial(&[("live.reproduction", 1.0)]))
        .expect("valid");
    assert!(on
        .ensure_observables_supported(&["live.births".to_string()])
        .is_ok());
    assert!(
        LiveWorldConfig::from_initial_conditions(&initial(&[("live.reproduction", 0.5)])).is_err()
    );
}
