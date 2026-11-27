// prototype, to be replaced — unit tests for WV2-LOOK r13 terrain channel & waterfall carving.
import { describe, expect, it } from 'vitest';
import type { World } from '../utils/worldGen';
import { ELEV_MIN_M, ELEV_SPAN_M, WaterKind, type WorldV2LoadedBundle } from './types';
import { carveWorldV2RiverAndWaterfallChannels } from './channelCarve';

describe('channelCarve WV2-LOOK r13', () => {
  it('carves a valley trough under channel cells below neighboring banks without sinking below sea', () => {
    const res = 16;
    const size = 32;
    const renderSize = 1200;
    const heightUnits = 168;
    const vertsPerSide = res + 1;
    const positions = new Float32Array(vertsPerSide * vertsPerSide * 3);
    const flatY = ((300 - ELEV_MIN_M) / ELEV_SPAN_M) * heightUnits;
    for (let z = 0; z <= res; z += 1) {
      for (let x = 0; x <= res; x += 1) {
        const i = z * vertsPerSide + x;
        positions[i * 3] = (x / res - 0.5) * renderSize;
        positions[i * 3 + 1] = flatY;
        positions[i * 3 + 2] = (z / res - 0.5) * renderSize;
      }
    }

    const n = size * size;
    const waterKind = new Uint8Array(n);
    const heightM = new Float32Array(n).fill(300);
    const hydroFlowRgba = new Uint8Array(n * 4);
    for (let sx = 0; sx < size; sx += 1) {
      const ci = 16 * size + sx;
      waterKind[ci] = WaterKind.River;
      hydroFlowRgba[ci * 4 + 2] = 64;
    }

    const world = {
      size,
      elevation: new Float32Array(n).fill((300 - ELEV_MIN_M) / ELEV_SPAN_M),
      seaLevel: 0.2,
      waterfallCount: 0,
    } as unknown as World;
    const worldV2 = { size, waterKind, heightM, hydroFlowRgba } as unknown as WorldV2LoadedBundle;

    carveWorldV2RiverAndWaterfallChannels(positions, res, world, worldV2, renderSize, heightUnits);

    const centerY = positions[(8 * vertsPerSide + 8) * 3 + 1];
    const farBankY = positions[(2 * vertsPerSide + 8) * 3 + 1];
    expect(centerY).toBeLessThan(flatY);
    expect(farBankY).toBeCloseTo(flatY, 5);
    expect(centerY).toBeGreaterThan(world.seaLevel * heightUnits);
  });
});
