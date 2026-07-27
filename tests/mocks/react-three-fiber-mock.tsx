// Mock of @react-three/fiber for jsdom tests (and for the production tsc build's
// type resolution). The R3F reconciler cannot run under jsdom and pulls in a real
// WebGL context, so it is stubbed here. Real `three` is used everywhere else —
// its geometry/math classes run fine headless.
//
// The callback signatures below are typed (not bare `any`) so that components
// passing `useFrame((state, delta) => ...)` / `<Canvas onCreated={(state) => ...}>`
// get contextual types and don't trip `noImplicitAny` during `tsc` build.
//
// Where a member exists because production code depends on its *shape* — the renderer's
// `getContext`/`render`, the store's `setFrameloop` — it is typed properly rather than left to the
// index signature. This file is what `tsc` sees for the whole app, so an `any` here is an unchecked
// call site in `src/`, not merely a loose test.

/**
 * The slice of `THREE.WebGLRenderer` the app touches.
 *
 * Structural and open: real three satisfies it, and the index signature keeps the many one-off
 * renderer fields (`shadowMap`, `toneMapping`, …) usable without enumerating them.
 */
export interface MockRenderer {
  setSize: (width: number, height: number) => void;
  domElement: HTMLCanvasElement;
  render: (scene: any, camera: any) => void;
  getContext: () => WebGLRenderingContext;
  [key: string]: any;
}

export interface RootState {
  clock: { getElapsedTime: () => number; elapsedTime: number };
  scene: any;
  camera: any;
  gl: MockRenderer;
  size: { width: number; height: number };
  /** Real R3F's frameloop control. Capture mode stops the loop with it before its final render. */
  setFrameloop: (frameloop?: 'always' | 'demand' | 'never') => void;
  [key: string]: any;
}

interface CanvasProps {
  children?: any;
  onCreated?: (state: RootState) => void;
  [key: string]: any;
}

export const Canvas = ({ children }: CanvasProps) => (
  <div data-testid="mock-canvas">{children}</div>
);

export const useFrame = (_callback: (state: RootState, delta: number) => void): void => {};

/** A fresh stub state. Built per call, so a test mutating one does not leak into the next. */
function mockState(): RootState {
  const canvas =
    typeof document !== 'undefined' ? document.createElement('canvas') : ({} as HTMLCanvasElement);
  return {
    clock: { getElapsedTime: () => 0, elapsedTime: 0 },
    scene: { fog: null, add: () => {}, remove: () => {} },
    camera: {
      position: { set: () => {}, x: 0, y: 0, z: 0 },
      lookAt: () => {},
      rotation: { set: () => {} },
      quaternion: { set: () => {} },
      rotateX: () => {},
      rotateY: () => {},
    },
    gl: {
      setSize: () => {},
      domElement: canvas,
      render: () => {},
      // jsdom has no WebGL, and nothing under jsdom reaches a path that uses the context: the
      // capture branch that calls this is gated on a `capture=1` URL the unit tests never set.
      getContext: () => ({}) as WebGLRenderingContext,
    },
    size: { width: 800, height: 600 },
    setFrameloop: () => {},
  };
}

// Real R3F's two forms. The selector form is the one production code should use — it is how a
// component subscribes to one field instead of the whole store — so the mock has to offer it or the
// type check pushes `src/` toward the worse API.
export function useThree(): RootState;
export function useThree<T>(selector: (state: RootState) => T): T;
export function useThree<T>(selector?: (state: RootState) => T): RootState | T {
  const state = mockState();
  return selector ? selector(state) : state;
}

export const extend = (_objects: any): void => {};
