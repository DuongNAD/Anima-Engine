import { ImprovedNoise2D } from './terrainGenerator';

// ---------------------------------------------------------------------------------------
// Huge-scale, Structure-of-Arrays (SoA) procedural world generator.
//
// Everything is stored in flat TypedArrays (one value per cell) instead of an array of
// per-cell objects, so a 1024x1024 (1M cell) world stays a handful of MB, is GC-friendly,
// and can be persisted to IndexedDB as raw binary (see worldCache.ts).
// ---------------------------------------------------------------------------------------

export const WORLD_GEN_VERSION = 20;

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

/** Human-readable biome names (Vietnamese) for the explorer HUD "you are in…" banner. */
export const BIOME_NAMES_VI: ReadonlyArray<string> = [
  'Đại dương', // Ocean
  'Bãi biển', // Beach
  'Sa mạc', // Desert
  'Xavan', // Savanna
  'Đồng cỏ', // Grassland
  'Vùng cây bụi', // Shrubland
  'Rừng ôn đới', // Forest
  'Rừng nhiệt đới', // Jungle
  'Rừng taiga', // Taiga
  'Đài nguyên', // Tundra
  'Đầm lầy', // Swamp
  'Núi đá', // Rock
  'Đỉnh tuyết', // Snow
  'Dòng sông', // River
  'Hồ nước', // Lake
  'Rừng ngập mặn', // Mangrove
  'Rừng bụi khô', // Chaparral
  'Thảo nguyên', // Steppe
  'Đồng cỏ núi cao', // Alpine
  'Đất cằn', // Badlands
  'Sông băng', // Glacier
  'Đầm than bùn', // Bog
];

/** One flavour emoji per biome, shown beside the location name. */
export const BIOME_EMOJI: ReadonlyArray<string> = [
  '🌊', '🏖', '🏜', '🦁', '🌾', '🌿', '🌲', '🌴', '🌲', '❄', '🐊', '⛰', '🏔', '🏞', '💧',
  '🌴', '🍂', '🌾', '🏔', '🪨', '🧊', '🍄',
];

/** A distinct lake: one flat water plane is rendered per basin (cell-space bbox + level). */
export interface LakeBasin {
  /** Normalized water-surface elevation (constant across the basin). */
  level: number;
  minX: number;
  maxX: number;
  minY: number;
  maxY: number;
  /** Endorheic (terminal) lake in an arid basin: no outflow river, ringed by a salt flat. */
  saline?: boolean;
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
  /** River-ribbon strength per cell (0..255): 0 = dry, 255 = centre of a major river. */
  riverAmt: Uint8Array;
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
  /** Waterfalls: steep drops along river channels (cell-centred world coords + top elevation). */
  waterfallX: Float32Array;
  waterfallZ: Float32Array;
  waterfallTopE: Float32Array; // normalized elevation at the lip
  waterfallDrop: Float32Array; // normalized height of the drop
  waterfallYaw: Float32Array; // downhill direction (radians about +Y)
  waterfallCount: number;
  /** Cave mouths: dark openings set into steep rock faces. */
  caveX: Float32Array;
  caveZ: Float32Array;
  caveE: Float32Array; // normalized floor elevation of the mouth
  caveYaw: Float32Array; // downhill (outward-facing) direction
  caveCount: number;
  /** Normalized sea level (elevation below this is ocean). */
  seaLevel: number;
}

export enum FloraType {
  Pine = 0, // conifers: taiga, alpine edge
  Round = 1, // broadleaf: forest, grassland accents
  Jungle = 2, // layered tropical canopy: jungle, swamp
  Cactus = 3, // desert
  Rock = 4, // boulders on bare rock
  Acacia = 5, // umbrella-canopy savanna tree
  Palm = 6, // mangrove coasts, jungle fringe
  DeadTree = 7, // bare snag: desert, badlands
  Bush = 8, // low shrub: shrubland, chaparral, steppe
  Reed = 9, // wetland reeds: swamp, bog
  Tuft = 10, // grass tuft: grassland, steppe, alpine meadow
  // --- Aquatic (placed on the SHALLOW SEABED, seen through the transparent water) ---
  Coral = 11, // tropical reef heads: pink/orange/purple via instance tint
  Kelp = 12, // temperate kelp stands: tall olive fronds
  Seagrass = 13, // shallowest fringe meadows
}

// ---- Seeded helpers ------------------------------------------------------------------

/**
 * The seed a world identity actually generates under, as the u32 written into the World Artifact
 * header. Exported so evidence tests can bind a committed artifact's bytes to `sharedWorld.ts`
 * without regenerating a 2048² world to find out.
 */
export function hashSeed(seed: string | number): number {
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

// ---- Climate-band biome classification ------------------------------------------------
//
// The backbone is still strict and map-like (no salt-and-pepper): oceans, a thin distance-
// gated beach ribbon, and mountain caps. But the mountain lines are now drawn in LAPSED
// temperature — latitude and altitude combined — instead of raw elevation, so the snow line
// slides down towards the poles (polar ice caps at sea level) while equatorial peaks stay
// green far higher, exactly like the real Earth. The vegetated bulk in between is a full
// Whittaker matrix (temperature bands x moisture columns) fed by SMOOTH continental-scale
// climate fields (see the rain-shadow pass), so every biome forms as a large coherent
// region — a Sahara, an Amazon, a steppe belt — never as speckle.

const BEACH_TOP = 0.02; // sand strip height above the water line (very thin)
const COAST_CLIFF_SLOPE = 0.45; // a steep shore is a rock cliff, not a beach
const INLAND_CLIFF_SLOPE = 0.8; // very steep vegetated faces expose rock

// Mountain zonation in lapsed temperature: cold is cold whether it comes from the poles or
// from altitude, so one set of thresholds yields latitude-dependent snow/tree lines for free.
const T_GLACIER = 0.045; // permanent thick ice (polar shelves, the very highest peaks)
const T_SNOW = 0.125; // seasonal snow cap
const T_ROCK = 0.19; // above the treeline: bare rock and talus
const T_ALPINE = 0.26; // alpine meadow band just under the treeline

function classify(
  elev: number,
  temp: number,
  moist: number,
  flow: number,
  slope: number,
  coast: number,
  seaLevel: number,
): Biome {
  if (elev < seaLevel) return Biome.Ocean;

  // Thin coastal ribbon: the narrow strip of low, gentle land right at the OCEAN'S edge.
  // Gating by distance-to-water (coast), not just height, keeps it a thin ribbon even where
  // the plains are dead flat — otherwise a flat lowland would read as sand for miles.
  if (coast > 0.65 && elev < seaLevel + BEACH_TOP) {
    if (slope > COAST_CLIFF_SLOPE) return Biome.Rock; // cliffed coast
    // Hot, humid, calm shores silt up into mangrove forest instead of open sand.
    if (temp > 0.62 && moist > 0.6 && slope < 0.18) return Biome.Mangrove;
    return Biome.Beach;
  }

  // Mountain caps by lapsed temperature (+ a height failsafe for the tallest COLD peaks).
  // The lines are NOT ruler-straight contours: wet flanks push the snowline down (more
  // precipitation), and steep faces shed their snow (need to be colder to stay white), so
  // the caps ripple and bare-rock streaks break through on the sheer sides — like a real
  // massif instead of dip-dyed cones.
  const capJit = (moist - 0.45) * 0.06;
  if (temp < T_GLACIER + capJit * 0.5) return Biome.Glacier;
  if (temp < T_SNOW + capJit - slope * 0.05 || (elev > 0.94 && temp < T_ROCK))
    return Biome.Snow;
  if (temp < T_ROCK + capJit * 0.7 + slope * 0.04 || slope > INLAND_CLIFF_SLOPE) return Biome.Rock;

  // Rivers thread the vegetated land (they freeze over / vanish under the caps above).
  // 0.615 sits above the band of parallel hillslope rills D8 carves on smooth slopes —
  // below it, "rivers" render as an ugly diagonal hatching across every mountainside.
  if (flow > 0.615) return Biome.River;

  // Alpine meadow: the high band just under the treeline.
  if (temp < T_ALPINE + capJit * 0.5 && elev > seaLevel + 0.18 && moist > 0.25) return Biome.Alpine;

  // Wetlands: low, flat, waterlogged ground — peat bog when cold, swamp when warm.
  if (elev < seaLevel + 0.075 && moist > 0.72 && slope < 0.22 && flow > 0.18) {
    return temp < 0.32 ? Biome.Bog : Biome.Swamp;
  }
  if (temp < 0.3 && moist > 0.68 && slope < 0.15) return Biome.Bog; // upland peatland

  // --- Whittaker matrix: temperature bands (cold -> hot) x moisture columns (dry -> wet) ---
  if (temp < 0.24) return moist < 0.6 ? Biome.Tundra : Biome.Taiga; // polar fringe
  if (temp < 0.42) {
    // boreal
    if (moist < 0.16) return Biome.Steppe;
    if (moist < 0.3) return Biome.Shrubland; // forest-steppe ecotone
    return Biome.Taiga;
  }
  if (temp < 0.6) {
    // cool temperate
    if (moist < 0.14) return Biome.Desert;
    if (moist < 0.28) return Biome.Steppe;
    if (moist < 0.42) return Biome.Grassland;
    if (moist < 0.56) return Biome.Shrubland;
    return Biome.Forest;
  }
  if (temp < 0.72) {
    // warm temperate / subtropical — rugged dry country erodes into badlands
    if (moist < 0.15) return slope > 0.24 ? Biome.Badlands : Biome.Desert;
    if (moist < 0.24) return slope > 0.3 ? Biome.Badlands : Biome.Steppe;
    if (moist < 0.38) return Biome.Chaparral; // Mediterranean scrub
    if (moist < 0.52) return Biome.Grassland;
    if (moist < 0.74) return Biome.Forest;
    return Biome.Jungle;
  }
  // tropical
  if (moist < 0.18) return slope > 0.24 ? Biome.Badlands : Biome.Desert;
  if (moist < 0.38) return slope > 0.3 ? Biome.Badlands : Biome.Savanna;
  if (moist < 0.52) return Biome.Shrubland; // dry tropical woodland
  if (moist < 0.68) return Biome.Forest; // monsoon forest
  return Biome.Jungle; // rainforest
}

/**
 * Weighted flora mix per biome: every ecosystem gets its own blend of species instead of one
 * repeated prop (a savanna is scattered umbrella acacias over grass; a swamp mixes canopy
 * trees with reed beds). `r` is a uniform random draw in [0, 1).
 */
function pickFlora(b: Biome, r: number): FloraType | -1 {
  switch (b) {
    case Biome.Taiga:
      return r < 0.86 ? FloraType.Pine : r < 0.95 ? FloraType.Bush : FloraType.Rock; // glacial erratics
    case Biome.Tundra:
      return r < 0.52 ? FloraType.Bush : r < 0.92 ? FloraType.Tuft : FloraType.Rock;
    case Biome.Alpine:
      // Alpine meadow sits just under the treeline: tufts + dwarf scrub over scree, no trees.
      return r < 0.6 ? FloraType.Tuft : r < 0.9 ? FloraType.Bush : FloraType.Rock;
    case Biome.Forest:
      return r < 0.78 ? FloraType.Round : FloraType.Bush;
    case Biome.Grassland:
      return r < 0.6 ? FloraType.Tuft : r < 0.9 ? FloraType.Bush : FloraType.Round;
    case Biome.Shrubland:
      return r < 0.7 ? FloraType.Bush : FloraType.Round;
    case Biome.Chaparral:
      return r < 0.8 ? FloraType.Bush : FloraType.DeadTree; // dry scrub
    case Biome.Steppe:
      return r < 0.75 ? FloraType.Tuft : FloraType.Bush;
    case Biome.Jungle:
      return r < 0.72 ? FloraType.Jungle : FloraType.Palm;
    case Biome.Swamp:
      return r < 0.55 ? FloraType.Jungle : FloraType.Reed;
    case Biome.Bog:
      return r < 0.55 ? FloraType.Reed : FloraType.Bush;
    case Biome.Mangrove:
      return r < 0.6 ? FloraType.Palm : FloraType.Jungle;
    case Biome.Desert:
      return r < 0.6 ? FloraType.Cactus : FloraType.DeadTree;
    case Biome.Savanna:
      return r < 0.7 ? FloraType.Acacia : FloraType.Tuft;
    case Biome.Badlands:
      return FloraType.DeadTree;
    case Biome.Rock:
      return FloraType.Rock; // scattered boulders on the high bare band
    default:
      // ocean / beach / river / lake / snow / glacier: nothing grows.
      return -1;
  }
}

/** Per-biome flora density (probability a candidate cell spawns flora). */
function floraDensity(b: Biome): number {
  switch (b) {
    case Biome.Jungle:
      return 0.62;
    case Biome.Forest:
      return 0.5;
    case Biome.Taiga:
      return 0.52;
    case Biome.Swamp:
      return 0.36;
    case Biome.Shrubland:
      return 0.24;
    case Biome.Grassland:
      return 0.22;
    case Biome.Savanna:
      return 0.08;
    case Biome.Tundra:
      return 0.08;
    case Biome.Desert:
      return 0.022;
    case Biome.Rock:
      return 0.015;
    case Biome.Mangrove:
      return 0.4;
    case Biome.Bog:
      return 0.3;
    case Biome.Alpine:
      return 0.16;
    case Biome.Chaparral:
      return 0.16;
    case Biome.Steppe:
      return 0.12;
    case Biome.Badlands:
      return 0.015;
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
): { water: Float32Array; basins: LakeBasin[]; outletPaths: number[][] } {
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
  const MIN_LAKE_CELLS = 5; // small basins are kept as PONDS (rendering merges all basins
  const MAX_LAKES = 520; //   into one mesh, so more basins no longer cost draw calls)
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

  // ---- Spillway outflow routing (hydrological consistency) ------------------------------
  // Priority-Flood already knows each basin's spill level AND the depression-free `filled`
  // drainage surface (which descends monotonically to an outlet). From each kept basin's
  // lowest saddle (its pour point) we trace the overflow downhill across `filled` until it
  // reaches the ocean, another water body, or the map edge — so every lake DRAINS as a river
  // instead of sitting as a sealed puddle (conservation of water). Returns the flat list of
  // channel cell indices for the caller to stamp as rivers.
  // One traced spillway path per KEPT basin (aligned with `basins`), so the caller can decide
  // per-basin whether it actually drains (humid → river) or is a terminal salt lake (arid → none).
  const outletPaths: number[][] = basins.map(() => []);
  {
    const keptOrder = new Int32Array(basinsAll.length).fill(-1);
    keptIdx.forEach((id, k) => (keptOrder[id] = k));
    // One seed cell per kept basin (any of its lake cells).
    const seed = new Int32Array(basinsAll.length).fill(-1);
    for (let i = 0; i < n; i++) {
      const ci = comp[i];
      if (ci >= 0 && kept[ci] && seed[ci] < 0) seed[ci] = i;
    }
    // The deep lake is ringed by a shallow flooded rim that shares the same `filled == level`
    // plateau, so a lake cell's direct neighbours are never below the spill level — the escape
    // is across that rim. Grow a BFS over the whole flooded plateau (cells with filled ≈ level)
    // until it touches a cell with filled < level: that is the pour point (lowest saddle).
    const pourExit = new Int32Array(basinsAll.length).fill(-1);
    const plateauMark = new Int32Array(n).fill(-1); // basin id whose plateau BFS owns this cell
    const stack: number[] = [];
    for (let b = 0; b < basinsAll.length; b++) {
      if (!kept[b] || seed[b] < 0) continue;
      const level = basinsAll[b].level;
      let bestFilled = Infinity;
      stack.length = 0;
      stack.push(seed[b]);
      plateauMark[seed[b]] = b;
      while (stack.length > 0) {
        const c = stack.pop() as number;
        const cx = c % size;
        const cy = (c / size) | 0;
        for (let dy = -1; dy <= 1; dy++) {
          for (let dx = -1; dx <= 1; dx++) {
            if (dx === 0 && dy === 0) continue;
            const nx = cx + dx;
            const ny = cy + dy;
            if (nx < 0 || ny < 0 || nx >= size || ny >= size) continue;
            const ni = ny * size + nx;
            if (plateauMark[ni] === b) continue;
            if (filled[ni] > level + 1e-6) continue; // rim wall (real terrain) — not the plateau
            if (filled[ni] < level - 1e-6) {
              // Drains below the lake surface → a spill exit. Keep the lowest one.
              if (filled[ni] < bestFilled) {
                bestFilled = filled[ni];
                pourExit[b] = ni;
              }
              continue;
            }
            plateauMark[ni] = b; // same flooded plateau — keep growing
            stack.push(ni);
          }
        }
      }
    }
    // Trace each spill exit downhill on (filled, then elevation) — filled keeps us depression-
    // free, the elevation tiebreak drains flats without looping. Bounded by MAX_STEPS + a
    // per-channel visited mark.
    const traceMark = new Int32Array(n).fill(-1);
    const MAX_STEPS = size * 2;
    for (let b = 0; b < basinsAll.length; b++) {
      if (!kept[b] || pourExit[b] < 0) continue;
      const path = outletPaths[keptOrder[b]];
      let cur = pourExit[b];
      let steps = 0;
      while (steps++ < MAX_STEPS) {
        // The ocean, another lake, and the border are all outlets — stop there.
        if (elev[cur] < seaLevel || water[cur] > 0) break;
        const cx = cur % size;
        const cy = (cur / size) | 0;
        if (cx === 0 || cy === 0 || cx === size - 1 || cy === size - 1) break;
        if (traceMark[cur] === b) break; // revisited within this basin's trace → stop (loop guard)
        traceMark[cur] = b;
        path.push(cur);
        let best = -1;
        let bestFilled = filled[cur];
        let bestElev = elev[cur];
        for (let dy = -1; dy <= 1; dy++) {
          for (let dx = -1; dx <= 1; dx++) {
            if (dx === 0 && dy === 0) continue;
            const nx = cx + dx;
            const ny = cy + dy;
            if (nx < 0 || ny < 0 || nx >= size || ny >= size) continue;
            const ni = ny * size + nx;
            if (
              filled[ni] < bestFilled - 1e-7 ||
              (filled[ni] <= bestFilled + 1e-7 && elev[ni] < bestElev - 1e-7)
            ) {
              best = ni;
              bestFilled = filled[ni];
              bestElev = elev[ni];
            }
          }
        }
        if (best < 0) break; // no descent available (already at an outlet)
        cur = best;
      }
    }
  }

  return { water, basins, outletPaths };
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
  const maxFlora = opts.maxFlora ?? 130000;
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

  // Fraction of the map that ends up as dry land. The sea level is chosen from the actual
  // elevation histogram (below), so this target holds regardless of the noise parameters —
  // a broad continent whose deep interior sits far enough from any coast for the
  // rain-shadow sweep to dry it into steppe and true desert.
  const LAND_FRACTION = 0.38;

  // Continental shaping: gentler falloff for a big continent, stronger for an island.
  const falloffPow = shape === 'island' ? 2.4 : 3.4;
  const falloffRadius = shape === 'island' ? 1.7 : 1.55;

  // ---- Pass 1: elevation (domain-warped fBm + ridged mountains + falloff) ----
  const FREQ = 0.95; // very low base frequency -> ONE huge cohesive continent, broad plains
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

      // Continental base: 8 octaves of fBm with a gentle gain so the high-frequency octaves
      // stay small — big smooth landmasses instead of a blotchy, spongy surface.
      const base = Math.min(1, (fbm(elevNoise, sx, sy, 8, 2.0, 0.42) + 1) / 2 + 0.12); // [0, 1]

      // Ridged mountains, emerging only over the higher continental interior. The heavier
      // ridge weight gives real arêtes and serrated crests instead of smooth stacked cones.
      const ridge = ridged(ridgeNoise, sx * 1.5, sy * 1.5, 5, 2.0, 0.5);
      const mountainMask = smoothstep(0.58, 0.9, base);
      let e = base * 0.85 + ridge * mountainMask * 0.58;

      // Elevation curve: strongly flatten the lowlands into broad plains, keep peaks sharp.
      e = Math.pow(Math.max(0, e), 1.7);

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
    // Enough erosion to cut real ravines and gullies into the mountainsides (the AO bake
    // and residual normal map pick them up), while the low fBm gain keeps plains smooth.
    const droplets = opts.erosionDroplets ?? Math.min(120000, Math.floor(n * 0.03));
    hydraulicErosion(elevation, size, erosionRng, droplets);
    // Erosion nudges a few cells outside [0, 1]; clamp so downstream thresholds stay valid.
    for (let i = 0; i < n; i++) elevation[i] = elevation[i] < 0 ? 0 : elevation[i] > 1 ? 1 : elevation[i];
  }

  // Sea level = the (1 - LAND_FRACTION) elevation percentile of the FINAL (eroded) relief,
  // from a 16-bit histogram, so the land/ocean split hits the design target for any seed or
  // noise tuning.
  let seaLevel: number;
  {
    const K = 65536;
    const histo = new Uint32Array(K);
    for (let i = 0; i < n; i++) histo[Math.min(K - 1, (elevation[i] * (K - 1)) | 0)]++;
    const target = Math.floor(n * (1 - LAND_FRACTION));
    let acc = 0;
    let k = 0;
    while (k < K - 1 && acc + histo[k] < target) acc += histo[k++];
    seaLevel = k / (K - 1);
  }

  // ---- Pass 2: D8 flow accumulation (rivers) — no fluid sim, just graph flow ----
  // Sort cells by elevation (high -> low) and push unit rain downslope to the lowest neighbour.
  // A 16-bit counting sort keeps this O(n): a comparator sort over 4M boxed indices used to
  // dominate the whole generation time at 2048^2.
  const elevArr = elevation;
  const order = new Uint32Array(n);
  {
    const K = 65536;
    const q = new Uint16Array(n);
    const counts = new Uint32Array(K + 1);
    for (let i = 0; i < n; i++) {
      const v = Math.min(K - 1, (elevArr[i] * (K - 1)) | 0);
      q[i] = v;
      counts[v + 1]++;
    }
    for (let k = 0; k < K; k++) counts[k + 1] += counts[k];
    for (let i = 0; i < n; i++) order[counts[q[i]]++] = i; // ascending elevation
  }
  for (let i = 0; i < n; i++) flow[i] = 1; // each cell starts with one unit of rain
  for (let k = n - 1; k >= 0; k--) {
    const i = order[k]; // descending elevation
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

  // ---- Pass 2b: river ribbons + channel carving ----
  // Raw D8 channels are 1-cell zigzag chains — they render as jagged pixel threads. Stamping
  // a flow-scaled disc along each channel turns them into smooth ribbons that WIDEN
  // DOWNSTREAM and taper at the heads; a shallow carve then sinks each ribbon into a real
  // bed, so rivers sit IN the terrain (and the residual normal map shades their banks).
  const riverAmt = new Uint8Array(n);
  {
    const CORE_T = 0.615; // matches the River biome cut — above the hillslope-rill band
    const widthScale = size / 1024; // same physical width at any data resolution
    // Meander wiggle: D8 channels run dead straight along grid diagonals, and parallel
    // channels on a uniform slope render as a mechanical hatching. Warping each stamp by a
    // smooth noise field bends every channel into its own wavy course (neighbouring cells
    // share almost the same offset, so a channel stays connected).
    const wig = 1.6 * widthScale;
    for (let y = 1; y < size - 1; y++) {
      const lat = 1 - Math.abs((y / (size - 1)) * 2 - 1);
      for (let x = 1; x < size - 1; x++) {
        const i = y * size + x;
        const f = flow[i];
        if (f < CORE_T || elevation[i] < seaLevel) continue;
        // Frozen heights carry no liquid rivers: without this gate the ribbons still CARVE
        // grooves under the snow caps, which show through the normal map as the parallel
        // fall-line striping all over every snowy mountainside.
        const tApprox = lat * 0.82 + 0.09 - Math.max(0, elevation[i] - seaLevel) * 1.35;
        if (tApprox < 0.2) continue;
        // On very steep faces only substantial rivers keep their ribbon: thin fall-line
        // channels there stack into a dense parallel striping across every mountainside.
        const grad =
          Math.abs(elevation[i + 1] - elevation[i - 1]) + Math.abs(elevation[i + size] - elevation[i - size]);
        if (grad > 0.004 * (2048 / size) && f < 0.72) continue;
        // Squared ramp: only the true trunk rivers get wide — a thin thread at the spring,
        // swelling with every confluence, a broad band by the time it reaches the plain.
        const s0 = smoothstep(CORE_T, 1.0, f);
        const strength = s0 * s0;
        const rad = 0.4 + strength * widthScale * 2.6;
        const r = Math.ceil(rad);
        const amt0 = 90 + 165 * strength;
        const cx = x + Math.round(warpNoise.noise(x * 0.18 + 31.7, y * 0.18 + 77.1) * wig);
        const cy = y + Math.round(warpNoise.noise(x * 0.18 + 90.2, y * 0.18 + 12.9) * wig);
        for (let dy = -r; dy <= r; dy++) {
          const yy = cy + dy;
          if (yy < 0 || yy >= size) continue;
          for (let dx = -r; dx <= r; dx++) {
            const xx = cx + dx;
            if (xx < 0 || xx >= size) continue;
            const d = Math.sqrt(dx * dx + dy * dy);
            const edge = rad + 0.5 - d; // soft 1-cell feather at the bank
            if (edge <= 0) continue;
            const j = yy * size + xx;
            const a = Math.round(amt0 * Math.min(1, edge));
            if (a > riverAmt[j]) riverAmt[j] = a;
          }
        }
      }
    }
    // Carve the beds (kept well under LAKE_MIN_DEPTH so channels don't read as lakes).
    for (let i = 0; i < n; i++) {
      if (riverAmt[i] === 0) continue;
      const e = elevation[i];
      if (e < seaLevel) continue;
      elevation[i] = Math.max(seaLevel - 0.002, e - 0.0035 * (riverAmt[i] / 255));
    }
  }

  // ---- Pass 3: climate — temperature (latitude + altitude lapse) & moisture -------------
  //
  // Moisture is built like real weather instead of pure value-noise. Prevailing winds sweep
  // humid ocean air across the land; rising terrain wrings the moisture out (orographic
  // rain), so the far side of a mountain range sits in a RAIN SHADOW — that is where the
  // deserts form, while windward coasts stay lush. The wind direction follows Earth-like
  // bands: trade winds (east -> west) in the tropics, westerlies in the mid-latitudes,
  // polar easterlies near the map's edges. Because the resulting field is smooth at
  // continental scale, the Whittaker classifier produces LARGE coherent biome regions.

  // Orographic sweep: humidity left in the air after crossing the terrain, per direction.
  const humW = new Float32Array(n); // air blown eastward (+x): westerlies
  const humE = new Float32Array(n); // air blown westward (-x): trades / polar easterlies
  {
    // Per-cell rates scale with resolution so the sweep depends on WORLD distance, not on
    // how many cells happen to subdivide it (a 2048 map must not dry out twice as fast).
    const rate = 1024 / size;
    const RISE = 2.6; // per unit climb — resolution-independent already
    const DRIZZLE = 0.01 * rate; // baseline rain-out over land
    const LAND_ET = 0.0015 * rate; // faint evapotranspiration — deep interiors still dry out
    const OCEAN_WET = 0.055 * rate; // open water re-saturates the air quickly
    for (let y = 0; y < size; y++) {
      let hum = 1;
      let prev = seaLevel;
      for (let x = 0; x < size; x++) {
        const i = y * size + x;
        const e = elevation[i];
        if (e < seaLevel) {
          hum = Math.min(1, hum + OCEAN_WET);
          prev = seaLevel;
          humW[i] = hum;
          continue;
        }
        hum = Math.max(0, hum - hum * (DRIZZLE + Math.max(0, e - prev) * RISE) + LAND_ET * (1 - hum));
        humW[i] = hum;
        prev = e;
      }
      hum = 1;
      prev = seaLevel;
      for (let x = size - 1; x >= 0; x--) {
        const i = y * size + x;
        const e = elevation[i];
        if (e < seaLevel) {
          hum = Math.min(1, hum + OCEAN_WET);
          prev = seaLevel;
          humE[i] = hum;
          continue;
        }
        hum = Math.max(0, hum - hum * (DRIZZLE + Math.max(0, e - prev) * RISE) + LAND_ET * (1 - hum));
        humE[i] = hum;
        prev = e;
      }
    }
  }

  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = y * size + x;
      const nx = (x / (size - 1)) * 2 - 1;
      const ny = (y / (size - 1)) * 2 - 1;
      const e = elevation[i];

      const lat = 1 - Math.abs(ny); // 1 at equator (map centre row), 0 at poles
      // Altitude cooling steep enough that the very highest equatorial peaks still freeze
      // (Kilimanjaro-style), while polar lowlands sit below the snow line on their own.
      const lapse = Math.max(0, e - seaLevel) * 1.35;
      const tNoise = (fbm(tempNoise, nx * 2.2, ny * 2.2, 3, 2.0, 0.5) + 1) / 2;
      temperature[i] = Math.max(0, Math.min(1, lat * 0.82 + tNoise * 0.18 - lapse));

      // Earth-like wind bands select which sweep feeds this latitude (smoothly blended).
      const trades = smoothstep(0.62, 0.75, lat); // tropics: air arrives from the east
      const polar = 1 - smoothstep(0.18, 0.3, lat); // polar fringe: easterlies again
      const wEast = Math.min(1, trades + polar);
      const hum = humE[i] * wEast + humW[i] * (1 - wEast);

      // Low-frequency belts + a little local detail keep region borders organic.
      const mBelt = (fbm(moistNoise, nx * 1.1, ny * 1.1, 3, 2.0, 0.5) + 1) / 2;
      const mLocal = (fbm(moistNoise, nx * 5.0, ny * 5.0, 3, 2.0, 0.5) + 1) / 2;
      const seaProx = smoothstep(seaLevel + 0.12, seaLevel, e) * 0.12; // maritime coasts
      const flowWet = flow[i] * 0.15; // river corridors stay green
      const evaporation = Math.max(0, temperature[i] - 0.55) * 0.22; // hot air dries the soil
      // Hadley circulation: descending dry air parks a desert belt over the subtropics
      // (Earth's Sahara / Arabian / Australian deserts), while the rising equatorial air
      // (ITCZ) keeps the deep tropics rain-soaked. This guarantees a hot-arid belt and an
      // equatorial rainforest belt on every seed, right where the real Earth grows them.
      const dLat = lat - 0.62;
      const hadleyDry = Math.exp(-(dLat * dLat) / 0.0162) * 0.16;
      const itczWet = smoothstep(0.85, 1.0, lat) * 0.1;
      // Rain shadow dominates; the noise belts only modulate it, so a leeward interior can
      // dry all the way down into desert instead of being floored by the belt average.
      moisture[i] = Math.max(
        0,
        Math.min(
          1,
          hum * 0.62 + mBelt * 0.22 + mLocal * 0.12 + seaProx + flowWet + itczWet - evaporation - hadleyDry,
        ),
      );
    }
  }

  // ---- Pass 3b: terrain slope (steepness) ----
  const slope = computeSlope(elevation, size);

  // ---- Pass 3c: distance to the OCEAN (closeness 0..1) — keeps the beach a thin ribbon ----
  // Empty water arg => sources are ocean cells only (lakes get their own biome later).
  const coast = computeShore(elevation, new Float32Array(0), size, seaLevel);

  // ---- Pass 4: biome classification (climate bands x height x slope x coast) ----
  for (let i = 0; i < n; i++) {
    biome[i] = classify(elevation[i], temperature[i], moisture[i], flow[i], slope[i], coast[i], seaLevel);
  }
  // The widened river ribbon becomes the River biome (so the minimap, colour bake and flora
  // rules all see the same ribbon, not the raw 1-cell channel). Ice stays frozen over.
  for (let i = 0; i < n; i++) {
    if (riverAmt[i] < 140 || elevation[i] < seaLevel) continue;
    const b = biome[i];
    if (b === Biome.Glacier || b === Biome.Snow || b === Biome.Ocean || b === Biome.Lake) continue;
    biome[i] = Biome.River;
  }

  // ---- Pass 4a: 3x3 majority filter over the vegetated land ----
  // Where two climate fields sit right on a threshold, classification flickers cell-to-cell;
  // one mode-filter pass absorbs those single-cell speckles into the surrounding region.
  // Water, rivers, the beach ribbon, mangrove fringes and ice caps are legitimately thin
  // features — they neither vote nor change.
  {
    const KEEP =
      (1 << Biome.Ocean) |
      (1 << Biome.River) |
      (1 << Biome.Lake) |
      (1 << Biome.Beach) |
      (1 << Biome.Mangrove) |
      (1 << Biome.Glacier) |
      (1 << Biome.Snow);
    const src = new Uint8Array(biome); // vote against the unfiltered snapshot
    const counts = new Uint8Array(BIOME_COUNT);
    const seen: number[] = [];
    for (let y = 1; y < size - 1; y++) {
      for (let x = 1; x < size - 1; x++) {
        const i = y * size + x;
        const b = src[i];
        if (KEEP & (1 << b)) continue;
        seen.length = 0;
        let best = b;
        let bestCount = 0;
        for (let dy = -1; dy <= 1; dy++) {
          for (let dx = -1; dx <= 1; dx++) {
            const nb = src[i + dy * size + dx];
            if (KEEP & (1 << nb)) continue; // thin features don't vote
            if (counts[nb] === 0) seen.push(nb);
            const c = ++counts[nb];
            if (c > bestCount) {
              bestCount = c;
              best = nb;
            }
          }
        }
        for (let k = 0; k < seen.length; k++) counts[seen[k]] = 0;
        if (best !== b && bestCount >= 5) biome[i] = best;
      }
    }
  }

  // ---- Pass 4b: lake basins (standing water in depressions) ----
  const { water, basins: lakeBasins, outletPaths } = useLakes
    ? computeLakes(elevation, size, seaLevel)
    : { water: new Float32Array(n), basins: [] as LakeBasin[], outletPaths: [] as number[][] };
  // Recolour flooded cells as Lake so the terrain mesh + minimap read as water.
  if (useLakes) {
    for (let i = 0; i < n; i++) if (water[i] > 0) biome[i] = Biome.Lake;
  }

  // ---- Pass 4b-2: lake drainage — humid basins spill a river; arid basins are terminal salt lakes ----
  // computeLakes traced each basin's overflow downhill, but whether that overflow EXISTS depends on
  // the water balance. In a wet basin the lake overtops its sill and feeds a river (exorheic). In an
  // arid basin evaporation removes the inflow before it can spill, so the lake is ENDORHEIC — a
  // terminal salt lake (Dead Sea / Great Salt Lake) with no outflow, ringed by a salt flat. We decide
  // per basin from its mean moisture.
  if (useLakes) {
    const ENDORHEIC_MOISTURE = 0.24; // drier than this → the basin cannot overflow → terminal
    for (let k = 0; k < lakeBasins.length; k++) {
      const lb = lakeBasins[k];
      let msum = 0;
      let mcount = 0;
      for (let y = lb.minY; y <= lb.maxY; y++) {
        for (let x = lb.minX; x <= lb.maxX; x++) {
          const i = y * size + x;
          if (water[i] > 0) {
            msum += moisture[i];
            mcount++;
          }
        }
      }
      const meanMoist = mcount > 0 ? msum / mcount : 1;

      if (meanMoist < ENDORHEIC_MOISTURE) {
        // Endorheic: no outflow. Ring the lake with a thin salt flat (reuse Beach = pale sand).
        lb.saline = true;
        const y0 = Math.max(1, lb.minY - 1);
        const y1 = Math.min(size - 2, lb.maxY + 1);
        const x0 = Math.max(1, lb.minX - 1);
        const x1 = Math.min(size - 2, lb.maxX + 1);
        for (let y = y0; y <= y1; y++) {
          for (let x = x0; x <= x1; x++) {
            const i = y * size + x;
            if (water[i] > 0 || elevation[i] < seaLevel) continue;
            const b = biome[i];
            if (b === Biome.Lake || b === Biome.River || b === Biome.Glacier || b === Biome.Snow) continue;
            let touchesLake = false;
            for (let dy = -1; dy <= 1 && !touchesLake; dy++) {
              for (let dx = -1; dx <= 1; dx++) {
                if (water[(y + dy) * size + (x + dx)] > 0) {
                  touchesLake = true;
                  break;
                }
              }
            }
            if (touchesLake) biome[i] = Biome.Beach; // salt-flat ring around the terminal lake
          }
        }
        continue; // this lake does not drain — skip its spillway
      }

      // Exorheic: stamp the traced spillway as an outflow river (mask + biome).
      const path = outletPaths[k] ?? [];
      for (let p = 0; p < path.length; p++) {
        const i = path[p];
        if (elevation[i] < seaLevel || water[i] > 0) continue;
        const b = biome[i];
        if (b === Biome.Glacier || b === Biome.Snow || b === Biome.Ocean || b === Biome.Lake) continue;
        if (riverAmt[i] < 170) riverAmt[i] = 170; // visible outflow stream (mask + ribbon tint)
        biome[i] = Biome.River;
      }
    }
  }

  // ---- Pass 4c: shoreline band (damp sand around oceans & lakes) ----
  const shore = computeShore(elevation, water, size, seaLevel);

  // ---- Pass 4d: river-mouth deltas — sandy sediment fans where rivers meet the sea ----
  // A river drops its sediment load as it slows entering the ocean, shoaling the seabed and raising
  // emergent sandy bars: a delta. We deposit a small flow-scaled fan into the near-shore shallows at
  // each river mouth; cells raised above sea level become sandy Beach lobes, the rest stay bright
  // sandy shallows. (Deltas legitimately add a little land, so the land fraction ticks up slightly.)
  if (useLakes) {
    const DELTA_MIN_RIVER = 150; // only substantial rivers build a delta
    const DELTA_MAX_DEPTH = 0.03; // deposit only into the near-shore shallows
    const DELTA_DEPOSIT = 0.05; // max sediment thickness at the mouth
    const mouths: number[] = [];
    for (let y = 1; y < size - 1; y++) {
      for (let x = 1; x < size - 1; x++) {
        const i = y * size + x;
        if (riverAmt[i] < DELTA_MIN_RIVER || elevation[i] < seaLevel) continue;
        let sea = false;
        for (let dy = -1; dy <= 1 && !sea; dy++) {
          for (let dx = -1; dx <= 1; dx++) {
            const j = (y + dy) * size + (x + dx);
            if (elevation[j] < seaLevel && water[j] === 0) {
              sea = true;
              break;
            }
          }
        }
        if (sea) mouths.push(i);
      }
    }
    for (let m = 0; m < mouths.length; m++) {
      const mi = mouths[m];
      const strength = Math.min(1, riverAmt[mi] / 255);
      const rad = 1 + Math.round(strength * 3);
      const mx = mi % size;
      const my = (mi / size) | 0;
      for (let dy = -rad; dy <= rad; dy++) {
        const yy = my + dy;
        if (yy < 1 || yy >= size - 1) continue;
        for (let dx = -rad; dx <= rad; dx++) {
          const xx = mx + dx;
          if (xx < 1 || xx >= size - 1) continue;
          const d = Math.sqrt(dx * dx + dy * dy);
          if (d > rad + 0.5) continue;
          const j = yy * size + xx;
          if (elevation[j] >= seaLevel || water[j] > 0) continue; // only deposit into open sea
          if (seaLevel - elevation[j] > DELTA_MAX_DEPTH) continue; // hug the coast
          const fan = (1 - d / (rad + 0.5)) * strength;
          elevation[j] = Math.min(elevation[j] + fan * DELTA_DEPOSIT, seaLevel + 0.004);
          if (elevation[j] >= seaLevel) biome[j] = Biome.Beach; // emergent sandy delta lobe
        }
      }
    }
  }

  // ---- Pass 4e: waterfalls — steep drops along river channels ----
  // A river cell whose steepest descent exceeds MIN_DROP reads as a waterfall: keep the lip
  // position, drop height and downhill bearing so the renderer can hang a foam curtain on the
  // face. Falls are kept biggest-first with a spacing filter so one long cascade doesn't
  // spawn a ladder of overlapping curtains.
  const wfX: number[] = [];
  const wfZ: number[] = [];
  const wfTopE: number[] = [];
  const wfDrop: number[] = [];
  const wfYaw: number[] = [];
  {
    // Per-CELL drop threshold: halving the cell size halves each step's drop, so scale by
    // resolution to keep the same physical cliff height qualifying at any WORLD_SIZE.
    const MIN_DROP = 0.009 * (1024 / size);
    const MAX_FALLS = 450;
    const cand: Array<{ x: number; z: number; e: number; d: number; yaw: number }> = [];
    for (let y = 1; y < size - 1; y++) {
      for (let x = 1; x < size - 1; x++) {
        const i = y * size + x;
        if (biome[i] !== Biome.River || water[i] > 0) continue;
        let drop = 0;
        let dirX = 0;
        let dirY = 0;
        for (let dy = -1; dy <= 1; dy++) {
          for (let dx = -1; dx <= 1; dx++) {
            if (dx === 0 && dy === 0) continue;
            const d = elevation[i] - elevation[(y + dy) * size + (x + dx)];
            if (d > drop) {
              drop = d;
              dirX = dx;
              dirY = dy;
            }
          }
        }
        if (drop < MIN_DROP) continue;
        cand.push({
          x: x - size / 2 + dirX * 0.5,
          z: y - size / 2 + dirY * 0.5,
          e: elevation[i],
          d: drop,
          yaw: Math.atan2(dirY, dirX),
        });
      }
    }
    cand.sort((a, b) => b.d - a.d);
    const takenX: number[] = [];
    const takenZ: number[] = [];
    for (const c of cand) {
      if (wfX.length >= MAX_FALLS) break;
      let crowded = false;
      for (let k = 0; k < takenX.length; k++) {
        const ddx = takenX[k] - c.x;
        const ddz = takenZ[k] - c.z;
        if (ddx * ddx + ddz * ddz < 9) {
          crowded = true;
          break;
        }
      }
      if (crowded) continue;
      takenX.push(c.x);
      takenZ.push(c.z);
      wfX.push(c.x);
      wfZ.push(c.z);
      wfTopE.push(c.e);
      wfDrop.push(c.d);
      wfYaw.push(c.yaw);
    }
  }

  // ---- Pass 4f: cave mouths — dark openings set into steep bare-rock faces ----
  const cvX: number[] = [];
  const cvZ: number[] = [];
  const cvE: number[] = [];
  const cvYaw: number[] = [];
  {
    const caveRng = mulberry32((baseSeed ^ 0x7ac0beef) >>> 0);
    const MAX_CAVES = 80;
    outer: for (let y = 2; y < size - 2; y += 2) {
      const lat = 1 - Math.abs((y / (size - 1)) * 2 - 1);
      for (let x = 2; x < size - 2; x += 2) {
        const i = y * size + x;
        // Bare rock and eroded badlands host openings; needs a proper face to sink into.
        // Sparse and NEVER up in the frozen band — dark mouths against snow-bright rock
        // read as floating black-dot glitches from any distance.
        if (biome[i] !== Biome.Rock && biome[i] !== Biome.Badlands) continue;
        if (slope[i] < 0.45) continue;
        if (elevation[i] < seaLevel + 0.02) continue;
        if (lat * 0.82 + 0.09 - Math.max(0, elevation[i] - seaLevel) * 1.35 < 0.135) continue; // not on snow
        if (caveRng() > 0.018) continue;
        // Outward (downhill) bearing from the local gradient, so the mouth faces out of the hill.
        const dEdx = (elevation[i + 1] - elevation[i - 1]) * 0.5;
        const dEdz = (elevation[i + size] - elevation[i - size]) * 0.5;
        cvX.push(x - size / 2);
        cvZ.push(y - size / 2);
        cvE.push(elevation[i]);
        cvYaw.push(Math.atan2(-dEdz, -dEdx));
        if (cvX.length >= MAX_CAVES) break outer;
      }
    }
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
      // Hard exclusions — only vegetated dry land, clear of water, cliffs and the waterline:
      if (elevation[i] <= seaLevel) continue; // must be above sea level
      if (water[i] > 0) continue; // never in a lake
      if (riverAmt[i] > 100) continue; // never in a river ribbon
      if (slope[i] > 0.78) continue; // never on steep cliffs
      if (shore[i] > 0.8) continue; // never right at the water's edge / on the beach
      const b = biome[i] as Biome;
      // Denser where it's wetter — and MUCH denser along water: shore[] rises toward lakes
      // and the sea, flow[] rises toward river channels, so gallery forest crowds the banks
      // and even deserts grow an oasis fringe along their wadis.
      const waterBoost = 1 + shore[i] * 1.1 + Math.min(1, flow[i] * 1.6) * 0.9;
      const density = floraDensity(b) * (0.5 + moisture[i]) * waterBoost;
      if (rng() > density) continue;
      // pickFlora returns -1 for ocean / beach / river / lake / snow / glacier.
      let ft = pickFlora(b, rng());
      if (ft === -1) continue;
      // Above the treeline nothing tall can root: any tree that would land in the bare-rock
      // band or colder (temp < T_ROCK — the same treeline classify() uses for the Rock cap)
      // becomes alpine ground cover instead, so pines never climb onto the snow caps.
      const isTallFlora =
        ft === FloraType.Pine ||
        ft === FloraType.Round ||
        ft === FloraType.Jungle ||
        ft === FloraType.Acacia ||
        ft === FloraType.Palm ||
        ft === FloraType.Cactus ||
        ft === FloraType.DeadTree;
      if (isTallFlora && temperature[i] < T_ROCK) {
        ft = FloraType.Tuft;
      } else if (
        // Tall trees cannot root on steep faces either — scrub takes over near cliffs.
        slope[i] > 0.55 &&
        (ft === FloraType.Pine || ft === FloraType.Round || ft === FloraType.Jungle || ft === FloraType.Acacia || ft === FloraType.Palm)
      ) {
        ft = FloraType.Bush;
      }
      // World coordinates centred on origin (1 cell = 1 unit).
      const wx = x - size / 2 + (rng() - 0.5) * stride;
      const wz = y - size / 2 + (rng() - 0.5) * stride;
      // Re-check the jittered landing cell so nothing drifts onto water/sea.
      const jx = Math.min(size - 1, Math.max(0, Math.round(wx + size / 2)));
      const jz = Math.min(size - 1, Math.max(0, Math.round(wz + size / 2)));
      const ji = jz * size + jx;
      if (water[ji] > 0 || elevation[ji] <= seaLevel || riverAmt[ji] > 100) continue;
      fX.push(wx);
      fZ.push(wz);
      fS.push(0.6 + rng() * 0.8);
      fT.push(ft);
    }
  }

  // ---- Pass 5b: aquatic flora — the underwater ecosystem of the sunlit shelf ----
  // Coral heads crowd the warm tropical shallows, kelp stands sway in the cooler temperate
  // water, and seagrass meadows carpet the shallowest fringe. All sit ON the seabed mesh and
  // are seen through the transparent shallow water.
  {
    const aquaRng = mulberry32((baseSeed + 445566) >>> 0);
    const MAX_AQUA = 22000;
    let placed = 0;
    outer: for (let y = 0; y < size; y += stride) {
      for (let x = 0; x < size; x += stride) {
        if (placed >= MAX_AQUA) break outer;
        const i = y * size + x;
        const e = elevation[i];
        if (e >= seaLevel) continue; // ocean floor only
        if (water[i] > 0) continue;
        const depth = seaLevel - e;
        if (depth > 0.085) continue; // below the photic shelf: too dark to matter visually
        const t = temperature[i];
        let ft: FloraType | -1 = -1;
        let dens = 0;
        if (depth <= 0.018) {
          if (t > 0.35) {
            ft = FloraType.Seagrass;
            dens = 0.16;
          }
        } else if (t > 0.6 && depth < 0.055) {
          ft = FloraType.Coral;
          dens = 0.24; // dense, patchy reef band
        } else if (t > 0.28 && t <= 0.6) {
          ft = FloraType.Kelp;
          dens = 0.11;
        }
        if (ft === -1 || aquaRng() > dens) continue;
        const wx = x - size / 2 + (aquaRng() - 0.5) * stride;
        const wz = y - size / 2 + (aquaRng() - 0.5) * stride;
        const jx = Math.min(size - 1, Math.max(0, Math.round(wx + size / 2)));
        const jz = Math.min(size - 1, Math.max(0, Math.round(wz + size / 2)));
        const ji = jz * size + jx;
        if (elevation[ji] >= seaLevel || water[ji] > 0) continue; // stay submerged after jitter
        fX.push(wx);
        fZ.push(wz);
        fS.push(0.6 + aquaRng() * 0.9);
        fT.push(ft);
        placed++;
      }
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
    riverAmt,
    shore,
    lakeBasins,
    biome,
    floraX: new Float32Array(fX),
    floraZ: new Float32Array(fZ),
    floraScale: new Float32Array(fS),
    floraType: new Uint8Array(fT),
    floraCount: fX.length,
    waterfallX: new Float32Array(wfX),
    waterfallZ: new Float32Array(wfZ),
    waterfallTopE: new Float32Array(wfTopE),
    waterfallDrop: new Float32Array(wfDrop),
    waterfallYaw: new Float32Array(wfYaw),
    waterfallCount: wfX.length,
    caveX: new Float32Array(cvX),
    caveZ: new Float32Array(cvZ),
    caveE: new Float32Array(cvE),
    caveYaw: new Float32Array(cvYaw),
    caveCount: cvX.length,
    seaLevel,
  };
}
