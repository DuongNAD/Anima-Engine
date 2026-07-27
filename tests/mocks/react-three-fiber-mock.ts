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
// This file is what `tsc` sees for the whole app, so an `any` here is an unchecked call site in
// `src/`, not merely a loose test. `RootState` and `MockRenderer` name the members production code
// actually reaches, typed against real three classes; the open-ended remainder is `unknown`, which
// still forces a consumer to narrow.

import * as React from 'react';
import type * as THREE from 'three';

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
  /** Toggled live by the quality preset, without remounting the GL context. */
  shadowMap: THREE.WebGLShadowMap;
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
 * The three objects are structural stand-ins rather than real instances: constructing a real
 * `THREE.Scene` here would be honest too, but it drags the whole scene graph into every render of
 * every component that only wanted to read `size`. The casts are narrow and each one is a stub
 * standing in for a class jsdom cannot host.
 */
function mockState(): RootState {
  const canvas =
    typeof document !== 'undefined' ? document.createElement('canvas') : ({} as HTMLCanvasElement);
  return {
    clock: { getElapsedTime: () => 0, elapsedTime: 0 },
    scene: { fog: null, add: () => {}, remove: () => {} } as unknown as THREE.Scene,
    camera: {
      position: { set: () => {}, x: 0, y: 0, z: 0 },
      lookAt: () => {},
      rotation: { set: () => {} },
      quaternion: { set: () => {} },
      rotateX: () => {},
      rotateY: () => {},
    } as unknown as THREE.Camera,
    gl: {
      setSize: () => {},
      domElement: canvas,
      render: () => {},
      // jsdom has no WebGL, and nothing under jsdom reaches a path that uses the context: the
      // capture branch that calls this is gated on a `capture=1` URL the unit tests never set.
      getContext: () => ({}) as WebGLRenderingContext,
      shadowMap: { enabled: false, needsUpdate: false } as THREE.WebGLShadowMap,
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
