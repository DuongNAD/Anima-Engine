use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};
use crate::evolution::genotype::MorphologyGenotype;
use crate::evolution::meta_ai::EnvironmentalEvent;

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

#[derive(Component, Clone, Copy, Debug)]
pub struct Food {
    pub energy_value: f32,
    pub hydration_value: f32,
}

#[derive(Resource, Clone, Copy, Debug)]
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

pub fn apply_environmental_effects_system(
    active_event: Res<ActiveEnvironmentEvent>,
    mut food_settings: ResMut<FoodSpawnSettings>,
    mut agent_query: Query<&mut crate::ai::hrrl::HomeostaticState, With<Agent>>,
) {
    let (max_food_multiplier, target_temp_shift) = match active_event.0 {
        EnvironmentalEvent::ResourceDrought => (0.5, 0.0),
        EnvironmentalEvent::TemperatureSpike => (1.0, 5.0),
        EnvironmentalEvent::GlacialPeriod => (1.0, -5.0),
        EnvironmentalEvent::ToxicDeluge => (0.8, 0.0),
        EnvironmentalEvent::Stable => (1.0, 0.0),
    };

    food_settings.max_food_count = (50.0 * max_food_multiplier) as usize;

    for mut homeo in agent_query.iter_mut() {
        homeo.temp_target = 37.0 + target_temp_shift;
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Position(pub Vec3);

#[derive(Component, Clone, Copy, Debug)]
pub struct Rotation(pub Quat);

#[derive(Component, Clone, Copy, Debug)]
pub struct Velocity(pub Vec3);

#[derive(Resource, Clone, Copy, Debug)]
pub struct SimulationSettings {
    pub target_fps: u32,
}

#[derive(Resource, Clone, Copy, Debug)]
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

// System cập nhật vị trí thực thể dựa trên vận tốc
pub fn update_positions_system(
    mut query: Query<(&mut Position, &Velocity), Without<crate::physics::RigidBody>>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    for (mut pos, vel) in query.iter_mut() {
        pos.0 += vel.0 * time_step.0;
    }
}

// System wrap vị trí xung quanh trục X và Z dựa vào MapBounds
pub fn wrap_coordinates_system(mut query: Query<&mut Position>, bounds: Res<MapBounds>) {
    let x_min = bounds.min.x;
    let x_max = bounds.max.x;
    let x_range = x_max - x_min;

    let z_min = bounds.min.z;
    let z_max = bounds.max.z;
    let z_range = z_max - z_min;

    for mut pos in query.iter_mut() {
        if x_range > 0.0 {
            pos.0.x = x_min + (pos.0.x - x_min).rem_euclid(x_range);
        }
        if z_range > 0.0 {
            pos.0.z = z_min + (pos.0.z - z_min).rem_euclid(z_range);
        }
    }
}

// System giảm năng lượng theo thời gian
pub fn energy_decay_system(
    mut query: Query<&mut crate::ai::hrrl::HomeostaticState>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    let decay = 0.5 * time_step.0;
    for mut homeo in query.iter_mut() {
        homeo.energy = (homeo.energy - decay).max(0.0);
    }
}

// Khởi tạo thế giới mô phỏng Bevy ECS
pub fn init_world() -> World {
    let mut world = World::new();
    world.insert_resource(SimulationSettings { target_fps: 60 });
    world.insert_resource(crate::ai::cpg::TimeStep(1.0 / 60.0));
    let bounds = MapBounds::default();
    world.insert_resource(crate::physics::SpatialHashGrid::new_prepopulated(10.0, &bounds));
    world.insert_resource(bounds);

    // Generate and insert TerrainGrid resource (128x128 grid, seed 42)
    let terrain_grid = crate::core::terrain::generate_terrain(128, 128, 42);
    world.insert_resource(terrain_grid);

    world.insert_resource(ActiveRaycasts { raycasts: Vec::with_capacity(1000) });
    world.insert_resource(CombatEvents {
        events: Vec::with_capacity(1000),
        predator_centroids: Vec::with_capacity(128),
        prey_centroids: Vec::with_capacity(128),
    });
    world.insert_resource(ActiveEnvironmentEvent::default());
    world.insert_resource(FloraSpawnSettings::default());
    world.insert_resource(EnvironmentState::default());

    // RNG dùng chung của mọi system. Phải chèn TRƯỚC các system cần nó.
    world.insert_resource(crate::core::rng::SimRng::default());

    // Spawn 3 clouds with random positions, velocities, and radii
    {
        use rand::Rng;
        let mut rng = crate::core::rng::SimRng::default().0;
        for _ in 0..3 {
            let x = rng.gen_range(-100.0..100.0);
            let z = rng.gen_range(-100.0..100.0);
            let vx = rng.gen_range(-2.0..2.0);
            let vz = rng.gen_range(-2.0..2.0);
            let radius = rng.gen_range(15.0..25.0);
            let shadow_intensity = rng.gen_range(0.7..0.9);
            
            world.spawn((
                Position(glam::vec3(x, 0.0, z)),
                Velocity(glam::vec3(vx, 0.0, vz)),
                Cloud { radius, shadow_intensity },
            ));
        }
    }

    world
}

#[derive(Component, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Flora {
    pub energy: f32,
    pub max_energy: f32,
    pub growth_rate: f32,
    pub decay_rate: f32,
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct FloraSpawnSettings {
    pub max_flora_count: usize,
    pub initial_energy: f32,
    pub default_max_energy: f32,
    pub default_growth_rate: f32,
    pub default_decay_rate: f32,
    pub spontaneous_spawn_chance: f32,
    pub vegetative_reproduction_threshold: f32,
    pub vegetative_reproduction_chance: f32,
}

impl Default for FloraSpawnSettings {
    fn default() -> Self {
        Self {
            max_flora_count: 300,
            initial_energy: 5.0,
            default_max_energy: 100.0,
            default_growth_rate: 1.5,
            default_decay_rate: 0.2,
            spontaneous_spawn_chance: 0.05,
            vegetative_reproduction_threshold: 80.0,
            vegetative_reproduction_chance: 0.002,
        }
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct EnvironmentState {
    pub ambient_light: f32,
    pub elapsed_time: f32,
    pub day_length: f32,
}

impl Default for EnvironmentState {
    fn default() -> Self {
        Self {
            ambient_light: 1.0,
            elapsed_time: 0.0,
            day_length: 120.0,
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct Cloud {
    pub radius: f32,
    pub shadow_intensity: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct BaseCloudProperties {
    pub base_shadow_intensity: f32,
    pub base_velocity: glam::Vec3,
}


pub fn grow_flora_system(
    time_step: Res<crate::ai::cpg::TimeStep>,
    terrain_grid: Res<crate::core::terrain::TerrainGrid>,
    bounds: Res<MapBounds>,
    active_event: Res<ActiveEnvironmentEvent>,
    env_state: Option<Res<EnvironmentState>>,
    mut flora_query: Query<(&Position, &mut Flora)>,
) {
    let dt = time_step.0;
    let light_factor = env_state.map(|e| e.ambient_light).unwrap_or(1.0);

    for (pos, mut flora) in flora_query.iter_mut() {
        let mut moisture_factor = 0.0;
        let mut decay_multiplier = 1.0;

        if let Some(idx) = terrain_grid.pos_to_index(pos.0, &bounds) {
            let cell = terrain_grid.cells[idx];
            
            moisture_factor = match cell.biome {
                crate::core::terrain::BiomeType::Grassland => 1.0,
                crate::core::terrain::BiomeType::Mud => 0.9,
                crate::core::terrain::BiomeType::Sand => 0.3,
                crate::core::terrain::BiomeType::Rock => 0.05,
                crate::core::terrain::BiomeType::Water => {
                    if cell.water_depth > 0.5 {
                        decay_multiplier = 3.0; // Drowning stress
                        0.0
                    } else {
                        0.5 // Shallow water growth
                    }
                }
            };
        }

        // Apply drought event stress multiplier
        if active_event.0 == crate::evolution::meta_ai::EnvironmentalEvent::ResourceDrought {
            decay_multiplier *= 2.0; 
        }

        let growth_delta = flora.growth_rate * moisture_factor * light_factor * dt;
        let decay_delta = flora.decay_rate * decay_multiplier * dt;

        let max_energy = flora.max_energy.max(0.0);
        let next_energy = flora.energy + growth_delta - decay_delta;
        flora.energy = if next_energy.is_nan() {
            0.0
        } else {
            next_energy.clamp(0.0, max_energy)
        };
    }
}

pub fn wither_flora_system(
    mut commands: Commands,
    flora_query: Query<(Entity, &Flora)>,
) {
    for (entity, flora) in flora_query.iter() {
        if flora.energy <= 0.0 || flora.energy.is_nan() {
            commands.entity(entity).despawn();
        }
    }
}

// Toroidal distance helper
pub fn toroidal_distance_sq(p1: glam::Vec3, p2: glam::Vec3, bounds: &MapBounds) -> f32 {
    let x_range = bounds.max.x - bounds.min.x;
    let z_range = bounds.max.z - bounds.min.z;
    if x_range <= 0.0 || z_range <= 0.0 {
        return p1.distance_squared(p2);
    }
    
    let mut dx = (p1.x - p2.x).abs();
    if dx > x_range * 0.5 {
        dx = x_range - dx;
    }
    
    let mut dz = (p1.z - p2.z).abs();
    if dz > z_range * 0.5 {
        dz = z_range - dz;
    }
    
    dx * dx + dz * dz
}

// Calculate the light level factor [0.05, 1.0] at a given position
pub fn get_local_light(
    pos: glam::Vec3,
    ambient_light: f32,
    clouds: &[(glam::Vec3, f32, f32)], // (cloud_pos, radius, shadow_intensity)
    bounds: &MapBounds,
) -> f32 {
    let mut max_shadow: f32 = 0.0;
    
    for &(cloud_pos, radius, intensity) in clouds {
        let dist_sq = toroidal_distance_sq(pos, cloud_pos, bounds);
        let r_sq = radius * radius;
        
        if dist_sq < r_sq {
            let dist = dist_sq.sqrt();
            // Linear falloff: 100% intensity at center, 0% at boundary edge
            let shadow = intensity * (1.0 - dist / radius);
            max_shadow = max_shadow.max(shadow);
        }
    }
    
    // Global ambient light is attenuated by the strongest shadow
    // We clamp the minimum visibility to 0.05 to avoid total blindness (agent failure)
    (ambient_light * (1.0 - max_shadow)).max(0.05)
}

pub fn update_day_night_cycle_system(
    time_step: Res<crate::ai::cpg::TimeStep>,
    mut env_state: ResMut<EnvironmentState>,
) {
    env_state.elapsed_time += time_step.0;
    
    let day_len = if env_state.day_length > 0.0 { env_state.day_length } else { 120.0 };
    
    // Map elapsed time to [0, 2*PI]
    let phase = (env_state.elapsed_time * 2.0 * std::f32::consts::PI / day_len) % (2.0 * std::f32::consts::PI);
    
    // Min light (night): 0.1, Max light (day): 1.0
    let min_light = 0.1;
    let max_light = 1.0;
    
    // Sinusoidal day-night transition
    env_state.ambient_light = min_light + (max_light - min_light) * (0.5 * (1.0 + phase.sin()));
}

pub fn spawn_flora_system(
    mut commands: Commands,
    flora_query: Query<(&Position, &Flora)>,
    bounds: Res<MapBounds>,
    settings: Res<FloraSpawnSettings>,
    terrain: Res<crate::core::terrain::TerrainGrid>,
    mut sim_rng: ResMut<crate::core::rng::SimRng>,
) {
    use rand::Rng;
    let rng = &mut sim_rng.0;
    let mut current_count = flora_query.iter().count();

    // 1. Spontaneous Sprouting (Wind Dispersal)
    if current_count < settings.max_flora_count
        && rng.gen_bool(settings.spontaneous_spawn_chance as f64)
    {
        let x = rng.gen_range(bounds.min.x..bounds.max.x);
        let z = rng.gen_range(bounds.min.z..bounds.max.z);
        let pos = glam::Vec3::new(x, 0.0, z);

        if let Some(idx) = terrain.pos_to_index(pos, &bounds) {
            let cell = terrain.cells[idx];
            if cell.biome == crate::core::terrain::BiomeType::Grassland || cell.biome == crate::core::terrain::BiomeType::Mud {
                commands.spawn((
                    Flora {
                        energy: settings.initial_energy,
                        max_energy: settings.default_max_energy,
                        growth_rate: settings.default_growth_rate,
                        decay_rate: settings.default_decay_rate,
                    },
                    Position(pos),
                    crate::physics::SpatialCollider { radius: 0.3 },
                ));
                current_count += 1;
            }
        }
    }

    // 2. Vegetative Seeding (Local Offspring)
    for (pos, flora) in flora_query.iter() {
        if current_count >= settings.max_flora_count {
            break;
        }
        if flora.energy >= settings.vegetative_reproduction_threshold
            && rng.gen_bool(settings.vegetative_reproduction_chance as f64)
        {
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            let dist = rng.gen_range(2.0..6.0);
            let offset = glam::Vec3::new(angle.cos() * dist, 0.0, angle.sin() * dist);
            let sprout_pos = pos.0 + offset;

            // Apply toroidal wrapping matching wrap_coordinates_system
            let x_min = bounds.min.x;
            let x_max = bounds.max.x;
            let x_range = x_max - x_min;
            let z_min = bounds.min.z;
            let z_max = bounds.max.z;
            let z_range = z_max - z_min;

            let mut wrapped_pos = sprout_pos;
            if x_range > 0.0 {
                wrapped_pos.x = x_min + (sprout_pos.x - x_min).rem_euclid(x_range);
            }
            if z_range > 0.0 {
                wrapped_pos.z = z_min + (sprout_pos.z - z_min).rem_euclid(z_range);
            }

            if let Some(idx) = terrain.pos_to_index(wrapped_pos, &bounds) {
                let cell = terrain.cells[idx];
                if cell.biome == crate::core::terrain::BiomeType::Grassland || cell.biome == crate::core::terrain::BiomeType::Mud {
                    commands.spawn((
                        Flora {
                            energy: settings.initial_energy,
                            max_energy: settings.default_max_energy,
                            growth_rate: settings.default_growth_rate,
                            decay_rate: settings.default_decay_rate,
                        },
                        Position(wrapped_pos),
                        crate::physics::SpatialCollider { radius: 0.3 },
                    ));
                    current_count += 1;
                }
            }
        }
    }
}


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

#[derive(Resource, Clone, Copy, Debug)]
pub struct EpochManager {
    pub ticks_per_epoch: u64,
    pub current_epoch_ticks: u64,
    pub current_epoch: u32,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct EvolutionQueue {
    pub pending_replacements: Vec<(Entity, MorphologyGenotype, glam::Vec3, String, u32)>,
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

pub fn metabolic_decay_system(
    mut agent_query: Query<(Entity, &mut crate::ai::hrrl::HomeostaticState, Option<&mut FeatureTracker>, Option<&Velocity>, Option<&Predator>)>,
    segment_query: Query<(&ParentAgent, &crate::physics::dynamics::RigidBody, &Velocity, Option<&SegmentJointForce>)>,
    time_step: Res<crate::ai::cpg::TimeStep>,
) {
    let dt = time_step.0;

    let k_mass = 0.05;
    let k_velocity = 0.2;
    let k_force = 0.3;

    for (agent_entity, mut homeo, opt_tracker, velocity, opt_predator) in agent_query.iter_mut() {
        let k_base = if opt_predator.is_some() { 0.2 } else { 0.1 };
        let mut total_cost = k_base;
        for (parent, body, vel, joint_force) in segment_query.iter() {
            if parent.0 == agent_entity {
                let segment_mass = body.mass;
                let segment_speed = vel.0.length();
                let force_output = joint_force.map(|jf| jf.0).unwrap_or(0.0);

                let segment_cost = (k_mass * segment_mass)
                    + (k_velocity * segment_speed)
                    + (k_force * force_output);

                total_cost += segment_cost;
            }
        }

        let sweat_rate = if homeo.temperature > homeo.temp_target {
            0.5 * (homeo.temperature - homeo.temp_target)
        } else {
            0.0
        };

        let decay_hyd = (0.05 + 0.15 * total_cost + sweat_rate) * dt;
        homeo.hydration = (homeo.hydration - decay_hyd).max(0.0);

        let h_prod = 0.5 * total_cost;
        let h_diss = 0.1 * (homeo.temperature - homeo.temp_target);
        let h_evap = 0.2 * sweat_rate;
        let delta_temp = (h_prod - h_diss - h_evap) * dt;
        homeo.temperature = (homeo.temperature + delta_temp).clamp(30.0, 45.0);

        let decay = total_cost * dt;
        homeo.energy = (homeo.energy - decay).max(0.0);

        if let Some(mut tracker) = opt_tracker {
            let speed = velocity.map(|v| v.0.length()).unwrap_or(0.0);
            tracker.cumulative_energy_decay += decay;
            tracker.cumulative_distance += speed * dt;
            tracker.tick_count += 1;
        }
    }
}

pub fn spawn_food_system(
    mut commands: Commands,
    food_query: Query<&Food>,
    bounds: Res<MapBounds>,
    settings: Res<FoodSpawnSettings>,
    mut sim_rng: ResMut<crate::core::rng::SimRng>,
) {
    use rand::Rng;
    let current_food_count = food_query.iter().count();
    if current_food_count < settings.max_food_count {
        let to_spawn = settings.max_food_count - current_food_count;
        let rng = &mut sim_rng.0;
        for _ in 0..to_spawn {
            let x = rng.gen_range(bounds.min.x..bounds.max.x);
            let z = rng.gen_range(bounds.min.z..bounds.max.z);
            commands.spawn((
                Food {
                    energy_value: settings.default_energy,
                    hydration_value: settings.default_hydration,
                },
                Position(glam::Vec3::new(x, 0.0, z)),
                crate::physics::SpatialCollider { radius: 0.5 },
            ));
        }
    }
}

pub fn detect_food_collisions_system(
    mut commands: Commands,
    mut agent_query: Query<(Entity, &Position, &mut crate::ai::hrrl::HomeostaticState), With<Agent>>,
    segment_query: Query<(&Position, &ParentAgent)>,
    food_query: Query<(Entity, &Position, &Food)>,
) {
    for (agent_entity, agent_pos, mut homeo) in agent_query.iter_mut() {
        let mut sum_pos = glam::Vec3::ZERO;
        let mut count = 0;
        for (seg_pos, parent_agent) in segment_query.iter() {
            if parent_agent.0 == agent_entity {
                sum_pos += seg_pos.0;
                count += 1;
            }
        }
        let centroid = if count > 0 {
            sum_pos / count as f32
        } else {
            agent_pos.0
        };

        for (food_entity, food_pos, food) in food_query.iter() {
            if centroid.distance(food_pos.0) < 1.5 {
                commands.entity(food_entity).despawn();
                homeo.energy = (homeo.energy + food.energy_value).min(homeo.energy_target);
                homeo.hydration = (homeo.hydration + food.hydration_value).min(homeo.hydration_target);
                break;
            }
        }
    }
}

pub fn combat_system(
    mut predator_query: Query<(Entity, &Position, &mut crate::ai::hrrl::HomeostaticState), (With<Agent>, With<Predator>)>,
    mut prey_query: Query<(Entity, &Position, &mut crate::ai::hrrl::HomeostaticState), (With<Agent>, With<Prey>, Without<Predator>)>,
    segment_query: Query<(&Position, &ParentAgent)>,
    mut combat_events: Option<ResMut<CombatEvents>>,
) {
    if let Some(ref mut events_res) = combat_events {
        events_res.events.clear();
        events_res.predator_centroids.clear();
        events_res.prey_centroids.clear();

        // Populate lists with entity and default position
        for (entity, pos, _) in predator_query.iter() {
            events_res.predator_centroids.push((entity, pos.0, Vec3::ZERO, 0));
        }
        for (entity, pos, _) in prey_query.iter() {
            events_res.prey_centroids.push((entity, pos.0, Vec3::ZERO, 0));
        }

        // Single pass over segments to accumulate positions and counts for all parent agents
        for (seg_pos, parent_agent) in segment_query.iter() {
            if let Some(entry) = events_res.predator_centroids.iter_mut().find(|e| e.0 == parent_agent.0) {
                entry.2 += seg_pos.0;
                entry.3 += 1;
            } else if let Some(entry) = events_res.prey_centroids.iter_mut().find(|e| e.0 == parent_agent.0) {
                entry.2 += seg_pos.0;
                entry.3 += 1;
            }
        }

        // Finalize centroids
        for entry in events_res.predator_centroids.iter_mut() {
            if entry.3 > 0 {
                entry.1 = entry.2 / entry.3 as f32;
            }
        }
        for entry in events_res.prey_centroids.iter_mut() {
            if entry.3 > 0 {
                entry.1 = entry.2 / entry.3 as f32;
            }
        }

        // Perform disjoint mutable updates on HomeostaticState
        for i in 0..events_res.predator_centroids.len() {
            let (pred_entity, pred_centroid, _, _) = events_res.predator_centroids[i];
            for j in 0..events_res.prey_centroids.len() {
                let (prey_entity, prey_centroid, _, _) = events_res.prey_centroids[j];
                
                // Fast distance check
                if pred_centroid.distance(prey_centroid) < 1.5 {
                    // Get prey state first and check energy to avoid unnecessary predator query mut borrow
                    if let Ok((_, _, mut prey_homeo)) = prey_query.get_mut(prey_entity) {
                        if prey_homeo.energy <= 0.0 {
                            continue;
                        }
                        if let Ok((_, _, mut pred_homeo)) = predator_query.get_mut(pred_entity) {
                            let needed = (pred_homeo.energy_target - pred_homeo.energy).max(0.0);
                            if needed > 0.0 {
                                let transfer = needed.min(prey_homeo.energy);
                                prey_homeo.energy = (prey_homeo.energy - transfer).max(0.0);
                                pred_homeo.energy = (pred_homeo.energy + transfer).min(pred_homeo.energy_target);
                                if transfer > 0.0 && events_res.events.len() < events_res.events.capacity() {
                                    events_res.events.push(CombatEvent {
                                        predator_id: pred_entity.index(),
                                        prey_id: prey_entity.index(),
                                        damage: transfer,
                                        energy_transferred: transfer,
                                    });
                                }
                            }
                        }
                    }
                    break;
                }
            }
        }
    } else {
        // Fallback slow path (no resource provided, does not log events and does not allocate)
        for (pred_entity, pred_pos, mut pred_homeo) in predator_query.iter_mut() {
            let mut pred_sum = glam::Vec3::ZERO;
            let mut pred_count = 0;
            for (seg_pos, parent_agent) in segment_query.iter() {
                if parent_agent.0 == pred_entity {
                    pred_sum += seg_pos.0;
                    pred_count += 1;
                }
            }
            let pred_centroid = if pred_count > 0 {
                pred_sum / pred_count as f32
            } else {
                pred_pos.0
            };

            for (prey_entity, prey_pos, mut prey_homeo) in prey_query.iter_mut() {
                if prey_homeo.energy <= 0.0 {
                    continue;
                }
                let mut prey_sum = glam::Vec3::ZERO;
                let mut prey_count = 0;
                for (seg_pos, parent_agent) in segment_query.iter() {
                    if parent_agent.0 == prey_entity {
                        prey_sum += seg_pos.0;
                        prey_count += 1;
                    }
                }
                let prey_centroid = if prey_count > 0 {
                    prey_sum / prey_count as f32
                } else {
                    prey_pos.0
                };

                if pred_centroid.distance(prey_centroid) < 1.5 {
                    let needed = (pred_homeo.energy_target - pred_homeo.energy).max(0.0);
                    if needed > 0.0 {
                        let transfer = needed.min(prey_homeo.energy);
                        prey_homeo.energy = (prey_homeo.energy - transfer).max(0.0);
                        pred_homeo.energy = (pred_homeo.energy + transfer).min(pred_homeo.energy_target);
                    }
                    break;
                }
            }
        }
    }
}

#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentParentLineageIds(pub Vec<String>);

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct ShardingConfig {
    pub local_port: u16,
    pub left_target_port: Option<u16>,
    pub right_target_port: Option<u16>,
}

#[derive(Resource, Clone)]
pub struct ShardingResource(pub std::sync::Arc<std::sync::RwLock<ShardingConfig>>);

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

#[derive(Resource)]
pub struct InboundMigrationReceiver(pub crossbeam_channel::Receiver<AgentMigrationData>);

#[derive(Resource)]
pub struct OutboundMigrationSender(pub crossbeam_channel::Sender<OutboundMigration>);

#[derive(Resource)]
pub struct BevyMigrationTrigger(pub crossbeam_channel::Receiver<u16>);

pub fn check_migration_boundaries_system(
    mut commands: Commands,
    agent_query: Query<(
        Entity,
        &Position,
        &Velocity,
        &crate::ai::hrrl::HomeostaticState,
        &crate::core::engine::AgentGenotype,
        &crate::core::engine::AgentLineageId,
        &crate::core::engine::AgentGeneration,
        Option<&AgentParentLineageIds>,
        Option<&Predator>,
        Option<&crate::core::engine::AgentEvaluation>,
        Option<&FeatureTracker>,
        Option<&crate::ai::hrrl::LastTransitionState>,
    ), With<Agent>>,
    children_query: Query<&ChildrenLinks>,
    bounds: Res<MapBounds>,
    sharding: Res<ShardingResource>,
    outbound_sender: Option<Res<OutboundMigrationSender>>,
) {
    let sender = match outbound_sender {
        Some(s) => s,
        None => return,
    };

    let config = match sharding.0.read() {
        Ok(c) => c,
        Err(_) => return,
    };

    let x_min = bounds.min.x;
    let x_max = bounds.max.x;
    let x_range = x_max - x_min;

    for (entity, pos, vel, homeo, genotype, lineage_id, generation, opt_parents, opt_predator, opt_eval, opt_tracker, opt_last_transition) in agent_query.iter() {
        let x = pos.0.x;
        let mut target_port = None;
        let mut target_x = pos.0.x;

        if x < x_min {
            if let Some(port) = config.left_target_port {
                target_port = Some(port);
                let wrapped = x_min + (x - x_min).rem_euclid(x_range);
                target_x = wrapped.clamp(x_min + 0.01, x_max - 0.01);
            }
        } else if x > x_max {
            if let Some(port) = config.right_target_port {
                target_port = Some(port);
                let wrapped = x_min + (x - x_min).rem_euclid(x_range);
                target_x = wrapped.clamp(x_min + 0.01, x_max - 0.01);
            }
        }

        if let Some(port) = target_port {
            let agent_class = if opt_predator.is_some() {
                AgentClass::Predator
            } else {
                AgentClass::Prey
            };

            let parent_ids = opt_parents.map(|p| p.0.clone()).unwrap_or_default();

            let migration_data = AgentMigrationData {
                genotype: genotype.0.clone(),
                homeostatic_state: homeo.clone(),
                position: glam::Vec3::new(target_x, pos.0.y, pos.0.z),
                velocity: vel.0,
                lineage_id: lineage_id.0.clone(),
                generation: generation.0,
                agent_class,
                parent_ids,
                evaluation: opt_eval.cloned(),
                feature_tracker: opt_tracker.cloned(),
                last_transition_state: opt_last_transition.cloned(),
                source_port: config.local_port,
            };

            let _ = sender.0.send(OutboundMigration {
                target_port: port,
                data: migration_data,
                bounds_min_x: x_min,
                bounds_max_x: x_max,
            });

            // Recursively despawn the root agent and all child segments using ChildrenLinks in O(K)
            let mut stack = [entity; 64];
            let mut stack_len = 1;
            while stack_len > 0 {
                stack_len -= 1;
                let current = stack[stack_len];
                commands.entity(current).despawn();
                if let Ok(children) = children_query.get(current) {
                    for &child in &children.0 {
                        if stack_len < 64 {
                            stack[stack_len] = child;
                            stack_len += 1;
                        }
                    }
                }
            }
        }
    }
}

pub fn manual_migration_system(
    mut commands: Commands,
    trigger: Option<Res<BevyMigrationTrigger>>,
    agent_query: Query<(
        Entity,
        &Position,
        &Velocity,
        &crate::ai::hrrl::HomeostaticState,
        &crate::core::engine::AgentGenotype,
        &crate::core::engine::AgentLineageId,
        &crate::core::engine::AgentGeneration,
        Option<&AgentParentLineageIds>,
        Option<&Predator>,
        Option<&crate::core::engine::AgentEvaluation>,
        Option<&FeatureTracker>,
        Option<&crate::ai::hrrl::LastTransitionState>,
    ), With<Agent>>,
    children_query: Query<&ChildrenLinks>,
    bounds: Res<MapBounds>,
    sharding: Res<ShardingResource>,
    outbound_sender: Option<Res<OutboundMigrationSender>>,
    mut sim_rng: ResMut<crate::core::rng::SimRng>,
) {
    let trigger = match trigger {
        Some(t) => t,
        None => return,
    };
    let sender = match outbound_sender {
        Some(s) => s,
        None => return,
    };
    let config = match sharding.0.read() {
        Ok(c) => c,
        Err(_) => return,
    };

    while let Ok(target_port) = trigger.0.try_recv() {
        use rand::seq::IteratorRandom;
        let rng = &mut sim_rng.0;
        if let Some((entity, pos, vel, homeo, genotype, lineage_id, generation, opt_parents, opt_predator, opt_eval, opt_tracker, opt_last_transition)) = agent_query.iter().choose(rng) {
            let x_min = bounds.min.x;
            let x_max = bounds.max.x;

            let agent_class = if opt_predator.is_some() {
                AgentClass::Predator
            } else {
                AgentClass::Prey
            };

            let parent_ids = opt_parents.map(|p| p.0.clone()).unwrap_or_default();

            let migration_data = AgentMigrationData {
                genotype: genotype.0.clone(),
                homeostatic_state: homeo.clone(),
                position: pos.0,
                velocity: vel.0,
                lineage_id: lineage_id.0.clone(),
                generation: generation.0,
                agent_class,
                parent_ids,
                evaluation: opt_eval.cloned(),
                feature_tracker: opt_tracker.cloned(),
                last_transition_state: opt_last_transition.cloned(),
                source_port: config.local_port,
            };

            let _ = sender.0.send(OutboundMigration {
                target_port,
                data: migration_data,
                bounds_min_x: x_min,
                bounds_max_x: x_max,
            });

            let mut stack = [entity; 64];
            let mut stack_len = 1;
            while stack_len > 0 {
                stack_len -= 1;
                let current = stack[stack_len];
                commands.entity(current).despawn();
                if let Ok(children) = children_query.get(current) {
                    for &child in &children.0 {
                        if stack_len < 64 {
                            stack[stack_len] = child;
                            stack_len += 1;
                        }
                    }
                }
            }
        }
    }
}


pub struct SpawnMigrationCommand {
    pub data: AgentMigrationData,
}

impl bevy_ecs::system::Command for SpawnMigrationCommand {
    fn apply(self, world: &mut World) {
        use crate::physics::dynamics::RigidBody;
        use crate::core::ecs::{Velocity, ChildrenLinks};
        use crate::evolution::genotype::decode_genotype;
        use crate::core::engine::{AgentGenotype, AgentEvaluation, AgentLineageId, AgentGeneration};

        let initial_pos = self.data.position;
        let initial_rot = glam::Quat::IDENTITY;

        // 1. Spawn morphology via decode_genotype
        let root_entity = decode_genotype(world, &self.data.genotype, initial_pos, initial_rot);

        // 2. Insert components to complete agent identity
        let eval = self.data.evaluation.unwrap_or(AgentEvaluation {
            start_position: initial_pos,
            total_distance: 0.0,
            total_energy_expended: 0.0,
            survival_ticks: 0,
            last_position: initial_pos,
        });

        let tracker = self.data.feature_tracker.unwrap_or_default();

        world.entity_mut(root_entity).insert((
            AgentGenotype(self.data.genotype.clone()),
            eval,
            tracker,
            AgentLineageId(self.data.lineage_id.clone()),
            AgentGeneration(self.data.generation),
            AgentParentLineageIds(self.data.parent_ids.clone()),
        ));

        if let Some(lts) = self.data.last_transition_state {
            world.entity_mut(root_entity).insert(lts);
        }

        match self.data.agent_class {
            AgentClass::Predator => {
                world.entity_mut(root_entity).insert(Predator);
            }
            AgentClass::Prey => {
                world.entity_mut(root_entity).insert(Prey);
            }
        }

        // Overwrite homeostatic state with the received one
        if let Some(mut homeo) = world.get_mut::<crate::ai::hrrl::HomeostaticState>(root_entity) {
            *homeo = self.data.homeostatic_state;
        }

        // Set velocities to conserve momentum across all segments recursively in O(K)
        let velocity = self.data.velocity;
        let mut stack = [root_entity; 64];
        let mut stack_len = 1;
        while stack_len > 0 {
            stack_len -= 1;
            let current = stack[stack_len];
            
            if let Some(mut vel) = world.get_mut::<Velocity>(current) {
                vel.0 = velocity;
            }
            if let Some(mut body) = world.get_mut::<RigidBody>(current) {
                body.velocity = velocity;
            }
            
            if let Some(children) = world.get::<ChildrenLinks>(current) {
                for &child in &children.0 {
                    if stack_len < 64 {
                        stack[stack_len] = child;
                        stack_len += 1;
                    }
                }
            }
        }
    }
}

pub fn process_inbound_migrations_system(
    mut commands: Commands,
    inbound_receiver: Option<Res<InboundMigrationReceiver>>,
) {
    let receiver = match inbound_receiver {
        Some(r) => r,
        None => return,
    };

    while let Ok(data) = receiver.0.try_recv() {
        commands.add(SpawnMigrationCommand { data });
    }
}

pub fn herbivore_grazing_system(
    mut prey_query: Query<(Entity, &mut crate::ai::hrrl::HomeostaticState, &Position), (With<Prey>, With<Agent>)>,
    segment_query: Query<(&Position, &ParentAgent)>,
    mut flora_query: Query<(&mut Flora, &Position)>,
    time_step: Res<crate::ai::cpg::TimeStep>,
    bounds: Option<Res<MapBounds>>,
) {
    let dt = time_step.0;
    for (prey_entity, mut homeo, prey_pos) in prey_query.iter_mut() {
        if homeo.energy >= homeo.energy_target {
            continue;
        }
        let mut sum_pos = glam::Vec3::ZERO;
        let mut count = 0;
        for (seg_pos, parent_agent) in segment_query.iter() {
            if parent_agent.0 == prey_entity {
                sum_pos += seg_pos.0;
                count += 1;
            }
        }
        let centroid = if count > 0 {
            sum_pos / count as f32
        } else {
            prey_pos.0
        };

        let max_grazable = 15.0 * dt;
        for (mut flora, flora_pos) in flora_query.iter_mut() {
            let dist_sq = if let Some(bounds) = bounds.as_ref() {
                toroidal_distance_sq(centroid, flora_pos.0, bounds)
            } else {
                centroid.distance_squared(flora_pos.0)
            };
            if flora.energy > 0.0 && dist_sq < 2.25 {
                let energy_needed = (homeo.energy_target - homeo.energy).max(0.0);
                let actual_grazed = max_grazable.min(flora.energy).min(energy_needed);
                if actual_grazed > 0.0 {
                    flora.energy -= actual_grazed;
                    homeo.energy += actual_grazed;
                    homeo.hydration = (homeo.hydration + actual_grazed * 0.4).min(homeo.hydration_target);
                    break;
                }
            }
        }
    }
}



