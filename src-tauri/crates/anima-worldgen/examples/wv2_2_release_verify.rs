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
use anima_worldgen::lakes_falls::{LakeClassification, LakesFallsConfig};
use anima_worldgen::pipeline::{read_bin_f32, read_bin_u8, read_bin_u8x4, run_worldgen_to_dir};
use anima_worldgen::tectonic::LithologyClass;
use anima_worldgen::{BiomeId, WaterKind};
use rayon::ThreadPoolBuilder;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::Instant;

const STREAM_CHUNK_ELEMS: usize = 65_536;

fn read_chunk_f32(r: &mut BufReader<File>, dst: &mut Vec<f32>, elems: usize) {
    dst.clear();
    let mut raw = vec![0_u8; elems * 4];
    r.read_exact(&mut raw).expect("read f32 chunk");
    let (chunks, _) = raw.as_chunks::<4>();
    for &ch in chunks {
        dst.push(f32::from_le_bytes(ch));
    }
}

fn read_chunk_u8(r: &mut BufReader<File>, dst: &mut Vec<u8>, elems: usize) {
    dst.resize(elems, 0);
    r.read_exact(dst).expect("read u8 chunk");
}

fn read_chunk_u8x4(r: &mut BufReader<File>, dst: &mut Vec<[u8; 4]>, elems: usize) {
    dst.clear();
    let mut raw = vec![0_u8; elems * 4];
    r.read_exact(&mut raw).expect("read u8x4 chunk");
    let (chunks, _) = raw.as_chunks::<4>();
    dst.extend_from_slice(chunks);
}

/// Streams `(total_h_m, water_kind, precip_m_yr, pet_m_yr)` in `65_536`-element chunks (`<= 1 MiB`)
/// and returns `(endorheic_humid_total_frac, frac_p_ge_pet, frac_0_5_to_1_0)`.
fn stream_endorheic_humid_breakdown(dir: &Path, len: usize) -> (f32, f32, f32) {
    let mut r_h = BufReader::new(File::open(dir.join("total_h_m.bin")).expect("total_h"));
    let mut r_wk = BufReader::new(File::open(dir.join("water_kind.bin")).expect("water_kind"));
    let mut r_p = BufReader::new(File::open(dir.join("precip_m_yr.bin")).expect("precip"));
    let mut r_pet = BufReader::new(File::open(dir.join("pet_m_yr.bin")).expect("pet"));

    let mut b_h = Vec::with_capacity(STREAM_CHUNK_ELEMS);
    let mut b_wk = Vec::with_capacity(STREAM_CHUNK_ELEMS);
    let mut b_p = Vec::with_capacity(STREAM_CHUNK_ELEMS);
    let mut b_pet = Vec::with_capacity(STREAM_CHUNK_ELEMS);

    let mut land = 0_usize;
    let mut p_ge_pet = 0_usize;
    let mut p_half_to_1 = 0_usize;
    let mut rem = len;
    while rem > 0 {
        let take = rem.min(STREAM_CHUNK_ELEMS);
        read_chunk_f32(&mut r_h, &mut b_h, take);
        read_chunk_u8(&mut r_wk, &mut b_wk, take);
        read_chunk_f32(&mut r_p, &mut b_p, take);
        read_chunk_f32(&mut r_pet, &mut b_pet, take);
        for i in 0..take {
            if b_h[i] > 0.0 {
                land += 1;
            }
            if b_wk[i] == WaterKind::EndorheicLake as u8 {
                let ratio = b_p[i] / b_pet[i].max(0.05);
                if ratio >= 1.0 {
                    p_ge_pet += 1;
                } else if ratio >= 0.5 {
                    p_half_to_1 += 1;
                }
            }
        }
        rem -= take;
    }
    let denom = land.max(1) as f32;
    (
        (p_ge_pet + p_half_to_1) as f32 / denom,
        p_ge_pet as f32 / denom,
        p_half_to_1 as f32 / denom,
    )
}

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
    let mut violations: Vec<String> = Vec::new();
    for &seed in &seeds {
        let guard = TempDirGuard::new(&format!("card3_1024_{seed}"));
        let cfg = WorldV2Config::for_size(seed, size);
        let meta = run_worldgen_to_dir(&cfg, guard.path()).expect("pipeline 1024");
        let large_lakes = meta.basins.iter().filter(|b| b.area_m2 > 100_000.0).count();
        let paramo_lakes = meta
            .basins
            .iter()
            .filter(|b| b.mean_elev_m > 3_300.0)
            .count();
        let caldera_or_trough = meta
            .basins
            .iter()
            .filter(|b| {
                b.classification == LakeClassification::CalderaOrTrough
                    || b.max_wall_slope_deg > 25.0
                    || b.elongation > 3.5
            })
            .count();
        let small_paramo_ponds = meta
            .basins
            .iter()
            .filter(|b| b.mean_elev_m > 3_300.0 && b.area_m2 < 100_000.0)
            .count();
        let (eh_tot, eh_ge1, eh_half) = stream_endorheic_humid_breakdown(guard.path(), n);
        let dune = read_bin_f32(&guard.path().join("dune_h_m.bin"), n).expect("dune");
        let dune_cells = dune.iter().filter(|&&d| d > 0.0).count();
        drop(dune);
        let min_b16 = BiomeId::ZONED_TERRESTRIAL_16
            .iter()
            .map(|&b| meta.stats.biome_counts[b as usize])
            .min()
            .unwrap_or(0);

        println!(
            "1024 seed={seed:<10} | wf={:<3} | riv={:.6} | lake={:.6} (lg={large_lakes}, par={paramo_lakes}, cald={caldera_or_trough}, ponds={small_paramo_ponds}, s4pond={}) | sev={} pit={} | endo_hum={:.6} (p>=pet={:.6}, 0.5..1={:.6}) endo_land={:.6} | caves={} | eco={:.4} jump={:.5} min16={min_b16} dune={dune_cells}",
            meta.stats.waterfall_count,
            meta.stats.river_surface_fraction,
            meta.stats.lake_surface_fraction,
            meta.stats.micro_pond_count,
            meta.stats.severed_humid_spillways,
            meta.stats.unmodeled_micro_pits,
            eh_tot,
            eh_ge1,
            eh_half,
            meta.stats.endorheic_land_fraction,
            meta.stats.cave_entrance_count,
            meta.stats.ecotone_fraction,
            meta.stats.non_cliff_jump_fraction,
        );

        if !(20..=80).contains(&meta.stats.waterfall_count) {
            violations.push(format!(
                "seed={seed}: waterfall_count={} outside 20..=80",
                meta.stats.waterfall_count
            ));
        }
        if !(0.0005..=0.0030).contains(&meta.stats.river_surface_fraction) {
            violations.push(format!(
                "seed={seed}: river_surface_fraction={} outside [0.0005, 0.0030]",
                meta.stats.river_surface_fraction
            ));
        }
        if !(0.008..=0.045).contains(&meta.stats.lake_surface_fraction) {
            violations.push(format!(
                "seed={seed}: lake_surface_fraction={} outside [0.008, 0.045]",
                meta.stats.lake_surface_fraction
            ));
        }
        if large_lakes < 5 {
            violations.push(format!("seed={seed}: large_lakes={large_lakes} < 5"));
        }
        if meta.stats.severed_humid_spillways != 0 {
            violations.push(format!(
                "seed={seed}: severed_humid_spillways={} != 0",
                meta.stats.severed_humid_spillways
            ));
        }
        if meta.stats.unmodeled_micro_pits != 0 {
            violations.push(format!(
                "seed={seed}: unmodeled_micro_pits={} != 0",
                meta.stats.unmodeled_micro_pits
            ));
        }
        if meta.stats.endorheic_humid_fraction != 0.0 {
            violations.push(format!(
                "seed={seed}: endorheic_humid_fraction={} (p>=pet={eh_ge1}, 0.5..1={eh_half}) != 0.0",
                meta.stats.endorheic_humid_fraction
            ));
        }
        if !(30..=48).contains(&meta.stats.cave_entrance_count) {
            violations.push(format!(
                "seed={seed}: cave_entrance_count={} outside [30, 48]",
                meta.stats.cave_entrance_count
            ));
        }
        let min_sp = LakesFallsConfig::default().waterfall_min_spacing_m;
        let wk = read_bin_u8(&guard.path().join("water_kind.bin"), n).expect("water_kind");
        for (wf_idx, wf) in meta.waterfalls.iter().enumerate() {
            if wf.drop_m < 12.0 {
                violations.push(format!(
                    "seed={seed}: waterfall {wf_idx} drop_m={} < 12.0",
                    wf.drop_m
                ));
            }
            if wf.plunge_depth_m <= 0.0 {
                violations.push(format!(
                    "seed={seed}: waterfall {wf_idx} plunge_depth_m={} <= 0.0",
                    wf.plunge_depth_m
                ));
            }
            if wk[wf.base_cell as usize] != WaterKind::Waterfall as u8 {
                violations.push(format!(
                    "seed={seed}: waterfall {wf_idx} base_cell not WaterKind::Waterfall"
                ));
            }
            for other in &meta.waterfalls[(wf_idx + 1)..] {
                let d = libm::hypotf(wf.x_m - other.x_m, wf.z_m - other.z_m);
                if d < min_sp - 1e-2 {
                    violations.push(format!("seed={seed}: waterfall spacing {d} < {min_sp}"));
                }
            }
        }
        if !(0.15..=0.55).contains(&meta.stats.ecotone_fraction) {
            violations.push(format!(
                "seed={seed}: ecotone_fraction={} outside [0.15, 0.55]",
                meta.stats.ecotone_fraction
            ));
        }
        if meta.stats.non_cliff_jump_fraction >= 0.03 {
            violations.push(format!(
                "seed={seed}: non_cliff_jump_fraction={} >= 0.03",
                meta.stats.non_cliff_jump_fraction
            ));
        }
        for &b in &BiomeId::ZONED_TERRESTRIAL_16 {
            let cnt = meta.stats.biome_counts[b as usize];
            if cnt < scaled_floor {
                violations.push(format!(
                    "seed={seed}: biome {b:?} has {cnt} < {scaled_floor}"
                ));
            }
        }
        if dune_cells == 0 {
            violations.push(format!("seed={seed}: expected dune_cells > 0"));
        }
    }
    assert!(
        violations.is_empty(),
        "card3_1024_12seeds failure(s) ({} total):\n  - {}",
        violations.len(),
        violations.join("\n  - ")
    );
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
    let mut violations: Vec<String> = Vec::new();
    for &seed in &seeds {
        let guard = TempDirGuard::new(&format!("biomes_512_{seed}"));
        let cfg = WorldV2Config::for_size(seed, size);
        let meta = run_worldgen_to_dir(&cfg, guard.path()).expect("full pipeline 512");
        let large_lakes = meta.basins.iter().filter(|b| b.area_m2 > 100_000.0).count();
        let paramo_lakes = meta
            .basins
            .iter()
            .filter(|b| b.mean_elev_m > 3_300.0)
            .count();
        let small_paramo_ponds = meta
            .basins
            .iter()
            .filter(|b| b.mean_elev_m > 3_300.0 && b.area_m2 < 100_000.0)
            .count();
        let (eh_tot, eh_ge1, eh_half) = stream_endorheic_humid_breakdown(guard.path(), n);

        let ids = read_bin_u8x4(&guard.path().join("biome_ids.bin"), n).expect("ids");
        let wts = read_bin_u8x4(&guard.path().join("biome_weights.bin"), n).expect("wts");
        let dune = read_bin_f32(&guard.path().join("dune_h_m.bin"), n).expect("dune");
        let precip = read_bin_f32(&guard.path().join("precip_m_yr.bin"), n).expect("precip");
        let moist = read_bin_f32(&guard.path().join("moisture.bin"), n).expect("moist");
        let lith = read_bin_u8(&guard.path().join("lithology_id.bin"), n).expect("lith");
        for (id4, w4) in ids.iter().zip(wts.iter()) {
            if w4.iter().map(|&w| u16::from(w)).sum::<u16>() != 255 {
                violations.push(format!("seed={seed}: biome_weights sum != 255"));
                break;
            }
            if id4.contains(&(BiomeId::Chaparral as u8)) || id4.contains(&(BiomeId::Steppe as u8)) {
                violations.push(format!("seed={seed}: forbidden biome in top4"));
                break;
            }
        }
        let mut dune_cells = 0usize;
        for i in 0..n {
            if dune[i] > 0.0 {
                dune_cells += 1;
                if precip[i] >= 0.48 || moist[i] >= 0.28 || lith[i] == 1 {
                    violations.push(format!(
                        "seed={seed}: invalid dune cell i={i} p={} m={} lith={}",
                        precip[i], moist[i], lith[i]
                    ));
                    break;
                }
            }
        }
        let min_b16 = BiomeId::ZONED_TERRESTRIAL_16
            .iter()
            .map(|&b| meta.stats.biome_counts[b as usize])
            .min()
            .unwrap_or(0);

        println!(
            "512  seed={seed:<20} | wf={:<2} | riv={:.6} | lake={:.6} (lg={large_lakes}, par={paramo_lakes}, ponds={small_paramo_ponds}) | endo_hum={:.6} (p>=pet={:.6}, 0.5..1={:.6}) endo_land={:.6} | caves={} | eco={:.4} jump={:.5} min16={min_b16} dune={dune_cells}",
            meta.stats.waterfall_count,
            meta.stats.river_surface_fraction,
            meta.stats.lake_surface_fraction,
            eh_tot,
            eh_ge1,
            eh_half,
            meta.stats.endorheic_land_fraction,
            meta.stats.cave_entrance_count,
            meta.stats.ecotone_fraction,
            meta.stats.non_cliff_jump_fraction,
        );

        if !(0.40..=0.50).contains(&meta.stats.ocean_fraction) {
            violations.push(format!(
                "seed={seed}: ocean_fraction={} outside [0.40, 0.50]",
                meta.stats.ocean_fraction
            ));
        }
        if !(0.008..=0.045).contains(&meta.stats.lake_surface_fraction) {
            violations.push(format!(
                "seed={seed}: lake_surface_fraction={} outside [0.008, 0.045]",
                meta.stats.lake_surface_fraction
            ));
        }
        if !(0.0005..=0.0030).contains(&meta.stats.river_surface_fraction) {
            violations.push(format!(
                "seed={seed}: river_surface_fraction={} outside [0.0005, 0.0030]",
                meta.stats.river_surface_fraction
            ));
        }
        if meta.stats.severed_humid_spillways != 0 {
            violations.push(format!(
                "seed={seed}: severed_humid_spillways={}",
                meta.stats.severed_humid_spillways
            ));
        }
        if meta.stats.unmodeled_micro_pits != 0 {
            violations.push(format!(
                "seed={seed}: unmodeled_micro_pits={}",
                meta.stats.unmodeled_micro_pits
            ));
        }
        if meta.stats.endorheic_humid_fraction != 0.0 {
            violations.push(format!(
                "seed={seed}: endorheic_humid_fraction={} (p>=pet={eh_ge1}, 0.5..1={eh_half}) != 0.0",
                meta.stats.endorheic_humid_fraction
            ));
        }
        if meta.stats.endorheic_land_fraction > 0.10 {
            violations.push(format!(
                "seed={seed}: endorheic_land_fraction={} > 0.10",
                meta.stats.endorheic_land_fraction
            ));
        }
        if !(30..=48).contains(&meta.stats.cave_entrance_count) {
            violations.push(format!(
                "seed={seed}: cave_entrance_count={} outside [30, 48]",
                meta.stats.cave_entrance_count
            ));
        }
        if meta.stats.biome_counts[BiomeId::Chaparral as usize] != 0
            || meta.stats.biome_counts[BiomeId::Steppe as usize] != 0
        {
            violations.push(format!("seed={seed}: forbidden biome count != 0"));
        }
        for &b in &BiomeId::ZONED_TERRESTRIAL_16 {
            if meta.stats.biome_counts[b as usize] < scaled_floor {
                violations.push(format!(
                    "seed={seed}: biome {b:?} has {} < {scaled_floor}",
                    meta.stats.biome_counts[b as usize]
                ));
            }
        }
        if !(0.15..=0.55).contains(&meta.stats.ecotone_fraction) {
            violations.push(format!(
                "seed={seed}: ecotone_fraction={} outside [0.15, 0.55]",
                meta.stats.ecotone_fraction
            ));
        }
        if meta.stats.non_cliff_jump_fraction >= 0.03 {
            violations.push(format!(
                "seed={seed}: non_cliff_jump_fraction={} >= 0.03",
                meta.stats.non_cliff_jump_fraction
            ));
        }
        if dune_cells == 0 {
            violations.push(format!("seed={seed}: expected dune_cells > 0"));
        }
    }
    assert!(
        violations.is_empty(),
        "biomes_512_11seeds failure(s) ({} total):\n  - {}",
        violations.len(),
        violations.join("\n  - ")
    );
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

    assert_eq!(m_a, m_b);
    assert_eq!(m_a, m_1t);

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
    let g_a = TempDirGuard::new("4096_full_a");
    let g_b = TempDirGuard::new("4096_full_b");
    let cfg = WorldV2Config::for_size(0x5756_3201, 4096);
    let m_a = run_worldgen_to_dir(&cfg, g_a.path()).expect("pipeline 4096 run A");
    let m_b = run_worldgen_to_dir(&cfg, g_b.path()).expect("pipeline 4096 run B");
    assert_eq!(m_a.width, 4096);
    assert_eq!(m_a.height, 4096);
    assert_eq!(m_a.layers.len(), 29);
    for layer in &m_a.layers {
        let fmeta = std::fs::metadata(g_a.path().join(&layer.filename)).expect("layer exists");
        assert_eq!(fmeta.len(), layer.byte_len);
    }
    let cave_a = std::fs::read(g_a.path().join("cave_network.bin")).expect("cave_network.bin A");
    let cave_b = std::fs::read(g_b.path().join("cave_network.bin")).expect("cave_network.bin B");
    assert_eq!(
        m_a, m_b,
        "4096^2 metadata must be identical across two runs"
    );
    assert_eq!(
        cave_a, cave_b,
        "4096^2 cave_network.bin must be byte-identical"
    );
    let net = CaveNetwork::from_bytes(&cave_a).expect("valid ANMC v1");
    drop(g_b);
    drop(cave_b);
    drop(cave_a);

    let n = 4096_usize * 4096_usize;
    let lith = read_bin_u8(&g_a.path().join("lithology_id.bin"), n).expect("lithology_id");
    let caprock_bad = m_a
        .waterfalls
        .iter()
        .filter(|wf| lith[wf.lip_cell as usize] != LithologyClass::SandstoneCaprock as u8)
        .count();
    drop(lith);

    let wk = read_bin_u8(&g_a.path().join("water_kind.bin"), n).expect("water_kind");
    let wf_base_bad = m_a
        .waterfalls
        .iter()
        .filter(|wf| wk[wf.base_cell as usize] != WaterKind::Waterfall as u8)
        .count();
    drop(wk);

    let min_sp = LakesFallsConfig::default().waterfall_min_spacing_m;
    let mut min_drop = f32::INFINITY;
    let mut min_plunge = f32::INFINITY;
    let mut min_spacing = f32::INFINITY;
    for (i, wf) in m_a.waterfalls.iter().enumerate() {
        if wf.drop_m < min_drop {
            min_drop = wf.drop_m;
        }
        if wf.plunge_depth_m < min_plunge {
            min_plunge = wf.plunge_depth_m;
        }
        for other in &m_a.waterfalls[(i + 1)..] {
            let d = libm::hypotf(wf.x_m - other.x_m, wf.z_m - other.z_m);
            if d < min_spacing {
                min_spacing = d;
            }
        }
    }

    let large_lakes = m_a.basins.iter().filter(|b| b.area_m2 > 100_000.0).count();
    let paramo_lakes = m_a
        .basins
        .iter()
        .filter(|b| b.mean_elev_m > 3_300.0)
        .count();
    let caldera_or_trough = m_a
        .basins
        .iter()
        .filter(|b| {
            b.classification == LakeClassification::CalderaOrTrough
                || b.max_wall_slope_deg > 25.0
                || b.elongation > 3.5
        })
        .count();
    let small_paramo_ponds = m_a
        .basins
        .iter()
        .filter(|b| b.mean_elev_m > 3_300.0 && b.area_m2 < 100_000.0)
        .count();

    let mut r_h = BufReader::new(File::open(g_a.path().join("total_h_m.bin")).expect("total_h"));
    let mut r_ela = BufReader::new(File::open(g_a.path().join("ela_m.bin")).expect("ela_m"));
    let mut r_glac =
        BufReader::new(File::open(g_a.path().join("glacier_h_m.bin")).expect("glacier_h_m"));
    let mut r_dom =
        BufReader::new(File::open(g_a.path().join("dominant_biome.bin")).expect("dominant_biome"));
    let mut b_h = Vec::with_capacity(STREAM_CHUNK_ELEMS);
    let mut b_ela = Vec::with_capacity(STREAM_CHUNK_ELEMS);
    let mut b_glac = Vec::with_capacity(STREAM_CHUNK_ELEMS);
    let mut b_dom = Vec::with_capacity(STREAM_CHUNK_ELEMS);
    let mut ela_min = f32::INFINITY;
    let mut ela_max = f32::NEG_INFINITY;
    let mut sub_ela_violations = 0_usize;
    let mut rem = n;
    while rem > 0 {
        let take = rem.min(STREAM_CHUNK_ELEMS);
        read_chunk_f32(&mut r_h, &mut b_h, take);
        read_chunk_f32(&mut r_ela, &mut b_ela, take);
        read_chunk_f32(&mut r_glac, &mut b_glac, take);
        read_chunk_u8(&mut r_dom, &mut b_dom, take);
        for i in 0..take {
            let e = b_ela[i];
            if e < ela_min {
                ela_min = e;
            }
            if e > ela_max {
                ela_max = e;
            }
            if b_h[i] < e
                && (b_glac[i] > 0.0
                    || b_dom[i] == BiomeId::Snow as u8
                    || b_dom[i] == BiomeId::Glacier as u8)
            {
                sub_ela_violations += 1;
            }
        }
        rem -= take;
    }
    drop(b_h);
    drop(b_ela);
    drop(b_glac);
    drop(b_dom);

    let mut r_ids =
        BufReader::new(File::open(g_a.path().join("biome_ids.bin")).expect("biome_ids"));
    let mut r_wts =
        BufReader::new(File::open(g_a.path().join("biome_weights.bin")).expect("biome_weights"));
    let mut b_ids = Vec::with_capacity(STREAM_CHUNK_ELEMS);
    let mut b_wts = Vec::with_capacity(STREAM_CHUNK_ELEMS);
    let mut weight_sum_bad = 0_usize;
    let mut forbidden_top4 = 0_usize;
    let mut rem_b = n;
    while rem_b > 0 {
        let take = rem_b.min(STREAM_CHUNK_ELEMS);
        read_chunk_u8x4(&mut r_ids, &mut b_ids, take);
        read_chunk_u8x4(&mut r_wts, &mut b_wts, take);
        for (id4, w4) in b_ids.iter().zip(b_wts.iter()) {
            if w4.iter().map(|&w| u16::from(w)).sum::<u16>() != 255 {
                weight_sum_bad += 1;
            }
            if id4.contains(&(BiomeId::Chaparral as u8)) || id4.contains(&(BiomeId::Steppe as u8)) {
                forbidden_top4 += 1;
            }
        }
        rem_b -= take;
    }
    drop(b_ids);
    drop(b_wts);

    let mut r_dune = BufReader::new(File::open(g_a.path().join("dune_h_m.bin")).expect("dune_h_m"));
    let mut b_dune = Vec::with_capacity(STREAM_CHUNK_ELEMS);
    let mut dune_cells = 0_usize;
    let mut rem_d = n;
    while rem_d > 0 {
        let take = rem_d.min(STREAM_CHUNK_ELEMS);
        read_chunk_f32(&mut r_dune, &mut b_dune, take);
        dune_cells += b_dune.iter().filter(|&&d| d > 0.0).count();
        rem_d -= take;
    }
    drop(b_dune);

    let (eh_tot, eh_ge1, eh_half) = stream_endorheic_humid_breakdown(g_a.path(), n);

    println!(
        "=== 4096^2 REPORT (seed=0x{:x}, elapsed={:.2}s) ===",
        cfg.seed,
        t0.elapsed().as_secs_f64()
    );
    println!("world_sha256_a:       {}", m_a.world_sha256);
    println!("world_sha256_b:       {}", m_b.world_sha256);
    println!("cave_network_sha256:  {}", m_a.cave_network_sha256);
    println!("ocean_fraction:       {:.6}", m_a.stats.ocean_fraction);
    println!(
        "waterfalls:           count={}, min_drop={:.3}, min_plunge={:.3}, min_spacing={:.3}, caprock_bad={}, base_bad={}",
        m_a.waterfalls.len(),
        min_drop,
        min_plunge,
        min_spacing,
        caprock_bad,
        wf_base_bad
    );
    println!(
        "lakes_c3:             lake_surface_fraction={:.6}, large_lakes={}, paramo_lakes={}, caldera_or_trough={}, small_paramo_ponds={}, severed_humid_spillways={}, unmodeled_micro_pits={}, endorheic_humid_fraction={:.6} (stream_tot={:.6}, p>=pet={:.6}, 0.5<=p/pet<1.0={:.6}), endorheic_land_fraction={:.6}",
        m_a.stats.lake_surface_fraction,
        large_lakes,
        paramo_lakes,
        caldera_or_trough,
        small_paramo_ponds,
        m_a.stats.severed_humid_spillways,
        m_a.stats.unmodeled_micro_pits,
        m_a.stats.endorheic_humid_fraction,
        eh_tot,
        eh_ge1,
        eh_half,
        m_a.stats.endorheic_land_fraction
    );
    println!(
        "rivers_caves:         river_surface_fraction={:.6}, cave_entrance_count={} (ANMC={})",
        m_a.stats.river_surface_fraction,
        m_a.stats.cave_entrance_count,
        net.entrances.len()
    );
    println!(
        "biomes_c1_c2:         ecotone_fraction={:.6}, non_cliff_jump_fraction={:.6}, chaparral={}, steppe={}, weight_sum_bad={}, forbidden_top4={}, dune_cells={}",
        m_a.stats.ecotone_fraction,
        m_a.stats.non_cliff_jump_fraction,
        m_a.stats.biome_counts[BiomeId::Chaparral as usize],
        m_a.stats.biome_counts[BiomeId::Steppe as usize],
        weight_sum_bad,
        forbidden_top4,
        dune_cells
    );
    println!(
        "ela_nival:            ela_min={:.2}, ela_max={:.2}, sub_ela_violations={}",
        ela_min, ela_max, sub_ela_violations
    );
    for &b in &BiomeId::ZONED_TERRESTRIAL_16 {
        println!(
            "  biome {:<11} count={}",
            format!("{b:?}"),
            m_a.stats.biome_counts[b as usize]
        );
    }

    let mut violations: Vec<String> = Vec::new();
    if !(0.40..=0.50).contains(&m_a.stats.ocean_fraction) {
        violations.push(format!(
            "ocean_fraction={} outside [0.40, 0.50]",
            m_a.stats.ocean_fraction
        ));
    }
    if !(20..=80).contains(&m_a.waterfalls.len()) {
        violations.push(format!(
            "waterfall_count={} outside 20..=80",
            m_a.waterfalls.len()
        ));
    }
    if !m_a.waterfalls.is_empty() && min_drop < 12.0 {
        violations.push(format!("min_drop={min_drop} < 12.0"));
    }
    if !m_a.waterfalls.is_empty() && min_plunge <= 0.0 {
        violations.push(format!("min_plunge={min_plunge} <= 0.0"));
    }
    if m_a.waterfalls.len() >= 2 && min_spacing < min_sp - 1e-2 {
        violations.push(format!("min_spacing={min_spacing} < {min_sp}"));
    }
    if caprock_bad > 0 {
        violations.push(format!("caprock_bad={caprock_bad} > 0"));
    }
    if wf_base_bad > 0 {
        violations.push(format!("wf_base_bad={wf_base_bad} > 0"));
    }
    if !(0.008..=0.045).contains(&m_a.stats.lake_surface_fraction) {
        violations.push(format!(
            "lake_surface_fraction={} outside [0.008, 0.045]",
            m_a.stats.lake_surface_fraction
        ));
    }
    if large_lakes < 5 {
        violations.push(format!("large_lakes={large_lakes} < 5"));
    }
    if paramo_lakes < 1 {
        violations.push(format!("paramo_lakes={paramo_lakes} < 1"));
    }
    if caldera_or_trough < 1 {
        violations.push(format!("caldera_or_trough={caldera_or_trough} < 1"));
    }
    if small_paramo_ponds < 10 {
        violations.push(format!("small_paramo_ponds={small_paramo_ponds} < 10"));
    }
    if m_a.stats.severed_humid_spillways != 0 {
        violations.push(format!(
            "severed_humid_spillways={} != 0",
            m_a.stats.severed_humid_spillways
        ));
    }
    if m_a.stats.unmodeled_micro_pits != 0 {
        violations.push(format!(
            "unmodeled_micro_pits={} != 0",
            m_a.stats.unmodeled_micro_pits
        ));
    }
    if m_a.stats.endorheic_humid_fraction != 0.0 {
        violations.push(format!(
            "endorheic_humid_fraction={} != 0.0",
            m_a.stats.endorheic_humid_fraction
        ));
    }
    if m_a.stats.endorheic_land_fraction > 0.10 {
        violations.push(format!(
            "endorheic_land_fraction={} > 0.10",
            m_a.stats.endorheic_land_fraction
        ));
    }
    if !(0.0005..=0.0030).contains(&m_a.stats.river_surface_fraction) {
        violations.push(format!(
            "river_surface_fraction={} outside [0.0005, 0.0030]",
            m_a.stats.river_surface_fraction
        ));
    }
    if !(30..=48).contains(&net.entrances.len()) {
        violations.push(format!(
            "cave_entrance_count={} outside [30, 48]",
            net.entrances.len()
        ));
    }
    for &b in &BiomeId::ZONED_TERRESTRIAL_16 {
        let cnt = m_a.stats.biome_counts[b as usize];
        if cnt < 2_500 {
            violations.push(format!("biome {b:?} count={cnt} < 2500"));
        }
    }
    if m_a.stats.biome_counts[BiomeId::Chaparral as usize] != 0 {
        violations.push(format!(
            "Chaparral count={} != 0",
            m_a.stats.biome_counts[BiomeId::Chaparral as usize]
        ));
    }
    if m_a.stats.biome_counts[BiomeId::Steppe as usize] != 0 {
        violations.push(format!(
            "Steppe count={} != 0",
            m_a.stats.biome_counts[BiomeId::Steppe as usize]
        ));
    }
    if weight_sum_bad != 0 {
        violations.push(format!("weight_sum_bad={weight_sum_bad} != 0"));
    }
    if forbidden_top4 != 0 {
        violations.push(format!("forbidden_top4={forbidden_top4} != 0"));
    }
    if !(0.15..=0.55).contains(&m_a.stats.ecotone_fraction) {
        violations.push(format!(
            "ecotone_fraction={} outside [0.15, 0.55]",
            m_a.stats.ecotone_fraction
        ));
    }
    if m_a.stats.non_cliff_jump_fraction >= 0.03 {
        violations.push(format!(
            "non_cliff_jump_fraction={} >= 0.03",
            m_a.stats.non_cliff_jump_fraction
        ));
    }
    if sub_ela_violations != 0 {
        violations.push(format!("sub_ela_violations={sub_ela_violations} != 0"));
    }
    if dune_cells == 0 {
        violations.push("dune_cells=0".to_owned());
    }

    assert!(
        violations.is_empty(),
        "4096^2 Card 3 invariant failure(s):\n  - {}",
        violations.join("\n  - ")
    );
    println!(
        "PASS full_4096 (world_sha256={}, waterfalls={}, caves={}) in {:.2}s",
        m_a.world_sha256,
        m_a.stats.waterfall_count,
        net.entrances.len(),
        t0.elapsed().as_secs_f64()
    );
}

fn main() {
    let include_4096 = std::env::args().any(|a| a == "--include-4096");
    let only_4096 = std::env::args().any(|a| a == "--only-4096");
    let only_1024 = std::env::args().any(|a| a == "--only-1024");
    let only_512 = std::env::args().any(|a| a == "--only-512");
    let only_rayon = std::env::args().any(|a| a == "--only-rayon");
    if only_4096 {
        verify_4096_full_pipeline();
        return;
    }
    if only_1024 {
        verify_card3_1024_sweep();
        return;
    }
    if only_512 {
        verify_biome_512_sweep();
        return;
    }
    if only_rayon {
        verify_1024_rayon_invariance();
        return;
    }
    verify_card3_1024_sweep();
    verify_biome_512_sweep();
    verify_1024_rayon_invariance();
    if include_4096 {
        verify_4096_full_pipeline();
    }
}
