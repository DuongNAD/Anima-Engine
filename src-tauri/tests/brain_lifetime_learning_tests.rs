//! **EB-S08** — in-life learning, behind its own flag.
//!
//! ADR-0003 decision 6: evolution decides where a brain starts, learning refines it within one
//! lifetime, and what is learned dies with the individual. This covers the ECS side; the gradient
//! itself is pinned in `evolution::brain_genotype` by a finite-difference check.
//!
//! The gate asks for a report rather than a threshold, so most of what matters here is negative
//! space: with the flag off nothing moves, the genome is never written back, and an agent outside
//! the active radius is left alone.

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::{HomeostaticState, LastTransitionState};
use anima_engine_lib::ai::model::hrrl_learning_system;
use anima_engine_lib::core::components::AgentBrain;
use anima_engine_lib::core::ecs::{Agent, Position, Rotation};
use anima_engine_lib::core::resources::{BrainPolicy, LifetimeLearning};
use anima_engine_lib::evolution::brain_genotype::{
    learn_step, BrainGenotype, LearnScratch, EVOLVED_ARCH,
};
use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};
use rand::rngs::StdRng;
use rand::SeedableRng;

fn brain(seed: u64) -> AgentBrain {
    let mut rng = StdRng::seed_from_u64(seed);
    AgentBrain::from_genotype(BrainGenotype::random(EVOLVED_ARCH, &mut rng).unwrap())
}

fn learning(enabled: bool) -> LifetimeLearning {
    LifetimeLearning {
        enabled,
        learning_rate: 0.05,
        discount: 0.99,
        interval: 1,
        active_radius: f32::INFINITY,
    }
}

/// A world with one agent that has already acted, so there is a transition to learn from.
fn world_with_agent(policy: BrainPolicy, at: Vec3) -> (World, Entity) {
    let mut world = World::new();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(policy);

    let entity = world
        .spawn((
            Agent,
            Position(at),
            Rotation(Quat::IDENTITY),
            HomeostaticState {
                energy: 60.0,
                energy_target: 100.0,
                hydration: 60.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                // A previous deviation well above the current one makes the reward positive.
                previous_deviation: 50.0,
            },
            LastTransitionState {
                state: [0.3; 15],
                action: [0.9, 0.1, 0.8, 0.2],
                has_last: true,
            },
            brain(7),
        ))
        .id();
    (world, entity)
}

fn run(world: &mut World, ticks: usize) {
    let mut schedule = Schedule::default();
    schedule.add_systems(hrrl_learning_system);
    for _ in 0..ticks {
        schedule.run(world);
    }
}

fn live_weights(world: &World, e: Entity) -> Vec<f32> {
    world
        .entity(e)
        .get::<AgentBrain>()
        .unwrap()
        .live_weights()
        .to_vec()
}

// --- the flag is off by default -----------------------------------------------------------------

#[test]
fn nothing_learns_by_default() {
    let policy = BrainPolicy {
        evolved: true,
        ..Default::default()
    };
    assert!(
        !policy.lifetime_learning.enabled,
        "the flag must default off"
    );

    let (mut world, agent) = world_with_agent(policy, Vec3::ZERO);
    let before = live_weights(&world, agent);
    run(&mut world, 20);

    assert_eq!(live_weights(&world, agent), before);
    assert!(
        world
            .entity(agent)
            .get::<AgentBrain>()
            .unwrap()
            .learned
            .is_none(),
        "with learning off, an agent must still be running its genome"
    );
}

#[test]
fn learning_requires_an_evolved_brain_to_learn_in() {
    // Learning without a per-agent brain has nothing of its own to change; enabling it alone must
    // not start writing to agents that are meant to be on the shared model.
    let policy = BrainPolicy {
        evolved: false,
        lifetime_learning: learning(true),
        ..Default::default()
    };
    let (mut world, agent) = world_with_agent(policy, Vec3::ZERO);
    let before = live_weights(&world, agent);
    run(&mut world, 10);
    assert_eq!(live_weights(&world, agent), before);
}

// --- switched on ---------------------------------------------------------------------------------

#[test]
fn an_enabled_agent_learns_within_its_lifetime() {
    let policy = BrainPolicy {
        evolved: true,
        lifetime_learning: learning(true),
        ..Default::default()
    };
    let (mut world, agent) = world_with_agent(policy, Vec3::ZERO);
    let before = live_weights(&world, agent);
    run(&mut world, 5);

    let after = live_weights(&world, agent);
    assert_ne!(after, before, "an enabled agent must actually change");
    assert!(after.iter().all(|w| w.is_finite()));
}

#[test]
fn lifetime_learning_uses_the_exact_completed_transition() {
    let policy = BrainPolicy {
        evolved: true,
        lifetime_learning: learning(true),
        ..Default::default()
    };
    let (mut world, agent) = world_with_agent(policy, Vec3::ZERO);
    let before = world
        .entity(agent)
        .get::<AgentBrain>()
        .unwrap()
        .live()
        .as_ref()
        .clone();
    let previous_state = [0.3; 15];
    let action = [0.9, 0.1, 0.8, 0.2];
    let next_state = [
        0.0, 0.0, 0.0, 60.0, 100.0, 60.0, 100.0, 37.0, 37.0, 10.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    ];
    let homeo = world.entity(agent).get::<HomeostaticState>().unwrap();
    let reward = homeo.previous_deviation - homeo.compute_deviation();

    let mut expected = before.clone();
    let next_value = expected
        .forward(&next_state)
        .expect("the fixture is finite")
        .1;
    learn_step(
        &mut expected,
        &previous_state,
        &action,
        reward,
        next_value,
        0.99,
        0.05,
        &mut LearnScratch::default(),
    )
    .expect("the exact completed transition is learnable");

    // This is the production order. `hrrl_learning_system` owns the exact (s, a, r, s') tuple;
    // lifetime learning must consume that tuple before its state/deviation bookkeeping advances.
    let mut schedule = Schedule::default();
    schedule.add_systems(hrrl_learning_system);
    schedule.run(&mut world);

    let actual = world
        .entity(agent)
        .get::<AgentBrain>()
        .unwrap()
        .live_weights();
    let first_mismatch = actual
        .iter()
        .zip(&expected.weights)
        .position(|(actual, expected)| actual.to_bits() != expected.to_bits());
    assert_eq!(actual.len(), expected.weights.len());
    assert!(
        first_mismatch.is_none(),
        "lifetime learning must use previous state for the actor, current state for V(s'), and the \
         pre-refresh homeostatic reward; first mismatching weight: {first_mismatch:?}"
    );
}

#[test]
fn learning_never_writes_back_into_the_genome() {
    // The Baldwin position, checked through the running system rather than on the type alone: an
    // agent that learns must still pass its *original* brain to its offspring.
    let policy = BrainPolicy {
        evolved: true,
        lifetime_learning: learning(true),
        ..Default::default()
    };
    let (mut world, agent) = world_with_agent(policy, Vec3::ZERO);
    let genome_before = world
        .entity(agent)
        .get::<AgentBrain>()
        .unwrap()
        .genotype
        .weights
        .clone();

    run(&mut world, 25);

    let brain = world.entity(agent).get::<AgentBrain>().unwrap();
    assert_eq!(
        brain.genotype.weights, genome_before,
        "what was learned must not become heritable"
    );
    assert!(brain.learned.is_some());
    assert_ne!(brain.live_weights(), genome_before.as_slice());
}

#[test]
fn learning_is_reproducible_under_one_seed() {
    // Learning must not be a second source of run-to-run variation, or the EB-S08 comparison would
    // be measuring noise.
    let once = || {
        let policy = BrainPolicy {
            evolved: true,
            lifetime_learning: learning(true),
            ..Default::default()
        };
        let (mut world, agent) = world_with_agent(policy, Vec3::ZERO);
        run(&mut world, 15);
        live_weights(&world, agent)
    };
    assert_eq!(once(), once());
}

// --- the constraints hold ------------------------------------------------------------------------

#[test]
fn only_agents_inside_the_active_radius_learn() {
    // ADR-0003 decision 6 confines learning to the active radius, because one backward pass per
    // agent per tick is the expensive half of the hybrid.
    let policy = BrainPolicy {
        evolved: true,
        lifetime_learning: LifetimeLearning {
            active_radius: 10.0,
            ..learning(true)
        },
        ..Default::default()
    };

    let (mut inside, a) = world_with_agent(policy, Vec3::new(3.0, 0.0, 0.0));
    let (mut outside, b) = world_with_agent(policy, Vec3::new(100.0, 0.0, 0.0));
    let before_a = live_weights(&inside, a);
    let before_b = live_weights(&outside, b);

    run(&mut inside, 5);
    run(&mut outside, 5);

    assert_ne!(
        live_weights(&inside, a),
        before_a,
        "an agent in range must learn"
    );
    assert_eq!(
        live_weights(&outside, b),
        before_b,
        "an agent out of range must be left alone"
    );
}

#[test]
fn the_interval_throttles_how_often_learning_runs() {
    // Each update replaces the agent's network, which allocates; the interval is the knob that trades
    // adaptation speed against that cost, so it has to actually take effect.
    let policy_at = |interval: u32| BrainPolicy {
        evolved: true,
        lifetime_learning: LifetimeLearning {
            interval,
            ..learning(true)
        },
        ..Default::default()
    };

    let (mut every_tick, a) = world_with_agent(policy_at(1), Vec3::ZERO);
    let (mut rarely, b) = world_with_agent(policy_at(100), Vec3::ZERO);
    let before = live_weights(&every_tick, a);

    run(&mut every_tick, 8);
    run(&mut rarely, 8);

    assert_ne!(live_weights(&every_tick, a), before);
    assert_eq!(
        live_weights(&rarely, b),
        before,
        "an interval longer than the run must produce no update at all"
    );
}

#[test]
fn an_agent_that_has_not_acted_yet_is_skipped() {
    // `has_last = false` means there is no transition to learn from. Training on the zeroed
    // placeholder would teach the agent about a situation that never happened.
    let policy = BrainPolicy {
        evolved: true,
        lifetime_learning: learning(true),
        ..Default::default()
    };
    let (mut world, agent) = world_with_agent(policy, Vec3::ZERO);
    world
        .entity_mut(agent)
        .get_mut::<LastTransitionState>()
        .unwrap()
        .has_last = false;

    let before = live_weights(&world, agent);
    run(&mut world, 10);
    assert_eq!(live_weights(&world, agent), before);
}

#[test]
fn a_world_without_the_policy_resource_does_not_panic() {
    // The system takes the policy as optional so a harness that never configured brains — most of
    // the existing test suite — keeps working rather than tripping over a missing resource.
    let mut world = World::new();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.spawn((Agent, Position(Vec3::ZERO)));
    run(&mut world, 3);
}
