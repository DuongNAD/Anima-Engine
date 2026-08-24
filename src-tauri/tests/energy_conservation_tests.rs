//! G1.1 conservation gate: a live world must not create or destroy EU.
//!
//! The existing conservation tests prove that individual transaction *functions* balance. They do
//! not prove that a whole live run balances, which is the claim `SIMULATION_RULES.md` actually
//! makes. This file runs the live energy schedule for millions of ticks with births, deaths,
//! feeding, grazing, predation, fruiting and a save/load cycle, and asserts the closed-EU residual
//! stays inside [`RESIDUAL_ABS_TOLERANCE_EU`].
//!
//! The tolerance is declared in `core::energy_ledger` and justified there: it is a bound on
//! `f32`→`f64` census widening, not an empirical fudge factor. If this test fails, energy is moving
//! somewhere that does not go through `EnergyLedger`; raising the tolerance would hide that.
//!
//! Tick count is overridable with `ANIMA_ENERGY_GATE_TICKS` so the full multi-million-tick run can
//! be reproduced on demand without making every `cargo test` pay for it.

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::apply_staggered_evolution_system;
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::ecs::{
    init_world, Agent, EpochManager, Food, MapBounds, Position, Predator, Prey, Tree,
};
use anima_engine_lib::core::energy_ledger::{
    closed_total_eu, EnergyLedger, RESIDUAL_ABS_TOLERANCE_EU,
};
use anima_engine_lib::core::environmental_systems::{
    detect_environmental_collisions_system, ecosystem_census_system, fruit_growth_system,
    herbivore_grazing_system, resource_field_regrowth_system,
};
use anima_engine_lib::core::resources::{
    EnvironmentalSpawnSettings, EvolutionQueue, EvolutionReceiver, FoodSpawnSettings,
};
use anima_engine_lib::core::world_systems::{
    combat_system, detect_food_collisions_system, metabolic_decay_system, spawn_food_system,
};
use anima_engine_lib::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;

/// Default tick count. The gate's headline run is millions of ticks; this default keeps a plain
/// `cargo test` to a few seconds while still exercising thousands of feeding, grazing, predation
/// and replacement transactions. `ANIMA_ENERGY_GATE_TICKS` overrides it.
const DEFAULT_TICKS: u64 = 120_000;

/// How often an agent is killed and replaced, so the run has births and deaths rather than a
/// static population.
const REPLACE_EVERY: u64 = 500;

fn gate_ticks() -> u64 {
    std::env::var("ANIMA_ENERGY_GATE_TICKS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_TICKS)
}

fn test_genotype() -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    g.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.3,
        mass: 1.0,
    });
    g.add_node(MorphologyNode {
        id: 1,
        length: 1.0,
        radius: 0.3,
        mass: 1.0,
    });
    g.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 1,
        joint_anchor: glam::Vec3::new(1.0, 0.0, 0.0),
        joint_axis: glam::Vec3::new(0.0, 0.0, 1.0),
    });
    g
}

/// The world's closed EU, summed from the three **authoritative** stores: the resource field's
/// cells, every living agent's reserve, and the detritus pool. Deliberately not read off the
/// `EcosystemBiomass` mirrors, so a bug that desynchronises a mirror from its store is visible
/// here instead of being hidden by it.
fn closed_total(world: &mut World) -> f64 {
    let plants = world
        .get_resource::<ResourceField>()
        .map(|f| f.total_biomass())
        .unwrap_or(0.0);
    let detritus = world
        .get_resource::<EcosystemBiomass>()
        .map(|p| p.detritus)
        .unwrap_or(0.0);
    let mut animals = 0.0f64;
    let mut q = world.query_filtered::<&HomeostaticState, With<Agent>>();
    for homeo in q.iter(world) {
        animals += homeo.energy.max(0.0) as f64;
    }
    closed_total_eu(plants, animals, detritus)
}

fn living_agents(world: &mut World) -> Vec<Entity> {
    let mut q = world.query_filtered::<Entity, With<Agent>>();
    q.iter(world).collect()
}

/// Build a live world: real terrain, real resource field, real closed ledger, a founding
/// population of prey and predators, and trees to fruit.
fn build_live_world() -> (World, crossbeam_channel::Sender<EvolutionMsg>) {
    let mut world = init_world();

    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(FoodSpawnSettings::default());
    world.insert_resource(EnvironmentalSpawnSettings::default());
    world.insert_resource(EpochManager::default());

    let (tx, rx) = crossbeam_channel::unbounded::<EvolutionMsg>();
    world.insert_resource(EvolutionReceiver(rx));
    world.insert_resource(EvolutionQueue {
        pending_replacements: Vec::new(),
    });

    let genotype = test_genotype();
    let bounds = *world.resource::<MapBounds>();
    for i in 0..10 {
        let pos = glam::Vec3::new(
            bounds.min.x + (i as f32 + 1.0) * 3.0,
            0.0,
            bounds.min.z + 10.0,
        );
        let e = decode_genotype(&mut world, &genotype, pos, glam::Quat::IDENTITY);
        if i < 7 {
            world.entity_mut(e).insert(Prey);
        } else {
            world.entity_mut(e).insert(Predator);
        }
    }

    // Trees so `fruit_growth_system` and the fruit-eating branch actually fire.
    for i in 0..8 {
        world.spawn((
            Tree {
                current_fruit: 5.0,
                max_fruit: 50.0,
                fruit_growth_rate: 2.0,
                time_since_last_drop: 0.0,
                seed_drop_cooldown: 1.0e9, // no seeding: tree count is not what this gate measures
                seed_spread_radius: 5.0,
            },
            Position(glam::Vec3::new(
                bounds.min.x + (i as f32 + 1.0) * 3.0,
                0.0,
                bounds.min.z + 10.0,
            )),
            anima_engine_lib::physics::SpatialCollider { radius: 4.0 },
        ));
    }

    (world, tx)
}

type EvolutionMsg = (
    Entity,
    MorphologyGenotype,
    glam::Vec3,
    String,
    u32,
    Vec<String>,
);

fn energy_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    // Every live system that moves EU, in the same order the engine runs them: grazing and
    // regrowth before the census, so the census sees all three stores current.
    schedule.add_systems((
        metabolic_decay_system,
        spawn_food_system,
        detect_food_collisions_system.after(spawn_food_system),
        combat_system,
        fruit_growth_system,
        detect_environmental_collisions_system.after(fruit_growth_system),
        apply_staggered_evolution_system,
        herbivore_grazing_system,
        resource_field_regrowth_system.after(herbivore_grazing_system),
        ecosystem_census_system.after(resource_field_regrowth_system),
    ));
    schedule
}

#[test]
fn live_world_conserves_energy_across_births_deaths_and_a_save_load_cycle() {
    let ticks = gate_ticks();
    let (mut world, _tx) = build_live_world();
    let mut schedule = energy_schedule();

    // One tick so the census locks the baseline (D06: the baseline is measured after genesis has
    // initialised plants, animals and detritus).
    schedule.run(&mut world);
    let baseline = world
        .resource::<EnergyLedger>()
        .baseline()
        .expect("the census must lock a baseline on its first run");
    let measured = closed_total(&mut world);
    assert!(
        (baseline - measured).abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "the locked baseline {baseline} must equal the authoritative store sum {measured}"
    );

    let genotype = test_genotype();
    let save_at = ticks / 2;
    let mut replacements = 0u64;
    let mut total_before_save = 0.0f64;
    let mut total_after_load = 0.0f64;
    let mut worst_residual = 0.0f64;

    for tick in 1..=ticks {
        // Births and deaths: kill an agent and queue its replacement. The replacement is funded
        // from detritus, which is exactly where the dead agent's reserve just went (D06).
        if tick % REPLACE_EVERY == 0 {
            let agents = living_agents(&mut world);
            if !agents.is_empty() {
                let victim = agents[(tick as usize / REPLACE_EVERY as usize) % agents.len()];
                let pos = world
                    .get::<Position>(victim)
                    .map(|p| p.0)
                    .unwrap_or(glam::Vec3::ZERO);
                world
                    .resource_mut::<EvolutionQueue>()
                    .pending_replacements
                    .push((
                        victim,
                        genotype.clone(),
                        pos,
                        format!("lineage-{tick}"),
                        1,
                        Vec::new(),
                    ));
                replacements += 1;
            }
        }

        schedule.run(&mut world);

        let residual = world.resource::<EnergyLedger>().last_residual();
        if residual.abs() > worst_residual.abs() {
            worst_residual = residual;
        }

        // The save/load cycle the gate requires. `restore_energy_state` is the real restore path
        // the engine uses; a save that dropped the energy compartments would show up right here as
        // a jump in the closed total.
        if tick == save_at {
            total_before_save = closed_total(&mut world);
            let saved = snapshot_energy(&mut world);

            let mut reloaded = build_live_world().0;
            // A freshly built world has its own genesis energy; restoring must overwrite it with
            // the saved state rather than add to it.
            anima_engine_lib::core::simulation_state::restore_energy_state(&mut reloaded, &saved);
            // Match the saved population's reserves so `animals` is comparable. The gate here is
            // about the pool and the standing crop, which are what a save used to drop entirely.
            copy_agent_reserves(&mut world, &mut reloaded);
            total_after_load = closed_total(&mut reloaded);
        }
    }

    let final_total = closed_total(&mut world);
    let residual = final_total - baseline;
    let ledger = *world.resource::<EnergyLedger>();

    println!(
        "ticks={ticks} replacements={replacements} baseline={baseline:.6} final={final_total:.6}\n\
         residual={residual:.9} worst_seen={worst_residual:.9} tolerance={RESIDUAL_ABS_TOLERANCE_EU}\n\
         ledger: granted={:.3} refused={:.3} settled={}",
        ledger.granted(),
        ledger.refused(),
        ledger.settled_count(),
    );

    assert!(
        replacements > 0,
        "the run must actually contain births and deaths"
    );
    assert!(
        ledger.settled_count() > 0,
        "the run must actually move energy through the ledger"
    );
    assert!(
        (total_before_save - total_after_load).abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "save/load moved energy: {total_before_save} before, {total_after_load} after"
    );
    assert!(
        residual.abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "closed EU drifted by {residual} over {ticks} ticks (tolerance {RESIDUAL_ABS_TOLERANCE_EU}). \
         Energy is moving outside EnergyLedger — find that site rather than raising the tolerance."
    );
    assert!(
        worst_residual.abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "closed EU drifted to {worst_residual} at some point mid-run even though it ended at {residual}"
    );
}

/// Build the energy half of a `SavedSimulationState` the way `serialize_world_state` does.
fn snapshot_energy(
    world: &mut World,
) -> anima_engine_lib::core::simulation_state::SavedSimulationState {
    let pool = *world.resource::<EcosystemBiomass>();
    let field_r = world.resource::<ResourceField>().r.clone();
    let mut state = empty_saved_state();
    state.eco_detritus = pool.detritus;
    state.eco_plants = pool.plants;
    state.eco_animals = pool.animals;
    state.resource_field_r = field_r;
    state
}

fn copy_agent_reserves(from: &mut World, to: &mut World) {
    let mut src = from.query_filtered::<&HomeostaticState, With<Agent>>();
    let reserves: Vec<HomeostaticState> = src.iter(from).cloned().collect();
    let targets = living_agents(to);
    for (entity, saved) in targets.iter().zip(reserves.iter()) {
        if let Some(mut homeo) = to.get_mut::<HomeostaticState>(*entity) {
            *homeo = saved.clone();
        }
    }
    // Any agent the save did not cover must not keep a reserve the save never recorded.
    for entity in targets.iter().skip(reserves.len()) {
        if let Some(mut homeo) = to.get_mut::<HomeostaticState>(*entity) {
            homeo.energy = 0.0;
        }
    }
}

fn empty_saved_state() -> anima_engine_lib::core::simulation_state::SavedSimulationState {
    anima_engine_lib::core::simulation_state::empty_saved_state_for_tests()
}

/// Food entities are markers, not stores: eating one must move EU out of detritus, never mint it.
/// This is the specific leak the audit recorded at `world_systems.rs:150,206`.
#[test]
fn eating_food_moves_energy_out_of_detritus_instead_of_minting_it() {
    let (mut world, _tx) = build_live_world();
    let mut schedule = energy_schedule();
    schedule.run(&mut world);

    // Give the pool something to fund with, then park food on top of every agent.
    world.resource_mut::<EcosystemBiomass>().detritus = 500.0;
    let before = closed_total(&mut world);
    let detritus_before = world.resource::<EcosystemBiomass>().detritus;

    let positions: Vec<glam::Vec3> = {
        let mut q = world.query_filtered::<&Position, With<Agent>>();
        q.iter(&world).map(|p| p.0).collect()
    };
    for pos in &positions {
        world.spawn((
            Food {
                energy_value: 20.0,
                hydration_value: 0.0,
            },
            Position(*pos),
            anima_engine_lib::physics::SpatialCollider { radius: 0.5 },
        ));
    }
    // Make room in the reserves so the grants are not all refused at the cap.
    {
        let mut q = world.query_filtered::<&mut HomeostaticState, With<Agent>>();
        for mut homeo in q.iter_mut(&mut world) {
            homeo.energy = 10.0;
        }
    }

    let before_eat = closed_total(&mut world);
    schedule.run(&mut world);
    let after_eat = closed_total(&mut world);

    assert!(
        (after_eat - before_eat).abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "eating food changed the closed total by {} — food is minting energy",
        after_eat - before_eat
    );
    assert!(
        world.resource::<EcosystemBiomass>().detritus < detritus_before,
        "feeding must draw the pool down"
    );
    let _ = before;
}

/// Epoch replacement must fund the replacement from the individual it replaces (D06), not hand it
/// a fresh reserve on top of returning the corpse to detritus.
#[test]
fn epoch_replacement_does_not_create_energy() {
    let (mut world, _tx) = build_live_world();
    let mut schedule = energy_schedule();
    schedule.run(&mut world);
    world.resource_mut::<EcosystemBiomass>().detritus = 1000.0;

    let before = closed_total(&mut world);
    let genotype = test_genotype();

    for round in 0..20u64 {
        let agents = living_agents(&mut world);
        assert!(
            !agents.is_empty(),
            "population died out before the test ended"
        );
        let victim = agents[round as usize % agents.len()];
        let pos = world
            .get::<Position>(victim)
            .map(|p| p.0)
            .unwrap_or(glam::Vec3::ZERO);
        world
            .resource_mut::<EvolutionQueue>()
            .pending_replacements
            .push((
                victim,
                genotype.clone(),
                pos,
                format!("lineage-{round}"),
                1,
                Vec::new(),
            ));
        schedule.run(&mut world);
    }

    let after = closed_total(&mut world);
    let drift = after - before;
    println!("20 replacements: before={before:.6} after={after:.6} drift={drift:.9}");
    assert!(
        drift.abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "20 epoch replacements moved the closed total by {drift}; before G1.1 each one minted a \
         full starting reserve"
    );
}

/// Diagnostic: run each energy system in isolation for many ticks and report which one moves the
/// closed total. Ignored by default — it is a bisector for conservation bugs, not a gate.
///
/// Run it with `cargo test --features desktop diagnose_which_system_moves_the_closed_total --
/// --ignored --nocapture` when `live_world_conserves_energy_across_births_deaths_and_a_save_load_cycle`
/// goes red and you need to know *which* system leaked.
///
/// The ignore is a recorded decision, not a parked test: it is named with this reasoning in
/// `scripts/test_target_policy.mjs`, and `check_test_targets.mjs` fails on any `#[ignore]` that is
/// not. Turning a print-only bisector into an always-pass assertion would buy a slow test that
/// proves nothing; the invariant itself is gated by the aggregate test named above.
#[test]
#[ignore]
fn diagnose_which_system_moves_the_closed_total() {
    use bevy_ecs::schedule::IntoSystemConfigs;
    let names: [&str; 10] = [
        "metabolic_decay",
        "spawn_food",
        "detect_food_collisions",
        "combat",
        "fruit_growth",
        "detect_environmental_collisions",
        "apply_staggered_evolution",
        "herbivore_grazing",
        "resource_field_regrowth",
        "ecosystem_census",
    ];
    for (idx, name) in names.iter().enumerate() {
        let (mut world, _tx) = build_live_world();
        // Warm up with the full schedule so the world is in a realistic state.
        let mut warm = energy_schedule();
        for _ in 0..200 {
            warm.run(&mut world);
        }
        let mut single = Schedule::default();
        match idx {
            0 => single.add_systems(metabolic_decay_system.into_configs()),
            1 => single.add_systems(spawn_food_system.into_configs()),
            2 => single.add_systems(detect_food_collisions_system.into_configs()),
            3 => single.add_systems(combat_system.into_configs()),
            4 => single.add_systems(fruit_growth_system.into_configs()),
            5 => single.add_systems(detect_environmental_collisions_system.into_configs()),
            6 => single.add_systems(apply_staggered_evolution_system.into_configs()),
            7 => single.add_systems(herbivore_grazing_system.into_configs()),
            8 => single.add_systems(resource_field_regrowth_system.into_configs()),
            _ => single.add_systems(ecosystem_census_system.into_configs()),
        };
        let before = closed_total(&mut world);
        for _ in 0..2000 {
            single.run(&mut world);
        }
        let after = closed_total(&mut world);
        println!("{name:38} delta={:+.9}", after - before);
    }
}
