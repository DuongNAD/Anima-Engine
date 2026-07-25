import React, { useEffect, useMemo } from 'react';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { buildCavesGeometry } from './utils/caveGeometry';

// ---------------------------------------------------------------------------------------
// WorldCaves — real 3D cave mouths on steep bare-rock faces (World.cave*). Each mouth is a
// rocky, noise-displaced funnel that flares open at the cliff and tapers back to a dark
// pocket (see caveGeometry.ts), all merged into ONE lit mesh / one draw call. Vertex colours
// carry the interior darkness so the hollow reads as a cave whatever the sun angle.
// ---------------------------------------------------------------------------------------

export interface WorldCavesProps {
  world: World;
  renderSize?: number;
  heightRatio?: number;
  meshResolution?: number;
}

export const WorldCaves: React.FC<WorldCavesProps> = ({
  world,
  renderSize = 400,
  heightRatio = 0.13,
  meshResolution = 384,
}) => {
  const heightUnits = renderSize * heightRatio;

  const geom = useMemo(
    () => buildCavesGeometry(world, renderSize, heightUnits, meshResolution),
    [world, renderSize, heightUnits, meshResolution],
  );

  // Dispose the merged geometry when the world (and thus the geometry) changes or unmounts.
  useEffect(() => {
    return () => {
      if (geom && typeof geom.dispose === 'function') geom.dispose();
    };
  }, [geom]);

  if (!geom) return null;
  return (
    <mesh geometry={geom} name="world-caves" frustumCulled={false}>
      <meshStandardMaterial vertexColors roughness={1} metalness={0} side={THREE.DoubleSide} />
    </mesh>
  );
};

export default WorldCaves;
