//! Decide when a loop has stopped making progress.
//!
//! Pulled out of `persistence_stress_tests.rs` so it can be tested at all. The watchdog that uses it
//! ends in `std::process::exit`, which cannot be exercised in-process, and lowering its budget does
//! **not** exercise it either: the counter resets on every observed change, so a healthy fast loop
//! never trips regardless of how small the budget is. That is correct behaviour, and it is also why
//! the firing rule needs tests of its own rather than a manual smoke run.
//!
//! Lives in `common/` rather than beside its caller for a second reason: `persistence_stress_tests`
//! installs a `#[global_allocator]`, and per `common/allocator.rs` such a binary should carry exactly
//! **one** `#[test]` — libtest spawns a thread per test and spawning allocates, so siblings land
//! inside another test's measurement window. Adding three unit tests there would have re-armed the
//! flake that `brain_budget_tests` lost to roughly one run in three.

#![allow(dead_code)]

use std::time::Duration;

/// Watches a monotonically-advancing counter and reports when it has been still for too long.
pub struct StallDetector {
    last_seen: u64,
    stalled_for: Duration,
    budget: Duration,
}

impl StallDetector {
    pub fn new(budget: Duration) -> Self {
        Self {
            last_seen: 0,
            stalled_for: Duration::ZERO,
            budget,
        }
    }

    /// Feed one observation. Returns `true` once the counter has been unchanged for the whole budget.
    pub fn observe(&mut self, counter: u64, tick: Duration) -> bool {
        if counter != self.last_seen {
            self.last_seen = counter;
            self.stalled_for = Duration::ZERO;
            return false;
        }
        self.stalled_for += tick;
        self.stalled_for >= self.budget
    }

    /// How long the counter has been still. For the diagnostic the caller prints when it fires.
    pub fn stalled_for(&self) -> Duration {
        self.stalled_for
    }
}
