// Mapping between the simulation's world coordinates and the landscape's render coordinates.
//
// # Two spaces for one world, and why the factor is not a magic number
//
// The backend simulates inside `MapBounds::default()` — `[-100, 100]` on x and z, a span of 200
// world units. The landscape renders the same world across `RENDER_SIZE` units, 1200 today. So a
// position crossing the IPC boundary has to be rescaled by `renderSize / 200`, which is 6 at the
// current size.
//
// That factor has already been got wrong once in this repository, in the opposite direction: the map
// manifest published flora collider radii straight out of the render-space formula while declaring
// positions in the canonical 200-unit bounds, making every trunk six times too fat to the review
// gates (`floraClearance.ts`). Same two spaces, same six. Naming the extent here rather than writing
// `* 6` anywhere is what keeps the next change of `RENDER_SIZE` from silently teleporting the
// population.

/**
 * Span of the simulation's world on x and z, in world units.
 *
 * Mirrors `MapBounds::default()` in `src-tauri/src/core/resources.rs`: `min = (-100, 0, -100)`,
 * `max = (100, 10, 100)`. If that default ever changes, this must change with it — a mismatch does
 * not fail, it just draws the population at the wrong scale, which looks like a physics bug.
 */
export const SIM_BOUNDS_EXTENT = 200;

/** Half the simulation extent: the coordinate of the map edge in world units. */
export const SIM_BOUNDS_HALF = SIM_BOUNDS_EXTENT / 2;

/**
 * Convert one simulation axis coordinate into render units.
 *
 * @param world - Coordinate in `[-100, 100]`, as the backend publishes it.
 * @param renderSize - The scene's `RENDER_SIZE`.
 */
export function simToRender(world: number, renderSize: number): number {
  return (world * renderSize) / SIM_BOUNDS_EXTENT;
}

/**
 * Convert a render-space coordinate back into simulation units.
 *
 * The inverse exists so a camera position can be expressed as a question about the simulation —
 * "which agents are near where the player is standing" — without restating the ratio at the call
 * site and getting it upside down.
 */
export function renderToSim(render: number, renderSize: number): number {
  return (render * SIM_BOUNDS_EXTENT) / renderSize;
}

/**
 * Whether a published position is inside the simulation's own bounds.
 *
 * A coordinate outside them is not something to draw at the edge of the map: it means the publisher
 * and this consumer disagree about the space, and drawing it anyway hides that. Callers skip such
 * instances rather than clamping them.
 */
export function isInsideSimBounds(x: number, z: number): boolean {
  return (
    Number.isFinite(x) &&
    Number.isFinite(z) &&
    Math.abs(x) <= SIM_BOUNDS_HALF &&
    Math.abs(z) <= SIM_BOUNDS_HALF
  );
}
