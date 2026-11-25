import React, { useEffect, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import type { QualityTier } from '../utils/qualityTiers';
import { sceneElapsed } from '../utils/sceneClock';
import { ATMOSPHERE_FRAGMENT_PARS, atmosphereUniforms } from '../materials/atmosphere';
import { ELEV_SPAN_M, SEA_LEVEL_NORM, WORLD_EXTENT_M, type WorldV2LoadedBundle } from './types';

// prototype, to be replaced — WV2-LOOK surf spray / sea mist (reference board CST-01).
//
// One instanced draw call of soft, camera-facing (Y-axis) haze cards standing on the surf zone,
// placed from the baked shore SDF where the bake's wave exposure is high. Each card is a low
// bright veil, densest at the water and fading upward, so it softens the beach and land just
// behind the breakers. It brightens when you look toward the sun (forward scattering) and
// shares the aerial perspective, so distant spray melts into the haze like everything else.
// High tier: up to 900 cards; medium: 450; low: off (no draw call). Exposed coasts get more
// and denser cards than sheltered ones.

export interface WorldV2SprayProps {
  bundle: WorldV2LoadedBundle;
  renderSize?: number;
  heightRatio?: number;
  tier?: QualityTier;
}

const VERT = /* glsl */ `
  attribute vec2 aSeed;
  varying vec2 vUv;
  varying vec2 vSeed;
  varying vec3 vWp;
  void main() {
    vec3 center = (modelMatrix * instanceMatrix * vec4(0.0, 0.0, 0.0, 1.0)).xyz;
    float sx = length(instanceMatrix[0].xyz);
    float sy = length(instanceMatrix[1].xyz);
    vec3 toCam = cameraPosition - center;
    // Cards the camera stands in are fully faded anyway (nearFade): collapse them so their
    // (large, invisible) quads cost no fill.
    float camD = length(toCam);
    float keep = step(1.5, camD);
    vec3 right = normalize(vec3(toCam.z, 0.0, -toCam.x) + vec3(1e-5, 0.0, 0.0));
    vec3 wp = center + (right * position.x * sx + vec3(0.0, position.y * sy, 0.0)) * keep;
    vUv = uv;
    vSeed = aSeed;
    vWp = wp;
    gl_Position = projectionMatrix * viewMatrix * vec4(wp, 1.0);
  }
`;

const FRAG = /* glsl */ `
  precision highp float;
  uniform float uTime;
  uniform float uOpacity;
  varying vec2 vUv;
  varying vec2 vSeed;
  varying vec3 vWp;
  ${ATMOSPHERE_FRAGMENT_PARS}
  void main() {
    float x = vUv.x * 2.0 - 1.0;
    float h = vUv.y;
    // Densest at the water, thinning upward; soft sides.
    float vertical = exp(-h * 2.4) * smoothstep(0.0, 0.04, h);
    float side = 1.0 - smoothstep(0.35, 1.0, abs(x));
    // Each card breathes with the sets of breakers.
    float pulse = 0.55 + 0.45 * sin(uTime * 0.45 + vSeed.x * 6.2832);
    float d = distance(cameraPosition, vWp);
    float nearFade = smoothstep(2.0, 9.0, d);
    vec3 vdir = normalize(vWp - cameraPosition);
    float fwd = pow(max(dot(vdir, normalize(uAtmoSunDir)), 0.0), 4.0);
    vec3 hazeC = atmoSkyColor(normalize(vec3(vdir.x, vdir.y * 0.2, vdir.z)));
    vec3 col = mix(hazeC, vec3(0.97, 0.98, 1.0), 0.55) * (1.0 + 0.5 * fwd);
    float a = uOpacity * vSeed.y * vertical * side * pulse * nearFade * (0.75 + 1.1 * fwd);
    col = mix(col, hazeC, atmoHazeAmount(cameraPosition, vWp));
    gl_FragColor = vec4(min(col, vec3(1.0)), clamp(a, 0.0, 0.6));
  }
`;

/** Advance the cards' clock (a named operation so the frame loop does not mutate render state inline). */
function tickSpray(material: THREE.ShaderMaterial, timeSec: number): void {
  material.uniforms.uTime.value = timeSec;
}

function seeded(seed: number): () => number {
  let s = seed >>> 0 || 1;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

export const WorldV2Spray: React.FC<WorldV2SprayProps> = ({
  bundle,
  renderSize = 1200,
  heightRatio = 0.14,
  tier = 'high',
}) => {
  const maxCards = tier === 'high' ? 900 : tier === 'medium' ? 450 : 0;
  const heightUnits = renderSize * heightRatio;

  const cards = useMemo(() => {
    if (maxCards === 0) return null;
    const { size, shoreSdfRg } = bundle;
    const unitsPerM = renderSize / (bundle.extentM || WORLD_EXTENT_M);
    const unitsPerMY = heightUnits / ELEV_SPAN_M;
    const seaY = SEA_LEVEL_NORM * heightUnits;
    const rnd = seeded(bundle.seed ^ 0x5a7a);
    const step = Math.max(2, Math.round(size / 340));
    const picks: Array<{ x: number; z: number; w: number; h: number; seed: number; expo: number }> = [];
    for (let gy = 0; gy < size; gy += step) {
      for (let gx = 0; gx < size; gx += step) {
        const u = (gx + 0.5) / size;
        const v = (gy + 0.5) / size;
        if (u < 0.07 || u > 0.93 || v < 0.07 || v > 0.93) continue; // sunk map edge: no surf
        const i = gy * size + gx;
        const sdfM = (shoreSdfRg[i * 2] / 255) * 1000 - 500;
        const expo = shoreSdfRg[i * 2 + 1] / 255;
        if (sdfM > -50 || sdfM < -300 || expo < 0.03) continue;
        if (rnd() > 0.45 + 0.55 * Math.min(1, expo * 3)) continue;
        const ju = (rnd() - 0.5) * step / size;
        const jv = (rnd() - 0.5) * step / size;
        picks.push({
          x: (u + ju - 0.5) * renderSize,
          z: (v + jv - 0.5) * renderSize,
          w: (260 + rnd() * 200) * unitsPerM,
          h: (35 + rnd() * 35 + expo * 30) * unitsPerMY,
          seed: rnd(),
          expo,
        });
      }
    }
    // Keep the most exposed cards when over budget.
    picks.sort((a, b) => b.expo - a.expo);
    const list = picks.slice(0, maxCards);
    if (list.length === 0) return null;
    const matrices = new Float32Array(list.length * 16);
    const seeds = new Float32Array(list.length * 2);
    const m = new THREE.Matrix4();
    for (let k = 0; k < list.length; k += 1) {
      const c = list[k];
      m.makeScale(c.w, c.h, 1).setPosition(c.x, seaY, c.z);
      matrices.set(m.elements, k * 16);
      seeds[k * 2] = c.seed;
      seeds[k * 2 + 1] = 0.45 + 0.55 * Math.min(1, c.expo * 2.5); // sheltered coasts: thinner spray
    }
    const geometry = new THREE.PlaneGeometry(1, 1).translate(0, 0.5, 0);
    geometry.setAttribute('aSeed', new THREE.InstancedBufferAttribute(seeds, 2));
    return { geometry, matrices, count: list.length };
  }, [bundle, renderSize, heightUnits, maxCards]);

  const material = useMemo(
    () =>
      new THREE.ShaderMaterial({
        vertexShader: VERT,
        fragmentShader: FRAG,
        uniforms: {
          uTime: { value: 0 },
          uOpacity: { value: 0.75 },
          ...atmosphereUniforms,
        },
        transparent: true,
        depthWrite: false,
        fog: false,
        side: THREE.DoubleSide,
      }),
    [],
  );

  useFrame((state) => {
    tickSpray(material, sceneElapsed(state.clock));
  });

  useEffect(() => () => cards?.geometry.dispose(), [cards]);
  useEffect(() => () => material.dispose(), [material]);

  if (!cards) return null;
  return (
    <instancedMesh
      name="world-v2-spray"
      args={[cards.geometry, material, cards.count]}
      frustumCulled={false}
      renderOrder={2}
      userData={{ decorative: true }}
      ref={(mesh: THREE.InstancedMesh | null) => {
        if (!mesh || !mesh.instanceMatrix) return;
        mesh.instanceMatrix.array.set(cards.matrices);
        mesh.instanceMatrix.needsUpdate = true;
      }}
    />
  );
};

export default WorldV2Spray;
