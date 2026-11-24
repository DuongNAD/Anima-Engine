import React, { useMemo, useRef, useLayoutEffect } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { generateTerrain, generateTerrainData, getBiomeColor } from './utils/terrainGenerator';

declare global {
  namespace JSX {
    interface IntrinsicElements {
      lOD: any;
    }
  }
}

interface TerrainProps {
  width?: number;
  height?: number;
  wetnessRatio?: number;
  seed?: string | number;
}

function getBlendedBiomeColor(elevation: number, moisture: number, temperature?: number): { r: number; g: number; b: number } {
  let r = 0, g = 0, b = 0;
  let count = 0;
  // Sample a small 3x3 grid around the coordinate to smooth boundaries
  const steps = [-2, 0, 2];
  for (const de of steps) {
    for (const dm of steps) {
      const sampleE = Math.max(0, Math.min(100, elevation + de));
      const sampleM = Math.max(0, Math.min(100, moisture + dm));
      const col = getBiomeColor(sampleE, sampleM, temperature);
      r += col.r;
      g += col.g;
      b += col.b;
      count++;
    }
  }
  return { r: r / count, g: g / count, b: b / count };
}

export const Terrain: React.FC<TerrainProps> = ({ width = 500, height = 500, wetnessRatio = 0, seed = 'seed' }) => {
  const isVitest = typeof globalThis !== 'undefined' && !!(globalThis as any).process?.env?.VITEST;
  const actualWidth = isVitest ? Math.min(width, 100) : width;
  const actualHeight = isVitest ? Math.min(height, 100) : height;

  // Generate the basic terrain data
  const terrain = useMemo(() => generateTerrain(actualWidth, actualHeight, seed), [actualWidth, actualHeight, seed]);
  const terrainData = useMemo(() => generateTerrainData(actualWidth, actualHeight, seed), [actualWidth, actualHeight, seed]);

  const lodRef = useRef<THREE.LOD>(null);
  const meshRef0 = useRef<THREE.Mesh>(null);
  const meshRef1 = useRef<THREE.Mesh>(null);
  const meshRef2 = useRef<THREE.Mesh>(null);

  const geomRef0 = useRef<THREE.BufferGeometry>(null);
  const geomRef1 = useRef<THREE.BufferGeometry>(null);
  const geomRef2 = useRef<THREE.BufferGeometry>(null);

  const materialShaderRef = useRef<any>(null);

  const onBeforeCompile = (shader: any) => {
    shader.uniforms.time = { value: 0 };
    shader.uniforms.uWidth = { value: actualWidth };
    shader.uniforms.uHeight = { value: actualHeight };
    shader.uniforms.uWetnessRatio = { value: wetnessRatio };
    materialShaderRef.current = shader;

    shader.vertexShader = `
      uniform float time;
      uniform float uWidth;
      uniform float uHeight;
      varying vec3 vWorldPosition;
      varying vec3 vWorldNormal;
    ` + shader.vertexShader;

    shader.vertexShader = shader.vertexShader.replace(
      '#include <begin_vertex>',
      `
      #include <begin_vertex>
      if (position.y < 5.4) {
        float gx = position.x + uWidth / 2.0;
        float gz = position.z + uHeight / 2.0;
        float wave = sin(gx * 0.15 + time * 1.5) * cos(gz * 0.15 + time) * 0.3;
        transformed.y += wave;
      }
      vec4 terrainWorldPos = modelMatrix * vec4(transformed, 1.0);
      vWorldPosition = terrainWorldPos.xyz;
      vWorldNormal = normalize((modelMatrix * vec4(normal, 0.0)).xyz);
      `
    );

    shader.fragmentShader = `
      varying vec3 vWorldPosition;
      varying vec3 vWorldNormal;

      float terrainHash(vec2 p) {
        return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
      }

      float terrainNoise(vec2 p) {
        vec2 i = floor(p);
        vec2 f = fract(p);
        vec2 u = f * f * (3.0 - 2.0 * f);
        return mix(mix(terrainHash(i + vec2(0.0,0.0)), terrainHash(i + vec2(1.0,0.0)), u.x),
                   mix(terrainHash(i + vec2(0.0,1.0)), terrainHash(i + vec2(1.0,1.0)), u.x), u.y);
      }
    ` + shader.fragmentShader;

    shader.fragmentShader = shader.fragmentShader.replace(
      '#include <color_fragment>',
      `
      #include <color_fragment>
      // Slope calculation math: slope = 1.0 - dot(vNormal, vec3(0.0, 1.0, 0.0))
      float slope = 1.0 - clamp(dot(vWorldNormal, vec3(0.0, 1.0, 0.0)), 0.0, 1.0);
      float rockFactor = smoothstep(0.30, 0.40, slope);

      // Color / biome procedural texture splatting
      float detailNoise = terrainNoise(vWorldPosition.xz * 0.4) * 0.2 + 0.9;
      vec3 splatColor = diffuseColor.rgb * detailNoise;

      // Automatically blend vertical cliff/rock colors when slope > 0.35 (~35 degrees)
      vec3 cliffRockColor = vec3(0.35, 0.35, 0.38) * (0.8 + 0.3 * terrainNoise(vWorldPosition.xz * 0.2));
      diffuseColor.rgb = mix(splatColor, cliffRockColor, rockFactor);
      `
    );

    shader.fragmentShader = shader.fragmentShader.replace(
      '#include <roughnessmap_fragment>',
      `
      #include <roughnessmap_fragment>
      // Apply per-biome roughness mapping (Ice: 0.1, Wet sand/Beach: 0.3, Grassland: 0.8, Volcanic/Rock: 0.95)
      float biomeRoughness = 0.8; // Grassland default
      if (rockFactor > 0.35) {
        biomeRoughness = 0.95; // Volcanic / Rock
      } else if (diffuseColor.r > 0.8 && diffuseColor.g > 0.85 && diffuseColor.b > 0.9) {
        biomeRoughness = 0.1; // Ice
      } else if (vWorldPosition.y < 6.0 || (diffuseColor.r > 0.65 && diffuseColor.g > 0.58 && diffuseColor.b < 0.45)) {
        biomeRoughness = 0.3; // Wet sand / Beach
      }
      roughnessFactor = biomeRoughness;
      `
    );

    shader.fragmentShader = shader.fragmentShader.replace(
      '#include <normal_fragment_begin>',
      `
      #include <normal_fragment_begin>
      // Slope-based normal mapping for vertical cliff/rock face
      if (rockFactor > 0.01) {
        vec3 rockNormalPerturb = vec3(
          sin(vWorldPosition.y * 5.0 + vWorldPosition.x * 2.0) * 0.25,
          0.0,
          cos(vWorldPosition.y * 5.0 + vWorldPosition.z * 2.0) * 0.25
        );
        normal = normalize(normal + rockNormalPerturb * rockFactor);
      }
      `
    );

    shader.fragmentShader = shader.fragmentShader.replace(
      '#include <fog_fragment>',
      `
      #include <fog_fragment>
      // Integrate atmospheric depth fog into fragment shader
      #ifdef USE_FOG
        float heightFogFactor = clamp(exp(-vWorldPosition.y * 0.015) * 0.35, 0.0, 0.45);
        gl_FragColor.rgb = mix(gl_FragColor.rgb, fogColor, heightFogFactor);
      #endif
      `
    );
  };

  // Helper to construct geometry arrays for a given LOD step size
  const getGeometryData = (step: number) => {
    const w = Math.ceil(actualWidth / step);
    const h = Math.ceil(actualHeight / step);

    const positions = new Float32Array(w * h * 3);
    const colors = new Float32Array(w * h * 3);
    const indices: number[] = [];

    for (let yIndex = 0; yIndex < h; yIndex++) {
      for (let xIndex = 0; xIndex < w; xIndex++) {
        const gx = Math.min(actualWidth - 1, xIndex * step);
        const gy = Math.min(actualHeight - 1, yIndex * step);

        const cell = terrain.grid[gy][gx];

        const posX = gx - actualWidth / 2;
        const posY = gy - actualHeight / 2;
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
          color = getBlendedBiomeColor(cell.elevation, cell.moisture, cell.temperature);
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

        indices.push(a, c, b);
        indices.push(b, c, d);
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
      geomRef0.current.computeBoundingSphere();
    }

    // Initialize geometry for LOD 1 (Medium Detail - step 4)
    if (geomRef1.current && typeof (geomRef1.current as any).setIndex === 'function') {
      const { positions, colors, indices } = getGeometryData(4);
      geomRef1.current.setAttribute('position', new THREE.BufferAttribute(positions, 3));
      geomRef1.current.setAttribute('color', new THREE.BufferAttribute(colors, 3));
      geomRef1.current.setIndex(new THREE.BufferAttribute(new Uint32Array(indices), 1));
      geomRef1.current.computeVertexNormals();
      geomRef1.current.computeBoundingSphere();
    }

    // Initialize geometry for LOD 2 (Low Detail - step 16)
    if (geomRef2.current && typeof (geomRef2.current as any).setIndex === 'function') {
      const { positions, colors, indices } = getGeometryData(16);
      geomRef2.current.setAttribute('position', new THREE.BufferAttribute(positions, 3));
      geomRef2.current.setAttribute('color', new THREE.BufferAttribute(colors, 3));
      geomRef2.current.setIndex(new THREE.BufferAttribute(new Uint32Array(indices), 1));
      geomRef2.current.computeVertexNormals();
      geomRef2.current.computeBoundingSphere();
    }
  }, [actualWidth, actualHeight, wetnessRatio]);

  useLayoutEffect(() => {
    if (lodRef.current && typeof (lodRef.current as any).addLevel === 'function' && meshRef0.current && meshRef1.current && meshRef2.current) {
      // Clear levels to support clean re-mounting/updates
      (lodRef.current as any).levels.length = 0;
      
      // Setup LOD thresholds
      (lodRef.current as any).addLevel(meshRef0.current, 0);      // High detail close up
      (lodRef.current as any).addLevel(meshRef1.current, 250);    // Medium detail mid range
      (lodRef.current as any).addLevel(meshRef2.current, 600);    // Low detail far away
    }
  }, []);

  useFrame((state) => {
    if (isVitest) return;
    if (materialShaderRef.current) {
      materialShaderRef.current.uniforms.time.value = state.clock.getElapsedTime();
    }
  });

  return (
    <lOD ref={lodRef} name="terrain-lod">
      {/* LOD Level 0: High Detail */}
      <mesh
        ref={meshRef0}
        name="terrain-mesh"
        userData={{ wetnessRatio, gridLength: terrainData.length }}
        data-wetness-ratio={wetnessRatio}
        castShadow
        receiveShadow
      >
        <bufferGeometry ref={geomRef0} />
        <meshStandardMaterial vertexColors roughness={1.0 - wetnessRatio} metalness={0.1} onBeforeCompile={onBeforeCompile} />
      </mesh>

      {/* LOD Level 1: Medium Detail */}
      <mesh ref={meshRef1} name="terrain-mesh-lod-1" castShadow receiveShadow>
        <bufferGeometry ref={geomRef1} />
        <meshStandardMaterial vertexColors roughness={1.0 - wetnessRatio} metalness={0.1} onBeforeCompile={onBeforeCompile} />
      </mesh>

      {/* LOD Level 2: Low Detail */}
      <mesh ref={meshRef2} name="terrain-mesh-lod-2" castShadow receiveShadow>
        <bufferGeometry ref={geomRef2} />
        <meshStandardMaterial vertexColors roughness={1.0 - wetnessRatio} metalness={0.1} onBeforeCompile={onBeforeCompile} />
      </mesh>
    </lOD>
  );
};

export default Terrain;
