//! F-037 — under the trunk's dual mortality schedule, a declared `starve_ticks` grace never runs,
//! and `live.deaths` reports a zero while the animals die.
//!
//! `core::simulation_schedule::build_tick_schedule` registers both
//! `core::agent_systems::agent_mortality_system` and `core::mortality::mortality_system`. The first
//! returns early only when a `MortalityPolicy` is present **and disabled**. With a death rule
//! declared (`live.mortality = 1` in a manifest, `ANIMA_MORTALITY` in the app) it runs too, and it
//! kills any agent whose energy is at or below zero on the first such tick. `mortality_system`
//! would wait for `starving_ticks >= starve_ticks`, and it is the only writer of
//! `VitalStatistics`, which `live.deaths` and `live.mean_lifespan` drain. So the policy's grace is
//! dead code, and the turnover observables count none of the deaths that actually happen.
//! `LiveExperimentAdapter::ensure_observables_supported` refuses `live.deaths` without a death rule
//! precisely so it can never report a fabricated zero; with one declared, it reports one.
//!
//! This file pins today's behaviour so it cannot drift silently. QĐ-A′ (owner, 2026-11-14) drops
//! `agent_mortality_system` from the schedule. When that lands every pinned value below flips, and
//! the test must be inverted to assert the grace and the counted death, not deleted.

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::ecs::{Agent, EpochManager};
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    live_observables, LiveExperimentAdapter, LIVE_KEY_DEHYDRATION_TICKS, LIVE_KEY_FOOD_CAP,
    LIVE_KEY_FOUNDERS, LIVE_KEY_LAKES, LIVE_KEY_MORTALITY, LIVE_KEY_PREDATOR_FRACTION,
    LIVE_KEY_STARVE_TICKS, LIVE_KEY_TREES,
};
use anima_engine_lib::core::mortality::mortality_system;
use anima_engine_lib::core::resources::{MortalityPolicy, VitalStatistics};
use anima_engine_lib::core::telemetry_export::DemographicTracker;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ExecutorKind;

const SEED: u64 = 0x5EED;
/// Long enough that a death honouring it is unmistakable against zero ticks at the bottom.
const STARVE_TICKS: u32 = 30;
const MAX_TICKS: u32 = 2_000;

/// One founder, no predators, food, trees or lakes, and an emptied resource field: it can only
/// starve. Mortality is declared the way a manifest declares it, with a 30-tick starvation grace;
/// dehydration is pushed out of reach so it cannot be the cause.
fn starving_founder_world() -> LiveExperimentAdapter {
    let initials = InitialConditionSet::new(vec![
        (LIVE_KEY_FOUNDERS.to_string(), 1.0),
        (LIVE_KEY_PREDATOR_FRACTION.to_string(), 0.0),
        (LIVE_KEY_TREES.to_string(), 0.0),
        (LIVE_KEY_LAKES.to_string(), 0.0),
        (LIVE_KEY_FOOD_CAP.to_string(), 0.0),
        (LIVE_KEY_MORTALITY.to_string(), 1.0),
        (LIVE_KEY_STARVE_TICKS.to_string(), STARVE_TICKS as f64),
        (LIVE_KEY_DEHYDRATION_TICKS.to_string(), 65_536.0),
    ]);
    let adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initials,
        &[],
        SEED,
        (16, 16),
        0,
    )
    .expect("a live world with one founder and a declared death rule builds");
    {
        let mut world = adapter.world();
        assert_eq!(
            *world.resource::<MortalityPolicy>(),
            MortalityPolicy {
                enabled: true,
                starve_ticks: STARVE_TICKS,
                dehydration_ticks: 65_536,
            },
            "the manifest keys are what reaches the world"
        );
        // Live worlds do not carry the app's tracker. It is inserted here only to attribute the
        // death: `agent_mortality_system` is its sole writer.
        world.insert_resource(DemographicTracker::default());
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
            homeo.energy = 2.0;
            homeo.energy_target = 100.0;
            homeo.hydration = 500.0;
            homeo.starving_ticks = 0;
        }
        *world.resource_mut::<EcosystemBiomass>() = EcosystemBiomass {
            detritus: 0.0,
            plants: 0.0,
            animals: 2.0,
            dormant: 0.0,
        };
    }
    adapter
}

fn observable(sample: &[(String, f64)], id: &str) -> f64 {
    sample
        .iter()
        .find_map(|(name, value)| (name == id).then_some(*value))
        .unwrap_or_else(|| panic!("live_observables has no `{id}`"))
}

#[test]
fn a_declared_starvation_grace_is_bypassed_and_live_deaths_reports_zero() {
    let mut adapter = starving_founder_world();

    // The most consecutive bottom ticks the founder was ever seen alive with, after a tick ran.
    let mut most_bottom_ticks_alive = 0_u32;
    // `live.deaths` summed over every tick's window, sampled exactly as the experiment runner does.
    let mut live_deaths = 0.0_f64;
    let mut death_tick = None;
    for tick in 1..=MAX_TICKS {
        adapter.run_schedule_once();
        let mut world = adapter.world();
        let sample = live_observables(&mut world);
        live_deaths += observable(&sample, "live.deaths");
        let mut q = world.query_filtered::<&HomeostaticState, With<Agent>>();
        match q.iter(&world).next() {
            Some(homeo) => {
                most_bottom_ticks_alive = most_bottom_ticks_alive.max(homeo.starving_ticks)
            }
            None => {
                assert_eq!(observable(&sample, "live.agent_count"), 0.0);
                death_tick = Some(tick);
                break;
            }
        }
    }
    let death_tick = death_tick
        .unwrap_or_else(|| panic!("a founder with no food must die within {MAX_TICKS} ticks"));
    let tracker_starvation = adapter
        .world()
        .resource::<DemographicTracker>()
        .deaths_starvation;
    println!(
        "F037 death_tick={death_tick} most_bottom_ticks_alive={most_bottom_ticks_alive} \
         starve_ticks={STARVE_TICKS} tracker_starvation={tracker_starvation} live_deaths={live_deaths}"
    );

    // One assertion, so a change reports all three values at once. Pinned today:
    // - 0 bottom ticks survived: killed the tick its energy ran out, not after `STARVE_TICKS`;
    // - 1 death in `DemographicTracker`, so `agent_mortality_system` did the killing;
    // - 0 in `live.deaths`: `mortality_system`, the only writer of `VitalStatistics`, never fired.
    // After QD-A' expect (STARVE_TICKS - 1, 0, 1.0), what the control below measures: invert this
    // test then, do not delete it.
    assert_eq!(
        (most_bottom_ticks_alive, tracker_starvation, live_deaths),
        (0, 1, 0.0),
        "(bottom ticks survived, DemographicTracker starvation deaths, summed live.deaths) changed: \
         if agent_mortality_system has left the schedule (QD-A'), invert this test"
    );
}

/// Control: `mortality_system` alone honours the grace and records the death, so the pinned values
/// above come from the schedule running `agent_mortality_system` beside it, not from this rule.
///
/// A hand-built one-system schedule, on purpose: it isolates the rule and claims nothing about the
/// app's schedule. The test advances `starving_ticks` itself, as `metabolic_decay_system` does for
/// an animal at the bottom.
#[test]
fn mortality_system_alone_honours_the_grace_and_records_the_death() {
    let mut world = World::new();
    world.insert_resource(EcosystemBiomass::default());
    world.insert_resource(VitalStatistics::default());
    world.insert_resource(MortalityPolicy {
        enabled: true,
        starve_ticks: STARVE_TICKS,
        dehydration_ticks: u32::MAX,
    });
    world.spawn((
        Agent,
        HomeostaticState {
            energy: 0.0,
            energy_target: 100.0,
            hydration: 500.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
            starving_ticks: 0,
        },
    ));
    let mut schedule = Schedule::default();
    schedule.set_executor_kind(ExecutorKind::SingleThreaded);
    schedule.add_systems((mortality_system, apply_deferred).chain());

    let mut most_bottom_ticks_alive = 0_u32;
    for _ in 0..=STARVE_TICKS {
        let mut q = world.query_filtered::<&mut HomeostaticState, With<Agent>>();
        let Some(mut homeo) = q.iter_mut(&mut world).next() else {
            break;
        };
        homeo.starving_ticks += 1;
        schedule.run(&mut world);
        let mut q = world.query_filtered::<&HomeostaticState, With<Agent>>();
        if let Some(homeo) = q.iter(&world).next() {
            most_bottom_ticks_alive = most_bottom_ticks_alive.max(homeo.starving_ticks);
        }
    }
    let alive = world
        .query_filtered::<(), With<Agent>>()
        .iter(&world)
        .count();
    let (_, window_deaths, _) = world.resource_mut::<VitalStatistics>().sample_window();

    // Alive through every bottom tick short of the grace, dead on the tick that reaches it, and
    // counted where `live.deaths` reads.
    assert_eq!(
        (most_bottom_ticks_alive, alive, window_deaths),
        (STARVE_TICKS - 1, 0, 1),
        "(bottom ticks survived, agents left, VitalStatistics window deaths)"
    );
}
