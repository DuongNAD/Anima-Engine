import { ImprovedNoise2D } from './terrainGenerator';

// ---------------------------------------------------------------------------------------
// Huge-scale, Structure-of-Arrays (SoA) procedural world generator.
//
// Everything is stored in flat TypedArrays (one value per cell) instead of an array of
// per-cell objects, so a 1024x1024 (1M cell) world stays a handful of MB, is GC-friendly,
// and can be persisted to IndexedDB as raw binary (see worldCache.ts).
// ---------------------------------------------------------------------------------------

export const WORLD_GEN_VERSION = 5;

export enum Biome {
  Ocean = 0,
  Beach = 1,
  Desert = 2,
  Savanna = 3,
  Grassland = 4,
  Shrubland = 5,
  Forest = 6,
  Jungle = 7,
  Taiga = 8,
  Tundra = 9,
  Swamp = 10,
  Rock = 11,
  Snow = 12,
  River = 13,
  // --- Expanded environments (v3) ---
  Lake = 14,
  Mangrove = 15,
  Chaparral = 16,
  Steppe = 17,
  Alpine = 18, // alpine meadow
  Badlands = 19,
  Glacier = 20,
  Bog = 21,
}

export const BIOME_COUNT = 22;

/** RGB (0..255) per biome — single source of truth for colouring the world & minimap. */
export const BIOME_RGB: ReadonlyArray<readonly [number, number, number]> = [
  [26, 60, 120], // Ocean
  [234, 216, 162], // Beach — bright sand
  [230, 196, 104], // Desert — saturated yellow sand
  [200, 190, 96], // Savanna — dry gold-green
  [132, 196, 92], // Grassland — fresh light green
  [150, 170, 92], // Shrubland — olive
  [40, 124, 50], // Forest — vivid green
  [20, 98, 40], // Jungle — deep lush green
  [50, 104, 80], // Taiga — dark blue-green
  [166, 176, 154], // Tundra — pale grey-green
  [66, 90, 54], // Swamp — dark green with mud
  [134, 128, 120], // Rock — grey
  [248, 251, 255], // Snow — pure white
  [58, 132, 188], // River
  [42, 118, 176], // Lake
  [60, 106, 68], // Mangrove — muddy green
  [178, 158, 92], // Chaparral — olive-tan
  [190, 186, 120], // Steppe — pale tan-green
  [122, 162, 116], // Alpine — muted meadow green
  [176, 104, 66], // Badlands — red-brown
  [220, 238, 248], // Glacier — icy white-blue
  [72, 82, 58], // Bog — dark olive-brown
];

/** A distinct lake: one flat water plane is rendered per basin (cell-space bbox + level). */
export interface LakeBasin {
  /** Normalized water-surface elevation (constant across the basin). */
  level: number;
  minX: number;
  maxX: number;
  minY: number;
  maxY: number;
}

export interface World {
  size: number;
  seed: number;
  version: number;
  /** Normalized elevation in [0, 1]. */
  elevation: Float32Array;
  moisture: Float32Array; // [0, 1]
  temperature: Float32Array; // [0, 1] (cold -> hot)
  flow: Float32Array; // river flow accumulation (normalized 0..1)
  /** Terrain steepness in [0, 1] (~tan of the world-space slope angle, clamped). */
  slope: Float32Array;
  /** Inland still-water (lake) surface elevation, normalized; 0 where there is no lake. */
  water: Float32Array;
  /** Land closeness to any water (ocean/lake): ~1 at the shoreline, fading to 0 inland. */
  shore: Float32Array;
  /** Distinct lake basins, each rendered as a single water plane. */
  lakeBasins: LakeBasin[];
  biome: Uint8Array; // Biome enum per cell
  /** Flora instances as SoA (world coordinates centred on origin). */
  floraX: Float32Array;
  floraZ: Float32Array;
  floraScale: Float32Array;
  floraType: Uint8Array; // FloraType
  floraCount: number;
  /** Normalized sea level (elevation below this is ocean). */
  seaLevel: number;
}

export enum FloraType {
  Pine = 0, // taiga / tundra edge
  Round = 1, // forest / grassland
  Jungle = 2, // jungle / swamp
  Cactus = 3, // desert / savanna
  Rock = 4, // bare / alpine
}

// ---- Seeded helpers ------------------------------------------------------------------

function hashSeed(seed: string | number): number {
  if (typeof seed === 'number') return seed >>> 0;
  let h = 2166136261;
  for (let i = 0; i < seed.length; i++) {
    h ^= seed.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

function mulberry32(a: number): () => number {
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Fractal Brownian motion: summed octaves of Perlin. Returns roughly [-1, 1]. */
function fbm(noise: ImprovedNoise2D, x: number, y: number, octaves: number, lac: number, gain: number): number {
  let value = 0;
  let amp = 1;
  let freq = 1;
  let max = 0;
  for (let o = 0; o < octaves; o++) {
    value += noise.noise(x * freq, y * freq) * amp;
    max += amp;
    amp *= gain;
    freq *= lac;
  }
  return max > 0 ? value / max : 0;
}

/** Ridged multifractal: sharp mountain ridges. Returns [0, 1]. */
function ridged(noise: ImprovedNoise2D, x: number, y: number, octaves: number, lac: number, gain: number): number {
  let value = 0;
  let amp = 0.5;
  let freq = 1;
  let max = 0;
  for (let o = 0; o < octaves; o++) {
    let n = 1 - Math.abs(noise.noise(x * freq, y * freq));
    n *= n; // sharpen
    value += n * amp;
    max += amp;
    amp *= gain;
    freq *= lac;
  }
  return max > 0 ? value / max : 0;
}

function smoothstep(e0: number, e1: number, x: number): number {
  if (e0 === e1) return x < e0 ? 0 : 1;
  const t = Math.max(0, Math.min(1, (x - e0) / (e1 - e0)));
  return t * t * (3 - 2 * t);
}

/**
 * Per-cell terrain steepness in [0, 1], from the elevation gradient. Scaled so ~1 corresponds
 * to a ~45deg world-space slope (independent of map size, using the nominal render height
 * ratio), so downstream thresholds read as real slope angles.
 *
 * The gradient is measured over a wider stencil (`step` cells) rather than adjacent cells, so
 * it captures the BROAD mountainside slope the render mesh actually shows — not the fine
 * per-cell roughness that hydraulic erosion adds (which would flag almost everything "steep").
 */
function computeSlope(elev: Float32Array, size: number): Float32Array {
  const n = size * size;
  const slope = new Float32Array(n);
  const step = Math.max(2, Math.round(size / 200)); // broad mountainside slope, not erosion noise
  const ref = 0.06 * (size - 1); // maps the gradient into a well-spread [0, 1] steepness
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = y * size + x;
      const xl = Math.max(0, x - step);
      const xr = Math.min(size - 1, x + step);
      const yu = Math.max(0, y - step);
      const yd = Math.min(size - 1, y + step);
      const dx = (elev[y * size + xr] - elev[y * size + xl]) / (xr - xl || 1);
      const dy = (elev[yd * size + x] - elev[yu * size + x]) / (yd - yu || 1);
      slope[i] = Math.min(1, Math.sqrt(dx * dx + dy * dy) * ref);
    }
  }
  return slope;
}

// ---- Whittaker biome classification --------------------------------------------------

function classify(
  elev: number,
  temp: number,
  moist: number,
  flow: number,
  slope: number,
  seaLevel: number,
): Biome {
  const beach = seaLevel + 0.022; // a slightly wider sandy coastal band
  if (elev < seaLevel) return Biome.Ocean;

  // Coast: hot, wet, low shores grow mangroves; otherwise a sandy beach.
  if (elev < beach) {
    if (temp > 0.66 && moist > 0.6) return Biome.Mangrove;
    return Biome.Beach;
  }

  // Rivers cut across land where flow accumulates and the slope isn't a peak.
  if (flow > 0.55 && elev < 0.8) return Biome.River;

  // --- High-elevation bands ---
  if (elev > 0.92) {
    // Summits: ice caps where cold or moist, bare snow otherwise.
    return temp < 0.35 || moist > 0.5 ? Biome.Glacier : Biome.Snow;
  }
  if (elev > 0.82) {
    return temp < 0.45 ? Biome.Snow : Biome.Rock;
  }
  if (elev > 0.7) {
    // Alpine band: bare rock where cold/dry, grassy meadows on moist slopes.
    if (temp < 0.38) return Biome.Rock;
    if (moist > 0.38) return Biome.Alpine;
    return Biome.Rock;
  }

  // The steepest mid-elevation faces are bare rock (cliffs), regardless of climate — trees
  // can't cling to them. This carves rocky mountainsides out of the otherwise-green land.
  if (slope > 0.85) return Biome.Rock;

  // --- Low, wet, flat depressions: bog (cold) or swamp (warm) ---
  const lowland = smoothstep(seaLevel + 0.14, seaLevel + 0.02, elev); // 1 near the coast lowlands
  if (lowland > 0.5 && flow > 0.12 && moist > 0.6) {
    return temp < 0.4 ? Biome.Bog : Biome.Swamp;
  }

  // --- Whittaker temperature x moisture matrix for the bulk of the land ---
  if (temp >= 0.66) {
    // Hot: dry sand -> eroded badlands -> savanna -> scrub -> rainforest.
    if (moist < 0.24) return Biome.Desert;
    if (moist < 0.34) return Biome.Badlands;
    if (moist < 0.46) return Biome.Savanna;
    if (moist < 0.58) return Biome.Chaparral;
    return Biome.Jungle;
  }
  if (temp >= 0.48) {
    // Warm
    if (moist < 0.24) return Biome.Steppe;
    if (moist < 0.45) return Biome.Grassland;
    if (moist < 0.66) return Biome.Shrubland;
    return Biome.Forest;
  }
  if (temp >= 0.34) {
    // Temperate / cool
    if (moist < 0.3) return Biome.Steppe;
    if (moist < 0.55) return Biome.Grassland;
    return Biome.Forest;
  }
  // Cold belt.
  if (moist < 0.4) return Biome.Tundra;
  return Biome.Taiga;
}

function floraForBiome(b: Biome): FloraType | -1 {
  switch (b) {
    case Biome.Taiga:
    case Biome.Tundra:
    case Biome.Alpine:
      return FloraType.Pine;
    case Biome.Forest:
    case Biome.Grassland:
    case Biome.Shrubland:
    case Biome.Chaparral:
    case Biome.Steppe:
      return FloraType.Round;
    case Biome.Jungle:
    case Biome.Swamp:
    case Biome.Mangrove:
    case Biome.Bog:
      return FloraType.Jungle;
    case Biome.Desert:
    case Biome.Savanna:
      return FloraType.Cactus;
    case Biome.Rock:
    case Biome.Badlands:
      return FloraType.Rock;
    default:
      return -1; // ocean / beach / river / lake / snow / glacier: no flora
  }
}

/** Per-biome flora density (probability a candidate cell spawns flora). */
function floraDensity(b: Biome): number {
  switch (b) {
    case Biome.Jungle:
      return 0.55;
    case Biome.Forest:
      return 0.4;
    case Biome.Taiga:
      return 0.45;
    case Biome.Swamp:
      return 0.25;
    case Biome.Shrubland:
      return 0.18;
    case Biome.Grassland:
      return 0.08;
    case Biome.Savanna:
      return 0.05;
    case Biome.Tundra:
      return 0.06;
    case Biome.Desert:
      return 0.012;
    case Biome.Rock:
      return 0.02;
    case Biome.Mangrove:
      return 0.35;
    case Biome.Bog:
      return 0.2;
    case Biome.Alpine:
      return 0.12;
    case Biome.Chaparral:
      return 0.12;
    case Biome.Steppe:
      return 0.04;
    case Biome.Badlands:
      return 0.02;
    default:
      return 0;
  }
}

// ---- Hydraulic erosion (droplet simulation) ------------------------------------------

/**
 * Particle-based hydraulic erosion: rain droplets pick up sediment on steep descents and drop
 * it in flats, carving valleys / riverbeds and depositing plains. Mutates `h` in place; each
 * droplet erodes/deposits across the four bilinear corners of its cell. Deterministic given
 * the supplied RNG. See github.com/SebLague/Hydraulic-Erosion for the canonical formulation.
 */
function hydraulicErosion(h: Float32Array, size: number, rng: () => number, droplets: number): void {
  const inertia = 0.05;
  const capacityFactor = 4;
  const minCapacity = 0.01;
  const erodeRate = 0.3;
  const depositRate = 0.3;
  const evaporate = 0.02;
  const gravity = 4;
  const maxLifetime = 30;
  const maxIdx = size - 1;

  for (let d = 0; d < droplets; d++) {
    let posX = rng() * maxIdx;
    let posY = rng() * maxIdx;
    let dirX = 0;
    let dirY = 0;
    let speed = 1;
    let water = 1;
    let sediment = 0;

    for (let life = 0; life < maxLifetime; life++) {
      const nodeX = posX | 0;
      const nodeY = posY | 0;
      const fx = posX - nodeX;
      const fy = posY - nodeY;
      const i = nodeY * size + nodeX;

      const hNW = h[i];
      const hNE = h[i + 1];
      const hSW = h[i + size];
      const hSE = h[i + size + 1];

      const gradX = (hNE - hNW) * (1 - fy) + (hSE - hSW) * fy;
      const gradY = (hSW - hNW) * (1 - fx) + (hSE - hNE) * fx;
      const oldHeight = hNW * (1 - fx) * (1 - fy) + hNE * fx * (1 - fy) + hSW * (1 - fx) * fy + hSE * fx * fy;

      dirX = dirX * inertia - gradX * (1 - inertia);
      dirY = dirY * inertia - gradY * (1 - inertia);
      const len = Math.hypot(dirX, dirY);
      if (len !== 0) {
        dirX /= len;
        dirY /= len;
      }
      posX += dirX;
      posY += dirY;

      if ((dirX === 0 && dirY === 0) || posX < 0 || posX >= maxIdx || posY < 0 || posY >= maxIdx) break;

      const nnX = posX | 0;
      const nnY = posY | 0;
      const nfx = posX - nnX;
      const nfy = posY - nnY;
      const ni = nnY * size + nnX;
      const newHeight =
        h[ni] * (1 - nfx) * (1 - nfy) +
        h[ni + 1] * nfx * (1 - nfy) +
        h[ni + size] * (1 - nfx) * nfy +
        h[ni + size + 1] * nfx * nfy;
      const deltaHeight = newHeight - oldHeight;

      const capacity = Math.max(-deltaHeight * speed * water * capacityFactor, minCapacity);

      if (sediment > capacity || deltaHeight > 0) {
        // Deposit: fill uphill steps fully, otherwise shed the excess above capacity.
        const drop = deltaHeight > 0 ? Math.min(deltaHeight, sediment) : (sediment - capacity) * depositRate;
        sediment -= drop;
        h[i] += drop * (1 - fx) * (1 - fy);
        h[i + 1] += drop * fx * (1 - fy);
        h[i + size] += drop * (1 - fx) * fy;
        h[i + size + 1] += drop * fx * fy;
      } else {
        // Erode: take up to the remaining capacity, but never more than the local drop.
        const grab = Math.min((capacity - sediment) * erodeRate, -deltaHeight);
        h[i] -= grab * (1 - fx) * (1 - fy);
        h[i + 1] -= grab * fx * (1 - fy);
        h[i + size] -= grab * (1 - fx) * fy;
        h[i + size + 1] -= grab * fx * fy;
        sediment += grab;
      }

      speed = Math.sqrt(Math.max(0, speed * speed + deltaHeight * gravity));
      water *= 1 - evaporate;
      if (water < 0.001) break;
    }
  }
}

// ---- Lake basins (priority-flood depression filling) ---------------------------------

/**
 * Priority-Flood (Barnes et al.): flood the terrain inward from the ocean / map edge, raising
 * each cell to the highest sill it must cross to reach an outlet. Where the resulting filled
 * surface sits above the real ground, that depression holds standing water — a lake.
 *
 * The flooded cells are then grouped into connected basins (so each real lake is rendered as
 * ONE plane, not a grid of little quads). Tiny pits are dropped and only the largest basins
 * are kept, to bound draw calls. Returns the per-cell water surface (0 = dry) and the basins.
 */
function computeLakes(
  elev: Float32Array,
  size: number,
  seaLevel: number,
): { water: Float32Array; basins: LakeBasin[] } {
  const n = size * size;
  const filled = new Float32Array(n);
  const closed = new Uint8Array(n);
  // Binary min-heap keyed by fill level.
  const heapLvl = new Float32Array(n);
  const heapIdx = new Uint32Array(n);
  let heapN = 0;

  const push = (lvl: number, idx: number) => {
    let c = heapN++;
    heapLvl[c] = lvl;
    heapIdx[c] = idx;
    while (c > 0) {
      const p = (c - 1) >> 1;
      if (heapLvl[p] <= heapLvl[c]) break;
      const tl = heapLvl[p];
      heapLvl[p] = heapLvl[c];
      heapLvl[c] = tl;
      const ti = heapIdx[p];
      heapIdx[p] = heapIdx[c];
      heapIdx[c] = ti;
      c = p;
    }
  };
  const pop = (): number => {
    const top = heapIdx[0];
    heapN--;
    if (heapN > 0) {
      heapLvl[0] = heapLvl[heapN];
      heapIdx[0] = heapIdx[heapN];
      let c = 0;
      for (;;) {
        const l = 2 * c + 1;
        const r = l + 1;
        let m = c;
        if (l < heapN && heapLvl[l] < heapLvl[m]) m = l;
        if (r < heapN && heapLvl[r] < heapLvl[m]) m = r;
        if (m === c) break;
        const tl = heapLvl[m];
        heapLvl[m] = heapLvl[c];
        heapLvl[c] = tl;
        const ti = heapIdx[m];
        heapIdx[m] = heapIdx[c];
        heapIdx[c] = ti;
        c = m;
      }
    }
    return top;
  };

  // Outlets: every border cell and every ocean cell drains freely at its own height.
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = y * size + x;
      if (x === 0 || y === 0 || x === size - 1 || y === size - 1 || elev[i] < seaLevel) {
        filled[i] = elev[i];
        closed[i] = 1;
        push(elev[i], i);
      }
    }
  }

  while (heapN > 0) {
    const c = pop();
    const level = filled[c];
    const cx = c % size;
    const cy = (c / size) | 0;
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        if (dx === 0 && dy === 0) continue;
        const nx = cx + dx;
        const ny = cy + dy;
        if (nx < 0 || ny < 0 || nx >= size || ny >= size) continue;
        const ni = ny * size + nx;
        if (closed[ni]) continue;
        closed[ni] = 1;
        filled[ni] = elev[ni] > level ? elev[ni] : level;
        push(filled[ni], ni);
      }
    }
  }

  // Candidate lake cells: land depressions the flood raised above the real ground.
  const LAKE_MIN_DEPTH = 0.006; // ignore shallow pits (mostly erosion speckle)
  const MIN_LAKE_CELLS = 16; // drop ponds smaller than this (avoids speckle -> planes)
  const MAX_LAKES = 280; // cap rendered planes; keep the largest basins
  const isLakeCell = (i: number) => elev[i] >= seaLevel && filled[i] - elev[i] > LAKE_MIN_DEPTH;

  // Label connected basins (8-connectivity) with an explicit stack (no recursion on 4M cells).
  const comp = new Int32Array(n).fill(-1);
  const stack: number[] = [];
  const basinsAll: Array<{ level: number; minX: number; maxX: number; minY: number; maxY: number; count: number }> = [];
  for (let s = 0; s < n; s++) {
    if (comp[s] >= 0 || !isLakeCell(s)) continue;
    const id = basinsAll.length;
    let minX = size, maxX = 0, minY = size, maxY = 0, count = 0, level = 0;
    stack.length = 0;
    stack.push(s);
    comp[s] = id;
    while (stack.length > 0) {
      const c = stack.pop() as number;
      const cx = c % size;
      const cy = (c / size) | 0;
      if (cx < minX) minX = cx;
      if (cx > maxX) maxX = cx;
      if (cy < minY) minY = cy;
      if (cy > maxY) maxY = cy;
      if (filled[c] > level) level = filled[c]; // spill level (constant across a basin)
      count++;
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          if (dx === 0 && dy === 0) continue;
          const nx = cx + dx;
          const ny = cy + dy;
          if (nx < 0 || ny < 0 || nx >= size || ny >= size) continue;
          const ni = ny * size + nx;
          if (comp[ni] < 0 && isLakeCell(ni)) {
            comp[ni] = id;
            stack.push(ni);
          }
        }
      }
    }
    basinsAll.push({ level, minX, maxX, minY, maxY, count });
  }

  // Keep the largest qualifying basins only.
  const keptIdx = basinsAll
    .map((b, id) => ({ id, count: b.count }))
    .filter((b) => b.count >= MIN_LAKE_CELLS)
    .sort((a, b) => b.count - a.count)
    .slice(0, MAX_LAKES)
    .map((b) => b.id);
  const kept = new Uint8Array(basinsAll.length);
  keptIdx.forEach((id) => (kept[id] = 1));

  const water = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    const ci = comp[i];
    if (ci >= 0 && kept[ci]) water[i] = basinsAll[ci].level;
  }
  const basins: LakeBasin[] = keptIdx.map((id) => {
    const b = basinsAll[id];
    return { level: b.level, minX: b.minX, maxX: b.maxX, minY: b.minY, maxY: b.maxY };
  });
  return { water, basins };
}

/**
 * Distance-limited flood from every water cell (ocean or lake) so land near the water reads as
 * damp sand. Returns per-cell closeness in [0, 1]: ~1 right at the shoreline, fading to 0 a few
 * cells inland; water cells themselves are 0.
 */
function computeShore(elev: Float32Array, water: Float32Array, size: number, seaLevel: number): Float32Array {
  const n = size * size;
  const D = 8; // shore band width in cells
  const dist = new Uint8Array(n).fill(255);
  const queue = new Int32Array(n);
  let qh = 0;
  let qt = 0;
  for (let i = 0; i < n; i++) {
    if (elev[i] < seaLevel || water[i] > 0) {
      dist[i] = 0;
      queue[qt++] = i;
    }
  }
  while (qh < qt) {
    const c = queue[qh++];
    const d = dist[c];
    if (d >= D) continue;
    const cx = c % size;
    const cy = (c / size) | 0;
    for (let k = 0; k < 4; k++) {
      const nx = cx + (k === 0 ? -1 : k === 1 ? 1 : 0);
      const ny = cy + (k === 2 ? -1 : k === 3 ? 1 : 0);
      if (nx < 0 || ny < 0 || nx >= size || ny >= size) continue;
      const ni = ny * size + nx;
      if (dist[ni] > d + 1) {
        dist[ni] = d + 1;
        queue[qt++] = ni;
      }
    }
  }
  const shore = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    if (elev[i] < seaLevel || water[i] > 0) continue; // water is not sand
    const d = dist[i];
    if (d >= 1 && d <= D) shore[i] = (D - d + 1) / D;
  }
  return shore;
}

export interface WorldGenOptions {
  size?: number;
  /** 'island' = finite landmass ringed by ocean; 'continent' = land fills most of the map. */
  shape?: 'island' | 'continent';
  /** Upper bound on flora instances (keeps GPU instancing bounded on huge maps). */
  maxFlora?: number;
  /** Run droplet hydraulic erosion after base elevation (default true). */
  erosion?: boolean;
  /** Override the erosion droplet count (default scales with map area). */
  erosionDroplets?: number;
  /** Fill depressions with lake water (default true). */
  lakes?: boolean;
}

/**
 * Generate a huge SoA world. Deterministic for a given (seed, size, shape).
 * Sharp relief comes from many-octave fBm + ridged multifractal + domain warping.
 */
export function generateWorld(seed: string | number, opts: WorldGenOptions = {}): World {
  const size = opts.size ?? 1024;
  const shape = opts.shape ?? 'continent';
  const maxFlora = opts.maxFlora ?? 90000;
  const useErosion = opts.erosion ?? true;
  const useLakes = opts.lakes ?? true;
  const n = size * size;
  const baseSeed = hashSeed(seed);

  const elevNoise = new ImprovedNoise2D(baseSeed);
  const warpNoise = new ImprovedNoise2D(baseSeed ^ 0x9e3779b9);
  const ridgeNoise = new ImprovedNoise2D((baseSeed + 1013) >>> 0);
  const moistNoise = new ImprovedNoise2D((baseSeed + 7919) >>> 0);
  const tempNoise = new ImprovedNoise2D((baseSeed + 4253) >>> 0);

  const elevation = new Float32Array(n);
  const moisture = new Float32Array(n);
  const temperature = new Float32Array(n);
  const flow = new Float32Array(n);
  const biome = new Uint8Array(n);

  const seaLevel = 0.42;

  // Continental shaping: gentler falloff for a big continent, stronger for an island.
  const falloffPow = shape === 'island' ? 2.4 : 3.0;
  const falloffRadius = shape === 'island' ? 1.7 : 1.45;

  // ---- Pass 1: elevation (domain-warped fBm + ridged mountains + falloff) ----
  const FREQ = 2.4; // base feature frequency across the map (lower = larger landmasses)
  let minE = Infinity;
  let maxE = -Infinity;
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = y * size + x;
      const nx = (x / (size - 1)) * 2 - 1; // [-1, 1]
      const ny = (y / (size - 1)) * 2 - 1;

      // Domain warp: bend the sample coordinates a little to break up smooth, blobby hills
      // and give meandering coastlines — but gently, so landmasses stay cohesive.
      const wx = warpNoise.noise(nx * FREQ * 0.5 + 4.7, ny * FREQ * 0.5 + 1.3);
      const wy = warpNoise.noise(nx * FREQ * 0.5 + 8.9, ny * FREQ * 0.5 + 6.1);
      const sx = nx * FREQ + wx * 0.35;
      const sy = ny * FREQ + wy * 0.35;

      // Continental base: 8 octaves of fBm, biased upward so it reads as one big landmass
      // rather than scattered islands.
      const base = Math.min(1, (fbm(elevNoise, sx, sy, 8, 2.0, 0.5) + 1) / 2 + 0.12); // [0, 1]

      // Ridged mountains, emerging only over the higher continental interior.
      const ridge = ridged(ridgeNoise, sx * 1.4, sy * 1.4, 6, 2.0, 0.55);
      const mountainMask = smoothstep(0.5, 0.85, base);
      let e = base * 0.8 + ridge * mountainMask * 0.55;

      // Fine roughness for a less "clay" surface.
      e += fbm(elevNoise, sx * 6, sy * 6, 3, 2.0, 0.5) * 0.04;

      // Elevation curve: flatten lowlands (plains) and steepen highlands.
      e = Math.pow(Math.max(0, e), 1.25);

      // Continental falloff (island / continent edge).
      const d = Math.min(1, Math.sqrt(nx * nx + ny * ny) / falloffRadius);
      const fall = Math.max(0, 1 - Math.pow(d, falloffPow));
      e *= fall;

      elevation[i] = e;
      if (e < minE) minE = e;
      if (e > maxE) maxE = e;
    }
  }
  // Normalize to [0, 1].
  const range = maxE - minE || 1;
  for (let i = 0; i < n; i++) elevation[i] = (elevation[i] - minE) / range;

  // ---- Pass 1b: hydraulic erosion (carves valleys / riverbeds) ----
  // Runs before flow & biomes so both follow the eroded relief. Deterministic per seed.
  if (useErosion) {
    const erosionRng = mulberry32((baseSeed ^ 0x51ed270b) >>> 0);
    const droplets = opts.erosionDroplets ?? Math.min(120000, Math.floor(n * 0.06));
    hydraulicErosion(elevation, size, erosionRng, droplets);
    // Erosion nudges a few cells outside [0, 1]; clamp so downstream thresholds stay valid.
    for (let i = 0; i < n; i++) elevation[i] = elevation[i] < 0 ? 0 : elevation[i] > 1 ? 1 : elevation[i];
  }

  // ---- Pass 2: D8 flow accumulation (rivers) — no fluid sim, just graph flow ----
  // Sort cells by elevation (high -> low) and push unit rain downslope to the lowest neighbour.
  const order = new Uint32Array(n);
  for (let i = 0; i < n; i++) order[i] = i;
  // Float32 elevations: sort indices by elevation descending.
  const elevArr = elevation;
  const orderArr = Array.from(order);
  orderArr.sort((a, b) => elevArr[b] - elevArr[a]);
  for (let i = 0; i < n; i++) flow[i] = 1; // each cell starts with one unit of rain
  for (let k = 0; k < n; k++) {
    const i = orderArr[k];
    if (elevArr[i] < seaLevel) continue; // ocean drains away
    const x = i % size;
    const y = (i / size) | 0;
    let lowest = -1;
    let lowestE = elevArr[i];
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        if (dx === 0 && dy === 0) continue;
        const xx = x + dx;
        const yy = y + dy;
        if (xx < 0 || yy < 0 || xx >= size || yy >= size) continue;
        const j = yy * size + xx;
        if (elevArr[j] < lowestE) {
          lowestE = elevArr[j];
          lowest = j;
        }
      }
    }
    if (lowest >= 0) flow[lowest] += flow[i];
  }
  // Normalize flow logarithmically (a few channels carry huge accumulation).
  let maxF = 0;
  for (let i = 0; i < n; i++) {
    flow[i] = Math.log2(1 + flow[i]);
    if (flow[i] > maxF) maxF = flow[i];
  }
  if (maxF > 0) for (let i = 0; i < n; i++) flow[i] /= maxF;

  // ---- Pass 3: temperature (latitude + lapse rate) & moisture (fBm + water + flow) ----
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = y * size + x;
      const nx = (x / (size - 1)) * 2 - 1;
      const ny = (y / (size - 1)) * 2 - 1;
      const e = elevation[i];

      const lat = 1 - Math.abs(ny); // 1 at equator (map centre row), 0 at poles
      const lapse = Math.max(0, e - seaLevel) * 1.0;
      // A bit more regional noise weight spreads hot/cold belts wider across the map (not just
      // a narrow central band), so hot-dry and hot-wet biomes appear in more places.
      const tNoise = (fbm(tempNoise, nx * 2.5, ny * 2.5, 3, 2.0, 0.5) + 1) / 2;
      temperature[i] = Math.max(0, Math.min(1, lat * 0.78 + tNoise * 0.22 - lapse));

      // Two-scale moisture: large dry/wet belts (low freq) + local variation (higher freq).
      const mBelt = (fbm(moistNoise, nx * 1.3, ny * 1.3, 3, 2.0, 0.5) + 1) / 2;
      const mLocal = (fbm(moistNoise, nx * 4.0, ny * 4.0, 4, 2.0, 0.5) + 1) / 2;
      const mBase = mBelt * 0.6 + mLocal * 0.4;
      const seaProx = smoothstep(seaLevel + 0.14, seaLevel, e) * 0.18; // wetter near the coast
      const flowWet = flow[i] * 0.25;
      // Hot regions lose moisture to evaporation -> arid deserts / dry plains form there.
      // A wider moisture base lets hot cells span from parched (desert) to soaked (jungle).
      const evaporation = Math.max(0, temperature[i] - 0.5) * 0.5;
      moisture[i] = Math.max(0, Math.min(1, mBase * 0.95 + seaProx + flowWet - evaporation));
    }
  }

  // ---- Pass 3b: terrain slope (steepness) ----
  const slope = computeSlope(elevation, size);

  // ---- Pass 4: biome classification (height x moisture x temperature x slope) ----
  for (let i = 0; i < n; i++) {
    biome[i] = classify(elevation[i], temperature[i], moisture[i], flow[i], slope[i], seaLevel);
  }

  // ---- Pass 4b: lake basins (standing water in depressions) ----
  const { water, basins: lakeBasins } = useLakes
    ? computeLakes(elevation, size, seaLevel)
    : { water: new Float32Array(n), basins: [] as LakeBasin[] };
  // Recolour flooded cells as Lake so the terrain mesh + minimap read as water.
  if (useLakes) {
    for (let i = 0; i < n; i++) if (water[i] > 0) biome[i] = Biome.Lake;
  }

  // ---- Pass 4c: shoreline band (damp sand around oceans & lakes) ----
  const shore = computeShore(elevation, water, size, seaLevel);

  // ---- Pass 5: flora placement (SoA, density by biome, capped) ----
  const rng = mulberry32(baseSeed + 99173);
  const fX: number[] = [];
  const fZ: number[] = [];
  const fS: number[] = [];
  const fT: number[] = [];
  // Sample on a stride so huge maps stay within the flora budget.
  const targetCandidates = size * size;
  const stride = Math.max(1, Math.floor(Math.sqrt(targetCandidates / (maxFlora * 6))));
  for (let y = 0; y < size; y += stride) {
    for (let x = 0; x < size; x += stride) {
      if (fX.length >= maxFlora) break;
      const i = y * size + x;
      if (water[i] > 0) continue; // no trees standing in a lake
      if (slope[i] > 0.78) continue; // nothing grows on steep cliffs
      const b = biome[i] as Biome;
      const ft = floraForBiome(b);
      if (ft === -1) continue;
      // Denser where it's wetter, sparser where arid (a lush forest vs a dry plain).
      const density = floraDensity(b) * (0.5 + moisture[i]);
      if (rng() > density) continue;
      // World coordinates centred on origin (1 cell = 1 unit).
      const wx = x - size / 2 + (rng() - 0.5) * stride;
      const wz = y - size / 2 + (rng() - 0.5) * stride;
      fX.push(wx);
      fZ.push(wz);
      fS.push(0.6 + rng() * 0.8);
      fT.push(ft);
    }
  }

  return {
    size,
    seed: baseSeed,
    version: WORLD_GEN_VERSION,
    elevation,
    moisture,
    temperature,
    flow,
    slope,
    water,
    shore,
    lakeBasins,
    biome,
    floraX: new Float32Array(fX),
    floraZ: new Float32Array(fZ),
    floraScale: new Float32Array(fS),
    floraType: new Uint8Array(fT),
    floraCount: fX.length,
    seaLevel,
  };
}
