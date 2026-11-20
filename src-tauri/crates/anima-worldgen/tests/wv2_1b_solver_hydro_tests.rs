#![cfg(feature = "generate")]

use anima_worldgen::config::WorldV2Config;
use anima_worldgen::erosion_fluvial::{
    erode_fluvial_step_f32, step_explicit_forward_euler_dag, step_fastscape_sd_dag, AreaWeightMode,
    FluvialDagView, FluvialError, FluvialGridStep, FluvialParams, FluvialSolverMode,
};
use anima_worldgen::erosion_thermal::{
    deg_to_slope, relax_thermal_and_hillslope, slope_to_deg, ThermalHillslopeParams,
};
use anima_worldgen::hydrology::{
    count_pits, priority_flood_d8, route_hybrid_mfd_d8, HydrologyConfig, NO_RECEIVER,
};
use anima_worldgen::tectonic::{generate_uplift, LithologyClass};
use anima_worldgen::LITHOLOGY_CLASS_COUNT;
use rayon::ThreadPoolBuilder;
use std::time::Instant;

fn make_1d_stream_dag(n: usize) -> (Vec<u32>, Vec<f32>, Vec<u32>, Vec<bool>) {
    // Node 0 is baselevel outlet (h = 0 fixed); node k drains to k - 1 down to 0.
    let mut rec = vec![NO_RECEIVER; n];
    let df = vec![1.0_f32; n];
    let mut fixed = vec![false; n];
    fixed[0] = true;
    for (i, r) in rec.iter_mut().enumerate().skip(1) {
        *r = (i - 1) as u32;
    }
    let order: Vec<u32> = (0..n as u32).collect();
    (rec, df, order, fixed)
}

#[test]
fn fastscape_g0_concavity_converges_to_m_over_n() {
    let n = 64usize;
    let dx = 10.0_f64;
    let u_val = 5e-4_f64;
    let k_val = 2e-5_f64;
    let target_theta = 0.45_f64;

    let (rec, df, order, fixed) = make_1d_stream_dag(n);
    let uplift = vec![u_val; n];
    let k = vec![k_val; n];
    let p = vec![1.0_f64; n];
    let e = vec![0.0_f64; n];
    let dag = FluvialDagView {
        width: n,
        height: 1,
        d8_receiver: &rec,
        receiver_dist_factor: &df,
        order_downstream_to_upstream: &order,
        fixed_outlet: &fixed,
        uplift_m_yr: &uplift,
        k_erodibility: &k,
        precip_m_yr: &p,
        pet_m_yr: &e,
    };

    let mut h: Vec<f64> = (0..n).map(|i| i as f64 * 0.5).collect();
    let params = FluvialParams {
        dx_m: dx,
        dt_yr: 100_000.0,
        m: target_theta,
        n: 1.0,
        g_deposition: 0.0,
        ..FluvialParams::default()
    };
    for _ in 0..100 {
        let st = step_fastscape_sd_dag(&mut h, &dag, &params).expect("G=0 step converges");
        assert!(st.converged);
        assert_eq!(st.non_finite_count, 0);
    }

    // Ordinary least squares on ln(S) vs ln(A_tilde) across all interior stream nodes.
    let mut sum_x = 0.0_f64;
    let mut sum_y = 0.0_f64;
    let mut count = 0.0_f64;
    let mut xs = Vec::with_capacity(n - 1);
    let mut ys = Vec::with_capacity(n - 1);
    for i in 1..n {
        let area = ((n - i) as f64) * dx * dx;
        let slope = (h[i] - h[i - 1]) / dx;
        assert!(slope > 0.0);
        let x = libm::log(area);
        let y = libm::log(slope);
        xs.push(x);
        ys.push(y);
        sum_x += x;
        sum_y += y;
        count += 1.0;
    }
    let mx = sum_x / count;
    let my = sum_y / count;
    let mut num = 0.0_f64;
    let mut den = 0.0_f64;
    for (&x, &y) in xs.iter().zip(ys.iter()) {
        num += (x - mx) * (y - my);
        den += (x - mx) * (x - mx);
    }
    let measured_theta = -num / den;
    assert!(
        (measured_theta - target_theta).abs() <= 0.03,
        "measured concavity theta = {measured_theta}, expected {target_theta} +- 0.03"
    );
}

#[test]
fn fastscape_g_pos_steady_state_and_neg_control_a() {
    let n = 64usize;
    let dx = 10.0_f64;
    let u_val = 5e-4_f64;
    let k_val = 2e-5_f64;
    let m_val = 0.45_f64;
    let g_val = 0.85_f64;

    let (rec, df, order, fixed) = make_1d_stream_dag(n);
    let uplift = vec![u_val; n];
    let k = vec![k_val; n];
    let p = vec![1.0_f64; n];
    let e = vec![0.0_f64; n];
    let dag = FluvialDagView {
        width: n,
        height: 1,
        d8_receiver: &rec,
        receiver_dist_factor: &df,
        order_downstream_to_upstream: &order,
        fixed_outlet: &fixed,
        uplift_m_yr: &uplift,
        k_erodibility: &k,
        precip_m_yr: &p,
        pet_m_yr: &e,
    };

    let mut max_rel_err_good = 0.0_f64;
    let mut h: Vec<f64> = (0..n).map(|i| i as f64 * 0.5).collect();
    let params = FluvialParams {
        dx_m: dx,
        dt_yr: 100_000.0,
        m: m_val,
        n: 1.0,
        g_deposition: g_val,
        max_gs_iters: 32,
        gs_tol_m: 1e-8,
        solver_mode: FluvialSolverMode::CoupledGaussSeidel,
        ..FluvialParams::default()
    };
    for _ in 0..120 {
        let stats = step_fastscape_sd_dag(&mut h, &dag, &params).expect("G>0 step converges");
        assert!(stats.converged);
        assert_eq!(stats.non_finite_count, 0);
    }

    for i in 1..n {
        let a_tilde = ((n - i) as f64) * dx * dx;
        let s_star = (1.0 + g_val) * u_val / (k_val * libm::pow(a_tilde, m_val));
        let s_meas = (h[i] - h[i - 1]) / dx;
        let rel = libm::fabs(s_meas - s_star) / s_star;
        max_rel_err_good = max_rel_err_good.max(rel);
    }
    assert!(
        max_rel_err_good <= 0.01,
        "G>0 coupled GS steady-state error incl. channel heads was {max_rel_err_good:.4} (> 1%)"
    );

    // Negative control (a.1): omitting node i's local term (`beta_i`) fails at channel heads (`i = n - 1`).
    let mut h_no_local: Vec<f64> = (0..n).map(|i| i as f64 * 0.5).collect();
    let params_no_local = FluvialParams {
        solver_mode: FluvialSolverMode::NoLocalTermNegativeControl,
        ..params.clone()
    };
    for _ in 0..120 {
        let st = step_fastscape_sd_dag(&mut h_no_local, &dag, &params_no_local)
            .expect("NoLocal negative control step");
        assert!(st.converged);
    }
    let head = n - 1;
    let s_star_head = (1.0 + g_val) * u_val / (k_val * libm::pow(dx * dx, m_val));
    let s_head_no_local = (h_no_local[head] - h_no_local[head - 1]) / dx;
    let head_err = libm::fabs(s_head_no_local - s_star_head) / s_star_head;
    assert!(
        head_err > 0.15,
        "negative control (NoLocalTerm) unexpectedly matched channel head slope: err = {head_err}"
    );

    // Negative control (a.2): skipping Gauss-Seidel Φ_in iteration (1 pass with Φ_in = 0) fails along trunk.
    let mut h_no_iter: Vec<f64> = (0..n).map(|i| i as f64 * 0.5).collect();
    let params_no_iter = FluvialParams {
        solver_mode: FluvialSolverMode::SinglePassNoIterationNegativeControl,
        ..params
    };
    for _ in 0..120 {
        let st = step_fastscape_sd_dag(&mut h_no_iter, &dag, &params_no_iter)
            .expect("SinglePass negative control step");
        assert!(!st.converged);
    }
    let a_mid = ((n - 10) as f64) * dx * dx;
    let s_star_mid = (1.0 + g_val) * u_val / (k_val * libm::pow(a_mid, m_val));
    let s_mid_no_iter = (h_no_iter[10] - h_no_iter[9]) / dx;
    let mid_err = libm::fabs(s_mid_no_iter - s_star_mid) / s_star_mid;
    assert!(
        mid_err > 0.15,
        "negative control (SinglePassNoIteration) unexpectedly matched trunk slope: err = {mid_err}"
    );
}

#[test]
fn fastscape_g_pos_short_transient_matches_explicit_small_dt_reference() {
    // Carry-over WV2-D r4 note: use K_test = 1e-3, T = 1,000 yr transient, dt_imp = 25-50 yr so Courant <= 0.45 (< 0.5),
    // and assert stated error norm: RMS(h_imp - h_exp) / RMS(h_exp - h_0) <= 0.02 (<= 2%).
    fn relative_transient_rms(h0: &[f64], h_exp: &[f64], h_imp: &[f64], fixed: &[bool]) -> f64 {
        let mut sq_diff = 0.0_f64;
        let mut sq_change = 0.0_f64;
        for i in 0..h0.len() {
            if fixed[i] {
                continue;
            }
            let d = h_imp[i] - h_exp[i];
            let c = h_exp[i] - h0[i];
            sq_diff += d * d;
            sq_change += c * c;
        }
        (sq_diff / sq_change.max(1e-24)).sqrt()
    }

    // 1) 64-node 1-D stream profile: dt_imp = 50 yr (20 steps, Courant ~ 0.26) vs dt_exp = 0.5 yr (2,000 steps).
    let n = 64usize;
    let dx = 10.0_f64;
    let k_test = 1.0e-3_f64;
    let u_val = 1.0e-3_f64;
    let (rec, df, order, fixed) = make_1d_stream_dag(n);
    let uplift = vec![u_val; n];
    let k = vec![k_test; n];
    let p = vec![1.0_f64; n];
    let e = vec![0.0_f64; n];
    let dag = FluvialDagView {
        width: n,
        height: 1,
        d8_receiver: &rec,
        receiver_dist_factor: &df,
        order_downstream_to_upstream: &order,
        fixed_outlet: &fixed,
        uplift_m_yr: &uplift,
        k_erodibility: &k,
        precip_m_yr: &p,
        pet_m_yr: &e,
    };

    let h0_1d: Vec<f64> = (0..n).map(|i| i as f64 * 12.0).collect();
    let mut h_exp = h0_1d.clone();
    let exp_params = FluvialParams {
        dx_m: dx,
        dt_yr: 0.5,
        g_deposition: 0.85,
        ..FluvialParams::default()
    };
    for _ in 0..2_000 {
        step_explicit_forward_euler_dag(&mut h_exp, &dag, &exp_params);
    }

    let mut h_imp = h0_1d.clone();
    let imp_params = FluvialParams {
        dt_yr: 50.0,
        ..exp_params.clone()
    };
    for _ in 0..20 {
        let st = step_fastscape_sd_dag(&mut h_imp, &dag, &imp_params).expect("1D transient step");
        assert!(st.converged);
    }
    let rel_err_1d = relative_transient_rms(&h0_1d, &h_exp, &h_imp, &fixed);
    assert!(
        rel_err_1d <= 0.02,
        "1-D transient relative RMS = {rel_err_1d:.4} (> 0.02)"
    );

    // Negative control: skipping GS iteration fails the 1-D transient error check.
    let mut h_bad = h0_1d.clone();
    let bad_params = FluvialParams {
        solver_mode: FluvialSolverMode::SinglePassNoIterationNegativeControl,
        ..imp_params
    };
    for _ in 0..20 {
        let _ = step_fastscape_sd_dag(&mut h_bad, &dag, &bad_params).expect("1D neg ctrl");
    }
    let rel_err_bad = relative_transient_rms(&h0_1d, &h_exp, &h_bad, &fixed);
    assert!(
        rel_err_bad > 0.05,
        "negative control unexpectedly passed transient RMS: {rel_err_bad:.4}"
    );

    // 2) 32x32 tilted mini-grid (`Courant <= 0.45` at `dt_imp = 25 yr`, 40 steps vs 2,000 explicit steps).
    let w = 32usize;
    let m = w * w;
    let mut h0_2d_f32 = vec![0.0_f32; m];
    for r in 0..w {
        for c in 0..w {
            let edge_dist = (r.min(w - 1 - r)).min(c.min(w - 1 - c)) as f32;
            h0_2d_f32[r * w + c] = edge_dist * 35.0;
        }
    }
    let pf = priority_flood_d8(&h0_2d_f32, w, w, None);
    let fixed_2d: Vec<bool> = pf.d8_receiver.iter().map(|&r| r == NO_RECEIVER).collect();
    let uplift_2d = vec![1.0e-3_f64; m];
    let k_2d = vec![1.0e-3_f64; m];
    let p_2d = vec![1.0_f64; m];
    let e_2d = vec![0.0_f64; m];
    let dag_2d = FluvialDagView {
        width: w,
        height: w,
        d8_receiver: &pf.d8_receiver,
        receiver_dist_factor: &pf.receiver_dist_factor,
        order_downstream_to_upstream: &pf.order_downstream_to_upstream,
        fixed_outlet: &fixed_2d,
        uplift_m_yr: &uplift_2d,
        k_erodibility: &k_2d,
        precip_m_yr: &p_2d,
        pet_m_yr: &e_2d,
    };
    let h0_2d: Vec<f64> = h0_2d_f32.iter().map(|&v| f64::from(v)).collect();
    let mut h_exp_2d = h0_2d.clone();
    for _ in 0..2_000 {
        step_explicit_forward_euler_dag(&mut h_exp_2d, &dag_2d, &exp_params);
    }
    let mut h_imp_2d = h0_2d.clone();
    let imp_2d_params = FluvialParams {
        dt_yr: 25.0,
        ..exp_params
    };
    for _ in 0..40 {
        let st = step_fastscape_sd_dag(&mut h_imp_2d, &dag_2d, &imp_2d_params)
            .expect("2D transient step");
        assert!(st.converged);
    }
    let rel_err_2d = relative_transient_rms(&h0_2d, &h_exp_2d, &h_imp_2d, &fixed_2d);
    assert!(
        rel_err_2d <= 0.02,
        "32x32 mini-grid transient relative RMS = {rel_err_2d:.4} (> 0.02)"
    );

    // Assert NoLocalTerm negative control also fails the 2-D transient (> 5%; observed ~14.1%).
    let mut h_no_local_2d = h0_2d.clone();
    let no_local_2d_params = FluvialParams {
        solver_mode: FluvialSolverMode::NoLocalTermNegativeControl,
        ..imp_2d_params
    };
    for _ in 0..40 {
        let st = step_fastscape_sd_dag(&mut h_no_local_2d, &dag_2d, &no_local_2d_params)
            .expect("2D NoLocal step");
        assert!(st.converged);
    }
    let rel_err_no_local_2d = relative_transient_rms(&h0_2d, &h_exp_2d, &h_no_local_2d, &fixed_2d);
    assert!(
        rel_err_no_local_2d > 0.05,
        "NoLocalTerm unexpectedly passed 2-D transient RMS: {rel_err_no_local_2d:.4}"
    );
    eprintln!(
        "WV2-1b transient norms: 1D={rel_err_1d:.4} 1D_SinglePass={rel_err_bad:.4} 2D={rel_err_2d:.4} 2D_NoLocal={rel_err_no_local_2d:.4}"
    );
}

#[test]
fn fastscape_arid_rain_shadow_positivity_and_unclamped_runoff_neg_control() {
    // Rain-shadow basin: P = 0.20 m/yr < PET = 1.40 m/yr (`max(0, P - PET) = 0`),
    // while precipitation weighting keeps `nu = 0.20 / 1.0 = 0.20 > 0`.
    let w = 32usize;
    let m = w * w;
    let mut elev_f32 = vec![0.0_f32; m];
    for r in 0..w {
        for c in 0..w {
            let border_dist = (r.min(w - 1 - r)).min(c.min(w - 1 - c)) as f32;
            elev_f32[r * w + c] = border_dist * 45.0;
        }
    }
    let pf = priority_flood_d8(&elev_f32, w, w, None);
    let fixed: Vec<bool> = pf.d8_receiver.iter().map(|&r| r == NO_RECEIVER).collect();
    let uplift = vec![0.0_f64; m];
    let k = vec![4.0e-5_f64; m];
    let p_arid_f64 = vec![0.20_f64; m];
    let pet_arid_f64 = vec![1.40_f64; m];
    let dag = FluvialDagView {
        width: w,
        height: w,
        d8_receiver: &pf.d8_receiver,
        receiver_dist_factor: &pf.receiver_dist_factor,
        order_downstream_to_upstream: &pf.order_downstream_to_upstream,
        fixed_outlet: &fixed,
        uplift_m_yr: &uplift,
        k_erodibility: &k,
        precip_m_yr: &p_arid_f64,
        pet_m_yr: &pet_arid_f64,
    };

    let h0: Vec<f64> = pf.filled_h.iter().map(|&v| f64::from(v)).collect();
    let mut h_precip = h0.clone();
    let params = FluvialParams {
        dt_yr: 10_000.0,
        g_deposition: 0.85,
        min_precip_m_yr: 0.15,
        area_weight_mode: AreaWeightMode::Precipitation,
        ..FluvialParams::default()
    };
    let mut max_beta_seen = 0.0_f64;
    for _ in 0..10 {
        let st = step_fastscape_sd_dag(&mut h_precip, &dag, &params).expect("arid basin converges");
        assert!(st.converged);
        assert_eq!(st.non_finite_count, 0);
        max_beta_seen = max_beta_seen.max(st.max_beta);
    }
    assert!(
        max_beta_seen <= params.g_deposition / (params.min_precip_m_yr / params.nu_ref_m_yr) + 1e-9,
        "beta exceeded bound: {max_beta_seen}"
    );
    let max_incision = h0
        .iter()
        .zip(h_precip.iter())
        .map(|(&a, &b)| a - b)
        .fold(0.0_f64, f64::max);
    assert!(
        max_incision > 0.5,
        "arid rain-shadow wadi failed to incise cleanly: max_incision = {max_incision}"
    );

    // Hydraulic discharge independently sees zero surface runoff in the same arid basin.
    let p_arid_f32 = vec![0.20_f32; m];
    let pet_arid_f32 = vec![1.40_f32; m];
    let hydro = route_hybrid_mfd_d8(
        &pf.filled_h,
        &pf.d8_receiver,
        w,
        w,
        &p_arid_f32,
        &pet_arid_f32,
        &HydrologyConfig {
            sea_level_m: None,
            ..HydrologyConfig::default()
        },
    );
    assert!(hydro.discharge_q_m3_s.iter().all(|&q| q == 0.0));
    assert!(hydro.precip_weighted_area_m2.iter().all(|&a| a >= 15.0));

    // Negative control (b): unclamped runoff `max(0, P - PET)` weighting triggers singularity & loud error!
    let mut h_bad = h0;
    let params_bad = FluvialParams {
        area_weight_mode: AreaWeightMode::UnclampedRunoffNegativeControl,
        ..params
    };
    let res_bad = step_fastscape_sd_dag(&mut h_bad, &dag, &params_bad);
    assert!(
        res_bad.is_err(),
        "negative control (UnclampedRunoff) failed to reject β=inf singularity loudly: {res_bad:?}"
    );
}

#[test]
fn thermal_hillslope_mass_conservation_and_spike_elimination() {
    let w = 48usize;
    let h = 48usize;
    let m = w * h;
    let mut bedrock_h = vec![100.0_f32; m];
    let mut sediment_s = vec![2.0_f32; m];
    let mut talus_frac = vec![0.0_f32; m];
    let lithology_id = vec![4_u8; m]; // SoftShale (`34 deg` repose)

    // Inject needle spikes far steeper than 75 deg (`dz = 900 m` over `10 m` -> `89.3 deg`).
    bedrock_h[16 * w + 16] += 900.0;
    bedrock_h[30 * w + 28] += 650.0;

    let tan_75 = deg_to_slope(75.0);
    let params = ThermalHillslopeParams {
        dx_m: 10.0,
        dt_yr: 100.0,
        diffusivity_d: 0.05,
        talus_rate: 0.65,
        substeps: 40,
        ..ThermalHillslopeParams::default()
    };
    let stats = relax_thermal_and_hillslope(
        &mut bedrock_h,
        &mut sediment_s,
        &mut talus_frac,
        &lithology_id,
        w,
        h,
        &params,
    );

    assert!(
        stats.rel_mass_error < 1e-6,
        "relative mass conservation drift = {:e} (before={}, after={})",
        stats.rel_mass_error,
        stats.mass_before,
        stats.mass_after
    );
    assert_eq!(stats.non_cliff_spikes_over_75_deg, 0);
    assert!(
        deg_to_slope(stats.max_slope_deg_after) <= tan_75,
        "residual spike {} deg > 75 deg",
        stats.max_slope_deg_after
    );
}

#[test]
fn priority_flood_zero_pit_invariant_and_unfilled_neg_control() {
    let w = 48usize;
    let h = 48usize;
    let m = w * h;
    let mut elev = vec![0.0_f32; m];
    for r in 0..h {
        for c in 0..w {
            let dr = r as f32 - 24.0;
            let dc = c as f32 - 24.0;
            let dist = (dr * dr + dc * dc).sqrt();
            // Sloped massif punctured by multiple interior craters.
            let crater = -180.0
                * libm::expf(-((r as f32 - 18.0).sq() + (c as f32 - 20.0).sq()) / 9.0)
                - 140.0
                    * libm::expf(
                        -((r as f32 - 30.0) * (r as f32 - 30.0)
                            + (c as f32 - 29.0) * (c as f32 - 29.0))
                            / 9.0,
                    );
            elev[r * w + c] = 500.0 - dist * 15.0 + crater;
        }
    }

    let raw_pits = count_pits(&elev, w, h, None);
    assert!(
        raw_pits > 0,
        "negative control: cratered DEM must have interior pits before Flood"
    );

    let pf = priority_flood_d8(&elev, w, h, None);
    assert_eq!(pf.pit_count_before, raw_pits);
    assert_eq!(
        pf.pit_count_after, 0,
        "post-Priority-Flood DEM must have zero pits"
    );
    assert_eq!(pf.order_downstream_to_upstream.len(), m);
}

// Note: avoid any `.powi` / `.powf` helper name confusion with clippy.toml inside tests too!
trait SqExt {
    fn sq(self) -> f32;
}
impl SqExt for f32 {
    fn sq(self) -> f32 {
        self * self
    }
}

#[test]
fn priority_flood_handles_seed_cratered_s1_dem() {
    let field = generate_uplift(&WorldV2Config::for_size(77, 128)).expect("S1 field");
    let mut cratered = field.bedrock_h;
    // Plant artificial interior closed crater on dry massif shoulder (> 500 m).
    let idx = 64 * 128 + 80;
    cratered[idx] -= 400.0;
    assert!(count_pits(&cratered, 128, 128, Some(0.0)) > 0);
    let pf = priority_flood_d8(&cratered, 128, 128, Some(0.0));
    assert_eq!(pf.pit_count_after, 0);
}

#[test]
fn hybrid_mfd_d8_hillslope_isotropy_and_pure_d8_neg_control() {
    let n = 65usize;
    let m = n * n;
    let mut cone = vec![0.0_f32; m];
    let center = 32.0_f32;
    for r in 0..n {
        for c in 0..n {
            let dr = r as f32 - center;
            let dc = c as f32 - center;
            let dist = (dr * dr + dc * dc).sqrt() * 10.0;
            cone[r * n + c] = (400.0 - dist).max(0.0);
        }
    }
    let pf = priority_flood_d8(&cone, n, n, None);
    let precip = vec![1.5_f32; m];
    let pet = vec![0.5_f32; m];

    let ring_cv = |flow_acc: &[f64]| -> f64 {
        let mut vals = Vec::new();
        for r in 0..n {
            for c in 0..n {
                let dr = r as f64 - 32.0;
                let dc = c as f64 - 32.0;
                let rad = (dr * dr + dc * dc).sqrt();
                if (14.5..=16.5).contains(&rad) {
                    vals.push(flow_acc[r * n + c]);
                }
            }
        }
        let mean = vals.iter().sum::<f64>() / vals.len() as f64;
        let var = vals.iter().map(|&v| (v - mean) * (v - mean)).sum::<f64>() / vals.len() as f64;
        var.sqrt() / mean
    };

    let mfd = route_hybrid_mfd_d8(
        &pf.filled_h,
        &pf.d8_receiver,
        n,
        n,
        &precip,
        &pet,
        &HydrologyConfig {
            channel_init_threshold: 1.0e9,
            force_pure_d8: false,
            sea_level_m: None,
            ..HydrologyConfig::default()
        },
    );
    let cv_mfd = ring_cv(&mfd.flow_acc_m2);
    assert!(
        cv_mfd <= 0.15,
        "Hybrid MFD p=1.6 ring CV = {cv_mfd:.4} (> 0.15)"
    );

    // Negative control (d): pure D8 routing on hillslope cone creates strong cardinal/diagonal spokes (`CV > 0.50`).
    let d8 = route_hybrid_mfd_d8(
        &pf.filled_h,
        &pf.d8_receiver,
        n,
        n,
        &precip,
        &pet,
        &HydrologyConfig {
            force_pure_d8: true,
            sea_level_m: None,
            ..HydrologyConfig::default()
        },
    );
    let cv_d8 = ring_cv(&d8.flow_acc_m2);
    assert!(
        cv_d8 > 0.50,
        "negative control (force_pure_d8) did not spike ring anisotropy: CV = {cv_d8:.4}"
    );
}

#[test]
fn single_vs_multi_thread_rayon_invariance_s2_s3_s5() {
    let run_pipeline = |threads: usize| -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let pool = ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        pool.install(|| {
            let cfg = WorldV2Config::for_size(2026_1120, 64);
            let mut s1 = generate_uplift(&cfg).unwrap();
            let n = 64 * 64;
            let mut sediment = vec![0.0_f32; n];
            let mut talus = vec![0.0_f32; n];
            let k_vec: Vec<f32> = s1.stratum_k.iter().map(|&k| k * 3e-5).collect();
            let uplift: Vec<f32> = s1.uplift_u.iter().map(|&u| u * 1e-6).collect();
            let precip = vec![2.0_f32; n];
            let pet = vec![1.0_f32; n];

            let mut grid = FluvialGridStep {
                bedrock_h: &mut s1.bedrock_h,
                sediment_s: &mut sediment,
                uplift_m_yr: &uplift,
                k_erodibility: &k_vec,
                precip_m_yr: &precip,
                pet_m_yr: &pet,
                width: 64,
                height: 64,
                sea_level_m: Some(0.0),
            };
            let fs_stats = erode_fluvial_step_f32(
                &mut grid,
                &FluvialParams {
                    dx_m: 160.0,
                    dt_yr: 1_000.0,
                    g_deposition: 0.85,
                    ..FluvialParams::default()
                },
            )
            .expect("64x64 S1 fluvial step converges");
            assert!(fs_stats.converged);
            let _ = relax_thermal_and_hillslope(
                &mut s1.bedrock_h,
                &mut sediment,
                &mut talus,
                &s1.lithology_id,
                64,
                64,
                &ThermalHillslopeParams::default(),
            );
            let pf = priority_flood_d8(&s1.bedrock_h, 64, 64, Some(0.0));
            let hydro = route_hybrid_mfd_d8(
                &pf.filled_h,
                &pf.d8_receiver,
                64,
                64,
                &precip,
                &pet,
                &HydrologyConfig::default(),
            );
            (s1.bedrock_h, sediment, hydro.water_h_m)
        })
    };

    let (b1, s1, w1) = run_pipeline(1);
    let (b4, s4, w4) = run_pipeline(4);
    assert_eq!(b1.len(), b4.len());
    for ((a, b), ((c, d), (e, f))) in b1
        .iter()
        .zip(b4.iter())
        .zip(s1.iter().zip(s4.iter()).zip(w1.iter().zip(w4.iter())))
    {
        assert_eq!(a.to_bits(), b.to_bits());
        assert_eq!(c.to_bits(), d.to_bits());
        assert_eq!(e.to_bits(), f.to_bits());
    }
}

#[test]
fn single_step_4096_timing_or_64_smoke() {
    let size = if std::env::var_os("ANIMA_WV2_BENCH_4096").is_some() {
        4096_u32
    } else {
        64_u32
    };
    let n = (size as usize) * (size as usize);
    let cfg = WorldV2Config::for_size(0x5756_3201, size);
    let t_s1 = Instant::now();
    let s1 = generate_uplift(&cfg).expect("uplift");
    let dt_s1 = t_s1.elapsed();

    let t_pf = Instant::now();
    let pf = priority_flood_d8(&s1.bedrock_h, size as usize, size as usize, Some(0.0));
    let dt_pf = t_pf.elapsed();
    assert_eq!(pf.pit_count_after, 0);

    let mut h_f64: Vec<f64> = pf.filled_h.iter().map(|&v| f64::from(v)).collect();
    let fixed: Vec<bool> = pf
        .d8_receiver
        .iter()
        .enumerate()
        .map(|(i, &r)| r == NO_RECEIVER || pf.filled_h[i] <= 0.0)
        .collect();
    let uplift_f64: Vec<f64> = s1.uplift_u.iter().map(|&u| f64::from(u) * 1e-6).collect();
    let k_f64: Vec<f64> = s1.stratum_k.iter().map(|&k| f64::from(k) * 2e-5).collect();
    let precip_f64 = vec![2.0_f64; n];
    let pet_f64 = vec![1.0_f64; n];
    let dag = FluvialDagView {
        width: size as usize,
        height: size as usize,
        d8_receiver: &pf.d8_receiver,
        receiver_dist_factor: &pf.receiver_dist_factor,
        order_downstream_to_upstream: &pf.order_downstream_to_upstream,
        fixed_outlet: &fixed,
        uplift_m_yr: &uplift_f64,
        k_erodibility: &k_f64,
        precip_m_yr: &precip_f64,
        pet_m_yr: &pet_f64,
    };

    let t_fs = Instant::now();
    let stats = step_fastscape_sd_dag(
        &mut h_f64,
        &dag,
        &FluvialParams {
            dx_m: f64::from(cfg.cell_size_m()),
            dt_yr: 1_000.0,
            g_deposition: 0.85,
            ..FluvialParams::default()
        },
    )
    .expect("timing smoke step converges");
    let dt_fs = t_fs.elapsed();
    assert!(stats.converged);
    assert_eq!(stats.non_finite_count, 0);

    eprintln!(
        "WV2-1b timing size={size}x{size}: s1={:.3}s priority_flood_d8={:.3}s fastscape_sd_g085={:.3}s (gs_iters={}, substeps={})",
        dt_s1.as_secs_f64(),
        dt_pf.as_secs_f64(),
        dt_fs.as_secs_f64(),
        stats.gs_iterations,
        stats.substeps
    );
}

#[test]
fn arid_512_and_long_chain_converge_via_adaptive_halving_and_report_failures() {
    // R1a: Realistic 512x512 S1 network (~300-cell channels) in rain shadow (nu = 0.15, PET = 1.0)
    // previously failed silently at the 32-iter cap and blew up to ~1e8 m at large dt.
    let cfg = WorldV2Config::for_size(0x5756_3201, 512);
    let s1 = generate_uplift(&cfg).expect("S1 512x512");
    let w = 512usize;
    let n = w * w;
    let pf = priority_flood_d8(&s1.bedrock_h, w, w, Some(0.0));
    let fixed: Vec<bool> = pf
        .d8_receiver
        .iter()
        .enumerate()
        .map(|(i, &r)| r == NO_RECEIVER || pf.filled_h[i] <= 0.0)
        .collect();
    let uplift: Vec<f64> = s1.uplift_u.iter().map(|&u| f64::from(u) * 1e-6).collect();
    let k_erod: Vec<f64> = s1.stratum_k.iter().map(|&k| f64::from(k) * 2e-5).collect();
    let precip = vec![0.15_f64; n];
    let pet = vec![1.0_f64; n];
    let dag = FluvialDagView {
        width: w,
        height: w,
        d8_receiver: &pf.d8_receiver,
        receiver_dist_factor: &pf.receiver_dist_factor,
        order_downstream_to_upstream: &pf.order_downstream_to_upstream,
        fixed_outlet: &fixed,
        uplift_m_yr: &uplift,
        k_erodibility: &k_erod,
        precip_m_yr: &precip,
        pet_m_yr: &pet,
    };

    for &dt in &[1_000.0_f64, 10_000.0_f64, 100_000.0_f64] {
        let h0: Vec<f64> = pf.filled_h.iter().map(|&v| f64::from(v)).collect();
        let mut h = h0.clone();
        let st = step_fastscape_sd_dag(
            &mut h,
            &dag,
            &FluvialParams {
                dx_m: f64::from(cfg.cell_size_m()),
                dt_yr: dt,
                g_deposition: 0.85,
                min_precip_m_yr: 0.15,
                ..FluvialParams::default()
            },
        )
        .expect("arid 512x512 must converge via adaptive dt halving");
        assert!(st.converged);
        assert_eq!(st.non_finite_count, 0);
        if dt >= 100_000.0 {
            assert!(
                st.substeps > 1,
                "expected adaptive dt halving at large dt={dt}, got substeps={}",
                st.substeps
            );
        }
        let max_dh = h
            .iter()
            .zip(h0.iter())
            .map(|(&a, &b)| libm::fabs(a - b))
            .fold(0.0_f64, f64::max);
        let max_allowed_dh = 50.0 + 0.02 * dt;
        assert!(
            max_dh < max_allowed_dh,
            "arid 512x512 dt={dt} exceeded bound {max_allowed_dh}: max_dh={max_dh:.3e}"
        );
    }

    // R1b: 300-node arid chain (nu = 0.15) converges at large dt via halving; also check steady state on nu = 0.2.
    let chain_len = 300usize;
    let (rec, df, order, fx) = make_1d_stream_dag(chain_len);
    let up_c = vec![5e-4_f64; chain_len];
    let k_c = vec![2e-5_f64; chain_len];
    let p_c = vec![0.15_f64; chain_len];
    let pet_c = vec![1.0_f64; chain_len];
    let dag_c = FluvialDagView {
        width: chain_len,
        height: 1,
        d8_receiver: &rec,
        receiver_dist_factor: &df,
        order_downstream_to_upstream: &order,
        fixed_outlet: &fx,
        uplift_m_yr: &up_c,
        k_erodibility: &k_c,
        precip_m_yr: &p_c,
        pet_m_yr: &pet_c,
    };
    let mut h_c: Vec<f64> = (0..chain_len).map(|i| i as f64 * 2.0).collect();
    let st_c = step_fastscape_sd_dag(
        &mut h_c,
        &dag_c,
        &FluvialParams {
            dt_yr: 10_000.0,
            g_deposition: 0.85,
            ..FluvialParams::default()
        },
    )
    .expect("300-cell arid chain converges");
    assert!(st_c.converged);
    assert_eq!(st_c.non_finite_count, 0);

    // R1c: When max_dt_halvings = 0 permits no retries and cap is insufficient, return explicit error
    // naming cell, last delta, and dt without modifying input h.
    let h_saved = h_c.clone();
    let err = step_fastscape_sd_dag(
        &mut h_c,
        &dag_c,
        &FluvialParams {
            dt_yr: 100_000.0,
            g_deposition: 0.85,
            max_gs_iters: 4,
            max_dt_halvings: 0,
            ..FluvialParams::default()
        },
    )
    .expect_err("expected explicit NonConvergence error");
    assert_eq!(h_c, h_saved, "failed step must leave input h unmodified");
    match err {
        FluvialError::NonConvergence {
            last_delta_m,
            dt_yr,
            ..
        } => {
            assert!(!last_delta_m.is_finite() || last_delta_m >= 1e-6);
            assert!((dt_yr - 100_000.0).abs() < 1e-6);
            let msg = err.to_string();
            assert!(
                msg.contains("cell ")
                    && msg.contains("last_delta_m=")
                    && msg.contains("sub_dt_yr=")
            );
        }
        other => panic!("unexpected error variant: {other:?}"),
    }

    // R1d: Single-attempt GS without halving at nu = 1.0, dt = 10 kyr converges cleanly (~22 iters)
    // under in-place upstream-first GS and catches wrong-order sweep mutations (M2a).
    let n64 = 64usize;
    let (r64, df64, o64, f64x) = make_1d_stream_dag(n64);
    let up64 = vec![5e-4_f64; n64];
    let k64 = vec![2e-5_f64; n64];
    let p64 = vec![1.0_f64; n64];
    let e64 = vec![0.0_f64; n64];
    let d64 = FluvialDagView {
        width: n64,
        height: 1,
        d8_receiver: &r64,
        receiver_dist_factor: &df64,
        order_downstream_to_upstream: &o64,
        fixed_outlet: &f64x,
        uplift_m_yr: &up64,
        k_erodibility: &k64,
        precip_m_yr: &p64,
        pet_m_yr: &e64,
    };
    let mut h64: Vec<f64> = (0..n64).map(|i| i as f64 * 0.5).collect();
    let st64 = step_fastscape_sd_dag(
        &mut h64,
        &d64,
        &FluvialParams {
            dx_m: 10.0,
            dt_yr: 10_000.0,
            gs_tol_m: 1e-6,
            max_gs_iters: 32,
            max_dt_halvings: 0,
            ..FluvialParams::default()
        },
    )
    .expect("in-place upstream-first GS converges without halving at nu=1.0 dt=10kyr");
    assert!(st64.converged && st64.substeps == 1 && st64.max_gs_delta_m < 1e-6);

    let mut h64_large: Vec<f64> = (0..n64).map(|i| i as f64 * 0.5).collect();
    let st64_large = step_fastscape_sd_dag(
        &mut h64_large,
        &d64,
        &FluvialParams {
            dx_m: 10.0,
            dt_yr: 100_000.0,
            gs_tol_m: 1e-6,
            ..FluvialParams::default()
        },
    )
    .expect("large dt=100kyr converges with adaptive halving");
    assert!(st64_large.converged && st64_large.substeps > 1);
}

#[test]
fn steady_state_variable_precip_with_pet_pins_precipitation_weighting() {
    // R2: Spatially varying P (0.15..3.15 m/yr) with PET = 1.4 m/yr (most cells arid or runoff != P).
    // Analytic steady state is S* = U * (1 + G * A / A_tilde) / (K * A_tilde^m), computed independently here.
    let n = 64usize;
    let dx = 10.0_f64;
    let (u, k, m, g) = (5e-4_f64, 2e-5_f64, 0.45_f64, 0.85_f64);
    let (rec, df, order, fixed) = make_1d_stream_dag(n);
    let precip: Vec<f64> = (0..n)
        .map(|i| 0.15 + 3.0 * ((i * 7919) % 13) as f64 / 13.0)
        .collect();
    let pet = vec![1.4_f64; n];
    let up = vec![u; n];
    let kk = vec![k; n];
    let dag = FluvialDagView {
        width: n,
        height: 1,
        d8_receiver: &rec,
        receiver_dist_factor: &df,
        order_downstream_to_upstream: &order,
        fixed_outlet: &fixed,
        uplift_m_yr: &up,
        k_erodibility: &kk,
        precip_m_yr: &precip,
        pet_m_yr: &pet,
    };
    let params = FluvialParams {
        dx_m: dx,
        dt_yr: 100_000.0,
        m,
        n: 1.0,
        g_deposition: g,
        min_precip_m_yr: 0.15,
        gs_tol_m: 1e-8,
        ..FluvialParams::default()
    };
    let mut h: Vec<f64> = (0..n).map(|i| i as f64 * 0.5).collect();
    for _ in 0..400 {
        let st = step_fastscape_sd_dag(&mut h, &dag, &params).expect("variable-P steady converges");
        assert!(st.converged);
    }

    let mut max_rel = 0.0_f64;
    for i in 1..n {
        let a_geo = (n - i) as f64 * dx * dx;
        let a_tilde_indep: f64 = (i..n).map(|idx| precip[idx].max(0.15) * dx * dx).sum();
        let s_star = u * (1.0 + g * a_geo / a_tilde_indep) / (k * libm::pow(a_tilde_indep, m));
        let s_meas = (h[i] - h[i - 1]) / dx;
        max_rel = max_rel.max(libm::fabs(s_meas - s_star) / s_star);
    }
    assert!(
        max_rel < 1e-3,
        "variable-P + PET analytic steady-state error {max_rel:.3e} >= 1e-3"
    );
}

#[test]
fn g_zero_short_transient_matches_explicit_reference() {
    // R3: Pins G = 0 downstream-to-upstream sweep order against explicit reference (~0.77%).
    let n = 64usize;
    let (rec, df, order, fixed) = make_1d_stream_dag(n);
    let up = vec![1e-3_f64; n];
    let kk = vec![1e-3_f64; n];
    let p = vec![1.0_f64; n];
    let e = vec![0.0_f64; n];
    let dag = FluvialDagView {
        width: n,
        height: 1,
        d8_receiver: &rec,
        receiver_dist_factor: &df,
        order_downstream_to_upstream: &order,
        fixed_outlet: &fixed,
        uplift_m_yr: &up,
        k_erodibility: &kk,
        precip_m_yr: &p,
        pet_m_yr: &e,
    };
    let h0: Vec<f64> = (0..n).map(|i| i as f64 * 12.0).collect();
    let mut h_exp = h0.clone();
    let p_exp = FluvialParams {
        dx_m: 10.0,
        dt_yr: 0.5,
        g_deposition: 0.0,
        ..FluvialParams::default()
    };
    for _ in 0..2_000 {
        step_explicit_forward_euler_dag(&mut h_exp, &dag, &p_exp);
    }
    let mut h_imp = h0.clone();
    let p_imp = FluvialParams {
        dt_yr: 50.0,
        ..p_exp
    };
    for _ in 0..20 {
        let st = step_fastscape_sd_dag(&mut h_imp, &dag, &p_imp).expect("G=0 transient converges");
        assert!(st.converged);
    }
    let mut sq_diff = 0.0_f64;
    let mut sq_chg = 0.0_f64;
    for i in 1..n {
        let d = h_imp[i] - h_exp[i];
        let c = h_exp[i] - h0[i];
        sq_diff += d * d;
        sq_chg += c * c;
    }
    let rel = (sq_diff / sq_chg.max(1e-24)).sqrt();
    assert!(
        rel <= 0.02,
        "G=0 short transient relative RMS = {rel:.4} (> 0.02)"
    );
}

#[test]
fn steady_state_2d_tributaries_and_diagonal_receivers() {
    // R4: 2-D jittered pyramid with tributary junctions (donors >= 2) and diagonal receivers,
    // verified against an independently accumulated drainage area.
    let w = 40usize;
    let nn = w * w;
    let mut z = vec![0.0_f32; nn];
    for r in 0..w {
        for c in 0..w {
            let ed = (r.min(w - 1 - r)).min(c.min(w - 1 - c)) as f32;
            let jitter = (((r * 131 + c * 197) * 2_654_435_761_usize) % 1_000) as f32 / 1_000.0;
            z[r * w + c] = ed * 20.0 + jitter * 15.0;
        }
    }
    let pf = priority_flood_d8(&z, w, w, None);
    let fixed: Vec<bool> = pf.d8_receiver.iter().map(|&r| r == NO_RECEIVER).collect();
    let dx = 10.0_f64;
    let (u, k, m, g) = (5e-4_f64, 2e-5_f64, 0.45_f64, 0.85_f64);
    let up = vec![u; nn];
    let kk = vec![k; nn];
    let p = vec![1.0_f64; nn];
    let e = vec![0.0_f64; nn];
    let dag = FluvialDagView {
        width: w,
        height: w,
        d8_receiver: &pf.d8_receiver,
        receiver_dist_factor: &pf.receiver_dist_factor,
        order_downstream_to_upstream: &pf.order_downstream_to_upstream,
        fixed_outlet: &fixed,
        uplift_m_yr: &up,
        k_erodibility: &kk,
        precip_m_yr: &p,
        pet_m_yr: &e,
    };
    let params = FluvialParams {
        dx_m: dx,
        dt_yr: 100_000.0,
        m,
        g_deposition: g,
        gs_tol_m: 1e-8,
        ..FluvialParams::default()
    };
    let mut h: Vec<f64> = pf.filled_h.iter().map(|&v| f64::from(v)).collect();
    for _ in 0..300 {
        let st =
            step_fastscape_sd_dag(&mut h, &dag, &params).expect("2D tributary steady converges");
        assert!(st.converged);
    }

    let mut indep_area = vec![0.0_f64; nn];
    let mut donors = vec![0usize; nn];
    let mut diag_count = 0usize;
    for s in 0..nn {
        let mut cur = s;
        if pf.d8_receiver[s] != NO_RECEIVER {
            let rc = pf.d8_receiver[s] as usize;
            donors[rc] += 1;
            if rc / w != s / w && rc % w != s % w {
                diag_count += 1;
            }
        }
        loop {
            indep_area[cur] += dx * dx;
            let r = pf.d8_receiver[cur];
            if r == NO_RECEIVER {
                break;
            }
            cur = r as usize;
        }
    }
    assert!(donors.iter().copied().max().unwrap_or(0) >= 2);
    assert!(diag_count > 0);

    let mut max_rel = 0.0_f64;
    for i in 0..nn {
        if fixed[i] {
            continue;
        }
        let r = pf.d8_receiver[i] as usize;
        let d = dx * f64::from(pf.receiver_dist_factor[i]);
        let s = (h[i] - h[r]) / d;
        let s_star = (1.0 + g) * u / (k * libm::pow(indep_area[i], m));
        max_rel = max_rel.max(libm::fabs(s - s_star) / s_star);
    }
    assert!(
        max_rel < 1e-2,
        "2D tributary steady-state error {max_rel:.3e} >= 1e-2"
    );
}

#[test]
fn thermal_talus_relaxes_cliff_to_lithology_repose_angle() {
    // R5: With creep D = 0, a 100 m vertical cliff relaxes via talus down to the lithology repose angle
    // (~34 deg SoftShale vs ~58 deg GraniteCrystalline), catching disabled talus (M12) or broken repose (M13).
    let w = 40usize;
    let mut measured_angles = [0.0_f32; 2];
    for (idx, &(lith, lo, hi)) in [(4_u8, 30.0_f32, 39.0_f32), (0_u8, 53.0_f32, 62.0_f32)]
        .iter()
        .enumerate()
    {
        let mut b = vec![0.0_f32; w * w];
        for r in 0..w {
            for c in 20..w {
                b[r * w + c] = 100.0;
            }
        }
        let mut s = vec![0.0_f32; w * w];
        let mut t = vec![0.0_f32; w * w];
        let li = vec![lith; w * w];
        let st = relax_thermal_and_hillslope(
            &mut b,
            &mut s,
            &mut t,
            &li,
            w,
            w,
            &ThermalHillslopeParams {
                dx_m: 10.0,
                diffusivity_d: 0.0,
                talus_rate: 0.65,
                substeps: 600,
                ..ThermalHillslopeParams::default()
            },
        );
        let row = 20;
        let mut max_card = 0.0_f32;
        for c in 0..(w - 1) {
            let ha = b[row * w + c] + s[row * w + c];
            let hb = b[row * w + c + 1] + s[row * w + c + 1];
            max_card = max_card.max(slope_to_deg((hb - ha).abs() / 10.0));
        }
        assert!(
            (lo..=hi).contains(&max_card),
            "lith={lith}: relaxed cliff cardinal angle {max_card:.2} outside [{lo}, {hi}] deg"
        );
        assert!(st.rel_mass_error < 1e-6);
        measured_angles[idx] = max_card;
    }
    assert!(
        measured_angles[1] > measured_angles[0] + 15.0,
        "granite ({:.2}) must stand > 15 deg steeper than shale ({:.2})",
        measured_angles[1],
        measured_angles[0]
    );
}

#[test]
fn priority_flood_minimality_and_steepest_d8_receivers() {
    // R6: Independent exact depression fill comparison (never < input, within eps-accumulation bound)
    // plus steepest-descent D8 receiver check on a jittered cone.
    let w = 128usize;
    let cfg = WorldV2Config::for_size(77, w as u32);
    let field = generate_uplift(&cfg).expect("S1 128");
    let mut dem = field.bedrock_h;
    dem[64 * w + 80] -= 400.0;

    let pf = priority_flood_d8(&dem, w, w, Some(0.0));
    assert_eq!(pf.pit_count_after, 0);

    // Independent exact fill (no epsilon): iterative fixed point W = max(dem, min_nb(W)).
    let n = w * w;
    let mut exact = vec![f32::INFINITY; n];
    for r in 0..w {
        for c in 0..w {
            let i = r * w + c;
            if r == 0 || c == 0 || r + 1 == w || c + 1 == w || dem[i] <= 0.0 {
                exact[i] = dem[i];
            }
        }
    }
    loop {
        let mut changed = false;
        for r in 1..(w - 1) {
            for c in 1..(w - 1) {
                let i = r * w + c;
                if exact[i] == dem[i] {
                    continue;
                }
                let mut mn = f32::INFINITY;
                for dr in -1_isize..=1 {
                    for dc in -1_isize..=1 {
                        if dr == 0 && dc == 0 {
                            continue;
                        }
                        let nb = ((r as isize + dr) as usize) * w + ((c as isize + dc) as usize);
                        mn = mn.min(exact[nb]);
                    }
                }
                let nv = dem[i].max(mn);
                if nv < exact[i] {
                    exact[i] = nv;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    let mut max_eps_diff = 0.0_f32;
    for i in 0..n {
        assert!(
            pf.filled_h[i] >= dem[i],
            "cell {i}: filled_h {} < input {}",
            pf.filled_h[i],
            dem[i]
        );
        max_eps_diff = max_eps_diff.max((pf.filled_h[i] - exact[i]).abs());
    }
    assert!(
        max_eps_diff < 0.05,
        "Priority-Flood exceeded exact fill eps bound: max_diff = {max_eps_diff}"
    );

    // Steepest-descent D8 receiver invariant on jittered 65x65 cone.
    let cn = 65usize;
    let cm = cn * cn;
    let mut cone = vec![0.0_f32; cm];
    for r in 0..cn {
        for c in 0..cn {
            let dr = r as f32 - 32.0;
            let dc = c as f32 - 32.0;
            let jitter = (((r * 7 + c * 13) % 5) as f32) * 0.3;
            cone[r * cn + c] = (400.0 - (dr * dr + dc * dc).sqrt() * 10.0).max(0.0) + jitter;
        }
    }
    let pf_c = priority_flood_d8(&cone, cn, cn, None);
    let mut diag = 0usize;
    for r in 1..(cn - 1) {
        for c in 1..(cn - 1) {
            let i = r * cn + c;
            let rc = pf_c.d8_receiver[i] as usize;
            let dist = if rc / cn != r && rc % cn != c {
                diag += 1;
                std::f64::consts::SQRT_2
            } else {
                1.0
            };
            let s_rec = (f64::from(pf_c.filled_h[i]) - f64::from(pf_c.filled_h[rc])) / dist;
            for dr in -1_isize..=1 {
                for dc in -1_isize..=1 {
                    if dr == 0 && dc == 0 {
                        continue;
                    }
                    let j = ((r as isize + dr) as usize) * cn + ((c as isize + dc) as usize);
                    let d = if dr != 0 && dc != 0 {
                        std::f64::consts::SQRT_2
                    } else {
                        1.0
                    };
                    let s = (f64::from(pf_c.filled_h[i]) - f64::from(pf_c.filled_h[j])) / d;
                    assert!(
                        s <= s_rec + 1e-12,
                        "cell ({r},{c}) D8 receiver slope {s_rec} beaten by neighbor slope {s}"
                    );
                }
            }
        }
    }
    assert!(diag > 0);
}

#[test]
fn hybrid_mfd_d8_switch_and_exponent_p_1_6() {
    // R7a: Finite-threshold cone activates channels and channelized cells send 100% flow to D8 receiver.
    let cn = 65usize;
    let cm = cn * cn;
    let mut cone = vec![0.0_f32; cm];
    for r in 0..cn {
        for c in 0..cn {
            let dr = r as f32 - 32.0;
            let dc = c as f32 - 32.0;
            cone[r * cn + c] = (400.0 - (dr * dr + dc * dc).sqrt() * 10.0).max(0.0);
        }
    }
    let pf_c = priority_flood_d8(&cone, cn, cn, None);
    let p_c = vec![1.5_f32; cm];
    let e_c = vec![0.5_f32; cm];
    let hy_c = route_hybrid_mfd_d8(
        &pf_c.filled_h,
        &pf_c.d8_receiver,
        cn,
        cn,
        &p_c,
        &e_c,
        &HydrologyConfig {
            channel_init_threshold: 250.0,
            sea_level_m: None,
            ..HydrologyConfig::default()
        },
    );
    let ch_count = hy_c.is_channel.iter().filter(|&&c| c).count();
    assert!(ch_count > 0 && ch_count < cm);

    // Exact 3x3 unit cell with 2 downhill cardinal neighbors:
    // Case 1 (above threshold -> channelized): 100% routes to steepest neighbor (1,2), 0% to (2,1).
    let dem_chan = [
        200.0_f32, 200.0, 200.0, //
        200.0, 100.0, 0.0, // center (1,1)=100 -> steepest (1,2)=0 (slope 10.0)
        200.0, 50.0, 200.0, // secondary (2,1)=50 (slope 5.0)
    ];
    let pf_chan = priority_flood_d8(&dem_chan, 3, 3, None);
    let p9 = vec![1.0_f32; 9];
    let e9 = vec![0.0_f32; 9];
    let hy_chan = route_hybrid_mfd_d8(
        &pf_chan.filled_h,
        &pf_chan.d8_receiver,
        3,
        3,
        &p9,
        &e9,
        &HydrologyConfig {
            dx_m: 10.0,
            channel_init_threshold: 250.0,
            sea_level_m: None,
            ..HydrologyConfig::default()
        },
    );
    assert!(hy_chan.is_channel[4]);
    assert!(
        (hy_chan.flow_acc_m2[5] - 200.0).abs() < 1e-6
            && (hy_chan.flow_acc_m2[7] - 100.0).abs() < 1e-6,
        "channelized cell leaked flow off D8 receiver: {:?}",
        hy_chan.flow_acc_m2
    );

    // Case 2 (below threshold -> hillslope MFD): flux split must equal (s1/s2)^1.6 = 2^1.6.
    let dem_mfd = [
        200.0_f32, 200.0, 200.0, //
        200.0, 100.0, 90.0, // (1,2)=90 -> s1 = 1.0
        200.0, 95.0, 200.0, // (2,1)=95 -> s2 = 0.5
    ];
    let pf_mfd = priority_flood_d8(&dem_mfd, 3, 3, None);
    let hy_mfd = route_hybrid_mfd_d8(
        &pf_mfd.filled_h,
        &pf_mfd.d8_receiver,
        3,
        3,
        &p9,
        &e9,
        &HydrologyConfig {
            dx_m: 10.0,
            channel_init_threshold: 1.0e9,
            sea_level_m: None,
            ..HydrologyConfig::default()
        },
    );
    assert!(!hy_mfd.is_channel[4]);
    let inflow_east = hy_mfd.flow_acc_m2[5] - 100.0;
    let inflow_south = hy_mfd.flow_acc_m2[7] - 100.0;
    let ratio = inflow_east / inflow_south;
    let expected = libm::pow(2.0_f64, 1.6_f64);
    assert!(
        libm::fabs(ratio - expected) < 1e-4,
        "MFD p=1.6 split ratio = {ratio:.6}, expected {expected:.6}"
    );
}

#[test]
fn erode_fluvial_step_f32_books_uplift_to_bedrock_and_splits_residual() {
    // R8a: Pure uplift (K = 0, U*dt = 1 m) raises bedrock_h by +1 m and leaves sediment_s = 0.
    let w = 16usize;
    let nn = w * w;
    let mut b = vec![0.0_f32; nn];
    for r in 0..w {
        for c in 0..w {
            b[r * w + c] = 10.0 + ((r.min(w - 1 - r)).min(c.min(w - 1 - c)) as f32) * 5.0;
        }
    }
    let b0 = b.clone();
    let mut sed = vec![0.0_f32; nn];
    let up = vec![1e-3_f32; nn];
    let k0 = vec![0.0_f32; nn];
    let p = vec![1.0_f32; nn];
    let pet = vec![0.0_f32; nn];
    let mut grid = FluvialGridStep {
        bedrock_h: &mut b,
        sediment_s: &mut sed,
        uplift_m_yr: &up,
        k_erodibility: &k0,
        precip_m_yr: &p,
        pet_m_yr: &pet,
        width: w,
        height: w,
        sea_level_m: None,
    };
    let st = erode_fluvial_step_f32(
        &mut grid,
        &FluvialParams {
            dt_yr: 1_000.0,
            g_deposition: 0.85,
            ..FluvialParams::default()
        },
    )
    .expect("pure-uplift wrapper step converges");
    assert!(st.converged);
    let mid = 8 * w + 8;
    assert!(
        (b[mid] - (b0[mid] + 1.0)).abs() < 1e-4 && sed[mid].abs() < 1e-5,
        "uplift misbooked: b0={} b={} sed={}",
        b0[mid],
        b[mid],
        sed[mid]
    );

    // R8b: Active incision (U = 0, K > 0, initial sed = 0.2) strips sediment first then bedrock.
    let mut b_inc = b0.clone();
    let mut sed_inc = vec![0.2_f32; nn];
    let up0 = vec![0.0_f32; nn];
    let k_act = vec![5e-4_f32; nn];
    let mut grid_inc = FluvialGridStep {
        bedrock_h: &mut b_inc,
        sediment_s: &mut sed_inc,
        uplift_m_yr: &up0,
        k_erodibility: &k_act,
        precip_m_yr: &p,
        pet_m_yr: &pet,
        width: w,
        height: w,
        sea_level_m: None,
    };
    let st_inc = erode_fluvial_step_f32(
        &mut grid_inc,
        &FluvialParams {
            dt_yr: 2_000.0,
            g_deposition: 0.85,
            ..FluvialParams::default()
        },
    )
    .expect("incision wrapper step converges");
    assert!(st_inc.converged);
    assert!(
        b_inc[mid] < b0[mid] - 0.1 && sed_inc[mid] == 0.0,
        "expected bedrock incision at center: b0={} b={} sed={}",
        b0[mid],
        b_inc[mid],
        sed_inc[mid]
    );

    // R8c: Steep upland feeding gentle outer apron with G = 2.0 deposits residual (> 0) into sediment_s,
    // leaving apron bedrock_h unchanged (catches M19 positive dh -> bedrock and M20 ignored result).
    let mut b_agg = vec![0.0_f32; nn];
    for r in 0..w {
        for c in 0..w {
            let bd = (r.min(w - 1 - r)).min(c.min(w - 1 - c));
            b_agg[r * w + c] = if bd == 0 {
                0.0
            } else if bd <= 2 {
                (bd as f32) * 0.2
            } else {
                100.0 + ((bd - 2) as f32) * 40.0
            };
        }
    }
    let b_agg0 = b_agg.clone();
    let mut sed_agg = vec![0.0_f32; nn];
    let mut grid_agg = FluvialGridStep {
        bedrock_h: &mut b_agg,
        sediment_s: &mut sed_agg,
        uplift_m_yr: &up0,
        k_erodibility: &vec![1e-3_f32; nn],
        precip_m_yr: &p,
        pet_m_yr: &pet,
        width: w,
        height: w,
        sea_level_m: None,
    };
    let st_agg = erode_fluvial_step_f32(
        &mut grid_agg,
        &FluvialParams {
            dt_yr: 1_000.0,
            g_deposition: 2.0,
            ..FluvialParams::default()
        },
    )
    .expect("aggradation wrapper step converges");
    assert!(st_agg.converged);
    let mut aggraded_count = 0usize;
    let mut max_apron_sed = 0.0_f32;
    for r in 1..(w - 1) {
        for c in 1..(w - 1) {
            let i = r * w + c;
            assert!(
                b_agg[i] <= b_agg0[i] + 1e-5,
                "with U=0 bedrock must never increase (deposition misbooked as bedrock at cell {i}: {} -> {})",
                b_agg0[i],
                b_agg[i]
            );
            if sed_agg[i] > 0.01 {
                aggraded_count += 1;
                max_apron_sed = max_apron_sed.max(sed_agg[i]);
                assert!(
                    (b_agg[i] - b_agg0[i]).abs() < 1e-5,
                    "aggrading cell {i} modified bedrock_h instead of sediment_s: {} -> {}",
                    b_agg0[i],
                    b_agg[i]
                );
            }
        }
    }
    assert!(
        aggraded_count > 0 && max_apron_sed > 0.01,
        "expected downstream apron deposition into sediment_s, got count={aggraded_count} max={max_apron_sed}"
    );
}

#[test]
fn fluvial_params_validation_rejects_non_unit_n_and_non_positive_min_precip() {
    // R9: Implicit Fastscape-SD must reject n != 1.0 and min_precip_m_yr <= 0.0 loudly.
    let (rec, df, order, fixed) = make_1d_stream_dag(8);
    let up = vec![0.0_f64; 8];
    let kk = vec![1e-5_f64; 8];
    let p = vec![1.0_f64; 8];
    let e = vec![0.0_f64; 8];
    let dag = FluvialDagView {
        width: 8,
        height: 1,
        d8_receiver: &rec,
        receiver_dist_factor: &df,
        order_downstream_to_upstream: &order,
        fixed_outlet: &fixed,
        uplift_m_yr: &up,
        k_erodibility: &kk,
        precip_m_yr: &p,
        pet_m_yr: &e,
    };

    for &bad_n in &[0.5_f64, 1.5, f64::NAN] {
        let mut h = vec![1.0_f64; 8];
        let res = step_fastscape_sd_dag(
            &mut h,
            &dag,
            &FluvialParams {
                n: bad_n,
                ..FluvialParams::default()
            },
        );
        assert!(matches!(
            res,
            Err(FluvialError::InvalidSlopeExponent { .. })
        ));
    }
    for &bad_min_p in &[0.0_f64, -0.1, f64::NAN] {
        let mut h = vec![1.0_f64; 8];
        let res = step_fastscape_sd_dag(
            &mut h,
            &dag,
            &FluvialParams {
                min_precip_m_yr: bad_min_p,
                ..FluvialParams::default()
            },
        );
        assert!(matches!(res, Err(FluvialError::InvalidMinPrecip { .. })));
    }
}

#[test]
fn s1_caprock_tableland_coverage_and_eastern_inland_sea_allowance() {
    // Carried WV2-1a -> WV2-1b DoD: 15-20 km leeward sandstone caprock plateau targets ~14% of
    // domain (asserted [9%, 21%]), and east-third structure allows an eastern inland sea (<= 30%).
    for &seed in &[
        11_u64,
        42,
        99,
        1337,
        7777,
        0x2026_1120,
        0x5756_3201,
        0xDEAD_BEEF,
    ] {
        let field = generate_uplift(&WorldV2Config::for_size(seed, 256)).expect("S1 field");
        let cap_id = LithologyClass::SandstoneCaprock as u8;
        let cap_count = field
            .lithology_id
            .iter()
            .filter(|&&id| id == cap_id)
            .count();
        let cap_frac = cap_count as f64 / field.lithology_id.len() as f64;
        let mut seen = [false; LITHOLOGY_CLASS_COUNT];
        for &id in &field.lithology_id {
            seen[usize::from(id)] = true;
        }
        assert!(
            seen.iter().all(|&ok| ok),
            "seed {seed} missing lithology class"
        );

        let w = 256usize;
        let east_start = (2 * w) / 3;
        let mut east_ocean = 0usize;
        let mut east_total = 0usize;
        for row in 0..w {
            for col in east_start..w {
                if field.bedrock_h[row * w + col] < 0.0 {
                    east_ocean += 1;
                }
                east_total += 1;
            }
        }
        let east_ocean_frac = east_ocean as f64 / east_total as f64;
        eprintln!(
            "WV2-1b DoD seed={seed}: caprock_frac={cap_frac:.4} east_ocean_frac={east_ocean_frac:.4}"
        );
        assert!(
            (0.09..=0.21).contains(&cap_frac),
            "seed {seed}: caprock coverage {cap_frac:.4} outside [0.09, 0.21]"
        );
        assert!(
            east_ocean_frac <= 0.30,
            "seed {seed}: east_ocean_frac {east_ocean_frac:.4} > 0.30"
        );
    }
}
