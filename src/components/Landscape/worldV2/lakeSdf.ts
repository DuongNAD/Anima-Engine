// prototype, to be replaced — WV2-LOOK r13 smooth 1024^2 lake SDF + 16-bit lake water elevation map.

import * as THREE from 'three';
import { ELEV_MIN_M, ELEV_SPAN_M, WaterKind, type WorldV2LoadedBundle } from './types';

export const LAKE_SDF_SIZE = 1024;
export const LAKE_SDF_RANGE_M = 320;

function isLakeKind(wk: number): boolean {
  return wk === WaterKind.FreshwaterLake || wk === WaterKind.EndorheicLake;
}

/**
 * Computes a high-resolution (1024^2) signed distance field + nearest-lake 16-bit elevation texture
 * from `worldV2.waterKind` and `worldV2.waterHeightM`:
 * - R: normalized SDF `[0..255]` (`128` = 0 m shoreline, `>128` inside lake up to +320 m, `<128` outside)
 * - G, B: big-endian 16-bit normalized water elevation (`[ELEV_MIN_M, ELEV_MIN_M + ELEV_SPAN_M]`)
 * - A: 255
 */
export function buildWorldV2LakeSdfData(
  bundle: Pick<WorldV2LoadedBundle, 'size' | 'extentM' | 'waterKind' | 'waterHeightM'>,
  outSize = LAKE_SDF_SIZE,
): Uint8Array {
  const { size, extentM, waterKind, waterHeightM } = bundle;
  const n = outSize * outSize;
  const texelM = extentM / Math.max(1, outSize);
  const scaleSrc = (size - 1) / Math.max(1, outSize - 1);

  const inMask = new Uint8Array(n);
  const levelM = new Float32Array(n).fill(-9999);

  for (let y = 0; y < outSize; y += 1) {
    const sy = Math.min(size - 1, Math.round(y * scaleSrc));
    const row = y * outSize;
    for (let x = 0; x < outSize; x += 1) {
      const sx = Math.min(size - 1, Math.round(x * scaleSrc));
      const si = sy * size + sx;
      if (isLakeKind(waterKind[si])) {
        inMask[row + x] = 1;
        levelM[row + x] = waterHeightM[si];
      }
    }
  }

  // Two-pass Chamfer 3-4 distance transform (scales ~3.0 orthogonal, ~4.242 diagonal -> / 3 * texelM)
  const ORTH = 3;
  const DIAG = 4;
  const INF = 1e8;
  const distOut = new Float32Array(n);
  const distIn = new Float32Array(n);
  for (let i = 0; i < n; i += 1) {
    distOut[i] = inMask[i] ? 0 : INF;
    distIn[i] = inMask[i] ? INF : 0;
  }

  // Forward pass: propagate BOTH distance outside lakes and nearest lake water level
  for (let y = 0; y < outSize; y += 1) {
    const row = y * outSize;
    for (let x = 0; x < outSize; x += 1) {
      const i = row + x;
      let dO = distOut[i];
      let dI = distIn[i];
      let lvl = levelM[i];
      const checkOut = (ni: number, w: number) => {
        const cand = distOut[ni] + w;
        if (cand < dO) {
          dO = cand;
          if (!inMask[i] && levelM[ni] > -9000) lvl = levelM[ni];
        }
      };
      const checkIn = (ni: number, w: number) => {
        const cand = distIn[ni] + w;
        if (cand < dI) dI = cand;
      };
      if (x > 0) {
        checkOut(i - 1, ORTH);
        checkIn(i - 1, ORTH);
      }
      if (y > 0) {
        checkOut(i - outSize, ORTH);
        checkIn(i - outSize, ORTH);
        if (x > 0) {
          checkOut(i - outSize - 1, DIAG);
          checkIn(i - outSize - 1, DIAG);
        }
        if (x + 1 < outSize) {
          checkOut(i - outSize + 1, DIAG);
          checkIn(i - outSize + 1, DIAG);
        }
      }
      distOut[i] = dO;
      distIn[i] = dI;
      levelM[i] = lvl;
    }
  }

  // Backward pass
  for (let y = outSize - 1; y >= 0; y -= 1) {
    const row = y * outSize;
    for (let x = outSize - 1; x >= 0; x -= 1) {
      const i = row + x;
      let dO = distOut[i];
      let dI = distIn[i];
      let lvl = levelM[i];
      const checkOut = (ni: number, w: number) => {
        const cand = distOut[ni] + w;
        if (cand < dO) {
          dO = cand;
          if (!inMask[i] && levelM[ni] > -9000) lvl = levelM[ni];
        }
      };
      const checkIn = (ni: number, w: number) => {
        const cand = distIn[ni] + w;
        if (cand < dI) dI = cand;
      };
      if (x + 1 < outSize) {
        checkOut(i + 1, ORTH);
        checkIn(i + 1, ORTH);
      }
      if (y + 1 < outSize) {
        checkOut(i + outSize, ORTH);
        checkIn(i + outSize, ORTH);
        if (x + 1 < outSize) {
          checkOut(i + outSize + 1, DIAG);
          checkIn(i + outSize + 1, DIAG);
        }
        if (x > 0) {
          checkOut(i + outSize - 1, DIAG);
          checkIn(i + outSize - 1, DIAG);
        }
      }
      distOut[i] = dO;
      distIn[i] = dI;
      levelM[i] = lvl;
    }
  }

  const out = new Uint8Array(n * 4);
  const unitToM = texelM / ORTH;
  for (let i = 0; i < n; i += 1) {
    const signedM = inMask[i] ? Math.max(0.5 * texelM, (distIn[i] - 1.5) * unitToM) : -(distOut[i] - 1.5) * unitToM;
    const sdfNorm = Math.max(-1, Math.min(1, signedM / LAKE_SDF_RANGE_M));
    const rByte = Math.max(0, Math.min(255, Math.round((sdfNorm * 0.5 + 0.5) * 255)));

    const lNorm = levelM[i] > -9000 ? Math.max(0, Math.min(1, (levelM[i] - ELEV_MIN_M) / ELEV_SPAN_M)) : 0;
    const q16 = Math.round(lNorm * 65535);
    const o4 = i * 4;
    out[o4] = rByte;
    out[o4 + 1] = (q16 >> 8) & 0xff;
    out[o4 + 2] = q16 & 0xff;
    out[o4 + 3] = 255;
  }
  return out;
}

export function buildWorldV2LakeSdfTexture(
  bundle: Pick<WorldV2LoadedBundle, 'size' | 'extentM' | 'waterKind' | 'waterHeightM'>,
): THREE.DataTexture {
  const data = buildWorldV2LakeSdfData(bundle, LAKE_SDF_SIZE);
  const tex = new THREE.DataTexture(data, LAKE_SDF_SIZE, LAKE_SDF_SIZE, THREE.RGBAFormat, THREE.UnsignedByteType);
  tex.minFilter = THREE.LinearFilter;
  tex.magFilter = THREE.LinearFilter;
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.generateMipmaps = false;
  tex.needsUpdate = true;
  return tex;
}
