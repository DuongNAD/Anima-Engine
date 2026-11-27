// prototype, to be replaced — WV2-LOOK r14 on-demand verified L0 (20 m) height tile cache.

import {
  TILE_ACTIVE_SIZE,
  TILE_GUTTER,
  TILE_TOTAL_SIZE,
  type L0HeightManifest,
} from './types';
import {
  VK_FORMAT_R16_UNORM,
  copyActiveTilePatch,
  decodeKtx2Level0,
  decodeR16HeightM,
} from './ktx2Decoder';
import { verifyTileBytes } from './sha256';

export interface L0HeightCacheOptions {
  maxTiles?: number;
  fetcher?: typeof fetch;
}

export class L0HeightTileCache {
  private readonly manifest: L0HeightManifest | undefined;
  private readonly maxTiles: number;
  private readonly fetcher: typeof fetch;
  private readonly tiles = new Map<string, Float32Array>();
  private readonly inflight = new Set<string>();
  private readonly failed = new Set<string>();
  private _version = 0;

  constructor(manifest: L0HeightManifest | undefined, options: L0HeightCacheOptions = {}) {
    this.manifest = manifest;
    this.maxTiles = Math.max(4, options.maxTiles ?? 24);
    this.fetcher = options.fetcher ?? fetch.bind(globalThis);
  }

  get version(): number {
    return this._version;
  }

  get tileCount(): number {
    return this.tiles.size;
  }

  get isSupported(): boolean {
    return Boolean(this.manifest && this.manifest.entries.size > 0);
  }

  /** Directly injects a decoded 256x256 active height tile (for unit tests). */
  setDecodedTile(tx: number, ty: number, heightsM: Float32Array): void {
    if (heightsM.length !== TILE_ACTIVE_SIZE * TILE_ACTIVE_SIZE) {
      throw new Error(`Expected ${TILE_ACTIVE_SIZE * TILE_ACTIVE_SIZE} texels, got ${heightsM.length}`);
    }
    const key = `${tx}:${ty}`;
    this.tiles.delete(key);
    this.tiles.set(key, heightsM);
    this.evictIfNeeded();
    this._version += 1;
  }

  private evictIfNeeded(): void {
    while (this.tiles.size > this.maxTiles) {
      const oldest = this.tiles.keys().next().value;
      if (oldest === undefined) break;
      this.tiles.delete(oldest);
    }
  }

  private touchTile(key: string): Float32Array | undefined {
    const hit = this.tiles.get(key);
    if (!hit) return undefined;
    this.tiles.delete(key);
    this.tiles.set(key, hit);
    return hit;
  }

  private tileRangeForUv(u0: number, v0: number, u1: number, v1: number): {
    tx0: number;
    tx1: number;
    ty0: number;
    ty1: number;
  } | null {
    if (!this.manifest) return null;
    const { gridSize, tilesPerSide } = this.manifest;
    const cu0 = Math.min(1, Math.max(0, u0)) * (gridSize - 1);
    const cu1 = Math.min(1, Math.max(0, u1)) * (gridSize - 1);
    const cv0 = Math.min(1, Math.max(0, v0)) * (gridSize - 1);
    const cv1 = Math.min(1, Math.max(0, v1)) * (gridSize - 1);
    return {
      tx0: Math.max(0, Math.floor(Math.floor(cu0) / TILE_ACTIVE_SIZE)),
      tx1: Math.min(tilesPerSide - 1, Math.floor(Math.ceil(cu1) / TILE_ACTIVE_SIZE)),
      ty0: Math.max(0, Math.floor(Math.floor(cv0) / TILE_ACTIVE_SIZE)),
      ty1: Math.min(tilesPerSide - 1, Math.floor(Math.ceil(cv1) / TILE_ACTIVE_SIZE)),
    };
  }

  /** Returns true when all L0 tiles covering `[u0..u1, v0..v1]` are decoded and resident. */
  isRegionReady(u0: number, v0: number, u1: number, v1: number): boolean {
    const range = this.tileRangeForUv(u0, v0, u1, v1);
    if (!range) return true;
    for (let ty = range.ty0; ty <= range.ty1; ty += 1) {
      for (let tx = range.tx0; tx <= range.tx1; tx += 1) {
        const key = `${tx}:${ty}`;
        if (!this.tiles.has(key) && !this.failed.has(key)) return false;
      }
    }
    return true;
  }

  /** Triggers SHA-256 verified fetches for any missing L0 tiles intersecting `[u0..u1, v0..v1]`. */
  requestRegion(u0: number, v0: number, u1: number, v1: number): void {
    const manifest = this.manifest;
    const range = this.tileRangeForUv(u0, v0, u1, v1);
    if (!manifest || !range) return;
    const base = manifest.tilesBaseUrl.replace(/\/+$/, '');
    for (let ty = range.ty0; ty <= range.ty1; ty += 1) {
      for (let tx = range.tx0; tx <= range.tx1; tx += 1) {
        const key = `${tx}:${ty}`;
        if (this.tiles.has(key) || this.inflight.has(key) || this.failed.has(key)) continue;
        const entry = manifest.entries.get(key);
        if (!entry) continue;
        this.inflight.add(key);
        const url = `${base}/${entry.path.replace(/^\/+/, '')}`;
        void this.fetcher(url, { cache: 'force-cache' })
          .then((res) => {
            if (!res.ok) throw new Error(`HTTP ${res.status} loading L0 tile ${entry.path}`);
            return res.arrayBuffer();
          })
          .then(async (buf) => {
            const rawBytes = new Uint8Array(buf);
            await verifyTileBytes(entry, rawBytes);
            const ktx = decodeKtx2Level0(rawBytes);
            if (ktx.width !== TILE_TOTAL_SIZE || ktx.height !== TILE_TOTAL_SIZE) {
              throw new Error(`L0 tile ${entry.path} unexpected size ${ktx.width}x${ktx.height}`);
            }
            if (ktx.vkFormat !== VK_FORMAT_R16_UNORM) {
              throw new Error(`L0 tile ${entry.path} unexpected format ${ktx.vkFormat}`);
            }
            const decoded = decodeR16HeightM(ktx.level0, ktx.width, ktx.height);
            const active = new Float32Array(TILE_ACTIVE_SIZE * TILE_ACTIVE_SIZE);
            copyActiveTilePatch(
              decoded,
              active,
              1,
              TILE_ACTIVE_SIZE,
              0,
              0,
              TILE_TOTAL_SIZE,
              TILE_ACTIVE_SIZE,
              TILE_GUTTER,
            );
            this.tiles.set(key, active);
            this.evictIfNeeded();
            this._version += 1;
          })
          .catch(() => {
            this.failed.add(key);
          })
          .finally(() => {
            this.inflight.delete(key);
          });
      }
    }
  }

  private sampleTexelM(ix: number, iy: number): number | null {
    const manifest = this.manifest;
    if (!manifest) return null;
    const { gridSize, tilesPerSide } = manifest;
    const cx = Math.min(gridSize - 1, Math.max(0, ix));
    const cy = Math.min(gridSize - 1, Math.max(0, iy));
    const tx = Math.min(tilesPerSide - 1, Math.floor(cx / TILE_ACTIVE_SIZE));
    const ty = Math.min(tilesPerSide - 1, Math.floor(cy / TILE_ACTIVE_SIZE));
    const tile = this.touchTile(`${tx}:${ty}`);
    if (!tile) return null;
    const lx = cx - tx * TILE_ACTIVE_SIZE;
    const ly = cy - ty * TILE_ACTIVE_SIZE;
    return tile[ly * TILE_ACTIVE_SIZE + lx];
  }

  /** Bilinear L0 height sample in metres at normalized `[u, v]`, or `null` if tile not loaded. */
  sampleHeightM(u: number, v: number): number | null {
    const manifest = this.manifest;
    if (!manifest) return null;
    const span = manifest.gridSize - 1;
    const fx = Math.min(1, Math.max(0, u)) * span;
    const fy = Math.min(1, Math.max(0, v)) * span;
    const x0 = Math.floor(fx);
    const y0 = Math.floor(fy);
    const x1 = Math.min(span, x0 + 1);
    const y1 = Math.min(span, y0 + 1);
    const tx = fx - x0;
    const ty = fy - y0;
    const h00 = this.sampleTexelM(x0, y0);
    const h10 = this.sampleTexelM(x1, y0);
    const h01 = this.sampleTexelM(x0, y1);
    const h11 = this.sampleTexelM(x1, y1);
    if (h00 === null || h10 === null || h01 === null || h11 === null) return null;
    return (h00 * (1 - tx) + h10 * tx) * (1 - ty) + (h01 * (1 - tx) + h11 * tx) * ty;
  }
}
