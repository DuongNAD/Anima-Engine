use anima_engine_lib::evolution::lineage::{
    InMemoryLineageTracker, FallbackLineageTracker, LineageTracker, RelationType
};
use anima_engine_lib::evolution::genotype::{
    MorphologyGenotype, MorphologyNode
};

#[test]
fn test_in_memory_lineage_tracker() {
    let tracker = InMemoryLineageTracker::new();

    // Create root genotype
    let mut root_gen = MorphologyGenotype::new();
    root_gen.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.2, mass: 1.0 });

    // Add root node
    let root_id = "root-1".to_string();
    assert!(tracker.add_root(root_id.clone(), root_gen.clone()).is_ok());

    // Add clone reproduction
    let child_1_id = "child-1".to_string();
    assert!(tracker.add_reproduction(
        child_1_id.clone(),
        1,
        root_gen.clone(),
        vec![root_id.clone()],
        RelationType::Clone
    ).is_ok());

    // Add crossover reproduction
    let parent_2_id = "parent-2".to_string();
    assert!(tracker.add_root(parent_2_id.clone(), root_gen.clone()).is_ok());

    let child_2_id = "child-2".to_string();
    assert!(tracker.add_reproduction(
        child_2_id.clone(),
        2,
        root_gen.clone(),
        vec![child_1_id.clone(), parent_2_id.clone()],
        RelationType::Crossover
    ).is_ok());

    // Query graph
    let (nodes, relations) = tracker.get_lineage_graph().unwrap();

    // Verify nodes
    assert_eq!(nodes.len(), 4);
    assert!(nodes.iter().any(|n| n.id == root_id && n.generation == 0));
    assert!(nodes.iter().any(|n| n.id == child_1_id && n.generation == 1));
    assert!(nodes.iter().any(|n| n.id == parent_2_id && n.generation == 0));
    assert!(nodes.iter().any(|n| n.id == child_2_id && n.generation == 2));

    // Verify relations
    assert_eq!(relations.len(), 3);
    assert!(relations.iter().any(|r| r.source_id == root_id && r.target_id == child_1_id && r.relation_type == RelationType::Clone));
    assert!(relations.iter().any(|r| r.source_id == child_1_id && r.target_id == child_2_id && r.relation_type == RelationType::Crossover));
    assert!(relations.iter().any(|r| r.source_id == parent_2_id && r.target_id == child_2_id && r.relation_type == RelationType::Crossover));
}

#[test]
fn test_fallback_lineage_tracker_offline() {
    // Attempting to connect to an offline/non-existent Neo4j instance
    // Uri must be invalid/offline to verify graceful fallback
    let tracker = FallbackLineageTracker::new("bolt://localhost:9999", "neo4j", "password");

    // It should report offline immediately (or very quickly without blocking/panicking)
    assert!(!tracker.is_online());

    // Operations should work seamlessly with the in-memory fallback
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.2, mass: 1.0 });

    let root_id = "root-offline".to_string();
    assert!(tracker.add_root(root_id.clone(), genotype.clone()).is_ok());

    let child_id = "child-offline".to_string();
    assert!(tracker.add_reproduction(
        child_id.clone(),
        1,
        genotype.clone(),
        vec![root_id.clone()],
        RelationType::Mutate
    ).is_ok());

    // Fetch and verify lineage graph
    let (nodes, relations) = tracker.get_lineage_graph().unwrap();
    assert_eq!(nodes.len(), 2);
    assert_eq!(relations.len(), 1);
    assert_eq!(relations[0].source_id, root_id);
    assert_eq!(relations[0].target_id, child_id);
    assert_eq!(relations[0].relation_type, RelationType::Mutate);
}

#[test]
fn test_lineage_tier2_timeout_and_invalid_inputs() {
    // 1. Connection error / timeout simulation (offline / invalid host)
    // Connecting to a non-existent port should time out or refuse connection.
    let tracker = FallbackLineageTracker::new("bolt://127.0.0.1:65535", "neo4j", "password");
    assert!(!tracker.is_online(), "Should immediately mark as offline without panic");

    // 2. Empty/Invalid genotype inputs (empty node list)
    let empty_genotype = MorphologyGenotype::new();
    let root_id = "empty-root".to_string();
    assert!(tracker.add_root(root_id.clone(), empty_genotype.clone()).is_ok());

    let offspring_id = "empty-child".to_string();
    assert!(tracker.add_reproduction(
        offspring_id.clone(),
        1,
        empty_genotype.clone(),
        vec![root_id.clone()],
        RelationType::Clone
    ).is_ok());

    // 3. Very long lineage trees (100 generations)
    let mut current_parent = "root-long".to_string();
    assert!(tracker.add_root(current_parent.clone(), empty_genotype.clone()).is_ok());

    for gen in 1..=100 {
        let child_id = format!("child-gen-{}", gen);
        assert!(tracker.add_reproduction(
            child_id.clone(),
            gen,
            empty_genotype.clone(),
            vec![current_parent.clone()],
            RelationType::Mutate
        ).is_ok());
        current_parent = child_id;
    }

    let (nodes, relations) = tracker.get_lineage_graph().unwrap();
    // 2 (empty path) + 1 (root-long) + 100 (long path) = 103 nodes
    assert_eq!(nodes.len(), 103);
    // 1 (empty relation) + 100 (long path relations) = 101 relations
    assert_eq!(relations.len(), 101);
}

#[test]
fn test_lineage_tier3_crossover_multiple_parents() {
    let tracker = InMemoryLineageTracker::new();
    let genotype = MorphologyGenotype::new();

    // Create 4 parents
    let mut parent_ids = Vec::new();
    for i in 0..4 {
        let pid = format!("parent-{}", i);
        assert!(tracker.add_root(pid.clone(), genotype.clone()).is_ok());
        parent_ids.push(pid);
    }

    // Breed child from all 4 parents
    let child_id = "child-crossover-4".to_string();
    assert!(tracker.add_reproduction(
        child_id.clone(),
        1,
        genotype.clone(),
        parent_ids.clone(),
        RelationType::Crossover
    ).is_ok());

    let (nodes, relations) = tracker.get_lineage_graph().unwrap();
    assert_eq!(nodes.len(), 5);
    assert_eq!(relations.len(), 4);

    for pid in parent_ids {
        assert!(relations.iter().any(|r| r.source_id == pid && r.target_id == child_id && r.relation_type == RelationType::Crossover));
    }
}

#[test]
fn test_lineage_tier4_evolutionary_workload() {
    let tracker = InMemoryLineageTracker::new();
    let genotype = MorphologyGenotype::new();

    // Generation 0: Roots
    let mut gen_parents = vec!["root-A".to_string(), "root-B".to_string()];
    for parent in &gen_parents {
        assert!(tracker.add_root(parent.clone(), genotype.clone()).is_ok());
    }

    // Simulate 5 generations (1 to 5)
    for gen in 1..=5 {
        let mut next_parents = Vec::new();

        // 1. Breed via Clone
        let child_clone = format!("gen-{}-clone", gen);
        assert!(tracker.add_reproduction(
            child_clone.clone(),
            gen,
            genotype.clone(),
            vec![gen_parents[0].clone()],
            RelationType::Clone
        ).is_ok());
        next_parents.push(child_clone);

        // 2. Breed via Mutate
        let child_mutate = format!("gen-{}-mutate", gen);
        assert!(tracker.add_reproduction(
            child_mutate.clone(),
            gen,
            genotype.clone(),
            vec![gen_parents[1].clone()],
            RelationType::Mutate
        ).is_ok());
        next_parents.push(child_mutate);

        // 3. Breed via Crossover
        let child_crossover = format!("gen-{}-crossover", gen);
        assert!(tracker.add_reproduction(
            child_crossover.clone(),
            gen,
            genotype.clone(),
            vec![gen_parents[0].clone(), gen_parents[1].clone()],
            RelationType::Crossover
        ).is_ok());
        next_parents.push(child_crossover);

        gen_parents = next_parents;
    }

    let (nodes, relations) = tracker.get_lineage_graph().unwrap();
    // 2 (Gen 0) + 5 generations * 3 agents per gen = 17 nodes
    assert_eq!(nodes.len(), 17);

    // Relationships: each generation has:
    // Clone: 1 relation
    // Mutate: 1 relation
    // Crossover: 2 relations
    // Total relations per gen = 4. 5 generations * 4 = 20 relations
    assert_eq!(relations.len(), 20);

    // Verify parent-child links for the last generation
    assert!(relations.iter().any(|r| r.source_id == "gen-4-clone" && r.target_id == "gen-5-clone" && r.relation_type == RelationType::Clone));
    assert!(relations.iter().any(|r| r.source_id == "gen-4-mutate" && r.target_id == "gen-5-mutate" && r.relation_type == RelationType::Mutate));
    assert!(relations.iter().any(|r| r.source_id == "gen-4-clone" && r.target_id == "gen-5-crossover" && r.relation_type == RelationType::Crossover));
    assert!(relations.iter().any(|r| r.source_id == "gen-4-mutate" && r.target_id == "gen-5-crossover" && r.relation_type == RelationType::Crossover));
}
