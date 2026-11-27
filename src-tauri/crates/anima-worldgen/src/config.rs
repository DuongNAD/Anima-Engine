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

/// Regional macro-layout mode for World v2 tectonic generation (`WV2-BIG-1`).
///
/// [`WorldLayout::SingleMassif`] preserves trunk `40.96 km` generation bit-for-bit;
/// [`WorldLayout::MultiEnvironment`] activates the opt-in metre-based `81.92 km`
/// multi-environment layout (`E1..E9`, `§4b` coastal plain setbacks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldLayout {
    #[default]
    SingleMassif,
    MultiEnvironment,
}

/// Top-level configuration for World v2 regional terrain generation (`LD-1` / `LD-5`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldV2Config {
    /// Master 64-bit deterministic seed.
    pub seed: u64,
    /// Schema/preset tag hashed into PRNG domain separation.
    pub schema_tag: String,
    /// Regional tectonic layout mode (default: `single_massif`, bit-identical to trunk).
    #[serde(default)]
    pub layout: WorldLayout,
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
    /// Channel initiation threshold on `A * S^2` (`m^2`) transitioning MFD hillslopes -> D8 channels.
    #[serde(default = "default_channel_init")]
    pub channel_init: f32,
    /// MFD slope-partition exponent `p` controlling hillslope convergence.
    #[serde(default = "default_mfd_p")]
    pub mfd_p: f32,
    /// Seasonal convective storm-runoff yield coefficient (`0.0..=0.50`) for sub-humid/semi-arid basins.
    #[serde(default = "default_storm_runoff")]
    pub storm_runoff: f32,
    /// Dimensionless multiplier on Fastscape-SD fluvial erodibility scale across multigrid evolution.
    #[serde(default = "default_fluvial_k_mult")]
    pub fluvial_k_mult: f32,
    /// Caprock-over-shale differential plunge sapping gain (`0.0..=6.0`).
    #[serde(default = "default_sapping_gain")]
    pub sapping_gain: f32,
    /// Amplitude multiplier for structural pull-apart/foreland lake basins and caldera relief (`0.5..=3.0`).
    #[serde(default = "default_basin_amp")]
    pub basin_amp: f32,
    /// Multiplier for LGM paleo-glacial cirque steps and intensity above `3,350 m` (`0.5..=5.0`).
    #[serde(default = "default_glacial_mult")]
    pub glacial_mult: f32,
    /// Multi-octave fractal coastline amplitude (`0.0..=0.12`).
    #[serde(default = "default_coast_frac_amp")]
    pub coast_frac_amp: f32,
    /// Target headland fraction along rocky coastal arcs (`0.0..=0.70`).
    #[serde(default = "default_coast_cliff_share")]
    pub coast_cliff_share: f32,
    /// Substeps for fine-scale (`< 30 m`) Stage S3 thermal/talus relaxation (`1..=64`).
    #[serde(default = "default_talus_substeps")]
    pub talus_substeps: usize,
    /// Opt-in multiplier for Stage S2/S3 multigrid steps at levels `>= 1024` (`1.0` = trunk).
    #[serde(default = "default_one_f32")]
    pub fine_steps_mult: f32,
    /// Opt-in multiplier on Stage S2 Fastscape-SD `dt_yr` (`1.0` = trunk).
    #[serde(default = "default_one_f32")]
    pub fluvial_dt_mult: f32,
    /// Opt-in stream-power area exponent `m` (`0.45` = trunk; concavity proxy `m/n` with `n = 1.0`).
    #[serde(default = "default_fluvial_m")]
    pub fluvial_m: f32,
    /// Opt-in multiplier on Stage S3 hillslope diffusivity `D` (`1.0` = trunk).
    #[serde(default = "default_one_f32")]
    pub hillslope_d_mult: f32,
    /// Opt-in hydrology routing diagnostic forcing pure D8 on hillslopes (`false` = trunk Hybrid MFD->D8).
    #[serde(default)]
    pub force_pure_d8: bool,
    /// Opt-in multi-scale ridge/fault roughness factor on `MultiEnvironment` massif uplift (`0.0` = trunk).
    #[serde(default)]
    pub uplift_roughness: f32,
    /// Opt-in initial massif bedrock fraction (`1.0` = trunk pre-formed massifs; `< 1.0` grows from lower initial relief).
    #[serde(default = "default_one_f32")]
    pub init_bedrock_frac: f32,
    /// Opt-in scalloped piedmont & oblique foothill dissection for `MultiEnvironment` (`0.0` = trunk).
    #[serde(default)]
    pub foothill_dissection: f32,
    /// Opt-in boundary warp and canyon/cuesta dissection on `MultiEnvironment` caprock plateau (`0.0` = trunk).
    #[serde(default)]
    pub plateau_dissection: f32,
}

const fn default_channel_init() -> f32 {
    100.0
}
const fn default_mfd_p() -> f32 {
    1.90
}
const fn default_storm_runoff() -> f32 {
    0.12
}
const fn default_fluvial_k_mult() -> f32 {
    1.65
}
const fn default_sapping_gain() -> f32 {
    1.5
}
const fn default_basin_amp() -> f32 {
    1.35
}
const fn default_glacial_mult() -> f32 {
    1.8
}
const fn default_coast_frac_amp() -> f32 {
    0.055
}
const fn default_coast_cliff_share() -> f32 {
    0.28
}
const fn default_talus_substeps() -> usize {
    20
}
const fn default_one_f32() -> f32 {
    1.0
}
const fn default_fluvial_m() -> f32 {
    0.45
}

impl Default for WorldV2Config {
    fn default() -> Self {
        Self {
            seed: 0x0057_5632_2026_1120,
            schema_tag: "ANMW3-SNSM-v1".to_owned(),
            layout: WorldLayout::default(),
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
            channel_init: default_channel_init(),
            mfd_p: default_mfd_p(),
            storm_runoff: default_storm_runoff(),
            fluvial_k_mult: default_fluvial_k_mult(),
            sapping_gain: default_sapping_gain(),
            basin_amp: default_basin_amp(),
            glacial_mult: default_glacial_mult(),
            coast_frac_amp: default_coast_frac_amp(),
            coast_cliff_share: default_coast_cliff_share(),
            talus_substeps: default_talus_substeps(),
            fine_steps_mult: default_one_f32(),
            fluvial_dt_mult: default_one_f32(),
            fluvial_m: default_fluvial_m(),
            hillslope_d_mult: default_one_f32(),
            force_pure_d8: false,
            uplift_roughness: 0.0,
            init_bedrock_frac: default_one_f32(),
            foothill_dissection: 0.0,
            plateau_dissection: 0.0,
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
        if self.grid_size < 64 || !self.grid_size.is_power_of_two() {
            return Err(err("grid_size must be a power of two >= 64"));
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
        if !self.channel_init.is_finite()
            || self.channel_init <= 0.0
            || self.channel_init > 10_000.0
        {
            return Err(err("channel_init must lie in (0.0, 10000.0]"));
        }
        if !self.mfd_p.is_finite() || !(1.0..=4.0).contains(&self.mfd_p) {
            return Err(err("mfd_p must lie within [1.0, 4.0]"));
        }
        if !self.storm_runoff.is_finite() || !(0.0..=0.50).contains(&self.storm_runoff) {
            return Err(err("storm_runoff must lie within [0.0, 0.50]"));
        }
        if !self.fluvial_k_mult.is_finite() || !(0.2..=10.0).contains(&self.fluvial_k_mult) {
            return Err(err("fluvial_k_mult must lie within [0.2, 10.0]"));
        }
        if !self.sapping_gain.is_finite() || !(0.0..=6.0).contains(&self.sapping_gain) {
            return Err(err("sapping_gain must lie within [0.0, 6.0]"));
        }
        if !self.basin_amp.is_finite() || !(0.5..=3.0).contains(&self.basin_amp) {
            return Err(err("basin_amp must lie within [0.5, 3.0]"));
        }
        if !self.glacial_mult.is_finite() || !(0.5..=5.0).contains(&self.glacial_mult) {
            return Err(err("glacial_mult must lie within [0.5, 5.0]"));
        }
        if !self.coast_frac_amp.is_finite() || !(0.0..=0.12).contains(&self.coast_frac_amp) {
            return Err(err("coast_frac_amp must lie within [0.0, 0.12]"));
        }
        if !self.coast_cliff_share.is_finite() || !(0.0..=0.70).contains(&self.coast_cliff_share) {
            return Err(err("coast_cliff_share must lie within [0.0, 0.70]"));
        }
        if !(1..=64).contains(&self.talus_substeps) {
            return Err(err("talus_substeps must lie within [1, 64]"));
        }
        if !self.fine_steps_mult.is_finite() || !(0.25..=16.0).contains(&self.fine_steps_mult) {
            return Err(err("fine_steps_mult must lie within [0.25, 16.0]"));
        }
        if !self.fluvial_dt_mult.is_finite() || !(0.1..=20.0).contains(&self.fluvial_dt_mult) {
            return Err(err("fluvial_dt_mult must lie within [0.1, 20.0]"));
        }
        if !self.fluvial_m.is_finite() || !(0.20..=0.80).contains(&self.fluvial_m) {
            return Err(err("fluvial_m must lie within [0.20, 0.80]"));
        }
        if !self.hillslope_d_mult.is_finite() || !(0.0..=16.0).contains(&self.hillslope_d_mult) {
            return Err(err("hillslope_d_mult must lie within [0.0, 16.0]"));
        }
        if !self.uplift_roughness.is_finite() || !(0.0..=2.0).contains(&self.uplift_roughness) {
            return Err(err("uplift_roughness must lie within [0.0, 2.0]"));
        }
        if !self.init_bedrock_frac.is_finite() || !(0.05..=1.0).contains(&self.init_bedrock_frac) {
            return Err(err("init_bedrock_frac must lie within [0.05, 1.0]"));
        }
        if !self.foothill_dissection.is_finite() || !(0.0..=2.0).contains(&self.foothill_dissection)
        {
            return Err(err("foothill_dissection must lie within [0.0, 2.0]"));
        }
        if !self.plateau_dissection.is_finite() || !(0.0..=2.0).contains(&self.plateau_dissection) {
            return Err(err("plateau_dissection must lie within [0.0, 2.0]"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{WorldLayout, WorldV2Config};

    #[test]
    fn default_layout_is_single_massif_and_legacy_toml_defaults_to_it() {
        assert_eq!(WorldV2Config::default().layout, WorldLayout::SingleMassif);
        let legacy_toml = include_str!("../presets/wv2_c01_40960.toml");
        let parsed: WorldV2Config = toml::from_str(legacy_toml).expect("parse legacy preset");
        assert_eq!(parsed.layout, WorldLayout::SingleMassif);
        assert!(parsed.validate().is_ok());
    }

    #[test]
    fn shipped_preset_v2_is_the_wv2_big_81920_preset() {
        let shipped_toml = include_str!("../../../../artifacts/world-v2/preset_v2.toml");
        let big_toml = include_str!("../presets/wv2_big_81920.toml");
        let shipped: WorldV2Config =
            toml::from_str(shipped_toml).expect("parse shipped preset_v2.toml");
        let big: WorldV2Config = toml::from_str(big_toml).expect("parse wv2_big_81920.toml");
        assert_eq!(shipped, big);
        assert_eq!(shipped.layout, WorldLayout::MultiEnvironment);
        assert!(shipped.validate().is_ok());
    }

    #[test]
    fn wv2_big_81920_preset_toml_parses_and_validates() {
        let big_toml = include_str!("../presets/wv2_big_81920.toml");
        let parsed: WorldV2Config = toml::from_str(big_toml).expect("parse wv2_big_81920.toml");
        assert_eq!(parsed.layout, WorldLayout::MultiEnvironment);
        assert!((parsed.extent_m - 81_920.0).abs() < 1e-3);
        assert_eq!(parsed.grid_size, 4096);
        assert!(parsed.validate().is_ok());
    }

    #[test]
    fn shipped_presets_deserialize_with_exact_opt_in_diagnostic_defaults() {
        for raw in [
            include_str!("../presets/wv2_c01_40960.toml"),
            include_str!("../presets/wv2_big_81920.toml"),
        ] {
            let cfg: WorldV2Config = toml::from_str(raw).expect("parse shipped preset");
            assert_eq!(cfg.fine_steps_mult.to_bits(), 1.0_f32.to_bits());
            assert_eq!(cfg.fluvial_dt_mult.to_bits(), 1.0_f32.to_bits());
            assert_eq!(cfg.fluvial_m.to_bits(), 0.45_f32.to_bits());
            assert_eq!(cfg.hillslope_d_mult.to_bits(), 1.0_f32.to_bits());
            assert!(!cfg.force_pure_d8);
            assert_eq!(cfg.uplift_roughness.to_bits(), 0.0_f32.to_bits());
            assert_eq!(cfg.init_bedrock_frac.to_bits(), 1.0_f32.to_bits());
            assert_eq!(cfg.foothill_dissection.to_bits(), 0.0_f32.to_bits());
            assert_eq!(cfg.plateau_dissection.to_bits(), 0.0_f32.to_bits());
        }
    }

    #[test]
    fn shape_exp_preset_toml_parses_and_validates() {
        let raw = include_str!("../presets/wv2_big_81920_shape_exp.toml");
        let cfg: WorldV2Config = toml::from_str(raw).expect("parse wv2_big_81920_shape_exp.toml");
        assert_eq!(cfg.layout, WorldLayout::MultiEnvironment);
        assert!((cfg.extent_m - 81_920.0).abs() < 1e-3);
        assert!(cfg.foothill_dissection > 0.0);
        assert!(cfg.plateau_dissection > 0.0);
        assert!(cfg.validate().is_ok());
    }
}
