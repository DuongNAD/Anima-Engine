mod common;

use std::hint::black_box;
use std::sync::{Arc, Barrier};

#[global_allocator]
static ALLOCATOR: common::allocator::ThreadTrackingAllocator =
    common::allocator::ThreadTrackingAllocator::new();

#[test]
fn current_thread_tracking_ignores_allocations_from_other_threads() {
    let start = Arc::new(Barrier::new(2));
    let finished = Arc::new(Barrier::new(2));
    let worker_start = Arc::clone(&start);
    let worker_finished = Arc::clone(&finished);
    let worker = std::thread::spawn(move || {
        // Vòng 1: khô (warm-up) — vòng 2: vòng được đo.
        for _ in 0..2 {
            worker_start.wait();
            let allocation = Box::new([7_u8; 1024]);
            black_box(&allocation);
            worker_finished.wait();
        }
    });

    // Vòng khô (warm-up): Barrier/Condvar/parking của std có thể cấp phát lười
    // trong LẦN DÙNG ĐẦU TIÊN trên luồng này. Vòng chạy dưới đây diễn ra TRƯỚC
    // cửa sổ đo để những cấp phát đó không bị đếm nhầm vào cổng zero-alloc.
    start.wait();
    finished.wait();

    ALLOCATOR.start_tracking();
    start.wait();
    let allocation = Box::new([9_u8; 1024]);
    black_box(&allocation);
    finished.wait();
    let allocations = ALLOCATOR.stop_tracking();

    worker.join().expect("worker exits cleanly");
    assert_eq!(
        allocations, 1,
        "only the allocation made by the measured thread belongs to this gate"
    );
}
