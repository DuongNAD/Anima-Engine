import React from 'react';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, cleanup, act } from '@testing-library/react';
import * as THREE from 'three';
import type { SimulationTickPayload } from '../../src/types/generated/SimulationTickPayload';
import type { SegmentState } from '../../src/types/generated/SegmentState';
import type { FrameCallback } from '../mocks/r3f-frame-state';
import { makeFrameState } from '../mocks/r3f-frame-state';
import { installStubbedProperty, removeStubbedProperties } from '../mocks/r3f-dom-stubs';
import { generateWorld, type World } from '../../src/components/Landscape/utils/worldGen';
import { surfaceHeight } from '../../src/components/Landscape/utils/worldSample';
import {
  CREATURE_BODY_SCALE,
  buildAgentPoses,
  computeCreatureColor,
} from '../../src/components/Landscape/utils/agentPose';
import {
  ArchetypeMeshCache,
  SkinnedCreaturePool,
  buildMorphologySkinArchetype,
  countOpenBoundaryEdges,
  type MorphologySegmentInput,
} from '../../src/components/Landscape/utils/creatureSkinMesh';
import {
  CREATURE_NEAR_SKINNED_CAP,
  CreatureLodSelector,
  LOD_CULLED,
  LOD_FAR_MARKER,
  LOD_MID_INSTANCED,
  LOD_NEAR_SKINNED,
} from '../../src/components/Landscape/utils/creatureLod';
import { createSyntheticTickGenerator } from '../../src/components/Landscape/utils/syntheticTicks';
import type { TickSource } from '../../src/components/Landscape/utils/tickRecording';
import {
  TickInterpolationBuffer,
  type InterpolatedAgentView,
} from '../../src/components/Landscape/agentInterpolation';

// prototype, to be replaced — B1 unit & integration tests for continuous skinned bodies, 3-tier LOD, and culling.

const { frameCallbacks } = vi.hoisted(() => ({
  frameCallbacks: [] as FrameCallback[],
}));

vi.mock('@react-three/fiber', async () => {
  return {
    Canvas: ({ children }: { children?: React.ReactNode }) => (
      <div data-testid="mock-canvas">{children}</div>
    ),
    useFrame: (cb: FrameCallback) => {
      frameCallbacks.push(cb);
    },
  };
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(null),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const { LiveAgents } = await import('../../src/components/Landscape/LiveAgents');

interface MockInstancedEl extends HTMLElement {
  count?: number;
  visible?: boolean;
  instanceMatrix?: { needsUpdate: boolean };
  instanceColor?: { needsUpdate: boolean };
  _b1Draws?: number[][];
}

interface MockSkinnedGroupEl extends HTMLElement {
  isObject3D?: boolean;
  _b1Children3D?: THREE.Object3D[];
}

function minDistanceToPosedSkin(mesh: THREE.SkinnedMesh, target: THREE.Vector3): number {
  const index = mesh.geometry.getIndex();
  const pos = mesh.geometry.getAttribute('position');
  if (!index || !pos) return Infinity;
  const va = new THREE.Vector3();
  const vb = new THREE.Vector3();
  const vc = new THREE.Vector3();
  const closest = new THREE.Vector3();
  const tri = new THREE.Triangle();
  let minSq = Infinity;
  const arr = index.array;
  for (let i = 0; i + 2 < arr.length; i += 3) {
    mesh.getVertexPosition(arr[i], va);
    mesh.getVertexPosition(arr[i + 1], vb);
    mesh.getVertexPosition(arr[i + 2], vc);
    tri.set(va, vb, vc);
    tri.closestPointToPoint(target, closest);
    const dSq = closest.distanceToSquared(target);
    if (dSq < minSq) minSq = dSq;
  }
  return Math.sqrt(minSq);
}

function advanceThreeFrame(overrides: Parameters<typeof makeFrameState>[0] = {}): void {
  const state = makeFrameState(overrides);
  for (const cb of frameCallbacks) {
    cb(state, 1 / 60);
  }
}

function makeCameraAt(pos: [number, number, number], target: [number, number, number]): THREE.PerspectiveCamera {
  const cam = new THREE.PerspectiveCamera(55, 16 / 9, 0.5, 4000);
  cam.position.set(pos[0], pos[1], pos[2]);
  cam.lookAt(target[0], target[1], target[2]);
  cam.updateMatrixWorld();
  cam.updateProjectionMatrix();
  return cam;
}

function makeManualSource(): {
  source: TickSource;
  emit: (payload: SimulationTickPayload) => void;
} {
  const listeners = new Set<(payload: SimulationTickPayload) => void>();
  return {
    source: {
      kind: 'tauri',
      screenLabel: null,
      subscribe(listener) {
        listeners.add(listener);
        return () => {
          listeners.delete(listener);
        };
      },
    },
    emit(payload) {
      listeners.forEach((l) => l(payload));
    },
  };
}

function makeSyntheticPayloadSegments(
  agentId: number,
  agentType: 'prey' | 'predator',
  simX: number,
  simZ: number,
  segCount: number,
): SegmentState[] {
  const segs: SegmentState[] = [];
  for (let s = 0; s < segCount; s++) {
    segs.push({
      agent_id: agentId,
      segment_id: s,
      parent_segment_id: s === 0 ? null : s - 1,
      length: s === 0 ? 1.0 : 0.85,
      radius: s === 0 ? 0.25 : 0.2,
      x: simX,
      y: 2.0,
      z: simZ + s * 1.5,
      yaw: 0,
      pitch: 0,
      roll: 0,
      joint_anchor_x: 0,
      joint_anchor_y: 0,
      joint_anchor_z: s === 0 ? 0 : 1.0,
      joint_axis_x: 1,
      joint_axis_y: 0,
      joint_axis_z: 0,
      energy: 80,
      hydration: 75,
      head_direction: [0, 0, 1],
      agent_type: agentType,
    });
  }
  return segs;
}

function makeLodAgentView(id: number, x: number, y: number, z: number): InterpolatedAgentView {
  const buf = new TickInterpolationBuffer();
  buf.push(
    {
      segments: makeSyntheticPayloadSegments(id, id % 3 === 0 ? 'predator' : 'prey', 0, 0, 1),
      environmental_state: { elements: [] },
      head_directions: {},
    },
    0,
    1200,
    () => 0,
  );
  const sampled = buf.sampleLatest().agents[0];
  sampled.rootPosition[0] = x;
  sampled.rootPosition[1] = y;
  sampled.rootPosition[2] = z;
  sampled.boundingRadius = 4;
  return sampled;
}

describe('B1 continuous SkinnedMesh bodies, 3-tier LOD, and synthetic env', () => {
  let world: World;

  beforeEach(() => {
    frameCallbacks.length = 0;
    vi.clearAllMocks();
    world = generateWorld('b1-test-world', { size: 32, shape: 'island' });

    installStubbedProperty(HTMLElement.prototype, 'isObject3D', true);
    installStubbedProperty(HTMLElement.prototype, 'add', function (
      this: MockSkinnedGroupEl,
      obj: THREE.Object3D,
    ) {
      if (!this._b1Children3D) this._b1Children3D = [];
      if (!this._b1Children3D.includes(obj)) {
        this._b1Children3D.push(obj);
      }
      return this;
    });
    installStubbedProperty(HTMLElement.prototype, 'remove', function (
      this: MockSkinnedGroupEl,
      obj?: THREE.Object3D,
    ) {
      if (!obj) return this;
      if (this._b1Children3D) {
        this._b1Children3D = this._b1Children3D.filter((item) => item !== obj);
      }
      return this;
    });
    installStubbedProperty(
      HTMLElement.prototype,
      'setMatrixAt',
      vi.fn(function (this: MockInstancedEl, index: number, matrix: THREE.Matrix4) {
        if (!this._b1Draws) this._b1Draws = [];
        this._b1Draws[index] = Array.from(matrix.elements);
      }),
    );
    installStubbedProperty(HTMLElement.prototype, 'setColorAt', vi.fn());
  });

  afterEach(() => {
    cleanup();
    removeStubbedProperties(
      HTMLElement.prototype,
      'isObject3D',
      'add',
      'remove',
      'setMatrixAt',
      'setColorAt',
    );
    document.querySelectorAll('[data-testid^="rw-"]').forEach((node) => node.remove());
  });

  it('builds watertight manifold continuous skin archetypes for 1, 3 (founder gap), 5 (branches), and 9 segments', () => {
    // 1 segment
    const arch1 = buildMorphologySkinArchetype('k-1', [
      {
        segmentId: 0,
        parentSegmentId: null,
        isRoot: true,
        renderLength: 3.0,
        renderRadius: 1.0,
        jointAnchor: [0, 0, 0],
      },
    ]);
    expect(arch1.boneCount).toBe(1);
    expect(countOpenBoundaryEdges(arch1.geometry)).toBe(0);

    // 3-segment founder morphology matching `genotype.rs`: anchor [0, 0, 1.0] leaves gap between rear of 0 (+0.5) and front of 1 (+1.0).
    const founder3: MorphologySegmentInput[] = [
      {
        segmentId: 0,
        parentSegmentId: null,
        isRoot: true,
        renderLength: 1.0,
        renderRadius: 0.25,
        jointAnchor: [0, 0, 0],
      },
      {
        segmentId: 1,
        parentSegmentId: 0,
        isRoot: false,
        renderLength: 1.0,
        renderRadius: 0.25,
        jointAnchor: [0, 0, 1.0],
      },
      {
        segmentId: 2,
        parentSegmentId: 1,
        isRoot: false,
        renderLength: 1.0,
        renderRadius: 0.25,
        jointAnchor: [0, 0, 1.0],
      },
    ];
    const arch3a = buildMorphologySkinArchetype('founder-3', founder3);
    const arch3b = buildMorphologySkinArchetype('founder-3', founder3);
    expect(arch3a.boneCount).toBe(3);
    expect(countOpenBoundaryEdges(arch3a.geometry)).toBe(0);
    // Exact Rust equilibrium centers: 0, +1.5, +3.0
    expect(arch3a.restPositions[0][2]).toBeCloseTo(0, 5);
    expect(arch3a.restPositions[1][2]).toBeCloseTo(1.5, 5);
    expect(arch3a.restPositions[2][2]).toBeCloseTo(3.0, 5);

    // Verify deterministic positions and continuous Bezier bridge vertices across the [0.5, 1.0] gap.
    const posAttr = arch3a.geometry.getAttribute('position') as THREE.BufferAttribute;
    const posB = arch3b.geometry.getAttribute('position') as THREE.BufferAttribute;
    expect(Array.from(posAttr.array)).toEqual(Array.from(posB.array));

    let bridgeVertsInGap = 0;
    for (let i = 0; i < posAttr.count; i++) {
      const z = posAttr.getZ(i);
      if (z > 0.55 && z < 0.95) bridgeVertsInGap++;
    }
    expect(bridgeVertsInGap).toBeGreaterThan(0);

    // Verify normalized skinWeights (`sum ~= 1.0`), valid skinIndex bounds, AND distance-dependent bone ownership (kills M4).
    const skinW = arch3a.geometry.getAttribute('skinWeight') as THREE.BufferAttribute;
    const skinI = arch3a.geometry.getAttribute('skinIndex') as THREE.BufferAttribute;
    let dominantBone0 = 0;
    let dominantBone1 = 0;
    let dominantBone2 = 0;
    for (let i = 0; i < posAttr.count; i++) {
      const sum = skinW.getX(i) + skinW.getY(i) + skinW.getZ(i) + skinW.getW(i);
      expect(sum).toBeCloseTo(1.0, 4);
      expect(skinI.getX(i)).toBeLessThan(3);
      const vz = posAttr.getZ(i);
      if (vz < 0.2 && skinI.getX(i) === 0) dominantBone0++;
      if (Math.abs(vz - 1.5) < 0.3 && skinI.getX(i) === 1) dominantBone1++;
      if (vz > 2.7 && skinI.getX(i) === 2) dominantBone2++;
    }
    expect(dominantBone0).toBeGreaterThan(0);
    expect(dominantBone1).toBeGreaterThan(0);
    expect(dominantBone2).toBeGreaterThan(0);

    // 5-segment creature with paired lateral appendages.
    const branch5: MorphologySegmentInput[] = [
      founder3[0],
      founder3[1],
      founder3[2],
      {
        segmentId: 3,
        parentSegmentId: 0,
        isRoot: false,
        renderLength: 0.9,
        renderRadius: 0.14,
        jointAnchor: [0.24, -0.08, 0],
      },
      {
        segmentId: 4,
        parentSegmentId: 0,
        isRoot: false,
        renderLength: 0.9,
        renderRadius: 0.14,
        jointAnchor: [-0.24, -0.08, 0],
      },
    ];
    const arch5 = buildMorphologySkinArchetype('branch-5', branch5);
    expect(arch5.boneCount).toBe(5);
    expect(countOpenBoundaryEdges(arch5.geometry)).toBe(0);

    // 9-segment morphology from synthetic ticks
    const gen = createSyntheticTickGenerator({ agents: 2, segmentsPerAgent: 9, seed: 7 });
    const poses = buildAgentPoses(gen.tickAt(0).segments, 1200, () => 0);
    const morph9: MorphologySegmentInput[] = poses[0].segments.map((s) => ({
      segmentId: s.segmentId,
      parentSegmentId: s.parentSegmentId,
      isRoot: s.isRoot,
      renderLength: s.renderLength,
      renderRadius: s.renderRadius,
      jointAnchor: s.jointAnchor,
    }));
    const arch9 = buildMorphologySkinArchetype(poses[0].morphologyKey, morph9);
    expect(arch9.boneCount).toBe(9);
    expect(countOpenBoundaryEdges(arch9.geometry)).toBe(0);
    expect(arch9.triangleCount).toBeGreaterThan(100);

    arch1.geometry.dispose();
    arch3a.geometry.dispose();
    arch3b.geometry.dispose();
    arch5.geometry.dispose();
    arch9.geometry.dispose();
  });

  it('caches and evicts archetypes deterministically by morphologyKey in ArchetypeMeshCache', () => {
    const cache = new ArchetypeMeshCache(8);
    const segs: MorphologySegmentInput[] = [
      {
        segmentId: 0,
        parentSegmentId: null,
        isRoot: true,
        renderLength: 2,
        renderRadius: 0.6,
        jointAnchor: [0, 0, 0],
      },
    ];
    const first = cache.getOrBuild('alpha', segs);
    const second = cache.getOrBuild('alpha', segs);
    expect(second).toBe(first);
    expect(cache.size()).toBe(1);

    for (let i = 0; i < 12; i++) {
      cache.getOrBuild(`morph-${i}`, segs);
    }
    expect(cache.size()).toBe(8);
    cache.disposeAll();
    expect(cache.size()).toBe(0);
  });

  it('enforces 3-tier LOD caps K=8/12/16 and frustum culls behind positioned cameras', () => {
    const selector = new CreatureLodSelector();
    const agents: InterpolatedAgentView[] = [];
    for (let i = 0; i < 40; i++) {
      // Cluster inside 80 units along +Z? Place in front of cam looking toward origin from z=+50.
      agents.push(makeLodAgentView(i + 1, (i - 20) * 1.2, 0, i * 1.5 - 10));
    }
    const cam = makeCameraAt([0, 20, 90], [0, 0, 0]);

    expect(CREATURE_NEAR_SKINNED_CAP).toEqual({ low: 8, medium: 12, high: 16 });
    expect(CREATURE_BODY_SCALE).toBeLessThan(0.5);
    for (const tier of ['low', 'medium', 'high'] as const) {
      const res = selector.select(agents, agents.length, cam, tier, 1200);
      expect(res.nearCount).toBe(CREATURE_NEAR_SKINNED_CAP[tier]);
      expect(res.nearCount + res.midCount).toBe(agents.length);
      expect(res.farCount).toBe(0);
      expect(res.culledCount).toBe(0);
    }

    // Distance transition + frustum culling checks on medium (`near<=160`, `mid<=360`, `far>360`).
    const mixed: InterpolatedAgentView[] = [
      makeLodAgentView(1, 0, 0, 100), // dist = 100 from z=200 -> near
      makeLodAgentView(2, 0, 0, -50), // dist = 250 from z=200 -> mid
      makeLodAgentView(3, 0, 0, -250), // dist = 450 from z=200 -> far
      makeLodAgentView(4, 0, 0, 400), // behind camera at z=200 looking toward -Z -> culled
    ];
    const frustumCam = makeCameraAt([0, 0, 200], [0, 0, -500]);
    const mixRes = selector.select(mixed, mixed.length, frustumCam, 'medium', 1200);
    expect(mixRes.tiers[0]).toBe(LOD_NEAR_SKINNED);
    expect(mixRes.tiers[1]).toBe(LOD_MID_INSTANCED);
    expect(mixRes.tiers[2]).toBe(LOD_FAR_MARKER);
    expect(mixRes.tiers[3]).toBe(LOD_CULLED);
  });

  it('includes deterministic synthetic lakes, trees, and food in syntheticTicks without mutating simulation claims', () => {
    const genA = createSyntheticTickGenerator({ agents: 6, seed: 42 });
    const genB = createSyntheticTickGenerator({ agents: 6, seed: 42 });
    const tickA = genA.tickAt(0);
    const tickB = genB.tickAt(0);

    expect(genA.screenLabel.toLowerCase()).toContain('synthetic');
    expect(tickA.environmental_state.elements.length).toBeGreaterThan(10);
    expect(tickA.environmental_state.elements).toEqual(tickB.environmental_state.elements);

    const types = new Set(tickA.environmental_state.elements.map((e) => e.type));
    expect(types.has('lake')).toBe(true);
    expect(types.has('tree')).toBe(true);
    expect(types.has('food')).toBe(true);
  });

  it('renders continuous SkinnedMesh bodies in LiveAgents with class+hash PBR colors, mid/far LOD fallback, and click selection', () => {
    const manual = makeManualSource();
    const { container } = render(
      <LiveAgents
        world={world}
        renderSize={1200}
        heightRatio={0.14}
        meshResolution={64}
        tickSource={manual.source}
        qualityTier="low"
      />,
    );

    // Emit 12 near prey/predator + 1 mid + 1 far + 1 behind-camera culled creature.
    const segments: SegmentState[] = [];
    for (let i = 0; i < 12; i++) {
      segments.push(
        ...makeSyntheticPayloadSegments(i + 1, i === 1 ? 'predator' : 'prey', (i - 6) * 0.5, 2 + (i % 3) * 0.5, 3),
      );
    }
    // Agent 100 at mid range (`sim Z = -16 -> render Z = -96`, dist ~157 in `(90, 220]` on low -> MID)
    segments.push(...makeSyntheticPayloadSegments(100, 'prey', 0, -16, 3));
    // Agent 101 at far range (`sim Z = -60 -> render Z = -360`, dist ~420 > 220 -> FAR on low!)
    segments.push(...makeSyntheticPayloadSegments(101, 'prey', 0, -60, 3));
    // Agent 102 behind camera (`sim Z = +40 -> render Z = +240`, behind cam at z=60 looking toward -Z -> CULLED!)
    segments.push(...makeSyntheticPayloadSegments(102, 'predator', 0, 40, 3));

    act(() => {
      manual.emit({
        segments,
        environmental_state: { elements: [] },
        head_directions: {},
      });
    });

    const cam = makeCameraAt([0, 20, 60], [0, 0, -100]);
    act(() => {
      advanceThreeFrame({ camera: cam, size: { width: 1280, height: 720 } });
    });

    const skinnedGroup = container.querySelector('group[name="live-agents-skinned"]') as MockSkinnedGroupEl;
    expect(skinnedGroup).not.toBeNull();
    const visibleSkinned = (skinnedGroup._b1Children3D ?? []).filter(
      (obj): obj is THREE.SkinnedMesh => obj instanceof THREE.SkinnedMesh && obj.visible,
    );
    // Capped at low tier K = 8 (12 near candidates -> 8 skinned + 4 overflow + 1 mid = 5 mid creatures = 15 segments)
    expect(visibleSkinned.length).toBe(CREATURE_NEAR_SKINNED_CAP.low);

    // Check first visible SkinnedMesh uses MeshStandardMaterial colored by computeCreatureColor.
    const firstMesh = visibleSkinned[0];
    const targetId = firstMesh.userData.agentId as number;
    const expectedType = targetId === 2 ? 'predator' : 'prey';
    expect(firstMesh.material).toBeInstanceOf(THREE.MeshStandardMaterial);
    const expectedColor = computeCreatureColor(expectedType, targetId, 80);
    const mat = firstMesh.material as THREE.MeshStandardMaterial;
    expect(mat.color.r).toBeCloseTo(expectedColor.r, 4);
    expect(mat.color.g).toBeCloseTo(expectedColor.g, 4);
    expect(mat.color.b).toBeCloseTo(expectedColor.b, 4);
    expect(firstMesh.skeleton.bones.length).toBe(3);
    expect(
      Math.abs(firstMesh.skeleton.bones[1].position.z - firstMesh.skeleton.bones[0].position.z),
    ).toBeCloseTo(1.5 * 6 * CREATURE_BODY_SCALE, 4);

    const segMesh = container.querySelector('instancedmesh[name="live-agents-segments"]') as MockInstancedEl;
    const markerMesh = container.querySelector('instancedmesh[name="live-agents-markers"]') as MockInstancedEl;
    expect(segMesh.count).toBe(15); // 4 overflow + 1 mid * 3 segs each, while agent 102 is culled
    expect(markerMesh.count).toBe(1); // agent 101 far marker

    // Clicking live-agents-skinned with an intersected SkinnedMesh selects that agent cleanly.
    const reactPropsEntry = Object.entries(skinnedGroup).find(([k]) => k.startsWith('__reactProps$'));
    const onClick = (
      reactPropsEntry?.[1] as { onClick?: (e: { object: THREE.Object3D; stopPropagation: () => void }) => void } | undefined
    )?.onClick;
    expect(onClick).toBeDefined();
    const stopPropagation = vi.fn();
    act(() => {
      onClick?.({ object: firstMesh, stopPropagation });
    });
    expect(stopPropagation).toHaveBeenCalledTimes(1);
    const hudId = document.body.querySelector('[data-testid="rw-selected-agent-id"]');
    expect(hudId?.textContent).toBe(String(targetId));
  });

  it('R1/R2: keeps synthetic joint anchors inside Rust mutation.rs limits across 3..15 segments', () => {
    for (const segCount of [3, 5, 9, 15]) {
      const gen = createSyntheticTickGenerator({ agents: 12, segmentsPerAgent: segCount, seed: 1 });
      const payload = gen.tickAt(0);
      const segByKey = new Map(payload.segments.map((s) => [`${s.agent_id}:${s.segment_id}`, s]));
      for (const s of payload.segments) {
        if (s.parent_segment_id === null) continue;
        const parent = segByKey.get(`${s.agent_id}:${s.parent_segment_id}`)!;
        expect(Math.abs(s.joint_anchor_x)).toBeLessThanOrEqual(parent.radius + 1e-6);
        expect(Math.abs(s.joint_anchor_y)).toBeLessThanOrEqual(parent.radius + 1e-6);
        expect(Math.abs(s.joint_anchor_z)).toBeLessThanOrEqual(parent.length * 0.5 + 1e-6);
      }
    }
  });

  it('R2 & N5: aligns near-skinned eyes and predator snout flush with posed skin for live founder (1,0,0) and +L/2 chains', () => {
    const buildRotatedFounderSegments = (
      agentId: number,
      agentType: 'prey' | 'predator',
      simX: number,
      simZ: number,
      yaw: number,
    ): SegmentState[] => {
      const q = new THREE.Quaternion().setFromEuler(new THREE.Euler(0, yaw, 0, 'YXZ'));
      const fwd = new THREE.Vector3(0, 0, 1).applyQuaternion(q);
      const aVec = new THREE.Vector3();
      const halfVec = new THREE.Vector3(0, 0, 0.5).applyQuaternion(q);
      const segs: SegmentState[] = [
        {
          agent_id: agentId,
          segment_id: 0,
          parent_segment_id: null,
          length: 1.0,
          radius: 0.2,
          x: simX,
          y: 1.0,
          z: simZ,
          yaw,
          pitch: 0,
          roll: 0,
          joint_anchor_x: 0,
          joint_anchor_y: 0,
          joint_anchor_z: 0,
          joint_axis_x: 0,
          joint_axis_y: 0,
          joint_axis_z: 0,
          energy: 90,
          hydration: 80,
          head_direction: [fwd.x, fwd.y, fwd.z],
          agent_type: agentType,
        },
      ];
      for (let s = 1; s <= 2; s++) {
        const p = segs[s - 1];
        aVec.set(1.0, 0, 0).applyQuaternion(q);
        segs.push({
          agent_id: agentId,
          segment_id: s,
          parent_segment_id: s - 1,
          length: 1.0,
          radius: 0.2,
          x: p.x + aVec.x + halfVec.x,
          y: p.y + aVec.y + halfVec.y,
          z: p.z + aVec.z + halfVec.z,
          yaw,
          pitch: 0,
          roll: 0,
          joint_anchor_x: 1.0,
          joint_anchor_y: 0,
          joint_anchor_z: 0,
          joint_axis_x: 0,
          joint_axis_y: 0,
          joint_axis_z: 1,
          energy: 90,
          hydration: 80,
          head_direction: [fwd.x, fwd.y, fwd.z],
          agent_type: agentType,
        });
      }
      return segs;
    };

    const manual = makeManualSource();
    const { container, unmount } = render(
      <LiveAgents
        world={world}
        renderSize={1200}
        heightRatio={0.14}
        meshResolution={64}
        tickSource={manual.source}
        qualityTier="high"
      />,
    );

    const payload: SimulationTickPayload = {
      segments: [
        ...buildRotatedFounderSegments(1, 'prey', 0, 0, 0.7),
        ...buildRotatedFounderSegments(2, 'predator', 4, 0, -0.5),
        // Agent 3: mutation +L/2 forward chain (`joint_anchor = [0, 0, 0.5]`, `pos.z = 0, 1.0, 2.0`)
        {
          agent_id: 3,
          segment_id: 0,
          parent_segment_id: null,
          length: 1.0,
          radius: 0.2,
          x: -4,
          y: 1.0,
          z: 0,
          yaw: 0,
          pitch: 0,
          roll: 0,
          joint_anchor_x: 0,
          joint_anchor_y: 0,
          joint_anchor_z: 0,
          joint_axis_x: 0,
          joint_axis_y: 0,
          joint_axis_z: 0,
          energy: 90,
          hydration: 80,
          head_direction: [0, 0, 1],
          agent_type: 'prey',
        },
        {
          agent_id: 3,
          segment_id: 1,
          parent_segment_id: 0,
          length: 1.0,
          radius: 0.2,
          x: -4,
          y: 1.0,
          z: 1.0,
          yaw: 0,
          pitch: 0,
          roll: 0,
          joint_anchor_x: 0,
          joint_anchor_y: 0,
          joint_anchor_z: 0.5,
          joint_axis_x: 0,
          joint_axis_y: 1,
          joint_axis_z: 0,
          energy: 90,
          hydration: 80,
          head_direction: [0, 0, 1],
          agent_type: 'prey',
        },
        {
          agent_id: 3,
          segment_id: 2,
          parent_segment_id: 1,
          length: 1.0,
          radius: 0.2,
          x: -4,
          y: 1.0,
          z: 2.0,
          yaw: 0,
          pitch: 0,
          roll: 0,
          joint_anchor_x: 0,
          joint_anchor_y: 0,
          joint_anchor_z: 0.5,
          joint_axis_x: 0,
          joint_axis_y: 1,
          joint_axis_z: 0,
          energy: 90,
          hydration: 80,
          head_direction: [0, 0, 1],
          agent_type: 'prey',
        },
      ],
      environmental_state: { elements: [] },
      head_directions: {},
    };

    act(() => {
      manual.emit(payload);
    });
    const gY0 = surfaceHeight(world, 0, 0, 1200, 0.14, 64);
    const cam = makeCameraAt([0, gY0 + 35, 55], [0, gY0, 0]);
    act(() => {
      advanceThreeFrame({ camera: cam, size: { width: 1280, height: 720 } });
    });

    const skinnedGroup = container.querySelector('group[name="live-agents-skinned"]') as MockSkinnedGroupEl;
    const sphereMesh = container.querySelector('instancedmesh[name="live-agents-spheres"]') as MockInstancedEl;
    const snoutMesh = container.querySelector('instancedmesh[name="live-agents-snouts"]') as MockInstancedEl;
    const visibleSkinned = (skinnedGroup._b1Children3D ?? []).filter(
      (obj): obj is THREE.SkinnedMesh => obj instanceof THREE.SkinnedMesh && obj.visible,
    );
    expect(visibleSkinned.length).toBe(3);
    expect(sphereMesh.count).toBe(6); // 2 eyes * 3 creatures
    expect(snoutMesh.count).toBe(1); // agent 2 predator snout

    const tmpMat = new THREE.Matrix4();
    const eyePos = new THREE.Vector3();
    const eyeQuat = new THREE.Quaternion();
    const eyeScale = new THREE.Vector3();
    const meshByAgent = new Map(visibleSkinned.map((m) => [m.userData.agentId as number, m]));
    const rRadFounder = 0.2 * 6 * CREATURE_BODY_SCALE;

    // All 6 eye centers sit flush within eyeRadius of their creature's posed SkinnedMesh, and eye radius scales down with CREATURE_BODY_SCALE (N5).
    for (let i = 0; i < 6; i++) {
      tmpMat.fromArray(sphereMesh._b1Draws![i]);
      tmpMat.decompose(eyePos, eyeQuat, eyeScale);
      const ownerId = i < 2 ? 1 : i < 4 ? 2 : 3;
      const dist = minDistanceToPosedSkin(meshByAgent.get(ownerId)!, eyePos);
      expect(dist).toBeLessThanOrEqual(eyeScale.x);
      expect(eyeScale.x / rRadFounder).toBeLessThan(0.32);
    }

    // +L/2 forward chain places eyes near leading bone 2 nose, not buried behind child 1.
    tmpMat.fromArray(sphereMesh._b1Draws![4]);
    tmpMat.decompose(eyePos, eyeQuat, eyeScale);
    const chainBone2Z = meshByAgent.get(3)!.skeleton.bones[2].position.z;
    expect(eyePos.z).toBeGreaterThan(chainBone2Z);

    // Predator snout anchors inside the front nose dome and extends forward past it (no floating gap).
    const snoutMat = new THREE.Matrix4().fromArray(snoutMesh._b1Draws![0]);
    const snoutPos = new THREE.Vector3();
    const snoutQuat = new THREE.Quaternion();
    const snoutScale = new THREE.Vector3();
    snoutMat.decompose(snoutPos, snoutQuat, snoutScale);
    const snoutFwd = new THREE.Vector3(0, 0, 1).applyQuaternion(snoutQuat).normalize();
    const snoutUp = new THREE.Vector3(0, 1, 0).applyQuaternion(snoutQuat).normalize();
    const snoutBaseCollar = snoutPos
      .clone()
      .addScaledVector(snoutFwd, -snoutScale.z * 0.25)
      .addScaledVector(snoutUp, snoutScale.y * 0.5);
    expect(minDistanceToPosedSkin(meshByAgent.get(2)!, snoutBaseCollar)).toBeLessThanOrEqual(
      snoutScale.x * 0.6,
    );
    expect(snoutPos.clone().addScaledVector(snoutFwd, snoutScale.z * 0.5).distanceTo(meshByAgent.get(2)!.skeleton.bones[0].position)).toBeGreaterThan(
      0.96,
    );

    unmount();
  });

  it('R3: raycasts moved SkinnedMesh creatures and returns zero hits on hidden slots', () => {
    vi.useFakeTimers();
    let clockMs = 1000;
    const manual = makeManualSource();
    const { container, unmount } = render(
      <LiveAgents
        world={world}
        renderSize={1200}
        heightRatio={0.14}
        meshResolution={64}
        tickSource={manual.source}
        nowMs={() => clockMs}
        qualityTier="low"
      />,
    );

    const gYA = surfaceHeight(world, 0, 0, 1200, 0.14, 64);
    const cam = makeCameraAt([0, gYA + 35, 45], [0, gYA, 0]);
    act(() => {
      manual.emit({
        segments: makeSyntheticPayloadSegments(1, 'prey', 0, 0, 3),
        environmental_state: { elements: [] },
        head_directions: {},
      });
    });
    act(() => {
      advanceThreeFrame({ camera: cam, size: { width: 1280, height: 720 } });
    });

    const skinnedGroup = container.querySelector('group[name="live-agents-skinned"]') as MockSkinnedGroupEl;
    const mesh = (skinnedGroup._b1Children3D ?? []).find(
      (obj): obj is THREE.SkinnedMesh => obj instanceof THREE.SkinnedMesh && obj.visible,
    )!;
    expect(mesh).toBeDefined();

    const bone0 = mesh.skeleton.bones[0].position.clone();
    const raycaster = new THREE.Raycaster();
    raycaster.set(new THREE.Vector3(bone0.x, bone0.y + 50, bone0.z), new THREE.Vector3(0, -1, 0));
    const hitsA = raycaster.intersectObject(mesh, false);
    expect(hitsA.length).toBeGreaterThan(0);
    expect(hitsA[0].object.userData.agentId).toBe(1);

    // Move creature 120 render units (`simX = 20`) cleanly to position B and advance past delay.
    act(() => {
      clockMs = 1033;
      manual.emit({
        segments: makeSyntheticPayloadSegments(1, 'prey', 20, 0, 3),
        environmental_state: { elements: [] },
        head_directions: {},
      });
    });
    clockMs = 1066;
    const gYB = surfaceHeight(world, 120, 0, 1200, 0.14, 64);
    const camB = makeCameraAt([120, gYB + 35, 45], [120, gYB, 0]);
    act(() => {
      advanceThreeFrame({ camera: camB, size: { width: 1280, height: 720 } });
    });

    // Old position A misses; new position B hits!
    raycaster.set(new THREE.Vector3(bone0.x, bone0.y + 50, bone0.z), new THREE.Vector3(0, -1, 0));
    expect(raycaster.intersectObject(mesh, false)).toHaveLength(0);

    const movedPos = mesh.skeleton.bones[0].position.clone();
    expect(movedPos.x).toBeCloseTo(120, 1);
    raycaster.set(new THREE.Vector3(movedPos.x, movedPos.y + 50, movedPos.z), new THREE.Vector3(0, -1, 0));
    const hitsB = raycaster.intersectObject(mesh, false);
    expect(hitsB.length).toBeGreaterThan(0);
    expect(hitsB[0].object.userData.agentId).toBe(1);

    // Move camera so creature is behind frustum -> culled -> hidden slot never returns ghost hits.
    const behindCam = makeCameraAt([120, gYB + 35, -120], [120, gYB, -240]);
    act(() => {
      advanceThreeFrame({ camera: behindCam, size: { width: 1280, height: 720 } });
    });
    expect(mesh.visible).toBe(false);
    raycaster.set(new THREE.Vector3(movedPos.x, movedPos.y + 50, movedPos.z), new THREE.Vector3(0, -1, 0));
    expect(raycaster.intersectObject(mesh, false)).toHaveLength(0);

    vi.useRealTimers();
    unmount();
  });

  it('N1 & N2: wires WorldShowcase qualityTier prop and runs zero URLSearchParams allocations inside useFrame', () => {
    const here = dirname(fileURLToPath(import.meta.url));
    const showcaseSrc = readFileSync(
      resolve(here, '../../src/components/Landscape/WorldShowcase.tsx'),
      'utf8',
    );
    expect(showcaseSrc).toMatch(/<LiveAgents[\s\S]*qualityTier=\{effectiveQuality\}/);

    const manual = makeManualSource();
    const { container, rerender, unmount } = render(
      <LiveAgents
        world={world}
        renderSize={1200}
        heightRatio={0.14}
        meshResolution={64}
        tickSource={manual.source}
        qualityTier="low"
      />,
    );

    const segs: SegmentState[] = [];
    for (let i = 1; i <= 20; i++) {
      segs.push(...makeSyntheticPayloadSegments(i, 'prey', (i - 10) * 0.4, 0, 3));
    }
    act(() => {
      manual.emit({ segments: segs, environmental_state: { elements: [] }, head_directions: {} });
    });

    const gY = surfaceHeight(world, 0, 0, 1200, 0.14, 64);
    const cam = makeCameraAt([0, gY + 30, 50], [0, gY, 0]);
    const OrigURLSearchParams = globalThis.URLSearchParams;
    let urlSearchConstructs = 0;
    const urlSpy = vi
      .spyOn(globalThis, 'URLSearchParams')
      .mockImplementation(function (this: URLSearchParams, init?: ConstructorParameters<typeof URLSearchParams>[0]) {
        urlSearchConstructs++;
        return new OrigURLSearchParams(init);
      } as unknown as typeof URLSearchParams);

    for (let f = 0; f < 5; f++) {
      act(() => {
        advanceThreeFrame({ camera: cam, size: { width: 1280, height: 720 } });
      });
    }
    expect(urlSearchConstructs).toBe(0);

    const skinnedGroup = container.querySelector('group[name="live-agents-skinned"]') as MockSkinnedGroupEl;
    const countVisible = () =>
      (skinnedGroup._b1Children3D ?? []).filter(
        (o): o is THREE.SkinnedMesh => o instanceof THREE.SkinnedMesh && o.visible,
      ).length;

    expect(countVisible()).toBe(CREATURE_NEAR_SKINNED_CAP.low);
    rerender(
      <LiveAgents
        world={world}
        renderSize={1200}
        heightRatio={0.14}
        meshResolution={64}
        tickSource={manual.source}
        qualityTier="medium"
      />,
    );
    act(() => {
      advanceThreeFrame({ camera: cam, size: { width: 1280, height: 720 } });
    });
    expect(countVisible()).toBe(CREATURE_NEAR_SKINNED_CAP.medium);

    rerender(
      <LiveAgents
        world={world}
        renderSize={1200}
        heightRatio={0.14}
        meshResolution={64}
        tickSource={manual.source}
        qualityTier="high"
      />,
    );
    act(() => {
      advanceThreeFrame({ camera: cam, size: { width: 1280, height: 720 } });
    });
    expect(countVisible()).toBe(CREATURE_NEAR_SKINNED_CAP.high);

    urlSpy.mockRestore();
    unmount();
  });

  it('N3, N4, N6 & radius-width: protects active LRU geometry, keeps sticky slots + top-8 shadows, snaps morphology change, and scales skin width by radius', () => {
    // 1. N3: LRU cache never disposes a geometry in `protectedKeys` during eviction overflow.
    const cache = new ArchetypeMeshCache(8);
    const baseSegs: MorphologySegmentInput[] = [
      {
        segmentId: 0,
        parentSegmentId: null,
        isRoot: true,
        renderLength: 2,
        renderRadius: 0.6,
        jointAnchor: [0, 0, 0],
      },
    ];
    const m0 = cache.getOrBuild('m0', baseSegs);
    const m1 = cache.getOrBuild('m1', baseSegs);
    const dispose0 = vi.spyOn(m0.geometry, 'dispose');
    const dispose1 = vi.spyOn(m1.geometry, 'dispose');
    for (let i = 2; i < 8; i++) cache.getOrBuild(`m${i}`, baseSegs);

    const protectedSet = new Set(['m0']);
    cache.getOrBuild('m8', baseSegs, protectedSet);
    expect(dispose0).not.toHaveBeenCalled();
    expect(dispose1).toHaveBeenCalledTimes(1);

    // Once unprotected, m0 becomes eligible for normal eviction.
    cache.getOrBuild('m9', baseSegs, new Set());
    expect(dispose0).toHaveBeenCalledTimes(1);
    cache.disposeAll();

    // 2. N4 & Radius-width: sticky slots by agentId, closest-8 shadow assignment, and posed width reflects radius.
    const buf = new TickInterpolationBuffer();
    const segs: SegmentState[] = [];
    for (let id = 1; id <= 12; id++) {
      const r = id === 1 ? 0.15 : id === 2 ? 0.45 : 0.25;
      segs.push(
        {
          agent_id: id,
          segment_id: 0,
          parent_segment_id: null,
          length: 1.0,
          radius: r,
          x: id * 0.5,
          y: 1.0,
          z: id * 2.0,
          yaw: 0,
          pitch: 0,
          roll: 0,
          joint_anchor_x: 0,
          joint_anchor_y: 0,
          joint_anchor_z: 0,
          joint_axis_x: 0,
          joint_axis_y: 0,
          joint_axis_z: 0,
          energy: 80,
          hydration: 80,
          head_direction: [0, 0, 1],
          agent_type: 'prey',
        },
        {
          agent_id: id,
          segment_id: 1,
          parent_segment_id: 0,
          length: 1.0,
          radius: r,
          x: id * 0.5,
          y: 1.0,
          z: id * 2.0 + 1.0,
          yaw: 0,
          pitch: 0,
          roll: 0,
          joint_anchor_x: 0,
          joint_anchor_y: 0,
          joint_anchor_z: 0.5,
          joint_axis_x: 0,
          joint_axis_y: 1,
          joint_axis_z: 0,
          energy: 80,
          hydration: 80,
          head_direction: [0, 0, 1],
          agent_type: 'prey',
        },
      );
    }
    buf.push(
      { segments: segs, environmental_state: { elements: [] }, head_directions: {} },
      0,
      1200,
      () => 0,
      CREATURE_BODY_SCALE,
    );
    const frame1 = buf.sampleLatest();
    const cam = makeCameraAt([0, 5, -20], [0, 5, 100]);
    const selector = new CreatureLodSelector();
    const pool = new SkinnedCreaturePool(16);
    const scratch = {
      pos: new THREE.Vector3(),
      quat: new THREE.Quaternion(),
      scale: new THREE.Vector3(),
      color: new THREE.Color(),
    };

    const lod1 = selector.select(frame1.agents, frame1.agentCount, cam, 'high', 1200);
    const slots1 = pool.prepareNearFrame(frame1.agents, lod1.nearIndices, lod1.nearCount);
    let slotForAgent5Frame1 = -1;
    let skeletonAgent5Frame1: THREE.Skeleton | null = null;
    const meshById = new Map<number, THREE.SkinnedMesh>();
    const group = new THREE.Group();
    pool.bindGroup(group);

    for (let r = 0; r < lod1.nearCount; r++) {
      const p = frame1.agents[lod1.nearIndices[r]];
      const s = slots1[r];
      pool.updateSlot(s, p, 0, scratch, r < 8);
      const m = group.children[s] as THREE.SkinnedMesh;
      meshById.set(p.agentId, m);
      expect(m.castShadow).toBe(r < 8);
      if (p.agentId === 5) {
        slotForAgent5Frame1 = s;
        skeletonAgent5Frame1 = m.skeleton;
      }
    }

    // Skinned-tier radius drives posed mesh X-width (`radius = 0.45` vs `0.15`).
    const spanX = (m: THREE.SkinnedMesh): number => {
      const attr = m.geometry.getAttribute('position');
      const v = new THREE.Vector3();
      let minX = Infinity;
      let maxX = -Infinity;
      for (let i = 0; i < attr.count; i++) {
        m.getVertexPosition(i, v);
        if (v.x < minX) minX = v.x;
        if (v.x > maxX) maxX = v.x;
      }
      return maxX - minX;
    };
    expect(spanX(meshById.get(2)!) / spanX(meshById.get(1)!)).toBeGreaterThan(2.2);

    // Frame 2 inserts agent 99 right next to camera -> shifts everyone's rank, but agent 5 keeps its sticky slot & Skeleton!
    const segs2 = [
      ...segs,
      {
        ...segs[0],
        agent_id: 99,
        x: 0,
        z: 0,
      },
      {
        ...segs[1],
        agent_id: 99,
        x: 0,
        z: 1.0,
      },
    ];
    buf.push(
      { segments: segs2, environmental_state: { elements: [] }, head_directions: {} },
      33,
      1200,
      () => 0,
      CREATURE_BODY_SCALE,
    );
    const frame2 = buf.sampleLatest();
    const lod2 = selector.select(frame2.agents, frame2.agentCount, cam, 'high', 1200);
    const slots2 = pool.prepareNearFrame(frame2.agents, lod2.nearIndices, lod2.nearCount);
    for (let r = 0; r < lod2.nearCount; r++) {
      const p = frame2.agents[lod2.nearIndices[r]];
      const s = slots2[r];
      pool.updateSlot(s, p, 0.03, scratch, r < 8);
      const m = group.children[s] as THREE.SkinnedMesh;
      if (p.agentId === 5) {
        expect(s).toBe(slotForAgent5Frame1);
        expect(m.skeleton).toBe(skeletonAgent5Frame1);
      }
    }
    pool.dispose();

    // 3. N6: changing morphologyKey on reused agent_id snaps immediately so morphologySegments matches morphologyKey.
    const n6Buf = new TickInterpolationBuffer();
    n6Buf.push(
      {
        segments: makeSyntheticPayloadSegments(1, 'prey', 0, 0, 3),
        environmental_state: { elements: [] },
        head_directions: {},
      },
      0,
      1200,
      () => 0,
      CREATURE_BODY_SCALE,
    );
    const taller = makeSyntheticPayloadSegments(1, 'prey', 0.2, 0, 3).map((s) => ({
      ...s,
      length: 2.2,
    }));
    n6Buf.push(
      { segments: taller, environmental_state: { elements: [] }, head_directions: {} },
      33,
      1200,
      () => 0,
      CREATURE_BODY_SCALE,
    );
    const midView = n6Buf.sample(49.5, 1200, () => 0);
    expect(midView.agents[0].morphologySegments[0].renderLength).toBeCloseTo(
      2.2 * 6 * CREATURE_BODY_SCALE,
      4,
    );
  });

  it('D1: places decorative forward-chain eyes and snout at the identical world point in near and mid tiers', () => {
    const buildForwardChain = (
      agentId: number,
      agentType: 'prey' | 'predator',
      simX: number,
      simZ: number,
      yaw: number,
    ): SegmentState[] => {
      const q = new THREE.Quaternion().setFromEuler(new THREE.Euler(0, yaw, 0, 'YXZ'));
      const step = new THREE.Vector3(0, 0, 1.0).applyQuaternion(q);
      const segs: SegmentState[] = [];
      for (let s = 0; s < 3; s++) {
        segs.push({
          agent_id: agentId,
          segment_id: s,
          parent_segment_id: s === 0 ? null : s - 1,
          length: 1.0,
          radius: 0.25,
          x: simX + step.x * s,
          y: 1.0,
          z: simZ + step.z * s,
          yaw,
          pitch: 0,
          roll: 0,
          joint_anchor_x: 0,
          joint_anchor_y: 0,
          joint_anchor_z: s === 0 ? 0 : 0.5,
          joint_axis_x: 0,
          joint_axis_y: 1,
          joint_axis_z: 0,
          energy: 90,
          hydration: 80,
          head_direction: [step.x, step.y, step.z],
          agent_type: agentType,
        });
      }
      return segs;
    };

    const manual = makeManualSource();
    const { container, unmount } = render(
      <LiveAgents
        world={world}
        renderSize={1200}
        heightRatio={0.14}
        meshResolution={64}
        tickSource={manual.source}
        qualityTier="high"
      />,
    );

    const preySegs = buildForwardChain(1, 'prey', -2, 0, 0.45);
    const predSegs = buildForwardChain(2, 'predator', 2, 0, -0.35);
    act(() => {
      manual.emit({
        segments: [...preySegs, ...predSegs],
        environmental_state: { elements: [] },
        head_directions: {},
      });
    });

    const gY = surfaceHeight(world, 0, 0, 1200, 0.14, 64);
    const sphereMesh = container.querySelector('instancedmesh[name="live-agents-spheres"]') as MockInstancedEl;
    const snoutMesh = container.querySelector('instancedmesh[name="live-agents-snouts"]') as MockInstancedEl;
    const segMesh = container.querySelector('instancedmesh[name="live-agents-segments"]') as MockInstancedEl;
    const skinnedGroup = container.querySelector('group[name="live-agents-skinned"]') as MockSkinnedGroupEl;

    // 1. Frame 1: Near tier (dist ~ 45 <= 160).
    const nearCam = makeCameraAt([0, gY + 25, 40], [0, gY, 0]);
    act(() => {
      advanceThreeFrame({ camera: nearCam, size: { width: 1280, height: 720 } });
    });
    const visibleNear = (skinnedGroup._b1Children3D ?? []).filter(
      (obj): obj is THREE.SkinnedMesh => obj instanceof THREE.SkinnedMesh && obj.visible,
    );
    expect(visibleNear).toHaveLength(2);
    expect(segMesh.count).toBe(0);
    expect(snoutMesh.count).toBe(1);
    expect(sphereMesh.count).toBe(4); // 2 eyes * 2 agents

    const readPos = (draws: number[][], idx: number) =>
      new THREE.Vector3(draws[idx][12], draws[idx][13], draws[idx][14]);

    // Near draw order depends on closest-first sort, so match by agentId encoded on sphereAgentIds/X-side or test deterministic agent positions.
    // Agent 1 is at simX = -2 (renderX = -12) and Agent 2 is at simX = +2 (renderX = +12).
    const nearEyesAll = [0, 1, 2, 3].map((idx) => readPos(sphereMesh._b1Draws!, idx));
    const nearPreyEyes = nearEyesAll.filter((p) => p.x < 0);
    const nearPredEyes = nearEyesAll.filter((p) => p.x > 0);
    expect(nearPreyEyes).toHaveLength(2);
    expect(nearPredEyes).toHaveLength(2);
    const nearSnout = readPos(snoutMesh._b1Draws!, 0);

    // Eyes are genuinely anchored at the leading +L/2 nose segment, well ahead of root (0).
    const root1 = new THREE.Vector3(-12, nearPreyEyes[0].y, 0);
    expect(nearPreyEyes[0].distanceTo(root1)).toBeGreaterThan(2.5);

    // 2. Frame 2: Mid tier on the exact same pose (dist = hypot(140, 220) ~= 260.8 in (160, 460]).
    const midCam = makeCameraAt([0, gY + 140, 220], [0, gY, 0]);
    act(() => {
      advanceThreeFrame({ camera: midCam, size: { width: 1280, height: 720 } });
    });
    const visibleAfterMid = (skinnedGroup._b1Children3D ?? []).filter(
      (obj): obj is THREE.SkinnedMesh => obj instanceof THREE.SkinnedMesh && obj.visible,
    );
    expect(visibleAfterMid).toHaveLength(0);
    expect(segMesh.count).toBe(6);
    expect(snoutMesh.count).toBe(1);
    // Mid draws per prey (2 joints + 1 head dome + 2 eyes = 5) and per predator (2 joints + 2 eyes = 4) => 9 spheres.
    expect(sphereMesh.count).toBe(9);

    const midPreyEye0 = readPos(sphereMesh._b1Draws!, 3);
    const midPreyEye1 = readPos(sphereMesh._b1Draws!, 4);
    const midPredEye0 = readPos(sphereMesh._b1Draws!, 7);
    const midPredEye1 = readPos(sphereMesh._b1Draws!, 8);
    const midSnout = readPos(snoutMesh._b1Draws!, 0);

    const matchClosest = (target: THREE.Vector3, pair: THREE.Vector3[]) =>
      Math.min(target.distanceTo(pair[0]), target.distanceTo(pair[1]));
    expect(matchClosest(midPreyEye0, nearPreyEyes)).toBeLessThan(1e-4);
    expect(matchClosest(midPreyEye1, nearPreyEyes)).toBeLessThan(1e-4);
    expect(matchClosest(midPredEye0, nearPredEyes)).toBeLessThan(1e-4);
    expect(matchClosest(midPredEye1, nearPredEyes)).toBeLessThan(1e-4);
    expect(midSnout.distanceTo(nearSnout)).toBeLessThan(1e-4);

    unmount();
  });

  it("N3': releases hidden slot geometry and rebuilds cleanly after LRU cache eviction", () => {
    const cache = new ArchetypeMeshCache(8);
    const pool = new SkinnedCreaturePool(3, cache);
    const group = new THREE.Group();
    pool.bindGroup(group);

    const buf = new TickInterpolationBuffer();
    const pushAgents = (
      items: Array<{ id: number; length: number; radius: number }>,
      timeMs: number,
    ) => {
      const segs: SegmentState[] = [];
      for (const item of items) {
        segs.push(
          ...makeSyntheticPayloadSegments(item.id, 'prey', 0, 0, 2).map((s) => ({
            ...s,
            length: item.length,
            radius: item.radius,
          })),
        );
      }
      buf.push(
        { segments: segs, environmental_state: { elements: [] }, head_directions: {} },
        timeMs,
        1200,
        () => 0,
        CREATURE_BODY_SCALE,
      );
      return buf.sampleLatest();
    };

    const scratch = {
      pos: new THREE.Vector3(),
      quat: new THREE.Quaternion(),
      scale: new THREE.Vector3(),
      color: new THREE.Color(),
    };

    // Frame 1: Bind Agent 1 (morph A) in slot 0 and Agent 2 (morph B) in slot 1.
    const f1 = pushAgents(
      [
        { id: 1, length: 1.0, radius: 0.2 },
        { id: 2, length: 1.4, radius: 0.24 },
      ],
      0,
    );
    const keyA = f1.agents[0].morphologyKey;
    const slots1 = pool.prepareNearFrame(f1.agents, new Int32Array([0, 1]), 2);
    expect(pool.updateSlot(slots1[0], f1.agents[0], 0, scratch, false)).toBe(true);
    expect(pool.updateSlot(slots1[1], f1.agents[1], 0, scratch, false)).toBe(true);
    const slot0Mesh = group.children[slots1[0]] as THREE.SkinnedMesh;
    const geomA = slot0Mesh.geometry;
    const disposeSpyA = vi.spyOn(geomA, 'dispose');
    expect(cache.has(keyA)).toBe(true);

    // Frame 2: Agent 1 leaves near tier -> slot 0 hides and releases geomA/skeleton/key.
    const f2 = pushAgents([{ id: 2, length: 1.4, radius: 0.24 }], 33);
    const slots2 = pool.prepareNearFrame(f2.agents, new Int32Array([0]), 1);
    expect(pool.updateSlot(slots2[0], f2.agents[0], 0.033, scratch, false)).toBe(true);
    expect(slot0Mesh.visible).toBe(false);
    expect(slot0Mesh.userData.agentId).toBe(0);
    expect(slot0Mesh.geometry).not.toBe(geomA);

    // Frames 3..14: Overflow LRU cache (maxEntries = 8) on active slot 1 while slot 0 stays hidden.
    for (let k = 1; k <= 12; k++) {
      const fk = pushAgents([{ id: 2, length: 1.5 + k * 0.25, radius: 0.16 + k * 0.02 }], 33 + k * 33);
      const slk = pool.prepareNearFrame(fk.agents, new Int32Array([0]), 1);
      expect(pool.updateSlot(slk[0], fk.agents[0], 0.1 + k * 0.03, scratch, false)).toBe(true);
    }
    expect(disposeSpyA).toHaveBeenCalledTimes(1);
    expect(cache.has(keyA)).toBe(false);

    // Frame 15: Agent 1 (morph A) re-enters -> binds fresh non-disposed geometry and re-caches keyA.
    const fReturn = pushAgents(
      [
        { id: 1, length: 1.0, radius: 0.2 },
        { id: 2, length: 4.5, radius: 0.4 },
      ],
      600,
    );
    const slReturn = pool.prepareNearFrame(fReturn.agents, new Int32Array([0, 1]), 2);
    expect(pool.updateSlot(slReturn[0], fReturn.agents[0], 0.6, scratch, false)).toBe(true);
    const meshReturn = group.children[slReturn[0]] as THREE.SkinnedMesh;
    expect(meshReturn.visible).toBe(true);
    expect(meshReturn.geometry).not.toBe(geomA);
    expect(cache.has(keyA)).toBe(true);

    pool.dispose();
  });
});
