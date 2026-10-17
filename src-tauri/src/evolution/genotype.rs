use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MorphologyNode {
    pub id: u32,
    pub length: f32,
    pub radius: f32,
    pub mass: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MorphologyEdge {
    pub source_node: u32,
    pub target_node: u32,
    pub joint_anchor: Vec3,
    pub joint_axis: Vec3,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MorphologyGenotype {
    pub nodes: Vec<MorphologyNode>,
    pub edges: Vec<MorphologyEdge>,
}

/// Developed phenotype attached once to the root `Agent` entity when its body is materialized
/// (`decode_genotype`), per `docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md` (D01, CM-405, CM-406).
///
/// Carrying `total_mass` in the phenotype avoids re-traversing the genotype graph on every tick in
/// the ecosystem-snapshot publish path of `simulation_loop.rs` and at epoch evaluation.
#[derive(bevy_ecs::prelude::Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DevelopedPhenotype {
    pub total_mass: f32,
}

impl MorphologyGenotype {
    /// Total body mass = the mass of the segments this genotype actually grows: the root plus
    /// everything reachable from it, each counted once. In the Metabolic Theory of Ecology this
    /// single number sets an organism's metabolism, lifespan and reproductive rate, so it is
    /// the most important ecological / life-history niche axis for the MAP-Elites archive.
    ///
    /// **Reachability is the definition, not an implementation detail.** `decode_genotype` spawns
    /// by the same root-finding and BFS below, so a node no edge path reaches never becomes a
    /// segment in the world. Summing `nodes` flat instead would place an agent in a heavier
    /// MAP-Elites cell than the body it is running around in — and every metabolic rate derived
    /// from that mass would describe an organism that does not exist. The test named
    /// `total_mass_matches_the_body_decode_genotype_spawns` holds the two traversals together.
    ///
    /// For all engine morphologies (`nodes.len() <= 64`, mutation caps at 15), the BFS runs on
    /// fixed stack arrays and performs zero heap allocations.
    pub fn total_mass(&self) -> f32 {
        if self.nodes.is_empty() {
            return 0.0;
        }

        let root_node = self
            .nodes
            .iter()
            .find(|n| !self.edges.iter().any(|e| e.target_node == n.id))
            .unwrap_or(&self.nodes[0]);

        const STACK_CAP: usize = 64;
        if self.nodes.len() <= STACK_CAP {
            let mut visited = [0u32; STACK_CAP];
            let mut visited_len = 1usize;
            visited[0] = root_node.id;

            let mut queue = [0u32; STACK_CAP];
            let mut head = 0usize;
            let mut tail = 1usize;
            queue[0] = root_node.id;

            let mut mass = root_node.mass;

            while head < tail {
                let curr_id = queue[head];
                head += 1;
                for edge in self.edges.iter().filter(|e| e.source_node == curr_id) {
                    if !visited[..visited_len].contains(&edge.target_node) {
                        if let Some(target_node) =
                            self.nodes.iter().find(|n| n.id == edge.target_node)
                        {
                            if visited_len < STACK_CAP && tail < STACK_CAP {
                                visited[visited_len] = edge.target_node;
                                visited_len += 1;
                                mass += target_node.mass;
                                queue[tail] = edge.target_node;
                                tail += 1;
                            }
                        }
                    }
                }
            }
            return mass;
        }

        use std::collections::{HashSet, VecDeque};
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        visited.insert(root_node.id);
        queue.push_back(root_node.id);

        let mut mass = root_node.mass;

        while let Some(curr_id) = queue.pop_front() {
            for edge in self.edges.iter().filter(|e| e.source_node == curr_id) {
                if !visited.contains(&edge.target_node) {
                    if let Some(target_node) = self.nodes.iter().find(|n| n.id == edge.target_node)
                    {
                        visited.insert(edge.target_node);
                        mass += target_node.mass;
                        queue.push_back(edge.target_node);
                    }
                }
            }
        }
        mass
    }

    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: MorphologyNode) {
        self.nodes.push(node);
    }

    pub fn add_edge(&mut self, edge: MorphologyEdge) {
        self.edges.push(edge);
    }
}

pub fn decode_genotype(
    world: &mut bevy_ecs::prelude::World,
    genotype: &MorphologyGenotype,
    initial_pos: glam::Vec3,
    initial_rot: glam::Quat,
) -> bevy_ecs::prelude::Entity {
    use crate::ai::cpg::CpgOscillator;
    use crate::ai::hrrl::{HomeostaticState, LastTransitionState};
    use crate::core::components::ActionGates;
    use crate::core::ecs::{
        Agent, ChildrenLinks, CognitiveState, InertiaComponent, JointAxis, ParentAgent, ParentLink,
        Position, Rotation, Segment, SegmentJointForce, SensoryBufferComponent, Velocity,
    };
    use crate::physics::dynamics::{JointConstraint, RigidBody};
    use crate::physics::SpatialCollider;
    use std::collections::{HashSet, VecDeque};

    // Kept as a panic rather than converted to a `Result`, deliberately.
    //
    // `decode_genotype` returns an `Entity` and is called from genesis, birth, epoch replacement,
    // restore and migration. Threading a `Result` out would change all of those, and every caller
    // would have to invent an answer to "what is a creature with no body" — which is a question the
    // creature-development contract already answers by construction: a genotype with no nodes cannot
    // be produced by mutation, crossover or decode, so reaching here means an upstream invariant is
    // already broken and continuing would spawn a corrupt agent into the world.
    assert!(
        !genotype.nodes.is_empty(),
        "cannot decode a genotype with no nodes; a morphology genome always has at least a root"
    );

    // Find the root node: the first node in genotype.nodes or a node with no incoming edges.
    let has_incoming: HashSet<u32> = genotype.edges.iter().map(|e| e.target_node).collect();
    let root_node = genotype
        .nodes
        .iter()
        .find(|n| !has_incoming.contains(&n.id))
        .unwrap_or(&genotype.nodes[0]);

    let mut visited = HashSet::new();
    visited.insert(root_node.id);
    let mut developed_mass = root_node.mass;

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
                pending_state: None,
            },
            ChildrenLinks(Vec::new()),
            SpatialCollider {
                radius: root_node.radius,
            },
            CognitiveState::default(),
            InertiaComponent::default(),
            SensoryBufferComponent::default(),
        ))
        .id();

    // Attach ParentAgent(root_entity) and SegmentJointForce(0.0). `ActionGates::default()` is fully
    // open, so an agent spawned here behaves exactly as it did before the gates existed — the
    // component is installed now so that later steps have somewhere to write, not to change anything.
    world.entity_mut(root_entity).insert((
        ParentAgent(root_entity),
        SegmentJointForce(0.0),
        ActionGates::default(),
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
                developed_mass += child_node.mass;

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
                        SpatialCollider {
                            radius: child_node.radius,
                        },
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

    world.entity_mut(root_entity).insert(DevelopedPhenotype {
        total_mass: developed_mass,
    });

    root_entity
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: u32, mass: f32) -> MorphologyNode {
        MorphologyNode {
            id,
            length: 1.0,
            radius: 0.25,
            mass,
        }
    }

    fn edge(source_node: u32, target_node: u32) -> MorphologyEdge {
        MorphologyEdge {
            source_node,
            target_node,
            joint_anchor: Vec3::new(0.0, 0.0, 1.0),
            joint_axis: Vec3::X,
        }
    }

    fn genotype(nodes: &[(u32, f32)], edges: &[(u32, u32)]) -> MorphologyGenotype {
        let mut g = MorphologyGenotype::new();
        for &(id, mass) in nodes {
            g.add_node(node(id, mass));
        }
        for &(s, t) in edges {
            g.add_edge(edge(s, t));
        }
        g
    }

    #[test]
    fn total_mass_of_an_empty_genotype_is_zero() {
        assert_eq!(MorphologyGenotype::new().total_mass(), 0.0);
    }

    #[test]
    fn total_mass_sums_a_connected_chain() {
        let g = genotype(&[(0, 3.0), (1, 5.0), (2, 7.0)], &[(0, 1), (1, 2)]);
        assert_eq!(g.total_mass(), 15.0);
    }

    // The behaviour that separates this from `nodes.iter().map(|n| n.mass).sum()`. Node 2 is in the
    // genome but no edge reaches it, so `decode_genotype` never spawns a segment for it and the
    // organism does not carry its mass.
    #[test]
    fn total_mass_excludes_nodes_unreachable_from_the_root() {
        let g = genotype(&[(0, 3.0), (1, 5.0), (2, 7.0)], &[(0, 1)]);
        assert_eq!(
            g.total_mass(),
            8.0,
            "the unreachable node's 7.0 must not be counted; a flat sum over `nodes` gives 15.0"
        );
    }

    // A diamond: two parents, one child. The child is one segment, so it weighs once.
    #[test]
    fn total_mass_counts_a_shared_child_once() {
        let g = genotype(
            &[(0, 3.0), (1, 5.0), (2, 7.0), (3, 11.0)],
            &[(0, 1), (0, 2), (1, 3), (2, 3)],
        );
        assert_eq!(g.total_mass(), 26.0);
    }

    // No node is edge-free here, so root-finding falls through to `nodes[0]`. Without the `visited`
    // set this would not terminate.
    #[test]
    fn total_mass_terminates_and_counts_once_on_a_cycle() {
        let g = genotype(&[(0, 3.0), (1, 5.0), (2, 7.0)], &[(0, 1), (1, 2), (2, 0)]);
        assert_eq!(g.total_mass(), 15.0);
    }

    // The invariant the descriptor exists to express: the number fed to MAP-Elites is the mass of
    // the body that actually gets spawned, and `DevelopedPhenotype` carries that mass on the root.
    #[test]
    fn total_mass_matches_the_body_decode_genotype_spawns() {
        use crate::core::ecs::Segment;
        use bevy_ecs::prelude::World;

        let g = genotype(&[(0, 3.0), (1, 5.0), (2, 7.0)], &[(0, 1)]);

        let mut world = World::new();
        let root = decode_genotype(&mut world, &g, Vec3::ZERO, glam::Quat::IDENTITY);

        let mut segments = world.query::<&Segment>();
        let spawned_mass: f32 = segments.iter(&world).map(|s| s.mass).sum();
        let pheno = world
            .get::<DevelopedPhenotype>(root)
            .expect("decode_genotype attaches DevelopedPhenotype on root");

        assert_eq!(
            g.total_mass(),
            spawned_mass,
            "the MAP-Elites body-mass descriptor and the spawned body disagree"
        );
        assert_eq!(
            pheno.total_mass, spawned_mass,
            "DevelopedPhenotype.total_mass must equal the spawned body mass"
        );
        assert_eq!(
            segments.iter(&world).count(),
            2,
            "the unreachable node must not have been spawned"
        );
    }
}
