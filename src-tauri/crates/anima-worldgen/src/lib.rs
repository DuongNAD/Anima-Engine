#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

//! Deterministic World v2 generator, `ANMW v3` codec, and offline bake pipeline.
//!
//! Card `WV2-1a` establishes the zero-default-dependency crate boundary (`reader` / `generate` /
//! `bake`), the `clippy.toml` ban on std transcendentals in favor of `libm`, domain-separated
//! `ChaCha12Rng` seeding, configuration validation, and the Stage S1 regional tectonic uplift &
//! stratigraphic lithology generator.

#[cfg(feature = "generate")]
pub mod biomes;
#[cfg(any(feature = "reader", feature = "generate"))]
pub mod caves;
#[cfg(feature = "generate")]
pub mod climate;
#[cfg(feature = "generate")]
pub mod config;
#[cfg(feature = "generate")]
pub mod deposition;
#[cfg(feature = "generate")]
pub mod erosion_fluvial;
#[cfg(feature = "generate")]
pub mod erosion_thermal;
#[cfg(feature = "generate")]
pub mod hydrology;
#[cfg(feature = "generate")]
pub mod lakes_falls;
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

/// Canonical number of biomes shared with `worldGen.ts` (`0..=21`) and `world_artifact.rs`.
pub const BIOME_COUNT: usize = 22;

/// Canonical 22-biome taxonomy matching `src/components/Landscape/utils/worldGen.ts:19-43`
/// and `src-tauri/src/core/world_artifact.rs:399-401`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "generate", derive(serde::Serialize, serde::Deserialize))]
pub enum BiomeId {
    Ocean = 0,
    Beach = 1,
    Desert = 2,
    Savanna = 3,
    Grassland = 4,
    Shrubland = 5,
    Forest = 6,
    Jungle = 7,
    Taiga = 8,
    Tundra = 9,
    Swamp = 10,
    Rock = 11,
    Snow = 12,
    River = 13,
    Lake = 14,
    Mangrove = 15,
    Chaparral = 16,
    Steppe = 17,
    Alpine = 18,
    Badlands = 19,
    Glacier = 20,
    Bog = 21,
}

impl BiomeId {
    /// All 22 canonical biomes ordered by `u8` index `0..=21`.
    pub const ALL: [Self; BIOME_COUNT] = [
        Self::Ocean,
        Self::Beach,
        Self::Desert,
        Self::Savanna,
        Self::Grassland,
        Self::Shrubland,
        Self::Forest,
        Self::Jungle,
        Self::Taiga,
        Self::Tundra,
        Self::Swamp,
        Self::Rock,
        Self::Snow,
        Self::River,
        Self::Lake,
        Self::Mangrove,
        Self::Chaparral,
        Self::Steppe,
        Self::Alpine,
        Self::Badlands,
        Self::Glacier,
        Self::Bog,
    ];

    /// The 16 terrestrial biomes gated by `G-COMP v2` `C1` area floor (`>= 2,500` cells @ `4096^2`).
    pub const ZONED_TERRESTRIAL_16: [Self; 16] = [
        Self::Mangrove,
        Self::Swamp,
        Self::Jungle,
        Self::Grassland,
        Self::Desert,
        Self::Badlands,
        Self::Savanna,
        Self::Shrubland,
        Self::Forest,
        Self::Taiga,
        Self::Alpine,
        Self::Tundra,
        Self::Bog,
        Self::Rock,
        Self::Snow,
        Self::Glacier,
    ];

    /// Converts a canonical `u8` index (`0..=21`) into [`BiomeId`].
    #[must_use]
    pub const fn from_u8(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::Ocean),
            1 => Some(Self::Beach),
            2 => Some(Self::Desert),
            3 => Some(Self::Savanna),
            4 => Some(Self::Grassland),
            5 => Some(Self::Shrubland),
            6 => Some(Self::Forest),
            7 => Some(Self::Jungle),
            8 => Some(Self::Taiga),
            9 => Some(Self::Tundra),
            10 => Some(Self::Swamp),
            11 => Some(Self::Rock),
            12 => Some(Self::Snow),
            13 => Some(Self::River),
            14 => Some(Self::Lake),
            15 => Some(Self::Mangrove),
            16 => Some(Self::Chaparral),
            17 => Some(Self::Steppe),
            18 => Some(Self::Alpine),
            19 => Some(Self::Badlands),
            20 => Some(Self::Glacier),
            21 => Some(Self::Bog),
            _ => None,
        }
    }

    /// Returns `true` for the 4 water / coastal strip classes (`Ocean`, `Beach`, `River`, `Lake`).
    #[must_use]
    pub const fn is_water_or_strip(self) -> bool {
        matches!(self, Self::Ocean | Self::Beach | Self::River | Self::Lake)
    }

    /// Returns `true` for the 2 biomes absent by design in World v2 (`Chaparral`, `Steppe`).
    #[must_use]
    pub const fn is_absent_in_wv2(self) -> bool {
        matches!(self, Self::Chaparral | Self::Steppe)
    }

    /// Returns `true` for the 16 zoned terrestrial biomes present in World v2.
    #[must_use]
    pub const fn is_zoned_terrestrial(self) -> bool {
        !self.is_water_or_strip() && !self.is_absent_in_wv2()
    }
}

/// Number of canonical water-surface kinds (`0..=6`, `§3.2` `water_surface.G8`).
pub const WATER_KIND_COUNT: usize = 7;

/// Canonical per-cell water surface classification (`0..=6`, `§3.2`, `§3.4`, `§4.3`).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "generate", derive(serde::Serialize, serde::Deserialize))]
pub enum WaterKind {
    None = 0,
    Ocean = 1,
    FreshwaterLake = 2,
    EndorheicLake = 3,
    River = 4,
    Waterfall = 5,
    DeltaTurbid = 6,
}

impl WaterKind {
    /// Converts a canonical `u8` index (`0..=6`) into [`WaterKind`].
    #[must_use]
    pub const fn from_u8(kind: u8) -> Option<Self> {
        match kind {
            0 => Some(Self::None),
            1 => Some(Self::Ocean),
            2 => Some(Self::FreshwaterLake),
            3 => Some(Self::EndorheicLake),
            4 => Some(Self::River),
            5 => Some(Self::Waterfall),
            6 => Some(Self::DeltaTurbid),
            _ => None,
        }
    }

    /// Returns `true` for inland lakes (`FreshwaterLake` or `EndorheicLake`).
    #[must_use]
    pub const fn is_lake(self) -> bool {
        matches!(self, Self::FreshwaterLake | Self::EndorheicLake)
    }
}

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

    #[test]
    fn biome_ids_and_water_kinds_cover_canonical_indices() {
        assert_eq!(BIOME_COUNT, 22);
        assert_eq!(BiomeId::ALL.len(), 22);
        assert_eq!(BiomeId::ZONED_TERRESTRIAL_16.len(), 16);

        let mut water_strip_count = 0usize;
        let mut zoned_count = 0usize;
        let mut absent_count = 0usize;
        for idx in 0..BIOME_COUNT as u8 {
            let b = BiomeId::from_u8(idx).expect("valid biome index");
            assert_eq!(b as u8, idx);
            assert_eq!(BiomeId::ALL[idx as usize], b);
            if b.is_water_or_strip() {
                water_strip_count += 1;
            } else if b.is_absent_in_wv2() {
                absent_count += 1;
            } else if b.is_zoned_terrestrial() {
                zoned_count += 1;
            }
        }
        assert_eq!(water_strip_count, 4);
        assert_eq!(zoned_count, 16);
        assert_eq!(absent_count, 2);
        assert_eq!(BiomeId::from_u8(22), None);
        assert_eq!(BiomeId::Chaparral as u8, 16);
        assert_eq!(BiomeId::Steppe as u8, 17);
        assert!(BiomeId::Chaparral.is_absent_in_wv2());
        assert!(BiomeId::Steppe.is_absent_in_wv2());
        for &b in &BiomeId::ZONED_TERRESTRIAL_16 {
            assert!(b.is_zoned_terrestrial());
        }

        assert_eq!(WATER_KIND_COUNT, 7);
        for idx in 0..WATER_KIND_COUNT as u8 {
            let w = WaterKind::from_u8(idx).expect("valid water kind");
            assert_eq!(w as u8, idx);
        }
        assert_eq!(WaterKind::from_u8(7), None);
        assert!(WaterKind::FreshwaterLake.is_lake());
        assert!(WaterKind::EndorheicLake.is_lake());
        assert!(!WaterKind::Ocean.is_lake());
        assert!(!WaterKind::River.is_lake());
    }
}
