// A complete r3f frame state, for tests that stub `useFrame` themselves.
//
// # Why this exists
//
// Several suites replace `@react-three/fiber` with their own inline `vi.mock` so they can capture
// the `useFrame` callbacks and drive them by hand. Each one then built the state to call those
// callbacks with, and each built a different, smaller one — usually `{ clock: { getElapsedTime } }`,
// because that was all the component under test happened to read at the time.
//
// That is a stub shaped like the test rather than like r3f, and it fails in the one direction that
// matters: production code moved to reading `state.scene` (the handle r3f documents for imperative
// work, and the one the React Compiler rules leave writable) and three suites went red with
// `Cannot read properties of undefined (reading 'fog')` — a crash the real renderer cannot produce,
// because `RootState.scene` is never undefined.
//
// So the shape lives in one place, typed against the same `RootState` the aliased mock exports, and
// a caller overrides only the fields its assertions are about.
//
// # Why the import is type-only
//
// `tests/vitest.config.ts` aliases `@react-three/fiber` to `react-three-fiber-mock.ts`, so a suite
// that calls `vi.mock('@react-three/fiber', factory)` replaces that module *by resolved path*. A
// runtime import of the mock from here would get the suite's factory instead of the real thing. The
// type import is erased before any of that can happen.

import type * as THREE from 'three';
import type { MockRenderer, RootState } from './react-three-fiber-mock';

/** What `useFrame` hands its callback. */
export type FrameCallback = (state: RootState, delta: number) => void;

/**
 * A frame state with every field r3f guarantees, overridable per test.
 *
 * The three objects are structural stand-ins, for the same reason the aliased mock's are: building
 * a real `THREE.Scene` drags the scene graph into every test that only wanted to advance a clock.
 */
export function makeFrameState(overrides: Partial<RootState> = {}): RootState {
  const canvas =
    typeof document !== 'undefined' ? document.createElement('canvas') : ({} as HTMLCanvasElement);
  const gl: MockRenderer = {
    setSize: () => {},
    domElement: canvas,
    render: () => {},
    getContext: () => ({}) as WebGLRenderingContext,
    shadowMap: { enabled: false, needsUpdate: false } as THREE.WebGLShadowMap,
  };
  return {
    clock: { getElapsedTime: () => 0, elapsedTime: 0 },
    scene: { fog: null, background: null, add: () => {}, remove: () => {} } as unknown as THREE.Scene,
    camera: {
      position: { set: () => {}, copy: () => {}, x: 0, y: 0, z: 0 },
      lookAt: () => {},
      rotation: { set: () => {} },
      quaternion: { set: () => {} },
      rotateX: () => {},
      rotateY: () => {},
      getWorldDirection: (v: THREE.Vector3) => v,
    } as unknown as THREE.Camera,
    gl,
    size: { width: 800, height: 600 },
    setFrameloop: () => {},
    setDpr: () => {},
    ...overrides,
  };
}

/** A frame state whose clock reads `elapsed`. The common single-field override. */
export function frameStateAt(elapsed: number): RootState {
  return makeFrameState({ clock: { getElapsedTime: () => elapsed, elapsedTime: elapsed } });
}
