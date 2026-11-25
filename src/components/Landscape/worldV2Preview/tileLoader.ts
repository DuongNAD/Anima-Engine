import {
  VK_FORMAT_BC5_UNORM_BLOCK,
  VK_FORMAT_R16G16_UNORM,
  VK_FORMAT_R16_UNORM,
  VK_FORMAT_R8G8B8A8_SRGB,
  VK_FORMAT_R8G8B8A8_UNORM,
  copyActiveTilePatch,
  decodeBc5Unorm,
  decodeKtx2Level0,
  decodeR16HeightM,
  decodeWaterR16G16,
} from './ktx2Decoder';
import { verifyTileBytes } from './sha256';
import {
  TILE_ACTIVE_SIZE,
  TILE_TOTAL_SIZE,
  type TileManifestEntry,
  type WorldV2IndexJson,
  type WorldV2PreviewGrid,
} from './types';

export interface LoadWorldV2PreviewOptions {
  indexUrl?: string;
  tilesBaseUrl?: string;
  mip?: number;
  signal?: AbortSignal;
  fetcher?: typeof fetch;
  onProgress?: (msg: string) => void;
}

const REQUIRED_LAYERS = [
  'height',
  'normal_xy',
  'macro_albedo',
  'biome_ids',
  'biome_weights',
  'terrain_lighting',
] as const;

function assertTileDimensions(decodedWidth: number, decodedHeight: number, path: string): void {
  if (decodedWidth !== TILE_TOTAL_SIZE || decodedHeight !== TILE_TOTAL_SIZE) {
    throw new Error(
      `Tile ${path} dimensions mismatch: expected ${TILE_TOTAL_SIZE}x${TILE_TOTAL_SIZE}, got ${decodedWidth}x${decodedHeight}`,
    );
  }
}

function assertFormat(actual: number, expected: number, path: string): void {
  if (actual !== expected) {
    throw new Error(`Tile ${path} vkFormat mismatch: expected ${expected}, got ${actual}`);
  }
}

function tileKey(layer: string, mip: number, tx: number, ty: number): string {
  return `${layer}:${mip}:${tx}:${ty}`;
}

async function fetchCheckedTile(
  entry: TileManifestEntry,
  tilesBaseUrl: string,
  fetcher: typeof fetch,
  signal?: AbortSignal,
): Promise<Uint8Array> {
  const base = tilesBaseUrl.replace(/\/+$/, '');
  const rel = entry.path.replace(/^\/+/, '');
  const url = `${base}/${rel}`;
  const res = await fetcher(url, { signal, cache: 'no-store' });
  if (!res.ok) {
    throw new Error(
      `Missing World v2 tile ${entry.path} (HTTP ${res.status} from ${url}). Ensure the C01 tile pack is extracted into world-v2-tiles-local/.`,
    );
  }
  const bytes = new Uint8Array(await res.arrayBuffer());
  await verifyTileBytes(entry, bytes);
  return bytes;
}

/**
 * Loads `artifacts/world-v2/index.json` and decodes verified local C01 tiles at `mip`
 * (default `mip = 2` -> `4x4` tiles = `1024x1024`). Fails loudly on missing or tampered tiles.
 */
export async function loadWorldV2Preview(
  options: LoadWorldV2PreviewOptions = {},
): Promise<WorldV2PreviewGrid> {
  const indexUrl = options.indexUrl ?? '/artifacts/world-v2/index.json';
  const tilesBaseUrl = options.tilesBaseUrl ?? '/world-v2-tiles-local';
  const mip = options.mip ?? 2;
  const fetcher = options.fetcher ?? globalThis.fetch;
  const signal = options.signal;

  options.onProgress?.(`Loading World v2 manifest (${indexUrl})…`);
  const idxRes = await fetcher(indexUrl, { signal, cache: 'no-store' });
  if (!idxRes.ok) {
    throw new Error(`Failed to fetch ${indexUrl} (HTTP ${idxRes.status})`);
  }
  const index = (await idxRes.json()) as WorldV2IndexJson;
  if (
    !index ||
    index.format_version !== 1 ||
    index.generator_version !== 3 ||
    !Array.isArray(index.tiles) ||
    index.tiles.length === 0
  ) {
    throw new Error(`Invalid World v2 index.json schema at ${indexUrl}`);
  }

  const gridAtMip = index.grid_size >>> mip;
  if (gridAtMip < TILE_ACTIVE_SIZE || gridAtMip % TILE_ACTIVE_SIZE !== 0) {
    throw new Error(`Invalid mip ${mip} for grid_size ${index.grid_size}`);
  }
  const tilesPerSide = gridAtMip / TILE_ACTIVE_SIZE;
  const size = tilesPerSide * TILE_ACTIVE_SIZE;
  const cellCount = size * size;

  const entryMap = new Map<string, TileManifestEntry>();
  for (const entry of index.tiles) {
    if (entry.mip === mip) {
      entryMap.set(tileKey(entry.layer, entry.mip, entry.tx, entry.ty), entry);
    }
  }

  const heightM = new Float32Array(cellCount);
  const normalRg = new Uint8Array(cellCount * 2);
  const macroAlbedoRgba = new Uint8Array(cellCount * 4);
  const biomeIdsRgba = new Uint8Array(cellCount * 4);
  const biomeWeightsRgba = new Uint8Array(cellCount * 4);
  const lightingRgba = new Uint8Array(cellCount * 4);
  const waterHeightM = new Float32Array(cellCount);
  const waterKind = new Uint8Array(cellCount);

  let tileCountVerified = 0;

  for (let ty = 0; ty < tilesPerSide; ty += 1) {
    for (let tx = 0; tx < tilesPerSide; tx += 1) {
      options.onProgress?.(
        `Verifying & decoding L${mip} tile (${tx},${ty}) [${tileCountVerified} checked]…`,
      );

      for (const layer of REQUIRED_LAYERS) {
        const entry = entryMap.get(tileKey(layer, mip, tx, ty));
        if (!entry) {
          throw new Error(`Manifest index.json missing required tile ${layer} L${mip} (${tx},${ty})`);
        }
        const rawBytes = await fetchCheckedTile(entry, tilesBaseUrl, fetcher, signal);
        tileCountVerified += 1;

        const ktx = decodeKtx2Level0(rawBytes);
        assertTileDimensions(ktx.width, ktx.height, entry.path);

        if (layer === 'height') {
          assertFormat(ktx.vkFormat, VK_FORMAT_R16_UNORM, entry.path);
          const tileH = decodeR16HeightM(ktx.level0, ktx.width, ktx.height);
          copyActiveTilePatch(tileH, heightM, 1, size, tx, ty);
        } else if (layer === 'normal_xy') {
          assertFormat(ktx.vkFormat, VK_FORMAT_BC5_UNORM_BLOCK, entry.path);
          const tileN = decodeBc5Unorm(ktx.level0, ktx.width, ktx.height);
          copyActiveTilePatch(tileN, normalRg, 2, size, tx, ty);
        } else if (layer === 'macro_albedo') {
          assertFormat(ktx.vkFormat, VK_FORMAT_R8G8B8A8_SRGB, entry.path);
          copyActiveTilePatch(ktx.level0, macroAlbedoRgba, 4, size, tx, ty);
        } else if (layer === 'biome_ids') {
          assertFormat(ktx.vkFormat, VK_FORMAT_R8G8B8A8_UNORM, entry.path);
          copyActiveTilePatch(ktx.level0, biomeIdsRgba, 4, size, tx, ty);
        } else if (layer === 'biome_weights') {
          assertFormat(ktx.vkFormat, VK_FORMAT_R8G8B8A8_UNORM, entry.path);
          copyActiveTilePatch(ktx.level0, biomeWeightsRgba, 4, size, tx, ty);
        } else if (layer === 'terrain_lighting') {
          assertFormat(ktx.vkFormat, VK_FORMAT_R8G8B8A8_UNORM, entry.path);
          copyActiveTilePatch(ktx.level0, lightingRgba, 4, size, tx, ty);
        }
      }

      // `water_surface` tiles are omitted by pack_ktx2 when all cells in the tile are dry.
      const waterEntry = entryMap.get(tileKey('water_surface', mip, tx, ty));
      if (waterEntry) {
        const rawBytes = await fetchCheckedTile(waterEntry, tilesBaseUrl, fetcher, signal);
        tileCountVerified += 1;
        const ktx = decodeKtx2Level0(rawBytes);
        assertTileDimensions(ktx.width, ktx.height, waterEntry.path);
        assertFormat(ktx.vkFormat, VK_FORMAT_R16G16_UNORM, waterEntry.path);
        const decodedWater = decodeWaterR16G16(ktx.level0, ktx.width, ktx.height);
        copyActiveTilePatch(decodedWater.waterHeightM, waterHeightM, 1, size, tx, ty);
        copyActiveTilePatch(decodedWater.waterKind, waterKind, 1, size, tx, ty);
      }
    }
  }

  return {
    simWorldIdentity: index.sim_world_identity,
    seed: index.seed,
    extentM: index.extent_m,
    mip,
    size,
    tileCountVerified,
    heightM,
    normalRg,
    macroAlbedoRgba,
    biomeIdsRgba,
    biomeWeightsRgba,
    lightingRgba,
    waterHeightM,
    waterKind,
  };
}
