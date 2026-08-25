//! PF-S01 evidence probe (branch H): prints the live-world checksum after a fixed number of
//! ticks at a fixed seed, so the pre-refactor and post-refactor trees can be compared bit for
//! bit. Not a pass/fail gate by itself — the assertion lives in the comparison of two runs.

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::LiveExperimentAdapter;

#[test]
fn print_checksum_for_pf_s01_comparison() {
    let initials = InitialConditionSet::new(vec![
        ("live.founders".to_string(), 8.0),
        ("live.predator_fraction".to_string(), 0.3),
        ("live.trees".to_string(), 6.0),
        ("live.lakes".to_string(), 2.0),
        ("live.food_cap".to_string(), 50.0),
    ]);
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initials,
        &[],
        0x5EED,
        (16, 16),
        0,
    )
    .expect("the live world builds");
    for _ in 0..1_500u32 {
        a.run_schedule_once();
    }
    let mut world = a.world();
    let checksum = anima_engine_lib::core::snapshot::world_checksum(&mut world);
    println!("PF_S01_CHECKSUM={checksum}");
}
