import * as THREE from 'three';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';

// Shared low-poly building blocks for the world's creatures.
//
// `WorldWildlife` grew these three privately; `WorldFauna` needs the same three. Two copies of a
// vertex-colour writer is how two families of animals end up subtly differently lit.

/** Deterministic hash in [0, 1). Placement uses it instead of `Math.random` so every visit finds the
 * same animals in the same haunts — and so a frozen capture clock produces a frozen, reproducible
 * world rather than a different one each run. */
export function hash01(n: number): number {
  let h = Math.imul(n + 1, 374761393);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

/** Fill a geometry's vertex colours with one flat colour, so merged parts keep their own tone. */
export function paint(geom: THREE.BufferGeometry, hex: string): THREE.BufferGeometry {
  const c = new THREE.Color(hex);
  const count = geom.attributes.position.count;
  const colors = new Float32Array(count * 3);
  for (let i = 0; i < count; i++) {
    colors[i * 3] = c.r;
    colors[i * 3 + 1] = c.g;
    colors[i * 3 + 2] = c.b;
  }
  geom.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  return geom;
}

/** Merge painted parts into one geometry and dispose the parts, so a species is one draw call. */
export function merged(parts: THREE.BufferGeometry[]): THREE.BufferGeometry {
  const m = mergeGeometries(parts, false) ?? parts[0];
  parts.forEach((p) => p !== m && p.dispose());
  return m;
}
