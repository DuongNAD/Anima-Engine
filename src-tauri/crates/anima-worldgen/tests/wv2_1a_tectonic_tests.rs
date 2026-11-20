#![cfg(feature = "generate")]

use anima_worldgen::config::WorldV2Config;
use anima_worldgen::tectonic::generate_uplift;
use anima_worldgen::{ELEV_MAX_M, ELEV_MIN_M, LITHOLOGY_CLASS_COUNT};
use rayon::ThreadPoolBuilder;

#[test]
fn single_vs_multi_thread_rayon_invariance() {
    let cfg = WorldV2Config::for_size(0x2026_1120_u64, 256);

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
fn regional_massif_ocean_fraction_relief_and_lithology_invariants() {
    for &seed in &[11_u64, 1337_u64, 0x5756_3201_u64] {
        let cfg = WorldV2Config::for_size(seed, 256);
        let field = generate_uplift(&cfg).expect("valid tectonic field");

        let ocean_fraction = field.ocean_fraction();
        assert!(
            (0.40..=0.50).contains(&ocean_fraction),
            "seed {seed}: ocean_fraction {ocean_fraction} outside [0.40, 0.50]"
        );

        let mut min_h = f32::INFINITY;
        let mut max_h = f32::NEG_INFINITY;
        let mut min_k = f32::INFINITY;
        let mut max_k = f32::NEG_INFINITY;
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
            min_h = min_h.min(h);
            max_h = max_h.max(h);
            min_k = min_k.min(k);
            max_k = max_k.max(k);
            seen_classes[usize::from(id)] = true;
        }

        assert!(min_h <= -300.0, "seed {seed}: deep ocean min_h = {min_h}");
        assert!(
            (4_800.0..=ELEV_MAX_M).contains(&max_h),
            "seed {seed}: summit max_h = {max_h} did not cross tropical ELA band"
        );
        assert!(
            seen_classes.iter().all(|&present| present),
            "seed {seed}: missing lithology class in {seen_classes:?}"
        );
        let contrast = max_k / min_k;
        assert!(
            contrast > 1.0 && contrast <= 8.0 + 1e-6,
            "seed {seed}: erodibility contrast {contrast} outside (1, 8]"
        );
    }

    let a = generate_uplift(&WorldV2Config::for_size(11, 128)).unwrap();
    let b = generate_uplift(&WorldV2Config::for_size(22, 128)).unwrap();
    assert_ne!(a.sha256_bytes(), b.sha256_bytes());
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
