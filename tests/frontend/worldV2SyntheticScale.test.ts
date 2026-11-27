import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { buildAgentPoses, CREATURE_BODY_SCALE } from '../../src/components/Landscape/utils/agentPose';
import { simScale } from '../../src/components/Landscape/utils/liveAgentTransform';
import { mapSimulationElements } from '../../src/components/Landscape/utils/simEnvironment';
import {
  createSyntheticTickGenerator,
  selectWorldV2LandAnchors,
  type SyntheticAnchorWorld,
} from '../../src/components/Landscape/utils/syntheticTicks';
import { decodeAnmwV3Sim } from '../../src/components/Landscape/utils/worldArtifactV3';
import {
  BIG_WORLD_EXTENT_M,
  resolveContactShadowY,
  resolveCreatureEyeRadius,
  resolveWorldSimEntityScaleRatio,
} from '../../src/components/Landscape/worldV2/types';

const HERE = dirname(fileURLToPath(import.meta.url));
const SIM_256_PATH = resolve(HERE, '../../artifacts/world-v2/sim_256.anmw3');

describe('WV2-SYNTH-SCALE Option 1a (0.32 u/sim u on BIG_4096_v2)', () => {
  it('pins BIG_4096_v2 scale ratio to 1/simScale while keeping C01 and World v1 byte-identical (ratio === 1)', () => {
    const renderSize = 1200;

    // World v1 and C01 (40,960 m) must stay exact literal 1
    expect(resolveWorldSimEntityScaleRatio('v1', undefined, renderSize)).toBe(1);
    expect(resolveWorldSimEntityScaleRatio('v1', 40_960, renderSize)).toBe(1);
    expect(resolveWorldSimEntityScaleRatio('v1', BIG_WORLD_EXTENT_M, renderSize)).toBe(1);
    expect(resolveWorldSimEntityScaleRatio('v2', 40_960, renderSize)).toBe(1);
    expect(resolveWorldSimEntityScaleRatio('v2', null, renderSize)).toBe(1);

    // BIG_4096_v2 (81,920 m) drops simScale magnification (1/6 at RENDER_SIZE=1200 -> 0.32 u/sim u)
    const bigRatio = resolveWorldSimEntityScaleRatio('v2', BIG_WORLD_EXTENT_M, renderSize);
    expect(bigRatio).toBeCloseTo(1 / 6, 12);
    expect(simScale(renderSize) * CREATURE_BODY_SCALE * bigRatio).toBeCloseTo(0.32, 12);

    // Eye and shadow helpers at ratio = 1 match trunk formulas identically
    expect(resolveCreatureEyeRadius('predator', 0.1, 1)).toBe(
      Math.max(0.1 * 0.28, 0.22 * CREATURE_BODY_SCALE),
    );
    expect(resolveCreatureEyeRadius('prey', 0.1, 1)).toBe(
      Math.max(0.1 * 0.24, 0.2 * CREATURE_BODY_SCALE),
    );
    expect(resolveContactShadowY(12.5, 1)).toBe(12.5 + 0.08);

    // Big-world ratio scales eye floors and shadow lift proportionally
    expect(resolveCreatureEyeRadius('predator', 0.05, bigRatio)).toBeCloseTo(
      Math.max(0.05 * 0.28, 0.22 * CREATURE_BODY_SCALE * bigRatio),
      12,
    );
    expect(resolveContactShadowY(12.5, bigRatio)).toBeCloseTo(12.5 + 0.08 * bigRatio, 12);

    // simEnvironment mapping at ratio = 1 matches default; at 1/6 scales size & floors by 1/6 while keeping position
    const sampleElems = [
      { type: 'lake', x: 50, y: 10, radius: 3.0, resources: 80 },
      { type: 'tree', x: 52, y: 12, radius: 1.2, resources: 50 },
      { type: 'food', x: 54, y: 14, radius: 0.45, resources: 40 },
      { type: 'tree', x: 55, y: 15, radius: 0.1, resources: 10 }, // hits tree floor
    ];
    const v1Default = mapSimulationElements(sampleElems, renderSize, () => 4);
    const c01Explicit = mapSimulationElements(sampleElems, renderSize, () => 4, 1);
    expect(c01Explicit).toEqual(v1Default);

    const bigMapped = mapSimulationElements(sampleElems, renderSize, () => 4, bigRatio);
    expect(bigMapped.lakes[0].renderX).toBe(v1Default.lakes[0].renderX);
    expect(bigMapped.lakes[0].renderZ).toBe(v1Default.lakes[0].renderZ);
    expect(bigMapped.lakes[0].renderRadius).toBeCloseTo(3.0, 10);
    expect(bigMapped.trees[0].renderRadius).toBeCloseTo(1.2, 10);
    expect(bigMapped.food[0].renderRadius).toBeCloseTo(0.45, 10);
    // Floor check (tree with radius 0.1)
    expect(bigMapped.trees[1].renderRadius).toBeCloseTo(1.5 * bigRatio, 10);
    expect(bigMapped.trees[1].heightScale).toBeCloseTo(1.8 * bigRatio, 10);
  });

  it('keeps eyes non-overlapping and shadows at/below root radius across capture_map 200x5 synthetic population on BIG_4096_v2', () => {
    const raw = readFileSync(SIM_256_PATH);
    const sim = decodeAnmwV3Sim(new Uint8Array(raw.buffer, raw.byteOffset, raw.byteLength));
    const v2World: SyntheticAnchorWorld = {
      size: sim.width,
      elevation: sim.elevation,
      biome: sim.biome22,
      slope: sim.slope,
      seaLevel: sim.seaLevelNorm,
    };

    const renderSize = 1200;
    const scaleRatio = resolveWorldSimEntityScaleRatio('v2', BIG_WORLD_EXTENT_M, renderSize);
    const bodyScaleFactor = CREATURE_BODY_SCALE * scaleRatio;

    const anchors = selectWorldV2LandAnchors(v2World);
    const gen = createSyntheticTickGenerator({
      agents: 200,
      segmentsPerAgent: 5,
      anchors,
    });
    const tick0 = gen.tickAt(0);
    const poses = buildAgentPoses(
      tick0.segments,
      renderSize,
      () => 10,
      tick0.head_directions,
      bodyScaleFactor,
    );

    expect(poses.length).toBe(200);
    for (const pose of poses) {
      const headSeg =
        pose.segments.find((s) => s.segmentId === pose.headSegmentId) ?? pose.segments[0];
      const headRadius = Math.max(0.05, headSeg.renderRadius);
      const eyeRadius = resolveCreatureEyeRadius(pose.agentType, headRadius, scaleRatio);
      expect(eyeRadius).toBeLessThan(0.62 * headRadius);

      const shadowY = resolveContactShadowY(pose.groundY, scaleRatio);
      expect(shadowY).toBeLessThanOrEqual(pose.groundY + pose.rootRenderRadius);
    }
  });
});
