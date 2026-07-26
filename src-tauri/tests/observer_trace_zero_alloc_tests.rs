//! The observer trace is written from the tick path, so it may not allocate there.
//!
//! ADR-0004 lists this as a hard constraint rather than a nicety: recording runs 60 times a second
//! for as long as someone is watching, so an allocation here is not a one-off, it is a malloc/free
//! pair per tick forever on the simulation thread. The buffer is therefore sized once up front and
//! never grown — and when it fills, samples are *counted* rather than the buffer being reallocated
//! behind the caller's back.
//!
//! # One `#[test]` in this binary, on purpose
//!
//! The counter in `common::allocator` is process-wide. libtest gives every `#[test]` its own thread
//! and spawning those threads allocates *outside* any lock the test bodies share, so a sibling
//! starting mid-measurement is counted against the code under test. That is not hypothetical — see
//! the note in `tests/common/allocator.rs`, where `brain_budget_tests` failed about one run in three
//! for exactly this reason. One test means one thread means no siblings.

mod common;

use anima_engine_lib::core::observer::ObserverTrace;
use anima_engine_lib::core::simulation_lod::LodFocus;
use glam::Vec3;

#[global_allocator]
static ALLOCATOR: common::allocator::TrackingAllocator =
    common::allocator::TrackingAllocator::new();

const STEADY_TICKS: u64 = 5_000;

fn at(x: f32) -> LodFocus {
    LodFocus::at(Vec3::new(x, 0.0, 0.0))
}

#[test]
fn the_observer_trace_does_not_allocate_on_the_tick_path() {
    eprintln!("phase 1: a moving camera, every tick a change");
    a_moving_camera_allocates_nothing();

    eprintln!("phase 2: a still camera, every tick a no-op");
    a_still_camera_allocates_nothing();

    eprintln!("phase 3: a full buffer, every tick a counted drop");
    a_full_trace_allocates_nothing();
}

/// The worst realistic case: the observer pans continuously, so every tick is a real sample.
fn a_moving_camera_allocates_nothing() {
    let mut trace = ObserverTrace::with_capacity(STEADY_TICKS as usize + 1);
    // Pay for the buffer, and for the first push, before measuring.
    trace.record(0, at(-1.0));

    ALLOCATOR.start_tracking();
    for tick in 1..=STEADY_TICKS {
        trace.record(tick, at(tick as f32));
    }
    let allocs = ALLOCATOR.stop_tracking();

    assert_eq!(
        trace.len(),
        STEADY_TICKS as usize + 1,
        "the measurement is only meaningful if the samples were actually stored"
    );
    assert_eq!(
        allocs, 0,
        "recording {STEADY_TICKS} camera moves made {allocs} heap allocations"
    );
}

/// The common case: nobody is moving, so `record` should decide "no change" and return.
fn a_still_camera_allocates_nothing() {
    let mut trace = ObserverTrace::with_capacity(64);
    trace.record(0, at(3.0));

    ALLOCATOR.start_tracking();
    for tick in 1..=STEADY_TICKS {
        trace.record(tick, at(3.0));
    }
    let allocs = ALLOCATOR.stop_tracking();

    assert_eq!(trace.len(), 1, "an unchanged focus is not an event");
    assert_eq!(allocs, 0, "a still camera made {allocs} heap allocations");
}

/// The case a naive implementation gets wrong: a full buffer must count the overflow, **not** grow.
/// A `Vec::push` past capacity reallocates, which is precisely the allocation this gate exists to
/// catch — and it would happen only after an hour of watching, where nobody would look for it.
fn a_full_trace_allocates_nothing() {
    let mut trace = ObserverTrace::with_capacity(8);
    for tick in 0..8u64 {
        trace.record(tick, at(tick as f32));
    }
    assert_eq!(trace.len(), 8, "the buffer should be full before measuring");

    ALLOCATOR.start_tracking();
    for tick in 8..(8 + STEADY_TICKS) {
        trace.record(tick, at(tick as f32));
    }
    let allocs = ALLOCATOR.stop_tracking();

    assert_eq!(
        allocs, 0,
        "an overflowing trace made {allocs} heap allocations"
    );
    assert_eq!(
        trace.len(),
        8,
        "the buffer grew past the capacity it declared"
    );
    assert_eq!(
        trace.dropped(),
        STEADY_TICKS,
        "every sample past capacity must be counted, or a partial trace reads as a complete one"
    );
    assert!(trace.is_truncated());
}
