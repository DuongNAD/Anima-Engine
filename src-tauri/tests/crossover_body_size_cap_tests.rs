//! F-038 (SMELL) — crossover builds bodies larger than restore and migration accept, and nothing
//! between the two checks the size.
//!
//! Restore (`spawn_serialized_agent`), waking a hibernated agent, migration and the MAP-Elites
//! archive load all refuse a morphology above `MAX_MIGRATION_MORPHOLOGY_NODES`
//! (`validate_morphology_payload`; the refusal itself is pinned by
//! `migration_enqueue_atomicity_tests`). Live evolution has no such cap:
//!
//! - `mutate_genotype` adds a node only below 15, but never removes one;
//! - `crossover_genotypes` swaps a subtree of parent A for a subtree of parent B, so the child
//!   can hold up to `|A| - 1 + |B|` nodes, and nothing after it in the evolution worker checks;
//! - the archive cell is `floor(body_mass / resolution)` with no upper bound
//!   (`map_elites.rs:151`), so a child heavier than every elite always lands in an empty cell and
//!   is always kept. Upward drift in body size is retained, never competed away.
//!
//! What this file proves is the operator half: starting from the largest body mutation builds,
//! repeated crossover under "keep the heavier" selection passes the restore cap with every
//! intermediate body a valid tree. "Keep the heavier" is a stand-in for the archive's unbounded
//! mass axis, not the archive itself; whether a real run reaches the cap is not measured here.
//! A world holding such an agent saves (serialisation does not validate) and then cannot be
//! restored: `simulation_loop.rs` aborts the whole restore on the first refused agent.

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
const MAX_GENERATIONS: u32 = 200;

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

#[test]
fn repeated_crossover_from_a_mutation_sized_body_passes_the_restore_cap() {
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(HD_SEED);
    let mut node_id_counter = MUTATION_BUILDS_UP_TO;
    let mut heaviest = chain(MUTATION_BUILDS_UP_TO);
    assert!(is_valid_genotype(&heaviest));

    let mut generations = 0_u32;
    while heaviest.nodes.len() <= MAX_MIGRATION_MORPHOLOGY_NODES && generations < MAX_GENERATIONS {
        let child = crossover_genotypes(&heaviest, &heaviest, &mut node_id_counter, &mut rng)
            .expect("the node-id counter is far from exhaustion");
        assert!(
            is_valid_genotype(&child),
            "generation {generations}: crossover must keep a valid tree"
        );
        if child.total_mass() > heaviest.total_mass() {
            heaviest = child;
        }
        generations += 1;
    }
    println!(
        "F038 generations={generations} nodes={} cap={MAX_MIGRATION_MORPHOLOGY_NODES} start={MUTATION_BUILDS_UP_TO}",
        heaviest.nodes.len()
    );

    assert!(
        heaviest.nodes.len() > MAX_MIGRATION_MORPHOLOGY_NODES,
        "after {generations} generations the heaviest body has {} nodes, within the cap of \
         {MAX_MIGRATION_MORPHOLOGY_NODES}: crossover no longer outgrows it (has a size cap been \
         added? then F-038 is fixed and this test should assert the cap instead)",
        heaviest.nodes.len()
    );
}
