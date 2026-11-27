import * as THREE from 'three';
import type { AgentType } from '../../../types/generated/AgentType';
import type { SegmentState } from '../../../types/generated/SegmentState';
import { SIM_BOUNDS_HALF, isInsideSimBounds, simScale, simToRender } from './liveAgentTransform';

// ---------------------------------------------------------------------------------------
// Pure multi-segment creature pose, coloration, and LOD-marker helpers (RW2-1 .. RW2-7).
//
// # RW2-1 Hypotheses (to verify against RW-D4-RECORDING once a real recording is dated)
//
// 1. Segment local long axis is local `Z` (`[-length / 2, +length / 2]`), inferred from
//    `src-tauri/src/evolution/genotype.rs:193-198` & `mutation.rs` (clamping `joint_anchor.z`
//    to `±length / 2`) and `physics/dynamics.rs:148-150`.
// 2. Simulation `y` lives in a `0..10` band unrelated to terrain elevation (`LiveAgents.tsx`), so
//    only the vertical difference `(seg.y - root.y) * simScale` between segments belonging to the
//    SAME creature is meaningful; the creature as a whole rests on `surfaceHeight`.
// ---------------------------------------------------------------------------------------

export const MIN_SIM_SEGMENT_LENGTH = 0.08;
export const MIN_SIM_SEGMENT_RADIUS = 0.04;
export const FAR_MARKER_DISTANCE_FACTOR = 0.45;
// prototype rendering choice — NOT to scale with positions: root positions stay at 6 render
// units per sim unit while bodies draw at 1.92 (0.32x) so bodies fit decorative `WorldVegetation`
// trees and lakes. Touching sim creatures (r = 0.2) draw ~4 body radii apart, and bodies draw at
// 0.32x relative to simulated lakes (MSG_20261119_042316, B1 verdict R4).
export const CREATURE_BODY_SCALE = 0.32;

export interface EulerAnglesYXZ {
  yaw: number;
  pitch: number;
  roll: number;
}

export type GroundHeightSampler = (renderX: number, renderZ: number) => number;

export interface SegmentRenderPose {
  segmentId: number;
  parentSegmentId: number | null;
  isRoot: boolean;
  renderPosition: [number, number, number];
  quaternion: [number, number, number, number];
  renderLength: number;
  renderRadius: number;
  /** Scaled rest joint offset relative to parent center (`[0, 0, 0]` on root). */
  jointAnchor: [number, number, number];
  /** Column-major 16-element matrix for a unit Z-aligned cylinder (length=1 on Z, radius=1 on XY). */
  matrixElements: number[];
  /** World-space rear attachment point (`pos - R * (0, 0, length / 2)`) for child segments. */
  jointRenderPosition: [number, number, number] | null;
  jointRenderRadius: number;
}

export interface AgentRenderPose {
  agentId: number;
  agentType: AgentType | null;
  energy: number;
  hydration: number;
  segmentCount: number;
  groundY: number;
  rootPosition: [number, number, number];
  rootQuaternion: [number, number, number, number];
  rootRenderLength: number;
  rootRenderRadius: number;
  /** World-space bounding radius centred on `rootPosition` for LOD/frustum culling. */
  boundingRadius: number;
  /** Deterministic cache key for this creature's scaled segment & joint morphology. */
  morphologyKey: string;
  /** Segment ID carrying the decorative front eyes/snout across near & mid tiers (`D1`). */
  headSegmentId: number;
  /** Local forward axis of `headSegmentId` pointing out of the front pole (`D1`). */
  headLocalForward: [number, number, number];
  headDirection: [number, number, number];
  segments: SegmentRenderPose[];
}

export interface MorphologySegmentLike {
  segmentId: number;
  parentSegmentId: number | null;
  isRoot: boolean;
  renderLength: number;
  renderRadius: number;
  jointAnchor: readonly [number, number, number];
}

export interface MorphologyRestNode {
  boneIdx: number;
  segmentId: number;
  parentBoneIdx: number | null;
  length: number;
  radius: number;
  restPos: [number, number, number];
  jointPos: [number, number, number];
}

export interface MorphologyHeadAttachment {
  readonly boneIdx: number;
  readonly segmentId: number;
  readonly frontShoulderDist: number;
  readonly headRadius: number;
  readonly localForward: readonly [number, number, number];
}

export interface MorphologyRestLayout {
  readonly nodes: MorphologyRestNode[];
  readonly spineChain: MorphologyRestNode[];
  readonly inSpine: Uint8Array;
  readonly headAttachment: MorphologyHeadAttachment;
  readonly usesRootAxialCap: boolean;
}

function normalizeVec3Or(
  vx: number,
  vy: number,
  vz: number,
  fx: number,
  fy: number,
  fz: number,
): [number, number, number] {
  const len = Math.hypot(vx, vy, vz);
  if (len > 1e-6) return [vx / len, vy / len, vz / len];
  return [fx, fy, fz];
}

// Decorative head attachment shared by near (SkinnedMesh) and mid (instanced) tiers (D1):
// eyes and snout are decorative and mark the front of the body along head_direction;
// the simulation has no head organ and senses at the root position.
export function buildMorphologyRestLayout(
  segments: readonly MorphologySegmentLike[],
): MorphologyRestLayout {
  const safeSegments: readonly MorphologySegmentLike[] =
    segments.length > 0
      ? segments
      : [
          {
            segmentId: 0,
            parentSegmentId: null,
            isRoot: true,
            renderLength: 0.8,
            renderRadius: 0.25,
            jointAnchor: [0, 0, 0],
          },
        ];

  const ordered = safeSegments
    .slice()
    .sort((a, b) => (a.isRoot === b.isRoot ? a.segmentId - b.segmentId : a.isRoot ? -1 : 1));

  const indexBySegId = new Map<number, number>();
  for (let i = 0; i < ordered.length; i++) indexBySegId.set(ordered[i].segmentId, i);

  const nodes: MorphologyRestNode[] = ordered.map((s, boneIdx) => ({
    boneIdx,
    segmentId: s.segmentId,
    parentBoneIdx:
      boneIdx === 0
        ? null
        : s.parentSegmentId !== null
          ? (indexBySegId.get(s.parentSegmentId) ?? 0)
          : 0,
    length: Math.max(0.1, Number.isFinite(s.renderLength) ? s.renderLength : 0.6),
    radius: Math.max(0.05, Number.isFinite(s.renderRadius) ? s.renderRadius : 0.2),
    restPos: [0, 0, 0],
    jointPos: [0, 0, 0],
  }));

  const placed = new Uint8Array(nodes.length);
  placed[0] = 1;
  const placeFrom = (
    node: MorphologyRestNode,
    ref: MorphologyRestNode,
    rawA: readonly [number, number, number] | null,
  ) => {
    const pr = ref.restPos;
    const ax = rawA ? finiteOr(rawA[0]) : 0;
    const ay = rawA ? finiteOr(rawA[1]) : 0;
    const az = rawA ? finiteOr(rawA[2]) : 0;
    const axial = Math.hypot(ax, ay, az) <= 1e-5;
    const jx = axial ? pr[0] : pr[0] + ax;
    const jy = axial ? pr[1] : pr[1] + ay;
    const jz = axial ? pr[2] + ref.length * 0.5 : pr[2] + az;
    node.jointPos = [jx, jy, jz];
    node.restPos = [jx, jy, jz + node.length * 0.5];
  };
  let progress = true;
  while (progress) {
    progress = false;
    for (let i = 1; i < nodes.length; i++) {
      if (placed[i]) continue;
      const node = nodes[i];
      const pIdx = node.parentBoneIdx ?? 0;
      if (!placed[pIdx] && i !== pIdx) continue;
      placeFrom(node, nodes[pIdx === i ? 0 : pIdx], ordered[i].jointAnchor);
      placed[i] = 1;
      progress = true;
    }
  }

  for (let i = 1; i < nodes.length; i++) {
    if (!placed[i]) placeFrom(nodes[i], nodes[i - 1], null);
  }

  const childrenByParent = new Map<number, MorphologyRestNode[]>();
  for (let i = 1; i < nodes.length; i++) {
    const pIdx = nodes[i].parentBoneIdx ?? 0;
    const list = childrenByParent.get(pIdx);
    if (list) list.push(nodes[i]);
    else childrenByParent.set(pIdx, [nodes[i]]);
  }

  const spineChain: MorphologyRestNode[] = [nodes[0]];
  const inSpine = new Uint8Array(nodes.length);
  inSpine[0] = 1;
  let cursor = nodes[0];
  while (true) {
    const kids = childrenByParent.get(cursor.boneIdx);
    if (!kids || kids.length === 0) break;
    const cx = cursor.restPos[0];
    let best = kids[0];
    let bestLateral = Math.abs(best.jointPos[0] - cx);
    for (let i = 1; i < kids.length; i++) {
      const lat = Math.abs(kids[i].jointPos[0] - cx);
      if (lat < bestLateral) {
        bestLateral = lat;
        best = kids[i];
      }
    }
    if (inSpine[best.boneIdx]) break;
    spineChain.push(best);
    inSpine[best.boneIdx] = 1;
    cursor = best;
  }

  const firstNode = spineChain[0];
  const lastNode = spineChain[spineChain.length - 1];
  let headNode = firstNode;
  let headForward: [number, number, number] = [0, 0, 1];
  let usesRootAxialCap = false;

  if (spineChain.length > 1) {
    const fr = firstNode.restPos;
    const sr = spineChain[1].restPos;
    const lr = lastNode.restPos;
    const plr = spineChain[spineChain.length - 2].restPos;
    const startDir = normalizeVec3Or(sr[0] - fr[0], sr[1] - fr[1], sr[2] - fr[2], 0, 0, 1);
    const endDir = normalizeVec3Or(lr[0] - plr[0], lr[1] - plr[1], lr[2] - plr[2], 0, 0, 1);

    if (startDir[2] < -0.7) {
      headForward = [-startDir[0], -startDir[1], -startDir[2]];
    } else if (Math.abs(startDir[2]) <= 0.7) {
      usesRootAxialCap = true;
    } else {
      headNode = lastNode;
      headForward = endDir;
    }
  }

  return {
    nodes,
    spineChain,
    inSpine,
    headAttachment: {
      boneIdx: headNode.boneIdx,
      segmentId: headNode.segmentId,
      frontShoulderDist: headNode.length * 0.5,
      headRadius: headNode.radius,
      localForward: headForward,
    },
    usesRootAxialCap,
  };
}

const DEFAULT_FALLBACK_LENGTH = 0.5;
const DEFAULT_FALLBACK_RADIUS = 0.15;
const LOCAL_FORWARD = new THREE.Vector3(0, 0, 1);
const WORLD_UP = new THREE.Vector3(0, 1, 0);
const FALLBACK_UP = new THREE.Vector3(1, 0, 0);

function finiteOr(value: number, fallback = 0): number {
  return Number.isFinite(value) ? value : fallback;
}

/**
 * Convert backend `(yaw, pitch, roll)` (`glam::EulerRot::YXZ` in `simulation_loop.rs:2034`)
 * into a unit `THREE.Quaternion`. Three.js stores Euler `(x, y, z, 'YXZ')` as `(pitch, yaw, roll)`.
 */
export function eulerYXZToQuaternion(
  yaw: number,
  pitch: number,
  roll: number,
  out = new THREE.Quaternion(),
  scratchEuler = new THREE.Euler(0, 0, 0, 'YXZ'),
): THREE.Quaternion {
  scratchEuler.set(finiteOr(pitch), finiteOr(yaw), finiteOr(roll), 'YXZ');
  return out.setFromEuler(scratchEuler);
}

/**
 * Recover `{ yaw, pitch, roll }` in intrinsic `YXZ` order from a unit `THREE.Quaternion`.
 */
export function quaternionToEulerYXZ(
  quat: THREE.Quaternion,
  scratchEuler = new THREE.Euler(0, 0, 0, 'YXZ'),
): EulerAnglesYXZ {
  scratchEuler.setFromQuaternion(quat, 'YXZ');
  return {
    yaw: scratchEuler.y,
    pitch: scratchEuler.x,
    roll: scratchEuler.z,
  };
}

/** Deterministic integer hash in `[0, 1)` keyed by `agent_id` (RW2-4, zero global RNG). */
export function hashAgentId01(agentId: number, salt = 0): number {
  let h = Math.imul((Math.trunc(finiteOr(agentId)) ^ (salt * 0x9e3779b1)) + 1, 374761393);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

/**
 * Deterministic creature color (`RW2-4`):
 * - base hue from `agent_type` (`prey` green, `predator` crimson, `null` slate — separated by >100°
 *   plus distinct lightness so prey and predator remain separable in grayscale);
 * - subtle deterministic per-individual variation from `agent_id`;
 * - energy brightness curve preserved (`0.45 + 0.55 * clamp(energy / 100, 0, 1)`).
 */
export function computeCreatureColor(
  agentType: AgentType | null,
  agentId: number,
  energy: number,
  out = new THREE.Color(),
): THREE.Color {
  const hueOffset = hashAgentId01(agentId, 1) - 0.5;
  const satOffset = hashAgentId01(agentId, 2) - 0.5;

  if (agentType === 'prey') {
    const h = 0.38 + hueOffset * 0.08; // [0.34, 0.42] emerald-lime band
    const s = 0.62 + satOffset * 0.1;
    const l = 0.58;
    out.setHSL(h, s, l);
  } else if (agentType === 'predator') {
    const h = (1.005 + hueOffset * 0.065) % 1; // [0.9725, 1) U [0, 0.0375] crimson-scarlet band
    const s = 0.74 + satOffset * 0.1;
    const l = 0.44;
    out.setHSL(h, s, l);
  } else {
    const h = 0.58 + hueOffset * 0.04;
    const s = 0.12;
    const l = 0.76;
    out.setHSL(h, s, l);
  }

  const normalizedEnergy = Math.min(1, Math.max(0, finiteOr(energy) / 100));
  const brightness = 0.45 + 0.55 * normalizedEnergy;
  return out.multiplyScalar(brightness);
}

function trySetUnitVec3(v: readonly number[] | undefined, out: THREE.Vector3): boolean {
  if (
    Array.isArray(v) &&
    v.length >= 3 &&
    Number.isFinite(v[0]) &&
    Number.isFinite(v[1]) &&
    Number.isFinite(v[2])
  ) {
    out.set(v[0], v[1], v[2]);
    if (out.lengthSq() > 1e-8) {
      out.normalize();
      return true;
    }
  }
  return false;
}

/**
 * Resolve normalized forward heading for a creature root segment (`headDirections` map ->
 * `root.head_direction` -> root forward vector rotated by `rootQuat`).
 */
export function resolveHeadDirectionVector(
  root: SegmentState,
  rootQuat: THREE.Quaternion,
  headDirections?: Readonly<Record<number, [number, number, number] | undefined>>,
  out = new THREE.Vector3(),
): THREE.Vector3 {
  if (
    trySetUnitVec3(headDirections?.[root.agent_id], out) ||
    trySetUnitVec3(root.head_direction, out)
  ) {
    return out;
  }
  out.copy(LOCAL_FORWARD).applyQuaternion(rootQuat);
  return out.lengthSq() > 1e-8 ? out.normalize() : out.set(0, 0, 1);
}

/**
 * Derive an orthonormal `(right, up)` basis perpendicular to a unit `forward` heading vector.
 */
export function computeHeadBasis(
  forward: THREE.Vector3,
  outRight: THREE.Vector3,
  outUp: THREE.Vector3,
): void {
  const upRef = Math.abs(forward.dot(WORLD_UP)) > 0.94 ? FALLBACK_UP : WORLD_UP;
  outRight.crossVectors(upRef, forward);
  if (outRight.lengthSq() <= 1e-8) {
    outRight.set(1, 0, 0);
  } else {
    outRight.normalize();
  }
  outUp.crossVectors(forward, outRight).normalize();
}

export function copy3(
  src: readonly [number, number, number],
  dst: [number, number, number],
): void {
  dst[0] = src[0];
  dst[1] = src[1];
  dst[2] = src[2];
}

export function set3(
  dst: [number, number, number],
  x: number,
  y: number,
  z: number,
): void {
  dst[0] = x;
  dst[1] = y;
  dst[2] = z;
}

export function set4(
  dst: [number, number, number, number],
  x: number,
  y: number,
  z: number,
  w: number,
): void {
  dst[0] = x;
  dst[1] = y;
  dst[2] = z;
  dst[3] = w;
}

export interface AgentPosePool {
  readonly poses: AgentRenderPose[];
  readonly jointBufs: Array<Array<[number, number, number]>>;
}

export interface AgentPoseScratch {
  run(
    segments: readonly SegmentState[],
    renderSize: number,
    sampleGroundY: GroundHeightSampler,
    outPoses: AgentRenderPose[],
    pool: AgentPosePool,
    headDirections?: Readonly<Record<number, [number, number, number] | undefined>>,
    bodyScaleFactor?: number,
  ): AgentRenderPose[];
}

// Tuple types keep internal pool slots property-free so minification drops every field name.
// [stamp, count, segs]
type GroupBucket = [number, number, SegmentState[]];
// [stamp, key, headSegId, fwd0, fwd1, fwd2, count, sig]
type CachedMorph = [
  number,
  string,
  number,
  number,
  number,
  number,
  number,
  Array<number | null>,
];

function createEmptySegPose(): SegmentRenderPose {
  return {
    segmentId: 0,
    parentSegmentId: null,
    isRoot: false,
    renderPosition: [0, 0, 0],
    quaternion: [0, 0, 0, 1],
    renderLength: 0,
    renderRadius: 0,
    jointAnchor: [0, 0, 0],
    matrixElements: new Array<number>(16).fill(0),
    jointRenderPosition: null,
    jointRenderRadius: 0,
  };
}

function createEmptyAgentPose(): AgentRenderPose {
  return {
    agentId: 0,
    agentType: null,
    energy: 0,
    hydration: 0,
    segmentCount: 0,
    groundY: 0,
    rootPosition: [0, 0, 0],
    rootQuaternion: [0, 0, 0, 1],
    rootRenderLength: 0,
    rootRenderRadius: 0,
    boundingRadius: 0,
    morphologyKey: '',
    headSegmentId: 0,
    headLocalForward: [0, 0, 1],
    headDirection: [0, 0, 1],
    segments: [],
  };
}

export function createAgentPosePool(): AgentPosePool {
  return { poses: [], jointBufs: [] };
}

/**
 * Allocate reusable scratch state closed over local bindings so `buildAgentPosesInto` performs
 * zero per-tick allocations while adding minimal minified bundle bytes.
 */
export function createAgentPoseScratch(): AgentPoseScratch {
  const euler = new THREE.Euler(0, 0, 0, 'YXZ');
  const quat = new THREE.Quaternion();
  const rootQuat = new THREE.Quaternion();
  const posVec = new THREE.Vector3();
  const scaleVec = new THREE.Vector3();
  const halfBackVec = new THREE.Vector3();
  const headVec = new THREE.Vector3();
  const matrix = new THREE.Matrix4();
  const grouped = new Map<number, GroupBucket>();
  const groupIds: number[] = [];
  const validSegs: SegmentState[] = [];
  const sortedSegs: SegmentRenderPose[] = [];
  const keyParts: string[] = [];
  const morphCache = new Map<number, CachedMorph>();
  let groupStamp = 0;

  const dropStale = <T extends [number, ...unknown[]]>(
    map: Map<number, T>,
    max: number,
    st: number,
  ): void => {
    if (map.size > max) {
      map.forEach((v, id) => {
        if (v[0] !== st) map.delete(id);
      });
    }
  };
  const clearSlots = (arr: unknown[], n: number): void => {
    for (let i = 0; i < n; i++) arr[i] = undefined;
  };
  const syncMorph = (
    sig: Array<number | null>,
    b: number,
    sp: SegmentRenderPose,
    ja: readonly [number, number, number],
  ): boolean => {
    const isR = sp.isRoot ? 1 : 0;
    const ok =
      sig[b] === sp.segmentId &&
      sig[b + 1] === sp.parentSegmentId &&
      sig[b + 2] === isR &&
      sig[b + 3] === sp.renderLength &&
      sig[b + 4] === sp.renderRadius &&
      sig[b + 5] === ja[0] &&
      sig[b + 6] === ja[1] &&
      sig[b + 7] === ja[2];
    if (!ok) {
      sig[b] = sp.segmentId;
      sig[b + 1] = sp.parentSegmentId;
      sig[b + 2] = isR;
      sig[b + 3] = sp.renderLength;
      sig[b + 4] = sp.renderRadius;
      sig[b + 5] = ja[0];
      sig[b + 6] = ja[1];
      sig[b + 7] = ja[2];
    }
    return ok;
  };
  const segDim = (v: number, isRad: boolean, s: number) =>
    (!Number.isFinite(v) || v <= 0
      ? isRad
        ? DEFAULT_FALLBACK_RADIUS
        : DEFAULT_FALLBACK_LENGTH
      : Math.max(isRad ? MIN_SIM_SEGMENT_RADIUS : MIN_SIM_SEGMENT_LENGTH, v)) * s;

  return {
    run(segments, renderSize, sampleGroundY, outPoses, pool, headDirections, bodyScaleFactor = 1) {
      const scale = simScale(renderSize);
      const stamp = ++groupStamp;
      if (!Array.isArray(segments) || segments.length === 0 || !Number.isFinite(scale) || scale <= 0) {
        dropStale(grouped, 0, stamp);
        dropStale(morphCache, 0, stamp);
        if (outPoses.length !== 0) outPoses.length = 0;
        return outPoses;
      }

      const safeBodyFactor =
        Number.isFinite(bodyScaleFactor) && bodyScaleFactor > 0 ? bodyScaleFactor : 1;
      const bodyScale = scale * safeBodyFactor;

      let groupCount = 0;
      for (let i = 0; i < segments.length; i++) {
        const seg = segments[i];
        if (!seg || typeof seg.agent_id !== 'number' || !Number.isFinite(seg.agent_id)) continue;
        let bucket = grouped.get(seg.agent_id);
        if (!bucket) {
          bucket = [0, 0, []];
          grouped.set(seg.agent_id, bucket);
        }
        if (bucket[0] !== stamp) {
          bucket[0] = stamp;
          bucket[1] = 0;
          groupIds[groupCount++] = seg.agent_id;
        }
        bucket[2][bucket[1]++] = seg;
      }

      const childExtentBound = SIM_BOUNDS_HALF * 1.5;
      let poseCount = 0;

      for (let g = 0; g < groupCount; g++) {
        const agentId = groupIds[g];
        const bucket = grouped.get(agentId)!;
        const rawList = bucket[2];
        const rawLen = bucket[1];

        let root = rawList[0];
        for (let i = 0; i < rawLen; i++) {
          const candidate = rawList[i];
          if (candidate.parent_segment_id === null || candidate.parent_segment_id === undefined) {
            root = candidate;
            break;
          }
        }

        let validCount = 0;
        if (isInsideSimBounds(root.x, root.z)) {
          for (let i = 0; i < rawLen; i++) {
            const s = rawList[i];
            if (
              !Number.isFinite(s.x) ||
              !Number.isFinite(s.z) ||
              Math.abs(s.x) > childExtentBound ||
              Math.abs(s.z) > childExtentBound
            ) {
              continue;
            }
            validSegs[validCount++] = s;
          }
        }
        clearSlots(rawList, rawLen);
        bucket[1] = 0;
        if (validCount === 0) continue;

        const rootRx = simToRender(root.x, renderSize);
        const rootRz = simToRender(root.z, renderSize);
        const sampledGroundY = sampleGroundY(rootRx, rootRz);
        const groundY = Number.isFinite(sampledGroundY) ? sampledGroundY : 0;

        const rootY = finiteOr(root.y);
        const rootRenderLength = segDim(root.length, false, bodyScale);
        const rootRenderRadius = segDim(root.radius, true, bodyScale);

        let minBottomOffset = -rootRenderRadius;
        for (let i = 0; i < validCount; i++) {
          const s = validSegs[i];
          const bottomOffset =
            (finiteOr(s.y, rootY) - rootY) * bodyScale - segDim(s.radius, true, bodyScale);
          if (bottomOffset < minBottomOffset) minBottomOffset = bottomOffset;
        }
        const rootRy = groundY - minBottomOffset;

        eulerYXZToQuaternion(root.yaw, root.pitch, root.roll, rootQuat, euler);
        resolveHeadDirectionVector(root, rootQuat, headDirections, headVec);

        while (pool.poses.length <= poseCount) {
          pool.poses.push(createEmptyAgentPose());
          pool.jointBufs.push([]);
        }
        const agentPose = pool.poses[poseCount];
        const jointBufs = pool.jointBufs[poseCount];
        const segmentPoses = agentPose.segments;
        while (segmentPoses.length < validCount) {
          segmentPoses.push(createEmptySegPose());
          jointBufs.push([0, 0, 0]);
        }
        if (segmentPoses.length !== validCount) segmentPoses.length = validCount;

        let boundingRadius = Math.max(
          rootRenderLength * 0.5 + rootRenderRadius,
          rootRenderRadius * 2,
          1.5,
        );
        for (let i = 0; i < validCount; i++) {
          const s = validSegs[i];
          const segLength = segDim(s.length, false, bodyScale);
          const segRadius = segDim(s.radius, true, bodyScale);

          const rx = rootRx + (s.x - root.x) * bodyScale;
          const ry = rootRy + (finiteOr(s.y, rootY) - rootY) * bodyScale;
          const rz = rootRz + (s.z - root.z) * bodyScale;

          eulerYXZToQuaternion(s.yaw, s.pitch, s.roll, quat, euler);
          posVec.set(rx, ry, rz);
          scaleVec.set(segRadius, segRadius, segLength);
          matrix.compose(posVec, quat, scaleVec);

          const isRootSeg = s === root;
          const segReach =
            Math.hypot(rx - rootRx, ry - rootRy, rz - rootRz) + segLength * 0.5 + segRadius;
          if (segReach > boundingRadius) boundingRadius = segReach;

          const segPose = segmentPoses[i];
          if (!isRootSeg) {
            halfBackVec.set(0, 0, segLength * 0.5).applyQuaternion(quat);
            const jb = jointBufs[i];
            set3(jb, rx - halfBackVec.x, ry - halfBackVec.y, rz - halfBackVec.z);
            segPose.jointRenderPosition = jb;
            let parentRadius = rootRenderRadius;
            if (s.parent_segment_id !== null && s.parent_segment_id !== undefined) {
              for (let j = validCount - 1; j >= 0; j--) {
                if (validSegs[j].segment_id === s.parent_segment_id) {
                  parentRadius = segDim(validSegs[j].radius, true, bodyScale);
                  break;
                }
              }
            }
            segPose.jointRenderRadius = Math.min(parentRadius, segRadius) * 0.92;
            set3(
              segPose.jointAnchor,
              finiteOr(s.joint_anchor_x) * bodyScale,
              finiteOr(s.joint_anchor_y) * bodyScale,
              finiteOr(s.joint_anchor_z) * bodyScale,
            );
          } else {
            segPose.jointRenderPosition = null;
            segPose.jointRenderRadius = 0;
            set3(segPose.jointAnchor, 0, 0, 0);
          }

          segPose.segmentId = s.segment_id;
          segPose.parentSegmentId = s.parent_segment_id ?? null;
          segPose.isRoot = isRootSeg;
          set3(segPose.renderPosition, rx, ry, rz);
          set4(segPose.quaternion, quat.x, quat.y, quat.z, quat.w);
          segPose.renderLength = segLength;
          segPose.renderRadius = segRadius;
          const me = matrix.elements;
          const dstM = segPose.matrixElements;
          for (let m = 0; m < 16; m++) dstM[m] = me[m];

          sortedSegs[i] = segPose;
        }

        clearSlots(validSegs, validCount);

        for (let i = 1; i < validCount; i++) {
          const cur = sortedSegs[i];
          let j = i - 1;
          while (j >= 0) {
            const prev = sortedSegs[j];
            if ((prev.isRoot === cur.isRoot ? prev.segmentId - cur.segmentId : prev.isRoot ? -1 : 1) <= 0) {
              break;
            }
            sortedSegs[j + 1] = prev;
            j--;
          }
          sortedSegs[j + 1] = cur;
        }

        let mc = morphCache.get(agentId);
        let same = mc !== undefined && mc[6] === validCount;
        if (!mc) {
          mc = [stamp, '', 0, 0, 0, 1, 0, []];
          morphCache.set(agentId, mc);
          same = false;
        }
        const sig = mc[7];
        for (let i = 0; i < validCount; i++) {
          const sp = sortedSegs[i];
          if (!syncMorph(sig, i * 8, sp, sp.jointAnchor)) same = false;
        }

        mc[0] = stamp;
        if (!same) {
          if (keyParts.length !== validCount) keyParts.length = validCount;
          for (let i = 0; i < validCount; i++) {
            const sp = sortedSegs[i];
            let pSlot = -1;
            if (sp.parentSegmentId !== null) {
              for (let j = validCount - 1; j >= 0; j--) {
                if (sortedSegs[j].segmentId === sp.parentSegmentId) {
                  pSlot = j;
                  break;
                }
              }
            }
            keyParts[i] = `${pSlot}:${Math.round(sp.renderLength * 100)}:${Math.round(sp.renderRadius * 100)}:${Math.round(sp.jointAnchor[0] * 100)}:${Math.round(sp.jointAnchor[1] * 100)}:${Math.round(sp.jointAnchor[2] * 100)}`;
          }
          const { headAttachment } = buildMorphologyRestLayout(segmentPoses);
          const lf = headAttachment.localForward;
          mc[1] = keyParts.join('|');
          mc[2] = headAttachment.segmentId;
          mc[3] = lf[0];
          mc[4] = lf[1];
          mc[5] = lf[2];
          mc[6] = validCount;
        }
        agentPose.morphologyKey = mc[1];
        agentPose.headSegmentId = mc[2];
        set3(agentPose.headLocalForward, mc[3], mc[4], mc[5]);

        clearSlots(sortedSegs, validCount);

        agentPose.agentId = agentId;
        agentPose.agentType = root.agent_type ?? null;
        agentPose.energy = finiteOr(root.energy);
        agentPose.hydration = finiteOr(root.hydration);
        agentPose.segmentCount = validCount;
        agentPose.groundY = groundY;
        set3(agentPose.rootPosition, rootRx, rootRy, rootRz);
        set4(agentPose.rootQuaternion, rootQuat.x, rootQuat.y, rootQuat.z, rootQuat.w);
        agentPose.rootRenderLength = rootRenderLength;
        agentPose.rootRenderRadius = rootRenderRadius;
        agentPose.boundingRadius = boundingRadius;
        set3(agentPose.headDirection, headVec.x, headVec.y, headVec.z);

        outPoses[poseCount++] = agentPose;
      }

      if (outPoses.length !== poseCount) outPoses.length = poseCount;
      dropStale(grouped, groupCount, stamp);
      dropStale(morphCache, poseCount, stamp);

      return outPoses;
    },
  };
}

/**
 * Pooled multi-segment pose builder (`FE-MEM-2`): writes grounded poses into `outPoses` reusing
 * `pool` slots and `scratch` working buffers so steady-state ticks allocate zero JS objects/arrays
 * while matching `buildAgentPoses` output value for value.
 */
export function buildAgentPosesInto(
  segments: readonly SegmentState[],
  renderSize: number,
  sampleGroundY: GroundHeightSampler,
  outPoses: AgentRenderPose[],
  pool: AgentPosePool,
  scratch: AgentPoseScratch,
  headDirections?: Readonly<Record<number, [number, number, number] | undefined>>,
  bodyScaleFactor = 1,
): AgentRenderPose[] {
  return scratch.run(
    segments,
    renderSize,
    sampleGroundY,
    outPoses,
    pool,
    headDirections,
    bodyScaleFactor,
  );
}

/**
 * Pure multi-segment pose builder (`RW2-1`):
 * Groups flat segments by `agent_id`, grounds the lowest point of each creature on `surfaceHeight`,
 * places all segments using exact relative `(seg - root) * simScale` offsets, sizes cylinder
 * matrices along local `Z` by `(radius, radius, length)`, and computes rear joint anchors.
 */
export function buildAgentPoses(
  segments: readonly SegmentState[],
  renderSize: number,
  sampleGroundY: GroundHeightSampler,
  headDirections?: Readonly<Record<number, [number, number, number] | undefined>>,
  bodyScaleFactor = 1,
): AgentRenderPose[] {
  return buildAgentPosesInto(
    segments,
    renderSize,
    sampleGroundY,
    [],
    createAgentPosePool(),
    createAgentPoseScratch(),
    headDirections,
    bodyScaleFactor,
  );
}

/** Distance beyond which a creature simplifies to a billboarded flat overview marker (RW2-6). */
export function isBeyondFarMarkerDistance(
  cameraPos: { x: number; y: number; z: number },
  rootPos: readonly [number, number, number],
  renderSize: number,
): boolean {
  const dx = cameraPos.x - rootPos[0];
  const dy = cameraPos.y - rootPos[1];
  const dz = cameraPos.z - rootPos[2];
  const distSq = dx * dx + dy * dy + dz * dz;
  const threshold = renderSize * FAR_MARKER_DISTANCE_FACTOR;
  return distSq > threshold * threshold;
}

/**
 * Compute world-space square size for a flat camera-facing marker so it spans at least
 * `minPixels` vertically on screen at `distance` (`>= 6 px` for countable overview dots, RW2-6).
 */
export function computeFarMarkerScale(
  distance: number,
  viewportHeightPx: number,
  fovDeg: number,
  minPixels = 7.5,
): number {
  const height = Number.isFinite(viewportHeightPx) && viewportHeightPx > 0 ? viewportHeightPx : 768;
  const fovRad = ((Number.isFinite(fovDeg) && fovDeg > 0 ? fovDeg : 55) * Math.PI) / 180;
  const worldHeight = 2 * Math.max(1, finiteOr(distance, 1)) * Math.tan(fovRad * 0.5);
  const worldPerPixel = worldHeight / height;
  const targetPx = Math.max(6.5, minPixels);
  return Math.min(18, Math.max(4, worldPerPixel * targetPx));
}
