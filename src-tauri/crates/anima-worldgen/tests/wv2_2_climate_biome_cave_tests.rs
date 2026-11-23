#![cfg(feature = "generate")]
#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

use anima_worldgen::biomes::{classify_biomes, BiomeConfig, BiomeInput};
use anima_worldgen::caves::CaveNetwork;
use anima_worldgen::climate::{compute_climate, couple_climate_for_resolution, ClimateConfig};
use anima_worldgen::config::WorldV2Config;
use anima_worldgen::hydrology::{priority_flood_d8, route_hybrid_mfd_d8, HydrologyConfig};
use anima_worldgen::lakes_falls::{
    generate_lakes_and_waterfalls, LakeClassification, LakesFallsConfig, LakesFallsInput,
};
use anima_worldgen::pipeline::{
    read_bin_f32, read_bin_u8, read_bin_u8x4, run_worldgen_to_dir, RawWorldMeta,
};
use anima_worldgen::tectonic::generate_uplift;
use anima_worldgen::{BiomeId, WaterKind, WORLD_EXTENT_M};
use rayon::ThreadPoolBuilder;
use std::path::PathBuf;
use std::process::Command;

struct TempDirGuard(PathBuf);

impl TempDirGuard {
    fn new(label: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("anima_wv2_2_{}_{}", label, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        Self(dir)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn synthetic_coastal_massif_dem(n: usize, peak_m: f32) -> (Vec<f32>, f32) {
    let dx_m = WORLD_EXTENT_M / n as f32;
    let mut elev = vec![0.0_f32; n * n];
    for r in 0..n {
        for c in 0..n {
            let u = c as f32 / (n - 1) as f32;
            let h = if u < 0.36 {
                -220.0 * (0.36 - u) / 0.36
            } else {
                let du = (u - 0.56) / 0.11;
                peak_m * libm::expf(-0.5 * du * du) + 140.0 * ((u - 0.36) / 0.64)
            };
            elev[r * n + c] = h;
        }
    }
    (elev, dx_m)
}

#[test]
fn climate_11n_zero_latitudinal_gradient_maritime_buffering_and_lapse_rate() {
    let n = 128usize;
    let (elev, dx_m) = synthetic_coastal_massif_dem(n, 5_550.0);
    let cfg = ClimateConfig {
        dx_m,
        ..ClimateConfig::default()
    };
    assert!((cfg.latitude_deg - 11.0).abs() < 1e-6);
    assert!((cfg.t0_sea_level_c - 27.5).abs() < 1e-6);
    assert!((cfg.lapse_rate_c_per_km - 6.3).abs() < 1e-6);

    let clim = compute_climate(&elev, n, n, None, &cfg).expect("climate");
    let q_rows = n / 4;
    let north_mean: f64 = clim.temp_c[..q_rows * n]
        .iter()
        .map(|&v| f64::from(v))
        .sum::<f64>()
        / (q_rows * n) as f64;
    let south_mean: f64 = clim.temp_c[(n - q_rows) * n..]
        .iter()
        .map(|&v| f64::from(v))
        .sum::<f64>()
        / (q_rows * n) as f64;
    assert!(
        (north_mean - south_mean).abs() < 0.05,
        "11N fixed latitude requires zero N-S gradient: north={north_mean}, south={south_mean}"
    );

    // Coastal maritime buffering: low-elevation coast (d < 500 m) warmer than inland at same low elev.
    let mut flat_coast = vec![10.0_f32; n * n];
    for r in 0..n {
        for c in 0..20 {
            flat_coast[r * n + c] = -80.0;
        }
    }
    let clim_coast = compute_climate(&flat_coast, n, n, None, &cfg).expect("flat coast");
    let mid = n / 2;
    let t_coastal = clim_coast.temp_c[mid * n + 20];
    let t_inland = clim_coast.temp_c[mid * n + 100];
    assert!(
        t_coastal - t_inland > 0.45 && t_coastal - t_inland < 0.95,
        "expected ~0.5..0.9 C maritime buffering, got {}",
        t_coastal - t_inland
    );

    // Exact 6.3 C/km lapse rate & 0 C isotherm near 4,365 m.
    let mut flat_high = vec![4_365.0_f32; n * n];
    for r in 0..n {
        for c in 0..n {
            if c < 20 {
                flat_high[r * n + c] = -100.0;
            } else if c > 80 {
                flat_high[r * n + c] = 5_700.0;
            }
        }
    }
    let clim_hi = compute_climate(&flat_high, n, n, None, &cfg).expect("flat_high");
    let t_4365 = clim_hi.temp_c[mid * n + 50];
    let t_5700 = clim_hi.temp_c[mid * n + 110];
    assert!(t_4365.abs() < 0.25, "expected ~0 C at 4365 m, got {t_4365}");
    assert!(
        (t_5700 - (-8.41)).abs() < 0.25,
        "expected ~-8.41 C at 5700 m, got {t_5700}"
    );
    let measured_gamma = (t_4365 - t_5700) / ((5_700.0 - 4_365.0) * 0.001);
    assert!(
        (6.1..=6.5).contains(&measured_gamma),
        "measured lapse rate {measured_gamma} outside [6.1, 6.5] C/km"
    );
}

#[test]
fn ela_decoupled_from_zero_c_isotherm_and_glacier_constraints() {
    let n = 96usize;
    let dx_m = WORLD_EXTENT_M / n as f32;
    let mut elev = vec![0.0_f32; n * n];
    for r in 0..n {
        let dr = (r as f32 - 48.0) / 14.0;
        let h_ew = 3_600.0 + 1_950.0 * libm::expf(-0.5 * dr * dr);
        for c in 0..n {
            elev[r * n + c] = if c < 10 { -100.0 } else { h_ew };
        }
    }
    let ccfg = ClimateConfig {
        dx_m,
        ..ClimateConfig::default()
    };
    let clim = compute_climate(&elev, n, n, None, &ccfg).expect("climate");
    for &ela in &clim.ela_m {
        assert!(
            (4_600.0..=5_200.0).contains(&ela),
            "ELA {ela} outside [4600, 5200] m"
        );
    }

    // Sunlit south aspect (r > 48) has higher ELA than shaded north aspect (r < 48).
    let north_cell = 40 * n + 48;
    let south_cell = 56 * n + 48;
    assert!(clim.ela_m[south_cell] > clim.ela_m[north_cell]);

    let zeros = vec![0.0_f32; n * n];
    let zeros_u8 = vec![0_u8; n * n];
    let talus = vec![0.35_f32; n * n];
    let b_in = BiomeInput {
        width: n,
        height: n,
        total_h_m: &elev,
        temp_c: &clim.temp_c,
        precip_m_yr: &clim.precip_m_yr,
        moisture: &clim.moisture,
        twi: &clim.twi,
        ela_local_m: &clim.ela_m,
        alluvial_mask: &zeros,
        talus_frac: &talus,
        sediment_s_m: &zeros,
        water_kind: &zeros_u8,
        river_w_m: &zeros,
        lithology_id: &zeros_u8,
    };
    let bf = classify_biomes(
        &b_in,
        &BiomeConfig {
            dx_m,
            ..BiomeConfig::default()
        },
    )
    .expect("biomes");

    let mut sub_ela_freezing_verified = 0usize;
    let mut glacier_above_ela = 0usize;
    let mut snow_above_ela = 0usize;
    let mut moraine_nonzero = 0usize;

    for r in 1..(n - 1) {
        for c in 1..(n - 1) {
            let i = r * n + c;
            let h = elev[i];
            let dom = bf.dominant_biome[i];
            let hx = (elev[r * n + (c + 1)] - elev[r * n + (c - 1)]) / (2.0 * dx_m);
            let hz = (elev[(r + 1) * n + c] - elev[(r - 1) * n + c]) / (2.0 * dx_m);
            let slope_deg = libm::atanf(libm::hypotf(hx, hz)) * (180.0 / std::f32::consts::PI);

            if (4_400.0..=4_540.0).contains(&h) {
                assert!(clim.temp_c[i] < 0.0, "expected T < 0 C above 4400 m");
                assert_eq!(bf.glacier_h_m[i], 0.0);
                assert_ne!(dom, BiomeId::Snow as u8);
                assert_ne!(dom, BiomeId::Glacier as u8);
                sub_ela_freezing_verified += 1;
            }
            if h < 4_550.0 {
                assert_eq!(bf.glacier_h_m[i], 0.0);
                assert_ne!(dom, BiomeId::Glacier as u8);
            }
            if h < clim.ela_m[i] {
                assert_ne!(dom, BiomeId::Snow as u8);
                assert_ne!(dom, BiomeId::Glacier as u8);
            }
            if slope_deg > 42.0 {
                assert_eq!(bf.glacier_h_m[i], 0.0);
                assert_ne!(dom, BiomeId::Glacier as u8);
            }
            if h >= clim.ela_m[i] && bf.glacier_h_m[i] > 0.0 {
                glacier_above_ela += 1;
            }
            if dom == BiomeId::Snow as u8 {
                snow_above_ela += 1;
            }
            if bf.moraine_debris[i] > 0 {
                moraine_nonzero += 1;
            }
        }
    }
    assert!(sub_ela_freezing_verified > 0);
    assert!(glacier_above_ela > 0);
    assert!(snow_above_ela > 0);
    assert!(moraine_nonzero > 0);
}

#[test]
fn orographic_precip_floor_rain_shadow_and_twi() {
    let wcfg = WorldV2Config::for_size(0x0057_5632_2026_1120, 128);
    let tect = generate_uplift(&wcfg).expect("S1");
    let ccfg = ClimateConfig::for_world(&wcfg);
    let clim = compute_climate(&tect.bedrock_h, 128, 128, None, &ccfg).expect("S7");

    let mut min_p = f32::INFINITY;
    let mut windward_peak = 0.0_f32;
    let mut lee_sum = 0.0_f64;
    let mut lee_cnt = 0_usize;
    for r in 16..(128 - 16) {
        for c in 0..128 {
            let idx = r * 128 + c;
            let p = clim.precip_m_yr[idx];
            min_p = min_p.min(p);
            let u = c as f32 / 127.0;
            let h = tect.bedrock_h[idx];
            if h > 0.0 && (0.44..=0.62).contains(&u) {
                windward_peak = windward_peak.max(p);
            }
            if h > 0.0 && (0.72..=0.92).contains(&u) {
                lee_sum += f64::from(p);
                lee_cnt += 1;
            }
        }
    }
    assert!(min_p >= 0.15, "min_p {min_p} < 0.15");
    assert!(
        (2.5..=4.3).contains(&windward_peak),
        "windward_peak {windward_peak} outside [2.5, 4.3]"
    );
    let lee_mean = (lee_sum / lee_cnt as f64) as f32;
    assert!(
        lee_mean < 0.5 * windward_peak,
        "lee_mean {lee_mean} >= 0.5 * windward_peak {windward_peak}"
    );

    // Exact TWI formula & valley > ridge check + 2-pass coupling.
    let n = 64usize;
    let dx_m = WORLD_EXTENT_M / n as f32;
    let flat = vec![100.0_f32; n * n];
    let cell_a = f64::from(dx_m) * f64::from(dx_m);
    let mut flow_acc = vec![cell_a; n * n];
    let valley_idx = 32 * n + 32;
    let ridge_idx = 32 * n + 10;
    flow_acc[valley_idx] = 100.0 * cell_a;
    let c_flat = compute_climate(
        &flat,
        n,
        n,
        Some(&flow_acc),
        &ClimateConfig {
            dx_m,
            ..ClimateConfig::default()
        },
    )
    .expect("flat");
    let expected_twi = libm::logf(((100.0 * cell_a) as f32 / dx_m + dx_m) / 0.002);
    assert!((c_flat.twi[valley_idx] - expected_twi).abs() < 1e-4);
    assert!(c_flat.twi[valley_idx] > c_flat.twi[ridge_idx]);
    assert!(c_flat.moisture[valley_idx] > c_flat.moisture[ridge_idx]);

    let coupled = couple_climate_for_resolution(
        &tect.bedrock_h,
        128,
        128,
        256,
        256,
        Some(&vec![(dx_m * dx_m) as f64; 256 * 256]),
        &ccfg,
    )
    .expect("2-pass coupling");
    assert_eq!(coupled.width, 256);
    assert!(coupled.precip_m_yr.iter().all(|&p| p >= 0.15));
}

#[test]
fn fill_spill_merge_humid_spillways_endorheic_equilibrium_and_c3_diversity() {
    let guard = TempDirGuard::new("c3_diversity");
    let cfg = WorldV2Config::for_size(0x0057_5632_2026_1120, 256);
    let meta: RawWorldMeta = run_worldgen_to_dir(&cfg, guard.path()).expect("pipeline 256");

    assert_eq!(meta.stats.severed_humid_spillways, 0);
    assert_eq!(meta.stats.unmodeled_micro_pits, 0);
    assert_eq!(meta.stats.endorheic_humid_fraction, 0.0);
    assert!(meta.stats.endorheic_land_fraction <= 0.10);
    assert!(
        (0.008..=0.045).contains(&meta.stats.lake_surface_fraction),
        "lake_surface_fraction={} outside [0.008, 0.045]",
        meta.stats.lake_surface_fraction
    );
    let large_lakes = meta.basins.iter().filter(|b| b.area_m2 > 100_000.0).count();
    assert!(
        large_lakes >= 5,
        "expected >= 5 lakes > 0.1 km^2, got {large_lakes}"
    );
    let paramo_lakes = meta
        .basins
        .iter()
        .filter(|b| b.mean_elev_m > 3_300.0)
        .count();
    assert!(paramo_lakes >= 1, "expected >= 1 high-paramo lake > 3300 m");
    let caldera_or_trough = meta
        .basins
        .iter()
        .filter(|b| {
            b.classification == LakeClassification::CalderaOrTrough
                || b.max_wall_slope_deg > 25.0
                || b.elongation > 3.5
        })
        .count();
    assert!(caldera_or_trough >= 1, "expected >= 1 caldera/trough lake");
    let small_paramo_ponds = meta
        .basins
        .iter()
        .filter(|b| b.mean_elev_m > 3_300.0 && b.area_m2 < 100_000.0)
        .count();
    assert!(
        small_paramo_ponds >= 10 || meta.stats.micro_pond_count >= 10,
        "expected >= 10 small paramo ponds (basins={small_paramo_ponds}, s4={})",
        meta.stats.micro_pond_count
    );

    // Arid closed basin equilibrium: |A(h_eq)*PET - Q_in| / Q_in < 0.10.
    let n = 64usize;
    let dx_m = 40.0_f32;
    let mut elev = vec![0.0_f32; n * n];
    for r in 0..n {
        for c in 0..n {
            let dr = r as f32 - 32.0;
            let dc = c as f32 - 32.0;
            let dist = (dr * dr + dc * dc).sqrt();
            let bowl = 200.0 + (dist * 2.2).min(180.0);
            elev[r * n + c] = if r == 0 || r + 1 == n || c == 0 || c + 1 == n {
                380.0
            } else {
                bowl
            };
        }
    }
    let pf = priority_flood_d8(&elev, n, n, None);
    let precip = vec![0.45_f32; n * n];
    let pet = vec![1.35_f32; n * n];
    let hydro = route_hybrid_mfd_d8(
        &pf.filled_h,
        &pf.d8_receiver,
        n,
        n,
        &precip,
        &pet,
        &HydrologyConfig {
            dx_m,
            sea_level_m: None,
            ..HydrologyConfig::default()
        },
    );
    // Inject small arid inflow patch on rim so Q_in > 0 yet A_full * PET > Q_in.
    let mut p_in = precip;
    for r in 6..18 {
        for c in 6..18 {
            p_in[r * n + c] = 2.6;
        }
    }
    let mut sed = vec![0.0_f32; n * n];
    let mut wh = hydro.water_h_m.clone();
    let mut wk = vec![WaterKind::None as u8; n * n];
    let s6_in = LakesFallsInput::from_hydrology(&hydro, &p_in, &pet);
    let s6_arid = generate_lakes_and_waterfalls(
        &mut elev,
        &mut sed,
        &mut wh,
        &mut wk,
        &s6_in,
        &LakesFallsConfig {
            dx_m,
            sea_level_m: -1_000.0,
            ..LakesFallsConfig::default()
        },
    )
    .expect("arid equilibrium");
    let endo = s6_arid
        .basins
        .iter()
        .find(|b| b.classification == LakeClassification::EndorheicClosed)
        .expect("endorheic closed basin");
    assert_eq!(endo.q_out_m3_s, 0.0);
    let q_in_yr = f64::from(endo.q_in_m3_s) * anima_worldgen::hydrology::SECONDS_PER_YEAR;
    let evap_yr = endo.area_m2 * 1.35;
    let rel_eq_err = libm::fabs(evap_yr - q_in_yr) / q_in_yr.max(1.0);
    assert!(
        rel_eq_err < 0.10,
        "endorheic water balance error {rel_eq_err:.4} >= 0.10"
    );
}

#[test]
fn knickpoint_waterfalls_20_to_80_with_plunge_pools() {
    let guard = TempDirGuard::new("waterfalls");
    let size = 512_u32;
    let cfg = WorldV2Config::for_size(0x0057_5632_2026_1120, size);
    let meta = run_worldgen_to_dir(&cfg, guard.path()).expect("pipeline 512");
    assert!(
        (20..=80).contains(&meta.waterfalls.len()),
        "expected 20..=80 waterfalls, got {}",
        meta.waterfalls.len()
    );
    let wk = read_bin_u8(
        &guard.path().join("water_kind.bin"),
        (size as usize) * (size as usize),
    )
    .expect("water_kind");
    let min_sp = LakesFallsConfig::default().waterfall_min_spacing_m;
    for (idx, wf) in meta.waterfalls.iter().enumerate() {
        assert!(wf.drop_m >= 12.0, "waterfall {idx} drop_m < 12.0");
        assert!(
            wf.plunge_depth_m > 0.0,
            "waterfall {idx} plunge_depth_m <= 0"
        );
        assert_eq!(
            wk[wf.base_cell as usize],
            WaterKind::Waterfall as u8,
            "base cell not marked Waterfall"
        );
        for other in &meta.waterfalls[(idx + 1)..] {
            let d = libm::hypotf(wf.x_m - other.x_m, wf.z_m - other.z_m);
            assert!(d >= min_sp - 1e-2, "waterfall spacing {d} < {min_sp}");
        }
    }
}

#[test]
fn multi_seed_card3_waterfalls_rivers_lakes_dunes_invariants() {
    let size = 512_u32;
    let n = (size as usize) * (size as usize);
    let scaled_floor =
        ((2500.0_f64 * (f64::from(size) / 4096.0) * (f64::from(size) / 4096.0)).ceil()) as usize;

    // 2 diverse seeds at 512^2 keep debug CI fast; all 12 seeds at 1024^2 run in `examples/wv2_2_release_verify.rs`.
    for &seed in &[57_563_201_u64, 42_u64] {
        let guard = TempDirGuard::new(&format!("card3_{size}_{seed}"));
        let cfg = WorldV2Config::for_size(seed, size);
        let meta = run_worldgen_to_dir(&cfg, guard.path()).expect("pipeline 512");

        assert!(
            (20..=80).contains(&meta.stats.waterfall_count),
            "seed={seed}: waterfall_count={} outside 20..=80",
            meta.stats.waterfall_count
        );
        assert!(
            (0.0005..=0.0030).contains(&meta.stats.river_surface_fraction),
            "seed={seed}: river_surface_fraction={} outside [0.0005, 0.0030]",
            meta.stats.river_surface_fraction
        );
        assert!(
            (0.008..=0.045).contains(&meta.stats.lake_surface_fraction),
            "seed={seed}: lake_surface_fraction={} outside [0.008, 0.045]",
            meta.stats.lake_surface_fraction
        );
        let large_lakes = meta.basins.iter().filter(|b| b.area_m2 > 100_000.0).count();
        assert!(
            large_lakes >= 5,
            "seed={seed}: expected >= 5 lakes > 0.1 km^2, got {large_lakes}"
        );
        assert_eq!(meta.stats.severed_humid_spillways, 0);
        assert_eq!(meta.stats.unmodeled_micro_pits, 0);
        assert_eq!(meta.stats.endorheic_humid_fraction, 0.0);
        assert!(
            (30..=48).contains(&meta.stats.cave_entrance_count),
            "seed={seed}: cave_entrance_count={} outside [30, 48]",
            meta.stats.cave_entrance_count
        );
        for (wf_idx, wf) in meta.waterfalls.iter().enumerate() {
            assert!(
                wf.plunge_depth_m > 0.0,
                "seed={seed}: waterfall {wf_idx} plunge_depth_m={} <= 0.0",
                wf.plunge_depth_m
            );
        }
        assert!(
            (0.15..=0.55).contains(&meta.stats.ecotone_fraction),
            "seed={seed}: ecotone_fraction={} outside [0.15, 0.55]",
            meta.stats.ecotone_fraction
        );
        assert!(
            meta.stats.non_cliff_jump_fraction < 0.03,
            "seed={seed}: non_cliff_jump_fraction={} >= 0.03",
            meta.stats.non_cliff_jump_fraction
        );
        for &b in &BiomeId::ZONED_TERRESTRIAL_16 {
            let cnt = meta.stats.biome_counts[b as usize];
            assert!(
                cnt >= scaled_floor,
                "seed={seed}: biome {b:?} has {cnt} < {scaled_floor}"
            );
        }
        let dune = read_bin_f32(&guard.path().join("dune_h_m.bin"), n).expect("dune");
        let dune_cells = dune.iter().filter(|&&d| d > 0.0).count();
        assert!(dune_cells > 0, "seed={seed}: expected dune_cells > 0");
    }
}

#[test]
fn negative_control_disabled_fill_spill_merge_severs_humid_spillways() {
    let n = 64usize;
    let dx_m = 40.0_f32;
    let mut base_elev = vec![0.0_f32; n * n];
    for r in 0..n {
        for c in 0..n {
            let dr = r as f32 - 32.0;
            let dc = c as f32 - 32.0;
            let dist = (dr * dr + dc * dc).sqrt();
            let outer = (500.0 - dist * 8.0).max(-40.0);
            let bowl = if (r as isize - 26).abs() <= 6 && (c as isize - 32).abs() <= 6 {
                let b_d = libm::hypotf(r as f32 - 26.0, c as f32 - 32.0);
                -110.0 * (1.0 - (b_d / 7.0).min(1.0))
            } else {
                0.0
            };
            base_elev[r * n + c] = outer + bowl;
        }
    }
    let pf = priority_flood_d8(&base_elev, n, n, Some(0.0));
    let precip = vec![3.0_f32; n * n];
    let pet = vec![0.8_f32; n * n];
    let hydro = route_hybrid_mfd_d8(
        &pf.filled_h,
        &pf.d8_receiver,
        n,
        n,
        &precip,
        &pet,
        &HydrologyConfig {
            dx_m,
            ..HydrologyConfig::default()
        },
    );

    let mut elev_on = base_elev.clone();
    let mut sed_on = vec![0.0_f32; n * n];
    let mut wh_on = hydro.water_h_m.clone();
    let mut wk_on = vec![WaterKind::None as u8; n * n];
    let s6_in = LakesFallsInput::from_hydrology(&hydro, &precip, &pet);
    let res_on = generate_lakes_and_waterfalls(
        &mut elev_on,
        &mut sed_on,
        &mut wh_on,
        &mut wk_on,
        &s6_in,
        &LakesFallsConfig {
            dx_m,
            enable_fill_spill_merge: true,
            ..LakesFallsConfig::default()
        },
    )
    .expect("FSM enabled");
    assert_eq!(res_on.severed_humid_spillways, 0);

    let mut elev_off = base_elev;
    let mut sed_off = vec![0.0_f32; n * n];
    let mut wh_off = hydro.water_h_m.clone();
    let mut wk_off = vec![WaterKind::None as u8; n * n];
    let res_off = generate_lakes_and_waterfalls(
        &mut elev_off,
        &mut sed_off,
        &mut wh_off,
        &mut wk_off,
        &s6_in,
        &LakesFallsConfig {
            dx_m,
            enable_fill_spill_merge: false,
            ..LakesFallsConfig::default()
        },
    )
    .expect("FSM disabled");
    assert!(
        res_off.severed_humid_spillways > 0,
        "negative control: disabled FillSpillMerge must sever humid spillways"
    );
}

#[test]
fn negative_control_flattened_peaks_fail_snowline_and_rain_shadow() {
    let n = 128usize;
    let wcfg = WorldV2Config::for_size(0x0057_5632_2026_1120, n as u32);
    let mut tect = generate_uplift(&wcfg).expect("S1");
    for h in &mut tect.bedrock_h {
        if *h > 0.0 {
            *h = 120.0;
        }
    }
    let dx_m = wcfg.cell_size_m();
    let ccfg = ClimateConfig::for_world(&wcfg);
    let clim = compute_climate(&tect.bedrock_h, n, n, None, &ccfg).expect("S7");

    let mut west_sum = 0.0_f64;
    let mut west_cnt = 0usize;
    let mut east_sum = 0.0_f64;
    let mut east_cnt = 0usize;
    for r in 16..(n - 16) {
        for c in 0..n {
            let u = c as f32 / (n - 1) as f32;
            let idx = r * n + c;
            if tect.bedrock_h[idx] > 0.0 {
                if (0.48..=0.62).contains(&u) {
                    west_sum += f64::from(clim.precip_m_yr[idx]);
                    west_cnt += 1;
                } else if (0.76..=0.94).contains(&u) {
                    east_sum += f64::from(clim.precip_m_yr[idx]);
                    east_cnt += 1;
                }
            }
        }
    }
    let ratio = (east_sum / east_cnt.max(1) as f64) / (west_sum / west_cnt.max(1) as f64);
    assert!(
        ratio >= 0.85,
        "flattened peaks must collapse rain shadow (ratio >= 0.85), got {ratio}"
    );

    let zeros = vec![0.0_f32; n * n];
    let zeros_u8 = vec![0_u8; n * n];
    let b_in = BiomeInput {
        width: n,
        height: n,
        total_h_m: &tect.bedrock_h,
        temp_c: &clim.temp_c,
        precip_m_yr: &clim.precip_m_yr,
        moisture: &clim.moisture,
        twi: &clim.twi,
        ela_local_m: &clim.ela_m,
        alluvial_mask: &zeros,
        talus_frac: &zeros,
        sediment_s_m: &zeros,
        water_kind: &zeros_u8,
        river_w_m: &zeros,
        lithology_id: &tect.lithology_id,
    };
    let bf = classify_biomes(
        &b_in,
        &BiomeConfig {
            dx_m,
            ..BiomeConfig::default()
        },
    )
    .expect("S8");
    let ge_ela = tect
        .bedrock_h
        .iter()
        .zip(clim.ela_m.iter())
        .filter(|(&h, &ela)| h >= ela)
        .count();
    assert_eq!(ge_ela, 0);
    let snow_or_glacier = bf
        .dominant_biome
        .iter()
        .filter(|&&b| b == BiomeId::Snow as u8 || b == BiomeId::Glacier as u8)
        .count();
    assert_eq!(snow_or_glacier, 0);
}

#[test]
fn full_pipeline_22_biome_blending_c1_c2_and_leeward_dunes() {
    let size = 512_u32;
    let n = (size as usize) * (size as usize);
    let scaled_floor =
        ((2500.0_f64 * (f64::from(size) / 4096.0) * (f64::from(size) / 4096.0)).ceil()) as usize;

    // 3 diverse seeds at 512^2 keep debug CI fast; all 11 seeds at 512^2 run in `examples/wv2_2_release_verify.rs`.
    for &seed in &[0x0057_5632_2026_1120_u64, 0x5756_3201_u64, 42_u64] {
        let guard = TempDirGuard::new(&format!("biomes_{seed}"));
        let cfg = WorldV2Config::for_size(seed, size);
        let meta = run_worldgen_to_dir(&cfg, guard.path()).expect("full pipeline 512");

        assert!(
            (0.40..=0.50).contains(&meta.stats.ocean_fraction),
            "seed={seed}: ocean_fraction={} outside [0.40, 0.50]",
            meta.stats.ocean_fraction
        );
        assert!(
            (0.008..=0.045).contains(&meta.stats.lake_surface_fraction),
            "seed={seed}: lake_surface_fraction={} outside [0.008, 0.045]",
            meta.stats.lake_surface_fraction
        );
        assert!(
            (0.0005..=0.0030).contains(&meta.stats.river_surface_fraction),
            "seed={seed}: river_surface_fraction={} outside [0.0005, 0.0030]",
            meta.stats.river_surface_fraction
        );
        assert_eq!(meta.stats.severed_humid_spillways, 0);
        assert_eq!(meta.stats.unmodeled_micro_pits, 0);
        assert_eq!(meta.stats.endorheic_humid_fraction, 0.0);
        assert!(meta.stats.endorheic_land_fraction <= 0.10);
        assert!(
            (30..=48).contains(&meta.stats.cave_entrance_count),
            "seed={seed}: cave_entrance_count={} outside [30, 48]",
            meta.stats.cave_entrance_count
        );
        for (wf_idx, wf) in meta.waterfalls.iter().enumerate() {
            assert!(
                wf.plunge_depth_m > 0.0,
                "seed={seed}: waterfall {wf_idx} plunge_depth_m={} <= 0.0",
                wf.plunge_depth_m
            );
        }
        assert_eq!(meta.stats.biome_counts[BiomeId::Chaparral as usize], 0);
        assert_eq!(meta.stats.biome_counts[BiomeId::Steppe as usize], 0);
        for &b in &BiomeId::ZONED_TERRESTRIAL_16 {
            let cnt = meta.stats.biome_counts[b as usize];
            assert!(
                cnt >= scaled_floor,
                "seed={seed}: biome {b:?} has {cnt} < {scaled_floor}"
            );
        }
        assert!(
            (0.15..=0.55).contains(&meta.stats.ecotone_fraction),
            "seed={seed}: ecotone_fraction={} outside [0.15, 0.55]",
            meta.stats.ecotone_fraction
        );
        assert!(
            meta.stats.non_cliff_jump_fraction < 0.03,
            "seed={seed}: non_cliff_jump_fraction={} >= 0.03",
            meta.stats.non_cliff_jump_fraction
        );

        let ids = read_bin_u8x4(&guard.path().join("biome_ids.bin"), n).expect("ids");
        let wts = read_bin_u8x4(&guard.path().join("biome_weights.bin"), n).expect("wts");
        let dune = read_bin_f32(&guard.path().join("dune_h_m.bin"), n).expect("dune");
        let precip = read_bin_f32(&guard.path().join("precip_m_yr.bin"), n).expect("precip");
        let moist = read_bin_f32(&guard.path().join("moisture.bin"), n).expect("moist");
        let lith = read_bin_u8(&guard.path().join("lithology_id.bin"), n).expect("lith");

        for (id4, w4) in ids.iter().zip(wts.iter()) {
            assert_eq!(w4.iter().map(|&w| u16::from(w)).sum::<u16>(), 255);
            assert!(!id4.contains(&(BiomeId::Chaparral as u8)));
            assert!(!id4.contains(&(BiomeId::Steppe as u8)));
        }

        let mut dune_cells = 0usize;
        for i in 0..n {
            if dune[i] > 0.0 {
                dune_cells += 1;
                assert!(precip[i] < 0.48, "dune cell precip={} >= 0.48", precip[i]);
                assert!(moist[i] < 0.28, "dune cell moist={} >= 0.28", moist[i]);
                assert_ne!(lith[i], 1, "dune formed on SandstoneCaprock");
            }
        }
        assert!(dune_cells > 0, "seed={seed}: expected leeward Werner dunes");
    }
}

#[test]
fn full_pipeline_s9_caves_and_anmc_v1_roundtrip() {
    let guard = TempDirGuard::new("caves_anmc");
    let size = 256_u32;
    let n = (size as usize) * (size as usize);
    let cfg = WorldV2Config::for_size(0x5756_3201, size);
    let meta = run_worldgen_to_dir(&cfg, guard.path()).expect("pipeline 256");

    let cave_bytes =
        std::fs::read(guard.path().join("cave_network.bin")).expect("cave_network.bin");
    let network = CaveNetwork::from_bytes(&cave_bytes).expect("valid ANMC v1");
    assert_eq!(network.to_bytes(), cave_bytes);
    assert!(
        (30..=48).contains(&network.entrances.len()),
        "entrance count {} outside [30, 48]",
        network.entrances.len()
    );
    assert_eq!(meta.stats.cave_entrance_count, network.entrances.len());

    let expected_len =
        24 + network.nodes.len() * 20 + network.edges.len() * 12 + network.entrances.len() * 20;
    assert_eq!(cave_bytes.len(), expected_len);
    assert_eq!(&cave_bytes[0..4], b"ANMC");

    let cave_mask = read_bin_u8(&guard.path().join("cave_mask.bin"), n).expect("cave_mask");
    let lith = read_bin_u8(&guard.path().join("lithology_id.bin"), n).expect("lith");
    let wk = read_bin_u8(&guard.path().join("water_kind.bin"), n).expect("wk");
    let glacier = read_bin_f32(&guard.path().join("glacier_h_m.bin"), n).expect("glacier");
    let temp = read_bin_f32(&guard.path().join("temp_c.bin"), n).expect("temp");
    let dom = read_bin_u8(&guard.path().join("dominant_biome.bin"), n).expect("dom");

    let portal_count = cave_mask.iter().filter(|&&m| m == 2).count();
    assert_eq!(portal_count, network.entrances.len());

    let dx_m = cfg.cell_size_m();
    let min_dist_m = 0.01 * f64::from(size) * f64::from(dx_m);
    for (i, ent) in network.entrances.iter().enumerate() {
        let node = network.nodes[ent.node_idx as usize];
        let col = ((node.x_m / dx_m).floor() as usize).min(size as usize - 1);
        let row = ((node.z_m / dx_m).floor() as usize).min(size as usize - 1);
        let cell = row * size as usize + col;
        assert!(
            lith[cell] == 2 || lith[cell] == 3,
            "invalid cave lith {}",
            lith[cell]
        );
        assert_eq!(wk[cell], WaterKind::None as u8);
        assert_eq!(glacier[cell], 0.0);
        assert!(temp[cell] > 0.5);
        assert!(!matches!(
            BiomeId::from_u8(dom[cell]),
            Some(
                BiomeId::Ocean | BiomeId::Lake | BiomeId::River | BiomeId::Snow | BiomeId::Glacier
            )
        ));
        for other in &network.entrances[(i + 1)..] {
            let onode = network.nodes[other.node_idx as usize];
            let d = libm::hypot(
                f64::from(node.x_m - onode.x_m),
                f64::from(node.z_m - onode.z_m),
            );
            assert!(d >= min_dist_m - 1e-3, "cave spacing {d} < {min_dist_m}");
        }
    }
}

#[test]
fn pipeline_sha256_repeatability_and_rayon_thread_invariance() {
    let g_a = TempDirGuard::new("256_4t_a");
    let g_b = TempDirGuard::new("256_4t_b");
    let g_1t = TempDirGuard::new("256_1t");
    let g_other = TempDirGuard::new("256_seed2");

    let cfg = WorldV2Config::for_size(0x5756_3201, 256);
    let pool_4 = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    let pool_1 = ThreadPoolBuilder::new().num_threads(1).build().unwrap();

    let m_a = pool_4.install(|| run_worldgen_to_dir(&cfg, g_a.path()).expect("256 4t a"));
    let m_b = pool_4.install(|| run_worldgen_to_dir(&cfg, g_b.path()).expect("256 4t b"));
    let m_1t = pool_1.install(|| run_worldgen_to_dir(&cfg, g_1t.path()).expect("256 1t"));

    assert_eq!(m_a.world_sha256, m_b.world_sha256);
    assert_eq!(m_a.world_sha256, m_1t.world_sha256);
    assert_eq!(m_a.layers, m_1t.layers);
    assert_eq!(m_a.cave_network_sha256, m_1t.cave_network_sha256);

    let c_a = std::fs::read(g_a.path().join("cave_network.bin")).unwrap();
    let c_1t = std::fs::read(g_1t.path().join("cave_network.bin")).unwrap();
    assert_eq!(c_a, c_1t);

    let cfg2 = WorldV2Config::for_size(0x2026_1120, 256);
    let m_other = pool_4.install(|| run_worldgen_to_dir(&cfg2, g_other.path()).expect("256 seed2"));
    assert_ne!(m_a.world_sha256, m_other.world_sha256);
}

#[test]
fn cli_generate_smoke_writes_raw_buffers_and_valid_anmc_v1() {
    let guard = TempDirGuard::new("cli_smoke");
    let bin = env!("CARGO_BIN_EXE_anima-worldgen");
    let out = Command::new(bin)
        .args(["generate", "--seed", "0x57563201", "--size", "256", "--out"])
        .arg(guard.path())
        .output()
        .expect("run anima-worldgen CLI");

    assert!(
        out.status.success(),
        "CLI failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let meta_raw =
        std::fs::read_to_string(guard.path().join("raw_meta.json")).expect("raw_meta.json");
    let meta: RawWorldMeta = serde_json::from_str(&meta_raw).expect("parse raw_meta.json");
    assert_eq!(meta.width, 256);
    assert_eq!(meta.height, 256);
    assert_eq!(meta.seed, 0x5756_3201);
    for layer in &meta.layers {
        let p = guard.path().join(&layer.filename);
        let fmeta = std::fs::metadata(&p).expect("layer file exists");
        assert_eq!(fmeta.len(), layer.byte_len);
    }

    let cave_bytes =
        std::fs::read(guard.path().join("cave_network.bin")).expect("cave_network.bin");
    let net = CaveNetwork::from_bytes(&cave_bytes).expect("valid ANMC v1");
    assert!((30..=48).contains(&net.entrances.len()));
}

#[test]
fn cli_generate_preset_toml_and_error_handling() {
    let guard = TempDirGuard::new("cli_preset");
    let bin = env!("CARGO_BIN_EXE_anima-worldgen");

    let preset_path = guard.path().join("preset.toml");
    std::fs::write(
        &preset_path,
        "schema_tag = \"PRESET-TEST-v2\"\ntarget_ocean_fraction = 0.44\nvolcano_relief_m = 2950.0\n",
    )
    .expect("write preset");

    let out_dir = guard.path().join("raw");
    let ok = Command::new(bin)
        .args(["generate", "--seed", "42", "--size", "256", "--out"])
        .arg(&out_dir)
        .arg("--preset")
        .arg(&preset_path)
        .output()
        .expect("run CLI with preset");
    assert!(
        ok.status.success(),
        "CLI preset failed: {}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let meta: RawWorldMeta =
        serde_json::from_str(&std::fs::read_to_string(out_dir.join("raw_meta.json")).unwrap())
            .unwrap();
    assert_eq!(meta.schema_tag, "PRESET-TEST-v2");
    assert!((meta.config.target_ocean_fraction - 0.44).abs() < 1e-6);
    assert!((meta.config.volcano_relief_m - 2950.0).abs() < 1e-6);

    // Invalid --size 129
    let bad_size = Command::new(bin)
        .args(["generate", "--seed", "42", "--size", "129", "--out"])
        .arg(guard.path().join("bad1"))
        .output()
        .expect("bad size");
    assert!(!bad_size.status.success());
    assert!(!bad_size.stderr.is_empty());

    // Missing --out
    let missing_out = Command::new(bin)
        .args(["generate", "--seed", "42", "--size", "256"])
        .output()
        .expect("missing out");
    assert!(!missing_out.status.success());
    assert!(!missing_out.stderr.is_empty());

    // Unknown flag
    let unknown_flag = Command::new(bin)
        .args([
            "generate",
            "--seed",
            "42",
            "--size",
            "256",
            "--out",
            "/tmp/unused_wv2",
            "--bogus",
        ])
        .output()
        .expect("unknown flag");
    assert!(!unknown_flag.status.success());
    assert!(!unknown_flag.stderr.is_empty());

    // Malformed / unknown-field / out-of-bounds TOML preset
    let bad_toml = guard.path().join("bad_unknown.toml");
    std::fs::write(&bad_toml, "unknown_key = 123\n").unwrap();
    let res_unknown = Command::new(bin)
        .args(["generate", "--seed", "42", "--size", "256", "--out"])
        .arg(guard.path().join("bad2"))
        .arg("--preset")
        .arg(&bad_toml)
        .output()
        .expect("bad toml key");
    assert!(!res_unknown.status.success());
    assert!(!res_unknown.stderr.is_empty());

    let oob_toml = guard.path().join("bad_oob.toml");
    std::fs::write(&oob_toml, "target_ocean_fraction = 0.75\n").unwrap();
    let res_oob = Command::new(bin)
        .args(["generate", "--seed", "42", "--size", "256", "--out"])
        .arg(guard.path().join("bad3"))
        .arg("--preset")
        .arg(&oob_toml)
        .output()
        .expect("bad toml oob");
    assert!(!res_oob.status.success());
    assert!(!res_oob.stderr.is_empty());
}
