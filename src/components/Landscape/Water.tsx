import React, { useRef, useMemo, useEffect } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { generateTerrain, getBilinearInterpolatedElevation } from './utils/terrainGenerator';
import { getSkyParams } from './utils/skyParams'; // Assumes the refactored shared utility path

interface WaterProps {
  windSpeed?: number;
  reflectionColor?: string;
  depthTransparency?: number;
  width?: number;
  height?: number;
  timeOfDay?: number; // Added to sync day/night lighting
}

// Custom GLSL Vertex Shader
const vertexShader = `
  uniform float time;
  uniform float windSpeed;
  uniform float uWaterType;       // 0.0 = Ocean, 1.0 = Lake, 2.0 = River
  uniform vec2 uFlowDirection;    // Direction of river flow
  
  varying vec3 vWorldPosition;
  varying vec3 vViewPosition;
  varying vec3 vNormal;
  varying vec2 vUv;
  varying float vType;

  #include <fog_pars_vertex>

  void main() {
    vUv = uv;
    vType = uWaterType;
    
    vec3 localPos = position;
    
    // Wave displacements based on water body type
    if (uWaterType < 0.5) {
      // Ocean: Large slow swells
      float waveX = sin(position.x * 0.02 + time * windSpeed * 0.4) * 0.3;
      float waveY = cos(position.y * 0.02 + time * windSpeed * 0.3) * 0.3;
      localPos.z += waveX + waveY;
    } else if (uWaterType < 1.5) {
      // Lake: Calm tiny ripples
      localPos.z += sin(position.x * 0.2 + time * windSpeed * 1.5) * 
                    cos(position.y * 0.2 + time * windSpeed * 1.2) * 0.1;
    } else {
      // River: Flowing waves along uFlowDirection
      float flowTime = time * windSpeed * 2.0;
      float coord = dot(position.xz, normalize(uFlowDirection));
      localPos.y += sin(coord * 0.25 - flowTime) * 
                    cos(dot(position.xz, vec2(-uFlowDirection.y, uFlowDirection.x)) * 0.25 + time * 1.5) * 0.08;
    }

    // Transform to world space
    vec4 worldPos = modelMatrix * vec4(localPos, 1.0);
    vWorldPosition = worldPos.xyz;

    vNormal = normalize((modelMatrix * vec4(normal, 0.0)).xyz);
    vec4 mvPosition = viewMatrix * worldPos;
    vViewPosition = -mvPosition.xyz;

    #include <fog_vertex>

    gl_Position = projectionMatrix * mvPosition;
  }
`;

// Custom GLSL Fragment Shader
const fragmentShader = `
  uniform float time;
  uniform float windSpeed;
  uniform vec3 reflectionColor;
  uniform float depthTransparency;

  // Heightmap and terrain details
  uniform sampler2D tHeightMap;
  uniform vec2 terrainSize;

  // Lighting parameters
  uniform vec3 sunDirection;
  uniform vec3 sunColor;
  uniform float sunIntensity;

  uniform vec3 moonDirection;
  uniform vec3 moonColor;
  uniform float moonIntensity;

  uniform vec3 ambientColor;
  uniform float ambientIntensity;

  uniform vec2 uFlowDirection;

  varying vec3 vWorldPosition;
  varying vec3 vViewPosition;
  varying vec3 vNormal;
  varying vec2 vUv;
  varying float vType;

  #include <fog_pars_fragment>

  // Simple procedural noise functions
  float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
  }

  float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i + vec2(0.0,0.0)), hash(i + vec2(1.0,0.0)), u.x),
               mix(hash(i + vec2(0.0,1.0)), hash(i + vec2(1.0,1.0)), u.x), u.y);
  }

  // Fractional Brownian Motion (3 octaves) for foam patterns
  float fbm(vec2 p) {
    float v = 0.0;
    float a = 0.5;
    vec2 shift = vec2(100.0);
    mat2 rot = mat2(cos(0.5), sin(0.5), -sin(0.5), cos(0.5));
    for (int i = 0; i < 3; ++i) {
      v += a * noise(p);
      p = rot * p * 2.0 + shift;
      a *= 0.5;
    }
    return v;
  }

  void main() {
    // 1. Heightmap UV Mapping and Depth Calculation
    // Map world XZ coordinate [-size/2, size/2] to UV [0, 1]
    vec2 terrainUV = (vWorldPosition.xz + terrainSize * 0.5) / terrainSize;
    
    // Sample heightmap (elevation is pre-scaled by 1.8 in CPU texture creation)
    float terrainWorldY = texture2D(tHeightMap, terrainUV).r;
    
    // Calculate water depth
    float depth = vWorldPosition.y - terrainWorldY;
    depth = max(depth, 0.0); // Clamp negative depths

    // 2. Depth Blending
    float depthFactor = clamp(depth / 12.0, 0.0, 1.0); // Normalize over 12-unit depth
    
    vec3 shallowColor = vec3(0.48, 0.88, 1.0); // light teal
    vec3 deepColor = vec3(0.03, 0.18, 0.4);   // dark blue
    vec3 waterColor = mix(shallowColor, deepColor, depthFactor);

    // 3. Normal Perturbation (Ripples & Waves)
    vec3 normal = normalize(vNormal);
    
    vec2 waveUv1 = vWorldPosition.xz * 0.2 + vec2(time * 0.05 * windSpeed, time * 0.03 * windSpeed);
    vec2 waveUv2 = vWorldPosition.xz * 0.5 - vec2(time * 0.08 * windSpeed, -time * 0.06 * windSpeed);
    
    if (vType > 1.5) {
      // Rivers flow faster
      vec2 flowDir = normalize(uFlowDirection);
      waveUv1 += flowDir * time * 0.4 * windSpeed;
      waveUv2 -= flowDir * time * 0.3 * windSpeed;
    }
    
    float n1 = noise(waveUv1 * 6.0);
    float n2 = noise(waveUv2 * 10.0);
    
    normal.x += (n1 - 0.5) * 0.06 + (n2 - 0.5) * 0.03;
    normal.z += (n2 - 0.5) * 0.06 + (n1 - 0.5) * 0.03;
    normal = normalize(normal);

    // 4. Lighting and Day/Night integration
    vec3 viewDir = normalize(cameraPosition - vWorldPosition);
    
    // Sun Contribution (Lambertian diffuse + Blinn-Phong specular)
    vec3 sunDir = normalize(sunDirection);
    float sunDiffuse = max(dot(normal, sunDir), 0.0);
    vec3 sunHalfDir = normalize(sunDir + viewDir);
    float sunSpec = pow(max(dot(normal, sunHalfDir), 0.0), 64.0) * 1.5;

    // Moon Contribution
    vec3 moonDir = normalize(moonDirection);
    float moonDiffuse = max(dot(normal, moonDir), 0.0);
    vec3 moonHalfDir = normalize(moonDir + viewDir);
    float moonSpec = pow(max(dot(normal, moonHalfDir), 0.0), 32.0) * 0.6;

    // Ambient Term
    vec3 ambientTerm = ambientColor * ambientIntensity;

    // Combine light contributions
    vec3 diffuseTerm = (sunDiffuse * 0.3 * sunColor * sunIntensity) + (moonDiffuse * 0.2 * moonColor * moonIntensity);
    vec3 specularTerm = (sunSpec * sunColor * sunIntensity) + (moonSpec * moonColor * moonIntensity);
    vec3 finalColor = waterColor * (ambientTerm + diffuseTerm) + specularTerm;

    // Fresnel Reflection Mix
    float fresnel = pow(1.0 - max(dot(normal, viewDir), 0.0), 5.0);
    fresnel = clamp(fresnel, 0.0, 0.85);
    vec3 lightColorCombined = ambientTerm + diffuseTerm + specularTerm;
    finalColor = mix(finalColor, reflectionColor * lightColorCombined, fresnel);

    // 5. Shoreline Foam Lines
    float foamWidth = 1.0; // Foam zone thickness in world units
    float foamLine = 0.0;
    
    if (depth < foamWidth) {
      float foamNoise = fbm(vWorldPosition.xz * 2.0 + vec2(0.0, time * 0.8));
      float distFactor = clamp(1.0 - (depth / foamWidth), 0.0, 1.0);
      
      // Crisp foam strands using thresholded noise
      foamLine = step(0.45, foamNoise * distFactor);
      
      // Smoothly fade out foam exactly at the waterline contact
      float edgeBlend = smoothstep(0.0, 0.15, depth);
      foamLine *= edgeBlend;
      
      finalColor = mix(finalColor, vec3(1.0), foamLine * 0.9);
    }

    // 6. Base Transparency & Water Edge Softening
    float alpha = mix(0.35, 0.95, depthFactor) * depthTransparency;
    if (depth < foamWidth) {
      alpha = mix(alpha, 0.95, foamLine); // Foam increases opacity
    }
    
    // Smoothly fade to 0 opacity exactly at depth=0 (edge wetness blend)
    alpha *= smoothstep(0.0, 0.05, depth);

    gl_FragColor = vec4(finalColor, alpha);

    #include <fog_fragment>
  }
`;

export const Water: React.FC<WaterProps> = ({
  windSpeed = 1.0,
  reflectionColor = '#0055ff',
  depthTransparency = 0.8,
  width = 500,
  height = 500,
  timeOfDay = 12.0,
}) => {
  const oceanMeshRef = useRef<THREE.Mesh>(null);
  const oceanMaterialRef = useRef<THREE.ShaderMaterial>(null);
  const riverMeshRef = useRef<THREE.Mesh>(null);
  const riverMaterialRef = useRef<THREE.ShaderMaterial>(null);
  const lakesGroupRef = useRef<THREE.Group>(null);
  const particlesRef = useRef<THREE.Points>(null);

  // Generate terrain data
  const terrain = useMemo(() => generateTerrain(width, height, 'seed'), [width, height]);

  // Convert elevation data into a Float32 Red DataTexture
  const heightMapTexture = useMemo(() => {
    const data = new Float32Array(width * height);
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        // Pre-multiply elevation by 1.8 to match the physical terrain vertex transformation
        data[y * width + x] = terrain.grid[y][x].elevation * 1.8;
      }
    }
    const texture = new THREE.DataTexture(
      data,
      width,
      height,
      THREE.RedFormat,
      THREE.FloatType
    );
    texture.minFilter = THREE.LinearFilter;
    texture.magFilter = THREE.LinearFilter;
    texture.wrapS = THREE.ClampToEdgeWrapping;
    texture.wrapT = THREE.ClampToEdgeWrapping;
    texture.needsUpdate = true;
    return texture;
  }, [terrain, width, height]);

  // Ocean Geometry
  const oceanGeometry = useMemo(() => {
    const geom = new THREE.PlaneGeometry(900, 900, 64, 64); // Higher subdivisions for better swells
    return geom;
  }, []);

  // Connected Component Lake Detection (remains unchanged)
  const lakesList = useMemo(() => {
    const visited = new Uint8Array(width * height);
    const list: Array<{ x: number; z: number; waterY: number; radius: number; key: string }> = [];

    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const i = y * width + x;
        const cell = terrain.grid[y][x];
        if (cell.isLake && !visited[i]) {
          const queue: Array<[number, number]> = [[x, y]];
          visited[i] = 1;

          let sumX = 0;
          let sumY = 0;
          let sumWaterY = 0;
          let count = 0;
          const cells: Array<[number, number]> = [];

          let head = 0;
          while (head < queue.length) {
            const [cx, cy] = queue[head++];
            sumX += cx;
            sumY += cy;
            const cCell = terrain.grid[cy][cx];
            sumWaterY += cCell.waterY || 0;
            count++;
            cells.push([cx, cy]);

            for (let dy = -1; dy <= 1; dy++) {
              for (let dx = -1; dx <= 1; dx++) {
                if (dx === 0 && dy === 0) continue;
                const nx = cx + dx;
                const ny = cy + dy;
                if (nx >= 0 && nx < width && ny >= 0 && ny < height) {
                  const ni = ny * width + nx;
                  if (terrain.grid[ny][nx].isLake && !visited[ni]) {
                    visited[ni] = 1;
                    queue.push([nx, ny]);
                  }
                }
              }
            }
          }

          const avgX = sumX / count;
          const avgY = sumY / count;
          const avgWaterY = sumWaterY / count;

          let maxD = 0;
          for (const [cx, cy] of cells) {
            const dx = cx - avgX;
            const dy = cy - avgY;
            const d = Math.sqrt(dx * dx + dy * dy);
            if (d > maxD) maxD = d;
          }

          list.push({
            x: avgX - width / 2,
            z: avgY - height / 2,
            waterY: avgWaterY * 1.8,
            radius: Math.max(1.5, maxD),
            key: `lake-${list.length}-${avgX.toFixed(1)}-${avgY.toFixed(1)}`,
          });
        }
      }
    }
    return list;
  }, [terrain, width, height]);

  // River Vertex Height Resolver (unchanged)
  const getRiverVertexHeight = (cx: number, cy: number): number => {
    let minD = Infinity;
    let nearestLakeCell = null;
    const startX = Math.max(0, Math.floor(cx - 2));
    const endX = Math.min(width - 1, Math.ceil(cx + 2));
    const startY = Math.max(0, Math.floor(cy - 2));
    const endY = Math.min(height - 1, Math.ceil(cy + 2));

    for (let y = startY; y <= endY; y++) {
      for (let x = startX; x <= endX; x++) {
        const cell = terrain.grid[y]?.[x];
        if (cell && cell.isLake) {
          const d = Math.sqrt((x - cx) ** 2 + (y - cy) ** 2);
          if (d < minD) {
            minD = d;
            nearestLakeCell = cell;
          }
        }
      }
    }

    const terrainHeight = getBilinearInterpolatedElevation(cx, cy, width, height, terrain.grid) * 1.8;
    let targetHeight = terrainHeight;
    if (nearestLakeCell && nearestLakeCell.waterY !== undefined && minD <= 2.0) {
      const lakeWaterLevel = nearestLakeCell.waterY * 1.8;
      const t = Math.min(1.0, minD / 2.0);
      targetHeight = lakeWaterLevel * (1 - t) + terrainHeight * t;
    }

    if (targetHeight < 12.0) {
      const t = Math.min(1.0, Math.max(0.0, (targetHeight - 5.5) / 6.5));
      targetHeight = 5.5 * (1 - t) + targetHeight * t;
    }

    return targetHeight;
  };

  // Static River BufferGeometry Setup (Waves are offloaded to GPU vertex shader)
  const riverData = useMemo(() => {
    const vertices: number[] = [];

    for (let y = 0; y < height - 1; y++) {
      for (let x = 0; x < width - 1; x++) {
        const c00 = terrain.grid[y][x];
        const c10 = terrain.grid[y][x + 1];
        const c01 = terrain.grid[y + 1][x];
        const c11 = terrain.grid[y + 1][x + 1];

        const r00 = c00.isRiver;
        const r10 = c10.isRiver;
        const r01 = c01.isRiver;
        const r11 = c11.isRiver;

        if (r00 || r10 || r01 || r11) {
          const x0 = x - width / 2;
          const z0 = y - height / 2;
          const x1 = (x + 1) - width / 2;
          const z1 = (y + 1) - height / 2;

          const y00 = getRiverVertexHeight(x, y);
          const y10 = getRiverVertexHeight(x + 1, y);
          const y01 = getRiverVertexHeight(x, y + 1);
          const y11 = getRiverVertexHeight(x + 1, y + 1);

          // Triangle 1
          vertices.push(x0, y00 + 0.15, z0);
          vertices.push(x0, y01 + 0.15, z1);
          vertices.push(x1, y10 + 0.15, z0);

          // Triangle 2
          vertices.push(x1, y10 + 0.15, z0);
          vertices.push(x0, y01 + 0.15, z1);
          vertices.push(x1, y11 + 0.15, z1);
        }
      }
    }

    const geom = new THREE.BufferGeometry();
    const positions = new Float32Array(vertices);

    if (positions.length > 0) {
      geom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
      geom.computeVertexNormals();
    }

    return { geom, positions };
  }, [terrain, width, height]);

  // Waterfall Points and Particles setup (remains unchanged)
  const waterfallPoints = useMemo(() => {
    const points: { x: number; y: number; z: number }[] = [];
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const cell = terrain.grid[y][x];
        if (cell.isWaterfall) {
          points.push({
            x: x - width / 2,
            y: cell.elevation * 1.8,
            z: y - height / 2,
          });
        }
      }
    }
    return points;
  }, [terrain, width, height]);

  const particleData = useMemo(() => {
    const pPerPoint = 20;
    const count = waterfallPoints.length * pPerPoint;
    const positions = new Float32Array(count * 3);
    const velocities = new Float32Array(count * 3);
    const initialPositions = new Float32Array(count * 3);

    let idx = 0;
    waterfallPoints.forEach((pt) => {
      for (let i = 0; i < pPerPoint; i++) {
        const px = pt.x + (Math.random() - 0.5) * 0.5;
        const py = pt.y;
        const pz = pt.z + (Math.random() - 0.5) * 0.5;

        positions[idx * 3] = px;
        positions[idx * 3 + 1] = py;
        positions[idx * 3 + 2] = pz;

        initialPositions[idx * 3] = px;
        initialPositions[idx * 3 + 1] = py;
        initialPositions[idx * 3 + 2] = pz;

        velocities[idx * 3] = (Math.random() - 0.5) * 0.5;
        velocities[idx * 3 + 1] = -1.0 - Math.random() * 2.0;
        velocities[idx * 3 + 2] = (Math.random() - 0.5) * 0.5;

        idx++;
      }
    });

    return { positions, velocities, initialPositions, count };
  }, [waterfallPoints]);

  // Create base uniforms structure
  const createBaseUniforms = () => ({
    time: { value: 0 },
    windSpeed: { value: windSpeed },
    reflectionColor: { value: new THREE.Color(reflectionColor) },
    depthTransparency: { value: depthTransparency },
    
    // Heightmap
    tHeightMap: { value: heightMapTexture },
    terrainSize: { value: new THREE.Vector2(width, height) },
    
    // Lighting values updated on frame
    sunDirection: { value: new THREE.Vector3() },
    sunColor: { value: new THREE.Color() },
    sunIntensity: { value: 0 },
    
    moonDirection: { value: new THREE.Vector3() },
    moonColor: { value: new THREE.Color() },
    moonIntensity: { value: 0 },
    
    ambientColor: { value: new THREE.Color() },
    ambientIntensity: { value: 0 },
  });

  const oceanUniforms = useMemo(() => ({
    ...createBaseUniforms(),
    uWaterType: { value: 0.0 },
    uFlowDirection: { value: new THREE.Vector2(0, 0) },
  }), [heightMapTexture, width, height]);

  const riverUniforms = useMemo(() => ({
    ...createBaseUniforms(),
    uWaterType: { value: 2.0 },
    uFlowDirection: { value: new THREE.Vector2(1.0, 0.0) }, // Flows in +X direction
  }), [heightMapTexture, width, height]);

  // Frame Loop updates
  useFrame((state, delta) => {
    const time = state.clock.getElapsedTime();

    // 1. Calculate lighting parameters matching Sky.tsx
    const skyParams = getSkyParams(timeOfDay);
    const angle = ((timeOfDay - 6) / 24) * Math.PI * 2;
    
    const sunDir = new THREE.Vector3(
      Math.cos(angle),
      Math.sin(angle),
      Math.sin(angle) * 0.1
    ).normalize();
    
    const moonAngle = angle + Math.PI;
    const moonDir = new THREE.Vector3(
      Math.cos(moonAngle),
      Math.sin(moonAngle),
      Math.sin(moonAngle) * 0.1
    ).normalize();

    const sunIntensity = skyParams.sunIntensity;
    const moonIntensity = moonDir.y > 0 ? 0.2 * moonDir.y : 0.0;

    // Helper to sync uniforms on a material
    const syncUniforms = (mat: THREE.ShaderMaterial) => {
      if (!mat || !mat.uniforms) return;
      if (mat.uniforms.time) mat.uniforms.time.value = time;
      if (mat.uniforms.windSpeed) mat.uniforms.windSpeed.value = windSpeed;
      if (mat.uniforms.reflectionColor) mat.uniforms.reflectionColor.value.set(reflectionColor);
      if (mat.uniforms.depthTransparency) mat.uniforms.depthTransparency.value = depthTransparency;

      if (mat.uniforms.sunDirection) mat.uniforms.sunDirection.value.copy(sunDir);
      if (mat.uniforms.sunColor) mat.uniforms.sunColor.value.set(skyParams.sunColor);
      if (mat.uniforms.sunIntensity) mat.uniforms.sunIntensity.value = sunIntensity;

      if (mat.uniforms.moonDirection) mat.uniforms.moonDirection.value.copy(moonDir);
      if (mat.uniforms.moonColor) mat.uniforms.moonColor.value.set('#e0f2fe');
      if (mat.uniforms.moonIntensity) mat.uniforms.moonIntensity.value = moonIntensity;

      if (mat.uniforms.ambientColor) mat.uniforms.ambientColor.value.set(skyParams.ambientColor);
      if (mat.uniforms.ambientIntensity) mat.uniforms.ambientIntensity.value = skyParams.ambientIntensity;
    };

    // 2. Update Ocean
    if (oceanMaterialRef.current) {
      syncUniforms(oceanMaterialRef.current);
    }
    if (oceanMeshRef.current && oceanMeshRef.current.position) {
      oceanMeshRef.current.position.y = 5.5 + Math.sin(time * 0.8) * 0.2; // Ocean bobbing
    }

    // 3. Update River
    if (riverMaterialRef.current) {
      syncUniforms(riverMaterialRef.current);
    }

    // 4. Update Lakes
    if (lakesGroupRef.current && lakesGroupRef.current.children) {
      Array.from(lakesGroupRef.current.children).forEach((child: any, index) => {
        const lake = lakesList[index];
        if (lake && child.position) {
          const px = child.position.x || 0;
          child.position.y = lake.waterY + Math.sin(time * 1.2 + px * 0.01) * 0.15; // Lake bobbing
          if (child.material) {
            syncUniforms(child.material);
          }
        }
      });
    }

    // 5. Update Waterfall particles
    if (
      particlesRef.current &&
      particlesRef.current.geometry &&
      waterfallPoints.length > 0 &&
      particleData.count > 0
    ) {
      const geo = particlesRef.current.geometry;
      const posAttr = geo.getAttribute('position') as THREE.BufferAttribute;
      if (posAttr) {
        const posArray = posAttr.array as Float32Array;
        const { velocities, initialPositions, count } = particleData;
        const dt = Math.min(delta, 0.1);

        for (let i = 0; i < count; i++) {
          posArray[i * 3] += velocities[i * 3] * dt;
          posArray[i * 3 + 1] += velocities[i * 3 + 1] * dt;
          posArray[i * 3 + 2] += velocities[i * 3 + 2] * dt;

          const currentY = posArray[i * 3 + 1];
          const initY = initialPositions[i * 3 + 1];
          if (currentY < initY - 3.0) {
            posArray[i * 3] = initialPositions[i * 3] + (Math.random() - 0.5) * 0.5;
            posArray[i * 3 + 1] = initialPositions[i * 3 + 1];
            posArray[i * 3 + 2] = initialPositions[i * 3 + 2] + (Math.random() - 0.5) * 0.5;
          }
        }
        posAttr.needsUpdate = true;
      }
    }
  });

  useEffect(() => {
    return () => {
      heightMapTexture.dispose();
      oceanGeometry.dispose();
      if (riverData.geom) {
        riverData.geom.dispose();
      }
    };
  }, [heightMapTexture, oceanGeometry, riverData.geom]);

  return (
    <group name="water-group">
      {/* 1. Ocean Mesh */}
      <mesh
        ref={oceanMeshRef}
        name="water-mesh"
        data-testid="water-mesh"
        geometry={oceanGeometry}
        rotation-x={-Math.PI / 2}
        position={[0, 5.5, 0]}
        userData={{ windSpeed, reflectionColor, depthTransparency }}
        data-wind-speed={windSpeed}
        data-reflection-color={reflectionColor}
        data-depth-transparency={depthTransparency}
      >
        <shaderMaterial
          ref={oceanMaterialRef}
          vertexShader={vertexShader}
          fragmentShader={fragmentShader}
          uniforms={oceanUniforms}
          transparent
          depthWrite={false}
          fog={true}
        />
      </mesh>

      {/* 2. Lakes Group */}
      <group ref={lakesGroupRef} name="lakes-group">
        {lakesList.map((lake) => {
          // Lake-specific uniforms
          const lakeUniforms = {
            ...createBaseUniforms(),
            uWaterType: { value: 1.0 },
            uFlowDirection: { value: new THREE.Vector2(0, 0) },
          };
          return (
            <mesh
              key={lake.key}
              rotation-x={-Math.PI / 2}
              position={[lake.x, lake.waterY, lake.z]}
              name="lake-mesh"
            >
              <circleGeometry args={[lake.radius, 32]} />
              <shaderMaterial
                vertexShader={vertexShader}
                fragmentShader={fragmentShader}
                uniforms={lakeUniforms}
                transparent
                depthWrite={false}
                fog={true}
              />
            </mesh>
          );
        })}
      </group>

      {/* 3. River Mesh */}
      {riverData.positions.length > 0 && (
        <mesh ref={riverMeshRef} geometry={riverData.geom} name="river-mesh">
          <shaderMaterial
            ref={riverMaterialRef}
            vertexShader={vertexShader}
            fragmentShader={fragmentShader}
            uniforms={riverUniforms}
            transparent
            depthWrite={false}
            side={THREE.DoubleSide}
            fog={true}
          />
        </mesh>
      )}

      {/* 4. Waterfall particles */}
      <points ref={particlesRef} name="waterfall-particles">
        <bufferGeometry>
          <bufferAttribute
            attach="attributes-position"
            count={particleData.count}
            array={particleData.positions}
            itemSize={3}
          />
        </bufferGeometry>
        <pointsMaterial size={0.2} color="#ffffff" transparent opacity={0.8} />
      </points>
    </group>
  );
};

export default Water;
