import * as THREE from 'three';
import type { AgentType } from '../../../types/generated/AgentType';
import type { EnvironmentalElement } from '../../../types/generated/EnvironmentalElement';
import type { SegmentState } from '../../../types/generated/SegmentState';
import type { SimulationTickPayload } from '../../../types/generated/SimulationTickPayload';
import { SIM_BOUNDS_HALF } from './liveAgentTransform';

// ---------------------------------------------------------------------------------------
// SYNTHETIC tick fixture generator (RW0-2) — NOT simulation output.
//
// # Purpose and boundary
//
// This module produces deterministic, purely procedural SYNTHETIC creature payloads so the
// 3D landscape, frame-budget benchmarks, and Playwright E2E gates can render 1..5000 multi-
// segment creatures in an ordinary browser with no Tauri backend running.
//
// **NEVER present outputs of this module as simulation output or scientific evidence.**
// Every exported fixture carries `kind: 'synthetic'` and `synthetic: true`, the URL trigger
// is `?replay=synthetic`, and the UI displays `SYNTHETIC_SCREEN_LABEL` whenever it runs.
// ---------------------------------------------------------------------------------------

export const SYNTHETIC_REPLAY_PARAM = 'synthetic' as const;
export const SYNTHETIC_NOTICE =
  'SYNTHETIC FIXTURE — procedural test data, NOT simulation output' as const;
export const SYNTHETIC_SCREEN_LABEL =
  'SYNTHETIC FIXTURE (synthetic) — not simulation output' as const;

export const DEFAULT_SYNTHETIC_AGENTS = 24;
export const DEFAULT_SYNTHETIC_TICKS = 180;
export const DEFAULT_SYNTHETIC_SEED = 0x517e2026;
export const SYNTHETIC_MIN_SEGMENTS = 3;
export const SYNTHETIC_MAX_SEGMENTS = 15;
export const SYNTHETIC_TICK_INTERVAL_MS = 33;

const MAX_SYNTHETIC_AGENTS = 5000;
const MAX_SYNTHETIC_TICKS = 3600;
const TWO_PI = Math.PI * 2;

export interface SyntheticTicksOptions {
  /** Number of synthetic creatures to generate (clamped to integer [0, 5000], default 24). */
  agents?: number;
  /** Number of ticks in one closed synthetic cycle (clamped to integer [1, 3600], default 180). */
  ticks?: number;
  /** Deterministic 32-bit PRNG seed (default 0x517e2026). */
  seed?: number;
  /** Optional fixed segment count per creature in [3, 15]. Omit or null to span 3..15 across agents. */
  segmentsPerAgent?: number | null;
}

export interface NormalizedSyntheticOptions {
  agents: number;
  ticks: number;
  seed: number;
  segmentsPerAgent: number | null;
}

export interface SyntheticTickFrame {
  offsetMs: number;
  payload: SimulationTickPayload;
}

export interface SyntheticTickFixture {
  schemaVersion: 1;
  kind: typeof SYNTHETIC_REPLAY_PARAM;
  synthetic: true;
  notice: typeof SYNTHETIC_NOTICE;
  params: NormalizedSyntheticOptions & { tickIntervalMs: number };
  frames: SyntheticTickFrame[];
}

export interface SyntheticTickGenerator {
  readonly options: NormalizedSyntheticOptions;
  readonly screenLabel: string;
  tickAt(tickIndex: number): SimulationTickPayload;
}

interface SyntheticSegmentSpec {
  segmentId: number;
  parentIndex: number | null;
  parentSegmentId: number | null;
  length: number;
  radius: number;
  jointAnchor: [number, number, number];
  jointAxis: [number, number, number];
  swingAmplitude: number;
  phaseOffset: number;
  jointCycles: number;
}

interface SyntheticAgentSpec {
  agentId: number;
  agentType: AgentType;
  baseEnergy: number;
  baseHydration: number;
  centerX: number;
  centerZ: number;
  radiusX: number;
  radiusZ: number;
  baseY: number;
  bobAmplitude: number;
  orbitCycles: number;
  orbitPhase: number;
  xFrequencyMul: number;
  zFrequencyMul: number;
  segments: SyntheticSegmentSpec[];
}

/**
 * Lowland land anchors inside the shipped 2048x2048 `#161` `supercontinent` world (`[-100, 100]`
 * canonical bounds), chosen strictly on lowland terrestrial biomes (`Grassland`, `Forest`,
 * `Jungle`, `Savanna`, `Swamp`) well away from peaks, snow/glacier, rivers, lakes, beaches, and
 * ocean so synthetic `tree`/`lake`/`food` fixtures and herds never strand on snow peaks or water.
 */
export const LAND_ANCHORS: ReadonlyArray<readonly [number, number]> = [
  [-53.0, 8.0], // western lowland forest (elev 0.138)
  [-29.0, 20.0], // western lowland savanna (elev 0.164)
  [-53.0, 4.0], // western lowland jungle (elev 0.140)
  [-25.0, 20.0], // western lowland chaparral (elev 0.194)
  [-49.0, 16.0], // western lowland savanna (elev 0.143)
  [-17.0, 28.0], // western lowland steppe (elev 0.150)
];

/**
 * Deterministically generate clearly-labelled synthetic `environmental_state` elements
 * (`lake`, `tree`, `food`) near `LAND_ANCHORS` from an independent PRNG stream (`(seed ^ 0x517e0e1e)
 * >>> 0`), preserving trunk root trajectories and segment PRNG draws while chaining spine
 * `jointAnchor.z` inside Rust's `mutation.rs` `|z| <= parent.length / 2` clamp.
 */
function buildSyntheticEnvironmentalElements(seed: number): EnvironmentalElement[] {
  const envRand = mulberry32((seed ^ 0x517e0e1e) >>> 0);
  const elements: EnvironmentalElement[] = [];
  const bound = Math.min(80, SIM_BOUNDS_HALF - 16);

  const pushSpec = (
    count: number,
    type: 'lake' | 'tree' | 'food',
    minRadius: number,
    radiusSpan: number,
    minResources: number,
    resourcesSpan: number,
    spread: number,
    anchorOffset: number,
  ): void => {
    for (let i = 0; i < count; i++) {
      const anchor = LAND_ANCHORS[(i + anchorOffset) % LAND_ANCHORS.length];
      elements.push({
        type,
        x: clamp(anchor[0] + (envRand() - 0.5) * spread * 2, -bound, bound),
        y: clamp(anchor[1] + (envRand() - 0.5) * spread * 2, -bound, bound),
        radius: minRadius + envRand() * radiusSpan,
        resources: minResources + envRand() * resourcesSpan,
      });
    }
  };

  pushSpec(3, 'lake', 2.6, 1.4, 75, 25, 16, 0);
  pushSpec(8, 'tree', 1.0, 0.5, 65, 35, 18, 1);
  pushSpec(10, 'food', 0.42, 0.18, 45, 50, 14, 2);
  return elements;
}

function mulberry32(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = Math.imul(state ^ (state >>> 15), 1 | state);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function clampInt(raw: number | undefined, fallback: number, min: number, max: number): number {
  if (raw === undefined || !Number.isFinite(raw)) return fallback;
  const rounded = Math.trunc(raw);
  if (rounded < min) return min;
  if (rounded > max) return max;
  return rounded;
}

function clamp(v: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, v));
}

function parseOptionalInt(raw: string | null): number | undefined {
  if (raw === null || raw.trim() === '') return undefined;
  const parsed = Number(raw);
  return Number.isFinite(parsed) ? parsed : undefined;
}

/** Normalize synthetic generator options deterministically. */
export function normalizeSyntheticOptions(
  opts: SyntheticTicksOptions = {},
): NormalizedSyntheticOptions {
  const agents = clampInt(opts.agents, DEFAULT_SYNTHETIC_AGENTS, 0, MAX_SYNTHETIC_AGENTS);
  const ticks = clampInt(opts.ticks, DEFAULT_SYNTHETIC_TICKS, 1, MAX_SYNTHETIC_TICKS);
  const seed =
    opts.seed !== undefined && Number.isFinite(opts.seed)
      ? Math.trunc(opts.seed) >>> 0
      : DEFAULT_SYNTHETIC_SEED;
  const segmentsPerAgent =
    opts.segmentsPerAgent !== undefined &&
    opts.segmentsPerAgent !== null &&
    Number.isFinite(opts.segmentsPerAgent)
      ? clampInt(
          opts.segmentsPerAgent,
          SYNTHETIC_MIN_SEGMENTS,
          SYNTHETIC_MIN_SEGMENTS,
          SYNTHETIC_MAX_SEGMENTS,
        )
      : null;
  return { agents, ticks, seed, segmentsPerAgent };
}

/**
 * Parse `?replay=synthetic&agents=N` (plus optional `&ticks=T&seed=S&segments=K`) from a URL
 * query string. Returns `null` unless `replay=synthetic` is explicitly requested.
 */
export function parseSyntheticReplaySearch(search: string): NormalizedSyntheticOptions | null {
  const params = new URLSearchParams(search);
  if (params.get('replay') !== SYNTHETIC_REPLAY_PARAM) return null;
  return normalizeSyntheticOptions({
    agents: parseOptionalInt(params.get('agents')),
    ticks: parseOptionalInt(params.get('ticks')),
    seed: parseOptionalInt(params.get('seed')),
    segmentsPerAgent: parseOptionalInt(params.get('segments')),
  });
}

function buildAgentMorphology(
  agentIndex: number,
  segmentCount: number,
  isPredator: boolean,
  rand: () => number,
): SyntheticSegmentSpec[] {
  const specs: SyntheticSegmentSpec[] = [];
  const scale = isPredator ? 1.15 : 0.95;
  const rootLength = (0.78 + rand() * 0.42) * scale;
  const rootRadius = (0.24 + rand() * 0.14) * scale;

  specs.push({
    segmentId: 0,
    parentIndex: null,
    parentSegmentId: null,
    length: rootLength,
    radius: rootRadius,
    jointAnchor: [0, 0, 0],
    jointAxis: [0, 0, 0],
    swingAmplitude: 0,
    phaseOffset: 0,
    jointCycles: 1,
  });

  // Axial body spine (up to 6 segments) chained cleanly forward inside Rust `mutation.rs`
  // `|z| <= parent.length / 2` (`dynamics.rs`: P_child = P_parent + anchor + R_child * (0,0,+L/2)).
  const spineTarget = Math.min(segmentCount, 1 + Math.max(2, Math.ceil((segmentCount - 1) * 0.45)));
  for (let idx = 1; idx < spineTarget; idx++) {
    const parentIndex = idx - 1;
    const parent = specs[parentIndex];
    const taper = Math.max(0.42, 1 - idx * 0.12);
    const segLen = (0.58 + rand() * 0.34) * scale * (0.85 + 0.15 * taper);
    const segRad = Math.max(0.11, parent.radius * (0.76 + rand() * 0.14));
    specs.push({
      segmentId: idx,
      parentIndex,
      parentSegmentId: parent.segmentId,
      length: segLen,
      radius: segRad,
      jointAnchor: [0, 0, parent.length * 0.48],
      jointAxis: [0, 1, 0],
      swingAmplitude: 0.24 + rand() * 0.24,
      phaseOffset: agentIndex * 0.47 + idx * 0.75,
      jointCycles: 3,
    });
  }

  // Remaining segments attach as paired lateral limb chains beside/below torso (`|x|,|y| <= r`).
  for (let idx = spineTarget; idx < segmentCount; idx++) {
    const limbSlot = idx - spineTarget;
    const side = limbSlot % 2 === 0 ? 1 : -1;
    const pairIndex = Math.floor(limbSlot / 2);
    const parentIndex = pairIndex % spineTarget;
    const parent = specs[parentIndex];
    const segLen = (0.45 + rand() * 0.32) * scale;
    const segRad = Math.max(0.1, parent.radius * (0.52 + rand() * 0.12));
    specs.push({
      segmentId: idx,
      parentIndex,
      parentSegmentId: parent.segmentId,
      length: segLen,
      radius: segRad,
      jointAnchor: [side * parent.radius * 0.9, -parent.radius * 0.35, 0],
      jointAxis: [1, 0, 0],
      swingAmplitude: 0.35 + rand() * 0.25,
      phaseOffset: agentIndex * 0.37 + limbSlot * Math.PI * 0.5,
      jointCycles: 4,
    });
  }

  return specs;
}

function buildAgentSpecs(opts: NormalizedSyntheticOptions): SyntheticAgentSpec[] {
  const rand = mulberry32(opts.seed);
  const agents: SyntheticAgentSpec[] = [];
  const span = SYNTHETIC_MAX_SEGMENTS - SYNTHETIC_MIN_SEGMENTS + 1;
  const maxSafeCoord = SIM_BOUNDS_HALF - 20;

  for (let i = 0; i < opts.agents; i++) {
    const agentId = i + 1;
    // Guarantee a readable predator/prey mix for N >= 2 without using non-deterministic RNG.
    const agentType: AgentType =
      i === 0 ? 'prey' : i === 1 ? 'predator' : rand() < 0.28 ? 'predator' : 'prey';

    const segmentCount =
      opts.segmentsPerAgent !== null
        ? opts.segmentsPerAgent
        : SYNTHETIC_MIN_SEGMENTS + ((i * 5) % span);

    const anchor = LAND_ANCHORS[i % LAND_ANCHORS.length];
    const spreadJitter = i < LAND_ANCHORS.length ? 3.5 : 14.0;
    const centerX = clamp(anchor[0] + (rand() - 0.5) * spreadJitter * 2, -maxSafeCoord, maxSafeCoord);
    const centerZ = clamp(anchor[1] + (rand() - 0.5) * spreadJitter * 2, -maxSafeCoord, maxSafeCoord);
    const radiusX = 3.5 + rand() * 5.5;
    const radiusZ = 3.5 + rand() * 5.5;

    agents.push({
      agentId,
      agentType,
      baseEnergy: 42 + rand() * 56,
      baseHydration: 48 + rand() * 48,
      centerX,
      centerZ,
      radiusX,
      radiusZ,
      baseY: 1.8 + rand() * 1.2,
      bobAmplitude: 0.12 + rand() * 0.14,
      orbitCycles: 1,
      orbitPhase: rand() * TWO_PI,
      xFrequencyMul: 1,
      zFrequencyMul: i % 3 === 0 ? 2 : 1,
      segments: buildAgentMorphology(i, segmentCount, agentType === 'predator', rand),
    });
  }

  return agents;
}

/**
 * Build a stateful-cursor-free deterministic generator of synthetic ticks.
 * Calling `gen.tickAt(k)` always returns the exact same `SimulationTickPayload` for `k`.
 */
export function createSyntheticTickGenerator(
  opts: SyntheticTicksOptions = {},
): SyntheticTickGenerator {
  const options = normalizeSyntheticOptions(opts);
  const agents = buildAgentSpecs(options);
  const syntheticEnvElements = buildSyntheticEnvironmentalElements(options.seed);

  // Scratch math objects reused inside synchronous `tickAt` calls.
  const qParent = new THREE.Quaternion();
  const qChild = new THREE.Quaternion();
  const qSwing = new THREE.Quaternion();
  const axisVec = new THREE.Vector3();
  const anchorVec = new THREE.Vector3();
  const halfLenVec = new THREE.Vector3();
  const forwardVec = new THREE.Vector3();
  const euler = new THREE.Euler(0, 0, 0, 'YXZ');

  const tickAt = (tickIndex: number): SimulationTickPayload => {
    const normalizedTick = ((Math.trunc(tickIndex) % options.ticks) + options.ticks) % options.ticks;
    const theta = (normalizedTick / options.ticks) * TWO_PI;

    const segments: SegmentState[] = [];
    const headDirections: Record<number, [number, number, number]> = {};

    for (let aIdx = 0; aIdx < agents.length; aIdx++) {
      const agent = agents[aIdx];
      const phase = theta * agent.orbitCycles + agent.orbitPhase;
      const xAngle = phase * agent.xFrequencyMul;
      const zAngle = phase * agent.zFrequencyMul;

      const rootX = clamp(
        agent.centerX + agent.radiusX * Math.cos(xAngle),
        -(SIM_BOUNDS_HALF - 4),
        SIM_BOUNDS_HALF - 4,
      );
      const rootZ = clamp(
        agent.centerZ + agent.radiusZ * Math.sin(zAngle),
        -(SIM_BOUNDS_HALF - 4),
        SIM_BOUNDS_HALF - 4,
      );
      const rootY = agent.baseY + agent.bobAmplitude * Math.sin(phase * 2);

      // Velocity tangent along closed orbital curve gives consistent heading (`atan2(dx, dz)`).
      const dx = -agent.radiusX * agent.xFrequencyMul * Math.sin(xAngle);
      const dz = agent.radiusZ * agent.zFrequencyMul * Math.cos(zAngle);
      const rootYaw = Math.atan2(dx, dz);
      const rootPitch = 0.08 * Math.sin(phase * 2);
      const rootRoll = 0.05 * Math.cos(phase * 2);

      euler.set(rootPitch, rootYaw, rootRoll, 'YXZ');
      qParent.setFromEuler(euler);
      forwardVec.set(0, 0, 1).applyQuaternion(qParent).normalize();
      const headDir: [number, number, number] = [forwardVec.x, forwardVec.y, forwardVec.z];
      headDirections[agent.agentId] = headDir;

      const energy = clamp(agent.baseEnergy + 6 * Math.sin(phase), 10, 100);
      const hydration = clamp(agent.baseHydration + 5 * Math.cos(phase), 10, 100);

      const segPosX: number[] = new Array(agent.segments.length);
      const segPosY: number[] = new Array(agent.segments.length);
      const segPosZ: number[] = new Array(agent.segments.length);
      const segQuats: THREE.Quaternion[] = new Array(agent.segments.length);

      for (let sIdx = 0; sIdx < agent.segments.length; sIdx++) {
        const spec = agent.segments[sIdx];
        if (spec.parentIndex === null) {
          segPosX[sIdx] = rootX;
          segPosY[sIdx] = rootY;
          segPosZ[sIdx] = rootZ;
          segQuats[sIdx] = qParent.clone();

          segments.push({
            agent_id: agent.agentId,
            segment_id: spec.segmentId,
            parent_segment_id: null,
            length: spec.length,
            radius: spec.radius,
            x: rootX,
            y: rootY,
            z: rootZ,
            yaw: rootYaw,
            pitch: rootPitch,
            roll: rootRoll,
            joint_anchor_x: 0,
            joint_anchor_y: 0,
            joint_anchor_z: 0,
            joint_axis_x: 0,
            joint_axis_y: 0,
            joint_axis_z: 0,
            energy,
            hydration,
            head_direction: [headDir[0], headDir[1], headDir[2]],
            agent_type: agent.agentType,
          });
          continue;
        }

        const pIdx = spec.parentIndex;
        const parentQuat = segQuats[pIdx];
        const swingAngle =
          spec.swingAmplitude * Math.sin(theta * spec.jointCycles + spec.phaseOffset);

        axisVec.set(spec.jointAxis[0], spec.jointAxis[1], spec.jointAxis[2]);
        if (axisVec.lengthSq() > 0) axisVec.normalize();
        else axisVec.set(0, 1, 0);

        qSwing.setFromAxisAngle(axisVec, swingAngle);
        qChild.copy(parentQuat).multiply(qSwing);
        segQuats[sIdx] = qChild.clone();

        // Exact joint match (`src-tauri/src/physics/dynamics.rs:148-150`):
        // P_child = P_parent + R_parent * anchor_offset + R_child * (0, 0, child.length / 2)
        anchorVec
          .set(spec.jointAnchor[0], spec.jointAnchor[1], spec.jointAnchor[2])
          .applyQuaternion(parentQuat);
        halfLenVec.set(0, 0, spec.length * 0.5).applyQuaternion(qChild);

        const cx = segPosX[pIdx] + anchorVec.x + halfLenVec.x;
        const cy = segPosY[pIdx] + anchorVec.y + halfLenVec.y;
        const cz = segPosZ[pIdx] + anchorVec.z + halfLenVec.z;

        segPosX[sIdx] = cx;
        segPosY[sIdx] = cy;
        segPosZ[sIdx] = cz;

        euler.setFromQuaternion(qChild, 'YXZ');

        segments.push({
          agent_id: agent.agentId,
          segment_id: spec.segmentId,
          parent_segment_id: spec.parentSegmentId,
          length: spec.length,
          radius: spec.radius,
          x: cx,
          y: cy,
          z: cz,
          yaw: euler.y,
          pitch: euler.x,
          roll: euler.z,
          joint_anchor_x: spec.jointAnchor[0],
          joint_anchor_y: spec.jointAnchor[1],
          joint_anchor_z: spec.jointAnchor[2],
          joint_axis_x: spec.jointAxis[0],
          joint_axis_y: spec.jointAxis[1],
          joint_axis_z: spec.jointAxis[2],
          energy,
          hydration,
          head_direction: [headDir[0], headDir[1], headDir[2]],
          agent_type: agent.agentType,
        });
      }
    }

    return {
      segments,
      environmental_state: { elements: syntheticEnvElements },
      head_directions: headDirections,
    };
  };

  return {
    options,
    screenLabel: `${SYNTHETIC_SCREEN_LABEL} · ${options.agents} agents · ${syntheticEnvElements.length} synthetic env`,
    tickAt,
  };
}

/** Generate a complete deterministic synthetic fixture document ready for replay or JSON export. */
export function generateSyntheticTickFixture(
  opts: SyntheticTicksOptions = {},
): SyntheticTickFixture {
  const gen = createSyntheticTickGenerator(opts);
  const frames: SyntheticTickFrame[] = [];
  for (let t = 0; t < gen.options.ticks; t++) {
    frames.push({
      offsetMs: t * SYNTHETIC_TICK_INTERVAL_MS,
      payload: gen.tickAt(t),
    });
  }
  return {
    schemaVersion: 1,
    kind: SYNTHETIC_REPLAY_PARAM,
    synthetic: true,
    notice: SYNTHETIC_NOTICE,
    params: {
      ...gen.options,
      tickIntervalMs: SYNTHETIC_TICK_INTERVAL_MS,
    },
    frames,
  };
}

/** Serialize the deterministic synthetic fixture to JSON. Identical inputs yield identical strings. */
export function serializeSyntheticTickFixture(opts: SyntheticTicksOptions = {}): string {
  return JSON.stringify(generateSyntheticTickFixture(opts));
}
