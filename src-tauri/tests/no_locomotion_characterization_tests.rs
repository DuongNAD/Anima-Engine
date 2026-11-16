//! F-042 — creatures cannot translate. Joint spring forces are internal (`child.force += f;
//! parent.force -= f`), `integrate_physics_system` has no ground contact, friction or drag, and
//! nothing in `src/` ever writes `InertiaComponent.target_velocity`, the only external force term.
//! So the CPG oscillates the segments but the root never goes anywhere.
//!
//! CHARACTERIZATION: this test PINS TODAY'S DEFECT and passes today. When locomotion exists
//! (anisotropic ground friction and/or a steering drive), it must be INVERTED: founders should
//! then cover a distance well above the bound below.
//!
//! The world is the default live manifest (no mortality, no reproduction, so no births) built
//! through `LiveExperimentAdapter::from_manifest` and driven by the app's own schedule.

use anima_engine_lib::core::components::{Agent, ParentLink, Position, Segment, Velocity};
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::LiveExperimentAdapter;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::*;
use std::collections::HashMap;

const SEED: u64 = 1;
const TICKS: u32 = 1_000;

fn build() -> LiveExperimentAdapter {
    LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &InitialConditionSet::new(vec![]),
        &[],
        SEED,
        (16, 16),
        0,
    )
    .expect("the default live world builds")
}

fn roots(world: &mut World) -> HashMap<Entity, [f32; 3]> {
    let mut q = world.query_filtered::<(Entity, &Position), (With<Agent>, Without<ParentLink>)>();
    q.iter(world)
        .map(|(e, p)| (e, [p.0.x, p.0.y, p.0.z]))
        .collect()
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn founders_oscillate_but_do_not_translate() {
    let mut adapter = build();
    let start = roots(&mut adapter.world());
    assert!(!start.is_empty(), "the world must have founders");
    let mut last = start.clone();
    let mut path: HashMap<Entity, f32> = start.keys().map(|e| (*e, 0.0)).collect();
    let mut seg_speed_sum = 0.0f64;

    for _ in 0..TICKS {
        adapter.run_schedule_once();
        let mut world = adapter.world();
        let now = roots(&mut world);
        for (e, p) in &now {
            if let Some(l) = last.get(e) {
                *path.entry(*e).or_insert(0.0) += dist(*l, *p);
            }
        }
        last = now;
        let mut q = world.query_filtered::<&Velocity, With<Segment>>();
        seg_speed_sum += q.iter(&world).map(|v| v.0.length() as f64).sum::<f64>();
    }

    let mut max_net = 0.0f32;
    let mut path_sum = 0.0f32;
    let mut n = 0usize;
    for (e, s) in &start {
        let Some(end) = last.get(e) else { continue };
        max_net = max_net.max(dist(*s, *end));
        path_sum += path[e];
        n += 1;
    }
    assert!(n > 0, "founders must survive the run (no mortality)");
    let mean_path = path_sum / n as f32;
    println!("F042 ticks={TICKS} founders={n} max_net={max_net} mean_path={mean_path}");

    assert!(
        seg_speed_sum > 1.0,
        "the CPG must be demonstrably active (summed segment speed {seg_speed_sum})"
    );
    assert!(
        max_net < 0.5,
        "F-042: founders do not translate (max net displacement {max_net} over {TICKS} ticks). \
         If this fails, locomotion exists: invert this test."
    );
}
