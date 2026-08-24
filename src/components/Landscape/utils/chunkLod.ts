import * as THREE from 'three';
import type { World } from './worldGen';
import { sampleElevation } from './worldSample';

// ---------------------------------------------------------------------------------------
// Chunk + distance-LOD framework for the terrain (M3).
//
// The showcase draws the whole world as ONE mesh at a fixed resolution, so every triangle is
// processed every frame even when most of the world is off-screen or far away. This module
// splits the terrain into a CxC grid of chunks so the renderer can (a) frustum-cull whole
// chunks the camera can't see and (b) drop the triangle budget of distant chunks (LOD) — the
// standard way to keep a large world cheap and leave GPU/CPU headroom for the agents.
//
// Coordinates match WorldTerrain EXACTLY (X=(u-0.5)*renderSize, Y=elev*heightUnits,
// Z=(v-0.5)*renderSize, uv=(u,v)) so a chunk mesh is a drop-in for a slice of the single mesh
// and shares its textures/material. A UNIFORM grid (every chunk at baseRes/C) samples the same
// grid points as the single mesh → pixel-identical terrain, just partitioned. LOD only changes
// which chunks sample coarser.
// ---------------------------------------------------------------------------------------

export interface Chunk {
  ix: number;
  iz: number;
  u0: number;
  v0: number;
  u1: number;
  v1: number;
  /** Chunk centre in normalized [0,1] space. */
  cu: number;
  cv: number;
}

/** Split the unit square into a CxC grid of chunks (uv-space, renderSize-agnostic). */
export function makeChunkGrid(chunksPerSide: number): Chunk[] {
  const C = Math.max(1, Math.floor(chunksPerSide));
  const chunks: Chunk[] = [];
  for (let iz = 0; iz < C; iz++) {
    for (let ix = 0; ix < C; ix++) {
      const u0 = ix / C;
      const u1 = (ix + 1) / C;
      const v0 = iz / C;
      const v1 = (iz + 1) / C;
      chunks.push({ ix, iz, u0, v0, u1, v1, cu: (u0 + u1) / 2, cv: (v0 + v1) / 2 });
    }
  }
  return chunks;
}

/**
 * LOD level (0 = full detail) for a chunk whose centre is `dist` world-units from the camera.
 * `lodDistances` are ascending world-space radii; crossing each one drops one LOD level.
 */
export function lodForDistance(dist: number, lodDistances: number[]): number {
  let lod = 0;
  for (let i = 0; i < lodDistances.length; i++) {
    if (dist > lodDistances[i]) lod = i + 1;
  }
  return lod;
}

/**
 * Per-chunk mesh resolution (segments per side) at a given LOD. At lod 0 a chunk has
 * baseRes/chunksPerSide segments — so a full-detail grid reproduces the single mesh exactly.
 * Each further LOD halves the resolution, clamped to `minRes`.
 */
export function resForLod(baseRes: number, chunksPerSide: number, lod: number, minRes = 4): number {
  const full = Math.max(1, Math.round(baseRes / Math.max(1, chunksPerSide)));
  const r = Math.round(full / Math.pow(2, lod));
  return Math.max(minRes, r);
}

export interface CostOpts {
  renderSize: number;
  baseRes: number;
  /** Camera position in world space (X, Z; Y ignored for the horizontal distance). */
  camX: number;
  camZ: number;
  /** Chunks whose centre is beyond this radius are culled entirely. */
  cullDistance: number;
  lodDistances: number[];
}

export interface CostReport {
  chunks: number;
  drawn: number;
  culled: number;
  triangles: number;
  /** Triangles a single uniform mesh at baseRes would cost (for comparison). */
  uniformTriangles: number;
  /** Fraction of the uniform-mesh triangle budget actually drawn (lower is better). */
  ratio: number;
}

/**
 * Estimate the triangle/draw cost of the chunked-LOD terrain for a given camera, vs. the single
 * uniform mesh — the "đo trần tài nguyên" measurement M3 asks for. Pure arithmetic, no GPU.
 */
export function estimateCost(chunks: Chunk[], o: CostOpts): CostReport {
  let drawn = 0;
  let culled = 0;
  let triangles = 0;
  for (const c of chunks) {
    const cx = (c.cu - 0.5) * o.renderSize;
    const cz = (c.cv - 0.5) * o.renderSize;
    const dist = Math.hypot(cx - o.camX, cz - o.camZ);
    if (dist > o.cullDistance) {
      culled++;
      continue;
    }
    const lod = lodForDistance(dist, o.lodDistances);
    const res = resForLod(o.baseRes, chunks.length ** 0.5, lod);
    triangles += res * res * 2;
    drawn++;
  }
  const uniformTriangles = o.baseRes * o.baseRes * 2;
  return {
    chunks: chunks.length,
    drawn,
    culled,
    triangles,
    uniformTriangles,
    ratio: uniformTriangles > 0 ? triangles / uniformTriangles : 0,
  };
}

/**
 * LOD level for every chunk given a camera at world (camX, camZ). Pure (no three), so the render
 * loop and the tests share the exact same selection. Empty `lodDistances` → every chunk at LOD 0.
 */
export function assignChunkLods(
  chunks: Chunk[],
  camX: number,
  camZ: number,
  renderSize: number,
  lodDistances: number[],
): number[] {
  if (!lodDistances.length) return chunks.map(() => 0);
  return chunks.map((c) => {
    const cx = (c.cu - 0.5) * renderSize;
    const cz = (c.cv - 0.5) * renderSize;
    return lodForDistance(Math.hypot(cx - camX, cz - camZ), lodDistances);
  });
}

/**
 * Streaming: which chunks should be resident (mounted) for a camera at world (camX, camZ).
 * A chunk LOADS when its centre comes within `loadRadius` and UNLOADS only once it passes
 * `unloadRadius` (> loadRadius) — the hysteresis band stops chunks thrashing on the boundary.
 * This keeps the resident chunk count bounded (~π·loadRadius² / chunkArea) no matter how large
 * the world grows, which is what lets the map scale past a single in-memory mesh. Pure.
 */
export function updateActiveChunks(
  chunks: Chunk[],
  camX: number,
  camZ: number,
  renderSize: number,
  loadRadius: number,
  unloadRadius: number,
  prev: ReadonlySet<number>,
): { active: Set<number>; changed: boolean } {
  const active = new Set<number>(prev);
  let changed = false;
  for (let i = 0; i < chunks.length; i++) {
    const cx = (chunks[i].cu - 0.5) * renderSize;
    const cz = (chunks[i].cv - 0.5) * renderSize;
    const d = Math.hypot(cx - camX, cz - camZ);
    if (d <= loadRadius) {
      if (!active.has(i)) {
        active.add(i);
        changed = true;
      }
    } else if (d > unloadRadius) {
      if (active.has(i)) {
        active.delete(i);
        changed = true;
      }
    }
    // Between loadRadius and unloadRadius: keep whatever state it already had (hysteresis).
  }
  return { active, changed };
}

/**
 * Build one chunk's terrain BufferGeometry (positions/uv/normals), matching WorldTerrain's
 * coordinate convention so it shares that material and its textures. Optional `skirtDepth`
 * drops a flap of geometry around the chunk edge to hide the thin cracks that appear where two
 * chunks meet at different LOD.
 */
export function buildChunkGeometry(
  world: World,
  chunk: Chunk,
  res: number,
  renderSize: number,
  heightRatio: number,
  skirtDepth = 0,
): THREE.BufferGeometry {
  const heightUnits = renderSize * heightRatio;
  const n = res + 1;
  const withSkirt = skirtDepth > 0;
  const skirtVerts = withSkirt ? 4 * n : 0;
  const verts = n * n + skirtVerts;
  const positions = new Float32Array(verts * 3);
  const uvs = new Float32Array(verts * 2);

  const put = (i: number, u: number, v: number, yOff: number) => {
    const e = sampleElevation(world, u, v);
    positions[i * 3] = (u - 0.5) * renderSize;
    positions[i * 3 + 1] = e * heightUnits - yOff;
    positions[i * 3 + 2] = (v - 0.5) * renderSize;
    uvs[i * 2] = u;
    uvs[i * 2 + 1] = v;
  };

  for (let gy = 0; gy < n; gy++) {
    for (let gx = 0; gx < n; gx++) {
      const u = chunk.u0 + (chunk.u1 - chunk.u0) * (gx / res);
      const v = chunk.v0 + (chunk.v1 - chunk.v0) * (gy / res);
      put(gy * n + gx, u, v, 0);
    }
  }

  const indices: number[] = [];
  for (let gy = 0; gy < res; gy++) {
    for (let gx = 0; gx < res; gx++) {
      const a = gy * n + gx;
      const b = a + 1;
      const c = a + n;
      const d = c + 1;
      // CCW from above (matches WorldTerrain) so +Y normals, no back-face culling.
      indices.push(a, c, b, b, c, d);
    }
  }

  if (withSkirt) {
    // Four skirt strips, each a dropped copy of one chunk edge, stitched to that edge.
    let base = n * n;
    const edge = (
      get: (k: number) => number, // top-grid vertex index along the edge, k = 0..res
      uAt: (k: number) => number,
      vAt: (k: number) => number,
    ) => {
      const start = base;
      for (let k = 0; k < n; k++) put(start + k, uAt(k), vAt(k), skirtDepth);
      for (let k = 0; k < res; k++) {
        const top0 = get(k);
        const top1 = get(k + 1);
        const bot0 = start + k;
        const bot1 = start + k + 1;
        indices.push(top0, bot0, top1, top1, bot0, bot1);
        indices.push(top1, bot0, top0, top0, bot0, bot1); // both windings so it shows from either side
      }
      base += n;
    };
    const uOf = (gx: number) => chunk.u0 + (chunk.u1 - chunk.u0) * (gx / res);
    const vOf = (gy: number) => chunk.v0 + (chunk.v1 - chunk.v0) * (gy / res);
    edge((k) => k, uOf, () => chunk.v0); // top edge (gy=0)
    edge((k) => res * n + k, uOf, () => chunk.v1); // bottom edge (gy=res)
    edge((k) => k * n, () => chunk.u0, vOf); // left edge (gx=0)
    edge((k) => k * n + res, () => chunk.u1, vOf); // right edge (gx=res)
  }

  const geom = new THREE.BufferGeometry();
  geom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  geom.setAttribute('uv', new THREE.BufferAttribute(uvs, 2));
  geom.setIndex(indices);
  geom.computeVertexNormals();
  return geom;
}
