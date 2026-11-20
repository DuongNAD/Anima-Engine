//! Stage S2: Coupled Stream-Power Fluvial Erosion–Deposition (`Fastscape-SD`, `Yuan et al. 2019 [44]`,
//! `Braun 2022 [49]`, `Braun & Willett 2013 [5]`).
//!
//! Uses **precipitation-weighted** upstream area $\tilde{A}_i = \int \nu\,dA'$ ($\nu = P / \nu_0 > 0$)
//! so channel heads and arid rain-shadow wadis keep finite $\beta_i = G\Delta x^2 / \tilde{A}_i$.
//! Runoff `max(0, P - PET)` belongs solely to Stage S5 discharge `Q_ma`, never to $\tilde{A}_i$.

use crate::hydrology::{priority_flood_d8, NO_RECEIVER};

/// Determines upstream drainage area weighting $\tilde{A}$ inside Fastscape-SD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaWeightMode {
    /// Physical precipitation weighting $\nu_i = \max(P_i, P_{\min}) / \nu_0 > 0$ (`Yuan 2019`, `Braun 2022`).
    Precipitation,
    /// Negative control only: unclamped runoff `max(0, P - PET) / nu_0`, which vanishes (`0` -> `NaN`) in arid zones.
    UnclampedRunoffNegativeControl,
}

/// Solver variant for $G > 0$ deposition coupling (`§2.2.2` & `WV2-1b` negative controls).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FluvialSolverMode {
    /// Full Gauss–Seidel fixed-point iteration with node $i$'s own implicit local term (`beta_i`).
    CoupledGaussSeidel,
    /// Negative control: omits node $i$'s own implicit local deposition term (`beta_i`).
    NoLocalTermNegativeControl,
    /// Negative control: performs only 1 pass with `Phi_in = 0` instead of fixed-point iteration.
    SinglePassNoIterationNegativeControl,
}

/// Parameters controlling a Fastscape-SD step (`§2.2.2`).
#[derive(Debug, Clone, PartialEq)]
pub struct FluvialParams {
    pub dx_m: f64,
    pub dt_yr: f64,
    pub m: f64,
    pub n: f64,
    /// Braun (2022) `[49]` eq. 8 dimensionless deposition number $G = d^* v_s / \nu_0$.
    pub g_deposition: f64,
    pub nu_ref_m_yr: f64,
    pub min_precip_m_yr: f64,
    pub max_gs_iters: usize,
    pub gs_tol_m: f64,
    pub area_weight_mode: AreaWeightMode,
    pub solver_mode: FluvialSolverMode,
}

impl Default for FluvialParams {
    fn default() -> Self {
        Self {
            dx_m: 10.0,
            dt_yr: 1_000.0,
            m: 0.45,
            n: 1.0,
            g_deposition: 0.85,
            nu_ref_m_yr: 1.0,
            min_precip_m_yr: 0.15,
            max_gs_iters: 32,
            gs_tol_m: 1e-6,
            area_weight_mode: AreaWeightMode::Precipitation,
            solver_mode: FluvialSolverMode::CoupledGaussSeidel,
        }
    }
}

/// Summary diagnostics from one fluvial step.
#[derive(Debug, Clone, PartialEq)]
pub struct FluvialStepStats {
    pub gs_iterations: usize,
    pub max_gs_delta_m: f64,
    pub max_beta: f64,
    pub max_alpha: f64,
    pub non_finite_count: usize,
}

/// Precomputed D8 DAG inputs for [`step_fastscape_sd_dag`] and [`step_explicit_forward_euler_dag`].
pub struct FluvialDagView<'a> {
    pub width: usize,
    pub height: usize,
    pub d8_receiver: &'a [u32],
    pub receiver_dist_factor: &'a [f32],
    pub order_downstream_to_upstream: &'a [u32],
    pub fixed_outlet: &'a [bool],
    pub uplift_m_yr: &'a [f64],
    pub k_erodibility: &'a [f64],
    pub precip_m_yr: &'a [f64],
    pub pet_m_yr: &'a [f64],
}

fn compute_weighted_area(dag: &FluvialDagView<'_>, params: &FluvialParams) -> Vec<f64> {
    let n = dag.width * dag.height;
    let dx2 = params.dx_m * params.dx_m;
    let nu_ref = params.nu_ref_m_yr.max(1e-12);
    let mut area_tilde = vec![0.0_f64; n];

    for (i, slot) in area_tilde.iter_mut().enumerate().take(n) {
        let nu = match params.area_weight_mode {
            AreaWeightMode::Precipitation => {
                dag.precip_m_yr[i].max(params.min_precip_m_yr) / nu_ref
            }
            AreaWeightMode::UnclampedRunoffNegativeControl => {
                (dag.precip_m_yr[i] - dag.pet_m_yr[i]).max(0.0) / nu_ref
            }
        };
        *slot = nu * dx2;
    }

    // Accumulate strictly upstream -> downstream using reverse order.
    for &node_u in dag.order_downstream_to_upstream.iter().rev() {
        let i = node_u as usize;
        let rec = dag.d8_receiver[i];
        if rec != NO_RECEIVER {
            area_tilde[rec as usize] += area_tilde[i];
        }
    }
    area_tilde
}

/// Advances elevations `h` by one coupled Fastscape-SD step on a fixed D8 DAG (`§2.2.2`).
#[must_use]
pub fn step_fastscape_sd_dag(
    h: &mut [f64],
    dag: &FluvialDagView<'_>,
    params: &FluvialParams,
) -> FluvialStepStats {
    let n = dag.width * dag.height;
    assert_eq!(h.len(), n);
    let dx2 = params.dx_m * params.dx_m;
    let area_tilde = compute_weighted_area(dag, params);

    let mut alpha = vec![0.0_f64; n];
    let mut beta = vec![0.0_f64; n];
    let mut max_beta = 0.0_f64;
    let mut max_alpha = 0.0_f64;

    for i in 0..n {
        if dag.fixed_outlet[i] || dag.d8_receiver[i] == NO_RECEIVER {
            continue;
        }
        let d_i = params.dx_m * f64::from(dag.receiver_dist_factor[i]);
        let a_tilde = area_tilde[i];
        let a_m = if a_tilde > 0.0 {
            libm::pow(a_tilde, params.m)
        } else {
            0.0
        };
        let al = (dag.k_erodibility[i] * a_m * params.dt_yr) / d_i;
        let be = if params.g_deposition > 0.0 {
            params.g_deposition * dx2 / a_tilde
        } else {
            0.0
        };
        alpha[i] = al;
        beta[i] = be;
        if al.is_finite() && al > max_alpha {
            max_alpha = al;
        }
        if be.is_finite() && be > max_beta {
            max_beta = be;
        }
    }

    // Fast G = 0 path: single exact downstream-to-upstream implicit pass.
    if params.g_deposition == 0.0 {
        for &node_u in dag.order_downstream_to_upstream {
            let i = node_u as usize;
            let rec = dag.d8_receiver[i];
            if dag.fixed_outlet[i] || rec == NO_RECEIVER {
                continue;
            }
            h[i] = (h[i] + params.dt_yr * dag.uplift_m_yr[i] + alpha[i] * h[rec as usize])
                / (1.0 + alpha[i]);
        }
        let non_finite_count = h.iter().filter(|v| !v.is_finite()).count();
        return FluvialStepStats {
            gs_iterations: 1,
            max_gs_delta_m: 0.0,
            max_beta,
            max_alpha,
            non_finite_count,
        };
    }

    let h_t = h.to_vec();
    let mut phi_in = vec![0.0_f64; n];
    let mut h_next = h_t.clone();

    let n_iters = match params.solver_mode {
        FluvialSolverMode::SinglePassNoIterationNegativeControl => 1,
        FluvialSolverMode::CoupledGaussSeidel | FluvialSolverMode::NoLocalTermNegativeControl => {
            params.max_gs_iters.max(1)
        }
    };

    let mut gs_iterations = 0usize;
    let mut max_gs_delta_m = 0.0_f64;

    for iter in 0..n_iters {
        gs_iterations = iter + 1;
        phi_in.fill(0.0);

        if params.solver_mode != FluvialSolverMode::SinglePassNoIterationNegativeControl {
            // 1. Upstream-to-downstream flux sweep accumulating Φ_in^(r)(i).
            for &node_u in dag.order_downstream_to_upstream.iter().rev() {
                let k = node_u as usize;
                let rec = dag.d8_receiver[k];
                if rec == NO_RECEIVER {
                    continue;
                }
                let net_vol = if dag.fixed_outlet[k] {
                    0.0
                } else {
                    (h_t[k] + params.dt_yr * dag.uplift_m_yr[k] - h_next[k]) * dx2
                };
                phi_in[rec as usize] += net_vol + phi_in[k];
            }
        }

        // 2. Downstream-to-upstream implicit update with node i's local term folded cleanly.
        max_gs_delta_m = 0.0;
        for &node_u in dag.order_downstream_to_upstream {
            let i = node_u as usize;
            let rec = dag.d8_receiver[i];
            if dag.fixed_outlet[i] || rec == NO_RECEIVER {
                continue;
            }
            let r_idx = rec as usize;
            let target_free = h_t[i] + params.dt_yr * dag.uplift_m_yr[i];
            let dep_up = if params.g_deposition > 0.0 {
                (params.g_deposition / area_tilde[i]) * phi_in[i]
            } else {
                0.0
            };

            let updated = match params.solver_mode {
                FluvialSolverMode::CoupledGaussSeidel
                | FluvialSolverMode::SinglePassNoIterationNegativeControl => {
                    (target_free * (1.0 + beta[i]) + alpha[i] * h_next[r_idx] + dep_up)
                        / (1.0 + alpha[i] + beta[i])
                }
                FluvialSolverMode::NoLocalTermNegativeControl => {
                    (target_free + alpha[i] * h_next[r_idx] + dep_up) / (1.0 + alpha[i])
                }
            };

            let diff = libm::fabs(updated - h_next[i]);
            if diff > max_gs_delta_m || !diff.is_finite() {
                max_gs_delta_m = diff;
            }
            h_next[i] = updated;
        }

        if !max_gs_delta_m.is_finite() {
            break;
        }
        if iter > 0 && max_gs_delta_m < params.gs_tol_m {
            break;
        }
    }

    h.copy_from_slice(&h_next);
    let non_finite_count = h.iter().filter(|v| !v.is_finite()).count();
    FluvialStepStats {
        gs_iterations,
        max_gs_delta_m,
        max_beta,
        max_alpha,
        non_finite_count,
    }
}

/// Explicit forward-Euler reference integrator for the coupled $G \ge 0$ Fastscape-SD PDE (`§2.2.2`).
///
/// Solving $E_i = \bigl(K_i \tilde{A}_i^m S_i^t + (G/\tilde{A}_i)\Phi_{E,\text{in}}(i)\bigr)/(1 + \beta_i)$
/// upstream-to-downstream (where $\Phi_{E,\text{in}}(i) = \sum_{k \in \text{up}(i)} [E_k \Delta x^2 + \Phi_{E,\text{in}}(k)]$
/// since $\int (U - \partial_t h)\,dA' = \int E\,dA'$) gives the exact explicit $\Delta t \to 0$ PDE rate!
pub fn step_explicit_forward_euler_dag(
    h: &mut [f64],
    dag: &FluvialDagView<'_>,
    params: &FluvialParams,
) {
    let n = dag.width * dag.height;
    assert_eq!(h.len(), n);
    let dx2 = params.dx_m * params.dx_m;
    let area_tilde = compute_weighted_area(dag, params);
    let mut phi_e_in = vec![0.0_f64; n];
    let mut erosion_rate = vec![0.0_f64; n];

    for &node_u in dag.order_downstream_to_upstream.iter().rev() {
        let i = node_u as usize;
        let rec = dag.d8_receiver[i];
        if dag.fixed_outlet[i] || rec == NO_RECEIVER {
            continue;
        }
        let d_i = params.dx_m * f64::from(dag.receiver_dist_factor[i]);
        let slope = ((h[i] - h[rec as usize]) / d_i).max(0.0);
        let inc =
            dag.k_erodibility[i] * libm::pow(area_tilde[i], params.m) * libm::pow(slope, params.n);
        let be = params.g_deposition * dx2 / area_tilde[i];
        let dep_in = if params.g_deposition > 0.0 {
            (params.g_deposition / area_tilde[i]) * phi_e_in[i]
        } else {
            0.0
        };
        let e_i = (inc - dep_in) / (1.0 + be);
        erosion_rate[i] = e_i;
        phi_e_in[rec as usize] += e_i * dx2 + phi_e_in[i];
    }

    for i in 0..n {
        if !dag.fixed_outlet[i] && dag.d8_receiver[i] != NO_RECEIVER {
            h[i] += params.dt_yr * (dag.uplift_m_yr[i] - erosion_rate[i]);
        }
    }
}

/// Mutable elevation/sediment buffers and static forcings for [`erode_fluvial_step_f32`].
pub struct FluvialGridStep<'a> {
    pub bedrock_h: &'a mut [f32],
    pub sediment_s: &'a mut [f32],
    pub uplift_m_yr: &'a [f32],
    pub k_erodibility: &'a [f32],
    pub precip_m_yr: &'a [f32],
    pub pet_m_yr: &'a [f32],
    pub width: usize,
    pub height: usize,
    pub sea_level_m: Option<f32>,
}

/// High-level `f32` wrapper running Priority-Flood + one Fastscape-SD step on gridded Stage S1/S2 fields.
#[must_use]
pub fn erode_fluvial_step_f32(
    grid: &mut FluvialGridStep<'_>,
    params: &FluvialParams,
) -> FluvialStepStats {
    let width = grid.width;
    let height = grid.height;
    let n = width * height;
    assert_eq!(grid.bedrock_h.len(), n);
    assert_eq!(grid.sediment_s.len(), n);

    let mut total_h: Vec<f32> = grid
        .bedrock_h
        .iter()
        .zip(grid.sediment_s.iter())
        .map(|(&b, &s)| b + s)
        .collect();
    let pf = priority_flood_d8(&total_h, width, height, grid.sea_level_m);

    let fixed_outlet: Vec<bool> = pf
        .d8_receiver
        .iter()
        .enumerate()
        .map(|(i, &rec)| {
            rec == NO_RECEIVER || grid.sea_level_m.is_some_and(|sl| pf.filled_h[i] <= sl)
        })
        .collect();

    let uplift_f64: Vec<f64> = grid.uplift_m_yr.iter().map(|&v| f64::from(v)).collect();
    let k_f64: Vec<f64> = grid.k_erodibility.iter().map(|&v| f64::from(v)).collect();
    let p_f64: Vec<f64> = grid.precip_m_yr.iter().map(|&v| f64::from(v)).collect();
    let e_f64: Vec<f64> = grid.pet_m_yr.iter().map(|&v| f64::from(v)).collect();
    let mut h_f64: Vec<f64> = pf.filled_h.iter().map(|&v| f64::from(v)).collect();

    let dag = FluvialDagView {
        width,
        height,
        d8_receiver: &pf.d8_receiver,
        receiver_dist_factor: &pf.receiver_dist_factor,
        order_downstream_to_upstream: &pf.order_downstream_to_upstream,
        fixed_outlet: &fixed_outlet,
        uplift_m_yr: &uplift_f64,
        k_erodibility: &k_f64,
        precip_m_yr: &p_f64,
        pet_m_yr: &e_f64,
    };

    let stats = step_fastscape_sd_dag(&mut h_f64, &dag, params);
    for i in 0..n {
        if h_f64[i].is_finite() {
            let new_total = h_f64[i] as f32;
            let dh = new_total - pf.filled_h[i];
            if dh >= 0.0 {
                grid.sediment_s[i] += dh;
            } else {
                let inc = -dh;
                if grid.sediment_s[i] >= inc {
                    grid.sediment_s[i] -= inc;
                } else {
                    let rem = inc - grid.sediment_s[i];
                    grid.sediment_s[i] = 0.0;
                    grid.bedrock_h[i] -= rem;
                }
            }
            total_h[i] = grid.bedrock_h[i] + grid.sediment_s[i];
        }
    }
    stats
}
