//! Backpressure contract for the bounded inference recycle pools.
//!
//! When action resolution has not yet returned a response batch, an inference worker must wait
//! rather than allocate a seventeenth batch. Otherwise a slow consumer turns the "bounded" pool
//! into permanent heap growth.

use anima_engine_lib::core::agent_systems::{
    wait_for_recycled_response_batch_until, InferenceResponseBatch, ResponsePoolWaitError,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn an_empty_response_pool_applies_backpressure_until_a_batch_is_recycled() {
    let running = Arc::new(AtomicBool::new(true));
    let (recycle_tx, recycle_rx) = crossbeam_channel::unbounded();
    let (done_tx, done_rx) = crossbeam_channel::bounded(1);
    let worker_running = Arc::clone(&running);

    let worker = std::thread::spawn(move || {
        done_tx
            .send(wait_for_recycled_response_batch_until(
                &worker_running,
                &recycle_rx,
                Duration::from_secs(1),
            ))
            .unwrap();
    });

    assert!(
        done_rx.recv_timeout(Duration::from_millis(25)).is_err(),
        "an empty pool must block the producer instead of manufacturing a new response batch"
    );

    recycle_tx
        .send(InferenceResponseBatch {
            responses: Vec::with_capacity(7),
        })
        .unwrap();
    let returned = done_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("the recycled batch should release backpressure")
        .expect("the simulation is still running");
    assert_eq!(returned.responses.capacity(), 7);

    running.store(false, Ordering::SeqCst);
    worker.join().unwrap();
}

#[test]
fn shutdown_releases_a_worker_waiting_on_the_response_pool() {
    let running = Arc::new(AtomicBool::new(true));
    let (_recycle_tx, recycle_rx) = crossbeam_channel::unbounded();
    let worker_running = Arc::clone(&running);
    let worker = std::thread::spawn(move || {
        wait_for_recycled_response_batch_until(&worker_running, &recycle_rx, Duration::from_secs(1))
    });

    std::thread::sleep(Duration::from_millis(25));
    running.store(false, Ordering::SeqCst);

    assert!(
        matches!(worker.join().unwrap(), Err(ResponsePoolWaitError::Shutdown)),
        "shutdown must not leave the inference worker blocked on an empty pool"
    );
}

#[test]
fn a_disconnected_response_pool_is_not_misreported_as_shutdown() {
    let running = AtomicBool::new(true);
    let (recycle_tx, recycle_rx) = crossbeam_channel::unbounded();
    drop(recycle_tx);

    assert!(matches!(
        wait_for_recycled_response_batch_until(&running, &recycle_rx, Duration::from_secs(1),),
        Err(ResponsePoolWaitError::Disconnected)
    ));
}

#[test]
fn a_missing_response_buffer_has_a_deterministic_stall_deadline() {
    let running = AtomicBool::new(true);
    let (_recycle_tx, recycle_rx) = crossbeam_channel::unbounded::<InferenceResponseBatch>();

    assert!(matches!(
        wait_for_recycled_response_batch_until(&running, &recycle_rx, Duration::ZERO),
        Err(ResponsePoolWaitError::Stalled)
    ));
}

#[test]
fn shutdown_wins_even_when_a_recycled_batch_is_already_available() {
    let running = AtomicBool::new(false);
    let (recycle_tx, recycle_rx) = crossbeam_channel::unbounded();
    recycle_tx
        .send(InferenceResponseBatch {
            responses: Vec::new(),
        })
        .unwrap();

    assert!(matches!(
        wait_for_recycled_response_batch_until(&running, &recycle_rx, Duration::from_secs(1),),
        Err(ResponsePoolWaitError::Shutdown)
    ));
}
