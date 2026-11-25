/**
 * Shared constants and types for the dev-only World v2 tile preview (`WV2-6-PREVIEW`).
 * Values match `src-tauri/crates/anima-worldgen/src/lib.rs`, `bake/mod.rs`, and `bake/pack_ktx2.rs`.
 */

export const BANNER_TEXT = 'World v2 Preview — not the simulated world';

export const TILE_ACTIVE_SIZE = 256;
export const TILE_GUTTER = 2;
export const TILE_TOTAL_SIZE = TILE_ACTIVE_SIZE + 2 * TILE_GUTTER; // 260

export const ELEV_MIN_M = -1500.0;
export const ELEV_MAX_M = 5700.0;
export const ELEV_SPAN_M = ELEV_MAX_M - ELEV_MIN_M; // 7200.0
export const SEA_LEVEL_M = 0.0;
export const SEA_LEVEL_NORM = (SEA_LEVEL_M - ELEV_MIN_M) / ELEV_SPAN_M;

export const WORLD_EXTENT_M = 40960.0;
export const RENDER_SIZE = 1200.0;

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

export interface WorldV2PreviewGrid {
  simWorldIdentity: string;
  seed: number;
  extentM: number;
  mip: number;
  size: number;
  tileCountVerified: number;
  heightM: Float32Array;
  normalRg: Uint8Array;
  macroAlbedoRgba: Uint8Array;
  biomeIdsRgba: Uint8Array;
  biomeWeightsRgba: Uint8Array;
  lightingRgba: Uint8Array;
  waterHeightM: Float32Array;
  waterKind: Uint8Array;
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
