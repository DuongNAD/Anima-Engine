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
  isTrunkRiver,
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

  it('computes Strahler order >= 3, traces trunks unbroken past joining tributaries, and keeps raw localGrade', () => {
    const size = 64;
    const bundle = makeRiverTestBundle(size, 2560);
    const addLink = (x: number, y: number, h: number, q: number, dxByte = 255, dyByte = 128) => {
      const i = y * size + x;
      bundle.waterKind[i] = WaterKind.River;
      bundle.heightM[i] = h;
      bundle.waterHeightM[i] = h + 1;
      bundle.hydroFlowRgba[i * 4] = dxByte;
      bundle.hydroFlowRgba[i * 4 + 1] = dyByte;
      bundle.hydroFlowRgba[i * 4 + 2] = q;
      bundle.hydroFlowRgba[i * 4 + 3] = 180;
    };
    // Branch A & B (order 1) merge at x=16 into order 2
    for (let x = 8; x <= 15; x += 1) {
      addLink(x, 24, 300 - x * 2, 3);
      addLink(x, 16 + (x - 8), 310 - x * 2, 3, 218, 218);
    }
    // Branch C & D (order 1) merge at (24, 16) -> order 2 descending to meet at (24, 24) -> order 3!
    for (let y = 8; y <= 15; y += 1) {
      addLink(24, y, 320 - y * 2, 3, 128, 255);
      addLink(24 - (16 - y), y, 325 - y * 2, 3, 218, 218);
    }
    for (let y = 16; y < 24; y += 1) addLink(24, y, 285 - y * 2, 8, 128, 255);
    // Main stem from x=16..48 with step drop at x=36..37 and minor order-1 side creek at x=30
    for (let x = 16; x <= 48; x += 1) {
      const stepDrop = x >= 37 ? 12 : 0;
      const q = x >= 24 ? 50 : 12;
      addLink(x, 24, 240 - x * 0.4 - stepDrop, q);
    }
    for (let y = 32; y > 24; y -= 1) addLink(30, y, 260 + (y - 24) * 2, 2, 128, 1);

    const net = extractRiverNetwork(bundle);
    const trunkHit = net.queryNearestRiver(40 / (size - 1), 24 / (size - 1), 100);
    expect(trunkHit).not.toBeNull();
    expect(trunkHit!.strahler).toBeGreaterThanOrEqual(3);
    expect(isTrunkRiver(trunkHit!.halfW, trunkHit!.strahler)).toBe(true);

    // Unbroken trunk passes continuously through confluence at x=30
    const mainTrunk = net.polylines.find((l) => l.some((p) => p.x <= 18) && l.some((p) => p.x >= 44));
    expect(mainTrunk).toBeDefined();
    // Step drop near x=36 has strictly higher localGrade than flat reach near x=20
    const flatPt = mainTrunk!.reduce((a, b) => (Math.abs(a.x - 20) < Math.abs(b.x - 20) ? a : b));
    const stepPt = mainTrunk!.reduce((a, b) => (Math.abs(a.x - 36) < Math.abs(b.x - 36) ? a : b));
    expect(stepPt.localGrade).toBeGreaterThan(flatPt.localGrade * 3);
  });

  it('prunes 1-cell diagonal wart pixels beside channels while snapping real multi-cell tributaries to smoothed trunks', () => {
    const size = 48;
    const bundle = makeRiverTestBundle(size, 1920);
    // Curved main channel with a staircase jog near x=24 so Laplacian moves cell (24,24)
    const stem: Array<[number, number]> = [];
    for (let x = 8; x <= 21; x += 1) stem.push([x, 22]);
    stem.push([22, 23], [23, 24], [24, 24], [25, 24], [26, 23], [27, 22]);
    for (let x = 28; x <= 40; x += 1) stem.push([x, 22]);
    stem.forEach(([x, y], idx) => {
      const i = y * size + x;
      bundle.waterKind[i] = WaterKind.River;
      bundle.heightM[i] = 200 - idx * 0.6;
      bundle.waterHeightM[i] = bundle.heightM[i] + 1.0;
      bundle.hydroFlowRgba[i * 4] = 255;
      bundle.hydroFlowRgba[i * 4 + 1] = 128;
      bundle.hydroFlowRgba[i * 4 + 2] = 40;
      bundle.hydroFlowRgba[i * 4 + 3] = 180;
    });
    bundle.waterKind[22 * size + 41] = WaterKind.Ocean;
    bundle.heightM[22 * size + 41] = 150;

    // 1-cell diagonal wart stub at (18, 21) draining into (18, 22)
    const wartIdx = 21 * size + 18;
    bundle.waterKind[wartIdx] = WaterKind.River;
    bundle.heightM[wartIdx] = 205;
    bundle.waterHeightM[wartIdx] = 206;
    bundle.hydroFlowRgba[wartIdx * 4] = 128;
    bundle.hydroFlowRgba[wartIdx * 4 + 1] = 255;
    bundle.hydroFlowRgba[wartIdx * 4 + 2] = 1;
    bundle.hydroFlowRgba[wartIdx * 4 + 3] = 140;

    // Real 6-cell tributary draining into (24, 24)
    for (let y = 30; y >= 25; y -= 1) {
      const i = y * size + 24;
      bundle.waterKind[i] = WaterKind.River;
      bundle.heightM[i] = 200 + (y - 24) * 2;
      bundle.waterHeightM[i] = bundle.heightM[i] + 1.0;
      bundle.hydroFlowRgba[i * 4] = 128;
      bundle.hydroFlowRgba[i * 4 + 1] = 1;
      bundle.hydroFlowRgba[i * 4 + 2] = 5;
      bundle.hydroFlowRgba[i * 4 + 3] = 150;
    }

    const net = extractRiverNetwork(bundle);
    // Only the main stem and real 6-cell tributary remain; the 1-cell wart is pruned.
    expect(net.polylines.length).toBe(2);
    const trib = net.polylines.find((l) => l[0].y >= 29);
    const main = net.polylines.find((l) => l[0].x <= 9 && l[l.length - 1].x >= 39);
    expect(trib).toBeDefined();
    expect(main).toBeDefined();
    const tribEnd = trib![trib!.length - 1];
    const minDistToMain = Math.min(...main!.map((p) => Math.hypot(p.x - tribEnd.x, p.y - tribEnd.y)));
    expect(minDistToMain).toBeLessThan(0.6);
  });
});
