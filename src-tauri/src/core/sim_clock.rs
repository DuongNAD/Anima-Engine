//! # Multi-rate simulation clock (M2.1) — one deterministic tick counter, many rates.
//!
//! The plan runs different processes at very different frequencies (§2.5 / §10.2): physics at the
//! 60 Hz base tick, local ecology at ~1 Hz, plant growth / decomposition at ~0.1–0.2 Hz, climate
//! slower still. Rather than a separate timer per system (a determinism hazard), everything is
//! derived from a single monotonically-advancing base-tick counter: a rate band with `period` P
//! fires exactly on the ticks where `tick % P == 0`. Given the same tick count, every band fires the
//! same number of times on the same ticks every run — the determinism S10 checks, and the
//! foundation replay (S13) stands on.

use crate::core::sim_rules::TICK_HZ;
use serde::{Deserialize, Serialize};

/// Base tick rate (Hz) — the fastest band. Mirrors [`crate::core::sim_rules::TICK_HZ`].
pub const BASE_TICK_HZ: f64 = TICK_HZ;

/// Canonical rate-band periods, in base ticks at 60 Hz (see plan §2.5). A `period` of P means the
/// band advances once every P base ticks.
pub const PHYSICS_PERIOD: u64 = 1; // 60 Hz — movement, collision
pub const ECOLOGY_PERIOD: u64 = 60; // 1 Hz — local resource/heat/hydration exchange
pub const PLANT_PERIOD: u64 = 300; // 0.2 Hz — plant growth, decomposition
pub const CLIMATE_PERIOD: u64 = 600; // 0.1 Hz — climate/season background

/// A named process that advances once every `period` base ticks.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateBand {
    pub name: &'static str,
    pub period: u64,
}

impl RateBand {
    pub const fn new(name: &'static str, period: u64) -> Self {
        Self { name, period }
    }

    /// The band's effective frequency in Hz, given the base tick rate.
    pub fn hz(&self) -> f64 {
        if self.period == 0 {
            0.0
        } else {
            BASE_TICK_HZ / self.period as f64
        }
    }
}

/// A monotonically-advancing base-tick counter. `tick` is 1-based after the first [`advance`], so a
/// band fires on its very first period boundary (e.g. a 60-period band fires on tick 60, not 0).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimClock {
    pub tick: u64,
}

impl SimClock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance one base tick and return the new (1-based) tick number.
    pub fn advance(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    /// Whether a band of `period` fires on `tick` (`tick % period == 0`; a `0` period never fires).
    pub fn fires(tick: u64, period: u64) -> bool {
        period != 0 && tick.is_multiple_of(period)
    }

    /// Whether a band of `period` fires on the current tick.
    pub fn band_fires(&self, period: u64) -> bool {
        Self::fires(self.tick, period)
    }

    /// How many times a band of `period` fires across the base ticks `1..=total` — the exact,
    /// closed-form count the S10 test checks a simulated run against.
    pub fn fire_count(total: u64, period: u64) -> u64 {
        total.checked_div(period).unwrap_or(0)
    }

    /// Sim-seconds elapsed at the current tick.
    pub fn seconds(&self) -> f64 {
        self.tick as f64 / BASE_TICK_HZ
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::sim_rules::TICKS_PER_YEAR;

    #[test]
    fn s10_bands_fire_the_expected_number_of_times() {
        // One simulated year (6000 base ticks). Drive the clock and count real firings per band,
        // then check them against the closed-form fire_count — they must agree exactly.
        let total = TICKS_PER_YEAR; // 6000
        let bands = [
            RateBand::new("physics", PHYSICS_PERIOD),
            RateBand::new("ecology", ECOLOGY_PERIOD),
            RateBand::new("plant", PLANT_PERIOD),
            RateBand::new("climate", CLIMATE_PERIOD),
        ];
        let mut counts = [0u64; 4];
        let mut clock = SimClock::new();
        for _ in 0..total {
            clock.advance();
            for (i, b) in bands.iter().enumerate() {
                if clock.band_fires(b.period) {
                    counts[i] += 1;
                }
            }
        }
        for (i, b) in bands.iter().enumerate() {
            assert_eq!(
                counts[i],
                SimClock::fire_count(total, b.period),
                "band {} fired {} times, expected {}",
                b.name,
                counts[i],
                SimClock::fire_count(total, b.period)
            );
        }
        // Concretely: physics every tick, ecology 100×, plant 20×, climate 10× per year.
        assert_eq!(counts, [6000, 100, 20, 10]);
    }

    #[test]
    fn s10_hz_and_seconds_are_consistent() {
        assert!((RateBand::new("e", ECOLOGY_PERIOD).hz() - 1.0).abs() < 1e-9);
        assert!((RateBand::new("p", PLANT_PERIOD).hz() - 0.2).abs() < 1e-9);
        let mut clock = SimClock::new();
        for _ in 0..60 {
            clock.advance();
        }
        assert!((clock.seconds() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn zero_period_never_fires() {
        assert!(!SimClock::fires(1000, 0));
        assert_eq!(SimClock::fire_count(1000, 0), 0);
    }
}
