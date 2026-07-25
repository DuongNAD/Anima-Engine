use bevy_ecs::prelude::*;

use crate::core::agent_systems::{AgentEvaluation, AgentGeneration, AgentGenotype, AgentLineageId};
use crate::core::ecs::*;
use crate::evolution::lineage::LineageTracker;
use std::sync::{Arc, RwLock};

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

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
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

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, Clone, Debug)]
pub struct SimulationTickPayload {
    pub segments: Vec<SegmentState>,
    pub environmental_state: crate::core::ecs::EnvironmentalState,
    pub head_directions: std::collections::HashMap<u32, [f32; 3]>,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug)]
pub struct SimulationStatus {
    pub running: bool,
    /// Crosses the Tauri bridge as a JSON number, not a bigint: serde_json emits `u64` as a bare
    /// number and JS parses it into a `number`. ts-rs would map `u64` to `bigint`, which is what
    /// the type would be if this were a `BigInt`-aware transport — it is not. Precision is
    /// therefore bounded by 2^53, which a tick counter will not reach in any real run.
    #[ts(type = "number")]
    pub tick_count: u64,
    pub avg_tick_time_ms: f64,
    pub fps: f64,
}

#[derive(ts_rs::TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ChronicleEvent {
    pub id: String,
    pub event_type: String, // "Drought" | "TemperatureSpike" | "PredatorWave" | "Abundance"
    /// Milliseconds since the unix epoch, as a JSON number — see `SimulationStatus::tick_count`.
    #[ts(type = "number")]
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
    /// The agent's own brain, when it has one. `None` is a legacy agent running on the shared
    /// [`crate::ai::model::BrainModel`] — and is what every save written before ADR-0003 decodes to,
    /// so old saves keep their old behaviour (invariant D09).
    #[serde(default)]
    pub brain: Option<crate::core::components::AgentBrain>,
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
    /// Identity of the world this save belongs to (M1.5 / S08). `#[serde(default)]` keeps pre-M1
    /// saves (which lack the field) loadable — they deserialize to the all-zero default.
    #[serde(default)]
    pub world_identity: crate::core::world_artifact::WorldIdentity,
    /// The closed-energy compartments (G1.1). Without these a load rebuilt `EcosystemBiomass` from
    /// scratch — detritus back to zero, plants back to a full resource field — which created or
    /// destroyed EU at every save/load boundary and made whole-run conservation unprovable.
    ///
    /// Stored as three plain scalars rather than the `EcosystemBiomass` struct because that type
    /// lives in `core::ecology`, which is outside G1's allowed files and does not derive `Serialize`.
    /// G1.2 replaces this with a proper versioned snapshot envelope.
    #[serde(default)]
    pub eco_detritus: f64,
    #[serde(default)]
    pub eco_plants: f64,
    #[serde(default)]
    pub eco_animals: f64,
    /// Standing plant resource per cell. `r_max` is not stored: it is derived from the world's
    /// biomes, which the world identity already pins. Empty on a pre-G1.1 save, in which case the
    /// field keeps whatever `init_world` generated.
    #[serde(default)]
    pub resource_field_r: Vec<f32>,

    // ---- G1.2: the rest of what makes this a checkpoint rather than a picture ----------------
    /// Seed of the run's RNG stream.
    #[serde(default)]
    pub sim_rng_seed: u64,
    /// **Draw position** of that stream. This is the field that separates a checkpoint from a save:
    /// restoring the seed alone restarts the sequence, so a resumed run diverges from an
    /// uninterrupted one on its very next random draw. Zero on a pre-G1.2 save, which reads as
    /// "start of stream" — the old behaviour.
    #[serde(default)]
    pub sim_rng_pos: u128,
    /// Season clock, which scales plant regrowth. Without it a reload lands in a different season
    /// and the ecology diverges immediately.
    #[serde(default)]
    pub season_phase: f32,
    #[serde(default)]
    pub season_rate: f32,
    /// The closed-EU baseline the energy ledger locked after genesis (G1.1). Carried so a resumed
    /// run keeps measuring conservation against the *original* genesis, instead of re-baselining on
    /// load and thereby forgiving any drift that happened before the save.
    #[serde(default)]
    pub energy_baseline: Option<f64>,

    /// Which on-disk schema this state was read from, filled in by
    /// [`crate::core::snapshot::read`]. Runtime-only: never written, so it cannot disagree with the
    /// envelope that carries the real version.
    #[serde(skip)]
    pub loaded_from_schema: u32,
}

/// A `SavedSimulationState` with every field at its zero value.
///
/// Exists because several test suites (and the snapshot module's own tests) need a state to mutate
/// a field or two of, and hand-writing all twenty-odd fields at each site meant every new field
/// broke five test files at once.
pub fn empty_saved_state_for_tests() -> SavedSimulationState {
    SavedSimulationState {
        tick_count: 0,
        active_environment_event: crate::evolution::meta_ai::EnvironmentalEvent::Stable,
        food_spawn_settings: crate::core::ecs::FoodSpawnSettings::default(),
        map_bounds: crate::core::ecs::MapBounds {
            min: glam::Vec3::new(-100.0, 0.0, -100.0),
            max: glam::Vec3::new(100.0, 10.0, 100.0),
        },
        epoch_manager: crate::core::ecs::EpochManager::default(),
        pheromone_grid: SerializedPheromoneGrid {
            values: vec![0.0; 16384],
            diffusion_rate: 0.12,
            decay_rate: 0.04,
        },
        foods: Vec::new(),
        agents: Vec::new(),
        evolution_settings: crate::commands::EvolutionSettings {
            mutation_rate: 0.2,
            selection_bias: 1.2,
            grid_resolution: 30,
        },
        map_elites_grid: crate::commands::MapElitesGridState {
            grid: std::collections::HashMap::new(),
            grid_resolution: 30,
        },
        chronicle_history: Vec::new(),
        lineage_nodes: Vec::new(),
        lineage_relations: Vec::new(),
        lakes: Vec::new(),
        trees: Vec::new(),
        world_identity: Default::default(),
        eco_detritus: 0.0,
        eco_plants: 0.0,
        eco_animals: 0.0,
        resource_field_r: Vec::new(),
        sim_rng_seed: 0,
        sim_rng_pos: 0,
        season_phase: 0.0,
        season_rate: 0.0,
        energy_baseline: None,
        loaded_from_schema: 0,
    }
}

/// Put the saved closed-energy state back into the world (G1.1).
///
/// Call this on the restore path *after* agents have been respawned, so the `animals` compartment
/// the save recorded is not immediately contradicted. Invariant D06 says restore transfers the same
/// reserve and adds no EU; that is only true if the pool and the standing crop come back too.
///
/// A pre-G1.1 save carries zeroes and an empty field, which this treats as "nothing to restore" and
/// leaves whatever `init_world` built — the old behaviour, so old saves stay loadable (D09).
pub fn restore_energy_state(world: &mut World, state: &SavedSimulationState) {
    if !state.resource_field_r.is_empty() {
        if let Some(mut field) = world.get_resource_mut::<crate::core::ecology::ResourceField>() {
            // Only adopt a field of the same shape. A mismatch means the save belongs to a
            // differently sized world, which the S08 identity check already warns about; silently
            // resizing here would turn that warning into corrupted energy accounting.
            if field.r.len() == state.resource_field_r.len() {
                field.r.copy_from_slice(&state.resource_field_r);
            } else {
                eprintln!(
                    "saved resource field has {} cells but this world has {}; \
                     keeping the generated field and its energy",
                    state.resource_field_r.len(),
                    field.r.len()
                );
            }
        }
    }
    let has_pool = state.eco_detritus != 0.0 || state.eco_plants != 0.0 || state.eco_animals != 0.0;
    if has_pool {
        if let Some(mut pool) = world.get_resource_mut::<crate::core::ecology::EcosystemBiomass>() {
            pool.detritus = state.eco_detritus;
            pool.plants = state.eco_plants;
            pool.animals = state.eco_animals;
        }
    }

    // G1.2. A save that predates these carries zeroes, which read as "leave what init_world built" —
    // i.e. the old behaviour, so old saves stay loadable (D09).
    if state.sim_rng_seed != 0 || state.sim_rng_pos != 0 {
        world.insert_resource(crate::core::resources::SimRng::restore(
            state.sim_rng_seed,
            state.sim_rng_pos,
        ));
    }
    if state.season_rate != 0.0 {
        if let Some(mut clock) = world.get_resource_mut::<crate::core::ecology::SeasonClock>() {
            clock.phase = state.season_phase;
            clock.rate = state.season_rate;
        }
    }
    if let Some(baseline) = state.energy_baseline {
        if let Some(mut ledger) =
            world.get_resource_mut::<crate::core::energy_ledger::EnergyLedger>()
        {
            // `lock_baseline` ignores repeat calls, so this only takes effect on a ledger that has
            // not locked yet — which is exactly the freshly built world a restore lands in.
            ledger.lock_baseline(baseline);
        }
    }
}

pub fn spawn_serialized_agent(world: &mut World, agent: &SerializedAgent) {
    use crate::ai::cpg::CpgOscillator;
    use crate::core::ecs::{
        AgentClass, AgentParentLineageIds, ParentAgent, Position, Predator, Prey, Rotation,
        Segment, Velocity,
    };
    use crate::evolution::genotype::decode_genotype;
    use crate::physics::dynamics::RigidBody;

    let root_entity = decode_genotype(
        world,
        &agent.genotype,
        agent.root_position,
        agent.root_rotation,
    );

    world.entity_mut(root_entity).insert((
        AgentGenotype(agent.genotype.clone()),
        agent.evaluation.clone(),
        agent.feature_tracker,
        AgentLineageId(agent.lineage_id.clone()),
        AgentGeneration(agent.generation),
        AgentParentLineageIds(agent.parent_ids.clone()),
    ));
    world
        .entity_mut(root_entity)
        .insert(agent.last_transition_state);

    // Restore the saved brain verbatim. Invariant D01: restore is not development, so a saved
    // individual must never be handed a freshly rolled brain — that would be a different creature
    // wearing the same lineage id. A `None` here is a legacy agent and correctly stays on the
    // shared model rather than being upgraded behind the user's back.
    if let Some(brain) = &agent.brain {
        match brain.validate() {
            Ok(()) => {
                world.entity_mut(root_entity).insert(brain.clone());
            }
            Err(e) => {
                eprintln!(
                    "agent {} has an unreadable brain ({e}); restoring it on the shared model",
                    agent.lineage_id
                );
            }
        }
    }

    match agent.class {
        AgentClass::Predator => {
            world.entity_mut(root_entity).insert(Predator);
        }
        AgentClass::Prey => {
            world.entity_mut(root_entity).insert(Prey);
        }
    }

    if let Some(mut homeo) = world.get_mut::<crate::ai::hrrl::HomeostaticState>(root_entity) {
        *homeo = agent.homeostatic_state.clone();
    }

    if let Some(mut pos) = world.get_mut::<Position>(root_entity) {
        pos.0 = agent.root_position;
    }
    if let Some(mut rot) = world.get_mut::<Rotation>(root_entity) {
        rot.0 = agent.root_rotation;
    }
    if let Some(mut vel) = world.get_mut::<Velocity>(root_entity) {
        vel.0 = agent.root_velocity;
    }
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
            if let Some(mut pos) = world.get_mut::<Position>(entity) {
                pos.0 = seg_state.position;
            }
            if let Some(mut rot) = world.get_mut::<Rotation>(entity) {
                rot.0 = seg_state.rotation;
            }
            if let Some(mut vel) = world.get_mut::<Velocity>(entity) {
                vel.0 = seg_state.velocity;
            }
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
    use crate::ai::cpg::CpgOscillator;
    use crate::core::ecs::{
        ActiveEnvironmentEvent, AgentClass, AgentParentLineageIds, EpochManager, Food,
        FoodSpawnSettings, Lake, MapBounds, ParentAgent, Predator, Segment, Tree, Velocity,
    };
    let active_environment_event = world
        .get_resource::<ActiveEnvironmentEvent>()
        .map(|e| e.0)
        .unwrap_or(crate::evolution::meta_ai::EnvironmentalEvent::Stable);
    let food_spawn_settings = world
        .get_resource::<FoodSpawnSettings>()
        .cloned()
        .unwrap_or_default();
    let map_bounds = world
        .get_resource::<MapBounds>()
        .cloned()
        .unwrap_or_default();
    let epoch_manager = world
        .get_resource::<EpochManager>()
        .cloned()
        .unwrap_or_default();
    // Closed-energy state (G1.1). A save that omits these is not a checkpoint of the energy system:
    // reloading it would rebuild detritus at zero and plants at full capacity, silently moving EU.
    let (eco_detritus, eco_plants, eco_animals) = world
        .get_resource::<crate::core::ecology::EcosystemBiomass>()
        .map(|p| (p.detritus, p.plants, p.animals))
        .unwrap_or((0.0, 0.0, 0.0));
    let resource_field_r = world
        .get_resource::<crate::core::ecology::ResourceField>()
        .map(|f| f.r.clone())
        .unwrap_or_default();
    // G1.2: the rest of the trajectory-relevant state. The RNG's *draw position* is the field that
    // makes this a checkpoint — with only the seed, a resumed run restarts the stream and diverges
    // on its next draw.
    let (sim_rng_seed, sim_rng_pos) = world
        .get_resource::<crate::core::resources::SimRng>()
        .map(|r| (r.seed(), r.stream_pos()))
        .unwrap_or((0, 0));
    let (season_phase, season_rate) = world
        .get_resource::<crate::core::ecology::SeasonClock>()
        .map(|c| (c.phase, c.rate))
        .unwrap_or((0.0, 0.0));
    // Carried so a resumed run measures conservation against the ORIGINAL genesis rather than
    // re-baselining on load, which would forgive any drift that happened before the save.
    let energy_baseline = world
        .get_resource::<crate::core::energy_ledger::EnergyLedger>()
        .and_then(|l| l.baseline());
    // World identity so the save is pinned to the world it belongs to (S08); default if a world was
    // built before this resource existed.
    let world_identity = world
        .get_resource::<crate::core::world_artifact::WorldIdentity>()
        .copied()
        .unwrap_or_default();

    let pheromone_grid =
        if let Some(grid) = world.get_resource::<crate::ai::pheromone::PheromoneGrid>() {
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
        Option<&crate::core::components::AgentBrain>,
    )>();

    let mut collected_agents = Vec::new();
    for (
        entity,
        pos,
        rot,
        vel,
        homeo,
        last_trans,
        genotype,
        eval,
        tracker,
        lineage_id,
        gen,
        parents,
        predator,
        brain,
    ) in agent_query.iter(world)
    {
        collected_agents.push((
            entity,
            pos.0,
            rot.0,
            vel.0,
            homeo.clone(),
            *last_trans,
            genotype.0.clone(),
            eval.clone(),
            *tracker,
            lineage_id.0.clone(),
            gen.0,
            parents.0.clone(),
            predator.is_some(),
            brain.cloned(),
        ));
    }

    let mut segment_query = world.query::<(
        Entity,
        &Segment,
        &Position,
        &Rotation,
        &Velocity,
        &ParentAgent,
        Option<&CpgOscillator>,
    )>();
    for (
        entity,
        root_pos,
        root_rot,
        root_vel,
        homeo,
        last_trans,
        genotype,
        eval,
        tracker,
        lineage_id,
        gen,
        parents,
        is_predator,
        brain,
    ) in collected_agents
    {
        let class = if is_predator {
            AgentClass::Predator
        } else {
            AgentClass::Prey
        };
        let mut segments = Vec::new();

        for (seg_entity, segment, seg_pos, seg_rot, seg_vel, parent_agent, opt_osc) in
            segment_query.iter(world)
        {
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
            brain,
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
        evolution_settings: evolution_settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone(),
        map_elites_grid: map_elites_grid
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone(),
        chronicle_history: chronicle_history
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone(),
        lineage_nodes: lineage_tracker.get_lineage_graph().unwrap_or_default().0,
        lineage_relations: lineage_tracker.get_lineage_graph().unwrap_or_default().1,
        lakes,
        trees,
        world_identity,
        eco_detritus,
        eco_plants,
        eco_animals,
        resource_field_r,
        sim_rng_seed,
        sim_rng_pos,
        season_phase,
        season_rate,
        energy_baseline,
        loaded_from_schema: crate::core::snapshot::SCHEMA_VERSION,
    }
}
