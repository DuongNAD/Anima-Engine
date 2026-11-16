//! (f) `steering_drive_system` allocates nothing in steady state (CLAUDE.md: zero heap allocation
//! in tick systems). The system runs alone, on a world warmed by the real schedule, with the
//! drive enabled and agents that have targets.

mod common;

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::LiveExperimentAdapter;
use bevy_ecs::prelude::*;

#[global_allocator]
static ALLOCATOR: common::allocator::ThreadTrackingAllocator =
    common::allocator::ThreadTrackingAllocator::new();

#[test]
fn steering_drive_system_allocates_nothing_once_warm() {
    let init = InitialConditionSet::new(vec![("live.locomotion".to_string(), 1.0)]);
    let mut a =
        LiveExperimentAdapter::from_manifest(&WorldLawSet::baseline(), &init, &[], 1, (16, 16), 0)
            .expect("builds");
    for _ in 0..50 {
        a.run_schedule_once();
    }
    let mut only = Schedule::default();
    only.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    only.add_systems(anima_engine_lib::core::locomotion::steering_drive_system);
    let mut world = a.world();
    for _ in 0..10 {
        only.run(&mut world);
    }
    ALLOCATOR.start_tracking();
    for _ in 0..200 {
        only.run(&mut world);
    }
    let allocations = ALLOCATOR.stop_tracking();
    assert_eq!(
        allocations, 0,
        "steering_drive_system allocated {allocations} times"
    );
}

#[test]
fn steering_drive_system_allocates_nothing_with_floor_and_thirst_on() {
    let init = InitialConditionSet::new(vec![
        ("live.locomotion".to_string(), 1.0),
        ("live.drive_floor".to_string(), 0.5),
        ("live.thirst_steering".to_string(), 1.0),
    ]);
    let mut a =
        LiveExperimentAdapter::from_manifest(&WorldLawSet::baseline(), &init, &[], 1, (16, 16), 0)
            .expect("builds");
    for _ in 0..50 {
        a.run_schedule_once();
    }
    let mut world = a.world();
    // Make every agent thirsty so the lake branch is the one being measured.
    {
        let mut q = world.query::<&mut anima_engine_lib::ai::hrrl::HomeostaticState>();
        for mut h in q.iter_mut(&mut world) {
            h.hydration = h.hydration_target * 0.1;
        }
    }
    let mut only = Schedule::default();
    only.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    only.add_systems(anima_engine_lib::core::locomotion::steering_drive_system);
    for _ in 0..10 {
        only.run(&mut world);
    }
    ALLOCATOR.start_tracking();
    for _ in 0..200 {
        only.run(&mut world);
    }
    let allocations = ALLOCATOR.stop_tracking();
    assert_eq!(
        allocations, 0,
        "steering_drive_system allocated {allocations} times with floor + thirst on"
    );
}
