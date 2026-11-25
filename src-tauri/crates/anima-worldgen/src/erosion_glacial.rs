//! Stage S2b: Deterministic coarse-level LGM paleo-glacial cirque overdeepening & mass-conserving
//! terminal moraine deposition (Design Amendment A2, `MAP-D2`).
//!
//! Runs strictly on coarse multigrid levels (`512^2`, `1024^2`) so `2048^2` and `4096^2` incur
//! `0 MiB` extra memory overhead. Uses `libm` exclusively and deterministic D8 topological order.

use std::collections::VecDeque;

use crate::hydrology::{priority_flood_d8_lean_into, NO_DIR};

const DX8: [isize; 8] = [1, 1, 0, -1, -1, -1, 0, 1];
const DZ8: [isize; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
const DIST8: [f64; 8] = [
    1.0,
    std::f64::consts::SQRT_2,
    1.0,
    std::f64::consts::SQRT_2,
    1.0,
    std::f64::consts::SQRT_2,
    1.0,
    std::f64::consts::SQRT_2,
];

#[inline]
fn dir_neighbor(idx: usize, dir: u8, width: usize) -> usize {
    let r = (idx / width) as isize + DZ8[dir as usize];
    let c = (idx % width) as isize + DX8[dir as usize];
    r as usize * width + c as usize
}

#[allow(clippy::too_many_arguments)]
fn spread_excess_bfs(
    src: usize,
    vol_m3: f64,
    cap_m: f64,
    skip_src: bool,
    width: usize,
    height: usize,
    dx2: f64,
    sea_level_m: Option<f32>,
    total_h: &[f32],
    glacial_added: &[f64],
    moraine_delta: &mut [f64],
    visited_stamp: &mut [u32],
    bfs_stamp: &mut u32,
    bfs_queue: &mut VecDeque<usize>,
) {
    let mut rem = vol_m3;
    if rem <= 1e-9 {
        return;
    }
    if *bfs_stamp == u32::MAX {
        visited_stamp.fill(0);
        *bfs_stamp = 1;
    } else {
        *bfs_stamp += 1;
    }
    let stamp = *bfs_stamp;
    bfs_queue.clear();
    visited_stamp[src] = stamp;
    bfs_queue.push_back(src);
    let mut popped = 0_usize;
    while let Some(u) = bfs_queue.pop_front() {
        popped += 1;
        if !skip_src || u != src {
            let hr = (cap_m - (glacial_added[u] + moraine_delta[u])).max(0.0);
            if hr > 0.0 {
                let dep = rem.min(hr * dx2);
                moraine_delta[u] += dep / dx2;
                rem -= dep;
                if rem <= 1e-9 {
                    return;
                }
            }
        }
        if popped >= 4096 {
            break;
        }
        let r = u / width;
        let c = u % width;
        for dir in 0..8 {
            let nr = r as isize + DZ8[dir];
            let nc = c as isize + DX8[dir];
            if nr < 0 || nc < 0 || nr >= height as isize || nc >= width as isize {
                continue;
            }
            let nb = nr as usize * width + nc as usize;
            if visited_stamp[nb] == stamp {
                continue;
            }
            if sea_level_m.is_some_and(|sl| total_h[nb] <= sl) {
                continue;
            }
            visited_stamp[nb] = stamp;
            bfs_queue.push_back(nb);
        }
    }
    if rem > 1e-9 {
        moraine_delta[src] += rem / dx2;
    }
}

/// Physical parameters for the `S2b` LGM paleo-glacial cirque abrasion & moraine pass.
#[derive(Debug, Clone, PartialEq)]
pub struct GlacialParams {
    pub dx_m: f64,
    /// Timestep per glacial iteration (`2,000 yr`).
    pub dt_yr: f64,
    /// Number of glacial iterations to run on this coarse grid level.
    pub steps: usize,
    /// SNSM LGM paleo-ELA (`4,100 m`, Lachniet & Vázquez-Selem 2005 pp. 129-144; Benn et al. 2005).
    pub ela_paleo_m: f64,
    /// Mass-balance vertical gradient `b_0` (`0.005 yr^-1`, MacGregor et al. 2000; Herman & Braun 2008).
    pub mass_balance_grad_yr: f64,
    /// Upper accumulation cap (`m/yr`) preventing unphysical summit snowfall extremes.
    pub max_accum_m_yr: f64,
    /// Avalanche wall limit (`tan(42 deg)`): arête slopes above this shed snow (`b_i = 0`).
    pub max_accum_slope_tan: f64,
    /// Dimensionless glacial abrasion constant `K_g = 1.0e-4` (Humphrey & Raymond 1994; MacGregor et al. 2000).
    pub k_g: f64,
    /// Sliding exponent `alpha_g = 1.0`.
    pub alpha_g: f64,
    /// Temperate shallow-ice sliding velocity coefficient `c_b` (`yr^-1`), `u_s = sqrt(c_b * q_spec * S_eff)`.
    pub sliding_coeff_yr: f64,
    /// Maximum vertical bedrock/sediment abrasion allowed per step (`m`) for clean bowl evolution.
    pub max_step_erode_m: f64,
    /// Lower bed elevation gate (`m`) below which ice streams terminate/ablate without incising lowland valleys.
    pub min_bed_elev_m: f64,
    pub sea_level_m: Option<f32>,
}

impl Default for GlacialParams {
    fn default() -> Self {
        Self {
            dx_m: 80.0,
            dt_yr: 2_000.0,
            steps: 10,
            ela_paleo_m: 4_100.0,
            mass_balance_grad_yr: 0.005,
            max_accum_m_yr: 1.5,
            max_accum_slope_tan: libm::tan(42.0 * (std::f64::consts::PI / 180.0)),
            k_g: 1.0e-4,
            alpha_g: 1.0,
            sliding_coeff_yr: 0.16,
            max_step_erode_m: 5.0,
            min_bed_elev_m: 3_350.0,
            sea_level_m: Some(0.0),
        }
    }
}

/// Summary diagnostics returned by [`run_paleo_glacial_pass`].
#[derive(Debug, Clone, PartialEq)]
pub struct GlacialStats {
    pub steps_run: usize,
    pub glaciated_cells_peak: usize,
    pub eroded_volume_m3: f64,
    pub deposited_volume_m3: f64,
    pub rel_mass_error: f64,
}

/// Executes deterministic mass-conserving LGM paleo-glacial cirque abrasion and terminal moraine deposition.
pub fn run_paleo_glacial_pass(
    bedrock_h: &mut [f32],
    sediment_s: &mut [f32],
    width: usize,
    height: usize,
    params: &GlacialParams,
) -> GlacialStats {
    let n = width * height;
    assert_eq!(bedrock_h.len(), n);
    assert_eq!(sediment_s.len(), n);
    if params.steps == 0 || params.dt_yr <= 0.0 || n < 9 {
        return GlacialStats {
            steps_run: 0,
            glaciated_cells_peak: 0,
            eroded_volume_m3: 0.0,
            deposited_volume_m3: 0.0,
            rel_mass_error: 0.0,
        };
    }

    let mass_before: f64 = bedrock_h
        .iter()
        .zip(sediment_s.iter())
        .map(|(&b, &s)| f64::from(b) + f64::from(s))
        .sum();

    let dx = params.dx_m.max(1e-3);
    let dx2 = dx * dx;

    let mut total_h = vec![0.0_f32; n];
    let mut filled_h = vec![0.0_f32; n];
    let mut d8_dir = vec![NO_DIR; n];
    let mut order = Vec::with_capacity(n);
    let mut q_ice = vec![0.0_f64; n];
    let mut a_glac = vec![0.0_f64; n];
    let mut phi_till = vec![0.0_f64; n];
    let mut glacial_added = vec![0.0_f64; n];
    let max_hops = ((8_000.0 / dx) as usize).clamp(32, 200);
    let mut visited_stamp = vec![0_u32; n];
    let mut bfs_stamp = 0_u32;
    let mut bfs_queue: VecDeque<usize> = VecDeque::with_capacity(256);

    let mut glaciated_cells_peak = 0_usize;
    let mut eroded_volume_m3 = 0.0_f64;
    let mut deposited_volume_m3 = 0.0_f64;

    for _step in 0..params.steps {
        for ((dst, &b), &s) in total_h
            .iter_mut()
            .zip(bedrock_h.iter())
            .zip(sediment_s.iter())
        {
            *dst = b + s;
        }
        filled_h.copy_from_slice(&total_h);
        order.clear();
        priority_flood_d8_lean_into(
            &mut filled_h,
            &mut d8_dir,
            &mut order,
            width,
            height,
            params.sea_level_m,
            true,
        );

        q_ice.fill(0.0);
        a_glac.fill(0.0);
        phi_till.fill(0.0);

        let mut glaciated_this_step = 0_usize;

        // Pass 1 (upstream -> downstream): accumulate glacier ice discharge `Q_ice` (`m^3/yr`).
        for &node_u in order.iter().rev() {
            let i = node_u as usize;
            let d = d8_dir[i];
            let _h_ice = f64::from(filled_h[i]);
            let h_bed = f64::from(total_h[i]);

            let b_i = if h_bed > params.ela_paleo_m {
                let mut raw_max_slope = 0.0_f64;
                let r = i / width;
                let c = i % width;
                for dir in 0..8 {
                    let nr = r as isize + DZ8[dir];
                    let nc = c as isize + DX8[dir];
                    if nr >= 0 && nc >= 0 && nr < height as isize && nc < width as isize {
                        let nb = nr as usize * width + nc as usize;
                        let sl = (h_bed - f64::from(total_h[nb])) / (dx * DIST8[dir]);
                        if sl > raw_max_slope {
                            raw_max_slope = sl;
                        }
                    }
                }
                if raw_max_slope <= params.max_accum_slope_tan {
                    (params.mass_balance_grad_yr * (h_bed - params.ela_paleo_m))
                        .min(params.max_accum_m_yr)
                } else {
                    0.0
                }
            } else {
                params.mass_balance_grad_yr * (h_bed - params.ela_paleo_m)
            };

            let q_in = q_ice[i];
            let a_in = a_glac[i];
            if b_i <= 0.0 && q_in <= 0.0 {
                continue;
            }
            let w_ablate = if b_i < 0.0 {
                dx.max(1.25 * libm::sqrt((a_in + dx2).max(dx2)))
            } else {
                dx
            };
            let q_out = (q_in + b_i * dx * w_ablate).max(0.0);
            let a_self = if b_i > 0.0 { dx2 } else { 0.0 };
            let a_tot = a_in + a_self;

            if d < NO_DIR && h_bed > params.min_bed_elev_m && q_out > 0.0 {
                let rec = dir_neighbor(i, d, width);
                q_ice[rec] += q_out;
                a_glac[rec] += a_tot;
            }
        }

        // Track mass-conserving glacial sediment increments separately per step so freshly
        // deposited terminal/recessional moraines can spread gently at scree repose (`<= 34 deg`).
        let mut moraine_delta = vec![0.0_f64; n];

        // Pass 2 (upstream -> downstream): basal sliding abrasion + mass-conserving moraine deposition.
        for &node_u in order.iter().rev() {
            let i = node_u as usize;
            let q_in = q_ice[i];
            let phi_here = phi_till[i];
            let h_ice = f64::from(filled_h[i]);
            let h_bed = f64::from(total_h[i]);

            let b_i = if h_bed > params.ela_paleo_m {
                let mut raw_max_slope = 0.0_f64;
                let r = i / width;
                let c = i % width;
                for dir in 0..8 {
                    let nr = r as isize + DZ8[dir];
                    let nc = c as isize + DX8[dir];
                    if nr >= 0 && nc >= 0 && nr < height as isize && nc < width as isize {
                        let nb = nr as usize * width + nc as usize;
                        let sl = (h_bed - f64::from(total_h[nb])) / (dx * DIST8[dir]);
                        if sl > raw_max_slope {
                            raw_max_slope = sl;
                        }
                    }
                }
                if raw_max_slope <= params.max_accum_slope_tan {
                    (params.mass_balance_grad_yr * (h_bed - params.ela_paleo_m))
                        .min(params.max_accum_m_yr)
                } else {
                    0.0
                }
            } else {
                params.mass_balance_grad_yr * (h_bed - params.ela_paleo_m)
            };

            let w_ablate = if b_i < 0.0 {
                dx.max(1.25 * libm::sqrt((a_glac[i] + dx2).max(dx2)))
            } else {
                dx
            };
            let q_out = (q_in + b_i * dx * w_ablate).max(0.0);
            let q_mid = 0.5 * (q_in + q_out);

            let mut local_eroded_vol = 0.0_f64;
            let d = d8_dir[i];
            // Focus glacial sliding inside active cirques and ice streams (`A_glac >= 0.0256 km^2`)
            // rather than eroding convex interfluve snowfields uniformly.
            if a_glac[i] >= 25_600.0 && q_mid > 0.0 && h_bed > params.min_bed_elev_m {
                glaciated_this_step += 1;
                let s_ice = if d < NO_DIR {
                    let rec = dir_neighbor(i, d, width);
                    let dist = dx * DIST8[d as usize];
                    ((h_ice - f64::from(filled_h[rec])) / dist).clamp(0.02, 0.70)
                } else {
                    0.03
                };
                // Cirque-floor abrasion law (`Herman & Braun 2008`): active where thick glacier
                // occupies gentle troughs (`S ~ 0.05..0.14`) and strongly decoupled on steep rock
                // riegels (`S > 0.22`), carving closed cirque tarns behind rock lips with modest total till.
                let r_s = s_ice / 0.10;
                let denom = 1.0 + r_s * r_s;
                let slope_coupling = (1.5 * r_s) / (denom * denom);
                let w_glac = dx.max(0.25 * libm::sqrt((a_glac[i] + dx2).max(dx2)));
                let q_spec = q_mid / w_glac;
                let u_s = libm::sqrt(params.sliding_coeff_yr * q_spec * slope_coupling);
                let dh_headroom = (h_bed - (params.min_bed_elev_m - 100.0)).max(0.0);
                let dh = (params.k_g * libm::pow(u_s, params.alpha_g) * params.dt_yr)
                    .min(params.max_step_erode_m)
                    .min(dh_headroom);

                if dh > 0.0 {
                    let mut remaining = dh;
                    let sed_avail = f64::from(sediment_s[i]).max(0.0);
                    let sed_cut = remaining.min(sed_avail);
                    if sed_cut > 0.0 {
                        sediment_s[i] = (sed_avail - sed_cut) as f32;
                        glacial_added[i] = (glacial_added[i] - sed_cut).max(0.0);
                        remaining -= sed_cut;
                    }
                    if remaining > 0.0 {
                        bedrock_h[i] -= remaining as f32;
                    }
                    local_eroded_vol = dh * dx2;
                    eroded_volume_m3 += local_eroded_vol;
                }
            }

            let till_avail = phi_here + local_eroded_vol;
            if till_avail <= 0.0 {
                continue;
            }

            // Deposit arcuate moraine ridges across the snout & prograde proglacial outwash aprons
            // down-valley along D8 drainage when local till thickness reaches 30 m.
            let cap_m = 30.0_f64;
            let mut deposit_spread = |center: usize, vol_m3: f64| {
                let mut cur = center;
                let mut rem_vol = vol_m3;
                for hop in 0..max_hops {
                    if rem_vol <= 1e-9 {
                        break;
                    }
                    let r = cur / width;
                    let c = cur % width;
                    let h_c = f64::from(total_h[cur]);
                    let mut ring = [cur; 9];
                    let mut count = 1_usize;
                    for dir in 0..8 {
                        let nr = r as isize + DZ8[dir];
                        let nc = c as isize + DX8[dir];
                        if nr >= 0 && nc >= 0 && nr < height as isize && nc < width as isize {
                            let nb = nr as usize * width + nc as usize;
                            let h_nb = f64::from(total_h[nb]);
                            let elev_ok = if hop == 0 {
                                h_nb <= h_c + 12.0 && h_nb > params.min_bed_elev_m - 200.0
                            } else {
                                h_nb <= h_c + 16.0
                            };
                            if elev_ok && params.sea_level_m.is_none_or(|sl| total_h[nb] > sl) {
                                ring[count] = nb;
                                count += 1;
                            }
                        }
                    }
                    let mut hr = [0.0_f64; 9];
                    for (hr_slot, &cell) in hr[..count].iter_mut().zip(ring[..count].iter()) {
                        *hr_slot = (cap_m - (glacial_added[cell] + moraine_delta[cell])).max(0.0);
                    }
                    let want_c = if count == 1 { rem_vol } else { 0.45 * rem_vol };
                    let dep_c = want_c.min(hr[0] * dx2);
                    if dep_c > 0.0 {
                        moraine_delta[ring[0]] += dep_c / dx2;
                        rem_vol -= dep_c;
                    }
                    if count > 1 && rem_vol > 1e-9 {
                        let sum_nb_hr: f64 = hr[1..count].iter().sum();
                        if sum_nb_hr > 0.0 {
                            let mut used_nb = 0.0_f64;
                            let rem_before_nb = rem_vol;
                            for (&nb, &hr_k) in ring[1..count].iter().zip(hr[1..count].iter()) {
                                let dep_k = (rem_before_nb * (hr_k / sum_nb_hr)).min(hr_k * dx2);
                                if dep_k > 0.0 {
                                    moraine_delta[nb] += dep_k / dx2;
                                    used_nb += dep_k;
                                }
                            }
                            rem_vol = (rem_vol - used_nb).max(0.0);
                        }
                    }
                    if rem_vol > 1e-9 {
                        let extra_c_cap = (hr[0] * dx2 - dep_c).max(0.0);
                        let extra_c = rem_vol.min(extra_c_cap);
                        if extra_c > 0.0 {
                            moraine_delta[ring[0]] += extra_c / dx2;
                            rem_vol -= extra_c;
                        }
                    }
                    if rem_vol <= 1e-9 {
                        break;
                    }
                    let d_cur = d8_dir[cur];
                    if d_cur >= NO_DIR {
                        break;
                    }
                    let next = dir_neighbor(cur, d_cur, width);
                    if params.sea_level_m.is_some_and(|sl| total_h[next] <= sl) {
                        break;
                    }
                    cur = next;
                }
                if rem_vol > 1e-9 {
                    spread_excess_bfs(
                        cur,
                        rem_vol,
                        cap_m,
                        false,
                        width,
                        height,
                        dx2,
                        params.sea_level_m,
                        &total_h,
                        &glacial_added,
                        &mut moraine_delta,
                        &mut visited_stamp,
                        &mut bfs_stamp,
                        &mut bfs_queue,
                    );
                }
            };

            if d >= NO_DIR || h_bed <= params.min_bed_elev_m || q_out <= 0.0 {
                deposit_spread(i, till_avail);
                deposited_volume_m3 += till_avail;
            } else {
                let rec = dir_neighbor(i, d, width);
                let melt_frac = if b_i < 0.0 && q_in > 0.0 {
                    ((-b_i * dx * w_ablate) / q_in).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                // Retain 80% of englacial till toward the snout/margin so distinct terminal &
                // recessional moraine ridges build near `q_out -> 0` instead of washing out flat.
                let eff_melt = 0.20 * melt_frac;
                let dep = till_avail * eff_melt;
                if dep > 0.0 {
                    deposit_spread(i, dep);
                    deposited_volume_m3 += dep;
                }
                phi_till[rec] += till_avail - dep;
            }
        }

        // Conservatively relax fresh moraines (`<= 34 deg` scree angle) over 16 substeps.
        let repose_moraine_tan = libm::tan(34.0 * (std::f64::consts::PI / 180.0));
        for _relax in 0..16 {
            for r in 0..height {
                for c in 0..width {
                    let i = r * width + c;
                    if moraine_delta[i] <= 0.05 {
                        continue;
                    }
                    let h_i = f64::from(bedrock_h[i]) + f64::from(sediment_s[i]) + moraine_delta[i];
                    let mut total_w = 0.0_f64;
                    let mut nbs = [(0_usize, 0.0_f64); 8];
                    let mut nb_cnt = 0_usize;
                    for dir in 0..8 {
                        let nr = r as isize + DZ8[dir];
                        let nc = c as isize + DX8[dir];
                        if nr < 0 || nc < 0 || nr >= height as isize || nc >= width as isize {
                            continue;
                        }
                        let nb = nr as usize * width + nc as usize;
                        let h_nb = f64::from(bedrock_h[nb])
                            + f64::from(sediment_s[nb])
                            + moraine_delta[nb];
                        let dist = dx * DIST8[dir];
                        let slope_ex = (h_i - h_nb - repose_moraine_tan * dist).max(0.0);
                        if slope_ex > 0.0 {
                            nbs[nb_cnt] = (nb, slope_ex);
                            nb_cnt += 1;
                            total_w += slope_ex;
                        }
                    }
                    if total_w > 0.0 {
                        let out_amt = (0.40 * moraine_delta[i]).min(0.18 * total_w);
                        if out_amt > 0.0 {
                            let scale = out_amt / total_w;
                            moraine_delta[i] -= out_amt;
                            for &(nb, w) in &nbs[..nb_cnt] {
                                moraine_delta[nb] += w * scale;
                            }
                        }
                    }
                }
            }
        }

        // Proglacial outwash cap-spill in strict upstream->downstream topological order so no
        // steep gorge pocket or caldera floor accumulates unphysical > 42 m till stacks while
        // conserving total deposited mass to machine precision.
        let spill_cap_m = 42.0_f64;
        for &node_u in order.iter().rev() {
            let i = node_u as usize;
            let dm = moraine_delta[i];
            if dm <= 0.0 {
                continue;
            }
            let excess = (glacial_added[i] + dm - spill_cap_m).max(0.0).min(dm);
            if excess <= 0.0 {
                continue;
            }
            let d_i = d8_dir[i];
            if d_i < NO_DIR {
                let rec = dir_neighbor(i, d_i, width);
                if params.sea_level_m.is_none_or(|sl| total_h[rec] > sl) {
                    moraine_delta[i] -= excess;
                    moraine_delta[rec] += excess;
                } else {
                    moraine_delta[i] -= excess;
                    spread_excess_bfs(
                        i,
                        excess * dx2,
                        spill_cap_m,
                        true,
                        width,
                        height,
                        dx2,
                        params.sea_level_m,
                        &total_h,
                        &glacial_added,
                        &mut moraine_delta,
                        &mut visited_stamp,
                        &mut bfs_stamp,
                        &mut bfs_queue,
                    );
                }
            } else {
                moraine_delta[i] -= excess;
                spread_excess_bfs(
                    i,
                    excess * dx2,
                    spill_cap_m,
                    true,
                    width,
                    height,
                    dx2,
                    params.sea_level_m,
                    &total_h,
                    &glacial_added,
                    &mut moraine_delta,
                    &mut visited_stamp,
                    &mut bfs_stamp,
                    &mut bfs_queue,
                );
            }
        }
        for ((s_slot, g_slot), &dm) in sediment_s
            .iter_mut()
            .zip(glacial_added.iter_mut())
            .zip(moraine_delta.iter())
        {
            if dm > 0.0 {
                *s_slot += dm as f32;
                *g_slot += dm;
            }
        }

        if glaciated_this_step > glaciated_cells_peak {
            glaciated_cells_peak = glaciated_this_step;
        }
    }

    let mass_after: f64 = bedrock_h
        .iter()
        .zip(sediment_s.iter())
        .map(|(&b, &s)| f64::from(b) + f64::from(s))
        .sum();
    let rel_mass_error = libm::fabs(mass_after - mass_before) / mass_before.abs().max(1.0);

    GlacialStats {
        steps_run: params.steps,
        glaciated_cells_peak,
        eroded_volume_m3,
        deposited_volume_m3,
        rel_mass_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glacial_pass_carves_cirque_overdeepening_deposits_moraine_and_conserves_mass() {
        let n = 48usize;
        let dx_m = 80.0_f64;
        let mut bedrock = vec![0.0_f32; n * n];
        // Concave alpine cirque above ELA=4,100 m draining east onto a gentle 4,020 -> 3,990 m bench.
        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                if r == 0 || r + 1 == n || c == 0 || c + 1 == n {
                    bedrock[idx] = 2_400.0;
                } else {
                    let lat = libm::fabs(r as f64 - 24.0);
                    let wall_lift = 18.0 * (lat / 8.0) * (lat / 8.0);
                    let c_f = c as f64;
                    let long_profile = if c_f <= 16.0 {
                        4_900.0 - c_f * 50.0 // headwall 4,900 -> 4,100 m
                    } else if c_f <= 34.0 {
                        3_950.0 - (c_f - 16.0) * 4.0 // ablation glacial trough bench 3,950 -> 3,878 m
                    } else {
                        3_878.0 - (c_f - 34.0) * 80.0 // valley outlet step below snout
                    };
                    bedrock[idx] = (long_profile + wall_lift) as f32;
                }
            }
        }
        let mut sediment = vec![0.0_f32; n * n];
        let stats = run_paleo_glacial_pass(
            &mut bedrock,
            &mut sediment,
            n,
            n,
            &GlacialParams {
                dx_m,
                steps: 10,
                dt_yr: 2_000.0,
                ..GlacialParams::default()
            },
        );
        assert!(
            stats.rel_mass_error < 1e-6,
            "glacial mass drift {} >= 1e-6",
            stats.rel_mass_error
        );
        assert!(stats.glaciated_cells_peak > 20);
        assert!(stats.eroded_volume_m3 > 1.0e5);
        assert!(
            libm::fabs(stats.eroded_volume_m3 - stats.deposited_volume_m3)
                / stats.eroded_volume_m3.max(1.0)
                < 1e-9
        );

        let max_moraine_s = sediment.iter().copied().fold(0.0_f32, f32::max);
        assert!(
            max_moraine_s > 2.0,
            "expected > 2 m terminal/lateral moraine deposit, got {max_moraine_s:.2}"
        );

        // Priority-Flood should detect a closed depression behind the terminal moraine above 3,300 m.
        let total: Vec<f32> = bedrock
            .iter()
            .zip(sediment.iter())
            .map(|(&b, &s)| b + s)
            .collect();
        let pf = crate::hydrology::priority_flood_d8_lean(&total, n, n, None, false);
        let max_tarn_depth = pf
            .filled_h
            .iter()
            .zip(total.iter())
            .filter(|&(&fh, &th)| fh > 3_300.0 && th > 3_300.0)
            .map(|(&fh, &th)| fh - th)
            .fold(0.0_f32, f32::max);
        assert!(
            max_tarn_depth > 1.0,
            "expected > 1 m moraine-dammed overdeepening > 3300 m, got {max_tarn_depth:.2}"
        );
    }

    #[test]
    fn glacial_pass_forms_paramo_tarns_on_real_massif_after_fluvial_valleys() {
        let size = 512_u32;
        let n = size as usize;
        let cfg = crate::config::WorldV2Config::for_size(0x5756_3201, size);
        let tect = crate::tectonic::generate_uplift(&cfg).expect("s1 512");
        let clim = crate::climate::compute_climate(
            &tect.bedrock_h,
            n,
            n,
            None,
            &crate::climate::ClimateConfig::for_world(&cfg),
        )
        .expect("s7 512");
        let mut bedrock = tect.bedrock_h;
        let mut sediment = vec![0.0_f32; bedrock.len()];
        let uplift_phys: Vec<f32> = tect.uplift_u.iter().map(|&u| u * 1.0e-6).collect();
        let mut scratch = crate::erosion_fluvial::FluvialLeanScratch::default();
        let fp = crate::erosion_fluvial::FluvialParams {
            dx_m: f64::from(cfg.cell_size_m()),
            dt_yr: 1_000.0,
            g_deposition: 0.85,
            max_gs_iters: 64,
            gs_tol_m: 5.0e-4,
            ..crate::erosion_fluvial::FluvialParams::default()
        };
        for _ in 0..5 {
            let mut fg = crate::erosion_fluvial::FluvialGridLean {
                bedrock_h: &mut bedrock,
                sediment_s: &mut sediment,
                uplift_m_yr: &uplift_phys,
                k_erodibility: crate::erosion_fluvial::FluvialErodibility::LithologyLut {
                    lithology_id: &tect.lithology_id,
                    k_scale: 2.2e-5,
                },
                precip_m_yr: &clim.precip_m_yr,
                pet_m_yr: None,
                width: n,
                height: n,
                sea_level_m: Some(0.0),
            };
            crate::erosion_fluvial::erode_fluvial_step_f32_lean(&mut fg, &fp, Some(&mut scratch))
                .expect("s2 512");
        }
        let sed_before = sediment.clone();
        let total_before: Vec<f32> = bedrock
            .iter()
            .zip(sediment.iter())
            .map(|(&b, &s)| b + s)
            .collect();
        let pf_before =
            crate::hydrology::priority_flood_d8_lean(&total_before, n, n, Some(0.0), false);
        let pits_before = pf_before
            .filled_h
            .iter()
            .zip(total_before.iter())
            .filter(|&(&fh, &th)| th > 3_300.0 && fh - th > 0.5)
            .count();

        let stats = run_paleo_glacial_pass(
            &mut bedrock,
            &mut sediment,
            n,
            n,
            &GlacialParams {
                dx_m: f64::from(cfg.cell_size_m()),
                steps: 6,
                dt_yr: 2_000.0,
                ..GlacialParams::default()
            },
        );
        let total: Vec<f32> = bedrock
            .iter()
            .zip(sediment.iter())
            .map(|(&b, &s)| b + s)
            .collect();
        let pf_after = crate::hydrology::priority_flood_d8_lean(&total, n, n, Some(0.0), false);
        let pits_after = pf_after
            .filled_h
            .iter()
            .zip(total.iter())
            .filter(|&(&fh, &th)| th > 3_300.0 && fh - th > 0.5)
            .count();
        let max_glacial_moraine = sediment
            .iter()
            .zip(sed_before.iter())
            .map(|(&a, &b)| a - b)
            .fold(0.0_f32, f32::max);
        eprintln!(
            "512 S2->S2b stats: glaciated_peak={}, eroded_m3={:.2e}, max_glacial_moraine={max_glacial_moraine:.2}m, paramo_pits {pits_before} -> {pits_after}",
            stats.glaciated_cells_peak, stats.eroded_volume_m3
        );
        assert!(stats.rel_mass_error < 1e-6);
        assert!(
            pits_after > pits_before && max_glacial_moraine <= 65.0,
            "expected S2b to carve new high-paramo glacial pits (>3300 m) with moraines <=65m: before={pits_before}, after={pits_after}, max_moraine={max_glacial_moraine:.2}"
        );
    }
}
