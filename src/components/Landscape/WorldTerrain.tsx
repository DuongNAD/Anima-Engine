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
} from './utils/worldTerrainTextures';
import { sceneElapsed } from './utils/sceneClock';
import { isCaptureRequested } from './utils/captureMode';
import type { QualityTier } from './utils/qualityTiers';
import { createTerrainPbrMaterial } from './materials/terrainPbrMaterial';

export interface WorldTerrainProps {
  world: World;
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
  }, [world, renderSize, heightRatio, meshResolution]);

  const colorMap = useMemo(() => buildColorTexture(world), [world]);
  const normalMap = useMemo(
    () => buildNormalTexture(world, renderSize, heightRatio, meshResolution),
    [world, renderSize, heightRatio, meshResolution],
  );
  const roughnessMap = useMemo(() => buildRoughnessTexture(world), [world]);
  const detailMap = useMemo(() => buildDetailTexture(), []);

  // River-ribbon mask (feathered), so the shader knows where flowing water lies.
  const riverMaskMap = useMemo(() => buildRiverMaskTexture(world), [world]);
  const splatMap = useMemo(() => buildSplatTexture(world), [world]);

  const terrainMat = useMemo(
    () =>
      createTerrainPbrMaterial({
        colorMap,
        normalMap,
        roughnessMap,
        detailMap,
        riverMaskMap,
        splatMap,
        heightUnits: renderSize * heightRatio,
      }),
    [colorMap, normalMap, roughnessMap, detailMap, riverMaskMap, splatMap, renderSize, heightRatio],
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
  });

  useEffect(() => {
    return () => {
      colorMap.dispose();
      normalMap.dispose();
      roughnessMap.dispose();
      detailMap.dispose();
      riverMaskMap.dispose();
      splatMap.dispose();
      terrainMat.handle.dispose();
      geometry.dispose();
    };
  }, [colorMap, normalMap, roughnessMap, detailMap, riverMaskMap, splatMap, terrainMat, geometry]);

  return (
    <mesh geometry={geometry} material={terrainMat.material} name="world-terrain" receiveShadow />
  );
};

export default WorldTerrain;
