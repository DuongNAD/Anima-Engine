#![cfg(feature = "generate")]
#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

use anima_worldgen::anmw_v3::{compute_sim_world_identity, sha256_to_hex, AnmwV3Sim};
use anima_worldgen::caves::CaveNetwork;
use anima_worldgen::config::WorldV2Config;
use anima_worldgen::lakes_falls::{LakeBasinRecord, LakeClassification, WaterfallRecord};
use anima_worldgen::pipeline::run_worldgen_to_dir;
use anima_worldgen::verify::g_comp::{evaluate_c4_c5_and_flags, finalize_g_comp_v2};
use anima_worldgen::verify::g_shape::{evaluate_g_shape, GShapeInput, HackLawReport};
use anima_worldgen::verify::{
    verify_dem_file, verify_raw_dir, VerificationReport, WorldV2IndexJson,
};
use anima_worldgen::{BiomeId, WaterKind};
use std::path::{Path, PathBuf};
use std::process::Command;

fn temp_test_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("anima_wv2_4_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

/// Point-to-segment distance in grid cell units.
fn dist_to_segment(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let abx = bx - ax;
    let aby = by - ay;
    let len2 = abx * abx + aby * aby;
    if len2 <= 1e-12 {
        return libm::hypot(px - ax, py - ay);
    }
    let t = (((px - ax) * abx + (py - ay) * aby) / len2).clamp(0.0, 1.0);
    let qx = ax + t * abx;
    let qy = ay + t * aby;
    libm::hypot(px - qx, py - qy)
}

#[test]
fn neg_control_a_parallel_comb_dem_fails_junction_angle_and_horton() {
    let n = 192_usize;
    let dx = 40.0_f32;
    let mut dem = vec![0.0_f32; n * n];

    // Orthogonal parallel-comb drainage: straight N-S trunk at `c = 160` fed by 7 perpendicular
    // E-W ribs (`r = 24, 48, 72, 96, 120, 144, 168`) at 90 deg joins, plus ocean below `r >= 182`.
    // Each E-W rib receives a perpendicular N-S feeder trench at `c = 70` draining a wide hillslope
    // sub-basin so every rib reaches Strahler `omega >= 2` upstream of `c = 160` and actually
    // registers as an order>=2 + order>=2 confluence in Metric 2 (not a vacuous 0-junction pass).
    for r in 0..n {
        for c in 0..n {
            let idx = r * n + c;
            if r >= 182 {
                dem[idx] = -25.0;
                continue;
            }
            let h_trunk = 360.0 - 1.6 * r as f64;
            let trunk_d = (c as f64 - 160.0).abs();
            let mut h = h_trunk + 18.0 * trunk_d;

            if c <= 160 && (13..=179).contains(&r) {
                let rib_k = ((((r as f64 - 24.0) / 24.0).round() * 24.0) + 24.0).clamp(24.0, 168.0);
                let rib_trunk = 360.0 - 1.6 * rib_k;
                let rib_bed = rib_trunk + 2.4 * (160.0 - c as f64);
                let z_rib = rib_bed + 8.0 * (r as f64 - rib_k).abs();
                h = h.min(z_rib);

                // Orthogonal N-S feeder trench on the north side of each rib at `c = 70` (`r in [rib_k-11, rib_k]`)
                if r as f64 <= rib_k
                    && (rib_k - r as f64) <= 11.5
                    && (c as f64 - 70.0).abs() <= 24.0
                {
                    let feeder_bed_at_70 = rib_trunk + 2.4 * (160.0 - 70.0);
                    let z_feeder =
                        feeder_bed_at_70 + 3.2 * (rib_k - r as f64) + 1.6 * (c as f64 - 70.0).abs();
                    h = h.min(z_feeder);
                }
            }
            dem[idx] = h.max(1.0) as f32;
        }
    }

    let out = evaluate_g_shape(&GShapeInput::from_dem_only(&dem, n, n, dx, 0.0));
    assert!(!out.report.passed, "parallel-comb DEM must fail G-SHAPE");
    assert!(
        !out.report.metric_2_junction_angles.passed,
        "orthogonal 90 deg parallel-comb joins must fail Metric 2 (got median={:.1} deg, n={})",
        out.report.metric_2_junction_angles.median_deg,
        out.report.metric_2_junction_angles.junction_count
    );
    assert!(
        out.report.metric_2_junction_angles.junction_count >= 5,
        "parallel-comb DEM must register >= 5 order>=2 confluences so Metric 2 fails on 90 deg geometry rather than 0 junctions (got {})",
        out.report.metric_2_junction_angles.junction_count
    );
    assert!(
        out.report.metric_2_junction_angles.median_deg > 78.0,
        "orthogonal parallel-comb joins must fail Metric 2 via high median angle > 78 deg (got {:.2})",
        out.report.metric_2_junction_angles.median_deg
    );
    assert!(
        !out.report.metric_1_strahler_horton.passed,
        "parallel-comb network must fail Metric 1 Strahler/Horton (omega_max={}, r_b={:.2}, r_l={:.2})",
        out.report.metric_1_strahler_horton.omega_max,
        out.report.metric_1_strahler_horton.r_b,
        out.report.metric_1_strahler_horton.r_l
    );
}

#[test]
fn neg_control_b_cratered_noise_dem_fails_micro_pits_and_hypsometry() {
    let n = 128_usize;
    let dx = 40.0_f32;
    let mut dem = vec![0.0_f32; n * n];

    // Smooth coastal dome corrupted by unmodeled micro-craters and >75 deg needle spikes.
    for r in 0..n {
        for c in 0..n {
            let idx = r * n + c;
            if r >= 112 {
                dem[idx] = -30.0;
                continue;
            }
            let norm_y = (112 - r) as f32 / 112.0;
            let base = 40.0 + 450.0 * norm_y;
            let pit = if r > 8 && r < 105 && c > 8 && c < 120 && (r % 13 == 4) && (c % 13 == 5) {
                -60.0
            } else {
                0.0
            };
            let spike = if r > 10 && r < 102 && c > 10 && c < 118 && (r % 19 == 7) && (c % 19 == 9)
            {
                260.0 // > 260/40 = 6.5 >> tan(75 deg) = 3.732 opposing slope reversal!
            } else {
                0.0
            };
            dem[idx] = base + pit + spike;
        }
    }

    let out = evaluate_g_shape(&GShapeInput::from_dem_only(&dem, n, n, dx, 0.0));
    assert!(!out.report.passed, "cratered/spiky DEM must fail G-SHAPE");
    assert!(
        !out.report.metric_6_endorheic_spillways.passed,
        "cratered noise DEM must fail Metric 6"
    );
    assert!(
        out.report
            .metric_6_endorheic_spillways
            .raw_non_water_land_pits
            > 0
            && out.report.metric_6_endorheic_spillways.unmodeled_micro_pits > 0,
        "expected unmodeled crater pits > 0, got {:?}",
        out.report.metric_6_endorheic_spillways
    );
    assert!(
        !out.report.metric_7_hypsometry_spikes.passed
            && out
                .report
                .metric_7_hypsometry_spikes
                .non_cliff_needle_spikes
                > 0,
        "spiky DEM must detect >75 deg needle spikes in Metric 7, got {:?}",
        out.report.metric_7_hypsometry_spikes
    );
}

#[test]
fn neg_control_c_inverted_valley_fails_vf_ratio() {
    let n = 160_usize;
    let dx = 40.0_f32;
    let mut dem = vec![0.0_f32; n * n];

    // Symmetric binary branching river tree producing Strahler orders 1 -> 2 -> 3 -> 4 at r=95..148:
    // Order 4 trunk: (80, 95) -> (80, 148)
    // Order 3: (60, 75)->(80, 95) and (100, 75)->(80, 95)
    // Order 2: (46, 55)->(60, 75), (72, 55)->(60, 75), (88, 55)->(100, 75), (114, 55)->(100, 75)
    // Order 1 tips feeding each order-2 node from r = 24..55.
    let segs: [(f64, f64, f64, f64, f64, f64); 15] = [
        // (ax, ay, bx, by, elev_a, elev_b)
        (80.0, 95.0, 80.0, 149.0, 140.0, 4.0), // trunk (order 4)
        (60.0, 75.0, 80.0, 95.0, 230.0, 140.0), // W order 3
        (100.0, 75.0, 80.0, 95.0, 230.0, 140.0), // E order 3
        (46.0, 55.0, 60.0, 75.0, 320.0, 230.0), // order 2
        (72.0, 55.0, 60.0, 75.0, 320.0, 230.0),
        (88.0, 55.0, 100.0, 75.0, 320.0, 230.0),
        (114.0, 55.0, 100.0, 75.0, 320.0, 230.0),
        // 8 order-1 headwaters
        (36.0, 24.0, 46.0, 55.0, 480.0, 320.0),
        (54.0, 24.0, 46.0, 55.0, 480.0, 320.0),
        (65.0, 24.0, 72.0, 55.0, 480.0, 320.0),
        (78.0, 24.0, 72.0, 55.0, 480.0, 320.0),
        (82.0, 24.0, 88.0, 55.0, 480.0, 320.0),
        (95.0, 24.0, 88.0, 55.0, 480.0, 320.0),
        (106.0, 24.0, 114.0, 55.0, 480.0, 320.0),
        (124.0, 24.0, 114.0, 55.0, 480.0, 320.0),
    ];

    for r in 0..n {
        for c in 0..n {
            let idx = r * n + c;
            if r >= 150 {
                dem[idx] = -20.0;
                continue;
            }
            let x = c as f64;
            let y = r as f64;
            let mut best_d = f64::INFINITY;
            let mut best_axis_z = 500.0_f64;
            for &(ax, ay, bx, by, za, zb) in &segs {
                let d = dist_to_segment(x, y, ax, ay, bx, by);
                if d < best_d {
                    best_d = d;
                    let abx = bx - ax;
                    let aby = by - ay;
                    let len2 = (abx * abx + aby * aby).max(1e-9);
                    let t = (((x - ax) * abx + (y - ay) * aby) / len2).clamp(0.0, 1.0);
                    best_axis_z = za + t * (zb - za);
                }
            }
            let dist_m = best_d * f64::from(dx);
            // Inverted geometry (`Card 5 (c)`): wide flat U-troughs upper (`r < 93`), sharp V-notch down (`r >= 93`).
            let wall = if r < 93 {
                // Broad flat valley floor up to ~200 m half-width, gentle power >= 2.2 walls.
                if dist_m <= 160.0 {
                    0.002 * dist_m
                } else {
                    0.32 + 0.0005 * libm::pow(dist_m - 160.0, 2.1)
                }
            } else {
                // Deep narrow downstream V-notch (`b ~ 1.0`, steep wall rising immediately).
                0.30 * dist_m
            };
            dem[idx] = (best_axis_z + wall.min(180.0)).max(0.5) as f32;
        }
    }

    let out = evaluate_g_shape(&GShapeInput::from_dem_only(&dem, n, n, dx, 0.0));
    let m5 = &out.report.metric_5_valleys_flint;
    assert!(
        m5.upper_samples > 0 && m5.downstream_samples > 0,
        "expected both upper (order 2/3) and downstream (order 4) samples, got {:?}",
        m5
    );
    assert!(
        !m5.passed,
        "inverted valley DEM must fail Metric 5: {:?}",
        m5
    );
    assert!(
        m5.vf_ratio_down_over_up < 2.5,
        "inverted valley must yield Vf_down / Vf_up < 2.5 (got {:.3}, vf_up={:.3}, vf_dn={:.3})",
        m5.vf_ratio_down_over_up,
        m5.vf_up_median,
        m5.vf_down_median
    );
}

#[test]
fn neg_control_d_uniform_or_steppe_biome_fails_g_comp_v2() {
    let size = 64_usize;
    let n = size * size;
    let dx = 160.0_f32;
    let extent = size as f32 * dx;

    let mut total_h = vec![200.0_f32; n];
    let mut water_h = vec![200.0_f32; n];
    let mut water_kind = vec![WaterKind::None as u8; n];
    let glacier_h = vec![0.0_f32; n];
    let mut river_w = vec![0.0_f32; n];
    let is_channel = vec![false; n];

    // First 29 rows (~45% area) ocean, gentle/cliff coastal slopes, and ~2% freshwater lake basin.
    for r in 0..size {
        for c in 0..size {
            let i = r * size + c;
            if r < 29 {
                total_h[i] = -100.0;
                water_h[i] = 0.0;
                water_kind[i] = WaterKind::Ocean as u8;
            } else if r == 29 {
                total_h[i] = if c < 32 { 15.0 } else { 90.0 };
                water_h[i] = total_h[i];
            } else {
                total_h[i] = 200.0 + (r - 29) as f32 * 100.0;
                water_h[i] = total_h[i];
            }
        }
    }
    // Inject a 90-cell lake (`~2.2%`) and minor channel widths.
    for r in 34..43 {
        for c in 20..30 {
            let i = r * size + c;
            water_kind[i] = WaterKind::FreshwaterLake as u8;
            water_h[i] = total_h[i] + 15.0;
        }
    }
    river_w[32 * size + 10] = 2.0;

    let summary_base = || {
        evaluate_c4_c5_and_flags(
            size,
            size,
            dx,
            0.0,
            &total_h,
            &water_h,
            &water_kind,
            &glacier_h,
            &river_w,
            &is_channel,
            &[(2, 5, 2.0), (3, 5, 5.0), (4, 5, 10.0)],
            &HackLawReport {
                passed: true,
                sample_count: 500,
                exponent_h: 0.57,
                prefactor_c_km: 1.4,
                r_squared: 0.95,
                max_path_km: 25.0,
                median_mainstem_sinuosity: 1.3,
            },
            3,
            2,
        )
    };

    // Valid waterfall/lake/cave scaffolding so ONLY biome partition differs between controls.
    let wfs: Vec<WaterfallRecord> = (0..25)
        .map(|k| WaterfallRecord {
            lip_cell: (32 * size + k) as u32,
            base_cell: (33 * size + k) as u32,
            x_m: k as f32 * dx,
            z_m: 32.0 * dx,
            lip_elev_m: 300.0,
            drop_m: 20.0,
            yaw_rad: 0.0,
            discharge_q_m3_s: 5.0,
            plunge_depth_m: 3.0,
        })
        .collect();
    let basins: Vec<LakeBasinRecord> = (0..6)
        .map(|k| LakeBasinRecord {
            id: (k + 1) as u32,
            water_kind: WaterKind::FreshwaterLake as u8,
            classification: if k == 0 {
                LakeClassification::HighParamoGlacial
            } else if k == 1 {
                LakeClassification::CalderaOrTrough
            } else {
                LakeClassification::FreshwaterSpilling
            },
            water_level_m: if k == 0 { 3500.0 } else { 800.0 },
            spill_elev_m: 801.0,
            pit_elev_m: 780.0,
            area_m2: 150_000.0,
            max_depth_m: 20.0,
            mean_elev_m: 790.0,
            max_wall_slope_deg: if k == 1 { 32.0 } else { 15.0 },
            elongation: 2.0,
            outlet_cell: 0,
            q_in_m3_s: 2.0,
            q_out_m3_s: 2.0,
            spillway_reaches_outlet: true,
        })
        .collect();
    let mut cave_nodes = Vec::new();
    let mut cave_entrances = Vec::new();
    for k in 0..32_usize {
        let gx = ((k * 5 + 3) % size) as f32 * dx;
        let gz = (45 + (k / 12) * 5) as f32 * dx;
        cave_nodes.push(anima_worldgen::caves::CaveNode {
            x_m: gx,
            y_m: 200.0,
            z_m: gz,
            radius_m: 4.0,
            light_decay: 0.1,
        });
        cave_entrances.push(anima_worldgen::caves::CaveEntrance {
            node_idx: k as u32,
            mouth_kind: 0,
            realm_status: 1,
            yaw_rad: 0.0,
            elevation_m: 200.0,
            clearance_m: 3.0,
        });
    }
    let caves = CaveNetwork {
        flags: 0,
        nodes: cave_nodes,
        edges: Vec::new(),
        entrances: cave_entrances,
    };

    // Variant D1: uniform single terrestrial biome (`Forest` everywhere) + zero ecotone blending.
    let mut dom_uniform = vec![BiomeId::Forest as u8; n];
    for r in 0..29 {
        for c in 0..size {
            dom_uniform[r * size + c] = BiomeId::Ocean as u8;
        }
    }
    let ids_uniform = vec![[BiomeId::Forest as u8, 0, 0, 0]; n];
    let wts_uniform = vec![[255_u8, 0, 0, 0]; n];

    let rep_uniform = finalize_g_comp_v2(
        size,
        size,
        dx,
        extent,
        &water_kind,
        &dom_uniform,
        &ids_uniform,
        &wts_uniform,
        &wfs,
        &basins,
        12,
        &caves,
        summary_base(),
    );
    assert!(
        !rep_uniform.c1_partition.passed,
        "uniform biome must fail G-COMP v2 C1"
    );
    assert_eq!(rep_uniform.c1_partition.zoned_16_biomes_passing, 1);
    assert!(
        !rep_uniform.c2_ecotone.passed,
        "zero-ecotone world must fail G-COMP v2 C2"
    );

    // Variant D2: all 16 zoned biomes have >= min_patch contiguous blocks, EXCEPT 1 Steppe cell is
    // planted (`Card 5 (d)`): C1 fails ONLY while Steppe > 0, and flips to PASS once replaced!
    let min_req = rep_uniform.c1_partition.min_patch_cells_required;
    let block_w = ((libm::sqrt(min_req as f64)).ceil() as usize).max(4);
    let mut dom_16 = dom_uniform;
    for (bi_idx, &b) in BiomeId::ZONED_TERRESTRIAL_16.iter().enumerate() {
        let br = 30 + (bi_idx / 4) * block_w;
        let bc = 2 + (bi_idx % 4) * block_w;
        for dr in 0..block_w {
            for dc in 0..block_w {
                let r = (br + dr).min(size - 1);
                let c = (bc + dc).min(size - 1);
                dom_16[r * size + c] = b as u8;
            }
        }
    }
    // Place Steppe contaminant on an unused border cell (30, 0).
    dom_16[30 * size] = BiomeId::Steppe as u8;
    let mut ids_blend = vec![[BiomeId::Forest as u8, BiomeId::Grassland as u8, 0, 0]; n];
    let mut wts_blend = vec![[180_u8, 75, 0, 0]; n];
    for i in 0..n {
        ids_blend[i][0] = dom_16[i];
        wts_blend[i] = [180, 75, 0, 0];
    }

    let rep_steppe = finalize_g_comp_v2(
        size,
        size,
        dx,
        extent,
        &water_kind,
        &dom_16,
        &ids_blend,
        &wts_blend,
        &wfs,
        &basins,
        12,
        &caves,
        summary_base(),
    );
    assert!(
        !rep_steppe.c1_partition.passed && rep_steppe.c1_partition.steppe_cells > 0,
        "Steppe presence must fail G-COMP v2 C1"
    );
    assert_eq!(rep_steppe.c1_partition.zoned_16_biomes_passing, 16);

    // Clear Steppe -> C1 passes!
    dom_16[30 * size] = BiomeId::Forest as u8;
    let rep_clean = finalize_g_comp_v2(
        size,
        size,
        dx,
        extent,
        &water_kind,
        &dom_16,
        &ids_blend,
        &wts_blend,
        &wfs,
        &basins,
        12,
        &caves,
        summary_base(),
    );
    assert!(
        rep_clean.c1_partition.passed,
        "expected C1 to pass once Steppe removed: {:?}",
        rep_clean.c1_partition
    );
}

#[test]
fn cli_generate_preset_defaults_verify_gates_and_sorted_index_identity() {
    let root = temp_test_dir("cli_e2e");
    let raw_dir = root.join("raw");
    let preset_path = root.join("preset.toml");
    let gates_path = root.join("gates.json");
    let sim_path = root.join("sim_256.anmw3");
    let idx_path = root.join("index.json");

    // Generate using `--preset` ONLY (verifying seed & grid_size default cleanly from TOML).
    std::fs::write(
        &preset_path,
        "seed = 0x57563201\ngrid_size = 256\nschema_tag = \"wv2_test_preset\"\ntarget_ocean_fraction = 0.45\n",
    )
    .expect("write preset");

    let bin = env!("CARGO_BIN_EXE_anima-worldgen");
    let gen_out = Command::new(bin)
        .args([
            "generate",
            "--preset",
            preset_path.to_str().unwrap(),
            "--out",
            raw_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run anima-worldgen generate");
    assert!(
        gen_out.status.success(),
        "generate with preset failed: stderr={}",
        String::from_utf8_lossy(&gen_out.stderr)
    );

    // Run `verify --raw ... --report-only` to produce `gates.json`, `sim_256.anmw3`, `index.json`.
    let ver_out = Command::new(bin)
        .args([
            "verify",
            "--raw",
            raw_dir.to_str().unwrap(),
            "--out-gates",
            gates_path.to_str().unwrap(),
            "--out-sim",
            sim_path.to_str().unwrap(),
            "--out-index",
            idx_path.to_str().unwrap(),
            "--report-only",
        ])
        .output()
        .expect("run anima-worldgen verify");
    assert!(
        ver_out.status.success(),
        "verify --report-only failed: stderr={}",
        String::from_utf8_lossy(&ver_out.stderr)
    );

    let gates_bytes = std::fs::read(&gates_path).expect("read gates.json");
    let gates_val: serde_json::Value =
        serde_json::from_slice(&gates_bytes).expect("parse gates.json");
    assert_eq!(gates_val["width"], 256);
    assert_eq!(gates_val["seed"], 0x5756_3201_u64);

    let sim_bytes = std::fs::read(&sim_path).expect("read sim_256.anmw3");
    let sim = AnmwV3Sim::from_bytes(&sim_bytes).expect("decode sim_256.anmw3");
    assert_eq!(sim.width, 256);
    assert_eq!(sim.seed_u64, 0x5756_3201);

    let idx_bytes = std::fs::read(&idx_path).expect("read index.json");
    let idx: WorldV2IndexJson = serde_json::from_slice(&idx_bytes).expect("parse index.json");
    let expected_sim_id = sha256_to_hex(&compute_sim_world_identity(&sim_bytes));
    assert_eq!(idx.sim_world_identity, expected_sim_id);
    assert_eq!(idx_bytes, idx.to_canonical_json_bytes().unwrap());

    // Negative control: 1-byte tamper alters sim_world_identity immediately.
    let mut tampered = sim_bytes.clone();
    tampered[60] ^= 0x01;
    assert_ne!(
        sha256_to_hex(&compute_sim_world_identity(&tampered)),
        idx.sim_world_identity
    );

    // Test `--dem-input` mode on a small cratered DEM: strict fail exits 1 and writes gates.json.
    let dem_path = root.join("crater.f32");
    let dem_gates = root.join("dem_gates.json");
    let mut dem_bytes = Vec::with_capacity(64 * 64 * 4);
    for r in 0..64 {
        for c in 0..64 {
            let v = if r >= 56 {
                -10.0_f32
            } else if r == 20 && c == 20 {
                -5.0_f32
            } else {
                20.0 + (56 - r) as f32 * 2.0
            };
            dem_bytes.extend_from_slice(&v.to_le_bytes());
        }
    }
    std::fs::write(&dem_path, dem_bytes).expect("write crater.f32");
    let dem_strict = Command::new(bin)
        .args([
            "verify",
            "--dem-input",
            dem_path.to_str().unwrap(),
            "--size",
            "64",
            "--dx",
            "40",
            "--strict-gates",
            "--report",
            dem_gates.to_str().unwrap(),
        ])
        .output()
        .expect("run --dem-input strict");
    assert_eq!(
        dem_strict.status.code(),
        Some(1),
        "strict cratered --dem-input verify must exit 1"
    );
    assert!(
        dem_gates.exists(),
        "gates.json must still be written on exit 1"
    );

    // Also confirm `WorldV2Config::for_size` round-trips bit-identically with CLI `--preset`.
    let mut lib_cfg = WorldV2Config::for_size(0x5756_3201, 256);
    lib_cfg.schema_tag = "wv2_test_preset".to_owned();
    lib_cfg.target_ocean_fraction = 0.45;
    let lib_meta = run_worldgen_to_dir(&lib_cfg, &root.join("raw_lib"))
        .expect("run 256x256 library parity worldgen");
    assert_eq!(gates_val["world_sha256"], lib_meta.world_sha256);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn hillslope_rills_below_q_min_excluded_from_channel_mask() {
    let n = 128_usize;
    let dx = 40.0_f32;
    let mut dem = vec![0.0_f32; n * n];
    // Steep concave convergent bowl draining down to r = 115 (`S > 0.7`, so `A * S^2 >= 500` even at 1 cell).
    for r in 0..n {
        for c in 0..n {
            let idx = r * n + c;
            if r >= 118 {
                dem[idx] = -20.0;
            } else {
                let lat = (c as f32 - 64.0).abs();
                dem[idx] = 20.0 + (118 - r) as f32 * 30.0 + lat * 28.0;
            }
        }
    }

    let mut base_in = GShapeInput::from_dem_only(&dem, n, n, dx, 0.0);
    base_in.q_min_m3_s = 0.0;
    let out_no_qmin = evaluate_g_shape(&base_in);

    let mut qmin_in = GShapeInput::from_dem_only(&dem, n, n, dx, 0.0);
    qmin_in.q_min_m3_s = 0.027_778;
    let out_with_qmin = evaluate_g_shape(&qmin_in);

    let cnt_no_qmin = out_no_qmin.is_channel.iter().filter(|&&c| c).count();
    let cnt_with_qmin = out_with_qmin.is_channel.iter().filter(|&&c| c).count();
    assert!(
        cnt_with_qmin > 0 && cnt_with_qmin * 2 < cnt_no_qmin,
        "Q_min gate must exclude shallow steep hillside trickles while retaining mainstem channels (no_qmin={cnt_no_qmin}, with_qmin={cnt_with_qmin})"
    );
    assert!(
        out_with_qmin
            .report
            .metric_3_drainage_density
            .global_km_per_km2
            < out_no_qmin
                .report
                .metric_3_drainage_density
                .global_km_per_km2
    );
}

#[test]
fn metric_6_rejects_nonzero_humid_endorheic_fraction() {
    let n = 64_usize;
    let dx = 40.0_f32;
    let mut dem = vec![0.0_f32; n * n];
    for r in 0..n {
        for c in 0..n {
            let idx = r * n + c;
            dem[idx] = if r >= 58 {
                -20.0
            } else {
                10.0 + (58 - r) as f32 * 4.0 + (c as f32 - 32.0).abs() * 1.5
            };
        }
    }
    let mut g_in = GShapeInput::from_dem_only(&dem, n, n, dx, 0.0);
    g_in.reported_unmodeled_micro_pits = Some(0);
    g_in.reported_severed_humid_spillways = Some(0);
    g_in.reported_endorheic_land_fraction = Some(1e-7);
    g_in.reported_endorheic_humid_fraction = Some(1e-7);
    let out = evaluate_g_shape(&g_in);
    assert!(
        !out.report.metric_6_endorheic_spillways.passed,
        "Metric 6 requires 0% endorheic humid fraction; 1e-7 must fail"
    );
}

#[test]
fn verify_raw_dir_rejects_malformed_meta_and_nan_elevation() {
    let root = temp_test_dir("raw_malformed_nan");
    let valid_dir = root.join("valid_256");
    let mut cfg = WorldV2Config::for_size(0x5756_3201, 256);
    cfg.target_ocean_fraction = 0.45;
    run_worldgen_to_dir(&cfg, &valid_dir).expect("generate 256x256 baseline raw dir");

    // 1. Copy and tamper `raw_meta.json` width/height to 8x8 -> must return Err cleanly (no panic).
    let bad_meta_dir = root.join("bad_meta");
    std::fs::create_dir_all(&bad_meta_dir).expect("create bad_meta dir");
    let meta_txt = std::fs::read_to_string(valid_dir.join("raw_meta.json")).unwrap();
    let mut meta_val: serde_json::Value = serde_json::from_str(&meta_txt).unwrap();
    meta_val["width"] = serde_json::json!(8);
    meta_val["height"] = serde_json::json!(8);
    std::fs::write(
        bad_meta_dir.join("raw_meta.json"),
        serde_json::to_vec(&meta_val).unwrap(),
    )
    .unwrap();
    let err_meta = verify_raw_dir(&bad_meta_dir, 3, None, None)
        .expect_err("malformed 8x8 raw_meta.json must fail cleanly with Err");
    assert!(
        err_meta.message.contains("invalid raw_meta.json geometry"),
        "unexpected error for malformed geometry: {}",
        err_meta.message
    );

    // 2. Corrupt `total_h_m.bin` element 0 with NaN in-place -> must return Err cleanly.
    let nan_dir = root.join("nan_h");
    std::fs::create_dir_all(&nan_dir).expect("create nan_h dir");
    for entry in std::fs::read_dir(&valid_dir).unwrap() {
        let entry = entry.unwrap();
        let dst = nan_dir.join(entry.file_name());
        std::fs::copy(entry.path(), &dst).unwrap();
    }
    let h_path = nan_dir.join("total_h_m.bin");
    let mut h_bytes = std::fs::read(&h_path).unwrap();
    h_bytes[..4].copy_from_slice(&f32::NAN.to_le_bytes());
    std::fs::write(&h_path, &h_bytes).unwrap();
    let err_nan = verify_raw_dir(&nan_dir, 3, None, None)
        .expect_err("NaN in raw total_h_m.bin must fail cleanly with Err");
    assert!(
        err_nan.message.contains("NaN or Inf"),
        "unexpected error for NaN elevation: {}",
        err_nan.message
    );

    // 3. Reject non-finite sea_level_m in verify_dem_file cleanly with Err.
    let err_dem_sl = verify_dem_file(&valid_dir.join("total_h_m.bin"), 256, 256, 40.0, f32::NAN)
        .expect_err("NaN sea_level_m in verify_dem_file must return Err");
    assert!(
        err_dem_sl.message.contains("invalid DEM geometry"),
        "unexpected error for NaN sea_level_m: {}",
        err_dem_sl.message
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn official_world_v2_provisional_c01_artifacts_integrity_and_passing_gates() {
    let artifacts_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../artifacts");
    let v2_dir = artifacts_root.join("world-v2");

    let preset_txt = std::fs::read_to_string(v2_dir.join("preset_v2.toml"))
        .expect("read committed artifacts/world-v2/preset_v2.toml");
    let cfg: WorldV2Config = toml::from_str(&preset_txt).expect("parse preset_v2.toml");
    cfg.validate().expect("preset_v2.toml must be valid");
    assert_eq!(cfg.seed, 0x5756_3201);
    assert_eq!(cfg.grid_size, 4096);
    assert_eq!(cfg.schema_tag, "ANMW3-SNSM-v1");

    let sim_bytes = std::fs::read(v2_dir.join("sim_256.anmw3"))
        .expect("read committed artifacts/world-v2/sim_256.anmw3");
    let peeked = AnmwV3Sim::peek_seed(&sim_bytes).expect("peek v3 seed");
    assert_eq!(peeked, cfg.seed as u32);
    let sim = AnmwV3Sim::from_bytes(&sim_bytes).expect("decode sim_256.anmw3");
    assert_eq!(sim.width, 256);
    assert_eq!(sim.height, 256);
    assert_eq!(sim.seed_u64, cfg.seed);

    let index_bytes = std::fs::read(v2_dir.join("index.json"))
        .expect("read committed artifacts/world-v2/index.json");
    let idx: WorldV2IndexJson =
        serde_json::from_slice(&index_bytes).expect("parse artifacts/world-v2/index.json");
    assert_eq!(
        idx.to_canonical_json_bytes().expect("canonicalize index"),
        index_bytes,
        "committed index.json must stay deterministically sorted and canonical"
    );
    assert_eq!(idx.sim_256_byte_len, sim_bytes.len());
    assert_eq!(idx.grid_size, cfg.grid_size);
    assert_eq!(idx.seed, cfg.seed);
    assert_eq!(idx.schema_tag, cfg.schema_tag);
    assert_eq!(
        sha256_to_hex(&compute_sim_world_identity(&sim_bytes)),
        idx.sim_world_identity,
        "index.json sim_world_identity must equal sha256(sim_256.anmw3)"
    );

    let gates_bytes = std::fs::read(v2_dir.join("gates.json"))
        .expect("read committed artifacts/world-v2/gates.json");
    let gates: VerificationReport =
        serde_json::from_slice(&gates_bytes).expect("parse artifacts/world-v2/gates.json");
    assert_eq!(gates.status.as_deref(), Some("provisional"));
    assert_eq!(gates.decision.as_deref(), Some("MAP-D4"));
    assert_eq!(
        gates.sim_world_identity.as_deref(),
        Some(idx.sim_world_identity.as_str())
    );
    assert_eq!(gates.width, 4096);
    assert_eq!(gates.height, 4096);
    assert_eq!(gates.seed, Some(0x5756_3201));

    let endo = &gates.g_shape.metric_6_endorheic_spillways;
    assert!(
        endo.passed,
        "provisional C01 passes G-SHAPE.6 endorheic spillways"
    );
    assert_eq!(endo.unmodeled_micro_pits, 0);
    assert_eq!(endo.severed_humid_spillways, 0);
    assert_eq!(endo.endorheic_humid_fraction, 0.0);

    let comp = gates.g_comp_v2.as_ref().expect("g_comp_v2 present");
    assert!(
        comp.c2_ecotone.passed,
        "provisional C01 passes G-COMP-v2.C2 ecotone continuity"
    );

    let sim_rep = gates.g_sim_raw.as_ref().expect("g_sim_raw present");
    assert!(sim_rep.round_trip_ok);
    assert!(sim_rep.max_mean_norm_elev_diff <= 1e-5);
    assert!(sim_rep.all_biomes_in_range);
    assert_eq!(sim_rep.water_mask_mismatches, 0);
    assert!(sim_rep.largest_walkable_fraction >= 0.80);

    let legacy = std::fs::read(artifacts_root.join("world_256.anmw"))
        .expect("legacy artifacts/world_256.anmw must remain untouched");
    assert!(legacy.len() >= 8);
    assert_eq!(&legacy[0..4], b"ANMW");
    assert_eq!(
        u32::from_le_bytes([legacy[4], legacy[5], legacy[6], legacy[7]]),
        2
    );
}

#[test]
#[ignore = "MAP-D4: G-SHAPE (1-5, 7, 4.2), G-COMP v2 (C1, C3-C5), and G-SIM (GS-1, GS-2) not met by provisional C01, see docs/ai/planning/2026-11-24-wv2-4.md"]
fn official_world_v2_c01_passes_all_15_g_shape_g_comp_and_g_sim_gates() {
    let v2_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../artifacts/world-v2");
    let gates_bytes = std::fs::read(v2_dir.join("gates.json"))
        .expect("read committed artifacts/world-v2/gates.json");
    let gates: VerificationReport =
        serde_json::from_slice(&gates_bytes).expect("parse artifacts/world-v2/gates.json");
    assert!(
        gates.overall_pass,
        "official World v2 gates.json failed: {:?}",
        gates.failed_gates
    );
    assert!(gates.failed_gates.is_empty(), "expected 0 failed gates");
    assert!(gates.g_shape.passed, "G-SHAPE must pass");
    assert!(
        gates.g_comp_v2.as_ref().expect("g_comp_v2").passed,
        "G-COMP v2 must pass"
    );
    assert!(
        gates.g_sim_raw.as_ref().expect("g_sim_raw").passed,
        "G-SIM raw must pass"
    );
}
