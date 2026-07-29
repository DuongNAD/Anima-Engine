use bevy_ecs::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::ai::hrrl::{Transition, TransitionSender};
use crate::ai::model::{BrainInferenceBuffer, BrainModel};
use crate::core::agent_systems::*;
use crate::core::ecs::*;
#[allow(unused_imports)]
use crate::core::networking_systems::*;
use crate::core::resources::EvolutionQueue;
use crate::core::simulation_state::*;
// Thread names and the exit registry `stop` reports against (§3.7).
use crate::core::thread_supervisor as sup;
// The shared learner — its objective, loop and backend threads — lives in `core::training`.
#[cfg(feature = "ml-wgpu")]
use crate::core::training::spawn_wgpu_learner;
use crate::core::training::{spawn_ndarray_learner, ModelUpdate};
use crate::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};
use crate::evolution::lineage::LineageTracker;
use crate::physics::JointConstraint;
use tauri::Emitter;

/// How long `stop` waits for every thread to report exit before naming the stragglers (§3.7).
///
/// Chosen from what the threads actually promise: the sim loop targets 60 FPS, the websocket server
/// re-checks `running` against a 100 ms sleep, and the rest poll their channels. A thread needing more
/// than a second to notice a flag is already misbehaving, so thirty is not a performance budget — it
/// is slack for a loaded CI runner, and anything past it is a real fault rather than slowness.
///
/// On the happy path this costs nothing: `wait_for_exit` returns as soon as the registry empties.
const SHUTDOWN_GRACE_SECS: u64 = 30;

/// Epochs between lineage compactions (OSS-071).
///
/// Compaction walks the whole graph, so running it every epoch would cost more than holding the
/// nodes it frees. Fifty is a deliberately unexciting number: large enough that the walk is
/// amortised to nothing, small enough that the graph cannot grow by more than fifty epochs of
/// genotypes between passes. It is not tuned against a measurement, and the honest reason is that
/// the memory it bounds has never been profiled on a long run — if that changes, tune it then.
const COMPACT_EVERY_EPOCHS: u32 = 50;

/// Nanoseconds since `since`, saturated into a `u64`.
///
/// `Duration::as_nanos` is a `u128` because a duration can outlive a `u64` of nanoseconds (~584
/// years). Truncating with `as u64` would wrap a stalled tick into a small, plausible number;
/// saturating makes it obviously wrong instead, and the capture drops it.
fn elapsed_ns(since: Instant) -> u64 {
    since.elapsed().as_nanos().min(u64::MAX as u128) as u64
}

/// Mean wall-clock milliseconds per tick **this run has measured**.
///
/// The denominator is not the world's `tick_count`. A restored world starts at whatever tick the
/// save carried, while the accumulated duration starts at zero — so dividing by `tick_count`
/// charged this run's work against every tick the world had ever run. Resume a world saved at a
/// million ticks and the status panel reads ~0.001 ms per tick, and it never recovers, because the
/// denominator keeps its head start forever. Both UI readouts of this number colour anything under
/// 2 ms as healthy, so the failure is silent and always in the reassuring direction.
///
/// Zero measured ticks is `0.0` rather than `NaN`: the field is serialised over IPC and rendered
/// with `toFixed(2)`, where a NaN would reach the panel as text.
fn mean_tick_time_ms(total: Duration, measured_ticks: u64) -> f64 {
    if measured_ticks == 0 {
        return 0.0;
    }
    total.as_secs_f64() * 1000.0 / measured_ticks as f64
}

pub struct SimulationEngine {
    pub running: Arc<AtomicBool>,
    pub status: Arc<Mutex<SimulationStatus>>,
    pub agent_states: Arc<RwLock<Vec<SegmentState>>>,
    pub pheromone_grid_state: Arc<RwLock<crate::ai::pheromone::PheromoneGridState>>,
    pub active_raycasts: Arc<RwLock<Vec<crate::core::ecs::RaycastTelemetry>>>,
    pub combat_events: Arc<RwLock<Vec<crate::core::ecs::CombatEvent>>>,
    /// The threads `start` spawned, each paired with the name `stop` reports it by.
    ///
    /// Named because an unnamed `Vec<JoinHandle<()>>` makes a stalled shutdown indistinguishable
    /// across five candidates — see [`crate::core::thread_supervisor`] (§3.7).
    pub threads: Mutex<Option<Vec<(&'static str, thread::JoinHandle<()>)>>>,
    /// Which of those threads have reported exit. Read by `stop` to bound its wait.
    pub supervisor: crate::core::thread_supervisor::ThreadSupervisor,
    pub lineage_tracker: Arc<crate::evolution::lineage::FallbackLineageTracker>,
    pub chronicle_history: Arc<RwLock<Vec<ChronicleEvent>>>,
    pub sharding_config: Arc<RwLock<crate::core::ecs::ShardingConfig>>,
    /// Where simulation detail is centred, written by `set_lod_focus` and read once per tick by
    /// [`crate::core::simulation_lod::sync_lod_focus_system`]. Starts disabled, which tiers every
    /// agent `Hot` — an engine nobody has pointed a camera at behaves exactly as it did before.
    pub lod_focus: crate::core::simulation_lod::SharedLodFocus,
    /// What the observer *did*, queued by Tauri commands and drained into the trace once per tick by
    /// [`crate::core::observer::drain_observer_actions_system`] (ADR-0004 C3).
    ///
    /// Same handle shape as `lod_focus`, and for the same reason: commands run on their own thread and
    /// cannot reach the world's resources.
    pub observer_actions: crate::core::observer::SharedObserverActions,
    /// The in-process tick profiler (`ANIMA_TICK_CAPTURE`, or the capture IPC commands).
    ///
    /// Lives on the engine rather than only in the world for the same reason `lod_focus` does: the
    /// commands that start, stop and export a capture run on another thread and cannot borrow the
    /// simulation's world. Idle until something starts it, and a capture never touches simulation
    /// state — see [`crate::core::tick_capture`].
    pub tick_capture: crate::core::tick_capture::SharedTickCapture,
    /// Monotonic counters for the current run's process-local migration handoff boundary.
    pub migration_handoff_diagnostics: crate::core::resources::MigrationHandoffDiagnostics,
    pub manual_migration_trigger: crossbeam_channel::Sender<u16>,
    pub manual_migration_receiver: crossbeam_channel::Receiver<u16>,

    /// A save request, carrying the channel the sim thread answers on.
    ///
    /// The answer is a `Result` because serializing a world is not guaranteed to succeed. It began
    /// as the channel for a refusal — the snapshot could not carry dormant cohorts, so saving while
    /// anything slept would have written a file silently missing a population. Schema 4 carries
    /// them, so that particular refusal is gone; the fallible shape stays, because the failure it
    /// was protecting against is the kind that is invisible when it is not typed.
    pub save_request_tx:
        crossbeam_channel::Sender<std::sync::mpsc::Sender<Result<SavedSimulationState, String>>>,
    pub save_request_rx:
        crossbeam_channel::Receiver<std::sync::mpsc::Sender<Result<SavedSimulationState, String>>>,
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
            supervisor: crate::core::thread_supervisor::ThreadSupervisor::new(),
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
            lod_focus: crate::core::simulation_lod::SharedLodFocus::new_disabled(),
            observer_actions: crate::core::observer::SharedObserverActions::new(),
            tick_capture: crate::core::tick_capture::SharedTickCapture::new(),
            migration_handoff_diagnostics:
                crate::core::resources::MigrationHandoffDiagnostics::default(),
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
        self.migration_handoff_diagnostics.reset();

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

        let use_gpu = crate::core::resources::gpu_backend_requested();

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
            spawn_wgpu_learner(learn_args, self.supervisor.token(sup::LEARN))
        } else {
            spawn_ndarray_learner(learn_args, self.supervisor.token(sup::LEARN))
        };
        #[cfg(not(feature = "ml-wgpu"))]
        let learn_handle = {
            let _ = has_wgpu;
            spawn_ndarray_learner(learn_args, self.supervisor.token(sup::LEARN))
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

        let evo_exit = self.supervisor.token(sup::EVO);
        let evo_handle = thread::spawn(move || {
            // Dropped when this thread unwinds, panic included — see `core::thread_supervisor`.
            let _exit = evo_exit;
            let initial_resolution = {
                let settings = evolution_settings_clone
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
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
            // Reused across epochs so the hot path does not allocate a fresh Vec per batch.
            let mut new_offspring_ids: Vec<String> = Vec::new();

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
                        let settings = evolution_settings_clone
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
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
                            let grid_state = map_elites_grid_clone
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
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

                        new_offspring_ids.push(offspring_id.clone());

                        let _ = spawn_tx.send((
                            stats.entity,
                            offspring,
                            stats.position,
                            offspring_id,
                            offspring_generation,
                            parent_ids,
                        ));
                    }

                    // ---- OSS-071: bound the lineage tracker's memory -------------------------
                    //
                    // Every reproduction above stored a node holding a full `MorphologyGenotype`
                    // clone, and nothing ever removed one, so this is the growth that had no
                    // ceiling. Compaction drops the branches that went extinct, which is where the
                    // overwhelming majority of those genotypes are.
                    //
                    // The sample set is the part that has to be right. It is NOT "who is alive":
                    // every `lineage_id` in the MAP-Elites archive can be selected as a parent by a
                    // later epoch, and an elite need not be an ancestor of anyone currently alive.
                    // Pruning by liveness alone would delete a node the very next reproduction
                    // names. `add_reproduction` now refuses to write an edge to an unknown parent,
                    // so a mistake here costs one ancestry link rather than an orphan edge that
                    // poisons export and every future compaction.
                    //
                    // Every `COMPACT_EVERY_EPOCHS` rather than every epoch: compaction is O(graph),
                    // and running it on each batch would spend more time walking the lineage than
                    // the lineage costs to hold.
                    if meta_ai_epoch.is_multiple_of(COMPACT_EVERY_EPOCHS) {
                        let mut samples: Vec<String> = archive
                            .grid
                            .values()
                            .map(|elite| elite.lineage_id.clone())
                            .collect();
                        samples.append(&mut new_offspring_ids);
                        match lineage_tracker_evo.compact(&samples) {
                            Ok(report) if report.nodes_removed() > 0 => eprintln!(
                                "lineage compaction: {} -> {} nodes, {} -> {} relations",
                                report.nodes_before,
                                report.nodes_after,
                                report.relations_before,
                                report.relations_after
                            ),
                            Ok(_) => {}
                            // A refusal means the graph is malformed (cycle, orphan edge, duplicate
                            // id). Losing the compaction is the right outcome — silently rewriting
                            // a graph that is already wrong would destroy the evidence.
                            Err(e) => eprintln!("lineage compaction skipped: {e}"),
                        }
                    }
                    new_offspring_ids.clear();
                }
            }
        });

        let app_handle_clone = app_handle.clone();
        let app_handle_emit = app_handle.clone();
        let app_handle_net = app_handle.clone();
        let lineage_tracker_sim = Arc::clone(&self.lineage_tracker);
        let sharding_config_sim = Arc::clone(&self.sharding_config);
        let lod_focus_sim = self.lod_focus.clone();
        // Cloned out here rather than inside the thread closure: touching `self` in there would
        // borrow the engine into the thread and the borrow escapes the method.
        let observer_actions_sim = self.observer_actions.clone();
        let tick_capture_shared = self.tick_capture.clone();
        let migration_handoff_diagnostics_sim = self.migration_handoff_diagnostics.clone();
        let migration_handoff_diagnostics_net = self.migration_handoff_diagnostics.clone();
        // A second handle on the same capture, kept out of the sink so the loop can notice the run
        // finishing. Without it a release build has no way to retrieve a capture at all: the four
        // capture commands are IPC-only, no UI calls them, and a release binary has no DevTools to
        // call them from — so `ANIMA_TICK_CAPTURE` would fill 1800 samples that die with the
        // process. See `ANIMA_TICK_CAPTURE_OUT` below.
        let auto_export_capture = self.tick_capture.clone();
        let app_handle_capture = app_handle.clone();
        let manual_migration_receiver_clone = self.manual_migration_receiver.clone();

        let pending_load_state_clone = Arc::clone(&self.pending_load_state);
        let save_request_rx_clone = self.save_request_rx.clone();
        let chronicle_history_clone_save = Arc::clone(&self.chronicle_history);
        let lineage_tracker_sim_save = Arc::clone(&self.lineage_tracker);
        let evolution_settings_clone_save = Arc::clone(&evolution_settings);
        let map_elites_grid_clone_save = Arc::clone(&map_elites_grid);
        let terrain_map_clone = Arc::clone(&self.terrain_map);

        let (inbound_tx, inbound_rx) = crate::core::resources::inbound_migration_channel();
        let (outbound_tx, outbound_rx) = crate::core::resources::outbound_migration_channel();

        let sim_exit = self.supervisor.token(sup::SIM);
        let sim_handle = thread::spawn(move || {
            let _exit = sim_exit;
            let state_to_load = pending_load_state_clone
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();

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

            // Simulation LOD. Until now this whole subsystem was unreachable from the running app:
            // every system took `Option<Res<..>>` and nothing ever inserted the resources, so ~1900
            // lines of tested code bought the shipped engine exactly nothing. These three lines are
            // what connect it.
            //
            // Tier one is safe to have present unconditionally because its default is off *in the
            // data*, not in the wiring: `LodFocus::default()` is disabled, a disabled focus tiers
            // every agent `Hot`, and `Hot` is the pre-LOD behaviour. So an app that never calls
            // `set_lod_focus` is bit-identical to one built before this existed.
            world.insert_resource(crate::core::simulation_lod::LodFocus::default());
            world.insert_resource(crate::core::simulation_lod::LodBands::default());
            world.insert_resource(lod_focus_sim);

            // ADR-0004. The app has driven `set_lod_focus` from `PixiViewport.tsx` since LOD was
            // wired up, which means the camera has been tiering the world all along — deciding
            // which agents get to think. That is `Inhabit`, and saying so is the whole point of the
            // ADR: the perturbation was always there, only undeclared.
            //
            // Behaviour is unchanged. `Inhabit` is the one policy that lets the focus through, so
            // this is the same engine with an honest label. What it buys is that the effects now
            // have a root — `CAUSE_OBSERVER` — instead of reading as baseline dynamics.
            world.insert_resource(crate::core::observer::ObserverPolicy::Inhabit {
                cause_id: anima_domain::causal::CAUSE_OBSERVER,
            });
            world.insert_resource(crate::core::observer::ObserverTrace::default());
            // ADR-0004 C3. The four IPC commands that already write into a running world —
            // `update_evolution_settings`, `toggle_evolution`, `trigger_migration`,
            // `set_sharding_config` — queue here, and the drain below puts them on the record.
            world.insert_resource(observer_actions_sim);

            // Tier two is not, so it stays behind an explicit switch. It destroys bodies, runs a
            // second ecology, and refuses to save while anything is asleep — see
            // `aggregate_lod_enabled_from_env`. Absent the resource, all three of its systems
            // return on their first line.
            //
            // A restore takes precedence over the switch, and deliberately so: a save that carries
            // dormant cohorts carries agents, and their EU is already counted in the biomass
            // compartments this same load restores. Skipping them because an environment variable
            // happens to be unset in this process would delete a population and leave the ledger
            // describing energy that no longer exists. The file decides, not the shell.
            let saved_cohorts = state_to_load
                .as_ref()
                .and_then(|s| s.dormant_cohorts.as_ref());
            match saved_cohorts {
                Some(saved) => {
                    match crate::core::aggregate_population::DormantCohorts::from_saved(saved) {
                        Ok(cohorts) => world.insert_resource(cohorts),
                        // Refused rather than silently dropped: the alternative is a world whose
                        // animal compartment counts EU held by cohorts that were never restored.
                        Err(why) => eprintln!(
                            "WARNING: dormant cohorts in the snapshot are unusable and were not \
                             restored — {why}. The closed-EU total will read short by whatever \
                             they were holding."
                        ),
                    }
                }
                None if crate::core::aggregate_population::aggregate_lod_enabled_from_env() => {
                    world.insert_resource(
                        crate::core::aggregate_population::DormantCohorts::from_bounds(
                            run_seed,
                            &loaded_bounds,
                        ),
                    );
                }
                None => {}
            }

            let (req_tx, req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
            let (recycle_req_tx, recycle_req_rx) =
                crossbeam_channel::unbounded::<InferenceRequestBatch>();
            let (res_tx, res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();
            let (recycle_res_tx, recycle_res_rx) =
                crossbeam_channel::unbounded::<InferenceResponseBatch>();

            // Pre-populate recycle pools to ensure zero heap allocations in the hot path.
            //
            // This count is the memory bound of the inference path, not a starting point:
            // `sensory_system` returns rather than allocating when the pool is empty, so the number
            // of batches in flight can never exceed it. Before that, an empty pool allocated a new
            // batch which was then recycled into the pool for good — 8.5 MB/min headless, measured.
            for _ in 0..crate::core::agent_systems::INFERENCE_POOL_BATCHES {
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
            // Retained, not dropped. This worker was spawned and its JoinHandle thrown away, so a
            // stopped simulation left an inference thread running until the process happened to
            // exit — and a restart spawned a second one alongside it. It is joined at the end of
            // this thread, below, once `running` is false and the batch channel has closed.
            let inference_handle = thread::spawn(move || {
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
                        // Through `BrainModel`'s own API rather than by reaching into its backend.
                        // A swapped-in model carries the same lazy `Param` cells a fresh one does,
                        // and the replace methods re-materialise them; the field is private now
                        // precisely so this cannot be done any other way.
                        match new_model {
                            ModelUpdate::NdArray(new_m) => {
                                if let Some(old) = brain_model.replace_ndarray_model(new_m) {
                                    let _ = old_model_tx_inference.send(ModelUpdate::NdArray(old));
                                }
                            }
                            #[cfg(feature = "ml-wgpu")]
                            ModelUpdate::Wgpu(new_m) => {
                                if let Some(old) = brain_model.replace_wgpu_model(new_m) {
                                    let _ = old_model_tx_inference.send(ModelUpdate::Wgpu(old));
                                }
                            }
                        }
                    }

                    // Receive request batch
                    if let Ok(req_batch) = req_rx.recv_timeout(Duration::from_millis(2)) {
                        if !req_batch.requests.is_empty() {
                            let mut res_batch =
                                match crate::core::agent_systems::wait_for_recycled_response_batch(
                                    &running_inference,
                                    &recycle_res_rx,
                                ) {
                                    Ok(batch) => batch,
                                    Err(
                                        crate::core::agent_systems::ResponsePoolWaitError::Shutdown,
                                    ) => {
                                        let _ = recycle_req_tx.try_send(req_batch);
                                        break;
                                    }
                                    Err(reason) => {
                                        eprintln!(
                                            "inference response pool failed ({reason:?}); stopping \
                                             the simulation instead of allocating or hanging"
                                        );
                                        running_inference.store(false, Ordering::SeqCst);
                                        let _ = recycle_req_tx.try_send(req_batch);
                                        break;
                                    }
                                };
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
                let settings_lock = evolution_settings_clone_save
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
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
            world.insert_resource(migration_handoff_diagnostics_sim);
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

                // Ten on a line unless `ANIMA_FOUNDING_POPULATION` says otherwise, in which case a
                // grid inset from the map edge. The knob exists because `BENCHMARK_BASELINE.md`
                // asks for a tick cost at 1000 agents and this loop was the reason that row could
                // not be measured: the count was a literal, and `x = i * 5.0` puts the twenty-first
                // founder on the `+100` boundary. Unset is bit-identical to the literal it replaced
                // — see `FoundingPlan` and its tests.
                let founding = crate::core::resources::FoundingPlan::from_env();
                let founding_bounds = world
                    .get_resource::<MapBounds>()
                    .copied()
                    .unwrap_or_default();
                for i in 0..founding.count {
                    let initial_pos = founding.position(i, &founding_bounds);
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

            // In-process tick capture (opt-in, bounded, and inert unless started). The sink is
            // inserted unconditionally so a capture can be started from the UI mid-run without
            // rebuilding the world; the three checkpoint systems it feeds return on their first
            // line while no capture is active. `ANIMA_TICK_CAPTURE` starts one at boot for a run
            // nobody is watching.
            tick_capture_shared.set_executor(crate::core::simulation_schedule::executor_name(
                deterministic,
            ));
            match crate::core::tick_capture::CaptureConfig::from_env() {
                Some(Ok(config)) => {
                    if let Err(e) = tick_capture_shared.start(config) {
                        eprintln!("ANIMA_TICK_CAPTURE is not a usable configuration ({e}); tick capture stays off");
                    }
                }
                Some(Err(e)) => eprintln!(
                    "ANIMA_TICK_CAPTURE is not a usable configuration ({e}); tick capture stays off"
                ),
                None => {}
            }
            world.insert_resource(crate::core::tick_capture::TickCaptureSink::new(
                tick_capture_shared,
            ));

            // Where a completed capture writes itself, from `ANIMA_TICK_CAPTURE_OUT`. Unset means
            // nothing is written and the capture is retrieved over IPC exactly as before.
            let auto_export_name = crate::core::tick_capture::auto_export_name_from_env();
            let mut auto_export_done = false;

            let mut schedule = crate::core::simulation_schedule::build_tick_schedule(deterministic);
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
            // Counted separately from `tick_count`, which a restored world seeds from its save —
            // see `mean_tick_time_ms`. This is the number of ticks *this* thread has timed.
            let mut measured_ticks: u64 = 0;

            let mut state_buffer = Vec::with_capacity(1000);
            let mut state_raycast_buffer = Vec::with_capacity(1000);
            let mut local_env_state = crate::core::ecs::EnvironmentalState {
                elements: Vec::with_capacity(64),
            };

            while running_clone.load(Ordering::SeqCst) {
                let start_time = Instant::now();

                // One resource lookup and one uncontended mutex read decide whether this tick is
                // measured at all; everything below is skipped when it is not.
                let capturing = world
                    .get_resource_mut::<crate::core::tick_capture::TickCaptureSink>()
                    .map(|mut sink| sink.begin_tick())
                    .unwrap_or(false);

                let schedule_started = Instant::now();
                schedule.run(&mut world);
                let schedule_ns = elapsed_ns(schedule_started);
                tick_count += 1;

                if let Ok(tx) = save_request_rx_clone.try_recv() {
                    // No longer refused while anything is dormant: schema 4 carries the cohorts,
                    // their archived genomes and the dormancy RNG position, so a world holding a
                    // sleeping herd is one this format can describe. The `Result` stays because a
                    // save that cannot fail is a claim, not a fact — restore already has one
                    // failure mode (a grid whose chunk count disagrees with its side).
                    let answer = Ok(serialize_world_state(
                        &mut world,
                        tick_count,
                        &chronicle_history_clone_save,
                        &lineage_tracker_sim_save,
                        &evolution_settings_clone_save,
                        &map_elites_grid_clone_save,
                    ));
                    let _ = tx.send(answer);
                }

                // The telemetry bracket starts here, *after* the save request. A save serializes
                // the whole world and is an operator action rather than tick work; folding it into
                // the publish phase would put a several-millisecond spike into a distribution that
                // is meant to describe an ordinary frame.
                let telemetry_started = Instant::now();

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

                let telemetry_ns = elapsed_ns(telemetry_started);
                if capturing {
                    if let Some(mut sink) =
                        world.get_resource_mut::<crate::core::tick_capture::TickCaptureSink>()
                    {
                        sink.commit(tick_count, schedule_ns, telemetry_ns);
                    }
                }

                // Written once, on the tick the capture reaches `Complete`, and only when a name
                // was asked for. Checked after `commit` so the sample that completes the run is in
                // the file, and guarded by a flag so a finished capture is not rewritten every
                // tick for the rest of the session.
                if !auto_export_done {
                    if let Some(name) = auto_export_name.as_deref() {
                        if auto_export_capture.status()
                            == crate::core::tick_capture::CaptureStatus::Complete
                        {
                            auto_export_done = true;
                            match crate::commands::capture::export_capture_to_app_data(
                                &auto_export_capture,
                                app_handle_capture.as_ref(),
                                name,
                            ) {
                                Ok(path) => eprintln!(
                                    "tick capture complete; wrote {} ({} samples)",
                                    path.display(),
                                    auto_export_capture.sample_count()
                                ),
                                Err(e) => eprintln!(
                                    "tick capture complete but ANIMA_TICK_CAPTURE_OUT could not be \
                                     written ({e}); the samples are still retrievable over IPC"
                                ),
                            }
                        }
                    }
                }

                let elapsed = start_time.elapsed();
                total_tick_duration += elapsed;
                measured_ticks += 1;

                let avg_tick_time = mean_tick_time_ms(total_tick_duration, measured_ticks);
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

            // The inference worker watches the same `running` flag, so it is already winding down;
            // joining makes that ordering explicit instead of leaving it to process exit.
            if let Err(e) = inference_handle.join() {
                eprintln!("inference thread panicked: {e:?}");
            }

            let mut stat = status_clone.lock().unwrap_or_else(|e| e.into_inner());
            stat.running = false;
        });

        let emit_handle = crate::core::emit::spawn_emit_thread(
            crate::core::emit::EmitChannels {
                running: Arc::clone(&self.running),
                agent_states: Arc::clone(&self.agent_states),
                pheromone_grid: Arc::clone(&self.pheromone_grid_state),
                active_raycasts: Arc::clone(&self.active_raycasts),
                combat_events: Arc::clone(&self.combat_events),
                environmental_state: Arc::clone(&self.environmental_state),
            },
            app_handle_emit,
            self.supervisor.token(sup::EMIT),
        );

        let running_clone_net = Arc::clone(&self.running);
        let sharding_config_clone = Arc::clone(&self.sharding_config);
        let inbound_tx_clone = inbound_tx.clone();

        let net_exit = self.supervisor.token(sup::NET);
        let net_handle = thread::spawn(move || {
            let _exit = net_exit;
            // Deliberately fatal, and confined to this thread: without a runtime there is no
            // cross-shard migration at all, and every path below is `rt.block_on`. The message is
            // the point — a bare `.unwrap()` here reported only "called Option::unwrap on a None
            // value" from a thread the user never started explicitly.
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!(
                        "migration thread could not start a tokio runtime ({e}); cross-shard \
                         migration is disabled for this run"
                    );
                    return;
                }
            };

            rt.block_on(async {
                let local_port = {
                    let config = sharding_config_clone
                        .read()
                        .unwrap_or_else(|e| e.into_inner());
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
                        migration_handoff_diagnostics_net,
                    );
                }
                #[cfg(feature = "networking")]
                {
                    let server_fut = run_websocket_server_with_diagnostics(
                        local_port,
                        inbound_tx_clone,
                        running_clone_net.clone(),
                        app_handle_net,
                        migration_handoff_diagnostics_net,
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
        // Paired with the same names the supervisor registered, so `stop` can join them in a known
        // order and name whichever one did not come back.
        *threads_lock = Some(vec![
            (sup::SIM, sim_handle),
            (sup::EMIT, emit_handle),
            (sup::EVO, evo_handle),
            (sup::NET, net_handle),
            (sup::LEARN, learn_handle),
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
        let Some(handles) = threads_lock.take() else {
            return;
        };

        // Wait for every thread to *report* exit before joining any of them (§3.7). The order matters:
        // `JoinHandle::join` has no timeout, so joining first means one thread ignoring `running`
        // blocks `stop` forever — which is exactly how this looked from CI, an unkillable wait with no
        // output to say which thread or why.
        let stuck = self
            .supervisor
            .wait_for_exit(std::time::Duration::from_secs(SHUTDOWN_GRACE_SECS));

        for (name, handle) in handles {
            if stuck.contains(&name) {
                // Deliberately not joined. Joining is the unbounded wait this exists to remove;
                // skipping detaches the thread, which leaks it for the life of the process. Leaking
                // one *named* thread and saying so beats a `stop()` that never returns — but it is a
                // trade, and the real fix is whatever made this thread ignore `running`.
                eprintln!(
                    "simulation shutdown: thread '{name}' did not exit within {SHUTDOWN_GRACE_SECS}s \
                     and was left detached; it is leaked for the life of this process"
                );
                continue;
            }
            // The result is inspected rather than discarded: `let _ = join()` made a thread that
            // panicked indistinguishable from one that shut down cleanly.
            if handle.join().is_err() {
                eprintln!("simulation shutdown: thread '{name}' panicked");
            }
        }
    }

    pub fn get_status(&self) -> SimulationStatus {
        let stat = self.status.lock().unwrap_or_else(|e| e.into_inner());
        *stat
    }
}

/// The arithmetic half of the tick-time readout. The behavioural half — that a resumed run reports
/// its own speed rather than one divided by the tick the save happened to carry — is
/// `tests/tick_measurement_tests.rs`, because this function cannot see which counter it was given.
#[cfg(test)]
mod mean_tick_time_tests {
    use super::*;

    #[test]
    fn the_mean_is_the_total_over_the_ticks_that_were_timed() {
        let total = Duration::from_millis(120);
        assert!((mean_tick_time_ms(total, 60) - 2.0).abs() < 1e-9);
        assert!((mean_tick_time_ms(total, 1) - 120.0).abs() < 1e-9);
    }

    /// Before any tick has been timed there is no mean. `NaN` would be serialised over IPC and
    /// rendered into the status panel by `toFixed(2)`.
    #[test]
    fn no_measured_tick_reads_as_zero_not_nan() {
        let mean = mean_tick_time_ms(Duration::from_millis(5), 0);
        assert!(mean.is_finite(), "must not be NaN or infinite");
        assert_eq!(mean, 0.0);
    }

    /// The defect this replaced, stated as arithmetic: the same measured work divided by a world
    /// tick count that a restored save started high reads as a fraction of the real cost. Thirty
    /// ticks of 2 ms resumed at a million ticks reported 0.00006 ms, which both UI readouts colour
    /// as healthy.
    #[test]
    fn a_resumed_worlds_tick_offset_cannot_dilute_the_mean() {
        let measured = 30;
        let total = Duration::from_millis(60);
        let honest = mean_tick_time_ms(total, measured);
        assert!((honest - 2.0).abs() < 1e-9);

        let diluted = total.as_secs_f64() * 1000.0 / (1_000_000.0 + measured as f64);
        assert!(
            diluted < honest / 1000.0,
            "the old expression has to be wrong by orders of magnitude, or this test proves nothing"
        );
    }
}
