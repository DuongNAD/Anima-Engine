mod common;

use bevy_ecs::prelude::*;
use glam::{Vec3, Quat};
use std::sync::Mutex;

use anima_engine_lib::core::ecs::{
    Position, Rotation, Velocity, MapBounds, EnvironmentState, Cloud,
    toroidal_distance_sq, get_local_light, update_day_night_cycle_system,
    Segment, JointAxis, ParentAgent, SegmentJointForce
};
use anima_engine_lib::physics::{
    resolve_joints_system, integrate_physics_system, RigidBody, JointConstraint
};
use anima_engine_lib::ai::cpg::{CpgOscillator, TimeStep};

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_stress_zero_joint_axis() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = World::new();
    world.insert_resource(TimeStep(0.016));
    world.insert_resource(MapBounds::default());

    let parent = world.spawn((
        Position(Vec3::ZERO),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 0, length: 2.0, radius: 0.5, mass: 1.0 },
        RigidBody { mass: 1.0, velocity: Vec3::ZERO, force: Vec3::ZERO },
    )).id();

    // Spawn child with a zero joint axis
    let child = world.spawn((
        Position(Vec3::new(0.0, 0.0, 2.0)),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 1, length: 2.0, radius: 0.5, mass: 1.0 },
        RigidBody { mass: 1.0, velocity: Vec3::ZERO, force: Vec3::ZERO },
        JointConstraint {
            parent_entity: parent,
            anchor_offset: Vec3::new(0.0, 0.0, 1.0),
            stiffness: 10.0,
            damping: 1.0,
        },
        JointAxis(Vec3::ZERO),
        CpgOscillator::new(1.0, 0.5),
        SegmentJointForce(0.0),
    )).id();

    let mut schedule = Schedule::default();
    schedule.add_systems((
        resolve_joints_system,
        integrate_physics_system.after(resolve_joints_system),
    ));

    // Run schedule for 5 ticks
    for _ in 0..5 {
        schedule.run(&mut world);
    }

    let child_rot = world.get::<Rotation>(child).unwrap().0;
    let child_pos = world.get::<Position>(child).unwrap().0;
    let child_vel = world.get::<Velocity>(child).unwrap().0;
    
    // Assert that no NaNs are produced
    assert!(!child_rot.is_nan(), "Child rotation became NaN due to zero JointAxis!");
    assert!(!child_pos.is_nan(), "Child position became NaN due to zero JointAxis!");
    assert!(!child_vel.is_nan(), "Child velocity became NaN due to zero JointAxis!");
}

#[test]
fn test_stress_nan_joint_axis() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = World::new();
    world.insert_resource(TimeStep(0.016));
    world.insert_resource(MapBounds::default());

    let parent = world.spawn((
        Position(Vec3::ZERO),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 0, length: 2.0, radius: 0.5, mass: 1.0 },
        RigidBody { mass: 1.0, velocity: Vec3::ZERO, force: Vec3::ZERO },
    )).id();

    let child = world.spawn((
        Position(Vec3::new(0.0, 0.0, 2.0)),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 1, length: 2.0, radius: 0.5, mass: 1.0 },
        RigidBody { mass: 1.0, velocity: Vec3::ZERO, force: Vec3::ZERO },
        JointConstraint {
            parent_entity: parent,
            anchor_offset: Vec3::new(0.0, 0.0, 1.0),
            stiffness: 10.0,
            damping: 1.0,
        },
        JointAxis(Vec3::NAN),
        CpgOscillator::new(1.0, 0.5),
        SegmentJointForce(0.0),
    )).id();

    let mut schedule = Schedule::default();
    schedule.add_systems((
        resolve_joints_system,
        integrate_physics_system.after(resolve_joints_system),
    ));

    for _ in 0..5 {
        schedule.run(&mut world);
    }

    let child_rot = world.get::<Rotation>(child).unwrap().0;
    let child_pos = world.get::<Position>(child).unwrap().0;
    let child_vel = world.get::<Velocity>(child).unwrap().0;
    
    // Assert that no NaNs are produced
    assert!(!child_rot.is_nan(), "Child rotation became NaN due to NaN JointAxis!");
    assert!(!child_pos.is_nan(), "Child position became NaN due to NaN JointAxis!");
    assert!(!child_vel.is_nan(), "Child velocity became NaN due to NaN JointAxis!");
}

#[test]
fn test_stress_extreme_coordinates_wrapping() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = World::new();
    world.insert_resource(TimeStep(0.016));
    let bounds = MapBounds {
        min: Vec3::new(-10.0, 0.0, -10.0),
        max: Vec3::new(10.0, 10.0, 10.0),
    };
    world.insert_resource(bounds);

    let parent = world.spawn((
        Position(Vec3::new(1000.0, 0.0, 1000.0)),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 0, length: 2.0, radius: 0.5, mass: 1.0 },
        RigidBody { mass: 1.0, velocity: Vec3::ZERO, force: Vec3::ZERO },
    )).id();

    let child = world.spawn((
        Position(Vec3::new(-1000.0, 0.0, -1000.0)),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 1, length: 2.0, radius: 0.5, mass: 1.0 },
        RigidBody { mass: 1.0, velocity: Vec3::ZERO, force: Vec3::ZERO },
        JointConstraint {
            parent_entity: parent,
            anchor_offset: Vec3::new(0.0, 0.0, 1.0),
            stiffness: 10.0,
            damping: 1.0,
        },
        JointAxis(Vec3::Y),
        CpgOscillator::new(1.0, 0.5),
        SegmentJointForce(0.0),
    )).id();

    let mut schedule = Schedule::default();
    schedule.add_systems((
        anima_engine_lib::core::ecs::wrap_coordinates_system,
        resolve_joints_system.after(anima_engine_lib::core::ecs::wrap_coordinates_system),
        integrate_physics_system.after(resolve_joints_system),
    ));

    schedule.run(&mut world);

    let parent_pos = world.get::<Position>(parent).unwrap().0;
    let child_pos = world.get::<Position>(child).unwrap().0;

    assert!(parent_pos.x >= bounds.min.x && parent_pos.x <= bounds.max.x);
    assert!(child_pos.x >= bounds.min.x && child_pos.x <= bounds.max.x);
}

#[test]
fn test_stress_sensor_attenuation_extreme_inputs() {
    let bounds = MapBounds {
        min: Vec3::new(-10.0, 0.0, -10.0),
        max: Vec3::new(10.0, 10.0, 10.0),
    };

    // Test with NaN ambient light
    let light_nan = get_local_light(Vec3::ZERO, f32::NAN, &[], &bounds);
    assert!(light_nan.is_nan() || light_nan == 0.05, "Expected NaN or clamped min 0.05, got {}", light_nan);

    // Test with negative ambient light
    let light_neg = get_local_light(Vec3::ZERO, -1.0, &[], &bounds);
    assert_eq!(light_neg, 0.05, "Expected clamped min 0.05 for negative ambient light, got {}", light_neg);

    // Test with negative cloud radius
    let clouds_neg_radius = vec![(Vec3::ZERO, -5.0, 1.0)];
    let light_neg_rad = get_local_light(Vec3::ZERO, 1.0, &clouds_neg_radius, &bounds);
    assert!(light_neg_rad >= 0.05);

    // Test with NaN cloud position
    let clouds_nan_pos = vec![(Vec3::NAN, 5.0, 1.0)];
    let light_nan_pos = get_local_light(Vec3::ZERO, 1.0, &clouds_nan_pos, &bounds);
    assert!(light_nan_pos >= 0.05);
}

#[test]
fn test_stress_day_night_extreme_inputs() {
    let env = EnvironmentState {
        ambient_light: 1.0,
        elapsed_time: f32::NAN,
        day_length: 0.0,
    };
    
    let mut world = World::new();
    world.insert_resource(TimeStep(0.016));
    world.insert_resource(env);

    let mut schedule = Schedule::default();
    schedule.add_systems(update_day_night_cycle_system);

    // Run - should handle NaN elapsed_time or 0.0 day_length without panicking
    schedule.run(&mut world);

    let env_res = world.resource::<EnvironmentState>();
    println!("Ambient light after day/night stress: {}", env_res.ambient_light);
}

#[test]
fn test_stress_timestep_nan_panic() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = World::new();
    world.insert_resource(TimeStep(f32::NAN));
    world.insert_resource(MapBounds::default());

    let parent = world.spawn((
        Position(Vec3::ZERO),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 0, length: 2.0, radius: 0.5, mass: 1.0 },
        RigidBody { mass: 1.0, velocity: Vec3::ZERO, force: Vec3::ZERO },
    )).id();

    let child = world.spawn((
        Position(Vec3::new(0.0, 0.0, 2.0)),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 1, length: 2.0, radius: 0.5, mass: 1.0 },
        RigidBody { mass: 1.0, velocity: Vec3::ZERO, force: Vec3::ZERO },
        JointConstraint {
            parent_entity: parent,
            anchor_offset: Vec3::new(0.0, 0.0, 1.0),
            stiffness: 10.0,
            damping: 1.0,
        },
        JointAxis(Vec3::Y),
        CpgOscillator::new(1.0, 0.5),
        SegmentJointForce(0.0),
    )).id();

    let mut schedule = Schedule::default();
    schedule.add_systems((
        resolve_joints_system,
        integrate_physics_system.after(resolve_joints_system),
    ));

    // Run schedule - check if we panic
    schedule.run(&mut world);

    let child_rot = world.get::<Rotation>(child).unwrap().0;
    let child_pos = world.get::<Position>(child).unwrap().0;
    let child_vel = world.get::<Velocity>(child).unwrap().0;
    
    // Assert that no NaNs are produced
    assert!(!child_rot.is_nan(), "Child rotation became NaN due to NaN TimeStep!");
    assert!(!child_pos.is_nan(), "Child position became NaN due to NaN TimeStep!");
    assert!(!child_vel.is_nan(), "Child velocity became NaN due to NaN TimeStep!");
}
