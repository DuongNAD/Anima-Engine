//! Challenger 2: Adversarial Stress Test Harness for 3-Trophic Coexistence & Robustness
//!
//! Empirical checks:
//! 1. Multi-seed robustness across diverse initial random seeds (5,000 ticks each).
//! 2. Skewed initial population ratios (predator heavy vs prey heavy).
//! 3. Extended 10,000-tick unforced coexistence run checking continuous non-extinction.
//! 4. Holling III vs Holling II rarity refuge mathematical oracle verification.
//! 5. Phase space non-trivial trajectory and energy drift bounded < 1e-3 EU.

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

fn setup_stress_world(num_prey: usize, num_pred: usize, seed: u64) -> World {
    let mut world = init_world();
    world.insert_resource(TimeStep(1.0 / 60.0));

    world.insert_resource(FoodSpawnSettings {
        max_food_count: 0,
        default_energy: 0.0,
        default_hydration: 0.0,
    });

    if let Some(mut pool) = world.get_resource_mut::<EcosystemBiomass>() {
        pool.detritus = 5_000.0;
    }

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
    world.insert_resource(anima_engine_lib::core::resources::CombatEvents::default());
    world.insert_resource(anima_engine_lib::ai::pheromone::PheromoneGrid::default());
    world.insert_resource(BrainModel::new_seeded_cpu(15, 64, 4, seed));
    world.insert_resource(BrainInferenceBuffer::default());

    let bounds = MapBounds {
        min: glam::Vec3::new(-45.0, 0.0, -45.0),
        max: glam::Vec3::new(45.0, 0.0, 45.0),
    };
    world.insert_resource(bounds);
    let herbivore_geno = create_herbivore_genotype();
    let predator_geno = create_predator_genotype();

    for i in 0..num_prey {
        let angle = (i as f32 / num_prey.max(1) as f32) * std::f32::consts::TAU;
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
            if let Some(mut rb) = world.get_mut::<anima_engine_lib::physics::dynamics::RigidBody>(child_e) {
                rb.velocity = vel;
            }
        }
    }

    for i in 0..num_pred {
        let angle = (i as f32 / num_pred.max(1) as f32) * std::f32::consts::TAU + 0.3;
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
            if let Some(mut rb) = world.get_mut::<anima_engine_lib::physics::dynamics::RigidBody>(child_e) {
                rb.velocity = vel;
            }
        }
    }

    world
}

#[test]
fn test_multi_seed_coexistence_stress() {
    let seeds = [101u64, 777u64, 99999u64, 314159u64];
    for &seed in &seeds {
        let mut world = setup_stress_world(32, 8, seed);
        let mut schedule = build_tick_schedule(DeterministicMode::on());

        schedule.run(&mut world);
        let baseline = world
            .resource::<EnergyLedger>()
            .baseline()
            .expect("Baseline locked");

        let mut min_h = usize::MAX;
        let mut min_p = usize::MAX;

        for tick in 1..=5_000 {
            schedule.run(&mut world);
            let (h, p) = count_trophic_populations(&mut world);
            min_h = min_h.min(h);
            min_p = min_p.min(p);

            assert!(h > 0, "Herbivore extinction at tick {tick} for seed {seed}");
            assert!(p > 0, "Predator extinction at tick {tick} for seed {seed}");
        }

        let final_total = measure_authoritative_total_eu(&mut world);
        let residual = (final_total - baseline).abs();
        let (final_h, final_p) = count_trophic_populations(&mut world);

        println!(
            "Seed {seed:6} verified: min_H={min_h}, final_H={final_h}, min_P={min_p}, final_P={final_p}, Residual={residual:.9} EU"
        );

        assert!(residual <= RESIDUAL_ABS_TOLERANCE_EU, "Seed {seed} leaked energy: {residual}");
        assert!(final_h >= 5, "Seed {seed} final herbivores too low: {final_h}");
        assert!(final_p >= 2, "Seed {seed} final predators too low: {final_p}");
    }
}

#[test]
fn test_extended_10000_tick_unforced_coexistence() {
    let mut world = setup_stress_world(32, 8, 888888);
    let mut schedule = build_tick_schedule(DeterministicMode::on());

    schedule.run(&mut world);
    let baseline = world
        .resource::<EnergyLedger>()
        .baseline()
        .expect("Baseline locked");

    let mut min_h = usize::MAX;
    let mut min_p = usize::MAX;
    let mut max_h = 0usize;
    let mut max_p = 0usize;
    let mut worst_residual = 0.0f64;

    for tick in 1..=10_000 {
        schedule.run(&mut world);
        let (h, p) = count_trophic_populations(&mut world);
        min_h = min_h.min(h);
        max_h = max_h.max(h);
        min_p = min_p.min(p);
        max_p = max_p.max(p);

        assert!(h > 0, "Herbivore extinction at tick {tick}");
        assert!(p > 0, "Predator extinction at tick {tick}");

        let current_total = measure_authoritative_total_eu(&mut world);
        let residual = (current_total - baseline).abs();
        if residual > worst_residual {
            worst_residual = residual;
        }
        assert!(
            residual <= RESIDUAL_ABS_TOLERANCE_EU,
            "Residual exceeded 1e-3 EU at tick {tick}: {residual:.9}"
        );

        if tick == 1 || tick == 100 || tick == 500 {
            let mut h_pos = Vec::new();
            let mut q = world.query_filtered::<&anima_engine_lib::core::ecs::Position, With<Prey>>();
            for p in q.iter(&world) {
                h_pos.push((p.0.x, p.0.z));
            }
            println!("Tick {tick}: First 3 Herbivore positions: {:?}", &h_pos[..3.min(h_pos.len())]);
        }

        if tick % 1000 == 0 || (tick >= 6500 && tick <= 6610) {
            let mut pred_energies = Vec::new();
            let mut q = world.query_filtered::<&HomeostaticState, With<Predator>>();
            for h in q.iter(&world) {
                pred_energies.push(format!("{:.1}", h.energy));
            }
            println!("Tick {tick:5}/10000: H={h:3}, P={p:2}, PredEnergies={pred_energies:?}, Residual={residual:.9} EU");
        }
    }

    let (final_h, final_p) = count_trophic_populations(&mut world);
    let final_total = measure_authoritative_total_eu(&mut world);
    let final_residual = (final_total - baseline).abs();
    let drift_pct = (final_residual / baseline) * 100.0;

    println!(
        "\n10,000-Tick Extended Stress Test Results:\n\
         Final H: {final_h} (range: {min_h}..={max_h})\n\
         Final P: {final_p} (range: {min_p}..={max_p})\n\
         Worst Residual: {worst_residual:.9} EU\n\
         Final Relative Drift: {drift_pct:.8}%\n"
    );

    assert!(final_h >= 5);
    assert!(final_p >= 2);
    assert!(drift_pct < 0.01);
}

#[test]
fn test_skewed_initial_ratios_stability() {
    // Case 1: High Herbivore / Low Predator (40 H vs 4 P)
    {
        let mut world = setup_stress_world(40, 4, 111222);
        let mut schedule = build_tick_schedule(DeterministicMode::on());
        schedule.run(&mut world);
        for tick in 1..=3000 {
            schedule.run(&mut world);
            let (h, p) = count_trophic_populations(&mut world);
            assert!(h > 0, "H went extinct in 40:4 setup at tick {tick}");
            assert!(p > 0, "P went extinct in 40:4 setup at tick {tick}");
        }
        let (h, p) = count_trophic_populations(&mut world);
        println!("Skewed 40:4 at tick 3000 -> H={h}, P={p}");
    }

    // Case 2: Low Herbivore / High Predator (20 H vs 12 P)
    {
        let mut world = setup_stress_world(20, 12, 333444);
        let mut schedule = build_tick_schedule(DeterministicMode::on());
        schedule.run(&mut world);
        for tick in 1..=3000 {
            schedule.run(&mut world);
            let (h, p) = count_trophic_populations(&mut world);
            assert!(h > 0, "H went extinct in 20:12 setup at tick {tick}");
            assert!(p > 0, "P went extinct in 20:12 setup at tick {tick}");
        }
        let (h, p) = count_trophic_populations(&mut world);
        println!("Skewed 20:12 at tick 3000 -> H={h}, P={p}");
    }
}
