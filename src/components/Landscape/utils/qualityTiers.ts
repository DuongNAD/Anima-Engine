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

export const AUTO_QUALITY_WARMUP_FRAMES = 30;
export const AUTO_QUALITY_SETTLE_MS = 1000;
export const AUTO_QUALITY_SETTLE_MIN_FRAMES = 15;
export const AUTO_QUALITY_DOWNGRADE_WINDOW_MS = 2000;
export const AUTO_QUALITY_UPGRADE_WINDOW_MS = 10_000;
export const AUTO_QUALITY_MIN_WINDOW_FRAMES = 8;
export const AUTO_QUALITY_MAX_SAMPLE_FRAME_MS = 500;
export const AUTO_QUALITY_OVER_BUDGET_RATIO = 1.1;
export const AUTO_QUALITY_HEADROOM_RATIO = 0.85;

export type AutoQualityPhase = 'warmup' | 'settling' | 'running';

export interface AutoQualityOptions {
  readonly initialTier?: QualityTier;
  readonly devicePixelRatio?: number;
}

export interface AutoQualityState {
  readonly phase: AutoQualityPhase;
  readonly tier: QualityTier;
  readonly dprOverride: number | undefined;
  readonly devicePixelRatio: number;
  readonly upgradedOnce: boolean;
  readonly warmupFrames: number;
  readonly settleSamplesMs: readonly number[];
  readonly settleElapsedMs: number;
  readonly windowSamplesMs: readonly number[];
  readonly windowElapsedMs: number;
  readonly headroomSamplesMs: readonly number[];
  readonly headroomElapsedMs: number;
}

/** Compute median frame time in milliseconds, or `0` for an empty sample array. */
export function medianFrameMs(samples: readonly number[]): number {
  if (samples.length === 0) return 0;
  const sorted = [...samples].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) * 0.5;
}

/** Create initial state for automatic tier selection (`warmup` -> `settling` -> `running`). */
export function createAutoQualityState(options?: AutoQualityOptions): AutoQualityState {
  const rawDpr = options?.devicePixelRatio;
  const dpr = rawDpr !== undefined && Number.isFinite(rawDpr) && rawDpr > 0 ? rawDpr : 1.5;
  return {
    phase: 'warmup',
    tier: options?.initialTier ?? 'high',
    dprOverride: undefined,
    devicePixelRatio: dpr,
    upgradedOnce: false,
    warmupFrames: 0,
    settleSamplesMs: [],
    settleElapsedMs: 0,
    windowSamplesMs: [],
    windowElapsedMs: 0,
    headroomSamplesMs: [],
    headroomElapsedMs: 0,
  };
}

/** Pick initial running tier from the settle window median frame time. */
export function pickSettledQualityTier(medianMs: number): QualityTier {
  if (medianMs <= QUALITY_TIER_CONFIGS.high.targetFrameMs * AUTO_QUALITY_OVER_BUDGET_RATIO) {
    return 'high';
  }
  if (medianMs <= QUALITY_TIER_CONFIGS.low.targetFrameMs) {
    return 'medium';
  }
  return 'low';
}

/** Step down DPR before lowering the quality tier (`RW4-5`). */
export function stepDownQuality(
  tier: QualityTier,
  dprOverride: number | undefined,
  devicePixelRatio = 1.5,
): { readonly tier: QualityTier; readonly dprOverride: number | undefined } {
  const minDpr = QUALITY_TIER_CONFIGS[tier].dprRange[0];
  const currentDpr = resolveDeviceDpr(tier, devicePixelRatio, dprOverride);
  if (currentDpr > minDpr + 1e-6) {
    return { tier, dprOverride: minDpr };
  }
  if (tier === 'high') return { tier: 'medium', dprOverride: undefined };
  if (tier === 'medium') return { tier: 'low', dprOverride: undefined };
  return { tier: 'low', dprOverride: minDpr };
}

/** Step up once when sustained headroom allows (`RW4-5`). */
export function stepUpQualityOnce(
  tier: QualityTier,
  dprOverride: number | undefined,
): { readonly tier: QualityTier; readonly dprOverride: number | undefined } {
  if (dprOverride !== undefined) return { tier, dprOverride: undefined };
  if (tier === 'low') return { tier: 'medium', dprOverride: undefined };
  return { tier: 'high', dprOverride: undefined };
}

function trimSlidingWindow(
  samples: number[],
  elapsedMs: number,
  maxWindowMs: number,
  minSamples: number,
): number {
  let elapsed = elapsedMs;
  while (samples.length > minSamples && elapsed - samples[0] >= maxWindowMs) {
    elapsed -= samples.shift()!;
  }
  return elapsed;
}

/** Advance pure auto-tier adaptation state by one frame duration (`RW4-5`). */
export function stepAutoQualityState(state: AutoQualityState, frameMs: number): AutoQualityState {
  if (!Number.isFinite(frameMs) || frameMs <= 0 || frameMs > AUTO_QUALITY_MAX_SAMPLE_FRAME_MS) {
    return state;
  }

  if (state.phase === 'warmup') {
    const warmupFrames = state.warmupFrames + 1;
    return {
      ...state,
      warmupFrames,
      phase: warmupFrames >= AUTO_QUALITY_WARMUP_FRAMES ? 'settling' : 'warmup',
    };
  }

  if (state.phase === 'settling') {
    const settleSamplesMs = [...state.settleSamplesMs, frameMs];
    const settleElapsedMs = state.settleElapsedMs + frameMs;
    if (
      settleElapsedMs >= AUTO_QUALITY_SETTLE_MS &&
      settleSamplesMs.length >= AUTO_QUALITY_SETTLE_MIN_FRAMES
    ) {
      return {
        ...state,
        phase: 'running',
        tier: pickSettledQualityTier(medianFrameMs(settleSamplesMs)),
        dprOverride: undefined,
        settleSamplesMs: [],
        settleElapsedMs: 0,
        windowSamplesMs: [],
        windowElapsedMs: 0,
        headroomSamplesMs: [],
        headroomElapsedMs: 0,
      };
    }
    return { ...state, settleSamplesMs, settleElapsedMs };
  }

  const nextWindow = [...state.windowSamplesMs, frameMs];
  let windowElapsedMs = state.windowElapsedMs + frameMs;
  const targetMs = QUALITY_TIER_CONFIGS[state.tier].targetFrameMs;
  const overBudgetMs = targetMs * AUTO_QUALITY_OVER_BUDGET_RATIO;
  const headroomLimitMs = targetMs * AUTO_QUALITY_HEADROOM_RATIO;

  if (
    windowElapsedMs >= AUTO_QUALITY_DOWNGRADE_WINDOW_MS &&
    nextWindow.length >= AUTO_QUALITY_MIN_WINDOW_FRAMES
  ) {
    if (medianFrameMs(nextWindow) > overBudgetMs) {
      const down = stepDownQuality(state.tier, state.dprOverride, state.devicePixelRatio);
      if (down.tier !== state.tier || down.dprOverride !== state.dprOverride) {
        return {
          ...state,
          tier: down.tier,
          dprOverride: down.dprOverride,
          windowSamplesMs: [],
          windowElapsedMs: 0,
          headroomSamplesMs: [],
          headroomElapsedMs: 0,
        };
      }
    }
    windowElapsedMs = trimSlidingWindow(
      nextWindow,
      windowElapsedMs,
      AUTO_QUALITY_DOWNGRADE_WINDOW_MS,
      AUTO_QUALITY_MIN_WINDOW_FRAMES,
    );
  }

  let headroomSamplesMs = state.headroomSamplesMs;
  let headroomElapsedMs = state.headroomElapsedMs;

  if (!state.upgradedOnce && (state.tier !== 'high' || state.dprOverride !== undefined)) {
    const windowBroken =
      nextWindow.length >= AUTO_QUALITY_MIN_WINDOW_FRAMES &&
      medianFrameMs(nextWindow) > headroomLimitMs;
    if (frameMs > overBudgetMs || windowBroken) {
      headroomSamplesMs = [];
      headroomElapsedMs = 0;
    } else {
      const nextHeadroom = [...headroomSamplesMs, frameMs];
      const nextHeadroomElapsed = headroomElapsedMs + frameMs;
      if (
        nextHeadroomElapsed >= AUTO_QUALITY_UPGRADE_WINDOW_MS &&
        nextHeadroom.length >= AUTO_QUALITY_MIN_WINDOW_FRAMES &&
        medianFrameMs(nextHeadroom) <= headroomLimitMs
      ) {
        const up = stepUpQualityOnce(state.tier, state.dprOverride);
        return {
          ...state,
          tier: up.tier,
          dprOverride: up.dprOverride,
          upgradedOnce: true,
          windowSamplesMs: [],
          windowElapsedMs: 0,
          headroomSamplesMs: [],
          headroomElapsedMs: 0,
        };
      }
      headroomSamplesMs = nextHeadroom;
      headroomElapsedMs = nextHeadroomElapsed;
    }
  } else if (headroomSamplesMs.length > 0) {
    headroomSamplesMs = [];
    headroomElapsedMs = 0;
  }

  return {
    ...state,
    windowSamplesMs: nextWindow,
    windowElapsedMs,
    headroomSamplesMs,
    headroomElapsedMs,
  };
}

/** Resolve effective tier + DPR override: capture bypasses auto; manual always wins (`RW4-4`, `RW4-5`). */
export function resolveEffectiveQualityState(
  selection: QualitySelection,
  autoState: Pick<AutoQualityState, 'tier' | 'dprOverride'>,
  captureQuality?: QualityTier,
): { readonly tier: QualityTier; readonly dprOverride: number | undefined } {
  if (captureQuality !== undefined) {
    return { tier: captureQuality, dprOverride: undefined };
  }
  if (selection !== 'auto') {
    return { tier: selection, dprOverride: undefined };
  }
  return { tier: autoState.tier, dprOverride: autoState.dprOverride };
}
