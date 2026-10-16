import * as THREE from 'three';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';

// -------------------------------------------------------------------------------------------------
// Map 2 Wildlife Geometries (Low-Poly Procedural Silhouettes & Merged Meshes)
// -------------------------------------------------------------------------------------------------

function paint(geom: THREE.BufferGeometry, hex: string): THREE.BufferGeometry {
  const c = new THREE.Color(hex);
  const count = geom.attributes.position.count;
  const colors = new Float32Array(count * 3);
  for (let i = 0; i < count; i++) {
    colors[i * 3] = c.r;
    colors[i * 3 + 1] = c.g;
    colors[i * 3 + 2] = c.b;
  }
  geom.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  if (!geom.attributes.normal) geom.computeVertexNormals();
  return geom;
}

function mergeGeom(parts: THREE.BufferGeometry[]): THREE.BufferGeometry {
  parts.forEach((p) => {
    if (!p.attributes.normal) p.computeVertexNormals();
  });
  const merged = mergeGeometries(parts, false) ?? parts[0];
  parts.forEach((p) => {
    if (p !== merged) p.dispose();
  });
  merged.computeVertexNormals();
  return merged;
}

/**
 * 3D Bird Geometry: Shallow-V wingspan silhouette.
 */
export function makeBirdGeometry(): THREE.BufferGeometry {
  const geom = new THREE.BufferGeometry();
  const verts = new Float32Array([
    0, 0, 0.45, 0, 0, -0.28, -1.2, 0.4, -0.08,
    0, 0, 0.45, 1.2, 0.4, -0.08, 0, 0, -0.28,
  ]);
  geom.setAttribute('position', new THREE.BufferAttribute(verts, 3));
  geom.computeVertexNormals();
  return geom;
}

/**
 * Fish Geometry: Flat side-view silhouette (body triangle + tail fin).
 */
export function makeFishGeometry(): THREE.BufferGeometry {
  const geom = new THREE.BufferGeometry();
  const verts = new Float32Array([
    0.5, 0, 0, -0.32, 0.16, 0, -0.32, -0.14, 0,
    -0.3, 0, 0, -0.58, 0.18, 0, -0.58, -0.18, 0,
  ]);
  geom.setAttribute('position', new THREE.BufferAttribute(verts, 3));
  geom.computeVertexNormals();
  return geom;
}

/**
 * Low-poly 4-legged Deer with two-tone vertex colors.
 */
export function makeDeerGeometry(): THREE.BufferGeometry {
  const legs: THREE.BufferGeometry[] = [];
  for (const lx of [-0.3, 0.3]) {
    for (const lz of [-0.14, 0.14]) {
      legs.push(paint(new THREE.CylinderGeometry(0.045, 0.04, 0.55, 4).translate(lx, 0.275, lz), '#604229'));
    }
  }
  return mergeGeom([
    paint(new THREE.BoxGeometry(0.9, 0.45, 0.38).translate(0, 0.72, 0), '#785434'),
    ...legs,
    paint(new THREE.CylinderGeometry(0.06, 0.1, 0.45, 4).rotateZ(-0.65).translate(0.52, 1.05, 0), '#785434'),
    paint(new THREE.BoxGeometry(0.26, 0.16, 0.14).translate(0.72, 1.24, 0), '#6c4a2c'),
  ]);
}

/**
 * Alpine Horned Goat with white/cream body and dark horns.
 */
export function makeGoatGeometry(): THREE.BufferGeometry {
  const legs: THREE.BufferGeometry[] = [];
  for (const lx of [-0.26, 0.26]) {
    for (const lz of [-0.12, 0.12]) {
      legs.push(paint(new THREE.CylinderGeometry(0.04, 0.035, 0.48, 4).translate(lx, 0.24, lz), '#44403c'));
    }
  }
  return mergeGeom([
    paint(new THREE.BoxGeometry(0.75, 0.4, 0.34).translate(0, 0.62, 0), '#f1f5f9'),
    ...legs,
    paint(new THREE.CylinderGeometry(0.055, 0.085, 0.38, 4).rotateZ(-0.6).translate(0.44, 0.88, 0), '#f1f5f9'),
    paint(new THREE.BoxGeometry(0.22, 0.14, 0.12).translate(0.6, 1.04, 0), '#e2e8f0'),
    paint(new THREE.ConeGeometry(0.025, 0.22, 4).rotateZ(-0.35).translate(0.56, 1.18, 0.05), '#292524'),
    paint(new THREE.ConeGeometry(0.025, 0.22, 4).rotateZ(-0.35).translate(0.56, 1.18, -0.05), '#292524'),
  ]);
}

/**
 * Mallard Duck floating on water surface.
 */
export function makeDuckGeometry(): THREE.BufferGeometry {
  return mergeGeom([
    paint(new THREE.SphereGeometry(0.24, 6, 5).scale(1.35, 0.75, 0.9).translate(0, 0.09, 0), '#854d0e'),
    paint(new THREE.SphereGeometry(0.11, 5, 4).translate(0.26, 0.28, 0), '#15803d'),
    paint(new THREE.ConeGeometry(0.05, 0.16, 4).rotateZ(-Math.PI / 2).translate(0.38, 0.26, 0), '#eab308'),
  ]);
}

/**
 * Meadow Butterfly with bright wings.
 */
export function makeButterflyGeometry(): THREE.BufferGeometry {
  const geom = new THREE.BufferGeometry();
  const verts = new Float32Array([
    0, 0, 0.1, 0, 0, -0.1, -0.3, 0.15, 0,
    0, 0, 0.1, 0.3, 0.15, 0, 0, 0, -0.1,
  ]);
  geom.setAttribute('position', new THREE.BufferAttribute(verts, 3));
  const colors = new Float32Array(6 * 3).fill(1);
  geom.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  geom.computeVertexNormals();
  return geom;
}
