#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

//! `JOB-W0` (`probe-gpu`) & `JOB-W4` (`--backend wgpu`): Optional WebGPU/wgpu compute path
//! (`#![cfg(feature = "gpu-bake")]`, pinned to workspace `wgpu 30.0.2`).

use crate::bake::lighting_cpu::{
    compute_sky_bounce_probe_512, load_lighting_inputs_from_raw, write_u8x4_slice,
    LightingBakeOutput, LightingBakeReport,
};
use crate::bake::pack_ktx2::compute_ssim_u8;
use crate::bake::{quantize_u8, BakeError, TileRange, HORIZON_HALO_CELLS, TILE_ACTIVE_SIZE};
use bytemuck::{Pod, Zeroable};
use futures_executor::block_on;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use wgpu::util::DeviceExt;

const HORIZON_SHADER_WGSL: &str = r#"
struct Params {
    full_w: i32,
    full_h: i32,
    patch_x0: i32,
    patch_y0: i32,
    patch_w: i32,
    patch_h: i32,
    tile_x0: i32,
    tile_y0: i32,
    out_w: u32,
    out_h: u32,
    radius: i32,
    _pad: u32,
    dx_m: f32,
    inv_half_pi: f32,
    _pad2: f32,
    _pad3: f32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> patch_h: array<f32>;
@group(0) @binding(2) var<storage, read> patch_luma: array<f32>;
@group(0) @binding(3) var<storage, read_write> out_lighting: array<u32>;
@group(0) @binding(4) var<storage, read_write> out_horizon: array<u32>;

fn pack_u8(v: f32) -> u32 {
    let c = clamp(v, 0.0, 1.0);
    return min(u32(c * 255.0 + 0.5), 255u);
}

fn pack_rgba8(a: u32, b: u32, c: u32, d: u32) -> u32 {
    return (a & 255u) | ((b & 255u) << 8u) | ((c & 255u) << 16u) | ((d & 255u) << 24u);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.out_w || gid.y >= params.out_h) {
        return;
    }
    let gx = params.tile_x0 + i32(gid.x);
    let gz = params.tile_y0 + i32(gid.y);
    let px = gx - params.patch_x0;
    let pz = gz - params.patch_y0;
    let pidx = pz * params.patch_w + px;
    let h0 = patch_h[pidx];
    let alb_luma = patch_luma[pidx];

    let dirs = array<vec2<i32>, 8>(
        vec2<i32>(1, 0),
        vec2<i32>(-1, 0),
        vec2<i32>(0, 1),
        vec2<i32>(0, -1),
        vec2<i32>(1, 1),
        vec2<i32>(-1, 1),
        vec2<i32>(1, -1),
        vec2<i32>(-1, -1)
    );
    let sqrt2 = 1.41421356237;
    let mults = array<f32, 8>(1.0, 1.0, 1.0, 1.0, sqrt2, sqrt2, sqrt2, sqrt2);

    var sum_cos2 = 0.0;
    var sum_occ = 0.0;
    var card_thetas = array<u32, 4>(0u, 0u, 0u, 0u);

    for (var k = 0u; k < 8u; k = k + 1u) {
        let dir = dirs[k];
        let step_m = params.dx_m * mults[k];
        var max_tan = 0.0;
        for (var d = 1; d <= params.radius; d = d + 1) {
            let nx = gx + d * dir.x;
            let nz = gz + d * dir.y;
            if (nx < 0 || nx >= params.full_w || nz < 0 || nz >= params.full_h) {
                break;
            }
            let np = (nz - params.patch_y0) * params.patch_w + (nx - params.patch_x0);
            let dh = patch_h[np] - h0;
            if (dh > 0.0) {
                let t = dh / (f32(d) * step_m);
                if (t > max_tan) {
                    max_tan = t;
                }
            }
        }
        let theta = atan(max_tan);
        let s = sin(theta);
        let c = cos(theta);
        sum_cos2 = sum_cos2 + c * c;
        sum_occ = sum_occ + clamp(1.0 - s, 0.0, 1.0);
        if (k == 0u) {
            card_thetas[0] = pack_u8(theta * params.inv_half_pi);
        } else if (k == 1u) {
            card_thetas[1] = pack_u8(theta * params.inv_half_pi);
        } else if (k == 2u) {
            card_thetas[3] = pack_u8(theta * params.inv_half_pi);
        } else if (k == 3u) {
            card_thetas[2] = pack_u8(theta * params.inv_half_pi);
        }
    }

    let svf = clamp(sum_cos2 * 0.125, 0.0, 1.0);
    let ao = clamp(sum_occ * 0.125, 0.0, 1.0);
    let bounce_luma = clamp((1.0 - svf) * clamp(alb_luma, 0.0, 1.0) * (0.30 + 0.70 * svf), 0.0, 1.0);

    let oidx = gid.y * params.out_w + gid.x;
    out_lighting[oidx] = pack_rgba8(pack_u8(ao), pack_u8(svf), pack_u8(bounce_luma), 255u);
    out_horizon[oidx] = pack_rgba8(card_thetas[0], card_thetas[1], card_thetas[2], card_thetas[3]);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuHorizonParams {
    full_w: i32,
    full_h: i32,
    patch_x0: i32,
    patch_y0: i32,
    patch_w: i32,
    patch_h: i32,
    tile_x0: i32,
    tile_y0: i32,
    out_w: u32,
    out_h: u32,
    radius: i32,
    _pad: u32,
    dx_m: f32,
    inv_half_pi: f32,
    _pad2: f32,
    _pad3: f32,
}

/// Information about a single GPU adapter discovered by `JOB-W0` (`probe-gpu`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuAdapterSummary {
    pub name: String,
    pub vendor: u32,
    pub device: u32,
    pub device_type: String,
    pub backend: String,
    pub driver: String,
    pub driver_info: String,
}

/// Output JSON report for `anima-bake probe-gpu`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuProbeReport {
    pub adapter_count: usize,
    pub selected: Option<GpuAdapterSummary>,
    pub adapters: Vec<GpuAdapterSummary>,
}

/// CPU-vs-GPU lighting parity measurements against Card 6 limits
/// (`max_abs_delta <= 2`, `rmse <= 0.4`, `ssim >= 0.999`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LightingParityReport {
    pub max_abs_delta: u8,
    pub rmse: f64,
    pub ssim: f64,
}

fn summarize_adapter(info: wgpu::AdapterInfo) -> GpuAdapterSummary {
    GpuAdapterSummary {
        name: info.name,
        vendor: info.vendor,
        device: info.device,
        device_type: format!("{:?}", info.device_type),
        backend: format!("{:?}", info.backend),
        driver: info.driver,
        driver_info: info.driver_info,
    }
}

/// Enumerates all `wgpu` adapters visible to the current process/session (`JOB-W0`).
pub fn probe_gpu() -> Result<GpuProbeReport, BakeError> {
    let instance = wgpu::Instance::default();
    let enumerated = block_on(instance.enumerate_adapters(wgpu::Backends::all()));
    let adapters: Vec<GpuAdapterSummary> = enumerated
        .into_iter()
        .map(|a| summarize_adapter(a.get_info()))
        .collect();
    let selected = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
        ..wgpu::RequestAdapterOptions::default()
    }))
    .ok()
    .map(|a| summarize_adapter(a.get_info()));

    if adapters.is_empty() && selected.is_none() {
        return Err(BakeError::new(
            "probe-gpu found zero wgpu adapters (CPU fallback only)",
        ));
    }
    Ok(GpuProbeReport {
        adapter_count: adapters.len(),
        selected,
        adapters,
    })
}

/// Evaluates GPU compute Stewart/horizon lighting across a rectangular grid or tile region (`JOB-W4 --backend wgpu`).
#[allow(clippy::too_many_arguments)]
pub fn bake_lighting_wgpu_region(
    total_h_m: &[f32],
    albedo_luma: &[f32],
    macro_albedo: Option<&[[u8; 3]]>,
    width: usize,
    height: usize,
    dx_m: f32,
    horizon_radius: usize,
    region: Option<(usize, usize, usize, usize)>,
    samples: u32,
) -> Result<LightingBakeOutput, BakeError> {
    if samples == 0 {
        return Err(BakeError::new("--samples must be >= 1"));
    }
    let n = width * height;
    if total_h_m.len() != n || albedo_luma.len() != n {
        return Err(BakeError::new(
            "GPU lighting input length does not match width*height",
        ));
    }
    let (rx0, ry0, rw, rh) = region.unwrap_or((0, 0, width, height));
    if rx0 + rw > width || ry0 + rh > height || rw == 0 || rh == 0 {
        return Err(BakeError::new("GPU lighting region out of bounds"));
    }

    let instance = wgpu::Instance::default();
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
        ..wgpu::RequestAdapterOptions::default()
    }))
    .map_err(|e| BakeError::new(format!("wgpu request_adapter failed: {e}")))?;

    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("anima-bake-lighting"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        ..wgpu::DeviceDescriptor::default()
    }))
    .map_err(|e| BakeError::new(format!("wgpu request_device failed: {e}")))?;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("horizon-ao-shader"),
        source: wgpu::ShaderSource::Wgsl(HORIZON_SHADER_WGSL.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("horizon-ao-pipeline"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    let bgl = pipeline.get_bind_group_layout(0);

    let mut terrain_lighting = vec![[0_u8; 4]; rw * rh];
    let mut terrain_horizon = vec![[0_u8; 4]; rw * rh];

    // Chunk dispatch in <= 512x512 sub-blocks (e.g. 256x256 tile chunks) so VRAM stays < 16 MiB even at 8192^2!
    let chunk = 512_usize
        .min(rw)
        .min(rh)
        .max(TILE_ACTIVE_SIZE.min(rw).min(rh));
    let mut cy = 0_usize;
    while cy < rh {
        let ch = chunk.min(rh - cy);
        let mut cx = 0_usize;
        while cx < rw {
            let cw = chunk.min(rw - cx);
            let tile_x0 = rx0 + cx;
            let tile_y0 = ry0 + cy;
            let patch_x0 = tile_x0.saturating_sub(horizon_radius);
            let patch_y0 = tile_y0.saturating_sub(horizon_radius);
            let patch_x1 = (tile_x0 + cw + horizon_radius).min(width);
            let patch_y1 = (tile_y0 + ch + horizon_radius).min(height);
            let patch_w = patch_x1 - patch_x0;
            let patch_h = patch_y1 - patch_y0;

            let mut patch_h_buf = Vec::with_capacity(patch_w * patch_h);
            let mut patch_l_buf = Vec::with_capacity(patch_w * patch_h);
            for gr in patch_y0..patch_y1 {
                let row_s = gr * width + patch_x0;
                let row_e = row_s + patch_w;
                patch_h_buf.extend_from_slice(&total_h_m[row_s..row_e]);
                patch_l_buf.extend_from_slice(&albedo_luma[row_s..row_e]);
            }

            let params = GpuHorizonParams {
                full_w: width as i32,
                full_h: height as i32,
                patch_x0: patch_x0 as i32,
                patch_y0: patch_y0 as i32,
                patch_w: patch_w as i32,
                patch_h: patch_h as i32,
                tile_x0: tile_x0 as i32,
                tile_y0: tile_y0 as i32,
                out_w: cw as u32,
                out_h: ch as u32,
                radius: horizon_radius as i32,
                _pad: 0,
                dx_m,
                inv_half_pi: 1.0 / std::f32::consts::FRAC_PI_2,
                _pad2: 0.0,
                _pad3: 0.0,
            };

            let out_bytes_len = (cw * ch * 4) as u64;
            let ubo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let h_ssbo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("patch_h"),
                contents: bytemuck::cast_slice(&patch_h_buf),
                usage: wgpu::BufferUsages::STORAGE,
            });
            let l_ssbo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("patch_luma"),
                contents: bytemuck::cast_slice(&patch_l_buf),
                usage: wgpu::BufferUsages::STORAGE,
            });
            let out_l_ssbo = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("out_lighting"),
                size: out_bytes_len,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let out_h_ssbo = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("out_horizon"),
                size: out_bytes_len,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let stage_l = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("stage_l"),
                size: out_bytes_len,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let stage_h = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("stage_h"),
                size: out_bytes_len,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("horizon-bg"),
                layout: &bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: ubo.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: h_ssbo.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: l_ssbo.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: out_l_ssbo.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: out_h_ssbo.as_entire_binding(),
                    },
                ],
            });

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("horizon-enc"),
            });
            {
                let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
                cpass.set_pipeline(&pipeline);
                cpass.set_bind_group(0, &bg, &[]);
                cpass.dispatch_workgroups((cw as u32).div_ceil(8), (ch as u32).div_ceil(8), 1);
            }
            encoder.copy_buffer_to_buffer(&out_l_ssbo, 0, &stage_l, 0, out_bytes_len);
            encoder.copy_buffer_to_buffer(&out_h_ssbo, 0, &stage_h, 0, out_bytes_len);
            queue.submit(Some(encoder.finish()));

            let (tx_l, rx_l) = std::sync::mpsc::channel();
            let (tx_h, rx_h) = std::sync::mpsc::channel();
            stage_l.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx_l.send(r);
            });
            stage_h.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx_h.send(r);
            });
            device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| BakeError::new(format!("wgpu device poll failed: {e}")))?;
            rx_l.recv()
                .map_err(|e| BakeError::new(format!("stage_l recv failed: {e}")))?
                .map_err(|e| BakeError::new(format!("stage_l map failed: {e}")))?;
            rx_h.recv()
                .map_err(|e| BakeError::new(format!("stage_h recv failed: {e}")))?
                .map_err(|e| BakeError::new(format!("stage_h map failed: {e}")))?;

            {
                let view_l = stage_l
                    .slice(..)
                    .get_mapped_range()
                    .map_err(|e| BakeError::new(format!("stage_l get_mapped_range failed: {e}")))?;
                let view_h = stage_h
                    .slice(..)
                    .get_mapped_range()
                    .map_err(|e| BakeError::new(format!("stage_h get_mapped_range failed: {e}")))?;
                let (l_chunks, _) = view_l.as_chunks::<4>();
                let (h_chunks, _) = view_h.as_chunks::<4>();
                for lr in 0..ch {
                    let dst_off = (cy + lr) * rw + cx;
                    let src_off = lr * cw;
                    terrain_lighting[dst_off..(dst_off + cw)]
                        .copy_from_slice(&l_chunks[src_off..(src_off + cw)]);
                    terrain_horizon[dst_off..(dst_off + cw)]
                        .copy_from_slice(&h_chunks[src_off..(src_off + cw)]);
                }
            }
            stage_l.unmap();
            stage_h.unmap();

            cx += chunk;
        }
        cy += chunk;
    }

    let sky_bounce_probe = if rx0 == 0 && ry0 == 0 && rw == width && rh == height {
        if let Some(alb) = macro_albedo {
            compute_sky_bounce_probe_512(&terrain_lighting, alb, width, height)
        } else {
            let gray: Vec<[u8; 3]> = albedo_luma
                .iter()
                .map(|&l| {
                    let v = quantize_u8(l);
                    [v, v, v]
                })
                .collect();
            compute_sky_bounce_probe_512(&terrain_lighting, &gray, width, height)
        }
    } else {
        Vec::new()
    };

    Ok(LightingBakeOutput {
        width,
        height,
        tile_origin_x: rx0 / TILE_ACTIVE_SIZE,
        tile_origin_y: ry0 / TILE_ACTIVE_SIZE,
        out_width: rw,
        out_height: rh,
        terrain_lighting,
        terrain_horizon,
        sky_bounce_probe,
    })
}

/// Runs `JOB-W4 --backend wgpu` from an `anima-worldgen generate` directory.
pub fn bake_lighting_wgpu_from_raw_dir(
    raw_dir: &Path,
    out_dir: &Path,
    tile_range: Option<TileRange>,
    samples: u32,
) -> Result<LightingBakeReport, BakeError> {
    let (meta, total_h_m, albedo_luma, macro_albedo) = load_lighting_inputs_from_raw(raw_dir)?;
    let w = meta.width as usize;
    let h = meta.height as usize;
    fs::create_dir_all(out_dir)?;

    if let Some(range) = tile_range {
        if !w.is_multiple_of(TILE_ACTIVE_SIZE) || !h.is_multiple_of(TILE_ACTIVE_SIZE) {
            return Err(BakeError::new(format!(
                "tile-range GPU lighting requires grid divisible by {TILE_ACTIVE_SIZE}"
            )));
        }
        let tx_cnt = w / TILE_ACTIVE_SIZE;
        let ty_cnt = h / TILE_ACTIVE_SIZE;
        let total_tiles = tx_cnt * ty_cnt;
        if range.end > total_tiles {
            return Err(BakeError::new(format!(
                "--tile-range end {} exceeds total tile count {total_tiles}",
                range.end
            )));
        }
        let tiles_dir = out_dir.join("tiles");
        fs::create_dir_all(&tiles_dir)?;
        let mut last_l = String::new();
        let mut last_h = String::new();
        for tid in range.start..range.end {
            let tx = tid % tx_cnt;
            let ty = tid / tx_cnt;
            let baked = bake_lighting_wgpu_region(
                &total_h_m,
                &albedo_luma,
                Some(&macro_albedo),
                w,
                h,
                meta.dx_m,
                HORIZON_HALO_CELLS,
                Some((
                    tx * TILE_ACTIVE_SIZE,
                    ty * TILE_ACTIVE_SIZE,
                    TILE_ACTIVE_SIZE,
                    TILE_ACTIVE_SIZE,
                )),
                samples,
            )?;
            last_l = write_u8x4_slice(
                &tiles_dir.join(format!("tile_{tid}_lighting.bin")),
                &baked.terrain_lighting,
            )?;
            last_h = write_u8x4_slice(
                &tiles_dir.join(format!("tile_{tid}_horizon.bin")),
                &baked.terrain_horizon,
            )?;
        }
        let report = LightingBakeReport {
            backend: "wgpu".to_owned(),
            width: meta.width,
            height: meta.height,
            dx_m: meta.dx_m,
            horizon_radius_cells: HORIZON_HALO_CELLS,
            tile_range_start: Some(range.start),
            tile_range_end: Some(range.end),
            lighting_sha256: last_l,
            horizon_sha256: last_h,
            probe_sha256: None,
        };
        fs::write(
            out_dir.join(format!(
                "lighting_meta_wgpu_{}_{}.json",
                range.start, range.end
            )),
            format!("{}\n", serde_json::to_string_pretty(&report)?),
        )?;
        Ok(report)
    } else {
        let baked = bake_lighting_wgpu_region(
            &total_h_m,
            &albedo_luma,
            Some(&macro_albedo),
            w,
            h,
            meta.dx_m,
            HORIZON_HALO_CELLS,
            None,
            samples,
        )?;
        let lighting_sha256 = write_u8x4_slice(
            &out_dir.join("terrain_lighting.bin"),
            &baked.terrain_lighting,
        )?;
        let horizon_sha256 =
            write_u8x4_slice(&out_dir.join("terrain_horizon.bin"), &baked.terrain_horizon)?;
        let probe_sha = write_u8x4_slice(
            &out_dir.join("sky_bounce_probe.bin"),
            &baked.sky_bounce_probe,
        )?;
        let report = LightingBakeReport {
            backend: "wgpu".to_owned(),
            width: meta.width,
            height: meta.height,
            dx_m: meta.dx_m,
            horizon_radius_cells: HORIZON_HALO_CELLS,
            tile_range_start: None,
            tile_range_end: None,
            lighting_sha256,
            horizon_sha256,
            probe_sha256: Some(probe_sha),
        };
        fs::write(
            out_dir.join("lighting_meta.json"),
            format!("{}\n", serde_json::to_string_pretty(&report)?),
        )?;
        Ok(report)
    }
}

/// Compares CPU and GPU `[u8; 4]` outputs directly (`max_abs_delta`, `rmse`, `ssim`).
#[must_use]
pub fn compare_lighting_parity(
    cpu: &[[u8; 4]],
    gpu: &[[u8; 4]],
    width: usize,
    height: usize,
) -> LightingParityReport {
    assert_eq!(cpu.len(), width * height);
    assert_eq!(gpu.len(), width * height);
    let mut max_delta = 0_u8;
    let mut sq_sum = 0.0_f64;
    let mut flat_a = Vec::with_capacity(cpu.len() * 4);
    let mut flat_b = Vec::with_capacity(gpu.len() * 4);
    for (&a, &b) in cpu.iter().zip(gpu.iter()) {
        for ch in 0..4 {
            let d = a[ch].abs_diff(b[ch]);
            if d > max_delta {
                max_delta = d;
            }
            sq_sum += f64::from(d) * f64::from(d);
            flat_a.push(a[ch]);
            flat_b.push(b[ch]);
        }
    }
    let rmse = (sq_sum / ((cpu.len() * 4).max(1) as f64)).sqrt();
    let ssim = compute_ssim_u8(&flat_a, &flat_b, width, height, 4);
    LightingParityReport {
        max_abs_delta: max_delta,
        rmse,
        ssim,
    }
}
