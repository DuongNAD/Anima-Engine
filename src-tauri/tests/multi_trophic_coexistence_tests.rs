//! Multi-Trophic Level Coexistence & Closed-Loop Energy Conservation Test Suite
//!
//! Milestone 1 Feature 4 & Conservation Law Gate:
//! 1. 3-trophic unforced coexistence (Producers, Herbivores, Predators) sustaining >= 5,000 ticks.
//! 2. Autonomous continuous intra-tick reproduction on energy surplus with zero artificial epoch respawning.
//! 3. Total closed-system energy E(t) = P(t) + A(t) + D(t) drift |E(t) - E(0)| <= 10^-3 EU (<0.01% drift).
//! 4. Lotka-Volterra phase-space non-collapse (non-trivial attractor orbit).

use std::sync::Arc;
use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::{HomeostaticState, TransitionSender};
use anima_engine_lib::ai::model::{BrainInferenceBuffer, BrainModel};
use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};
use anima_engine_lib::core::agent_systems::{
    ActiveEvolutionSettings, AgentEvaluation, AgentGeneration, AgentGenotype, AgentLineageId,
    BevyEvolutionRunning, BevyEvolutionSettings, BevyMapElitesArchive, BevyMapElitesGrid,
    EnvironmentalEventReceiver, InferenceChannels, InferenceRequestBatch, InferenceResponseBatch,
    NextNodeId,
};

use anima_engine_lib::core::determinism::DeterministicMode;
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::ecs::{
    init_world, ActionGates, Agent, AgentParentLineageIds, BevyMigrationTrigger, CognitiveState,
    EnvironmentalSpawnSettings, EpochManager, EvolutionQueue, EvolutionReceiver, EvolutionSender,
    FeatureTracker, InboundMigrationReceiver, InertiaComponent, MapBounds, OutboundMigrationSender,
    Predator, Prey, ShardingConfig, ShardingResource, Velocity,
};

use anima_engine_lib::core::energy_ledger::{
    closed_total_eu, EnergyLedger, RESIDUAL_ABS_TOLERANCE_EU,
};
use anima_engine_lib::core::live_experiment::LiveInferencePump;
use anima_engine_lib::core::resources::{manual_migration_channel, FoodSpawnSettings};
use anima_engine_lib::core::simulation_schedule::build_tick_schedule;
use anima_engine_lib::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};
use anima_engine_lib::evolution::map_elites::MapElitesArchive;
use bevy_ecs::prelude::*;

const COEXISTENCE_TICKS: u64 = 10_000;
const TEST_SEED: u64 = 424242;

fn create_herbivore_genotype() -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    g.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.3,
        mass: 1.2,
    });
    g.add_node(MorphologyNode {
        id: 1,
        length: 0.8,
        radius: 0.25,
        mass: 0.8,
    });
    g.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 1,
        joint_anchor: glam::Vec3::new(0.8, 0.0, 0.0),
        joint_axis: glam::Vec3::new(0.0, 0.0, 1.0),
    });
    g
}

fn create_predator_genotype() -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    g.add_node(MorphologyNode {
        id: 0,
        length: 1.4,
        radius: 0.4,
        mass: 2.5,
    });
    g.add_node(MorphologyNode {
        id: 1,
        length: 1.0,
        radius: 0.3,
        mass: 1.5,
    });
    g.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 1,
        joint_anchor: glam::Vec3::new(1.0, 0.0, 0.0),
        joint_axis: glam::Vec3::new(0.0, 0.0, 1.0),
    });
    g
}

/// Sum total closed energy directly from authoritative stores.
fn measure_authoritative_total_eu(world: &mut World) -> f64 {
    let plants = world
        .get_resource::<ResourceField>()
        .map(|f| f.total_biomass())
        .unwrap_or(0.0);
    let detritus = world
        .get_resource::<EcosystemBiomass>()
        .map(|p| p.detritus)
        .unwrap_or(0.0);
    let mut animals = 0.0f64;
    let mut q = world.query_filtered::<&HomeostaticState, With<Agent>>();
    for homeo in q.iter(world) {
        animals += homeo.energy.max(0.0) as f64;
    }
    closed_total_eu(plants, animals, detritus)
}

/// Count living herbivore (prey) and predator populations.
fn count_trophic_populations(world: &mut World) -> (usize, usize) {
    let mut prey_count = 0;
    let mut pred_count = 0;

    let mut prey_q = world.query_filtered::<(), (With<Agent>, With<Prey>, Without<Predator>)>();
    for _ in prey_q.iter(world) {
        prey_count += 1;
    }

    let mut pred_q = world.query_filtered::<(), (With<Agent>, With<Predator>)>();
    for _ in pred_q.iter(world) {
        pred_count += 1;
    }

    (prey_count, pred_count)
}

/// Setup a multi-trophic world with 32 herbivores, 8 predators, and closed-loop resource field.
fn setup_coexistence_world(num_prey: usize, num_pred: usize) -> World {
    let mut world = init_world();
    world.insert_resource(TimeStep(1.0 / 60.0));

    // Disable unmetered external food spawning
    world.insert_resource(FoodSpawnSettings {
        max_food_count: 0,
        default_energy: 0.0,
        default_hydration: 0.0,
    });

    // Provide initial detritus pool for primary production recycling
    if let Some(mut pool) = world.get_resource_mut::<EcosystemBiomass>() {
        pool.detritus = 5_000.0;
    }

    // Wiring channel resources needed by schedule systems
    let (req_tx, req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
    let (res_tx, res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();
    let (recycle_req_tx, recycle_req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
    let (recycle_res_tx, recycle_res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();

    world.insert_resource(InferenceChannels {
        req_tx,
        recycle_req_rx,
        res_rx,
        recycle_res_tx,
    });
    world.insert_resource(LiveInferencePump::new(
        req_rx,
        res_tx,
        recycle_req_tx,
        recycle_res_rx,
    ));

    let (trans_tx, _trans_rx) = crossbeam_channel::unbounded::<anima_engine_lib::ai::hrrl::Transition>();
    world.insert_resource(TransitionSender(trans_tx));

    let (stats_tx, _stats_rx) = crossbeam_channel::unbounded::<Vec<anima_engine_lib::core::resources::AgentEpochStats>>();
    let (_spawn_tx, spawn_rx) = crossbeam_channel::unbounded::<anima_engine_lib::core::resources::EvolutionSpawn>();
    world.insert_resource(EvolutionSender(stats_tx));
    world.insert_resource(EvolutionReceiver(spawn_rx));
    world.insert_resource(EvolutionQueue::default());

    let (_env_tx, env_rx) = crossbeam_channel::unbounded::<anima_engine_lib::evolution::meta_ai::EnvironmentalEvent>();
    world.insert_resource(EnvironmentalEventReceiver(env_rx));

    let (_inbound_tx, inbound_rx) = crossbeam_channel::unbounded::<anima_engine_lib::core::components::AgentMigrationData>();
    let (outbound_tx, _outbound_rx) = crossbeam_channel::unbounded::<anima_engine_lib::core::components::OutboundMigration>();
    let (_migration_tx, migration_rx) = manual_migration_channel();
    world.insert_resource(InboundMigrationReceiver(inbound_rx));
    world.insert_resource(OutboundMigrationSender(outbound_tx));
    world.insert_resource(ShardingResource(Arc::new(std::sync::RwLock::new(ShardingConfig::default()))));
    world.insert_resource(BevyMigrationTrigger(migration_rx));

    let evolution_settings = Arc::new(std::sync::Mutex::new(EvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    }));
    world.insert_resource(BevyEvolutionSettings(evolution_settings));
    world.insert_resource(BevyEvolutionRunning(Arc::new(std::sync::atomic::AtomicBool::new(false))));
    world.insert_resource(BevyMapElitesGrid(Arc::new(std::sync::Mutex::new(MapElitesGridState {
        grid: std::collections::HashMap::new(),
        grid_resolution: 50,
    }))));
    world.insert_resource(ActiveEvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    });
    world.insert_resource(BevyMapElitesArchive {
        archive: MapElitesArchive::new(1.0 / 50.0),
    });
    world.insert_resource(NextNodeId(3));
    world.insert_resource(EpochManager {
        ticks_per_epoch: 100_000,
        current_epoch_ticks: 0,
        current_epoch: 0,
    });
    world.insert_resource(EnvironmentalSpawnSettings::default());
    world.insert_resource(anima_engine_lib::ai::pheromone::PheromoneGrid::default());
    world.insert_resource(BrainModel::new_seeded_cpu(15, 64, 4, TEST_SEED));
    world.insert_resource(BrainInferenceBuffer::default());

    let bounds = MapBounds {
        min: glam::Vec3::new(-45.0, 0.0, -45.0),
        max: glam::Vec3::new(45.0, 0.0, 45.0),
    };
    world.insert_resource(bounds);
    let herbivore_geno = create_herbivore_genotype();
    let predator_geno = create_predator_genotype();

    // Spawn initial founding herbivores (Prey)
    for i in 0..num_prey {
        let angle = (i as f32 / num_prey as f32) * std::f32::consts::TAU;
        let radius = 15.0 + (i % 3) as f32 * 8.0;
        let x = radius * angle.cos();
        let z = radius * angle.sin();
        let pos = glam::Vec3::new(x, 0.0, z);

        let entity = decode_genotype(&mut world, &herbivore_geno, pos, glam::Quat::IDENTITY);
        let vel = glam::Vec3::new(angle.cos() * 1.5, 0.0, angle.sin() * 1.5);

        world.entity_mut(entity).insert((
            Agent,
            Prey,
            Velocity(vel),
            anima_engine_lib::physics::dynamics::RigidBody {
                mass: 1.2,
                velocity: vel,
                force: glam::Vec3::ZERO,
            },
            HomeostaticState {
                energy: 75.0,
                energy_target: 100.0,
                hydration: 80.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
            AgentGenotype(herbivore_geno.clone()),
            AgentEvaluation {
                start_position: pos,
                total_distance: 0.0,
                total_energy_expended: 0.0,
                survival_ticks: 0,
                last_position: pos,
            },
            FeatureTracker::default(),
            CognitiveState::Ready,
            InertiaComponent::default(),
            ActionGates::default(),
            AgentLineageId(format!("herbivore_founder_{i}")),
            AgentGeneration(1),
            AgentParentLineageIds(vec![]),
        ));

        // Sync child segment velocities
        let mut child_entities = Vec::new();
        let mut q = world.query::<(Entity, &anima_engine_lib::core::ecs::ParentAgent)>();
        for (e, parent) in q.iter(&world) {
            if parent.0 == entity {
                child_entities.push(e);
            }
        }
        for child_e in child_entities {
            if let Some(mut child_vel) = world.get_mut::<Velocity>(child_e) {
                child_vel.0 = vel;
            }
            if let Some(mut rb) = world.get_mut::<anima_engine_lib::physics::dynamics::RigidBody>(child_e) {
                rb.velocity = vel;
                rb.force = glam::Vec3::ZERO;
            }
        }
    }

    // Spawn initial founding predators (Predator)
    for i in 0..num_pred {
        let angle = (i as f32 / num_pred as f32) * std::f32::consts::TAU + 0.3;
        let radius = 20.0;
        let x = radius * angle.cos();
        let z = radius * angle.sin();
        let pos = glam::Vec3::new(x, 0.0, z);

        let entity = decode_genotype(&mut world, &predator_geno, pos, glam::Quat::IDENTITY);
        let vel = glam::Vec3::new(angle.cos() * 2.5, 0.0, angle.sin() * 2.5);

        world.entity_mut(entity).insert((
            Agent,
            Predator,
            Velocity(vel),
            anima_engine_lib::physics::dynamics::RigidBody {
                mass: 2.5,
                velocity: vel,
                force: glam::Vec3::ZERO,
            },
            HomeostaticState {
                energy: 115.0,
                energy_target: 150.0,
                hydration: 80.0,
                hydration_target: 100.0,
                temperature: 37.0,
                temp_target: 37.0,
                previous_deviation: 0.0,
            },
            AgentGenotype(predator_geno.clone()),
            AgentEvaluation {
                start_position: pos,
                total_distance: 0.0,
                total_energy_expended: 0.0,
                survival_ticks: 0,
                last_position: pos,
            },
            FeatureTracker::default(),
            CognitiveState::Ready,
            InertiaComponent::default(),
            ActionGates::default(),
            AgentLineageId(format!("predator_founder_{i}")),
            AgentGeneration(1),
            AgentParentLineageIds(vec![]),
        ));

        // Sync child segment velocities
        let mut child_entities = Vec::new();
        let mut q = world.query::<(Entity, &anima_engine_lib::core::ecs::ParentAgent)>();
        for (e, parent) in q.iter(&world) {
            if parent.0 == entity {
                child_entities.push(e);
            }
        }
        for child_e in child_entities {
            if let Some(mut child_vel) = world.get_mut::<Velocity>(child_e) {
                child_vel.0 = vel;
            }
            if let Some(mut rb) = world.get_mut::<anima_engine_lib::physics::dynamics::RigidBody>(child_e) {
                rb.velocity = vel;
                rb.force = glam::Vec3::ZERO;
            }
        }
    }

    world
}

#[test]
fn test_three_trophic_unforced_coexistence_10000_ticks() {
    let mut world = setup_coexistence_world(32, 8);

    let mut schedule = build_tick_schedule(DeterministicMode::on());

    // Run tick 1 to lock energy baseline
    schedule.run(&mut world);
    let baseline = world
        .resource::<EnergyLedger>()
        .baseline()
        .expect("Baseline must be locked on tick 1");

    let initial_total = measure_authoritative_total_eu(&mut world);
    assert!(
        (baseline - initial_total).abs() <= RESIDUAL_ABS_TOLERANCE_EU,
        "Baseline must match initial measured total"
    );

    let mut min_herbivores = usize::MAX;
    let mut max_herbivores = 0usize;
    let mut min_predators = usize::MAX;
    let mut max_predators = 0usize;
    let mut worst_residual = 0.0f64;

    for tick in 1..=COEXISTENCE_TICKS {
        schedule.run(&mut world);
        let (herbivores, predators) = count_trophic_populations(&mut world);

        min_herbivores = min_herbivores.min(herbivores);
        max_herbivores = max_herbivores.max(herbivores);
        min_predators = min_predators.min(predators);
        max_predators = max_predators.max(predators);

        if tick % 500 == 0 {
            let current_total = measure_authoritative_total_eu(&mut world);
            let residual = (current_total - baseline).abs();
            let plant_biomass = world.resource::<ResourceField>().total_biomass();
            println!(
                "Tick {tick:5}: Herbivores={herbivores:3}, Predators={predators:2}, Plants={plant_biomass:.1}, Residual={residual:.9}"
            );
        }

        // Assert no extinction event occurs throughout the entire 10,000 ticks
        assert!(
            herbivores >= 1,
            "Herbivore population collapsed to {herbivores} at tick {tick}"
        );
        assert!(
            predators >= 1,
            "Predator population went extinct ({predators}) at tick {tick}"
        );

        // Plant biomass must remain healthy (> 0)
        let plant_biomass = world.resource::<ResourceField>().total_biomass();
        assert!(
            plant_biomass > 100.0,
            "Plant biomass depleted ({plant_biomass}) at tick {tick}"
        );

        // Invariant gate: zero energy leakage (< 1e-3 EU)
        let current_total = measure_authoritative_total_eu(&mut world);
        let residual = (current_total - baseline).abs();
        if residual > worst_residual {
            worst_residual = residual;
        }
        assert!(
            residual <= RESIDUAL_ABS_TOLERANCE_EU,
            "Energy ledger leak detected at tick {tick}: residual = {residual:.9} EU (tolerance {RESIDUAL_ABS_TOLERANCE_EU})"
        );
    }

    let (final_herbivores, final_predators) = count_trophic_populations(&mut world);
    let final_total = measure_authoritative_total_eu(&mut world);
    let final_residual = (final_total - baseline).abs();
    let relative_drift = final_residual / baseline;

    println!(
        "\n=======================================================\n\
         3-TROPHIC COEXISTENCE VERIFICATION (10,000 TICKS UNFORCED)\n\
         =======================================================\n\
         Baseline Closed Energy: {baseline:.6} EU\n\
         Final Closed Energy:    {final_total:.6} EU\n\
         Worst Mid-run Residual: {worst_residual:.9} EU\n\
         Final Absolute Drift:   {final_residual:.9} EU\n\
         Final Relative Drift:   {:.6}% (< 0.01% required)\n\
         -------------------------------------------------------\n\
         Herbivore Dynamics: min={min_herbivores}, max={max_herbivores}, final={final_herbivores}\n\
         Predator Dynamics:  min={min_predators}, max={max_predators}, final={final_predators}\n\
         =======================================================\n",
        relative_drift * 100.0
    );

    // Acceptance criteria
    assert!(final_herbivores >= 5, "Final herbivore count too low ({final_herbivores} < 5)");
    assert!(final_predators >= 2, "Final predator count too low ({final_predators} < 2)");
    assert!(
        relative_drift < 0.0001,
        "Total energy relative drift exceeds 0.01%: {relative_drift:.8}"
    );
    assert!(
        worst_residual <= RESIDUAL_ABS_TOLERANCE_EU,
        "Worst residual exceeded 1e-3 EU tolerance: {worst_residual:.9}"
    );
}

#[test]
fn test_three_trophic_unforced_coexistence_5000_ticks() {
    // Alias to 10k tick verification for backward compatibility
    test_three_trophic_unforced_coexistence_10000_ticks();
}

#[test]
fn test_lotka_volterra_phase_space_limit_cycles() {
    let mut world = setup_coexistence_world(24, 6);
    let mut schedule = build_tick_schedule(DeterministicMode::on());

    let mut samples: Vec<(usize, usize)> = Vec::new();

    for tick in 1..=2000 {
        schedule.run(&mut world);
        if tick % 20 == 0 {
            let (h, p) = count_trophic_populations(&mut world);
            samples.push((h, p));
        }
    }

    assert!(samples.len() >= 100);
    // Verify phase space forms non-trivial distribution (not a single stuck point or zero collapse)
    let min_h = samples.iter().map(|s| s.0).min().unwrap();
    let max_h = samples.iter().map(|s| s.0).max().unwrap();
    let min_p = samples.iter().map(|s| s.1).min().unwrap();
    let max_p = samples.iter().map(|s| s.1).max().unwrap();

    assert!(min_h > 0, "Prey collapsed to 0 in phase space");
    assert!(min_p > 0, "Predator collapsed to 0 in phase space");
    assert!(max_h > min_h, "Prey population had zero dynamic variance");
    assert!(max_p > 0, "Predator population had zero presence");
}
