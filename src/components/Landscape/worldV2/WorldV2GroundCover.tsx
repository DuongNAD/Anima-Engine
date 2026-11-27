import React, { useEffect, useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { sampleElevation } from '../utils/worldSample';
import type { World } from '../utils/worldGen';
import type { QualityTier } from '../utils/qualityTiers';
import { WORLD_EXTENT_M, type WorldV2LoadedBundle } from './types';
import { buildWorldV2GrassMask, GRASS_PATCH_GLSL } from './grassTables';
import {
  ATMOSPHERE_FRAGMENT_PARS,
  atmosphereFragmentApply,
  bindAtmosphereUniforms,
} from '../materials/atmosphere';

// prototype, to be replaced — WV2-LOOK r10 ground cover: a continuous sward at eye level in
// three instanced layers:
//   1. `fine`    (near camera): thin distributed grass blades forming a continuous sward;
//   2. `tussock` (overlapping mid-range): wider arching tussocks tapering seamlessly into turf;
//   3. `herbs`   (high/medium): low leafy clumps topped with small wildflower petals.
// All layers share `grassTables.ts` (zero on sand, rock, snow, mud, or water) and the exact
// dry/lush patch field mixed into the terrain turf shader. Decoration only: no colliders.

export interface WorldV2GroundCoverProps {
  bundle: WorldV2LoadedBundle;
  world: World;
  renderSize?: number;
  heightRatio?: number;
  meshResolution?: number;
  tier?: QualityTier;
}

type CoverKind = 'fine' | 'tussock' | 'herbs';

function makeFineGrassGeometry(bladeCount: number): THREE.BufferGeometry {
  // Distributed cluster of thin, slightly arching tapered blades across a unit-diameter patch
  // (NOT radiating from one pinpoint spike). Base color harmonizes with the shaded turf under it.
  const pos: number[] = [];
  const col: number[] = [];
  const nrm: number[] = [];
  const base = new THREE.Color('#31501b');
  const tip = new THREE.Color('#86a847');
  const rng = seeded(0x91f3);
  for (let k = 0; k < bladeCount; k += 1) {
    const radialAngle = (k / bladeCount) * Math.PI * 2 + (rng() - 0.5) * 0.45;
    const rad = Math.sqrt((k + 0.35) / bladeCount) * 0.54;
    const ox = Math.cos(radialAngle) * rad;
    const oz = Math.sin(radialAngle) * rad;
    const faceAngle = rng() * Math.PI * 2;
    const ca = Math.cos(faceAngle);
    const sa = Math.sin(faceAngle);
    const w = 0.03 + rng() * 0.016;
    const lean = 0.1 + rng() * 0.16;
    const hgt = 0.65 + rng() * 0.4;
    const midY = hgt * 0.52;
    const midLean = lean * 0.35;
    const midW = w * 0.58;
    // Lower quad (2 tris) + pointed tip triangle (3 tris per blade), slightly bent outward.
    const x0L = ox - sa * w;
    const z0L = oz + ca * w;
    const x0R = ox + sa * w;
    const z0R = oz - ca * w;
    const x1L = ox + ca * midLean - sa * midW;
    const z1L = oz + sa * midLean + ca * midW;
    const x1R = ox + ca * midLean + sa * midW;
    const z1R = oz + sa * midLean - ca * midW;
    const xt = ox + ca * lean;
    const zt = oz + sa * lean;
    const nx = ca * 0.25;
    const ny = 0.94;
    const nz = sa * 0.25;
    const midC = new THREE.Color().copy(base).lerp(tip, 0.55);

    const pushV = (x: number, y: number, z: number, c: THREE.Color) => {
      pos.push(x, y, z);
      col.push(c.r, c.g, c.b);
      nrm.push(nx, ny, nz);
    };
    pushV(x0L, 0, z0L, base);
    pushV(x0R, 0, z0R, base);
    pushV(x1R, midY, z1R, midC);
    pushV(x0L, 0, z0L, base);
    pushV(x1R, midY, z1R, midC);
    pushV(x1L, midY, z1L, midC);
    pushV(x1L, midY, z1L, midC);
    pushV(x1R, midY, z1R, midC);
    pushV(xt, hgt, zt, tip);
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  g.setAttribute('color', new THREE.Float32BufferAttribute(col, 3));
  g.setAttribute('normal', new THREE.Float32BufferAttribute(nrm, 3));
  return g;
}

function makeTussockGeometry(): THREE.BufferGeometry {
  // Eight arching tussock blades distributed across a patch so mid-range meadows read as plush
  // grass clumps seamlessly matching the close fine layer.
  const pos: number[] = [];
  const col: number[] = [];
  const nrm: number[] = [];
  const base = new THREE.Color('#2f4c19');
  const tip = new THREE.Color('#7c9a41');
  for (let k = 0; k < 8; k += 1) {
    const a = (k / 8) * Math.PI * 2 + (k % 2) * 0.35;
    const ca = Math.cos(a);
    const sa = Math.sin(a);
    const off = (k % 2 === 0 ? 0.18 : 0.34);
    const ox = ca * off;
    const oz = sa * off;
    const w = 0.11;
    const lean = 0.26 + (k % 3) * 0.08;
    const hgt = 0.74 + (k % 3) * 0.14;
    pos.push(ox - sa * w, 0, oz + ca * w, ox + sa * w, 0, oz - ca * w, ox + ca * lean, hgt, oz + sa * lean);
    col.push(base.r, base.g, base.b, base.r, base.g, base.b, tip.r, tip.g, tip.b);
    for (let v = 0; v < 3; v += 1) nrm.push(ca * 0.35, 0.92, sa * 0.35);
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  g.setAttribute('color', new THREE.Float32BufferAttribute(col, 3));
  g.setAttribute('normal', new THREE.Float32BufferAttribute(nrm, 3));
  return g;
}

function makeHerbGeometry(): THREE.BufferGeometry {
  // Four low basal leaves + slender stem + 3-petal wildflower head (neutral pale petals tinted by
  // per-instance color in the vertex/fragment stage).
  const pos: number[] = [];
  const col: number[] = [];
  const nrm: number[] = [];
  const leafBase = new THREE.Color('#254217');
  const leafTip = new THREE.Color('#5a8030');
  const flower = new THREE.Color('#f4f4ee');
  for (let k = 0; k < 4; k += 1) {
    const a = (k / 4) * Math.PI * 2 + 0.3;
    const ca = Math.cos(a);
    const sa = Math.sin(a);
    pos.push(-sa * 0.06, 0, ca * 0.06, sa * 0.06, 0, -ca * 0.06, ca * 0.26, 0.48, sa * 0.26);
    col.push(leafBase.r, leafBase.g, leafBase.b, leafBase.r, leafBase.g, leafBase.b, leafTip.r, leafTip.g, leafTip.b);
    for (let v = 0; v < 3; v += 1) nrm.push(ca * 0.3, 0.94, sa * 0.3);
  }
  for (let k = 0; k < 4; k += 1) {
    const a = (k / 4) * Math.PI * 2 + 0.15;
    const ca = Math.cos(a);
    const sa = Math.sin(a);
    const ox = ca * 0.04;
    const oz = sa * 0.04;
    const spread = 0.24;
    pos.push(
      ox - sa * 0.075,
      0.84,
      oz + ca * 0.075,
      ox + sa * 0.075,
      0.84,
      oz - ca * 0.075,
      ox + ca * spread,
      1.02,
      oz + sa * spread,
    );
    col.push(flower.r, flower.g, flower.b, flower.r, flower.g, flower.b, flower.r, flower.g, flower.b);
    for (let v = 0; v < 3; v += 1) nrm.push(ca * 0.2, 0.97, sa * 0.2);
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  g.setAttribute('color', new THREE.Float32BufferAttribute(col, 3));
  g.setAttribute('normal', new THREE.Float32BufferAttribute(nrm, 3));
  return g;
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
  bladeDetail: number;
  seed: number;
  textures: GroundCoverTextures;
  world: World;
  renderSize: number;
  heightUnits: number;
  meshResolution: number;
  unitsPerM: number;
}

const HERB_PALETTE: ReadonlyArray<readonly [number, number, number]> = [
  [1.05, 0.92, 0.35], // warm buttercup yellow
  [1.02, 1.0, 0.88], // daisy cream white
  [0.86, 0.72, 1.06], // wild lavender
  [1.05, 0.68, 0.6], // meadow coral
  [0.64, 0.82, 1.08], // alpine bluebell
];

const GroundCoverLayer: React.FC<GroundCoverLayerProps> = ({
  kind,
  count,
  radiusM,
  innerFadeM = 0,
  bladeDetail,
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
        hM = 0.28 + rnd() * 0.42;
        wM = 0.56 + rnd() * 0.42;
      } else if (kind === 'tussock') {
        const tall = rnd() < 0.2;
        hM = tall ? 0.58 + rnd() * 0.56 : 0.34 + rnd() * 0.44;
        wM = tall ? 1.15 + rnd() * 0.65 : 0.8 + rnd() * 0.58;
      } else {
        hM = 0.34 + rnd() * 0.3;
        wM = 0.48 + rnd() * 0.32;
      }
      e.set((rnd() - 0.5) * 0.14, rnd() * Math.PI * 2, (rnd() - 0.5) * 0.14);
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
        const v = 0.82 + rnd() * 0.36;
        const dry = rnd();
        colors[k * 3] = v * (0.94 + dry * 0.2);
        colors[k * 3 + 1] = v * (0.98 + dry * 0.06);
        colors[k * 3 + 2] = v * (0.98 - dry * 0.24);
      }
    }
    const geometry =
      kind === 'fine'
        ? makeFineGrassGeometry(bladeDetail)
        : kind === 'tussock'
          ? makeTussockGeometry()
          : makeHerbGeometry();
    return {
      geometry,
      matrices,
      colors,
      tile,
    };
  }, [kind, bladeDetail, seed, count, radiusUnits, unitsPerM]);

  const timeUniform = useMemo(() => ({ value: 0 }), []);

  const material = useMemo(() => {
    const mat = new THREE.MeshStandardMaterial({
      vertexColors: true,
      roughness: 0.9,
      metalness: 0,
      side: THREE.DoubleSide,
    });
    const uniforms = {
      uGcHeight: { value: textures.heightTex },
      uGcMask: { value: textures.maskTex },
      uGcSize: { value: textures.size },
      uGcMeshRes: { value: meshResolution },
      uGcRender: { value: renderSize },
      uGcTile: { value: instances.tile },
      uGcRadius: { value: radiusUnits },
      uGcInner: { value: innerUnits },
      uGcIsHerb: { value: kind === 'herbs' ? 1 : 0 },
      uGcTime: timeUniform,
      uGcSway: { value: kind === 'fine' ? 0.2 : kind === 'herbs' ? 0.16 : 0.1 },
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
uniform float uGcInner;
uniform float uGcIsHerb;
uniform float uGcTime;
uniform float uGcSway;
uniform float uAtmoSeaY;
varying vec3 vLkGcWp;
varying float vGcDry;
varying float vGcTip;
varying float vGcFlower;
float gcH(vec2 uvc){
return texture2D(uGcHeight,(uvc*(uGcSize-1.0)+0.5)/uGcSize).r;
}
${GRASS_PATCH_GLSL}
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
float gcPatch=lkGrassPatch(gcPw);
vGcDry=clamp(gcM.b+gcPatch*0.24,0.0,1.0);
float gcDens=uGcIsHerb>0.5
?gcM.a*(1.0-0.55*vGcDry)
:gcM.r*(1.0-0.20*max(gcPatch,0.0));
float gcDist=length(vec3(gcPw.x-gcCam.x,gcY-cameraPosition.y,gcPw.y-gcCam.y));
float gcOuterFade=1.0-smoothstep(uGcRadius*0.52,uGcRadius,gcDist);
float gcInnerScale=uGcInner>0.001?mix(0.72,1.0,smoothstep(uGcInner*0.5,uGcInner*1.4,gcDist)):1.0;
float gcRingThresh=uGcInner>0.001?gcOuterFade:1.0;
float gcKeep=step(gcSeed,gcDens*mix(1.0,0.65+0.35*gcRingThresh,step(0.001,gcDens)))
*gcOuterFade*gcInnerScale
*step(uAtmoSeaY+0.02,gcY)
*step(0.0,gcUv.x)*step(gcUv.x,1.0)*step(0.0,gcUv.y)*step(0.0,1.0-gcUv.y);
vec3 gcT=transformed;
vGcTip=clamp(gcT.y,0.0,1.0);
vGcFlower=uGcIsHerb>0.5?step(0.65,gcT.y):0.0;
float hBio=max(0.25,gcM.g*2.0);
gcT.y*=mix(hBio,0.55+hBio*0.45,uGcIsHerb)*(0.82+0.36*fract(gcSeed*37.0));
float gcW=sin(uGcTime*1.6+gcPw.x*3.1+gcPw.y*2.3)+0.5*sin(uGcTime*2.7+gcPw.x*5.3);
gcT.x+=gcW*uGcSway*gcT.y*gcT.y;
gcT.z+=cos(uGcTime*1.3+gcPw.y*2.7)*uGcSway*0.6*gcT.y*gcT.y;
vec3 gcLocal=mat3(instanceMatrix)*gcT*gcKeep;
vec4 gcWorld=vec4(gcPw.x+gcLocal.x,gcY+gcLocal.y-0.003,gcPw.y+gcLocal.z,1.0);
vLkGcWp=gcWorld.xyz;
vec4 mvPosition=viewMatrix*gcWorld;
gl_Position=projectionMatrix*mvPosition;
`,
        );
      shader.fragmentShader =
        'varying vec3 vLkGcWp;\nvarying float vGcDry;\nvarying float vGcTip;\nvarying float vGcFlower;\n' +
        ATMOSPHERE_FRAGMENT_PARS +
        shader.fragmentShader
          .replace(
            '#include <color_fragment>',
            `
#include <color_fragment>
vec3 gcTint=mix(vec3(0.95,1.08,0.88),vec3(1.58,1.30,0.58),vGcDry);
diffuseColor.rgb*=mix(gcTint,vec3(1.18),vGcFlower);
float gcBacklit=pow(max(dot(normalize(vLkGcWp-cameraPosition),normalize(uAtmoSunDir)),0.0),3.0);
diffuseColor.rgb+=diffuseColor.rgb*gcBacklit*0.42*vGcTip*(1.0-vGcFlower*0.6);
diffuseColor.rgb*=mix(0.62,1.0,atmoCloudShadow(vLkGcWp));
`,
          )
          .replace('#include <fog_fragment>', atmosphereFragmentApply('vLkGcWp'));
    };
    mat.customProgramCacheKey = () => `wv2-look-groundcover-v3-${kind}`;
    return mat;
  }, [textures, instances, meshResolution, renderSize, radiusUnits, innerUnits, timeUniform, kind]);

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
  const fineCount = tier === 'high' ? 72000 : tier === 'medium' ? 36000 : 14000;
  const fineRadiusM = tier === 'low' ? 15 : 18;
  const fineBladeDetail = tier === 'low' ? 7 : 12;
  const tussockCount = tier === 'high' ? 36000 : tier === 'medium' ? 18000 : 7000;
  const tussockRadiusM = tier === 'high' ? 115 : tier === 'medium' ? 90 : 55;
  const herbCount = tier === 'high' ? 9000 : tier === 'medium' ? 4500 : 0;
  const herbRadiusM = tier === 'high' ? 28 : 24;
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
        bladeDetail={fineBladeDetail}
        seed={bundle.seed ^ 0x6b1d}
        {...common}
      />
      <GroundCoverLayer
        kind="tussock"
        count={tussockCount}
        radiusM={tussockRadiusM}
        innerFadeM={fineRadiusM}
        bladeDetail={8}
        seed={bundle.seed ^ 0x6c0d}
        {...common}
      />
      {herbCount > 0 ? (
        <GroundCoverLayer
          kind="herbs"
          count={herbCount}
          radiusM={herbRadiusM}
          bladeDetail={4}
          seed={bundle.seed ^ 0x6d7f}
          {...common}
        />
      ) : null}
    </>
  );
};

export default WorldV2GroundCover;
