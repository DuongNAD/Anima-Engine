import { test, expect } from '@playwright/test';
import type { Page } from '@playwright/test';
import { QUALITY_TIERS, QUALITY_TIER_CONFIGS, type QualityTier } from '../../src/components/Landscape/utils/qualityTiers';
import type { FrameRenderCounters } from '../../src/components/Landscape/utils/diagnosticsWindow';
import type { CanonicalViewId } from '../../src/components/Landscape/utils/mapManifest';

// ---------------------------------------------------------------------------------------
// RW-C4 (RW4-6): the render budget gate.
//
// For each of the three canonical poses x each of the three quality tiers this reads
// `window.__animaFrameCounters` (RW0-4) and fails when a frame exceeds a budget of
// docs/ai/planning/2026-11-16-feature-readable-world.md §4a, naming the layer and the budget.
//
// # Why it runs on CI
//
// Triangle and draw-call counts come from three.js `renderer.info`, i.e. from what the app submits,
// not from what the GPU does with it. They are identical on SwiftShader, ANGLE/Metal and D3D11
// (RW-C3 recorded the same flora number on a Mac and on the Dell). So unlike the canonical-view
// capture this spec has no software-rasteriser refusal and uses the normal test:e2e config. It does
// NOT measure FPS; that is RW6.
//
// # Where the budgets live
//
//   total triangles, total draw calls, creature triangles  -> `QUALITY_TIER_CONFIGS` (qualityTiers.ts),
//                                                              imported, never copied
//   flora triangles and flora draw calls                   -> `FLORA_BUDGET` below. §4a has them, but
//                                                              `qualityTiers.ts` (lane B) has no field
//                                                              for them; this is their only copy.
//
// # What is deliberately not asserted
//
// The 1000-creature synthetic replay. Its tick arrival is wall-clock driven, so the number of
// visible instances (and so the creature triangles) varies from frame to frame (548k and 485k were
// read in one session at the same tier), and with the pre-RW-A2 sphere mesh the creature layer is
// above its §4a cap anyway. The creature columns of §4a belong to RW-A4. The creature cap is still
// asserted here, against the zero creatures this spec spawns, so the check is live the day a
// deterministic fixture is added.
//
// # Negative control
//
// `?floraDensityScale=N` multiplies the tier's flora density. It is honoured only when
// `import.meta.env.DEV` is true (the dev server this suite starts) and is dead code in
// `npm run build`. The last test turns it up and requires this same checker to report the flora
// layer. Note: x2 alone is NOT enough to leave §4a at `low` (flora 260k -> 519k against 530k), so
// the control uses x4; see `CONTROL_SCALE`.
// ---------------------------------------------------------------------------------------

const POSES: readonly CanonicalViewId[] = ['overview', 'ecosystem', 'spawn'];

/** §4a: the flora column. Tris include the shadow pass at `high`. */
const FLORA_BUDGET: Readonly<Record<QualityTier, { triangles: number; calls: number }>> = {
  low: { triangles: 530_000, calls: 16 },
  medium: { triangles: 1_730_000, calls: 16 },
  high: { triangles: 5_030_000, calls: 23 },
};

/** The size RW-C3 measured at; the counts depend on the frustum a little. */
const VIEWPORT = { width: 1024, height: 768 };

/** Mirrors `FLORA_DENSITY_SCALE_PARAM`; not imported because that module uses `import.meta`, which the Playwright loader cannot run. */
const FLORA_DENSITY_SCALE_PARAM = 'floraDensityScale';

const CONTROL_SCALE = 4;

/** Manual red-run switch: `RW_BUDGET_FLORA_SCALE=2 npm run test:e2e --prefix tests -- rw_budget`. */
const MANUAL_SCALE = Number(process.env.RW_BUDGET_FLORA_SCALE ?? 1);

interface Violation {
  layer: string;
  metric: 'triangles' | 'calls';
  value: number;
  budget: number;
}

/** Pure: every §4a budget the counters exceed. */
function checkBudget(tier: QualityTier, c: FrameRenderCounters): Violation[] {
  const cfg = QUALITY_TIER_CONFIGS[tier];
  const flora = FLORA_BUDGET[tier];
  const checks: Violation[] = [
    { layer: 'total', metric: 'triangles', value: c.total.triangles, budget: cfg.maxTriangles },
    { layer: 'total', metric: 'calls', value: c.total.calls, budget: cfg.maxDrawCalls },
    { layer: 'flora', metric: 'triangles', value: c.layers.flora.triangles, budget: flora.triangles },
    { layer: 'flora', metric: 'calls', value: c.layers.flora.calls, budget: flora.calls },
    { layer: 'creatures', metric: 'triangles', value: c.layers.creatures.triangles, budget: cfg.maxCreatureTriangles },
  ];
  return checks.filter((v) => v.value > v.budget);
}

function describe(tier: string, pose: string, vs: Violation[]): string {
  return vs
    .map((v) => `[${v.layer}] ${v.metric} ${v.value.toLocaleString('en-US')} > §4a budget ${v.budget.toLocaleString('en-US')} (${tier}/${pose})`)
    .join('\n');
}

/**
 * Pin the pose and wait for the frame counters to settle: the same numbers over `STABLE` consecutive
 * published frames, at least `MIN_FRAMES` after the pin (the chunk selection re-runs when the camera
 * moves). Polls frames, not wall time, so a slow software renderer only takes longer.
 */
async function settledCounters(page: Page, pose: CanonicalViewId): Promise<FrameRenderCounters> {
  const handle = await page.evaluateHandle(
    ([p]) =>
      new Promise<FrameRenderCounters>((resolve, reject) => {
        const w = window as unknown as {
          __animaSetCanonicalPose: (v: string) => void;
          __animaFrameCounters?: FrameRenderCounters;
        };
        const STABLE = 4;
        const MIN_FRAMES = 4;
        w.__animaSetCanonicalPose(p);
        const start = w.__animaFrameCounters?.frame ?? 0;
        let last = '';
        let lastFrame = -1;
        let run = 0;
        const t0 = Date.now();
        const tick = () => {
          const c = w.__animaFrameCounters;
          if (Date.now() - t0 > 90_000) return reject(new Error(`counters never settled for ${p}`));
          if (c && c.frame !== lastFrame && c.frame >= start + MIN_FRAMES) {
            lastFrame = c.frame;
            const key = JSON.stringify([c.total, c.layers, c.other]);
            run = key === last ? run + 1 : 1;
            last = key;
            if (run >= STABLE) return resolve(c);
          }
          requestAnimationFrame(tick);
        };
        tick();
      }),
    [pose],
  );
  const counters = (await handle.jsonValue()) as FrameRenderCounters;
  await handle.dispose();
  return counters;
}

async function openScene(page: Page, tier: QualityTier, scale: number): Promise<void> {
  await page.setViewportSize(VIEWPORT);
  const extra = scale === 1 ? '' : `&${FLORA_DENSITY_SCALE_PARAM}=${scale}`;
  await page.goto(`/landscape.html?quality=${tier}${extra}`, { waitUntil: 'load', timeout: 120_000 });
  // Fail closed: a scene that never publishes counters is a red gate, never a skip.
  await page.waitForFunction(
    () => {
      const w = window as unknown as { __animaSetCanonicalPose?: unknown; __animaFrameCounters?: unknown };
      return typeof w.__animaSetCanonicalPose === 'function' && w.__animaFrameCounters !== undefined;
    },
    undefined,
    { timeout: 180_000 },
  );
}

test.describe('RW-C4 render budget (§4a)', () => {
  test.describe.configure({ mode: 'serial' });
  test.setTimeout(300_000);

  for (const tier of QUALITY_TIERS) {
    test(`${tier}: every pose is within the §4a budget`, async ({ page }) => {
      await openScene(page, tier, MANUAL_SCALE);
      const failures: string[] = [];
      for (const pose of POSES) {
        const c = await settledCounters(page, pose);
        console.log(
          `rw-budget ${tier}/${pose}: total ${c.total.triangles} tris / ${c.total.calls} calls, ` +
            `flora ${c.layers.flora.triangles} / ${c.layers.flora.calls}, ` +
            `creatures ${c.layers.creatures.triangles} / ${c.layers.creatures.calls}`,
        );
        expect(c.layers.flora.triangles, `${tier}/${pose}: flora layer produced nothing (counters broken?)`).toBeGreaterThan(0);
        const vs = checkBudget(tier, c);
        if (vs.length > 0) failures.push(describe(tier, pose, vs));
      }
      expect(failures, `render budget exceeded:\n${failures.join('\n')}`).toEqual([]);
    });
  }

  test(`negative control: flora density x${CONTROL_SCALE} turns the gate red and names the flora layer`, async ({ page }) => {
    await openScene(page, 'low', CONTROL_SCALE);
    const c = await settledCounters(page, 'ecosystem');
    const vs = checkBudget('low', c);
    console.log(`rw-budget control (x${CONTROL_SCALE}):\n${describe('low', 'ecosystem', vs)}`);
    expect(vs.map((v) => v.layer), 'the checker must name the layer that exceeded').toContain('flora');
    expect(vs.some((v) => v.layer === 'flora' && v.metric === 'triangles')).toBe(true);
  });
});
