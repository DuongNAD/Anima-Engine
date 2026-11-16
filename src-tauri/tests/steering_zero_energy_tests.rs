//! STATE §3.00 P3 (follow-up to #76): `steering_drive_system` stops an agent that has no energy left.
//!
//! An agent at or below zero energy must get a zero `target_velocity`, even with a target in range
//! and even if it was steering on the previous tick, or the root force in `physics/dynamics.rs`
//! would keep pushing a body with nothing to pay for it. The code does this (`locomotion.rs`,
//! the `homeo.energy <= 0.0` branch); until now no test held it there.

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecs::{Agent, Food, InertiaComponent, Position};
use anima_engine_lib::core::locomotion::steering_drive_system;
use anima_engine_lib::core::resources::LocomotionPolicy;
use bevy_ecs::prelude::*;
use glam::Vec3;

/// A lone agent at the origin with `energy`, a food item 10 units away on +x, steering enabled
/// with a drive floor of 1 so the speed does not depend on the (absent) CPG. The agent starts with
/// a stale nonzero target velocity, as if it had been steering the tick before.
fn world_with(energy: f32) -> (World, Entity) {
    let mut world = World::new();
    world.insert_resource(LocomotionPolicy {
        enabled: true,
        drive_floor: 1.0,
        ..Default::default()
    });
    world.spawn((
        Food {
            energy_value: 30.0,
            hydration_value: 0.0,
        },
        Position(Vec3::new(10.0, 0.0, 0.0)),
    ));
    let homeo = HomeostaticState {
        energy,
        ..Default::default()
    };
    let inertia = InertiaComponent {
        target_velocity: Vec3::new(1.0, 2.0, 3.0),
        ..Default::default()
    };
    let agent = world
        .spawn((Agent, Position(Vec3::ZERO), homeo, inertia))
        .id();
    (world, agent)
}

fn steer_once(world: &mut World, agent: Entity) -> Vec3 {
    let mut schedule = Schedule::default();
    schedule.add_systems(steering_drive_system);
    schedule.run(world);
    world
        .get::<InertiaComponent>(agent)
        .expect("the agent keeps its inertia")
        .target_velocity
}

#[test]
fn an_agent_with_no_energy_gets_a_zero_target_velocity() {
    for energy in [0.0, -0.5, -100.0] {
        let (mut world, agent) = world_with(energy);
        assert_eq!(
            steer_once(&mut world, agent),
            Vec3::ZERO,
            "energy {energy}: a spent agent must stop steering, even with food in range"
        );
    }
}

#[test]
fn the_same_agent_with_energy_does_steer_toward_the_food() {
    // Control: the zero above comes from the energy check, not from a setup that never steers.
    let (mut world, agent) = world_with(50.0);
    let v = steer_once(&mut world, agent);
    assert!(
        v.x > 0.0 && v.y.abs() < 1e-6 && v.z.abs() < 1e-6,
        "a fed agent steers along +x toward the food, got {v:?}"
    );
}
