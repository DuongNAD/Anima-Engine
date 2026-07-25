import React, { useEffect, useMemo, useRef, useState } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import type { World } from './utils/worldGen';
import {
  buildColorTexture,
  buildNormalTexture,
  buildRoughnessTexture,
  buildDetailTexture,
} from './WorldTerrain';
import {
  makeChunkGrid,
  buildChunkGeometry,
  assignChunkLods,
  updateActiveChunks,
  resForLod,
} from './utils/chunkLod';

// ---------------------------------------------------------------------------------------
// WorldTerrainLod — the chunked, distance-LOD, streamable terrain (M3), an opt-in alternative
// to the single-mesh WorldTerrain. The world is a CxC grid of chunk meshes that share ONE
// material + the SAME baked textures, so the renderer can:
//   • frustum-cull whole off-screen chunks (three does this per-mesh automatically),
//   • draw distant chunks coarser — LOD that FOLLOWS THE CAMERA (dynamicLod), geometry per
//     (chunk, LOD) built once, cached, and swapped in imperatively (no rebuilds / re-renders),
//   • STREAM: with loadRadius > 0, only chunks near the camera stay resident; far ones unmount
//     and their geometry is disposed — so the resident/GPU cost is bounded no matter how large
//     the world grows (the path to worlds bigger than one in-memory mesh). Hysteresis
//     (unloadRadius > loadRadius) stops boundary thrashing; edge skirts hide LOD seams.
//
// Coordinates match WorldTerrain exactly, so a UNIFORM, un-streamed grid is pixel-identical —
// just partitioned. Numeric verification lives in chunkLod.test.ts + the chunk_check/dynlod/
// stream harnesses. NOTE: streaming unloads distant terrain, so it suits ground-level (walk/fly,
// fogged horizon) views, NOT the whole-map overview — hence opt-in, off by default. The live
// LOD/stream look (popping, skirts) still wants a visual pass on real hardware.
// ---------------------------------------------------------------------------------------

export interface WorldTerrainLodProps {
  world: World;
  renderSize?: number;
  heightRatio?: number;
  meshResolution?: number;
  /** Grid is chunksPerSide × chunksPerSide. */
  chunksPerSide?: number;
  /** Ascending world-radii from the camera; each crossing drops one LOD level. Empty = uniform. */
  lodDistances?: number[];
  /** Depth of the crack-hiding skirt around each LOD'd chunk (0 = none). */
  skirtDepth?: number;
  /** Follow the camera (re-LOD as it moves). If false, LOD is static from the map centre. */
  dynamicLod?: boolean;
  /** Stream chunks: keep only chunks within this world-radius of the camera resident (0 = off). */
  loadRadius?: number;
  /** Chunks unload past this radius (hysteresis). Defaults to loadRadius × 1.35 when streaming. */
  unloadRadius?: number;
}

// Re-evaluate LOD / streaming only after the camera has moved this far (world units).
const RECOMPUTE_MOVE_SQ = 30 * 30;

export const WorldTerrainLod: React.FC<WorldTerrainLodProps> = ({
  world,
  renderSize = 400,
  heightRatio = 0.13,
  meshResolution = 384,
  chunksPerSide = 6,
  lodDistances = [],
  skirtDepth = 0,
  dynamicLod = true,
  loadRadius = 0,
  unloadRadius,
}) => {
  const streamingOn = loadRadius > 0;
  const unload = unloadRadius && unloadRadius > loadRadius ? unloadRadius : loadRadius * 1.35;

  const colorMap = useMemo(() => buildColorTexture(world), [world]);
  const normalMap = useMemo(
    () => buildNormalTexture(world, renderSize, heightRatio, meshResolution),
    [world, renderSize, heightRatio, meshResolution],
  );
  const roughnessMap = useMemo(() => buildRoughnessTexture(world), [world]);
  const detailMap = useMemo(() => buildDetailTexture(), []);
  const riverMaskMap = useMemo(() => {
    const { size, riverAmt } = world;
    const data = new Uint8Array(size * size);
    if (riverAmt) data.set(riverAmt);
    const tex = new THREE.DataTexture(data, size, size, THREE.RedFormat, THREE.UnsignedByteType);
    tex.minFilter = THREE.LinearFilter;
    tex.magFilter = THREE.LinearFilter;
    tex.wrapS = THREE.ClampToEdgeWrapping;
    tex.wrapT = THREE.ClampToEdgeWrapping;
    tex.needsUpdate = true;
    return tex;
  }, [world]);

  const grid = useMemo(() => makeChunkGrid(chunksPerSide), [chunksPerSide]);

  // Lazy geometry cache keyed by `${chunkIndex}:${lod}` — each (chunk, LOD) is built at most once.
  const cacheBundle = useMemo(() => {
    const cache = new Map<string, THREE.BufferGeometry>();
    const get = (idx: number, lod: number): THREE.BufferGeometry => {
      const key = `${idx}:${lod}`;
      let g = cache.get(key);
      if (!g) {
        const res = resForLod(meshResolution, chunksPerSide, lod);
        g = buildChunkGeometry(world, grid[idx], res, renderSize, heightRatio, lod > 0 ? skirtDepth : 0);
        cache.set(key, g);
      }
      return g;
    };
    return { cache, get };
  }, [world, grid, chunksPerSide, meshResolution, renderSize, heightRatio, skirtDepth]);

  const initialLods = useMemo(
    () => assignChunkLods(grid, 0, 0, renderSize, lodDistances),
    [grid, renderSize, lodDistances],
  );

  // Which chunks are resident. Streaming off → all chunks; on → the initial load window at centre.
  const initialActive = useMemo(() => {
    if (!streamingOn) return new Set<number>(grid.map((_, i) => i));
    return updateActiveChunks(grid, 0, 0, renderSize, loadRadius, unload, new Set<number>()).active;
  }, [grid, streamingOn, renderSize, loadRadius, unload]);
  const [activeChunks, setActiveChunks] = useState<Set<number>>(initialActive);

  const meshRefs = useRef<Array<THREE.Mesh | null>>([]);
  const lodState = useRef<number[]>([]);
  const activeRef = useRef<Set<number>>(initialActive);
  const lastCam = useRef<{ x: number; z: number }>({ x: Infinity, z: Infinity });
  useEffect(() => {
    lodState.current = initialLods.slice();
    activeRef.current = initialActive;
    setActiveChunks(initialActive);
    lastCam.current = { x: Infinity, z: Infinity }; // force a recompute next frame
  }, [initialLods, initialActive, cacheBundle]);

  // After a commit (chunks that unmounted are gone), dispose the geometry of any non-resident
  // chunk so streaming actually frees memory.
  useEffect(() => {
    if (!streamingOn) return;
    const { cache } = cacheBundle;
    for (const key of Array.from(cache.keys())) {
      const idx = Number(key.slice(0, key.indexOf(':')));
      if (!activeChunks.has(idx)) {
        const g = cache.get(key);
        if (g && typeof g.dispose === 'function') g.dispose();
        cache.delete(key);
      }
    }
  }, [activeChunks, streamingOn, cacheBundle]);

  // One shared material with the exact same look + flowing-river shimmer as WorldTerrain.
  const shaderRef = useRef<{ uniforms: Record<string, { value: unknown }> } | null>(null);
  const material = useMemo(() => {
    const m = new THREE.MeshStandardMaterial({
      map: colorMap,
      normalMap,
      roughnessMap,
      roughness: 1.0,
      metalness: 0.0,
    });
    m.normalScale = new THREE.Vector2(1, 1);
    m.onBeforeCompile = (shader: { uniforms: Record<string, { value: unknown }>; fragmentShader: string }) => {
      shader.uniforms.uDetail = { value: detailMap };
      shader.uniforms.uRiverMask = { value: riverMaskMap };
      shader.uniforms.uTime = { value: 0 };
      shader.fragmentShader =
        'uniform sampler2D uDetail;\nuniform sampler2D uRiverMask;\nuniform float uTime;\n' +
        shader.fragmentShader.replace(
          '#include <map_fragment>',
          `#include <map_fragment>
          {
            float dtl = texture2D(uDetail, vMapUv * 220.0).r * 2.0;
            diffuseColor.rgb *= mix(1.0, dtl, 0.34);
            float riv = texture2D(uRiverMask, vMapUv).r;
            if (riv > 0.01) {
              float flow1 = texture2D(uDetail, vMapUv * 260.0 + vec2(uTime * 0.025, uTime * 0.045)).r * 2.0;
              float flow2 = texture2D(uDetail, vMapUv * 140.0 - vec2(uTime * 0.018, uTime * 0.03)).r * 2.0;
              diffuseColor.rgb *= mix(1.0, flow1 * 0.55 + flow2 * 0.45, riv * 0.3);
            }
          }`,
        );
      shaderRef.current = shader;
    };
    return m;
  }, [colorMap, normalMap, roughnessMap, detailMap, riverMaskMap]);

  useFrame((state) => {
    const sh = shaderRef.current;
    if (sh && sh.uniforms.uTime) sh.uniforms.uTime.value = state.clock.getElapsedTime();

    const cam = state.camera && state.camera.position;
    if (!cam || !Number.isFinite(cam.x) || !Number.isFinite(cam.z)) return; // (also skips test mocks)
    const wantsLod = dynamicLod && lodDistances.length > 0;
    if (!wantsLod && !streamingOn) return;
    const dx = cam.x - lastCam.current.x;
    const dz = cam.z - lastCam.current.z;
    if (dx * dx + dz * dz < RECOMPUTE_MOVE_SQ) return; // throttle
    lastCam.current = { x: cam.x, z: cam.z };

    // Camera-following LOD: swap each resident chunk's geometry when its LOD level changes.
    if (wantsLod) {
      const lods = assignChunkLods(grid, cam.x, cam.z, renderSize, lodDistances);
      for (let i = 0; i < grid.length; i++) {
        if (lods[i] === lodState.current[i]) continue;
        lodState.current[i] = lods[i];
        const mesh = meshRefs.current[i];
        if (mesh) mesh.geometry = cacheBundle.get(i, lods[i]);
      }
    }
    // Streaming: mount/unmount chunks by distance (disposal happens post-commit above).
    if (streamingOn) {
      const { active, changed } = updateActiveChunks(grid, cam.x, cam.z, renderSize, loadRadius, unload, activeRef.current);
      if (changed) {
        activeRef.current = active;
        setActiveChunks(active);
      }
    }
  });

  useEffect(() => {
    return () => {
      colorMap.dispose();
      normalMap.dispose();
      roughnessMap.dispose();
      detailMap.dispose();
      riverMaskMap.dispose();
      if (typeof material.dispose === 'function') material.dispose();
      cacheBundle.cache.forEach((g) => typeof g.dispose === 'function' && g.dispose());
      cacheBundle.cache.clear();
    };
  }, [colorMap, normalMap, roughnessMap, detailMap, riverMaskMap, material, cacheBundle]);

  const rendered = streamingOn ? Array.from(activeChunks) : grid.map((_, i) => i);
  return (
    <group name="world-terrain-lod">
      {rendered.map((i) => (
        <mesh
          key={i}
          ref={(m: THREE.Mesh | null) => {
            meshRefs.current[i] = m;
          }}
          geometry={cacheBundle.get(i, lodState.current[i] ?? initialLods[i])}
          material={material}
          receiveShadow
        />
      ))}
    </group>
  );
};

export default WorldTerrainLod;
