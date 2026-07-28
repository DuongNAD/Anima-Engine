import * as THREE from 'three';
import { merged, paint } from './lowPoly';

// Bodies for the creatures the simulation is actually running.
//
// `LiveAgents` drew every published segment as a sphere. That was the right first step — a sphere
// proves the positions crossed the IPC boundary and landed on the terrain — and the wrong thing to
// leave in place, because a thousand red spheres in a grid tell a viewer nothing about what the
// population *is*. A shape does: prey read as prey at a glance, a predator reads as a predator, and
// two prey of different builds read as two kinds of animal rather than two instances of one.
//
// Shared rather than private to one component: `WorldWildlife` and `WorldFauna` draw the same kinds
// of animal for scenery, and two low-poly deer that disagree about their proportions look like a
// bug in the renderer rather than a variation in the world.

/** Low-poly deer pointing +X (body, four legs, neck, head). */
export function makeDeer(): THREE.BufferGeometry {
  const legs: THREE.BufferGeometry[] = [];
  for (const lx of [-0.28, 0.28]) {
    for (const lz of [-0.12, 0.12]) {
      legs.push(paint(new THREE.CylinderGeometry(0.045, 0.04, 0.5, 4).translate(lx, 0.25, lz), '#74553a'));
    }
  }
  return merged([
    paint(new THREE.BoxGeometry(0.85, 0.4, 0.36).translate(0, 0.66, 0), '#8a6844'),
    ...legs,
    paint(new THREE.CylinderGeometry(0.06, 0.09, 0.42, 4).rotateZ(-0.65).translate(0.5, 0.95, 0), '#8a6844'),
    paint(new THREE.BoxGeometry(0.24, 0.15, 0.13).translate(0.68, 1.12, 0), '#7c5c3c'),
  ]);
}

/** Rabbit: small crouched body with two upright ears, pointing +X. */
export function makeRabbit(): THREE.BufferGeometry {
  return merged([
    paint(new THREE.SphereGeometry(0.14, 5, 4).scale(1.4, 0.9, 0.95).translate(0, 0.14, 0), '#9c8d7a'),
    paint(new THREE.SphereGeometry(0.09, 5, 4).translate(0.16, 0.24, 0), '#a89a86'),
    paint(new THREE.BoxGeometry(0.03, 0.16, 0.05).translate(0.15, 0.38, 0.05), '#8d7f6d'),
    paint(new THREE.BoxGeometry(0.03, 0.16, 0.05).translate(0.15, 0.38, -0.05), '#8d7f6d'),
    paint(new THREE.SphereGeometry(0.05, 4, 3).translate(-0.19, 0.16, 0), '#efeae0'),
  ]);
}

/** Lion: heavy shoulders, mane, low head — the big predator silhouette. */
export function makeLion(): THREE.BufferGeometry {
  const legs: THREE.BufferGeometry[] = [];
  for (const lx of [-0.26, 0.26]) {
    for (const lz of [-0.14, 0.14]) {
      legs.push(paint(new THREE.CylinderGeometry(0.055, 0.05, 0.34, 4).translate(lx, 0.17, lz), '#a87f45'));
    }
  }
  return merged([
    paint(new THREE.BoxGeometry(0.8, 0.34, 0.36).translate(0, 0.5, 0), '#c1954f'),
    ...legs,
    // The mane is the whole reason a lion reads as a lion at fifty metres.
    paint(new THREE.SphereGeometry(0.24, 6, 5).scale(1, 1, 0.9).translate(0.44, 0.58, 0), '#6f4a1f'),
    paint(new THREE.BoxGeometry(0.22, 0.18, 0.2).translate(0.58, 0.56, 0), '#c1954f'),
    paint(new THREE.CylinderGeometry(0.025, 0.02, 0.42, 4).rotateZ(0.9).translate(-0.5, 0.6, 0), '#a87f45'),
  ]);
}

/** Wildcat: lean, low, long tail — a smaller hunter than the lion. */
export function makeWildcat(): THREE.BufferGeometry {
  const legs: THREE.BufferGeometry[] = [];
  for (const lx of [-0.18, 0.18]) {
    for (const lz of [-0.09, 0.09]) {
      legs.push(paint(new THREE.CylinderGeometry(0.032, 0.028, 0.24, 4).translate(lx, 0.12, lz), '#6d6357'));
    }
  }
  return merged([
    paint(new THREE.BoxGeometry(0.52, 0.2, 0.22).translate(0, 0.32, 0), '#8a7c68'),
    ...legs,
    paint(new THREE.SphereGeometry(0.12, 5, 4).scale(1, 0.95, 0.95).translate(0.32, 0.38, 0), '#8a7c68'),
    paint(new THREE.ConeGeometry(0.04, 0.09, 4).translate(0.3, 0.5, 0.06), '#6d6357'),
    paint(new THREE.ConeGeometry(0.04, 0.09, 4).translate(0.3, 0.5, -0.06), '#6d6357'),
    paint(new THREE.CylinderGeometry(0.022, 0.016, 0.46, 4).rotateZ(1.15).translate(-0.34, 0.4, 0), '#7b6f5e'),
  ]);
}
