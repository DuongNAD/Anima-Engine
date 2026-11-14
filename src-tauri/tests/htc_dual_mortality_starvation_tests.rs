//! Card T2 (HT-C) dual-mortality closed-EU measurement and regression guard.
//!
//! After merging `fix/ci-gates-green` into `feat/ecological-selection`,
//! `core::simulation_schedule::build_tick_schedule` registers BOTH
//! `core::agent_systems::agent_mortality_system` and `core::mortality::mortality_system` in the
//! same tick, and both terminate dying agents via `ReclaimAndDespawnAgentCommand`.
//!
//! These tests measure whether enabling `MortalityPolicy { enabled: true, .. }` under that
//! dual-mortality schedule credits `EcosystemBiomass::detritus` with a dying agent's energy
//! reserve strictly ONCE (preserving the closed EU budget) rather than twice, and act as a
//! regression guard against ever splitting reserve reclamation from entity despawn inside
//! `ReclaimAndDespawnAgentCommand::apply`.

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::ReclaimAndDespawnAgentCommand;
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::ecs::{Agent, EpochManager};
use anima_engine_lib::core::energy_ledger::{closed_total_eu, RESIDUAL_ABS_TOLERANCE_EU};
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    LiveExperimentAdapter, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS, LIVE_KEY_LAKES,
    LIVE_KEY_PREDATOR_FRACTION, LIVE_KEY_TREES,
};
use anima_engine_lib::core::resources::{MortalityPolicy, VitalStatistics};
use anima_engine_lib::core::telemetry_export::DemographicTracker;
use bevy_ecs::prelude::*;
use bevy_ecs::system::CommandQueue;

const SEED: u64 = 0x5EED;

fn deprived_single_founder_initials() -> InitialConditionSet {
    InitialConditionSet::new(vec![
        (LIVE_KEY_FOUNDERS.to_string(), 1.0),
        (LIVE_KEY_PREDATOR_FRACTION.to_string(), 0.0),
        (LIVE_KEY_TREES.to_string(), 0.0),
        (LIVE_KEY_LAKES.to_string(), 0.0),
        (LIVE_KEY_FOOD_CAP.to_string(), 0.0),
    ])
}

fn build_isolated_world(initial_animal_energy: f32) -> LiveExperimentAdapter {
    let adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &deprived_single_founder_initials(),
        &[],
        SEED,
        (16, 16),
        0,
    )
    .expect("live experiment adapter builds for single deprived founder");

    {
        let mut world = adapter.world();
        if !world.contains_resource::<DemographicTracker>() {
            world.insert_resource(DemographicTracker::default());
        }
        if !world.contains_resource::<VitalStatistics>() {
            world.insert_resource(VitalStatistics::default());
        }
        {
            let mut field = world.resource_mut::<ResourceField>();
            field.r.fill(0.0);
            field.r_max.fill(0.0);
        }
        if let Some(mut epoch_mgr) = world.get_resource_mut::<EpochManager>() {
            epoch_mgr.ticks_per_epoch = 1_000_000;
        }
        {
            let mut q = world.query_filtered::<&mut HomeostaticState, With<Agent>>();
            for mut homeo in q.iter_mut(&mut world) {
                homeo.energy = initial_animal_energy;
                homeo.energy_target = 100.0;
                homeo.hydration = 500.0;
                homeo.starving_ticks = 0;
            }
        }
        *world.resource_mut::<EcosystemBiomass>() = EcosystemBiomass {
            detritus: 0.0,
            plants: 0.0,
            animals: initial_animal_energy as f64,
            dormant: 0.0,
        };
    }

    adapter
}

fn agent_count(adapter: &LiveExperimentAdapter) -> usize {
    let mut world = adapter.world();
    let mut q = world.query_filtered::<Entity, With<Agent>>();
    q.iter(&world).count()
}

#[test]
fn one_starving_agent_transfers_its_energy_to_detritus_once_under_dual_mortality_schedule() {
    let initial_energy = 2.0_f32;
    let mut adapter = build_isolated_world(initial_energy);
    assert_eq!(agent_count(&adapter), 1, "expected exactly 1 founder agent");

    adapter.world().insert_resource(MortalityPolicy {
        enabled: true,
        starve_ticks: 1,
        dehydration_ticks: 1,
    });

    let initial_closed_eu = closed_total_eu(0.0, initial_energy as f64, 0.0, 0.0);
    let mut ticks_to_death = 0_u32;
    for tick in 1_u32..2_000_u32 {
        adapter.run_schedule_once();
        if agent_count(&adapter) == 0 {
            ticks_to_death = tick;
            break;
        }
    }

    assert_eq!(
        agent_count(&adapter),
        0,
        "founder agent must starve within 2000 ticks under dual mortality schedule"
    );

    let (standing, detritus, animals_pool, dormant, tracker_deaths, vitals_deaths) = {
        let world = adapter.world();
        let standing = world.resource::<ResourceField>().total_biomass();
        let pool = world.resource::<EcosystemBiomass>();
        let tracker_deaths = world
            .get_resource::<DemographicTracker>()
            .map(|t| t.deaths_starvation)
            .unwrap_or(0);
        let vitals_deaths = world
            .get_resource::<VitalStatistics>()
            .map(|v| v.total_deaths)
            .unwrap_or(0);
        (
            standing,
            pool.detritus,
            pool.animals,
            pool.dormant,
            tracker_deaths,
            vitals_deaths,
        )
    };

    let final_closed_eu = closed_total_eu(standing, animals_pool, detritus, dormant);
    let residual_eu = final_closed_eu - initial_closed_eu;
    println!(
        "HTC_STARVATION ticks_to_death={ticks_to_death} initial_closed_eu={initial_closed_eu:?} \
         standing={standing:?} detritus={detritus:?} animals_pool={animals_pool:?} dormant={dormant:?} \
         final_closed_eu={final_closed_eu:?} residual_eu={residual_eu:?} \
         tracker_deaths={tracker_deaths} vitals_deaths={vitals_deaths}"
    );

    assert_eq!(standing, 0.0);
    assert_eq!(animals_pool, 0.0);
    assert_eq!(dormant, 0.0);
    assert!(
        (detritus - initial_closed_eu).abs() < RESIDUAL_ABS_TOLERANCE_EU,
        "detritus {detritus:?} diverged from initial EU {initial_closed_eu:?}"
    );
    assert!(
        residual_eu.abs() < RESIDUAL_ABS_TOLERANCE_EU,
        "closed-EU residual {residual_eu:?} exceeded tolerance"
    );
}

/// Tests positive corpse reserves (`25.0` EU) across two queued reclaim commands and on the live
/// schedule: metabolic starvation reaches death with `0.0` energy (`0.0 * 2 == 0.0`), which would
/// mask a double-credit bug if reclaim and despawn were ever separated across systems/commands.
#[test]
fn dual_reclaim_commands_with_positive_corpse_reserve_credit_detritus_only_once() {
    // Part A: Two ReclaimAndDespawnAgentCommands queued at the same sync point for a 25.0 EU agent.
    let reserve_eu = 25.0_f32;
    let expected_eu = reserve_eu as f64;
    let adapter_a = build_isolated_world(reserve_eu);
    assert_eq!(agent_count(&adapter_a), 1);

    let detritus_after_dual_queue = {
        let mut world = adapter_a.world();
        let entity = {
            let mut q = world.query_filtered::<Entity, With<Agent>>();
            q.iter(&world).next().expect("founder entity exists")
        };
        let mut queue = CommandQueue::default();
        queue.push(ReclaimAndDespawnAgentCommand { root: entity });
        queue.push(ReclaimAndDespawnAgentCommand { root: entity });
        queue.apply(&mut world);
        world.resource::<EcosystemBiomass>().detritus
    };
    println!(
        "HTC_DUAL_QUEUE expected_eu={expected_eu:?} detritus_after_dual_queue={detritus_after_dual_queue:?}"
    );
    assert_eq!(agent_count(&adapter_a), 0);
    assert!(
        (detritus_after_dual_queue - expected_eu).abs() < RESIDUAL_ABS_TOLERANCE_EU,
        "dual queue reclaim credited {detritus_after_dual_queue:?} instead of {expected_eu:?}"
    );

    // Part B: Real merged tick schedule dehydration death while holding a 25.0 EU reserve.
    let mut adapter_b = build_isolated_world(reserve_eu);
    assert_eq!(agent_count(&adapter_b), 1);
    {
        let mut world = adapter_b.world();
        world.insert_resource(MortalityPolicy {
            enabled: true,
            starve_ticks: 2,
            dehydration_ticks: 2,
        });
        let mut q = world.query_filtered::<&mut HomeostaticState, With<Agent>>();
        for mut homeo in q.iter_mut(&mut world) {
            homeo.hydration = 0.0;
            homeo.starving_ticks = 2;
        }
    }
    adapter_b.run_schedule_once();
    assert_eq!(
        agent_count(&adapter_b),
        0,
        "dehydrated founder with 25.0 EU must despawn on this tick"
    );

    let (detritus_after_dehydration, animals_after_dehydration, vitals_deaths) = {
        let world = adapter_b.world();
        let pool = world.resource::<EcosystemBiomass>();
        let vitals_deaths = world
            .get_resource::<VitalStatistics>()
            .map(|v| v.total_deaths)
            .unwrap_or(0);
        (pool.detritus, pool.animals, vitals_deaths)
    };
    println!(
        "HTC_DEHYDRATION_RESERVE expected_eu={expected_eu:?} \
         detritus_after_dehydration={detritus_after_dehydration:?} \
         animals_after_dehydration={animals_after_dehydration:?} vitals_deaths={vitals_deaths}"
    );
    assert_eq!(animals_after_dehydration, 0.0);
    assert!(
        (detritus_after_dehydration - expected_eu).abs() < RESIDUAL_ABS_TOLERANCE_EU,
        "dehydration death credited {detritus_after_dehydration:?} instead of {expected_eu:?}"
    );
}
