/**
 * Browser-safe binary codecs for World v2 (`ANMW v3`, `ANMC v1`, `ANMF v1`),
 * WebGL2 tile capability/fallback packing (`EXT_texture_norm16` -> `RG8`, `BC5` -> `RGBA8`),
 * and pure `world_v2_manifest.json` validation (`WV2-6`).
 *
 * Byte layouts match `src-tauri/crates/anima-worldgen/src/{anmw_v3.rs,caves.rs,bake/flora_place.rs}`.
 */

export const ANMW_MAGIC = [0x41, 0x4e, 0x4d, 0x57] as const; // "ANMW"
export const ANMW_V3_VERSION = 3;
export const ANMW_V3_HEADER_LEN = 44;
export const ANMW_V3_WATERFALL_LEN = 32;
export const ANMW_V3_CAVE_ENTRANCE_LEN = 24;
export const ANMW_V3_MAX_RECORD_COUNT = 65_536;
export const ANMW_V3_WORLD_SCALE = 200.0;

export const ANMC_MAGIC = [0x41, 0x4e, 0x4d, 0x43] as const; // "ANMC"
export const ANMC_VERSION = 1;
export const ANMC_HEADER_LEN = 24;
export const ANMC_NODE_LEN = 20;
export const ANMC_EDGE_LEN = 12;
export const ANMC_ENTRANCE_LEN = 20;

export const ANMF_MAGIC = [0x41, 0x4e, 0x4d, 0x46] as const; // "ANMF"
export const ANMF_VERSION = 1;
export const ANMF_HEADER_LEN = 16;
export const ANMF_INSTANCE_LEN = 8;
export const ANMF_FLAG_DECORATIVE = 0x01;

export const WV2_ELEV_MIN_M = -1500.0;
export const WV2_ELEV_MAX_M = 5700.0;
export const WV2_ELEV_SPAN_M = WV2_ELEV_MAX_M - WV2_ELEV_MIN_M; // 7200.0
export const WV2_SEA_LEVEL_M = 0.0;
export const WV2_SEA_LEVEL_NORM = (WV2_SEA_LEVEL_M - WV2_ELEV_MIN_M) / WV2_ELEV_SPAN_M;

export interface AnmwV3Waterfall {
  x_m: number;
  z_m: number;
  lip_elev_m: number;
  drop_m: number;
  yaw_rad: number;
  discharge_q_m3_s: number;
  plunge_depth_m: number;
  flags: number;
}

export interface AnmwV3CaveEntrance {
  x_m: number;
  z_m: number;
  elevation_m: number;
  yaw_rad: number;
  clearance_m: number;
  mouth_kind: number;
  realm_status: number;
}

export interface AnmwV3Sim {
  width: number;
  height: number;
  seaLevelNorm: number;
  simSeedU32: number;
  seedU64: bigint;
  generatorVersion: number;
  worldScale: number;
  flags: number;
  elevation: Float32Array;
  moisture: Float32Array;
  temperature: Float32Array;
  flow: Float32Array;
  slope: Float32Array;
  biome22: Uint8Array;
  waterKind: Uint8Array;
  waterfalls: AnmwV3Waterfall[];
  caveEntrances: AnmwV3CaveEntrance[];
}

export interface CaveNode {
  x_m: number;
  y_m: number;
  z_m: number;
  radius_m: number;
  light_decay: number;
}

export interface CaveEdge {
  from_idx: number;
  to_idx: number;
  passage_kind: number;
}

export interface CaveEntrance {
  node_idx: number;
  mouth_kind: number;
  realm_status: number;
  yaw_rad: number;
  elevation_m: number;
  clearance_m: number;
}

export interface CaveNetwork {
  flags: number;
  nodes: CaveNode[];
  edges: CaveEdge[];
  entrances: CaveEntrance[];
}

export interface FloraInstance {
  x_local_u16: number;
  z_local_u16: number;
  species_id: number;
  scale_u8: number;
  yaw_u8: number;
  flags_u8: number;
}

export interface FloraSectorBlob {
  sectorX: number;
  sectorZ: number;
  decorativeOnly: boolean;
  instances: FloraInstance[];
}

export interface WebGl2TileCapabilities {
  hasNorm16: boolean;
  hasRgtc: boolean;
  hasBptc: boolean;
  hasAstc: boolean;
  hasEtc2: boolean;
  heightEncoding: 'r16' | 'rg8';
  normalEncoding: 'bc5' | 'rgba8';
}

export function deriveSimSeedU32(seedU64: bigint): number {
  const lo = Number(seedU64 & 0xffff_ffffn);
  const hi = Number((seedU64 >> 32n) & 0xffff_ffffn);
  return (lo ^ hi) >>> 0;
}

export function peekAnmwV3Seed(bytes: Uint8Array): number {
  if (bytes.byteLength < ANMW_V3_HEADER_LEN) {
    throw new Error(`ANMW v3 buffer shorter than ${ANMW_V3_HEADER_LEN}-byte header`);
  }
  for (let i = 0; i < 4; i += 1) {
    if (bytes[i] !== ANMW_MAGIC[i]) {
      throw new Error('bad ANMW v3 magic (expected ANMW)');
    }
  }
  const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const version = dv.getUint32(4, true);
  if (version !== ANMW_V3_VERSION) {
    throw new Error(`unsupported ANMW version ${version} (expected ${ANMW_V3_VERSION})`);
  }
  const seedU64 = dv.getBigUint64(24, true);
  const simSeedU32 = dv.getUint32(20, true);
  const expected = deriveSimSeedU32(seedU64);
  if (simSeedU32 !== expected) {
    throw new Error(`ANMW v3 sim_seed_u32 (${simSeedU32}) does not match seed_u64 fold (${expected})`);
  }
  return simSeedU32;
}

export function decodeAnmwV3Sim(bytes: Uint8Array): AnmwV3Sim {
  const simSeedU32 = peekAnmwV3Seed(bytes);
  const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const width = dv.getUint32(8, true);
  const height = dv.getUint32(12, true);
  const seaLevelNorm = dv.getFloat32(16, true);
  const seedU64 = dv.getBigUint64(24, true);
  const generatorVersion = dv.getUint32(32, true);
  const worldScale = dv.getFloat32(36, true);
  const flags = dv.getUint32(40, true);

  if (width === 0 || height === 0) {
    throw new Error('ANMW v3 has zero grid dimensions');
  }
  if (!Number.isFinite(seaLevelNorm) || !Number.isFinite(worldScale) || worldScale <= 0) {
    throw new Error('ANMW v3 header has invalid float metadata');
  }
  if (flags !== 0) {
    throw new Error(`unsupported ANMW v3 flags ${flags}`);
  }

  const n = width * height;
  const gridEnd = ANMW_V3_HEADER_LEN + n * (5 * 4 + 2);
  if (bytes.byteLength < gridEnd + 8) {
    throw new Error(`ANMW v3 payload truncated: expected >= ${gridEnd + 8}, got ${bytes.byteLength}`);
  }

  const readF32Grid = (offset: number, nonNeg: boolean, name: string): Float32Array => {
    const out = new Float32Array(n);
    for (let i = 0; i < n; i += 1) {
      const v = dv.getFloat32(offset + i * 4, true);
      if (!Number.isFinite(v) || (nonNeg && v < 0)) {
        throw new Error(`ANMW v3 invalid ${name}[${i}] = ${v}`);
      }
      out[i] = v;
    }
    return out;
  };

  let cursor = ANMW_V3_HEADER_LEN;
  const elevation = readF32Grid(cursor, false, 'elevation');
  cursor += n * 4;
  const moisture = readF32Grid(cursor, false, 'moisture');
  cursor += n * 4;
  const temperature = readF32Grid(cursor, false, 'temperature');
  cursor += n * 4;
  const flow = readF32Grid(cursor, true, 'flow');
  cursor += n * 4;
  const slope = readF32Grid(cursor, true, 'slope');
  cursor += n * 4;

  const biome22 = bytes.slice(cursor, cursor + n);
  cursor += n;
  const waterKind = bytes.slice(cursor, cursor + n);
  cursor += n;

  for (let i = 0; i < n; i += 1) {
    if (biome22[i] >= 22) {
      throw new Error(`ANMW v3 invalid biome_22[${i}] = ${biome22[i]}`);
    }
    if (waterKind[i] >= 7) {
      throw new Error(`ANMW v3 invalid water_kind[${i}] = ${waterKind[i]}`);
    }
  }

  const wfCount = dv.getUint32(cursor, true);
  cursor += 4;
  if (wfCount > ANMW_V3_MAX_RECORD_COUNT) {
    throw new Error(`ANMW v3 waterfall count ${wfCount} exceeds limit`);
  }
  const wfEnd = cursor + wfCount * ANMW_V3_WATERFALL_LEN + 4;
  if (bytes.byteLength < wfEnd) {
    throw new Error(`ANMW v3 truncated before cave count: expected >= ${wfEnd}, got ${bytes.byteLength}`);
  }

  const waterfalls: AnmwV3Waterfall[] = [];
  for (let i = 0; i < wfCount; i += 1) {
    const wf: AnmwV3Waterfall = {
      x_m: dv.getFloat32(cursor, true),
      z_m: dv.getFloat32(cursor + 4, true),
      lip_elev_m: dv.getFloat32(cursor + 8, true),
      drop_m: dv.getFloat32(cursor + 12, true),
      yaw_rad: dv.getFloat32(cursor + 16, true),
      discharge_q_m3_s: dv.getFloat32(cursor + 20, true),
      plunge_depth_m: dv.getFloat32(cursor + 24, true),
      flags: dv.getUint32(cursor + 28, true),
    };
    if (
      !Number.isFinite(wf.x_m) ||
      !Number.isFinite(wf.z_m) ||
      !Number.isFinite(wf.lip_elev_m) ||
      !Number.isFinite(wf.drop_m) ||
      !Number.isFinite(wf.yaw_rad) ||
      !Number.isFinite(wf.discharge_q_m3_s) ||
      !Number.isFinite(wf.plunge_depth_m) ||
      wf.drop_m < 0 ||
      wf.discharge_q_m3_s < 0 ||
      wf.plunge_depth_m < 0
    ) {
      throw new Error(`ANMW v3 invalid waterfall[${i}]`);
    }
    waterfalls.push(wf);
    cursor += ANMW_V3_WATERFALL_LEN;
  }

  const caveCount = dv.getUint32(cursor, true);
  cursor += 4;
  if (caveCount > ANMW_V3_MAX_RECORD_COUNT) {
    throw new Error(`ANMW v3 cave count ${caveCount} exceeds limit`);
  }
  const expectedLen = cursor + caveCount * ANMW_V3_CAVE_ENTRANCE_LEN;
  if (bytes.byteLength !== expectedLen) {
    throw new Error(`ANMW v3 byte length mismatch: expected ${expectedLen}, got ${bytes.byteLength}`);
  }

  const caveEntrances: AnmwV3CaveEntrance[] = [];
  for (let i = 0; i < caveCount; i += 1) {
    if (bytes[cursor + 22] !== 0 || bytes[cursor + 23] !== 0) {
      throw new Error('ANMW v3 non-zero reserved cave entrance bytes');
    }
    const ent: AnmwV3CaveEntrance = {
      x_m: dv.getFloat32(cursor, true),
      z_m: dv.getFloat32(cursor + 4, true),
      elevation_m: dv.getFloat32(cursor + 8, true),
      yaw_rad: dv.getFloat32(cursor + 12, true),
      clearance_m: dv.getFloat32(cursor + 16, true),
      mouth_kind: bytes[cursor + 20],
      realm_status: bytes[cursor + 21],
    };
    if (
      !Number.isFinite(ent.x_m) ||
      !Number.isFinite(ent.z_m) ||
      !Number.isFinite(ent.elevation_m) ||
      !Number.isFinite(ent.yaw_rad) ||
      !Number.isFinite(ent.clearance_m) ||
      ent.clearance_m <= 0 ||
      ent.mouth_kind >= 3
    ) {
      throw new Error(`ANMW v3 invalid cave_entrance[${i}]`);
    }
    caveEntrances.push(ent);
    cursor += ANMW_V3_CAVE_ENTRANCE_LEN;
  }

  return {
    width,
    height,
    seaLevelNorm,
    simSeedU32,
    seedU64,
    generatorVersion,
    worldScale,
    flags,
    elevation,
    moisture,
    temperature,
    flow,
    slope,
    biome22,
    waterKind,
    waterfalls,
    caveEntrances,
  };
}

export function encodeAnmwV3Sim(sim: AnmwV3Sim): Uint8Array {
  const n = sim.width * sim.height;
  const totalLen =
    ANMW_V3_HEADER_LEN +
    n * (5 * 4 + 2) +
    4 +
    sim.waterfalls.length * ANMW_V3_WATERFALL_LEN +
    4 +
    sim.caveEntrances.length * ANMW_V3_CAVE_ENTRANCE_LEN;
  const out = new Uint8Array(totalLen);
  const dv = new DataView(out.buffer);
  out.set(ANMW_MAGIC, 0);
  dv.setUint32(4, ANMW_V3_VERSION, true);
  dv.setUint32(8, sim.width, true);
  dv.setUint32(12, sim.height, true);
  dv.setFloat32(16, sim.seaLevelNorm, true);
  dv.setUint32(20, sim.simSeedU32, true);
  dv.setBigUint64(24, sim.seedU64, true);
  dv.setUint32(32, sim.generatorVersion, true);
  dv.setFloat32(36, sim.worldScale, true);
  dv.setUint32(40, sim.flags, true);

  let cursor = ANMW_V3_HEADER_LEN;
  for (const field of [sim.elevation, sim.moisture, sim.temperature, sim.flow, sim.slope]) {
    for (let i = 0; i < n; i += 1) {
      dv.setFloat32(cursor + i * 4, field[i], true);
    }
    cursor += n * 4;
  }
  out.set(sim.biome22.subarray(0, n), cursor);
  cursor += n;
  out.set(sim.waterKind.subarray(0, n), cursor);
  cursor += n;

  dv.setUint32(cursor, sim.waterfalls.length, true);
  cursor += 4;
  for (const wf of sim.waterfalls) {
    dv.setFloat32(cursor, wf.x_m, true);
    dv.setFloat32(cursor + 4, wf.z_m, true);
    dv.setFloat32(cursor + 8, wf.lip_elev_m, true);
    dv.setFloat32(cursor + 12, wf.drop_m, true);
    dv.setFloat32(cursor + 16, wf.yaw_rad, true);
    dv.setFloat32(cursor + 20, wf.discharge_q_m3_s, true);
    dv.setFloat32(cursor + 24, wf.plunge_depth_m, true);
    dv.setUint32(cursor + 28, wf.flags, true);
    cursor += ANMW_V3_WATERFALL_LEN;
  }

  dv.setUint32(cursor, sim.caveEntrances.length, true);
  cursor += 4;
  for (const ent of sim.caveEntrances) {
    dv.setFloat32(cursor, ent.x_m, true);
    dv.setFloat32(cursor + 4, ent.z_m, true);
    dv.setFloat32(cursor + 8, ent.elevation_m, true);
    dv.setFloat32(cursor + 12, ent.yaw_rad, true);
    dv.setFloat32(cursor + 16, ent.clearance_m, true);
    out[cursor + 20] = ent.mouth_kind;
    out[cursor + 21] = ent.realm_status;
    out[cursor + 22] = 0;
    out[cursor + 23] = 0;
    cursor += ANMW_V3_CAVE_ENTRANCE_LEN;
  }
  return out;
}

export function decodeCaveNetworkBin(bytes: Uint8Array): CaveNetwork {
  if (bytes.byteLength < ANMC_HEADER_LEN) {
    throw new Error('ANMC buffer shorter than 24-byte header');
  }
  for (let i = 0; i < 4; i += 1) {
    if (bytes[i] !== ANMC_MAGIC[i]) {
      throw new Error('invalid ANMC magic bytes');
    }
  }
  const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const version = dv.getUint32(4, true);
  if (version !== ANMC_VERSION) {
    throw new Error(`unsupported ANMC version ${version}`);
  }
  const flags = dv.getUint32(8, true);
  const nodeCount = dv.getUint32(12, true);
  const edgeCount = dv.getUint32(16, true);
  const entranceCount = dv.getUint32(20, true);
  const expected =
    ANMC_HEADER_LEN +
    nodeCount * ANMC_NODE_LEN +
    edgeCount * ANMC_EDGE_LEN +
    entranceCount * ANMC_ENTRANCE_LEN;
  if (bytes.byteLength !== expected) {
    throw new Error(`ANMC byte length mismatch: expected ${expected}, got ${bytes.byteLength}`);
  }

  let cursor = ANMC_HEADER_LEN;
  const nodes: CaveNode[] = [];
  for (let i = 0; i < nodeCount; i += 1) {
    const node: CaveNode = {
      x_m: dv.getFloat32(cursor, true),
      y_m: dv.getFloat32(cursor + 4, true),
      z_m: dv.getFloat32(cursor + 8, true),
      radius_m: dv.getFloat32(cursor + 12, true),
      light_decay: dv.getFloat32(cursor + 16, true),
    };
    if (
      !Number.isFinite(node.x_m) ||
      !Number.isFinite(node.y_m) ||
      !Number.isFinite(node.z_m) ||
      !Number.isFinite(node.radius_m) ||
      node.radius_m <= 0 ||
      !Number.isFinite(node.light_decay)
    ) {
      throw new Error(`ANMC invalid node[${i}]`);
    }
    nodes.push(node);
    cursor += ANMC_NODE_LEN;
  }

  const edges: CaveEdge[] = [];
  for (let i = 0; i < edgeCount; i += 1) {
    const from_idx = dv.getUint32(cursor, true);
    const to_idx = dv.getUint32(cursor + 4, true);
    const passage_kind = bytes[cursor + 8];
    if (bytes[cursor + 9] !== 0 || bytes[cursor + 10] !== 0 || bytes[cursor + 11] !== 0) {
      throw new Error('ANMC non-zero edge reserved padding');
    }
    if (from_idx >= nodeCount || to_idx >= nodeCount || passage_kind >= 4) {
      throw new Error(`ANMC invalid edge[${i}]`);
    }
    edges.push({ from_idx, to_idx, passage_kind });
    cursor += ANMC_EDGE_LEN;
  }

  const entrances: CaveEntrance[] = [];
  for (let i = 0; i < entranceCount; i += 1) {
    const node_idx = dv.getUint32(cursor, true);
    const mouth_kind = bytes[cursor + 4];
    const realm_status = bytes[cursor + 5];
    if (bytes[cursor + 6] !== 0 || bytes[cursor + 7] !== 0) {
      throw new Error('ANMC non-zero entrance reserved padding');
    }
    const yaw_rad = dv.getFloat32(cursor + 8, true);
    const elevation_m = dv.getFloat32(cursor + 12, true);
    const clearance_m = dv.getFloat32(cursor + 16, true);
    if (
      node_idx >= nodeCount ||
      mouth_kind >= 3 ||
      !Number.isFinite(yaw_rad) ||
      !Number.isFinite(elevation_m) ||
      !Number.isFinite(clearance_m) ||
      clearance_m <= 0
    ) {
      throw new Error(`ANMC invalid entrance[${i}]`);
    }
    entrances.push({ node_idx, mouth_kind, realm_status, yaw_rad, elevation_m, clearance_m });
    cursor += ANMC_ENTRANCE_LEN;
  }

  return { flags, nodes, edges, entrances };
}

export function encodeCaveNetworkBin(net: CaveNetwork): Uint8Array {
  const total =
    ANMC_HEADER_LEN +
    net.nodes.length * ANMC_NODE_LEN +
    net.edges.length * ANMC_EDGE_LEN +
    net.entrances.length * ANMC_ENTRANCE_LEN;
  const out = new Uint8Array(total);
  const dv = new DataView(out.buffer);
  out.set(ANMC_MAGIC, 0);
  dv.setUint32(4, ANMC_VERSION, true);
  dv.setUint32(8, net.flags, true);
  dv.setUint32(12, net.nodes.length, true);
  dv.setUint32(16, net.edges.length, true);
  dv.setUint32(20, net.entrances.length, true);

  let cursor = ANMC_HEADER_LEN;
  for (const n of net.nodes) {
    dv.setFloat32(cursor, n.x_m, true);
    dv.setFloat32(cursor + 4, n.y_m, true);
    dv.setFloat32(cursor + 8, n.z_m, true);
    dv.setFloat32(cursor + 12, n.radius_m, true);
    dv.setFloat32(cursor + 16, n.light_decay, true);
    cursor += ANMC_NODE_LEN;
  }
  for (const e of net.edges) {
    dv.setUint32(cursor, e.from_idx, true);
    dv.setUint32(cursor + 4, e.to_idx, true);
    out[cursor + 8] = e.passage_kind;
    cursor += ANMC_EDGE_LEN;
  }
  for (const ent of net.entrances) {
    dv.setUint32(cursor, ent.node_idx, true);
    out[cursor + 4] = ent.mouth_kind;
    out[cursor + 5] = ent.realm_status;
    dv.setFloat32(cursor + 8, ent.yaw_rad, true);
    dv.setFloat32(cursor + 12, ent.elevation_m, true);
    dv.setFloat32(cursor + 16, ent.clearance_m, true);
    cursor += ANMC_ENTRANCE_LEN;
  }
  return out;
}

export function decodeFloraSectorAnmf(bytes: Uint8Array): FloraSectorBlob {
  if (bytes.byteLength < ANMF_HEADER_LEN) {
    throw new Error('ANMF buffer shorter than 16-byte header');
  }
  for (let i = 0; i < 4; i += 1) {
    if (bytes[i] !== ANMF_MAGIC[i]) {
      throw new Error('invalid ANMF magic bytes');
    }
  }
  const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const ver = dv.getUint32(4, true);
  if (ver !== ANMF_VERSION) {
    throw new Error(`unsupported ANMF version ${ver}`);
  }
  const sectorX = dv.getUint16(8, true);
  const sectorZ = dv.getUint16(10, true);
  const count = dv.getUint32(12, true);
  const expected = ANMF_HEADER_LEN + count * ANMF_INSTANCE_LEN;
  if (bytes.byteLength !== expected) {
    throw new Error(`ANMF byte length mismatch: expected ${expected}, got ${bytes.byteLength}`);
  }
  const instances: FloraInstance[] = [];
  let cursor = ANMF_HEADER_LEN;
  for (let i = 0; i < count; i += 1) {
    const flags_u8 = bytes[cursor + 7];
    if ((flags_u8 & ANMF_FLAG_DECORATIVE) === 0) {
      throw new Error(`ANMF instance[${i}] missing FLORA_FLAG_DECORATIVE bit`);
    }
    instances.push({
      x_local_u16: dv.getUint16(cursor, true),
      z_local_u16: dv.getUint16(cursor + 2, true),
      species_id: bytes[cursor + 4],
      scale_u8: bytes[cursor + 5],
      yaw_u8: bytes[cursor + 6],
      flags_u8,
    });
    cursor += ANMF_INSTANCE_LEN;
  }
  return { sectorX, sectorZ, decorativeOnly: true, instances };
}

export function encodeFloraSectorAnmf(sector: FloraSectorBlob): Uint8Array {
  if (!sector.decorativeOnly) {
    throw new Error('flora_tiles must remain decorativeOnly=true (ADR-0010 §4)');
  }
  const out = new Uint8Array(ANMF_HEADER_LEN + sector.instances.length * ANMF_INSTANCE_LEN);
  const dv = new DataView(out.buffer);
  out.set(ANMF_MAGIC, 0);
  dv.setUint32(4, ANMF_VERSION, true);
  dv.setUint16(8, sector.sectorX, true);
  dv.setUint16(10, sector.sectorZ, true);
  dv.setUint32(12, sector.instances.length, true);
  let cursor = ANMF_HEADER_LEN;
  for (const inst of sector.instances) {
    if ((inst.flags_u8 & ANMF_FLAG_DECORATIVE) === 0) {
      throw new Error('FloraInstance missing FLORA_FLAG_DECORATIVE bit');
    }
    dv.setUint16(cursor, inst.x_local_u16, true);
    dv.setUint16(cursor + 2, inst.z_local_u16, true);
    out[cursor + 4] = inst.species_id;
    out[cursor + 5] = inst.scale_u8;
    out[cursor + 6] = inst.yaw_u8;
    out[cursor + 7] = inst.flags_u8;
    cursor += ANMF_INSTANCE_LEN;
  }
  return out;
}

export function detectWebGl2TileCapabilities(
  gl: Pick<WebGL2RenderingContext, 'getExtension'> | null | undefined,
): WebGl2TileCapabilities {
  const has = (name: string): boolean => {
    try {
      return Boolean(gl && gl.getExtension(name));
    } catch {
      return false;
    }
  };
  const hasNorm16 = has('EXT_texture_norm16');
  const hasRgtc = has('EXT_texture_compression_rgtc');
  const hasBptc = has('EXT_texture_compression_bptc');
  const hasAstc = has('WEBGL_compressed_texture_astc');
  const hasEtc2 = has('WEBGL_compressed_texture_etc');
  return {
    hasNorm16,
    hasRgtc,
    hasBptc,
    hasAstc,
    hasEtc2,
    heightEncoding: hasNorm16 ? 'r16' : 'rg8',
    normalEncoding: hasRgtc ? 'bc5' : 'rgba8',
  };
}

/**
 * Lossless packing of `R16_UNORM` little-endian texels into `RG8_UNORM` (`[hi, lo]`)
 * for WebGL2 contexts without `EXT_texture_norm16` (`§3.2`: `(r*255 + g/256) / 255`, 0-ULP parity).
 */
export function packR16ToRg8Unorm(rawR16Bytes: Uint8Array, width: number, height: number): Uint8Array {
  const count = width * height;
  if (rawR16Bytes.byteLength !== count * 2) {
    throw new Error(`packR16ToRg8Unorm size mismatch: expected ${count * 2}, got ${rawR16Bytes.byteLength}`);
  }
  const dv = new DataView(rawR16Bytes.buffer, rawR16Bytes.byteOffset, rawR16Bytes.byteLength);
  const rg8 = new Uint8Array(count * 2);
  for (let i = 0; i < count; i += 1) {
    const q = dv.getUint16(i * 2, true);
    rg8[i * 2] = (q >>> 8) & 0xff;
    rg8[i * 2 + 1] = q & 0xff;
  }
  return rg8;
}

export function unpackRg8HeightM(rg8: Uint8Array, texelIndex: number): number {
  const hi = rg8[texelIndex * 2];
  const lo = rg8[texelIndex * 2 + 1];
  const q = (hi << 8) | lo;
  return WV2_ELEV_MIN_M + (q / 65535.0) * WV2_ELEV_SPAN_M;
}

/**
 * Converts software-decoded `normal_xy` (`[nx_u8, nz_u8]` pairs) into uncompressed `RGBA8`
 * tangent-space normals `[nx, ny_up, nz, 255]` when `EXT_texture_compression_rgtc` is absent.
 */
export function expandNormalRgToRgba8(normalRg: Uint8Array, texelCount: number): Uint8Array {
  if (normalRg.byteLength !== texelCount * 2) {
    throw new Error(`expandNormalRgToRgba8 mismatch: expected ${texelCount * 2}, got ${normalRg.byteLength}`);
  }
  const rgba = new Uint8Array(texelCount * 4);
  for (let i = 0; i < texelCount; i += 1) {
    const rx = normalRg[i * 2];
    const rz = normalRg[i * 2 + 1];
    const nx = (rx / 255.0) * 2.0 - 1.0;
    const nz = (rz / 255.0) * 2.0 - 1.0;
    const ny = Math.sqrt(Math.max(0.0, 1.0 - nx * nx - nz * nz));
    const o = i * 4;
    rgba[o] = rx;
    rgba[o + 1] = Math.min(255, Math.max(0, Math.round((ny * 0.5 + 0.5) * 255)));
    rgba[o + 2] = rz;
    rgba[o + 3] = 255;
  }
  return rgba;
}

export interface WorldV2Manifest {
  $schema?: string;
  schemaVersion: number;
  _generated: {
    by: string;
    seed: number;
    schemaTag: string;
    gridSize: number;
    extentM: number;
    generatorVersion: number;
  };
  identities: {
    simWorldIdentity: string;
    bundleIdentity: string;
    releaseArchive: string;
    releaseArchiveSha256: string;
    gatesSha256: string;
    presetSha256: string;
  };
  simBundle: {
    path: string;
    bytes: number;
    width: number;
    height: number;
    seaLevelNorm: number;
    simSeedU32: number;
    waterfallCount: number;
    caveEntranceCount: number;
  };
  indexSummary: {
    path: string;
    bytes: number;
    tileEntryCount: number;
  };
  coordinateContract: {
    extentM: number;
    elevMinM: number;
    elevMaxM: number;
    worldMinXZ: number;
    worldMaxXZ: number;
    worldMinY: number;
    worldMaxY: number;
    simGridDim: number;
  };
  ownerGates: {
    publicTileRelease: string;
    irisXeBeforeAfterFps: string;
  };
}

export interface WorldV2ActualEvidence {
  sim256Sha256: string;
  sim256Bytes: number;
  indexSha256: string;
  indexBytes: number;
  gatesSha256: string;
  presetSha256: string;
  releaseArchiveSha256: string;
  tileEntryCount: number;
  decodedSim: Pick<
    AnmwV3Sim,
    'width' | 'height' | 'seaLevelNorm' | 'simSeedU32' | 'generatorVersion'
  > & { waterfallCount: number; caveEntranceCount: number };
}

export function validateWorldV2Manifest(
  manifest: unknown,
  actual: WorldV2ActualEvidence,
): { ok: boolean; errors: string[] } {
  const errors: string[] = [];
  if (!manifest || typeof manifest !== 'object') {
    return { ok: false, errors: ['manifest must be an object'] };
  }
  const m = manifest as Partial<WorldV2Manifest>;
  if (m.schemaVersion !== 1) errors.push(`schemaVersion must be 1 (got ${String(m.schemaVersion)})`);

  const id = m.identities;
  if (!id) {
    errors.push('missing identities block');
  } else {
    if (id.simWorldIdentity !== actual.sim256Sha256) {
      errors.push(`simWorldIdentity mismatch: ${id.simWorldIdentity} != ${actual.sim256Sha256}`);
    }
    if (id.bundleIdentity !== actual.indexSha256) {
      errors.push(`bundleIdentity mismatch: ${id.bundleIdentity} != ${actual.indexSha256}`);
    }
    if (id.gatesSha256 !== actual.gatesSha256) {
      errors.push(`gatesSha256 mismatch: ${id.gatesSha256} != ${actual.gatesSha256}`);
    }
    if (id.presetSha256 !== actual.presetSha256) {
      errors.push(`presetSha256 mismatch: ${id.presetSha256} != ${actual.presetSha256}`);
    }
    if (id.releaseArchiveSha256 !== actual.releaseArchiveSha256) {
      errors.push(`releaseArchiveSha256 mismatch: ${id.releaseArchiveSha256} != ${actual.releaseArchiveSha256}`);
    }
  }

  const sb = m.simBundle;
  if (!sb) {
    errors.push('missing simBundle block');
  } else {
    if (sb.bytes !== actual.sim256Bytes) {
      errors.push(`simBundle.bytes mismatch: ${sb.bytes} != ${actual.sim256Bytes}`);
    }
    if (sb.width !== actual.decodedSim.width || sb.height !== actual.decodedSim.height) {
      errors.push('simBundle dimensions mismatch');
    }
    if (sb.simSeedU32 !== actual.decodedSim.simSeedU32) {
      errors.push(`simSeedU32 mismatch: ${sb.simSeedU32} != ${actual.decodedSim.simSeedU32}`);
    }
    if (sb.waterfallCount !== actual.decodedSim.waterfallCount) {
      errors.push(`waterfallCount mismatch: ${sb.waterfallCount} != ${actual.decodedSim.waterfallCount}`);
    }
    if (sb.caveEntranceCount !== actual.decodedSim.caveEntranceCount) {
      errors.push(`caveEntranceCount mismatch: ${sb.caveEntranceCount} != ${actual.decodedSim.caveEntranceCount}`);
    }
  }

  const idx = m.indexSummary;
  if (!idx) {
    errors.push('missing indexSummary block');
  } else {
    if (idx.bytes !== actual.indexBytes) {
      errors.push(`indexSummary.bytes mismatch: ${idx.bytes} != ${actual.indexBytes}`);
    }
    if (idx.tileEntryCount !== actual.tileEntryCount) {
      errors.push(`tileEntryCount mismatch: ${idx.tileEntryCount} != ${actual.tileEntryCount}`);
    }
  }

  const cc = m.coordinateContract;
  if (!cc || cc.simGridDim !== 256 || cc.worldMinXZ !== -100 || cc.worldMaxXZ !== 100) {
    errors.push('coordinateContract mismatch against backend MapBounds');
  }

  return { ok: errors.length === 0, errors };
}
