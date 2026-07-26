import * as THREE from 'three';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';
import { FloraType } from './worldGen';

// ---------------------------------------------------------------------------------------
// The authored shape of every flora type, in its own module.
//
// Moved out of `WorldVegetation.tsx` (unchanged — this is a lift, not a redesign) so that the
// footprint a tree actually occupies can be **measured** rather than asserted.
//
// `floraClearance.ts` publishes a per-type canopy radius that the spawn picker and the canonical
// view capture keep the camera outside of. That number has to agree with the geometry below, and
// the only trustworthy way to check is to build the geometry and measure its bounding box —
// `scripts/check_flora_footprint.mjs` does exactly that, with real three, and fails when the two
// disagree. Reading the source for `0.95` and hoping is what let the numbers drift in the first
// place.
//
// `floraClearance.ts` deliberately does NOT import this module: it must stay usable from
// `worldSample.ts` and from Node scripts that never touch three.
// ---------------------------------------------------------------------------------------

/** Fills a geometry's vertex colours with one flat colour (so merged parts keep two tones). */
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
  return geom;
}

const TRUNK = '#6b4a2f';

/**
 * One merged low-poly geometry per flora type, base sitting on y=0. Trunks and canopies are
 * separate vertex-colour tones inside a single geometry, so each type still renders as ONE
 * InstancedMesh (two-tone without extra draw calls).
 */
export function makeFloraGeometry(type: FloraType): THREE.BufferGeometry {
  const parts: THREE.BufferGeometry[] = [];
  switch (type) {
    case FloraType.Pine: {
      parts.push(paint(new THREE.CylinderGeometry(0.09, 0.13, 0.5, 5).translate(0, 0.25, 0), TRUNK));
      parts.push(paint(new THREE.ConeGeometry(0.55, 1.1, 6).translate(0, 0.95, 0), '#2f6b3d'));
      parts.push(paint(new THREE.ConeGeometry(0.38, 0.8, 6).translate(0, 1.6, 0), '#3a7c49'));
      break;
    }
    case FloraType.Round: {
      parts.push(paint(new THREE.CylinderGeometry(0.1, 0.15, 0.6, 5).translate(0, 0.3, 0), TRUNK));
      parts.push(paint(new THREE.SphereGeometry(0.62, 6, 5).translate(0, 1.05, 0), '#3f8a3a'));
      parts.push(paint(new THREE.SphereGeometry(0.38, 5, 4).translate(0.34, 1.32, 0.18), '#4d9c46'));
      break;
    }
    case FloraType.Jungle: {
      parts.push(paint(new THREE.CylinderGeometry(0.09, 0.13, 1.15, 5).translate(0, 0.575, 0), '#5d4028'));
      parts.push(
        paint(new THREE.SphereGeometry(0.78, 7, 5).scale(1, 0.55, 1).translate(0, 1.35, 0), '#1d6b2e'),
      );
      parts.push(
        paint(new THREE.SphereGeometry(0.5, 6, 4).scale(1, 0.55, 1).translate(0.25, 1.66, 0.12), '#2a7d3c'),
      );
      break;
    }
    case FloraType.Cactus: {
      parts.push(paint(new THREE.CylinderGeometry(0.2, 0.24, 1.2, 6).translate(0, 0.6, 0), '#5f8a4a'));
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.09, 0.11, 0.55, 5).rotateZ(Math.PI / 2.6).translate(0.3, 0.78, 0),
          '#547e42',
        ),
      );
      break;
    }
    case FloraType.Acacia: {
      // Umbrella-canopy savanna tree: tall thin trunk, flat spreading top.
      parts.push(paint(new THREE.CylinderGeometry(0.07, 0.11, 1.05, 5).translate(0, 0.52, 0), TRUNK));
      parts.push(paint(new THREE.CylinderGeometry(0.95, 0.5, 0.34, 7).translate(0, 1.25, 0), '#7ca03e'));
      break;
    }
    case FloraType.Palm: {
      parts.push(paint(new THREE.CylinderGeometry(0.08, 0.12, 1.0, 5).translate(0, 0.5, 0), '#7d6243'));
      parts.push(
        paint(new THREE.CylinderGeometry(0.06, 0.08, 0.75, 5).rotateZ(-0.16).translate(0.1, 1.32, 0), '#7d6243'),
      );
      // Fronds: flattened cones splayed outward from the crown.
      for (let k = 0; k < 5; k++) {
        const a = (k / 5) * Math.PI * 2;
        parts.push(
          paint(
            new THREE.ConeGeometry(0.1, 0.85, 4)
              .rotateX(Math.PI / 2.15)
              .rotateY(a)
              .translate(0.16 + Math.cos(a) * 0.3, 1.72, Math.sin(a) * 0.3),
            k % 2 ? '#2e7d3f' : '#35914a',
          ),
        );
      }
      break;
    }
    case FloraType.DeadTree: {
      parts.push(paint(new THREE.CylinderGeometry(0.06, 0.11, 1.15, 5).translate(0, 0.57, 0), '#6e5b48'));
      parts.push(
        paint(new THREE.CylinderGeometry(0.03, 0.05, 0.55, 4).rotateZ(0.75).translate(0.2, 0.98, 0), '#665441'),
      );
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.025, 0.045, 0.45, 4).rotateZ(-0.9).rotateY(1.2).translate(-0.16, 0.8, 0.05),
          '#665441',
        ),
      );
      break;
    }
    case FloraType.Bush: {
      parts.push(paint(new THREE.SphereGeometry(0.4, 5, 4).translate(0, 0.34, 0), '#5d7c3a'));
      parts.push(paint(new THREE.SphereGeometry(0.27, 5, 4).translate(0.26, 0.28, 0.1), '#6b8a42'));
      break;
    }
    case FloraType.Reed: {
      parts.push(paint(new THREE.CylinderGeometry(0.02, 0.035, 0.95, 4).translate(0, 0.47, 0), '#9aa86a'));
      parts.push(
        paint(new THREE.CylinderGeometry(0.018, 0.03, 0.8, 4).rotateZ(0.12).translate(0.09, 0.4, 0.04), '#8c9c5e'),
      );
      parts.push(
        paint(new THREE.CylinderGeometry(0.018, 0.03, 0.7, 4).rotateZ(-0.14).translate(-0.08, 0.35, -0.03), '#a5b273'),
      );
      break;
    }
    case FloraType.Tuft: {
      // Neutral pale base: the per-instance tint decides whether this tuft reads as grass
      // (most) or as a wildflower (a scattered few) — see the tint logic in TypedInstances.
      parts.push(paint(new THREE.ConeGeometry(0.17, 0.38, 5).translate(0, 0.19, 0), '#d2dcaa'));
      break;
    }
    case FloraType.Coral: {
      // Branching head over a mound; PALE base — the per-instance tint paints the reef
      // pink / orange / purple / red (see the tint logic in TypedInstances).
      parts.push(paint(new THREE.SphereGeometry(0.28, 5, 4).translate(0, 0.16, 0), '#cfc4bb'));
      parts.push(paint(new THREE.CylinderGeometry(0.05, 0.08, 0.6, 4).rotateZ(0.35).translate(0.12, 0.45, 0), '#d8cec6'));
      parts.push(
        paint(new THREE.CylinderGeometry(0.045, 0.07, 0.5, 4).rotateZ(-0.5).rotateY(1.1).translate(-0.12, 0.4, 0.06), '#d3c8bf'),
      );
      parts.push(paint(new THREE.CylinderGeometry(0.04, 0.06, 0.42, 4).rotateX(0.4).translate(0, 0.42, -0.12), '#dcd2c9'));
      break;
    }
    case FloraType.Kelp: {
      parts.push(paint(new THREE.ConeGeometry(0.09, 2.0, 4).translate(0, 1.0, 0), '#4a6b35'));
      parts.push(paint(new THREE.ConeGeometry(0.07, 1.6, 4).rotateZ(0.14).translate(0.18, 0.8, 0.05), '#557a3c'));
      parts.push(paint(new THREE.ConeGeometry(0.06, 1.3, 4).rotateZ(-0.16).translate(-0.15, 0.65, -0.04), '#42612f'));
      break;
    }
    case FloraType.Seagrass: {
      parts.push(paint(new THREE.ConeGeometry(0.1, 0.6, 4).translate(0, 0.3, 0), '#4f8a68'));
      parts.push(paint(new THREE.ConeGeometry(0.08, 0.45, 4).rotateZ(0.2).translate(0.1, 0.22, 0.04), '#5c9a74'));
      break;
    }
    case FloraType.Rock:
    default: {
      parts.push(paint(new THREE.DodecahedronGeometry(0.6, 0), '#8a847e'));
      break;
    }
  }
  const merged = mergeGeometries(parts, false) ?? parts[0];
  parts.forEach((p) => p !== merged && p.dispose());
  return merged;
}

/**
 * Largest horizontal distance from the instance origin reached by any vertex of `type`, at unit
 * instance scale. This is the number `FLORA_CANOPY_UNIT_RADIUS` must match; measuring it needs
 * real three, so it lives here and is checked by a script gate rather than a jsdom test (both
 * Vitest configs alias `three` to a mock).
 */
export function measureFloraFootprintRadius(type: FloraType): number {
  const geom = makeFloraGeometry(type);
  const pos = geom.attributes.position;
  let maxR2 = 0;
  for (let i = 0; i < pos.count; i++) {
    const x = pos.getX(i);
    const z = pos.getZ(i);
    const r2 = x * x + z * z;
    if (r2 > maxR2) maxR2 = r2;
  }
  geom.dispose();
  return Math.sqrt(maxR2);
}
