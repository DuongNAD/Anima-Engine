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

/// Hard upper bound on `FluvialParams.max_dt_halvings` (`2^12 = 4,096` sub-steps max), preventing
/// integer shift overflow (`>= 64`) and multi-hour retry loops.
pub const MAX_ALLOWED_DT_HALVINGS: u32 = 12;

/// Default total Gauss–Seidel sweep budget across Phase A and Phase B sub-steps in a single step.
pub const DEFAULT_MAX_TOTAL_SWEEPS: usize = 4_096;

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
    /// Maximum number of $\Delta t$ halving retries (`substeps = 2^halving`, capped at [`MAX_ALLOWED_DT_HALVINGS`]).
    pub max_dt_halvings: u32,
    /// Total sweep budget across Phase A and all Phase B retries before failing loudly.
    pub max_total_sweeps: usize,
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
            max_dt_halvings: 10,
            max_total_sweeps: DEFAULT_MAX_TOTAL_SWEEPS,
            area_weight_mode: AreaWeightMode::Precipitation,
            solver_mode: FluvialSolverMode::CoupledGaussSeidel,
        }
    }
}

/// Failure returned when Fastscape-SD parameters are invalid or Gauss–Seidel fails to converge
/// within `max_dt_halvings` / `max_total_sweeps`.
#[derive(Debug, Clone, PartialEq)]
pub enum FluvialError {
    InvalidSlopeExponent {
        n: f64,
    },
    InvalidMinPrecip {
        min_precip_m_yr: f64,
    },
    InvalidParameter {
        name: &'static str,
        value: f64,
    },
    NonConvergence {
        cell_index: usize,
        last_delta_m: f64,
        dt_yr: f64,
        substeps_attempted: usize,
        gs_iterations: usize,
        total_sweeps: usize,
        budget_exhausted: bool,
    },
}

impl std::fmt::Display for FluvialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSlopeExponent { n } => write!(
                f,
                "implicit Fastscape-SD solver only supports slope exponent n = 1.0 (got n = {n})"
            ),
            Self::InvalidMinPrecip { min_precip_m_yr } => write!(
                f,
                "min_precip_m_yr must be finite and > 0.0 under Precipitation weighting (got {min_precip_m_yr})"
            ),
            Self::InvalidParameter { name, value } => {
                write!(f, "invalid fluvial parameter `{name}` = {value}")
            }
            Self::NonConvergence {
                cell_index,
                last_delta_m,
                dt_yr,
                substeps_attempted,
                gs_iterations,
                total_sweeps,
                budget_exhausted,
            } => write!(
                f,
                "Fastscape-SD Gauss-Seidel did not converge at cell {cell_index}: last_delta_m={last_delta_m:e}, sub_dt_yr={dt_yr:e} after {substeps_attempted} substeps ({gs_iterations} GS iters, total_sweeps={total_sweeps}, budget_exhausted={budget_exhausted})"
            ),
        }
    }
}

impl std::error::Error for FluvialError {}

/// Summary diagnostics from one fluvial step.
#[derive(Debug, Clone, PartialEq)]
pub struct FluvialStepStats {
    pub converged: bool,
    pub used_fallback_order: bool,
    pub substeps: usize,
    pub gs_iterations: usize,
    pub total_gs_iterations: usize,
    pub max_gs_delta_m: f64,
    pub max_beta: f64,
    pub max_alpha: f64,
    pub non_finite_count: usize,
}

fn validate_implicit_params(params: &FluvialParams) -> Result<(), FluvialError> {
    if !params.n.is_finite() || (params.n - 1.0).abs() > 1e-12 {
        return Err(FluvialError::InvalidSlopeExponent { n: params.n });
    }
    if params.area_weight_mode == AreaWeightMode::Precipitation
        && (!params.min_precip_m_yr.is_finite() || params.min_precip_m_yr <= 0.0)
    {
        return Err(FluvialError::InvalidMinPrecip {
            min_precip_m_yr: params.min_precip_m_yr,
        });
    }
    if !params.dx_m.is_finite() || params.dx_m <= 0.0 {
        return Err(FluvialError::InvalidParameter {
            name: "dx_m",
            value: params.dx_m,
        });
    }
    if !params.dt_yr.is_finite() || params.dt_yr < 0.0 {
        return Err(FluvialError::InvalidParameter {
            name: "dt_yr",
            value: params.dt_yr,
        });
    }
    if !params.m.is_finite() || params.m < 0.0 {
        return Err(FluvialError::InvalidParameter {
            name: "m",
            value: params.m,
        });
    }
    if !params.g_deposition.is_finite() || params.g_deposition < 0.0 {
        return Err(FluvialError::InvalidParameter {
            name: "g_deposition",
            value: params.g_deposition,
        });
    }
    if !params.nu_ref_m_yr.is_finite() || params.nu_ref_m_yr <= 0.0 {
        return Err(FluvialError::InvalidParameter {
            name: "nu_ref_m_yr",
            value: params.nu_ref_m_yr,
        });
    }
    if !params.gs_tol_m.is_finite() || params.gs_tol_m <= 0.0 {
        return Err(FluvialError::InvalidParameter {
            name: "gs_tol_m",
            value: params.gs_tol_m,
        });
    }
    if params.max_gs_iters == 0 {
        return Err(FluvialError::InvalidParameter {
            name: "max_gs_iters",
            value: 0.0,
        });
    }
    if params.max_dt_halvings > MAX_ALLOWED_DT_HALVINGS {
        return Err(FluvialError::InvalidParameter {
            name: "max_dt_halvings",
            value: f64::from(params.max_dt_halvings),
        });
    }
    if params.max_total_sweeps == 0 {
        return Err(FluvialError::InvalidParameter {
            name: "max_total_sweeps",
            value: 0.0,
        });
    }
    Ok(())
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
///
/// Phase A attempts the fast receiver-first two-sweep Gauss–Seidel at full `dt_yr` (single pass,
/// preserving humid-regime throughput). Only if Phase A fails to converge within `max_gs_iters`
/// does Phase B fall back to in-place upstream-to-downstream Gauss–Seidel with adaptive `dt`
/// halving up to `max_dt_halvings`, bounded by `max_total_sweeps` across both phases.
pub fn step_fastscape_sd_dag(
    h: &mut [f64],
    dag: &FluvialDagView<'_>,
    params: &FluvialParams,
) -> Result<FluvialStepStats, FluvialError> {
    validate_implicit_params(params)?;
    let n = dag.width * dag.height;
    assert_eq!(h.len(), n);
    let dx2 = params.dx_m * params.dx_m;
    let area_tilde = compute_weighted_area(dag, params);

    let mut alpha_base = vec![0.0_f64; n];
    let mut beta = vec![0.0_f64; n];
    let mut max_beta = 0.0_f64;
    let mut max_alpha_base = 0.0_f64;

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
        alpha_base[i] = al;
        beta[i] = be;
        if al.is_finite() {
            if al > max_alpha_base {
                max_alpha_base = al;
            }
        } else {
            max_alpha_base = f64::INFINITY;
        }
        if be.is_finite() {
            if be > max_beta {
                max_beta = be;
            }
        } else {
            max_beta = f64::INFINITY;
        }
    }

    // Fast G = 0 path: single exact downstream-to-upstream implicit pass.
    if params.g_deposition == 0.0 {
        let mut h_next = h.to_vec();
        for &node_u in dag.order_downstream_to_upstream {
            let i = node_u as usize;
            let rec = dag.d8_receiver[i];
            if dag.fixed_outlet[i] || rec == NO_RECEIVER {
                continue;
            }
            h_next[i] = (h_next[i]
                + params.dt_yr * dag.uplift_m_yr[i]
                + alpha_base[i] * h_next[rec as usize])
                / (1.0 + alpha_base[i]);
        }
        if let Some((cell_index, &val)) = h_next.iter().enumerate().find(|(_, v)| !v.is_finite()) {
            return Err(FluvialError::NonConvergence {
                cell_index,
                last_delta_m: val,
                dt_yr: params.dt_yr,
                substeps_attempted: 1,
                gs_iterations: 1,
                total_sweeps: 1,
                budget_exhausted: false,
            });
        }
        h.copy_from_slice(&h_next);
        return Ok(FluvialStepStats {
            converged: true,
            used_fallback_order: false,
            substeps: 1,
            gs_iterations: 1,
            total_gs_iterations: 1,
            max_gs_delta_m: 0.0,
            max_beta,
            max_alpha: max_alpha_base,
            non_finite_count: 0,
        });
    }

    // Negative control: single pass with Phi_in = 0 returns unsubstepped single-pass output.
    if params.solver_mode == FluvialSolverMode::SinglePassNoIterationNegativeControl {
        let h_t = h.to_vec();
        let mut max_gs_delta_m = 0.0_f64;
        let mut h_next = h_t.clone();
        for &node_u in dag.order_downstream_to_upstream {
            let i = node_u as usize;
            let rec = dag.d8_receiver[i];
            if dag.fixed_outlet[i] || rec == NO_RECEIVER {
                continue;
            }
            let r_idx = rec as usize;
            let target_free = h_t[i] + params.dt_yr * dag.uplift_m_yr[i];
            let updated = (target_free * (1.0 + beta[i]) + alpha_base[i] * h_next[r_idx])
                / (1.0 + alpha_base[i] + beta[i]);
            let diff = libm::fabs(updated - h_next[i]);
            if diff > max_gs_delta_m || !diff.is_finite() {
                max_gs_delta_m = diff;
            }
            h_next[i] = updated;
        }
        h.copy_from_slice(&h_next);
        let non_finite_count = h.iter().filter(|v| !v.is_finite()).count();
        return Ok(FluvialStepStats {
            converged: false,
            used_fallback_order: false,
            substeps: 1,
            gs_iterations: 1,
            total_gs_iterations: 1,
            max_gs_delta_m,
            max_beta,
            max_alpha: max_alpha_base,
            non_finite_count,
        });
    }

    let n_iters = params.max_gs_iters;
    let mut phi_in = vec![0.0_f64; n];
    let mut h_t = h.to_vec();
    let mut h_next = h.to_vec();
    let mut sweeps_spent: usize = 0;
    let mut budget_hit = false;
    let mut last_failure: Option<FluvialError> = None;

    // Phase A: Fast receiver-first two-sweep Gauss-Seidel at full dt (no sub-stepping).
    let mut phase_a_iters = 0_usize;
    let mut phase_a_last_delta = 0.0_f64;
    let mut phase_a_worst_cell = 0_usize;
    let mut phase_a_converged = false;
    for iter in 0..n_iters {
        if sweeps_spent >= params.max_total_sweeps {
            budget_hit = true;
            last_failure = Some(FluvialError::NonConvergence {
                cell_index: phase_a_worst_cell,
                last_delta_m: phase_a_last_delta,
                dt_yr: params.dt_yr,
                substeps_attempted: 1,
                gs_iterations: phase_a_iters,
                total_sweeps: sweeps_spent,
                budget_exhausted: true,
            });
            break;
        }
        sweeps_spent += 1;
        phase_a_iters = iter + 1;
        phi_in.fill(0.0);

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

        let mut iter_max_delta = 0.0_f64;
        let mut iter_worst_cell = 0_usize;
        for &node_u in dag.order_downstream_to_upstream {
            let i = node_u as usize;
            let rec = dag.d8_receiver[i];
            if dag.fixed_outlet[i] || rec == NO_RECEIVER {
                continue;
            }
            let r_idx = rec as usize;
            let al = alpha_base[i];
            let target_free = h_t[i] + params.dt_yr * dag.uplift_m_yr[i];
            let dep_up = (params.g_deposition / area_tilde[i]) * phi_in[i];

            let updated = match params.solver_mode {
                FluvialSolverMode::CoupledGaussSeidel => {
                    (target_free * (1.0 + beta[i]) + al * h_next[r_idx] + dep_up)
                        / (1.0 + al + beta[i])
                }
                FluvialSolverMode::NoLocalTermNegativeControl => {
                    (target_free + al * h_next[r_idx] + dep_up) / (1.0 + al)
                }
                FluvialSolverMode::SinglePassNoIterationNegativeControl => unreachable!(),
            };

            let diff = if updated.is_finite() && h_next[i].is_finite() {
                libm::fabs(updated - h_next[i])
            } else {
                f64::NAN
            };
            if !diff.is_finite() {
                iter_max_delta = diff;
                iter_worst_cell = i;
                h_next[i] = updated;
                break;
            }
            if diff > iter_max_delta {
                iter_max_delta = diff;
                iter_worst_cell = i;
            }
            h_next[i] = updated;
        }

        phase_a_last_delta = iter_max_delta;
        phase_a_worst_cell = iter_worst_cell;

        if !iter_max_delta.is_finite() || (iter >= 2 && iter_max_delta > 1.0e6) {
            break;
        }
        if iter > 0 && iter_max_delta < params.gs_tol_m {
            phase_a_converged = h_next.iter().all(|v| v.is_finite());
            break;
        }
    }

    if phase_a_converged {
        h.copy_from_slice(&h_next);
        return Ok(FluvialStepStats {
            converged: true,
            used_fallback_order: false,
            substeps: 1,
            gs_iterations: phase_a_iters,
            total_gs_iterations: sweeps_spent,
            max_gs_delta_m: phase_a_last_delta,
            max_beta,
            max_alpha: max_alpha_base,
            non_finite_count: 0,
        });
    }

    if !budget_hit {
        last_failure = Some(FluvialError::NonConvergence {
            cell_index: phase_a_worst_cell,
            last_delta_m: phase_a_last_delta,
            dt_yr: params.dt_yr,
            substeps_attempted: 1,
            gs_iterations: phase_a_iters,
            total_sweeps: sweeps_spent,
            budget_exhausted: false,
        });
    }

    // Phase B: Fallback to in-place upstream-to-downstream Gauss-Seidel with adaptive dt halving.
    let mut h_trial = vec![0.0_f64; n];
    for halving in 0..=params.max_dt_halvings {
        if budget_hit || sweeps_spent >= params.max_total_sweeps {
            break;
        }
        let substeps = 1_usize << halving;
        let scale = 1.0 / (substeps as f64);
        let dt_sub = params.dt_yr * scale;
        h_trial.copy_from_slice(h);

        let mut halving_ok = true;
        let mut max_gs_iters_seen = 0_usize;
        let mut worst_final_delta = 0.0_f64;

        for _sub in 0..substeps {
            h_t.copy_from_slice(&h_trial);
            h_next.copy_from_slice(&h_trial);
            let mut sub_converged = false;
            let mut iters_used = 0_usize;
            let mut last_delta = 0.0_f64;
            let mut last_worst_cell = 0_usize;

            for iter in 0..n_iters {
                if sweeps_spent >= params.max_total_sweeps {
                    budget_hit = true;
                    last_failure = Some(FluvialError::NonConvergence {
                        cell_index: last_worst_cell,
                        last_delta_m: last_delta,
                        dt_yr: dt_sub,
                        substeps_attempted: substeps,
                        gs_iterations: iters_used,
                        total_sweeps: sweeps_spent,
                        budget_exhausted: true,
                    });
                    break;
                }
                sweeps_spent += 1;
                iters_used = iter + 1;
                phi_in.fill(0.0);

                // In-place upstream-to-downstream Gauss-Seidel: donors update `h_next` and push
                // `phi_in` before node `i` executes, inverting (D + U) cleanly every pass.
                let mut iter_max_delta = 0.0_f64;
                let mut iter_worst_cell = 0_usize;
                for &node_u in dag.order_downstream_to_upstream.iter().rev() {
                    let i = node_u as usize;
                    let rec = dag.d8_receiver[i];
                    if rec == NO_RECEIVER {
                        continue;
                    }
                    let r_idx = rec as usize;
                    if dag.fixed_outlet[i] {
                        phi_in[r_idx] += phi_in[i];
                        continue;
                    }

                    let al = alpha_base[i] * scale;
                    let target_free = h_t[i] + dt_sub * dag.uplift_m_yr[i];
                    let dep_up = (params.g_deposition / area_tilde[i]) * phi_in[i];

                    let updated = match params.solver_mode {
                        FluvialSolverMode::CoupledGaussSeidel => {
                            (target_free * (1.0 + beta[i]) + al * h_next[r_idx] + dep_up)
                                / (1.0 + al + beta[i])
                        }
                        FluvialSolverMode::NoLocalTermNegativeControl => {
                            (target_free + al * h_next[r_idx] + dep_up) / (1.0 + al)
                        }
                        FluvialSolverMode::SinglePassNoIterationNegativeControl => unreachable!(),
                    };

                    let diff = if updated.is_finite() && h_next[i].is_finite() {
                        libm::fabs(updated - h_next[i])
                    } else {
                        f64::NAN
                    };
                    if !diff.is_finite() {
                        iter_max_delta = diff;
                        iter_worst_cell = i;
                        h_next[i] = updated;
                        break;
                    }
                    if diff > iter_max_delta {
                        iter_max_delta = diff;
                        iter_worst_cell = i;
                    }
                    h_next[i] = updated;
                    let net_vol = (target_free - updated) * dx2;
                    phi_in[r_idx] += net_vol + phi_in[i];
                }

                last_delta = iter_max_delta;
                last_worst_cell = iter_worst_cell;

                if !iter_max_delta.is_finite() || (iter >= 2 && iter_max_delta > 1.0e6) {
                    break;
                }
                if iter > 0 && iter_max_delta < params.gs_tol_m {
                    sub_converged = true;
                    break;
                }
            }

            max_gs_iters_seen = max_gs_iters_seen.max(iters_used);

            if budget_hit {
                halving_ok = false;
                break;
            }
            if !sub_converged {
                halving_ok = false;
                last_failure = Some(FluvialError::NonConvergence {
                    cell_index: last_worst_cell,
                    last_delta_m: last_delta,
                    dt_yr: dt_sub,
                    substeps_attempted: substeps,
                    gs_iterations: iters_used,
                    total_sweeps: sweeps_spent,
                    budget_exhausted: false,
                });
                break;
            }
            if let Some((cell_index, &val)) =
                h_next.iter().enumerate().find(|(_, v)| !v.is_finite())
            {
                halving_ok = false;
                last_failure = Some(FluvialError::NonConvergence {
                    cell_index,
                    last_delta_m: val,
                    dt_yr: dt_sub,
                    substeps_attempted: substeps,
                    gs_iterations: iters_used,
                    total_sweeps: sweeps_spent,
                    budget_exhausted: false,
                });
                break;
            }

            worst_final_delta = worst_final_delta.max(last_delta);
            h_trial.copy_from_slice(&h_next);
        }

        if halving_ok {
            h.copy_from_slice(&h_trial);
            return Ok(FluvialStepStats {
                converged: true,
                used_fallback_order: true,
                substeps,
                gs_iterations: max_gs_iters_seen,
                total_gs_iterations: sweeps_spent,
                max_gs_delta_m: worst_final_delta,
                max_beta,
                max_alpha: max_alpha_base * scale,
                non_finite_count: 0,
            });
        }
    }

    Err(last_failure.unwrap_or(FluvialError::NonConvergence {
        cell_index: 0,
        last_delta_m: f64::NAN,
        dt_yr: params.dt_yr,
        substeps_attempted: 1_usize << params.max_dt_halvings,
        gs_iterations: params.max_gs_iters,
        total_sweeps: sweeps_spent,
        budget_exhausted: budget_hit,
    }))
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
    assert!(
        params.n.is_finite() && params.n > 0.0,
        "slope exponent n must be finite and > 0.0"
    );
    assert!(
        params.area_weight_mode != AreaWeightMode::Precipitation
            || (params.min_precip_m_yr.is_finite() && params.min_precip_m_yr > 0.0),
        "min_precip_m_yr must be finite and > 0.0"
    );
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
///
/// Books tectonic uplift directly into `bedrock_h`, then splits the remaining fluvial residual:
/// net aggradation accumulates in `sediment_s`, while net incision strips `sediment_s` first
/// before eroding `bedrock_h`.
pub fn erode_fluvial_step_f32(
    grid: &mut FluvialGridStep<'_>,
    params: &FluvialParams,
) -> Result<FluvialStepStats, FluvialError> {
    validate_implicit_params(params)?;
    let width = grid.width;
    let height = grid.height;
    let n = width * height;
    assert_eq!(grid.bedrock_h.len(), n);
    assert_eq!(grid.sediment_s.len(), n);

    let total_h: Vec<f32> = grid
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

    let stats = step_fastscape_sd_dag(&mut h_f64, &dag, params)?;
    for i in 0..n {
        if fixed_outlet[i] || !h_f64[i].is_finite() {
            continue;
        }
        let uplift_m = f64::from(grid.uplift_m_yr[i]) * params.dt_yr;
        grid.bedrock_h[i] += uplift_m as f32;

        let lifted_surface = f64::from(pf.filled_h[i]) + uplift_m;
        let residual_dh = (h_f64[i] - lifted_surface) as f32;
        if residual_dh >= 0.0 {
            grid.sediment_s[i] += residual_dh;
        } else {
            let inc = -residual_dh;
            if grid.sediment_s[i] >= inc {
                grid.sediment_s[i] -= inc;
            } else {
                let rem = inc - grid.sediment_s[i];
                grid.sediment_s[i] = 0.0;
                grid.bedrock_h[i] -= rem;
            }
        }
    }
    Ok(stats)
}
