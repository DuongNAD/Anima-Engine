use bevy_ecs::prelude::*;

use std::sync::{Arc, RwLock};
use crate::core::ecs::*;
use crate::core::agent_systems::{AgentGenotype, AgentEvaluation, AgentLineageId, AgentGeneration};
use crate::evolution::lineage::LineageTracker;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug)]
pub struct AgentState {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub energy: f32,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default)]
pub struct SegmentState {
    pub agent_id: u32,
    pub segment_id: u32,
    pub parent_segment_id: Option<u32>,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub joint_anchor_x: f32,
    pub joint_anchor_y: f32,
    pub joint_anchor_z: f32,
    pub joint_axis_x: f32,
    pub joint_axis_y: f32,
    pub joint_axis_z: f32,
    pub energy: f32,
    pub hydration: f32,
    pub head_direction: [f32; 3],
    pub agent_type: Option<crate::core::ecs::AgentType>,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct SimulationTickPayload {
    pub segments: Vec<SegmentState>,
    pub environmental_state: crate::core::ecs::EnvironmentalState,
    pub head_directions: std::collections::HashMap<u32, [f32; 3]>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug)]
pub struct SimulationStatus {
    pub running: bool,
    pub tick_count: u64,
    pub avg_tick_time_ms: f64,
    pub fps: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ChronicleEvent {
    pub id: String,
    pub event_type: String, // "Drought" | "TemperatureSpike" | "PredatorWave" | "Abundance"
    pub timestamp: u64,
    pub title: String,
    pub description: String,
    pub parameter_delta: std::collections::HashMap<String, f64>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CpgOscillatorState {
    pub phase: f32,
    pub frequency: f32,
    pub amplitude: f32,
    pub output: f32,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SerializedSegmentState {
    pub segment_id: u32,
    pub position: glam::Vec3,
    pub rotation: glam::Quat,
    pub velocity: glam::Vec3,
    pub oscillator: Option<CpgOscillatorState>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SerializedAgent {
    pub genotype: crate::evolution::genotype::MorphologyGenotype,
    pub class: crate::core::ecs::AgentClass,
    pub lineage_id: String,
    pub generation: u32,
    pub parent_ids: Vec<String>,
    pub evaluation: crate::core::agent_systems::AgentEvaluation,
    pub feature_tracker: crate::core::ecs::FeatureTracker,
    pub root_position: glam::Vec3,
    pub root_rotation: glam::Quat,
    pub root_velocity: glam::Vec3,
    pub homeostatic_state: crate::ai::hrrl::HomeostaticState,
    pub last_transition_state: crate::ai::hrrl::LastTransitionState,
    pub segments: Vec<SerializedSegmentState>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SerializedFood {
    pub position: glam::Vec3,
    pub energy_value: f32,
    pub hydration_value: f32,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SerializedPheromoneGrid {
    pub values: Vec<f32>,
    pub diffusion_rate: f32,
    pub decay_rate: f32,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SerializedLake {
    pub position: glam::Vec3,
    pub radius: f32,
    pub current_water: f32,
    pub max_water: f32,
    pub replenishment_rate: f32,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SerializedTree {
    pub position: glam::Vec3,
    pub radius: f32,
    pub current_fruit: f32,
    pub max_fruit: f32,
    pub fruit_growth_rate: f32,
    pub time_since_last_drop: f32,
    pub seed_drop_cooldown: f32,
    pub seed_spread_radius: f32,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SavedSimulationState {
    pub tick_count: u64,
    pub active_environment_event: crate::evolution::meta_ai::EnvironmentalEvent,
    pub food_spawn_settings: crate::core::ecs::FoodSpawnSettings,
    pub map_bounds: crate::core::ecs::MapBounds,
    pub epoch_manager: crate::core::ecs::EpochManager,
    pub pheromone_grid: SerializedPheromoneGrid,
    pub foods: Vec<SerializedFood>,
    pub agents: Vec<SerializedAgent>,
    pub evolution_settings: crate::commands::EvolutionSettings,
    pub map_elites_grid: crate::commands::MapElitesGridState,
    pub chronicle_history: Vec<ChronicleEvent>,
    pub lineage_nodes: Vec<crate::evolution::lineage::LineageNode>,
    pub lineage_relations: Vec<crate::evolution::lineage::LineageRelation>,
    pub lakes: Vec<SerializedLake>,
    pub trees: Vec<SerializedTree>,
}

pub fn spawn_serialized_agent(world: &mut World, agent: &SerializedAgent) {
    use crate::core::ecs::{AgentClass, AgentParentLineageIds, Position, Rotation, Velocity, Segment, ParentAgent, Predator, Prey};
    use crate::physics::dynamics::RigidBody;
    use crate::ai::cpg::CpgOscillator;
    use crate::evolution::genotype::decode_genotype;
    
    let root_entity = decode_genotype(world, &agent.genotype, agent.root_position, agent.root_rotation);

    world.entity_mut(root_entity).insert((
        AgentGenotype(agent.genotype.clone()),
        agent.evaluation.clone(),
        agent.feature_tracker.clone(),
        AgentLineageId(agent.lineage_id.clone()),
        AgentGeneration(agent.generation),
        AgentParentLineageIds(agent.parent_ids.clone()),
    ));
    world.entity_mut(root_entity).insert(agent.last_transition_state.clone());

    match agent.class {
        AgentClass::Predator => { world.entity_mut(root_entity).insert(Predator); }
        AgentClass::Prey => { world.entity_mut(root_entity).insert(Prey); }
    }

    if let Some(mut homeo) = world.get_mut::<crate::ai::hrrl::HomeostaticState>(root_entity) {
        *homeo = agent.homeostatic_state.clone();
    }

    if let Some(mut pos) = world.get_mut::<Position>(root_entity) { pos.0 = agent.root_position; }
    if let Some(mut rot) = world.get_mut::<Rotation>(root_entity) { rot.0 = agent.root_rotation; }
    if let Some(mut vel) = world.get_mut::<Velocity>(root_entity) { vel.0 = agent.root_velocity; }
    if let Some(mut body) = world.get_mut::<RigidBody>(root_entity) {
        body.velocity = agent.root_velocity;
        body.force = glam::Vec3::ZERO;
    }

    let mut segment_entities = Vec::new();
    let mut query = world.query::<(Entity, &Segment, &ParentAgent)>();
    for (entity, segment, parent_agent) in query.iter(world) {
        if parent_agent.0 == root_entity && entity != root_entity {
            segment_entities.push((entity, segment.id));
        }
    }

    for (entity, segment_id) in segment_entities {
        if let Some(seg_state) = agent.segments.iter().find(|s| s.segment_id == segment_id) {
            if let Some(mut pos) = world.get_mut::<Position>(entity) { pos.0 = seg_state.position; }
            if let Some(mut rot) = world.get_mut::<Rotation>(entity) { rot.0 = seg_state.rotation; }
            if let Some(mut vel) = world.get_mut::<Velocity>(entity) { vel.0 = seg_state.velocity; }
            if let Some(mut body) = world.get_mut::<RigidBody>(entity) {
                body.velocity = seg_state.velocity;
                body.force = glam::Vec3::ZERO;
            }
            if let Some(saved_osc) = &seg_state.oscillator {
                if let Some(mut osc) = world.get_mut::<CpgOscillator>(entity) {
                    osc.phase = saved_osc.phase;
                    osc.frequency = saved_osc.frequency;
                    osc.amplitude = saved_osc.amplitude;
                    osc.output = saved_osc.output;
                }
            }
        }
    }
}

pub fn serialize_world_state(
    world: &mut World,
    tick_count: u64,
    chronicle_history: &Arc<RwLock<Vec<ChronicleEvent>>>,
    lineage_tracker: &Arc<crate::evolution::lineage::FallbackLineageTracker>,
    evolution_settings: &Arc<std::sync::Mutex<crate::commands::EvolutionSettings>>,
    map_elites_grid: &Arc<std::sync::Mutex<crate::commands::MapElitesGridState>>,
) -> SavedSimulationState {
    use crate::core::ecs::{Food, AgentClass, ActiveEnvironmentEvent, FoodSpawnSettings, MapBounds, EpochManager, Predator, Velocity, AgentParentLineageIds, Segment, ParentAgent, Lake, Tree};
    use crate::ai::cpg::CpgOscillator;
    let active_environment_event = world.get_resource::<ActiveEnvironmentEvent>().map(|e| e.0).unwrap_or(crate::evolution::meta_ai::EnvironmentalEvent::Stable);
    let food_spawn_settings = world.get_resource::<FoodSpawnSettings>().cloned().unwrap_or_default();
    let map_bounds = world.get_resource::<MapBounds>().cloned().unwrap_or_default();
    let epoch_manager = world.get_resource::<EpochManager>().cloned().unwrap_or_default();

    let pheromone_grid = if let Some(grid) = world.get_resource::<crate::ai::pheromone::PheromoneGrid>() {
        SerializedPheromoneGrid {
            values: grid.values.clone(),
            diffusion_rate: grid.diffusion_rate,
            decay_rate: grid.decay_rate,
        }
    } else {
        SerializedPheromoneGrid {
            values: vec![0.0; crate::ai::pheromone::CELL_COUNT],
            diffusion_rate: 0.1,
            decay_rate: 0.05,
        }
    };

    let mut foods = Vec::new();
    let mut food_query = world.query::<(&Position, &Food)>();
    for (pos, food) in food_query.iter(world) {
        foods.push(SerializedFood {
            position: pos.0,
            energy_value: food.energy_value,
            hydration_value: food.hydration_value,
        });
    }

    let mut agents = Vec::new();
    let mut agent_query = world.query::<(
        Entity,
        &Position,
        &Rotation,
        &Velocity,
        &crate::ai::hrrl::HomeostaticState,
        &crate::ai::hrrl::LastTransitionState,
        &AgentGenotype,
        &AgentEvaluation,
        &FeatureTracker,
        &AgentLineageId,
        &AgentGeneration,
        &AgentParentLineageIds,
        Option<&Predator>,
    )>();

    let mut collected_agents = Vec::new();
    for (entity, pos, rot, vel, homeo, last_trans, genotype, eval, tracker, lineage_id, gen, parents, predator) in agent_query.iter(world) {
        collected_agents.push((
            entity,
            pos.0,
            rot.0,
            vel.0,
            homeo.clone(),
            last_trans.clone(),
            genotype.0.clone(),
            eval.clone(),
            tracker.clone(),
            lineage_id.0.clone(),
            gen.0,
            parents.0.clone(),
            predator.is_some(),
        ));
    }

    let mut segment_query = world.query::<(Entity, &Segment, &Position, &Rotation, &Velocity, &ParentAgent, Option<&CpgOscillator>)>();
    for (entity, root_pos, root_rot, root_vel, homeo, last_trans, genotype, eval, tracker, lineage_id, gen, parents, is_predator) in collected_agents {
        let class = if is_predator { AgentClass::Predator } else { AgentClass::Prey };
        let mut segments = Vec::new();
        
        for (seg_entity, segment, seg_pos, seg_rot, seg_vel, parent_agent, opt_osc) in segment_query.iter(world) {
            if parent_agent.0 == entity && seg_entity != entity {
                segments.push(SerializedSegmentState {
                    segment_id: segment.id,
                    position: seg_pos.0,
                    rotation: seg_rot.0,
                    velocity: seg_vel.0,
                    oscillator: opt_osc.map(|osc| CpgOscillatorState {
                        phase: osc.phase,
                        frequency: osc.frequency,
                        amplitude: osc.amplitude,
                        output: osc.output,
                    }),
                });
            }
        }

        agents.push(SerializedAgent {
            genotype,
            class,
            lineage_id,
            generation: gen,
            parent_ids: parents,
            evaluation: eval,
            feature_tracker: tracker,
            root_position: root_pos,
            root_rotation: root_rot,
            root_velocity: root_vel,
            homeostatic_state: homeo,
            last_transition_state: last_trans,
            segments,
        });
    }

    let mut lakes = Vec::new();
    let mut lake_query = world.query::<(&Position, &crate::physics::SpatialCollider, &Lake)>();
    for (pos, collider, lake) in lake_query.iter(world) {
        lakes.push(SerializedLake {
            position: pos.0,
            radius: collider.radius,
            current_water: lake.current_water,
            max_water: lake.max_water,
            replenishment_rate: lake.replenishment_rate,
        });
    }

    let mut trees = Vec::new();
    let mut tree_query = world.query::<(&Position, &crate::physics::SpatialCollider, &Tree)>();
    for (pos, collider, tree) in tree_query.iter(world) {
        trees.push(SerializedTree {
            position: pos.0,
            radius: collider.radius,
            current_fruit: tree.current_fruit,
            max_fruit: tree.max_fruit,
            fruit_growth_rate: tree.fruit_growth_rate,
            time_since_last_drop: tree.time_since_last_drop,
            seed_drop_cooldown: tree.seed_drop_cooldown,
            seed_spread_radius: tree.seed_spread_radius,
        });
    }

    SavedSimulationState {
        tick_count,
        active_environment_event,
        food_spawn_settings,
        map_bounds,
        epoch_manager,
        pheromone_grid,
        foods,
        agents,
        evolution_settings: evolution_settings.lock().unwrap().clone(),
        map_elites_grid: map_elites_grid.lock().unwrap().clone(),
        chronicle_history: chronicle_history.read().unwrap().clone(),
        lineage_nodes: lineage_tracker.get_lineage_graph().unwrap_or_default().0,
        lineage_relations: lineage_tracker.get_lineage_graph().unwrap_or_default().1,
        lakes,
        trees,
    }
}
