// What the sea floor does past the edge of the world, and why the world used to have a visible one.
//
// # The defect
//
// The ocean shader reads the terrain height under each fragment to work out the water's depth, and
// the heightmap only covers the terrain footprint. Outside it, the shader multiplied the (clamped,
// therefore smeared) sample by a binary inside/outside step:
//
//     float inMap  = step(abs(uv.x - 0.5), 0.5) * step(abs(uv.y - 0.5), 0.5);
//     float floorY = texture2D(uHeightMap, clamp(uv, 0.0, 1.0)).r * inMap;
//
// One texel inside the boundary the floor is a coastal shelf a few units below the surface; one texel
// outside it is zero — the abyss. Depth therefore jumps the full water column across a single texel,
// and everything derived from depth jumps with it: the colour snaps from `uShallow` to `uDeep` and the
// alpha from 0.32 to 0.92. The result is a hard turquoise-to-navy line, straight, on all four sides,
// exactly where the world ends.
//
// This is a **rendering defect in the shipped scene**, not a property of the canonical captures. Any
// elevated camera sees it — orbit, fly, top — and three of the eight canonical views were rejected on
// it. It had previously been read as "the frame contains the world boundary, so move the camera",
// which is why the camera solver in `viewFraming.ts` exists; the solver is right for close-up subjects
// and could never fix `overview`, whose subject *is* the whole bounded world. There is no camera pose
// that photographs a bounded world without its boundary. There is a sea floor that does not fall off a
// cliff at it.
//
// # The fix
//
// The floor descends from the border height to abyssal over a band, instead of in one texel. The ramp
// is exactly 1 at the boundary, so the sea floor is continuous with the terrain it continues from, and
// reaches 0 well inside the fog distance. What a viewer sees is a continental shelf falling away into
// deep water — which is also what this world is.
//
// The clamp's original failure mode stays fixed. Where land touches the border, the smeared land
// height now decays to zero across the band rather than persisting to infinity, so the "sky-coloured
// hole fanning across the horizon" cannot re-form: past the band the floor is abyssal everywhere.

/**
 * Width of the shelf band, as a fraction of `renderSize`.
 *
 * A quarter of the world. Two constraints fix it from opposite sides: too short and the ramp is still
 * a visible line, just a softer one; too long and the water stays shallow-coloured far past any land,
 * which reads as a flooded plain rather than an ocean. A quarter puts the far end of the ramp at 300
 * render units on the shipped 1200-unit world, against a fog that starts at 960 — so the transition
 * finishes on its own terms and is not being hidden by haze.
 */
export const OCEAN_SHELF_BAND_FRACTION = 0.25;

/** Hermite smoothstep, matching GLSL's. */
function smoothstep(edge0: number, edge1: number, x: number): number {
  const t = Math.min(1, Math.max(0, (x - edge0) / (edge1 - edge0)));
  return t * t * (3 - 2 * t);
}

/**
 * How much of the sampled terrain height survives at a point `outside` units past the footprint.
 *
 * The reference implementation of the shader's `shelf` term. Kept here so the property that matters —
 * continuity at the boundary — is asserted by a test rather than by reading GLSL, and so the constant
 * above has somewhere to be explained. `oceanShelf.test.ts` also scans the shader source, because a
 * reference implementation that has drifted from the shader is worse than none.
 *
 * @param outside - Distance past the terrain footprint, render units. 0 or less means inside.
 * @param band - Width of the falloff, render units.
 * @returns 1 at and inside the boundary, falling smoothly to 0 at `band`.
 */
export function oceanShelfFalloff(outside: number, band: number): number {
  return 1 - smoothstep(0, band, Math.max(outside, 0));
}

/**
 * Distance from `(x, z)` to the terrain footprint, in render units; 0 inside it.
 *
 * Chebyshev rather than Euclidean, because the footprint is a square: the distance to a square's
 * boundary along either axis is what decides when the heightmap stopped having data.
 */
export function distanceOutsideFootprint(x: number, z: number, renderSize: number): number {
  return Math.max(Math.max(Math.abs(x), Math.abs(z)) - renderSize / 2, 0);
}
