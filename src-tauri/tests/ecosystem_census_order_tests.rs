//! The census must describe the world it is part of.
//!
//! `ecosystem_census_system` does not accumulate — it takes a **snapshot**:
//!
//! ```ignore
//! pool.animals = total;   // total = Σ agent reserves, recomputed from scratch each tick
//! ```
//!
//! `plants` and `detritus` are carried incrementally (`pool.plants -= grazed`), so *when* in the
//! tick their writers run cannot change the value at the tick boundary. A snapshot is different:
//! its value is entirely a function of when it is taken. If the census runs before
//! `metabolic_decay_system`, `pool.animals` reports the reserves the agents had *last* tick, and
//! `live.animals_eu` — and `live.closed_eu_total`, which is derived from it — is one tick of
//! metabolism away from the world it claims to describe.
//!
//! # Why this is a determinism bug and not merely an off-by-one
//!
//! The census declared `.after(resource_field_regrowth_system)` and
//! `.after(rehydrate_wakeable_chunks_system)` and **nothing** about the systems that mutate agent
//! reserves. Two systems with no ordering edge between them are ordered by the schedule's
//! topological sort, and that sort is not a declared property of the schedule — so the order was
//! chosen per process. Measured on the E2 smoke seed at commit `0bcb330`, twelve independent
//! processes running one binary at one seed produced **three** outcomes:
//!
//! | processes | world checksum | `live.animals_eu` at tick 600 |
//! |--:|---|---|
//! | 8 | `784036196` | `920.1691818237305` |
//! | 3 | `784036196` | `920.3547668457031` ← same world, different census |
//! | 1 | `3406435134` | `920.1710510253906` ← a *different* world |
//!
//! The middle row is this bug: an identical world reported through a census taken 0.186 EU earlier,
//! which is one tick of metabolic decay across ten agents. Adding the ordering edges removes it —
//! twelve processes then give only the first and third rows (11 and 1).
//!
//! The third row is a **separate, pre-existing** ambiguity somewhere else in the schedule that moves
//! the trajectory itself, at roughly one process in twelve, and it occurs at the same rate with and
//! without this fix. It is recorded as its own finding; it is not addressed here, and it is why
//! `tests/live_cross_process_probe.rs` exists.
//!
//! Neither was visible to `the_same_seed_and_manifest_give_the_same_live_checksum`: that gate
//! compares two runs *inside one process*, which is exactly the comparison a per-process ordering
//! cannot fail. This one was doubly invisible, because `world_checksum` covers agent reserves and
//! the resource field but **not** `EcosystemBiomass`.
//!
//! That is the shape CLAUDE.md's G1.3 rule exists to prevent: execution order must be declared, not
//! incidental. It is also the shape ADR-0003's history keeps finding — code that runs, returns
//! finite numbers, and is silently wrong — and it was invisible to
//! `the_same_seed_and_manifest_give_the_same_live_checksum`, because `world_checksum` covers agent
//! reserves and the resource field but **not** `EcosystemBiomass`.
//!
//! # What this test asserts
//!
//! One invariant, bit-exact: at a tick boundary, the animal compartment equals the reserves of the
//! animals. It is not a statement about the order of any particular pair of systems, so it keeps
//! holding when a new system that moves energy is added — and fails the moment one is added after
//! the census.

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecology::EcosystemBiomass;
use anima_engine_lib::core::ecs::Agent;
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{LiveExperimentAdapter, LIVE_KEY_EVOLVED_BRAINS};
use bevy_ecs::prelude::*;

/// A seed belonging to no E2 manifest: this is a property of the schedule, not of an experiment.
const SEED: u64 = 8_675_309;

fn initial(evolved: bool) -> InitialConditionSet {
    let mut values = vec![
        ("live.founders".to_string(), 10.0),
        ("live.predator_fraction".to_string(), 0.3),
        ("live.trees".to_string(), 8.0),
        ("live.lakes".to_string(), 2.0),
        ("live.food_cap".to_string(), 50.0),
    ];
    if evolved {
        values.push((LIVE_KEY_EVOLVED_BRAINS.to_string(), 1.0));
    }
    InitialConditionSet::new(values)
}

fn adapter(evolved: bool) -> LiveExperimentAdapter {
    LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial(evolved),
        &[],
        SEED,
        (16, 16),
        0,
    )
    .expect("the live world builds")
}

/// `(pool.animals, Σ agent reserves)` at this instant, both as `f64`, computed exactly the way the
/// census computes its own sum so a mismatch can only be a *timing* difference and never a
/// different formula.
fn pool_and_reserves(a: &LiveExperimentAdapter) -> (f64, f64) {
    let mut world = a.world();
    let pool = world
        .get_resource::<EcosystemBiomass>()
        .expect("a live world always has the biomass pool")
        .animals;
    let world = &mut *world;
    let mut q = world.query_filtered::<&HomeostaticState, With<Agent>>();
    let mut total = 0.0f64;
    for homeo in q.iter(world) {
        total += homeo.energy.max(0.0) as f64;
    }
    (pool, total)
}

#[test]
fn the_census_agrees_with_the_reserves_it_counted() {
    for evolved in [false, true] {
        let mut a = adapter(evolved);

        // Genesis, before any tick: the pool is whatever the world was built with, and the census
        // has not run. Checked from tick 1 onward instead, where the invariant is meaningful.
        for tick in 1..=180u32 {
            a.run_schedule_once();
            let (pool, reserves) = pool_and_reserves(&a);
            assert_eq!(
                pool.to_bits(),
                reserves.to_bits(),
                "evolved={evolved}, tick {tick}: the animal compartment holds {pool} EU while the \
                 animals hold {reserves} EU — a difference of {} EU. The census took its snapshot \
                 at a different point in the tick from the one the world ended at, which means \
                 `live.animals_eu` and `live.closed_eu_total` describe a world that no longer \
                 exists.",
                pool - reserves
            );
        }
    }
}

#[test]
fn the_census_is_the_last_word_on_reserves_across_a_long_run() {
    // The short test above would pass on a schedule that merely happens to end a tick with the
    // census. This one runs long enough for feeding, combat and starvation to all have occurred, so
    // every energy-moving path in the tick has been exercised before the invariant is checked.
    let mut a = adapter(true);
    for _ in 0..1_200 {
        a.run_schedule_once();
    }
    let (pool, reserves) = pool_and_reserves(&a);
    assert_eq!(
        pool.to_bits(),
        reserves.to_bits(),
        "after 1,200 ticks the animal compartment is {pool} EU and the animals hold {reserves} EU"
    );
}
