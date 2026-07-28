use crate::ai::cpg::CpgOscillator;
use crate::ai::hrrl::HomeostaticState;
use crate::core::ecs::ActiveEnvironmentEvent;
use crate::core::ecs::Velocity;
use crate::core::ecs::{
    AgentClass, AgentEpochStats, CognitiveState, EpochManager, EvolutionQueue, EvolutionReceiver,
    EvolutionSender, FeatureTracker, Food, InertiaComponent, ParentAgent, Position, Predator, Prey,
    Rotation, Segment, SegmentJointForce,
};
use crate::evolution::genotype::{decode_genotype, MorphologyGenotype};
use bevy_ecs::prelude::*;
use std::sync::Arc;

#[derive(Resource)]
pub struct BevyEvolutionSettings(pub Arc<std::sync::Mutex<crate::commands::EvolutionSettings>>);

#[derive(Resource)]
pub struct BevyEvolutionRunning(pub Arc<std::sync::atomic::AtomicBool>);

#[derive(Resource)]
pub struct BevyMapElitesGrid(pub Arc<std::sync::Mutex<crate::commands::MapElitesGridState>>);

#[derive(Resource, Clone)]
pub struct BevyAppHandle<R: tauri::Runtime>(pub Option<tauri::AppHandle<R>>);

#[derive(Resource)]
pub struct ActiveEvolutionSettings {
    pub mutation_rate: f32,
    pub selection_bias: f32,
    pub grid_resolution: u32,
}

#[derive(Resource)]
pub struct BevyMapElitesArchive {
    pub archive: crate::evolution::map_elites::MapElitesArchive,
}

#[derive(Component, Clone, Debug)]
pub struct AgentGenotype(pub MorphologyGenotype);

#[derive(Component, Debug, Clone)]
pub struct AgentLineageId(pub String);

#[derive(Component, Debug, Clone, Copy)]
pub struct AgentGeneration(pub u32);

#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentEvaluation {
    pub start_position: glam::Vec3,
    pub total_distance: f32,
    pub total_energy_expended: f32,
    pub survival_ticks: u32,
    pub last_position: glam::Vec3,
}

#[derive(Resource)]
pub struct NextNodeId(pub u32);

#[derive(Resource)]
pub struct EnvironmentalEventReceiver(
    pub crossbeam_channel::Receiver<crate::evolution::meta_ai::EnvironmentalEvent>,
);

pub fn receive_environmental_events_system(
    receiver: Res<EnvironmentalEventReceiver>,
    mut active_event: ResMut<ActiveEnvironmentEvent>,
) {
    while let Ok(event) = receiver.0.try_recv() {
        active_event.0 = event;
    }
}

pub struct SpawnGenotypeCommand {
    pub genotype: MorphologyGenotype,
    pub initial_pos: glam::Vec3,
    pub initial_rot: glam::Quat,
    pub agent_class: AgentClass,
    pub lineage_id: String,
    pub generation: u32,
    pub parent_ids: Vec<String>,
}

/// Reclaim a dying agent's energy reserve into detritus and destroy it, as one indivisible step.
///
/// Splitting those two was an order-dependent energy leak, and a subtle one.
/// `apply_staggered_evolution_system` used to credit detritus with the corpse's reserve
/// *immediately*, but despawn through `Commands`, which do not apply until the end of the schedule
/// run. In between, the agent was still alive holding a reserve that had already been banked:
///
/// - any system running later that tick which burned that reserve (metabolism) credited detritus a
///   second time with energy it had already received — EU created;
/// - any system that fed it (grazing, food, fruit) drew EU out of detritus into a body that was
///   about to be destroyed — EU destroyed.
///
/// Which of the two happened, and how often, depended on the order Bevy's multi-threaded executor
/// happened to pick, so a run's residual drifted a different direction every time and the whole
/// thing looked like floating-point noise. Doing the reclaim at the same sync point as the despawn
/// removes the window entirely: nothing can observe a reserve that has been banked but not yet
/// destroyed.
pub struct ReclaimAndDespawnAgentCommand {
    /// The agent's root entity. Its segments are found by `ParentAgent` and go with it.
    pub root: Entity,
}

impl bevy_ecs::system::Command for ReclaimAndDespawnAgentCommand {
    fn apply(self, world: &mut World) {
        // Zero the reserve as it is banked, so the transfer is exact and there is no reserve left
        // for anything to double-count even in principle.
        let reclaimed = match world.get_mut::<HomeostaticState>(self.root) {
            Some(mut homeo) => {
                let amount = homeo.energy;
                crate::core::energy_ledger::debit_reserve(&mut homeo.energy, amount)
            }
            None => 0.0,
        };
        if reclaimed > 0.0 {
            if let Some(mut pool) =
                world.get_resource_mut::<crate::core::ecology::EcosystemBiomass>()
            {
                pool.detritus += reclaimed;
            }
        }

        let mut doomed: Vec<Entity> = Vec::new();
        let mut q = world.query::<(Entity, &ParentAgent)>();
        for (entity, parent) in q.iter(world) {
            if parent.0 == self.root {
                doomed.push(entity);
            }
        }
        for entity in doomed {
            if let Some(e) = world.get_entity_mut(entity) {
                e.despawn();
            }
        }
        if let Some(e) = world.get_entity_mut(self.root) {
            e.despawn();
        }
    }
}

impl bevy_ecs::system::Command for SpawnGenotypeCommand {
    fn apply(self, world: &mut World) {
        let entity = decode_genotype(world, &self.genotype, self.initial_pos, self.initial_rot);
        world.entity_mut(entity).insert((
            AgentGenotype(self.genotype),
            AgentEvaluation {
                start_position: self.initial_pos,
                total_distance: 0.0,
                total_energy_expended: 0.0,
                survival_ticks: 0,
                last_position: self.initial_pos,
            },
            FeatureTracker::default(),
            AgentLineageId(self.lineage_id),
            AgentGeneration(self.generation),
            crate::core::ecs::AgentParentLineageIds(self.parent_ids),
        ));

        match self.agent_class {
            AgentClass::Predator => {
                world.entity_mut(entity).insert(Predator);
            }
            AgentClass::Prey => {
                world.entity_mut(entity).insert(Prey);
            }
        }

        // Invariant D06: evolutionary replacement is NOT a birth, and its energy must come from
        // the individual it replaces. `decode_genotype` hands every new body a flat starting
        // reserve; before G1.1 that reserve came from nowhere while the replaced individual's own
        // reserve had *also* just been returned to detritus by `apply_staggered_evolution_system`,
        // so every epoch replacement created roughly one full reserve of EU out of thin air.
        //
        // The reserve is now withdrawn from detritus — exactly where the predecessor's energy
        // went. A pool that cannot cover it means the replacement starts hungry and the shortfall
        // is counted in `EnergyLedger::refused`, rather than the world quietly gaining energy.
        // Worlds built without the closed ledger (bare unit-test harnesses) keep the flat grant.
        if world.contains_resource::<crate::core::ecology::EcosystemBiomass>()
            && world.contains_resource::<crate::core::energy_ledger::EnergyLedger>()
        {
            world.resource_scope(
                |world, mut ledger: Mut<crate::core::energy_ledger::EnergyLedger>| {
                    world.resource_scope(
                        |world, mut pool: Mut<crate::core::ecology::EcosystemBiomass>| {
                            if let Some(mut homeo) =
                                world.get_mut::<crate::ai::hrrl::HomeostaticState>(entity)
                            {
                                let requested = homeo.energy;
                                let cap = homeo.energy_target;
                                homeo.energy = 0.0;
                                ledger.transfer_into_reserve(
                                    &mut pool,
                                    crate::core::energy_ledger::Compartment::Detritus,
                                    &mut homeo.energy,
                                    cap,
                                    requested,
                                    crate::core::energy_ledger::EnergyEvent::Replacement,
                                );
                            }
                        },
                    );
                },
            );
        }

        // Evolutionary replacement creates a *new* individual, so it gets a new brain — unlike
        // restore and migration, which carry one that already exists (invariant D01). The draw comes
        // from `SimRng`, so the same run seed produces the same founding brains.
        //
        // Legacy note: this is `EvolutionaryReplacement`, not biological reproduction, so the brain
        // is rolled fresh rather than inherited from the parents that MAP-Elites selected. Making it
        // heritable across replacement is part of the birth work in M5, not of this ADR.
        let policy = world
            .get_resource::<crate::core::resources::BrainPolicy>()
            .copied()
            .unwrap_or_default();
        if policy.evolved {
            let brain = world
                .get_resource_mut::<crate::core::resources::SimRng>()
                .and_then(|mut rng| policy.new_brain(rng.rng()));
            if let Some(brain) = brain {
                world.entity_mut(entity).insert(brain);
            }
        }
    }
}

pub fn sync_evolution_settings_system(
    shared_settings: Res<BevyEvolutionSettings>,
    mut active_settings: ResMut<ActiveEvolutionSettings>,
) {
    if let Ok(settings) = shared_settings.0.try_lock() {
        active_settings.mutation_rate = settings.mutation_rate as f32;
        active_settings.selection_bias = settings.selection_bias as f32;
        active_settings.grid_resolution = settings.grid_resolution;
    }
}

pub fn update_agent_evaluation_system(
    mut agent_query: Query<(Entity, &Position, &mut AgentEvaluation, &HomeostaticState)>,
    segment_query: Query<(
        &ParentAgent,
        &crate::physics::dynamics::RigidBody,
        &Velocity,
        Option<&SegmentJointForce>,
    )>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    let dt = time_step.0;
    let k_base = 0.1;
    let k_mass = 0.05;
    let k_velocity = 0.2;
    let k_force = 0.3;

    for (agent_entity, pos, mut eval, homeo) in agent_query.iter_mut() {
        if homeo.energy <= 0.0 || homeo.hydration <= 0.0 {
            continue;
        }
        eval.survival_ticks += 1;
        let dist = pos.0.distance(eval.last_position);
        eval.total_distance += dist;
        eval.last_position = pos.0;

        let mut total_cost = k_base;
        for (parent, _body, vel, joint_force) in segment_query.iter() {
            if parent.0 == agent_entity {
                let segment_mass = _body.mass;
                let segment_speed = vel.0.length();
                let force_output = joint_force.map(|jf| jf.0).unwrap_or(0.0);

                let segment_cost = (k_mass * segment_mass)
                    + (k_velocity * segment_speed)
                    + (k_force * force_output);
                total_cost += segment_cost;
            }
        }
        eval.total_energy_expended += total_cost * dt;
    }
}

pub fn check_epoch_completion_system(
    mut epoch_manager: ResMut<EpochManager>,
    evolution_sender: Res<EvolutionSender>,
    mut agent_query: Query<(
        Entity,
        &AgentGenotype,
        &AgentEvaluation,
        &HomeostaticState,
        &mut FeatureTracker,
        &AgentLineageId,
        &AgentGeneration,
    )>,
    bounds: Res<crate::core::ecs::MapBounds>,
    time_step: Res<crate::ai::cpg::TimeStep>,
    mut sim_rng: ResMut<crate::core::resources::SimRng>,
) {
    epoch_manager.current_epoch_ticks += 1;
    if epoch_manager.current_epoch_ticks >= epoch_manager.ticks_per_epoch {
        epoch_manager.current_epoch_ticks = 0;
        epoch_manager.current_epoch += 1;

        let dt = time_step.0;
        let mut stats_batch = Vec::new();
        let rng = sim_rng.rng();
        use rand::Rng;

        for (agent_entity, genotype, _eval, _homeo, mut tracker, lineage_id, generation) in
            agent_query.iter_mut()
        {
            let avg_speed = tracker.cumulative_distance / (tracker.tick_count as f32 * dt + 1e-6);
            let efficiency = tracker.cumulative_distance / (tracker.cumulative_energy_decay + 1e-6);
            let fitness = tracker.cumulative_distance + tracker.tick_count as f32;
            // Ecological niche descriptors: body mass (MTE master trait) + foraging range.
            let body_mass = genotype.0.total_mass();
            let foraging_range = tracker.cumulative_distance;

            let spawn_x = rng.gen_range(bounds.min.x..bounds.max.x);
            let spawn_z = rng.gen_range(bounds.min.z..bounds.max.z);
            let next_pos = glam::Vec3::new(spawn_x, 0.0, spawn_z);

            stats_batch.push(AgentEpochStats {
                entity: agent_entity,
                genotype: genotype.0.clone(),
                fitness,
                speed: avg_speed,
                efficiency,
                body_mass,
                foraging_range,
                position: next_pos,
                lineage_id: lineage_id.0.clone(),
                generation: generation.0,
            });

            // Reset FeatureTracker values
            tracker.cumulative_distance = 0.0;
            tracker.cumulative_energy_decay = 0.0;
            tracker.tick_count = 0;
        }

        let _ = evolution_sender.0.send(stats_batch);
    }
}

pub fn apply_staggered_evolution_system(
    mut commands: Commands,
    evolution_receiver: Res<EvolutionReceiver>,
    mut queue: ResMut<EvolutionQueue>,
    position_query: Query<&Position>,
    predator_query: Query<&Predator>,
    // The corpse's reserve and its segments are both handled by
    // `ReclaimAndDespawnAgentCommand` at the sync point, so this system no longer needs the
    // segment query or direct access to the biomass pool.
) {
    // Collect all spawn instructions
    while let Ok((old_entity, next_genotype, initial_pos, lineage_id, generation, parent_ids)) =
        evolution_receiver.0.try_recv()
    {
        queue.pending_replacements.push((
            old_entity,
            next_genotype,
            initial_pos,
            lineage_id,
            generation,
            parent_ids,
        ));
    }

    // Pop at most 1 replacement from the EvolutionQueue per frame
    if let Some((old_entity, next_genotype, default_pos, lineage_id, generation, parent_ids)) =
        queue.pending_replacements.pop()
    {
        let spawn_pos = position_query
            .get(old_entity)
            .map(|p| p.0)
            .unwrap_or(default_pos);

        let agent_class = if predator_query.get(old_entity).is_ok() {
            AgentClass::Predator
        } else {
            AgentClass::Prey
        };

        // Corpse decomposition: the dying agent's remaining reserve returns to the closed detritus
        // pool instead of vanishing at despawn — the death half of the energy cycle
        // (plants → animals → detritus → plants). Reclaim and despawn happen together, in one
        // command, for the reason documented on `ReclaimAndDespawnAgentCommand`.
        commands.add(ReclaimAndDespawnAgentCommand { root: old_entity });

        // Spawn new offspring at the same position
        commands.add(SpawnGenotypeCommand {
            genotype: next_genotype,
            initial_pos: spawn_pos,
            initial_rot: glam::Quat::IDENTITY,
            agent_class,
            lineage_id,
            generation,
            parent_ids,
        });
    }
}

#[derive(Debug, Clone)]
pub struct AgentInferenceRequest {
    pub entity: Entity,
    pub sensory_input: [f32; 15],
    pub request_id: u64,
    /// The agent's own brain, when it has one. Carried as an `Arc` clone — a refcount bump, not a
    /// copy of the weight vector — so attaching it costs the tick path no allocation.
    ///
    /// `None` routes the agent through the shared [`crate::ai::model::BrainModel`] exactly as before.
    pub brain: Option<std::sync::Arc<crate::evolution::brain_genotype::BrainGenotype>>,
}

/// Actions an inference produces.
///
/// Sized for the evolved architecture: `0..CPG_LEN` are the CPG parameters, the rest are the
/// ecological gates (see [`crate::evolution::brain_genotype::action_index`]). A shared-model agent
/// fills only the CPG slots and leaves the gates at their fully-open default, so widening this array
/// does not change what a legacy agent does.
pub const ACTION_SLOTS: usize = crate::evolution::brain_genotype::action_index::COUNT;

#[derive(Debug, Clone)]
pub struct AgentInferenceResponse {
    pub entity: Entity,
    pub actions: [f32; ACTION_SLOTS],
    pub request_id: u64,
}

impl AgentInferenceResponse {
    /// Actions for an agent whose brain produced nothing usable: no locomotion change and every gate
    /// left open. Chosen so a failed inference degrades to "carry on as before", never to an agent
    /// that silently stops eating.
    pub fn open_gates_default() -> [f32; ACTION_SLOTS] {
        let mut actions = [0.0f32; ACTION_SLOTS];
        for slot in actions
            .iter_mut()
            .skip(crate::evolution::brain_genotype::action_index::CPG_LEN)
        {
            *slot = 1.0;
        }
        actions
    }
}

#[derive(Debug, Clone)]
pub struct InferenceRequestBatch {
    pub requests: Vec<AgentInferenceRequest>,
}

/// Batches circulating between the tick loop and the inference worker.
///
/// A hard ceiling rather than an initial size: `sensory_system` skips a tick's inference when the
/// pool is empty instead of allocating, so this bounds the memory the inference path can ever hold.
/// Sixteen is deep enough that the worker has to fall sixteen batches behind before any agent skips
/// a think, and shallow enough that falling behind is felt as a lower think rate rather than as
/// growing memory.
pub const INFERENCE_POOL_BATCHES: usize = 16;

#[derive(Debug, Clone)]
pub struct InferenceResponseBatch {
    pub responses: Vec<AgentInferenceResponse>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponsePoolWaitError {
    Shutdown,
    Disconnected,
    Stalled,
}

/// Length of one missing-response warning window.
///
/// The measured 4,000-agent tick is under five seconds. Thirty seconds permits several such ticks
/// before the worker starts treating the absence as suspicious. Two consecutive windows are
/// required below, so machine suspend/resume or one unusually slow tick cannot stop a healthy run.
pub const INFERENCE_RESPONSE_STALL_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(30);

/// Wait for a response buffer without violating the inference pool's memory bound.
///
/// A slow `action_resolution_system` temporarily owns every response batch. Allocating another one
/// here would make overload permanent heap growth because every batch is recycled. Holding the
/// request until a response buffer returns instead propagates backpressure to the already-bounded
/// request pool. Short polls keep shutdown observable even when the pool is empty.
pub fn wait_for_recycled_response_batch(
    running: &std::sync::atomic::AtomicBool,
    recycle_res_rx: &crossbeam_channel::Receiver<InferenceResponseBatch>,
) -> Result<InferenceResponseBatch, ResponsePoolWaitError> {
    wait_for_recycled_response_batch_until(
        running,
        recycle_res_rx,
        INFERENCE_RESPONSE_STALL_TIMEOUT,
    )
}

/// Deadline-parameterized form used by the worker and deterministic regression tests.
pub fn wait_for_recycled_response_batch_until(
    running: &std::sync::atomic::AtomicBool,
    recycle_res_rx: &crossbeam_channel::Receiver<InferenceResponseBatch>,
    max_wait: std::time::Duration,
) -> Result<InferenceResponseBatch, ResponsePoolWaitError> {
    let mut window_started = std::time::Instant::now();
    let mut expired_windows = 0u8;
    while running.load(std::sync::atomic::Ordering::SeqCst) {
        let remaining = max_wait
            .checked_sub(window_started.elapsed())
            .unwrap_or_default();
        if remaining.is_zero() {
            match recycle_res_rx.try_recv() {
                Ok(batch) => return Ok(batch),
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    return if running.load(std::sync::atomic::Ordering::SeqCst) {
                        Err(ResponsePoolWaitError::Disconnected)
                    } else {
                        Err(ResponsePoolWaitError::Shutdown)
                    };
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {}
            }
            expired_windows += 1;
            if expired_windows >= 2 {
                return Err(ResponsePoolWaitError::Stalled);
            }
            window_started = std::time::Instant::now();
            continue;
        }
        let poll_interval = remaining.min(std::time::Duration::from_millis(10));
        match recycle_res_rx.recv_timeout(poll_interval) {
            Ok(batch) => return Ok(batch),
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                return if running.load(std::sync::atomic::Ordering::SeqCst) {
                    Err(ResponsePoolWaitError::Disconnected)
                } else {
                    Err(ResponsePoolWaitError::Shutdown)
                };
            }
        }
    }
    Err(ResponsePoolWaitError::Shutdown)
}

#[derive(Resource, Clone)]
pub struct InferenceChannels {
    pub req_tx: crossbeam_channel::Sender<InferenceRequestBatch>,
    pub recycle_req_rx: crossbeam_channel::Receiver<InferenceRequestBatch>,
    pub res_rx: crossbeam_channel::Receiver<InferenceResponseBatch>,
    pub recycle_res_tx: crossbeam_channel::Sender<InferenceResponseBatch>,
}

pub fn sensory_system(
    mut agent_query: Query<
        (
            Entity,
            &Position,
            &Rotation,
            &HomeostaticState,
            Option<&Predator>,
            Option<&crate::ai::pheromone::OlfactorySensors>,
            &mut CognitiveState,
            Option<&crate::core::components::AgentBrain>,
        ),
        With<crate::core::ecs::Agent>,
    >,
    food_query: Query<&Position, With<Food>>,
    prey_query: Query<(&Position, &HomeostaticState), (With<crate::core::ecs::Agent>, With<Prey>)>,
    spatial_grid: Option<Res<crate::physics::SpatialHashGrid>>,
    bounds: Option<Res<crate::core::ecs::MapBounds>>,
    collider_query: Query<(&Position, &crate::physics::SpatialCollider)>,
    food_tag_query: Query<(), With<Food>>,
    predator_tag_query: Query<(), With<Predator>>,
    prey_tag_query: Query<(), With<Prey>>,
    parent_agent_query: Query<&ParentAgent>,
    mut active_raycasts: Option<ResMut<crate::core::ecs::ActiveRaycasts>>,
    channels: Res<InferenceChannels>,
    mut ticket_counter: Local<u64>,
    mut local_batch: Local<Option<InferenceRequestBatch>>,
    mut lod: crate::core::simulation_lod::LodGate,
) {
    if let Some(ref mut raycasts_res) = active_raycasts {
        raycasts_res.raycasts.clear();
    }

    // One snapshot for the whole population, so every agent is tiered against the same focus.
    let lod = lod.begin_tick();

    // The recycle pool is the memory bound of the whole inference path, and this is where it was
    // being broken.
    //
    // `simulation_loop` pre-fills the pool with `INFERENCE_POOL_BATCHES` batches and says why: "to
    // ensure zero heap allocations in the hot path". Nothing enforced the number. When the worker
    // had not yet returned a batch, this allocated a fresh one — and because every batch is
    // recycled rather than dropped, that allocation joined the pool permanently. A tick loop that
    // outruns the inference worker therefore grew the pool without limit, one batch per tick, for
    // as long as the run lasted.
    //
    // Measured on 2026-07-28, headless (no webview, no emit, no evolution, 10 agents): **8.5 MB/min,
    // indefinitely** — and the same shape in the desktop app at 14 MB/min, which is 19 GB after a
    // day. It was not the unbounded lineage growth `STATE_OF_THE_PROJECT.md` §3.15 predicts: that
    // path writes nothing unless evolution is running, and it was not.
    //
    // An empty pool now means "the worker is behind", and the answer to that is to skip this tick's
    // inference rather than to buy more memory. The system is already built for it: an agent that
    // does not think keeps its last CPG parameters and goes on moving, eating and metabolising —
    // the same tolerance the LOD tiering above relies on. Degrading the think rate under load is
    // what a real-time simulation is supposed to do; growing without bound is not.
    let Some(mut batch) = local_batch
        .take()
        .or_else(|| channels.recycle_req_rx.try_recv().ok())
    else {
        return;
    };
    batch.requests.clear();

    for (entity, agent_pos, rotation, homeo, opt_predator, opt_sensors, mut cog_state, opt_brain) in
        agent_query.iter_mut()
    {
        if !matches!(*cog_state, CognitiveState::Ready) {
            continue;
        }

        // Simulation LOD: how often this agent gets to think, by where it is. With no focus set —
        // every headless run, and any UI that has not published a view position — `tier_at` returns
        // `Hot` for everything and this is a no-op.
        //
        // A skipped agent is not frozen: it keeps its last CPG parameters and goes on moving,
        // eating and metabolising. Only the inference is skipped, which is the dominant cost.
        if !lod.should_think(agent_pos.0, entity.index()) {
            continue;
        }

        let is_predator = opt_predator.is_some();
        let target_pos = if is_predator {
            let mut nearest_prey = None;
            let mut min_dist_sq = f32::MAX;
            for (prey_pos, prey_homeo) in prey_query.iter() {
                if prey_homeo.energy > 0.0 {
                    let dist_sq = agent_pos.0.distance_squared(prey_pos.0);
                    if dist_sq < min_dist_sq {
                        min_dist_sq = dist_sq;
                        nearest_prey = Some(prey_pos.0);
                    }
                }
            }
            nearest_prey
        } else {
            let mut nearest_food = None;
            let mut min_dist_sq = f32::MAX;
            for food_pos in food_query.iter() {
                let dist_sq = agent_pos.0.distance_squared(food_pos.0);
                if dist_sq < min_dist_sq {
                    min_dist_sq = dist_sq;
                    nearest_food = Some(food_pos.0);
                }
            }
            nearest_food
        };

        let local_target_vec = if let Some(t_pos) = target_pos {
            rotation.0.inverse() * (t_pos - agent_pos.0)
        } else {
            glam::Vec3::ZERO
        };

        let mut hit_distance = 10.0;
        let mut hit_is_food = 0.0;
        let mut hit_is_predator = 0.0;
        let mut hit_is_prey = 0.0;
        let mut hit_type = crate::core::ecs::HitEntityType::None;
        let direction = rotation.0 * glam::Vec3::Z;

        if let (Some(grid), Some(map_bounds)) = (&spatial_grid, &bounds) {
            let ray = crate::physics::Ray3D {
                origin: agent_pos.0,
                direction,
            };

            if let Some(hit) = grid.raycast(&ray, 10.0, map_bounds, &collider_query) {
                let root_agent_id = if let Ok(parent) = parent_agent_query.get(hit.entity) {
                    parent.0
                } else {
                    hit.entity
                };

                if root_agent_id != entity {
                    hit_distance = hit.distance;
                    if food_tag_query.get(hit.entity).is_ok() {
                        hit_is_food = 1.0;
                        hit_type = crate::core::ecs::HitEntityType::Food;
                    } else if predator_tag_query.get(root_agent_id).is_ok() {
                        hit_is_predator = 1.0;
                        hit_type = crate::core::ecs::HitEntityType::Predator;
                    } else if prey_tag_query.get(root_agent_id).is_ok() {
                        hit_is_prey = 1.0;
                        hit_type = crate::core::ecs::HitEntityType::Prey;
                    } else {
                        hit_type = crate::core::ecs::HitEntityType::Obstacle;
                    }
                }
            }
        }

        if let Some(ref mut raycasts_res) = active_raycasts {
            raycasts_res
                .raycasts
                .push(crate::core::ecs::RaycastTelemetry {
                    origin: agent_pos.0.to_array(),
                    direction: direction.to_array(),
                    hit_distance,
                    hit_entity_type: hit_type,
                    agent_id: entity.index(),
                });
        }

        let (left_reading, right_reading) = if let Some(sensors) = opt_sensors {
            (sensors.left_reading, sensors.right_reading)
        } else {
            (0.0, 0.0)
        };

        let state_arr = [
            local_target_vec.x,
            local_target_vec.y,
            local_target_vec.z,
            homeo.energy,
            homeo.energy_target,
            homeo.hydration,
            homeo.hydration_target,
            homeo.temperature,
            homeo.temp_target,
            hit_distance,
            hit_is_food,
            hit_is_predator,
            hit_is_prey,
            left_reading,
            right_reading,
        ];

        let ticket_id = *ticket_counter;
        *ticket_counter += 1;

        *cog_state = CognitiveState::PendingInference(ticket_id);

        batch.requests.push(AgentInferenceRequest {
            entity,
            sensory_input: state_arr,
            request_id: ticket_id,
            // An `Arc` clone: a refcount bump, no weight copy, so the request stays allocation-free.
            //
            // `live()` is the learned network when there is one, else the genome. Both sit behind
            // the same `Arc`, so which branch it takes costs nothing here — the reason learning
            // replaces the whole network on an interval instead of mutating weights every tick.
            brain: opt_brain.map(|b| std::sync::Arc::clone(b.live())),
        });
    }

    if !batch.requests.is_empty() {
        let _ = channels.req_tx.send(batch);
    } else {
        *local_batch = Some(batch);
    }
}

pub fn apply_inertia_to_oscillators(
    agent_entity: Entity,
    cpg_parameters: &[f32; 4],
    segment_query: &Query<(Entity, &ParentAgent, &Segment)>,
    oscillator_query: &mut Query<&mut CpgOscillator>,
    child_buf: &mut Vec<(u32, Entity)>,
) {
    child_buf.clear();
    for (seg_entity, parent, segment) in segment_query.iter() {
        if parent.0 == agent_entity {
            child_buf.push((segment.id, seg_entity));
        }
    }
    child_buf.sort_unstable_by_key(|&(id, _)| id);
    for (seg_idx, &(_, seg_entity)) in child_buf.iter().enumerate() {
        if let Ok(mut osc) = oscillator_query.get_mut(seg_entity) {
            let freq_idx = seg_idx * 2;
            let amp_idx = seg_idx * 2 + 1;
            if let Some(&freq_raw) = cpg_parameters.get(freq_idx) {
                osc.frequency = 0.1 + freq_raw * 2.9;
            }
            if let Some(&amp_raw) = cpg_parameters.get(amp_idx) {
                osc.amplitude = amp_raw * 1.5;
            }
        }
    }
}

pub fn action_resolution_system(
    channels: Res<InferenceChannels>,
    mut agent_query: Query<(
        Entity,
        &mut CognitiveState,
        &mut InertiaComponent,
        Option<&mut crate::ai::hrrl::LastTransitionState>,
        Option<&mut crate::core::components::ActionGates>,
    )>,
    segment_query: Query<(Entity, &ParentAgent, &Segment)>,
    mut oscillator_query: Query<&mut CpgOscillator>,
    mut child_buf: Local<Vec<(u32, Entity)>>,
) {
    while let Ok(batch) = channels.res_rx.try_recv() {
        for response in &batch.responses {
            if let Ok((_entity, mut cog_state, mut inertia, opt_last, opt_gates)) =
                agent_query.get_mut(response.entity)
            {
                if let CognitiveState::PendingInference(ticket_id) = *cog_state {
                    if ticket_id == response.request_id {
                        use crate::evolution::brain_genotype::action_index;

                        // Outputs `0..CPG_LEN` steer locomotion, exactly as before.
                        let mut cpg = [0.0f32; action_index::CPG_LEN];
                        cpg.copy_from_slice(&response.actions[..action_index::CPG_LEN]);
                        inertia.cpg_parameters = cpg;
                        inertia.ticks_pending = 0;

                        // Reset state to Ready
                        *cog_state = CognitiveState::Ready;

                        // The remaining outputs are the ecological gates. A shared-model agent gets
                        // them filled with the fully-open default upstream, so this assignment is a
                        // no-op for it — the legacy path keeps behaving as it did.
                        if let Some(mut gates) = opt_gates {
                            gates.pheromone_emit = response.actions[action_index::PHEROMONE_EMIT];
                            gates.attack_intent = response.actions[action_index::ATTACK_INTENT];
                            gates.feed_intent = response.actions[action_index::FEED_INTENT];
                        }

                        // Save last transition state. This feeds the shared model's A2C update,
                        // which only ever knew about the CPG parameters, so it keeps its 4 slots
                        // rather than growing to hold gates no gradient touches.
                        if let Some(mut last) = opt_last {
                            last.action = cpg;
                            last.has_last = true;
                        }

                        // Apply InertiaComponent parameters to oscillators
                        apply_inertia_to_oscillators(
                            response.entity,
                            &inertia.cpg_parameters,
                            &segment_query,
                            &mut oscillator_query,
                            &mut child_buf,
                        );
                    }
                }
            }
        }
        let _ = channels.recycle_res_tx.send(batch);
    }
}
