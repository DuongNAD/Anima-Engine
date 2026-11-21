//! Stage S6: Barnes et al. (2020) depression hierarchy, Barnes et al. (2021) Fill-Spill-Merge
//! lake-level & spillway routing (humid overflowing lakes + arid rain-shadow `EndorheicLake`
//! equilibrium), micro-pit cleanup, and knickpoint waterfalls + plunge pools (`Howard, Dietrich &
//! Seidl 1994`).

use crate::hydrology::{HydrologyField, NO_RECEIVER, SECONDS_PER_YEAR};
use crate::WaterKind;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::fmt;

const DX8: [isize; 8] = [1, 1, 0, -1, -1, -1, 0, 1];
const DZ8: [isize; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
const DIST8: [f32; 8] = [
    1.0,
    std::f32::consts::SQRT_2,
    1.0,
    std::f32::consts::SQRT_2,
    1.0,
    std::f32::consts::SQRT_2,
    1.0,
    std::f32::consts::SQRT_2,
];

/// Error returned when Stage S6 inputs or configuration are invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LakesFallsError {
    pub message: String,
}

impl fmt::Display for LakesFallsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LakesFallsError {}

/// Configuration for Stage S6 Fill-Spill-Merge lakes and knickpoint waterfalls (`§2.2.6`).
#[derive(Debug, Clone, PartialEq)]
pub struct LakesFallsConfig {
    pub dx_m: f32,
    pub sea_level_m: f32,
    /// When `false` (negative control), humid lake spillways are NOT routed downstream.
    pub enable_fill_spill_merge: bool,
    /// Minimum water depth (`m`) required to retain a depression as a lake.
    pub min_lake_depth_m: f32,
    /// Minimum cell count required for a lowland/mid-elevation lake.
    pub min_lake_cells: usize,
    /// Minimum cell count required for a high-páramo cirque pond (`>= paramo_elev_m`).
    pub min_pond_cells: usize,
    /// Elevation threshold (`3,300 m`) above which alpine/páramo ponds & glacial lakes occur.
    pub paramo_elev_m: f32,
    /// Minimum vertical drop (`12.0 m`) over `waterfall_max_run_m` for knickpoint waterfalls.
    pub waterfall_min_drop_m: f32,
    /// Maximum horizontal path distance (`30.0 m`) over which `waterfall_min_drop_m` occurs.
    pub waterfall_max_run_m: f32,
    /// Maximum upstream channel slope (`0.08`) above a waterfall lip.
    pub waterfall_max_up_slope: f32,
    /// Minimum active discharge (`m^3/s`) to form a mapped waterfall.
    pub waterfall_min_q_m3_s: f32,
    /// Minimum Euclidean spacing (`m`) between waterfall records (`20..=80` target count).
    pub waterfall_min_spacing_m: f32,
    /// Maximum number of recorded waterfalls (`80`).
    pub waterfall_max_count: usize,
}

impl Default for LakesFallsConfig {
    fn default() -> Self {
        Self {
            dx_m: 10.0,
            sea_level_m: 0.0,
            enable_fill_spill_merge: true,
            min_lake_depth_m: 0.35,
            min_lake_cells: 4,
            min_pond_cells: 2,
            paramo_elev_m: 3_300.0,
            waterfall_min_drop_m: 12.0,
            waterfall_max_run_m: 30.0,
            waterfall_max_up_slope: 0.08,
            waterfall_min_q_m3_s: 0.25,
            waterfall_min_spacing_m: 300.0,
            waterfall_max_count: 80,
        }
    }
}

/// Borrowed input slices for Stage S6 (avoids retaining S5's `f64` arrays at `4096^2`).
pub struct LakesFallsInput<'a> {
    pub width: usize,
    pub height: usize,
    pub precip_m_yr: &'a [f32],
    pub pet_m_yr: &'a [f32],
    pub discharge_q_m3_s: &'a [f32],
    pub river_w_m: &'a [f32],
    pub is_channel: &'a [bool],
}

impl<'a> LakesFallsInput<'a> {
    /// Builds a borrowed Stage S6 input view directly from a [`HydrologyField`] and climate slices.
    #[must_use]
    pub fn from_hydrology(
        hydro: &'a HydrologyField,
        precip_m_yr: &'a [f32],
        pet_m_yr: &'a [f32],
    ) -> Self {
        Self {
            width: hydro.width,
            height: hydro.height,
            precip_m_yr,
            pet_m_yr,
            discharge_q_m3_s: &hydro.discharge_q_m3_s,
            river_w_m: &hydro.river_w_m,
            is_channel: &hydro.is_channel,
        }
    }
}

/// Geomorphic classification of a retained lake basin (`§5.1` `C3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LakeClassification {
    FreshwaterSpilling,
    EndorheicClosed,
    HighParamoGlacial,
    CalderaOrTrough,
    ParamoCirquePond,
}

/// Summary metadata for a retained lake basin.
#[derive(Debug, Clone, PartialEq)]
pub struct LakeBasinRecord {
    pub id: u32,
    pub water_kind: u8,
    pub classification: LakeClassification,
    pub water_level_m: f32,
    pub spill_elev_m: f32,
    pub pit_elev_m: f32,
    pub area_m2: f64,
    pub max_depth_m: f32,
    pub mean_elev_m: f32,
    pub max_wall_slope_deg: f32,
    pub elongation: f32,
    pub outlet_cell: u32,
    pub q_in_m3_s: f32,
    pub q_out_m3_s: f32,
    pub spillway_reaches_outlet: bool,
}

/// Mapped knickpoint waterfall and plunge-pool metadata (`§2.2.6`, `§5.1` `C1`).
#[derive(Debug, Clone, PartialEq)]
pub struct WaterfallRecord {
    pub lip_cell: u32,
    pub base_cell: u32,
    pub x_m: f32,
    pub z_m: f32,
    pub lip_elev_m: f32,
    pub drop_m: f32,
    pub yaw_rad: f32,
    pub discharge_q_m3_s: f32,
    pub plunge_depth_m: f32,
}

/// Output buffers and diagnostics produced by [`generate_lakes_and_waterfalls`].
///
/// Note: `water_h_m` and `water_kind` are mutated in place on the caller-supplied `&mut` slices
/// rather than duplicated here (saving `80 MiB` at `4096^2`).
#[derive(Debug, Clone)]
pub struct LakesFallsField {
    pub width: usize,
    pub height: usize,
    /// Per-cell basin ID (`0` = not a lake; `1..=K` maps to `basins[id - 1]`).
    pub lake_id: Vec<u32>,
    pub basins: Vec<LakeBasinRecord>,
    pub waterfalls: Vec<WaterfallRecord>,
    pub severed_humid_spillways: usize,
    pub unmodeled_micro_pits: usize,
    pub endorheic_land_fraction: f32,
    pub endorheic_humid_fraction: f32,
    pub lake_surface_fraction: f32,
}

#[inline]
fn float_ord_key(x: f32) -> u32 {
    let bits = x.to_bits();
    if (bits >> 31) != 0 {
        !bits
    } else {
        bits | 0x8000_0000
    }
}

#[inline]
fn bit_get(bits: &[u64], idx: usize) -> bool {
    (bits[idx >> 6] & (1_u64 << (idx & 63))) != 0
}

#[inline]
fn bit_set(bits: &mut [u64], idx: usize) {
    bits[idx >> 6] |= 1_u64 << (idx & 63);
}

#[derive(Debug, Clone)]
struct DepNode {
    pit_elev_m: f32,
    spill_elev_m: f32,
    outlet_cell: u32,
    ext_spill_h: f32,
    ext_outlet_cell: u32,
    children: Option<(u32, u32)>,
    parent_node: u32,
    own_cells: Vec<u32>,
}

fn find_root(dsu: &mut [u32], mut x: u32) -> u32 {
    while dsu[x as usize] != x {
        let p = dsu[x as usize];
        dsu[x as usize] = dsu[p as usize];
        x = p;
    }
    x
}

fn collect_subtree_cells(nodes: &[DepNode], root: u32, out: &mut Vec<u32>) {
    let mut stack = vec![root];
    while let Some(u) = stack.pop() {
        let node = &nodes[u as usize];
        out.extend_from_slice(&node.own_cells);
        if let Some((a, b)) = node.children {
            stack.push(a);
            stack.push(b);
        }
    }
}

fn suppress_subtree_descendants(nodes: &[DepNode], root: u32, suppressed: &mut [bool]) {
    let mut stack = vec![root];
    while let Some(v) = stack.pop() {
        if let Some((a, b)) = nodes[v as usize].children {
            suppressed[a as usize] = true;
            suppressed[b as usize] = true;
            stack.push(a);
            stack.push(b);
        }
    }
}

fn has_any_humid_submerged(
    cells: &[u32],
    elev_m: &[f32],
    level_m: f32,
    precip_m_yr: &[f32],
    pet_m_yr: &[f32],
) -> bool {
    for &c in cells {
        let cu = c as usize;
        if elev_m[cu] <= level_m {
            let pet_c = pet_m_yr[cu].max(0.05);
            if (precip_m_yr[cu] / pet_c) >= 0.5 {
                return true;
            }
        }
    }
    false
}

fn pick_receiving_child(
    nodes: &[DepNode],
    a: u32,
    b: u32,
    entry_outlet_cell: u32,
    w: usize,
    scratch: &mut Vec<u32>,
) -> (u32, u32) {
    if entry_outlet_cell == NO_RECEIVER {
        return if nodes[a as usize].pit_elev_m <= nodes[b as usize].pit_elev_m {
            (a, b)
        } else {
            (b, a)
        };
    }
    let er = (entry_outlet_cell as usize / w) as isize;
    let ec = (entry_outlet_cell as usize % w) as isize;
    let min_dist_sq = |root: u32, buf: &mut Vec<u32>| -> isize {
        buf.clear();
        collect_subtree_cells(nodes, root, buf);
        let mut best = isize::MAX;
        for &c in buf.iter() {
            let dr = (c as usize / w) as isize - er;
            let dc = (c as usize % w) as isize - ec;
            let d2 = dr * dr + dc * dc;
            if d2 < best {
                best = d2;
            }
        }
        best
    };
    let da = min_dist_sq(a, scratch);
    let db = min_dist_sq(b, scratch);
    if da <= db {
        (a, b)
    } else {
        (b, a)
    }
}

fn solve_equilibrium_level(
    elevs: &[f32],
    pit_elev_m: f32,
    spill_elev_m: f32,
    q_in_m3_yr: f64,
    mean_pet_m_yr: f64,
    cell_area_m2: f64,
) -> f32 {
    if q_in_m3_yr <= 0.0 || mean_pet_m_yr <= 1e-9 || elevs.is_empty() {
        return pit_elev_m;
    }
    let mut lo = pit_elev_m;
    let mut hi = spill_elev_m;
    for _ in 0..32 {
        let mid = 0.5 * (lo + hi);
        let cnt = elevs.iter().filter(|&&h| h <= mid).count();
        let evap = (cnt as f64) * cell_area_m2 * mean_pet_m_yr;
        if evap < q_in_m3_yr {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

fn compute_wall_slope_and_elongation(
    cells: &[u32],
    elev_m: &[f32],
    water_level_m: f32,
    width: usize,
    height: usize,
    dx_m: f32,
) -> (f32, f32) {
    let mut max_slope_tan = 0.0_f32;
    let mut sum_r = 0.0_f64;
    let mut sum_c = 0.0_f64;
    let count = cells.len().max(1) as f64;
    for &idx_u in cells {
        let idx = idx_u as usize;
        let r = idx / width;
        let c = idx % width;
        sum_r += r as f64;
        sum_c += c as f64;
        for dir in 0..8 {
            let nr = r as isize + DZ8[dir];
            let nc = c as isize + DX8[dir];
            if nr < 0 || nc < 0 || nr >= height as isize || nc >= width as isize {
                continue;
            }
            let nb = nr as usize * width + nc as usize;
            let dh = elev_m[nb] - water_level_m;
            if dh > 0.0 {
                let s = dh / (dx_m * DIST8[dir]);
                if s > max_slope_tan {
                    max_slope_tan = s;
                }
            }
        }
    }
    let max_wall_slope_deg = libm::atanf(max_slope_tan) * (180.0 / std::f32::consts::PI);

    // 2x2 covariance eigenvalues for basin elongation.
    let mean_r = sum_r / count;
    let mean_c = sum_c / count;
    let mut s_rr = 0.0_f64;
    let mut s_cc = 0.0_f64;
    let mut s_rc = 0.0_f64;
    for &idx_u in cells {
        let idx = idx_u as usize;
        let dr = (idx / width) as f64 - mean_r;
        let dc = (idx % width) as f64 - mean_c;
        s_rr += dr * dr;
        s_cc += dc * dc;
        s_rc += dr * dc;
    }
    s_rr /= count;
    s_cc /= count;
    s_rc /= count;
    let tr = s_rr + s_cc;
    let diff = s_rr - s_cc;
    let disc = (diff * diff + 4.0 * s_rc * s_rc).sqrt();
    let lam1 = 0.5 * (tr + disc);
    let lam2 = (0.5 * (tr - disc)).max(1e-6);
    let elongation = ((lam1 / lam2).sqrt() as f32).max(1.0);
    (max_wall_slope_deg, elongation)
}

/// Runs Barnes et al. (2020/2021) Fill-Spill-Merge lakes, micro-pit cleanup, and knickpoint
/// waterfall carving on `elev_m`, mutating `elev_m`, `sediment_s`, `water_h_m`, and `water_kind`.
pub fn generate_lakes_and_waterfalls(
    elev_m: &mut [f32],
    sediment_s: &mut [f32],
    water_h_m: &mut [f32],
    water_kind: &mut [u8],
    input: &LakesFallsInput<'_>,
    cfg: &LakesFallsConfig,
) -> Result<LakesFallsField, LakesFallsError> {
    let w = input.width;
    let h = input.height;
    if w < 8 || h < 8 {
        return Err(LakesFallsError {
            message: "grid dimensions must be >= 8x8".to_owned(),
        });
    }
    let n = w * h;
    if elev_m.len() != n
        || sediment_s.len() != n
        || water_h_m.len() != n
        || water_kind.len() != n
        || input.precip_m_yr.len() != n
        || input.pet_m_yr.len() != n
        || input.discharge_q_m3_s.len() != n
        || input.river_w_m.len() != n
        || input.is_channel.len() != n
    {
        return Err(LakesFallsError {
            message: "input slice lengths do not match grid dimensions".to_owned(),
        });
    }
    if !cfg.dx_m.is_finite() || cfg.dx_m <= 0.0 {
        return Err(LakesFallsError {
            message: "dx_m must be finite and > 0".to_owned(),
        });
    }

    // 1. Memory-lean Priority-Flood with bit-packed `closed` (2 MiB @ 4096^2).
    let mut filled_h = elev_m.to_vec();
    let word_count = (n + 63) >> 6;
    let mut closed = vec![0_u64; word_count];
    let mut is_outlet = vec![0_u64; word_count];
    let mut open: BinaryHeap<Reverse<(u32, u32)>> = BinaryHeap::with_capacity(w * 2 + h * 2);
    let mut pit_queue: VecDeque<(u32, f32)> = VecDeque::new();

    for r in 0..h {
        for c in 0..w {
            let idx = r * w + c;
            let border = r == 0 || r + 1 == h || c == 0 || c + 1 == w;
            let marine = filled_h[idx] <= cfg.sea_level_m;
            if border || marine {
                bit_set(&mut closed, idx);
                bit_set(&mut is_outlet, idx);
                open.push(Reverse((float_ord_key(filled_h[idx]), idx as u32)));
            }
            if marine {
                water_kind[idx] = WaterKind::Ocean as u8;
                water_h_m[idx] = cfg.sea_level_m;
            }
        }
    }

    while !open.is_empty() || !pit_queue.is_empty() {
        let (cell, current_h) = if let Some((c, hc)) = pit_queue.pop_front() {
            (c as usize, hc)
        } else {
            let Reverse((_, c)) = open.pop().expect("open heap non-empty");
            let cu = c as usize;
            (cu, filled_h[cu])
        };
        let r = cell / w;
        let c = cell % w;
        for dir in 0..8 {
            let nr = r as isize + DZ8[dir];
            let nc = c as isize + DX8[dir];
            if nr < 0 || nc < 0 || nr >= h as isize || nc >= w as isize {
                continue;
            }
            let nb = nr as usize * w + nc as usize;
            if bit_get(&closed, nb) {
                continue;
            }
            bit_set(&mut closed, nb);
            if filled_h[nb] <= current_h {
                let raised = current_h.next_up();
                filled_h[nb] = raised;
                pit_queue.push_back((nb as u32, raised));
            } else {
                open.push(Reverse((float_ord_key(filled_h[nb]), nb as u32)));
            }
        }
    }
    drop(closed);

    // Steepest-descent D8 receiver on `filled_h`.
    let mut d8_rec = vec![NO_RECEIVER; n];
    for r in 0..h {
        for c in 0..w {
            let idx = r * w + c;
            if bit_get(&is_outlet, idx) {
                continue;
            }
            let hi = f64::from(filled_h[idx]);
            let mut best_slope = 0.0_f64;
            let mut best_nb = NO_RECEIVER;
            for dir in 0..8 {
                let nr = r as isize + DZ8[dir];
                let nc = c as isize + DX8[dir];
                if nr < 0 || nc < 0 || nr >= h as isize || nc >= w as isize {
                    continue;
                }
                let nb = nr as usize * w + nc as usize;
                let dh = hi - f64::from(filled_h[nb]);
                if dh > 0.0 {
                    let s = dh / f64::from(DIST8[dir]);
                    if s > best_slope {
                        best_slope = s;
                        best_nb = nb as u32;
                    }
                }
            }
            d8_rec[idx] = best_nb;
        }
    }
    drop(is_outlet);

    // 2. Collect ONLY depression cells (`filled_h > elev_m`, `elev_m > sea_level_m`) and build
    //    Barnes et al. (2020) depression hierarchy.
    let mut dep_cells: Vec<(u32, u32)> = Vec::new();
    for i in 0..n {
        if filled_h[i] > elev_m[i] && elev_m[i] > cfg.sea_level_m {
            dep_cells.push((float_ord_key(elev_m[i]), i as u32));
        }
    }
    dep_cells.sort_unstable();

    let mut cell_leaf = vec![NO_RECEIVER; n];
    let mut nodes: Vec<DepNode> = Vec::new();
    let mut dsu: Vec<u32> = Vec::new();

    for &(_, c_u32) in &dep_cells {
        let c_idx = c_u32 as usize;
        let h_c = elev_m[c_idx];
        let r = c_idx / w;
        let col = c_idx % w;

        let mut neigh_roots: [u32; 8] = [NO_RECEIVER; 8];
        let mut nr_len = 0usize;
        let mut best_ext_h = f32::INFINITY;
        let mut best_ext_nb = NO_RECEIVER;

        for dir in 0..8 {
            let nr = r as isize + DZ8[dir];
            let nc = col as isize + DX8[dir];
            if nr < 0 || nc < 0 || nr >= h as isize || nc >= w as isize {
                continue;
            }
            let nb = nr as usize * w + nc as usize;
            if filled_h[nb] == elev_m[nb] {
                let sill = elev_m[nb].max(h_c);
                if sill < best_ext_h {
                    best_ext_h = sill;
                    best_ext_nb = nb as u32;
                }
            } else if cell_leaf[nb] != NO_RECEIVER {
                let root = find_root(&mut dsu, cell_leaf[nb]);
                if !neigh_roots[..nr_len].contains(&root) {
                    neigh_roots[nr_len] = root;
                    nr_len += 1;
                }
            }
        }

        if nr_len == 0 {
            let id = nodes.len() as u32;
            nodes.push(DepNode {
                pit_elev_m: h_c,
                spill_elev_m: f32::INFINITY,
                outlet_cell: NO_RECEIVER,
                ext_spill_h: best_ext_h,
                ext_outlet_cell: best_ext_nb,
                children: None,
                parent_node: NO_RECEIVER,
                own_cells: vec![c_u32],
            });
            dsu.push(id);
            cell_leaf[c_idx] = id;
        } else if nr_len == 1 {
            let root = neigh_roots[0];
            nodes[root as usize].own_cells.push(c_u32);
            if best_ext_h < nodes[root as usize].ext_spill_h {
                nodes[root as usize].ext_spill_h = best_ext_h;
                nodes[root as usize].ext_outlet_cell = best_ext_nb;
            }
            cell_leaf[c_idx] = root;
        } else {
            let mut cur = neigh_roots[0];
            for &other in &neigh_roots[1..nr_len] {
                let p_id = nodes.len() as u32;
                let pit_min = nodes[cur as usize]
                    .pit_elev_m
                    .min(nodes[other as usize].pit_elev_m);
                let (ext_h, ext_nb) =
                    if nodes[cur as usize].ext_spill_h <= nodes[other as usize].ext_spill_h {
                        (
                            nodes[cur as usize].ext_spill_h,
                            nodes[cur as usize].ext_outlet_cell,
                        )
                    } else {
                        (
                            nodes[other as usize].ext_spill_h,
                            nodes[other as usize].ext_outlet_cell,
                        )
                    };
                nodes[cur as usize].spill_elev_m = h_c;
                nodes[cur as usize].outlet_cell = c_u32;
                nodes[cur as usize].parent_node = p_id;
                nodes[other as usize].spill_elev_m = h_c;
                nodes[other as usize].outlet_cell = c_u32;
                nodes[other as usize].parent_node = p_id;

                nodes.push(DepNode {
                    pit_elev_m: pit_min,
                    spill_elev_m: f32::INFINITY,
                    outlet_cell: NO_RECEIVER,
                    ext_spill_h: ext_h,
                    ext_outlet_cell: ext_nb,
                    children: Some((cur, other)),
                    parent_node: NO_RECEIVER,
                    own_cells: Vec::new(),
                });
                dsu.push(p_id);
                dsu[cur as usize] = p_id;
                dsu[other as usize] = p_id;
                cur = p_id;
            }
            if best_ext_h < nodes[cur as usize].ext_spill_h {
                nodes[cur as usize].ext_spill_h = best_ext_h;
                nodes[cur as usize].ext_outlet_cell = best_ext_nb;
            }
            nodes[cur as usize].own_cells.push(c_u32);
            cell_leaf[c_idx] = cur;
        }
    }

    for node in &mut nodes {
        if node.parent_node == NO_RECEIVER {
            node.spill_elev_m = node.ext_spill_h;
            node.outlet_cell = node.ext_outlet_cell;
        }
    }

    // 3. Route non-depression catchment runoff into depression nodes (zero double counting).
    let cell_area_m2 = f64::from(cfg.dx_m) * f64::from(cfg.dx_m);
    let cell_area_f32 = cfg.dx_m * cfg.dx_m;
    let node_count = nodes.len();
    let mut node_inflow_m3_yr = vec![0.0_f64; node_count];
    if node_count > 0 {
        let mut in_deg = vec![0_u8; n];
        for i in 0..n {
            if cell_leaf[i] == NO_RECEIVER {
                let rec = d8_rec[i];
                if rec != NO_RECEIVER && cell_leaf[rec as usize] == NO_RECEIVER {
                    in_deg[rec as usize] = in_deg[rec as usize].saturating_add(1);
                }
            }
        }
        let mut runoff_acc = vec![0.0_f32; n];
        let mut queue: VecDeque<u32> = VecDeque::new();
        for i in 0..n {
            let q_loc = (input.precip_m_yr[i] - input.pet_m_yr[i]).max(0.0) * cell_area_f32;
            runoff_acc[i] = q_loc;
            if cell_leaf[i] == NO_RECEIVER && in_deg[i] == 0 {
                queue.push_back(i as u32);
            }
        }
        while let Some(u_u32) = queue.pop_front() {
            let u = u_u32 as usize;
            let rec = d8_rec[u];
            if rec == NO_RECEIVER {
                continue;
            }
            let rec_u = rec as usize;
            let leaf_rec = cell_leaf[rec_u];
            if leaf_rec != NO_RECEIVER {
                node_inflow_m3_yr[leaf_rec as usize] += f64::from(runoff_acc[u]);
            } else {
                runoff_acc[rec_u] += runoff_acc[u];
                in_deg[rec_u] = in_deg[rec_u].saturating_sub(1);
                if in_deg[rec_u] == 0 {
                    queue.push_back(rec);
                }
            }
        }
        for i in 0..n {
            let lf = cell_leaf[i];
            if lf != NO_RECEIVER {
                node_inflow_m3_yr[lf as usize] += f64::from(runoff_acc[i]);
            }
        }
    }
    drop(cell_leaf);

    // 4. Bottom-up Barnes et al. (2021) Fill-Spill-Merge evaluation.
    let mut subtree_inflow = vec![0.0_f64; node_count];
    let mut subtree_pet_sum = vec![0.0_f64; node_count];
    let mut subtree_p_sum = vec![0.0_f64; node_count];
    let mut subtree_cnt = vec![0_usize; node_count];
    for u in 0..node_count {
        let mut q = node_inflow_m3_yr[u];
        let mut pet_s = 0.0_f64;
        let mut p_s = 0.0_f64;
        let mut cnt = nodes[u].own_cells.len();
        for &c in &nodes[u].own_cells {
            pet_s += f64::from(input.pet_m_yr[c as usize]);
            p_s += f64::from(input.precip_m_yr[c as usize]);
        }
        if let Some((a, b)) = nodes[u].children {
            q += subtree_inflow[a as usize] + subtree_inflow[b as usize];
            pet_s += subtree_pet_sum[a as usize] + subtree_pet_sum[b as usize];
            p_s += subtree_p_sum[a as usize] + subtree_p_sum[b as usize];
            cnt += subtree_cnt[a as usize] + subtree_cnt[b as usize];
        }
        subtree_inflow[u] = q;
        subtree_pet_sum[u] = pet_s;
        subtree_p_sum[u] = p_s;
        subtree_cnt[u] = cnt;
    }

    struct PendingLake {
        node_idx: u32,
        water_level_m: f32,
        is_endorheic: bool,
        q_in_m3_yr: f64,
        q_out_m3_yr: f64,
        spills_into_sib: Option<u32>,
    }

    struct FsmState<'a> {
        nodes: &'a [DepNode],
        elev_m: &'a [f32],
        precip_m_yr: &'a [f32],
        pet_m_yr: &'a [f32],
        subtree_pet_sum: &'a [f64],
        subtree_p_sum: &'a [f64],
        subtree_cnt: &'a [usize],
        cell_area_m2: f64,
        w: usize,
        filled_to_sill: Vec<bool>,
        excess_q: Vec<f64>,
        absorbed_q: Vec<f64>,
        suppressed_child: Vec<bool>,
        pending: Vec<PendingLake>,
        pending_idx: Vec<u32>,
        scratch_cells: Vec<u32>,
        scratch_elevs: Vec<f32>,
    }

    impl<'a> FsmState<'a> {
        fn upsert_pending(&mut self, lake: PendingLake) {
            let u = lake.node_idx as usize;
            let slot = self.pending_idx[u];
            if slot != NO_RECEIVER {
                self.pending[slot as usize] = lake;
            } else {
                let idx = self.pending.len() as u32;
                self.pending.push(lake);
                self.pending_idx[u] = idx;
            }
        }

        fn evap_to_sill(&mut self, u: u32, spill: f32) -> (f64, f64, f64) {
            self.scratch_cells.clear();
            collect_subtree_cells(self.nodes, u, &mut self.scratch_cells);
            let cnt_below = self
                .scratch_cells
                .iter()
                .filter(|&&c| self.elev_m[c as usize] <= spill)
                .count();
            let denom = self.subtree_cnt[u as usize].max(1) as f64;
            let mean_pet = (self.subtree_pet_sum[u as usize] / denom).max(0.05);
            let mean_p = self.subtree_p_sum[u as usize] / denom;
            let evap = (cnt_below as f64) * self.cell_area_m2 * mean_pet;
            (evap, mean_pet, mean_p)
        }

        fn evaluate_basin(&mut self, u: u32, q_total: f64) {
            let spill = self.nodes[u as usize].spill_elev_m;
            if !spill.is_finite() {
                return;
            }
            let (evap_full, mean_pet, mean_p) = self.evap_to_sill(u, spill);
            let is_humid_mean = (mean_p / mean_pet) >= 0.5;
            let q_pos = q_total.max(0.0);

            if q_pos > evap_full {
                self.filled_to_sill[u as usize] = true;
                let ex = q_pos - evap_full;
                self.excess_q[u as usize] = ex;
                self.upsert_pending(PendingLake {
                    node_idx: u,
                    water_level_m: spill,
                    is_endorheic: false,
                    q_in_m3_yr: q_pos,
                    q_out_m3_yr: ex,
                    spills_into_sib: None,
                });
            } else if is_humid_mean {
                // Humid zones (P/PET >= 0.5) never form permanent EndorheicLake salt pans.
                self.filled_to_sill[u as usize] = true;
                let seasonal_out = (q_pos * 0.15).max(100.0);
                self.excess_q[u as usize] = seasonal_out;
                self.upsert_pending(PendingLake {
                    node_idx: u,
                    water_level_m: spill,
                    is_endorheic: false,
                    q_in_m3_yr: q_pos,
                    q_out_m3_yr: seasonal_out,
                    spills_into_sib: None,
                });
            } else {
                self.scratch_elevs.clear();
                self.scratch_elevs
                    .extend(self.scratch_cells.iter().map(|&c| self.elev_m[c as usize]));
                let h_eq = solve_equilibrium_level(
                    &self.scratch_elevs,
                    self.nodes[u as usize].pit_elev_m,
                    spill,
                    q_pos,
                    mean_pet,
                    self.cell_area_m2,
                );
                if has_any_humid_submerged(
                    &self.scratch_cells,
                    self.elev_m,
                    h_eq,
                    self.precip_m_yr,
                    self.pet_m_yr,
                ) {
                    // Transition-zone basin intersecting humid cells (P/PET >= 0.5) is treated as humid.
                    self.filled_to_sill[u as usize] = true;
                    let seasonal_out = (q_pos * 0.15).max(100.0);
                    self.excess_q[u as usize] = seasonal_out;
                    self.upsert_pending(PendingLake {
                        node_idx: u,
                        water_level_m: spill,
                        is_endorheic: false,
                        q_in_m3_yr: q_pos,
                        q_out_m3_yr: seasonal_out,
                        spills_into_sib: None,
                    });
                } else {
                    self.upsert_pending(PendingLake {
                        node_idx: u,
                        water_level_m: h_eq,
                        is_endorheic: true,
                        q_in_m3_yr: q_pos,
                        q_out_m3_yr: 0.0,
                        spills_into_sib: None,
                    });
                }
            }
        }

        fn route_spill_into_subtree(&mut self, node_u: u32, q_spill: f64, entry_outlet_cell: u32) {
            self.absorbed_q[node_u as usize] += q_spill;
            let q_tot = self.absorbed_q[node_u as usize];
            if self.pending_idx[node_u as usize] != NO_RECEIVER
                || self.nodes[node_u as usize].children.is_none()
            {
                self.evaluate_basin(node_u, q_tot);
                return;
            }
            let Some((a, b)) = self.nodes[node_u as usize].children else {
                self.evaluate_basin(node_u, q_tot);
                return;
            };
            let fa = self.filled_to_sill[a as usize];
            let fb = self.filled_to_sill[b as usize];
            if fa ^ fb {
                let (full_c, empty_c) = if fa { (a, b) } else { (b, a) };
                let s_inner = self.nodes[empty_c as usize].spill_elev_m;
                let (evap_empty, _, _) = self.evap_to_sill(empty_c, s_inner);
                if self.absorbed_q[empty_c as usize] + q_spill <= evap_empty {
                    let outlet = self.nodes[full_c as usize].outlet_cell;
                    self.route_spill_into_subtree(empty_c, q_spill, outlet);
                } else {
                    suppress_subtree_descendants(self.nodes, node_u, &mut self.suppressed_child);
                    self.evaluate_basin(node_u, q_tot);
                }
            } else {
                let (recv_c, other_c) = pick_receiving_child(
                    self.nodes,
                    a,
                    b,
                    entry_outlet_cell,
                    self.w,
                    &mut self.scratch_cells,
                );
                let s_inner = self.nodes[recv_c as usize].spill_elev_m;
                let (evap_recv, _, _) = self.evap_to_sill(recv_c, s_inner);
                let q_recv_new = self.absorbed_q[recv_c as usize] + q_spill;
                if q_recv_new <= evap_recv {
                    self.route_spill_into_subtree(recv_c, q_spill, entry_outlet_cell);
                } else {
                    let excess = q_recv_new - evap_recv;
                    let (evap_other, _, _) = self.evap_to_sill(other_c, s_inner);
                    if self.absorbed_q[other_c as usize] + excess <= evap_other {
                        suppress_subtree_descendants(
                            self.nodes,
                            recv_c,
                            &mut self.suppressed_child,
                        );
                        self.absorbed_q[recv_c as usize] = q_recv_new;
                        self.filled_to_sill[recv_c as usize] = true;
                        self.excess_q[recv_c as usize] = excess;
                        self.upsert_pending(PendingLake {
                            node_idx: recv_c,
                            water_level_m: s_inner,
                            is_endorheic: false,
                            q_in_m3_yr: q_recv_new,
                            q_out_m3_yr: excess,
                            spills_into_sib: Some(other_c),
                        });
                        let outlet = self.nodes[recv_c as usize].outlet_cell;
                        self.route_spill_into_subtree(other_c, excess, outlet);
                    } else {
                        suppress_subtree_descendants(
                            self.nodes,
                            node_u,
                            &mut self.suppressed_child,
                        );
                        self.evaluate_basin(node_u, q_tot);
                    }
                }
            }
        }
    }

    let mut fsm = FsmState {
        nodes: &nodes,
        elev_m,
        precip_m_yr: input.precip_m_yr,
        pet_m_yr: input.pet_m_yr,
        subtree_pet_sum: &subtree_pet_sum,
        subtree_p_sum: &subtree_p_sum,
        subtree_cnt: &subtree_cnt,
        cell_area_m2,
        w,
        filled_to_sill: vec![false; node_count],
        excess_q: vec![0.0_f64; node_count],
        absorbed_q: subtree_inflow,
        suppressed_child: vec![false; node_count],
        pending: Vec::new(),
        pending_idx: vec![NO_RECEIVER; node_count],
        scratch_cells: Vec::new(),
        scratch_elevs: Vec::new(),
    };

    for u in 0..node_count {
        let q_total = fsm.absorbed_q[u];
        // Handle internal merge node children spillover logic.
        if let Some((a, b)) = nodes[u].children {
            let fa = fsm.filled_to_sill[a as usize];
            let fb = fsm.filled_to_sill[b as usize];
            if fa && fb {
                // Both filled to internal sill -> merge cleanly into `u` and suppress entire subtree.
                suppress_subtree_descendants(&nodes, u as u32, &mut fsm.suppressed_child);
            } else if fa ^ fb {
                let (full_c, empty_c) = if fa { (a, b) } else { (b, a) };
                let sib_spill = nodes[empty_c as usize].spill_elev_m;
                let (evap_sib, _, _) = fsm.evap_to_sill(empty_c, sib_spill);
                let excess = fsm.excess_q[full_c as usize];
                let q_sib_new = fsm.absorbed_q[empty_c as usize] + excess;
                if q_sib_new > evap_sib {
                    suppress_subtree_descendants(&nodes, u as u32, &mut fsm.suppressed_child);
                } else {
                    // Sibling absorbs spillover below sill `S` without merging into `u`.
                    let slot = fsm.pending_idx[full_c as usize];
                    if slot != NO_RECEIVER {
                        fsm.pending[slot as usize].spills_into_sib = Some(empty_c);
                    }
                    let outlet = nodes[full_c as usize].outlet_cell;
                    fsm.route_spill_into_subtree(empty_c, excess, outlet);
                    continue;
                }
            } else {
                // Neither child reached internal sill; keep independent children.
                continue;
            }
        }

        fsm.evaluate_basin(u as u32, q_total);
    }

    let suppressed_child = fsm.suppressed_child;
    let pending = fsm.pending;
    let mut scratch_cells = fsm.scratch_cells;

    // 5. Retain basins meeting depth/cell thresholds, stamp `lake_id`, `water_h_m`, `water_kind`,
    //    and trace continuous D8 spillways for overflowing lakes.
    let mut lake_id = vec![0_u32; n];
    let mut basins: Vec<LakeBasinRecord> = Vec::new();
    let mut severed_humid_spillways = 0usize;
    let mut total_lake_cells = 0usize;
    let mut endorheic_cells = 0usize;
    let mut endorheic_humid_cells = 0usize;
    let mut land_cells = 0usize;

    for i in 0..n {
        if elev_m[i] > cfg.sea_level_m {
            land_cells += 1;
        }
        if input.is_channel[i]
            && elev_m[i] > cfg.sea_level_m
            && water_kind[i] == WaterKind::None as u8
        {
            water_kind[i] = WaterKind::River as u8;
        }
    }

    let mut spill_traces: Vec<(u32, f32)> = Vec::new();
    let mut internal_spill_traces: Vec<(u32, u32, f32, u32)> = Vec::new();

    for cand in pending {
        if suppressed_child[cand.node_idx as usize] {
            continue;
        }
        scratch_cells.clear();
        collect_subtree_cells(&nodes, cand.node_idx, &mut scratch_cells);

        let mut submerged: Vec<u32> = Vec::with_capacity(scratch_cells.len());
        let mut max_depth = 0.0_f32;
        let mut elev_sum = 0.0_f64;
        for &c in &scratch_cells {
            let h_c = elev_m[c as usize];
            if h_c <= cand.water_level_m {
                submerged.push(c);
                max_depth = max_depth.max(cand.water_level_m - h_c);
                elev_sum += f64::from(h_c);
            }
        }
        if submerged.is_empty() {
            continue;
        }
        let mean_elev = (elev_sum / submerged.len() as f64) as f32;
        let min_cells = if mean_elev >= cfg.paramo_elev_m {
            cfg.min_pond_cells
        } else {
            cfg.min_lake_cells
        };
        if max_depth < cfg.min_lake_depth_m || submerged.len() < min_cells {
            continue;
        }

        let next_id = (basins.len() + 1) as u32;
        let wk = if cand.is_endorheic {
            WaterKind::EndorheicLake as u8
        } else {
            WaterKind::FreshwaterLake as u8
        };

        for &c in &submerged {
            let cu = c as usize;
            lake_id[cu] = next_id;
            water_h_m[cu] = cand.water_level_m;
            water_kind[cu] = wk;
            total_lake_cells += 1;
            if cand.is_endorheic {
                endorheic_cells += 1;
                let pet_c = input.pet_m_yr[cu].max(0.05);
                if (input.precip_m_yr[cu] / pet_c) >= 0.5 {
                    endorheic_humid_cells += 1;
                }
            }
        }

        let (max_wall_slope_deg, elongation) = compute_wall_slope_and_elongation(
            &submerged,
            elev_m,
            cand.water_level_m,
            w,
            h,
            cfg.dx_m,
        );
        let area_m2 = submerged.len() as f64 * cell_area_m2;

        let classification = if cand.is_endorheic {
            LakeClassification::EndorheicClosed
        } else if max_wall_slope_deg > 25.0 || elongation > 3.5 {
            LakeClassification::CalderaOrTrough
        } else if mean_elev >= cfg.paramo_elev_m {
            if area_m2 >= 5_000.0 {
                LakeClassification::HighParamoGlacial
            } else {
                LakeClassification::ParamoCirquePond
            }
        } else {
            LakeClassification::FreshwaterSpilling
        };

        let mut reaches_outlet = false;
        let outlet_cell = if cand.is_endorheic {
            NO_RECEIVER
        } else {
            nodes[cand.node_idx as usize].outlet_cell
        };

        if !cand.is_endorheic && cand.q_out_m3_yr > 0.0 {
            if cfg.enable_fill_spill_merge && outlet_cell != NO_RECEIVER {
                reaches_outlet = true;
                let q_s = (cand.q_out_m3_yr / SECONDS_PER_YEAR) as f32;
                if let Some(target_sib) = cand.spills_into_sib {
                    internal_spill_traces.push((outlet_cell, next_id, q_s, target_sib));
                } else {
                    spill_traces.push((outlet_cell, q_s));
                }
            } else {
                severed_humid_spillways += 1;
            }
        }

        basins.push(LakeBasinRecord {
            id: next_id,
            water_kind: wk,
            classification,
            water_level_m: cand.water_level_m,
            spill_elev_m: nodes[cand.node_idx as usize].spill_elev_m,
            pit_elev_m: nodes[cand.node_idx as usize].pit_elev_m,
            area_m2,
            max_depth_m: max_depth,
            mean_elev_m: mean_elev,
            max_wall_slope_deg,
            elongation,
            outlet_cell,
            q_in_m3_s: (cand.q_in_m3_yr / SECONDS_PER_YEAR) as f32,
            q_out_m3_s: if cand.is_endorheic {
                0.0
            } else {
                (cand.q_out_m3_yr / SECONDS_PER_YEAR) as f32
            },
            spillway_reaches_outlet: reaches_outlet,
        });
    }

    // Trace continuous D8 spillways downstream until hitting ocean, another lake, or border.
    if cfg.enable_fill_spill_merge {
        for (start_cell, q_out) in spill_traces {
            let mut cur = start_cell;
            let mut steps = 0usize;
            while cur != NO_RECEIVER && steps < n {
                let cu = cur as usize;
                if elev_m[cu] <= cfg.sea_level_m {
                    break;
                }
                if lake_id[cu] == 0 {
                    water_kind[cu] = WaterKind::River as u8;
                    let depth = (0.25 * libm::powf(q_out.max(0.05), 0.40)).clamp(0.15, 3.0);
                    water_h_m[cu] = water_h_m[cu].max(elev_m[cu] + depth);
                }
                let next = d8_rec[cu];
                if next == NO_RECEIVER || next == cur {
                    break;
                }
                if lake_id[next as usize] != 0 && cur != start_cell {
                    break;
                }
                cur = next;
                steps += 1;
            }
        }

        // Trace internal sibling spillways downhill on unfilled `elev_m` into `target_sib`
        // (never using `d8_rec` over `filled_h` so water stays inside `u`'s rim).
        for (start_cell, full_id, q_out, target_sib) in internal_spill_traces {
            scratch_cells.clear();
            collect_subtree_cells(&nodes, target_sib, &mut scratch_cells);
            scratch_cells.sort_unstable();
            let depth = (0.25 * libm::powf(q_out.max(0.05), 0.40)).clamp(0.15, 3.0);
            let mut cur = start_cell;
            let mut steps = 0usize;
            while cur != NO_RECEIVER && steps < n {
                let cu = cur as usize;
                if elev_m[cu] <= cfg.sea_level_m {
                    break;
                }
                if lake_id[cu] == 0 {
                    water_kind[cu] = WaterKind::River as u8;
                    water_h_m[cu] = water_h_m[cu].max(elev_m[cu] + depth);
                }
                let r = cu / w;
                let c = cu % w;
                let mut best_nb = NO_RECEIVER;
                let mut best_slope = 0.0_f32;
                let mut best_in_sib = false;
                for dir in 0..8 {
                    let nr = r as isize + DZ8[dir];
                    let nc = c as isize + DX8[dir];
                    if nr < 0 || nc < 0 || nr >= h as isize || nc >= w as isize {
                        continue;
                    }
                    let nb = nr as usize * w + nc as usize;
                    if lake_id[nb] == full_id {
                        continue;
                    }
                    let in_sib = scratch_cells.binary_search(&(nb as u32)).is_ok()
                        || (lake_id[nb] != 0 && lake_id[nb] != full_id);
                    let dh = elev_m[cu] - elev_m[nb];
                    let s = dh / DIST8[dir];
                    if cur == start_cell {
                        if in_sib && (!best_in_sib || s > best_slope) {
                            best_in_sib = true;
                            best_slope = s;
                            best_nb = nb as u32;
                        } else if !best_in_sib && dh > 0.0 && s > best_slope {
                            best_slope = s;
                            best_nb = nb as u32;
                        }
                    } else if dh > 0.0 && s > best_slope {
                        best_slope = s;
                        best_nb = nb as u32;
                    }
                }
                if best_nb == NO_RECEIVER {
                    break;
                }
                if lake_id[best_nb as usize] != 0 {
                    break;
                }
                cur = best_nb;
                steps += 1;
            }
        }
    }

    // 6. Micro-pit cleanup outside retained lakes (only when Fill-Spill-Merge is enabled).
    // If retained EndorheicLake basins exist (`water_level_m < spill_elev_m`), recompute
    // Priority-Flood seeded with those EndorheicLake surfaces so their dry interior bowl walls
    // retain their exact pre-existing slope instead of being flattened up to `spill_elev_m`.
    if cfg.enable_fill_spill_merge {
        if endorheic_cells > 0 {
            filled_h.copy_from_slice(elev_m);
            let mut closed2 = vec![0_u64; word_count];
            let mut open2: BinaryHeap<Reverse<(u32, u32)>> =
                BinaryHeap::with_capacity(w * 2 + h * 2);
            let mut pit_q2: VecDeque<(u32, f32)> = VecDeque::new();
            for r in 0..h {
                for c in 0..w {
                    let idx = r * w + c;
                    let border = r == 0 || r + 1 == h || c == 0 || c + 1 == w;
                    let marine = filled_h[idx] <= cfg.sea_level_m;
                    let endo_seed =
                        lake_id[idx] != 0 && water_kind[idx] == WaterKind::EndorheicLake as u8;
                    if endo_seed {
                        filled_h[idx] = water_h_m[idx];
                    }
                    if border || marine || endo_seed {
                        bit_set(&mut closed2, idx);
                        open2.push(Reverse((float_ord_key(filled_h[idx]), idx as u32)));
                    }
                }
            }
            while !open2.is_empty() || !pit_q2.is_empty() {
                let (cell, current_h) = if let Some((c, hc)) = pit_q2.pop_front() {
                    (c as usize, hc)
                } else {
                    let Reverse((_, c)) = open2.pop().expect("open2 heap non-empty");
                    let cu = c as usize;
                    (cu, filled_h[cu])
                };
                let r = cell / w;
                let c = cell % w;
                for dir in 0..8 {
                    let nr = r as isize + DZ8[dir];
                    let nc = c as isize + DX8[dir];
                    if nr < 0 || nc < 0 || nr >= h as isize || nc >= w as isize {
                        continue;
                    }
                    let nb = nr as usize * w + nc as usize;
                    if bit_get(&closed2, nb) {
                        continue;
                    }
                    bit_set(&mut closed2, nb);
                    if filled_h[nb] <= current_h {
                        let raised = current_h.next_up();
                        filled_h[nb] = raised;
                        pit_q2.push_back((nb as u32, raised));
                    } else {
                        open2.push(Reverse((float_ord_key(filled_h[nb]), nb as u32)));
                    }
                }
            }
        }

        for i in 0..n {
            if lake_id[i] == 0
                && (water_kind[i] == WaterKind::FreshwaterLake as u8
                    || water_kind[i] == WaterKind::EndorheicLake as u8)
            {
                water_kind[i] = if input.is_channel[i] {
                    WaterKind::River as u8
                } else {
                    WaterKind::None as u8
                };
            }
            if elev_m[i] > cfg.sea_level_m && filled_h[i] > elev_m[i] && lake_id[i] == 0 {
                let delta = filled_h[i] - elev_m[i];
                elev_m[i] = filled_h[i];
                sediment_s[i] += delta;
                water_h_m[i] = water_h_m[i].max(elev_m[i]);
            }
        }
    }

    let unmodeled_micro_pits = if cfg.enable_fill_spill_merge {
        let mut rem_pits = 0usize;
        for r in 1..(h - 1) {
            for c in 1..(w - 1) {
                let idx = r * w + c;
                let hi = elev_m[idx];
                if hi <= cfg.sea_level_m || lake_id[idx] != 0 {
                    continue;
                }
                let mut has_lower = false;
                for dir in 0..8 {
                    let nr = (r as isize + DZ8[dir]) as usize;
                    let nc = (c as isize + DX8[dir]) as usize;
                    if elev_m[nr * w + nc] < hi {
                        has_lower = true;
                        break;
                    }
                }
                if !has_lower {
                    rem_pits += 1;
                }
            }
        }
        rem_pits
    } else {
        0
    };

    // 7. Knickpoint waterfalls (`Howard, Dietrich & Seidl 1994`) + gorge lip & plunge pool carving.
    let max_steps = (cfg.waterfall_max_run_m / cfg.dx_m).round().clamp(1.0, 6.0) as usize;
    struct WaterfallCand {
        lip_cell: u32,
        base_cell: u32,
        drop_m: f32,
        q_m3_s: f32,
        path_cells: Vec<u32>,
    }
    let mut wf_cands: Vec<WaterfallCand> = Vec::new();

    for r in 1..(h - 1) {
        for c in 1..(w - 1) {
            let idx = r * w + c;
            if lake_id[idx] != 0 || elev_m[idx] <= cfg.sea_level_m + 1.0 {
                continue;
            }
            let active_stream = input.is_channel[idx] || water_kind[idx] == WaterKind::River as u8;
            let q = input.discharge_q_m3_s[idx];
            if !active_stream || q < cfg.waterfall_min_q_m3_s {
                continue;
            }

            // Estimate upstream channel slope from highest-discharge / steepest higher neighbor.
            let mut up_slope = 0.0_f32;
            let mut found_up = false;
            let mut best_up_q = -1.0_f32;
            for dir in 0..8 {
                let nr = (r as isize + DZ8[dir]) as usize;
                let nc = (c as isize + DX8[dir]) as usize;
                let nb = nr * w + nc;
                if elev_m[nb] >= elev_m[idx] && d8_rec[nb] == idx as u32 {
                    let s = (elev_m[nb] - elev_m[idx]) / (cfg.dx_m * DIST8[dir]);
                    let nq = input.discharge_q_m3_s[nb];
                    if nq > best_up_q {
                        best_up_q = nq;
                        up_slope = s;
                        found_up = true;
                    }
                }
            }
            if !found_up {
                // Fallback to gentlest higher neighbor slope.
                for dir in 0..8 {
                    let nr = (r as isize + DZ8[dir]) as usize;
                    let nc = (c as isize + DX8[dir]) as usize;
                    let nb = nr * w + nc;
                    if elev_m[nb] > elev_m[idx] {
                        let s = (elev_m[nb] - elev_m[idx]) / (cfg.dx_m * DIST8[dir]);
                        if !found_up || s < up_slope {
                            up_slope = s;
                            found_up = true;
                        }
                    }
                }
            }
            if found_up && up_slope >= cfg.waterfall_max_up_slope {
                continue;
            }

            // Walk downstream up to `max_steps` within `waterfall_max_run_m`.
            let mut cur = idx;
            let mut dist_m = 0.0_f32;
            let mut best_drop = 0.0_f32;
            let mut best_base = NO_RECEIVER;
            let mut path = vec![idx as u32];
            let mut best_path_len = 1usize;

            for _ in 0..max_steps {
                let rec = d8_rec[cur];
                if rec == NO_RECEIVER {
                    break;
                }
                let rec_u = rec as usize;
                if lake_id[rec_u] != 0 || elev_m[rec_u] <= cfg.sea_level_m {
                    break;
                }
                let rr = cur / w;
                let cc = cur % w;
                let nr = rec_u / w;
                let nc = rec_u % w;
                let step_d = libm::hypotf(
                    (nc as f32 - cc as f32) * cfg.dx_m,
                    (nr as f32 - rr as f32) * cfg.dx_m,
                );
                dist_m += step_d;
                if dist_m > cfg.waterfall_max_run_m + 1e-3 {
                    break;
                }
                path.push(rec);
                let drop = elev_m[idx] - elev_m[rec_u];
                if drop > best_drop {
                    best_drop = drop;
                    best_base = rec;
                    best_path_len = path.len();
                }
                cur = rec_u;
            }

            if best_base != NO_RECEIVER && best_drop >= cfg.waterfall_min_drop_m {
                path.truncate(best_path_len);
                wf_cands.push(WaterfallCand {
                    lip_cell: idx as u32,
                    base_cell: best_base,
                    drop_m: best_drop,
                    q_m3_s: q,
                    path_cells: path,
                });
            }
        }
    }

    wf_cands.sort_by(|a, b| {
        b.drop_m
            .total_cmp(&a.drop_m)
            .then_with(|| b.q_m3_s.total_cmp(&a.q_m3_s))
            .then_with(|| a.lip_cell.cmp(&b.lip_cell))
    });

    let min_sp_sq = cfg.waterfall_min_spacing_m * cfg.waterfall_min_spacing_m;
    let mut waterfalls: Vec<WaterfallRecord> = Vec::new();
    for cand in wf_cands {
        if waterfalls.len() >= cfg.waterfall_max_count {
            break;
        }
        let lip_u = cand.lip_cell as usize;
        let lr = lip_u / w;
        let lc = lip_u % w;
        let lx = (lc as f32 + 0.5) * cfg.dx_m;
        let lz = (lr as f32 + 0.5) * cfg.dx_m;

        let too_close = waterfalls.iter().any(|wf| {
            let dx = wf.x_m - lx;
            let dz = wf.z_m - lz;
            dx * dx + dz * dz < min_sp_sq
        });
        if too_close {
            continue;
        }

        let base_u = cand.base_cell as usize;
        let br = base_u / w;
        let bc = base_u % w;
        let yaw_rad = libm::atan2f(br as f32 - lr as f32, bc as f32 - lc as f32);

        // Carve gorge lip notch (preserving drop >= waterfall_min_drop_m).
        let max_notch = (elev_m[lip_u] - elev_m[base_u] - cfg.waterfall_min_drop_m).max(0.0);
        let notch = (0.08 * cand.drop_m).min(1.2).min(max_notch);
        if notch > 0.0 {
            let sed_cut = notch.min(sediment_s[lip_u]);
            sediment_s[lip_u] -= sed_cut;
            elev_m[lip_u] -= notch;
        }

        // Carve plunge pool at `base_cell` (clamped strictly above `sea_level_m`).
        let plunge_depth_m = (0.15 * cand.drop_m).clamp(1.5, 8.0);
        let pool_rad = ((1.5 * input.river_w_m[lip_u] / cfg.dx_m).round() as isize).clamp(1, 2);
        let pool_surface = elev_m[base_u] + 0.35;
        let min_pool_floor = cfg.sea_level_m + 0.15;
        let base_before = elev_m[base_u];
        for dr in -pool_rad..=pool_rad {
            for dc in -pool_rad..=pool_rad {
                let nr = br as isize + dr;
                let nc = bc as isize + dc;
                if nr < 1 || nc < 1 || nr >= (h - 1) as isize || nc >= (w - 1) as isize {
                    continue;
                }
                let dist = libm::hypotf(dr as f32, dc as f32);
                if dist > pool_rad as f32 + 0.25 {
                    continue;
                }
                let nb = nr as usize * w + nc as usize;
                if lake_id[nb] != 0 || elev_m[nb] <= cfg.sea_level_m + 0.5 {
                    continue;
                }
                let t = (1.0 - dist / (pool_rad as f32 + 0.5)).max(0.0);
                let max_cut = (elev_m[nb] - min_pool_floor).max(0.0);
                let cut = (plunge_depth_m * t).min(max_cut);
                if cut > 0.0 {
                    let sed_cut = cut.min(sediment_s[nb]);
                    sediment_s[nb] -= sed_cut;
                    elev_m[nb] -= cut;
                }
                water_h_m[nb] = water_h_m[nb].max(pool_surface);
                water_kind[nb] = WaterKind::Waterfall as u8;
            }
        }
        for &pc in &cand.path_cells {
            let pu = pc as usize;
            if elev_m[pu] > cfg.sea_level_m {
                water_kind[pu] = WaterKind::Waterfall as u8;
                water_h_m[pu] = water_h_m[pu].max(elev_m[pu] + 0.25);
            }
        }
        let actual_plunge = (base_before - elev_m[base_u]).max(0.0);

        waterfalls.push(WaterfallRecord {
            lip_cell: cand.lip_cell,
            base_cell: cand.base_cell,
            x_m: lx,
            z_m: lz,
            lip_elev_m: elev_m[lip_u],
            drop_m: cand.drop_m + actual_plunge - notch,
            yaw_rad,
            discharge_q_m3_s: cand.q_m3_s,
            plunge_depth_m: actual_plunge,
        });
    }

    drop(filled_h);
    drop(d8_rec);

    let land_denom = land_cells.max(1) as f32;
    Ok(LakesFallsField {
        width: w,
        height: h,
        lake_id,
        basins,
        waterfalls,
        severed_humid_spillways,
        unmodeled_micro_pits,
        endorheic_land_fraction: endorheic_cells as f32 / land_denom,
        endorheic_humid_fraction: endorheic_humid_cells as f32 / land_denom,
        lake_surface_fraction: total_lake_cells as f32 / land_denom,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_two_subbasin_terrain(n: usize) -> Vec<f32> {
        let mut elev = vec![0.0_f32; n * n];
        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                if c < 6 || c + 1 == n || r == 0 || r + 1 == n {
                    elev[idx] = -5.0;
                    continue;
                }
                // General slope draining West toward c < 6, with rim around (r=32, c=32)
                // enclosing two subbasins centered at (24, 34) and (40, 34) separated by saddle at (32, 34).
                let base = 80.0 + (c as f32 - 6.0) * 1.5;
                let d1 = libm::hypotf(r as f32 - 24.0, c as f32 - 34.0);
                let d2 = libm::hypotf(r as f32 - 40.0, c as f32 - 34.0);
                let sub1 = -38.0 * libm::expf(-(d1 / 6.5) * (d1 / 6.5));
                let sub2 = -34.0 * libm::expf(-(d2 / 6.5) * (d2 / 6.5));
                // Lower outlet notch on west rim at r = 32, c = 22.
                let d_notch = libm::hypotf(r as f32 - 32.0, c as f32 - 22.0);
                let notch = -14.0 * libm::expf(-(d_notch / 3.0) * (d_notch / 3.0));
                elev[idx] = base + sub1 + sub2 + notch;
            }
        }
        elev
    }

    #[test]
    fn fill_spill_merge_humid_spillways_reach_outlet() {
        let n = 64;
        let mut elev = synthetic_two_subbasin_terrain(n);
        let mut sed = vec![5.0_f32; n * n];
        let mut water_h = elev.clone();
        let mut water_kind = vec![0_u8; n * n];
        let precip = vec![3.2_f32; n * n];
        let pet = vec![1.3_f32; n * n];
        let q = vec![0.1_f32; n * n];
        let rw = vec![0.0_f32; n * n];
        let is_ch = vec![false; n * n];

        let input = LakesFallsInput {
            width: n,
            height: n,
            precip_m_yr: &precip,
            pet_m_yr: &pet,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            is_channel: &is_ch,
        };
        let cfg = LakesFallsConfig {
            dx_m: 10.0,
            enable_fill_spill_merge: true,
            ..LakesFallsConfig::default()
        };
        let field = generate_lakes_and_waterfalls(
            &mut elev,
            &mut sed,
            &mut water_h,
            &mut water_kind,
            &input,
            &cfg,
        )
        .expect("lakes");

        assert!(
            !field.basins.is_empty(),
            "expected at least 1 merged humid lake"
        );
        assert_eq!(field.severed_humid_spillways, 0);
        assert_eq!(field.endorheic_humid_fraction, 0.0);
        assert!(field
            .basins
            .iter()
            .any(|b| b.water_kind == WaterKind::FreshwaterLake as u8 && b.spillway_reaches_outlet));
        assert!(water_kind.contains(&(WaterKind::River as u8)));
    }

    #[test]
    fn negative_control_disabled_fill_spill_merge_severs_humid_outlets() {
        let n = 64;
        let mut elev = synthetic_two_subbasin_terrain(n);
        let mut sed = vec![5.0_f32; n * n];
        let mut water_h = elev.clone();
        let mut water_kind = vec![0_u8; n * n];
        let precip = vec![3.2_f32; n * n];
        let pet = vec![1.3_f32; n * n];
        let q = vec![0.1_f32; n * n];
        let rw = vec![0.0_f32; n * n];
        let is_ch = vec![false; n * n];

        let input = LakesFallsInput {
            width: n,
            height: n,
            precip_m_yr: &precip,
            pet_m_yr: &pet,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            is_channel: &is_ch,
        };
        let cfg = LakesFallsConfig {
            dx_m: 10.0,
            enable_fill_spill_merge: false,
            ..LakesFallsConfig::default()
        };
        let field = generate_lakes_and_waterfalls(
            &mut elev,
            &mut sed,
            &mut water_h,
            &mut water_kind,
            &input,
            &cfg,
        )
        .expect("lakes neg control");

        assert!(
            field.severed_humid_spillways > 0,
            "disabling Fill-Spill-Merge must leave severed humid spillways > 0"
        );
    }

    #[test]
    fn endorheic_rain_shadow_equilibrium_exactness() {
        let n = 64;
        let mut elev = vec![0.0_f32; n * n];
        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                if r == 0 || r + 1 == n || c == 0 || c + 1 == n {
                    elev[idx] = -5.0;
                } else {
                    let d = libm::hypotf(r as f32 - 32.0, c as f32 - 32.0);
                    elev[idx] = if d <= 12.0 {
                        // Interior dry pit rising from 58 m at center to 100 m at rim d=12.
                        58.0 + 42.0 * (d / 12.0) * (d / 12.0)
                    } else if d <= 24.0 {
                        // Surrounding catchment rising inward-to-outward from 102 m to 138 m (drains INWARD).
                        102.0 + (d - 12.0) * 3.0
                    } else {
                        // Outer flank sloping down to border.
                        138.0 - (d - 24.0) * 4.5
                    };
                }
            }
        }
        // Carve a single spillway saddle & outlet channel on East side (r=32, c >= 44) at 100.0 m
        // so the wet catchment ring (d in 13..23, elev > 100 m) stays OUTSIDE the depression!
        for c in 44..(n - 1) {
            elev[32 * n + c] = (100.0 - (c - 44) as f32 * 2.0).max(1.0);
        }
        let mut sed = vec![2.0_f32; n * n];
        let mut water_h = elev.clone();
        let mut water_kind = vec![0_u8; n * n];
        // Arid basin P/PET < 0.5 inside pit (d <= 12) with wet catchment ring outside rim (d in 14..22).
        let mut precip = vec![0.25_f32; n * n];
        let pet = vec![1.35_f32; n * n];
        for r in 6..58 {
            for c in 6..58 {
                let d = libm::hypotf(r as f32 - 32.0, c as f32 - 32.0);
                if d > 14.0 && d < 22.0 && !(r == 32 && c >= 44) {
                    precip[r * n + c] = 1.65;
                }
            }
        }
        let q = vec![0.05_f32; n * n];
        let rw = vec![0.0_f32; n * n];
        let is_ch = vec![false; n * n];

        let input = LakesFallsInput {
            width: n,
            height: n,
            precip_m_yr: &precip,
            pet_m_yr: &pet,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            is_channel: &is_ch,
        };
        let elev_before = elev.clone();
        let cfg = LakesFallsConfig::default();
        let field = generate_lakes_and_waterfalls(
            &mut elev,
            &mut sed,
            &mut water_h,
            &mut water_kind,
            &input,
            &cfg,
        )
        .expect("endorheic");

        let endo = field
            .basins
            .iter()
            .find(|b| b.water_kind == WaterKind::EndorheicLake as u8)
            .expect("expected EndorheicLake in arid closed basin");
        assert!(endo.water_level_m < endo.spill_elev_m);
        assert_eq!(endo.q_out_m3_s, 0.0);
        assert_eq!(field.endorheic_humid_fraction, 0.0);
        assert_eq!(field.unmodeled_micro_pits, 0);

        // Verify dry interior bowl wall cells (water_level_m < elev < spill_elev_m) keep
        // their pre-existing sloping elevation rather than being flattened to spill_elev_m.
        let mut dry_wall_checked = 0usize;
        for r in 1..(n - 1) {
            for c in 1..(n - 1) {
                let idx = r * n + c;
                let d = libm::hypotf(r as f32 - 32.0, c as f32 - 32.0);
                if d <= 12.0
                    && elev_before[idx] > endo.water_level_m + 0.5
                    && elev_before[idx] < endo.spill_elev_m - 0.5
                {
                    assert_eq!(field.lake_id[idx], 0);
                    assert!(
                        (elev[idx] - elev_before[idx]).abs() < 1e-4,
                        "dry endorheic interior wall cell {idx} was flattened from {} to {}",
                        elev_before[idx],
                        elev[idx]
                    );
                    assert!(elev[idx] < endo.spill_elev_m);
                    dry_wall_checked += 1;
                }
            }
        }
        assert!(dry_wall_checked >= 20, "expected dry interior bowl cells");

        let q_in_yr = f64::from(endo.q_in_m3_s) * SECONDS_PER_YEAR;
        let evap_yr = endo.area_m2 * 1.35;
        let rel_err = (evap_yr - q_in_yr).abs() / q_in_yr.max(1.0);
        assert!(
            rel_err < 0.10,
            "equilibrium A(h)*PET={evap_yr} vs Q_in={q_in_yr} (rel_err={rel_err})"
        );
    }

    #[test]
    fn waterfall_knickpoint_detection_plunge_pool_and_spacing() {
        let n = 64;
        let mut elev = vec![0.0_f32; n * n];
        let mut q = vec![0.0_f32; n * n];
        let mut rw = vec![0.0_f32; n * n];
        let mut is_ch = vec![false; n * n];
        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                if r == 0 || r + 1 == n || c == 0 || c + 1 == n {
                    elev[idx] = -2.0;
                    continue;
                }
                // Sloping valley West->East with two caprock steps 15 cells (150 m) apart.
                let dist_from_channel = (r as f32 - 32.0).abs();
                let step = if c < 20 {
                    100.0 - 0.3 * c as f32
                } else if c < 35 {
                    70.0 - 0.3 * c as f32
                } else {
                    40.0 - 0.3 * c as f32
                };
                elev[idx] = step + dist_from_channel * 2.5;
                if r == 32 && (2..62).contains(&c) {
                    is_ch[idx] = true;
                    q[idx] = 5.0;
                    rw[idx] = 6.0;
                }
            }
        }
        let mut sed = vec![10.0_f32; n * n];
        let mut water_h = elev.clone();
        let mut water_kind = vec![0_u8; n * n];
        let precip = vec![2.5_f32; n * n];
        let pet = vec![1.2_f32; n * n];

        let input = LakesFallsInput {
            width: n,
            height: n,
            precip_m_yr: &precip,
            pet_m_yr: &pet,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            is_channel: &is_ch,
        };
        let cfg = LakesFallsConfig {
            waterfall_min_spacing_m: 300.0,
            ..LakesFallsConfig::default()
        };
        let elev_before = elev.clone();
        let field = generate_lakes_and_waterfalls(
            &mut elev,
            &mut sed,
            &mut water_h,
            &mut water_kind,
            &input,
            &cfg,
        )
        .expect("waterfalls");

        assert_eq!(
            field.waterfalls.len(),
            1,
            "spacing filter (300 m) should retain 1 of the two 150 m-spaced steps"
        );
        let wf = &field.waterfalls[0];
        assert!(wf.drop_m >= 12.0);
        assert!(elev[wf.base_cell as usize] < elev_before[wf.base_cell as usize]);
        assert_eq!(
            water_kind[wf.base_cell as usize],
            WaterKind::Waterfall as u8
        );
    }

    #[test]
    fn micro_pit_cleanup_leaves_zero_unmodeled_pits() {
        let n = 64;
        let mut elev = vec![0.0_f32; n * n];
        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                if r == 0 || r + 1 == n || c == 0 || c + 1 == n {
                    elev[idx] = -2.0;
                } else {
                    elev[idx] = 20.0 + c as f32 * 0.5 + (r as f32 - 32.0).abs() * 0.2;
                }
            }
        }
        // Inject 1-cell sub-threshold micro-pits.
        elev[20 * n + 20] -= 0.15;
        elev[40 * n + 30] -= 0.20;

        let mut sed = vec![2.0_f32; n * n];
        let mut water_h = elev.clone();
        let mut water_kind = vec![0_u8; n * n];
        let precip = vec![2.5_f32; n * n];
        let pet = vec![1.2_f32; n * n];
        let q = vec![0.1_f32; n * n];
        let rw = vec![0.0_f32; n * n];
        let is_ch = vec![false; n * n];

        let input = LakesFallsInput {
            width: n,
            height: n,
            precip_m_yr: &precip,
            pet_m_yr: &pet,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            is_channel: &is_ch,
        };
        let cfg = LakesFallsConfig::default();
        let field = generate_lakes_and_waterfalls(
            &mut elev,
            &mut sed,
            &mut water_h,
            &mut water_kind,
            &input,
            &cfg,
        )
        .expect("cleanup");
        assert_eq!(field.unmodeled_micro_pits, 0);
    }

    fn three_basin_nested_terrain(n: usize) -> Vec<f32> {
        let mut elev = vec![0.0_f32; n * n];
        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                if r == 0 || r + 1 == n || c == 0 || c + 1 == n {
                    elev[idx] = -5.0;
                    continue;
                }
                let d_main = libm::hypotf(r as f32 - 48.0, c as f32 - 48.0);
                // Outer dome/rim peaking around d=34 at ~150 m, sloping down outside to border.
                let outer = if d_main <= 34.0 {
                    150.0 - 0.12 * (34.0 - d_main) * (34.0 - d_main)
                } else {
                    150.0 - (d_main - 34.0) * 8.0
                };
                // Three sub-pits: sub1 at (36, 36), sub2a at (54, 46), sub2b at (54, 62).
                // sub2a and sub2b have a lower saddle (~120 m) between them than between sub1 and sub2 (~132 m).
                let d1 = libm::hypotf(r as f32 - 36.0, c as f32 - 36.0);
                let d2a = libm::hypotf(r as f32 - 54.0, c as f32 - 46.0);
                let d2b = libm::hypotf(r as f32 - 54.0, c as f32 - 62.0);
                let p1 = -65.0 * libm::expf(-(d1 / 7.5) * (d1 / 7.5));
                let p2a = -52.0 * libm::expf(-(d2a / 6.5) * (d2a / 6.5));
                let p2b = -48.0 * libm::expf(-(d2b / 6.5) * (d2b / 6.5));
                // Lower outer spillway notch on west border at (48, 15).
                let d_out = libm::hypotf(r as f32 - 48.0, c as f32 - 15.0);
                let notch = -18.0 * libm::expf(-(d_out / 3.0) * (d_out / 3.0));
                elev[idx] = (outer + p1 + p2a + p2b + notch).max(1.0);
            }
        }
        elev
    }

    #[test]
    fn three_basin_multi_level_fsm_suppression_and_asymmetric_spillover() {
        let n = 96;

        // Case A: sub1 overflows and merges with unmerged internal node merge(sub2a, sub2b).
        let mut elev_a = three_basin_nested_terrain(n);
        let mut sed_a = vec![5.0_f32; n * n];
        let mut wh_a = elev_a.clone();
        let mut wk_a = vec![0_u8; n * n];
        let mut precip_a = vec![0.30_f32; n * n];
        let pet_a = vec![1.50_f32; n * n];
        for r in 20..46 {
            for c in 20..46 {
                precip_a[r * n + c] = 18.0;
            }
        }
        let q = vec![0.0_f32; n * n];
        let rw = vec![0.0_f32; n * n];
        let is_ch = vec![false; n * n];
        let input_a = LakesFallsInput {
            width: n,
            height: n,
            precip_m_yr: &precip_a,
            pet_m_yr: &pet_a,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            is_channel: &is_ch,
        };
        let cfg = LakesFallsConfig::default();
        let field_a = generate_lakes_and_waterfalls(
            &mut elev_a,
            &mut sed_a,
            &mut wh_a,
            &mut wk_a,
            &input_a,
            &cfg,
        )
        .expect("3-basin merge");

        assert!(!field_a.basins.is_empty());
        for b in &field_a.basins {
            let stamped = field_a.lake_id.iter().filter(|&&id| id == b.id).count();
            assert!(
                stamped > 0,
                "basin id={} is a phantom basin with stamped_cells=0",
                b.id
            );
        }

        // Case B: asymmetric fa ^ fb where sub1 spills moderate excess absorbed by empty_c = merge(sub2a, sub2b)
        // below the outer rim (`q_sib_new <= evap_sib`). Must not emit external river across outer rim!
        let mut elev_b = three_basin_nested_terrain(n);
        let mut sed_b = vec![5.0_f32; n * n];
        let mut wh_b = elev_b.clone();
        let mut wk_b = vec![0_u8; n * n];
        let mut precip_b = vec![0.18_f32; n * n];
        let pet_b = vec![4.50_f32; n * n];
        // Give sub1 just enough excess to spill over saddle into sub2a+sub2b without filling outer rim.
        for r in 28..44 {
            for c in 28..44 {
                precip_b[r * n + c] = 9.5;
            }
        }
        let input_b = LakesFallsInput {
            width: n,
            height: n,
            precip_m_yr: &precip_b,
            pet_m_yr: &pet_b,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            is_channel: &is_ch,
        };
        let field_b = generate_lakes_and_waterfalls(
            &mut elev_b,
            &mut sed_b,
            &mut wh_b,
            &mut wk_b,
            &input_b,
            &cfg,
        )
        .expect("3-basin asymmetric spill");
        for b in &field_b.basins {
            let stamped = field_b.lake_id.iter().filter(|&&id| id == b.id).count();
            assert!(
                stamped > 0,
                "asymmetric basin id={} has stamped_cells=0",
                b.id
            );
        }
        // Outer border vicinity (c < 18) must not receive a river spillway from internal absorption.
        let outer_rivers = (0..n)
            .flat_map(|r| (1..18).map(move |c| r * n + c))
            .filter(|&i| wk_b[i] == WaterKind::River as u8)
            .count();
        assert_eq!(
            outer_rivers, 0,
            "internally absorbed spillover must not trace uphill over outer rim"
        );
    }

    #[test]
    fn transition_zone_straddling_p_pet_half_never_emits_endorheic_humid() {
        let n = 64;
        let mut elev = vec![0.0_f32; n * n];
        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                if r == 0 || r + 1 == n || c == 0 || c + 1 == n {
                    elev[idx] = -5.0;
                } else {
                    let d = libm::hypotf(r as f32 - 32.0, c as f32 - 32.0);
                    elev[idx] = if d <= 14.0 {
                        50.0 + 60.0 * (d / 14.0) * (d / 14.0)
                    } else {
                        110.0 - (d - 14.0) * 3.0
                    };
                }
            }
        }
        let mut sed = vec![2.0_f32; n * n];
        let mut water_h = elev.clone();
        let mut water_kind = vec![0_u8; n * n];
        // Basin mean P/PET < 0.5, yet eastern half of submerged pit has P/PET >= 0.5.
        let mut precip = vec![0.25_f32; n * n];
        let pet = vec![1.20_f32; n * n];
        for r in 0..n {
            for c in 33..n {
                precip[r * n + c] = 0.72; // P/PET = 0.60 >= 0.5
            }
        }
        let q = vec![0.05_f32; n * n];
        let rw = vec![0.0_f32; n * n];
        let is_ch = vec![false; n * n];
        let input = LakesFallsInput {
            width: n,
            height: n,
            precip_m_yr: &precip,
            pet_m_yr: &pet,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            is_channel: &is_ch,
        };
        let field = generate_lakes_and_waterfalls(
            &mut elev,
            &mut sed,
            &mut water_h,
            &mut water_kind,
            &input,
            &LakesFallsConfig::default(),
        )
        .expect("transition zone");
        assert_eq!(field.endorheic_humid_fraction, 0.0);
    }

    #[test]
    fn coastal_waterfall_plunge_pool_clamped_above_sea_level() {
        let n = 64;
        let mut elev = vec![0.0_f32; n * n];
        let mut q = vec![0.0_f32; n * n];
        let mut rw = vec![0.0_f32; n * n];
        let mut is_ch = vec![false; n * n];
        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                if r == 0 || r + 1 == n || c == 0 || c + 1 == n || c >= 38 {
                    elev[idx] = -2.0;
                    continue;
                }
                let dist_ch = (r as f32 - 32.0).abs();
                let step = if c <= 33 {
                    26.0 - 0.1 * c as f32
                } else {
                    1.5 - 0.1 * (c - 34) as f32
                };
                elev[idx] = step + dist_ch * 2.0;
                if r == 32 && (2..=37).contains(&c) {
                    is_ch[idx] = true;
                    q[idx] = 8.0;
                    rw[idx] = 8.0;
                }
            }
        }
        let mut sed = vec![10.0_f32; n * n];
        let mut water_h = elev.clone();
        let mut water_kind = vec![0_u8; n * n];
        let precip = vec![2.5_f32; n * n];
        let pet = vec![1.2_f32; n * n];
        let input = LakesFallsInput {
            width: n,
            height: n,
            precip_m_yr: &precip,
            pet_m_yr: &pet,
            discharge_q_m3_s: &q,
            river_w_m: &rw,
            is_channel: &is_ch,
        };
        let field = generate_lakes_and_waterfalls(
            &mut elev,
            &mut sed,
            &mut water_h,
            &mut water_kind,
            &input,
            &LakesFallsConfig::default(),
        )
        .expect("coastal waterfall");
        assert!(!field.waterfalls.is_empty());
        for i in 0..(n * n) {
            if water_kind[i] == WaterKind::Waterfall as u8 {
                assert!(
                    elev[i] > 0.0,
                    "coastal waterfall cell {i} plunged below sea level: {}",
                    elev[i]
                );
            }
        }
    }
}
