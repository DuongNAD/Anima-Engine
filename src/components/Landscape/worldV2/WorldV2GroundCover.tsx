import React, { useEffect, useMemo } from 'react';
import * as THREE from 'three';
import type { World } from '../utils/worldGen';
import type { QualityTier } from '../utils/qualityTiers';
import { WORLD_EXTENT_M, type WorldV2LoadedBundle } from './types';
import {
  ATMOSPHERE_FRAGMENT_PARS,
  atmosphereFragmentApply,
  bindAtmosphereUniforms,
} from '../materials/atmosphere';

// prototype, to be replaced — WV2-LOOK ground cover: grass tussocks and low shrub tufts within a
// few hundred metres of the camera, in ONE instanced draw call.
//
// Every tuft owns a fixed position inside a square tile twice the cover radius wide; the vertex
// shader moves it to the copy of that tile nearest the camera (so tufts never swim as the camera
// moves), reads the terrain height exactly as the terrain mesh interpolates it (same grid, same
// triangle split), and collapses tufts that are out of range, under water or on ground the biome
// mask says is bare (rock, snow, sand, water). Decoration only: no collider, no data written.
// High: 14k tufts; medium: 7k; low: off (no draw call).

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
  0, 0.12, 0.05, 0.9, 1.0, 0.85, 0.6, 0.5, 0.4, 0.55, 0.6, 0.08, 0, 0, 0, 0.3, 0.75, 0.8, 0.55, 0.1, 0, 0.5,
];

function makeTuftGeometry(): THREE.BufferGeometry {
  // Four crossed blades leaning outward around a tiny mound: 4 + 4 triangles.
  const pos: number[] = [];
  const col: number[] = [];
  const nrm: number[] = [];
  const base = new THREE.Color('#2a3516');
  const tip = new THREE.Color('#7d8a45');
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

function seeded(seed: number): () => number {
  let s = seed >>> 0 || 1;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

export const WorldV2GroundCover: React.FC<WorldV2GroundCoverProps> = ({
  bundle,
  world,
  renderSize = 1200,
  heightRatio = 0.14,
  meshResolution = 384,
  tier = 'high',
}) => {
  const count = tier === 'high' ? 14000 : tier === 'medium' ? 7000 : 0;
  const heightUnits = renderSize * heightRatio;
  const unitsPerM = renderSize / (bundle.extentM || WORLD_EXTENT_M);
  const radiusUnits = 320 * unitsPerM;

  const textures = useMemo(() => {
    if (count === 0) return null;
    const { size, elevation, biome, slope } = world;
    // Terrain height in scene units, filtered like the mesh's own bilinear sampling.
    const h = new Float32Array(size * size);
    for (let i = 0; i < h.length; i += 1) h[i] = elevation[i] * heightUnits;
    const heightTex = new THREE.DataTexture(h, size, size, THREE.RedFormat, THREE.FloatType);
    heightTex.minFilter = THREE.LinearFilter;
    heightTex.magFilter = THREE.LinearFilter;
    heightTex.needsUpdate = true;
    // Cover density from the biome, thinned on steep ground.
    const M = 512;
    const mask = new Uint8Array(M * M);
    for (let y = 0; y < M; y += 1) {
      for (let x = 0; x < M; x += 1) {
        const sx = Math.min(size - 1, Math.round((x / (M - 1)) * (size - 1)));
        const sy = Math.min(size - 1, Math.round((y / (M - 1)) * (size - 1)));
        const i = sy * size + sx;
        const s = slope?.[i] ?? 0;
        const steep = Math.min(1, Math.max(0, (s - 0.25) / 0.2));
        mask[y * M + x] = Math.round((BIOME_COVER[biome[i]] ?? 0.3) * (1 - steep) * 255);
      }
    }
    const maskTex = new THREE.DataTexture(mask, M, M, THREE.RedFormat, THREE.UnsignedByteType);
    maskTex.minFilter = THREE.LinearFilter;
    maskTex.magFilter = THREE.LinearFilter;
    maskTex.needsUpdate = true;
    return { heightTex, maskTex, size };
  }, [world, heightUnits, count]);

  const instances = useMemo(() => {
    if (count === 0) return null;
    const rnd = seeded(bundle.seed ^ 0x6c0d);
    const tile = radiusUnits * 2;
    const matrices = new Float32Array(count * 16);
    const colors = new Float32Array(count * 3);
    const m = new THREE.Matrix4();
    const q = new THREE.Quaternion();
    const e = new THREE.Euler();
    const sc = new THREE.Vector3();
    const p = new THREE.Vector3();
    for (let k = 0; k < count; k += 1) {
      const shrub = rnd() < 0.22;
      const hM = shrub ? 1.4 + rnd() * 1.6 : 0.6 + rnd() * 0.9;
      const wM = shrub ? 1.6 + rnd() * 1.4 : 0.8 + rnd() * 0.7;
      e.set((rnd() - 0.5) * 0.15, rnd() * Math.PI * 2, (rnd() - 0.5) * 0.15);
      q.setFromEuler(e);
      sc.set(wM * unitsPerM, hM * unitsPerM, wM * unitsPerM * (0.8 + rnd() * 0.4));
      p.set(rnd() * tile, rnd(), rnd() * tile); // y carries the density-test seed
      m.compose(p, q, sc);
      matrices.set(m.elements, k * 16);
      const v = 0.8 + rnd() * 0.4;
      const dry = rnd();
      colors[k * 3] = v * (0.9 + dry * 0.35);
      colors[k * 3 + 1] = v * (0.95 + dry * 0.1);
      colors[k * 3 + 2] = v * (1.0 - dry * 0.35);
    }
    return { geometry: makeTuftGeometry(), matrices, colors, tile };
  }, [bundle.seed, count, radiusUnits, unitsPerM]);

  const material = useMemo(() => {
    if (!textures || !instances) return null;
    const mat = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.95, metalness: 0, side: THREE.DoubleSide });
    const uniforms = {
      uGcHeight: { value: textures.heightTex },
      uGcMask: { value: textures.maskTex },
      uGcSize: { value: textures.size },
      uGcMeshRes: { value: meshResolution },
      uGcRender: { value: renderSize },
      uGcTile: { value: instances.tile },
      uGcRadius: { value: radiusUnits },
    };
    mat.onBeforeCompile = (shader) => {
      bindAtmosphereUniforms(shader.uniforms);
      Object.assign(shader.uniforms, uniforms);
      shader.vertexShader =
        `uniform sampler2D uGcHeight;
uniform sampler2D uGcMask;
uniform float uGcSize;
uniform float uGcMeshRes;
uniform float uGcRender;
uniform float uGcTile;
uniform float uGcRadius;
uniform float uAtmoSeaY;
varying vec3 vLkGcWp;
float gcH(vec2 uvc) {
  return texture2D(uGcHeight, (uvc * (uGcSize - 1.0) + 0.5) / uGcSize).r;
}
` +
        shader.vertexShader.replace(
          '#include <project_vertex>',
          `vec2 gcP0 = instanceMatrix[3].xz;
          float gcSeed = instanceMatrix[3].y;
          vec2 gcCam = cameraPosition.xz;
          vec2 gcPw = gcP0 + uGcTile * floor((gcCam - gcP0) / uGcTile + 0.5);
          vec2 gcUv = gcPw / uGcRender + 0.5;
          // Height exactly as the terrain mesh interpolates it (same grid, same diagonal).
          vec2 gcG = clamp(gcUv, 0.0, 1.0) * uGcMeshRes;
          vec2 gcI = floor(gcG);
          vec2 gcF = gcG - gcI;
          float hA = gcH(gcI / uGcMeshRes);
          float hB = gcH((gcI + vec2(1.0, 0.0)) / uGcMeshRes);
          float hC = gcH((gcI + vec2(0.0, 1.0)) / uGcMeshRes);
          float hD = gcH((gcI + vec2(1.0, 1.0)) / uGcMeshRes);
          float gcY = (gcF.x + gcF.y < 1.0)
            ? hA + (hB - hA) * gcF.x + (hC - hA) * gcF.y
            : hD + (hC - hD) * (1.0 - gcF.x) + (hB - hD) * (1.0 - gcF.y);
          float gcDens = texture2D(uGcMask, clamp(gcUv, 0.0, 1.0)).r;
          float gcDist = length(vec3(gcPw.x - gcCam.x, gcY - cameraPosition.y, gcPw.y - gcCam.y));
          float gcKeep = step(gcSeed, gcDens)
            * (1.0 - smoothstep(uGcRadius * 0.65, uGcRadius, gcDist))
            * step(uAtmoSeaY + 0.02, gcY)
            * step(0.0, gcUv.x) * step(gcUv.x, 1.0) * step(0.0, gcUv.y) * step(gcUv.y, 1.0);
          vec3 gcLocal = mat3(instanceMatrix) * transformed * gcKeep;
          vec4 gcWorld = vec4(gcPw.x + gcLocal.x, gcY + gcLocal.y - 0.002, gcPw.y + gcLocal.z, 1.0);
          vLkGcWp = gcWorld.xyz;
          vec4 mvPosition = viewMatrix * gcWorld;
          gl_Position = projectionMatrix * mvPosition;`,
        );
      shader.fragmentShader =
        'varying vec3 vLkGcWp;\n' +
        ATMOSPHERE_FRAGMENT_PARS +
        shader.fragmentShader
          .replace(
            '#include <color_fragment>',
            `#include <color_fragment>
            diffuseColor.rgb *= mix(0.6, 1.0, atmoCloudShadow(vLkGcWp));`,
          )
          .replace('#include <fog_fragment>', atmosphereFragmentApply('vLkGcWp'));
    };
    mat.customProgramCacheKey = () => 'wv2-look-groundcover-v1';
    return mat;
  }, [textures, instances, meshResolution, renderSize, radiusUnits]);

  useEffect(
    () => () => {
      textures?.heightTex.dispose();
      textures?.maskTex.dispose();
    },
    [textures],
  );
  useEffect(() => () => instances?.geometry.dispose(), [instances]);
  useEffect(() => () => material?.dispose(), [material]);

  if (!instances || !material) return null;
  return (
    <instancedMesh
      name="world-v2-flora-groundcover"
      args={[instances.geometry, material, count]}
      frustumCulled={false}
      castShadow={false}
      receiveShadow={false}
      userData={{ decorative: true, rwLayer: 'flora' }}
      ref={(mesh: THREE.InstancedMesh | null) => {
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

export default WorldV2GroundCover;
