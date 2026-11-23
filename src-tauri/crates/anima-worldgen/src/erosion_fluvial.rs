//! Stage S2: Coupled Stream-Power Fluvial Erosion–Deposition (`Fastscape-SD`, `Yuan et al. 2019 [44]`,
//! `Braun 2022 [49]`, `Braun & Willett 2013 [5]`).
//!
//! Uses **precipitation-weighted** upstream area $\tilde{A}_i = \int \nu\,dA'$ ($\nu = P / \nu_0 > 0$)
//! so channel heads and arid rain-shadow wadis keep finite $\beta_i = G\Delta x^2 / \tilde{A}_i$.
//! Runoff `max(0, P - PET)` belongs solely to Stage S5 discharge `Q_ma`, never to $\tilde{A}_i$.

use crate::hydrology::{priority_flood_d8_lean_into, NO_DIR, NO_RECEIVER};

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
    /// Optional per-step maximum downward surface incision cap in metres (`<= 0.25 * T_min`).
    /// When exceeded in [`erode_fluvial_step_f32_lean`], returns [`FluvialError::IncisionCapExceeded`]
    /// BEFORE mutating `bedrock_h` or `sediment_s`.
    pub max_incision_cap_m: Option<f64>,
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
            max_incision_cap_m: None,
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
    IncisionCapExceeded {
        max_dh_m: f64,
        cap_m: f64,
        recommended_k: usize,
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
            Self::IncisionCapExceeded {
                max_dh_m,
                cap_m,
                recommended_k,
            } => write!(
                f,
                "Fastscape-SD per-step incision {max_dh_m:.3} m exceeds cap {cap_m:.3} m (recommended_k={recommended_k})"
            ),
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
    pub max_bedrock_incision_m: f64,
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
    if let Some(cap) = params.max_incision_cap_m {
        if !cap.is_finite() || cap <= 0.0 {
            return Err(FluvialError::InvalidParameter {
                name: "max_incision_cap_m",
                value: cap,
            });
        }
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
            max_bedrock_incision_m: 0.0,
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
            max_bedrock_incision_m: 0.0,
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
            max_bedrock_incision_m: 0.0,
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

    // Phase B: Fallback to in-place upstream-to-downstream Gauss-Seidel with adaptive dt halving
    // (eliminates `h_trial` via `swap(&mut h_t, &mut h_next)` across substeps).
    for halving in 0..=params.max_dt_halvings {
        if budget_hit || sweeps_spent >= params.max_total_sweeps {
            break;
        }
        let substeps = 1_usize << halving;
        let scale = 1.0 / (substeps as f64);
        let dt_sub = params.dt_yr * scale;
        h_t.copy_from_slice(h);

        let mut halving_ok = true;
        let mut max_gs_iters_seen = 0_usize;
        let mut worst_final_delta = 0.0_f64;

        for _sub in 0..substeps {
            h_next.copy_from_slice(&h_t);
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
            std::mem::swap(&mut h_t, &mut h_next);
        }

        if halving_ok {
            h.copy_from_slice(&h_t);
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
                max_bedrock_incision_m: 0.0,
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
/// Delegates to [`erode_fluvial_step_f32_lean`] so callers avoid multi-hundred-MiB `f64` copies.
pub fn erode_fluvial_step_f32(
    grid: &mut FluvialGridStep<'_>,
    params: &FluvialParams,
) -> Result<FluvialStepStats, FluvialError> {
    let mut lean = FluvialGridLean {
        bedrock_h: grid.bedrock_h,
        sediment_s: grid.sediment_s,
        uplift_m_yr: grid.uplift_m_yr,
        k_erodibility: FluvialErodibility::PerCell(grid.k_erodibility),
        precip_m_yr: grid.precip_m_yr,
        pet_m_yr: Some(grid.pet_m_yr),
        width: grid.width,
        height: grid.height,
        sea_level_m: grid.sea_level_m,
    };
    erode_fluvial_step_f32_lean(&mut lean, params, None)
}

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
const LITH_K_LUT: [f64; 6] = [0.15, 0.22, 0.55, 0.30, 1.00, 1.20];

#[inline]
fn dir_neighbor(idx: usize, dir: u8, width: usize) -> usize {
    let r = (idx / width) as isize + DZ8[dir as usize];
    let c = (idx % width) as isize + DX8[dir as usize];
    r as usize * width + c as usize
}

/// Erodibility source for [`FluvialGridLean`].
pub enum FluvialErodibility<'a> {
    PerCell(&'a [f32]),
    LithologyLut {
        lithology_id: &'a [u8],
        k_scale: f64,
    },
}

#[derive(Debug, Default)]
struct SplitF64 {
    half: usize,
    lo: Vec<f64>,
    hi: Vec<f64>,
}

impl SplitF64 {
    fn resize(&mut self, n: usize, val: f64) {
        self.half = n / 2;
        self.lo.resize(self.half, val);
        self.hi.resize(n - self.half, val);
    }

    fn fill(&mut self, val: f64) {
        self.lo.fill(val);
        self.hi.fill(val);
    }

    #[inline]
    fn get(&self, i: usize) -> f64 {
        if i < self.half {
            self.lo[i]
        } else {
            self.hi[i - self.half]
        }
    }

    #[inline]
    fn set(&mut self, i: usize, v: f64) {
        if i < self.half {
            self.lo[i] = v;
        } else {
            self.hi[i - self.half] = v;
        }
    }

    #[inline]
    fn add_assign(&mut self, i: usize, v: f64) {
        if i < self.half {
            self.lo[i] += v;
        } else {
            self.hi[i - self.half] += v;
        }
    }

    fn copy_from_f32(&mut self, src: &[f32]) {
        for (dst, &v) in self.lo.iter_mut().zip(src[..self.half].iter()) {
            *dst = f64::from(v);
        }
        for (dst, &v) in self.hi.iter_mut().zip(src[self.half..].iter()) {
            *dst = f64::from(v);
        }
    }

    fn copy_from_split(&mut self, src: &Self) {
        self.lo.copy_from_slice(&src.lo);
        self.hi.copy_from_slice(&src.hi);
    }

    fn find_non_finite(&self) -> Option<(usize, f64)> {
        if let Some((idx, &v)) = self.lo.iter().enumerate().find(|(_, v)| !v.is_finite()) {
            return Some((idx, v));
        }
        self.hi
            .iter()
            .enumerate()
            .find(|(_, v)| !v.is_finite())
            .map(|(idx, &v)| (self.half + idx, v))
    }

    fn all_finite(&self) -> bool {
        self.lo.iter().all(|v| v.is_finite()) && self.hi.iter().all(|v| v.is_finite())
    }
}

/// Reusable scratch buffers for [`erode_fluvial_step_f32_lean`] (`592 MiB` Phase A @ `4096^2`,
/// split into `<= 64 MiB` halves so `libmalloc` reuses freed blocks across downstream stages).
#[derive(Debug, Default)]
pub struct FluvialLeanScratch {
    pub filled_h: Vec<f32>,
    pub d8_dir: Vec<u8>,
    pub order: Vec<u32>,
    inv_area_tilde: Vec<f32>,
    alpha_base: Vec<f32>,
    phi_in: SplitF64,
    h_next: SplitF64,
    h_t: SplitF64,
}

impl FluvialLeanScratch {
    fn prepare(&mut self, n: usize) {
        self.filled_h.resize(n, 0.0);
        self.d8_dir.resize(n, NO_DIR);
        if self.order.capacity() < n {
            self.order.reserve(n - self.order.len());
        }
        self.inv_area_tilde.resize(n, 0.0);
        self.alpha_base.resize(n, 0.0);
        self.phi_in.resize(n, 0.0);
        self.h_next.resize(n, 0.0);
    }

    pub(crate) fn thermal_halves_mut(
        &mut self,
    ) -> (&mut [f64], &mut [f64], &mut [f64], &mut [f64]) {
        (
            &mut self.phi_in.lo,
            &mut self.phi_in.hi,
            &mut self.h_next.lo,
            &mut self.h_next.hi,
        )
    }
}

/// Memory-lean gridded Stage S2 inputs (supports LUT erodibility and optional `pet_m_yr`).
pub struct FluvialGridLean<'a> {
    pub bedrock_h: &'a mut [f32],
    pub sediment_s: &'a mut [f32],
    pub uplift_m_yr: &'a [f32],
    pub k_erodibility: FluvialErodibility<'a>,
    pub precip_m_yr: &'a [f32],
    pub pet_m_yr: Option<&'a [f32]>,
    pub width: usize,
    pub height: usize,
    pub sea_level_m: Option<f32>,
}

/// Memory-lean `f32`-streaming Priority-Flood + Phase A/B Fastscape-SD step (`<= 928 MiB` Phase A).
pub fn erode_fluvial_step_f32_lean(
    grid: &mut FluvialGridLean<'_>,
    params: &FluvialParams,
    scratch: Option<&mut FluvialLeanScratch>,
) -> Result<FluvialStepStats, FluvialError> {
    validate_implicit_params(params)?;
    let width = grid.width;
    let height = grid.height;
    let n = width * height;
    assert_eq!(grid.bedrock_h.len(), n);
    assert_eq!(grid.sediment_s.len(), n);
    assert_eq!(grid.uplift_m_yr.len(), n);
    assert_eq!(grid.precip_m_yr.len(), n);

    let mut local_scratch;
    let s = match scratch {
        Some(ext) => ext,
        None => {
            local_scratch = FluvialLeanScratch::default();
            &mut local_scratch
        }
    };
    s.prepare(n);

    for ((dst, &b), &sed) in s
        .filled_h
        .iter_mut()
        .zip(grid.bedrock_h.iter())
        .zip(grid.sediment_s.iter())
    {
        *dst = b + sed;
    }

    priority_flood_d8_lean_into(
        &mut s.filled_h,
        &mut s.d8_dir,
        &mut s.order,
        width,
        height,
        grid.sea_level_m,
        true,
    );

    let dx2 = params.dx_m * params.dx_m;
    let nu_ref = params.nu_ref_m_yr.max(1e-12);
    // Temporarily accumulate weighted drainage area in `s.h_next` (not used until GS sweeps).
    for i in 0..n {
        let p = f64::from(grid.precip_m_yr[i]);
        let nu = match params.area_weight_mode {
            AreaWeightMode::Precipitation => p.max(params.min_precip_m_yr) / nu_ref,
            AreaWeightMode::UnclampedRunoffNegativeControl => {
                let pet = grid.pet_m_yr.map_or(0.0, |sl| f64::from(sl[i]));
                (p - pet).max(0.0) / nu_ref
            }
        };
        s.h_next.set(i, nu * dx2);
    }
    for &node_u in s.order.iter().rev() {
        let i = node_u as usize;
        let d = s.d8_dir[i];
        if d < NO_DIR {
            let rec = dir_neighbor(i, d, width);
            let add_v = s.h_next.get(i);
            s.h_next.add_assign(rec, add_v);
        }
    }

    let mut max_beta = 0.0_f64;
    let mut max_alpha_base = 0.0_f64;
    for i in 0..n {
        let d = s.d8_dir[i];
        let fixed = d >= NO_DIR || grid.sea_level_m.is_some_and(|sl| s.filled_h[i] <= sl);
        if fixed {
            s.alpha_base[i] = 0.0;
            s.inv_area_tilde[i] = 0.0; // inv_area_tilde = 0 for fixed outlets
            continue;
        }
        let d_i = params.dx_m * DIST8[d as usize];
        let a_tilde = s.h_next.get(i);
        let a_m = if a_tilde > 0.0 {
            libm::pow(a_tilde, params.m)
        } else {
            0.0
        };
        let k_i = match grid.k_erodibility {
            FluvialErodibility::PerCell(k_slice) => f64::from(k_slice[i]),
            FluvialErodibility::LithologyLut {
                lithology_id,
                k_scale,
            } => LITH_K_LUT[(lithology_id[i] as usize).min(5)] * k_scale,
        };
        let al = (k_i * a_m * params.dt_yr) / d_i;
        let inv_a = 1.0 / a_tilde;
        let be = if params.g_deposition > 0.0 {
            params.g_deposition * dx2 * inv_a
        } else {
            0.0
        };
        s.alpha_base[i] = al as f32;
        s.inv_area_tilde[i] = inv_a as f32;
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

    let check_incision_cap = |grid: &FluvialGridLean<'_>,
                              filled_h: &[f32],
                              d8_dir: &[u8],
                              alpha_base: &[f32],
                              h_final: &SplitF64|
     -> Result<f64, FluvialError> {
        let Some(cap_m) = params.max_incision_cap_m else {
            return Ok(0.0);
        };
        let mut max_dh_m = 0.0_f64;
        let mut max_k = 1_usize;
        for i in 0..n {
            let d = d8_dir[i];
            let fixed = d >= NO_DIR || grid.sea_level_m.is_some_and(|sl| filled_h[i] <= sl);
            let hf = h_final.get(i);
            if fixed || !hf.is_finite() {
                continue;
            }
            let uplift_m = f64::from(grid.uplift_m_yr[i]) * params.dt_yr;
            let lifted_surface = f64::from(filled_h[i]) + uplift_m;
            let surf_inc = (lifted_surface - hf).max(0.0);
            let bed_inc = (surf_inc - f64::from(grid.sediment_s[i])).max(0.0);
            if bed_inc > max_dh_m {
                max_dh_m = bed_inc;
            }
            if bed_inc > cap_m {
                let rec = dir_neighbor(i, d, width);
                let h_rec = h_final.get(rec);
                let dh_avail =
                    (lifted_surface - f64::from(grid.sediment_s[i]) - h_rec).max(bed_inc);
                let al_i = f64::from(alpha_base[i]);
                let al_eff = if dh_avail > bed_inc + 1e-6 {
                    (bed_inc / (dh_avail - bed_inc)).min(al_i)
                } else {
                    al_i
                };
                let k_est = if dh_avail > cap_m + 1e-6 {
                    (al_eff * (dh_avail / cap_m - 1.0)).ceil()
                } else {
                    (bed_inc / cap_m).ceil()
                };
                max_k = max_k.max(k_est.clamp(2.0, 16.0) as usize);
            }
        }
        if max_dh_m > cap_m {
            return Err(FluvialError::IncisionCapExceeded {
                max_dh_m,
                cap_m,
                recommended_k: max_k,
            });
        }
        Ok(max_dh_m)
    };

    let apply_surface =
        |grid: &mut FluvialGridLean<'_>, filled_h: &[f32], d8_dir: &[u8], h_final: &SplitF64| {
            for i in 0..n {
                let fixed =
                    d8_dir[i] >= NO_DIR || grid.sea_level_m.is_some_and(|sl| filled_h[i] <= sl);
                let hf = h_final.get(i);
                if fixed || !hf.is_finite() {
                    continue;
                }
                let uplift_m = f64::from(grid.uplift_m_yr[i]) * params.dt_yr;
                grid.bedrock_h[i] += uplift_m as f32;
                let lifted_surface = f64::from(filled_h[i]) + uplift_m;
                let residual_dh = (hf - lifted_surface) as f32;
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
        };

    if params.g_deposition == 0.0 {
        s.h_next.copy_from_f32(&s.filled_h);
        for &node_u in &s.order {
            let i = node_u as usize;
            let d = s.d8_dir[i];
            if d >= NO_DIR || grid.sea_level_m.is_some_and(|sl| s.filled_h[i] <= sl) {
                continue;
            }
            let rec = dir_neighbor(i, d, width);
            let al = f64::from(s.alpha_base[i]);
            let upd = (s.h_next.get(i)
                + params.dt_yr * f64::from(grid.uplift_m_yr[i])
                + al * s.h_next.get(rec))
                / (1.0 + al);
            s.h_next.set(i, upd);
        }
        if let Some((cell_index, val)) = s.h_next.find_non_finite() {
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
        let max_bedrock_incision_m =
            check_incision_cap(grid, &s.filled_h, &s.d8_dir, &s.alpha_base, &s.h_next)?;
        apply_surface(grid, &s.filled_h, &s.d8_dir, &s.h_next);
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
            max_bedrock_incision_m,
        });
    }

    if params.solver_mode == FluvialSolverMode::SinglePassNoIterationNegativeControl {
        s.h_next.copy_from_f32(&s.filled_h);
        let mut max_gs_delta_m = 0.0_f64;
        for &node_u in &s.order {
            let i = node_u as usize;
            let d = s.d8_dir[i];
            if d >= NO_DIR || grid.sea_level_m.is_some_and(|sl| s.filled_h[i] <= sl) {
                continue;
            }
            let r_idx = dir_neighbor(i, d, width);
            let beta_i = params.g_deposition * dx2 * f64::from(s.inv_area_tilde[i]);
            let target_free =
                f64::from(s.filled_h[i]) + params.dt_yr * f64::from(grid.uplift_m_yr[i]);
            let al = f64::from(s.alpha_base[i]);
            let updated =
                (target_free * (1.0 + beta_i) + al * s.h_next.get(r_idx)) / (1.0 + al + beta_i);
            let diff = libm::fabs(updated - s.h_next.get(i));
            if diff > max_gs_delta_m || !diff.is_finite() {
                max_gs_delta_m = diff;
            }
            s.h_next.set(i, updated);
        }
        let non_finite_count = s.h_next.lo.iter().filter(|v| !v.is_finite()).count()
            + s.h_next.hi.iter().filter(|v| !v.is_finite()).count();
        apply_surface(grid, &s.filled_h, &s.d8_dir, &s.h_next);
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
            max_bedrock_incision_m: 0.0,
        });
    }

    s.h_next.copy_from_f32(&s.filled_h);
    let n_iters = params.max_gs_iters;
    let mut sweeps_spent: usize = 0;
    let mut budget_hit = false;
    let mut last_failure: Option<FluvialError> = None;

    // Phase A: Fast receiver-first two-sweep Gauss-Seidel reading `filled_h` directly (`0` `h_t`).
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
        s.phi_in.fill(0.0);

        for &node_u in s.order.iter().rev() {
            let k = node_u as usize;
            let d = s.d8_dir[k];
            if d >= NO_DIR {
                continue;
            }
            let r_idx = dir_neighbor(k, d, width);
            let net_vol = if grid.sea_level_m.is_some_and(|sl| s.filled_h[k] <= sl) {
                0.0
            } else {
                (f64::from(s.filled_h[k]) + params.dt_yr * f64::from(grid.uplift_m_yr[k])
                    - s.h_next.get(k))
                    * dx2
            };
            let p_k = s.phi_in.get(k);
            s.phi_in.add_assign(r_idx, net_vol + p_k);
        }

        let mut iter_max_delta = 0.0_f64;
        let mut iter_worst_cell = 0_usize;
        for &node_u in &s.order {
            let i = node_u as usize;
            let d = s.d8_dir[i];
            if d >= NO_DIR || grid.sea_level_m.is_some_and(|sl| s.filled_h[i] <= sl) {
                continue;
            }
            let r_idx = dir_neighbor(i, d, width);
            let al = f64::from(s.alpha_base[i]);
            let inv_a = f64::from(s.inv_area_tilde[i]);
            let beta_i = params.g_deposition * dx2 * inv_a;
            let target_free =
                f64::from(s.filled_h[i]) + params.dt_yr * f64::from(grid.uplift_m_yr[i]);
            let dep_up = params.g_deposition * inv_a * s.phi_in.get(i);

            let h_rec = s.h_next.get(r_idx);
            let updated = match params.solver_mode {
                FluvialSolverMode::CoupledGaussSeidel => {
                    (target_free * (1.0 + beta_i) + al * h_rec + dep_up) / (1.0 + al + beta_i)
                }
                FluvialSolverMode::NoLocalTermNegativeControl => {
                    (target_free + al * h_rec + dep_up) / (1.0 + al)
                }
                FluvialSolverMode::SinglePassNoIterationNegativeControl => unreachable!(),
            };

            let prev_i = s.h_next.get(i);
            let diff = if updated.is_finite() && prev_i.is_finite() {
                libm::fabs(updated - prev_i)
            } else {
                f64::NAN
            };
            if !diff.is_finite() {
                iter_max_delta = diff;
                iter_worst_cell = i;
                s.h_next.set(i, updated);
                break;
            }
            if diff > iter_max_delta {
                iter_max_delta = diff;
                iter_worst_cell = i;
            }
            s.h_next.set(i, updated);
        }

        phase_a_last_delta = iter_max_delta;
        phase_a_worst_cell = iter_worst_cell;

        if !iter_max_delta.is_finite() || (iter >= 2 && iter_max_delta > 1.0e6) {
            break;
        }
        if iter > 0 && iter_max_delta < params.gs_tol_m {
            phase_a_converged = s.h_next.all_finite();
            break;
        }
    }

    if phase_a_converged {
        let max_bedrock_incision_m =
            check_incision_cap(grid, &s.filled_h, &s.d8_dir, &s.alpha_base, &s.h_next)?;
        apply_surface(grid, &s.filled_h, &s.d8_dir, &s.h_next);
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
            max_bedrock_incision_m,
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

    // Phase B: Lazy `h_t` allocation + in-place upstream-first GS with adaptive dt halving.
    for halving in 0..=params.max_dt_halvings {
        if budget_hit || sweeps_spent >= params.max_total_sweeps {
            break;
        }
        let substeps = 1_usize << halving;
        let scale = 1.0 / (substeps as f64);
        let dt_sub = params.dt_yr * scale;
        if substeps > 1 {
            s.h_t.resize(n, 0.0);
            s.h_t.copy_from_f32(&s.filled_h);
        }

        let mut halving_ok = true;
        let mut max_gs_iters_seen = 0_usize;
        let mut worst_final_delta = 0.0_f64;

        for _sub in 0..substeps {
            if substeps == 1 {
                s.h_next.copy_from_f32(&s.filled_h);
            } else {
                s.h_next.copy_from_split(&s.h_t);
            }
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
                s.phi_in.fill(0.0);

                let mut iter_max_delta = 0.0_f64;
                let mut iter_worst_cell = 0_usize;
                for &node_u in s.order.iter().rev() {
                    let i = node_u as usize;
                    let d = s.d8_dir[i];
                    if d >= NO_DIR {
                        continue;
                    }
                    let r_idx = dir_neighbor(i, d, width);
                    if grid.sea_level_m.is_some_and(|sl| s.filled_h[i] <= sl) {
                        let p_i = s.phi_in.get(i);
                        s.phi_in.add_assign(r_idx, p_i);
                        continue;
                    }

                    let al = f64::from(s.alpha_base[i]) * scale;
                    let inv_a = f64::from(s.inv_area_tilde[i]);
                    let beta_i = params.g_deposition * dx2 * inv_a;
                    let h_t_i = if substeps == 1 {
                        f64::from(s.filled_h[i])
                    } else {
                        s.h_t.get(i)
                    };
                    let target_free = h_t_i + dt_sub * f64::from(grid.uplift_m_yr[i]);
                    let p_i = s.phi_in.get(i);
                    let dep_up = params.g_deposition * inv_a * p_i;

                    let h_rec = s.h_next.get(r_idx);
                    let updated = match params.solver_mode {
                        FluvialSolverMode::CoupledGaussSeidel => {
                            (target_free * (1.0 + beta_i) + al * h_rec + dep_up)
                                / (1.0 + al + beta_i)
                        }
                        FluvialSolverMode::NoLocalTermNegativeControl => {
                            (target_free + al * h_rec + dep_up) / (1.0 + al)
                        }
                        FluvialSolverMode::SinglePassNoIterationNegativeControl => unreachable!(),
                    };

                    let prev_i = s.h_next.get(i);
                    let diff = if updated.is_finite() && prev_i.is_finite() {
                        libm::fabs(updated - prev_i)
                    } else {
                        f64::NAN
                    };
                    if !diff.is_finite() {
                        iter_max_delta = diff;
                        iter_worst_cell = i;
                        s.h_next.set(i, updated);
                        break;
                    }
                    if diff > iter_max_delta {
                        iter_max_delta = diff;
                        iter_worst_cell = i;
                    }
                    s.h_next.set(i, updated);
                    let net_vol = (target_free - updated) * dx2;
                    s.phi_in.add_assign(r_idx, net_vol + p_i);
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
            if let Some((cell_index, val)) = s.h_next.find_non_finite() {
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
            std::mem::swap(&mut s.h_t, &mut s.h_next);
        }

        if halving_ok {
            let max_bedrock_incision_m =
                check_incision_cap(grid, &s.filled_h, &s.d8_dir, &s.alpha_base, &s.h_t)?;
            apply_surface(grid, &s.filled_h, &s.d8_dir, &s.h_t);
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
                max_bedrock_incision_m,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incision_cap_aborts_without_mutating_bedrock_or_sediment() {
        let w = 8usize;
        let h = 8usize;
        let n = w * h;
        let mut bedrock = vec![0.0_f32; n];
        for r in 0..h {
            for c in 0..w {
                bedrock[r * w + c] = if r == 0 || r + 1 == h || c == 0 || c + 1 == w {
                    0.0
                } else {
                    1_200.0 + (c as f32) * 120.0
                };
            }
        }
        let orig_bedrock = bedrock.clone();
        let mut sediment = vec![0.0_f32; n];
        let orig_sediment = sediment.clone();
        let uplift = vec![0.0_f32; n];
        let k = vec![2.2e-5_f32; n];
        let precip = vec![4.0_f32; n];

        let mut grid = FluvialGridLean {
            bedrock_h: &mut bedrock,
            sediment_s: &mut sediment,
            uplift_m_yr: &uplift,
            k_erodibility: FluvialErodibility::PerCell(&k),
            precip_m_yr: &precip,
            pet_m_yr: None,
            width: w,
            height: h,
            sea_level_m: None,
        };
        let params = FluvialParams {
            dx_m: 10.0,
            dt_yr: 10_000.0,
            g_deposition: 0.85,
            max_incision_cap_m: Some(25.0),
            ..FluvialParams::default()
        };
        let err = erode_fluvial_step_f32_lean(&mut grid, &params, None)
            .expect_err("steep scarp must trip 25 m incision cap");
        assert!(
            matches!(
                err,
                FluvialError::IncisionCapExceeded {
                    max_dh_m,
                    cap_m,
                    recommended_k
                } if max_dh_m > cap_m && (cap_m - 25.0).abs() < 1e-9 && (2..=16).contains(&recommended_k)
            ),
            "unexpected error: {err:?}"
        );
        assert_eq!(bedrock, orig_bedrock);
        assert_eq!(sediment, orig_sediment);
    }

    #[test]
    fn incision_cap_ignores_thick_sediment_stripping_and_estimates_concave_k() {
        let w = 8usize;
        let h = 8usize;
        let n = w * h;
        let mut bedrock = vec![0.0_f32; n];
        let mut sediment = vec![0.0_f32; n];
        for r in 0..h {
            for c in 0..w {
                if !(r == 0 || r + 1 == h || c == 0 || c + 1 == w) {
                    bedrock[r * w + c] = 500.0;
                    sediment[r * w + c] = 120.0 + (c as f32) * 40.0;
                }
            }
        }
        let uplift = vec![0.0_f32; n];
        let k = vec![2.2e-5_f32; n];
        let precip = vec![4.0_f32; n];

        let mut grid_sed = FluvialGridLean {
            bedrock_h: &mut bedrock,
            sediment_s: &mut sediment,
            uplift_m_yr: &uplift,
            k_erodibility: FluvialErodibility::PerCell(&k),
            precip_m_yr: &precip,
            pet_m_yr: None,
            width: w,
            height: h,
            sea_level_m: None,
        };
        let params_sed = FluvialParams {
            dx_m: 10.0,
            dt_yr: 1_000.0,
            g_deposition: 0.0,
            max_incision_cap_m: Some(25.0),
            ..FluvialParams::default()
        };
        let stats_sed = erode_fluvial_step_f32_lean(&mut grid_sed, &params_sed, None)
            .expect("pure sediment stripping with 0 bedrock incision must not trip bedrock cap");
        assert!(stats_sed.max_bedrock_incision_m <= 25.0);

        let mut bed_steep = vec![0.0_f32; n];
        for r in 1..(h - 1) {
            for c in 1..(w - 1) {
                bed_steep[r * w + c] = 1_500.0 + (c as f32) * 200.0;
            }
        }
        let mut sed_zero = vec![0.0_f32; n];
        let mut grid_steep = FluvialGridLean {
            bedrock_h: &mut bed_steep,
            sediment_s: &mut sed_zero,
            uplift_m_yr: &uplift,
            k_erodibility: FluvialErodibility::PerCell(&k),
            precip_m_yr: &precip,
            pet_m_yr: None,
            width: w,
            height: h,
            sea_level_m: None,
        };
        let params_steep = FluvialParams {
            dx_m: 10.0,
            dt_yr: 80_000.0,
            g_deposition: 0.85,
            max_incision_cap_m: Some(25.0),
            ..FluvialParams::default()
        };
        let err = erode_fluvial_step_f32_lean(&mut grid_steep, &params_steep, None)
            .expect_err("steep high-alpha bedrock step must trigger IncisionCapExceeded");
        match err {
            FluvialError::IncisionCapExceeded {
                max_dh_m,
                cap_m,
                recommended_k,
            } => {
                assert!(max_dh_m > cap_m);
                assert!((2..=16).contains(&recommended_k));
            }
            other => panic!("expected IncisionCapExceeded, got {other:?}"),
        }
    }
}
