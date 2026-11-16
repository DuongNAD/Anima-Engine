/**
 * Authoritative quality tier configuration for the 3D Landscape scene (`RW4-1`).
 *
 * Every quality-dependent surface (canvas DPR, shadows, flora density/LOD radii, creature LOD
 * radii, and frame budgets) reads from this table so lanes A, B, and C share one definition.
 */

export const QUALITY_TIERS = ['low', 'medium', 'high'] as const;
export type QualityTier = (typeof QUALITY_TIERS)[number];

export const QUALITY_SELECTIONS = ['auto', 'low', 'medium', 'high'] as const;
export type QualitySelection = (typeof QUALITY_SELECTIONS)[number];

export const QUALITY_SELECTION_LABELS: Readonly<Record<QualitySelection, string>> = {
  auto: 'Tự động',
  low: 'Thấp',
  medium: 'Vừa',
  high: 'Cao',
};

export interface QualityTierConfig {
  readonly tier: QualityTier;
  readonly label: string;
  readonly dprRange: readonly [number, number];
  readonly defaultDpr: number;
  readonly maxDpr: number;
  readonly shadows: boolean;
  readonly shadowMapSize: number;
  readonly contactShadows: boolean;
  readonly waterReflections: boolean;
  readonly waterRefractions: boolean;
  readonly cloudLayers: number;
  readonly vegetationDensity: number;
  readonly floraNearRadius: number;
  readonly floraMidRadius: number;
  readonly creatureFullDetailRadius: number;
  readonly creatureSimplifiedRadius: number;
  readonly maxTriangles: number;
  readonly maxCreatureTriangles: number;
  readonly maxDrawCalls: number;
  readonly targetFps: number;
  readonly targetFrameMs: number;
}

export const QUALITY_TIER_CONFIGS: Readonly<Record<QualityTier, QualityTierConfig>> = {
  low: {
    tier: 'low',
    label: 'Thấp',
    dprRange: [0.75, 1],
    defaultDpr: 1,
    maxDpr: 1,
    shadows: false,
    shadowMapSize: 1024,
    contactShadows: true,
    waterReflections: false,
    waterRefractions: false,
    cloudLayers: 1,
    vegetationDensity: 0.25,
    floraNearRadius: 160,
    floraMidRadius: 280,
    creatureFullDetailRadius: 90,
    creatureSimplifiedRadius: 220,
    maxTriangles: 1_200_000,
    maxCreatureTriangles: 300_000,
    maxDrawCalls: 60,
    targetFps: 30,
    targetFrameMs: 1000 / 30,
  },
  medium: {
    tier: 'medium',
    label: 'Vừa',
    dprRange: [1, 1],
    defaultDpr: 1,
    maxDpr: 1,
    shadows: false,
    shadowMapSize: 1024,
    contactShadows: true,
    waterReflections: false,
    waterRefractions: true,
    cloudLayers: 2,
    vegetationDensity: 0.5,
    floraNearRadius: 240,
    floraMidRadius: 420,
    creatureFullDetailRadius: 160,
    creatureSimplifiedRadius: 360,
    maxTriangles: 2_500_000,
    maxCreatureTriangles: 400_000,
    maxDrawCalls: 60,
    targetFps: 60,
    targetFrameMs: 1000 / 60,
  },
  high: {
    tier: 'high',
    label: 'Cao',
    dprRange: [1, 1.5],
    defaultDpr: 1.5,
    maxDpr: 1.5,
    shadows: true,
    shadowMapSize: 2048,
    contactShadows: false,
    waterReflections: true,
    waterRefractions: true,
    cloudLayers: 3,
    vegetationDensity: 1,
    floraNearRadius: 360,
    floraMidRadius: 700,
    creatureFullDetailRadius: 260,
    creatureSimplifiedRadius: 560,
    maxTriangles: 6_000_000,
    maxCreatureTriangles: 600_000,
    maxDrawCalls: 70,
    targetFps: 60,
    targetFrameMs: 1000 / 60,
  },
};

export function isQualityTier(value: unknown): value is QualityTier {
  return typeof value === 'string' && (QUALITY_TIERS as readonly string[]).includes(value);
}

export function isQualitySelection(value: unknown): value is QualitySelection {
  return typeof value === 'string' && (QUALITY_SELECTIONS as readonly string[]).includes(value);
}

export function getQualityTierConfig(tier: QualityTier): QualityTierConfig {
  return QUALITY_TIER_CONFIGS[tier];
}

export function resolveEffectiveQualityTier(
  selection: QualitySelection,
  autoTier: QualityTier = 'high',
): QualityTier {
  return selection === 'auto' ? autoTier : selection;
}

export function readInitialQualitySelection(
  search: string,
  captureQuality?: QualityTier,
): QualitySelection {
  if (captureQuality !== undefined) {
    return captureQuality;
  }
  const param = new URLSearchParams(search).get('quality');
  return isQualitySelection(param) ? param : 'auto';
}

export function resolveDeviceDpr(
  tier: QualityTier,
  devicePixelRatio: number,
  dprOverride?: number,
): number {
  const cfg = QUALITY_TIER_CONFIGS[tier];
  const [minDpr, maxDpr] = cfg.dprRange;
  if (dprOverride !== undefined && Number.isFinite(dprOverride)) {
    return Math.min(Math.max(dprOverride, minDpr), maxDpr);
  }
  if (tier === 'high') {
    const raw = Number.isFinite(devicePixelRatio) && devicePixelRatio > 0 ? devicePixelRatio : 1;
    return Math.min(Math.max(raw, minDpr), maxDpr);
  }
  return cfg.defaultDpr;
}

export function getCanvasDprProp(
  tier: QualityTier,
  dprOverride?: number,
): number | [number, number] {
  const cfg = QUALITY_TIER_CONFIGS[tier];
  const [minDpr, maxDpr] = cfg.dprRange;
  if (dprOverride !== undefined && Number.isFinite(dprOverride)) {
    return Math.min(Math.max(dprOverride, minDpr), maxDpr);
  }
  if (tier === 'high') {
    return [minDpr, maxDpr];
  }
  return cfg.defaultDpr;
}

/** Map 3-tier quality to legacy 2-tier `WorldVegetation` prop until `RW-C3`. */
export function legacyVegetationQuality(tier: QualityTier): 'high' | 'low' {
  return tier === 'high' ? 'high' : 'low';
}
