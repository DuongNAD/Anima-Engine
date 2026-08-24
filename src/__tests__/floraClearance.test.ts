import { describe, it, expect, beforeAll } from 'vitest';
import {
  TALL_FLORA_TYPES,
  isTallFlora,
  floraColliderRadius,
  floraCanopyRadius,
  FLORA_COLLIDER_BASE_RADIUS,
  FLORA_COLLIDER_SCALE_COEFF,
  FLORA_FOOTPRINT_UNIT_RADIUS,
  VEGETATION_BASE_SIZE,
  SPAWN_CLEARANCE_MARGIN,
  buildFloraColliderIndex,
  floraOverlapAt,
  isFloraClear,
  resolveFloraOverlap,
  spawnClearanceRadius,
} from '../components/Landscape/utils/floraClearance';
import type { FloraSource } from '../components/Landscape/utils/floraClearance';
import { FloraType, generateWorld } from '../components/Landscape/utils/worldGen';
import { SHARED_WORLD_SEED, SHARED_WORLD_SHAPE } from '../utils/sharedWorld';
import { findSpawn } from '../components/Landscape/utils/worldSample';

// ---------------------------------------------------------------------------------------
// One clearance policy, four consumers.
//
// The defect this file exists for: the walk-mode spawn put the camera *inside* a trunk. Three
// places each had their own idea of what a solid tree is —
//
//   * `WorldCameraRig` had an inline `TALL` set and an inline `r = 0.45 + floraScale*0.25`;
//   * `scripts/gen_map_manifest.ts` restated both in a comment and in code;
//   * `findSpawn` did not know flora existed at all, so it happily returned a cell with a pine
//     growing out of it.
//
// A fourth consumer (the canonical view capture harness) was about to restate them a fourth time.
// So the policy moves into one module and every consumer imports it. These tests pin the policy
// itself, then prove the two behaviours that were actually wrong: spawn clearance, and the
// zero-distance overlap the runtime push-out silently declined to resolve.
// ---------------------------------------------------------------------------------------

/** A flora source with hand-placed trunks — exact positions, so the geometry is checkable. */
function floraAt(
  trunks: Array<{ x: number; z: number; scale: number; type?: FloraType }>,
  size = 100,
): FloraSource {
  return {
    size,
    floraCount: trunks.length,
    floraX: Float32Array.from(trunks.map((t) => t.x)),
    floraZ: Float32Array.from(trunks.map((t) => t.z)),
    floraScale: Float32Array.from(trunks.map((t) => t.scale)),
    floraType: Uint8Array.from(trunks.map((t) => t.type ?? FloraType.Pine)),
  };
}

describe('flora clearance policy — the shared definition of "a tree is in the way"', () => {
  it('marks exactly the seven trunked types solid', () => {
    // These seven are what `WorldCameraRig` collided against and what the MCP manifest declares
    // `collider.enabled`. Ground cover and aquatic flora are walked through.
    expect([...TALL_FLORA_TYPES].sort((a, b) => a - b)).toEqual([
      FloraType.Pine,
      FloraType.Round,
      FloraType.Jungle,
      FloraType.Cactus,
      FloraType.Acacia,
      FloraType.Palm,
      FloraType.DeadTree,
    ].sort((a, b) => a - b));

    for (const t of TALL_FLORA_TYPES) expect(isTallFlora(t)).toBe(true);
    for (const t of [FloraType.Rock, FloraType.Bush, FloraType.Reed, FloraType.Tuft, FloraType.Coral, FloraType.Kelp, FloraType.Seagrass]) {
      expect(isTallFlora(t)).toBe(false);
    }
  });

  it('keeps the walk-mode collider radius the rig already used', () => {
    // Not a new number: `r = 0.45 + floraScale * 0.25`, lifted verbatim so the manifest, the
    // spawn picker and the runtime cannot drift apart.
    expect(FLORA_COLLIDER_BASE_RADIUS).toBe(0.45);
    expect(FLORA_COLLIDER_SCALE_COEFF).toBe(0.25);
    expect(floraColliderRadius(0)).toBeCloseTo(0.45, 10);
    expect(floraColliderRadius(1)).toBeCloseTo(0.7, 10);
    expect(floraColliderRadius(2.4)).toBeCloseTo(1.05, 10);
  });

  it('separates the trunk collider from the space the tree occupies', () => {
    // The distinction the finding turned on. An acacia at unit flora scale collides within 0.7
    // units but spreads its umbrella to 0.95 * 1.4 = 1.33 — at walking eye height. A clearance
    // rule built only on the collider calls the gap between those two radii "clear" while the
    // camera is looking at the underside of a canopy.
    expect(floraColliderRadius(1)).toBeCloseTo(0.7, 10);
    expect(floraCanopyRadius(FloraType.Acacia, 1)).toBeCloseTo(0.95 * VEGETATION_BASE_SIZE, 10);
    expect(floraCanopyRadius(FloraType.Acacia, 1)).toBeGreaterThan(floraColliderRadius(1));

    // Every solid type is declared, and every declaration is wider than nothing.
    for (const t of TALL_FLORA_TYPES) {
      expect(FLORA_FOOTPRINT_UNIT_RADIUS[t], `${FloraType[t]} has no declared footprint`).toBeGreaterThan(0);
    }
    // Ground cover is not solid, so it has no entry to keep in sync.
    expect(FLORA_FOOTPRINT_UNIT_RADIUS[FloraType.Tuft]).toBeUndefined();
    expect(floraCanopyRadius(FloraType.Tuft, 1)).toBe(0);
  });

  it('queries the canopy when asked to', () => {
    const src = floraAt([{ x: 0, z: 0, scale: 1, type: FloraType.Acacia }]);
    const index = buildFloraColliderIndex(src, 100);
    const between = (floraColliderRadius(1) + floraCanopyRadius(FloraType.Acacia, 1)) / 2;

    expect(floraOverlapAt(index, between, 0, 0, 'collider')).toBeNull();
    expect(floraOverlapAt(index, between, 0, 0, 'canopy')).not.toBeNull();
  });

  it('reports an overlap inside a trunk and none outside it', () => {
    const src = floraAt([{ x: 0, z: 0, scale: 1 }]); // r = 0.7 at renderSize == size
    const index = buildFloraColliderIndex(src, 100);

    expect(floraOverlapAt(index, 0.2, 0)).not.toBeNull();
    expect(floraOverlapAt(index, 0.69, 0)).not.toBeNull();
    expect(floraOverlapAt(index, 0.71, 0)).toBeNull();
    expect(floraOverlapAt(index, 40, 40)).toBeNull();
  });

  it('scales flora coordinates into render space', () => {
    // Flora coords are in data-grid units; the rig multiplies by renderSize / world.size. A policy
    // that forgot this would test clearance in the wrong space and always report "clear".
    const src = floraAt([{ x: 10, z: 0, scale: 1 }], 100);
    const index = buildFloraColliderIndex(src, 1000); // 10x

    expect(floraOverlapAt(index, 10, 0)).toBeNull(); // the data-space position: nothing there
    expect(floraOverlapAt(index, 100, 0)).not.toBeNull(); // the render-space position: a trunk
  });

  it('ignores ground cover and aquatic flora', () => {
    const src = floraAt([
      { x: 0, z: 0, scale: 1, type: FloraType.Bush },
      { x: 5, z: 0, scale: 1, type: FloraType.Kelp },
    ]);
    const index = buildFloraColliderIndex(src, 100);
    expect(index.count).toBe(0);
    expect(floraOverlapAt(index, 0, 0)).toBeNull();
    expect(floraOverlapAt(index, 5, 0)).toBeNull();
  });

  describe('zero-distance overlap', () => {
    // The finding: the runtime push-out ran only when `d2 > 1e-6`. A player standing at exactly the
    // trunk centre — which is what a spawn picked from a flora cell produces — divided by a
    // distance it had already refused to trust, so the branch was skipped and the player stayed
    // inside the tree forever.
    it('pushes a point out of a trunk it is exactly centred on', () => {
      const src = floraAt([{ x: 0, z: 0, scale: 1 }]);
      const index = buildFloraColliderIndex(src, 100);

      expect(floraOverlapAt(index, 0, 0)).not.toBeNull();
      const out = resolveFloraOverlap(index, 0, 0);
      expect(Math.hypot(out.x, out.z)).toBeCloseTo(floraColliderRadius(1), 6);
      expect(floraOverlapAt(index, out.x, out.z)).toBeNull();
    });

    it('picks the same direction every time', () => {
      const src = floraAt([{ x: 3, z: -7, scale: 0.5 }]);
      const index = buildFloraColliderIndex(src, 100);
      const a = resolveFloraOverlap(index, 3, -7);
      const b = resolveFloraOverlap(index, 3, -7);
      expect(a).toEqual(b);
      // Documented fallback: +X. Any fixed axis works; naming one keeps captures reproducible.
      expect(a.x).toBeGreaterThan(3);
      expect(a.z).toBeCloseTo(-7, 10);
    });

    it('is idempotent — resolving an already-clear point moves nothing', () => {
      const src = floraAt([{ x: 0, z: 0, scale: 1 }]);
      const index = buildFloraColliderIndex(src, 100);
      const once = resolveFloraOverlap(index, 0, 0);
      const twice = resolveFloraOverlap(index, once.x, once.z);
      expect(twice.x).toBeCloseTo(once.x, 10);
      expect(twice.z).toBeCloseTo(once.z, 10);
    });

    it('escapes a clump of trunks sharing one centre', () => {
      const src = floraAt([
        { x: 0, z: 0, scale: 1 },
        { x: 0, z: 0, scale: 2 },
        { x: 0.3, z: 0.1, scale: 1.5 },
      ]);
      const index = buildFloraColliderIndex(src, 100);
      const out = resolveFloraOverlap(index, 0, 0);
      expect(floraOverlapAt(index, out.x, out.z)).toBeNull();
    });
  });

  it('rejects the geometry the browser reproduction actually had', () => {
    // The measurement that identified the defect, kept as a test rather than as prose.
    //
    // Browser reproduction: landscape.html, walk mode, shipped world, camera at render
    // (-129, -94), biome Đồng cỏ, centre of frame filled by a dark canopy. Measuring the
    // neighbourhood of that point found a broadleaf (Round, floraScale 1.255 -> canopy radius
    // 1.340) at distance 1.896 — subtending ninety degrees of view.
    //
    // This is stated as geometry, not as a position in a particular world, on purpose. Worldgen
    // 21 moved the offending tree (species is now chosen from the cell an instance lands in), so
    // that exact coordinate is clear today. The rule still has to reject the *situation*, or a
    // future world reintroduces it and nothing notices.
    const HISTORIC_CANOPY_R = floraCanopyRadius(FloraType.Round, 1.255);
    const HISTORIC_DISTANCE = 1.896;
    expect(HISTORIC_CANOPY_R).toBeCloseTo(1.34, 2);

    const src = floraAt([{ x: 0, z: 0, scale: 1.255, type: FloraType.Round }]);
    const index = buildFloraColliderIndex(src, 100);

    // Outside the trunk, outside the leaves — and still the whole picture. This is why neither of
    // the first two footprints caught it.
    expect(floraOverlapAt(index, HISTORIC_DISTANCE, 0, 0, 'collider')).toBeNull();
    expect(floraOverlapAt(index, HISTORIC_DISTANCE, 0, 0, 'canopy')).toBeNull();
    expect(floraOverlapAt(index, HISTORIC_DISTANCE, 0, 0, 'spawn')).not.toBeNull();

    // And the rule is the derivation, not a tuned constant: D >= 2R.
    expect(spawnClearanceRadius(HISTORIC_CANOPY_R)).toBeCloseTo(2 * HISTORIC_CANOPY_R, 10);
    expect(floraOverlapAt(index, 2 * HISTORIC_CANOPY_R + 0.01, 0, 0, 'spawn')).toBeNull();
  });

  it('honours a clearance margin', () => {
    const src = floraAt([{ x: 0, z: 0, scale: 1 }]); // r = 0.7
    const index = buildFloraColliderIndex(src, 100);
    expect(isFloraClear(index, 0.8, 0, 0)).toBe(true);
    expect(isFloraClear(index, 0.8, 0, 0.5)).toBe(false); // 0.8 < 0.7 + 0.5
    expect(SPAWN_CLEARANCE_MARGIN).toBeGreaterThan(0);
  });
});

describe('findSpawn — the world the app actually ships', () => {
  // This suite runs at the SHIPPED identity, not a convenient small one, because the defect only
  // exists there. `sharedWorld.ts` declares (seed "seed", 2048, continent) and that is what
  // `WorldShowcase` renders; at 256 or 512 the same code returns a position that is already
  // clear, so a cheaper world would have made this test pass without the fix. It was measured:
  //
  //   size  flora    old spawn            blocked by the new rule?
  //   256    7 319   (-101.2, -72.9)      no
  //   512   29 810   ( 20.0, -128.0)      no
  //   1024 118 137   ( 19.4, -128.4)      no
  //   2048 117 730   (-128.7,  -93.5)     YES — d=1.910, needs 2.680
  //
  // Generating 2048² costs ~7 s and ~500 MB, which is why it happens once in `beforeAll` with an
  // explicit budget rather than per test.
  const RENDER_SIZE = 1200; // WorldShowcase RENDER_SIZE

  let world: ReturnType<typeof generateWorld>;
  let index: ReturnType<typeof buildFloraColliderIndex>;
  let spawn: { x: number; z: number };

  beforeAll(() => {
    world = generateWorld(SHARED_WORLD_SEED, { size: 2048, shape: SHARED_WORLD_SHAPE });
    index = buildFloraColliderIndex(world, RENDER_SIZE);
    spawn = findSpawn(world, RENDER_SIZE);
  }, 180_000);

  it('generates a world that actually has solid flora to be blocked by', () => {
    // Guards the test itself: if flora placement stopped emitting tall types the clearance
    // assertions below would all pass vacuously.
    expect(index.count).toBeGreaterThan(10_000);
  });

  it('returns a position clear of every solid canopy', () => {
    const hit = floraOverlapAt(index, spawn.x, spawn.z, 0, 'spawn');
    expect(
      hit,
      hit
        ? `spawn (${spawn.x.toFixed(2)}, ${spawn.z.toFixed(2)}) is ${hit.distance.toFixed(3)} from ` +
          `flora at (${hit.x.toFixed(2)}, ${hit.z.toFixed(2)}) needing ${hit.radius.toFixed(3)}`
        : '',
    ).toBeNull();
    expect(isFloraClear(index, spawn.x, spawn.z, 0, 'collider')).toBe(true);
  });

  it('is still deterministic', () => {
    expect(findSpawn(world, RENDER_SIZE)).toEqual(spawn);
  });

  it('still lands on dry land', () => {
    // Clearance must not be bought by walking into the sea — the other way this could "pass".
    const u = spawn.x / RENDER_SIZE + 0.5;
    const v = spawn.z / RENDER_SIZE + 0.5;
    const cx = Math.min(world.size - 1, Math.max(0, Math.round(u * (world.size - 1))));
    const cz = Math.min(world.size - 1, Math.max(0, Math.round(v * (world.size - 1))));
    expect(world.elevation[cz * world.size + cx]).toBeGreaterThan(world.seaLevel);
  });
});
