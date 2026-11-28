import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import v8 from 'node:v8';
import vm from 'node:vm';
import { describe, expect, it } from 'vitest';
import type { SegmentState } from '../../src/types/generated/SegmentState';
import type { SimulationTickPayload } from '../../src/types/generated/SimulationTickPayload';
import {
  TickInterpolationBuffer,
  createPoseSnapshotSlot,
  preparePoseSnapshot,
  preparePoseSnapshotInto,
} from '../../src/components/Landscape/agentInterpolation';
import {
  CREATURE_BODY_SCALE,
  MIN_SIM_SEGMENT_LENGTH,
  MIN_SIM_SEGMENT_RADIUS,
  buildAgentPoses,
  buildAgentPosesInto,
  buildMorphologyRestLayout,
  createAgentPosePool,
  createAgentPoseScratch,
} from '../../src/components/Landscape/utils/agentPose';
import { simScale } from '../../src/components/Landscape/utils/liveAgentTransform';
import { segment } from '../mocks/segment_fixtures';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '../..');

function terrainSampler(rx: number, rz: number): number {
  return 10 + Math.sin(rx * 0.013) * 2.5 + Math.cos(rz * 0.017) * 1.75;
}

function makeMutableThousandAgentPayload(agentCount = 1000, segmentsPerAgent = 3): SimulationTickPayload {
  const segs: SegmentState[] = [];
  const headDirections: Record<number, [number, number, number]> = {};
  for (let i = 0; i < agentCount; i++) {
    const agentId = i + 1;
    const isPred = i >= 700;
    const angle = (i * 0.61803398875) % (Math.PI * 2);
    const radiusSim = 8 + ((i * 19) % 78);
    const baseX = Math.cos(angle) * radiusSim;
    const baseZ = Math.sin(angle) * radiusSim;
    const yaw = angle + 0.4;
    const hx = Math.sin(yaw);
    const hz = Math.cos(yaw);
    headDirections[agentId] = [hx, 0, hz];

    for (let s = 0; s < segmentsPerAgent; s++) {
      const segLen = isPred ? 1.05 - s * 0.08 : 0.85 - s * 0.05;
      const segRad = isPred ? 0.26 - s * 0.03 : 0.22 - s * 0.025;
      segs.push(
        segment({
          agent_id: agentId,
          segment_id: s,
          parent_segment_id: s === 0 ? null : s - 1,
          length: segLen,
          radius: segRad,
          x: baseX - hx * s * 0.9,
          y: 1.5 + s * 0.05,
          z: baseZ - hz * s * 0.9,
          yaw,
          pitch: 0.04 * (s + 1),
          roll: -0.02 * s,
          joint_anchor_x: 0,
          joint_anchor_y: 0,
          joint_anchor_z: s === 0 ? 0 : segLen * 0.5,
          energy: isPred ? 88 : 76,
          hydration: 82,
          head_direction: [hx, 0, hz],
          agent_type: isPred ? 'predator' : 'prey',
        }),
      );
    }
  }
  return {
    segments: segs,
    environmental_state: { elements: [] },
    head_directions: headDirections,
  };
}

function advancePayloadInPlace(payload: SimulationTickPayload, tick: number): void {
  const segs = payload.segments;
  const heads = payload.head_directions;
  const dt = 0.033;
  for (let i = 0; i < segs.length; i++) {
    const s = segs[i];
    const yaw = ((s.agent_id * 0.13 + tick * 0.04 + s.segment_id * 0.08) % (Math.PI * 2));
    const speed = 0.18;
    const nx = s.x + Math.sin(yaw) * speed * dt;
    const nz = s.z + Math.cos(yaw) * speed * dt;
    s.x = nx > 90 ? nx - 180 : nx < -90 ? nx + 180 : nx;
    s.z = nz > 90 ? nz - 180 : nz < -90 ? nz + 180 : nz;
    s.yaw = yaw;
    s.pitch = Math.sin(tick * 0.1 + s.segment_id) * 0.12;
    s.roll = Math.cos(tick * 0.08 + s.agent_id) * 0.05;
    s.energy = 50 + ((s.agent_id + tick) % 50);
    if (s.parent_segment_id === null) {
      const hx = Math.sin(yaw);
      const hz = Math.cos(yaw);
      const hd = s.head_direction;
      hd[0] = hx;
      hd[1] = 0;
      hd[2] = hz;
      const mapped = heads[s.agent_id];
      if (mapped) {
        mapped[0] = hx;
        mapped[1] = 0;
        mapped[2] = hz;
      }
    }
  }
}

describe('FE-MEM-2: pooled per-tick pose builder and ping-pong snapshot buffer', () => {
  it('matches fresh reference poses value-for-value across branches, clamps, births/deaths, and 1,000 agents', () => {
    const slot = createPoseSnapshotSlot();
    const sharedScratch = createAgentPoseScratch();

    // Warm the pool up with a larger random payload first so reused shrinking/growing slots
    // must faithfully overwrite every field, segment slice, and lookup entry.
    const warmup = makeMutableThousandAgentPayload(20, 4);
    preparePoseSnapshotInto(slot, warmup, 10, 1200, terrainSampler, CREATURE_BODY_SCALE, sharedScratch);

    const mixedTicks: SimulationTickPayload[] = [
      // Tick A: backward chain, lateral chain, clamped tiny dims, NaN dims, out-of-bounds root & child,
      // missing parent id, root placed AFTER child in segment array, untyped creature.
      {
        segments: [
          // Agent 10: backward-facing +Z spine (startDir.z < -0.7) with child before root
          segment({
            agent_id: 10,
            segment_id: 2,
            parent_segment_id: 0,
            length: 0.9,
            radius: 0.2,
            x: 12,
            y: 1.4,
            z: 8,
            joint_anchor_z: -0.9,
            agent_type: 'prey',
          }),
          segment({
            agent_id: 10,
            segment_id: 0,
            parent_segment_id: null,
            length: 1.1,
            radius: 0.3,
            x: 12,
            y: 1.1,
            z: 9,
            yaw: 0.5,
            pitch: -0.2,
            roll: 0.1,
            agent_type: 'prey',
          }),
          // Agent 11: lateral first branch (|startDir.z| <= 0.7) + tiny clampable dimensions & NaN fields
          segment({
            agent_id: 11,
            segment_id: 0,
            parent_segment_id: null,
            length: 0.001, // clamped to MIN_SIM_SEGMENT_LENGTH
            radius: 0.001, // clamped to MIN_SIM_SEGMENT_RADIUS
            x: -25,
            y: Number.NaN,
            z: 14,
            yaw: -1.1,
            energy: Number.NaN,
            hydration: Number.POSITIVE_INFINITY,
            head_direction: [0, 0, 0],
            agent_type: 'predator',
          }),
          segment({
            agent_id: 11,
            segment_id: 5,
            parent_segment_id: 999, // missing parent fallback -> rootRenderRadius
            length: -5, // invalid <= 0 -> DEFAULT_FALLBACK_LENGTH
            radius: Number.NaN, // invalid NaN -> DEFAULT_FALLBACK_RADIUS
            x: -24,
            y: 0.8,
            z: 14,
            joint_anchor_x: 0.8,
            joint_anchor_y: 0,
            joint_anchor_z: 0,
            agent_type: 'predator',
          }),
          // Agent 12: forward chain (startDir.z > 0.7) + out-of-bounds child filtered out
          segment({
            agent_id: 12,
            segment_id: 0,
            parent_segment_id: null,
            length: 1.0,
            radius: 0.25,
            x: 40,
            y: 2.0,
            z: -30,
            agent_type: null,
          }),
          segment({
            agent_id: 12,
            segment_id: 1,
            parent_segment_id: 0,
            length: 1.0,
            radius: 0.22,
            x: 40,
            y: 2.0,
            z: -29,
            agent_type: null,
          }),
          segment({
            agent_id: 12,
            segment_id: 2,
            parent_segment_id: 1,
            length: 1.0,
            radius: 0.2,
            x: 190, // > SIM_BOUNDS_HALF * 1.5 -> skipped
            y: 2.0,
            z: -28,
            agent_type: null,
          }),
          // Agent 13: out-of-bounds root -> skipped completely
          segment({
            agent_id: 13,
            segment_id: 0,
            parent_segment_id: null,
            x: -120,
            z: 0,
          }),
        ],
        environmental_state: { elements: [] },
        head_directions: { 10: [0, 0.2, -0.98] },
      },
      // Tick B: empty payload (must zero lengths and purge lookup maps)
      {
        segments: [],
        environmental_state: { elements: [] },
        head_directions: {},
      },
      // Tick C: regrow from 0 to 1,000 moving agents
      makeMutableThousandAgentPayload(1000, 3),
    ];

    advancePayloadInPlace(mixedTicks[2], 7);

    for (let t = 0; t < mixedTicks.length; t++) {
      const payload = mixedTicks[t];
      const ts = 50 + t * 33;
      const pooledSnap = preparePoseSnapshotInto(
        slot,
        payload,
        ts,
        1200,
        terrainSampler,
        CREATURE_BODY_SCALE,
        sharedScratch,
      );
      const refSnap = preparePoseSnapshot(payload, ts, 1200, terrainSampler, CREATURE_BODY_SCALE);

      expect(pooledSnap.receivedAtMs).toBe(refSnap.receivedAtMs);
      expect(pooledSnap.poses.length).toBe(refSnap.poses.length);
      expect(pooledSnap.poses).toEqual(refSnap.poses);
      expect(pooledSnap.agentById.size).toBe(refSnap.agentById.size);
      expect(pooledSnap.segmentByKey.size).toBe(refSnap.segmentByKey.size);

      refSnap.agentById.forEach((expectedPose, id) => {
        expect(pooledSnap.agentById.get(id)).toEqual(expectedPose);
      });
      refSnap.segmentByKey.forEach((expectedSeg, key) => {
        expect(pooledSnap.segmentByKey.get(key)).toEqual(expectedSeg);
      });

      // Verify enumerable property keys and rest-layout head attachment exactness
      for (let i = 0; i < pooledSnap.poses.length; i++) {
        const pooledPose = pooledSnap.poses[i];
        const expectedLayout = buildMorphologyRestLayout(refSnap.poses[i].segments);
        expect(pooledPose.headSegmentId).toBe(expectedLayout.headAttachment.segmentId);
        expect(pooledPose.headLocalForward).toEqual([...expectedLayout.headAttachment.localForward]);
        expect(Object.keys(pooledPose).sort()).toEqual(Object.keys(refSnap.poses[i]).sort());
        for (let s = 0; s < pooledPose.segments.length; s++) {
          expect(Object.keys(pooledPose.segments[s]).sort()).toEqual(
            Object.keys(refSnap.poses[i].segments[s]).sort(),
          );
        }
      }
    }

    // Explicitly verify agentPose.ts minimum clamps were preserved unchanged
    const clampPayload: SimulationTickPayload = {
      segments: [
        segment({
          agent_id: 901,
          segment_id: 0,
          parent_segment_id: null,
          length: 0.001,
          radius: 0.001,
          x: 0,
          z: 0,
        }),
      ],
      environmental_state: { elements: [] },
      head_directions: {},
    };
    const [clampedPose] = buildAgentPoses(
      clampPayload.segments,
      1200,
      terrainSampler,
      clampPayload.head_directions,
      CREATURE_BODY_SCALE,
    );
    const bodyScale = simScale(1200) * CREATURE_BODY_SCALE;
    expect(clampedPose.rootRenderLength).toBeCloseTo(MIN_SIM_SEGMENT_LENGTH * bodyScale, 12);
    expect(clampedPose.rootRenderRadius).toBeCloseTo(MIN_SIM_SEGMENT_RADIUS * bodyScale, 12);
  });

  it('matches reference interpolated frames across 1,000 agents with births, deaths, and world rebuilds', () => {
    const tick0 = makeMutableThousandAgentPayload(1000, 3);
    const tick1 = makeMutableThousandAgentPayload(1000, 3);
    advancePayloadInPlace(tick1, 1);
    // Drop first 10 agents & mutate morphology of agent 50 to test death + N6 snap parity
    const tick2: SimulationTickPayload = {
      segments: tick1.segments
        .filter((s) => s.agent_id > 10)
        .map((s) => (s.agent_id === 50 ? { ...s, length: 2.1, yaw: s.yaw + 0.25 } : { ...s })),
      environmental_state: { elements: [] },
      head_directions: { ...tick1.head_directions },
    };

    const pooledBuf = new TickInterpolationBuffer();
    pooledBuf.push(tick0, 0, 1200, terrainSampler, CREATURE_BODY_SCALE);
    pooledBuf.push(tick1, 33, 1200, terrainSampler, CREATURE_BODY_SCALE);
    pooledBuf.push(tick2, 66, 1200, terrainSampler, CREATURE_BODY_SCALE);
    pooledBuf.rebuildForWorld(960, (rx, rz) => terrainSampler(rx, rz) + 2, CREATURE_BODY_SCALE);

    const pooledView = pooledBuf.sample(82.5, 960, (rx, rz) => terrainSampler(rx, rz) + 2);
    expect(pooledView.agentCount).toBe(1000);
    expect(pooledView.totalSegments).toBe(3000);

    // Build an independent single-pair reference for ticks 1 -> 2 at t = 82.5
    const refBuf = new TickInterpolationBuffer();
    refBuf.push(tick1, 33, 960, (rx, rz) => terrainSampler(rx, rz) + 2, CREATURE_BODY_SCALE);
    refBuf.push(tick2, 66, 960, (rx, rz) => terrainSampler(rx, rz) + 2, CREATURE_BODY_SCALE);
    const refView = refBuf.sample(82.5, 960, (rx, rz) => terrainSampler(rx, rz) + 2);

    expect(pooledView.agentCount).toBe(refView.agentCount);
    expect(pooledView.totalSegments).toBe(refView.totalSegments);
    expect(pooledView.alpha).toBe(refView.alpha);
    for (let i = 0; i < refView.agentCount; i++) {
      const a = pooledView.agents[i];
      const b = refView.agents[i];
      expect(a.agentId).toBe(b.agentId);
      expect(a.rootPosition).toEqual(b.rootPosition);
      expect(a.headSegmentId).toBe(b.headSegmentId);
      expect(a.headLocalForward).toEqual(b.headLocalForward);
      expect(a.headDirection).toEqual(b.headDirection);
      expect(a.morphologyKey).toBe(b.morphologyKey);
      for (let s = 0; s < a.segmentCount; s++) {
        expect(a.segments[s].renderPosition).toEqual(b.segments[s].renderPosition);
        expect(a.segments[s].quaternion).toEqual(b.segments[s].quaternion);
        expect(a.segments[s].jointRenderPosition).toEqual(b.segments[s].jointRenderPosition);
        expect(a.segments[s].jointRenderRadius).toBe(b.segments[s].jointRenderRadius);
      }
    }
  });

  it('allocates zero new objects/arrays in steady state over 60 ticks for 1,000 agents (3,000 segments)', () => {
    const payload = makeMutableThousandAgentPayload(1000, 3);
    const pool = createAgentPosePool();
    const scratch = createAgentPoseScratch();
    const outPoses = buildAgentPosesInto(
      payload.segments,
      1200,
      terrainSampler,
      [],
      pool,
      scratch,
      payload.head_directions,
      CREATURE_BODY_SCALE,
    );
    expect(outPoses).toHaveLength(1000);

    // 1. Direct pose pool + scratch identity stability over 60 moving ticks
    const slot = createPoseSnapshotSlot();
    const warmSnap = preparePoseSnapshotInto(
      slot,
      payload,
      0,
      1200,
      terrainSampler,
      CREATURE_BODY_SCALE,
      scratch,
    );
    expect(warmSnap.poses).toHaveLength(1000);

    const agentRefs = warmSnap.poses.slice();
    const rootPosRefs = warmSnap.poses.map((p) => p.rootPosition);
    const rootQuatRefs = warmSnap.poses.map((p) => p.rootQuaternion);
    const headFwdRefs = warmSnap.poses.map((p) => p.headLocalForward);
    const headDirRefs = warmSnap.poses.map((p) => p.headDirection);
    const segArrRefs = warmSnap.poses.map((p) => p.segments);
    const morphKeys = warmSnap.poses.map((p) => p.morphologyKey);
    const segRefs = warmSnap.poses.flatMap((p) => p.segments);
    const segPosRefs = segRefs.map((s) => s.renderPosition);
    const segQuatRefs = segRefs.map((s) => s.quaternion);
    const segAnchorRefs = segRefs.map((s) => s.jointAnchor);
    const segMatRefs = segRefs.map((s) => s.matrixElements);
    const segJointPosRefs = segRefs.map((s) => s.jointRenderPosition);

    const mapAgentsRef = warmSnap.agentById;
    const mapSegsRef = warmSnap.segmentByKey;
    const posesArrRef = warmSnap.poses;
    const initialX0 = warmSnap.poses[0].rootPosition[0];

    let allSnapsSame = true;
    for (let tick = 1; tick <= 60; tick++) {
      advancePayloadInPlace(payload, tick);
      const nextSnap = preparePoseSnapshotInto(
        slot,
        payload,
        tick * 33,
        1200,
        terrainSampler,
        CREATURE_BODY_SCALE,
        scratch,
      );
      if (
        nextSnap !== warmSnap ||
        nextSnap.poses !== posesArrRef ||
        nextSnap.agentById !== mapAgentsRef ||
        nextSnap.segmentByKey !== mapSegsRef ||
        nextSnap.poses.length !== 1000
      ) {
        allSnapsSame = false;
      }
    }
    expect(allSnapsSame).toBe(true);

    // All 1,000 agent pose objects, nested tuples/matrices, and morphologyKey strings stayed identical by reference
    expect(warmSnap.poses[0].rootPosition[0]).not.toBe(initialX0);
    for (let i = 0; i < 1000; i++) {
      const p = warmSnap.poses[i];
      expect(p).toBe(agentRefs[i]);
      expect(p.rootPosition).toBe(rootPosRefs[i]);
      expect(p.rootQuaternion).toBe(rootQuatRefs[i]);
      expect(p.headLocalForward).toBe(headFwdRefs[i]);
      expect(p.headDirection).toBe(headDirRefs[i]);
      expect(p.segments).toBe(segArrRefs[i]);
      expect(p.morphologyKey).toBe(morphKeys[i]);
    }
    expect(segRefs).toHaveLength(3000);
    for (let k = 0; k < 3000; k++) {
      const s = warmSnap.poses[Math.floor(k / 3)].segments[k % 3];
      expect(s).toBe(segRefs[k]);
      expect(s.renderPosition).toBe(segPosRefs[k]);
      expect(s.quaternion).toBe(segQuatRefs[k]);
      expect(s.jointAnchor).toBe(segAnchorRefs[k]);
      expect(s.matrixElements).toBe(segMatRefs[k]);
      expect(s.jointRenderPosition).toBe(segJointPosRefs[k]);
    }

    // 2. Ping-pong TickInterpolationBuffer identity stability + V8 heap delta over 60 ticks
    const buffer = new TickInterpolationBuffer();
    buffer.push(payload, 0, 1200, terrainSampler, CREATURE_BODY_SCALE);
    buffer.push(payload, 33, 1200, terrainSampler, CREATURE_BODY_SCALE);
    // Warm up several ticks so V8 JIT feedback and both ping-pong slots stabilize
    for (let t = 2; t <= 15; t++) {
      advancePayloadInPlace(payload, 100 + t);
      buffer.push(payload, t * 33, 1200, terrainSampler, CREATURE_BODY_SCALE);
      buffer.sample(t * 33 - 16.5, 1200, terrainSampler);
    }
    for (let t = 0; t < 5; t++) {
      advancePayloadInPlace(payload, 200 + t);
      preparePoseSnapshot(payload, t * 33, 1200, terrainSampler, CREATURE_BODY_SCALE);
    }
    const warmView = buffer.sample(15 * 33 - 16.5, 1200, terrainSampler);
    const viewAgentRefs = warmView.agents.slice(0, 1000);
    const viewSegRefs = warmView.agents.slice(0, 1000).flatMap((a) => a.segments.slice(0, 3));

    v8.setFlagsFromString('--expose_gc');
    const gc = vm.runInNewContext('gc') as () => void;

    let allViewsSame = true;
    gc();
    const pooledRetainedBefore = process.memoryUsage().heapUsed;
    for (let t = 16; t <= 75; t++) {
      advancePayloadInPlace(payload, 100 + t);
      buffer.push(payload, t * 33, 1200, terrainSampler, CREATURE_BODY_SCALE);
      const v = buffer.sample(t * 33 - 16.5, 1200, terrainSampler);
      if (v !== warmView || v.agentCount !== 1000 || v.totalSegments !== 3000) {
        allViewsSame = false;
      }
    }
    gc();
    const pooledRetainedDelta = process.memoryUsage().heapUsed - pooledRetainedBefore;

    // Single-burst allocation rate (4 ticks fit inside young-gen nursery before scavenge)
    gc();
    const pooledBurstBefore = process.memoryUsage().heapUsed;
    for (let t = 76; t < 80; t++) {
      advancePayloadInPlace(payload, 100 + t);
      buffer.push(payload, t * 33, 1200, terrainSampler, CREATURE_BODY_SCALE);
      buffer.sample(t * 33 - 16.5, 1200, terrainSampler);
    }
    const pooledBurstDelta = process.memoryUsage().heapUsed - pooledBurstBefore;

    gc();
    const unpooledBurstBefore = process.memoryUsage().heapUsed;
    for (let t = 0; t < 4; t++) {
      advancePayloadInPlace(payload, 300 + t);
      preparePoseSnapshot(payload, t * 33, 1200, terrainSampler, CREATURE_BODY_SCALE);
    }
    const unpooledBurstDelta = process.memoryUsage().heapUsed - unpooledBurstBefore;

    expect(allViewsSame).toBe(true);
    for (let i = 0; i < 1000; i++) {
      expect(warmView.agents[i]).toBe(viewAgentRefs[i]);
      for (let s = 0; s < 3; s++) {
        expect(warmView.agents[i].segments[s]).toBe(viewSegRefs[i * 3 + s]);
      }
    }

    // Zero retained heap growth over 60 moving ticks (post-GC), and drastically lower churn vs unpooled
    expect(pooledRetainedDelta).toBeLessThan(64 * 1024);
    expect(unpooledBurstDelta).toBeGreaterThan(10 * 1024 * 1024);
    expect(pooledBurstDelta).toBeLessThan(unpooledBurstDelta * 0.2);

    // 3. Structural audit: no Array.from or per-tick new Map / new THREE in pooled hot functions
    const poseSrc = readFileSync(resolve(ROOT, 'src/components/Landscape/utils/agentPose.ts'), 'utf8');
    const runStart = poseSrc.indexOf('run(segments');
    const buildIntoStart = poseSrc.indexOf('export function buildAgentPosesInto(');
    expect(runStart).toBeGreaterThan(0);
    expect(buildIntoStart).toBeGreaterThan(runStart);
    const hotPoseBody = poseSrc.slice(runStart, buildIntoStart);
    expect(hotPoseBody).not.toContain('Array.from');
    expect(hotPoseBody).not.toMatch(/\bnew\s+(?:Map|Set|THREE)\b/);

    const interpSrc = readFileSync(
      resolve(ROOT, 'src/components/Landscape/agentInterpolation.ts'),
      'utf8',
    );
    const fillStart = interpSrc.indexOf('fill(payload');
    const prepIntoStart = interpSrc.indexOf('export function preparePoseSnapshotInto(');
    expect(fillStart).toBeGreaterThan(0);
    expect(prepIntoStart).toBeGreaterThan(fillStart);
    const hotPrepBody = interpSrc.slice(fillStart, prepIntoStart);
    expect(hotPrepBody).not.toContain('Array.from');
    expect(hotPrepBody).not.toMatch(/\bnew\s+(?:Map|Set|THREE)\b/);
  });
});
