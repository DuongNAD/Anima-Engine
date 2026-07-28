// Where a grazing animal is at a given moment.
//
// Split out of `WorldWildlife`'s frame loop so the path has a name and a test. The properties that
// matter are not visual — they are that the animal stays inside the circle its habitat check
// approved, that a frozen clock freezes the herd, and that two animals with different seeds do not
// walk in lockstep. None of those are observable by looking at one frame.

/** A point on the ground plane, in render units. */
export interface GrazePoint {
  x: number;
  z: number;
}

/**
 * Position of a grazing animal at time `t`.
 *
 * Polar, deliberately: a drifting heading and a drifting distance, so the offset length is bounded
 * by `radius` **by construction**.
 *
 * The first version bounded x and z separately. Each axis stayed inside the radius and the point
 * still did not: the reachable set was a square of half-width `radius`, whose corners are `radius ×
 * √2` away — 9.9 units for a circle of 7. `grazing.test.ts` measured 9.07 and failed, which is the
 * whole reason that test exists, because on screen it would have looked like nothing more than a
 * deer standing in shallow water.
 *
 * A pure function of `(anchor, radius, seed, t)`: no state, no randomness. Capture mode freezes the
 * clock, so it freezes the herd, and the canonical views stay byte-reproducible.
 *
 * @param anchor - Where the animal was placed.
 * @param radius - Safe wander radius from the habitat probe; `0` pins it to the anchor.
 * @param seed - Per-animal phase offset, so a herd spreads instead of moving as one body.
 * @param t - Scene time in seconds.
 * @param speed - Radians per second along the path.
 */
export function grazePosition(
  anchor: GrazePoint,
  radius: number,
  seed: number,
  t: number,
  speed: number,
): GrazePoint {
  const a = t * speed + seed * 0.37;
  // Heading wanders rather than sweeping a clean circle; distance breathes between a tenth of the
  // radius and all of it, so the animal crosses its own patch instead of tracing the rim.
  const heading = a + Math.sin(a * 0.27 + seed) * 0.9;
  const distance = radius * (0.55 + 0.45 * Math.sin(a * 0.83 + 1.7));
  return {
    x: anchor.x + Math.cos(heading) * distance,
    z: anchor.z + Math.sin(heading) * distance,
  };
}

/**
 * Which way the animal should face at time `t`: toward where it will be shortly.
 *
 * Looking ahead rather than differentiating backwards means a model turns *into* its path. A
 * standing animal (`radius === 0`) has no path to face along, so it keeps the yaw it was placed
 * with — returning 0 there would snap every motionless deer to face north.
 */
export function grazeYaw(
  anchor: GrazePoint,
  radius: number,
  seed: number,
  t: number,
  speed: number,
  placedYaw: number,
  lookAhead = 0.35,
): number {
  if (radius <= 0) return placedYaw;
  const here = grazePosition(anchor, radius, seed, t, speed);
  const next = grazePosition(anchor, radius, seed, t + lookAhead, speed);
  return Math.atan2(next.x - here.x, next.z - here.z);
}
