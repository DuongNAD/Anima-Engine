//! What the engine reports about its own speed, on the path that reports it wrongly.
//!
//! `SimulationStatus::avg_tick_time_ms` is the only per-tick cost the running app ever shows, and
//! both readouts of it (`App.tsx`, `StatusPanel.tsx`) colour anything under 2 ms as healthy. It used
//! to be `total_tick_duration / tick_count`, where the numerator starts at zero and the numerator's
//! counterpart does not: a restored world seeds `tick_count` from its save. The unit half of the fix
//! is `simulation_loop::mean_tick_time_tests`; this is the half that proves the loop divides by the
//! counter it timed rather than by the world's.

use anima_engine_lib::commands::{EvolutionSettings, MapElitesGridState};
use anima_engine_lib::core::simulation_lifecycle::SimulationEngine;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use std::time::{Duration, Instant};

/// A liveness deadline, not a performance budget — `start` spawns four threads before the first
/// tick. Matches the gates in `persistence_tests.rs`, and duplicated for the same reason they are.
const READY_DEADLINE: Duration = Duration::from_secs(10);
const READY_POLL: Duration = Duration::from_millis(5);

/// The tick a restored world is placed at. Large on purpose: the defect scales with it, and a
/// realistic long-running save is far past this.
const RESUME_AT_TICK: u64 = 1_000_000;

/// Ticks each engine is allowed to time before its average is read. Small enough to stay fast,
/// large enough that the average is not one outlier tick.
const TICKS_TO_MEASURE: u64 = 30;

fn handles() -> (
    Arc<Mutex<EvolutionSettings>>,
    Arc<AtomicBool>,
    Arc<Mutex<MapElitesGridState>>,
) {
    (
        Arc::new(Mutex::new(EvolutionSettings {
            mutation_rate: 0.1,
            selection_bias: 1.0,
            grid_resolution: 40,
        })),
        // Evolution off: this measures the tick loop, not the epoch machinery.
        Arc::new(AtomicBool::new(false)),
        Arc::new(Mutex::new(MapElitesGridState {
            grid: std::collections::HashMap::new(),
            grid_resolution: 40,
        })),
    )
}

/// Wait until the engine is running and has ticked past `tick_floor`, or panic naming the last
/// status seen.
fn await_running_past_tick(engine: &SimulationEngine, tick_floor: u64) {
    let start = Instant::now();
    loop {
        let status = engine.get_status();
        if status.running && status.tick_count > tick_floor {
            return;
        }
        let waited = start.elapsed();
        assert!(
            waited < READY_DEADLINE,
            "timed out after {waited:.2?} waiting for the engine to tick past {tick_floor}; \
             last status: running={}, tick_count={}",
            status.running,
            status.tick_count
        );
        std::thread::sleep(READY_POLL);
    }
}

/// A run that resumed a save must report the cost of the ticks *it* ran.
///
/// Both engines do the same work — ten founders, evolution off, the same schedule — and differ only
/// in the tick the world believes it is at. The old expression divided by that tick, so the resumed
/// engine reported roughly `TICKS_TO_MEASURE / RESUME_AT_TICK` of the truth: about thirty
/// thousandths of a percent, which renders as `0.00 ms` and reads as a very fast simulator.
///
/// The comparison is a ratio rather than an absolute budget so the gate measures the code and not
/// the machine's mood; the factor of 100 is enormous slack against an error of ~33,000×.
#[test]
fn a_resumed_run_reports_its_own_tick_cost_not_one_divided_by_the_saves_tick() {
    let (evo_settings, evo_running, map_elites_grid) = handles();

    // A fresh world, for the reference number.
    let engine = SimulationEngine::new();
    engine.start::<tauri::test::MockRuntime>(
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_elites_grid),
    );
    await_running_past_tick(&engine, TICKS_TO_MEASURE);
    let fresh_avg = engine.get_status().avg_tick_time_ms;

    // Take a real snapshot rather than hand-building one, so the resumed engine restores through
    // the same path the app does.
    let (tx, rx) = std::sync::mpsc::channel();
    engine
        .save_request_tx
        .send(tx)
        .expect("the sim thread must accept a save request");
    let mut saved = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the sim thread must answer a save request")
        .expect("a world with no dormant cohorts must save");
    engine.stop();

    assert!(
        fresh_avg > 0.0,
        "a fresh run must report a positive per-tick cost, got {fresh_avg}"
    );

    // The same world, told it is a million ticks old.
    saved.tick_count = RESUME_AT_TICK;
    *engine
        .pending_load_state
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(saved);

    engine.start::<tauri::test::MockRuntime>(
        None,
        Arc::clone(&evo_settings),
        Arc::clone(&evo_running),
        Arc::clone(&map_elites_grid),
    );
    await_running_past_tick(&engine, RESUME_AT_TICK + TICKS_TO_MEASURE);
    let resumed_status = engine.get_status();
    engine.stop();

    assert!(
        resumed_status.tick_count > RESUME_AT_TICK,
        "the pending load was not applied; tick_count = {}",
        resumed_status.tick_count
    );
    assert!(
        resumed_status.avg_tick_time_ms.is_finite(),
        "the reported average must be a number, got {}",
        resumed_status.avg_tick_time_ms
    );
    assert!(
        resumed_status.avg_tick_time_ms > fresh_avg / 100.0,
        "a run resumed at tick {RESUME_AT_TICK} reported {} ms per tick against a fresh run's \
         {fresh_avg} ms — the average is being divided by the world's tick count instead of by the \
         ticks this run timed",
        resumed_status.avg_tick_time_ms
    );
}
