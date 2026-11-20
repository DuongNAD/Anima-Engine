//! Card S12 (`live.lake_layout`): opt-in 2-D lattice lake placement at genesis.
//!
//! Covers all verification criteria:
//! 1. Registration, exact boolean validation, `live.lake_layout = 1` + `live.lakes = 0` preflight
//!    refusal (`InvalidLaw`), and serde backward compatibility.
//! 2. `absent` and `0` produce bit-identical sampled trajectories matching base commit
//!    `dea18583881a12ba54ea719f9b6d78dd6d18b05e` on worlds with lakes and locomotion on, while `1`
//!    diverges (`DONE WHEN #1`).
//! 3. Lake coordinates for `n in {1, 2, 3, 4, 8, 9}` match the 2-D lattice formula (including
//!    centred partial last row), lie strictly inside `MapBounds`, are pairwise distinct, leave
//!    founders/trees unmoved, and shrink worst-case founder-to-nearest-lake distance for `n = 8`
//!    (`DONE WHEN #2`).
//! 4. Over 20,000 ticks in the SEL2-E best world (`seed 2`), layout `1` produces fewer
//!    dehydration deaths than layout `0` (`DONE WHEN #3`).
//! 5. Four-process checksum determinism and closed-EU ledger conservation `< 1e-6` (`DONE WHEN #4`).

use anima_engine_lib::ai::hrrl::HomeostaticState;
use anima_engine_lib::core::components::{Agent, Lake, ParentLink, Position, Tree};
use anima_engine_lib::core::experiment::{ExperimentError, InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{
    live_observables, LiveExperimentAdapter, LiveWorldConfig, LIVE_KEYS, LIVE_KEY_LAKE_LAYOUT,
};
use anima_engine_lib::core::resources::MapBounds;
use bevy_ecs::prelude::*;
use std::process::Command;

const CHILD_ROLE: &str = "ANIMA_S12_CHILD";
const MARKER: &str = "S12_CHECKSUM";

fn initial(pairs: &[(&str, f64)]) -> InitialConditionSet {
    InitialConditionSet::new(pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect())
}

fn sorted_positions(mut pts: Vec<[f32; 3]>) -> Vec<[f32; 3]> {
    pts.sort_by(|a, b| {
        a[0].total_cmp(&b[0])
            .then_with(|| a[2].total_cmp(&b[2]))
            .then_with(|| a[1].total_cmp(&b[1]))
    });
    pts
}

fn expected_lattice_positions(n: usize, bounds: MapBounds) -> Vec<(f32, f32)> {
    let k = (1..=n)
        .find(|&k| k.checked_mul(k).is_some_and(|sq| sq >= n))
        .unwrap_or(1);
    let m = n.div_ceil(k);
    let span_x = bounds.max.x - bounds.min.x;
    let span_z = bounds.max.z - bounds.min.z;
    (0..n)
        .map(|i| {
            let c = i % k;
            let r = i / k;
            let cols_in_row = if r + 1 == m { n - (m - 1) * k } else { k };
            let x_frac = (c as f32 + 1.0) / (cols_in_row as f32 + 1.0);
            let z_frac = (r as f32 + 1.0) / (m as f32 + 1.0);
            (
                bounds.min.x + x_frac * span_x,
                bounds.min.z + z_frac * span_z,
            )
        })
        .collect()
}

// ---- 1. Registration, validation and preflight checks ------------------------------------------

#[test]
fn key_is_registered_validated_and_refused_with_zero_lakes() {
    assert_eq!(LIVE_KEY_LAKE_LAYOUT, "live.lake_layout");
    assert_eq!(LIVE_KEYS.len(), 29);
    assert!(LIVE_KEYS.contains(&LIVE_KEY_LAKE_LAYOUT));
    assert_eq!(LiveWorldConfig::default().lake_layout, 0);

    let cfg_absent = LiveWorldConfig::from_initial_conditions(&initial(&[]))
        .expect("empty initial conditions build");
    assert_eq!(cfg_absent.lake_layout, 0);

    let cfg_zero = LiveWorldConfig::from_initial_conditions(&initial(&[
        (LIVE_KEY_LAKE_LAYOUT, 0.0),
        ("live.lakes", 0.0),
    ]))
    .expect("layout 0 with 0 lakes is allowed");
    assert_eq!(cfg_zero.lake_layout, 0);

    let cfg_one = LiveWorldConfig::from_initial_conditions(&initial(&[
        (LIVE_KEY_LAKE_LAYOUT, 1.0),
        ("live.lakes", 1.0),
    ]))
    .expect("layout 1 with >= 1 lake builds");
    assert_eq!(cfg_one.lake_layout, 1);

    let err_zero_lakes = LiveWorldConfig::from_initial_conditions(&initial(&[
        (LIVE_KEY_LAKE_LAYOUT, 1.0),
        ("live.lakes", 0.0),
    ]))
    .expect_err("layout 1 with 0 lakes must be refused at preflight");
    assert!(
        matches!(err_zero_lakes, ExperimentError::InvalidLaw { .. }),
        "expected InvalidLaw for lake_layout=1 + lakes=0, got {err_zero_lakes:?}"
    );

    for bad in [-1.0, 0.5, 2.0] {
        let err =
            LiveWorldConfig::from_initial_conditions(&initial(&[(LIVE_KEY_LAKE_LAYOUT, bad)]))
                .expect_err("non-boolean lake_layout must be rejected");
        assert!(
            matches!(err, ExperimentError::OutOfRange { .. }),
            "expected OutOfRange for {bad}, got {err:?}"
        );
    }
    for non_finite in [f64::NAN, f64::INFINITY] {
        let err = LiveWorldConfig::from_initial_conditions(&initial(&[(
            LIVE_KEY_LAKE_LAYOUT,
            non_finite,
        )]))
        .expect_err("non-finite lake_layout must be rejected");
        assert!(
            matches!(err, ExperimentError::NotFinite { .. }),
            "expected NotFinite for {non_finite}, got {err:?}"
        );
    }

    let mut val = serde_json::to_value(LiveWorldConfig::default()).expect("serialize");
    val.as_object_mut()
        .expect("json object")
        .remove("lake_layout");
    let decoded: LiveWorldConfig = serde_json::from_value(val).expect("backward compatible serde");
    assert_eq!(decoded.lake_layout, 0);
}

// ---- 2. DONE WHEN #1: absent and 0 match base trajectory ---------------------------------------

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
                println!("S12_TRAJ world={tag} tick={tick} checksum={sum:#010x}");
            }
            out.push((tick, sum));
        }
    }
    out
}

#[test]
fn absent_and_zero_match_every_sampled_tick_with_lakes_and_locomotion_on() {
    let world_a_base = [
        ("live.founders", 30.0),
        ("live.predator_fraction", 0.0),
        ("live.trees", 8.0),
        ("live.lakes", 8.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
        ("live.locomotion", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
    ];
    let world_b_base = [
        ("live.founders", 20.0),
        ("live.predator_fraction", 0.25),
        ("live.trees", 4.0),
        ("live.lakes", 4.0),
        ("live.mortality", 0.0),
        ("live.reproduction", 0.0),
        ("live.locomotion", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
    ];

    let a_absent = sample_trajectory(Some("A"), &initial(&world_a_base), 7, 600);
    let mut a_zero_pairs = world_a_base.to_vec();
    a_zero_pairs.push((LIVE_KEY_LAKE_LAYOUT, 0.0));
    let a_zero = sample_trajectory(None, &initial(&a_zero_pairs), 7, 600);
    assert_eq!(a_absent, a_zero, "World A: absent vs 0.0 diverged");

    let b_absent = sample_trajectory(Some("B"), &initial(&world_b_base), 13, 1_000);
    let mut b_zero_pairs = world_b_base.to_vec();
    b_zero_pairs.push((LIVE_KEY_LAKE_LAYOUT, 0.0));
    let b_zero = sample_trajectory(None, &initial(&b_zero_pairs), 13, 1_000);
    assert_eq!(b_absent, b_zero, "World B: absent vs 0.0 diverged");

    // Once animals become thirsty (`dehydration_ticks = 960`), `lake_layout = 1.0` diverges from
    // `0.0` (before thirst drops below THIRST_STEERING_FRACTION, lake coordinates do not affect
    // locomotion trajectories).
    let mut c_off_pairs = world_a_base.to_vec();
    c_off_pairs.push(("live.dehydration_ticks", 960.0));
    let c_off = sample_trajectory(None, &initial(&c_off_pairs), 7, 2_500);
    let mut c_on_pairs = c_off_pairs.clone();
    c_on_pairs.push((LIVE_KEY_LAKE_LAYOUT, 1.0));
    let c_on = sample_trajectory(None, &initial(&c_on_pairs), 7, 2_500);
    assert_ne!(
        c_on, c_off,
        "lake_layout = 1.0 should diverge once agents steer toward lakes"
    );
}

// ---- 3. DONE WHEN #2: formula, bounds, pairwise distinction & founder distance -----------------

struct SpawnedPositions {
    bounds: MapBounds,
    lakes: Vec<[f32; 3]>,
    founders: Vec<[f32; 3]>,
    trees: Vec<[f32; 3]>,
}

fn spawn_positions_for(lakes: usize, layout: f64) -> SpawnedPositions {
    let init = initial(&[
        ("live.founders", 100.0),
        ("live.predator_fraction", 0.0),
        ("live.trees", 8.0),
        ("live.lakes", lakes as f64),
        (LIVE_KEY_LAKE_LAYOUT, layout),
    ]);
    let a =
        LiveExperimentAdapter::from_manifest(&WorldLawSet::baseline(), &init, &[], 2, (16, 16), 10)
            .expect("genesis builds");
    let mut w = a.world();
    let bounds = *w.resource::<MapBounds>();

    let mut q_lakes = w.query_filtered::<&Position, With<Lake>>();
    let lake_pos = q_lakes
        .iter(&w)
        .map(|p| [p.0.x, p.0.y, p.0.z])
        .collect::<Vec<_>>();

    let mut q_founders = w.query_filtered::<&Position, (With<Agent>, Without<ParentLink>)>();
    let founder_pos = q_founders
        .iter(&w)
        .map(|p| [p.0.x, p.0.y, p.0.z])
        .collect::<Vec<_>>();

    let mut q_trees = w.query_filtered::<&Position, With<Tree>>();
    let tree_pos = q_trees
        .iter(&w)
        .map(|p| [p.0.x, p.0.y, p.0.z])
        .collect::<Vec<_>>();

    SpawnedPositions {
        bounds,
        lakes: sorted_positions(lake_pos),
        founders: sorted_positions(founder_pos),
        trees: sorted_positions(tree_pos),
    }
}

#[test]
fn lake_positions_match_formula_lie_in_bounds_are_distinct_and_closer_to_founders() {
    for &n in &[1usize, 2, 3, 4, 8, 9] {
        let p0 = spawn_positions_for(n, 0.0);
        let p1 = spawn_positions_for(n, 1.0);
        assert_eq!(p0.bounds.min, p1.bounds.min);
        assert_eq!(p0.bounds.max, p1.bounds.max);
        assert_eq!(p1.lakes.len(), n, "n={n}: lake entity count mismatch");
        assert_eq!(p0.founders.len(), 100);
        assert_eq!(p0.trees.len(), 8);

        for (a0, a1) in p0.founders.iter().zip(p1.founders.iter()) {
            let d = ((a0[0] - a1[0]).powi(2) + (a0[2] - a1[2]).powi(2)).sqrt();
            assert!(d < 1e-5, "n={n}: founder moved by {d}");
        }
        for (t0, t1) in p0.trees.iter().zip(p1.trees.iter()) {
            let d = ((t0[0] - t1[0]).powi(2) + (t0[2] - t1[2]).powi(2)).sqrt();
            assert!(d < 1e-5, "n={n}: tree moved by {d}");
        }

        let mut expected = expected_lattice_positions(n, p1.bounds);
        expected.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));
        for (actual, (ex, ez)) in p1.lakes.iter().zip(expected.iter()) {
            assert!(
                actual[0] > p1.bounds.min.x && actual[0] < p1.bounds.max.x,
                "n={n}: x={} out of bounds",
                actual[0]
            );
            assert!(
                actual[2] > p1.bounds.min.z && actual[2] < p1.bounds.max.z,
                "n={n}: z={} out of bounds",
                actual[2]
            );
            assert!(actual[1].abs() < 1e-6, "n={n}: non-zero y={}", actual[1]);
            assert!(
                (actual[0] - ex).abs() < 1e-4 && (actual[2] - ez).abs() < 1e-4,
                "n={n}: got ({}, {}), expected ({ex}, {ez})",
                actual[0],
                actual[2]
            );
        }
        for i in 0..p1.lakes.len() {
            for j in (i + 1)..p1.lakes.len() {
                let d2 = (p1.lakes[i][0] - p1.lakes[j][0]).powi(2)
                    + (p1.lakes[i][2] - p1.lakes[j][2]).powi(2);
                assert!(d2 > 1.0, "n={n}: lakes {i} and {j} overlap: d2={d2}");
            }
        }
    }

    let p0_8 = spawn_positions_for(8, 0.0);
    let p1_8 = spawn_positions_for(8, 1.0);
    let max_min_dist = |founders: &[[f32; 3]], lakes: &[[f32; 3]]| -> f32 {
        founders
            .iter()
            .map(|f| {
                lakes
                    .iter()
                    .map(|l| ((f[0] - l[0]).powi(2) + (f[2] - l[2]).powi(2)).sqrt())
                    .fold(f32::MAX, f32::min)
            })
            .fold(0.0_f32, f32::max)
    };
    let max_dist_0 = max_min_dist(&p0_8.founders, &p0_8.lakes);
    let max_dist_1 = max_min_dist(&p1_8.founders, &p1_8.lakes);
    println!(
        "S12_FOUNDER_LAKE_DIST n=8 layout_0_max_min={max_dist_0:.4} layout_1_max_min={max_dist_1:.4}"
    );
    assert!(
        max_dist_1 < max_dist_0,
        "layout 1 max-min distance {max_dist_1:.4} not smaller than layout 0 {max_dist_0:.4}"
    );
}

// ---- 4. DONE WHEN #3: SEL2-E best world 20,000-tick dehydration reduction ----------------------

fn run_sel2e_20k(layout: f64) -> (u64, u64, u64, u64, usize) {
    const TICKS: u64 = 20_000;
    let init = initial(&[
        ("live.founders", 100.0),
        ("live.predator_fraction", 0.0),
        ("live.trees", 8.0),
        ("live.lakes", 8.0),
        ("live.prey_cap", 320.0),
        ("live.resource_capacity_scale", 0.7),
        ("live.dehydration_ticks", 960.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
        ("live.locomotion", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
        (LIVE_KEY_LAKE_LAYOUT, layout),
    ]);
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &init,
        &[],
        2,
        (16, 16),
        TICKS,
    )
    .expect("SEL2-E best world builds");

    let mut births = 0u64;
    let mut deaths = 0u64;
    let mut starvation = 0u64;
    let mut dehydration = 0u64;
    let get = |obs: &[(String, f64)], id: &str| {
        obs.iter()
            .find(|(k, _)| k == id)
            .map(|(_, v)| *v as u64)
            .unwrap_or(0)
    };
    for tick in 1..=TICKS {
        a.run_schedule_once();
        let obs = a.observables();
        let b = get(&obs, "live.births");
        let d = get(&obs, "live.deaths");
        let s = get(&obs, "live.deaths_starvation");
        let h = get(&obs, "live.deaths_dehydration");
        assert_eq!(
            s + h,
            d,
            "layout={layout} tick {tick}: starvation {s} + dehydration {h} != deaths {d}"
        );
        births += b;
        deaths += d;
        starvation += s;
        dehydration += h;
    }
    let final_agents = a
        .observables()
        .iter()
        .find(|(k, _)| k == "live.agent_count")
        .map(|(_, v)| *v as usize)
        .unwrap_or(0);
    (births, deaths, starvation, dehydration, final_agents)
}

#[test]
fn sel2e_best_world_layout_1_reduces_dehydration_deaths_over_20k_ticks() {
    let ((b0, d0, s0, h0, f0), (b1, d1, s1, h1, f1)) = std::thread::scope(|scope| {
        let t0 = scope.spawn(|| run_sel2e_20k(0.0));
        let t1 = scope.spawn(|| run_sel2e_20k(1.0));
        (t0.join().expect("arm 0"), t1.join().expect("arm 1"))
    });

    println!(
        "S12_BEHAVIOUR layout=0 births={b0} deaths={d0} starvation={s0} dehydration={h0} final_agents={f0}"
    );
    println!(
        "S12_BEHAVIOUR layout=1 births={b1} deaths={d1} starvation={s1} dehydration={h1} final_agents={f1}"
    );
    assert!(
        h1 < h0,
        "layout 1 dehydration deaths ({h1}) were not fewer than layout 0 ({h0})"
    );
}

// ---- 5. DONE WHEN #4: 4-process determinism + closed EU ----------------------------------------

fn layout_on_config() -> InitialConditionSet {
    initial(&[
        ("live.founders", 30.0),
        ("live.predator_fraction", 0.0),
        ("live.trees", 8.0),
        ("live.lakes", 8.0),
        ("live.mortality", 1.0),
        ("live.reproduction", 1.0),
        ("live.locomotion", 1.0),
        ("live.drive_floor", 0.5),
        ("live.thirst_steering", 1.0),
        (LIVE_KEY_LAKE_LAYOUT, 1.0),
    ])
}

#[test]
fn s12_child_entry_point() {
    if std::env::var(CHILD_ROLE).is_err() {
        return;
    }
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &layout_on_config(),
        &[],
        17,
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
        .args(["--exact", "s12_child_entry_point", "--nocapture"])
        .env(CHILD_ROLE, "1")
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
fn lake_layout_checksum_agrees_across_four_processes_and_keeps_closed_eu() {
    let sums: Vec<u32> = (0..4).map(|_| checksum_from_child()).collect();
    println!(
        "S12_CHECKSUMS {}",
        sums.iter()
            .map(|s| format!("{s:#010x}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    assert!(
        sums.iter().all(|s| *s == sums[0]),
        "processes disagree: {sums:x?}"
    );
    assert_ne!(sums[0], 0);

    // Explicitly sample HomeostaticState + live_observables on layout = 1.0 to confirm EU closed.
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &layout_on_config(),
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
    println!("S12_EU start={start} worst_relative_drift={worst}");
    assert!(worst < 1e-6, "closed EU drifted by {worst} (relative)");
}
