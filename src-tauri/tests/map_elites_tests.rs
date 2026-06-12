use glam::Vec3;
use anima_engine_lib::evolution::genotype::{MorphologyGenotype, MorphologyNode, MorphologyEdge};
use anima_engine_lib::evolution::map_elites::{MapElitesArchive, EliteIndividual};
use anima_engine_lib::evolution::mutation::mutate_genotype;
use anima_engine_lib::evolution::crossover::crossover_genotypes;

#[test]
fn test_map_elites_binning() {
    let archive = MapElitesArchive::new(0.1);
    
    // Normal features
    let coords = archive.get_bin_coords(&[0.15, 0.25]);
    assert_eq!(coords, (1, 2));
    
    // Negative features
    let coords_neg = archive.get_bin_coords(&[-0.15, -0.25]);
    assert_eq!(coords_neg, (-2, -3));
    
    // Empty features fallback to (0, 0)
    let coords_empty = archive.get_bin_coords(&[]);
    assert_eq!(coords_empty, (0, 0));
}

#[test]
fn test_map_elites_archive_empty() {
    let mut archive = MapElitesArchive::new(0.5);
    
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode { id: 1, length: 1.2, radius: 0.3, mass: 1.0 });
    
    let individual = EliteIndividual {
        genotype,
        fitness: 5.5,
        features: vec![0.2, 0.4],
        lineage_id: "".to_string(),
        generation: 0,
    };
    
    // Adding to empty niche should succeed
    let added = archive.add_individual(individual);
    assert!(added);
    assert_eq!(archive.grid.len(), 1);
    
    let stored = archive.grid.get(&(0, 0)).unwrap();
    assert_eq!(stored.fitness, 5.5);
}

#[test]
fn test_map_elites_archive_replace() {
    let mut archive = MapElitesArchive::new(0.5);
    
    // Create base individual
    let mut gen_a = MorphologyGenotype::new();
    gen_a.add_node(MorphologyNode { id: 1, length: 1.2, radius: 0.3, mass: 1.0 });
    let ind_a = EliteIndividual {
        genotype: gen_a,
        fitness: 5.5,
        features: vec![0.2, 0.4],
        lineage_id: "".to_string(),
        generation: 0,
    };
    
    // Create better individual
    let mut gen_b = MorphologyGenotype::new();
    gen_b.add_node(MorphologyNode { id: 1, length: 1.5, radius: 0.3, mass: 1.2 });
    let ind_b = EliteIndividual {
        genotype: gen_b,
        fitness: 8.2,
        features: vec![0.2, 0.4],
        lineage_id: "".to_string(),
        generation: 0,
    };
    
    // Create worse individual
    let mut gen_c = MorphologyGenotype::new();
    gen_c.add_node(MorphologyNode { id: 1, length: 1.0, radius: 0.2, mass: 0.8 });
    let ind_c = EliteIndividual {
        genotype: gen_c,
        fitness: 2.1,
        features: vec![0.2, 0.4],
        lineage_id: "".to_string(),
        generation: 0,
    };
    
    // Add A (should succeed)
    assert!(archive.add_individual(ind_a));
    assert_eq!(archive.grid.get(&(0, 0)).unwrap().fitness, 5.5);
    
    // Add C (worse, should fail to replace)
    assert!(!archive.add_individual(ind_c));
    assert_eq!(archive.grid.get(&(0, 0)).unwrap().fitness, 5.5);
    
    // Add B (better, should replace)
    assert!(archive.add_individual(ind_b));
    assert_eq!(archive.grid.get(&(0, 0)).unwrap().fitness, 8.2);
}

#[test]
fn test_mutate_genotype() {
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode { id: 0, length: 2.0, radius: 0.5, mass: 1.0 });
    
    // With rate 0.0, nothing changes
    let mut counter = 1;
    let original = genotype.clone();
    mutate_genotype(&mut genotype, &mut counter, 0.0);
    assert_eq!(genotype.nodes.len(), original.nodes.len());
    assert_eq!(genotype.nodes[0].length, original.nodes[0].length);

    // With rate 1.0, some mutation happens (either parametric perturb or structural add node)
    let mut mutated = false;
    for _ in 0..100 {
        let mut temp_genotype = original.clone();
        let mut temp_counter = 1;
        mutate_genotype(&mut temp_genotype, &mut temp_counter, 1.0);
        if temp_genotype.nodes.len() == 2 || temp_genotype.nodes[0].length != 2.0 || temp_genotype.nodes[0].radius != 0.5 || temp_genotype.nodes[0].mass != 1.0 {
            mutated = true;
            break;
        }
    }
    assert!(mutated);
}

#[test]
fn test_crossover_genotypes() {
    let mut parent_a = MorphologyGenotype::new();
    parent_a.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.2, mass: 1.0 });
    parent_a.add_node(MorphologyNode { id: 2, length: 1.0, radius: 0.2, mass: 1.0 });
    
    let mut parent_b = MorphologyGenotype::new();
    parent_b.add_node(MorphologyNode { id: 1, length: 2.0, radius: 0.3, mass: 1.5 });
    parent_b.add_node(MorphologyNode { id: 3, length: 2.0, radius: 0.3, mass: 1.5 });
    
    // Add edge in parent_a
    parent_a.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 2,
        joint_anchor: Vec3::ZERO,
        joint_axis: Vec3::Y,
    });
    
    // Run crossover
    let mut counter = 4;
    let child = crossover_genotypes(&parent_a, &parent_b, &mut counter);
    
    // The subtree crossover should result in 2 nodes: the root (id 0) and a grafted node from parent_b with remapped id 4.
    assert_eq!(child.nodes.len(), 2);
    assert!(child.nodes.iter().any(|n| n.id == 0));
    assert!(child.nodes.iter().any(|n| n.id == 4));
    
    // The child should have 1 edge grafting the new node to the root.
    assert_eq!(child.edges.len(), 1);
    assert_eq!(child.edges[0].source_node, 0);
    assert_eq!(child.edges[0].target_node, 4);
}

#[test]
fn test_map_elites_extremes() {
    // 1. Boundary checks on mutation with max node length (>= 15)
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.2, mass: 1.0 });
    for i in 1..15 {
        genotype.add_node(MorphologyNode {
            id: i,
            length: 1.0,
            radius: 0.2,
            mass: 1.0,
        });
        genotype.add_edge(MorphologyEdge {
            source_node: i - 1,
            target_node: i,
            joint_anchor: Vec3::ZERO,
            joint_axis: Vec3::Y,
        });
    }
    
    let mut counter = 15;
    // Mutate multiple times with mutation_rate = 1.0, verify it never exceeds 15 nodes
    for _ in 0..50 {
        mutate_genotype(&mut genotype, &mut counter, 1.0);
        assert!(genotype.nodes.len() <= 15);
    }
    
    // 2. Large/Extreme feature coordinates clamping check (must not panic)
    let archive = MapElitesArchive::new(0.5);
    let coords_large = archive.get_bin_coords(&[1e9, -1e9]);
    assert!(coords_large.0 > 0);
    assert!(coords_large.1 < 0);
    
    // Extreme values check (Infinity, NaN, etc.)
    let coords_inf = archive.get_bin_coords(&[f32::INFINITY, f32::NEG_INFINITY]);
    // Casting infinity to i32 in Rust returns the minimum or maximum integer value, does not panic
    assert!(coords_inf.0 == i32::MAX || coords_inf.0 == i32::MIN);
    assert!(coords_inf.1 == i32::MAX || coords_inf.1 == i32::MIN);
}
