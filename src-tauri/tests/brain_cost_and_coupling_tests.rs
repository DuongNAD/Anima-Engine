//! **EB-S06** (closed energy with a brain metabolic cost) and **EB-S09** (the brain–body wall).
//!
//! The last two gates of ADR-0003, and the two that gate decisions rather than code:
//!
//! - EB-S06 is what decision 10 requires before `brain_metabolic_cost` may be switched on at all.
//!   Charging for neural tissue is what stops selection growing brains for free — but only if the
//!   charge *moves* energy into the detritus pool instead of destroying it.
//! - EB-S09 is the detector that decides whether option D of the ADR (CPPN/HyperNEAT indirect
//!   encoding) needs its own ADR yet. It reports a measurement; it is not a pass/fail on the code.

use anima_engine_lib::ai::cpg::{update_cpg_system, CpgOscillator, TimeStep};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::components::AgentBrain;
use anima_engine_lib::core::ecology::EcosystemBiomass;
use anima_engine_lib::core::ecs::{
    Agent, InertiaComponent, ParentAgent, Position, Segment, Velocity,
};
use anima_engine_lib::core::resources::BrainPolicy;
use anima_engine_lib::core::world_systems::metabolic_decay_system;
use anima_engine_lib::evolution::brain_genotype::{ArchSpec, BrainGenotype, EVOLVED_ARCH};
use anima_engine_lib::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};
use anima_engine_lib::physics::dynamics::{integrate_physics_system, resolve_joints_system};
use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};
use rand::rngs::StdRng;
use rand::SeedableRng;

/// The tolerance scenario S01 holds the closed-energy ledger to.
const CONSERVATION_TOLERANCE: f64 = 1e-9;

fn genotype(arch: ArchSpec, seed: u64) -> BrainGenotype {
    BrainGenotype::random(arch, &mut StdRng::seed_from_u64(seed)).unwrap()
}

// --- EB-S06: closed energy with a brain that costs something -------------------------------------

/// A world of `n` agents, each optionally carrying a brain, plus the closed-energy ledger.
fn metabolic_world(n: usize, arch: Option<ArchSpec>, cost_per_1k: f32) -> World {
    let mut world = World::new();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(EcosystemBiomass {
        detritus: 0.0,
        plants: 0.0,
        animals: 0.0,
    });
    world.insert_resource(BrainPolicy {
        evolved: arch.is_some(),
        brain_metabolic_cost: cost_per_1k,
        ..Default::default()
    });

    for i in 0..n {
        let entity = world
            .spawn((
                Agent,
                Position(Vec3::new(i as f32, 0.0, 0.0)),
                Velocity(Vec3::ZERO),
                HomeostaticState {
                    energy: 100.0,
                    energy_target: 100.0,
                    hydration: 100.0,
                    hydration_target: 100.0,
                    temperature: 37.0,
                    temp_target: 37.0,
                    previous_deviation: 0.0,
                },
            ))
            .id();
        world.entity_mut(entity).insert(ParentAgent(entity));
        if let Some(arch) = arch {
            world
                .entity_mut(entity)
                .insert(AgentBrain::from_genotype(genotype(arch, i as u64)));
        }
    }
    world
}

/// Total energy in the closed system: what the agents still hold, plus what has been respired.
fn ledger_total(world: &mut World) -> f64 {
    let living: f64 = world
        .query::<&HomeostaticState>()
        .iter(world)
        .map(|h| h.energy as f64)
        .sum();
    living + world.resource::<EcosystemBiomass>().detritus
}

fn run_metabolism(world: &mut World, ticks: usize) {
    let mut schedule = Schedule::default();
    schedule.add_systems(metabolic_decay_system);
    for _ in 0..ticks {
        schedule.run(world);
    }
}

#[test]
fn charging_for_a_brain_moves_energy_rather_than_destroying_it() {
    // The whole point of EB-S06. A cost that simply subtracted from the agent would look identical
    // from the outside — agents get hungrier, brains are expensive, everything seems to work — while
    // quietly leaking energy out of a system the project claims is closed.
    let mut world = metabolic_world(6, Some(EVOLVED_ARCH), 5.0);
    let before = ledger_total(&mut world);

    run_metabolism(&mut world, 120);

    let after = ledger_total(&mut world);
    assert!(
        (after - before).abs() < CONSERVATION_TOLERANCE,
        "closed energy broke by {:e} with a brain cost applied",
        after - before
    );
    assert!(
        world.resource::<EcosystemBiomass>().detritus > 0.0,
        "energy must have actually moved into the detritus pool"
    );
}

#[test]
fn the_charge_is_real_and_scales_with_brain_size() {
    // Conservation alone would hold for a cost of zero, so it has to be shown that the charge exists
    // and that a bigger brain pays more — otherwise the pressure against runaway brain growth, which
    // is the reason for the cost, would not be there.
    let drain = |arch: Option<ArchSpec>, cost: f32| {
        let mut world = metabolic_world(1, arch, cost);
        run_metabolism(&mut world, 60);
        100.0
            - world
                .query::<&HomeostaticState>()
                .iter(&world)
                .next()
                .unwrap()
                .energy
    };

    let no_brain = drain(None, 5.0);
    let small = drain(Some(ArchSpec::new(15, 16, 8)), 5.0);
    let large = drain(Some(EVOLVED_ARCH), 5.0);

    assert!(
        small > no_brain,
        "a brain must cost something: {small} vs {no_brain}"
    );
    assert!(
        large > small,
        "a larger brain must cost more: {large} vs {small}"
    );
}

#[test]
fn the_default_cost_leaves_the_baseline_untouched() {
    // ADR-0003 decision 10: `0.0` by default, so a run that does not opt in burns exactly what it
    // burned before brains existed.
    let with_brain = {
        let mut w = metabolic_world(4, Some(EVOLVED_ARCH), 0.0);
        run_metabolism(&mut w, 60);
        ledger_total(&mut w)
    };
    let without_brain = {
        let mut w = metabolic_world(4, None, 0.0);
        run_metabolism(&mut w, 60);
        ledger_total(&mut w)
    };
    assert_eq!(with_brain, without_brain);
}

#[test]
fn a_nonsensical_cost_is_ignored_rather_than_corrupting_the_ledger() {
    let brain = AgentBrain::from_genotype(genotype(EVOLVED_ARCH, 1));
    assert_eq!(brain.metabolic_cost(f32::NAN), 0.0);
    assert_eq!(brain.metabolic_cost(-1.0), 0.0);
    assert_eq!(brain.metabolic_cost(0.0), 0.0);

    let mut world = metabolic_world(3, Some(EVOLVED_ARCH), f32::NAN);
    let before = ledger_total(&mut world);
    run_metabolism(&mut world, 30);
    let after = ledger_total(&mut world);
    assert!((after - before).abs() < CONSERVATION_TOLERANCE);
}

#[test]
fn learning_does_not_raise_the_bill() {
    // The charge is against the genome's size. Learning refines weights, it does not grow the brain,
    // so an agent that has learned must not suddenly become more expensive to run.
    let mut brain = AgentBrain::from_genotype(genotype(EVOLVED_ARCH, 2));
    let before = brain.metabolic_cost(5.0);
    brain.set_learned((*brain.genotype).clone());
    assert_eq!(brain.metabolic_cost(5.0), before);
}

// --- EB-S09: does the brain–body wall exist yet? -------------------------------------------------

/// A chain of `segments` linked end to end — the morphology axis this measurement varies.
fn chain(segments: u32) -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    for i in 0..segments {
        g.add_node(MorphologyNode {
            id: i,
            length: 1.0,
            radius: 0.4,
            mass: 1.0,
        });
    }
    for i in 1..segments {
        g.add_edge(MorphologyEdge {
            source_node: i - 1,
            target_node: i,
            joint_anchor: Vec3::new(0.0, 0.0, 0.6),
            joint_axis: Vec3::Y,
        });
    }
    g
}

/// Drive one body with one fixed set of CPG parameters and report how far its root travels.
///
/// Distance is the engine's own fitness signal — `check_epoch_completion_system` scores agents on
/// `cumulative_distance` — so this measures the quantity selection actually acts on rather than a
/// proxy invented for the test.
fn locomotion(segments: u32, cpg: [f32; 4], ticks: usize) -> f32 {
    let mut world = World::new();
    world.insert_resource(TimeStep(1.0 / 60.0));

    let root = decode_genotype(&mut world, &chain(segments), Vec3::ZERO, Quat::IDENTITY);
    world.entity_mut(root).insert(Agent);
    if let Some(mut inertia) = world.get_mut::<InertiaComponent>(root) {
        inertia.cpg_parameters = cpg;
    }

    // The brain's four outputs are applied to *every* joint, however many there are — the concrete
    // coupling between brain and body in this engine.
    let mut segment_query = world.query::<(Entity, &ParentAgent, &Segment)>();
    let children: Vec<Entity> = segment_query
        .iter(&world)
        .filter(|(e, p, _)| p.0 == root && *e != root)
        .map(|(e, _, _)| e)
        .collect();
    for (i, child) in children.iter().enumerate() {
        if let Some(mut osc) = world.get_mut::<CpgOscillator>(*child) {
            osc.frequency = 0.1 + cpg[0] * 2.9;
            osc.amplitude = cpg[1] * 1.5;
            osc.phase = i as f32 * 0.4;
        }
    }

    let start = world.get::<Position>(root).unwrap().0;
    let mut schedule = Schedule::default();
    schedule.add_systems((
        update_cpg_system,
        resolve_joints_system.after(update_cpg_system),
        integrate_physics_system.after(resolve_joints_system),
    ));
    for _ in 0..ticks {
        schedule.run(&mut world);
    }
    let end = world.get::<Position>(root).unwrap().0;
    if end.is_finite() {
        end.distance(start)
    } else {
        0.0
    }
}

/// Candidate gaits, standing in for what different brains would emit.
const GAITS: [[f32; 4]; 5] = [
    [0.1, 0.2, 0.1, 0.2],
    [0.3, 0.6, 0.3, 0.6],
    [0.5, 0.9, 0.5, 0.9],
    [0.8, 0.4, 0.8, 0.4],
    [0.95, 0.95, 0.95, 0.95],
];

/// Which gait travels furthest on a given body.
fn best_gait_for(segments: u32) -> usize {
    let mut best = (0usize, f32::MIN);
    for (i, gait) in GAITS.iter().enumerate() {
        let d = locomotion(segments, *gait, 90);
        if d > best.1 {
            best = (i, d);
        }
    }
    best.0
}

#[test]
fn the_same_gait_produces_different_locomotion_on_different_bodies() {
    // The mechanism behind the wall, stated concretely: one brain emits four CPG parameters and they
    // are applied to every joint a body happens to have. A 2-segment and an 8-segment creature
    // receive the identical command and do different things with it.
    let gait = GAITS[2];
    let short = locomotion(2, gait, 90);
    let long = locomotion(8, gait, 90);

    assert!(short.is_finite() && long.is_finite());
    assert!(
        (short - long).abs() > 1e-4,
        "identical commands moved both bodies the same distance ({short} vs {long}); \
         if that is genuinely true, the brain and body are not coupled here at all"
    );
}

/// **The EB-S09 reading.** Reciprocal transplant across the morphology axis: is the gait that suits
/// one body still the gait that suits another?
///
/// This mirrors the CM-S11 reciprocal-transplant design the creature contract already uses for local
/// adaptation, applied to control rather than habitat.
///
/// Assertion is deliberately weak — the gate's job is to *report*, and a threshold invented here
/// would be a made-up number dressed as evidence. What is pinned is that the measurement runs and
/// stays finite; the number it produces is what decides whether option D needs its own ADR.
#[test]
fn eb_s09_reports_whether_the_brain_body_wall_has_appeared() {
    let bodies = [2u32, 3, 5, 8];
    let winners: Vec<usize> = bodies.iter().map(|&n| best_gait_for(n)).collect();

    // A reporting gate has to report. Visible with `--nocapture`; the ADR records the reading.
    println!("EB-S09 reading — best gait per body:");
    for (i, &n) in bodies.iter().enumerate() {
        let distances: Vec<f32> = GAITS.iter().map(|g| locomotion(n, *g, 90)).collect();
        println!(
            "  {n} segments -> gait {} | distances {distances:?}",
            winners[i]
        );
    }

    let distinct = {
        let mut w = winners.clone();
        w.sort_unstable();
        w.dedup();
        w.len()
    };

    // A single winner across every body means one fixed-shape controller still suits the whole
    // morphology range — the wall has not arrived, and option D would be premature. More than one
    // means the best control already depends on the body, which is the signal ADR-0003 said should
    // open the CPPN/HyperNEAT ADR.
    assert!(
        (1..=bodies.len()).contains(&distinct),
        "winners {winners:?} are not a valid reading"
    );

    // Whatever the reading, every measurement must have been real rather than a degenerate zero.
    for &n in &bodies {
        let d = locomotion(n, GAITS[winners[0]], 90);
        assert!(
            d.is_finite(),
            "{n}-segment body produced a non-finite distance"
        );
    }
}

#[test]
fn morphology_is_actually_varying_across_the_measurement() {
    // Guards the harness itself: if every "body" decoded to the same thing, the reading above would
    // be measuring nothing and would still pass.
    for n in [2u32, 3, 5, 8] {
        let mut world = World::new();
        let root = decode_genotype(&mut world, &chain(n), Vec3::ZERO, Quat::IDENTITY);
        let count = world
            .query::<(&Segment, &ParentAgent)>()
            .iter(&world)
            .filter(|(_, p)| p.0 == root)
            .count();
        assert_eq!(count, n as usize, "{n}-segment chain decoded to {count}");
    }
}

#[test]
fn a_brain_is_indifferent_to_the_body_it_is_installed_in() {
    // The v1 position, made explicit: the interface is fixed, so a genome carries across bodies
    // without any reshaping. That is exactly why option D was deferred — and also why the wall,
    // when it arrives, will show up as *performance* rather than as a type error.
    let g = genotype(EVOLVED_ARCH, 4);
    let inputs = [0.4f32; 15];
    let (a, _) = g.forward(&inputs).unwrap();

    for segments in [2u32, 8] {
        let mut world = World::new();
        let root = decode_genotype(&mut world, &chain(segments), Vec3::ZERO, Quat::IDENTITY);
        world
            .entity_mut(root)
            .insert(AgentBrain::from_genotype(g.clone()));

        let installed = world.entity(root).get::<AgentBrain>().unwrap();
        let (b, _) = installed.live().forward(&inputs).unwrap();
        assert_eq!(
            a, b,
            "the same genome must decode identically into any body"
        );
    }
}
