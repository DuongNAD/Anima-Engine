//! F-044 (SMELL, OPEN): the starvation and the thirst grace share one counter.
//!
//! `metabolic_decay_system` advances `HomeostaticState::starving_ticks` while energy **or** hydration
//! is at the bottom, and `mortality_system` compares both `starve_ticks` and `dehydration_ticks` with
//! that one counter. When the two graces differ they are not independent: an animal that has been
//! hungry for a long time dies of thirst on the first tick its water runs out, without the thirst
//! grace it was declared. With the defaults (120 / 120) the rule is "120 consecutive bottom ticks of
//! either kind", so no run made so far is affected.
//!
//! This is a CHARACTERIZATION test, like F-042's: it pins today's behaviour so the smell cannot
//! change silently, and it must be INVERTED when the counters are split (an owner decision, because
//! the fix adds a field to `HomeostaticState`, touches the save format and changes the trajectory
//! of every configuration whose two graces differ). The control shows the thirst grace itself works.

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::ecs::{Agent, EpochManager};
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LIVE_KEY_DEHYDRATION_TICKS, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS,
    LIVE_KEY_LAKES, LIVE_KEY_MORTALITY, LIVE_KEY_PREDATOR_FRACTION, LIVE_KEY_STARVE_TICKS,
    LIVE_KEY_TREES,
};
use bevy_ecs::prelude::*;

const SEED: u64 = 0xF044;
/// A starvation grace the test never reaches, so hunger alone cannot kill.
const STARVE_TICKS: u32 = 65_536;
/// The declared thirst grace.
const DEHYDRATION_TICKS: u32 = 10;
/// Ticks of water before it is taken away.
const WATERED_TICKS: u32 = 200;
const MAX_TICKS_AFTER_THIRST: u32 = 100;

/// One founder in a world with nothing to eat or drink: no predators, trees, lakes or food items,
/// and an emptied resource field. Mortality is declared with a huge starvation grace and a short
/// thirst grace. The founder starts with `energy` and plenty of water.
fn lone_founder(energy: f32) -> LiveExperimentAdapter {
    let initials = InitialConditionSet::new(vec![
        (LIVE_KEY_FOUNDERS.to_string(), 1.0),
        (LIVE_KEY_PREDATOR_FRACTION.to_string(), 0.0),
        (LIVE_KEY_TREES.to_string(), 0.0),
        (LIVE_KEY_LAKES.to_string(), 0.0),
        (LIVE_KEY_FOOD_CAP.to_string(), 0.0),
        (LIVE_KEY_MORTALITY.to_string(), 1.0),
        (LIVE_KEY_STARVE_TICKS.to_string(), STARVE_TICKS as f64),
        (
            LIVE_KEY_DEHYDRATION_TICKS.to_string(),
            DEHYDRATION_TICKS as f64,
        ),
    ]);
    let adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initials,
        &[],
        SEED,
        (16, 16),
        0,
    )
    .expect("a one-founder world with a declared death rule builds");
    {
        let mut world = adapter.world();
        {
            let mut field = world.resource_mut::<ResourceField>();
            field.r.fill(0.0);
            field.r_max.fill(0.0);
        }
        if let Some(mut epochs) = world.get_resource_mut::<EpochManager>() {
            epochs.ticks_per_epoch = 1_000_000;
        }
        let mut q = world.query_filtered::<&mut HomeostaticState, With<Agent>>();
        for mut homeo in q.iter_mut(&mut world) {
            homeo.energy = energy;
            homeo.energy_target = 100.0;
            homeo.hydration = 5_000.0;
            homeo.starving_ticks = 0;
        }
        *world.resource_mut::<EcosystemBiomass>() = EcosystemBiomass {
            detritus: 0.0,
            plants: 0.0,
            animals: energy as f64,
            dormant: 0.0,
        };
    }
    adapter
}

fn founder(adapter: &LiveExperimentAdapter) -> Option<HomeostaticState> {
    let mut world = adapter.world();
    let mut q = world.query_filtered::<&HomeostaticState, With<Agent>>();
    q.iter(&world).next().cloned()
}

/// Run `WATERED_TICKS` ticks with water, take the water away, and return how many more ticks the
/// founder lived, together with its counter when the water ran out.
fn ticks_survived_after_thirst(energy: f32) -> (u32, u32) {
    let mut adapter = lone_founder(energy);
    for _ in 0..WATERED_TICKS {
        adapter.run_schedule_once();
    }
    let counter_at_thirst = {
        let mut world = adapter.world();
        let mut q = world.query_filtered::<&mut HomeostaticState, With<Agent>>();
        let mut homeo = q
            .iter_mut(&mut world)
            .next()
            .expect("the founder is alive while it has water");
        homeo.hydration = 0.0;
        homeo.starving_ticks
    };
    for tick in 1..=MAX_TICKS_AFTER_THIRST {
        adapter.run_schedule_once();
        if founder(&adapter).is_none() {
            return (tick - 1, counter_at_thirst);
        }
    }
    panic!("a founder without water must die within {MAX_TICKS_AFTER_THIRST} ticks");
}

#[test]
fn f044_a_long_hungry_animal_gets_no_thirst_grace() {
    // Energy 0 from the start: the shared counter climbs through the watered ticks.
    let (survived, counter) = ticks_survived_after_thirst(0.0);
    println!("F044 hungry: counter_at_thirst={counter} ticks_survived_after_thirst={survived}");
    assert!(
        counter >= DEHYDRATION_TICKS,
        "the hunger ticks must have filled the shared counter ({counter})"
    );
    // PINNED SMELL: with independent counters the founder would live ~DEHYDRATION_TICKS - 1 more
    // ticks. Invert this when F-044 is fixed.
    assert!(
        survived < 2,
        "a long-hungry founder now lives {survived} ticks without water; if the counters were split \
         (F-044 fixed), invert this assertion to expect about {} ticks",
        DEHYDRATION_TICKS - 1
    );
}

#[test]
fn f044_control_a_fed_animal_does_get_its_thirst_grace() {
    // Energy 50 never reaches the bottom in these ticks, so the counter starts at thirst.
    let (survived, counter) = ticks_survived_after_thirst(50.0);
    println!("F044 fed: counter_at_thirst={counter} ticks_survived_after_thirst={survived}");
    assert_eq!(counter, 0, "a fed, watered founder is not at the bottom");
    assert!(
        (DEHYDRATION_TICKS - 2..=DEHYDRATION_TICKS).contains(&survived),
        "a fed founder lives through its thirst grace of {DEHYDRATION_TICKS} ticks, got {survived}"
    );
}
