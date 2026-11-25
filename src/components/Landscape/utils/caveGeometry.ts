import * as THREE from 'three';
import type { World } from './worldGen';
import { sampleMeshHeight } from './worldSample';

// ---------------------------------------------------------------------------------------
// Real 3D cave-mouth geometry (`AMEND W3-lite`).
//
// Each cave site (World.cave*) becomes a prominently framed cliffside rocky arch:
//   - Uses proper centred UV mapping `(cell + size * 0.5) / size` matching WorldVegetation/terrain
//     so mouths always seat directly on the exposed bare-rock cliff face.
//   - Deep pitch-black arched recess + protruding craggy stone collar rim so openings read
//     clearly from cliffside distances as dark shadowed cave entrances (still decoration only).
//   - All mouths and stone rims merge into ONE BufferGeometry -> one draw call.
// ---------------------------------------------------------------------------------------

const RINGS = 8; // rings from the closed pocket (apex) out to the mouth
const COLLAR_RINGS = 2; // stone rim collar flaring around the entrance lip and tucking into the cliff
const SEG = 14; // radial segments per ring

/** Deterministic [0,1) hash so a world always rebuilds the exact same caves. */
function hash(a: number, b: number, c: number): number {
  const s = Math.sin(a * 127.1 + b * 311.7 + c * 74.7) * 43758.5453;
  return s - Math.floor(s);
}

function addArchRing(
  positions: number[],
  colors: number[],
  cxp: number,
  cyp: number,
  czp: number,
  rx: number,
  ry: number,
  rz: number,
  ux: number,
  uy: number,
  uz: number,
  radX: number,
  radY: number,
  cr: number,
  cg: number,
  cb: number,
  caveIdx: number,
  ringIdx: number,
  jitAmp: number,
): void {
  for (let s = 0; s < SEG; s++) {
    const ang = (s / SEG) * Math.PI * 2;
    const ca = Math.cos(ang);
    // Natural cavern arch: full vaulted roof above (sin >= 0), flatter stone sill below (sin < 0).
    const saRaw = Math.sin(ang);
    const sa = saRaw >= 0 ? saRaw * 1.05 : saRaw * 0.48;
    const jit = 1 + (hash(caveIdx, ringIdx * 13 + 5, s * 7 + 3) - 0.5) * jitAmp;
    const dx = (rx * ca * radX + ux * sa * radY) * jit;
    const dy = (ry * ca * radX + uy * sa * radY) * jit;
    const dz = (rz * ca * radX + uz * sa * radY) * jit;
    positions.push(cxp + dx, cyp + dy, czp + dz);
    const grain = (hash(caveIdx, ringIdx, s) - 0.5) * 0.05;
    colors.push(
      Math.max(0, Math.min(1, cr + grain)),
      Math.max(0, Math.min(1, cg + grain)),
      Math.max(0, Math.min(1, cb + grain)),
    );
  }
}

/**
 * Build one merged BufferGeometry holding every cave mouth + rocky rim for this world, or null
 * if there are no caves. `heightUnits` is the world-space height the normalized elevation maps
 * to; `meshRes` is the terrain mesh resolution so caves seat on the SAME surface the player sees.
 */
export function buildCavesGeometry(
  world: World,
  renderSize: number,
  heightUnits: number,
  meshRes: number,
  /**
   * prototype, to be replaced — WV2-LOOK: `scale` shrinks every mouth (the defaults were sized
   * for the small v1 map; on the 41 km World v2 map they were 60-90 m black tubes) and `sink`
   * pushes it into the hillside by that fraction of its radius so only the opening shows.
   * Defaults (1, 0) reproduce the original geometry exactly.
   */
  opts: { scale?: number; sink?: number } = {},
): THREE.BufferGeometry | null {
  const k = opts.scale ?? 1;
  const sinkF = opts.sink ?? 0;
  const count = world.caveCount ?? 0;
  if (count === 0 || !world.caveX) return null;

  const positions: number[] = [];
  const colors: number[] = [];
  const indices: number[] = [];

  // Linear cliff-rock tones: weathered slate rim matching the rocky hillside framing a pitch-black throat.
  const rim = [0.23, 0.215, 0.2];
  const deep = [0.005, 0.005, 0.008];

  for (let c = 0; c < count; c++) {
    const cellX = world.caveX[c];
    const cellZ = world.caveZ[c];
    const yaw = world.caveYaw[c];
    const depth = (1.9 + hash(c, 1, 0) * 1.1) * k;
    const mouthR = (1.65 + hash(c, 2, 0) * 0.85) * k;
    const sag = (0.22 + hash(c, 3, 0) * 0.25) * k;

    // Outward (downhill) bearing — the mouth faces out of the hill.
    const ox = Math.cos(yaw);
    const oz = Math.sin(yaw);
    // Centred cell coords -> normalized UV [0, 1] matching terrain & flora.
    const u = Math.min(1, Math.max(0, (cellX + world.size * 0.5) / world.size));
    const v = Math.min(1, Math.max(0, (cellZ + world.size * 0.5) / world.size));
    const surfY = sampleMeshHeight(world, u, v, meshRes) * heightUnits - mouthR * sinkF * 0.35;
    const sx = (u - 0.5) * renderSize - ox * mouthR * sinkF;
    const sz = (v - 0.5) * renderSize - oz * mouthR * sinkF;

    // Seat pocket downhill/above surfY so uphill terrain never pokes through the dark throat.
    const tip = [sx + ox * 0.35 * k, surfY + mouthR * 0.45, sz + oz * 0.35 * k];
    const mouth = [
      sx + ox * (0.35 * k + depth * 0.82),
      surfY + mouthR * 0.56,
      sz + oz * (0.35 * k + depth * 0.82),
    ];
    let axx = mouth[0] - tip[0];
    let axy = mouth[1] - tip[1];
    let axz = mouth[2] - tip[2];
    const axl = Math.hypot(axx, axy, axz) || 1;
    axx /= axl;
    axy /= axl;
    axz /= axl;

    let upx = 0;
    let upy = 1;
    const upz = 0;
    if (Math.abs(axy) > 0.9) {
      upx = 1;
      upy = 0;
    }
    let rx = upy * axz - upz * axy;
    let ry = upz * axx - upx * axz;
    let rz = upx * axy - upy * axx;
    const rl = Math.hypot(rx, ry, rz) || 1;
    rx /= rl;
    ry /= rl;
    rz /= rl;
    const ux = axy * rz - axz * ry;
    const uy = axz * rx - axx * rz;
    const uz = axx * ry - axy * rx;

    const base = positions.length / 3;
    // Apex (closed pitch-black pocket).
    positions.push(tip[0], tip[1], tip[2]);
    colors.push(deep[0], deep[1], deep[2]);

    // Inner cavern funnel rings (steep darkening keeps throat crisp black against weathered rim).
    for (let k = 1; k <= RINGS; k++) {
      const t = k / RINGS;
      const r = mouthR * Math.pow(t, 0.58);
      const cxp = tip[0] + (mouth[0] - tip[0]) * t;
      const cyp = tip[1] + (mouth[1] - tip[1]) * t - Math.sin(t * Math.PI) * sag;
      const czp = tip[2] + (mouth[2] - tip[2]) * t;
      const shade = Math.pow(t, 2.8) * 0.42;
      addArchRing(
        positions,
        colors,
        cxp,
        cyp,
        czp,
        rx,
        ry,
        rz,
        ux,
        uy,
        uz,
        r * 1.15,
        r,
        deep[0] + (rim[0] - deep[0]) * shade,
        deep[1] + (rim[1] - deep[1]) * shade,
        deep[2] + (rim[2] - deep[2]) * shade,
        c,
        k,
        0.32,
      );
    }

    // Tight weathered stone collar framing the arch and anchoring cleanly into the cliff.
    for (let ck = 1; ck <= COLLAR_RINGS; ck++) {
      const flare = ck === 1 ? 1.14 : 1.26;
      const setback = ck === 1 ? 0.15 * k : -(depth + 0.9 * k);
      const tone = ck === 1 ? 0.92 : 0.72;
      addArchRing(
        positions,
        colors,
        mouth[0] + axx * setback,
        mouth[1] + axy * setback - sag * 0.1,
        mouth[2] + axz * setback,
        rx,
        ry,
        rz,
        ux,
        uy,
        uz,
        mouthR * 1.15 * flare,
        mouthR * flare,
        rim[0] * tone,
        rim[1] * tone,
        rim[2] * tone,
        c,
        RINGS + ck,
        0.42,
      );
    }

    const totalRings = RINGS + COLLAR_RINGS;
    const ring1 = base + 1;
    for (let s = 0; s < SEG; s++) {
      const a = ring1 + s;
      const b = ring1 + ((s + 1) % SEG);
      indices.push(base, b, a);
    }
    for (let k = 0; k < totalRings - 1; k++) {
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
