use bevy_ecs::prelude::*;
use std::sync::Arc;
use crate::core::ecs::{
    Position, ParentAgent, SegmentJointForce,
    EpochManager, FeatureTracker, AgentClass, Predator, Prey,
};
use crate::core::ecs::ActiveEnvironmentEvent;
use crate::core::ecs::Velocity;
use crate::ai::hrrl::HomeostaticState;
use crate::evolution::genotype::{MorphologyGenotype, decode_genotype};

#[derive(Resource)]
pub struct BevyEvolutionSettings(pub Arc<std::sync::Mutex<crate::commands::EvolutionSettings>>);

#[derive(Resource)]
pub struct BevyEvolutionRunning(pub Arc<std::sync::atomic::AtomicBool>);

#[derive(Resource)]
pub struct BevyMapElitesGrid(pub Arc<std::sync::Mutex<crate::commands::MapElitesGridState>>);

#[derive(Resource)]
pub struct BevyEnvironmentSettings(pub Arc<std::sync::Mutex<crate::commands::EnvironmentSettings>>);

#[derive(Resource)]
pub struct BevyManualDisaster(pub Arc<std::sync::Mutex<Option<crate::evolution::meta_ai::EnvironmentalEvent>>>);

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

pub fn sync_environment_settings_system(
    shared_settings: Res<BevyEnvironmentSettings>,
    mut time_step: ResMut<crate::ai::cpg::TimeStep>,
    mut flora_settings: ResMut<crate::core::ecs::FloraSpawnSettings>,
    mut commands: Commands,
    mut cloud_query: Query<(
        Entity,
        &mut crate::core::ecs::Cloud,
        &mut crate::core::ecs::Velocity,
        Option<&crate::core::ecs::BaseCloudProperties>,
    )>,
) {
    if let Ok(settings) = shared_settings.0.try_lock() {
        let time_speed = if settings.time_speed.is_finite() {
            settings.time_speed.clamp(0.0, 5.0)
        } else {
            1.0
        };
        time_step.0 = (1.0 / 60.0) * time_speed;

        let fertility = if settings.fertility.is_finite() {
            settings.fertility.clamp(0.0, 3.0)
        } else {
            1.0
        };
        flora_settings.max_flora_count = (300.0 * fertility) as usize;

        let weather_severity = if settings.weather_severity.is_finite() {
            settings.weather_severity.clamp(0.0, 5.0)
        } else {
            1.0
        };

        for (entity, mut cloud, mut velocity, base_props) in cloud_query.iter_mut() {
            let (base_shadow, base_vel) = if let Some(props) = base_props {
                (props.base_shadow_intensity, props.base_velocity)
            } else {
                let base_shadow = cloud.shadow_intensity;
                let base_vel = velocity.0;
                commands.entity(entity).insert(crate::core::ecs::BaseCloudProperties {
                    base_shadow_intensity: base_shadow,
                    base_velocity: base_vel,
                });
                (base_shadow, base_vel)
            };
            cloud.shadow_intensity = (base_shadow * weather_severity).clamp(0.0, 1.0);
            velocity.0 = base_vel * weather_severity;
        }
    }
}

pub fn receive_manual_disasters_system(
    shared_disaster: Res<BevyManualDisaster>,
    mut active_event: ResMut<crate::core::ecs::ActiveEnvironmentEvent>,
) {
    if let Ok(mut opt_disaster) = shared_disaster.0.try_lock() {
        if let Some(event) = opt_disaster.take() {
            active_event.0 = event;
        }
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
    mut sim_rng: ResMut<crate::core::rng::SimRng>,
) {
    epoch_manager.current_epoch_ticks += 1;
    if epoch_manager.current_epoch_ticks >= epoch_manager.ticks_per_epoch {
        epoch_manager.current_epoch_ticks = 0;
        epoch_manager.current_epoch += 1;

        let dt = time_step.0;
        let mut stats_batch = Vec::new();
        let rng = &mut sim_rng.0;
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

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct OrganismPreset {
    pub name: String,
    pub description: String,
    pub segment_count: usize,
    pub agent_class: String, // "Prey" or "Predator"
    pub initial_energy: f32,
    pub mass: f32,
    pub length: f32,
    pub radius: f32,
    pub frequency: f32,
    pub amplitude: f32,
}

pub fn get_default_presets() -> Vec<OrganismPreset> {
    vec![
        OrganismPreset {
            name: "Default Prey".to_string(),
            description: "Default prey organism".to_string(),
            segment_count: 3,
            agent_class: "Prey".to_string(),
            initial_energy: 100.0,
            mass: 1.0,
            length: 1.0,
            radius: 0.2,
            frequency: 1.0,
            amplitude: 0.5,
        },
        OrganismPreset {
            name: "Default Predator".to_string(),
            description: "Default predator organism".to_string(),
            segment_count: 3,
            agent_class: "Predator".to_string(),
            initial_energy: 120.0,
            mass: 1.2,
            length: 1.0,
            radius: 0.25,
            frequency: 1.2,
            amplitude: 0.6,
        },
        OrganismPreset {
            name: "Giant Prey".to_string(),
            description: "A large, slow-moving prey organism".to_string(),
            segment_count: 5,
            agent_class: "Prey".to_string(),
            initial_energy: 200.0,
            mass: 3.0,
            length: 1.5,
            radius: 0.4,
            frequency: 0.6,
            amplitude: 0.4,
        },
        OrganismPreset {
            name: "Speedy Predator".to_string(),
            description: "A fast, agile predator organism".to_string(),
            segment_count: 2,
            agent_class: "Predator".to_string(),
            initial_energy: 90.0,
            mass: 0.8,
            length: 0.8,
            radius: 0.15,
            frequency: 2.0,
            amplitude: 0.8,
        },
        OrganismPreset {
            name: "Long Snake".to_string(),
            description: "A long snake-like prey organism".to_string(),
            segment_count: 8,
            agent_class: "Prey".to_string(),
            initial_energy: 150.0,
            mass: 0.7,
            length: 0.8,
            radius: 0.12,
            frequency: 1.5,
            amplitude: 0.7,
        },
    ]
}

pub fn build_preset_genotype(preset: &OrganismPreset) -> MorphologyGenotype {
    let mut genotype = MorphologyGenotype::new();
    for i in 0..preset.segment_count {
        genotype.add_node(crate::evolution::genotype::MorphologyNode {
            id: i as u32,
            length: preset.length,
            radius: preset.radius,
            mass: preset.mass,
        });
    }
    for i in 0..(preset.segment_count - 1) {
        genotype.add_edge(crate::evolution::genotype::MorphologyEdge {
            source_node: i as u32,
            target_node: (i + 1) as u32,
            joint_anchor: glam::Vec3::new(preset.length, 0.0, 0.0),
            joint_axis: glam::Vec3::new(0.0, 0.0, 1.0),
        });
    }
    genotype
}

#[derive(Resource)]
pub struct BevyPresetSpawnTrigger(pub crossbeam_channel::Receiver<String>);

#[derive(Resource, Clone)]
pub struct BevyLineageTracker(pub Arc<crate::evolution::lineage::FallbackLineageTracker>);

pub fn process_preset_spawns_system(world: &mut World) {
    let mut presets_to_spawn = Vec::new();
    if let Some(trigger) = world.get_resource::<BevyPresetSpawnTrigger>() {
        while let Ok(preset_name) = trigger.0.try_recv() {
            presets_to_spawn.push(preset_name);
        }
    }

    if presets_to_spawn.is_empty() {
        return;
    }

    let tracker = if let Some(tr) = world.get_resource::<BevyLineageTracker>() {
        tr.0.clone()
    } else {
        return;
    };

    let presets = get_default_presets();

    for preset_name in presets_to_spawn {
        if let Some(preset) = presets.iter().find(|p| p.name == preset_name) {
            let genotype = build_preset_genotype(preset);
            let lineage_id = uuid::Uuid::new_v4().to_string();

            // Register as root in the lineage tracker
            use crate::evolution::lineage::LineageTracker;
            if let Err(e) = tracker.add_root(lineage_id.clone(), genotype.clone()) {
                eprintln!("Failed to add root lineage for preset: {}", e);
            }

            // Spawn procedurally
            let initial_pos = glam::Vec3::ZERO;
            let initial_rot = glam::Quat::IDENTITY;

            let root_entity = decode_genotype(world, &genotype, initial_pos, initial_rot);

            // Update initial energy
            if let Some(mut homeo) = world.get_mut::<HomeostaticState>(root_entity) {
                homeo.energy = preset.initial_energy;
                homeo.energy_target = preset.initial_energy;
            }

            // Update CpgOscillator parameters for all child segments
            let mut query = world.query::<(Entity, &ParentAgent, &mut crate::ai::cpg::CpgOscillator)>();
            for (_entity, parent_agent, mut oscillator) in query.iter_mut(world) {
                if parent_agent.0 == root_entity {
                    oscillator.frequency = preset.frequency;
                    oscillator.amplitude = preset.amplitude;
                }
            }

            // Insert Agent components on root entity
            world.entity_mut(root_entity).insert((
                AgentGenotype(genotype),
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

            if preset.agent_class == "Predator" {
                world.entity_mut(root_entity).insert(Predator);
            } else {
                world.entity_mut(root_entity).insert(Prey);
            }
        }
    }
}

