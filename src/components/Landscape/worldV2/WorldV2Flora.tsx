import React, { useEffect, useMemo } from 'react';
import * as THREE from 'three';
import { makeFloraGeometry } from '../utils/floraGeometry';
import { FloraType, type World } from '../utils/worldGen';
import { sampleMeshHeight } from '../utils/worldSample';
import type { QualityTier } from '../utils/qualityTiers';
import { getQualityTierConfig } from '../utils/qualityTiers';
import { VEGETATION_BASE_SIZE } from '../utils/floraClearance';
import { debugFloraDensityScale } from '../utils/diagnosticsWindow';
import { CASTS_SHADOW } from '../utils/floraChunks';
import { resolveWorldV2FloraStride, type WorldV2LoadedBundle } from './types';
import { WV2_SPECIES_TO_FLORA } from './worldV2Adapter';

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
  floraType: FloraType;
  geometry: THREE.BufferGeometry;
  matrices: Float32Array;
  count: number;
}

const SPECIES_COUNT = 8;

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
  const totalInstances = useMemo(() => {
    let n = 0;
    for (const sector of bundle.floraSectors) n += sector.instances.length;
    return n;
  }, [bundle.floraSectors]);
  const effectiveStride = resolveWorldV2FloraStride(tier, totalInstances);
  // Test-only (RW-C4 negative control); `1` in every production build.
  const densityScale = useMemo(() => debugFloraDensityScale(), []);
  const shadowsEnabled = getQualityTierConfig(tier).shadows && tier === 'high';

  const material = useMemo(
    () =>
      new THREE.MeshStandardMaterial({
        vertexColors: true,
        roughness: 0.88,
        metalness: 0.0,
      }),
    [],
  );

  const batches = useMemo<SpeciesBatch[]>(() => {
    const perSpecies: Array<Array<{ x: number; y: number; z: number; scale: number; yaw: number }>> =
      Array.from({ length: SPECIES_COUNT }, () => []);

    const rate = densityScale / effectiveStride;
    let idx = 0;
    for (const sector of bundle.floraSectors) {
      for (const inst of sector.instances) {
        const copies =
          densityScale === 1
            ? idx % effectiveStride === 0
              ? 1
              : 0
            : Math.ceil((idx + 1) * rate - 1e-9) - Math.ceil(idx * rate - 1e-9);
        idx += 1;
        if (copies <= 0) continue;
        const sid = inst.species_id < SPECIES_COUNT ? inst.species_id : 7;
        const u = (sector.sectorX + inst.x_local_u16 / 65535.0) / bundle.floraSectorsPerSide;
        const v = (sector.sectorZ + inst.z_local_u16 / 65535.0) / bundle.floraSectorsPerSide;
        const wx = (u - 0.5) * renderSize;
        const wz = (v - 0.5) * renderSize;
        const wy = sampleMeshHeight(world, u, v, meshResolution) * heightUnits;
        const scale = VEGETATION_BASE_SIZE * (0.45 + (inst.scale_u8 / 255.0) * 1.15);
        const yaw = (inst.yaw_u8 / 255.0) * Math.PI * 2;
        for (let c = 0; c < copies; c += 1) {
          perSpecies[sid].push({ x: wx + c * 0.15, y: wy, z: wz + c * 0.15, scale, yaw });
        }
      }
    }

    const dummy = new THREE.Object3D();
    const out: SpeciesBatch[] = [];
    for (let sid = 0; sid < SPECIES_COUNT; sid += 1) {
      const list = perSpecies[sid];
      if (list.length === 0) continue;
      const floraType = WV2_SPECIES_TO_FLORA[sid] ?? FloraType.Bush;
      const geometry = makeFloraGeometry(floraType);
      const matrices = new Float32Array(list.length * 16);
      for (let i = 0; i < list.length; i += 1) {
        const item = list[i];
        dummy.position.set(item.x, item.y, item.z);
        dummy.rotation.set(0, item.yaw, 0);
        dummy.scale.setScalar(item.scale);
        dummy.updateMatrix();
        matrices.set(dummy.matrix.elements, i * 16);
      }
      out.push({
        speciesId: sid,
        floraType,
        geometry,
        matrices,
        count: list.length,
      });
    }
    return out;
  }, [bundle, world, renderSize, heightUnits, meshResolution, effectiveStride, densityScale]);

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
          key={`${batch.speciesId}-${batch.count}`}
          args={[batch.geometry, material, batch.count]}
          castShadow={shadowsEnabled && CASTS_SHADOW.has(batch.floraType)}
          receiveShadow={false}
          name={`world-v2-flora-s${batch.speciesId}`}
          ref={(mesh: THREE.InstancedMesh | null) => {
            if (!mesh || !mesh.instanceMatrix) return;
            mesh.instanceMatrix.array.set(batch.matrices);
            mesh.instanceMatrix.needsUpdate = true;
            mesh.computeBoundingSphere();
          }}
        />
      ))}
    </group>
  );
};

export default WorldV2Flora;
