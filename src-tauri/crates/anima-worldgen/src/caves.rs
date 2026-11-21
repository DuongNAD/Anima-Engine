//! Stage S9: Data-only karst (`lithology == 2`) & volcanic basalt (`lithology == 3`) cave
//! networks (`§2.2.9`, `QD-REALMS`, `Palmer 1991`), `cave_mask` projection, and exact little-endian
//! `ANMC` v1 `cave_network.bin` serialization/deserialization (codec available under `reader`).

#[cfg(feature = "generate")]
use crate::rng::stage_rng;
#[cfg(feature = "generate")]
use crate::{BiomeId, WaterKind};
#[cfg(feature = "generate")]
use rand::Rng;
use std::fmt;

/// Magic header bytes for `cave_network.bin` (`ANMC`).
pub const ANMC_MAGIC: [u8; 4] = *b"ANMC";
/// Binary format version (`1`).
pub const ANMC_VERSION: u32 = 1;
/// Fixed header length in bytes (`24`).
pub const ANMC_HEADER_LEN: usize = 24;
/// Fixed node record length in bytes (`20`).
pub const ANMC_NODE_LEN: usize = 20;
/// Fixed edge record length in bytes (`12`).
pub const ANMC_EDGE_LEN: usize = 12;
/// Fixed entrance record length in bytes (`20`).
pub const ANMC_ENTRANCE_LEN: usize = 20;

/// Error returned when decoding an `ANMC` v1 `cave_network.bin` payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaveCodecError {
    pub message: String,
}

impl fmt::Display for CaveCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CaveCodecError {}

/// Canonical subterranean passage morphology (`§2.2.9`).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassageKind {
    KarstPhreatic = 0,
    KarstVadose = 1,
    BasaltLavaTube = 2,
    SinkholeShaft = 3,
}

impl PassageKind {
    /// Converts a `u8` tag into [`PassageKind`].
    #[must_use]
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::KarstPhreatic),
            1 => Some(Self::KarstVadose),
            2 => Some(Self::BasaltLavaTube),
            3 => Some(Self::SinkholeShaft),
            _ => None,
        }
    }
}

/// Canonical cave entrance portal type (`§2.2.9`).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouthKind {
    CliffPortal = 0,
    KarstSinkhole = 1,
    LavaTubeMouth = 2,
}

impl MouthKind {
    /// Converts a `u8` tag into [`MouthKind`].
    #[must_use]
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::CliffPortal),
            1 => Some(Self::KarstSinkhole),
            2 => Some(Self::LavaTubeMouth),
            _ => None,
        }
    }
}

/// A 3-D vertex in the subterranean cave graph (`20` bytes LE on disk).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaveNode {
    pub x_m: f32,
    pub y_m: f32,
    pub z_m: f32,
    pub radius_m: f32,
    pub light_decay: f32,
}

/// An undirected/directed passage link between two [`CaveNode`] indices (`12` bytes LE on disk).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaveEdge {
    pub from_idx: u32,
    pub to_idx: u32,
    pub passage_kind: u8,
}

/// A surface entrance portal bound to a [`CaveNode`] (`20` bytes LE on disk).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaveEntrance {
    pub node_idx: u32,
    pub mouth_kind: u8,
    /// `1 = decorative_until_underground_adr` (`§2.2.9`).
    pub realm_status: u8,
    pub yaw_rad: f32,
    pub elevation_m: f32,
    pub clearance_m: f32,
}

/// Full cave passage & entrance graph (`cave_network.bin` `ANMC` v1).
#[derive(Debug, Clone, PartialEq)]
pub struct CaveNetwork {
    pub flags: u32,
    pub nodes: Vec<CaveNode>,
    pub edges: Vec<CaveEdge>,
    pub entrances: Vec<CaveEntrance>,
}

#[inline]
fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[inline]
fn read_f32_le(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

impl CaveNetwork {
    /// Serializes the cave network to exact little-endian `ANMC` v1 binary bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let total_len = ANMC_HEADER_LEN
            + self.nodes.len() * ANMC_NODE_LEN
            + self.edges.len() * ANMC_EDGE_LEN
            + self.entrances.len() * ANMC_ENTRANCE_LEN;
        let mut out = Vec::with_capacity(total_len);

        out.extend_from_slice(&ANMC_MAGIC);
        out.extend_from_slice(&ANMC_VERSION.to_le_bytes());
        out.extend_from_slice(&self.flags.to_le_bytes());
        out.extend_from_slice(&(self.nodes.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.edges.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.entrances.len() as u32).to_le_bytes());

        for node in &self.nodes {
            out.extend_from_slice(&node.x_m.to_le_bytes());
            out.extend_from_slice(&node.y_m.to_le_bytes());
            out.extend_from_slice(&node.z_m.to_le_bytes());
            out.extend_from_slice(&node.radius_m.to_le_bytes());
            out.extend_from_slice(&node.light_decay.to_le_bytes());
        }
        for edge in &self.edges {
            out.extend_from_slice(&edge.from_idx.to_le_bytes());
            out.extend_from_slice(&edge.to_idx.to_le_bytes());
            out.push(edge.passage_kind);
            out.extend_from_slice(&[0_u8; 3]);
        }
        for ent in &self.entrances {
            out.extend_from_slice(&ent.node_idx.to_le_bytes());
            out.push(ent.mouth_kind);
            out.push(ent.realm_status);
            out.extend_from_slice(&[0_u8; 2]);
            out.extend_from_slice(&ent.yaw_rad.to_le_bytes());
            out.extend_from_slice(&ent.elevation_m.to_le_bytes());
            out.extend_from_slice(&ent.clearance_m.to_le_bytes());
        }
        out
    }

    /// Decodes and strictly validates an `ANMC` v1 little-endian binary payload.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CaveCodecError> {
        let err = |msg: &str| CaveCodecError {
            message: msg.to_owned(),
        };
        if bytes.len() < ANMC_HEADER_LEN {
            return Err(err("ANMC buffer shorter than 24-byte header"));
        }
        if bytes[0..4] != ANMC_MAGIC {
            return Err(err("invalid ANMC magic bytes"));
        }
        let version = read_u32_le(bytes, 4);
        if version != ANMC_VERSION {
            return Err(CaveCodecError {
                message: format!("unsupported ANMC version {version}"),
            });
        }
        let flags = read_u32_le(bytes, 8);
        if flags != 0 {
            return Err(CaveCodecError {
                message: format!("unsupported ANMC flags {flags}"),
            });
        }

        let node_count = read_u32_le(bytes, 12) as usize;
        let edge_count = read_u32_le(bytes, 16) as usize;
        let entrance_count = read_u32_le(bytes, 20) as usize;

        // Overflow-safe exact byte-length validation BEFORE any allocation.
        let nodes_bytes = node_count
            .checked_mul(ANMC_NODE_LEN)
            .ok_or_else(|| err("node_count byte length overflow"))?;
        let edges_bytes = edge_count
            .checked_mul(ANMC_EDGE_LEN)
            .ok_or_else(|| err("edge_count byte length overflow"))?;
        let entrances_bytes = entrance_count
            .checked_mul(ANMC_ENTRANCE_LEN)
            .ok_or_else(|| err("entrance_count byte length overflow"))?;
        let expected_len = ANMC_HEADER_LEN
            .checked_add(nodes_bytes)
            .and_then(|v| v.checked_add(edges_bytes))
            .and_then(|v| v.checked_add(entrances_bytes))
            .ok_or_else(|| err("ANMC total length overflow"))?;

        if bytes.len() != expected_len {
            return Err(CaveCodecError {
                message: format!(
                    "ANMC byte length mismatch: expected {expected_len}, got {}",
                    bytes.len()
                ),
            });
        }

        let mut cursor = ANMC_HEADER_LEN;
        let mut nodes = Vec::with_capacity(node_count);
        for _ in 0..node_count {
            let x_m = read_f32_le(bytes, cursor);
            let y_m = read_f32_le(bytes, cursor + 4);
            let z_m = read_f32_le(bytes, cursor + 8);
            let radius_m = read_f32_le(bytes, cursor + 12);
            let light_decay = read_f32_le(bytes, cursor + 16);
            if !x_m.is_finite()
                || !y_m.is_finite()
                || !z_m.is_finite()
                || !radius_m.is_finite()
                || !light_decay.is_finite()
                || radius_m <= 0.0
                || light_decay < 0.0
            {
                return Err(err("invalid or non-finite ANMC node fields"));
            }
            nodes.push(CaveNode {
                x_m,
                y_m,
                z_m,
                radius_m,
                light_decay,
            });
            cursor += ANMC_NODE_LEN;
        }

        let mut edges = Vec::with_capacity(edge_count);
        for _ in 0..edge_count {
            let from_idx = read_u32_le(bytes, cursor);
            let to_idx = read_u32_le(bytes, cursor + 4);
            let passage_kind = bytes[cursor + 8];
            if (from_idx as usize) >= node_count || (to_idx as usize) >= node_count {
                return Err(err("ANMC edge node index out of bounds"));
            }
            if PassageKind::from_u8(passage_kind).is_none() {
                return Err(err("invalid ANMC passage_kind tag"));
            }
            edges.push(CaveEdge {
                from_idx,
                to_idx,
                passage_kind,
            });
            cursor += ANMC_EDGE_LEN;
        }

        let mut entrances = Vec::with_capacity(entrance_count);
        for _ in 0..entrance_count {
            let node_idx = read_u32_le(bytes, cursor);
            let mouth_kind = bytes[cursor + 4];
            let realm_status = bytes[cursor + 5];
            let yaw_rad = read_f32_le(bytes, cursor + 8);
            let elevation_m = read_f32_le(bytes, cursor + 12);
            let clearance_m = read_f32_le(bytes, cursor + 16);
            if (node_idx as usize) >= node_count {
                return Err(err("ANMC entrance node_idx out of bounds"));
            }
            if MouthKind::from_u8(mouth_kind).is_none() {
                return Err(err("invalid ANMC mouth_kind tag"));
            }
            if !yaw_rad.is_finite()
                || !elevation_m.is_finite()
                || !clearance_m.is_finite()
                || clearance_m <= 0.0
            {
                return Err(err("invalid or non-finite ANMC entrance fields"));
            }
            entrances.push(CaveEntrance {
                node_idx,
                mouth_kind,
                realm_status,
                yaw_rad,
                elevation_m,
                clearance_m,
            });
            cursor += ANMC_ENTRANCE_LEN;
        }

        Ok(Self {
            flags,
            nodes,
            edges,
            entrances,
        })
    }
}

/// Error returned when Stage S9 cave generation fails invariants.
#[cfg(feature = "generate")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaveError {
    pub message: String,
}

#[cfg(feature = "generate")]
impl fmt::Display for CaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

#[cfg(feature = "generate")]
impl std::error::Error for CaveError {}

/// Configuration for Stage S9 cave entrance selection & passage network generation.
#[cfg(feature = "generate")]
#[derive(Debug, Clone, PartialEq)]
pub struct CaveConfig {
    pub seed: u64,
    pub schema_tag: String,
    pub dx_m: f32,
    pub sea_level_m: f32,
    /// Minimum accepted entrance count (`30`).
    pub min_entrances: usize,
    /// Maximum accepted entrance count (`48`).
    pub max_entrances: usize,
    /// Target entrance count within `[min_entrances, max_entrances]` (`40`).
    pub target_entrances: usize,
    /// Minimum pairwise Euclidean spacing as a fraction of grid side (`0.01`).
    pub min_spacing_fraction: f32,
}

#[cfg(feature = "generate")]
impl Default for CaveConfig {
    fn default() -> Self {
        Self {
            seed: 0x0057_5632_2026_1120,
            schema_tag: "ANMW3-SNSM-v1".to_owned(),
            dx_m: 10.0,
            sea_level_m: 0.0,
            min_entrances: 30,
            max_entrances: 48,
            target_entrances: 40,
            min_spacing_fraction: 0.01,
        }
    }
}

/// Borrowed input slices consumed by [`generate_caves`].
#[cfg(feature = "generate")]
pub struct CaveInput<'a> {
    pub width: usize,
    pub height: usize,
    pub total_h_m: &'a [f32],
    pub lithology_id: &'a [u8],
    pub water_kind: &'a [u8],
    pub temp_c: &'a [f32],
    pub glacier_h_m: &'a [f32],
    pub dominant_biome: &'a [u8],
}

/// Output `cave_mask` and [`CaveNetwork`] graph generated by [`generate_caves`].
#[cfg(feature = "generate")]
#[derive(Debug, Clone)]
pub struct CaveField {
    pub width: usize,
    pub height: usize,
    /// Sparse projection (`0` = none, `1` = subterranean passage, `2` = entrance portal).
    pub cave_mask: Vec<u8>,
    pub network: CaveNetwork,
}

/// Generates karst (`lithology == 2`) & basalt (`lithology == 3`) cave networks (`30..=48`
/// entrances, spacing `>= 0.01 * size`, zero ocean/lake/river/glacier/snow/sub-zero portals).
#[cfg(feature = "generate")]
pub fn generate_caves(input: &CaveInput<'_>, cfg: &CaveConfig) -> Result<CaveField, CaveError> {
    let w = input.width;
    let h = input.height;
    if w < 16 || h < 16 {
        return Err(CaveError {
            message: "grid dimensions must be >= 16x16".to_owned(),
        });
    }
    let n = w * h;
    if input.total_h_m.len() != n
        || input.lithology_id.len() != n
        || input.water_kind.len() != n
        || input.temp_c.len() != n
        || input.glacier_h_m.len() != n
        || input.dominant_biome.len() != n
    {
        return Err(CaveError {
            message: "input slice lengths do not match grid dimensions".to_owned(),
        });
    }

    let mut rng = stage_rng(cfg.seed, w as u32, h as u32, &cfg.schema_tag, "S9-CAVE");

    struct Cand {
        score: f32,
        idx: usize,
        slope_deg: f32,
        dh_dx: f32,
        dh_dz: f32,
    }

    let mut cands: Vec<Cand> = Vec::new();
    let inv_2dx = 0.5 / cfg.dx_m;

    for r in 2..(h - 2) {
        for c in 2..(w - 2) {
            let idx = r * w + c;
            let lith = input.lithology_id[idx];
            if lith != 2 && lith != 3 {
                continue;
            }
            let elev = input.total_h_m[idx];
            if elev <= cfg.sea_level_m + 8.0 {
                continue;
            }
            if input.water_kind[idx] != WaterKind::None as u8 {
                continue;
            }
            if input.glacier_h_m[idx] > 0.0 || input.temp_c[idx] <= 0.5 {
                continue;
            }
            let dom = input.dominant_biome[idx];
            if dom == BiomeId::Ocean as u8
                || dom == BiomeId::Lake as u8
                || dom == BiomeId::River as u8
                || dom == BiomeId::Snow as u8
                || dom == BiomeId::Glacier as u8
            {
                continue;
            }

            let dh_dx = (input.total_h_m[idx + 1] - input.total_h_m[idx - 1]) * inv_2dx;
            let dh_dz =
                (input.total_h_m[(r + 1) * w + c] - input.total_h_m[(r - 1) * w + c]) * inv_2dx;
            let slope_deg =
                libm::atanf(libm::hypotf(dh_dx, dh_dz)) * (180.0 / std::f32::consts::PI);
            let jitter: f32 = rng.gen_range(0.0_f32..3.0_f32);
            cands.push(Cand {
                score: slope_deg + jitter,
                idx,
                slope_deg,
                dh_dx,
                dh_dz,
            });
        }
    }

    cands.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.idx.cmp(&b.idx)));

    let min_dim = w.min(h) as f64;
    let min_dist_cells = f64::from(cfg.min_spacing_fraction) * min_dim * (1.0 + 1e-6);
    let min_dist_sq = min_dist_cells * min_dist_cells;
    let target = cfg
        .target_entrances
        .clamp(cfg.min_entrances, cfg.max_entrances);

    let mut chosen: Vec<&Cand> = Vec::with_capacity(target);
    let mut chosen_set = vec![false; n];

    // Tiered slope fallback: steep cliffs (>=20 deg), moderate slopes (>=5 deg), then any (>=0 deg).
    for &min_slope in &[20.0_f32, 5.0_f32, 0.0_f32] {
        if chosen.len() >= target {
            break;
        }
        for cand in &cands {
            if chosen.len() >= target {
                break;
            }
            if cand.slope_deg < min_slope || chosen_set[cand.idx] {
                continue;
            }
            let r = (cand.idx / w) as f64;
            let c = (cand.idx % w) as f64;
            let too_close = chosen.iter().any(|other| {
                let or = (other.idx / w) as f64;
                let oc = (other.idx % w) as f64;
                let dr = r - or;
                let dc = c - oc;
                dr * dr + dc * dc < min_dist_sq
            });
            if !too_close {
                chosen_set[cand.idx] = true;
                chosen.push(cand);
            }
        }
    }

    if chosen.len() < cfg.min_entrances {
        return Err(CaveError {
            message: format!(
                "insufficient eligible karst/basalt cave entrances: found {}, need >= {}",
                chosen.len(),
                cfg.min_entrances
            ),
        });
    }

    let mut cave_mask = vec![0_u8; n];
    let mut nodes: Vec<CaveNode> = Vec::new();
    let mut edges: Vec<CaveEdge> = Vec::new();
    let mut entrances: Vec<CaveEntrance> = Vec::new();
    let mut system_roots: Vec<(usize, u32, u8)> = Vec::new();

    for cand in chosen {
        let r = cand.idx / w;
        let c = cand.idx % w;
        let lith = input.lithology_id[cand.idx];
        let elev = input.total_h_m[cand.idx];
        let x0 = (c as f32 + 0.5) * cfg.dx_m;
        let z0 = (r as f32 + 0.5) * cfg.dx_m;

        // Outward downhill normal facing out of the cliff/rock face (`worldGen.ts:1893`).
        let yaw_rad = libm::atan2f(-cand.dh_dz, -cand.dh_dx);
        let grad_len = libm::hypotf(cand.dh_dx, cand.dh_dz);
        // March INTO the hillside (uphill direction).
        let (in_x, in_z) = if grad_len > 1e-4 {
            (cand.dh_dx / grad_len, cand.dh_dz / grad_len)
        } else {
            let th: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
            (libm::cosf(th), libm::sinf(th))
        };

        let mouth_kind = if lith == 3 {
            MouthKind::LavaTubeMouth
        } else if cand.slope_deg >= 18.0 {
            MouthKind::CliffPortal
        } else {
            MouthKind::KarstSinkhole
        };
        let passage_kind = if lith == 3 {
            PassageKind::BasaltLavaTube
        } else if mouth_kind == MouthKind::KarstSinkhole {
            PassageKind::KarstVadose
        } else {
            PassageKind::KarstPhreatic
        };

        let radius_0 = rng.gen_range(2.2_f32..4.2_f32);
        let clearance_m = (radius_0 * 1.45).clamp(2.5, 6.5);
        let ent_node_idx = nodes.len() as u32;
        nodes.push(CaveNode {
            x_m: x0,
            y_m: elev - 1.5,
            z_m: z0,
            radius_m: radius_0,
            light_decay: rng.gen_range(0.14_f32..0.28_f32),
        });
        entrances.push(CaveEntrance {
            node_idx: ent_node_idx,
            mouth_kind: mouth_kind as u8,
            realm_status: 1, // decorative_until_underground_adr
            yaw_rad,
            elevation_m: elev,
            clearance_m,
        });
        cave_mask[cand.idx] = 2;

        // March 4..=8 subterranean passage segments with occasional karst branching.
        let seg_count = rng.gen_range(4_usize..=8_usize);
        let mut prev_node = ent_node_idx;
        let mut cx = x0;
        let mut cz = z0;
        let mut dir_ang = libm::atan2f(in_z, in_x);
        let mut last_deep_node = ent_node_idx;

        for s in 1..=seg_count {
            dir_ang += rng.gen_range(-0.45_f32..0.45_f32);
            let step_m = rng.gen_range(25.0_f32..55.0_f32);
            cx = (cx + step_m * libm::cosf(dir_ang))
                .clamp(cfg.dx_m * 2.0, (w - 2) as f32 * cfg.dx_m);
            cz = (cz + step_m * libm::sinf(dir_ang))
                .clamp(cfg.dx_m * 2.0, (h - 2) as f32 * cfg.dx_m);
            let gc = (cx / cfg.dx_m).floor() as usize;
            let gr = (cz / cfg.dx_m).floor() as usize;
            let g_idx = gr.min(h - 1) * w + gc.min(w - 1);
            let surf_h = input.total_h_m[g_idx];
            let node_y = surf_h - 6.5 - (s as f32 * 2.5);

            let new_idx = nodes.len() as u32;
            nodes.push(CaveNode {
                x_m: cx,
                y_m: node_y,
                z_m: cz,
                radius_m: rng.gen_range(1.8_f32..4.0_f32),
                light_decay: rng.gen_range(0.18_f32..0.35_f32),
            });
            let p_kind = if s == 1 && mouth_kind == MouthKind::KarstSinkhole {
                PassageKind::SinkholeShaft as u8
            } else {
                passage_kind as u8
            };
            edges.push(CaveEdge {
                from_idx: prev_node,
                to_idx: new_idx,
                passage_kind: p_kind,
            });
            rasterize_passage_segment(
                &mut cave_mask,
                w,
                h,
                cfg.dx_m,
                nodes[prev_node as usize],
                nodes[new_idx as usize],
            );

            // Karst systems branch into side galleries (`Palmer 1991`).
            if lith == 2 && s == 3 {
                let br_ang = dir_ang + if rng.gen_bool(0.5) { 1.05 } else { -1.05 };
                let bx = (cx + 35.0 * libm::cosf(br_ang))
                    .clamp(cfg.dx_m * 2.0, (w - 2) as f32 * cfg.dx_m);
                let bz = (cz + 35.0 * libm::sinf(br_ang))
                    .clamp(cfg.dx_m * 2.0, (h - 2) as f32 * cfg.dx_m);
                let bg_idx = ((bz / cfg.dx_m).floor() as usize).min(h - 1) * w
                    + ((bx / cfg.dx_m).floor() as usize).min(w - 1);
                let br_idx = nodes.len() as u32;
                nodes.push(CaveNode {
                    x_m: bx,
                    y_m: input.total_h_m[bg_idx] - 12.0,
                    z_m: bz,
                    radius_m: rng.gen_range(1.6_f32..3.2_f32),
                    light_decay: 0.30,
                });
                edges.push(CaveEdge {
                    from_idx: new_idx,
                    to_idx: br_idx,
                    passage_kind: PassageKind::KarstPhreatic as u8,
                });
                rasterize_passage_segment(
                    &mut cave_mask,
                    w,
                    h,
                    cfg.dx_m,
                    nodes[new_idx as usize],
                    nodes[br_idx as usize],
                );
            }

            prev_node = new_idx;
            last_deep_node = new_idx;
        }
        system_roots.push((cand.idx, last_deep_node, lith));
    }

    // Connect nearby karst deep nodes (< 380 m apart) into multi-entrance cavern systems.
    for i in 0..system_roots.len() {
        if system_roots[i].2 != 2 {
            continue;
        }
        for j in (i + 1)..system_roots.len() {
            if system_roots[j].2 != 2 {
                continue;
            }
            let na = nodes[system_roots[i].1 as usize];
            let nb = nodes[system_roots[j].1 as usize];
            let d = libm::hypotf(na.x_m - nb.x_m, na.z_m - nb.z_m);
            if d < 380.0 {
                edges.push(CaveEdge {
                    from_idx: system_roots[i].1,
                    to_idx: system_roots[j].1,
                    passage_kind: PassageKind::KarstPhreatic as u8,
                });
                rasterize_passage_segment(&mut cave_mask, w, h, cfg.dx_m, na, nb);
                break;
            }
        }
    }

    Ok(CaveField {
        width: w,
        height: h,
        cave_mask,
        network: CaveNetwork {
            flags: 0,
            nodes,
            edges,
            entrances,
        },
    })
}

#[cfg(feature = "generate")]
fn rasterize_passage_segment(
    cave_mask: &mut [u8],
    w: usize,
    h: usize,
    dx_m: f32,
    a: CaveNode,
    b: CaveNode,
) {
    let dist_m = libm::hypotf(b.x_m - a.x_m, b.z_m - a.z_m);
    let steps = ((dist_m / (0.5 * dx_m)).ceil() as usize).max(1);
    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let x = a.x_m + t * (b.x_m - a.x_m);
        let z = a.z_m + t * (b.z_m - a.z_m);
        let c = (x / dx_m).floor() as isize;
        let r = (z / dx_m).floor() as isize;
        if r >= 0 && c >= 0 && r < h as isize && c < w as isize {
            let idx = r as usize * w + c as usize;
            if cave_mask[idx] == 0 {
                cave_mask[idx] = 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anmc_v1_roundtrip_is_exact_and_matches_byte_layout() {
        let net = CaveNetwork {
            flags: 0,
            nodes: vec![
                CaveNode {
                    x_m: 120.5,
                    y_m: 340.0,
                    z_m: 512.25,
                    radius_m: 3.0,
                    light_decay: 0.2,
                },
                CaveNode {
                    x_m: 155.0,
                    y_m: 325.0,
                    z_m: 540.0,
                    radius_m: 2.4,
                    light_decay: 0.3,
                },
            ],
            edges: vec![CaveEdge {
                from_idx: 0,
                to_idx: 1,
                passage_kind: PassageKind::KarstPhreatic as u8,
            }],
            entrances: vec![CaveEntrance {
                node_idx: 0,
                mouth_kind: MouthKind::CliffPortal as u8,
                realm_status: 1,
                yaw_rad: -0.75,
                elevation_m: 341.5,
                clearance_m: 4.2,
            }],
        };

        let bytes = net.to_bytes();
        assert_eq!(bytes.len(), 24 + 2 * 20 + 12 + 20);
        assert_eq!(&bytes[0..4], b"ANMC");
        assert_eq!(
            u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            1
        );
        assert_eq!(
            u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
            2
        );
        assert_eq!(
            u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
            1
        );
        assert_eq!(
            u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
            1
        );

        let decoded = CaveNetwork::from_bytes(&bytes).expect("decode ANMC");
        assert_eq!(decoded, net);
    }

    #[test]
    fn anmc_v1_rejects_corrupt_truncated_padded_oob_and_non_finite() {
        let valid = CaveNetwork {
            flags: 0,
            nodes: vec![CaveNode {
                x_m: 10.0,
                y_m: 20.0,
                z_m: 30.0,
                radius_m: 2.0,
                light_decay: 0.1,
            }],
            edges: vec![CaveEdge {
                from_idx: 0,
                to_idx: 0,
                passage_kind: 0,
            }],
            entrances: vec![CaveEntrance {
                node_idx: 0,
                mouth_kind: 0,
                realm_status: 1,
                yaw_rad: 0.0,
                elevation_m: 21.0,
                clearance_m: 3.0,
            }],
        }
        .to_bytes();

        // Truncated header & body.
        assert!(CaveNetwork::from_bytes(&valid[..23]).is_err());
        assert!(CaveNetwork::from_bytes(&valid[..valid.len() - 1]).is_err());

        // Trailing byte.
        let mut padded = valid.clone();
        padded.push(0);
        assert!(CaveNetwork::from_bytes(&padded).is_err());

        // Bad magic, version, flags.
        let mut bad_magic = valid.clone();
        bad_magic[0] = b'X';
        assert!(CaveNetwork::from_bytes(&bad_magic).is_err());

        let mut bad_ver = valid.clone();
        bad_ver[4..8].copy_from_slice(&2_u32.to_le_bytes());
        assert!(CaveNetwork::from_bytes(&bad_ver).is_err());

        let mut bad_flags = valid.clone();
        bad_flags[8..12].copy_from_slice(&1_u32.to_le_bytes());
        assert!(CaveNetwork::from_bytes(&bad_flags).is_err());

        // Overflow node_count without OOM allocation.
        let mut overflow = valid.clone();
        overflow[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(CaveNetwork::from_bytes(&overflow).is_err());

        // OOB edge index.
        let mut oob_edge = valid.clone();
        let edge_off = 24 + 20;
        oob_edge[edge_off..edge_off + 4].copy_from_slice(&99_u32.to_le_bytes());
        assert!(CaveNetwork::from_bytes(&oob_edge).is_err());

        // NaN node coordinate.
        let mut nan_node = valid.clone();
        nan_node[24..28].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(CaveNetwork::from_bytes(&nan_node).is_err());
    }

    #[cfg(feature = "generate")]
    #[test]
    fn caves_strictly_limited_to_karst_and_basalt_with_spacing_and_exclusions() {
        let n = 128;
        let dx_m = 10.0_f32;
        let mut total_h = vec![0.0_f32; n * n];
        let mut lith = vec![0_u8; n * n];
        let mut wk = vec![0_u8; n * n];
        let mut temp = vec![20.0_f32; n * n];
        let mut glacier = vec![0.0_f32; n * n];
        let mut dom = vec![BiomeId::Forest as u8; n * n];

        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                if c < 30 {
                    total_h[idx] = -50.0;
                    wk[idx] = WaterKind::Ocean as u8;
                    dom[idx] = BiomeId::Ocean as u8;
                    lith[idx] = 2; // even if lithology=2 in ocean, must be excluded!
                } else if c > 105 {
                    total_h[idx] = 5_100.0;
                    temp[idx] = -4.0;
                    glacier[idx] = 12.0;
                    dom[idx] = BiomeId::Glacier as u8;
                    lith[idx] = 3; // glacier peak must be excluded!
                } else {
                    // Alternating karst (2), basalt (3), and excluded granite/shale bands.
                    total_h[idx] =
                        250.0 + ((c - 30) as f32) * 28.0 + 40.0 * libm::sinf(r as f32 * 0.3);
                    lith[idx] = match (r / 12 + c / 16) % 4 {
                        0 => 2, // SolubleLimestone
                        1 => 3, // BasaltVolcanic
                        2 => 0, // Granite (excluded)
                        _ => 4, // Shale (excluded)
                    };
                }
            }
        }

        let input = CaveInput {
            width: n,
            height: n,
            total_h_m: &total_h,
            lithology_id: &lith,
            water_kind: &wk,
            temp_c: &temp,
            glacier_h_m: &glacier,
            dominant_biome: &dom,
        };
        let cfg = CaveConfig {
            dx_m,
            ..CaveConfig::default()
        };
        let field = generate_caves(&input, &cfg).expect("caves");

        let count = field.network.entrances.len();
        assert!(
            (30..=48).contains(&count),
            "expected 30..=48 entrances, got {count}"
        );
        assert!(field.cave_mask.iter().filter(|&&m| m == 2).count() == count);
        assert!(
            field.cave_mask.contains(&1),
            "expected subterranean passage mask cells"
        );

        let min_spacing = 0.01 * n as f64;
        for (i, e1) in field.network.entrances.iter().enumerate() {
            assert_eq!(e1.realm_status, 1);
            let n1 = field.network.nodes[e1.node_idx as usize];
            let gc = (n1.x_m / dx_m).floor() as usize;
            let gr = (n1.z_m / dx_m).floor() as usize;
            let idx = gr * n + gc;
            assert!(lith[idx] == 2 || lith[idx] == 3);
            assert!(total_h[idx] > 8.0);
            assert_eq!(glacier[idx], 0.0);
            assert!(temp[idx] > 0.5);
            assert_eq!(wk[idx], WaterKind::None as u8);

            for e2 in &field.network.entrances[(i + 1)..] {
                let n2 = field.network.nodes[e2.node_idx as usize];
                let gc2 = (n2.x_m / dx_m).floor() as f64;
                let gr2 = (n2.z_m / dx_m).floor() as f64;
                let d = libm::hypot(gc as f64 - gc2, gr as f64 - gr2);
                assert!(d >= min_spacing, "spacing {d} < {min_spacing}");
            }
        }

        let roundtrip = CaveNetwork::from_bytes(&field.network.to_bytes()).expect("roundtrip");
        assert_eq!(roundtrip, field.network);
    }

    #[cfg(feature = "generate")]
    #[test]
    fn caves_rejects_all_ocean_or_granite_grid() {
        let n = 64;
        let total_h = vec![500.0_f32; n * n];
        let lith = vec![0_u8; n * n]; // all granite -> zero karst/basalt!
        let wk = vec![0_u8; n * n];
        let temp = vec![20.0_f32; n * n];
        let glacier = vec![0.0_f32; n * n];
        let dom = vec![BiomeId::Rock as u8; n * n];

        let input = CaveInput {
            width: n,
            height: n,
            total_h_m: &total_h,
            lithology_id: &lith,
            water_kind: &wk,
            temp_c: &temp,
            glacier_h_m: &glacier,
            dominant_biome: &dom,
        };
        let cfg = CaveConfig::default();
        assert!(generate_caves(&input, &cfg).is_err());
    }
}
