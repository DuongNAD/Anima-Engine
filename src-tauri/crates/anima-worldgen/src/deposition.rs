//! Stage S4: Post-routing sediment deposition (`§2.2.4`) — piedmont alluvial fans (`AER-03`),
//! Howard-Knutson (1984) 35-step kinematic meandering floodplains & oxbow lakes (`AER-07`, `RIV-01`),
//! DeltaRCM (`Liang et al. 2015`) prograding coastal deltas (`AER-04`, `AER-05`), and upland
//! flat-slope braided micro-ponds (`AER-08`).

use crate::hydrology::HydrologyField;
use crate::rng::stage_rng;
use crate::WaterKind;
use rand::Rng;
use std::fmt;

const DX8: [isize; 8] = [1, 1, 0, -1, -1, -1, 0, 1];
const DZ8: [isize; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
const UX8: [f32; 8] = [
    1.0,
    std::f32::consts::FRAC_1_SQRT_2,
    0.0,
    -std::f32::consts::FRAC_1_SQRT_2,
    -1.0,
    -std::f32::consts::FRAC_1_SQRT_2,
    0.0,
    std::f32::consts::FRAC_1_SQRT_2,
];
const UZ8: [f32; 8] = [
    0.0,
    std::f32::consts::FRAC_1_SQRT_2,
    1.0,
    std::f32::consts::FRAC_1_SQRT_2,
    0.0,
    -std::f32::consts::FRAC_1_SQRT_2,
    -1.0,
    -std::f32::consts::FRAC_1_SQRT_2,
];

/// Error returned when Stage S4 inputs or configuration are invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepositionError {
    pub message: String,
}

impl fmt::Display for DepositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DepositionError {}

/// Configuration for Stage S4 alluvial fans, meanders, deltas, and upland micro-ponds.
#[derive(Debug, Clone, PartialEq)]
pub struct DepositionConfig {
    pub seed: u64,
    pub schema_tag: String,
    pub dx_m: f32,
    pub sea_level_m: f32,
    /// Minimum canyon-to-piedmont slope drop ratio (`S_up / S_down >= 4.0`) for alluvial fans.
    pub fan_slope_drop_ratio: f32,
    pub fan_min_q_m3_s: f32,
    pub fan_min_spacing_m: f32,
    /// Maximum valley slope (`0.004`) permitting Howard-Knutson lateral meander migration.
    pub meander_max_slope: f32,
    pub meander_min_q_m3_s: f32,
    /// Number of kinematic meander evolution steps (`35` per `§2.2.4`).
    pub meander_steps: usize,
    /// Maximum offshore shelf depth (`40.0 m`) permitting DeltaRCM delta progradation.
    pub delta_max_shelf_depth_m: f32,
    pub delta_min_q_m3_s: f32,
    /// Total water+sediment parcels per delta mouth (`1,500..=4,000`).
    pub delta_parcels_per_mouth: usize,
    pub delta_max_mouths: usize,
    /// Minimum elevation (`3,300 m`) for upland páramo braided micro-ponds.
    pub paramo_min_elev_m: f32,
    /// Maximum slope (`0.035`) for upland braided micro-ponds.
    pub paramo_max_slope: f32,
    pub target_micro_ponds: usize,
}

impl Default for DepositionConfig {
    fn default() -> Self {
        Self {
            seed: 0x0057_5632_2026_1120,
            schema_tag: "ANMW3-SNSM-v1".to_owned(),
            dx_m: 10.0,
            sea_level_m: 0.0,
            fan_slope_drop_ratio: 4.0,
            fan_min_q_m3_s: 0.8,
            fan_min_spacing_m: 1_500.0,
            meander_max_slope: 0.004,
            meander_min_q_m3_s: 3.0,
            meander_steps: 35,
            delta_max_shelf_depth_m: 40.0,
            delta_min_q_m3_s: 2.0,
            delta_parcels_per_mouth: 2_500,
            delta_max_mouths: 4,
            paramo_min_elev_m: 3_300.0,
            paramo_max_slope: 0.035,
            target_micro_ponds: 0,
        }
    }
}

/// Borrowed S5 routing slices consumed by Stage S4.
pub struct DepositionInput<'a> {
    pub width: usize,
    pub height: usize,
    pub discharge_q_m3_s: &'a [f32],
    pub river_w_m: &'a [f32],
    pub river_d_m: &'a [f32],
    pub flow_dir_x: &'a [f32],
    pub flow_dir_z: &'a [f32],
    pub is_channel: &'a [bool],
    pub d8_receiver: Option<&'a [u32]>,
}

impl<'a> DepositionInput<'a> {
    /// Builds a Stage S4 view directly from a [`HydrologyField`] and optional D8 receiver array.
    #[must_use]
    pub fn from_hydrology(hydro: &'a HydrologyField, d8_receiver: Option<&'a [u32]>) -> Self {
        Self {
            width: hydro.width,
            height: hydro.height,
            discharge_q_m3_s: &hydro.discharge_q_m3_s,
            river_w_m: &hydro.river_w_m,
            river_d_m: &hydro.river_d_m,
            flow_dir_x: &hydro.flow_dir_x,
            flow_dir_z: &hydro.flow_dir_z,
            is_channel: &hydro.is_channel,
            d8_receiver,
        }
    }
}

/// Output depositional masks and diagnostics from [`apply_deposition`].
#[derive(Debug, Clone)]
pub struct DepositionField {
    pub width: usize,
    pub height: usize,
    /// Alluvial fan & point-bar deposition intensity in `[0.0, 1.0]`.
    pub alluvial_mask: Vec<f32>,
    /// Concentric meander scroll-bar / point-bar imprint in `[0.0, 1.0]`.
    pub meander_scroll: Vec<f32>,
    /// Coastal delta & plume suspended sediment turbidity in `[0.0, 1.0]`.
    pub water_turbidity: Vec<f32>,
    pub fan_count: usize,
    pub meander_reach_count: usize,
    pub oxbow_count: usize,
    pub delta_count: usize,
    pub micro_pond_count: usize,
}

#[inline]
fn total_elev(bedrock_h: &[f32], sediment_s: &[f32], idx: usize) -> f32 {
    bedrock_h[idx] + sediment_s[idx]
}

#[inline]
fn bit_get(bits: &[u64], idx: usize) -> bool {
    (bits[idx >> 6] & (1_u64 << (idx & 63))) != 0
}

#[inline]
fn bit_set(bits: &mut [u64], idx: usize) {
    bits[idx >> 6] |= 1_u64 << (idx & 63);
}

fn compute_polyline_normals_curv(pts: &[(f32, f32)]) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
    let m = pts.len();
    let mut s_arc = vec![0.0_f32; m];
    for k in 1..m {
        s_arc[k] = s_arc[k - 1] + libm::hypotf(pts[k].0 - pts[k - 1].0, pts[k].1 - pts[k - 1].1);
    }
    let mut curv = vec![0.0_f32; m];
    let mut nx = vec![0.0_f32; m];
    let mut nz = vec![0.0_f32; m];
    for k in 1..m.saturating_sub(1) {
        let ds_back = libm::hypotf(pts[k].0 - pts[k - 1].0, pts[k].1 - pts[k - 1].1).max(1.0);
        let ds_fwd = libm::hypotf(pts[k + 1].0 - pts[k].0, pts[k + 1].1 - pts[k].1).max(1.0);
        let ds_tot = ds_back + ds_fwd;
        let tx = (pts[k + 1].0 - pts[k - 1].0) / ds_tot;
        let tz = (pts[k + 1].1 - pts[k - 1].1) / ds_tot;
        let t_len = libm::hypotf(tx, tz).max(1e-5);
        // Outward normal for positive curvature `C`: (tz, -tx) / |t|.
        nx[k] = tz / t_len;
        nz[k] = -tx / t_len;

        let x1 = (pts[k].0 - pts[k - 1].0) / ds_back;
        let z1 = (pts[k].1 - pts[k - 1].1) / ds_back;
        let x2 = (pts[k + 1].0 - pts[k].0) / ds_fwd;
        let z2 = (pts[k + 1].1 - pts[k].1) / ds_fwd;
        let xpp = (x2 - x1) / (0.5 * ds_tot);
        let zpp = (z2 - z1) / (0.5 * ds_tot);
        let speed3 = t_len * t_len * t_len;
        curv[k] = (tx * zpp - tz * xpp) / speed3.max(1e-6);
    }
    (s_arc, curv, nx, nz)
}

fn steepest_downstream(
    bedrock_h: &[f32],
    sediment_s: &[f32],
    d8_rec: Option<&[u32]>,
    w: usize,
    h: usize,
    idx: usize,
) -> Option<usize> {
    if let Some(rec) = d8_rec {
        let r_idx = rec[idx];
        return if r_idx != crate::hydrology::NO_RECEIVER {
            Some(r_idx as usize)
        } else {
            None
        };
    }
    let r = idx / w;
    let c = idx % w;
    let h0 = total_elev(bedrock_h, sediment_s, idx);
    let mut best_s = 0.0_f32;
    let mut best_nb = None;
    for dir in 0..8 {
        let nr = r as isize + DZ8[dir];
        let nc = c as isize + DX8[dir];
        if nr < 0 || nc < 0 || nr >= h as isize || nc >= w as isize {
            continue;
        }
        let nb = nr as usize * w + nc as usize;
        let dh = h0 - total_elev(bedrock_h, sediment_s, nb);
        if dh > 0.0 {
            let d = if dir % 2 == 0 {
                1.0
            } else {
                std::f32::consts::SQRT_2
            };
            let s = dh / d;
            if s > best_s {
                best_s = s;
                best_nb = Some(nb);
            }
        }
    }
    best_nb
}

struct DiskStamp {
    cx: f32,
    cz: f32,
    rad_cells: f32,
    depth_m: f32,
    wk: u8,
}

fn stamp_disk_river(
    bedrock_h: &[f32],
    sediment_s: &mut [f32],
    water_h_m: &mut [f32],
    water_kind: &mut [u8],
    dims: (usize, usize),
    stamp: DiskStamp,
) {
    let (w, h) = dims;
    let r0 = (libm::floorf(stamp.cz - stamp.rad_cells) as isize).max(1) as usize;
    let r1 = (libm::ceilf(stamp.cz + stamp.rad_cells) as isize).min(h as isize - 2) as usize;
    let c0 = (libm::floorf(stamp.cx - stamp.rad_cells) as isize).max(1) as usize;
    let c1 = (libm::ceilf(stamp.cx + stamp.rad_cells) as isize).min(w as isize - 2) as usize;
    let rad_sq = stamp.rad_cells * stamp.rad_cells;
    for r in r0..=r1 {
        for c in c0..=c1 {
            let dr = r as f32 - stamp.cz;
            let dc = c as f32 - stamp.cx;
            if dr * dr + dc * dc <= rad_sq {
                let idx = r * w + c;
                let surf = total_elev(bedrock_h, sediment_s, idx);
                let cut = stamp.depth_m.min(sediment_s[idx].max(0.0) * 0.5);
                sediment_s[idx] -= cut;
                let new_surf = bedrock_h[idx] + sediment_s[idx];
                water_h_m[idx] = water_h_m[idx].max(surf.max(new_surf + stamp.depth_m));
                water_kind[idx] = stamp.wk;
            }
        }
    }
}

/// Executes Stage S4 post-routing deposition onto `sediment_s`, `water_h_m`, and `water_kind`.
pub fn apply_deposition(
    bedrock_h: &mut [f32],
    sediment_s: &mut [f32],
    water_h_m: &mut [f32],
    water_kind: &mut [u8],
    input: &DepositionInput<'_>,
    cfg: &DepositionConfig,
) -> Result<DepositionField, DepositionError> {
    let w = input.width;
    let h = input.height;
    if w < 16 || h < 16 {
        return Err(DepositionError {
            message: "grid dimensions must be >= 16x16".to_owned(),
        });
    }
    let n = w * h;
    if bedrock_h.len() != n
        || sediment_s.len() != n
        || water_h_m.len() != n
        || water_kind.len() != n
        || input.discharge_q_m3_s.len() != n
        || input.river_w_m.len() != n
        || input.river_d_m.len() != n
        || input.flow_dir_x.len() != n
        || input.flow_dir_z.len() != n
        || input.is_channel.len() != n
    {
        return Err(DepositionError {
            message: "input slice lengths do not match grid dimensions".to_owned(),
        });
    }
    if !cfg.dx_m.is_finite() || cfg.dx_m <= 0.0 {
        return Err(DepositionError {
            message: "dx_m must be finite and > 0".to_owned(),
        });
    }

    let mut alluvial_mask = vec![0.0_f32; n];
    let mut meander_scroll = vec![0.0_f32; n];
    let mut water_turbidity = vec![0.0_f32; n];

    // Seed initial active river channel cells in water_kind if not already marked.
    for i in 0..n {
        let elev = total_elev(bedrock_h, sediment_s, i);
        if elev <= cfg.sea_level_m {
            water_kind[i] = WaterKind::Ocean as u8;
            water_h_m[i] = cfg.sea_level_m;
        } else if input.is_channel[i]
            && input.river_w_m[i] >= 0.5
            && water_kind[i] == WaterKind::None as u8
        {
            water_kind[i] = WaterKind::River as u8;
            water_h_m[i] = water_h_m[i].max(elev + input.river_d_m[i]);
        }
    }

    // -------------------------------------------------------------------------
    // 1. Piedmont Alluvial Fans (`AER-03`)
    // -------------------------------------------------------------------------
    let mut fan_rng = stage_rng(cfg.seed, w as u32, h as u32, &cfg.schema_tag, "S4-FAN");
    let step_look = ((180.0 / cfg.dx_m).round() as usize).clamp(4, 24);
    let mut fan_cands: Vec<(f32, usize, f32, f32)> = Vec::new();

    for r in 2..(h - 2) {
        for c in 2..(w - 2) {
            let idx = r * w + c;
            if !input.is_channel[idx] || input.discharge_q_m3_s[idx] < cfg.fan_min_q_m3_s {
                continue;
            }
            let h_apex = total_elev(bedrock_h, sediment_s, idx);
            if h_apex <= cfg.sea_level_m + 15.0 {
                continue;
            }
            // Measure downstream slope over `step_look` steps.
            let mut down_cur = idx;
            let mut down_dist = 0.0_f32;
            for _ in 0..step_look {
                if let Some(nxt) =
                    steepest_downstream(bedrock_h, sediment_s, input.d8_receiver, w, h, down_cur)
                {
                    let dr = (nxt / w) as f32 - (down_cur / w) as f32;
                    let dc = (nxt % w) as f32 - (down_cur % w) as f32;
                    down_dist += libm::hypotf(dc, dr) * cfg.dx_m;
                    down_cur = nxt;
                    if total_elev(bedrock_h, sediment_s, nxt) <= cfg.sea_level_m {
                        break;
                    }
                } else {
                    break;
                }
            }
            if down_dist < 2.0 * cfg.dx_m {
                continue;
            }
            let h_down = total_elev(bedrock_h, sediment_s, down_cur);
            let s_down = ((h_apex - h_down) / down_dist).max(0.0);

            // Measure upstream slope by walking uphill along highest-Q neighbors.
            let mut up_cur = idx;
            let mut up_dist = 0.0_f32;
            for _ in 0..step_look {
                let ur = up_cur / w;
                let uc = up_cur % w;
                let hu = total_elev(bedrock_h, sediment_s, up_cur);
                let mut best_up = None;
                let mut best_q = -1.0_f32;
                let mut best_d = 1.0_f32;
                for dir in 0..8 {
                    let nr = ur as isize + DZ8[dir];
                    let nc = uc as isize + DX8[dir];
                    if nr < 0 || nc < 0 || nr >= h as isize || nc >= w as isize {
                        continue;
                    }
                    let nb = nr as usize * w + nc as usize;
                    if total_elev(bedrock_h, sediment_s, nb) > hu {
                        let nq = input.discharge_q_m3_s[nb];
                        if nq > best_q {
                            best_q = nq;
                            best_up = Some(nb);
                            best_d = if dir % 2 == 0 {
                                1.0
                            } else {
                                std::f32::consts::SQRT_2
                            };
                        }
                    }
                }
                if let Some(nb) = best_up {
                    up_dist += best_d * cfg.dx_m;
                    up_cur = nb;
                } else {
                    break;
                }
            }
            if up_dist < 2.0 * cfg.dx_m {
                continue;
            }
            let h_up = total_elev(bedrock_h, sediment_s, up_cur);
            let s_up = (h_up - h_apex) / up_dist;

            if s_up >= 0.04 && s_up / s_down.max(1e-4) >= cfg.fan_slope_drop_ratio {
                let ux = input.flow_dir_x[idx];
                let uz = input.flow_dir_z[idx];
                let (dux, duz) = if libm::hypotf(ux, uz) > 0.1 {
                    let l = libm::hypotf(ux, uz);
                    (ux / l, uz / l)
                } else {
                    let dx = (down_cur % w) as f32 - c as f32;
                    let dz = (down_cur / w) as f32 - r as f32;
                    let l = libm::hypotf(dx, dz).max(1.0);
                    (dx / l, dz / l)
                };
                fan_cands.push((input.discharge_q_m3_s[idx], idx, dux, duz));
            }
        }
    }

    fan_cands.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let fan_sp_sq = cfg.fan_min_spacing_m * cfg.fan_min_spacing_m;
    let mut accepted_fans: Vec<(usize, f32, f32, f32)> = Vec::new();
    let max_fan_r_grid = 0.32 * (w.min(h) as f32) * cfg.dx_m;

    for (q, idx, ux, uz) in fan_cands {
        let fr = (idx / w) as f32 * cfg.dx_m;
        let fc = (idx % w) as f32 * cfg.dx_m;
        let too_close = accepted_fans.iter().any(|&(a_idx, _, _, _)| {
            let ar = (a_idx / w) as f32 * cfg.dx_m;
            let ac = (a_idx % w) as f32 * cfg.dx_m;
            (fr - ar) * (fr - ar) + (fc - ac) * (fc - ac) < fan_sp_sq
        });
        if too_close {
            continue;
        }
        let r_fan =
            (350.0 * libm::powf(q.max(0.5), 0.35)).clamp(200.0, 1_800.0_f32.min(max_fan_r_grid));
        let h_cone = (2.5 + 1.8 * libm::powf(q.max(0.5), 0.25)).clamp(2.5, 14.0);
        let apex_elev = total_elev(bedrock_h, sediment_s, idx);
        let ar = idx / w;
        let ac = idx % w;
        let rad_cells = (r_fan / cfg.dx_m).ceil() as isize;
        let half_ang_rad = 55.0_f32 * (std::f32::consts::PI / 180.0);

        for dr in -rad_cells..=rad_cells {
            for dc in -rad_cells..=rad_cells {
                let nr = ar as isize + dr;
                let nc = ac as isize + dc;
                if nr < 1 || nc < 1 || nr >= (h - 1) as isize || nc >= (w - 1) as isize {
                    continue;
                }
                let dist_m = libm::hypotf(dc as f32, dr as f32) * cfg.dx_m;
                if dist_m < cfg.dx_m * 0.5 || dist_m > r_fan {
                    continue;
                }
                let vx = (dc as f32 * cfg.dx_m) / dist_m;
                let vz = (dr as f32 * cfg.dx_m) / dist_m;
                let cos_th = (vx * ux + vz * uz).clamp(-1.0, 1.0);
                let ang = libm::acosf(cos_th);
                if ang > half_ang_rad {
                    continue;
                }
                let ang_w = libm::cosf((ang / half_ang_rad) * (std::f32::consts::FRAC_PI_2));
                let rad_w = libm::powf((1.0 - dist_m / r_fan).max(0.0), 1.35);
                let fan_w = rad_w * ang_w * ang_w;
                if fan_w <= 0.01 {
                    continue;
                }
                let nb = nr as usize * w + nc as usize;
                let cur_e = total_elev(bedrock_h, sediment_s, nb);
                if cur_e <= cfg.sea_level_m + 0.5 {
                    continue;
                }
                let max_allowed = apex_elev - 0.002 * dist_m;
                let dep = (h_cone * fan_w).min((max_allowed - cur_e).max(0.0));
                if dep > 0.0 {
                    sediment_s[nb] += dep;
                    alluvial_mask[nb] = alluvial_mask[nb].max(fan_w.clamp(0.0, 1.0));
                }
            }
        }

        // Trace 1 active wet braid + 3 abandoned dry braids on fan surface.
        let wet_angle: f32 = fan_rng.gen_range(-0.18..0.18);
        let dry_angles: [f32; 3] = [
            fan_rng.gen_range(-0.68..-0.28),
            fan_rng.gen_range(0.28..0.68),
            fan_rng.gen_range(-0.45..0.45),
        ];
        let base_theta = libm::atan2f(uz, ux);
        let steps_len = (r_fan / cfg.dx_m).round() as usize;

        for (b_i, &delta_ang) in std::iter::once(&wet_angle)
            .chain(dry_angles.iter())
            .enumerate()
        {
            let is_wet = b_i == 0;
            let mut bx = ac as f32;
            let mut bz = ar as f32;
            let th = base_theta + delta_ang;
            let step_x = libm::cosf(th);
            let step_z = libm::sinf(th);
            for s in 1..steps_len {
                let wobble = 0.25 * libm::sinf(s as f32 * 0.45 + b_i as f32 * 1.7);
                bx += step_x - step_z * wobble * 0.3;
                bz += step_z + step_x * wobble * 0.3;
                let r_i = bx.round() as isize;
                let z_i = bz.round() as isize;
                if z_i < 1 || r_i < 1 || z_i >= (h - 1) as isize || r_i >= (w - 1) as isize {
                    break;
                }
                let nb = z_i as usize * w + r_i as usize;
                let cur_e = total_elev(bedrock_h, sediment_s, nb);
                if cur_e <= cfg.sea_level_m + 0.5 {
                    break;
                }
                if is_wet {
                    let cut = 0.5_f32.min(sediment_s[nb] * 0.3);
                    sediment_s[nb] -= cut;
                    let surf = total_elev(bedrock_h, sediment_s, nb);
                    water_kind[nb] = WaterKind::River as u8;
                    water_h_m[nb] = water_h_m[nb].max(surf + 0.45);
                    alluvial_mask[nb] = alluvial_mask[nb].max(0.6);
                } else {
                    // Abandoned dry braid incised into fan sediment; strictly WaterKind::None.
                    let cut = 0.6_f32.min(sediment_s[nb] * 0.35);
                    sediment_s[nb] -= cut;
                    water_kind[nb] = WaterKind::None as u8;
                    alluvial_mask[nb] = alluvial_mask[nb].max(0.45);
                }
            }
        }

        accepted_fans.push((idx, r_fan, ux, uz));
    }

    // -------------------------------------------------------------------------
    // 2. Howard & Knutson (1984) 35-Step Kinematic Meanders (`AER-07`, `RIV-01`)
    // -------------------------------------------------------------------------
    let word_count = (n + 63) >> 6;
    let mut visited_meander = vec![0_u64; word_count];
    let mut meander_reach_count = 0usize;
    let mut oxbow_count = 0usize;

    // Collect candidate low-slope trunk starts sorted by Q descending.
    let mut trunk_cells: Vec<(f32, u32)> = Vec::new();
    for r in 2..(h - 2) {
        for c in 2..(w - 2) {
            let idx = r * w + c;
            if !input.is_channel[idx] || input.discharge_q_m3_s[idx] < cfg.meander_min_q_m3_s {
                continue;
            }
            let e = total_elev(bedrock_h, sediment_s, idx);
            if e <= cfg.sea_level_m + 2.0 {
                continue;
            }
            if let Some(nxt) =
                steepest_downstream(bedrock_h, sediment_s, input.d8_receiver, w, h, idx)
            {
                let dr = (nxt / w) as f32 - r as f32;
                let dc = (nxt % w) as f32 - c as f32;
                let d_m = libm::hypotf(dc, dr) * cfg.dx_m;
                let s = (e - total_elev(bedrock_h, sediment_s, nxt)) / d_m.max(cfg.dx_m);
                if s >= 0.0 && s < cfg.meander_max_slope {
                    trunk_cells.push((input.discharge_q_m3_s[idx], idx as u32));
                }
            }
        }
    }
    trunk_cells.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    // Build upstream-to-downstream reaches (smallest Q upstream first -> longest continuous trunk).
    for &(_, start_u32) in &trunk_cells {
        let start_idx = start_u32 as usize;
        if bit_get(&visited_meander, start_idx) {
            continue;
        }
        let mut chain = vec![start_idx];
        bit_set(&mut visited_meander, start_idx);
        let mut cur = start_idx;
        while let Some(nxt) =
            steepest_downstream(bedrock_h, sediment_s, input.d8_receiver, w, h, cur)
        {
            if bit_get(&visited_meander, nxt) {
                break;
            }
            let e_cur = total_elev(bedrock_h, sediment_s, cur);
            let e_nxt = total_elev(bedrock_h, sediment_s, nxt);
            if e_nxt <= cfg.sea_level_m + 1.5 {
                break;
            }
            let dr = (nxt / w) as f32 - (cur / w) as f32;
            let dc = (nxt % w) as f32 - (cur % w) as f32;
            let d_m = libm::hypotf(dc, dr) * cfg.dx_m;
            let s = (e_cur - e_nxt) / d_m.max(cfg.dx_m);
            if s >= cfg.meander_max_slope || input.discharge_q_m3_s[nxt] < cfg.meander_min_q_m3_s {
                break;
            }
            bit_set(&mut visited_meander, nxt);
            chain.push(nxt);
            cur = nxt;
        }
        if chain.len() < 7 {
            continue;
        }

        let mean_w: f32 = chain
            .iter()
            .map(|&i| input.river_w_m[i].max(cfg.dx_m))
            .sum::<f32>()
            / chain.len() as f32;
        let mean_d: f32 = chain
            .iter()
            .map(|&i| input.river_d_m[i].max(0.6))
            .sum::<f32>()
            / chain.len() as f32;
        let w_bank = mean_w.max(cfg.dx_m);
        let ds_target = (1.5 * w_bank).clamp(cfg.dx_m, 35.0_f32.max(cfg.dx_m));

        // Convert to physical metres polyline & resample to uniform `ds_target`.
        let raw_pts: Vec<(f32, f32)> = chain
            .iter()
            .map(|&i| ((i % w) as f32 * cfg.dx_m, (i / w) as f32 * cfg.dx_m))
            .collect();
        let mut pts = resample_polyline(&raw_pts, ds_target);
        if pts.len() < 7 {
            continue;
        }
        meander_reach_count += 1;

        let omega = -1.0_f32;
        let gamma_hk = 2.5_f32;
        let k_mig_dt = (0.55 * w_bank).max(8.5);
        let rad_fp_cells = ((3.2 * w_bank) / cfg.dx_m).clamp(2.0, 8.0);

        for step in 0..cfg.meander_steps {
            let m = pts.len();
            if m < 7 {
                break;
            }
            let (s_arc, mut curv, mut nx, mut nz) = compute_polyline_normals_curv(&pts);

            // Upstream-weighted Howard-Knutson migration.
            let mut next_pts = pts.clone();
            let max_disp = 0.35 * ds_target;
            for k in 1..(m - 1) {
                let mut integral = 0.0_f32;
                for j in 1..=k {
                    let ds_j = 0.5 * (s_arc[j + 1] - s_arc[j - 1]);
                    let lag = 2.0 * (s_arc[k] - s_arc[j]) / w_bank;
                    if lag < 12.0 {
                        integral += curv[j] * libm::expf(-lag) * (ds_j / w_bank);
                    }
                }
                let r1 = omega * curv[k] + gamma_hk * integral;
                let dn = (k_mig_dt * r1).clamp(-max_disp, max_disp);
                next_pts[k].0 = (pts[k].0 + dn * nx[k]).clamp(cfg.dx_m, (w - 2) as f32 * cfg.dx_m);
                next_pts[k].1 = (pts[k].1 + dn * nz[k]).clamp(cfg.dx_m, (h - 2) as f32 * cfg.dx_m);
            }

            // Mild 3-point Laplacian smoothing to avoid grid-aliasing zigzags.
            let mut smoothed = next_pts.clone();
            for k in 1..(m - 1) {
                smoothed[k].0 =
                    0.85 * next_pts[k].0 + 0.075 * (next_pts[k - 1].0 + next_pts[k + 1].0);
                smoothed[k].1 =
                    0.85 * next_pts[k].1 + 0.075 * (next_pts[k - 1].1 + next_pts[k + 1].1);
            }
            pts = smoothed;

            // Neck cutoff detection (`|k2 - k1| > 6`, Euclidean distance < 1.3 * w_bank).
            let cutoff_thresh = (1.35 * w_bank).max(2.0 * cfg.dx_m);
            let mut cut_span: Option<(usize, usize)> = None;
            'outer: for k1 in 1..pts.len().saturating_sub(8) {
                for k2 in (k1 + 7)..(pts.len() - 1) {
                    let d = libm::hypotf(pts[k2].0 - pts[k1].0, pts[k2].1 - pts[k1].1);
                    if d < cutoff_thresh {
                        cut_span = Some((k1, k2));
                        break 'outer;
                    }
                }
            }
            let did_cutoff = cut_span.is_some();
            if let Some((k1, k2)) = cut_span {
                // Excavate bypassed loop as a single coherent crescent oxbow lake (`WaterKind::FreshwaterLake = 2`).
                let ox_rad = (0.75 * w_bank / cfg.dx_m).clamp(1.2, 2.5);
                let mut ox_cells: Vec<usize> = Vec::new();
                let mut min_loop_surf = f32::INFINITY;
                for &(px, pz) in &pts[(k1 + 1)..k2] {
                    let cx = px / cfg.dx_m;
                    let cz = pz / cfg.dx_m;
                    let r0 = (libm::floorf(cz - ox_rad) as isize).max(1) as usize;
                    let r1 = (libm::ceilf(cz + ox_rad) as isize).min(h as isize - 2) as usize;
                    let c0 = (libm::floorf(cx - ox_rad) as isize).max(1) as usize;
                    let c1 = (libm::ceilf(cx + ox_rad) as isize).min(w as isize - 2) as usize;
                    let rad_sq = ox_rad * ox_rad;
                    for rr in r0..=r1 {
                        for cc in c0..=c1 {
                            let dr = rr as f32 - cz;
                            let dc = cc as f32 - cx;
                            if dr * dr + dc * dc <= rad_sq {
                                let nb = rr * w + cc;
                                let e_nb = total_elev(bedrock_h, sediment_s, nb);
                                if e_nb > cfg.sea_level_m + 0.5 {
                                    min_loop_surf = min_loop_surf.min(e_nb);
                                    ox_cells.push(nb);
                                }
                            }
                        }
                    }
                }
                ox_cells.sort_unstable();
                ox_cells.dedup();
                if min_loop_surf.is_finite() && !ox_cells.is_empty() {
                    let target_floor = (min_loop_surf - 1.35).max(cfg.sea_level_m + 0.25);
                    let target_water = (min_loop_surf - 0.15).max(target_floor + 0.6);
                    for nb in ox_cells {
                        let cur_e = total_elev(bedrock_h, sediment_s, nb);
                        if cur_e > target_floor {
                            let cut = cur_e - target_floor;
                            if sediment_s[nb] >= cut {
                                sediment_s[nb] -= cut;
                            } else {
                                let rem = cut - sediment_s[nb].max(0.0);
                                sediment_s[nb] = 0.0;
                                bedrock_h[nb] -= rem;
                            }
                        }
                        water_h_m[nb] = water_h_m[nb].max(target_water);
                        water_kind[nb] = WaterKind::FreshwaterLake as u8;
                    }
                }
                pts.drain((k1 + 1)..k2);
                oxbow_count += 1;
            }

            // Imprint point bars, scroll ridges, and cut banks on periodic snapshots.
            if step % 5 == 4 || step + 1 == cfg.meander_steps {
                if did_cutoff && pts.len() >= 3 {
                    let (_, c_new, nx_new, nz_new) = compute_polyline_normals_curv(&pts);
                    curv = c_new;
                    nx = nx_new;
                    nz = nz_new;
                }
                let phase = step as f32 * 0.7;
                for k in 1..(pts.len().saturating_sub(1)) {
                    let cx = pts[k].0 / cfg.dx_m;
                    let cz = pts[k].1 / cfg.dx_m;
                    let r0 = (libm::floorf(cz - rad_fp_cells) as isize).max(1) as usize;
                    let r1 = (libm::ceilf(cz + rad_fp_cells) as isize).min(h as isize - 2) as usize;
                    let c0 = (libm::floorf(cx - rad_fp_cells) as isize).max(1) as usize;
                    let c1 = (libm::ceilf(cx + rad_fp_cells) as isize).min(w as isize - 2) as usize;
                    let sign_c = if curv[k] >= 0.0 { 1.0_f32 } else { -1.0_f32 };
                    let (nkx, nkz) = (nx[k], nz[k]);

                    for rr in r0..=r1 {
                        for cc in c0..=c1 {
                            let dx_c = cc as f32 - cx;
                            let dz_c = rr as f32 - cz;
                            let dist = libm::hypotf(dx_c, dz_c);
                            if dist > rad_fp_cells {
                                continue;
                            }
                            let nb = rr * w + cc;
                            if total_elev(bedrock_h, sediment_s, nb) <= cfg.sea_level_m + 0.5
                                || water_kind[nb] == WaterKind::FreshwaterLake as u8
                            {
                                continue;
                            }
                            let side = (dx_c * nkx + dz_c * nkz) * sign_c;
                            let radial = (1.0 - dist / rad_fp_cells).max(0.0);
                            alluvial_mask[nb] = alluvial_mask[nb].max(0.75 * radial);
                            if side < 0.0 {
                                // Inner point bar & concentric scroll bars.
                                let scroll_v = 0.5 + 0.5 * libm::sinf(dist * 2.1 + phase);
                                let scroll = (radial * scroll_v).clamp(0.0, 1.0);
                                meander_scroll[nb] = meander_scroll[nb].max(scroll);
                                sediment_s[nb] += 0.04 * scroll;
                            }
                        }
                    }
                }
            }
        }

        // Rasterize final evolved meander river centerline.
        let ch_rad = (0.5 * w_bank / cfg.dx_m).clamp(0.8, 2.5);
        for &(px, pz) in &pts {
            stamp_disk_river(
                bedrock_h,
                sediment_s,
                water_h_m,
                water_kind,
                (w, h),
                DiskStamp {
                    cx: px / cfg.dx_m,
                    cz: pz / cfg.dx_m,
                    rad_cells: ch_rad,
                    depth_m: mean_d.max(0.6),
                    wk: WaterKind::River as u8,
                },
            );
        }
    }

    // -------------------------------------------------------------------------
    // 3. DeltaRCM (`Liang et al. 2015`) Coastal Prograding Deltas (`AER-04/05`)
    // -------------------------------------------------------------------------
    let mut delta_rng = stage_rng(cfg.seed, w as u32, h as u32, &cfg.schema_tag, "S4-DELT");
    let mut mouth_cands: Vec<(f32, usize, f32, f32)> = Vec::new();

    for r in 2..(h - 2) {
        for c in 2..(w - 2) {
            let idx = r * w + c;
            let e = total_elev(bedrock_h, sediment_s, idx);
            let q = input.discharge_q_m3_s[idx];
            if e <= cfg.sea_level_m
                || q < cfg.delta_min_q_m3_s
                || (!input.is_channel[idx] && water_kind[idx] != WaterKind::River as u8)
            {
                continue;
            }
            let mut touches_ocean = false;
            let mut off_x = 0.0_f32;
            let mut off_z = 0.0_f32;
            for dir in 0..8 {
                let nr = (r as isize + DZ8[dir]) as usize;
                let nc = (c as isize + DX8[dir]) as usize;
                let nb = nr * w + nc;
                if total_elev(bedrock_h, sediment_s, nb) <= cfg.sea_level_m {
                    touches_ocean = true;
                    off_x += UX8[dir];
                    off_z += UZ8[dir];
                }
            }
            if !touches_ocean {
                continue;
            }
            let (ux, uz) = if libm::hypotf(input.flow_dir_x[idx], input.flow_dir_z[idx]) > 0.1 {
                let l = libm::hypotf(input.flow_dir_x[idx], input.flow_dir_z[idx]);
                (input.flow_dir_x[idx] / l, input.flow_dir_z[idx] / l)
            } else {
                let l = libm::hypotf(off_x, off_z).max(1.0);
                (off_x / l, off_z / l)
            };

            // Measure offshore shelf depth along `(ux, uz)` within 5..=30 cells.
            let max_probe = ((400.0 / cfg.dx_m).round() as usize).clamp(4, w.min(h) / 3);
            let mut depth_sum = 0.0_f32;
            let mut depth_cnt = 0_usize;
            for step in 2..=max_probe {
                let pr = (r as f32 + uz * step as f32).round() as isize;
                let pc = (c as f32 + ux * step as f32).round() as isize;
                if pr < 0 || pc < 0 || pr >= h as isize || pc >= w as isize {
                    break;
                }
                let pe = total_elev(bedrock_h, sediment_s, pr as usize * w + pc as usize);
                if pe <= cfg.sea_level_m {
                    depth_sum += cfg.sea_level_m - pe;
                    depth_cnt += 1;
                }
            }
            if depth_cnt > 0 {
                let mean_depth = depth_sum / depth_cnt as f32;
                if mean_depth < cfg.delta_max_shelf_depth_m {
                    mouth_cands.push((q, idx, ux, uz));
                }
            }
        }
    }

    mouth_cands.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let mouth_sp_sq = 2_000.0_f32 * 2_000.0_f32;
    let mut delta_count = 0usize;
    let mut selected_mouths: Vec<(f32, usize, f32, f32)> = Vec::new();

    for cand in mouth_cands {
        if selected_mouths.len() >= cfg.delta_max_mouths {
            break;
        }
        let mr = (cand.1 / w) as f32 * cfg.dx_m;
        let mc = (cand.1 % w) as f32 * cfg.dx_m;
        let too_close = selected_mouths.iter().any(|&(_, m_idx, _, _)| {
            let ar = (m_idx / w) as f32 * cfg.dx_m;
            let ac = (m_idx % w) as f32 * cfg.dx_m;
            (mr - ar) * (mr - ar) + (mc - ac) * (mc - ac) < mouth_sp_sq
        });
        if !too_close {
            selected_mouths.push(cand);
        }
    }

    for (q_mouth, m_idx, ux0, uz0) in selected_mouths {
        delta_count += 1;
        let mr = m_idx / w;
        let mc = m_idx % w;
        let half_roi = 64_usize.min(w / 2).min(h / 2);
        let r_min = mr.saturating_sub(half_roi);
        let r_max = (mr + half_roi).min(h - 1);
        let c_min = mc.saturating_sub(half_roi);
        let c_max = (mc + half_roi).min(w - 1);
        let rw = c_max - c_min + 1;
        let rh = r_max - r_min + 1;

        let mut roi_q = vec![0.0_f32; rw * rh];
        let mut roi_dep = vec![0.0_f32; rw * rh];
        let total_parcels = cfg.delta_parcels_per_mouth.clamp(500, 4_000);
        let n_water = (total_parcels * 3) / 5;
        let n_sed = total_parcels - n_water;
        let max_hops = 180_usize;

        let sm_r = mr - r_min;
        let sm_c = mc - c_min;

        // Water parcels (Liang et al. 2015 weighted random walk).
        for _ in 0..n_water {
            let mut cr = sm_r;
            let mut cc = sm_c;
            let mut vx = ux0;
            let mut vz = uz0;
            for _ in 0..max_hops {
                let loc = cr * rw + cc;
                roi_q[loc] += 1.0;

                let mut weights = [0.0_f32; 8];
                let mut nbs = [(0_usize, 0_usize); 8];
                let mut w_sum = 0.0_f32;
                let mut cnt = 0_usize;
                for dir in 0..8 {
                    let nr = cr as isize + DZ8[dir];
                    let nc = cc as isize + DX8[dir];
                    if nr < 0 || nc < 0 || nr >= rh as isize || nc >= rw as isize {
                        continue;
                    }
                    let nru = nr as usize;
                    let ncu = nc as usize;
                    let g_idx = (r_min + nru) * w + (c_min + ncu);
                    let e_nb = total_elev(bedrock_h, sediment_s, g_idx) + roi_dep[nru * rw + ncu];
                    let depth = (cfg.sea_level_m + 1.5 - e_nb).max(0.05);
                    let align = (vx * UX8[dir] + vz * UZ8[dir]).max(0.0);
                    let wt = depth * (0.15 + 0.85 * align);
                    if wt > 0.0 {
                        weights[cnt] = wt;
                        nbs[cnt] = (nru, ncu);
                        w_sum += wt;
                        cnt += 1;
                    }
                }
                if cnt == 0 || w_sum <= 0.0 {
                    break;
                }
                let pick = delta_rng.gen::<f32>() * w_sum;
                let mut acc = 0.0_f32;
                let mut chosen = nbs[cnt - 1];
                for k in 0..cnt {
                    acc += weights[k];
                    if pick <= acc {
                        chosen = nbs[k];
                        break;
                    }
                }
                let dx = chosen.1 as f32 - cc as f32;
                let dz = chosen.0 as f32 - cr as f32;
                let l = libm::hypotf(dx, dz).max(1.0);
                vx = 0.55 * vx + 0.45 * (dx / l);
                vz = 0.55 * vz + 0.45 * (dz / l);
                let vl = libm::hypotf(vx, vz).max(1e-5);
                vx /= vl;
                vz /= vl;
                cr = chosen.0;
                cc = chosen.1;
                if cr == 0 || cr + 1 == rh || cc == 0 || cc + 1 == rw {
                    break;
                }
            }
        }

        // Sediment parcels aggrade shallow delta lobes where flow decelerates offshore.
        let max_q_roi = roi_q.iter().copied().fold(1.0_f32, f32::max);
        for _ in 0..n_sed {
            let mut cr = sm_r;
            let mut cc = sm_c;
            let mut sed_load = 1.0_f32;
            for hop in 0..max_hops {
                let loc = cr * rw + cc;
                let g_idx = (r_min + cr) * w + (c_min + cc);
                let e_here = total_elev(bedrock_h, sediment_s, g_idx) + roi_dep[loc];
                let q_norm = roi_q[loc] / max_q_roi;

                if hop > 1 && e_here < cfg.sea_level_m + 1.2 && q_norm < 0.35 && sed_load > 0.05 {
                    let dep = 0.08 * sed_load * (1.0 - q_norm);
                    roi_dep[loc] += dep;
                    sed_load -= dep;
                }

                // Step along highest weighted neighbor in `roi_q`.
                let mut best_wt = 0.0_f32;
                let mut sum_wt = 0.0_f32;
                let mut nbs = [(0_usize, 0_usize); 8];
                let mut wts = [0.0_f32; 8];
                let mut cnt = 0usize;
                for dir in 0..8 {
                    let nr = cr as isize + DZ8[dir];
                    let nc = cc as isize + DX8[dir];
                    if nr < 0 || nc < 0 || nr >= rh as isize || nc >= rw as isize {
                        continue;
                    }
                    let nru = nr as usize;
                    let ncu = nc as usize;
                    let wt = roi_q[nru * rw + ncu] + 0.02;
                    let align = ((ncu as f32 - sm_c as f32) * ux0
                        + (nru as f32 - sm_r as f32) * uz0)
                        .max(0.05);
                    let w_tot = wt * align;
                    nbs[cnt] = (nru, ncu);
                    wts[cnt] = w_tot;
                    sum_wt += w_tot;
                    best_wt = best_wt.max(w_tot);
                    cnt += 1;
                }
                if cnt == 0 || sum_wt <= 0.0 {
                    break;
                }
                let pick = delta_rng.gen::<f32>() * sum_wt;
                let mut acc = 0.0_f32;
                let mut chosen = nbs[cnt - 1];
                for k in 0..cnt {
                    acc += wts[k];
                    if pick <= acc {
                        chosen = nbs[k];
                        break;
                    }
                }
                cr = chosen.0;
                cc = chosen.1;
                if cr == 0 || cr + 1 == rh || cc == 0 || cc + 1 == rw {
                    break;
                }
            }
        }

        // Write ROI deposits, alluvial mask, and offshore turbidity plume (`WaterKind::DeltaTurbid = 6`).
        let plume_scale = (220.0 / cfg.dx_m).max(8.0);
        let q_scale = (q_mouth / 20.0).clamp(0.25, 1.0);
        for rr in 0..rh {
            for cc in 0..rw {
                let loc = rr * rw + cc;
                let g_idx = (r_min + rr) * w + (c_min + cc);
                let dep = roi_dep[loc].min(8.0);
                if dep > 0.0 {
                    sediment_s[g_idx] += dep;
                    alluvial_mask[g_idx] = alluvial_mask[g_idx].max((dep / 1.5).clamp(0.0, 1.0));
                }
                let dist_cells = libm::hypotf(cc as f32 - sm_c as f32, rr as f32 - sm_r as f32);
                let q_rel = (roi_q[loc] / max_q_roi).clamp(0.0, 1.0);
                let plume = libm::expf(-dist_cells / plume_scale)
                    * (0.35 + 0.65 * q_rel)
                    * (0.45 + 0.55 * q_scale);
                let surf = total_elev(bedrock_h, sediment_s, g_idx);
                if surf <= cfg.sea_level_m + 0.2 {
                    water_turbidity[g_idx] = water_turbidity[g_idx].max(plume.clamp(0.0, 1.0));
                    if water_turbidity[g_idx] >= 0.15 {
                        water_kind[g_idx] = WaterKind::DeltaTurbid as u8;
                        water_h_m[g_idx] = water_h_m[g_idx].max(cfg.sea_level_m);
                    }
                } else if dep > 0.2 && q_rel > 0.25 {
                    water_kind[g_idx] = WaterKind::River as u8;
                    water_h_m[g_idx] = water_h_m[g_idx].max(surf + 0.35);
                }
            }
        }
    }

    Ok(DepositionField {
        width: w,
        height: h,
        alluvial_mask,
        meander_scroll,
        water_turbidity,
        fan_count: accepted_fans.len(),
        meander_reach_count,
        oxbow_count,
        delta_count,
        micro_pond_count: 0,
    })
}

fn resample_polyline(pts: &[(f32, f32)], ds: f32) -> Vec<(f32, f32)> {
    if pts.len() < 2 {
        return pts.to_vec();
    }
    let mut cum = vec![0.0_f32; pts.len()];
    for i in 1..pts.len() {
        cum[i] = cum[i - 1] + libm::hypotf(pts[i].0 - pts[i - 1].0, pts[i].1 - pts[i - 1].1);
    }
    let total = cum[pts.len() - 1];
    if total < ds * 2.0 {
        return pts.to_vec();
    }
    let steps = (total / ds).floor() as usize;
    let mut out = Vec::with_capacity(steps + 1);
    let mut seg = 0usize;
    for k in 0..=steps {
        let target = (k as f32 * ds).min(total);
        while seg + 1 < pts.len() - 1 && cum[seg + 1] < target {
            seg += 1;
        }
        let span = (cum[seg + 1] - cum[seg]).max(1e-5);
        let t = ((target - cum[seg]) / span).clamp(0.0, 1.0);
        out.push((
            pts[seg].0 + t * (pts[seg + 1].0 - pts[seg].0),
            pts[seg].1 + t * (pts[seg + 1].1 - pts[seg].1),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alluvial_fan_canyon_slope_drop_and_wet_vs_dry_braids() {
        let n = 96;
        let dx_m = 10.0_f32;
        let mut bedrock = vec![0.0_f32; n * n];
        let mut sed = vec![4.0_f32; n * n];
        let mut water_h = vec![0.0_f32; n * n];
        let mut water_kind = vec![0_u8; n * n];
        let mut q = vec![0.0_f32; n * n];
        let mut rw = vec![0.0_f32; n * n];
        let mut rd = vec![0.0_f32; n * n];
        let mut fx = vec![1.0_f32; n * n];
        let mut fz = vec![0.0_f32; n * n];
        let mut is_ch = vec![false; n * n];

        // Steep canyon (c < 36, slope 0.16) debouching onto flat piedmont (c >= 36, slope 0.015).
        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                let base_e = if c < 36 {
                    300.0 + (36 - c) as f32 * 1.6
                } else {
                    300.0 - (c - 36) as f32 * 0.15
                };
                let bank = if c < 36 {
                    (r as f32 - 48.0).abs() * 0.8
                } else {
                    (r as f32 - 48.0).abs() * 0.02
                };
                bedrock[idx] = base_e + bank;
                water_h[idx] = bedrock[idx] + sed[idx];
                if r == 48 && (2..(n - 2)).contains(&c) {
                    is_ch[idx] = true;
                    q[idx] = 6.0;
                    rw[idx] = 7.0;
                    rd[idx] = 0.6;
                    fx[idx] = 1.0;
                    fz[idx] = 0.0;
                }
            }
        }
        let sed_before = sed.clone();
        let input = DepositionInput {
            width: n,
            height: n,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            river_d_m: &rd,
            flow_dir_x: &fx,
            flow_dir_z: &fz,
            is_channel: &is_ch,
            d8_receiver: None,
        };
        let cfg = DepositionConfig {
            dx_m,
            ..DepositionConfig::default()
        };
        let field = apply_deposition(
            &mut bedrock,
            &mut sed,
            &mut water_h,
            &mut water_kind,
            &input,
            &cfg,
        )
        .expect("fan");

        assert!(field.fan_count >= 1, "expected at least 1 alluvial fan");
        let deposited_cells = sed
            .iter()
            .zip(sed_before.iter())
            .filter(|(&a, &b)| a > b + 0.1)
            .count();
        assert!(
            deposited_cells >= 20,
            "expected radial sediment deposition on piedmont"
        );
        assert!(field.alluvial_mask.iter().any(|&m| m > 0.5));
        let dry_braid_cells = (0..n * n)
            .filter(|&i| field.alluvial_mask[i] > 0.3 && water_kind[i] == WaterKind::None as u8)
            .count();
        let wet_braid_cells = (0..n * n)
            .filter(|&i| field.alluvial_mask[i] > 0.3 && water_kind[i] == WaterKind::River as u8)
            .count();
        assert!(wet_braid_cells > 0 && dry_braid_cells > 0);
    }

    #[test]
    fn howard_knutson_35_step_meander_scrolls_and_oxbow() {
        let n = 128;
        let dx_m = 10.0_f32;
        let mut bedrock = vec![40.0_f32; n * n];
        let mut sed = vec![6.0_f32; n * n];
        let mut water_h = vec![46.0_f32; n * n];
        let mut water_kind = vec![0_u8; n * n];
        let mut q = vec![0.0_f32; n * n];
        let mut rw = vec![0.0_f32; n * n];
        let mut rd = vec![0.0_f32; n * n];
        let fx = vec![1.0_f32; n * n];
        let fz = vec![0.0_f32; n * n];
        let mut is_ch = vec![false; n * n];
        let mut d8_rec = vec![crate::hydrology::NO_RECEIVER; n * n];

        // Gentle 0.002 slope floodplain with a mature omega meander loop so 35 steps trigger neck cutoff.
        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                bedrock[idx] = 50.0 - 0.02 * c as f32;
                water_h[idx] = bedrock[idx] + sed[idx];
            }
        }
        let mut chain: Vec<usize> = Vec::new();
        for c in 10..=61 {
            chain.push(64 * n + c);
        }
        // Omega loop around (c=63, r=81) exiting at (c=65, r=64) — 40 m initial neck gap.
        for k in 1..48 {
            let frac = k as f32 / 48.0;
            let th = frac * std::f32::consts::TAU;
            let c = (63.0 + (frac - 0.5) * 4.0 - 11.0 * libm::sinf(th))
                .round()
                .clamp(10.0, 118.0) as usize;
            let r = (65.0 + 16.0 * (1.0 - libm::cosf(th))).round() as usize;
            let idx = r * n + c;
            if !chain.contains(&idx) {
                chain.push(idx);
            }
        }
        for c in 65..=114 {
            let idx = 64 * n + c;
            if !chain.contains(&idx) {
                chain.push(idx);
            }
        }
        // Densify D8 connectivity between chain points without revisiting cells.
        let mut full_chain: Vec<usize> = Vec::new();
        for w_pair in chain.windows(2) {
            let mut cr = (w_pair[0] / n) as isize;
            let mut cc = (w_pair[0] % n) as isize;
            let tr = (w_pair[1] / n) as isize;
            let tc = (w_pair[1] % n) as isize;
            while cr != tr || cc != tc {
                let idx = cr as usize * n + cc as usize;
                if !full_chain.contains(&idx) {
                    full_chain.push(idx);
                }
                cr += (tr - cr).signum();
                cc += (tc - cc).signum();
            }
        }
        if let Some(&last) = chain.last() {
            if !full_chain.contains(&last) {
                full_chain.push(last);
            }
        }
        for pair in full_chain.windows(2) {
            let u = pair[0];
            let v = pair[1];
            d8_rec[u] = v as u32;
            is_ch[u] = true;
            q[u] = 12.0;
            rw[u] = 12.0;
            rd[u] = 0.8;
        }

        let input = DepositionInput {
            width: n,
            height: n,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            river_d_m: &rd,
            flow_dir_x: &fx,
            flow_dir_z: &fz,
            is_channel: &is_ch,
            d8_receiver: Some(&d8_rec),
        };
        let cfg = DepositionConfig {
            dx_m,
            meander_steps: 35,
            ..DepositionConfig::default()
        };
        let field = apply_deposition(
            &mut bedrock,
            &mut sed,
            &mut water_h,
            &mut water_kind,
            &input,
            &cfg,
        )
        .expect("meander");

        assert!(field.meander_reach_count >= 1);
        assert!(field.meander_scroll.iter().any(|&s| s > 0.2));
        assert!(field.alluvial_mask.iter().any(|&a| a > 0.2));
        assert!(
            field.oxbow_count >= 1 && water_kind.contains(&(WaterKind::FreshwaterLake as u8)),
            "expected neck cutoff creating oxbow lake"
        );
    }

    #[test]
    fn deltarcm_shallow_shelf_progradation_and_turbidity() {
        let n = 96;
        let dx_m = 10.0_f32;
        let mut bedrock = vec![0.0_f32; n * n];
        let mut sed = vec![0.0_f32; n * n];
        let mut water_h = vec![0.0_f32; n * n];
        let mut water_kind = vec![0_u8; n * n];
        let mut q = vec![0.0_f32; n * n];
        let mut rw = vec![0.0_f32; n * n];
        let mut rd = vec![0.0_f32; n * n];
        let mut fx = vec![1.0_f32; n * n];
        let fz = vec![0.0_f32; n * n];
        let mut is_ch = vec![false; n * n];

        // Land on c < 36, shallow shelf (-6 m) on c >= 36.
        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                if c < 36 {
                    bedrock[idx] = 2.0 + (36 - c) as f32 * 0.2;
                    sed[idx] = 2.0;
                    water_h[idx] = bedrock[idx] + sed[idx];
                } else {
                    bedrock[idx] = -6.0 - (c - 36) as f32 * 0.15;
                    water_h[idx] = 0.0;
                }
                if r == 48 && c <= 35 {
                    is_ch[idx] = true;
                    q[idx] = 18.0;
                    rw[idx] = 14.0;
                    rd[idx] = 1.0;
                    fx[idx] = 1.0;
                }
            }
        }
        let sed_before = sed.clone();
        let input = DepositionInput {
            width: n,
            height: n,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            river_d_m: &rd,
            flow_dir_x: &fx,
            flow_dir_z: &fz,
            is_channel: &is_ch,
            d8_receiver: None,
        };
        let cfg = DepositionConfig {
            dx_m,
            ..DepositionConfig::default()
        };
        let field = apply_deposition(
            &mut bedrock,
            &mut sed,
            &mut water_h,
            &mut water_kind,
            &input,
            &cfg,
        )
        .expect("delta");

        assert!(field.delta_count >= 1);
        let aggraded = (0..n * n)
            .filter(|&i| (i % n) >= 36 && sed[i] > sed_before[i] + 0.05)
            .count();
        assert!(
            aggraded > 0,
            "expected delta sediment aggradation on shallow shelf"
        );
        assert!(field.water_turbidity.iter().any(|&t| t > 0.2));
        assert!(water_kind.contains(&(WaterKind::DeltaTurbid as u8)));
    }

    #[test]
    fn negative_control_deep_shelf_or_steep_reach_suppresses_deltas_and_meanders() {
        let n = 64;
        let dx_m = 10.0_f32;
        let mut bedrock = vec![0.0_f32; n * n];
        let mut sed = vec![2.0_f32; n * n];
        let mut water_h = vec![0.0_f32; n * n];
        let mut water_kind = vec![0_u8; n * n];
        let mut q = vec![0.0_f32; n * n];
        let mut rw = vec![0.0_f32; n * n];
        let mut rd = vec![0.0_f32; n * n];
        let fx = vec![1.0_f32; n * n];
        let fz = vec![0.0_f32; n * n];
        let mut is_ch = vec![false; n * n];

        // Steep land S = 0.06 (>= 0.004) dropping directly onto 200 m deep trench (>= 40 m).
        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                if c < 32 {
                    bedrock[idx] = 10.0 + (32 - c) as f32 * 0.6;
                    water_h[idx] = bedrock[idx] + sed[idx];
                } else {
                    bedrock[idx] = -200.0;
                    water_h[idx] = 0.0;
                }
                if r == 32 && c < 32 {
                    is_ch[idx] = true;
                    q[idx] = 15.0;
                    rw[idx] = 10.0;
                    rd[idx] = 0.8;
                }
            }
        }
        let input = DepositionInput {
            width: n,
            height: n,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            river_d_m: &rd,
            flow_dir_x: &fx,
            flow_dir_z: &fz,
            is_channel: &is_ch,
            d8_receiver: None,
        };
        let cfg = DepositionConfig {
            dx_m,
            ..DepositionConfig::default()
        };
        let field = apply_deposition(
            &mut bedrock,
            &mut sed,
            &mut water_h,
            &mut water_kind,
            &input,
            &cfg,
        )
        .expect("neg control");

        assert_eq!(
            field.delta_count, 0,
            "deep shelf >= 40 m must suppress deltas"
        );
        assert_eq!(
            field.meander_reach_count, 0,
            "steep reach S >= 0.004 must suppress meanders"
        );
    }

    #[test]
    fn deposition_does_not_stamp_synthetic_upland_micro_ponds() {
        let n = 64;
        let dx_m = 10.0_f32;
        let mut bedrock = vec![3_600.0_f32; n * n];
        let mut sed = vec![4.0_f32; n * n];
        let mut water_h = vec![3_604.0_f32; n * n];
        let mut water_kind = vec![0_u8; n * n];
        let q = vec![0.1_f32; n * n];
        let rw = vec![0.0_f32; n * n];
        let rd = vec![0.0_f32; n * n];
        let fx = vec![1.0_f32; n * n];
        let fz = vec![0.0_f32; n * n];
        let is_ch = vec![false; n * n];

        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                bedrock[idx] = 3_600.0 + c as f32 * 0.05;
                water_h[idx] = bedrock[idx] + sed[idx];
            }
        }
        let input = DepositionInput {
            width: n,
            height: n,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            river_d_m: &rd,
            flow_dir_x: &fx,
            flow_dir_z: &fz,
            is_channel: &is_ch,
            d8_receiver: None,
        };
        let cfg = DepositionConfig {
            dx_m,
            ..DepositionConfig::default()
        };
        assert_eq!(cfg.target_micro_ponds, 0);
        let field = apply_deposition(
            &mut bedrock,
            &mut sed,
            &mut water_h,
            &mut water_kind,
            &input,
            &cfg,
        )
        .expect("deposition");

        assert_eq!(field.micro_pond_count, 0);
        assert!(!water_kind.contains(&(WaterKind::FreshwaterLake as u8)));
    }

    #[test]
    fn s4_to_s6_chain_preserves_coherent_basins_and_step4_cutoff_scrolls() {
        // 1. Pre-existing upland depression chained from S4 -> S6 leaves zero FreshwaterLake cells with lake_id == 0.
        let n = 64;
        let dx_m = 10.0_f32;
        let mut bedrock = vec![0.0_f32; n * n];
        let mut sed = vec![4.0_f32; n * n];
        let mut water_h = vec![0.0_f32; n * n];
        let mut water_kind = vec![0_u8; n * n];
        let q = vec![0.05_f32; n * n];
        let rw = vec![0.0_f32; n * n];
        let rd = vec![0.0_f32; n * n];
        let fx = vec![1.0_f32; n * n];
        let fz = vec![0.0_f32; n * n];
        let is_ch = vec![false; n * n];

        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                if r == 0 || r + 1 == n || c == 0 || c + 1 == n {
                    bedrock[idx] = -5.0;
                    sed[idx] = 0.0;
                } else {
                    let d = libm::hypotf(r as f32 - 32.0, c as f32 - 32.0);
                    bedrock[idx] =
                        3_560.0 + c as f32 * 0.08 - 18.0 * libm::expf(-(d / 7.0) * (d / 7.0));
                }
                water_h[idx] = bedrock[idx] + sed[idx];
            }
        }
        let s4_in = DepositionInput {
            width: n,
            height: n,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            river_d_m: &rd,
            flow_dir_x: &fx,
            flow_dir_z: &fz,
            is_channel: &is_ch,
            d8_receiver: None,
        };
        let s4_cfg = DepositionConfig {
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
        assert_eq!(s4.micro_pond_count, 0);

        let mut total_e: Vec<f32> = bedrock
            .iter()
            .zip(sed.iter())
            .map(|(&b, &s)| b + s)
            .collect();
        let precip = vec![2.2_f32; n * n];
        let pet = vec![0.6_f32; n * n];
        let lith = vec![crate::tectonic::LithologyClass::GraniteCrystalline as u8; n * n];
        let s6_in = crate::lakes_falls::LakesFallsInput {
            width: n,
            height: n,
            precip_m_yr: &precip,
            pet_m_yr: &pet,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            is_channel: &is_ch,
            lithology_id: &lith,
        };
        let s6 = crate::lakes_falls::generate_lakes_and_waterfalls(
            &mut total_e,
            &mut sed,
            &mut water_h,
            &mut water_kind,
            &s6_in,
            &crate::lakes_falls::LakesFallsConfig::default(),
        )
        .expect("s6 after s4");
        assert!(!s6.basins.is_empty(), "S6 should retain S4 upland ponds");
        for (i, &wk_i) in water_kind.iter().enumerate() {
            if wk_i == WaterKind::FreshwaterLake as u8 {
                assert_ne!(
                    s6.lake_id[i], 0,
                    "S4->S6 cell {i} marked FreshwaterLake without S6 lake_id"
                );
            }
        }

        // 2. Step-4 cutoff (step % 5 == 4) recomputes scroll normals cleanly without panic or corruption.
        let m = 128;
        let mut m_bed = vec![40.0_f32; m * m];
        let mut m_sed = vec![6.0_f32; m * m];
        let mut m_wh = vec![46.0_f32; m * m];
        let mut m_wk = vec![0_u8; m * m];
        let mut m_q = vec![0.0_f32; m * m];
        let mut m_rw = vec![0.0_f32; m * m];
        let mut m_rd = vec![0.0_f32; m * m];
        let m_fx = vec![1.0_f32; m * m];
        let m_fz = vec![0.0_f32; m * m];
        let mut m_ch = vec![false; m * m];
        let mut m_d8 = vec![crate::hydrology::NO_RECEIVER; m * m];
        for r in 0..m {
            for c in 0..m {
                let idx = r * m + c;
                m_bed[idx] = 50.0 - 0.02 * c as f32;
                m_wh[idx] = m_bed[idx] + m_sed[idx];
            }
        }
        let mut chain: Vec<usize> = Vec::new();
        for c in 10..=62 {
            chain.push(64 * m + c);
        }
        for k in 1..48 {
            let frac = k as f32 / 48.0;
            let th = frac * std::f32::consts::TAU;
            let c = (63.5 + (frac - 0.5) * 2.0 - 11.0 * libm::sinf(th))
                .round()
                .clamp(10.0, 118.0) as usize;
            let r = (65.0 + 16.0 * (1.0 - libm::cosf(th))).round() as usize;
            let idx = r * m + c;
            if !chain.contains(&idx) {
                chain.push(idx);
            }
        }
        for c in 64..=114 {
            let idx = 64 * m + c;
            if !chain.contains(&idx) {
                chain.push(idx);
            }
        }
        let mut full: Vec<usize> = Vec::new();
        for w_pair in chain.windows(2) {
            let mut cr = (w_pair[0] / m) as isize;
            let mut cc = (w_pair[0] % m) as isize;
            let tr = (w_pair[1] / m) as isize;
            let tc = (w_pair[1] % m) as isize;
            while cr != tr || cc != tc {
                let idx = cr as usize * m + cc as usize;
                if !full.contains(&idx) {
                    full.push(idx);
                }
                cr += (tr - cr).signum();
                cc += (tc - cc).signum();
            }
        }
        if let Some(&last) = chain.last() {
            if !full.contains(&last) {
                full.push(last);
            }
        }
        for pair in full.windows(2) {
            m_d8[pair[0]] = pair[1] as u32;
            m_ch[pair[0]] = true;
            m_q[pair[0]] = 12.0;
            m_rw[pair[0]] = 12.0;
            m_rd[pair[0]] = 0.8;
        }
        let m_in = DepositionInput {
            width: m,
            height: m,
            discharge_q_m3_s: &m_q,
            river_w_m: &m_rw,
            river_d_m: &m_rd,
            flow_dir_x: &m_fx,
            flow_dir_z: &m_fz,
            is_channel: &m_ch,
            d8_receiver: Some(&m_d8),
        };
        let m_cfg = DepositionConfig {
            dx_m: 10.0,
            meander_steps: 5, // step 4 is `step % 5 == 4`
            ..DepositionConfig::default()
        };
        let m_out = apply_deposition(&mut m_bed, &mut m_sed, &mut m_wh, &mut m_wk, &m_in, &m_cfg)
            .expect("step4 meander");
        assert!(m_out.meander_scroll.iter().all(|v| v.is_finite()));
        assert!(m_out.meander_scroll.iter().any(|&v| v > 0.1));
    }

    #[test]
    fn sub_half_meter_channel_rills_stay_dry_water_kind_none() {
        let n = 16;
        let mut bed = vec![50.0_f32; n * n];
        let mut sed = vec![0.0_f32; n * n];
        let mut wh = bed.clone();
        let mut wk = vec![WaterKind::None as u8; n * n];
        let mut q = vec![0.0_f32; n * n];
        let mut rw = vec![0.0_f32; n * n];
        let rd = vec![0.1_f32; n * n];
        let fx = vec![1.0_f32; n * n];
        let fz = vec![0.0_f32; n * n];
        let mut is_ch = vec![false; n * n];

        let rill_idx = 5 * n + 5;
        let river_idx = 5 * n + 8;
        is_ch[rill_idx] = true;
        rw[rill_idx] = 0.49;
        q[rill_idx] = 0.02;
        is_ch[river_idx] = true;
        rw[river_idx] = 0.50;
        q[river_idx] = 0.30;

        let input = DepositionInput {
            width: n,
            height: n,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            river_d_m: &rd,
            flow_dir_x: &fx,
            flow_dir_z: &fz,
            is_channel: &is_ch,
            d8_receiver: None,
        };
        apply_deposition(
            &mut bed,
            &mut sed,
            &mut wh,
            &mut wk,
            &input,
            &DepositionConfig::default(),
        )
        .expect("deposition rill test");

        assert_eq!(wk[rill_idx], WaterKind::None as u8);
        assert_eq!(wk[river_idx], WaterKind::River as u8);
    }
}
