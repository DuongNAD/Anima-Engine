#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

//! Offline World v2 bake pipeline (`WV2-5`, Card 6):
//! - `fields`: central-difference normals, exact Meijster (2000) 2-pass `shore_sdf`, `hydro_flow`, and seeded decorative `flora_tiles`
//! - `lighting_cpu`: Stewart (1998) blocked upper-hull horizon AO/SVF/bounce/4-azimuth horizon sweep
//! - `lighting_gpu`: optional `wgpu` adapter probe (`JOB-W0`) & compute shader parity (`gpu-bake`)
//! - `pack_ktx2`: pure-Rust KTX2 v2.0 + BC4/BC5 encoder, ZLIB-compressed tile pyramid, deterministic tar.gz archiver, & SHA-256 verifier
//! - `assets`: `QD-ASSETS` CC0 manifest validator, deterministic `.obj` LOD simplifier, and octahedral impostor baker

pub mod assets;
pub mod fields;
pub mod flora_place;
pub mod lighting_cpu;
#[cfg(feature = "gpu-bake")]
pub mod lighting_gpu;
pub mod pack_ktx2;

use crate::anmw_v3::AnmwV3Error;
use crate::caves::CaveCodecError;
use crate::pipeline::PipelineError;
use crate::BiomeId;
use std::fmt;

/// Active texels per tile edge (`256 × 256` active texels, `§3.2`).
pub const TILE_ACTIVE_SIZE: usize = 256;

/// Bilinear overlap gutter in texels on every tile border (`2` texels, `§3.2`).
pub const TILE_GUTTER: usize = 2;

/// Total padded tile edge length (`256 + 2 * 2 = 260`, exact multiple of 4 for BCn/UASTC).
pub const TILE_TOTAL_SIZE: usize = TILE_ACTIVE_SIZE + 2 * TILE_GUTTER;

/// Texture compression block width/height (`4 × 4`).
pub const BC_BLOCK_SIZE: usize = 4;

/// Horizon search radius in fine grid cells (`256` cells = `2,560 m` @ `10 m/cell`, `§6.2`).
pub const HORIZON_HALO_CELLS: usize = 256;

/// Clamp distance in metres for `shore_sdf` (`[-500, +500] m`, `§3.2`).
pub const SHORE_SDF_CLAMP_M: f32 = 500.0;

/// Error returned across `anima-worldgen::bake` operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakeError {
    pub message: String,
}

impl BakeError {
    #[inline]
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for BakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BakeError {}

impl From<PipelineError> for BakeError {
    fn from(err: PipelineError) -> Self {
        Self::new(err.message)
    }
}

impl From<AnmwV3Error> for BakeError {
    fn from(err: AnmwV3Error) -> Self {
        Self::new(err.message)
    }
}

impl From<CaveCodecError> for BakeError {
    fn from(err: CaveCodecError) -> Self {
        Self::new(err.message)
    }
}

impl From<std::io::Error> for BakeError {
    fn from(err: std::io::Error) -> Self {
        Self::new(err.to_string())
    }
}

impl From<serde_json::Error> for BakeError {
    fn from(err: serde_json::Error) -> Self {
        Self::new(err.to_string())
    }
}

/// Strip sharding specifier `--tile-strip <S>/<K>` (`0 <= index < count`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileStrip {
    pub index: usize,
    pub count: usize,
}

impl TileStrip {
    /// Parses `<S>/<K>` (e.g. `"0/4"`).
    pub fn parse(raw: &str) -> Result<Self, BakeError> {
        let (s_str, k_str) = raw.split_once('/').ok_or_else(|| {
            BakeError::new(format!("invalid --tile-strip '{raw}': expected <S>/<K>"))
        })?;
        let index: usize = s_str
            .parse()
            .map_err(|e| BakeError::new(format!("invalid strip index '{s_str}': {e}")))?;
        let count: usize = k_str
            .parse()
            .map_err(|e| BakeError::new(format!("invalid strip count '{k_str}': {e}")))?;
        if count == 0 || index >= count {
            return Err(BakeError::new(format!(
                "invalid --tile-strip '{raw}': require K > 0 and 0 <= S < K"
            )));
        }
        Ok(Self { index, count })
    }

    /// Contiguous row range `[row_start, row_end)` partitioning `0..height`.
    #[must_use]
    pub const fn row_bounds(&self, height: usize) -> (usize, usize) {
        let start = (self.index * height) / self.count;
        let end = ((self.index + 1) * height) / self.count;
        (start, end)
    }
}

/// Linear tile-index range `--tile-range <A>..<B>` (`start < end`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileRange {
    pub start: usize,
    pub end: usize,
}

impl TileRange {
    /// Parses `<A>..<B>` (half-open range `[A, B)`).
    pub fn parse(raw: &str) -> Result<Self, BakeError> {
        let (a_str, b_str) = raw.split_once("..").ok_or_else(|| {
            BakeError::new(format!("invalid --tile-range '{raw}': expected <A>..<B>"))
        })?;
        let start: usize = a_str
            .parse()
            .map_err(|e| BakeError::new(format!("invalid range start '{a_str}': {e}")))?;
        let end: usize = b_str
            .parse()
            .map_err(|e| BakeError::new(format!("invalid range end '{b_str}': {e}")))?;
        if start >= end {
            return Err(BakeError::new(format!(
                "invalid --tile-range '{raw}': require A < B"
            )));
        }
        Ok(Self { start, end })
    }
}

/// Dimensions of one mip level inside the tile pyramid (`L0` finest down to single-tile top mip).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MipLevelSpec {
    pub mip: u32,
    pub grid_size: usize,
    pub tiles_x: usize,
    pub tiles_y: usize,
}

/// Computes the power-of-two tile pyramid down to `256 × 256` (`1 × 1` tile).
pub fn compute_mip_pyramid(base_size: usize) -> Result<Vec<MipLevelSpec>, BakeError> {
    if base_size < TILE_ACTIVE_SIZE || !base_size.is_power_of_two() {
        return Err(BakeError::new(format!(
            "tile pyramid base_size ({base_size}) must be a power of two >= {TILE_ACTIVE_SIZE}"
        )));
    }
    let mut levels = Vec::new();
    let mut mip = 0_u32;
    let mut cur = base_size;
    while cur >= TILE_ACTIVE_SIZE {
        let tiles = cur / TILE_ACTIVE_SIZE;
        levels.push(MipLevelSpec {
            mip,
            grid_size: cur,
            tiles_x: tiles,
            tiles_y: tiles,
        });
        if cur == TILE_ACTIVE_SIZE {
            break;
        }
        cur /= 2;
        mip += 1;
    }
    Ok(levels)
}

/// Extracts one `(active + 2 * gutter) × (active + 2 * gutter)` tile with clamped border gutter.
#[must_use]
pub fn extract_tile_padded<T: Copy>(
    grid: &[T],
    width: usize,
    height: usize,
    tx: usize,
    ty: usize,
    active: usize,
    gutter: usize,
) -> Vec<T> {
    assert_eq!(grid.len(), width * height);
    let total = active + 2 * gutter;
    let mut out = Vec::with_capacity(total * total);
    let max_x = width.saturating_sub(1) as isize;
    let max_z = height.saturating_sub(1) as isize;
    let base_x = (tx * active) as isize - (gutter as isize);
    let base_z = (ty * active) as isize - (gutter as isize);
    for lr in 0..total {
        let gz = (base_z + lr as isize).clamp(0, max_z) as usize;
        let row_off = gz * width;
        for lc in 0..total {
            let gx = (base_x + lc as isize).clamp(0, max_x) as usize;
            out.push(grid[row_off + gx]);
        }
    }
    out
}

/// Verifies that the overlapping gutter band between two horizontally adjacent padded tiles
/// (`tile_left` at `(tx, ty)`, `tile_right` at `(tx + 1, ty)`) matches texel-for-texel.
pub fn verify_horizontal_gutter_overlap<T: PartialEq + Copy>(
    tile_left: &[T],
    tile_right: &[T],
    active: usize,
    gutter: usize,
) -> Result<(), BakeError> {
    if gutter < 1 {
        return Err(BakeError::new(format!(
            "tile gutter must be >= 1 to provide boundary overlap (got {gutter})"
        )));
    }
    let total = active + 2 * gutter;
    if tile_left.len() != total * total || tile_right.len() != total * total {
        return Err(BakeError::new(format!(
            "tile buffer size mismatch for horizontal gutter check (expected {}x{})",
            total, total
        )));
    }
    let overlap_cols = 2 * gutter;
    for r in gutter..(gutter + active) {
        let left_start = r * total + active;
        let right_start = r * total;
        if tile_left[left_start..(left_start + overlap_cols)]
            != tile_right[right_start..(right_start + overlap_cols)]
        {
            return Err(BakeError::new(format!(
                "horizontal tile gutter boundary mismatch at local row {r}"
            )));
        }
    }
    Ok(())
}

/// Verifies that the overlapping gutter band between two vertically adjacent padded tiles
/// (`tile_top` at `(tx, ty)`, `tile_bottom` at `(tx, ty + 1)`) matches texel-for-texel.
pub fn verify_vertical_gutter_overlap<T: PartialEq + Copy>(
    tile_top: &[T],
    tile_bottom: &[T],
    active: usize,
    gutter: usize,
) -> Result<(), BakeError> {
    if gutter < 1 {
        return Err(BakeError::new(format!(
            "tile gutter must be >= 1 to provide boundary overlap (got {gutter})"
        )));
    }
    let total = active + 2 * gutter;
    if tile_top.len() != total * total || tile_bottom.len() != total * total {
        return Err(BakeError::new(format!(
            "tile buffer size mismatch for vertical gutter check (expected {}x{})",
            total, total
        )));
    }
    let overlap_rows = 2 * gutter;
    for dr in 0..overlap_rows {
        let top_row = (active + dr) * total;
        let bot_row = dr * total;
        if tile_top[(top_row + gutter)..(top_row + gutter + active)]
            != tile_bottom[(bot_row + gutter)..(bot_row + gutter + active)]
        {
            return Err(BakeError::new(format!(
                "vertical tile gutter boundary mismatch at overlap row {dr}"
            )));
        }
    }
    Ok(())
}

/// Quantizes `val` in `[0.0, 1.0]` deterministically into `0..=255`.
#[inline]
#[must_use]
pub fn quantize_u8(val: f32) -> u8 {
    let clamped = if val.is_finite() {
        val.clamp(0.0, 1.0)
    } else {
        0.0
    };
    (clamped * 255.0 + 0.5) as u8
}

/// Deterministic canonical sRGB macro-albedo RGB per cell shared identically between lighting & KTX2.
#[must_use]
pub fn compute_macro_albedo_rgb(
    dom_biome: u8,
    elev_m: f32,
    moisture: f32,
    talus_frac: f32,
    glacier_h_m: f32,
) -> [u8; 3] {
    if glacier_h_m > 0.25 {
        return [226, 236, 245];
    }
    let base: [f32; 3] = match BiomeId::from_u8(dom_biome) {
        Some(BiomeId::Ocean) => [24.0, 62.0, 96.0],
        Some(BiomeId::Beach) => [206.0, 189.0, 148.0],
        Some(BiomeId::Desert) => [198.0, 164.0, 112.0],
        Some(BiomeId::Savanna) => [156.0, 152.0, 86.0],
        Some(BiomeId::Grassland) => [92.0, 134.0, 68.0],
        Some(BiomeId::Shrubland) => [124.0, 129.0, 79.0],
        Some(BiomeId::Forest) => [48.0, 96.0, 52.0],
        Some(BiomeId::Jungle) => [34.0, 84.0, 42.0],
        Some(BiomeId::Taiga) => [42.0, 74.0, 52.0],
        Some(BiomeId::Tundra) => [118.0, 122.0, 92.0],
        Some(BiomeId::Swamp) => [56.0, 82.0, 54.0],
        Some(BiomeId::Rock) => [118.0, 114.0, 108.0],
        Some(BiomeId::Snow) => [234.0, 239.0, 244.0],
        Some(BiomeId::River) => [42.0, 92.0, 118.0],
        Some(BiomeId::Lake) => [34.0, 84.0, 112.0],
        Some(BiomeId::Mangrove) => [44.0, 78.0, 48.0],
        Some(BiomeId::Chaparral) => [126.0, 128.0, 76.0],
        Some(BiomeId::Steppe) => [164.0, 146.0, 98.0],
        Some(BiomeId::Alpine) => [102.0, 124.0, 78.0],
        Some(BiomeId::Badlands) => [164.0, 116.0, 86.0],
        Some(BiomeId::Glacier) => [224.0, 236.0, 246.0],
        Some(BiomeId::Bog) => [88.0, 102.0, 68.0],
        None => [112.0, 112.0, 112.0],
    };

    let m_mod = 0.90 + 0.20 * moisture.clamp(0.0, 1.0);
    let talus_blend = talus_frac.clamp(0.0, 0.75);
    let elev_bright = ((elev_m.clamp(-1500.0, 5700.0) + 1500.0) / 7200.0 - 0.3) * 14.0;
    let rock = [118.0_f32, 114.0, 108.0];
    let mut rgb = [0_u8; 3];
    for ch in 0..3 {
        let v = base[ch] * m_mod * (1.0 - talus_blend) + rock[ch] * talus_blend + elev_bright;
        rgb[ch] = v.round().clamp(0.0, 255.0) as u8;
    }
    rgb
}

/// Computes Rec.709 relative luminance `[0.0, 1.0]` from an sRGB `[u8; 3]` triple.
#[inline]
#[must_use]
pub fn srgb_luma(rgb: [u8; 3]) -> f32 {
    (0.2126 * f32::from(rgb[0]) + 0.7152 * f32::from(rgb[1]) + 0.0722 * f32::from(rgb[2])) / 255.0
}

/// Builds per-cell scalar albedo luma grid (`[0.0, 1.0]`) from raw layers.
#[must_use]
pub fn build_albedo_luma_grid(
    dominant_biome: &[u8],
    total_h_m: &[f32],
    moisture: &[f32],
    talus_frac: &[f32],
    glacier_h_m: &[f32],
) -> Vec<f32> {
    let n = total_h_m.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let rgb = compute_macro_albedo_rgb(
            dominant_biome[i],
            total_h_m[i],
            moisture[i],
            talus_frac[i],
            glacier_h_m[i],
        );
        out.push(srgb_luma(rgb));
    }
    out
}

/// Downsamples a scalar `f32` grid by `2 × 2` mean pooling.
#[must_use]
pub fn downsample_f32_2x2(src: &[f32], width: usize, height: usize) -> Vec<f32> {
    let nw = width / 2;
    let nh = height / 2;
    let mut dst = Vec::with_capacity(nw * nh);
    for r in 0..nh {
        let r0 = r * 2;
        let r1 = r0 + 1;
        for c in 0..nw {
            let c0 = c * 2;
            let c1 = c0 + 1;
            let s = src[r0 * width + c0]
                + src[r0 * width + c1]
                + src[r1 * width + c0]
                + src[r1 * width + c1];
            dst.push(0.25 * s);
        }
    }
    dst
}

/// Downsamples an `[u8; 4]` grid by exact `2 × 2` box average.
#[must_use]
pub fn downsample_u8x4_mean_2x2(src: &[[u8; 4]], width: usize, height: usize) -> Vec<[u8; 4]> {
    let nw = width / 2;
    let nh = height / 2;
    let mut dst = Vec::with_capacity(nw * nh);
    for r in 0..nh {
        let r0 = r * 2;
        let r1 = r0 + 1;
        for c in 0..nw {
            let c0 = c * 2;
            let c1 = c0 + 1;
            let a = src[r0 * width + c0];
            let b = src[r0 * width + c1];
            let d = src[r1 * width + c0];
            let e = src[r1 * width + c1];
            let mut px = [0_u8; 4];
            for ch in 0..4 {
                let sum = u16::from(a[ch]) + u16::from(b[ch]) + u16::from(d[ch]) + u16::from(e[ch]);
                px[ch] = ((sum + 2) / 4) as u8;
            }
            dst.push(px);
        }
    }
    dst
}

/// Downsamples an `[u8; 2]` grid by exact `2 × 2` box average.
#[must_use]
pub fn downsample_u8x2_mean_2x2(src: &[[u8; 2]], width: usize, height: usize) -> Vec<[u8; 2]> {
    let nw = width / 2;
    let nh = height / 2;
    let mut dst = Vec::with_capacity(nw * nh);
    for r in 0..nh {
        let r0 = r * 2;
        let r1 = r0 + 1;
        for c in 0..nw {
            let c0 = c * 2;
            let c1 = c0 + 1;
            let a = src[r0 * width + c0];
            let b = src[r0 * width + c1];
            let d = src[r1 * width + c0];
            let e = src[r1 * width + c1];
            let s0 = u16::from(a[0]) + u16::from(b[0]) + u16::from(d[0]) + u16::from(e[0]);
            let s1 = u16::from(a[1]) + u16::from(b[1]) + u16::from(d[1]) + u16::from(e[1]);
            dst.push([((s0 + 2) / 4) as u8, ((s1 + 2) / 4) as u8]);
        }
    }
    dst
}

/// Downsamples a scalar `u8` mask by `2 × 2` max pooling (preserving sparse feature presence).
#[must_use]
pub fn downsample_u8_max_2x2(src: &[u8], width: usize, height: usize) -> Vec<u8> {
    let nw = width / 2;
    let nh = height / 2;
    let mut dst = Vec::with_capacity(nw * nh);
    for r in 0..nh {
        let r0 = r * 2;
        let r1 = r0 + 1;
        for c in 0..nw {
            let c0 = c * 2;
            let c1 = c0 + 1;
            let m = src[r0 * width + c0]
                .max(src[r0 * width + c1])
                .max(src[r1 * width + c0])
                .max(src[r1 * width + c1]);
            dst.push(m);
        }
    }
    dst
}
