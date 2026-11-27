// prototype, to be replaced — WV2-LOOK r14 continuous 3D discharge-scaled river ribbon mesh.

import React, { useEffect, useMemo, useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import type { World } from '../utils/worldGen';
import type { QualityTier } from '../utils/qualityTiers';
import { WORLD_EXTENT_M, type WorldV2LoadedBundle } from './types';
import { buildRiverRibbonGeometry } from './riverRibbon';
import { bindNearPatchUniforms, NEAR_PATCH_SAMPLER_GLSL } from './terrainNearPatch';
import { ATMOSPHERE_FRAGMENT_PARS, bindAtmosphereUniforms } from '../materials/atmosphere';
import { buildLookNoiseTexture } from '../utils/worldTerrainTextures';

export interface WorldV2RiversProps {
  bundle: WorldV2LoadedBundle;
  world: World;
  renderSize?: number;
  heightRatio?: number;
  meshResolution?: number;
  tier?: QualityTier;
}

function setUniformValue(u: { value: number }, v: number): void {
  u.value = v;
}

const RIVER_VERTEX_SHADER = /* glsl */ `
attribute vec4 aRibbon;
attribute vec4 aFlow;

uniform float uUnitsPerM;
uniform float uMPerUnitH;
uniform float uPxPerUnit;

${NEAR_PATCH_SAMPLER_GLSL}

varying vec3 vWorldPos;
varying vec3 vCenterPos;
varying float vAcrossU;
varying float vHalfW;
varying float vAlongM;
varying float vGrade;
varying float vSignedCurv;
varying float vQNorm;
varying float vNearBlend;
varying float vCoverage;
varying float vIsWaterfall;

void main() {
  vec3 centerPos = position;
  vec2 sideDir = aRibbon.xy;
  float halfW = aRibbon.z;
  float isWf = aRibbon.w;
  float sideSign = uv.x;

  vec4 np = sampleNearPatch(centerPos.xz);
  float nearB = (uNearPatchBounds.z > uNearPatchBounds.x &&
    centerPos.x >= uNearPatchBounds.x && centerPos.x <= uNearPatchBounds.z &&
    centerPos.z >= uNearPatchBounds.y && centerPos.z <= uNearPatchBounds.w)
    ? np.b
    : 0.0;

  float camDist = distance(cameraPosition, centerPos);
  float pxWorldM = camDist * uPxPerUnit * uMPerUnitH;
  float nearSkirt = nearB * clamp(halfW * 0.35, 1.0, 4.0);
  float geomHalfW = max(halfW + nearSkirt, min(halfW * 2.4, pxWorldM * 0.80));
  vCoverage = clamp(halfW / max(geomHalfW - nearSkirt, 0.01), 0.0, 1.0);

  vec3 wp = centerPos;
  wp.xz += sideDir * (geomHalfW * uUnitsPerM);
  float coarseLift = clamp(camDist * 0.0022, 0.07, 0.65);
  float targetY = centerPos.y + coarseLift;
  if (nearB > 0.001) {
    float steepBedLift = smoothstep(0.06, 0.16, aFlow.y) * 0.03;
    float nearWaterY = max(np.a, np.r + steepBedLift);
    targetY = mix(targetY, nearWaterY, nearB);
  }
  wp.y = targetY;

  vWorldPos = wp;
  vCenterPos = centerPos;
  vAcrossU = sideSign * (geomHalfW / max(halfW, 0.01));
  vHalfW = halfW;
  vAlongM = aFlow.x;
  vGrade = aFlow.y;
  vSignedCurv = aFlow.z;
  vQNorm = aFlow.w;
  vNearBlend = nearB;
  vIsWaterfall = isWf;

  gl_Position = projectionMatrix * viewMatrix * vec4(wp, 1.0);
}
`;

const RIVER_FRAGMENT_SHADER = /* glsl */ `
precision highp float;

uniform float uTime;
uniform float uMPerUnitH;
uniform float uDepthMPerUnit;
uniform float uEdgeScale;
uniform float uRenderSize;
uniform sampler2D uLookNoise;

${NEAR_PATCH_SAMPLER_GLSL}
${ATMOSPHERE_FRAGMENT_PARS}

varying vec3 vWorldPos;
varying vec3 vCenterPos;
varying float vAcrossU;
varying float vHalfW;
varying float vAlongM;
varying float vGrade;
varying float vSignedCurv;
varying float vQNorm;
varying float vNearBlend;
varying float vCoverage;
varying float vIsWaterfall;

void main() {
  if (vCenterPos.y < uAtmoSeaY - 0.02 || vWorldPos.y < uAtmoSeaY - 0.02) discard;

  vec2 uvMap = vWorldPos.xz / uRenderSize + 0.5;
  float edgeD = min(min(uvMap.x, 1.0 - uvMap.x), min(uvMap.y, 1.0 - uvMap.y));
  if (edgeD < 0.014 * uEdgeScale) discard;

  float camDist = distance(cameraPosition, vWorldPos);
  float camDistM = camDist * uMPerUnitH;
  float widthM = vHalfW * 2.0;

  // Distance fade: tiny headwater threads recede before overview so only true rivers read afar.
  float maxVisM = mix(widthM * 850.0, 220000.0, smoothstep(6.0, 14.0, widthM));
  maxVisM = max(maxVisM, smoothstep(0.11, 0.22, vGrade) * 11000.0);
  float distFade = 1.0 - smoothstep(maxVisM * 0.42, maxVisM, camDistM);
  distFade *= 1.0 - smoothstep(0.15, 0.85, vIsWaterfall);
  if (distFade < 0.005) discard;

  vec2 worldM = vWorldPos.xz * uMPerUnitH;
  float bendSide = vAcrossU * vSignedCurv;
  float edgeWarp = (texture2D(uLookNoise, worldM * 0.012).r - 0.5) * 0.24
    + (texture2D(uLookNoise, worldM * 0.045).g - 0.5) * 0.12;
  float insideShrink = max(0.0, bendSide) * 0.16;
  float effHalfRatio = max(0.35, 1.0 - insideShrink + edgeWarp);
  float edgeDist = effHalfRatio - abs(vAcrossU);
  float edgeAA = max(0.02, fwidth(edgeDist) * 1.25);
  float edgeAlpha = smoothstep(-edgeAA, edgeAA, edgeDist);
  if (edgeAlpha < 0.01) discard;

  float physDepthM = 0.0;
  float shoreAlpha = 1.0;
  if (vNearBlend > 0.02) {
    float bedY = sampleNearPatch(vWorldPos.xz).r;
    physDepthM = max(0.0, (vWorldPos.y - bedY) * uDepthMPerUnit);
    float outerBand = smoothstep(0.55, 0.96, abs(vAcrossU));
    shoreAlpha = mix(1.0, smoothstep(-0.08, 0.18, physDepthM), vNearBlend * outerBand);
  }
  if (shoreAlpha < 0.01) discard;

  float thalwegU = clamp(vAcrossU + vSignedCurv * 0.32, -1.0, 1.0);
  float analyticDepth = max(0.0, 1.0 - thalwegU * thalwegU) * clamp(0.45 + sqrt(widthM) * 0.42, 0.5, 3.2);
  float depthM = vNearBlend > 0.25 ? mix(analyticDepth, physDepthM, vNearBlend * 0.72) : analyticDepth;

  // River body palette: light warm olive/amber shallows -> teal mid -> deep river blue-slate in thalweg.
  vec3 shallowCol = vec3(0.18, 0.20, 0.13);
  vec3 midCol = vec3(0.055, 0.145, 0.150);
  vec3 deepCol = vec3(0.018, 0.072, 0.125);
  float dClear = 1.0 - exp(-depthM / 0.85);
  float dDeep = 1.0 - exp(-depthM / 2.2);
  vec3 bodyCol = mix(mix(shallowCol, midCol, dClear), deepCol, dDeep);

  // Filtered mipmapped flowing ripples (along-across with hardware derivatives -> zero striping).
  float flowSpeed = mix(0.45, 1.9, smoothstep(0.01, 0.14, vGrade)) + vQNorm * 0.25;
  vec2 flowUv = vec2(vAlongM, vAcrossU * vHalfW);
  vec2 fDx = clamp(dFdx(flowUv), vec2(-20.0), vec2(20.0));
  vec2 fDy = clamp(dFdy(flowUv), vec2(-20.0), vec2(20.0));

  float p0 = fract(uTime * flowSpeed * 0.32);
  float p1 = fract(p0 + 0.5);
  float tri = abs(2.0 * p0 - 1.0);
  vec2 sA0 = flowUv * vec2(0.055, 0.085) - vec2(p0 * 1.4, 0.0);
  vec2 sA1 = flowUv * vec2(0.055, 0.085) - vec2(p1 * 1.4, 0.37);
  vec2 sB0 = flowUv * vec2(0.16, 0.24) - vec2(p0 * 2.1, 0.21);
  vec2 sB1 = flowUv * vec2(0.16, 0.24) - vec2(p1 * 2.1, 0.59);

  vec4 nA = mix(
    textureGrad(uLookNoise, sA0, fDx * 0.055, fDy * 0.055),
    textureGrad(uLookNoise, sA1, fDx * 0.055, fDy * 0.055),
    tri
  );
  vec4 nB = mix(
    textureGrad(uLookNoise, sB0, fDx * 0.16, fDy * 0.16),
    textureGrad(uLookNoise, sB1, fDx * 0.16, fDy * 0.16),
    tri
  );
  float ripFade = 1.0 - smoothstep(140.0, 950.0, camDistM);
  vec2 ripTilt = ((nA.rg - 0.5) * 0.16 + (nB.gb - 0.5) * 0.10) * ripFade;
  vec3 N = normalize(vec3(ripTilt.x, 1.0, ripTilt.y));

  vec3 V = normalize(cameraPosition - vWorldPos);
  vec3 sdir = normalize(uAtmoSunDir);
  vec3 H = normalize(sdir + V);
  float diff = clamp(dot(N, sdir) * 0.40 + 0.65, 0.0, 1.0);
  float cloudSh = atmoCloudShadow(vWorldPos);
  diff *= mix(0.72, 1.0, cloudSh);

  float schlick = 0.02 + 0.98 * pow(1.0 - max(dot(N, V), 0.0), 5.0);
  float refl = clamp(schlick * 1.15 + 0.04, 0.0, 0.82);
  vec3 R = reflect(-V, normalize(vec3(N.x * 0.4, 1.0, N.z * 0.4)));
  R.y = abs(R.y);
  vec3 skyRefl = atmoSkyColor(normalize(R));

  float spec = (pow(max(dot(N, H), 0.0), 180.0) * 0.85 * ripFade + pow(max(dot(N, H), 0.0), 24.0) * 0.04)
    * cloudSh * cloudSh;
  vec3 litBody = bodyCol * diff;
  vec3 col = mix(litBody, skyRefl, refl) + uAtmoSunColor * spec;

  // Steep mountain reaches -> broken white-water rapids over wet boulders (never a solid blue ribbon!)
  float steep = smoothstep(0.085, 0.19, vGrade);
  float rockBreakAlpha = 1.0;
  if (steep > 0.01) {
    float foamNoise = nA.b * 0.55 + nB.a * 0.45;
    float boulderN = texture2D(uLookNoise, worldM * 0.085 + vec2(0.41, 0.17)).r * 0.6
      + texture2D(uLookNoise, worldM * 0.26).g * 0.4;
    float nearRocks = 1.0 - smoothstep(800.0, 3200.0, camDistM);
    float rockPoke = smoothstep(0.54, 0.76, boulderN) * steep * nearRocks;
    float rockCollar = smoothstep(0.40, 0.56, boulderN) * (1.0 - rockPoke) * steep * 0.88;
    float foam = clamp(steep * (0.38 + 0.85 * foamNoise) + rockCollar, 0.0, 0.96);
    vec3 rapidWaterCol = mix(col, vec3(0.14, 0.24, 0.25) * diff, steep * 0.55);
    vec3 foamCol = vec3(0.86, 0.90, 0.93) * (0.80 + 0.20 * diff);
    vec3 boulderCol = vec3(0.08, 0.078, 0.075) * (0.65 + 0.35 * diff);
    col = mix(rapidWaterCol, foamCol, foam);
    col = mix(col, boulderCol, rockPoke * 0.88);
    rockBreakAlpha = 1.0 - rockPoke * 0.55 * smoothstep(0.35, 0.92, abs(vAcrossU));
  } else if (vIsWaterfall > 0.5) {
    col = mix(col, vec3(0.82, 0.87, 0.90), 0.55);
  }

  float absorb = mix(0.42, 0.97, dClear);
  float alpha = clamp((refl + (1.0 - refl) * absorb) + steep * 0.24, 0.40, 0.97);
  alpha *= edgeAlpha * shoreAlpha * distFade * rockBreakAlpha * vCoverage;
  if (alpha < 0.01) discard;

  vec3 hdir = normalize(vWorldPos - cameraPosition);
  vec3 hazeC = atmoSkyColor(normalize(vec3(hdir.x, hdir.y * 0.2, hdir.z)));
  float hz = atmoHazeAmount(cameraPosition, vWorldPos);
  col = mix(col, hazeC, hz);

  gl_FragColor = vec4(col, alpha);
}
`;

export const WorldV2Rivers: React.FC<WorldV2RiversProps> = ({
  bundle,
  world,
  renderSize = 2400,
  heightRatio = 0.07,
  meshResolution = 512,
  tier = 'high',
}) => {
  const heightUnits = renderSize * heightRatio;
  const carveBanks = tier !== 'low';

  const geometry = useMemo(
    () => buildRiverRibbonGeometry(bundle, world, renderSize, heightUnits, meshResolution, carveBanks),
    [bundle, world, renderSize, heightUnits, meshResolution, carveBanks],
  );

  const lookNoiseTex = useMemo(() => buildLookNoiseTexture(), []);
  const timeUniform = useMemo(() => ({ value: 0 }), []);
  const pxUniform = useMemo(() => ({ value: 0.0014 }), []);

  const material = useMemo(() => {
    const extentM = bundle.extentM || WORLD_EXTENT_M;
    const mat = new THREE.ShaderMaterial({
      vertexShader: RIVER_VERTEX_SHADER,
      fragmentShader: RIVER_FRAGMENT_SHADER,
      uniforms: {
        uTime: timeUniform,
        uUnitsPerM: { value: renderSize / extentM },
        uMPerUnitH: { value: extentM / renderSize },
        uDepthMPerUnit: { value: 7200 / Math.max(1e-6, heightUnits) },
        uPxPerUnit: pxUniform,
        uEdgeScale: { value: 40960 / extentM },
        uRenderSize: { value: renderSize },
        uLookNoise: { value: lookNoiseTex },
      },
      transparent: true,
      depthWrite: false,
      polygonOffset: true,
      polygonOffsetFactor: -2,
      polygonOffsetUnits: -2,
      fog: false,
    });
    bindAtmosphereUniforms(mat.uniforms);
    bindNearPatchUniforms(mat.uniforms);
    return mat;
  }, [bundle.extentM, renderSize, heightUnits, lookNoiseTex, timeUniform, pxUniform]);

  const meshRef = useRef<THREE.Mesh>(null);

  useFrame((state) => {
    setUniformValue(timeUniform, state.clock.elapsedTime);
    const cam = state.camera as THREE.PerspectiveCamera;
    const fovRad = ((cam.fov || 55) * Math.PI) / 180;
    const viewH = Math.max(1, state.size.height);
    setUniformValue(pxUniform, (2 * Math.tan(fovRad * 0.5)) / viewH);
  });

  useEffect(() => () => geometry.dispose(), [geometry]);
  useEffect(() => () => lookNoiseTex.dispose(), [lookNoiseTex]);
  useEffect(() => () => material.dispose(), [material]);

  return (
    <mesh
      ref={meshRef}
      name="world-v2-rivers"
      geometry={geometry}
      material={material}
      frustumCulled={false}
      renderOrder={1}
    />
  );
};

export default WorldV2Rivers;
