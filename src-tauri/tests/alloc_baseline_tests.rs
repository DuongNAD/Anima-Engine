//! Card P2 ALLOC-BASELINE — Heap allocation baseline through `LiveExperimentAdapter` on the live schedule.
//!
//! Measures total heap allocations per 120-tick window (after a 60-tick warm-up) across ~100
//! agents under [`LiveExperimentAdapter`], comparing shared `BrainModel` inference (`batch_size > 0`,
//! `live.evolved_brains = 0.0`) against the per-agent `AgentBrain` negative control (`batch_size == 0`,
//! `live.evolved_brains = 1.0`).
//!
//! Per `src-tauri/tests/common/allocator.rs`, the live schedule may invoke worker threads / rayon,
//! so this suite keeps process-wide `ALLOCATOR.start_tracking()` and places every phase inside one
//! `#[test]`. Process-wide tracking can include a few allocations from libtest's main thread; we
//! report the median of 5 runs without gating on the baseline count, and verify only that the
//! `AgentBrain` negative control drops clearly beyond that harness noise.

mod common;

use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::live_experiment::{LiveExperimentAdapter, LIVE_KEY_EVOLVED_BRAINS};
use std::sync::Mutex;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

const AGENT_COUNT: usize = 100;
const PREDATOR_FRACTION: f64 = 0.30;
const WARMUP_TICKS: usize = 60;
const MEASURED_TICKS: usize = 120;
const NUM_RUNS: usize = 5;
const SEEDS: [u64; NUM_RUNS] = [9_001_001, 9_001_002, 9_001_003, 9_001_004, 9_001_005];

fn live_agent_count(adapter: &LiveExperimentAdapter) -> f64 {
    ExperimentModel::observables_unsampled(adapter)
        .into_iter()
        .find(|(k, _)| k == "live.agent_count")
        .map(|(_, v)| v)
        .expect("live.agent_count observable present")
}

fn measure_run(seed: u64, evolved_brains: bool) -> usize {
    let initial = InitialConditionSet::new(vec![
        ("live.founders".to_string(), AGENT_COUNT as f64),
        ("live.predator_fraction".to_string(), PREDATOR_FRACTION),
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
        seed,
        (256, 256),
        100_000,
    )
    .expect("P2 live manifest must build");

    assert_eq!(
        live_agent_count(&adapter),
        AGENT_COUNT as f64,
        "agent count before warm-up must match target"
    );

    for _ in 0..WARMUP_TICKS {
        adapter.run_schedule_once();
    }

    // Keeps process-wide tracking because the live Bevy schedule can involve worker threads
    // (`common/allocator.rs`).
    ALLOCATOR.start_tracking();
    for _ in 0..MEASURED_TICKS {
        adapter.run_schedule_once();
    }
    let allocs = ALLOCATOR.stop_tracking();

    assert_eq!(
        live_agent_count(&adapter),
        AGENT_COUNT as f64,
        "agent count must stay fixed (mortality=0, reproduction=0)"
    );

    allocs
}

fn median_usize(values: &[usize]) -> usize {
    assert!(!values.is_empty());
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

#[test]
fn measure_live_schedule_allocations_shared_vs_evolved_brains() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let mut shared_runs = Vec::with_capacity(NUM_RUNS);
    let mut evolved_runs = Vec::with_capacity(NUM_RUNS);

    for &seed in &SEEDS {
        let shared_allocs = measure_run(seed, false);
        let evolved_allocs = measure_run(seed, true);
        println!(
            "P2_RUN seed={seed} shared_allocs_120ticks={shared_allocs} ({:.2}/tick) evolved_allocs_120ticks={evolved_allocs} ({:.2}/tick)",
            shared_allocs as f64 / MEASURED_TICKS as f64,
            evolved_allocs as f64 / MEASURED_TICKS as f64,
        );
        shared_runs.push(shared_allocs);
        evolved_runs.push(evolved_allocs);
    }

    let shared_median = median_usize(&shared_runs);
    let evolved_median = median_usize(&evolved_runs);
    let drop_median = shared_median.saturating_sub(evolved_median);

    println!(
        "P2_MEDIAN runs={NUM_RUNS} ticks={MEASURED_TICKS} shared_median={shared_median} ({:.2}/tick) evolved_median={evolved_median} ({:.2}/tick) drop={drop_median} ({:.2}/tick)",
        shared_median as f64 / MEASURED_TICKS as f64,
        evolved_median as f64 / MEASURED_TICKS as f64,
        drop_median as f64 / MEASURED_TICKS as f64,
    );

    // Do NOT gate on the baseline count (`shared_median`), which reflects current hot-path sites
    // (`src/ai/model.rs:349-354`: `scratch.inputs.clone()`, `TensorData::new`, `Tensor::from_data`,
    // `actor_out.into_data().try_into_vec()`) plus possible single-digit libtest main-thread noise.
    // Negative control: giving every agent an `AgentBrain` empties the shared batch (`batch_size == 0`)
    // and skips the shared Burn forward pass, producing a drop well beyond libtest main-thread noise.
    assert!(
        shared_median > evolved_median + MEASURED_TICKS,
        "negative control expected evolved_brains=1.0 to drop heap allocations well beyond \
         harness noise (> {MEASURED_TICKS} over {MEASURED_TICKS} ticks): shared_median={shared_median}, \
         evolved_median={evolved_median}"
    );
}
