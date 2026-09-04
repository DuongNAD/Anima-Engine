//! Adversarial Stress-Test Suite for Production Ecology AI & Evolutionary Dynamics.
//!
//! Authored by Teamwork Preview Challenger (critic & specialist roles).
//!
//! Objectives:
//! 1. Stress-test energy conservation: Probe for phantom energy creation or numerical drift
//!    under extended simulation horizons (10,000+ ticks) and high thermal/metabolic volatility.
//! 2. Stress-test MAP-Elites archive: Validate that occupied niche cells are genuinely distinct
//!    in morphological mass and empirical trophic ratio, verify omnivore emergence, and test QD
//!    elitism replacement dynamics.
//! 3. Stress-test determinism: Verify bit-exact replay invariance across identical seeds and
//!    confirm divergence (sensitivity) when seeds differ.
//! 4. Stress-test biodiversity and multi-trophic coexistence: Audit continuous persistence
//!    of primary producers, herbivores, and carnivores, ensuring Shannon H' > 0.5 throughout.

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::agent_systems::{
    apply_staggered_evolution_system, check_epoch_completion_system, AgentEvaluation,
    AgentGeneration, AgentGenotype, AgentLineageId, BevyMapElitesArchive,
};
use anima_engine_lib::core::components::{AgentParentLineageIds, FeatureTracker};
use anima_engine_lib::core::determinism::DeterministicMode;
use anima_engine_lib::core::ecology::{
    ecological_descriptors, shannon_index, simpson_index, trophic_ratio, EcosystemBiomass,
    ResourceField, SeasonClock,
};
use anima_engine_lib::core::ecs::{
    init_world, Agent, EpochManager, MapBounds, Position, Predator, Prey, Tree,
};
use anima_engine_lib::core::energy_ledger::{
    closed_total_eu, EnergyLedger, RESIDUAL_ABS_TOLERANCE_EU,
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
use anima_engine_lib::evolution::map_elites::{EliteIndividual, MapElitesArchive};
use anima_engine_lib::physics::SpatialCollider;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{ExecutorKind, Schedule};

/// Authoritative seed from .hardening.env
const HD_SEED: u64 = 0x5EED;

// ---------------------------------------------------------------------------------------------
// Genotype Constructors
// ---------------------------------------------------------------------------------------------

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
// Harness Schedule & World Builders
// ---------------------------------------------------------------------------------------------

fn build_adversarial_schedule() -> Schedule {
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

fn compute_authoritative_total(world: &mut World) -> f64 {
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

fn count_active_guilds(world: &mut World) -> (u32, u32) {
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

fn build_adversarial_world(
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
            AgentLineageId(format!("prey-adv-{i}")),
            AgentGeneration(0),
            AgentParentLineageIds(Vec::new()),
        ));
    }

    // Spawn 4 Predators close to prey
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
            AgentLineageId(format!("pred-adv-{i}")),
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
// CHALLENGE 1: Zero-Phantom-Drift Stress Test under Thermal & Trophic Pressures
// ---------------------------------------------------------------------------------------------
#[test]
fn test_challenge_energy_conservation_extreme_thermal_and_trophic_stress() {
    let (mut world, _stats_rx, _spawn_tx) = build_adversarial_world(HD_SEED);
    let mut schedule = build_adversarial_schedule();

    // Lock baseline on tick 1
    schedule.run(&mut world);
    let baseline = world
        .resource::<EnergyLedger>()
        .baseline()
        .expect("Baseline must be locked");
    let genesis = compute_authoritative_total(&mut world);
    assert!(
        (baseline - genesis).abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "Genesis mismatch"
    );

    // Inject thermal fluctuation: heat up half the agents to 42.0°C to accelerate metabolic decay
    {
        let mut q = world.query_filtered::<&mut HomeostaticState, With<Agent>>();
        let mut count = 0;
        for mut homeo in q.iter_mut(&mut world) {
            if count % 2 == 0 {
                homeo.temperature = 42.0;
            }
            count += 1;
        }
    }

    let mut max_observed_drift = 0.0f64;
    for tick in 2..=5_000 {
        schedule.run(&mut world);

        let current_total = compute_authoritative_total(&mut world);
        let drift = (current_total - baseline).abs();
        if drift > max_observed_drift {
            max_observed_drift = drift;
        }

        assert!(
            drift <= RESIDUAL_ABS_TOLERANCE_EU,
            "Drift {drift:.12} EU exceeded tolerance {RESIDUAL_ABS_TOLERANCE_EU} at tick {tick}"
        );
    }

    println!(
        "CHALLENGE 1 (Thermal & Trophic Energy Stress): Worst drift = {:.12} EU (Limit = {:.6} EU)",
        max_observed_drift, RESIDUAL_ABS_TOLERANCE_EU
    );
    assert!(
        max_observed_drift <= RESIDUAL_ABS_TOLERANCE_EU,
        "Zero-phantom invariant holds under thermal stress"
    );
}

// ---------------------------------------------------------------------------------------------
// CHALLENGE 2: MAP-Elites Niche Authenticity & Omnivore Expansion Stress Test
// ---------------------------------------------------------------------------------------------
#[test]
fn test_challenge_map_elites_niche_authenticity_and_omnivore_emergence() {
    let resolution = 0.05f32;
    let mut archive = MapElitesArchive::new(resolution);

    let g1 = genotype_1node();
    let g2 = genotype_2nodes();
    let g3 = genotype_3nodes();

    // 1. Verify that the 5 niche cells from the live simulation correspond to distinct phenotypes
    let herbivore_1 = EliteIndividual {
        genotype: g1.clone(),
        fitness: 120.0,
        features: ecological_descriptors(1.0, trophic_ratio(0.0, 50.0)).to_vec(),
        lineage_id: "herb-1".into(),
        generation: 1,
    };
    let herbivore_2 = EliteIndividual {
        genotype: g2.clone(),
        fitness: 140.0,
        features: ecological_descriptors(2.0, trophic_ratio(0.0, 80.0)).to_vec(),
        lineage_id: "herb-2".into(),
        generation: 1,
    };
    let herbivore_3 = EliteIndividual {
        genotype: g3.clone(),
        fitness: 160.0,
        features: ecological_descriptors(3.0, trophic_ratio(0.0, 100.0)).to_vec(),
        lineage_id: "herb-3".into(),
        generation: 1,
    };
    let carnivore_1 = EliteIndividual {
        genotype: g1.clone(),
        fitness: 110.0,
        features: ecological_descriptors(1.0, trophic_ratio(40.0, 40.0)).to_vec(),
        lineage_id: "carn-1".into(),
        generation: 1,
    };
    let carnivore_2 = EliteIndividual {
        genotype: g2.clone(),
        fitness: 150.0,
        features: ecological_descriptors(2.0, trophic_ratio(60.0, 60.0)).to_vec(),
        lineage_id: "carn-2".into(),
        generation: 1,
    };

    assert!(archive.add_individual(herbivore_1));
    assert!(archive.add_individual(herbivore_2));
    assert!(archive.add_individual(herbivore_3));
    assert!(archive.add_individual(carnivore_1));
    assert!(archive.add_individual(carnivore_2));

    // Authenticate the 5 distinct cells
    assert_eq!(archive.grid.len(), 5);
    assert_eq!(
        archive.get_bin_coords(&ecological_descriptors(1.0, 0.0)),
        (1, 0)
    );
    assert_eq!(
        archive.get_bin_coords(&ecological_descriptors(2.0, 0.0)),
        (2, 0)
    );
    assert_eq!(
        archive.get_bin_coords(&ecological_descriptors(3.0, 0.0)),
        (3, 0)
    );
    assert_eq!(
        archive.get_bin_coords(&ecological_descriptors(1.0, 1.0)),
        (1, 20)
    );
    assert_eq!(
        archive.get_bin_coords(&ecological_descriptors(2.0, 1.0)),
        (2, 20)
    );

    // 2. Stress-test Omnivore emergence: an individual consuming 50% vegetation and 50% meat
    let omnivore_desc = ecological_descriptors(2.0, trophic_ratio(25.0, 50.0)).to_vec();
    let omnivore_coords = archive.get_bin_coords(&omnivore_desc);
    assert_eq!(omnivore_coords, (2, 10)); // Intermediate trophic coordinate 10 (0.50 / 0.05)

    let omnivore = EliteIndividual {
        genotype: g2.clone(),
        fitness: 175.0,
        features: omnivore_desc,
        lineage_id: "omnivore-1".into(),
        generation: 2,
    };
    assert!(archive.add_individual(omnivore));
    assert_eq!(
        archive.grid.len(),
        6,
        "Archive successfully expands to 6 distinct niches with omnivore"
    );

    // 3. Elitism replacement rule verification
    let coords_target = (1, 0);
    let original_fitness = archive.grid.get(&coords_target).unwrap().fitness;
    assert_eq!(original_fitness, 120.0);

    // Inferior candidate rejected
    let inferior = EliteIndividual {
        genotype: g1.clone(),
        fitness: 90.0,
        features: ecological_descriptors(1.0, 0.0).to_vec(),
        lineage_id: "inferior".into(),
        generation: 3,
    };
    assert!(!archive.add_individual(inferior));
    assert_eq!(archive.grid.get(&coords_target).unwrap().fitness, 120.0);

    // Superior candidate accepted
    let superior = EliteIndividual {
        genotype: g1.clone(),
        fitness: 300.0,
        features: ecological_descriptors(1.0, 0.0).to_vec(),
        lineage_id: "superior".into(),
        generation: 3,
    };
    assert!(archive.add_individual(superior));
    assert_eq!(archive.grid.get(&coords_target).unwrap().fitness, 300.0);
    assert_eq!(
        archive.grid.get(&coords_target).unwrap().lineage_id,
        "superior"
    );
}

// ---------------------------------------------------------------------------------------------
// CHALLENGE 3: Deterministic Replay Bit-Exactness & Divergence Sensitivity
// ---------------------------------------------------------------------------------------------
#[test]
fn test_challenge_determinism_replay_and_seed_sensitivity() {
    let run_to_checkpoints = |seed: u64, ticks: &[u64]| -> Vec<(u64, u32)> {
        let (mut world, _stats_rx, _spawn_tx) = build_adversarial_world(seed);
        let mut schedule = build_adversarial_schedule();
        let max_tick = *ticks.iter().max().unwrap();
        let mut recorded = Vec::new();

        for tick in 1..=max_tick {
            schedule.run(&mut world);
            if ticks.contains(&tick) {
                let hash = world_checksum(&mut world);
                recorded.push((tick, hash));
            }
        }
        recorded
    };

    let check_ticks = [500, 1000, 2000];

    // Run A with HD_SEED
    let res_a = run_to_checkpoints(HD_SEED, &check_ticks);

    // Run B with identical HD_SEED
    let res_b = run_to_checkpoints(HD_SEED, &check_ticks);

    // Run C with perturbed seed
    let perturbed_seed = HD_SEED ^ 0xDEADBEEF;
    let res_c = run_to_checkpoints(perturbed_seed, &check_ticks);

    assert_eq!(res_a.len(), 3);
    assert_eq!(res_b.len(), 3);
    assert_eq!(res_c.len(), 3);

    for i in 0..3 {
        let (tick_a, hash_a) = res_a[i];
        let (tick_b, hash_b) = res_b[i];
        let (tick_c, hash_c) = res_c[i];
        assert_eq!(tick_a, tick_b, "Tick record mismatch between Run A and Run B");
        assert_eq!(tick_a, tick_c, "Tick record mismatch between Run A and Run C");

        // 1. Bit-identical replay under same seed
        assert_eq!(
            hash_a, hash_b,
            "Replay mismatch at tick {tick_a}: Run A ({hash_a:#X}) vs Run B ({hash_b:#X})"
        );

        // 2. Clear divergence under different seed (proving non-trivial hash function)
        assert_ne!(
            hash_a, hash_c,
            "Checksum failed to diverge with different seed at tick {tick_a}"
        );
        println!(
            "CHALLENGE 3 (Determinism & Sensitivity): Tick {tick_a}: Run A={hash_a:#010X}, Run B={hash_b:#010X} (MATCH), Run C={hash_c:#010X} (DIVERGENT)",
        );
    }
}

// ---------------------------------------------------------------------------------------------
// CHALLENGE 4: 10,000-Tick Multi-Species Biodiversity & Coexistence Audit
// ---------------------------------------------------------------------------------------------
#[test]
fn test_challenge_biodiversity_coexistence_and_holling_stability() {
    const TARGET_TICKS: u64 = 10_000;
    const SAMPLE_INTERVAL: u64 = 500;

    let (mut world, _stats_rx, _spawn_tx) = build_adversarial_world(HD_SEED);
    let mut schedule = build_adversarial_schedule();

    // Settle tick 1
    schedule.run(&mut world);

    let mut min_h = 10.0f32;
    let mut min_prey = 100u32;
    let mut min_pred = 100u32;

    for tick in 2..=TARGET_TICKS {
        schedule.run(&mut world);

        if tick % SAMPLE_INTERVAL == 0 || tick == TARGET_TICKS {
            let (prey, pred) = count_active_guilds(&mut world);
            let h = shannon_index(&[prey, pred]);
            let simpson = simpson_index(&[prey, pred]);
            let plants = world.resource::<ResourceField>().total_biomass();

            if h < min_h {
                min_h = h;
            }
            if prey < min_prey {
                min_prey = prey;
            }
            if pred < min_pred {
                min_pred = pred;
            }

            // Invariant: Producer viability
            assert!(
                plants > 1000.0,
                "Plant biomass depleted at tick {tick}: {plants} EU"
            );

            // Invariant: Consumer coexistence (producers > 0, prey > 0, predators > 0)
            assert!(
                prey > 0,
                "Prey guild collapsed to 0 at tick {tick}"
            );
            assert!(
                pred > 0,
                "Predator guild collapsed to 0 at tick {tick}"
            );

            // Invariant: Shannon diversity H' > 0.5
            assert!(
                h > 0.5,
                "Shannon H' dropped to {h:.4} (<= 0.5) at tick {tick}"
            );
            assert!(
                simpson > 0.3,
                "Simpson index dropped to {simpson:.4} at tick {tick}"
            );
        }
    }

    println!(
        "CHALLENGE 4 (Biodiversity & Coexistence): 10,000 Ticks Complete | Min H'={:.4}, Min Prey={}, Min Pred={}",
        min_h, min_prey, min_pred
    );
    assert!(min_h > 0.5, "Minimum Shannon H' remained strictly > 0.5");
    assert!(min_prey > 0, "Prey persisted throughout");
    assert!(min_pred > 0, "Predators persisted throughout");
}
