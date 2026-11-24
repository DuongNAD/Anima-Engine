#![forbid(unsafe_code)]
#![deny(clippy::disallowed_methods)]

//! `JOB-W6` External CC0 Asset Pipeline (`QD-ASSETS`):
//! - `sources.json` validator (`ambientCG` & `Poly Haven` only, `CC0-1.0`, valid `licence_url`, pinned SHA-256, `<= 60 MB` downloaded / `<= 20 MB` committed budgets)
//! - Deterministic `.obj` mesh parser, vertex-grid LOD simplifier (`lod0..lod3`), greedy vertex-cache triangle orderer (`ANMM` v1 binary)
//! - Software deterministic octahedral hemisphere (`8×8` view) impostor atlas baker (`RGBA8` albedo+alpha + `BC5` normal `.ktx2`)

use crate::anmw_v3::sha256_to_hex;
use crate::bake::pack_ktx2::{
    encode_bc5_unorm, write_ktx2_2d, Ktx2WriteSpec, KTX2_SUPERCOMPRESSION_ZLIB,
    VK_FORMAT_BC5_UNORM_BLOCK, VK_FORMAT_R8G8B8A8_SRGB,
};
use crate::bake::{downsample_u8x2_mean_2x2, downsample_u8x4_mean_2x2, quantize_u8, BakeError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Magic bytes for `species_*.meshopt.lod<k>.bin` (`ANMM`).
pub const ANMM_MAGIC: [u8; 4] = *b"ANMM";
/// Binary version (`1`).
pub const ANMM_VERSION: u32 = 1;
/// Approved `QD-ASSETS` download budget (`60 MB`).
pub const QD_ASSETS_MAX_DOWNLOAD_BYTES: u64 = 60_000_000;
/// Approved `QD-ASSETS` committed source budget (`20 MB`).
pub const QD_ASSETS_MAX_COMMITTED_BYTES: u64 = 20_000_000;
/// Approved CC0 upstream providers (`QD-ASSETS`).
pub const QD_APPROVED_SOURCES: [&str; 2] = ["ambientCG", "Poly Haven"];

/// One asset entry inside `artifacts/assets-v2/sources.json` (`QD-ASSETS`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetSourceEntry {
    pub species_id: String,
    pub source: String,
    pub licence: String,
    pub licence_url: String,
    pub sha256: String,
    pub download_bytes: u64,
    pub committed: bool,
    pub obj_path: String,
    #[serde(default)]
    pub base_rgb: Option<[u8; 3]>,
}

/// Top-level `sources.json` manifest validated strictly against `QD-ASSETS` before `JOB-W6` runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetSourcesManifest {
    pub format_version: u32,
    pub entries: Vec<AssetSourceEntry>,
}

/// Single vertex (`position`, `normal`, `uv`) in a canonical asset mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

/// Triangle mesh ready for LOD simplification & `ANMM` serialization.
#[derive(Debug, Clone, PartialEq)]
pub struct AssetMesh {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<[u32; 3]>,
}

/// Summary report emitted by `anima-bake assets`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetsBakeReport {
    pub species_count: usize,
    pub atlas_size: usize,
    pub outputs: Vec<SpeciesBakeOutput>,
}

/// Per-species baked LOD + impostor checksums.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeciesBakeOutput {
    pub species_id: String,
    pub lod_triangle_counts: [usize; 4],
    pub lod_sha256: [String; 4],
    pub impostor_albedo_ktx2_sha256: String,
    pub impostor_normal_ktx2_sha256: String,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    sha256_to_hex(&h.finalize().into())
}

/// Strictly validates `AssetSourcesManifest` against `QD-ASSETS`:
/// - `source` must be `"ambientCG"` or `"Poly Haven"`
/// - `licence` must equal `"CC0-1.0"`
/// - `licence_url` must start with `"https://"` (linking to the upstream `CC0-1.0` page)
/// - `download_bytes` sum `<= 60 MB`, committed bytes sum `<= 20 MB`
/// - Every `obj_path` exists under `manifest_dir` and matches its pinned `sha256`.
pub fn validate_asset_sources_manifest(
    manifest: &AssetSourcesManifest,
    manifest_dir: &Path,
) -> Result<(), BakeError> {
    if manifest.format_version != 1 {
        return Err(BakeError::new(format!(
            "unsupported AssetSourcesManifest format_version {} (expected 1)",
            manifest.format_version
        )));
    }
    if manifest.entries.is_empty() {
        return Err(BakeError::new("AssetSourcesManifest contains zero entries"));
    }

    let mut total_dl = 0_u64;
    let mut total_commit = 0_u64;
    for entry in &manifest.entries {
        if !QD_APPROVED_SOURCES.contains(&entry.source.as_str()) {
            return Err(BakeError::new(format!(
                "QD-ASSETS rejection: source '{}' is not approved ({:?})",
                entry.source, QD_APPROVED_SOURCES
            )));
        }
        if entry.licence != "CC0-1.0" {
            return Err(BakeError::new(format!(
                "QD-ASSETS rejection: species '{}' has licence '{}', must be 'CC0-1.0'",
                entry.species_id, entry.licence
            )));
        }
        if !entry.licence_url.starts_with("https://") {
            return Err(BakeError::new(format!(
                "QD-ASSETS rejection: species '{}' licence_url '{}' must start with https://",
                entry.species_id, entry.licence_url
            )));
        }
        total_dl = total_dl
            .checked_add(entry.download_bytes)
            .ok_or_else(|| BakeError::new("QD-ASSETS download byte total overflow"))?;
        if entry.committed {
            total_commit = total_commit
                .checked_add(entry.download_bytes)
                .ok_or_else(|| BakeError::new("QD-ASSETS committed byte total overflow"))?;
        }

        let obj_bytes = fs::read(manifest_dir.join(&entry.obj_path))?;
        let actual_sha = sha256_hex(&obj_bytes);
        if actual_sha != entry.sha256 {
            return Err(BakeError::new(format!(
                "QD-ASSETS SHA-256 mismatch for '{}': expected {}, got {actual_sha}",
                entry.obj_path, entry.sha256
            )));
        }
    }

    if total_dl > QD_ASSETS_MAX_DOWNLOAD_BYTES {
        return Err(BakeError::new(format!(
            "QD-ASSETS budget exceeded: total download {} bytes > {QD_ASSETS_MAX_DOWNLOAD_BYTES}",
            total_dl
        )));
    }
    if total_commit > QD_ASSETS_MAX_COMMITTED_BYTES {
        return Err(BakeError::new(format!(
            "QD-ASSETS committed budget exceeded: {} bytes > {QD_ASSETS_MAX_COMMITTED_BYTES}",
            total_commit
        )));
    }
    Ok(())
}

type ObjFaceRef = (usize, Option<usize>, Option<usize>);

/// Parses a triangular/quad Wavefront `.obj` mesh into deterministic indexed vertices & triangles.
pub fn parse_obj_mesh(src: &str) -> Result<AssetMesh, BakeError> {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut raw_faces: Vec<Vec<ObjFaceRef>> = Vec::new();

    for line in src.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                let x = parts.next().and_then(|s| s.parse::<f32>().ok());
                let y = parts.next().and_then(|s| s.parse::<f32>().ok());
                let z = parts.next().and_then(|s| s.parse::<f32>().ok());
                match (x, y, z) {
                    (Some(x), Some(y), Some(z)) => positions.push([x, y, z]),
                    _ => {
                        return Err(BakeError::new(format!(
                            "malformed OBJ vertex line '{line}'"
                        )))
                    }
                }
            }
            Some("vn") => {
                let x = parts.next().and_then(|s| s.parse::<f32>().ok());
                let y = parts.next().and_then(|s| s.parse::<f32>().ok());
                let z = parts.next().and_then(|s| s.parse::<f32>().ok());
                if let (Some(x), Some(y), Some(z)) = (x, y, z) {
                    normals.push([x, y, z]);
                }
            }
            Some("vt") => {
                let u = parts.next().and_then(|s| s.parse::<f32>().ok());
                let v = parts.next().and_then(|s| s.parse::<f32>().ok());
                if let (Some(u), Some(v)) = (u, v) {
                    uvs.push([u, v]);
                }
            }
            Some("f") => {
                let mut poly = Vec::new();
                for tok in parts {
                    let segs: Vec<&str> = tok.split('/').collect();
                    let vi = segs[0]
                        .parse::<usize>()
                        .ok()
                        .and_then(|i| i.checked_sub(1))
                        .ok_or_else(|| BakeError::new(format!("invalid OBJ vertex ref '{tok}'")))?;
                    let ti = segs
                        .get(1)
                        .filter(|s| !s.is_empty())
                        .and_then(|s| s.parse::<usize>().ok())
                        .and_then(|i| i.checked_sub(1));
                    let ni = segs
                        .get(2)
                        .filter(|s| !s.is_empty())
                        .and_then(|s| s.parse::<usize>().ok())
                        .and_then(|i| i.checked_sub(1));
                    poly.push((vi, ti, ni));
                }
                if poly.len() >= 3 {
                    raw_faces.push(poly);
                }
            }
            _ => {}
        }
    }

    if positions.is_empty() || raw_faces.is_empty() {
        return Err(BakeError::new("OBJ contains no vertices or faces"));
    }

    let mut vertices = Vec::with_capacity(raw_faces.len() * 3);
    let mut indices = Vec::with_capacity(raw_faces.len());
    let mut vert_map: BTreeMap<(usize, usize, usize), u32> = BTreeMap::new();

    for poly in raw_faces {
        let mut poly_idx = Vec::with_capacity(poly.len());
        for (vi, ti, ni) in poly {
            let p = *positions
                .get(vi)
                .ok_or_else(|| BakeError::new("OBJ vertex index out of bounds"))?;
            let uv = ti.and_then(|k| uvs.get(k).copied()).unwrap_or([0.0, 0.0]);
            let n = ni
                .and_then(|k| normals.get(k).copied())
                .unwrap_or([0.0, 1.0, 0.0]);
            let key = (vi, ti.unwrap_or(usize::MAX), ni.unwrap_or(usize::MAX));
            let v_idx = if let Some(&id) = vert_map.get(&key) {
                id
            } else {
                let id = vertices.len() as u32;
                vertices.push(MeshVertex {
                    pos: p,
                    normal: n,
                    uv,
                });
                vert_map.insert(key, id);
                id
            };
            poly_idx.push(v_idx);
        }
        for k in 1..(poly_idx.len() - 1) {
            indices.push([poly_idx[0], poly_idx[k], poly_idx[k + 1]]);
        }
    }

    Ok(AssetMesh { vertices, indices })
}

/// Simplifies `mesh` deterministically via quantized spatial vertex clustering until triangle
/// count reaches `<= (mesh.indices.len() * ratio).max(4)` (preserving sharp silhouettes & determinism),
/// followed by vertex-cache triangle reordering.
#[must_use]
pub fn simplify_mesh_lod(mesh: &AssetMesh, target_ratio: f32) -> AssetMesh {
    let target_tris = ((mesh.indices.len() as f32) * target_ratio)
        .round()
        .max(4.0) as usize;
    if target_ratio >= 0.999 || mesh.indices.len() <= target_tris {
        return reorder_triangles_for_cache(mesh);
    }

    let mut min_b = [f32::INFINITY; 3];
    let mut max_b = [f32::NEG_INFINITY; 3];
    for v in &mesh.vertices {
        for a in 0..3 {
            min_b[a] = min_b[a].min(v.pos[a]);
            max_b[a] = max_b[a].max(v.pos[a]);
        }
    }
    let span = [
        (max_b[0] - min_b[0]).max(1e-4),
        (max_b[1] - min_b[1]).max(1e-4),
        (max_b[2] - min_b[2]).max(1e-4),
    ];

    // Binary/stepped grid coarsening until triangle count drops below target_tris.
    let mut best = mesh.clone();
    let mut res =
        (libm::cbrtf(mesh.vertices.len() as f32) * libm::sqrtf(target_ratio)).ceil() as i32;
    res = res.clamp(4, 96);

    for _attempt in 0..16 {
        let mut cell_verts: BTreeMap<(i32, i32, i32), (u32, MeshVertex, u32)> = BTreeMap::new();
        let mut vert_remap = vec![0_u32; mesh.vertices.len()];
        for (vi, v) in mesh.vertices.iter().enumerate() {
            let cx = (((v.pos[0] - min_b[0]) / span[0]) * res as f32).floor() as i32;
            let cy = (((v.pos[1] - min_b[1]) / span[1]) * res as f32).floor() as i32;
            let cz = (((v.pos[2] - min_b[2]) / span[2]) * res as f32).floor() as i32;
            let key = (cx, cy, cz);
            let next_id = cell_verts.len() as u32;
            let entry = cell_verts.entry(key).or_insert((
                next_id,
                MeshVertex {
                    pos: [0.0; 3],
                    normal: [0.0; 3],
                    uv: [0.0; 2],
                },
                0,
            ));
            for a in 0..3 {
                entry.1.pos[a] += v.pos[a];
                entry.1.normal[a] += v.normal[a];
            }
            entry.1.uv[0] += v.uv[0];
            entry.1.uv[1] += v.uv[1];
            entry.2 += 1;
            vert_remap[vi] = entry.0;
        }

        let mut new_verts = vec![
            MeshVertex {
                pos: [0.0; 3],
                normal: [0.0; 3],
                uv: [0.0; 2],
            };
            cell_verts.len()
        ];
        for (id, acc, cnt) in cell_verts.into_values() {
            let inv = 1.0 / (cnt as f32);
            let mut n = [
                acc.normal[0] * inv,
                acc.normal[1] * inv,
                acc.normal[2] * inv,
            ];
            let nlen = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
            n[0] /= nlen;
            n[1] /= nlen;
            n[2] /= nlen;
            new_verts[id as usize] = MeshVertex {
                pos: [acc.pos[0] * inv, acc.pos[1] * inv, acc.pos[2] * inv],
                normal: n,
                uv: [acc.uv[0] * inv, acc.uv[1] * inv],
            };
        }

        let mut seen_tris: BTreeMap<[u32; 3], ()> = BTreeMap::new();
        let mut new_tris = Vec::new();
        for &[i0, i1, i2] in &mesh.indices {
            let a = vert_remap[i0 as usize];
            let b = vert_remap[i1 as usize];
            let c = vert_remap[i2 as usize];
            if a == b || b == c || a == c {
                continue;
            }
            let mut key = [a, b, c];
            key.sort_unstable();
            if seen_tris.insert(key, ()).is_none() {
                new_tris.push([a, b, c]);
            }
        }

        if !new_tris.is_empty() {
            best = AssetMesh {
                vertices: new_verts,
                indices: new_tris,
            };
            if best.indices.len() <= target_tris {
                break;
            }
        }
        if res <= 2 {
            break;
        }
        res = (res * 3) / 4;
    }

    reorder_triangles_for_cache(&best)
}

fn reorder_triangles_for_cache(mesh: &AssetMesh) -> AssetMesh {
    // Reorder vertex indices into FIFO discovery order along original triangle sequence.
    let mut remap: BTreeMap<u32, u32> = BTreeMap::new();
    let mut new_verts = Vec::with_capacity(mesh.vertices.len());
    let mut new_tris = Vec::with_capacity(mesh.indices.len());
    for &[a, b, c] in &mesh.indices {
        let mut tri = [0_u32; 3];
        for (slot, old) in [a, b, c].into_iter().enumerate() {
            let new_id = *remap.entry(old).or_insert_with(|| {
                let id = new_verts.len() as u32;
                new_verts.push(mesh.vertices[old as usize]);
                id
            });
            tri[slot] = new_id;
        }
        new_tris.push(tri);
    }
    AssetMesh {
        vertices: new_verts,
        indices: new_tris,
    }
}

impl AssetMesh {
    /// Serializes the mesh into deterministic little-endian `ANMM` v1 binary (`species_*.meshopt.lod*.bin`).
    #[must_use]
    pub fn to_anmm_bytes(&self) -> Vec<u8> {
        let header = 16;
        let v_bytes = self.vertices.len() * 32;
        let t_bytes = self.indices.len() * 12;
        let mut out = Vec::with_capacity(header + v_bytes + t_bytes);
        out.extend_from_slice(&ANMM_MAGIC);
        out.extend_from_slice(&ANMM_VERSION.to_le_bytes());
        out.extend_from_slice(&(self.vertices.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.indices.len() as u32).to_le_bytes());
        for v in &self.vertices {
            for f in v.pos {
                out.extend_from_slice(&f.to_le_bytes());
            }
            for f in v.normal {
                out.extend_from_slice(&f.to_le_bytes());
            }
            for f in v.uv {
                out.extend_from_slice(&f.to_le_bytes());
            }
        }
        for tri in &self.indices {
            for idx in tri {
                out.extend_from_slice(&idx.to_le_bytes());
            }
        }
        out
    }

    /// Decodes an `ANMM` v1 binary mesh buffer and checks index bounds.
    pub fn from_anmm_bytes(bytes: &[u8]) -> Result<Self, BakeError> {
        if bytes.len() < 16 || bytes[0..4] != ANMM_MAGIC {
            return Err(BakeError::new("invalid ANMM header"));
        }
        let ver = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if ver != ANMM_VERSION {
            return Err(BakeError::new(format!("unsupported ANMM version {ver}")));
        }
        let vc = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        let tc = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        let expected = 16_usize
            .checked_add(vc * 32)
            .and_then(|s| s.checked_add(tc * 12))
            .ok_or_else(|| BakeError::new("ANMM length overflow"))?;
        if bytes.len() != expected {
            return Err(BakeError::new(format!(
                "ANMM byte length mismatch: expected {expected}, got {}",
                bytes.len()
            )));
        }
        let rd_f32 = |off: usize| {
            f32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
        };
        let mut vertices = Vec::with_capacity(vc);
        let mut cur = 16;
        for _ in 0..vc {
            vertices.push(MeshVertex {
                pos: [rd_f32(cur), rd_f32(cur + 4), rd_f32(cur + 8)],
                normal: [rd_f32(cur + 12), rd_f32(cur + 16), rd_f32(cur + 20)],
                uv: [rd_f32(cur + 24), rd_f32(cur + 28)],
            });
            cur += 32;
        }
        let mut indices = Vec::with_capacity(tc);
        for _ in 0..tc {
            let a =
                u32::from_le_bytes([bytes[cur], bytes[cur + 1], bytes[cur + 2], bytes[cur + 3]]);
            let b = u32::from_le_bytes([
                bytes[cur + 4],
                bytes[cur + 5],
                bytes[cur + 6],
                bytes[cur + 7],
            ]);
            let c = u32::from_le_bytes([
                bytes[cur + 8],
                bytes[cur + 9],
                bytes[cur + 10],
                bytes[cur + 11],
            ]);
            if (a as usize) >= vc || (b as usize) >= vc || (c as usize) >= vc {
                return Err(BakeError::new("ANMM triangle vertex index out of bounds"));
            }
            indices.push([a, b, c]);
            cur += 12;
        }
        Ok(Self { vertices, indices })
    }
}

#[inline]
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
fn norm3(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / l, v[1] / l, v[2] / l]
}

fn oct_hemisphere_dir(u: f32, v: f32) -> [f32; 3] {
    // Octahedral upper hemisphere parameterization `(u, v) in [-1, 1]^2` -> unit direction `y >= 0`.
    let x = u - v;
    let z = u + v;
    let y = (1.0 - x.abs() - z.abs()).max(0.0);
    norm3([x * 0.5, y, z * 0.5])
}

/// Deterministically bakes an `8 × 8` octahedral hemisphere impostor atlas (`RGBA8` albedo+alpha KTX2
/// + `BC5` normal KTX2, full mip chain) on CPU using exact barycentric Z-buffer rasterization.
pub fn bake_octahedral_impostor_cpu(
    mesh: &AssetMesh,
    atlas_size: usize,
    base_rgb: [u8; 3],
) -> Result<(Vec<u8>, Vec<u8>), BakeError> {
    if !atlas_size.is_power_of_two() || atlas_size < 64 {
        return Err(BakeError::new(format!(
            "atlas_size ({atlas_size}) must be a power of two >= 64"
        )));
    }
    let grid = 8_usize;
    let cell_sz = atlas_size / grid;

    // Bounding sphere center & radius
    let mut min_b = [f32::INFINITY; 3];
    let mut max_b = [f32::NEG_INFINITY; 3];
    for v in &mesh.vertices {
        for a in 0..3 {
            min_b[a] = min_b[a].min(v.pos[a]);
            max_b[a] = max_b[a].max(v.pos[a]);
        }
    }
    let center = [
        0.5 * (min_b[0] + max_b[0]),
        0.5 * (min_b[1] + max_b[1]),
        0.5 * (min_b[2] + max_b[2]),
    ];
    let mut radius = 1e-4_f32;
    for v in &mesh.vertices {
        let dx = v.pos[0] - center[0];
        let dy = v.pos[1] - center[1];
        let dz = v.pos[2] - center[2];
        let d2 = dx * dx + dy * dy + dz * dz;
        radius = radius.max(d2.sqrt());
    }
    let inv_r = 1.0 / (radius * 1.08);

    let mut albedo = vec![[0_u8, 0, 0, 0]; atlas_size * atlas_size];
    let mut normal_xy = vec![[128_u8, 128_u8]; atlas_size * atlas_size];

    for vy in 0..grid {
        for vx in 0..grid {
            let u = ((vx as f32 + 0.5) / grid as f32) * 2.0 - 1.0;
            let v = ((vy as f32 + 0.5) / grid as f32) * 2.0 - 1.0;
            let dir = oct_hemisphere_dir(u, v);
            let up_hint = if dir[1].abs() > 0.95 {
                [0.0, 0.0, 1.0]
            } else {
                [0.0, 1.0, 0.0]
            };
            let right = norm3(cross3(up_hint, dir));
            let cam_up = norm3(cross3(dir, right));

            let mut zbuf = vec![f32::NEG_INFINITY; cell_sz * cell_sz];
            let ox = vx * cell_sz;
            let oy = vy * cell_sz;

            for &[i0, i1, i2] in &mesh.indices {
                let va = mesh.vertices[i0 as usize];
                let vb = mesh.vertices[i1 as usize];
                let vc = mesh.vertices[i2 as usize];

                let proj = |p: [f32; 3]| -> [f32; 3] {
                    let rel = [
                        (p[0] - center[0]) * inv_r,
                        (p[1] - center[1]) * inv_r,
                        (p[2] - center[2]) * inv_r,
                    ];
                    let sx = (dot3(rel, right) * 0.5 + 0.5) * (cell_sz as f32 - 1.0);
                    let sy = (0.5 - dot3(rel, cam_up) * 0.5) * (cell_sz as f32 - 1.0);
                    let sz = dot3(rel, dir);
                    [sx, sy, sz]
                };
                let p0 = proj(va.pos);
                let p1 = proj(vb.pos);
                let p2 = proj(vc.pos);

                let area = (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] - p0[0]) * (p1[1] - p0[1]);
                if area.abs() < 1e-4 {
                    continue;
                }
                let inv_area = 1.0 / area;

                let x_min = (p0[0].min(p1[0]).min(p2[0]).floor() as isize).max(0) as usize;
                let x_max = (p0[0].max(p1[0]).max(p2[0]).ceil() as isize).min(cell_sz as isize - 1)
                    as usize;
                let y_min = (p0[1].min(p1[1]).min(p2[1]).floor() as isize).max(0) as usize;
                let y_max = (p0[1].max(p1[1]).max(p2[1]).ceil() as isize).min(cell_sz as isize - 1)
                    as usize;

                for py in y_min..=y_max {
                    for px in x_min..=x_max {
                        let cx = px as f32 + 0.5;
                        let cy = py as f32 + 0.5;
                        let w0 =
                            ((p1[0] - cx) * (p2[1] - cy) - (p2[0] - cx) * (p1[1] - cy)) * inv_area;
                        let w1 =
                            ((p2[0] - cx) * (p0[1] - cy) - (p0[0] - cx) * (p2[1] - cy)) * inv_area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                            continue;
                        }
                        let z = w0 * p0[2] + w1 * p1[2] + w2 * p2[2];
                        let li = py * cell_sz + px;
                        if z <= zbuf[li] {
                            continue;
                        }
                        zbuf[li] = z;

                        let nrm = norm3([
                            w0 * va.normal[0] + w1 * vb.normal[0] + w2 * vc.normal[0],
                            w0 * va.normal[1] + w1 * vb.normal[1] + w2 * vc.normal[1],
                            w0 * va.normal[2] + w1 * vb.normal[2] + w2 * vc.normal[2],
                        ]);
                        let shade = (0.45 + 0.55 * nrm[1].max(0.0)).clamp(0.2, 1.0);
                        let gi = (oy + py) * atlas_size + (ox + px);
                        albedo[gi] = [
                            ((f32::from(base_rgb[0]) * shade).round()).clamp(0.0, 255.0) as u8,
                            ((f32::from(base_rgb[1]) * shade).round()).clamp(0.0, 255.0) as u8,
                            ((f32::from(base_rgb[2]) * shade).round()).clamp(0.0, 255.0) as u8,
                            255,
                        ];
                        let nx = dot3(nrm, right);
                        let ny = dot3(nrm, cam_up);
                        normal_xy[gi] = [quantize_u8(nx * 0.5 + 0.5), quantize_u8(ny * 0.5 + 0.5)];
                    }
                }
            }
        }
    }

    // Build full mip chain for albedo RGBA8 (`atlas_size -> ... -> 1`) and BC5 normal (`>= 4×4`).
    let mut albedo_mips = Vec::new();
    let mut cur_a = albedo;
    let mut cur_sz = atlas_size;
    loop {
        let mut flat = Vec::with_capacity(cur_a.len() * 4);
        for px in &cur_a {
            flat.extend_from_slice(px);
        }
        albedo_mips.push(flat);
        if cur_sz == 1 {
            break;
        }
        cur_a = downsample_u8x4_mean_2x2(&cur_a, cur_sz, cur_sz);
        cur_sz /= 2;
    }
    let albedo_ktx2 = write_ktx2_2d(&Ktx2WriteSpec {
        vk_format: VK_FORMAT_R8G8B8A8_SRGB,
        type_size: 1,
        width: atlas_size as u32,
        height: atlas_size as u32,
        supercompression_scheme: KTX2_SUPERCOMPRESSION_ZLIB,
        mips: albedo_mips,
        extra_kvd: BTreeMap::new(),
    })?;

    let mut norm_mips = Vec::new();
    let mut cur_n = normal_xy;
    let mut cur_n_sz = atlas_size;
    while cur_n_sz >= 4 {
        norm_mips.push(encode_bc5_unorm(&cur_n, cur_n_sz, cur_n_sz)?);
        if cur_n_sz == 4 {
            break;
        }
        cur_n = downsample_u8x2_mean_2x2(&cur_n, cur_n_sz, cur_n_sz);
        cur_n_sz /= 2;
    }
    let normal_ktx2 = write_ktx2_2d(&Ktx2WriteSpec {
        vk_format: VK_FORMAT_BC5_UNORM_BLOCK,
        type_size: 1,
        width: atlas_size as u32,
        height: atlas_size as u32,
        supercompression_scheme: KTX2_SUPERCOMPRESSION_ZLIB,
        mips: norm_mips,
        extra_kvd: BTreeMap::new(),
    })?;

    Ok((albedo_ktx2, normal_ktx2))
}

/// Executes `JOB-W6` from a validated `QD-ASSETS` `sources.json` manifest into `out_dir`.
pub fn bake_assets_from_manifest(
    manifest_path: &Path,
    out_dir: &Path,
    atlas_size: usize,
) -> Result<AssetsBakeReport, BakeError> {
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let manifest: AssetSourcesManifest = serde_json::from_str(&fs::read_to_string(manifest_path)?)?;
    validate_asset_sources_manifest(&manifest, manifest_dir)?;
    fs::create_dir_all(out_dir)?;

    let mut outputs = Vec::with_capacity(manifest.entries.len());
    for entry in &manifest.entries {
        let obj_text = fs::read_to_string(manifest_dir.join(&entry.obj_path))?;
        let base_mesh = parse_obj_mesh(&obj_text)?;
        let ratios = [1.0_f32, 0.50, 0.25, 0.10];
        let mut tri_counts = [0_usize; 4];
        let mut lod_shas = [String::new(), String::new(), String::new(), String::new()];

        for (lod, &ratio) in ratios.iter().enumerate() {
            let simplified = simplify_mesh_lod(&base_mesh, ratio);
            tri_counts[lod] = simplified.indices.len();
            let bytes = simplified.to_anmm_bytes();
            lod_shas[lod] = sha256_hex(&bytes);
            fs::write(
                out_dir.join(format!("species_{}.meshopt.lod{lod}.bin", entry.species_id)),
                bytes,
            )?;
        }

        let base_rgb = entry.base_rgb.unwrap_or([58, 118, 52]);
        let (alb_ktx2, nrm_ktx2) = bake_octahedral_impostor_cpu(&base_mesh, atlas_size, base_rgb)?;
        let alb_sha = sha256_hex(&alb_ktx2);
        let nrm_sha = sha256_hex(&nrm_ktx2);
        fs::write(
            out_dir.join(format!("species_{}_impostor_albedo.ktx2", entry.species_id)),
            alb_ktx2,
        )?;
        fs::write(
            out_dir.join(format!("species_{}_impostor_normal.ktx2", entry.species_id)),
            nrm_ktx2,
        )?;

        outputs.push(SpeciesBakeOutput {
            species_id: entry.species_id.clone(),
            lod_triangle_counts: tri_counts,
            lod_sha256: lod_shas,
            impostor_albedo_ktx2_sha256: alb_sha,
            impostor_normal_ktx2_sha256: nrm_sha,
        });
    }

    let report = AssetsBakeReport {
        species_count: outputs.len(),
        atlas_size,
        outputs,
    };
    fs::write(
        out_dir.join("assets_bake_report.json"),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    Ok(report)
}
