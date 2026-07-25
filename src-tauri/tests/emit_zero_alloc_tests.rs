//! The IPC emit path must not allocate once it reaches steady state.
//!
//! This path runs ~30 times a second for the life of the process, so an allocation here is not a
//! one-off — it is 30 malloc/free pairs per second, forever, on the thread that feeds the UI. The
//! version this pins replaced one that fought its own pre-allocation: it filled a
//! `Vec::with_capacity(1000)` and then `.clone()`d it into a freshly constructed payload, allocating
//! the exact Vec the capacity hint existed to avoid, cloned the environmental state twice over, and
//! built a fresh `HashMap` with no capacity hint every frame.
//!
//! `simulation_loop.rs` carries no `#[cfg(test)]` block at all, which is why the logic under test
//! was lifted into `core::emit` as plain functions first — the emit thread itself needs a Tauri
//! `AppHandle` and cannot be driven from a test.

mod common;

use anima_engine_lib::ai::pheromone::{PheromoneGridState, CELL_COUNT, GRID_SIZE};
use anima_engine_lib::core::components::{EnvironmentalElement, EnvironmentalState};
use anima_engine_lib::core::emit::{new_tick_payload, refresh_tick_payload, PheromoneEmitGate};
use anima_engine_lib::core::simulation_state::SegmentState;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn segment(agent_id: u32, segment_id: u32, parent: Option<u32>) -> SegmentState {
    SegmentState {
        agent_id,
        segment_id,
        parent_segment_id: parent,
        x: 1.0,
        y: 2.0,
        z: 3.0,
        yaw: 0.1,
        pitch: 0.2,
        roll: 0.3,
        joint_anchor_x: 0.0,
        joint_anchor_y: 0.0,
        joint_anchor_z: 0.0,
        joint_axis_x: 0.0,
        joint_axis_y: 1.0,
        joint_axis_z: 0.0,
        energy: 50.0,
        hydration: 50.0,
        head_direction: [1.0, 0.0, 0.0],
        agent_type: None,
    }
}

/// 200 agents, 5 segments each — a full-looking frame, well inside the payload's starting capacity.
fn frame_segments() -> Vec<SegmentState> {
    let mut segments = Vec::with_capacity(1000);
    for agent in 0..200u32 {
        segments.push(segment(agent, 0, None));
        for seg in 1..5u32 {
            segments.push(segment(agent, seg, Some(seg - 1)));
        }
    }
    segments
}

fn environmental_state() -> EnvironmentalState {
    EnvironmentalState {
        elements: (0..300)
            .map(|i| EnvironmentalElement {
                element_type: "tree".to_string(),
                x: i as f32,
                y: i as f32,
                radius: 1.5,
                resources: 50.0,
            })
            .collect(),
    }
}

fn empty_grid() -> PheromoneGridState {
    PheromoneGridState {
        grid: vec![0.0; CELL_COUNT],
        width: GRID_SIZE as u32,
        height: GRID_SIZE as u32,
    }
}

#[test]
fn tick_payload_refresh_is_allocation_free_in_steady_state() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let segments = frame_segments();
    let env = environmental_state();
    let mut payload = new_tick_payload();

    // Warm up: the first frames grow the head-direction table and the environmental-element Vec to
    // their working size. Steady state is what this test is about, not the first frame.
    for _ in 0..10 {
        refresh_tick_payload(&mut payload, &segments, &env);
    }

    ALLOCATOR.start_tracking();
    for _ in 0..100 {
        refresh_tick_payload(&mut payload, &segments, &env);
    }
    let allocs = ALLOCATOR.stop_tracking();

    assert_eq!(
        allocs, 0,
        "100 emit frames should reuse their buffers, but made {allocs} heap allocations"
    );
}

#[test]
fn refresh_still_allocates_nothing_when_the_population_changes_size() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Agents die and are born every epoch, so the segment count moves between frames. Shrinking
    // must not free the buffer and growing back must not re-allocate it — that would put the cost
    // back on every epoch boundary instead of every frame.
    let full = frame_segments();
    let half = &full[..full.len() / 2];
    let env = environmental_state();
    let mut payload = new_tick_payload();

    for _ in 0..10 {
        refresh_tick_payload(&mut payload, &full, &env);
        refresh_tick_payload(&mut payload, half, &env);
    }

    ALLOCATOR.start_tracking();
    for _ in 0..50 {
        refresh_tick_payload(&mut payload, &full, &env);
        refresh_tick_payload(&mut payload, half, &env);
    }
    let allocs = ALLOCATOR.stop_tracking();

    assert_eq!(
        allocs, 0,
        "alternating population size should reuse buffers, but made {allocs} heap allocations"
    );
}

#[test]
fn pheromone_gate_is_allocation_free_whether_or_not_it_sends() {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let start = Instant::now();
    let interval = Duration::from_millis(100);
    let mut gate = PheromoneEmitGate::new(CELL_COUNT, interval, start);
    let mut shared = empty_grid();
    let mut out = empty_grid();

    // Warm up past the first, unconditional send.
    for frame in 1..=5u64 {
        shared.grid[0] = frame as f32;
        gate.poll(&shared, &mut out, start + Duration::from_millis(33 * frame));
    }

    ALLOCATOR.start_tracking();
    let mut sent = 0;
    for frame in 6..=200u64 {
        // Changed every frame, so both branches of the gate are exercised: the rate limit rejects
        // most polls and the copy path runs on the rest.
        shared.grid[(frame as usize) % CELL_COUNT] = frame as f32;
        if gate.poll(&shared, &mut out, start + Duration::from_millis(33 * frame)) {
            sent += 1;
        }
    }
    let allocs = ALLOCATOR.stop_tracking();

    assert!(sent > 0, "the gate must still send a changing field");
    assert_eq!(
        allocs, 0,
        "the pheromone gate should compare and copy in place, but made {allocs} heap allocations"
    );
}
