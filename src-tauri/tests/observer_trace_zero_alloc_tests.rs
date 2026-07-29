//! The observer trace is written from the tick path, so it may not allocate there.
//!
//! ADR-0004 lists this as a hard constraint rather than a nicety: recording runs 60 times a second
//! for as long as someone is watching, so an allocation here is not a one-off, it is a malloc/free
//! pair per tick forever on the simulation thread. The buffer is therefore sized once up front and
//! never grown — and when it fills, samples are *counted* rather than the buffer being reallocated
//! behind the caller's back.
//!
//! # One `#[test]` in this binary, and a thread-scoped counter
//!
//! `common::allocator`'s counter is process-wide by default. libtest gives every `#[test]` its own
//! thread and spawning those threads allocates *outside* any lock the test bodies share, so a
//! sibling starting mid-measurement is counted against the code under test — see the note in
//! `tests/common/allocator.rs`, where `brain_budget_tests` failed about one run in three for exactly
//! this reason. One `#[test]` means one test thread means no siblings.
//!
//! That was not enough. On 2026-07-29 this binary — already a single `#[test]` — failed a full
//! `cargo test --features desktop` run with two allocations in phase 1, and passed 3/3 when run on
//! its own. libtest's *main* thread is alive for the whole run and allocates as it handles harness
//! events; no arrangement of the tests in here can remove it.
//!
//! So the measurements below use `start_tracking_this_thread`. Everything they measure —
//! `ObserverTrace::record` writing into a pre-allocated buffer — happens on the calling thread and
//! hands nothing to another, which is the precondition for that call being honest rather than merely
//! quiet.

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

    eprintln!("phase 4: control — the counter is not simply blind");
    the_counter_still_sees_a_real_allocation();

    eprintln!("phase 5: control — and it is blind to OTHER threads, deliberately");
    another_threads_allocations_are_not_counted();
}

/// Control: a counter that always returned 0 would make every assertion above pass.
///
/// Phases 1-3 all assert `allocs == 0`, so switching them to a thread-scoped counter could have
/// turned three real gates into three tautologies and nothing would have gone red. This proves the
/// narrower counter still counts.
fn the_counter_still_sees_a_real_allocation() {
    ALLOCATOR.start_tracking_this_thread();
    std::hint::black_box(Vec::<u8>::with_capacity(64));
    let allocs = ALLOCATOR.stop_tracking();

    assert!(
        allocs >= 1,
        "the thread-scoped counter saw {allocs} allocations for a heap Vec — it is not counting at \
         all, which would make every zero-alloc assertion in this file meaningless"
    );
}

/// Control: the other half of the trade, stated as a test rather than left in a comment.
///
/// `start_tracking_this_thread` is what makes this suite immune to libtest's main thread. The price
/// is that work handed to another thread is invisible, which is exactly why
/// `map_generation_zero_alloc_tests` (rayon) must keep the process-wide `start_tracking`. Pinning it
/// here means the limitation cannot be forgotten and then relied on by accident.
fn another_threads_allocations_are_not_counted() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let go = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    let (worker_go, worker_done) = (Arc::clone(&go), Arc::clone(&done));

    // Spawned BEFORE the window opens: `thread::spawn` allocates on the *calling* thread, so
    // spawning inside the measurement would be counted and this control would prove nothing.
    let worker = std::thread::spawn(move || {
        while !worker_go.load(Ordering::SeqCst) {
            std::hint::spin_loop();
        }
        for _ in 0..1_000 {
            std::hint::black_box(Vec::<u8>::with_capacity(64));
        }
        worker_done.store(true, Ordering::SeqCst);
    });

    ALLOCATOR.start_tracking_this_thread();
    go.store(true, Ordering::SeqCst);
    // Spin rather than `recv()` or `park()`: blocking primitives allocate, and this thread's
    // allocations are the ones being counted.
    while !done.load(Ordering::SeqCst) {
        std::hint::spin_loop();
    }
    let allocs = ALLOCATOR.stop_tracking();

    worker.join().expect("control worker thread panicked");

    assert_eq!(
        allocs, 0,
        "a thread-scoped measurement counted {allocs} allocations made by another thread"
    );
}

/// The worst realistic case: the observer pans continuously, so every tick is a real sample.
fn a_moving_camera_allocates_nothing() {
    let mut trace = ObserverTrace::with_capacity(STEADY_TICKS as usize + 1);
    // Pay for the buffer, and for the first push, before measuring.
    trace.record(0, at(-1.0));

    ALLOCATOR.start_tracking_this_thread();
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

    ALLOCATOR.start_tracking_this_thread();
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

    ALLOCATOR.start_tracking_this_thread();
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
