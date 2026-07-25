use bevy_ecs::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use burn::backend::Autodiff;
use burn::module::AutodiffModule;
use burn::optim::{AdamConfig, GradientsParams, Optimizer};
use burn::tensor::backend::Backend;
use burn::tensor::{Data, Shape, Tensor};

use crate::ai::cpg::update_cpg_system;
use crate::ai::hrrl::{Transition, TransitionSender};
use crate::ai::model::{hrrl_learning_system, ActorCriticModel, BrainInferenceBuffer, BrainModel};
use crate::core::agent_systems::*;
use crate::core::ecs::*;
#[allow(unused_imports)]
use crate::core::networking_systems::*;
use crate::core::resources::EvolutionQueue;
use crate::core::simulation_state::*;
use crate::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};
use crate::evolution::lineage::LineageTracker;
use crate::physics::{
    integrate_physics_system, rebuild_spatial_grid_system, resolve_joints_system, JointConstraint,
};
use tauri::Emitter;

pub enum ModelUpdate {
    NdArray(ActorCriticModel<burn_ndarray::NdArray<f32>>),
    #[cfg(feature = "ml-wgpu")]
    Wgpu(ActorCriticModel<burn_wgpu::Wgpu<burn_wgpu::AutoGraphicsApi, f32, i32>>),
}

/// The four handles a learner thread needs: the running flag, the transition receiver, the model
/// sender and the old-model receiver.
type LearnArgs = (
    Arc<AtomicBool>,
    crossbeam_channel::Receiver<Transition>,
    crossbeam_channel::Sender<ModelUpdate>,
    crossbeam_channel::Receiver<ModelUpdate>,
);

/// CPU learner. Always available — it is the fallback both when the GPU probe fails and when the
/// `ml-wgpu` feature is off entirely.
fn spawn_ndarray_learner(args: LearnArgs) -> thread::JoinHandle<()> {
    let (running, trans_rx, model_tx, old_model_rx) = args;
    thread::spawn(move || {
        let device = burn_ndarray::NdArrayDevice::Cpu;
        run_training_loop::<burn_ndarray::NdArray<f32>>(
            running,
            trans_rx,
            model_tx,
            old_model_rx,
            device,
            ModelUpdate::NdArray,
        );
    })
}

/// GPU learner. Only exists with the `ml-wgpu` feature; the whole wgpu/naga/ash stack goes with it.
#[cfg(feature = "ml-wgpu")]
fn spawn_wgpu_learner(args: LearnArgs) -> thread::JoinHandle<()> {
    let (running, trans_rx, model_tx, old_model_rx) = args;
    thread::spawn(move || {
        let device = burn_wgpu::WgpuDevice::default();
        run_training_loop::<burn_wgpu::Wgpu<burn_wgpu::AutoGraphicsApi, f32, i32>>(
            running,
            trans_rx,
            model_tx,
            old_model_rx,
            device,
            ModelUpdate::Wgpu,
        );
    })
}

pub struct SimulationEngine {
    pub running: Arc<AtomicBool>,
    pub status: Arc<Mutex<SimulationStatus>>,
    pub agent_states: Arc<RwLock<Vec<SegmentState>>>,
    pub pheromone_grid_state: Arc<RwLock<crate::ai::pheromone::PheromoneGridState>>,
    pub active_raycasts: Arc<RwLock<Vec<crate::core::ecs::RaycastTelemetry>>>,
    pub combat_events: Arc<RwLock<Vec<crate::core::ecs::CombatEvent>>>,
    pub threads: Mutex<Option<Vec<thread::JoinHandle<()>>>>,
    pub lineage_tracker: Arc<crate::evolution::lineage::FallbackLineageTracker>,
    pub chronicle_history: Arc<RwLock<Vec<ChronicleEvent>>>,
    pub sharding_config: Arc<RwLock<crate::core::ecs::ShardingConfig>>,
    pub manual_migration_trigger: crossbeam_channel::Sender<u16>,
    pub manual_migration_receiver: crossbeam_channel::Receiver<u16>,

    pub save_request_tx: crossbeam_channel::Sender<std::sync::mpsc::Sender<SavedSimulationState>>,
    pub save_request_rx: crossbeam_channel::Receiver<std::sync::mpsc::Sender<SavedSimulationState>>,
    pub pending_load_state: Arc<Mutex<Option<SavedSimulationState>>>,
    pub environmental_state: Arc<RwLock<crate::core::ecs::EnvironmentalState>>,
    pub terrain_map: Arc<RwLock<Option<crate::commands::environment::TerrainMapState>>>,
    pub ecosystem_state: Arc<RwLock<crate::commands::environment::EcosystemState>>,
}

impl Default for SimulationEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SimulationEngine {
    pub fn new() -> Self {
        let uri =
            std::env::var("NEO4J_URI").unwrap_or_else(|_| "bolt://localhost:7687".to_string());
        let user = std::env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".to_string());
        let pass = std::env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "password".to_string());
        let lineage_tracker = Arc::new(crate::evolution::lineage::FallbackLineageTracker::new(
            &uri, &user, &pass,
        ));
        let sharding_config = Arc::new(RwLock::new(crate::core::ecs::ShardingConfig::default()));
        let (manual_migration_trigger, manual_migration_receiver) =
            crossbeam_channel::unbounded::<u16>();
        let (save_request_tx, save_request_rx) = crossbeam_channel::unbounded();
        let pending_load_state = Arc::new(Mutex::new(None));

        Self {
            running: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new(SimulationStatus {
                running: false,
                tick_count: 0,
                avg_tick_time_ms: 0.0,
                fps: 0.0,
            })),
            agent_states: Arc::new(RwLock::new(Vec::with_capacity(1000))),
            pheromone_grid_state: Arc::new(RwLock::new(crate::ai::pheromone::PheromoneGridState {
                grid: vec![0.0; 128 * 128],
                width: 128,
                height: 128,
            })),
            active_raycasts: Arc::new(RwLock::new(Vec::with_capacity(1000))),
            combat_events: Arc::new(RwLock::new(Vec::with_capacity(100))),
            threads: Mutex::new(None),
            lineage_tracker,
            chronicle_history: Arc::new(RwLock::new(Vec::new())),
            sharding_config,
            manual_migration_trigger,
            manual_migration_receiver,
            save_request_tx,
            save_request_rx,
            pending_load_state,
            environmental_state: Arc::new(RwLock::new(
                crate::core::ecs::EnvironmentalState::default(),
            )),
            terrain_map: Arc::new(RwLock::new(None)),
            ecosystem_state: Arc::new(RwLock::new(
                crate::commands::environment::EcosystemState::default(),
            )),
        }
    }

    pub fn start<R: tauri::Runtime>(
        &self,
        app_handle: Option<tauri::AppHandle<R>>,
        evolution_settings: Arc<std::sync::Mutex<crate::commands::EvolutionSettings>>,
        evolution_running: Arc<std::sync::atomic::AtomicBool>,
        map_elites_grid: Arc<std::sync::Mutex<crate::commands::MapElitesGridState>>,
    ) {
        while self.manual_migration_receiver.try_recv().is_ok() {}

        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let running_clone = Arc::clone(&self.running);
        let status_clone = Arc::clone(&self.status);
        let agent_states_clone = Arc::clone(&self.agent_states);
        let pheromone_grid_state_clone = Arc::clone(&self.pheromone_grid_state);
        let active_raycasts_clone = Arc::clone(&self.active_raycasts);
        let combat_events_clone = Arc::clone(&self.combat_events);
        let environmental_state_clone = Arc::clone(&self.environmental_state);
        let ecosystem_state_clone = Arc::clone(&self.ecosystem_state);

        let (trans_tx, trans_rx) = crossbeam_channel::bounded::<Transition>(4096);
        let (model_tx, model_rx) = crossbeam_channel::bounded::<ModelUpdate>(32);
        let (old_model_tx, old_model_rx) = crossbeam_channel::bounded::<ModelUpdate>(32);

        let use_gpu = std::env::var("ANIMA_USE_GPU")
            .map(|val| val != "false" && val != "0")
            .unwrap_or(true);

        #[cfg_attr(not(feature = "ml-wgpu"), allow(unused_mut))]
        let mut has_wgpu = false;
        // The GPU backend is behind the `ml-wgpu` feature (G2). Without it there is no device to
        // probe for, so the learner always takes the ndarray path — which is the same path a
        // machine with no usable GPU already took.
        #[cfg(feature = "ml-wgpu")]
        if use_gpu {
            let probe = std::panic::catch_unwind(|| {
                let _ = burn_wgpu::WgpuDevice::default();
            });
            if probe.is_ok() {
                has_wgpu = true;
            }
        }
        #[cfg(not(feature = "ml-wgpu"))]
        let _ = use_gpu;

        // Both learners take the same four handles. Extracted so the feature split does not have to
        // duplicate the ndarray body into a second cfg branch.
        let learn_args = (
            Arc::clone(&self.running),
            trans_rx.clone(),
            model_tx.clone(),
            old_model_rx.clone(),
        );

        #[cfg(feature = "ml-wgpu")]
        let learn_handle = if has_wgpu {
            spawn_wgpu_learner(learn_args)
        } else {
            spawn_ndarray_learner(learn_args)
        };
        #[cfg(not(feature = "ml-wgpu"))]
        let learn_handle = {
            let _ = has_wgpu;
            spawn_ndarray_learner(learn_args)
        };

        let (stats_tx, stats_rx) = crossbeam_channel::bounded::<Vec<AgentEpochStats>>(128);
        let (spawn_tx, spawn_rx) = crossbeam_channel::bounded::<(
            Entity,
            MorphologyGenotype,
            glam::Vec3,
            String,
            u32,
            Vec<String>,
        )>(128);
        let (env_tx, env_rx) =
            crossbeam_channel::bounded::<crate::evolution::meta_ai::EnvironmentalEvent>(32);

        let running_clone_evo = Arc::clone(&self.running);
        let evolution_running_clone = Arc::clone(&evolution_running);
        let evolution_settings_clone = Arc::clone(&evolution_settings);
        let map_elites_grid_clone = Arc::clone(&map_elites_grid);
        let app_handle_evo = app_handle.clone();
        let lineage_tracker_evo = Arc::clone(&self.lineage_tracker);
        let chronicle_history_clone = Arc::clone(&self.chronicle_history);

        let evo_handle = thread::spawn(move || {
            let initial_resolution = {
                let settings = evolution_settings_clone.lock().unwrap();
                settings.grid_resolution
            };
            let mut archive = crate::evolution::map_elites::MapElitesArchive::new(
                1.0 / (initial_resolution as f32),
            );
            let mut node_id_counter = 3u32;
            // Selection, recombination and mutation all draw from this one stream, so the same run
            // seed reproduces the same offspring — see `resources::sim_stream`.
            //
            // This thread is spawned *before* the ECS world exists, so it cannot read `SimRng` off
            // the world; it resolves the same seed from the same source the world will use. That the
            // two agree is pinned by `sim_determinism_tests::evolution_thread_and_world_agree_on_seed`.
            let mut evo_rng = crate::core::resources::derived_rng(
                crate::core::resources::resolve_run_seed(
                    crate::core::world_artifact::world_seed_from_disk(),
                ),
                crate::core::resources::sim_stream::EVOLUTION,
            );
            // G1.3. This thread mints lineage and chronicle ids and stamps chronicle entries, all of
            // which end up in saved state — so in a deterministic run they must come from the run,
            // not from OS entropy and the wall clock. It resolves the mode and the run id from the
            // same sources the world will use, for the same reason `evo_rng` above does: the thread
            // is spawned before the ECS world exists.
            let evo_deterministic = crate::core::determinism::DeterministicMode::from_env();
            let evo_run_id = crate::core::resources::resolve_run_seed(
                crate::core::world_artifact::world_seed_from_disk(),
            );
            // Separate namespaces so two id sources on one thread cannot collide.
            let chronicle_ids = crate::core::determinism::RunIdentity::new(evo_run_id, "chronicle");
            let offspring_ids = crate::core::determinism::RunIdentity::new(evo_run_id, "lineage");
            let meta_ai_client: Box<dyn crate::evolution::meta_ai::MetaAiClient> =
                match std::env::var("GEMINI_SESSION_TOKEN") {
                    Ok(token) if !token.trim().is_empty() => Box::new(
                        crate::evolution::meta_ai::GeminiWebSessionClient::new(&token),
                    ),
                    _ => Box::new(crate::evolution::meta_ai::GeminiMetaAiClient::new(
                        Duration::from_secs(5),
                    )),
                };
            let mut meta_ai_history = Vec::new();
            let mut meta_ai_epoch = 0u32;

            while running_clone_evo.load(Ordering::SeqCst) {
                if let Ok(stats_batch) = stats_rx.recv_timeout(Duration::from_millis(10)) {
                    if !evolution_running_clone.load(Ordering::SeqCst) {
                        continue;
                    }

                    meta_ai_epoch += 1;
                    let new_event = meta_ai_client.generate_event(meta_ai_epoch, &meta_ai_history);
                    meta_ai_history.push(new_event);
                    let _ = env_tx.send(new_event);

                    let id =
                        crate::core::determinism::next_entity_id(evo_deterministic, &chronicle_ids);
                    let (event_type, title, description) = match new_event {
                        crate::evolution::meta_ai::EnvironmentalEvent::ResourceDrought => (
                            "Drought".to_string(),
                            "Resource Drought".to_string(),
                            format!("Epoch {}: An extreme drought limits food spawning and reduces available nutrients.", meta_ai_epoch)
                        ),
                        crate::evolution::meta_ai::EnvironmentalEvent::TemperatureSpike => (
                            "TemperatureSpike".to_string(),
                            "Temperature Spike".to_string(),
                            format!("Epoch {}: An intense heatwave sets in, shifting homeostasis targets up.", meta_ai_epoch)
                        ),
                        crate::evolution::meta_ai::EnvironmentalEvent::GlacialPeriod => (
                            "TemperatureSpike".to_string(),
                            "Glacial Period".to_string(),
                            format!("Epoch {}: Deep freeze spreads across the sector, lowering target temperatures.", meta_ai_epoch)
                        ),
                        crate::evolution::meta_ai::EnvironmentalEvent::ToxicDeluge => (
                            "Drought".to_string(),
                            "Toxic Deluge".to_string(),
                            format!("Epoch {}: Acidic rainfall degrades local resources and increases metabolic stress.", meta_ai_epoch)
                        ),
                        crate::evolution::meta_ai::EnvironmentalEvent::Stable => (
                            "Abundance".to_string(),
                            "Stable Climate".to_string(),
                            format!("Epoch {}: Conditions return to equilibrium. The climate is stable.", meta_ai_epoch)
                        ),
                    };

                    // Derived from the epoch counter in a deterministic run, so replaying a manifest
                    // reproduces the same chronicle rather than one stamped with when it happened
                    // to be replayed.
                    let timestamp = crate::core::determinism::timestamp_ms(
                        evo_deterministic,
                        meta_ai_epoch as u64 * crate::core::sim_rules::TICKS_PER_EPOCH,
                    );

                    let mut parameter_delta = std::collections::HashMap::new();
                    match new_event {
                        crate::evolution::meta_ai::EnvironmentalEvent::ResourceDrought => {
                            parameter_delta.insert("food_multiplier".to_string(), 0.5);
                        }
                        crate::evolution::meta_ai::EnvironmentalEvent::TemperatureSpike => {
                            parameter_delta.insert("temp_target".to_string(), 5.0);
                        }
                        crate::evolution::meta_ai::EnvironmentalEvent::GlacialPeriod => {
                            parameter_delta.insert("temp_target".to_string(), -5.0);
                        }
                        crate::evolution::meta_ai::EnvironmentalEvent::ToxicDeluge => {
                            parameter_delta.insert("food_multiplier".to_string(), 0.8);
                        }
                        _ => {}
                    }

                    let chronicle_event = ChronicleEvent {
                        id,
                        event_type,
                        timestamp,
                        title,
                        description,
                        parameter_delta,
                    };

                    if let Ok(mut history) = chronicle_history_clone.write() {
                        history.push(chronicle_event.clone());
                    }

                    if let Some(ref handle) = app_handle_evo {
                        let _ = handle.emit("chronicle-event", &chronicle_event);
                    }

                    let mut grid_updated = false;
                    let (selection_bias, mutation_rate, grid_res) = {
                        let settings = evolution_settings_clone.lock().unwrap();
                        (
                            settings.selection_bias,
                            settings.mutation_rate,
                            settings.grid_resolution,
                        )
                    };

                    let target_res = 1.0 / (grid_res as f32);
                    if (archive.grid_resolution - target_res).abs() > 1e-5 {
                        archive.grid_resolution = target_res;
                        archive.grid.clear();
                    }

                    for stats in &stats_batch {
                        // MAP-Elites niche axes = ecological descriptors (body mass × foraging
                        // range), normalized so the QD archive illuminates ecological diversity.
                        let features = crate::core::ecology::ecological_descriptors(
                            stats.body_mass,
                            stats.foraging_range,
                        )
                        .to_vec();
                        let elite = crate::evolution::map_elites::EliteIndividual {
                            genotype: stats.genotype.clone(),
                            fitness: stats.fitness,
                            features,
                            lineage_id: stats.lineage_id.clone(),
                            generation: stats.generation,
                        };
                        if archive.add_individual(elite) {
                            grid_updated = true;
                        }
                    }

                    if grid_updated {
                        if let Ok(mut grid_state) = map_elites_grid_clone.lock() {
                            grid_state.grid.clear();
                            for (coords, elite) in archive.grid.iter() {
                                let key = format!("{},{}", coords.0, coords.1);
                                grid_state.grid.insert(
                                    key,
                                    crate::commands::EliteIndividualState {
                                        fitness: elite.fitness as f64,
                                        features: elite
                                            .features
                                            .iter()
                                            .map(|&f| f as f64)
                                            .collect(),
                                    },
                                );
                            }
                        }

                        let grid_to_emit = {
                            let grid_state = map_elites_grid_clone.lock().unwrap();
                            grid_state.clone()
                        };
                        if let Some(ref handle) = app_handle_evo {
                            let _ = handle.emit("map-elites-update", grid_to_emit);
                        }
                    }

                    for stats in stats_batch {
                        let parent_a = archive.select_parent(selection_bias, &mut evo_rng);
                        let parent_b = archive.select_parent(selection_bias, &mut evo_rng);

                        let (mut offspring, parent_ids, max_parent_gen, relation_type) =
                            if let Some(elite_a) = parent_a {
                                if let Some(elite_b) = parent_b {
                                    let child = crate::evolution::crossover::crossover_genotypes(
                                        &elite_a.genotype,
                                        &elite_b.genotype,
                                        &mut node_id_counter,
                                        &mut evo_rng,
                                    );
                                    (
                                        child,
                                        vec![
                                            elite_a.lineage_id.clone(),
                                            elite_b.lineage_id.clone(),
                                        ],
                                        elite_a.generation.max(elite_b.generation),
                                        crate::evolution::lineage::RelationType::Crossover,
                                    )
                                } else {
                                    (
                                        elite_a.genotype.clone(),
                                        vec![elite_a.lineage_id.clone()],
                                        elite_a.generation,
                                        crate::evolution::lineage::RelationType::Clone,
                                    )
                                }
                            } else {
                                (
                                    stats.genotype.clone(),
                                    vec![stats.lineage_id.clone()],
                                    stats.generation,
                                    crate::evolution::lineage::RelationType::Clone,
                                )
                            };

                        let mut final_rel_type = relation_type;
                        if mutation_rate > 0.0 {
                            if parent_ids.len() == 1 {
                                final_rel_type = crate::evolution::lineage::RelationType::Mutate;
                            }
                            crate::evolution::mutation::mutate_genotype(
                                &mut offspring,
                                &mut node_id_counter,
                                mutation_rate,
                                &mut evo_rng,
                            );
                        }

                        let offspring_generation = max_parent_gen + 1;
                        let offspring_id = crate::core::determinism::next_entity_id(
                            evo_deterministic,
                            &offspring_ids,
                        );

                        let _ = lineage_tracker_evo.add_reproduction(
                            offspring_id.clone(),
                            offspring_generation,
                            offspring.clone(),
                            parent_ids.clone(),
                            final_rel_type,
                        );

                        let _ = spawn_tx.send((
                            stats.entity,
                            offspring,
                            stats.position,
                            offspring_id,
                            offspring_generation,
                            parent_ids,
                        ));
                    }
                }
            }
        });

        let app_handle_clone = app_handle.clone();
        let app_handle_emit = app_handle.clone();
        let app_handle_net = app_handle.clone();
        let lineage_tracker_sim = Arc::clone(&self.lineage_tracker);
        let sharding_config_sim = Arc::clone(&self.sharding_config);
        let manual_migration_receiver_clone = self.manual_migration_receiver.clone();

        let pending_load_state_clone = Arc::clone(&self.pending_load_state);
        let save_request_rx_clone = self.save_request_rx.clone();
        let chronicle_history_clone_save = Arc::clone(&self.chronicle_history);
        let lineage_tracker_sim_save = Arc::clone(&self.lineage_tracker);
        let evolution_settings_clone_save = Arc::clone(&evolution_settings);
        let map_elites_grid_clone_save = Arc::clone(&map_elites_grid);
        let terrain_map_clone = Arc::clone(&self.terrain_map);

        let (inbound_tx, inbound_rx) =
            crossbeam_channel::unbounded::<crate::core::ecs::AgentMigrationData>();
        let (outbound_tx, outbound_rx) =
            crossbeam_channel::unbounded::<crate::core::ecs::OutboundMigration>();

        let sim_handle = thread::spawn(move || {
            let state_to_load = pending_load_state_clone.lock().unwrap().take();

            let mut world = init_world();

            // S08: warn loudly if the save belongs to a DIFFERENT world than the one just built, so
            // saved agents/positions aren't silently dropped into a mismatched world. Compared
            // against the WorldIdentity resource init_world inserts (the terrain-domain fingerprint),
            // never the artifact header checksum. A default (all-zero) saved identity is a legacy
            // save and is skipped.
            if let Some(state) = state_to_load.as_ref() {
                let current = world
                    .get_resource::<crate::core::world_artifact::WorldIdentity>()
                    .copied()
                    .unwrap_or_default();
                if let Some(reason) = state.world_identity.mismatch_against(&current) {
                    eprintln!(
                        "WARNING: loading a save from a different world — {reason}; saved agents may be placed in a mismatched world"
                    );
                }
            }

            let loaded_bounds = state_to_load
                .as_ref()
                .map(|s| s.map_bounds)
                .unwrap_or_default();
            world.insert_resource(loaded_bounds);

            if let Some(terrain_map) = world.get_resource::<crate::core::terrain::TerrainMap>() {
                let state = crate::commands::environment::TerrainMapState::from_resource(
                    terrain_map,
                    &loaded_bounds,
                );
                if let Ok(mut lock) = terrain_map_clone.write() {
                    *lock = Some(state);
                }
            }

            world.insert_resource(crate::physics::SpatialHashGrid::new_prepopulated(
                10.0,
                &loaded_bounds,
            ));

            let loaded_pheromone = state_to_load
                .as_ref()
                .map(|s| crate::ai::pheromone::PheromoneGrid {
                    values: s.pheromone_grid.values.clone(),
                    scratch: vec![0.0; crate::ai::pheromone::CELL_COUNT],
                    diffusion_rate: s.pheromone_grid.diffusion_rate,
                    decay_rate: s.pheromone_grid.decay_rate,
                })
                .unwrap_or_default();
            world.insert_resource(loaded_pheromone);

            // Seeded from the run, so the shared brain a legacy agent uses is the same network every
            // time the same world is launched. `SimRng` is already in the world by now (`init_world`).
            let run_seed = world.resource::<crate::core::resources::SimRng>().seed();
            world.insert_resource(BrainModel::new_seeded(15, 64, 4, run_seed));
            world.insert_resource(BrainInferenceBuffer::default());

            let (req_tx, req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
            let (recycle_req_tx, recycle_req_rx) =
                crossbeam_channel::unbounded::<InferenceRequestBatch>();
            let (res_tx, res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();
            let (recycle_res_tx, recycle_res_rx) =
                crossbeam_channel::unbounded::<InferenceResponseBatch>();

            // Pre-populate recycle pools to ensure zero heap allocations in the hot path
            for _ in 0..16 {
                let req_batch = InferenceRequestBatch {
                    requests: Vec::with_capacity(128),
                };
                let res_batch = InferenceResponseBatch {
                    responses: Vec::with_capacity(128),
                };
                let _ = recycle_req_tx.send(req_batch);
                let _ = recycle_res_tx.send(res_batch);
            }

            let channels = InferenceChannels {
                req_tx,
                recycle_req_rx,
                res_rx,
                recycle_res_tx,
            };
            world.insert_resource(channels);

            let running_inference = Arc::clone(&running_clone);
            let model_rx_inference = model_rx;
            let old_model_tx_inference = old_model_tx;

            let inference_seed = run_seed;
            thread::spawn(move || {
                // The same seed the world's copy used. These are two separately-constructed models
                // that are meant to be the same network until the training thread starts sending
                // updates; with unseeded initialisation they never were.
                let mut brain_model = BrainModel::new_seeded(15, 64, 4, inference_seed);
                // Allocated once and reused every batch: the worker runs on the tick path's critical
                // chain, so per-batch allocation here would show up as frame jitter.
                let mut inference_scratch = crate::ai::model::InferenceScratch::with_capacity(256);

                while running_inference.load(Ordering::SeqCst) {
                    // Check for model update
                    if let Ok(new_model) = model_rx_inference.try_recv() {
                        match (new_model, &mut brain_model.backend) {
                            (
                                ModelUpdate::NdArray(new_m),
                                crate::ai::model::BrainModelBackend::NdArray(ref mut old_m, _),
                            ) => {
                                let old = std::mem::replace(old_m, new_m);
                                let _ = old_model_tx_inference.send(ModelUpdate::NdArray(old));
                            }
                            #[cfg(feature = "ml-wgpu")]
                            (
                                ModelUpdate::Wgpu(new_m),
                                crate::ai::model::BrainModelBackend::Wgpu(ref mut old_m, _),
                            ) => {
                                let old = std::mem::replace(old_m, new_m);
                                let _ = old_model_tx_inference.send(ModelUpdate::Wgpu(old));
                            }
                            #[cfg_attr(not(feature = "ml-wgpu"), allow(unreachable_patterns))]
                            _ => {}
                        }
                    }

                    // Receive request batch
                    if let Ok(req_batch) = req_rx.recv_timeout(Duration::from_millis(2)) {
                        if !req_batch.requests.is_empty() {
                            let mut res_batch = recycle_res_rx.try_recv().unwrap_or_else(|_| {
                                InferenceResponseBatch {
                                    responses: Vec::with_capacity(128),
                                }
                            });
                            res_batch.responses.clear();

                            // Same function the tests drive synchronously — see
                            // `ai::model::run_inference_batch`. Keeping the worker a thin shell
                            // around it means the logic that decides every agent's action each tick
                            // is reachable by a test instead of sealed inside a spawned closure.
                            crate::ai::model::run_inference_batch(
                                &brain_model,
                                &req_batch.requests,
                                &mut res_batch.responses,
                                &mut inference_scratch,
                            );

                            let _ = res_tx.send(res_batch);
                        }

                        let _ = recycle_req_tx.send(req_batch);
                    }
                }
            });

            let loaded_food_settings = state_to_load
                .as_ref()
                .map(|s| s.food_spawn_settings)
                .unwrap_or_default();
            world.insert_resource(loaded_food_settings);
            world.insert_resource(EnvironmentalSpawnSettings::default());

            world.insert_resource(TransitionSender(trans_tx));

            world.insert_resource(BevyEvolutionSettings(evolution_settings));
            world.insert_resource(BevyEvolutionRunning(evolution_running));
            world.insert_resource(BevyMapElitesGrid(map_elites_grid));
            world.insert_resource(BevyAppHandle(app_handle_clone));

            let initial_resolution = {
                let settings_lock = evolution_settings_clone_save.lock().unwrap();
                settings_lock.grid_resolution
            };
            world.insert_resource(ActiveEvolutionSettings {
                mutation_rate: 0.15,
                selection_bias: 1.5,
                grid_resolution: initial_resolution,
            });
            world.insert_resource(BevyMapElitesArchive {
                archive: crate::evolution::map_elites::MapElitesArchive::new(
                    1.0 / (initial_resolution as f32),
                ),
            });

            let mut next_node_id = 3;
            if let Some(ref state) = state_to_load {
                for agent in &state.agents {
                    for node in &agent.genotype.nodes {
                        if node.id >= next_node_id {
                            next_node_id = node.id + 1;
                        }
                    }
                }
            }
            world.insert_resource(NextNodeId(next_node_id));

            world.insert_resource(EvolutionSender(stats_tx));
            world.insert_resource(EvolutionReceiver(spawn_rx));
            world.insert_resource(EnvironmentalEventReceiver(env_rx));

            let loaded_epoch =
                state_to_load
                    .as_ref()
                    .map(|s| s.epoch_manager)
                    .unwrap_or(EpochManager {
                        ticks_per_epoch: 1000,
                        current_epoch_ticks: 0,
                        current_epoch: 0,
                    });
            world.insert_resource(loaded_epoch);
            world.insert_resource(EvolutionQueue::default());

            world.insert_resource(crate::core::ecs::InboundMigrationReceiver(inbound_rx));
            world.insert_resource(crate::core::ecs::OutboundMigrationSender(outbound_tx));
            world.insert_resource(crate::core::ecs::ShardingResource(sharding_config_sim));
            world.insert_resource(crate::core::ecs::BevyMigrationTrigger(
                manual_migration_receiver_clone,
            ));

            let loaded_env = state_to_load
                .as_ref()
                .map(|s| ActiveEnvironmentEvent(s.active_environment_event))
                .unwrap_or_default();
            world.insert_resource(loaded_env);

            if let Some(ref state) = state_to_load {
                lineage_tracker_sim
                    .load_state(state.lineage_nodes.clone(), state.lineage_relations.clone());
                if let Ok(mut history) = chronicle_history_clone_save.write() {
                    *history = state.chronicle_history.clone();
                }

                for agent in &state.agents {
                    spawn_serialized_agent(&mut world, agent);
                }
                // G1.1: the closed-energy compartments and the standing crop come back with the
                // agents. Without this a load rebuilt detritus at zero and plants at full capacity,
                // so every save/load boundary moved EU and whole-run conservation was unprovable.
                crate::core::simulation_state::restore_energy_state(&mut world, state);
                for food in &state.foods {
                    use crate::core::ecs::Food;
                    world.spawn((
                        Food {
                            energy_value: food.energy_value,
                            hydration_value: food.hydration_value,
                        },
                        Position(food.position),
                        crate::physics::SpatialCollider { radius: 0.5 },
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
                        crate::physics::SpatialCollider {
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
                        crate::physics::SpatialCollider {
                            radius: tree.radius,
                        },
                    ));
                }
            } else {
                let mut genotype = MorphologyGenotype::new();
                genotype.add_node(MorphologyNode {
                    id: 0,
                    length: 1.0,
                    radius: 0.2,
                    mass: 1.5,
                });
                genotype.add_node(MorphologyNode {
                    id: 1,
                    length: 1.0,
                    radius: 0.2,
                    mass: 1.0,
                });
                genotype.add_node(MorphologyNode {
                    id: 2,
                    length: 1.0,
                    radius: 0.2,
                    mass: 0.8,
                });

                genotype.add_edge(MorphologyEdge {
                    source_node: 0,
                    target_node: 1,
                    joint_anchor: glam::Vec3::new(1.0, 0.0, 0.0),
                    joint_axis: glam::Vec3::new(0.0, 0.0, 1.0),
                });
                genotype.add_edge(MorphologyEdge {
                    source_node: 1,
                    target_node: 2,
                    joint_anchor: glam::Vec3::new(1.0, 0.0, 0.0),
                    joint_axis: glam::Vec3::new(0.0, 0.0, 1.0),
                });

                for i in 0..10 {
                    let initial_pos = glam::Vec3::new(i as f32 * 5.0, 0.0, 0.0);
                    let initial_rot = glam::Quat::IDENTITY;
                    let agent_entity =
                        decode_genotype(&mut world, &genotype, initial_pos, initial_rot);
                    let lineage_id = uuid::Uuid::new_v4().to_string();
                    let _ = lineage_tracker_sim.add_root(lineage_id.clone(), genotype.clone());

                    world.entity_mut(agent_entity).insert((
                        AgentGenotype(genotype.clone()),
                        AgentEvaluation {
                            start_position: initial_pos,
                            total_distance: 0.0,
                            total_energy_expended: 0.0,
                            survival_ticks: 0,
                            last_position: initial_pos,
                        },
                        FeatureTracker::default(),
                        AgentLineageId(lineage_id),
                        AgentGeneration(0),
                        crate::core::ecs::AgentParentLineageIds(Vec::new()),
                    ));
                    if i < 7 {
                        world.entity_mut(agent_entity).insert(Prey);
                    } else {
                        world.entity_mut(agent_entity).insert(Predator);
                    }

                    // Genesis creates individuals, so it develops brains (invariant D01). Off
                    // unless `ANIMA_EVOLVED_BRAINS` is set, in which case each founder draws its
                    // own weights from the run's stream — same seed, same founding population.
                    let policy = world
                        .get_resource::<crate::core::resources::BrainPolicy>()
                        .copied()
                        .unwrap_or_default();
                    if policy.evolved {
                        let brain = world
                            .get_resource_mut::<crate::core::resources::SimRng>()
                            .and_then(|mut rng| policy.new_brain(rng.rng()));
                        if let Some(brain) = brain {
                            world.entity_mut(agent_entity).insert(brain);
                        }
                    }
                }

                let env_settings = world
                    .get_resource::<EnvironmentalSpawnSettings>()
                    .cloned()
                    .unwrap_or_default();
                let terrain_map = world
                    .get_resource::<crate::core::terrain::TerrainMap>()
                    .cloned();
                let bounds = world
                    .get_resource::<MapBounds>()
                    .cloned()
                    .unwrap_or_default();

                let mut lake_candidates = Vec::new();
                let mut tree_candidates = Vec::new();

                if let Some(ref tm) = terrain_map {
                    for row in 0..tm.height {
                        for col in 0..tm.width {
                            let idx = row * tm.width + col;
                            let biome = tm.biomes[idx];
                            let elevation = tm.elevations[idx];

                            let px = bounds.min.x
                                + ((col as f32 + 0.5) / tm.width as f32)
                                    * (bounds.max.x - bounds.min.x);
                            let pz = bounds.min.z
                                + ((row as f32 + 0.5) / tm.height as f32)
                                    * (bounds.max.z - bounds.min.z);
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

                use rand::seq::SliceRandom;
                // Setup code, not a system, so it takes its own reproducible stream. The world
                // exists by now, so the run seed is read straight off `SimRng` rather than resolved
                // a second time.
                let run_seed = world.resource::<crate::core::resources::SimRng>().seed();
                let mut rng = crate::core::resources::derived_rng(
                    run_seed,
                    crate::core::resources::sim_stream::WORLD_INIT,
                );

                // Spawn Lakes
                if !lake_candidates.is_empty() {
                    lake_candidates.shuffle(&mut rng);
                    let num_lakes_to_spawn = 5.min(lake_candidates.len());
                    for &pos in lake_candidates.iter().take(num_lakes_to_spawn) {
                        world.spawn((
                            Lake {
                                current_water: env_settings.default_lake_water,
                                max_water: env_settings.default_lake_water,
                                replenishment_rate: env_settings.default_lake_replenish,
                            },
                            Position(pos),
                            crate::physics::SpatialCollider { radius: 10.0 },
                        ));
                    }
                } else {
                    world.spawn((
                        Lake {
                            current_water: env_settings.default_lake_water,
                            max_water: env_settings.default_lake_water,
                            replenishment_rate: env_settings.default_lake_replenish,
                        },
                        Position(glam::Vec3::new(50.0, 0.0, 50.0)),
                        crate::physics::SpatialCollider { radius: 30.0 },
                    ));
                }

                // Spawn Trees
                if !tree_candidates.is_empty() {
                    tree_candidates.shuffle(&mut rng);
                    let num_trees_to_spawn = env_settings.max_tree_count.min(tree_candidates.len());
                    for &pos in tree_candidates.iter().take(num_trees_to_spawn) {
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
                            crate::physics::SpatialCollider { radius: 2.0 },
                        ));
                    }
                } else {
                    world.spawn((
                        Tree {
                            current_fruit: env_settings.default_tree_fruit,
                            max_fruit: env_settings.default_tree_fruit,
                            fruit_growth_rate: env_settings.default_tree_growth,
                            time_since_last_drop: 0.0,
                            seed_drop_cooldown: env_settings.default_seed_cooldown,
                            seed_spread_radius: env_settings.default_seed_spread,
                        },
                        Position(glam::Vec3::new(-50.0, 0.0, -50.0)),
                        crate::physics::SpatialCollider { radius: 10.0 },
                    ));
                }
            }

            let deterministic = world
                .get_resource::<crate::core::determinism::DeterministicMode>()
                .copied()
                .unwrap_or_default();

            let mut schedule = Schedule::default();
            // G1.3: system execution order must be declared, not incidental.
            //
            // Bevy's multi-threaded executor guarantees that two systems with conflicting access
            // never run at the same time, but NOT which of them goes first. The `.after(...)`
            // constraints below pin the order that matters causally; everything else was left to
            // whatever the executor happened to pick, which is not a property of the manifest. That
            // is not a theoretical concern: G1.1 found an energy residual whose *sign* changed
            // between runs because of it, and G1.2's checkpoint gate had to declare its own order
            // to get a stable checksum at all.
            //
            // The single-threaded executor walks the schedule's topological order, which is a
            // function of the declared constraints and insertion order alone — the same binary and
            // manifest therefore produce the same order every time. It costs parallelism, which is
            // the correct trade for a run whose purpose is to be reproducible; an interactive
            // session leaves determinism off and keeps the multi-threaded executor.
            if deterministic.is_enabled() {
                schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
            }
            schedule.add_systems((
                sync_evolution_settings_system,
                receive_environmental_events_system,
                apply_environmental_effects_system.after(receive_environmental_events_system),
                sensory_system,
                action_resolution_system,
                update_cpg_system.after(action_resolution_system),
                resolve_joints_system.after(update_cpg_system),
                integrate_physics_system.after(resolve_joints_system),
                crate::ai::pheromone::agent_release_pheromone_system
                    .after(integrate_physics_system),
                crate::ai::pheromone::update_pheromone_grid_system
                    .after(crate::ai::pheromone::agent_release_pheromone_system),
                crate::ai::pheromone::agent_read_pheromone_system
                    .after(crate::ai::pheromone::update_pheromone_grid_system),
            ));
            schedule.add_systems((
                update_agent_evaluation_system.after(integrate_physics_system),
                crate::core::ecs::check_migration_boundaries_system.after(integrate_physics_system),
                apply_deferred.after(crate::core::ecs::check_migration_boundaries_system),
                wrap_coordinates_system.after(apply_deferred),
                rebuild_spatial_grid_system.after(wrap_coordinates_system),
                crate::core::ecs::process_inbound_migrations_system.after(integrate_physics_system),
                metabolic_decay_system.after(integrate_physics_system),
                spawn_food_system.after(apply_environmental_effects_system),
                detect_food_collisions_system.after(integrate_physics_system),
                combat_system.after(integrate_physics_system),
                hrrl_learning_system.after(metabolic_decay_system),
                // Runs after `hrrl_learning_system`, which is where `LastTransitionState` and the
                // homeostatic deviation this reads are refreshed. Returns immediately unless both
                // evolved brains and lifetime learning are switched on.
                crate::ai::model::lifetime_learning_system.after(hrrl_learning_system),
                check_epoch_completion_system.after(metabolic_decay_system),
                apply_staggered_evolution_system.after(check_epoch_completion_system),
                crate::core::ecs::manual_migration_system.after(integrate_physics_system),
                fruit_growth_system.after(apply_environmental_effects_system),
                lake_replenishment_system.after(apply_environmental_effects_system),
                seed_dropping_system.after(apply_environmental_effects_system),
                detect_environmental_collisions_system.after(integrate_physics_system),
            ));

            // Ecosystem-dynamics systems (Phase 7) in their own tuple — Bevy caps a single
            // add_systems tuple at 20, and `.after(...)` ordering resolves across calls.
            schedule.add_systems((
                herbivore_grazing_system.after(integrate_physics_system),
                resource_field_regrowth_system.after(herbivore_grazing_system),
                // Simulation LOD tier two. Both return immediately without a `DormantCohorts`
                // resource, which nothing inserts by default, so a stock run is unaffected.
                //
                // After physics, so an agent is tiered on the position it actually reached this
                // tick; before the census, because the census is the only place a dormant cohort's
                // energy is counted and it has to see the result of both. Bevy inserts the sync
                // point that applies their commands from these ordering constraints.
                crate::core::aggregate_population::dehydrate_cold_agents_system
                    .after(integrate_physics_system),
                crate::core::aggregate_population::rehydrate_wakeable_chunks_system
                    .after(crate::core::aggregate_population::dehydrate_cold_agents_system),
                // The dormant cohorts' own ecology, sitting where its live counterparts sit: after
                // live grazing and before regrowth, so both consumers draw on the same standing
                // field before it grows back.
                crate::core::aggregate_population::dormant_cohort_ecology_system
                    .after(herbivore_grazing_system)
                    .before(resource_field_regrowth_system),
                ecosystem_census_system
                    .after(resource_field_regrowth_system)
                    .after(crate::core::aggregate_population::rehydrate_wakeable_chunks_system),
            ));

            schedule.run(&mut world);
            let mut query_state = world.query::<(
                Entity,
                &Segment,
                &Position,
                &Rotation,
                &ParentAgent,
                Option<&JointConstraint>,
                Option<&JointAxis>,
            )>();
            let _ = query_state.iter(&world).count();

            let mut tick_count = state_to_load.as_ref().map(|s| s.tick_count).unwrap_or(0);
            let target_frame_duration = Duration::from_secs_f64(1.0 / 60.0);
            let mut total_tick_duration = Duration::ZERO;

            let mut state_buffer = Vec::with_capacity(1000);
            let mut state_raycast_buffer = Vec::with_capacity(1000);
            let mut local_env_state = crate::core::ecs::EnvironmentalState {
                elements: Vec::with_capacity(64),
            };

            while running_clone.load(Ordering::SeqCst) {
                let start_time = Instant::now();

                schedule.run(&mut world);
                tick_count += 1;

                if let Ok(tx) = save_request_rx_clone.try_recv() {
                    let serialized = serialize_world_state(
                        &mut world,
                        tick_count,
                        &chronicle_history_clone_save,
                        &lineage_tracker_sim_save,
                        &evolution_settings_clone_save,
                        &map_elites_grid_clone_save,
                    );
                    let _ = tx.send(serialized);
                }

                state_buffer.clear();

                for (entity, segment, pos, rot, parent_agent, joint_constraint, joint_axis) in
                    query_state.iter(&world)
                {
                    let (yaw, pitch, roll) = rot.0.to_euler(glam::EulerRot::YXZ);

                    let parent_segment_id = world
                        .get::<ParentLink>(entity)
                        .and_then(|parent_link| world.get::<Segment>(parent_link.0))
                        .map(|parent_segment| parent_segment.id);

                    let (energy, hydration) = if let Some(homeo) =
                        world.get::<crate::ai::hrrl::HomeostaticState>(parent_agent.0)
                    {
                        (homeo.energy, homeo.hydration)
                    } else {
                        (0.0, 0.0)
                    };

                    let head_rot = world
                        .get::<Rotation>(parent_agent.0)
                        .map(|r| r.0)
                        .unwrap_or(glam::Quat::IDENTITY);
                    let head_direction = (head_rot * glam::Vec3::Z).to_array();

                    let joint_anchor = joint_constraint
                        .map(|jc| jc.anchor_offset)
                        .unwrap_or(glam::Vec3::ZERO);
                    let j_axis = joint_axis.map(|ja| ja.0).unwrap_or(glam::Vec3::ZERO);

                    let agent_type = if world.get::<Predator>(parent_agent.0).is_some() {
                        Some(crate::core::ecs::AgentType::Predator)
                    } else if world.get::<Prey>(parent_agent.0).is_some() {
                        Some(crate::core::ecs::AgentType::Prey)
                    } else {
                        None
                    };

                    state_buffer.push(SegmentState {
                        agent_id: parent_agent.0.index(),
                        segment_id: segment.id,
                        parent_segment_id,
                        x: pos.0.x,
                        y: pos.0.y,
                        z: pos.0.z,
                        yaw,
                        pitch,
                        roll,
                        joint_anchor_x: joint_anchor.x,
                        joint_anchor_y: joint_anchor.y,
                        joint_anchor_z: joint_anchor.z,
                        joint_axis_x: j_axis.x,
                        joint_axis_y: j_axis.y,
                        joint_axis_z: j_axis.z,
                        energy,
                        hydration,
                        head_direction,
                        agent_type,
                    });
                }

                {
                    let mut shared = agent_states_clone
                        .write()
                        .unwrap_or_else(|e| e.into_inner());
                    std::mem::swap(&mut *shared, &mut state_buffer);
                }

                if let Some(grid) = world.get_resource::<crate::ai::pheromone::PheromoneGrid>() {
                    let mut grid_state = pheromone_grid_state_clone
                        .write()
                        .unwrap_or_else(|e| e.into_inner());
                    grid_state.grid.copy_from_slice(&grid.values);
                }

                state_raycast_buffer.clear();
                if let Some(raycasts_res) = world.get_resource::<crate::core::ecs::ActiveRaycasts>()
                {
                    state_raycast_buffer.extend_from_slice(&raycasts_res.raycasts);
                }
                {
                    let mut shared = active_raycasts_clone
                        .write()
                        .unwrap_or_else(|e| e.into_inner());
                    std::mem::swap(&mut *shared, &mut state_raycast_buffer);
                }

                if let Some(mut combat_res) =
                    world.get_resource_mut::<crate::core::ecs::CombatEvents>()
                {
                    if !combat_res.events.is_empty() {
                        let mut shared = combat_events_clone
                            .write()
                            .unwrap_or_else(|e| e.into_inner());
                        shared.extend(combat_res.events.drain(..));
                    }
                }

                {
                    local_env_state.elements.clear();
                    let mut lake_query = world.query::<(
                        &Position,
                        &crate::physics::SpatialCollider,
                        &crate::core::ecs::Lake,
                    )>();
                    for (pos, collider, lake) in lake_query.iter(&world) {
                        local_env_state
                            .elements
                            .push(crate::core::ecs::EnvironmentalElement {
                                element_type: "lake".to_string(),
                                x: pos.0.x,
                                y: pos.0.z,
                                radius: collider.radius,
                                resources: lake.current_water,
                            });
                    }

                    let mut tree_query = world.query::<(
                        &Position,
                        &crate::physics::SpatialCollider,
                        &crate::core::ecs::Tree,
                    )>();
                    for (pos, collider, tree) in tree_query.iter(&world) {
                        local_env_state
                            .elements
                            .push(crate::core::ecs::EnvironmentalElement {
                                element_type: "tree".to_string(),
                                x: pos.0.x,
                                y: pos.0.z,
                                radius: collider.radius,
                                resources: tree.current_fruit,
                            });
                    }

                    let mut shared = environmental_state_clone
                        .write()
                        .unwrap_or_else(|e| e.into_inner());
                    std::mem::swap(&mut shared.elements, &mut local_env_state.elements);
                }

                // Publish the live ecosystem snapshot: the conserved biomass ledger, the
                // predator/prey split and the biodiversity indices over that split.
                {
                    let (detritus, plants, animals) = world
                        .get_resource::<crate::core::ecology::EcosystemBiomass>()
                        .map(|b| (b.detritus, b.plants, b.animals))
                        .unwrap_or((0.0, 0.0, 0.0));
                    let mut prey_count = 0u32;
                    let mut predator_count = 0u32;
                    // Guild body masses feed the character-displacement / Red-Queen signal
                    // (mean total body mass of prey vs predators).
                    let mut prey_mass_sum = 0.0f32;
                    let mut pred_mass_sum = 0.0f32;
                    let mut prey_q = world
                        .query_filtered::<&crate::core::agent_systems::AgentGenotype, (
                            With<crate::core::components::Agent>,
                            With<crate::core::components::Prey>,
                        )>();
                    for g in prey_q.iter(&world) {
                        prey_count += 1;
                        prey_mass_sum += g.0.total_mass();
                    }
                    let mut pred_q = world
                        .query_filtered::<&crate::core::agent_systems::AgentGenotype, (
                            With<crate::core::components::Agent>,
                            With<crate::core::components::Predator>,
                        )>();
                    for g in pred_q.iter(&world) {
                        predator_count += 1;
                        pred_mass_sum += g.0.total_mass();
                    }
                    let prey_mass = if prey_count > 0 {
                        prey_mass_sum / prey_count as f32
                    } else {
                        0.0
                    };
                    let predator_mass = if predator_count > 0 {
                        pred_mass_sum / predator_count as f32
                    } else {
                        0.0
                    };
                    let archive_coverage = world
                        .get_resource::<crate::core::agent_systems::BevyMapElitesArchive>()
                        .map(|a| a.archive.grid.len() as u32)
                        .unwrap_or(0);
                    let counts = [prey_count, predator_count];
                    let mut shared = ecosystem_state_clone
                        .write()
                        .unwrap_or_else(|e| e.into_inner());
                    shared.detritus = detritus;
                    shared.plants = plants;
                    shared.animals = animals;
                    shared.total = detritus + plants + animals;
                    shared.prey_count = prey_count;
                    shared.predator_count = predator_count;
                    shared.shannon = crate::core::ecology::shannon_index(&counts);
                    shared.simpson = crate::core::ecology::simpson_index(&counts);
                    shared.prey_mass = prey_mass;
                    shared.predator_mass = predator_mass;
                    shared.niche_divergence =
                        crate::core::ecology::niche_divergence(prey_mass, predator_mass);
                    shared.archive_coverage = archive_coverage;
                }

                let elapsed = start_time.elapsed();
                total_tick_duration += elapsed;

                let avg_tick_time = total_tick_duration.as_secs_f64() * 1000.0 / tick_count as f64;
                let actual_fps = 1.0 / elapsed.as_secs_f64();

                {
                    let mut stat = status_clone.lock().unwrap_or_else(|e| e.into_inner());
                    stat.running = true;
                    stat.tick_count = tick_count;
                    stat.avg_tick_time_ms = avg_tick_time;
                    stat.fps = if actual_fps.is_finite() {
                        actual_fps
                    } else {
                        0.0
                    };
                }

                if elapsed < target_frame_duration {
                    thread::sleep(target_frame_duration - elapsed);
                }
            }

            let mut stat = status_clone.lock().unwrap_or_else(|e| e.into_inner());
            stat.running = false;
        });

        let running_clone_emit = Arc::clone(&self.running);
        let agent_states_clone_emit = Arc::clone(&self.agent_states);
        let pheromone_grid_state_emit = Arc::clone(&self.pheromone_grid_state);
        let active_raycasts_emit = Arc::clone(&self.active_raycasts);
        let combat_events_emit = Arc::clone(&self.combat_events);
        let environmental_state_clone_emit = Arc::clone(&self.environmental_state);

        let emit_handle = thread::spawn(move || {
            let mut local_emit_buffer = Vec::with_capacity(1000);
            let mut local_pheromone_emit = crate::ai::pheromone::PheromoneGridState {
                grid: vec![0.0; 128 * 128],
                width: 128,
                height: 128,
            };
            let mut local_raycast_emit = Vec::with_capacity(1000);
            let mut local_combat_emit = Vec::with_capacity(100);

            while running_clone_emit.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(33));

                if let Some(ref handle) = app_handle_emit {
                    local_emit_buffer.clear();
                    {
                        let states = agent_states_clone_emit
                            .read()
                            .unwrap_or_else(|e| e.into_inner());
                        local_emit_buffer.extend_from_slice(&states);
                    }
                    let local_environmental_state = {
                        let shared = environmental_state_clone_emit
                            .read()
                            .unwrap_or_else(|e| e.into_inner());
                        shared.clone()
                    };
                    let mut head_directions = std::collections::HashMap::new();
                    for seg in &local_emit_buffer {
                        if seg.parent_segment_id.is_none() || seg.segment_id == 0 {
                            head_directions.insert(seg.agent_id, seg.head_direction);
                        }
                    }
                    let tick_payload = SimulationTickPayload {
                        segments: local_emit_buffer.clone(),
                        environmental_state: local_environmental_state.clone(),
                        head_directions,
                    };
                    let _ = handle.emit("simulation-tick", &tick_payload);

                    {
                        let shared = pheromone_grid_state_emit
                            .read()
                            .unwrap_or_else(|e| e.into_inner());
                        local_pheromone_emit.grid.copy_from_slice(&shared.grid);
                        local_pheromone_emit.width = shared.width;
                        local_pheromone_emit.height = shared.height;
                    }
                    let _ = handle.emit("pheromone-update", &local_pheromone_emit);

                    local_raycast_emit.clear();
                    {
                        let shared = active_raycasts_emit
                            .read()
                            .unwrap_or_else(|e| e.into_inner());
                        local_raycast_emit.extend_from_slice(&shared);
                    }
                    let _ = handle.emit("raycast-update", &local_raycast_emit);

                    local_combat_emit.clear();
                    {
                        let mut shared = combat_events_emit
                            .write()
                            .unwrap_or_else(|e| e.into_inner());
                        std::mem::swap(&mut *shared, &mut local_combat_emit);
                    }
                    for event in &local_combat_emit {
                        let _ = handle.emit("combat-event", event);
                    }
                }
            }
        });

        let running_clone_net = Arc::clone(&self.running);
        let sharding_config_clone = Arc::clone(&self.sharding_config);
        let inbound_tx_clone = inbound_tx.clone();

        let net_handle = thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                let local_port = {
                    let config = sharding_config_clone.read().unwrap();
                    config.local_port
                };

                // Cross-shard migration is behind the `networking` feature (G2). Without it the thread
                // still exists and still owns its channels — it simply has no transport to run, so a
                // single-node build shuts down through exactly the same path.
                #[cfg(not(feature = "networking"))]
                {
                    let _ = (
                        local_port,
                        inbound_tx_clone,
                        running_clone_net,
                        app_handle_net,
                        outbound_rx,
                        inbound_tx,
                        app_handle,
                    );
                }
                #[cfg(feature = "networking")]
                {
                    let server_fut = run_websocket_server(
                        local_port,
                        inbound_tx_clone,
                        running_clone_net.clone(),
                        app_handle_net,
                    );
                    let client_fut = run_websocket_client(
                        outbound_rx,
                        inbound_tx,
                        running_clone_net,
                        app_handle,
                        local_port,
                    );

                    let _ = tokio::join!(server_fut, client_fut);
                }
            });
        });

        let mut threads_lock = self.threads.lock().unwrap_or_else(|e| e.into_inner());
        *threads_lock = Some(vec![
            sim_handle,
            emit_handle,
            evo_handle,
            net_handle,
            learn_handle,
        ]);
    }

    pub fn stop(&self) {
        while self.manual_migration_receiver.try_recv().is_ok() {}

        if self
            .running
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let mut threads_lock = self.threads.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(handles) = threads_lock.take() {
            for handle in handles {
                let _ = handle.join();
            }
        }
    }

    pub fn get_status(&self) -> SimulationStatus {
        let stat = self.status.lock().unwrap_or_else(|e| e.into_inner());
        *stat
    }
}

fn run_training_loop<B>(
    running: Arc<AtomicBool>,
    trans_rx: crossbeam_channel::Receiver<Transition>,
    model_tx: crossbeam_channel::Sender<ModelUpdate>,
    old_model_rx: crossbeam_channel::Receiver<ModelUpdate>,
    device: B::Device,
    to_model_update: impl Fn(ActorCriticModel<B>) -> ModelUpdate + Send + 'static,
) where
    B: Backend<FloatElem = f32> + 'static,
    B::Device: Clone + Send + Sync + 'static,
    Autodiff<B>: Backend<FloatElem = f32, IntElem = B::IntElem, Device = B::Device>
        + burn::tensor::backend::AutodiffBackend<
            Device = B::Device,
            FloatElem = f32,
            IntElem = B::IntElem,
        > + 'static,
    ActorCriticModel<Autodiff<B>>:
        AutodiffModule<Autodiff<B>, InnerModule = ActorCriticModel<B>> + Send + 'static,
{
    let mut train_model = ActorCriticModel::<Autodiff<B>>::new(15, 64, 4, &device);
    let mut optim = AdamConfig::new().init();

    let mut batch = Vec::new();
    while running.load(Ordering::SeqCst) {
        match trans_rx.recv_timeout(Duration::from_millis(10)) {
            Ok(transition) => {
                batch.push(transition);
                if batch.len() >= 32 {
                    let mut states_vec = Vec::with_capacity(32 * 15);
                    let mut next_states_vec = Vec::with_capacity(32 * 15);
                    let mut actions_vec = Vec::with_capacity(32 * 4);
                    let mut rewards_vec = Vec::with_capacity(32);
                    for t in batch.iter() {
                        states_vec.extend_from_slice(&t.state);
                        next_states_vec.extend_from_slice(&t.next_state);
                        actions_vec.extend_from_slice(&t.action);
                        rewards_vec.push(t.reward);
                    }

                    let states_tensor = Tensor::<Autodiff<B>, 2>::from_data(
                        Data::new(states_vec, Shape::new([32, 15])),
                        &device,
                    );
                    let next_states_tensor = Tensor::<Autodiff<B>, 2>::from_data(
                        Data::new(next_states_vec, Shape::new([32, 15])),
                        &device,
                    );
                    let actions_tensor = Tensor::<Autodiff<B>, 2>::from_data(
                        Data::new(actions_vec, Shape::new([32, 4])),
                        &device,
                    );
                    let rewards_tensor = Tensor::<Autodiff<B>, 2>::from_data(
                        Data::new(rewards_vec, Shape::new([32, 1])),
                        &device,
                    );

                    let (actor_out, critic_out) = train_model.forward(states_tensor.clone());
                    let (_, critic_out_next) = train_model.forward(next_states_tensor.clone());

                    let target = rewards_tensor + critic_out_next.detach() * 0.99;
                    let td_error = target - critic_out.clone();

                    let critic_diff = td_error.clone();
                    let loss_critic = (critic_diff.clone() * critic_diff).mean();

                    let diff = actor_out - actions_tensor;
                    let loss_actor = ((diff.clone() * diff) * (-td_error.detach())).mean();

                    let loss_total = loss_actor + loss_critic * 0.5;

                    let grads = loss_total.backward();
                    let grads_params = GradientsParams::from_grads(grads, &train_model);
                    train_model = optim.step(1e-3, train_model, grads_params);

                    let eval_model = train_model.valid();
                    let _ = model_tx.send(to_model_update(eval_model));
                    batch.clear();
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
        while let Ok(old_model) = old_model_rx.try_recv() {
            drop(old_model);
        }
    }
}
