// prototype, to be replaced — WV2-LOOK r14 river network, discharge widths & SDF unit tests.

import { describe, expect, it } from 'vitest';
import {
  RIVER_SDF_ZERO_BYTE,
  RIVER_WIDTH_MAX_M,
  RIVER_WIDTH_MIN_M,
  buildSmoothedRiverMaskData,
  computeRiverWidthHistogram,
  computeRiverWidthM,
  extractRiverNetwork,
  type RiverMaskInput,
} from './riverSmooth';
import { WaterKind } from './types';

function makeRiverTestBundle(size = 64, extentM = 2560): RiverMaskInput {
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
      bundle.hydroFlowRgba[i * 4 + 2] = 100;
      bundle.hydroFlowRgba[i * 4 + 3] = 220;
      elev -= 0.4;
    }

    const mask = buildSmoothedRiverMaskData(bundle);
    const centerByte = mask[(26 * size + 26) * 4];
    expect(centerByte).toBeLessThan(RIVER_SDF_ZERO_BYTE);
    const farSdfM = mask[(58 * size + 4) * 4] - RIVER_SDF_ZERO_BYTE;
    expect(farSdfM).toBeGreaterThan(20);
  });

  it('keeps wet banks narrow and leaves standing Ocean/FreshwaterLake excluded (R=255)', () => {
    const size = 64;
    const bundle = makeRiverTestBundle(size, 2560);
    for (let x = 10; x <= 40; x += 1) {
      const i = 24 * size + x;
      bundle.waterKind[i] = WaterKind.River;
      bundle.heightM[i] = 100 - x * 0.2;
      bundle.waterHeightM[i] = bundle.heightM[i] + 1.0;
      bundle.hydroFlowRgba[i * 4] = 255;
      bundle.hydroFlowRgba[i * 4 + 1] = 128;
      bundle.hydroFlowRgba[i * 4 + 2] = 75;
      bundle.hydroFlowRgba[i * 4 + 3] = 200;
    }
    bundle.waterKind[10 * size + 10] = WaterKind.Ocean;
    bundle.waterKind[12 * size + 12] = WaterKind.FreshwaterLake;

    const mask = buildSmoothedRiverMaskData(bundle);
    expect(mask[(10 * size + 10) * 4]).toBe(255);
    expect(mask[(12 * size + 12) * 4]).toBe(255);
    expect(mask[(24 * size + 25) * 4]).toBeLessThan(RIVER_SDF_ZERO_BYTE);
    expect(mask[(26 * size + 25) * 4] - RIVER_SDF_ZERO_BYTE).toBeGreaterThan(15);
  });

  it('keeps high steep mountain headwaters connected and boosts energy at confluences', () => {
    const size = 48;
    const bundle = makeRiverTestBundle(size, 1920);
    for (let k = 8; k <= 34; k += 1) {
      const i = k * size + 24;
      bundle.waterKind[i] = WaterKind.River;
      bundle.heightM[i] = 1800 - k * 30;
      bundle.waterHeightM[i] = bundle.heightM[i] + 1.0;
      bundle.normalRg[i * 2] = 220;
      bundle.normalRg[i * 2 + 1] = 128;
      bundle.hydroFlowRgba[i * 4] = 128;
      bundle.hydroFlowRgba[i * 4 + 1] = 255;
      bundle.hydroFlowRgba[i * 4 + 2] = k < 24 ? 4 : 60;
      bundle.hydroFlowRgba[i * 4 + 3] = 180;
    }
    for (let x = 16; x < 24; x += 1) {
      const i = 24 * size + x;
      bundle.waterKind[i] = WaterKind.River;
      bundle.heightM[i] = 1800 - 20 * 30 + (24 - x) * 15;
      bundle.waterHeightM[i] = bundle.heightM[i] + 1.0;
      bundle.hydroFlowRgba[i * 4] = 255;
      bundle.hydroFlowRgba[i * 4 + 1] = 128;
      bundle.hydroFlowRgba[i * 4 + 2] = 12;
      bundle.hydroFlowRgba[i * 4 + 3] = 180;
    }

    const mask = buildSmoothedRiverMaskData(bundle);
    expect(mask[(12 * size + 24) * 4]).toBeLessThan(RIVER_SDF_ZERO_BYTE);
    const headSdf = mask[(12 * size + 24) * 4];
    const trunkSdf = mask[(30 * size + 24) * 4];
    expect(trunkSdf).toBeLessThanOrEqual(headSdf);
    expect(mask[(24 * size + 24) * 4 + 3]).toBeGreaterThanOrEqual(200);
  });

  it('scales headwaters to 1-5 m without a 23 m floor and shrinks steep reaches', () => {
    const hw1 = computeRiverWidthM(1, 40, 0.01, WaterKind.River);
    const hw10 = computeRiverWidthM(10, 40, 0.01, WaterKind.River);
    const stem80 = computeRiverWidthM(80, 40, 0.01, WaterKind.River);
    const steep10 = computeRiverWidthM(10, 40, 0.25, WaterKind.River);
    expect(hw1).toBeGreaterThanOrEqual(RIVER_WIDTH_MIN_M);
    expect(hw1).toBeLessThan(2.5);
    expect(hw10).toBeGreaterThanOrEqual(1.5);
    expect(hw10).toBeLessThan(5.0);
    expect(stem80).toBeGreaterThan(15);
    expect(stem80).toBeLessThanOrEqual(RIVER_WIDTH_MAX_M);
    expect(steep10).toBeLessThan(hw10);
  });

  it('identifies inside-bend (positive bendSide) vs outside cut-bank (negative) and computes histograms', () => {
    const size = 64;
    const bundle = makeRiverTestBundle(size, 2560);
    // L-shaped bend turning right (South -> West): (+y then -x)
    const pathPts: Array<[number, number]> = [];
    for (let y = 12; y <= 32; y += 1) pathPts.push([36, y]);
    for (let x = 35; x >= 16; x -= 1) pathPts.push([x, 32]);
    for (let k = 0; k < pathPts.length; k += 1) {
      const [x, y] = pathPts[k];
      const i = y * size + x;
      bundle.waterKind[i] = WaterKind.River;
      bundle.heightM[i] = 120 - k * 0.3;
      bundle.waterHeightM[i] = bundle.heightM[i] + 1.5;
      bundle.hydroFlowRgba[i * 4] = y === 32 ? 1 : 128;
      bundle.hydroFlowRgba[i * 4 + 1] = y === 32 ? 128 : 255;
      bundle.hydroFlowRgba[i * 4 + 2] = 45;
      bundle.hydroFlowRgba[i * 4 + 3] = 180;
    }

    const net = extractRiverNetwork(bundle);
    expect(net.polylines.length).toBeGreaterThan(0);
    // Inner apex of the South->West right turn vs outer cut-bank
    const insideHit = net.queryNearestRiver(34 / (size - 1), 30 / (size - 1), 350);
    const outsideHit = net.queryNearestRiver(38 / (size - 1), 34 / (size - 1), 350);
    expect(insideHit).not.toBeNull();
    expect(outsideHit).not.toBeNull();
    expect(insideHit!.bendSide).toBeGreaterThan(0.1);
    expect(outsideHit!.bendSide).toBeLessThan(-0.1);

    const hist = computeRiverWidthHistogram(bundle);
    expect(hist.totalCells).toBe(pathPts.length);
    expect(hist.beforeMinM).toBeGreaterThanOrEqual(40);
    expect(hist.afterP50M).toBeLessThan(15);
  });
});
