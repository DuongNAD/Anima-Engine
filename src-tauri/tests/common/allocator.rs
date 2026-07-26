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
// counted as if the code under test had done it. A `TEST_LOCK` mutex — the convention across these
// suites — serialises the test *bodies*, but libtest gives each `#[test]` its own thread and
// spawning those threads allocates outside the lock. On an idle machine every thread is up before
// the first body starts measuring and nothing is observed; under the load of a full
// `cargo test --features desktop` they interleave and a sibling's start-up lands mid-window.
//
// This is not hypothetical. `brain_budget_tests::a_learning_step_allocates_nothing` failed about one
// run in three with "made 4 heap allocations" in a function that makes none, and a new emit-path
// suite failed the same way on its first full run while passing in isolation every time.
//
// The fix that costs nothing: give the binary a single `#[test]` that calls each measurement in
// sequence, with an `eprintln!` per phase so a failure still says which one broke. One test means one
// thread means no siblings. `brain_budget_tests.rs` and `emit_zero_alloc_tests.rs` are written that
// way. The remaining suites still have one `#[test]` per measurement and are latent — they have not
// lost the race yet.
#![allow(dead_code)]

use std::alloc::{GlobalAlloc, Layout, System};
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
