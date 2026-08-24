//! **EB-S05** — installing the ecological action gates must not change any behaviour.
//!
//! ADR-0003 decision 4 gives the brain control over three things that were previously automatic:
//! pheromone emission, striking prey, and eating. This step wires the valves but leaves every one of
//! them fully open, so the simulation must compute exactly what it computed before.
//!
//! "Exactly" is the load-bearing word, which is why the identity tests here compare a world carrying
//! `ActionGates::default()` against a world carrying no gates component at all. Each is paired with a
//! test that a *closed* gate really does suppress the action — an identity test alone would also pass
//! if the gate were silently ignored, which is precisely the failure worth catching.

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::ai::pheromone::{
    agent_release_pheromone_system, PheromoneGrid, PheromoneReleaser,
};
use anima_engine_lib::core::components::{ActionGates, ACTION_GATE_THRESHOLD};
use anima_engine_lib::core::ecs::{
    Agent, Food, MapBounds, ParentAgent, Position, Predator, Prey, Velocity,
};
use anima_engine_lib::core::resources::{CombatEvents, FoodSpawnSettings};
use anima_engine_lib::core::world_systems::{combat_system, detect_food_collisions_system};
use bevy_ecs::prelude::*;
use glam::Vec3;

fn bounds() -> MapBounds {
    MapBounds {
        min: Vec3::new(-50.0, 0.0, -50.0),
        max: Vec3::new(50.0, 10.0, 50.0),
    }
}

fn homeostasis(energy: f32) -> HomeostaticState {
    HomeostaticState {
        energy,
        energy_target: 100.0,
        hydration: 50.0,
        hydration_target: 100.0,
        temperature: 37.0,
        temp_target: 37.0,
        previous_deviation: 0.0,
    }
}

// --- pheromone emission -------------------------------------------------------------------------

/// Emit for one tick and return the grid cell the agent stands on.
fn emit_once(gates: Option<ActionGates>) -> f32 {
    let mut world = World::new();
    world.insert_resource(PheromoneGrid::new(0.0, 0.0)); // no diffusion, no decay: isolate emission
    world.insert_resource(bounds());
    world.insert_resource(TimeStep(1.0));

    let pos = Position(Vec3::new(1.0, 0.0, 2.0));
    let mut agent = world.spawn((pos, PheromoneReleaser { strength: 4.0 }));
    if let Some(g) = gates {
        agent.insert(g);
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(agent_release_pheromone_system);
    schedule.run(&mut world);

    let grid = world.resource::<PheromoneGrid>();
    let idx = grid
        .pos_to_index(pos.0, &bounds())
        .expect("agent is in bounds");
    grid.values[idx]
}

#[test]
fn default_gates_emit_exactly_what_no_gates_emit() {
    let legacy = emit_once(None);
    assert!(legacy > 0.0, "the baseline must actually deposit something");
    assert_eq!(emit_once(Some(ActionGates::default())), legacy);
}

#[test]
fn a_closed_pheromone_gate_silences_the_agent() {
    assert_eq!(
        emit_once(Some(ActionGates {
            pheromone_emit: 0.0,
            ..Default::default()
        })),
        0.0,
        "a closed gate must be wired, not ignored"
    );
}

#[test]
fn pheromone_emission_scales_continuously() {
    // Emission strength is a continuous quantity, so its gate is a multiplier rather than a switch.
    let full = emit_once(None);
    let half = emit_once(Some(ActionGates {
        pheromone_emit: 0.5,
        ..Default::default()
    }));
    assert!((half - full * 0.5).abs() < 1e-6, "{half} vs {full}");
}

#[test]
fn pheromone_scale_is_clamped_against_wild_outputs() {
    // Nothing constrains a future brain's output, so the gate must not let it inject unbounded
    // pheromone or subtract from the field.
    let full = emit_once(None);
    assert_eq!(
        emit_once(Some(ActionGates {
            pheromone_emit: 25.0,
            ..Default::default()
        })),
        full
    );
    assert_eq!(
        emit_once(Some(ActionGates {
            pheromone_emit: -3.0,
            ..Default::default()
        })),
        0.0
    );
}

// --- feeding ------------------------------------------------------------------------------------

/// Place one agent on top of one food item, run the collision system, return the agent's energy.
fn feed_once(gates: Option<ActionGates>) -> f32 {
    let mut world = World::new();
    world.insert_resource(FoodSpawnSettings::default());

    let at = Vec3::new(0.0, 0.0, 0.0);
    let mut agent = world.spawn((Agent, Position(at), Velocity(Vec3::ZERO), homeostasis(10.0)));
    if let Some(g) = gates {
        agent.insert(g);
    }
    let agent = agent.id();
    world.entity_mut(agent).insert(ParentAgent(agent));

    world.spawn((
        Food {
            energy_value: 30.0,
            hydration_value: 20.0,
        },
        Position(at),
    ));

    let mut schedule = Schedule::default();
    schedule.add_systems(detect_food_collisions_system);
    schedule.run(&mut world);

    world
        .entity(agent)
        .get::<HomeostaticState>()
        .unwrap()
        .energy
}

#[test]
fn default_gates_feed_exactly_as_no_gates_do() {
    let legacy = feed_once(None);
    assert!(legacy > 10.0, "the baseline must actually eat");
    assert_eq!(feed_once(Some(ActionGates::default())), legacy);
}

#[test]
fn a_closed_feed_gate_leaves_the_food_alone() {
    assert_eq!(
        feed_once(Some(ActionGates {
            feed_intent: 0.0,
            ..Default::default()
        })),
        10.0,
        "a closed gate must be wired, not ignored"
    );
}

// --- predation ----------------------------------------------------------------------------------

/// Put a predator on top of prey, run combat, return `(predator_energy, prey_energy)`.
/// `with_events` selects which of the two branches inside `combat_system` executes.
fn strike_once(gates: Option<ActionGates>, with_events: bool) -> (f32, f32) {
    let mut world = World::new();
    if with_events {
        world.insert_resource(CombatEvents {
            events: Vec::with_capacity(16),
            predator_centroids: Vec::with_capacity(16),
            prey_centroids: Vec::with_capacity(16),
        });
    }

    let at = Vec3::new(0.0, 0.0, 0.0);
    let mut pred = world.spawn((Agent, Predator, Position(at), homeostasis(10.0)));
    if let Some(g) = gates {
        pred.insert(g);
    }
    let pred = pred.id();
    world.entity_mut(pred).insert(ParentAgent(pred));

    let prey = world
        .spawn((Agent, Prey, Position(at), homeostasis(80.0)))
        .id();
    world.entity_mut(prey).insert(ParentAgent(prey));

    let mut schedule = Schedule::default();
    schedule.add_systems(combat_system);
    schedule.run(&mut world);

    (
        world.entity(pred).get::<HomeostaticState>().unwrap().energy,
        world.entity(prey).get::<HomeostaticState>().unwrap().energy,
    )
}

#[test]
fn default_gates_strike_exactly_as_no_gates_do() {
    // Both branches of `combat_system` must agree, since which one runs depends only on whether the
    // CombatEvents resource happens to exist.
    for with_events in [true, false] {
        let legacy = strike_once(None, with_events);
        assert!(
            legacy.0 > 10.0 && legacy.1 < 80.0,
            "with_events={with_events}: the baseline must actually predate, got {legacy:?}"
        );
        assert_eq!(
            strike_once(Some(ActionGates::default()), with_events),
            legacy,
            "with_events={with_events}"
        );
    }
}

#[test]
fn a_closed_attack_gate_spares_the_prey() {
    for with_events in [true, false] {
        assert_eq!(
            strike_once(
                Some(ActionGates {
                    attack_intent: 0.0,
                    ..Default::default()
                }),
                with_events
            ),
            (10.0, 80.0),
            "with_events={with_events}: a closed gate must be wired in both branches"
        );
    }
}

// --- archetype ordering ---------------------------------------------------------------------------

/// Three predators share one prey; returns each predator's energy in spawn order.
///
/// This is the order-sensitive path in the engine. `combat_system` mutates homeostasis **directly**
/// rather than through `Commands`, and `predation_capture` is a function of the prey's *current*
/// energy — so the predator that strikes first takes the richest bite and the rest divide what is
/// left. Change the iteration order and the split changes.
///
/// Feeding deliberately is not used here: `detect_food_collisions_system` despawns food through
/// deferred `Commands`, so every agent in a tick sees the food as still present and nobody is
/// out-competed. There is no contested outcome there to reorder.
fn contested_predation(with_gates: bool) -> Vec<f32> {
    let mut world = World::new();
    world.insert_resource(CombatEvents {
        events: Vec::with_capacity(16),
        predator_centroids: Vec::with_capacity(16),
        prey_centroids: Vec::with_capacity(16),
    });

    let at = Vec3::new(0.0, 0.0, 0.0);
    let mut predators = Vec::new();
    for _ in 0..3 {
        let mut e = world.spawn((Agent, Predator, Position(at), homeostasis(10.0)));
        if with_gates {
            e.insert(ActionGates::default());
        }
        let id = e.id();
        world.entity_mut(id).insert(ParentAgent(id));
        predators.push(id);
    }

    let prey = world
        .spawn((Agent, Prey, Position(at), homeostasis(60.0)))
        .id();
    world.entity_mut(prey).insert(ParentAgent(prey));

    let mut schedule = Schedule::default();
    schedule.add_systems(combat_system);
    schedule.run(&mut world);

    predators
        .into_iter()
        .map(|p| world.entity(p).get::<HomeostaticState>().unwrap().energy)
        .collect()
}

/// Adding a component moves an entity to a different archetype, and Bevy iterates queries by
/// archetype. With every gate wide open the *arithmetic* is unchanged, but a reordering would still
/// change **which** predator strikes first and therefore how the prey's energy is divided — a
/// behaviour change the per-system identity tests above cannot see, because each of those has a
/// single actor.
#[test]
fn installing_gates_does_not_reorder_contested_outcomes() {
    let without = contested_predation(false);
    let with = contested_predation(true);

    assert!(
        without.iter().any(|e| *e > 10.0),
        "the scenario must actually predate, got {without:?}"
    );
    assert!(
        without.windows(2).any(|w| w[0] != w[1]),
        "predators must receive unequal shares or the test cannot detect a reorder, got {without:?}"
    );
    assert_eq!(
        with, without,
        "gates changed how the prey was divided: archetype ordering is not neutral"
    );
}

// --- gate semantics -----------------------------------------------------------------------------

#[test]
fn a_missing_component_reads_as_fully_open() {
    // The dangerous default would be the other way round: a save written before gates existed must
    // not load as an agent that refuses to eat.
    assert_eq!(ActionGates::of(None), ActionGates::OPEN);
    assert_eq!(ActionGates::default(), ActionGates::OPEN);
    assert!(ActionGates::OPEN.attacks() && ActionGates::OPEN.feeds());
    assert_eq!(ActionGates::OPEN.pheromone_scale(), 1.0);
}

#[test]
fn intent_fires_exactly_at_the_threshold() {
    let at = |v: f32| ActionGates {
        attack_intent: v,
        feed_intent: v,
        ..Default::default()
    };
    assert!(at(ACTION_GATE_THRESHOLD).attacks() && at(ACTION_GATE_THRESHOLD).feeds());
    let below = ACTION_GATE_THRESHOLD - f32::EPSILON;
    assert!(!at(below).attacks() && !at(below).feeds());
}

#[test]
fn gates_round_trip_through_serde() {
    // They will ride along in save state and migration payloads.
    let gates = ActionGates {
        pheromone_emit: 0.25,
        attack_intent: 0.75,
        feed_intent: 0.5,
    };
    let json = serde_json::to_string(&gates).unwrap();
    assert_eq!(serde_json::from_str::<ActionGates>(&json).unwrap(), gates);
}

#[test]
fn decoded_agents_carry_open_gates() {
    use anima_engine_lib::evolution::genotype::{
        decode_genotype, MorphologyGenotype, MorphologyNode,
    };

    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.5,
        mass: 1.0,
    });

    let mut world = World::new();
    let root = decode_genotype(&mut world, &genotype, Vec3::ZERO, glam::Quat::IDENTITY);

    assert_eq!(
        world.entity(root).get::<ActionGates>().copied(),
        Some(ActionGates::OPEN),
        "spawning must install gates, and install them open"
    );
}
