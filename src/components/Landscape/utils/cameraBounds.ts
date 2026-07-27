// One place that decides how far a camera may travel from the middle of the world.
//
// # Why this is a module and not two numbers in the rig
//
// It used to be `const lim = mode === 'walk' ? renderSize * 0.5 : renderSize * 0.75` inside
// `WorldCameraRig`'s frame callback, written in response to a defect found by looking at a
// screenshot. Before that single line it was one number, `0.75`, for every mode — and that was a
// boundary escape: the terrain mesh spans ±renderSize/2, `surfaceY` returns sea level for anything
// off the mesh, so a walker could carry on for another quarter of a world across open water with no
// ground beneath them.
//
// The fix was right and untested, which is the same problem one step along: a number chosen by
// looking at a picture, in a `useFrame` body, is not reachable by a test. So the policy moved here.
//
// # Why the two limits differ
//
// They answer different questions.
//
//   walk       "where may a person stand?"   Only on the terrain. The mesh footprint, exactly.
//   spectator  "from where may the world be photographed?"  Framing the whole continent at the
//              scene's 55° FOV needs the camera *outside* the footprint — `overview` sits 950
//              render units out over a 1200-unit map. A spectator camera off the mesh is not a
//              defect; it is the only way to see the mesh.
//
// So `walk` gets the footprint and `orbit`/`fly`/`top`/`cine` get a wider framing limit. The bug was
// giving the walker the photographer's allowance.

/** Camera behaviours the rig implements. Mirrors `WorldCameraRig`'s `CameraMode`. */
export type BoundedCameraMode = 'orbit' | 'fly' | 'walk' | 'top' | 'cine';

/**
 * Half-extent of the terrain mesh as a fraction of `renderSize`.
 *
 * `WorldTerrain` builds a `renderSize × renderSize` plane centred on the origin, so the mesh
 * occupies `[-renderSize/2, +renderSize/2]` on both axes. This is that geometry, not a preference.
 */
export const TERRAIN_FOOTPRINT_FRACTION = 0.5;

/**
 * Half-extent a spectator camera may reach, as a fraction of `renderSize`.
 *
 * 0.75 leaves room to pull back far enough to frame the whole map — the binding case is `overview`
 * at 950 render units on a 1200-unit world — without letting an orbit camera wander into empty
 * space where the world is a distant speck.
 */
export const SPECTATOR_FRAMING_FRACTION = 0.75;

/** Modes whose camera represents a person standing on the ground. */
const GROUNDED_MODES: readonly BoundedCameraMode[] = ['walk'];

/** True when `mode` puts the camera on the terrain rather than above/outside it. */
export function isGroundedMode(mode: BoundedCameraMode): boolean {
  return GROUNDED_MODES.includes(mode);
}

/**
 * The half-extent `mode` may reach on either horizontal axis, in render units.
 *
 * @param mode - Camera behaviour.
 * @param renderSize - Scene extent (the terrain plane is `renderSize` across).
 */
export function horizontalLimit(mode: BoundedCameraMode, renderSize: number): number {
  const fraction = isGroundedMode(mode) ? TERRAIN_FOOTPRINT_FRACTION : SPECTATOR_FRAMING_FRACTION;
  return renderSize * fraction;
}

/** Clamp one coordinate into `[-limit, limit]`. */
function clamp1(v: number, limit: number): number {
  return Math.max(-limit, Math.min(limit, v));
}

/**
 * Clamp a horizontal position to what `mode` is allowed to reach.
 *
 * Returns a new object rather than mutating: the rig assigns straight onto `camera.position`, and a
 * function that quietly edited its argument would be one refactor away from clamping the wrong
 * vector.
 */
export function clampToCameraBounds(
  mode: BoundedCameraMode,
  renderSize: number,
  x: number,
  z: number,
): { x: number; z: number } {
  const lim = horizontalLimit(mode, renderSize);
  return { x: clamp1(x, lim), z: clamp1(z, lim) };
}

/**
 * True when `(x, z)` is over the terrain mesh — the region where `surfaceY` returns real ground.
 *
 * This is the predicate a walk-confinement test asserts against. Outside it `sampleMeshHeight` has
 * no data and the rig falls back to sea level, which is what made the escape invisible: the camera
 * kept a plausible height while standing on nothing.
 */
export function isOverTerrain(renderSize: number, x: number, z: number): boolean {
  const half = renderSize * TERRAIN_FOOTPRINT_FRACTION;
  return Math.abs(x) <= half && Math.abs(z) <= half;
}
