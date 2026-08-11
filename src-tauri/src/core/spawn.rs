//! Cầu nối giữa genotype (dữ liệu thuần) và thế giới ECS.
//!
//! Hàm này từng nằm trong `evolution/genotype.rs` và là thứ DUY NHẤT khiến cả
//! module `evolution` không tách khỏi vỏ ứng dụng được: nó kéo theo `core::ecs`,
//! `ai::cpg`, `ai::hrrl`, `physics::dynamics`, `physics::SpatialCollider`.
//!
//! Chuyển nó sang đây để `evolution` trở thành dữ liệu + thuật toán thuần, tách
//! được thành crate riêng. Xem F-007 trong FINDINGS.md.
use crate::evolution::genotype::MorphologyGenotype;

pub fn decode_genotype(
    world: &mut bevy_ecs::prelude::World,
    genotype: &MorphologyGenotype,
    initial_pos: glam::Vec3,
    initial_rot: glam::Quat,
) -> bevy_ecs::prelude::Entity {
    use crate::ai::cpg::CpgOscillator;
    use crate::ai::hrrl::{HomeostaticState, LastTransitionState};
    use crate::core::ecs::{
        Agent, ChildrenLinks, JointAxis, ParentAgent, ParentLink, Position, Rotation, Segment,
        SegmentJointForce, Velocity,
    };
    use crate::physics::dynamics::{JointConstraint, RigidBody};
    use crate::physics::SpatialCollider;
    use std::collections::{HashSet, VecDeque};

    if genotype.nodes.is_empty() {
        panic!("Cannot decode empty genotype");
    }

    // Find the root node: the first node in genotype.nodes or a node with no incoming edges.
    let has_incoming: HashSet<u32> = genotype.edges.iter().map(|e| e.target_node).collect();
    let root_node = genotype
        .nodes
        .iter()
        .find(|n| !has_incoming.contains(&n.id))
        .unwrap_or(&genotype.nodes[0]);

    let mut visited = HashSet::new();
    visited.insert(root_node.id);

    // Spawn root segment
    let root_entity = world
        .spawn((
            Agent,
            Position(initial_pos),
            Rotation(initial_rot),
            Velocity(glam::Vec3::ZERO),
            RigidBody {
                mass: root_node.mass,
                velocity: glam::Vec3::ZERO,
                force: glam::Vec3::ZERO,
            },
            Segment {
                id: root_node.id,
                length: root_node.length,
                radius: root_node.radius,
                mass: root_node.mass,
            },
            HomeostaticState {
                energy: 100.0,
                energy_target: 100.0,
                hydration: 100.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
            LastTransitionState {
                state: [0.0; 15],
                action: [0.0; 4],
                has_last: false,
            },
            ChildrenLinks(Vec::new()),
            SpatialCollider { radius: root_node.radius },
        ))
        .id();

    // Attach ParentAgent(root_entity) and SegmentJointForce(0.0)
    world.entity_mut(root_entity).insert((
        ParentAgent(root_entity),
        SegmentJointForce(0.0),
    ));

    // BFS Queue holds: (current_node, parent_entity, parent_pos, parent_rot)
    let mut queue = VecDeque::new();
    queue.push_back((root_node, root_entity, initial_pos, initial_rot));

    while let Some((curr_node, parent_entity, parent_pos, parent_rot)) = queue.pop_front() {
        // Find all outgoing edges from this node
        for edge in genotype
            .edges
            .iter()
            .filter(|e| e.source_node == curr_node.id)
        {
            if visited.contains(&edge.target_node) {
                continue;
            }

            // Find the target node details
            if let Some(child_node) = genotype.nodes.iter().find(|n| n.id == edge.target_node) {
                visited.insert(child_node.id);

                // Compute global position and rotation
                // P_child = P_parent + R_parent * joint_anchor + R_child * (0.0, 0.0, length_child / 2.0)
                // Assume R_child = R_parent initially.
                let r_child = parent_rot;
                let p_child = parent_pos
                    + parent_rot * edge.joint_anchor
                    + r_child * glam::Vec3::new(0.0, 0.0, child_node.length / 2.0);

                let child_entity = world
                    .spawn((
                        Position(p_child),
                        Rotation(r_child),
                        Velocity(glam::Vec3::ZERO),
                        RigidBody {
                            mass: child_node.mass,
                            velocity: glam::Vec3::ZERO,
                            force: glam::Vec3::ZERO,
                        },
                        Segment {
                            id: child_node.id,
                            length: child_node.length,
                            radius: child_node.radius,
                            mass: child_node.mass,
                        },
                        ParentLink(parent_entity),
                        ChildrenLinks(Vec::new()),
                        JointConstraint {
                            parent_entity,
                            anchor_offset: edge.joint_anchor,
                            stiffness: 10.0,
                            damping: 1.0,
                        },
                        JointAxis(edge.joint_axis),
                        CpgOscillator::new(1.0, 0.5),
                        ParentAgent(root_entity),
                        SegmentJointForce(0.0),
                        SpatialCollider { radius: child_node.radius },
                    ))
                    .id();

                // Update parent's ChildrenLinks
                if let Some(mut parent_children) = world.get_mut::<ChildrenLinks>(parent_entity) {
                    parent_children.0.push(child_entity);
                }

                queue.push_back((child_node, child_entity, p_child, r_child));
            }
        }
    }

    root_entity
}
