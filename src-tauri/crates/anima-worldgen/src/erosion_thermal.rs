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
/// Edge fluxes across `(E, S, SE, SW)` neighbor pairs are computed row-parallel over immutable
/// read-only slices and summed conservatively in `f64`, guaranteeing bit-identical 1-thread vs.
/// `N`-thread `rayon` execution and relative mass conservation `< 1e-6`.
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
    assert_eq!(bedrock_h.len(), n);
    assert_eq!(sediment_s.len(), n);
    assert_eq!(talus_frac.len(), n);
    assert_eq!(lithology_id.len(), n);

    let mut bed_f64: Vec<f64> = bedrock_h.iter().map(|&v| f64::from(v)).collect();
    let mut sed_f64: Vec<f64> = sediment_s.iter().map(|&v| f64::from(v)).collect();
    let mut total_h: Vec<f64> = bed_f64
        .iter()
        .zip(sed_f64.iter())
        .map(|(&b, &s)| b + s)
        .collect();

    let mass_before: f64 = total_h.iter().sum();

    let dx = f64::from(params.dx_m.max(1e-3));
    let dx_diag = dx * std::f64::consts::SQRT_2;
    let sc = f64::from(params.critical_slope_sc.max(0.05));
    let diff_coeff =
        f64::from(params.diffusivity_d.max(0.0)) * f64::from(params.dt_yr.max(0.0)) / (dx * dx);
    let talus_gain = f64::from(params.talus_rate.clamp(0.0, 0.8));

    let repose_slope: Vec<f64> = (0..n)
        .map(|i| {
            f64::from(deg_to_slope(lithology_repose_deg(
                lithology_id[i],
                sed_f64[i] > 0.05,
            )))
        })
        .collect();

    // 4 undirected edge directions anchored at cell `i`: E (0), S (1), SE (2), SW (3).
    // Stored cleanly per row in `flux_edges[i * 4 + k]` (positive = mass moving from `i` -> neighbor).
    let mut flux_edges = vec![0.0_f64; n * 4];
    let mut delta_total = vec![0.0_f64; n];

    let steps = params.substeps.max(1);
    for _ in 0..steps {
        let total_ref: &[f64] = &total_h;
        let repose_ref: &[f64] = &repose_slope;

        // Parallel per-row flux evaluation (pure read of `total_ref`, zero cross-row races).
        flux_edges
            .par_chunks_mut(width * 4)
            .enumerate()
            .for_each(|(r, row_flux)| {
                let row_base = r * width;
                for c in 0..width {
                    let i = row_base + c;
                    let h_i = total_ref[i];
                    let base = c * 4;

                    let edges: [(isize, isize, f64); 4] = [
                        (0, 1, dx),       // E
                        (1, 0, dx),       // S
                        (1, 1, dx_diag),  // SE
                        (1, -1, dx_diag), // SW
                    ];

                    for (k, &(dr, dc, dist)) in edges.iter().enumerate() {
                        let nr = r as isize + dr;
                        let nc = c as isize + dc;
                        if nr < 0 || nc < 0 || nr >= height as isize || nc >= width as isize {
                            row_flux[base + k] = 0.0;
                            continue;
                        }
                        let j = nr as usize * width + nc as usize;
                        let dh = h_i - total_ref[j];
                        let s_mag = libm::fabs(dh) / dist;

                        // Roering nonlinear creep flux + thermal excess-repose slip.
                        // Clamp per-edge transfer <= 0.10 * |dh| so total 8-neighbor outflow <= 0.80 < 1.0
                        // (TVD / strictly non-oscillatory checkerboard stability).
                        let ratio = (s_mag / sc).min(0.92);
                        let nl_factor = 1.0 / (1.0 - ratio * ratio);
                        let creep_dh = (diff_coeff * dh * nl_factor)
                            .clamp(-0.08 * libm::fabs(dh), 0.08 * libm::fabs(dh));

                        let rep_hi = if dh >= 0.0 {
                            repose_ref[i]
                        } else {
                            repose_ref[j]
                        };
                        let max_allowed_dh = rep_hi * dist;
                        let excess = (libm::fabs(dh) - max_allowed_dh).max(0.0);
                        let thermal_dh = if dh >= 0.0 {
                            (talus_gain / 8.0) * excess
                        } else {
                            -(talus_gain / 8.0) * excess
                        };

                        row_flux[base + k] = (creep_dh + thermal_dh)
                            .clamp(-0.10 * libm::fabs(dh), 0.10 * libm::fabs(dh));
                    }
                }
            });

        // Deterministic accumulation guaranteeing exact symmetry `sum(delta_total) == 0`.
        delta_total.fill(0.0);
        for r in 0..height {
            let row_base = r * width;
            for c in 0..width {
                let i = row_base + c;
                let base = i * 4;
                // E
                if c + 1 < width {
                    let f = flux_edges[base];
                    delta_total[i] -= f;
                    delta_total[i + 1] += f;
                }
                // S
                if r + 1 < height {
                    let f = flux_edges[base + 1];
                    delta_total[i] -= f;
                    delta_total[i + width] += f;
                    // SE
                    if c + 1 < width {
                        let fd = flux_edges[base + 2];
                        delta_total[i] -= fd;
                        delta_total[i + width + 1] += fd;
                    }
                    // SW
                    if c > 0 {
                        let fd = flux_edges[base + 3];
                        delta_total[i] -= fd;
                        delta_total[i + width - 1] += fd;
                    }
                }
            }
        }

        for i in 0..n {
            let d = delta_total[i];
            if d >= 0.0 {
                sed_f64[i] += d;
            } else {
                let loss = -d;
                if sed_f64[i] >= loss {
                    sed_f64[i] -= loss;
                } else {
                    let rem = loss - sed_f64[i];
                    sed_f64[i] = 0.0;
                    bed_f64[i] -= rem;
                }
            }
            total_h[i] = bed_f64[i] + sed_f64[i];
        }
    }

    for i in 0..n {
        bedrock_h[i] = bed_f64[i] as f32;
        sediment_s[i] = sed_f64[i] as f32;
        talus_frac[i] = (sed_f64[i] / 5.0).clamp(0.0, 1.0) as f32;
    }

    let mass_after: f64 = bedrock_h
        .iter()
        .zip(sediment_s.iter())
        .map(|(&b, &s)| f64::from(b) + f64::from(s))
        .sum();
    let rel_mass_error = libm::fabs(mass_after - mass_before) / mass_before.abs().max(1.0);

    let mut max_slope_deg_after = 0.0_f32;
    let mut non_cliff_spikes_over_75_deg = 0usize;
    let cliff_tan_75 = deg_to_slope(75.0);

    for r in 0..height {
        for c in 0..width {
            let i = r * width + c;
            let h_i = total_h[i];
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
                    let s = ((h_i - total_h[nb]) / dist) as f32;
                    if s > local_max_s {
                        local_max_s = s;
                    }
                }
            }
            let deg = slope_to_deg(local_max_s);
            if deg > max_slope_deg_after {
                max_slope_deg_after = deg;
            }
            if local_max_s > cliff_tan_75 && lithology_id[i] != 1 {
                non_cliff_spikes_over_75_deg += 1;
            }
        }
    }

    ThermalStepStats {
        mass_before,
        mass_after,
        rel_mass_error,
        max_slope_deg_after,
        non_cliff_spikes_over_75_deg,
    }
}
