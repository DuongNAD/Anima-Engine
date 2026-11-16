//! F-043 — `live.food_cap` was inert: a declared food cap never reached the world.
//!
//! The live adapter put the declared cap into `FoodSpawnSettings::max_food_count` at genesis, but
//! `apply_environmental_effects_system` rewrote that field on every tick as `50 × event
//! multiplier`, with a literal 50. So from the first tick on, every live world ran with the
//! default cap whatever its manifest said. The E-SEL1 smoke found it: at seed 999971 the scarce arm
//! (`live.food_cap` 25, 17, 13, 6) ended on the same final checksum as the abundant arm (50), bit
//! for bit, so the experiment's declared factor did nothing.
//!
//! FIXED by `LiveFoodCap`: the adapter declares the base cap (on genesis and on restore), and the
//! system scales that base instead of the literal. A world without the resource — every app run —
//! keeps the literal 50, so the default path is unchanged.

use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::ecs::{ActiveEnvironmentEvent, FoodSpawnSettings};
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LiveFoodCap, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS, LIVE_KEY_LAKES,
    LIVE_KEY_PREDATOR_FRACTION, LIVE_KEY_TREES,
};
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::evolution::meta_ai::EnvironmentalEvent;
use rand::SeedableRng;

const SEED: u64 = 0xF043;
const TICKS: u64 = 600;
const SCARCE_CAP: usize = 6;

fn initials(food_cap: Option<f64>) -> InitialConditionSet {
    let mut v = vec![
        (LIVE_KEY_FOUNDERS.to_string(), 10.0),
        (LIVE_KEY_PREDATOR_FRACTION.to_string(), 0.0),
        (LIVE_KEY_TREES.to_string(), 8.0),
        (LIVE_KEY_LAKES.to_string(), 2.0),
    ];
    if let Some(cap) = food_cap {
        v.push((LIVE_KEY_FOOD_CAP.to_string(), cap));
    }
    InitialConditionSet::new(v)
}

fn build(food_cap: Option<f64>) -> LiveExperimentAdapter {
    LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initials(food_cap),
        &[],
        SEED,
        (16, 16),
        TICKS,
    )
    .expect("the live world builds")
}

/// Step `adapter` from `clock` for `n` ticks the way the experiment runner does, calling `each`
/// after every tick.
fn run(
    adapter: &mut LiveExperimentAdapter,
    clock: &mut SimClock,
    n: u64,
    rng: &mut rand::rngs::StdRng,
    ledger: &mut CausalLedger,
    mut each: impl FnMut(&LiveExperimentAdapter),
) {
    for _ in 0..n {
        clock.advance();
        adapter.step(clock, &[], ledger, rng);
        each(adapter);
    }
}

fn observable(adapter: &LiveExperimentAdapter, id: &str) -> f64 {
    adapter
        .observables()
        .into_iter()
        .find_map(|(k, v)| (k == id).then_some(v))
        .unwrap_or_else(|| panic!("the live world reports no `{id}`"))
}

/// The cap in force on a tick, and the cap the declared base implies for the tick's event.
fn cap_in_force(adapter: &LiveExperimentAdapter, base: usize) -> (usize, usize) {
    let world = adapter.world();
    let multiplier = match world.resource::<ActiveEnvironmentEvent>().0 {
        EnvironmentalEvent::ResourceDrought => 0.5,
        EnvironmentalEvent::ToxicDeluge => 0.8,
        _ => 1.0,
    };
    (
        world.resource::<FoodSpawnSettings>().max_food_count,
        (base as f64 * multiplier) as usize,
    )
}

/// Final checksum and the sum of `live.food_items` over every tick of a `TICKS`-tick run.
fn trajectory(food_cap: Option<f64>) -> (u32, f64) {
    let mut adapter = build(food_cap);
    let mut rng = rand::rngs::StdRng::seed_from_u64(SEED);
    let mut ledger = CausalLedger::new();
    let mut clock = SimClock::new();
    let mut food_items = 0.0;
    run(
        &mut adapter,
        &mut clock,
        TICKS,
        &mut rng,
        &mut ledger,
        |a| {
            food_items += observable(a, "live.food_items");
        },
    );
    (adapter.checksum(), food_items)
}

#[test]
fn a_declared_food_cap_is_the_cap_in_force_on_every_tick() {
    let mut adapter = build(Some(SCARCE_CAP as f64));
    assert_eq!(
        *adapter.world().resource::<LiveFoodCap>(),
        LiveFoodCap(SCARCE_CAP),
        "the adapter declares the manifest's cap"
    );
    let mut rng = rand::rngs::StdRng::seed_from_u64(SEED);
    let mut ledger = CausalLedger::new();
    let mut clock = SimClock::new();
    let mut ticks = 0;
    run(
        &mut adapter,
        &mut clock,
        TICKS,
        &mut rng,
        &mut ledger,
        |a| {
            let (in_force, implied) = cap_in_force(a, SCARCE_CAP);
            assert_eq!(
            in_force, implied,
            "tick {ticks}: the cap in force is not the declared {SCARCE_CAP} scaled by the event; \
             a literal base has overwritten it again (F-043)"
        );
            ticks += 1;
        },
    );
    assert_eq!(ticks, TICKS);
}

#[test]
fn a_scarce_cap_changes_the_world_and_feeds_it_less() {
    // The inverse of what the E-SEL1 smoke found: there, cap 6 and cap 50 ended bit-identical.
    let (scarce_checksum, scarce_food) = trajectory(Some(SCARCE_CAP as f64));
    let (abundant_checksum, abundant_food) = trajectory(Some(50.0));
    assert_ne!(
        scarce_checksum, abundant_checksum,
        "a food cap of {SCARCE_CAP} and one of 50 ran the same world; the declared factor is inert"
    );
    assert!(
        scarce_food < abundant_food,
        "a scarce cap must leave fewer food items standing: {scarce_food} vs {abundant_food}"
    );
}

#[test]
fn omitting_the_key_and_declaring_the_default_cap_run_the_same_world() {
    // The default cap is 50, the literal the system used before the fix, so a manifest without
    // `live.food_cap` (and every world without `LiveFoodCap`) keeps its old trajectory.
    assert_eq!(FoodSpawnSettings::default().max_food_count, 50);
    assert_eq!(trajectory(None), trajectory(Some(50.0)));
}

#[test]
fn a_resumed_run_keeps_its_declared_cap() {
    const K: u64 = TICKS / 3;
    let (reference, _) = trajectory(Some(SCARCE_CAP as f64));

    let mut first = build(Some(SCARCE_CAP as f64));
    let mut rng = rand::rngs::StdRng::seed_from_u64(SEED);
    let mut ledger = CausalLedger::new();
    let mut clock = SimClock::new();
    run(&mut first, &mut clock, K, &mut rng, &mut ledger, |a| {
        let _ = a.observables();
    });
    let mut resumed =
        LiveExperimentAdapter::from_snapshot(&first.snapshot()).expect("the snapshot resumes");
    assert_eq!(
        *resumed.world().resource::<LiveFoodCap>(),
        LiveFoodCap(SCARCE_CAP),
        "a restore must declare the manifest's cap again, not fall back to 50"
    );
    let mut clock = SimClock { tick: K };
    run(
        &mut resumed,
        &mut clock,
        TICKS - K,
        &mut rng,
        &mut ledger,
        |a| {
            let (in_force, implied) = cap_in_force(a, SCARCE_CAP);
            assert_eq!(in_force, implied, "a resumed run fell back to another cap");
            let _ = a.observables();
        },
    );
    assert_eq!(
        resumed.checksum(),
        reference,
        "a run resumed at tick {K} diverged from the uninterrupted one"
    );
}
