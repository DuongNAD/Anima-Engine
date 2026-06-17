mod common;

use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};
use anima_engine_lib::physics::{
    resolve_joints_system, integrate_physics_system, RigidBody, JointConstraint,
    rebuild_spatial_grid_system, SpatialCollider, SpatialHashGrid, Ray3D,
};
use anima_engine_lib::core::ecs::{
    init_world, wrap_coordinates_system, energy_decay_system, Position, Rotation, Velocity, Segment, JointAxis, MapBounds,
};
use anima_engine_lib::ai::cpg::{update_cpg_system, CpgOscillator, TimeStep};
use std::sync::Mutex;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_static_equilibrium() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = World::new();
    world.insert_resource(TimeStep(0.01));

    let parent = world.spawn((
        Position(Vec3::ZERO),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 0, length: 2.0, radius: 0.5, mass: 1.0 },
        RigidBody { mass: 1.0, velocity: Vec3::ZERO, force: Vec3::ZERO },
    )).id();

    let child = world.spawn((
        Position(Vec3::new(0.0, 0.0, 3.0)),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 1, length: 2.0, radius: 0.5, mass: 1.0 },
        RigidBody { mass: 1.0, velocity: Vec3::ZERO, force: Vec3::ZERO },
        JointConstraint {
            parent_entity: parent,
            anchor_offset: Vec3::new(0.0, 0.0, 1.0),
            stiffness: 100.0,
            damping: 10.0,
        },
    )).id();

    let mut schedule = Schedule::default();
    schedule.add_systems((
        resolve_joints_system,
        integrate_physics_system.after(resolve_joints_system),
    ));

    // Initial error:
    // p_joint_parent = 0 + (0,0,1) = (0,0,1)
    // p_joint_child = (0,0,3) - (0,0,1) = (0,0,2)
    // distance = 1.0.

    // Run the system for 5 ticks
    for _ in 0..5 {
        schedule.run(&mut world);
    }

    let p_parent = world.get::<Position>(parent).unwrap().0;
    let p_child = world.get::<Position>(child).unwrap().0;
    let r_parent = world.get::<Rotation>(parent).unwrap().0;
    let r_child = world.get::<Rotation>(child).unwrap().0;

    let p_joint_parent = p_parent + r_parent * Vec3::new(0.0, 0.0, 1.0);
    let p_joint_child = p_child - r_child * Vec3::new(0.0, 0.0, 1.0);
    let final_error = (p_joint_parent - p_joint_child).length();

    assert!(final_error < 1.0, "Drift correction should pull segments closer; initial error 1.0, final error {}", final_error);
}

#[test]
fn test_cpg_driven_oscillation() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = World::new();
    world.insert_resource(TimeStep(0.016)); // ~60fps

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
            stiffness: 100.0, // large enough to track instantly
            damping: 10.0,
        },
        JointAxis(Vec3::Y),
        CpgOscillator::new(1.0, 0.5),
    )).id();

    let mut schedule = Schedule::default();
    schedule.add_systems((
        update_cpg_system,
        resolve_joints_system.after(update_cpg_system),
    ));

    // Run 15 ticks and verify correlation between cpg.output and child_rot angle
    for tick in 0..15 {
        schedule.run(&mut world);
        let cpg = world.get::<CpgOscillator>(child).unwrap();
        let child_rot = world.get::<Rotation>(child).unwrap().0;

        // Reconstruct angle from Quat
        let (axis, angle) = child_rot.to_axis_angle();
        let signed_angle = if axis.y < 0.0 { -angle } else { angle };

        // Verify that signed_angle is close to cpg.output
        assert!((signed_angle - cpg.output).abs() < 1e-4, "Tick {}: signed_angle ({}) should track cpg.output ({})", tick, signed_angle, cpg.output);
    }
}

#[test]
fn test_zero_allocation_hot_path() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = init_world();

    // Spawn parent and child connected by joint
    let parent = world.spawn((
        Position(Vec3::ZERO),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 0, length: 2.0, radius: 0.5, mass: 1.0 },
        RigidBody { mass: 1.0, velocity: Vec3::ZERO, force: Vec3::ZERO },
    )).id();

    let _child = world.spawn((
        Position(Vec3::new(0.0, 0.0, 2.0)),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 1, length: 2.0, radius: 0.5, mass: 1.0 },
        RigidBody { mass: 1.0, velocity: Vec3::ZERO, force: Vec3::ZERO },
        JointConstraint {
            parent_entity: parent,
            anchor_offset: Vec3::new(0.0, 0.0, 1.0),
            stiffness: 100.0,
            damping: 10.0,
        },
        JointAxis(Vec3::Y),
        CpgOscillator::new(1.0, 0.5),
    )).id();

    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems((
        update_cpg_system,
        resolve_joints_system.after(update_cpg_system),
        integrate_physics_system.after(resolve_joints_system),
        wrap_coordinates_system.after(integrate_physics_system),
        energy_decay_system.after(integrate_physics_system),
    ));

    // Warm up (allow allocations here for query initialization, etc.)
    for _ in 0..100 {
        schedule.run(&mut world);
    }

    ALLOCATOR.start_tracking();
    for _ in 0..100 {
        schedule.run(&mut world);
    }
    let allocs = ALLOCATOR.stop_tracking();

    assert_eq!(allocs, 0, "Physics hot path should perform 0 heap allocations, but made {}", allocs);
}

#[test]
fn test_damping_effect() {
    let _lock = TEST_LOCK.lock().unwrap();
    // 1. Undamped case (damping = 0.0)
    let mut world_undamped = World::new();
    world_undamped.insert_resource(TimeStep(0.01));

    let parent_u = world_undamped.spawn((
        Position(Vec3::ZERO),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 0, length: 2.0, radius: 0.5, mass: 1.0 },
        RigidBody { mass: 1e6, velocity: Vec3::ZERO, force: Vec3::ZERO }, // static parent
    )).id();

    let child_u = world_undamped.spawn((
        Position(Vec3::new(0.0, 0.0, 3.0)), // displaced by 1.0 from joint anchor (2.0)
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 1, length: 2.0, radius: 0.5, mass: 1.0 },
        RigidBody { mass: 1.0, velocity: Vec3::ZERO, force: Vec3::ZERO },
        JointConstraint {
            parent_entity: parent_u,
            anchor_offset: Vec3::new(0.0, 0.0, 1.0),
            stiffness: 100.0,
            damping: 0.0, // undamped
        },
    )).id();

    let mut schedule_u = Schedule::default();
    schedule_u.add_systems((
        resolve_joints_system,
        integrate_physics_system.after(resolve_joints_system),
    ));

    // Record displacements
    let mut max_err_undamped = 0.0;
    for i in 0..200 {
        schedule_u.run(&mut world_undamped);
        let p_parent = world_undamped.get::<Position>(parent_u).unwrap().0;
        let p_child = world_undamped.get::<Position>(child_u).unwrap().0;
        let err = (p_parent + Vec3::new(0.0, 0.0, 1.0) - (p_child - Vec3::new(0.0, 0.0, 1.0))).length();
        if i > 150 && err > max_err_undamped {
            max_err_undamped = err;
        }
    }

    // 2. Damped case (damping = 10.0)
    let mut world_damped = World::new();
    world_damped.insert_resource(TimeStep(0.01));

    let parent_d = world_damped.spawn((
        Position(Vec3::ZERO),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 0, length: 2.0, radius: 0.5, mass: 1.0 },
        RigidBody { mass: 1e6, velocity: Vec3::ZERO, force: Vec3::ZERO },
    )).id();

    let child_d = world_damped.spawn((
        Position(Vec3::new(0.0, 0.0, 3.0)),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        Segment { id: 1, length: 2.0, radius: 0.5, mass: 1.0 },
        RigidBody { mass: 1.0, velocity: Vec3::ZERO, force: Vec3::ZERO },
        JointConstraint {
            parent_entity: parent_d,
            anchor_offset: Vec3::new(0.0, 0.0, 1.0),
            stiffness: 100.0,
            damping: 10.0, // damped
        },
    )).id();

    let mut schedule_d = Schedule::default();
    schedule_d.add_systems((
        resolve_joints_system,
        integrate_physics_system.after(resolve_joints_system),
    ));

    for _ in 0..200 {
        schedule_d.run(&mut world_damped);
    }

    let p_parent = world_damped.get::<Position>(parent_d).unwrap().0;
    let p_child = world_damped.get::<Position>(child_d).unwrap().0;
    let final_err_damped = (p_parent + Vec3::new(0.0, 0.0, 1.0) - (p_child - Vec3::new(0.0, 0.0, 1.0))).length();

    // Assert that the undamped system is still oscillating (max error remains high)
    // while the damped system has settled (error is very small).
    assert!(max_err_undamped > 0.5, "Undamped system should continue to oscillate with high amplitude, got max_err={}", max_err_undamped);
    assert!(final_err_damped < 0.05, "Damped system should settle to near equilibrium, got final_err={}", final_err_damped);
}

#[test]
fn test_spatial_grid_rebuild_and_raycast() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = World::new();
    let bounds = MapBounds {
        min: Vec3::new(-100.0, 0.0, -100.0),
        max: Vec3::new(100.0, 10.0, 100.0),
    };
    world.insert_resource(bounds);

    let grid = SpatialHashGrid::new_prepopulated(10.0, &bounds);
    world.insert_resource(grid);

    let entity_a = world.spawn((
        Position(Vec3::new(10.0, 0.0, 10.0)),
        SpatialCollider { radius: 1.0 },
    )).id();

    let entity_b = world.spawn((
        Position(Vec3::new(99.0, 0.0, 0.0)),
        SpatialCollider { radius: 1.0 },
    )).id();

    let entity_c = world.spawn((
        Position(Vec3::new(-99.0, 0.0, 0.0)),
        SpatialCollider { radius: 1.0 },
    )).id();

    let mut schedule = Schedule::default();
    schedule.add_systems(rebuild_spatial_grid_system);
    schedule.run(&mut world);

    let mut system_state: bevy_ecs::system::SystemState<Query<(&Position, &SpatialCollider)>> = bevy_ecs::system::SystemState::new(&mut world);
    let query = system_state.get(&world);
    let grid = world.get_resource::<SpatialHashGrid>().unwrap();

    // 1. Normal hit
    let ray1 = Ray3D {
        origin: Vec3::new(5.0, 0.0, 10.0),
        direction: Vec3::new(1.0, 0.0, 0.0),
    };
    let hit1 = grid.raycast(&ray1, 10.0, &bounds, &query);
    assert!(hit1.is_some());
    let hit1 = hit1.unwrap();
    assert_eq!(hit1.entity, entity_a);
    assert!((hit1.distance - 4.0).abs() < 1e-3);

    // 2. Direct hit on B
    let ray2 = Ray3D {
        origin: Vec3::new(95.0, 0.0, 0.0),
        direction: Vec3::new(1.0, 0.0, 0.0),
    };
    let hit2 = grid.raycast(&ray2, 10.0, &bounds, &query);
    assert!(hit2.is_some());
    let hit2 = hit2.unwrap();
    assert_eq!(hit2.entity, entity_b);
    assert!((hit2.distance - 3.0).abs() < 1e-3);

    // 3. Wrapped hit on C crossing periodic boundary
    let ray_wrapped = Ray3D {
        origin: Vec3::new(99.0, 0.0, 0.0),
        direction: Vec3::new(1.0, 0.0, 0.0),
    };
    let hit_wrapped = grid.raycast(&ray_wrapped, 10.0, &bounds, &query);
    assert!(hit_wrapped.is_some());
    let hit_wrapped = hit_wrapped.unwrap();
    assert_eq!(hit_wrapped.entity, entity_c);
    assert!((hit_wrapped.distance - 1.0).abs() < 1e-3);
}

#[test]
fn test_spatial_grid_zero_allocation() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = World::new();
    let bounds = MapBounds {
        min: Vec3::new(-100.0, 0.0, -100.0),
        max: Vec3::new(100.0, 10.0, 100.0),
    };
    world.insert_resource(bounds);

    let grid = SpatialHashGrid::new_prepopulated(10.0, &bounds);
    world.insert_resource(grid);

    // Spawn 20 entities
    for i in 0..20 {
        world.spawn((
            Position(Vec3::new(i as f32 * 5.0 - 50.0, 0.0, i as f32 * 5.0 - 50.0)),
            SpatialCollider { radius: 1.0 },
        ));
    }

    let mut schedule = Schedule::default();
    schedule.add_systems(rebuild_spatial_grid_system);
    schedule.run(&mut world);

    let mut system_state: bevy_ecs::system::SystemState<Query<(&Position, &SpatialCollider)>> = bevy_ecs::system::SystemState::new(&mut world);
    let query = system_state.get(&world);
    let grid = world.get_resource::<SpatialHashGrid>().unwrap();
    let ray = Ray3D {
        origin: Vec3::new(0.0, 0.0, 0.0),
        direction: Vec3::new(1.0, 0.0, 1.0),
    };

    // Warm up
    let _ = grid.raycast(&ray, 100.0, &bounds, &query);

    ALLOCATOR.start_tracking();
    let _ = grid.raycast(&ray, 100.0, &bounds, &query);
    let allocs = ALLOCATOR.stop_tracking();

    assert_eq!(allocs, 0, "Raycast should perform 0 heap allocations, but made {}", allocs);
}

#[test]
fn test_decoupled_systems_zero_allocation() {
    let _lock = TEST_LOCK.lock().unwrap();
    let mut world = World::new();

    // Insert necessary resources
    let bounds = MapBounds {
        min: Vec3::new(-100.0, 0.0, -100.0),
        max: Vec3::new(100.0, 10.0, 100.0),
    };
    world.insert_resource(bounds);
    world.insert_resource(TimeStep(0.01));

    let grid = SpatialHashGrid::new_prepopulated(10.0, &bounds);
    world.insert_resource(grid);

    // Create and insert InferenceChannels
    let (req_tx, req_rx) = crossbeam_channel::unbounded::<anima_engine_lib::core::agent_systems::InferenceRequestBatch>();
    let (recycle_req_tx, recycle_req_rx) = crossbeam_channel::unbounded::<anima_engine_lib::core::agent_systems::InferenceRequestBatch>();
    let (res_tx, res_rx) = crossbeam_channel::unbounded::<anima_engine_lib::core::agent_systems::InferenceResponseBatch>();
    let (recycle_res_tx, recycle_res_rx) = crossbeam_channel::unbounded::<anima_engine_lib::core::agent_systems::InferenceResponseBatch>();

    // Pre-populate pools
    for _ in 0..8 {
        let req_batch = anima_engine_lib::core::agent_systems::InferenceRequestBatch {
            requests: Vec::with_capacity(32),
        };
        let res_batch = anima_engine_lib::core::agent_systems::InferenceResponseBatch {
            responses: Vec::with_capacity(32),
        };
        let _ = recycle_req_tx.send(req_batch);
        let _ = recycle_res_tx.send(res_batch);
    }

    let channels = anima_engine_lib::core::agent_systems::InferenceChannels {
        req_tx,
        recycle_req_rx,
        res_rx,
        recycle_res_tx,
    };
    world.insert_resource(channels);

    // Spawn agent entity
    let agent = world.spawn((
        anima_engine_lib::core::ecs::Agent,
        Position(Vec3::ZERO),
        Rotation(Quat::IDENTITY),
        Velocity(Vec3::ZERO),
        RigidBody { mass: 1.0, velocity: Vec3::ZERO, force: Vec3::ZERO },
        anima_engine_lib::ai::hrrl::HomeostaticState {
            energy: 100.0,
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        anima_engine_lib::core::ecs::CognitiveState::default(),
        anima_engine_lib::core::ecs::InertiaComponent::default(),
        anima_engine_lib::core::ecs::SensoryBufferComponent::default(),
    )).id();

    // Insert ParentAgent to identify itself
    world.entity_mut(agent).insert(anima_engine_lib::core::ecs::ParentAgent(agent));

    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems((
        anima_engine_lib::core::agent_systems::sensory_system,
        anima_engine_lib::core::agent_systems::action_resolution_system,
        integrate_physics_system,
    ));

    // Warm-up
    for _ in 0..10 {
        schedule.run(&mut world);
        // Process requests and send mock responses
        if let Ok(req_batch) = req_rx.try_recv() {
            if let Some(req) = req_batch.requests.first() {
                if let Ok(mut res_batch) = recycle_res_rx.try_recv() {
                    res_batch.responses.clear();
                    res_batch.responses.push(anima_engine_lib::core::agent_systems::AgentInferenceResponse {
                        entity: req.entity,
                        actions: [1.2, 0.5, 1.2, 0.5],
                        request_id: req.request_id,
                    });
                    let _ = res_tx.send(res_batch);
                }
            }
            let _ = recycle_req_tx.send(req_batch);
        }
    }

    // Start tracking allocations
    ALLOCATOR.start_tracking();
    
    schedule.run(&mut world);
    
    // Process requests
    if let Ok(req_batch) = req_rx.try_recv() {
        if let Some(req) = req_batch.requests.first() {
            if let Ok(mut res_batch) = recycle_res_rx.try_recv() {
                res_batch.responses.clear();
                res_batch.responses.push(anima_engine_lib::core::agent_systems::AgentInferenceResponse {
                    entity: req.entity,
                    actions: [1.2, 0.5, 1.2, 0.5],
                    request_id: req.request_id,
                });
                let _ = res_tx.send(res_batch);
            }
        }
        let _ = recycle_req_tx.send(req_batch);
    }

    let allocs = ALLOCATOR.stop_tracking();
    assert_eq!(allocs, 0, "Decoupled hot path loop should perform 0 heap allocations, but made {}", allocs);
}
