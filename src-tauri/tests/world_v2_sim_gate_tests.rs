//! Card WV2-3 `G-SIM` Gate Suite (`GS-1..GS-5`) & Pre-Switchover Negative Controls.
//!
//! Verifies:
//! - `GS-1`: Render-Sim grid parity (`ANMW v3` round-trip, `AnmwV3Sim::peek_seed`, dual `LD-2`
//!   identities, raw `f32` block mean normalized elevation parity `<= 1e-5`, biome bounds `<= 10`,
//!   100% `Ocean`/`DeepOcean` mask parity, and Strahler >= 4 river 8-connectivity on `256^2`).
//! - `GS-2`: `LAND1` walkability, `>= 80%` largest 4-connected walkable component, `>= 500` walkable
//!   shoreline cells, 200-agent 1,000-tick run via `build_tick_schedule` with `0` agents on
//!   `DeepOcean`/`Ocean`, plus `Ocean`-founder/ocean-step reversion negative controls.
//! - `GS-3`: World v2 `ResourceField::from_biomes` carrying capacity within `[0.4x, 2.0x]` of
//!   World v1 baseline and closed-EU residual drift `< 1e-6` over 1,000 ticks with zero `NaN`/`Inf`.
//! - `GS-4`: Zero heap allocations on the warm World v2 tick path (`ThreadTrackingAllocator`).
//! - `GS-5`: 1-byte tamper rejection on `sim_256.anmw3` & `index.json`, loud pre-switchover panic on
//!   `ANMW v3` in `world_seed_from_disk()` and `ecs::init_world()`, and bit-identical paired
//!   same-machine `E-SEL1` & `SEL2-P` runs regardless of `ANMW v3` presence on disk.

mod common;

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::{HomeostaticState, Transition, TransitionSender};
use anima_engine_lib::ai::model::{BrainInferenceBuffer, BrainModel};
use anima_engine_lib::ai::pheromone::PheromoneGrid;
use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};
use anima_engine_lib::core::agent_systems::{
    ActiveEvolutionSettings, BevyEvolutionRunning, BevyEvolutionSettings, BevyMapElitesArchive,
    BevyMapElitesGrid, EnvironmentalEventReceiver, InferenceChannels, InferenceRequestBatch,
    InferenceResponseBatch, NextNodeId,
};
use anima_engine_lib::core::components::{
    Agent, LastWalkablePosition, ParentAgent, Position, Velocity,
};
use anima_engine_lib::core::determinism::DeterministicMode;
use anima_engine_lib::core::ecology::{EcosystemBiomass, ResourceField};
use anima_engine_lib::core::ecs::{
    init_world, init_world_with, ActiveEnvironmentEvent, BevyMigrationTrigger,
    EnvironmentalSpawnSettings, EpochManager, FoodSpawnSettings, InboundMigrationReceiver,
    OutboundMigrationSender, ShardingConfig, ShardingResource, TerrainSource,
};
use anima_engine_lib::core::energy_ledger::{closed_total_eu, EnergyLedger};
use anima_engine_lib::core::environmental_systems::{
    ecosystem_census_system, herbivore_grazing_system, resource_field_regrowth_system,
};
use anima_engine_lib::core::experiment::{ExperimentManifest, InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::run_manifest_seed;
use anima_engine_lib::core::live_experiment::{
    try_apply_live_genesis_and_policies, LiveExperimentAdapter, LiveExperimentState, LiveForcings,
    LiveInferencePump, LiveInterventions, LiveWorldConfig,
};
use anima_engine_lib::core::locomotion::steering_drive_system;
use anima_engine_lib::core::observer::{ObserverPolicy, ObserverTrace, SharedObserverActions};
use anima_engine_lib::core::resources::{
    manual_migration_channel, AgentEpochStats, MapBounds, ScientificCounterFault,
};
use anima_engine_lib::core::simulation_lod::{LodBands, LodFocus};
use anima_engine_lib::core::simulation_schedule::build_tick_schedule;
use anima_engine_lib::core::terrain::{BiomeType, TerrainMap};
use anima_engine_lib::core::terrain_navigation::{
    cell_center, is_biome_walkable, is_pos_walkable, is_walkable_cell_next_to_water,
    is_water_biome, terrain_navigation_system, wrap_pos,
};
use anima_engine_lib::core::world_artifact::{
    default_artifact_path, terrain_map_from_v3_sim, try_world_seed_from_disk, world_seed_from_disk,
    WorldArtifact, WorldIdentity,
};
use anima_engine_lib::core::world_systems::wrap_coordinates_system;
use anima_engine_lib::evolution::map_elites::MapElitesArchive;
use anima_engine_lib::physics::dynamics::RigidBody;
use anima_engine_lib::physics::{integrate_physics_system, SpatialHashGrid};
use anima_worldgen::anmw_v3::{
    compute_bundle_identity, compute_sim_world_identity, compute_strahler_order,
    derive_sim_seed_u32, project_raw_to_sim_256, verify_bundle_identity, verify_sim_world_identity,
    AnmwV3CaveEntrance, AnmwV3Sim, AnmwV3Waterfall, RawSimProjectionInput, ANMW_V3_FLAGS_STANDARD,
    ANMW_V3_SIM_SIZE, ANMW_V3_WORLD_SCALE, D8_NO_DIR,
};
use anima_worldgen::{normalize_elevation, BiomeId, WaterKind, WORLD_EXTENT_M};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ExecutorKind;
use crossbeam_channel::Receiver;
use glam::Vec3;
use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[global_allocator]
static ALLOCATOR: common::allocator::ThreadTrackingAllocator =
    common::allocator::ThreadTrackingAllocator::new();

static ENV_LOCK: Mutex<()> = Mutex::new(());
const DEEP_OCEAN_THRESHOLD_M: f32 = -220.0;

const ENV_VARS: [&str; 5] = [
    "ANIMA_WORLD_ARTIFACT",
    "ANIMA_CACHE_DIR",
    "TMPDIR",
    "TEMP",
    "TMP",
];

struct EnvGuard {
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl EnvGuard {
    fn capture() -> Self {
        let saved = ENV_VARS
            .iter()
            .map(|&name| (name, std::env::var_os(name)))
            .collect();
        Self { saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (name, val) in &self.saved {
            match val {
                Some(v) => std::env::set_var(name, v),
                None => std::env::remove_var(name),
            }
        }
    }
}

struct SyntheticRawWorld {
    size: usize,
    seed_u64: u64,
    elev_m: Vec<f32>,
    moisture: Vec<f32>,
    temp_c: Vec<f32>,
    flow: Vec<f32>,
    slope: Vec<f32>,
    river_w_m: Vec<f32>,
    biome_22: Vec<u8>,
    water_kind: Vec<u8>,
    d8_dir: Vec<u8>,
    is_channel: Vec<bool>,
    strahler: Vec<u8>,
    order4_cells: Vec<(usize, usize)>,
}

fn dir_from_delta(dc: isize, dr: isize) -> u8 {
    // Matches `DX`/`DZ` D8 encoding in `anima_worldgen::anmw_v3`:
    // 0: E, 1: SE, 2: S, 3: SW, 4: W, 5: NW, 6: N, 7: NE.
    match (dc, dr) {
        (1, 0) => 0,
        (1, 1) => 1,
        (0, 1) => 2,
        (-1, 1) => 3,
        (-1, 0) => 4,
        (-1, -1) => 5,
        (0, -1) => 6,
        (1, -1) => 7,
        _ => D8_NO_DIR,
    }
}

fn build_synthetic_raw_world_512() -> SyntheticRawWorld {
    let size = 512usize;
    let n = size * size;
    let seed_u64 = 0x5756_3203_u64;

    let mut elev_m = vec![0.0f32; n];
    let mut moisture = vec![0.0f32; n];
    let mut temp_c = vec![24.0f32; n];
    let mut flow = vec![0.1f32; n];
    let mut slope = vec![0.02f32; n];
    let mut river_w_m = vec![0.0f32; n];
    let mut biome_22 = vec![BiomeId::Grassland as u8; n];
    let mut water_kind = vec![WaterKind::None as u8; n];
    let mut d8_dir = vec![D8_NO_DIR; n];
    let mut is_channel = vec![false; n];

    // Sinusoidal coastline around x ~= 224 gives ~44% Ocean/DeepOcean, >= 500 shoreline cells
    // on 256^2, and realistic global NPP within [0.4x, 2.0x] of World v1.
    let mut coast_x_for_row = vec![224.0f32; size];
    for (r, cx) in coast_x_for_row.iter_mut().enumerate() {
        let phase = (r as f32 / size as f32) * std::f32::consts::TAU * 5.0;
        *cx = 224.0 + 26.0 * phase.sin() + 10.0 * (phase * 2.0).cos();
    }

    for (r, &coast_x) in coast_x_for_row.iter().enumerate() {
        let lat_frac = r as f32 / size as f32;
        for c in 0..size {
            let idx = r * size + c;
            let dist_inland = c as f32 - coast_x;
            if dist_inland < 0.0 {
                water_kind[idx] = WaterKind::Ocean as u8;
                biome_22[idx] = BiomeId::Ocean as u8;
                let depth = if (c as f32) < coast_x - 75.0 {
                    -260.0 + (c as f32 - (coast_x - 75.0)) * 5.0
                } else {
                    -10.0 + dist_inland * 2.4
                };
                elev_m[idx] = depth.max(-1400.0);
                moisture[idx] = 1.0;
                temp_c[idx] = 26.0;
                slope[idx] = 0.01;
                continue;
            }

            let ridge_dx = (c as f32 - 395.0) / 115.0;
            let ridge_dy = (r as f32 - 256.0) / 160.0;
            let gauss = (-(ridge_dx * ridge_dx + ridge_dy * ridge_dy)).exp();
            let base_slope = (dist_inland * 3.0).min(1150.0);
            let peak = 4500.0 * gauss;
            let z = (base_slope + peak).clamp(2.0, 5600.0);
            elev_m[idx] = z;

            let m = if c < 395 {
                0.68 - 0.18 * (c as f32 - 224.0) / 171.0
            } else {
                0.24 + 0.14 * lat_frac
            };
            moisture[idx] = m.clamp(0.15, 0.95);
            temp_c[idx] = (28.0 - 0.0063 * z).clamp(-12.0, 34.0);
            slope[idx] = (0.03 + 0.34 * gauss).clamp(0.01, 0.85);

            biome_22[idx] = if dist_inland < 8.0 && z < 26.0 {
                BiomeId::Beach as u8
            } else if z > 4600.0 {
                BiomeId::Snow as u8
            } else if z > 3200.0 {
                BiomeId::Rock as u8
            } else if z > 2200.0 {
                BiomeId::Taiga as u8
            } else if c > 410 && !(180..=330).contains(&r) {
                BiomeId::Desert as u8
            } else if c < 285 && (150..=310).contains(&r) {
                BiomeId::Jungle as u8
            } else if c < 340 && (110..=380).contains(&r) {
                BiomeId::Forest as u8
            } else {
                BiomeId::Grassland as u8
            };
        }
    }

    // Highland freshwater lake (12x12 raw -> ~6x6 sim cells, well inland of coast).
    for r in 296..308 {
        for c in 310..322 {
            let idx = r * size + c;
            elev_m[idx] = 420.0;
            water_kind[idx] = WaterKind::FreshwaterLake as u8;
            biome_22[idx] = BiomeId::Lake as u8;
            moisture[idx] = 1.0;
            flow[idx] = 0.0;
        }
    }

    let mut carve_segment = |pts: &[(usize, usize)], q_flow: f32, w_m: f32| {
        for w in pts.windows(2) {
            let (mut cx, mut cy) = w[0];
            let (tx, ty) = w[1];
            while (cx, cy) != (tx, ty) {
                let dc = (tx as isize - cx as isize).signum();
                let dr = (ty as isize - cy as isize).signum();
                let nx = (cx as isize + dc) as usize;
                let ny = (cy as isize + dr) as usize;
                let idx = cy * size + cx;
                d8_dir[idx] = dir_from_delta(dc, dr);
                is_channel[idx] = true;
                if water_kind[idx] != WaterKind::Ocean as u8 {
                    water_kind[idx] = WaterKind::River as u8;
                    biome_22[idx] = BiomeId::River as u8;
                    flow[idx] = flow[idx].max(q_flow);
                    river_w_m[idx] = river_w_m[idx].max(w_m);
                }
                cx = nx;
                cy = ny;
            }
        }
    };

    // Disjoint 45-degree binary tree yielding exact Strahler >= 4 along the 1-cell-wide trunk.
    let mouth_x = coast_x_for_row[220].ceil() as usize;
    let leaves_o1: [((usize, usize), (usize, usize)); 8] = [
        ((364, 146), (350, 160)),
        ((364, 174), (350, 160)),
        ((360, 190), (350, 200)),
        ((360, 210), (350, 200)),
        ((360, 230), (350, 240)),
        ((360, 250), (350, 240)),
        ((364, 266), (350, 280)),
        ((364, 294), (350, 280)),
    ];
    for (s, t) in leaves_o1 {
        carve_segment(&[s, t], 2.0, 2.0);
    }
    let merges_o2: [((usize, usize), (usize, usize)); 4] = [
        ((350, 160), (330, 180)),
        ((350, 200), (330, 180)),
        ((350, 240), (330, 260)),
        ((350, 280), (330, 260)),
    ];
    for (s, t) in merges_o2 {
        carve_segment(&[s, t], 6.0, 4.5);
    }
    carve_segment(&[(330, 180), (290, 220)], 14.0, 6.5);
    carve_segment(&[(330, 260), (290, 220)], 14.0, 6.5);
    carve_segment(&[(290, 220), (mouth_x.saturating_sub(1), 220)], 32.0, 10.0);

    let mut order4_cells = Vec::new();
    for x in ((mouth_x + 1)..=290).rev() {
        order4_cells.push((x, 220));
    }

    let strahler = compute_strahler_order(&d8_dir, &is_channel, size, size);

    SyntheticRawWorld {
        size,
        seed_u64,
        elev_m,
        moisture,
        temp_c,
        flow,
        slope,
        river_w_m,
        biome_22,
        water_kind,
        d8_dir,
        is_channel,
        strahler,
        order4_cells,
    }
}

fn project_synthetic_v3(raw: &SyntheticRawWorld) -> (AnmwV3Sim, Vec<u8>) {
    let input = RawSimProjectionInput {
        src_size: raw.size as u32,
        dx_m: WORLD_EXTENT_M / raw.size as f32,
        seed_u64: raw.seed_u64,
        generator_version: 3,
        world_scale: ANMW_V3_WORLD_SCALE,
        total_h_m: &raw.elev_m,
        moisture: &raw.moisture,
        temp_c: &raw.temp_c,
        discharge_q_m3_s: &raw.flow,
        slope: Some(&raw.slope),
        river_w_m: &raw.river_w_m,
        water_kind: &raw.water_kind,
        dominant_biome: &raw.biome_22,
        d8_dir: &raw.d8_dir,
        is_channel: &raw.is_channel,
        waterfalls: vec![AnmwV3Waterfall {
            x_m: 30.0,
            z_m: -12.0,
            lip_elev_m: 1200.0,
            drop_m: 38.0,
            yaw_rad: std::f32::consts::PI,
            discharge_q_m3_s: 18.0,
            plunge_depth_m: 6.5,
            flags: 0,
        }],
        cave_entrances: vec![AnmwV3CaveEntrance {
            x_m: 42.0,
            z_m: 18.0,
            elevation_m: 840.0,
            yaw_rad: 0.75,
            clearance_m: 4.0,
            mouth_kind: 1,
            realm_status: 1,
        }],
    };
    let sim = project_raw_to_sim_256(&input).expect("project synthetic raw world to 256^2");
    let bytes = sim.to_bytes().expect("encode ANMW v3 bytes");
    (sim, bytes)
}

struct WorldV2Harness {
    world: World,
    schedule: Schedule,
    transitions: Receiver<Transition>,
    epoch_stats: Receiver<Vec<AgentEpochStats>>,
    _spawn_tx: crossbeam_channel::Sender<anima_engine_lib::core::resources::EvolutionSpawn>,
    _env_tx: crossbeam_channel::Sender<anima_engine_lib::evolution::meta_ai::EnvironmentalEvent>,
    _inbound_tx: crossbeam_channel::Sender<anima_engine_lib::core::components::AgentMigrationData>,
    _outbound_rx: Receiver<anima_engine_lib::core::components::OutboundMigration>,
    _migration_tx: crossbeam_channel::Sender<u16>,
}

impl WorldV2Harness {
    fn step_once(&mut self) {
        self.schedule.run(&mut self.world);
        while self.transitions.try_recv().is_ok() {}
        while self.epoch_stats.try_recv().is_ok() {}
    }

    fn drain_queues(&self) {
        while self.transitions.try_recv().is_ok() {}
        while self.epoch_stats.try_recv().is_ok() {}
    }
}

fn build_world_v2_harness(
    terrain: TerrainMap,
    world_identity: WorldIdentity,
    pairs: &[(&str, f64)],
    seed: u64,
) -> WorldV2Harness {
    let initial =
        InitialConditionSet::new(pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect());
    let config = LiveWorldConfig::from_initial_conditions(&initial).expect("valid LiveWorldConfig");
    let laws = WorldLawSet::baseline();

    let mut world = init_world_with(TerrainSource::Generated);
    let bounds = MapBounds::default();
    world.insert_resource(bounds);
    world.insert_resource(SpatialHashGrid::new_prepopulated(10.0, &bounds));

    let resource_field = ResourceField::from_biomes(
        &terrain.biomes,
        terrain.width,
        terrain.height,
        bounds.min.x,
        bounds.min.z,
        bounds.max.x,
        bounds.max.z,
        0.02,
    );
    let starting_plants = resource_field.total_biomass();
    world.insert_resource(resource_field);
    world.insert_resource(EcosystemBiomass {
        detritus: 0.0,
        plants: starting_plants,
        animals: 0.0,
        dormant: 0.0,
    });
    world.insert_resource(EnergyLedger::default());
    world.insert_resource(terrain);
    world.insert_resource(world_identity);
    world.insert_resource(PheromoneGrid::default());
    world.insert_resource(EpochManager {
        ticks_per_epoch: 1000,
        current_epoch_ticks: 0,
        current_epoch: 0,
    });
    world.insert_resource(ActiveEnvironmentEvent::default());

    let (req_tx, req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
    let (recycle_req_tx, recycle_req_rx) = crossbeam_channel::unbounded::<InferenceRequestBatch>();
    let (res_tx, res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();
    let (recycle_res_tx, recycle_res_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();
    for _ in 0..16 {
        let _ = recycle_req_tx.send(InferenceRequestBatch {
            requests: Vec::with_capacity(128),
        });
        let _ = recycle_res_tx.send(InferenceResponseBatch {
            responses: Vec::with_capacity(128),
        });
    }
    world.insert_resource(InferenceChannels {
        req_tx,
        recycle_req_rx,
        res_rx,
        recycle_res_tx,
    });
    world.insert_resource(LiveInferencePump::new(
        req_rx,
        res_tx,
        recycle_req_tx,
        recycle_res_rx,
    ));
    let (trans_tx, trans_rx) = crossbeam_channel::unbounded::<Transition>();
    world.insert_resource(TransitionSender(trans_tx));
    let (stats_tx, stats_rx) = crossbeam_channel::unbounded::<Vec<AgentEpochStats>>();
    let (spawn_tx, spawn_rx) =
        crossbeam_channel::unbounded::<anima_engine_lib::core::resources::EvolutionSpawn>();
    world.insert_resource(anima_engine_lib::core::resources::EvolutionSender(stats_tx));
    world.insert_resource(anima_engine_lib::core::resources::EvolutionReceiver(
        spawn_rx,
    ));
    world.insert_resource(anima_engine_lib::core::resources::EvolutionQueue::default());
    world.insert_resource(anima_engine_lib::core::resources::VitalStatistics::default());

    let (env_tx, env_rx) =
        crossbeam_channel::unbounded::<anima_engine_lib::evolution::meta_ai::EnvironmentalEvent>();
    world.insert_resource(EnvironmentalEventReceiver(env_rx));
    let (inbound_tx, inbound_rx) =
        crossbeam_channel::unbounded::<anima_engine_lib::core::components::AgentMigrationData>();
    let (outbound_tx, outbound_rx) =
        crossbeam_channel::unbounded::<anima_engine_lib::core::components::OutboundMigration>();
    let (migration_tx, migration_rx) = manual_migration_channel();
    world.insert_resource(InboundMigrationReceiver(inbound_rx));
    world.insert_resource(OutboundMigrationSender(outbound_tx));
    world.insert_resource(ShardingResource(Arc::new(std::sync::RwLock::new(
        ShardingConfig::default(),
    ))));
    world.insert_resource(BevyMigrationTrigger(migration_rx));

    world.insert_resource(BevyEvolutionSettings(Arc::new(Mutex::new(
        EvolutionSettings {
            mutation_rate: 0.15,
            selection_bias: 1.5,
            grid_resolution: 50,
        },
    ))));
    world.insert_resource(BevyEvolutionRunning(Arc::new(AtomicBool::new(false))));
    world.insert_resource(BevyMapElitesGrid(Arc::new(Mutex::new(
        MapElitesGridState {
            grid: std::collections::HashMap::new(),
            grid_resolution: 50,
        },
    ))));
    world.insert_resource(ActiveEvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    });
    world.insert_resource(BevyMapElitesArchive {
        archive: MapElitesArchive::new(1.0 / 50.0),
    });
    world.insert_resource(NextNodeId(3));
    world.insert_resource(EnvironmentalSpawnSettings::default());
    world.insert_resource(FoodSpawnSettings::default());
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(BrainModel::new_seeded_cpu(15, 64, 4, seed));
    world.insert_resource(BrainInferenceBuffer::default());
    world.insert_resource(LodFocus::default());
    world.insert_resource(LodBands::default());
    world.insert_resource(ObserverPolicy::Absent);
    world.insert_resource(ObserverTrace::default());
    world.insert_resource(SharedObserverActions::new());

    try_apply_live_genesis_and_policies(&mut world, seed, &config, bounds)
        .expect("apply live genesis on decoded World v2 TerrainMap");

    world.insert_resource(LiveExperimentState::new(seed, laws, initial, 0, 0));
    world.insert_resource(LiveForcings::default());
    world.insert_resource(LiveInterventions::with_capacity(16));
    world.insert_resource(ScientificCounterFault::default());

    let deterministic = DeterministicMode::on();
    world.insert_resource(deterministic);
    let schedule = build_tick_schedule(deterministic);

    WorldV2Harness {
        world,
        schedule,
        transitions: trans_rx,
        epoch_stats: stats_rx,
        _spawn_tx: spawn_tx,
        _env_tx: env_tx,
        _inbound_tx: inbound_tx,
        _outbound_rx: outbound_rx,
        _migration_tx: migration_tx,
    }
}

fn biome_from_u8(id: u8) -> BiomeType {
    match id {
        0 => BiomeType::DeepOcean,
        1 => BiomeType::Ocean,
        2 => BiomeType::Beach,
        3 => BiomeType::River,
        4 => BiomeType::Grassland,
        5 => BiomeType::TemperateForest,
        6 => BiomeType::BorealForest,
        7 => BiomeType::Rainforest,
        8 => BiomeType::Desert,
        9 => BiomeType::MountainRock,
        10 => BiomeType::Snow,
        other => panic!("unexpected backend biome index {other}"),
    }
}

// ---- GS-1: Render-Sim Grid Parity ---------------------------------------------------------------

#[test]
fn gs1_render_sim_grid_parity_and_strahler_river_connectivity() {
    let raw = build_synthetic_raw_world_512();
    let (sim, bytes) = project_synthetic_v3(&raw);

    assert_eq!(sim.width, ANMW_V3_SIM_SIZE);
    assert_eq!(sim.height, ANMW_V3_SIM_SIZE);
    assert_eq!(sim.flags, ANMW_V3_FLAGS_STANDARD);
    assert_eq!(
        AnmwV3Sim::peek_seed(&bytes).expect("peek v3 seed"),
        derive_sim_seed_u32(raw.seed_u64)
    );
    assert!(
        raw.order4_cells.iter().all(|&(cx, cy)| {
            let ord = raw.strahler[cy * raw.size + cx];
            ord >= 4
        }),
        "synthetic main stem must have Strahler order >= 4"
    );

    let decoded = AnmwV3Sim::from_bytes(&bytes).expect("decode ANMW v3");
    let (tm, wid) = terrain_map_from_v3_sim(&decoded, &bytes).expect("map ANMW v3 to TerrainMap");
    assert_eq!(tm.width, 256);
    assert_eq!(tm.height, 256);
    assert_eq!(wid.seed, sim.sim_seed_u32);
    assert_eq!(
        wid.sim_world_sha256,
        Some(compute_sim_world_identity(&bytes))
    );

    assert!(tm.biomes.iter().all(|&b| b <= 10));

    let block = raw.size / 256;
    let mut max_elev_diff = 0.0f64;
    let mut ocean_cells = 0usize;
    let mut deep_ocean_cells = 0usize;
    for by in 0..256 {
        for bx in 0..256 {
            let mut sum_norm = 0.0f64;
            let mut sum_m = 0.0f64;
            let mut ocean_raw = 0usize;
            let mut lake_raw = 0usize;
            for dy in 0..block {
                for dx in 0..block {
                    let sidx = (by * block + dy) * raw.size + (bx * block + dx);
                    let m = raw.elev_m[sidx];
                    sum_m += m as f64;
                    sum_norm += normalize_elevation(m) as f64;
                    let wk = raw.water_kind[sidx];
                    if wk == WaterKind::Ocean as u8 {
                        ocean_raw += 1;
                    } else if wk == WaterKind::FreshwaterLake as u8
                        || wk == WaterKind::EndorheicLake as u8
                    {
                        lake_raw += 1;
                    }
                }
            }
            let block_n = (block * block) as f64;
            let expected_norm = sum_norm / block_n;
            let ti = by * 256 + bx;
            let diff = (tm.elevations[ti] as f64 - expected_norm).abs();
            max_elev_diff = max_elev_diff.max(diff);

            let block_cells = block * block;
            let expected_is_water_body =
                (ocean_raw * 2 >= block_cells) || (lake_raw * 2 >= block_cells);
            let actual_is_ocean = tm.biomes[ti] == BiomeType::Ocean as u8
                || tm.biomes[ti] == BiomeType::DeepOcean as u8;
            assert_eq!(
                actual_is_ocean, expected_is_water_body,
                "GS-1 water mask mismatch at ({bx}, {by}): biome={} ocean_raw={ocean_raw} lake_raw={lake_raw}",
                tm.biomes[ti]
            );
            if ocean_raw * 2 >= block_cells && (sum_m / block_n) < DEEP_OCEAN_THRESHOLD_M as f64 {
                assert_eq!(tm.biomes[ti], BiomeType::DeepOcean as u8);
                deep_ocean_cells += 1;
            } else if actual_is_ocean {
                assert_eq!(tm.biomes[ti], BiomeType::Ocean as u8);
                ocean_cells += 1;
            }
        }
    }
    assert!(deep_ocean_cells > 100 && ocean_cells > 100);
    assert!(
        max_elev_diff <= 1e-5,
        "GS-1 normalized elevation parity failed: max_diff={max_elev_diff}"
    );

    let mut river_targets: Vec<(usize, usize)> = raw
        .order4_cells
        .iter()
        .map(|&(sx, sy)| (sx / block, sy / block))
        .collect();
    river_targets.dedup();
    for &(tx, ty) in &river_targets {
        assert_eq!(
            tm.biomes[ty * 256 + tx],
            BiomeType::River as u8,
            "GS-1 Strahler>=4 block ({tx}, {ty}) lost River biome"
        );
    }
    let start = river_targets[0];
    let goal = *river_targets.last().unwrap();
    let mut visited = vec![false; 256 * 256];
    let mut queue = VecDeque::new();
    visited[start.1 * 256 + start.0] = true;
    queue.push_back(start);
    let mut reached_goal = false;
    while let Some((cx, cy)) = queue.pop_front() {
        if (cx, cy) == goal {
            reached_goal = true;
            break;
        }
        for dy in -1isize..=1 {
            for dx in -1isize..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = cx as isize + dx;
                let ny = cy as isize + dy;
                if (0..256).contains(&nx) && (0..256).contains(&ny) {
                    let (ux, uy) = (nx as usize, ny as usize);
                    let idx = uy * 256 + ux;
                    if !visited[idx] && tm.biomes[idx] == BiomeType::River as u8 {
                        visited[idx] = true;
                        queue.push_back((ux, uy));
                    }
                }
            }
        }
    }
    assert!(
        reached_goal,
        "GS-1 Strahler>=4 river path disconnected on 256^2 between {start:?} and {goal:?}"
    );
    println!(
        "GS-1 PASS max_elev_diff={max_elev_diff:.3e} deep_ocean={deep_ocean_cells} ocean={ocean_cells} order4_blocks={}",
        river_targets.len()
    );
}

// ---- GS-2: LAND1 Walkability, Connectivity & 200-Agent 1000-Tick Run ----------------------------

#[test]
fn gs2_land1_walkability_connectivity_and_200_agent_1000_tick_run() {
    for b in 0u8..=10 {
        assert_eq!(is_biome_walkable(b), b >= 2);
        assert_eq!(is_water_biome(b), b <= 1 || b == 3);
    }

    let raw = build_synthetic_raw_world_512();
    let (_sim, bytes) = project_synthetic_v3(&raw);
    let decoded = AnmwV3Sim::from_bytes(&bytes).expect("decode v3");
    let (tm, wid) = terrain_map_from_v3_sim(&decoded, &bytes).expect("map v3");

    let total_cells = tm.width * tm.height;
    let mut visited = vec![false; total_cells];
    let mut walkable_total = 0usize;
    let mut largest_comp = 0usize;
    let mut shoreline_cells = 0usize;
    for r in 0..tm.height {
        for c in 0..tm.width {
            let idx = r * tm.width + c;
            if is_biome_walkable(tm.biomes[idx]) {
                walkable_total += 1;
                if is_walkable_cell_next_to_water(&tm, c, r) {
                    shoreline_cells += 1;
                }
                if !visited[idx] {
                    let mut comp = 0usize;
                    let mut q = VecDeque::new();
                    visited[idx] = true;
                    q.push_back((c, r));
                    while let Some((cx, cy)) = q.pop_front() {
                        comp += 1;
                        for (dc, dr) in [(-1isize, 0), (1, 0), (0, -1), (0, 1)] {
                            let nc = cx as isize + dc;
                            let nr = cy as isize + dr;
                            if (0..tm.width as isize).contains(&nc)
                                && (0..tm.height as isize).contains(&nr)
                            {
                                let ni = nr as usize * tm.width + nc as usize;
                                if !visited[ni] && is_biome_walkable(tm.biomes[ni]) {
                                    visited[ni] = true;
                                    q.push_back((nc as usize, nr as usize));
                                }
                            }
                        }
                    }
                    largest_comp = largest_comp.max(comp);
                }
            }
        }
    }
    let comp_frac = largest_comp as f64 / walkable_total.max(1) as f64;
    assert!(
        comp_frac >= 0.80,
        "GS-2 largest walkable component too small: {largest_comp}/{walkable_total} ({comp_frac:.3})"
    );
    assert!(
        shoreline_cells >= 500,
        "GS-2 insufficient walkable shoreline cells: {shoreline_cells} < 500"
    );

    let pairs = [
        ("live.founders", 200.0),
        ("live.founder_layout", 1.0),
        ("live.predator_fraction", 0.2),
        ("live.land_mask", 1.0),
        ("live.locomotion", 1.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
        ("live.food_cap", 80.0),
        ("live.trees", 12.0),
        ("live.lakes", 6.0),
    ];
    let mut harness = build_world_v2_harness(tm.clone(), wid, &pairs, 202_611);
    let bounds = *harness.world.resource::<MapBounds>();

    let check_all_agents_walkable = |w: &mut World, tick: u64| {
        let map = w.resource::<TerrainMap>().clone();
        let b = *w.resource::<MapBounds>();
        let mut q = w.query_filtered::<(Entity, &Position), With<Agent>>();
        let mut count = 0usize;
        for (entity, pos) in q.iter(w) {
            count += 1;
            let wrapped = wrap_pos(pos.0, &b);
            let (col, row) = map
                .get_map_indices(wrapped, &b)
                .expect("agent inside MapBounds");
            let biome = map.biomes[row * map.width + col];
            assert!(
                is_biome_walkable(biome),
                "GS-2 tick {tick}: agent {entity:?} landed on non-walkable {:?} at {:?}",
                biome_from_u8(biome),
                pos.0
            );
        }
        count
    };

    let init_agents = check_all_agents_walkable(&mut harness.world, 0);
    assert_eq!(init_agents, 200, "GS-2 expected 200 founding agents");

    for tick in 1..=1_000 {
        harness.step_once();
        let alive = check_all_agents_walkable(&mut harness.world, tick);
        assert!(alive > 0, "GS-2 all agents died by tick {tick}");
    }

    // Negative control (Card 4c): Ocean position fails is_pos_walkable and is reverted.
    let ocean_pos = (0..tm.height * tm.width)
        .find_map(|idx| {
            let b = tm.biomes[idx];
            if b == BiomeType::Ocean as u8 || b == BiomeType::DeepOcean as u8 {
                Some(cell_center(&tm, &bounds, idx % tm.width, idx / tm.width))
            } else {
                None
            }
        })
        .expect("World v2 map contains Ocean");
    assert!(
        !is_pos_walkable(&tm, &bounds, ocean_pos),
        "GS-2 negative control: Ocean cell must fail is_pos_walkable"
    );

    let root = {
        let mut q = harness.world.query_filtered::<Entity, With<Agent>>();
        q.iter(&harness.world).next().expect("living agent")
    };
    let expected_safe = harness
        .world
        .get::<LastWalkablePosition>(root)
        .expect("LastWalkablePosition")
        .pos;
    harness.world.get_mut::<Position>(root).unwrap().0 = ocean_pos;
    harness.world.get_mut::<Velocity>(root).unwrap().0 = Vec3::new(1.5, 0.0, 0.0);
    harness.world.get_mut::<RigidBody>(root).unwrap().velocity = Vec3::new(1.5, 0.0, 0.0);
    if let Some((child, _)) = harness
        .world
        .query_filtered::<(Entity, &ParentAgent), Without<Agent>>()
        .iter(&harness.world)
        .find(|(_, p)| p.0 == root)
    {
        harness.world.get_mut::<Position>(child).unwrap().0 += ocean_pos - expected_safe;
    }

    let mut nav_only = Schedule::default();
    nav_only.set_executor_kind(ExecutorKind::SingleThreaded);
    nav_only.add_systems(terrain_navigation_system);
    nav_only.run(&mut harness.world);

    let reverted = harness.world.get::<Position>(root).unwrap().0;
    assert!(
        reverted.distance(expected_safe) < 1e-4,
        "GS-2 negative control: Ocean agent not reverted ({reverted:?} != {expected_safe:?})"
    );
    println!(
        "GS-2 PASS walkable_comp={largest_comp}/{walkable_total} ({comp_frac:.3}) shoreline={shoreline_cells} agents_1000=ok"
    );
}

// ---- GS-3: Closed-EU Ledger & Realistic ResourceField -------------------------------------------

#[test]
fn gs3_closed_eu_ledger_and_resource_field_on_v3_sim() {
    let raw = build_synthetic_raw_world_512();
    let (_sim, bytes) = project_synthetic_v3(&raw);
    let decoded = AnmwV3Sim::from_bytes(&bytes).expect("decode v3");
    let (tm_v2, wid_v2) = terrain_map_from_v3_sim(&decoded, &bytes).expect("map v3");

    let v1_world = init_world_with(TerrainSource::Generated);
    let v1_plants = v1_world.resource::<ResourceField>().total_biomass();
    let bounds = *v1_world.resource::<MapBounds>();
    let v2_field = ResourceField::from_biomes(
        &tm_v2.biomes,
        tm_v2.width,
        tm_v2.height,
        bounds.min.x,
        bounds.min.z,
        bounds.max.x,
        bounds.max.z,
        0.02,
    );
    let v2_plants = v2_field.total_biomass();
    let cap_ratio = v2_plants / v1_plants.max(1e-6);
    assert!(
        (0.4..=2.0).contains(&cap_ratio),
        "GS-3 World v2 capacity {v2_plants:.2} out of [0.4x, 2.0x] of World v1 {v1_plants:.2} (ratio={cap_ratio:.3})"
    );

    let pairs = [
        ("live.founders", 30.0),
        ("live.founder_layout", 1.0),
        ("live.predator_fraction", 0.2),
        ("live.land_mask", 1.0),
        ("live.locomotion", 1.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
        ("live.food_cap", 50.0),
        ("live.trees", 8.0),
        ("live.lakes", 6.0),
    ];
    let mut harness = build_world_v2_harness(tm_v2, wid_v2, &pairs, 303_611);

    let authoritative_eu = |w: &mut World| -> f64 {
        let plants = w.resource::<ResourceField>().total_biomass();
        let pool = *w.resource::<EcosystemBiomass>();
        let mut q = w.query_filtered::<&HomeostaticState, With<Agent>>();
        let animals: f64 = q
            .iter(w)
            .map(|h| {
                assert!(h.energy.is_finite(), "non-finite agent energy");
                h.energy as f64
            })
            .sum();
        assert!(
            plants.is_finite() && pool.detritus.is_finite() && pool.dormant.is_finite(),
            "non-finite ecosystem biomass"
        );
        closed_total_eu(plants, animals, pool.detritus, pool.dormant)
    };

    harness.step_once();
    let start_eu = authoritative_eu(&mut harness.world);
    let mut worst_rel_drift = 0.0f64;

    for _tick in 2..=1_000 {
        harness.step_once();
        let now_eu = authoritative_eu(&mut harness.world);
        let rel = (now_eu - start_eu).abs() / start_eu.abs().max(1.0);
        worst_rel_drift = worst_rel_drift.max(rel);
    }

    let rf = harness.world.resource::<ResourceField>();
    assert!(rf.r.iter().all(|v| v.is_finite()));
    let ledger = *harness.world.resource::<EnergyLedger>();
    assert!(
        ledger.settled_count() > 0,
        "GS-3 expected active EU ledger settlements over 1,000 ticks"
    );
    assert!(
        worst_rel_drift < 1e-6,
        "GS-3 closed-EU relative drift {worst_rel_drift:.3e} >= 1e-6"
    );
    println!(
        "GS-3 PASS v1_plants={v1_plants:.1} v2_plants={v2_plants:.1} ratio={cap_ratio:.3} worst_rel_drift={worst_rel_drift:.3e} settled={}",
        ledger.settled_count()
    );
}

// ---- GS-4: Zero-Allocation Tick Path on World v2 ------------------------------------------------

#[test]
fn gs4_zero_allocation_tick_path_on_world_v2() {
    let raw = build_synthetic_raw_world_512();
    let (_sim, bytes) = project_synthetic_v3(&raw);
    let decoded = AnmwV3Sim::from_bytes(&bytes).expect("decode v3");
    let (tm_v2, wid_v2) = terrain_map_from_v3_sim(&decoded, &bytes).expect("map v3");

    let quiescent_pairs = [
        ("live.founders", 20.0),
        ("live.founder_layout", 1.0),
        ("live.predator_fraction", 0.0),
        ("live.land_mask", 1.0),
        ("live.locomotion", 1.0),
        ("live.mortality", 0.0),
        ("live.reproduction", 0.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
        ("live.food_cap", 0.0),
        ("live.trees", 0.0),
        ("live.lakes", 4.0),
    ];
    let mut harness = build_world_v2_harness(tm_v2.clone(), wid_v2, &quiescent_pairs, 404_611);
    for _ in 0..60 {
        harness.step_once();
    }
    harness.drain_queues();

    // 1. World v2 terrain/physics/navigation/resource hot-path schedule in exact
    // `build_tick_schedule` order must allocate 0 times once warm (Note: full 52-system
    // `build_tick_schedule` in trunk has unrelated telemetry allocators such as
    // `demographic_census_system` / `build_demographic_census`).
    let mut hot_schedule = Schedule::default();
    hot_schedule.set_executor_kind(ExecutorKind::SingleThreaded);
    hot_schedule.add_systems(
        (
            steering_drive_system,
            integrate_physics_system,
            terrain_navigation_system,
            wrap_coordinates_system,
            herbivore_grazing_system,
            resource_field_regrowth_system,
            ecosystem_census_system,
        )
            .chain(),
    );
    for _ in 0..10 {
        hot_schedule.run(&mut harness.world);
    }

    ALLOCATOR.start_tracking();
    for _ in 0..200 {
        hot_schedule.run(&mut harness.world);
    }
    let hot_allocs = ALLOCATOR.stop_tracking();
    assert_eq!(
        hot_allocs, 0,
        "GS-4 World v2 terrain/navigation/resource schedule allocated {hot_allocs} times"
    );

    // 2. World v2 shoreline ocean-reversion + child-segment shift path allocates 0 times.
    let bounds = *harness.world.resource::<MapBounds>();
    let ocean_pos = (0..tm_v2.height * tm_v2.width)
        .find_map(|idx| {
            let b = tm_v2.biomes[idx];
            if b == BiomeType::Ocean as u8 || b == BiomeType::DeepOcean as u8 {
                Some(cell_center(
                    &tm_v2,
                    &bounds,
                    idx % tm_v2.width,
                    idx / tm_v2.width,
                ))
            } else {
                None
            }
        })
        .expect("World v2 map has ocean");

    let root = {
        let mut q = harness.world.query_filtered::<Entity, With<Agent>>();
        q.iter(&harness.world).next().expect("agent exists")
    };
    let child = harness
        .world
        .query_filtered::<(Entity, &ParentAgent), Without<Agent>>()
        .iter(&harness.world)
        .find(|(_, p)| p.0 == root)
        .map(|(e, _)| e)
        .expect("child segment exists");

    let mut nav_only = Schedule::default();
    nav_only.set_executor_kind(ExecutorKind::SingleThreaded);
    nav_only.add_systems(terrain_navigation_system);
    let push_into_ocean = |w: &mut World| {
        let last_pos = w.get::<LastWalkablePosition>(root).unwrap().pos;
        let delta = ocean_pos - last_pos;
        w.get_mut::<Position>(root).unwrap().0 = ocean_pos;
        w.get_mut::<Velocity>(root).unwrap().0 = Vec3::new(1.0, 0.0, 0.0);
        w.get_mut::<RigidBody>(root).unwrap().velocity = Vec3::new(1.0, 0.0, 0.0);
        w.get_mut::<Position>(child).unwrap().0 += delta;
    };

    push_into_ocean(&mut harness.world);
    nav_only.run(&mut harness.world);

    ALLOCATOR.start_tracking();
    for _ in 0..50 {
        push_into_ocean(&mut harness.world);
        nav_only.run(&mut harness.world);
    }
    let revert_allocs = ALLOCATOR.stop_tracking();
    assert_eq!(
        revert_allocs, 0,
        "GS-4 World v2 shoreline ocean reversion allocated {revert_allocs} times"
    );

    // 3. Non-blind allocator positive control.
    ALLOCATOR.start_tracking();
    std::hint::black_box(Vec::<u8>::with_capacity(64));
    let control_allocs = ALLOCATOR.stop_tracking();
    assert!(
        control_allocs >= 1,
        "GS-4 ThreadTrackingAllocator positive control saw 0 allocations"
    );

    println!(
        "GS-4 PASS hot_schedule_allocs={hot_allocs} revert_allocs={revert_allocs} control_allocs={control_allocs}"
    );
}

// ---- GS-5: Tamper Rejection, Loud Pre-Switchover Refusal & Paired E-SEL1 / SEL2-P Isolation ----

fn load_fixture_manifest(rel_path: &str, max_ticks: u64) -> ExperimentManifest {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel_path);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing fixture {}: {e}", path.display()));
    let mut m: ExperimentManifest =
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    m.duration_ticks = max_ticks;
    m.seeds.truncate(1);
    m
}

fn set_temp_root(dir: &std::path::Path) {
    std::env::set_var("TMPDIR", dir);
    std::env::set_var("TEMP", dir);
    std::env::set_var("TMP", dir);
}

#[test]
fn gs5_paired_run_isolation_identity_rejection_and_loud_v3_refusal() {
    let raw = build_synthetic_raw_world_512();
    let (_sim, v3_bytes) = project_synthetic_v3(&raw);

    let sim_id = compute_sim_world_identity(&v3_bytes);
    verify_sim_world_identity(&v3_bytes, &sim_id).expect("untampered sim_256 verifies");
    let mut bad_sim = v3_bytes.clone();
    let mid = bad_sim.len() / 2;
    bad_sim[mid] ^= 0x01;
    assert!(
        verify_sim_world_identity(&bad_sim, &sim_id).is_err(),
        "GS-5: 1-byte tamper in sim_256.anmw3 must fail verification"
    );

    let index_json = br#"{"schema_version":1,"sim_world_identity":"00"}"#;
    let bundle_id = compute_bundle_identity(index_json);
    assert_ne!(sim_id, bundle_id);
    verify_bundle_identity(index_json, &bundle_id).expect("untampered index.json verifies");
    let mut bad_index = index_json.to_vec();
    bad_index[10] ^= 0x01;
    assert!(
        verify_bundle_identity(&bad_index, &bundle_id).is_err(),
        "GS-5: 1-byte tamper in index.json must fail verification"
    );

    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _guard = EnvGuard::capture();

    let scratch = std::env::temp_dir().join(format!("anima_wv2_3_gs5_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let clean_tmp = scratch.join("clean_tmp");
    let cache_dir = scratch.join("cache");
    let v3_tmp = scratch.join("v3_tmp");
    std::fs::create_dir_all(&clean_tmp).expect("mkdir clean_tmp");
    std::fs::create_dir_all(&cache_dir).expect("mkdir cache");
    std::fs::create_dir_all(&v3_tmp).expect("mkdir v3_tmp");

    let sim_disk_path = scratch.join("sim_256.anmw3");
    std::fs::write(&sim_disk_path, &v3_bytes).expect("write v3 artifact");
    let v3_default_tmp_path = v3_tmp.join("anima_world_artifact.anmw");
    std::fs::write(&v3_default_tmp_path, &v3_bytes).expect("write v3 at default temp path");

    std::env::set_var("ANIMA_CACHE_DIR", &cache_dir);
    std::env::set_var("ANIMA_WORLD_ARTIFACT", &sim_disk_path);
    set_temp_root(&clean_tmp);

    let err_seed = try_world_seed_from_disk().expect_err("try_world_seed_from_disk refuses v3");
    assert!(err_seed.contains("ANMW v3"), "unexpected err: {err_seed}");
    assert!(
        std::panic::catch_unwind(world_seed_from_disk).is_err(),
        "GS-5 negative control: world_seed_from_disk must panic loudly on ANMW v3"
    );
    assert!(
        std::panic::catch_unwind(init_world).is_err(),
        "GS-5 negative control: init_world must panic loudly on ANMW v3"
    );

    // Positive control: legacy v2 fixture continues decoding cleanly in both entry points.
    let v2_fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/world_real_128.anmw");
    let v2_bytes = std::fs::read(&v2_fixture).expect("read world_real_128.anmw");
    let v2_expected_seed = WorldArtifact::peek_seed(&v2_bytes).expect("peek v2 seed");
    std::env::set_var("ANIMA_WORLD_ARTIFACT", &v2_fixture);
    assert_eq!(
        try_world_seed_from_disk().expect("v2 try seed"),
        v2_expected_seed
    );
    assert_eq!(world_seed_from_disk(), v2_expected_seed);
    let v2_world = init_world();
    assert_eq!(v2_world.resource::<WorldIdentity>().seed, v2_expected_seed);
    assert_eq!(v2_world.resource::<WorldIdentity>().sim_world_sha256, None);

    let esel1_manifest = load_fixture_manifest(
        "tests/fixtures/experiments_esel1/esel1-smoke-scarce.json",
        240,
    );
    let sel2p_manifest = load_fixture_manifest(
        "tests/fixtures/experiments_sel2p/sel2p-smoke-selected.json",
        240,
    );

    let reg = anima_engine_lib::core::experiment::ObservableRegistry::live_default();

    std::env::remove_var("ANIMA_WORLD_ARTIFACT");
    set_temp_root(&clean_tmp);
    assert!(!default_artifact_path().exists());
    let esel1_clean = run_manifest_seed::<LiveExperimentAdapter>(
        &esel1_manifest,
        &reg,
        esel1_manifest.seeds[0],
        None,
        None,
    );
    let sel2p_clean = run_manifest_seed::<LiveExperimentAdapter>(
        &sel2p_manifest,
        &reg,
        sel2p_manifest.seeds[0],
        None,
        None,
    );

    std::env::set_var("ANIMA_WORLD_ARTIFACT", &sim_disk_path);
    set_temp_root(&v3_tmp);
    let esel1_with_v3 = run_manifest_seed::<LiveExperimentAdapter>(
        &esel1_manifest,
        &reg,
        esel1_manifest.seeds[0],
        None,
        None,
    );
    let sel2p_with_v3 = run_manifest_seed::<LiveExperimentAdapter>(
        &sel2p_manifest,
        &reg,
        sel2p_manifest.seeds[0],
        None,
        None,
    );

    for (label, a, b) in [
        ("E-SEL1", &esel1_clean, &esel1_with_v3),
        ("SEL2-P", &sel2p_clean, &sel2p_with_v3),
    ] {
        assert!(
            a.status.is_completed(),
            "{label} clean failed: {:?}",
            a.status
        );
        assert!(
            b.status.is_completed(),
            "{label} v3-disk failed: {:?}",
            b.status
        );
        assert_eq!(a.provenance.terrain_source.as_deref(), Some("generated"));
        assert_eq!(b.provenance.terrain_source.as_deref(), Some("generated"));
        assert_eq!(a.provenance.world_identity, b.provenance.world_identity);
        assert_eq!(
            a.provenance.world_identity.and_then(|w| w.sim_world_sha256),
            None
        );
        assert_eq!(
            a.final_checksum, b.final_checksum,
            "{label} checksum drifted with v3 on disk"
        );
        assert_eq!(
            a.final_observables, b.final_observables,
            "{label} observables drifted with v3 on disk"
        );
        assert_eq!(a.series.len(), b.series.len());
        for (sa, sb) in a.series.iter().zip(&b.series) {
            assert_eq!(sa.tick, sb.tick);
            assert_eq!(sa.observables.len(), sb.observables.len());
            for ((ka, va), (kb, vb)) in sa.observables.iter().zip(&sb.observables) {
                assert_eq!(ka, kb);
                assert_eq!(
                    va.to_bits(),
                    vb.to_bits(),
                    "{label} bit drift on {ka} at tick {}",
                    sa.tick
                );
            }
        }
    }

    let _ = std::fs::remove_dir_all(&scratch);
    println!(
        "GS-5 PASS esel1_checksum={:#010x} sel2p_checksum={:#010x}",
        esel1_clean.final_checksum, sel2p_clean.final_checksum
    );
}
