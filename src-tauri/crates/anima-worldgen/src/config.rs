//! Validated configuration for World v2 regional coastal massif generation.

use crate::{ELEV_MAX_M, ELEV_MIN_M, WORLD_EXTENT_M};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Error returned when a [`WorldV2Config`] violates domain invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ConfigError {}

/// Top-level configuration for World v2 regional terrain generation (`LD-1` / `LD-5`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldV2Config {
    /// Master 64-bit deterministic seed.
    pub seed: u64,
    /// Schema/preset tag hashed into PRNG domain separation.
    pub schema_tag: String,
    /// Grid resolution per side (`width == height`). Minimum supported is `64`.
    pub grid_size: u32,
    /// Physical square domain extent in metres (`40,960.0` default).
    pub extent_m: f32,
    /// Lower bathymetric bound in metres (`-1,500.0`).
    pub elev_min_m: f32,
    /// Upper alpine massif summit bound in metres (`+5,700.0`).
    pub elev_max_m: f32,
    /// Target oceanic fraction strictly inside `[0.40, 0.50]`.
    pub target_ocean_fraction: f32,
    /// Regional massif ridge uplift amplitude in `m/Myr` proxy units.
    pub massif_uplift_amplitude: f32,
    /// Stratocone peak relief in metres above surrounding foreland baseline.
    pub volcano_relief_m: f32,
    /// Caprock plateau lower altitude bound in metres (`450.0` default).
    pub caprock_min_m: f32,
    /// Caprock plateau upper altitude bound in metres (`1,100.0` default).
    pub caprock_max_m: f32,
    /// Maximum allowed ratio `max(stratum_k) / min(stratum_k)` across the 6 lithology classes (`<= 8.0`).
    pub max_erodibility_contrast: f32,
}

impl Default for WorldV2Config {
    fn default() -> Self {
        Self {
            seed: 0x0057_5632_2026_1120,
            schema_tag: "ANMW3-SNSM-v1".to_owned(),
            grid_size: 4096,
            extent_m: WORLD_EXTENT_M,
            elev_min_m: ELEV_MIN_M,
            elev_max_m: ELEV_MAX_M,
            target_ocean_fraction: 0.45,
            massif_uplift_amplitude: 1_200.0,
            volcano_relief_m: 3_100.0,
            caprock_min_m: 450.0,
            caprock_max_m: 1_100.0,
            max_erodibility_contrast: 8.0,
        }
    }
}

impl WorldV2Config {
    /// Creates a validated default configuration overridden with `seed` and square `grid_size`.
    #[must_use]
    pub fn for_size(seed: u64, grid_size: u32) -> Self {
        Self {
            seed,
            grid_size,
            ..Self::default()
        }
    }

    /// Cell spacing in metres (`extent_m / grid_size`).
    #[must_use]
    pub fn cell_size_m(&self) -> f32 {
        self.extent_m / self.grid_size as f32
    }

    /// Checks physical and numerical preconditions before launching Stage S1+.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let err = |msg: &str| ConfigError {
            message: msg.to_owned(),
        };
        if self.grid_size < 64 || !self.grid_size.is_multiple_of(2) {
            return Err(err("grid_size must be an even integer >= 64"));
        }
        if !self.extent_m.is_finite() || self.extent_m <= 0.0 {
            return Err(err("extent_m must be finite and > 0"));
        }
        if !self.elev_min_m.is_finite()
            || !self.elev_max_m.is_finite()
            || self.elev_min_m >= 0.0
            || self.elev_max_m <= 0.0
        {
            return Err(err("elev_min_m < 0 < elev_max_m required"));
        }
        if !self.target_ocean_fraction.is_finite()
            || !(0.40..=0.50).contains(&self.target_ocean_fraction)
        {
            return Err(err("target_ocean_fraction must lie within [0.40, 0.50]"));
        }
        if !self.caprock_min_m.is_finite()
            || !self.caprock_max_m.is_finite()
            || self.caprock_min_m <= 0.0
            || self.caprock_max_m <= self.caprock_min_m
        {
            return Err(err(
                "caprock_min_m > 0 and caprock_max_m > caprock_min_m required",
            ));
        }
        if !self.max_erodibility_contrast.is_finite()
            || self.max_erodibility_contrast <= 1.0
            || self.max_erodibility_contrast > 8.0
        {
            return Err(err("max_erodibility_contrast must lie in (1.0, 8.0]"));
        }
        Ok(())
    }
}
