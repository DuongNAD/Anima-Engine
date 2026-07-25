// Mock of @react-three/fiber for jsdom tests (and for the production tsc build's
// type resolution). The R3F reconciler cannot run under jsdom and pulls in a real
// WebGL context, so it is stubbed here. Real `three` is used everywhere else —
// its geometry/math classes run fine headless.
//
// The callback signatures below are typed (not bare `any`) so that components
// passing `useFrame((state, delta) => ...)` / `<Canvas onCreated={(state) => ...}>`
// get contextual types and don't trip `noImplicitAny` during `tsc` build.

export interface RootState {
  clock: { getElapsedTime: () => number; elapsedTime: number };
  scene: any;
  camera: any;
  gl: any;
  size: { width: number; height: number };
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

export const useThree = (): RootState => ({
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
    domElement:
      typeof document !== 'undefined'
        ? document.createElement('canvas')
        : ({} as any),
  },
  size: { width: 800, height: 600 },
});

export const extend = (_objects: any): void => {};
