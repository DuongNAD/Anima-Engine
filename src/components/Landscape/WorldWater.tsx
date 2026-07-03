import React, { useMemo, useRef, useEffect } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';
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
  uniform float uTerrainSize;
  varying vec3 vWorldPos;

  void main() {
    // Swell is applied in WORLD space (always along +Y), so the same shader serves both the
    // rotated ocean plane and the merged, pre-transformed lake mesh. It dies out with
    // distance: far vertices are further apart than the wavelength, so displacing them just
    // aliases into moire bands and a jagged horizon silhouette.
    vec4 world = modelMatrix * vec4(position, 1.0);
    float fade = 1.0 - smoothstep(uTerrainSize * 0.5, uTerrainSize * 1.4,
                                  distance(cameraPosition.xz, world.xz));
    float w = sin(world.x * 0.03 + uTime * 0.6) * 0.6
            + cos(world.z * 0.025 + uTime * 0.45) * 0.6;
    world.y += w * uWaveAmp * fade;

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
  // Manual fog, synced from scene.fog each frame. ShaderMaterials opt out of three's built-in
  // fog, but the water MUST fade exactly like the fogged terrain or the horizon shows a hard
  // seam of un-fogged, aliasing water.
  uniform vec3 uFogColor;
  uniform float uFogNear;
  uniform float uFogFar;

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
    // OUTSIDE the heightmap's footprint the floor is abyssal ocean (0), NOT the clamped
    // border texel: where land touches the map border, the clamp used to smear that land
    // height out to infinity, zeroing the water depth -> alpha 0 -> a sky-coloured hole
    // fanning across the horizon.
    vec2 uv = (vWorldPos.xz + uTerrainSize * 0.5) / uTerrainSize;
    float inMap = step(abs(uv.x - 0.5), 0.5) * step(abs(uv.y - 0.5), 0.5);
    float floorY = texture2D(uHeightMap, clamp(uv, 0.0, 1.0)).r * inMap;
    float depth = max(vWorldPos.y - floorY, 0.0);

    float depthN = clamp(depth / (uSeaY * 0.7 + 0.001), 0.0, 1.0);
    vec3 color = mix(uShallow, uDeep, depthN);

    // Micro-detail (waves / sparkle / foam) dies out with distance: far water is way past the
    // noise's pixel frequency and would otherwise shimmer into ugly static at the horizon.
    float camDist = distance(cameraPosition, vWorldPos);
    float detail = 1.0 - smoothstep(uTerrainSize * 0.35, uTerrainSize * 1.3, camDist) * 0.85;

    // Animated micro-normal for sparkle + specular.
    vec2 sw = vWorldPos.xz * 0.25;
    float n1 = noise(sw + vec2(uTime * 0.20, uTime * 0.13));
    float n2 = noise(sw * 1.9 - vec2(uTime * 0.16, -uTime * 0.11));
    vec3 normal = normalize(vec3((n1 - 0.5) * 0.6 * detail, 1.0, (n2 - 0.5) * 0.6 * detail));

    vec3 viewDir = normalize(cameraPosition - vWorldPos);
    vec3 sunDir = normalize(uSunDir);
    vec3 halfDir = normalize(sunDir + viewDir);
    // Specular glitter dies off hard with distance (detail^2): far water otherwise shows a
    // banded, aliasing sun-column instead of a soft glitter path.
    float spec = pow(max(dot(normal, halfDir), 0.0), 80.0) * detail * detail;
    float diff = clamp(dot(normal, sunDir) * 0.5 + 0.6, 0.0, 1.0);
    color = color * diff + uSunColor * spec * 1.4;

    // Grazing angles reflect the SKY (its haze tint), not the turquoise shallows.
    float fres = pow(1.0 - max(dot(normal, viewDir), 0.0), 4.0);
    color = mix(color, uFogColor, clamp(fres, 0.0, 0.55));

    // More transparent in the shallows (the sandy sea floor shows through as turquoise),
    // opaque over deep water.
    float alpha = mix(0.4, 0.92, depthN) * uOpacity;

    // Shoreline foam where the water is very shallow. The band is an ABSOLUTE depth (world
    // units), not a fraction of the sea depth: metres-deep mountain lakes would otherwise
    // sit entirely inside the band and froth over corner to corner.
    float foamBand = (1.0 - smoothstep(0.0, 1.5, depth)) * detail;
    if (foamBand > 0.0) {
      float f = fbm(vWorldPos.xz * 0.6 + vec2(0.0, uTime * 0.6));
      float foam = smoothstep(0.45, 0.9, f) * foamBand;
      color = mix(color, vec3(1.0), foam);
      alpha = mix(alpha, 0.95, foam);
    }

    // Fade to 0 exactly at the waterline -> the plane only shows over submerged ground.
    alpha *= smoothstep(0.0, uSeaY * 0.03 + 0.001, depth);

    // Manual fog, identical curve to the scene's linear fog: the far ocean melts into the
    // same haze as the terrain (and turns opaque, so the plane's rim can never show).
    float fogF = smoothstep(uFogNear, uFogFar, camDist);
    color = mix(color, uFogColor, fogF);
    alpha = mix(alpha, 1.0, fogF * fogF);

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
    uFogColor: { value: new THREE.Color('#cfe4f2') },
    uFogNear: { value: renderSize * 0.8 },
    uFogFar: { value: renderSize * 3.2 },
  });

  const oceanUniforms = useMemo(
    // Turquoise shallows -> deep blue. Shallow water is transparent so the sandy sea floor
    // tints it turquoise near the shore (see WorldTerrain's Beach-coloured coastal shelf).
    () => makeUniforms('#48ddca', '#05203f', 0.9, 1.0),
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

  // Lakes & ponds: one bbox quad per basin at its spill level, ALL merged into a single
  // geometry (one draw call for hundreds of water bodies). The fragment shader fades alpha
  // to 0 over dry land, so each bbox quad only shows across its actual basin.
  const lakesGeom = useMemo(() => {
    const { size } = world;
    const lakeBasins = world.lakeBasins ?? []; // tolerate an older/partial world
    if (lakeBasins.length === 0) return null;
    const margin = (renderSize / (size - 1)) * 2;
    const toWorld = (g: number) => (g / (size - 1) - 0.5) * renderSize;
    const parts = lakeBasins.map((b) => {
      const x0 = toWorld(b.minX) - margin;
      const x1 = toWorld(b.maxX) + margin;
      const z0 = toWorld(b.minY) - margin;
      const z1 = toWorld(b.maxY) + margin;
      const w = Math.max(0.001, x1 - x0);
      const h = Math.max(0.001, z1 - z0);
      const seg = Math.max(1, Math.min(12, Math.round(Math.max(w, h) / 14)));
      return new THREE.PlaneGeometry(w, h, seg, seg)
        .rotateX(-Math.PI / 2)
        .translate((x0 + x1) / 2, b.level * heightUnits, (z0 + z1) / 2);
    });
    const merged = mergeGeometries(parts, false);
    parts.forEach((p) => p.dispose());
    return merged;
  }, [world, renderSize, heightUnits]);

  useFrame((state) => {
    const t = state.clock.getElapsedTime();
    const fog = state.scene.fog as THREE.Fog | null; // WorldWeather owns scene.fog (linear)
    const mats = [oceanMat.current, lakeMaterial];
    for (const mat of mats) {
      if (!mat) continue;
      mat.uniforms.uTime.value = t;
      mat.uniforms.uSunDir.value.set(sunDir[0], sunDir[1], sunDir[2]);
      if (fog && typeof fog.near === 'number') {
        mat.uniforms.uFogColor.value.copy(fog.color);
        mat.uniforms.uFogNear.value = fog.near;
        mat.uniforms.uFogFar.value = fog.far;
      }
    }
  });

  useEffect(() => {
    return () => {
      heightTex.dispose();
      lakeMaterial.dispose();
      lakesGeom?.dispose();
    };
  }, [heightTex, lakeMaterial, lakesGeom]);

  return (
    <group name="world-water">
      {/* Ocean plane at sea level. Its half-width must exceed the camera's max orbit offset
          (renderSize*3.2) PLUS the far clip (renderSize*11), or the far-clip circle pokes past
          the plane's rim and the sky dome shines through as a bright wedge on the horizon.
          The shader has long since melted the water into the fog haze by that distance. */}
      <mesh rotation-x={-Math.PI / 2} position={[0, seaY, 0]} name="world-ocean" frustumCulled={false}>
        <planeGeometry args={[renderSize * 30, renderSize * 30, 128, 128]} />
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

      {/* Every lake & pond in ONE merged mesh (a single draw call). */}
      {lakesGeom && <mesh geometry={lakesGeom} material={lakeMaterial} name="world-lakes" />}
    </group>
  );
};

export default WorldWater;
