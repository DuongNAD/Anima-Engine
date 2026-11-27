// prototype, to be replaced — WV2-LOOK r14 geomorphed 20 m L0 near-camera terrain patch & CPU/GPU height sync.

import * as THREE from 'three';
import type { World } from '../utils/worldGen';
import { sampleElevation } from '../utils/worldSample';
import { L0HeightTileCache } from './l0HeightCache';
import { getCachedRiverNetwork } from './riverSmooth';
import { ELEV_MIN_M, ELEV_SPAN_M, SEA_LEVEL_NORM, type WorldV2LoadedBundle } from './types';
import { evaluateDrawnWaterfall } from './waterfallFit';

export interface NearPatchBuildParams {
  gx0: number;
  gz0: number;
  coarseSpan: number;
  subDiv: number;
  meshRes: number;
  renderSize: number;
  heightUnits: number;
  world: World;
  bundle: WorldV2LoadedBundle;
  coarsePositions: Float32Array;
  coarseNormals: Float32Array;
  l0Cache: L0HeightTileCache;
  outPositions: Float32Array;
  outUvs: Float32Array;
  outNormals: Float32Array;
  outTexData: Float32Array;
}

export interface ActiveNearPatchState {
  tex: THREE.DataTexture;
  bounds: THREE.Vector4; // [minX, minZ, maxX, maxZ]
  res: number; // quad subdivisions per side (texture size = res + 1)
  data: Float32Array | null;
  version: number;
}

const dummyTex = new THREE.DataTexture(new Float32Array([0, 0, 0, 0]), 1, 1, THREE.RGBAFormat, THREE.FloatType);
dummyTex.minFilter = THREE.NearestFilter;
dummyTex.magFilter = THREE.NearestFilter;
dummyTex.wrapS = THREE.ClampToEdgeWrapping;
dummyTex.wrapT = THREE.ClampToEdgeWrapping;
dummyTex.needsUpdate = true;

const activeState: ActiveNearPatchState = {
  tex: dummyTex,
  bounds: new THREE.Vector4(0, 0, -1, -1),
  res: 1,
  data: null,
  version: 0,
};

const nearPatchUniforms = {
  uNearPatchTex: { value: dummyTex as THREE.Texture },
  uNearPatchBounds: { value: activeState.bounds },
  uNearPatchRes: { value: 1 },
};

export function bindNearPatchUniforms(uniforms: Record<string, { value: unknown }>): void {
  uniforms.uNearPatchTex = nearPatchUniforms.uNearPatchTex;
  uniforms.uNearPatchBounds = nearPatchUniforms.uNearPatchBounds;
  uniforms.uNearPatchRes = nearPatchUniforms.uNearPatchRes;
}

export function setActiveNearPatchState(
  tex: THREE.DataTexture | null,
  bounds: readonly [number, number, number, number],
  res: number,
  data: Float32Array | null,
): void {
  activeState.tex = tex ?? dummyTex;
  activeState.bounds.set(bounds[0], bounds[1], bounds[2], bounds[3]);
  activeState.res = Math.max(1, res);
  activeState.data = data;
  activeState.version += 1;
  nearPatchUniforms.uNearPatchTex.value = activeState.tex;
  nearPatchUniforms.uNearPatchRes.value = activeState.res;
}

export function clearActiveNearPatchState(): void {
  setActiveNearPatchState(null, [0, 0, -1, -1], 1, null);
}

export function getActiveNearPatchVersion(): number {
  return activeState.version;
}

/** Exact barycentric sample matching `world-terrain-near` triangles (`null` when outside active patch). */
export function sampleActiveNearPatchY(x: number, z: number): number | null {
  const data = activeState.data;
  const b = activeState.bounds;
  if (!data || b.z <= b.x || b.w <= b.y) return null;
  if (x < b.x || x > b.z || z < b.y || z > b.w) return null;
  const res = activeState.res;
  const side = res + 1;
  const fx = Math.min(res - 1e-6, Math.max(0, ((x - b.x) / (b.z - b.x)) * res));
  const fz = Math.min(res - 1e-6, Math.max(0, ((z - b.y) / (b.w - b.y)) * res));
  const ix = Math.floor(fx);
  const iz = Math.floor(fz);
  const tx = fx - ix;
  const tz = fz - iz;
  const h00 = data[(iz * side + ix) * 4];
  const h10 = data[(iz * side + ix + 1) * 4];
  const h01 = data[((iz + 1) * side + ix) * 4];
  const h11 = data[((iz + 1) * side + ix + 1) * 4];
  return tx + tz < 1
    ? h00 + (h10 - h00) * tx + (h01 - h00) * tz
    : h11 + (h01 - h11) * (1 - tx) + (h10 - h11) * (1 - tz);
}

/** Manual barycentric texelFetch sampler (WebGL2 core; requires zero float-linear extension). */
export const NEAR_PATCH_SAMPLER_GLSL = /* glsl */ `
uniform sampler2D uNearPatchTex;
uniform vec4 uNearPatchBounds;
uniform float uNearPatchRes;
vec4 sampleNearPatch(vec2 xz){
  if(uNearPatchBounds.z<=uNearPatchBounds.x||xz.x<uNearPatchBounds.x||xz.x>uNearPatchBounds.z||xz.y<uNearPatchBounds.y||xz.y>uNearPatchBounds.w){
    return vec4(0.0);
  }
  float res=max(1.0,uNearPatchRes);
  vec2 g=clamp((xz-uNearPatchBounds.xy)/max(vec2(1e-5),uNearPatchBounds.zw-uNearPatchBounds.xy),0.0,1.0)*res;
  vec2 i0=min(floor(g),vec2(res-1.0));
  vec2 f=g-i0;
  ivec2 p=ivec2(i0);
  vec4 h00=texelFetch(uNearPatchTex,p,0);
  vec4 h10=texelFetch(uNearPatchTex,p+ivec2(1,0),0);
  vec4 h01=texelFetch(uNearPatchTex,p+ivec2(0,1),0);
  vec4 h11=texelFetch(uNearPatchTex,p+ivec2(1,1),0);
  return (f.x+f.y<1.0)
    ? h00+(h10-h00)*f.x+(h01-h00)*f.y
    : h11+(h01-h11)*(1.0-f.x)+(h10-h11)*(1.0-f.y);
}
`;

function smoothstep(e0: number, e1: number, x: number): number {
  const t = Math.min(1, Math.max(0, (x - e0) / (e1 - e0)));
  return t * t * (3 - 2 * t);
}

/** Barycentric height sample over the global `meshRes^2` carved vertex array. */
export function sampleCoarseTriangleAttr(
  attr: Float32Array,
  stride: number,
  comp: number,
  meshRes: number,
  u: number,
  v: number,
): number {
  const verts = meshRes + 1;
  const fx = Math.min(meshRes - 1e-6, Math.max(0, u * meshRes));
  const fz = Math.min(meshRes - 1e-6, Math.max(0, v * meshRes));
  const x0 = Math.floor(fx);
  const z0 = Math.floor(fz);
  const x1 = Math.min(meshRes, x0 + 1);
  const z1 = Math.min(meshRes, z0 + 1);
  const tx = fx - x0;
  const tz = fz - z0;
  const v00 = attr[(z0 * verts + x0) * stride + comp];
  const v10 = attr[(z0 * verts + x1) * stride + comp];
  const v01 = attr[(z1 * verts + x0) * stride + comp];
  const v11 = attr[(z1 * verts + x1) * stride + comp];
  return tx + tz < 1
    ? v00 + (v10 - v00) * tx + (v01 - v00) * tz
    : v11 + (v01 - v11) * (1 - tx) + (v10 - v11) * (1 - tz);
}

/** Builds the index buffer for the `patchRes^2` grid plus a downward outer skirt to hide 1-ULP cracks. */
export function buildNearPatchIndices(patchRes: number, skirtDepthUnits = 0.08): {
  indices: Uint32Array;
  vertCount: number;
  baseVertCount: number;
} {
  const gridVerts = (patchRes + 1) * (patchRes + 1);
  const skirtVerts = patchRes * 4 * 2;
  const triIndices = patchRes * patchRes * 6 + patchRes * 4 * 6;
  const indices = new Uint32Array(triIndices);
  let o = 0;
  const stride = patchRes + 1;
  for (let z = 0; z < patchRes; z += 1) {
    for (let x = 0; x < patchRes; x += 1) {
      const a = z * stride + x;
      const b = a + 1;
      const c = a + stride;
      const d = c + 1;
      indices[o++] = a;
      indices[o++] = c;
      indices[o++] = b;
      indices[o++] = b;
      indices[o++] = c;
      indices[o++] = d;
    }
  }
  // 4 outer skirt strips tucked below the perimeter edge
  let sv = gridVerts;
  const pushStrip = (len: number, at: (k: number) => number, flipWinding: boolean) => {
    for (let k = 0; k < len; k += 1) {
      const top0 = at(k);
      const top1 = at(k + 1);
      const bot0 = sv++;
      const bot1 = sv++;
      if (!flipWinding) {
        indices[o++] = top0;
        indices[o++] = bot0;
        indices[o++] = top1;
        indices[o++] = top1;
        indices[o++] = bot0;
        indices[o++] = bot1;
      } else {
        indices[o++] = top0;
        indices[o++] = top1;
        indices[o++] = bot0;
        indices[o++] = top1;
        indices[o++] = bot1;
        indices[o++] = bot0;
      }
    }
  };
  pushStrip(patchRes, (k) => k, false);
  pushStrip(patchRes, (k) => patchRes * stride + k, true);
  pushStrip(patchRes, (k) => k * stride, true);
  pushStrip(patchRes, (k) => k * stride + patchRes, false);
  void skirtDepthUnits;
  return { indices, vertCount: gridVerts + skirtVerts, baseVertCount: gridVerts };
}

/** Updates coarse `meshRes^2` index buffer omitting quads `[gx0..gx1-1] x [gz0..gz1-1]` cleanly. */
export function updateCoarseCutoutIndices(
  dst: Uint32Array,
  meshRes: number,
  gx0: number,
  gz0: number,
  gx1: number,
  gz1: number,
): number {
  const verts = meshRes + 1;
  let o = 0;
  for (let gz = 0; gz < meshRes; gz += 1) {
    const rowInHoleZ = gz >= gz0 && gz < gz1;
    const rowBase = gz * verts;
    for (let gx = 0; gx < meshRes; gx += 1) {
      if (rowInHoleZ && gx >= gx0 && gx < gx1) continue;
      const a = rowBase + gx;
      const b = a + 1;
      const c = a + verts;
      const d = c + 1;
      dst[o++] = a;
      dst[o++] = c;
      dst[o++] = b;
      dst[o++] = b;
      dst[o++] = c;
      dst[o++] = d;
    }
  }
  return o;
}

/** Builds vertex positions, smooth L0/carved heights, geomorphed borders, normals, and GPU LUT data. */
export function buildNearPatchBuffers(params: NearPatchBuildParams, skirtDepthUnits = 0.08): void {
  const {
    gx0,
    gz0,
    coarseSpan,
    subDiv,
    meshRes,
    renderSize,
    heightUnits,
    world,
    bundle,
    coarsePositions,
    coarseNormals,
    l0Cache,
    outPositions,
    outUvs,
    outNormals,
    outTexData,
  } = params;

  const patchRes = coarseSpan * subDiv;
  const side = patchRes + 1;
  const uPerM = heightUnits / ELEV_SPAN_M;
  const seaY = SEA_LEVEL_NORM * heightUnits;
  const riverNetwork = getCachedRiverNetwork(bundle);

  // Kept waterfall plunge depressions evaluated over coarse heights inside this patch
  const wfPools: Array<{ x: number; z: number; targetY: number; radSq: number }> = [];
  const wfCount = world.waterfallCount ?? 0;
  const toWorld = renderSize / world.size;
  const uMin = gx0 / meshRes;
  const uMax = (gx0 + coarseSpan) / meshRes;
  const vMin = gz0 / meshRes;
  const vMax = (gz0 + coarseSpan) / meshRes;
  for (let i = 0; i < wfCount; i += 1) {
    const wu = world.waterfallX[i] / world.size;
    const wv = world.waterfallZ[i] / world.size;
    if (wu < uMin - 0.02 || wu > uMax + 0.02 || wv < vMin - 0.02 || wv > vMax + 0.02) continue;
    const fit = evaluateDrawnWaterfall(world, bundle, meshRes, renderSize, heightUnits, i);
    if (!fit.keep) continue;
    const yaw = world.waterfallYaw[i];
    const px = world.waterfallX[i] * toWorld + Math.cos(yaw) * 1.4;
    const pz = world.waterfallZ[i] * toWorld + Math.sin(yaw) * 1.4;
    wfPools.push({
      x: px,
      z: pz,
      targetY: Math.max(seaY + 0.01, fit.topY - fit.dropY - 0.12),
      radSq: 3.6 * 3.6,
    });
  }

  for (let iz = 0; iz <= patchRes; iz += 1) {
    const gz = gz0 + iz / subDiv;
    const v = gz / meshRes;
    const nZ = Math.abs((iz / patchRes) * 2 - 1);
    for (let ix = 0; ix <= patchRes; ix += 1) {
      const gx = gx0 + ix / subDiv;
      const u = gx / meshRes;
      const nX = Math.abs((ix / patchRes) * 2 - 1);
      const blend = 1 - smoothstep(0.6, 1.0, Math.max(nX, nZ));
      const idx = iz * side + ix;

      const x = (u - 0.5) * renderSize;
      const z = (v - 0.5) * renderSize;
      const baseY = sampleCoarseTriangleAttr(coarsePositions, 3, 1, meshRes, u, v);

      const l0M = l0Cache.sampleHeightM(u, v);
      let detailY =
        l0M !== null
          ? ((l0M - ELEV_MIN_M) / ELEV_SPAN_M) * heightUnits
          : sampleElevation(world, u, v) * heightUnits;

      let waterY: number | null = null;
      const hit = riverNetwork.queryNearestRiver(u, v, 110);
      if (hit && !hit.isWaterfall && detailY > seaY + 0.02) {
        const isSteepStream = hit.grade > 0.075 && hit.halfW < 4.5;
        const bendIn = Math.max(-1, Math.min(1, hit.bendSide));
        const carveBedHalfW = isSteepStream
          ? Math.max(hit.halfW + 6.5, 11.0)
          : Math.max(hit.halfW + 13.0, 20.0);
        const bankRampM = isSteepStream
          ? 8.5
          : bendIn > 0
            ? 14 + bendIn * 10
            : 12;
        if (hit.distM <= carveBedHalfW + bankRampM + 8) {
          const cElev = l0Cache.sampleHeightM(hit.projU, hit.projV);
          const centerY =
            cElev !== null
              ? ((cElev - ELEV_MIN_M) / ELEV_SPAN_M) * heightUnits
              : sampleElevation(world, hit.projU, hit.projV) * heightUnits;
          const bankJitterM = Math.sin(hit.projU * 1300 + hit.projV * 970) * 0.25;
          const bankDropM = isSteepStream
            ? Math.min(2.4, Math.max(1.35, 1.25 + hit.grade * 4.5))
            : hit.bankDropM;
          const computedWaterY = Math.max(
            seaY + 0.01,
            centerY - (bankDropM + bankJitterM) * uPerM,
          );
          waterY = computedWaterY;

          const bedDepthM = isSteepStream
            ? 0.9 + Math.min(0.6, hit.grade * 2.5)
            : bendIn > 0
              ? 1.1 + (0.45 - 1.1) * bendIn
              : Math.min(2.4, 1.2 + Math.sqrt(Math.max(0.5, hit.halfW)) * 0.22 + -bendIn * 0.4);

          if (hit.distM <= carveBedHalfW) {
            const centerFactor = 1 - 0.55 * smoothstep(0, carveBedHalfW, hit.distM);
            let bedTargetY = Math.max(
              seaY + 0.005,
              computedWaterY - bedDepthM * uPerM * centerFactor,
            );
            // Low gently sloped slip-off bar on inside of wide sinuous bends
            if (!isSteepStream && bendIn > 0.15 && hit.halfW >= 4.0 && hit.distM > hit.halfW * 0.72) {
              const barRise = smoothstep(hit.halfW * 0.72, carveBedHalfW, hit.distM) * bendIn;
              bedTargetY = Math.max(bedTargetY, computedWaterY + (-0.25 + 0.65 * barRise) * uPerM);
            }
            detailY = Math.min(detailY, bedTargetY);
          } else if (hit.distM < carveBedHalfW + bankRampM) {
            const bankT = smoothstep(carveBedHalfW, carveBedHalfW + bankRampM, hit.distM);
            const lipOffsetM = !isSteepStream && bendIn > 0.15 && hit.halfW >= 4.0 ? 0.35 * bendIn : -0.35;
            const lipY = Math.max(seaY + 0.005, computedWaterY + lipOffsetM * uPerM);
            const bankTargetY = lipY + (detailY - lipY) * bankT;
            detailY = Math.min(detailY, bankTargetY);
          }
        }
      }
      for (let p = 0; p < wfPools.length; p += 1) {
        const pool = wfPools[p];
        const d2 = (x - pool.x) ** 2 + (z - pool.z) ** 2;
        if (d2 < pool.radSq && pool.targetY < detailY) {
          const pw = 1 - Math.sqrt(d2 / pool.radSq);
          detailY += (pool.targetY - detailY) * pw * 0.85;
        }
      }

      const drawnY = iz === 0 || iz === patchRes || ix === 0 || ix === patchRes
        ? baseY
        : baseY + (detailY - baseY) * blend;

      outPositions[idx * 3] = x;
      outPositions[idx * 3 + 1] = drawnY;
      outPositions[idx * 3 + 2] = z;
      outUvs[idx * 2] = u;
      outUvs[idx * 2 + 1] = v;

      outTexData[idx * 4] = drawnY;
      outTexData[idx * 4 + 1] = drawnY - baseY;
      outTexData[idx * 4 + 2] = blend;
      outTexData[idx * 4 + 3] = waterY !== null ? waterY : drawnY;
    }
  }

  // Normals via finite differences inside patch, pinned smoothly to coarse mesh normals on border
  const stepUnits = ((coarseSpan / meshRes) * renderSize) / patchRes;
  const twoStep = Math.max(1e-4, 2 * stepUnits);
  for (let iz = 0; iz <= patchRes; iz += 1) {
    const nZ = Math.abs((iz / patchRes) * 2 - 1);
    const v = (gz0 + iz / subDiv) / meshRes;
    for (let ix = 0; ix <= patchRes; ix += 1) {
      const nX = Math.abs((ix / patchRes) * 2 - 1);
      const idx = iz * side + ix;
      const u = (gx0 + ix / subDiv) / meshRes;
      const blend = 1 - smoothstep(0.6, 1.0, Math.max(nX, nZ));
      const cnx = sampleCoarseTriangleAttr(coarseNormals, 3, 0, meshRes, u, v);
      const cny = sampleCoarseTriangleAttr(coarseNormals, 3, 1, meshRes, u, v);
      const cnz = sampleCoarseTriangleAttr(coarseNormals, 3, 2, meshRes, u, v);

      if (ix === 0 || ix === patchRes || iz === 0 || iz === patchRes || blend <= 1e-4) {
        outNormals[idx * 3] = cnx;
        outNormals[idx * 3 + 1] = cny;
        outNormals[idx * 3 + 2] = cnz;
      } else {
        const hL = outPositions[(iz * side + (ix - 1)) * 3 + 1];
        const hR = outPositions[(iz * side + (ix + 1)) * 3 + 1];
        const hD = outPositions[((iz - 1) * side + ix) * 3 + 1];
        const hU = outPositions[((iz + 1) * side + ix) * 3 + 1];
        const dx = hL - hR;
        const dz = hD - hU;
        const nl = Math.hypot(dx, twoStep, dz) || 1;
        const dnx = dx / nl;
        const dny = twoStep / nl;
        const dnz = dz / nl;
        const mx = cnx + (dnx - cnx) * blend;
        const my = cny + (dny - cny) * blend;
        const mz = cnz + (dnz - cnz) * blend;
        const ml = Math.hypot(mx, my, mz) || 1;
        outNormals[idx * 3] = mx / ml;
        outNormals[idx * 3 + 1] = my / ml;
        outNormals[idx * 3 + 2] = mz / ml;
      }
    }
  }

  // Copy perimeter vertices into the 4 skirt strips tucked down by skirtDepthUnits
  let sv = side * side;
  const copySkirt = (len: number, at: (k: number) => number) => {
    for (let k = 0; k < len; k += 1) {
      for (const endIdx of [at(k), at(k + 1)]) {
        outPositions[sv * 3] = outPositions[endIdx * 3];
        outPositions[sv * 3 + 1] = outPositions[endIdx * 3 + 1] - skirtDepthUnits;
        outPositions[sv * 3 + 2] = outPositions[endIdx * 3 + 2];
        outUvs[sv * 2] = outUvs[endIdx * 2];
        outUvs[sv * 2 + 1] = outUvs[endIdx * 2 + 1];
        outNormals[sv * 3] = outNormals[endIdx * 3];
        outNormals[sv * 3 + 1] = outNormals[endIdx * 3 + 1];
        outNormals[sv * 3 + 2] = outNormals[endIdx * 3 + 2];
        sv += 1;
      }
    }
  };
  copySkirt(patchRes, (k) => k);
  copySkirt(patchRes, (k) => patchRes * side + k);
  copySkirt(patchRes, (k) => k * side);
  copySkirt(patchRes, (k) => k * side + patchRes);
}
