mod common;

use bevy_ecs::prelude::*;
use glam::Vec3;
use anima_engine_lib::core::ecs::{
    Position, Velocity, Rotation, MapBounds, init_world,
    ParentAgent, SegmentJointForce, metabolic_decay_system, Segment,
};
use anima_engine_lib::ai::cpg::{CpgOscillator, TimeStep, update_cpg_system};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::physics::dynamics::{RigidBody, JointConstraint, resolve_joints_system, integrate_physics_system};
use anima_engine_lib::evolution::genotype::{MorphologyGenotype, MorphologyNode, MorphologyEdge};

// Bind to tracking allocator
#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator = common::allocator::TrackingAllocator::new();

// Decode Genotype into ECS Entities
fn decode_genotype_to_ecs(world: &mut World, genotype: &MorphologyGenotype) -> Entity {
    // 1. Spawn root agent entity
    let agent_entity = world.spawn((
        HomeostaticState {
            energy: 100.0,
            energy_target: 100.0,
            hydration: 100.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
        },
    )).id();

    // Map genotype node IDs to Bevy entity IDs
    let mut node_to_entity = std::collections::HashMap::new();

    // 2. Spawn segments for each MorphologyNode
    for node in &genotype.nodes {
        let seg_entity = world.spawn((
            ParentAgent(agent_entity),
            Position(Vec3::new(0.0, 0.0, 0.0)),
            Rotation(glam::Quat::IDENTITY),
            Velocity(Vec3::ZERO),
            RigidBody {
                mass: node.mass,
                velocity: Vec3::ZERO,
                force: Vec3::ZERO,
            },
            Segment {
                id: node.id,
                length: node.length,
                radius: node.radius,
                mass: node.mass,
            },
            CpgOscillator::new(1.0, 0.5),
            SegmentJointForce(0.0),
        )).id();
        node_to_entity.insert(node.id, seg_entity);
    }

    // 3. Setup JointConstraints for each MorphologyEdge
    for edge in &genotype.edges {
        if let (Some(&src), Some(&tgt)) = (node_to_entity.get(&edge.source_node), node_to_entity.get(&edge.target_node)) {
            world.entity_mut(tgt).insert(JointConstraint {
                parent_entity: src,
                anchor_offset: edge.joint_anchor,
                stiffness: 100.0,
                damping: 10.0,
            });
        }
    }

    agent_entity
}

// Drive Joints System from CPG
fn drive_joints_system(
    mut query: Query<(&CpgOscillator, &mut RigidBody, &mut SegmentJointForce)>,
) {
    for (cpg, mut body, mut joint_force) in query.iter_mut() {
        // CPG outputs control forces applied to segments
        let force_mag = cpg.output * 10.0;
        body.force += Vec3::new(0.0, force_mag, 0.0);
        joint_force.0 = force_mag.abs();
    }
}

#[test]
fn test_morphological_evolution_hot_path() {
    let mut world = init_world();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(MapBounds::default());

    // Create a 3-segment linear genotype
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.2, mass: 1.5 });
    genotype.add_node(MorphologyNode { id: 1, length: 1.0, radius: 0.2, mass: 1.0 });
    genotype.add_node(MorphologyNode { id: 2, length: 1.0, radius: 0.2, mass: 0.8 });
    
    genotype.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 1,
        joint_anchor: Vec3::new(1.0, 0.0, 0.0),
        joint_axis: Vec3::new(0.0, 0.0, 1.0),
    });
    genotype.add_edge(MorphologyEdge {
        source_node: 1,
        target_node: 2,
        joint_anchor: Vec3::new(1.0, 0.0, 0.0),
        joint_axis: Vec3::new(0.0, 0.0, 1.0),
    });

    let agent_entity = decode_genotype_to_ecs(&mut world, &genotype);

    let mut schedule = Schedule::default();
    schedule.add_systems((
        update_cpg_system,
        drive_joints_system.after(update_cpg_system),
        resolve_joints_system.after(drive_joints_system),
        integrate_physics_system.after(resolve_joints_system),
        metabolic_decay_system.after(integrate_physics_system),
    ));

    // Warm-up run to compile Bevy tables and system states (allows initial heap allocations)
    schedule.run(&mut world);
    
    // Warm-up queries
    let mut query_state = world.query::<(Entity, &ParentAgent, &RigidBody, &Velocity, &SegmentJointForce)>();
    let _ = query_state.iter(&world).count();

    // Start tracking memory allocations
    ALLOCATOR.start_tracking();

    // Run active tick loop
    for _ in 0..20 {
        schedule.run(&mut world);
    }

    // Stop tracking
    let allocations = ALLOCATOR.stop_tracking();

    // Assert zero heap allocations in the hot path
    assert_eq!(
        allocations, 0,
        "Active tick loop produced {} heap allocation(s)!",
        allocations
    );

    // Verify energy depletion logic
    let homeo = world.get::<HomeostaticState>(agent_entity).unwrap();
    assert!(homeo.energy < 100.0, "Energy did not decay: {}", homeo.energy);
    
    // Verify CPG driven forces and physical movement
    let mut checked_forces = false;
    for (_entity, _parent, _body, vel, joint_force) in query_state.iter(&world) {
        assert!(joint_force.0 >= 0.0);
        assert!(vel.0 != Vec3::ZERO, "Physics did not integrate force into velocity / physical movement");
        checked_forces = true;
    }
    assert!(checked_forces, "No segment forces checked");
}
