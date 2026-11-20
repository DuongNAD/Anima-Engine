//! Card LAND1 (`live.land_mask`): biome walkability navigation, land-only spawning, and
//! desktop app shoreline lake placement.
//!
//! Covers every acceptance check:
//! - Key registration (`LIVE_KEY_LAND_MASK = "live.land_mask"`, `LIVE_KEYS.len() = 28`), exact
//!   boolean validation, and serde backward compatibility.
//! - (a) `absent` and `0.0` produce bit-identical sampled trajectories at every sampled tick.
//! - (b) With `live.land_mask = 1.0`, over 2,000 ticks no agent root is ever in `DeepOcean` or
//!   `Ocean`, and founders, food, and all trees present at genesis and tick 2,000 are on walkable
//!   cells; reproduction (`live.reproduction = 1.0`) keeps all roots walkable with `births > 0`;
//!   checkpoint/restore reproduces `LastWalkablePosition` and continues identically.
//! - (c) Zero heap allocations in `terrain_navigation_system` once warm (walkable & revert paths).
//! - (d) Closed EU conservation unchanged (`< 1e-6` relative drift across 2,000 ticks).
//! - (e) Identical checksum across 3 separate processes with `live.land_mask = 1.0`.
//! - (f) Desktop app genesis (`SimulationEngine::start::<tauri::test::MockRuntime>`) places lakes
//!   on walkable cells next to water and keeps founders/trees/food on walkable land.

mod common;

use anima_engine_lib::ai::cpg::TimeStep;
use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};
use anima_engine_lib::core::components::{
    Agent, Food, Lake, LastWalkablePosition, ParentAgent, Position, Tree, Velocity,
};
use anima_engine_lib::core::determinism::DeterministicMode;
use anima_engine_lib::core::ecs::init_world;
use anima_engine_lib::core::experiment::{ExperimentError, InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    live_observables, LiveExperimentAdapter, LiveSnapshot, LiveWorldConfig, LIVE_KEYS,
    LIVE_KEY_LAND_MASK,
};
use anima_engine_lib::core::resources::{
    EnvironmentalSpawnSettings, MapBounds, TerrainNavigation, VitalStatistics,
};
use anima_engine_lib::core::simulation_lifecycle::SimulationEngine;
use anima_engine_lib::core::simulation_schedule::build_tick_schedule;
use anima_engine_lib::core::snapshot::{self, SnapshotEnvelope};
use anima_engine_lib::core::terrain::{BiomeType, TerrainMap};
use anima_engine_lib::core::terrain_navigation::{
    cell_center, is_walkable_cell_next_to_water, terrain_navigation_system, wrap_pos,
};
use anima_engine_lib::core::world_artifact::WorldIdentity;
use anima_engine_lib::core::world_systems::wrap_coordinates_system;
use anima_engine_lib::evolution::genotype::{
    decode_genotype, MorphologyEdge, MorphologyGenotype, MorphologyNode,
};
use anima_engine_lib::physics::dynamics::RigidBody;
use anima_engine_lib::physics::spatial::IndexedInGrid;
use anima_engine_lib::physics::{integrate_physics_system, SpatialCollider};
use bevy_ecs::prelude::*;
use glam::Vec3;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[global_allocator]
static ALLOCATOR: common::allocator::ThreadTrackingAllocator =
    common::allocator::ThreadTrackingAllocator::new();

const CHILD_ROLE: &str = "ANIMA_LAND1_CHILD";
const MARKER: &str = "LAND1_CHECKSUM";
static DISK_SEQ: AtomicU64 = AtomicU64::new(0);

fn initial(pairs: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect())
}

fn biome_from_id(id: u8) -> BiomeType {
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
        other => panic!("unexpected biome id {other}"),
    }
}

/// Independent test-side walkability oracle (N2): `DeepOcean` and `Ocean` are not land; all
/// other biomes (`River`, `Beach`, and inland biomes) are walkable land.
fn is_land_biome(biome: BiomeType) -> bool {
    !matches!(biome, BiomeType::DeepOcean | BiomeType::Ocean)
}

fn cell_biome(tm: &TerrainMap, bounds: &MapBounds, pos: Vec3) -> BiomeType {
    let wrapped = wrap_pos(pos, bounds);
    let (col, row) = tm
        .get_map_indices(wrapped, bounds)
        .expect("position must lie within MapBounds");
    biome_from_id(tm.biomes[row * tm.width + col])
}

fn is_land_pos(tm: &TerrainMap, bounds: &MapBounds, pos: Vec3) -> bool {
    is_land_biome(cell_biome(tm, bounds, pos))
}

fn assert_all_roots_walkable(world: &mut World, context: &str) {
    let bounds = *world.resource::<MapBounds>();
    let tm = world.resource::<TerrainMap>().clone();
    let mut q = world.query_filtered::<(Entity, &Position), With<Agent>>();
    for (entity, pos) in q.iter(world) {
        let biome = cell_biome(&tm, &bounds, pos.0);
        assert!(
            is_land_biome(biome),
            "{context}: agent root {entity:?} at {:?} landed on non-walkable {biome:?}",
            pos.0
        );
    }
}

fn assert_all_trees_and_food_walkable(world: &mut World, check_trees: bool, context: &str) {
    let bounds = *world.resource::<MapBounds>();
    let tm = world.resource::<TerrainMap>().clone();
    let mut food_q = world.query_filtered::<&Position, With<Food>>();
    for pos in food_q.iter(world) {
        let biome = cell_biome(&tm, &bounds, pos.0);
        assert!(
            is_land_biome(biome),
            "{context}: food at {:?} spawned on non-walkable {biome:?}",
            pos.0
        );
    }
    if check_trees {
        let mut tree_q = world.query_filtered::<&Position, With<Tree>>();
        for pos in tree_q.iter(world) {
            let biome = cell_biome(&tm, &bounds, pos.0);
            assert!(
                is_land_biome(biome),
                "{context}: tree at {:?} is on non-walkable {biome:?}",
                pos.0
            );
        }
    }
}

// ---- Key registration and validation -----------------------------------------------------------

#[test]
fn key_is_registered_validated_and_serde_backward_compatible() {
    assert_eq!(LIVE_KEY_LAND_MASK, "live.land_mask");
    assert_eq!(LIVE_KEYS.len(), 28);
    assert!(LIVE_KEYS.contains(&LIVE_KEY_LAND_MASK));
    assert!(!LiveWorldConfig::default().land_mask);

    let cfg_absent =
        LiveWorldConfig::from_initial_conditions(&initial(&[])).expect("empty manifest builds");
    assert!(!cfg_absent.land_mask);

    let cfg_zero = LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_LAND_MASK, 0.0)]))
        .expect("0.0 builds");
    assert!(!cfg_zero.land_mask);

    let cfg_one = LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_LAND_MASK, 1.0)]))
        .expect("1.0 builds");
    assert!(cfg_one.land_mask);

    for bad in [-1.0, 0.5, 2.0] {
        let err = LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_LAND_MASK, bad)]))
            .expect_err("non-boolean land_mask must be refused");
        assert!(
            matches!(err, ExperimentError::OutOfRange { .. }),
            "expected OutOfRange for {bad}, got {err:?}"
        );
    }
    for non_finite in [f64::NAN, f64::INFINITY] {
        let err =
            LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_LAND_MASK, non_finite)]))
                .expect_err("non-finite land_mask must be refused");
        assert!(
            matches!(err, ExperimentError::NotFinite { .. }),
            "expected NotFinite for {non_finite}, got {err:?}"
        );
    }

    let mut val = serde_json::to_value(LiveWorldConfig::default()).expect("serialize");
    val.as_object_mut()
        .expect("json object")
        .remove("land_mask");
    let decoded: LiveWorldConfig = serde_json::from_value(val).expect("backward compatible serde");
    assert!(!decoded.land_mask);
}

// ---- (a) Absent and 0.0 are bit-identical at every sampled tick --------------------------------

fn sample_trajectory(
    label: Option<&str>,
    init: &InitialConditionSet,
    seed: u64,
    ticks: u64,
) -> Vec<(u64, u32)> {
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        init,
        &[],
        seed,
        (16, 16),
        ticks,
    )
    .expect("live world builds");
    let mut out = Vec::new();
    for tick in 1..=ticks {
        a.run_schedule_once();
        if tick % 100 == 0 {
            let sum = a.checksum();
            if let Some(tag) = label {
                println!("LAND1_TRAJ world={tag} tick={tick} checksum={sum:#010x}");
            }
            out.push((tick, sum));
        }
    }
    out
}

#[test]
fn absent_and_zero_match_at_every_sampled_tick() {
    let base_a: [(&str, f64); 0] = [];
    let base_b = [
        ("live.founders", 20.0),
        ("live.predator_fraction", 0.25),
        ("live.trees", 8.0),
        ("live.lakes", 4.0),
        ("live.food_cap", 50.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
        ("live.locomotion", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
    ];

    let a_absent = sample_trajectory(Some("default"), &initial(&base_a), 1, 1_000);
    let a_zero = sample_trajectory(None, &initial(&[(LIVE_KEY_LAND_MASK, 0.0)]), 1, 1_000);
    assert_eq!(a_absent, a_zero, "default world: absent vs 0.0 diverged");

    let b_absent = sample_trajectory(Some("active"), &initial(&base_b), 7, 1_000);
    let mut b_zero_pairs = base_b.to_vec();
    b_zero_pairs.push((LIVE_KEY_LAND_MASK, 0.0));
    let b_zero = sample_trajectory(None, &initial(&b_zero_pairs), 7, 1_000);
    assert_eq!(b_absent, b_zero, "active world: absent vs 0.0 diverged");
}

// ---- (b) Over 2,000 ticks with key 1: roots, founders, trees, lakes, food stay on land ---------

fn land_masked_active_init(reproduction: bool) -> InitialConditionSet {
    let mut pairs = vec![
        ("live.founders", 20.0),
        ("live.predator_fraction", 0.2),
        ("live.trees", 8.0),
        ("live.lakes", 8.0),
        ("live.lake_layout", 1.0),
        ("live.food_cap", 50.0),
        ("live.mortality", 1.0),
        ("live.locomotion", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
        (LIVE_KEY_LAND_MASK, 1.0),
    ];
    if reproduction {
        pairs.push(("live.reproduction", 1.0));
    }
    initial(&pairs)
}

#[test]
fn land_mask_keeps_agents_founders_trees_lakes_and_food_on_land_over_2000_ticks() {
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &land_masked_active_init(false),
        &[],
        11,
        (16, 16),
        2_000,
    )
    .expect("land_mask world builds");

    // Tick 0: check founders, trees, lakes
    {
        let mut world = a.world();
        let nav = world
            .get_resource::<TerrainNavigation>()
            .expect("TerrainNavigation inserted");
        assert!(nav.enabled);

        assert_eq!(
            world
                .query_filtered::<Entity, With<Agent>>()
                .iter(&world)
                .count(),
            20
        );
        assert_all_roots_walkable(&mut world, "tick 0 founders");

        let bounds = *world.resource::<MapBounds>();
        let tm = world.resource::<TerrainMap>().clone();
        let lakes: Vec<Vec3> = world
            .query_filtered::<&Position, With<Lake>>()
            .iter(&world)
            .map(|p| p.0)
            .collect();
        assert_eq!(lakes.len(), 8);
        for pos in lakes {
            assert!(
                is_land_pos(&tm, &bounds, pos),
                "genesis lake at {pos:?} must be snapped onto walkable cell"
            );
        }

        let trees: Vec<Vec3> = world
            .query_filtered::<&Position, With<Tree>>()
            .iter(&world)
            .map(|p| p.0)
            .collect();
        assert_eq!(trees.len(), 8);
        for pos in trees {
            assert!(
                is_land_pos(&tm, &bounds, pos),
                "genesis tree at {pos:?} must be snapped onto walkable cell"
            );
        }
    }

    for tick in 1..=2_000 {
        a.run_schedule_once();
        let mut world = a.world();
        assert_all_roots_walkable(&mut world, &format!("tick {tick}"));
        assert_all_trees_and_food_walkable(
            &mut world,
            tick % 100 == 0 || tick == 2_000,
            &format!("tick {tick}"),
        );
    }

    // Non-vacuity and post-seed-drop tree check at tick 2,000
    let mut world = a.world();
    let agent_count = world
        .query_filtered::<Entity, With<Agent>>()
        .iter(&world)
        .count();
    let food_count = world
        .query_filtered::<Entity, With<Food>>()
        .iter(&world)
        .count();
    let tree_count = world
        .query_filtered::<Entity, With<Tree>>()
        .iter(&world)
        .count();
    assert!(agent_count >= 1, "agents must remain non-empty for check");
    assert!(food_count >= 1, "food must have spawned on walkable cells");
    assert!(tree_count >= 8, "trees must be present at tick 2,000");
}

#[test]
fn reproduction_with_land_mask_keeps_all_roots_on_land_and_records_births() {
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &land_masked_active_init(true),
        &[],
        17,
        (16, 16),
        2_000,
    )
    .expect("reproduction land_mask world builds");

    for tick in 1..=2_000 {
        a.run_schedule_once();
        let mut world = a.world();
        assert_all_roots_walkable(&mut world, &format!("repro tick {tick}"));
        if tick == 2_000 {
            assert_all_trees_and_food_walkable(&mut world, true, "repro tick 2000");
        }
    }

    let world = a.world();
    let vitals = world
        .get_resource::<VitalStatistics>()
        .expect("VitalStatistics present");
    assert!(
        vitals.total_births > 0,
        "reproduction run must record at least one birth (got total_births=0)"
    );
}

#[test]
fn restore_reproduces_last_walkable_position_and_matches_continuing_run() {
    let init = land_masked_active_init(true);
    let mut uninterrupted = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &init,
        &[],
        23,
        (16, 16),
        600,
    )
    .expect("uninterrupted builds");

    for _ in 0..300 {
        uninterrupted.run_schedule_once();
    }

    let expected_positions: Vec<([u32; 3], [u32; 3])> = {
        let mut w = uninterrupted.world();
        let mut pairs: Vec<([u32; 3], [u32; 3])> = w
            .query_filtered::<(&Position, &LastWalkablePosition), With<Agent>>()
            .iter(&w)
            .map(|(p, l)| {
                (
                    [p.0.x.to_bits(), p.0.y.to_bits(), p.0.z.to_bits()],
                    [l.pos.x.to_bits(), l.pos.y.to_bits(), l.pos.z.to_bits()],
                )
            })
            .collect();
        pairs.sort_unstable();
        pairs
    };
    assert!(!expected_positions.is_empty());

    let seq = DISK_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "anima_land1_restore_{}_{}",
        std::process::id(),
        seq
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("checkpoint.json");
    let snap = uninterrupted.snapshot();
    let envelope = SnapshotEnvelope::seal(snap.saved.clone()).expect("seal snapshot");
    snapshot::write_atomic(&path, &envelope).expect("write snapshot");
    let loaded = snapshot::read(&path).expect("read snapshot");
    let _ = std::fs::remove_dir_all(&dir);

    let mut resumed = LiveExperimentAdapter::from_snapshot(&LiveSnapshot {
        saved: loaded,
        config: snap.config,
        seed: snap.seed,
    })
    .expect("restore builds");

    let restored_positions: Vec<([u32; 3], [u32; 3])> = {
        let mut w = resumed.world();
        let mut pairs: Vec<([u32; 3], [u32; 3])> = w
            .query_filtered::<(&Position, &LastWalkablePosition), With<Agent>>()
            .iter(&w)
            .map(|(p, l)| {
                (
                    [p.0.x.to_bits(), p.0.y.to_bits(), p.0.z.to_bits()],
                    [l.pos.x.to_bits(), l.pos.y.to_bits(), l.pos.z.to_bits()],
                )
            })
            .collect();
        pairs.sort_unstable();
        pairs
    };
    assert_eq!(
        restored_positions, expected_positions,
        "restore must reproduce LastWalkablePosition cleanly"
    );

    assert_eq!(resumed.checksum(), uninterrupted.checksum());
    for t in 301..=600 {
        uninterrupted.run_schedule_once();
        resumed.run_schedule_once();
        assert_eq!(
            resumed.checksum(),
            uninterrupted.checksum(),
            "restored land_mask run diverged at tick {t}"
        );
    }
}

// ---- (c) Zero heap allocations once warm -------------------------------------------------------

#[test]
fn terrain_navigation_system_allocates_nothing_once_warm() {
    let init = land_masked_active_init(false);
    let mut a =
        LiveExperimentAdapter::from_manifest(&WorldLawSet::baseline(), &init, &[], 5, (16, 16), 0)
            .expect("builds");
    for _ in 0..50 {
        a.run_schedule_once();
    }

    let mut only = Schedule::default();
    only.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    only.add_systems(terrain_navigation_system);

    let mut world = a.world();
    for _ in 0..10 {
        only.run(&mut world);
    }

    ALLOCATOR.start_tracking();
    for _ in 0..200 {
        only.run(&mut world);
    }
    let steady_allocs = ALLOCATOR.stop_tracking();
    assert_eq!(
        steady_allocs, 0,
        "terrain_navigation_system allocated {steady_allocs} times on land"
    );

    // Also exercise the shoreline-revert + child-segment shift path under the allocation counter.
    let bounds = *world.resource::<MapBounds>();
    let ocean_pos = {
        let tm = world.resource::<TerrainMap>();
        let mut found = None;
        for row in 0..tm.height {
            for col in 0..tm.width {
                if !is_land_biome(biome_from_id(tm.biomes[row * tm.width + col])) {
                    found = Some(cell_center(tm, &bounds, col, row));
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        found.expect("default map has ocean cells")
    };

    let root = {
        let mut q = world.query_filtered::<Entity, With<Agent>>();
        q.iter(&world).next().expect("agent exists")
    };
    let child = {
        let mut q = world.query_filtered::<(Entity, &ParentAgent), Without<Agent>>();
        q.iter(&world)
            .find(|(_, p)| p.0 == root)
            .map(|(e, _)| e)
            .expect("child segment exists")
    };

    let push_into_ocean = |w: &mut World| {
        let last_pos = w.get::<LastWalkablePosition>(root).unwrap().pos;
        let delta = ocean_pos - last_pos;
        w.get_mut::<Position>(root).unwrap().0 = ocean_pos;
        w.get_mut::<Velocity>(root).unwrap().0 = Vec3::new(1.0, 0.0, 0.0);
        w.get_mut::<RigidBody>(root).unwrap().velocity = Vec3::new(1.0, 0.0, 0.0);
        w.get_mut::<Position>(child).unwrap().0 += delta;
    };

    push_into_ocean(&mut world);
    only.run(&mut world);

    ALLOCATOR.start_tracking();
    for _ in 0..50 {
        push_into_ocean(&mut world);
        only.run(&mut world);
    }
    let revert_allocs = ALLOCATOR.stop_tracking();
    assert_eq!(
        revert_allocs, 0,
        "terrain_navigation_system allocated {revert_allocs} times during ocean reversion"
    );
}

// ---- (d) Closed EU unchanged -------------------------------------------------------------------

#[test]
fn closed_eu_unchanged_with_land_mask_enabled() {
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &land_masked_active_init(true),
        &[],
        17,
        (16, 16),
        2_000,
    )
    .expect("EU run builds");

    let closed_eu = |a: &mut LiveExperimentAdapter| -> f64 {
        let mut w = a.world();
        let obs = live_observables(&mut w);
        let total = obs
            .iter()
            .find(|(k, _)| k == "live.closed_eu_total")
            .expect("closed_eu_total")
            .1;
        let mut q = w.query::<&HomeostaticState>();
        assert!(q.iter(&w).count() >= 1);
        total
    };

    a.run_schedule_once();
    let start = closed_eu(&mut a);
    let mut worst = 0.0_f64;
    for _ in 1..2_000 {
        a.run_schedule_once();
        worst = worst.max((closed_eu(&mut a) - start).abs() / start.abs().max(1.0));
    }
    println!("LAND1_EU start={start} worst_relative_drift={worst}");
    assert!(worst < 1e-6, "closed EU drifted by {worst} (relative)");
}

// ---- (e) Same checksum across 3 separate processes with key 1 ----------------------------------

#[test]
fn land1_child_entry_point() {
    if std::env::var(CHILD_ROLE).is_err() {
        return;
    }
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &land_masked_active_init(true),
        &[],
        29,
        (16, 16),
        600,
    )
    .expect("child builds");
    for _ in 0..600 {
        a.run_schedule_once();
    }
    println!("{MARKER} {:#010x}", a.checksum());
}

fn checksum_from_child() -> u32 {
    let output = Command::new(std::env::current_exe().expect("current_exe"))
        .args(["--exact", "land1_child_entry_point", "--nocapture"])
        .env(CHILD_ROLE, "1")
        .env("ANIMA_DETERMINISTIC", "1")
        .env("ANIMA_USE_GPU", "0")
        .output()
        .expect("spawn child");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|l| l.trim_start().starts_with(MARKER))
        .unwrap_or_else(|| {
            panic!(
                "child printed no checksum\n{stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
    let hex = line.trim().rsplit(' ').next().expect("token");
    u32::from_str_radix(hex.trim_start_matches("0x"), 16).expect("hex checksum")
}

#[test]
fn same_checksum_in_three_processes_with_land_mask_enabled() {
    let sums: Vec<u32> = (0..3).map(|_| checksum_from_child()).collect();
    println!(
        "LAND1_CHECKSUMS {}",
        sums.iter()
            .map(|s| format!("{s:#010x}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    assert!(
        sums.iter().all(|s| *s == sums[0]),
        "processes disagree with live.land_mask=1: {sums:x?}"
    );
    assert_ne!(sums[0], 0);
}

// ---- (f) Desktop app genesis places lakes on land next to water (MockRuntime) ------------------

#[test]
fn desktop_app_genesis_places_lakes_on_land_next_to_water() {
    let engine = SimulationEngine::new();
    let evo_settings = Arc::new(Mutex::new(EvolutionSettings {
        mutation_rate: 0.15,
        selection_bias: 1.5,
        grid_resolution: 50,
    }));
    let evo_running = Arc::new(AtomicBool::new(false));
    let map_elites_grid = Arc::new(Mutex::new(MapElitesGridState {
        grid: std::collections::HashMap::new(),
        grid_resolution: 50,
    }));

    engine.start::<tauri::test::MockRuntime>(None, evo_settings, evo_running, map_elites_grid);

    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let status = engine.get_status();
        if status.running && status.tick_count > 0 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "SimulationEngine failed to tick within 15s (status={status:?})"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    let (tx, rx) = std::sync::mpsc::channel();
    engine
        .save_request_tx
        .send(tx)
        .expect("sim thread accepts save request");
    let saved = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("sim thread answers save request")
        .expect("fresh app world saves");
    engine.stop();

    let ref_world = init_world();
    let ref_identity = *ref_world.resource::<WorldIdentity>();
    assert_eq!(
        saved.world_identity, ref_identity,
        "reference TerrainMap must match engine world_identity"
    );
    let tm = ref_world.resource::<TerrainMap>();

    assert!(
        !saved.lakes.is_empty(),
        "desktop app genesis must spawn at least one lake"
    );
    for lake in &saved.lakes {
        let (col, row) = tm
            .get_map_indices(lake.position, &saved.map_bounds)
            .expect("lake inside MapBounds");
        let biome = biome_from_id(tm.biomes[row * tm.width + col]);
        assert!(
            is_land_biome(biome),
            "app lake at {:?} placed on non-walkable biome {biome:?}",
            lake.position
        );
        assert!(
            is_walkable_cell_next_to_water(tm, col, row),
            "app lake at {:?} (cell ({col}, {row}), biome {biome:?}) is not adjacent to water",
            lake.position
        );
    }

    assert!(!saved.agents.is_empty(), "founders must be present");
    for agent in &saved.agents {
        assert!(
            is_land_pos(tm, &saved.map_bounds, agent.root_position),
            "app agent root at {:?} is not on walkable land",
            agent.root_position
        );
    }

    assert!(!saved.trees.is_empty(), "trees must be present");
    for tree in &saved.trees {
        assert!(
            is_land_pos(tm, &saved.map_bounds, tree.position),
            "app tree at {:?} is not on walkable land",
            tree.position
        );
    }

    for food in &saved.foods {
        assert!(
            is_land_pos(tm, &saved.map_bounds, food.position),
            "app food at {:?} is not on walkable land",
            food.position
        );
    }
}

#[test]
fn coastal_birth_and_staggered_evolution_use_walkable_positions_without_extra_rng_draws() {
    use anima_engine_lib::core::agent_systems::{
        apply_staggered_evolution_system, autonomous_reproduction_system, AgentEvaluation,
        AgentGeneration, AgentGenotype, AgentLineageId, NextNodeId,
    };
    use anima_engine_lib::core::components::{AgentParentLineageIds, FeatureTracker, Prey};
    use anima_engine_lib::core::resources::{
        EvolutionQueue, EvolutionReceiver, MutationPolicy, ReproductionPolicy, SimRng,
    };
    use anima_engine_lib::evolution::genotype::{
        decode_genotype, MorphologyGenotype, MorphologyNode,
    };

    let bounds = MapBounds {
        min: Vec3::new(-100.0, 0.0, -100.0),
        max: Vec3::new(100.0, 10.0, 100.0),
    };
    // 200x200 grid -> 1x1 world unit per cell. Only cell (100, 100) ([0,1) x [0,1)) is land;
    // all neighbors within [-1.5, 1.5] outside that 1x1 cell are DeepOcean.
    let mut tm = TerrainMap {
        width: 200,
        height: 200,
        elevations: vec![0.0; 40_000],
        moistures: vec![0.5; 40_000],
        temperatures: vec![0.5; 40_000],
        biomes: vec![BiomeType::DeepOcean as u8; 40_000],
        flows: vec![0.0; 40_000],
        pois: Vec::new(),
    };
    tm.biomes[100 * 200 + 100] = BiomeType::Grassland as u8;

    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.2,
        mass: 1.0,
    });

    // Pick a deterministic seed whose birth jitter pushes the newborn off the 1x1 island into DeepOcean.
    let build_repro_world = |land_enabled: bool, seed: u64| -> (World, Entity) {
        let mut world = init_world();
        world.insert_resource(bounds);
        world.insert_resource(tm.clone());
        world.insert_resource(SimRng::from_seed(seed));
        world.insert_resource(NextNodeId(10));
        world.insert_resource(ReproductionPolicy { enabled: true });
        world.insert_resource(MutationPolicy { rate: 0.0 });
        world.insert_resource(TerrainNavigation {
            enabled: land_enabled,
        });

        let parent_pos = Vec3::new(0.5, 0.0, 0.5);
        let parent = decode_genotype(&mut world, &genotype, parent_pos, glam::Quat::IDENTITY);
        world.entity_mut(parent).insert((
            Prey,
            AgentGenotype(genotype.clone()),
            AgentEvaluation {
                start_position: parent_pos,
                total_distance: 0.0,
                total_energy_expended: 0.0,
                total_energy_gained: 0.0,
                prey_energy_gained: 0.0,
                survival_ticks: 0,
                last_position: parent_pos,
            },
            FeatureTracker::default(),
            AgentLineageId("coast-parent".into()),
            AgentGeneration(0),
            AgentParentLineageIds(Vec::new()),
        ));

        let mut sched = Schedule::default();
        sched.add_systems((autonomous_reproduction_system, apply_deferred).chain());
        sched.run(&mut world);
        (world, parent)
    };

    // Find a seed whose unmasked jitter leaves the 1x1 island so the fallback branch is proven active.
    let mut active_seed = None;
    for candidate_seed in 1..64u64 {
        let (mut w_off, parent_off) = build_repro_world(false, candidate_seed);
        let mut q = w_off.query_filtered::<(Entity, &Position), With<Agent>>();
        if let Some((_, child_pos)) = q.iter(&w_off).find(|(e, _)| *e != parent_off) {
            if !is_land_pos(&tm, &bounds, child_pos.0) {
                active_seed = Some(candidate_seed);
                break;
            }
        }
    }
    let seed = active_seed.expect("found seed whose jitter enters DeepOcean");
    let (w_off, _) = build_repro_world(false, seed);
    let stream_off = w_off.resource::<SimRng>().stream_pos();

    let (mut w_on, parent_on) = build_repro_world(true, seed);
    let stream_on = w_on.resource::<SimRng>().stream_pos();
    assert_eq!(
        stream_on, stream_off,
        "coastal reproduction land check must consume zero extra RNG draws"
    );

    let mut q = w_on.query_filtered::<(Entity, &Position, &LastWalkablePosition), With<Agent>>();
    let children: Vec<_> = q
        .iter(&w_on)
        .filter(|(e, _, _)| *e != parent_on)
        .map(|(_, p, l)| (p.0, l.pos))
        .collect();
    assert_eq!(children.len(), 1);
    assert!(
        is_land_pos(&tm, &bounds, children[0].0),
        "newborn spawned at non-walkable position {:?}",
        children[0].0
    );
    assert_eq!(
        children[0].1, children[0].0,
        "newborn LastWalkablePosition must equal its walkable spawn position"
    );

    // Now test apply_staggered_evolution_system for both despawned ocean default_pos and live parent
    // whose LastWalkablePosition is on land.
    let mut evo_world = init_world();
    evo_world.insert_resource(bounds);
    evo_world.insert_resource(tm);
    evo_world.insert_resource(TerrainNavigation { enabled: true });
    let (_tx, rx) = crossbeam_channel::unbounded();
    evo_world.insert_resource(EvolutionReceiver(rx));

    let land_pos = Vec3::new(0.5, 0.0, 0.5);
    let ocean_pos = Vec3::new(-50.0, 0.0, -50.0);

    // Case 1: live old_entity temporarily at ocean_pos, but LastWalkablePosition at land_pos.
    let old_live = decode_genotype(&mut evo_world, &genotype, land_pos, glam::Quat::IDENTITY);
    evo_world.entity_mut(old_live).insert(Prey);
    evo_world.get_mut::<Position>(old_live).unwrap().0 = ocean_pos;

    // Case 2: dead placeholder Entity with ocean default_pos -> snaps onto land_pos.
    let dead_entity = evo_world.spawn_empty().id();
    evo_world.entity_mut(dead_entity).despawn();

    evo_world.insert_resource(EvolutionQueue {
        pending_replacements: vec![
            (
                dead_entity,
                genotype.clone(),
                ocean_pos,
                "evo-dead".into(),
                1,
                vec![],
            ),
            (
                old_live,
                genotype.clone(),
                ocean_pos,
                "evo-live".into(),
                1,
                vec![],
            ),
        ],
    });

    let mut evo_sched = Schedule::default();
    evo_sched.add_systems((apply_staggered_evolution_system, apply_deferred).chain());
    evo_sched.run(&mut evo_world);
    evo_sched.run(&mut evo_world);

    let mut rep_q = evo_world.query_filtered::<(&Position, &LastWalkablePosition), With<Agent>>();
    let spawned: Vec<(Vec3, Vec3)> = rep_q.iter(&evo_world).map(|(p, l)| (p.0, l.pos)).collect();
    assert_eq!(spawned.len(), 2);
    for (p, last) in spawned {
        let rep_bounds = evo_world.resource::<MapBounds>();
        let rep_tm = evo_world.resource::<TerrainMap>();
        assert!(
            is_land_pos(rep_tm, rep_bounds, p),
            "replacement spawned at non-walkable {p:?}"
        );
        assert_eq!(
            last, p,
            "replacement LastWalkablePosition must match walkable spawn"
        );
    }
}

// ---- N1: Policy-off (absent / enabled: false) follows pure physics and never snaps sea roots ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BodyBitSnapshot {
    root_pos: [u32; 3],
    root_vel: [u32; 3],
    root_body_vel: [u32; 3],
    last_walkable_pos: [u32; 3],
    child_pos: [u32; 3],
    child_vel: [u32; 3],
    child_body_vel: [u32; 3],
}

fn vec3_bits(v: Vec3) -> [u32; 3] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

fn snapshot_body(world: &World, root: Entity, child: Entity) -> BodyBitSnapshot {
    BodyBitSnapshot {
        root_pos: vec3_bits(world.get::<Position>(root).expect("root pos").0),
        root_vel: vec3_bits(world.get::<Velocity>(root).expect("root vel").0),
        root_body_vel: vec3_bits(world.get::<RigidBody>(root).expect("root body").velocity),
        last_walkable_pos: vec3_bits(
            world
                .get::<LastWalkablePosition>(root)
                .expect("last pos")
                .pos,
        ),
        child_pos: vec3_bits(world.get::<Position>(child).expect("child pos").0),
        child_vel: vec3_bits(world.get::<Velocity>(child).expect("child vel").0),
        child_body_vel: vec3_bits(world.get::<RigidBody>(child).expect("child body").velocity),
    }
}

fn build_small_sea_world(
    tm: &TerrainMap,
    bounds: MapBounds,
    genotype: &MorphologyGenotype,
    start_pos: Vec3,
    initial_vel: Vec3,
    policy: Option<bool>,
) -> (World, Entity, Entity) {
    let mut world = World::new();
    world.insert_resource(TimeStep(1.0 / 60.0));
    world.insert_resource(bounds);
    world.insert_resource(tm.clone());
    if let Some(enabled) = policy {
        world.insert_resource(TerrainNavigation { enabled });
    }

    let root = decode_genotype(&mut world, genotype, start_pos, glam::Quat::IDENTITY);
    let child = {
        let mut q = world.query_filtered::<(Entity, &ParentAgent), Without<Agent>>();
        q.iter(&world)
            .find(|(_, p)| p.0 == root)
            .map(|(e, _)| e)
            .expect("child segment exists")
    };

    for entity in [root, child] {
        world.get_mut::<Velocity>(entity).expect("vel").0 = initial_vel;
        world.get_mut::<RigidBody>(entity).expect("body").velocity = initial_vel;
    }
    (world, root, child)
}

#[test]
fn terrain_navigation_absent_and_disabled_leave_sea_agent_on_pure_physics() {
    let bounds = MapBounds {
        min: Vec3::new(-40.0, 0.0, -40.0),
        max: Vec3::new(40.0, 10.0, 40.0),
    };
    // 4x4 grid over [-40, 40]^2 (20x20 world units per cell):
    // col 0 = DeepOcean, col 1 = Ocean, cols 2..4 = Grassland.
    let mut tm = TerrainMap {
        width: 4,
        height: 4,
        elevations: vec![0.5; 16],
        moistures: vec![0.5; 16],
        temperatures: vec![0.5; 16],
        biomes: vec![BiomeType::Grassland as u8; 16],
        flows: vec![0.0; 16],
        pois: Vec::new(),
    };
    for row in 0..4 {
        tm.biomes[row * 4] = BiomeType::DeepOcean as u8;
        tm.biomes[row * 4 + 1] = BiomeType::Ocean as u8;
    }

    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.2,
        mass: 1.5,
    });
    genotype.add_node(MorphologyNode {
        id: 1,
        length: 1.0,
        radius: 0.2,
        mass: 1.0,
    });
    genotype.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 1,
        joint_anchor: Vec3::new(1.0, 0.0, 0.0),
        joint_axis: Vec3::new(0.0, 0.0, 1.0),
    });

    let initial_vel = Vec3::new(1.5, 0.0, 0.5);

    for (sea_biome, col, row) in [(BiomeType::Ocean, 1, 1), (BiomeType::DeepOcean, 0, 2)] {
        let start_pos = cell_center(&tm, &bounds, col, row);
        assert_eq!(cell_biome(&tm, &bounds, start_pos), sea_biome);
        assert!(!is_land_biome(sea_biome));

        // 1. Direct terrain_navigation_system: absent / enabled=false leave sea agent bit-unchanged,
        // while positive control (enabled=true) snaps both root and child to land and zeroes vel.
        let (mut w_abs, r_abs, c_abs) =
            build_small_sea_world(&tm, bounds, &genotype, start_pos, initial_vel, None);
        assert!(w_abs.get_resource::<TerrainNavigation>().is_none());
        let (mut w_dis, r_dis, c_dis) =
            build_small_sea_world(&tm, bounds, &genotype, start_pos, initial_vel, Some(false));
        let (mut w_on, r_on, c_on) =
            build_small_sea_world(&tm, bounds, &genotype, start_pos, initial_vel, Some(true));

        let initial_snap = snapshot_body(&w_abs, r_abs, c_abs);
        let initial_offset =
            w_abs.get::<Position>(c_abs).unwrap().0 - w_abs.get::<Position>(r_abs).unwrap().0;

        let nav_only = || {
            let mut s = Schedule::default();
            s.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
            s.add_systems(terrain_navigation_system);
            s
        };
        let mut s_abs = nav_only();
        let mut s_dis = nav_only();
        let mut s_on = nav_only();

        for tick in 1..=3 {
            s_abs.run(&mut w_abs);
            s_dis.run(&mut w_dis);
            s_on.run(&mut w_on);

            assert_eq!(
                snapshot_body(&w_abs, r_abs, c_abs),
                initial_snap,
                "{sea_biome:?} direct tick {tick}: absent TerrainNavigation mutated sea body"
            );
            assert_eq!(
                cell_biome(&tm, &bounds, w_abs.get::<Position>(r_abs).unwrap().0),
                sea_biome
            );

            assert_eq!(
                snapshot_body(&w_dis, r_dis, c_dis),
                initial_snap,
                "{sea_biome:?} direct tick {tick}: disabled TerrainNavigation mutated sea body"
            );
            assert_eq!(
                cell_biome(&tm, &bounds, w_dis.get::<Position>(r_dis).unwrap().0),
                sea_biome
            );

            let on_root_pos = w_on.get::<Position>(r_on).unwrap().0;
            let on_child_pos = w_on.get::<Position>(c_on).unwrap().0;
            assert!(
                is_land_biome(cell_biome(&tm, &bounds, on_root_pos)),
                "{sea_biome:?} positive control direct tick {tick} must snap root onto land"
            );
            assert_ne!(on_root_pos, start_pos);
            assert_eq!(
                w_on.get::<LastWalkablePosition>(r_on).unwrap().pos,
                on_root_pos
            );
            assert_eq!(w_on.get::<Velocity>(r_on).unwrap().0, Vec3::ZERO);
            assert_eq!(w_on.get::<RigidBody>(r_on).unwrap().velocity, Vec3::ZERO);
            assert_eq!(w_on.get::<Velocity>(c_on).unwrap().0, Vec3::ZERO);
            assert_eq!(w_on.get::<RigidBody>(c_on).unwrap().velocity, Vec3::ZERO);
            assert!(
                (on_child_pos - on_root_pos).distance(initial_offset) < 1e-5,
                "{sea_biome:?} positive control must preserve root-child offset"
            );
        }

        // 2. Multi-tick physics schedule: absent and disabled match pure physics bit-for-bit across
        // 5 ticks on the sea cell, while positive control (enabled=true) reverts onto walkable land.
        let (mut w_phys, r_phys, c_phys) =
            build_small_sea_world(&tm, bounds, &genotype, start_pos, initial_vel, None);
        let (mut w_abs, r_abs, c_abs) =
            build_small_sea_world(&tm, bounds, &genotype, start_pos, initial_vel, None);
        assert!(w_abs.get_resource::<TerrainNavigation>().is_none());
        let (mut w_dis, r_dis, c_dis) =
            build_small_sea_world(&tm, bounds, &genotype, start_pos, initial_vel, Some(false));
        let (mut w_on, r_on, c_on) =
            build_small_sea_world(&tm, bounds, &genotype, start_pos, initial_vel, Some(true));

        let mut phys_sched = Schedule::default();
        phys_sched.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
        phys_sched.add_systems((integrate_physics_system, wrap_coordinates_system).chain());

        let phys_nav_sched = || {
            let mut s = Schedule::default();
            s.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
            s.add_systems(
                (
                    integrate_physics_system,
                    terrain_navigation_system,
                    wrap_coordinates_system,
                )
                    .chain(),
            );
            s
        };
        let mut s_abs = phys_nav_sched();
        let mut s_dis = phys_nav_sched();
        let mut s_on = phys_nav_sched();

        let mut prev_phys_pos = start_pos;
        for tick in 1..=5 {
            phys_sched.run(&mut w_phys);
            s_abs.run(&mut w_abs);
            s_dis.run(&mut w_dis);
            s_on.run(&mut w_on);

            let phys_pos = w_phys.get::<Position>(r_phys).unwrap().0;
            assert_ne!(
                phys_pos, prev_phys_pos,
                "{sea_biome:?} tick {tick}: pure physics must advance agent position"
            );
            assert_eq!(cell_biome(&tm, &bounds, phys_pos), sea_biome);
            prev_phys_pos = phys_pos;

            let expected = snapshot_body(&w_phys, r_phys, c_phys);
            assert_eq!(
                snapshot_body(&w_abs, r_abs, c_abs),
                expected,
                "{sea_biome:?} schedule tick {tick}: absent policy diverged from pure physics"
            );
            assert_eq!(
                snapshot_body(&w_dis, r_dis, c_dis),
                expected,
                "{sea_biome:?} schedule tick {tick}: disabled policy diverged from pure physics"
            );

            let on_root_pos = w_on.get::<Position>(r_on).unwrap().0;
            let on_child_pos = w_on.get::<Position>(c_on).unwrap().0;
            assert!(
                is_land_biome(cell_biome(&tm, &bounds, on_root_pos)),
                "{sea_biome:?} positive control schedule tick {tick} must keep root on land"
            );
            assert_ne!(on_root_pos, phys_pos);
            assert!(
                (on_child_pos - on_root_pos).distance(initial_offset) < 1e-4,
                "{sea_biome:?} positive control schedule tick {tick} must preserve root-child offset"
            );
            if tick == 1 {
                assert_eq!(w_on.get::<Velocity>(r_on).unwrap().0, Vec3::ZERO);
                assert_eq!(w_on.get::<RigidBody>(r_on).unwrap().velocity, Vec3::ZERO);
                assert_eq!(w_on.get::<Velocity>(c_on).unwrap().0, Vec3::ZERO);
                assert_eq!(w_on.get::<RigidBody>(c_on).unwrap().velocity, Vec3::ZERO);
            }
        }
    }
}

// ---- F051b: LAND1 food and seed-drop gates via build_tick_schedule -----------------------------

fn set_terrain_nav_policy(world: &mut World, policy: Option<bool>) {
    match policy {
        None => {
            world.remove_resource::<TerrainNavigation>();
            assert!(world.get_resource::<TerrainNavigation>().is_none());
        }
        Some(enabled) => {
            world.insert_resource(TerrainNavigation { enabled });
        }
    }
}

fn collect_positions_and_water_hits<C: Component>(
    world: &mut World,
    tm: &TerrainMap,
    bounds: &MapBounds,
) -> (Vec<(u32, u32, u32)>, usize) {
    let mut positions = Vec::new();
    let mut water_hits = 0usize;
    let mut q = world.query_filtered::<&Position, With<C>>();
    for pos in q.iter(world) {
        if !is_land_biome(cell_biome(tm, bounds, pos.0)) {
            water_hits += 1;
        }
        positions.push((pos.0.x.to_bits(), pos.0.y.to_bits(), pos.0.z.to_bits()));
    }
    positions.sort_unstable();
    (positions, water_hits)
}

#[test]
fn spawn_food_through_tick_schedule_respects_land_mask_policy() {
    let base_init = initial(&[
        ("live.founders", 0.0),
        ("live.trees", 0.0),
        ("live.lakes", 0.0),
        ("live.food_cap", 80.0),
        ("live.mortality", 0.0),
        ("live.reproduction", 0.0),
        ("live.land_mask", 0.0),
    ]);

    let build_world = |policy: Option<bool>| {
        let adapter = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &base_init,
            &[],
            42,
            (1, 1),
            60,
        )
        .expect("food gate world builds");
        set_terrain_nav_policy(&mut adapter.world(), policy);
        adapter
    };

    let a_abs = build_world(None);
    let a_dis = build_world(Some(false));
    let a_on = build_world(Some(true));

    let mut s_abs = build_tick_schedule(DeterministicMode::on());
    let mut s_dis = build_tick_schedule(DeterministicMode::on());
    let mut s_on = build_tick_schedule(DeterministicMode::on());

    s_abs.run(&mut a_abs.world());
    s_dis.run(&mut a_dis.world());
    s_on.run(&mut a_on.world());

    let (tm, bounds) = {
        let w = a_abs.world();
        (
            w.resource::<TerrainMap>().clone(),
            *w.resource::<MapBounds>(),
        )
    };

    let (foods_abs, water_abs) =
        collect_positions_and_water_hits::<Food>(&mut a_abs.world(), &tm, &bounds);
    let (foods_dis, water_dis) =
        collect_positions_and_water_hits::<Food>(&mut a_dis.world(), &tm, &bounds);
    let (foods_on, water_on) =
        collect_positions_and_water_hits::<Food>(&mut a_on.world(), &tm, &bounds);

    assert_eq!(foods_abs.len(), 80);
    assert_eq!(foods_dis.len(), 80);
    assert_eq!(foods_on.len(), 80);
    assert_eq!(
        foods_abs, foods_dis,
        "absent and disabled TerrainNavigation must spawn identical food"
    );
    assert!(
        water_abs > 0,
        "positive control: absent policy must place some food on DeepOcean/Ocean"
    );
    assert!(
        water_dis > 0,
        "positive control: disabled policy must place some food on DeepOcean/Ocean"
    );
    assert_eq!(
        water_on, 0,
        "enabled policy must never spawn food on DeepOcean/Ocean"
    );
    assert_ne!(
        foods_on, foods_abs,
        "enabled policy must divert water-bound spawns onto land"
    );
}

#[test]
fn seed_dropping_through_tick_schedule_respects_land_mask_policy() {
    let base_init = initial(&[
        ("live.founders", 0.0),
        ("live.trees", 0.0),
        ("live.lakes", 0.0),
        ("live.food_cap", 0.0),
        ("live.mortality", 0.0),
        ("live.reproduction", 0.0),
        ("live.land_mask", 0.0),
    ]);

    let probe = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &base_init,
        &[],
        77,
        (1, 1),
        60,
    )
    .expect("probe world builds");
    let (tm, bounds) = {
        let w = probe.world();
        (
            w.resource::<TerrainMap>().clone(),
            *w.resource::<MapBounds>(),
        )
    };
    drop(probe);

    // Deterministically pick a walkable coastal cell whose [2, 20]-unit seed-spread annulus covers
    // both DeepOcean/Ocean cells and walkable land cells.
    let coastal_pos = (0..tm.height)
        .flat_map(|row| (0..tm.width).map(move |col| (col, row)))
        .find_map(|(col, row)| {
            if !is_land_biome(biome_from_id(tm.biomes[row * tm.width + col])) {
                return None;
            }
            let center = cell_center(&tm, &bounds, col, row);
            let mut water_samples = 0usize;
            let mut land_samples = 0usize;
            for step in 0..16usize {
                let angle = (step as f32) * std::f32::consts::TAU / 16.0;
                for &dist in &[6.0f32, 12.0, 18.0] {
                    let candidate = Vec3::new(
                        (center.x + angle.cos() * dist).clamp(bounds.min.x, bounds.max.x),
                        0.0,
                        (center.z + angle.sin() * dist).clamp(bounds.min.z, bounds.max.z),
                    );
                    if is_land_biome(cell_biome(&tm, &bounds, candidate)) {
                        land_samples += 1;
                    } else {
                        water_samples += 1;
                    }
                }
            }
            (water_samples >= 8 && land_samples >= 8).then_some(center)
        })
        .expect("real map must contain a coastal cell adjacent to both water and land");

    let build_world = |policy: Option<bool>| {
        let adapter = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &base_init,
            &[],
            77,
            (1, 1),
            60,
        )
        .expect("seed gate world builds");
        {
            let mut w = adapter.world();
            w.insert_resource(EnvironmentalSpawnSettings {
                max_tree_count: 100,
                default_seed_cooldown: 0.01,
                default_seed_spread: 20.0,
                ..Default::default()
            });
            w.spawn((
                Tree {
                    current_fruit: 0.0,
                    max_fruit: 100.0,
                    fruit_growth_rate: 2.0,
                    time_since_last_drop: 0.0,
                    seed_drop_cooldown: 0.01,
                    seed_spread_radius: 20.0,
                },
                Position(coastal_pos),
                SpatialCollider { radius: 1.5 },
                IndexedInGrid(false),
            ));
            set_terrain_nav_policy(&mut w, policy);
        }
        adapter
    };

    let a_abs = build_world(None);
    let a_dis = build_world(Some(false));
    let a_on = build_world(Some(true));

    let mut s_abs = build_tick_schedule(DeterministicMode::on());
    let mut s_dis = build_tick_schedule(DeterministicMode::on());
    let mut s_on = build_tick_schedule(DeterministicMode::on());

    for _ in 0..25 {
        s_abs.run(&mut a_abs.world());
        s_dis.run(&mut a_dis.world());
        s_on.run(&mut a_on.world());
    }

    let (trees_abs, water_abs) =
        collect_positions_and_water_hits::<Tree>(&mut a_abs.world(), &tm, &bounds);
    let (trees_dis, water_dis) =
        collect_positions_and_water_hits::<Tree>(&mut a_dis.world(), &tm, &bounds);
    let (trees_on, water_on) =
        collect_positions_and_water_hits::<Tree>(&mut a_on.world(), &tm, &bounds);

    assert!(
        trees_abs.len() > 1,
        "positive control: absent policy must drop new trees"
    );
    assert!(
        trees_on.len() > 1,
        "enabled policy must still allow inland seed drops near the coast"
    );
    assert_eq!(
        trees_abs, trees_dis,
        "absent and disabled TerrainNavigation must drop identical trees"
    );
    assert!(
        water_abs > 0,
        "positive control: absent policy must drop some seeds onto DeepOcean/Ocean"
    );
    assert!(
        water_dis > 0,
        "positive control: disabled policy must drop some seeds onto DeepOcean/Ocean"
    );
    assert_eq!(
        water_on, 0,
        "enabled policy must never place dropped trees on DeepOcean/Ocean"
    );
    assert_ne!(
        trees_on, trees_abs,
        "enabled policy must differ from unmasked seed dropping"
    );
}
