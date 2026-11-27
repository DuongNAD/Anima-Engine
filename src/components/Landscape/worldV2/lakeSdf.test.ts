// prototype, to be replaced — unit tests for WV2-LOOK r13 smooth 1024^2 lake SDF.
import { describe, expect, it } from 'vitest';
import { WaterKind } from './types';
import { LAKE_SDF_RANGE_M, buildWorldV2LakeSdfData } from './lakeSdf';

describe('lakeSdf WV2-LOOK r13', () => {
  it('encodes positive interior SDF, negative exterior SDF, and preserves distinct lake levels', () => {
    const size = 64;
    const outSize = 64;
    const n = size * size;
    const waterKind = new Uint8Array(n);
    const waterHeightM = new Float32Array(n);

    // Lake 1 (lowland at 20 m): circle at (20, 20) radius 8
    // Lake 2 (alpine pond at 1200 m): square at (48..54, 48..54)
    for (let y = 0; y < size; y += 1) {
      for (let x = 0; x < size; x += 1) {
        const i = y * size + x;
        if (Math.hypot(x - 20, y - 20) <= 8) {
          waterKind[i] = WaterKind.FreshwaterLake;
          waterHeightM[i] = 20;
        } else if (x >= 48 && x <= 54 && y >= 48 && y <= 54) {
          waterKind[i] = WaterKind.EndorheicLake;
          waterHeightM[i] = 1200;
        }
      }
    }

    const data = buildWorldV2LakeSdfData(
      { size, extentM: 2560, waterKind, waterHeightM },
      outSize,
    );

    const decode = (x: number, y: number) => {
      const o4 = (y * outSize + x) * 4;
      const sdfM = ((data[o4] / 255) * 2 - 1) * LAKE_SDF_RANGE_M;
      const q16 = (data[o4 + 1] << 8) | data[o4 + 2];
      const elevM = -1500 + (q16 / 65535) * 7200;
      return { sdfM, elevM };
    };

    const in1 = decode(20, 20);
    const out1 = decode(4, 4);
    const in2 = decode(51, 51);
    expect(in1.sdfM).toBeGreaterThan(80);
    expect(out1.sdfM).toBeLessThan(-80);
    expect(in1.elevM).toBeCloseTo(20, 0);
    expect(in2.elevM).toBeCloseTo(1200, 0);
  });
});
