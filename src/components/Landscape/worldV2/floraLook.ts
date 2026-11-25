import * as THREE from 'three';
import { mergeGeometries, mergeVertices } from 'three/examples/jsm/utils/BufferGeometryUtils.js';

// prototype, to be replaced — WV2-LOOK flora shapes for World v2 decorative flora.
//
// Separate from `utils/floraGeometry.ts` on purpose: that module's footprints are measured by
// `check:flora-footprint` and feed the walk-mode colliders/spawn clearance. These shapes are
// decoration only, built at unit height (y = 0..1) and scaled per instance to real-world
// metres, so they never change a collider.
//
// Budget: 18-34 triangles per plant. A plant is a few pixels tall at the canonical view
// distances, so silhouette, size and colour variation matter far more than facets.

export const LOOK_SPECIES = {
  Pine: 0,
  Broadleaf: 1,
  Jungle: 2,
  Cactus: 3,
  Rock: 4,
  Acacia: 5,
  Palm: 6,
  Bush: 7,
} as const;

/** Height range in metres per bake species id (0..7), and the canopy width / height ratio. */
export const LOOK_SPECIES_SIZE: ReadonlyArray<{ minM: number; maxM: number; widthRatio: number }> = [
  { minM: 14, maxM: 32, widthRatio: 0.42 }, // 0 pine / spruce
  { minM: 10, maxM: 24, widthRatio: 0.8 }, // 1 broadleaf
  { minM: 20, maxM: 38, widthRatio: 0.75 }, // 2 jungle emergent
  { minM: 4, maxM: 9, widthRatio: 0.45 }, // 3 cactus
  { minM: 3, maxM: 11, widthRatio: 1.4 }, // 4 boulder
  { minM: 6, maxM: 12, widthRatio: 1.5 }, // 5 acacia
  { minM: 9, maxM: 18, widthRatio: 0.7 }, // 6 palm
  { minM: 2.5, maxM: 5.5, widthRatio: 1.5 }, // 7 shrub
];

function seededRandom(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

/** Paint a vertical gradient: darker at the base of the part (self-shadow), lighter on top. */
function paintGradient(
  geom: THREE.BufferGeometry,
  bottomHex: string,
  topHex: string,
  y0: number,
  y1: number,
): THREE.BufferGeometry {
  const g = geom.index ? geom.toNonIndexed() : geom;
  if (g !== geom) geom.dispose();
  const a = new THREE.Color(bottomHex);
  const b = new THREE.Color(topHex);
  const pos = g.attributes.position;
  const colors = new Float32Array(pos.count * 3);
  const c = new THREE.Color();
  for (let i = 0; i < pos.count; i += 1) {
    const t = THREE.MathUtils.clamp((pos.getY(i) - y0) / Math.max(1e-4, y1 - y0), 0, 1);
    c.copy(a).lerp(b, t);
    colors[i * 3] = c.r;
    colors[i * 3 + 1] = c.g;
    colors[i * 3 + 2] = c.b;
  }
  g.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  return g;
}

/** Jitter vertices radially (shared positions move together, so the mesh stays closed). */
function jitter(geom: THREE.BufferGeometry, amount: number, seed: number): THREE.BufferGeometry {
  const rnd = seededRandom(seed);
  const pos = geom.attributes.position;
  const moved = new Map<string, [number, number, number]>();
  for (let i = 0; i < pos.count; i += 1) {
    const key = `${pos.getX(i).toFixed(4)},${pos.getY(i).toFixed(4)},${pos.getZ(i).toFixed(4)}`;
    let d = moved.get(key);
    if (!d) {
      d = [(rnd() - 0.5) * amount, (rnd() - 0.5) * amount, (rnd() - 0.5) * amount];
      moved.set(key, d);
    }
    pos.setXYZ(i, pos.getX(i) + d[0], pos.getY(i) + d[1], pos.getZ(i) + d[2]);
  }
  pos.needsUpdate = true;
  return geom;
}

/** Normals pointing out of a crown centre: soft volumetric foliage shading instead of facets. */
function crownNormals(geom: THREE.BufferGeometry, cx: number, cy: number, cz: number, up = 0.35): void {
  const pos = geom.attributes.position;
  const normals = new Float32Array(pos.count * 3);
  const v = new THREE.Vector3();
  for (let i = 0; i < pos.count; i += 1) {
    v.set(pos.getX(i) - cx, pos.getY(i) - cy, pos.getZ(i) - cz).normalize();
    v.y += up;
    v.normalize();
    normals[i * 3] = v.x;
    normals[i * 3 + 1] = v.y;
    normals[i * 3 + 2] = v.z;
  }
  geom.setAttribute('normal', new THREE.BufferAttribute(normals, 3));
}

function trunk(h: number, r: number, hex: string): THREE.BufferGeometry {
  const g = new THREE.CylinderGeometry(r * 0.7, r, h, 3, 1, true).translate(0, h / 2, 0);
  const out = paintGradient(g, hex, hex, 0, h);
  out.computeVertexNormals();
  return out;
}

function finish(parts: THREE.BufferGeometry[]): THREE.BufferGeometry {
  for (const p of parts) {
    if (p.attributes.uv) p.deleteAttribute('uv');
  }
  const merged = mergeGeometries(parts, false) ?? parts[0];
  for (const p of parts) if (p !== merged) p.dispose();
  // Re-index: the parts are flattened for per-vertex colour, which left every triangle with
  // three private vertices. Shared corners (same position, normal, colour) collapse again, so
  // ~100k instanced plants run the vertex shader 2-4x less often.
  const indexed = mergeVertices(merged, 1e-4);
  if (indexed !== merged) merged.dispose();
  return indexed;
}

/** Unit-height (y 0..1) plant geometry, canopy radius ~0.5 * widthRatio, for bake species id. */
export function makeLookFloraGeometry(speciesId: number): THREE.BufferGeometry {
  const w = LOOK_SPECIES_SIZE[speciesId]?.widthRatio ?? 0.8;
  switch (speciesId) {
    case LOOK_SPECIES.Pine: {
      // Layered spire: three open cones, slightly offset, dark at the skirt.
      const r = w * 0.5;
      const c1 = new THREE.ConeGeometry(r, 0.55, 6, 1, true).translate(0, 0.06 + 0.275, 0);
      const c2 = new THREE.ConeGeometry(r * 0.74, 0.5, 6, 1, true).rotateY(0.5).translate(0.01, 0.34 + 0.25, 0);
      const c3 = new THREE.ConeGeometry(r * 0.46, 0.42, 5, 1, true).rotateY(1.1).translate(0, 0.58 + 0.21, 0.01);
      const parts = [c1, c2, c3].map((g) => {
        const p = paintGradient(g, '#0d1c12', '#1d3520', 0.06, 1.0);
        crownNormals(p, 0, 0.45, 0, 0.6);
        return p;
      });
      return finish([trunk(0.12, 0.035, '#3b2b1e'), ...parts]);
    }
    case LOOK_SPECIES.Broadleaf: {
      const r = w * 0.5;
      const main = jitter(new THREE.IcosahedronGeometry(r, 0), r * 0.28, 11)
        .scale(1, 0.78, 1)
        .translate(0, 0.62, 0);
      const side = jitter(new THREE.OctahedronGeometry(r * 0.62, 0), r * 0.2, 12).translate(r * 0.45, 0.48, r * 0.2);
      const a = paintGradient(main, '#16290f', '#304a1d', 0.3, 1.0);
      const b = paintGradient(side, '#142610', '#2a421a', 0.25, 0.8);
      crownNormals(a, 0, 0.55, 0);
      crownNormals(b, r * 0.45, 0.45, r * 0.2);
      return finish([trunk(0.4, 0.04, '#3e2e20'), a, b]);
    }
    case LOOK_SPECIES.Jungle: {
      const r = w * 0.5;
      const top = jitter(new THREE.IcosahedronGeometry(r, 0), r * 0.25, 21).scale(1, 0.45, 1).translate(0, 0.84, 0);
      const mid = jitter(new THREE.OctahedronGeometry(r * 0.7, 0), r * 0.2, 22).scale(1, 0.55, 1).translate(-r * 0.4, 0.62, r * 0.25);
      const a = paintGradient(top, '#0f2612', '#26461d', 0.65, 1.0);
      const b = paintGradient(mid, '#0d2010', '#1d3a18', 0.45, 0.8);
      crownNormals(a, 0, 0.75, 0);
      crownNormals(b, -r * 0.4, 0.55, r * 0.25);
      return finish([trunk(0.75, 0.035, '#4a3826'), a, b]);
    }
    case LOOK_SPECIES.Acacia: {
      const r = w * 0.5;
      const crown = jitter(new THREE.IcosahedronGeometry(r, 0), r * 0.22, 31).scale(1, 0.3, 1).translate(0, 0.86, 0);
      const a = paintGradient(crown, '#2c3616', '#4c5826', 0.74, 1.0);
      crownNormals(a, 0, 0.7, 0, 0.8);
      return finish([trunk(0.78, 0.035, '#4a3a2a'), a]);
    }
    case LOOK_SPECIES.Palm: {
      const parts: THREE.BufferGeometry[] = [trunk(0.88, 0.03, '#6b5a44')];
      for (let k = 0; k < 5; k += 1) {
        const ang = (k / 5) * Math.PI * 2 + 0.3;
        const frond = new THREE.PlaneGeometry(0.1, 0.42, 1, 1)
          .translate(0, 0.21, 0)
          .rotateX(-1.05)
          .rotateY(ang)
          .translate(0, 0.86, 0);
        const p = paintGradient(frond, '#1c3a16', '#355626', 0.7, 0.95);
        crownNormals(p, 0, 0.7, 0, 1.0);
        parts.push(p);
      }
      return finish(parts);
    }
    case LOOK_SPECIES.Cactus: {
      const body = new THREE.CylinderGeometry(0.07, 0.08, 1, 5, 1, true).translate(0, 0.5, 0);
      const arm = new THREE.CylinderGeometry(0.045, 0.05, 0.35, 4, 1, true).translate(0.13, 0.62, 0);
      const a = paintGradient(body, '#3a5230', '#5d7a45', 0, 1);
      const b = paintGradient(arm, '#3a5230', '#5d7a45', 0.45, 0.8);
      a.computeVertexNormals();
      b.computeVertexNormals();
      return finish([a, b]);
    }
    case LOOK_SPECIES.Rock: {
      const r = w * 0.5;
      const a = jitter(new THREE.IcosahedronGeometry(r, 0), r * 0.45, 41).scale(1, 0.55, 0.85).translate(0, r * 0.22, 0);
      const b = jitter(new THREE.OctahedronGeometry(r * 0.55, 0), r * 0.3, 42).scale(1, 0.6, 1).translate(r * 0.7, r * 0.1, r * 0.35);
      const pa = paintGradient(a, '#4a4743', '#77726a', -r * 0.2, r * 0.6);
      const pb = paintGradient(b, '#45423e', '#6e695f', -r * 0.2, r * 0.4);
      pa.computeVertexNormals();
      pb.computeVertexNormals();
      return finish([pa, pb]);
    }
    case LOOK_SPECIES.Bush:
    default: {
      const r = w * 0.5;
      const a = jitter(new THREE.OctahedronGeometry(r, 0), r * 0.3, 51).scale(1, 0.6, 1).translate(0, 0.38, 0);
      const b = jitter(new THREE.OctahedronGeometry(r * 0.7, 0), r * 0.25, 52).scale(1, 0.6, 1).translate(r * 0.5, 0.28, -r * 0.3);
      const pa = paintGradient(a, '#1e2912', '#3a4820', 0.0, 0.8);
      const pb = paintGradient(b, '#1c2811', '#36451e', 0.0, 0.6);
      crownNormals(pa, 0, 0.2, 0, 0.6);
      crownNormals(pb, r * 0.5, 0.15, -r * 0.3, 0.6);
      return finish([pa, pb]);
    }
  }
}
