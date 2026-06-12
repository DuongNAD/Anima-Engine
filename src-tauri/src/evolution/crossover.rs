use crate::evolution::genotype::{MorphologyEdge, MorphologyGenotype, MorphologyNode};
use crate::evolution::mutation::is_valid_genotype;
use rand::seq::SliceRandom;

fn get_subtree(genotype: &MorphologyGenotype, root_id: u32) -> (Vec<MorphologyNode>, Vec<MorphologyEdge>) {
    let mut subtree_nodes = Vec::new();
    let mut subtree_edges = Vec::new();
    let mut queue = vec![root_id];
    let mut visited = std::collections::HashSet::new();

    while let Some(curr) = queue.pop() {
        if !visited.insert(curr) {
            continue;
        }
        if let Some(node) = genotype.nodes.iter().find(|n| n.id == curr) {
            subtree_nodes.push(node.clone());
        }
        for edge in &genotype.edges {
            if edge.source_node == curr {
                subtree_edges.push(edge.clone());
                queue.push(edge.target_node);
            }
        }
    }
    (subtree_nodes, subtree_edges)
}

pub fn crossover_genotypes(
    parent_a: &MorphologyGenotype,
    parent_b: &MorphologyGenotype,
    node_id_counter: &mut u32,
) -> MorphologyGenotype {
    if parent_a.nodes.is_empty() {
        return parent_b.clone();
    }
    if parent_b.nodes.is_empty() {
        return parent_a.clone();
    }

    let mut child = parent_a.clone();
    let backup_counter = *node_id_counter;
    let mut rng = rand::thread_rng();

    // Identify the root of child
    let mut incoming = std::collections::HashSet::new();
    for edge in &child.edges {
        incoming.insert(edge.target_node);
    }
    let roots: Vec<u32> = child.nodes.iter()
        .map(|n| n.id)
        .filter(|id| !incoming.contains(id))
        .collect();

    let root_id = roots.first().cloned().unwrap_or_else(|| child.nodes[0].id);
    let non_roots: Vec<u32> = child.nodes.iter()
        .map(|n| n.id)
        .filter(|&id| id != root_id)
        .collect();

    if non_roots.is_empty() {
        // Fallback case: Child only has a root. Graft parent_b's subtree as child.
        if let Some(w_node) = parent_b.nodes.choose(&mut rng) {
            let (sub_nodes, sub_edges) = get_subtree(parent_b, w_node.id);
            let mut map = std::collections::HashMap::new();
            for node in &sub_nodes {
                let new_id = *node_id_counter;
                *node_id_counter += 1;
                map.insert(node.id, new_id);
                child.nodes.push(MorphologyNode {
                    id: new_id,
                    length: node.length,
                    radius: node.radius,
                    mass: node.mass,
                });
            }
            for edge in &sub_edges {
                child.edges.push(MorphologyEdge {
                    source_node: map[&edge.source_node],
                    target_node: map[&edge.target_node],
                    joint_anchor: edge.joint_anchor,
                    joint_axis: edge.joint_axis,
                });
            }

            let w_new = map[&w_node.id];
            let (anchor, axis) = if let Some(parent_b_edge) = parent_b.edges.iter().find(|e| e.target_node == w_node.id) {
                (parent_b_edge.joint_anchor, parent_b_edge.joint_axis)
            } else {
                let parent_len = child.nodes[0].length;
                (glam::Vec3::new(0.0, 0.0, parent_len / 2.0), glam::Vec3::Y)
            };

            child.edges.push(MorphologyEdge {
                source_node: root_id,
                target_node: w_new,
                joint_anchor: anchor,
                joint_axis: axis,
            });
        }
    } else {
        // Select a random non-root node v in the child
        if let Some(&v) = non_roots.choose(&mut rng) {
            if let Some(incoming_edge_idx) = child.edges.iter().position(|e| e.target_node == v) {
                let mut incoming_edge = child.edges.remove(incoming_edge_idx);
                let (v_sub_nodes, _v_sub_edges) = get_subtree(&child, v);
                let v_sub_ids: std::collections::HashSet<u32> = v_sub_nodes.iter().map(|n| n.id).collect();

                // Remove the subtree rooted at v from child
                child.nodes.retain(|n| !v_sub_ids.contains(&n.id));
                child.edges.retain(|e| !v_sub_ids.contains(&e.source_node) && !v_sub_ids.contains(&e.target_node));

                // Select a random node w from Parent B
                if let Some(w_node) = parent_b.nodes.choose(&mut rng) {
                    let (w_sub_nodes, w_sub_edges) = get_subtree(parent_b, w_node.id);
                    
                    // Graft the subtree rooted at w from parent_b into child
                    let mut map = std::collections::HashMap::new();
                    for node in &w_sub_nodes {
                        let new_id = *node_id_counter;
                        *node_id_counter += 1;
                        map.insert(node.id, new_id);
                        child.nodes.push(MorphologyNode {
                            id: new_id,
                            length: node.length,
                            radius: node.radius,
                            mass: node.mass,
                        });
                    }
                    for edge in &w_sub_edges {
                        child.edges.push(MorphologyEdge {
                            source_node: map[&edge.source_node],
                            target_node: map[&edge.target_node],
                            joint_anchor: edge.joint_anchor,
                            joint_axis: edge.joint_axis,
                        });
                    }

                    let w_new = map[&w_node.id];
                    incoming_edge.target_node = w_new;
                    child.edges.push(incoming_edge);
                }
            }
        }
    }

    if is_valid_genotype(&child) {
        child
    } else {
        *node_id_counter = backup_counter;
        parent_a.clone()
    }
}
