import React, { useEffect, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { sampleElevation } from './utils/worldSample';
import {
  buildColorTexture,
  buildDetailTexture,
  buildNormalTexture,
  buildRiverMaskTexture,
  buildRoughnessTexture,
  buildSplatTexture,
  buildWorldV2TerrainTextures,
} from './utils/worldTerrainTextures';
import { sceneElapsed } from './utils/sceneClock';
import { isCaptureRequested } from './utils/captureMode';
import type { QualityTier } from './utils/qualityTiers';
import { createTerrainPbrMaterial } from './materials/terrainPbrMaterial';
import { ELEV_MIN_M, ELEV_SPAN_M, type WorldV2LoadedBundle } from './worldV2/types';
import { worldRenderScale } from './worldV2/worldScale';
import { carveWorldV2RiverAndWaterfallChannels } from './worldV2/channelCarve';

export interface WorldTerrainProps {
  world: World;
  /** Verified World v2 bundle (when active, replaces v1 procedural texture bakes). */
  worldV2?: WorldV2LoadedBundle | null;
  /** World-space direction to sun for baked 4-azimuth horizon shadowing. */
  sunDir?: [number, number, number];
  /** World-space width the terrain is rendered at, regardless of data resolution. */
  renderSize?: number;
  /** Peak height as a fraction of renderSize. */
  heightRatio?: number;
  /** Mesh resolution (segments per side). Decoupled from the data resolution. */
  meshResolution?: number;
  /** Quality tier controlling CC0 PBR array resolution (`512` on low, `1024` on medium/high). */
  tier?: QualityTier;
}

/**
 * Renders the huge SoA world as a single height-displaced mesh. The mesh is built at
 * `meshResolution` (e.g. 384) while COLOUR comes from a full data-resolution texture and the
 * fine relief from a residual normal map — so a 2048^2 world reads at full detail without a
 * 4M-vertex mesh. The texture palette is `BIOME_DISPLAY_RGB` (derived from `BIOME_RGB`), shared
 * with the minimap so the 3D world and minimap always match. The bakes themselves live in
 * `utils/worldTerrainTextures`, shared with `WorldTerrainLod`.
 */
export const WorldTerrain: React.FC<WorldTerrainProps> = ({
  world,
  worldV2 = null,
  sunDir = [0.6, 0.7, 0.35],
  renderSize = 400,
  heightRatio = 0.13,
  meshResolution = 256,
  tier = 'high',
}) => {
  const geometry = useMemo(() => {
    const res = meshResolution;
    const verts = (res + 1) * (res + 1);
    const positions = new Float32Array(verts * 3);
    const uvs = new Float32Array(verts * 2);
    const heightUnits = renderSize * heightRatio;

    for (let gy = 0; gy <= res; gy++) {
      for (let gx = 0; gx <= res; gx++) {
        const u = gx / res;
        const v = gy / res;
        const e = sampleElevation(world, u, v);
        const i = gy * (res + 1) + gx;

        positions[i * 3] = (u - 0.5) * renderSize; // X
        positions[i * 3 + 1] = e * heightUnits; // Y (up)
        positions[i * 3 + 2] = (v - 0.5) * renderSize; // Z
        uvs[i * 2] = u;
        uvs[i * 2 + 1] = v;
      }
    }

    if (worldV2 && tier !== 'low') {
      carveWorldV2RiverAndWaterfallChannels(positions, res, world, worldV2, renderSize, heightUnits);
    }

    const indices = new Uint32Array(res * res * 6);
    let o = 0;
    for (let gy = 0; gy < res; gy++) {
      for (let gx = 0; gx < res; gx++) {
        const a = gy * (res + 1) + gx;
        const b = a + 1;
        const c = a + (res + 1);
        const d = c + 1;
        // CCW from above so computeVertexNormals() yields +Y normals (no back-face culling).
        indices[o++] = a;
        indices[o++] = c;
        indices[o++] = b;
        indices[o++] = b;
        indices[o++] = c;
        indices[o++] = d;
      }
    }

    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geom.setAttribute('uv', new THREE.BufferAttribute(uvs, 2));
    geom.setIndex(new THREE.BufferAttribute(indices, 1));
    geom.computeVertexNormals();
    return geom;
  }, [world, worldV2, renderSize, heightRatio, meshResolution, tier]);

  const textures = useMemo(() => {
    if (worldV2) {
      return buildWorldV2TerrainTextures(worldV2, world);
    }
    return {
      colorMap: buildColorTexture(world),
      normalMap: buildNormalTexture(world, renderSize, heightRatio, meshResolution),
      roughnessMap: buildRoughnessTexture(world),
      detailMap: buildDetailTexture(),
      riverMaskMap: buildRiverMaskTexture(world),
      splatMap: buildSplatTexture(world),
      horizonMap: null as THREE.DataTexture | null,
      shoreSdfMap: null as THREE.DataTexture | null,
      hydroFlowMap: null as THREE.DataTexture | null,
      lookMaskMap: null as THREE.DataTexture | null,
      lookNoiseMap: null as THREE.DataTexture | null,
      lookGrassMap: null as THREE.DataTexture | null,
    };
  }, [world, worldV2, renderSize, heightRatio, meshResolution]);

  const terrainMat = useMemo(
    () =>
      createTerrainPbrMaterial({
        colorMap: textures.colorMap,
        normalMap: textures.normalMap,
        roughnessMap: textures.roughnessMap,
        detailMap: textures.detailMap,
        riverMaskMap: textures.riverMaskMap,
        splatMap: textures.splatMap,
        horizonMap: textures.horizonMap,
        lookMaskMap: textures.lookMaskMap,
        lookNoiseMap: textures.lookNoiseMap,
        lookGrassMap: textures.lookGrassMap,
        lookShoreMap: textures.shoreSdfMap,
        elevMinM: ELEV_MIN_M,
        elevSpanM: ELEV_SPAN_M,
        normalScale: worldV2 ? 0.45 : 1,
        heightUnits: renderSize * heightRatio,
        edgeScale: worldRenderScale(worldV2?.extentM).edgeScale,
        carveBanks: tier !== 'low',
      }),
    [textures, worldV2, renderSize, heightRatio, tier],
  );

  useEffect(() => {
    terrainMat.handle.applyPbrArrays(null);
    if (typeof window !== 'undefined' && isCaptureRequested(window.location.search)) {
      return;
    }
    let cancelled = false;
    void import('./assets/worldAssets').then(({ loadTerrainPbrArrays }) => {
      if (cancelled) return;
      return loadTerrainPbrArrays(tier).then((arrays) => {
        if (!cancelled && arrays) {
          terrainMat.handle.applyPbrArrays(arrays);
        }
      });
    });
    return () => {
      cancelled = true;
    };
  }, [terrainMat, tier]);

  useFrame((state) => {
    terrainMat.handle.updateTime(sceneElapsed(state.clock));
    terrainMat.handle.updateSunDir(sunDir);
  });

  useEffect(() => {
    return () => {
      textures.colorMap.dispose();
      textures.normalMap.dispose();
      textures.roughnessMap.dispose();
      textures.detailMap.dispose();
      textures.riverMaskMap.dispose();
      textures.splatMap.dispose();
      textures.horizonMap?.dispose();
      textures.shoreSdfMap?.dispose();
      textures.hydroFlowMap?.dispose();
      textures.lookMaskMap?.dispose();
      textures.lookNoiseMap?.dispose();
      textures.lookGrassMap?.dispose();
      terrainMat.handle.dispose();
      geometry.dispose();
    };
  }, [textures, terrainMat, geometry]);

  return (
    <mesh geometry={geometry} material={terrainMat.material} name="world-terrain" receiveShadow />
  );
};

export default WorldTerrain;
