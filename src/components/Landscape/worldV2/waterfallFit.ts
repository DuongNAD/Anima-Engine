import type { World } from '../utils/worldGen';
import { sampleMeshHeight } from '../utils/worldSample';
import { ELEV_SPAN_M, WORLD_EXTENT_M, type WorldV2LoadedBundle } from './types';

export interface DrawnWaterfallFit {
  keep: boolean;
  topY: number;
  dropY: number;
  pitchRad: number;
}

export const WF_MIN_DRAWN_DROP_M = 22;
export const WF_MIN_DRAWN_SLOPE = 0.4;

/**
 * Evaluates whether waterfall `i` stands over a real steep cliff drop on the drawn terrain mesh,
 * returning exact anchor `topY`, `dropY`, and face pitch. Culled falls return `keep: false`.
 */
export function evaluateDrawnWaterfall(
  world: World,
  worldV2: WorldV2LoadedBundle,
  meshResolution: number,
  renderSize: number,
  heightUnits: number,
  i: number,
  sampleSurfaceY?: (sx: number, sz: number) => number | null | undefined,
): DrawnWaterfallFit {
  const x = world.waterfallX[i] * (renderSize / world.size);
  const z = world.waterfallZ[i] * (renderSize / world.size);
  const dataDropY = Math.max(1.2, world.waterfallDrop[i] * heightUnits);
  const yaw = world.waterfallYaw[i];
  const dirX = Math.cos(yaw);
  const dirZ = Math.sin(yaw);
  const mPerUnitH = (worldV2.extentM || WORLD_EXTENT_M) / renderSize;
  const baseSpanUnits = 1.6;
  const sampleAt = (sx: number, sz: number) => {
    const patchY = sampleSurfaceY?.(sx, sz);
    if (typeof patchY === 'number' && Number.isFinite(patchY)) return patchY;
    return sampleMeshHeight(world, sx / renderSize + 0.5, sz / renderSize + 0.5, meshResolution) * heightUnits;
  };

  const lipY = sampleAt(x, z);
  const upY = sampleAt(x - dirX * 1.4, z - dirZ * 1.4);
  const baseY = sampleAt(x + dirX * baseSpanUnits, z + dirZ * baseSpanUnits);
  const drawnDropY = lipY - baseY;
  const drawnDropM = (drawnDropY / Math.max(1e-6, heightUnits)) * ELEV_SPAN_M;
  const drawnSlope = drawnDropM / Math.max(1e-6, baseSpanUnits * mPerUnitH);

  if (
    drawnDropM < WF_MIN_DRAWN_DROP_M ||
    drawnSlope < WF_MIN_DRAWN_SLOPE ||
    upY < lipY - 0.15 ||
    lipY <= baseY + 0.4
  ) {
    return { keep: false, topY: 0, dropY: 0, pitchRad: 0 };
  }
  const actualDropY = Math.max(1.8, Math.min(drawnDropY * 1.05 + 0.4, Math.max(dataDropY * 1.35, 2.4)));
  const pitchRad = Math.min(0.55, Math.max(0.2, Math.atan2(baseSpanUnits * 0.52, actualDropY) * 0.72));
  return {
    keep: true,
    topY: lipY + 0.2,
    dropY: actualDropY,
    pitchRad,
  };
}
