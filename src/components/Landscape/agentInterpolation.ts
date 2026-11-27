import type { AgentType } from '../../types/generated/AgentType';
import type { SimulationTickPayload } from '../../types/generated/SimulationTickPayload';
import {
  buildAgentPosesInto,
  copy3,
  createAgentPosePool,
  createAgentPoseScratch,
  set3,
  set4,
  type AgentPoseScratch,
  type AgentRenderPose,
  type GroundHeightSampler,
  type SegmentRenderPose,
} from './utils/agentPose';
import { simScale } from './utils/liveAgentTransform';

// N1-1 / RW3-1..RW3-3 — sub-tick buffered creature interpolation (living-world & readable-world).
//
// Renderer runs at display refresh rates (30/60/144 Hz) while simulation emits ~30 Hz (`~33 ms`).
// Keeping two snapshots (prev, curr) with one emit period (`TICK_RENDER_DELAY_MS = 33 ms`) of
// buffered playout latency lets every frame sample `(nowMs - 33 ms)` within `[windowStartMs,
// windowEndMs]` using linear position interpolation, shortest-arc quaternion slerp, and pre-indexed
// `(agent_id, segment_id)` lookup maps built once per incoming packet with zero per-frame
// allocations inside `useFrame`.

export const TICK_RENDER_DELAY_MS = 33;
export const DEFAULT_SNAP_DISTANCE_SIM = 10;

export interface Vec3Like {
  x: number;
  y: number;
  z: number;
}

/** Clamp interpolation `alpha` to `[0, 1]` — never extrapolating beyond 1 tick (N1-2 / RW3-2). */
export function clamp01(alpha: number): number {
  if (!Number.isFinite(alpha)) return 1;
  return Math.min(1, Math.max(0, alpha));
}

/** Pure linear interpolation between two Vec3Like values (`alpha` clamped in `[0, 1]`). */
export function lerpVec3<T extends Vec3Like>(a: T, b: T, alpha: number): Vec3Like {
  const t = clamp01(alpha);
  return {
    x: a.x + (b.x - a.x) * t,
    y: a.y + (b.y - a.y) * t,
    z: a.z + (b.z - a.z) * t,
  };
}

/** In-place linear interpolation writing into `out` with zero heap allocations (`RW3-3`). */
export function lerpVec3Into<T extends Vec3Like>(
  a: Vec3Like,
  b: Vec3Like,
  alpha: number,
  out: T,
): T {
  const t = clamp01(alpha);
  out.x = a.x + (b.x - a.x) * t;
  out.y = a.y + (b.y - a.y) * t;
  out.z = a.z + (b.z - a.z) * t;
  return out;
}

/** Shortest-arc degree angle interpolation in `[0, 360)` (`alpha` clamped in `[0, 1]`). */
export function lerpAngle(fromDeg: number, toDeg: number, alpha: number): number {
  const t = clamp01(alpha);
  let delta = (toDeg - fromDeg) % 360;
  if (delta > 180) delta -= 360;
  if (delta <= -180) delta += 360;
  return (((fromDeg + delta * t) % 360) + 360) % 360;
}

function lerpNum(a: number, b: number, t: number): number {
  return a + (b - a) * t;
}

/** In-place 3-tuple linear interpolation (`alpha` clamped in `[0, 1]`, zero allocations). */
export function lerpTuple3Into(
  a: readonly [number, number, number],
  b: readonly [number, number, number],
  alpha: number,
  out: [number, number, number],
): [number, number, number] {
  const t = clamp01(alpha);
  set3(out, lerpNum(a[0], b[0], t), lerpNum(a[1], b[1], t), lerpNum(a[2], b[2], t));
  return out;
}

/**
 * In-place shortest-arc unit quaternion slerp on `[x, y, z, w]` tuples (`RW3-1`, `RW3-3`).
 */
export function slerpQuatTupleInto(
  a: readonly [number, number, number, number],
  b: readonly [number, number, number, number],
  alpha: number,
  out: [number, number, number, number],
): [number, number, number, number] {
  const t = clamp01(alpha);
  const ax = a[0];
  const ay = a[1];
  const az = a[2];
  const aw = a[3];
  let bx = b[0];
  let by = b[1];
  let bz = b[2];
  let bw = b[3];

  let dot = ax * bx + ay * by + az * bz + aw * bw;
  if (dot < 0) {
    dot = -dot;
    bx = -bx;
    by = -by;
    bz = -bz;
    bw = -bw;
  }

  let ox: number;
  let oy: number;
  let oz: number;
  let ow: number;
  if (dot > 0.9995) {
    ox = ax + t * (bx - ax);
    oy = ay + t * (by - ay);
    oz = az + t * (bz - az);
    ow = aw + t * (bw - aw);
  } else {
    const theta0 = Math.acos(Math.min(1, dot));
    const sinTheta0 = Math.sin(theta0);
    const theta = theta0 * t;
    const sinTheta = Math.sin(theta);
    const s1 = sinTheta / sinTheta0;
    const s0 = Math.cos(theta) - dot * s1;
    ox = s0 * ax + s1 * bx;
    oy = s0 * ay + s1 * by;
    oz = s0 * az + s1 * bz;
    ow = s0 * aw + s1 * bw;
  }

  const lenSq = ox * ox + oy * oy + oz * oz + ow * ow;
  if (lenSq > 1e-12) {
    const inv = 1 / Math.sqrt(lenSq);
    set4(out, ox * inv, oy * inv, oz * inv, ow * inv);
  } else {
    set4(out, 0, 0, 0, 1);
  }
  return out;
}

/**
 * In-place unit direction blend for head vectors (`RW3-1`, `RW3-3`), falling back cleanly when nearly
 * collinear/antipodal.
 */
export function slerpDirection3Into(
  a: readonly [number, number, number],
  b: readonly [number, number, number],
  alpha: number,
  out: [number, number, number],
): [number, number, number] {
  const t = clamp01(alpha);
  const alen = Math.hypot(a[0], a[1], a[2]);
  const blen = Math.hypot(b[0], b[1], b[2]);
  if (alen <= 1e-8 || blen <= 1e-8) {
    if (blen > 1e-8) {
      set3(out, b[0] / blen, b[1] / blen, b[2] / blen);
    } else if (alen > 1e-8) {
      set3(out, a[0] / alen, a[1] / alen, a[2] / alen);
    } else {
      set3(out, 0, 0, 1);
    }
    return out;
  }

  const ax = a[0] / alen;
  const ay = a[1] / alen;
  const az = a[2] / alen;
  const bx = b[0] / blen;
  const by = b[1] / blen;
  const bz = b[2] / blen;
  const dot = Math.max(-1, Math.min(1, ax * bx + ay * by + az * bz));

  if (dot > 0.9995 || dot < -0.9995) {
    // Almost parallel or exact opposite: choose deterministic lerp (or perp nudge when antipodal).
    let lx = ax + t * (bx - ax);
    let ly = ay + t * (by - ay);
    let lz = az + t * (bz - az);
    let len = Math.hypot(lx, ly, lz);
    if (len <= 1e-6) {
      // Orthogonal fallback in XZ plane so 180-degree flips stay normalized mid-slerp.
      lx = -az;
      ly = 0;
      lz = ax;
      len = Math.hypot(lx, ly, lz);
      if (len <= 1e-6) {
        lx = 1;
        ly = 0;
        lz = 0;
        len = 1;
      }
    }
    set3(out, lx / len, ly / len, lz / len);
    return out;
  }

  const theta = Math.acos(dot);
  const sinTheta = Math.sin(theta);
  const s0 = Math.sin((1 - t) * theta) / sinTheta;
  const s1 = Math.sin(t * theta) / sinTheta;
  const ox = s0 * ax + s1 * bx;
  const oy = s0 * ay + s1 * by;
  const oz = s0 * az + s1 * bz;
  const len = Math.hypot(ox, oy, oz);
  if (len > 1e-8) {
    set3(out, ox / len, oy / len, oz / len);
  } else {
    set3(out, bx, by, bz);
  }
  return out;
}

/**
 * Collision-free numeric key for `(agent_id, segment_id)` lookup without string allocation (`RW3-3`).
 */
export function packAgentSegmentKey(agentId: number, segmentId: number): number {
  return Math.trunc(agentId) * 65536 + (Math.trunc(segmentId) & 0xffff);
}

const EMPTY_POSES: readonly AgentRenderPose[] = [];
const EMPTY_TICK_PAYLOAD: SimulationTickPayload = {
  segments: [],
  environmental_state: { elements: [] },
  head_directions: {},
};

export interface PreparedPoseSnapshot {
  receivedAtMs: number;
  payload: SimulationTickPayload;
  poses: AgentRenderPose[];
  agentById: Map<number, AgentRenderPose>;
  segmentByKey: Map<number, SegmentRenderPose>;
}

function copy4(
  src: readonly [number, number, number, number],
  dst: [number, number, number, number],
): void {
  dst[0] = src[0];
  dst[1] = src[1];
  dst[2] = src[2];
  dst[3] = src[3];
}

export interface PoseSnapshotSlot {
  readonly snapshot: PreparedPoseSnapshot;
  fill(
    payload: SimulationTickPayload,
    receivedAtMs: number,
    renderSize: number,
    sampleGroundY: GroundHeightSampler,
    bodyScaleFactor: number,
    scratch: AgentPoseScratch,
  ): PreparedPoseSnapshot;
}

export function createPoseSnapshotSlot(): PoseSnapshotSlot {
  const agentById = new Map<number, AgentRenderPose>();
  const segmentByKey = new Map<number, SegmentRenderPose>();
  const snapshot: PreparedPoseSnapshot = {
    receivedAtMs: 0,
    payload: EMPTY_TICK_PAYLOAD,
    poses: [],
    agentById,
    segmentByKey,
  };
  const pool = createAgentPosePool();

  return {
    snapshot,
    fill(payload, receivedAtMs, renderSize, sampleGroundY, bodyScaleFactor, scratch) {
      snapshot.receivedAtMs = receivedAtMs;
      snapshot.payload = payload;
      const poses = buildAgentPosesInto(
        payload.segments,
        renderSize,
        sampleGroundY,
        snapshot.poses,
        pool,
        scratch,
        payload.head_directions,
        bodyScaleFactor,
      );

      for (let pass = 0; pass < 2; pass++) {
        let totalSegs = 0;
        for (let i = 0; i < poses.length; i++) {
          const pose = poses[i];
          agentById.set(pose.agentId, pose);
          for (let s = 0; s < pose.segments.length; s++) {
            const seg = pose.segments[s];
            segmentByKey.set(packAgentSegmentKey(pose.agentId, seg.segmentId), seg);
            totalSegs++;
          }
        }
        if (agentById.size === poses.length && segmentByKey.size === totalSegs) break;
        if (pass === 0) {
          agentById.clear();
          segmentByKey.clear();
        }
      }

      return snapshot;
    },
  };
}

/**
 * Pooled snapshot preparation (`FE-MEM-2`): writes poses and `(agent_id, segment_id)` lookup
 * maps into `slot` reusing existing arrays/objects/Maps with zero steady-state heap allocations.
 */
export function preparePoseSnapshotInto(
  slot: PoseSnapshotSlot,
  payload: SimulationTickPayload,
  receivedAtMs: number,
  renderSize: number,
  sampleGroundY: GroundHeightSampler,
  bodyScaleFactor = 1,
  scratch = createAgentPoseScratch(),
): PreparedPoseSnapshot {
  return slot.fill(
    payload,
    receivedAtMs,
    renderSize,
    sampleGroundY,
    bodyScaleFactor,
    scratch,
  );
}

/**
 * Build grounded creature poses and `(agent_id, segment_id)` lookup maps once per arrived packet
 * (`RW3-3`). Never call inside `useFrame`.
 */
export function preparePoseSnapshot(
  payload: SimulationTickPayload,
  receivedAtMs: number,
  renderSize: number,
  sampleGroundY: GroundHeightSampler,
  bodyScaleFactor = 1,
): PreparedPoseSnapshot {
  return preparePoseSnapshotInto(
    createPoseSnapshotSlot(),
    payload,
    receivedAtMs,
    renderSize,
    sampleGroundY,
    bodyScaleFactor,
  );
}

export interface InterpolatedSegmentView {
  segmentId: number;
  parentSegmentId: number | null;
  isRoot: boolean;
  renderPosition: [number, number, number];
  quaternion: [number, number, number, number];
  renderLength: number;
  renderRadius: number;
  jointAnchor: [number, number, number];
  hasJoint: boolean;
  jointRenderPosition: [number, number, number];
  jointRenderRadius: number;
}

export interface InterpolatedAgentView {
  agentId: number;
  agentType: AgentType | null;
  energy: number;
  hydration: number;
  segmentCount: number;
  groundY: number;
  rootPosition: [number, number, number];
  rootRenderLength: number;
  rootRenderRadius: number;
  boundingRadius: number;
  morphologyKey: string;
  /** Snapshot segment dimensions paired with `morphologyKey` so archetype builds match the key. */
  morphologySegments: readonly SegmentRenderPose[];
  headSegmentId: number;
  headLocalForward: [number, number, number];
  headDirection: [number, number, number];
  segments: InterpolatedSegmentView[];
}

export interface InterpolatedFrameView {
  agentCount: number;
  totalSegments: number;
  alpha: number;
  agents: InterpolatedAgentView[];
}

function createEmptySegmentView(): InterpolatedSegmentView {
  return {
    segmentId: 0,
    parentSegmentId: null,
    isRoot: false,
    renderPosition: [0, 0, 0],
    quaternion: [0, 0, 0, 1],
    renderLength: 0,
    renderRadius: 0,
    jointAnchor: [0, 0, 0],
    hasJoint: false,
    jointRenderPosition: [0, 0, 0],
    jointRenderRadius: 0,
  };
}

function createEmptyAgentView(): InterpolatedAgentView {
  return {
    agentId: 0,
    agentType: null,
    energy: 0,
    hydration: 0,
    segmentCount: 0,
    groundY: 0,
    rootPosition: [0, 0, 0],
    rootRenderLength: 0,
    rootRenderRadius: 0,
    boundingRadius: 0,
    morphologyKey: '',
    morphologySegments: [],
    headSegmentId: 0,
    headLocalForward: [0, 0, 1],
    headDirection: [0, 0, 1],
    segments: [],
  };
}

function copySegmentInto(src: SegmentRenderPose, dst: InterpolatedSegmentView): void {
  dst.segmentId = src.segmentId;
  dst.parentSegmentId = src.parentSegmentId;
  dst.isRoot = src.isRoot;
  copy3(src.renderPosition, dst.renderPosition);
  copy4(src.quaternion, dst.quaternion);
  dst.renderLength = src.renderLength;
  dst.renderRadius = src.renderRadius;
  copy3(src.jointAnchor, dst.jointAnchor);
  if (src.jointRenderPosition) {
    dst.hasJoint = true;
    copy3(src.jointRenderPosition, dst.jointRenderPosition);
    dst.jointRenderRadius = src.jointRenderRadius;
  } else {
    dst.hasJoint = false;
    dst.jointRenderRadius = 0;
  }
}

function copyAgentInto(src: AgentRenderPose, dst: InterpolatedAgentView): number {
  dst.agentId = src.agentId;
  dst.agentType = src.agentType;
  dst.energy = src.energy;
  dst.hydration = src.hydration;
  dst.segmentCount = src.segmentCount;
  dst.groundY = src.groundY;
  copy3(src.rootPosition, dst.rootPosition);
  dst.rootRenderLength = src.rootRenderLength;
  dst.rootRenderRadius = src.rootRenderRadius;
  dst.boundingRadius = src.boundingRadius;
  dst.morphologyKey = src.morphologyKey;
  dst.morphologySegments = src.segments;
  dst.headSegmentId = src.headSegmentId;
  copy3(src.headLocalForward, dst.headLocalForward);
  copy3(src.headDirection, dst.headDirection);
  const n = src.segments.length;
  for (let i = 0; i < n; i++) {
    copySegmentInto(src.segments[i], dst.segments[i]);
  }
  return n;
}

/** Compute world-space rear joint position (`segPos - quat * (0, 0, length / 2)`) in place. */
function writeRearJointPositionInto(dst: InterpolatedSegmentView): void {
  const halfLen = dst.renderLength * 0.5;
  const qx = dst.quaternion[0];
  const qy = dst.quaternion[1];
  const qz = dst.quaternion[2];
  const qw = dst.quaternion[3];
  set3(
    dst.jointRenderPosition,
    dst.renderPosition[0] - halfLen * 2 * (qx * qz + qw * qy),
    dst.renderPosition[1] - halfLen * 2 * (qy * qz - qw * qx),
    dst.renderPosition[2] - halfLen * (1 - 2 * (qx * qx + qy * qy)),
  );
}

function maxSegLen(arr: readonly AgentRenderPose[]): number {
  let m = 0;
  for (let i = 0; i < arr.length; i++) if (arr[i].segments.length > m) m = arr[i].segments.length;
  return m;
}

export interface TickInterpolationBufferOptions {
  renderDelayMs?: number;
  snapDistanceSim?: number;
}

/**
 * Two-snapshot playout buffer (`RW3-1`, `RW3-2`, `RW3-3`) maintaining `renderDelayMs` (`33 ms`) of
 * buffered latency and zero per-frame allocations inside `sample()` / `sampleLatest()`.
 */
export class TickInterpolationBuffer {
  readonly renderDelayMs: number;
  readonly snapDistanceSim: number;

  private readonly slotA: PoseSnapshotSlot = createPoseSnapshotSlot();
  private readonly slotB: PoseSnapshotSlot = createPoseSnapshotSlot();
  private readonly poseScratch: AgentPoseScratch = createAgentPoseScratch();
  private prevSlot: PoseSnapshotSlot | null = null;
  private currSlot: PoseSnapshotSlot | null = null;
  private windowStartMs = 0;
  private windowEndMs = 0;
  private lastBodyScaleFactor = 1;

  private readonly view: InterpolatedFrameView = {
    agentCount: 0,
    totalSegments: 0,
    alpha: 1,
    agents: [],
  };

  constructor(options: TickInterpolationBufferOptions = {}) {
    this.renderDelayMs =
      typeof options.renderDelayMs === 'number' &&
      Number.isFinite(options.renderDelayMs) &&
      options.renderDelayMs >= 0
        ? options.renderDelayMs
        : TICK_RENDER_DELAY_MS;
    this.snapDistanceSim =
      typeof options.snapDistanceSim === 'number' &&
      Number.isFinite(options.snapDistanceSim) &&
      options.snapDistanceSim > 0
        ? options.snapDistanceSim
        : DEFAULT_SNAP_DISTANCE_SIM;
  }

  hasSnapshots(): boolean {
    return this.currSlot !== null;
  }

  hasActiveAgents(): boolean {
    return (this.currSlot !== null && this.currSlot.snapshot.poses.length > 0) ||
      (this.prevSlot !== null && this.prevSlot.snapshot.poses.length > 0);
  }

  private resetView(): InterpolatedFrameView {
    const view = this.view;
    view.agentCount = 0;
    view.totalSegments = 0;
    view.alpha = 1;
    return view;
  }

  clear(): void {
    this.prevSlot = null;
    this.currSlot = null;
    this.slotA.snapshot.payload = EMPTY_TICK_PAYLOAD;
    this.slotB.snapshot.payload = EMPTY_TICK_PAYLOAD;
    this.windowStartMs = 0;
    this.windowEndMs = 0;
    this.resetView();
  }

  private ensurePoolCapacity(): void {
    const prevAgents = this.prevSlot !== null ? this.prevSlot.snapshot.poses : EMPTY_POSES;
    const currAgents = this.currSlot !== null ? this.currSlot.snapshot.poses : EMPTY_POSES;
    const maxAgentSlots = prevAgents.length + currAgents.length;
    const maxSegs = Math.max(maxSegLen(prevAgents), maxSegLen(currAgents));

    const pool = this.view.agents;
    while (pool.length < maxAgentSlots) {
      pool.push(createEmptyAgentView());
    }
    for (let i = 0; i < pool.length; i++) {
      const segs = pool[i].segments;
      while (segs.length < maxSegs) {
        segs.push(createEmptySegmentView());
      }
    }
  }

  /**
   * Push one arriving packet into the two-snapshot ring (`RW3-1`, `RW3-2`, `RW3-3`).
   * Drops stale out-of-order packets (`receivedAtMs < curr.receivedAtMs`).
   */
  push(
    payload: SimulationTickPayload,
    receivedAtMs: number,
    renderSize: number,
    sampleGroundY: GroundHeightSampler,
    bodyScaleFactor = this.lastBodyScaleFactor,
  ): boolean {
    if (!Number.isFinite(receivedAtMs)) return false;

    const prevCurr = this.currSlot;
    if (prevCurr !== null && receivedAtMs < prevCurr.snapshot.receivedAtMs) {
      // Out-of-order packet arrives earlier in wall time than current snapshot: ignore (`RW3-2`).
      return false;
    }

    const safeFactor =
      Number.isFinite(bodyScaleFactor) && bodyScaleFactor > 0
        ? bodyScaleFactor
        : this.lastBodyScaleFactor;
    this.lastBodyScaleFactor = safeFactor;
    const targetSlot = prevCurr === this.slotA ? this.slotB : this.slotA;
    targetSlot.fill(
      payload,
      receivedAtMs,
      renderSize,
      sampleGroundY,
      safeFactor,
      this.poseScratch,
    );

    if (prevCurr === null || receivedAtMs === prevCurr.snapshot.receivedAtMs) {
      this.prevSlot = null;
      this.currSlot = targetSlot;
      this.windowStartMs = receivedAtMs;
      this.windowEndMs = receivedAtMs;
    } else {
      this.prevSlot = prevCurr;
      this.currSlot = targetSlot;
      this.windowStartMs = Math.max(prevCurr.snapshot.receivedAtMs, receivedAtMs - this.renderDelayMs);
      this.windowEndMs = receivedAtMs;
    }

    this.ensurePoolCapacity();
    return true;
  }

  /** Re-prepare cached snapshots when terrain sampling / world scale props change. */
  rebuildForWorld(
    renderSize: number,
    sampleGroundY: GroundHeightSampler,
    bodyScaleFactor = this.lastBodyScaleFactor,
  ): void {
    const safeFactor =
      Number.isFinite(bodyScaleFactor) && bodyScaleFactor > 0
        ? bodyScaleFactor
        : this.lastBodyScaleFactor;
    this.lastBodyScaleFactor = safeFactor;
    const rebuildSlot = (slot: PoseSnapshotSlot | null): void => {
      if (slot) {
        slot.fill(
          slot.snapshot.payload,
          slot.snapshot.receivedAtMs,
          renderSize,
          sampleGroundY,
          safeFactor,
          this.poseScratch,
        );
      }
    };
    rebuildSlot(this.prevSlot);
    rebuildSlot(this.currSlot);
    this.ensurePoolCapacity();
  }

  /**
   * Read the latest arrived snapshot un-interpolated (for deterministic `?capture=1&replay=` shots).
   * Zero allocations.
   */
  sampleLatest(): InterpolatedFrameView {
    const view = this.view;
    if (this.currSlot === null) return this.resetView();
    const poses = this.currSlot.snapshot.poses;
    const dstAgents = view.agents;
    let totalSegments = 0;
    for (let i = 0; i < poses.length; i++) {
      totalSegments += copyAgentInto(poses[i], dstAgents[i]);
    }
    view.agentCount = poses.length;
    view.totalSegments = totalSegments;
    view.alpha = 1;
    return view;
  }

  /**
   * Sample interpolated creature & segment poses for render clock `nowMs` (`nowMs - renderDelayMs`),
   * mutating and returning the reusable `InterpolatedFrameView`. Zero heap allocations (`RW3-3`).
   */
  sample(
    nowMs: number,
    renderSize: number,
    sampleGroundY: GroundHeightSampler,
  ): InterpolatedFrameView {
    const view = this.view;
    if (this.currSlot === null) return this.resetView();

    const curr = this.currSlot.snapshot;
    const prev = this.prevSlot !== null ? this.prevSlot.snapshot : null;
    let alpha = 1;
    if (prev !== null && this.windowEndMs > this.windowStartMs && Number.isFinite(nowMs)) {
      const renderTimeMs = nowMs - this.renderDelayMs;
      alpha = clamp01((renderTimeMs - this.windowStartMs) / (this.windowEndMs - this.windowStartMs));
    }

    const dstAgents = view.agents;
    if (prev === null || alpha >= 1) {
      return this.sampleLatest();
    }

    const snapRenderDist = this.snapDistanceSim * simScale(renderSize);
    const snapDistSq = snapRenderDist * snapRenderDist;
    const currPoses = curr.poses;
    const prevById = prev.agentById;
    const prevSegByKey = prev.segmentByKey;

    let outAgentCount = 0;
    let totalSegments = 0;

    for (let i = 0; i < currPoses.length; i++) {
      const currAgent = currPoses[i];
      const dst = dstAgents[outAgentCount];
      const prevAgent = prevById.get(currAgent.agentId);

      const cr = currAgent.rootPosition;
      const pr = prevAgent?.rootPosition;
      const rdx = pr ? cr[0] - pr[0] : 0;
      const rdz = pr ? cr[2] - pr[2] : 0;
      if (
        !prevAgent ||
        !pr ||
        rdx * rdx + rdz * rdz > snapDistSq ||
        prevAgent.morphologyKey !== currAgent.morphologyKey
      ) {
        // Newborn, teleport, large jump, or morphology change (`RW3-2`, `N6`): snap directly.
        totalSegments += copyAgentInto(currAgent, dst);
        outAgentCount++;
        continue;
      }

      if (alpha <= 0) {
        totalSegments += copyAgentInto(prevAgent, dst);
        outAgentCount++;
        continue;
      }

      dst.agentId = currAgent.agentId;
      dst.agentType = currAgent.agentType;
      dst.energy = lerpNum(prevAgent.energy, currAgent.energy, alpha);
      dst.hydration = lerpNum(prevAgent.hydration, currAgent.hydration, alpha);
      dst.segmentCount = currAgent.segmentCount;

      const rx = pr[0] + rdx * alpha;
      const rz = pr[2] + rdz * alpha;
      const gY = sampleGroundY(rx, rz);
      const ry = gY + lerpNum(pr[1] - prevAgent.groundY, cr[1] - currAgent.groundY, alpha);
      const groundDelta = gY - lerpNum(prevAgent.groundY, currAgent.groundY, alpha);

      dst.groundY = gY;
      set3(dst.rootPosition, rx, ry, rz);
      dst.rootRenderLength = lerpNum(prevAgent.rootRenderLength, currAgent.rootRenderLength, alpha);
      dst.rootRenderRadius = lerpNum(prevAgent.rootRenderRadius, currAgent.rootRenderRadius, alpha);
      dst.boundingRadius = Math.max(prevAgent.boundingRadius, currAgent.boundingRadius);
      dst.morphologyKey = currAgent.morphologyKey;
      dst.morphologySegments = currAgent.segments;
      dst.headSegmentId = currAgent.headSegmentId;
      copy3(currAgent.headLocalForward, dst.headLocalForward);
      slerpDirection3Into(prevAgent.headDirection, currAgent.headDirection, alpha, dst.headDirection);

      const currSegs = currAgent.segments;
      const dstSegs = dst.segments;
      for (let sIdx = 0; sIdx < currSegs.length; sIdx++) {
        const cSeg = currSegs[sIdx];
        const dSeg = dstSegs[sIdx];
        const pSeg = prevSegByKey.get(packAgentSegmentKey(currAgent.agentId, cSeg.segmentId));
        const crp = cSeg.renderPosition;
        const prp = pSeg?.renderPosition;
        const sdx = prp ? crp[0] - prp[0] : 0;
        const sdz = prp ? crp[2] - prp[2] : 0;
        if (!pSeg || !prp || sdx * sdx + sdz * sdz > snapDistSq) {
          copySegmentInto(cSeg, dSeg);
          continue;
        }

        const drp = dSeg.renderPosition;
        dSeg.segmentId = cSeg.segmentId;
        dSeg.parentSegmentId = cSeg.parentSegmentId;
        dSeg.isRoot = cSeg.isRoot;
        lerpTuple3Into(prp, crp, alpha, drp);
        drp[1] += groundDelta;
        slerpQuatTupleInto(pSeg.quaternion, cSeg.quaternion, alpha, dSeg.quaternion);
        dSeg.renderLength = lerpNum(pSeg.renderLength, cSeg.renderLength, alpha);
        dSeg.renderRadius = lerpNum(pSeg.renderRadius, cSeg.renderRadius, alpha);
        copy3(cSeg.jointAnchor, dSeg.jointAnchor);

        if (!cSeg.isRoot) {
          dSeg.hasJoint = true;
          writeRearJointPositionInto(dSeg);
          const cjr = cSeg.jointRenderRadius;
          dSeg.jointRenderRadius = lerpNum(pSeg.jointRenderPosition ? pSeg.jointRenderRadius : cjr, cjr, alpha);
        } else {
          dSeg.hasJoint = false;
          dSeg.jointRenderRadius = 0;
        }
      }

      totalSegments += currSegs.length;
      outAgentCount++;
    }

    // Dead creatures (`RW3-2`): keep visible only within the active interpolation window (`alpha < 1`,
    // i.e. <= 1 emit period after the packet reporting their absence), then drop cleanly.
    const prevPoses = prev.poses;
    const currById = curr.agentById;
    for (let i = 0; i < prevPoses.length; i++) {
      const prevAgent = prevPoses[i];
      if (!currById.has(prevAgent.agentId)) {
        totalSegments += copyAgentInto(prevAgent, dstAgents[outAgentCount]);
        outAgentCount++;
      }
    }

    view.agentCount = outAgentCount;
    view.totalSegments = totalSegments;
    view.alpha = alpha;
    return view;
  }
}
