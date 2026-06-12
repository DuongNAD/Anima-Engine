mod common;

use anima_engine_lib::ai::cpg::{update_cpg_system, CpgOscillator};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecs::{
    energy_decay_system, init_world, wrap_coordinates_system, MapBounds,
    Position, Rotation, Velocity,
};
use anima_engine_lib::physics::{resolve_joints_system, integrate_physics_system, RigidBody};
use bevy_ecs::prelude::*;
use glam::Vec3;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

#[test]
fn test_ecs_simulation_loop() {
    // 1. Initialize the real Bevy ECS world and spawn 1000 actual agents
    let mut world = init_world();

    // Assert MapBounds resource is registered
    assert!(world.contains_resource::<MapBounds>());

    let mut agent_entities = Vec::with_capacity(1000);
    for i in 0..1000 {
        let pos = if i == 0 {
            Vec3::new(99.9, 0.0, 99.9)
        } else if i == 1 {
            Vec3::new(-99.9, 0.0, -99.9)
        } else {
            Vec3::new(i as f32 * 0.1, 0.0, 0.0)
        };

        let vel = if i == 0 {
            Vec3::new(12.0, 0.0, 12.0)
        } else if i == 1 {
            Vec3::new(-12.0, 0.0, -12.0)
        } else {
            Vec3::new(0.1, 0.0, 0.0)
        };

        let energy = if i == 0 { 0.05 } else { 100.0 };

        let entity = world
            .spawn((
                Position(pos),
                Rotation(glam::Quat::IDENTITY),
                Velocity(vel),
                HomeostaticState {
                    energy,
                    energy_target: 100.0,
                    hydration: 100.0,
                    hydration_target: 100.0,
                    temperature: 37.0,
                    temp_target: 37.0,
                    previous_deviation: 0.0,
                },
                CpgOscillator::new(1.0, 0.5),
                RigidBody {
                    mass: 1.0,
                    velocity: vel,
                    force: Vec3::ZERO,
                },
            ))
            .id();
        agent_entities.push(entity);
    }

    let mut schedule = Schedule::default();
    schedule.add_systems((
        update_cpg_system,
        resolve_joints_system.after(update_cpg_system),
        integrate_physics_system.after(resolve_joints_system),
        wrap_coordinates_system.after(integrate_physics_system),
        energy_decay_system.after(integrate_physics_system),
    ));

    // Warm-up phase: Run the schedule once to let Bevy compile systems, archetypes, and tables (allocations allowed here)
    schedule.run(&mut world);

    // Warm-up query: Initialize the cached QueryState and query once (allocations allowed here)
    let mut query_state = world.query::<(Entity, &Position, &Rotation, &HomeostaticState)>();
    let _ = query_state.iter(&world).count();

    // Pre-allocate buffer capacity to avoid any dynamic resizing during tracking
    let mut state_buffer = Vec::with_capacity(1000);

    // 2. Start tracking allocations
    ALLOCATOR.start_tracking();

    // 3. Execute the tick loop (schedule execution + agent querying)
    for _ in 0..10 {
        // Run systems (0 allocations expected)
        schedule.run(&mut world);

        // Query agents and extract telemetry into buffer (0 allocations expected)
        state_buffer.clear();
        for (entity, pos, rot, homeo) in query_state.iter(&world) {
            let (yaw, pitch, roll) = rot.0.to_euler(glam::EulerRot::YXZ);
            state_buffer.push(anima_engine_lib::core::engine::AgentState {
                id: entity.index(),
                x: pos.0.x,
                y: pos.0.y,
                z: pos.0.z,
                yaw,
                pitch,
                roll,
                energy: homeo.energy,
            });
        }
    }

    // 4. Stop tracking and read the counter
    let allocations = ALLOCATOR.stop_tracking();

    // 5. Assertions on correctness
    assert_eq!(
        state_buffer.len(),
        1000,
        "Should have tracked exactly 1000 agents"
    );

    // Verify movement, wrapping, and energy decay logic
    let agent_0 = agent_entities[0];
    let pos_0 = world.get::<Position>(agent_0).unwrap();
    // After 11 ticks total (1 warm-up + 10 tracked), velocity X of 12.0 moving with dt=1/60 per tick:
    // 11 * 12.0 * (1.0 / 60.0) = 2.2
    // Initial: 99.9. Target: 102.1 -> wrapped to -97.9
    assert!(
        (pos_0.0.x - (-97.9)).abs() < 1e-4,
        "Agent 0 position x wrap failed: {}",
        pos_0.0.x
    );
    assert!(
        (pos_0.0.z - (-97.9)).abs() < 1e-4,
        "Agent 0 position z wrap failed: {}",
        pos_0.0.z
    );

    // Verify energy decay and clamping for agent 0
    let homeo_0 = world.get::<HomeostaticState>(agent_0).unwrap();
    assert_eq!(
        homeo_0.energy, 0.0,
        "Agent 0 energy did not clamp to 0.0: {}",
        homeo_0.energy
    );

    // Verify position wrapping for agent 1
    let agent_1 = agent_entities[1];
    let pos_1 = world.get::<Position>(agent_1).unwrap();
    // Displacement: 11 * -12.0 * (1.0 / 60.0) = -2.2
    // Initial: -99.9. Target: -102.1 -> wrapped to 97.9
    assert!(
        (pos_1.0.x - 97.9).abs() < 1e-4,
        "Agent 1 position x wrap failed: {}",
        pos_1.0.x
    );
    assert!(
        (pos_1.0.z - 97.9).abs() < 1e-4,
        "Agent 1 position z wrap failed: {}",
        pos_1.0.z
    );

    // Assert that subsequent loops perform exactly 0 allocations
    assert_eq!(
        allocations, 0,
        "Real Bevy ECS simulation loop performed {} heap allocation(s) inside the tick loop!",
        allocations
    );
}
