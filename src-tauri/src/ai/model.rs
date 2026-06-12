use burn::nn::{Linear, LinearConfig};
use burn::module::Module;
use burn::tensor::backend::Backend;
use burn::tensor::{Tensor, Data, Shape};
use bevy_ecs::prelude::*;

use crate::core::ecs::{Position, Rotation, ParentAgent, Segment, Food, Predator, Prey};
use crate::ai::hrrl::HomeostaticState;
use crate::ai::cpg::CpgOscillator;

pub type DefaultBackend = burn_ndarray::NdArray<f32>;

#[derive(Module, Debug)]
pub struct ActorCriticModel<B: Backend> {
    trunk1: Linear<B>,
    trunk2: Linear<B>,
    actor_head: Linear<B>,
    critic_head: Linear<B>,
}

impl<B: Backend> ActorCriticModel<B> {
    pub fn new(input_dim: usize, hidden_dim: usize, action_dim: usize, device: &B::Device) -> Self {
        let trunk1 = LinearConfig::new(input_dim, hidden_dim).init(device);
        let trunk2 = LinearConfig::new(hidden_dim, hidden_dim).init(device);
        let actor_head = LinearConfig::new(hidden_dim, action_dim).init(device);
        let critic_head = LinearConfig::new(hidden_dim, 1).init(device);
        Self {
            trunk1,
            trunk2,
            actor_head,
            critic_head,
        }
    }

    pub fn forward(&self, input: Tensor<B, 2>) -> (Tensor<B, 2>, Tensor<B, 2>) {
        let x = self.trunk1.forward(input);
        let x = burn::tensor::activation::relu(x);
        let x = self.trunk2.forward(x);
        let x = burn::tensor::activation::relu(x);

        let actor_out = self.actor_head.forward(x.clone());
        let actor_out = burn::tensor::activation::sigmoid(actor_out);

        let critic_out = self.critic_head.forward(x);

        (actor_out, critic_out)
    }
}

pub enum BrainModelBackend {
    NdArray(ActorCriticModel<burn_ndarray::NdArray<f32>>, burn_ndarray::NdArrayDevice),
    Wgpu(ActorCriticModel<burn_wgpu::Wgpu<burn_wgpu::AutoGraphicsApi, f32, i32>>, burn_wgpu::WgpuDevice),
}

pub struct BrainModel {
    pub backend: BrainModelBackend,
    pub input_dim: usize,
    pub action_dim: usize,
}

unsafe impl Send for BrainModel {}
unsafe impl Sync for BrainModel {}

impl bevy_ecs::system::Resource for BrainModel {}

impl BrainModel {
    pub fn new(input_dim: usize, hidden_dim: usize, action_dim: usize) -> Self {
        let use_gpu = std::env::var("ANIMA_USE_GPU")
            .map(|val| val != "false" && val != "0")
            .unwrap_or(true);

        if use_gpu {
            let wgpu_res = std::panic::catch_unwind(|| {
                let device = burn_wgpu::WgpuDevice::default();
                let model = ActorCriticModel::<burn_wgpu::Wgpu<burn_wgpu::AutoGraphicsApi, f32, i32>>::new(input_dim, hidden_dim, action_dim, &device);
                (model, device)
            });

            match wgpu_res {
                Ok((model, device)) => {
                    return Self {
                        backend: BrainModelBackend::Wgpu(model, device),
                        input_dim,
                        action_dim,
                    };
                }
                Err(_) => {
                    eprintln!("WGPU initialization failed, falling back to CPU NdArray.");
                }
            }
        }

        let device = burn_ndarray::NdArrayDevice::Cpu;
        let model = ActorCriticModel::<burn_ndarray::NdArray<f32>>::new(input_dim, hidden_dim, action_dim, &device);
        Self {
            backend: BrainModelBackend::NdArray(model, device),
            input_dim,
            action_dim,
        }
    }
}

#[derive(Resource, Default)]
pub struct BrainInferenceBuffer {
    pub inputs: Vec<f32>,
    pub outputs: Vec<f32>,
    pub agent_entities: Vec<Entity>,
    pub child_segments: Vec<(u32, Entity)>,
    pub agent_states: Vec<[f32; 15]>,
    pub segment_by_parent: Vec<(Entity, u32, Entity)>,
    pub segment_list: Vec<(u32, Entity, Option<usize>)>,
    pub parent_head: std::collections::HashMap<Entity, usize>,
}

pub fn brain_inference_system(
    brain_model: Res<BrainModel>,
    mut brain_buf: ResMut<BrainInferenceBuffer>,
    agent_query: Query<(
        Entity,
        &Position,
        &Rotation,
        &HomeostaticState,
        Option<&Predator>,
        Option<&crate::ai::pheromone::OlfactorySensors>,
    ), With<crate::core::ecs::Agent>>,
    food_query: Query<&Position, With<Food>>,
    prey_query: Query<(&Position, &HomeostaticState), (With<crate::core::ecs::Agent>, With<Prey>)>,
    mut oscillator_query: Query<&mut CpgOscillator>,
    segment_query: Query<(Entity, &ParentAgent, &Segment)>,
    mut last_state_query: Query<&mut crate::ai::hrrl::LastTransitionState>,
    spatial_grid: Option<Res<crate::physics::SpatialHashGrid>>,
    bounds: Option<Res<crate::core::ecs::MapBounds>>,
    collider_query: Query<(&Position, &crate::physics::SpatialCollider)>,
    food_tag_query: Query<(), With<Food>>,
    predator_tag_query: Query<(), With<Predator>>,
    prey_tag_query: Query<(), With<Prey>>,
    parent_agent_query: Query<&ParentAgent>,
    mut active_raycasts: Option<ResMut<crate::core::ecs::ActiveRaycasts>>,
) {
    if let Some(ref mut raycasts_res) = active_raycasts {
        raycasts_res.raycasts.clear();
    }

    let mut inputs = std::mem::take(&mut brain_buf.inputs);
    inputs.clear();

    let mut agent_entities = std::mem::take(&mut brain_buf.agent_entities);
    agent_entities.clear();

    let mut agent_inputs_list = std::mem::take(&mut brain_buf.agent_states);
    agent_inputs_list.clear();

    // Loop through agents and construct input features
    for (entity, agent_pos, rotation, homeo, opt_predator, opt_sensors) in agent_query.iter() {
        let is_predator = opt_predator.is_some();
        let target_pos = if is_predator {
            // Predator: target nearest active Prey agent
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
            // Prey: target nearest active Food node
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

        // 1. Raycast logic
        let mut hit_distance = 10.0; // max sensor range
        let mut hit_is_food = 0.0;
        let mut hit_is_predator = 0.0;
        let mut hit_is_prey = 0.0;
        let mut hit_type = crate::core::ecs::HitEntityType::None;
        let direction = rotation.0 * glam::Vec3::Z; // Forward is positive Z

        if let (Some(grid), Some(map_bounds)) = (&spatial_grid, &bounds) {
            let ray = crate::physics::Ray3D {
                origin: agent_pos.0,
                direction,
            };
            
            if let Some(hit) = grid.raycast(&ray, 10.0, map_bounds, &collider_query) {
                // Ignore self-collisions (own body segments)
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

        // 2. Olfactory Readings
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
        agent_inputs_list.push(state_arr);

        inputs.push(local_target_vec.x);
        inputs.push(local_target_vec.y);
        inputs.push(local_target_vec.z);
        inputs.push(homeo.energy);
        inputs.push(homeo.energy_target);
        inputs.push(homeo.hydration);
        inputs.push(homeo.hydration_target);
        inputs.push(homeo.temperature);
        inputs.push(homeo.temp_target);
        inputs.push(hit_distance);
        inputs.push(hit_is_food);
        inputs.push(hit_is_predator);
        inputs.push(hit_is_prey);
        inputs.push(left_reading);
        inputs.push(right_reading);

        agent_entities.push(entity);
    }

    let batch_size = agent_entities.len();
    if batch_size == 0 {
        // Return vectors to buffer
        brain_buf.inputs = inputs;
        brain_buf.agent_entities = agent_entities;
        brain_buf.agent_states = agent_inputs_list;
        return;
    }

    let outputs_vec = match &brain_model.backend {
        BrainModelBackend::NdArray(model, device) => {
            let data = Data::new(inputs, Shape::new([batch_size, 15]));
            let input_tensor = Tensor::<burn_ndarray::NdArray<f32>, 2>::from_data(data, device);
            let (actor_out, _) = model.forward(input_tensor);
            actor_out.into_data().value
        }
        BrainModelBackend::Wgpu(model, device) => {
            let data = Data::new(inputs, Shape::new([batch_size, 15]));
            let input_tensor = Tensor::<burn_wgpu::Wgpu<burn_wgpu::AutoGraphicsApi, f32, i32>, 2>::from_data(data, device);
            let (actor_out, _) = model.forward(input_tensor);
            actor_out.into_data().value
        }
    };

    let mut segment_list = std::mem::take(&mut brain_buf.segment_list);
    segment_list.clear();
    let mut parent_head = std::mem::take(&mut brain_buf.parent_head);
    parent_head.clear();

    for (seg_entity, parent_agent, segment) in segment_query.iter() {
        let parent = parent_agent.0;
        let seg_idx = segment_list.len();
        let next = parent_head.insert(parent, seg_idx);
        segment_list.push((segment.id, seg_entity, next));
    }

    let mut child_segments = std::mem::take(&mut brain_buf.child_segments);

    for (agent_idx, &agent_entity) in agent_entities.iter().enumerate() {
        child_segments.clear();
        if let Some(&first_idx) = parent_head.get(&agent_entity) {
            let mut curr = Some(first_idx);
            while let Some(idx) = curr {
                let (id, seg_entity, next) = segment_list[idx];
                child_segments.push((id, seg_entity));
                curr = next;
            }
        }
        child_segments.sort_unstable_by_key(|&(id, _)| id);

        for (seg_idx, &(_, seg_entity)) in child_segments.iter().enumerate() {
            if let Ok(mut osc) = oscillator_query.get_mut(seg_entity) {
                let freq_idx = agent_idx * 4 + seg_idx * 2;
                let amp_idx = agent_idx * 4 + seg_idx * 2 + 1;
                
                if let Some(&freq_raw) = outputs_vec.get(freq_idx) {
                    osc.frequency = 0.1 + freq_raw * 2.9;
                }
                if let Some(&amp_raw) = outputs_vec.get(amp_idx) {
                    osc.amplitude = amp_raw * 1.5;
                }
            }
        }

        // Save last transition state
        let mut action = [0.0; 4];
        for (k, act_val) in action.iter_mut().enumerate() {
            if let Some(&val) = outputs_vec.get(agent_idx * 4 + k) {
                *act_val = val;
            }
        }
        if let Ok(mut last) = last_state_query.get_mut(agent_entity) {
            last.state = agent_inputs_list[agent_idx];
            last.action = action;
            last.has_last = true;
        }
    }

    // Reclaim vectors
    brain_buf.inputs = outputs_vec;
    brain_buf.agent_entities = agent_entities;
    brain_buf.child_segments = child_segments;
    brain_buf.agent_states = agent_inputs_list;
    brain_buf.segment_list = segment_list;
    brain_buf.parent_head = parent_head;
}

pub fn hrrl_learning_system(
    mut agent_set: ParamSet<(
        Query<(&Position, &HomeostaticState), (With<crate::core::ecs::Agent>, With<Prey>)>,
        Query<(
            Entity,
            &Position,
            &Rotation,
            &mut HomeostaticState,
            &mut crate::ai::hrrl::LastTransitionState,
            Option<&Predator>,
            Option<&crate::ai::pheromone::OlfactorySensors>,
        )>,
    )>,
    food_query: Query<&Position, With<Food>>,
    transition_sender: Option<Res<crate::ai::hrrl::TransitionSender>>,
    spatial_grid: Option<Res<crate::physics::SpatialHashGrid>>,
    bounds: Option<Res<crate::core::ecs::MapBounds>>,
    collider_query: Query<(&Position, &crate::physics::SpatialCollider)>,
    food_tag_query: Query<(), With<Food>>,
    predator_tag_query: Query<(), With<Predator>>,
    prey_tag_query: Query<(), With<Prey>>,
    parent_agent_query: Query<&ParentAgent>,
) {
    let mut prey_data = [(glam::Vec3::ZERO, 0.0f32); 256];
    let mut prey_count = 0;
    for (pos, homeo) in agent_set.p0().iter() {
        if prey_count < 256 {
            prey_data[prey_count] = (pos.0, homeo.energy);
            prey_count += 1;
        }
    }

    let mut agent_query = agent_set.p1();
    for (entity, agent_pos, rotation, mut homeo, mut last, opt_predator, opt_sensors) in agent_query.iter_mut() {
        let is_predator = opt_predator.is_some();
        let target_pos = if is_predator {
            // Predator: target nearest active Prey agent from the pre-collected stack buffer
            let mut nearest_prey = None;
            let mut min_dist_sq = f32::MAX;
            for &(prey_pos, prey_energy) in prey_data.iter().take(prey_count) {
                if prey_energy > 0.0 {
                    let dist_sq = agent_pos.0.distance_squared(prey_pos);
                    if dist_sq < min_dist_sq {
                        min_dist_sq = dist_sq;
                        nearest_prey = Some(prey_pos);
                    }
                }
            }
            nearest_prey
        } else {
            // Prey: target nearest active Food node
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

        // 1. Raycast logic
        let mut hit_distance = 10.0;
        let mut hit_is_food = 0.0;
        let mut hit_is_predator = 0.0;
        let mut hit_is_prey = 0.0;

        if let (Some(grid), Some(map_bounds)) = (&spatial_grid, &bounds) {
            let direction = rotation.0 * glam::Vec3::Z;
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
                    } else if predator_tag_query.get(root_agent_id).is_ok() {
                        hit_is_predator = 1.0;
                    } else if prey_tag_query.get(root_agent_id).is_ok() {
                        hit_is_prey = 1.0;
                    }
                }
            }
        }

        // 2. Olfactory Readings
        let (left_reading, right_reading) = if let Some(sensors) = opt_sensors {
            (sensors.left_reading, sensors.right_reading)
        } else {
            (0.0, 0.0)
        };

        let current_state = [
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

        let current_deviation = homeo.compute_deviation();

        if last.has_last {
            let reward = homeo.previous_deviation - current_deviation;
            if let Some(ref sender) = transition_sender {
                let transition = crate::ai::hrrl::Transition {
                    state: last.state,
                    action: last.action,
                    reward,
                    next_state: current_state,
                };
                let _ = sender.0.send(transition);
            }
        }

        homeo.previous_deviation = current_deviation;
        last.state = current_state;
    }
}
