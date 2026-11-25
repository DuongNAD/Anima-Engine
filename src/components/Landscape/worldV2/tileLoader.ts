import {
  decodeCaveNetworkBin,
  decodeFloraSectorAnmf,
  decodeAnmwV3Sim,
  type CaveNetwork,
  type FloraSectorBlob,
} from '../utils/worldArtifactV3';
import type { QualityTier } from '../utils/qualityTiers';
import {
  VK_FORMAT_BC5_UNORM_BLOCK,
  VK_FORMAT_R16G16_UNORM,
  VK_FORMAT_R16_UNORM,
  VK_FORMAT_R8G8B8A8_SRGB,
  VK_FORMAT_R8G8B8A8_UNORM,
  VK_FORMAT_R8G8_UNORM,
  VK_FORMAT_R8_UNORM,
  copyActiveTilePatch,
  decodeBc5Unorm,
  decodeKtx2Level0,
  decodeR16HeightM,
  decodeWaterR16G16,
} from './ktx2Decoder';
import { computeSha256Hex, verifyTileBytes } from './sha256';
import {
  TILE_ACTIVE_SIZE,
  TILE_TOTAL_SIZE,
  TileIntegrityError,
  WorldV2LoadError,
  type TileManifestEntry,
  type WorldV2IndexJson,
  type WorldV2LoadedBundle,
} from './types';

export interface LoadWorldV2BundleOptions {
  indexUrl?: string;
  tilesBaseUrl?: string;
  mip?: number;
  includeFlora?: boolean;
  includeCaveNetwork?: boolean;
  concurrency?: number;
  signal?: AbortSignal;
  fetcher?: typeof fetch;
  onProgress?: (msg: string) => void;
}

const REQUIRED_MIP_LAYERS = [
  'height',
  'normal_xy',
  'biome_ids',
  'biome_weights',
  'terrain_lighting',
  'terrain_horizon',
  'hydro_flow',
  'shore_sdf',
] as const;

export function resolveWorldSourceMode(
  search: string,
  envMode = (import.meta as { env?: { MODE?: string } }).env?.MODE,
): 'v1' | 'v2' {
  const params = new URLSearchParams(search);
  const explicit = params.get('world');
  if (explicit === 'v2') return 'v2';
  if (explicit === 'v1') return 'v1';
  if (params.get('capture') === '1') return 'v1';
  if (envMode === 'test') return 'v1';
  return 'v2';
}

export function resolveWorldV2Mip(tier: QualityTier, search = ''): number {
  const params = new URLSearchParams(search);
  const overrideRaw = params.get('wv2Mip');
  if (overrideRaw !== null) {
    const parsed = Number.parseInt(overrideRaw, 10);
    if (Number.isInteger(parsed) && parsed >= 0 && parsed <= 4) {
      return parsed;
    }
  }
  // L1 (2048^2) on high quality, L2 (1024^2) on medium/low (§7.2 prototype tiering).
  return tier === 'high' ? 1 : 2;
}

function tileKey(layer: string, mip: number, tx: number, ty: number): string {
  return `${layer}:${mip}:${tx}:${ty}`;
}

function assertTileDimensions(w: number, h: number, path: string): void {
  if (w !== TILE_TOTAL_SIZE || h !== TILE_TOTAL_SIZE) {
    throw new WorldV2LoadError(
      `Tile ${path} dimensions mismatch: expected ${TILE_TOTAL_SIZE}x${TILE_TOTAL_SIZE}, got ${w}x${h}`,
      'DECODE',
      path,
    );
  }
}

function assertFormat(actual: number, expected: number, path: string): void {
  if (actual !== expected) {
    throw new WorldV2LoadError(
      `Tile ${path} vkFormat mismatch: expected ${expected}, got ${actual}`,
      'DECODE',
      path,
    );
  }
}

async function runWithConcurrency<T>(
  items: readonly T[],
  limit: number,
  worker: (item: T) => Promise<void>,
): Promise<void> {
  let cursor = 0;
  const count = Math.max(1, Math.min(limit, items.length));
  const lanes = Array.from({ length: count }, async () => {
    while (cursor < items.length) {
      const idx = cursor;
      cursor += 1;
      await worker(items[idx]);
    }
  });
  await Promise.all(lanes);
}

async function fetchCheckedBytes(
  entry: Pick<TileManifestEntry, 'path' | 'byte_len' | 'sha256'>,
  tilesBaseUrl: string,
  fetcher: typeof fetch,
  signal?: AbortSignal,
): Promise<Uint8Array> {
  const base = tilesBaseUrl.replace(/\/+$/, '');
  const rel = entry.path.replace(/^\/+/, '');
  const url = `${base}/${rel}`;
  let res: Response;
  try {
    res = await fetcher(url, { signal, cache: 'no-store' });
  } catch (err: unknown) {
    throw new WorldV2LoadError(
      `Network error fetching World v2 asset ${entry.path}: ${err instanceof Error ? err.message : String(err)}`,
      'MISSING_ASSET',
      entry.path,
    );
  }
  if (!res.ok) {
    throw new WorldV2LoadError(
      `Missing World v2 asset ${entry.path} (HTTP ${res.status} at ${url}). Run "npm run fetch:world-v2-tiles" to stage verified tiles.`,
      'MISSING_ASSET',
      entry.path,
    );
  }
  const bytes = new Uint8Array(await res.arrayBuffer());
  await verifyTileBytes(entry, bytes);
  return bytes;
}

/**
 * Loads `index.json`, `sim_256.anmw3`, all required/sparse KTX2 layers, optional `cave_network.bin`,
 * and decoded `flora_tiles` (`ANMF`) with mandatory WebCrypto SHA-256 verification per asset.
 * Fails loudly (`TileIntegrityError` / `WorldV2LoadError`) with zero v1 fallback.
 */
export async function loadWorldV2Bundle(
  options: LoadWorldV2BundleOptions = {},
): Promise<WorldV2LoadedBundle> {
  const indexUrl = options.indexUrl ?? '/world-v2/index.json';
  const tilesBaseUrl = options.tilesBaseUrl ?? '/world-v2';
  const mip = options.mip ?? 2;
  const includeFlora = options.includeFlora ?? true;
  const includeCaveNetwork = options.includeCaveNetwork ?? true;
  const concurrency = options.concurrency ?? 16;
  const fetcher = options.fetcher ?? globalThis.fetch;
  const signal = options.signal;

  options.onProgress?.(`Fetching World v2 index (${indexUrl})…`);
  let idxRes: Response;
  try {
    idxRes = await fetcher(indexUrl, { signal, cache: 'no-store' });
  } catch (err: unknown) {
    throw new WorldV2LoadError(
      `Failed to fetch World v2 index (${indexUrl}): ${err instanceof Error ? err.message : String(err)}`,
      'MISSING_INDEX',
      indexUrl,
    );
  }
  if (!idxRes.ok) {
    throw new WorldV2LoadError(
      `Missing World v2 index ${indexUrl} (HTTP ${idxRes.status}). Run "npm run fetch:world-v2-tiles" first.`,
      'MISSING_INDEX',
      indexUrl,
    );
  }

  const indexBytes = new Uint8Array(await idxRes.arrayBuffer());
  const bundleIdentity = await computeSha256Hex(indexBytes);

  let index: WorldV2IndexJson;
  try {
    index = JSON.parse(new TextDecoder().decode(indexBytes)) as WorldV2IndexJson;
  } catch (err: unknown) {
    throw new WorldV2LoadError(
      `Invalid JSON in ${indexUrl}: ${err instanceof Error ? err.message : String(err)}`,
      'INVALID_INDEX',
      indexUrl,
    );
  }

  if (
    !index ||
    index.format_version !== 1 ||
    index.generator_version !== 3 ||
    !Array.isArray(index.tiles) ||
    index.tiles.length === 0 ||
    typeof index.sim_256_file !== 'string' ||
    typeof index.sim_world_identity !== 'string'
  ) {
    throw new WorldV2LoadError(`Invalid World v2 index schema at ${indexUrl}`, 'INVALID_INDEX', indexUrl);
  }

  const gridAtMip = index.grid_size >>> mip;
  if (gridAtMip < TILE_ACTIVE_SIZE || gridAtMip % TILE_ACTIVE_SIZE !== 0) {
    throw new WorldV2LoadError(
      `Invalid mip ${mip} for World v2 grid_size ${index.grid_size}`,
      'INVALID_INDEX',
      indexUrl,
    );
  }

  const tilesPerSide = gridAtMip / TILE_ACTIVE_SIZE;
  const size = tilesPerSide * TILE_ACTIVE_SIZE;
  const cellCount = size * size;

  // `macro_albedo` is baked at L2..L4 in index.json; clamp mip >= 2 and upsample if needed.
  const macroAlbedoMip = Math.max(2, mip);
  const macroAlbedoPerSide = (index.grid_size >>> macroAlbedoMip) / TILE_ACTIVE_SIZE;
  const macroAlbedoSize = macroAlbedoPerSide * TILE_ACTIVE_SIZE;

  const mipEntryMap = new Map<string, TileManifestEntry>();
  let caveNetworkEntry: TileManifestEntry | null = null;
  const floraEntries: TileManifestEntry[] = [];

  for (const entry of index.tiles) {
    if (entry.layer === 'cave_network') {
      caveNetworkEntry = entry;
    } else if (entry.layer === 'flora_tiles') {
      floraEntries.push(entry);
    } else if (entry.mip === mip || (entry.layer === 'macro_albedo' && entry.mip === macroAlbedoMip)) {
      mipEntryMap.set(tileKey(entry.layer, entry.mip, entry.tx, entry.ty), entry);
    }
  }

  options.onProgress?.(`Verifying ${index.sim_256_file}…`);
  const simBytes = await fetchCheckedBytes(
    {
      path: index.sim_256_file,
      byte_len: index.sim_256_byte_len,
      sha256: index.sim_world_identity,
    },
    tilesBaseUrl,
    fetcher,
    signal,
  );
  let sim;
  try {
    sim = decodeAnmwV3Sim(simBytes);
  } catch (err: unknown) {
    throw new WorldV2LoadError(
      `Failed to decode ${index.sim_256_file}: ${err instanceof Error ? err.message : String(err)}`,
      'DECODE',
      index.sim_256_file,
    );
  }

  const heightM = new Float32Array(cellCount);
  const normalRg = new Uint8Array(cellCount * 2);
  const macroAlbedoRgba = new Uint8Array(macroAlbedoSize * macroAlbedoSize * 4);
  const biomeIdsRgba = new Uint8Array(cellCount * 4);
  const biomeWeightsRgba = new Uint8Array(cellCount * 4);
  const lightingRgba = new Uint8Array(cellCount * 4);
  const horizonRgba = new Uint8Array(cellCount * 4);
  const hydroFlowRgba = new Uint8Array(cellCount * 4);
  const shoreSdfRg = new Uint8Array(cellCount * 2);
  const waterHeightM = new Float32Array(cellCount);
  const waterKind = new Uint8Array(cellCount);
  const caveMask = new Uint8Array(cellCount);

  const jobs: Array<{ entry: TileManifestEntry; layer: string; tx: number; ty: number }> = [];

  for (let ty = 0; ty < tilesPerSide; ty += 1) {
    for (let tx = 0; tx < tilesPerSide; tx += 1) {
      for (const layer of REQUIRED_MIP_LAYERS) {
        const entry = mipEntryMap.get(tileKey(layer, mip, tx, ty));
        if (!entry) {
          throw new WorldV2LoadError(
            `Manifest index.json missing required tile ${layer} L${mip} (${tx},${ty})`,
            'MISSING_ASSET',
          );
        }
        jobs.push({ entry, layer, tx, ty });
      }
      const waterEntry = mipEntryMap.get(tileKey('water_surface', mip, tx, ty));
      if (waterEntry) jobs.push({ entry: waterEntry, layer: 'water_surface', tx, ty });
      const caveEntry = mipEntryMap.get(tileKey('cave_mask', mip, tx, ty));
      if (caveEntry) jobs.push({ entry: caveEntry, layer: 'cave_mask', tx, ty });
    }
  }

  for (let ty = 0; ty < macroAlbedoPerSide; ty += 1) {
    for (let tx = 0; tx < macroAlbedoPerSide; tx += 1) {
      const entry = mipEntryMap.get(tileKey('macro_albedo', macroAlbedoMip, tx, ty));
      if (!entry) {
        throw new WorldV2LoadError(
          `Manifest index.json missing required tile macro_albedo L${macroAlbedoMip} (${tx},${ty})`,
          'MISSING_ASSET',
        );
      }
      jobs.push({ entry, layer: 'macro_albedo', tx, ty });
    }
  }

  let tileCountVerified = 1; // sim_256.anmw3 already verified

  options.onProgress?.(`Verifying ${jobs.length} L${mip} terrain & water tiles…`);
  await runWithConcurrency(jobs, concurrency, async ({ entry, layer, tx, ty }) => {
    const rawBytes = await fetchCheckedBytes(entry, tilesBaseUrl, fetcher, signal);
    tileCountVerified += 1;

    try {
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
        copyActiveTilePatch(ktx.level0, macroAlbedoRgba, 4, macroAlbedoSize, tx, ty);
      } else if (layer === 'biome_ids') {
        assertFormat(ktx.vkFormat, VK_FORMAT_R8G8B8A8_UNORM, entry.path);
        copyActiveTilePatch(ktx.level0, biomeIdsRgba, 4, size, tx, ty);
      } else if (layer === 'biome_weights') {
        assertFormat(ktx.vkFormat, VK_FORMAT_R8G8B8A8_UNORM, entry.path);
        copyActiveTilePatch(ktx.level0, biomeWeightsRgba, 4, size, tx, ty);
      } else if (layer === 'terrain_lighting') {
        assertFormat(ktx.vkFormat, VK_FORMAT_R8G8B8A8_UNORM, entry.path);
        copyActiveTilePatch(ktx.level0, lightingRgba, 4, size, tx, ty);
      } else if (layer === 'terrain_horizon') {
        assertFormat(ktx.vkFormat, VK_FORMAT_R8G8B8A8_UNORM, entry.path);
        copyActiveTilePatch(ktx.level0, horizonRgba, 4, size, tx, ty);
      } else if (layer === 'hydro_flow') {
        assertFormat(ktx.vkFormat, VK_FORMAT_R8G8B8A8_UNORM, entry.path);
        copyActiveTilePatch(ktx.level0, hydroFlowRgba, 4, size, tx, ty);
      } else if (layer === 'shore_sdf') {
        assertFormat(ktx.vkFormat, VK_FORMAT_R8G8_UNORM, entry.path);
        copyActiveTilePatch(ktx.level0, shoreSdfRg, 2, size, tx, ty);
      } else if (layer === 'water_surface') {
        assertFormat(ktx.vkFormat, VK_FORMAT_R16G16_UNORM, entry.path);
        const decodedWater = decodeWaterR16G16(ktx.level0, ktx.width, ktx.height);
        copyActiveTilePatch(decodedWater.waterHeightM, waterHeightM, 1, size, tx, ty);
        copyActiveTilePatch(decodedWater.waterKind, waterKind, 1, size, tx, ty);
      } else if (layer === 'cave_mask') {
        assertFormat(ktx.vkFormat, VK_FORMAT_R8_UNORM, entry.path);
        copyActiveTilePatch(ktx.level0, caveMask, 1, size, tx, ty);
      }
    } catch (err: unknown) {
      if (err instanceof TileIntegrityError || err instanceof WorldV2LoadError) throw err;
      throw new WorldV2LoadError(
        `Tile decode error for ${entry.path}: ${err instanceof Error ? err.message : String(err)}`,
        'DECODE',
        entry.path,
      );
    }
  });

  let caveNetwork: CaveNetwork | null = null;
  if (includeCaveNetwork && caveNetworkEntry) {
    const caveBytes = await fetchCheckedBytes(caveNetworkEntry, tilesBaseUrl, fetcher, signal);
    tileCountVerified += 1;
    try {
      caveNetwork = decodeCaveNetworkBin(caveBytes);
    } catch (err: unknown) {
      throw new WorldV2LoadError(
        `Failed to decode ${caveNetworkEntry.path}: ${err instanceof Error ? err.message : String(err)}`,
        'DECODE',
        caveNetworkEntry.path,
      );
    }
  }

  const floraSectors: FloraSectorBlob[] = [];
  if (includeFlora && floraEntries.length > 0) {
    options.onProgress?.(`Verifying ${floraEntries.length} decorative flora sectors…`);
    await runWithConcurrency(floraEntries, concurrency, async (entry) => {
      const floraBytes = await fetchCheckedBytes(entry, tilesBaseUrl, fetcher, signal);
      tileCountVerified += 1;
      try {
        floraSectors.push(decodeFloraSectorAnmf(floraBytes));
      } catch (err: unknown) {
        throw new WorldV2LoadError(
          `Failed to decode ${entry.path}: ${err instanceof Error ? err.message : String(err)}`,
          'DECODE',
          entry.path,
        );
      }
    });
  }

  return {
    simWorldIdentity: index.sim_world_identity.toLowerCase(),
    bundleIdentity,
    schemaTag: index.schema_tag,
    seed: index.seed,
    extentM: index.extent_m,
    mip,
    size,
    macroAlbedoSize,
    tileCountVerified,
    sim,
    heightM,
    normalRg,
    macroAlbedoRgba,
    biomeIdsRgba,
    biomeWeightsRgba,
    lightingRgba,
    horizonRgba,
    hydroFlowRgba,
    shoreSdfRg,
    waterHeightM,
    waterKind,
    caveMask,
    caveNetwork,
    floraSectors,
  };
}
