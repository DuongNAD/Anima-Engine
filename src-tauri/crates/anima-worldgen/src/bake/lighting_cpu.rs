#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

//! `JOB-W4` (`--backend cpu`): Deterministic bounded-radius (`horizon_radius = 256` cells)
//! Stewart (1998) blocked upper-hull horizon AO, sky visibility (`svf`), indirect bounce luma,
//! 4-azimuth directional horizon elevation angles `(theta_E, theta_W, theta_N, theta_S)`, and
//! `512 × 512` sky/bounce probe (`§3.2` #3, `§6.2`).

use crate::bake::{
    build_albedo_luma_grid, compute_macro_albedo_rgb, quantize_u8, BakeError, TileRange,
    HORIZON_HALO_CELLS, TILE_ACTIVE_SIZE,
};
use crate::pipeline::{read_bin_f32, read_bin_u8, RawWorldMeta};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::f32::consts::{FRAC_PI_2, SQRT_2};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

const IO_CHUNK_BYTES: usize = 65_536;
const CHUNK_LEN: usize = 16;

/// Eight canonical azimuth steps `(dc, dr, step_len_mult)`.
pub const AZIMUTH_DIRS: [(isize, isize, f32); 8] = [
    (1, 0, 1.0),      // 0: East  (+X)
    (-1, 0, 1.0),     // 1: West  (-X)
    (0, 1, 1.0),      // 2: South (+Z)
    (0, -1, 1.0),     // 3: North (-Z)
    (1, 1, SQRT_2),   // 4: SE
    (-1, 1, SQRT_2),  // 5: SW
    (1, -1, SQRT_2),  // 6: NE
    (-1, -1, SQRT_2), // 7: NW
];

/// In-memory result of a CPU or GPU lighting pass (`terrain_lighting`, `terrain_horizon`, `512²` probe).
#[derive(Debug, Clone, PartialEq)]
pub struct LightingBakeOutput {
    pub width: usize,
    pub height: usize,
    /// Active tile offset `(tx0, ty0)` and tile count `(tiles_w, tiles_h)` when sharded (`0, 0, w/256, h/256` when full).
    pub tile_origin_x: usize,
    pub tile_origin_y: usize,
    pub out_width: usize,
    pub out_height: usize,
    /// `RGBA8`: `(ao, svf, bounce_luma, 255)` for `out_width × out_height`.
    pub terrain_lighting: Vec<[u8; 4]>,
    /// `RGBA8`: 4-azimuth directional horizon elevation angles `(theta_E, theta_W, theta_N, theta_S)` linearly quantized `[0, pi/2] -> [0, 255]`.
    pub terrain_horizon: Vec<[u8; 4]>,
    /// `512 × 512` `RGBA8` indirect bounce + sky visibility probe (present only for full-grid runs).
    pub sky_bounce_probe: Vec<[u8; 4]>,
}

/// JSON metadata emitted by `anima-bake lighting`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LightingBakeReport {
    pub backend: String,
    pub width: u32,
    pub height: u32,
    pub dx_m: f32,
    pub horizon_radius_cells: usize,
    pub tile_range_start: Option<usize>,
    pub tile_range_end: Option<usize>,
    pub lighting_sha256: String,
    pub horizon_sha256: String,
    pub probe_sha256: Option<String>,
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0F) as usize] as char);
    }
    s
}

#[inline]
pub(crate) fn pack_lighting_texel(max_tan: &[f32; 8], albedo_luma: f32) -> ([u8; 4], [u8; 4]) {
    let mut sum_cos2 = 0.0_f32;
    let mut sum_one_minus_sin = 0.0_f32;
    let mut card_u8 = [0_u8; 4];

    for (k, &t) in max_tan.iter().enumerate() {
        let t_pos = if t > 0.0 { t } else { 0.0 };
        let theta = libm::atanf(t_pos);
        let sin_t = libm::sinf(theta);
        let cos_t = libm::cosf(theta);
        sum_cos2 += cos_t * cos_t;
        sum_one_minus_sin += (1.0 - sin_t).clamp(0.0, 1.0);
        if k < 4 {
            // Card 6 ordering: [E, W, N, S], while AZIMUTH_DIRS is [E, W, S, N].
            let out_slot = match k {
                0 => 0, // E
                1 => 1, // W
                2 => 3, // S -> slot 3
                _ => 2, // N -> slot 2
            };
            card_u8[out_slot] = quantize_u8((theta / FRAC_PI_2).clamp(0.0, 1.0));
        }
    }

    let svf = (sum_cos2 * 0.125).clamp(0.0, 1.0);
    let ao = (sum_one_minus_sin * 0.125).clamp(0.0, 1.0);
    let bounce_luma =
        ((1.0 - svf) * albedo_luma.clamp(0.0, 1.0) * (0.30 + 0.70 * svf)).clamp(0.0, 1.0);
    (
        [
            quantize_u8(ao),
            quantize_u8(svf),
            quantize_u8(bounce_luma),
            255,
        ],
        card_u8,
    )
}

/// Exact bounded-radius (`1..=radius`) maximum forward slope along 1-D slice `line[0..len]`,
/// combining fixed-block Stewart (1998) backward upper convex hull inside the near block `[x+1..B)`
/// with 16-cell max-height pruning inside `[B..x+R]`.
///
/// Mathematically identical (`0` approximation) to brute-force `max_{1 <= d <= R} (line[x+d]-line[x])/(d*step_m)`
/// for every `x` in `x_start..x_end`.
pub fn solve_1d_horizon_bounded_stewart(
    line: &[f32],
    x_start: usize,
    x_end: usize,
    radius: usize,
    step_m: f32,
    out_tan: &mut [f32],
) {
    let len = line.len();
    debug_assert_eq!(out_tan.len(), x_end.saturating_sub(x_start));
    if len == 0 || x_start >= x_end || radius == 0 {
        return;
    }

    let inv_step = 1.0 / step_m;
    let block_w = radius.max(1);

    // Precompute per-16-cell chunk maxima over the forward domain `x_start..(x_end + radius).min(len)`.
    let max_idx = (x_end + radius).min(len);
    let num_chunks = max_idx.div_ceil(CHUNK_LEN);
    let mut chunk_max = Vec::with_capacity(num_chunks);
    for ch in 0..num_chunks {
        let s = ch * CHUNK_LEN;
        let e = (s + CHUNK_LEN).min(max_idx);
        let mut mx = f32::NEG_INFINITY;
        for &v in &line[s..e] {
            if v > mx {
                mx = v;
            }
        }
        chunk_max.push(mx);
    }

    // Hull stack holds vertex indices strictly increasing from right-to-left? No: scanned backward,
    // stack bottom = rightmost (`v_0`), stack top = closest to `x` (`v_{m-1}`), so positions strictly decrease!
    let mut hull: Vec<usize> = Vec::with_capacity(block_w);

    // Process each global block [b_lo .. b_hi) aligned to `block_w` intersecting `x_start..x_end`.
    let first_blk = x_start / block_w;
    let last_blk = (x_end - 1) / block_w;
    for blk in first_blk..=last_blk {
        let b_lo = (blk * block_w).max(x_start);
        let b_edge = ((blk + 1) * block_w).min(len);
        let b_hi = b_edge.min(x_end);
        if b_lo >= b_hi {
            continue;
        }

        // Initialize hull across any cells in [b_hi .. b_edge) if `b_edge > b_hi` (clipped tile end).
        hull.clear();
        for pos in (b_hi..b_edge).rev() {
            let h_pos = line[pos];
            while hull.len() >= 2 {
                let top = hull[hull.len() - 1];
                let nxt = hull[hull.len() - 2];
                // Pop `top` if segment `(pos, nxt)` lies above or on `top`:
                // (h_top - h_pos)/(top - pos) <= (h_nxt - h_pos)/(nxt - pos)
                let lhs = (line[top] - h_pos) * ((nxt - pos) as f32);
                let rhs = (line[nxt] - h_pos) * ((top - pos) as f32);
                if lhs <= rhs {
                    hull.pop();
                } else {
                    break;
                }
            }
            hull.push(pos);
        }

        for x in (b_lo..b_hi).rev() {
            let hx = line[x];
            let mut best_slope = 0.0_f32;

            // 1. Evaluate Stewart upper convex hull on `[x + 1 .. b_edge)` in O(log H) binary search!
            if !hull.is_empty() {
                let mut lo = 0_usize;
                let mut hi = hull.len() - 1;
                while lo < hi {
                    let mid = (lo + hi) / 2;
                    let a = hull[mid];
                    let b = hull[mid + 1];
                    let sa = (line[a] - hx) / ((a - x) as f32);
                    let sb = (line[b] - hx) / ((b - x) as f32);
                    if sb >= sa {
                        lo = mid + 1;
                    } else {
                        hi = mid;
                    }
                }
                let p = hull[lo];
                let s = (line[p] - hx) / ((p - x) as f32);
                if s > best_slope {
                    best_slope = s;
                }
            }

            // 2. Push `(x, hx)` onto the block hull for earlier cells `x' < x`.
            while hull.len() >= 2 {
                let top = hull[hull.len() - 1];
                let nxt = hull[hull.len() - 2];
                let lhs = (line[top] - hx) * ((nxt - x) as f32);
                let rhs = (line[nxt] - hx) * ((top - x) as f32);
                if lhs <= rhs {
                    hull.pop();
                } else {
                    break;
                }
            }
            hull.push(x);

            // 3. Evaluate remaining horizon interval `[b_edge ..= (x + radius).min(len - 1)]`
            // using 16-cell upper-bound pruning (`100%` exact).
            let r_lo = b_edge;
            let r_hi = (x + radius + 1).min(len);
            if r_lo < r_hi {
                let first_full_ch = r_lo.div_ceil(CHUNK_LEN);
                let head_end = (first_full_ch * CHUNK_LEN).min(r_hi);
                for (j, &hj) in line.iter().enumerate().take(head_end).skip(r_lo) {
                    let s = (hj - hx) / ((j - x) as f32);
                    if s > best_slope {
                        best_slope = s;
                    }
                }

                let last_full_ch = r_hi / CHUNK_LEN;
                for (ch, &c_mx) in chunk_max
                    .iter()
                    .enumerate()
                    .take(last_full_ch)
                    .skip(first_full_ch)
                {
                    let c_start = ch * CHUNK_LEN;
                    let ub = (c_mx - hx) / ((c_start - x) as f32);
                    if ub > best_slope {
                        let c_end = c_start + CHUNK_LEN;
                        for (j, &hj) in line.iter().enumerate().take(c_end).skip(c_start) {
                            let s = (hj - hx) / ((j - x) as f32);
                            if s > best_slope {
                                best_slope = s;
                            }
                        }
                    }
                }

                let tail_start = (last_full_ch * CHUNK_LEN).max(head_end);
                for (j, &hj) in line.iter().enumerate().take(r_hi).skip(tail_start) {
                    let s = (hj - hx) / ((j - x) as f32);
                    if s > best_slope {
                        best_slope = s;
                    }
                }
            }

            out_tan[x - x_start] = best_slope * inv_step;
        }
    }
}

/// Computes `512 × 512` `RGBA8` indirect bounce + SVF sky probe from a full-grid lighting pass.
#[must_use]
pub fn compute_sky_bounce_probe_512(
    terrain_lighting: &[[u8; 4]],
    macro_albedo: &[[u8; 3]],
    width: usize,
    height: usize,
) -> Vec<[u8; 4]> {
    const PROBE_SIZE: usize = 512;
    let mut out = vec![[0_u8; 4]; PROBE_SIZE * PROBE_SIZE];
    for pr in 0..PROBE_SIZE {
        let gr0 = (pr * height) / PROBE_SIZE;
        let gr1 = (((pr + 1) * height) / PROBE_SIZE).max(gr0 + 1).min(height);
        for pc in 0..PROBE_SIZE {
            let gc0 = (pc * width) / PROBE_SIZE;
            let gc1 = (((pc + 1) * width) / PROBE_SIZE).max(gc0 + 1).min(width);
            let mut sr = 0_u64;
            let mut sg = 0_u64;
            let mut sb = 0_u64;
            let mut ssvf = 0_u64;
            let mut cnt = 0_u64;
            for gr in gr0..gr1 {
                for gc in gc0..gc1 {
                    let i = gr * width + gc;
                    let alb = macro_albedo[i];
                    let bounce = u64::from(terrain_lighting[i][2]);
                    sr += u64::from(alb[0]) * bounce;
                    sg += u64::from(alb[1]) * bounce;
                    sb += u64::from(alb[2]) * bounce;
                    ssvf += u64::from(terrain_lighting[i][1]);
                    cnt += 1;
                }
            }
            let denom = (cnt * 255).max(1);
            out[pr * PROBE_SIZE + pc] = [
                (sr / denom).min(255) as u8,
                (sg / denom).min(255) as u8,
                (sb / denom).min(255) as u8,
                (ssvf / cnt.max(1)).min(255) as u8,
            ];
        }
    }
    out
}

/// Computes exact CPU Stewart horizon lighting for either the full grid or a rectangular tile slice.
///
/// For any tile, reading a `256`-cell halo guarantees `100%` byte-identical outputs vs full-grid pass.
#[allow(clippy::too_many_arguments)]
pub fn bake_lighting_cpu_region(
    total_h_m: &[f32],
    albedo_luma: &[f32],
    macro_albedo: Option<&[[u8; 3]]>,
    width: usize,
    height: usize,
    dx_m: f32,
    horizon_radius: usize,
    region: Option<(usize, usize, usize, usize)>,
) -> Result<LightingBakeOutput, BakeError> {
    let n = width * height;
    if total_h_m.len() != n || albedo_luma.len() != n {
        return Err(BakeError::new(
            "lighting input buffer length does not match width*height",
        ));
    }
    let (rx0, ry0, rw, rh) = region.unwrap_or((0, 0, width, height));
    if rx0 + rw > width || ry0 + rh > height || rw == 0 || rh == 0 {
        return Err(BakeError::new("lighting region out of bounds or empty"));
    }
    let out_n = rw * rh;

    // Accumulate max_tan[8] row by row for rows ry0..(ry0 + rh), columns rx0..(rx0 + rw).
    // Evaluating each azimuth along global lines ensures bit-identical results regardless of region!
    let mut max_tan = vec![[0.0_f32; 8]; out_n];

    // 0: East (+X), 1: West (-X) — scan horizontal rows ry0..ry0+rh
    max_tan
        .par_chunks_mut(rw)
        .enumerate()
        .for_each(|(lr, row_out)| {
            let gr = ry0 + lr;
            let row_slice = &total_h_m[gr * width..(gr + 1) * width];
            let mut tan_e = vec![0.0_f32; rw];
            solve_1d_horizon_bounded_stewart(
                row_slice,
                rx0,
                rx0 + rw,
                horizon_radius,
                dx_m,
                &mut tan_e,
            );

            // Reverse row for West (-X): global c maps to len - 1 - c
            let mut rev_line = Vec::with_capacity(width);
            rev_line.extend(row_slice.iter().rev().copied());
            let rx_rev_start = width - (rx0 + rw);
            let rx_rev_end = width - rx0;
            let mut tan_w = vec![0.0_f32; rw];
            solve_1d_horizon_bounded_stewart(
                &rev_line,
                rx_rev_start,
                rx_rev_end,
                horizon_radius,
                dx_m,
                &mut tan_w,
            );

            for lc in 0..rw {
                row_out[lc][0] = tan_e[lc];
                row_out[lc][1] = tan_w[rw - 1 - lc];
            }
        });

    // 2: South (+Z), 3: North (-Z) — scan vertical columns rx0..rx0+rw
    let col_ns: Vec<(Vec<f32>, Vec<f32>)> = (0..rw)
        .into_par_iter()
        .map(|lc| {
            let gc = rx0 + lc;
            let mut col_buf = Vec::with_capacity(height);
            for gr in 0..height {
                col_buf.push(total_h_m[gr * width + gc]);
            }
            let mut tan_s = vec![0.0_f32; rh];
            solve_1d_horizon_bounded_stewart(
                &col_buf,
                ry0,
                ry0 + rh,
                horizon_radius,
                dx_m,
                &mut tan_s,
            );
            col_buf.reverse();
            let ry_rev_start = height - (ry0 + rh);
            let ry_rev_end = height - ry0;
            let mut tan_n = vec![0.0_f32; rh];
            solve_1d_horizon_bounded_stewart(
                &col_buf,
                ry_rev_start,
                ry_rev_end,
                horizon_radius,
                dx_m,
                &mut tan_n,
            );
            tan_n.reverse();
            (tan_s, tan_n)
        })
        .collect();

    for (lc, (tan_s, tan_n)) in col_ns.into_iter().enumerate() {
        for lr in 0..rh {
            let idx = lr * rw + lc;
            max_tan[idx][2] = tan_s[lr];
            max_tan[idx][3] = tan_n[lr];
        }
    }

    // Diagonals: 4 (SE: +1,+1), 5 (SW: -1,+1), 6 (NE: +1,-1), 7 (NW: -1,-1)
    let diag_step = dx_m * SQRT_2;
    let diag_se_nw: Vec<(isize, usize, Vec<f32>, Vec<f32>)> =
        ((ry0 as isize - (rx0 + rw - 1) as isize)..=((ry0 + rh - 1) as isize - rx0 as isize))
            .into_par_iter()
            .filter_map(|diff| {
                // Line satisfies r - c == diff.
                let c_min = 0_isize.max(-diff) as usize;
                let c_max = (width as isize - 1).min(height as isize - 1 - diff) as usize;
                if c_min > c_max {
                    return None;
                }
                // Intersect with active box [rx0..rx0+rw) × [ry0..ry0+rh)
                let box_c_min = (rx0 as isize).max(ry0 as isize - diff);
                let box_c_max = ((rx0 + rw - 1) as isize).min((ry0 + rh - 1) as isize - diff);
                if box_c_min > box_c_max {
                    return None;
                }
                let x_start = box_c_min as usize - c_min;
                let x_end = box_c_max as usize - c_min + 1;
                let line_len = c_max - c_min + 1;
                let mut line = Vec::with_capacity(line_len);
                for c in c_min..=c_max {
                    let r = (c as isize + diff) as usize;
                    line.push(total_h_m[r * width + c]);
                }
                let mut tan_se = vec![0.0_f32; x_end - x_start];
                solve_1d_horizon_bounded_stewart(
                    &line,
                    x_start,
                    x_end,
                    horizon_radius,
                    diag_step,
                    &mut tan_se,
                );
                line.reverse();
                let rev_start = line_len - x_end;
                let rev_end = line_len - x_start;
                let mut tan_nw = vec![0.0_f32; x_end - x_start];
                solve_1d_horizon_bounded_stewart(
                    &line,
                    rev_start,
                    rev_end,
                    horizon_radius,
                    diag_step,
                    &mut tan_nw,
                );
                tan_nw.reverse();
                Some((diff, box_c_min as usize, tan_se, tan_nw))
            })
            .collect();

    for (diff, box_c0, tan_se, tan_nw) in diag_se_nw {
        for (k, (&se, &nw)) in tan_se.iter().zip(tan_nw.iter()).enumerate() {
            let gc = box_c0 + k;
            let gr = (gc as isize + diff) as usize;
            let idx = (gr - ry0) * rw + (gc - rx0);
            max_tan[idx][4] = se; // SE (+1,+1)
            max_tan[idx][7] = nw; // NW (-1,-1)
        }
    }

    let diag_sw_ne: Vec<(usize, usize, Vec<f32>, Vec<f32>)> = ((rx0 + ry0)
        ..=(rx0 + rw - 1 + ry0 + rh - 1))
        .into_par_iter()
        .filter_map(|sum_rc| {
            // Line satisfies r + c == sum_rc, parameterized forward along SW (-1, +1) -> r increases, c decreases.
            let r_min = sum_rc.saturating_sub(width - 1);
            let r_max = sum_rc.min(height - 1);
            if r_min > r_max {
                return None;
            }
            let box_r_min = ry0.max(sum_rc.saturating_sub(rx0 + rw - 1));
            let box_r_max = (ry0 + rh - 1).min(sum_rc.saturating_sub(rx0));
            if box_r_min > box_r_max {
                return None;
            }
            let x_start = box_r_min - r_min;
            let x_end = box_r_max - r_min + 1;
            let line_len = r_max - r_min + 1;
            let mut line = Vec::with_capacity(line_len);
            for r in r_min..=r_max {
                let c = sum_rc - r;
                line.push(total_h_m[r * width + c]);
            }
            let mut tan_sw = vec![0.0_f32; x_end - x_start];
            solve_1d_horizon_bounded_stewart(
                &line,
                x_start,
                x_end,
                horizon_radius,
                diag_step,
                &mut tan_sw,
            );
            line.reverse();
            let rev_start = line_len - x_end;
            let rev_end = line_len - x_start;
            let mut tan_ne = vec![0.0_f32; x_end - x_start];
            solve_1d_horizon_bounded_stewart(
                &line,
                rev_start,
                rev_end,
                horizon_radius,
                diag_step,
                &mut tan_ne,
            );
            tan_ne.reverse();
            Some((sum_rc, box_r_min, tan_sw, tan_ne))
        })
        .collect();

    for (sum_rc, box_r0, tan_sw, tan_ne) in diag_sw_ne {
        for (k, (&sw, &ne)) in tan_sw.iter().zip(tan_ne.iter()).enumerate() {
            let gr = box_r0 + k;
            let gc = sum_rc - gr;
            let idx = (gr - ry0) * rw + (gc - rx0);
            max_tan[idx][5] = sw; // SW (-1,+1)
            max_tan[idx][6] = ne; // NE (+1,-1)
        }
    }

    // Pack final RGBA8 lighting + horizon angles
    let mut terrain_lighting = Vec::with_capacity(out_n);
    let mut terrain_horizon = Vec::with_capacity(out_n);
    for lr in 0..rh {
        let gr = ry0 + lr;
        for lc in 0..rw {
            let l_idx = lr * rw + lc;
            let g_idx = gr * width + (rx0 + lc);
            let (lt, hz) = pack_lighting_texel(&max_tan[l_idx], albedo_luma[g_idx]);
            terrain_lighting.push(lt);
            terrain_horizon.push(hz);
        }
    }

    let sky_bounce_probe = if rx0 == 0 && ry0 == 0 && rw == width && rh == height {
        if let Some(alb) = macro_albedo {
            compute_sky_bounce_probe_512(&terrain_lighting, alb, width, height)
        } else {
            let gray: Vec<[u8; 3]> = albedo_luma
                .iter()
                .map(|&l| {
                    let v = quantize_u8(l);
                    [v, v, v]
                })
                .collect();
            compute_sky_bounce_probe_512(&terrain_lighting, &gray, width, height)
        }
    } else {
        Vec::new()
    };

    Ok(LightingBakeOutput {
        width,
        height,
        tile_origin_x: rx0 / TILE_ACTIVE_SIZE,
        tile_origin_y: ry0 / TILE_ACTIVE_SIZE,
        out_width: rw,
        out_height: rh,
        terrain_lighting,
        terrain_horizon,
        sky_bounce_probe,
    })
}

pub(crate) fn write_u8x4_slice(path: &Path, data: &[[u8; 4]]) -> Result<String, BakeError> {
    let mut writer = BufWriter::new(File::create(path)?);
    let mut hasher = Sha256::new();
    let mut buf = Vec::with_capacity(IO_CHUNK_BYTES);
    for &c in data {
        buf.extend_from_slice(&c);
        if buf.len() >= IO_CHUNK_BYTES {
            hasher.update(&buf);
            writer.write_all(&buf)?;
            buf.clear();
        }
    }
    if !buf.is_empty() {
        hasher.update(&buf);
        writer.write_all(&buf)?;
    }
    writer.flush()?;
    Ok(hex_encode(&hasher.finalize()))
}

pub(crate) fn read_u8x4_bin(path: &Path, expected_len: usize) -> Result<Vec<[u8; 4]>, BakeError> {
    use std::io::Read;
    let mut file = File::open(path)?;
    let mut bytes = Vec::with_capacity(expected_len * 4);
    file.read_to_end(&mut bytes)?;
    if bytes.len() != expected_len * 4 {
        return Err(BakeError::new(format!(
            "binary u8x4 file {} length mismatch: expected {}, got {}",
            path.display(),
            expected_len * 4,
            bytes.len()
        )));
    }
    let (chunks, _) = bytes.as_chunks::<4>();
    Ok(chunks.to_vec())
}

pub(crate) type LoadedLightingRaw = (RawWorldMeta, Vec<f32>, Vec<f32>, Vec<[u8; 3]>);

/// Loads `raw_meta.json` + required raw layers and computes albedo luma + RGB arrays.
pub fn load_lighting_inputs_from_raw(raw_dir: &Path) -> Result<LoadedLightingRaw, BakeError> {
    let meta_text = fs::read_to_string(raw_dir.join("raw_meta.json"))?;
    let meta: RawWorldMeta = serde_json::from_str(&meta_text)?;
    if meta.width == 0 || meta.width != meta.height {
        return Err(BakeError::new(format!(
            "invalid raw_meta.json dimensions {}x{}",
            meta.width, meta.height
        )));
    }
    let n = (meta.width as usize) * (meta.height as usize);
    let total_h_m = read_bin_f32(&raw_dir.join("total_h_m.bin"), n)?;
    let dominant_biome = read_bin_u8(&raw_dir.join("dominant_biome.bin"), n)?;
    let moisture = read_bin_f32(&raw_dir.join("moisture.bin"), n)?;
    let talus_frac = read_bin_f32(&raw_dir.join("talus_frac.bin"), n)?;
    let glacier_h_m = read_bin_f32(&raw_dir.join("glacier_h_m.bin"), n)?;

    let mut macro_albedo = Vec::with_capacity(n);
    for i in 0..n {
        macro_albedo.push(compute_macro_albedo_rgb(
            dominant_biome[i],
            total_h_m[i],
            moisture[i],
            talus_frac[i],
            glacier_h_m[i],
        ));
    }
    let albedo_luma = build_albedo_luma_grid(
        &dominant_biome,
        &total_h_m,
        &moisture,
        &talus_frac,
        &glacier_h_m,
    );
    Ok((meta, total_h_m, albedo_luma, macro_albedo))
}

/// Writes full-grid or per-tile CPU lighting outputs under `out_dir`.
pub fn bake_lighting_cpu_from_raw_dir(
    raw_dir: &Path,
    out_dir: &Path,
    tile_range: Option<TileRange>,
) -> Result<LightingBakeReport, BakeError> {
    let (meta, total_h_m, albedo_luma, macro_albedo) = load_lighting_inputs_from_raw(raw_dir)?;
    let w = meta.width as usize;
    let h = meta.height as usize;
    fs::create_dir_all(out_dir)?;

    if let Some(range) = tile_range {
        if !w.is_multiple_of(TILE_ACTIVE_SIZE) || !h.is_multiple_of(TILE_ACTIVE_SIZE) {
            return Err(BakeError::new(format!(
                "tile-range lighting requires grid divisible by {TILE_ACTIVE_SIZE}"
            )));
        }
        let tx_cnt = w / TILE_ACTIVE_SIZE;
        let ty_cnt = h / TILE_ACTIVE_SIZE;
        let total_tiles = tx_cnt * ty_cnt;
        if range.end > total_tiles {
            return Err(BakeError::new(format!(
                "--tile-range end {} exceeds total tile count {total_tiles}",
                range.end
            )));
        }
        let tiles_dir = out_dir.join("tiles");
        fs::create_dir_all(&tiles_dir)?;
        let mut last_l_sha = String::new();
        let mut last_h_sha = String::new();
        for tid in range.start..range.end {
            let tx = tid % tx_cnt;
            let ty = tid / tx_cnt;
            let rx0 = tx * TILE_ACTIVE_SIZE;
            let ry0 = ty * TILE_ACTIVE_SIZE;
            let baked = bake_lighting_cpu_region(
                &total_h_m,
                &albedo_luma,
                Some(&macro_albedo),
                w,
                h,
                meta.dx_m,
                HORIZON_HALO_CELLS,
                Some((rx0, ry0, TILE_ACTIVE_SIZE, TILE_ACTIVE_SIZE)),
            )?;
            last_l_sha = write_u8x4_slice(
                &tiles_dir.join(format!("tile_{tid}_lighting.bin")),
                &baked.terrain_lighting,
            )?;
            last_h_sha = write_u8x4_slice(
                &tiles_dir.join(format!("tile_{tid}_horizon.bin")),
                &baked.terrain_horizon,
            )?;
        }
        let report = LightingBakeReport {
            backend: "cpu".to_owned(),
            width: meta.width,
            height: meta.height,
            dx_m: meta.dx_m,
            horizon_radius_cells: HORIZON_HALO_CELLS,
            tile_range_start: Some(range.start),
            tile_range_end: Some(range.end),
            lighting_sha256: last_l_sha,
            horizon_sha256: last_h_sha,
            probe_sha256: None,
        };
        fs::write(
            out_dir.join(format!("lighting_meta_{}_{}.json", range.start, range.end)),
            format!("{}\n", serde_json::to_string_pretty(&report)?),
        )?;
        Ok(report)
    } else {
        let baked = bake_lighting_cpu_region(
            &total_h_m,
            &albedo_luma,
            Some(&macro_albedo),
            w,
            h,
            meta.dx_m,
            HORIZON_HALO_CELLS,
            None,
        )?;
        let lighting_sha256 = write_u8x4_slice(
            &out_dir.join("terrain_lighting.bin"),
            &baked.terrain_lighting,
        )?;
        let horizon_sha256 =
            write_u8x4_slice(&out_dir.join("terrain_horizon.bin"), &baked.terrain_horizon)?;
        let probe_sha = write_u8x4_slice(
            &out_dir.join("sky_bounce_probe.bin"),
            &baked.sky_bounce_probe,
        )?;
        let report = LightingBakeReport {
            backend: "cpu".to_owned(),
            width: meta.width,
            height: meta.height,
            dx_m: meta.dx_m,
            horizon_radius_cells: HORIZON_HALO_CELLS,
            tile_range_start: None,
            tile_range_end: None,
            lighting_sha256,
            horizon_sha256,
            probe_sha256: Some(probe_sha),
        };
        fs::write(
            out_dir.join("lighting_meta.json"),
            format!("{}\n", serde_json::to_string_pretty(&report)?),
        )?;
        Ok(report)
    }
}

/// Reassembles per-tile `tiles/tile_{id}_{lighting,horizon}.bin` files into full `terrain_lighting.bin` & `terrain_horizon.bin`.
pub fn merge_lighting_tiles(out_dir: &Path, width: usize, height: usize) -> Result<(), BakeError> {
    if !width.is_multiple_of(TILE_ACTIVE_SIZE) || !height.is_multiple_of(TILE_ACTIVE_SIZE) {
        return Err(BakeError::new("grid dimensions must be multiples of 256"));
    }
    let tx_cnt = width / TILE_ACTIVE_SIZE;
    let ty_cnt = height / TILE_ACTIVE_SIZE;
    let total_tiles = tx_cnt * ty_cnt;
    let tiles_dir = out_dir.join("tiles");
    let mut full_lighting = vec![[0_u8; 4]; width * height];
    let mut full_horizon = vec![[0_u8; 4]; width * height];

    for tid in 0..total_tiles {
        let tx = tid % tx_cnt;
        let ty = tid / tx_cnt;
        let tile_l = read_u8x4_bin(
            &tiles_dir.join(format!("tile_{tid}_lighting.bin")),
            TILE_ACTIVE_SIZE * TILE_ACTIVE_SIZE,
        )?;
        let tile_h = read_u8x4_bin(
            &tiles_dir.join(format!("tile_{tid}_horizon.bin")),
            TILE_ACTIVE_SIZE * TILE_ACTIVE_SIZE,
        )?;
        for lr in 0..TILE_ACTIVE_SIZE {
            let gr = ty * TILE_ACTIVE_SIZE + lr;
            let dst_off = gr * width + tx * TILE_ACTIVE_SIZE;
            let src_off = lr * TILE_ACTIVE_SIZE;
            full_lighting[dst_off..(dst_off + TILE_ACTIVE_SIZE)]
                .copy_from_slice(&tile_l[src_off..(src_off + TILE_ACTIVE_SIZE)]);
            full_horizon[dst_off..(dst_off + TILE_ACTIVE_SIZE)]
                .copy_from_slice(&tile_h[src_off..(src_off + TILE_ACTIVE_SIZE)]);
        }
    }

    write_u8x4_slice(&out_dir.join("terrain_lighting.bin"), &full_lighting)?;
    write_u8x4_slice(&out_dir.join("terrain_horizon.bin"), &full_horizon)?;
    Ok(())
}
