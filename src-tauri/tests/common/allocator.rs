// This module is compiled into every integration-test binary that pulls in `common`, but only the
// zero-allocation suites install the allocator. In the others nothing here is constructed, which is
// expected rather than dead code worth removing.
//
// # Put every measurement in ONE `#[test]` per binary
//
// The counter below is process-wide, and deliberately so: `terrain.rs` runs erosion on rayon
// workers, so a per-thread counter would stop counting the very allocations
// `map_generation_zero_alloc_tests` exists to catch.
//
// The cost of that choice is that anything else allocating in this process during a measurement is
// counted as if the code under test had done it. A `TEST_LOCK` mutex serialises test *bodies*, but
// libtest gives each `#[test]` its own thread and spawning those threads allocates outside the lock.
// Backend threads can also finish lazy start-up after a warm-up returns. On an idle machine that
// activity usually finishes before tracking starts; under load it can land inside the window.
//
// This is not hypothetical. `brain_budget_tests::a_learning_step_allocates_nothing` failed about one
// run in three with "made 4 heap allocations" in a function that makes none, and a new emit-path
// suite failed the same way on its first full run while passing in isolation every time.
//
// There are two honest fixes. A hot path that owns worker threads must keep this process-wide
// counter and give its binary one `#[test]` that runs phases in sequence; `brain_budget_tests.rs`,
// `emit_zero_alloc_tests.rs`, and `challenger_meta_ai_tests.rs` do that. A structurally
// single-threaded hot path should use `ThreadTrackingAllocator`, which excludes unrelated process
// threads without hiding work the path owns. The remaining multi-test process-wide suites are
// latent until they adopt the first shape.
#![allow(dead_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub struct TrackingAllocator {
    alloc_count: AtomicUsize,
    active: AtomicBool,
}

impl TrackingAllocator {
    pub const fn new() -> Self {
        Self {
            alloc_count: AtomicUsize::new(0),
            active: AtomicBool::new(false),
        }
    }

    pub fn start_tracking(&self) {
        self.alloc_count.store(0, Ordering::SeqCst);
        self.active.store(true, Ordering::SeqCst);
    }

    pub fn stop_tracking(&self) -> usize {
        self.active.store(false, Ordering::SeqCst);
        self.alloc_count.load(Ordering::SeqCst)
    }
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if self.active.load(Ordering::SeqCst) {
            self.alloc_count.fetch_add(1, Ordering::SeqCst);
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

thread_local! {
    static THREAD_TRACKING_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static THREAD_ALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
}

/// Counts allocations made by the thread that opened the measurement window.
///
/// Use this only when the hot path is structurally single-threaded. It keeps libtest, backend, and
/// other process threads from contaminating that measurement under machine load. Code that owns
/// worker-thread allocations must keep using [`TrackingAllocator`] so those allocations remain
/// visible.
pub struct ThreadTrackingAllocator;

impl ThreadTrackingAllocator {
    pub const fn new() -> Self {
        Self
    }

    pub fn start_tracking(&self) {
        THREAD_ALLOCATION_COUNT.with(|count| count.set(0));
        THREAD_TRACKING_ACTIVE.with(|active| active.set(true));
    }

    pub fn stop_tracking(&self) -> usize {
        THREAD_TRACKING_ACTIVE.with(|active| active.set(false));
        THREAD_ALLOCATION_COUNT.with(Cell::get)
    }
}

unsafe impl GlobalAlloc for ThreadTrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let tracking = THREAD_TRACKING_ACTIVE.try_with(Cell::get).unwrap_or(false);
        if tracking {
            let _ = THREAD_ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}
