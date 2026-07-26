//! §3.7 — `stop` reclaims every thread `start` spawned, and says so when it does not.
//!
//! This invariant was assumed, never checked. An audit of all seven `thread::spawn` sites in the
//! crate found every `JoinHandle` accounted for, so nothing was being dropped — but `stop` held an
//! unnamed `Vec<JoinHandle<()>>` and called `let _ = handle.join()`, which meant it could answer
//! neither question that matters when shutdown goes wrong: *which* thread failed to return, and
//! whether it hung or panicked. And `join` has no timeout, so one thread ignoring `running` made
//! `stop` block forever — an unkillable wait with no output, which is how it looked from CI.
//!
//! No `#[global_allocator]` here on purpose, so these can be ordinary parallel tests. See
//! `tests/common/allocator.rs` for why a binary that installs one should carry a single `#[test]`.

use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};
use anima_engine_lib::core::simulation_lifecycle::SimulationEngine;
use anima_engine_lib::core::thread_supervisor as sup;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// The threads `start` spawns, in the stable order the supervisor reports them.
const EXPECTED: [&str; 5] = [sup::EMIT, sup::EVO, sup::LEARN, sup::NET, sup::SIM];

struct Rig {
    engine: SimulationEngine,
    settings: Arc<Mutex<EvolutionSettings>>,
    evo_running: Arc<AtomicBool>,
    grid: Arc<Mutex<MapElitesGridState>>,
}

fn rig() -> Rig {
    Rig {
        engine: SimulationEngine::new(),
        settings: Arc::new(Mutex::new(EvolutionSettings {
            mutation_rate: 0.15,
            selection_bias: 1.5,
            grid_resolution: 50,
        })),
        evo_running: Arc::new(AtomicBool::new(false)),
        grid: Arc::new(Mutex::new(MapElitesGridState {
            grid: std::collections::HashMap::new(),
            grid_resolution: 50,
        })),
    }
}

impl Rig {
    fn start(&self) {
        self.engine.start::<tauri::test::MockRuntime>(
            None,
            Arc::clone(&self.settings),
            Arc::clone(&self.evo_running),
            Arc::clone(&self.grid),
        );
    }
}

/// A fresh engine has spawned nothing, so nothing is owed.
#[test]
fn an_engine_that_never_started_owes_no_threads() {
    let r = rig();
    assert!(r.engine.supervisor.live().is_empty());
}

/// Every thread registers before `start` returns, so the set is knowable synchronously rather than
/// after a sleep. A test that had to sleep to see them would be measuring the scheduler.
#[test]
fn start_registers_every_thread_it_spawns() {
    let r = rig();
    r.start();
    assert_eq!(
        r.engine.supervisor.live(),
        EXPECTED.to_vec(),
        "start spawned a set of threads the supervisor does not know about, so stop could not \
         report on them"
    );
    r.engine.stop();
}

/// **The §3.7 gate.** After `stop`, nothing `start` spawned is still running.
#[test]
fn stop_reclaims_every_thread_start_spawned() {
    let r = rig();
    r.start();
    assert_eq!(r.engine.supervisor.live().len(), EXPECTED.len());

    r.engine.stop();

    assert!(
        r.engine.supervisor.live().is_empty(),
        "stop returned with these threads still alive: {:?}. stop waits for every thread to report \
         exit before joining, so a non-empty set here means one ignored `running` and was left \
         detached — the leak §3.7 exists to close",
        r.engine.supervisor.live()
    );
    assert!(!r.engine.get_status().running);
}

/// Repeatably, because the failure this guards is cumulative: the CI hang appeared in a test that
/// cycles `start`/`stop` a hundred times, where leaking one thread per cycle is what compounds.
#[test]
fn repeated_start_stop_cycles_leave_nothing_behind() {
    let r = rig();
    for cycle in 1..=8 {
        r.start();
        assert_eq!(
            r.engine.supervisor.live().len(),
            EXPECTED.len(),
            "cycle {cycle}: start did not register the full set"
        );
        r.engine.stop();
        assert!(
            r.engine.supervisor.live().is_empty(),
            "cycle {cycle}: stop left {:?} behind",
            r.engine.supervisor.live()
        );
    }
}

/// `start` is guarded by a `compare_exchange(false, true)`, so a second call on a running engine is a
/// no-op. It must not register a second set of tokens — that would make the first `stop` report
/// phantom stragglers forever.
#[test]
fn starting_twice_does_not_double_register() {
    let r = rig();
    r.start();
    r.start();
    assert_eq!(
        r.engine.supervisor.live(),
        EXPECTED.to_vec(),
        "a redundant start registered threads it did not spawn"
    );
    r.engine.stop();
    assert!(r.engine.supervisor.live().is_empty());
}

/// `stop` on an engine that was never started returns without waiting out the grace period. The
/// timing assertion is the point: a `stop` that paid 30s here would make every teardown that cheap
/// mistake expensive.
#[test]
fn stopping_an_unstarted_engine_returns_at_once() {
    let r = rig();
    let began = std::time::Instant::now();
    r.engine.stop();
    assert!(
        began.elapsed() < std::time::Duration::from_secs(2),
        "stop on an unstarted engine took {:?}",
        began.elapsed()
    );
}
