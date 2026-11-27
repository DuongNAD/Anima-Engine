import React, { useEffect, useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { sampleElevation } from '../utils/worldSample';
import type { World } from '../utils/worldGen';
import type { QualityTier } from '../utils/qualityTiers';
import { WORLD_EXTENT_M, type WorldV2LoadedBundle } from './types';
import { buildWorldV2GrassMask, GRASS_FIELD_GLSL, GRASS_PATCH_GLSL } from './grassTables';
import {
  ATMOSPHERE_FRAGMENT_PARS,
  atmosphereFragmentApply,
  bindAtmosphereUniforms,
} from '../materials/atmosphere';

// prototype, to be replaced — WV2-LOOK r12 realistic instanced ground cover:
//   1. `fine`    (near camera): multi-segment quadratic Bézier tapered blades with rounded
//                cylindrical normals, grouped in Voronoi world-space clumps;
//   2. `tussock` (overlapping mid-to-far range): fewer-segment curved tufts + dry seed-head
//                culms that taper seamlessly into the synchronized anisotropic terrain turf;
//   3. `herbs`   (high/medium): curved basal leaves with wildflowers in moist biomes and golden
//                dry oat/foxtail seed panicles in dry biomes.
// All layers share `grassTables.ts` (zero on sand, rock, snow, mud, water, steep slopes) plus
// scrolling metre-space wind gust waves, root AO, and warm sun-backlit translucency.
// Decoration only: no colliders; `low` tier keeps 2 draw calls and stays under the triangle budget.

export interface WorldV2GroundCoverProps {
  bundle: WorldV2LoadedBundle;
  world: World;
  renderSize?: number;
  heightRatio?: number;
  meshResolution?: number;
  tier?: QualityTier;
}

type CoverKind = 'fine' | 'tussock' | 'herbs';

interface BladeSpec {
  ox: number;
  oz: number;
  ca: number;
  sa: number;
  width: number;
  height: number;
  lean: number;
  segments: number;
  kind: number; // 0 = grass blade, 1 = flower petal, 2 = dry seed head culm/panicle
  phase: number;
  tint: readonly [number, number, number];
}

function appendBezierBlade(
  pos: number[],
  col: number[],
  nrm: number[],
  meta: number[],
  spec: BladeSpec,
): void {
  const { ox, oz, ca, sa, width, height, lean, segments, kind, phase, tint } = spec;
  // Across-blade vector (-sa, 0, ca); front face looks toward (ca, ..., sa)
  const rx = -sa;
  const rz = ca;
  const p0x = ox;
  const p0y = 0;
  const p0z = oz;
  const p1x = ox + ca * lean * 0.28;
  const p1y = height * 0.58;
  const p1z = oz + sa * lean * 0.28;
  const p2x = ox + ca * lean;
  const p2y = height * (1.0 - Math.min(0.28, lean * 0.22));
  const p2z = oz + sa * lean;

  const evalRing = (t: number) => {
    const it = 1 - t;
    const x = it * it * p0x + 2 * it * t * p1x + t * t * p2x;
    const y = it * it * p0y + 2 * it * t * p1y + t * t * p2y;
    const z = it * it * p0z + 2 * it * t * p1z + t * t * p2z;
    // Derivative dP/dt for tangent-accurate curved normal
    const tx = 2 * it * (p1x - p0x) + 2 * t * (p2x - p1x);
    const ty = 2 * it * (p1y - p0y) + 2 * t * (p2y - p1y);
    const tz = 2 * it * (p1z - p0z) + 2 * t * (p2z - p1z);
    // Cross(tangent, across): across = (rx, 0, rz) = (-sa, 0, ca)
    let nx = ty * rz;
    let ny = tz * rx - tx * rz;
    let nz = -ty * rx;
    const nl = Math.hypot(nx, ny, nz) || 1;
    nx /= nl;
    ny /= nl;
    nz /= nl;
    if (ny < 0) {
      nx = -nx;
      ny = -ny;
      nz = -nz;
    }
    const wProfile =
      kind >= 1.5
        ? t < 0.72
          ? 0.45 * (1 - t * 0.3)
          : Math.sin(((t - 0.72) / 0.28) * Math.PI) * 1.85 + 0.15
        : (1 - Math.pow(t, 1.35)) * (1 + 0.14 * Math.sin(t * Math.PI));
    const hw = width * wProfile;
    return { x, y, z, nx, ny, nz, hw };
  };

  const pushVert = (vx: number, vy: number, vz: number, u: number, t: number, rnx: number, rny: number, rnz: number) => {
    pos.push(vx, vy, vz);
    col.push(tint[0], tint[1], tint[2]);
    // Rounded normal across blade width (`u` in [-1, 1]): tilts normal around spine axis
    const round = u * 0.56;
    const rLen = Math.hypot(rnx + rx * round, rny + 0.18, rnz + rz * round) || 1;
    nrm.push((rnx + rx * round) / rLen, (rny + 0.18) / rLen, (rnz + rz * round) / rLen);
    meta.push(u, t, kind, phase);
  };

  for (let s = 0; s < segments; s += 1) {
    const t0 = s / segments;
    const t1 = (s + 1) / segments;
    const r0 = evalRing(t0);
    const r1 = evalRing(t1);
    const l0x = r0.x - rx * r0.hw;
    const l0z = r0.z - rz * r0.hw;
    const r0x = r0.x + rx * r0.hw;
    const r0z = r0.z + rz * r0.hw;
    if (s === segments - 1) {
      pushVert(l0x, r0.y, l0z, -1, t0, r0.nx, r0.ny, r0.nz);
      pushVert(r0x, r0.y, r0z, 1, t0, r0.nx, r0.ny, r0.nz);
      pushVert(r1.x, r1.y, r1.z, 0, t1, r1.nx, r1.ny, r1.nz);
    } else {
      const l1x = r1.x - rx * r1.hw;
      const l1z = r1.z - rz * r1.hw;
      const r1x = r1.x + rx * r1.hw;
      const r1z = r1.z + rz * r1.hw;
      pushVert(l0x, r0.y, l0z, -1, t0, r0.nx, r0.ny, r0.nz);
      pushVert(r0x, r0.y, r0z, 1, t0, r0.nx, r0.ny, r0.nz);
      pushVert(r1x, r1.y, r1z, 1, t1, r1.nx, r1.ny, r1.nz);
      pushVert(l0x, r0.y, l0z, -1, t0, r0.nx, r0.ny, r0.nz);
      pushVert(r1x, r1.y, r1z, 1, t1, r1.nx, r1.ny, r1.nz);
      pushVert(l1x, r1.y, l1z, -1, t1, r1.nx, r1.ny, r1.nz);
    }
  }
}

function finalizeBladeGeometry(pos: number[], col: number[], nrm: number[], meta: number[]): THREE.BufferGeometry {
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  g.setAttribute('color', new THREE.Float32BufferAttribute(col, 3));
  g.setAttribute('normal', new THREE.Float32BufferAttribute(nrm, 3));
  g.setAttribute('aBladeMeta', new THREE.Float32BufferAttribute(meta, 4));
  return g;
}

function makeFineGrassGeometry(bladeCount: number, segments: number): THREE.BufferGeometry {
  const pos: number[] = [];
  const col: number[] = [];
  const nrm: number[] = [];
  const meta: number[] = [];
  const rng = seeded(0x91f3);
  for (let k = 0; k < bladeCount; k += 1) {
    // Organic micro-tuft footprint: irregular offsets so adjacent blades overlap without radial spikes.
    const radialAngle = rng() * Math.PI * 2;
    const rad = Math.sqrt(0.08 + 0.92 * rng()) * 0.25;
    const faceAngle = rng() * Math.PI * 2;
    const v = 0.92 + rng() * 0.22;
    const warm = rng();
    appendBezierBlade(pos, col, nrm, meta, {
      ox: Math.cos(radialAngle) * rad,
      oz: Math.sin(radialAngle) * rad,
      ca: Math.cos(faceAngle),
      sa: Math.sin(faceAngle),
      width: 0.011 + rng() * 0.009,
      height: 0.56 + rng() * 0.62,
      lean: 0.16 + rng() * 0.28,
      segments,
      kind: 0,
      phase: rng(),
      tint: [v * (0.94 + warm * 0.14), v * (1.03 - warm * 0.04), v * (0.96 - warm * 0.14)],
    });
  }
  return finalizeBladeGeometry(pos, col, nrm, meta);
}

function makeTussockGeometry(bladeCount: number, segments: number, withSeedCulm: boolean): THREE.BufferGeometry {
  const pos: number[] = [];
  const col: number[] = [];
  const nrm: number[] = [];
  const meta: number[] = [];
  const rng = seeded(0x4c19);
  for (let k = 0; k < bladeCount; k += 1) {
    const a = (k / bladeCount) * Math.PI * 2 + (rng() - 0.5) * 0.7;
    const off = 0.05 + rng() * 0.22;
    const v = 0.92 + rng() * 0.22;
    appendBezierBlade(pos, col, nrm, meta, {
      ox: Math.cos(a) * off,
      oz: Math.sin(a) * off,
      ca: Math.cos(a),
      sa: Math.sin(a),
      width: 0.022 + rng() * 0.015,
      height: 0.58 + rng() * 0.42,
      lean: 0.2 + rng() * 0.22,
      segments,
      kind: 0,
      phase: rng(),
      tint: [v * 0.98, v * 1.02, v * 0.94],
    });
  }
  if (withSeedCulm) {
    const saAngle = rng() * Math.PI * 2;
    appendBezierBlade(pos, col, nrm, meta, {
      ox: (rng() - 0.5) * 0.14,
      oz: (rng() - 0.5) * 0.14,
      ca: Math.cos(saAngle),
      sa: Math.sin(saAngle),
      width: 0.042,
      height: 1.24,
      lean: 0.2,
      segments: Math.max(2, segments),
      kind: 2,
      phase: rng(),
      tint: [1.05, 1.0, 0.86],
    });
  }
  return finalizeBladeGeometry(pos, col, nrm, meta);
}

function makeHerbGeometry(): THREE.BufferGeometry {
  // Curved basal leaves (`kind = 0`) + multi-petal wildflower head (`kind = 1`) + dry tall culm (`kind = 2`).
  const pos: number[] = [];
  const col: number[] = [];
  const nrm: number[] = [];
  const meta: number[] = [];
  const rng = seeded(0x7e21);
  for (let k = 0; k < 4; k += 1) {
    const a = (k / 4) * Math.PI * 2 + 0.35 + (rng() - 0.5) * 0.3;
    appendBezierBlade(pos, col, nrm, meta, {
      ox: Math.cos(a) * 0.04,
      oz: Math.sin(a) * 0.04,
      ca: Math.cos(a),
      sa: Math.sin(a),
      width: 0.055,
      height: 0.44 + rng() * 0.16,
      lean: 0.24,
      segments: 2,
      kind: 0,
      phase: rng(),
      tint: [0.88, 0.95, 0.85],
    });
  }
  // Wildflower stem (`kind = 0`)
  appendBezierBlade(pos, col, nrm, meta, {
    ox: 0,
    oz: 0,
    ca: 0.7,
    sa: 0.71,
    width: 0.012,
    height: 0.68,
    lean: 0.08,
    segments: 2,
    kind: 0,
    phase: 0.2,
    tint: [0.88, 0.96, 0.84],
  });
  // Delicate 5-petal wildflower head (`kind = 1`), scaled subtly so blooms dot the sward naturally
  for (let k = 0; k < 5; k += 1) {
    const a = (k / 5) * Math.PI * 2 + 0.2;
    const ca = Math.cos(a);
    const sa = Math.sin(a);
    const ox = ca * 0.015 + 0.04;
    const oz = sa * 0.015 + 0.04;
    const p = (vx: number, vy: number, vz: number, u: number, t: number) => {
      pos.push(vx, vy, vz);
      col.push(1, 1, 1);
      nrm.push(ca * 0.22, 0.96, sa * 0.22);
      meta.push(u, t, 1, k * 0.2);
    };
    p(ox - sa * 0.036, 0.67, oz + ca * 0.036, -1, 0.82);
    p(ox + sa * 0.036, 0.67, oz - ca * 0.036, 1, 0.82);
    p(ox + ca * 0.11, 0.76, oz + sa * 0.11, 0, 1.0);
  }
  // Tall dry seed-head culm (`kind = 2`): folded in moist meadows, rises in dry steppe/savanna
  const seedAngle = 1.9;
  appendBezierBlade(pos, col, nrm, meta, {
    ox: -0.05,
    oz: 0.04,
    ca: Math.cos(seedAngle),
    sa: Math.sin(seedAngle),
    width: 0.044,
    height: 1.28,
    lean: 0.22,
    segments: 3,
    kind: 2,
    phase: 0.65,
    tint: [1.05, 1.0, 0.88],
  });
  return finalizeBladeGeometry(pos, col, nrm, meta);
}

/** Named operations so the frame loop does not mutate a render-owned object inline. */
function setVisible(obj: THREE.Object3D, visible: boolean): void {
  obj.visible = visible;
}

function setUniformValue(u: { value: number }, value: number): void {
  u.value = value;
}

function seeded(seed: number): () => number {
  let s = seed >>> 0 || 1;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

interface GroundCoverTextures {
  heightTex: THREE.DataTexture;
  maskTex: THREE.DataTexture;
  size: number;
}

interface GroundCoverLayerProps {
  kind: CoverKind;
  count: number;
  radiusM: number;
  innerFadeM?: number;
  bladeCount: number;
  segments: number;
  withSeedCulm: boolean;
  seed: number;
  textures: GroundCoverTextures;
  world: World;
  renderSize: number;
  heightUnits: number;
  meshResolution: number;
  unitsPerM: number;
}

const HERB_PALETTE: ReadonlyArray<readonly [number, number, number]> = [
  [0.28, 0.2, 0.025], // buttercup gold (linear PBR albedo for V2_LOOK_EXPOSURE = 2.5)
  [0.24, 0.23, 0.19], // daisy warm cream
  [0.14, 0.09, 0.24], // wild lavender
  [0.25, 0.08, 0.07], // meadow coral
  [0.07, 0.13, 0.26], // alpine bluebell
];

const GroundCoverLayer: React.FC<GroundCoverLayerProps> = ({
  kind,
  count,
  radiusM,
  innerFadeM = 0,
  bladeCount,
  segments,
  withSeedCulm,
  seed,
  textures,
  world,
  renderSize,
  heightUnits,
  meshResolution,
  unitsPerM,
}) => {
  const radiusUnits = radiusM * unitsPerM;
  const innerUnits = innerFadeM * unitsPerM;

  const instances = useMemo(() => {
    const rnd = seeded(seed);
    const tile = radiusUnits * 2;
    const matrices = new Float32Array(count * 16);
    const colors = new Float32Array(count * 3);
    const m = new THREE.Matrix4();
    const q = new THREE.Quaternion();
    const e = new THREE.Euler();
    const sc = new THREE.Vector3();
    const p = new THREE.Vector3();
    for (let k = 0; k < count; k += 1) {
      let hM: number;
      let wM: number;
      if (kind === 'fine') {
        hM = 0.34 + rnd() * 0.44;
        wM = 0.62 + rnd() * 0.42;
      } else if (kind === 'tussock') {
        const tall = rnd() < 0.24;
        hM = tall ? 0.6 + rnd() * 0.54 : 0.38 + rnd() * 0.42;
        wM = tall ? 1.1 + rnd() * 0.62 : 0.78 + rnd() * 0.54;
      } else {
        hM = 0.38 + rnd() * 0.32;
        wM = 0.52 + rnd() * 0.32;
      }
      e.set((rnd() - 0.5) * 0.16, rnd() * Math.PI * 2, (rnd() - 0.5) * 0.16);
      q.setFromEuler(e);
      sc.set(wM * unitsPerM, hM * unitsPerM, wM * unitsPerM * (0.85 + rnd() * 0.3));
      p.set(rnd() * tile, rnd(), rnd() * tile); // y carries the density-test seed
      m.compose(p, q, sc);
      matrices.set(m.elements, k * 16);

      if (kind === 'herbs') {
        const pal = HERB_PALETTE[Math.floor(rnd() * HERB_PALETTE.length)];
        const v = 0.88 + rnd() * 0.24;
        colors[k * 3] = pal[0] * v;
        colors[k * 3 + 1] = pal[1] * v;
        colors[k * 3 + 2] = pal[2] * v;
      } else {
        const v = 0.92 + rnd() * 0.2;
        const dryShift = rnd();
        colors[k * 3] = v * (0.95 + dryShift * 0.14);
        colors[k * 3 + 1] = v * (1.03 - dryShift * 0.04);
        colors[k * 3 + 2] = v * (0.96 - dryShift * 0.16);
      }
    }
    const geometry =
      kind === 'fine'
        ? makeFineGrassGeometry(bladeCount, segments)
        : kind === 'tussock'
          ? makeTussockGeometry(bladeCount, segments, withSeedCulm)
          : makeHerbGeometry();
    return {
      geometry,
      matrices,
      colors,
      tile,
    };
  }, [kind, bladeCount, segments, withSeedCulm, seed, count, radiusUnits, unitsPerM]);

  const timeUniform = useMemo(() => ({ value: 0 }), []);

  const material = useMemo(() => {
    const mat = new THREE.MeshStandardMaterial({
      vertexColors: true,
      roughness: 0.9,
      metalness: 0,
      side: THREE.DoubleSide,
    });
    const mPerUnit = 1 / unitsPerM;
    const uniforms = {
      uGcHeight: { value: textures.heightTex },
      uGcMask: { value: textures.maskTex },
      uGcSize: { value: textures.size },
      uGcMeshRes: { value: meshResolution },
      uGcRender: { value: renderSize },
      uGcMPerUnit: { value: mPerUnit },
      uGcTile: { value: instances.tile },
      uGcRadius: { value: radiusUnits },
      uGcInner: { value: innerUnits },
      uGcIsHerb: { value: kind === 'herbs' ? 1 : 0 },
      uGcIsTussock: { value: kind === 'tussock' ? 1 : 0 },
      uGcTime: timeUniform,
      uGcSway: { value: kind === 'fine' ? 0.34 : kind === 'herbs' ? 0.28 : 0.24 },
    };
    mat.onBeforeCompile = (shader) => {
      bindAtmosphereUniforms(shader.uniforms);
      Object.assign(shader.uniforms, uniforms);
      shader.vertexShader =
        `
attribute vec4 aBladeMeta;
uniform sampler2D uGcHeight;
uniform sampler2D uGcMask;
uniform float uGcSize;
uniform float uGcMeshRes;
uniform float uGcRender;
uniform float uGcMPerUnit;
uniform float uGcTile;
uniform float uGcRadius;
uniform float uGcInner;
uniform float uGcIsHerb;
uniform float uGcIsTussock;
uniform float uGcTime;
uniform float uGcSway;
uniform float uAtmoSeaY;
varying vec3 vLkGcWp;
varying float vGcDry;
varying float vGcTip;
varying float vGcFlower;
varying float vGcSeedHead;
varying float vGcClumpTint;
varying float vGcDistM;
varying float vGcGust;
varying vec3 vGcUpView;
float gcH(vec2 uvc){
return texture2D(uGcHeight,(uvc*(uGcSize-1.0)+0.5)/uGcSize).r;
}
${GRASS_PATCH_GLSL}
${GRASS_FIELD_GLSL}
` +
        shader.vertexShader.replace(
          '#include <project_vertex>',
          `
vec2 gcP0=instanceMatrix[3].xz;
float gcSeed=instanceMatrix[3].y;
vec2 gcCam=cameraPosition.xz;
vec2 gcPw=gcP0+uGcTile*floor((gcCam-gcP0)/uGcTile+0.5);
vec2 gcUv=gcPw/uGcRender+0.5;
vec2 gcG=clamp(gcUv,0.0,1.0)*uGcMeshRes;
vec2 gcI=floor(gcG);
vec2 gcF=gcG-gcI;
float hA=gcH(gcI/uGcMeshRes);
float hB=gcH((gcI+vec2(1.0,0.0))/uGcMeshRes);
float hC=gcH((gcI+vec2(0.0,1.0))/uGcMeshRes);
float hD=gcH((gcI+vec2(1.0,1.0))/uGcMeshRes);
float gcY=(gcF.x+gcF.y<1.0)
?hA+(hB-hA)*gcF.x+(hC-hA)*gcF.y
:hD+(hC-hD)*(1.0-gcF.x)+(hB-hD)*(1.0-gcF.y);
vec4 gcM=texture2D(uGcMask,clamp(gcUv,0.0,1.0));
vec2 gcWorldM=gcPw*uGcMPerUnit;
vec4 gcClump=lkGrassClump(gcWorldM);
float gcPatch=lkGrassPatch(gcPw);
vGcDry=clamp(gcM.b+(gcClump.x-0.5)*0.28+gcPatch*0.20,0.0,1.0);
vGcClumpTint=gcClump.y;
float isSeed=step(1.5,aBladeMeta.z);
float isFlower=step(0.5,aBladeMeta.z)*(1.0-isSeed);
float gcDist=length(vec3(gcPw.x-gcCam.x,gcY-cameraPosition.y,gcPw.y-gcCam.y));
float gcDistM=gcDist*uGcMPerUnit;
vGcDistM=gcDistM;
float seedShow=smoothstep(0.24,0.60,vGcDry);
float dryHerbNeed=gcM.r*seedShow;
float gcDens=uGcIsHerb>0.5
?max(gcM.a*(1.0-0.45*vGcDry),dryHerbNeed*0.52)
:gcM.r*(1.0-0.18*max(gcPatch,0.0))*(1.0-smoothstep(0.68,1.15,gcClump.w)*0.22);
float outerStart=uGcIsTussock>0.5?uGcRadius*0.18:uGcRadius*0.32;
float gcOuterFade=pow(1.0-smoothstep(outerStart,uGcRadius,gcDist),1.35);
float gcInnerScale=uGcInner>0.001?mix(0.58,1.0,smoothstep(uGcInner*0.45,uGcInner*1.35,gcDist)):1.0;
float gcRingThresh=uGcInner>0.001?gcOuterFade:1.0;
float gcKeep=step(gcSeed,gcDens*mix(1.0,0.52+0.48*gcRingThresh,step(0.001,gcDens)))
*gcOuterFade*gcInnerScale
*step(uAtmoSeaY+0.02,gcY)
*step(0.0,gcUv.x)*step(gcUv.x,1.0)*step(0.0,gcUv.y)*step(0.0,1.0-gcUv.y);
vec3 gcT=transformed;
float bladeT=clamp(aBladeMeta.y,0.0,1.0);
vGcTip=bladeT;
vGcFlower=isFlower;
vGcSeedHead=isSeed;
float hBio=max(0.25,gcM.g*2.0);
float clumpH=mix(0.68,1.34,gcClump.z);
float farScale=uGcIsTussock>0.5?mix(1.0,0.72,smoothstep(45.0,145.0,gcDistM)):1.0;
float hMul=mix(hBio,0.55+hBio*0.45,uGcIsHerb)*clumpH*(0.84+0.32*fract(gcSeed*37.0))*farScale;
if(isSeed>0.5){
hMul*=mix(0.18,1.05,seedShow)*step(0.05,seedShow);
}
gcT.y*=hMul;
mat3 instRot=mat3(instanceMatrix);
vec3 gcWorldLocal=instRot*gcT;
float bendPow=pow(bladeT,1.55);
vec2 clumpLean=(gcClump.xy-0.5)*0.30*bendPow;
vec3 windField=lkGrassWind(gcWorldM,uGcTime);
vGcGust=windField.z;
float flutter=sin(uGcTime*5.6+aBladeMeta.w*6.283+dot(gcWorldM,vec2(0.8805,0.4741))*1.8)*0.045;
float stemH=max(abs(instRot[1].y)*hMul,0.001);
vec2 gustDisp=(windField.xy*(uGcSway*(0.65+0.70*windField.z))+vec2(flutter,flutter*0.6))*stemH*bendPow;
vec2 totalXZ=vec2(gcWorldLocal.x,gcWorldLocal.z)+clumpLean*stemH+gustDisp;
float sagRatio=clamp(dot(gustDisp,gustDisp)/(stemH*stemH),0.0,0.72);
gcWorldLocal.x=totalXZ.x;
gcWorldLocal.z=totalXZ.y;
gcWorldLocal.y*=sqrt(max(0.32,1.0-0.65*sagRatio));
gcWorldLocal*=gcKeep;
vec4 gcWorld=vec4(gcPw.x+gcWorldLocal.x,gcY+gcWorldLocal.y-0.003,gcPw.y+gcWorldLocal.z,1.0);
vLkGcWp=gcWorld.xyz;
vec3 bendNrm=normalize(instRot*normal+vec3(gustDisp.x,0.0,gustDisp.y)*1.5+vec3(0.0,0.25,0.0));
vNormal=normalize(normalMatrix*bendNrm);
vGcUpView=normalize((viewMatrix*vec4(0.0,1.0,0.0,0.0)).xyz);
vec4 mvPosition=viewMatrix*gcWorld;
gl_Position=projectionMatrix*mvPosition;
`,
        );
      shader.fragmentShader =
        'varying vec3 vLkGcWp;\nvarying float vGcDry;\nvarying float vGcTip;\nvarying float vGcFlower;\nvarying float vGcSeedHead;\nvarying float vGcClumpTint;\nvarying float vGcDistM;\nvarying float vGcGust;\nvarying vec3 vGcUpView;\n' +
        ATMOSPHERE_FRAGMENT_PARS +
        GRASS_FIELD_GLSL +
        shader.fragmentShader
          .replace(
            '#include <normal_fragment_maps>',
            `
#include <normal_fragment_maps>
normal=normalize(normal+vGcUpView*(gl_FrontFacing?0.34:0.46));
`,
          )
          .replace(
            '#include <color_fragment>',
            `
#include <color_fragment>
vec3 grassCol=lkGrassAlbedo(vGcDry,vGcTip,vGcClumpTint)*diffuseColor.rgb;
float farSoftAo=mix(0.42,0.68,smoothstep(12.0,32.0,vGcDistM));
float rootAo=mix(farSoftAo,1.16,smoothstep(0.0,0.52,vGcTip));
float seedTip=vGcSeedHead*smoothstep(0.62,0.88,vGcTip);
vec3 seedStraw=vec3(0.30,0.24,0.09)*mix(0.85,1.20,vGcClumpTint);
grassCol=mix(grassCol,seedStraw,seedTip*0.88);
float cloudVis=mix(0.62,1.0,atmoCloudShadow(vLkGcWp));
float gustTint=1.0+vGcGust*0.24*smoothstep(0.2,0.9,vGcTip);
vec3 finalGrass=grassCol*rootAo*gustTint;
vec3 petalCol=diffuseColor.rgb*mix(0.62,1.05,smoothstep(0.76,0.98,vGcTip));
diffuseColor.rgb=mix(finalGrass,petalCol,vGcFlower)*cloudVis;
`,
          )
          .replace(
            '#include <lights_fragment_end>',
            `
#include <lights_fragment_end>
{
  vec3 gcSunD=normalize(uAtmoSunDir);
  vec3 gcViewR=normalize(vLkGcWp-cameraPosition);
  float gcSunVis=smoothstep(0.02,0.22,gcSunD.y)*mix(0.35,1.0,atmoCloudShadow(vLkGcWp));
  float gcFwdWrap=pow(clamp(dot(gcViewR,normalize(vec3(gcSunD.x,gcSunD.y*0.52,gcSunD.z))),0.0,1.0),2.0);
  float gcFwdDirect=pow(clamp(dot(gcViewR,gcSunD),0.0,1.0),4.5);
  float gcFarFade=1.0-smoothstep(45.0,145.0,vGcDistM);
  float gcBack=(gcFwdWrap*0.72+gcFwdDirect*0.75)*smoothstep(0.12,0.92,vGcTip)*gcSunVis*(1.0-vGcFlower*0.45)*mix(0.35,1.0,gcFarFade);
  reflectedLight.directDiffuse+=diffuseColor.rgb*vec3(1.22,1.26,0.36)*gcBack*1.15;
  float gcSheen=pow(1.0-clamp(abs(dot(normal,geometryViewDir)),0.0,1.0),2.4)*smoothstep(0.28,0.96,vGcTip)*gcSunVis*mix(0.25,1.0,gcFarFade);
  reflectedLight.directSpecular+=diffuseColor.rgb*vec3(1.05,1.18,0.42)*gcSheen*0.42;
}
`,
          )
          .replace('#include <fog_fragment>', atmosphereFragmentApply('vLkGcWp'));
    };
    mat.customProgramCacheKey = () => `wv2-look-groundcover-v6-${kind}`;
    return mat;
  }, [textures, instances, meshResolution, renderSize, radiusUnits, innerUnits, unitsPerM, timeUniform, kind]);

  useEffect(() => () => instances.geometry.dispose(), [instances]);
  useEffect(() => () => material.dispose(), [material]);

  const meshRef = useRef<THREE.InstancedMesh | null>(null);
  useFrame((state) => {
    setUniformValue(timeUniform, state.clock.elapsedTime);
    const mesh = meshRef.current;
    if (!mesh) return;
    const cam = state.camera.position;
    const u = cam.x / renderSize + 0.5;
    const v = cam.z / renderSize + 0.5;
    const ground = u >= 0 && u <= 1 && v >= 0 && v <= 1 ? sampleElevation(world, u, v) * heightUnits : 0;
    setVisible(mesh, cam.y - ground < radiusUnits);
  });

  return (
    <instancedMesh
      name={`world-v2-flora-groundcover-${kind}`}
      args={[instances.geometry, material, count]}
      frustumCulled={false}
      castShadow={false}
      receiveShadow={false}
      userData={{ decorative: true, rwLayer: 'flora' }}
      ref={(mesh: THREE.InstancedMesh | null) => {
        meshRef.current = mesh;
        if (!mesh || !mesh.instanceMatrix) return;
        mesh.instanceMatrix.array.set(instances.matrices);
        mesh.instanceMatrix.needsUpdate = true;
        if (!mesh.instanceColor || mesh.instanceColor.count !== count) {
          mesh.instanceColor = new THREE.InstancedBufferAttribute(instances.colors, 3);
        }
      }}
    />
  );
};

export const WorldV2GroundCover: React.FC<WorldV2GroundCoverProps> = ({
  bundle,
  world,
  renderSize = 1200,
  heightRatio = 0.14,
  meshResolution = 384,
  tier = 'high',
}) => {
  const fineCount = tier === 'high' ? 62000 : tier === 'medium' ? 32000 : 14000;
  const fineRadiusM = tier === 'high' ? 32 : tier === 'medium' ? 24 : 16;
  const fineBlades = tier === 'high' ? 7 : 5;
  const fineSegs = tier === 'low' ? 2 : 3;
  const tussockCount = tier === 'high' ? 42000 : tier === 'medium' ? 18000 : 7000;
  const tussockRadiusM = tier === 'high' ? 145 : tier === 'medium' ? 110 : 60;
  const tussockBlades = tier === 'low' ? 4 : 5;
  const tussockSegs = tier === 'low' ? 1 : 2;
  const tussockSeedCulm = tier === 'high';
  const herbCount = tier === 'high' ? 6500 : tier === 'medium' ? 3500 : 0;
  const herbRadiusM = tier === 'high' ? 32 : 24;
  const heightUnits = renderSize * heightRatio;
  const unitsPerM = renderSize / (bundle.extentM || WORLD_EXTENT_M);

  const textures = useMemo<GroundCoverTextures>(() => {
    const { size, elevation } = world;
    const h = new Float32Array(size * size);
    for (let i = 0; i < h.length; i += 1) h[i] = elevation[i] * heightUnits;
    const heightTex = new THREE.DataTexture(h, size, size, THREE.RedFormat, THREE.FloatType);
    heightTex.minFilter = THREE.LinearFilter;
    heightTex.magFilter = THREE.LinearFilter;
    heightTex.needsUpdate = true;
    const maskTex = buildWorldV2GrassMask(bundle, world);
    return { heightTex, maskTex, size };
  }, [bundle, world, heightUnits]);

  useEffect(
    () => () => {
      textures.heightTex.dispose();
      textures.maskTex.dispose();
    },
    [textures],
  );

  const common = { textures, world, renderSize, heightUnits, meshResolution, unitsPerM };
  return (
    <>
      <GroundCoverLayer
        kind="fine"
        count={fineCount}
        radiusM={fineRadiusM}
        bladeCount={fineBlades}
        segments={fineSegs}
        withSeedCulm={false}
        seed={bundle.seed ^ 0x6b1d}
        {...common}
      />
      <GroundCoverLayer
        kind="tussock"
        count={tussockCount}
        radiusM={tussockRadiusM}
        innerFadeM={14}
        bladeCount={tussockBlades}
        segments={tussockSegs}
        withSeedCulm={tussockSeedCulm}
        seed={bundle.seed ^ 0x6c0d}
        {...common}
      />
      {herbCount > 0 ? (
        <GroundCoverLayer
          kind="herbs"
          count={herbCount}
          radiusM={herbRadiusM}
          bladeCount={4}
          segments={2}
          withSeedCulm={true}
          seed={bundle.seed ^ 0x6d7f}
          {...common}
        />
      ) : null}
    </>
  );
};

export default WorldV2GroundCover;
