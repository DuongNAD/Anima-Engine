use crate::evolution::genotype::{MorphologyEdge, MorphologyGenotype, MorphologyNode};
use glam::Vec3;
use rand::Rng;
use rand::seq::SliceRandom;

pub fn is_valid_genotype(genotype: &MorphologyGenotype) -> bool {
    if genotype.nodes.is_empty() {
        return false;
    }
    
    // Identify the root(s)
    let mut incoming = std::collections::HashSet::new();
    for edge in &genotype.edges {
        incoming.insert(edge.target_node);
    }
    
    let roots: Vec<u32> = genotype.nodes.iter()
        .map(|n| n.id)
        .filter(|id| !incoming.contains(id))
        .collect();
        
    if roots.len() != 1 {
        return false;
    }
    
    let root = roots[0];
    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![root];
    
    while let Some(curr) = stack.pop() {
        if !visited.insert(curr) {
            // Cycle detected!
            return false;
        }
        for edge in &genotype.edges {
            if edge.source_node == curr {
                stack.push(edge.target_node);
            }
        }
    }
    
    visited.len() == genotype.nodes.len()
}

pub fn mutate_genotype(genotype: &mut MorphologyGenotype, node_id_counter: &mut u32, mutation_rate: f64) {
    let mut rng = rand::thread_rng();
    if !rng.gen_bool(mutation_rate) {
        return;
    }

    let backup = genotype.clone();
    let backup_counter = *node_id_counter;

    // Shuffle the operators to try them randomly
    let mut operators = vec![1, 2, 3, 4];
    operators.shuffle(&mut rng);

    let mut success = false;
    for op in operators {
        match op {
            1 => {
                // Operator 1: Parametric Node Mutation
                if !genotype.nodes.is_empty() {
                    let idx = rng.gen_range(0..genotype.nodes.len());
                    let node_id = genotype.nodes[idx].id;
                    let length = (genotype.nodes[idx].length + rng.gen_range(-0.5..0.5)).clamp(0.1, 5.0);
                    let radius = (genotype.nodes[idx].radius + rng.gen_range(-0.1..0.1)).clamp(0.05, 1.0);
                    let mass = (genotype.nodes[idx].mass + rng.gen_range(-0.5..0.5)).clamp(0.05, 10.0);
                    
                    genotype.nodes[idx].length = length;
                    genotype.nodes[idx].radius = radius;
                    genotype.nodes[idx].mass = mass;

                    // Clamp outgoing edges to new parent boundaries
                    for edge in &mut genotype.edges {
                        if edge.source_node == node_id {
                            edge.joint_anchor.x = edge.joint_anchor.x.clamp(-radius, radius);
                            edge.joint_anchor.y = edge.joint_anchor.y.clamp(-radius, radius);
                            edge.joint_anchor.z = edge.joint_anchor.z.clamp(-length / 2.0, length / 2.0);
                        }
                    }
                    success = true;
                }
            }
            2 => {
                // Operator 2: Parametric Edge Mutation
                if !genotype.edges.is_empty() {
                    let idx = rng.gen_range(0..genotype.edges.len());
                    let edge = &mut genotype.edges[idx];
                    if let Some(parent) = genotype.nodes.iter().find(|n| n.id == edge.source_node) {
                        let radius = parent.radius;
                        let length = parent.length;
                        edge.joint_anchor.x = (edge.joint_anchor.x + rng.gen_range(-0.1..0.1)).clamp(-radius, radius);
                        edge.joint_anchor.y = (edge.joint_anchor.y + rng.gen_range(-0.1..0.1)).clamp(-radius, radius);
                        edge.joint_anchor.z = (edge.joint_anchor.z + rng.gen_range(-0.1..0.1)).clamp(-length / 2.0, length / 2.0);
                        
                        let axis_noise = Vec3::new(rng.gen_range(-0.2..0.2), rng.gen_range(-0.2..0.2), rng.gen_range(-0.2..0.2));
                        let perturbed_axis = edge.joint_axis + axis_noise;
                        edge.joint_axis = perturbed_axis.try_normalize().unwrap_or(Vec3::Y);
                        success = true;
                    }
                }
            }
            3 => {
                // Operator 3: Structural Add Node
                if genotype.nodes.len() < 15 && !genotype.nodes.is_empty() {
                    let parent_idx = rng.gen_range(0..genotype.nodes.len());
                    let parent = genotype.nodes[parent_idx].clone();
                    
                    let child_id = *node_id_counter;
                    
                    let length = (parent.length + rng.gen_range(-0.5..0.5)).clamp(0.1, 5.0);
                    let radius = (parent.radius + rng.gen_range(-0.1..0.1)).clamp(0.05, 1.0);
                    let mass = (parent.mass + rng.gen_range(-0.5..0.5)).clamp(0.05, 10.0);
                    
                    let child_node = MorphologyNode {
                        id: child_id,
                        length,
                        radius,
                        mass,
                    };
                    
                    let z_anchor = if rng.gen_bool(0.5) { parent.length / 2.0 } else { -parent.length / 2.0 };
                    let joint_anchor = Vec3::new(0.0, 0.0, z_anchor);
                    
                    let joint_axis = Vec3::new(
                        rng.gen_range(-1.0..1.0),
                        rng.gen_range(-1.0..1.0),
                        rng.gen_range(-1.0..1.0),
                    ).try_normalize().unwrap_or(Vec3::Y);
                    
                    let child_edge = MorphologyEdge {
                        source_node: parent.id,
                        target_node: child_id,
                        joint_anchor,
                        joint_axis,
                    };
                    
                    genotype.add_node(child_node);
                    genotype.add_edge(child_edge);
                    *node_id_counter += 1;
                    success = true;
                }
            }
            4 => {
                // Operator 4: Structural Remove Node
                if genotype.nodes.len() > 2 {
                    let mut outgoing = std::collections::HashSet::new();
                    for edge in &genotype.edges {
                        outgoing.insert(edge.source_node);
                    }
                    
                    let leaf_candidates: Vec<u32> = genotype.nodes.iter()
                        .map(|n| n.id)
                        .filter(|id| !outgoing.contains(id))
                        .collect();
                    
                    if !leaf_candidates.is_empty() {
                        let to_remove_id = leaf_candidates[rng.gen_range(0..leaf_candidates.len())];
                        genotype.nodes.retain(|n| n.id != to_remove_id);
                        genotype.edges.retain(|e| e.target_node != to_remove_id);
                        success = true;
                    }
                }
            }
            _ => {}
        }
        if success {
            break;
        }
    }

    // Fallback: If for some reason validation fails, revert to backup
    if success && !is_valid_genotype(genotype) {
        *genotype = backup;
        *node_id_counter = backup_counter;
    }
}
