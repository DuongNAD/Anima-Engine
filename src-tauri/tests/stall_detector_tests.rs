//! The firing rule behind the `persistence_stress_tests` watchdog.
//!
//! Its own target, and no `#[global_allocator]` here, so these can be ordinary parallel tests
//! without re-arming the measurement flake described in `common/allocator.rs`.

mod common;

use common::stall_detector::StallDetector;
use std::time::Duration;

#[test]
fn it_fires_only_after_the_whole_budget_without_progress() {
    let tick = Duration::from_secs(1);
    let mut d = StallDetector::new(Duration::from_secs(3));

    assert!(
        !d.observe(1, tick),
        "first sighting of a counter is progress"
    );
    assert!(!d.observe(1, tick), "1s still, budget is 3s");
    assert!(!d.observe(1, tick), "2s still");
    assert!(d.observe(1, tick), "3s still — must fire");
    assert_eq!(d.stalled_for(), Duration::from_secs(3));
}

#[test]
fn progress_resets_it() {
    let tick = Duration::from_secs(1);
    let mut d = StallDetector::new(Duration::from_secs(3));

    for counter in 1..=100u64 {
        assert!(
            !d.observe(counter, tick),
            "a loop making progress must never trip the watchdog; tripped at {counter}"
        );
    }
    // And a stall right after a long healthy stretch still fires on schedule.
    assert!(!d.observe(100, tick));
    assert!(!d.observe(100, tick));
    assert!(d.observe(100, tick));
}

/// The failure worth guarding: a budget that never elapses because the poll is coarser than it.
#[test]
fn a_tick_coarser_than_the_budget_still_fires_on_the_first_stall() {
    let mut d = StallDetector::new(Duration::from_secs(1));
    assert!(!d.observe(7, Duration::from_secs(5)), "first sighting");
    assert!(
        d.observe(7, Duration::from_secs(5)),
        "one coarse tick already exceeds the budget"
    );
}

/// A zero counter is the "not started yet" sentinel the watchdog begins with, so it must not read as
/// progress on the very first observation and reset a real stall.
#[test]
fn the_initial_zero_counter_is_not_treated_as_progress() {
    let tick = Duration::from_secs(1);
    let mut d = StallDetector::new(Duration::from_secs(2));
    // Counter never leaves 0: the loop never started. That is a stall, and it must be reported.
    assert!(!d.observe(0, tick));
    assert!(
        d.observe(0, tick),
        "a loop that never started is still a stall"
    );
}
