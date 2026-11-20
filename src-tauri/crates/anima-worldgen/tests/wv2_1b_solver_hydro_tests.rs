#![cfg(feature = "generate")]

use anima_worldgen::config::WorldV2Config;
use anima_worldgen::erosion_fluvial::{
    step_explicit_forward_euler_dag, step_fastscape_sd_dag, AreaWeightMode, FluvialDagView,
    FluvialParams, FluvialSolverMode,
};
use anima_worldgen::erosion_thermal::{
    deg_to_slope, relax_thermal_and_hillslope, ThermalHillslopeParams,
};
use anima_worldgen::hydrology::{
    count_pits, priority_flood_d8, route_hybrid_mfd_d8, HydrologyConfig, NO_RECEIVER,
};
use anima_worldgen::tectonic::generate_uplift;
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
        let _ = step_fastscape_sd_dag(&mut h, &dag, &params);
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
        let stats = step_fastscape_sd_dag(&mut h, &dag, &params);
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
        let _ = step_fastscape_sd_dag(&mut h_no_local, &dag, &params_no_local);
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
        let _ = step_fastscape_sd_dag(&mut h_no_iter, &dag, &params_no_iter);
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
        let _ = step_fastscape_sd_dag(&mut h_imp, &dag, &imp_params);
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
        let _ = step_fastscape_sd_dag(&mut h_bad, &dag, &bad_params);
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
        let _ = step_fastscape_sd_dag(&mut h_imp_2d, &dag_2d, &imp_2d_params);
    }
    let rel_err_2d = relative_transient_rms(&h0_2d, &h_exp_2d, &h_imp_2d, &fixed_2d);
    assert!(
        rel_err_2d <= 0.02,
        "32x32 mini-grid transient relative RMS = {rel_err_2d:.4} (> 0.02)"
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
        let st = step_fastscape_sd_dag(&mut h_precip, &dag, &params);
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

    // Negative control (b): unclamped runoff `max(0, P - PET)` weighting produces NaNs / zero incision!
    let mut h_bad = h0;
    let params_bad = FluvialParams {
        area_weight_mode: AreaWeightMode::UnclampedRunoffNegativeControl,
        ..params
    };
    let stats_bad = step_fastscape_sd_dag(&mut h_bad, &dag, &params_bad);
    assert!(
        stats_bad.non_finite_count > 0 || !stats_bad.max_beta.is_finite(),
        "negative control (UnclampedRunoff) failed to trigger singularity: {stats_bad:?}"
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

            let mut grid = anima_worldgen::erosion_fluvial::FluvialGridStep {
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
            let _ = anima_worldgen::erosion_fluvial::erode_fluvial_step_f32(
                &mut grid,
                &FluvialParams {
                    dx_m: 160.0,
                    dt_yr: 1_000.0,
                    g_deposition: 0.85,
                    ..FluvialParams::default()
                },
            );
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
    );
    let dt_fs = t_fs.elapsed();
    assert_eq!(stats.non_finite_count, 0);

    eprintln!(
        "WV2-1b timing size={size}x{size}: s1={:.3}s priority_flood_d8={:.3}s fastscape_sd_g085={:.3}s (gs_iters={})",
        dt_s1.as_secs_f64(),
        dt_pf.as_secs_f64(),
        dt_fs.as_secs_f64(),
        stats.gs_iterations
    );
}
