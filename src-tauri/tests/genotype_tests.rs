use anima_engine_lib::ai::cpg::CpgOscillator;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::ecs::{
    Agent, ChildrenLinks, JointAxis, ParentLink, Position, Rotation, Segment, Velocity,
};
use anima_engine_lib::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};
use anima_engine_lib::physics::dynamics::{JointConstraint, RigidBody};
use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};

#[test]
fn test_single_node_decoding() {
    let mut world = World::new();
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode {
        id: 1,
        length: 2.0,
        radius: 0.5,
        mass: 10.0,
    });

    let initial_pos = Vec3::new(1.0, 2.0, 3.0);
    let initial_rot = Quat::IDENTITY;

    let root_entity = decode_genotype(&mut world, &genotype, initial_pos, initial_rot);

    // Verify root entity components
    assert!(world.entity(root_entity).contains::<Agent>());

    let pos = world
        .get::<Position>(root_entity)
        .expect("Should have Position");
    assert_eq!(pos.0, initial_pos);

    let rot = world
        .get::<Rotation>(root_entity)
        .expect("Should have Rotation");
    assert_eq!(rot.0, initial_rot);

    let vel = world
        .get::<Velocity>(root_entity)
        .expect("Should have Velocity");
    assert_eq!(vel.0, Vec3::ZERO);

    let rb = world
        .get::<RigidBody>(root_entity)
        .expect("Should have RigidBody");
    assert_eq!(rb.mass, 10.0);
    assert_eq!(rb.velocity, Vec3::ZERO);
    assert_eq!(rb.force, Vec3::ZERO);

    let seg = world
        .get::<Segment>(root_entity)
        .expect("Should have Segment");
    assert_eq!(seg.id, 1);
    assert_eq!(seg.length, 2.0);
    assert_eq!(seg.radius, 0.5);
    assert_eq!(seg.mass, 10.0);

    let homeo = world
        .get::<HomeostaticState>(root_entity)
        .expect("Should have HomeostaticState");
    assert_eq!(homeo.energy, 100.0);

    let children = world
        .get::<ChildrenLinks>(root_entity)
        .expect("Should have ChildrenLinks");
    assert!(children.0.is_empty());

    // Verify only 1 entity spawned in total
    let total_entities = world.query::<&Segment>().iter(&world).count();
    assert_eq!(total_entities, 1);
}

#[test]
fn test_linear_chain_decoding() {
    let mut world = World::new();
    let mut genotype = MorphologyGenotype::new();

    // Node 1 (Root)
    genotype.add_node(MorphologyNode {
        id: 1,
        length: 2.0,
        radius: 0.5,
        mass: 10.0,
    });
    // Node 2 (Child)
    genotype.add_node(MorphologyNode {
        id: 2,
        length: 4.0,
        radius: 0.3,
        mass: 5.0,
    });

    // Edge Node 1 -> Node 2
    genotype.add_edge(MorphologyEdge {
        source_node: 1,
        target_node: 2,
        joint_anchor: Vec3::new(0.0, 0.0, 1.0),
        joint_axis: Vec3::new(0.0, 1.0, 0.0),
    });

    let initial_pos = Vec3::ZERO;
    let initial_rot = Quat::IDENTITY;

    let root_entity = decode_genotype(&mut world, &genotype, initial_pos, initial_rot);

    // Verify root children links
    let root_children = world.get::<ChildrenLinks>(root_entity).unwrap();
    assert_eq!(root_children.0.len(), 1);
    let child_entity = root_children.0[0];

    // Verify child components
    assert!(!world.entity(child_entity).contains::<Agent>());
    assert!(!world.entity(child_entity).contains::<HomeostaticState>());

    let child_pos = world.get::<Position>(child_entity).unwrap();
    // P_child = P_parent + R_parent * joint_anchor + R_child * (0.0, 0.0, length_child / 2.0)
    // P_child = 0.0 + 1.0 * (0.0, 0.0, 1.0) + 1.0 * (0.0, 0.0, 4.0 / 2.0) = (0.0, 0.0, 3.0)
    assert_eq!(child_pos.0, Vec3::new(0.0, 0.0, 3.0));

    let child_rot = world.get::<Rotation>(child_entity).unwrap();
    assert_eq!(child_rot.0, Quat::IDENTITY);

    let child_rb = world.get::<RigidBody>(child_entity).unwrap();
    assert_eq!(child_rb.mass, 5.0);

    let child_seg = world.get::<Segment>(child_entity).unwrap();
    assert_eq!(child_seg.id, 2);
    assert_eq!(child_seg.length, 4.0);

    let parent_link = world
        .get::<ParentLink>(child_entity)
        .expect("Child must have ParentLink");
    assert_eq!(parent_link.0, root_entity);

    let constraint = world
        .get::<JointConstraint>(child_entity)
        .expect("Child must have JointConstraint");
    assert_eq!(constraint.parent_entity, root_entity);
    assert_eq!(constraint.anchor_offset, Vec3::new(0.0, 0.0, 1.0));
    assert_eq!(constraint.stiffness, 10.0);
    assert_eq!(constraint.damping, 1.0);

    let axis = world
        .get::<JointAxis>(child_entity)
        .expect("Child must have JointAxis");
    assert_eq!(axis.0, Vec3::new(0.0, 1.0, 0.0));

    let osc = world
        .get::<CpgOscillator>(child_entity)
        .expect("Child must have CpgOscillator");
    assert_eq!(osc.frequency, 1.0);
    assert_eq!(osc.amplitude, 0.5);

    let total_entities = world.query::<&Segment>().iter(&world).count();
    assert_eq!(total_entities, 2);
}

#[test]
fn test_branching_decoding() {
    let mut world = World::new();
    let mut genotype = MorphologyGenotype::new();

    // Node 1 (Root), Node 2 (Left child), Node 3 (Right child)
    genotype.add_node(MorphologyNode {
        id: 1,
        length: 2.0,
        radius: 0.5,
        mass: 10.0,
    });
    genotype.add_node(MorphologyNode {
        id: 2,
        length: 2.0,
        radius: 0.5,
        mass: 5.0,
    });
    genotype.add_node(MorphologyNode {
        id: 3,
        length: 2.0,
        radius: 0.5,
        mass: 5.0,
    });

    genotype.add_edge(MorphologyEdge {
        source_node: 1,
        target_node: 2,
        joint_anchor: Vec3::new(-0.5, 0.0, 0.0),
        joint_axis: Vec3::new(1.0, 0.0, 0.0),
    });
    genotype.add_edge(MorphologyEdge {
        source_node: 1,
        target_node: 3,
        joint_anchor: Vec3::new(0.5, 0.0, 0.0),
        joint_axis: Vec3::new(1.0, 0.0, 0.0),
    });

    let root_entity = decode_genotype(&mut world, &genotype, Vec3::ZERO, Quat::IDENTITY);

    let root_children = world.get::<ChildrenLinks>(root_entity).unwrap();
    assert_eq!(root_children.0.len(), 2);

    let c1 = root_children.0[0];
    let c2 = root_children.0[1];

    let seg1 = world.get::<Segment>(c1).unwrap();
    let _seg2 = world.get::<Segment>(c2).unwrap();

    let (left_child, right_child) = if seg1.id == 2 { (c1, c2) } else { (c2, c1) };

    assert_eq!(world.get::<Segment>(left_child).unwrap().id, 2);
    assert_eq!(world.get::<Segment>(right_child).unwrap().id, 3);

    assert_eq!(world.get::<ParentLink>(left_child).unwrap().0, root_entity);
    assert_eq!(world.get::<ParentLink>(right_child).unwrap().0, root_entity);
}

#[test]
fn test_cycle_prevention() {
    let mut world = World::new();
    let mut genotype = MorphologyGenotype::new();

    // Cycle: 1 -> 2 -> 3 -> 1
    genotype.add_node(MorphologyNode {
        id: 1,
        length: 2.0,
        radius: 0.5,
        mass: 10.0,
    });
    genotype.add_node(MorphologyNode {
        id: 2,
        length: 2.0,
        radius: 0.5,
        mass: 5.0,
    });
    genotype.add_node(MorphologyNode {
        id: 3,
        length: 2.0,
        radius: 0.5,
        mass: 5.0,
    });

    genotype.add_edge(MorphologyEdge {
        source_node: 1,
        target_node: 2,
        joint_anchor: Vec3::new(0.0, 0.0, 1.0),
        joint_axis: Vec3::Y,
    });
    genotype.add_edge(MorphologyEdge {
        source_node: 2,
        target_node: 3,
        joint_anchor: Vec3::new(0.0, 0.0, 1.0),
        joint_axis: Vec3::Y,
    });
    genotype.add_edge(MorphologyEdge {
        source_node: 3,
        target_node: 1,
        joint_anchor: Vec3::new(0.0, 0.0, 1.0),
        joint_axis: Vec3::Y,
    });
    // Also add a duplicate edge 3 -> 2
    genotype.add_edge(MorphologyEdge {
        source_node: 3,
        target_node: 2,
        joint_anchor: Vec3::new(0.0, 0.0, 1.0),
        joint_axis: Vec3::Y,
    });

    let root_entity = decode_genotype(&mut world, &genotype, Vec3::ZERO, Quat::IDENTITY);

    // BFS should visit each node exactly once.
    let total_entities = world.query::<&Segment>().iter(&world).count();
    assert_eq!(total_entities, 3);

    // Let's verify root is 1, child of root is 2, child of 2 is 3, and 3 has no children spawned
    let root_id = world.get::<Segment>(root_entity).unwrap().id;
    assert_eq!(root_id, 1);

    let root_children = world.get::<ChildrenLinks>(root_entity).unwrap();
    assert_eq!(root_children.0.len(), 1);
    let node_2_entity = root_children.0[0];
    assert_eq!(world.get::<Segment>(node_2_entity).unwrap().id, 2);

    let node_2_children = world.get::<ChildrenLinks>(node_2_entity).unwrap();
    assert_eq!(node_2_children.0.len(), 1);
    let node_3_entity = node_2_children.0[0];
    assert_eq!(world.get::<Segment>(node_3_entity).unwrap().id, 3);

    let node_3_children = world.get::<ChildrenLinks>(node_3_entity).unwrap();
    assert!(
        node_3_children.0.is_empty(),
        "Node 3 should have no children links because of cycle prevention"
    );
}

#[test]
fn test_unreachable_nodes() {
    let mut world = World::new();
    let mut genotype = MorphologyGenotype::new();

    // Node 1 (Root), Node 2 (Connected), Node 3 (Unconnected)
    genotype.add_node(MorphologyNode {
        id: 1,
        length: 2.0,
        radius: 0.5,
        mass: 10.0,
    });
    genotype.add_node(MorphologyNode {
        id: 2,
        length: 2.0,
        radius: 0.5,
        mass: 5.0,
    });
    genotype.add_node(MorphologyNode {
        id: 3,
        length: 2.0,
        radius: 0.5,
        mass: 5.0,
    });

    genotype.add_edge(MorphologyEdge {
        source_node: 1,
        target_node: 2,
        joint_anchor: Vec3::new(0.0, 0.0, 1.0),
        joint_axis: Vec3::Y,
    });

    let root_entity = decode_genotype(&mut world, &genotype, Vec3::ZERO, Quat::IDENTITY);

    let total_entities = world.query::<&Segment>().iter(&world).count();
    assert_eq!(total_entities, 2, "Unreachable nodes must not be spawned");

    let root_children = world.get::<ChildrenLinks>(root_entity).unwrap();
    assert_eq!(root_children.0.len(), 1);
}

#[test]
fn test_kinematic_placement() {
    let mut world = World::new();
    let mut genotype = MorphologyGenotype::new();

    // Node 1 (Root), Node 2 (Child)
    genotype.add_node(MorphologyNode {
        id: 1,
        length: 2.0,
        radius: 0.5,
        mass: 10.0,
    });
    genotype.add_node(MorphologyNode {
        id: 2,
        length: 4.0,
        radius: 0.3,
        mass: 5.0,
    });

    genotype.add_edge(MorphologyEdge {
        source_node: 1,
        target_node: 2,
        joint_anchor: Vec3::new(0.0, 0.0, 1.0),
        joint_axis: Vec3::Y,
    });

    // Root is rotated by 90 degrees around Y axis
    let initial_pos = Vec3::new(1.0, 2.0, 3.0);
    let initial_rot = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);

    let root_entity = decode_genotype(&mut world, &genotype, initial_pos, initial_rot);
    let root_children = world.get::<ChildrenLinks>(root_entity).unwrap();
    let child_entity = root_children.0[0];

    let child_pos = world.get::<Position>(child_entity).unwrap().0;

    // Formula calculation:
    // P_child = P_parent + R_parent * joint_anchor + R_child * (0.0, 0.0, length_child / 2.0)
    // Assume R_child = R_parent = initial_rot
    // P_child = (1.0, 2.0, 3.0) + initial_rot * (0.0, 0.0, 1.0) + initial_rot * (0.0, 0.0, 2.0)
    // P_child = (1.0, 2.0, 3.0) + initial_rot * (0.0, 0.0, 3.0)
    // Since initial_rot rotates around Y by 90 degrees, (0.0, 0.0, 3.0) rotated is (3.0, 0.0, 0.0)
    // Expected P_child = (1.0 + 3.0, 2.0, 3.0) = (4.0, 2.0, 3.0)

    let expected_pos = Vec3::new(4.0, 2.0, 3.0);
    assert!((child_pos.x - expected_pos.x).abs() < 1e-4);
    assert!((child_pos.y - expected_pos.y).abs() < 1e-4);
    assert!((child_pos.z - expected_pos.z).abs() < 1e-4);
}
