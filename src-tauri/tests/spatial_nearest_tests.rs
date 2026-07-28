//! `SpatialHashGrid::nearest` — the index that replaced the O(N²) target scan in `sensory_system`.
//!
//! The property that matters is not "it returns something near". It is that it returns **the same
//! entity an exhaustive scan would have returned**, because that scan is what it replaced and any
//! disagreement is a behaviour change nobody asked for. Every test here compares against a brute
//! force answer rather than against a hand-picked expectation.

use anima_engine_lib::core::ecs::MapBounds;
use anima_engine_lib::physics::SpatialHashGrid;
use bevy_ecs::prelude::*;
use glam::Vec3;

/// Deterministic scatter: the same points every run, spread across the default bounds.
fn scatter(n: usize) -> Vec<Vec3> {
    (0..n)
        .map(|i| {
            let a = (i as f32) * 2.399_963;
            let r = 95.0 * ((i as f32 + 1.0) / (n as f32)).sqrt();
            Vec3::new(r * a.cos(), 0.0, r * a.sin())
        })
        .collect()
}

fn build(points: &[Vec3]) -> (SpatialHashGrid, MapBounds, Vec<Entity>, World) {
    let bounds = MapBounds::default();
    let mut grid = SpatialHashGrid::new_prepopulated(10.0, &bounds);
    let mut world = World::new();
    let entities: Vec<Entity> = points.iter().map(|_| world.spawn(()).id()).collect();
    for (e, p) in entities.iter().zip(points) {
        grid.insert(*p, &bounds, *e);
    }
    (grid, bounds, entities, world)
}

/// Brute force: what the code did before the index existed.
fn brute(from: Vec3, points: &[Vec3], entities: &[Entity], radius: f32) -> Option<Entity> {
    let mut best = None;
    let mut best_d = radius * radius;
    for (e, p) in entities.iter().zip(points) {
        let d = from.distance_squared(*p);
        if d < best_d {
            best_d = d;
            best = Some(*e);
        }
    }
    best
}

#[test]
fn agrees_with_an_exhaustive_scan_from_many_origins() {
    let points = scatter(400);
    let (grid, bounds, entities, _w) = build(&points);
    let index: std::collections::HashMap<Entity, Vec3> = entities
        .iter()
        .copied()
        .zip(points.iter().copied())
        .collect();

    for probe in 0..64 {
        let from = Vec3::new(
            -90.0 + (probe % 8) as f32 * 24.0,
            0.0,
            -90.0 + (probe / 8) as f32 * 24.0,
        );
        let expected = brute(from, &points, &entities, 60.0);
        let got = grid
            .nearest(from, 60.0, &bounds, |e| index.get(&e).copied())
            .map(|(e, _)| e);
        assert_eq!(got, expected, "disagreed with the scan at {from:?}");
    }
}

#[test]
fn respects_the_radius_rather_than_returning_the_globally_nearest() {
    // The radius is the behaviour change the index introduces, and it has to be real: an agent with
    // nothing in range must get nothing, not the nearest thing on the far side of the map.
    let far = vec![Vec3::new(90.0, 0.0, 90.0)];
    let (grid, bounds, entities, _w) = build(&far);
    let index: std::collections::HashMap<Entity, Vec3> =
        entities.iter().copied().zip(far.iter().copied()).collect();

    let origin = Vec3::new(-90.0, 0.0, -90.0);
    assert!(grid
        .nearest(origin, 30.0, &bounds, |e| index.get(&e).copied())
        .is_none());
    assert!(grid
        .nearest(origin, 400.0, &bounds, |e| index.get(&e).copied())
        .is_some());
}

#[test]
fn the_filter_decides_what_counts() {
    // `accept` is how the caller says "prey with energy left" or "food that still exists". A
    // rejected entity must not shadow a valid one further away.
    let points = vec![Vec3::new(0.0, 0.0, 2.0), Vec3::new(0.0, 0.0, 20.0)];
    let (grid, bounds, entities, _w) = build(&points);
    let near = entities[0];
    let far = entities[1];

    let got = grid
        .nearest(Vec3::ZERO, 60.0, &bounds, |e| {
            if e == near {
                None // the close one does not qualify
            } else {
                Some(points[1])
            }
        })
        .map(|(e, _)| e);
    assert_eq!(got, Some(far));
}

#[test]
fn an_empty_world_and_a_degenerate_radius_return_nothing() {
    let (grid, bounds, _e, _w) = build(&[]);
    assert!(grid
        .nearest(Vec3::ZERO, 60.0, &bounds, |_| Some(Vec3::ZERO))
        .is_none());

    let points = vec![Vec3::new(1.0, 0.0, 1.0)];
    let (grid, bounds, entities, _w) = build(&points);
    let index: std::collections::HashMap<Entity, Vec3> = entities
        .iter()
        .copied()
        .zip(points.iter().copied())
        .collect();
    for bad in [0.0, -5.0, f32::NAN] {
        assert!(grid
            .nearest(Vec3::ZERO, bad, &bounds, |e| index.get(&e).copied())
            .is_none());
    }
    assert!(grid
        .nearest(Vec3::new(f32::NAN, 0.0, 0.0), 60.0, &bounds, |e| index
            .get(&e)
            .copied())
        .is_none());
}

#[test]
fn visits_far_fewer_candidates_than_the_population() {
    // The point of the index. Counting `accept` calls measures the work done: a scan touches every
    // entity, and if this did too it would be a slower way to lose.
    let points = scatter(2000);
    let (grid, bounds, entities, _w) = build(&points);
    let index: std::collections::HashMap<Entity, Vec3> = entities
        .iter()
        .copied()
        .zip(points.iter().copied())
        .collect();

    let mut visited = 0usize;
    let got = grid.nearest(Vec3::new(10.0, 0.0, -10.0), 60.0, &bounds, |e| {
        visited += 1;
        index.get(&e).copied()
    });
    assert!(got.is_some(), "there is a target within 60 units");
    assert!(
        visited < points.len() / 4,
        "touched {visited} of {} candidates — the early exit is not working",
        points.len()
    );
}
