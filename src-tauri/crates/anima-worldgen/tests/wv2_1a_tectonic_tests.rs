#![cfg(feature = "generate")]

use anima_worldgen::config::WorldV2Config;
use anima_worldgen::tectonic::{generate_uplift, TectonicField};
use anima_worldgen::{ELEV_MAX_M, ELEV_MIN_M, LITHOLOGY_CLASS_COUNT, WORLD_EXTENT_M};
use rayon::ThreadPoolBuilder;

const EIGHT_TEST_SEEDS: [u64; 8] = [
    11_u64,
    42_u64,
    99_u64,
    1337_u64,
    7777_u64,
    0x2026_1120_u64,
    0x5756_3201_u64,
    0xDEAD_BEEF_u64,
];

fn ocean_fraction_in_cols(field: &TectonicField, col_start: usize, col_end: usize) -> f64 {
    let w = field.width as usize;
    let h = field.height as usize;
    let mut ocean = 0_usize;
    let mut count = 0_usize;
    for row in 0..h {
        let base = row * w;
        for col in col_start..col_end {
            if field.bedrock_h[base + col] < 0.0 {
                ocean += 1;
            }
            count += 1;
        }
    }
    ocean as f64 / count as f64
}

fn check_coastal_massif_structure(
    field: &TectonicField,
    domain_extent_m: f32,
) -> Result<(), String> {
    let w = field.width as usize;
    let h = field.height as usize;
    if w < 12 || h < 12 {
        return Err("grid too small for structural massif evaluation".to_string());
    }

    let west_edge_ocean = ocean_fraction_in_cols(field, 0, 1);
    if west_edge_ocean < 0.90 {
        return Err(format!(
            "west-edge column ocean fraction {west_edge_ocean:.3} < 0.90 (no western ocean margin)"
        ));
    }

    let t1 = w / 3;
    let t2 = (2 * w) / 3;
    let west_ocean = ocean_fraction_in_cols(field, 0, t1);
    let mid_ocean = ocean_fraction_in_cols(field, t1, t2);
    let east_ocean = ocean_fraction_in_cols(field, t2, w);

    if west_ocean < 0.85 {
        return Err(format!(
            "west-third ocean fraction {west_ocean:.3} < 0.85 (missing western continental margin)"
        ));
    }
    if east_ocean > 0.30 {
        return Err(format!(
            "east-third ocean fraction {east_ocean:.3} > 0.30 (planetary island or unanchored noise)"
        ));
    }
    if !(west_ocean > mid_ocean + 0.20 && mid_ocean > east_ocean + 0.05) {
        return Err(format!(
            "ocean share must decrease strictly west->mid->east (got {west_ocean:.3} -> {mid_ocean:.3} -> {east_ocean:.3})"
        ));
    }

    let mut east_sum = 0.0_f64;
    let mut east_count = 0_usize;
    for row in 0..h {
        let base = row * w;
        for col in t2..w {
            east_sum += f64::from(field.bedrock_h[base + col]);
            east_count += 1;
        }
    }
    let east_mean_m = east_sum / east_count as f64;
    if east_mean_m < 0.0 {
        return Err(format!(
            "east-third mean elevation {east_mean_m:.1} m < 0.0 m (foreland submerged)"
        ));
    }

    let mut max_h = f32::NEG_INFINITY;
    let mut s_row = 0_usize;
    let mut s_col = 0_usize;
    for row in 0..h {
        let base = row * w;
        for col in 0..w {
            let elev = field.bedrock_h[base + col];
            if elev > max_h {
                max_h = elev;
                s_row = row;
                s_col = col;
            }
        }
    }

    let summit_row = &field.bedrock_h[s_row * w..(s_row + 1) * w];
    let coast_col = summit_row
        .iter()
        .position(|&elev| elev >= 0.0)
        .ok_or_else(|| format!("summit row {s_row} has no positive coastline transition"))?;

    if s_col <= coast_col {
        return Err(format!(
            "summit column {s_col} is not inland of western coastline {coast_col}"
        ));
    }

    let cell_m = domain_extent_m / w as f32;
    let coast_offset_m = (s_col - coast_col) as f32 * cell_m;
    if !(3_000.0..=14_500.0).contains(&coast_offset_m) {
        return Err(format!(
            "summit coastline offset {coast_offset_m:.0} m outside [3000, 14500] m"
        ));
    }

    let flank_sum: f64 = summit_row[coast_col..=s_col]
        .iter()
        .map(|&v| f64::from(v))
        .sum();
    let flank_mean_m = flank_sum / (s_col - coast_col + 1) as f64;
    if f64::from(max_h) - flank_mean_m < 1_500.0 {
        return Err(format!(
            "oceanward flank relief contrast {:.1} m < 1500 m",
            f64::from(max_h) - flank_mean_m
        ));
    }

    Ok(())
}

#[test]
fn single_vs_multi_thread_rayon_invariance() {
    let cfg = WorldV2Config::for_size(0x2026_1120_u64, 512);

    let pool_1 = ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("1-thread rayon pool");
    let pool_4 = ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .expect("4-thread rayon pool");

    let run_1 = pool_1.install(|| generate_uplift(&cfg).expect("single-thread uplift"));
    let run_4 = pool_4.install(|| generate_uplift(&cfg).expect("multi-thread uplift"));

    assert_eq!(run_1.sha256_bytes(), run_4.sha256_bytes());
    assert_eq!(run_1.lithology_id, run_4.lithology_id);
    for (a, b) in run_1.bedrock_h.iter().zip(run_4.bedrock_h.iter()) {
        assert_eq!(a.to_bits(), b.to_bits());
    }
    for (a, b) in run_1.uplift_u.iter().zip(run_4.uplift_u.iter()) {
        assert_eq!(a.to_bits(), b.to_bits());
    }
}

#[test]
fn regional_massif_ocean_fraction_relief_lithology_and_structure() {
    let mut configs: Vec<(u64, u32)> = EIGHT_TEST_SEEDS.iter().map(|&s| (s, 256)).collect();
    configs.push((0x2026_1120_u64, 512));

    for (seed, size) in configs {
        let cfg = WorldV2Config::for_size(seed, size);
        let field = generate_uplift(&cfg).expect("valid tectonic field");

        let ocean_fraction = field.ocean_fraction();
        assert!(
            (0.40..=0.50).contains(&ocean_fraction),
            "seed {seed} ({size}x{size}): ocean_fraction {ocean_fraction} outside [0.40, 0.50]"
        );

        let mut min_h = f32::INFINITY;
        let mut max_h = f32::NEG_INFINITY;
        let mut min_k = f32::INFINITY;
        let mut max_k = f32::NEG_INFINITY;
        let mut pinned_at_bounds = 0_usize;
        let mut seen_classes = [false; LITHOLOGY_CLASS_COUNT];

        for ((&h, &u), (&id, &k)) in field
            .bedrock_h
            .iter()
            .zip(field.uplift_u.iter())
            .zip(field.lithology_id.iter().zip(field.stratum_k.iter()))
        {
            assert!(h.is_finite() && u.is_finite() && k.is_finite());
            assert!(u >= 0.0);
            assert!((ELEV_MIN_M..=ELEV_MAX_M).contains(&h));
            if (h - ELEV_MIN_M).abs() <= f32::EPSILON || (h - ELEV_MAX_M).abs() <= f32::EPSILON {
                pinned_at_bounds += 1;
            }
            min_h = min_h.min(h);
            max_h = max_h.max(h);
            min_k = min_k.min(k);
            max_k = max_k.max(k);
            seen_classes[usize::from(id)] = true;
        }

        let n = field.bedrock_h.len();
        assert!(
            pinned_at_bounds * 1_000 <= n,
            "seed {seed} ({size}x{size}): {pinned_at_bounds}/{n} cells pinned at vertical bounds (> 0.1%)"
        );
        assert!(
            min_h <= -300.0,
            "seed {seed} ({size}x{size}): deep ocean min_h = {min_h}"
        );
        assert!(
            (4_800.0..=ELEV_MAX_M).contains(&max_h),
            "seed {seed} ({size}x{size}): summit max_h = {max_h} did not cross tropical ELA band"
        );
        assert!(
            seen_classes.iter().all(|&present| present),
            "seed {seed} ({size}x{size}): missing lithology class in {seen_classes:?}"
        );
        let contrast = max_k / min_k;
        assert!(
            contrast > 1.0 && contrast <= 8.0 + 1e-6,
            "seed {seed} ({size}x{size}): erodibility contrast {contrast} outside (1, 8]"
        );

        let structural = check_coastal_massif_structure(&field, cfg.extent_m);
        assert!(
            structural.is_ok(),
            "seed {seed} ({size}x{size}): failed coastal-massif structural check: {structural:?}"
        );
    }

    let a = generate_uplift(&WorldV2Config::for_size(11, 128)).unwrap();
    let b = generate_uplift(&WorldV2Config::for_size(22, 128)).unwrap();
    assert_ne!(a.sha256_bytes(), b.sha256_bytes());
}

fn splitmix(mut x: u64) -> f64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    let bits = (x >> 11) as f64 / ((1_u64 << 53) as f64);
    bits * 2.0 - 1.0
}

fn value_noise(seed: u64, x: f64, y: f64) -> f64 {
    let xi = libm::floor(x) as i64;
    let yi = libm::floor(y) as i64;
    let tx = x - xi as f64;
    let ty = y - yi as f64;
    let corner = |cx: i64, cy: i64| -> f64 {
        let h = seed
            ^ (cx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (cy as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        splitmix(h)
    };
    let n00 = corner(xi, yi);
    let n10 = corner(xi + 1, yi);
    let n01 = corner(xi, yi + 1);
    let n11 = corner(xi + 1, yi + 1);
    let nx0 = n00 + tx * (n10 - n00);
    let nx1 = n01 + tx * (n11 - n01);
    nx0 + ty * (nx1 - nx0)
}

fn build_synthetic_field<F>(size: usize, raw_fn: F) -> TectonicField
where
    F: Fn(f64, f64) -> f64,
{
    let n = size * size;
    let denom = (size - 1) as f64;
    let mut raw = Vec::with_capacity(n);
    for row in 0..size {
        let v = row as f64 / denom;
        for col in 0..size {
            let u = col as f64 / denom;
            raw.push(raw_fn(u, v));
        }
    }
    let mut low = -20_000.0_f64;
    let mut high = 20_000.0_f64;
    for _ in 0..48 {
        let mid = 0.5 * (low + high);
        let frac = raw.iter().filter(|&&h| h + mid < 0.0).count() as f64 / n as f64;
        if frac > 0.45 {
            low = mid;
        } else {
            high = mid;
        }
    }
    let shift = 0.5 * (low + high);
    let bedrock_h = raw
        .into_iter()
        .map(|h| (h + shift).clamp(f64::from(ELEV_MIN_M), f64::from(ELEV_MAX_M)) as f32)
        .collect();
    TectonicField {
        width: size as u32,
        height: size as u32,
        bedrock_h,
        uplift_u: vec![0.0; n],
        lithology_id: vec![0; n],
        stratum_k: vec![1.0; n],
    }
}

#[test]
fn structural_check_rejects_pure_noise_and_centered_island() {
    let pure_noise =
        build_synthetic_field(256, |u, v| 9_000.0 * value_noise(42, u * 211.0, v * 211.0));
    let noise_res = check_coastal_massif_structure(&pure_noise, WORLD_EXTENT_M);
    assert!(
        noise_res.is_err(),
        "pure noise field unexpectedly passed coastal-massif structural check"
    );

    let centered_island = build_synthetic_field(256, |u, v| {
        let r = libm::hypot(u - 0.5, v - 0.5);
        20_000.0 * (0.40 - r) + 140.0 * value_noise(42, u * 14.0, v * 14.0)
    });
    let island_res = check_coastal_massif_structure(&centered_island, WORLD_EXTENT_M);
    assert!(
        island_res.is_err(),
        "centered planetary island unexpectedly passed coastal-massif structural check"
    );
}

#[test]
fn config_validation_rejects_invalid_bounds() {
    let mut bad = WorldV2Config::for_size(1, 32);
    assert!(bad.validate().is_err());

    bad = WorldV2Config::for_size(1, 129);
    assert!(bad.validate().is_err());

    bad = WorldV2Config::for_size(1, 128);
    bad.target_ocean_fraction = 0.55;
    assert!(bad.validate().is_err());

    bad = WorldV2Config::for_size(1, 128);
    bad.max_erodibility_contrast = 9.0;
    assert!(bad.validate().is_err());
}
