import React, { useEffect, useMemo } from 'react';
import * as THREE from 'three';
import { type World } from '../utils/worldGen';
import { sampleMeshHeight } from '../utils/worldSample';
import type { QualityTier } from '../utils/qualityTiers';
import {
  ELEV_SPAN_M,
  FLORA_SECTORS_PER_SIDE,
  SEA_LEVEL_NORM,
  WORLD_EXTENT_M,
  type WorldV2LoadedBundle,
} from './types';
import { LOOK_SPECIES, LOOK_SPECIES_SIZE, makeLookFloraGeometry } from './floraLook';
import {
  ATMOSPHERE_FRAGMENT_PARS,
  atmosphereFragmentApply,
  bindAtmosphereUniforms,
} from '../materials/atmosphere';

export interface WorldV2FloraProps {
  bundle: WorldV2LoadedBundle;
  world: World;
  renderSize?: number;
  heightRatio?: number;
  meshResolution?: number;
  tier?: QualityTier;
}

interface SpeciesBatch {
  speciesId: number;
  geometry: THREE.BufferGeometry;
  matrices: Float32Array;
  colors: Float32Array;
  count: number;
}

const SPECIES_COUNT = 8;

// prototype, to be replaced — WV2-LOOK placement rules.
//
// Each baked flora point becomes a small STAND of plants at real-world size (8-38 m trees,
// 3-11 m boulders) instead of one 40-130 m toy tree: the bake's spacing (~70-200 m) is far
// coarser than a real canopy, so a stand fills the gap without inventing new placement data.
// Stand size follows a low-frequency clump noise and the local biome purity, so forest edges
// thin out raggedly instead of stopping on a line; plants on cliff-steep cells are dropped.

/** Stand radius in metres per species. */
const STAND_RADIUS_M: ReadonlyArray<number> = [34, 30, 30, 18, 14, 30, 22, 16];
/** Plants per stand at `high`; medium/low scale this down. */
const STAND_BASE: ReadonlyArray<number> = [5, 4, 4, 2, 4, 2, 3, 4];

function hash2(x: number, y: number, seed: number): number {
  let h = Math.imul(x | 0, 374761393) ^ Math.imul(y | 0, 668265263) ^ Math.imul(seed, 2246822519);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  h ^= h >>> 16;
  return (h >>> 0) / 4294967296;
}

function valueNoise(x: number, y: number, seed: number): number {
  const xi = Math.floor(x);
  const yi = Math.floor(y);
  const fx = x - xi;
  const fy = y - yi;
  const sx = fx * fx * (3 - 2 * fx);
  const sy = fy * fy * (3 - 2 * fy);
  const a = hash2(xi, yi, seed);
  const b = hash2(xi + 1, yi, seed);
  const c = hash2(xi, yi + 1, seed);
  const d = hash2(xi + 1, yi + 1, seed);
  return (a * (1 - sx) + b * sx) * (1 - sy) + (c * (1 - sx) + d * sx) * sy;
}

function seeded(seed: number): () => number {
  let s = seed >>> 0 || 1;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

function makeFloraLookMaterial(farUnits: number): THREE.MeshStandardMaterial {
  const material = new THREE.MeshStandardMaterial({
    vertexColors: true,
    roughness: 0.92,
    metalness: 0.0,
  });
  const uFloraFar = { value: farUnits };
  material.onBeforeCompile = (shader) => {
    bindAtmosphereUniforms(shader.uniforms);
    shader.uniforms.uFloraFar = uFloraFar;
    // Past ~14 km a plant is under a pixel and the terrain's canopy layer already carries the
    // forest; shrink instances to their origin over the last 30% of the range so the GPU
    // rasterises nothing for them (degenerate triangles), with no visible pop.
    shader.vertexShader =
      'varying vec3 vLkFloraWp;\nuniform float uFloraFar;\n' +
      shader.vertexShader.replace(
        '#include <project_vertex>',
        `
#ifdef USE_INSTANCING
{
vec3 lkOrigin=(modelMatrix*instanceMatrix*vec4(0.0,0.0,0.0,1.0)).xyz;
float lkKeep=1.0-smoothstep(uFloraFar*0.7,uFloraFar,distance(lkOrigin,cameraPosition));
transformed*=lkKeep;
}
#endif
#include <project_vertex>
vec4 lkFloraWp4=vec4(transformed,1.0);
#ifdef USE_INSTANCING
lkFloraWp4=instanceMatrix*lkFloraWp4;
#endif
vLkFloraWp=(modelMatrix*lkFloraWp4).xyz;
`,
      );
    // Rocks in the swash band are wet: darker (CST-04).
    shader.fragmentShader =
      'varying vec3 vLkFloraWp;\n' +
      ATMOSPHERE_FRAGMENT_PARS +
      shader.fragmentShader
        .replace(
          '#include <color_fragment>',
          `
#include <color_fragment>
diffuseColor.rgb*=mix(1.0,0.55,1.0-smoothstep(uAtmoSeaY+0.01,uAtmoSeaY+0.07,vLkFloraWp.y));
diffuseColor.rgb*=mix(0.6,1.0,atmoCloudShadow(vLkFloraWp));
`,
        )
        .replace('#include <fog_fragment>', atmosphereFragmentApply('vLkFloraWp'));
  };
  material.customProgramCacheKey = () => 'wv2-look-flora-v4';
  return material;
}

/**
 * Renders decoded `flora_tiles` (`ANMF v1`, `FLORA_FLAG_DECORATIVE`) as up to 8 `InstancedMesh`
 * batches (<= 8 draw calls total). Strictly decorative — mounted only inside `showDecoration`.
 */
export const WorldV2Flora: React.FC<WorldV2FloraProps> = ({
  bundle,
  world,
  renderSize = 1200,
  heightRatio = 0.14,
  meshResolution = 384,
  tier = 'high',
}) => {
  const heightUnits = renderSize * heightRatio;
  const stride = tier === 'high' ? 1 : tier === 'medium' ? 2 : 4;
  const standScale = tier === 'high' ? 1 : tier === 'medium' ? 0.8 : 0.7;
  // WV2-LOOK: at real-world size a tree is under one texel of the 2048 shadow map spread over
  // the whole map, so its cast shadow was aliased noise that cost ~25-35% of the high-tier
  // frame. The canopy layer in the terrain shader carries the dark gaps instead.

  const material = useMemo(() => makeFloraLookMaterial(renderSize * 0.35), [renderSize]);

  const batches = useMemo<SpeciesBatch[]>(() => {
    const unitsPerM = renderSize / (bundle.extentM || WORLD_EXTENT_M);
    const unitsPerMY = heightUnits / ELEV_SPAN_M;
    const seaY = SEA_LEVEL_NORM * heightUnits;
    const { size } = bundle;
    const perSpecies: number[][] = Array.from({ length: SPECIES_COUNT }, () => []);
    const perSpeciesColor: number[][] = Array.from({ length: SPECIES_COUNT }, () => []);
    const rnd = seeded(bundle.seed ^ 0x5eed);
    const dummy = new THREE.Object3D();
    const tint = new THREE.Color();

    let idx = 0;
    for (const sector of bundle.floraSectors) {
      for (const inst of sector.instances) {
        if (stride > 1 && idx % stride !== 0) {
          idx += 1;
          continue;
        }
        idx += 1;
        const sid = inst.species_id < SPECIES_COUNT ? inst.species_id : LOOK_SPECIES.Bush;
        const u = (sector.sectorX + inst.x_local_u16 / 65535.0) / FLORA_SECTORS_PER_SIDE;
        const v = (sector.sectorZ + inst.z_local_u16 / 65535.0) / FLORA_SECTORS_PER_SIDE;
        // The terrain's map-edge falloff band (see terrainPbrMaterial) is under water.
        if (Math.min(u, 1 - u, v, 1 - v) < 0.085) continue;
        const cx = Math.min(size - 1, Math.max(0, Math.floor(u * size)));
        const cy = Math.min(size - 1, Math.max(0, Math.floor(v * size)));
        const ci = cy * size + cx;

        // Cliff-steep cells: no trees (boulders may stay).
        const nx = (bundle.normalRg[ci * 2] / 255) * 2 - 1;
        const nz = (bundle.normalRg[ci * 2 + 1] / 255) * 2 - 1;
        const ny = Math.sqrt(Math.max(0, 1 - nx * nx - nz * nz));
        const slope = 1 - ny;
        if (sid !== LOOK_SPECIES.Rock && slope > 0.32) continue;

        // Biome purity: 1 deep inside a biome, lower on an ecotone.
        const purity = (bundle.biomeWeightsRgba[ci * 4] ?? 255) / 255;
        const wx0 = (u - 0.5) * renderSize;
        const wz0 = (v - 0.5) * renderSize;
        const clump =
          valueNoise(wx0 * 0.045, wz0 * 0.045, 17) * 0.65 + valueNoise(wx0 * 0.16, wz0 * 0.16, 23) * 0.35;
        const keep = clump * 0.9 + purity * 0.55 - slope * 1.2;
        if (keep < 0.42) continue;
        const density = Math.min(1.25, Math.max(0.25, (keep - 0.42) * 2.2));
        const k = Math.max(1, Math.round(STAND_BASE[sid] * standScale * density + rnd() * 0.8));

        const sizeSpec = LOOK_SPECIES_SIZE[sid];
        const scale01 = inst.scale_u8 / 255;
        const radius = STAND_RADIUS_M[sid] * unitsPerM;
        const yaw0 = (inst.yaw_u8 / 255) * Math.PI * 2;
        // Clumped, not evenly spread: members gather around one or two sub-centres inside the
        // stand, leaving gaps (natural regeneration patches), and the whole stand is nudged.
        const subN = k > 2 && rnd() < 0.6 ? 2 : 1;
        const subs: Array<[number, number]> = [];
        for (let q = 0; q < subN; q += 1) {
          const sa = rnd() * Math.PI * 2;
          const sr = Math.sqrt(rnd()) * radius * 0.75;
          subs.push([wx0 + Math.cos(sa) * sr, wz0 + Math.sin(sa) * sr]);
        }
        for (let m = 0; m < k; m += 1) {
          const a = rnd() * Math.PI * 2;
          const sc = subs[m % subN];
          const r = Math.pow(rnd(), 0.8) * radius * 0.55;
          const px = sc[0] + Math.cos(a) * r;
          const pz = sc[1] + Math.sin(a) * r;
          const pu = px / renderSize + 0.5;
          const pv = pz / renderSize + 0.5;
          if (pu < 0 || pu > 1 || pv < 0 || pv > 1) continue;
          const hM =
            sizeSpec.minM +
            (sizeSpec.maxM - sizeSpec.minM) * Math.min(1, Math.max(0, scale01 * 0.6 + rnd() * 0.55 - 0.1));
          const h = hM * unitsPerM;
          const wJ = 0.72 + rnd() * 0.6;
          const groundY = sampleMeshHeight(world, pu, pv, meshResolution) * heightUnits;
          // Nothing grows under the sea; boulders may sit in the swash, half buried.
          if (sid === LOOK_SPECIES.Rock ? groundY < seaY - 2 * unitsPerMY : groundY < seaY + 0.6 * unitsPerMY) continue;
          const py = groundY - h * (sid === LOOK_SPECIES.Rock ? 0.3 : 0.03);
          dummy.position.set(px, py, pz);
          dummy.rotation.set((rnd() - 0.5) * 0.08, yaw0 + rnd() * Math.PI * 2, (rnd() - 0.5) * 0.08);
          dummy.scale.set(h * wJ, h * (0.88 + rnd() * 0.3), h * (0.72 + rnd() * 0.6));
          dummy.updateMatrix();
          const e = dummy.matrix.elements;
          const list = perSpecies[sid];
          for (let j = 0; j < 16; j += 1) list.push(e[j]);

          // Per-plant tint: value +-18%, drift between dry-warm and lush-cool.
          const val = 0.78 + rnd() * 0.4;
          const warm = rnd();
          if (sid === LOOK_SPECIES.Rock) {
            tint.setRGB(val, val * 0.98, val * (0.92 + warm * 0.1));
          } else {
            tint.setRGB(val * (0.9 + warm * 0.28), val * (0.98 + warm * 0.06), val * (1.06 - warm * 0.3));
          }
          perSpeciesColor[sid].push(tint.r, tint.g, tint.b);
        }
      }
    }

    const out: SpeciesBatch[] = [];
    for (let sid = 0; sid < SPECIES_COUNT; sid += 1) {
      const list = perSpecies[sid];
      if (list.length === 0) continue;
      out.push({
        speciesId: sid,
        geometry: makeLookFloraGeometry(sid),
        matrices: new Float32Array(list),
        colors: new Float32Array(perSpeciesColor[sid]),
        count: list.length / 16,
      });
    }
    return out;
  }, [bundle, world, renderSize, heightUnits, meshResolution, stride, standScale]);

  useEffect(() => {
    return () => {
      for (const b of batches) b.geometry.dispose();
    };
  }, [batches]);

  useEffect(() => {
    return () => {
      material.dispose();
    };
  }, [material]);

  if (batches.length === 0) return null;

  return (
    <group name="world-v2-flora" userData={{ decorative: true, rwLayer: 'flora' }}>
      {batches.map((batch) => (
        <instancedMesh
          key={batch.speciesId}
          args={[batch.geometry, material, batch.count]}
          castShadow={false}
          receiveShadow={false}
          name={`world-v2-flora-s${batch.speciesId}`}
          ref={(mesh: THREE.InstancedMesh | null) => {
            if (!mesh || !mesh.instanceMatrix) return;
            mesh.instanceMatrix.array.set(batch.matrices);
            mesh.instanceMatrix.needsUpdate = true;
            if (!mesh.instanceColor || mesh.instanceColor.count !== batch.count) {
              mesh.instanceColor = new THREE.InstancedBufferAttribute(batch.colors, 3);
            } else {
              (mesh.instanceColor.array as Float32Array).set(batch.colors);
              mesh.instanceColor.needsUpdate = true;
            }
            mesh.computeBoundingSphere();
          }}
        />
      ))}
    </group>
  );
};

export default WorldV2Flora;
