import type { AgentType } from '../../types/generated/AgentType';
import type { SimulationTickPayload } from '../../types/generated/SimulationTickPayload';
import {
  buildAgentPoses,
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

/** In-place 3-tuple linear interpolation (`alpha` clamped in `[0, 1]`, zero allocations). */
export function lerpTuple3Into(
  a: readonly [number, number, number],
  b: readonly [number, number, number],
  alpha: number,
  out: [number, number, number],
): [number, number, number] {
  const t = clamp01(alpha);
  out[0] = a[0] + (b[0] - a[0]) * t;
  out[1] = a[1] + (b[1] - a[1]) * t;
  out[2] = a[2] + (b[2] - a[2]) * t;
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
    out[0] = ox * inv;
    out[1] = oy * inv;
    out[2] = oz * inv;
    out[3] = ow * inv;
  } else {
    out[0] = 0;
    out[1] = 0;
    out[2] = 0;
    out[3] = 1;
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
      out[0] = b[0] / blen;
      out[1] = b[1] / blen;
      out[2] = b[2] / blen;
    } else if (alen > 1e-8) {
      out[0] = a[0] / alen;
      out[1] = a[1] / alen;
      out[2] = a[2] / alen;
    } else {
      out[0] = 0;
      out[1] = 0;
      out[2] = 1;
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
    out[0] = lx / len;
    out[1] = ly / len;
    out[2] = lz / len;
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
    out[0] = ox / len;
    out[1] = oy / len;
    out[2] = oz / len;
  } else {
    out[0] = bx;
    out[1] = by;
    out[2] = bz;
  }
  return out;
}

/**
 * Collision-free numeric key for `(agent_id, segment_id)` lookup without string allocation (`RW3-3`).
 */
export function packAgentSegmentKey(agentId: number, segmentId: number): number {
  return Math.trunc(agentId) * 65536 + (Math.trunc(segmentId) & 0xffff);
}

export interface PreparedPoseSnapshot {
  receivedAtMs: number;
  payload: SimulationTickPayload;
  poses: AgentRenderPose[];
  agentById: Map<number, AgentRenderPose>;
  segmentByKey: Map<number, SegmentRenderPose>;
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
): PreparedPoseSnapshot {
  const poses = buildAgentPoses(
    payload.segments,
    renderSize,
    sampleGroundY,
    payload.head_directions,
  );
  const agentById = new Map<number, AgentRenderPose>();
  const segmentByKey = new Map<number, SegmentRenderPose>();
  for (let i = 0; i < poses.length; i++) {
    const pose = poses[i];
    agentById.set(pose.agentId, pose);
    for (let s = 0; s < pose.segments.length; s++) {
      const seg = pose.segments[s];
      segmentByKey.set(packAgentSegmentKey(pose.agentId, seg.segmentId), seg);
    }
  }
  return {
    receivedAtMs,
    payload,
    poses,
    agentById,
    segmentByKey,
  };
}

export interface InterpolatedSegmentView {
  segmentId: number;
  isRoot: boolean;
  renderPosition: [number, number, number];
  quaternion: [number, number, number, number];
  renderLength: number;
  renderRadius: number;
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
    isRoot: false,
    renderPosition: [0, 0, 0],
    quaternion: [0, 0, 0, 1],
    renderLength: 0,
    renderRadius: 0,
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
    headDirection: [0, 0, 1],
    segments: [],
  };
}

function copySegmentInto(src: SegmentRenderPose, dst: InterpolatedSegmentView): void {
  dst.segmentId = src.segmentId;
  dst.isRoot = src.isRoot;
  dst.renderPosition[0] = src.renderPosition[0];
  dst.renderPosition[1] = src.renderPosition[1];
  dst.renderPosition[2] = src.renderPosition[2];
  dst.quaternion[0] = src.quaternion[0];
  dst.quaternion[1] = src.quaternion[1];
  dst.quaternion[2] = src.quaternion[2];
  dst.quaternion[3] = src.quaternion[3];
  dst.renderLength = src.renderLength;
  dst.renderRadius = src.renderRadius;
  if (src.jointRenderPosition) {
    dst.hasJoint = true;
    dst.jointRenderPosition[0] = src.jointRenderPosition[0];
    dst.jointRenderPosition[1] = src.jointRenderPosition[1];
    dst.jointRenderPosition[2] = src.jointRenderPosition[2];
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
  dst.rootPosition[0] = src.rootPosition[0];
  dst.rootPosition[1] = src.rootPosition[1];
  dst.rootPosition[2] = src.rootPosition[2];
  dst.rootRenderLength = src.rootRenderLength;
  dst.rootRenderRadius = src.rootRenderRadius;
  dst.headDirection[0] = src.headDirection[0];
  dst.headDirection[1] = src.headDirection[1];
  dst.headDirection[2] = src.headDirection[2];
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
  const backX = halfLen * 2 * (qx * qz + qw * qy);
  const backY = halfLen * 2 * (qy * qz - qw * qx);
  const backZ = halfLen * (1 - 2 * (qx * qx + qy * qy));
  dst.jointRenderPosition[0] = dst.renderPosition[0] - backX;
  dst.jointRenderPosition[1] = dst.renderPosition[1] - backY;
  dst.jointRenderPosition[2] = dst.renderPosition[2] - backZ;
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

  private prev: PreparedPoseSnapshot | null = null;
  private curr: PreparedPoseSnapshot | null = null;
  private windowStartMs = 0;
  private windowEndMs = 0;

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
    return this.curr !== null;
  }

  hasActiveAgents(): boolean {
    return (this.curr !== null && this.curr.poses.length > 0) ||
      (this.prev !== null && this.prev.poses.length > 0);
  }

  clear(): void {
    this.prev = null;
    this.curr = null;
    this.windowStartMs = 0;
    this.windowEndMs = 0;
    this.view.agentCount = 0;
    this.view.totalSegments = 0;
    this.view.alpha = 1;
  }

  private ensurePoolCapacity(): void {
    const prevAgents = this.prev?.poses ?? [];
    const currAgents = this.curr?.poses ?? [];
    const maxAgentSlots = prevAgents.length + currAgents.length;
    let maxSegs = 0;
    for (let i = 0; i < prevAgents.length; i++) {
      if (prevAgents[i].segments.length > maxSegs) maxSegs = prevAgents[i].segments.length;
    }
    for (let i = 0; i < currAgents.length; i++) {
      if (currAgents[i].segments.length > maxSegs) maxSegs = currAgents[i].segments.length;
    }

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
  ): boolean {
    if (!Number.isFinite(receivedAtMs)) return false;

    if (this.curr !== null && receivedAtMs < this.curr.receivedAtMs) {
      // Out-of-order packet arrives earlier in wall time than current snapshot: ignore (`RW3-2`).
      return false;
    }

    const nextSnap = preparePoseSnapshot(payload, receivedAtMs, renderSize, sampleGroundY);

    if (this.curr === null) {
      this.prev = null;
      this.curr = nextSnap;
      this.windowStartMs = receivedAtMs;
      this.windowEndMs = receivedAtMs;
    } else if (receivedAtMs === this.curr.receivedAtMs) {
      // Same-millisecond replacement (e.g. synchronous test ticks without clock advance).
      this.prev = null;
      this.curr = nextSnap;
      this.windowStartMs = receivedAtMs;
      this.windowEndMs = receivedAtMs;
    } else {
      this.prev = this.curr;
      this.curr = nextSnap;
      this.windowStartMs = Math.max(this.prev.receivedAtMs, receivedAtMs - this.renderDelayMs);
      this.windowEndMs = receivedAtMs;
    }

    this.ensurePoolCapacity();
    return true;
  }

  /** Re-prepare cached snapshots when terrain sampling / world scale props change. */
  rebuildForWorld(renderSize: number, sampleGroundY: GroundHeightSampler): void {
    if (this.prev) {
      this.prev = preparePoseSnapshot(
        this.prev.payload,
        this.prev.receivedAtMs,
        renderSize,
        sampleGroundY,
      );
    }
    if (this.curr) {
      this.curr = preparePoseSnapshot(
        this.curr.payload,
        this.curr.receivedAtMs,
        renderSize,
        sampleGroundY,
      );
    }
    this.ensurePoolCapacity();
  }

  /**
   * Read the latest arrived snapshot un-interpolated (for deterministic `?capture=1&replay=` shots).
   * Zero allocations.
   */
  sampleLatest(): InterpolatedFrameView {
    const view = this.view;
    if (this.curr === null) {
      view.agentCount = 0;
      view.totalSegments = 0;
      view.alpha = 1;
      return view;
    }
    const poses = this.curr.poses;
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
    const curr = this.curr;
    if (curr === null) {
      view.agentCount = 0;
      view.totalSegments = 0;
      view.alpha = 1;
      return view;
    }

    const prev = this.prev;
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

      if (!prevAgent) {
        // Newborn creature (`RW3-2`): snap directly to current spawn without sliding from origin.
        totalSegments += copyAgentInto(currAgent, dst);
        outAgentCount++;
        continue;
      }

      const rdx = currAgent.rootPosition[0] - prevAgent.rootPosition[0];
      const rdz = currAgent.rootPosition[2] - prevAgent.rootPosition[2];
      if (rdx * rdx + rdz * rdz > snapDistSq) {
        // Teleport / large discontinuous jump (`RW3-2`): snap directly without sliding.
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
      dst.energy = prevAgent.energy + (currAgent.energy - prevAgent.energy) * alpha;
      dst.hydration = prevAgent.hydration + (currAgent.hydration - prevAgent.hydration) * alpha;
      dst.segmentCount = currAgent.segmentCount;

      const rx = prevAgent.rootPosition[0] + rdx * alpha;
      const rz = prevAgent.rootPosition[2] + rdz * alpha;
      const gY = sampleGroundY(rx, rz);
      const prevLift = prevAgent.rootPosition[1] - prevAgent.groundY;
      const currLift = currAgent.rootPosition[1] - currAgent.groundY;
      const ry = gY + prevLift + (currLift - prevLift) * alpha;
      const blendedGroundY = prevAgent.groundY + (currAgent.groundY - prevAgent.groundY) * alpha;
      const groundDelta = gY - blendedGroundY;

      dst.groundY = gY;
      dst.rootPosition[0] = rx;
      dst.rootPosition[1] = ry;
      dst.rootPosition[2] = rz;
      dst.rootRenderLength =
        prevAgent.rootRenderLength + (currAgent.rootRenderLength - prevAgent.rootRenderLength) * alpha;
      dst.rootRenderRadius =
        prevAgent.rootRenderRadius + (currAgent.rootRenderRadius - prevAgent.rootRenderRadius) * alpha;
      slerpDirection3Into(prevAgent.headDirection, currAgent.headDirection, alpha, dst.headDirection);

      const currSegs = currAgent.segments;
      const dstSegs = dst.segments;
      for (let sIdx = 0; sIdx < currSegs.length; sIdx++) {
        const cSeg = currSegs[sIdx];
        const dSeg = dstSegs[sIdx];
        const pSeg = prevSegByKey.get(packAgentSegmentKey(currAgent.agentId, cSeg.segmentId));
        if (!pSeg) {
          copySegmentInto(cSeg, dSeg);
          continue;
        }
        const sdx = cSeg.renderPosition[0] - pSeg.renderPosition[0];
        const sdz = cSeg.renderPosition[2] - pSeg.renderPosition[2];
        if (sdx * sdx + sdz * sdz > snapDistSq) {
          copySegmentInto(cSeg, dSeg);
          continue;
        }

        dSeg.segmentId = cSeg.segmentId;
        dSeg.isRoot = cSeg.isRoot;
        lerpTuple3Into(pSeg.renderPosition, cSeg.renderPosition, alpha, dSeg.renderPosition);
        dSeg.renderPosition[1] += groundDelta;
        slerpQuatTupleInto(pSeg.quaternion, cSeg.quaternion, alpha, dSeg.quaternion);
        dSeg.renderLength =
          pSeg.renderLength + (cSeg.renderLength - pSeg.renderLength) * alpha;
        dSeg.renderRadius =
          pSeg.renderRadius + (cSeg.renderRadius - pSeg.renderRadius) * alpha;

        if (!cSeg.isRoot) {
          dSeg.hasJoint = true;
          writeRearJointPositionInto(dSeg);
          const pJR = pSeg.jointRenderPosition ? pSeg.jointRenderRadius : cSeg.jointRenderRadius;
          dSeg.jointRenderRadius = pJR + (cSeg.jointRenderRadius - pJR) * alpha;
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
