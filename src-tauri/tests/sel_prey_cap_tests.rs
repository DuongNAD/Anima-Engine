//! Card S6 (QD-SEL1b = A): `live.prey_cap` and `live.resource_capacity_scale`.
//!
//! The E-SEL1 diagnosis (`docs/ai/planning/2026-11-17-esel1-scarcity-diagnosis.md`) found that prey
//! reproduction stops at a hard-coded 80 prey, so the population never grows large enough to graze
//! the pasture down and food scarcity cannot limit it. These two keys make the ceiling and the
//! pasture's capacity declared inputs. Absent, the world is exactly today's.
//!
//! No body-mass value is read here: the scarcity line of experiments stays blind to it until a new
//! preregistration is frozen.

use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::intervention::InterventionCommand;
use anima_engine_lib::core::live_experiment::{
    live_observables, LiveExperimentAdapter, LIVE_KEY_DRIVE_FLOOR, LIVE_KEY_FOUNDERS,
    LIVE_KEY_LAKES, LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY, LIVE_KEY_PREDATOR_FRACTION,
    LIVE_KEY_PREY_CAP, LIVE_KEY_REPRODUCTION, LIVE_KEY_RESOURCE_CAPACITY_SCALE,
    LIVE_KEY_THIRST_STEERING, LIVE_KEY_TREES,
};
use anima_engine_lib::core::sim_clock::SimClock;
use rand::SeedableRng;

const SEED: u64 = 7;
const TICKS: u64 = 1_200;
/// The only observables this file reads. Nothing about body mass.
const ALLOWED: [&str; 5] = [
    "live.agent_count",
    "live.births",
    "live.standing_crop",
    "detritus",
    "live.closed_eu_total",
];

/// The E-SEL1 base world (prey only, turnover and steering on), plus `extra` keys.
fn initial(extra: &[(&str, f64)]) -> InitialConditionSet {
    let mut pairs: Vec<(String, f64)> = [
        (LIVE_KEY_FOUNDERS, 30.0),
        (LIVE_KEY_PREDATOR_FRACTION, 0.0),
        (LIVE_KEY_TREES, 8.0),
        (LIVE_KEY_LAKES, 2.0),
        (LIVE_KEY_MORTALITY, 1.0),
        (LIVE_KEY_REPRODUCTION, 1.0),
        (LIVE_KEY_LOCOMOTION, 1.0),
        (LIVE_KEY_DRIVE_FLOOR, 0.5),
        (LIVE_KEY_THIRST_STEERING, 1.0),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), *v))
    .collect();
    pairs.extend(extra.iter().map(|(k, v)| (k.to_string(), *v)));
    InitialConditionSet::new(pairs)
}

fn build(extra: &[(&str, f64)], run_ticks: u64) -> LiveExperimentAdapter {
    LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial(extra),
        &[],
        SEED,
        (16, 16),
        run_ticks,
    )
    .expect("the declared world builds")
}

/// Read the allowed observables only; everything else is dropped unread.
fn allowed(a: &LiveExperimentAdapter) -> Vec<(String, f64)> {
    let all = {
        let mut w = a.world();
        live_observables(&mut w)
    };
    all.into_iter()
        .filter(|(k, _)| ALLOWED.contains(&k.as_str()))
        .collect()
}

fn get(o: &[(String, f64)], id: &str) -> f64 {
    o.iter()
        .find(|(k, _)| k == id)
        .unwrap_or_else(|| panic!("{id} missing"))
        .1
}

/// Run `ticks` ticks; return the final checksum and the total births seen.
fn run(a: &mut LiveExperimentAdapter, ticks: u64) -> (u32, f64) {
    let mut births = 0.0;
    for _ in 0..ticks {
        a.run_schedule_once();
        births += get(&allowed(a), "live.births");
    }
    (a.checksum(), births)
}

#[test]
fn absent_keys_are_the_declared_defaults_bit_for_bit() {
    let (absent, _) = run(&mut build(&[], TICKS), TICKS);
    let (declared, _) = run(
        &mut build(
            &[
                (LIVE_KEY_PREY_CAP, 80.0),
                (LIVE_KEY_RESOURCE_CAPACITY_SCALE, 1.0),
            ],
            TICKS,
        ),
        TICKS,
    );
    assert_eq!(
        absent, declared,
        "omitting the keys must equal declaring the historical 80 and 1.0"
    );
}

#[test]
fn a_ceiling_below_the_population_stops_prey_reproduction() {
    // 30 prey founders: a ceiling of 1 is already exceeded, so no prey may be born. The control
    // with the historical ceiling shows the same world does reproduce.
    let (_, births_capped) = run(&mut build(&[(LIVE_KEY_PREY_CAP, 1.0)], TICKS), TICKS);
    let (_, births_default) = run(&mut build(&[], TICKS), TICKS);
    println!("S6 births cap1={births_capped} cap80={births_default}");
    assert_eq!(births_capped, 0.0, "no prey may be born above the ceiling");
    assert!(
        births_default > 0.0,
        "control: the same world reproduces under the historical ceiling"
    );
}

#[test]
fn a_capacity_scale_moves_pasture_eu_to_detritus_and_keeps_closed_eu() {
    let base = allowed(&build(&[], TICKS));
    let half = allowed(&build(&[(LIVE_KEY_RESOURCE_CAPACITY_SCALE, 0.5)], TICKS));
    let (crop_base, crop_half) = (
        get(&base, "live.standing_crop"),
        get(&half, "live.standing_crop"),
    );
    let (det_base, det_half) = (get(&base, "detritus"), get(&half, "detritus"));
    let (eu_base, eu_half) = (
        get(&base, "live.closed_eu_total"),
        get(&half, "live.closed_eu_total"),
    );
    println!(
        "S6 genesis crop {crop_base}->{crop_half} detritus {det_base}->{det_half} \
         closed_eu {eu_base}->{eu_half}"
    );
    assert!(
        (crop_half / crop_base - 0.5).abs() < 1e-3,
        "the pasture holds half its standing crop at genesis"
    );
    assert!(
        ((det_half - det_base) - (crop_base - crop_half)).abs() < 1e-3 * crop_base,
        "what left the pasture went to detritus"
    );
    assert!(
        (eu_half - eu_base).abs() < 1e-6 * eu_base,
        "the closed-EU total is unchanged by the scale"
    );
}

#[test]
fn out_of_range_values_are_refused() {
    for (key, value) in [
        (LIVE_KEY_PREY_CAP, 0.0),
        (LIVE_KEY_PREY_CAP, 1.5),
        (LIVE_KEY_PREY_CAP, 70_000.0),
        (LIVE_KEY_RESOURCE_CAPACITY_SCALE, 0.0),
        (LIVE_KEY_RESOURCE_CAPACITY_SCALE, -0.5),
        (LIVE_KEY_RESOURCE_CAPACITY_SCALE, 1.2),
    ] {
        let result = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &initial(&[(key, value)]),
            &[],
            SEED,
            (16, 16),
            TICKS,
        );
        assert!(result.is_err(), "{key} = {value} must be refused");
    }
}

/// Step through the `ExperimentModel` interface, as the runner does, so the adapter's clock (which
/// the snapshot records) advances with the world.
struct Driver {
    clock: SimClock,
    ledger: CausalLedger,
    rng: rand::rngs::StdRng,
}

impl Driver {
    fn new() -> Self {
        Self {
            clock: SimClock { tick: 0 },
            ledger: CausalLedger::new(),
            rng: rand::rngs::StdRng::seed_from_u64(SEED),
        }
    }
    fn step_n(&mut self, a: &mut LiveExperimentAdapter, n: u64) {
        let active: Vec<&InterventionCommand> = Vec::new();
        for _ in 0..n {
            self.clock.advance();
            a.step(&self.clock, &active, &mut self.ledger, &mut self.rng);
        }
    }
}

/// Uninterrupted checksum, and the checksum of the same run snapshotted and restored at its
/// midpoint, with the pasture read just before and just after the restore.
fn midpoint_restore(extra: &[(&str, f64)]) -> (u32, u32, f64, f64) {
    let mut whole = build(extra, TICKS);
    Driver::new().step_n(&mut whole, TICKS);
    let uninterrupted = whole.checksum();

    let mut first = build(extra, TICKS);
    let mut driver = Driver::new();
    driver.step_n(&mut first, TICKS / 2);
    let crop_at_snapshot = get(&allowed(&first), "live.standing_crop");
    let snapshot = first.snapshot();
    let mut resumed =
        LiveExperimentAdapter::from_snapshot(&snapshot).expect("the snapshot restores");
    let crop_after_restore = get(&allowed(&resumed), "live.standing_crop");
    driver.step_n(&mut resumed, TICKS - TICKS / 2);
    (
        uninterrupted,
        resumed.checksum(),
        crop_at_snapshot,
        crop_after_restore,
    )
}

#[test]
fn a_restored_world_keeps_its_scaled_capacity_and_its_crop() {
    // Run half, snapshot, restore, run the other half: the result must equal the uninterrupted
    // run. Two ways to get this wrong, and the first draft of S6 did the second one: scaling the
    // saved crop again on restore, or forgetting that a save carries the crop but not the capacity
    // (`r_max` is rebuilt from the biomes), so a restored pasture regrows towards full capacity.
    let (uninterrupted, resumed, crop_at_snapshot, crop_after_restore) = midpoint_restore(&[
        (LIVE_KEY_RESOURCE_CAPACITY_SCALE, 0.5),
        (LIVE_KEY_PREY_CAP, 200.0),
    ]);
    assert!(
        (crop_after_restore - crop_at_snapshot).abs() < 1e-6 * crop_at_snapshot.max(1.0),
        "restore must keep the saved pasture ({crop_at_snapshot} -> {crop_after_restore})"
    );
    assert_eq!(
        resumed, uninterrupted,
        "snapshot and restore at the midpoint must reproduce the uninterrupted run"
    );
}

#[test]
fn control_the_default_world_survives_the_same_midpoint_restore() {
    let (uninterrupted, resumed, _, _) = midpoint_restore(&[]);
    assert_eq!(resumed, uninterrupted);
}

#[test]
fn the_same_declared_world_twice_is_the_same_world() {
    let extra = [
        (LIVE_KEY_RESOURCE_CAPACITY_SCALE, 0.6),
        (LIVE_KEY_PREY_CAP, 160.0),
    ];
    let a = run(&mut build(&extra, TICKS), TICKS);
    let b = run(&mut build(&extra, TICKS), TICKS);
    assert_eq!(a, b);
}
