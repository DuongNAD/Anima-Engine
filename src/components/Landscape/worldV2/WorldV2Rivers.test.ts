// prototype, to be replaced — unit tests for WV2-LOOK r14 3D river ribbon geometry.

import { describe, expect, it } from 'vitest';
import type { World } from '../utils/worldGen';
import { ELEV_MIN_M, ELEV_SPAN_M, WaterKind, type WorldV2LoadedBundle } from './types';
import { buildRiverRibbonGeometry } from './riverRibbon';

describe('WorldV2Rivers', () => {
  it('builds finite continuous river ribbon geometry scaled by discharge without extra draw buffers', () => {
    const size = 48;
    const n = size * size;
    const waterKind = new Uint8Array(n);
    const heightM = new Float32Array(n).fill(300);
    const waterHeightM = new Float32Array(n).fill(301);
    const hydroFlowRgba = new Uint8Array(n * 4);
    const normalRg = new Uint8Array(n * 2).fill(128);

    for (let x = 8; x <= 40; x += 1) {
      const i = 24 * size + x;
      waterKind[i] = WaterKind.River;
      heightM[i] = 400 - x * 2;
      waterHeightM[i] = heightM[i] + 1.0;
      hydroFlowRgba[i * 4] = 255;
      hydroFlowRgba[i * 4 + 1] = 128;
      hydroFlowRgba[i * 4 + 2] = x < 24 ? 2 : 70;
      hydroFlowRgba[i * 4 + 3] = 180;
    }

    const bundle = {
      size,
      extentM: 1920,
      waterKind,
      heightM,
      waterHeightM,
      hydroFlowRgba,
      normalRg,
      sim: { waterfalls: [] },
    } as unknown as WorldV2LoadedBundle;

    const world = {
      size,
      elevation: new Float32Array(n).fill((300 - ELEV_MIN_M) / ELEV_SPAN_M),
      seaLevel: 0.2,
      waterfallCount: 0,
    } as unknown as World;

    const geom = buildRiverRibbonGeometry(bundle, world, 1200, 168, 32, true);
    const pos = geom.getAttribute('position');
    const aRibbon = geom.getAttribute('aRibbon');
    const idx = geom.getIndex();
    expect(pos.count).toBeGreaterThan(20);
    expect(idx).not.toBeNull();
    expect(idx!.count).toBeGreaterThan(30);

    let minW = Infinity;
    let maxW = 0;
    for (let i = 0; i < pos.count; i += 1) {
      expect(Number.isFinite(pos.getX(i))).toBe(true);
      expect(Number.isFinite(pos.getY(i))).toBe(true);
      expect(Number.isFinite(pos.getZ(i))).toBe(true);
      const hw = aRibbon.getZ(i);
      if (hw < minW) minW = hw;
      if (hw > maxW) maxW = hw;
    }
    expect(minW).toBeLessThan(3);
    expect(maxW).toBeGreaterThan(minW * 2);
    geom.dispose();
  });
});
