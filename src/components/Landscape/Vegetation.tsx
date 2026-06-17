import React, { useMemo, useRef, useLayoutEffect, useEffect } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { generateFloraPlacements, generateTerrain, mulberry32, getBilinearInterpolatedElevation } from './utils/terrainGenerator';

interface VegetationProps {
  width?: number;
  height?: number;
  windSpeed?: number;
  windAngle?: number;     // Existing/legacy prop name
  windDirection?: number; // Prop required by Milestone 4
  densityFactor?: number;
  maxCapacity?: number;
}

// Helper to merge two buffer geometries with distinct vertex colors
function mergeGeometries(
  geo1: THREE.BufferGeometry,
  color1: THREE.Color,
  geo2: THREE.BufferGeometry,
  color2: THREE.Color
) {
  const g1 = geo1.index !== null ? geo1.toNonIndexed() : geo1.clone();
  const g2 = geo2.index !== null ? geo2.toNonIndexed() : geo2.clone();

  const pos1 = g1.attributes.position.array as Float32Array;
  const pos2 = g2.attributes.position.array as Float32Array;

  const norm1 = g1.attributes.normal.array as Float32Array;
  const norm2 = g2.attributes.normal.array as Float32Array;

  const totalVertices = (pos1.length + pos2.length) / 3;

  const positions = new Float32Array(totalVertices * 3);
  positions.set(pos1, 0);
  positions.set(pos2, pos1.length);

  const normals = new Float32Array(totalVertices * 3);
  normals.set(norm1, 0);
  normals.set(norm2, norm1.length);

  const colors = new Float32Array(totalVertices * 3);
  for (let i = 0; i < pos1.length / 3; i++) {
    colors[i * 3] = color1.r;
    colors[i * 3 + 1] = color1.g;
    colors[i * 3 + 2] = color1.b;
  }
  const offset = pos1.length;
  for (let i = 0; i < pos2.length / 3; i++) {
    colors[offset + i * 3] = color2.r;
    colors[offset + i * 3 + 1] = color2.g;
    colors[offset + i * 3 + 2] = color2.b;
  }

  const merged = new THREE.BufferGeometry();
  merged.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  merged.setAttribute('normal', new THREE.BufferAttribute(normals, 3));
  merged.setAttribute('color', new THREE.BufferAttribute(colors, 3));

  g1.dispose();
  g2.dispose();

  return merged;
}

export const Vegetation: React.FC<VegetationProps> = ({
  width = 64,
  height = 64,
  windSpeed = 1.0,
  windAngle = 0.0,
  windDirection,
  densityFactor = 1.0,
  maxCapacity = 1000,
}) => {
  // Use windDirection if defined, fallback to windAngle
  const currentWindDirection = windDirection !== undefined ? windDirection : windAngle;

  const terrain = useMemo(() => generateTerrain(width, height, 'seed'), [width, height]);
  const allPlacements = useMemo(() => generateFloraPlacements(width, height), [width, height]);

  // Apply density factor and max capacity to main placements
  let placements = useMemo(() => {
    let list = allPlacements.filter((_, idx) => {
      return (idx / (allPlacements.length || 1)) < densityFactor;
    });

    if (list.length > maxCapacity) {
      list = list.slice(0, maxCapacity);
    }
    return list;
  }, [allPlacements, densityFactor, maxCapacity]);

  // Overlap prevention distance filter
  const filteredPlacements = useMemo(() => {
    const list: typeof placements = [];
    const minDistanceSq = 1.0;
    for (const p of placements) {
      let tooClose = false;
      for (const fp of list) {
        const dx = p.x - fp.x;
        const dy = p.y - fp.y;
        if (dx * dx + dy * dy < minDistanceSq) {
          tooClose = true;
          break;
        }
      }
      if (!tooClose) {
        // Double check biome rules just in case
        const cellY = Math.min(height - 1, Math.max(0, Math.floor(p.y)));
        const cellX = Math.min(width - 1, Math.max(0, Math.floor(p.x)));
        const cell = terrain.grid[cellY][cellX];
        const isWet = cell.elevation < 20 || cell.isLake || cell.isRiver || cell.isWaterfall;
        const isSnow = cell.biome === 'snow peaks' || cell.elevation >= 80;
        
        if (!isWet && !isSnow) {
          list.push(p);
        }
      }
    }
    return list;
  }, [placements, terrain, width, height]);

  // Seeded generation of grass patches
  const grassPlacements = useMemo(() => {
    const list: { x: number; y: number; scale: number }[] = [];
    // Ensure density factor filters grass too
    if (densityFactor <= 0) return list;

    const random = mulberry32(54321); // unique seed for grass
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const cell = terrain.grid[y][x];
        const isWet = cell.elevation < 20 || cell.isLake || cell.isRiver || cell.isWaterfall;
        const isSnow = cell.biome === 'snow peaks' || cell.elevation >= 80;
        if (isWet || isSnow) continue;

        let grassDensity = 0.7;
        if (cell.biome === 'grassland') {
          grassDensity = 0.7;
        } else if (cell.biome === 'forest') {
          grassDensity = 0.4;
        } else if (cell.biome === 'beach') {
          grassDensity = 0.05;
        }

        // Apply densityFactor
        grassDensity *= densityFactor;

        if (random() < grassDensity) {
          const numPatches = Math.floor(random() * 2) + 1;
          for (let i = 0; i < numPatches; i++) {
            const offsetX = random();
            const offsetY = random();
            const scale = 0.3 + random() * 0.4;
            list.push({
              x: x + offsetX,
              y: y + offsetY,
              scale,
            });
          }
        }
      }
    }
    
    // Cap grass elements to max capacity as well
    if (list.length > maxCapacity) {
      return list.slice(0, maxCapacity);
    }
    return list;
  }, [terrain, width, height, densityFactor, maxCapacity]);

  // Group placements by species
  const oakPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Oak'), [filteredPlacements]);
  const pinePlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Pine'), [filteredPlacements]);
  const bushPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Bush'), [filteredPlacements]);
  const rockPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Rock'), [filteredPlacements]);
  const palmPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Palm'), [filteredPlacements]);
  const cactusPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Cactus'), [filteredPlacements]);
  const junglePlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Jungle'), [filteredPlacements]);
  const birchPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Birch'), [filteredPlacements]);
  const flowersPlacements = useMemo(() => filteredPlacements.filter(p => p.type === 'Flowers'), [filteredPlacements]);

  const speciesCounts = useMemo(() => ({
    Oak: oakPlacements.length,
    Pine: pinePlacements.length,
    Bush: bushPlacements.length,
    Rock: rockPlacements.length,
    Palm: palmPlacements.length,
    Cactus: cactusPlacements.length,
    Jungle: junglePlacements.length,
    Birch: birchPlacements.length,
    Flowers: flowersPlacements.length,
    Grass: grassPlacements.length,
  }), [
    oakPlacements, pinePlacements, bushPlacements, rockPlacements,
    palmPlacements, cactusPlacements, junglePlacements, birchPlacements,
    flowersPlacements, grassPlacements
  ]);

  // 3D Low-poly models
  const geometries = useMemo(() => {
    const oakTrunk = new THREE.CylinderGeometry(0.35, 0.5, 2.5, 5);
    const oakLeaves = new THREE.DodecahedronGeometry(2.2, 1);
    const pineTrunk = new THREE.CylinderGeometry(0.25, 0.4, 2, 5);
    const pineLeaves1 = new THREE.ConeGeometry(2, 3.5, 5);
    const pineLeaves2 = new THREE.ConeGeometry(1.5, 2.8, 5);
    const pineLeaves3 = new THREE.ConeGeometry(0.9, 2, 5);
    const bushGeo = new THREE.DodecahedronGeometry(1.2, 0);
    const rockGeo = new THREE.DodecahedronGeometry(1.5, 0);
    const palmTrunk = new THREE.CylinderGeometry(0.2, 0.3, 5, 5);
    const palmLeaves = new THREE.DodecahedronGeometry(2.5, 0);
    const cactusGeo = new THREE.CylinderGeometry(0.3, 0.35, 2.5, 6);
    const jungleTrunk = new THREE.CylinderGeometry(0.4, 0.6, 4, 5);
    const jungleLeaves = new THREE.DodecahedronGeometry(3, 1);
    const birchTrunk = new THREE.CylinderGeometry(0.2, 0.3, 3, 5);
    const birchLeaves = new THREE.DodecahedronGeometry(1.8, 1);
    const flowersGeo = new THREE.SphereGeometry(0.4, 5, 5);

    // Grass (Intersecting Cross-Planes)
    const p1 = new THREE.PlaneGeometry(0.5, 0.5);
    p1.translate(0, 0.25, 0);
    const p2 = p1.clone();
    p2.rotateY(Math.PI / 2);
    const grassGeo = mergeGeometries(
      p1,
      new THREE.Color('#4ade80'),
      p2,
      new THREE.Color('#4ade80')
    );

    p1.dispose();
    p2.dispose();

    return {
      OakTrunk: oakTrunk,
      OakLeaves: oakLeaves,
      PineTrunk: pineTrunk,
      PineLeaves1: pineLeaves1,
      PineLeaves2: pineLeaves2,
      PineLeaves3: pineLeaves3,
      Bush: bushGeo,
      Rock: rockGeo,
      PalmTrunk: palmTrunk,
      PalmLeaves: palmLeaves,
      Cactus: cactusGeo,
      JungleTrunk: jungleTrunk,
      JungleLeaves: jungleLeaves,
      BirchTrunk: birchTrunk,
      BirchLeaves: birchLeaves,
      Flowers: flowersGeo,
      Grass: grassGeo,
    };
  }, []);

  // Shared Wind uniforms
  const uniforms = useMemo(() => ({
    uTime: { value: 0 },
    uWindSpeed: { value: windSpeed },
    uWindDirection: { value: currentWindDirection },
    time: { value: 0 },
    windSpeed: { value: windSpeed },
    windAngle: { value: currentWindDirection },
  }), []);

  // Sync prop changes reactively
  useEffect(() => {
    uniforms.uWindSpeed.value = windSpeed;
    uniforms.windSpeed.value = windSpeed;
  }, [windSpeed, uniforms]);

  useEffect(() => {
    uniforms.uWindDirection.value = currentWindDirection;
    uniforms.windAngle.value = currentWindDirection;
  }, [currentWindDirection, uniforms]);

  // Update clock time on frame ticks
  useFrame((state) => {
    const elapsed = state.clock.getElapsedTime();
    if (uniforms) {
      if (uniforms.uTime) uniforms.uTime.value = elapsed;
      if (uniforms.time) uniforms.time.value = elapsed;
    }
    // Update all materials' uniforms if they exist on the material
    Object.values(materials).forEach((mat) => {
      if (mat.userData.uniforms) {
        if (mat.userData.uniforms.uTime) mat.userData.uniforms.uTime.value = elapsed;
        if (mat.userData.uniforms.time) mat.userData.uniforms.time.value = elapsed;
        if (mat.userData.uniforms.uWindSpeed) mat.userData.uniforms.uWindSpeed.value = windSpeed;
        if (mat.userData.uniforms.windSpeed) mat.userData.uniforms.windSpeed.value = windSpeed;
        if (mat.userData.uniforms.uWindDirection) mat.userData.uniforms.uWindDirection.value = currentWindDirection;
        if (mat.userData.uniforms.windAngle) mat.userData.uniforms.windAngle.value = currentWindDirection;
      }
    });
  });

  // Shader customization function to inject wind sway warp
  const customizeWindSwayShader = (shader: any) => {
    shader.uniforms.uTime = uniforms.uTime;
    shader.uniforms.uWindSpeed = uniforms.uWindSpeed;
    shader.uniforms.uWindDirection = uniforms.uWindDirection;
    shader.uniforms.time = uniforms.time;
    shader.uniforms.windSpeed = uniforms.windSpeed;
    shader.uniforms.windAngle = uniforms.windAngle;

    shader.vertexShader = shader.vertexShader.replace(
      '#include <common>',
      `#include <common>
       uniform float uTime;
       uniform float uWindSpeed;
       uniform float uWindDirection;
       uniform float time;
       uniform float windSpeed;
       uniform float windAngle;
      `
    );

    shader.vertexShader = shader.vertexShader.replace(
      '#include <begin_vertex>',
      `#include <begin_vertex>
       float heightFactor = max(0.0, position.y);
       float sway = sin(time * windSpeed * 2.5 + position.x * 0.4 + position.z * 0.4) * (heightFactor * heightFactor * 0.04) * windSpeed;
       vec2 windDirVec = vec2(cos(windAngle), sin(windAngle));
       transformed.x += windDirVec.x * sway;
       transformed.z += windDirVec.y * sway;
      `
    );
  };

  const customizeGrassSwayShader = (shader: any) => {
    shader.uniforms.uTime = uniforms.uTime;
    shader.uniforms.uWindSpeed = uniforms.uWindSpeed;
    shader.uniforms.uWindDirection = uniforms.uWindDirection;
    shader.uniforms.time = uniforms.time;
    shader.uniforms.windSpeed = uniforms.windSpeed;
    shader.uniforms.windAngle = uniforms.windAngle;

    shader.vertexShader = shader.vertexShader.replace(
      '#include <common>',
      `#include <common>
       uniform float uTime;
       uniform float uWindSpeed;
       uniform float uWindDirection;
       uniform float time;
       uniform float windSpeed;
       uniform float windAngle;
      `
    );

    shader.vertexShader = shader.vertexShader.replace(
      '#include <begin_vertex>',
      `#include <begin_vertex>
       float heightFactor = max(0.0, position.y);
       float sway = sin(time * windSpeed * 3.5 + position.x * 0.8 + position.z * 0.8) * (heightFactor * 0.15) * windSpeed;
       vec2 windDirVec = vec2(cos(windAngle), sin(windAngle));
       transformed.x += windDirVec.x * sway;
       transformed.z += windDirVec.y * sway;
      `
    );
  };

  // Materials setup
  const materials = useMemo(() => {
    const createWindMat = (hex: string) => {
      const mat = new THREE.MeshStandardMaterial({
        color: new THREE.Color(hex),
        roughness: 0.9,
        metalness: 0.1,
      });
      mat.userData.uniforms = uniforms;
      mat.onBeforeCompile = customizeWindSwayShader;
      return mat;
    };

    const createStaticMat = (hex: string) => {
      return new THREE.MeshStandardMaterial({
        color: new THREE.Color(hex),
        roughness: 0.8,
        metalness: 0.2,
      });
    };

    const createGrassMat = (hex: string) => {
      const mat = new THREE.MeshStandardMaterial({
        color: new THREE.Color(hex),
        roughness: 1.0,
        metalness: 0.0,
        side: THREE.DoubleSide,
      });
      mat.userData.uniforms = uniforms;
      mat.onBeforeCompile = customizeGrassSwayShader;
      return mat;
    };

    return {
      OakTrunk: createWindMat('#543b23'),
      OakLeaves: createWindMat('#6b8e23'),
      PineTrunk: createWindMat('#5c4020'),
      PineLeaves1: createWindMat('#2e6b34'),
      PineLeaves2: createWindMat('#358238'),
      PineLeaves3: createWindMat('#3d9440'),
      Bush: createWindMat('#5a9e45'),
      Rock: createStaticMat('#a08860'),
      PalmTrunk: createWindMat('#8b6914'),
      PalmLeaves: createWindMat('#44aa33'),
      Cactus: createWindMat('#3a7a30'),
      JungleTrunk: createWindMat('#4a3520'),
      JungleLeaves: createWindMat('#1a6618'),
      BirchTrunk: createWindMat('#ccccbb'),
      BirchLeaves: createWindMat('#88b840'),
      Flowers: createWindMat('#dd6699'),
      Grass: createGrassMat('#4ade80'),
    };
  }, [uniforms]);

  // Instance refs
  const oakTRef = useRef<THREE.InstancedMesh>(null);
  const oakLRef = useRef<THREE.InstancedMesh>(null);
  const pineTRef = useRef<THREE.InstancedMesh>(null);
  const pineL1Ref = useRef<THREE.InstancedMesh>(null);
  const pineL2Ref = useRef<THREE.InstancedMesh>(null);
  const pineL3Ref = useRef<THREE.InstancedMesh>(null);
  const bushRef = useRef<THREE.InstancedMesh>(null);
  const rockRef = useRef<THREE.InstancedMesh>(null);
  const palmTRef = useRef<THREE.InstancedMesh>(null);
  const palmLRef = useRef<THREE.InstancedMesh>(null);
  const cactusRef = useRef<THREE.InstancedMesh>(null);
  const jungleTRef = useRef<THREE.InstancedMesh>(null);
  const jungleLRef = useRef<THREE.InstancedMesh>(null);
  const birchTRef = useRef<THREE.InstancedMesh>(null);
  const birchLRef = useRef<THREE.InstancedMesh>(null);
  const flowersRef = useRef<THREE.InstancedMesh>(null);
  const grassRef = useRef<THREE.InstancedMesh>(null);

  const dummy = useMemo(() => new THREE.Object3D(), []);

  // Update instance matrices on placements change
  useLayoutEffect(() => {
    const setupInstances = (
      ref: React.RefObject<THREE.InstancedMesh>,
      placementsArray: any[],
      yOffset: number,
      rotType: 'y' | 'all' | 'none',
      seedOffset: number
    ) => {
      if (!ref.current) return;
      const inst = ref.current;
      if (placementsArray.length === 0) {
        const gx = Math.min(width - 1, Math.max(0, Math.floor(width / 2)));
        const gy = Math.min(height - 1, Math.max(0, Math.floor(height / 2)));
        const cell = terrain.grid[gy]?.[gx];
        const x = gx - width / 2;
        const z = gy - height / 2;
        const y = cell ? cell.elevation * 1.8 : 0;
        const fallbackYOffset = ref === flowersRef ? 0.3 : 0;

        dummy.position.set(x, y + fallbackYOffset, z);
        dummy.scale.set(0, 0, 0);
        dummy.rotation.set(0, 0, 0);
        dummy.updateMatrix();
        if (typeof inst.setMatrixAt === 'function') {
          inst.setMatrixAt(0, dummy.matrix);
        }
      } else {
        placementsArray.forEach((p, idx) => {
          const x = p.x - width / 2;
          const z = p.y - height / 2;
          const y = getBilinearInterpolatedElevation(p.x, p.y, width, height, terrain.grid) * 1.8;

          dummy.position.set(x, y + yOffset * p.scale, z);
          dummy.scale.set(p.scale, p.scale, p.scale);
          dummy.rotation.set(0, 0, 0);

          const rotSeed = p.x * seedOffset + p.y * (seedOffset + 1.23);
          const ry = rotSeed % (Math.PI * 2);

          if (rotType === 'y') {
            dummy.rotateY(ry);
          } else if (rotType === 'all') {
            dummy.rotation.set(ry * 3, ry * 3, ry * 3);
          }

          dummy.updateMatrix();
          if (typeof inst.setMatrixAt === 'function') {
            inst.setMatrixAt(idx, dummy.matrix);
          }
        });
      }
      if (inst.instanceMatrix) {
        inst.instanceMatrix.needsUpdate = true;
      }
    };

    setupInstances(oakTRef, oakPlacements, 1.25, 'y', 12.3);
    setupInstances(oakLRef, oakPlacements, 3.0, 'y', 12.3);

    setupInstances(pineTRef, pinePlacements, 1.0, 'none', 23.4);
    setupInstances(pineL1Ref, pinePlacements, 2.2, 'none', 23.4);
    setupInstances(pineL2Ref, pinePlacements, 3.4, 'none', 23.4);
    setupInstances(pineL3Ref, pinePlacements, 4.5, 'none', 23.4);

    setupInstances(bushRef, bushPlacements, 0.4, 'y', 34.5);
    setupInstances(rockRef, rockPlacements, -0.3, 'all', 45.6);

    setupInstances(palmTRef, palmPlacements, 2.5, 'y', 56.7);
    setupInstances(palmLRef, palmPlacements, 5.0, 'y', 56.7);

    setupInstances(cactusRef, cactusPlacements, 1.2, 'y', 67.8);

    setupInstances(jungleTRef, junglePlacements, 2.0, 'y', 78.9);
    setupInstances(jungleLRef, junglePlacements, 5.0, 'y', 78.9);

    setupInstances(birchTRef, birchPlacements, 1.5, 'y', 89.01);
    setupInstances(birchLRef, birchPlacements, 3.2, 'y', 89.01);

    setupInstances(flowersRef, flowersPlacements, 0.3, 'y', 100.12);
    setupInstances(grassRef, grassPlacements, 0, 'y', 111.23);
  }, [
    width,
    height,
    terrain,
    oakPlacements,
    pinePlacements,
    bushPlacements,
    rockPlacements,
    palmPlacements,
    cactusPlacements,
    junglePlacements,
    birchPlacements,
    flowersPlacements,
    grassPlacements,
    dummy,
  ]);

  return (
    <group
      name="vegetation-group"
      userData={{
        windSpeed,
        windAngle: currentWindDirection,
        density: filteredPlacements.length + grassPlacements.length,
        speciesCounts,
      }}
      data-wind-speed={windSpeed}
      data-wind-angle={currentWindDirection}
      data-density={filteredPlacements.length + grassPlacements.length}
    >
      <instancedMesh
        ref={oakTRef}
        name="vegetation-instanced-mesh-oak"
        args={[null as any, null as any, Math.max(1, speciesCounts.Oak)]}
      >
        <primitive object={geometries.OakTrunk} attach="geometry" />
        <primitive object={materials.OakTrunk} attach="material" />
      </instancedMesh>
      <instancedMesh
        ref={oakLRef}
        name="vegetation-instanced-mesh-oak-leaves"
        args={[null as any, null as any, Math.max(1, speciesCounts.Oak)]}
      >
        <primitive object={geometries.OakLeaves} attach="geometry" />
        <primitive object={materials.OakLeaves} attach="material" />
      </instancedMesh>

      <instancedMesh
        ref={pineTRef}
        name="vegetation-instanced-mesh-pine"
        args={[null as any, null as any, Math.max(1, speciesCounts.Pine)]}
      >
        <primitive object={geometries.PineTrunk} attach="geometry" />
        <primitive object={materials.PineTrunk} attach="material" />
      </instancedMesh>
      <instancedMesh
        ref={pineL1Ref}
        name="vegetation-instanced-mesh-pine-leaves-1"
        args={[null as any, null as any, Math.max(1, speciesCounts.Pine)]}
      >
        <primitive object={geometries.PineLeaves1} attach="geometry" />
        <primitive object={materials.PineLeaves1} attach="material" />
      </instancedMesh>
      <instancedMesh
        ref={pineL2Ref}
        name="vegetation-instanced-mesh-pine-leaves-2"
        args={[null as any, null as any, Math.max(1, speciesCounts.Pine)]}
      >
        <primitive object={geometries.PineLeaves2} attach="geometry" />
        <primitive object={materials.PineLeaves2} attach="material" />
      </instancedMesh>
      <instancedMesh
        ref={pineL3Ref}
        name="vegetation-instanced-mesh-pine-leaves-3"
        args={[null as any, null as any, Math.max(1, speciesCounts.Pine)]}
      >
        <primitive object={geometries.PineLeaves3} attach="geometry" />
        <primitive object={materials.PineLeaves3} attach="material" />
      </instancedMesh>

      <instancedMesh
        ref={bushRef}
        name="vegetation-instanced-mesh-bush"
        args={[null as any, null as any, Math.max(1, speciesCounts.Bush)]}
      >
        <primitive object={geometries.Bush} attach="geometry" />
        <primitive object={materials.Bush} attach="material" />
      </instancedMesh>

      <instancedMesh
        ref={rockRef}
        name="vegetation-instanced-mesh-rock"
        args={[null as any, null as any, Math.max(1, speciesCounts.Rock)]}
      >
        <primitive object={geometries.Rock} attach="geometry" />
        <primitive object={materials.Rock} attach="material" />
      </instancedMesh>

      <instancedMesh
        ref={palmTRef}
        name="vegetation-instanced-mesh-palm"
        args={[null as any, null as any, Math.max(1, speciesCounts.Palm)]}
      >
        <primitive object={geometries.PalmTrunk} attach="geometry" />
        <primitive object={materials.PalmTrunk} attach="material" />
      </instancedMesh>
      <instancedMesh
        ref={palmLRef}
        name="vegetation-instanced-mesh-palm-leaves"
        args={[null as any, null as any, Math.max(1, speciesCounts.Palm)]}
      >
        <primitive object={geometries.PalmLeaves} attach="geometry" />
        <primitive object={materials.PalmLeaves} attach="material" />
      </instancedMesh>

      <instancedMesh
        ref={cactusRef}
        name="vegetation-instanced-mesh-cactus"
        args={[null as any, null as any, Math.max(1, speciesCounts.Cactus)]}
      >
        <primitive object={geometries.Cactus} attach="geometry" />
        <primitive object={materials.Cactus} attach="material" />
      </instancedMesh>

      <instancedMesh
        ref={jungleTRef}
        name="vegetation-instanced-mesh-jungle"
        args={[null as any, null as any, Math.max(1, speciesCounts.Jungle)]}
      >
        <primitive object={geometries.JungleTrunk} attach="geometry" />
        <primitive object={materials.JungleTrunk} attach="material" />
      </instancedMesh>
      <instancedMesh
        ref={jungleLRef}
        name="vegetation-instanced-mesh-jungle-leaves"
        args={[null as any, null as any, Math.max(1, speciesCounts.Jungle)]}
      >
        <primitive object={geometries.JungleLeaves} attach="geometry" />
        <primitive object={materials.JungleLeaves} attach="material" />
      </instancedMesh>

      <instancedMesh
        ref={birchTRef}
        name="vegetation-instanced-mesh-birch"
        args={[null as any, null as any, Math.max(1, speciesCounts.Birch)]}
      >
        <primitive object={geometries.BirchTrunk} attach="geometry" />
        <primitive object={materials.BirchTrunk} attach="material" />
      </instancedMesh>
      <instancedMesh
        ref={birchLRef}
        name="vegetation-instanced-mesh-birch-leaves"
        args={[null as any, null as any, Math.max(1, speciesCounts.Birch)]}
      >
        <primitive object={geometries.BirchLeaves} attach="geometry" />
        <primitive object={materials.BirchLeaves} attach="material" />
      </instancedMesh>

      <instancedMesh
        ref={flowersRef}
        name="vegetation-instanced-mesh-flowers"
        args={[null as any, null as any, Math.max(1, speciesCounts.Flowers)]}
      >
        <primitive object={geometries.Flowers} attach="geometry" />
        <primitive object={materials.Flowers} attach="material" />
      </instancedMesh>

      <instancedMesh
        ref={grassRef}
        name="vegetation-instanced-mesh-grass"
        args={[null as any, null as any, Math.max(1, speciesCounts.Grass)]}
      >
        <primitive object={geometries.Grass} attach="geometry" />
        <primitive object={materials.Grass} attach="material" />
      </instancedMesh>
    </group>
  );
};

export default Vegetation;
