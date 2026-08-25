//! Mortality gates — feature `ecological-selection`, branch M.
//!
//! Gates (docs/ai/testing/2026-08-25-feature-ecological-selection.md §1):
//!
//! - **MO-S01** — deaths enabled, long run ⇒ residual EU ≤ `RESIDUAL_ABS_TOLERANCE_EU`.
//! - **MO-S02** — a death banks the corpse's reserve through the only sanctioned primitive,
//!   `ReclaimAndDespawnAgentCommand`; the closed total is bit-exact across the deaths.
//! - **MO-S03** — one meal ⇒ `starving_ticks` back to 0.
//! - **MO-S04** — `mortality_system` runs after `metabolic_decay_system` and before
//!   `ecosystem_census_system`, proven behaviourally on the real schedule.
//! - **MO-S05** — flag off ⇒ bit-identical trajectory (absent ≡ disabled ≡ enabled-no-deaths).
//! - **MO-S06** — a pre-feature snapshot loads with `starving_ticks == 0`; round trip keeps it.
//! - **CA-S02** — NPP = 0 collapses the population **only** with mortality enabled; the same
//!   drained world without the flag must not lose a single agent.
//!
//! Every test drives [`LiveExperimentAdapter`], which runs the one true schedule built by
//! `core::simulation_schedule::build_tick_schedule`. "NPP = 0" is expressed as a harness
//! intervention — standing crop zeroed at genesis, detritus drained every tick — applied
//! identically to both sides of the causal gate so that only the flag differs.

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::ecs::Agent;
use anima_engine_lib::core::energy_ledger::{EnergyLedger, RESIDUAL_ABS_TOLERANCE_EU};
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    live_observables, LiveExperimentAdapter, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS, LIVE_KEY_LAKES,
    LIVE_KEY_PREDATOR_FRACTION, LIVE_KEY_TREES,
};
use anima_engine_lib::core::resources::MortalityPolicy;
use anima_engine_lib::core::snapshot::world_checksum;
use bevy_ecs::prelude::*;

/// A seed belonging to no E2 manifest: these are properties of the mortality branch.
const SEED: u64 = 0x5EED;

fn deprived_initials(founders: f64) -> InitialConditionSet {
    InitialConditionSet::new(vec![
        (LIVE_KEY_FOUNDERS.to_string(), founders),
        (LIVE_KEY_PREDATOR_FRACTION.to_string(), 0.0),
        (LIVE_KEY_TREES.to_string(), 0.0),
        (LIVE_KEY_LAKES.to_string(), 0.0),
        (LIVE_KEY_FOOD_CAP.to_string(), 0.0),
    ])
}

fn healthy_initials() -> InitialConditionSet {
    InitialConditionSet::new(vec![
        (LIVE_KEY_FOUNDERS.to_string(), 10.0),
        (LIVE_KEY_PREDATOR_FRACTION.to_string(), 0.3),
        (LIVE_KEY_TREES.to_string(), 8.0),
        ("live.lakes".to_string(), 2.0),
        (LIVE_KEY_FOOD_CAP.to_string(), 50.0),
    ])
}

fn adapter(initials: &InitialConditionSet) -> LiveExperimentAdapter {
    LiveExperimentAdapter::from_manifest(&WorldLawSet::baseline(), initials, &[], SEED, (16, 16), 0)
        .expect("the live world builds")
}

fn agent_count(a: &LiveExperimentAdapter) -> usize {
    let mut world = a.world();
    let mut q = world.query_filtered::<Entity, With<Agent>>();
    q.iter(&world).count()
}

/// Zero the standing crop once at genesis: from here on there is no primary production unless
/// detritus regrows it, and the harness drains detritus every tick.
fn kill_primary_production(a: &mut LiveExperimentAdapter) {
    let mut world = a.world();
    let mut field = world.resource_mut::<ResourceField>();
    for r in field.r.iter_mut() {
        *r = 0.0;
    }
}

fn drain_detritus(a: &mut LiveExperimentAdapter) {
    let mut world = a.world();
    world.resource_mut::<EcosystemBiomass>().detritus = 0.0;
}

/// Give every agent the same reserves in every compared world, so treatment and negative
/// control differ only in the flag under test.
fn endow_all_agents(a: &mut LiveExperimentAdapter, energy: f32, hydration: f32) {
    let mut world = a.world();
    let mut q = world.query_filtered::<&mut HomeostaticState, With<Agent>>();
    for mut homeo in q.iter_mut(&mut world) {
        homeo.energy = energy;
        homeo.hydration = hydration;
        homeo.starving_ticks = 0;
    }
}

fn set_policy(a: &mut LiveExperimentAdapter, policy: MortalityPolicy) {
    a.world().insert_resource(policy);
}

fn homeo_of_first_agent(a: &mut LiveExperimentAdapter) -> HomeostaticState {
    let mut world = a.world();
    let mut q = world.query_filtered::<&HomeostaticState, With<Agent>>();
    q.iter(&world).next().cloned().expect("an agent")
}

fn set_every_agent_state(a: &mut LiveExperimentAdapter, energy: f32, starving: u32) {
    let mut world = a.world();
    let mut q = world.query_filtered::<&mut HomeostaticState, With<Agent>>();
    for mut homeo in q.iter_mut(&mut world) {
        homeo.energy = energy;
        homeo.starving_ticks = starving;
    }
}

/// EU-neutral death sentence: hydration is not a closed-EU store, so zeroing it (with the
/// counter pre-advanced past the policy threshold) moves no energy — the deaths themselves then
/// route each agent's remaining reserve through `ReclaimAndDespawnAgentCommand`.
fn dehydrate_everyone(a: &mut LiveExperimentAdapter, threshold: u32) {
    let mut world = a.world();
    let mut q = world.query_filtered::<&mut HomeostaticState, With<Agent>>();
    for mut homeo in q.iter_mut(&mut world) {
        homeo.hydration = 0.0;
        homeo.starving_ticks = threshold;
    }
}

// ---------------------------------------------------------------------------------------
// MO-S03 — the counter itself
// ---------------------------------------------------------------------------------------

#[test]
fn a_starving_tick_is_counted_and_one_meal_resets_it() {
    let mut a = adapter(&healthy_initials());
    set_policy(&mut a, MortalityPolicy::disabled());

    // A well-fed tick leaves the counter at zero.
    endow_all_agents(&mut a, 500.0, 500.0);
    a.run_schedule_once();
    assert_eq!(homeo_of_first_agent(&mut a).starving_ticks, 0);

    // Bottom of the barrel: metabolism floors energy to zero ⇒ exactly one starving tick.
    set_every_agent_state(&mut a, 0.0, 0);
    a.run_schedule_once();
    assert_eq!(
        homeo_of_first_agent(&mut a).starving_ticks,
        1,
        "one tick at the bottom must count exactly one starving tick"
    );

    // One meal later the animal is out of the bottom: the reset is unconditional.
    endow_all_agents(&mut a, 500.0, 500.0);
    a.run_schedule_once();
    assert_eq!(
        homeo_of_first_agent(&mut a).starving_ticks,
        0,
        "MO-S03: a single meal must reset the starving counter"
    );
}

// ---------------------------------------------------------------------------------------
// MO-S04 — placement in the real schedule, proven behaviourally
// ---------------------------------------------------------------------------------------

/// The agent enters the tick with a sliver of energy and the counter one below the threshold.
/// Only `metabolic_decay_system` can floor that sliver and push the counter over the threshold;
/// a `mortality_system` running *before* decay would see `energy > 0`, counter below threshold,
/// and let the agent survive this tick. Dying **this** tick proves the declared order.
#[test]
fn a_death_caused_by_this_ticks_metabolism_lands_in_this_tick() {
    let mut a = adapter(&deprived_initials(6.0));
    set_policy(
        &mut a,
        MortalityPolicy {
            enabled: true,
            starve_ticks: 3,
            dehydration_ticks: 3,
        },
    );
    // Healthy warm-up tick so genesis state cannot leak into the assertion. The field is
    // zeroed first: otherwise the very next tick's grazing would refill the sliver and the
    // animal would legitimately survive.
    kill_primary_production(&mut a);
    endow_all_agents(&mut a, 500.0, 500.0);
    a.run_schedule_once();

    let before = agent_count(&a);
    set_every_agent_state(&mut a, 1e-4, 2);
    a.run_schedule_once();

    assert!(
        agent_count(&a) < before,
        "MO-S04: an agent whose energy was floored to zero by this tick's metabolic decay must \
         be reclaimed in the same tick — only possible if mortality runs after decay"
    );
}

/// The census is the last word on reserves: after a death tick, `pool.animals` must equal the
/// surviving reserves exactly. If the census ran before the reclaim applied, the corpses would
/// still hold their reserves and the snapshot would describe a world that no longer exists.
#[test]
fn the_census_counts_the_world_after_the_corpse_is_reclaimed() {
    let mut a = adapter(&deprived_initials(6.0));
    set_policy(
        &mut a,
        MortalityPolicy {
            enabled: true,
            starve_ticks: 2,
            dehydration_ticks: 2,
        },
    );
    endow_all_agents(&mut a, 7.5, 0.0);
    set_every_agent_state(&mut a, 7.5, 2);
    a.run_schedule_once();

    assert_eq!(
        agent_count(&a),
        0,
        "dehydrated agents holding 7.5 EU each must die on this tick"
    );
    let world = a.world();
    let animals = world.resource::<EcosystemBiomass>().animals;
    assert_eq!(
        animals.to_bits(),
        0f64.to_bits(),
        "MO-S04: the census must count the post-reclaim world — banked reserves belong to \
         detritus now, not to `animals`"
    );
}

// ---------------------------------------------------------------------------------------
// MO-S02 / MO-S01 — the closed ledger across deaths
// ---------------------------------------------------------------------------------------

#[test]
fn deaths_never_move_the_closed_total() {
    const DEHYDRATION_TICKS: u32 = 45;
    let mut a = adapter(&deprived_initials(8.0));
    set_policy(
        &mut a,
        MortalityPolicy {
            enabled: true,
            starve_ticks: DEHYDRATION_TICKS,
            dehydration_ticks: DEHYDRATION_TICKS,
        },
    );
    a.run_schedule_once(); // the first census locks the ledger baseline
    let baseline = a
        .world()
        .resource::<EnergyLedger>()
        .baseline()
        .expect("the census must lock a baseline on its first run");

    // Death waves without touching a single EU: every 800 ticks the harness zeroes *hydration*
    // (not an EU store) and pre-advances the counter, so every agent dies through the one
    // sanctioned primitive — which banks its remaining reserve into detritus exactly. Grazing,
    // regrowth, epoch replacement and the save-free run all keep moving real EU the whole time;
    // the ledger must close over all of it.
    let mut deaths_observed = false;
    let mut last_count = agent_count(&a);
    for tick in 0..gate_ticks() {
        if tick % 800 == 400 {
            dehydrate_everyone(&mut a, DEHYDRATION_TICKS);
        }
        a.run_schedule_once();
        let c = agent_count(&a);
        if c < last_count {
            deaths_observed = true;
        }
        last_count = c;
    }
    assert!(
        deaths_observed,
        "the gate run must contain at least one death"
    );

    // Authoritative stores, exactly as ENERGY_LEDGER_CONTRACT.md §1 defines them.
    let (standing, detritus, dormant) = {
        let world = a.world();
        (
            world.resource::<ResourceField>().total_biomass(),
            world.resource::<EcosystemBiomass>().detritus,
            world.resource::<EcosystemBiomass>().dormant,
        )
    };
    let reserves = {
        let mut world = a.world();
        let mut q = world.query_filtered::<&HomeostaticState, With<Agent>>();
        let mut t = 0.0f64;
        for h in q.iter(&world) {
            t += h.energy as f64;
        }
        t
    };
    let residual = standing + reserves + detritus + dormant - baseline;
    assert!(
        residual.abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "MO-S01: residual {residual} EU exceeds the tolerance across a run with deaths \
         (baseline {baseline}, measured {})",
        standing + reserves + detritus + dormant
    );
}

/// Mirrors `gate_ticks()` of `energy_conservation_tests.rs`: the contract run is 120,000 ticks,
/// overridable for local iteration without touching the default.
fn gate_ticks() -> u32 {
    std::env::var("ANIMA_MORTALITY_GATE_TICKS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(120_000)
}

// ---------------------------------------------------------------------------------------
// MO-S05 — flags off ⇒ today, bit for bit
// ---------------------------------------------------------------------------------------

#[test]
fn with_the_flag_off_three_ways_of_not_dying_give_identical_worlds() {
    let run = |policy: Option<MortalityPolicy>| {
        let mut a = adapter(&healthy_initials());
        if let Some(p) = policy {
            set_policy(&mut a, p);
        }
        // Reserves no six-hundred-tick run can exhaust, so the enabled world loses nobody and
        // the only possible difference is the flag machinery itself.
        endow_all_agents(&mut a, 10_000.0, 10_000.0);
        for _ in 0..600u32 {
            a.run_schedule_once();
        }
        let checksum = {
            let mut world = a.world();
            world_checksum(&mut world)
        };
        let obs = {
            let mut world = a.world();
            live_observables(&mut world)
        };
        (checksum, obs)
    };

    let absent = run(None);
    let disabled = run(Some(MortalityPolicy::disabled()));
    let enabled_healthy = run(Some(MortalityPolicy {
        enabled: true,
        starve_ticks: 120,
        dehydration_ticks: 120,
    }));

    assert_eq!(
        absent.0, disabled.0,
        "MO-S05: a disabled policy must be indistinguishable from a missing one"
    );
    assert_eq!(
        absent.0, enabled_healthy.0,
        "MO-S05: an enabled policy under which nothing starves must not move the checksum"
    );
    assert_eq!(absent.1, disabled.1, "observables must agree bit for bit");
    assert_eq!(absent.1, enabled_healthy.1);
}

// ---------------------------------------------------------------------------------------
// MO-S06 — snapshots from before the field existed
// ---------------------------------------------------------------------------------------

#[test]
fn a_pre_feature_snapshot_loads_with_the_counter_at_zero() {
    let legacy: HomeostaticState = serde_json::from_str(
        r#"{
            "energy": 50.0,
            "energy_target": 100.0,
            "hydration": 50.0,
            "hydration_target": 100.0,
            "temperature": 37.0,
            "temp_target": 37.0,
            "previous_deviation": 0.0
        }"#,
    )
    .expect("a snapshot written before starving_ticks existed must load");
    assert_eq!(
        legacy.starving_ticks, 0,
        "MO-S06: serde(default) means 'not starving'"
    );

    let mut round_trip = legacy;
    round_trip.starving_ticks = 42;
    let text = serde_json::to_string(&round_trip).expect("serialize");
    let restored: HomeostaticState = serde_json::from_str(&text).expect("deserialize");
    assert_eq!(
        restored.starving_ticks, 42,
        "a round trip must keep the counter"
    );
}

// ---------------------------------------------------------------------------------------
// CA-S02 — the causal channel, with its mandatory negative control
// ---------------------------------------------------------------------------------------

#[test]
fn npp_zero_collapses_the_population_only_when_mortality_is_enabled() {
    let population_minimum = |policy: Option<MortalityPolicy>| {
        let mut a = adapter(&deprived_initials(8.0));
        if let Some(p) = policy {
            set_policy(&mut a, p);
        }
        // Small enough reserves that the floored-to-zero state is reached well inside the
        // window even at Kleiber-scale metabolic rates; identical in both worlds.
        endow_all_agents(&mut a, 2.0, 2.0);
        kill_primary_production(&mut a);
        let mut minimum = usize::MAX;
        for _ in 0..1200u32 {
            drain_detritus(&mut a);
            a.run_schedule_once();
            minimum = minimum.min(agent_count(&a));
        }
        minimum
    };

    let with_mortality = population_minimum(Some(MortalityPolicy {
        enabled: true,
        starve_ticks: 90,
        dehydration_ticks: 90,
    }));
    let without_mortality = population_minimum(None);

    assert_eq!(
        with_mortality, 0,
        "CA-S02: with NPP = 0 and mortality enabled the population must collapse to 0"
    );
    assert_eq!(
        without_mortality, 8,
        "CA-S02 negative control: the same drained world WITHOUT the mortality flag must not \
         lose a single agent"
    );
}

// ---------------------------------------------------------------------------------------
// M-7 — death is a DECLARED input, not an ambient one
// ---------------------------------------------------------------------------------------

/// The manifest path must reach the same kill as the direct-insertion path: `live.mortality` in
/// the initial conditions turns the flag on inside `build_live_world`, with no test-side
/// `insert_resource` and no environment variable involved.
#[test]
fn mortality_is_expressible_through_the_manifest_and_absent_means_off() {
    let mut declared = vec![
        (LIVE_KEY_FOUNDERS.to_string(), 6.0),
        (LIVE_KEY_PREDATOR_FRACTION.to_string(), 0.0),
        (LIVE_KEY_TREES.to_string(), 0.0),
        (LIVE_KEY_LAKES.to_string(), 0.0),
        (LIVE_KEY_FOOD_CAP.to_string(), 0.0),
        ("live.mortality".to_string(), 1.0),
        ("live.starve_ticks".to_string(), 2.0),
        ("live.dehydration_ticks".to_string(), 2.0),
    ];
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &InitialConditionSet::new(declared.clone()),
        &[],
        SEED,
        (16, 16),
        0,
    )
    .expect("the live world builds");
    endow_all_agents(&mut a, 7.5, 0.0);
    set_every_agent_state(&mut a, 7.5, 2);
    a.run_schedule_once();
    assert_eq!(
        agent_count(&a),
        0,
        "a manifest declaring live.mortality = 1.0 must be able to kill dehydrated agents"
    );

    // Negative control over the *declaration* itself: drop only the keys, keep everything else.
    declared.truncate(5);
    let mut b = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &InitialConditionSet::new(declared),
        &[],
        SEED,
        (16, 16),
        0,
    )
    .expect("the live world builds");
    endow_all_agents(&mut b, 7.5, 0.0);
    set_every_agent_state(&mut b, 7.5, 2);
    b.run_schedule_once();
    assert_eq!(
        agent_count(&b),
        6,
        "without the declared keys nobody dies — absent means off, bit for bit"
    );
}

