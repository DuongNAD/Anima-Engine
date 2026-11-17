//! STATE §3.00 P1 — a declared configuration in which predators and prey live together.
//!
//! P1's definition of done: a configuration declared through manifest keys (absent keys = the old
//! world) under which predators and prey are both alive at t = 6,000 in at least three seeds, with
//! closed EU, multi-process determinism, and a behavioural test that locks it. The configuration is
//! ADR-0006 locomotion, ADR-0007 card A (`live.drive_floor`, `live.thirst_steering`) and card B
//! (`live.kill_hydration`, `live.kill_efficiency`, `live.target_min_energy`) at the point the
//! exploration measured (k = 1, e = 0.6, E = 10). Closed EU, determinism across processes and
//! restore with these keys on are covered by `eco_card_b_tests`; this file is the behavioural lock.
//!
//! The exploratory measurement on macOS arm64 (2026-11-17, seeds 1-10) found coexistence at seeds 2,
//! 4, 6, 8 and 10, i.e. 5 of 10. A trajectory is chaotic and a world checksum differs between
//! platforms, so which seeds coexist may differ on Windows or Linux; the assertion is therefore a
//! count with a margin (at least 3 of 10), never a list of seeds and never a checksum.
//!
//! What this does NOT claim: Red-Queen coevolution (S43), that coexistence is stable past 6,000
//! ticks, or that the card B values are anything but measurement points. Seeds where it fails do so
//! for two measured reasons (ADR-0007, "Bằng chứng cho phương án C"): founder predators that
//! reproduce before their first kill and starve on the way to prey, and prey populations that
//! water, not predators, holds down.

use anima_engine_lib::core::components::{Agent, ParentLink, Predator, Prey};
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LIVE_KEY_DRIVE_FLOOR, LIVE_KEY_FOUNDERS, LIVE_KEY_KILL_EFFICIENCY,
    LIVE_KEY_KILL_HYDRATION, LIVE_KEY_LAKES, LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY,
    LIVE_KEY_PREDATOR_FRACTION, LIVE_KEY_REPRODUCTION, LIVE_KEY_TARGET_MIN_ENERGY,
    LIVE_KEY_THIRST_STEERING, LIVE_KEY_TREES,
};
use bevy_ecs::prelude::*;

const TICKS: u32 = 6_000;
const SEEDS: std::ops::RangeInclusive<u64> = 1..=10;
/// P1's threshold. The macOS measurement gave 5; the margin absorbs platform-dependent trajectories.
const MIN_COEXISTING_SEEDS: usize = 3;

fn declared_configuration() -> InitialConditionSet {
    InitialConditionSet::new(
        [
            (LIVE_KEY_FOUNDERS, 10.0),
            (LIVE_KEY_PREDATOR_FRACTION, 0.3),
            (LIVE_KEY_TREES, 8.0),
            (LIVE_KEY_LAKES, 2.0),
            (LIVE_KEY_MORTALITY, 1.0),
            (LIVE_KEY_REPRODUCTION, 1.0),
            (LIVE_KEY_LOCOMOTION, 1.0),
            (LIVE_KEY_DRIVE_FLOOR, 0.5),
            (LIVE_KEY_THIRST_STEERING, 1.0),
            (LIVE_KEY_KILL_HYDRATION, 1.0),
            (LIVE_KEY_KILL_EFFICIENCY, 0.6),
            (LIVE_KEY_TARGET_MIN_ENERGY, 10.0),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect(),
    )
}

/// `(predators, prey)` alive: root bodies only, as the census counts them.
fn census(adapter: &LiveExperimentAdapter) -> (usize, usize) {
    let mut world = adapter.world();
    let predators = world
        .query_filtered::<Entity, (With<Agent>, With<Predator>, Without<ParentLink>)>()
        .iter(&world)
        .count();
    let prey = world
        .query_filtered::<Entity, (With<Agent>, With<Prey>, Without<ParentLink>)>()
        .iter(&world)
        .count();
    (predators, prey)
}

/// Run one seed to t = `TICKS`; return the final census and the predators' minimum and maximum.
fn run_seed(seed: u64) -> ((usize, usize), (usize, usize)) {
    let mut adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &declared_configuration(),
        &[],
        seed,
        (16, 16),
        0,
    )
    .expect("the declared configuration builds");
    let (mut pred_min, mut pred_max) = (usize::MAX, 0);
    for _ in 0..TICKS {
        adapter.run_schedule_once();
        let (p, _) = census(&adapter);
        pred_min = pred_min.min(p);
        pred_max = pred_max.max(p);
    }
    (census(&adapter), (pred_min, pred_max))
}

#[test]
fn predators_and_prey_coexist_at_t6000_in_at_least_three_of_ten_seeds() {
    let mut coexisting = Vec::new();
    for seed in SEEDS {
        let ((predators, prey), (pred_min, pred_max)) = run_seed(seed);
        println!(
            "P1_COEXIST seed={seed} predators={predators} prey={prey} predators_min={pred_min} \
             predators_max={pred_max}"
        );
        if predators > 0 && prey > 0 {
            coexisting.push(seed);
        }
    }
    println!(
        "P1_COEXIST seeds_coexisting={} of {} ({coexisting:?})",
        coexisting.len(),
        SEEDS.count()
    );
    assert!(
        coexisting.len() >= MIN_COEXISTING_SEEDS,
        "only {} of 10 seeds keep predators and prey alive to t = {TICKS} under the declared \
         configuration ({coexisting:?}); P1 needs at least {MIN_COEXISTING_SEEDS}",
        coexisting.len()
    );
}
