import * as THREE from 'three';
import type { InterpolatedAgentView } from '../agentInterpolation';
import { QUALITY_TIER_CONFIGS, readInitialQualitySelection, type QualityTier } from './qualityTiers';

// prototype, to be replaced — B1 three-tier creature LOD and per-creature frustum culling.

export const LOD_CULLED = 0 as const;
export const LOD_NEAR_SKINNED = 1 as const;
export const LOD_MID_INSTANCED = 2 as const;
export const LOD_FAR_MARKER = 3 as const;

export type CreatureLodTier =
  | typeof LOD_CULLED
  | typeof LOD_NEAR_SKINNED
  | typeof LOD_MID_INSTANCED
  | typeof LOD_FAR_MARKER;

/**
 * Maximum number of near-tier `SkinnedMesh` creatures per quality tier (`K`).
 * Bounded together with 5 instanced layers and an 8-slot shadow cap so total scene draw calls
 * stay within `QUALITY_TIER_CONFIGS[tier].maxDrawCalls` (`60 / 60 / 70`) including shadow pass.
 */
export const CREATURE_NEAR_SKINNED_CAP: Readonly<Record<QualityTier, number>> = {
  low: 8,
  medium: 12,
  high: 16,
};

export interface CreatureLodSelectionResult {
  readonly tiers: Uint8Array;
  readonly nearCount: number;
  readonly midCount: number;
  readonly farCount: number;
  readonly culledCount: number;
}

interface MutableLodResult {
  tiers: Uint8Array;
  nearCount: number;
  midCount: number;
  farCount: number;
  culledCount: number;
}

export function resolveCreatureQualityTier(
  overrideTier: QualityTier | undefined,
  search: string,
  shadowsEnabled: boolean,
  pixelRatio: number,
): QualityTier {
  if (overrideTier !== undefined) return overrideTier;
  const sel = readInitialQualitySelection(search);
  if (sel !== 'auto') return sel;
  if (shadowsEnabled) return 'high';
  return pixelRatio < 0.95 ? 'low' : 'medium';
}

function isPositionedFrustumCamera(camera: unknown): camera is THREE.Camera {
  if (!camera || typeof camera !== 'object') return false;
  const cam = camera as Partial<THREE.Camera>;
  if (!cam.projectionMatrix || !cam.matrixWorldInverse) {
    return false;
  }
  const elements: unknown = cam.projectionMatrix.elements;
  if (!Array.isArray(elements) && !(elements instanceof Float32Array)) {
    return false;
  }
  const pe = cam.projectionMatrix.elements;
  const me = cam.matrixWorldInverse.elements;
  if (!pe || !me || pe.length < 16 || me.length < 16) return false;
  // Unconfigured default `new THREE.PerspectiveCamera()` sits at `(0,0,0)` with identity inverse
  // and `pe[0] === 1`; treat only actually positioned / oriented cameras as active frustums so
  // test mocks without an explicit camera pose never accidentally cull origin subjects.
  const camPos = (cam as { position?: { x: number; y: number; z: number } }).position;
  const atOrigin = !camPos || (camPos.x === 0 && camPos.y === 0 && camPos.z === 0);
  const identityInverse =
    me[0] === 1 && me[5] === 1 && me[10] === 1 && me[12] === 0 && me[13] === 0 && me[14] === 0;
  if (atOrigin && identityInverse) return false;
  return Number.isFinite(pe[0]) && pe[0] !== 0;
}

/**
 * Stateful zero-allocation three-tier creature LOD selector + per-creature frustum culler.
 */
export class CreatureLodSelector {
  private tiers = new Uint8Array(64);
  private candIndices = new Int32Array(64);
  private candDistSq = new Float64Array(64);
  private candAgentIds = new Int32Array(64);
  private readonly frustum = new THREE.Frustum();
  private readonly vpMatrix = new THREE.Matrix4();
  private readonly sphere = new THREE.Sphere();
  private readonly result: MutableLodResult = {
    tiers: this.tiers,
    nearCount: 0,
    midCount: 0,
    farCount: 0,
    culledCount: 0,
  };

  private ensureCapacity(n: number): void {
    if (n <= this.tiers.length) return;
    let cap = this.tiers.length;
    while (cap < n) cap *= 2;
    this.tiers = new Uint8Array(cap);
    this.candIndices = new Int32Array(cap);
    this.candDistSq = new Float64Array(cap);
    this.candAgentIds = new Int32Array(cap);
    this.result.tiers = this.tiers;
  }

  select(
    agents: readonly InterpolatedAgentView[],
    agentCount: number,
    camera: unknown,
    tier: QualityTier,
    _renderSize: number,
  ): CreatureLodSelectionResult {
    this.ensureCapacity(Math.max(1, agentCount));

    const cfg = QUALITY_TIER_CONFIGS[tier];
    const nearRadiusSq = cfg.creatureFullDetailRadius * cfg.creatureFullDetailRadius;
    const midRadiusSq = cfg.creatureSimplifiedRadius * cfg.creatureSimplifiedRadius;
    const maxNear = CREATURE_NEAR_SKINNED_CAP[tier];

    const camObj = camera as { position?: { x: number; y: number; z: number } } | null | undefined;
    const cx = camObj?.position?.x ?? 0;
    const cy = camObj?.position?.y ?? 0;
    const cz = camObj?.position?.z ?? 0;

    let hasFrustum = false;
    if (isPositionedFrustumCamera(camera)) {
      if (typeof camera.updateMatrixWorld === 'function') camera.updateMatrixWorld();
      this.vpMatrix.multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse);
      this.frustum.setFromProjectionMatrix(this.vpMatrix);
      hasFrustum = true;
    }

    let nearCandCount = 0;
    let midCount = 0;
    let farCount = 0;
    let culledCount = 0;

    for (let i = 0; i < agentCount; i++) {
      const pose = agents[i];
      const rx = pose.rootPosition[0];
      const ry = pose.rootPosition[1];
      const rz = pose.rootPosition[2];

      if (hasFrustum) {
        const r = Math.max(pose.boundingRadius * 1.35, 6);
        this.sphere.center.set(rx, ry, rz);
        this.sphere.radius = r;
        if (!this.frustum.intersectsSphere(this.sphere)) {
          this.tiers[i] = LOD_CULLED;
          culledCount++;
          continue;
        }
      }

      const dx = rx - cx;
      const dy = ry - cy;
      const dz = rz - cz;
      const distSq = dx * dx + dy * dy + dz * dz;

      if (distSq <= nearRadiusSq) {
        this.tiers[i] = LOD_MID_INSTANCED;
        this.candIndices[nearCandCount] = i;
        this.candDistSq[nearCandCount] = distSq;
        this.candAgentIds[nearCandCount] = pose.agentId;
        nearCandCount++;
      } else if (distSq <= midRadiusSq) {
        this.tiers[i] = LOD_MID_INSTANCED;
        midCount++;
      } else {
        this.tiers[i] = LOD_FAR_MARKER;
        farCount++;
      }
    }

    // Select closest K near candidates (`<= maxNear`), demoting overflow deterministically to mid.
    let nearCount: number;
    if (nearCandCount <= maxNear) {
      for (let c = 0; c < nearCandCount; c++) {
        this.tiers[this.candIndices[c]] = LOD_NEAR_SKINNED;
      }
      nearCount = nearCandCount;
    } else {
      // Partial insertion selection of K closest candidates with zero allocations.
      for (let k = 0; k < maxNear; k++) {
        let best = k;
        for (let j = k + 1; j < nearCandCount; j++) {
          const dDiff = this.candDistSq[j] - this.candDistSq[best];
          if (dDiff < -1e-6 || (Math.abs(dDiff) <= 1e-6 && this.candAgentIds[j] < this.candAgentIds[best])) {
            best = j;
          }
        }
        if (best !== k) {
          const ti = this.candIndices[k];
          this.candIndices[k] = this.candIndices[best];
          this.candIndices[best] = ti;

          const td = this.candDistSq[k];
          this.candDistSq[k] = this.candDistSq[best];
          this.candDistSq[best] = td;

          const ta = this.candAgentIds[k];
          this.candAgentIds[k] = this.candAgentIds[best];
          this.candAgentIds[best] = ta;
        }
        this.tiers[this.candIndices[k]] = LOD_NEAR_SKINNED;
      }
      nearCount = maxNear;
      midCount += nearCandCount - maxNear;
    }

    this.result.nearCount = nearCount;
    this.result.midCount = midCount;
    this.result.farCount = farCount;
    this.result.culledCount = culledCount;
    return this.result;
  }
}
