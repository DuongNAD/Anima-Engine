//! Stage S3: Nonlinear Hillslope Diffusion (`Roering et al. 1999 [34]`) & Lithology-Dependent Thermal
//! Talus Relaxation (`Musgrave et al. 1989 [27]`), using closed-flux pairwise mass conservation and
//! `libm`-only trigonometry.

use rayon::prelude::*;

/// Lithology-dependent angle of repose in degrees (`§2.2.3`). All angles remain strictly `< 75°`.
#[must_use]
pub fn lithology_repose_deg(lithology_id: u8, has_sediment: bool) -> f32 {
    if has_sediment {
        return 34.0; // Unconsolidated scree/talus repose (`alpha_scree = 34°`)
    }
    match lithology_id {
        0 => 58.0, // Granite/Crystalline
        1 => 65.0, // Sandstone Caprock
        2 => 48.0, // Soluble Limestone/Karst
        3 => 55.0, // Basalt/Volcanic
        4 => 34.0, // Soft Shale / Regolith
        _ => 28.0, // Unconsolidated Alluvium / Soil (`alpha_soil = 28°`)
    }
}

/// Converts angle in degrees to tangent slope (`dz / dx`) via `libm::tanf`.
#[inline]
#[must_use]
pub fn deg_to_slope(deg: f32) -> f32 {
    libm::tanf(deg * (std::f32::consts::PI / 180.0))
}

/// Converts positive tangent slope (`dz / dx`) to angle in degrees via `libm::atanf`.
#[inline]
#[must_use]
pub fn slope_to_deg(slope: f32) -> f32 {
    libm::atanf(slope.max(0.0)) * (180.0 / std::f32::consts::PI)
}

/// Parameters controlling [`relax_thermal_and_hillslope`].
#[derive(Debug, Clone, PartialEq)]
pub struct ThermalHillslopeParams {
    pub dx_m: f32,
    pub dt_yr: f32,
    /// Hillslope soil diffusivity `D` (`m^2/yr`).
    pub diffusivity_d: f32,
    /// Critical Roering nonlinear creep slope `S_c = tan(critical_deg)`.
    pub critical_slope_sc: f32,
    /// Relaxation fraction applied per substep to excess over-repose height difference (`0..=0.8`).
    pub talus_rate: f32,
    /// Number of conservative substeps.
    pub substeps: usize,
}

impl Default for ThermalHillslopeParams {
    fn default() -> Self {
        Self {
            dx_m: 10.0,
            dt_yr: 100.0,
            diffusivity_d: 0.025,
            critical_slope_sc: deg_to_slope(38.0),
            talus_rate: 0.45,
            substeps: 12,
        }
    }
}

/// Mass conservation and steepest-slope diagnostics from [`relax_thermal_and_hillslope`].
#[derive(Debug, Clone, PartialEq)]
pub struct ThermalStepStats {
    pub mass_before: f64,
    pub mass_after: f64,
    pub rel_mass_error: f64,
    pub max_slope_deg_after: f32,
    pub non_cliff_spikes_over_75_deg: usize,
}

/// Relaxes hillslopes via nonlinear creep (`Roering 1999`) + thermal talus (`Musgrave 1989`).
///
/// Uses 16 fixed `rayon` row-bands with 1-row boundary halos, 2-row sliding buffers, and a
/// 7-entry repose LUT (`<= 258 MiB` internal @ `4096^2`), guaranteeing bit-identical 1-thread
/// vs. `N`-thread execution and relative mass conservation `< 1e-6`.
#[must_use]
pub fn relax_thermal_and_hillslope(
    bedrock_h: &mut [f32],
    sediment_s: &mut [f32],
    talus_frac: &mut [f32],
    lithology_id: &[u8],
    width: usize,
    height: usize,
    params: &ThermalHillslopeParams,
) -> ThermalStepStats {
    let n = width * height;
    let band_rows = height.div_ceil(16).max(1);
    let num_bands = height.div_ceil(band_rows);
    let half_bands = num_bands / 2;
    let half_rows = half_bands * band_rows;
    let half_n = (half_rows * width).min(n);
    let mut sed_lo = vec![0.0_f64; half_n];
    let mut sed_hi = vec![0.0_f64; n - half_n];
    let mut total_lo = vec![0.0_f64; half_n];
    let mut total_hi = vec![0.0_f64; n - half_n];
    relax_thermal_and_hillslope_with_scratch(
        bedrock_h,
        sediment_s,
        talus_frac,
        lithology_id,
        width,
        height,
        params,
        &mut sed_lo,
        &mut sed_hi,
        &mut total_lo,
        &mut total_hi,
    )
}

/// Variant of [`relax_thermal_and_hillslope`] that reuses caller-provided `f64` half-buffers
/// (`sed_lo`, `sed_hi`, `total_lo`, `total_hi`) when their lengths match the band split, avoiding
/// `256 MiB` of temporary `f64` allocations at `4096^2` inside the S2+S3 loop.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn relax_thermal_and_hillslope_with_scratch(
    bedrock_h: &mut [f32],
    sediment_s: &mut [f32],
    talus_frac: &mut [f32],
    lithology_id: &[u8],
    width: usize,
    height: usize,
    params: &ThermalHillslopeParams,
    scratch_sed_lo: &mut [f64],
    scratch_sed_hi: &mut [f64],
    scratch_total_lo: &mut [f64],
    scratch_total_hi: &mut [f64],
) -> ThermalStepStats {
    let n = width * height;
    assert_eq!(bedrock_h.len(), n);
    assert_eq!(sediment_s.len(), n);
    assert_eq!(talus_frac.len(), n);
    assert_eq!(lithology_id.len(), n);

    let band_rows = height.div_ceil(16).max(1);
    let num_bands = height.div_ceil(band_rows);
    let half_bands = num_bands / 2;
    let half_rows = half_bands * band_rows;
    let half_n = (half_rows * width).min(n);

    let mut fallback_sed_lo = Vec::new();
    let mut fallback_sed_hi = Vec::new();
    let mut fallback_total_lo = Vec::new();
    let mut fallback_total_hi = Vec::new();

    let use_caller = scratch_sed_lo.len() >= half_n
        && scratch_sed_hi.len() >= n - half_n
        && scratch_total_lo.len() >= half_n
        && scratch_total_hi.len() >= n - half_n;

    let (sed_lo, sed_hi, total_lo, total_hi): (&mut [f64], &mut [f64], &mut [f64], &mut [f64]) =
        if use_caller {
            (
                &mut scratch_sed_lo[..half_n],
                &mut scratch_sed_hi[..n - half_n],
                &mut scratch_total_lo[..half_n],
                &mut scratch_total_hi[..n - half_n],
            )
        } else {
            fallback_sed_lo.resize(half_n, 0.0);
            fallback_sed_hi.resize(n - half_n, 0.0);
            fallback_total_lo.resize(half_n, 0.0);
            fallback_total_hi.resize(n - half_n, 0.0);
            (
                &mut fallback_sed_lo,
                &mut fallback_sed_hi,
                &mut fallback_total_lo,
                &mut fallback_total_hi,
            )
        };

    for (dst, &s) in sed_lo.iter_mut().zip(sediment_s[..half_n].iter()) {
        *dst = f64::from(s);
    }
    for (dst, &s) in sed_hi.iter_mut().zip(sediment_s[half_n..].iter()) {
        *dst = f64::from(s);
    }
    for ((dst, &b), &s) in total_lo
        .iter_mut()
        .zip(bedrock_h[..half_n].iter())
        .zip(sed_lo.iter())
    {
        *dst = f64::from(b) + s;
    }
    for ((dst, &b), &s) in total_hi
        .iter_mut()
        .zip(bedrock_h[half_n..].iter())
        .zip(sed_hi.iter())
    {
        *dst = f64::from(b) + s;
    }

    let mass_before: f64 = total_lo.iter().sum::<f64>() + total_hi.iter().sum::<f64>();

    let dx = f64::from(params.dx_m.max(1e-3));
    let dx_diag = dx * std::f64::consts::SQRT_2;
    let sc = f64::from(params.critical_slope_sc.max(0.05));
    let diff_coeff =
        f64::from(params.diffusivity_d.max(0.0)) * f64::from(params.dt_yr.max(0.0)) / (dx * dx);
    let talus_gain = f64::from(params.talus_rate.clamp(0.0, 0.8));

    // 7-entry LUT replacing `repose_slope: Vec<f64>` (`128 MiB` -> `56 bytes`).
    let repose_lut: [f64; 7] = [
        f64::from(deg_to_slope(lithology_repose_deg(0, false))),
        f64::from(deg_to_slope(lithology_repose_deg(1, false))),
        f64::from(deg_to_slope(lithology_repose_deg(2, false))),
        f64::from(deg_to_slope(lithology_repose_deg(3, false))),
        f64::from(deg_to_slope(lithology_repose_deg(4, false))),
        f64::from(deg_to_slope(lithology_repose_deg(5, false))),
        f64::from(deg_to_slope(lithology_repose_deg(0, true))),
    ];
    let word_count = (n + 63) >> 6;
    let mut has_sed_bits = vec![0_u64; word_count];
    for (i, &s) in sed_lo.iter().chain(sed_hi.iter()).enumerate() {
        if s > 0.05 {
            has_sed_bits[i >> 6] |= 1_u64 << (i & 63);
        }
    }
    let repose_at = |idx: usize| -> f64 {
        let has_sed = (has_sed_bits[idx >> 6] & (1_u64 << (idx & 63))) != 0;
        if has_sed {
            repose_lut[6]
        } else {
            repose_lut[(lithology_id[idx] as usize).min(5)]
        }
    };

    let mut halos_top: Vec<Vec<f64>> = (0..num_bands).map(|_| vec![0.0_f64; width]).collect();
    let mut halos_bot: Vec<Vec<f64>> = (0..num_bands).map(|_| vec![0.0_f64; width]).collect();

    let steps = params.substeps.max(1);
    for _ in 0..steps {
        for b in 0..num_bands {
            let r_start = b * band_rows;
            let r_end = ((b + 1) * band_rows).min(height);
            if r_start > 0 {
                let src = (r_start - 1) * width;
                if src < half_n {
                    halos_top[b].copy_from_slice(&total_lo[src..src + width]);
                } else {
                    let off = src - half_n;
                    halos_top[b].copy_from_slice(&total_hi[off..off + width]);
                }
            }
            if r_end < height {
                let src = r_end * width;
                if src < half_n {
                    halos_bot[b].copy_from_slice(&total_lo[src..src + width]);
                } else {
                    let off = src - half_n;
                    halos_bot[b].copy_from_slice(&total_hi[off..off + width]);
                }
            }
        }

        let run_band = |(b, (band_total, band_sed)): (usize, (&mut [f64], &mut [f64]))| {
            let r_start = b * band_rows;
            let local_rows = band_total.len() / width;
            let mut prev_row = vec![0.0_f64; width];
            if r_start > 0 {
                prev_row.copy_from_slice(&halos_top[b]);
            }
            let mut curr_row = vec![0.0_f64; width];

            for lr in 0..local_rows {
                let r = r_start + lr;
                let row_off = lr * width;
                let (cur_row_mut, rest_rows) = band_total[row_off..].split_at_mut(width);
                curr_row.copy_from_slice(cur_row_mut);
                let next_row: &[f64] = if lr + 1 < local_rows {
                    &rest_rows[..width]
                } else if r + 1 < height {
                    &halos_bot[b]
                } else {
                    &[]
                };

                let global_row_base = r * width;
                for c in 0..width {
                    let i = global_row_base + c;
                    let h_i = curr_row[c];
                    let rep_i = repose_at(i);
                    let mut d = 0.0_f64;

                    // Fixed 8-neighbor order with strictly anti-symmetric IEEE-754 edge flux.
                    let nbs: [(isize, isize, f64); 8] = [
                        (0, 1, dx),
                        (0, -1, dx),
                        (1, 0, dx),
                        (-1, 0, dx),
                        (1, 1, dx_diag),
                        (1, -1, dx_diag),
                        (-1, 1, dx_diag),
                        (-1, -1, dx_diag),
                    ];
                    for &(dr, dc, dist) in &nbs {
                        let nr = r as isize + dr;
                        let nc = c as isize + dc;
                        if nr < 0 || nc < 0 || nr >= height as isize || nc >= width as isize {
                            continue;
                        }
                        let ncu = nc as usize;
                        let h_j = match dr {
                            -1 => prev_row[ncu],
                            0 => curr_row[ncu],
                            1 => next_row[ncu],
                            _ => unreachable!(),
                        };
                        let j = nr as usize * width + ncu;
                        let dh = h_i - h_j;
                        let abs_dh = libm::fabs(dh);
                        let s_mag = abs_dh / dist;
                        let ratio = (s_mag / sc).min(0.92);
                        let nl_factor = 1.0 / (1.0 - ratio * ratio);
                        let creep_dh =
                            (diff_coeff * dh * nl_factor).clamp(-0.08 * abs_dh, 0.08 * abs_dh);

                        let rep_hi = if dh >= 0.0 { rep_i } else { repose_at(j) };
                        let excess = (abs_dh - rep_hi * dist).max(0.0);
                        let thermal_dh = if dh >= 0.0 {
                            (talus_gain / 8.0) * excess
                        } else {
                            -(talus_gain / 8.0) * excess
                        };
                        let flux = (creep_dh + thermal_dh).clamp(-0.10 * abs_dh, 0.10 * abs_dh);
                        d -= flux;
                    }

                    cur_row_mut[c] = h_i + d;
                    band_sed[row_off + c] = (band_sed[row_off + c] + d).max(0.0);
                }
                std::mem::swap(&mut prev_row, &mut curr_row);
            }
        };

        let band_stride = band_rows * width;
        rayon::join(
            || {
                total_lo
                    .par_chunks_mut(band_stride)
                    .zip(sed_lo.par_chunks_mut(band_stride))
                    .enumerate()
                    .for_each(run_band);
            },
            || {
                total_hi
                    .par_chunks_mut(band_stride)
                    .zip(sed_hi.par_chunks_mut(band_stride))
                    .enumerate()
                    .for_each(|(b_hi, pair)| run_band((half_bands + b_hi, pair)));
            },
        );
    }
    drop(has_sed_bits);
    drop(halos_top);
    drop(halos_bot);

    let write_half =
        |bh_sl: &mut [f32], s_sl: &mut [f32], t_sl: &mut [f32], th_sl: &[f64], sf_sl: &[f64]| {
            bh_sl
                .par_chunks_mut(width)
                .zip(s_sl.par_chunks_mut(width))
                .zip(t_sl.par_chunks_mut(width))
                .zip(th_sl.par_chunks(width))
                .zip(sf_sl.par_chunks(width))
                .for_each(|((((bh_row, s_row), t_row), th_row), sf_row)| {
                    for c in 0..bh_row.len() {
                        bh_row[c] = (th_row[c] - sf_row[c]) as f32;
                        s_row[c] = sf_row[c] as f32;
                        t_row[c] = (sf_row[c] / 5.0).clamp(0.0, 1.0) as f32;
                    }
                });
        };
    write_half(
        &mut bedrock_h[..half_n],
        &mut sediment_s[..half_n],
        &mut talus_frac[..half_n],
        total_lo,
        sed_lo,
    );
    write_half(
        &mut bedrock_h[half_n..],
        &mut sediment_s[half_n..],
        &mut talus_frac[half_n..],
        total_hi,
        sed_hi,
    );

    let mass_after: f64 = bedrock_h
        .iter()
        .zip(sediment_s.iter())
        .map(|(&b, &s)| f64::from(b) + f64::from(s))
        .sum();
    let rel_mass_error = libm::fabs(mass_after - mass_before) / mass_before.abs().max(1.0);

    let cliff_tan_75 = deg_to_slope(75.0);
    let total_at = |idx: usize| -> f64 {
        if idx < half_n {
            total_lo[idx]
        } else {
            total_hi[idx - half_n]
        }
    };
    let row_stats: Vec<(f32, usize)> = (0..height)
        .into_par_iter()
        .map(|r| {
            let mut row_max_deg = 0.0_f32;
            let mut row_spikes = 0usize;
            for c in 0..width {
                let i = r * width + c;
                let h_i = total_at(i);
                let mut local_max_s = 0.0_f32;
                for &(dr, dc, dist) in &[
                    (0_isize, 1_isize, dx),
                    (0, -1, dx),
                    (1, 0, dx),
                    (-1, 0, dx),
                    (1, 1, dx_diag),
                    (1, -1, dx_diag),
                    (-1, 1, dx_diag),
                    (-1, -1, dx_diag),
                ] {
                    let nr = r as isize + dr;
                    let nc = c as isize + dc;
                    if nr >= 0 && nc >= 0 && nr < height as isize && nc < width as isize {
                        let nb = nr as usize * width + nc as usize;
                        let s = ((h_i - total_at(nb)) / dist) as f32;
                        if s > local_max_s {
                            local_max_s = s;
                        }
                    }
                }
                let deg = slope_to_deg(local_max_s);
                if deg > row_max_deg {
                    row_max_deg = deg;
                }
                if local_max_s > cliff_tan_75 && lithology_id[i] != 1 {
                    row_spikes += 1;
                }
            }
            (row_max_deg, row_spikes)
        })
        .collect();

    let mut max_slope_deg_after = 0.0_f32;
    let mut non_cliff_spikes_over_75_deg = 0usize;
    for (rmax, rspikes) in row_stats {
        if rmax > max_slope_deg_after {
            max_slope_deg_after = rmax;
        }
        non_cliff_spikes_over_75_deg += rspikes;
    }

    ThermalStepStats {
        mass_before,
        mass_after,
        rel_mass_error,
        max_slope_deg_after,
        non_cliff_spikes_over_75_deg,
    }
}
