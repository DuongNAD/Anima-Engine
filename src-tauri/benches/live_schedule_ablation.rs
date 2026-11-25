//! Card P1 PERF-ABLATION — Live schedule ablation grid across N, predator fraction, and evolved brains.
//!
//! Runs [`LiveExperimentAdapter`] across:
//!   N in {100, 1_000, 2_048, 4_096}
//!   x predator_fraction in {0.30, 0.0}
//!   x evolved_brains in {0.0, 1.0}
//!
//! Tests whether the all-pairs predator x prey loops (`agent_systems.rs:1747`,
//! `world_systems.rs:461-463`) drive the scaling knee when predators = 0.

use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{LiveExperimentAdapter, LIVE_KEY_EVOLVED_BRAINS};

const SEED: u64 = 20_261_125;
const WORLD_WARMUP_TICKS: usize = 20;
const AGENT_COUNTS: [usize; 4] = [100, 1_000, 2_048, 4_096];
const PREDATOR_FRACTIONS: [f64; 2] = [0.30, 0.0];
const EVOLVED_ARMS: [bool; 2] = [false, true];

fn observable_value(adapter: &LiveExperimentAdapter, name: &str) -> f64 {
    ExperimentModel::observables_unsampled(adapter)
        .into_iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v)
        .unwrap_or_else(|| panic!("{name} observable present"))
}

fn build_ablation_adapter(
    count: usize,
    predator_fraction: f64,
    evolved_brains: bool,
) -> LiveExperimentAdapter {
    let initial = InitialConditionSet::new(vec![
        ("live.founders".to_string(), count as f64),
        ("live.predator_fraction".to_string(), predator_fraction),
        ("live.trees".to_string(), 8.0),
        ("live.lakes".to_string(), 2.0),
        ("live.food_cap".to_string(), 50.0),
        ("live.mortality".to_string(), 0.0),
        ("live.reproduction".to_string(), 0.0),
        ("live.locomotion".to_string(), 1.0),
        ("live.drive_floor".to_string(), 0.5),
        ("live.thirst_steering".to_string(), 1.0),
        (
            LIVE_KEY_EVOLVED_BRAINS.to_string(),
            if evolved_brains { 1.0 } else { 0.0 },
        ),
    ]);

    let mut adapter = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &initial,
        &[],
        SEED,
        (256, 256),
        100_000,
    )
    .expect("P1 ablation manifest builds cleanly");

    for _ in 0..WORLD_WARMUP_TICKS {
        adapter.run_schedule_once();
    }
    adapter
}

fn bench_live_schedule_ablation(c: &mut Criterion) {
    let mut group = c.benchmark_group("live_schedule_ablation");
    group.warm_up_time(Duration::from_secs(2));

    for &pred in &PREDATOR_FRACTIONS {
        for &evolved in &EVOLVED_ARMS {
            let arm = format!(
                "pred{}_evolved{}",
                if pred > 0.0 { "0.3" } else { "0" },
                usize::from(evolved)
            );
            for &count in &AGENT_COUNTS {
                let mut adapter = build_ablation_adapter(count, pred, evolved);
                assert_eq!(
                    observable_value(&adapter, "live.agent_count"),
                    count as f64,
                    "agent count must match target prior to benchmark for {arm}/{count}"
                );
                if pred == 0.0 {
                    assert_eq!(
                        observable_value(&adapter, "live.predator_count"),
                        0.0,
                        "predator_count must be 0 when predator_fraction=0 for {arm}/{count}"
                    );
                }

                if count <= 1_000 {
                    group.sample_size(20);
                    group.measurement_time(Duration::from_secs(6));
                } else {
                    group.sample_size(15);
                    group.measurement_time(Duration::from_secs(8));
                }

                group.bench_with_input(BenchmarkId::new(&arm, count), &count, |b, _| {
                    b.iter(|| {
                        adapter.run_schedule_once();
                        black_box(adapter.tick());
                    });
                });

                assert_eq!(
                    observable_value(&adapter, "live.agent_count"),
                    count as f64,
                    "agent count drifted during measurement for {arm}/{count}"
                );
            }
        }
    }

    group.finish();
}

criterion_group!(benches, bench_live_schedule_ablation);
criterion_main!(benches);
