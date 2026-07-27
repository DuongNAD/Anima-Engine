// The window properties the landscape scene publishes for tooling, and for its own HTML overlays.
//
// # Why they exist
//
// The 3D scene lives inside a `<Canvas>` and the minimap, the Playwright harnesses and the
// landscape tests all live outside it. Rather than thread a camera reference back out through React,
// `CameraControls` writes the live objects onto `window` each frame and everyone else reads them.
//
// # Why they are declared centrally
//
// `CameraControls` declared this shape privately and `Minimap` read the same properties off an
// untyped view of `window`, which is the arrangement where a rename on the writing side is
// discovered by a user. The members now live in the one global declaration this app keeps for
// window properties, `window-globals.d.ts`, so both sides compile against the same contract — and
// `unknown` on the two object-valued entries keeps a reader honest about narrowing what it got.
//
// The function below is what everything calls; it exists so the intent reads at the call site
// (`diagnostics().activeCamera`, not `window.activeCamera`) and so the surface has one doc comment.

/** Terrain-height sampler, in the legacy landscape's grid coordinates. */
export type TerrainHeightProbe = (x: number, z: number) => number;

/** Move the active camera's look-at target to a world XZ position. */
export type TeleportCameraTarget = (worldX: number, worldZ: number) => void;

/** The scene's published diagnostics surface. Every member is absent until the scene mounts. */
export function diagnostics(): Window {
  return window;
}

/** The minimum a reader needs off `activeCamera`: where it is. */
export interface CameraPositionReadout {
  position: { x: number; y: number; z: number };
}

/**
 * The live camera's position, or `null` when no scene has published one.
 *
 * A checked read rather than a cast: `activeCamera` is written by a component that may not be
 * mounted, and the minimap draws every animation frame whether it is or not.
 */
export function activeCameraPosition(): CameraPositionReadout['position'] | null {
  const camera = diagnostics().activeCamera;
  if (!camera || typeof camera !== 'object') return null;
  const { position } = camera as Partial<CameraPositionReadout>;
  if (!position || typeof position.x !== 'number' || typeof position.z !== 'number') return null;
  return position;
}
