#![cfg(feature = "gpu-bake")]
#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

use anima_worldgen::bake::lighting_cpu::{bake_lighting_cpu_region, load_lighting_inputs_from_raw};
use anima_worldgen::bake::lighting_gpu::{
    bake_lighting_wgpu_region, compare_lighting_parity, probe_gpu,
};
use anima_worldgen::bake::{HORIZON_HALO_CELLS, TILE_ACTIVE_SIZE};
use anima_worldgen::config::WorldV2Config;
use anima_worldgen::pipeline::run_worldgen_to_dir;
use std::fs;
use std::path::PathBuf;

fn temp_gpu_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "anima_wv2_5_gpu_{tag}_{}_{}",
        std::process::id(),
        line!()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp gpu test dir");
    dir
}

#[test]
fn gpu_probe_enumerates_adapter_and_formats_json() {
    let rep = probe_gpu().expect("probe-gpu should enumerate >= 1 wgpu adapter");
    assert!(rep.adapter_count >= 1 || rep.selected.is_some());
    let sel = rep.selected.as_ref().expect("selected adapter present");
    assert!(!sel.name.is_empty());
    let json = serde_json::to_string_pretty(&rep).expect("serialize gpu probe report");
    assert!(json.contains("\"adapter_count\""));
}

#[test]
fn gpu_horizon_lighting_matches_cpu_stewart_sweep_within_card6_tolerance() {
    let root = temp_gpu_dir("parity_256");
    let raw_dir = root.join("raw");
    let mut cfg = WorldV2Config::for_size(0x5756_3201, 256);
    cfg.target_ocean_fraction = 0.45;
    run_worldgen_to_dir(&cfg, &raw_dir).expect("generate 256x256 raw terrain");

    let (meta, total_h_m, albedo_luma, macro_albedo) =
        load_lighting_inputs_from_raw(&raw_dir).expect("load lighting inputs");
    let w = meta.width as usize;
    let h = meta.height as usize;

    let cpu = bake_lighting_cpu_region(
        &total_h_m,
        &albedo_luma,
        Some(&macro_albedo),
        w,
        h,
        meta.dx_m,
        HORIZON_HALO_CELLS,
        None,
    )
    .expect("cpu stewart lighting");

    let gpu = bake_lighting_wgpu_region(
        &total_h_m,
        &albedo_luma,
        Some(&macro_albedo),
        w,
        h,
        meta.dx_m,
        HORIZON_HALO_CELLS,
        None,
        64,
    )
    .expect("wgpu horizon lighting");

    let p_light = compare_lighting_parity(&cpu.terrain_lighting, &gpu.terrain_lighting, w, h);
    let p_hor = compare_lighting_parity(&cpu.terrain_horizon, &gpu.terrain_horizon, w, h);

    assert!(
        p_light.max_abs_delta <= 2 && p_light.rmse <= 0.4 && p_light.ssim >= 0.999,
        "GPU vs CPU terrain_lighting parity failed: max_delta={} (<=2) rmse={:.4} (<=0.4) ssim={:.6} (>=0.999)",
        p_light.max_abs_delta,
        p_light.rmse,
        p_light.ssim
    );
    assert!(
        p_hor.max_abs_delta <= 2 && p_hor.rmse <= 0.4 && p_hor.ssim >= 0.999,
        "GPU vs CPU terrain_horizon parity failed: max_delta={} (<=2) rmse={:.4} (<=0.4) ssim={:.6} (>=0.999)",
        p_hor.max_abs_delta,
        p_hor.rmse,
        p_hor.ssim
    );

    // Tile-region GPU call must agree cleanly with full GPU call.
    let gpu_tile = bake_lighting_wgpu_region(
        &total_h_m,
        &albedo_luma,
        Some(&macro_albedo),
        w,
        h,
        meta.dx_m,
        HORIZON_HALO_CELLS,
        Some((0, 0, TILE_ACTIVE_SIZE, TILE_ACTIVE_SIZE)),
        64,
    )
    .expect("tile wgpu");
    assert_eq!(gpu_tile.terrain_lighting, gpu.terrain_lighting);
    assert_eq!(gpu_tile.terrain_horizon, gpu.terrain_horizon);

    let _ = fs::remove_dir_all(&root);
}
