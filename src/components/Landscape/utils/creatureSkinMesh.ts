import * as THREE from 'three';
import { computeCreatureColor, hashAgentId01 } from './agentPose';
import type { InterpolatedAgentView, InterpolatedSegmentView } from '../agentInterpolation';

// prototype, to be replaced — B1 continuous skinned creature bodies built from segment morphology.

export const SKIN_RADIAL_SIDES = 8;
export const SKIN_BRIDGE_RINGS = 3;
export const SKIN_CAP_RINGS = 2;
export const MAX_ARCHETYPE_CACHE = 128;

export interface MorphologySegmentInput {
  segmentId: number;
  parentSegmentId: number | null;
  isRoot: boolean;
  renderLength: number;
  renderRadius: number;
  jointAnchor: readonly [number, number, number];
}

export interface BuiltArchetype {
  readonly key: string;
  readonly geometry: THREE.BufferGeometry;
  readonly boneCount: number;
  readonly boneSegmentIds: readonly number[];
  readonly restPositions: ReadonlyArray<readonly [number, number, number]>;
  readonly boneInverses: readonly THREE.Matrix4[];
  readonly triangleCount: number;
}

interface SkinNode {
  boneIdx: number;
  segmentId: number;
  parentBoneIdx: number | null;
  length: number;
  radius: number;
  restPos: [number, number, number];
  jointPos: [number, number, number];
}

interface LoftRing {
  center: [number, number, number];
  radius: number;
  tangent: [number, number, number];
}

function normalizeOr(vx: number, vy: number, vz: number, fx: number, fy: number, fz: number): [number, number, number] {
  const len = Math.hypot(vx, vy, vz);
  if (len > 1e-6) return [vx / len, vy / len, vz / len];
  return [fx, fy, fz];
}

function buildRestNodes(segments: readonly MorphologySegmentInput[]): SkinNode[] {
  const ordered = segments
    .slice()
    .sort((a, b) => (a.isRoot === b.isRoot ? a.segmentId - b.segmentId : a.isRoot ? -1 : 1));

  const indexBySegId = new Map<number, number>();
  for (let i = 0; i < ordered.length; i++) {
    indexBySegId.set(ordered[i].segmentId, i);
  }

  const nodes: SkinNode[] = ordered.map((s, boneIdx) => ({
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

  return nodes;
}

function appendClosedTube(
  pathRings: readonly LoftRing[],
  frontPole: readonly [number, number, number],
  rearPole: readonly [number, number, number],
  sides: number,
  positions: number[],
  indices: number[],
): void {
  if (pathRings.length === 0) return;

  const baseVertex = positions.length / 3;
  // Front pole vertex.
  positions.push(frontPole[0], frontPole[1], frontPole[2]);

  const prevRight = new THREE.Vector3(1, 0, 0);
  const tVec = new THREE.Vector3();
  const upVec = new THREE.Vector3();
  const refVec = new THREE.Vector3();

  for (let r = 0; r < pathRings.length; r++) {
    const ring = pathRings[r];
    tVec.set(ring.tangent[0], ring.tangent[1], ring.tangent[2]);
    if (tVec.lengthSq() <= 1e-8) tVec.set(0, 0, 1);
    else tVec.normalize();

    // Parallel-transport frame along the loft path to prevent ring twisting.
    refVec.copy(prevRight).addScaledVector(tVec, -prevRight.dot(tVec));
    if (refVec.lengthSq() <= 1e-6) {
      refVec.set(0, 1, 0).addScaledVector(tVec, -tVec.y);
      if (refVec.lengthSq() <= 1e-6) refVec.set(1, 0, 0);
    }
    refVec.normalize();
    prevRight.copy(refVec);
    upVec.crossVectors(tVec, refVec).normalize();

    for (let s = 0; s < sides; s++) {
      const a = (s / sides) * Math.PI * 2;
      const cosA = Math.cos(a);
      const sinA = Math.sin(a);
      positions.push(
        ring.center[0] + (refVec.x * cosA + upVec.x * sinA) * ring.radius,
        ring.center[1] + (refVec.y * cosA + upVec.y * sinA) * ring.radius,
        ring.center[2] + (refVec.z * cosA + upVec.z * sinA) * ring.radius,
      );
    }
  }

  // Rear pole vertex.
  const rearPoleIdx = positions.length / 3;
  positions.push(rearPole[0], rearPole[1], rearPole[2]);

  // Front cap fan (watertight single pole shared by `sides` triangles).
  const firstRingStart = baseVertex + 1;
  for (let s = 0; s < sides; s++) {
    const sNext = (s + 1) % sides;
    indices.push(baseVertex, firstRingStart + sNext, firstRingStart + s);
  }

  // Tube quad strip between consecutive rings.
  for (let r = 0; r < pathRings.length - 1; r++) {
    const r0 = firstRingStart + r * sides;
    const r1 = r0 + sides;
    for (let s = 0; s < sides; s++) {
      const sNext = (s + 1) % sides;
      const a = r0 + s;
      const b = r0 + sNext;
      const c = r1 + s;
      const d = r1 + sNext;
      indices.push(a, b, d, a, d, c);
    }
  }

  // Rear cap fan.
  const lastRingStart = firstRingStart + (pathRings.length - 1) * sides;
  for (let s = 0; s < sides; s++) {
    const sNext = (s + 1) % sides;
    indices.push(rearPoleIdx, lastRingStart + s, lastRingStart + sNext);
  }
}

function addChainLoft(
  chainNodes: readonly SkinNode[],
  positions: number[],
  indices: number[],
): void {
  const first = chainNodes[0];
  const last = chainNodes[chainNodes.length - 1];
  const rings: LoftRing[] = [];

  const startDir: [number, number, number] =
    chainNodes.length >= 2
      ? normalizeOr(
          chainNodes[1].restPos[0] - first.restPos[0],
          chainNodes[1].restPos[1] - first.restPos[1],
          chainNodes[1].restPos[2] - first.restPos[2],
          0,
          0,
          1,
        )
      : [0, 0, 1];
  const endDir: [number, number, number] =
    chainNodes.length >= 2
      ? normalizeOr(
          last.restPos[0] - chainNodes[chainNodes.length - 2].restPos[0],
          last.restPos[1] - chainNodes[chainNodes.length - 2].restPos[1],
          last.restPos[2] - chainNodes[chainNodes.length - 2].restPos[2],
          0,
          0,
          1,
        )
      : [0, 0, 1];

  const frontCapRadius = first.radius * 0.92;
  const startHalf = first.length * 0.5;
  const startShoulder: [number, number, number] = [
    first.restPos[0] - startDir[0] * startHalf,
    first.restPos[1] - startDir[1] * startHalf,
    first.restPos[2] - startDir[2] * startHalf,
  ];
  const frontPole: [number, number, number] = [
    startShoulder[0] - startDir[0] * frontCapRadius * 0.72,
    startShoulder[1] - startDir[1] * frontCapRadius * 0.72,
    startShoulder[2] - startDir[2] * frontCapRadius * 0.72,
  ];

  // Hemispherical start cap stepping from frontPole toward startShoulder along +startDir.
  for (let k = SKIN_CAP_RINGS - 1; k >= 1; k--) {
    const frac = k / SKIN_CAP_RINGS;
    const theta = frac * Math.PI * 0.5;
    const ext = Math.sin(theta) * frontCapRadius * 0.72;
    rings.push({
      center: [
        startShoulder[0] - startDir[0] * ext,
        startShoulder[1] - startDir[1] * ext,
        startShoulder[2] - startDir[2] * ext,
      ],
      radius: Math.max(first.radius * 0.25, Math.cos(theta) * frontCapRadius),
      tangent: startDir,
    });
  }

  // Start shoulder ring and root center ring.
  rings.push({
    center: startShoulder,
    radius: frontCapRadius,
    tangent: startDir,
  });

  rings.push({
    center: [first.restPos[0], first.restPos[1], first.restPos[2]],
    radius: first.radius,
    tangent: startDir,
  });

  // Walk each link: parent center -> joint bridge -> child center.
  for (let idx = 1; idx < chainNodes.length; idx++) {
    const parent = chainNodes[idx - 1];
    const child = chainNodes[idx];
    const pCenter = parent.restPos;
    const jCenter = child.jointPos;
    const cCenter = child.restPos;
    const tangent = normalizeOr(
      cCenter[0] - pCenter[0],
      cCenter[1] - pCenter[1],
      cCenter[2] - pCenter[2],
      endDir[0],
      endDir[1],
      endDir[2],
    );
    const bridgeRadius = Math.max(0.04, Math.min(parent.radius, child.radius) * 0.78);

    // Foldback guard: if jointPos sits outside (pCenter -> cCenter) (e.g. folded anchor + +Z/2),
    // use midpoint control so the Bezier skin bridge never doubles back over the parent.
    const pjDotJc =
      (jCenter[0] - pCenter[0]) * (cCenter[0] - jCenter[0]) +
      (jCenter[1] - pCenter[1]) * (cCenter[1] - jCenter[1]) +
      (jCenter[2] - pCenter[2]) * (cCenter[2] - jCenter[2]);
    const ctrlX = pjDotJc < 0 ? (pCenter[0] + cCenter[0]) * 0.5 : jCenter[0];
    const ctrlY = pjDotJc < 0 ? (pCenter[1] + cCenter[1]) * 0.5 : jCenter[1];
    const ctrlZ = pjDotJc < 0 ? (pCenter[2] + cCenter[2]) * 0.5 : jCenter[2];

    for (let step = 1; step <= SKIN_BRIDGE_RINGS; step++) {
      const t = step / (SKIN_BRIDGE_RINGS + 1);
      const mt = 1 - t;
      const bx = mt * mt * pCenter[0] + 2 * mt * t * ctrlX + t * t * cCenter[0];
      const by = mt * mt * pCenter[1] + 2 * mt * t * ctrlY + t * t * cCenter[1];
      const bz = mt * mt * pCenter[2] + 2 * mt * t * ctrlZ + t * t * cCenter[2];
      const pinch = Math.sin(t * Math.PI);
      const rBase = parent.radius * mt + child.radius * t;
      const rad = rBase * (1 - 0.22 * pinch) + bridgeRadius * 0.22 * pinch;
      rings.push({
        center: [bx, by, bz],
        radius: Math.max(0.04, rad),
        tangent,
      });
    }

    rings.push({
      center: [cCenter[0], cCenter[1], cCenter[2]],
      radius: child.radius,
      tangent,
    });
  }

  // Rear end cap of chain extending along +endDir.
  const endHalf = last.length * 0.5;
  const endShoulder: [number, number, number] = [
    last.restPos[0] + endDir[0] * endHalf,
    last.restPos[1] + endDir[1] * endHalf,
    last.restPos[2] + endDir[2] * endHalf,
  ];
  const rearCapRadius = last.radius * 0.88;
  rings.push({
    center: endShoulder,
    radius: rearCapRadius,
    tangent: endDir,
  });

  for (let k = 1; k < SKIN_CAP_RINGS; k++) {
    const frac = k / SKIN_CAP_RINGS;
    const theta = frac * Math.PI * 0.5;
    const ext = Math.sin(theta) * rearCapRadius * 0.75;
    rings.push({
      center: [
        endShoulder[0] + endDir[0] * ext,
        endShoulder[1] + endDir[1] * ext,
        endShoulder[2] + endDir[2] * ext,
      ],
      radius: Math.max(last.radius * 0.22, Math.cos(theta) * rearCapRadius),
      tangent: endDir,
    });
  }

  const rearPole: [number, number, number] = [
    endShoulder[0] + endDir[0] * rearCapRadius * 0.75,
    endShoulder[1] + endDir[1] * rearCapRadius * 0.75,
    endShoulder[2] + endDir[2] * rearCapRadius * 0.75,
  ];

  appendClosedTube(rings, frontPole, rearPole, SKIN_RADIAL_SIDES, positions, indices);
}

function addBranchLoft(
  parent: SkinNode,
  branch: SkinNode,
  positions: number[],
  indices: number[],
): void {
  const startPole: [number, number, number] = [
    parent.restPos[0],
    parent.restPos[1],
    parent.restPos[2],
  ];
  const branchDir = normalizeOr(
    branch.restPos[0] - branch.jointPos[0],
    branch.restPos[1] - branch.jointPos[1],
    branch.restPos[2] - branch.jointPos[2],
    0,
    0,
    1,
  );
  const entryTangent = normalizeOr(
    branch.restPos[0] - startPole[0],
    branch.restPos[1] - startPole[1],
    branch.restPos[2] - startPole[2],
    branchDir[0],
    branchDir[1],
    branchDir[2],
  );
  const midPos: [number, number, number] = [
    (branch.jointPos[0] + branch.restPos[0]) * 0.5,
    (branch.jointPos[1] + branch.restPos[1]) * 0.5,
    (branch.jointPos[2] + branch.restPos[2]) * 0.5,
  ];
  const halfLen = branch.length * 0.5;
  const tipPos: [number, number, number] = [
    branch.restPos[0] + branchDir[0] * halfLen,
    branch.restPos[1] + branchDir[1] * halfLen,
    branch.restPos[2] + branchDir[2] * halfLen,
  ];
  const endPole: [number, number, number] = [
    tipPos[0] + branchDir[0] * branch.radius * 0.65,
    tipPos[1] + branchDir[1] * branch.radius * 0.65,
    tipPos[2] + branchDir[2] * branch.radius * 0.65,
  ];

  const rings: LoftRing[] = [
    { center: branch.jointPos, radius: Math.min(parent.radius * 0.55, branch.radius * 0.95), tangent: entryTangent },
    { center: midPos, radius: branch.radius * 0.92, tangent: branchDir },
    { center: branch.restPos, radius: branch.radius, tangent: branchDir },
    { center: tipPos, radius: branch.radius * 0.72, tangent: branchDir },
  ];

  appendClosedTube(rings, startPole, endPole, SKIN_RADIAL_SIDES, positions, indices);
}

function distancePointToSegmentSq(
  px: number,
  py: number,
  pz: number,
  ax: number,
  ay: number,
  az: number,
  bx: number,
  by: number,
  bz: number,
): number {
  const abx = bx - ax;
  const aby = by - ay;
  const abz = bz - az;
  const lenSq = abx * abx + aby * aby + abz * abz;
  if (lenSq <= 1e-10) {
    const dx = px - ax;
    const dy = py - ay;
    const dz = pz - az;
    return dx * dx + dy * dy + dz * dz;
  }
  const t = Math.min(1, Math.max(0, ((px - ax) * abx + (py - ay) * aby + (pz - az) * abz) / lenSq));
  const cx = ax + abx * t;
  const cy = ay + aby * t;
  const cz = az + abz * t;
  const dx = px - cx;
  const dy = py - cy;
  const dz = pz - cz;
  return dx * dx + dy * dy + dz * dz;
}

/**
 * Deterministically build a continuous skinned mesh (one bone per segment, distance-weighted
 * `skinIndex`/`skinWeight` attributes, closed manifold caps) from a creature morphology.
 */
export function buildMorphologySkinArchetype(
  key: string,
  segments: readonly MorphologySegmentInput[],
): BuiltArchetype {
  const safeSegments: readonly MorphologySegmentInput[] =
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

  const nodes = buildRestNodes(safeSegments);

  // Build primary spine chain starting at root along non-lateral first children, and collect lateral/fork branches.
  const childrenByParent = new Map<number, SkinNode[]>();
  for (let i = 1; i < nodes.length; i++) {
    const pIdx = nodes[i].parentBoneIdx ?? 0;
    const list = childrenByParent.get(pIdx);
    if (list) list.push(nodes[i]);
    else childrenByParent.set(pIdx, [nodes[i]]);
  }

  const spineChain: SkinNode[] = [nodes[0]];
  const inSpine = new Uint8Array(nodes.length);
  inSpine[0] = 1;
  let cursor = nodes[0];
  while (true) {
    const kids = childrenByParent.get(cursor.boneIdx);
    if (!kids || kids.length === 0) break;
    // Prefer axial (small |jointPos.x - cursor.x|) child for the main spine spline.
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

  const positions: number[] = [];
  const indices: number[] = [];
  addChainLoft(spineChain, positions, indices);

  for (let i = 1; i < nodes.length; i++) {
    if (inSpine[i]) continue;
    const node = nodes[i];
    const parent = nodes[node.parentBoneIdx ?? 0];
    addBranchLoft(parent, node, positions, indices);
  }

  const vertexCount = positions.length / 3;
  const skinIndices = new Uint16Array(vertexCount * 4);
  const skinWeights = new Float32Array(vertexCount * 4);

  let minRadius = Infinity;
  let totalLen = 0;
  for (let b = 0; b < nodes.length; b++) {
    if (nodes[b].radius < minRadius) minRadius = nodes[b].radius;
    totalLen += nodes[b].length;
  }
  const avgLen = totalLen / nodes.length;
  const sigma = Math.max(minRadius * 1.15, avgLen * 0.35, 0.25);
  const twoSigmaSq = 2 * sigma * sigma;

  for (let v = 0; v < vertexCount; v++) {
    const vx = positions[v * 3];
    const vy = positions[v * 3 + 1];
    const vz = positions[v * 3 + 2];

    let i0 = 0;
    let i1 = 0;
    let i2 = 0;
    let i3 = 0;
    let d0 = Infinity;
    let d1 = Infinity;
    let d2 = Infinity;
    let d3 = Infinity;

    for (let b = 0; b < nodes.length; b++) {
      const bn = nodes[b];
      const halfL = bn.length * 0.45;
      const dSq = distancePointToSegmentSq(
        vx,
        vy,
        vz,
        bn.restPos[0],
        bn.restPos[1],
        bn.restPos[2] - halfL,
        bn.restPos[0],
        bn.restPos[1],
        bn.restPos[2] + halfL,
      );
      if (dSq < d0) {
        d3 = d2; i3 = i2;
        d2 = d1; i2 = i1;
        d1 = d0; i1 = i0;
        d0 = dSq; i0 = b;
      } else if (dSq < d1) {
        d3 = d2; i3 = i2;
        d2 = d1; i2 = i1;
        d1 = dSq; i1 = b;
      } else if (dSq < d2) {
        d3 = d2; i3 = i2;
        d2 = dSq; i2 = b;
      } else if (dSq < d3) {
        d3 = dSq; i3 = b;
      }
    }

    const activeCount = Math.min(4, nodes.length);
    const w0 = Math.exp(-d0 / twoSigmaSq);
    const w1 = activeCount > 1 ? Math.exp(-d1 / twoSigmaSq) : 0;
    const w2 = activeCount > 2 ? Math.exp(-d2 / twoSigmaSq) : 0;
    const w3 = activeCount > 3 ? Math.exp(-d3 / twoSigmaSq) : 0;
    const sum = w0 + w1 + w2 + w3;
    const invSum = sum > 1e-12 ? 1 / sum : 1;

    const base = v * 4;
    skinIndices[base] = i0;
    skinIndices[base + 1] = i1;
    skinIndices[base + 2] = i2;
    skinIndices[base + 3] = i3;
    skinWeights[base] = sum > 1e-12 ? w0 * invSum : 1;
    skinWeights[base + 1] = sum > 1e-12 ? w1 * invSum : 0;
    skinWeights[base + 2] = sum > 1e-12 ? w2 * invSum : 0;
    skinWeights[base + 3] = sum > 1e-12 ? w3 * invSum : 0;
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
  geometry.setAttribute('skinIndex', new THREE.Uint16BufferAttribute(skinIndices, 4));
  geometry.setAttribute('skinWeight', new THREE.Float32BufferAttribute(skinWeights, 4));
  geometry.setIndex(indices);
  geometry.computeVertexNormals();
  geometry.computeBoundingSphere();

  const boneSegmentIds = nodes.map((n) => n.segmentId);
  const restPositions = nodes.map((n): [number, number, number] => [n.restPos[0], n.restPos[1], n.restPos[2]]);
  const boneInverses = nodes.map((n) =>
    new THREE.Matrix4().makeTranslation(-n.restPos[0], -n.restPos[1], -n.restPos[2]),
  );

  return {
    key,
    geometry,
    boneCount: nodes.length,
    boneSegmentIds,
    restPositions,
    boneInverses,
    triangleCount: Math.floor(indices.length / 3),
  };
}

/** Verify indexed mesh edge topology has zero open boundary edges (every undirected edge >= 2 half-edges). */
export function countOpenBoundaryEdges(geometry: THREE.BufferGeometry): number {
  const index = geometry.getIndex();
  if (!index) return -1;
  const edgeCounts = new Map<string, number>();
  const arr = index.array;
  for (let i = 0; i + 2 < arr.length; i += 3) {
    const a = arr[i];
    const b = arr[i + 1];
    const c = arr[i + 2];
    const pairs: ReadonlyArray<readonly [number, number]> = [
      [a, b],
      [b, c],
      [c, a],
    ];
    for (const [u, v] of pairs) {
      const k = u < v ? `${u}:${v}` : `${v}:${u}`;
      edgeCounts.set(k, (edgeCounts.get(k) ?? 0) + 1);
    }
  }
  let open = 0;
  edgeCounts.forEach((count) => {
    if (count === 1) open += 1;
  });
  return open;
}

/** Bounded LRU cache storing skinned BufferGeometry archetypes keyed by morphology signature. */
export class ArchetypeMeshCache {
  private readonly maxEntries: number;
  private readonly entries = new Map<string, BuiltArchetype>();

  constructor(maxEntries = MAX_ARCHETYPE_CACHE) {
    this.maxEntries = Math.max(8, maxEntries);
  }

  size(): number {
    return this.entries.size;
  }

  getOrBuild(key: string, segments: readonly MorphologySegmentInput[]): BuiltArchetype {
    const existing = this.entries.get(key);
    if (existing) {
      this.entries.delete(key);
      this.entries.set(key, existing);
      return existing;
    }
    const built = buildMorphologySkinArchetype(key, segments);
    if (this.entries.size >= this.maxEntries) {
      const oldestKey = this.entries.keys().next().value;
      if (oldestKey !== undefined) {
        const evicted = this.entries.get(oldestKey);
        this.entries.delete(oldestKey);
        evicted?.geometry.dispose();
      }
    }
    this.entries.set(key, built);
    return built;
  }

  disposeAll(): void {
    this.entries.forEach((entry) => entry.geometry.dispose());
    this.entries.clear();
  }
}

interface SkinnedSlot {
  readonly mesh: THREE.SkinnedMesh;
  readonly material: THREE.MeshStandardMaterial;
  archetypeKey: string | null;
  archetype: BuiltArchetype | null;
  bones: THREE.Bone[];
  skeleton: THREE.Skeleton | null;
}

export interface SkinnedUpdateScratch {
  pos: THREE.Vector3;
  quat: THREE.Quaternion;
  scale: THREE.Vector3;
  color: THREE.Color;
}

function findSegmentById(
  segments: readonly InterpolatedSegmentView[],
  segCount: number,
  segmentId: number,
): InterpolatedSegmentView | null {
  for (let i = 0; i < segCount; i++) {
    if (segments[i].segmentId === segmentId) return segments[i];
  }
  return null;
}

/**
 * Reusable pool of near-tier `THREE.SkinnedMesh` instances.
 * Allocates new geometries/skeletons only when an unseen morphology arrives; steady-state frame
 * updates mutate bone `matrixWorld` in place with zero heap allocations.
 */
export class SkinnedCreaturePool {
  readonly cache: ArchetypeMeshCache;
  private readonly slots: SkinnedSlot[] = [];
  private attachedGroup: THREE.Group | null = null;
  private lastBoundMaxSlots = 0;

  constructor(maxSlots = 32, cache = new ArchetypeMeshCache()) {
    this.cache = cache;
    const count = Math.max(1, maxSlots);
    const placeholderGeom = new THREE.BufferGeometry();
    for (let i = 0; i < count; i++) {
      const material = new THREE.MeshStandardMaterial({
        roughness: 0.6,
        metalness: 0.05,
        flatShading: false,
      });
      const mesh = new THREE.SkinnedMesh(placeholderGeom, material);
      mesh.name = `live-agents-skinned-${i}`;
      mesh.frustumCulled = false;
      mesh.castShadow = i < 8;
      mesh.receiveShadow = true;
      mesh.visible = false;
      this.slots.push({
        mesh,
        material,
        archetypeKey: null,
        archetype: null,
        bones: [],
        skeleton: null,
      });
    }
    this.lastBoundMaxSlots = count;
  }

  bindGroup(group: THREE.Group | null): void {
    if (this.attachedGroup === group) return;
    const isThreeObject = (obj: THREE.Group | null): obj is THREE.Group =>
      Boolean(obj && typeof obj === 'object' && 'isObject3D' in obj && obj.isObject3D);
    if (isThreeObject(this.attachedGroup) && typeof this.attachedGroup.remove === 'function') {
      for (let i = 0; i < this.slots.length; i++) {
        this.attachedGroup.remove(this.slots[i].mesh);
      }
    }
    this.attachedGroup = group;
    if (isThreeObject(group) && typeof group.add === 'function') {
      for (let i = 0; i < this.slots.length; i++) {
        group.add(this.slots[i].mesh);
      }
    }
  }

  maxSlots(): number {
    return this.lastBoundMaxSlots;
  }

  updateSlot(
    slotIdx: number,
    pose: InterpolatedAgentView,
    timeSec: number,
    scratch: SkinnedUpdateScratch,
  ): boolean {
    if (slotIdx < 0 || slotIdx >= this.slots.length) return false;
    const slot = this.slots[slotIdx];
    const key = pose.morphologyKey || `fallback-${pose.segmentCount}`;

    if (slot.archetypeKey !== key || slot.archetype === null) {
      const segInputs: MorphologySegmentInput[] = new Array(pose.segmentCount);
      for (let s = 0; s < pose.segmentCount; s++) {
        const seg = pose.segments[s];
        segInputs[s] = {
          segmentId: seg.segmentId,
          parentSegmentId: seg.parentSegmentId,
          isRoot: seg.isRoot,
          renderLength: seg.renderLength,
          renderRadius: seg.renderRadius,
          jointAnchor: seg.jointAnchor,
        };
      }
      const archetype = this.cache.getOrBuild(key, segInputs);
      const bones: THREE.Bone[] = new Array(archetype.boneCount);
      const inverses: THREE.Matrix4[] = new Array(archetype.boneCount);
      for (let b = 0; b < archetype.boneCount; b++) {
        const bone = new THREE.Bone();
        bone.matrixAutoUpdate = false;
        bone.matrixWorldAutoUpdate = false;
        bones[b] = bone;
        inverses[b] = archetype.boneInverses[b].clone();
      }
      slot.skeleton?.dispose();
      const skeleton = new THREE.Skeleton(bones, inverses);
      slot.mesh.geometry = archetype.geometry;
      slot.mesh.bind(skeleton, new THREE.Matrix4());
      slot.bones = bones;
      slot.skeleton = skeleton;
      slot.archetype = archetype;
      slot.archetypeKey = key;
    }

    const archetype = slot.archetype;
    if (!archetype || !slot.skeleton) return false;

    const breathPhase = hashAgentId01(pose.agentId, 3) * Math.PI * 2;
    const breath = 1 + 0.025 * Math.sin(timeSec * 2.4 + breathPhase);

    const { pos, quat, scale, color } = scratch;
    const boneSegIds = archetype.boneSegmentIds;
    for (let b = 0; b < slot.bones.length; b++) {
      const seg =
        findSegmentById(pose.segments, pose.segmentCount, boneSegIds[b]) ?? pose.segments[0];
      const bone = slot.bones[b];
      if (!seg) continue;
      pos.set(seg.renderPosition[0], seg.renderPosition[1], seg.renderPosition[2]);
      quat.set(seg.quaternion[0], seg.quaternion[1], seg.quaternion[2], seg.quaternion[3]);
      scale.set(breath, breath, 1);
      bone.matrixWorld.compose(pos, quat, scale);
      bone.matrix.copy(bone.matrixWorld);
      bone.position.copy(pos);
      bone.quaternion.copy(quat);
    }

    slot.skeleton.update();
    computeCreatureColor(pose.agentType, pose.agentId, pose.energy, color);
    slot.material.color.copy(color);
    slot.mesh.userData.agentId = pose.agentId;
    slot.mesh.visible = true;
    return true;
  }

  hideFrom(startSlot: number): void {
    const from = Math.max(0, startSlot);
    for (let i = from; i < this.slots.length; i++) {
      this.slots[i].mesh.visible = false;
    }
  }

  dispose(): void {
    this.bindGroup(null);
    for (let i = 0; i < this.slots.length; i++) {
      this.slots[i].skeleton?.dispose();
      this.slots[i].material.dispose();
    }
    this.slots.length = 0;
    this.cache.disposeAll();
  }
}
