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
#[derive(Debug, Clone)]
pub struct LakesFallsField {
    pub width: usize,
    pub height: usize,
    /// Per-cell basin ID (`0` = not a lake; `1..=K` maps to `basins[id - 1]`).
    pub lake_id: Vec<u32>,
    /// Water surface elevation in metres (`elev_m` on dry land).
    pub water_h_m: Vec<f32>,
    /// Per-cell canonical [`WaterKind`] discriminant (`0..=6`).
    pub water_kind: Vec<u8>,
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
        let mut runoff_acc = vec![0.0_f64; n];
        let mut queue = VecDeque::new();
        for i in 0..n {
            let q_loc =
                f64::from((input.precip_m_yr[i] - input.pet_m_yr[i]).max(0.0)) * cell_area_m2;
            runoff_acc[i] = q_loc;
            if cell_leaf[i] == NO_RECEIVER && in_deg[i] == 0 {
                queue.push_back(i);
            }
        }
        while let Some(u) = queue.pop_front() {
            let rec = d8_rec[u];
            if rec == NO_RECEIVER {
                continue;
            }
            let rec_u = rec as usize;
            let leaf_rec = cell_leaf[rec_u];
            if leaf_rec != NO_RECEIVER {
                node_inflow_m3_yr[leaf_rec as usize] += runoff_acc[u];
            } else {
                runoff_acc[rec_u] += runoff_acc[u];
                in_deg[rec_u] = in_deg[rec_u].saturating_sub(1);
                if in_deg[rec_u] == 0 {
                    queue.push_back(rec_u);
                }
            }
        }
        for i in 0..n {
            let lf = cell_leaf[i];
            if lf != NO_RECEIVER {
                node_inflow_m3_yr[lf as usize] += runoff_acc[i];
            }
        }
    }

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
    }

    let mut filled_to_sill = vec![false; node_count];
    let mut excess_q = vec![0.0_f64; node_count];
    let mut extra_inflow = vec![0.0_f64; node_count];
    let mut suppressed_child = vec![false; node_count];
    let mut pending: Vec<PendingLake> = Vec::new();
    let mut scratch_cells: Vec<u32> = Vec::new();
    let mut scratch_elevs: Vec<f32> = Vec::new();

    for u in 0..node_count {
        let mut q_total = subtree_inflow[u] + extra_inflow[u];
        // Handle internal merge node children spillover logic.
        if let Some((a, b)) = nodes[u].children {
            let a_u = a as usize;
            let b_u = b as usize;
            let fa = filled_to_sill[a_u];
            let fb = filled_to_sill[b_u];
            if fa && fb {
                // Both filled to internal sill -> merge cleanly into `u`.
                suppressed_child[a_u] = true;
                suppressed_child[b_u] = true;
            } else if fa ^ fb {
                let (full_c, empty_c) = if fa { (a, b) } else { (b, a) };
                // Spill excess from `full_c` into sibling `empty_c` and check if sibling now fills.
                let q_sib_new = subtree_inflow[empty_c as usize] + excess_q[full_c as usize];
                scratch_cells.clear();
                collect_subtree_cells(&nodes, empty_c, &mut scratch_cells);
                let sib_mean_pet = (subtree_pet_sum[empty_c as usize]
                    / subtree_cnt[empty_c as usize].max(1) as f64)
                    .max(0.05);
                let sib_spill = nodes[empty_c as usize].spill_elev_m;
                let sib_cnt_full = scratch_cells
                    .iter()
                    .filter(|&&c| elev_m[c as usize] <= sib_spill)
                    .count();
                let evap_sib = (sib_cnt_full as f64) * cell_area_m2 * sib_mean_pet;
                if q_sib_new > evap_sib {
                    suppressed_child[a_u] = true;
                    suppressed_child[b_u] = true;
                } else {
                    // Sibling absorbs spillover below sill `S` without merging into `u`.
                    extra_inflow[empty_c as usize] += excess_q[full_c as usize];
                    // Re-evaluate `empty_c` at equilibrium level below `S` if arid.
                    let sib_mean_p = subtree_p_sum[empty_c as usize]
                        / subtree_cnt[empty_c as usize].max(1) as f64;
                    scratch_elevs.clear();
                    scratch_elevs.extend(scratch_cells.iter().map(|&c| elev_m[c as usize]));
                    let h_eq = solve_equilibrium_level(
                        &scratch_elevs,
                        nodes[empty_c as usize].pit_elev_m,
                        sib_spill,
                        q_sib_new,
                        sib_mean_pet,
                        cell_area_m2,
                    );
                    let is_humid = (sib_mean_p / sib_mean_pet) >= 0.5;
                    if let Some(prev) = pending.iter_mut().find(|p| p.node_idx == empty_c) {
                        prev.water_level_m = if is_humid { sib_spill } else { h_eq };
                        prev.is_endorheic = !is_humid;
                        prev.q_in_m3_yr = q_sib_new;
                    }
                    continue;
                }
            } else {
                // Neither child reached internal sill; keep independent children.
                continue;
            }
        }

        let spill = nodes[u].spill_elev_m;
        if !spill.is_finite() {
            continue;
        }
        scratch_cells.clear();
        collect_subtree_cells(&nodes, u as u32, &mut scratch_cells);
        let cnt_below_spill = scratch_cells
            .iter()
            .filter(|&&c| elev_m[c as usize] <= spill)
            .count();
        let mean_pet = (subtree_pet_sum[u] / subtree_cnt[u].max(1) as f64).max(0.05);
        let mean_p = subtree_p_sum[u] / subtree_cnt[u].max(1) as f64;
        let evap_full = (cnt_below_spill as f64) * cell_area_m2 * mean_pet;
        let is_humid = (mean_p / mean_pet) >= 0.5;

        if q_total > evap_full {
            filled_to_sill[u] = true;
            excess_q[u] = q_total - evap_full;
            pending.push(PendingLake {
                node_idx: u as u32,
                water_level_m: spill,
                is_endorheic: false,
                q_in_m3_yr: q_total,
                q_out_m3_yr: excess_q[u],
            });
        } else if is_humid {
            // Humid zones (P/PET >= 0.5) never form permanent EndorheicLake salt pans.
            filled_to_sill[u] = true;
            let seasonal_out = (q_total * 0.15).max(100.0);
            excess_q[u] = seasonal_out;
            pending.push(PendingLake {
                node_idx: u as u32,
                water_level_m: spill,
                is_endorheic: false,
                q_in_m3_yr: q_total,
                q_out_m3_yr: seasonal_out,
            });
        } else {
            // Rain-shadow closed basin equilibrates at A(h_lake) * PET = Q_in.
            q_total = q_total.max(0.0);
            scratch_elevs.clear();
            scratch_elevs.extend(scratch_cells.iter().map(|&c| elev_m[c as usize]));
            let h_eq = solve_equilibrium_level(
                &scratch_elevs,
                nodes[u].pit_elev_m,
                spill,
                q_total,
                mean_pet,
                cell_area_m2,
            );
            pending.push(PendingLake {
                node_idx: u as u32,
                water_level_m: h_eq,
                is_endorheic: true,
                q_in_m3_yr: q_total,
                q_out_m3_yr: 0.0,
            });
        }
    }

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
                spill_traces.push((outlet_cell, (cand.q_out_m3_yr / SECONDS_PER_YEAR) as f32));
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
    }

    // 6. Micro-pit cleanup outside retained lakes (only when Fill-Spill-Merge is enabled).
    if cfg.enable_fill_spill_merge {
        for i in 0..n {
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

        // Carve plunge pool at `base_cell`.
        let plunge_depth_m = (0.15 * cand.drop_m).clamp(1.5, 8.0);
        let pool_rad = ((1.5 * input.river_w_m[lip_u] / cfg.dx_m).round() as isize).clamp(1, 2);
        let pool_surface = elev_m[base_u] + 0.35;
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
                let cut = plunge_depth_m * t;
                let sed_cut = cut.min(sediment_s[nb]);
                sediment_s[nb] -= sed_cut;
                elev_m[nb] -= cut;
                water_h_m[nb] = water_h_m[nb].max(pool_surface);
                water_kind[nb] = WaterKind::Waterfall as u8;
            }
        }
        for &pc in &cand.path_cells {
            let pu = pc as usize;
            water_kind[pu] = WaterKind::Waterfall as u8;
            water_h_m[pu] = water_h_m[pu].max(elev_m[pu] + 0.25);
        }

        waterfalls.push(WaterfallRecord {
            lip_cell: cand.lip_cell,
            base_cell: cand.base_cell,
            x_m: lx,
            z_m: lz,
            lip_elev_m: elev_m[lip_u],
            drop_m: cand.drop_m + plunge_depth_m - notch,
            yaw_rad,
            discharge_q_m3_s: cand.q_m3_s,
            plunge_depth_m,
        });
    }

    let land_denom = land_cells.max(1) as f32;
    Ok(LakesFallsField {
        width: w,
        height: h,
        lake_id,
        water_h_m: water_h_m.to_vec(),
        water_kind: water_kind.to_vec(),
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
}
