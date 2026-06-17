use bevy_ecs::prelude::*;
use glam::Vec3;
use std::sync::{Arc, RwLock};
use crate::evolution::meta_ai::EnvironmentalEvent;
use crate::evolution::genotype::MorphologyGenotype;
use crate::core::components::{RaycastTelemetry, CombatEvent, ShardingConfig, AgentMigrationData, OutboundMigration};

#[derive(Resource, Default)]
pub struct ActiveRaycasts {
    pub raycasts: Vec<RaycastTelemetry>,
}

#[derive(Resource, Default)]
pub struct CombatEvents {
    pub events: Vec<CombatEvent>,
    pub predator_centroids: Vec<(Entity, Vec3, Vec3, u32)>,
    pub prey_centroids: Vec<(Entity, Vec3, Vec3, u32)>,
}

#[derive(Resource, serde::Serialize, serde::Deserialize, Clone, Copy, Debug)]
pub struct FoodSpawnSettings {
    pub max_food_count: usize,
    pub default_energy: f32,
    pub default_hydration: f32,
}

impl Default for FoodSpawnSettings {
    fn default() -> Self {
        Self {
            max_food_count: 50,
            default_energy: 30.0,
            default_hydration: 20.0,
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveEnvironmentEvent(pub EnvironmentalEvent);

impl Default for ActiveEnvironmentEvent {
    fn default() -> Self {
        Self(EnvironmentalEvent::Stable)
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct SimulationSettings {
    pub target_fps: u32,
}

#[derive(Resource, serde::Serialize, serde::Deserialize, Clone, Copy, Debug)]
pub struct MapBounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl Default for MapBounds {
    fn default() -> Self {
        Self {
            min: Vec3::new(-100.0, 0.0, -100.0),
            max: Vec3::new(100.0, 10.0, 100.0),
        }
    }
}

#[derive(Resource, serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default)]
pub struct EpochManager {
    pub ticks_per_epoch: u64,
    pub current_epoch_ticks: u64,
    pub current_epoch: u32,
}

#[derive(Clone, Debug)]
pub struct AgentEpochStats {
    pub entity: Entity,
    pub genotype: MorphologyGenotype,
    pub fitness: f32,
    pub speed: f32,
    pub efficiency: f32,
    pub position: glam::Vec3,
    pub lineage_id: String,
    pub generation: u32,
}

#[derive(Resource)]
pub struct EvolutionSender(pub crossbeam_channel::Sender<Vec<AgentEpochStats>>);

#[derive(Resource, Clone, Debug, Default)]
pub struct EvolutionQueue {
    pub pending_replacements: Vec<(Entity, MorphologyGenotype, glam::Vec3, String, u32, Vec<String>)>,
}

#[derive(Resource)]
pub struct EvolutionReceiver(pub crossbeam_channel::Receiver<(Entity, MorphologyGenotype, glam::Vec3, String, u32, Vec<String>)>);

#[derive(Resource, Clone)]
pub struct ShardingResource(pub Arc<RwLock<ShardingConfig>>);

#[derive(Resource)]
pub struct InboundMigrationReceiver(pub crossbeam_channel::Receiver<AgentMigrationData>);

#[derive(Resource)]
pub struct OutboundMigrationSender(pub crossbeam_channel::Sender<OutboundMigration>);

#[derive(Resource)]
pub struct BevyMigrationTrigger(pub crossbeam_channel::Receiver<u16>);

#[derive(Resource, serde::Serialize, serde::Deserialize, Clone, Copy, Debug)]
pub struct EnvironmentalSpawnSettings {
    pub max_tree_count: usize,
    pub default_lake_water: f32,
    pub default_lake_replenish: f32,
    pub default_tree_fruit: f32,
    pub default_tree_growth: f32,
    pub default_seed_cooldown: f32,
    pub default_seed_spread: f32,
}

impl Default for EnvironmentalSpawnSettings {
    fn default() -> Self {
        Self {
            max_tree_count: 50,
            default_lake_water: 500.0,
            default_lake_replenish: 5.0,
            default_tree_fruit: 100.0,
            default_tree_growth: 2.0,
            default_seed_cooldown: 15.0,
            default_seed_spread: 20.0,
        }
    }
}
