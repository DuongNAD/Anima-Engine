import React, { useEffect, useMemo, useRef } from 'react';
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
import { getCachedCarvedCoarseHeights } from './worldV2/channelCarve';
import { L0HeightTileCache } from './worldV2/l0HeightCache';
import {
  buildNearPatchBuffers,
  buildNearPatchIndices,
  clearActiveNearPatchState,
  setActiveNearPatchState,
  updateCoarseCutoutIndices,
} from './worldV2/terrainNearPatch';

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
function markTextureUpdated(tex: THREE.DataTexture): void {
  tex.needsUpdate = true;
}

export const WorldTerrain: React.FC<WorldTerrainProps> = ({
  world,
  worldV2 = null,
  sunDir = [0.6, 0.7, 0.35],
  renderSize = 400,
  heightRatio = 0.13,
  meshResolution = 256,
  tier = 'high',
}) => {
  const heightUnits = renderSize * heightRatio;
  const nearMeshRef = useRef<THREE.Mesh>(null);
  const enableNearPatch = Boolean(worldV2) && tier !== 'low';

  const coarseBuffers = useMemo(() => {
    const res = meshResolution;
    const verts = (res + 1) * (res + 1);
    const positions = new Float32Array(verts * 3);
    const uvs = new Float32Array(verts * 2);

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

    if (worldV2) {
      const carvedY = getCachedCarvedCoarseHeights(
        world,
        worldV2,
        res,
        renderSize,
        heightUnits,
        tier !== 'low',
      );
      for (let i = 0; i < verts; i += 1) {
        positions[i * 3 + 1] = carvedY[i];
      }
    }

    const fullIndices = new Uint32Array(res * res * 6);
    const cutoutIndices = new Uint32Array(res * res * 6);
    let o = 0;
    for (let gy = 0; gy < res; gy++) {
      for (let gx = 0; gx < res; gx++) {
        const a = gy * (res + 1) + gx;
        const b = a + 1;
        const c = a + (res + 1);
        const d = c + 1;
        // CCW from above so computeVertexNormals() yields +Y normals (no back-face culling).
        fullIndices[o++] = a;
        fullIndices[o++] = c;
        fullIndices[o++] = b;
        fullIndices[o++] = b;
        fullIndices[o++] = c;
        fullIndices[o++] = d;
      }
    }

    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geom.setAttribute('uv', new THREE.BufferAttribute(uvs, 2));
    geom.setIndex(new THREE.BufferAttribute(fullIndices, 1));
    geom.computeVertexNormals();
    const normals = geom.getAttribute('normal').array as Float32Array;
    return {
      geometry: geom,
      positions,
      normals,
      fullIndices,
      cutoutIndices,
      fullIndexCount: fullIndices.length,
    };
  }, [world, worldV2, renderSize, heightUnits, meshResolution, tier]);

  const geometry = coarseBuffers.geometry;

  const l0Cache = useMemo(
    () => (enableNearPatch ? new L0HeightTileCache(worldV2?.l0HeightManifest) : null),
    [enableNearPatch, worldV2],
  );

  const nearPatchResources = useMemo(() => {
    if (!enableNearPatch) return null;
    const coarseSpan = Math.min(meshResolution, tier === 'high' ? 20 : 16);
    const subDiv = tier === 'high' ? 8 : 6;
    const patchRes = coarseSpan * subDiv;
    const { indices, vertCount } = buildNearPatchIndices(patchRes);
    const positions = new Float32Array(vertCount * 3);
    const uvs = new Float32Array(vertCount * 2);
    const normals = new Float32Array(vertCount * 3);
    const texData = new Float32Array((patchRes + 1) * (patchRes + 1) * 4);

    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geom.setAttribute('uv', new THREE.BufferAttribute(uvs, 2));
    geom.setAttribute('normal', new THREE.BufferAttribute(normals, 3));
    geom.setIndex(new THREE.BufferAttribute(indices, 1));

    const tex = new THREE.DataTexture(
      texData,
      patchRes + 1,
      patchRes + 1,
      THREE.RGBAFormat,
      THREE.FloatType,
    );
    tex.minFilter = THREE.NearestFilter;
    tex.magFilter = THREE.NearestFilter;
    tex.wrapS = THREE.ClampToEdgeWrapping;
    tex.wrapT = THREE.ClampToEdgeWrapping;
    tex.needsUpdate = true;

    return {
      coarseSpan,
      subDiv,
      patchRes,
      geom,
      positions,
      uvs,
      normals,
      tex,
      texData,
    };
  }, [enableNearPatch, meshResolution, tier]);

  const nearStateRef = useRef({ gx0: -1, gz0: -1, l0Version: -1, holeActive: false });

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
        hasRiverMesh: Boolean(worldV2) && tier !== 'low',
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

    const nearMesh = nearMeshRef.current;
    if (!nearPatchResources || !nearMesh || !worldV2 || !l0Cache) return;

    const cam = state.camera.position;
    const cu = cam.x / renderSize + 0.5;
    const cv = cam.z / renderSize + 0.5;
    const groundY = cu >= 0 && cu <= 1 && cv >= 0 && cv <= 1 ? sampleElevation(world, cu, cv) * heightUnits : 0;
    const alt = cam.y - groundY;
    const nearState = nearStateRef.current;
    const idxAttr = geometry.getIndex() as THREE.BufferAttribute;

    // Disable 20 m detail patch when high overhead (overview/horizon/peak/vista)
    if (alt > 45 || cu < -0.1 || cu > 1.1 || cv < -0.1 || cv > 1.1) {
      if (nearMesh.visible || nearState.holeActive) {
        nearMesh.visible = false;
        if (nearState.holeActive) {
          (idxAttr.array as Uint32Array).set(coarseBuffers.fullIndices);
          idxAttr.needsUpdate = true;
          geometry.setDrawRange(0, coarseBuffers.fullIndexCount);
          nearState.holeActive = false;
        }
        clearActiveNearPatchState();
      }
      return;
    }

    const { coarseSpan, subDiv, patchRes, geom, positions, uvs, normals, tex, texData } =
      nearPatchResources;
    const camGx = Math.round(cu * meshResolution);
    const camGz = Math.round(cv * meshResolution);
    const gx0 = Math.max(0, Math.min(meshResolution - coarseSpan, camGx - (coarseSpan >> 1)));
    const gz0 = Math.max(0, Math.min(meshResolution - coarseSpan, camGz - (coarseSpan >> 1)));
    const gx1 = gx0 + coarseSpan;
    const gz1 = gz0 + coarseSpan;
    const u0 = gx0 / meshResolution;
    const v0 = gz0 / meshResolution;
    const u1 = gx1 / meshResolution;
    const v1 = gz1 / meshResolution;

    l0Cache.requestRegion(u0, v0, u1, v1);
    const l0Ready = l0Cache.isRegionReady(u0, v0, u1, v1);

    if (
      nearState.gx0 !== gx0 ||
      nearState.gz0 !== gz0 ||
      nearState.l0Version !== l0Cache.version ||
      !nearMesh.visible
    ) {
      buildNearPatchBuffers({
        gx0,
        gz0,
        coarseSpan,
        subDiv,
        meshRes: meshResolution,
        renderSize,
        heightUnits,
        world,
        bundle: worldV2,
        coarsePositions: coarseBuffers.positions,
        coarseNormals: coarseBuffers.normals,
        l0Cache,
        outPositions: positions,
        outUvs: uvs,
        outNormals: normals,
        outTexData: texData,
      });

      (geom.getAttribute('position') as THREE.BufferAttribute).needsUpdate = true;
      (geom.getAttribute('uv') as THREE.BufferAttribute).needsUpdate = true;
      (geom.getAttribute('normal') as THREE.BufferAttribute).needsUpdate = true;
      geom.computeBoundingBox();
      geom.computeBoundingSphere();
      markTextureUpdated(tex);

      const activeIdxCount = updateCoarseCutoutIndices(
        coarseBuffers.cutoutIndices,
        meshResolution,
        gx0,
        gz0,
        gx1,
        gz1,
      );
      (idxAttr.array as Uint32Array).set(coarseBuffers.cutoutIndices);
      idxAttr.needsUpdate = true;
      geometry.setDrawRange(0, activeIdxCount);

      const minX = (u0 - 0.5) * renderSize;
      const minZ = (v0 - 0.5) * renderSize;
      const maxX = (u1 - 0.5) * renderSize;
      const maxZ = (v1 - 0.5) * renderSize;
      setActiveNearPatchState(tex, [minX, minZ, maxX, maxZ], patchRes, texData);

      nearMesh.visible = true;
      nearMesh.userData = {
        gx0,
        gz0,
        gx1,
        gz1,
        coarseSpan,
        subDiv,
        patchRes,
        minX,
        minZ,
        maxX,
        maxZ,
        l0Ready,
      };
      nearState.gx0 = gx0;
      nearState.gz0 = gz0;
      nearState.l0Version = l0Cache.version;
      nearState.holeActive = true;
    } else if (nearMesh.userData) {
      nearMesh.userData.l0Ready = l0Ready;
    }
  });

  useEffect(() => {
    return () => {
      clearActiveNearPatchState();
      nearPatchResources?.geom.dispose();
      nearPatchResources?.tex.dispose();
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
  }, [textures, terrainMat, geometry, nearPatchResources]);

  return (
    <group name="world-terrain-root">
      <mesh geometry={geometry} material={terrainMat.material} name="world-terrain" receiveShadow />
      {nearPatchResources ? (
        <mesh
          ref={nearMeshRef}
          geometry={nearPatchResources.geom}
          material={terrainMat.material}
          name="world-terrain-near"
          visible={false}
          receiveShadow
        />
      ) : null}
    </group>
  );
};

export default WorldTerrain;
