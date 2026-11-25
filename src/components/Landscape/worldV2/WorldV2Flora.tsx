import React, { useEffect, useMemo } from 'react';
import * as THREE from 'three';
import { makeFloraGeometry } from '../utils/floraGeometry';
import { FloraType, type World } from '../utils/worldGen';
import { sampleMeshHeight } from '../utils/worldSample';
import type { QualityTier } from '../utils/qualityTiers';
import { getQualityTierConfig } from '../utils/qualityTiers';
import { VEGETATION_BASE_SIZE } from '../utils/floraClearance';
import { FLORA_SECTORS_PER_SIDE, type WorldV2LoadedBundle } from './types';
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
  const stride = tier === 'high' ? 1 : tier === 'medium' ? 2 : 4;
  const castsShadow = getQualityTierConfig(tier).shadows && tier === 'high';

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

    let idx = 0;
    for (const sector of bundle.floraSectors) {
      for (const inst of sector.instances) {
        if (stride > 1 && idx % stride !== 0) {
          idx += 1;
          continue;
        }
        idx += 1;
        const sid = inst.species_id < SPECIES_COUNT ? inst.species_id : 7;
        const u = (sector.sectorX + inst.x_local_u16 / 65535.0) / FLORA_SECTORS_PER_SIDE;
        const v = (sector.sectorZ + inst.z_local_u16 / 65535.0) / FLORA_SECTORS_PER_SIDE;
        const wx = (u - 0.5) * renderSize;
        const wz = (v - 0.5) * renderSize;
        const wy = sampleMeshHeight(world, u, v, meshResolution) * heightUnits;
        const scale = VEGETATION_BASE_SIZE * (0.45 + (inst.scale_u8 / 255.0) * 1.15);
        const yaw = (inst.yaw_u8 / 255.0) * Math.PI * 2;
        perSpecies[sid].push({ x: wx, y: wy, z: wz, scale, yaw });
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
  }, [bundle, world, renderSize, heightUnits, meshResolution, stride]);

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
    <group name="world-v2-flora" userData={{ decorative: true }}>
      {batches.map((batch) => (
        <instancedMesh
          key={batch.speciesId}
          args={[batch.geometry, material, batch.count]}
          castShadow={castsShadow}
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
