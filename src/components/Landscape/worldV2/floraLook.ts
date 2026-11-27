import * as THREE from 'three';
import { mergeGeometries, mergeVertices } from 'three/examples/jsm/utils/BufferGeometryUtils.js';

// prototype, to be replaced — WV2-LOOK flora shapes for World v2 decorative flora.
//
// Separate from `utils/floraGeometry.ts` on purpose: that module's footprints are measured by
// `check:flora-footprint` and feed the walk-mode colliders/spawn clearance. These shapes are
// decoration only, built at unit height (y = 0..1) and scaled per instance to real-world
// metres, so they never change a collider.
//
// Budget: 18-34 triangles per plant. A plant is a few pixels tall at the canonical view
// distances, so silhouette, size and colour variation matter far more than facets.

/**
 * Bake species ids exactly as `bake/flora_place.rs` `FloraSpeciesId` writes them into ANMF.
 * r1-r8 read these ids as look shapes directly, so coastal rainforest drew as pines, mangroves
 * as boulders and paramo espeletia as palms (r9 fix). A bake id is an ARCHETYPE for its biome;
 * `chooseLookShape` turns it into a plant that fits the local habitat.
 */
export const BAKE_SPECIES = {
  RainforestCanopy: 0,
  MontaneBroadleaf: 1,
  ElfinConifer: 2,
  DryThornScrub: 3,
  MangroveProp: 4,
  SavannaAcacia: 5,
  ParamoEspeletia: 6,
  RiparianWillow: 7,
} as const;
export const BAKE_SPECIES_COUNT = 8;

/** Look shapes: one instanced batch (one draw call) each. */
export const LOOK_SPECIES = {
  Pine: 0,
  Broadleaf: 1,
  Jungle: 2,
  Cactus: 3,
  Rock: 4,
  Acacia: 5,
  Palm: 6,
  Bush: 7,
  Mangrove: 8,
  Rosette: 9,
  Willow: 10,
  TreeFern: 11,
} as const;
export const LOOK_SHAPE_COUNT = 12;

/** Height range in metres per look shape, and the canopy width / height ratio. */
export const LOOK_SPECIES_SIZE: ReadonlyArray<{ minM: number; maxM: number; widthRatio: number }> = [
  { minM: 14, maxM: 32, widthRatio: 0.42 }, // 0 pine / podocarp
  { minM: 10, maxM: 24, widthRatio: 0.8 }, // 1 broadleaf
  { minM: 20, maxM: 38, widthRatio: 0.75 }, // 2 jungle emergent
  { minM: 4, maxM: 9, widthRatio: 0.45 }, // 3 cactus
  { minM: 3, maxM: 11, widthRatio: 1.4 }, // 4 boulder
  { minM: 6, maxM: 12, widthRatio: 1.5 }, // 5 acacia
  { minM: 9, maxM: 18, widthRatio: 0.7 }, // 6 palm
  { minM: 2.5, maxM: 5.5, widthRatio: 1.5 }, // 7 shrub
  { minM: 4, maxM: 12, widthRatio: 1.25 }, // 8 mangrove (prop roots, low dense crown)
  { minM: 1, maxM: 3.2, widthRatio: 0.55 }, // 9 rosette (espeletia, frailejon)
  { minM: 8, maxM: 16, widthRatio: 1.0 }, // 10 willow (riparian)
  { minM: 3, maxM: 8, widthRatio: 0.95 }, // 11 tree fern (understory)
];

/** Canonical biome ids used by the habitat rules (`BIOME_SRGB_PALETTE` order). */
const BIOME = { Desert: 2, Savanna: 3, Badlands: 19, Alpine: 18 } as const;

/** Local habitat of one plant, measured on the drawn terrain. */
export interface FloraHabitat {
  /** Ground height above sea level, metres. */
  elevM: number;
  /** Horizontal distance to the nearest sea, metres. */
  distSeaM: number;
  /** 1 - normal.y of the ground (0 flat, ~0.3 cliff-steep). */
  slope: number;
  /** Canonical biome id of the cell. */
  biome: number;
}

/** Upper limit of trees on this tropical world (humid tropics: ~3,500-4,000 m). */
export const TREELINE_M = 3600;
/** Conifers (podocarps, elfin conifers) only in the cool montane belt, never on the lowland coast. */
export const CONIFER_MIN_ELEV_M = 600;
/** Bare sand next to the water: no woody plants (dune grass comes from the ground cover). */
export const BEACH_BARE_M = 45;
/** Salt-spray strand zone: palms and strand scrub instead of the inland forest. */
export const STRAND_M = 220;
/** Ground steeper than this holds no trees; some of it holds boulders (talus). */
export const CLIFF_SLOPE = 0.32;

/**
 * Pick the plant drawn for one bake point member, or -1 for nothing. `r` is a uniform [0,1) draw.
 * Rules follow the seaward-to-inland zonation of tropical coasts (bare beach, strand scrub with
 * palms, littoral forest; mangroves only on the tidal flats) and the altitudinal belts of a humid
 * tropical mountain (lowland rainforest, montane broadleaf with tree ferns, conifers only above
 * ~600 m, paramo rosettes and scrub above the treeline).
 */
export function chooseLookShape(bakeId: number, env: FloraHabitat, r: number): number {
  const S = LOOK_SPECIES;
  if (env.slope > CLIFF_SLOPE) return r < 0.3 ? S.Rock : -1;
  if (bakeId !== BAKE_SPECIES.MangroveProp && env.distSeaM < BEACH_BARE_M) return -1;
  if (env.elevM > TREELINE_M || bakeId === BAKE_SPECIES.ParamoEspeletia) {
    if (r < 0.6) return S.Rosette;
    if (r < 0.9) return S.Bush;
    return env.biome === BIOME.Alpine ? S.Rock : -1;
  }
  if (bakeId === BAKE_SPECIES.MangroveProp) {
    if (env.distSeaM > 600) return r < 0.6 ? S.Broadleaf : S.Bush;
    return r < 0.88 ? S.Mangrove : S.Bush;
  }
  if (env.distSeaM < STRAND_M && env.elevM < 25) {
    if (r < 0.35) return S.Palm;
    if (r < 0.8) return S.Bush;
    return S.Broadleaf;
  }
  switch (bakeId) {
    case BAKE_SPECIES.RainforestCanopy:
      if (r < 0.4) return S.Jungle;
      if (r < 0.78) return S.Broadleaf;
      if (r < 0.86) return S.Palm;
      if (r < 0.94) return S.TreeFern;
      return S.Bush;
    case BAKE_SPECIES.MontaneBroadleaf:
      if (env.elevM >= 1800) {
        if (r < 0.55) return S.Broadleaf;
        if (r < 0.8) return S.Pine;
        if (r < 0.92) return S.TreeFern;
        return S.Bush;
      }
      if (r < 0.72) return S.Broadleaf;
      if (r < 0.84) return S.TreeFern;
      return S.Bush;
    case BAKE_SPECIES.ElfinConifer:
      if (env.elevM < CONIFER_MIN_ELEV_M) return r < 0.7 ? S.Broadleaf : S.Bush;
      if (r < 0.7) return S.Pine;
      if (r < 0.85) return S.Broadleaf;
      return S.Bush;
    case BAKE_SPECIES.DryThornScrub:
      if (env.biome === BIOME.Desert) return r < 0.35 ? S.Cactus : r < 0.85 ? S.Bush : -1;
      if (env.biome === BIOME.Badlands) return r < 0.2 ? S.Cactus : r < 0.7 ? S.Bush : S.Rock;
      return r < 0.75 ? S.Bush : S.Acacia;
    case BAKE_SPECIES.SavannaAcacia:
      if (r < 0.68) return S.Acacia;
      if (r < 0.86) return S.Bush;
      if (r < 0.94) return S.Broadleaf;
      return env.biome === BIOME.Savanna ? S.Palm : S.Bush;
    case BAKE_SPECIES.RiparianWillow:
      if (r < 0.55) return S.Willow;
      if (r < 0.85) return S.Broadleaf;
      return S.Bush;
    default:
      return S.Bush;
  }
}

/** Understory plant under a forest stand, or -1. Tree ferns in the humid tropics, shrubs elsewhere. */
export function chooseUnderstoryShape(bakeId: number, env: FloraHabitat, r: number): number {
  if (env.slope > CLIFF_SLOPE || env.distSeaM < STRAND_M || env.elevM > TREELINE_M) return -1;
  switch (bakeId) {
    case BAKE_SPECIES.RainforestCanopy:
    case BAKE_SPECIES.MontaneBroadleaf:
      return r < 0.45 ? LOOK_SPECIES.TreeFern : LOOK_SPECIES.Bush;
    case BAKE_SPECIES.ElfinConifer:
    case BAKE_SPECIES.RiparianWillow:
      return LOOK_SPECIES.Bush;
    default:
      return -1;
  }
}

function seededRandom(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

/** Paint a vertical gradient: darker at the base of the part (self-shadow), lighter on top. */
function paintGradient(
  geom: THREE.BufferGeometry,
  bottomHex: string,
  topHex: string,
  y0: number,
  y1: number,
): THREE.BufferGeometry {
  const g = geom.index ? geom.toNonIndexed() : geom;
  if (g !== geom) geom.dispose();
  const a = new THREE.Color(bottomHex);
  const b = new THREE.Color(topHex);
  const pos = g.attributes.position;
  const colors = new Float32Array(pos.count * 3);
  const c = new THREE.Color();
  for (let i = 0; i < pos.count; i += 1) {
    const t = THREE.MathUtils.clamp((pos.getY(i) - y0) / Math.max(1e-4, y1 - y0), 0, 1);
    c.copy(a).lerp(b, t);
    colors[i * 3] = c.r;
    colors[i * 3 + 1] = c.g;
    colors[i * 3 + 2] = c.b;
  }
  g.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  return g;
}

/** Jitter vertices radially (shared positions move together, so the mesh stays closed). */
function jitter(geom: THREE.BufferGeometry, amount: number, seed: number): THREE.BufferGeometry {
  const rnd = seededRandom(seed);
  const pos = geom.attributes.position;
  const moved = new Map<string, [number, number, number]>();
  for (let i = 0; i < pos.count; i += 1) {
    const key = `${pos.getX(i).toFixed(4)},${pos.getY(i).toFixed(4)},${pos.getZ(i).toFixed(4)}`;
    let d = moved.get(key);
    if (!d) {
      d = [(rnd() - 0.5) * amount, (rnd() - 0.5) * amount, (rnd() - 0.5) * amount];
      moved.set(key, d);
    }
    pos.setXYZ(i, pos.getX(i) + d[0], pos.getY(i) + d[1], pos.getZ(i) + d[2]);
  }
  pos.needsUpdate = true;
  return geom;
}

/** Normals pointing out of a crown centre: soft volumetric foliage shading instead of facets. */
function crownNormals(geom: THREE.BufferGeometry, cx: number, cy: number, cz: number, up = 0.35): void {
  const pos = geom.attributes.position;
  const normals = new Float32Array(pos.count * 3);
  const v = new THREE.Vector3();
  for (let i = 0; i < pos.count; i += 1) {
    v.set(pos.getX(i) - cx, pos.getY(i) - cy, pos.getZ(i) - cz).normalize();
    v.y += up;
    v.normalize();
    normals[i * 3] = v.x;
    normals[i * 3 + 1] = v.y;
    normals[i * 3 + 2] = v.z;
  }
  geom.setAttribute('normal', new THREE.BufferAttribute(normals, 3));
}

function trunk(h: number, r: number, hex: string): THREE.BufferGeometry {
  const g = new THREE.CylinderGeometry(r * 0.7, r, h, 3, 1, true).translate(0, h / 2, 0);
  const out = paintGradient(g, hex, hex, 0, h);
  out.computeVertexNormals();
  return out;
}

function finish(parts: THREE.BufferGeometry[]): THREE.BufferGeometry {
  for (const p of parts) {
    if (p.attributes.uv) p.deleteAttribute('uv');
  }
  const merged = mergeGeometries(parts, false) ?? parts[0];
  for (const p of parts) if (p !== merged) p.dispose();
  // Re-index: the parts are flattened for per-vertex colour, which left every triangle with
  // three private vertices. Shared corners (same position, normal, colour) collapse again, so
  // ~100k instanced plants run the vertex shader 2-4x less often.
  const indexed = mergeVertices(merged, 1e-4);
  if (indexed !== merged) merged.dispose();
  return indexed;
}

/**
 * Compact <= 9 triangle volumetric impostor silhouette (including 3-tri trunk for elevated
 * canopies so mid/far trees along open river valleys never float).
 */
export function makeFloraImpostorGeometry(speciesId: number): THREE.BufferGeometry {
  const w = LOOK_SPECIES_SIZE[speciesId]?.widthRatio ?? 0.8;
  const r = w * 0.5;
  if (speciesId === LOOK_SPECIES.Pine || speciesId === LOOK_SPECIES.Cactus || speciesId === LOOK_SPECIES.Rosette) {
    const [lo, hi, cy] =
      speciesId === LOOK_SPECIES.Pine
        ? ['#0d1c12', '#1d3520', 0.45]
        : speciesId === LOOK_SPECIES.Cactus
          ? ['#3a5230', '#5d7a45', 0.5]
          : ['#6d7a5e', '#a9b79a', 0.65];
    const h = speciesId === LOOK_SPECIES.Pine ? 0.94 : 0.92;
    const rad = speciesId === LOOK_SPECIES.Cactus ? 0.14 : r * 0.96;
    const cone = new THREE.ConeGeometry(rad, h, 5, 1, true).translate(0, 0.08 + h * 0.5, 0);
    const p = paintGradient(cone, lo, hi, 0.08, 1.0);
    crownNormals(p, 0, cy, 0, 0.55);
    return finish([p]);
  }
  const palette: Readonly<Record<number, readonly [string, string, number, number]>> = {
    [LOOK_SPECIES.Broadleaf]: ['#16290f', '#304a1d', 0.68, 0.34],
    [LOOK_SPECIES.Jungle]: ['#0e2410', '#2d5320', 0.72, 0.36],
    [LOOK_SPECIES.Acacia]: ['#253313', '#5a6b2b', 0.82, 0.2],
    [LOOK_SPECIES.Palm]: ['#1c3a16', '#355626', 0.86, 0.18],
    [LOOK_SPECIES.Rock]: ['#353330', '#5a5650', 0.25, 0.22],
    [LOOK_SPECIES.Mangrove]: ['#14260f', '#2c4a1a', 0.58, 0.26],
    [LOOK_SPECIES.Willow]: ['#2a3d17', '#58702c', 0.56, 0.34],
    [LOOK_SPECIES.TreeFern]: ['#24401a', '#4f7a2c', 0.76, 0.2],
    [LOOK_SPECIES.Bush]: ['#1e2912', '#3a4820', 0.36, 0.22],
  };
  const [lo, hi, cy, halfH] = palette[speciesId] ?? ['#1e2912', '#3a4820', 0.55, 0.3];
  const crownR = r * (speciesId === LOOK_SPECIES.Acacia ? 0.95 : 0.88);
  const topCap = new THREE.ConeGeometry(crownR, halfH * 1.15, 3, 1, true).translate(0, cy + halfH * 0.575, 0);
  const botCap = new THREE.ConeGeometry(crownR, halfH * 0.85, 3, 1, true)
    .rotateX(Math.PI)
    .translate(0, cy - halfH * 0.425, 0);
  const pt = paintGradient(topCap, lo, hi, cy - halfH, cy + halfH);
  const pb = paintGradient(botCap, lo, hi, cy - halfH, cy + halfH);
  crownNormals(pt, 0, cy - halfH * 0.15, 0, 0.55);
  crownNormals(pb, 0, cy - halfH * 0.15, 0, 0.55);

  const hasTrunk =
    speciesId !== LOOK_SPECIES.Rock &&
    speciesId !== LOOK_SPECIES.Bush &&
    speciesId !== LOOK_SPECIES.Mangrove &&
    speciesId !== LOOK_SPECIES.TreeFern;
  if (!hasTrunk) return finish([pt, pb]);

  const stemH = Math.max(0.25, cy - halfH * 0.35);
  const stemR = Math.max(0.032, crownR * 0.14);
  const stem = new THREE.ConeGeometry(stemR, stemH, 3, 1, true).translate(0, stemH * 0.5, 0);
  const pStem = paintGradient(stem, '#35261a', '#4f3b29', 0, stemH);
  pStem.computeVertexNormals();
  return finish([pStem, pt, pb]);
}

/**
 * Unit-height (y 0..1) plant geometry, canopy radius ~0.5 * widthRatio, for bake species id.
 * `lite` (low tier): trunk plus the main crown mass only (palms keep three fronds), so the low
 * tier can afford twice the trees for about the same triangles.
 */
export function makeLookFloraGeometry(speciesId: number, lite = false, nearDetail = false): THREE.BufferGeometry {
  const w = LOOK_SPECIES_SIZE[speciesId]?.widthRatio ?? 0.8;
  const pick = (parts: THREE.BufferGeometry[], keep: number) => {
    if (!lite) return finish(parts);
    for (const g of parts.slice(keep)) g.dispose();
    return finish(parts.slice(0, keep));
  };
  switch (speciesId) {
    case LOOK_SPECIES.Pine: {
      // Layered spire: three open cones, slightly offset, dark at the skirt.
      const r = w * 0.5;
      const c1 = new THREE.ConeGeometry(r, 0.55, 6, 1, true).translate(0, 0.06 + 0.275, 0);
      const c2 = new THREE.ConeGeometry(r * 0.74, 0.5, 6, 1, true).rotateY(0.5).translate(0.01, 0.34 + 0.25, 0);
      const c3 = new THREE.ConeGeometry(r * 0.46, 0.42, 5, 1, true).rotateY(1.1).translate(0, 0.58 + 0.21, 0.01);
      const parts = [c1, c2, c3].map((g) => {
        const p = paintGradient(g, '#0d1c12', '#1d3520', 0.06, 1.0);
        crownNormals(p, 0, 0.45, 0, 0.6);
        return p;
      });
      return pick([trunk(0.12, 0.035, '#3b2b1e'), ...parts], 2);
    }
    case LOOK_SPECIES.Broadleaf: {
      // Multi-lobe crown: one main mass and two smaller lobes at different
      // heights and azimuths, so the silhouette is irregular from every side (instances are
      // also yawed and stretched independently).
      const r = w * 0.5;
      const main = jitter(new THREE.IcosahedronGeometry(r * 0.78, 0), r * 0.26, 11).scale(1, 0.8, 1).translate(-r * 0.08, 0.64, 0.02);
      const l1 = jitter(new THREE.OctahedronGeometry(r * 0.52, 0), r * 0.18, 12).scale(1, 0.85, 1).translate(r * 0.5, 0.5, r * 0.18);
      const l2 = jitter(new THREE.OctahedronGeometry(r * 0.46, 0), r * 0.16, 13).scale(1, 0.8, 1).translate(-r * 0.42, 0.46, -r * 0.34);
      const lobes = [
        [main, '#16290f', '#304a1d', 0.3, 1.0, -r * 0.08, 0.6, 0.02],
        [l1, '#132510', '#2b441b', 0.3, 0.8, r * 0.5, 0.48, r * 0.18],
        [l2, '#142710', '#2e471c', 0.28, 0.75, -r * 0.42, 0.44, -r * 0.34],
      ] as const;
      const parts = lobes.map(([g, lo, hi, y0, y1, cx, cy, cz]) => {
        const p = paintGradient(g, lo, hi, y0, y1);
        crownNormals(p, cx, cy, cz);
        return p;
      });
      return pick([trunk(0.42, 0.04, '#3e2e20'), ...parts], 2);
    }
    case LOOK_SPECIES.Jungle: {
      const r = w * 0.5;
      if (nearDetail) {
        const stem = new THREE.CylinderGeometry(0.032, 0.058, 0.68, 5, 1, true).translate(0, 0.34, 0);
        const pStem = paintGradient(stem, '#3e2d1e', '#59442e', 0, 0.68);
        pStem.computeVertexNormals();
        const parts: THREE.BufferGeometry[] = [pStem];
        for (let b = 0; b < 3; b += 1) {
          const ang = (b / 3) * Math.PI * 2 + 0.4;
          const bough = new THREE.CylinderGeometry(0.015, 0.026, 0.34, 3, 1, true)
            .translate(0, 0.17, 0)
            .rotateZ(-0.55)
            .rotateY(ang)
            .translate(0, 0.56, 0);
          const pb = paintGradient(bough, '#463423', '#5c4631', 0.56, 0.86);
          pb.computeVertexNormals();
          parts.push(pb);
        }
        const pads: ReadonlyArray<readonly [number, number, number, number, number]> = [
          [0, 0.84, 0, r * 0.72, 0.68],
          [r * 0.42, 0.76, r * 0.18, r * 0.56, 0.64],
          [-r * 0.44, 0.74, r * 0.26, r * 0.54, 0.66],
          [r * 0.16, 0.72, -r * 0.45, r * 0.52, 0.62],
          [-r * 0.25, 0.64, -r * 0.35, r * 0.48, 0.6],
          [r * 0.48, 0.6, -r * 0.22, r * 0.44, 0.58],
          [-r * 0.12, 0.9, r * 0.14, r * 0.46, 0.6],
          [r * 0.15, 0.58, r * 0.44, r * 0.42, 0.58],
        ];
        pads.forEach(([px, py, pz, pr, sy], idx) => {
          const dome = jitter(new THREE.OctahedronGeometry(pr, 1), pr * 0.26, 21 + idx)
            .scale(1.06, sy, 0.96)
            .translate(px, py, pz);
          const pd = paintGradient(dome, '#0c210e', '#345a22', py - pr * sy, py + pr * sy);
          crownNormals(pd, px * 0.5, py - pr * 0.25, pz * 0.5, 0.45);
          parts.push(pd);
        });
        return finish(parts);
      }
      const top = jitter(new THREE.IcosahedronGeometry(r, 0), r * 0.25, 21).scale(1, 0.62, 1).translate(0, 0.82, 0);
      const mid = jitter(new THREE.OctahedronGeometry(r * 0.72, 0), r * 0.22, 22).scale(1, 0.64, 1).translate(-r * 0.4, 0.64, r * 0.25);
      const low = jitter(new THREE.OctahedronGeometry(r * 0.58, 0), r * 0.2, 23).scale(1, 0.6, 1).translate(r * 0.45, 0.58, -r * 0.3);
      const sub = jitter(new THREE.OctahedronGeometry(r * 0.5, 0), r * 0.18, 24).scale(1, 0.58, 1).translate(r * 0.1, 0.62, r * 0.44);
      const a = paintGradient(top, '#0f2612', '#2b4f1f', 0.6, 1.0);
      const b = paintGradient(mid, '#0d2010', '#20421a', 0.42, 0.82);
      const c = paintGradient(low, '#0e2211', '#23461b', 0.38, 0.74);
      const d = paintGradient(sub, '#0e2211', '#254a1c', 0.4, 0.76);
      crownNormals(a, 0, 0.72, 0);
      crownNormals(b, -r * 0.4, 0.54, r * 0.25);
      crownNormals(c, r * 0.45, 0.48, -r * 0.3);
      crownNormals(d, r * 0.1, 0.52, r * 0.44);
      return pick([trunk(0.75, 0.035, '#4a3826'), a, b, c, d], 2);
    }
    case LOOK_SPECIES.Acacia: {
      const r = w * 0.5;
      if (nearDetail) {
        const stem = trunk(0.5, 0.036, '#483828');
        const parts: THREE.BufferGeometry[] = [stem];
        for (let b = 0; b < 4; b += 1) {
          const ang = (b / 4) * Math.PI * 2 + 0.35;
          const len = 0.38 + (b % 2) * 0.06;
          const bough = new THREE.CylinderGeometry(0.014, 0.024, len, 3, 1, true)
            .translate(0, len * 0.5, 0)
            .rotateZ(-0.68)
            .rotateY(ang)
            .translate(0, 0.46, 0);
          const pb = paintGradient(bough, '#483828', '#5c4a35', 0.46, 0.8);
          pb.computeVertexNormals();
          parts.push(pb);
        }
        const pads: ReadonlyArray<readonly [number, number, number, number, number]> = [
          [r * 0.06, 0.87, 0.02, r * 0.54, 0.42],
          [-r * 0.48, 0.81, r * 0.24, r * 0.44, 0.4],
          [r * 0.44, 0.79, -r * 0.36, r * 0.42, 0.42],
          [-r * 0.22, 0.77, -r * 0.46, r * 0.38, 0.4],
          [r * 0.32, 0.84, r * 0.38, r * 0.38, 0.42],
          [-r * 0.14, 0.93, -r * 0.08, r * 0.36, 0.44],
          [r * 0.52, 0.74, r * 0.06, r * 0.32, 0.38],
        ];
        pads.forEach(([px, py, pz, pr, sy], idx) => {
          const lobe = jitter(new THREE.OctahedronGeometry(pr, 1), pr * 0.28, 31 + idx)
            .scale(1.12, sy, 0.98)
            .translate(px, py, pz);
          const pl = paintGradient(lobe, '#222f11', idx % 2 === 0 ? '#5c6e2c' : '#4e5f29', py - pr * sy, py + pr * sy);
          crownNormals(pl, px * 0.4, py - pr * 0.35, pz * 0.4, 0.65);
          parts.push(pl);
        });
        return finish(parts);
      }
      // Standard far tier: forked silhouette + fuller organic cushions instead of flat paper slabs.
      const a0 = jitter(new THREE.IcosahedronGeometry(r * 0.65, 0), r * 0.18, 31).scale(1, 0.44, 1).translate(r * 0.1, 0.86, 0);
      const a1 = jitter(new THREE.OctahedronGeometry(r * 0.5, 0), r * 0.15, 32).scale(1, 0.42, 1).translate(-r * 0.45, 0.8, r * 0.28);
      const a2 = jitter(new THREE.OctahedronGeometry(r * 0.46, 0), r * 0.14, 33).scale(1, 0.42, 1).translate(r * 0.36, 0.78, -r * 0.4);
      const parts = [a0, a1, a2].map((g, k) => {
        const p = paintGradient(g, '#283615', k === 0 ? '#53642a' : '#48572a', 0.7, 1.0);
        crownNormals(p, 0, 0.66, 0, 0.7);
        return p;
      });
      return pick([trunk(0.76, 0.035, '#4a3a2a'), ...parts], 2);
    }
    case LOOK_SPECIES.Palm: {
      const parts: THREE.BufferGeometry[] = [trunk(0.88, 0.03, '#6b5a44')];
      for (let k = 0; k < 5; k += 1) {
        const ang = (k / 5) * Math.PI * 2 + 0.3;
        const frond = new THREE.PlaneGeometry(0.1, 0.42, 1, 1)
          .translate(0, 0.21, 0)
          .rotateX(-1.05)
          .rotateY(ang)
          .translate(0, 0.86, 0);
        const p = paintGradient(frond, '#1c3a16', '#355626', 0.7, 0.95);
        crownNormals(p, 0, 0.7, 0, 1.0);
        parts.push(p);
      }
      return pick(parts, 4);
    }
    case LOOK_SPECIES.Cactus: {
      const body = new THREE.CylinderGeometry(0.07, 0.08, 1, 5, 1, true).translate(0, 0.5, 0);
      const arm = new THREE.CylinderGeometry(0.045, 0.05, 0.35, 4, 1, true).translate(0.13, 0.62, 0);
      const a = paintGradient(body, '#3a5230', '#5d7a45', 0, 1);
      const b = paintGradient(arm, '#3a5230', '#5d7a45', 0.45, 0.8);
      a.computeVertexNormals();
      b.computeVertexNormals();
      return finish([a, b]);
    }
    case LOOK_SPECIES.Rock: {
      const r = w * 0.5;
      const a = jitter(new THREE.IcosahedronGeometry(r, 0), r * 0.45, 41).scale(1, 0.55, 0.85).translate(0, r * 0.22, 0);
      const b = jitter(new THREE.OctahedronGeometry(r * 0.55, 0), r * 0.3, 42).scale(1, 0.6, 1).translate(r * 0.7, r * 0.1, r * 0.35);
      const pa = paintGradient(a, '#363431', '#5a5650', -r * 0.2, r * 0.6);
      const pb = paintGradient(b, '#32302d', '#55514b', -r * 0.2, r * 0.4);
      // Rounded (weathered) shading instead of flat facets: normals radiate from each block's
      // centre with an upward bias, which reads as a boulder rather than a low-poly gem.
      crownNormals(pa, 0, r * 0.05, 0, 0.25);
      crownNormals(pb, r * 0.7, 0, r * 0.35, 0.25);
      return finish([pa, pb]);
    }
    case LOOK_SPECIES.Mangrove: {
      // Arching prop roots under a short trunk and a low, dense, glossy crown.
      const r = w * 0.5;
      const parts: THREE.BufferGeometry[] = [];
      for (let k = 0; k < 5; k += 1) {
        const ang = (k / 5) * Math.PI * 2 + 0.4;
        const root = new THREE.CylinderGeometry(0.012, 0.016, 0.36, 3, 1, true)
          .rotateZ(0.55)
          .translate(r * 0.22, 0.14, 0)
          .rotateY(ang);
        const p = paintGradient(root, '#2c2318', '#4a3b2a', 0, 0.32);
        p.computeVertexNormals();
        parts.push(p);
      }
      const crown = jitter(new THREE.IcosahedronGeometry(r * 0.85, 0), r * 0.22, 61).scale(1, 0.5, 1).translate(0, 0.62, 0);
      const lobe = jitter(new THREE.OctahedronGeometry(r * 0.55, 0), r * 0.15, 62).scale(1, 0.55, 1).translate(r * 0.5, 0.55, -r * 0.25);
      const pc = paintGradient(crown, '#14260f', '#2c4a1a', 0.4, 0.85);
      const pl = paintGradient(lobe, '#13240f', '#29451a', 0.38, 0.75);
      crownNormals(pc, 0, 0.55, 0, 0.5);
      crownNormals(pl, r * 0.5, 0.5, -r * 0.25, 0.5);
      return pick([trunk(0.5, 0.03, '#4a3b2a'), pc, pl, ...parts], 3);
    }
    case LOOK_SPECIES.Rosette: {
      // Frailejon: a pillar shaggy with dead leaves, a silver-green rosette on top.
      const pillar = new THREE.CylinderGeometry(0.11, 0.13, 0.72, 5, 1, true).translate(0, 0.36, 0);
      const pp = paintGradient(pillar, '#3d352a', '#6b604c', 0, 0.72);
      pp.computeVertexNormals();
      const parts: THREE.BufferGeometry[] = [pp];
      for (let k = 0; k < 6; k += 1) {
        const ang = (k / 6) * Math.PI * 2;
        const leaf = new THREE.PlaneGeometry(0.1, 0.32, 1, 1).translate(0, 0.16, 0).rotateX(-0.75).rotateY(ang).translate(0, 0.72, 0);
        const p = paintGradient(leaf, '#6d7a5e', '#a9b79a', 0.72, 0.98);
        crownNormals(p, 0, 0.7, 0, 1.0);
        parts.push(p);
      }
      return pick(parts, 4);
    }
    case LOOK_SPECIES.Willow: {
      // Wide, soft, drooping crown: a tall mass plus an open skirt hanging past its edge.
      const r = w * 0.5;
      const mass = jitter(new THREE.IcosahedronGeometry(r * 0.75, 0), r * 0.2, 71).scale(1, 0.85, 1).translate(0, 0.62, 0);
      const skirt = new THREE.ConeGeometry(r * 0.95, 0.55, 7, 1, true).rotateX(Math.PI).translate(0, 0.5, 0);
      const pm = paintGradient(mass, '#2a3d17', '#58702c', 0.3, 1.0);
      const ps = paintGradient(skirt, '#2f4419', '#4f6827', 0.22, 0.78);
      crownNormals(pm, 0, 0.6, 0, 0.4);
      crownNormals(ps, 0, 0.7, 0, 0.2);
      return pick([trunk(0.45, 0.04, '#4a3d2c'), pm, ps], 2);
    }
    case LOOK_SPECIES.TreeFern: {
      // Slim trunk and a ring of arching fronds: the understory of humid tropical forest.
      const parts: THREE.BufferGeometry[] = [trunk(0.78, 0.025, '#3a2f22')];
      for (let k = 0; k < 7; k += 1) {
        const ang = (k / 7) * Math.PI * 2 + 0.2;
        const frond = new THREE.PlaneGeometry(0.16, 0.5, 1, 1).translate(0, 0.25, 0).rotateX(-1.2).rotateY(ang).translate(0, 0.78, 0);
        const p = paintGradient(frond, '#24401a', '#4f7a2c', 0.62, 0.9);
        crownNormals(p, 0, 0.7, 0, 1.0);
        parts.push(p);
      }
      return pick(parts, 5);
    }
    case LOOK_SPECIES.Bush:
    default: {
      const r = w * 0.5;
      const a = jitter(new THREE.OctahedronGeometry(r, 0), r * 0.3, 51).scale(1, 0.6, 1).translate(0, 0.38, 0);
      const b = jitter(new THREE.OctahedronGeometry(r * 0.7, 0), r * 0.25, 52).scale(1, 0.6, 1).translate(r * 0.5, 0.28, -r * 0.3);
      const pa = paintGradient(a, '#1e2912', '#3a4820', 0.0, 0.8);
      const pb = paintGradient(b, '#1c2811', '#36451e', 0.0, 0.6);
      crownNormals(pa, 0, 0.2, 0, 0.6);
      crownNormals(pb, r * 0.5, 0.15, -r * 0.3, 0.6);
      return finish([pa, pb]);
    }
  }
}
