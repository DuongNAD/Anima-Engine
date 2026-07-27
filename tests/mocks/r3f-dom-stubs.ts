// The 3D properties these suites graft onto jsdom elements, named instead of cast away.
//
// # What is going on in those suites
//
// `@react-three/fiber` is replaced by an inline `vi.mock` whose `Canvas` is a plain `<div>`, so
// every `<mesh>`, `<bufferGeometry>` and `<instancedMesh>` under it renders as an *unknown HTML
// element*. jsdom happily creates one, but it has none of the three API a component then reaches
// for. So each suite installs the missing pieces on `Element.prototype` / `HTMLElement.prototype`
// in `beforeEach` and removes them again in `afterEach`.
//
// That much is reasonable — it is how you observe imperative three writes without a GPU. What was
// not reasonable is that the observation was then done through an untyped view of the element:
// forty-odd of those across four files, each one a place where a test could read a property no
// component ever writes and pass anyway. The types here say exactly which grafted members exist, so
// a typo is a compile error and the set of things the doubles pretend to be is written down in one
// place.
//
// # How to use them
//
// Compose only what a suite installs:
//
//     const body = screen.getByTestId('rabbit-body') as HTMLElement & StubTransform;
//     expect(body.position.y).toBeGreaterThan(0);
//
// An intersection with `HTMLElement` keeps every cast a single, ordinary downcast — not a trip
// through `unknown`, which would be the same hole with more punctuation.

import type { Mock } from 'vitest';
import type * as THREE from 'three';

/** A three vector, as the stubs model it. */
export interface StubVec3 {
  x: number;
  y: number;
  z: number;
}

/**
 * Transform writes observed as spy calls rather than as resulting values.
 *
 * Two suites need the difference. Reading `position.x` after the fact answers "where did it end
 * up"; `position.set` as a `Mock` answers "was it written at all, and with what" — which is the
 * question when the assertion is `not.toHaveBeenCalled()` for a payload the component should have
 * rejected.
 */
export interface StubSpiedTransform {
  position: { set: Mock };
  rotation: { set: Mock };
  scale: { set: Mock };
}

/** Material colour writes, likewise observed as spy calls. */
export interface StubSpiedMaterial {
  material: { color: { setRGB: Mock; set: Mock } };
}

/** Both of the above: what an element standing in for a rendered `<mesh>` carries. */
export type StubSpiedMesh = StubSpiedTransform & StubSpiedMaterial;

/** Elements a component positions, rotates or scales. */
export interface StubTransform {
  position: StubVec3;
  rotation: StubVec3;
  scale: StubVec3;
}

/** A three colour, as the stubs model it. */
export interface StubColor {
  r: number;
  g: number;
  b: number;
  setRGB(r: number, g: number, b: number): void;
}

/** Elements whose material a component recolours. */
export interface StubMaterialHolder {
  material: { color: StubColor };
}

/** One vertex attribute, as the geometry stub returns it. */
export interface StubBufferAttribute {
  array: Float32Array;
  needsUpdate: boolean;
}

/** Elements whose geometry a component reads attributes off. */
export interface StubGeometryHolder {
  geometry: { getAttribute(name: string): StubBufferAttribute | null };
}

/**
 * Attributes captured from `setAttribute`.
 *
 * r3f's `<bufferAttribute attach="attributes-position" .../>` becomes a `setAttribute` call on the
 * unknown element, with a real `THREE.BufferAttribute` as the value. jsdom would stringify it; the
 * suites intercept and keep the object so a test can assert on the actual vertex data.
 */
export interface StubAttributeCapture {
  _capturedAttributes: Map<string, THREE.BufferAttribute>;
}

/** `<lod>` elements: the level list a component builds. */
export interface StubLod {
  levels: unknown[];
  addLevel(object: unknown, distance: number): void;
}

/** `<instancedMesh>` elements: the per-instance matrix writes a component makes. */
export interface StubInstanced {
  setMatrixAt(index: number, matrix: THREE.Matrix4): void;
}

/** `<shaderMaterial>` elements: the uniform block a component drives. */
export interface StubUniforms {
  uniforms: Record<string, { value: unknown }>;
}

/**
 * Remove a property a suite grafted onto a prototype.
 *
 * Deleting `position` off `Element.prototype` in an `afterEach` is the single most repeated untyped
 * view in these files, and it is there only because `delete` demands an optional property.
 * `Reflect` asks the same question of the object rather than of its type.
 */
export function removeStubbedProperty(prototype: object, name: string): void {
  Reflect.deleteProperty(prototype, name);
}

/** Remove several at once — the shape every one of these `afterEach` blocks actually has. */
export function removeStubbedProperties(prototype: object, ...names: string[]): void {
  for (const name of names) removeStubbedProperty(prototype, name);
}
