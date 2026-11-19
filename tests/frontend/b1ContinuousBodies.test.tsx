import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, cleanup, act } from '@testing-library/react';
import * as THREE from 'three';
import type { SimulationTickPayload } from '../../src/types/generated/SimulationTickPayload';
import type { SegmentState } from '../../src/types/generated/SegmentState';
import type { FrameCallback } from '../mocks/r3f-frame-state';
import { makeFrameState } from '../mocks/r3f-frame-state';
import { installStubbedProperty, removeStubbedProperties } from '../mocks/r3f-dom-stubs';
import { generateWorld, type World } from '../../src/components/Landscape/utils/worldGen';
import {
  CREATURE_BODY_SCALE,
  buildAgentPoses,
  computeCreatureColor,
} from '../../src/components/Landscape/utils/agentPose';
import {
  ArchetypeMeshCache,
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
}

interface MockSkinnedGroupEl extends HTMLElement {
  isObject3D?: boolean;
  _b1Children3D?: THREE.Object3D[];
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
    installStubbedProperty(HTMLElement.prototype, 'setMatrixAt', vi.fn());
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

    // Verify normalized skinWeights (`sum ~= 1.0`) and valid skinIndex bounds.
    const skinW = arch3a.geometry.getAttribute('skinWeight') as THREE.BufferAttribute;
    const skinI = arch3a.geometry.getAttribute('skinIndex') as THREE.BufferAttribute;
    for (let i = 0; i < posAttr.count; i++) {
      const sum = skinW.getX(i) + skinW.getY(i) + skinW.getZ(i) + skinW.getW(i);
      expect(sum).toBeCloseTo(1.0, 4);
      expect(skinI.getX(i)).toBeLessThan(3);
    }

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

  it('enforces 3-tier LOD caps K=8/14/24 and frustum culls behind positioned cameras', () => {
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
});
