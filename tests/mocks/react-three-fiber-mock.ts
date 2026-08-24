// Mock of @react-three/fiber for jsdom tests (and for the production tsc build's type resolution).
// The R3F reconciler cannot run under jsdom and pulls in a real WebGL context, so it is stubbed here.
// Real `three` is used everywhere else — its geometry/math classes run fine headless.
//
// # Why this is `.ts` and not `.tsx`
//
// It exports a component (`Canvas`) alongside three non-components (`useFrame`, `useThree`,
// `extend`), which is exactly the shape `react-refresh/only-export-components` exists to flag: a
// module Fast Refresh cannot hot-swap cleanly. Splitting it would mean two files behind one alias,
// and the alias can only name one. Building the element with `React.createElement` instead of JSX
// costs one line of clarity and takes the file out of the rule's scope honestly — a mock does not
// need JSX sugar, and nothing about it is hot-reloaded.
//
// # Why the types are real
//
// This file is what `tsc` sees for the whole app, so a loose type here is an unchecked call site in
// `src/`, not merely a loose test. `RootState` and `MockRenderer` name the members production code
// actually reaches, typed against real three classes; the open-ended remainder is `unknown`, which
// still forces a consumer to narrow.
//
// # Why `three` is imported for its values
//
// The scene and the camera below are real `THREE.Scene` and `THREE.PerspectiveCamera` instances,
// not object literals describing them. Only the R3F reconciler is mocked — `three` itself runs
// headless, its geometry and math classes need no GL context — so there was no reason to
// impersonate two classes that were available. Impersonating them cost something real: the literals
// carried whichever members production happened to read when each was written, and when a component
// moved to `state.scene.background` three suites failed with `Cannot read properties of undefined`,
// a crash the real renderer cannot produce.
//
// This file is only ever *typed* into the production build — `vite.config.ts` aliases
// `@react-three/fiber` under `mode === 'test'` alone — so nothing here is bundled.

import * as React from 'react';
import * as THREE from 'three';

/**
 * The slice of `THREE.WebGLRenderer` the app touches.
 *
 * Open at the end: the renderer has a large surface and components reach for one-off fields
 * (`shadowMap`, `toneMapping`). `unknown` keeps those honest — reading one requires narrowing.
 */
export interface MockRenderer {
  setSize: (width: number, height: number) => void;
  domElement: HTMLCanvasElement;
  render: (scene: THREE.Scene, camera: THREE.Camera) => void;
  getContext: () => WebGLRenderingContext;
  /**
   * Toggled live by the quality preset, without remounting the GL context.
   *
   * The two flags, not the whole `WebGLShadowMap`: that object is built by a `WebGLRenderer` against
   * a live context and cannot be constructed on its own, so a mock claiming to be one could only
   * ever be a cast. Naming what the app writes is the same policy the rest of this interface
   * follows, and it means a component reaching for a third field is a compile error here.
   */
  shadowMap: Pick<THREE.WebGLShadowMap, 'enabled' | 'needsUpdate'>;
  [key: string]: unknown;
}

/** The r3f store, as far as this project reads it. */
export interface RootState {
  clock: { getElapsedTime: () => number; elapsedTime: number };
  scene: THREE.Scene;
  camera: THREE.Camera;
  gl: MockRenderer;
  size: { width: number; height: number };
  /** Real r3f's frameloop control. Capture mode stops the loop with it before its final render. */
  setFrameloop: (frameloop?: 'always' | 'demand' | 'never') => void;
  /** Sets the render scale. The quality preset drives it between 1 and the clamped device ratio. */
  setDpr: (dpr: number) => void;
  [key: string]: unknown;
}

/** Props `<Canvas>` accepts here. Only `onCreated` and `children` have behaviour. */
export interface CanvasProps {
  children?: React.ReactNode;
  onCreated?: (state: RootState) => void;
  [key: string]: unknown;
}

export const Canvas = ({ children }: CanvasProps): React.ReactElement =>
  React.createElement('div', { 'data-testid': 'mock-canvas' }, children);

export const useFrame = (_callback: (state: RootState, delta: number) => void): void => {};

/**
 * A fresh stub state. Built per call, so a test mutating one does not leak into the next.
 *
 * The scene and camera are real three instances. Building them costs a handful of small allocations
 * and buys the thing a literal could not: every member the real classes have, behaving as they do,
 * so a component that starts reading a new one keeps working instead of crashing on `undefined`.
 */
function mockState(): RootState {
  return {
    clock: { getElapsedTime: () => 0, elapsedTime: 0 },
    scene: new THREE.Scene(),
    camera: new THREE.PerspectiveCamera(),
    gl: {
      setSize: () => {},
      // A real element: both vitest configs run under jsdom, and the camera rig adds listeners to
      // this and calls `requestPointerLock` on it.
      domElement: document.createElement('canvas'),
      render: () => {},
      // jsdom has no WebGL, and nothing under jsdom reaches a path that uses the context: the
      // capture branch that calls this is gated on a `capture=1` URL the unit tests never set.
      // Throwing states that rather than handing back an empty object dressed as a context — if the
      // assumption ever stops holding, this says so instead of failing several frames later.
      getContext: () => {
        throw new Error(
          'the r3f mock has no WebGL context: nothing running under jsdom should reach one',
        );
      },
      shadowMap: { enabled: false, needsUpdate: false },
    },
    size: { width: 800, height: 600 },
    setFrameloop: () => {},
    setDpr: () => {},
  };
}

// Real r3f's two forms. The selector form is the one production code should use — it is how a
// component subscribes to one field instead of the whole store — so the mock has to offer it or the
// type check pushes `src/` toward the worse API.
export function useThree(): RootState;
export function useThree<T>(selector: (state: RootState) => T): T;
export function useThree<T>(selector?: (state: RootState) => T): RootState | T {
  const state = mockState();
  return selector ? selector(state) : state;
}

/** Registers three classes as JSX intrinsics. A no-op here; `r3f-intrinsics.d.ts` has the types. */
export const extend = (_objects: Record<string, unknown>): void => {};
