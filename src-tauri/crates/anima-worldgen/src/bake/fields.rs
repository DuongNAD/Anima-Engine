#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

//! `JOB-W3` Offline Baked Fields:
//! - `normal_xy`: signed central-difference terrain normals (`[u8; 2]`)
//! - `shore_sdf`: exact Meijster (2000) 2-pass $O(N)$ signed Euclidean Distance Transform (`[-500, +500] m`) + wave exposure (`[f32]`/`u8`)
//! - `hydro_flow`: flow direction unit vector `RG`, `log2(1 + Q)` `B`, active `river_amt` + waterfall wetness `A`
//! - `flora_tiles`: deterministic decorative `256²` sector placements (`flora_place.rs`)
//! - Supports `--tile-strip S/K` row-strip sharding byte-identical to global pass.

use crate::bake::flora_place::{
    generate_decorative_flora_sectors, write_flora_sectors, FloraPlacementSummary,
    FLORA_SECTOR_SIZE,
};
use crate::bake::{quantize_u8, BakeError, TileStrip, SHORE_SDF_CLAMP_M};
use crate::pipeline::{read_bin_f32, read_bin_u8, RawWorldMeta};
use crate::WaterKind;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

const IO_CHUNK_BYTES: usize = 65_536;

/// Summary report emitted alongside `normal_xy.bin`, `shore_sdf.bin`, `hydro_flow.bin`, and `flora/`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldsBakeReport {
    pub width: u32,
    pub height: u32,
    pub dx_m: f32,
    pub row_start: usize,
    pub row_end: usize,
    pub strip_index: Option<usize>,
    pub strip_count: Option<usize>,
    pub shore_water_cells: usize,
    pub normal_xy_sha256: String,
    pub shore_sdf_sha256: Option<String>,
    pub hydro_flow_sha256: String,
    pub flora: Option<FloraPlacementSummary>,
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
fn f(x: i64, i: i64, gi: i64) -> i64 {
    let dx = x - i;
    dx * dx + gi * gi
}

#[inline]
fn sep(i: i64, u: i64, gi: i64, gu: i64) -> i64 {
    let num = u * u - i * i + gu * gu - gi * gi;
    let den = 2 * (u - i);
    num.div_euclid(den)
}

/// Exact Meijster (2000) 2-pass $O(N)$ squared Euclidean Distance Transform to `mask[idx] == true`.
///
/// Returns `u32` squared cell distances (`0` where `mask[idx] == true`, or `u32::MAX` everywhere if `mask` has no `true` cells).
#[must_use]
pub fn meijster_edt_sq(mask: &[bool], width: usize, height: usize) -> Vec<u32> {
    assert_eq!(mask.len(), width * height);
    if width == 0 || height == 0 {
        return Vec::new();
    }
    if !mask.iter().any(|&v| v) {
        return vec![u32::MAX; width * height];
    }

    let w_i = width as i64;
    let h_i = height as i64;
    let inf = w_i + h_i + 1;
    let mut g = vec![0_i64; width * height];

    // Phase 1: per-column Manhattan 1-D DT along rows.
    for c in 0..width {
        g[c] = if mask[c] { 0 } else { inf };
        for r in 1..height {
            let idx = r * width + c;
            g[idx] = if mask[idx] {
                0
            } else {
                g[(r - 1) * width + c] + 1
            };
        }
        if height >= 2 {
            for r in (0..(height - 1)).rev() {
                let idx = r * width + c;
                let below = g[(r + 1) * width + c] + 1;
                if below < g[idx] {
                    g[idx] = below;
                }
            }
        }
    }

    // Phase 2: per-row exact parabolic lower envelope scan.
    let mut dt = vec![0_u32; width * height];
    let mut s = vec![0_i64; width];
    let mut t = vec![0_i64; width];

    for r in 0..height {
        let row_off = r * width;
        let mut q_len = 1_usize;
        s[0] = 0;
        t[0] = 0;

        for u in 1..width {
            let u_i = u as i64;
            let g_u = g[row_off + u];
            while q_len > 0 {
                let top_s = s[q_len - 1];
                let top_t = t[q_len - 1];
                let g_top = g[row_off + top_s as usize];
                if f(top_t, top_s, g_top) > f(top_t, u_i, g_u) {
                    q_len -= 1;
                } else {
                    break;
                }
            }
            if q_len == 0 {
                s[0] = u_i;
                t[0] = 0;
                q_len = 1;
            } else {
                let top_s = s[q_len - 1];
                let g_top = g[row_off + top_s as usize];
                let w_sep = 1 + sep(top_s, u_i, g_top, g_u);
                if w_sep < w_i {
                    s[q_len] = u_i;
                    t[q_len] = w_sep.max(0);
                    q_len += 1;
                }
            }
        }

        for u in (0..width).rev() {
            let u_i = u as i64;
            let top_s = s[q_len - 1];
            let d2 = f(u_i, top_s, g[row_off + top_s as usize]);
            dt[row_off + u] = u32::try_from(d2).unwrap_or(u32::MAX);
            if q_len > 1 && u_i == t[q_len - 1] {
                q_len -= 1;
            }
        }
    }

    dt
}

#[inline]
fn is_shore_water(wk: u8, h_m: f32, sea_level_m: f32) -> bool {
    wk == WaterKind::Ocean as u8
        || (wk == WaterKind::DeltaTurbid as u8 && h_m <= sea_level_m + 0.1)
        || h_m <= sea_level_m
}

/// Computes exact `shore_sdf` (`[-500, +500] m`, negative underwater, positive on land)
/// and normalized coastal wave exposure `[0.0, 1.0]` using Meijster 2-pass exact EDT (`Pass A`).
#[must_use]
pub fn compute_shore_sdf_and_exposure(
    total_h_m: &[f32],
    water_kind: &[u8],
    width: usize,
    height: usize,
    dx_m: f32,
    sea_level_m: f32,
    wind_azimuth_rad: f32,
) -> (Vec<f32>, Vec<f32>, usize) {
    let n = width * height;
    assert_eq!(total_h_m.len(), n);
    assert_eq!(water_kind.len(), n);

    let mut is_water = Vec::with_capacity(n);
    let mut is_land = Vec::with_capacity(n);
    let mut water_cnt = 0_usize;
    for i in 0..n {
        let w = is_shore_water(water_kind[i], total_h_m[i], sea_level_m);
        if w {
            water_cnt += 1;
        }
        is_water.push(w);
        is_land.push(!w);
    }

    let d2_to_water = meijster_edt_sq(&is_water, width, height);
    let d2_to_land = meijster_edt_sq(&is_land, width, height);

    let mut sdf = Vec::with_capacity(n);
    let mut exposure = vec![0.0_f32; n];
    let wind_x = libm::cosf(wind_azimuth_rad);
    let wind_z = libm::sinf(wind_azimuth_rad);

    for r in 0..height {
        let r_prev = r.saturating_sub(1);
        let r_next = (r + 1).min(height - 1);
        for c in 0..width {
            let idx = r * width + c;
            let val = if is_water[idx] {
                if d2_to_land[idx] == u32::MAX {
                    -SHORE_SDF_CLAMP_M
                } else {
                    -((d2_to_land[idx] as f32).sqrt() * dx_m).min(SHORE_SDF_CLAMP_M)
                }
            } else if d2_to_water[idx] == u32::MAX {
                SHORE_SDF_CLAMP_M
            } else {
                ((d2_to_water[idx] as f32).sqrt() * dx_m).min(SHORE_SDF_CLAMP_M)
            };
            sdf.push(val);

            // Coastal wave exposure peaks along wind-facing shores (`|sdf| <= 220 m`).
            let shore_prox = (1.0 - val.abs() / 220.0).clamp(0.0, 1.0);
            if shore_prox > 0.0 {
                let c_prev = c.saturating_sub(1);
                let c_next = (c + 1).min(width - 1);
                let gx = (total_h_m[r * width + c_next] - total_h_m[r * width + c_prev])
                    / ((c_next - c_prev).max(1) as f32 * dx_m);
                let gz = (total_h_m[r_next * width + c] - total_h_m[r_prev * width + c])
                    / ((r_next - r_prev).max(1) as f32 * dx_m);
                let gmag = libm::hypotf(gx, gz);
                let align = if gmag > 1e-5 {
                    ((-gx / gmag) * wind_x + (-gz / gmag) * wind_z).clamp(-1.0, 1.0)
                } else {
                    0.0
                };
                let exp_w = 0.55 + 0.45 * (0.5 + 0.5 * align);
                exposure[idx] = (shore_prox * exp_w).clamp(0.0, 1.0);
            }
        }
    }

    (sdf, exposure, water_cnt)
}

/// Packs `(shore_sdf, wave_exposure)` into `[u8; 2]` (`R` = signed distance `[-500,+500]`, `G` = exposure).
#[must_use]
pub fn pack_shore_sdf_u8x2(sdf_m: &[f32], exposure: &[f32]) -> Vec<[u8; 2]> {
    sdf_m
        .iter()
        .zip(exposure.iter())
        .map(|(&s, &e)| {
            let s_norm = ((s + SHORE_SDF_CLAMP_M) / (2.0 * SHORE_SDF_CLAMP_M)).clamp(0.0, 1.0);
            [quantize_u8(s_norm), quantize_u8(e)]
        })
        .collect()
}

/// Computes central-difference terrain `normal_xy` (`[u8; 2]`) for rows `row_start..row_end`.
#[must_use]
pub fn compute_normal_xy_rows(
    total_h_m: &[f32],
    width: usize,
    height: usize,
    dx_m: f32,
    row_start: usize,
    row_end: usize,
) -> Vec<[u8; 2]> {
    assert_eq!(total_h_m.len(), width * height);
    let mut out = Vec::with_capacity((row_end - row_start) * width);
    for r in row_start..row_end {
        let r0 = r.saturating_sub(1);
        let r1 = (r + 1).min(height - 1);
        let span_z = ((r1 - r0).max(1) as f32) * dx_m;
        for c in 0..width {
            let c0 = c.saturating_sub(1);
            let c1 = (c + 1).min(width - 1);
            let span_x = ((c1 - c0).max(1) as f32) * dx_m;
            let dh_dx = (total_h_m[r * width + c1] - total_h_m[r * width + c0]) / span_x;
            let dh_dz = (total_h_m[r1 * width + c] - total_h_m[r0 * width + c]) / span_z;
            let inv_len = 1.0 / (dh_dx * dh_dx + 1.0 + dh_dz * dh_dz).sqrt();
            let nx = -dh_dx * inv_len;
            let nz = -dh_dz * inv_len;
            out.push([quantize_u8(nx * 0.5 + 0.5), quantize_u8(nz * 0.5 + 0.5)]);
        }
    }
    out
}

/// Computes `hydro_flow` (`[u8; 4]`) for rows `row_start..row_end` using a 2-cell waterfall halo.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn compute_hydro_flow_rows(
    flow_dir_x: &[f32],
    flow_dir_z: &[f32],
    discharge_q_m3_s: &[f32],
    river_w_m: &[f32],
    water_kind: &[u8],
    width: usize,
    height: usize,
    dx_m: f32,
    row_start: usize,
    row_end: usize,
) -> Vec<[u8; 4]> {
    let n = width * height;
    assert_eq!(flow_dir_x.len(), n);
    assert_eq!(flow_dir_z.len(), n);
    assert_eq!(discharge_q_m3_s.len(), n);
    assert_eq!(river_w_m.len(), n);
    assert_eq!(water_kind.len(), n);

    let mut out = Vec::with_capacity((row_end - row_start) * width);
    for r in row_start..row_end {
        for c in 0..width {
            let idx = r * width + c;
            let fx = flow_dir_x[idx];
            let fz = flow_dir_z[idx];
            let mag = libm::hypotf(fx, fz);
            let (ux, uz) = if mag > 1e-6 {
                (fx / mag, fz / mag)
            } else {
                (0.0, 0.0)
            };

            let q = discharge_q_m3_s[idx].max(0.0);
            let log_q = (libm::log2f(1.0 + q) / 10.0).clamp(0.0, 1.0);
            let wk = water_kind[idx];
            let mut amt = if wk == WaterKind::River as u8
                || wk == WaterKind::DeltaTurbid as u8
                || river_w_m[idx] >= 0.5
            {
                (river_w_m[idx] / dx_m).clamp(0.0, 1.0)
            } else {
                0.0
            };

            if wk == WaterKind::Waterfall as u8 {
                amt = 1.0;
            } else {
                // Bounded 2-cell spray halo keeps strip sharding 100% byte-identical.
                let r_lo = r.saturating_sub(2);
                let r_hi = (r + 2).min(height - 1);
                let c_lo = c.saturating_sub(2);
                let c_hi = (c + 2).min(width - 1);
                let mut best_spray = 0.0_f32;
                for nr in r_lo..=r_hi {
                    for nc in c_lo..=c_hi {
                        if water_kind[nr * width + nc] == WaterKind::Waterfall as u8 {
                            let d2 = ((nr as isize - r as isize).pow(2)
                                + (nc as isize - c as isize).pow(2))
                                as f32;
                            let spray = (1.0 - d2 / 6.0).clamp(0.0, 0.85);
                            if spray > best_spray {
                                best_spray = spray;
                            }
                        }
                    }
                }
                amt = amt.max(best_spray);
            }

            out.push([
                quantize_u8(ux * 0.5 + 0.5),
                quantize_u8(uz * 0.5 + 0.5),
                quantize_u8(log_q),
                quantize_u8(amt),
            ]);
        }
    }
    out
}

fn write_u8x2_bin(path: &Path, data: &[[u8; 2]]) -> Result<String, BakeError> {
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

fn write_u8x4_bin(path: &Path, data: &[[u8; 4]]) -> Result<String, BakeError> {
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

/// Runs `JOB-W3` field generation (`normal_xy`, exact Meijster `shore_sdf`, `hydro_flow`, and `flora_tiles`)
/// from an `anima-worldgen generate` output directory.
pub fn bake_fields_from_raw_dir(
    raw_dir: &Path,
    out_dir: &Path,
    strip: Option<TileStrip>,
) -> Result<FieldsBakeReport, BakeError> {
    let meta_text = fs::read_to_string(raw_dir.join("raw_meta.json"))?;
    let meta: RawWorldMeta = serde_json::from_str(&meta_text)?;
    if meta.width == 0 || meta.width != meta.height {
        return Err(BakeError::new(format!(
            "invalid raw_meta.json dimensions {}x{}",
            meta.width, meta.height
        )));
    }
    let w = meta.width as usize;
    let h = meta.height as usize;
    let n = w * h;

    let total_h_m = read_bin_f32(&raw_dir.join("total_h_m.bin"), n)?;
    let water_kind = read_bin_u8(&raw_dir.join("water_kind.bin"), n)?;
    let flow_dir_x = read_bin_f32(&raw_dir.join("flow_dir_x.bin"), n)?;
    let flow_dir_z = read_bin_f32(&raw_dir.join("flow_dir_z.bin"), n)?;
    let discharge_q = read_bin_f32(&raw_dir.join("discharge_q_m3_s.bin"), n)?;
    let river_w_m = read_bin_f32(&raw_dir.join("river_w_m.bin"), n)?;

    fs::create_dir_all(out_dir)?;

    let (row_start, row_end) = match strip {
        Some(s) => s.row_bounds(h),
        None => (0, h),
    };

    // Pass A (global fast O(N) Meijster shore_sdf) runs in full mode or on strip 0 (and whenever absent).
    let (shore_water_cells, shore_sdf_sha256) =
        if strip.is_none() || strip.is_some_and(|s| s.index == 0) {
            let (sdf, exposure, wcnt) = compute_shore_sdf_and_exposure(
                &total_h_m,
                &water_kind,
                w,
                h,
                meta.dx_m,
                meta.sea_level_m,
                meta.wind_azimuth_rad,
            );
            let packed = pack_shore_sdf_u8x2(&sdf, &exposure);
            let digest = write_u8x2_bin(&out_dir.join("shore_sdf.bin"), &packed)?;
            (wcnt, Some(digest))
        } else {
            (
                water_kind
                    .iter()
                    .zip(total_h_m.iter())
                    .filter(|&(&wk, &elev)| is_shore_water(wk, elev, meta.sea_level_m))
                    .count(),
                None,
            )
        };

    // Pass B (strip-sharded byte-identical row ranges)
    let normal_xy = compute_normal_xy_rows(&total_h_m, w, h, meta.dx_m, row_start, row_end);
    let hydro_flow = compute_hydro_flow_rows(
        &flow_dir_x,
        &flow_dir_z,
        &discharge_q,
        &river_w_m,
        &water_kind,
        w,
        h,
        meta.dx_m,
        row_start,
        row_end,
    );

    let (normal_path, hydro_path) = match strip {
        Some(s) => {
            let sdir = out_dir.join("strips");
            fs::create_dir_all(&sdir)?;
            (
                sdir.join(format!("strip_{}_of_{}_normal_xy.bin", s.index, s.count)),
                sdir.join(format!("strip_{}_of_{}_hydro_flow.bin", s.index, s.count)),
            )
        }
        None => (
            out_dir.join("normal_xy.bin"),
            out_dir.join("hydro_flow.bin"),
        ),
    };

    let normal_xy_sha256 = write_u8x2_bin(&normal_path, &normal_xy)?;
    let hydro_flow_sha256 = write_u8x4_bin(&hydro_path, &hydro_flow)?;

    // Decorative flora placement runs deterministically per 256^2 sector in full mode (or strip 0).
    let flora = if strip.is_none() || strip.is_some_and(|s| s.index == 0) {
        if w.is_multiple_of(FLORA_SECTOR_SIZE) && h.is_multiple_of(FLORA_SECTOR_SIZE) {
            let dominant_biome = read_bin_u8(&raw_dir.join("dominant_biome.bin"), n)?;
            let moisture = read_bin_f32(&raw_dir.join("moisture.bin"), n)?;
            let twi = read_bin_f32(&raw_dir.join("twi.bin"), n)?;
            let glacier_h_m = read_bin_f32(&raw_dir.join("glacier_h_m.bin"), n)?;
            let (blobs, summary) = generate_decorative_flora_sectors(
                meta.seed,
                &meta.schema_tag,
                w,
                h,
                meta.dx_m,
                &total_h_m,
                &dominant_biome,
                &water_kind,
                &moisture,
                &twi,
                &river_w_m,
                &glacier_h_m,
                None,
            )?;
            write_flora_sectors(&out_dir.join("flora"), &blobs, &summary)?;
            Some(summary)
        } else {
            None
        }
    } else {
        None
    };

    let report = FieldsBakeReport {
        width: meta.width,
        height: meta.height,
        dx_m: meta.dx_m,
        row_start,
        row_end,
        strip_index: strip.map(|s| s.index),
        strip_count: strip.map(|s| s.count),
        shore_water_cells,
        normal_xy_sha256,
        shore_sdf_sha256,
        hydro_flow_sha256,
        flora,
    };

    let meta_name = match strip {
        Some(s) => format!("fields_meta_strip_{}_of_{}.json", s.index, s.count),
        None => "fields_meta.json".to_owned(),
    };
    fs::write(
        out_dir.join(meta_name),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    Ok(report)
}

/// Concatenates sharded `strips/strip_{S}_of_{K}_{normal_xy,hydro_flow}.bin` files into global `.bin` files.
pub fn merge_field_strips(out_dir: &Path, strip_count: usize) -> Result<(), BakeError> {
    if strip_count == 0 {
        return Err(BakeError::new("strip_count must be > 0"));
    }
    let sdir = out_dir.join("strips");
    for stem in ["normal_xy", "hydro_flow"] {
        let out_path = out_dir.join(format!("{stem}.bin"));
        let mut writer = BufWriter::new(File::create(&out_path)?);
        for s in 0..strip_count {
            let p = sdir.join(format!("strip_{s}_of_{strip_count}_{stem}.bin"));
            let bytes = fs::read(&p)?;
            writer.write_all(&bytes)?;
        }
        writer.flush()?;
    }
    Ok(())
}
