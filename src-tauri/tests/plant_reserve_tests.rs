//! Card S10 (QD-SEL2): `live.plant_reserve`, a fraction `ρ ∈ [0, 1)` of each pasture cell's
//! capacity `r_max` that herbivores cannot eat.
//!
//! Without a reserve the logistic pasture has no part an animal cannot reach, so a population that
//! grazes hard enough to make food scarce slides all the way to the bottom (the SEL2 redesign
//! proposal, `docs/ai/planning/2026-11-18-sel2-redesign-proposal.md` §4). With one, both grazing
//! paths offer `max(r − ρ·r_max, 0)` instead of `r`: `herbivore_grazing_system` for live agents and
//! `DormantCohorts::graze_all_with_reserve` for the aggregate tier.
//!
//! What these tests hold (the card's definition of done):
//! - (1) absent == `0.0` == a world with no `PlantReservePolicy` resource at all, bit for bit (runs
//!   compared, never an absolute checksum: checksums differ by platform). The comparison against
//!   the commit *before* the key existed is a separate measurement recorded in the card's note;
//! - (2) with `ρ > 0`, no cell is ever below `ρ·r_max` in a run dense enough that, without the
//!   reserve, the same seed grazes cells far below that line;
//! - (3) the closed-EU total stays within the threshold the existing turnover test uses;
//! - (4) the same manifest gives the same checksum in several separate processes;
//! - a checkpoint taken mid-run and resumed through disk keeps the reserve;
//! - the aggregate tier obeys the same floor and stays exactly conserved.
//!
//! No body-mass value is read here: the scarcity line of experiments stays blind to it.

use anima_engine_lib::core::aggregate_population::{
    ArchivedIndividual, DormantCohorts, DormantVitals,
};
use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::components::AgentClass;
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::experiment::{ExperimentError, InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::intervention::{InterventionCommand, InterventionQueue};
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LiveSnapshot, LiveWorldConfig, LIVE_KEYS, LIVE_KEY_PLANT_RESERVE,
    LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::resources::PlantReservePolicy;
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::core::snapshot::{self, SnapshotEnvelope};
use anima_engine_lib::evolution::genotype::MorphologyGenotype;
use rand::SeedableRng;
use std::process::Command;

const SEED: u64 = 3;
const TICKS: u64 = 1_500;
const RESERVE: f64 = 0.3;

const CHILD_ROLE: &str = "ANIMA_S10_RESERVE_CHILD";
const MARKER: &str = "S10_RESERVE_CHECKSUM";

/// Every live world here is heavy in a debug build; run them one at a time.
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// ---- The world ---------------------------------------------------------------------------------

/// The E-SEL1 / S6 base world (30 prey founders, no predators, 8 trees, 2 lakes; death, birth and
/// steering to water on) with the prey ceiling raised to 160 so the herd grows dense enough to
/// graze the pasture down. Keys are spelled as strings so the world matches the S6 and S9 drivers.
fn initial(extra: &[(&str, f64)]) -> InitialConditionSet {
    let mut pairs: Vec<(String, f64)> = [
        ("live.founders", 30.0),
        ("live.predator_fraction", 0.0),
        ("live.trees", 8.0),
        ("live.lakes", 2.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
        ("live.locomotion", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
        ("live.prey_cap", 160.0),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), *v))
    .collect();
    pairs.extend(extra.iter().map(|(k, v)| (k.to_string(), *v)));
    InitialConditionSet::new(pairs)
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
    .expect("the declared world builds")
}

/// Steps the adapter the way the shared runner does.
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
        .unwrap_or_else(|| panic!("{id} missing"))
        .1
}

/// The legacy twelve observables only (no trait or mass ids), as bit patterns.
fn legacy_bits(o: &[(String, f64)]) -> Vec<u64> {
    LIVE_OBSERVABLE_IDS
        .iter()
        .map(|id| obs(o, id).to_bits())
        .collect()
}

/// Run `ticks` ticks; return the final checksum and the legacy observables at every tick.
fn trajectory(a: &mut LiveExperimentAdapter, ticks: u64) -> (u32, Vec<Vec<u64>>) {
    let mut d = Driver::new(0);
    let series = (0..ticks).map(|_| legacy_bits(&d.step(a))).collect();
    (a.checksum(), series)
}

// ---- (1) off is the old world ------------------------------------------------------------------

#[test]
fn absent_zero_and_no_policy_resource_are_bit_identical() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (absent_sum, absent) = trajectory(&mut build(&initial(&[]), TICKS), TICKS);
    let (zero_sum, zero) = trajectory(
        &mut build(&initial(&[(LIVE_KEY_PLANT_RESERVE, 0.0)]), TICKS),
        TICKS,
    );
    // The desktop app never inserts the resource; both grazing systems read it as `Option`.
    let mut bare = build(&initial(&[]), TICKS);
    assert!(bare
        .world()
        .remove_resource::<PlantReservePolicy>()
        .is_some());
    let (bare_sum, bare_series) = trajectory(&mut bare, TICKS);

    println!("S10_OFF absent={absent_sum:#010x} zero={zero_sum:#010x} bare={bare_sum:#010x}");
    assert_eq!(
        absent_sum, zero_sum,
        "absent and 0.0 must be the same world"
    );
    assert_eq!(
        absent_sum, bare_sum,
        "absent and no resource must be the same world"
    );
    for (t, ((a, z), b)) in absent.iter().zip(&zero).zip(&bare_series).enumerate() {
        assert_eq!(
            a,
            z,
            "legacy observables differ at tick {} (absent vs 0)",
            t + 1
        );
        assert_eq!(
            a,
            b,
            "legacy observables differ at tick {} (absent vs bare)",
            t + 1
        );
    }
}

#[test]
fn a_reserve_changes_the_world() {
    // Guards (1) against vacuity: if the reserve never bit, "off == old" would prove nothing.
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (off, _) = trajectory(&mut build(&initial(&[]), TICKS), TICKS);
    let (on, _) = trajectory(
        &mut build(&initial(&[(LIVE_KEY_PLANT_RESERVE, RESERVE)]), TICKS),
        TICKS,
    );
    assert_ne!(off, on, "a reserve of {RESERVE} left the world unchanged");
}

// ---- (2) the floor holds -----------------------------------------------------------------------

/// Per tick: the smallest `(r − ρ·r_max) / r_max` over cells with capacity, and how many cells sit
/// below `ρ·r_max`.
fn floor_scan(field: &ResourceField, policy: PlantReservePolicy) -> (f64, usize) {
    let mut min_headroom = f64::INFINITY;
    let mut below = 0usize;
    for (r, r_max) in field.r.iter().zip(&field.r_max) {
        if *r_max <= 0.0 {
            continue;
        }
        let floor = policy.floor(*r_max);
        min_headroom = min_headroom.min(((*r - floor) / *r_max) as f64);
        if *r < floor {
            below += 1;
        }
    }
    (min_headroom, below)
}

#[test]
fn no_cell_is_grazed_below_the_reserve_in_a_dense_population() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let policy = PlantReservePolicy {
        fraction: RESERVE as f32,
    };

    // Control: the same seed without a reserve, scanned against the same line.
    let mut control = build(&initial(&[]), TICKS);
    let mut d = Driver::new(0);
    let (mut control_below_max, mut control_min) = (0usize, f64::INFINITY);
    for _ in 0..TICKS {
        d.step(&mut control);
        let (min, below) = floor_scan(control.world().resource::<ResourceField>(), policy);
        control_below_max = control_below_max.max(below);
        control_min = control_min.min(min);
    }

    let mut a = build(&initial(&[(LIVE_KEY_PLANT_RESERVE, RESERVE)]), TICKS);
    assert_eq!(*a.world().resource::<PlantReservePolicy>(), policy);
    let mut d = Driver::new(0);
    let (mut peak_agents, mut births, mut min_headroom) = (0.0f64, 0.0f64, f64::INFINITY);
    for tick in 1..=TICKS {
        let o = d.step(&mut a);
        peak_agents = peak_agents.max(obs(&o, "live.agent_count"));
        births += obs(&o, "live.births");
        let (min, below) = floor_scan(a.world().resource::<ResourceField>(), policy);
        min_headroom = min_headroom.min(min);
        assert_eq!(below, 0, "{below} cells below rho*r_max at tick {tick}");
    }
    println!(
        "S10_FLOOR rho={RESERVE} ticks={TICKS} peak_agents={peak_agents} births={births} \
         min_headroom_with_reserve={min_headroom:e} \
         control_min_headroom={control_min:e} control_cells_below_max={control_below_max}"
    );
    assert!(
        peak_agents >= 60.0 && births > 0.0,
        "the population must be dense (peak {peak_agents}, births {births})"
    );
    assert!(
        control_below_max > 0,
        "the control never grazed a cell below rho*r_max, so the floor was never tested"
    );
    assert!(
        min_headroom < 1e-3,
        "with the reserve no cell was grazed down to its floor (min headroom {min_headroom}); \
         the floor was never reached"
    );
}

// ---- (3) closed EU ------------------------------------------------------------------------------

/// Worst relative drift of `live.closed_eu_total` after tick 1, and worst absolute gap between the
/// carried `plants` total and the field it describes.
fn ledger_drift(extra: &[(&str, f64)]) -> (f64, f64) {
    let mut a = build(&initial(extra), TICKS);
    let mut d = Driver::new(0);
    // Tick 1 settles genesis (seeded regrowth), as in `live_turnover_tests.rs`.
    let start = obs(&d.step(&mut a), "live.closed_eu_total");
    let (mut worst, mut worst_gap) = (0.0f64, 0.0f64);
    for _ in 1..TICKS {
        let o = d.step(&mut a);
        let drift = (obs(&o, "live.closed_eu_total") - start).abs() / start.abs().max(1.0);
        worst = worst.max(drift);
        let world = a.world();
        let carried = world.resource::<EcosystemBiomass>().plants;
        let actual = world.resource::<ResourceField>().total_biomass();
        worst_gap = worst_gap.max((carried - actual).abs());
    }
    (worst, worst_gap)
}

#[test]
fn closed_eu_stays_closed_with_a_reserve() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (drift, gap) = ledger_drift(&[(LIVE_KEY_PLANT_RESERVE, RESERVE)]);
    let (control_drift, control_gap) = ledger_drift(&[]);
    println!(
        "S10_EU rho={RESERVE} worst_relative_drift={drift:e} worst_plants_vs_field_gap={gap:e} \
         control_drift={control_drift:e} control_gap={control_gap:e}"
    );
    // The threshold of `live_turnover_tests::closed_eu_stays_closed_with_turnover_on`.
    assert!(drift < 1e-6, "closed EU drifted by {drift} (relative)");
    // The threshold of `ecology_field_stride_tests::plants_tracks_the_field_it_describes`. The
    // closed total is a sum of the carried pools, so it cannot see the field and its mirror part;
    // this can, and the floor pin in `herbivore_grazing_system` is exactly where they could.
    assert!(
        gap < 1e-3,
        "plants drifted {gap} EU from the field it describes"
    );
}

// ---- (4) separate processes --------------------------------------------------------------------

fn reserve_checksum() -> u32 {
    let mut a = build(&initial(&[(LIVE_KEY_PLANT_RESERVE, RESERVE)]), TICKS);
    let mut d = Driver::new(0);
    for _ in 0..TICKS {
        d.step(&mut a);
    }
    a.checksum()
}

/// Child entry point. A plain test that does nothing unless the parent set the role variable, so
/// it needs neither `#[ignore]` nor a target-policy entry (the pattern of `live_turnover_tests.rs`).
#[test]
fn reserve_child_entry_point() {
    if std::env::var(CHILD_ROLE).is_err() {
        return;
    }
    println!("{MARKER} {:#010x}", reserve_checksum());
}

fn checksum_from_child() -> u32 {
    let output = Command::new(std::env::current_exe().expect("current_exe"))
        .args(["--exact", "reserve_child_entry_point", "--nocapture"])
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
fn the_reserve_checksum_agrees_across_four_separate_processes() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sums: Vec<u32> = (0..4).map(|_| checksum_from_child()).collect();
    println!(
        "S10_CHECKSUMS {}",
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

// ---- A checkpoint keeps the reserve ------------------------------------------------------------

fn through_disk(s: &LiveSnapshot) -> LiveSnapshot {
    let dir = std::env::temp_dir().join(format!("anima_s10_{}", std::process::id()));
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

/// The base world without `live.reproduction`. A save/load in the full base world does not follow
/// the uninterrupted run even without a reserve, on the commit before S10 as well: births together
/// with deaths and locomotion leave state the snapshot does not reproduce (reported with card S10).
/// That is not this card's to fix, so the restore check runs where restore is known to hold.
fn restorable(extra: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(
        initial(extra)
            .values
            .into_iter()
            .filter(|(k, _)| k != "live.reproduction")
            .collect(),
    )
}

fn checksum_after(init: &InitialConditionSet, ticks: u64) -> u32 {
    let mut a = build(init, ticks);
    let mut d = Driver::new(0);
    for _ in 0..ticks {
        d.step(&mut a);
    }
    a.checksum()
}

#[test]
fn save_load_mid_run_keeps_the_reserve() {
    let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    const K: u64 = TICKS / 2;
    let init = restorable(&[(LIVE_KEY_PLANT_RESERVE, RESERVE)]);
    let uninterrupted = checksum_after(&init, TICKS);
    assert_ne!(
        uninterrupted,
        checksum_after(&restorable(&[]), TICKS),
        "the reserve must change this world, or a lost reserve would go unnoticed"
    );

    let mut first = build(&init, TICKS);
    let mut d1 = Driver::new(0);
    for _ in 0..K {
        d1.step(&mut first);
    }
    let snap = through_disk(&first.snapshot());
    let mut resumed = LiveExperimentAdapter::from_snapshot(&snap).expect("resume");
    assert_eq!(
        *resumed.world().resource::<PlantReservePolicy>(),
        PlantReservePolicy {
            fraction: RESERVE as f32
        },
        "the resumed world lost its reserve"
    );
    let mut d2 = Driver {
        clock: SimClock { tick: K },
        ..d1
    };
    for _ in 0..(TICKS - K) {
        d2.step(&mut resumed);
    }
    assert_eq!(
        resumed.checksum(),
        uninterrupted,
        "save at {K} -> load -> run the rest diverged"
    );
}

// ---- The declared key --------------------------------------------------------------------------

#[test]
fn the_key_is_declared_and_bounded_to_the_half_open_unit_interval() {
    assert!(LIVE_KEYS.contains(&LIVE_KEY_PLANT_RESERVE));
    assert_eq!(LIVE_KEY_PLANT_RESERVE, "live.plant_reserve");
    let parse = |v: f64| {
        LiveWorldConfig::from_initial_conditions(&InitialConditionSet::new(vec![(
            LIVE_KEY_PLANT_RESERVE.to_string(),
            v,
        )]))
    };
    assert_eq!(LiveWorldConfig::default().plant_reserve, 0.0);
    assert_eq!(parse(0.0).expect("0 is off").plant_reserve, 0.0);
    assert_eq!(parse(0.4).expect("0.4 is valid").plant_reserve, 0.4);
    assert_eq!(parse(0.999).expect("just under 1").plant_reserve, 0.999);
    for bad in [1.0, 1.5, -0.1] {
        match parse(bad) {
            Err(ExperimentError::OutOfRange { field, .. }) => {
                assert_eq!(field, LIVE_KEY_PLANT_RESERVE)
            }
            other => panic!("{bad} must be refused as out of range, got {other:?}"),
        }
    }
    // Non-finite values are refused before any key is read.
    for bad in [f64::NAN, f64::INFINITY] {
        assert!(parse(bad).is_err(), "{bad} must be refused");
    }
}

#[test]
fn a_config_serialized_before_the_key_reads_back_as_no_reserve() {
    let mut json = serde_json::to_value(LiveWorldConfig::default()).expect("serialize");
    json.as_object_mut()
        .expect("object")
        .remove("plant_reserve")
        .expect("the field is serialized");
    let back: LiveWorldConfig = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, LiveWorldConfig::default());
}

// ---- The aggregate tier ------------------------------------------------------------------------

fn individual(tag: u32) -> ArchivedIndividual {
    ArchivedIndividual {
        morphology: MorphologyGenotype::default(),
        brain: None,
        class: AgentClass::Prey,
        lineage_id: format!("s10-{tag}"),
        generation: tag,
    }
}

/// A 64×64 field over `[-100, 100]²` whose capacity varies cell by cell, starting full.
fn varied_field() -> ResourceField {
    let side = 64usize;
    let r_max: Vec<f32> = (0..side * side)
        .map(|i| 2.0 + (i % 7) as f32 * 1.5)
        .collect();
    ResourceField {
        width: side,
        height: side,
        min_x: -100.0,
        min_z: -100.0,
        max_x: 100.0,
        max_z: 100.0,
        regrowth_phase: 0,
        r: r_max.clone(),
        r_max,
        growth_rate: 0.0,
    }
}

/// A large, permanently hungry herd in one chunk: it would strip the cells bare without a reserve.
fn hungry_herd() -> DormantCohorts {
    let mut c = DormantCohorts::new(42, -100.0, -100.0, 100.0, 100.0);
    for tag in 0..200 {
        assert!(c.absorb(
            0.0,
            0.0,
            DormantVitals {
                energy: 1.0,
                hydration: 0.0,
                energy_cap: 1.0e6,
                burn_per_tick: 0.0,
                is_prey: true,
            },
            individual(tag),
        ));
    }
    c
}

#[test]
fn the_aggregate_tier_obeys_the_same_floor_and_stays_conserved() {
    let policy = PlantReservePolicy {
        fraction: RESERVE as f32,
    };
    let mut control_field = varied_field();
    let mut control = hungry_herd();
    let mut field = varied_field();
    let mut herd = hungry_herd();
    let (mut grazed_total, mut control_grazed) = (0.0f64, 0.0f64);
    // Recorded, not asserted: the pre-S10 path measures each cell's loss as an `f32` difference,
    // which is inexact once a cell loses more than half its value (reported with card S10).
    let mut control_worst_gap = 0.0f64;
    for _ in 0..20_000 {
        let control_before = control_field.total_biomass();
        let g = control.graze_all(&mut control_field, 1.0 / 60.0);
        control_worst_gap =
            control_worst_gap.max(((control_before - control_field.total_biomass()) - g).abs());
        control_grazed += g;
        let before_field = field.total_biomass();
        let before_herd = herd.total_energy();
        let grazed = herd.graze_all_with_reserve(&mut field, 1.0 / 60.0, policy);
        let spilled = herd.take_spilled();
        let field_lost = before_field - field.total_biomass();
        assert!(
            (field_lost - grazed).abs() < 1e-9,
            "the field lost {field_lost} but grazing reported {grazed}"
        );
        assert!(
            ((herd.total_energy() - before_herd) + spilled - grazed).abs() < 1e-9,
            "gain plus spill must equal what was grazed"
        );
        grazed_total += grazed;
    }
    let (min_headroom, below) = {
        let mut min = f64::INFINITY;
        let mut below = 0;
        for (r, r_max) in field.r.iter().zip(&field.r_max) {
            min = min.min(((*r - policy.floor(*r_max)) / *r_max) as f64);
            below += usize::from(*r < policy.floor(*r_max));
        }
        (min, below)
    };
    let control_bare = control_field
        .r
        .iter()
        .zip(&control_field.r_max)
        .filter(|(r, r_max)| **r < policy.floor(**r_max))
        .count();
    println!(
        "S10_AGGREGATE grazed={grazed_total} control_grazed={control_grazed} \
         min_headroom={min_headroom:e} cells_below={below} control_cells_below={control_bare} \
         control_worst_field_vs_reported_gap={control_worst_gap:e}"
    );
    assert!(grazed_total > 0.0, "the herd never grazed");
    assert!(
        control_bare > 0,
        "without the reserve the herd never went below rho*r_max, so the floor was never tested"
    );
    assert_eq!(
        below, 0,
        "{below} cells below rho*r_max in the aggregate tier"
    );
    assert!(
        min_headroom < 1e-3,
        "the aggregate herd never reached the floor (min headroom {min_headroom})"
    );
}

#[test]
fn the_aggregate_tier_leaves_a_cell_already_under_its_floor_untouched() {
    let policy = PlantReservePolicy {
        fraction: RESERVE as f32,
    };
    let mut field = varied_field();
    // A cell under its floor (e.g. after a deforestation intervention) offers nothing.
    let probe = field.cell_index(0.0, 0.0).expect("origin is on the grid");
    let low = 0.5 * policy.floor(field.r_max[probe]);
    field.r[probe] = low;
    let mut herd = hungry_herd();
    for _ in 0..1_000 {
        herd.graze_all_with_reserve(&mut field, 1.0 / 60.0, policy);
    }
    assert_eq!(field.r[probe].to_bits(), low.to_bits());
}
