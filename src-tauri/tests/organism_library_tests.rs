mod common;

use bevy_ecs::prelude::*;
use std::sync::Arc;

use anima_engine_lib::core::ecs::{
    init_world, MapBounds, ParentAgent, metabolic_decay_system,
};
use anima_engine_lib::ai::cpg::{CpgOscillator, TimeStep, update_cpg_system};
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::physics::dynamics::{resolve_joints_system, integrate_physics_system};
use anima_engine_lib::core::agent_systems::{
    get_default_presets, build_preset_genotype, process_preset_spawns_system,
    BevyPresetSpawnTrigger, BevyLineageTracker,
    AgentGenotype, AgentLineageId,
};
use anima_engine_lib::evolution::lineage::FallbackLineageTracker;

#[test]
fn test_preset_metadata_and_genotype_building() {
    let presets = get_default_presets();
    assert_eq!(presets.len(), 5, "Should have exactly 5 default presets");

    for preset in &presets {
        assert!(!preset.name.is_empty(), "Preset name should not be empty");
        assert!(!preset.description.is_empty(), "Preset description should not be empty");
        assert!(preset.segment_count >= 2, "Segment count must be at least 2");
        assert!(preset.initial_energy > 0.0, "Initial energy must be positive");
        assert!(preset.mass > 0.0, "Mass must be positive");
        assert!(preset.length > 0.0, "Length must be positive");
        assert!(preset.radius > 0.0, "Radius must be positive");

        let genotype = build_preset_genotype(preset);
        assert_eq!(genotype.nodes.len(), preset.segment_count as usize);
        assert_eq!(genotype.edges.len(), (preset.segment_count - 1) as usize);

        for (i, node) in genotype.nodes.iter().enumerate() {
            assert_eq!(node.id, i as u32);
            assert_eq!(node.length, preset.length);
            assert_eq!(node.radius, preset.radius);
            assert_eq!(node.mass, preset.mass);
        }
    }
}

#[test]
fn test_process_preset_spawns_happy_path() {
    let presets = get_default_presets();

    for preset in &presets {
        let mut world = init_world();
        world.insert_resource(TimeStep(1.0 / 60.0));
        world.insert_resource(MapBounds::default());

        let (tx, rx) = crossbeam_channel::unbounded();
        world.insert_resource(BevyPresetSpawnTrigger(rx));

        let tracker = Arc::new(FallbackLineageTracker::new("bolt://localhost:9999", "neo4j", "password"));
        world.insert_resource(BevyLineageTracker(tracker));

        // Queue the spawn
        tx.send(preset.name.clone()).unwrap();

        // Run the preset spawning system
        process_preset_spawns_system(&mut world);

        // Find the spawned root entity
        let mut query = world.query::<(Entity, &AgentGenotype, &AgentLineageId, &HomeostaticState)>();
        let spawned_entities: Vec<_> = query.iter(&world).collect();

        assert_eq!(spawned_entities.len(), 1, "Should spawn exactly one root entity for preset {}", preset.name);
        let (entity, agent_genotype, _lineage_id, homeo) = spawned_entities[0];
        assert_eq!(agent_genotype.0.nodes.len(), preset.segment_count as usize);
        assert_eq!(homeo.energy, preset.initial_energy);
                
        // Check that child segments have CPG set with correct frequency/amplitude
        let mut cpg_query = world.query::<(Entity, &ParentAgent, &CpgOscillator)>();
        let mut child_count = 0;
        for (_c_entity, parent_agent, oscillator) in cpg_query.iter(&world) {
            if parent_agent.0 == entity {
                assert_eq!(oscillator.frequency, preset.frequency);
                assert_eq!(oscillator.amplitude, preset.amplitude);
                child_count += 1;
            }
        }
        // Root node doesn't have a CpgOscillator, only child segments do, so count should be segment_count - 1
        assert_eq!(child_count, preset.segment_count - 1);

        // Now run the Bevy schedule to ensure ticking is stable
        let mut schedule = Schedule::default();
        schedule.add_systems((
            update_cpg_system,
            resolve_joints_system.after(update_cpg_system),
            integrate_physics_system.after(resolve_joints_system),
            metabolic_decay_system.after(integrate_physics_system),
        ));

        // Run tick multiple times
        for _ in 0..10 {
            schedule.run(&mut world);
        }
    }
}

#[test]
fn test_process_preset_spawns_invalid_name() {
    let mut world = init_world();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(MapBounds::default());

    let (tx, rx) = crossbeam_channel::unbounded();
    world.insert_resource(BevyPresetSpawnTrigger(rx));

    let tracker = Arc::new(FallbackLineageTracker::new("bolt://localhost:9999", "neo4j", "password"));
    world.insert_resource(BevyLineageTracker(tracker));

    // Count entities before spawning
    let initial_entities_count = world.entities().len();

    // Queue invalid spawns
    tx.send("Unknown Organism".to_string()).unwrap();
    tx.send("".to_string()).unwrap();

    // Run preset spawn system
    process_preset_spawns_system(&mut world);

    // Verify entity count remains the same
    assert_eq!(world.entities().len(), initial_entities_count);
}

#[test]
fn test_preset_spawns_stress_spawning() {
    let mut world = init_world();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(MapBounds::default());

    let (tx, rx) = crossbeam_channel::unbounded();
    world.insert_resource(BevyPresetSpawnTrigger(rx));

    let tracker = Arc::new(FallbackLineageTracker::new("bolt://localhost:9999", "neo4j", "password"));
    world.insert_resource(BevyLineageTracker(tracker));

    let presets = get_default_presets();
    let num_spawns_per_preset = 20;

    // Send 100 valid spawn requests and some invalid ones
    for _ in 0..num_spawns_per_preset {
        for preset in &presets {
            tx.send(preset.name.clone()).unwrap();
        }
        tx.send("Invalid Preset Name".to_string()).unwrap();
    }

    // Process all of them in a single system execution
    process_preset_spawns_system(&mut world);

    // Verify that we successfully spawned all of them
    // Each preset spawns 1 root agent entity + child segment entities
    let mut query = world.query::<(Entity, &AgentGenotype)>();
    let spawned_root_count = query.iter(&world).count();
    
    // We expect 5 presets * 20 spawns = 100 root agents
    assert!(spawned_root_count >= 100);

    // Run the Bevy schedule to ensure stable ticking under heavy load of 100+ entities
    let mut schedule = Schedule::default();
    schedule.add_systems((
        update_cpg_system,
        resolve_joints_system.after(update_cpg_system),
        integrate_physics_system.after(resolve_joints_system),
        metabolic_decay_system.after(integrate_physics_system),
    ));

    for _ in 0..50 {
        schedule.run(&mut world);
    }
}

#[test]
fn test_preset_spawns_full_simulation_stability() {
    use std::sync::atomic::AtomicBool;
    use anima_engine_lib::ai::model::{BrainModel, BrainInferenceBuffer, brain_inference_system, hrrl_learning_system};
    use anima_engine_lib::core::ecs::{
        FoodSpawnSettings, OutboundMigrationSender, InboundMigrationReceiver, ShardingResource,
        ShardingConfig, BevyMigrationTrigger, Position, Rotation, Velocity,
        apply_environmental_effects_system, spawn_food_system,
        detect_food_collisions_system, combat_system,
        wrap_coordinates_system,
    };
    use anima_engine_lib::physics::rebuild_spatial_grid_system;
    use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState, EnvironmentSettings};
    use anima_engine_lib::core::simulation_lifecycle::{
        BevyEvolutionSettings, BevyEvolutionRunning, BevyMapElitesGrid, BevyAppHandle,
        ActiveEvolutionSettings, BevyMapElitesArchive, NextNodeId, EnvironmentalEventReceiver,
        EvolutionSender, EvolutionReceiver, EvolutionQueue,
    };
    use anima_engine_lib::core::agent_systems::{
        sync_evolution_settings_system, sync_environment_settings_system,
        receive_manual_disasters_system, receive_environmental_events_system,
        update_agent_evaluation_system, check_epoch_completion_system,
        apply_staggered_evolution_system, BevyEnvironmentSettings, BevyManualDisaster,
    };

    let mut world = init_world();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(MapBounds::default());
    world.insert_resource(anima_engine_lib::ai::pheromone::PheromoneGrid::default());
    world.insert_resource(BrainModel::new(15, 64, 4));
    world.insert_resource(BrainInferenceBuffer::default());
    world.insert_resource(FoodSpawnSettings::default());

    let (trans_tx, _trans_rx) = crossbeam_channel::bounded::<anima_engine_lib::ai::hrrl::Transition>(4096);
    world.insert_resource(anima_engine_lib::ai::hrrl::TransitionSender(trans_tx));

    let evolution_settings = Arc::new(std::sync::Mutex::new(EvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    }));
    let evolution_running = Arc::new(AtomicBool::new(false));
    let map_elites_grid = Arc::new(std::sync::Mutex::new(MapElitesGridState {
        grid: std::collections::HashMap::new(),
        grid_resolution: 50,
    }));

    world.insert_resource(BevyEvolutionSettings(evolution_settings));
    world.insert_resource(BevyEvolutionRunning(evolution_running));
    world.insert_resource(BevyMapElitesGrid(map_elites_grid));
    world.insert_resource(BevyAppHandle::<tauri::test::MockRuntime>(None));
    world.insert_resource(ActiveEvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    });
    world.insert_resource(BevyMapElitesArchive {
        archive: anima_engine_lib::evolution::map_elites::MapElitesArchive::new(0.5),
    });
    world.insert_resource(NextNodeId(3));

    let env_settings = Arc::new(std::sync::Mutex::new(EnvironmentSettings {
        time_speed: 1.0,
        fertility: 1.0,
        weather_severity: 1.0,
    }));
    world.insert_resource(BevyEnvironmentSettings(env_settings));

    let manual_disaster = Arc::new(std::sync::Mutex::new(None));
    world.insert_resource(BevyManualDisaster(manual_disaster));

    let (stats_tx, _stats_rx) = crossbeam_channel::bounded(128);
    let (_spawn_tx, spawn_rx) = crossbeam_channel::bounded(128);
    let (_env_tx, env_rx) = crossbeam_channel::bounded(32);

    world.insert_resource(EvolutionSender(stats_tx));
    world.insert_resource(EvolutionReceiver(spawn_rx));
    world.insert_resource(EnvironmentalEventReceiver(env_rx));
    world.insert_resource(anima_engine_lib::core::ecs::EpochManager {
        ticks_per_epoch: 1000,
        current_epoch_ticks: 0,
        current_epoch: 0,
    });
    world.insert_resource(EvolutionQueue::default());

    // Migration resources
    let (_inbound_tx, inbound_rx) = crossbeam_channel::unbounded();
    let (outbound_tx, _outbound_rx) = crossbeam_channel::unbounded();
    let sharding_config = Arc::new(std::sync::RwLock::new(ShardingConfig::default()));
    let (_manual_migration_tx, manual_migration_rx) = crossbeam_channel::unbounded();

    world.insert_resource(InboundMigrationReceiver(inbound_rx));
    world.insert_resource(OutboundMigrationSender(outbound_tx));
    world.insert_resource(ShardingResource(sharding_config));
    world.insert_resource(BevyMigrationTrigger(manual_migration_rx));

    let (tx, rx) = crossbeam_channel::unbounded();
    world.insert_resource(BevyPresetSpawnTrigger(rx));

    let tracker = Arc::new(FallbackLineageTracker::new("bolt://localhost:9999", "neo4j", "password"));
    world.insert_resource(BevyLineageTracker(tracker));

    // Queue all default presets to spawn
    let presets = get_default_presets();
    for preset in &presets {
        tx.send(preset.name.clone()).unwrap();
    }

    // Process spawns
    process_preset_spawns_system(&mut world);

    // Build the full production simulation schedule
    let mut schedule = Schedule::default();
    schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    schedule.add_systems((
        anima_engine_lib::core::ecs::update_day_night_cycle_system,
        sync_evolution_settings_system,
        sync_environment_settings_system,
        receive_manual_disasters_system,
        receive_environmental_events_system,
        apply_environmental_effects_system.after(receive_environmental_events_system),
        brain_inference_system,
        update_cpg_system.after(brain_inference_system),
        resolve_joints_system.after(update_cpg_system),
        integrate_physics_system.after(resolve_joints_system),
        anima_engine_lib::ai::pheromone::agent_release_pheromone_system.after(integrate_physics_system),
        anima_engine_lib::ai::pheromone::update_pheromone_grid_system.after(anima_engine_lib::ai::pheromone::agent_release_pheromone_system),
        anima_engine_lib::ai::pheromone::agent_read_pheromone_system.after(anima_engine_lib::ai::pheromone::update_pheromone_grid_system),
    ));
    schedule.add_systems((
        update_agent_evaluation_system.after(integrate_physics_system),
        anima_engine_lib::core::ecs::check_migration_boundaries_system.after(integrate_physics_system),
        apply_deferred.after(anima_engine_lib::core::ecs::check_migration_boundaries_system),
        anima_engine_lib::core::ecs::update_positions_system,
        wrap_coordinates_system.after(apply_deferred).after(anima_engine_lib::core::ecs::update_positions_system),
        anima_engine_lib::core::ecs::herbivore_grazing_system.after(wrap_coordinates_system),
        rebuild_spatial_grid_system.after(wrap_coordinates_system),
        anima_engine_lib::core::ecs::process_inbound_migrations_system.after(integrate_physics_system),
        metabolic_decay_system.after(integrate_physics_system),
        spawn_food_system.after(apply_environmental_effects_system),
        detect_food_collisions_system.after(integrate_physics_system),
        combat_system.after(integrate_physics_system),
        hrrl_learning_system.after(metabolic_decay_system),
        check_epoch_completion_system.after(metabolic_decay_system),
        apply_staggered_evolution_system.after(check_epoch_completion_system),
    ));

    // Run the simulation schedule for 100 ticks
    for _ in 0..100 {
        schedule.run(&mut world);
    }

    // Verify all agents are in a healthy non-crashing state
    let mut query = world.query::<(Entity, &Position, &Rotation, &Velocity, &HomeostaticState)>();
    let agents: Vec<_> = query.iter(&world).collect();
    
    assert!(agents.len() >= 5, "Expected at least 5 spawned preset agents");
    for (_entity, pos, rot, vel, homeo) in agents {
        assert!(!pos.0.is_nan(), "Agent position became NaN");
        assert!(!rot.0.is_nan(), "Agent rotation became NaN");
        assert!(!vel.0.is_nan(), "Agent velocity became NaN");
        assert!(!homeo.energy.is_nan(), "Agent energy became NaN");
        assert!(!homeo.hydration.is_nan(), "Agent hydration became NaN");
    }
}

