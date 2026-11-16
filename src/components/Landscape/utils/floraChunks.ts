import * as THREE from 'three';
import type { World } from './worldGen';
import { FloraType } from './worldGen';
import { sampleMeshHeight } from './worldSample';
import { FLORA_CELLS_PER_SIDE, FLORA_IMPOSTOR_TRIANGLES, floraCellOf, floraDensityKeep } from './chunkLod';

/** Tall types cast sun shadows; ground cover (bushes, reeds, tufts, boulders) skips the
 *  shadow pass — it reads fine without and roughly halves the shadow-map vertex load. */
export const CASTS_SHADOW = new Set<FloraType>([
  FloraType.Pine,
  FloraType.Round,
  FloraType.Jungle,
  FloraType.Cactus,
  FloraType.Acacia,
  FloraType.Palm,
  FloraType.DeadTree,
]);

/** Deterministic per-instance hash in [0, 1) from the flora's world position. */
export function hash01(a: number, b: number): number {
  let h = Math.imul((a * 1024) | 0, 374761393) + Math.imul((b * 1024) | 0, 668265263);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

/**
 * Per-instance tint (vertex colours carry the two-tone; this multiplies on top) so a forest reads as
 * thousands of slightly different trees, not one repeated prop. Shared by the legacy path and the
 * chunked path so both colour a given instance identically.
 */
export function floraTint(type: FloraType, h: number, h2: number, tint: THREE.Color): THREE.Color {
  if (type === FloraType.Tuft) {
    // Meadow mix over the pale neutral base: mostly grass greens, a scattering of
    // pink / yellow / white wildflowers.
    if (h < 0.055) tint.setRGB(1.05, 0.6, 0.78); // pink
    else if (h < 0.1) tint.setRGB(1.12, 1.02, 0.42); // yellow
    else if (h < 0.13) tint.setRGB(1.12, 1.1, 1.05); // white
    else {
      const g = 0.5 + h2 * 0.18;
      tint.setRGB(g * 0.78, g * 1.05, g * 0.55); // grass green range
    }
  } else if (type === FloraType.Coral) {
    // Reef palette over the pale base: pink / orange / purple / red / cream heads.
    if (h < 0.3) tint.setRGB(1.25, 0.55, 0.72);
    else if (h < 0.55) tint.setRGB(1.3, 0.75, 0.4);
    else if (h < 0.75) tint.setRGB(0.85, 0.55, 1.15);
    else if (h < 0.9) tint.setRGB(1.3, 0.45, 0.45);
    else tint.setRGB(1.1, 1.05, 0.88);
  } else {
    const lum = 0.82 + h * 0.36; // ±18% brightness
    const warm = 0.96 + h2 * 0.08; // slight hue drift
    tint.setRGB(lum * warm, lum, lum * (2 - warm));
  }
  return tint;
}

// ---------------------------------------------------------------------------------------
// Chunked flora (RW4-2 / RW4-3).
//
// Every kept instance's matrix and tint is computed ONCE per (world, density) into cell-sorted
// Float32Arrays. Each frame the camera classifies the cells (utils/chunkLod.ts); only NEAR cells are
// block-copied into the per-type instance buffers, and a single ≤ 8-triangle impostor mesh carries
// the trees (and shrubs, mid range) of the farther cells. The per-frame cost is the classification
// of 256 cells; buffers are re-uploaded only when the selection actually changes.
// ---------------------------------------------------------------------------------------

/** Tall types: shadow casters and the only ones that survive to continent scale as impostors. */
export const TREE_TYPES = CASTS_SHADOW;
/** Mid-range-only impostors: visible out to `floraMidRadius`, dropped beyond. */
export const SHRUB_TYPES = new Set<FloraType>([FloraType.Bush, FloraType.Rock]);

/** 6-triangle cone, base on y = 0, apex at y = 1, unit radius, white vertex colour. */
export function makeImpostorGeometry(): THREE.BufferGeometry {
  const sides = FLORA_IMPOSTOR_TRIANGLES;
  const pos = new Float32Array((sides + 1) * 3);
  for (let k = 0; k < sides; k++) {
    const a = (k / sides) * Math.PI * 2;
    pos[k * 3] = Math.cos(a);
    pos[k * 3 + 2] = Math.sin(a);
  }
  pos[sides * 3 + 1] = 1; // apex
  const index: number[] = [];
  for (let k = 0; k < sides; k++) index.push(k, sides, (k + 1) % sides);
  const colors = new Float32Array((sides + 1) * 3).fill(1);
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.BufferAttribute(pos, 3));
  g.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  g.setIndex(index);
  g.computeVertexNormals();
  return g;
}

export interface TypeSource {
  type: FloraType;
  count: number;
  matrices: Float32Array; // 16 per instance, cell-sorted
  colors: Float32Array; // 3 per instance
  cellStart: Uint32Array; // C*C + 1
}

export interface ImpostorSource {
  count: number;
  matrices: Float32Array;
  colors: Float32Array;
  /** 2*C*C + 1 slots: slot 2c = trees of cell c, slot 2c+1 = shrubs of cell c. */
  slotStart: Uint32Array;
}

export interface FloraSources {
  types: TypeSource[];
  impostor: ImpostorSource;
  cellYMin: Float32Array;
  cellYMax: Float32Array;
}

/** Average vertex colour and bounding box of a unit flora geometry, for sizing its impostor. */
function impostorShape(geometry: THREE.BufferGeometry): { r: number; g: number; b: number; w: number; h: number } {
  geometry.computeBoundingBox();
  const bb = geometry.boundingBox;
  const col = geometry.attributes.color;
  let r = 0.3;
  let g = 0.5;
  let b = 0.3;
  if (col && col.count > 0) {
    r = g = b = 0;
    for (let k = 0; k < col.count; k++) {
      r += col.getX(k);
      g += col.getY(k);
      b += col.getZ(k);
    }
    r /= col.count;
    g /= col.count;
    b /= col.count;
  }
  const w = bb ? Math.max(bb.max.x - bb.min.x, bb.max.z - bb.min.z) * 0.5 : 0.5;
  const h = bb ? bb.max.y - bb.min.y : 1;
  return { r, g, b, w, h };
}

/** Build every cell-sorted source buffer for a world at a given density. Exported for tests. */
export function buildFloraSources(
  world: World,
  geoms: ReadonlyArray<{ type: FloraType; geometry: THREE.BufferGeometry }>,
  opts: {
    renderSize: number;
    heightRatio: number;
    meshResolution: number;
    baseSize: number;
    density: number;
    cellsPerSide?: number;
  },
): FloraSources {
  const C = opts.cellsPerSide ?? FLORA_CELLS_PER_SIDE;
  const { renderSize, heightRatio, meshResolution, baseSize, density } = opts;
  const heightUnits = renderSize * heightRatio;
  const { size } = world;
  const nCells = C * C;
  const cellYMin = new Float32Array(nCells).fill(Infinity);
  const cellYMax = new Float32Array(nCells).fill(-Infinity);

  // Ground position of every kept item (shared by the real and the impostor matrices).
  const px = new Float32Array(world.floraCount);
  const pz = new Float32Array(world.floraCount);
  const py = new Float32Array(world.floraCount);
  const posDone = new Uint8Array(world.floraCount);
  const ensurePos = (i: number) => {
    if (posDone[i]) return;
    const u = (world.floraX[i] + size / 2) / size;
    const v = (world.floraZ[i] + size / 2) / size;
    px[i] = (u - 0.5) * renderSize;
    pz[i] = (v - 0.5) * renderSize;
    // Snap to the RENDER MESH surface (coarser than the data) so nothing floats/sinks.
    py[i] = sampleMeshHeight(world, u, v, meshResolution) * heightUnits;
    posDone[i] = 1;
  };

  const dummy = new THREE.Object3D();
  const tint = new THREE.Color();
  const types: TypeSource[] = [];
  const impostorEntries: Array<{ i: number; type: FloraType; slot: number }> = [];
  const shapes = new Map<FloraType, ReturnType<typeof impostorShape>>();

  for (const { type, geometry } of geoms) {
    geometry.computeBoundingBox();
    const groundLift = geometry.boundingBox ? -geometry.boundingBox.min.y : 0;
    const topH = geometry.boundingBox ? geometry.boundingBox.max.y - geometry.boundingBox.min.y : 1;
    const kept: number[] = [];
    let seen = 0;
    for (let i = 0; i < world.floraCount; i++) {
      if (world.floraType[i] !== type) continue;
      if (floraDensityKeep(seen, density)) kept.push(i);
      seen++;
    }
    const cellOfItem = new Uint32Array(kept.length);
    const cellStart = new Uint32Array(nCells + 1);
    for (let k = 0; k < kept.length; k++) {
      ensurePos(kept[k]);
      const c = floraCellOf(px[kept[k]], pz[kept[k]], renderSize, C);
      cellOfItem[k] = c;
      cellStart[c + 1]++;
    }
    for (let c = 0; c < nCells; c++) cellStart[c + 1] += cellStart[c];
    const fill = cellStart.slice(0, nCells);
    const order = new Uint32Array(kept.length);
    for (let k = 0; k < kept.length; k++) order[fill[cellOfItem[k]]++] = k;

    const matrices = new Float32Array(kept.length * 16);
    const colors = new Float32Array(kept.length * 3);
    const isTree = TREE_TYPES.has(type);
    const isShrub = SHRUB_TYPES.has(type);
    for (let n = 0; n < kept.length; n++) {
      const i = kept[order[n]];
      const s = world.floraScale[i] * baseSize;
      dummy.position.set(px[i], py[i] + groundLift * s, pz[i]);
      dummy.scale.set(s, s, s);
      dummy.rotation.set(0, (world.floraX[i] * 0.7 + world.floraZ[i] * 0.3) % (Math.PI * 2), 0);
      dummy.updateMatrix();
      matrices.set(dummy.matrix.elements, n * 16);
      const h = hash01(world.floraX[i], world.floraZ[i]);
      const h2 = hash01(world.floraZ[i], world.floraX[i]);
      floraTint(type, h, h2, tint);
      colors[n * 3] = tint.r;
      colors[n * 3 + 1] = tint.g;
      colors[n * 3 + 2] = tint.b;
      const c = cellOfItem[order[n]];
      if (py[i] < cellYMin[c]) cellYMin[c] = py[i];
      const top = py[i] + topH * s;
      if (top > cellYMax[c]) cellYMax[c] = top;
      if (isTree || isShrub) impostorEntries.push({ i, type, slot: c * 2 + (isTree ? 0 : 1) });
    }
    types.push({ type, count: kept.length, matrices, colors, cellStart });
    if (isTree || isShrub) shapes.set(type, impostorShape(geometry));
  }

  // Impostor buffer: sorted by slot (cell, trees before shrubs).
  const slotStart = new Uint32Array(2 * nCells + 1);
  for (const e of impostorEntries) slotStart[e.slot + 1]++;
  for (let k = 0; k < 2 * nCells; k++) slotStart[k + 1] += slotStart[k];
  const slotFill = slotStart.slice(0, 2 * nCells);
  const sorted = new Array<(typeof impostorEntries)[number]>(impostorEntries.length);
  for (const e of impostorEntries) sorted[slotFill[e.slot]++] = e;
  const impMatrices = new Float32Array(sorted.length * 16);
  const impColors = new Float32Array(sorted.length * 3);
  for (let n = 0; n < sorted.length; n++) {
    const { i, type } = sorted[n];
    const shape = shapes.get(type)!;
    const s = world.floraScale[i] * baseSize;
    dummy.position.set(px[i], py[i], pz[i]);
    dummy.scale.set(shape.w * s, shape.h * s, shape.w * s);
    dummy.rotation.set(0, 0, 0);
    dummy.updateMatrix();
    impMatrices.set(dummy.matrix.elements, n * 16);
    const h = hash01(world.floraX[i], world.floraZ[i]);
    const h2 = hash01(world.floraZ[i], world.floraX[i]);
    floraTint(type, h, h2, tint);
    impColors[n * 3] = tint.r * shape.r;
    impColors[n * 3 + 1] = tint.g * shape.g;
    impColors[n * 3 + 2] = tint.b * shape.b;
  }

  return {
    types,
    impostor: { count: sorted.length, matrices: impMatrices, colors: impColors, slotStart },
    cellYMin,
    cellYMax,
  };
}

