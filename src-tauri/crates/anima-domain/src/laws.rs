//! Time laws: the constants that define how fast simulated time runs.
//!
//! These moved out of `core::sim_rules` because they are laws, not engine settings — the headless
//! runner and the live Bevy world must agree on them or a manifest replayed in one does not
//! describe the other. `core::sim_rules` re-exports them, so the units table and the S01 machine
//! checks still read as one document.

/// Physics/collision tick rate in Hz — the fixed simulation step. Mirrors `TimeStep(1.0/60.0)`
/// inserted in `core/ecs.rs`.
pub const TICK_HZ: f64 = 60.0;

/// Fixed simulation time step, in sim-seconds (`1 / TICK_HZ` ≈ 0.016667).
pub const TICK_DT_SECONDS: f64 = 1.0 / TICK_HZ;

/// Ticks in one evolutionary epoch (`EpochManager::ticks_per_epoch`, `core/simulation_loop.rs`).
pub const TICKS_PER_EPOCH: u64 = 1000;

/// Sim-seconds in one simulated year: the `SeasonClock` sweeps one full 2π phase over this span
/// (`rate = TAU / SECONDS_PER_YEAR` in `core/ecology.rs`).
pub const SECONDS_PER_YEAR: f64 = 100.0;

/// Ticks in one simulated year (`SECONDS_PER_YEAR * TICK_HZ` = 6000 at 60 Hz).
pub const TICKS_PER_YEAR: u64 = 6000;
