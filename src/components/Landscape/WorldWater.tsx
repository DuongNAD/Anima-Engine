import React, { useMemo, useRef, useEffect } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import type { World } from './utils/worldGen';

// ---------------------------------------------------------------------------------------
// WorldWater — the ocean and inland lakes for the huge SoA world.
//
// Rivers/streams are NOT rendered here: they are baked into the terrain mesh's vertex colours
// (see WorldTerrain), so they read as a continuous ribbon following the ground instead of a
// trail of floating quads.
//
// Water surfaces are flat, horizontal planes (rotated -90deg about X, so local +z is up). The
// fragment shader samples a heightmap texture to get the terrain depth under each fragment,
// grades the colour shallow->deep, draws shoreline foam, and — crucially — fades alpha to 0
// where the plane sits over dry land. That last part lets each lake be ONE bounding-box plane:
// it only shows where the terrain is actually below the water level.
//
// - Ocean: one big plane at sea level.
// - Lakes: one plane per basin (World.lakeBasins), each sized to its cell-space bounding box.
//
// ShaderMaterials use fog={false} — three only injects fog uniforms for built-in materials.
// ---------------------------------------------------------------------------------------

export interface WorldWaterProps {
  world: World;
  renderSize?: number;
  heightRatio?: number;
  /** World-space direction TO the sun (the scene directional light's position direction). */
  sunDir?: [number, number, number];
}

const vertexShader = /* glsl */ `
  uniform float uTime;
  uniform float uWaveAmp;
  varying vec3 vWorldPos;

  void main() {
    // Plane is rotated -90deg about X, so local +z is world-up: displace z for gentle swell.
    vec3 p = position;
    float w = sin(position.x * 0.03 + uTime * 0.6) * 0.6
            + cos(position.y * 0.025 + uTime * 0.45) * 0.6;
    p.z += w * uWaveAmp;

    vec4 world = modelMatrix * vec4(p, 1.0);
    vWorldPos = world.xyz;
    gl_Position = projectionMatrix * viewMatrix * world;
  }
`;

const fragmentShader = /* glsl */ `
  precision highp float;

  uniform float uTime;
  uniform sampler2D uHeightMap;
  uniform float uTerrainSize;   // world-space extent of the terrain (= renderSize)
  uniform float uSeaY;
  uniform vec3 uSunDir;
  uniform vec3 uSunColor;
  uniform vec3 uShallow;
  uniform vec3 uDeep;
  uniform float uOpacity;

  varying vec3 vWorldPos;

  float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123); }
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
    // Terrain world-Y under this fragment (heightmap stores world height directly).
    vec2 uv = (vWorldPos.xz + uTerrainSize * 0.5) / uTerrainSize;
    float floorY = texture2D(uHeightMap, clamp(uv, 0.0, 1.0)).r;
    float depth = max(vWorldPos.y - floorY, 0.0);

    float depthN = clamp(depth / (uSeaY * 0.7 + 0.001), 0.0, 1.0);
    vec3 color = mix(uShallow, uDeep, depthN);

    // Animated micro-normal for sparkle + specular.
    vec2 sw = vWorldPos.xz * 0.25;
    float n1 = noise(sw + vec2(uTime * 0.20, uTime * 0.13));
    float n2 = noise(sw * 1.9 - vec2(uTime * 0.16, -uTime * 0.11));
    vec3 normal = normalize(vec3((n1 - 0.5) * 0.6, 1.0, (n2 - 0.5) * 0.6));

    vec3 viewDir = normalize(cameraPosition - vWorldPos);
    vec3 sunDir = normalize(uSunDir);
    vec3 halfDir = normalize(sunDir + viewDir);
    float spec = pow(max(dot(normal, halfDir), 0.0), 80.0);
    float diff = clamp(dot(normal, sunDir) * 0.5 + 0.6, 0.0, 1.0);
    color = color * diff + uSunColor * spec * 1.4;

    float fres = pow(1.0 - max(dot(normal, viewDir), 0.0), 4.0);
    color = mix(color, uShallow * 1.1 + 0.05, clamp(fres, 0.0, 0.6));

    float alpha = mix(0.55, 0.92, depthN) * uOpacity;

    // Shoreline foam where the water is very shallow.
    float foamBand = 1.0 - smoothstep(0.0, uSeaY * 0.12 + 0.001, depth);
    if (foamBand > 0.0) {
      float f = fbm(vWorldPos.xz * 0.6 + vec2(0.0, uTime * 0.6));
      float foam = smoothstep(0.45, 0.9, f) * foamBand;
      color = mix(color, vec3(1.0), foam);
      alpha = mix(alpha, 0.95, foam);
    }

    // Fade to 0 exactly at the waterline -> the plane only shows over submerged ground.
    alpha *= smoothstep(0.0, uSeaY * 0.03 + 0.001, depth);

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

  const heightUnits = renderSize * heightRatio;
  const seaY = world.seaLevel * heightUnits;

  // Heightmap: Float32 red texture storing the terrain's WORLD height per cell, so the shader
  // reads depth = waterY - terrainY directly.
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

  const makeUniforms = (shallow: string, deep: string, opacity: number, waveAmp: number) => ({
    uTime: { value: 0 },
    uWaveAmp: { value: waveAmp },
    uHeightMap: { value: heightTex },
    uTerrainSize: { value: renderSize },
    uSeaY: { value: seaY },
    uSunDir: { value: new THREE.Vector3(...sunDir) },
    uSunColor: { value: new THREE.Color('#fff4d6') },
    uShallow: { value: new THREE.Color(shallow) },
    uDeep: { value: new THREE.Color(deep) },
    uOpacity: { value: opacity },
  });

  const oceanUniforms = useMemo(
    () => makeUniforms('#3fcfe0', '#06203f', 0.88, 1.0),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [heightTex, renderSize, seaY],
  );

  // One shared material for every lake plane (uniforms updated once per frame).
  const lakeMaterial = useMemo(
    () =>
      new THREE.ShaderMaterial({
        vertexShader,
        fragmentShader,
        uniforms: makeUniforms('#57c7e8', '#134a76', 0.86, 0.4),
        transparent: true,
        depthWrite: false,
        fog: false,
      }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [heightTex, renderSize, seaY],
  );

  // One plane per lake basin, sized to its cell-space bounding box (+ a small margin). The
  // shader hides the parts over land, so a single bbox plane cleanly fills the whole basin.
  const lakePlanes = useMemo(() => {
    const { size, lakeBasins } = world;
    const margin = (renderSize / (size - 1)) * 2;
    const toWorld = (g: number) => (g / (size - 1) - 0.5) * renderSize;
    return lakeBasins.map((b) => {
      const x0 = toWorld(b.minX) - margin;
      const x1 = toWorld(b.maxX) + margin;
      const z0 = toWorld(b.minY) - margin;
      const z1 = toWorld(b.maxY) + margin;
      const w = Math.max(0.001, x1 - x0);
      const h = Math.max(0.001, z1 - z0);
      const seg = Math.max(1, Math.min(24, Math.round(Math.max(w, h) / 12)));
      const geom = new THREE.PlaneGeometry(w, h, seg, seg);
      return { geom, cx: (x0 + x1) / 2, cz: (z0 + z1) / 2, y: b.level * heightUnits };
    });
  }, [world, renderSize, heightUnits]);

  useFrame((state) => {
    const t = state.clock.getElapsedTime();
    if (oceanMat.current) {
      oceanMat.current.uniforms.uTime.value = t;
      oceanMat.current.uniforms.uSunDir.value.set(sunDir[0], sunDir[1], sunDir[2]);
    }
    lakeMaterial.uniforms.uTime.value = t;
    lakeMaterial.uniforms.uSunDir.value.set(sunDir[0], sunDir[1], sunDir[2]);
  });

  useEffect(() => {
    return () => {
      heightTex.dispose();
      lakeMaterial.dispose();
      lakePlanes.forEach((p) => p.geom.dispose());
    };
  }, [heightTex, lakeMaterial, lakePlanes]);

  return (
    <group name="world-water">
      {/* Ocean plane at sea level. */}
      <mesh rotation-x={-Math.PI / 2} position={[0, seaY, 0]} name="world-ocean">
        <planeGeometry args={[renderSize * 2.2, renderSize * 2.2, 96, 96]} />
        <shaderMaterial
          ref={oceanMat}
          vertexShader={vertexShader}
          fragmentShader={fragmentShader}
          uniforms={oceanUniforms}
          transparent
          depthWrite={false}
          fog={false}
        />
      </mesh>

      {/* One plane per lake basin. */}
      {lakePlanes.map((p, i) => (
        <mesh
          key={i}
          geometry={p.geom}
          material={lakeMaterial}
          rotation-x={-Math.PI / 2}
          position={[p.cx, p.y, p.cz]}
          name="world-lake"
        />
      ))}
    </group>
  );
};

export default WorldWater;
