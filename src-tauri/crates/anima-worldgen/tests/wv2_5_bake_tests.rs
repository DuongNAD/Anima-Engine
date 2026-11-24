#![cfg(feature = "bake")]
#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

use anima_worldgen::anmw_v3::project_raw_dir_to_sim_256;
use anima_worldgen::bake::assets::{
    bake_assets_from_manifest, bake_octahedral_impostor_cpu, parse_glb_mesh, parse_obj_mesh,
    simplify_mesh_lod, validate_asset_sources_manifest, AssetMesh, AssetSourceEntry,
    AssetSourcesManifest,
};
use anima_worldgen::bake::fields::{
    bake_fields_from_raw_dir, compute_shore_sdf_and_exposure, meijster_edt_sq, merge_field_strips,
};
use anima_worldgen::bake::flora_place::{FloraInstance, FloraSectorBlob, FLORA_FLAG_DECORATIVE};
use anima_worldgen::bake::lighting_cpu::{
    bake_lighting_cpu_from_raw_dir, bake_lighting_cpu_region, merge_lighting_tiles,
    solve_1d_horizon_bounded_stewart,
};
use anima_worldgen::bake::pack_ktx2::{
    compute_block_psnr_db, compute_ssim_u8, decode_bc5_unorm, decode_ktx2_level, encode_bc5_unorm,
    pack_entries_to_archive, pack_world_v2_tiles, parse_ktx2, verify_tile_pack,
    VK_FORMAT_BC5_UNORM_BLOCK, VK_FORMAT_R16_UNORM,
};
use anima_worldgen::bake::{
    compute_mip_pyramid, extract_tile_padded, verify_horizontal_gutter_overlap,
    verify_vertical_gutter_overlap, TileRange, TileStrip, TILE_ACTIVE_SIZE, TILE_GUTTER,
    TILE_TOTAL_SIZE,
};
use anima_worldgen::caves::{CaveEdge, CaveEntrance, CaveNetwork, CaveNode};
use anima_worldgen::config::WorldV2Config;
use anima_worldgen::pipeline::run_worldgen_to_dir;
use anima_worldgen::verify::WorldV2IndexJson;
use anima_worldgen::WaterKind;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::f32::consts::TAU;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::{Archive, Builder, Header};

fn temp_test_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "anima_wv2_5_{tag}_{}_{}",
        std::process::id(),
        line!()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0F) as usize] as char);
    }
    s
}

#[test]
fn meijster_edt_matches_brute_force_exact_on_128x128() {
    let w = 128_usize;
    let h = 128_usize;
    let mut mask = vec![false; w * h];
    let mut water_kind = vec![WaterKind::None as u8; w * h];
    let mut total_h_m = vec![40.0_f32; w * h];

    // Multi-island + concave bay + off-center channels & peninsulas (`LD-1` coastal test).
    for r in 0..h {
        for c in 0..w {
            let idx = r * w + c;
            let dr = r as f32 - 64.0;
            let dc = c as f32 - 64.0;
            let angle = libm::atan2f(dr, dc);
            let rad = libm::hypotf(dr, dc);
            let coast_r = 36.0 + 12.0 * libm::sinf(angle * 3.0) + 6.0 * libm::cosf(angle * 5.0);
            let island = (r.abs_diff(24) <= 5 && c.abs_diff(24) <= 7)
                || (r.abs_diff(98) <= 6 && c.abs_diff(92) <= 4);
            let is_water = (rad > coast_r && !island) || (r < 6 && c > 110);
            mask[idx] = is_water;
            if is_water {
                water_kind[idx] = WaterKind::Ocean as u8;
                total_h_m[idx] = -25.0;
            } else {
                total_h_m[idx] = 25.0 + 2.0 * (coast_r - rad).max(0.0);
            }
        }
    }

    let got = meijster_edt_sq(&mask, w, h);

    // Exact brute-force reference over all feature points.
    let features: Vec<(isize, isize)> = mask
        .iter()
        .enumerate()
        .filter(|&(_, &v)| v)
        .map(|(idx, _)| ((idx / w) as isize, (idx % w) as isize))
        .collect();
    for r in 0..h {
        for c in 0..w {
            let mut best = u32::MAX;
            for &(fr, fc) in &features {
                let dr = r as isize - fr;
                let dc = c as isize - fc;
                let d2 = (dr * dr + dc * dc) as u32;
                if d2 < best {
                    best = d2;
                }
            }
            assert_eq!(
                got[r * w + c],
                best,
                "Meijster EDT differs from brute-force at ({r}, {c})"
            );
        }
    }

    // Check degenerate all-false / all-true cases cleanly.
    assert!(meijster_edt_sq(&[false; 16], 4, 4)
        .iter()
        .all(|&v| v == u32::MAX));
    assert!(meijster_edt_sq(&[true; 16], 4, 4).iter().all(|&v| v == 0));

    // Check shore_sdf clamping and sign invariants.
    let (sdf, exposure, wcnt) =
        compute_shore_sdf_and_exposure(&total_h_m, &water_kind, w, h, 40.0, 0.0, 0.5);
    assert_eq!(wcnt, features.len());
    for i in 0..(w * h) {
        assert!(sdf[i] >= -500.0 && sdf[i] <= 500.0);
        if mask[i] {
            assert!(sdf[i] <= 0.0);
        } else {
            assert!(sdf[i] > 0.0);
        }
        assert!(exposure[i] >= 0.0 && exposure[i] <= 1.0);
    }
}

#[test]
fn stewart_bounded_horizon_and_strip_sharding_are_byte_identical_to_global_pass() {
    // 1. Exact 1-D verification of blocked Stewart convex hull vs brute force over 1..=256.
    let len = 640_usize;
    let radius = 256_usize;
    let dx = 10.0_f32;
    let line: Vec<f32> = (0..len)
        .map(|i| {
            let x = i as f32;
            300.0 + 180.0 * libm::sinf(x * 0.037) + 95.0 * libm::cosf(x * 0.113) + 0.15 * x
        })
        .collect();
    let mut got_tan = vec![0.0_f32; len];
    solve_1d_horizon_bounded_stewart(&line, 0, len, radius, dx, &mut got_tan);

    for i in 0..len {
        let mut ref_tan = 0.0_f32;
        for d in 1..=radius {
            if i + d >= len {
                break;
            }
            let s = (line[i + d] - line[i]) / (d as f32 * dx);
            if s > ref_tan {
                ref_tan = s;
            }
        }
        let diff = (got_tan[i] - ref_tan).abs();
        assert!(
            diff <= 1e-5,
            "Stewart bounded horizon mismatch at i={i}: got={} ref={} diff={diff}",
            got_tan[i],
            ref_tan
        );
    }

    // 2. Fields strip sharding byte-identity (`--tile-strip 0/4 .. 3/4` == global pass) on 256^2 world.
    let root = temp_test_dir("strip_parity");
    let raw_dir = root.join("raw");
    let full_dir = root.join("fields_full");
    let strip_dir = root.join("fields_strips");
    let mut cfg = WorldV2Config::for_size(0x5756_3201, 256);
    cfg.target_ocean_fraction = 0.45;
    run_worldgen_to_dir(&cfg, &raw_dir).expect("generate 256x256 raw");

    let full_rep = bake_fields_from_raw_dir(&raw_dir, &full_dir, None).expect("global fields");
    for s in 0..4 {
        bake_fields_from_raw_dir(&raw_dir, &strip_dir, Some(TileStrip { index: s, count: 4 }))
            .expect("strip field");
    }
    merge_field_strips(&strip_dir, 4).expect("merge strips");

    assert_eq!(
        fs::read(full_dir.join("normal_xy.bin")).unwrap(),
        fs::read(strip_dir.join("normal_xy.bin")).unwrap(),
        "sharded normal_xy strips must be bit-identical to global pass"
    );
    assert_eq!(
        fs::read(full_dir.join("hydro_flow.bin")).unwrap(),
        fs::read(strip_dir.join("hydro_flow.bin")).unwrap(),
        "sharded hydro_flow strips must be bit-identical to global pass"
    );
    assert_eq!(
        fs::read(full_dir.join("shore_sdf.bin")).unwrap(),
        fs::read(strip_dir.join("shore_sdf.bin")).unwrap(),
    );
    assert!(
        full_rep
            .flora
            .as_ref()
            .expect("flora summary")
            .decorative_only
    );

    // 3. Multi-tile (`512x512` = 4 tiles of `256x256`) Stewart horizon tile-range byte-identity.
    let w512 = 512_usize;
    let mut h512 = vec![0.0_f32; w512 * w512];
    let luma512 = vec![0.35_f32; w512 * w512];
    for r in 0..w512 {
        for c in 0..w512 {
            let rx = c as f32 * 0.025;
            let rz = r as f32 * 0.031;
            h512[r * w512 + c] =
                500.0 + 420.0 * libm::sinf(rx) * libm::cosf(rz) + 60.0 * libm::sinf(rx * 4.0);
        }
    }
    let full_512 = bake_lighting_cpu_region(&h512, &luma512, None, w512, w512, 10.0, 256, None)
        .expect("full 512 lighting");
    for ty in 0..2 {
        for tx in 0..2 {
            let tile_out = bake_lighting_cpu_region(
                &h512,
                &luma512,
                None,
                w512,
                w512,
                10.0,
                256,
                Some((
                    tx * TILE_ACTIVE_SIZE,
                    ty * TILE_ACTIVE_SIZE,
                    TILE_ACTIVE_SIZE,
                    TILE_ACTIVE_SIZE,
                )),
            )
            .expect("tile lighting");
            for lr in 0..TILE_ACTIVE_SIZE {
                let gr = ty * TILE_ACTIVE_SIZE + lr;
                let g_off = gr * w512 + tx * TILE_ACTIVE_SIZE;
                let l_off = lr * TILE_ACTIVE_SIZE;
                assert_eq!(
                    &tile_out.terrain_lighting[l_off..(l_off + TILE_ACTIVE_SIZE)],
                    &full_512.terrain_lighting[g_off..(g_off + TILE_ACTIVE_SIZE)],
                    "tile ({tx},{ty}) terrain_lighting must equal global pass bit-for-bit"
                );
                assert_eq!(
                    &tile_out.terrain_horizon[l_off..(l_off + TILE_ACTIVE_SIZE)],
                    &full_512.terrain_horizon[g_off..(g_off + TILE_ACTIVE_SIZE)],
                    "tile ({tx},{ty}) terrain_horizon must equal global pass bit-for-bit"
                );
            }
        }
    }

    // Also test CLI `--tile-range` + `--merge-size` round-trip via library helpers.
    let light_full_dir = root.join("light_full");
    let light_tiles_dir = root.join("light_tiles");
    bake_lighting_cpu_from_raw_dir(&raw_dir, &light_full_dir, None).expect("full 256 lighting");
    bake_lighting_cpu_from_raw_dir(
        &raw_dir,
        &light_tiles_dir,
        Some(TileRange { start: 0, end: 1 }),
    )
    .expect("tile 0 lighting");
    merge_lighting_tiles(&light_tiles_dir, 256, 256).expect("merge 256 tiles");
    assert_eq!(
        fs::read(light_full_dir.join("terrain_lighting.bin")).unwrap(),
        fs::read(light_tiles_dir.join("terrain_lighting.bin")).unwrap(),
    );

    let _ = fs::remove_dir_all(&root);
}

fn write_synthetic_512_world(dir: &Path) -> PathBuf {
    let raw_dir = dir.join("raw");
    let fields_dir = dir.join("fields");
    fs::create_dir_all(&raw_dir).unwrap();

    // Generate a 256 world first to reuse realistic `sim_256.anmw3` & `raw_meta.json`, then upsample into 512x512 raw!
    let seed_dir = dir.join("seed256");
    let mut cfg = WorldV2Config::for_size(0x5756_3201, 256);
    cfg.target_ocean_fraction = 0.45;
    run_worldgen_to_dir(&cfg, &seed_dir).expect("seed 256");
    let sim = project_raw_dir_to_sim_256(&seed_dir, 3).expect("sim_256");
    let sim_path = dir.join("sim_256.anmw3");
    fs::write(&sim_path, sim.to_bytes().unwrap()).unwrap();

    let w = 512_usize;
    let n = w * w;
    let mut meta: anima_worldgen::pipeline::RawWorldMeta =
        serde_json::from_str(&fs::read_to_string(seed_dir.join("raw_meta.json")).unwrap()).unwrap();
    meta.width = w as u32;
    meta.height = w as u32;
    meta.dx_m = meta.extent_m / w as f32;

    // Upsample layers 2x nearest/bilinear smoothly so 512x512 gives a 2x2 L0 + 1x1 L1 tile pyramid.
    let up_f32 = |name: &str| {
        let src = anima_worldgen::pipeline::read_bin_f32(&seed_dir.join(name), 256 * 256).unwrap();
        let mut out = vec![0.0_f32; n];
        for r in 0..w {
            for c in 0..w {
                out[r * w + c] = src[(r / 2) * 256 + (c / 2)];
            }
        }
        let mut bytes = Vec::with_capacity(n * 4);
        for v in out {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        fs::write(raw_dir.join(name), bytes).unwrap();
    };
    let up_u8 = |name: &str| {
        let src = anima_worldgen::pipeline::read_bin_u8(&seed_dir.join(name), 256 * 256).unwrap();
        let mut out = vec![0_u8; n];
        for r in 0..w {
            for c in 0..w {
                out[r * w + c] = src[(r / 2) * 256 + (c / 2)];
            }
        }
        fs::write(raw_dir.join(name), out).unwrap();
    };
    let up_u8x4 = |name: &str| {
        let src = anima_worldgen::pipeline::read_bin_u8x4(&seed_dir.join(name), 256 * 256).unwrap();
        let mut out = Vec::with_capacity(n * 4);
        for r in 0..w {
            for c in 0..w {
                out.extend_from_slice(&src[(r / 2) * 256 + (c / 2)]);
            }
        }
        fs::write(raw_dir.join(name), out).unwrap();
    };

    for f in [
        "total_h_m.bin",
        "water_h_m.bin",
        "sediment_s_m.bin",
        "talus_frac.bin",
        "glacier_h_m.bin",
        "temp_c.bin",
        "moisture.bin",
        "precip_m_yr.bin",
        "flow_dir_x.bin",
        "flow_dir_z.bin",
        "discharge_q_m3_s.bin",
        "river_w_m.bin",
        "twi.bin",
    ] {
        up_f32(f);
    }
    for f in [
        "water_kind.bin",
        "dominant_biome.bin",
        "lithology_id.bin",
        "moraine_debris.bin",
        "cave_mask.bin",
    ] {
        up_u8(f);
    }
    up_u8x4("biome_ids.bin");
    up_u8x4("biome_weights.bin");
    fs::copy(
        seed_dir.join("cave_network.bin"),
        raw_dir.join("cave_network.bin"),
    )
    .unwrap();
    fs::write(
        raw_dir.join("raw_meta.json"),
        serde_json::to_string_pretty(&meta).unwrap(),
    )
    .unwrap();

    bake_fields_from_raw_dir(&raw_dir, &fields_dir, None).unwrap();
    bake_lighting_cpu_from_raw_dir(&raw_dir, &fields_dir, None).unwrap();
    sim_path
}

#[test]
fn ktx2_bc5_psnr_ssim_mip_completeness_and_cave_flora_roundtrip() {
    let root = temp_test_dir("pack_roundtrip");
    let sim_path = write_synthetic_512_world(&root);
    let sim_sha_before = sha256_hex(&fs::read(&sim_path).unwrap());

    // 1. Standalone BC5 Block PSNR & SSIM check on rich smooth+ridged tangent gradient (`260x260`).
    let mut grad = vec![[0_u8; 2]; TILE_TOTAL_SIZE * TILE_TOTAL_SIZE];
    for r in 0..TILE_TOTAL_SIZE {
        for c in 0..TILE_TOTAL_SIZE {
            let nx = (0.5 + 0.45 * libm::sinf(c as f32 * 0.08 + r as f32 * 0.03)).clamp(0.0, 1.0);
            let nz = (0.5 + 0.45 * libm::cosf(r as f32 * 0.07 - c as f32 * 0.02)).clamp(0.0, 1.0);
            grad[r * TILE_TOTAL_SIZE + c] =
                [(nx * 255.0).round() as u8, (nz * 255.0).round() as u8];
        }
    }
    let bc5 = encode_bc5_unorm(&grad, TILE_TOTAL_SIZE, TILE_TOTAL_SIZE).expect("encode bc5");
    let recon = decode_bc5_unorm(&bc5, TILE_TOTAL_SIZE, TILE_TOTAL_SIZE).expect("decode bc5");
    let orig_flat: Vec<u8> = grad.iter().flat_map(|p| p.iter().copied()).collect();
    let recon_flat: Vec<u8> = recon.iter().flat_map(|p| p.iter().copied()).collect();
    let psnr = compute_block_psnr_db(&orig_flat, &recon_flat, TILE_TOTAL_SIZE, TILE_TOTAL_SIZE, 2);
    let ssim = compute_ssim_u8(&orig_flat, &recon_flat, TILE_TOTAL_SIZE, TILE_TOTAL_SIZE, 2);
    assert!(psnr >= 42.0, "BC5 Block PSNR {psnr:.2} dB < 42 dB");
    assert!(ssim >= 0.996, "BC5 SSIM {ssim:.5} < 0.996");

    // 2. CaveNetwork LE binary round-trip + FloraSectorBlob 8-byte decorative round-trip.
    let net = CaveNetwork {
        flags: 0,
        nodes: vec![
            CaveNode {
                x_m: 1200.0,
                y_m: 340.0,
                z_m: 2400.0,
                radius_m: 3.5,
                light_decay: 0.25,
            },
            CaveNode {
                x_m: 1260.0,
                y_m: 332.0,
                z_m: 2430.0,
                radius_m: 2.8,
                light_decay: 0.40,
            },
        ],
        edges: vec![CaveEdge {
            from_idx: 0,
            to_idx: 1,
            passage_kind: 1,
        }],
        entrances: vec![CaveEntrance {
            node_idx: 0,
            mouth_kind: 0,
            realm_status: 1,
            yaw_rad: 0.9,
            elevation_m: 340.0,
            clearance_m: 3.2,
        }],
    };
    assert_eq!(CaveNetwork::from_bytes(&net.to_bytes()).unwrap(), net);

    let flora = FloraSectorBlob {
        sector_x: 1,
        sector_z: 2,
        decorative_only: true,
        instances: vec![FloraInstance {
            x_local_u16: 12345,
            z_local_u16: 54321,
            species_id: 2,
            scale_u8: 180,
            yaw_u8: 77,
            flags_u8: FLORA_FLAG_DECORATIVE,
        }],
    };
    assert_eq!(
        FloraSectorBlob::from_bytes(&flora.to_bytes().unwrap()).unwrap(),
        flora
    );

    // 3. Full 512x512 pyramid pack (`L0 = 2x2`, `L1 = 1x1`) + strict archive verify.
    let out_dir = root.join("packed");
    let summary = pack_world_v2_tiles(
        &root.join("fields"),
        &root.join("raw"),
        &sim_path,
        "world-v2-test",
        &out_dir,
    )
    .expect("pack 512x512 world");
    assert_eq!(summary.mip_levels, 2);
    assert_eq!(summary.sim_world_identity, sim_sha_before);
    assert!(summary.normal_xy_bc5_block_psnr_db >= 42.0);
    assert!(summary.normal_xy_bc5_ssim >= 0.996);

    let arch_path = out_dir.join(&summary.archive_filename);
    let idx_path = out_dir.join("index.json");
    let vrep = verify_tile_pack(&idx_path, &out_dir.join("sim_256.anmw3"), &arch_path)
        .expect("verify packed archive");
    assert!(vrep.passed);
    assert_eq!(vrep.sim_world_identity, sim_sha_before);
    assert!(vrep.ktx2_headers_verified >= 40);

    // Inspect KTX2 header + KVD + mip completeness inside archive: height KVD & 512^2 sky probe (10 mips!).
    let arch_raw = fs::read(&arch_path).unwrap();
    let gz = GzDecoder::new(arch_raw.as_slice());
    let mut tar = Archive::new(gz);
    let mut saw_height = false;
    let mut saw_normal = false;
    let mut saw_probe = false;
    for entry_res in tar.entries().unwrap() {
        let mut entry = entry_res.unwrap();
        let path = entry.path().unwrap().to_string_lossy().into_owned();
        if path == "tiles/height/L0/t_0_0.ktx2" {
            let mut b = Vec::new();
            entry.read_to_end(&mut b).unwrap();
            let (parsed, raw_l0) = decode_ktx2_level(&b, 0).unwrap();
            assert_eq!(parsed.vk_format, VK_FORMAT_R16_UNORM);
            assert_eq!(parsed.width, TILE_TOTAL_SIZE as u32);
            assert_eq!(raw_l0.len(), TILE_TOTAL_SIZE * TILE_TOTAL_SIZE * 2);
            assert_eq!(
                parsed.kvd.get("animaBorderF32").map(Vec::len),
                Some(TILE_TOTAL_SIZE * 4 * 4)
            );
            saw_height = true;
        } else if path == "tiles/normal_xy/L0/t_0_0.ktx2" {
            let mut b = Vec::new();
            entry.read_to_end(&mut b).unwrap();
            let parsed = parse_ktx2(&b).unwrap();
            assert_eq!(parsed.vk_format, VK_FORMAT_BC5_UNORM_BLOCK);
            saw_normal = true;
        } else if path == "tiles/sky_bounce_probe.ktx2" {
            let mut b = Vec::new();
            entry.read_to_end(&mut b).unwrap();
            let parsed = parse_ktx2(&b).unwrap();
            assert_eq!(parsed.width, 512);
            assert_eq!(parsed.level_count, 10); // 512,256,128,64,32,16,8,4,2,1
            saw_probe = true;
        }
    }
    assert!(saw_height && saw_normal && saw_probe);
    assert_eq!(compute_mip_pyramid(4096).unwrap().len(), 5);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn negative_control_a_removing_tile_gutter_fails_boundary_equality() {
    let w = 512_usize;
    let mut grid = vec![0.0_f32; w * w];
    for r in 0..w {
        for c in 0..w {
            grid[r * w + c] = 100.0 + (c as f32) * 1.7 + (r as f32) * 0.9;
        }
    }

    // With 2-texel gutter (`260x260`), adjacent tiles match identically across the 4-texel overlap band!
    let t00 = extract_tile_padded(&grid, w, w, 0, 0, TILE_ACTIVE_SIZE, TILE_GUTTER);
    let t10 = extract_tile_padded(&grid, w, w, 1, 0, TILE_ACTIVE_SIZE, TILE_GUTTER);
    let t01 = extract_tile_padded(&grid, w, w, 0, 1, TILE_ACTIVE_SIZE, TILE_GUTTER);
    verify_horizontal_gutter_overlap(&t00, &t10, TILE_ACTIVE_SIZE, TILE_GUTTER)
        .expect("2-texel horizontal gutter overlaps identically");
    verify_vertical_gutter_overlap(&t00, &t01, TILE_ACTIVE_SIZE, TILE_GUTTER)
        .expect("2-texel vertical gutter overlaps identically");

    // Removing the gutter (`gutter = 0`, `256x256`) fails both structural overlap AND border equality!
    let g0_00 = extract_tile_padded(&grid, w, w, 0, 0, TILE_ACTIVE_SIZE, 0);
    let g0_10 = extract_tile_padded(&grid, w, w, 1, 0, TILE_ACTIVE_SIZE, 0);
    assert!(verify_horizontal_gutter_overlap(&g0_00, &g0_10, TILE_ACTIVE_SIZE, 0).is_err());
    // Right edge column (col 255) != next tile's left edge column (col 256):
    assert_ne!(g0_00[255], g0_10[0]);
}

#[test]
fn negative_control_b_tampering_one_tile_byte_fails_archive_verifier() {
    let root = temp_test_dir("neg_b_tamper");
    let sim_path = write_synthetic_512_world(&root);
    let out_dir = root.join("packed");
    let summary = pack_world_v2_tiles(
        &root.join("fields"),
        &root.join("raw"),
        &sim_path,
        "world-v2-tamper-test",
        &out_dir,
    )
    .unwrap();
    let idx_path = out_dir.join("index.json");
    let sim_out = out_dir.join("sim_256.anmw3");
    let arch_path = out_dir.join(&summary.archive_filename);

    // Control 1: flip 1 byte in archive file directly -> fails archive SHA-256.
    let mut arch_bytes = fs::read(&arch_path).unwrap();
    let mid = arch_bytes.len() / 2;
    arch_bytes[mid] ^= 0x01;
    let bad_arch_1 = out_dir.join("bad_raw.tar.gz");
    fs::write(&bad_arch_1, &arch_bytes).unwrap();
    assert!(verify_tile_pack(&idx_path, &sim_out, &bad_arch_1).is_err());

    // Control 2: decompress, tamper 1 byte inside `tiles/normal_xy/L0/t_0_0.ktx2`, re-pack, AND update
    // ONLY `release_archive_sha256` -> proves per-tile SHA-256 check ALSO catches tile tampering!
    let orig_arch = fs::read(&arch_path).unwrap();
    let mut tar_in = Archive::new(GzDecoder::new(orig_arch.as_slice()));
    let mut entries = BTreeMap::new();
    for ent in tar_in.entries().unwrap() {
        let mut ent = ent.unwrap();
        let p = ent.path().unwrap().to_string_lossy().into_owned();
        let mut buf = Vec::new();
        ent.read_to_end(&mut buf).unwrap();
        if p == "tiles/normal_xy/L0/t_0_0.ktx2" {
            let last = buf.len() - 1;
            buf[last] ^= 0xFF;
        }
        entries.insert(p, buf);
    }
    let bad_arch_2 = out_dir.join("bad_tile_rehashed_arch.tar.gz");
    let (_, bad_arch_sha) = pack_entries_to_archive(&entries, &bad_arch_2).unwrap();
    let mut idx: WorldV2IndexJson = serde_json::from_slice(&fs::read(&idx_path).unwrap()).unwrap();
    idx.release_archive_sha256 = Some(bad_arch_sha);
    let idx_2 = out_dir.join("index_tampered_arch.json");
    fs::write(&idx_2, idx.to_canonical_json_bytes().unwrap()).unwrap();
    let err =
        verify_tile_pack(&idx_2, &sim_out, &bad_arch_2).expect_err("1-byte tile tamper must fail");
    assert!(
        err.message.contains("SHA-256 mismatch"),
        "expected per-tile SHA-256 mismatch, got: {}",
        err.message
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn negative_control_c_including_index_json_in_archive_fails_self_check() {
    let root = temp_test_dir("neg_c_index_cycle");
    let sim_path = write_synthetic_512_world(&root);
    let out_dir = root.join("packed");
    let summary = pack_world_v2_tiles(
        &root.join("fields"),
        &root.join("raw"),
        &sim_path,
        "world-v2-cycle-test",
        &out_dir,
    )
    .unwrap();
    let idx_path = out_dir.join("index.json");
    let sim_out = out_dir.join("sim_256.anmw3");

    // 1. Packager self-check refuses to put `index.json` or `tiles/index.json` inside an archive.
    let mut entries = BTreeMap::new();
    entries.insert("index.json".to_owned(), fs::read(&idx_path).unwrap());
    let err_pack = pack_entries_to_archive(&entries, &root.join("forbidden.tar.gz"))
        .expect_err("packager must forbid index.json inside archive");
    assert!(err_pack.message.contains("index.json"));

    let mut nested = BTreeMap::new();
    nested.insert("sub/INDEX.JSON".to_owned(), b"{}".to_vec());
    assert!(pack_entries_to_archive(&nested, &root.join("forbidden2.tar.gz")).is_err());

    // 2. Verifier ALSO rejects a hand-crafted `.tar.gz` containing `index.json` even if hashes match.
    let raw_arch_with_index = root.join("crafted_cycle.tar.gz");
    {
        let f = fs::File::create(&raw_arch_with_index).unwrap();
        let gz = GzEncoder::new(f, Compression::fast());
        let mut tar = Builder::new(gz);
        let idx_bytes = fs::read(&idx_path).unwrap();
        let mut hdr = Header::new_ustar();
        hdr.set_size(idx_bytes.len() as u64);
        hdr.set_mode(0o644);
        hdr.set_cksum();
        tar.append_data(&mut hdr, "index.json", idx_bytes.as_slice())
            .unwrap();
        tar.into_inner().unwrap().finish().unwrap();
    }
    let crafted_sha = sha256_hex(&fs::read(&raw_arch_with_index).unwrap());
    let mut idx: WorldV2IndexJson = serde_json::from_slice(&fs::read(&idx_path).unwrap()).unwrap();
    idx.release_archive_sha256 = Some(crafted_sha);
    let idx_cycle = root.join("index_cycle.json");
    fs::write(&idx_cycle, idx.to_canonical_json_bytes().unwrap()).unwrap();
    let err_ver = verify_tile_pack(&idx_cycle, &sim_out, &raw_arch_with_index)
        .expect_err("verifier must reject archive containing index.json");
    assert!(
        err_ver.message.contains("MUST NOT contain 'index.json'"),
        "unexpected error: {}",
        err_ver.message
    );
    let _ = summary;
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn assets_lod_impostor_pipeline_and_qd_assets_manifest_validation() {
    let root = temp_test_dir("assets_qd");
    let src_dir = root.join("assets_src");
    let out_dir = root.join("assets_out");
    fs::create_dir_all(&src_dir).unwrap();

    // Build deterministic icosphere-style UV grid .obj (`~400` triangles)
    let mut obj = String::new();
    let stacks = 10_usize;
    let slices = 20_usize;
    for s in 0..=stacks {
        let phi = std::f32::consts::PI * (s as f32 / stacks as f32);
        let y = 2.0 + 1.5 * libm::cosf(phi);
        let rad = 1.5 * libm::sinf(phi);
        for k in 0..=slices {
            let theta = TAU * (k as f32 / slices as f32);
            let x = rad * libm::cosf(theta);
            let z = rad * libm::sinf(theta);
            obj.push_str(&format!("v {x:.5} {y:.5} {z:.5}\n"));
            let nx = libm::sinf(phi) * libm::cosf(theta);
            let ny = libm::cosf(phi);
            let nz = libm::sinf(phi) * libm::sinf(theta);
            obj.push_str(&format!("vn {nx:.5} {ny:.5} {nz:.5}\n"));
        }
    }
    let row = slices + 1;
    for s in 0..stacks {
        for k in 0..slices {
            let a = s * row + k + 1;
            let b = (s + 1) * row + k + 1;
            let c = b + 1;
            let d = a + 1;
            obj.push_str(&format!("f {a}//{a} {b}//{b} {c}//{c}\n"));
            obj.push_str(&format!("f {a}//{a} {c}//{c} {d}//{d}\n"));
        }
    }
    let parsed_mesh = parse_obj_mesh(&obj).expect("parse synthetic sphere obj");
    assert_eq!(parsed_mesh.indices.len(), stacks * slices * 2);

    let obj_path = src_dir.join("canopy.obj");
    fs::write(&obj_path, obj.as_bytes()).unwrap();
    let obj_sha = sha256_hex(obj.as_bytes());
    let obj_len = obj.len() as u64;

    let valid_manifest = AssetSourcesManifest {
        format_version: 1,
        entries: vec![AssetSourceEntry {
            species_id: "cecropia".to_owned(),
            source: "Poly Haven".to_owned(),
            licence: "CC0-1.0".to_owned(),
            licence_url: "https://polyhaven.com/license".to_owned(),
            sha256: obj_sha.clone(),
            download_bytes: obj_len,
            committed: true,
            obj_path: "canopy.obj".to_owned(),
            base_rgb: Some([48, 118, 46]),
        }],
    };
    let manifest_path = src_dir.join("sources.json");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&valid_manifest).unwrap(),
    )
    .unwrap();

    let rep = bake_assets_from_manifest(&manifest_path, &out_dir, 128).expect("bake assets");
    assert_eq!(rep.species_count, 1);
    let sp = &rep.outputs[0];
    assert!(sp.lod_triangle_counts[0] >= sp.lod_triangle_counts[1]);
    assert!(sp.lod_triangle_counts[1] >= sp.lod_triangle_counts[2]);
    assert!(sp.lod_triangle_counts[2] >= sp.lod_triangle_counts[3]);
    assert!(sp.lod_triangle_counts[3] >= 4);

    for lod in 0..4 {
        let b = fs::read(out_dir.join(format!("species_cecropia.meshopt.lod{lod}.bin"))).unwrap();
        let decoded = AssetMesh::from_anmm_bytes(&b).expect("decode ANMM");
        assert_eq!(decoded.indices.len(), sp.lod_triangle_counts[lod]);
    }
    let alb_bytes = fs::read(out_dir.join("species_cecropia_impostor_albedo.ktx2")).unwrap();
    let (alb_hdr, alb_raw) = decode_ktx2_level(&alb_bytes, 0).unwrap();
    assert_eq!(alb_hdr.level_count, 8); // 128 -> 64 -> ... -> 1
    assert!(alb_raw.as_chunks::<4>().0.iter().any(|px| px[3] == 255));
    let nrm_hdr =
        parse_ktx2(&fs::read(out_dir.join("species_cecropia_impostor_normal.ktx2")).unwrap())
            .unwrap();
    assert_eq!(nrm_hdr.vk_format, VK_FORMAT_BC5_UNORM_BLOCK);
    assert_eq!(nrm_hdr.level_count, 6); // 128 -> 64 -> 32 -> 16 -> 8 -> 4

    // QD-ASSETS negative controls:
    let mut bad_source = valid_manifest.clone();
    bad_source.entries[0].source = "Sketchfab".to_owned();
    assert!(validate_asset_sources_manifest(&bad_source, &src_dir).is_err());

    let mut bad_lic = valid_manifest.clone();
    bad_lic.entries[0].licence = "CC-BY-4.0".to_owned();
    assert!(validate_asset_sources_manifest(&bad_lic, &src_dir).is_err());

    let mut bad_budget = valid_manifest;
    bad_budget.entries[0].download_bytes = 25_000_000;
    assert!(validate_asset_sources_manifest(&bad_budget, &src_dir).is_err());

    assert!(parse_glb_mesh(b"glTF\x02\x00\x00\x00").is_err());
    assert!(parse_glb_mesh(b"BAD!00000000000000000000").is_err());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn committed_assets_v2_sources_manifest_passes_qd_assets_and_glb_decode() {
    let assets_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../artifacts/assets-v2");
    let manifest_path = assets_dir.join("SOURCES.json");
    let raw_json = fs::read_to_string(&manifest_path)
        .expect("read committed artifacts/assets-v2/SOURCES.json");
    let manifest: AssetSourcesManifest =
        serde_json::from_str(&raw_json).expect("parse artifacts/assets-v2/SOURCES.json");
    validate_asset_sources_manifest(&manifest, &assets_dir)
        .expect("committed SOURCES.json must satisfy QD-ASSETS");
    assert_eq!(manifest.entries.len(), 6);

    let mut decoded_moss = None;
    for entry in &manifest.entries {
        let bytes = fs::read(assets_dir.join(&entry.obj_path))
            .unwrap_or_else(|e| panic!("read {}: {e}", entry.obj_path));
        let mesh =
            parse_glb_mesh(&bytes).unwrap_or_else(|e| panic!("parse GLB {}: {e}", entry.obj_path));
        assert!(
            !mesh.vertices.is_empty() && !mesh.indices.is_empty(),
            "asset {} must have non-empty geometry",
            entry.species_id
        );
        assert!(
            mesh.vertices.iter().all(|v| {
                v.pos.iter().all(|x| x.is_finite()) && v.normal.iter().all(|x| x.is_finite())
            }),
            "asset {} must have finite vertices and normals",
            entry.species_id
        );
        if entry.species_id == "cloud_forest_moss" {
            decoded_moss = Some((mesh, entry.base_rgb.unwrap_or([64, 110, 56])));
        }
    }

    let (moss_mesh, moss_rgb) = decoded_moss.expect("cloud_forest_moss present");
    let mut prev_tris = usize::MAX;
    for ratio in [1.0_f32, 0.50, 0.25, 0.10] {
        let lod = simplify_mesh_lod(&moss_mesh, ratio);
        assert!(lod.indices.len() <= prev_tris);
        assert!(lod.indices.len() >= 4);
        let roundtrip = AssetMesh::from_anmm_bytes(&lod.to_anmm_bytes())
            .expect("ANMM roundtrip for committed Poly Haven moss GLB");
        assert_eq!(roundtrip.indices.len(), lod.indices.len());
        prev_tris = lod.indices.len();
    }
    let (alb_ktx2, nrm_ktx2) =
        bake_octahedral_impostor_cpu(&moss_mesh, 64, moss_rgb).expect("impostor bake");
    let (alb_hdr, alb_raw) = decode_ktx2_level(&alb_ktx2, 0).expect("decode impostor albedo");
    assert_eq!(alb_hdr.level_count, 7);
    assert!(alb_raw.as_chunks::<4>().0.iter().any(|px| px[3] == 255));
    let nrm_hdr = parse_ktx2(&nrm_ktx2).expect("parse impostor normal");
    assert_eq!(nrm_hdr.vk_format, VK_FORMAT_BC5_UNORM_BLOCK);
}

#[test]
fn anima_bake_cli_end_to_end_smoke_on_256_world() {
    let root = temp_test_dir("cli_smoke_256");
    let raw_dir = root.join("raw");
    let baked_dir = root.join("baked");
    let pack_dir = root.join("pack_out");
    let mut cfg = WorldV2Config::for_size(0x5756_3201, 256);
    cfg.target_ocean_fraction = 0.45;
    run_worldgen_to_dir(&cfg, &raw_dir).expect("generate 256 raw");
    let sim = project_raw_dir_to_sim_256(&raw_dir, 3).unwrap();
    let sim_path = root.join("sim_256.anmw3");
    fs::write(&sim_path, sim.to_bytes().unwrap()).unwrap();

    let bin = env!("CARGO_BIN_EXE_anima-bake");
    let out_f = Command::new(bin)
        .args([
            "fields",
            "--raw",
            raw_dir.to_str().unwrap(),
            "--out",
            baked_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run anima-bake fields");
    assert!(
        out_f.status.success(),
        "fields failed: {}",
        String::from_utf8_lossy(&out_f.stderr)
    );

    let out_l = Command::new(bin)
        .args([
            "lighting",
            "--backend",
            "cpu",
            "--raw",
            raw_dir.to_str().unwrap(),
            "--out",
            baked_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run anima-bake lighting");
    assert!(
        out_l.status.success(),
        "lighting failed: {}",
        String::from_utf8_lossy(&out_l.stderr)
    );

    let out_p = Command::new(bin)
        .args([
            "pack",
            "--in",
            baked_dir.to_str().unwrap(),
            "--raw",
            raw_dir.to_str().unwrap(),
            "--sim",
            sim_path.to_str().unwrap(),
            "--release-tag",
            "world-v2-cli-smoke",
            "--out",
            pack_dir.to_str().unwrap(),
            "--verify",
        ])
        .output()
        .expect("run anima-bake pack");
    assert!(
        out_p.status.success(),
        "pack failed: {}",
        String::from_utf8_lossy(&out_p.stderr)
    );

    // Explicitly test `anima-bake verify` subcommand on produced index/sim/archive.
    let sum_txt = fs::read_to_string(pack_dir.join("pack_summary.json")).unwrap();
    let sum_val: serde_json::Value = serde_json::from_str(&sum_txt).unwrap();
    let arch_name = sum_val["archive_filename"].as_str().unwrap();
    let out_v = Command::new(bin)
        .args([
            "verify",
            "--index",
            pack_dir.join("index.json").to_str().unwrap(),
            "--sim",
            pack_dir.join("sim_256.anmw3").to_str().unwrap(),
            "--archive",
            pack_dir.join(arch_name).to_str().unwrap(),
        ])
        .output()
        .expect("run anima-bake verify");
    assert!(
        out_v.status.success(),
        "verify failed: {}",
        String::from_utf8_lossy(&out_v.stderr)
    );

    let _ = fs::remove_dir_all(&root);
}
