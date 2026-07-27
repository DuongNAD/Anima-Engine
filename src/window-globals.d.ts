// Everything this app writes onto `window`, declared in one place.
//
// # Why a declaration instead of a cast
//
// `window` is typed with exactly the DOM's own members, so every one of these properties used to be
// reached through a widening cast at each site — `(window as Record<string, unknown>)[FLAG]` to
// write, a differently-shaped one to read. A cast is not a contract: the writer in
// `WorldShowcase.tsx` and the reader in `canonical_views.spec.ts` each described the same property
// separately, and nothing checked that the two descriptions agreed. A rename on one side would have
// been found by a capture that silently timed out.
//
// Declaring them here is the truthful version of the same claim — these properties really are on
// `window` while the scene is mounted — and it is checked: the flag constants below are string
// literals, so `window[CAPTURE_READY_FLAG]` resolves to the member declared here or fails to
// compile.
//
// # Why every member is optional
//
// None of them exists before the scene mounts, and two of them never exist outside a capture run.
// `?` is what forces a reader to handle the ordinary case where the scene is not up.

import type * as THREE from 'three';
import type { World } from './components/Landscape/utils/worldGen';
import type {
  TerrainHeightProbe,
  TeleportCameraTarget,
} from './components/Landscape/utils/diagnosticsWindow';

declare global {
  interface Window {
    // ---- the landscape scene's diagnostics surface -------------------------------------------
    //
    // The 3D scene lives inside a `<Canvas>` while the minimap, the Playwright harnesses and the
    // landscape tests all live outside it. Rather than thread a camera reference back out through
    // React, `CameraControls` writes the live objects here each frame and everyone else reads them.

    /** Terrain-height sampler, in the legacy landscape's grid coordinates. */
    getTerrainHeight?: TerrainHeightProbe;
    globalTerrainHeightMap?: Float32Array;
    teleportCameraTarget?: TeleportCameraTarget;
    /** The live `THREE.Camera`. `unknown` because the readers only need its `position`. */
    activeCamera?: unknown;
    /** The live `THREE.Scene`, for the Playwright harnesses. */
    activeScene?: unknown;

    // ---- the world showcase's tooling hooks ----------------------------------------------------

    /** The generated world, for tooling and for the canonical-view capture's preflight. */
    __world?: World | null;
    /** The showcase's scene graph, published on `onCreated` for the same readers. */
    __worldScene?: THREE.Scene;

    // ---- the canonical-view capture handshake --------------------------------------------------
    //
    // Both names are held as exported constants — `CAPTURE_READY_FLAG` in `captureMode.ts` and
    // `MAP_EVIDENCE_GLOBAL` in `mapEvidence.ts` — and both sides index `window` with the constant
    // rather than retyping the string, so the constant, this declaration and every use stay one
    // thing.

    /**
     * Set once, by `CaptureReadySignal`, after the frame loop has stopped and the final render has
     * run. The capture harness polls it; observing it means the drawing buffer holds a frame
     * nothing will overwrite.
     */
    __animaCaptureReady?: boolean;
    /**
     * The navigation/collision evidence record the harness injects before the page loads.
     *
     * `unknown` deliberately: it arrives as parsed JSON from a file on disk, so nothing has checked
     * it yet. `readInjectedEvidence()` is where it becomes a `MapEvidenceRecord` or `null`.
     */
    __animaMapEvidence?: unknown;
  }
}
