// The window properties the landscape scene publishes for tooling, and for its own HTML overlays.
//
// # Why they exist
//
// The 3D scene lives inside a `<Canvas>` and the minimap, the Playwright harnesses and the
// landscape tests all live outside it. Rather than thread a camera reference back out through React,
// `CameraControls` writes the live objects onto `window` each frame and everyone else reads them.
//
// # Why they are declared here
//
// `CameraControls` declared this shape privately and `Minimap` read the same properties through
// `(window as any)`, which is the arrangement where a rename on the writing side is discovered by a
// user. One declaration, imported by both, makes the two sides the same contract — and `unknown` on
// the two object-valued entries keeps a reader honest about narrowing what it got.

/** Terrain-height sampler, in the legacy landscape's grid coordinates. */
export type TerrainHeightProbe = (x: number, z: number) => number;

/** Move the active camera's look-at target to a world XZ position. */
export type TeleportCameraTarget = (worldX: number, worldZ: number) => void;

/** The scene's published diagnostics surface. Every member is absent until the scene mounts. */
export interface DiagnosticsWindow extends Window {
  getTerrainHeight?: TerrainHeightProbe;
  globalTerrainHeightMap?: Float32Array;
  teleportCameraTarget?: TeleportCameraTarget;
  /** The live `THREE.Camera`. `unknown` because the readers here only need its `position`. */
  activeCamera?: unknown;
  /** The live `THREE.Scene`, for the Playwright harnesses. */
  activeScene?: unknown;
}

/** The scene's diagnostics surface on the current window. */
export function diagnostics(): DiagnosticsWindow {
  return window as DiagnosticsWindow;
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
