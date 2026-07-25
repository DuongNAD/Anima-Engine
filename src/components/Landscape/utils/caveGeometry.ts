import * as THREE from 'three';
import type { World } from './worldGen';
import { sampleMeshHeight } from './worldSample';

// ---------------------------------------------------------------------------------------
// Real 3D cave-mouth geometry (replaces the old flat unlit decal in WorldCaves).
//
// Each cave site (World.cave*) becomes a rocky, noise-displaced FUNNEL: it flares open at the
// cliff (protruding a little along the outward/downhill bearing) and tapers back to a closed,
// dark pocket. Vertex colours darken toward the pocket so the interior reads dark while the rim
// keeps a lit rock tone; per-vertex jitter gives the walls a rough, rocky silhouette. All mouths
// are merged into ONE BufferGeometry → one draw call.
//
// HEIGHTMAP LIMITATION (honest note): the terrain is a single continuous heightmap mesh with no
// hole, so a fully walk-THROUGH tunnel bored into the hill would just be occluded by the cliff
// surface. A protruding concave niche is the occlusion-free way to render a believable cave mouth
// on this terrain representation. Truly enterable tunnels/overhangs need a voxel/SDF terrain
// (see WORLD_DESIGN.md, roadmap M4+).
// ---------------------------------------------------------------------------------------

const RINGS = 8; // rings from the closed pocket (apex) out to the mouth
const SEG = 12; // radial segments per ring

/** Deterministic [0,1) hash so a world always rebuilds the exact same caves. */
function hash(a: number, b: number, c: number): number {
  const s = Math.sin(a * 127.1 + b * 311.7 + c * 74.7) * 43758.5453;
  return s - Math.floor(s);
}

/**
 * Build one merged BufferGeometry holding every cave mouth for this world, or null if there are
 * no caves. `heightUnits` is the world-space height the normalized elevation maps to; `meshRes`
 * is the terrain mesh resolution so caves seat on the SAME surface the player sees.
 */
export function buildCavesGeometry(
  world: World,
  renderSize: number,
  heightUnits: number,
  meshRes: number,
): THREE.BufferGeometry | null {
  const count = world.caveCount ?? 0;
  if (count === 0 || !world.caveX) return null;
  const toWorld = renderSize / world.size;

  const positions: number[] = [];
  const colors: number[] = [];
  const indices: number[] = [];

  // Rock tones: lit rim → near-black pocket.
  const rim = [0.3, 0.26, 0.22];
  const deep = [0.03, 0.025, 0.02];

  for (let c = 0; c < count; c++) {
    const cellX = world.caveX[c];
    const cellZ = world.caveZ[c];
    const yaw = world.caveYaw[c];
    const depth = 2.2 + hash(c, 1, 0) * 1.9; // how far the mouth protrudes / recedes
    const mouthR = 1.3 + hash(c, 2, 0) * 0.9; // opening radius
    const sag = 0.4 + hash(c, 3, 0) * 0.7; // downward droop of the axis

    // Outward (downhill) bearing — the mouth faces out of the hill, matching the old decal.
    const ox = Math.cos(yaw);
    const oz = Math.sin(yaw);
    // Seat on the actual render-mesh surface (what the player walks/looks at), like the trees do.
    const u = cellX / (world.size - 1);
    const v = cellZ / (world.size - 1);
    const surfY = sampleMeshHeight(world, u, v, meshRes) * heightUnits;
    const sx = cellX * toWorld;
    const sz = cellZ * toWorld;

    // Axis: closed pocket sits just INTO the cliff; the flared mouth protrudes outward + a touch up.
    const tip = [sx - ox * 0.6, surfY + 0.25, sz - oz * 0.6];
    const mouth = [sx + ox * depth, surfY + 0.9, sz + oz * depth];
    let axx = mouth[0] - tip[0];
    let axy = mouth[1] - tip[1];
    let axz = mouth[2] - tip[2];
    const axl = Math.hypot(axx, axy, axz) || 1;
    axx /= axl;
    axy /= axl;
    axz /= axl;
    // A stable frame around the axis (right, up2) for placing ring vertices.
    let upx = 0;
    let upy = 1;
    const upz = 0;
    if (Math.abs(axy) > 0.9) {
      upx = 1;
      upy = 0;
    }
    // right = up × axis
    let rx = upy * axz - upz * axy;
    let ry = upz * axx - upx * axz;
    let rz = upx * axy - upy * axx;
    const rl = Math.hypot(rx, ry, rz) || 1;
    rx /= rl;
    ry /= rl;
    rz /= rl;
    // up2 = axis × right
    const ux = axy * rz - axz * ry;
    const uy = axz * rx - axx * rz;
    const uz = axx * ry - axy * rx;

    const base = positions.length / 3;
    // Apex (closed dark pocket).
    positions.push(tip[0], tip[1], tip[2]);
    colors.push(deep[0], deep[1], deep[2]);

    for (let k = 1; k <= RINGS; k++) {
      const t = k / RINGS;
      const r = mouthR * Math.pow(t, 0.65); // rounded flare, wide at the mouth
      // Centre of this ring: lerp tip→mouth with a downward parabolic droop for a natural hollow.
      const cxp = tip[0] + (mouth[0] - tip[0]) * t;
      const cyp = tip[1] + (mouth[1] - tip[1]) * t - Math.sin(t * Math.PI) * sag;
      const czp = tip[2] + (mouth[2] - tip[2]) * t;
      const shade = t; // 0 = pocket (dark), 1 = rim (lit)
      const cr = deep[0] + (rim[0] - deep[0]) * shade;
      const cg = deep[1] + (rim[1] - deep[1]) * shade;
      const cb = deep[2] + (rim[2] - deep[2]) * shade;
      for (let s = 0; s < SEG; s++) {
        const ang = (s / SEG) * Math.PI * 2;
        const ca = Math.cos(ang);
        const sa = Math.sin(ang);
        // Rocky wall: jitter the radius per vertex (deterministic).
        const jit = 1 + (hash(c, k * 13 + 5, s * 7 + 3) - 0.5) * 0.5;
        const rr = r * jit;
        const dirx = rx * ca + ux * sa;
        const diry = ry * ca + uy * sa;
        const dirz = rz * ca + uz * sa;
        positions.push(cxp + dirx * rr, cyp + diry * rr, czp + dirz * rr);
        // A little colour grain so the rock isn't a flat tone.
        const g = (hash(c, k, s) - 0.5) * 0.04;
        colors.push(cr + g, cg + g, cb + g);
      }
    }

    // Fan the apex to the first ring (closes the pocket).
    const ring1 = base + 1;
    for (let s = 0; s < SEG; s++) {
      const a = ring1 + s;
      const b = ring1 + ((s + 1) % SEG);
      indices.push(base, b, a);
    }
    // Quad strips between successive rings.
    for (let k = 0; k < RINGS - 1; k++) {
      const r0 = base + 1 + k * SEG;
      const r1 = base + 1 + (k + 1) * SEG;
      for (let s = 0; s < SEG; s++) {
        const s1 = (s + 1) % SEG;
        const a = r0 + s;
        const b = r0 + s1;
        const cc = r1 + s;
        const d = r1 + s1;
        indices.push(a, d, cc);
        indices.push(a, b, d);
      }
    }
  }

  const geo = new THREE.BufferGeometry();
  geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(positions), 3));
  geo.setAttribute('color', new THREE.BufferAttribute(new Float32Array(colors), 3));
  geo.setIndex(indices);
  geo.computeVertexNormals();
  return geo;
}
