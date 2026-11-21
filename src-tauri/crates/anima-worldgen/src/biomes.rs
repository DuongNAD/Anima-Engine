//! Stage S8: Continuous `C^2` log-likelihood blending across the 22 canonical biomes (`§2.2.8`,
//! `§5.1` `C1`/`C2`), micro-ecotone domain warping (`40–150 m`), top-4 `biome_ids` + `biome_weights`
//! (`sum == 255`), cirque glaciers (`Oerlemans 2001`; `Cuffey & Paterson 2010`) strictly `>= ELA(x)`
//! and `slope <= 42°`, `moraine_debris`, and leeward Werner (1995) aeolian dunes (`~12°` stoss /
//! `32–34°` slip face).

use crate::{BiomeId, WaterKind, BIOME_COUNT};
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::fmt;

/// Error returned when Stage S8 inputs or configuration are invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiomeError {
    pub message: String,
}

impl fmt::Display for BiomeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BiomeError {}

/// Configuration for Stage S8 continuous biome classification, cirque glaciers, and dunes.
#[derive(Debug, Clone, PartialEq)]
pub struct BiomeConfig {
    pub dx_m: f32,
    pub sea_level_m: f32,
    /// Regional Equilibrium Line Altitude base (`4,900 m`).
    pub ela_base_m: f32,
    /// Hard floor offset below `ela_base_m` (`350 m` -> `4,550 m`) forbidding `Glacier`.
    pub glacier_floor_offset_m: f32,
    /// Maximum surface slope (`42.0°`) supporting glacial ice accumulation.
    pub glacier_max_slope_deg: f32,
    /// Micro-ecotone domain warp amplitude in metres (`40..=150 m`, default `90 m`).
    pub ecotone_warp_m: f32,
    /// Softmax inverse-temperature gain controlling ecotone transition width.
    pub softmax_sharpness: f32,
    /// Prevailing onshore wind unit vector `(ux, uz)` orienting Werner leeward dunes.
    pub wind_dir: (f32, f32),
    /// Deterministic seed for stateless micro-ecotone warp and dune phase.
    pub seed: u64,
}

impl Default for BiomeConfig {
    fn default() -> Self {
        Self {
            dx_m: 10.0,
            sea_level_m: 0.0,
            ela_base_m: 4_900.0,
            glacier_floor_offset_m: 350.0,
            glacier_max_slope_deg: 42.0,
            ecotone_warp_m: 90.0,
            softmax_sharpness: 1.30,
            wind_dir: (1.0, 0.0),
            seed: 0x0057_5632_2026_1120,
        }
    }
}

/// Borrowed input slices consumed by [`classify_biomes`].
pub struct BiomeInput<'a> {
    pub width: usize,
    pub height: usize,
    pub total_h_m: &'a [f32],
    pub temp_c: &'a [f32],
    pub precip_m_yr: &'a [f32],
    pub moisture: &'a [f32],
    pub twi: &'a [f32],
    pub ela_local_m: &'a [f32],
    pub alluvial_mask: &'a [f32],
    pub talus_frac: &'a [f32],
    pub sediment_s_m: &'a [f32],
    pub water_kind: &'a [u8],
    pub river_w_m: &'a [f32],
    pub lithology_id: &'a [u8],
}

/// Output buffers produced deterministically by [`classify_biomes`].
#[derive(Debug, Clone)]
pub struct BiomeField {
    pub width: usize,
    pub height: usize,
    /// Top-4 canonical [`BiomeId`] indices per cell sorted descending by weight.
    pub biome_ids: Vec<[u8; 4]>,
    /// Top-4 quantized weights summing to `255` at every cell.
    pub biome_weights: Vec<[u8; 4]>,
    /// Dominant biome (`biome_ids[i][0]`).
    pub dominant_biome: Vec<u8>,
    /// Cirque/summit glacier ice thickness in metres (`0.0` below ELA or on `slope > 42°`).
    pub glacier_h_m: Vec<f32>,
    /// Supraglacial and lateral moraine debris density `[0..=255]`.
    pub moraine_debris: Vec<u8>,
    /// Leeward Werner (1995) aeolian dune height in metres.
    pub dune_h_m: Vec<f32>,
}

impl BiomeField {
    /// Computes deterministic SHA-256 over dimensions and all output buffers.
    #[must_use]
    pub fn sha256_bytes(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update((self.width as u32).to_le_bytes());
        hasher.update((self.height as u32).to_le_bytes());
        for ids in &self.biome_ids {
            hasher.update(ids);
        }
        for w in &self.biome_weights {
            hasher.update(w);
        }
        hasher.update(&self.dominant_biome);
        for &g in &self.glacier_h_m {
            hasher.update(g.to_bits().to_le_bytes());
        }
        hasher.update(&self.moraine_debris);
        for &d in &self.dune_h_m {
            hasher.update(d.to_bits().to_le_bytes());
        }
        let digest = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }
}

#[inline]
fn hash_lattice(seed: u64, ix: i64, iz: i64) -> f32 {
    let mut z = seed
        .wrapping_add((ix as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add((iz as u64).wrapping_mul(0x94D0_49BB_1331_11EB));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 * (1.0 / ((1u32 << 24) as f32))
}

#[inline]
fn quintic_noise_2d(seed: u64, x: f32, z: f32) -> f32 {
    let fx = libm::floorf(x);
    let fz = libm::floorf(z);
    let ix = fx as i64;
    let iz = fz as i64;
    let tx = x - fx;
    let tz = z - fz;
    let sx = tx * tx * tx * (tx * (tx * 6.0 - 15.0) + 10.0);
    let sz = tz * tz * tz * (tz * (tz * 6.0 - 15.0) + 10.0);
    let v00 = hash_lattice(seed, ix, iz);
    let v10 = hash_lattice(seed, ix + 1, iz);
    let v01 = hash_lattice(seed, ix, iz + 1);
    let v11 = hash_lattice(seed, ix + 1, iz + 1);
    let vx0 = v00 + sx * (v10 - v00);
    let vx1 = v01 + sx * (v11 - v01);
    (vx0 + sz * (vx1 - vx0)) * 2.0 - 1.0
}

#[inline]
fn smoothstep_c2(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < 1e-6 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn gauss_ll(x: f32, mu: f32, sigma: f32) -> f32 {
    let z = (x - mu) / sigma.max(1e-4);
    -0.5 * z * z
}

#[inline]
fn gate_ll(t_c2: f32) -> f32 {
    1.65 * libm::logf(t_c2.clamp(1e-6, 1.0))
}

/// Computes `u16` Chamfer `(3, 4)` coastal distance (1 step = `dx_m / 3.0`), `32 MiB` @ `4096^2`.
fn compute_coast_distance_u16(
    total_h_m: &[f32],
    water_kind: &[u8],
    w: usize,
    h: usize,
    sea_level_m: f32,
) -> Vec<u16> {
    let n = w * h;
    let mut dist = vec![u16::MAX; n];
    for i in 0..n {
        let wk = water_kind[i];
        let is_ocean = wk == WaterKind::Ocean as u8
            || (total_h_m[i] <= sea_level_m
                && wk != WaterKind::FreshwaterLake as u8
                && wk != WaterKind::EndorheicLake as u8);
        if is_ocean {
            dist[i] = 0;
        }
    }
    for r in 0..h {
        for c in 0..w {
            let i = r * w + c;
            let mut d = dist[i];
            if d == 0 {
                continue;
            }
            if c > 0 {
                d = d.min(dist[i - 1].saturating_add(3));
            }
            if r > 0 {
                d = d.min(dist[i - w].saturating_add(3));
                if c > 0 {
                    d = d.min(dist[i - w - 1].saturating_add(4));
                }
                if c + 1 < w {
                    d = d.min(dist[i - w + 1].saturating_add(4));
                }
            }
            dist[i] = d;
        }
    }
    for r in (0..h).rev() {
        for c in (0..w).rev() {
            let i = r * w + c;
            let mut d = dist[i];
            if d == 0 {
                continue;
            }
            if c + 1 < w {
                d = d.min(dist[i + 1].saturating_add(3));
            }
            if r + 1 < h {
                d = d.min(dist[i + w].saturating_add(3));
                if c + 1 < w {
                    d = d.min(dist[i + w + 1].saturating_add(4));
                }
                if c > 0 {
                    d = d.min(dist[i + w - 1].saturating_add(4));
                }
            }
            dist[i] = d;
        }
    }
    dist
}

/// Packs top-4 non-absent biomes with deterministic Hamilton largest-remainder rounding (`sum == 255`).
fn pack_top4_weights(probs: &[f32; BIOME_COUNT]) -> ([u8; 4], [u8; 4]) {
    let mut ranked: [(u8, f32); BIOME_COUNT] = core::array::from_fn(|i| (i as u8, probs[i]));
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut ids = [
        BiomeId::Rock as u8,
        BiomeId::Grassland as u8,
        BiomeId::Forest as u8,
        BiomeId::Alpine as u8,
    ];
    let mut top_p = [0.0_f32; 4];
    let mut k = 0usize;
    for &(id, p) in &ranked {
        if id == BiomeId::Chaparral as u8 || id == BiomeId::Steppe as u8 {
            continue;
        }
        ids[k] = id;
        top_p[k] = p.max(0.0);
        k += 1;
        if k == 4 {
            break;
        }
    }
    let sum: f32 = top_p.iter().sum();
    let mut weights = [0_u8; 4];
    if sum <= 0.0 || !sum.is_finite() {
        weights[0] = 255;
        return (ids, weights);
    }

    let mut rem = [(0_usize, 0.0_f32); 4];
    let mut floor_sum = 0_u16;
    for i in 0..4 {
        let exact = (top_p[i] / sum) * 255.0;
        let fl = (libm::floorf(exact) as u16).min(255) as u8;
        weights[i] = fl;
        floor_sum += u16::from(fl);
        rem[i] = (i, exact - f32::from(fl));
    }
    let deficit = (255_u16.saturating_sub(floor_sum)) as usize;
    rem.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    for &(slot, _) in rem.iter().take(deficit) {
        weights[slot] += 1;
    }
    (ids, weights)
}

fn pack_water_override(primary: BiomeId) -> ([u8; 4], [u8; 4]) {
    let mut ids = [
        primary as u8,
        BiomeId::Beach as u8,
        BiomeId::Rock as u8,
        BiomeId::Grassland as u8,
    ];
    if primary == BiomeId::Beach {
        ids[1] = BiomeId::Ocean as u8;
    }
    (ids, [255, 0, 0, 0])
}

/// Classifies continuous top-4 biomes, cirque glaciers, moraines, and Werner dunes across `input`.
pub fn classify_biomes(
    input: &BiomeInput<'_>,
    cfg: &BiomeConfig,
) -> Result<BiomeField, BiomeError> {
    let w = input.width;
    let h = input.height;
    if w < 8 || h < 8 {
        return Err(BiomeError {
            message: "grid dimensions must be >= 8x8".to_owned(),
        });
    }
    let n = w * h;
    if input.total_h_m.len() != n
        || input.temp_c.len() != n
        || input.precip_m_yr.len() != n
        || input.moisture.len() != n
        || input.twi.len() != n
        || input.ela_local_m.len() != n
        || input.alluvial_mask.len() != n
        || input.talus_frac.len() != n
        || input.sediment_s_m.len() != n
        || input.water_kind.len() != n
        || input.river_w_m.len() != n
        || input.lithology_id.len() != n
    {
        return Err(BiomeError {
            message: "input slice lengths do not match grid dimensions".to_owned(),
        });
    }
    if !cfg.dx_m.is_finite() || cfg.dx_m <= 0.0 {
        return Err(BiomeError {
            message: "dx_m must be finite and > 0".to_owned(),
        });
    }

    let coast_dist_u16 =
        compute_coast_distance_u16(input.total_h_m, input.water_kind, w, h, cfg.sea_level_m);
    let coast_scale = cfg.dx_m / 3.0;
    let glacier_hard_floor_m = cfg.ela_base_m - cfg.glacier_floor_offset_m;

    let wind_len = libm::hypotf(cfg.wind_dir.0, cfg.wind_dir.1).max(1e-5);
    let w_ux = cfg.wind_dir.0 / wind_len;
    let w_uz = cfg.wind_dir.1 / wind_len;

    let mut biome_ids = vec![[0_u8; 4]; n];
    let mut biome_weights = vec![[0_u8; 4]; n];
    let mut dominant_biome = vec![0_u8; n];
    let mut glacier_h_m = vec![0.0_f32; n];
    let mut moraine_debris = vec![0_u8; n];
    let mut dune_h_m = vec![0.0_f32; n];

    let inv_dx2 = 1.0 / (cfg.dx_m * cfg.dx_m);

    biome_ids
        .par_chunks_mut(w)
        .zip(biome_weights.par_chunks_mut(w))
        .zip(dominant_biome.par_chunks_mut(w))
        .zip(glacier_h_m.par_chunks_mut(w))
        .zip(moraine_debris.par_chunks_mut(w))
        .zip(dune_h_m.par_chunks_mut(w))
        .enumerate()
        .for_each(
            |(r, (((((ids_row, w_row), dom_row), glac_row), mor_row), dune_row))| {
                let r_lo = r.saturating_sub(1);
                let r_hi = (r + 1).min(h - 1);
                let dz_span = ((r_hi - r_lo).max(1) as f32) * cfg.dx_m;
                let row_base = r * w;

                for c in 0..w {
                    let c_lo = c.saturating_sub(1);
                    let c_hi = (c + 1).min(w - 1);
                    let dx_span = ((c_hi - c_lo).max(1) as f32) * cfg.dx_m;
                    let idx = row_base + c;

                    let h_m = input.total_h_m[idx];
                    let t_c = input.temp_c[idx];
                    let p_m = input.precip_m_yr[idx];
                    let moist = input.moisture[idx];
                    let twi_v = input.twi[idx];
                    let ela = input.ela_local_m[idx];
                    let alluv = input.alluvial_mask[idx].clamp(0.0, 1.0);
                    let talus = input.talus_frac[idx].clamp(0.0, 1.0);
                    let sed = input.sediment_s_m[idx].max(0.0);
                    let wk = input.water_kind[idx];
                    let lith = input.lithology_id[idx];
                    let coast_d_m = f32::from(coast_dist_u16[idx]) * coast_scale;

                    // Row-sliding finite-difference slope (deg) & Laplacian curvature (0 MiB N^2).
                    let h_l = input.total_h_m[row_base + c_lo];
                    let h_r = input.total_h_m[row_base + c_hi];
                    let h_u = input.total_h_m[r_lo * w + c];
                    let h_d = input.total_h_m[r_hi * w + c];
                    let hx = (h_r - h_l) / dx_span;
                    let hz = (h_d - h_u) / dz_span;
                    let slope_tan = libm::hypotf(hx, hz);
                    let slope_deg = libm::atanf(slope_tan) * (180.0 / std::f32::consts::PI);
                    let lap = (h_l + h_r + h_u + h_d - 4.0 * h_m) * inv_dx2;

                    // Cirque glacier & moraine debris (strictly >= ela, >= ela_base-350, slope <= 42 deg).
                    if wk == WaterKind::None as u8
                        && h_m >= ela
                        && h_m >= glacier_hard_floor_m
                        && slope_deg <= cfg.glacier_max_slope_deg
                    {
                        let dz_ela = (h_m - ela).max(0.0);
                        let s_fac = smoothstep_c2(cfg.glacier_max_slope_deg, 10.0, slope_deg);
                        // Concave cirques (`lap > 0`) accumulate thicker ice than convex ridges (`lap < 0`).
                        let c_fac = (1.0 + (lap * 16.0).clamp(-0.35, 0.85)).max(0.3);
                        let p_fac = (p_m / 2.0).clamp(0.6, 1.4);
                        let ice_h = (0.095 * dz_ela * s_fac * c_fac * p_fac).clamp(0.0, 95.0);
                        glac_row[c] = ice_h;
                        if ice_h > 0.0 && ice_h < 18.0 {
                            let margin = smoothstep_c2(18.0, 2.0, ice_h);
                            let debris = (margin * (0.55 + 0.45 * talus) * 255.0)
                                .round()
                                .clamp(0.0, 255.0);
                            mor_row[c] = debris as u8;
                        }
                    } else if h_m >= glacier_hard_floor_m - 220.0
                        && h_m < ela
                        && talus > 0.25
                        && wk == WaterKind::None as u8
                    {
                        // Proglacial lateral/terminal moraine debris apron right below ELA.
                        let apron = smoothstep_c2(glacier_hard_floor_m - 220.0, ela, h_m) * talus;
                        mor_row[c] = (apron * 190.0).round().clamp(0.0, 255.0) as u8;
                    }

                    // Leeward Werner (1995) aeolian dunes (`~12 deg` stoss / `32-34 deg` slip face).
                    let x_m = (c as f32 + 0.5) * cfg.dx_m;
                    let z_m = (r as f32 + 0.5) * cfg.dx_m;
                    if wk == WaterKind::None as u8
                        && p_m < 0.48
                        && moist < 0.28
                        && (10.0..=950.0).contains(&h_m)
                        && slope_deg < 16.0
                        && lith != 1
                    {
                        let m_p = smoothstep_c2(0.48, 0.28, p_m);
                        let m_m = smoothstep_c2(0.28, 0.14, moist);
                        let m_s = smoothstep_c2(16.0, 7.0, slope_deg);
                        let m_h = smoothstep_c2(10.0, 45.0, h_m) * smoothstep_c2(950.0, 600.0, h_m);
                        let m_dune = m_p * m_m * m_s * m_h;
                        if m_dune > 0.01 {
                            let lambda = 100.0_f32;
                            let h_peak = 16.0_f32;
                            let phi_crest = 0.75_f32; // stoss: 16/75 -> 12.0 deg; slip: 16/25 -> 32.6 deg
                            let warp_s = 0.22
                                * quintic_noise_2d(cfg.seed ^ 0xD04E, x_m / 240.0, z_m / 240.0);
                            let s_coord = (x_m * w_ux + z_m * w_uz) / lambda + warp_s;
                            let phi = s_coord - libm::floorf(s_coord);
                            let h_raw = if phi < phi_crest {
                                phi / phi_crest
                            } else {
                                (1.0 - phi) / (1.0 - phi_crest)
                            };
                            dune_row[c] = m_dune * h_peak * h_raw.clamp(0.0, 1.0);
                        }
                    }

                    // Physical water-body overrides.
                    if wk == WaterKind::Ocean as u8
                        || (wk == WaterKind::DeltaTurbid as u8 && h_m <= cfg.sea_level_m + 0.1)
                    {
                        let (ids, wts) = pack_water_override(BiomeId::Ocean);
                        ids_row[c] = ids;
                        w_row[c] = wts;
                        dom_row[c] = ids[0];
                        continue;
                    }
                    if wk == WaterKind::FreshwaterLake as u8 || wk == WaterKind::EndorheicLake as u8
                    {
                        let (ids, wts) = pack_water_override(BiomeId::Lake);
                        ids_row[c] = ids;
                        w_row[c] = wts;
                        dom_row[c] = ids[0];
                        continue;
                    }

                    let river_amt = if (wk == WaterKind::River as u8
                        || wk == WaterKind::Waterfall as u8)
                        && h_m < ela
                    {
                        (input.river_w_m[idx] / cfg.dx_m).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    if river_amt >= 0.85 {
                        let (ids, wts) = pack_water_override(BiomeId::River);
                        ids_row[c] = ids;
                        w_row[c] = wts;
                        dom_row[c] = ids[0];
                        continue;
                    }

                    // Micro-ecotone domain warp (40-150 m) applied ONLY to sub-nival terrestrial
                    // environmental predictors; hard physical guards use unwarped values.
                    let warp_scale = (cfg.ecotone_warp_m / 180.0).clamp(0.22, 0.85);
                    let n1 = quintic_noise_2d(cfg.seed ^ 0xEC01, x_m / 240.0, z_m / 240.0);
                    let n2 = quintic_noise_2d(cfg.seed ^ 0xEC02, x_m / 110.0, z_m / 110.0);
                    let n3 = quintic_noise_2d(cfg.seed ^ 0xEC03, x_m / 180.0, z_m / 180.0);
                    let h_w = h_m + warp_scale * (75.0 * n1 + 35.0 * n2);
                    let t_w = t_c - 0.0063 * (warp_scale * (75.0 * n1 + 35.0 * n2)) + 0.7 * n3;
                    let moist_w = (moist + warp_scale * 0.09 * n3).clamp(0.0, 1.0);
                    let p_w = (p_m + warp_scale * 0.14 * n1).max(0.15);

                    let mut ll = [f32::NEG_INFINITY; BIOME_COUNT];

                    if h_m >= ela {
                        // Nival zone (Snow + Glacier + Rock). Strictly zero Snow/Glacier below ELA.
                        let rock_slope = smoothstep_c2(34.0, 52.0, slope_deg);
                        ll[BiomeId::Rock as usize] = -2.0 + 5.2 * rock_slope;
                        let snow_slope = smoothstep_c2(58.0, 34.0, slope_deg);
                        ll[BiomeId::Snow as usize] =
                            2.45 + gauss_ll(h_m, ela + 150.0, 360.0) + gate_ll(snow_slope);

                        if h_m >= glacier_hard_floor_m && slope_deg <= cfg.glacier_max_slope_deg {
                            let cirque = smoothstep_c2(cfg.glacier_max_slope_deg, 16.0, slope_deg);
                            let ice_boost = smoothstep_c2(0.5, 6.5, glac_row[c]);
                            ll[BiomeId::Glacier as usize] = 1.85
                                + 2.4 * ice_boost
                                + gate_ll(cirque)
                                + gauss_ll(h_m, ela + 320.0, 460.0);
                        }
                    } else {
                        // Sub-nival terrestrial + coastal strip (14 zoned biomes + Beach).
                        // Chaparral (16) and Steppe (17) remain strictly -INFINITY.

                        // Endorheic salt-flat rim check (8-neighbors).
                        let mut near_endo = false;
                        for dr in -1_isize..=1 {
                            for dc in -1_isize..=1 {
                                let nr = (r as isize + dr).clamp(0, h as isize - 1) as usize;
                                let nc = (c as isize + dc).clamp(0, w as isize - 1) as usize;
                                if input.water_kind[nr * w + nc] == WaterKind::EndorheicLake as u8 {
                                    near_endo = true;
                                }
                            }
                        }

                        // 1. Coastal Beach (<= 80 m from sea, gentle slope <= 20 deg) or salt flat.
                        if (coast_d_m <= 80.0 && (0.0..=28.0).contains(&h_m) && slope_deg <= 20.0)
                            || (near_endo && slope_deg <= 18.0)
                        {
                            let d_gate = if near_endo {
                                1.0
                            } else {
                                smoothstep_c2(80.0, 10.0, coast_d_m)
                                    * smoothstep_c2(28.0, 3.0, h_m)
                                    * smoothstep_c2(20.0, 6.0, slope_deg)
                            };
                            ll[BiomeId::Beach as usize] = 3.2 + gate_ll(d_gate);
                        }

                        // 2. Mangrove (warm, wet, calm coast/delta <= 550 m, h <= 22 m, slope <= 16 deg).
                        if coast_d_m <= 550.0 && h_m <= 22.0 && slope_deg <= 16.0 && moist >= 0.40 {
                            let mg = smoothstep_c2(550.0, 80.0, coast_d_m)
                                * smoothstep_c2(22.0, 2.0, h_m)
                                * smoothstep_c2(16.0, 3.0, slope_deg)
                                * smoothstep_c2(0.40, 0.62, moist)
                                * smoothstep_c2(20.0, 24.5, t_c);
                            ll[BiomeId::Mangrove as usize] = 4.6 + 1.2 * alluv + gate_ll(mg);
                        }

                        // 3. Swamp (lowland wetland/floodplain: TWI / alluvial / moist > 0.60, slope < 14 deg).
                        if h_w <= 420.0 && slope_deg <= 14.0 {
                            let wet_g = smoothstep_c2(0.58, 0.76, moist_w)
                                * smoothstep_c2(14.0, 4.0, slope_deg)
                                * smoothstep_c2(420.0, 80.0, h_w);
                            let twi_boost = smoothstep_c2(6.5, 10.0, twi_v);
                            ll[BiomeId::Swamp as usize] =
                                2.4 + 1.4 * twi_boost + 1.0 * alluv + gate_ll(wet_g);
                        }

                        // 4. Jungle (humid windward lowland/foothill rainforest).
                        let j_gate = smoothstep_c2(1_150.0, 600.0, h_w)
                            * smoothstep_c2(0.52, 0.68, moist_w)
                            * smoothstep_c2(38.0, 20.0, slope_deg);
                        ll[BiomeId::Jungle as usize] = 2.65
                            + gauss_ll(t_w, 25.0, 4.5)
                            + gauss_ll(p_w, 2.8, 0.9)
                            + gauss_ll(moist_w, 0.76, 0.14)
                            + gate_ll(j_gate);

                        // 5. Grassland (warm/mesic valley & alluvial plains).
                        let gr_gate = smoothstep_c2(1_450.0, 750.0, h_w)
                            * smoothstep_c2(24.0, 8.0, slope_deg);
                        ll[BiomeId::Grassland as usize] = 2.25
                            + 0.9 * alluv
                            + gauss_ll(moist_w, 0.54, 0.13)
                            + gauss_ll(t_w, 23.0, 5.0)
                            + gate_ll(gr_gate);

                        // 6. Desert (driest leeward sandy pocket & thorn scrub).
                        let d_gate = smoothstep_c2(0.66, 0.28, p_w)
                            * smoothstep_c2(0.44, 0.16, moist_w)
                            * smoothstep_c2(22.0, 8.0, slope_deg);
                        ll[BiomeId::Desert as usize] = 3.05
                            + gauss_ll(moist_w, 0.16, 0.11)
                            + gauss_ll(p_w, 0.22, 0.16)
                            + 0.6 * smoothstep_c2(0.0, 6.0, dune_row[c])
                            + gate_ll(d_gate);

                        // 7. Badlands (eroded semi-arid slopes & caprock canyon walls).
                        let bl_gate = smoothstep_c2(0.75, 0.40, p_w)
                            * smoothstep_c2(0.40, 0.24, moist_w)
                            * smoothstep_c2(8.0, 22.0, slope_deg)
                            * smoothstep_c2(3_200.0, 2_200.0, h_w);
                        ll[BiomeId::Badlands as usize] =
                            2.7 + 0.8 * talus + gauss_ll(slope_deg, 25.0, 10.0) + gate_ll(bl_gate);

                        // 8. Savanna (warm leeward sub-humid lowlands).
                        let sv_gate = smoothstep_c2(1_250.0, 750.0, h_w)
                            * smoothstep_c2(24.0, 10.0, slope_deg);
                        ll[BiomeId::Savanna as usize] = 2.25
                            + gauss_ll(moist_w, 0.34, 0.12)
                            + gauss_ll(p_w, 0.72, 0.32)
                            + gauss_ll(t_w, 24.5, 4.5)
                            + gate_ll(sv_gate);

                        // 9. Shrubland (transitional dry-to-montane slopes).
                        let sh_gate = smoothstep_c2(2_100.0, 1_300.0, h_w);
                        ll[BiomeId::Shrubland as usize] = 2.0
                            + gauss_ll(moist_w, 0.40, 0.11)
                            + gauss_ll(h_w, 950.0, 500.0)
                            + gauss_ll(slope_deg, 16.0, 11.0)
                            + gate_ll(sh_gate);

                        // 10. Forest (montane cloud forest & mid-altitude forest).
                        let f_gate = smoothstep_c2(480.0, 1_000.0, h_w)
                            * smoothstep_c2(2_850.0, 2_350.0, h_w)
                            * smoothstep_c2(0.28, 0.45, moist_w)
                            * smoothstep_c2(44.0, 28.0, slope_deg);
                        ll[BiomeId::Forest as usize] = 3.25
                            + gauss_ll(h_w, 1_700.0, 550.0)
                            + gauss_ll(t_w, 16.5, 5.0)
                            + gauss_ll(moist_w, 0.58, 0.24)
                            + gate_ll(f_gate);

                        // 11. Taiga (upper-montane / sub-páramo elfin & conifer proxy).
                        let tg_gate = smoothstep_c2(2_050.0, 2_450.0, h_w)
                            * smoothstep_c2(3_650.0, 3_200.0, h_w)
                            * smoothstep_c2(0.24, 0.40, moist_w)
                            * smoothstep_c2(44.0, 26.0, slope_deg);
                        ll[BiomeId::Taiga as usize] = 3.45
                            + gauss_ll(h_w, 2_850.0, 420.0)
                            + gauss_ll(t_w, 9.5, 4.2)
                            + gauss_ll(moist_w, 0.54, 0.26)
                            + gate_ll(tg_gate);

                        // 12. Alpine (lower páramo meadow 3,200-4,200 m).
                        let al_gate = smoothstep_c2(3_050.0, 3_400.0, h_w)
                            * smoothstep_c2(4_350.0, 3_950.0, h_w)
                            * smoothstep_c2(38.0, 22.0, slope_deg);
                        ll[BiomeId::Alpine as usize] = 2.8
                            + gauss_ll(h_w, 3_680.0, 340.0)
                            + gauss_ll(t_w, 4.2, 3.0)
                            + gate_ll(al_gate);

                        // 13. Bog (high-páramo cushion peatlands & gentle wet hollows > 2,800 m).
                        let bg_gate = smoothstep_c2(2_750.0, 3_050.0, h_w)
                            * smoothstep_c2(4_450.0, 4_000.0, h_w)
                            * smoothstep_c2(26.0, 16.0, slope_deg)
                            * smoothstep_c2(0.34, 0.48, moist_w);
                        ll[BiomeId::Bog as usize] = 3.65
                            + 1.2 * smoothstep_c2(4.5, 7.5, twi_v)
                            + 0.6 * smoothstep_c2(0.42, 0.62, moist_w)
                            + gate_ll(bg_gate);

                        // 14. Tundra (superpáramo cushion grassland 3,850 m .. ELA).
                        let tu_gate = smoothstep_c2(3_750.0, 4_080.0, h_w)
                            * smoothstep_c2(38.0, 22.0, slope_deg);
                        ll[BiomeId::Tundra as usize] = 2.85
                            + gauss_ll(h_w, 4_350.0, 360.0)
                            + gauss_ll(t_w, 0.0, 3.0)
                            + gate_ll(tu_gate);

                        // 15. Rock (steep cliffs >= 34 deg, talus aprons, exposed sub-nival scree).
                        let cliff = smoothstep_c2(33.0, 44.0, slope_deg);
                        let scree = smoothstep_c2(4_250.0, 4_700.0, h_m)
                            * smoothstep_c2(14.0, 28.0, slope_deg);
                        let high_talus = smoothstep_c2(0.50, 0.85, talus);
                        let bare_rock = cliff.max(scree).max(high_talus * 0.85);
                        ll[BiomeId::Rock as usize] =
                            -0.8 + 4.4 * bare_rock + 0.4 * (1.0 - sed.min(1.0));
                    }

                    // Softmax over log-likelihoods -> exact [f32; BIOME_COUNT] probabilities.
                    let mut max_ll = f32::NEG_INFINITY;
                    for (b_i, &v) in ll.iter().enumerate() {
                        if b_i == BiomeId::Chaparral as usize || b_i == BiomeId::Steppe as usize {
                            continue;
                        }
                        if v > max_ll {
                            max_ll = v;
                        }
                    }

                    let mut probs = [0.0_f32; BIOME_COUNT];
                    if max_ll.is_finite() {
                        let mut p_sum = 0.0_f32;
                        for b_i in 0..BIOME_COUNT {
                            if b_i == BiomeId::Chaparral as usize
                                || b_i == BiomeId::Steppe as usize
                                || !ll[b_i].is_finite()
                            {
                                continue;
                            }
                            let p = libm::expf(cfg.softmax_sharpness * (ll[b_i] - max_ll));
                            probs[b_i] = p;
                            p_sum += p;
                        }
                        if p_sum > 0.0 {
                            for p in &mut probs {
                                *p /= p_sum;
                            }
                        }
                    } else {
                        probs[BiomeId::Rock as usize] = 1.0;
                    }

                    // Blend intermediate river cells (`0.15 <= river_amt < 0.85`) with River (13).
                    if river_amt >= 0.15 {
                        let r_id = BiomeId::River as usize;
                        let blend = river_amt.clamp(0.0, 1.0);
                        for (b_i, p) in probs.iter_mut().enumerate() {
                            if b_i == r_id {
                                *p = blend;
                            } else {
                                *p *= 1.0 - blend;
                            }
                        }
                    }

                    let (ids, weights) = pack_top4_weights(&probs);
                    ids_row[c] = ids;
                    w_row[c] = weights;
                    dom_row[c] = ids[0];
                }
            },
        );

    Ok(BiomeField {
        width: w,
        height: h,
        biome_ids,
        biome_weights,
        dominant_biome,
        glacier_h_m,
        moraine_debris,
        dune_h_m,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top4_packing_always_sums_to_255_and_excludes_chaparral_steppe() {
        let mut probs = [0.0_f32; BIOME_COUNT];
        probs[BiomeId::Forest as usize] = 0.25;
        probs[BiomeId::Taiga as usize] = 0.25;
        probs[BiomeId::Alpine as usize] = 0.25;
        probs[BiomeId::Grassland as usize] = 0.25;
        let (ids, wts) = pack_top4_weights(&probs);
        let sum: u16 = wts.iter().map(|&w| u16::from(w)).sum();
        assert_eq!(sum, 255);
        assert!(!ids.contains(&(BiomeId::Chaparral as u8)));
        assert!(!ids.contains(&(BiomeId::Steppe as u8)));

        // Even if Chaparral/Steppe had nonzero scores passed in, packer excludes them.
        let mut bad = [0.0_f32; BIOME_COUNT];
        bad[BiomeId::Chaparral as usize] = 0.5;
        bad[BiomeId::Steppe as usize] = 0.4;
        bad[BiomeId::Desert as usize] = 0.1;
        let (ids2, wts2) = pack_top4_weights(&bad);
        assert_eq!(wts2.iter().map(|&w| u16::from(w)).sum::<u16>(), 255);
        assert!(!ids2.contains(&(BiomeId::Chaparral as u8)));
        assert!(!ids2.contains(&(BiomeId::Steppe as u8)));

        // Zero-sum fallback still yields exact 255 and valid non-absent IDs.
        let zero = [0.0_f32; BIOME_COUNT];
        let (ids3, wts3) = pack_top4_weights(&zero);
        assert_eq!(wts3.iter().map(|&w| u16::from(w)).sum::<u16>(), 255);
        assert!(!ids3.contains(&(BiomeId::Chaparral as u8)));
        assert!(!ids3.contains(&(BiomeId::Steppe as u8)));
    }

    #[test]
    fn glacier_strictly_obeys_ela_floor_and_42_deg_slope_limit() {
        let n = 64;
        let dx_m = 100.0_f32;
        let mut elev = vec![0.0_f32; n * n];
        let mut temp = vec![0.0_f32; n * n];
        let precip = vec![2.8_f32; n * n];
        let moist = vec![0.75_f32; n * n];
        let twi = vec![6.5_f32; n * n];
        let ela = vec![4_900.0_f32; n * n];
        let alluv = vec![0.0_f32; n * n];
        let talus = vec![0.35_f32; n * n];
        let sed = vec![0.5_f32; n * n];
        let wk = vec![0_u8; n * n];
        let rw = vec![0.0_f32; n * n];
        let lith = vec![0_u8; n * n];

        // Gentle ramp 3,000..5,600 m (~22 deg <= 42 deg); rows 48..64 above ELA have steep cliff (> 55 deg).
        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                let h = if r >= 48 && c >= 40 {
                    // Steep >55 deg cliff face above 5,000 m (`dh/dx = 1.6 -> ~58 deg`).
                    5_000.0 + ((c - 40) as f32) * 160.0
                } else {
                    3_000.0 + (c as f32 / (n - 1) as f32) * 2_600.0
                };
                elev[idx] = h;
                temp[idx] = 27.5 - 6.3 * (h * 0.001);
            }
        }

        let input = BiomeInput {
            width: n,
            height: n,
            total_h_m: &elev,
            temp_c: &temp,
            precip_m_yr: &precip,
            moisture: &moist,
            twi: &twi,
            ela_local_m: &ela,
            alluvial_mask: &alluv,
            talus_frac: &talus,
            sediment_s_m: &sed,
            water_kind: &wk,
            river_w_m: &rw,
            lithology_id: &lith,
        };
        let cfg = BiomeConfig {
            dx_m,
            ..BiomeConfig::default()
        };
        let field = classify_biomes(&input, &cfg).expect("glaciers");

        let mut glacier_cells_high_gentle = 0usize;
        for r in 1..(n - 1) {
            for c in 1..(n - 1) {
                let idx = r * n + c;
                let h = elev[idx];
                let hx = (elev[r * n + (c + 1)] - elev[r * n + (c - 1)]) / (2.0 * dx_m);
                let hz = (elev[(r + 1) * n + c] - elev[(r - 1) * n + c]) / (2.0 * dx_m);
                let slope_deg = libm::atanf(libm::hypotf(hx, hz)) * (180.0 / std::f32::consts::PI);
                let dom = field.dominant_biome[idx];

                if h < 4_550.0 {
                    assert_ne!(dom, BiomeId::Glacier as u8, "no Glacier below 4,550 m");
                    assert_eq!(field.glacier_h_m[idx], 0.0);
                }
                if h < ela[idx] {
                    assert_ne!(dom, BiomeId::Snow as u8, "no Snow below ELA");
                    assert_ne!(dom, BiomeId::Glacier as u8, "no Glacier below ELA");
                }
                if slope_deg > 42.0 {
                    assert_ne!(dom, BiomeId::Glacier as u8, "no Glacier on slope > 42 deg");
                    assert_eq!(field.glacier_h_m[idx], 0.0);
                }
                if h > 5_000.0 && slope_deg <= 30.0 && field.glacier_h_m[idx] > 0.0 {
                    glacier_cells_high_gentle += 1;
                }
            }
        }
        assert!(
            glacier_cells_high_gentle > 0,
            "expected cirque glaciers on gentle >5,000 m peaks"
        );
        assert!(field.moraine_debris.iter().any(|&m| m > 0));
    }

    #[test]
    fn werner_dunes_have_asymmetric_stoss_and_slip_angles() {
        let n = 64;
        let dx_m = 10.0_f32;
        let elev = vec![250.0_f32; n * n];
        let temp = vec![26.0_f32; n * n];
        let precip = vec![0.20_f32; n * n];
        let moist = vec![0.12_f32; n * n];
        let twi = vec![4.0_f32; n * n];
        let ela = vec![4_900.0_f32; n * n];
        let alluv = vec![0.0_f32; n * n];
        let talus = vec![0.0_f32; n * n];
        let sed = vec![2.0_f32; n * n];
        let wk = vec![0_u8; n * n];
        let rw = vec![0.0_f32; n * n];
        let lith = vec![5_u8; n * n];

        let input = BiomeInput {
            width: n,
            height: n,
            total_h_m: &elev,
            temp_c: &temp,
            precip_m_yr: &precip,
            moisture: &moist,
            twi: &twi,
            ela_local_m: &ela,
            alluvial_mask: &alluv,
            talus_frac: &talus,
            sediment_s_m: &sed,
            water_kind: &wk,
            river_w_m: &rw,
            lithology_id: &lith,
        };
        let cfg = BiomeConfig {
            dx_m,
            ..BiomeConfig::default()
        };
        let field = classify_biomes(&input, &cfg).expect("dunes");

        let mut stoss_slopes = Vec::new();
        let mut slip_slopes = Vec::new();
        let r = n / 2;
        for c in 2..(n - 2) {
            let dh = field.dune_h_m[r * n + c + 1] - field.dune_h_m[r * n + c];
            let deg = libm::atanf(dh.abs() / dx_m) * (180.0 / std::f32::consts::PI);
            if dh > 0.5 {
                stoss_slopes.push(deg);
            } else if dh < -1.5 {
                slip_slopes.push(deg);
            }
        }
        assert!(!stoss_slopes.is_empty() && !slip_slopes.is_empty());
        let mean_stoss = stoss_slopes.iter().sum::<f32>() / stoss_slopes.len() as f32;
        let max_slip = slip_slopes.iter().copied().fold(0.0_f32, f32::max);
        assert!(
            (8.0..=16.0).contains(&mean_stoss),
            "expected ~12 deg Werner stoss, got {mean_stoss}"
        );
        assert!(
            max_slip >= 24.0 && max_slip > mean_stoss * 1.8,
            "expected steeper leeward slip face (~32 deg), got max_slip={max_slip}, stoss={mean_stoss}"
        );
    }

    #[test]
    fn ecotone_blending_is_smooth_and_deterministic_across_threads() {
        let n = 96;
        let dx_m = 100.0_f32;
        let mut elev = vec![0.0_f32; n * n];
        let mut temp = vec![0.0_f32; n * n];
        let mut precip = vec![0.0_f32; n * n];
        let mut moist = vec![0.0_f32; n * n];
        let twi = vec![6.0_f32; n * n];
        let ela = vec![4_900.0_f32; n * n];
        let alluv = vec![0.2_f32; n * n];
        let talus = vec![0.1_f32; n * n];
        let sed = vec![1.0_f32; n * n];
        let wk = vec![0_u8; n * n];
        let rw = vec![0.0_f32; n * n];
        let lith = vec![4_u8; n * n];

        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                let u = c as f32 / (n - 1) as f32;
                let h = 200.0 + u * 4_400.0;
                elev[idx] = h;
                temp[idx] = 27.5 - 6.3 * (h * 0.001);
                precip[idx] = 0.4 + 2.6 * (1.0 - (r as f32 / (n - 1) as f32));
                moist[idx] = (precip[idx] / 3.5).clamp(0.15, 0.90);
            }
        }
        let input = BiomeInput {
            width: n,
            height: n,
            total_h_m: &elev,
            temp_c: &temp,
            precip_m_yr: &precip,
            moisture: &moist,
            twi: &twi,
            ela_local_m: &ela,
            alluvial_mask: &alluv,
            talus_frac: &talus,
            sediment_s_m: &sed,
            water_kind: &wk,
            river_w_m: &rw,
            lithology_id: &lith,
        };
        let cfg = BiomeConfig {
            dx_m,
            ..BiomeConfig::default()
        };
        let f1 = classify_biomes(&input, &cfg).expect("f1");
        let pool1 = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .expect("1-thread pool");
        let f_1t = pool1.install(|| classify_biomes(&input, &cfg).expect("f1t"));
        assert_eq!(f1.sha256_bytes(), f_1t.sha256_bytes());

        let ecotone_cells = f1
            .biome_weights
            .iter()
            .filter(|w| w[0] < 230) // w_0 / 255 < 0.90
            .count();
        let ecotone_frac = ecotone_cells as f32 / (n * n) as f32;
        assert!(
            ecotone_frac > 0.10,
            "expected continuous ecotone blending zone, got {ecotone_frac}"
        );
        for (ids, wts) in f1.biome_ids.iter().zip(f1.biome_weights.iter()) {
            assert_eq!(wts.iter().map(|&w| u16::from(w)).sum::<u16>(), 255);
            assert!(!ids.contains(&(BiomeId::Chaparral as u8)));
            assert!(!ids.contains(&(BiomeId::Steppe as u8)));
        }
    }

    #[test]
    fn realistic_512_tectonic_massif_satisfies_c1_all_16_biomes_and_c2_ecotone_band() {
        use crate::climate::{compute_climate, ClimateConfig};
        use crate::config::WorldV2Config;
        use crate::deposition::{apply_deposition, DepositionConfig, DepositionInput};
        use crate::hydrology::{priority_flood_d8, route_hybrid_mfd_d8, HydrologyConfig};
        use crate::lakes_falls::{
            generate_lakes_and_waterfalls, LakesFallsConfig, LakesFallsInput,
        };
        use crate::tectonic::generate_uplift;

        for &seed in &[0x0057_5632_2026_1120_u64, 42_u64, 1_u64, 0xdead_beef_u64] {
            let n = 512usize;
            let wcfg = WorldV2Config::for_size(seed, n as u32);
            let dx_m = wcfg.cell_size_m();
            let tect = generate_uplift(&wcfg).expect("s1");

            // S7 pass 1 + S5 hydrology + S4 deposition + S6 lakes + S7 pass 2 + S8 biomes.
            let clim_cfg = ClimateConfig::for_world(&wcfg);
            let clim1 = compute_climate(&tect.bedrock_h, n, n, None, &clim_cfg).expect("s7 p1");
            let pf = priority_flood_d8(&tect.bedrock_h, n, n, Some(0.0));
            let hcfg = HydrologyConfig {
                dx_m,
                ..HydrologyConfig::default()
            };
            let hydro = route_hybrid_mfd_d8(
                &pf.filled_h,
                &pf.d8_receiver,
                n,
                n,
                &clim1.precip_m_yr,
                &clim1.pet_m_yr,
                &hcfg,
            );
            let clim = compute_climate(&tect.bedrock_h, n, n, Some(&hydro.flow_acc_m2), &clim_cfg)
                .expect("s7 p2");

            let mut bedrock = tect.bedrock_h.clone();
            let mut sed = vec![0.0_f32; n * n];
            let mut water_h = hydro.water_h_m.clone();
            let mut water_kind = vec![0_u8; n * n];
            let s4_in = DepositionInput::from_hydrology(&hydro, Some(&pf.d8_receiver));
            let s4_cfg = DepositionConfig {
                seed,
                dx_m,
                ..DepositionConfig::default()
            };
            let s4 = apply_deposition(
                &mut bedrock,
                &mut sed,
                &mut water_h,
                &mut water_kind,
                &s4_in,
                &s4_cfg,
            )
            .expect("s4");

            let mut total_e: Vec<f32> = bedrock
                .iter()
                .zip(sed.iter())
                .map(|(&b, &s)| b + s)
                .collect();
            let s6_in = LakesFallsInput::from_hydrology(&hydro, &clim.precip_m_yr, &clim.pet_m_yr);
            let s6_cfg = LakesFallsConfig {
                dx_m,
                ..LakesFallsConfig::default()
            };
            let s6 = generate_lakes_and_waterfalls(
                &mut total_e,
                &mut sed,
                &mut water_h,
                &mut water_kind,
                &s6_in,
                &s6_cfg,
            )
            .expect("s6");
            assert_eq!(
                s6.endorheic_humid_fraction, 0.0,
                "seed={seed}: endorheic_humid_fraction must be strictly 0.0"
            );

            let talus = vec![0.0_f32; n * n];
            let b_in = BiomeInput {
                width: n,
                height: n,
                total_h_m: &total_e,
                temp_c: &clim.temp_c,
                precip_m_yr: &clim.precip_m_yr,
                moisture: &clim.moisture,
                twi: &clim.twi,
                ela_local_m: &clim.ela_m,
                alluvial_mask: &s4.alluvial_mask,
                talus_frac: &talus,
                sediment_s_m: &sed,
                water_kind: &water_kind,
                river_w_m: &hydro.river_w_m,
                lithology_id: &tect.lithology_id,
            };
            let b_cfg = BiomeConfig {
                dx_m,
                seed,
                ..BiomeConfig::default()
            };
            let bf = classify_biomes(&b_in, &b_cfg).expect("s8");

            let mut counts = [0_usize; BIOME_COUNT];
            let mut land_cells = 0_usize;
            let mut ecotone_land = 0_usize;
            for (i, &elev_i) in total_e.iter().enumerate() {
                let dom = bf.dominant_biome[i] as usize;
                counts[dom] += 1;
                let sum: u16 = bf.biome_weights[i].iter().map(|&w| u16::from(w)).sum();
                assert_eq!(sum, 255);
                assert!(!bf.biome_ids[i].contains(&(BiomeId::Chaparral as u8)));
                assert!(!bf.biome_ids[i].contains(&(BiomeId::Steppe as u8)));
                if elev_i > 0.0
                    && !BiomeId::from_u8(bf.dominant_biome[i])
                        .is_some_and(BiomeId::is_water_or_strip)
                {
                    land_cells += 1;
                    if bf.biome_weights[i][0] < 230 {
                        ecotone_land += 1;
                    }
                }
            }
            assert_eq!(counts[BiomeId::Chaparral as usize], 0);
            assert_eq!(counts[BiomeId::Steppe as usize], 0);

            let scaled_c1_floor = 40_usize; // ceil(2500 * (512 / 4096)^2)
            let ecotone_frac = ecotone_land as f32 / land_cells.max(1) as f32;
            for &b in &BiomeId::ZONED_TERRESTRIAL_16 {
                let c = counts[b as usize];
                assert!(
                    c >= scaled_c1_floor,
                    "seed={seed}: biome {b:?} ({}) has {c} < {scaled_c1_floor} dominant cells",
                    b as u8
                );
            }
            assert!(
                (0.15..=0.55).contains(&ecotone_frac),
                "seed={seed}: C2 ecotone_frac={ecotone_frac} outside [0.15, 0.55]"
            );

            // Verify non-cliff adjacent biome jumps > 0.70 stay < 3%.
            let mut adj_pairs = 0_usize;
            let mut jump_pairs = 0_usize;
            for r in 1..(n - 1) {
                for c in 1..(n - 1) {
                    let i = r * n + c;
                    if total_e[i] <= 0.0 || water_kind[i] != WaterKind::None as u8 {
                        continue;
                    }
                    let slope_i = libm::hypotf(
                        (total_e[i + 1] - total_e[i - 1]) / (2.0 * dx_m),
                        (total_e[(r + 1) * n + c] - total_e[(r - 1) * n + c]) / (2.0 * dx_m),
                    );
                    if slope_i > 0.70 {
                        // ~35 deg cliff
                        continue;
                    }
                    for &nb in &[i + 1, (r + 1) * n + c] {
                        if total_e[nb] <= 0.0 || water_kind[nb] != WaterKind::None as u8 {
                            continue;
                        }
                        adj_pairs += 1;
                        let mut p_a = [0.0_f32; BIOME_COUNT];
                        let mut p_b = [0.0_f32; BIOME_COUNT];
                        for k in 0..4 {
                            p_a[bf.biome_ids[i][k] as usize] +=
                                f32::from(bf.biome_weights[i][k]) / 255.0;
                            p_b[bf.biome_ids[nb][k] as usize] +=
                                f32::from(bf.biome_weights[nb][k]) / 255.0;
                        }
                        let tv = 0.5
                            * p_a
                                .iter()
                                .zip(p_b.iter())
                                .map(|(&a, &b)| (a - b).abs())
                                .sum::<f32>();
                        if tv > 0.70 {
                            jump_pairs += 1;
                        }
                    }
                }
            }
            let jump_frac = jump_pairs as f32 / adj_pairs.max(1) as f32;
            assert!(
                jump_frac < 0.03,
                "seed={seed}: non-cliff jump_frac={jump_frac} >= 0.03"
            );
        }
    }
}
