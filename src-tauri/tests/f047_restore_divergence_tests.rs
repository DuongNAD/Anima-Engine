//! F047 / F048: live-adapter restore fidelity and sibling-collision fix for pending-inference restore.
//!
//! 1. `LiveExperimentAdapter` (`R3-like` shared/evolved and `S12/S13` worlds): zero restore
//!    divergence at every tested checkpoint because `live_inference_pump_system` answers every
//!    request inside the same tick before `action_resolution_system` drains responses.
//! 2. Desktop-app `AppWorldHarness` (`R2a` and `R3`): uses the shared
//!    `route_restored_pending_inference` helper (`simulation_state.rs`) to key restored responses
//!    by `(lineage_id, request_id)` matching each waiting entity's
//!    `CognitiveState::PendingInference(ticket_id)`, eliminating sibling collision at odd and even
//!    checkpoints across platforms (`div == None` everywhere).
//! 3. Disk snapshot round-trip (`SavedSharedLearningState` + `snapshot::read`): verifies two
//!    same-lineage siblings simultaneously in `PendingInference` save, seal, load cleanly via
//!    `snapshot::read` without `InvalidData`, route to their exact entity handles, and drop/count
//!    unmatched responses without ever routing to a sibling.

use anima_engine_lib::ai::hrrl::{LearningQueueSnapshot, Transition};
use anima_engine_lib::ai::model::{
    run_inference_batch, ActorCriticModel, BrainInferenceBuffer, BrainModel, InferenceScratch,
};
use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};
use anima_engine_lib::core::agent_systems::{
    ActiveEvolutionSettings, AgentEvaluation, AgentGeneration, AgentGenotype, AgentLineageId,
    BevyAppHandle, BevyEvolutionRunning, BevyEvolutionSettings, BevyMapElitesArchive,
    BevyMapElitesGrid, DeferredEnvironmentalEvent, DormantAgents, EnvironmentalEventReceiver,
    InferenceChannels, InferenceRequestBatch, InferenceResponseBatch, NextNodeId, ACTION_SLOTS,
};
use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::components::{ActionGates, CognitiveState};
use anima_engine_lib::core::determinism::{next_entity_id, DeterministicMode, RunIdentity};
use anima_engine_lib::core::ecs::{
    init_world, ActiveEnvironmentEvent, AgentClass, AgentParentLineageIds,
    EnvironmentalSpawnSettings, EpochManager, FeatureTracker, Food, Lake, MapBounds, Position,
    Predator, Prey, ScientificCounterFault, Tree,
};
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{LiveExperimentAdapter, LiveSnapshot};
use anima_engine_lib::core::observer::{ObserverPolicy, ObserverTrace, SharedObserverActions};
use anima_engine_lib::core::resources::{
    BrainPolicy, EvolutionQueue, EvolutionReceiver, EvolutionSender, FoundingPlan,
    LocomotionPolicy, MigrationHandoffDiagnostics, MortalityPolicy, ReproductionPolicy, SimRng,
};
use anima_engine_lib::core::sim_clock::SimClock;
use anima_engine_lib::core::simulation_lod::{LodBands, LodFocus, SharedLodFocus};
use anima_engine_lib::core::simulation_schedule::build_tick_schedule;
use anima_engine_lib::core::simulation_state::{
    empty_saved_state_for_tests, populate_restored_spatial_grid, restore_energy_state,
    restored_next_node_id, route_restored_pending_inference, serialize_world_state,
    spawn_serialized_agent, SavedInferenceResponse, SavedInferenceResponseBatch,
    SavedSharedLearningState, SavedSimulationState, SerializedAgent,
};
use anima_engine_lib::core::snapshot::{self, world_checksum, SnapshotEnvelope, SCHEMA_VERSION};
use anima_engine_lib::core::training::{
    ModelUpdateDiagnostics, ModelUpdateSnapshot, SavedLearnerWorkerState, ACTION_DIM, HIDDEN_DIM,
    STATE_DIM,
};
use anima_engine_lib::evolution::fitness::FitnessRuleSetting;
use anima_engine_lib::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};
use anima_engine_lib::evolution::lineage::{FallbackLineageTracker, LineageTracker};
use anima_engine_lib::physics::spatial::{SpatialCollider, SpatialHashGrid};
use bevy_ecs::prelude::{Schedule, World};
use burn::module::Module;
use burn::optim::AdamConfig;
use burn::tensor::Device;
use rand::seq::SliceRandom;
use rand::{rngs::StdRng, SeedableRng};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

static DISK_SEQ: AtomicU64 = AtomicU64::new(0);

fn through_disk_live(snap: &LiveSnapshot, tag: &str) -> LiveSnapshot {
    let seq = DISK_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("anima_f047_{}_{}_{}", std::process::id(), seq, tag));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp checkpoint dir");
    let path = dir.join("checkpoint.json");
    let envelope = SnapshotEnvelope::seal(snap.saved.clone()).expect("seal live snapshot");
    snapshot::write_atomic(&path, &envelope).expect("write live snapshot");
    let back = snapshot::read(&path).expect("read live snapshot");
    let _ = std::fs::remove_dir_all(&dir);
    LiveSnapshot {
        saved: back,
        config: snap.config,
        seed: snap.seed,
    }
}

fn measure_live_adapter_checkpoints(
    label: &str,
    initial: &InitialConditionSet,
    seed: u64,
    horizon: u64,
    checkpoints: &[u64],
) {
    let laws = WorldLawSet::baseline();
    let mut reference =
        LiveExperimentAdapter::from_manifest(&laws, initial, &[], seed, (16, 16), horizon)
            .expect("build reference live world");
    let mut clock_ref = SimClock::new();
    let mut ledger_ref = CausalLedger::new();
    let mut rng_ref = StdRng::seed_from_u64(seed);
    let mut ref_checksums = Vec::with_capacity((horizon + 1) as usize);
    ref_checksums.push(reference.checksum());
    for _ in 1..=horizon {
        let _ = clock_ref.advance();
        reference.step(&clock_ref, &[], &mut ledger_ref, &mut rng_ref);
        let _ = reference.observables();
        ref_checksums.push(reference.checksum());
    }

    let max_k = *checkpoints.iter().max().unwrap_or(&0);
    let mut interrupted =
        LiveExperimentAdapter::from_manifest(&laws, initial, &[], seed, (16, 16), horizon)
            .expect("build checkpointed live world");
    let mut clock_int = SimClock::new();
    let mut ledger_int = CausalLedger::new();
    let mut rng_int = StdRng::seed_from_u64(seed);
    let mut snaps = Vec::new();
    for t in 1..=max_k {
        let _ = clock_int.advance();
        interrupted.step(&clock_int, &[], &mut ledger_int, &mut rng_int);
        let _ = interrupted.observables();
        if checkpoints.contains(&t) {
            assert_eq!(
                interrupted.checksum(),
                ref_checksums[t as usize],
                "{label} prefix must match uninterrupted reference at K={t}"
            );
            let snap = through_disk_live(&interrupted.snapshot(), &format!("{label}_{t}"));
            snaps.push((t, snap, ledger_int.clone(), rng_int.clone()));
        }
    }

    for (k, snap, mut ledger_res, mut rng_res) in snaps {
        let mut resumed = LiveExperimentAdapter::from_snapshot(&snap).expect("restore live world");
        assert_eq!(
            resumed.checksum(),
            ref_checksums[k as usize],
            "{label} immediate restored checksum must match uninterrupted at K={k}"
        );
        let mut clock_res = SimClock { tick: k };
        let mut first_div = None;
        for &expected in &ref_checksums[(k + 1) as usize..=horizon as usize] {
            let tick = clock_res.advance();
            resumed.step(&clock_res, &[], &mut ledger_res, &mut rng_res);
            let _ = resumed.observables();
            let actual = resumed.checksum();
            if first_div.is_none() && actual != expected {
                first_div = Some((tick, expected, actual));
                break;
            }
        }
        assert_eq!(
            first_div, None,
            "{label} restore at K={k} must match uninterrupted run through tick {horizon}"
        );
    }
}

#[test]
fn f047_live_adapter_r3_like_restore_matches_uninterrupted() {
    let r3_shared = InitialConditionSet::new(vec![
        ("live.founders".into(), 30.0),
        ("live.predator_fraction".into(), 0.25),
        ("live.locomotion".into(), 1.0),
        ("live.drive_floor".into(), 0.5),
        ("live.thirst_steering".into(), 1.0),
        ("live.mortality".into(), 1.0),
        ("live.reproduction".into(), 1.0),
    ]);
    measure_live_adapter_checkpoints("live_r3_like_shared", &r3_shared, 3, 1000, &[1, 899, 900]);

    let r3_evolved = InitialConditionSet::new(vec![
        ("live.founders".into(), 30.0),
        ("live.predator_fraction".into(), 0.25),
        ("live.evolved_brains".into(), 1.0),
        ("live.locomotion".into(), 1.0),
        ("live.drive_floor".into(), 0.5),
        ("live.thirst_steering".into(), 1.0),
        ("live.mortality".into(), 1.0),
        ("live.reproduction".into(), 1.0),
    ]);
    measure_live_adapter_checkpoints("live_r3_like_evolved", &r3_evolved, 3, 1000, &[899, 900]);
}

#[test]
fn f047_live_adapter_s12_s13_world_restore_matches_uninterrupted() {
    let s12_s13 = InitialConditionSet::new(vec![
        ("live.founders".into(), 100.0),
        ("live.predator_fraction".into(), 0.0),
        ("live.trees".into(), 8.0),
        ("live.lakes".into(), 16.0),
        ("live.lake_layout".into(), 1.0),
        ("live.mortality".into(), 1.0),
        ("live.reproduction".into(), 1.0),
        ("live.locomotion".into(), 1.0),
        ("live.drive_floor".into(), 0.5),
        ("live.thirst_steering".into(), 1.0),
        ("live.prey_cap".into(), 320.0),
        ("live.resource_capacity_scale".into(), 0.63),
        ("live.dehydration_ticks".into(), 960.0),
    ]);
    measure_live_adapter_checkpoints("live_s12_s13", &s12_s13, 3, 1000, &[899, 900]);
}

// ---- Desktop-app headless lockstep harness & shared pending-inference restore --------------------

struct AppConfig {
    label: &'static str,
    founders: usize,
    max_tree_count: usize,
    mortality: bool,
    reproduction: bool,
    locomotion: bool,
}

struct AppWorldHarness {
    world: World,
    schedule: Schedule,
    brain_model: BrainModel,
    inference_scratch: InferenceScratch,
    req_rx: crossbeam_channel::Receiver<InferenceRequestBatch>,
    res_tx: crossbeam_channel::Sender<InferenceResponseBatch>,
    recycle_req_tx: crossbeam_channel::Sender<InferenceRequestBatch>,
    recycle_res_rx: crossbeam_channel::Receiver<InferenceResponseBatch>,
    chronicle: Arc<RwLock<Vec<anima_engine_lib::core::simulation_state::ChronicleEvent>>>,
    lineage: Arc<FallbackLineageTracker>,
    evolution_settings: Arc<Mutex<EvolutionSettings>>,
    map_elites_grid: Arc<Mutex<MapElitesGridState>>,
    tick_count: u64,
}

fn default_founder_genotype() -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    g.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.2,
        mass: 1.5,
    });
    g.add_node(MorphologyNode {
        id: 1,
        length: 1.0,
        radius: 0.2,
        mass: 1.0,
    });
    g.add_node(MorphologyNode {
        id: 2,
        length: 1.0,
        radius: 0.2,
        mass: 0.8,
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
        joint_axis: glam::Vec3::new(0.0, 0.0, 1.0),
    });
    g
}

fn build_app_world(
    cfg: &AppConfig,
    restore: Option<(&SavedSimulationState, &[SavedInferenceResponseBatch])>,
) -> AppWorldHarness {
    let mut world = init_world();
    let loaded_bounds = restore.map(|(s, _)| s.map_bounds).unwrap_or_default();
    world.insert_resource(loaded_bounds);
    world.insert_resource(SpatialHashGrid::new_prepopulated(10.0, &loaded_bounds));

    let loaded_pheromone = restore
        .map(|(s, _)| anima_engine_lib::ai::pheromone::PheromoneGrid {
            values: s.pheromone_grid.values.clone(),
            scratch: vec![0.0; anima_engine_lib::ai::pheromone::CELL_COUNT],
            diffusion_rate: s.pheromone_grid.diffusion_rate,
            decay_rate: s.pheromone_grid.decay_rate,
        })
        .unwrap_or_default();
    world.insert_resource(loaded_pheromone);

    let run_seed = restore.map(|(s, _)| s.sim_rng_seed).unwrap_or_else(|| {
        world
            .get_resource::<SimRng>()
            .map(|r| r.seed())
            .unwrap_or(1)
    });
    world.insert_resource(SimRng::from_seed(run_seed));
    world.insert_resource(DeterministicMode::on());
    world.insert_resource(BrainPolicy {
        evolved: true,
        ..Default::default()
    });
    let brain_model = BrainModel::new_seeded_cpu(15, 64, 4, run_seed);
    world.insert_resource(BrainModel::new_seeded_cpu(15, 64, 4, run_seed));
    world.insert_resource(BrainInferenceBuffer::default());

    world.insert_resource(LodFocus::default());
    world.insert_resource(LodBands::default());
    world.insert_resource(SharedLodFocus(Arc::new(RwLock::new(LodFocus::default()))));
    world.insert_resource(ObserverPolicy::Inhabit {
        cause_id: anima_domain::causal::CAUSE_OBSERVER,
    });
    world.insert_resource(ObserverTrace::default());
    world.insert_resource(SharedObserverActions::new());

    let (req_tx, req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
    let (recycle_req_tx, recycle_req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
    let (res_tx, res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();
    let (recycle_res_tx, recycle_res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();
    for _ in 0..anima_engine_lib::core::agent_systems::INFERENCE_POOL_BATCHES {
        let _ = recycle_req_tx.send(InferenceRequestBatch {
            requests: Vec::with_capacity(128),
        });
        let _ = recycle_res_tx.send(InferenceResponseBatch {
            responses: Vec::with_capacity(128),
        });
    }
    world.insert_resource(InferenceChannels {
        req_tx,
        recycle_req_rx,
        res_rx,
        recycle_res_tx,
    });

    let loaded_food_settings = restore
        .map(|(s, _)| s.food_spawn_settings)
        .unwrap_or_default();
    world.insert_resource(loaded_food_settings);
    let env_settings = EnvironmentalSpawnSettings {
        max_tree_count: cfg.max_tree_count,
        default_seed_cooldown: 15.0,
        ..Default::default()
    };
    world.insert_resource(env_settings);

    let (trans_tx, _trans_rx) =
        crossbeam_channel::bounded::<anima_engine_lib::ai::hrrl::Transition>(4096);
    world.insert_resource(anima_engine_lib::ai::hrrl::TransitionSender(trans_tx));
    world.insert_resource(anima_engine_lib::ai::hrrl::LearningQueueDiagnostics::default());
    world.insert_resource(ModelUpdateDiagnostics::default());

    let evolution_settings = Arc::new(Mutex::new(EvolutionSettings {
        mutation_rate: 0.1,
        selection_bias: 1.0,
        grid_resolution: 40,
    }));
    let evolution_running = Arc::new(AtomicBool::new(false));
    let map_elites_grid = Arc::new(Mutex::new(MapElitesGridState {
        grid: HashMap::new(),
        grid_resolution: 40,
    }));
    world.insert_resource(BevyEvolutionSettings(Arc::clone(&evolution_settings)));
    world.insert_resource(BevyEvolutionRunning(Arc::clone(&evolution_running)));
    world.insert_resource(BevyMapElitesGrid(Arc::clone(&map_elites_grid)));
    world.insert_resource(BevyAppHandle::<tauri::test::MockRuntime>(None));
    world.insert_resource(MortalityPolicy {
        enabled: cfg.mortality,
        ..MortalityPolicy::disabled()
    });
    world.insert_resource(ReproductionPolicy {
        enabled: cfg.reproduction,
    });
    if cfg.locomotion {
        world.insert_resource(LocomotionPolicy {
            enabled: true,
            drive_floor: 0.5,
            thirst_steering: true,
        });
    } else {
        world.insert_resource(LocomotionPolicy::disabled());
    }
    world.insert_resource(FitnessRuleSetting::default());
    world.insert_resource(anima_engine_lib::core::resources::VitalStatistics::default());

    world.insert_resource(ActiveEvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 40,
    });
    world.insert_resource(BevyMapElitesArchive {
        archive: anima_engine_lib::evolution::map_elites::MapElitesArchive::new(1.0 / 40.0),
    });

    let next_node_id = restore.map(|(s, _)| restored_next_node_id(s)).unwrap_or(3);
    world.insert_resource(NextNodeId(next_node_id));

    let (stats_tx, _stats_rx) = crossbeam_channel::unbounded();
    let (_spawn_tx, spawn_rx) = crossbeam_channel::unbounded();
    let (_env_tx, env_rx) = crossbeam_channel::unbounded();
    world.insert_resource(EvolutionSender(stats_tx));
    world.insert_resource(EvolutionReceiver(spawn_rx));
    world.insert_resource(EnvironmentalEventReceiver(env_rx));
    world.insert_resource(DeferredEnvironmentalEvent::default());

    let loaded_epoch = restore
        .map(|(s, _)| s.epoch_manager)
        .unwrap_or(EpochManager {
            ticks_per_epoch: 1000,
            current_epoch_ticks: 0,
            current_epoch: 0,
        });
    world.insert_resource(loaded_epoch);
    world.insert_resource(ScientificCounterFault::default());
    world.insert_resource(EvolutionQueue::default());

    let (_in_tx, in_rx) = anima_engine_lib::core::resources::inbound_migration_channel();
    let (out_tx, _out_rx) = anima_engine_lib::core::resources::outbound_migration_channel();
    let (_mig_tx, mig_rx) = anima_engine_lib::core::resources::manual_migration_channel();
    world.insert_resource(anima_engine_lib::core::ecs::InboundMigrationReceiver(in_rx));
    world.insert_resource(anima_engine_lib::core::ecs::OutboundMigrationSender(out_tx));
    world.insert_resource(MigrationHandoffDiagnostics::default());
    world.insert_resource(anima_engine_lib::core::ecs::ShardingResource(Arc::new(
        RwLock::new(anima_engine_lib::core::ecs::ShardingConfig::default()),
    )));
    world.insert_resource(anima_engine_lib::core::ecs::BevyMigrationTrigger(mig_rx));

    let loaded_env = restore
        .map(|(s, _)| ActiveEnvironmentEvent(s.active_environment_event))
        .unwrap_or_default();
    world.insert_resource(loaded_env);

    let chronicle = Arc::new(RwLock::new(Vec::new()));
    let lineage = Arc::new(FallbackLineageTracker::new(
        "bolt://127.0.0.1:1",
        "neo4j",
        "password",
    ));

    if let Some((state, pending_batches)) = restore {
        lineage.load_state(state.lineage_nodes.clone(), state.lineage_relations.clone());
        *chronicle.write().expect("chronicle write lock") = state.chronicle_history.clone();
        for agent in &state.agents {
            spawn_serialized_agent(&mut world, agent).expect("spawn restored agent");
        }
        restore_energy_state(&mut world, state);
        world.insert_resource(DormantAgents(state.dormant_agents.clone()));
        for food in &state.foods {
            world.spawn((
                Food {
                    energy_value: food.energy_value,
                    hydration_value: food.hydration_value,
                },
                Position(food.position),
                SpatialCollider { radius: 0.5 },
            ));
        }
        for lake in &state.lakes {
            world.spawn((
                Lake {
                    current_water: lake.current_water,
                    max_water: lake.max_water,
                    replenishment_rate: lake.replenishment_rate,
                },
                Position(lake.position),
                SpatialCollider {
                    radius: lake.radius,
                },
            ));
        }
        for tree in &state.trees {
            world.spawn((
                Tree {
                    current_fruit: tree.current_fruit,
                    max_fruit: tree.max_fruit,
                    fruit_growth_rate: tree.fruit_growth_rate,
                    time_since_last_drop: tree.time_since_last_drop,
                    seed_drop_cooldown: tree.seed_drop_cooldown,
                    seed_spread_radius: tree.seed_spread_radius,
                },
                Position(tree.position),
                SpatialCollider {
                    radius: tree.radius,
                },
            ));
        }
        populate_restored_spatial_grid(&mut world, state);

        if !pending_batches.is_empty() {
            let (routed_batches, dropped) =
                route_restored_pending_inference(&mut world, pending_batches);
            assert_eq!(
                dropped, 0,
                "every checkpointed in-flight response must match its restored PendingInference entity"
            );
            for responses in routed_batches {
                let mut batch = recycle_res_rx.try_recv().expect("recycle res batch");
                batch.responses.clear();
                batch.responses.extend(responses);
                res_tx.send(batch).expect("restore pending inference");
            }
        }
    } else {
        let genotype = default_founder_genotype();
        let founding = if cfg.founders == 10 {
            FoundingPlan::default()
        } else {
            FoundingPlan::parse(&cfg.founders.to_string()).expect("parse founder count")
        };
        let bounds = *world.resource::<MapBounds>();
        let founder_ids = RunIdentity::new(run_seed, "founder");
        let deterministic_mode = *world.resource::<DeterministicMode>();

        for i in 0..founding.count {
            let initial_pos = founding.position(i, &bounds);
            let agent_entity =
                decode_genotype(&mut world, &genotype, initial_pos, glam::Quat::IDENTITY);
            let lineage_id = next_entity_id(deterministic_mode, &founder_ids);
            let _ = lineage.add_root(lineage_id.clone(), genotype.clone());
            world.entity_mut(agent_entity).insert((
                AgentGenotype(genotype.clone()),
                AgentEvaluation {
                    start_position: initial_pos,
                    total_distance: 0.0,
                    total_energy_expended: 0.0,
                    total_energy_gained: 0.0,
                    prey_energy_gained: 0.0,
                    survival_ticks: 0,
                    last_position: initial_pos,
                },
                FeatureTracker::default(),
                AgentLineageId(lineage_id),
                AgentGeneration(0),
                AgentParentLineageIds(Vec::new()),
            ));
            let prey_quota = (founding.count * 7) / 10;
            if i < prey_quota {
                world.entity_mut(agent_entity).insert(Prey);
            } else {
                world.entity_mut(agent_entity).insert(Predator);
            }
            let policy = *world.resource::<BrainPolicy>();
            if policy.evolved {
                let brain = world
                    .get_resource_mut::<SimRng>()
                    .and_then(|mut rng| policy.new_brain(rng.rng()));
                if let Some(b) = brain {
                    world.entity_mut(agent_entity).insert(b);
                }
            }
        }

        let mut lake_candidates = Vec::new();
        let mut tree_candidates = Vec::new();
        if let Some(tm) = world.get_resource::<anima_engine_lib::core::terrain::TerrainMap>() {
            for row in 0..tm.height {
                for col in 0..tm.width {
                    let idx = row * tm.width + col;
                    let biome = tm.biomes[idx];
                    let elevation = tm.elevations[idx];
                    let px = bounds.min.x
                        + ((col as f32 + 0.5) / tm.width as f32) * (bounds.max.x - bounds.min.x);
                    let pz = bounds.min.z
                        + ((row as f32 + 0.5) / tm.height as f32) * (bounds.max.z - bounds.min.z);
                    let pos = glam::Vec3::new(px, 0.0, pz);
                    if (biome == 0 || biome == 1 || biome == 3) && elevation < 0.4 {
                        lake_candidates.push(pos);
                    }
                    if biome == 5 || biome == 6 || biome == 7 {
                        tree_candidates.push(pos);
                    }
                }
            }
        }

        let mut rng = anima_engine_lib::core::resources::derived_rng(
            run_seed,
            anima_engine_lib::core::resources::sim_stream::WORLD_INIT,
        );
        if !lake_candidates.is_empty() {
            lake_candidates.shuffle(&mut rng);
            let n = 5.min(lake_candidates.len());
            for &pos in lake_candidates.iter().take(n) {
                world.spawn((
                    Lake {
                        current_water: env_settings.default_lake_water,
                        max_water: env_settings.default_lake_water,
                        replenishment_rate: env_settings.default_lake_replenish,
                    },
                    Position(pos),
                    SpatialCollider { radius: 10.0 },
                ));
            }
        }
        if !tree_candidates.is_empty() {
            tree_candidates.shuffle(&mut rng);
            let n = env_settings.max_tree_count.min(tree_candidates.len());
            for &pos in tree_candidates.iter().take(n) {
                world.spawn((
                    Tree {
                        current_fruit: env_settings.default_tree_fruit,
                        max_fruit: env_settings.default_tree_fruit,
                        fruit_growth_rate: env_settings.default_tree_growth,
                        time_since_last_drop: 0.0,
                        seed_drop_cooldown: env_settings.default_seed_cooldown,
                        seed_spread_radius: env_settings.default_seed_spread,
                    },
                    Position(pos),
                    SpatialCollider { radius: 2.0 },
                ));
            }
        }
    }

    let schedule = build_tick_schedule(DeterministicMode::on());
    let mut harness = AppWorldHarness {
        world,
        schedule,
        brain_model,
        inference_scratch: InferenceScratch::with_capacity(256),
        req_rx,
        res_tx,
        recycle_req_tx,
        recycle_res_rx,
        chronicle,
        lineage,
        evolution_settings,
        map_elites_grid,
        tick_count: restore.map(|(s, _)| s.tick_count).unwrap_or(0),
    };
    if restore.is_none() {
        harness.run_schedule_and_pump();
    }
    harness
}

impl AppWorldHarness {
    fn pump_inference(&mut self) {
        while let Ok(req_batch) = self.req_rx.try_recv() {
            if !req_batch.requests.is_empty() {
                let mut res_batch = self
                    .recycle_res_rx
                    .try_recv()
                    .expect("inference response pool non-empty");
                res_batch.responses.clear();
                run_inference_batch(
                    &self.brain_model,
                    &req_batch.requests,
                    &mut res_batch.responses,
                    &mut self.inference_scratch,
                );
                self.res_tx.send(res_batch).expect("send res batch");
            }
            let _ = self.recycle_req_tx.try_send(req_batch);
        }
    }

    fn run_schedule_and_pump(&mut self) {
        self.schedule.run(&mut self.world);
        self.pump_inference();
    }

    fn step_one(&mut self) {
        self.run_schedule_and_pump();
        self.tick_count += 1;
    }

    fn snapshot_and_roundtrip(
        &mut self,
        tag: &str,
    ) -> (SavedSimulationState, Vec<SavedInferenceResponseBatch>) {
        let res_rx = self
            .world
            .get_resource::<InferenceChannels>()
            .expect("inference channels")
            .res_rx
            .clone();
        let mut drained_raw = Vec::new();
        while let Ok(batch) = res_rx.try_recv() {
            drained_raw.push(batch);
        }
        let pending_batches: Vec<SavedInferenceResponseBatch> = drained_raw
            .iter()
            .map(|batch| {
                let responses = batch
                    .responses
                    .iter()
                    .filter_map(|r| {
                        self.world.get::<AgentLineageId>(r.entity).map(|lid| {
                            SavedInferenceResponse {
                                lineage_id: lid.0.clone(),
                                actions: r.actions,
                                request_id: r.request_id,
                            }
                        })
                    })
                    .collect();
                SavedInferenceResponseBatch { responses }
            })
            .collect();
        for batch in drained_raw {
            self.res_tx.send(batch).expect("re-enqueue pending batch");
        }

        let state = serialize_world_state(
            &mut self.world,
            self.tick_count,
            &self.chronicle,
            &self.lineage,
            &self.evolution_settings,
            &self.map_elites_grid,
        );
        let seq = DISK_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "anima_f047_app_{}_{}_{}",
            std::process::id(),
            seq,
            tag
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("checkpoint.json");
        let envelope = SnapshotEnvelope::seal(state).expect("seal app state");
        snapshot::write_atomic(&path, &envelope).expect("write app state");
        let read_back = snapshot::read(&path).expect("read app state");
        let _ = std::fs::remove_dir_all(&dir);
        (read_back, pending_batches)
    }
}

fn measure_app_restore(
    cfg: &AppConfig,
    k: u64,
    horizon: u64,
    orig_hashes: &[u32],
) -> Option<(u64, u32, u32)> {
    let mut orig = build_app_world(cfg, None);
    for _ in 1..=k {
        orig.step_one();
    }
    assert_eq!(
        world_checksum(&mut orig.world),
        orig_hashes[k as usize],
        "{} prefix at K={k} must match uninterrupted reference",
        cfg.label
    );
    let (saved, pending) = orig.snapshot_and_roundtrip(&format!("{}_{k}", cfg.label));
    let mut resumed = build_app_world(cfg, Some((&saved, &pending)));
    assert_eq!(
        world_checksum(&mut resumed.world),
        orig_hashes[k as usize],
        "{} immediate restored checksum at K={k} must match uninterrupted",
        cfg.label
    );

    for (t, &expected) in orig_hashes
        .iter()
        .enumerate()
        .take(horizon as usize + 1)
        .skip(k as usize + 1)
    {
        resumed.step_one();
        let rc = world_checksum(&mut resumed.world);
        if rc != expected {
            return Some((t as u64, expected, rc));
        }
    }
    None
}

fn uninterrupted_app_hashes(cfg: &AppConfig, horizon: u64) -> Vec<u32> {
    let mut orig_full = build_app_world(cfg, None);
    let mut hashes = Vec::with_capacity((horizon + 1) as usize);
    hashes.push(world_checksum(&mut orig_full.world));
    for _ in 1..=horizon {
        orig_full.step_one();
        hashes.push(world_checksum(&mut orig_full.world));
    }
    hashes
}

#[test]
fn f047_app_harness_r2a_restore_checkpoints() {
    let r2a = AppConfig {
        label: "R2a",
        founders: 10,
        max_tree_count: 80,
        mortality: false,
        reproduction: true,
        locomotion: false,
    };
    let horizon = 960;
    let orig_hashes = uninterrupted_app_hashes(&r2a, horizon);

    for &k in &[898, 899, 900] {
        let div = measure_app_restore(&r2a, k, horizon, &orig_hashes);
        assert_eq!(
            div, None,
            "R2a checkpoint K={k} must match uninterrupted run through tick {horizon}"
        );
    }
}

#[test]
fn f047_app_harness_r3_restore_checkpoints() {
    let r3 = AppConfig {
        label: "R3",
        founders: 30,
        max_tree_count: 80,
        mortality: true,
        reproduction: true,
        locomotion: true,
    };
    let horizon = 960;
    let orig_hashes = uninterrupted_app_hashes(&r3, horizon);

    for &k in &[1, 899, 900] {
        let div = measure_app_restore(&r3, k, horizon, &orig_hashes);
        assert_eq!(
            div, None,
            "R3 checkpoint K={k} must match uninterrupted run through tick {horizon}"
        );
    }
}

fn sibling_agent_fixture(
    lineage_id: &str,
    root_x: f32,
    ticket_id: u64,
    pending_state: [f32; STATE_DIM],
) -> SerializedAgent {
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.25,
        mass: 1.0,
    });
    SerializedAgent {
        genotype,
        class: AgentClass::Prey,
        lineage_id: lineage_id.to_owned(),
        generation: 1,
        parent_ids: vec!["founder-0000000000000029-00000000".to_owned()],
        evaluation: AgentEvaluation {
            start_position: glam::Vec3::new(root_x, 0.0, 0.0),
            total_distance: 0.0,
            total_energy_expended: 0.0,
            total_energy_gained: 0.0,
            prey_energy_gained: 0.0,
            survival_ticks: 10,
            last_position: glam::Vec3::new(root_x, 0.0, 0.0),
        },
        feature_tracker: FeatureTracker::default(),
        root_position: glam::Vec3::new(root_x, 0.0, 0.0),
        root_rotation: glam::Quat::IDENTITY,
        root_velocity: glam::Vec3::ZERO,
        homeostatic_state: anima_engine_lib::ai::hrrl::HomeostaticState {
            energy: 60.0,
            energy_target: 100.0,
            hydration: 60.0,
            hydration_target: 100.0,
            temperature: 37.0,
            temp_target: 37.0,
            previous_deviation: 0.0,
            starving_ticks: 0,
            dehydrating_ticks: Some(0),
        },
        last_transition_state: anima_engine_lib::ai::hrrl::LastTransitionState {
            state: [0.0; STATE_DIM],
            action: [0.0; ACTION_DIM],
            has_last: false,
            pending_state: Some(pending_state),
        },
        cognitive_state: CognitiveState::PendingInference(ticket_id),
        inertia: anima_engine_lib::core::components::InertiaComponent {
            cpg_parameters: [0.1, 0.2, 0.3, 0.4],
            ..Default::default()
        },
        action_gates: Some(ActionGates {
            pheromone_emit: 0.25,
            attack_intent: 0.5,
            feed_intent: 0.75,
        }),
        segments: Vec::new(),
        brain: None,
        sex: None,
    }
}

fn valid_shared_learning_fixture(
    seed: u64,
    pending_inference: Vec<SavedInferenceResponseBatch>,
) -> SavedSharedLearningState {
    let device = Device::flex().autodiff();
    let weights = BrainModel::new_seeded_cpu(STATE_DIM, HIDDEN_DIM, ACTION_DIM, seed)
        .checkpoint_weights()
        .expect("extract seeded weights");
    let model =
        ActorCriticModel::from_flat_weights(STATE_DIM, HIDDEN_DIM, ACTION_DIM, &weights, &device)
            .expect("build fixture training model");
    let optimizer = AdamConfig::new().init();
    let training_model_record = model
        .into_record()
        .into_bytes()
        .expect("encode model record")
        .to_vec();
    let optimizer_record = optimizer.into_bytes().expect("encode Adam record").to_vec();
    let transition = Transition {
        state: [0.25; STATE_DIM],
        action: [0.5; ACTION_DIM],
        reward: 0.75,
        next_state: [1.0; STATE_DIM],
    };
    let pending_weights = BrainModel::new_seeded_cpu(STATE_DIM, HIDDEN_DIM, ACTION_DIM, seed + 1)
        .checkpoint_weights()
        .expect("extract pending seeded weights");
    SavedSharedLearningState {
        learner: SavedLearnerWorkerState {
            training_model_record,
            optimizer_record,
            partial_batch: vec![transition],
        },
        inference_weights: weights,
        pending_inference_weights: Some(pending_weights),
        queued_transitions: vec![transition],
        pending_inference,
        learning_queue_diagnostics: LearningQueueSnapshot::default(),
        model_update_diagnostics: ModelUpdateSnapshot::default(),
    }
}

#[test]
fn f048_two_pending_siblings_roundtrip_disk_snapshot_and_route_by_ticket() {
    let shared_lineage = "founder-0000000000000029-00000000.1";
    let actions_a: [f32; ACTION_SLOTS] = [0.1, 0.2, 0.3, 0.4, 0.25, 0.5, 0.75, 0.0];
    let actions_b: [f32; ACTION_SLOTS] = [0.8, 0.7, 0.6, 0.5, 0.75, 0.25, 0.5, 1.0];

    let mut state = empty_saved_state_for_tests();
    state.tick_count = 42;
    state.sim_rng_seed = 29;
    state.agents = vec![
        sibling_agent_fixture(shared_lineage, -10.0, 101, [0.25; STATE_DIM]),
        sibling_agent_fixture(shared_lineage, 10.0, 102, [0.5; STATE_DIM]),
    ];
    state.shared_learning = Some(valid_shared_learning_fixture(
        state.sim_rng_seed,
        vec![SavedInferenceResponseBatch {
            responses: vec![
                SavedInferenceResponse {
                    lineage_id: shared_lineage.to_owned(),
                    actions: actions_a,
                    request_id: 101,
                },
                SavedInferenceResponse {
                    lineage_id: shared_lineage.to_owned(),
                    actions: actions_b,
                    request_id: 102,
                },
            ],
        }],
    ));

    let seq = DISK_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "anima_f048_siblings_{}_{}",
        std::process::id(),
        seq
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("siblings_pending.json");
    let envelope = SnapshotEnvelope::seal(state)
        .expect("two same-lineage siblings in PendingInference must seal without collision");
    snapshot::write_atomic(&path, &envelope).expect("write sibling snapshot");
    let read_back = snapshot::read(&path)
        .expect("real snapshot::read must load two same-lineage PendingInference siblings");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(read_back.loaded_from_schema, SCHEMA_VERSION);

    let mut world = init_world();
    let entity_a =
        spawn_serialized_agent(&mut world, &read_back.agents[0]).expect("spawn sibling A");
    let entity_b =
        spawn_serialized_agent(&mut world, &read_back.agents[1]).expect("spawn sibling B");
    assert_ne!(entity_a, entity_b);

    let pending_batches = &read_back
        .shared_learning
        .as_ref()
        .expect("shared_learning preserved across disk round-trip")
        .pending_inference;
    let (routed_batches, dropped) = route_restored_pending_inference(&mut world, pending_batches);
    assert_eq!(dropped, 0, "both siblings' responses must match cleanly");
    assert_eq!(routed_batches.len(), 1);
    assert_eq!(routed_batches[0].len(), 2);
    assert_eq!(routed_batches[0][0].entity, entity_a);
    assert_eq!(routed_batches[0][0].request_id, 101);
    assert_eq!(routed_batches[0][0].actions, actions_a);
    assert_eq!(routed_batches[0][1].entity, entity_b);
    assert_eq!(routed_batches[0][1].request_id, 102);
    assert_eq!(routed_batches[0][1].actions, actions_b);

    // An unmatched `(lineage_id, request_id)` must be dropped and counted, never routed to a sibling.
    world.entity_mut(entity_b).insert(CognitiveState::Ready);
    let adversarial_batches = vec![SavedInferenceResponseBatch {
        responses: vec![
            SavedInferenceResponse {
                lineage_id: shared_lineage.to_owned(),
                actions: actions_a,
                request_id: 101,
            },
            SavedInferenceResponse {
                lineage_id: shared_lineage.to_owned(),
                actions: actions_b,
                request_id: 999,
            },
            SavedInferenceResponse {
                lineage_id: "absent-lineage".to_owned(),
                actions: actions_b,
                request_id: 101,
            },
        ],
    }];
    let (filtered_batches, dropped_unmatched) =
        route_restored_pending_inference(&mut world, &adversarial_batches);
    assert_eq!(
        dropped_unmatched, 2,
        "unmatched (lineage_id, request_id) pairs must be dropped and counted, never routed to sibling B"
    );
    assert_eq!(filtered_batches.len(), 1);
    assert_eq!(filtered_batches[0].len(), 1);
    assert_eq!(filtered_batches[0][0].entity, entity_a);
    assert_eq!(filtered_batches[0][0].request_id, 101);
}
