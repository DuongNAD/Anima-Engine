#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

//! Release-mode Card 3 verification suite for `anima-worldgen` (`WV2-2`):
//! runs the full `1024^2` 12-seed invariant sweep, `512^2` 11-seed biome-blending sweep,
//! `1024^2` Rayon thread invariance (`4t == 1t`), and optional `4096^2` end-to-end check.
//!
//! Usage:
//!   cargo run --release -p anima-worldgen --features "reader,generate,bake" --example wv2_2_release_verify
//!   cargo run --release -p anima-worldgen --features "reader,generate,bake" --example wv2_2_release_verify -- --include-4096

use anima_worldgen::caves::CaveNetwork;
use anima_worldgen::config::WorldV2Config;
use anima_worldgen::lakes_falls::LakesFallsConfig;
use anima_worldgen::pipeline::{read_bin_f32, read_bin_u8, read_bin_u8x4, run_worldgen_to_dir};
use anima_worldgen::{BiomeId, WaterKind};
use rayon::ThreadPoolBuilder;
use std::path::PathBuf;
use std::time::Instant;

struct TempDirGuard(PathBuf);

impl TempDirGuard {
    fn new(label: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("anima_wv2_2_rel_{}_{}", label, std::process::id()));
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

fn verify_card3_1024_sweep() {
    let size = 1024_u32;
    let n = (size as usize) * (size as usize);
    let scaled_floor =
        ((2500.0_f64 * (f64::from(size) / 4096.0) * (f64::from(size) / 4096.0)).ceil()) as usize;
    let seeds = [
        57_563_201_u64,
        0x5756_3201_u64,
        42_u64,
        1337_u64,
        314_159_u64,
        2027_u64,
        8191_u64,
        10_022_u64,
        777_777_u64,
        888_888_u64,
        1_234_567_u64,
        20_261_122_u64,
    ];
    let t0 = Instant::now();
    for &seed in &seeds {
        let guard = TempDirGuard::new(&format!("card3_1024_{seed}"));
        let cfg = WorldV2Config::for_size(seed, size);
        let meta = run_worldgen_to_dir(&cfg, guard.path()).expect("pipeline 1024");
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
        let min_sp = LakesFallsConfig::default().waterfall_min_spacing_m;
        let wk = read_bin_u8(&guard.path().join("water_kind.bin"), n).expect("water_kind");
        for (wf_idx, wf) in meta.waterfalls.iter().enumerate() {
            assert!(
                wf.drop_m >= 12.0,
                "seed={seed}: waterfall {wf_idx} drop_m < 12.0"
            );
            assert!(
                wf.plunge_depth_m > 0.0,
                "seed={seed}: waterfall {wf_idx} plunge_depth_m={} <= 0.0",
                wf.plunge_depth_m
            );
            assert_eq!(wk[wf.base_cell as usize], WaterKind::Waterfall as u8);
            for other in &meta.waterfalls[(wf_idx + 1)..] {
                let d = libm::hypotf(wf.x_m - other.x_m, wf.z_m - other.z_m);
                assert!(
                    d >= min_sp - 1e-2,
                    "seed={seed}: waterfall spacing {d} < {min_sp}"
                );
            }
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
    println!(
        "PASS card3_1024_12seeds ({} seeds) in {:.2}s",
        seeds.len(),
        t0.elapsed().as_secs_f64()
    );
}

fn verify_biome_512_sweep() {
    let size = 512_u32;
    let n = (size as usize) * (size as usize);
    let scaled_floor =
        ((2500.0_f64 * (f64::from(size) / 4096.0) * (f64::from(size) / 4096.0)).ceil()) as usize;
    let seeds = [
        0x0057_5632_2026_1120_u64,
        57_563_201_u64,
        0x5756_3201_u64,
        42_u64,
        1337_u64,
        314_159_u64,
        2027_u64,
        8191_u64,
        10_022_u64,
        777_777_u64,
        19_283_746_u64,
    ];
    let t0 = Instant::now();
    for &seed in &seeds {
        let guard = TempDirGuard::new(&format!("biomes_512_{seed}"));
        let cfg = WorldV2Config::for_size(seed, size);
        let meta = run_worldgen_to_dir(&cfg, guard.path()).expect("full pipeline 512");
        assert!((0.40..=0.50).contains(&meta.stats.ocean_fraction));
        assert!((0.008..=0.045).contains(&meta.stats.lake_surface_fraction));
        assert!((0.0005..=0.0030).contains(&meta.stats.river_surface_fraction));
        assert_eq!(meta.stats.severed_humid_spillways, 0);
        assert_eq!(meta.stats.unmodeled_micro_pits, 0);
        assert_eq!(meta.stats.endorheic_humid_fraction, 0.0);
        assert!(meta.stats.endorheic_land_fraction <= 0.10);
        assert!((30..=48).contains(&meta.stats.cave_entrance_count));
        assert_eq!(meta.stats.biome_counts[BiomeId::Chaparral as usize], 0);
        assert_eq!(meta.stats.biome_counts[BiomeId::Steppe as usize], 0);
        for &b in &BiomeId::ZONED_TERRESTRIAL_16 {
            assert!(meta.stats.biome_counts[b as usize] >= scaled_floor);
        }
        assert!((0.15..=0.55).contains(&meta.stats.ecotone_fraction));
        assert!(meta.stats.non_cliff_jump_fraction < 0.03);

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
                assert!(precip[i] < 0.48);
                assert!(moist[i] < 0.28);
                assert_ne!(lith[i], 1);
            }
        }
        assert!(dune_cells > 0);
    }
    println!(
        "PASS biomes_512_11seeds ({} seeds) in {:.2}s",
        seeds.len(),
        t0.elapsed().as_secs_f64()
    );
}

fn verify_1024_rayon_invariance() {
    let t0 = Instant::now();
    let g_a = TempDirGuard::new("1024_4t_a");
    let g_b = TempDirGuard::new("1024_4t_b");
    let g_1t = TempDirGuard::new("1024_1t");
    let g_other = TempDirGuard::new("1024_seed2");

    let cfg = WorldV2Config::for_size(0x5756_3201, 1024);
    let pool_4 = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    let pool_1 = ThreadPoolBuilder::new().num_threads(1).build().unwrap();

    let m_a = pool_4.install(|| run_worldgen_to_dir(&cfg, g_a.path()).expect("1024 4t a"));
    let m_b = pool_4.install(|| run_worldgen_to_dir(&cfg, g_b.path()).expect("1024 4t b"));
    let m_1t = pool_1.install(|| run_worldgen_to_dir(&cfg, g_1t.path()).expect("1024 1t"));

    assert_eq!(m_a.world_sha256, m_b.world_sha256);
    assert_eq!(m_a.world_sha256, m_1t.world_sha256);
    assert_eq!(m_a.layers, m_1t.layers);
    assert_eq!(m_a.cave_network_sha256, m_1t.cave_network_sha256);

    let c_a = std::fs::read(g_a.path().join("cave_network.bin")).unwrap();
    let c_1t = std::fs::read(g_1t.path().join("cave_network.bin")).unwrap();
    assert_eq!(c_a, c_1t);

    let cfg2 = WorldV2Config::for_size(0x2026_1120, 1024);
    let m_other =
        pool_4.install(|| run_worldgen_to_dir(&cfg2, g_other.path()).expect("1024 seed2"));
    assert_ne!(m_a.world_sha256, m_other.world_sha256);
    println!(
        "PASS rayon_1024_invariance (sha256={}) in {:.2}s",
        m_a.world_sha256,
        t0.elapsed().as_secs_f64()
    );
}

fn verify_4096_full_pipeline() {
    let t0 = Instant::now();
    let guard = TempDirGuard::new("4096_full");
    let cfg = WorldV2Config::for_size(0x5756_3201, 4096);
    let meta = run_worldgen_to_dir(&cfg, guard.path()).expect("pipeline 4096");
    assert_eq!(meta.width, 4096);
    assert_eq!(meta.height, 4096);
    assert_eq!(meta.layers.len(), 29);
    for layer in &meta.layers {
        let fmeta = std::fs::metadata(guard.path().join(&layer.filename)).expect("layer exists");
        assert_eq!(fmeta.len(), layer.byte_len);
    }
    let cave_bytes =
        std::fs::read(guard.path().join("cave_network.bin")).expect("cave_network.bin");
    let net = CaveNetwork::from_bytes(&cave_bytes).expect("valid ANMC v1");
    assert!((30..=48).contains(&net.entrances.len()));
    assert_eq!(meta.stats.severed_humid_spillways, 0);
    assert_eq!(meta.stats.unmodeled_micro_pits, 0);
    assert_eq!(meta.stats.endorheic_humid_fraction, 0.0);
    println!(
        "PASS full_4096 (world_sha256={}, waterfalls={}, caves={}) in {:.2}s",
        meta.world_sha256,
        meta.stats.waterfall_count,
        net.entrances.len(),
        t0.elapsed().as_secs_f64()
    );
}

fn main() {
    let include_4096 = std::env::args().any(|a| a == "--include-4096");
    verify_card3_1024_sweep();
    verify_biome_512_sweep();
    verify_1024_rayon_invariance();
    if include_4096 {
        verify_4096_full_pipeline();
    }
}
