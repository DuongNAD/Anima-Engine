//! Stage S5: Exact $O(N \log N)$ Priority-Flood depression filling (`Barnes et al. 2014 [1]`),
//! iterative D8 topological ordering (`Braun & Willett 2013 [5]`), and Hybrid MFD->D8 flow routing
//! with precipitation-vs-runoff decoupling (`Leopold & Maddock 1953 [23]`).

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

/// Sentinel receiver index marking boundary or marine outlets.
pub const NO_RECEIVER: u32 = u32::MAX;

/// Seconds per Julian year used to convert `m^3/yr` catchment runoff to `m^3/s`.
pub const SECONDS_PER_YEAR: f64 = 31_557_600.0;

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

/// Order-preserving bijection `f32 -> u32` for deterministic min-heap keying.
#[inline]
fn float_ord_key(x: f32) -> u32 {
    let bits = x.to_bits();
    if (bits >> 31) != 0 {
        !bits
    } else {
        bits | 0x8000_0000
    }
}

/// Counts interior non-outlet cells with no strictly lower 8-neighbor (`filled_h[nb] < filled_h[i]`).
#[must_use]
pub fn count_pits(elev: &[f32], width: usize, height: usize, sea_level_m: Option<f32>) -> usize {
    assert_eq!(elev.len(), width * height);
    if width < 3 || height < 3 {
        return 0;
    }
    let mut pits = 0usize;
    for r in 1..(height - 1) {
        let row_base = r * width;
        for c in 1..(width - 1) {
            let idx = row_base + c;
            let h = elev[idx];
            if sea_level_m.is_some_and(|sl| h <= sl) {
                continue;
            }
            let mut has_lower = false;
            for dir in 0..8 {
                let nr = (r as isize + DZ8[dir]) as usize;
                let nc = (c as isize + DX8[dir]) as usize;
                if elev[nr * width + nc] < h {
                    has_lower = true;
                    break;
                }
            }
            if !has_lower {
                pits += 1;
            }
        }
    }
    pits
}

/// Result of Priority-Flood depression filling plus steepest-descent D8 tree & CSR donor graph.
#[derive(Debug, Clone)]
pub struct PriorityFloodResult {
    pub width: usize,
    pub height: usize,
    /// Pit-free elevation field (`count_pits(&filled_h, ..) == 0`).
    pub filled_h: Vec<f32>,
    /// Downstream receiver index per cell (`NO_RECEIVER` on border/marine outlets).
    pub d8_receiver: Vec<u32>,
    /// Flow step distance factor (`1.0` cardinal, `sqrt(2)` diagonal, `1.0` for outlets).
    pub receiver_dist_factor: Vec<f32>,
    /// CSR offsets into `donors` (length `n + 1`).
    pub donor_offsets: Vec<u32>,
    /// Packed upstream donor cell indices per receiver.
    pub donors: Vec<u32>,
    /// Deterministic downstream-to-upstream topological order (every outlet before its donors).
    pub order_downstream_to_upstream: Vec<u32>,
    pub pit_count_before: usize,
    pub pit_count_after: usize,
}

/// Executes exact $O(N \log N)$ Priority-Flood (`Barnes et al. 2014`) + iterative D8 DAG construction.
#[must_use]
pub fn priority_flood_d8(
    elev: &[f32],
    width: usize,
    height: usize,
    sea_level_m: Option<f32>,
) -> PriorityFloodResult {
    let n = width * height;
    assert_eq!(elev.len(), n);
    let pit_count_before = count_pits(elev, width, height, sea_level_m);

    let mut filled_h = elev.to_vec();
    let mut closed = vec![false; n];
    let mut is_outlet = vec![false; n];
    let mut open: BinaryHeap<Reverse<(u32, usize)>> =
        BinaryHeap::with_capacity(width * 2 + height * 2);
    let mut pit_queue: VecDeque<(usize, f32)> = VecDeque::new();

    for r in 0..height {
        for c in 0..width {
            let idx = r * width + c;
            let border = r == 0 || r + 1 == height || c == 0 || c + 1 == width;
            let marine = sea_level_m.is_some_and(|sl| filled_h[idx] <= sl);
            if border || marine {
                closed[idx] = true;
                is_outlet[idx] = true;
                open.push(Reverse((float_ord_key(filled_h[idx]), idx)));
            }
        }
    }

    while !open.is_empty() || !pit_queue.is_empty() {
        let (cell, current_h) = if let Some((c, h_c)) = pit_queue.pop_front() {
            (c, h_c)
        } else {
            let Reverse((_, c)) = open.pop().expect("open heap non-empty");
            (c, filled_h[c])
        };

        let r = cell / width;
        let c = cell % width;
        for dir in 0..8 {
            let nr = r as isize + DZ8[dir];
            let nc = c as isize + DX8[dir];
            if nr < 0 || nc < 0 || nr >= height as isize || nc >= width as isize {
                continue;
            }
            let nb = nr as usize * width + nc as usize;
            if closed[nb] {
                continue;
            }
            closed[nb] = true;
            if filled_h[nb] <= current_h {
                let raised = current_h.next_up();
                filled_h[nb] = raised;
                pit_queue.push_back((nb, raised));
            } else {
                open.push(Reverse((float_ord_key(filled_h[nb]), nb)));
            }
        }
    }

    let pit_count_after = count_pits(&filled_h, width, height, sea_level_m);

    // Assign steepest-descent D8 receiver from `filled_h` (guaranteed strict decrease for all non-outlets).
    let mut d8_receiver = vec![NO_RECEIVER; n];
    let mut receiver_dist_factor = vec![1.0_f32; n];
    let mut donor_counts = vec![0_u32; n];

    for r in 0..height {
        for c in 0..width {
            let idx = r * width + c;
            if is_outlet[idx] {
                continue;
            }
            let h_i = filled_h[idx] as f64;
            let mut best_slope = 0.0_f64;
            let mut best_nb = NO_RECEIVER;
            let mut best_df = 1.0_f32;
            for dir in 0..8 {
                let nr = r as isize + DZ8[dir];
                let nc = c as isize + DX8[dir];
                if nr < 0 || nc < 0 || nr >= height as isize || nc >= width as isize {
                    continue;
                }
                let nb = nr as usize * width + nc as usize;
                let dh = h_i - filled_h[nb] as f64;
                if dh > 0.0 {
                    let slope = dh / DIST8[dir];
                    if slope > best_slope {
                        best_slope = slope;
                        best_nb = nb as u32;
                        best_df = DIST8[dir] as f32;
                    }
                }
            }
            d8_receiver[idx] = best_nb;
            receiver_dist_factor[idx] = best_df;
            if best_nb != NO_RECEIVER {
                donor_counts[best_nb as usize] += 1;
            }
        }
    }

    let mut donor_offsets = vec![0_u32; n + 1];
    for i in 0..n {
        donor_offsets[i + 1] = donor_offsets[i] + donor_counts[i];
    }
    let mut cursor = donor_offsets[..n].to_vec();
    let mut donors = vec![0_u32; donor_offsets[n] as usize];
    for (idx, &rec) in d8_receiver.iter().enumerate() {
        if rec != NO_RECEIVER {
            let pos = cursor[rec as usize] as usize;
            donors[pos] = idx as u32;
            cursor[rec as usize] += 1;
        }
    }

    // Iterative BFS/stack walk from outlets yields deterministic downstream-to-upstream order.
    let mut order_downstream_to_upstream = Vec::with_capacity(n);
    for (idx, &rec) in d8_receiver.iter().enumerate() {
        if rec == NO_RECEIVER {
            order_downstream_to_upstream.push(idx as u32);
        }
    }
    let mut head = 0;
    while head < order_downstream_to_upstream.len() {
        let u = order_downstream_to_upstream[head] as usize;
        head += 1;
        let start = donor_offsets[u] as usize;
        let end = donor_offsets[u + 1] as usize;
        for &d in &donors[start..end] {
            order_downstream_to_upstream.push(d);
        }
    }
    debug_assert_eq!(order_downstream_to_upstream.len(), n);

    PriorityFloodResult {
        width,
        height,
        filled_h,
        d8_receiver,
        receiver_dist_factor,
        donor_offsets,
        donors,
        order_downstream_to_upstream,
        pit_count_before,
        pit_count_after,
    }
}

/// Configuration for Stage S5 Hybrid MFD->D8 routing and hydraulic geometry (`§2.2.5`).
#[derive(Debug, Clone, PartialEq)]
pub struct HydrologyConfig {
    /// Cell spacing in metres (`10.0` at `4096^2`).
    pub dx_m: f32,
    /// MFD slope-partition exponent (`p = 1.6` design tuning intermediate between `1` and `4..6`).
    pub mfd_exponent_p: f64,
    /// Channel initiation threshold on `A * S^2` (`m^2`) transitioning MFD hillslopes -> D8 tree.
    pub channel_init_threshold: f64,
    /// Negative-control toggle forcing pure D8 routing everywhere (including hillslopes).
    pub force_pure_d8: bool,
    /// Reference precipitation rate `nu_0` (`1.0 m/yr`).
    pub reference_precip_m_yr: f32,
    /// Strictly positive rain-shadow precipitation floor (`0.15 m/yr`) keeping `nu > 0`.
    pub min_precip_m_yr: f32,
    /// Sea-level cutoff (cells `<= sea_level_m` do not export freshwater channels into marine cells).
    pub sea_level_m: Option<f32>,
    /// Leopold & Maddock width coefficient `k_w` (`w = k_w * Q^b_w`).
    pub k_w: f32,
    /// Leopold & Maddock width exponent `b_w` (`0.50`).
    pub b_w: f32,
    /// Leopold & Maddock depth coefficient `k_d` (`d = k_d * Q^f_d`).
    pub k_d: f32,
    /// Leopold & Maddock depth exponent `f_d` (`0.40`).
    pub f_d: f32,
}

impl Default for HydrologyConfig {
    fn default() -> Self {
        Self {
            dx_m: 10.0,
            mfd_exponent_p: 1.6,
            channel_init_threshold: 250.0,
            force_pure_d8: false,
            reference_precip_m_yr: 1.0,
            min_precip_m_yr: 0.15,
            sea_level_m: Some(0.0),
            k_w: 3.0,
            b_w: 0.50,
            k_d: 0.25,
            f_d: 0.40,
        }
    }
}

/// Routed hydrology & channel geometry outputs from [`route_hybrid_mfd_d8`].
#[derive(Debug, Clone)]
pub struct HydrologyField {
    pub width: usize,
    pub height: usize,
    /// Geometric contributing drainage area in `m^2`.
    pub flow_acc_m2: Vec<f64>,
    /// Precipitation-weighted drainage area `tilde_A` (`m^2`), always `>= nu_min * dx^2 > 0`.
    pub precip_weighted_area_m2: Vec<f64>,
    /// Mean annual discharge `Q_ma` (`m^3/s`) routed strictly from `max(0, P - PET)`.
    pub discharge_q_m3_s: Vec<f32>,
    pub flow_dir_x: Vec<f32>,
    pub flow_dir_z: Vec<f32>,
    pub river_w_m: Vec<f32>,
    pub river_d_m: Vec<f32>,
    pub water_h_m: Vec<f32>,
    pub is_channel: Vec<bool>,
}

/// Routes drainage area, precipitation-weighted area, and net runoff over a pit-free DEM.
///
/// Uses Kahn topological sorting over the strict downhill MFD DAG (seeded by ascending cell index),
/// ensuring deterministic accumulation independent of thread scheduling.
#[must_use]
pub fn route_hybrid_mfd_d8(
    filled_h: &[f32],
    d8_receiver: &[u32],
    width: usize,
    height: usize,
    precip_m_yr: &[f32],
    pet_m_yr: &[f32],
    cfg: &HydrologyConfig,
) -> HydrologyField {
    let n = width * height;
    assert_eq!(filled_h.len(), n);
    assert_eq!(d8_receiver.len(), n);
    assert_eq!(precip_m_yr.len(), n);
    assert_eq!(pet_m_yr.len(), n);

    let cell_area = f64::from(cfg.dx_m) * f64::from(cfg.dx_m);
    let nu_ref = f64::from(cfg.reference_precip_m_yr.max(1e-6));
    let mut area_m2 = vec![cell_area; n];
    let mut weighted_area_m2 = Vec::with_capacity(n);
    let mut runoff_m3_yr = Vec::with_capacity(n);

    for i in 0..n {
        let p = f64::from(precip_m_yr[i].max(cfg.min_precip_m_yr));
        weighted_area_m2.push((p / nu_ref) * cell_area);
        // Net runoff rate `R = max(0, P - PET)` in `m/yr`; cell volume `q_cell = R * dx^2`.
        let r_m_yr = f64::from((precip_m_yr[i] - pet_m_yr[i]).max(0.0));
        runoff_m3_yr.push(r_m_yr * cell_area);
    }

    // Build in-degree counts over the strict downhill DAG (`filled_h[nb] < filled_h[i]`).
    let mut in_deg = vec![0_u8; n];
    for r in 0..height {
        for c in 0..width {
            let idx = r * width + c;
            if cfg.sea_level_m.is_some_and(|sl| filled_h[idx] <= sl)
                || d8_receiver[idx] == NO_RECEIVER
            {
                continue;
            }
            let h_i = filled_h[idx];
            if cfg.force_pure_d8 {
                let rec = d8_receiver[idx] as usize;
                in_deg[rec] = in_deg[rec].saturating_add(1);
            } else {
                for dir in 0..8 {
                    let nr = r as isize + DZ8[dir];
                    let nc = c as isize + DX8[dir];
                    if nr < 0 || nc < 0 || nr >= height as isize || nc >= width as isize {
                        continue;
                    }
                    let nb = nr as usize * width + nc as usize;
                    if filled_h[nb] < h_i {
                        in_deg[nb] = in_deg[nb].saturating_add(1);
                    }
                }
            }
        }
    }

    let mut queue = VecDeque::with_capacity(n / 4);
    for (idx, &deg) in in_deg.iter().enumerate() {
        if deg == 0 {
            queue.push_back(idx);
        }
    }

    let mut flow_dir_x = vec![0.0_f32; n];
    let mut flow_dir_z = vec![0.0_f32; n];
    let mut is_channel = vec![false; n];

    while let Some(idx) = queue.pop_front() {
        let r = idx / width;
        let c = idx % width;
        let h_i = filled_h[idx] as f64;
        let rec = d8_receiver[idx];
        if rec == NO_RECEIVER || cfg.sea_level_m.is_some_and(|sl| filled_h[idx] <= sl) {
            continue;
        }

        // Compute steepest downhill slope and MFD neighbor weights.
        let mut weights = [0.0_f64; 8];
        let mut nbs = [0_usize; 8];
        let mut down_count = 0usize;
        let mut max_slope = 0.0_f64;
        let mut best_dir = 0usize;

        for dir in 0..8 {
            let nr = r as isize + DZ8[dir];
            let nc = c as isize + DX8[dir];
            if nr < 0 || nc < 0 || nr >= height as isize || nc >= width as isize {
                continue;
            }
            let nb = nr as usize * width + nc as usize;
            if filled_h[nb] < filled_h[idx] {
                let s = (h_i - filled_h[nb] as f64) / (f64::from(cfg.dx_m) * DIST8[dir]);
                if s > max_slope {
                    max_slope = s;
                    best_dir = dir;
                }
                if !cfg.force_pure_d8 {
                    let w = libm::pow(s.max(0.0), cfg.mfd_exponent_p);
                    weights[down_count] = w;
                    nbs[down_count] = nb;
                    down_count += 1;
                }
            }
        }

        let init_criterion = area_m2[idx] * max_slope * max_slope;
        let channelized = cfg.force_pure_d8 || init_criterion >= cfg.channel_init_threshold;
        is_channel[idx] = channelized;

        let a_here = area_m2[idx];
        let wa_here = weighted_area_m2[idx];
        let r_here = runoff_m3_yr[idx];

        if cfg.force_pure_d8 {
            let rec_u = rec as usize;
            area_m2[rec_u] += a_here;
            weighted_area_m2[rec_u] += wa_here;
            runoff_m3_yr[rec_u] += r_here;
            flow_dir_x[idx] = UX8[best_dir];
            flow_dir_z[idx] = UZ8[best_dir];
            in_deg[rec_u] = in_deg[rec_u].saturating_sub(1);
            if in_deg[rec_u] == 0 {
                queue.push_back(rec_u);
            }
        } else {
            let w_sum: f64 = weights[..down_count].iter().sum();
            let rec_u = rec as usize;
            if channelized || w_sum <= 0.0 {
                area_m2[rec_u] += a_here;
                weighted_area_m2[rec_u] += wa_here;
                runoff_m3_yr[rec_u] += r_here;
                flow_dir_x[idx] = UX8[best_dir];
                flow_dir_z[idx] = UZ8[best_dir];
            } else {
                let mut vx = 0.0_f64;
                let mut vz = 0.0_f64;
                for k in 0..down_count {
                    let frac = weights[k] / w_sum;
                    let nb = nbs[k];
                    area_m2[nb] += a_here * frac;
                    weighted_area_m2[nb] += wa_here * frac;
                    runoff_m3_yr[nb] += r_here * frac;
                    // Match `nr - r` and `nc - c` normalized unit flow direction.
                    let dr = (nb / width) as f64 - r as f64;
                    let dc = (nb % width) as f64 - c as f64;
                    let len = (dc * dc + dr * dr).sqrt();
                    vx += frac * (dc / len);
                    vz += frac * (dr / len);
                }
                let vlen = (vx * vx + vz * vz).sqrt();
                if vlen > 1e-12 {
                    flow_dir_x[idx] = (vx / vlen) as f32;
                    flow_dir_z[idx] = (vz / vlen) as f32;
                }
            }
            for &nb in &nbs[..down_count] {
                in_deg[nb] = in_deg[nb].saturating_sub(1);
                if in_deg[nb] == 0 {
                    queue.push_back(nb);
                }
            }
        }
    }

    let mut discharge_q_m3_s = vec![0.0_f32; n];
    let mut river_w_m = vec![0.0_f32; n];
    let mut river_d_m = vec![0.0_f32; n];
    let mut water_h_m = filled_h.to_vec();

    for i in 0..n {
        let q = (runoff_m3_yr[i] / SECONDS_PER_YEAR) as f32;
        discharge_q_m3_s[i] = q;
        if is_channel[i] && q > 0.0 && cfg.sea_level_m.is_none_or(|sl| filled_h[i] > sl) {
            let w = cfg.k_w * libm::powf(q, cfg.b_w);
            let d = cfg.k_d * libm::powf(q, cfg.f_d);
            river_w_m[i] = w;
            river_d_m[i] = d;
            water_h_m[i] = filled_h[i] + d;
        }
    }

    HydrologyField {
        width,
        height,
        flow_acc_m2: area_m2,
        precip_weighted_area_m2: weighted_area_m2,
        discharge_q_m3_s,
        flow_dir_x,
        flow_dir_z,
        river_w_m,
        river_d_m,
        water_h_m,
        is_channel,
    }
}
