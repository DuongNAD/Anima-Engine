//! Quantitative geomorphology (`G-SHAPE` §4.1 metrics `1..=7` and §4.2 shoreline `D_shore`).

use crate::anmw_v3::{compute_strahler_order, D8_NO_DIR};
use crate::hydrology::{
    priority_flood_d8_lean_into, route_hybrid_mfd_d8_lean_halves, HydrologyConfig, NO_DIR,
};
use crate::lakes_falls::{LakeBasinRecord, WaterfallRecord};
use crate::tectonic::LithologyClass;
use crate::WaterKind;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

const DX8: [isize; 8] = [1, 1, 0, -1, -1, -1, 0, 1];
const DZ8: [isize; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
const DIST8: [f64; 8] = [
    1.0,
    core::f64::consts::SQRT_2,
    1.0,
    core::f64::consts::SQRT_2,
    1.0,
    core::f64::consts::SQRT_2,
    1.0,
    core::f64::consts::SQRT_2,
];
const RAD_TO_DEG: f64 = 180.0 / core::f64::consts::PI;
const TAN_75_DEG: f32 = 3.732_051;

/// Input slices for [`evaluate_g_shape`].
pub struct GShapeInput<'a> {
    pub width: usize,
    pub height: usize,
    pub dx_m: f32,
    pub sea_level_m: f32,
    pub total_h_m: &'a [f32],
    pub precip_m_yr: Option<&'a [f32]>,
    pub pet_m_yr: Option<&'a [f32]>,
    pub discharge_q_m3_s: Option<&'a [f32]>,
    pub river_w_m: Option<&'a [f32]>,
    pub water_kind: Option<&'a [u8]>,
    pub glacier_h_m: Option<&'a [f32]>,
    pub lithology_id: Option<&'a [u8]>,
    pub basins: Option<&'a [LakeBasinRecord]>,
    pub waterfalls: Option<&'a [WaterfallRecord]>,
    pub reported_unmodeled_micro_pits: Option<usize>,
    pub reported_severed_humid_spillways: Option<usize>,
    pub reported_endorheic_land_fraction: Option<f32>,
    pub reported_endorheic_humid_fraction: Option<f32>,
    /// Minimum mean-annual discharge `Q >= q_min_m3_s` (`§4.1` Metric 1) excluding dry gullies.
    pub q_min_m3_s: f32,
    /// Minimum channel node sample floor for Hack's Law (`500` in §4.1 at regional scales).
    pub min_hack_samples: usize,
    /// When false (`--dem-input` without rainfall grids), skip rain-shadow vs windward subchecks.
    pub has_climate_fields: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrahlerHortonReport {
    pub passed: bool,
    pub omega_max: u8,
    pub cycle_count: usize,
    pub r_b: f64,
    pub r_l: f64,
    pub stream_counts_by_order: Vec<usize>,
    pub mean_length_m_by_order: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JunctionAngleReport {
    pub passed: bool,
    pub junction_count: usize,
    pub median_deg: f64,
    pub obtuse_fraction: f64,
    pub rain_shadow_mean_deg: Option<f64>,
    pub windward_humid_mean_deg: Option<f64>,
    pub rain_shadow_samples: usize,
    pub windward_humid_samples: usize,
    pub rain_shadow_lt_windward: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrainageDensityReport {
    pub passed: bool,
    pub global_km_per_km2: f64,
    pub dry_land_km2: f64,
    pub total_channel_km: f64,
    pub windward_mountain_km_per_km2: f64,
    pub plains_rain_shadow_km_per_km2: f64,
    pub dissection_ratio: f64,
    pub caprock_tile_count: usize,
    pub caprock_tile_cv: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HackLawReport {
    pub passed: bool,
    pub sample_count: usize,
    pub exponent_h: f64,
    pub prefactor_c_km: f64,
    pub r_squared: f64,
    pub max_path_km: f64,
    pub median_mainstem_sinuosity: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValleyFlintReport {
    pub passed: bool,
    pub upper_samples: usize,
    pub downstream_samples: usize,
    pub b_up_median: f64,
    pub vf_up_median: f64,
    pub b_down_median: f64,
    pub vf_down_median: f64,
    pub vf_ratio_down_over_up: f64,
    pub flint_bins: usize,
    pub flint_concavity_theta: f64,
    pub flint_r_squared: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndorheicAuditReport {
    pub passed: bool,
    pub unmodeled_micro_pits: usize,
    pub raw_non_water_land_pits: usize,
    pub severed_humid_spillways: usize,
    pub endorheic_land_fraction: f64,
    pub endorheic_humid_fraction: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HypsometrySpikeReport {
    pub passed: bool,
    pub hi_global: f64,
    pub catchment_count: usize,
    pub catchment_hi_median: f64,
    pub non_cliff_needle_spikes: usize,
    pub hillslope_p999_deg: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShorelineFractalReport {
    pub passed: bool,
    pub coastline_cells: usize,
    pub box_sizes: Vec<usize>,
    pub d_shore: Option<f64>,
    pub r_squared: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GShapeReport {
    pub passed: bool,
    pub metric_1_strahler_horton: StrahlerHortonReport,
    pub metric_2_junction_angles: JunctionAngleReport,
    pub metric_3_drainage_density: DrainageDensityReport,
    pub metric_4_hack_law: HackLawReport,
    pub metric_5_valleys_flint: ValleyFlintReport,
    pub metric_6_endorheic_spillways: EndorheicAuditReport,
    pub metric_7_hypsometry_spikes: HypsometrySpikeReport,
    pub metric_4_2_shoreline_fractal: ShorelineFractalReport,
}

/// Reusable hydrologic summary returned alongside [`GShapeReport`] so `G-COMP v2` (`C4`)
/// does not duplicate Priority-Flood, MFD->D8 routing, or Strahler ordering at `4096^2`.
pub struct GShapeHydroOutput {
    pub report: GShapeReport,
    pub d8_dir: Vec<u8>,
    pub is_channel: Vec<bool>,
    pub strahler: Vec<u8>,
    pub mean_width_by_order: Vec<(u8, usize, f64)>,
}

#[inline]
fn step_cell(idx: usize, dir: u8, width: usize, height: usize) -> Option<usize> {
    if dir >= 8 {
        return None;
    }
    let r = (idx / width) as isize + DZ8[dir as usize];
    let c = (idx % width) as isize + DX8[dir as usize];
    if r < 0 || c < 0 || r >= height as isize || c >= width as isize {
        return None;
    }
    Some((r as usize) * width + (c as usize))
}

#[inline]
fn ols_fit(xs: &[f64], ys: &[f64]) -> Option<(f64, f64, f64)> {
    let n = xs.len().min(ys.len());
    if n < 2 {
        return None;
    }
    let nf = n as f64;
    let mx = xs[..n].iter().sum::<f64>() / nf;
    let my = ys[..n].iter().sum::<f64>() / nf;
    let mut sxx = 0.0_f64;
    let mut sxy = 0.0_f64;
    let mut syy = 0.0_f64;
    for (&x, &y) in xs[..n].iter().zip(ys[..n].iter()) {
        let dx = x - mx;
        let dy = y - my;
        sxx += dx * dx;
        sxy += dx * dy;
        syy += dy * dy;
    }
    if sxx <= 1e-18 {
        return None;
    }
    let slope = sxy / sxx;
    let intercept = my - slope * mx;
    let r2 = if syy <= 1e-18 {
        1.0
    } else {
        let corr = sxy / libm::sqrt(sxx * syy);
        (corr * corr).clamp(0.0, 1.0)
    };
    Some((slope, intercept, r2))
}

#[inline]
fn median_f64(vals: &mut [f64]) -> Option<f64> {
    if vals.is_empty() {
        return None;
    }
    vals.sort_by(f64::total_cmp);
    let mid = vals.len() / 2;
    if vals.len() % 2 == 1 {
        Some(vals[mid])
    } else {
        Some(0.5 * (vals[mid - 1] + vals[mid]))
    }
}

#[inline]
fn is_water_mask_cell(
    wk_opt: Option<&[u8]>,
    h_m: f32,
    sea_level_m: f32,
    idx: usize,
) -> (bool, bool) {
    match wk_opt {
        Some(wk) => {
            let w = wk[idx];
            let ocean = w == WaterKind::Ocean as u8
                || (w == WaterKind::DeltaTurbid as u8 && h_m <= sea_level_m + 0.1)
                || h_m <= sea_level_m;
            let lake = w == WaterKind::FreshwaterLake as u8 || w == WaterKind::EndorheicLake as u8;
            (ocean, lake)
        }
        None => (h_m <= sea_level_m, false),
    }
}

/// Counts interior subaerial non-water cells on the un-flooded DEM strictly lower than all 8
/// neighbors. Catching micro-pits here directly guards against cratered noise DEMs (`Metric 6`).
fn count_raw_non_water_land_pits(
    total_h_m: &[f32],
    water_kind: Option<&[u8]>,
    width: usize,
    height: usize,
    sea_level_m: f32,
) -> usize {
    let mut pits = 0_usize;
    for r in 1..height.saturating_sub(1) {
        let row = r * width;
        for c in 1..width.saturating_sub(1) {
            let idx = row + c;
            let h = total_h_m[idx];
            let (is_ocean, is_lake) = is_water_mask_cell(water_kind, h, sea_level_m, idx);
            if is_ocean || is_lake {
                continue;
            }
            if let Some(wk) = water_kind {
                if wk[idx] != WaterKind::None as u8 {
                    continue;
                }
            }
            let mut has_lower_or_equal = false;
            for dir in 0..8 {
                let nr = (r as isize + DZ8[dir]) as usize;
                let nc = (c as isize + DX8[dir]) as usize;
                if total_h_m[nr * width + nc] <= h {
                    has_lower_or_equal = true;
                    break;
                }
            }
            if !has_lower_or_equal {
                pits += 1;
            }
        }
    }
    pits
}

/// Evaluates `G-SHAPE` metrics `1..=7` and shoreline fractal dimension `D_shore` (§4.1–§4.2).
pub fn evaluate_g_shape(input: &GShapeInput<'_>) -> GShapeHydroOutput {
    let w = input.width;
    let h = input.height;
    let n = w * h;
    let dx = f64::from(input.dx_m.max(1e-3));

    assert_eq!(input.total_h_m.len(), n, "total_h_m length mismatch");

    let mut filled_h = input.total_h_m.to_vec();
    let mut d8_dir = vec![NO_DIR; n];
    let mut dummy_order = Vec::new();
    let (raw_pit_before, _pit_after) = priority_flood_d8_lean_into(
        &mut filled_h,
        &mut d8_dir,
        &mut dummy_order,
        w,
        h,
        Some(input.sea_level_m),
        false,
    );

    let default_p;
    let default_pet;
    let precip: &[f32] = match input.precip_m_yr {
        Some(p) => p,
        None => {
            default_p = vec![1.2_f32; n];
            &default_p
        }
    };
    let pet: &[f32] = match input.pet_m_yr {
        Some(e) => e,
        None => {
            default_pet = vec![0.4_f32; n];
            &default_pet
        }
    };

    let hydro_cfg = HydrologyConfig {
        dx_m: input.dx_m,
        sea_level_m: Some(input.sea_level_m),
        ..HydrologyConfig::default()
    };
    let hydro = route_hybrid_mfd_d8_lean_halves(&filled_h, &d8_dir, w, h, precip, pet, &hydro_cfg);
    let flow_acc_m2 = |i: usize| -> f64 {
        let half = n / 2;
        if i < half {
            hydro.flow_acc_lo[i]
        } else {
            hydro.flow_acc_hi[i - half]
        }
    };
    let eff_river_w: &[f32] = input.river_w_m.unwrap_or(&hydro.river_w_m);
    let eff_q: &[f32] = input.discharge_q_m3_s.unwrap_or(&hydro.discharge_q_m3_s);

    // Build continuous D8 channel DAG mask off Ocean/Lake water bodies (`A S^2 >= tau_chan & Q >= Q_min`).
    let mut is_channel = vec![false; n];
    for i in 0..n {
        let (is_ocean, is_lake) =
            is_water_mask_cell(input.water_kind, input.total_h_m[i], input.sea_level_m, i);
        if is_ocean || is_lake {
            continue;
        }
        if (hydro.is_channel[i] && eff_q[i] >= input.q_min_m3_s)
            || (input.river_w_m.is_some() && eff_river_w[i] >= 0.5)
        {
            is_channel[i] = true;
        }
    }
    // Bridge downstream channel steps across flat alluvial benches so D8 stems remain unbroken.
    let mut seeds = Vec::new();
    for (i, &c) in is_channel.iter().enumerate() {
        if c {
            seeds.push(i as u32);
        }
    }
    for s_u32 in seeds {
        let mut cur = s_u32 as usize;
        for _ in 0..n {
            let Some(nxt) = step_cell(cur, d8_dir[cur], w, h) else {
                break;
            };
            if nxt == cur {
                break;
            }
            let (nxt_ocean, nxt_lake) = is_water_mask_cell(
                input.water_kind,
                input.total_h_m[nxt],
                input.sea_level_m,
                nxt,
            );
            if nxt_ocean || nxt_lake || is_channel[nxt] {
                break;
            }
            is_channel[nxt] = true;
            cur = nxt;
        }
    }

    let strahler = compute_strahler_order(&d8_dir, &is_channel, w, h);
    let m1 = eval_metric_1_strahler_horton(&d8_dir, &is_channel, &strahler, w, h, dx);

    let m2 = eval_metric_2_junction_angles(
        &d8_dir,
        &is_channel,
        &strahler,
        flow_acc_m2,
        precip,
        pet,
        input.has_climate_fields,
        w,
        h,
        dx,
    );

    let m3 = eval_metric_3_drainage_density(input, &d8_dir, &is_channel, precip, w, h, dx);

    let m4 = eval_metric_4_hack_law(
        &d8_dir,
        &is_channel,
        &strahler,
        flow_acc_m2,
        w,
        h,
        dx,
        input.min_hack_samples,
    );

    let m5 = eval_metric_5_valleys_flint(
        input,
        &filled_h,
        &d8_dir,
        &is_channel,
        &strahler,
        flow_acc_m2,
        w,
        h,
        dx,
    );

    let m6 = eval_metric_6_endorheic_spillways(input, raw_pit_before, w, h);

    let m7 = eval_metric_7_hypsometry_spikes(input, &d8_dir, &is_channel, &strahler, w, h, dx);

    let m42 = eval_shoreline_fractal(input.total_h_m, input.water_kind, w, h, input.sea_level_m);

    // Mean river width per Strahler order for G-COMP v2 C4 progression check.
    let mut sum_w_by_ord = vec![0.0_f64; usize::from(m1.omega_max) + 1];
    let mut cnt_w_by_ord = vec![0_usize; usize::from(m1.omega_max) + 1];
    for i in 0..n {
        if !is_channel[i] {
            continue;
        }
        let o = strahler[i] as usize;
        if o > 0 && o < sum_w_by_ord.len() && eff_river_w[i] > 0.0 {
            sum_w_by_ord[o] += f64::from(eff_river_w[i]);
            cnt_w_by_ord[o] += 1;
        }
    }
    let mut mean_width_by_order = Vec::new();
    for ord in 1..sum_w_by_ord.len() {
        let cnt = cnt_w_by_ord[ord];
        let mean_w = if cnt > 0 {
            sum_w_by_ord[ord] / cnt as f64
        } else {
            0.0
        };
        mean_width_by_order.push((ord as u8, cnt, mean_w));
    }

    let passed = m1.passed
        && m2.passed
        && m3.passed
        && m4.passed
        && m5.passed
        && m6.passed
        && m7.passed
        && m42.passed;

    GShapeHydroOutput {
        report: GShapeReport {
            passed,
            metric_1_strahler_horton: m1,
            metric_2_junction_angles: m2,
            metric_3_drainage_density: m3,
            metric_4_hack_law: m4,
            metric_5_valleys_flint: m5,
            metric_6_endorheic_spillways: m6,
            metric_7_hypsometry_spikes: m7,
            metric_4_2_shoreline_fractal: m42,
        },
        d8_dir,
        is_channel,
        strahler,
        mean_width_by_order,
    }
}

fn eval_metric_1_strahler_horton(
    d8_dir: &[u8],
    is_channel: &[bool],
    strahler: &[u8],
    w: usize,
    h: usize,
    dx: f64,
) -> StrahlerHortonReport {
    let n = w * h;
    let mut in_deg = vec![0_u16; n];
    let mut chan_total = 0_usize;
    let mut omega_max = 0_u8;

    for i in 0..n {
        if !is_channel[i] {
            continue;
        }
        chan_total += 1;
        omega_max = omega_max.max(strahler[i]);
        if let Some(nxt) = step_cell(i, d8_dir[i], w, h) {
            if nxt != i && is_channel[nxt] {
                in_deg[nxt] = in_deg[nxt].saturating_add(1);
            }
        }
    }

    let mut q = VecDeque::new();
    for (i, &deg) in in_deg.iter().enumerate() {
        if is_channel[i] && deg == 0 {
            q.push_back(i as u32);
        }
    }
    let mut visited = 0_usize;
    while let Some(u_u32) = q.pop_front() {
        visited += 1;
        let u = u_u32 as usize;
        if let Some(v) = step_cell(u, d8_dir[u], w, h) {
            if v != u && is_channel[v] && in_deg[v] > 0 {
                in_deg[v] -= 1;
                if in_deg[v] == 0 {
                    q.push_back(v as u32);
                }
            }
        }
    }
    let cycle_count = chan_total.saturating_sub(visited);

    let max_ord = usize::from(omega_max);
    let mut seg_counts = vec![0_usize; max_ord + 1];
    let mut len_sums = vec![0.0_f64; max_ord + 1];
    for i in 0..n {
        if !is_channel[i] || strahler[i] == 0 {
            continue;
        }
        let ord = strahler[i];
        let ord_u = usize::from(ord);
        let step_l = if d8_dir[i] < 8 {
            dx * DIST8[usize::from(d8_dir[i])]
        } else {
            dx
        };
        len_sums[ord_u] += step_l;
        let terminates_seg = match step_cell(i, d8_dir[i], w, h) {
            Some(nxt) if nxt != i && is_channel[nxt] => strahler[nxt] != ord,
            _ => true,
        };
        if terminates_seg {
            seg_counts[ord_u] += 1;
        }
    }

    let mut mean_len = vec![0.0_f64; max_ord + 1];
    for ord in 1..=max_ord {
        if seg_counts[ord] > 0 {
            mean_len[ord] = len_sums[ord] / seg_counts[ord] as f64;
        }
    }

    let fit_hi = max_ord.saturating_sub(1);
    let mut xs = Vec::new();
    let mut ys_n = Vec::new();
    let mut ys_l = Vec::new();
    for ord in 1..=fit_hi {
        if seg_counts[ord] > 0 && mean_len[ord] > 0.0 {
            xs.push(ord as f64);
            ys_n.push(libm::log(seg_counts[ord] as f64));
            ys_l.push(libm::log(mean_len[ord]));
        }
    }
    let r_b = ols_fit(&xs, &ys_n)
        .map(|(slope, _, _)| libm::exp(-slope))
        .unwrap_or(0.0);
    let r_l = ols_fit(&xs, &ys_l)
        .map(|(slope, _, _)| libm::exp(slope))
        .unwrap_or(0.0);

    let passed = omega_max >= 4
        && cycle_count == 0
        && (3.0..=5.0).contains(&r_b)
        && (1.5..=3.0).contains(&r_l);

    StrahlerHortonReport {
        passed,
        omega_max,
        cycle_count,
        r_b,
        r_l,
        stream_counts_by_order: seg_counts,
        mean_length_m_by_order: mean_len,
    }
}

#[allow(clippy::too_many_arguments)]
fn eval_metric_2_junction_angles<F: Fn(usize) -> f64>(
    d8_dir: &[u8],
    is_channel: &[bool],
    strahler: &[u8],
    flow_acc_m2: F,
    precip: &[f32],
    pet: &[f32],
    has_climate_fields: bool,
    w: usize,
    h: usize,
    dx: f64,
) -> JunctionAngleReport {
    let n = w * h;
    // Collect incoming (order >= 2) neighbors for every channel node.
    let mut ord2_in_count = vec![0_u8; n];
    for u in 0..n {
        if !is_channel[u] || strahler[u] < 2 {
            continue;
        }
        if let Some(v) = step_cell(u, d8_dir[u], w, h) {
            if v != u && is_channel[v] && strahler[v] >= 2 {
                ord2_in_count[v] = ord2_in_count[v].saturating_add(1);
            }
        }
    }

    // Build reverse D8 main-upstream index only for cells with incoming channel edges (`0` extra
    // recursion; one O(n) pass finds max-area upstream order>=2 predecessor per cell).
    let mut best_up = vec![u32::MAX; n];
    let mut best_up_acc = vec![-1.0_f64; n];
    for u in 0..n {
        if !is_channel[u] || strahler[u] < 2 {
            continue;
        }
        if let Some(v) = step_cell(u, d8_dir[u], w, h) {
            if v != u && is_channel[v] && strahler[v] >= 2 {
                let acc = flow_acc_m2(u);
                if acc > best_up_acc[v] {
                    best_up_acc[v] = acc;
                    best_up[v] = u as u32;
                }
            }
        }
    }

    let target_reach_m = if dx > 250.0 {
        dx
    } else {
        250.0_f64.clamp(150.0, 400.0)
    };
    let trace_upstream_vec = |junction: usize, first_up: usize| -> Option<(f64, f64)> {
        let mut cur = first_up;
        let mut dist = dx
            * libm::hypot(
                ((first_up % w) as isize - (junction % w) as isize) as f64,
                ((first_up / w) as isize - (junction / w) as isize) as f64,
            );
        let mut steps = 1_usize;
        while dist < target_reach_m && steps < 64 {
            let nxt = best_up[cur];
            if nxt == u32::MAX {
                break;
            }
            let nu = nxt as usize;
            if nu == cur || nu == junction {
                break;
            }
            let step_d = dx
                * libm::hypot(
                    ((nu % w) as isize - (cur % w) as isize) as f64,
                    ((nu / w) as isize - (cur / w) as isize) as f64,
                );
            if dist + step_d > 400.0 && dist >= 150.0 {
                break;
            }
            dist += step_d;
            cur = nu;
            steps += 1;
        }
        if dist < 150.0 && steps < 1 {
            return None;
        }
        let vx = ((cur % w) as isize - (junction % w) as isize) as f64;
        let vz = ((cur / w) as isize - (junction / w) as isize) as f64;
        let len = libm::hypot(vx, vz);
        if len <= 1e-9 {
            None
        } else {
            Some((vx / len, vz / len))
        }
    };

    let mut angles = Vec::new();
    let mut obtuse = 0_usize;
    let mut rain_shadow_sum = 0.0_f64;
    let mut rain_shadow_n = 0_usize;
    let mut windward_sum = 0.0_f64;
    let mut windward_n = 0_usize;

    for v in 0..n {
        if ord2_in_count[v] < 2 {
            continue;
        }
        let r = v / w;
        let c = v % w;
        let mut incoming: Vec<(f64, usize)> = Vec::new();
        for dir in 0..8 {
            let nr = r as isize + DZ8[dir];
            let nc = c as isize + DX8[dir];
            if nr < 0 || nc < 0 || nr >= h as isize || nc >= w as isize {
                continue;
            }
            let u = (nr as usize) * w + (nc as usize);
            if is_channel[u] && strahler[u] >= 2 && step_cell(u, d8_dir[u], w, h) == Some(v) {
                incoming.push((flow_acc_m2(u), u));
            }
        }
        if incoming.len() < 2 {
            continue;
        }
        incoming.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        // Measure junction angle between the two dominant tributaries meeting at `v`.
        let u1 = incoming[0].1;
        let u2 = incoming[1].1;
        let (Some(t1), Some(t2)) = (trace_upstream_vec(v, u1), trace_upstream_vec(v, u2)) else {
            continue;
        };
        let dot = (t1.0 * t2.0 + t1.1 * t2.1).clamp(-1.0, 1.0);
        let deg = libm::acos(dot) * RAD_TO_DEG;
        angles.push(deg);
        if deg > 100.0 {
            obtuse += 1;
        }
        if has_climate_fields {
            let p = f64::from(precip[v]);
            let pet_v = f64::from(pet[v].max(0.05));
            if p / pet_v < 0.85 || p < 0.85 {
                rain_shadow_sum += deg;
                rain_shadow_n += 1;
            } else if p >= 1.35 {
                windward_sum += deg;
                windward_n += 1;
            }
        }
    }

    let junction_count = angles.len();
    let median_deg = median_f64(&mut angles).unwrap_or(0.0);
    let obtuse_fraction = if junction_count > 0 {
        obtuse as f64 / junction_count as f64
    } else {
        1.0
    };
    let rain_shadow_mean_deg = (rain_shadow_n >= 3).then(|| rain_shadow_sum / rain_shadow_n as f64);
    let windward_humid_mean_deg = (windward_n >= 3).then(|| windward_sum / windward_n as f64);
    let rain_shadow_lt_windward = match (rain_shadow_mean_deg, windward_humid_mean_deg) {
        (Some(rs), Some(wh)) => Some(rs < wh),
        _ => None,
    };

    let climate_sub_ok = if has_climate_fields {
        rain_shadow_lt_windward == Some(true)
    } else {
        true
    };
    let passed = junction_count >= 5
        && (40.0..=78.0).contains(&median_deg)
        && obtuse_fraction < 0.10
        && climate_sub_ok;

    JunctionAngleReport {
        passed,
        junction_count,
        median_deg,
        obtuse_fraction,
        rain_shadow_mean_deg,
        windward_humid_mean_deg,
        rain_shadow_samples: rain_shadow_n,
        windward_humid_samples: windward_n,
        rain_shadow_lt_windward,
    }
}

#[allow(clippy::too_many_arguments)]
fn eval_metric_3_drainage_density(
    input: &GShapeInput<'_>,
    d8_dir: &[u8],
    is_channel: &[bool],
    precip: &[f32],
    w: usize,
    h: usize,
    dx: f64,
) -> DrainageDensityReport {
    let n = w * h;
    let cell_km2 = (dx * dx) / 1_000_000.0;
    let mut dry_land_cells = 0_usize;
    let mut total_chan_km = 0.0_f64;

    let mut windward_cells = 0_usize;
    let mut windward_chan_km = 0.0_f64;
    let mut plains_cells = 0_usize;
    let mut plains_chan_km = 0.0_f64;

    for r in 0..h {
        let row = r * w;
        for c in 0..w {
            let i = row + c;
            let elev = input.total_h_m[i];
            let (is_ocean, is_lake) =
                is_water_mask_cell(input.water_kind, elev, input.sea_level_m, i);
            let is_glacier = input.glacier_h_m.is_some_and(|g| g[i] > 0.01);
            if is_ocean || is_lake || is_glacier || elev <= input.sea_level_m {
                continue;
            }
            dry_land_cells += 1;
            let seg_km = if is_channel[i] {
                let d = d8_dir[i];
                let mult = if d < 8 { DIST8[usize::from(d)] } else { 1.0 };
                (dx * mult) / 1000.0
            } else {
                0.0
            };
            total_chan_km += seg_km;

            // Compute local central-diff slope for mountain/alluvial separation when climate absent.
            let c_l = c.saturating_sub(1);
            let c_r = (c + 1).min(w - 1);
            let r_u = r.saturating_sub(1);
            let r_d = (r + 1).min(h - 1);
            let gx = (f64::from(input.total_h_m[row + c_r])
                - f64::from(input.total_h_m[row + c_l]))
                / (dx * (c_r - c_l).max(1) as f64);
            let gz = (f64::from(input.total_h_m[r_d * w + c])
                - f64::from(input.total_h_m[r_u * w + c]))
                / (dx * (r_d - r_u).max(1) as f64);
            let slope = libm::hypot(gx, gz);

            if input.has_climate_fields {
                let p = precip[i];
                if (p >= 1.3 && elev > 150.0) || (slope >= 0.08 && p >= 0.9) {
                    windward_cells += 1;
                    windward_chan_km += seg_km;
                } else if p < 0.8 || (slope < 0.045 && elev < 600.0) {
                    plains_cells += 1;
                    plains_chan_km += seg_km;
                }
            } else if slope >= 0.08 {
                windward_cells += 1;
                windward_chan_km += seg_km;
            } else if slope < 0.045 {
                plains_cells += 1;
                plains_chan_km += seg_km;
            }
        }
    }

    let dry_land_km2 = dry_land_cells as f64 * cell_km2;
    let global_dd = if dry_land_km2 > 0.0 {
        total_chan_km / dry_land_km2
    } else {
        0.0
    };
    let windward_dd = if windward_cells > 0 {
        windward_chan_km / (windward_cells as f64 * cell_km2)
    } else {
        0.0
    };
    let plains_dd = if plains_cells > 0 {
        plains_chan_km / (plains_cells as f64 * cell_km2)
    } else {
        0.0
    };
    let dissection_ratio = if plains_dd > 1e-6 {
        windward_dd / plains_dd
    } else {
        0.0
    };

    // Per-`2 km x 2 km` tile D_d CV on uniform sandstone caprock plateaus (`§4.1` Metric 3).
    let tile_px = ((2000.0 / dx).round() as usize).clamp(4, w.min(h));
    let mut caprock_dds = Vec::new();
    if let Some(lith) = input.lithology_id {
        for tr in 0..(h / tile_px) {
            for tc in 0..(w / tile_px) {
                let mut dry_t = 0_usize;
                let mut cap_t = 0_usize;
                let mut len_t = 0.0_f64;
                for dr in 0..tile_px {
                    let r = tr * tile_px + dr;
                    let row = r * w;
                    for dc in 0..tile_px {
                        let c = tc * tile_px + dc;
                        let i = row + c;
                        let h_m = input.total_h_m[i];
                        let (oc, lk) =
                            is_water_mask_cell(input.water_kind, h_m, input.sea_level_m, i);
                        let gl = input.glacier_h_m.is_some_and(|g| g[i] > 0.01);
                        if oc || lk || gl || h_m <= input.sea_level_m {
                            continue;
                        }
                        dry_t += 1;
                        if lith[i] == LithologyClass::SandstoneCaprock as u8 {
                            cap_t += 1;
                        }
                        if is_channel[i] {
                            let d = d8_dir[i];
                            let mult = if d < 8 { DIST8[usize::from(d)] } else { 1.0 };
                            len_t += (dx * mult) / 1000.0;
                        }
                    }
                }
                let full_tile = tile_px * tile_px;
                if dry_t * 5 >= full_tile * 4 && cap_t * 5 >= dry_t * 4 {
                    caprock_dds.push(len_t / (dry_t as f64 * cell_km2));
                }
            }
        }
    }
    let caprock_tile_cv = if caprock_dds.len() >= 2 {
        let m = caprock_dds.iter().sum::<f64>() / caprock_dds.len() as f64;
        if m > 1e-6 {
            let var = caprock_dds.iter().map(|&v| (v - m) * (v - m)).sum::<f64>()
                / caprock_dds.len() as f64;
            Some(libm::sqrt(var) / m)
        } else {
            Some(0.0)
        }
    } else {
        None
    };

    let caprock_ok = caprock_tile_cv.is_none_or(|cv| cv <= 0.45);
    let passed = (0.8..=3.5).contains(&global_dd) && dissection_ratio >= 1.5 && caprock_ok;
    let _ = n;

    DrainageDensityReport {
        passed,
        global_km_per_km2: global_dd,
        dry_land_km2,
        total_channel_km: total_chan_km,
        windward_mountain_km_per_km2: windward_dd,
        plains_rain_shadow_km_per_km2: plains_dd,
        dissection_ratio,
        caprock_tile_count: caprock_dds.len(),
        caprock_tile_cv,
    }
}

#[allow(clippy::too_many_arguments)]
fn eval_metric_4_hack_law<F: Fn(usize) -> f64>(
    d8_dir: &[u8],
    is_channel: &[bool],
    strahler: &[u8],
    flow_acc_m2: F,
    w: usize,
    h: usize,
    dx: f64,
    min_samples: usize,
) -> HackLawReport {
    let n = w * h;
    // Kahn topological sweep along the channel DAG to compute exact longest upstream channel path
    // and the headwater cell index of that longest path (for trunk sinuosity).
    let mut in_deg = vec![0_u16; n];
    for i in 0..n {
        if !is_channel[i] {
            continue;
        }
        if let Some(v) = step_cell(i, d8_dir[i], w, h) {
            if v != i && is_channel[v] {
                in_deg[v] = in_deg[v].saturating_add(1);
            }
        }
    }
    let mut longest_m = vec![0.0_f32; n];
    let mut head_cell = vec![u32::MAX; n];
    let mut q = VecDeque::new();
    for i in 0..n {
        if is_channel[i] && in_deg[i] == 0 {
            longest_m[i] = dx as f32;
            head_cell[i] = i as u32;
            q.push_back(i as u32);
        }
    }
    while let Some(u_u32) = q.pop_front() {
        let u = u_u32 as usize;
        let len_u = longest_m[u];
        let src_u = head_cell[u];
        if let Some(v) = step_cell(u, d8_dir[u], w, h) {
            if v != u && is_channel[v] {
                let d = d8_dir[u];
                let step_l = (dx * DIST8[usize::from(d)]) as f32;
                let cand = len_u + step_l;
                if cand > longest_m[v] {
                    longest_m[v] = cand;
                    head_cell[v] = src_u;
                }
                if in_deg[v] > 0 {
                    in_deg[v] -= 1;
                    if in_deg[v] == 0 {
                        q.push_back(v as u32);
                    }
                }
            }
        }
    }
    drop(in_deg);

    let mut eligible = Vec::new();
    let mut max_path_km = 0.0_f64;
    let mut sinuosities = Vec::new();
    for i in 0..n {
        if !is_channel[i] || strahler[i] < 2 {
            continue;
        }
        let a_km2 = flow_acc_m2(i) / 1_000_000.0;
        let l_km = f64::from(longest_m[i]) / 1000.0;
        if a_km2 > 0.02 && a_km2 <= 300.0 && l_km > 0.05 {
            max_path_km = max_path_km.max(l_km);
            eligible.push(i as u32);
            if strahler[i] >= 4 && l_km >= 3.0 {
                let hc = head_cell[i];
                if hc != u32::MAX {
                    let hu = hc as usize;
                    let dc = ((i % w) as isize - (hu % w) as isize) as f64;
                    let dr = ((i / w) as isize - (hu / w) as isize) as f64;
                    let straight_km = (dx * libm::hypot(dc, dr)) / 1000.0;
                    if straight_km >= 1.0 {
                        sinuosities.push(l_km / straight_km);
                    }
                }
            }
        }
    }

    let max_sample = 2_000_usize;
    let stride = (eligible.len() / max_sample).max(1);
    let mut xs = Vec::with_capacity(eligible.len().min(max_sample));
    let mut ys = Vec::with_capacity(eligible.len().min(max_sample));
    for &idx_u32 in eligible.iter().step_by(stride).take(max_sample) {
        let i = idx_u32 as usize;
        let a_km2 = flow_acc_m2(i) / 1_000_000.0;
        let l_km = f64::from(longest_m[i]) / 1000.0;
        xs.push(libm::log(a_km2));
        ys.push(libm::log(l_km));
    }

    let sample_count = xs.len();
    let (h_exp, c_km, r2) = match ols_fit(&xs, &ys) {
        Some((slope, intercept, r2)) => (slope, libm::exp(intercept), r2),
        None => (0.0, 0.0, 0.0),
    };
    let median_sinuosity = median_f64(&mut sinuosities).unwrap_or(1.0);
    let passed = sample_count >= min_samples
        && (0.52..=0.62).contains(&h_exp)
        && (1.0..=2.0).contains(&c_km)
        && r2 >= 0.85;

    HackLawReport {
        passed,
        sample_count,
        exponent_h: h_exp,
        prefactor_c_km: c_km,
        r_squared: r2,
        max_path_km,
        median_mainstem_sinuosity: median_sinuosity,
    }
}

#[inline]
fn bilinear_h(z: &[f32], w: usize, h: usize, c_f: f64, r_f: f64) -> Option<f64> {
    if c_f < 0.0 || r_f < 0.0 || c_f > (w - 1) as f64 || r_f > (h - 1) as f64 {
        return None;
    }
    let c0 = libm::floor(c_f) as usize;
    let r0 = libm::floor(r_f) as usize;
    let c1 = (c0 + 1).min(w - 1);
    let r1 = (r0 + 1).min(h - 1);
    let tc = c_f - c0 as f64;
    let tr = r_f - r0 as f64;
    let z00 = f64::from(z[r0 * w + c0]);
    let z10 = f64::from(z[r0 * w + c1]);
    let z01 = f64::from(z[r1 * w + c0]);
    let z11 = f64::from(z[r1 * w + c1]);
    Some((1.0 - tr) * ((1.0 - tc) * z00 + tc * z10) + tr * ((1.0 - tc) * z01 + tc * z11))
}

/// Fits power valley wall exponent `b` (`z(r) = z0 + k |r|^b`) and Bull & McFadden `V_f`.
#[allow(clippy::too_many_arguments)]
fn sample_valley_transect(
    z: &[f32],
    w: usize,
    h: usize,
    dx: f64,
    center: usize,
    dir: u8,
    sea_level_m: f32,
    max_half_m: f64,
) -> Option<(f64, f64)> {
    if dir >= 8 {
        return None;
    }
    let r0 = (center / w) as f64;
    let c0 = (center % w) as f64;
    let z_c = f64::from(z[center]);
    let ux = DX8[usize::from(dir)] as f64;
    let uz = DZ8[usize::from(dir)] as f64;
    let flow_len = libm::hypot(ux, uz).max(1e-6);
    // Perpendicular unit vector in grid coordinates.
    let px = -uz / flow_len;
    let pz = ux / flow_len;

    let step_m = dx;
    let max_steps = ((max_half_m / step_m).round() as usize).clamp(4, 150);

    let march_side = |sign: f64| -> (f64, Vec<(f64, f64)>) {
        let mut pts = Vec::new();
        let mut crest_h = z_c;
        let mut prev_h = z_c;
        for s in 1..=max_steps {
            let dist_m = s as f64 * step_m;
            let sc = c0 + sign * px * (s as f64);
            let sr = r0 + sign * pz * (s as f64);
            let Some(val) = bilinear_h(z, w, h, sc, sr) else {
                break;
            };
            if val <= f64::from(sea_level_m) {
                break;
            }
            if val > crest_h {
                crest_h = val;
            }
            pts.push((dist_m, val));
            // Stop once we pass the flanking ridge divide (`> 20%` relief drop after rising).
            if val + 1.5 < crest_h && prev_h < crest_h && s >= 4 {
                break;
            }
            prev_h = val;
        }
        (crest_h, pts)
    };

    let (crest_l, pts_l) = march_side(-1.0);
    let (crest_r, pts_r) = march_side(1.0);
    let hl = crest_l - z_c;
    let hr = crest_r - z_c;
    if hl < 4.0 || hr < 4.0 || pts_l.len() < 3 || pts_r.len() < 3 {
        return None;
    }

    let mean_wall_h = 0.5 * (hl + hr);
    let floor_thresh = 1.0_f64.max(0.08 * mean_wall_h);
    let left_floor_w = pts_l
        .iter()
        .take_while(|&&(_, val)| val - z_c <= floor_thresh)
        .last()
        .map_or(0.5 * step_m, |&(d, _)| d);
    let right_floor_w = pts_r
        .iter()
        .take_while(|&&(_, val)| val - z_c <= floor_thresh)
        .last()
        .map_or(0.5 * step_m, |&(d, _)| d);
    let v_fw = left_floor_w + right_floor_w;
    let vf = (2.0 * v_fw) / (hl + hr);

    let mut ln_r = Vec::new();
    let mut ln_dz = Vec::new();
    for pts in [&pts_l, &pts_r] {
        for &(dist_m, val) in pts {
            let dz = val - z_c;
            if dz > 0.25 {
                ln_r.push(libm::log(dist_m));
                ln_dz.push(libm::log(dz));
            }
        }
    }
    let (b_exp, _, _) = ols_fit(&ln_r, &ln_dz)?;
    if !b_exp.is_finite() || !vf.is_finite() || b_exp <= 0.1 {
        return None;
    }
    Some((b_exp, vf))
}

#[allow(clippy::too_many_arguments)]
fn eval_metric_5_valleys_flint<F: Fn(usize) -> f64>(
    input: &GShapeInput<'_>,
    filled_h: &[f32],
    d8_dir: &[u8],
    is_channel: &[bool],
    strahler: &[u8],
    flow_acc_m2: F,
    w: usize,
    h: usize,
    dx: f64,
) -> ValleyFlintReport {
    let n = w * h;
    let mut wf_mark = vec![false; n];
    if let Some(wfs) = input.waterfalls {
        for wf in wfs {
            let li = wf.lip_cell as usize;
            let bi = wf.base_cell as usize;
            if li < n {
                wf_mark[li] = true;
            }
            if bi < n {
                wf_mark[bi] = true;
            }
        }
    }

    let margin = ((300.0 / dx).round() as usize).clamp(2, w.min(h) / 6);
    let mut up_cands = Vec::new();
    let mut dn_cands = Vec::new();
    for i in 0..n {
        if !is_channel[i] || wf_mark[i] || d8_dir[i] >= 8 {
            continue;
        }
        let r = i / w;
        let c = i % w;
        if r < margin || r + margin >= h || c < margin || c + margin >= w {
            continue;
        }
        let ord = strahler[i];
        if ord == 2 || ord == 3 {
            up_cands.push(i);
        } else if ord >= 4 {
            dn_cands.push(i);
        }
    }

    let mut b_up = Vec::new();
    let mut vf_up = Vec::new();
    let up_stride = (up_cands.len() / 80).max(1);
    for &i in up_cands.iter().step_by(up_stride) {
        if let Some((b, vf)) = sample_valley_transect(
            input.total_h_m,
            w,
            h,
            dx,
            i,
            d8_dir[i],
            input.sea_level_m,
            600.0,
        ) {
            b_up.push(b);
            vf_up.push(vf);
            if b_up.len() >= 40 {
                break;
            }
        }
    }

    let mut b_dn = Vec::new();
    let mut vf_dn = Vec::new();
    let dn_stride = (dn_cands.len() / 80).max(1);
    for &i in dn_cands.iter().step_by(dn_stride) {
        if let Some((b, vf)) = sample_valley_transect(
            input.total_h_m,
            w,
            h,
            dx,
            i,
            d8_dir[i],
            input.sea_level_m,
            1_200.0,
        ) {
            b_dn.push(b);
            vf_dn.push(vf);
            if b_dn.len() >= 40 {
                break;
            }
        }
    }

    let b_up_med = median_f64(&mut b_up).unwrap_or(0.0);
    let vf_up_med = median_f64(&mut vf_up).unwrap_or(0.0);
    let b_dn_med = median_f64(&mut b_dn).unwrap_or(0.0);
    let vf_dn_med = median_f64(&mut vf_dn).unwrap_or(0.0);
    let vf_ratio = if vf_up_med > 1e-6 {
        vf_dn_med / vf_up_med
    } else {
        0.0
    };

    // Flint slope-area `log10(S) = log10(k_s) - theta * log10(A)` using smoothed ~120 m D8 reaches
    // binned into 20 log-area intervals so Priority-Flood flat epsilon steps do not skew OLS.
    let reach_steps = ((120.0 / dx).round() as usize).clamp(1, 16);
    const BINS: usize = 20;
    let mut bin_slopes: Vec<Vec<f64>> = vec![Vec::new(); BINS];
    let mut bin_areas: Vec<Vec<f64>> = vec![Vec::new(); BINS];
    let log_a_min = libm::log10(5.0e4); // 0.05 km^2
    let log_a_max = libm::log10(3.0e8); // 300 km^2
    for i in 0..n {
        if !is_channel[i] || wf_mark[i] || strahler[i] < 2 {
            continue;
        }
        let a_m2 = flow_acc_m2(i);
        if !(5.0e4..=3.0e8).contains(&a_m2) {
            continue;
        }
        let mut cur = i;
        let mut reach_len = 0.0_f64;
        for _ in 0..reach_steps {
            let Some(nxt) = step_cell(cur, d8_dir[cur], w, h) else {
                break;
            };
            if wf_mark[nxt] {
                break;
            }
            reach_len += dx * DIST8[usize::from(d8_dir[cur])];
            cur = nxt;
        }
        if reach_len <= 1e-3 {
            continue;
        }
        let dh = f64::from(filled_h[i]) - f64::from(filled_h[cur]);
        if dh < 0.2 {
            continue;
        }
        let slope = dh / reach_len;
        if slope < 1e-4 {
            continue;
        }
        let la = libm::log10(a_m2);
        let frac = ((la - log_a_min) / (log_a_max - log_a_min)).clamp(0.0, 0.999_999);
        let b_idx = (frac * BINS as f64) as usize;
        bin_areas[b_idx].push(la);
        bin_slopes[b_idx].push(libm::log10(slope));
    }

    let mut flint_x = Vec::new();
    let mut flint_y = Vec::new();
    for b in 0..BINS {
        if bin_areas[b].len() >= 3 {
            if let (Some(mx), Some(my)) = (
                median_f64(&mut bin_areas[b]),
                median_f64(&mut bin_slopes[b]),
            ) {
                flint_x.push(mx);
                flint_y.push(my);
            }
        }
    }
    let (theta, flint_r2) = match ols_fit(&flint_x, &flint_y) {
        Some((slope, _, r2)) => (-slope, r2),
        None => (0.0, 0.0),
    };

    let passed = !b_up.is_empty()
        && !b_dn.is_empty()
        && (0.85..=1.35).contains(&b_up_med)
        && (0.05..=0.80).contains(&vf_up_med)
        && (1.60..=3.50).contains(&b_dn_med)
        && vf_dn_med >= 1.50
        && vf_ratio >= 2.5
        && (0.35..=0.65).contains(&theta);

    ValleyFlintReport {
        passed,
        upper_samples: b_up.len(),
        downstream_samples: b_dn.len(),
        b_up_median: b_up_med,
        vf_up_median: vf_up_med,
        b_down_median: b_dn_med,
        vf_down_median: vf_dn_med,
        vf_ratio_down_over_up: vf_ratio,
        flint_bins: flint_x.len(),
        flint_concavity_theta: theta,
        flint_r_squared: flint_r2,
    }
}

fn eval_metric_6_endorheic_spillways(
    input: &GShapeInput<'_>,
    raw_pit_before: usize,
    w: usize,
    h: usize,
) -> EndorheicAuditReport {
    let raw_non_water_land_pits =
        count_raw_non_water_land_pits(input.total_h_m, input.water_kind, w, h, input.sea_level_m);
    let unmodeled_micro_pits = input
        .reported_unmodeled_micro_pits
        .unwrap_or(raw_pit_before)
        .max(raw_non_water_land_pits);
    let severed_humid_spillways = input.reported_severed_humid_spillways.unwrap_or(0);
    let endorheic_land_fraction = f64::from(input.reported_endorheic_land_fraction.unwrap_or(0.0));
    let endorheic_humid_fraction =
        f64::from(input.reported_endorheic_humid_fraction.unwrap_or(0.0));

    let passed = unmodeled_micro_pits == 0
        && severed_humid_spillways == 0
        && endorheic_land_fraction <= 0.10
        && endorheic_humid_fraction <= 1e-6;

    EndorheicAuditReport {
        passed,
        unmodeled_micro_pits,
        raw_non_water_land_pits,
        severed_humid_spillways,
        endorheic_land_fraction,
        endorheic_humid_fraction,
    }
}

#[allow(clippy::too_many_arguments)]
fn eval_metric_7_hypsometry_spikes(
    input: &GShapeInput<'_>,
    d8_dir: &[u8],
    is_channel: &[bool],
    strahler: &[u8],
    w: usize,
    h: usize,
    dx: f64,
) -> HypsometrySpikeReport {
    let n = w * h;
    let mut land_cnt = 0_usize;
    let mut land_sum = 0.0_f64;
    let mut land_min = f64::INFINITY;
    let mut land_max = f64::NEG_INFINITY;
    for &z in input.total_h_m {
        if z > input.sea_level_m {
            let zf = f64::from(z);
            land_cnt += 1;
            land_sum += zf;
            land_min = land_min.min(zf);
            land_max = land_max.max(zf);
        }
    }
    let hi_global = if land_cnt > 0 && land_max > land_min + 1e-3 {
        ((land_sum / land_cnt as f64) - land_min) / (land_max - land_min)
    } else {
        0.0
    };

    // Sample up to 25 well-spaced sub-catchments (disjoint Strahler >= 3 reaches) via upward BFS.
    let mut outlets = Vec::new();
    for i in 0..n {
        if is_channel[i] && strahler[i] == 3 {
            let nxt_is_higher = step_cell(i, d8_dir[i], w, h)
                .is_none_or(|nxt| !is_channel[nxt] || strahler[nxt] > 3);
            if nxt_is_higher {
                outlets.push(i);
            }
        }
    }
    let stride = (outlets.len() / 25).max(1);
    let mut catchment_ids = vec![0_u16; n];
    let mut next_cid = 1_u16;
    let mut c_stats: Vec<(usize, f64, f64, f64)> = Vec::new();
    for &out_idx in outlets.iter().step_by(stride).take(25) {
        if catchment_ids[out_idx] != 0 {
            continue;
        }
        let cid = next_cid;
        next_cid += 1;
        let mut q = VecDeque::new();
        catchment_ids[out_idx] = cid;
        q.push_back(out_idx as u32);
        let mut cnt = 0_usize;
        let mut sum = 0.0_f64;
        let mut zmin = f64::INFINITY;
        let mut zmax = f64::NEG_INFINITY;
        while let Some(v_u32) = q.pop_front() {
            let v = v_u32 as usize;
            let zv = f64::from(input.total_h_m[v]);
            cnt += 1;
            sum += zv;
            zmin = zmin.min(zv);
            zmax = zmax.max(zv);
            let vr = v / w;
            let vc = v % w;
            for dir in 0..8 {
                let nr = vr as isize + DZ8[dir];
                let nc = vc as isize + DX8[dir];
                if nr < 0 || nc < 0 || nr >= h as isize || nc >= w as isize {
                    continue;
                }
                let u = (nr as usize) * w + (nc as usize);
                if catchment_ids[u] == 0 && step_cell(u, d8_dir[u], w, h) == Some(v) {
                    catchment_ids[u] = cid;
                    q.push_back(u as u32);
                }
            }
        }
        if cnt >= 50 && zmax > zmin + 5.0 {
            c_stats.push((cnt, sum, zmin, zmax));
        }
    }
    drop(catchment_ids);

    let mut catchment_his: Vec<f64> = c_stats
        .iter()
        .map(|&(cnt, sum, zmin, zmax)| ((sum / cnt as f64) - zmin) / (zmax - zmin))
        .collect();
    let catchment_hi_median = median_f64(&mut catchment_his).unwrap_or(0.0);

    // Build waterfall / caprock exclusion mask for needle-spike & hillslope check.
    let mut wf_mask = vec![false; n];
    if let Some(wfs) = input.waterfalls {
        for wf in wfs {
            for &cell in &[wf.lip_cell as usize, wf.base_cell as usize] {
                if cell < n {
                    let cr = cell / w;
                    let cc = cell % w;
                    for dr in -2isize..=2 {
                        for dc in -2isize..=2 {
                            let nr = cr as isize + dr;
                            let nc = cc as isize + dc;
                            if nr >= 0 && nc >= 0 && nr < h as isize && nc < w as isize {
                                wf_mask[(nr as usize) * w + (nc as usize)] = true;
                            }
                        }
                    }
                }
            }
        }
    }

    let dx_f32 = dx as f32;
    let mut needle_spikes = 0_usize;
    let mut slopes_deg = Vec::with_capacity((land_cnt / 4).max(16));
    let mut sample_ctr = 0_usize;
    for r in 1..h.saturating_sub(1) {
        let row = r * w;
        for c in 1..w.saturating_sub(1) {
            let i = row + c;
            let hc = input.total_h_m[i];
            if hc <= input.sea_level_m {
                continue;
            }
            let hl = input.total_h_m[i - 1];
            let hr = input.total_h_m[i + 1];
            let hu = input.total_h_m[i - w];
            let hd = input.total_h_m[i + w];

            sample_ctr += 1;
            if sample_ctr.is_multiple_of(4) {
                let gx = f64::from((hr - hl) / (2.0 * dx_f32));
                let gz = f64::from((hd - hu) / (2.0 * dx_f32));
                let sl_deg = libm::atan(libm::hypot(gx, gz)) * RAD_TO_DEG;
                slopes_deg.push(sl_deg);
            }

            if wf_mask[i] {
                continue;
            }
            // Opposing `>75°` slope reversal along x or z catches 1-cell Perlin/noise needle cones
            // while never falsely flagging monotonic bedrock cliff escarpments.
            let x_rev = ((hc - hl) / dx_f32 > TAN_75_DEG && (hc - hr) / dx_f32 > TAN_75_DEG)
                || ((hl - hc) / dx_f32 > TAN_75_DEG && (hr - hc) / dx_f32 > TAN_75_DEG);
            let z_rev = ((hc - hu) / dx_f32 > TAN_75_DEG && (hc - hd) / dx_f32 > TAN_75_DEG)
                || ((hu - hc) / dx_f32 > TAN_75_DEG && (hd - hc) / dx_f32 > TAN_75_DEG);
            if x_rev || z_rev {
                needle_spikes += 1;
            }
        }
    }

    slopes_deg.sort_by(f64::total_cmp);
    let hillslope_p999_deg = if slopes_deg.is_empty() {
        0.0
    } else {
        let idx = ((slopes_deg.len() as f64) * 0.999) as usize;
        slopes_deg[idx.min(slopes_deg.len() - 1)]
    };

    let catchment_ok = !c_stats.is_empty() && (0.30..=0.60).contains(&catchment_hi_median);
    let passed = (0.22..=0.55).contains(&hi_global) && catchment_ok && needle_spikes == 0;

    HypsometrySpikeReport {
        passed,
        hi_global,
        catchment_count: c_stats.len(),
        catchment_hi_median,
        non_cliff_needle_spikes: needle_spikes,
        hillslope_p999_deg,
    }
}

fn eval_shoreline_fractal(
    total_h_m: &[f32],
    water_kind: Option<&[u8]>,
    w: usize,
    h: usize,
    sea_level_m: f32,
) -> ShorelineFractalReport {
    let mut is_shore = vec![false; w * h];
    let mut shore_cnt = 0_usize;
    for r in 0..h {
        let row = r * w;
        for c in 0..w {
            let i = row + c;
            let (is_oc, _) = is_water_mask_cell(water_kind, total_h_m[i], sea_level_m, i);
            if is_oc {
                continue;
            }
            let touches_ocean = [
                (r > 0, i.wrapping_sub(w)),
                (r + 1 < h, i + w),
                (c > 0, i.wrapping_sub(1)),
                (c + 1 < w, i + 1),
            ]
            .iter()
            .any(|&(valid, nb)| {
                valid && is_water_mask_cell(water_kind, total_h_m[nb], sea_level_m, nb).0
            });
            if touches_ocean {
                is_shore[i] = true;
                shore_cnt += 1;
            }
        }
    }

    let max_box = (w.min(h) / 4).max(2);
    let box_sizes: Vec<usize> = [2_usize, 4, 8, 16, 32, 64]
        .into_iter()
        .filter(|&s| s <= max_box)
        .collect();
    if shore_cnt < 16 || box_sizes.len() < 3 {
        return ShorelineFractalReport {
            passed: false,
            coastline_cells: shore_cnt,
            box_sizes,
            d_shore: None,
            r_squared: None,
        };
    }

    let mut xs = Vec::with_capacity(box_sizes.len());
    let mut ys = Vec::with_capacity(box_sizes.len());
    for &s in &box_sizes {
        let gx = w.div_ceil(s);
        let gz = h.div_ceil(s);
        let mut occupied = vec![false; gx * gz];
        for r in 0..h {
            let bz = r / s;
            let row = r * w;
            for c in 0..w {
                if is_shore[row + c] {
                    occupied[bz * gx + (c / s)] = true;
                }
            }
        }
        let n_boxes = occupied.iter().filter(|&&v| v).count();
        if n_boxes > 0 {
            xs.push(libm::log(1.0 / s as f64));
            ys.push(libm::log(n_boxes as f64));
        }
    }
    let (d_shore, r2) = match ols_fit(&xs, &ys) {
        Some((slope, _, r2)) => (Some(slope), Some(r2)),
        None => (None, None),
    };
    let passed = d_shore.is_some_and(|d| (1.15..=1.45).contains(&d));

    ShorelineFractalReport {
        passed,
        coastline_cells: shore_cnt,
        box_sizes,
        d_shore,
        r_squared: r2,
    }
}

impl GShapeInput<'_> {
    /// Returns a clean single-DEM `GShapeInput` (`--dem-input` mode, §4.2) with uniform runoff.
    #[must_use]
    pub fn from_dem_only(
        total_h_m: &[f32],
        width: usize,
        height: usize,
        dx_m: f32,
        sea_level_m: f32,
    ) -> GShapeInput<'_> {
        GShapeInput {
            width,
            height,
            dx_m,
            sea_level_m,
            total_h_m,
            precip_m_yr: None,
            pet_m_yr: None,
            discharge_q_m3_s: None,
            river_w_m: None,
            water_kind: None,
            glacier_h_m: None,
            lithology_id: None,
            basins: None,
            waterfalls: None,
            reported_unmodeled_micro_pits: None,
            reported_severed_humid_spillways: None,
            reported_endorheic_land_fraction: None,
            reported_endorheic_humid_fraction: None,
            q_min_m3_s: if width * height >= 512 * 512 {
                0.027_778
            } else {
                0.001
            },
            min_hack_samples: if width * height >= 512 * 512 { 500 } else { 20 },
            has_climate_fields: false,
        }
    }
}

/// Placeholder helper ensuring `D8_NO_DIR` remains consistently aligned with [`NO_DIR`].
pub const VERIFY_D8_NO_DIR: u8 = D8_NO_DIR;
