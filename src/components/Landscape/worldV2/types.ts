import type { QualityTier } from '../utils/qualityTiers';
import type { AnmwV3Sim, CaveNetwork, FloraSectorBlob } from '../utils/worldArtifactV3';
import {
  WV2_ELEV_MAX_M,
  WV2_ELEV_MIN_M,
  WV2_ELEV_SPAN_M,
  WV2_SEA_LEVEL_M,
  WV2_SEA_LEVEL_NORM,
} from '../utils/worldArtifactV3';

export const TILE_ACTIVE_SIZE = 256;
export const TILE_GUTTER = 2;
export const TILE_TOTAL_SIZE = TILE_ACTIVE_SIZE + 2 * TILE_GUTTER; // 260

export const ELEV_MIN_M = WV2_ELEV_MIN_M;
export const ELEV_MAX_M = WV2_ELEV_MAX_M;
export const ELEV_SPAN_M = WV2_ELEV_SPAN_M;
export const SEA_LEVEL_M = WV2_SEA_LEVEL_M;
export const SEA_LEVEL_NORM = WV2_SEA_LEVEL_NORM;

export const DEFAULT_HEIGHT_RATIO = 0.14;
export const BIG_WORLD_EXTENT_M = 81_920;
export const BIG_WORLD_HEIGHT_RATIO = 0.10;

export function floraSectorsPerSideForGrid(gridSize: number): number {
  if (!Number.isInteger(gridSize) || gridSize < TILE_ACTIVE_SIZE || gridSize % TILE_ACTIVE_SIZE !== 0) {
    throw new Error(
      `Invalid World v2 grid_size ${String(gridSize)} for flora sectors (expected positive multiple of ${TILE_ACTIVE_SIZE})`,
    );
  }
  return gridSize / TILE_ACTIVE_SIZE;
}

export function resolveWorldHeightRatio(
  worldSourceMode: 'v1' | 'v2',
  extentM?: number | null,
): number {
  if (worldSourceMode === 'v2' && typeof extentM === 'number' && Number.isFinite(extentM) && extentM >= BIG_WORLD_EXTENT_M) {
    return BIG_WORLD_HEIGHT_RATIO;
  }
  return DEFAULT_HEIGHT_RATIO;
}

export const WORLD_V2_FLORA_BASE_STRIDE: Readonly<Record<QualityTier, number>> = {
  low: 8,
  medium: 3,
  high: 2,
};

/**
 * Per-tier emitted instance ceiling for unchunked World v2 flora (`WorldV2Flora`).
 * Keeps both `C01` (~41k instances) and `BIG_4096_v2` (~536k instances) strictly within
 * §4a flora and total triangle budgets (even at worst-case 112 tris/inst, 224 with high
 * shadows), while preserving `low` x4 negative-control trips (> 530k tris even at 44 tris/inst).
 */
export const WORLD_V2_FLORA_MAX_INSTANCES: Readonly<Record<QualityTier, number>> = {
  low: 4_000,
  medium: 13_500,
  high: 20_000,
};

export function resolveWorldV2FloraStride(tier: QualityTier, totalInstances: number): number {
  const baseStride = WORLD_V2_FLORA_BASE_STRIDE[tier];
  if (!Number.isFinite(totalInstances) || totalInstances <= 0) {
    return baseStride;
  }
  const capStride = Math.ceil(totalInstances / WORLD_V2_FLORA_MAX_INSTANCES[tier]);
  return Math.max(baseStride, capStride);
}

export const WaterKind = {
  None: 0,
  Ocean: 1,
  FreshwaterLake: 2,
  EndorheicLake: 3,
  River: 4,
  Waterfall: 5,
  DeltaTurbid: 6,
} as const;

export type WaterKindValue = (typeof WaterKind)[keyof typeof WaterKind];

/**
 * Canonical 22-biome sRGB triples matching `compute_macro_albedo_rgb` in
 * `src-tauri/crates/anima-worldgen/src/bake/mod.rs:320-341`.
 */
export const BIOME_SRGB_PALETTE: ReadonlyArray<readonly [number, number, number]> = [
  [24, 62, 96], // 0: Ocean
  [206, 189, 148], // 1: Beach
  [198, 164, 112], // 2: Desert
  [156, 152, 86], // 3: Savanna
  [92, 134, 68], // 4: Grassland
  [124, 129, 79], // 5: Shrubland
  [48, 96, 52], // 6: Forest
  [34, 84, 42], // 7: Jungle
  [42, 74, 52], // 8: Taiga
  [118, 122, 92], // 9: Tundra
  [56, 82, 54], // 10: Swamp
  [118, 114, 108], // 11: Rock
  [234, 239, 244], // 12: Snow
  [42, 92, 118], // 13: River
  [34, 84, 112], // 14: Lake
  [44, 78, 48], // 15: Mangrove
  [126, 128, 76], // 16: Chaparral
  [164, 146, 98], // 17: Steppe
  [102, 124, 78], // 18: Alpine
  [164, 116, 86], // 19: Badlands
  [224, 236, 246], // 20: Glacier
  [88, 102, 68], // 21: Bog
];

export interface TileManifestEntry {
  byte_len: number;
  layer: string;
  mip: number;
  path: string;
  sha256: string;
  tx: number;
  ty: number;
}

export interface WorldV2IndexJson {
  extent_m: number;
  format_version: number;
  generator_version: number;
  grid_size: number;
  release_archive: string;
  release_archive_sha256: string;
  schema_tag: string;
  seed: number;
  sim_256_byte_len: number;
  sim_256_file: string;
  sim_world_identity: string;
  tiles: TileManifestEntry[];
}

export interface Ktx2LevelEntry {
  byteOffset: number;
  byteLength: number;
  uncompressedByteLength: number;
}

export interface DecodedKtx2 {
  vkFormat: number;
  typeSize: number;
  width: number;
  height: number;
  levelCount: number;
  supercompressionScheme: number;
  level0: Uint8Array;
}

export interface WorldV2LoadedBundle {
  simWorldIdentity: string;
  bundleIdentity: string;
  schemaTag: string;
  seed: number;
  extentM: number;
  gridSize: number;
  floraSectorsPerSide: number;
  mip: number;
  size: number;
  macroAlbedoSize: number;
  tileCountVerified: number;
  sim: AnmwV3Sim;
  heightM: Float32Array;
  normalRg: Uint8Array;
  macroAlbedoRgba: Uint8Array;
  biomeIdsRgba: Uint8Array;
  biomeWeightsRgba: Uint8Array;
  lightingRgba: Uint8Array;
  horizonRgba: Uint8Array;
  hydroFlowRgba: Uint8Array;
  shoreSdfRg: Uint8Array;
  waterHeightM: Float32Array;
  waterKind: Uint8Array;
  caveMask: Uint8Array;
  caveNetwork: CaveNetwork | null;
  floraSectors: FloraSectorBlob[];
}

export class TileIntegrityError extends Error {
  readonly tilePath: string;
  readonly expectedSha256: string;
  readonly actualSha256: string;

  constructor(message: string, tilePath: string, expectedSha256: string, actualSha256: string) {
    super(message);
    this.name = 'TileIntegrityError';
    this.tilePath = tilePath;
    this.expectedSha256 = expectedSha256;
    this.actualSha256 = actualSha256;
  }
}

export type WorldV2ErrorCode =
  | 'MISSING_INDEX'
  | 'INVALID_INDEX'
  | 'MISSING_ASSET'
  | 'INTEGRITY'
  | 'DECODE';

export class WorldV2LoadError extends Error {
  readonly code: WorldV2ErrorCode;
  readonly assetPath?: string;

  constructor(message: string, code: WorldV2ErrorCode, assetPath?: string) {
    super(message);
    this.name = 'WorldV2LoadError';
    this.code = code;
    this.assetPath = assetPath;
  }
}
