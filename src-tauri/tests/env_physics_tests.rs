mod common;

use bevy_ecs::prelude::*;
use glam::{Vec3, Quat};
use std::sync::Mutex;

use anima_engine_lib::ai::pheromone::{
    PheromoneGrid, OlfactorySensors, GRID_SIZE, agent_read_pheromone_system,
};
use anima_engine_lib::core::ecs::{
    Position, Rotation, MapBounds, EnvironmentState, Cloud,
    toroidal_distance_sq, get_local_light, update_day_night_cycle_system,
    grow_flora_system, wither_flora_system, Flora,
};
use anima_engine_lib::ai::cpg::TimeStep;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_toroidal_distance() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let bounds = MapBounds {
        min: Vec3::new(-10.0, 0.0, -10.0),
        max: Vec3::new(10.0, 10.0, 10.0),
    };
    let p1 = Vec3::new(9.0, 0.0, 0.0);
    let p2 = Vec3::new(-9.0, 0.0, 0.0);
    let dist_sq = toroidal_distance_sq(p1, p2, &bounds);
    assert!((dist_sq - 4.0).abs() < 1e-5, "Expected wrapped distance squared to be 4.0, got {}", dist_sq);
}

#[test]
fn test_sine_progression_day_night_cycle() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = World::new();
    world.insert_resource(TimeStep(10.0));
    world.insert_resource(EnvironmentState {
        ambient_light: 1.0,
        elapsed_time: 0.0,
        day_length: 120.0,
    });
    
    let mut schedule = Schedule::default();
    schedule.add_systems(update_day_night_cycle_system);
    
    for _ in 0..3 {
        schedule.run(&mut world);
    }
    
    let env_state = world.resource::<EnvironmentState>();
    assert!((env_state.ambient_light - 1.0).abs() < 1e-4, "Expected light to be 1.0, got {}", env_state.ambient_light);
}

#[test]
fn test_cloud_shadow_calculations() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let bounds = MapBounds {
        min: Vec3::new(-100.0, 0.0, -100.0),
        max: Vec3::new(100.0, 10.0, 100.0),
    };
    let ambient = 1.0;
    
    let clouds = vec![
        (Vec3::new(0.0, 0.0, 0.0), 20.0, 0.8),
    ];
    let light_center = get_local_light(Vec3::new(0.0, 0.0, 0.0), ambient, &clouds, &bounds);
    assert!((light_center - 0.2).abs() < 1e-5, "Expected light 0.2, got {}", light_center);
    
    let light_outside = get_local_light(Vec3::new(30.0, 0.0, 0.0), ambient, &clouds, &bounds);
    assert!((light_outside - 1.0).abs() < 1e-5, "Expected light 1.0, got {}", light_outside);
    
    let light_half = get_local_light(Vec3::new(10.0, 0.0, 0.0), ambient, &clouds, &bounds);
    assert!((light_half - 0.6).abs() < 1e-5, "Expected light 0.6, got {}", light_half);
}

#[test]
fn test_olfactory_sensor_attenuation() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = World::new();
    let bounds = MapBounds {
        min: Vec3::new(-10.0, 0.0, -10.0),
        max: Vec3::new(10.0, 10.0, 10.0),
    };
    world.insert_resource(bounds);

    let mut grid = PheromoneGrid::new(0.0, 0.0);
    let center_idx = 64 * GRID_SIZE + 64;
    grid.values[center_idx] = 4.0;
    grid.values[center_idx + 1] = 2.0;
    world.insert_resource(grid);

    world.spawn((
        Position(Vec3::new(0.0, 0.0, 0.0)),
        Cloud {
            radius: 5.0,
            shadow_intensity: 0.5,
        },
    ));

    let yaw_90 = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let agent_entity = world.spawn((
        Position(Vec3::new(0.0, 0.0, 0.0)),
        Rotation(yaw_90),
        OlfactorySensors::new(
            Vec3::new(-0.1, 0.0, 0.0),
            Vec3::new(0.1, 0.0, 0.0),
        ),
    )).id();

    world.insert_resource(EnvironmentState {
        ambient_light: 1.0,
        elapsed_time: 0.0,
        day_length: 120.0,
    });

    let mut schedule = Schedule::default();
    schedule.add_systems(agent_read_pheromone_system);

    schedule.run(&mut world);

    let sensors = world.get::<OlfactorySensors>(agent_entity).unwrap();
    assert!(sensors.left_reading > 0.0);
    assert!(sensors.right_reading > 0.0);
}

#[test]
fn test_regression_checks() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let terrain = anima_engine_lib::core::terrain::TerrainGrid::new(0, 0);
    let bounds = MapBounds::default();
    assert_eq!(terrain.pos_to_index(Vec3::ZERO, &bounds), None);

    let mut world = World::new();
    world.insert_resource(TimeStep(1.0));
    world.insert_resource(MapBounds::default());
    world.insert_resource(anima_engine_lib::core::ecs::ActiveEnvironmentEvent::default());
    
    let mut terrain = anima_engine_lib::core::terrain::TerrainGrid::new(3, 3);
    terrain.cells[4].biome = anima_engine_lib::core::terrain::BiomeType::Grassland;
    world.insert_resource(terrain);

    let flora_ent = world.spawn((
        Flora {
            energy: 10.0,
            max_energy: -5.0,
            growth_rate: 1.0,
            decay_rate: 0.1,
        },
        Position(Vec3::ZERO),
    )).id();

    let mut grow_schedule = Schedule::default();
    grow_schedule.add_systems(grow_flora_system);
    grow_schedule.run(&mut world);
    let flora = world.get::<Flora>(flora_ent).unwrap();
    assert_eq!(flora.energy, 0.0);

    let flora_nan = world.spawn((
        Flora {
            energy: f32::NAN,
            max_energy: 100.0,
            growth_rate: 1.0,
            decay_rate: 0.1,
        },
        Position(Vec3::ZERO),
    )).id();

    let mut wither_schedule = Schedule::default();
    wither_schedule.add_systems(wither_flora_system);
    wither_schedule.run(&mut world);
    assert!(world.get::<Flora>(flora_nan).is_none());
}

#[test]
fn test_environment_zero_allocations_on_hot_path() {
    use anima_engine_lib::core::ecs::{Velocity, update_positions_system, wrap_coordinates_system, Agent};
    use anima_engine_lib::ai::model::{BrainModel, BrainInferenceBuffer};
    use anima_engine_lib::physics::SpatialHashGrid;
    use anima_engine_lib::ai::hrrl::HomeostaticState;

    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = World::new();
    let bounds = MapBounds::default();
    world.insert_resource(bounds);
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(EnvironmentState::default());
    world.insert_resource(PheromoneGrid::default());
    world.insert_resource(BrainModel::new(15, 64, 4));
    world.insert_resource(BrainInferenceBuffer::default());

    let grid = SpatialHashGrid::new_prepopulated(10.0, &bounds);
    world.insert_resource(grid);

    // Spawn clouds with velocity (cloud movement)
    for i in 0..5 {
        world.spawn((
            Position(Vec3::new(i as f32 * 10.0, 0.0, 0.0)),
            Velocity(Vec3::new(1.0, 0.0, 1.0)),
            Cloud { radius: 20.0, shadow_intensity: 0.8 },
        ));
    }

    // Spawn agents with olfactory sensors (olfactory sensor attenuation)
    for i in 0..5 {
        world.spawn((
            Position(Vec3::new(i as f32 * 5.0, 0.0, 0.0)),
            Rotation(Quat::IDENTITY),
            OlfactorySensors::new(
                Vec3::new(-0.1, 0.0, 0.0),
                Vec3::new(0.1, 0.0, 0.0),
            ),
        ));
    }

    // Spawn agents with brain models (raycast sensor attenuation)
    for i in 0..5 {
        world.spawn((
            Agent,
            Position(Vec3::new(i as f32 * -5.0, 0.0, 0.0)),
            Rotation(Quat::IDENTITY),
            HomeostaticState {
                energy: 80.0,
                energy_target: 100.0,
                hydration: 70.0,
                hydration_target: 100.0,
                temperature: 36.5,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
            anima_engine_lib::ai::hrrl::LastTransitionState {
                state: [0.0; 15],
                action: [0.0; 4],
                has_last: false,
            },
        ));
    }

    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems((
        update_day_night_cycle_system,
        update_positions_system,
        wrap_coordinates_system,
        agent_read_pheromone_system,
    ));

    // Warm up the systems and queries to avoid initial capacity allocations
    for _ in 0..100 {
        schedule.run(&mut world);
    }

    ALLOCATOR.start_tracking();

    for _ in 0..10 {
        schedule.run(&mut world);
    }

    let allocations = ALLOCATOR.stop_tracking();
    assert_eq!(
        allocations, 0,
        "Environment systems performed {} heap allocation(s) inside tick loop!",
        allocations
    );
}

#[test]
fn test_raycast_sensor_attenuation() {
    use anima_engine_lib::core::ecs::{Agent, Food, ParentAgent, Segment};
    use anima_engine_lib::ai::model::{BrainModel, BrainInferenceBuffer, brain_inference_system};
    use anima_engine_lib::ai::cpg::CpgOscillator;
    use anima_engine_lib::ai::hrrl::HomeostaticState;

    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = World::new();

    // Register resources
    world.insert_resource(BrainModel::new(15, 64, 4));
    world.insert_resource(BrainInferenceBuffer::default());
    let bounds = MapBounds {
        min: Vec3::new(-100.0, 0.0, -100.0),
        max: Vec3::new(100.0, 10.0, 100.0),
    };
    world.insert_resource(bounds);

    // Spatial Hash Grid setup
    let grid = anima_engine_lib::physics::SpatialHashGrid::new_prepopulated(10.0, &bounds);
    world.insert_resource(grid);

    // Spawn an agent
    let agent_entity = world.spawn((
        Agent,
        Position(Vec3::new(0.0, 0.0, 0.0)),
        Rotation(Quat::IDENTITY), // Facing +Z (0,0,1)
        HomeostaticState {
            energy: 80.0,
            energy_target: 100.0,
            hydration: 70.0,
            hydration_target: 100.0,
            temperature: 36.5,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        anima_engine_lib::ai::hrrl::LastTransitionState {
            state: [0.0; 15],
            action: [0.0; 4],
            has_last: false,
        },
    )).id();

    // Spawn a child segment with CpgOscillator
    world.spawn((
        ParentAgent(agent_entity),
        Segment { id: 0, length: 1.0, radius: 0.1, mass: 1.0 },
        CpgOscillator::new(1.0, 0.5),
    ));

    // Spawn food at Z = 7.0 (facing agent)
    let food_entity = world.spawn((
        Food { energy_value: 30.0, hydration_value: 20.0 },
        Position(Vec3::new(0.0, 0.0, 7.0)),
        anima_engine_lib::physics::SpatialCollider { radius: 1.0 },
    )).id();

    // Rebuild spatial grid so food is in the grid
    let mut rebuild_schedule = Schedule::default();
    rebuild_schedule.add_systems(anima_engine_lib::physics::rebuild_spatial_grid_system);
    rebuild_schedule.run(&mut world);

    // Scenario 1: Bright light (ambient_light = 1.0, no clouds)
    world.insert_resource(EnvironmentState {
        ambient_light: 1.0,
        elapsed_time: 0.0,
        day_length: 120.0,
    });

    let mut inference_schedule = Schedule::default();
    inference_schedule.add_systems(brain_inference_system);
    inference_schedule.run(&mut world);

    {
        let buffer = world.get_resource::<BrainInferenceBuffer>().unwrap();
        // Since food is at Z=6.0 and range is 10.0, it should be detected
        // check agent_states inputs
        let state = buffer.agent_states[0];
        let hit_distance = state[9];
        let hit_is_food = state[10];
        assert!(hit_distance < 10.0, "Expected hit_distance < 10.0, got {}", hit_distance);
        assert!((hit_is_food - 1.0).abs() < 1e-5, "Expected hit_is_food = 1.0, got {}", hit_is_food);
    }

    // Reset buffer
    world.insert_resource(BrainInferenceBuffer::default());

    // Scenario 2: Dim light (ambient_light = 0.5, no clouds)
    // effective range is 10.0 * 0.5 = 5.0. Food at 6.0 should NOT be detected
    world.insert_resource(EnvironmentState {
        ambient_light: 0.5,
        elapsed_time: 0.0,
        day_length: 120.0,
    });

    inference_schedule.run(&mut world);

    {
        let buffer = world.get_resource::<BrainInferenceBuffer>().unwrap();
        let state = buffer.agent_states[0];
        let hit_distance = state[9];
        let hit_is_food = state[10];
        assert_eq!(hit_distance, 10.0, "Expected hit_distance to default to 10.0 (no hit), got {}", hit_distance);
        assert_eq!(hit_is_food, 0.0, "Expected hit_is_food to be 0.0, got {}", hit_is_food);
    }

    // Reset buffer
    world.insert_resource(BrainInferenceBuffer::default());

    // Scenario 3: Food is very close (Z = 3.0), which is within effective range (5.0),
    // but the classification confidence should be attenuated to 0.5.
    world.entity_mut(food_entity).insert(Position(Vec3::new(0.0, 0.0, 3.0)));
    rebuild_schedule.run(&mut world);
    
    inference_schedule.run(&mut world);

    {
        let buffer = world.get_resource::<BrainInferenceBuffer>().unwrap();
        let state = buffer.agent_states[0];
        let hit_distance = state[9];
        let hit_is_food = state[10];
        assert!(hit_distance < 5.0);
        assert!((hit_is_food - 0.5).abs() < 1e-5, "Expected hit_is_food to be attenuated to 0.5, got {}", hit_is_food);
    }
}

#[test]
fn test_stress_toroidal_wrapping_edge_cases() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    
    // Bounds: X in [-10, 10], Z in [-10, 10]
    let bounds = MapBounds {
        min: Vec3::new(-10.0, 0.0, -10.0),
        max: Vec3::new(10.0, 10.0, 10.0),
    };

    // 1. Overlapping clouds:
    // Cloud A at (0, 0, 0), radius 5.0, intensity 0.6
    // Cloud B at (3, 0, 0), radius 4.0, intensity 0.8
    let clouds = vec![
        (Vec3::new(0.0, 0.0, 0.0), 5.0, 0.6),
        (Vec3::new(3.0, 0.0, 0.0), 4.0, 0.8),
    ];
    
    // Test point at (1.5, 0.0, 0.0).
    // Dist to A = 1.5. Shadow A = 0.6 * (1.0 - 1.5/5.0) = 0.6 * 0.7 = 0.42.
    // Dist to B = 1.5. Shadow B = 0.8 * (1.0 - 1.5/4.0) = 0.8 * 0.625 = 0.5.
    // get_local_light should use max_shadow = max(0.42, 0.5) = 0.5.
    // With ambient light = 1.0, expected light = 1.0 * (1.0 - 0.5) = 0.5.
    let light_val = get_local_light(Vec3::new(1.5, 0.0, 0.0), 1.0, &clouds, &bounds);
    assert!((light_val - 0.5).abs() < 1e-5, "Expected light 0.5, got {}", light_val);

    // 2. Cloud coordinate wrap-around at bounds borders (diagonal):
    // Cloud C at (9.5, 0.0, 9.5), radius 2.0, intensity 0.8.
    // Agent at (-9.5, 0.0, -9.5).
    // Toroidal distance in X: dx = 20.0 - (9.5 - (-9.5)) = 20.0 - 19.0 = 1.0.
    // Toroidal distance in Z: dz = 20.0 - (9.5 - (-9.5)) = 20.0 - 19.0 = 1.0.
    // Distance squared = 1.0 + 1.0 = 2.0.
    // Distance = sqrt(2.0) ≈ 1.4142135.
    // Shadow = 0.8 * (1.0 - 1.4142135 / 2.0) ≈ 0.8 * (1.0 - 0.7071068) = 0.8 * 0.2928932 = 0.2343145.
    // Expected light = 1.0 * (1.0 - 0.2343145) = 0.7656854.
    let clouds_wrap = vec![
        (Vec3::new(9.5, 0.0, 9.5), 2.0, 0.8),
    ];
    let light_wrap = get_local_light(Vec3::new(-9.5, 0.0, -9.5), 1.0, &clouds_wrap, &bounds);
    assert!((light_wrap - 0.7656854).abs() < 1e-5, "Expected light 0.7656854, got {}", light_wrap);

    // 3. Exact bounds border wrapping (cloud at max, target at min):
    // Cloud at (10.0, 0.0, 0.0), radius 5.0, intensity 0.5.
    // Target at (-10.0, 0.0, 0.0).
    // Toroidal distance in X: dx = 20.0 - 20.0 = 0.0.
    // Dist = 0.0. Shadow = 0.5.
    // Expected light = 0.5.
    let clouds_edge = vec![
        (Vec3::new(10.0, 0.0, 0.0), 5.0, 0.5),
    ];
    let light_edge = get_local_light(Vec3::new(-10.0, 0.0, 0.0), 1.0, &clouds_edge, &bounds);
    assert!((light_edge - 0.5).abs() < 1e-5, "Expected light 0.5, got {}", light_edge);
}

#[test]
fn test_stress_sensor_attenuation_extreme_lighting() {
    use anima_engine_lib::core::ecs::{Agent, Food, ParentAgent, Segment, Predator};
    use anima_engine_lib::ai::model::{BrainModel, BrainInferenceBuffer, brain_inference_system};
    use anima_engine_lib::ai::cpg::CpgOscillator;
    use anima_engine_lib::ai::hrrl::HomeostaticState;

    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut world = World::new();

    // Register resources
    world.insert_resource(BrainModel::new(15, 64, 4));
    world.insert_resource(BrainInferenceBuffer::default());
    
    // Bounds X in [-10, 10], Z in [-10, 10]
    let bounds = MapBounds {
        min: Vec3::new(-10.0, 0.0, -10.0),
        max: Vec3::new(10.0, 10.0, 10.0),
    };
    world.insert_resource(bounds);

    // Spatial Hash Grid setup
    let grid = anima_engine_lib::physics::SpatialHashGrid::new_prepopulated(5.0, &bounds);
    world.insert_resource(grid);

    // 1. Near-zero ambient light test on get_local_light
    // Ambient = 0.0
    let light_0 = get_local_light(Vec3::ZERO, 0.0, &[], &bounds);
    assert_eq!(light_0, 0.05, "Expected light clamped to min 0.05, got {}", light_0);
    
    // Ambient = 0.01
    let light_01 = get_local_light(Vec3::ZERO, 0.01, &[], &bounds);
    assert_eq!(light_01, 0.05, "Expected light clamped to min 0.05, got {}", light_01);
    
    // Ambient = 0.06
    let light_06 = get_local_light(Vec3::ZERO, 0.06, &[], &bounds);
    assert!((light_06 - 0.06).abs() < 1e-5, "Expected light 0.06, got {}", light_06);

    // 2. Intense cloud shadows test on get_local_light
    // Ambient = 1.0, Cloud shadow intensity = 1.5 (heavy shadow)
    let clouds_heavy = vec![(Vec3::ZERO, 5.0, 1.5)];
    let light_shadow_heavy = get_local_light(Vec3::ZERO, 1.0, &clouds_heavy, &bounds);
    assert_eq!(light_shadow_heavy, 0.05, "Expected light under intense shadow clamped to min 0.05, got {}", light_shadow_heavy);

    // Spawn an agent at origin
    let agent_entity = world.spawn((
        Agent,
        Position(Vec3::new(0.0, 0.0, 0.0)),
        Rotation(Quat::IDENTITY), // Facing +Z (0,0,1)
        HomeostaticState {
            energy: 80.0,
            energy_target: 100.0,
            hydration: 70.0,
            hydration_target: 100.0,
            temperature: 36.5,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
        anima_engine_lib::ai::hrrl::LastTransitionState {
            state: [0.0; 15],
            action: [0.0; 4],
            has_last: false,
        },
    )).id();

    // Spawn a child segment for agent
    world.spawn((
        ParentAgent(agent_entity),
        Segment { id: 0, length: 1.0, radius: 0.1, mass: 1.0 },
        CpgOscillator::new(1.0, 0.5),
    ));

    // Spawn a Food entity at (0, 0, 1.2) with radius 0.5
    // This gives a hit distance of 0.7, which is > 0.5 (effective range at 0.05 ambient light)
    let food_entity = world.spawn((
        Food { energy_value: 30.0, hydration_value: 20.0 },
        Position(Vec3::new(0.0, 0.0, 1.2)),
        anima_engine_lib::physics::SpatialCollider { radius: 0.5 },
    )).id();

    // Rebuild grid.
    let mut rebuild_schedule = Schedule::default();
    rebuild_schedule.add_systems(anima_engine_lib::physics::rebuild_spatial_grid_system);
    rebuild_schedule.run(&mut world);

    // Schedule for inference
    let mut inference_schedule = Schedule::default();
    inference_schedule.add_systems(brain_inference_system);

    // Scenario A: Ambient light = 0.0 (clamped to 0.05)
    // Sensor range is 10.0 * 0.05 = 0.5. Food is at Z=1.0, radius=0.3, hit distance is 0.7, so it should NOT be detected!
    world.insert_resource(EnvironmentState {
        ambient_light: 0.0,
        elapsed_time: 0.0,
        day_length: 120.0,
    });
    inference_schedule.run(&mut world);
    {
        let buffer = world.get_resource::<BrainInferenceBuffer>().unwrap();
        let state = buffer.agent_states[0];
        let hit_distance = state[9];
        let hit_is_food = state[10];
        assert_eq!(hit_distance, 10.0, "Food should be out of range (0.5)");
        assert_eq!(hit_is_food, 0.0, "Food confidence should be 0.0");
    }

    // Reset buffer
    world.insert_resource(BrainInferenceBuffer::default());

    // Scenario B: Food is moved closer to Z = 0.65. Under ambient_light = 0.0 (clamped to 0.05),
    // range is 0.5. Food hit distance is 0.65 - 0.3 = 0.35, which is within range (0.5).
    // Hit is food classification confidence should be 1.0 * 0.05 = 0.05.
    world.entity_mut(food_entity).insert(Position(Vec3::new(0.0, 0.0, 0.65)));
    rebuild_schedule.run(&mut world);
    inference_schedule.run(&mut world);
    {
        let buffer = world.get_resource::<BrainInferenceBuffer>().unwrap();
        let state = buffer.agent_states[0];
        let hit_distance = state[9];
        let hit_is_food = state[10];
        assert!(hit_distance < 0.5, "Expected hit_distance < 0.5, got {}", hit_distance);
        assert!((hit_is_food - 0.05).abs() < 1e-5, "Expected hit_is_food to be exactly 0.05, got {}", hit_is_food);
    }

    // Reset buffer and remove food
    world.insert_resource(BrainInferenceBuffer::default());
    world.entity_mut(food_entity).despawn();

    // Scenario C: Spawn a Predator agent at Z = 0.4.
    // Set ambient light to 0.1 (effective range 1.0).
    // The agent should detect the Predator with classification confidence 0.1.
    let predator_agent = world.spawn((
        Agent,
        Predator,
        Position(Vec3::new(0.0, 0.0, 0.4)),
        Rotation(Quat::IDENTITY),
        anima_engine_lib::physics::SpatialCollider { radius: 0.1 },
        HomeostaticState {
            energy: 80.0,
            energy_target: 100.0,
            hydration: 70.0,
            hydration_target: 100.0,
            temperature: 36.5,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
    )).id();
    
    // Spawn predator segment
    world.spawn((
        ParentAgent(predator_agent),
        Segment { id: 0, length: 1.0, radius: 0.1, mass: 1.0 },
        CpgOscillator::new(1.0, 0.5),
    ));

    rebuild_schedule.run(&mut world);
    world.insert_resource(EnvironmentState {
        ambient_light: 0.1,
        elapsed_time: 0.0,
        day_length: 120.0,
    });
    inference_schedule.run(&mut world);
    {
        let buffer = world.get_resource::<BrainInferenceBuffer>().unwrap();
        // The first agent is our subject, the second is the predator
        let agent_idx = buffer.agent_entities.iter().position(|&e| e == agent_entity).unwrap();
        let state = buffer.agent_states[agent_idx];
        let hit_distance = state[9];
        let hit_is_predator = state[11];
        assert!(hit_distance < 1.0);
        assert!((hit_is_predator - 0.1).abs() < 1e-5, "Expected hit_is_predator to be 0.1, got {}", hit_is_predator);
    }
}


