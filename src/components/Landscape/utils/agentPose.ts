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
  for (let i = 0; i < ordered.length; i++) {
    indexBySegId.set(ordered[i].segmentId, i);
  }

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

  // Resolve parent positions before children regardless of segment_id ordering.
  const placed = new Uint8Array(nodes.length);
  placed[0] = 1;
  let progress = true;
  while (progress) {
    progress = false;
    for (let i = 1; i < nodes.length; i++) {
      if (placed[i]) continue;
      const node = nodes[i];
      const pIdx = node.parentBoneIdx ?? 0;
      if (!placed[pIdx] && i !== pIdx) continue;
      const parent = nodes[pIdx === i ? 0 : pIdx];
      const rawA = ordered[i].jointAnchor;
      const ax = Number.isFinite(rawA[0]) ? rawA[0] : 0;
      const ay = Number.isFinite(rawA[1]) ? rawA[1] : 0;
      const az = Number.isFinite(rawA[2]) ? rawA[2] : 0;
      const mag = Math.hypot(ax, ay, az);

      // Exact Rust kinematics equilibrium (`src-tauri/src/evolution/genotype.rs:194-199,266` &
      // `src-tauri/src/physics/dynamics.rs:80-82`):
      // P_child = P_parent + anchor_offset + (0, 0, +child.length / 2).
      if (mag <= 1e-5) {
        const jz = parent.restPos[2] + parent.length * 0.5;
        node.jointPos = [parent.restPos[0], parent.restPos[1], jz];
        node.restPos = [parent.restPos[0], parent.restPos[1], jz + node.length * 0.5];
      } else {
        const jx = parent.restPos[0] + ax;
        const jy = parent.restPos[1] + ay;
        const jz = parent.restPos[2] + az;
        node.jointPos = [jx, jy, jz];
        node.restPos = [jx, jy, jz + node.length * 0.5];
      }
      placed[i] = 1;
      progress = true;
    }
  }

  for (let i = 1; i < nodes.length; i++) {
    if (!placed[i]) {
      const prev = nodes[i - 1];
      const jz = prev.restPos[2] + prev.length * 0.5;
      nodes[i].jointPos = [prev.restPos[0], prev.restPos[1], jz];
      nodes[i].restPos = [prev.restPos[0], prev.restPos[1], jz + nodes[i].length * 0.5];
    }
  }

  // Primary spine chain starting at root along non-lateral first children.
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
    let best = kids[0];
    let bestLateral = Math.abs(best.jointPos[0] - cursor.restPos[0]);
    for (let i = 1; i < kids.length; i++) {
      const lat = Math.abs(kids[i].jointPos[0] - cursor.restPos[0]);
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
  let headAttachment: MorphologyHeadAttachment;
  let usesRootAxialCap = false;

  if (spineChain.length === 1) {
    headAttachment = {
      boneIdx: firstNode.boneIdx,
      segmentId: firstNode.segmentId,
      frontShoulderDist: firstNode.length * 0.5,
      headRadius: firstNode.radius,
      localForward: [0, 0, 1],
    };
  } else {
    const startDir = normalizeVec3Or(
      spineChain[1].restPos[0] - firstNode.restPos[0],
      spineChain[1].restPos[1] - firstNode.restPos[1],
      spineChain[1].restPos[2] - firstNode.restPos[2],
      0,
      0,
      1,
    );
    const endDir = normalizeVec3Or(
      lastNode.restPos[0] - spineChain[spineChain.length - 2].restPos[0],
      lastNode.restPos[1] - spineChain[spineChain.length - 2].restPos[1],
      lastNode.restPos[2] - spineChain[spineChain.length - 2].restPos[2],
      0,
      0,
      1,
    );

    if (startDir[2] < -0.7) {
      headAttachment = {
        boneIdx: firstNode.boneIdx,
        segmentId: firstNode.segmentId,
        frontShoulderDist: firstNode.length * 0.5,
        headRadius: firstNode.radius,
        localForward: [-startDir[0], -startDir[1], -startDir[2]],
      };
    } else if (Math.abs(startDir[2]) <= 0.7) {
      usesRootAxialCap = true;
      headAttachment = {
        boneIdx: firstNode.boneIdx,
        segmentId: firstNode.segmentId,
        frontShoulderDist: firstNode.length * 0.5,
        headRadius: firstNode.radius,
        localForward: [0, 0, 1],
      };
    } else {
      headAttachment = {
        boneIdx: lastNode.boneIdx,
        segmentId: lastNode.segmentId,
        frontShoulderDist: lastNode.length * 0.5,
        headRadius: lastNode.radius,
        localForward: [endDir[0], endDir[1], endDir[2]],
      };
    }
  }

  return {
    nodes,
    spineChain,
    inSpine,
    headAttachment,
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

function positiveDimOr(value: number, fallback: number, min: number): number {
  if (!Number.isFinite(value) || value <= 0) return fallback;
  return Math.max(min, value);
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
  const mapped = headDirections?.[root.agent_id];
  if (
    Array.isArray(mapped) &&
    mapped.length >= 3 &&
    Number.isFinite(mapped[0]) &&
    Number.isFinite(mapped[1]) &&
    Number.isFinite(mapped[2])
  ) {
    out.set(mapped[0], mapped[1], mapped[2]);
    if (out.lengthSq() > 1e-8) return out.normalize();
  }

  const embedded = root.head_direction;
  if (
    Array.isArray(embedded) &&
    embedded.length >= 3 &&
    Number.isFinite(embedded[0]) &&
    Number.isFinite(embedded[1]) &&
    Number.isFinite(embedded[2])
  ) {
    out.set(embedded[0], embedded[1], embedded[2]);
    if (out.lengthSq() > 1e-8) return out.normalize();
  }

  out.copy(LOCAL_FORWARD).applyQuaternion(rootQuat);
  if (out.lengthSq() > 1e-8) return out.normalize();
  return out.set(0, 0, 1);
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
  if (!Array.isArray(segments) || segments.length === 0) return [];
  const scale = simScale(renderSize);
  if (!Number.isFinite(scale) || scale <= 0) return [];
  const safeBodyFactor =
    Number.isFinite(bodyScaleFactor) && bodyScaleFactor > 0 ? bodyScaleFactor : 1;
  const bodyScale = scale * safeBodyFactor;

  const grouped = new Map<number, SegmentState[]>();
  for (let i = 0; i < segments.length; i++) {
    const seg = segments[i];
    if (!seg || typeof seg.agent_id !== 'number' || !Number.isFinite(seg.agent_id)) continue;
    let bucket = grouped.get(seg.agent_id);
    if (!bucket) {
      bucket = [];
      grouped.set(seg.agent_id, bucket);
    }
    bucket.push(seg);
  }

  const euler = new THREE.Euler(0, 0, 0, 'YXZ');
  const quat = new THREE.Quaternion();
  const rootQuat = new THREE.Quaternion();
  const posVec = new THREE.Vector3();
  const scaleVec = new THREE.Vector3();
  const halfBackVec = new THREE.Vector3();
  const headVec = new THREE.Vector3();
  const matrix = new THREE.Matrix4();

  const poses: AgentRenderPose[] = [];
  const childExtentBound = SIM_BOUNDS_HALF * 1.5;

  grouped.forEach((rawList, agentId) => {
    let root = rawList[0];
    for (let i = 0; i < rawList.length; i++) {
      const candidate = rawList[i];
      if (candidate.parent_segment_id === null || candidate.parent_segment_id === undefined) {
        root = candidate;
        break;
      }
    }

    if (!isInsideSimBounds(root.x, root.z)) return;

    const validSegs: SegmentState[] = [];
    for (let i = 0; i < rawList.length; i++) {
      const s = rawList[i];
      if (
        !Number.isFinite(s.x) ||
        !Number.isFinite(s.z) ||
        Math.abs(s.x) > childExtentBound ||
        Math.abs(s.z) > childExtentBound
      ) {
        continue;
      }
      validSegs.push(s);
    }
    if (validSegs.length === 0) return;

    const rootRx = simToRender(root.x, renderSize);
    const rootRz = simToRender(root.z, renderSize);
    const sampledGroundY = sampleGroundY(rootRx, rootRz);
    const groundY = Number.isFinite(sampledGroundY) ? sampledGroundY : 0;

    const rootY = finiteOr(root.y);
    const rootRenderLength =
      positiveDimOr(root.length, DEFAULT_FALLBACK_LENGTH, MIN_SIM_SEGMENT_LENGTH) * bodyScale;
    const rootRenderRadius =
      positiveDimOr(root.radius, DEFAULT_FALLBACK_RADIUS, MIN_SIM_SEGMENT_RADIUS) * bodyScale;

    // Ground the lowest segment sphere/body bottom onto terrain surfaceHeight while preserving exact
    // scaled intra-creature vertical differences `(seg.y - root.y) * bodyScale`.
    let minBottomOffset = -rootRenderRadius;
    const segById = new Map<number, SegmentState>();
    for (let i = 0; i < validSegs.length; i++) {
      const s = validSegs[i];
      segById.set(s.segment_id, s);
      const r = positiveDimOr(s.radius, DEFAULT_FALLBACK_RADIUS, MIN_SIM_SEGMENT_RADIUS) * bodyScale;
      const bottomOffset = (finiteOr(s.y, rootY) - rootY) * bodyScale - r;
      if (bottomOffset < minBottomOffset) minBottomOffset = bottomOffset;
    }
    const rootRy = groundY - minBottomOffset;

    eulerYXZToQuaternion(root.yaw, root.pitch, root.roll, rootQuat, euler);
    resolveHeadDirectionVector(root, rootQuat, headDirections, headVec);

    const segmentPoses: SegmentRenderPose[] = [];
    let boundingRadius = Math.max(rootRenderLength * 0.5 + rootRenderRadius, rootRenderRadius * 2, 1.5);
    for (let i = 0; i < validSegs.length; i++) {
      const s = validSegs[i];
      const segLength =
        positiveDimOr(s.length, DEFAULT_FALLBACK_LENGTH, MIN_SIM_SEGMENT_LENGTH) * bodyScale;
      const segRadius =
        positiveDimOr(s.radius, DEFAULT_FALLBACK_RADIUS, MIN_SIM_SEGMENT_RADIUS) * bodyScale;

      const rx = rootRx + (s.x - root.x) * bodyScale;
      const ry = rootRy + (finiteOr(s.y, rootY) - rootY) * bodyScale;
      const rz = rootRz + (s.z - root.z) * bodyScale;

      eulerYXZToQuaternion(s.yaw, s.pitch, s.roll, quat, euler);
      posVec.set(rx, ry, rz);
      scaleVec.set(segRadius, segRadius, segLength);
      matrix.compose(posVec, quat, scaleVec);

      const isRootSeg = s === root;
      const anchorX = isRootSeg ? 0 : finiteOr(s.joint_anchor_x) * bodyScale;
      const anchorY = isRootSeg ? 0 : finiteOr(s.joint_anchor_y) * bodyScale;
      const anchorZ = isRootSeg ? 0 : finiteOr(s.joint_anchor_z) * bodyScale;

      const segReach = Math.hypot(rx - rootRx, ry - rootRy, rz - rootRz) + segLength * 0.5 + segRadius;
      if (segReach > boundingRadius) boundingRadius = segReach;

      let jointRenderPosition: [number, number, number] | null = null;
      let jointRenderRadius = 0;
      if (!isRootSeg) {
        halfBackVec.set(0, 0, segLength * 0.5).applyQuaternion(quat);
        jointRenderPosition = [
          rx - halfBackVec.x,
          ry - halfBackVec.y,
          rz - halfBackVec.z,
        ];
        const parent =
          s.parent_segment_id !== null && s.parent_segment_id !== undefined
            ? segById.get(s.parent_segment_id)
            : undefined;
        const parentRadius = parent
          ? positiveDimOr(parent.radius, DEFAULT_FALLBACK_RADIUS, MIN_SIM_SEGMENT_RADIUS) * bodyScale
          : rootRenderRadius;
        jointRenderRadius = Math.min(parentRadius, segRadius) * 0.92;
      }

      segmentPoses.push({
        segmentId: s.segment_id,
        parentSegmentId: s.parent_segment_id ?? null,
        isRoot: isRootSeg,
        renderPosition: [rx, ry, rz],
        quaternion: [quat.x, quat.y, quat.z, quat.w],
        renderLength: segLength,
        renderRadius: segRadius,
        jointAnchor: [anchorX, anchorY, anchorZ],
        matrixElements: Array.from(matrix.elements),
        jointRenderPosition,
        jointRenderRadius,
      });
    }

    const sortedSegs = segmentPoses
      .slice()
      .sort((a, b) => (a.isRoot === b.isRoot ? a.segmentId - b.segmentId : a.isRoot ? -1 : 1));
    const slotBySegId = new Map<number, number>();
    for (let i = 0; i < sortedSegs.length; i++) {
      slotBySegId.set(sortedSegs[i].segmentId, i);
    }
    const keyParts: string[] = new Array(sortedSegs.length);
    for (let i = 0; i < sortedSegs.length; i++) {
      const sp = sortedSegs[i];
      const pSlot =
        sp.parentSegmentId !== null ? (slotBySegId.get(sp.parentSegmentId) ?? -1) : -1;
      keyParts[i] = `${pSlot}:${Math.round(sp.renderLength * 100)}:${Math.round(sp.renderRadius * 100)}:${Math.round(sp.jointAnchor[0] * 100)}:${Math.round(sp.jointAnchor[1] * 100)}:${Math.round(sp.jointAnchor[2] * 100)}`;
    }
    const morphologyKey = keyParts.join('|');
    const restLayout = buildMorphologyRestLayout(segmentPoses);

    poses.push({
      agentId,
      agentType: root.agent_type ?? null,
      energy: finiteOr(root.energy),
      hydration: finiteOr(root.hydration),
      segmentCount: validSegs.length,
      groundY,
      rootPosition: [rootRx, rootRy, rootRz],
      rootQuaternion: [rootQuat.x, rootQuat.y, rootQuat.z, rootQuat.w],
      rootRenderLength,
      rootRenderRadius,
      boundingRadius,
      morphologyKey,
      headSegmentId: restLayout.headAttachment.segmentId,
      headLocalForward: [
        restLayout.headAttachment.localForward[0],
        restLayout.headAttachment.localForward[1],
        restLayout.headAttachment.localForward[2],
      ],
      headDirection: [headVec.x, headVec.y, headVec.z],
      segments: segmentPoses,
    });
  });

  return poses;
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
