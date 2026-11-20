#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

//! Deterministic World v2 generator, `ANMW v3` codec, and offline bake pipeline.
//!
//! Card `WV2-1a` establishes the zero-default-dependency crate boundary (`reader` / `generate` /
//! `bake`), the `clippy.toml` ban on std transcendentals in favor of `libm`, domain-separated
//! `ChaCha12Rng` seeding, configuration validation, and the Stage S1 regional tectonic uplift &
//! stratigraphic lithology generator.

#[cfg(feature = "generate")]
pub mod config;
#[cfg(feature = "generate")]
pub mod erosion_fluvial;
#[cfg(feature = "generate")]
pub mod erosion_thermal;
#[cfg(feature = "generate")]
pub mod hydrology;
#[cfg(feature = "generate")]
pub mod rng;
#[cfg(feature = "generate")]
pub mod tectonic;

/// Fixed physical domain width/depth in metres (`40.96 km × 40.96 km` tropical coastal massif).
pub const WORLD_EXTENT_M: f32 = 40_960.0;

/// Minimum physical bathymetric elevation in metres (`-1,500 m`).
pub const ELEV_MIN_M: f32 = -1_500.0;

/// Maximum physical alpine summit elevation in metres (`+5,700 m`).
pub const ELEV_MAX_M: f32 = 5_700.0;

/// Physical sea level in metres (`0.0 m`).
pub const SEA_LEVEL_M: f32 = 0.0;

/// Normalized sea level (`(0 - (-1500)) / 7200 = 0.20833333...`).
pub const SEA_LEVEL_NORM: f32 = (SEA_LEVEL_M - ELEV_MIN_M) / (ELEV_MAX_M - ELEV_MIN_M);

/// Number of canonical lithology classes produced by Stage S1 (`0..=5`).
pub const LITHOLOGY_CLASS_COUNT: usize = 6;

/// Maps physical elevation in metres (`[-1500, +5700]`) to `[0.0, 1.0]`.
#[must_use]
pub fn normalize_elevation(elevation_m: f32) -> f32 {
    let span = ELEV_MAX_M - ELEV_MIN_M;
    ((elevation_m - ELEV_MIN_M) / span).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elevation_normalization_anchors_match_v3_vertical_scale() {
        assert!((WORLD_EXTENT_M - 40_960.0).abs() < f32::EPSILON);
        assert_eq!(LITHOLOGY_CLASS_COUNT, 6);
        assert!((normalize_elevation(ELEV_MIN_M) - 0.0).abs() < 1e-6);
        assert!((normalize_elevation(ELEV_MAX_M) - 1.0).abs() < 1e-6);
        assert!((normalize_elevation(SEA_LEVEL_M) - SEA_LEVEL_NORM).abs() < 1e-6);
        assert!((SEA_LEVEL_NORM - (1500.0_f32 / 7200.0_f32)).abs() < 1e-7);
    }
}
