import React, { useCallback, useMemo, useRef, useLayoutEffect, useEffect } from 'react';
import { extend, useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { generateTerrain, biomeColor, TERRAIN_HEIGHT_SCALE } from './utils/terrainGenerator';
import type { TerrainData } from './utils/terrainGenerator';
import { testAttrs } from './testAttrs';

// Register THREE.LOD with R3F so <lod> JSX element works
extend({ Lod: THREE.LOD });

// Shoreline ripple. Same wave the terrain has always had — amplitude, frequency and the 1.5x phase
// rate on X are carried over unchanged — but evaluated per vertex on the GPU.
//
// It used to run on the CPU in a `useFrame`: for each of the three LOD geometries it walked the
// whole grid (250,000 cells at LOD 0), wrote into the position buffer and then called
// `computeVertexNormals()` on a quarter-million vertices, every frame. That cost is why it was
// deleted. None of it was needed — the wave is a closed-form function of position and time, so the
// GPU can evaluate it in the vertex shader and the CPU's only per-frame job is writing one float.
const SHORE_WAVE_AMPLITUDE = 0.3;
const SHORE_WAVE_FREQUENCY = 0.15;
/** Below this elevation, and not a river, is shoreline. Matches the old CPU predicate exactly. */
const SHORE_ELEVATION_MAX = 3.0;

const glsl = (n: number) => n.toFixed(4);

/** The uniforms the shoreline wave drives. Only `uTime` changes per frame. */
export interface ShoreWaveUniforms {
  uTime: { value: number };
  uGridOrigin: { value: THREE.Vector2 };
}

/** The slice of three's `onBeforeCompile` shader argument this patch touches. */
interface PatchableShader {
  vertexShader: string;
  uniforms: Record<string, unknown>;
}

/**
 * Rewrites a MeshStandardMaterial vertex shader to displace and relight the shoreline.
 *
 * Exported and pure so it can be asserted without a GL context: under jsdom every material is a
 * plain DOM element, so a test that renders `<Terrain>` can never reach the compiled program. The
 * previous version of this feature had no test of any kind, which is how deleting it registered as
 * a pure performance win.
 */
export function applyShoreWave(shader: PatchableShader, uniforms: ShoreWaveUniforms): void {
  shader.uniforms.uTime = uniforms.uTime;
  shader.uniforms.uGridOrigin = uniforms.uGridOrigin;
  shader.vertexShader = shader.vertexShader
    .replace(
      '#include <common>',
      `#include <common>
      attribute float aShore;
      uniform float uTime;
      uniform vec2 uGridOrigin;`,
    )
    .replace(
      '#include <beginnormal_vertex>',
      `#include <beginnormal_vertex>
      vec2 shoreG = vec2(position.x + uGridOrigin.x, position.z + uGridOrigin.y);
      float shorePhaseX = shoreG.x * ${glsl(SHORE_WAVE_FREQUENCY)} + uTime * 1.5;
      float shorePhaseZ = shoreG.y * ${glsl(SHORE_WAVE_FREQUENCY)} + uTime;
      float shoreWave = ${glsl(SHORE_WAVE_AMPLITUDE)} * sin(shorePhaseX) * cos(shorePhaseZ);
      // The displaced normal, in closed form. This is what computeVertexNormals() bought, and why
      // deleting the animation was a visible loss rather than a free one: at this amplitude the eye
      // reads the moving light, not 0.3 units of displacement.
      float shoreDx = ${glsl(SHORE_WAVE_AMPLITUDE * SHORE_WAVE_FREQUENCY)} * cos(shorePhaseX) * cos(shorePhaseZ);
      float shoreDz = -${glsl(SHORE_WAVE_AMPLITUDE * SHORE_WAVE_FREQUENCY)} * sin(shorePhaseX) * sin(shorePhaseZ);
      objectNormal = normalize(mix(objectNormal, normalize(vec3(-shoreDx, 1.0, -shoreDz)), aShore));`,
    )
    .replace(
      '#include <begin_vertex>',
      `#include <begin_vertex>
      transformed.y += aShore * shoreWave;`,
    );
}

// `lod` used to be declared here as `any`. It is now in `src/r3f-intrinsics.d.ts` with the rest of
// the intrinsics, typed as `Object3DProps<THREE.LOD>`, so a second declaration here would only be a
// place for the two to disagree.

interface TerrainProps {
  width?: number;
  height?: number;
  wetnessRatio?: number;
  /** Shared, pre-generated (and cached) terrain. Falls back to generating when omitted. */
  terrain?: TerrainData;
}

export const Terrain: React.FC<TerrainProps> = ({ width = 500, height = 500, wetnessRatio = 0, terrain: propTerrain }) => {
  // Use the shared terrain when provided (generated once + cached); otherwise generate locally.
  const terrain = useMemo(
    () => propTerrain ?? generateTerrain(width, height, 'seed'),
    [propTerrain, width, height],
  );

  const lodRef = useRef<THREE.LOD>(null);
  const meshRef0 = useRef<THREE.Mesh>(null);
  const meshRef1 = useRef<THREE.Mesh>(null);
  const meshRef2 = useRef<THREE.Mesh>(null);

  const geomRef0 = useRef<THREE.BufferGeometry>(null);
  const geomRef1 = useRef<THREE.BufferGeometry>(null);
  const geomRef2 = useRef<THREE.BufferGeometry>(null);

  const matRef0 = useRef<THREE.MeshStandardMaterial>(null);
  const matRef1 = useRef<THREE.MeshStandardMaterial>(null);
  const matRef2 = useRef<THREE.MeshStandardMaterial>(null);

  // The wave's uniforms live in a ref, never in the render output: `uTime` is written every frame
  // and `uGridOrigin` on resize, and a value React renders is not the component's to mutate.
  const waveRef = useRef<ShoreWaveUniforms | null>(null);
  if (waveRef.current === null) {
    waveRef.current = {
      uTime: { value: 0 },
      // Grid space, not world space. The wave's phase was written against the generator's `(gx, gy)`
      // while a vertex sits at `gx - width / 2`; passing the offset keeps the shader's wave the same
      // wave, rather than one slid half a map sideways.
      uGridOrigin: { value: new THREE.Vector2(width / 2, height / 2) },
    };
  }

  // Installed onto the material through its ref rather than passed as an `onBeforeCompile` prop:
  // the materials stay ordinary `<meshStandardMaterial>` intrinsics, so R3F keeps driving
  // `roughness` from `wetnessRatio` exactly as before and nothing here mutates a rendered value.
  useLayoutEffect(() => {
    const uniforms = waveRef.current;
    if (!uniforms) return;
    for (const ref of [matRef0, matRef1, matRef2]) {
      const material = ref.current;
      // Same reason as the geometry guards below: under jsdom the r3f mock backs this intrinsic
      // with a plain DOM element, which has no onBeforeCompile and no program to rebuild.
      if (!material || typeof material.onBeforeCompile !== 'function') continue;
      material.onBeforeCompile = shader => applyShoreWave(shader, uniforms);
      material.needsUpdate = true;
    }
  }, []);

  // A resized map moves the wave's origin; it does not need a new material or a recompile.
  useEffect(() => {
    waveRef.current?.uGridOrigin.value.set(width / 2, height / 2);
  }, [width, height]);

  // The entire per-frame cost of the shoreline ripple, across all three LOD levels: one float.
  useFrame((state) => {
    const uniforms = waveRef.current;
    if (uniforms) uniforms.uTime.value = state.clock.getElapsedTime();
  });

  // Helper to construct geometry arrays for a given LOD step size. Memoised on what it reads, so
  // the layout effect below can name it: as a plain closure it was rebuilt on every render, and
  // listing it honestly would have rebuilt all three LOD geometries every time.
  const getGeometryData = useCallback((step: number) => {
    const w = Math.ceil(width / step);
    const h = Math.ceil(height / step);

    const positions = new Float32Array(w * h * 3);
    const colors = new Float32Array(w * h * 3);
    // 1 where the shoreline ripple applies, 0 elsewhere. Baked once here rather than tested per
    // frame, because the predicate reads `terrain.grid`, which the GPU cannot see.
    const shore = new Float32Array(w * h);
    const indices: number[] = [];

    for (let yIndex = 0; yIndex < h; yIndex++) {
      for (let xIndex = 0; xIndex < w; xIndex++) {
        const gx = Math.min(width - 1, xIndex * step);
        const gy = Math.min(height - 1, yIndex * step);

        const cell = terrain.grid[gy][gx];

        const posX = gx - width / 2;
        const posY = gy - height / 2;
        const posZ = cell.elevation * TERRAIN_HEIGHT_SCALE; // Elevation deformation

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
          color = biomeColor(cell.biome);
        }
        colors[i * 3] = color.r;
        colors[i * 3 + 1] = color.g;
        colors[i * 3 + 2] = color.b;

        shore[i] = cell.elevation < SHORE_ELEVATION_MAX && !cell.isRiver ? 1 : 0;
      }
    }

    for (let yIndex = 0; yIndex < h - 1; yIndex++) {
      for (let xIndex = 0; xIndex < w - 1; xIndex++) {
        const a = yIndex * w + xIndex;
        const b = yIndex * w + (xIndex + 1);
        const c = (yIndex + 1) * w + xIndex;
        const d = (yIndex + 1) * w + (xIndex + 1);

        // CCW winding when viewed from above so computeVertexNormals() yields +Y normals
        // (otherwise the flat top is back-face culled and the terrain looks transparent).
        indices.push(a, c, b);
        indices.push(b, c, d);
      }
    }

    return { positions, colors, shore, indices };
    // `wetnessRatio` is not read here. It was in the old effect's list, which is how a reader would
    // have assumed the LOD geometry re-bakes when the ground gets wet; it does not, and never did —
    // wetness is a material property, applied below.
  }, [terrain, width, height]);

  useLayoutEffect(() => {
    // Initialize geometry for LOD 0 (High Detail - step 1)
    if (geomRef0.current && typeof geomRef0.current.setIndex === 'function') {
      const { positions, colors, shore, indices } = getGeometryData(1);
      geomRef0.current.setAttribute('position', new THREE.BufferAttribute(positions, 3));
      geomRef0.current.setAttribute('color', new THREE.BufferAttribute(colors, 3));
      geomRef0.current.setAttribute('aShore', new THREE.BufferAttribute(shore, 1));
      geomRef0.current.setIndex(new THREE.BufferAttribute(new Uint32Array(indices), 1));
      geomRef0.current.computeVertexNormals();
    }

    // Initialize geometry for LOD 1 (Medium Detail - step 4)
    if (geomRef1.current && typeof geomRef1.current.setIndex === 'function') {
      const { positions, colors, shore, indices } = getGeometryData(4);
      geomRef1.current.setAttribute('position', new THREE.BufferAttribute(positions, 3));
      geomRef1.current.setAttribute('color', new THREE.BufferAttribute(colors, 3));
      geomRef1.current.setAttribute('aShore', new THREE.BufferAttribute(shore, 1));
      geomRef1.current.setIndex(new THREE.BufferAttribute(new Uint32Array(indices), 1));
      geomRef1.current.computeVertexNormals();
    }

    // Initialize geometry for LOD 2 (Low Detail - step 16)
    if (geomRef2.current && typeof geomRef2.current.setIndex === 'function') {
      const { positions, colors, shore, indices } = getGeometryData(16);
      geomRef2.current.setAttribute('position', new THREE.BufferAttribute(positions, 3));
      geomRef2.current.setAttribute('color', new THREE.BufferAttribute(colors, 3));
      geomRef2.current.setAttribute('aShore', new THREE.BufferAttribute(shore, 1));
      geomRef2.current.setIndex(new THREE.BufferAttribute(new Uint32Array(indices), 1));
      geomRef2.current.computeVertexNormals();
    }
  }, [getGeometryData]);

  useLayoutEffect(() => {
    // The `typeof` guards are not defensive noise: under jsdom the r3f mock backs these refs with
    // plain DOM elements, which have neither `addLevel` nor `setIndex`. The casts that used to wrap
    // them were, though — `lodRef` is a `THREE.LOD` and `geomRef*` are `THREE.BufferGeometry`, both
    // of which declare exactly these members.
    if (lodRef.current && typeof lodRef.current.addLevel === 'function' && meshRef0.current && meshRef1.current && meshRef2.current) {
      // Clear levels to support clean re-mounting/updates
      while (lodRef.current.levels && lodRef.current.levels.length > 0) { lodRef.current.levels.pop(); }

      // Setup LOD thresholds
      lodRef.current.addLevel(meshRef0.current, 0);      // High detail close up
      lodRef.current.addLevel(meshRef1.current, 150);    // Medium detail mid range
      lodRef.current.addLevel(meshRef2.current, 400);    // Low detail far away
    }
  }, []);

  useEffect(() => {
    const g0 = geomRef0.current;
    const g1 = geomRef1.current;
    const g2 = geomRef2.current;
    return () => {
      // Under jsdom tests, the R3F mock backs intrinsics with plain DOM elements that lack dispose methods
      if (g0 && typeof g0.dispose === 'function') g0.dispose();
      if (g1 && typeof g1.dispose === 'function') g1.dispose();
      if (g2 && typeof g2.dispose === 'function') g2.dispose();
    };
  }, []);


  return (
    <lod ref={lodRef} name="terrain-lod">
      {/* LOD Level 0: High Detail */}
      <mesh
        ref={meshRef0}
        name="terrain-mesh"
        userData={{ wetnessRatio, gridLength: width * height }}
        {...testAttrs({ 'data-wetness-ratio': wetnessRatio })}
      >
        <bufferGeometry ref={geomRef0} />
        <meshStandardMaterial ref={matRef0} vertexColors roughness={1.0 - wetnessRatio} metalness={0.1} />
      </mesh>

      {/* LOD Level 1: Medium Detail */}
      <mesh ref={meshRef1} name="terrain-mesh-lod-1">
        <bufferGeometry ref={geomRef1} />
        <meshStandardMaterial ref={matRef1} vertexColors roughness={1.0 - wetnessRatio} metalness={0.1} />
      </mesh>

      {/* LOD Level 2: Low Detail */}
      <mesh ref={meshRef2} name="terrain-mesh-lod-2">
        <bufferGeometry ref={geomRef2} />
        <meshStandardMaterial ref={matRef2} vertexColors roughness={1.0 - wetnessRatio} metalness={0.1} />
      </mesh>
    </lod>
  );
};

export default Terrain;
