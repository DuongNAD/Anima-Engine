//! F-038 (SMELL, fixed by owner decision 2026-11-16, direction (a)) — `crossover_genotypes` no
//! longer returns a body larger than restore and migration accept.
//!
//! Restore, waking a hibernated agent, migration and the MAP-Elites archive load all refuse a
//! morphology above `MAX_MIGRATION_MORPHOLOGY_NODES` (`validate_morphology_payload`). A crossover
//! child can hold up to `|A| - 1 + |B|` nodes, and the archive's unbounded mass axis keeps a
//! heavier child, so unchecked drift would eventually build an agent that saves but cannot be
//! restored. Now a child over the node cap (or with edges >= cap) is refused exactly as an invalid
//! child is: the id counter is restored and `parent_a` is returned.
//!
//! "Keep the heavier" below is a stand-in for the archive's unbounded mass axis, not the archive
//! itself; whether a real run reaches the cap was never measured, hence the finding stays SMELL.

use anima_engine_lib::core::components::MAX_MIGRATION_MORPHOLOGY_NODES;
use anima_engine_lib::evolution::crossover::crossover_genotypes;
use anima_engine_lib::evolution::genotype::{MorphologyEdge, MorphologyGenotype, MorphologyNode};
use anima_engine_lib::evolution::mutation::is_valid_genotype;
use glam::Vec3;
use rand::SeedableRng;

/// `HD_SEED` from `.hardening.env`.
const HD_SEED: u64 = 0x5EED;
/// The largest body `mutate_genotype` builds: its add-node operator requires `nodes.len() < 15`.
const MUTATION_BUILDS_UP_TO: u32 = 15;
const GENERATIONS: u32 = 50;

fn chain(nodes: u32) -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    for id in 0..nodes {
        g.add_node(MorphologyNode {
            id,
            length: 0.5,
            radius: 0.1,
            mass: 0.2,
        });
    }
    for id in 1..nodes {
        g.add_edge(MorphologyEdge {
            source_node: id - 1,
            target_node: id,
            joint_anchor: Vec3::new(0.0, 0.0, 0.25),
            joint_axis: Vec3::X,
        });
    }
    g
}

fn within_restore_limits(g: &MorphologyGenotype) -> bool {
    g.nodes.len() <= MAX_MIGRATION_MORPHOLOGY_NODES
        && g.edges.len() < MAX_MIGRATION_MORPHOLOGY_NODES
        && is_valid_genotype(g)
}

#[test]
fn repeated_crossover_from_a_mutation_sized_body_never_exceeds_the_restore_cap() {
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(HD_SEED);
    let mut node_id_counter = MUTATION_BUILDS_UP_TO;
    let mut heaviest = chain(MUTATION_BUILDS_UP_TO);
    assert!(is_valid_genotype(&heaviest));

    for generation in 0..GENERATIONS {
        let child = crossover_genotypes(&heaviest, &heaviest, &mut node_id_counter, &mut rng)
            .expect("the node-id counter is far from exhaustion");
        assert!(
            within_restore_limits(&child),
            "generation {generation}: child has {} nodes, cap {MAX_MIGRATION_MORPHOLOGY_NODES}",
            child.nodes.len()
        );
        if child.total_mass() > heaviest.total_mass() {
            heaviest = child;
        }
    }
    println!(
        "F038 generations={GENERATIONS} nodes={} cap={MAX_MIGRATION_MORPHOLOGY_NODES} start={MUTATION_BUILDS_UP_TO}",
        heaviest.nodes.len()
    );
    assert!(within_restore_limits(&heaviest));
}

#[test]
fn an_oversized_child_is_refused_with_parent_a_and_the_counter_unchanged() {
    let parent_a = chain(100);
    let parent_b = chain(120); // 100 + 120 > cap: many draws would build > 128 nodes
    let a_json = serde_json::to_string(&parent_a).unwrap();
    let mut refused = 0;
    for seed in 0..200_u64 {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
        let mut counter = 1_000_u32;
        let child = crossover_genotypes(&parent_a, &parent_b, &mut counter, &mut rng)
            .expect("counter is far from exhaustion");
        assert!(
            within_restore_limits(&child),
            "seed {seed}: {} nodes",
            child.nodes.len()
        );
        if serde_json::to_string(&child).unwrap() == a_json && counter == 1_000 {
            refused += 1;
        }
    }
    assert!(
        refused > 0,
        "no seed produced an over-cap child; the rejection branch was never exercised"
    );
}
