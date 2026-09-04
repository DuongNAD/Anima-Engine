//! Production-Grade E2E Integration Test Suite for Anima-Engine Ecology AI & Evolutionary Dynamics.
//!
//! Authoritative validation suite satisfying all Acceptance Criteria:
//! 1. Closed Energy Ledger Audit:
//!    - Across an automated 10,000-tick live simulation run, maximum total energy variance
//!      |E_total(t) - E_total(0)| <= 1e-3 EU (RESIDUAL_ABS_TOLERANCE_EU) at every 100-tick sample
//!      and at final tick 10,000.
//! 2. Multi-Species Persistence:
//!    - Primary producers (E_plants > 0) and at least two distinct consumer species (prey > 0 and
//!      predator > 0) remain active and viable after 10,000 continuous simulation ticks.
//! 3. Biodiversity Maintenance:
//!    - Shannon diversity index H' remains strictly > 0.5 continuously across the 10,000-tick evaluation.
//! 4. MAP-Elites Archive Occupancy:
//!    - Evolutionary archive discovers and retains individuals across >= 4 distinct phenotypic/trophic
//!      niche cells after evolutionary epochs.
//! 5. Deterministic Replay:
//!    - Two independent simulation runs with identical seed HD_SEED = 0x5EED produce bit-identical
//!      state hashes (world_checksum) at tick 1,000, tick 5,000, and tick 10,000.
//! 6. Boundary & Corner Cases:
//!    - Covers zero detritus, saturated reserves, winter troughs, and extreme temperatures.
//!
//! Adheres strictly to repository hardening standards under AGENTS.md and scripts/verify.sh:
//! - Deterministic seed HD_SEED = 0x5EED.
//! - Single-threaded execution under DeterministicMode::on().
//! - Strong behavioral assertions.
//! - Zero phantom energy creation or leakage.

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::{
    apply_staggered_evolution_system, check_epoch_completion_system, AgentEvaluation,
    AgentGeneration, AgentGenotype, AgentLineageId, BevyMapElitesArchive,
};
use anima_engine_lib::core::components::{AgentParentLineageIds, FeatureTracker};
use anima_engine_lib::core::determinism::DeterministicMode;
use anima_engine_lib::core::ecology::{
    boltzmann_factor, ecological_descriptors, metabolic_rate, shannon_index, simpson_index,
    trophic_ratio, EcosystemBiomass, ResourceField, SeasonClock, E_ANIMAL_EV, SEASON_AMPLITUDE,
};
use anima_engine_lib::core::ecs::{
    init_world, Agent, EpochManager, MapBounds, Position, Predator, Prey, Tree,
};
use anima_engine_lib::core::energy_ledger::{
    closed_total_eu, credit_reserve, Compartment, EnergyEvent, EnergyLedger,
    RESIDUAL_ABS_TOLERANCE_EU,
};
use anima_engine_lib::core::environmental_systems::{
    detect_environmental_collisions_system, ecosystem_census_system, fruit_growth_system,
    herbivore_grazing_system, resource_field_regrowth_system,
};
use anima_engine_lib::core::resources::{
    AgentEpochStats, EnvironmentalSpawnSettings, EvolutionQueue, EvolutionReceiver,
    EvolutionSender, EvolutionSpawn, FoodSpawnSettings, SimRng,
};
use anima_engine_lib::core::snapshot::world_checksum;
use anima_engine_lib::core::world_systems::{
    combat_system, detect_food_collisions_system, metabolic_decay_system, spawn_food_system,
};
use anima_engine_lib::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};
use anima_engine_lib::evolution::map_elites::{
    EliteIndividual, MapElitesArchive, DESCRIPTOR_SET_ID,
};
use anima_engine_lib::physics::SpatialCollider;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{ExecutorKind, Schedule};

/// Constant deterministic seed from .hardening.env
const HD_SEED: u64 = 0x5EED;

// ---------------------------------------------------------------------------------------------
// Genotype Factories for Phenotypic Diversity
// ---------------------------------------------------------------------------------------------

/// 1-node genotype: mass = 1.0 (normalized f0 = 1.0 / 20.0 = 0.05, bin_x = 1 at resolution 0.05)
fn genotype_1node() -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    g.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.25,
        mass: 1.0,
    });
    g
}

/// 2-node genotype: mass = 2.0 (normalized f0 = 2.0 / 20.0 = 0.10, bin_x = 2 at resolution 0.05)
fn genotype_2nodes() -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    g.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.3,
        mass: 1.0,
    });
    g.add_node(MorphologyNode {
        id: 1,
        length: 1.0,
        radius: 0.3,
        mass: 1.0,
    });
    g.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 1,
        joint_anchor: glam::Vec3::new(1.0, 0.0, 0.0),
        joint_axis: glam::Vec3::new(0.0, 0.0, 1.0),
    });
    g
}

/// 3-node genotype: mass = 3.0 (normalized f0 = 3.0 / 20.0 = 0.15, bin_x = 3 at resolution 0.05)
fn genotype_3nodes() -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    g.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.3,
        mass: 1.0,
    });
    g.add_node(MorphologyNode {
        id: 1,
        length: 1.0,
        radius: 0.3,
        mass: 1.0,
    });
    g.add_node(MorphologyNode {
        id: 2,
        length: 1.0,
        radius: 0.3,
        mass: 1.0,
    });
    g.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 1,
        joint_anchor: glam::Vec3::new(1.0, 0.0, 0.0),
        joint_axis: glam::Vec3::new(0.0, 0.0, 1.0),
    });
    g.add_edge(MorphologyEdge {
        source_node: 1,
        target_node: 2,
        joint_anchor: glam::Vec3::new(1.0, 0.0, 0.0),
        joint_axis: glam::Vec3::new(0.0, 1.0, 0.0),
    });
    g
}

// ---------------------------------------------------------------------------------------------
// Test Harness Setup
// ---------------------------------------------------------------------------------------------

/// Constructs the single-threaded, fully deterministic simulation schedule.
fn build_production_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.set_executor_kind(ExecutorKind::SingleThreaded);
    schedule.add_systems((
        metabolic_decay_system,
        spawn_food_system,
        detect_food_collisions_system.after(spawn_food_system),
        combat_system,
        fruit_growth_system,
        detect_environmental_collisions_system.after(fruit_growth_system),
        check_epoch_completion_system.after(metabolic_decay_system),
        apply_staggered_evolution_system.after(check_epoch_completion_system),
        herbivore_grazing_system,
        resource_field_regrowth_system.after(herbivore_grazing_system),
        ecosystem_census_system.after(resource_field_regrowth_system),
    ));
    schedule
}

/// Reads the authoritative closed-total energy from the world.
fn authoritative_closed_total(world: &mut World) -> f64 {
    let plants = world
        .get_resource::<ResourceField>()
        .map(|f| f.total_biomass())
        .unwrap_or(0.0);
    let (detritus, dormant) = world
        .get_resource::<EcosystemBiomass>()
        .map(|p| (p.detritus, p.dormant))
        .unwrap_or((0.0, 0.0));
    let mut animals = 0.0f64;
    let mut q = world.query_filtered::<&HomeostaticState, With<Agent>>();
    for homeo in q.iter(world) {
        animals += homeo.energy.max(0.0) as f64;
    }
    closed_total_eu(plants, animals, detritus, dormant)
}

/// Counts living prey and predator individuals with active metabolic reserves (energy > 0).
fn count_living_guilds(world: &mut World) -> (u32, u32) {
    let mut prey_count = 0u32;
    let mut pred_count = 0u32;
    let mut q_prey = world.query_filtered::<&HomeostaticState, (With<Agent>, With<Prey>)>();
    for homeo in q_prey.iter(world) {
        if homeo.energy > 0.0 {
            prey_count += 1;
        }
    }
    let mut q_pred = world.query_filtered::<&HomeostaticState, (With<Agent>, With<Predator>)>();
    for homeo in q_pred.iter(world) {
        if homeo.energy > 0.0 {
            pred_count += 1;
        }
    }
    (prey_count, pred_count)
}

/// Builds a production-grade live world with multi-guild founders and evolutionary resources.
fn build_production_world(
    seed: u64,
) -> (
    World,
    crossbeam_channel::Receiver<Vec<AgentEpochStats>>,
    crossbeam_channel::Sender<EvolutionSpawn>,
) {
    let mut world = init_world();

    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(FoodSpawnSettings::default());
    world.insert_resource(EnvironmentalSpawnSettings::default());
    world.insert_resource(EpochManager {
        ticks_per_epoch: 250,
        current_epoch_ticks: 0,
        current_epoch: 0,
    });
    world.insert_resource(SeasonClock::default());
    world.insert_resource(DeterministicMode::on());
    world.insert_resource(SimRng::from_seed(seed));

    let (stats_tx, stats_rx) = crossbeam_channel::unbounded::<Vec<AgentEpochStats>>();
    world.insert_resource(EvolutionSender(stats_tx));

    let (spawn_tx, spawn_rx) = crossbeam_channel::unbounded::<EvolutionSpawn>();
    world.insert_resource(EvolutionReceiver(spawn_rx));
    world.insert_resource(EvolutionQueue {
        pending_replacements: Vec::new(),
    });

    world.insert_resource(BevyMapElitesArchive {
        archive: MapElitesArchive::new(0.05),
    });

    let bounds = *world.resource::<MapBounds>();

    let g1 = genotype_1node();
    let g2 = genotype_2nodes();
    let g3 = genotype_3nodes();

    // Founding consumer populations:
    // Herbivores: 4 of g1, 4 of g2, 2 of g3 (Total = 10)
    // Predators: 2 of g1, 2 of g2 (Total = 4)
    // Initial consumer total = 14. Prey fraction = 10/14 (71.4%), Predator fraction = 4/14 (28.6%).
    // Initial Shannon H' = 0.5982 > 0.5.
    let prey_genotypes = [
        g1.clone(),
        g1.clone(),
        g1.clone(),
        g1.clone(),
        g2.clone(),
        g2.clone(),
        g2.clone(),
        g2.clone(),
        g3.clone(),
        g3.clone(),
    ];

    let pred_genotypes = [g1.clone(), g1.clone(), g2.clone(), g2.clone()];

    // Spawn 10 Prey
    for (i, g) in prey_genotypes.iter().enumerate() {
        let x_offset = bounds.min.x + (i as f32 + 1.0) * 4.0;
        let z_offset = bounds.min.z + 12.0 + ((i % 3) as f32 * 5.0);
        let pos = glam::Vec3::new(x_offset, 0.0, z_offset);
        let e = decode_genotype(&mut world, g, pos, glam::Quat::IDENTITY);
        world.entity_mut(e).insert((
            Prey,
            AgentGenotype(g.clone()),
            AgentEvaluation {
                start_position: pos,
                total_distance: 0.0,
                total_energy_expended: 0.0,
                total_energy_gained: 0.0,
                prey_energy_gained: 0.0,
                survival_ticks: 0,
                last_position: pos,
            },
            FeatureTracker::default(),
            AgentLineageId(format!("prey-founder-{i}")),
            AgentGeneration(0),
            AgentParentLineageIds(Vec::new()),
        ));
    }

    // Spawn 4 Predators placed within proximity (< 1.5 units) of prey for ecological hunting
    for (i, g) in pred_genotypes.iter().enumerate() {
        let prey_x = bounds.min.x + (i as f32 + 1.0) * 4.0;
        let prey_z = bounds.min.z + 12.0 + ((i % 3) as f32 * 5.0);
        let pos = glam::Vec3::new(prey_x + 0.8, 0.0, prey_z);
        let e = decode_genotype(&mut world, g, pos, glam::Quat::IDENTITY);
        world.entity_mut(e).insert((
            Predator,
            AgentGenotype(g.clone()),
            AgentEvaluation {
                start_position: pos,
                total_distance: 0.0,
                total_energy_expended: 0.0,
                total_energy_gained: 0.0,
                prey_energy_gained: 0.0,
                survival_ticks: 0,
                last_position: pos,
            },
            FeatureTracker::default(),
            AgentLineageId(format!("pred-founder-{i}")),
            AgentGeneration(0),
            AgentParentLineageIds(Vec::new()),
        ));
    }

    // Spawn 6 fruit-bearing trees
    for i in 0..6 {
        world.spawn((
            Tree {
                current_fruit: 10.0,
                max_fruit: 60.0,
                fruit_growth_rate: 3.0,
                time_since_last_drop: 0.0,
                seed_drop_cooldown: 1.0e9,
                seed_spread_radius: 5.0,
            },
            Position(glam::Vec3::new(
                bounds.min.x + (i as f32 + 1.0) * 5.0,
                0.0,
                bounds.min.z + 15.0,
            )),
            SpatialCollider { radius: 4.0 },
        ));
    }

    (world, stats_rx, spawn_tx)
}

// ---------------------------------------------------------------------------------------------
// TEST 1: Comprehensive 10,000-Tick Closed Ecology Simulation Gate
// Satisfies:
// - AC 1: Energy Ledger Audit (|E_total(t) - E_total(0)| <= 1e-3 EU)
// - AC 2: Multi-Species Persistence (producers, prey, predators active after 10,000 ticks)
// - AC 3: Biodiversity Maintenance (Shannon H' > 0.5 continuously across 10,000 ticks)
// - AC 4: MAP-Elites Archive Occupancy (>= 4 distinct phenotypic/trophic niche cells)
// ---------------------------------------------------------------------------------------------
#[test]
fn test_production_closed_ledger_energy_audit_and_biodiversity_10k_ticks() {
    const TARGET_TICKS: u64 = 10_000;
    const SAMPLE_INTERVAL: u64 = 100;
    const REPLACE_INTERVAL: u64 = 400;

    let (mut world, stats_rx, spawn_tx) = build_production_world(HD_SEED);
    let mut schedule = build_production_schedule();

    // Step tick 1 to settle genesis and lock the authoritative baseline
    schedule.run(&mut world);

    let baseline = world
        .resource::<EnergyLedger>()
        .baseline()
        .expect("EnergyLedger must establish and lock immutable baseline on first census");
    let measured_genesis = authoritative_closed_total(&mut world);

    assert!(
        (baseline - measured_genesis).abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "Locked baseline {baseline} must match measured genesis total {measured_genesis}"
    );

    let g1 = genotype_1node();
    let g2 = genotype_2nodes();
    let g3 = genotype_3nodes();

    let mut worst_variance = 0.0f64;
    let mut min_shannon = 10.0f32;
    let mut replacements_enqueued = 0u64;

    for tick in 1..=TARGET_TICKS {
        // Run the 60 Hz Bevy ECS schedule
        schedule.run(&mut world);

        // Process evolutionary epoch statistics into the MAP-Elites archive
        while let Ok(batch) = stats_rx.try_recv() {
            let mut archive_res = world.resource_mut::<BevyMapElitesArchive>();
            for stats in &batch {
                let features = ecological_descriptors(
                    stats.body_mass,
                    trophic_ratio(stats.prey_intake, stats.total_intake),
                )
                .to_vec();
                let elite = EliteIndividual {
                    genotype: stats.genotype.clone(),
                    fitness: stats.fitness,
                    features,
                    lineage_id: stats.lineage_id.clone(),
                    generation: stats.generation,
                };
                archive_res.archive.add_individual(elite);
            }
        }

        // Controlled generational replacement
        if tick % REPLACE_INTERVAL == 0 {
            let mut q_agents = world.query_filtered::<Entity, With<Agent>>();
            let agents: Vec<Entity> = q_agents.iter(&world).collect();
            if !agents.is_empty() {
                let victim = agents[(tick as usize / REPLACE_INTERVAL as usize) % agents.len()];
                let pos = world
                    .get::<Position>(victim)
                    .map(|p| p.0)
                    .unwrap_or(glam::Vec3::ZERO);

                let is_pred = world.get::<Predator>(victim).is_some();
                let replacement_genotype = if is_pred {
                    if (tick / REPLACE_INTERVAL) % 2 == 0 {
                        g1.clone()
                    } else {
                        g2.clone()
                    }
                } else {
                    match (tick / REPLACE_INTERVAL) % 3 {
                        0 => g1.clone(),
                        1 => g2.clone(),
                        _ => g3.clone(),
                    }
                };

                let _ = spawn_tx.send((
                    victim,
                    replacement_genotype,
                    pos,
                    format!("lineage-prod-{tick}"),
                    1,
                    Vec::new(),
                ));
                replacements_enqueued += 1;
            }
        }

        // Sample energy ledger variance and biodiversity metrics at each sample interval
        if tick % SAMPLE_INTERVAL == 0 || tick == TARGET_TICKS {
            let current_total = authoritative_closed_total(&mut world);
            let variance = (current_total - baseline).abs();
            if variance > worst_variance {
                worst_variance = variance;
            }

            assert!(
                variance <= RESIDUAL_ABS_TOLERANCE_EU,
                "Energy variance exceeded tolerance at tick {tick}: variance={variance:.9} EU (limit={RESIDUAL_ABS_TOLERANCE_EU})"
            );

            // Audit biodiversity
            let (prey, pred) = count_living_guilds(&mut world);
            let h = shannon_index(&[prey, pred]);
            if h < min_shannon {
                min_shannon = h;
            }

            assert!(
                h > 0.5,
                "Shannon diversity dropped below 0.5 at tick {tick}: prey={prey}, pred={pred}, H'={h:.4}"
            );

            // Audit persistence
            let plants = world.resource::<ResourceField>().total_biomass();
            assert!(
                plants > 0.0,
                "Primary producer biomass depleted at tick {tick}: {plants:.4} EU"
            );
            assert!(
                prey > 0,
                "Herbivore prey population collapsed at tick {tick}"
            );
            assert!(
                pred > 0,
                "Carnivore predator population collapsed at tick {tick}"
            );
        }
    }

    // Final Post-10,000-Tick Evaluations
    let final_total = authoritative_closed_total(&mut world);
    let final_variance = (final_total - baseline).abs();
    let (final_prey, final_pred) = count_living_guilds(&mut world);
    let final_plants = world.resource::<ResourceField>().total_biomass();
    let final_shannon = shannon_index(&[final_prey, final_pred]);
    let final_simpson = simpson_index(&[final_prey, final_pred]);
    let archive_occupancy = world.resource::<BevyMapElitesArchive>().archive.grid.len();

    println!(
        "=== PRODUCTION E2E 10,000-TICK AUDIT RESULTS ===\n\
         Total Ticks Completed: {}\n\
         Baseline Total EU:     {:.6}\n\
         Final Total EU:        {:.6}\n\
         Final Variance:        {:.9} EU\n\
         Worst Variance Seen:   {:.9} EU (Tolerance: {:.6} EU)\n\
         Final Plants Biomass:  {:.3} EU\n\
         Living Populations:    Prey={}, Predators={}\n\
         Final Biodiversity:    Shannon H'={:.4}, Simpson D={:.4} (Min H'={:.4})\n\
         MAP-Elites Occupancy:  {} distinct niche cells\n\
         Replacements Queued:   {}",
        TARGET_TICKS,
        baseline,
        final_total,
        final_variance,
        worst_variance,
        RESIDUAL_ABS_TOLERANCE_EU,
        final_plants,
        final_prey,
        final_pred,
        final_shannon,
        final_simpson,
        min_shannon,
        archive_occupancy,
        replacements_enqueued,
    );

    // AC 1: Energy Ledger Invariant Verification
    assert!(
        final_variance <= RESIDUAL_ABS_TOLERANCE_EU,
        "Final energy variance {final_variance:.9} EU exceeded tolerance {RESIDUAL_ABS_TOLERANCE_EU}"
    );
    assert!(
        worst_variance <= RESIDUAL_ABS_TOLERANCE_EU,
        "Worst energy variance {worst_variance:.9} EU during 10,000 ticks exceeded tolerance {RESIDUAL_ABS_TOLERANCE_EU}"
    );

    // AC 2: Multi-Species Persistence Verification
    assert!(
        final_plants > 0.0,
        "Primary producers must remain active and viable"
    );
    assert!(
        final_prey > 0,
        "Prey consumer species must remain active and viable (found {final_prey})"
    );
    assert!(
        final_pred > 0,
        "Predator consumer species must remain active and viable (found {final_pred})"
    );

    // AC 3: Biodiversity Maintenance Verification
    assert!(
        final_shannon > 0.5,
        "Final Shannon diversity {final_shannon:.4} must remain strictly > 0.5"
    );
    assert!(
        min_shannon > 0.5,
        "Minimum Shannon diversity {min_shannon:.4} across 10,000 ticks must remain strictly > 0.5"
    );
    assert!(
        final_simpson > 0.0,
        "Final Simpson index must remain strictly positive"
    );

    // AC 4: MAP-Elites Archive Occupancy Verification
    assert!(
        archive_occupancy >= 4,
        "MAP-Elites archive must occupy at least 4 distinct niche cells (found {archive_occupancy})"
    );
}

// ---------------------------------------------------------------------------------------------
// TEST 2: Deterministic Replay Bit-Identical Checksum Gate
// Satisfies:
// - AC 5: Two independent simulation runs executed with identical seed HD_SEED = 0x5EED produce
//   bit-identical state hashes (world_checksum) at tick 1,000, tick 5,000, and tick 10,000.
// ---------------------------------------------------------------------------------------------
#[test]
fn test_deterministic_replay_state_hashes_at_1k_5k_10k_ticks() {
    const TARGET_TICKS: u64 = 10_000;
    const CHECKPOINT_TICKS: [u64; 3] = [1_000, 5_000, 10_000];

    let g1 = genotype_1node();
    let g2 = genotype_2nodes();
    let g3 = genotype_3nodes();

    // Helper runner executing a complete deterministic trajectory and recording checksums
    let run_trajectory = |seed: u64| -> Vec<(u64, u32)> {
        let (mut world, stats_rx, spawn_tx) = build_production_world(seed);
        let mut schedule = build_production_schedule();
        let mut recorded = Vec::new();

        for tick in 1..=TARGET_TICKS {
            schedule.run(&mut world);

            // Drain stats to update archive deterministically
            while let Ok(batch) = stats_rx.try_recv() {
                let mut archive_res = world.resource_mut::<BevyMapElitesArchive>();
                for stats in &batch {
                    let features = ecological_descriptors(
                        stats.body_mass,
                        trophic_ratio(stats.prey_intake, stats.total_intake),
                    )
                    .to_vec();
                    let elite = EliteIndividual {
                        genotype: stats.genotype.clone(),
                        fitness: stats.fitness,
                        features,
                        lineage_id: stats.lineage_id.clone(),
                        generation: stats.generation,
                    };
                    archive_res.archive.add_individual(elite);
                }
            }

            // Controlled generational replacements with deterministic genotypes
            if tick % 400 == 0 {
                let mut q_agents = world.query_filtered::<Entity, With<Agent>>();
                let agents: Vec<Entity> = q_agents.iter(&world).collect();
                if !agents.is_empty() {
                    let victim = agents[(tick as usize / 400) % agents.len()];
                    let pos = world
                        .get::<Position>(victim)
                        .map(|p| p.0)
                        .unwrap_or(glam::Vec3::ZERO);

                    let is_pred = world.get::<Predator>(victim).is_some();
                    let replacement_genotype = if is_pred {
                        if (tick / 400) % 2 == 0 {
                            g1.clone()
                        } else {
                            g2.clone()
                        }
                    } else {
                        match (tick / 400) % 3 {
                            0 => g1.clone(),
                            1 => g2.clone(),
                            _ => g3.clone(),
                        }
                    };

                    let _ = spawn_tx.send((
                        victim,
                        replacement_genotype,
                        pos,
                        format!("lineage-replay-{tick}"),
                        1,
                        Vec::new(),
                    ));
                }
            }

            if CHECKPOINT_TICKS.contains(&tick) {
                let hash = world_checksum(&mut world);
                recorded.push((tick, hash));
            }
        }
        recorded
    };

    println!("Executing Run A with seed {HD_SEED:#X}...");
    let hashes_a = run_trajectory(HD_SEED);

    println!("Executing Run B with identical seed {HD_SEED:#X}...");
    let hashes_b = run_trajectory(HD_SEED);

    assert_eq!(
        hashes_a.len(),
        CHECKPOINT_TICKS.len(),
        "Run A must record all checkpoint hashes"
    );
    assert_eq!(
        hashes_b.len(),
        CHECKPOINT_TICKS.len(),
        "Run B must record all checkpoint hashes"
    );

    for i in 0..CHECKPOINT_TICKS.len() {
        let (tick_a, hash_a) = hashes_a[i];
        let (tick_b, hash_b) = hashes_b[i];
        assert_eq!(tick_a, tick_b, "Checkpoint tick mismatch");
        println!(
            "Tick {tick_a}: Run A Hash = {hash_a:#010X} | Run B Hash = {hash_b:#010X} | Match = {}",
            hash_a == hash_b
        );
        assert_eq!(
            hash_a, hash_b,
            "Non-deterministic drift detected at tick {tick_a}: Run A ({hash_a:#010X}) != Run B ({hash_b:#010X})"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// TEST 3: MAP-Elites Quality-Diversity Architecture & Multi-Niche Occupancy Gate
// Satisfies:
// - AC 4: Archive discovers and retains individuals across >= 4 distinct niche cells
// - Verifies descriptor set "body-mass+trophic-ratio/v2", binning arithmetic, and persistence
// ---------------------------------------------------------------------------------------------
#[test]
fn test_map_elites_multiniche_occupancy_and_descriptors_contract() {
    let resolution = 0.25f32;
    let mut archive = MapElitesArchive::new(resolution);
    assert_eq!(archive.grid_resolution, 0.25);
    assert_eq!(archive.grid.len(), 0);

    // Build diverse phenotypic genotypes
    let g_small = genotype_1node();
    let g_medium = genotype_2nodes();
    let g_large = genotype_3nodes();

    let mass_small = g_small.total_mass();
    let mass_medium = g_medium.total_mass();
    let mass_large = g_large.total_mass();

    assert_eq!(mass_small, 1.0);
    assert_eq!(mass_medium, 2.0);
    assert_eq!(mass_large, 3.0);

    // 1. Small Grazer (mass=1.0, total_intake=50.0, prey_intake=0.0 -> trophic=0.0)
    let desc_small_grazer =
        ecological_descriptors(mass_small, trophic_ratio(0.0, 50.0)).to_vec();
    let coords_small_grazer = archive.get_bin_coords(&desc_small_grazer);
    assert_eq!(coords_small_grazer, (0, 0));

    // 2. Medium Grazer (mass=6.0, total_intake=80.0, prey_intake=0.0 -> trophic=0.0)
    let desc_medium_grazer =
        ecological_descriptors(6.0, trophic_ratio(0.0, 80.0)).to_vec();
    let coords_medium_grazer = archive.get_bin_coords(&desc_medium_grazer);
    assert_eq!(coords_medium_grazer, (1, 0));

    // 3. Large Grazer (mass=12.0, total_intake=120.0, prey_intake=0.0 -> trophic=0.0)
    let desc_large_grazer =
        ecological_descriptors(12.0, trophic_ratio(0.0, 120.0)).to_vec();
    let coords_large_grazer = archive.get_bin_coords(&desc_large_grazer);
    assert_eq!(coords_large_grazer, (2, 0));

    // 4. Small Agile Predator (mass=1.0, total_intake=40.0, prey_intake=40.0 -> trophic=1.0)
    let desc_small_pred =
        ecological_descriptors(mass_small, trophic_ratio(40.0, 40.0)).to_vec();
    let coords_small_pred = archive.get_bin_coords(&desc_small_pred);
    assert_eq!(coords_small_pred, (0, 4));

    // 5. Large Apex Predator (mass=12.0, total_intake=100.0, prey_intake=100.0 -> trophic=1.0)
    let desc_large_pred =
        ecological_descriptors(12.0, trophic_ratio(100.0, 100.0)).to_vec();
    let coords_large_pred = archive.get_bin_coords(&desc_large_pred);
    assert_eq!(coords_large_pred, (2, 4));

    // 6. Omnivore Opportunist (mass=6.0, total_intake=60.0, prey_intake=30.0 -> trophic=0.5)
    let desc_omnivore =
        ecological_descriptors(6.0, trophic_ratio(30.0, 60.0)).to_vec();
    let coords_omnivore = archive.get_bin_coords(&desc_omnivore);
    assert_eq!(coords_omnivore, (1, 2));

    // Insert all candidates
    assert!(archive.add_individual(EliteIndividual {
        genotype: g_small.clone(),
        fitness: 100.0,
        features: desc_small_grazer,
        lineage_id: "lineage-grazer-sm".into(),
        generation: 1,
    }));
    assert!(archive.add_individual(EliteIndividual {
        genotype: g_medium.clone(),
        fitness: 150.0,
        features: desc_medium_grazer,
        lineage_id: "lineage-grazer-md".into(),
        generation: 1,
    }));
    assert!(archive.add_individual(EliteIndividual {
        genotype: g_large.clone(),
        fitness: 180.0,
        features: desc_large_grazer,
        lineage_id: "lineage-grazer-lg".into(),
        generation: 1,
    }));
    assert!(archive.add_individual(EliteIndividual {
        genotype: g_small.clone(),
        fitness: 120.0,
        features: desc_small_pred,
        lineage_id: "lineage-pred-sm".into(),
        generation: 1,
    }));
    assert!(archive.add_individual(EliteIndividual {
        genotype: g_large.clone(),
        fitness: 220.0,
        features: desc_large_pred,
        lineage_id: "lineage-pred-lg".into(),
        generation: 1,
    }));
    assert!(archive.add_individual(EliteIndividual {
        genotype: g_medium.clone(),
        fitness: 140.0,
        features: desc_omnivore,
        lineage_id: "lineage-omnivore".into(),
        generation: 1,
    }));

    // Verify occupancy across at least 4 distinct cells
    assert_eq!(archive.grid.len(), 6);
    assert!(archive.grid.len() >= 4);

    let occupied_cells: Vec<(i32, i32)> = archive.grid.keys().copied().collect();
    assert_eq!(
        occupied_cells,
        vec![(0, 0), (0, 4), (1, 0), (1, 2), (2, 0), (2, 4)]
    );

    // Verify Quality-Diversity replacement rule:
    // Lower fitness individual in an existing cell is REFUSED
    let rejected = archive.add_individual(EliteIndividual {
        genotype: g_small.clone(),
        fitness: 80.0, // inferior to 100.0
        features: archive.grid.get(&(0, 0)).unwrap().features.clone(),
        lineage_id: "inferior-candidate".into(),
        generation: 2,
    });
    assert!(!rejected);
    assert_eq!(archive.grid.get(&(0, 0)).unwrap().fitness, 100.0);

    // Higher fitness individual in an existing cell REPLACES elite
    let accepted = archive.add_individual(EliteIndividual {
        genotype: g_small.clone(),
        fitness: 250.0, // superior to 100.0
        features: archive.grid.get(&(0, 0)).unwrap().features.clone(),
        lineage_id: "superior-candidate".into(),
        generation: 2,
    });
    assert!(accepted);
    assert_eq!(archive.grid.get(&(0, 0)).unwrap().fitness, 250.0);
    assert_eq!(
        archive.grid.get(&(0, 0)).unwrap().lineage_id,
        "superior-candidate"
    );

    // Serialization and Deserialization round-trip audit
    let saved = archive.to_saved();
    assert_eq!(saved.descriptor_set_id, DESCRIPTOR_SET_ID);
    assert_eq!(saved.grid_resolution, 0.25);
    assert_eq!(saved.elites.len(), 6);

    let restored = MapElitesArchive::from_saved(saved).expect("Archive must cleanly restore");
    assert_eq!(restored.grid.len(), 6);
    assert_eq!(restored.grid_resolution, 0.25);
    assert_eq!(
        restored.grid.get(&(0, 0)).unwrap().fitness,
        250.0
    );
}

// ---------------------------------------------------------------------------------------------
// TEST 4: Boundary & Corner Cases in Ecological Limits
// Satisfies:
// - AC 6: Cover edge cases:
//   A. Zero Detritus (no regrowth, no negative detritus, no phantom energy, no panic)
//   B. Saturated Reserves (full capacity clamping, exact refused recording)
//   C. Winter Troughs (seasonal oscillation min fertility, sustained ledger conservation)
//   D. Extreme Temperatures (absolute zero, boiling heat, subnormals, monotonic physiological band)
// ---------------------------------------------------------------------------------------------
#[test]
fn test_boundary_and_corner_cases_ecological_limits() {
    // -------------------------------------------------------------------------
    // Subcase A: Zero Detritus Pool
    // -------------------------------------------------------------------------
    let mut pool_a = EcosystemBiomass {
        detritus: 0.0,
        plants: 1000.0,
        animals: 500.0,
        dormant: 0.0,
    };
    let mut ledger_a = EnergyLedger::default();
    ledger_a.lock_baseline(pool_a.total());

    // Plant regrowth attempted with 0 detritus must yield 0 EU regrowth
    let biomes = [0u8; 4];
    let mut field = ResourceField::from_biomes(&biomes, 2, 2, -10.0, -10.0, 10.0, 10.0, 0.1);
    field.r.fill(10.0);
    let before_biomass = field.total_biomass();

    // Step regrowth with detritus pool = 0.0
    let grown = field.step_regrowth_gated_strided(1.0 / 60.0, 1.0, pool_a.detritus, 0, 1);
    pool_a.detritus -= grown;
    pool_a.plants += grown;
    assert_eq!(grown, 0.0);
    assert_eq!(field.total_biomass(), before_biomass);
    assert_eq!(pool_a.detritus, 0.0);
    assert!(pool_a.detritus >= 0.0, "Detritus must never be negative");

    // Transfer into animal reserve when detritus = 0 must yield 0 and record refused
    let mut reserve = 20.0f32;
    let transferred = ledger_a.transfer_into_reserve(
        &mut pool_a,
        Compartment::Detritus,
        &mut reserve,
        100.0,
        50.0,
        EnergyEvent::Feeding,
    );
    assert_eq!(transferred, 0.0);
    assert_eq!(reserve, 20.0);
    assert_eq!(ledger_a.refused(), 50.0);
    assert_eq!(pool_a.total(), 1500.0);
    assert!(ledger_a.conserves(pool_a.total()));

    // -------------------------------------------------------------------------
    // Subcase B: Saturated Animal Reserves & Partial Shortfalls
    // -------------------------------------------------------------------------
    let mut pool_b = EcosystemBiomass {
        detritus: 500.0,
        plants: 1000.0,
        animals: 500.0,
        dormant: 0.0,
    };
    let mut ledger_b = EnergyLedger::default();
    ledger_b.lock_baseline(pool_b.total());

    let mut sat_reserve = 100.0f32; // Reserve already at capacity cap 100.0
    let sat_cap = 100.0f32;

    // Credit reserve helper must return 0.0 and clamp exactly
    let credited = credit_reserve(&mut sat_reserve, 50.0, sat_cap);
    assert_eq!(credited, 0.0);
    assert_eq!(sat_reserve, 100.0);

    // Transfer into reserve at capacity must transfer 0.0 and leave detritus unchanged
    let sat_moved = ledger_b.transfer_into_reserve(
        &mut pool_b,
        Compartment::Detritus,
        &mut sat_reserve,
        sat_cap,
        50.0,
        EnergyEvent::Feeding,
    );
    assert_eq!(sat_moved, 0.0);
    assert_eq!(sat_reserve, 100.0);
    assert_eq!(pool_b.detritus, 500.0);
    assert!(ledger_b.conserves(pool_b.total()));

    // Partial saturation: reserve at 80.0 with cap 100.0, requesting 50.0
    // Reserve absorbs 20.0 (up to cap 100.0), and 30.0 shortfall is recorded in refused
    let mut part_reserve = 80.0f32;
    let prev_refused = ledger_b.refused();
    let part_moved = ledger_b.transfer_into_reserve(
        &mut pool_b,
        Compartment::Detritus,
        &mut part_reserve,
        sat_cap,
        50.0,
        EnergyEvent::Feeding,
    );
    assert_eq!(part_moved, 20.0);
    assert_eq!(part_reserve, 100.0);
    assert_eq!(pool_b.detritus, 480.0);
    assert_eq!(ledger_b.refused() - prev_refused, 30.0);
    assert!(ledger_b.conserves(pool_b.total()));

    // -------------------------------------------------------------------------
    // Subcase C: Winter Troughs & Annual Seasonal Oscillation
    // -------------------------------------------------------------------------
    // Annual cycle phase sweep phi in [0, 2pi]
    let mut clock = SeasonClock::default();
    clock.phase = 0.0;
    clock.rate = std::f32::consts::TAU / 100.0;

    let mut min_fertility = 10.0f32;
    let mut max_fertility = -10.0f32;

    for tick in 0..1000 {
        let dt = 0.1f32;
        let fertility = clock.tick(dt);
        if fertility < min_fertility {
            min_fertility = fertility;
        }
        if fertility > max_fertility {
            max_fertility = fertility;
        }

        // Formula check: (1.0 + 0.5 * sin(phi)).max(0.0)
        let expected = (1.0 + SEASON_AMPLITUDE * clock.phase.sin()).max(0.0);
        assert!(
            (fertility - expected).abs() < 1e-5,
            "Seasonal fertility mismatch at tick {tick}"
        );
        assert!(
            fertility >= 0.0,
            "Seasonal fertility must remain non-negative"
        );
    }

    // Winter trough: minimum fertility should reach ~0.50 (1.0 - 0.5)
    // Summer peak: maximum fertility should reach ~1.50 (1.0 + 0.5)
    assert!(
        (min_fertility - 0.50).abs() < 0.05,
        "Winter trough fertility should be ~0.5, got {min_fertility}"
    );
    assert!(
        (max_fertility - 1.50).abs() < 0.05,
        "Summer peak fertility should be ~1.5, got {max_fertility}"
    );

    // -------------------------------------------------------------------------
    // Subcase D: Extreme Temperatures & Arrhenius Scaling
    // -------------------------------------------------------------------------
    // 1. Absolute zero (0 K = -273.15 °C) -> boltzmann_factor should cleanly return 0.0
    let b_zero = boltzmann_factor(-273.15, E_ANIMAL_EV);
    assert_eq!(b_zero, 0.0);
    let m_zero = metabolic_rate(10.0, -273.15, E_ANIMAL_EV);
    assert_eq!(m_zero, 0.0);

    // 2. Below absolute zero -> handled safely without panic
    let b_subzero = boltzmann_factor(-300.0, E_ANIMAL_EV);
    assert_eq!(b_subzero, 0.0);

    // 3. Deep freeze (-50 °C)
    let b_freeze = boltzmann_factor(-50.0, E_ANIMAL_EV);
    assert!(b_freeze > 0.0 && b_freeze < 1.0);
    assert!(b_freeze.is_finite());

    // 4. Reference temperature (20 °C) -> normalized factor is 1.0
    let b_ref = boltzmann_factor(20.0, E_ANIMAL_EV);
    assert!(
        (b_ref - 1.0).abs() < 1e-4,
        "Boltzmann factor at reference 20 °C must equal 1.0 (got {b_ref})"
    );

    // 5. High heat (+50 °C and +100 °C)
    let b_50 = boltzmann_factor(50.0, E_ANIMAL_EV);
    let b_100 = boltzmann_factor(100.0, E_ANIMAL_EV);
    assert!(b_50 > b_ref);
    assert!(b_100 > b_50);
    assert!(b_100.is_finite());

    // 6. Monotonicity in physiological temperature band [0 °C, 45 °C]
    let temps = [0.0f32, 10.0, 20.0, 30.0, 40.0, 45.0];
    for i in 0..temps.len() - 1 {
        let t1 = temps[i];
        let t2 = temps[i + 1];
        let rate1 = metabolic_rate(5.0, t1, E_ANIMAL_EV);
        let rate2 = metabolic_rate(5.0, t2, E_ANIMAL_EV);
        assert!(rate1 > 0.0 && rate1.is_finite());
        assert!(rate2 > 0.0 && rate2.is_finite());
        assert!(
            rate2 > rate1,
            "Metabolic rate must strictly increase with temperature: {t1}°C ({rate1}) vs {t2}°C ({rate2})"
        );
    }
}
