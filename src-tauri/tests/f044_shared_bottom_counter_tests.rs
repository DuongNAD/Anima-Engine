//! F-044: the starvation and the thirst grace must be counted separately (card S8, QD-F044).
//!
//! `metabolic_decay_system` used to advance one counter, `HomeostaticState::starving_ticks`, while
//! energy **or** hydration was at the bottom, and `mortality_system` compared both `starve_ticks`
//! and `dehydration_ticks` with that one counter. The two graces were therefore not independent: an
//! animal that had been hungry for a long time died of thirst on the first tick its water ran out,
//! without the thirst grace it was declared.
//!
//! This file was a CHARACTERIZATION test of that smell (#90). The owner decided on 2026-11-17 to split
//! the counters before any new preregistration, so the first test is inverted. A long-hungry animal
//! now lives through its full thirst grace, exactly like the fed control.

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
fn f044_a_long_hungry_animal_still_gets_its_full_thirst_grace() {
    // Energy 0 from the start: the hunger counter climbs through the watered ticks, far past the
    // thirst grace. That must not shorten the thirst grace.
    let (survived, counter) = ticks_survived_after_thirst(0.0);
    println!("F044 hungry: counter_at_thirst={counter} ticks_survived_after_thirst={survived}");
    assert!(
        counter >= DEHYDRATION_TICKS,
        "the hunger counter must have passed the thirst grace before the water runs out ({counter}), \
         or this test cannot tell a shared counter from two"
    );
    assert!(
        (DEHYDRATION_TICKS - 2..=DEHYDRATION_TICKS).contains(&survived),
        "a long-hungry founder must live through its thirst grace of {DEHYDRATION_TICKS} ticks like \
         the fed control, got {survived}: the hunger streak is still being charged to the thirst grace"
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

#[test]
fn a_homeostatic_state_saved_before_the_split_loads_with_no_thirst_streak_recorded() {
    // The JSON a save from the shared-counter era carries: `starving_ticks`, no `dehydrating_ticks`.
    let legacy = r#"{"energy":50.0,"energy_target":100.0,"hydration":0.0,"hydration_target":100.0,
        "temperature":37.0,"temp_target":37.0,"previous_deviation":0.0,"starving_ticks":7}"#;
    let homeo: HomeostaticState = serde_json::from_str(legacy).expect("an old save still loads");
    assert_eq!(homeo.starving_ticks, 7);
    assert_eq!(
        homeo.dehydrating_ticks, None,
        "a missing thirst streak must read as unknown, not as 0"
    );
}

/// Restore an animal from the shared-counter era that is out of water but fed, with 7 ticks on its
/// shared counter and a thirst grace of 10, and return the tick it dies on.
fn death_tick_of_a_restored_thirsty_animal(thirst_streak: Option<u32>) -> u32 {
    let adapter = lone_founder(50.0);
    {
        let mut world = adapter.world();
        let mut q = world.query_filtered::<&mut HomeostaticState, With<Agent>>();
        let mut homeo = q.iter_mut(&mut world).next().expect("the founder exists");
        homeo.hydration = 0.0;
        homeo.starving_ticks = 7;
        homeo.dehydrating_ticks = thirst_streak;
    }
    let mut adapter = adapter;
    for tick in 1..=MAX_TICKS_AFTER_THIRST {
        adapter.run_schedule_once();
        if tick == 1 {
            if let Some(homeo) = founder(&adapter) {
                if thirst_streak.is_none() {
                    assert_eq!(
                        homeo.dehydrating_ticks,
                        Some(8),
                        "the first tick carries the shared count over as the thirst streak"
                    );
                }
                assert_eq!(homeo.starving_ticks, 0, "a fed animal has no hunger streak");
            }
        }
        if founder(&adapter).is_none() {
            return tick;
        }
    }
    panic!("an animal without water must die within {MAX_TICKS_AFTER_THIRST} ticks");
}

#[test]
fn a_thirsty_animal_restored_from_the_shared_counter_era_keeps_its_progress() {
    // The old rule charged the shared counter to thirst: 7 ticks already spent of a 10-tick grace,
    // so the animal dies on the third tick after the restore. A missing field read as a fresh 0
    // would give it the whole grace again.
    let carried = death_tick_of_a_restored_thirsty_animal(None);
    let fresh = death_tick_of_a_restored_thirsty_animal(Some(0));
    println!("F044 restore: dies_at_tick carried={carried} fresh_counter={fresh}");
    assert_eq!(
        carried,
        DEHYDRATION_TICKS - 7,
        "a restored animal must die when the old shared counter would have killed it"
    );
    assert_eq!(
        fresh, DEHYDRATION_TICKS,
        "control: a recorded streak of 0 gets the full grace"
    );
}
