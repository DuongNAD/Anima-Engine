//! Which threads are still alive, and which one did not come back (§3.7).
//!
//! ## The gap this closes
//!
//! `SimulationEngine::start` spawns five threads and `stop` joins them. Every handle is accounted
//! for — an audit of all seven `thread::spawn` sites in the crate found no dropped handle. What was
//! missing is any answer to the two questions that matter when shutdown goes wrong:
//!
//! - **Which thread did not finish?** `stop` held a `Vec<JoinHandle<()>>` with no names, so a hang
//!   was indistinguishable across five candidates.
//! - **Did it hang, or did it panic?** `let _ = handle.join()` discards the `Err` a panicking thread
//!   returns, so a thread that died badly and one that never noticed `running` looked identical.
//!
//! And the failure mode itself was unbounded: `JoinHandle::join` has no timeout, so one thread
//! ignoring the shutdown flag makes `stop` block forever. That is the shape of the CI failure this
//! subsystem was blamed for — `cargo test` waiting on a child that never returns, with no output to
//! say why.
//!
//! ## How it works
//!
//! Every spawned thread carries an [`ExitToken`] named after it. The token is moved *into* the
//! closure, so it is dropped when the thread's stack unwinds — which happens on a normal return
//! **and on a panic**. A thread that died badly therefore reports as exited, not as hung, which is
//! the distinction the old code could not make.
//!
//! [`ThreadSupervisor::wait_for_exit`] then gives shutdown a deadline and, on expiry, the names of
//! the threads still outstanding.
//!
//! ## The deliberate trade
//!
//! When a thread misses the deadline, the caller is expected to **report it and move on rather than
//! join it**. Joining a hung thread is the unbounded wait this exists to remove; skipping it detaches
//! that thread, which leaks it for the life of the process. Leaking one named thread and saying so is
//! strictly better than an unkillable `stop()` that says nothing — but it is a trade, not a fix, and
//! the fix is whatever made the thread ignore `running`.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Names of the threads the engine spawns. `&'static str` rather than `String` so registering a
/// thread cannot fail or allocate, and so a typo is a compile error at the call site.
pub const SIM: &str = "sim";
pub const EMIT: &str = "emit";
pub const EVO: &str = "evo";
pub const NET: &str = "net";
pub const LEARN: &str = "learn";

/// Tracks which named threads have not yet reported exit.
///
/// `BTreeSet` rather than `HashSet`: the diagnostic names threads in a stable order, so two runs
/// reporting the same stuck set produce the same message. The determinism contract asks for this
/// habit generally and it costs nothing here.
#[derive(Clone, Default)]
pub struct ThreadSupervisor {
    live: Arc<Mutex<BTreeSet<&'static str>>>,
}

impl ThreadSupervisor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `name` as running and hand back the token whose drop marks it finished.
    ///
    /// Move the token into the spawned closure. Holding it anywhere else — the spawning thread, a
    /// struct that outlives the thread — makes the thread look permanently alive and would turn every
    /// shutdown into a false stall report.
    pub fn token(&self, name: &'static str) -> ExitToken {
        if let Ok(mut live) = self.live.lock() {
            live.insert(name);
        }
        ExitToken {
            name,
            live: Arc::clone(&self.live),
        }
    }

    /// Threads that have not reported exit, in a stable order.
    pub fn live(&self) -> Vec<&'static str> {
        match self.live.lock() {
            Ok(live) => live.iter().copied().collect(),
            // A poisoned lock means a thread panicked while holding it. Recovering the guard is the
            // convention in this crate: losing the registry would make shutdown blind, which is the
            // exact problem this type exists to remove.
            Err(e) => e.into_inner().iter().copied().collect(),
        }
    }

    /// Wait up to `deadline` for every registered thread to report exit.
    ///
    /// Returns the names still outstanding — empty when shutdown was clean. Polls rather than using a
    /// condvar because the wait is expected to end in microseconds on the happy path and this keeps
    /// the type free of a second synchronisation primitive to reason about.
    pub fn wait_for_exit(&self, deadline: Duration) -> Vec<&'static str> {
        let started = Instant::now();
        loop {
            let live = self.live();
            if live.is_empty() {
                return live;
            }
            if started.elapsed() >= deadline {
                return live;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

/// Marks its thread as finished when dropped. Move it into the thread it names.
pub struct ExitToken {
    name: &'static str,
    live: Arc<Mutex<BTreeSet<&'static str>>>,
}

impl ExitToken {
    pub fn name(&self) -> &'static str {
        self.name
    }
}

impl Drop for ExitToken {
    fn drop(&mut self) {
        // Runs during unwinding too, which is the point: a panicking thread must report as exited
        // rather than as hung, or shutdown would blame the wrong failure.
        match self.live.lock() {
            Ok(mut live) => {
                live.remove(self.name);
            }
            Err(e) => {
                e.into_inner().remove(self.name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_registered_thread_is_live_until_its_token_drops() {
        let sup = ThreadSupervisor::new();
        let token = sup.token(SIM);
        assert_eq!(sup.live(), vec![SIM]);
        drop(token);
        assert!(sup.live().is_empty());
    }

    #[test]
    fn names_come_back_in_a_stable_order() {
        let sup = ThreadSupervisor::new();
        // Registered out of alphabetical order on purpose.
        let _n = sup.token(NET);
        let _s = sup.token(SIM);
        let _e = sup.token(EMIT);
        assert_eq!(sup.live(), vec![EMIT, NET, SIM]);
    }

    #[test]
    fn waiting_returns_immediately_when_everything_has_exited() {
        let sup = ThreadSupervisor::new();
        drop(sup.token(SIM));
        let started = Instant::now();
        assert!(sup.wait_for_exit(Duration::from_secs(30)).is_empty());
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "the happy path must not pay the deadline"
        );
    }

    #[test]
    fn waiting_names_what_did_not_come_back() {
        let sup = ThreadSupervisor::new();
        let _stuck = sup.token(NET);
        drop(sup.token(SIM));
        let outstanding = sup.wait_for_exit(Duration::from_millis(30));
        assert_eq!(
            outstanding,
            vec![NET],
            "a stalled shutdown has to name the thread, not just fail"
        );
    }

    /// The distinction the old `let _ = handle.join()` could not draw: a thread that panicked has
    /// exited, and must not be reported as hung.
    #[test]
    fn a_panicking_thread_reports_as_exited_not_as_hung() {
        let sup = ThreadSupervisor::new();
        let token = sup.token(EVO);
        let handle = std::thread::spawn(move || {
            let _exit = token;
            panic!("deliberate");
        });
        assert!(handle.join().is_err(), "the thread should have panicked");
        assert!(
            sup.live().is_empty(),
            "an unwinding thread must still report exit, or shutdown blames a hang"
        );
    }

    #[test]
    fn a_real_thread_reports_exit_on_its_own() {
        let sup = ThreadSupervisor::new();
        let token = sup.token(LEARN);
        let handle = std::thread::spawn(move || {
            let _exit = token;
        });
        handle.join().expect("thread should finish");
        assert!(sup.wait_for_exit(Duration::from_secs(5)).is_empty());
    }
}
