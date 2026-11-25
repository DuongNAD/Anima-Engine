#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

//! `JOB-W5` Pure-Rust BC4/BC5 encoder, KTX2 v2.0 container codec, quality metrics (`Block PSNR`,
//! `SSIM`), tile pyramid packager, deterministic `.tar.gz` archive builder (strictly excluding
//! `index.json` / `sim_256.anmw3` to uphold `LD-2`), & strict archive verifier.

use crate::anmw_v3::{
    compute_bundle_identity, compute_sim_world_identity, sha256_to_hex, AnmwV3Sim,
};
use crate::bake::flora_place::{FloraSectorBlob, FLORA_SECTOR_SIZE};
use crate::bake::lighting_cpu::read_u8x4_bin;
use crate::bake::{
    compute_macro_albedo_rgb, compute_mip_pyramid, downsample_f32_2x2, downsample_u8_max_2x2,
    downsample_u8x2_mean_2x2, downsample_u8x4_mean_2x2, extract_tile_padded, quantize_u8,
    BakeError, MipLevelSpec, TILE_ACTIVE_SIZE, TILE_GUTTER, TILE_TOTAL_SIZE,
};
use crate::caves::CaveNetwork;
use crate::pipeline::{read_bin_f32, read_bin_u8, read_bin_u8x4, RawWorldMeta};
use crate::verify::WorldV2IndexJson;
use crate::{normalize_elevation, WaterKind};
use flate2::read::GzDecoder;
use flate2::write::{GzEncoder, ZlibEncoder};
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::Path;
use tar::{Archive, Builder, Header};

/// 12-byte KTX2 v2.0 file identifier.
pub const KTX2_MAGIC: [u8; 12] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];
/// KTX2 fixed header byte length (`80`).
pub const KTX2_HEADER_LEN: usize = 80;
/// KTX2 level-index record byte length (`24`).
pub const KTX2_LEVEL_LEN: usize = 24;

pub const VK_FORMAT_R8_UNORM: u32 = 9;
pub const VK_FORMAT_R8G8_UNORM: u32 = 16;
pub const VK_FORMAT_R8G8B8A8_UNORM: u32 = 37;
pub const VK_FORMAT_R8G8B8A8_SRGB: u32 = 43;
pub const VK_FORMAT_R16_UNORM: u32 = 70;
pub const VK_FORMAT_R16G16_UNORM: u32 = 77;
pub const VK_FORMAT_BC5_UNORM_BLOCK: u32 = 141;

pub const KTX2_SUPERCOMPRESSION_NONE: u32 = 0;
pub const KTX2_SUPERCOMPRESSION_ZLIB: u32 = 3;

const R16_MAX_F32: f32 = 65_535.0;
const DFD_COLOR_MODEL_RGBSDA: u8 = 1;
const DFD_COLOR_MODEL_BC5: u8 = 132;

/// Input specification for encoding one 2-D KTX2 v2.0 container.
#[derive(Debug, Clone)]
pub struct Ktx2WriteSpec {
    pub vk_format: u32,
    pub type_size: u32,
    pub width: u32,
    pub height: u32,
    pub supercompression_scheme: u32,
    /// Ordered `mip 0` (finest) through `mip L-1` (coarsest).
    pub mips: Vec<Vec<u8>>,
    /// Sorted key-value metadata embedded into KVD (in addition to `KTXwriter`).
    pub extra_kvd: BTreeMap<String, Vec<u8>>,
}

/// Parsed & validated KTX2 v2.0 header and level table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ktx2Parsed {
    pub vk_format: u32,
    pub type_size: u32,
    pub width: u32,
    pub height: u32,
    pub level_count: u32,
    pub supercompression_scheme: u32,
    pub kvd: BTreeMap<String, Vec<u8>>,
    pub levels: Vec<Ktx2LevelEntry>,
}

/// One mip level record inside a KTX2 file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ktx2LevelEntry {
    pub byte_offset: u64,
    pub byte_length: u64,
    pub uncompressed_byte_length: u64,
}

/// Result returned by [`pack_world_v2_tiles`] (`JOB-W5`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TilePackSummary {
    pub width: u32,
    pub height: u32,
    pub mip_levels: u32,
    pub release_tag: String,
    pub archive_filename: String,
    pub archive_byte_len: u64,
    pub release_archive_sha256: String,
    pub index_json_byte_len: usize,
    pub bundle_identity: String,
    pub sim_world_identity: String,
    pub entry_count: usize,
    pub tile_count: usize,
    pub normal_xy_bc5_block_psnr_db: f64,
    pub normal_xy_bc5_ssim: f64,
    pub by_layer_counts: BTreeMap<String, usize>,
    pub by_layer_bytes: BTreeMap<String, u64>,
}

/// Result returned by [`verify_tile_pack`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TilePackVerifyReport {
    pub passed: bool,
    pub bundle_identity: String,
    pub sim_world_identity: String,
    pub release_archive_sha256: String,
    pub archive_entries_verified: usize,
    pub ktx2_headers_verified: usize,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    sha256_to_hex(&h.finalize().into())
}

#[inline]
fn bc4_palette(a0: u8, a1: u8) -> [u8; 8] {
    let mut p = [0_u8; 8];
    p[0] = a0;
    p[1] = a1;
    let x0 = u16::from(a0);
    let x1 = u16::from(a1);
    if a0 > a1 {
        for (c, slot) in p.iter_mut().enumerate().skip(2) {
            let k = c as u16;
            *slot = (((8 - k) * x0 + (k - 1) * x1 + 3) / 7) as u8;
        }
    } else {
        for (c, slot) in p.iter_mut().enumerate().skip(2).take(4) {
            let k = c as u16;
            *slot = (((6 - k) * x0 + (k - 1) * x1 + 2) / 5) as u8;
        }
        p[6] = 0;
        p[7] = 255;
    }
    p
}

/// Encodes a single `4 × 4` `u8` scalar texel block into an `8`-byte BC4 block.
#[must_use]
pub fn encode_bc4_block(texels: &[u8; 16]) -> [u8; 8] {
    let mut min_v = 255_u8;
    let mut max_v = 0_u8;
    for &t in texels {
        min_v = min_v.min(t);
        max_v = max_v.max(t);
    }
    if min_v == max_v {
        return [max_v, min_v, 0, 0, 0, 0, 0, 0];
    }

    let palette = bc4_palette(max_v, min_v);
    let mut packed_bits = 0_u64;
    for (i, &v) in texels.iter().enumerate() {
        let mut best_code = 0_u64;
        let mut best_err = i32::MAX;
        for (code, &cand) in palette.iter().enumerate() {
            let err = (i32::from(v) - i32::from(cand)).abs();
            if err < best_err {
                best_err = err;
                best_code = code as u64;
                if err == 0 {
                    break;
                }
            }
        }
        packed_bits |= best_code << (3 * i);
    }

    let le = packed_bits.to_le_bytes();
    [max_v, min_v, le[0], le[1], le[2], le[3], le[4], le[5]]
}

/// Decodes a single `8`-byte BC4 block into `16` `u8` row-major texels.
#[must_use]
pub fn decode_bc4_block(block: &[u8; 8]) -> [u8; 16] {
    let palette = bc4_palette(block[0], block[1]);
    let bits = u64::from_le_bytes([
        block[2], block[3], block[4], block[5], block[6], block[7], 0, 0,
    ]);
    let mut out = [0_u8; 16];
    for (i, dst) in out.iter_mut().enumerate() {
        let code = ((bits >> (3 * i)) & 0x7) as usize;
        *dst = palette[code];
    }
    out
}

/// Encodes a `[u8; 2]` grid (`width` and `height` divisible by `4`) into packed BC5 UNORM blocks.
pub fn encode_bc5_unorm(rg: &[[u8; 2]], width: usize, height: usize) -> Result<Vec<u8>, BakeError> {
    if !width.is_multiple_of(4) || !height.is_multiple_of(4) || rg.len() != width * height {
        return Err(BakeError::new(format!(
            "BC5 requires width/height multiples of 4 and matching len (got {width}x{}, len {})",
            height,
            rg.len()
        )));
    }
    let bx_cnt = width / 4;
    let by_cnt = height / 4;
    let mut out = Vec::with_capacity(bx_cnt * by_cnt * 16);
    for by in 0..by_cnt {
        for bx in 0..bx_cnt {
            let mut r_blk = [0_u8; 16];
            let mut g_blk = [0_u8; 16];
            for ly in 0..4 {
                let row = (by * 4 + ly) * width + bx * 4;
                for lx in 0..4 {
                    let px = rg[row + lx];
                    r_blk[ly * 4 + lx] = px[0];
                    g_blk[ly * 4 + lx] = px[1];
                }
            }
            out.extend_from_slice(&encode_bc4_block(&r_blk));
            out.extend_from_slice(&encode_bc4_block(&g_blk));
        }
    }
    Ok(out)
}

/// Decodes packed BC5 UNORM blocks back into row-major `[u8; 2]` texels.
pub fn decode_bc5_unorm(
    blocks: &[u8],
    width: usize,
    height: usize,
) -> Result<Vec<[u8; 2]>, BakeError> {
    if !width.is_multiple_of(4) || !height.is_multiple_of(4) {
        return Err(BakeError::new(
            "BC5 decode requires width and height divisible by 4",
        ));
    }
    let bx_cnt = width / 4;
    let by_cnt = height / 4;
    let expected = bx_cnt * by_cnt * 16;
    if blocks.len() != expected {
        return Err(BakeError::new(format!(
            "BC5 block byte length mismatch: expected {expected}, got {}",
            blocks.len()
        )));
    }
    let mut out = vec![[0_u8; 2]; width * height];
    let mut off = 0_usize;
    for by in 0..by_cnt {
        for bx in 0..bx_cnt {
            let r_raw: [u8; 8] = blocks[off..(off + 8)].try_into().unwrap();
            let g_raw: [u8; 8] = blocks[(off + 8)..(off + 16)].try_into().unwrap();
            off += 16;
            let r_dec = decode_bc4_block(&r_raw);
            let g_dec = decode_bc4_block(&g_raw);
            for ly in 0..4 {
                let row = (by * 4 + ly) * width + bx * 4;
                for lx in 0..4 {
                    out[row + lx] = [r_dec[ly * 4 + lx], g_dec[ly * 4 + lx]];
                }
            }
        }
    }
    Ok(out)
}

/// Computes `Block PSNR` (dB) averaged over `4 × 4` blocks across `channels` (`1..=4`).
#[must_use]
pub fn compute_block_psnr_db(
    orig: &[u8],
    recon: &[u8],
    width: usize,
    height: usize,
    channels: usize,
) -> f64 {
    assert_eq!(orig.len(), width * height * channels);
    assert_eq!(recon.len(), orig.len());
    let mut mse_sum = 0.0_f64;
    let mut blk_cnt = 0_usize;
    let by_cnt = (height / 4).max(1);
    let bx_cnt = (width / 4).max(1);
    for by in 0..by_cnt {
        let r0 = by * 4;
        let r1 = (r0 + 4).min(height);
        for bx in 0..bx_cnt {
            let c0 = bx * 4;
            let c1 = (c0 + 4).min(width);
            let mut sq = 0.0_f64;
            let mut n_samp = 0_usize;
            for r in r0..r1 {
                for c in c0..c1 {
                    let base = (r * width + c) * channels;
                    for ch in 0..channels {
                        let d = f64::from(orig[base + ch]) - f64::from(recon[base + ch]);
                        sq += d * d;
                        n_samp += 1;
                    }
                }
            }
            mse_sum += sq / (n_samp.max(1) as f64);
            blk_cnt += 1;
        }
    }
    let mse = mse_sum / (blk_cnt.max(1) as f64);
    if mse <= 1e-12 {
        99.99
    } else {
        10.0 * libm::log10((255.0_f64 * 255.0_f64) / mse)
    }
}

/// Computes Wang et al. (2004) windowed `8 × 8` Structural Similarity (`SSIM`) averaged across channels.
#[must_use]
pub fn compute_ssim_u8(
    orig: &[u8],
    recon: &[u8],
    width: usize,
    height: usize,
    channels: usize,
) -> f64 {
    assert_eq!(orig.len(), width * height * channels);
    assert_eq!(recon.len(), orig.len());
    const C1: f64 = (0.01 * 255.0) * (0.01 * 255.0);
    const C2: f64 = (0.03 * 255.0) * (0.03 * 255.0);

    let win = 8_usize.min(width).min(height).max(1);
    let stride = 4_usize;
    let mut ssim_sum = 0.0_f64;
    let mut total_terms = 0_usize;

    for ch in 0..channels {
        let mut r = 0_usize;
        while r + win <= height {
            let mut c = 0_usize;
            while c + win <= width {
                let mut mx = 0.0_f64;
                let mut my = 0.0_f64;
                let n = (win * win) as f64;
                for dr in 0..win {
                    for dc in 0..win {
                        let idx = ((r + dr) * width + (c + dc)) * channels + ch;
                        mx += f64::from(orig[idx]);
                        my += f64::from(recon[idx]);
                    }
                }
                mx /= n;
                my /= n;

                let mut vx = 0.0_f64;
                let mut vy = 0.0_f64;
                let mut cov = 0.0_f64;
                for dr in 0..win {
                    for dc in 0..win {
                        let idx = ((r + dr) * width + (c + dc)) * channels + ch;
                        let dx = f64::from(orig[idx]) - mx;
                        let dy = f64::from(recon[idx]) - my;
                        vx += dx * dx;
                        vy += dy * dy;
                        cov += dx * dy;
                    }
                }
                vx /= (n - 1.0).max(1.0);
                vy /= (n - 1.0).max(1.0);
                cov /= (n - 1.0).max(1.0);

                let num = (2.0 * mx * my + C1) * (2.0 * cov + C2);
                let den = (mx * mx + my * my + C1) * (vx + vy + C2);
                ssim_sum += num / den.max(1e-12);
                total_terms += 1;
                c += stride;
            }
            r += stride;
        }
    }

    if total_terms == 0 {
        1.0
    } else {
        ssim_sum / (total_terms as f64)
    }
}

fn build_dfd_bytes(vk_format: u32) -> Result<Vec<u8>, BakeError> {
    struct DfdSpec {
        model: u8,
        transfer: u8,
        bw: u8,
        bh: u8,
        plane0_bytes: u8,
        samples: &'static [(u16, u8, u8)], // (bit_offset, bit_len_minus_1, channel_type_and_flags)
    }

    let spec = match vk_format {
        VK_FORMAT_R8_UNORM => DfdSpec {
            model: DFD_COLOR_MODEL_RGBSDA,
            transfer: 1,
            bw: 0,
            bh: 0,
            plane0_bytes: 1,
            samples: &[(0, 7, 0x00)],
        },
        VK_FORMAT_R8G8_UNORM => DfdSpec {
            model: DFD_COLOR_MODEL_RGBSDA,
            transfer: 1,
            bw: 0,
            bh: 0,
            plane0_bytes: 2,
            samples: &[(0, 7, 0x00), (8, 7, 0x01)],
        },
        VK_FORMAT_R8G8B8A8_UNORM => DfdSpec {
            model: DFD_COLOR_MODEL_RGBSDA,
            transfer: 1,
            bw: 0,
            bh: 0,
            plane0_bytes: 4,
            samples: &[(0, 7, 0x00), (8, 7, 0x01), (16, 7, 0x02), (24, 7, 0x03)],
        },
        VK_FORMAT_R8G8B8A8_SRGB => DfdSpec {
            model: DFD_COLOR_MODEL_RGBSDA,
            transfer: 2,
            bw: 0,
            bh: 0,
            plane0_bytes: 4,
            samples: &[(0, 7, 0x00), (8, 7, 0x01), (16, 7, 0x02), (24, 7, 0x13)],
        },
        VK_FORMAT_R16_UNORM => DfdSpec {
            model: DFD_COLOR_MODEL_RGBSDA,
            transfer: 1,
            bw: 0,
            bh: 0,
            plane0_bytes: 2,
            samples: &[(0, 15, 0x00)],
        },
        VK_FORMAT_R16G16_UNORM => DfdSpec {
            model: DFD_COLOR_MODEL_RGBSDA,
            transfer: 1,
            bw: 0,
            bh: 0,
            plane0_bytes: 4,
            samples: &[(0, 15, 0x00), (16, 15, 0x01)],
        },
        VK_FORMAT_BC5_UNORM_BLOCK => DfdSpec {
            model: DFD_COLOR_MODEL_BC5,
            transfer: 1,
            bw: 3, // 4 - 1
            bh: 3, // 4 - 1
            plane0_bytes: 16,
            samples: &[(0, 63, 0x00), (64, 63, 0x01)],
        },
        other => {
            return Err(BakeError::new(format!(
                "unsupported vk_format {other} in KTX2 writer"
            )));
        }
    };

    let block_size = 24 + 16 * spec.samples.len();
    let total_size = 4 + block_size;
    let mut dfd = Vec::with_capacity(total_size);
    dfd.extend_from_slice(&(total_size as u32).to_le_bytes());

    let header_word0: u32 = 0; // vendorId=0 (17 bits), descriptorType=0 (15 bits)
    let header_word1: u32 = 2 | ((block_size as u32) << 16);
    dfd.extend_from_slice(&header_word0.to_le_bytes());
    dfd.extend_from_slice(&header_word1.to_le_bytes());
    dfd.push(spec.model);
    dfd.push(1); // BT.709 primaries
    dfd.push(spec.transfer);
    dfd.push(0); // flags
    dfd.extend_from_slice(&[spec.bw, spec.bh, 0, 0]);
    dfd.extend_from_slice(&[spec.plane0_bytes, 0, 0, 0, 0, 0, 0, 0]);

    for &(bit_offset, bit_len_m1, chan_id) in spec.samples {
        let w0 = u32::from(bit_offset) | (u32::from(bit_len_m1) << 16) | (u32::from(chan_id) << 24);
        dfd.extend_from_slice(&w0.to_le_bytes());
        dfd.extend_from_slice(&0_u32.to_le_bytes());
        dfd.extend_from_slice(&0_u32.to_le_bytes()); // lower = 0
        let upper = if bit_len_m1 == 63 {
            u32::MAX
        } else if bit_len_m1 == 15 {
            65_535_u32
        } else {
            255_u32
        };
        dfd.extend_from_slice(&upper.to_le_bytes());
    }
    Ok(dfd)
}

/// Writes one 2-D KTX2 v2.0 container matching Khronos header, levelIndex, Basic DFD, and KVD layout.
pub fn write_ktx2_2d(spec: &Ktx2WriteSpec) -> Result<Vec<u8>, BakeError> {
    if spec.width == 0 || spec.height == 0 || spec.mips.is_empty() {
        return Err(BakeError::new(
            "KTX2 spec must have non-zero dimensions and >= 1 mip level",
        ));
    }
    let dfd = build_dfd_bytes(spec.vk_format)?;

    // Build sorted KVD entries (`KTXwriter` + caller keys).
    let mut kv_map = spec.extra_kvd.clone();
    kv_map
        .entry("KTXwriter".to_owned())
        .or_insert_with(|| b"anima-worldgen/anima-bake\0".to_vec());
    let mut kvd = Vec::new();
    for (k, v) in kv_map {
        let entry_len = k.len() + 1 + v.len();
        kvd.extend_from_slice(&(entry_len as u32).to_le_bytes());
        kvd.extend_from_slice(k.as_bytes());
        kvd.push(0);
        kvd.extend_from_slice(&v);
        while !kvd.len().is_multiple_of(4) {
            kvd.push(0);
        }
    }

    // Compress each mip payload if ZLIB supercompression is selected.
    let level_count = spec.mips.len();
    let mut encoded_mips: Vec<(Vec<u8>, u64)> = Vec::with_capacity(level_count);
    for raw in &spec.mips {
        let unc_len = raw.len() as u64;
        let payload = match spec.supercompression_scheme {
            KTX2_SUPERCOMPRESSION_NONE => raw.clone(),
            KTX2_SUPERCOMPRESSION_ZLIB => {
                let mut enc = ZlibEncoder::new(Vec::new(), Compression::new(6));
                enc.write_all(raw)?;
                enc.finish()?
            }
            other => {
                return Err(BakeError::new(format!(
                    "unsupported KTX2 supercompression scheme {other}"
                )));
            }
        };
        encoded_mips.push((payload, unc_len));
    }

    let level_index_offset = KTX2_HEADER_LEN;
    let level_index_len = level_count * KTX2_LEVEL_LEN;
    let dfd_offset = level_index_offset + level_index_len;
    let dfd_len = dfd.len();
    let kvd_offset = dfd_offset + dfd_len;
    let kvd_len = kvd.len();

    let mut cur_offset = (kvd_offset + kvd_len).next_multiple_of(16);
    let mut level_entries = vec![
        Ktx2LevelEntry {
            byte_offset: 0,
            byte_length: 0,
            uncompressed_byte_length: 0,
        };
        level_count
    ];
    // KTX2 places smallest mip first in the file stream (`level_count - 1 ..= 0`).
    let mut mip_blobs_in_order: Vec<(usize, &[u8])> = Vec::with_capacity(level_count);
    for mip in (0..level_count).rev() {
        cur_offset = cur_offset.next_multiple_of(16);
        let (ref blob, unc_len) = encoded_mips[mip];
        level_entries[mip] = Ktx2LevelEntry {
            byte_offset: cur_offset as u64,
            byte_length: blob.len() as u64,
            uncompressed_byte_length: unc_len,
        };
        mip_blobs_in_order.push((cur_offset, blob.as_slice()));
        cur_offset += blob.len();
    }

    let mut out = Vec::with_capacity(cur_offset);
    out.extend_from_slice(&KTX2_MAGIC);
    out.extend_from_slice(&spec.vk_format.to_le_bytes());
    out.extend_from_slice(&spec.type_size.to_le_bytes());
    out.extend_from_slice(&spec.width.to_le_bytes());
    out.extend_from_slice(&spec.height.to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes()); // pixelDepth
    out.extend_from_slice(&0_u32.to_le_bytes()); // layerCount
    out.extend_from_slice(&1_u32.to_le_bytes()); // faceCount
    out.extend_from_slice(&(level_count as u32).to_le_bytes());
    out.extend_from_slice(&spec.supercompression_scheme.to_le_bytes());
    out.extend_from_slice(&(dfd_offset as u32).to_le_bytes());
    out.extend_from_slice(&(dfd_len as u32).to_le_bytes());
    out.extend_from_slice(&(if kvd_len > 0 { kvd_offset as u32 } else { 0 }).to_le_bytes());
    out.extend_from_slice(&(kvd_len as u32).to_le_bytes());
    out.extend_from_slice(&0_u64.to_le_bytes()); // sgdByteOffset
    out.extend_from_slice(&0_u64.to_le_bytes()); // sgdByteLength

    for entry in &level_entries {
        out.extend_from_slice(&entry.byte_offset.to_le_bytes());
        out.extend_from_slice(&entry.byte_length.to_le_bytes());
        out.extend_from_slice(&entry.uncompressed_byte_length.to_le_bytes());
    }
    out.extend_from_slice(&dfd);
    out.extend_from_slice(&kvd);

    for (target_off, blob) in mip_blobs_in_order {
        while out.len() < target_off {
            out.push(0);
        }
        out.extend_from_slice(blob);
    }

    Ok(out)
}

#[inline]
fn u32_le(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

#[inline]
fn u64_le(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes([
        b[off],
        b[off + 1],
        b[off + 2],
        b[off + 3],
        b[off + 4],
        b[off + 5],
        b[off + 6],
        b[off + 7],
    ])
}

/// Parses and validates a KTX2 v2.0 byte buffer (header, DFD, KVD, and every mip offset/length).
pub fn parse_ktx2(bytes: &[u8]) -> Result<Ktx2Parsed, BakeError> {
    if bytes.len() < KTX2_HEADER_LEN {
        return Err(BakeError::new("KTX2 file shorter than 80-byte header"));
    }
    if bytes[0..12] != KTX2_MAGIC {
        return Err(BakeError::new("invalid KTX2 magic identifier"));
    }
    let vk_format = u32_le(bytes, 12);
    let type_size = u32_le(bytes, 16);
    let width = u32_le(bytes, 20);
    let height = u32_le(bytes, 24);
    let pixel_depth = u32_le(bytes, 28);
    let face_count = u32_le(bytes, 36);
    let level_count = u32_le(bytes, 40);
    let supercompression_scheme = u32_le(bytes, 44);
    let dfd_off = u32_le(bytes, 48) as usize;
    let dfd_len = u32_le(bytes, 52) as usize;
    let kvd_off = u32_le(bytes, 56) as usize;
    let kvd_len = u32_le(bytes, 60) as usize;

    if width == 0 || height == 0 || pixel_depth != 0 || face_count != 1 || level_count == 0 {
        return Err(BakeError::new(format!(
            "invalid KTX2 metadata ({width}x{height} depth={pixel_depth} faces={face_count} mips={level_count})"
        )));
    }
    let idx_bytes = (level_count as usize)
        .checked_mul(KTX2_LEVEL_LEN)
        .ok_or_else(|| BakeError::new("KTX2 level_count overflow"))?;
    let idx_end = KTX2_HEADER_LEN
        .checked_add(idx_bytes)
        .ok_or_else(|| BakeError::new("KTX2 levelIndex overflow"))?;
    if bytes.len() < idx_end {
        return Err(BakeError::new("KTX2 truncated inside levelIndex"));
    }
    if dfd_len < 4 || dfd_off.checked_add(dfd_len).is_none_or(|e| e > bytes.len()) {
        return Err(BakeError::new("KTX2 DFD range out of bounds"));
    }
    let dfd_total = u32_le(bytes, dfd_off) as usize;
    if dfd_total != dfd_len {
        return Err(BakeError::new("KTX2 DFD totalSize header mismatch"));
    }

    let mut kvd = BTreeMap::new();
    if kvd_len > 0 {
        let kvd_end = kvd_off
            .checked_add(kvd_len)
            .ok_or_else(|| BakeError::new("KTX2 KVD overflow"))?;
        if kvd_end > bytes.len() {
            return Err(BakeError::new("KTX2 KVD out of bounds"));
        }
        let mut cursor = kvd_off;
        while cursor < kvd_end {
            if cursor + 4 > kvd_end {
                return Err(BakeError::new("KTX2 KVD entry header truncated"));
            }
            let entry_len = u32_le(bytes, cursor) as usize;
            cursor += 4;
            if cursor + entry_len > kvd_end {
                return Err(BakeError::new("KTX2 KVD entry payload truncated"));
            }
            let raw = &bytes[cursor..(cursor + entry_len)];
            let nul_pos = raw
                .iter()
                .position(|&c| c == 0)
                .ok_or_else(|| BakeError::new("KTX2 KVD key missing NUL terminator"))?;
            let key = String::from_utf8(raw[..nul_pos].to_vec())
                .map_err(|_| BakeError::new("KTX2 KVD key is not UTF-8"))?;
            let val = raw[(nul_pos + 1)..].to_vec();
            kvd.insert(key, val);
            cursor = (cursor + entry_len).next_multiple_of(4);
        }
    }

    let mut levels = Vec::with_capacity(level_count as usize);
    for m in 0..(level_count as usize) {
        let off = KTX2_HEADER_LEN + m * KTX2_LEVEL_LEN;
        let byte_offset = u64_le(bytes, off);
        let byte_length = u64_le(bytes, off + 8);
        let uncompressed_byte_length = u64_le(bytes, off + 16);
        let end = byte_offset
            .checked_add(byte_length)
            .ok_or_else(|| BakeError::new("KTX2 mip offset overflow"))?;
        if end > bytes.len() as u64 || byte_length == 0 || uncompressed_byte_length == 0 {
            return Err(BakeError::new(format!("KTX2 mip {m} byte range invalid")));
        }
        levels.push(Ktx2LevelEntry {
            byte_offset,
            byte_length,
            uncompressed_byte_length,
        });
    }

    Ok(Ktx2Parsed {
        vk_format,
        type_size,
        width,
        height,
        level_count,
        supercompression_scheme,
        kvd,
        levels,
    })
}

/// Decompresses mip `level` payload bytes from a KTX2 container and checks exact `uncompressed_byte_length`.
pub fn decode_ktx2_level(bytes: &[u8], level: usize) -> Result<(Ktx2Parsed, Vec<u8>), BakeError> {
    let parsed = parse_ktx2(bytes)?;
    let entry = *parsed
        .levels
        .get(level)
        .ok_or_else(|| BakeError::new(format!("KTX2 mip level {level} missing")))?;
    let start = entry.byte_offset as usize;
    let end = start + (entry.byte_length as usize);
    let raw = &bytes[start..end];
    let out = match parsed.supercompression_scheme {
        KTX2_SUPERCOMPRESSION_NONE => raw.to_vec(),
        KTX2_SUPERCOMPRESSION_ZLIB => {
            use flate2::read::ZlibDecoder;
            let mut dec = ZlibDecoder::new(raw);
            let mut buf = Vec::with_capacity(entry.uncompressed_byte_length as usize);
            dec.read_to_end(&mut buf)?;
            buf
        }
        other => {
            return Err(BakeError::new(format!(
                "unsupported KTX2 supercompression scheme {other}"
            )));
        }
    };
    if out.len() as u64 != entry.uncompressed_byte_length {
        return Err(BakeError::new(format!(
            "KTX2 mip {level} decompressed len mismatch: expected {}, got {}",
            entry.uncompressed_byte_length,
            out.len()
        )));
    }
    Ok((parsed, out))
}

fn encode_tile_r16_unorm(elev_m: &[f32], include_border_kvd: bool) -> Result<Vec<u8>, BakeError> {
    let mut payload = Vec::with_capacity(elev_m.len() * 2);
    for &h in elev_m {
        let q = (normalize_elevation(h) * R16_MAX_F32)
            .round()
            .clamp(0.0, R16_MAX_F32) as u16;
        payload.extend_from_slice(&q.to_le_bytes());
    }
    let mut extra_kvd = BTreeMap::new();
    if include_border_kvd {
        // Store top/bottom/left/right exact `f32` border samples for seamless chunk stitching.
        let side = TILE_TOTAL_SIZE;
        let mut border_f32 = Vec::with_capacity(side * 4 * 4);
        for &v in elev_m.iter().take(side) {
            border_f32.extend_from_slice(&v.to_le_bytes());
        }
        for &v in elev_m.iter().skip((side - 1) * side).take(side) {
            border_f32.extend_from_slice(&v.to_le_bytes());
        }
        for r in 0..side {
            border_f32.extend_from_slice(&elev_m[r * side].to_le_bytes());
        }
        for r in 0..side {
            border_f32.extend_from_slice(&elev_m[r * side + (side - 1)].to_le_bytes());
        }
        extra_kvd.insert("animaBorderF32".to_owned(), border_f32);
    }
    write_ktx2_2d(&Ktx2WriteSpec {
        vk_format: VK_FORMAT_R16_UNORM,
        type_size: 2,
        width: TILE_TOTAL_SIZE as u32,
        height: TILE_TOTAL_SIZE as u32,
        supercompression_scheme: KTX2_SUPERCOMPRESSION_ZLIB,
        mips: vec![payload],
        extra_kvd,
    })
}

fn encode_tile_u8x4(data: &[[u8; 4]], vk_format: u32) -> Result<Vec<u8>, BakeError> {
    let mut payload = Vec::with_capacity(data.len() * 4);
    for &px in data {
        payload.extend_from_slice(&px);
    }
    write_ktx2_2d(&Ktx2WriteSpec {
        vk_format,
        type_size: 1,
        width: TILE_TOTAL_SIZE as u32,
        height: TILE_TOTAL_SIZE as u32,
        supercompression_scheme: KTX2_SUPERCOMPRESSION_ZLIB,
        mips: vec![payload],
        extra_kvd: BTreeMap::new(),
    })
}

fn encode_tile_u8x2(data: &[[u8; 2]]) -> Result<Vec<u8>, BakeError> {
    let mut payload = Vec::with_capacity(data.len() * 2);
    for &px in data {
        payload.extend_from_slice(&px);
    }
    write_ktx2_2d(&Ktx2WriteSpec {
        vk_format: VK_FORMAT_R8G8_UNORM,
        type_size: 1,
        width: TILE_TOTAL_SIZE as u32,
        height: TILE_TOTAL_SIZE as u32,
        supercompression_scheme: KTX2_SUPERCOMPRESSION_ZLIB,
        mips: vec![payload],
        extra_kvd: BTreeMap::new(),
    })
}

fn encode_tile_u8(data: &[u8]) -> Result<Vec<u8>, BakeError> {
    write_ktx2_2d(&Ktx2WriteSpec {
        vk_format: VK_FORMAT_R8_UNORM,
        type_size: 1,
        width: TILE_TOTAL_SIZE as u32,
        height: TILE_TOTAL_SIZE as u32,
        supercompression_scheme: KTX2_SUPERCOMPRESSION_ZLIB,
        mips: vec![data.to_vec()],
        extra_kvd: BTreeMap::new(),
    })
}

fn encode_tile_water_r16g16(data: &[(f32, u8)]) -> Result<Vec<u8>, BakeError> {
    let mut payload = Vec::with_capacity(data.len() * 4);
    for &(wh_m, wk) in data {
        let h_u16 = (normalize_elevation(wh_m) * R16_MAX_F32)
            .round()
            .clamp(0.0, R16_MAX_F32) as u16;
        let kind_u16 = u16::from(wk);
        payload.extend_from_slice(&h_u16.to_le_bytes());
        payload.extend_from_slice(&kind_u16.to_le_bytes());
    }
    write_ktx2_2d(&Ktx2WriteSpec {
        vk_format: VK_FORMAT_R16G16_UNORM,
        type_size: 2,
        width: TILE_TOTAL_SIZE as u32,
        height: TILE_TOTAL_SIZE as u32,
        supercompression_scheme: KTX2_SUPERCOMPRESSION_ZLIB,
        mips: vec![payload],
        extra_kvd: BTreeMap::new(),
    })
}

fn downsample_water_2x2(src_h: &[f32], src_k: &[u8], w: usize, h: usize) -> (Vec<f32>, Vec<u8>) {
    let nw = w / 2;
    let nh = h / 2;
    let mut out_h = Vec::with_capacity(nw * nh);
    let mut out_k = Vec::with_capacity(nw * nh);
    for r in 0..nh {
        let r0 = r * 2;
        let r1 = r0 + 1;
        for c in 0..nw {
            let c0 = c * 2;
            let c1 = c0 + 1;
            let ids = [r0 * w + c0, r0 * w + c1, r1 * w + c0, r1 * w + c1];
            let mut sum_w = 0.0_f32;
            let mut cnt_w = 0_u32;
            let mut best_k = 0_u8;
            for i in ids {
                if src_k[i] != WaterKind::None as u8 {
                    sum_w += src_h[i];
                    cnt_w += 1;
                    best_k = best_k.max(src_k[i]);
                }
            }
            if cnt_w > 0 {
                out_h.push(sum_w / (cnt_w as f32));
                out_k.push(best_k);
            } else {
                let dry_avg =
                    0.25 * (src_h[ids[0]] + src_h[ids[1]] + src_h[ids[2]] + src_h[ids[3]]);
                out_h.push(dry_avg);
                out_k.push(0);
            }
        }
    }
    (out_h, out_k)
}

fn downsample_biome_2x2(
    src_ids: &[[u8; 4]],
    src_w: &[[u8; 4]],
    w: usize,
    h: usize,
) -> (Vec<[u8; 4]>, Vec<[u8; 4]>) {
    let nw = w / 2;
    let nh = h / 2;
    let mut out_ids = Vec::with_capacity(nw * nh);
    let mut out_w = Vec::with_capacity(nw * nh);
    for r in 0..nh {
        let r0 = r * 2;
        let r1 = r0 + 1;
        for c in 0..nw {
            let c0 = c * 2;
            let c1 = c0 + 1;
            let idx = r0 * w + c0;
            // Deterministic representative dominant tuple + blended weight sanity normalization (`sum = 255`).
            let _ = (r1, c1);
            let mut w_cell = src_w[idx];
            let sum: u16 = w_cell.iter().map(|&v| u16::from(v)).sum();
            if sum == 0 {
                w_cell = [255, 0, 0, 0];
            }
            out_ids.push(src_ids[idx]);
            out_w.push(w_cell);
        }
    }
    (out_ids, out_w)
}

#[inline]
fn is_forbidden_archive_entry(path: &str) -> bool {
    let base = path.rsplit('/').next().unwrap_or(path);
    base.eq_ignore_ascii_case("index.json") || base.eq_ignore_ascii_case("sim_256.anmw3")
}

/// Packs `entries` (sorted deterministic paths -> bytes) into a reproducible `.tar.gz` archive,
/// **strictly rejecting** any entry whose filename is `index.json` or `sim_256.anmw3` (`LD-2` cycle guard).
pub fn pack_entries_to_archive(
    entries: &BTreeMap<String, Vec<u8>>,
    out_archive: &Path,
) -> Result<(u64, String), BakeError> {
    for path in entries.keys() {
        if is_forbidden_archive_entry(path) {
            return Err(BakeError::new(format!(
                "archive self-check failed: forbidden entry '{path}' must NEVER be inside the tile pack archive (LD-2)"
            )));
        }
    }
    if let Some(parent) = out_archive.parent() {
        fs::create_dir_all(parent)?;
    }

    let gz_file = BufWriter::new(File::create(out_archive)?);
    let gz_enc = GzEncoder::new(gz_file, Compression::new(6));
    let mut tar = Builder::new(gz_enc);

    for (path, data) in entries {
        let mut header = Header::new_ustar();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        tar.append_data(&mut header, path, data.as_slice())?;
    }
    let gz_enc = tar.into_inner()?;
    let mut inner_file = gz_enc.finish()?;
    inner_file.flush()?;

    let archive_bytes = fs::read(out_archive)?;
    Ok((archive_bytes.len() as u64, sha256_hex(&archive_bytes)))
}

/// Executes `JOB-W5` end-to-end: packs all `§3.2` baked layers into a KTX2/anmf `.tar.gz` archive
/// (strictly excluding `index.json`), emits canonical sorted-key `index.json` + `pack_summary.json`,
/// and preserves `sim_256.anmw3` (`sim_world_identity`) untouched.
pub fn pack_world_v2_tiles(
    bake_in_dir: &Path,
    raw_dir: &Path,
    sim_256_path: &Path,
    release_tag: &str,
    out_dir: &Path,
) -> Result<TilePackSummary, BakeError> {
    let meta: RawWorldMeta =
        serde_json::from_str(&fs::read_to_string(raw_dir.join("raw_meta.json"))?)?;
    let w = meta.width as usize;
    let h = meta.height as usize;
    if w != h || w < TILE_ACTIVE_SIZE || !w.is_power_of_two() {
        return Err(BakeError::new(format!(
            "pack requires square power-of-two grid >= {TILE_ACTIVE_SIZE}, got {}x{}",
            w, h
        )));
    }
    let n = w * h;
    let pyramid: Vec<MipLevelSpec> = compute_mip_pyramid(w)?;

    let sim_bytes = fs::read(sim_256_path)?;
    let _sim = AnmwV3Sim::from_bytes(&sim_bytes)?;
    let sim_world_identity = sha256_to_hex(&compute_sim_world_identity(&sim_bytes));

    // Load required baked & raw buffers cleanly.
    let total_h_m = read_bin_f32(&raw_dir.join("total_h_m.bin"), n)?;
    let water_h_m = read_bin_f32(&raw_dir.join("water_h_m.bin"), n)?;
    let water_kind = read_bin_u8(&raw_dir.join("water_kind.bin"), n)?;
    let biome_ids = read_bin_u8x4(&raw_dir.join("biome_ids.bin"), n)?;
    let biome_weights = read_bin_u8x4(&raw_dir.join("biome_weights.bin"), n)?;
    let dominant_biome = read_bin_u8(&raw_dir.join("dominant_biome.bin"), n)?;
    let lithology_id = read_bin_u8(&raw_dir.join("lithology_id.bin"), n)?;
    let regolith_m = read_bin_f32(&raw_dir.join("sediment_s_m.bin"), n)?;
    let talus_frac = read_bin_f32(&raw_dir.join("talus_frac.bin"), n)?;
    let moraine_debris = read_bin_u8(&raw_dir.join("moraine_debris.bin"), n)?;
    let glacier_h_m = read_bin_f32(&raw_dir.join("glacier_h_m.bin"), n)?;
    let temp_c = read_bin_f32(&raw_dir.join("temp_c.bin"), n)?;
    let moisture = read_bin_f32(&raw_dir.join("moisture.bin"), n)?;
    let precip_m_yr = read_bin_f32(&raw_dir.join("precip_m_yr.bin"), n)?;
    let cave_mask = read_bin_u8(&raw_dir.join("cave_mask.bin"), n)?;
    let cave_network_bytes = fs::read(raw_dir.join("cave_network.bin"))?;
    let _ = CaveNetwork::from_bytes(&cave_network_bytes)?;

    let normal_xy = read_bin_u8x2_exact(&bake_in_dir.join("normal_xy.bin"), n)?;
    let shore_sdf = read_bin_u8x2_exact(&bake_in_dir.join("shore_sdf.bin"), n)?;
    let hydro_flow = read_u8x4_bin(&bake_in_dir.join("hydro_flow.bin"), n)?;
    let terrain_lighting = read_u8x4_bin(&bake_in_dir.join("terrain_lighting.bin"), n)?;
    let terrain_horizon = read_u8x4_bin(&bake_in_dir.join("terrain_horizon.bin"), n)?;
    let probe_path = bake_in_dir.join("sky_bounce_probe.bin");
    let sky_bounce_probe = read_u8x4_bin(&probe_path, 512 * 512)?;

    // Build derived composite layers at mip 0.
    let mut lithology_u8x4 = Vec::with_capacity(n);
    let mut climate_u8x4 = Vec::with_capacity(n);
    let mut macro_albedo_u8x4 = Vec::with_capacity(n);
    for i in 0..n {
        let reg_u8 = quantize_u8((regolith_m[i] / 20.0).clamp(0.0, 1.0));
        let tal_u8 = quantize_u8(talus_frac[i].clamp(0.0, 1.0));
        let wet_mor = if moraine_debris[i] > 0 {
            255
        } else {
            quantize_u8(moisture[i].clamp(0.0, 1.0))
        };
        lithology_u8x4.push([lithology_id[i], reg_u8, tal_u8, wet_mor]);

        let t_norm = ((temp_c[i] + 20.0) / 60.0).clamp(0.0, 1.0);
        let p_norm = (precip_m_yr[i] / 5.0).clamp(0.0, 1.0);
        let snow_norm = (glacier_h_m[i] / 50.0).clamp(0.0, 1.0);
        climate_u8x4.push([
            quantize_u8(t_norm),
            quantize_u8(moisture[i].clamp(0.0, 1.0)),
            quantize_u8(p_norm),
            quantize_u8(snow_norm),
        ]);

        let rgb = compute_macro_albedo_rgb(
            dominant_biome[i],
            total_h_m[i],
            moisture[i],
            talus_frac[i],
            glacier_h_m[i],
        );
        macro_albedo_u8x4.push([rgb[0], rgb[1], rgb[2], 255]);
    }

    // Measure BC5 compression quality directly on base normal_xy (`§6.2` `Block PSNR >= 42 dB, SSIM >= 0.996`).
    let bc5_full = encode_bc5_unorm(&normal_xy, w, h)?;
    let normal_recon = decode_bc5_unorm(&bc5_full, w, h)?;
    let orig_flat: Vec<u8> = normal_xy.iter().flat_map(|p| p.iter().copied()).collect();
    let recon_flat: Vec<u8> = normal_recon
        .iter()
        .flat_map(|p| p.iter().copied())
        .collect();
    let bc5_psnr_db = compute_block_psnr_db(&orig_flat, &recon_flat, w, h, 2);
    let bc5_ssim = compute_ssim_u8(&orig_flat, &recon_flat, w, h, 2);
    drop(bc5_full);
    drop(normal_recon);
    drop(orig_flat);
    drop(recon_flat);

    let mut archive_entries: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut tile_manifest: Vec<serde_json::Value> = Vec::new();
    let mut by_layer_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_layer_bytes: BTreeMap<String, u64> = BTreeMap::new();

    let mut record_entry =
        |layer: &str, mip: u32, tx: usize, ty: usize, rel_path: String, bytes: Vec<u8>| {
            let digest = sha256_hex(&bytes);
            let byte_len = bytes.len();
            *by_layer_counts.entry(layer.to_owned()).or_insert(0) += 1;
            *by_layer_bytes.entry(layer.to_owned()).or_insert(0) += byte_len as u64;
            tile_manifest.push(serde_json::json!({
                "byte_len": byte_len,
                "layer": layer,
                "mip": mip,
                "path": rel_path,
                "sha256": digest,
                "tx": tx,
                "ty": ty,
            }));
            archive_entries.insert(rel_path, bytes);
        };

    // Pack pyramid levels iteratively downsampling `L0 -> L_max` to keep RAM bounded cleanly!
    let mut cur_h = total_h_m;
    let mut cur_norm = normal_xy;
    let mut cur_light = terrain_lighting;
    let mut cur_hor = terrain_horizon;
    let mut cur_flow = hydro_flow;
    let mut cur_water_h = water_h_m;
    let mut cur_water_k = water_kind;
    let mut cur_sdf = shore_sdf;
    let mut cur_bids = biome_ids;
    let mut cur_bwts = biome_weights;
    let mut cur_lith = lithology_u8x4;
    let mut cur_clim = climate_u8x4;
    let mut cur_macro = macro_albedo_u8x4;
    let mut cur_cave = cave_mask;

    // `climate` sits at half base res when base >= 512 (`2048^2` @ `4096^2`), `macro_albedo` at quarter res (`1024^2` @ `4096^2`, `§3.2` #9, #10).
    let climate_min_mip = if w >= 512 { 1_u32 } else { 0_u32 };
    let macro_min_mip = if w >= 1024 { 2_u32 } else { 0_u32 };

    for (idx_mip, spec) in pyramid.iter().enumerate() {
        let gw = spec.grid_size;
        let gh = spec.grid_size;
        let mip = spec.mip;

        for ty in 0..spec.tiles_y {
            for tx in 0..spec.tiles_x {
                let h_tile =
                    extract_tile_padded(&cur_h, gw, gh, tx, ty, TILE_ACTIVE_SIZE, TILE_GUTTER);
                record_entry(
                    "height",
                    mip,
                    tx,
                    ty,
                    format!("tiles/height/L{mip}/t_{tx}_{ty}.ktx2"),
                    encode_tile_r16_unorm(&h_tile, true)?,
                );

                let n_tile =
                    extract_tile_padded(&cur_norm, gw, gh, tx, ty, TILE_ACTIVE_SIZE, TILE_GUTTER);
                let bc5_tile = encode_bc5_unorm(&n_tile, TILE_TOTAL_SIZE, TILE_TOTAL_SIZE)?;
                record_entry(
                    "normal_xy",
                    mip,
                    tx,
                    ty,
                    format!("tiles/normal_xy/L{mip}/t_{tx}_{ty}.ktx2"),
                    write_ktx2_2d(&Ktx2WriteSpec {
                        vk_format: VK_FORMAT_BC5_UNORM_BLOCK,
                        type_size: 1,
                        width: TILE_TOTAL_SIZE as u32,
                        height: TILE_TOTAL_SIZE as u32,
                        supercompression_scheme: KTX2_SUPERCOMPRESSION_ZLIB,
                        mips: vec![bc5_tile],
                        extra_kvd: BTreeMap::new(),
                    })?,
                );

                let l_tile =
                    extract_tile_padded(&cur_light, gw, gh, tx, ty, TILE_ACTIVE_SIZE, TILE_GUTTER);
                record_entry(
                    "terrain_lighting",
                    mip,
                    tx,
                    ty,
                    format!("tiles/terrain_lighting/L{mip}/t_{tx}_{ty}.ktx2"),
                    encode_tile_u8x4(&l_tile, VK_FORMAT_R8G8B8A8_UNORM)?,
                );

                let hz_tile =
                    extract_tile_padded(&cur_hor, gw, gh, tx, ty, TILE_ACTIVE_SIZE, TILE_GUTTER);
                record_entry(
                    "terrain_horizon",
                    mip,
                    tx,
                    ty,
                    format!("tiles/terrain_horizon/L{mip}/t_{tx}_{ty}.ktx2"),
                    encode_tile_u8x4(&hz_tile, VK_FORMAT_R8G8B8A8_UNORM)?,
                );

                let fl_tile =
                    extract_tile_padded(&cur_flow, gw, gh, tx, ty, TILE_ACTIVE_SIZE, TILE_GUTTER);
                record_entry(
                    "hydro_flow",
                    mip,
                    tx,
                    ty,
                    format!("tiles/hydro_flow/L{mip}/t_{tx}_{ty}.ktx2"),
                    encode_tile_u8x4(&fl_tile, VK_FORMAT_R8G8B8A8_UNORM)?,
                );

                let wh_tile = extract_tile_padded(
                    &cur_water_h,
                    gw,
                    gh,
                    tx,
                    ty,
                    TILE_ACTIVE_SIZE,
                    TILE_GUTTER,
                );
                let wk_tile = extract_tile_padded(
                    &cur_water_k,
                    gw,
                    gh,
                    tx,
                    ty,
                    TILE_ACTIVE_SIZE,
                    TILE_GUTTER,
                );
                if wk_tile.iter().any(|&wk| wk != 0) {
                    let pairs: Vec<(f32, u8)> = wh_tile.into_iter().zip(wk_tile).collect();
                    record_entry(
                        "water_surface",
                        mip,
                        tx,
                        ty,
                        format!("tiles/water_surface/L{mip}/t_{tx}_{ty}.ktx2"),
                        encode_tile_water_r16g16(&pairs)?,
                    );
                }

                let sdf_tile =
                    extract_tile_padded(&cur_sdf, gw, gh, tx, ty, TILE_ACTIVE_SIZE, TILE_GUTTER);
                record_entry(
                    "shore_sdf",
                    mip,
                    tx,
                    ty,
                    format!("tiles/shore_sdf/L{mip}/t_{tx}_{ty}.ktx2"),
                    encode_tile_u8x2(&sdf_tile)?,
                );

                let bids_tile =
                    extract_tile_padded(&cur_bids, gw, gh, tx, ty, TILE_ACTIVE_SIZE, TILE_GUTTER);
                record_entry(
                    "biome_ids",
                    mip,
                    tx,
                    ty,
                    format!("tiles/biome_ids/L{mip}/t_{tx}_{ty}.ktx2"),
                    encode_tile_u8x4(&bids_tile, VK_FORMAT_R8G8B8A8_UNORM)?,
                );

                let bwts_tile =
                    extract_tile_padded(&cur_bwts, gw, gh, tx, ty, TILE_ACTIVE_SIZE, TILE_GUTTER);
                record_entry(
                    "biome_weights",
                    mip,
                    tx,
                    ty,
                    format!("tiles/biome_weights/L{mip}/t_{tx}_{ty}.ktx2"),
                    encode_tile_u8x4(&bwts_tile, VK_FORMAT_R8G8B8A8_UNORM)?,
                );

                let lith_tile =
                    extract_tile_padded(&cur_lith, gw, gh, tx, ty, TILE_ACTIVE_SIZE, TILE_GUTTER);
                record_entry(
                    "lithology",
                    mip,
                    tx,
                    ty,
                    format!("tiles/lithology/L{mip}/t_{tx}_{ty}.ktx2"),
                    encode_tile_u8x4(&lith_tile, VK_FORMAT_R8G8B8A8_UNORM)?,
                );

                if mip >= climate_min_mip {
                    let clim_tile = extract_tile_padded(
                        &cur_clim,
                        gw,
                        gh,
                        tx,
                        ty,
                        TILE_ACTIVE_SIZE,
                        TILE_GUTTER,
                    );
                    record_entry(
                        "climate",
                        mip,
                        tx,
                        ty,
                        format!("tiles/climate/L{mip}/t_{tx}_{ty}.ktx2"),
                        encode_tile_u8x4(&clim_tile, VK_FORMAT_R8G8B8A8_UNORM)?,
                    );
                }

                if mip >= macro_min_mip {
                    let mac_tile = extract_tile_padded(
                        &cur_macro,
                        gw,
                        gh,
                        tx,
                        ty,
                        TILE_ACTIVE_SIZE,
                        TILE_GUTTER,
                    );
                    record_entry(
                        "macro_albedo",
                        mip,
                        tx,
                        ty,
                        format!("tiles/macro_albedo/L{mip}/t_{tx}_{ty}.ktx2"),
                        encode_tile_u8x4(&mac_tile, VK_FORMAT_R8G8B8A8_SRGB)?,
                    );
                }

                let cave_tile =
                    extract_tile_padded(&cur_cave, gw, gh, tx, ty, TILE_ACTIVE_SIZE, TILE_GUTTER);
                if cave_tile.iter().any(|&v| v > 0) {
                    record_entry(
                        "cave_mask",
                        mip,
                        tx,
                        ty,
                        format!("tiles/cave_mask/L{mip}/t_{tx}_{ty}.ktx2"),
                        encode_tile_u8(&cave_tile)?,
                    );
                }
            }
        }

        if idx_mip + 1 < pyramid.len() {
            cur_h = downsample_f32_2x2(&cur_h, gw, gh);
            cur_norm = downsample_u8x2_mean_2x2(&cur_norm, gw, gh);
            cur_light = downsample_u8x4_mean_2x2(&cur_light, gw, gh);
            cur_hor = downsample_u8x4_mean_2x2(&cur_hor, gw, gh);
            cur_flow = downsample_u8x4_mean_2x2(&cur_flow, gw, gh);
            let (nw_h, nw_k) = downsample_water_2x2(&cur_water_h, &cur_water_k, gw, gh);
            cur_water_h = nw_h;
            cur_water_k = nw_k;
            cur_sdf = downsample_u8x2_mean_2x2(&cur_sdf, gw, gh);
            let (nb_i, nb_w) = downsample_biome_2x2(&cur_bids, &cur_bwts, gw, gh);
            cur_bids = nb_i;
            cur_bwts = nb_w;
            cur_lith = downsample_u8x4_mean_2x2(&cur_lith, gw, gh);
            cur_clim = downsample_u8x4_mean_2x2(&cur_clim, gw, gh);
            cur_macro = downsample_u8x4_mean_2x2(&cur_macro, gw, gh);
            cur_cave = downsample_u8_max_2x2(&cur_cave, gw, gh);
        }
    }

    // 512 x 512 full-mip-chain sky/bounce probe KTX2 (`512 -> 256 -> ... -> 1`).
    let mut probe_mips = Vec::new();
    let mut p_sz = 512_usize;
    let mut p_cur = sky_bounce_probe;
    loop {
        let mut raw = Vec::with_capacity(p_cur.len() * 4);
        for &px in &p_cur {
            raw.extend_from_slice(&px);
        }
        probe_mips.push(raw);
        if p_sz == 1 {
            break;
        }
        p_cur = downsample_u8x4_mean_2x2(&p_cur, p_sz, p_sz);
        p_sz /= 2;
    }
    record_entry(
        "sky_bounce_probe",
        0,
        0,
        0,
        "tiles/sky_bounce_probe.ktx2".to_owned(),
        write_ktx2_2d(&Ktx2WriteSpec {
            vk_format: VK_FORMAT_R8G8B8A8_UNORM,
            type_size: 1,
            width: 512,
            height: 512,
            supercompression_scheme: KTX2_SUPERCOMPRESSION_ZLIB,
            mips: probe_mips,
            extra_kvd: BTreeMap::new(),
        })?,
    );

    // Cave graph (`§3.2` #11)
    record_entry(
        "cave_network",
        0,
        0,
        0,
        "cave_network.bin".to_owned(),
        cave_network_bytes,
    );

    // Decorative `256^2` flora sector blobs (`§3.2` #12)
    let sx_cnt = w / FLORA_SECTOR_SIZE;
    let sz_cnt = h / FLORA_SECTOR_SIZE;
    let flora_dir = bake_in_dir.join("flora");
    for sz in 0..sz_cnt {
        for sx in 0..sx_cnt {
            let rel = format!("flora/s_{sx}_{sz}.anmf");
            let full = flora_dir.join(format!("s_{sx}_{sz}.anmf"));
            let bytes = fs::read(&full)?;
            let _ = FloraSectorBlob::from_bytes(&bytes)?;
            record_entry("flora_tiles", 0, sx, sz, rel, bytes);
        }
    }

    fs::create_dir_all(out_dir)?;
    let sim_dst = out_dir.join("sim_256.anmw3");
    if sim_256_path != sim_dst {
        fs::write(&sim_dst, &sim_bytes)?;
    }

    let id8 = &sim_world_identity[..8];
    let archive_filename = format!("world_v2_{id8}_tiles.tar.gz");
    let archive_path = out_dir.join(&archive_filename);
    let (archive_byte_len, release_archive_sha256) =
        pack_entries_to_archive(&archive_entries, &archive_path)?;

    let index = WorldV2IndexJson {
        extent_m: meta.extent_m.round() as u32,
        format_version: 1,
        generator_version: 3,
        grid_size: meta.width,
        release_archive: Some(format!("{release_tag}/{archive_filename}")),
        release_archive_sha256: Some(release_archive_sha256.clone()),
        schema_tag: meta.schema_tag,
        seed: meta.seed,
        sim_256_byte_len: sim_bytes.len(),
        sim_256_file: "sim_256.anmw3".to_owned(),
        sim_world_identity: sim_world_identity.clone(),
        tiles: tile_manifest,
    };
    let index_bytes = index.to_canonical_json_bytes()?;
    let bundle_identity = sha256_to_hex(&compute_bundle_identity(&index_bytes));
    fs::write(out_dir.join("index.json"), &index_bytes)?;

    let summary = TilePackSummary {
        width: meta.width,
        height: meta.height,
        mip_levels: pyramid.len() as u32,
        release_tag: release_tag.to_owned(),
        archive_filename,
        archive_byte_len,
        release_archive_sha256,
        index_json_byte_len: index_bytes.len(),
        bundle_identity,
        sim_world_identity,
        entry_count: archive_entries.len(),
        tile_count: archive_entries.len(),
        normal_xy_bc5_block_psnr_db: bc5_psnr_db,
        normal_xy_bc5_ssim: bc5_ssim,
        by_layer_counts,
        by_layer_bytes,
    };
    fs::write(
        out_dir.join("pack_summary.json"),
        format!("{}\n", serde_json::to_string_pretty(&summary)?),
    )?;
    Ok(summary)
}

/// Strictly verifies a packaged World v2 tile release against committed `index.json` & `sim_256.anmw3`:
/// checks canonical `index.json`, `sim_world_identity`, `release_archive_sha256`, absence of `index.json`
/// inside the archive (`LD-2`), per-entry byte length & SHA-256, KTX2 headers, `ANMC` cave graph, and `ANMF` flora.
pub fn verify_tile_pack(
    index_path: &Path,
    sim_path: &Path,
    archive_path: &Path,
) -> Result<TilePackVerifyReport, BakeError> {
    let index_bytes = fs::read(index_path)?;
    let bundle_identity = sha256_to_hex(&compute_bundle_identity(&index_bytes));
    let index: WorldV2IndexJson = serde_json::from_slice(&index_bytes)?;
    if index.to_canonical_json_bytes()? != index_bytes {
        return Err(BakeError::new(
            "index.json is not canonical LF sorted-key JSON",
        ));
    }

    let sim_bytes = fs::read(sim_path)?;
    let _ = AnmwV3Sim::from_bytes(&sim_bytes)?;
    let sim_world_identity = sha256_to_hex(&compute_sim_world_identity(&sim_bytes));
    if sim_world_identity != index.sim_world_identity {
        return Err(BakeError::new(format!(
            "sim_world_identity mismatch: index expects {}, disk sim_256 is {}",
            index.sim_world_identity, sim_world_identity
        )));
    }

    let expected_arch_sha = index
        .release_archive_sha256
        .as_deref()
        .ok_or_else(|| BakeError::new("index.json missing release_archive_sha256"))?;
    let archive_bytes = fs::read(archive_path)?;
    let actual_arch_sha = sha256_hex(&archive_bytes);
    if actual_arch_sha != expected_arch_sha {
        return Err(BakeError::new(format!(
            "release archive SHA-256 mismatch: expected {expected_arch_sha}, got {actual_arch_sha}"
        )));
    }

    // Build expected per-entry table from index.json
    let mut expected_entries: BTreeMap<String, (usize, String, String)> = BTreeMap::new();
    for val in &index.tiles {
        let path = val["path"]
            .as_str()
            .ok_or_else(|| BakeError::new("index.json tile entry missing path"))?
            .to_owned();
        if is_forbidden_archive_entry(&path) {
            return Err(BakeError::new(format!(
                "index.json erroneously lists forbidden archive entry '{path}'"
            )));
        }
        let byte_len = val["byte_len"]
            .as_u64()
            .ok_or_else(|| BakeError::new("index.json tile entry missing byte_len"))?
            as usize;
        let sha = val["sha256"]
            .as_str()
            .ok_or_else(|| BakeError::new("index.json tile entry missing sha256"))?
            .to_owned();
        let layer = val["layer"]
            .as_str()
            .ok_or_else(|| BakeError::new("index.json tile entry missing layer"))?
            .to_owned();
        if expected_entries
            .insert(path.clone(), (byte_len, sha, layer))
            .is_some()
        {
            return Err(BakeError::new(format!(
                "duplicate tile path '{path}' in index.json"
            )));
        }
    }

    let gz = GzDecoder::new(archive_bytes.as_slice());
    let mut tar = Archive::new(gz);
    let mut seen_count = 0_usize;
    let mut ktx2_count = 0_usize;

    for entry_res in tar.entries()? {
        let mut entry = entry_res?;
        let path = entry
            .path()?
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches("./")
            .to_owned();
        if is_forbidden_archive_entry(&path) {
            return Err(BakeError::new(format!(
                "LD-2 violation: tile archive MUST NOT contain '{path}'"
            )));
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        let (exp_len, exp_sha, _layer) = expected_entries.remove(&path).ok_or_else(|| {
            BakeError::new(format!(
                "unexpected archive entry '{path}' absent from index.json"
            ))
        })?;
        if buf.len() != exp_len {
            return Err(BakeError::new(format!(
                "archive entry '{path}' length mismatch: expected {exp_len}, got {}",
                buf.len()
            )));
        }
        let actual_sha = sha256_hex(&buf);
        if actual_sha != exp_sha {
            return Err(BakeError::new(format!(
                "archive entry '{path}' SHA-256 mismatch: expected {exp_sha}, got {actual_sha}"
            )));
        }
        if path.ends_with(".ktx2") {
            let _ = parse_ktx2(&buf)?;
            ktx2_count += 1;
        } else if path == "cave_network.bin" {
            let _ = CaveNetwork::from_bytes(&buf)?;
        } else if path.ends_with(".anmf") {
            let _ = FloraSectorBlob::from_bytes(&buf)?;
        }
        seen_count += 1;
    }

    if !expected_entries.is_empty() {
        let missing: Vec<&str> = expected_entries
            .keys()
            .take(5)
            .map(String::as_str)
            .collect();
        return Err(BakeError::new(format!(
            "archive missing {} entry(ies) listed in index.json (e.g. {:?})",
            expected_entries.len(),
            missing
        )));
    }

    Ok(TilePackVerifyReport {
        passed: true,
        bundle_identity,
        sim_world_identity,
        release_archive_sha256: actual_arch_sha,
        archive_entries_verified: seen_count,
        ktx2_headers_verified: ktx2_count,
    })
}

fn read_bin_u8x2_exact(path: &Path, expected_len: usize) -> Result<Vec<[u8; 2]>, BakeError> {
    let bytes = fs::read(path)?;
    if bytes.len() != expected_len * 2 {
        return Err(BakeError::new(format!(
            "binary u8x2 file {} length mismatch: expected {}, got {}",
            path.display(),
            expected_len * 2,
            bytes.len()
        )));
    }
    let (chunks, _) = bytes.as_chunks::<2>();
    Ok(chunks.to_vec())
}
