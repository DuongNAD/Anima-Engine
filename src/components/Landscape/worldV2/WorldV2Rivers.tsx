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
attribute vec2 aRiverMeta;

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
varying float vLocalGrade;
varying float vStrahler;
varying float vSignedCurv;
varying float vQNorm;
varying float vNearBlend;
varying float vCoverage;
varying float vIsWaterfall;
varying float vQuadU;
varying float vAerialT;

void main() {
  vec3 centerPos = position;
  vec2 sideDir = aRibbon.xy;
  float halfW = aRibbon.z;
  float isWf = aRibbon.w;
  float sideSign = uv.x;
  float strahler = aRiverMeta.x;
  float widthM = halfW * 2.0;
  float isTrunk = max(step(2.5, strahler), step(10.0, widthM));

  vec4 np = sampleNearPatch(centerPos.xz);
  float nearB = (uNearPatchBounds.z > uNearPatchBounds.x &&
    centerPos.x >= uNearPatchBounds.x && centerPos.x <= uNearPatchBounds.z &&
    centerPos.z >= uNearPatchBounds.y && centerPos.z <= uNearPatchBounds.w)
    ? np.b
    : 0.0;

  float camDist = distance(cameraPosition, centerPos);
  float camDistM = camDist * uMPerUnitH;
  float pxWorldM = camDist * uPxPerUnit * uMPerUnitH;
  float trunkRank = clamp((widthM - 4.0) / 22.0 + step(3.5, strahler) * 0.35, 0.0, 1.0);
  float aerialT = smoothstep(180.0, 900.0, camDistM) * (1.0 - nearB);
  float nearSkirt = nearB * clamp(halfW * 0.35, 1.0, 4.0);
  // Trunks expand in aerial/overview views so they read cleanly at 1.5-3 px instead of sub-pixeling.
  float aerialHalfCap = mix(1.05, 1.60, trunkRank);
  float maxTrunkExpand = max(halfW * 2.4, min(165.0, max(halfW * 7.5, pxWorldM * aerialHalfCap)));
  float nonTrunkHalf = min(halfW * 2.4, max(halfW, pxWorldM * 0.92));
  float expandedHalfW = mix(nonTrunkHalf, maxTrunkExpand, isTrunk);
  float geomHalfW = max(halfW + nearSkirt, expandedHalfW);
  float rawCov = clamp(halfW / max(geomHalfW - nearSkirt, 0.01), 0.0, 1.0);
  float trunkCovFloor = mix(0.84, 1.0, trunkRank);
  vCoverage = mix(rawCov, max(trunkCovFloor, pow(rawCov, 0.25)), isTrunk);

  vec3 wp = centerPos;
  wp.xz += sideDir * (geomHalfW * uUnitsPerM);
  float coarseLift = clamp(camDist * 0.0026 + smoothstep(0.08, 0.24, aFlow.y) * 0.22, 0.08, 0.85);
  float aerialLift = aerialT * isTrunk * (uv.y * 0.65 + clamp(camDist * 0.0005, 0.0, 0.25));
  float targetY = centerPos.y + coarseLift + aerialLift;
  if (nearB > 0.001) {
    float steepSlope = smoothstep(0.06, 0.16, max(aFlow.y, aRiverMeta.y));
    float steepBedLift = steepSlope * mix(0.055, 0.035, isTrunk);
    float nearWaterY = max(np.a, np.r + steepBedLift);
    targetY = mix(targetY, nearWaterY, nearB);
  }
  wp.y = targetY;
  // Subtle camera-ray bias on distant trunks prevents coarse terrain z-fighting without moving close water.
  vec3 toCam = normalize(cameraPosition - wp);
  wp += toCam * (aerialT * isTrunk * min(camDist * 0.014, 22.0 * uUnitsPerM));

  vWorldPos = wp;
  vCenterPos = centerPos;
  vAcrossU = sideSign * (geomHalfW / max(halfW, 0.01));
  vQuadU = sideSign;
  vAerialT = aerialT * isTrunk;
  vHalfW = halfW;
  vAlongM = aFlow.x;
  vGrade = aFlow.y;
  vLocalGrade = aRiverMeta.y;
  vStrahler = strahler;
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
uniform float uRiverIdMaskMode;

${NEAR_PATCH_SAMPLER_GLSL}
${ATMOSPHERE_FRAGMENT_PARS}

varying vec3 vWorldPos;
varying vec3 vCenterPos;
varying float vAcrossU;
varying float vHalfW;
varying float vAlongM;
varying float vGrade;
varying float vLocalGrade;
varying float vStrahler;
varying float vSignedCurv;
varying float vQNorm;
varying float vNearBlend;
varying float vCoverage;
varying float vIsWaterfall;
varying float vQuadU;
varying float vAerialT;

vec2 rivHash22(vec2 p) {
  p = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)));
  return fract(sin(p) * 43758.5453);
}

vec2 rivBoulderVoronoi(vec2 p) {
  vec2 ip = floor(p);
  vec2 fp = fract(p);
  float md = 8.0;
  float rid = 0.0;
  for (int j = -1; j <= 1; j++) {
    for (int i = -1; i <= 1; i++) {
      vec2 g = vec2(float(i), float(j));
      vec2 o = rivHash22(ip + g);
      vec2 r = g + (0.18 + 0.64 * o) - fp;
      float d = dot(r, r);
      if (d < md) {
        md = d;
        rid = o.x;
      }
    }
  }
  return vec2(sqrt(md), rid);
}

void main() {
  if (vCenterPos.y < uAtmoSeaY - 0.02 || vWorldPos.y < uAtmoSeaY - 0.02) discard;

  vec2 uvMap = vWorldPos.xz / uRenderSize + 0.5;
  float edgeD = min(min(uvMap.x, 1.0 - uvMap.x), min(uvMap.y, 1.0 - uvMap.y));
  if (edgeD < 0.014 * uEdgeScale) discard;

  float camDist = distance(cameraPosition, vWorldPos);
  float camDistM = camDist * uMPerUnitH;
  float widthM = vHalfW * 2.0;
  float isTrunk = max(step(2.5, vStrahler), step(10.0, widthM));

  // Headwater rills < 5 m fade out cleanly at distance; trunk rivers (>= 10 m or Strahler >= 3)
  // fade smoothly in alpha while staying visible as thin glinting lines from 300 m to overview.
  float maxVisM = mix(widthM * 680.0, 220000.0, smoothstep(5.5, 13.0, widthM));
  maxVisM = max(maxVisM, isTrunk * 220000.0);
  maxVisM = max(maxVisM, smoothstep(0.11, 0.22, vGrade) * 14000.0);
  float trunkRank = clamp((widthM - 4.0) / 22.0 + step(3.5, vStrahler) * 0.35, 0.0, 1.0);
  float distFade = 1.0 - smoothstep(maxVisM * 0.44, maxVisM, camDistM);
  float trunkFloor = mix(0.86, 1.0, trunkRank);
  distFade = max(distFade, isTrunk * trunkFloor);
  distFade *= 1.0 - smoothstep(0.15, 0.85, vIsWaterfall);
  if (distFade < 0.005) discard;

  vec2 worldM = vWorldPos.xz * uMPerUnitH;
  float bendSide = vAcrossU * vSignedCurv;
  float edgeN1 = texture2D(uLookNoise, worldM * 0.015 + vec2(0.13, 0.47)).r - 0.5;
  float edgeN2 = texture2D(uLookNoise, worldM * 0.065 + vec2(0.71, 0.29)).g - 0.5;
  float edgeN3 = texture2D(uLookNoise, worldM * 0.22 + vec2(0.37, 0.83)).b - 0.5;
  float rockyReach = smoothstep(0.065, 0.15, max(vGrade, vLocalGrade));
  float edgeWarp = (edgeN1 * 0.35 + edgeN2 * 0.35 + edgeN3 * 0.30) * mix(0.28, 0.58, rockyReach) * (1.0 - vAerialT * 0.72);
  float insideShrink = max(0.0, bendSide) * 0.12;
  float effHalfRatio = max(0.32, 1.0 - insideShrink + edgeWarp);
  float acrossClip = mix(abs(vAcrossU), abs(vQuadU), vAerialT);
  float edgeDist = effHalfRatio - acrossClip;
  float edgeAA = max(0.055, fwidth(edgeDist) * 1.45);
  float edgeAlpha = smoothstep(-edgeAA, edgeAA, edgeDist);
  if (edgeAlpha < 0.01) discard;

  float physDepthM = 0.0;
  float shoreAlpha = 1.0;
  if (vNearBlend > 0.02) {
    float bedY = sampleNearPatch(vWorldPos.xz).r;
    physDepthM = max(0.0, (vWorldPos.y - bedY) * uDepthMPerUnit);
    float outerBand = smoothstep(0.40, 0.96, abs(vAcrossU));
    shoreAlpha = mix(1.0, smoothstep(-0.10, 0.38 + edgeN2 * 0.14, physDepthM), vNearBlend * outerBand);
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
  float aerialTrunk = isTrunk * smoothstep(180.0, 850.0, camDistM);
  float refl = clamp(schlick * 1.15 + 0.06 + aerialTrunk * mix(0.56, 0.78, trunkRank), 0.0, 0.94);
  vec3 R = reflect(-V, normalize(vec3(N.x * 0.4, 1.0, N.z * 0.4)));
  R.y = abs(R.y);
  vec3 skyRefl = atmoSkyColor(normalize(R));

  float aerialGlint = aerialTrunk * pow(clamp(1.0 - abs(vQuadU) * 0.45, 0.0, 1.0), 1.2);
  float spec = (pow(max(dot(N, H), 0.0), 180.0) * 0.85 * ripFade + pow(max(dot(N, H), 0.0), 16.0) * (0.04 + 0.32 * aerialGlint))
    * cloudSh * cloudSh;
  vec3 litBody = bodyCol * diff;
  vec3 glintSky = mix(skyRefl, max(skyRefl, vec3(0.58, 0.76, 0.94)), 0.42 * aerialGlint);
  float aerialBankEdge = isTrunk * smoothstep(750.0, 2200.0, camDistM) * smoothstep(0.72, 0.98, abs(vQuadU));
  vec3 col = mix(litBody, glintSky, refl) + uAtmoSunColor * spec;
  col = mix(col, vec3(0.025, 0.055, 0.058), aerialBankEdge * 0.28);

  // Steep mountain streams -> mostly dark/clear reflective water between localized step foam & emergent rocks.
  float steep = smoothstep(0.065, 0.175, max(vGrade, vLocalGrade));
  float rockBreakAlpha = 1.0;
  if (steep > 0.01) {
    float bCoarse = texture2D(uLookNoise, worldM * 0.035 + vec2(0.19, 0.63)).r;
    // Distinct step-pool rhythm: ~80% clear dark reflective pools, ~20% narrow aerated drops.
    float stepPhase = vAlongM * 0.095 + bCoarse * 3.2;
    float stepDrop = pow(0.5 + 0.5 * sin(stepPhase), 4.0);
    float gradeDrop = smoothstep(0.10, 0.26, vLocalGrade) * (0.12 + 0.88 * stepDrop);
    float stepIntensity = clamp(gradeDrop + smoothstep(0.18, 0.34, vGrade) * stepDrop * 0.35, 0.0, 1.0);

    // Discrete ~2.3 m cellular boulders in stream space (more along margins, sparse mid-channel).
    float nearRocks = 1.0 - smoothstep(450.0, 1800.0, camDistM);
    float rockPoke = 0.0;
    float rockDome = 0.0;
    float rockWake = 0.0;
    if (nearRocks > 0.01) {
      vec2 bUv = flowUv * 0.42;
      vec2 bv = rivBoulderVoronoi(bUv);
      float bankProb = mix(0.76, 0.44, smoothstep(0.32, 0.90, abs(vAcrossU)));
      float hasRock = step(bankProb, bv.y);
      float rad = mix(0.20, 0.35, fract(bv.y * 7.13));
      rockPoke = (1.0 - smoothstep(rad * 0.68, rad, bv.x)) * hasRock * steep * nearRocks;
      rockDome = clamp(1.0 - bv.x / max(rad, 0.01), 0.0, 1.0);

      // Thin wispy downstream whitewater wake behind emergent boulders only.
      vec2 bvUp = rivBoulderVoronoi((flowUv - vec2(1.25, 0.0)) * 0.42);
      float hasUpRock = step(bankProb, bvUp.y);
      float radUp = mix(0.20, 0.35, fract(bvUp.y * 7.13));
      float wakeMask = (1.0 - smoothstep(radUp * 0.50, radUp * 1.15, bvUp.x)) * hasUpRock * (1.0 - rockPoke);
      rockWake = wakeMask * (0.35 + 0.65 * nB.a) * steep * nearRocks;
    }

    // Flow-elongated froth streaks strictly localized at step drops + thin boulder wakes.
    vec2 streakUv = vec2(vAlongM * 0.14 - p0 * 1.8, vAcrossU * vHalfW * 0.65);
    float streakN = textureGrad(uLookNoise, streakUv, fDx * 0.14, fDy * 0.65).r * 0.60 + nB.a * 0.40;
    float cascadeFoam = stepIntensity * smoothstep(0.46, 0.82, streakN + stepDrop * 0.32);
    float foam = clamp(cascadeFoam * 0.78 + rockWake * (0.32 + 0.38 * stepIntensity), 0.0, 0.82);

    // Clear dark olive-teal/slate reflective mountain pools dominate between steps.
    vec3 poolWaterCol = mix(vec3(0.014, 0.042, 0.052), vec3(0.024, 0.062, 0.070), 1.0 - stepIntensity) * diff;
    vec3 clearMountainWater = mix(poolWaterCol, skyRefl * 0.86, clamp(refl * 1.08, 0.22, 0.86)) + uAtmoSunColor * spec * 0.90;
    vec3 foamCol = vec3(0.78, 0.86, 0.90) * (0.82 + 0.18 * diff);
    vec3 wetRockBase = mix(vec3(0.032, 0.036, 0.040), vec3(0.155, 0.150, 0.140), smoothstep(0.15, 0.85, rockDome));
    vec3 boulderCol = wetRockBase * (0.64 + 0.36 * diff);

    col = mix(col, clearMountainWater, steep * 0.92);
    col = mix(col, foamCol, foam);
    col = mix(col, boulderCol, rockPoke * 0.95);
    rockBreakAlpha = 1.0 - rockPoke * 0.22 * smoothstep(0.55, 0.95, abs(vAcrossU));
  } else if (vIsWaterfall > 0.5) {
    col = mix(col, vec3(0.82, 0.87, 0.90), 0.55);
  }

  float absorb = mix(0.44, 0.97, dClear);
  float alpha = clamp((refl + (1.0 - refl) * absorb) + steep * 0.22 + aerialGlint * 0.42, 0.45, 0.98);
  alpha *= edgeAlpha * mix(shoreAlpha, 1.0, vAerialT) * distFade * rockBreakAlpha * vCoverage;
  if (alpha < 0.01) discard;

  vec3 hdir = normalize(vWorldPos - cameraPosition);
  vec3 hazeC = atmoSkyColor(normalize(vec3(hdir.x, hdir.y * 0.2, hdir.z)));
  float hz = atmoHazeAmount(cameraPosition, vWorldPos) * (1.0 - isTrunk * 0.58);
  col = mix(col, hazeC, hz);

  if (uRiverIdMaskMode > 0.5) {
    gl_FragColor = vec4(isTrunk, alpha, 1.0, 1.0);
    return;
  }
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
        uRiverIdMaskMode: { value: 0 },
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

  useEffect(() => {
    if (meshRef.current) {
      const ud = meshRef.current.userData ?? (meshRef.current.userData = {});
      ud.riverIdUniform = material.uniforms.uRiverIdMaskMode;
    }
  }, [material]);

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
