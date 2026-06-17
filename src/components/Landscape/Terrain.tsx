import React, { useMemo, useRef, useLayoutEffect } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { generateTerrain, generateTerrainData, getBiomeColor } from './utils/terrainGenerator';

declare global {
  namespace JSX {
    interface IntrinsicElements {
      lod: any;
    }
  }
}

interface TerrainProps {
  width?: number;
  height?: number;
  wetnessRatio?: number;
}

function getBlendedBiomeColor(elevation: number, moisture: number): { r: number; g: number; b: number } {
  let r = 0, g = 0, b = 0;
  let count = 0;
  // Sample a small 3x3 grid around the coordinate to smooth boundaries
  const steps = [-2, 0, 2];
  for (const de of steps) {
    for (const dm of steps) {
      const sampleE = Math.max(0, Math.min(100, elevation + de));
      const sampleM = Math.max(0, Math.min(100, moisture + dm));
      const col = getBiomeColor(sampleE, sampleM);
      r += col.r;
      g += col.g;
      b += col.b;
      count++;
    }
  }
  return { r: r / count, g: g / count, b: b / count };
}

export const Terrain: React.FC<TerrainProps> = ({ width = 500, height = 500, wetnessRatio = 0 }) => {
  // Generate the basic terrain data
  const terrain = useMemo(() => generateTerrain(width, height, 'seed'), [width, height]);
  const terrainData = useMemo(() => generateTerrainData(width, height), [width, height]);

  const lodRef = useRef<THREE.LOD>(null);
  const meshRef0 = useRef<THREE.Mesh>(null);
  const meshRef1 = useRef<THREE.Mesh>(null);
  const meshRef2 = useRef<THREE.Mesh>(null);

  const geomRef0 = useRef<THREE.BufferGeometry>(null);
  const geomRef1 = useRef<THREE.BufferGeometry>(null);
  const geomRef2 = useRef<THREE.BufferGeometry>(null);

  // Helper to construct geometry arrays for a given LOD step size
  const getGeometryData = (step: number) => {
    const w = Math.ceil(width / step);
    const h = Math.ceil(height / step);

    const positions = new Float32Array(w * h * 3);
    const colors = new Float32Array(w * h * 3);
    const indices: number[] = [];

    for (let yIndex = 0; yIndex < h; yIndex++) {
      for (let xIndex = 0; xIndex < w; xIndex++) {
        const gx = Math.min(width - 1, xIndex * step);
        const gy = Math.min(height - 1, yIndex * step);

        const cell = terrain.grid[gy][gx];

        const posX = gx - width / 2;
        const posY = gy - height / 2;
        const posZ = cell.elevation * 1.8; // Elevation deformation

        const i = yIndex * w + xIndex;
        positions[i * 3] = posX;
        positions[i * 3 + 1] = posZ;
        positions[i * 3 + 2] = posY;

        let color;
        if (cell.isRiver === 1) {
          color = { r: 0x22 / 255, g: 0x94 / 255, b: 0xa8 / 255 };
        } else if (cell.isRiver === 2 || cell.isWaterfall) {
          color = { r: 0x88 / 255, g: 0xdd / 255, b: 0xee / 255 };
        } else if (cell.isRiver === 3) {
          color = { r: 0x1a / 255, g: 0x7a / 255, b: 0x90 / 255 };
        } else {
          color = getBlendedBiomeColor(cell.elevation, cell.moisture);
        }
        colors[i * 3] = color.r;
        colors[i * 3 + 1] = color.g;
        colors[i * 3 + 2] = color.b;
      }
    }

    for (let yIndex = 0; yIndex < h - 1; yIndex++) {
      for (let xIndex = 0; xIndex < w - 1; xIndex++) {
        const a = yIndex * w + xIndex;
        const b = yIndex * w + (xIndex + 1);
        const c = (yIndex + 1) * w + xIndex;
        const d = (yIndex + 1) * w + (xIndex + 1);

        indices.push(a, b, c);
        indices.push(b, d, c);
      }
    }

    return { positions, colors, indices };
  };

  useLayoutEffect(() => {
    // Initialize geometry for LOD 0 (High Detail - step 1)
    if (geomRef0.current && typeof (geomRef0.current as any).setIndex === 'function') {
      const { positions, colors, indices } = getGeometryData(1);
      geomRef0.current.setAttribute('position', new THREE.BufferAttribute(positions, 3));
      geomRef0.current.setAttribute('color', new THREE.BufferAttribute(colors, 3));
      geomRef0.current.setIndex(new THREE.BufferAttribute(new Uint32Array(indices), 1));
      geomRef0.current.computeVertexNormals();
    }

    // Initialize geometry for LOD 1 (Medium Detail - step 4)
    if (geomRef1.current && typeof (geomRef1.current as any).setIndex === 'function') {
      const { positions, colors, indices } = getGeometryData(4);
      geomRef1.current.setAttribute('position', new THREE.BufferAttribute(positions, 3));
      geomRef1.current.setAttribute('color', new THREE.BufferAttribute(colors, 3));
      geomRef1.current.setIndex(new THREE.BufferAttribute(new Uint32Array(indices), 1));
      geomRef1.current.computeVertexNormals();
    }

    // Initialize geometry for LOD 2 (Low Detail - step 16)
    if (geomRef2.current && typeof (geomRef2.current as any).setIndex === 'function') {
      const { positions, colors, indices } = getGeometryData(16);
      geomRef2.current.setAttribute('position', new THREE.BufferAttribute(positions, 3));
      geomRef2.current.setAttribute('color', new THREE.BufferAttribute(colors, 3));
      geomRef2.current.setIndex(new THREE.BufferAttribute(new Uint32Array(indices), 1));
      geomRef2.current.computeVertexNormals();
    }
  }, [width, height, wetnessRatio]);

  useLayoutEffect(() => {
    if (lodRef.current && typeof (lodRef.current as any).addLevel === 'function' && meshRef0.current && meshRef1.current && meshRef2.current) {
      // Clear levels to support clean re-mounting/updates
      (lodRef.current as any).levels = [];
      
      // Setup LOD thresholds
      (lodRef.current as any).addLevel(meshRef0.current, 0);      // High detail close up
      (lodRef.current as any).addLevel(meshRef1.current, 150);    // Medium detail mid range
      (lodRef.current as any).addLevel(meshRef2.current, 400);    // Low detail far away
    }
  }, []);

  useFrame((state) => {
    const time = state.clock.getElapsedTime();
    const animateLOD = (geomRef: React.RefObject<THREE.BufferGeometry>, step: number) => {
      if (!geomRef.current) return;
      const geom = geomRef.current;
      const posAttr = geom.getAttribute('position') as THREE.BufferAttribute;
      if (!posAttr) return;
      const pos = posAttr.array as Float32Array;
      const w = Math.ceil(width / step);
      const h = Math.ceil(height / step);

      let needsUpdate = false;
      for (let yIndex = 0; yIndex < h; yIndex++) {
        for (let xIndex = 0; xIndex < w; xIndex++) {
          const gx = Math.min(width - 1, xIndex * step);
          const gy = Math.min(height - 1, yIndex * step);
          const cell = terrain.grid[gy][gx];

          if (cell.elevation < 3.0 && !cell.isRiver) {
            const i = yIndex * w + xIndex;
            const x = gx;
            const y = gy;
            const wave = Math.sin(x * 0.15 + time * 1.5) * Math.cos(y * 0.15 + time) * 0.3;
            pos[i * 3 + 1] = (cell.elevation * 1.8) + wave;
            needsUpdate = true;
          }
        }
      }
      if (needsUpdate) {
        posAttr.needsUpdate = true;
        geom.computeVertexNormals();
      }
    };

    animateLOD(geomRef0, 1);
    animateLOD(geomRef1, 4);
    animateLOD(geomRef2, 16);
  });

  return (
    <lod ref={lodRef} name="terrain-lod">
      {/* LOD Level 0: High Detail */}
      <mesh
        ref={meshRef0}
        name="terrain-mesh"
        userData={{ wetnessRatio, gridLength: terrainData.length }}
        data-wetness-ratio={wetnessRatio}
      >
        <bufferGeometry ref={geomRef0} />
        <meshStandardMaterial vertexColors roughness={1.0 - wetnessRatio} metalness={0.1} />
      </mesh>

      {/* LOD Level 1: Medium Detail */}
      <mesh ref={meshRef1} name="terrain-mesh-lod-1">
        <bufferGeometry ref={geomRef1} />
        <meshStandardMaterial vertexColors roughness={1.0 - wetnessRatio} metalness={0.1} />
      </mesh>

      {/* LOD Level 2: Low Detail */}
      <mesh ref={meshRef2} name="terrain-mesh-lod-2">
        <bufferGeometry ref={geomRef2} />
        <meshStandardMaterial vertexColors roughness={1.0 - wetnessRatio} metalness={0.1} />
      </mesh>
    </lod>
  );
};

export default Terrain;
