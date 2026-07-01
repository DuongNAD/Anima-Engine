import React, { useMemo, useRef, useEffect } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import type { World } from './utils/worldGen';
import { Biome } from './utils/worldGen';

// ---------------------------------------------------------------------------------------
// WorldWater — custom-shader ocean + flowing inland rivers for the huge SoA world.
//
// The terrain (see WorldTerrain.tsx) maps a normalized cell coordinate u,v in [0,1] to world
// space as  X = (u-0.5)*renderSize ,  Z = (v-0.5)*renderSize ,  Y = elevation * heightUnits
// where heightUnits = renderSize * heightRatio. This component reuses that exact mapping so
// the water lines up with the land:
//   - Ocean: one large plane at seaY whose fragment shader samples a heightmap texture to
//     get the sea-floor depth, colouring shallow water bright teal and deep water dark blue,
//     and drawing shoreline foam where depth -> 0.
//   - Rivers: thin quads emitted for River-biome cells, sitting just above the terrain with
//     fast scrolling ripples.
//
// IMPORTANT: ShaderMaterials use `fog={false}` — three's fog uniforms are only injected for
// built-in materials and a custom shader referencing them crashes in refreshFogUniforms.
// ---------------------------------------------------------------------------------------

export interface WorldWaterProps {
  world: World;
  renderSize?: number;
  heightRatio?: number;
  /** World-space direction TO the sun (the scene directional light's position direction). */
  sunDir?: [number, number, number];
}

const sharedVertex = /* glsl */ `
  uniform float uTime;
  uniform float uWaterType;   // 0 = ocean, 1 = river
  uniform float uWaveAmp;

  varying vec3 vWorldPos;

  void main() {
    vec3 p = position;

    if (uWaterType < 0.5) {
      // Ocean: two crossing slow swells. Plane is rotated -90deg about X, so local +z is up.
      float w = sin(position.x * 0.03 + uTime * 0.6) * 0.6
              + cos(position.y * 0.025 + uTime * 0.45) * 0.6;
      p.z += w * uWaveAmp;
    } else {
      // River quads are already in world orientation (local +y is up): tiny chop.
      p.y += sin(position.x * 0.6 + position.z * 0.6 + uTime * 2.2) * 0.05 * uWaveAmp;
    }

    vec4 world = modelMatrix * vec4(p, 1.0);
    vWorldPos = world.xyz;
    gl_Position = projectionMatrix * viewMatrix * world;
  }
`;

const sharedFragment = /* glsl */ `
  precision highp float;

  uniform float uTime;
  uniform float uWaterType;
  uniform sampler2D uHeightMap;
  uniform float uTerrainSize;   // world-space extent of the terrain (= renderSize)
  uniform float uSeaY;
  uniform vec3 uSunDir;
  uniform vec3 uSunColor;
  uniform vec3 uShallow;
  uniform vec3 uDeep;
  uniform float uOpacity;

  varying vec3 vWorldPos;

  float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
  }
  float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
               mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
  }
  float fbm(vec2 p) {
    float v = 0.0, a = 0.5;
    for (int k = 0; k < 4; k++) { v += a * noise(p); p *= 2.0; a *= 0.5; }
    return v;
  }

  void main() {
    // Terrain world-Y under this water fragment (heightmap stores world height directly).
    vec2 uv = (vWorldPos.xz + uTerrainSize * 0.5) / uTerrainSize;
    float floorY = texture2D(uHeightMap, clamp(uv, 0.0, 1.0)).r;
    float depth = max(vWorldPos.y - floorY, 0.0);

    // Depth-graded colour: bright shallows -> dark deeps.
    float depthN = clamp(depth / (uSeaY * 0.7 + 0.001), 0.0, 1.0);
    vec3 color = mix(uShallow, uDeep, depthN);

    // Animated micro-normal from two scrolling noise fields -> sparkle + specular.
    vec2 sw = vWorldPos.xz * 0.25;
    float n1 = noise(sw + vec2(uTime * 0.20, uTime * 0.13));
    float n2 = noise(sw * 1.9 - vec2(uTime * 0.16, -uTime * 0.11));
    vec3 normal = normalize(vec3((n1 - 0.5) * 0.6, 1.0, (n2 - 0.5) * 0.6));

    vec3 viewDir = normalize(cameraPosition - vWorldPos);
    vec3 sunDir = normalize(uSunDir);               // direction TO the sun
    vec3 halfDir = normalize(sunDir + viewDir);
    float spec = pow(max(dot(normal, halfDir), 0.0), 80.0);
    float diff = clamp(dot(normal, sunDir) * 0.5 + 0.6, 0.0, 1.0);

    color *= diff;
    color += uSunColor * spec * 1.4;

    // Fresnel sky tint towards grazing angles.
    float fres = pow(1.0 - max(dot(normal, viewDir), 0.0), 4.0);
    color = mix(color, uShallow * 1.1 + 0.05, clamp(fres, 0.0, 0.6));

    float alpha = mix(0.55, 0.92, depthN) * uOpacity;

    bool isRiver = abs(uWaterType - 1.0) < 0.5;
    if (!isRiver) {
      // Ocean & lakes: shoreline foam (noisy white band where the water is very shallow).
      float foamBand = 1.0 - smoothstep(0.0, uSeaY * 0.12 + 0.001, depth);
      if (foamBand > 0.0) {
        float f = fbm(vWorldPos.xz * 0.6 + vec2(0.0, uTime * 0.6));
        float foam = smoothstep(0.45, 0.9, f) * foamBand;
        color = mix(color, vec3(1.0), foam);
        alpha = mix(alpha, 0.95, foam);
      }
      // Fade out exactly at the waterline so the surface edge isn't a hard line.
      alpha *= smoothstep(0.0, uSeaY * 0.03 + 0.001, depth);
    } else {
      // Rivers stay bright and fairly opaque regardless of (tiny) depth.
      color = mix(color, uShallow, 0.4);
      alpha = uOpacity;
    }

    gl_FragColor = vec4(color, alpha);
  }
`;

export const WorldWater: React.FC<WorldWaterProps> = ({
  world,
  renderSize = 400,
  heightRatio = 0.13,
  sunDir = [1, 1.5, 0.6],
}) => {
  const oceanMat = useRef<THREE.ShaderMaterial>(null);
  const riverMat = useRef<THREE.ShaderMaterial>(null);
  const lakeMat = useRef<THREE.ShaderMaterial>(null);

  const heightUnits = renderSize * heightRatio;
  const seaY = world.seaLevel * heightUnits;

  // Heightmap as a Float32 red texture storing the terrain's WORLD height per cell, so the
  // shader can read sea-floor depth directly. Row order matches WorldTerrain's u,v sampling.
  const heightTex = useMemo(() => {
    const { size, elevation } = world;
    const data = new Float32Array(size * size);
    for (let i = 0; i < data.length; i++) data[i] = elevation[i] * heightUnits;
    const tex = new THREE.DataTexture(data, size, size, THREE.RedFormat, THREE.FloatType);
    tex.minFilter = THREE.LinearFilter;
    tex.magFilter = THREE.LinearFilter;
    tex.wrapS = THREE.ClampToEdgeWrapping;
    tex.wrapT = THREE.ClampToEdgeWrapping;
    tex.needsUpdate = true;
    return tex;
  }, [world, heightUnits]);

  // River geometry: quads for River-biome cells, lifted just above the terrain.
  const riverGeom = useMemo(() => {
    const { size, biome, elevation } = world;
    const cell = renderSize / (size - 1);
    const half = cell * 0.85; // slight overlap so ribbons read as continuous
    const lift = heightUnits * 0.01 + 0.05;
    const verts: number[] = [];
    const worldXZ = (g: number) => (g / (size - 1) - 0.5) * renderSize;
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        if (biome[y * size + x] !== Biome.River) continue;
        const cx = worldXZ(x);
        const cz = worldXZ(y);
        const cy = elevation[y * size + x] * heightUnits + lift;
        // Two CCW (viewed from above) triangles per cell quad.
        verts.push(
          cx - half, cy, cz - half,
          cx - half, cy, cz + half,
          cx + half, cy, cz - half,
          cx + half, cy, cz - half,
          cx - half, cy, cz + half,
          cx + half, cy, cz + half,
        );
      }
    }
    if (verts.length === 0) return null;
    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.BufferAttribute(new Float32Array(verts), 3));
    return geom;
  }, [world, renderSize, heightUnits]);

  // Lake geometry: flat quads at each basin's still-water surface (world.water).
  const lakeGeom = useMemo(() => {
    const { size, water } = world;
    const cell = renderSize / (size - 1);
    const half = cell * 0.9;
    const verts: number[] = [];
    const worldXZ = (g: number) => (g / (size - 1) - 0.5) * renderSize;
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const w = water[y * size + x];
        if (w <= 0) continue;
        const cx = worldXZ(x);
        const cz = worldXZ(y);
        const cy = w * heightUnits; // water surface (already above the eroded floor)
        verts.push(
          cx - half, cy, cz - half,
          cx - half, cy, cz + half,
          cx + half, cy, cz - half,
          cx + half, cy, cz - half,
          cx - half, cy, cz + half,
          cx + half, cy, cz + half,
        );
      }
    }
    if (verts.length === 0) return null;
    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.BufferAttribute(new Float32Array(verts), 3));
    return geom;
  }, [world, renderSize, heightUnits]);

  const oceanUniforms = useMemo(
    () => ({
      uTime: { value: 0 },
      uWaterType: { value: 0.0 },
      uWaveAmp: { value: 1.0 },
      uHeightMap: { value: heightTex },
      uTerrainSize: { value: renderSize },
      uSeaY: { value: seaY },
      uSunDir: { value: new THREE.Vector3(...sunDir) },
      uSunColor: { value: new THREE.Color('#fff4d6') },
      uShallow: { value: new THREE.Color('#3fcfe0') },
      uDeep: { value: new THREE.Color('#06203f') },
      uOpacity: { value: 0.88 },
    }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [heightTex, renderSize, seaY],
  );

  const riverUniforms = useMemo(
    () => ({
      uTime: { value: 0 },
      uWaterType: { value: 1.0 },
      uWaveAmp: { value: 1.0 },
      uHeightMap: { value: heightTex },
      uTerrainSize: { value: renderSize },
      uSeaY: { value: seaY },
      uSunDir: { value: new THREE.Vector3(...sunDir) },
      uSunColor: { value: new THREE.Color('#fff4d6') },
      uShallow: { value: new THREE.Color('#5fd0ff') },
      uDeep: { value: new THREE.Color('#1f6aa0') },
      uOpacity: { value: 0.85 },
    }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [heightTex, renderSize, seaY],
  );

  const lakeUniforms = useMemo(
    () => ({
      uTime: { value: 0 },
      uWaterType: { value: 2.0 },
      uWaveAmp: { value: 0.6 },
      uHeightMap: { value: heightTex },
      uTerrainSize: { value: renderSize },
      uSeaY: { value: seaY },
      uSunDir: { value: new THREE.Vector3(...sunDir) },
      uSunColor: { value: new THREE.Color('#fff4d6') },
      uShallow: { value: new THREE.Color('#57c7e8') },
      uDeep: { value: new THREE.Color('#134a76') },
      uOpacity: { value: 0.86 },
    }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [heightTex, renderSize, seaY],
  );

  useFrame((state) => {
    const t = state.clock.getElapsedTime();
    if (oceanMat.current) {
      oceanMat.current.uniforms.uTime.value = t;
      oceanMat.current.uniforms.uSunDir.value.set(sunDir[0], sunDir[1], sunDir[2]);
    }
    if (riverMat.current) {
      riverMat.current.uniforms.uTime.value = t;
      riverMat.current.uniforms.uSunDir.value.set(sunDir[0], sunDir[1], sunDir[2]);
    }
    if (lakeMat.current) {
      lakeMat.current.uniforms.uTime.value = t;
      lakeMat.current.uniforms.uSunDir.value.set(sunDir[0], sunDir[1], sunDir[2]);
    }
  });

  useEffect(() => {
    return () => {
      heightTex.dispose();
      riverGeom?.dispose();
      lakeGeom?.dispose();
    };
  }, [heightTex, riverGeom, lakeGeom]);

  return (
    <group name="world-water">
      {/* Ocean plane, subdivided enough for visible swell. */}
      <mesh rotation-x={-Math.PI / 2} position={[0, seaY, 0]} name="world-ocean">
        <planeGeometry args={[renderSize * 2.2, renderSize * 2.2, 96, 96]} />
        <shaderMaterial
          ref={oceanMat}
          vertexShader={sharedVertex}
          fragmentShader={sharedFragment}
          uniforms={oceanUniforms}
          transparent
          depthWrite={false}
          fog={false}
        />
      </mesh>

      {/* Inland lakes (filled depressions / basins). */}
      {lakeGeom && (
        <mesh geometry={lakeGeom} name="world-lakes">
          <shaderMaterial
            ref={lakeMat}
            vertexShader={sharedVertex}
            fragmentShader={sharedFragment}
            uniforms={lakeUniforms}
            transparent
            depthWrite={false}
            fog={false}
          />
        </mesh>
      )}

      {/* Inland rivers (flow / River biome). */}
      {riverGeom && (
        <mesh geometry={riverGeom} name="world-rivers">
          <shaderMaterial
            ref={riverMat}
            vertexShader={sharedVertex}
            fragmentShader={sharedFragment}
            uniforms={riverUniforms}
            transparent
            depthWrite={false}
            side={THREE.DoubleSide}
            fog={false}
          />
        </mesh>
      )}
    </group>
  );
};

export default WorldWater;
