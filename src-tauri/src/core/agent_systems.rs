use bevy_ecs::prelude::*;
use std::sync::Arc;
use crate::core::ecs::{
    Position, ParentAgent, SegmentJointForce,
    EpochManager, FeatureTracker, AgentClass, Predator, Prey,
    EvolutionQueue, EvolutionSender, EvolutionReceiver, AgentEpochStats,
    Segment, Food, CognitiveState, InertiaComponent,
    Rotation,
};
use crate::core::ecs::ActiveEnvironmentEvent;
use crate::core::ecs::Velocity;
use crate::ai::hrrl::HomeostaticState;
use crate::ai::cpg::CpgOscillator;
use crate::evolution::genotype::{MorphologyGenotype, decode_genotype};

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
pub struct EnvironmentalEventReceiver(pub crossbeam_channel::Receiver<crate::evolution::meta_ai::EnvironmentalEvent>);

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
    segment_query: Query<(&ParentAgent, &crate::physics::dynamics::RigidBody, &Velocity, Option<&SegmentJointForce>)>,
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
) {
    epoch_manager.current_epoch_ticks += 1;
    if epoch_manager.current_epoch_ticks >= epoch_manager.ticks_per_epoch {
        epoch_manager.current_epoch_ticks = 0;
        epoch_manager.current_epoch += 1;

        let dt = time_step.0;
        let mut stats_batch = Vec::new();
        let mut rng = rand::thread_rng();
        use rand::Rng;

        for (agent_entity, genotype, _eval, _homeo, mut tracker, lineage_id, generation) in agent_query.iter_mut() {
            let avg_speed = tracker.cumulative_distance / (tracker.tick_count as f32 * dt + 1e-6);
            let efficiency = tracker.cumulative_distance / (tracker.cumulative_energy_decay + 1e-6);
            let fitness = tracker.cumulative_distance + tracker.tick_count as f32;

            let spawn_x = rng.gen_range(bounds.min.x..bounds.max.x);
            let spawn_z = rng.gen_range(bounds.min.z..bounds.max.z);
            let next_pos = glam::Vec3::new(spawn_x, 0.0, spawn_z);

            stats_batch.push(AgentEpochStats {
                entity: agent_entity,
                genotype: genotype.0.clone(),
                fitness,
                speed: avg_speed,
                efficiency,
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
    parent_agent_query: Query<(Entity, &ParentAgent)>,
    position_query: Query<&Position>,
    predator_query: Query<&Predator>,
) {
    // Collect all spawn instructions
    while let Ok((old_entity, next_genotype, initial_pos, lineage_id, generation, parent_ids)) = evolution_receiver.0.try_recv() {
        queue.pending_replacements.push((old_entity, next_genotype, initial_pos, lineage_id, generation, parent_ids));
    }

    // Pop at most 1 replacement from the EvolutionQueue per frame
    if let Some((old_entity, next_genotype, default_pos, lineage_id, generation, parent_ids)) = queue.pending_replacements.pop() {
        let spawn_pos = position_query.get(old_entity).map(|p| p.0).unwrap_or(default_pos);

        let agent_class = if predator_query.get(old_entity).is_ok() {
            AgentClass::Predator
        } else {
            AgentClass::Prey
        };

        // Despawn old segments
        for (seg_entity, parent) in parent_agent_query.iter() {
            if parent.0 == old_entity {
                commands.entity(seg_entity).despawn();
            }
        }
        // Despawn root entity
        commands.entity(old_entity).despawn();

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
}

#[derive(Debug, Clone)]
pub struct AgentInferenceResponse {
    pub entity: Entity,
    pub actions: [f32; 4],
    pub request_id: u64,
}

#[derive(Debug, Clone)]
pub struct InferenceRequestBatch {
    pub requests: Vec<AgentInferenceRequest>,
}

#[derive(Debug, Clone)]
pub struct InferenceResponseBatch {
    pub responses: Vec<AgentInferenceResponse>,
}

#[derive(Resource, Clone)]
pub struct InferenceChannels {
    pub req_tx: crossbeam_channel::Sender<InferenceRequestBatch>,
    pub recycle_req_rx: crossbeam_channel::Receiver<InferenceRequestBatch>,
    pub res_rx: crossbeam_channel::Receiver<InferenceResponseBatch>,
    pub recycle_res_tx: crossbeam_channel::Sender<InferenceResponseBatch>,
}

pub fn sensory_system(
    mut agent_query: Query<(
        Entity,
        &Position,
        &Rotation,
        &HomeostaticState,
        Option<&Predator>,
        Option<&crate::ai::pheromone::OlfactorySensors>,
        &mut CognitiveState,
    ), With<crate::core::ecs::Agent>>,
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
) {
    if let Some(ref mut raycasts_res) = active_raycasts {
        raycasts_res.raycasts.clear();
    }

    let mut batch = local_batch.take().unwrap_or_else(|| {
        channels.recycle_req_rx.try_recv().unwrap_or_else(|_| InferenceRequestBatch {
            requests: Vec::with_capacity(128),
        })
    });
    batch.requests.clear();

    for (entity, agent_pos, rotation, homeo, opt_predator, opt_sensors, mut cog_state) in agent_query.iter_mut() {
        if !matches!(*cog_state, CognitiveState::Ready) {
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
            raycasts_res.raycasts.push(crate::core::ecs::RaycastTelemetry {
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
    )>,
    segment_query: Query<(Entity, &ParentAgent, &Segment)>,
    mut oscillator_query: Query<&mut CpgOscillator>,
    mut child_buf: Local<Vec<(u32, Entity)>>,
) {
    while let Ok(batch) = channels.res_rx.try_recv() {
        for response in &batch.responses {
            if let Ok((_entity, mut cog_state, mut inertia, opt_last)) = agent_query.get_mut(response.entity) {
                if let CognitiveState::PendingInference(ticket_id) = *cog_state {
                    if ticket_id == response.request_id {
                        // Update InertiaComponent
                        inertia.cpg_parameters = response.actions;
                        inertia.ticks_pending = 0;

                        // Reset state to Ready
                        *cog_state = CognitiveState::Ready;

                        // Save last transition state
                        if let Some(mut last) = opt_last {
                            last.action = response.actions;
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

