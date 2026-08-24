// JSX typings for the react-three-fiber intrinsic elements this project uses.
//
// # Why these are declared here at all
//
// `@react-three/fiber` carries its own global JSX augmentation, and this project cannot use it:
// `tsconfig.json` aliases the package to `tests/mocks/react-three-fiber-mock.tsx` so that `tsc` and
// jsdom see a reconciler that does not need a GPU. The mock has no JSX augmentation, so without this
// file every `<mesh>` in the codebase is an unknown intrinsic element.
//
// # Why they are no longer `any`
//
// They used to be, all twenty-seven of them, with a comment saying so. That is not a small local
// looseness: `IntrinsicElements` is what types *the entire 3D scene*, so every prop on every element
// in every `World*` and `Landscape*` component was unchecked. It is also what forced the cast on
// every `null` argument — nineteen of them in `Vegetation.tsx` alone. `args` had no declared
// content, so nothing said what belonged there, and the casts were people working around that.
//
// The props below are the ones this codebase actually passes, typed against real three classes. That
// is a smaller surface than r3f's own `ThreeElements` and a truthful one: it is checked, it is what
// the scene uses, and adding a prop means writing it down rather than having it silently accepted.
//
// # The dashed props
//
// r3f lets you set a nested property with a dashed prop — `shadow-mapSize-width={2048}` assigns
// `light.shadow.mapSize.width`. That is open-ended by construction, so it gets a template-literal
// index signature. `unknown` rather than `any`: assigning one still requires the consumer to narrow,
// and r3f itself does the narrowing at runtime.

import type * as THREE from 'three';
import type * as React from 'react';

/** A three vector prop: r3f accepts the object, a tuple, or a scalar broadcast to all three axes. */
type Vector3Prop = THREE.Vector3 | [number, number, number] | number;
/** A three euler prop. */
type EulerProp = THREE.Euler | [number, number, number];
/** A three colour prop: r3f accepts anything `new THREE.Color()` accepts. */
type ColorProp = THREE.Color | string | number;

/** Nested assignment via a dashed prop, e.g. `shadow-mapSize-width`. */
interface DashedProps {
  [key: `${string}-${string}`]: unknown;
}

/** What every scene object accepts. */
interface Object3DProps<T> extends DashedProps {
  ref?: React.Ref<T>;
  key?: React.Key;
  children?: React.ReactNode;
  name?: string;
  position?: Vector3Prop;
  rotation?: EulerProp;
  scale?: Vector3Prop;
  visible?: boolean;
  castShadow?: boolean;
  receiveShadow?: boolean;
  frustumCulled?: boolean;
  renderOrder?: number;
  userData?: Record<string, unknown>;
  /** r3f attaches the object to a named property of its parent instead of adding it as a child. */
  attach?: string;
  /** Constructor arguments, forwarded positionally to the three class. */
  args?: readonly unknown[];
  onClick?: (event: THREE.Intersection & { stopPropagation(): void }) => void;
  onPointerOver?: (event: THREE.Intersection & { stopPropagation(): void }) => void;
  onPointerOut?: (event: THREE.Intersection & { stopPropagation(): void }) => void;
  /** Playwright and Testing Library reach elements by this. */
  'data-testid'?: string;
}

/** What every material accepts. Materials are attached, not positioned. */
interface MaterialProps<T> extends DashedProps {
  ref?: React.Ref<T>;
  key?: React.Key;
  children?: React.ReactNode;
  attach?: string;
  args?: readonly unknown[];
  color?: ColorProp;
  transparent?: boolean;
  opacity?: number;
  side?: THREE.Side;
  depthWrite?: boolean;
  depthTest?: boolean;
  toneMapped?: boolean;
  fog?: boolean;
  visible?: boolean;
  vertexColors?: boolean;
  wireframe?: boolean;
  alphaTest?: number;
  blending?: THREE.Blending;
  /** Depth-buffer bias, used here to stop coplanar decals z-fighting with the ground. */
  polygonOffset?: boolean;
  polygonOffsetFactor?: number;
  polygonOffsetUnits?: number;
  /** three calls this with the compiled shader before linking, for on-the-fly injection. */
  onBeforeCompile?: (
    shader: THREE.WebGLProgramParametersWithUniforms,
    renderer: THREE.WebGLRenderer,
  ) => void;
}

/** Geometry elements: constructed from `args`, attached to their parent mesh. */
interface GeometryProps<T> extends DashedProps {
  ref?: React.Ref<T>;
  key?: React.Key;
  children?: React.ReactNode;
  attach?: string;
  args?: readonly unknown[];
}

/**
 * A scene object that draws something, so it can be handed a geometry and a material directly
 * instead of declaring them as children.
 *
 * Both forms are used in this codebase: `<mesh><boxGeometry/><meshBasicMaterial/></mesh>` where the
 * children attach themselves, and `<mesh geometry={built} material={shared}/>` where a `useMemo`
 * built them once and several meshes share them.
 */
interface RenderableProps<T> extends Object3DProps<T> {
  geometry?: THREE.BufferGeometry;
  material?: THREE.Material | THREE.Material[];
}

// `declare global` because the `import type`s above make this file a module, and a module's
// `namespace JSX` is local to it. The augmentation has to reach every `.tsx` in the project.
declare global {
  namespace JSX {
    interface IntrinsicElements {
    // ---- objects ----------------------------------------------------------------------------
    mesh: RenderableProps<THREE.Mesh>;
    /**
     * `args` is `[geometry, material, count]`, forwarded to `new THREE.InstancedMesh(...)`.
     *
     * three accepts `null` for the first two and substitutes its own defaults, which is what lets a
     * component declare geometry and material as *children* and still put the count third. That is
     * the shape the scene uses everywhere, and typing it is what removed nineteen casts on `null`
     * arguments from `Vegetation.tsx`: they existed because `args` had no declared content.
     */
    instancedMesh: RenderableProps<THREE.InstancedMesh> & {
      args?: readonly [
        geometry: THREE.BufferGeometry | null,
        material: THREE.Material | null,
        count: number,
      ];
      count?: number;
    };
    group: Object3DProps<THREE.Group>;
    object3D: Object3DProps<THREE.Object3D>;
    points: RenderableProps<THREE.Points>;
    line: RenderableProps<THREE.Line>;
    lineSegments: RenderableProps<THREE.LineSegments>;
    lod: Object3DProps<THREE.LOD>;
    /**
     * Hands r3f an object built imperatively, rather than constructing one from `args`.
     *
     * Not restricted to `Object3D`: r3f attaches whatever it is given, and this codebase passes
     * geometries and materials built in a `useMemo` as well as whole meshes.
     */
    primitive: Object3DProps<THREE.Object3D> & {
      object: THREE.Object3D | THREE.BufferGeometry | THREE.Material | THREE.Texture;
    };

    // ---- lights -----------------------------------------------------------------------------
    ambientLight: Object3DProps<THREE.AmbientLight> & { intensity?: number; color?: ColorProp };
    directionalLight: Object3DProps<THREE.DirectionalLight> & {
      intensity?: number;
      color?: ColorProp;
      target?: THREE.Object3D;
    };
    hemisphereLight: Object3DProps<THREE.HemisphereLight> & {
      intensity?: number;
      color?: ColorProp;
      groundColor?: ColorProp;
    };
    spotLight: Object3DProps<THREE.SpotLight> & {
      intensity?: number;
      color?: ColorProp;
      angle?: number;
      penumbra?: number;
      distance?: number;
      decay?: number;
    };
    pointLight: Object3DProps<THREE.PointLight> & {
      intensity?: number;
      color?: ColorProp;
      distance?: number;
      decay?: number;
    };

    // ---- geometries -------------------------------------------------------------------------
    bufferGeometry: GeometryProps<THREE.BufferGeometry>;
    /** Declares one vertex attribute. `attach` names it, e.g. `attach="attributes-position"`. */
    bufferAttribute: GeometryProps<THREE.BufferAttribute> & {
      array?: ArrayLike<number>;
      count?: number;
      itemSize?: number;
      normalized?: boolean;
    };
    boxGeometry: GeometryProps<THREE.BoxGeometry>;
    sphereGeometry: GeometryProps<THREE.SphereGeometry>;
    planeGeometry: GeometryProps<THREE.PlaneGeometry>;
    cylinderGeometry: GeometryProps<THREE.CylinderGeometry>;
    coneGeometry: GeometryProps<THREE.ConeGeometry>;
    circleGeometry: GeometryProps<THREE.CircleGeometry>;
    dodecahedronGeometry: GeometryProps<THREE.DodecahedronGeometry>;

    // ---- materials --------------------------------------------------------------------------
    meshStandardMaterial: MaterialProps<THREE.MeshStandardMaterial> & {
      roughness?: number;
      metalness?: number;
      emissive?: ColorProp;
      emissiveIntensity?: number;
      map?: THREE.Texture | null;
      normalMap?: THREE.Texture | null;
      normalScale?: THREE.Vector2;
      roughnessMap?: THREE.Texture | null;
      flatShading?: boolean;
    };
    meshBasicMaterial: MaterialProps<THREE.MeshBasicMaterial> & {
      map?: THREE.Texture | null;
    };
    pointsMaterial: MaterialProps<THREE.PointsMaterial> & {
      size?: number;
      sizeAttenuation?: boolean;
      map?: THREE.Texture | null;
    };
    shaderMaterial: MaterialProps<THREE.ShaderMaterial> & {
      vertexShader?: string;
      fragmentShader?: string;
      uniforms?: Record<string, { value: unknown }>;
    };
    lineBasicMaterial: MaterialProps<THREE.LineBasicMaterial> & { linewidth?: number };

    // ---- scene ------------------------------------------------------------------------------
    fog: GeometryProps<THREE.Fog> & { attach?: 'fog'; color?: ColorProp; near?: number; far?: number };
    fogExp2: GeometryProps<THREE.FogExp2> & { attach?: 'fog'; color?: ColorProp; density?: number };

    // ---- registered via `extend` --------------------------------------------------------------
    /**
     * `OrbitControls` from three's examples, registered with `extend({ OrbitControls })`.
     *
     * `args` is `[camera, domElement]`. The control-specific props below are the ones the camera rig
     * sets; the rest of the class is reached through the ref it already holds.
     */
    orbitControls: Object3DProps<import('three/examples/jsm/controls/OrbitControls.js').OrbitControls> & {
      args?: readonly [camera: THREE.Camera, domElement: HTMLElement];
      enableDamping?: boolean;
      dampingFactor?: number;
      enableRotate?: boolean;
      enablePan?: boolean;
      enableZoom?: boolean;
      screenSpacePanning?: boolean;
      minDistance?: number;
      maxDistance?: number;
      maxPolarAngle?: number;
      minPolarAngle?: number;
      target?: THREE.Vector3;
    };
    }
  }
}
