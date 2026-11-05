import React, { useEffect, useLayoutEffect, useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import {
  Map2FloraSpecies,
  makeMap2FloraGeometry,
  isTallFlora,
} from './utils/map2FloraGeometry';
import { ALL_SPECIES, generatePlacements, type FloraPlacement } from './utils/map2FloraPlacement';

// -------------------------------------------------------------------------------------------------
// Map 2 GPU Instanced Vegetation Manager (10,000+ Instances with Vertex Shader Wind Sway)
// -------------------------------------------------------------------------------------------------

export interface Map2VegetationProps {
  size?: number; // World bounds: [-size/2, size/2]
  waterLevel?: number;
  windSpeed?: number;
  windDirection?: number; // radians
  spawnClearanceRadius?: number; // Radius around (0, 0) kept clear
  quality?: 'high' | 'low';
}

/**
 * Creates a standard material with vertex colors and compiles custom vertex shader wind sway.
 */
interface WindUniforms {
  uTime: { value: number };
  uWindSpeed: { value: number };
  uWindDirection: { value: number };
}

function createWindSwayMaterial(isTall: boolean, uniforms: WindUniforms): THREE.MeshStandardMaterial {
  const mat = new THREE.MeshStandardMaterial({
    vertexColors: true,
    roughness: isTall ? 0.85 : 0.95,
    metalness: 0.05,
    flatShading: true,
  });

  mat.onBeforeCompile = (shader) => {
    shader.uniforms.uTime = uniforms.uTime;
    shader.uniforms.uWindSpeed = uniforms.uWindSpeed;
    shader.uniforms.uWindDirection = uniforms.uWindDirection;

    // Wind Sway Formula:
    // Trees: Quadratic height sway factor -> (heightFactor^2 * 0.04) * windSpeed
    // Grass & shrubs: Linear height sway factor -> (heightFactor * 0.15) * windSpeed
    const swayCode = isTall
      ? `
      float heightFactor = max(0.0, position.y);
      float sway = sin(uTime * uWindSpeed * 2.2 + position.x * 0.35 + position.z * 0.35) * (heightFactor * heightFactor * 0.04) * uWindSpeed;
      vec2 windDirVec = vec2(cos(uWindDirection), sin(uWindDirection));
      transformed.x += windDirVec.x * sway;
      transformed.z += windDirVec.y * sway;
      `
      : `
      float heightFactor = max(0.0, position.y);
      float sway = sin(uTime * uWindSpeed * 3.8 + position.x * 0.8 + position.z * 0.8) * (heightFactor * 0.15) * uWindSpeed;
      vec2 windDirVec = vec2(cos(uWindDirection), sin(uWindDirection));
      transformed.x += windDirVec.x * sway;
      transformed.z += windDirVec.y * sway;
      `;

    shader.vertexShader = `
      uniform float uTime;
      uniform float uWindSpeed;
      uniform float uWindDirection;
      ${shader.vertexShader}
    `;

    shader.vertexShader = shader.vertexShader.replace(
      '#include <begin_vertex>',
      `#include <begin_vertex>\n${swayCode}`,
    );
  };

  return mat;
}

/**
 * Individual InstancedMesh component for a single flora species.
 */
const SingleSpeciesInstances: React.FC<{
  species: Map2FloraSpecies;
  geometry: THREE.BufferGeometry;
  material: THREE.MeshStandardMaterial;
  placements: FloraPlacement[];
  castShadow: boolean;
}> = ({ species, geometry, material, placements, castShadow }) => {
  const meshRef = useRef<THREE.InstancedMesh>(null);

  useLayoutEffect(() => {
    const mesh = meshRef.current;
    if (!mesh || placements.length === 0) return;

    const dummy = new THREE.Object3D();
    for (let i = 0; i < placements.length; i++) {
      const p = placements[i];
      dummy.position.set(p.x, p.y, p.z);
      dummy.rotation.set(p.tiltX, p.rotationY, p.tiltZ);
      dummy.scale.set(p.scale, p.scale, p.scale);
      dummy.updateMatrix();

      mesh.setMatrixAt(i, dummy.matrix);
      mesh.setColorAt(i, p.colorTint);
    }

    if (mesh.instanceMatrix) mesh.instanceMatrix.needsUpdate = true;
    if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
  }, [placements]);

  if (placements.length === 0) return null;

  return (
    <instancedMesh
      ref={meshRef}
      args={[geometry, material, placements.length]}
      name={`map2-flora-${species}`}
      castShadow={castShadow}
      receiveShadow={castShadow}
      frustumCulled={false}
    />
  );
};

export const Map2Vegetation: React.FC<Map2VegetationProps> = ({
  size = 200,
  waterLevel = 12.0,
  windSpeed = 1.0,
  windDirection = 0.785, // 45 degrees
  spawnClearanceRadius = 6.0,
  quality = 'high',
}) => {
  // One set of wind uniforms shared by every species' material, created with the materials. The
  // component drives them through a ref, from layout effects and the frame loop only: mutating a
  // value `useMemo` returned is a render-time value changing under React, which its rules forbid.
  const { resources, sharedWindUniforms } = useMemo(() => {
    const uniforms: WindUniforms = {
      uTime: { value: 0 },
      uWindSpeed: { value: 1.0 },
      uWindDirection: { value: 0.785 },
    };
    const list = ALL_SPECIES.map((species) => {
      const isTall = isTallFlora(species);
      const geometry = makeMap2FloraGeometry(species);
      const material = createWindSwayMaterial(isTall, uniforms);
      return { species, isTall, geometry, material };
    });
    return { resources: list, sharedWindUniforms: uniforms };
  }, []);

  const windUniformsRef = useRef<WindUniforms | null>(null);
  useLayoutEffect(() => {
    windUniformsRef.current = sharedWindUniforms;
  }, [sharedWindUniforms]);

  useLayoutEffect(() => {
    const uniforms = windUniformsRef.current;
    if (!uniforms) return;
    uniforms.uWindSpeed.value = windSpeed;
    uniforms.uWindDirection.value = windDirection;
  }, [windSpeed, windDirection]);

  useFrame((state) => {
    const uniforms = windUniformsRef.current;
    if (uniforms) uniforms.uTime.value = state.clock.getElapsedTime();
  });

  // Clean disposal on unmount
  useEffect(() => {
    return () => {
      resources.forEach((r) => {
        r.geometry.dispose();
        r.material.dispose();
      });
    };
  }, [resources]);

  // Generate balanced flora placements procedurally adhering to natural ecological carrying capacity
  const placementsBySpecies = useMemo(() => {
    const { bySpecies } = generatePlacements(size, waterLevel, spawnClearanceRadius, quality);
    return bySpecies;
  }, [size, waterLevel, spawnClearanceRadius, quality]);

  return (
    <group name="map2-vegetation-root">
      {resources.map((r) => {
        const placements = placementsBySpecies.get(r.species) || [];
        return (
          <SingleSpeciesInstances
            key={r.species}
            species={r.species}
            geometry={r.geometry}
            material={r.material}
            placements={placements}
            castShadow={r.isTall && quality !== 'low'}
          />
        );
      })}
    </group>
  );
};

export default Map2Vegetation;

