// prototype, to be replaced — WV2-LOOK r14 coarse terrain channel & waterfall-step carving (`high`/`medium` only).

import type { World } from '../utils/worldGen';
import { ELEV_MIN_M, ELEV_SPAN_M, WaterKind, type WorldV2LoadedBundle } from './types';
import { evaluateDrawnWaterfall } from './waterfallFit';

function isChannelWaterKind(wk: number): boolean {
  return wk === WaterKind.River || wk === WaterKind.Waterfall || wk === WaterKind.DeltaTurbid;
}

const carvedCoarseHeightCache = new WeakMap<WorldV2LoadedBundle, Map<string, Float32Array>>();

/**
 * Returns (and caches per bundle + resolution) the carved coarse terrain vertex Y array (`(res+1)^2`)
 * so `WorldTerrain` and `WorldV2Rivers` sample bit-identical coarse heights.
 */
export function getCachedCarvedCoarseHeights(
  world: World,
  worldV2: WorldV2LoadedBundle,
  res: number,
  renderSize: number,
  heightUnits: number,
  carveBanks: boolean,
): Float32Array {
  let byKey = carvedCoarseHeightCache.get(worldV2);
  if (!byKey) {
    byKey = new Map();
    carvedCoarseHeightCache.set(worldV2, byKey);
  }
  const key = `${res}:${renderSize}:${heightUnits}:${carveBanks ? 1 : 0}`;
  const hit = byKey.get(key);
  if (hit) return hit;

  const vertsPerSide = res + 1;
  const heights = new Float32Array(vertsPerSide * vertsPerSide);
  const posTmp = new Float32Array(vertsPerSide * vertsPerSide * 3);
  for (let gy = 0; gy <= res; gy += 1) {
    const v = gy / res;
    for (let gx = 0; gx <= res; gx += 1) {
      const u = gx / res;
      const fx = Math.min(world.size - 1, Math.max(0, u * (world.size - 1)));
      const fz = Math.min(world.size - 1, Math.max(0, v * (world.size - 1)));
      const x0 = Math.floor(fx);
      const z0 = Math.floor(fz);
      const x1 = Math.min(world.size - 1, x0 + 1);
      const z1 = Math.min(world.size - 1, z0 + 1);
      const tx = fx - x0;
      const tz = fz - z0;
      const e =
        (world.elevation[z0 * world.size + x0] * (1 - tx) + world.elevation[z0 * world.size + x1] * tx) * (1 - tz) +
        (world.elevation[z1 * world.size + x0] * (1 - tx) + world.elevation[z1 * world.size + x1] * tx) * tz;
      const i = gy * vertsPerSide + gx;
      posTmp[i * 3] = (u - 0.5) * renderSize;
      posTmp[i * 3 + 1] = e * heightUnits;
      posTmp[i * 3 + 2] = (v - 0.5) * renderSize;
    }
  }
  if (carveBanks) {
    carveWorldV2RiverAndWaterfallChannels(posTmp, res, world, worldV2, renderSize, heightUnits);
  }
  for (let i = 0; i < heights.length; i += 1) heights[i] = posTmp[i * 3 + 1];
  byKey.set(key, heights);
  return heights;
}

/**
 * Gently seats major river stems (`q >= 22`) and kept waterfall plunge pools into the coarse terrain
 * mesh (`high`/`medium`) without carving giant dark `160 m` walls under small streams. Near-camera
 * `20 m` channel banks are carved at full fidelity by `terrainNearPatch.ts`.
 */
export function carveWorldV2RiverAndWaterfallChannels(
  positions: Float32Array,
  res: number,
  world: World,
  worldV2: WorldV2LoadedBundle,
  renderSize: number,
  heightUnits: number,
): void {
  const vertsPerSide = res + 1;
  const vCount = vertsPerSide * vertsPerSide;
  const { size, waterKind, heightM, hydroFlowRgba } = worldV2;
  const mPerElev = ELEV_SPAN_M;
  const uPerM = heightUnits / Math.max(1e-6, mPerElev);
  const seaY = world.seaLevel * heightUnits;

  const minChanY = new Float32Array(vCount).fill(Infinity);
  const maxChanQ = new Uint8Array(vCount);
  const minDist = new Float32Array(vCount).fill(Infinity);

  // Only trunk/secondary stems (q >= 12) seat subtly in the coarse 160 m mesh so small creeks never carve dark trenches.
  const radiusV = Math.max(1, Math.ceil((res / size) * 2.0));
  for (let sy = 0; sy < size; sy += 1) {
    const vNorm = sy / Math.max(1, size - 1);
    const cy = vNorm * res;
    const row = sy * size;
    for (let sx = 0; sx < size; sx += 1) {
      const ci = row + sx;
      if (!isChannelWaterKind(waterKind[ci])) continue;
      const q = hydroFlowRgba[ci * 4 + 2];
      if (q < 12) continue;
      const uNorm = sx / Math.max(1, size - 1);
      const cx = uNorm * res;
      const chanY = ((heightM[ci] - ELEV_MIN_M) / mPerElev) * heightUnits;

      const x0 = Math.max(0, Math.floor(cx) - radiusV);
      const x1 = Math.min(res, Math.ceil(cx) + radiusV);
      const y0 = Math.max(0, Math.floor(cy) - radiusV);
      const y1 = Math.min(res, Math.ceil(cy) + radiusV);
      for (let vy = y0; vy <= y1; vy += 1) {
        const dy = vy - cy;
        for (let vx = x0; vx <= x1; vx += 1) {
          const d = Math.hypot(vx - cx, dy);
          if (d > 1.0) continue;
          const vi = vy * vertsPerSide + vx;
          if (chanY < minChanY[vi]) minChanY[vi] = chanY;
          if (q > maxChanQ[vi]) maxChanQ[vi] = q;
          if (d < minDist[vi]) minDist[vi] = d;
        }
      }
    }
  }

  for (let vi = 0; vi < vCount; vi += 1) {
    if (!Number.isFinite(minChanY[vi])) continue;
    const origY = positions[vi * 3 + 1];
    if (origY <= seaY + 0.02) continue;
    const d = minDist[vi];
    const coreWeight = d < 0.4 ? 1.0 : Math.max(0, 1 - (d - 0.4) / 0.6);
    if (coreWeight <= 0) continue;
    const qFrac = Math.min(1, Math.max(0, (maxChanQ[vi] - 12) / 90));
    const carveM = 2.0 + Math.sqrt(qFrac) * 3.5;
    const maxDropM = 12.0;
    const desiredY = minChanY[vi] - carveM * uPerM;
    const clampedTarget = Math.max(seaY + 0.01, origY - maxDropM * uPerM, desiredY);
    if (clampedTarget < origY) {
      positions[vi * 3 + 1] = origY + (clampedTarget - origY) * coreWeight;
    }
  }

  // Carve a crisp plunge pool depression downstream of each kept waterfall while keeping its lip pinned.
  const wfCount = world.waterfallCount ?? 0;
  const toWorld = renderSize / world.size;
  const gridStepUnits = renderSize / res;
  for (let i = 0; i < wfCount; i += 1) {
    const fit = evaluateDrawnWaterfall(world, worldV2, res, renderSize, heightUnits, i);
    if (!fit.keep) continue;
    const x = world.waterfallX[i] * toWorld;
    const z = world.waterfallZ[i] * toWorld;
    const yaw = world.waterfallYaw[i];
    const dirX = Math.cos(yaw);
    const dirZ = Math.sin(yaw);
    const poolX = x + dirX * Math.max(gridStepUnits * 0.95, 1.35);
    const poolZ = z + dirZ * Math.max(gridStepUnits * 0.95, 1.35);
    const gx = (poolX / renderSize + 0.5) * res;
    const gz = (poolZ / renderSize + 0.5) * res;
    const baseTargetY = Math.max(seaY + 0.01, fit.topY - fit.dropY - 0.12);
    const ix0 = Math.max(0, Math.floor(gx) - 1);
    const ix1 = Math.min(res, Math.ceil(gx) + 1);
    const iz0 = Math.max(0, Math.floor(gz) - 1);
    const iz1 = Math.min(res, Math.ceil(gz) + 1);
    for (let vz = iz0; vz <= iz1; vz += 1) {
      for (let vx = ix0; vx <= ix1; vx += 1) {
        const d = Math.hypot(vx - gx, vz - gz);
        if (d > 1.2) continue;
        const vi = vz * vertsPerSide + vx;
        const w = Math.max(0, 1 - d / 1.2);
        const curY = positions[vi * 3 + 1];
        if (baseTargetY < curY) {
          positions[vi * 3 + 1] = curY + (baseTargetY - curY) * w * 0.85;
        }
      }
    }
  }
}
