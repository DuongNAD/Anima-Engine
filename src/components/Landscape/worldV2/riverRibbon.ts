// prototype, to be replaced — WV2-LOOK r14 3D continuous discharge-scaled river ribbon geometry builder.

import * as THREE from 'three';
import type { World } from '../utils/worldGen';
import { WORLD_EXTENT_M, type WorldV2LoadedBundle } from './types';
import { getCachedRiverNetwork } from './riverSmooth';
import { getCachedCarvedCoarseHeights } from './channelCarve';
import { sampleCoarseTriangleAttr } from './terrainNearPatch';
import { evaluateDrawnWaterfall } from './waterfallFit';

/**
 * Builds one merged BufferGeometry containing continuous camera-adaptive ribbons for every
 * traced river polyline, seated against carved coarse heights and upgraded in-shader to flat
 * horizontal water surfaces inside the L0 near-camera patch.
 */
export function buildRiverRibbonGeometry(
  bundle: WorldV2LoadedBundle,
  world: World,
  renderSize: number,
  heightUnits: number,
  meshResolution: number,
  carveBanks: boolean,
): THREE.BufferGeometry {
  const net = getCachedRiverNetwork(bundle);
  const coarseHeights = getCachedCarvedCoarseHeights(
    world,
    bundle,
    meshResolution,
    renderSize,
    heightUnits,
    carveBanks,
  );
  const extentM = bundle.extentM || WORLD_EXTENT_M;
  const unitsPerM = renderSize / extentM;
  const invSize = 1 / Math.max(1, bundle.size - 1);

  // Only fade out river ribbons directly adjacent to kept 3D waterfall curtains (not flat or
  // un-drawn WaterKind.Waterfall channel cells inside canyons such as `river`).
  const wfCount = world.waterfallCount ?? 0;
  const toWorld = renderSize / Math.max(1, world.size);
  const keptWfPts: Array<{ x: number; z: number; radSq: number }> = [];
  const radUnits = Math.max(1.2, 34 * unitsPerM);
  const wfRadSq = radUnits * radUnits;
  for (let i = 0; i < wfCount; i += 1) {
    const fit = evaluateDrawnWaterfall(world, bundle, meshResolution, renderSize, heightUnits, i);
    if (!fit.keep) continue;
    keptWfPts.push({
      x: world.waterfallX[i] * toWorld,
      z: world.waterfallZ[i] * toWorld,
      radSq: wfRadSq,
    });
  }

  let vertCount = 0;
  let idxCount = 0;
  for (let p = 0; p < net.polylines.length; p += 1) {
    const len = net.polylines[p].length;
    if (len < 2) continue;
    vertCount += len * 2;
    idxCount += (len - 1) * 6;
  }

  const positions = new Float32Array(vertCount * 3);
  const aRibbon = new Float32Array(vertCount * 4);
  const aFlow = new Float32Array(vertCount * 4);
  const aRiverMeta = new Float32Array(vertCount * 2);
  const uvs = new Float32Array(vertCount * 2);
  const indices = new Uint32Array(idxCount);

  let vCur = 0;
  let iCur = 0;

  for (let p = 0; p < net.polylines.length; p += 1) {
    const line = net.polylines[p];
    const n = line.length;
    if (n < 2) continue;

    const baseV = vCur;
    for (let k = 0; k < n; k += 1) {
      const pt = line[k];
      const p0 = line[k > 0 ? k - 1 : k];
      const p1 = line[k + 1 < n ? k + 1 : k];
      const dx = p1.x - p0.x;
      const dz = p1.y - p0.y;
      const dLen = Math.hypot(dx, dz);
      const tx = dLen > 1e-4 ? dx / dLen : 0.7071;
      const tz = dLen > 1e-4 ? dz / dLen : 0.7071;
      const rx = -tz;
      const rz = tx;

      const u = pt.x * invSize;
      const v = pt.y * invSize;
      const cx = (u - 0.5) * renderSize;
      const cz = (v - 0.5) * renderSize;
      const halfU = (pt.halfW * unitsPerM) / renderSize;
      const yC = sampleCoarseTriangleAttr(coarseHeights, 1, 0, meshResolution, u, v);
      const yL = sampleCoarseTriangleAttr(
        coarseHeights,
        1,
        0,
        meshResolution,
        Math.max(0, Math.min(1, u - rx * halfU)),
        Math.max(0, Math.min(1, v - rz * halfU)),
      );
      const yR = sampleCoarseTriangleAttr(
        coarseHeights,
        1,
        0,
        meshResolution,
        Math.max(0, Math.min(1, u + rx * halfU)),
        Math.max(0, Math.min(1, v + rz * halfU)),
      );
      const yCoarse = Math.max(yC, yL, yR);

      let wfFlag = 0;
      if (pt.isWaterfall && keptWfPts.length > 0) {
        for (let wi = 0; wi < keptWfPts.length; wi += 1) {
          const wp = keptWfPts[wi];
          const wx = cx - wp.x;
          const wz = cz - wp.z;
          if (wx * wx + wz * wz <= wp.radSq) {
            wfFlag = 1;
            break;
          }
        }
      }
      const qNorm = Math.min(1, pt.q / 110);

      for (const side of [-1, 1]) {
        const vo3 = vCur * 3;
        const vo4 = vCur * 4;
        const vo2 = vCur * 2;
        positions[vo3] = cx;
        positions[vo3 + 1] = yCoarse;
        positions[vo3 + 2] = cz;

        aRibbon[vo4] = rx * side;
        aRibbon[vo4 + 1] = rz * side;
        aRibbon[vo4 + 2] = pt.halfW;
        aRibbon[vo4 + 3] = wfFlag;

        aFlow[vo4] = pt.alongM;
        aFlow[vo4 + 1] = pt.grade;
        aFlow[vo4 + 2] = pt.signedCurv;
        aFlow[vo4 + 3] = qNorm;

        aRiverMeta[vo2] = pt.strahler;
        aRiverMeta[vo2 + 1] = pt.localGrade;

        uvs[vo2] = side;
        uvs[vo2 + 1] = 0;
        vCur += 1;
      }
    }

    for (let k = 0; k < n - 1; k += 1) {
      const a0 = baseV + k * 2;
      const a1 = a0 + 1;
      const b0 = a0 + 2;
      const b1 = a0 + 3;
      indices[iCur++] = a0;
      indices[iCur++] = a1;
      indices[iCur++] = b0;
      indices[iCur++] = b0;
      indices[iCur++] = a1;
      indices[iCur++] = b1;
    }
  }

  const geom = new THREE.BufferGeometry();
  geom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  geom.setAttribute('aRibbon', new THREE.BufferAttribute(aRibbon, 4));
  geom.setAttribute('aFlow', new THREE.BufferAttribute(aFlow, 4));
  geom.setAttribute('aRiverMeta', new THREE.BufferAttribute(aRiverMeta, 2));
  geom.setAttribute('uv', new THREE.BufferAttribute(uvs, 2));
  geom.setIndex(new THREE.BufferAttribute(indices, 1));
  geom.computeBoundingBox();
  geom.computeBoundingSphere();
  return geom;
}
