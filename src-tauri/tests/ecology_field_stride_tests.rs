//! The staggered resource-field update.
//!
//! Raising the sim world to 256² quadrupled the cell count, and the regrowth path was costing four
//! full passes per tick. Striding cuts that to two passes over a quarter of the cells, and moves the
//! exact accounting inside the loop so the `total_biomass()` sandwich is no longer needed.
//!
//! That trades a re-summed `plants` figure for an incrementally carried one, which is the risk this
//! file exists to hold: a running total can drift away from the thing it describes without anything
//! failing, and the closed-energy ledger would then be conserving a number that no longer matches
//! the field.

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField, SeasonClock};
use anima_engine_lib::core::ecs::{Agent, Position, Prey};
use anima_engine_lib::core::environmental_systems::{
    herbivore_grazing_system, resource_field_regrowth_system,
};
use anima_engine_lib::core::terrain::BiomeType;
use bevy_ecs::prelude::*;
use glam::Vec3;

const W: usize = 32;
const H: usize = 32;
/// Tolerance on the closed total. Matches the order the G1.1 conservation gate holds itself to.
const TOLERANCE: f64 = 1e-3;

fn field() -> ResourceField {
    // A mixed field so cells differ in carrying capacity; a uniform one would hide an indexing
    // mistake that visits the wrong cells.
    let biomes: Vec<u8> = (0..W * H)
        .map(|i| match i % 4 {
            0 => BiomeType::Rainforest as u8,
            1 => BiomeType::Grassland as u8,
            2 => BiomeType::TemperateForest as u8,
            _ => BiomeType::Desert as u8,
        })
        .collect();
    let mut f = ResourceField::from_biomes(&biomes, W, H, -100.0, -100.0, 100.0, 100.0, 0.02);
    // `from_biomes` starts every cell AT its carrying capacity, so a fresh field cannot grow at all
    // — logistic growth at `r == r_max` is exactly zero. Every test here would have passed
    // vacuously on a static field. Half capacity leaves real headroom to observe.
    for cell in f.r.iter_mut() {
        *cell *= 0.5;
    }
    f
}

fn world_with_grazers(grazers: usize) -> World {
    let mut world = World::new();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(SeasonClock::default());

    let f = field();
    let plants = f.total_biomass();
    world.insert_resource(f);
    world.insert_resource(EcosystemBiomass {
        // A large detritus stock so regrowth is not gated to zero and the strided path is actually
        // exercised rather than short-circuiting on an empty budget.
        detritus: 5_000.0,
        plants,
        animals: 0.0,
    });

    for i in 0..grazers {
        let x = -80.0 + (i as f32) * 7.0;
        world.spawn((
            Agent,
            Prey,
            Position(Vec3::new(x, 0.0, (i % 5) as f32 * 9.0 - 20.0)),
            HomeostaticState {
                energy: 10.0,
                energy_target: 100.0,
                hydration: 50.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
        ));
    }
    world
}

fn schedule() -> Schedule {
    let mut s = Schedule::default();
    s.add_systems((
        resource_field_regrowth_system,
        herbivore_grazing_system.after(resource_field_regrowth_system),
    ));
    s
}

fn closed_total(world: &World) -> f64 {
    let pool = world.resource::<EcosystemBiomass>();
    let animals: f64 = 0.0; // set by the caller's own census; not exercised here
    pool.plants + pool.detritus + animals
}

#[test]
fn plants_tracks_the_field_it_describes() {
    // The whole risk of dropping the re-sum. `plants` is now carried incrementally: regrowth adds
    // what it grew, grazing subtracts what it took. If either side ever misreports, the running
    // total drifts away from the field and nothing else notices.
    let mut world = world_with_grazers(12);
    let mut sched = schedule();

    for tick in 1..=2_000 {
        sched.run(&mut world);
        if tick % 250 == 0 {
            let carried = world.resource::<EcosystemBiomass>().plants;
            let actual = world.resource::<ResourceField>().total_biomass();
            assert!(
                (carried - actual).abs() < TOLERANCE,
                "at tick {tick} the carried plant total ({carried}) had drifted from the field \
                 it describes ({actual})"
            );
        }
    }
}

#[test]
fn striding_keeps_the_ledger_closed() {
    // Plants and detritus must stay exactly complementary: what one gains the other loses. Grazing
    // moves energy into animal reserves, so those are held out by giving the grazers no appetite.
    let mut world = world_with_grazers(0);
    let mut sched = schedule();

    let before = closed_total(&world);
    let biomass_before = world.resource::<ResourceField>().total_biomass();
    for _ in 0..2_000 {
        sched.run(&mut world);
    }
    let after = closed_total(&world);
    let biomass_after = world.resource::<ResourceField>().total_biomass();

    assert!(
        (after - before).abs() < TOLERANCE,
        "closed total moved by {:e} under a strided field update",
        after - before
    );
    // Conservation over a field that never changed would be trivially true, so the run has to have
    // actually moved energy from detritus into plants.
    assert!(
        biomass_after > biomass_before * 1.01,
        "the field barely grew ({biomass_before} -> {biomass_after}); conservation proves nothing here"
    );
}

#[test]
fn a_full_sweep_visits_every_cell() {
    // Striding is only safe if the phase really does cycle. A phase that stuck would leave three
    // quarters of the world frozen — and the ledger would still balance, so conservation alone
    // would not catch it.
    let mut world = world_with_grazers(0);
    let mut sched = schedule();

    let start: Vec<f32> = world.resource::<ResourceField>().r.clone();
    for _ in 0..ResourceField::REGROWTH_STRIDE {
        sched.run(&mut world);
    }
    let end: Vec<f32> = world.resource::<ResourceField>().r.clone();

    let untouched = start
        .iter()
        .zip(&end)
        .filter(|(a, b)| (**a - **b).abs() < f32::EPSILON)
        .count();
    assert_eq!(
        untouched,
        0,
        "{untouched} of {} cells were never visited in a full sweep",
        start.len()
    );
}

#[test]
fn one_tick_visits_only_its_share() {
    // The other half of the same property: a single tick must NOT touch everything, or the stride
    // is not doing anything and the cost saving is imaginary.
    let mut world = world_with_grazers(0);
    let mut sched = schedule();

    let start: Vec<f32> = world.resource::<ResourceField>().r.clone();
    sched.run(&mut world);
    let end: Vec<f32> = world.resource::<ResourceField>().r.clone();

    let touched = start
        .iter()
        .zip(&end)
        .filter(|(a, b)| (**a - **b).abs() >= f32::EPSILON)
        .count();
    let expected = start.len() / ResourceField::REGROWTH_STRIDE;

    assert!(
        touched <= expected + 1,
        "one tick touched {touched} cells, more than the ~{expected} a stride of {} allows",
        ResourceField::REGROWTH_STRIDE
    );
    assert!(touched > 0, "one tick touched nothing at all");
}

#[test]
fn strided_growth_tracks_the_unstrided_result() {
    // A cell visited once per sweep with `stride * dt` should end up where it would have been if
    // visited every tick with `dt`. Logistic growth is slow enough that the first-order error is
    // far below the conservation tolerance — but "far below" is a claim worth measuring.
    let dt = 1.0f32 / 60.0;
    let stride = ResourceField::REGROWTH_STRIDE;
    let ticks = 1_200;

    let mut every_tick = field();
    for _ in 0..ticks {
        every_tick.step_regrowth_gated(dt, 1.0, 1e9);
    }

    let mut strided = field();
    for t in 0..ticks {
        strided.step_regrowth_gated_strided(dt * stride as f32, 1.0, 1e9, t % stride, stride);
    }

    let a = every_tick.total_biomass();
    let b = strided.total_biomass();
    let relative = (a - b).abs() / a.max(1.0);
    assert!(
        relative < 0.02,
        "strided field ended {relative:.4} away from the unstrided one ({b} vs {a})"
    );
}

#[test]
fn the_growth_figure_is_what_the_field_actually_gained() {
    // The exactness the `total_biomass()` sandwich used to provide, now measured in the loop. The
    // amount asked for and the amount stored differ because `cell += delta` rounds; if the returned
    // figure were the former, detritus would be debited for growth that never happened.
    let mut f = field();
    let before = f.total_biomass();
    let reported = f.step_regrowth_gated_strided(1.0 / 15.0, 1.0, 1e9, 0, 4);
    let after = f.total_biomass();

    assert!(reported > 0.0, "nothing grew, so nothing is being checked");
    assert!(
        (reported - (after - before)).abs() < 1e-9,
        "reported growth {reported} != actual field gain {}",
        after - before
    );
}

#[test]
fn a_starved_budget_grows_nothing_and_charges_nothing() {
    let mut f = field();
    let before = f.total_biomass();
    assert_eq!(
        f.step_regrowth_gated_strided(1.0 / 15.0, 1.0, 0.0, 0, 4),
        0.0
    );
    assert_eq!(f.total_biomass(), before);
}
