import React, { useEffect, useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { sampleElevation } from '../utils/worldSample';
import type { World } from '../utils/worldGen';
import type { QualityTier } from '../utils/qualityTiers';
import { WORLD_EXTENT_M, type WorldV2LoadedBundle } from './types';
import {
  ATMOSPHERE_FRAGMENT_PARS,
  atmosphereFragmentApply,
  bindAtmosphereUniforms,
} from '../materials/atmosphere';

// prototype, to be replaced — WV2-LOOK ground cover: grass everywhere it grows, in TWO instanced
// draw calls: a dense blade layer within ~45 m of the camera (a continuous sward) and the r5
// tussock/shrub layer out to ~320 m. r9: per-biome grass height and dryness (tall golden savanna,
// green meadow, short alpine tussock, sedge in swamps, sparse dune grass on beaches), dry and
// lush patches inside a biome, and a gentle wind sway.
//
// Every tuft owns a fixed position inside a square tile twice the cover radius wide; the vertex
// shader moves it to the copy of that tile nearest the camera (so tufts never swim as the camera
// moves), reads the terrain height exactly as the terrain mesh interpolates it (same grid, same
// triangle split), and collapses tufts that are out of range, under water or on ground the biome
// mask says is bare (rock, snow, sand, water). Decoration only: no collider, no data written.
// Blades: high 40k, medium 20k (45 m), low 8k (30 m). Tufts: high 14k, medium 7k, low off.

export interface WorldV2GroundCoverProps {
  bundle: WorldV2LoadedBundle;
  world: World;
  renderSize?: number;
  heightRatio?: number;
  meshResolution?: number;
  tier?: QualityTier;
}

/** Tuft density per canonical biome id (grass / low shrub cover). */
const BIOME_COVER: ReadonlyArray<number> = [
  0, 0.15, 0.05, 0.95, 1.0, 0.75, 0.45, 0.35, 0.4, 0.6, 0.8, 0.08, 0, 0, 0, 0.1, 0.65, 0.9, 0.65, 0.1, 0, 0.6,
];
/** Grass height multiplier per canonical biome id (1 = meadow grass). */
const BIOME_GRASS_HEIGHT: ReadonlyArray<number> = [
  0, 0.8, 0.5, 1.7, 1.0, 0.8, 0.6, 0.7, 0.5, 0.35, 1.5, 0.4, 0, 0, 0, 0.6, 0.8, 0.9, 0.4, 0.5, 0, 0.8,
];
/** Grass dryness per canonical biome id (0 lush green, 1 straw). */
const BIOME_GRASS_DRY: ReadonlyArray<number> = [
  0, 0.55, 0.95, 0.75, 0.25, 0.55, 0.15, 0.05, 0.3, 0.5, 0.1, 0.5, 0, 0, 0, 0.2, 0.65, 0.7, 0.45, 0.85, 0, 0.2,
];

function makeTuftGeometry(): THREE.BufferGeometry {
  // Four crossed blades leaning outward around a tiny mound: 4 + 4 triangles.
  const pos: number[] = [];
  const col: number[] = [];
  const nrm: number[] = [];
  const base = new THREE.Color('#222d16');
  const tip = new THREE.Color('#566433');
  for (let k = 0; k < 4; k += 1) {
    const a = (k / 4) * Math.PI * 2 + 0.3;
    const ca = Math.cos(a);
    const sa = Math.sin(a);
    const w = 0.22;
    const lean = 0.35;
    // Blade: two base corners, one tip leaning out.
    pos.push(-sa * w, 0, ca * w, sa * w, 0, -ca * w, ca * lean, 1, sa * lean);
    col.push(base.r, base.g, base.b, base.r, base.g, base.b, tip.r, tip.g, tip.b);
    for (let v = 0; v < 3; v += 1) nrm.push(ca * 0.4, 0.9, sa * 0.4);
    // Mound facet.
    const b = a + Math.PI / 4;
    pos.push(Math.cos(a) * 0.45, 0, Math.sin(a) * 0.45, Math.cos(b + Math.PI / 4) * 0.45, 0, Math.sin(b + Math.PI / 4) * 0.45, 0, 0.28, 0);
    col.push(base.r, base.g, base.b, base.r, base.g, base.b, 0.2, 0.26, 0.11);
    for (let v = 0; v < 3; v += 1) nrm.push(Math.cos(b) * 0.5, 0.85, Math.sin(b) * 0.5);
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  g.setAttribute('color', new THREE.Float32BufferAttribute(col, 3));
  g.setAttribute('normal', new THREE.Float32BufferAttribute(nrm, 3));
  return g;
}

function makeBladeClumpGeometry(): THREE.BufferGeometry {
  // Seven thin blades fanning out of one point: 7 triangles, unit height.
  const pos: number[] = [];
  const col: number[] = [];
  const nrm: number[] = [];
  const base = new THREE.Color('#1f2a14');
  const tip = new THREE.Color('#6f8240');
  for (let k = 0; k < 7; k += 1) {
    const a = (k / 7) * Math.PI * 2 + (k % 2) * 0.4;
    const ca = Math.cos(a);
    const sa = Math.sin(a);
    const w = 0.035;
    const lean = 0.18 + (k % 3) * 0.12;
    const hgt = 0.75 + (k % 4) * 0.09;
    pos.push(-sa * w, 0, ca * w, sa * w, 0, -ca * w, ca * lean, hgt, sa * lean);
    col.push(base.r, base.g, base.b, base.r, base.g, base.b, tip.r, tip.g, tip.b);
    for (let v = 0; v < 3; v += 1) nrm.push(ca * 0.3, 0.95, sa * 0.3);
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  g.setAttribute('color', new THREE.Float32BufferAttribute(col, 3));
  g.setAttribute('normal', new THREE.Float32BufferAttribute(nrm, 3));
  return g;
}

/** A named operation so the frame loop does not mutate a render-owned object inline. */
function setVisible(obj: THREE.Object3D, visible: boolean): void {
  obj.visible = visible;
}

/** Same, for the wind clock uniform. */
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
  kind: 'blades' | 'tufts';
  count: number;
  radiusM: number;
  seed: number;
  textures: GroundCoverTextures;
  world: World;
  renderSize: number;
  heightUnits: number;
  meshResolution: number;
  unitsPerM: number;
}

const GroundCoverLayer: React.FC<GroundCoverLayerProps> = ({
  kind,
  count,
  radiusM,
  seed,
  textures,
  world,
  renderSize,
  heightUnits,
  meshResolution,
  unitsPerM,
}) => {
  const radiusUnits = radiusM * unitsPerM;

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
      if (kind === 'blades') {
        hM = 0.3 + rnd() * 0.45;
        wM = 0.35 + rnd() * 0.35;
      } else {
        const shrub = rnd() < 0.22;
        hM = shrub ? 1.4 + rnd() * 1.6 : 0.6 + rnd() * 0.9;
        wM = shrub ? 1.6 + rnd() * 1.4 : 0.8 + rnd() * 0.7;
      }
      e.set((rnd() - 0.5) * 0.15, rnd() * Math.PI * 2, (rnd() - 0.5) * 0.15);
      q.setFromEuler(e);
      sc.set(wM * unitsPerM, hM * unitsPerM, wM * unitsPerM * (0.8 + rnd() * 0.4));
      p.set(rnd() * tile, rnd(), rnd() * tile); // y carries the density-test seed
      m.compose(p, q, sc);
      matrices.set(m.elements, k * 16);
      const v = 0.8 + rnd() * 0.4;
      const dry = rnd();
      colors[k * 3] = v * (0.92 + dry * 0.22);
      colors[k * 3 + 1] = v * (0.96 + dry * 0.06);
      colors[k * 3 + 2] = v * (1.0 - dry * 0.25);
    }
    return {
      geometry: kind === 'blades' ? makeBladeClumpGeometry() : makeTuftGeometry(),
      matrices,
      colors,
      tile,
    };
  }, [kind, seed, count, radiusUnits, unitsPerM]);

  const timeUniform = useMemo(() => ({ value: 0 }), []);

  const material = useMemo(() => {
    const mat = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.95, metalness: 0, side: THREE.DoubleSide });
    const uniforms = {
      uGcHeight: { value: textures.heightTex },
      uGcMask: { value: textures.maskTex },
      uGcSize: { value: textures.size },
      uGcMeshRes: { value: meshResolution },
      uGcRender: { value: renderSize },
      uGcTile: { value: instances.tile },
      uGcRadius: { value: radiusUnits },
      uGcTime: timeUniform,
      uGcSway: { value: kind === 'blades' ? 0.22 : 0.08 },
    };
    mat.onBeforeCompile = (shader) => {
      bindAtmosphereUniforms(shader.uniforms);
      Object.assign(shader.uniforms, uniforms);
      shader.vertexShader =
        `
uniform sampler2D uGcHeight;
uniform sampler2D uGcMask;
uniform float uGcSize;
uniform float uGcMeshRes;
uniform float uGcRender;
uniform float uGcTile;
uniform float uGcRadius;
uniform float uGcTime;
uniform float uGcSway;
uniform float uAtmoSeaY;
varying vec3 vLkGcWp;
varying float vGcDry;
float gcH(vec2 uvc){
return texture2D(uGcHeight,(uvc*(uGcSize-1.0)+0.5)/uGcSize).r;
}
` +
        shader.vertexShader.replace(
          '#include <project_vertex>',
          `
vec2 gcP0=instanceMatrix[3].xz;
float gcSeed=instanceMatrix[3].y;
vec2 gcCam=cameraPosition.xz;
vec2 gcPw=gcP0+uGcTile*floor((gcCam-gcP0)/uGcTile+0.5);
vec2 gcUv=gcPw/uGcRender+0.5;
` +
            // Height exactly as the terrain mesh interpolates it (same grid, same diagonal).
            `vec2 gcG=clamp(gcUv,0.0,1.0)*uGcMeshRes;
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
` +
            // Dry and lush patches inside a biome (~0.1-1 km), thinner cover in the dry ones.
            `float gcPatch=sin(gcPw.x*0.83+1.7)*sin(gcPw.y*0.71+0.4)*0.6+sin(gcPw.x*2.9-gcPw.y*2.3+3.1)*0.4;
vGcDry=clamp(gcM.b+gcPatch*0.22,0.0,1.0);
float gcDens=gcM.r*(1.0-0.25*max(gcPatch,0.0));
float gcDist=length(vec3(gcPw.x-gcCam.x,gcY-cameraPosition.y,gcPw.y-gcCam.y));
float gcKeep=step(gcSeed,gcDens)
*(1.0-smoothstep(uGcRadius*0.65,uGcRadius,gcDist))
*step(uAtmoSeaY+0.02,gcY)
*step(0.0,gcUv.x)*step(gcUv.x,1.0)*step(0.0,gcUv.y)*step(gcUv.y,1.0);
vec3 gcT=transformed;
gcT.y*=gcM.g*2.0*(0.85+0.3*fract(gcSeed*37.0));
float gcW=sin(uGcTime*1.6+gcPw.x*3.1+gcPw.y*2.3)+0.5*sin(uGcTime*2.7+gcPw.x*5.3);
gcT.x+=gcW*uGcSway*gcT.y*gcT.y;
gcT.z+=cos(uGcTime*1.3+gcPw.y*2.7)*uGcSway*0.6*gcT.y*gcT.y;
vec3 gcLocal=mat3(instanceMatrix)*gcT*gcKeep;
vec4 gcWorld=vec4(gcPw.x+gcLocal.x,gcY+gcLocal.y-0.002,gcPw.y+gcLocal.z,1.0);
vLkGcWp=gcWorld.xyz;
vec4 mvPosition=viewMatrix*gcWorld;
gl_Position=projectionMatrix*mvPosition;
`,
        );
      shader.fragmentShader =
        'varying vec3 vLkGcWp;\nvarying float vGcDry;\n' +
        ATMOSPHERE_FRAGMENT_PARS +
        shader.fragmentShader
          .replace(
            '#include <color_fragment>',
            `
#include <color_fragment>
diffuseColor.rgb*=mix(vec3(1.0,1.04,0.95),vec3(1.7,1.38,0.64),vGcDry);
diffuseColor.rgb*=mix(0.6,1.0,atmoCloudShadow(vLkGcWp));
`,
          )
          .replace('#include <fog_fragment>', atmosphereFragmentApply('vLkGcWp'));
    };
    mat.customProgramCacheKey = () => 'wv2-look-groundcover-v2';
    return mat;
  }, [textures, instances, meshResolution, renderSize, radiusUnits, timeUniform, kind]);

  useEffect(() => () => instances.geometry.dispose(), [instances]);
  useEffect(() => () => material.dispose(), [material]);

  // Hide the whole draw when the camera is higher above the ground than the cover radius: every
  // blade would be collapsed anyway, but its vertices would still be shaded.
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
  const bladeCount = tier === 'high' ? 40000 : tier === 'medium' ? 20000 : 8000;
  const bladeRadiusM = tier === 'low' ? 30 : 45;
  const tuftCount = tier === 'high' ? 14000 : tier === 'medium' ? 7000 : 0;
  const heightUnits = renderSize * heightRatio;
  const unitsPerM = renderSize / (bundle.extentM || WORLD_EXTENT_M);

  const textures = useMemo<GroundCoverTextures>(() => {
    const { size, elevation, biome, slope } = world;
    // Terrain height in scene units, filtered like the mesh's own bilinear sampling.
    const h = new Float32Array(size * size);
    for (let i = 0; i < h.length; i += 1) h[i] = elevation[i] * heightUnits;
    const heightTex = new THREE.DataTexture(h, size, size, THREE.RedFormat, THREE.FloatType);
    heightTex.minFilter = THREE.LinearFilter;
    heightTex.magFilter = THREE.LinearFilter;
    heightTex.needsUpdate = true;
    // R: cover density from the biome, thinned on steep ground. G: grass height / 2. B: dryness.
    const M = 512;
    const mask = new Uint8Array(M * M * 4);
    for (let y = 0; y < M; y += 1) {
      for (let x = 0; x < M; x += 1) {
        const sx = Math.min(size - 1, Math.round((x / (M - 1)) * (size - 1)));
        const sy = Math.min(size - 1, Math.round((y / (M - 1)) * (size - 1)));
        const i = sy * size + sx;
        const s = slope?.[i] ?? 0;
        const steep = Math.min(1, Math.max(0, (s - 0.25) / 0.2));
        const b = biome[i];
        const o = (y * M + x) * 4;
        mask[o] = Math.round((BIOME_COVER[b] ?? 0.3) * (1 - steep) * 255);
        mask[o + 1] = Math.round(Math.min(2, BIOME_GRASS_HEIGHT[b] ?? 1) * 127.5);
        mask[o + 2] = Math.round((BIOME_GRASS_DRY[b] ?? 0.4) * 255);
        mask[o + 3] = 255;
      }
    }
    const maskTex = new THREE.DataTexture(mask, M, M, THREE.RGBAFormat, THREE.UnsignedByteType);
    maskTex.minFilter = THREE.LinearFilter;
    maskTex.magFilter = THREE.LinearFilter;
    maskTex.needsUpdate = true;
    return { heightTex, maskTex, size };
  }, [world, heightUnits]);

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
      <GroundCoverLayer kind="blades" count={bladeCount} radiusM={bladeRadiusM} seed={bundle.seed ^ 0x6b1d} {...common} />
      {tuftCount > 0 ? (
        <GroundCoverLayer kind="tufts" count={tuftCount} radiusM={320} seed={bundle.seed ^ 0x6c0d} {...common} />
      ) : null}
    </>
  );
};

export default WorldV2GroundCover;
