use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};
use crate::evolution::genotype::MorphologyGenotype;

#[derive(Component, Clone, Debug)]
pub struct Agent;

#[derive(Component, Clone, Copy, Debug)]
pub struct Predator;

#[derive(Component, Clone, Copy, Debug)]
pub struct Prey;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentClass {
    Predator,
    Prey,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitEntityType {
    Food,
    Predator,
    Prey,
    Obstacle,
    None,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug)]
pub struct RaycastTelemetry {
    pub origin: [f32; 3],
    pub direction: [f32; 3],
    pub hit_distance: f32,
    pub hit_entity_type: HitEntityType,
    pub agent_id: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug)]
pub struct CombatEvent {
    pub predator_id: u32,
    pub prey_id: u32,
    pub damage: f32,
    pub energy_transferred: f32,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentType {
    Predator,
    Prey,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Food {
    pub energy_value: f32,
    pub hydration_value: f32,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Position(pub Vec3);

#[derive(Component, Clone, Copy, Debug)]
pub struct Rotation(pub Quat);

#[derive(Component, Clone, Copy, Debug)]
pub struct Velocity(pub Vec3);

#[derive(Component, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct FeatureTracker {
    pub cumulative_distance: f32,
    pub cumulative_energy_decay: f32,
    pub tick_count: u32,
}

impl Default for FeatureTracker {
    fn default() -> Self {
        Self {
            cumulative_distance: 0.0,
            cumulative_energy_decay: 0.0,
            tick_count: 0,
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Segment {
    pub id: u32,
    pub length: f32,
    pub radius: f32,
    pub mass: f32,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct ParentLink(pub Entity);

#[derive(Component, Clone, Debug)]
pub struct ChildrenLinks(pub Vec<Entity>);

#[derive(Component, Clone, Copy, Debug)]
pub struct JointAxis(pub glam::Vec3);

#[derive(Component, Clone, Copy, Debug)]
pub struct ParentAgent(pub Entity);

#[derive(Component, Clone, Copy, Debug)]
pub struct SegmentJointForce(pub f32);

#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentParentLineageIds(pub Vec<String>);

#[derive(Component, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Lake {
    pub current_water: f32,
    pub max_water: f32,
    pub replenishment_rate: f32,
}

#[derive(Component, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Tree {
    pub current_fruit: f32,
    pub max_fruit: f32,
    pub fruit_growth_rate: f32,
    pub time_since_last_drop: f32,
    pub seed_drop_cooldown: f32,
    pub seed_spread_radius: f32,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct EnvironmentalElement {
    #[serde(rename = "type")]
    pub element_type: String, // "lake" | "tree"
    pub x: f32,
    pub y: f32, // Maps to Bevy's z coordinate
    pub radius: f32,
    pub resources: f32, // Maps to current water / current fruit
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct EnvironmentalState {
    pub elements: Vec<EnvironmentalElement>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AgentMigrationData {
    pub genotype: MorphologyGenotype,
    pub homeostatic_state: crate::ai::hrrl::HomeostaticState,
    pub position: glam::Vec3,
    pub velocity: glam::Vec3,
    pub lineage_id: String,
    pub generation: u32,
    pub agent_class: AgentClass,
    pub parent_ids: Vec<String>,
    pub evaluation: Option<crate::core::engine::AgentEvaluation>,
    pub feature_tracker: Option<FeatureTracker>,
    pub last_transition_state: Option<crate::ai::hrrl::LastTransitionState>,
    #[serde(default)]
    pub source_port: u16,
}

#[derive(Clone, Debug)]
pub struct OutboundMigration {
    pub target_port: u16,
    pub data: AgentMigrationData,
    pub bounds_min_x: f32,
    pub bounds_max_x: f32,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct ShardingConfig {
    pub local_port: u16,
    pub left_target_port: Option<u16>,
    pub right_target_port: Option<u16>,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CognitiveState {
    Ready,
    PendingInference(u64),
    Cooldown,
}

impl Default for CognitiveState {
    fn default() -> Self {
        Self::Ready
    }
}

#[derive(Component, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InertiaComponent {
    pub target_velocity: Vec3,
    pub cpg_parameters: [f32; 4],
    pub ticks_pending: u32,
    pub ticks_elapsed: u32,
    pub decay_rate: f32,
}

impl Default for InertiaComponent {
    fn default() -> Self {
        Self {
            target_velocity: Vec3::ZERO,
            cpg_parameters: [1.0, 0.0, 1.0, 0.0],
            ticks_pending: 0,
            ticks_elapsed: 0,
            decay_rate: 0.0,
        }
    }
}

#[derive(Component, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SensoryBufferComponent {
    pub buffer: Vec<f32>,
}

impl Default for SensoryBufferComponent {
    fn default() -> Self {
        Self {
            buffer: Vec::with_capacity(15),
        }
    }
}

