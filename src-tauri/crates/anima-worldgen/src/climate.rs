//! Stage S7: Regional tropical coastal-massif climate (`φ_0 = 11°N`), decoupled Equilibrium Line
//! Altitude (`ELA = 4,900 m ± 180 m aspect ± 120 m precip`), 2-D `512²` orographic wind/advection
//! precipitation (`Smith & Barstad 2004`; `Roe 2005`), `PET`, `TWI` (`Beven & Kirkby 1979`),
//! normalized `moisture`, and 2-pass climate–erosion coupling.

use crate::config::{WorldLayout, WorldV2Config};
use crate::rng::stage_rng;
use crate::SEA_LEVEL_M;
use rand::Rng;
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::fmt;

/// Error returned when [`ClimateConfig`] or input grid dimensions violate Stage S7 invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClimateError {
    pub message: String,
}

impl fmt::Display for ClimateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ClimateError {}

/// Configuration for Stage S7 regional climate & TWI calculation (`§2.2.7`, `LD-5`).
#[derive(Debug, Clone, PartialEq)]
pub struct ClimateConfig {
    pub seed: u64,
    pub schema_tag: String,
    /// Optional target-resolution grid size so coarse multigrid levels share the target RNG seed.
    pub params_grid_size: Option<u32>,
    /// Full-grid cell spacing in metres (`10.0` at `4096^2`).
    pub dx_m: f32,
    pub sea_level_m: f32,
    /// Fixed regional latitude in degrees North (`11.0°N`, zero N-S gradient).
    pub latitude_deg: f32,
    /// Sea-level baseline 2 m air temperature (`27.5 °C`).
    pub t0_sea_level_c: f32,
    /// SNSM environmental lapse rate (`6.3 °C/km`, `Hoyos et al. 2019`).
    pub lapse_rate_c_per_km: f32,
    /// Coastal maritime thermal buffering amplitude (`0.8 °C`).
    pub maritime_buffer_c: f32,
    /// E-folding coastal buffering decay distance in metres (`6,000 m`).
    pub maritime_decay_m: f32,
    /// Solar-aspect temperature modulation amplitude (`2.0 °C`).
    pub aspect_temp_amp_c: f32,
    /// Base Equilibrium Line Altitude in metres (`4,900 m ∈ [4,800, 5,100]`).
    pub ela_base_m: f32,
    /// Aspect modulation on `ELA` (`±180 m`).
    pub ela_aspect_amp_m: f32,
    /// Precipitation modulation on `ELA` (`±120 m`).
    pub ela_precip_amp_m: f32,
    /// Strict minimum annual precipitation floor (`0.15 m/yr`).
    pub min_precip_m_yr: f32,
    /// Target windward slope precipitation band midpoint (`3.2 m/yr`, band `2.5–3.8`).
    pub windward_precip_target_m_yr: f32,
    /// Coarse advection grid resolution (`min(N, 512)`).
    pub advect_grid_size: usize,
    /// Terrain-deflection gain for 2-D wind around massif peaks (`0.55`).
    pub wind_deflect_gain: f32,
    /// Clausius-Clapeyron moisture scale height (`3,200 m`).
    pub moisture_scale_height_m: f32,
    /// Leeward orographic drying multiplier (`1.0` default = unchanged trunk; `1.32` on `MultiEnvironment`).
    pub leeward_rain_shadow_gain: f32,
}

impl Default for ClimateConfig {
    fn default() -> Self {
        Self {
            seed: 0x0057_5632_2026_1120,
            schema_tag: "ANMW3-SNSM-v1".to_owned(),
            params_grid_size: None,
            dx_m: 10.0,
            sea_level_m: SEA_LEVEL_M,
            latitude_deg: 11.0,
            t0_sea_level_c: 27.5,
            lapse_rate_c_per_km: 6.3,
            maritime_buffer_c: 0.8,
            maritime_decay_m: 6_000.0,
            aspect_temp_amp_c: 2.0,
            ela_base_m: 4_900.0,
            ela_aspect_amp_m: 180.0,
            ela_precip_amp_m: 120.0,
            min_precip_m_yr: 0.15,
            windward_precip_target_m_yr: 3.2,
            advect_grid_size: 512,
            wind_deflect_gain: 0.55,
            moisture_scale_height_m: 3_200.0,
            leeward_rain_shadow_gain: 1.0,
        }
    }
}

impl ClimateConfig {
    /// Creates a [`ClimateConfig`] bound to a [`WorldV2Config`].
    #[must_use]
    pub fn for_world(cfg: &WorldV2Config) -> Self {
        Self::for_world_with_target(cfg, cfg.grid_size)
    }

    /// Creates a [`ClimateConfig`] bound to `cfg` while pinning RNG domain separation to `target_size`.
    #[must_use]
    pub fn for_world_with_target(cfg: &WorldV2Config, target_size: u32) -> Self {
        Self {
            seed: cfg.seed,
            schema_tag: cfg.schema_tag.clone(),
            params_grid_size: Some(target_size),
            dx_m: cfg.cell_size_m(),
            leeward_rain_shadow_gain: match cfg.layout {
                WorldLayout::SingleMassif => 1.0,
                WorldLayout::MultiEnvironment => 1.32,
            },
            ..Self::default()
        }
    }

    /// Validates physical invariants of the climate configuration.
    pub fn validate(&self) -> Result<(), ClimateError> {
        let err = |msg: &str| ClimateError {
            message: msg.to_owned(),
        };
        if let Some(sz) = self.params_grid_size {
            if sz == 0 {
                return Err(err("params_grid_size must be > 0"));
            }
        }
        if !self.dx_m.is_finite() || self.dx_m <= 0.0 {
            return Err(err("dx_m must be finite and > 0"));
        }
        if !self.sea_level_m.is_finite()
            || !self.latitude_deg.is_finite()
            || !self.t0_sea_level_c.is_finite()
            || !self.maritime_buffer_c.is_finite()
            || !self.aspect_temp_amp_c.is_finite()
            || !self.ela_aspect_amp_m.is_finite()
            || !self.ela_precip_amp_m.is_finite()
            || !self.wind_deflect_gain.is_finite()
        {
            return Err(err("climate configuration scalar fields must be finite"));
        }
        if !self.maritime_decay_m.is_finite() || self.maritime_decay_m <= 0.0 {
            return Err(err("maritime_decay_m must be finite and > 0"));
        }
        if !self.moisture_scale_height_m.is_finite() || self.moisture_scale_height_m <= 0.0 {
            return Err(err("moisture_scale_height_m must be finite and > 0"));
        }
        if !self.windward_precip_target_m_yr.is_finite() || self.windward_precip_target_m_yr <= 0.0
        {
            return Err(err("windward_precip_target_m_yr must be finite and > 0"));
        }
        if !self.ela_base_m.is_finite() || !(4_800.0..=5_100.0).contains(&self.ela_base_m) {
            return Err(err("ela_base_m must lie within [4800.0, 5100.0] m"));
        }
        if !self.min_precip_m_yr.is_finite() || self.min_precip_m_yr < 0.15 {
            return Err(err("min_precip_m_yr must be >= 0.15 m/yr"));
        }
        if self.advect_grid_size < 32 {
            return Err(err("advect_grid_size must be >= 32"));
        }
        if !self.lapse_rate_c_per_km.is_finite() || self.lapse_rate_c_per_km <= 0.0 {
            return Err(err("lapse_rate_c_per_km must be > 0"));
        }
        if !self.leeward_rain_shadow_gain.is_finite()
            || !(0.5..=3.0).contains(&self.leeward_rain_shadow_gain)
        {
            return Err(err("leeward_rain_shadow_gain must lie within [0.5, 3.0]"));
        }
        Ok(())
    }
}

/// Output climate buffers produced deterministically by [`compute_climate`].
#[derive(Debug, Clone, PartialEq)]
pub struct ClimateField {
    pub width: usize,
    pub height: usize,
    /// Mean annual 2 m air temperature in `°C` (`~27.5 °C` sea level, `-8.4 °C` @ `+5,700 m`).
    pub temp_c: Vec<f32>,
    /// Mean annual precipitation in `m/yr`, strictly `>= cfg.min_precip_m_yr`.
    pub precip_m_yr: Vec<f32>,
    /// Potential evapotranspiration in `m/yr`.
    pub pet_m_yr: Vec<f32>,
    /// Topographic Wetness Index `ln((A_spec + dx) / max(tan(beta), 0.002))`.
    pub twi: Vec<f32>,
    /// Normalized effective moisture in `[0.0, 1.0]`.
    pub moisture: Vec<f32>,
    /// Local Equilibrium Line Altitude in metres (`ela_base_m ± 180 m aspect ± 120 m precip`).
    pub ela_m: Vec<f32>,
    /// Sampled prevailing West-to-East onshore wind azimuth in radians (`[-0.26, +0.26]`).
    pub wind_azimuth_rad: f32,
}

impl ClimateField {
    /// Computes deterministic SHA-256 over dimensions and little-endian IEEE-754 bytes of all fields.
    #[must_use]
    pub fn sha256_bytes(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update((self.width as u32).to_le_bytes());
        hasher.update((self.height as u32).to_le_bytes());
        hasher.update(self.wind_azimuth_rad.to_bits().to_le_bytes());
        for slice in [
            &self.temp_c,
            &self.precip_m_yr,
            &self.pet_m_yr,
            &self.twi,
            &self.moisture,
            &self.ela_m,
        ] {
            for &v in slice {
                hasher.update(v.to_bits().to_le_bytes());
            }
        }
        let digest = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }

    /// Bilinearly resamples all climate layers onto `(target_w, target_h)` for 2-pass coupling.
    #[must_use]
    pub fn resample_bilinear(&self, target_w: usize, target_h: usize) -> Self {
        assert!(self.width >= 2 && self.height >= 2);
        assert!(target_w >= 2 && target_h >= 2);
        Self {
            width: target_w,
            height: target_h,
            temp_c: bilinear_resample(&self.temp_c, self.width, self.height, target_w, target_h),
            precip_m_yr: bilinear_resample(
                &self.precip_m_yr,
                self.width,
                self.height,
                target_w,
                target_h,
            ),
            pet_m_yr: bilinear_resample(
                &self.pet_m_yr,
                self.width,
                self.height,
                target_w,
                target_h,
            ),
            twi: bilinear_resample(&self.twi, self.width, self.height, target_w, target_h),
            moisture: bilinear_resample(
                &self.moisture,
                self.width,
                self.height,
                target_w,
                target_h,
            ),
            ela_m: bilinear_resample(&self.ela_m, self.width, self.height, target_w, target_h),
            wind_azimuth_rad: self.wind_azimuth_rad,
        }
    }
}

#[inline]
fn sample_bilinear_at(src: &[f32], sw: usize, sh: usize, u: f32, v: f32) -> f32 {
    let x = (u * (sw - 1) as f32).clamp(0.0, (sw - 1) as f32);
    let z = (v * (sh - 1) as f32).clamp(0.0, (sh - 1) as f32);
    let x0 = libm::floorf(x) as usize;
    let z0 = libm::floorf(z) as usize;
    let x1 = (x0 + 1).min(sw - 1);
    let z1 = (z0 + 1).min(sh - 1);
    let fx = x - x0 as f32;
    let fz = z - z0 as f32;
    let v00 = src[z0 * sw + x0];
    let v10 = src[z0 * sw + x1];
    let v01 = src[z1 * sw + x0];
    let v11 = src[z1 * sw + x1];
    let vx0 = v00 + fx * (v10 - v00);
    let vx1 = v01 + fx * (v11 - v01);
    vx0 + fz * (vx1 - vx0)
}

fn bilinear_resample(src: &[f32], sw: usize, sh: usize, tw: usize, th: usize) -> Vec<f32> {
    let mut dst = vec![0.0_f32; tw * th];
    let denom_x = (tw - 1) as f32;
    let denom_z = (th - 1) as f32;
    dst.par_chunks_mut(tw).enumerate().for_each(|(r, row)| {
        let v = r as f32 / denom_z;
        for (c, out) in row.iter_mut().enumerate() {
            let u = c as f32 / denom_x;
            *out = sample_bilinear_at(src, sw, sh, u, v);
        }
    });
    dst
}

fn box_blur_separable(buf: &mut [f32], w: usize, h: usize, radius: usize, passes: usize) {
    if radius == 0 || w < 3 || h < 3 {
        return;
    }
    let mut tmp = vec![0.0_f32; w * h];
    let rad = radius as isize;
    for _ in 0..passes {
        // Horizontal pass.
        tmp.par_chunks_mut(w).enumerate().for_each(|(r, out_row)| {
            let row = &buf[r * w..(r + 1) * w];
            for (c, out) in out_row.iter_mut().enumerate() {
                let mut acc = 0.0_f32;
                let mut cnt = 0_u32;
                for dc in -rad..=rad {
                    let nc = (c as isize + dc).clamp(0, w as isize - 1) as usize;
                    acc += row[nc];
                    cnt += 1;
                }
                *out = acc / cnt as f32;
            }
        });
        // Vertical pass.
        buf.par_chunks_mut(w).enumerate().for_each(|(r, out_row)| {
            for (c, out) in out_row.iter_mut().enumerate() {
                let mut acc = 0.0_f32;
                let mut cnt = 0_u32;
                for dr in -rad..=rad {
                    let nr = (r as isize + dr).clamp(0, h as isize - 1) as usize;
                    acc += tmp[nr * w + c];
                    cnt += 1;
                }
                *out = acc / cnt as f32;
            }
        });
    }
}

/// Computes `u16` coastal chamfer distance (in metres) on the coarse `mw x mh` grid.
fn compute_coast_distance_coarse(
    elev_c: &[f32],
    mw: usize,
    mh: usize,
    dx_c: f32,
    sea_level_m: f32,
) -> Vec<f32> {
    let n = mw * mh;
    // Chamfer (3, 4) units -> 1 orthogonal step = 3 units = dx_c metres.
    const INF_U16: u16 = u16::MAX;
    let mut dist_u16 = vec![INF_U16; n];
    for i in 0..n {
        if elev_c[i] <= sea_level_m {
            dist_u16[i] = 0;
        }
    }
    // Forward pass.
    for r in 0..mh {
        for c in 0..mw {
            let i = r * mw + c;
            let mut d = dist_u16[i];
            if d == 0 {
                continue;
            }
            if c > 0 {
                d = d.min(dist_u16[i - 1].saturating_add(3));
            }
            if r > 0 {
                d = d.min(dist_u16[i - mw].saturating_add(3));
                if c > 0 {
                    d = d.min(dist_u16[i - mw - 1].saturating_add(4));
                }
                if c + 1 < mw {
                    d = d.min(dist_u16[i - mw + 1].saturating_add(4));
                }
            }
            dist_u16[i] = d;
        }
    }
    // Backward pass.
    for r in (0..mh).rev() {
        for c in (0..mw).rev() {
            let i = r * mw + c;
            let mut d = dist_u16[i];
            if d == 0 {
                continue;
            }
            if c + 1 < mw {
                d = d.min(dist_u16[i + 1].saturating_add(3));
            }
            if r + 1 < mh {
                d = d.min(dist_u16[i + mw].saturating_add(3));
                if c + 1 < mw {
                    d = d.min(dist_u16[i + mw + 1].saturating_add(4));
                }
                if c > 0 {
                    d = d.min(dist_u16[i + mw - 1].saturating_add(4));
                }
            }
            dist_u16[i] = d;
        }
    }

    let scale = dx_c / 3.0;
    dist_u16.into_iter().map(|d| f32::from(d) * scale).collect()
}

/// Computes Stage S7 climate fields on `elev_m` (`width x height`).
///
/// The atmospheric orographic precipitation solver runs on a `min(width, cfg.advect_grid_size)^2`
/// coarse grid (`512^2` max) with West-to-East prevailing onshore wind (`u_x > 0`), matching
/// Stage S1's western ocean (`u < 0.44`) and eastern leeward plateau (`u ≈ 0.71..0.77`), then
/// bilinearly upsamples smoothly to `width x height`.
pub fn compute_climate(
    elev_m: &[f32],
    width: usize,
    height: usize,
    flow_acc_m2: Option<&[f64]>,
    cfg: &ClimateConfig,
) -> Result<ClimateField, ClimateError> {
    cfg.validate()?;
    if width < 8 || height < 8 {
        return Err(ClimateError {
            message: "grid dimensions must be >= 8x8".to_owned(),
        });
    }
    let n = width * height;
    if elev_m.len() != n {
        return Err(ClimateError {
            message: format!(
                "elev_m length {} does not match {}x{}",
                elev_m.len(),
                width,
                height
            ),
        });
    }
    if elev_m.iter().any(|h| !h.is_finite()) {
        return Err(ClimateError {
            message: "elev_m contains non-finite values".to_owned(),
        });
    }
    if let Some(acc) = flow_acc_m2 {
        if acc.len() != n {
            return Err(ClimateError {
                message: format!("flow_acc_m2 length {} does not match {}", acc.len(), n),
            });
        }
        if acc.iter().any(|&v| !v.is_finite() || v < 0.0) {
            return Err(ClimateError {
                message: "flow_acc_m2 contains non-finite or negative values".to_owned(),
            });
        }
    }

    let rng_w = cfg.params_grid_size.unwrap_or(width as u32);
    let rng_h = cfg.params_grid_size.unwrap_or(height as u32);
    let mut rng = stage_rng(cfg.seed, rng_w, rng_h, &cfg.schema_tag, "S7-CLIM");
    // Single regional onshore wind azimuth in [-14 deg, +14 deg] Eastward (u_x > 0.97).
    let wind_azimuth_rad: f32 = rng.gen_range(-0.24_f32..0.24_f32);
    let u0_x = libm::cosf(wind_azimuth_rad);
    let u0_z = libm::sinf(wind_azimuth_rad);

    // 1. Coarse M x M orographic precipitation & coastal distance solve.
    let mw = width.min(cfg.advect_grid_size);
    let mh = height.min(cfg.advect_grid_size);
    let dx_cx = (width as f32 / mw as f32) * cfg.dx_m;
    let dx_cz = (height as f32 / mh as f32) * cfg.dx_m;
    let dx_c = 0.5 * (dx_cx + dx_cz);

    let mut elev_c = vec![0.0_f32; mw * mh];
    for r in 0..mh {
        let r0 = (r * height) / mh;
        let r1 = (((r + 1) * height) / mh).max(r0 + 1).min(height);
        for c in 0..mw {
            let c0 = (c * width) / mw;
            let c1 = (((c + 1) * width) / mw).max(c0 + 1).min(width);
            let mut sum = 0.0_f32;
            let mut cnt = 0_u32;
            for sr in r0..r1 {
                let base = sr * width;
                for sc in c0..c1 {
                    sum += elev_m[base + sc];
                    cnt += 1;
                }
            }
            elev_c[r * mw + c] = sum / cnt as f32;
        }
    }

    let coast_c = compute_coast_distance_coarse(&elev_c, mw, mh, dx_c, cfg.sea_level_m);

    // Smooth coarse elevation (~2 km scale) for stable 2-D wind deflection & orographic slope.
    let mut elev_s = elev_c.clone();
    let blur_r = ((1_200.0 / dx_c).round() as usize).clamp(1, 6);
    box_blur_separable(&mut elev_s, mw, mh, blur_r, 2);

    let mut precip_c = vec![cfg.min_precip_m_yr; mw * mh];
    let mut q_prev_col = vec![1.0_f32; mh];
    let mut q_curr_col = vec![1.0_f32; mh];

    let step_scale = dx_c / 80.0;
    let oro_gain = (cfg.windward_precip_target_m_yr / 3.2).clamp(0.4, 2.0);
    let c_bg = 1.82 * oro_gain;
    let c_oro = 3.65 * oro_gain;
    let w_sat = 0.19_f32;
    let c_lee = 5.2_f32;
    let s0_sq = 0.25_f32 * 0.25_f32;

    // Column-by-column DAG sweep from West (c=0) to East (c=mw-1), since u_x > 0 strictly.
    for c in 0..mw {
        let c_lo = c.saturating_sub(1);
        let c_hi = (c + 1).min(mw - 1);
        let span_x = (c_hi - c_lo).max(1) as f32 * dx_cx;

        for r in 0..mh {
            let r_lo = r.saturating_sub(1);
            let r_hi = (r + 1).min(mh - 1);
            let span_z = (r_hi - r_lo).max(1) as f32 * dx_cz;

            let idx = r * mw + c;
            let hx = (elev_s[r * mw + c_hi] - elev_s[r * mw + c_lo]) / span_x;
            let hz = (elev_s[r_hi * mw + c] - elev_s[r_lo * mw + c]) / span_z;

            // Wind deflection around massif flanks.
            let dot_g = u0_x * hx + u0_z * hz;
            let g_sq = hx * hx + hz * hz;
            let deflect = cfg.wind_deflect_gain * dot_g / (g_sq + s0_sq);
            let ux_raw = (u0_x - deflect * hx).max(0.28 * u0_x);
            let uz_raw = u0_z - deflect * hz;
            let u_norm = libm::hypotf(ux_raw, uz_raw).max(1e-6);
            let ux = ux_raw / u_norm;
            let uz = uz_raw / u_norm;

            // Semi-Lagrangian upwind vapor advection from column c - 1.
            let q_in = if c == 0 {
                1.0_f32
            } else {
                let r_up = (r as f32 - uz / ux).clamp(0.0, (mh - 1) as f32);
                let r0 = libm::floorf(r_up) as usize;
                let r1 = (r0 + 1).min(mh - 1);
                let fr = r_up - r0 as f32;
                q_prev_col[r0] + fr * (q_prev_col[r1] - q_prev_col[r0])
            };

            let h = elev_c[idx];
            if h <= cfg.sea_level_m {
                let recharge = (0.22 * step_scale).min(0.85);
                q_curr_col[r] = (q_in + recharge * (1.0 - q_in)).clamp(0.10, 1.0);
                precip_c[idx] = 1.60;
            } else {
                let g_along = ux * hx + uz * hz;
                let w_oro = g_along.max(0.0);
                let w_down = (-g_along).max(0.0);
                let f_cc = libm::expf(-h.max(0.0) / cfg.moisture_scale_height_m);
                let oro_frac = w_oro / (w_oro + w_sat);
                let lee_damp = libm::expf(-c_lee * w_down);

                let cond = q_in * f_cc * (c_bg + c_oro * oro_frac) * lee_damp;
                precip_c[idx] = cond.max(cfg.min_precip_m_yr);

                // Rainout depletes column moisture during orographic condensation excess and
                // leeward adiabatic drying; flat humid lowlands gently recycle moisture.
                let oro_rain = (cond - q_in * f_cc * c_bg).max(0.0);
                let (rain_deplete, evap_recycle) =
                    if (cfg.leeward_rain_shadow_gain - 1.0).abs() < 1e-6 {
                        let rd = 0.0018 * step_scale * oro_rain + 0.005 * step_scale * w_down;
                        let er = if w_down < 0.02 && h < 1_200.0 {
                            0.0045 * step_scale * (0.96 - q_in).max(0.0)
                        } else {
                            0.0008 * step_scale * (0.50 - q_in).max(0.0)
                        };
                        (rd, er)
                    } else {
                        let rd = (0.0018 * step_scale * oro_rain + 0.005 * step_scale * w_down)
                            * cfg.leeward_rain_shadow_gain;
                        let t_dry = ((0.78 - q_in) / 0.30).clamp(0.0, 1.0);
                        let recycle_damp = 1.0 - 0.72 * t_dry * t_dry * (3.0 - 2.0 * t_dry);
                        let er = if w_down < 0.02 && h < 1_200.0 {
                            0.0045 * step_scale * recycle_damp * (0.96 - q_in).max(0.0)
                        } else {
                            0.0008 * step_scale * (0.50 - q_in).max(0.0)
                        };
                        (rd, er)
                    };
                q_curr_col[r] = (q_in - rain_deplete + evap_recycle).clamp(0.05, 1.0);
            }
        }
        std::mem::swap(&mut q_prev_col, &mut q_curr_col);
    }

    // Lateral eddy diffusion smoothing so 2-D precipitation has zero 1-D row streaks.
    let eddy_r = ((600.0 / dx_c).round() as usize).clamp(1, 3);
    box_blur_separable(&mut precip_c, mw, mh, eddy_r, 2);

    // 2. Compute full-resolution fields row-parallel (1-thread == N-thread bit-identical).
    let mut temp_c = vec![0.0_f32; n];
    let mut precip_m_yr = vec![cfg.min_precip_m_yr; n];
    let mut pet_m_yr = vec![0.0_f32; n];
    let mut twi = vec![0.0_f32; n];
    let mut moisture = vec![0.0_f32; n];
    let mut ela_m = vec![cfg.ela_base_m; n];

    // Fixed regional 11 deg N solar noon unit vector (zero latitudinal gradient across map).
    let lat_rad = cfg.latitude_deg * (std::f32::consts::PI / 180.0);
    let s_noon_y = libm::cosf(lat_rad);
    let s_noon_z = libm::sinf(lat_rad);

    let denom_x = (width - 1) as f32;
    let denom_z = (height - 1) as f32;

    temp_c
        .par_chunks_mut(width)
        .zip(precip_m_yr.par_chunks_mut(width))
        .zip(pet_m_yr.par_chunks_mut(width))
        .zip(twi.par_chunks_mut(width))
        .zip(moisture.par_chunks_mut(width))
        .zip(ela_m.par_chunks_mut(width))
        .enumerate()
        .for_each(
            |(r, (((((t_row, p_row), pet_row), twi_row), m_row), ela_row))| {
                let r_lo = r.saturating_sub(1);
                let r_hi = (r + 1).min(height - 1);
                let dz_span = if r_hi == r_lo {
                    cfg.dx_m
                } else {
                    (r_hi - r_lo) as f32 * cfg.dx_m
                };
                let v = r as f32 / denom_z;
                let row_base = r * width;

                for c in 0..width {
                    let c_lo = c.saturating_sub(1);
                    let c_hi = (c + 1).min(width - 1);
                    let dx_span = if c_hi == c_lo {
                        cfg.dx_m
                    } else {
                        (c_hi - c_lo) as f32 * cfg.dx_m
                    };
                    let u = c as f32 / denom_x;
                    let idx = row_base + c;
                    let h = elev_m[idx];

                    let hx = (elev_m[row_base + c_hi] - elev_m[row_base + c_lo]) / dx_span;
                    let hz = (elev_m[r_hi * width + c] - elev_m[r_lo * width + c]) / dz_span;
                    let slope_tan = libm::hypotf(hx, hz);
                    let n_len = libm::hypotf(slope_tan, 1.0);
                    let n_y = 1.0 / n_len;
                    let n_z = -hz / n_len;

                    // Solar aspect n . s_noon normalized relative to zenith projection.
                    let n_dot_s = n_y * s_noon_y + n_z * s_noon_z;
                    let aspect_raw = if s_noon_z.abs() > 1e-4 {
                        (n_dot_s - n_y * s_noon_y) / s_noon_z
                    } else {
                        0.0
                    };
                    let aspect_heat = (aspect_raw + 0.15 * (n_y - 1.0)).clamp(-1.0, 1.0);

                    let d_coast = sample_bilinear_at(&coast_c, mw, mh, u, v);
                    let p_up = sample_bilinear_at(&precip_c, mw, mh, u, v);
                    let p = p_up.max(cfg.min_precip_m_yr);
                    p_row[c] = p;

                    let t = if h > cfg.sea_level_m {
                        let mar = cfg.maritime_buffer_c
                            * libm::expf(-d_coast / cfg.maritime_decay_m)
                            * (1.0 - h / 1_200.0).clamp(0.0, 1.0);
                        cfg.t0_sea_level_c + mar - cfg.lapse_rate_c_per_km * (h * 0.001)
                            + cfg.aspect_temp_amp_c * aspect_heat
                    } else {
                        cfg.t0_sea_level_c - 0.5 * (h.abs() / 500.0).min(1.0)
                    };
                    t_row[c] = t;

                    // Decoupled Equilibrium Line Altitude: base 4,900 m +/- 180 m aspect +/- 120 m precip.
                    let p_norm = ((p - 2.0) / 1.5).clamp(-1.0, 1.0);
                    ela_row[c] = cfg.ela_base_m + cfg.ela_aspect_amp_m * aspect_heat
                        - cfg.ela_precip_amp_m * p_norm;

                    // Tropical lowland PET ~1.35 m/yr decreasing monotonically with altitude.
                    let t_term = ((t + 5.0).max(0.0)) / 32.5;
                    let pet = (0.10 + 1.28 * libm::powf(t_term, 1.35)).clamp(0.08, 1.65);
                    pet_row[c] = pet;

                    // TWI = ln((A_spec + dx) / max(tan(beta), 0.002)) computed in f64 to avoid
                    // f32 overflow on extreme finite flow accumulation values.
                    let twi_val = match flow_acc_m2 {
                        Some(acc) => {
                            let dx = f64::from(cfg.dx_m);
                            let denom = f64::from(slope_tan.max(0.002));
                            let log_num = if acc[idx] > 1e300_f64 {
                                libm::log(acc[idx]) - libm::log(dx)
                            } else {
                                libm::log(acc[idx] / dx + dx)
                            };
                            (log_num - libm::log(denom)) as f32
                        }
                        None => libm::logf((cfg.dx_m + cfg.dx_m) / slope_tan.max(0.002)),
                    };
                    twi_row[c] = twi_val;

                    // Normalized moisture in [0, 1] combining P/(P+PET) and valley wetness TWI.
                    let aridity_frac = p / (p + pet);
                    let twi_frac = ((twi_val - 4.0) / 10.0).clamp(0.0, 1.0);
                    m_row[c] = (0.78 * aridity_frac + 0.22 * twi_frac).clamp(0.0, 1.0);
                }
            },
        );

    Ok(ClimateField {
        width,
        height,
        temp_c,
        precip_m_yr,
        pet_m_yr,
        twi,
        moisture,
        ela_m,
        wind_azimuth_rad,
    })
}

/// Memory-lean Stage S7 Pass 2 helper computing only `(twi, moisture)` from existing
/// `precip_m_yr` and `pet_m_yr` plus Stage S5 `flow_acc_m2` (`128 MiB` output @ `4096^2`,
/// avoiding re-allocating `temp_c`/`precip_m_yr`/`pet_m_yr`/`ela_m`).
pub fn compute_twi_and_moisture(
    elev_m: &[f32],
    precip_m_yr: &[f32],
    pet_m_yr: &[f32],
    flow_acc_m2: &[f64],
    width: usize,
    height: usize,
    dx_m: f32,
) -> Result<(Vec<f32>, Vec<f32>), ClimateError> {
    if flow_acc_m2.len() != width * height {
        return Err(ClimateError {
            message: "input slice lengths do not match grid dimensions".to_owned(),
        });
    }
    let half = flow_acc_m2.len() / 2;
    compute_twi_and_moisture_halves(
        elev_m,
        precip_m_yr,
        pet_m_yr,
        &flow_acc_m2[..half],
        &flow_acc_m2[half..],
        width,
        height,
        dx_m,
    )
}

/// Split-halves variant of [`compute_twi_and_moisture`] avoiding `128 MiB` `Vec<f64>` @ `4096^2`.
#[allow(clippy::too_many_arguments)]
pub fn compute_twi_and_moisture_halves(
    elev_m: &[f32],
    precip_m_yr: &[f32],
    pet_m_yr: &[f32],
    flow_acc_lo: &[f64],
    flow_acc_hi: &[f64],
    width: usize,
    height: usize,
    dx_m: f32,
) -> Result<(Vec<f32>, Vec<f32>), ClimateError> {
    if width < 8 || height < 8 {
        return Err(ClimateError {
            message: "grid dimensions must be >= 8x8".to_owned(),
        });
    }
    if !dx_m.is_finite() || dx_m <= 0.0 {
        return Err(ClimateError {
            message: "dx_m must be finite and > 0".to_owned(),
        });
    }
    let n = width * height;
    let half = n / 2;
    if elev_m.len() != n
        || precip_m_yr.len() != n
        || pet_m_yr.len() != n
        || flow_acc_lo.len() != half
        || flow_acc_hi.len() != n - half
    {
        return Err(ClimateError {
            message: "input slice lengths do not match grid dimensions".to_owned(),
        });
    }
    if elev_m.iter().any(|h| !h.is_finite()) {
        return Err(ClimateError {
            message: "elev_m contains non-finite values".to_owned(),
        });
    }
    if precip_m_yr.iter().any(|&v| !v.is_finite() || v < 0.0) {
        return Err(ClimateError {
            message: "precip_m_yr contains non-finite or negative values".to_owned(),
        });
    }
    if pet_m_yr.iter().any(|&v| !v.is_finite() || v < 0.0) {
        return Err(ClimateError {
            message: "pet_m_yr contains non-finite or negative values".to_owned(),
        });
    }
    if flow_acc_lo.iter().any(|&v| !v.is_finite() || v < 0.0)
        || flow_acc_hi.iter().any(|&v| !v.is_finite() || v < 0.0)
    {
        return Err(ClimateError {
            message: "flow_acc_m2 contains non-finite or negative values".to_owned(),
        });
    }

    let mut twi = vec![0.0_f32; n];
    let mut moisture = vec![0.0_f32; n];
    let dx_f64 = f64::from(dx_m);

    twi.par_chunks_mut(width)
        .zip(moisture.par_chunks_mut(width))
        .enumerate()
        .for_each(|(r, (twi_row, m_row))| {
            let r_lo = r.saturating_sub(1);
            let r_hi = (r + 1).min(height - 1);
            let dz_span = ((r_hi - r_lo).max(1) as f32) * dx_m;
            let row_base = r * width;
            for c in 0..width {
                let c_lo = c.saturating_sub(1);
                let c_hi = (c + 1).min(width - 1);
                let dx_span = ((c_hi - c_lo).max(1) as f32) * dx_m;
                let idx = row_base + c;
                let hx = (elev_m[row_base + c_hi] - elev_m[row_base + c_lo]) / dx_span;
                let hz = (elev_m[r_hi * width + c] - elev_m[r_lo * width + c]) / dz_span;
                let slope_tan = libm::hypotf(hx, hz);
                let denom = f64::from(slope_tan.max(0.002));
                let acc = if idx < half {
                    flow_acc_lo[idx]
                } else {
                    flow_acc_hi[idx - half]
                };
                let log_num = if acc > 1e300_f64 {
                    libm::log(acc) - libm::log(dx_f64)
                } else {
                    libm::log(acc / dx_f64 + dx_f64)
                };
                let twi_val = (log_num - libm::log(denom)) as f32;
                twi_row[c] = twi_val;

                let p = precip_m_yr[idx];
                let pet = pet_m_yr[idx];
                let aridity_frac = if p + pet > 0.0 { p / (p + pet) } else { 0.0 };
                let twi_frac = ((twi_val - 4.0) / 10.0).clamp(0.0, 1.0);
                m_row[c] = (0.78 * aridity_frac + 0.22 * twi_frac).clamp(0.0, 1.0);
            }
        });

    Ok((twi, moisture))
}

/// 2-pass climate–erosion coupling helper (`§2.2.7` item 4).
///
/// # Resolution contract
/// `cfg.dx_m` MUST be the cell spacing (in metres) of the source `(src_w, src_h)` grid `elev_m`.
/// Pass 1 solves on `(src_w, src_h)` (e.g., `512^2`) and bilinearly upsamples onto `(target_w,
/// target_h)` (`1024^2`) when `flow_acc_m2` is `None`. When `flow_acc_m2` is provided at
/// `(target_w, target_h)`, Pass 2 automatically scales `target_dx = cfg.dx_m * (src_w / target_w)`
/// and recomputes full-resolution `twi` and `moisture` on the upsampled grid.
pub fn couple_climate_for_resolution(
    elev_m: &[f32],
    src_w: usize,
    src_h: usize,
    target_w: usize,
    target_h: usize,
    flow_acc_m2: Option<&[f64]>,
    cfg: &ClimateConfig,
) -> Result<ClimateField, ClimateError> {
    if src_w == target_w && src_h == target_h {
        return compute_climate(elev_m, target_w, target_h, flow_acc_m2, cfg);
    }
    if target_w < 2 || target_h < 2 {
        return Err(ClimateError {
            message: format!("target dimensions {}x{} must be >= 2x2", target_w, target_h),
        });
    }
    if elev_m.len() != src_w * src_h {
        return Err(ClimateError {
            message: format!(
                "elev_m length {} does not match src {}x{}",
                elev_m.len(),
                src_w,
                src_h
            ),
        });
    }
    if let Some(acc) = flow_acc_m2 {
        if acc.len() != target_w * target_h {
            return Err(ClimateError {
                message: format!(
                    "flow_acc_m2 length {} does not match target {}x{}",
                    acc.len(),
                    target_w,
                    target_h
                ),
            });
        }
        if acc.iter().any(|&v| !v.is_finite() || v < 0.0) {
            return Err(ClimateError {
                message: "flow_acc_m2 contains non-finite or negative values".to_owned(),
            });
        }
    }
    let coarse = compute_climate(elev_m, src_w, src_h, None, cfg)?;
    let mut upsampled = coarse.resample_bilinear(target_w, target_h);
    if let Some(acc) = flow_acc_m2 {
        let target_dx = cfg.dx_m * (src_w as f32 / target_w as f32);
        let elev_up = bilinear_resample(elev_m, src_w, src_h, target_w, target_h);
        let denom_z = (target_h - 1) as f32;
        let denom_x = (target_w - 1) as f32;
        let _ = (denom_x, denom_z);
        upsampled
            .twi
            .par_chunks_mut(target_w)
            .zip(upsampled.moisture.par_chunks_mut(target_w))
            .enumerate()
            .for_each(|(r, (twi_row, m_row))| {
                let r_lo = r.saturating_sub(1);
                let r_hi = (r + 1).min(target_h - 1);
                let dz_span = ((r_hi - r_lo).max(1) as f32) * target_dx;
                let base = r * target_w;
                for c in 0..target_w {
                    let c_lo = c.saturating_sub(1);
                    let c_hi = (c + 1).min(target_w - 1);
                    let dx_span = ((c_hi - c_lo).max(1) as f32) * target_dx;
                    let idx = base + c;
                    let hx = (elev_up[base + c_hi] - elev_up[base + c_lo]) / dx_span;
                    let hz =
                        (elev_up[r_hi * target_w + c] - elev_up[r_lo * target_w + c]) / dz_span;
                    let slope_tan = libm::hypotf(hx, hz);
                    let dx = f64::from(target_dx);
                    let denom = f64::from(slope_tan.max(0.002));
                    let log_num = if acc[idx] > 1e300_f64 {
                        libm::log(acc[idx]) - libm::log(dx)
                    } else {
                        libm::log(acc[idx] / dx + dx)
                    };
                    let twi_val = (log_num - libm::log(denom)) as f32;
                    twi_row[c] = twi_val;
                    let p = upsampled.precip_m_yr[idx];
                    let pet = upsampled.pet_m_yr[idx];
                    let aridity_frac = p / (p + pet);
                    let twi_frac = ((twi_val - 4.0) / 10.0).clamp(0.0, 1.0);
                    m_row[c] = (0.78 * aridity_frac + 0.22 * twi_frac).clamp(0.0, 1.0);
                }
            });
    }
    Ok(upsampled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tectonic::generate_uplift;

    fn symmetric_we_ridge(size: usize, peak_m: f32) -> (Vec<f32>, f32) {
        let dx_m = crate::WORLD_EXTENT_M / size as f32;
        let mut elev = vec![0.0_f32; size * size];
        for r in 0..size {
            for c in 0..size {
                let u = c as f32 / (size - 1) as f32;
                let h = if u < 0.35 {
                    -200.0 * (0.35 - u) / 0.35
                } else {
                    let du = (u - 0.58) / 0.10;
                    let ridge = peak_m * libm::expf(-0.5 * du * du);
                    ridge + 120.0 * ((u - 0.35) / 0.65)
                };
                elev[r * size + c] = h;
            }
        }
        (elev, dx_m)
    }

    #[test]
    fn fixed_latitude_zero_north_south_gradient_and_lapse_rate() {
        let n = 128;
        let (elev, dx_m) = symmetric_we_ridge(n, 5_600.0);
        let cfg = ClimateConfig {
            dx_m,
            ..ClimateConfig::default()
        };
        let clim = compute_climate(&elev, n, n, None, &cfg).expect("climate");

        let north_sum: f64 = clim.temp_c[..(n / 4) * n]
            .iter()
            .map(|&t| f64::from(t))
            .sum();
        let south_sum: f64 = clim.temp_c[(3 * n / 4) * n..]
            .iter()
            .map(|&t| f64::from(t))
            .sum();
        let q_len = ((n / 4) * n) as f64;
        let north_mean = north_sum / q_len;
        let south_mean = south_sum / q_len;
        assert!(
            (north_mean - south_mean).abs() < 0.05,
            "zero latitudinal gradient expected, got north={north_mean}, south={south_mean}"
        );

        // Verify exact Γ = 6.3 °C/km on flat summit/lowland cells away from maritime coast.
        let mut flat_high = vec![4_365.0_f32; n * n];
        for r in 0..n {
            for c in 0..n {
                if c < 20 {
                    flat_high[r * n + c] = -100.0;
                } else if c > 80 {
                    flat_high[r * n + c] = 5_700.0;
                }
            }
        }
        let clim_flat = compute_climate(&flat_high, n, n, None, &cfg).expect("flat_high");
        let t_4365 = clim_flat.temp_c[(n / 2) * n + 50];
        let t_5700 = clim_flat.temp_c[(n / 2) * n + 110];
        assert!(
            t_4365.abs() < 0.25,
            "0 deg C isotherm near 4,365 m expected, got {t_4365}"
        );
        assert!(
            (t_5700 - (-8.41)).abs() < 0.25,
            "5,700 m summit temp near -8.4 C expected, got {t_5700}"
        );
    }

    #[test]
    fn decoupled_ela_and_aspect_precip_modulation() {
        let n = 128;
        let dx_m = crate::WORLD_EXTENT_M / n as f32;
        let mut elev = vec![4_500.0_f32; n * n];
        // Build an East-West ridge at r = 64 so north-facing (r < 64) vs south-facing (r > 64) slopes exist.
        for r in 0..n {
            let dr = (r as f32 - 64.0) / 16.0;
            let h = 3_600.0 + 1_800.0 * libm::expf(-0.5 * dr * dr);
            for c in 0..n {
                elev[r * n + c] = if c < 12 { -100.0 } else { h };
            }
        }
        let cfg = ClimateConfig {
            dx_m,
            ..ClimateConfig::default()
        };
        let clim = compute_climate(&elev, n, n, None, &cfg).expect("climate");

        let north_aspect_cell = 56 * n + 64; // slopes down toward north (-z), shaded
        let south_aspect_cell = 72 * n + 64; // slopes down toward south (+z), sunlit
        assert!(
            clim.temp_c[south_aspect_cell] > clim.temp_c[north_aspect_cell],
            "south-facing 11N slope should be warmer than north-facing"
        );
        assert!(
            clim.ela_m[south_aspect_cell] > clim.ela_m[north_aspect_cell],
            "south-facing sunlit slope should have higher ELA than shaded slope"
        );
        for &ela in &clim.ela_m {
            assert!(
                (4_600.0..=5_200.0).contains(&ela),
                "ELA {ela} out of [4600, 5200] m decoupled band"
            );
        }
    }

    #[test]
    fn orographic_windward_rain_and_leeward_rain_shadow() {
        let wcfg = WorldV2Config::for_size(0x0057_5632_2026_1120, 128);
        let tect = generate_uplift(&wcfg).expect("tectonic");
        let cfg = ClimateConfig::for_world(&wcfg);
        let clim = compute_climate(&tect.bedrock_h, 128, 128, None, &cfg).expect("climate");

        let mut min_p = f32::INFINITY;
        let mut windward_peak = 0.0_f32;
        let mut lee_sum = 0.0_f64;
        let mut lee_cnt = 0_usize;

        for r in 16..(128 - 16) {
            for c in 0..128 {
                let idx = r * 128 + c;
                let p = clim.precip_m_yr[idx];
                min_p = min_p.min(p);
                let u = c as f32 / 127.0;
                let h = tect.bedrock_h[idx];
                if h > 0.0 && (0.44..=0.62).contains(&u) {
                    windward_peak = windward_peak.max(p);
                }
                if h > 0.0 && (0.72..=0.92).contains(&u) {
                    lee_sum += f64::from(p);
                    lee_cnt += 1;
                }
            }
        }
        assert!(
            min_p >= 0.15,
            "strict precipitation floor 0.15 m/yr violated: {min_p}"
        );
        assert!(
            (2.5..=4.3).contains(&windward_peak),
            "windward peak {windward_peak} outside expected [2.5, 4.3] m/yr"
        );
        let lee_mean = (lee_sum / lee_cnt as f64) as f32;
        assert!(
            lee_mean < 0.5 * windward_peak,
            "leeward rain shadow {lee_mean} must be < 0.5 * windward_peak ({windward_peak})"
        );
    }

    #[test]
    fn pet_twi_moisture_and_two_pass_coupling_determinism() {
        let n = 64;
        let (elev, dx_m) = symmetric_we_ridge(n, 4_500.0);
        let mut flow_acc = vec![(dx_m * dx_m) as f64; n * n];
        let test_idx = (n / 2) * n + (n / 2);
        flow_acc[test_idx] = 100_000.0;

        let cfg = ClimateConfig {
            dx_m,
            ..ClimateConfig::default()
        };
        let c1 = compute_climate(&elev, n, n, Some(&flow_acc), &cfg).expect("c1");
        let c2 = compute_climate(&elev, n, n, Some(&flow_acc), &cfg).expect("c2");
        assert_eq!(c1.sha256_bytes(), c2.sha256_bytes());

        // Lowland PET near [1.2, 1.5] and higher than alpine PET.
        let lowland_pet = c1.pet_m_yr[(n / 2) * n + 23];
        let alpine_pet = c1.pet_m_yr[(n / 2) * n + 37];
        assert!(
            (1.15..=1.55).contains(&lowland_pet),
            "lowland PET {lowland_pet} expected near 1.2..1.5"
        );
        assert!(alpine_pet < lowland_pet);

        // Verify TWI formula on flat cell with known flow_acc.
        let flat = vec![100.0_f32; n * n];
        let c_flat = compute_climate(&flat, n, n, Some(&flow_acc), &cfg).expect("flat");
        let expected_twi = libm::logf(((100_000.0_f32 / dx_m) + dx_m) / 0.002);
        assert!((c_flat.twi[test_idx] - expected_twi).abs() < 1e-4);

        for &m in &c1.moisture {
            assert!((0.0..=1.0).contains(&m));
        }

        // 2-pass coupling upsamples 64^2 -> 128^2 cleanly.
        let coupled =
            couple_climate_for_resolution(&elev, n, n, 128, 128, None, &cfg).expect("couple");
        assert_eq!(coupled.width, 128);
        assert_eq!(coupled.temp_c.len(), 128 * 128);
    }

    #[test]
    fn negative_control_flattened_peaks_collapse_rain_shadow_and_snowline() {
        let n = 128;
        let dx_m = crate::WORLD_EXTENT_M / n as f32;
        let mut elev = vec![80.0_f32; n * n];
        for r in 0..n {
            for c in 0..40 {
                elev[r * n + c] = -60.0;
            }
        }
        let cfg = ClimateConfig {
            dx_m,
            ..ClimateConfig::default()
        };
        let clim = compute_climate(&elev, n, n, None, &cfg).expect("flat");

        let mut west_sum = 0.0_f64;
        let mut west_cnt = 0_usize;
        let mut east_sum = 0.0_f64;
        let mut east_cnt = 0_usize;
        for r in 8..(n - 8) {
            for c in 42..64 {
                west_sum += f64::from(clim.precip_m_yr[r * n + c]);
                west_cnt += 1;
            }
            for c in 96..120 {
                east_sum += f64::from(clim.precip_m_yr[r * n + c]);
                east_cnt += 1;
            }
        }
        let west_mean = west_sum / west_cnt as f64;
        let east_mean = east_sum / east_cnt as f64;
        let ratio = east_mean / west_mean;
        assert!(
            ratio >= 0.85,
            "flattened DEM must collapse rain shadow (ratio >= 0.85), got {ratio}"
        );

        let above_ela = elev
            .iter()
            .zip(clim.ela_m.iter())
            .filter(|(&h, &ela)| h >= ela)
            .count();
        assert_eq!(above_ela, 0, "flattened DEM must have zero cells >= ELA");
    }

    #[test]
    fn climate_rejects_invalid_config_and_non_finite_elevation() {
        let bad_decay = ClimateConfig {
            maritime_decay_m: 0.0,
            ..ClimateConfig::default()
        };
        assert!(bad_decay.validate().is_err());

        let bad_scale_h = ClimateConfig {
            moisture_scale_height_m: -10.0,
            ..ClimateConfig::default()
        };
        assert!(bad_scale_h.validate().is_err());

        let bad_precip = ClimateConfig {
            windward_precip_target_m_yr: f32::NAN,
            ..ClimateConfig::default()
        };
        assert!(bad_precip.validate().is_err());

        let mut elev = vec![100.0_f32; 16 * 16];
        elev[5 * 16 + 5] = f32::NAN;
        let cfg = ClimateConfig::default();
        assert!(compute_climate(&elev, 16, 16, None, &cfg).is_err());

        let clean_elev = vec![100.0_f32; 16 * 16];
        let mut nan_acc = vec![100.0_f64; 16 * 16];
        nan_acc[3 * 16 + 4] = f64::NAN;
        assert!(compute_climate(&clean_elev, 16, 16, Some(&nan_acc), &cfg).is_err());

        let mut neg_acc = vec![100.0_f64; 16 * 16];
        neg_acc[3 * 16 + 4] = -1.0;
        assert!(compute_climate(&clean_elev, 16, 16, Some(&neg_acc), &cfg).is_err());

        let mut up_nan_acc = vec![100.0_f64; 32 * 32];
        up_nan_acc[7 * 32 + 7] = f64::INFINITY;
        assert!(couple_climate_for_resolution(
            &clean_elev,
            16,
            16,
            32,
            32,
            Some(&up_nan_acc),
            &cfg
        )
        .is_err());

        // Reject target dimensions < 2x2 cleanly without panicking in resample_bilinear.
        assert!(couple_climate_for_resolution(&clean_elev, 16, 16, 1, 1, None, &cfg).is_err());
        assert!(couple_climate_for_resolution(&clean_elev, 16, 16, 1, 16, None, &cfg).is_err());
        assert!(couple_climate_for_resolution(&clean_elev, 16, 16, 16, 1, None, &cfg).is_err());

        // Extreme finite f64 flow accumulation must yield finite TWI (never f32::INFINITY).
        let huge_acc = vec![1e40_f64; 16 * 16];
        let huge_clim = compute_climate(&clean_elev, 16, 16, Some(&huge_acc), &cfg)
            .expect("huge finite flow_acc_m2");
        assert!(huge_clim.twi.iter().all(|v| v.is_finite()));

        let huge_up_acc = vec![1e40_f64; 32 * 32];
        let huge_up_clim =
            couple_climate_for_resolution(&clean_elev, 16, 16, 32, 32, Some(&huge_up_acc), &cfg)
                .expect("huge finite upsampled flow_acc_m2");
        assert!(huge_up_clim.twi.iter().all(|v| v.is_finite()));

        let clean_p = vec![1.2_f32; 16 * 16];
        let clean_pet = vec![1.0_f32; 16 * 16];
        let clean_acc = vec![100.0_f64; 16 * 16];
        assert!(
            compute_twi_and_moisture(&elev, &clean_p, &clean_pet, &clean_acc, 16, 16, 100.0)
                .is_err()
        );

        let mut bad_p = clean_p.clone();
        bad_p[2] = f32::NAN;
        assert!(compute_twi_and_moisture(
            &clean_elev,
            &bad_p,
            &clean_pet,
            &clean_acc,
            16,
            16,
            100.0
        )
        .is_err());

        let mut bad_pet = clean_pet.clone();
        bad_pet[4] = f32::INFINITY;
        assert!(compute_twi_and_moisture(
            &clean_elev,
            &clean_p,
            &bad_pet,
            &clean_acc,
            16,
            16,
            100.0
        )
        .is_err());
    }

    #[test]
    fn wind_azimuth_is_invariant_across_multigrid_level_sizes() {
        let target_cfg = WorldV2Config::for_size(0x5756_3201, 4096);
        let mut expected_bits: Option<u32> = None;
        for &sz in &[64_u32, 128, 256] {
            let mut lvl_cfg = target_cfg.clone();
            lvl_cfg.grid_size = sz;
            let clim_cfg = ClimateConfig::for_world_with_target(&lvl_cfg, 4096);
            let n = sz as usize;
            let elev = vec![100.0_f32; n * n];
            let clim = compute_climate(&elev, n, n, None, &clim_cfg).expect("climate");
            match expected_bits {
                None => expected_bits = Some(clim.wind_azimuth_rad.to_bits()),
                Some(prev) => assert_eq!(
                    clim.wind_azimuth_rad.to_bits(),
                    prev,
                    "wind_azimuth_rad drifted at grid size {sz}"
                ),
            }
        }
    }

    #[test]
    fn leeward_rain_shadow_gain_preserves_default_and_amplifies_dry_side() {
        let mut cfg_default = WorldV2Config::for_size(0x5756_3201, 128);
        cfg_default.extent_m = 81_920.0;
        let clim_cfg_default = ClimateConfig::for_world(&cfg_default);
        assert!((clim_cfg_default.leeward_rain_shadow_gain - 1.0).abs() < 1e-6);

        let mut cfg_multi = cfg_default.clone();
        cfg_multi.layout = WorldLayout::MultiEnvironment;
        let clim_cfg_multi = ClimateConfig::for_world(&cfg_multi);
        assert!((clim_cfg_multi.leeward_rain_shadow_gain - 1.32).abs() < 1e-6);

        let mut clim_cfg_dry = clim_cfg_default.clone();
        clim_cfg_dry.leeward_rain_shadow_gain = 1.45;

        let w = 128_usize;
        let mut elev = vec![-100.0_f32; w * w];
        for r in 0..w {
            for c in 48..w {
                let x = (c - 48) as f32 / (w - 48) as f32;
                let ridge = libm::expf(-((x - 0.35) / 0.14) * ((x - 0.35) / 0.14));
                elev[r * w + c] = 40.0 + 4_500.0 * ridge;
            }
        }
        let f_def = compute_climate(&elev, w, w, None, &clim_cfg_default).expect("default");
        let f_mul = compute_climate(&elev, w, w, None, &clim_cfg_dry).expect("dry gain");
        assert!(f_mul
            .precip_m_yr
            .iter()
            .all(|&p| p >= clim_cfg_dry.min_precip_m_yr));

        let lee_idx = 64 * w + 112;
        assert!(
            f_mul.precip_m_yr[lee_idx] < f_def.precip_m_yr[lee_idx],
            "expected stronger leeward drying: dry={} vs default={}",
            f_mul.precip_m_yr[lee_idx],
            f_def.precip_m_yr[lee_idx]
        );
    }
}
