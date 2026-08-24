import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  CAPTURE_FIXED_DELTA,
  CAPTURE_FIXED_ELAPSED,
  isSceneTimeFrozen,
  makeSceneRandom,
  resetSceneClockCache,
  sceneDelta,
  sceneElapsed,
} from '@/components/Landscape/utils/sceneClock';
import { isCaptureRequested } from '@/components/Landscape/utils/captureMode';

// The reproducibility contract for the canonical captures.
//
// The previous pass described the capture clock as pinned. It pinned the day/night clock and left
// three other sources of variation running: r3f's render clock (which drives the water swell, the
// terrain shader, vegetation, wildlife, birds, fish and the waterfall curtains), `Math.random()` for
// 700 stars, and `Math.random()` again for precipitation. None is visible in one screenshot.
//
// The gate that catches it is `canonical_views.spec.ts` re-shooting each view and comparing SHA-256.
// This file is the unit-level half: the frozen clock behaves, the seeded streams are stable and
// independent, and — the part that actually stops a regression — no animated `World*` component reads
// the render clock or `Math.random` directly any more.

const HERE = dirname(fileURLToPath(import.meta.url));
const LANDSCAPE = resolve(HERE, '../../src/components/Landscape');

/** A clock whose elapsed time advances on every read, like the real one. */
function tickingClock(): { getElapsedTime: () => number } {
  let t = 0;
  return { getElapsedTime: () => (t += 1.5) };
}

function setSearch(search: string): void {
  window.history.replaceState({}, '', `/landscape.html${search}`);
  resetSceneClockCache();
}

describe('scene clock — live by default, frozen under capture', () => {
  beforeEach(() => setSearch(''));
  afterEach(() => setSearch(''));

  it('passes the real clock through on an ordinary visit', () => {
    expect(isSceneTimeFrozen()).toBe(false);
    const clock = tickingClock();
    expect(sceneElapsed(clock)).toBe(1.5);
    expect(sceneElapsed(clock)).toBe(3);
  });

  it('returns one fixed time under capture, however many times it is asked', () => {
    setSearch('?capture=1&view=water');
    expect(isSceneTimeFrozen()).toBe(true);
    const clock = tickingClock();
    expect(sceneElapsed(clock)).toBe(CAPTURE_FIXED_ELAPSED);
    expect(sceneElapsed(clock)).toBe(CAPTURE_FIXED_ELAPSED);
    expect(sceneElapsed(clock)).toBe(CAPTURE_FIXED_ELAPSED);
  });

  it('freezes at a time where things are actually moving', () => {
    // Zero is the one value at which every sine-driven animation sits at its rest position
    // simultaneously — flat water, unswayed grass. Reproducible, and a picture of the scene's initial
    // conditions rather than of the scene.
    expect(CAPTURE_FIXED_ELAPSED).toBeGreaterThan(0);
  });

  it('passes the real frame delta through on an ordinary visit', () => {
    expect(sceneDelta(0.0163)).toBe(0.0163);
    expect(sceneDelta(0.5)).toBe(0.5);
  });

  it('returns one fixed step under capture, whatever the real frame took', () => {
    // The second time channel. A component doing `x += delta * k` for ninety frames lands wherever
    // those ninety frames' real durations put it — reproducible only on a machine that renders at a
    // constant rate, which no machine does.
    setSearch('?capture=1&view=overview');
    expect(sceneDelta(0.0163)).toBe(CAPTURE_FIXED_DELTA);
    expect(sceneDelta(0.5)).toBe(CAPTURE_FIXED_DELTA);
    expect(CAPTURE_FIXED_DELTA).toBeGreaterThan(0);
  });

  it('only recognises the exact capture flag', () => {
    for (const search of ['', '?capture=0', '?capture=true', '?view=water', '?captured=1']) {
      expect(isCaptureRequested(search), search).toBe(false);
    }
    expect(isCaptureRequested('?capture=1')).toBe(true);
    expect(isCaptureRequested('?view=water&capture=1&t=10')).toBe(true);
  });
});

describe('scene randomness — seeded under capture, independent per stream', () => {
  beforeEach(() => setSearch(''));
  afterEach(() => setSearch(''));

  it('hands back `Math.random` itself on an ordinary visit', () => {
    expect(makeSceneRandom('sky.stars')).toBe(Math.random);
  });

  it('produces the same sequence twice under capture', () => {
    setSearch('?capture=1&view=lighting');
    const a = Array.from({ length: 8 }, makeSceneRandom('sky.stars'));
    const b = Array.from({ length: 8 }, makeSceneRandom('sky.stars'));
    expect(a).toEqual(b);
    for (const v of a) {
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThan(1);
    }
  });

  it('gives different streams different sequences', () => {
    setSearch('?capture=1&view=lighting');
    const stars = makeSceneRandom('sky.stars');
    const rain = makeSceneRandom('weather.rain');
    const s = Array.from({ length: 6 }, stars);
    const r = Array.from({ length: 6 }, rain);
    expect(s).not.toEqual(r);
  });

  it('is independent of how many numbers another stream drew first', () => {
    // The reason streams are named rather than sharing one generator: a shared generator makes every
    // consumer's output depend on component mount order, which is reproducible right up until a
    // `<Suspense>` boundary moves.
    setSearch('?capture=1&view=lighting');
    const alone = Array.from({ length: 4 }, makeSceneRandom('weather.snow'));

    setSearch('?capture=1&view=lighting');
    const other = makeSceneRandom('sky.stars');
    for (let i = 0; i < 137; i++) other();
    const after = Array.from({ length: 4 }, makeSceneRandom('weather.snow'));

    expect(after).toEqual(alone);
  });
});

describe('no animated scene component bypasses the frozen clock', () => {
  // The structural gate. Every fix above is undone by one component calling
  // `state.clock.getElapsedTime()` directly, and the resulting image still looks correct.
  const sources = readdirSync(LANDSCAPE)
    .filter((f) => f.startsWith('World') && f.endsWith('.tsx'))
    .map((f) => ({ file: f, text: readFileSync(resolve(LANDSCAPE, f), 'utf8') }));

  it('finds the components it is supposed to be checking', () => {
    // A directory rename would otherwise make this suite pass by checking nothing.
    expect(sources.length).toBeGreaterThan(10);
    expect(sources.map((s) => s.file)).toContain('WorldWater.tsx');
  });

  it('reads scene time only through `sceneElapsed`', () => {
    const offenders = sources
      .filter((s) => /\.getElapsedTime\(\)/.test(s.text))
      .map((s) => s.file);
    expect(
      offenders,
      'these read r3f\'s render clock directly, so two captures of the same view differ: route them ' +
        'through `sceneElapsed(state.clock)`',
    ).toEqual([]);
  });

  it('draws randomness only through `makeSceneRandom`', () => {
    const offenders = sources
      .filter((s) => /(^|[^.\w])Math\.random\(\)/m.test(stripComments(s.text)))
      .map((s) => s.file);
    expect(
      offenders,
      'these scatter scenery with `Math.random()`, which differs on every page load: use ' +
        '`makeSceneRandom(<stream>)`',
    ).toEqual([]);
  });

  it('names every `useFrame` delta `rawDelta`', () => {
    // A naming rule, so the next rule can be mechanical. `delta` is the name r3f documents and the
    // one a new `useFrame` will be written with; making the unpinned value carry an alarming name is
    // what turns "did anyone think about capture here" into something a regex can answer.
    const offenders: string[] = [];
    for (const s of sources) {
      for (const [, params] of stripComments(s.text).matchAll(/useFrame\(\s*\(([^)]*)\)/g)) {
        const second = params.split(',')[1]?.trim();
        if (second && second !== 'rawDelta') offenders.push(`${s.file}: useFrame((…, ${second})`);
      }
    }
    expect(
      offenders,
      "r3f's frame delta must be received as `rawDelta` and converted with `sceneDelta`",
    ).toEqual([]);
  });

  it('integrates frame deltas only through `sceneDelta`', () => {
    // The rule that has teeth. Every mention of `rawDelta` in the file must be its declaration or the
    // argument to `sceneDelta`, so a component cannot quietly integrate the real one — which is what
    // `WorldSky`'s cloud drift and `WorldWeather`'s precipitation both did, invisibly, because the
    // canonical defaults happen to multiply their results by zero.
    const offenders: string[] = [];
    for (const s of sources) {
      const text = stripComments(s.text);
      const mentions = text.match(/\brawDelta\b/g)?.length ?? 0;
      if (mentions === 0) continue;
      const declarations = text.match(/useFrame\(\s*\([^)]*\brawDelta\b[^)]*\)/g)?.length ?? 0;
      const converted = text.match(/\bsceneDelta\(\s*rawDelta\s*\)/g)?.length ?? 0;
      if (mentions !== declarations + converted) {
        offenders.push(
          `${s.file}: ${mentions} uses of rawDelta, ${declarations} declared + ${converted} through sceneDelta`,
        );
      }
    }
    expect(
      offenders,
      'these use r3f\'s frame delta directly, so a value integrated over the settle window depends ' +
        'on how fast those frames ran: pass it through `sceneDelta(rawDelta)` once at the top',
    ).toEqual([]);
  });
});

/** Drop `//` and block comments so a prose mention of `Math.random()` is not read as a call. */
function stripComments(text: string): string {
  return text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|[^:])\/\/.*$/gm, '$1');
}
