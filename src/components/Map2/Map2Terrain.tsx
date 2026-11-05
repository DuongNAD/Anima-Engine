import React, { useEffect, useMemo, useRef } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { sampleMap2Terrain } from './utils/map2TerrainGen';
import { createMap2TerrainMaterial } from './utils/map2Shaders';

export interface Map2TerrainProps {
  size?: number;
  segments?: number;
  skirtDepth?: number;
  wireframe?: boolean;
  receiveShadow?: boolean;
  castShadow?: boolean;
  sunDirection?: THREE.Vector3;
}

/**
 * Builds a watertight procedural terrain BufferGeometry with perimeter edge skirts.
 * Edge skirts extend down vertically at the 4 borders to hide LOD cracks and horizon gaps.
 */
function buildMap2TerrainGeometry(
  size = 200,
  segments = 128,
  skirtDepth = 15.0,
): THREE.BufferGeometry {
  const geometry = new THREE.BufferGeometry();
  const halfSize = size * 0.5;
  const gridCount = segments + 1;
  const topVertexCount = gridCount * gridCount;

  // 4 perimeter skirts: 4 edges * (segments + 1) * 2 vertices per quad strip
  const skirtVerticesPerEdge = gridCount * 2;
  const totalSkirtVertices = skirtVerticesPerEdge * 4;
  const totalVertices = topVertexCount + totalSkirtVertices;

  const positions = new Float32Array(totalVertices * 3);
  const normals = new Float32Array(totalVertices * 3);
  const uvs = new Float32Array(totalVertices * 2);
  const biomeWeights1 = new Float32Array(totalVertices * 3);
  const biomeWeights2 = new Float32Array(totalVertices * 3);
  const biomeWeights3 = new Float32Array(totalVertices * 2);

  // 1. Generate Top Grid Vertices
  for (let j = 0; j < gridCount; j++) {
    const z = -halfSize + (j / segments) * size;
    const v = j / segments;

    for (let i = 0; i < gridCount; i++) {
      const x = -halfSize + (i / segments) * size;
      const u = i / segments;
      const idx = j * gridCount + i;

      const sample = sampleMap2Terrain(x, z);

      // Positions
      positions[idx * 3] = x;
      positions[idx * 3 + 1] = sample.height;
      positions[idx * 3 + 2] = z;

      // Normals
      normals[idx * 3] = sample.normal.x;
      normals[idx * 3 + 1] = sample.normal.y;
      normals[idx * 3 + 2] = sample.normal.z;

      // UVs
      uvs[idx * 2] = u;
      uvs[idx * 2 + 1] = v;

      // Biome Weights
      biomeWeights1[idx * 3] = sample.biomeWeights[0];
      biomeWeights1[idx * 3 + 1] = sample.biomeWeights[1];
      biomeWeights1[idx * 3 + 2] = sample.biomeWeights[2];

      biomeWeights2[idx * 3] = sample.biomeWeights[3];
      biomeWeights2[idx * 3 + 1] = sample.biomeWeights[4];
      biomeWeights2[idx * 3 + 2] = sample.biomeWeights[5];

      biomeWeights3[idx * 2] = sample.biomeWeights[6];
      biomeWeights3[idx * 2 + 1] = sample.biomeWeights[7];
    }
  }

  // Top Grid Triangles
  const topIndicesCount = segments * segments * 6;
  const skirtIndicesCount = segments * 6 * 4;
  const indices = new Uint32Array(topIndicesCount + skirtIndicesCount);
  let indexOffset = 0;

  for (let j = 0; j < segments; j++) {
    for (let i = 0; i < segments; i++) {
      const a = j * gridCount + i;
      const b = (j + 1) * gridCount + i;
      const c = (j + 1) * gridCount + (i + 1);
      const d = j * gridCount + (i + 1);

      // Two triangles per grid cell: (a, b, d) and (b, c, d)
      indices[indexOffset++] = a;
      indices[indexOffset++] = b;
      indices[indexOffset++] = d;

      indices[indexOffset++] = b;
      indices[indexOffset++] = c;
      indices[indexOffset++] = d;
    }
  }

  // 2. Generate Skirt Geometry (South, North, West, East borders)
  let skirtVertexOffset = topVertexCount;

  const buildSkirtStrip = (getEdgeCoord: (k: number) => { x: number; z: number; u: number; v: number }) => {
    const stripStart = skirtVertexOffset;

    for (let k = 0; k < gridCount; k++) {
      const { x, z, u, v } = getEdgeCoord(k);
      const sample = sampleMap2Terrain(x, z);

      const topIdx = skirtVertexOffset++;
      const botIdx = skirtVertexOffset++;

      // Top vertex of skirt (matches terrain surface exactly)
      positions[topIdx * 3] = x;
      positions[topIdx * 3 + 1] = sample.height;
      positions[topIdx * 3 + 2] = z;

      normals[topIdx * 3] = sample.normal.x;
      normals[topIdx * 3 + 1] = sample.normal.y;
      normals[topIdx * 3 + 2] = sample.normal.z;

      uvs[topIdx * 2] = u;
      uvs[topIdx * 2 + 1] = v;

      biomeWeights1[topIdx * 3] = sample.biomeWeights[0];
      biomeWeights1[topIdx * 3 + 1] = sample.biomeWeights[1];
      biomeWeights1[topIdx * 3 + 2] = sample.biomeWeights[2];

      biomeWeights2[topIdx * 3] = sample.biomeWeights[3];
      biomeWeights2[topIdx * 3 + 1] = sample.biomeWeights[4];
      biomeWeights2[topIdx * 3 + 2] = sample.biomeWeights[5];

      biomeWeights3[topIdx * 2] = sample.biomeWeights[6];
      biomeWeights3[topIdx * 2 + 1] = sample.biomeWeights[7];

      // Bottom vertex of skirt (dropped vertically by skirtDepth)
      positions[botIdx * 3] = x;
      positions[botIdx * 3 + 1] = -skirtDepth;
      positions[botIdx * 3 + 2] = z;

      const skirtN = new THREE.Vector3(sample.normal.x, 0.6, sample.normal.z).normalize();
      normals[botIdx * 3] = skirtN.x;
      normals[botIdx * 3 + 1] = skirtN.y;
      normals[botIdx * 3 + 2] = skirtN.z;

      uvs[botIdx * 2] = u;
      uvs[botIdx * 2 + 1] = 0;

      biomeWeights1[botIdx * 3] = sample.biomeWeights[0];
      biomeWeights1[botIdx * 3 + 1] = sample.biomeWeights[1];
      biomeWeights1[botIdx * 3 + 2] = sample.biomeWeights[2];

      biomeWeights2[botIdx * 3] = sample.biomeWeights[3];
      biomeWeights2[botIdx * 3 + 1] = sample.biomeWeights[4];
      biomeWeights2[botIdx * 3 + 2] = sample.biomeWeights[5];

      biomeWeights3[botIdx * 2] = sample.biomeWeights[6];
      biomeWeights3[botIdx * 2 + 1] = sample.biomeWeights[7];
    }

    // Skirt Indices
    for (let k = 0; k < segments; k++) {
      const t1 = stripStart + k * 2;
      const b1 = stripStart + k * 2 + 1;
      const t2 = stripStart + (k + 1) * 2;
      const b2 = stripStart + (k + 1) * 2 + 1;

      indices[indexOffset++] = t1;
      indices[indexOffset++] = b1;
      indices[indexOffset++] = t2;

      indices[indexOffset++] = b1;
      indices[indexOffset++] = b2;
      indices[indexOffset++] = t2;
    }
  };

  // South Edge (z = -halfSize, x varies from -halfSize to +halfSize)
  buildSkirtStrip((k) => ({
    x: -halfSize + (k / segments) * size,
    z: -halfSize,
    u: k / segments,
    v: 0,
  }));

  // North Edge (z = +halfSize, x varies from +halfSize to -halfSize)
  buildSkirtStrip((k) => ({
    x: halfSize - (k / segments) * size,
    z: halfSize,
    u: 1.0 - k / segments,
    v: 1.0,
  }));

  // West Edge (x = -halfSize, z varies from +halfSize to -halfSize)
  buildSkirtStrip((k) => ({
    x: -halfSize,
    z: halfSize - (k / segments) * size,
    u: 0,
    v: 1.0 - k / segments,
  }));

  // East Edge (x = +halfSize, z varies from -halfSize to +halfSize)
  buildSkirtStrip((k) => ({
    x: halfSize,
    z: -halfSize + (k / segments) * size,
    u: 1.0,
    v: k / segments,
  }));

  // Attach Attributes
  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  geometry.setAttribute('normal', new THREE.BufferAttribute(normals, 3));
  geometry.setAttribute('uv', new THREE.BufferAttribute(uvs, 2));
  geometry.setAttribute('aBiomeWeights1', new THREE.BufferAttribute(biomeWeights1, 3));
  geometry.setAttribute('aBiomeWeights2', new THREE.BufferAttribute(biomeWeights2, 3));
  geometry.setAttribute('aBiomeWeights3', new THREE.BufferAttribute(biomeWeights3, 2));
  geometry.setIndex(new THREE.BufferAttribute(indices, 1));

  geometry.computeBoundingBox();
  geometry.computeBoundingSphere();

  return geometry;
}

/**
 * High-performance React Three Fiber procedural terrain mesh for Map 2.
 */
export const Map2Terrain: React.FC<Map2TerrainProps> = ({
  size = 200,
  segments = 128,
  skirtDepth = 15.0,
  wireframe = false,
  receiveShadow = false,
  castShadow = false,
  sunDirection = new THREE.Vector3(120, 180, 80).normalize(),
}) => {
  const meshRef = useRef<THREE.Mesh>(null);

  // 1. Build Geometry with Edge Skirts
  const terrainGeometry = useMemo(() => {
    return buildMap2TerrainGeometry(size, segments, skirtDepth);
  }, [size, segments, skirtDepth]);

  // 2. Create Custom Triplanar Splatting Material
  const terrainMaterial = useMemo(() => {
    const mat = createMap2TerrainMaterial({
      uSunDir: { value: sunDirection },
    });
    mat.wireframe = wireframe;
    return mat;
  }, [sunDirection, wireframe]);

  // 3. Render Loop Uniform Updates via mesh reference
  useFrame(({ clock }) => {
    const mesh = meshRef.current;
    if (mesh && mesh.material) {
      const mat = mesh.material as THREE.ShaderMaterial;
      if (mat.uniforms) {
        if (mat.uniforms.uTime) {
          mat.uniforms.uTime.value = clock.getElapsedTime();
        }
        if (mat.uniforms.uSunDir) {
          mat.uniforms.uSunDir.value.copy(sunDirection);
        }
      }
    }
  });

  // 4. Resource Disposal on Unmount
  useEffect(() => {
    return () => {
      terrainGeometry.dispose();
      terrainMaterial.dispose();
    };
  }, [terrainGeometry, terrainMaterial]);

  return (
    <mesh
      ref={meshRef}
      name="map2-procedural-terrain-mesh"
      geometry={terrainGeometry}
      material={terrainMaterial}
      receiveShadow={receiveShadow}
      castShadow={castShadow}
    />
  );
};

export default Map2Terrain;
