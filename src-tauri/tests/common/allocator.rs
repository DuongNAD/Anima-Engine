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
