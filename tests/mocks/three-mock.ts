const makeProxy = (): any => {
  const f = function(this: any) {};
  const p: any = new Proxy(f, {
    get: (_target, name) => {
      if (name === 'default' || name === '__esModule') return p;
      return p;
    },
    construct: (_target, args) => {
      const instance: any = new Proxy({
        r: typeof args[0] === 'number' ? args[0] : undefined,
        g: typeof args[1] === 'number' ? args[1] : undefined,
        b: typeof args[2] === 'number' ? args[2] : undefined,
        x: typeof args[0] === 'number' ? args[0] : undefined,
        y: typeof args[1] === 'number' ? args[1] : undefined,
        z: typeof args[2] === 'number' ? args[2] : undefined,
      }, {
        get: (t, prop) => {
          if (prop in t) {
            return (t as any)[prop];
          }
          if (prop === 'setRGB' || prop === 'set' || prop === 'clone' || prop === 'copy' || prop === 'add' || prop === 'remove') {
            return () => instance;
          }
          return p;
        }
      });
      return instance;
    }
  });
  return p;
};

const THREE = makeProxy();

export default THREE;
export const SphereGeometry = THREE;
export const EdgesGeometry = THREE;
export const Group = THREE;
export const Mesh = THREE;
export const Vector3 = THREE;
export const Euler = THREE;
export const Color = THREE;
export const MeshBasicMaterial = THREE;
export const MeshStandardMaterial = THREE;
export const AmbientLight = THREE;
export const DirectionalLight = THREE;
export const GridHelper = THREE;
export const Scene = THREE;
export const PerspectiveCamera = THREE;
export const WebGLRenderer = THREE;
export const Clock = THREE;

// Export types to satisfy type usage
export type Group = any;
export type Mesh = any;
export type SphereGeometry = any;
export type EdgesGeometry = any;
export type Vector3 = any;
export type Euler = any;
export type Color = any;
export type MeshBasicMaterial = any;
export type MeshStandardMaterial = any;
export type AmbientLight = any;
export type DirectionalLight = any;
export type GridHelper = any;
export type Scene = any;
export type PerspectiveCamera = any;
export type WebGLRenderer = any;
export type Clock = any;
