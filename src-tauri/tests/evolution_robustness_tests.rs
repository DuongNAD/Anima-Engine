use glam::Vec3;
use anima_engine_lib::evolution::genotype::{MorphologyGenotype, MorphologyNode, MorphologyEdge};
use anima_engine_lib::evolution::map_elites::{MapElitesArchive, EliteIndividual};
use anima_engine_lib::evolution::mutation::{mutate_genotype, is_valid_genotype};
use anima_engine_lib::evolution::crossover::crossover_genotypes;

#[test]
fn test_selection_bias_tournament() {
    let mut archive = MapElitesArchive::new(0.5);
    
    // Add individuals with different fitnesses to different bins
    let ind1 = EliteIndividual {
        genotype: MorphologyGenotype::default(),
        fitness: 1.0,
        features: vec![0.0, 0.0],
        lineage_id: "".to_string(),
        generation: 0,
    };
    let ind2 = EliteIndividual {
        genotype: MorphologyGenotype::default(),
        fitness: 5.0,
        features: vec![1.0, 0.0],
        lineage_id: "".to_string(),
        generation: 0,
    };
    let ind3 = EliteIndividual {
        genotype: MorphologyGenotype::default(),
        fitness: 10.0,
        features: vec![0.0, 1.0],
        lineage_id: "".to_string(),
        generation: 0,
    };
    let ind4 = EliteIndividual {
        genotype: MorphologyGenotype::default(),
        fitness: 20.0,
        features: vec![1.0, 1.0],
        lineage_id: "".to_string(),
        generation: 0,
    };
    
    archive.add_individual(ind1);
    archive.add_individual(ind2);
    archive.add_individual(ind3);
    archive.add_individual(ind4);
    
    // Sample with uniform selection (selection_bias <= 1.0)
    let samples_uniform = 1000;
    let mut counts_uniform = std::collections::HashMap::new();
    for _ in 0..samples_uniform {
        let selected = archive.select_parent(1.0).unwrap();
        *counts_uniform.entry(selected.fitness as i32).or_insert(0) += 1;
    }
    
    // Under uniform selection, each should be chosen roughly 25% of the time (250 +/- margin)
    assert!(counts_uniform.get(&1).cloned().unwrap_or(0) > 150);
    assert!(counts_uniform.get(&5).cloned().unwrap_or(0) > 150);
    assert!(counts_uniform.get(&10).cloned().unwrap_or(0) > 150);
    assert!(counts_uniform.get(&20).cloned().unwrap_or(0) > 150);
    
    // Sample with tournament selection (selection_bias = 5.0 -> K = 5)
    let samples_tournament = 1000;
    let mut counts_tournament = std::collections::HashMap::new();
    for _ in 0..samples_tournament {
        let selected = archive.select_parent(5.0).unwrap();
        *counts_tournament.entry(selected.fitness as i32).or_insert(0) += 1;
    }
    
    let count_lowest = counts_tournament.get(&1).cloned().unwrap_or(0);
    let count_highest = counts_tournament.get(&20).cloned().unwrap_or(0);
    
    assert!(count_highest > count_lowest);
    assert!(count_highest > 500);
    assert!(count_lowest < 50);
}

#[test]
fn test_robust_mutation() {
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.2, mass: 1.0 });
    genotype.add_node(MorphologyNode { id: 1, length: 1.2, radius: 0.25, mass: 1.5 });
    genotype.add_node(MorphologyNode { id: 2, length: 0.8, radius: 0.15, mass: 0.8 });
    
    genotype.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 1,
        joint_anchor: Vec3::new(0.0, 0.0, 0.5),
        joint_axis: Vec3::Y,
    });
    genotype.add_edge(MorphologyEdge {
        source_node: 1,
        target_node: 2,
        joint_anchor: Vec3::new(0.0, 0.0, -0.6),
        joint_axis: Vec3::X,
    });
    
    assert!(is_valid_genotype(&genotype));
    
    let mut counter = 3;
    
    // Mutate 200 times and check constraints every time
    for _ in 0..200 {
        let prev_genotype = genotype.clone();
        mutate_genotype(&mut genotype, &mut counter, 1.0);
        
        // Ensure the genotype is valid
        assert!(is_valid_genotype(&genotype), "Genotype became invalid after mutating from {:?}", prev_genotype);
        
        // Ensure bounds are respected
        assert!(genotype.nodes.len() >= 2, "Node count is less than 2: {}", genotype.nodes.len());
        assert!(genotype.nodes.len() <= 15, "Node count is greater than 15: {}", genotype.nodes.len());
        
        for node in &genotype.nodes {
            assert!(node.length >= 0.1 && node.length <= 5.0, "Node length out of bounds: {}", node.length);
            assert!(node.radius >= 0.05 && node.radius <= 1.0, "Node radius out of bounds: {}", node.radius);
            assert!(node.mass >= 0.05 && node.mass <= 10.0, "Node mass out of bounds: {}", node.mass);
        }
        
        for edge in &genotype.edges {
            // Find parent node
            let parent = genotype.nodes.iter().find(|n| n.id == edge.source_node).unwrap();
            let radius = parent.radius;
            let length = parent.length;
            
            let eps = 1e-4;
            assert!(edge.joint_anchor.x >= -radius - eps && edge.joint_anchor.x <= radius + eps, "Anchor x out of bounds: {} for parent radius {}", edge.joint_anchor.x, radius);
            assert!(edge.joint_anchor.y >= -radius - eps && edge.joint_anchor.y <= radius + eps, "Anchor y out of bounds: {} for parent radius {}", edge.joint_anchor.y, radius);
            assert!(edge.joint_anchor.z >= -length / 2.0 - eps && edge.joint_anchor.z <= length / 2.0 + eps, "Anchor z out of bounds: {} for parent length {}", edge.joint_anchor.z, length);
            
            let axis_len = edge.joint_axis.length();
            assert!((axis_len - 1.0).abs() < 1e-3, "Joint axis is not normalized: length is {}", axis_len);
        }
    }
}

#[test]
fn test_subtree_crossover() {
    let mut parent_a = MorphologyGenotype::new();
    parent_a.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.2, mass: 1.0 });
    parent_a.add_node(MorphologyNode { id: 1, length: 1.0, radius: 0.2, mass: 1.0 });
    parent_a.add_node(MorphologyNode { id: 2, length: 1.0, radius: 0.2, mass: 1.0 });
    parent_a.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 1,
        joint_anchor: Vec3::ZERO,
        joint_axis: Vec3::Y,
    });
    parent_a.add_edge(MorphologyEdge {
        source_node: 1,
        target_node: 2,
        joint_anchor: Vec3::ZERO,
        joint_axis: Vec3::Y,
    });
    
    let mut parent_b = MorphologyGenotype::new();
    parent_b.add_node(MorphologyNode { id: 10, length: 2.0, radius: 0.3, mass: 1.5 });
    parent_b.add_node(MorphologyNode { id: 11, length: 2.0, radius: 0.3, mass: 1.5 });
    parent_b.add_node(MorphologyNode { id: 12, length: 2.0, radius: 0.3, mass: 1.5 });
    parent_b.add_edge(MorphologyEdge {
        source_node: 10,
        target_node: 11,
        joint_anchor: Vec3::ZERO,
        joint_axis: Vec3::Y,
    });
    parent_b.add_edge(MorphologyEdge {
        source_node: 10,
        target_node: 12,
        joint_anchor: Vec3::ZERO,
        joint_axis: Vec3::Y,
    });
    
    assert!(is_valid_genotype(&parent_a));
    assert!(is_valid_genotype(&parent_b));
    
    let mut counter = 20;
    
    for _ in 0..50 {
        let child = crossover_genotypes(&parent_a, &parent_b, &mut counter);
        
        assert!(is_valid_genotype(&child), "Crossover result is invalid genotype: {:?}", child);
        
        // Ensure child node IDs are unique
        let mut ids = std::collections::HashSet::new();
        for node in &child.nodes {
            assert!(ids.insert(node.id), "Duplicate node ID found: {}", node.id);
        }
        
        // Check that child has exactly one root
        let mut incoming = std::collections::HashSet::new();
        for edge in &child.edges {
            incoming.insert(edge.target_node);
        }
        let roots: Vec<u32> = child.nodes.iter()
            .map(|n| n.id)
            .filter(|id| !incoming.contains(id))
            .collect();
        assert_eq!(roots.len(), 1, "Child has {} roots instead of 1", roots.len());
    }
}
