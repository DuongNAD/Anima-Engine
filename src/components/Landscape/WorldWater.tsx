import React, { useMemo, useRef, useState, useEffect } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';
import type { World } from './utils/worldGen';
import { sceneElapsed } from './utils/sceneClock';
import { OCEAN_SHELF_BAND_FRACTION } from './utils/oceanShelf';
import type { WorldV2LoadedBundle } from './worldV2/types';
import { ATMOSPHERE_FRAGMENT_PARS, atmosphereUniforms } from './materials/atmosphere';

export interface WorldWaterProps {
  world: World;
  worldV2?: WorldV2LoadedBundle | null;
  /** Decorative inland lakes/ponds (`ADR-0010 §4`). Ocean remains visible when false. */
  showDecorativeLakes?: boolean;
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
  // How far past the terrain footprint the sea floor takes to fall from the border height to
  // abyssal. See the note in main(): this is what stops the world's edge being drawn in water.
  uniform float uShelfBand;
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
  // Lake-basin mask (only sampled when uMaskOn=1, i.e. for the merged lake mesh). A basin's
  // bbox quad hangs over any DOWNHILL terrain that sits below the spill level but OUTSIDE
  // the basin — depth alone can't hide it (depth > 0 there), which used to leave sharp
  // floating water shards knifing through every slope. The mask pins water to actual
  // flooded cells.
  uniform sampler2D uLakeMask;
  uniform float uMaskOn;
  uniform sampler2D uShoreSdf;
  uniform sampler2D uHydroFlow;
  uniform float uUseWorldV2Water;

  varying vec3 vWorldPos;
  ${ATMOSPHERE_FRAGMENT_PARS}

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
    //
    // Outside the heightmap's footprint there is no terrain, and what the ocean does there decides
    // whether the world has a visible edge. Two wrong answers, in order:
    //
    //   clamp alone      the border texel smears out to infinity. Where land touches the border
    //                    that zeroes the water depth -> alpha 0 -> a sky-coloured hole fanning
    //                    across the horizon.
    //   times a step   the floor drops to abyssal in ONE TEXEL. Depth jumps from a shelf's few
    //                    units to the full column, so depthN goes 0->1, the colour snaps from
    //                    uShallow to uDeep and alpha from 0.32 to 0.92 — along a perfectly
    //                    straight line, on all four sides. That is the hard cyan cut-plane
    //                    independent review kept rejecting the canonical views for, and it was
    //                    never a framing problem: any elevated camera in orbit or fly mode sees
    //                    the world's square boundary drawn in water.
    //
    // The floor instead *descends* from the border height to abyssal across uShelfBand. The ramp
    // is exactly 1 at the boundary, so the sea floor is continuous with the terrain it continues
    // from, and reaches 0 well before the fog does — a continental shelf falling away into deep
    // water, which is also what the world is. Land at the border keeps its height for a few units
    // and then drops off, so the hole the clamp used to open never forms either.
    vec2 uv = (vWorldPos.xz + uTerrainSize * 0.5) / uTerrainSize;
    float borderDist = max(abs(vWorldPos.x), abs(vWorldPos.z)) - uTerrainSize * 0.5;
    float outside = max(borderDist, 0.0);
    float shelf = 1.0 - smoothstep(0.0, uShelfBand, outside);
    float floorY = texture2D(uHeightMap, clamp(uv, 0.0, 1.0)).r * shelf;
    float depth = max(vWorldPos.y - floorY, 0.0);

    // Smoothly drive the outer ocean into opaque deep water with a gently wavy outer margin
    // (boxN + nEdge from 0.84..0.98) plus a guaranteed hard floor inside -4%..-1% so coastal
    // shoals dissolve organically instead of forming a straight square perimeter or glacier sky cutout.
    float isOcean = 1.0 - step(0.5, uMaskOn);
    float boxN = max(abs(vWorldPos.x), abs(vWorldPos.z)) / (uTerrainSize * 0.5);
    float nEdge = (noise(vWorldPos.xz * 0.009) - 0.5) * 0.06;
    float edgeSoft = smoothstep(0.84, 0.98, boxN + nEdge) * isOcean;
    float edgeHard = smoothstep(-uTerrainSize * 0.04, -uTerrainSize * 0.01, borderDist) * isOcean;
    float edgeFade = max(edgeSoft, edgeHard);
    if (uUseWorldV2Water > 0.5) {
      // WV2-LOOK: a wider, noisier fall-off so the shelf colour does not stop on the map's line.
      float nEdge2 = (noise(vWorldPos.xz * 0.004) - 0.5) * 0.10 + (noise(vWorldPos.xz * 0.017) - 0.5) * 0.05;
      edgeFade = max(edgeFade * 0.0, smoothstep(0.86, 1.02, boxN + nEdge2) * isOcean);
    }
    float depthN = clamp(depth / (uSeaY * 0.7 + 0.001), 0.0, 1.0);
    depthN = mix(depthN, 1.0, edgeFade);
    vec3 color = mix(uShallow, uDeep, depthN);
    if (uUseWorldV2Water > 0.5) {
      // WV2-LOOK: absorption-style depth colour — turquoise over the shelf, through teal, to a
      // deep ocean blue that is not the near-black navy of the legacy palette.
      float dAbs = 1.0 - exp(-depth / (uSeaY * 0.10 + 0.001));
      float dDeep = 1.0 - exp(-depth / (uSeaY * 0.55 + 0.001));
      dAbs = mix(dAbs, 1.0, edgeFade);
      dDeep = mix(dDeep, 1.0, edgeFade);
      vec3 wShallow = vec3(0.16, 0.42, 0.40);
      vec3 wMid = vec3(0.05, 0.24, 0.30);
      vec3 wDeep = vec3(0.025, 0.11, 0.20);
      color = mix(mix(wShallow, wMid, dAbs), wDeep, dDeep);
      depthN = max(dAbs, depthN);
    }

    // Micro-detail dies completely before overview distance (~0.48 * terrainSize) so open ocean
    // never shimmers into white speckle noise.
    float camDist = distance(cameraPosition, vWorldPos);
    float detail = 1.0 - smoothstep(uTerrainSize * 0.12, uTerrainSize * 0.48, camDist);

    // Animated micro-normal for soft sun sheen.
    vec3 normal = vec3(0.0, 1.0, 0.0);
    if (uUseWorldV2Water < 0.5) {
      vec2 sw = vWorldPos.xz * 0.22;
      float n1 = noise(sw + vec2(uTime * 0.20, uTime * 0.13));
      float n2 = noise(sw * 1.9 - vec2(uTime * 0.16, -uTime * 0.11));
      normal = normalize(vec3((n1 - 0.5) * 0.48 * detail, 1.0, (n2 - 0.5) * 0.48 * detail));
    } else if (detail > 0.002) {
      // WV2-LOOK: wind ripples at a believable wavelength (~5-40 m), not 150 m blobs that the
      // broad specular lobe turned into cloud-like patches. Skipped where detail has faded.
      vec2 rw = vWorldPos.xz * 1.1;
      float r1 = noise(rw + vec2(uTime * 0.35, uTime * 0.21));
      float r2 = noise(rw * 2.3 - vec2(uTime * 0.27, -uTime * 0.19));
      float r3 = noise(rw * 0.37 + vec2(-uTime * 0.08, uTime * 0.05));
      float rx = (r1 - 0.5) * 0.22 + (r2 - 0.5) * 0.12 + (r3 - 0.5) * 0.10;
      float rz = (noise(rw.yx + 7.3 + uTime * 0.3) - 0.5) * 0.22 + (r2 - 0.5) * 0.08 + (r3 - 0.5) * 0.08;
      normal = normalize(vec3(rx * detail, 1.0, rz * detail));
    }
    vec3 viewDir = normalize(cameraPosition - vWorldPos);
    vec3 sunDir = normalize(uSunDir);
    vec3 halfDir = normalize(sunDir + viewDir);
    float ndh = max(dot(normal, halfDir), 0.0);
    float spec = uUseWorldV2Water > 0.5
      ? pow(ndh, 220.0) * 1.4 * detail + pow(ndh, 24.0) * 0.06
      : pow(ndh, 28.0) * 0.42 * detail * detail + pow(ndh, 8.0) * 0.12;
    float diff = clamp(dot(normal, sunDir) * 0.45 + 0.64, 0.0, 1.0);
    color = color * diff + uSunColor * spec;

    // Grazing angles reflect atmospheric horizon tint smoothly.
    float fres = pow(1.0 - max(dot(normal, viewDir), 0.0), 4.0);
    if (uUseWorldV2Water > 0.5) {
      // WV2-LOOK: Schlick fresnel against the sky dome's own gradient (shared uniforms).
      float ndv = max(dot(normal, viewDir), 0.0);
      float schlick = 0.02 + 0.98 * pow(1.0 - ndv, 5.0);
      vec3 rn = normalize(vec3(normal.x * 0.3, 1.0, normal.z * 0.3));
      float ndvR = max(dot(rn, viewDir), 0.0);
      schlick = 0.02 + 0.98 * pow(1.0 - ndvR, 5.0);
      vec3 rdir = reflect(-viewDir, rn);
      rdir.y = abs(rdir.y);
      vec3 skyR = atmoSkyColor(normalize(rdir));
      color = mix(color, skyR, clamp(schlick * 1.1 + 0.03, 0.0, 0.8));
    } else {
      color = mix(color, uFogColor, clamp(fres * 0.85, 0.0, 0.62));
    }

    // Transparent in coastal shallows (showing sandy/coral floor), seamlessly opaque in deep ocean
    // so the end of the terrain mesh at ±renderSize/2 never shows through the water.
    float oceanDeepOpaque = mix(uOpacity, 1.0, smoothstep(0.45, 0.92, depthN) * isOcean);
    float alpha = mix(0.36, 1.0, pow(depthN, 0.78)) * oceanDeepOpaque;
    if (uUseWorldV2Water > 0.5) alpha = max(alpha, 0.58 + 0.4 * depthN);

    // Shoreline foam strictly inside terrain boundaries & close camera range.
    float surf = sin(uTime * 0.9 + (vWorldPos.x + vWorldPos.z) * 0.045)
               + 0.5 * sin(uTime * 1.7 - (vWorldPos.x - vWorldPos.z) * 0.06);
    float foamReach = 0.95 + surf * 0.35;
    float waveExposure = 1.0;
    float sdfFoamBoost = 0.0;
    if (uUseWorldV2Water > 0.5) {
      vec4 sdfSample = texture2D(uShoreSdf, clamp(uv, 0.0, 1.0));
      float sdfM = sdfSample.r * 1000.0 - 500.0;
      waveExposure = mix(0.45, 1.55, sdfSample.g);
      float shoreRing = 1.0 - smoothstep(0.0, 95.0, abs(sdfM));
      sdfFoamBoost = shoreRing * (0.28 + 0.22 * sin(sdfM * 0.18 - uTime * 2.1)) * sdfSample.g;
      vec4 hydroSample = texture2D(uHydroFlow, clamp(uv, 0.0, 1.0));
      float estuaryMix = smoothstep(0.15, 0.85, hydroSample.a) * (1.0 - edgeFade) * 0.25;
      color = mix(color, mix(uShallow, vec3(0.42, 0.68, 0.72), 0.45), estuaryMix);
    }
    float foamBand = (1.0 - smoothstep(0.0, max(0.35, foamReach * waveExposure), depth)) * detail * detail * (1.0 - edgeFade);
    foamBand = clamp(foamBand + sdfFoamBoost * detail * (1.0 - edgeFade), 0.0, 1.0);
    if (foamBand > 0.001) {
      float f = fbm(vWorldPos.xz * 0.55 + vec2(0.0, uTime * 0.55));
      float foam = smoothstep(0.48, 0.90, f) * foamBand * 0.76;
      color = mix(color, vec3(0.96, 0.99, 1.0), foam);
      alpha = mix(alpha, 0.92, foam);
    }

    // Fade to 0 right at inland/coastal shoreline, then lock to opaque deep water past the border.
    float shoreWater = smoothstep(0.0, uSeaY * 0.03 + 0.001, depth);
    alpha *= shoreWater;
    alpha = mix(alpha, 1.0, edgeFade);

    // Lakes only: clip to flooded basin cells.
    if (uMaskOn > 0.5) {
      alpha *= smoothstep(0.2, 0.55, texture2D(uLakeMask, clamp(uv, 0.0, 1.0)).r);
    }

    // Smooth atmospheric fog + horizon melt so the ocean blends cleanly into the sky dome.
    float fogF = smoothstep(uFogNear * 0.72, uFogFar, camDist);
    if (uUseWorldV2Water > 0.5) {
      // WV2-LOOK: the same height-attenuated aerial perspective as terrain and flora.
      float hz = atmoHazeAmount(cameraPosition, vWorldPos);
      vec3 hdir = normalize(vWorldPos - cameraPosition);
      color = mix(color, atmoSkyColor(normalize(vec3(hdir.x, hdir.y * 0.2, hdir.z))), hz);
      alpha = mix(alpha, 1.0, hz * hz);
    } else {
      color = mix(color, uFogColor, fogF);
      alpha = mix(alpha, 1.0, fogF * fogF);
    }

    gl_FragColor = vec4(color, alpha);
  }
`;

/** The parts of the water uniforms that come from the world and the frame, not the palette. */
interface WaterUniformInputs {
  heightTex: THREE.Texture;
  lakeMaskTex: THREE.Texture;
  shoreSdfTex: THREE.Texture;
  hydroFlowTex: THREE.Texture;
  useWorldV2Water: number;
  renderSize: number;
  seaY: number;
  /** Seed only: `useFrame` writes `uSunDir` every frame from the live prop. */
  sunDir: [number, number, number];
}

/**
 * Build one water surface's uniform block.
 *
 * Module scope rather than a closure inside the component, because as a closure it captured five
 * values that its callers' dependency lists did not name — two lint-suppressed `useMemo`s whose
 * suppression was load-bearing, since listing `sunDir` honestly would have rebuilt the ocean's
 * uniforms on every render of a moving sun. Taking the inputs explicitly separates the two kinds:
 * what the block is *built* from (below) and what the frame loop *drives* (uTime, uSunDir, fog).
 */
function makeWaterUniforms(
  inputs: WaterUniformInputs,
  shallow: string,
  deep: string,
  opacity: number,
  waveAmp: number,
  maskOn = 0,
) {
  return {
    uTime: { value: 0 },
    uWaveAmp: { value: waveAmp },
    uHeightMap: { value: inputs.heightTex },
    uTerrainSize: { value: inputs.renderSize },
    uShelfBand: { value: inputs.renderSize * OCEAN_SHELF_BAND_FRACTION },
    uSeaY: { value: inputs.seaY },
    uSunDir: { value: new THREE.Vector3(...inputs.sunDir) },
    uSunColor: { value: new THREE.Color('#fff4d6') },
    uShallow: { value: new THREE.Color(shallow) },
    uDeep: { value: new THREE.Color(deep) },
    uOpacity: { value: opacity },
    uFogColor: { value: new THREE.Color('#cfe4f2') },
    uFogNear: { value: inputs.renderSize * 0.8 },
    uFogFar: { value: inputs.renderSize * 3.2 },
    uLakeMask: { value: inputs.lakeMaskTex },
    uMaskOn: { value: maskOn },
    uShoreSdf: { value: inputs.shoreSdfTex },
    uHydroFlow: { value: inputs.hydroFlowTex },
    uUseWorldV2Water: { value: inputs.useWorldV2Water },
    ...atmosphereUniforms,
  };
}

export const WorldWater: React.FC<WorldWaterProps> = ({
  world,
  worldV2 = null,
  showDecorativeLakes = true,
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

  // Per-cell lake mask (1 where standing water fills the cell), soft-edged by the GPU's
  // bilinear filter so basin shorelines stay smooth.
  const lakeMaskTex = useMemo(() => {
    const { size, water } = world;
    const data = new Uint8Array(size * size);
    for (let i = 0; i < data.length; i++) data[i] = water[i] > 0 ? 255 : 0;
    const tex = new THREE.DataTexture(data, size, size, THREE.RedFormat, THREE.UnsignedByteType);
    tex.minFilter = THREE.LinearFilter;
    tex.magFilter = THREE.LinearFilter;
    tex.wrapS = THREE.ClampToEdgeWrapping;
    tex.wrapT = THREE.ClampToEdgeWrapping;
    tex.needsUpdate = true;
    return tex;
  }, [world]);

  const { shoreSdfTex, hydroFlowTex } = useMemo(() => {
    if (worldV2) {
      const n = worldV2.size * worldV2.size;
      const sdfRgba = new Uint8Array(n * 4);
      for (let i = 0; i < n; i += 1) {
        sdfRgba[i * 4] = worldV2.shoreSdfRg[i * 2];
        sdfRgba[i * 4 + 1] = worldV2.shoreSdfRg[i * 2 + 1];
        sdfRgba[i * 4 + 3] = 255;
      }
      const sTex = new THREE.DataTexture(sdfRgba, worldV2.size, worldV2.size, THREE.RGBAFormat, THREE.UnsignedByteType);
      sTex.minFilter = THREE.LinearFilter;
      sTex.magFilter = THREE.LinearFilter;
      sTex.wrapS = THREE.ClampToEdgeWrapping;
      sTex.wrapT = THREE.ClampToEdgeWrapping;
      sTex.needsUpdate = true;

      const hTex = new THREE.DataTexture(
        new Uint8Array(worldV2.hydroFlowRgba),
        worldV2.size,
        worldV2.size,
        THREE.RGBAFormat,
        THREE.UnsignedByteType,
      );
      hTex.minFilter = THREE.LinearFilter;
      hTex.magFilter = THREE.LinearFilter;
      hTex.wrapS = THREE.ClampToEdgeWrapping;
      hTex.wrapT = THREE.ClampToEdgeWrapping;
      hTex.needsUpdate = true;
      return { shoreSdfTex: sTex, hydroFlowTex: hTex };
    }
    const empty = new Uint8Array([128, 0, 0, 255]);
    const sTex = new THREE.DataTexture(empty, 1, 1, THREE.RGBAFormat, THREE.UnsignedByteType);
    const hTex = new THREE.DataTexture(new Uint8Array([128, 128, 0, 0]), 1, 1, THREE.RGBAFormat, THREE.UnsignedByteType);
    sTex.needsUpdate = true;
    hTex.needsUpdate = true;
    return { shoreSdfTex: sTex, hydroFlowTex: hTex };
  }, [worldV2]);

  // The sun direction the uniform blocks are *seeded* with, captured at mount. `useFrame` writes
  // `uSunDir` from the live prop every frame, so this is an initial condition — pinning it is what
  // lets the two memos below list every value they read instead of suppressing the rule.
  const [uniformInputSunDir] = useState<[number, number, number]>(() => sunDir);
  const uniformInputs = useMemo<WaterUniformInputs>(
    () => ({
      heightTex,
      lakeMaskTex,
      shoreSdfTex,
      hydroFlowTex,
      useWorldV2Water: worldV2 ? 1 : 0,
      renderSize,
      seaY,
      sunDir: uniformInputSunDir,
    }),
    [heightTex, lakeMaskTex, shoreSdfTex, hydroFlowTex, worldV2, renderSize, seaY, uniformInputSunDir],
  );

  const oceanUniforms = useMemo(
    // WV2-LOOK: the swell amplitude was +-1 unit = +-43 m of sea level at World v2 scale, so
    // the shelf flooded and drained between frames. Keep a few metres.
    () => makeWaterUniforms(uniformInputs, '#2d99a2', '#103b66', 0.92, uniformInputs.useWorldV2Water ? 0.1 : 0.85),
    [uniformInputs],
  );

  // One shared material for every lake plane (uniforms updated once per frame).
  const lakeMaterial = useMemo(
    () =>
      new THREE.ShaderMaterial({
        vertexShader,
        fragmentShader,
        uniforms: makeWaterUniforms(uniformInputs, '#3b9cc0', '#164870', 0.88, 0.35, 1),
        transparent: true,
        depthWrite: false,
        fog: false,
      }),
    [uniformInputs],
  );

  // Lakes & ponds: one bbox quad per basin at its spill level, merged into (up to) TWO
  // geometries — liquid water and FROZEN water. Cold-climate basins (polar fringe, high
  // mountains via the lapse rate) render as opaque ice sheets instead of animated water.
  // The liquid fragment shader fades alpha to 0 over dry land, so each bbox quad only
  // shows across its actual basin; ice quads are shrunk slightly to hug their basin.
  const { lakesGeom, iceGeom } = useMemo(() => {
    const { size, temperature } = world;
    const lakeBasins = world.lakeBasins ?? []; // tolerate an older/partial world
    if (lakeBasins.length === 0) return { lakesGeom: null, iceGeom: null };
    const margin = (renderSize / (size - 1)) * 2;
    const toWorld = (g: number) => (g / (size - 1) - 0.5) * renderSize;
    const liquid: THREE.BufferGeometry[] = [];
    const ice: THREE.BufferGeometry[] = [];
    for (const b of lakeBasins) {
      const cx = Math.round((b.minX + b.maxX) / 2);
      const cy = Math.round((b.minY + b.maxY) / 2);
      const frozen = (temperature?.[cy * size + cx] ?? 0.5) < 0.19;
      const m = frozen ? margin * 0.25 : margin;
      const x0 = toWorld(b.minX) - m;
      const x1 = toWorld(b.maxX) + m;
      const z0 = toWorld(b.minY) - m;
      const z1 = toWorld(b.maxY) + m;
      const w = Math.max(0.001, x1 - x0);
      const h = Math.max(0.001, z1 - z0);
      const seg = frozen ? 1 : Math.max(1, Math.min(12, Math.round(Math.max(w, h) / 14)));
      const geom = new THREE.PlaneGeometry(w, h, seg, seg)
        .rotateX(-Math.PI / 2)
        .translate((x0 + x1) / 2, b.level * heightUnits + (frozen ? 0.12 : 0), (z0 + z1) / 2);
      (frozen ? ice : liquid).push(geom);
    }
    const mergedLiquid = liquid.length ? mergeGeometries(liquid, false) : null;
    const mergedIce = ice.length ? mergeGeometries(ice, false) : null;
    [...liquid, ...ice].forEach((p) => p.dispose());
    return { lakesGeom: mergedLiquid, iceGeom: mergedIce };
  }, [world, renderSize, heightUnits]);

  useFrame((state) => {
    const t = sceneElapsed(state.clock);
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
      lakeMaskTex.dispose();
      shoreSdfTex.dispose();
      hydroFlowTex.dispose();
      lakeMaterial.dispose();
      lakesGeom?.dispose();
      iceGeom?.dispose();
    };
  }, [heightTex, lakeMaskTex, shoreSdfTex, hydroFlowTex, lakeMaterial, lakesGeom, iceGeom]);

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

      {/* Every liquid lake & pond in ONE merged mesh (a single draw call, §6 decorative gate). */}
      {showDecorativeLakes && lakesGeom ? (
        <mesh geometry={lakesGeom} material={lakeMaterial} name="world-lakes" />
      ) : null}

      {/* Frozen lakes: opaque pale ice sheets over cold-climate basins. */}
      {/* WV2-LOOK prototype: World v2 skips the frozen-lake bbox quads. On the clipped 5,700 m
          summit plateau they read as pale squares on the snow; the snow shading covers them. */}
      {showDecorativeLakes && iceGeom && !worldV2 ? (
        <mesh geometry={iceGeom} name="world-lake-ice">
          <meshStandardMaterial color="#d7e9f2" roughness={0.35} metalness={0} />
        </mesh>
      ) : null}
    </group>
  );
};

export default WorldWater;
