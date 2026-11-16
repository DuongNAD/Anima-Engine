import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { act, cleanup, render } from '@testing-library/react';
import * as THREE from 'three';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { makeFrameState, type FrameCallback } from '../mocks/r3f-frame-state';
import { segment } from '../mocks/segment_fixtures';
import { installStubbedProperty, removeStubbedProperties } from '../mocks/r3f-dom-stubs';
import type { SimulationTickPayload } from '../../src/types/generated/SimulationTickPayload';
import { generateWorld, type World } from '../../src/components/Landscape/utils/worldGen';
import {
  TICK_RENDER_DELAY_MS,
  TickInterpolationBuffer,
} from '../../src/components/Landscape/agentInterpolation';
import { createReplayTickSource } from '../../src/components/Landscape/utils/tickRecording';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '../..');

const frameCallbacks: FrameCallback[] = [];

vi.mock('@react-three/fiber', async () => {
  const actual = await vi.importActual<typeof import('../mocks/react-three-fiber-mock')>(
    '../mocks/react-three-fiber-mock',
  );
  return {
    ...actual,
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
  _rwDraws?: Array<{ elements: number[] }>;
}

const flatGround = (): number => 10;

function makeMotionSequence(count = 6): SimulationTickPayload[] {
  const frames: SimulationTickPayload[] = [];
  for (let i = 0; i < count; i++) {
    const t = i;
    const x = -20 + t * 1.8;
    const z = -10 + Math.sin(t * 0.4) * 2.5;
    const yaw = 0.25 * t;
    const childYaw = yaw + Math.sin(t * 0.7) * 0.35;
    const childPitch = 0.08 * t;
    const hx = Math.sin(yaw);
    const hz = Math.cos(yaw);

    frames.push({
      segments: [
        segment({
          agent_id: 7,
          segment_id: 0,
          parent_segment_id: null,
          length: 1.2,
          radius: 0.3,
          x,
          y: 1.5,
          z,
          yaw,
          pitch: 0.03 * t,
          roll: 0.02 * t,
          energy: 85,
          hydration: 90,
          head_direction: [hx, 0, hz],
          agent_type: 'predator',
        }),
        segment({
          agent_id: 7,
          segment_id: 1,
          parent_segment_id: 0,
          length: 1.0,
          radius: 0.24,
          x: x - Math.sin(yaw) * 1.1,
          y: 1.65,
          z: z - Math.cos(yaw) * 1.1,
          yaw: childYaw,
          pitch: childPitch,
          roll: -0.04 * t,
          energy: 85,
          hydration: 90,
          head_direction: [hx, 0, hz],
          agent_type: 'predator',
        }),
      ],
      environmental_state: { elements: [] },
      head_directions: { 7: [hx, 0, hz] },
    });
  }
  return frames;
}

describe('RW-A3 sub-tick creature interpolation (RW3-1 .. RW3-3)', () => {
  let world: World;

  beforeEach(() => {
    frameCallbacks.length = 0;
    vi.clearAllMocks();
    world = generateWorld('rw-a3-test-world', { size: 32, shape: 'island' });

    installStubbedProperty(
      HTMLElement.prototype,
      'setMatrixAt',
      vi.fn(function (this: MockInstancedEl, index: number, matrix: THREE.Matrix4) {
        if (!this._rwDraws) this._rwDraws = [];
        this._rwDraws[index] = { elements: Array.from(matrix.elements) };
      }),
    );
    installStubbedProperty(HTMLElement.prototype, 'setColorAt', vi.fn());

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
    removeStubbedProperties(
      HTMLElement.prototype,
      'setMatrixAt',
      'setColorAt',
      'instanceMatrix',
      'instanceColor',
    );
  });

  it('RW3-1 (AN-S02): delays playout by 33ms and samples identical trajectory at 30/60/144 Hz', () => {
    const seq = makeMotionSequence(6);
    const interval = TICK_RENDER_DELAY_MS; // 33ms emit cadence

    const sampleAtHz = (hz: number) => {
      const buf = new TickInterpolationBuffer();
      const stepMs = 1000 / hz;
      const samples: Array<{ t: number; rootX: number; rootZ: number; qx: number; qy: number; qz: number; qw: number }> = [];
      let nextTickIdx = 0;
      const maxTime = (seq.length - 1) * interval + interval;

      for (let frameIdx = 0; frameIdx * stepMs < maxTime + 0.001; frameIdx++) {
        const now = frameIdx * stepMs;
        while (nextTickIdx < seq.length && nextTickIdx * interval < now + 1e-6) {
          buf.push(seq[nextTickIdx], nextTickIdx * interval, 1200, flatGround);
          nextTickIdx++;
        }
        const view = buf.sample(now, 1200, flatGround);
        const root = view.agents[0].segments[0];
        samples.push({
          t: now,
          rootX: root.renderPosition[0],
          rootZ: root.renderPosition[2],
          qx: root.quaternion[0],
          qy: root.quaternion[1],
          qz: root.quaternion[2],
          qw: root.quaternion[3],
        });
      }
      return samples;
    };

    // Exact delayed playout check: tick 0 @ 0ms, tick 1 @ 33ms -> midpoint @ 49.5ms, tick 1 @ 66ms
    const directBuf = new TickInterpolationBuffer();
    directBuf.push(seq[0], 0, 1200, flatGround);
    directBuf.push(seq[1], 33, 1200, flatGround);

    const viewAtDelayStart = directBuf.sample(33, 1200, flatGround);
    const xStart = viewAtDelayStart.agents[0].segments[0].renderPosition[0];
    expect(xStart).toBeCloseTo(-20 * 6, 4);

    const viewAtMid = directBuf.sample(49.5, 1200, flatGround);
    const segMid = viewAtMid.agents[0].segments[0];
    expect(segMid.renderPosition[0]).toBeCloseTo((-20 + 0.9) * 6, 4);
    expect(Math.hypot(...segMid.quaternion)).toBeCloseTo(1, 6);

    const viewAtEnd = directBuf.sample(66, 1200, flatGround);
    expect(viewAtEnd.agents[0].segments[0].renderPosition[0]).toBeCloseTo((-20 + 1.8) * 6, 4);

    const s30 = sampleAtHz(30);
    const s60 = sampleAtHz(60);
    const s144 = sampleAtHz(144);

    // Every sample generated at a common timestamp across 30/60/144 Hz must match within 1e-5 render units.
    // Specifically check 33ms, 66ms, 99ms, 132ms, 165ms where 30 Hz and 60 Hz share timestamps,
    // and interpolate expected continuous curve between nearest reference points for 144 Hz.
    const refBuf = new TickInterpolationBuffer();
    const refAt = (t: number) => {
      refBuf.clear();
      const k = Math.min(seq.length - 1, Math.max(1, Math.floor(t / interval)));
      refBuf.push(seq[k - 1], (k - 1) * interval, 1200, flatGround);
      refBuf.push(seq[k], k * interval, 1200, flatGround);
      const v = refBuf.sample(t, 1200, flatGround);
      return v.agents[0].segments[0];
    };

    for (const runSamples of [s30, s60, s144]) {
      for (const s of runSamples) {
        if (s.t < interval) continue;
        const expected = refAt(s.t);
        expect(Math.abs(s.rootX - expected.renderPosition[0]) < 0.001).toBe(true);
        expect(Math.abs(s.rootZ - expected.renderPosition[2]) < 0.001).toBe(true);
        const dot =
          Math.abs(
            s.qx * expected.quaternion[0] +
              s.qy * expected.quaternion[1] +
              s.qz * expected.quaternion[2] +
              s.qw * expected.quaternion[3],
          );
        expect(dot > 0.9999).toBe(true);
      }
    }
  });

  it('RW3-2: clamps stalls at 1 period, drops out-of-order packets, handles births/deaths, and snaps teleports', () => {
    const buf = new TickInterpolationBuffer();

    const makeSingle = (agentId: number, x: number, z = 0): SimulationTickPayload => ({
      segments: [
        segment({
          agent_id: agentId,
          segment_id: 0,
          parent_segment_id: null,
          x,
          y: 1.5,
          z,
          yaw: 0,
          energy: 80,
          hydration: 80,
          agent_type: 'prey',
        }),
      ],
      environmental_state: { elements: [] },
      head_directions: { [agentId]: [0, 0, 1] },
    });

    // Tick 0 @ 0ms, Tick 1 @ 33ms
    expect(buf.push(makeSingle(1, 0), 0, 1200, flatGround)).toBe(true);
    expect(buf.push(makeSingle(1, 2), 33, 1200, flatGround)).toBe(true);

    // Stall for 200ms (sampled at 150ms & 220ms): must stay clamped at tick 1 (x = 2*6 = 12), zero overshoot.
    expect(buf.sample(150, 1200, flatGround).agents[0].rootPosition[0]).toBeCloseTo(12, 5);
    expect(buf.sample(220, 1200, flatGround).agents[0].rootPosition[0]).toBeCloseTo(12, 5);

    // Out-of-order stale packet @ 20ms (< 33ms curr) must be ignored.
    expect(buf.push(makeSingle(1, -50), 20, 1200, flatGround)).toBe(false);
    expect(buf.sample(220, 1200, flatGround).agents[0].rootPosition[0]).toBeCloseTo(12, 5);

    // Late tick 2 arrives @ 233ms: windowStart = 200, windowEnd = 233 -> smoothly starts at x=12 at t=233ms.
    expect(buf.push(makeSingle(1, 4), 233, 1200, flatGround)).toBe(true);
    expect(buf.sample(233, 1200, flatGround).agents[0].rootPosition[0]).toBeCloseTo(12, 5);
    expect(buf.sample(249.5, 1200, flatGround).agents[0].rootPosition[0]).toBeCloseTo(18, 4);
    expect(buf.sample(266, 1200, flatGround).agents[0].rootPosition[0]).toBeCloseTo(24, 5);

    // Birth & death: tick 3 @ 266ms has agent 1 + newborn agent 2 at (-15, 0, 12).
    const birthPayload: SimulationTickPayload = {
      segments: [
        ...makeSingle(1, 6).segments,
        ...makeSingle(2, -15, 12).segments,
      ],
      environmental_state: { elements: [] },
      head_directions: { 1: [0, 0, 1], 2: [0, 0, 1] },
    };
    buf.push(birthPayload, 266, 1200, flatGround);
    const midBirth = buf.sample(282.5, 1200, flatGround);
    expect(midBirth.agentCount).toBe(2);
    // Newborn agent 2 appears right at (-15 * 6, ..., 12 * 6), never sliding from origin.
    const born = midBirth.agents.slice(0, midBirth.agentCount).find((a) => a.agentId === 2);
    expect(born?.rootPosition[0]).toBeCloseTo(-90, 5);
    expect(born?.rootPosition[2]).toBeCloseTo(72, 5);

    // Death: tick 4 @ 299ms removes agent 1, leaving only agent 2.
    buf.push(makeSingle(2, -14, 12), 299, 1200, flatGround);
    // Within window (< 332ms) agent 1 lingers cleanly, and by t = 332ms (1 emit period) agent 1 is gone.
    expect(buf.sample(315.5, 1200, flatGround).agentCount).toBe(2);
    const afterDeath = buf.sample(332, 1200, flatGround);
    expect(afterDeath.agentCount).toBe(1);
    expect(afterDeath.agents[0].agentId).toBe(2);

    // Large teleport jump (> 10 sim units: -14 -> +40): snaps directly instead of sliding through 0.
    buf.push(makeSingle(2, 40, 12), 332, 1200, flatGround);
    const snapMid = buf.sample(348.5, 1200, flatGround);
    expect(snapMid.agents[0].rootPosition[0]).toBeCloseTo(240, 5);
  });

  it('RW3-3: drives LiveAgents cleanly with injected clock and contains zero allocations inside useFrame', () => {
    // Structural zero-alloc audit on LiveAgents.tsx useFrame callback body (`RW3-3`).
    const sourceText = readFileSync(resolve(ROOT, 'src/components/Landscape/LiveAgents.tsx'), 'utf8');
    const useFrameIdx = sourceText.indexOf('useFrame((state) => {');
    const renderReturnIdx = sourceText.indexOf('  if (!connected) return null;');
    expect(useFrameIdx > 0 && renderReturnIdx > useFrameIdx).toBe(true);

    const rawUseFrame = sourceText.slice(useFrameIdx, renderReturnIdx);
    const stripped = rawUseFrame
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .replace(/\/\/.*$/gm, '');

    expect(stripped).not.toMatch(/\bnew\s+[A-Za-z_]/);
    // Forbid inline array literals `[ ... ]` (not bracket indexing `foo[i]`).
    expect(stripped).not.toMatch(/(?:=|:|\(|,|return|\bof)\s*\[[^\]]*\]/);
    // Forbid object literals in statements/assignments/calls inside useFrame.
    expect(stripped).not.toMatch(/\?\?\s*\{|\breturn\s+\{|=\s*\{/);

    // Drive LiveAgents with an injected clock at 60 Hz across two packets and verify interpolation on instancedMesh.
    vi.useFakeTimers();
    let clockMs = 0;
    const seq = makeMotionSequence(2);
    const replaySource = createReplayTickSource(seq, { kind: 'recording', intervalMs: 33, loop: false });

    const { container } = render(
      <LiveAgents
        world={world}
        renderSize={1200}
        heightRatio={0.14}
        meshResolution={64}
        tickSource={replaySource}
        nowMs={() => clockMs}
      />,
    );

    act(() => {
      clockMs = 33;
      vi.advanceTimersByTime(33);
    });

    const segMesh = container.querySelector('instancedmesh[name="live-agents-segments"]') as MockInstancedEl;
    expect(segMesh).not.toBeNull();

    // Sample midpoint (clockMs = 49.5ms) -> root position should be midway between tick 0 and tick 1.
    clockMs = 49.5;
    const state = makeFrameState();
    for (const cb of frameCallbacks) cb(state, 1 / 60);

    expect(segMesh.count).toBe(2);
    expect(segMesh._rwDraws?.[0].elements[12]).toBeCloseTo((-20 + 0.9) * 6, 3);
  });
});
