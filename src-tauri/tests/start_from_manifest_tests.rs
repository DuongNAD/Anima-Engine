//! Card W2-lite: start the desktop app engine from a validated experiment manifest (viewer only).
//!
//! Covers every requirement of W2-lite WHAT #5:
//! - (a) At tick 0 on the same machine, the world built by `apply_live_genesis_and_policies`
//!   equals `LiveExperimentAdapter`'s world (agents, lakes, trees, resource field, biomass,
//!   and all world-law policy resources; compared directly, never pinning a hardcoded checksum).
//! - (a') Headless `SimulationEngine::start::<tauri::test::MockRuntime>` via
//!   `start_from_manifest_with_engine` builds the SEL2-P selected-arm world (100 prey, 0
//!   predators, 16 spread lakes, 8 trees, scale 0.63, seed 780001).
//! - (b) Every preflight refusal (`laws.exotic_energy`, exotic forcings, unknown initial condition
//!   key, `lake_layout = 1` with `lakes = 0`, `drift_control = 1`, unsupported observables,
//!   malformed JSON) is propagated verbatim without starting or stopping the engine.
//! - (c) Legacy `SimulationEngine::start` with `pending_manifest = None` keeps today's genesis.

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::commands::{
    parse_and_validate_manifest_for_viewer, start_from_manifest_with_engine, EvolutionSettings,
    MapElitesGridState,
};
use anima_engine_lib::core::agent_systems::{AgentGenotype, AgentLineageId};
use anima_engine_lib::core::components::{Agent, Lake, Position, Predator, Prey, Tree};
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::ecs::{init_world, FoodSpawnSettings, MapBounds};
use anima_engine_lib::core::exotic_energy::{
    ExoticEnergyLaw, ExoticIntervention, ExoticInterventionKind,
};
use anima_engine_lib::core::experiment::{
    ExperimentError, ExperimentManifest, InitialConditionSet, ObservableRegistry, ObserverPolicy,
    WorldLawSet, MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::intervention::{Curve, InterventionCommand, InterventionKind, Region};
use anima_engine_lib::core::live_experiment::{
    apply_live_genesis_and_policies, LiveExperimentAdapter, LiveFoodCap, LiveWorldConfig,
    LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::resources::{
    BrainPolicy, LocomotionPolicy, MortalityPolicy, MutationPolicy, PlantReservePolicy,
    PredatorIncomePolicy, PreyCapPolicy, ReproductionPolicy, SimRng,
};
use anima_engine_lib::core::simulation_loop::SimulationEngine;
use anima_engine_lib::core::snapshot::world_checksum;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use anima_engine_lib::evolution::fitness::FitnessRuleSetting;
use bevy_ecs::prelude::*;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

static ENGINE_TEST_LOCK: Mutex<()> = Mutex::new(());

fn sel2p_selected_manifest() -> ExperimentManifest {
    let observable_ids: Vec<String> = LIVE_OBSERVABLE_IDS
        .iter()
        .copied()
        .chain([
            "live.trait_mass_mean",
            "live.trait_mass_var",
            "live.trait_mass_p10",
            "live.trait_mass_p90",
            "live.shannon",
            "live.simpson",
            "live.births",
            "live.deaths",
            "live.mean_lifespan",
            "live.archive_coverage",
            "live.mean_energy_integral",
            "live.trait_mass_mean_west",
            "live.trait_mass_mean_east",
        ])
        .map(str::to_string)
        .collect();

    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "sel2-p-food-scarcity-body-mass::selected".to_string(),
        name: "SEL2-P selected - 16 spread lakes, scale 0.63, mutation 0.05".to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: InitialConditionSet::new(vec![
            ("live.founders".into(), 100.0),
            ("live.predator_fraction".into(), 0.0),
            ("live.trees".into(), 8.0),
            ("live.lakes".into(), 16.0),
            ("live.lake_layout".into(), 1.0),
            ("live.resource_capacity_scale".into(), 0.63),
            ("live.plant_reserve".into(), 0.0),
            ("live.prey_cap".into(), 320.0),
            ("live.dehydration_ticks".into(), 960.0),
            ("live.mortality".into(), 1.0),
            ("live.reproduction".into(), 1.0),
            ("live.locomotion".into(), 1.0),
            ("live.drive_floor".into(), 0.5),
            ("live.thirst_steering".into(), 1.0),
            ("live.evolved_brains".into(), 0.0),
            ("live.mutation_rate".into(), 0.05),
            ("live.drift_control".into(), 0.0),
            ("live.drift_reset_ticks".into(), 0.0),
        ]),
        interventions: Vec::new(),
        seeds: vec![780001],
        duration_ticks: 200_000,
        sample_period: 500,
        observable_ids,
        exotic_interventions: Vec::new(),
        observer: ObserverPolicy::default(),
    }
}

#[derive(Debug, PartialEq)]
struct AgentRecord {
    lineage: String,
    is_predator: bool,
    is_prey: bool,
    pos: [i32; 3],
    mass_bits: u32,
    energy_bits: u32,
    hydration_bits: u32,
    has_brain: bool,
}

#[derive(Debug, PartialEq)]
struct LakeRecord {
    pos: [u32; 3],
    current_water: u32,
    max_water: u32,
    replenishment_rate: u32,
}

#[derive(Debug, PartialEq)]
struct TreeRecord {
    pos: [u32; 3],
    current_fruit: u32,
    max_fruit: u32,
    fruit_growth_rate: u32,
}

fn quantized_vec3_bits(v: glam::Vec3) -> [u32; 3] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

fn extract_agents(world: &mut World) -> Vec<AgentRecord> {
    let mut q = world.query_filtered::<(
        &AgentLineageId,
        &Position,
        &AgentGenotype,
        &HomeostaticState,
        Option<&Predator>,
        Option<&Prey>,
        Option<&anima_engine_lib::core::components::AgentBrain>,
    ), With<Agent>>();
    let mut out: Vec<AgentRecord> = q
        .iter(world)
        .map(|(lid, pos, geno, homeo, pred, prey, brain)| AgentRecord {
            lineage: lid.0.clone(),
            is_predator: pred.is_some(),
            is_prey: prey.is_some(),
            pos: [
                pos.0.x.to_bits() as i32,
                pos.0.y.to_bits() as i32,
                pos.0.z.to_bits() as i32,
            ],
            mass_bits: geno.0.total_mass().to_bits(),
            energy_bits: homeo.energy.to_bits(),
            hydration_bits: homeo.hydration.to_bits(),
            has_brain: brain.is_some(),
        })
        .collect();
    out.sort_by(|a, b| a.lineage.cmp(&b.lineage));
    out
}

fn extract_lakes(world: &mut World) -> Vec<LakeRecord> {
    let mut q = world.query::<(&Position, &Lake)>();
    let mut out: Vec<LakeRecord> = q
        .iter(world)
        .map(|(pos, lake)| LakeRecord {
            pos: quantized_vec3_bits(pos.0),
            current_water: lake.current_water.to_bits(),
            max_water: lake.max_water.to_bits(),
            replenishment_rate: lake.replenishment_rate.to_bits(),
        })
        .collect();
    out.sort_by_key(|l| l.pos);
    out
}

fn extract_trees(world: &mut World) -> Vec<TreeRecord> {
    let mut q = world.query::<(&Position, &Tree)>();
    let mut out: Vec<TreeRecord> = q
        .iter(world)
        .map(|(pos, tree)| TreeRecord {
            pos: quantized_vec3_bits(pos.0),
            current_fruit: tree.current_fruit.to_bits(),
            max_fruit: tree.max_fruit.to_bits(),
            fruit_growth_rate: tree.fruit_growth_rate.to_bits(),
        })
        .collect();
    out.sort_by_key(|t| t.pos);
    out
}

fn assert_worlds_equal_at_tick_zero(app_world: &mut World, adapter_world: &mut World, label: &str) {
    assert_eq!(
        extract_agents(app_world),
        extract_agents(adapter_world),
        "{label}: agent set and states must match at tick 0"
    );
    assert_eq!(
        extract_lakes(app_world),
        extract_lakes(adapter_world),
        "{label}: lakes must match at tick 0"
    );
    assert_eq!(
        extract_trees(app_world),
        extract_trees(adapter_world),
        "{label}: trees must match at tick 0"
    );

    let rf_app = app_world.resource::<ResourceField>();
    let rf_adp = adapter_world.resource::<ResourceField>();
    assert_eq!(rf_app.r, rf_adp.r, "{label}: standing crop r must match");
    assert_eq!(
        rf_app.r_max, rf_adp.r_max,
        "{label}: capacity r_max must match"
    );

    let bio_app = app_world.resource::<EcosystemBiomass>();
    let bio_adp = adapter_world.resource::<EcosystemBiomass>();
    assert_eq!(
        bio_app.plants, bio_adp.plants,
        "{label}: plants EU must match"
    );
    assert_eq!(
        bio_app.detritus, bio_adp.detritus,
        "{label}: detritus EU must match"
    );
    assert_eq!(
        bio_app.animals, bio_adp.animals,
        "{label}: animals EU must match"
    );
    assert_eq!(
        bio_app.dormant, bio_adp.dormant,
        "{label}: dormant EU must match"
    );

    assert_eq!(
        app_world.resource::<BrainPolicy>(),
        adapter_world.resource::<BrainPolicy>(),
        "{label}: BrainPolicy"
    );
    assert_eq!(
        app_world.resource::<MortalityPolicy>(),
        adapter_world.resource::<MortalityPolicy>(),
        "{label}: MortalityPolicy"
    );
    assert_eq!(
        app_world.resource::<ReproductionPolicy>(),
        adapter_world.resource::<ReproductionPolicy>(),
        "{label}: ReproductionPolicy"
    );
    assert_eq!(
        app_world.resource::<MutationPolicy>(),
        adapter_world.resource::<MutationPolicy>(),
        "{label}: MutationPolicy"
    );
    assert_eq!(
        app_world.resource::<PreyCapPolicy>(),
        adapter_world.resource::<PreyCapPolicy>(),
        "{label}: PreyCapPolicy"
    );
    assert_eq!(
        app_world.resource::<PlantReservePolicy>(),
        adapter_world.resource::<PlantReservePolicy>(),
        "{label}: PlantReservePolicy"
    );
    assert_eq!(
        app_world.resource::<LocomotionPolicy>(),
        adapter_world.resource::<LocomotionPolicy>(),
        "{label}: LocomotionPolicy"
    );
    assert_eq!(
        app_world.resource::<FitnessRuleSetting>(),
        adapter_world.resource::<FitnessRuleSetting>(),
        "{label}: FitnessRuleSetting"
    );
    assert_eq!(
        app_world.resource::<LiveFoodCap>(),
        adapter_world.resource::<LiveFoodCap>(),
        "{label}: LiveFoodCap"
    );
    assert_eq!(
        app_world.resource::<PredatorIncomePolicy>(),
        adapter_world.resource::<PredatorIncomePolicy>(),
        "{label}: PredatorIncomePolicy"
    );
    assert_eq!(
        app_world.resource::<FoodSpawnSettings>().max_food_count,
        adapter_world.resource::<FoodSpawnSettings>().max_food_count,
        "{label}: FoodSpawnSettings.max_food_count"
    );
    assert_eq!(
        app_world.resource::<SimRng>().seed(),
        adapter_world.resource::<SimRng>().seed(),
        "{label}: SimRng seed"
    );
    assert_eq!(
        app_world.resource::<SimRng>().stream_pos(),
        adapter_world.resource::<SimRng>().stream_pos(),
        "{label}: SimRng stream_pos"
    );

    assert_eq!(
        world_checksum(app_world),
        world_checksum(adapter_world),
        "{label}: same-machine tick-0 world checksum must match LiveExperimentAdapter"
    );
}

#[test]
fn app_world_matches_live_adapter_at_tick_zero_for_sel2p_and_full_policy_manifest() {
    let _guard = ENGINE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sel2p = sel2p_selected_manifest();
    let full_policy = ExperimentManifest {
        experiment_id: "w2-lite-all-policies".to_string(),
        name: "all-policies-check".to_string(),
        initial_conditions: InitialConditionSet::new(vec![
            ("live.founders".into(), 20.0),
            ("live.predator_fraction".into(), 0.3),
            ("live.trees".into(), 6.0),
            ("live.lakes".into(), 4.0),
            ("live.lake_layout".into(), 1.0),
            ("live.food_cap".into(), 75.0),
            ("live.evolved_brains".into(), 1.0),
            ("live.mortality".into(), 1.0),
            ("live.starve_ticks".into(), 90.0),
            ("live.dehydration_ticks".into(), 150.0),
            ("live.reproduction".into(), 1.0),
            ("live.mutation_rate".into(), 0.12),
            ("live.prey_cap".into(), 140.0),
            ("live.resource_capacity_scale".into(), 0.75),
            ("live.plant_reserve".into(), 0.2),
            ("live.locomotion".into(), 1.0),
            ("live.drive_floor".into(), 0.4),
            ("live.thirst_steering".into(), 1.0),
            ("live.ecological_fitness".into(), 1.0),
            ("live.w_surplus".into(), 1.5),
            ("live.kill_hydration".into(), 2.5),
            ("live.kill_efficiency".into(), 0.55),
            ("live.target_min_energy".into(), 10.0),
        ]),
        observable_ids: vec!["plants".into(), "live.agent_count".into()],
        seeds: vec![4242],
        ..sel2p.clone()
    };

    for (label, manifest) in [("sel2p-selected", &sel2p), ("full-policy", &full_policy)] {
        let json = serde_json::to_string(manifest).expect("serialize manifest");
        let pending = parse_and_validate_manifest_for_viewer(&json).expect("preflight accepts");
        let adapter = LiveExperimentAdapter::from_manifest(
            &manifest.laws,
            &manifest.initial_conditions,
            &manifest.exotic_interventions,
            pending.seed,
            (16, 16),
            manifest.duration_ticks,
        )
        .expect("adapter builds");

        // Simulate SimulationEngine::start()'s ambient resource inserts before calling the shared
        // genesis+policy helper, proving the manifest helper overrides env-based defaults.
        let mut app_world = init_world();
        app_world.insert_resource(MortalityPolicy::from_env());
        app_world.insert_resource(FitnessRuleSetting::from_env());
        let bounds = MapBounds::default();
        apply_live_genesis_and_policies(&mut app_world, pending.seed, &pending.config, bounds);

        let mut adapter_world = adapter.world();
        assert_worlds_equal_at_tick_zero(&mut app_world, &mut adapter_world, label);
    }
}

fn shared_handles() -> (
    Arc<Mutex<EvolutionSettings>>,
    Arc<AtomicBool>,
    Arc<Mutex<MapElitesGridState>>,
) {
    (
        Arc::new(Mutex::new(EvolutionSettings {
            mutation_rate: 0.15,
            selection_bias: 1.5,
            grid_resolution: 50,
        })),
        Arc::new(AtomicBool::new(false)),
        Arc::new(Mutex::new(MapElitesGridState {
            grid: std::collections::HashMap::new(),
            grid_resolution: 50,
        })),
    )
}

fn wait_for_first_published_tick(engine: &SimulationEngine) -> u64 {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(seed) = *engine.run_seed.read().unwrap_or_else(|e| e.into_inner()) {
            return seed;
        }
        assert!(
            Instant::now() < deadline,
            "simulation engine did not publish its first tick within 15 s"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn headless_engine_start_from_manifest_builds_sel2p_world() {
    let _guard = ENGINE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let manifest = sel2p_selected_manifest();
    let json = serde_json::to_string(&manifest).expect("serialize sel2p manifest");
    let config = LiveWorldConfig::from_initial_conditions(&manifest.initial_conditions)
        .expect("valid sel2p initial conditions");
    let adapter = LiveExperimentAdapter::from_manifest(
        &manifest.laws,
        &manifest.initial_conditions,
        &manifest.exotic_interventions,
        780001,
        (16, 16),
        manifest.duration_ticks,
    )
    .expect("adapter builds sel2p world");
    let mut adapter_lakes: Vec<(i32, i32)> = {
        let mut world = adapter.world();
        extract_lakes(&mut world)
            .into_iter()
            .map(|l| {
                (
                    (f32::from_bits(l.pos[0]) * 100.0).round() as i32,
                    (f32::from_bits(l.pos[2]) * 100.0).round() as i32,
                )
            })
            .collect()
    };
    adapter_lakes.sort();

    let engine = SimulationEngine::new();
    let (evo_settings, evo_running, map_grid) = shared_handles();
    let returned_name = start_from_manifest_with_engine::<tauri::test::MockRuntime>(
        &engine,
        None,
        evo_settings,
        evo_running,
        map_grid,
        &json,
    )
    .expect("start_from_manifest_with_engine must start SEL2-P world");
    assert_eq!(returned_name, manifest.name);

    let seed = wait_for_first_published_tick(&engine);
    engine.stop();
    assert_eq!(seed, 780001);
    assert_eq!(config.lakes, 16);

    let env_elements = engine
        .environmental_state
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .elements
        .clone();
    let mut engine_lakes: Vec<(i32, i32)> = env_elements
        .iter()
        .filter(|e| e.element_type == "lake")
        .map(|e| ((e.x * 100.0).round() as i32, (e.y * 100.0).round() as i32))
        .collect();
    engine_lakes.sort();
    let tree_count = env_elements
        .iter()
        .filter(|e| e.element_type == "tree")
        .count();
    assert_eq!(engine_lakes.len(), 16);
    assert_eq!(
        engine_lakes, adapter_lakes,
        "16 spread lake positions in SimulationEngine must match LiveExperimentAdapter"
    );
    assert_eq!(tree_count, 8);

    let eco = engine
        .ecosystem_state
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    assert!(
        eco.prey_count > 100 && eco.prey_count <= 320,
        "SEL2-P enables live.reproduction=1.0 with prey_cap=320, expected >100..=320 prey after priming tick, got {}",
        eco.prey_count
    );
    assert_eq!(eco.predator_count, 0);
    assert!(
        eco.detritus > 1000.0,
        "capacity scale 0.63 must move excess standing crop to detritus, got {}",
        eco.detritus
    );

    let segment_count = engine
        .agent_states
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .len();
    assert_eq!(
        segment_count,
        (eco.prey_count + eco.predator_count) as usize * 3,
        "each living three-segment founder/offspring publishes 3 segments"
    );

    // Second check: turning off live.reproduction isolates the exact 100-founder genesis and proves
    // ReproductionPolicy comes from the manifest rather than the app default.
    let mut no_repro = manifest;
    for (k, v) in &mut no_repro.initial_conditions.values {
        if k == "live.reproduction" {
            *v = 0.0;
        }
    }
    no_repro.observable_ids.retain(|id| id != "live.births");
    let no_repro_json = serde_json::to_string(&no_repro).expect("serialize no-repro manifest");
    let engine_fixed = SimulationEngine::new();
    let (evo_settings2, evo_running2, map_grid2) = shared_handles();
    let returned_fixed = start_from_manifest_with_engine::<tauri::test::MockRuntime>(
        &engine_fixed,
        None,
        evo_settings2,
        evo_running2,
        map_grid2,
        &no_repro_json,
    )
    .expect("start_from_manifest_with_engine must start non-reproducing manifest");
    assert_eq!(returned_fixed, no_repro.name);
    let seed_fixed = wait_for_first_published_tick(&engine_fixed);
    engine_fixed.stop();
    assert_eq!(seed_fixed, 780001);

    let eco_fixed = engine_fixed
        .ecosystem_state
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    assert_eq!(eco_fixed.prey_count, 100);
    assert_eq!(eco_fixed.predator_count, 0);
    assert_eq!(
        engine_fixed
            .agent_states
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .len(),
        300
    );
}

#[test]
fn preflight_refusals_are_propagated_verbatim_without_disturbing_engine() {
    let _guard = ENGINE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let base = sel2p_selected_manifest();

    // 1. Exotic energy law refused with the adapter's exact reason.
    let mut exotic_law = base.clone();
    exotic_law.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(100.0, 2));
    let expected_exotic_err = LiveExperimentAdapter::from_manifest(
        &exotic_law.laws,
        &exotic_law.initial_conditions,
        &exotic_law.exotic_interventions,
        780001,
        (16, 16),
        exotic_law.duration_ticks,
    )
    .expect_err("adapter refuses exotic law")
    .to_string();

    // 2. Exotic interventions refused (validation refuses exotic_interventions without law).
    let mut exotic_forcing = base.clone();
    exotic_forcing.exotic_interventions = vec![ExoticIntervention {
        id: 9,
        cause_id: 9,
        kind: ExoticInterventionKind::AddSource,
        region: Region::Global,
        start_tick: 1,
        duration_ticks: 100,
        amount: 1.0,
        curve: Curve::Step,
    }];

    // 3. Unknown initial-condition key refused.
    let mut unknown_key = base.clone();
    unknown_key
        .initial_conditions
        .values
        .push(("plants".into(), 100.0));
    let expected_unknown_err =
        LiveWorldConfig::from_initial_conditions(&unknown_key.initial_conditions)
            .expect_err("unknown key refused")
            .to_string();

    // 4. Impossible lake_layout = 1 with lakes = 0 refused.
    let mut bad_lakes = base.clone();
    bad_lakes.initial_conditions = InitialConditionSet::new(vec![
        ("live.lakes".into(), 0.0),
        ("live.lake_layout".into(), 1.0),
    ]);

    // 5. Drift control refused without a paired TurnoverSchedule.
    let mut drift = base.clone();
    drift.initial_conditions = InitialConditionSet::new(vec![
        ("live.mortality".into(), 1.0),
        ("live.reproduction".into(), 1.0),
        ("live.drift_control".into(), 1.0),
    ]);

    // 6. Observable requested without producing mortality rule refused (O-5).
    let mut missing_mortality = base.clone();
    missing_mortality.initial_conditions =
        InitialConditionSet::new(vec![("live.mortality".into(), 0.0)]);
    missing_mortality.observable_ids = vec!["live.deaths".into()];

    let engine = SimulationEngine::new();
    let (evo_settings, evo_running, map_grid) = shared_handles();

    let err_exotic = start_from_manifest_with_engine::<tauri::test::MockRuntime>(
        &engine,
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_grid),
        &serde_json::to_string(&exotic_law).unwrap(),
    )
    .expect_err("exotic energy must be refused");
    assert_eq!(err_exotic, expected_exotic_err);
    assert!(!engine.running.load(std::sync::atomic::Ordering::SeqCst));

    let err_unknown = start_from_manifest_with_engine::<tauri::test::MockRuntime>(
        &engine,
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_grid),
        &serde_json::to_string(&unknown_key).unwrap(),
    )
    .expect_err("unknown initial key must be refused");
    assert_eq!(err_unknown, expected_unknown_err);

    for bad_manifest in [exotic_forcing, bad_lakes, drift, missing_mortality] {
        let json = serde_json::to_string(&bad_manifest).unwrap();
        let err = parse_and_validate_manifest_for_viewer(&json)
            .expect_err("preflight must refuse invalid manifest");
        assert!(!err.is_empty());
    }

    assert!(matches!(
        LiveExperimentAdapter::from_manifest(
            &exotic_law.laws,
            &exotic_law.initial_conditions,
            &[],
            780001,
            (16, 16),
            100,
        ),
        Err(ExperimentError::InvalidLaw { .. })
    ));

    let err_json = start_from_manifest_with_engine::<tauri::test::MockRuntime>(
        &engine,
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_grid),
        "{not valid json",
    )
    .expect_err("malformed JSON must be refused");
    assert!(err_json.contains("invalid experiment manifest JSON"));
    assert!(!engine.running.load(std::sync::atomic::Ordering::SeqCst));

    // 7. Scheduled interventions are applied by LiveExperimentAdapter but not by the viewer.
    let mut scheduled = base.clone();
    scheduled.interventions = vec![InterventionCommand {
        id: 1,
        cause_id: 7,
        kind: InterventionKind::Deforestation,
        region: Region::Global,
        start_tick: 1,
        duration_ticks: 100,
        intensity: 0.5,
        signed_negative: true,
        curve: Curve::Step,
        reversible: false,
    }];
    scheduled
        .validate(&ObservableRegistry::live_default())
        .expect("scheduled intervention manifest is valid for headless runs");
    let scheduled_json = serde_json::to_string(&scheduled).unwrap();
    let err_scheduled = start_from_manifest_with_engine::<tauri::test::MockRuntime>(
        &engine,
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_grid),
        &scheduled_json,
    )
    .expect_err("viewer must refuse manifests that schedule interventions");
    assert_eq!(
        err_scheduled,
        "this manifest schedules interventions; the viewer cannot run them"
    );
    assert!(!engine.running.load(std::sync::atomic::Ordering::SeqCst));

    // A rejected start_from_manifest (including scheduled interventions) must NOT stop an already-running simulation.
    start_from_manifest_with_engine::<tauri::test::MockRuntime>(
        &engine,
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_grid),
        &serde_json::to_string(&base).unwrap(),
    )
    .expect("valid start");
    let _ = wait_for_first_published_tick(&engine);
    let refused_while_running = start_from_manifest_with_engine::<tauri::test::MockRuntime>(
        &engine,
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_grid),
        &scheduled_json,
    );
    assert_eq!(
        refused_while_running.expect_err("refuse while running"),
        "this manifest schedules interventions; the viewer cannot run them"
    );
    assert!(
        engine.running.load(std::sync::atomic::Ordering::SeqCst),
        "invalid manifest refusal must not stop a running simulation"
    );
    let refused_bad_json = start_from_manifest_with_engine::<tauri::test::MockRuntime>(
        &engine,
        None,
        evo_settings,
        evo_running,
        map_grid,
        "{bad}",
    );
    assert!(refused_bad_json.is_err());
    assert!(
        engine.running.load(std::sync::atomic::Ordering::SeqCst),
        "malformed manifest refusal must not stop a running simulation"
    );
    engine.stop();
}

#[test]
fn bundled_frontend_sel2p_manifest_passes_viewer_preflight_and_matches_fixture() {
    let tsx = include_str!("../../src/components/ExperimentWorldControl.tsx");
    let start_marker = "/* BEGIN_SEL2P_SELECTED_JSON */ `";
    let end_marker = "` /* END_SEL2P_SELECTED_JSON */";
    let after_start = tsx
        .split_once(start_marker)
        .expect("BEGIN_SEL2P_SELECTED_JSON marker")
        .1;
    let extracted_json = after_start
        .split_once(end_marker)
        .expect("END_SEL2P_SELECTED_JSON marker")
        .0;

    let pending = parse_and_validate_manifest_for_viewer(extracted_json)
        .expect("bundled frontend SEL2-P JSON must pass viewer preflight");
    assert_eq!(pending.seed, 780001);
    assert_eq!(pending.config.founders, 100);
    assert_eq!(pending.config.lakes, 16);
    assert_eq!(pending.config.lake_layout, 1);
    assert!((pending.config.resource_capacity_scale - 0.63).abs() < 1e-12);
    assert_eq!(pending.config.prey_cap, Some(320));
    assert_eq!(pending.config.dehydration_ticks, 960);

    let parsed: ExperimentManifest =
        serde_json::from_str(extracted_json).expect("deserialize ExperimentManifest");
    assert_eq!(parsed.seeds.len(), 20);
    assert_eq!(parsed.seeds[0], 780001);
    assert_eq!(parsed.seeds[19], 799001);
    assert!(parsed.interventions.is_empty());
    assert!(parsed.exotic_interventions.is_empty());

    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/experiments_sel2p/sel2p-selected.json");
    if fixture_path.exists() {
        let fixture_json =
            std::fs::read_to_string(&fixture_path).expect("read committed sel2p-selected.json");
        assert_eq!(
            extracted_json, fixture_json,
            "bundled frontend manifest drifted from committed fixture"
        );
    }
}

#[test]
fn legacy_start_without_manifest_keeps_default_genesis() {
    let _guard = ENGINE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut seeds = Vec::new();
    let mut env_counts = Vec::new();
    let mut lake_coords_runs = Vec::new();
    let mut eco_pairs = Vec::new();

    for _ in 0..2 {
        let engine = SimulationEngine::new();
        assert!(engine
            .pending_manifest
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_none());
        let (evo_settings, evo_running, map_grid) = shared_handles();
        engine.start::<tauri::test::MockRuntime>(None, evo_settings, evo_running, map_grid);
        let seed = wait_for_first_published_tick(&engine);
        engine.stop();

        let env = engine
            .environmental_state
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .elements
            .clone();
        let lakes = env.iter().filter(|e| e.element_type == "lake").count();
        let trees = env.iter().filter(|e| e.element_type == "tree").count();
        let mut lake_coords: Vec<(i32, i32)> = env
            .iter()
            .filter(|e| e.element_type == "lake")
            .map(|e| ((e.x * 100.0).round() as i32, (e.y * 100.0).round() as i32))
            .collect();
        lake_coords.sort();
        let eco = engine
            .ecosystem_state
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        assert!(
            (1..=5).contains(&lakes),
            "legacy genesis spawns at most 5 lakes (not 16 manifest lakes), got {lakes}"
        );
        assert!(
            (7..=20).contains(&eco.prey_count),
            "legacy genesis spawns 7 prey (with default ReproductionPolicy active on tick 1), got {}",
            eco.prey_count
        );
        assert_eq!(eco.predator_count, 3);

        seeds.push(seed);
        env_counts.push((lakes, trees));
        lake_coords_runs.push(lake_coords);
        eco_pairs.push((eco.prey_count, eco.predator_count));
    }

    assert_eq!(seeds[0], seeds[1]);
    assert_eq!(env_counts[0], env_counts[1]);
    assert_eq!(lake_coords_runs[0], lake_coords_runs[1]);
    assert_eq!(eco_pairs[0], eco_pairs[1]);
}
