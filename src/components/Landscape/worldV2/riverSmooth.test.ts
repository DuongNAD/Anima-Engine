// prototype, to be replaced — WV2-LOOK r11 smoothed river centerline SDF unit tests.

import { describe, expect, it } from 'vitest';
import { RIVER_SDF_ZERO_BYTE, buildSmoothedRiverMaskData } from './riverSmooth';
import { WaterKind } from './types';

function makeRiverTestBundle(size = 64, extentM = 2560) {
  const n = size * size;
  const waterKind = new Uint8Array(n);
  const heightM = new Float32Array(n).fill(100);
  const waterHeightM = new Float32Array(n).fill(100);
  const hydroFlowRgba = new Uint8Array(n * 4);
  const normalRg = new Uint8Array(n * 2).fill(128);
  return { size, extentM, waterKind, heightM, waterHeightM, hydroFlowRgba, normalRg };
}

describe('riverSmooth', () => {
  it('smooths a 90-degree Manhattan staircase into a continuous curved channel without tearing', () => {
    const size = 64;
    const bundle = makeRiverTestBundle(size, 2560);
    // Staircase running diagonally down-right via 90-degree steps from (8, 8) to (44, 44).
    const cells: Array<[number, number]> = [];
    for (let s = 8; s < 44; s += 2) {
      cells.push([s, s], [s + 1, s], [s + 2, s], [s + 2, s + 1], [s + 2, s + 2]);
    }
    let elev = 120;
    for (const [x, y] of cells) {
      const i = y * size + x;
      bundle.waterKind[i] = WaterKind.River;
      bundle.heightM[i] = elev;
      bundle.waterHeightM[i] = elev + 1.5;
      bundle.hydroFlowRgba[i * 4] = 218;
      bundle.hydroFlowRgba[i * 4 + 1] = 218;
      bundle.hydroFlowRgba[i * 4 + 2] = 160;
      bundle.hydroFlowRgba[i * 4 + 3] = 220;
      elev -= 0.4;
    }

    const mask = buildSmoothedRiverMaskData(bundle);
    // Mid-staircase corners pull smoothly inward onto diagonal (26, 26) -> water SDF < 0 (< 80 byte).
    const centerByte = mask[(26 * size + 26) * 4];
    expect(centerByte).toBeLessThan(RIVER_SDF_ZERO_BYTE);
    // Far from the river (4, 58), SDF is well above the 6.5 m narrow bank zone.
    const farSdfM = mask[(58 * size + 4) * 4] - RIVER_SDF_ZERO_BYTE;
    expect(farSdfM).toBeGreaterThan(20);
  });

  it('keeps wet banks narrow and leaves standing Ocean/FreshwaterLake excluded (R=255)', () => {
    const size = 64;
    const bundle = makeRiverTestBundle(size, 2560); // 40 m texel
    for (let x = 10; x <= 40; x += 1) {
      const i = 24 * size + x;
      bundle.waterKind[i] = WaterKind.River;
      bundle.heightM[i] = 100 - x * 0.2;
      bundle.waterHeightM[i] = bundle.heightM[i] + 1.0;
      bundle.hydroFlowRgba[i * 4] = 255;
      bundle.hydroFlowRgba[i * 4 + 1] = 128;
      bundle.hydroFlowRgba[i * 4 + 2] = 120;
      bundle.hydroFlowRgba[i * 4 + 3] = 200;
    }
    bundle.waterKind[10 * size + 10] = WaterKind.Ocean;
    bundle.waterKind[12 * size + 12] = WaterKind.FreshwaterLake;

    const mask = buildSmoothedRiverMaskData(bundle);
    expect(mask[(10 * size + 10) * 4]).toBe(255);
    expect(mask[(12 * size + 12) * 4]).toBe(255);
    // Channel center is water (< 80), while 2 texels (80 m) away is dry (> zero + 15 m).
    expect(mask[(24 * size + 25) * 4]).toBeLessThan(RIVER_SDF_ZERO_BYTE);
    expect(mask[(26 * size + 25) * 4] - RIVER_SDF_ZERO_BYTE).toBeGreaterThan(15);
  });
});
