import { ImprovedNoise2D } from './terrainGenerator';

// ---------------------------------------------------------------------------------------
// Huge-scale, Structure-of-Arrays (SoA) procedural world generator.
//
// Everything is stored in flat TypedArrays (one value per cell) instead of an array of
// per-cell objects, so a 1024x1024 (1M cell) world stays a handful of MB, is GC-friendly,
// and can be persisted to IndexedDB as raw binary (see worldCache.ts).
// ---------------------------------------------------------------------------------------

export const WORLD_GEN_VERSION = 1;

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
}

export const BIOME_COUNT = 14;

/** RGB (0..255) per biome — single source of truth for colouring the world & minimap. */
export const BIOME_RGB: ReadonlyArray<readonly [number, number, number]> = [
  [28, 62, 122], // Ocean
  [222, 206, 156], // Beach
  [214, 184, 108], // Desert
  [190, 182, 96], // Savanna
  [120, 178, 86], // Grassland
  [150, 168, 96], // Shrubland
  [46, 128, 58], // Forest
  [22, 96, 44], // Jungle
  [54, 104, 82], // Taiga
  [158, 168, 150], // Tundra
  [74, 96, 70], // Swamp
  [128, 124, 120], // Rock
  [242, 246, 250], // Snow
  [60, 130, 180], // River
];

export interface World {
  size: number;
  seed: number;
  version: number;
  /** Normalized elevation in [0, 1]. */
  elevation: Float32Array;
  moisture: Float32Array; // [0, 1]
  temperature: Float32Array; // [0, 1] (cold -> hot)
  flow: Float32Array; // river flow accumulation (normalized 0..1)
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

// ---- Whittaker biome classification --------------------------------------------------

function classify(elev: number, temp: number, moist: number, flow: number, seaLevel: number): Biome {
  const beach = seaLevel + 0.015;
  if (elev < seaLevel) return Biome.Ocean;
  if (elev < beach) return Biome.Beach;

  // Rivers cut across land where flow accumulates and the slope isn't a peak.
  if (flow > 0.55 && elev < 0.78) return Biome.River;

  // High terrain: snow caps over bare alpine rock.
  if (elev > 0.86) return temp < 0.45 || moist > 0.4 ? Biome.Snow : Biome.Rock;
  if (elev > 0.72) return Biome.Rock;

  // Low, wet, warm, flat depressions become swamp/marsh.
  const lowland = smoothstep(seaLevel + 0.12, seaLevel + 0.02, elev); // 1 near the coast lowlands
  if (lowland > 0.5 && moist > 0.62 && temp > 0.4 && flow > 0.12) return Biome.Swamp;

  // Whittaker temperature x moisture matrix for the bulk of the land.
  if (temp >= 0.66) {
    if (moist < 0.28) return Biome.Desert;
    if (moist < 0.55) return Biome.Savanna;
    return Biome.Jungle;
  }
  if (temp >= 0.4) {
    if (moist < 0.3) return Biome.Grassland;
    if (moist < 0.55) return Biome.Shrubland;
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
      return FloraType.Pine;
    case Biome.Forest:
    case Biome.Grassland:
    case Biome.Shrubland:
      return FloraType.Round;
    case Biome.Jungle:
    case Biome.Swamp:
      return FloraType.Jungle;
    case Biome.Desert:
    case Biome.Savanna:
      return FloraType.Cactus;
    case Biome.Rock:
      return FloraType.Rock;
    default:
      return -1; // ocean / beach / river / snow: no flora
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
    default:
      return 0;
  }
}

export interface WorldGenOptions {
  size?: number;
  /** 'island' = finite landmass ringed by ocean; 'continent' = land fills most of the map. */
  shape?: 'island' | 'continent';
  /** Upper bound on flora instances (keeps GPU instancing bounded on huge maps). */
  maxFlora?: number;
}

/**
 * Generate a huge SoA world. Deterministic for a given (seed, size, shape).
 * Sharp relief comes from many-octave fBm + ridged multifractal + domain warping.
 */
export function generateWorld(seed: string | number, opts: WorldGenOptions = {}): World {
  const size = opts.size ?? 1024;
  const shape = opts.shape ?? 'continent';
  const maxFlora = opts.maxFlora ?? 60000;
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
      const lapse = Math.max(0, e - seaLevel) * 1.1;
      const tNoise = (fbm(tempNoise, nx * 2.5, ny * 2.5, 3, 2.0, 0.5) + 1) / 2;
      temperature[i] = Math.max(0, Math.min(1, lat * 0.85 + tNoise * 0.15 - lapse));

      // Two-scale moisture: large dry/wet belts (low freq) + local variation (higher freq).
      const mBelt = (fbm(moistNoise, nx * 1.3, ny * 1.3, 3, 2.0, 0.5) + 1) / 2;
      const mLocal = (fbm(moistNoise, nx * 4.0, ny * 4.0, 4, 2.0, 0.5) + 1) / 2;
      const mBase = mBelt * 0.6 + mLocal * 0.4;
      const seaProx = smoothstep(seaLevel + 0.14, seaLevel, e) * 0.18; // wetter near the coast
      const flowWet = flow[i] * 0.25;
      // Hot regions lose moisture to evaporation -> arid deserts / dry plains form there.
      const evaporation = Math.max(0, temperature[i] - 0.55) * 0.45;
      moisture[i] = Math.max(0, Math.min(1, mBase * 0.75 + seaProx + flowWet - evaporation));
    }
  }

  // ---- Pass 4: biome classification ----
  for (let i = 0; i < n; i++) {
    biome[i] = classify(elevation[i], temperature[i], moisture[i], flow[i], seaLevel);
  }

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
      const b = biome[i] as Biome;
      const ft = floraForBiome(b);
      if (ft === -1) continue;
      if (rng() > floraDensity(b)) continue;
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
    biome,
    floraX: new Float32Array(fX),
    floraZ: new Float32Array(fZ),
    floraScale: new Float32Array(fS),
    floraType: new Uint8Array(fT),
    floraCount: fX.length,
    seaLevel,
  };
}
