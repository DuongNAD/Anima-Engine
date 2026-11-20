//! Card FOUNDER-LAYOUT (`live.founder_layout`): opt-in 2-D genesis founder placement over
//! walkable land (`0`/absent = legacy line with zero extra RNG draws; `1` = even 2-D grid over
//! walkable land; `2` = seeded scatter over walkable land from dedicated sub-stream
//! `sim_stream::LIVE_GENESIS_FOUNDERS`).
//!
//! Covers every acceptance requirement without pinning trajectory floats:
//! 1. Key registration (`LIVE_KEY_FOUNDER_LAYOUT = "live.founder_layout"`, `LIVE_KEYS.len() = 29`),
//!    exact `{0.0, 1.0, 2.0}` validation, `LiveWorldConfig::default().founder_layout = 0`, serde
//!    backward compatibility, and `live.founders = 0` no-op across all three modes.
//! 2. `absent` and `0.0` produce identical checksums, observables, and tick-0 coordinates (with and
//!    without `live.land_mask = 1.0`); `SimRng` position/draws, `AgentBrain` weights, and tree/lake
//!    positions stay isolated across `founder_layout = 0, 1, 2`.
//! 3. Mode `1` (`grid`): all founders lie inside `MapBounds`, satisfy `is_pos_walkable` when
//!    `land_mask = 1.0` (including ocean-heavy and tiny-island synthetic maps), respect
//!    `FOUNDER_MIN_SPACING` toroidally, span 2-D (not a horizontal line), and are deterministic.
//! 4. Mode `2` (`scatter`): all founders lie inside `MapBounds`, satisfy `is_pos_walkable` when
//!    `land_mask = 1.0` (including ocean-heavy synthetic maps), respect `FOUNDER_MIN_SPACING`
//!    toroidally (including tight `40x40`, `n = 80` packing), match for same seed, and vary across
//!    different seeds.
//! 5. Multi-process checksum determinism (`mode = 1` and `2` with `land_mask = 1.0` and
//!    `grass_steering = 1.0`), closed-EU conservation (`< 1e-6`), and non-pinned crowded-founder
//!    diagnostic comparison across modes `0`, `1`, `2`.

use anima_engine_lib::core::components::{Agent, AgentBrain, Lake, ParentLink, Position, Tree};
use anima_engine_lib::core::experiment::{ExperimentError, InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    apply_live_genesis_and_policies, live_observables, LiveExperimentAdapter, LiveWorldConfig,
    FOUNDER_MIN_SPACING, LIVE_KEYS, LIVE_KEY_DRIFT_CONTROL, LIVE_KEY_DRIVE_FLOOR,
    LIVE_KEY_EVOLVED_BRAINS, LIVE_KEY_FOOD_CAP, LIVE_KEY_FOUNDERS, LIVE_KEY_FOUNDER_LAYOUT,
    LIVE_KEY_GRASS_STEERING, LIVE_KEY_LAKES, LIVE_KEY_LAKE_LAYOUT, LIVE_KEY_LAND_MASK,
    LIVE_KEY_LOCOMOTION, LIVE_KEY_MORTALITY, LIVE_KEY_PREDATOR_FRACTION, LIVE_KEY_REPRODUCTION,
    LIVE_KEY_RESOURCE_CAPACITY_SCALE, LIVE_KEY_THIRST_STEERING, LIVE_KEY_TREES,
};
use anima_engine_lib::core::resources::{MapBounds, SimRng};
use anima_engine_lib::core::terrain::{BiomeType, TerrainMap};
use anima_engine_lib::core::terrain_navigation::is_pos_walkable;
use bevy_ecs::prelude::*;
use glam::Vec3;
use rand::RngCore;
use std::process::Command;

const CHILD_ROLE: &str = "ANIMA_FOUNDER_LAYOUT_CHILD";
const CHILD_MARKER: &str = "FOUNDER_LAYOUT_CHECKSUM";

fn initial(pairs: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(pairs.iter().map(|(k, v)| ((*k).to_string(), *v)).collect())
}

fn sorted_positions(mut pts: Vec<Vec3>) -> Vec<[f32; 3]> {
    let mut raw: Vec<[f32; 3]> = pts.drain(..).map(|p| [p.x, p.y, p.z]).collect();
    raw.sort_by(|a, b| {
        a[0].total_cmp(&b[0])
            .then_with(|| a[2].total_cmp(&b[2]))
            .then_with(|| a[1].total_cmp(&b[1]))
    });
    raw
}

fn founder_positions(adapter: &LiveExperimentAdapter) -> Vec<Vec3> {
    let mut w = adapter.world();
    let mut q = w.query_filtered::<&Position, (With<Agent>, Without<ParentLink>)>();
    q.iter(&w).map(|p| p.0).collect()
}

fn founder_world_positions(world: &mut World) -> Vec<Vec3> {
    let mut q = world.query_filtered::<&Position, (With<Agent>, Without<ParentLink>)>();
    q.iter(world).map(|p| p.0).collect()
}

fn tree_and_lake_positions(adapter: &LiveExperimentAdapter) -> (Vec<[f32; 3]>, Vec<[f32; 3]>) {
    let mut w = adapter.world();
    let mut qt = w.query_filtered::<&Position, With<Tree>>();
    let mut ql = w.query_filtered::<&Position, With<Lake>>();
    let trees = qt.iter(&w).map(|p| p.0).collect();
    let lakes = ql.iter(&w).map(|p| p.0).collect();
    (sorted_positions(trees), sorted_positions(lakes))
}

fn ecology_stream_state(adapter: &LiveExperimentAdapter) -> (u64, u128, Vec<u64>) {
    let mut world = adapter.world();
    let mut rng = world
        .get_resource_mut::<SimRng>()
        .expect("live world has SimRng");
    let seed = rng.seed();
    let pos = rng.stream_pos();
    let draws: Vec<u64> = (0..8).map(|_| rng.rng().next_u64()).collect();
    (seed, pos, draws)
}

fn toroidal_dist(a: Vec3, b: Vec3, bounds: &MapBounds) -> f32 {
    let span_x = bounds.max.x - bounds.min.x;
    let span_z = bounds.max.z - bounds.min.z;
    let mut dx = (a.x - b.x).abs();
    if span_x > 0.0 {
        dx = dx.rem_euclid(span_x);
        dx = dx.min(span_x - dx);
    }
    let mut dz = (a.z - b.z).abs();
    if span_z > 0.0 {
        dz = dz.rem_euclid(span_z);
        dz = dz.min(span_z - dz);
    }
    (dx * dx + dz * dz).sqrt()
}

fn min_pairwise_toroidal_dist(pts: &[Vec3], bounds: &MapBounds) -> f32 {
    let mut min_d = f32::INFINITY;
    for i in 0..pts.len() {
        for j in (i + 1)..pts.len() {
            min_d = min_d.min(toroidal_dist(pts[i], pts[j], bounds));
        }
    }
    min_d
}

fn assert_all_in_bounds(pts: &[Vec3], bounds: &MapBounds, label: &str) {
    for (idx, p) in pts.iter().enumerate() {
        assert!(
            p.x >= bounds.min.x
                && p.x <= bounds.max.x
                && p.z >= bounds.min.z
                && p.z <= bounds.max.z,
            "{label}: founder {idx} at ({}, {}) out of bounds [{}, {}] x [{}, {}]",
            p.x,
            p.z,
            bounds.min.x,
            bounds.max.x,
            bounds.min.z,
            bounds.max.z
        );
    }
}

fn assert_all_walkable(pts: &[Vec3], tm: &TerrainMap, bounds: &MapBounds, label: &str) {
    for (idx, p) in pts.iter().enumerate() {
        assert!(
            is_pos_walkable(tm, bounds, *p),
            "{label}: founder {idx} at ({}, {}) landed on non-walkable water",
            p.x,
            p.z
        );
    }
}

fn z_span(pts: &[Vec3]) -> f32 {
    let min_z = pts.iter().map(|p| p.z).fold(f32::INFINITY, f32::min);
    let max_z = pts.iter().map(|p| p.z).fold(f32::NEG_INFINITY, f32::max);
    max_z - min_z
}

fn make_synthetic_terrain(width: usize, height: usize, biomes: Vec<u8>) -> TerrainMap {
    assert_eq!(biomes.len(), width * height);
    let n = width * height;
    TerrainMap {
        width,
        height,
        elevations: vec![10.0; n],
        moistures: vec![0.5; n],
        temperatures: vec![0.5; n],
        biomes,
        flows: vec![0.0; n],
        pois: Vec::new(),
    }
}

// ---- 1. Key registration, exact validation, serde compatibility, founders=0 --------------------

#[test]
fn key_is_registered_validated_and_serde_backward_compatible() {
    assert_eq!(LIVE_KEY_FOUNDER_LAYOUT, "live.founder_layout");
    assert_eq!(LIVE_KEYS.len(), 29);
    assert!(LIVE_KEYS.contains(&LIVE_KEY_FOUNDER_LAYOUT));
    const {
        assert!(FOUNDER_MIN_SPACING > 0.0);
    }
    assert_eq!(LiveWorldConfig::default().founder_layout, 0);

    let cfg_absent =
        LiveWorldConfig::from_initial_conditions(&initial(&[])).expect("empty config builds");
    assert_eq!(cfg_absent.founder_layout, 0);

    for (raw, expected) in [(0.0, 0u8), (1.0, 1u8), (2.0, 2u8)] {
        let cfg =
            LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_FOUNDER_LAYOUT, raw)]))
                .unwrap_or_else(|e| panic!("founder_layout = {raw} must parse: {e}"));
        assert_eq!(cfg.founder_layout, expected);
    }

    for bad in [-1.0, 0.5, 1.5, 3.0] {
        let err =
            LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_FOUNDER_LAYOUT, bad)]))
                .expect_err("out-of-enum founder_layout must fail");
        assert!(
            matches!(err, ExperimentError::OutOfRange { .. }),
            "expected OutOfRange for {bad}, got {err:?}"
        );
    }
    for non_finite in [f64::NAN, f64::INFINITY] {
        let err = LiveWorldConfig::from_initial_conditions(&initial(&[(
            LIVE_KEY_FOUNDER_LAYOUT,
            non_finite,
        )]))
        .expect_err("non-finite founder_layout must fail");
        assert!(
            matches!(err, ExperimentError::NotFinite { .. }),
            "expected NotFinite for {non_finite}, got {err:?}"
        );
    }

    let mut val = serde_json::to_value(LiveWorldConfig::default()).expect("serialize");
    val.as_object_mut()
        .expect("json object")
        .remove("founder_layout");
    let decoded: LiveWorldConfig = serde_json::from_value(val).expect("backward compatible serde");
    assert_eq!(decoded.founder_layout, 0);

    for mode in [0.0, 1.0, 2.0] {
        let adapter = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &initial(&[(LIVE_KEY_FOUNDERS, 0.0), (LIVE_KEY_FOUNDER_LAYOUT, mode)]),
            &[],
            7,
            (16, 16),
            10,
        )
        .expect("zero founders allowed");
        assert_eq!(founder_positions(&adapter).len(), 0);
    }
}

// ---- 2. Absent == 0.0 parity and RNG stream isolation across 0, 1, 2 ---------------------------

#[test]
fn absent_and_zero_are_bit_identical_and_streams_stay_isolated() {
    let run_pair = |with_land_mask: bool, ticks: u64| {
        let sample = |layout: Option<f64>| {
            let mut pairs = vec![
                (LIVE_KEY_FOUNDERS, 30.0),
                (LIVE_KEY_PREDATOR_FRACTION, 0.2),
                (LIVE_KEY_TREES, 8.0),
                (LIVE_KEY_LAKES, 8.0),
                (LIVE_KEY_LAKE_LAYOUT, 1.0),
                (LIVE_KEY_MORTALITY, 1.0),
                (LIVE_KEY_REPRODUCTION, 1.0),
                (LIVE_KEY_LOCOMOTION, 1.0),
                (LIVE_KEY_DRIVE_FLOOR, 0.5),
                (LIVE_KEY_THIRST_STEERING, 1.0),
                (LIVE_KEY_GRASS_STEERING, 1.0),
            ];
            if with_land_mask {
                pairs.push((LIVE_KEY_LAND_MASK, 1.0));
            }
            if let Some(v) = layout {
                pairs.push((LIVE_KEY_FOUNDER_LAYOUT, v));
            }
            let mut a = LiveExperimentAdapter::from_manifest(
                &WorldLawSet::baseline(),
                &initial(&pairs),
                &[],
                19,
                (16, 16),
                ticks,
            )
            .expect("adapter builds");
            let init_pos = sorted_positions(founder_positions(&a));
            let mut traj = Vec::new();
            for t in 1..=ticks {
                a.run_schedule_once();
                if t == 1 || t % 200 == 0 {
                    traj.push((t, a.checksum(), a.observables()));
                }
            }
            (init_pos, traj)
        };

        assert_eq!(
            sample(None),
            sample(Some(0.0)),
            "absent and 0.0 must be bit-identical (land_mask={with_land_mask})"
        );
    };

    run_pair(false, 1_000);
    run_pair(true, 500);

    // Stream isolation across modes 0, 1, 2 with evolved brains on: neither `SimRng` nor
    // `brain_rng` nor tree/lake positions may shift when `live.founder_layout` changes.
    let inspect_genesis = |layout: f64| {
        let a = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &initial(&[
                (LIVE_KEY_FOUNDERS, 24.0),
                (LIVE_KEY_PREDATOR_FRACTION, 0.25),
                (LIVE_KEY_TREES, 8.0),
                (LIVE_KEY_LAKES, 4.0),
                (LIVE_KEY_EVOLVED_BRAINS, 1.0),
                (LIVE_KEY_LAND_MASK, 1.0),
                (LIVE_KEY_FOUNDER_LAYOUT, layout),
            ]),
            &[],
            42,
            (16, 16),
            20,
        )
        .expect("adapter builds");
        let sim_state = ecology_stream_state(&a);
        let env_pos = tree_and_lake_positions(&a);
        let founders = sorted_positions(founder_positions(&a));
        let mut w = a.world();
        let mut q = w.query_filtered::<&AgentBrain, (With<Agent>, Without<ParentLink>)>();
        let brain_count = q.iter(&w).count();
        let first_brain = q.iter(&w).next().expect("brain present").clone();
        (sim_state, env_pos, founders, brain_count, first_brain)
    };

    let (sim0, env0, pos0, bc0, brain0) = inspect_genesis(0.0);
    let (sim1, env1, pos1, bc1, brain1) = inspect_genesis(1.0);
    let (sim2, env2, pos2, bc2, brain2) = inspect_genesis(2.0);

    assert_eq!(sim0, sim1, "layout=1 must not alter SimRng");
    assert_eq!(sim0, sim2, "layout=2 must not alter SimRng");
    assert_eq!(env0, env1, "layout=1 must not move trees/lakes");
    assert_eq!(env0, env2, "layout=2 must not move trees/lakes");
    assert_eq!(bc0, 24);
    assert_eq!(bc0, bc1);
    assert_eq!(bc0, bc2);
    assert_eq!(brain0, brain1, "layout=1 must not alter brain RNG draws");
    assert_eq!(brain0, brain2, "layout=2 must not alter brain RNG draws");
    assert_ne!(pos0, pos1, "layout=1 must differ from legacy line");
    assert_ne!(pos0, pos2, "layout=2 must differ from legacy line");
    assert_ne!(
        pos1, pos2,
        "layout=2 scatter must differ from layout=1 grid"
    );
}

// ---- 3. Layout 1: even 2-D grid over walkable land, spacing, ocean-heavy & tiny islands --------

#[test]
fn layout_one_grid_places_evenly_on_walkable_land_with_min_spacing() {
    for land_mask in [0.0, 1.0] {
        for &n in &[1usize, 10, 100, 120] {
            let build = |seed: u64| {
                LiveExperimentAdapter::from_manifest(
                    &WorldLawSet::baseline(),
                    &initial(&[
                        (LIVE_KEY_FOUNDERS, n as f64),
                        (LIVE_KEY_PREDATOR_FRACTION, 0.2),
                        (LIVE_KEY_TREES, 4.0),
                        (LIVE_KEY_LAKES, 4.0),
                        (LIVE_KEY_LAND_MASK, land_mask),
                        (LIVE_KEY_LOCOMOTION, 1.0),
                        (LIVE_KEY_FOUNDER_LAYOUT, 1.0),
                    ]),
                    &[],
                    seed,
                    (16, 16),
                    200,
                )
                .expect("grid world builds")
            };
            let mut a = build(42);
            let b = build(99);
            let pos_a = founder_positions(&a);
            let pos_b = founder_positions(&b);
            assert_eq!(pos_a.len(), n);
            assert_eq!(
                sorted_positions(pos_a.clone()),
                sorted_positions(pos_b),
                "grid layout=1 must be seed-independent for fixed terrain"
            );

            let (bounds, tm) = {
                let w = a.world();
                (
                    *w.resource::<MapBounds>(),
                    w.resource::<TerrainMap>().clone(),
                )
            };
            assert_all_in_bounds(&pos_a, &bounds, "grid");
            if land_mask == 1.0 {
                assert_all_walkable(&pos_a, &tm, &bounds, "grid genesis");
            }
            if n >= 2 {
                let min_d = min_pairwise_toroidal_dist(&pos_a, &bounds);
                assert!(
                    min_d >= FOUNDER_MIN_SPACING,
                    "grid n={n} land_mask={land_mask}: min toroidal dist {min_d} < {FOUNDER_MIN_SPACING}"
                );
            }
            if n >= 10 {
                let span_z = bounds.max.z - bounds.min.z;
                let min_z_span = if land_mask == 0.0 {
                    0.5 * span_z
                } else {
                    0.45 * span_z
                };
                assert!(
                    z_span(&pos_a) >= min_z_span,
                    "grid n={n} must spread in 2-D, got z_span={} / {span_z}",
                    z_span(&pos_a)
                );
            }

            if land_mask == 1.0 {
                for _ in 0..200 {
                    a.run_schedule_once();
                }
                let pos_after = founder_positions(&a);
                assert_all_walkable(&pos_after, &tm, &bounds, "grid tick 200");
            }
        }
    }

    // Custom ocean-heavy archipelago (75% Ocean, including center strip) + tiny 6x6 island fallback.
    let bounds = MapBounds::default();
    let cfg = LiveWorldConfig::from_initial_conditions(&initial(&[
        (LIVE_KEY_FOUNDERS, 24.0),
        (LIVE_KEY_LAND_MASK, 1.0),
        (LIVE_KEY_FOUNDER_LAYOUT, 1.0),
    ]))
    .expect("cfg builds");

    let mut arch_biomes = vec![BiomeType::Ocean as u8; 64 * 64];
    for r in 0..64 {
        for c in 0..64 {
            if (r < 14 && c < 30) || (r >= 48 && c >= 34) {
                arch_biomes[r * 64 + c] = BiomeType::Grassland as u8;
            }
        }
    }
    let arch_tm = make_synthetic_terrain(64, 64, arch_biomes);
    let mut w_arch = World::new();
    w_arch.insert_resource(bounds);
    w_arch.insert_resource(arch_tm.clone());
    apply_live_genesis_and_policies(&mut w_arch, 7, &cfg, bounds);
    let pos_arch = founder_world_positions(&mut w_arch);
    assert_eq!(pos_arch.len(), 24);
    assert_all_walkable(&pos_arch, &arch_tm, &bounds, "grid archipelago");
    assert!(
        min_pairwise_toroidal_dist(&pos_arch, &bounds) >= FOUNDER_MIN_SPACING,
        "grid archipelago violated FOUNDER_MIN_SPACING"
    );

    let mut tiny_biomes = vec![BiomeType::DeepOcean as u8; 64 * 64];
    for r in 2..8 {
        for c in 2..8 {
            tiny_biomes[r * 64 + c] = BiomeType::Grassland as u8;
        }
    }
    let tiny_tm = make_synthetic_terrain(64, 64, tiny_biomes);
    let mut w_tiny = World::new();
    w_tiny.insert_resource(bounds);
    w_tiny.insert_resource(tiny_tm.clone());
    apply_live_genesis_and_policies(&mut w_tiny, 7, &cfg, bounds);
    let pos_tiny = founder_world_positions(&mut w_tiny);
    assert_eq!(pos_tiny.len(), 24);
    assert_all_walkable(&pos_tiny, &tiny_tm, &bounds, "grid tiny island");
}

// ---- 4. Layout 2: seeded random scatter over walkable land, spacing, seed sensitivity --------

#[test]
fn layout_two_scatter_places_on_walkable_land_seed_deterministic_and_varied() {
    let run_scatter = |land_mask: f64, n: usize, seed: u64| {
        LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &initial(&[
                (LIVE_KEY_FOUNDERS, n as f64),
                (LIVE_KEY_PREDATOR_FRACTION, 0.2),
                (LIVE_KEY_TREES, 4.0),
                (LIVE_KEY_LAKES, 4.0),
                (LIVE_KEY_LAND_MASK, land_mask),
                (LIVE_KEY_LOCOMOTION, 1.0),
                (LIVE_KEY_FOUNDER_LAYOUT, 2.0),
            ]),
            &[],
            seed,
            (16, 16),
            200,
        )
        .expect("scatter world builds")
    };

    for land_mask in [0.0, 1.0] {
        for &n in &[10usize, 100, 120] {
            let mut a1 = run_scatter(land_mask, n, 42);
            let a2 = run_scatter(land_mask, n, 42);
            let b = run_scatter(land_mask, n, 43);

            let p1 = founder_positions(&a1);
            let p2 = founder_positions(&a2);
            let pb = founder_positions(&b);
            assert_eq!(p1.len(), n);
            assert_eq!(
                p1, p2,
                "scatter layout=2 must reproduce identically for same seed"
            );
            assert_ne!(
                sorted_positions(p1.clone()),
                sorted_positions(pb),
                "scatter layout=2 must differ across seeds 42 vs 43 (n={n}, land_mask={land_mask})"
            );

            let (bounds, tm) = {
                let w = a1.world();
                (
                    *w.resource::<MapBounds>(),
                    w.resource::<TerrainMap>().clone(),
                )
            };
            assert_all_in_bounds(&p1, &bounds, "scatter");
            if land_mask == 1.0 {
                assert_all_walkable(&p1, &tm, &bounds, "scatter genesis");
            }
            let min_d = min_pairwise_toroidal_dist(&p1, &bounds);
            assert!(
                min_d >= FOUNDER_MIN_SPACING,
                "scatter n={n} land_mask={land_mask}: min toroidal dist {min_d} < {FOUNDER_MIN_SPACING}"
            );
            let span_z = bounds.max.z - bounds.min.z;
            let min_z_span = if n >= 100 {
                0.45 * span_z
            } else {
                0.15 * span_z
            };
            assert!(
                z_span(&p1) >= min_z_span,
                "scatter n={n} must spread in 2-D, got z_span={} / {span_z}",
                z_span(&p1)
            );

            if land_mask == 1.0 {
                for _ in 0..200 {
                    a1.run_schedule_once();
                }
                let p_after = founder_positions(&a1);
                assert_all_walkable(&p_after, &tm, &bounds, "scatter tick 200");
            }
        }
    }

    // Tight-packing bounds (40x40 wu, n = 80) across multiple seeds: unguarded random draws
    // collide with probability > 1 - 1e-11, so this catches removing spacing enforcement (ML4).
    let tight_bounds = MapBounds {
        min: Vec3::new(-20.0, 0.0, -20.0),
        max: Vec3::new(20.0, 10.0, 20.0),
    };
    let tight_cfg = LiveWorldConfig::from_initial_conditions(&initial(&[
        (LIVE_KEY_FOUNDERS, 80.0),
        (LIVE_KEY_FOUNDER_LAYOUT, 2.0),
    ]))
    .expect("tight cfg builds");
    for seed in [1u64, 2, 3] {
        let mut w = World::new();
        w.insert_resource(tight_bounds);
        apply_live_genesis_and_policies(&mut w, seed, &tight_cfg, tight_bounds);
        let pts = founder_world_positions(&mut w);
        assert_eq!(pts.len(), 80);
        assert_all_in_bounds(&pts, &tight_bounds, "tight scatter");
        let min_d = min_pairwise_toroidal_dist(&pts, &tight_bounds);
        assert!(
            min_d >= FOUNDER_MIN_SPACING,
            "tight 40x40 scatter seed={seed}: min dist {min_d} < {FOUNDER_MIN_SPACING}"
        );
    }

    // Ocean-heavy synthetic terrain (80% Ocean): catches removing LAND1 walkability check (ML3).
    let default_bounds = MapBounds::default();
    let ocean_cfg = LiveWorldConfig::from_initial_conditions(&initial(&[
        (LIVE_KEY_FOUNDERS, 30.0),
        (LIVE_KEY_LAND_MASK, 1.0),
        (LIVE_KEY_FOUNDER_LAYOUT, 2.0),
    ]))
    .expect("ocean cfg builds");
    let mut ocean_biomes = vec![BiomeType::Ocean as u8; 64 * 64];
    for r in 4..20 {
        for c in 4..44 {
            ocean_biomes[r * 64 + c] = BiomeType::Grassland as u8;
        }
    }
    let ocean_tm = make_synthetic_terrain(64, 64, ocean_biomes);
    let mut w_ocean = World::new();
    w_ocean.insert_resource(default_bounds);
    w_ocean.insert_resource(ocean_tm.clone());
    apply_live_genesis_and_policies(&mut w_ocean, 11, &ocean_cfg, default_bounds);
    let pos_ocean = founder_world_positions(&mut w_ocean);
    assert_eq!(pos_ocean.len(), 30);
    assert_all_walkable(
        &pos_ocean,
        &ocean_tm,
        &default_bounds,
        "scatter ocean-heavy",
    );
    assert!(
        min_pairwise_toroidal_dist(&pos_ocean, &default_bounds) >= FOUNDER_MIN_SPACING,
        "scatter ocean-heavy violated FOUNDER_MIN_SPACING"
    );
}

// ---- 5. Multi-process determinism, closed EU, and non-pinned crowded-founder diagnostic -------

#[test]
fn founder_layout_child_entry_point() {
    let Ok(mode_str) = std::env::var(CHILD_ROLE) else {
        return;
    };
    let mode: f64 = mode_str.parse().expect("valid child layout mode");
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial(&[
            (LIVE_KEY_FOUNDERS, 30.0),
            (LIVE_KEY_PREDATOR_FRACTION, 0.2),
            (LIVE_KEY_TREES, 8.0),
            (LIVE_KEY_LAKES, 8.0),
            (LIVE_KEY_LAKE_LAYOUT, 1.0),
            (LIVE_KEY_FOOD_CAP, 50.0),
            (LIVE_KEY_MORTALITY, 1.0),
            (LIVE_KEY_REPRODUCTION, 1.0),
            (LIVE_KEY_LOCOMOTION, 1.0),
            (LIVE_KEY_DRIVE_FLOOR, 0.5),
            (LIVE_KEY_THIRST_STEERING, 1.0),
            (LIVE_KEY_GRASS_STEERING, 1.0),
            (LIVE_KEY_LAND_MASK, 1.0),
            (LIVE_KEY_FOUNDER_LAYOUT, mode),
        ]),
        &[],
        23,
        (16, 16),
        300,
    )
    .expect("child builds");
    for _ in 0..300 {
        a.run_schedule_once();
    }
    println!("{CHILD_MARKER} {:#010x}", a.checksum());
}

fn child_checksum_for(mode: &str) -> u32 {
    let output = Command::new(std::env::current_exe().expect("current_exe"))
        .args(["--exact", "founder_layout_child_entry_point", "--nocapture"])
        .env(CHILD_ROLE, mode)
        .env("ANIMA_USE_GPU", "0")
        .output()
        .expect("spawn child");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|l| l.trim_start().starts_with(CHILD_MARKER))
        .unwrap_or_else(|| {
            panic!(
                "child failed to print checksum for mode={mode}\nstdout:\n{stdout}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
    let token = line.trim().rsplit(' ').next().expect("hex token");
    u32::from_str_radix(token.trim_start_matches("0x"), 16).expect("hex checksum")
}

#[test]
fn multi_process_determinism_closed_eu_and_crowded_diagnostic() {
    let closed_eu = |a: &mut LiveExperimentAdapter| -> f64 {
        let mut w = a.world();
        live_observables(&mut w)
            .into_iter()
            .find(|(k, _)| k == "live.closed_eu_total")
            .expect("closed_eu_total")
            .1
    };

    for (mode_str, mode_val) in [("1", 1.0), ("2", 2.0)] {
        let s1 = child_checksum_for(mode_str);
        let s2 = child_checksum_for(mode_str);
        assert_eq!(
            s1, s2,
            "multi-process determinism failed for mode={mode_str}"
        );
        assert_ne!(s1, 0);

        let mut a = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &initial(&[
                (LIVE_KEY_FOUNDERS, 30.0),
                (LIVE_KEY_PREDATOR_FRACTION, 0.2),
                (LIVE_KEY_TREES, 8.0),
                (LIVE_KEY_LAKES, 8.0),
                (LIVE_KEY_LAKE_LAYOUT, 1.0),
                (LIVE_KEY_FOOD_CAP, 50.0),
                (LIVE_KEY_MORTALITY, 1.0),
                (LIVE_KEY_REPRODUCTION, 1.0),
                (LIVE_KEY_LOCOMOTION, 1.0),
                (LIVE_KEY_DRIVE_FLOOR, 0.5),
                (LIVE_KEY_THIRST_STEERING, 1.0),
                (LIVE_KEY_GRASS_STEERING, 1.0),
                (LIVE_KEY_LAND_MASK, 1.0),
                (LIVE_KEY_FOUNDER_LAYOUT, mode_val),
                (LIVE_KEY_DRIFT_CONTROL, 0.0),
            ]),
            &[],
            23,
            (16, 16),
            300,
        )
        .expect("closed EU run builds");
        a.run_schedule_once();
        let start = closed_eu(&mut a);
        let mut worst = 0.0f64;
        for _ in 1..300 {
            a.run_schedule_once();
            worst = worst.max((closed_eu(&mut a) - start).abs() / start.abs().max(1.0));
        }
        assert!(
            worst < 1e-6,
            "mode={mode_str} closed EU drifted by {worst} relative to {start}"
        );
    }

    // Non-pinned crowded-founder diagnostic (SEL2-P smoke conditions, T = 500): prints surviving
    // count and stripped-cell share without pinning floats or counts.
    for mode in [0.0, 1.0, 2.0] {
        let mut a = LiveExperimentAdapter::from_manifest(
            &WorldLawSet::baseline(),
            &initial(&[
                (LIVE_KEY_FOUNDERS, 100.0),
                (LIVE_KEY_PREDATOR_FRACTION, 0.0),
                (LIVE_KEY_TREES, 8.0),
                (LIVE_KEY_LAKES, 16.0),
                (LIVE_KEY_LAKE_LAYOUT, 1.0),
                (LIVE_KEY_RESOURCE_CAPACITY_SCALE, 0.63),
                (LIVE_KEY_MORTALITY, 1.0),
                (LIVE_KEY_REPRODUCTION, 1.0),
                (LIVE_KEY_LOCOMOTION, 1.0),
                (LIVE_KEY_DRIVE_FLOOR, 0.5),
                (LIVE_KEY_THIRST_STEERING, 1.0),
                (LIVE_KEY_GRASS_STEERING, 1.0),
                (LIVE_KEY_FOUNDER_LAYOUT, mode),
            ]),
            &[],
            870_001,
            (16, 16),
            500,
        )
        .expect("diagnostic run builds");
        for _ in 0..500 {
            a.run_schedule_once();
        }
        let obs = a.observables();
        for (k, v) in &obs {
            assert!(v.is_finite(), "non-finite observable {k} for mode={mode}");
        }
        let agents = obs
            .iter()
            .find(|(k, _)| k == "live.agent_count")
            .map(|(_, v)| *v)
            .unwrap_or(0.0);
        let crop = obs
            .iter()
            .find(|(k, _)| k == "live.standing_crop")
            .map(|(_, v)| *v)
            .unwrap_or(0.0);
        println!(
            "FOUNDER_LAYOUT_DIAG mode={mode} tick=500 agents={agents} standing_crop={crop:.2} checksum={:#010x}",
            a.checksum()
        );
    }
}
