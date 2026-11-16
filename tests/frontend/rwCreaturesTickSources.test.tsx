import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, cleanup, act, waitFor, fireEvent } from '@testing-library/react';
import * as THREE from 'three';
import type { SimulationTickPayload } from '../../src/types/generated/SimulationTickPayload';
import type { FrameCallback } from '../mocks/r3f-frame-state';
import { frameStateAt } from '../mocks/r3f-frame-state';
import { installStubbedProperty, removeStubbedProperties } from '../mocks/r3f-dom-stubs';
import { segment } from '../mocks/segment_fixtures';
import { generateWorld, type World } from '../../src/components/Landscape/utils/worldGen';
import { isInsideSimBounds } from '../../src/components/Landscape/utils/liveAgentTransform';
import {
  SYNTHETIC_MAX_SEGMENTS,
  SYNTHETIC_MIN_SEGMENTS,
  SYNTHETIC_NOTICE,
  SYNTHETIC_REPLAY_PARAM,
  createSyntheticTickGenerator,
  generateSyntheticTickFixture,
  parseSyntheticReplaySearch,
  serializeSyntheticTickFixture,
} from '../../src/components/Landscape/utils/syntheticTicks';
import {
  DEFAULT_RECORDING_WINDOW_MS,
  TICK_EMIT_INTERVAL_MS,
  TickRecorder,
  createRecordingFileTickSource,
  createReplayTickSource,
  createTauriTickSource,
  parseTickRecordingJson,
  resolveTickSourceFromSearch,
} from '../../src/components/Landscape/utils/tickRecording';

let frameCallbacks: FrameCallback[] = [];
const mockTauriListen = vi.fn();

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

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => mockTauriListen(...args),
}));

const { LiveAgents } = await import('../../src/components/Landscape/LiveAgents');

interface RecordedSegmentDraw {
  index: number;
  elements: number[];
  color?: [number, number, number];
}

interface MockInstancedElement extends HTMLElement {
  count?: number;
  instanceMatrix?: { needsUpdate: boolean };
  instanceColor?: { needsUpdate: boolean };
  _rwDraws?: RecordedSegmentDraw[];
}

function getDrawnPose(el: MockInstancedElement): {
  count: number;
  draws: Array<{ elements: number[]; color: [number, number, number] }>;
} {
  const count = el.count ?? 0;
  const raw = el._rwDraws ?? [];
  const draws = raw.slice(0, count).map((entry) => {
    const color: [number, number, number] = entry.color
      ? [entry.color[0], entry.color[1], entry.color[2]]
      : [0, 0, 0];
    return {
      elements: entry.elements.slice(),
      color,
    };
  });
  return { count, draws };
}

function advanceFrames(): void {
  const state = frameStateAt(0.5);
  for (const cb of frameCallbacks) {
    cb(state, 1 / 60);
  }
}

function makeSampleSequence(): SimulationTickPayload[] {
  const ticks: SimulationTickPayload[] = [];
  for (let t = 0; t < 4; t++) {
    const dx = t * 1.5;
    ticks.push({
      segments: [
        segment({
          agent_id: 1,
          segment_id: 0,
          parent_segment_id: null,
          length: 1.1,
          radius: 0.3,
          x: -20 + dx,
          y: 1.5,
          z: -15 + t * 0.4,
          yaw: 0.2 * t,
          energy: 90 - t * 5,
          hydration: 80,
          agent_type: 'prey',
        }),
        segment({
          agent_id: 1,
          segment_id: 1,
          parent_segment_id: 0,
          length: 0.8,
          radius: 0.2,
          x: -20.8 + dx,
          y: 1.5,
          z: -15.3 + t * 0.4,
          yaw: 0.2 * t + 0.1,
          energy: 90 - t * 5,
          hydration: 80,
          agent_type: 'prey',
        }),
        segment({
          agent_id: 2,
          segment_id: 0,
          parent_segment_id: null,
          length: 1.3,
          radius: 0.36,
          x: 12 - dx,
          y: 1.8,
          z: 8 + t * 0.6,
          yaw: -0.3 * t,
          energy: 55 + t * 4,
          hydration: 70,
          agent_type: 'predator',
        }),
        // Out-of-bounds segment must be skipped identically by both tick sources.
        segment({
          agent_id: 99,
          segment_id: 0,
          parent_segment_id: null,
          x: 150,
          y: 1.0,
          z: 0,
          agent_type: null,
        }),
      ],
      environmental_state: { elements: [] },
      head_directions: {
        1: [0, 0, 1],
        2: [0, 0, -1],
      },
    });
  }
  return ticks;
}

describe('RW-A1 tick sources, synthetic fixture, and 30s tick recorder', () => {
  let world: World;

  beforeEach(() => {
    frameCallbacks = [];
    mockTauriListen.mockReset();
    world = generateWorld('rw-a1-test-world', { size: 32, shape: 'island' });

    installStubbedProperty(
      HTMLElement.prototype,
      'setMatrixAt',
      vi.fn(function (this: MockInstancedElement, index: number, matrix: THREE.Matrix4) {
        if (!this._rwDraws) this._rwDraws = [];
        const existing = this._rwDraws[index];
        this._rwDraws[index] = {
          index,
          elements: Array.from(matrix.elements),
          color: existing?.color,
        };
      }),
    );

    installStubbedProperty(
      HTMLElement.prototype,
      'setColorAt',
      vi.fn(function (this: MockInstancedElement, index: number, color: THREE.Color) {
        if (!this._rwDraws) this._rwDraws = [];
        const existing = this._rwDraws[index] ?? { index, elements: [] };
        existing.color = [color.r, color.g, color.b];
        this._rwDraws[index] = existing;
      }),
    );

    Object.defineProperty(HTMLElement.prototype, 'instanceMatrix', {
      get(this: HTMLElement & { _rwIM?: { needsUpdate: boolean } }) {
        if (!this._rwIM) this._rwIM = { needsUpdate: false };
        return this._rwIM;
      },
      configurable: true,
    });

    Object.defineProperty(HTMLElement.prototype, 'instanceColor', {
      get(this: HTMLElement & { _rwIC?: { needsUpdate: boolean } }) {
        if (!this._rwIC) this._rwIC = { needsUpdate: false };
        return this._rwIC;
      },
      configurable: true,
    });
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
    vi.restoreAllMocks();
    window.history.replaceState(null, '', '/');
    Reflect.deleteProperty(window, '__TAURI_INTERNALS__');
    removeStubbedProperties(
      HTMLElement.prototype,
      'setMatrixAt',
      'setColorAt',
      'instanceMatrix',
      'instanceColor',
    );
    document.querySelectorAll('[data-testid^="rw-"]').forEach((node) => node.remove());
  });

  it('RW0-1: produces identical pose sequences through Tauri and Replay tick sources', async () => {
    vi.useFakeTimers();
    const sequence = makeSampleSequence();

    let tauriHandler: ((event: { payload: SimulationTickPayload }) => void) | null = null;
    const tauriSource = createTauriTickSource({
      listenImpl: async (_eventName, handler) => {
        tauriHandler = handler;
        return () => {
          tauriHandler = null;
        };
      },
    });

    const tauriPoses: Array<ReturnType<typeof getDrawnPose>> = [];
    const tauriRender = render(
      <LiveAgents
        world={world}
        renderSize={1200}
        heightRatio={0.14}
        meshResolution={64}
        tickSource={tauriSource}
      />,
    );

    await act(async () => {
      await Promise.resolve();
    });
    expect(tauriHandler).not.toBeNull();

    for (let i = 0; i < sequence.length; i++) {
      await act(async () => {
        tauriHandler?.({ payload: sequence[i] });
      });
      advanceFrames();
      const meshEl = tauriRender.container.querySelector('instancedmesh') as MockInstancedElement | null;
      expect(meshEl).not.toBeNull();
      tauriPoses.push(getDrawnPose(meshEl!));
    }

    tauriRender.unmount();
    frameCallbacks = [];

    const replaySource = createReplayTickSource(sequence, {
      kind: 'recording',
      intervalMs: 33,
      loop: false,
    });
    const replayPoses: Array<ReturnType<typeof getDrawnPose>> = [];
    const replayRender = render(
      <LiveAgents
        world={world}
        renderSize={1200}
        heightRatio={0.14}
        meshResolution={64}
        tickSource={replaySource}
      />,
    );

    advanceFrames();
    const replayMesh0 = replayRender.container.querySelector(
      'instancedmesh',
    ) as MockInstancedElement | null;
    expect(replayMesh0).not.toBeNull();
    replayPoses.push(getDrawnPose(replayMesh0!));

    for (let i = 1; i < sequence.length; i++) {
      await act(async () => {
        vi.advanceTimersByTime(33);
      });
      advanceFrames();
      const replayMesh = replayRender.container.querySelector(
        'instancedmesh',
      ) as MockInstancedElement | null;
      expect(replayMesh).not.toBeNull();
      replayPoses.push(getDrawnPose(replayMesh!));
    }

    expect(replayPoses.length).toBe(tauriPoses.length);
    for (let i = 0; i < sequence.length; i++) {
      expect(replayPoses[i].count).toBe(3);
      expect(replayPoses[i]).toEqual(tauriPoses[i]);
    }
  });

  it('RW0-2: generates deterministic synthetic fixtures with 3..15 segments, trajectories, and sinusoidal joints', () => {
    const jsonA = serializeSyntheticTickFixture({ agents: 16, ticks: 20, seed: 42 });
    const jsonB = serializeSyntheticTickFixture({ agents: 16, ticks: 20, seed: 42 });
    const jsonDifferentSeed = serializeSyntheticTickFixture({ agents: 16, ticks: 20, seed: 99 });

    expect(jsonA).toBe(jsonB);
    expect(jsonA).not.toBe(jsonDifferentSeed);

    const fixture = generateSyntheticTickFixture({ agents: 26, ticks: 30, seed: 1234 });
    expect(fixture.kind).toBe(SYNTHETIC_REPLAY_PARAM);
    expect(fixture.synthetic).toBe(true);
    expect(fixture.notice).toBe(SYNTHETIC_NOTICE);
    expect(fixture.notice.toLowerCase()).toContain('synthetic');
    expect(fixture.frames).toHaveLength(30);

    const tick0 = fixture.frames[0].payload;
    const tick15 = fixture.frames[15].payload;

    const countsByAgent = new Map<number, number>();
    const typesSeen = new Set<string>();
    const rootPosTick0 = new Map<number, [number, number, number]>();
    const rootPosTick15 = new Map<number, [number, number, number]>();
    const childAnglesTick0 = new Map<string, [number, number, number]>();
    const childAnglesTick15 = new Map<string, [number, number, number]>();

    for (const seg of tick0.segments) {
      expect(isInsideSimBounds(seg.x, seg.z)).toBe(true);
      expect(seg.length).toBeGreaterThan(0);
      expect(seg.radius).toBeGreaterThan(0);
      countsByAgent.set(seg.agent_id, (countsByAgent.get(seg.agent_id) ?? 0) + 1);
      if (seg.agent_type) typesSeen.add(seg.agent_type);
      if (seg.parent_segment_id === null) {
        rootPosTick0.set(seg.agent_id, [seg.x, seg.y, seg.z]);
      } else {
        childAnglesTick0.set(`${seg.agent_id}:${seg.segment_id}`, [seg.yaw, seg.pitch, seg.roll]);
      }
    }

    for (const seg of tick15.segments) {
      expect(isInsideSimBounds(seg.x, seg.z)).toBe(true);
      if (seg.parent_segment_id === null) {
        rootPosTick15.set(seg.agent_id, [seg.x, seg.y, seg.z]);
      } else {
        childAnglesTick15.set(`${seg.agent_id}:${seg.segment_id}`, [seg.yaw, seg.pitch, seg.roll]);
      }
    }

    expect(countsByAgent.size).toBe(26);
    expect(typesSeen.has('prey')).toBe(true);
    expect(typesSeen.has('predator')).toBe(true);

    const segmentCounts = Array.from(countsByAgent.values());
    expect(Math.min(...segmentCounts)).toBe(SYNTHETIC_MIN_SEGMENTS);
    expect(Math.max(...segmentCounts)).toBe(SYNTHETIC_MAX_SEGMENTS);
    expect(segmentCounts).toContain(3);
    expect(segmentCounts).toContain(15);

    // Every creature moves along its trajectory and swings child joints between tick 0 and tick 15.
    let movingRoots = 0;
    for (const [agentId, p0] of rootPosTick0.entries()) {
      const p15 = rootPosTick15.get(agentId)!;
      const dist = Math.hypot(p15[0] - p0[0], p15[1] - p0[1], p15[2] - p0[2]);
      if (dist > 0.1) movingRoots++;
    }
    expect(movingRoots).toBe(26);

    let swingingJoints = 0;
    for (const [key, r0] of childAnglesTick0.entries()) {
      const r15 = childAnglesTick15.get(key)!;
      const delta = Math.abs(r15[0] - r0[0]) + Math.abs(r15[1] - r0[1]) + Math.abs(r15[2] - r0[2]);
      if (delta > 0.01) swingingJoints++;
    }
    expect(swingingJoints).toBeGreaterThan(20);

    // Joint continuity check matching dynamics.rs:148-150:
    // p_parent + R_parent * anchor_offset == p_child - R_child * (0, 0, child.length / 2).
    const gen = createSyntheticTickGenerator({ agents: 4, ticks: 24, segmentsPerAgent: 15 });
    const payload = gen.tickAt(7);
    const segByKey = new Map(
      payload.segments.map((s) => [`${s.agent_id}:${s.segment_id}`, s]),
    );
    const euler = new THREE.Euler(0, 0, 0, 'YXZ');
    const qParent = new THREE.Quaternion();
    const qChild = new THREE.Quaternion();
    const anchor = new THREE.Vector3();
    const halfLen = new THREE.Vector3();

    for (const seg of payload.segments) {
      if (seg.parent_segment_id === null) continue;
      const parent = segByKey.get(`${seg.agent_id}:${seg.parent_segment_id}`)!;
      euler.set(parent.pitch, parent.yaw, parent.roll, 'YXZ');
      qParent.setFromEuler(euler);
      euler.set(seg.pitch, seg.yaw, seg.roll, 'YXZ');
      qChild.setFromEuler(euler);

      anchor
        .set(seg.joint_anchor_x, seg.joint_anchor_y, seg.joint_anchor_z)
        .applyQuaternion(qParent)
        .add(new THREE.Vector3(parent.x, parent.y, parent.z));

      halfLen.set(0, 0, seg.length * 0.5).applyQuaternion(qChild);
      const childJoint = new THREE.Vector3(seg.x, seg.y, seg.z).sub(halfLen);
      expect(anchor.distanceTo(childJoint)).toBeLessThan(1e-5);
    }
  });

  it('RW0-2: renders synthetic creatures in an ordinary browser and labels them "synthetic" on screen', () => {
    vi.useFakeTimers();
    const parsed = parseSyntheticReplaySearch('?replay=synthetic&agents=5&segments=4');
    expect(parsed).toEqual({ agents: 5, ticks: 180, seed: 0x517e2026, segmentsPerAgent: 4 });

    window.history.replaceState(null, '', '/landscape.html?replay=synthetic&agents=5&segments=4');
    const { container, unmount } = render(
      <LiveAgents world={world} renderSize={1200} heightRatio={0.14} meshResolution={64} />,
    );

    advanceFrames();
    const groupedMeshEl = container.querySelector(
      'group[name="live-agents"] instancedmesh[name="live-agents-segments"]',
    ) as MockInstancedElement | null;
    expect(groupedMeshEl).not.toBeNull();
    expect(groupedMeshEl!.count).toBe(20);

    const banner = document.querySelector('[data-testid="rw-synthetic-banner"]');
    expect(banner).not.toBeNull();
    expect(banner!.textContent?.toLowerCase()).toContain('synthetic');
    expect(banner!.textContent?.toLowerCase()).toContain('not simulation output');
    expect(banner!.textContent).toContain('5 agents');
    expect(mockTauriListen).not.toHaveBeenCalled();

    unmount();
    expect(document.querySelector('[data-testid="rw-synthetic-banner"]')).toBeNull();
  });

  it('RW0-3: maintains a 30-second rolling buffer and clones payloads on push', () => {
    const recorder = new TickRecorder(DEFAULT_RECORDING_WINDOW_MS);
    const mutablePayload: SimulationTickPayload = {
      segments: [segment({ agent_id: 1, x: 5, z: -5, energy: 88 })],
      environmental_state: { elements: [] },
      head_directions: { 1: [0, 0, 1] },
    };

    recorder.push(mutablePayload, 1000);
    mutablePayload.segments[0].x = 77;
    mutablePayload.head_directions[1]![0] = 0.9;

    const snap1 = recorder.snapshot('2026-11-16T00:00:00Z');
    expect(snap1.frames).toHaveLength(1);
    expect(snap1.frames[0].offsetMs).toBe(0);
    expect(snap1.frames[0].payload.segments[0].x).toBe(5);
    expect(snap1.frames[0].payload.head_directions[1]).toEqual([0, 0, 1]);

    // Push 33 seconds of ticks at 33 ms cadence; frames older than 30,000 ms must be evicted.
    recorder.clear();
    for (let ms = 0; ms < 33_001; ms += TICK_EMIT_INTERVAL_MS) {
      recorder.push(
        {
          segments: [segment({ agent_id: 1, x: ms / 1000, z: 0 })],
          environmental_state: { elements: [] },
          head_directions: {},
        },
        ms,
      );
    }

    expect(recorder.durationMs()).toBeLessThanOrEqual(DEFAULT_RECORDING_WINDOW_MS);
    expect(recorder.durationMs()).toBeGreaterThan(29_000);
    const snap33 = recorder.snapshot();
    expect(snap33.kind).toBe('recording');
    expect(snap33.frames[0].offsetMs).toBe(0);
    expect(snap33.frames[0].payload.segments[0].x).toBeGreaterThan(2.9);
    expect(snap33.frames[snap33.frames.length - 1].offsetMs).toBeLessThanOrEqual(
      DEFAULT_RECORDING_WINDOW_MS,
    );
  });

  it('RW0-3: ?record=1 records live ticks, exports JSON, and ?replay=<file> replays in ordinary browser', async () => {
    vi.useFakeTimers();

    // 1. Desktop app session with ?record=1 captures incoming Tauri ticks.
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      value: {},
      configurable: true,
    });
    let activeTauriCb: ((event: { payload: SimulationTickPayload }) => void) | null = null;
    mockTauriListen.mockImplementation(
      async (_name: string, cb: (event: { payload: SimulationTickPayload }) => void) => {
        activeTauriCb = cb;
        return () => {
          activeTauriCb = null;
        };
      },
    );

    const capturedUrls: string[] = [];
    const origCreateObjectUrl = URL.createObjectURL;
    const origRevokeObjectUrl = URL.revokeObjectURL;
    URL.createObjectURL = vi.fn((blob: Blob) => {
      capturedUrls.push(`blob:${blob.size}`);
      return 'blob:rw-test-recording';
    });
    URL.revokeObjectURL = vi.fn();

    window.history.replaceState(null, '', '/landscape.html?record=1');
    const recordView = render(
      <LiveAgents world={world} renderSize={1200} heightRatio={0.14} meshResolution={64} />,
    );

    await act(async () => {
      await Promise.resolve();
    });
    expect(activeTauriCb).not.toBeNull();

    const hud = document.querySelector('[data-testid="rw-tick-recorder-hud"]');
    expect(hud).not.toBeNull();

    const sampleTicks = makeSampleSequence();
    for (let i = 0; i < sampleTicks.length; i++) {
      await act(async () => {
        vi.advanceTimersByTime(33);
        activeTauriCb?.({ payload: sampleTicks[i] });
      });
    }

    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      configurable: true,
    });

    const copyBtn = document.querySelector(
      '[data-testid="rw-tick-recorder-copy"]',
    ) as HTMLButtonElement | null;
    const exportBtn = document.querySelector(
      '[data-testid="rw-tick-recorder-export"]',
    ) as HTMLButtonElement | null;
    expect(copyBtn).not.toBeNull();
    expect(exportBtn).not.toBeNull();

    await act(async () => {
      copyBtn!.click();
      exportBtn!.click();
      await Promise.resolve();
    });

    expect(writeText).toHaveBeenCalledTimes(1);
    const exportedJson = writeText.mock.calls[0][0] as string;
    const parsedRecording = parseTickRecordingJson(exportedJson);
    expect(parsedRecording.kind).toBe('recording');
    expect(parsedRecording.frames).toHaveLength(sampleTicks.length);
    expect(capturedUrls).toHaveLength(1);

    URL.createObjectURL = origCreateObjectUrl;
    URL.revokeObjectURL = origRevokeObjectUrl;
    recordView.unmount();
    Reflect.deleteProperty(window, '__TAURI_INTERNALS__');
    frameCallbacks = [];

    // 2. Ordinary browser session (no __TAURI_INTERNALS__) loads exported JSON via ?replay=<file>.
    const fetchMock = vi.fn(async () => new Response(exportedJson, { status: 200 }));
    const fileReplaySource = createRecordingFileTickSource('/recordings/run.json', fetchMock);

    const replayView = render(
      <LiveAgents
        world={world}
        renderSize={1200}
        heightRatio={0.14}
        meshResolution={64}
        tickSource={fileReplaySource}
      />,
    );

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(fetchMock).toHaveBeenCalledWith('/recordings/run.json');
    advanceFrames();
    const meshEl = replayView.container.querySelector(
      'instancedmesh',
    ) as MockInstancedElement | null;
    expect(meshEl).not.toBeNull();
    expect(meshEl!.count).toBe(3);
    const firstFrameX = getDrawnPose(meshEl!).draws[0].elements[12];

    await act(async () => {
      vi.advanceTimersByTime(40);
    });
    advanceFrames();
    const secondFrameX = getDrawnPose(meshEl!).draws[0].elements[12];
    expect(secondFrameX).not.toBe(firstFrameX);

    replayView.unmount();
  });

  it('RW0-3: preserves synthetic flag when replaying a saved synthetic JSON file and allows file-picker replay', async () => {
    const syntheticJson = serializeSyntheticTickFixture({ agents: 3, ticks: 4, seed: 7 });
    const fetchMock = vi.fn(async () => new Response(syntheticJson, { status: 200 }));
    const fileSource = createRecordingFileTickSource('/fixtures/synth.json', fetchMock);

    const { container, unmount } = render(
      <LiveAgents
        world={world}
        renderSize={1200}
        heightRatio={0.14}
        meshResolution={64}
        tickSource={fileSource}
      />,
    );

    await waitFor(() => {
      expect(document.querySelector('[data-testid="rw-synthetic-banner"]')).not.toBeNull();
    });

    advanceFrames();
    const meshEl = container.querySelector('instancedmesh') as MockInstancedElement | null;
    expect(meshEl).not.toBeNull();
    expect(meshEl!.count).toBeGreaterThan(0);

    // Uploading a recording file via the file input updates the replay source too.
    const fileInput = document.querySelector(
      '[data-testid="rw-tick-replay-file-input"]',
    ) as HTMLInputElement | null;
    expect(fileInput).not.toBeNull();

    const rec = new TickRecorder();
    rec.push(makeSampleSequence()[0], 100);
    const uploadedFile = new File([rec.toJson()], 'owner_rw_recording_20261116.json', {
      type: 'application/json',
    });
    await act(async () => {
      fireEvent.change(fileInput!, { target: { files: [uploadedFile] } });
      await Promise.resolve();
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(document.querySelector('[data-testid="rw-tick-replay-hud"]')).not.toBeNull();
    });

    unmount();
  });

  it('keeps canonical capture mode (?capture=1 without replay=) free of creature draws and overlays', () => {
    expect(resolveTickSourceFromSearch('?capture=1&view=overview')).toBeNull();
    window.history.replaceState(null, '', '/landscape.html?capture=1&view=overview');

    const { container } = render(
      <LiveAgents world={world} renderSize={1200} heightRatio={0.14} meshResolution={64} />,
    );
    expect(container.querySelector('instancedmesh')).toBeNull();
    expect(document.querySelector('[data-testid^="rw-"]')).toBeNull();
    expect(mockTauriListen).not.toHaveBeenCalled();
  });

  it('rejects malformed recording JSON payloads clearly', () => {
    expect(() => parseTickRecordingJson('not-json')).toThrow(/invalid JSON/i);
    expect(() => parseTickRecordingJson('{"frames":[{"offsetMs":-1,"payload":{}}]}')).toThrow(
      /offsetMs/i,
    );
    expect(() =>
      parseTickRecordingJson(
        JSON.stringify([{ segments: [{ x: 'bad' }], environmental_state: { elements: [] } }]),
      ),
    ).toThrow(/invalid numeric field/i);
  });
});
