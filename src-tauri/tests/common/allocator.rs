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
<<<<<<< ours
// There are two honest fixes. A hot path that owns worker threads must keep this process-wide
// counter and give its binary one `#[test]` that runs phases in sequence; `brain_budget_tests.rs`,
// `emit_zero_alloc_tests.rs`, and `challenger_meta_ai_tests.rs` do that. A structurally
// single-threaded hot path should use `ThreadTrackingAllocator`, which excludes unrelated process
// threads without hiding work the path owns. The remaining multi-test process-wide suites are
// latent until they adopt the first shape.
=======
// The first mitigation: give the binary a single `#[test]` that calls each measurement in
// sequence, with an `eprintln!` per phase so a failure still says which one broke.
// `brain_budget_tests.rs` and `emit_zero_alloc_tests.rs` are written that way.
//
// # One `#[test]` is NOT sufficient, and that is measured
//
// `observer_trace_zero_alloc_tests` was already written as a single `#[test]` — and on 2026-07-29 it
// still failed a full `cargo test --features desktop` run with "recording 5000 camera moves made 2
// heap allocations", while passing 3/3 in isolation. One `#[test]` removes *sibling test* threads;
// it does not remove libtest's own main thread, which is alive in the process for the whole run and
// allocates while it handles the harness's events.
//
// So [`TrackingAllocator::start_tracking_this_thread`] exists: it counts only the thread that
// started the measurement, which is the only thread most of these suites ever use.
//
// # Which of the two to call — the rule, and why it is not "always the new one"
//
// `start_tracking` (process-wide) is still the default, and two suites MUST keep it:
// `map_generation_zero_alloc_tests` and `terrain_challenger_tests`. `TerrainMap::generate` runs its
// noise pass on rayon workers (`core/terrain.rs`, `.into_par_iter()`), so a thread-scoped counter
// would stop counting the very allocations those gates exist to catch — and would go green while
// doing it. A gate that gets quieter is worse than a gate that is occasionally loud.
//
// Use `start_tracking_this_thread` only after checking that the code inside the measurement window
// does not hand work to another thread. Everything it would have counted from elsewhere is silently
// dropped, so this is a real trade, not a free upgrade.
>>>>>>> theirs
#![allow(dead_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

thread_local! {
    /// Set on the thread that called [`TrackingAllocator::start_tracking_this_thread`].
    ///
    /// Both details here are load-bearing, because this is read from inside `alloc`:
    /// `const`-initialised, so first touch does not allocate (which inside the allocator is
    /// unbounded recursion), and holding a `Cell<bool>`, which has no `Drop` — a TLS with a
    /// destructor registers one, and that allocates too.
    static MEASURING_THIS_THREAD: Cell<bool> = const { Cell::new(false) };
}

pub struct TrackingAllocator {
    alloc_count: AtomicUsize,
    active: AtomicBool,
    /// `false` => count only the thread that started the measurement.
    all_threads: AtomicBool,
}

impl TrackingAllocator {
    pub const fn new() -> Self {
        Self {
            alloc_count: AtomicUsize::new(0),
            active: AtomicBool::new(false),
            all_threads: AtomicBool::new(true),
        }
    }

    /// Count every allocation in the process. Required when the measured code uses rayon or spawns
    /// threads; see the module docs before choosing this over
    /// [`start_tracking_this_thread`](Self::start_tracking_this_thread).
    pub fn start_tracking(&self) {
        self.begin(true);
    }

    /// Count only allocations made by the calling thread.
    ///
    /// Immune to whatever else the process is doing — libtest's main thread, a sibling target's
    /// start-up, a background reporter. Blind to any work the measured code hands to another
    /// thread, which is why it is opt-in.
    ///
    /// Call [`stop_tracking`](Self::stop_tracking) from the **same thread**. The per-thread flag can
    /// only be cleared by its own thread, so stopping elsewhere leaves this one marked; the next
    /// thread-scoped measurement anywhere in the process would then also count this thread's
    /// allocations. Every suite here is a single test body, so this costs nothing to honour.
    pub fn start_tracking_this_thread(&self) {
        self.begin(false);
    }

    fn begin(&self, all_threads: bool) {
        // Ordering matters: the TLS write and the mode both land while `active` is false, so a
        // first-touch allocation in here could never be counted against the caller.
        MEASURING_THIS_THREAD.with(|m| m.set(!all_threads));
        self.all_threads.store(all_threads, Ordering::SeqCst);
        self.alloc_count.store(0, Ordering::SeqCst);
        self.active.store(true, Ordering::SeqCst);
    }

    pub fn stop_tracking(&self) -> usize {
        self.active.store(false, Ordering::SeqCst);
        MEASURING_THIS_THREAD.with(|m| m.set(false));
        // Back to the default, so a suite that mixes both styles cannot leak the narrower mode
        // into a later measurement that needed the wider one.
        self.all_threads.store(true, Ordering::SeqCst);
        self.alloc_count.load(Ordering::SeqCst)
    }

    #[inline]
    fn counts_calling_thread(&self) -> bool {
        if self.all_threads.load(Ordering::SeqCst) {
            return true;
        }
        // `try_with`, not `with`: `with` panics once a thread's TLS has been destroyed, and a panic
        // raised inside the global allocator aborts the process. A thread that far into teardown is
        // not the measuring thread, so `false` is the right answer as well as the safe one.
        MEASURING_THIS_THREAD.try_with(|m| m.get()).unwrap_or(false)
    }
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if self.active.load(Ordering::SeqCst) && self.counts_calling_thread() {
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
