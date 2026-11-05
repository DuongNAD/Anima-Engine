import * as THREE from 'three';

// -------------------------------------------------------------------------------------------------
// Map 2 Terrain Shaders: Triplanar Mapping, Ecotone Multi-Splatting, Stochastic Noise & Crystal Glow
// -------------------------------------------------------------------------------------------------

export const MAP2_TERRAIN_VERTEX_SHADER = /* glsl */ `
#include <common>
#include <fog_pars_vertex>
#include <shadowmap_pars_vertex>

attribute vec3 aBiomeWeights1; // Biomes 0-2 (Snow, Rock, Plains)
attribute vec3 aBiomeWeights2; // Biomes 3-5 (Rainforest, Savanna, Desert)
attribute vec2 aBiomeWeights3; // Biomes 6-7 (Hydrology, Caves)

varying vec3 vWorldPosition;
varying vec3 vNormal;
varying vec2 vUv;
varying vec3 vBiomeWeights1;
varying vec3 vBiomeWeights2;
varying vec2 vBiomeWeights3;
varying float vElevation;

void main() {
  vUv = uv;
  vBiomeWeights1 = aBiomeWeights1;
  vBiomeWeights2 = aBiomeWeights2;
  vBiomeWeights3 = aBiomeWeights3;
  vElevation = position.y;

  vec3 transformed = vec3(position);
  vec3 objectNormal = vec3(normal);
  vec3 transformedNormal = normalMatrix * objectNormal;
  vec4 worldPosition = modelMatrix * vec4(transformed, 1.0);
  vWorldPosition = worldPosition.xyz;
  vNormal = normalize(transformedNormal);

  vec4 mvPosition = viewMatrix * worldPosition;
  gl_Position = projectionMatrix * mvPosition;

  #include <fog_vertex>
  #include <shadowmap_vertex>
}
`;

export const MAP2_TERRAIN_FRAGMENT_SHADER = /* glsl */ `
#include <common>
#include <fog_pars_fragment>
#include <shadowmap_pars_fragment>

uniform float uTime;
uniform vec3 uSunDir;
uniform vec3 uSunColor;
uniform vec3 uAmbientColor;
uniform float uWaterLevel;
uniform float uSnowLine;
uniform float uCliffThreshold;

// Biome Colors
uniform vec3 uColorSnow;
uniform vec3 uColorRock;
uniform vec3 uColorPlains;
uniform vec3 uColorRainforest;
uniform vec3 uColorSavanna;
uniform vec3 uColorDesert;
uniform vec3 uColorHydrology;
uniform vec3 uColorCave;

// Crystal Cave Glow
uniform vec3 uCrystalColorA;
uniform vec3 uCrystalColorB;
uniform float uCrystalIntensity;

varying vec3 vWorldPosition;
varying vec3 vNormal;
varying vec2 vUv;
varying vec3 vBiomeWeights1;
varying vec3 vBiomeWeights2;
varying vec2 vBiomeWeights3;
varying float vElevation;

// 2D Hash function for stochastic anti-tiling
float hash21(vec2 p) {
  p = fract(p * vec2(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

// Procedural 2D Value Noise
float valueNoise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  f = f * f * (3.0 - 2.0 * f);

  float a = hash21(i);
  float b = hash21(i + vec2(1.0, 0.0));
  float c = hash21(i + vec2(0.0, 1.0));
  float d = hash21(i + vec2(1.0, 1.0));

  return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

// Multi-octave FBM in GLSL
float glslFBM(vec2 p) {
  float v = 0.0;
  float a = 0.5;
  mat2 rot = mat2(0.87, 0.5, -0.5, 0.87);
  for (int i = 0; i < 4; i++) {
    v += a * valueNoise(p);
    p = rot * p * 2.0 + vec2(100.0);
    a *= 0.5;
  }
  return v;
}

// Stochastic Anti-Tiling Color Modulation
vec3 applyStochasticAntiTiling(vec3 baseColor, vec2 worldCoord, float scale) {
  float n1 = glslFBM(worldCoord * scale);
  float n2 = glslFBM(worldCoord * (scale * 2.3) + vec2(5.2, 1.3));
  float modFactor = 0.85 + 0.3 * (n1 * 0.7 + n2 * 0.3);
  return baseColor * modFactor;
}

// Triplanar Projection for Rock & Cliff Strata
vec3 getTriplanarRockColor(vec3 worldPos, vec3 normal, vec3 rockBaseColor) {
  vec3 blendWeights = pow(abs(normal), vec3(6.0));
  blendWeights /= (blendWeights.x + blendWeights.y + blendWeights.z + 1e-5);

  // High-frequency rock striation along vertical planes
  float striationY = sin(worldPos.y * 1.5 + glslFBM(worldPos.xz * 0.1) * 3.0) * 0.15;
  float grainX = glslFBM(worldPos.yz * 0.4);
  float grainZ = glslFBM(worldPos.xy * 0.4);
  float grainY = glslFBM(worldPos.xz * 0.4);

  vec3 rockX = rockBaseColor * (0.8 + 0.4 * grainX + striationY);
  vec3 rockY = rockBaseColor * (0.9 + 0.3 * grainY);
  vec3 rockZ = rockBaseColor * (0.8 + 0.4 * grainZ + striationY);

  return rockX * blendWeights.x + rockY * blendWeights.y + rockZ * blendWeights.z;
}

void main() {
  vec3 norm = normalize(vNormal);
  vec2 worldXZ = vWorldPosition.xz;
  float steepness = clamp(1.0 - norm.y, 0.0, 1.0);

  // 1. Evaluate Procedural Biome Colors with Stochastic Anti-Tiling
  vec3 snowCol = applyStochasticAntiTiling(uColorSnow, worldXZ, 0.25);
  // Add slight crystalline shimmer to snow
  float snowSparkle = pow(glslFBM(worldXZ * 1.8), 4.0) * 0.25;
  snowCol += vec3(snowSparkle);

  vec3 rockCol = getTriplanarRockColor(vWorldPosition, norm, uColorRock);
  vec3 plainsCol = applyStochasticAntiTiling(uColorPlains, worldXZ, 0.15);
  vec3 rainforestCol = applyStochasticAntiTiling(uColorRainforest, worldXZ, 0.18);
  vec3 savannaCol = applyStochasticAntiTiling(uColorSavanna, worldXZ, 0.12);

  // Desert Dunes with micro-dune wind ripples
  float duneRipples = sin(vWorldPosition.x * 1.8 + vWorldPosition.z * 0.9) * 0.06;
  vec3 desertCol = applyStochasticAntiTiling(uColorDesert, worldXZ, 0.1) + vec3(duneRipples);

  vec3 hydrologyCol = applyStochasticAntiTiling(uColorHydrology, worldXZ, 0.3);

  // Bioluminescent Cave with Pulsing Crystal Veins
  float pulse = 0.5 + 0.5 * sin(uTime * 2.2 + dot(worldXZ, vec2(0.25, 0.35)));
  vec3 crystalGlow = mix(uCrystalColorA, uCrystalColorB, pulse);
  float crystalVeins = pow(clamp(glslFBM(worldXZ * 0.8 + vec2(uTime * 0.05)), 0.0, 1.0), 3.0);
  vec3 caveCol = mix(uColorCave, crystalGlow, crystalVeins * uCrystalIntensity * 0.85);

  // 2. Continuous Soft-Max Biome Splatting Blending
  float w0 = vBiomeWeights1.x; // Snow
  float w1 = vBiomeWeights1.y; // Rock
  float w2 = vBiomeWeights1.z; // Plains
  float w3 = vBiomeWeights2.x; // Rainforest
  float w4 = vBiomeWeights2.y; // Savanna
  float w5 = vBiomeWeights2.z; // Desert
  float w6 = vBiomeWeights3.x; // Hydrology
  float w7 = vBiomeWeights3.y; // Caves

  // Enforce triplanar rock blending on extreme cliffs
  if (steepness > 0.45) {
    float cliffOverride = smoothstep(0.45, 0.80, steepness);
    w1 = mix(w1, 1.0, cliffOverride);
    float totalNonRock = w0 + w2 + w3 + w4 + w5 + w6 + w7 + 1e-5;
    float scale = (1.0 - w1) / totalNonRock;
    w0 *= scale;
    w2 *= scale;
    w3 *= scale;
    w4 *= scale;
    w5 *= scale;
    w6 *= scale;
    w7 *= scale;
  }

  vec3 diffuseColor =
    snowCol * w0 +
    rockCol * w1 +
    plainsCol * w2 +
    rainforestCol * w3 +
    savannaCol * w4 +
    desertCol * w5 +
    hydrologyCol * w6 +
    caveCol * w7;

  // Add emissive glow for crystal veins
  vec3 emissiveGlow = crystalGlow * (w7 * crystalVeins * uCrystalIntensity * 1.2);

  // 3. Direct Directional Sun Lighting + Ambient GI Approximation
  vec3 lightDir = normalize(uSunDir);
  float NdotL = max(dot(norm, lightDir), 0.0);
  
  // Half-lambert wrap lighting for soft organic appearance
  float wrapLight = (dot(norm, lightDir) * 0.5 + 0.5);
  vec3 directLight = uSunColor * (NdotL * 0.85 + wrapLight * 0.15);
  vec3 totalLight = uAmbientColor + directLight;

  vec3 finalColor = diffuseColor * totalLight + emissiveGlow;

  gl_FragColor = vec4(finalColor, 1.0);

  #include <fog_fragment>
}
`;

export interface Map2TerrainShaderUniforms {
  uTime: { value: number };
  uSunDir: { value: THREE.Vector3 };
  uSunColor: { value: THREE.Color };
  uAmbientColor: { value: THREE.Color };
  uWaterLevel: { value: number };
  uSnowLine: { value: number };
  uCliffThreshold: { value: number };
  uColorSnow: { value: THREE.Color };
  uColorRock: { value: THREE.Color };
  uColorPlains: { value: THREE.Color };
  uColorRainforest: { value: THREE.Color };
  uColorSavanna: { value: THREE.Color };
  uColorDesert: { value: THREE.Color };
  uColorHydrology: { value: THREE.Color };
  uColorCave: { value: THREE.Color };
  uCrystalColorA: { value: THREE.Color };
  uCrystalColorB: { value: THREE.Color };
  uCrystalIntensity: { value: number };
}

export function createMap2TerrainMaterial(
  customUniforms?: Partial<Map2TerrainShaderUniforms>,
): THREE.ShaderMaterial {
  const uniforms: Map2TerrainShaderUniforms = {
    ...THREE.UniformsUtils.clone(THREE.UniformsLib.fog),
    uTime: { value: 0 },
    uSunDir: { value: new THREE.Vector3(120, 180, 80).normalize() },
    uSunColor: { value: new THREE.Color('#fff7ed') },
    uAmbientColor: { value: new THREE.Color('#334155') },
    uWaterLevel: { value: 4.0 },
    uSnowLine: { value: 72.0 },
    uCliffThreshold: { value: 0.62 },
    uColorSnow: { value: new THREE.Color('#f8fbff') },
    uColorRock: { value: new THREE.Color('#4a4742') },
    uColorPlains: { value: new THREE.Color('#5ea844') },
    uColorRainforest: { value: new THREE.Color('#146328') },
    uColorSavanna: { value: new THREE.Color('#cbb25b') },
    uColorDesert: { value: new THREE.Color('#e8ba60') },
    uColorHydrology: { value: new THREE.Color('#3a84a7') },
    uColorCave: { value: new THREE.Color('#1e1b2e') },
    uCrystalColorA: { value: new THREE.Color('#9d4edd') },
    uCrystalColorB: { value: new THREE.Color('#00f5d4') },
    uCrystalIntensity: { value: 1.0 },
    ...customUniforms,
  };

  const mat = new THREE.ShaderMaterial({
    vertexShader: MAP2_TERRAIN_VERTEX_SHADER,
    fragmentShader: MAP2_TERRAIN_FRAGMENT_SHADER,
    uniforms: uniforms as unknown as { [uniform: string]: THREE.IUniform },
    fog: true,
    lights: false,
    side: THREE.DoubleSide,
  });

  return mat;
}

// -------------------------------------------------------------------------------------------------
// Map 2 Atmosphere Shaders: Physical Rayleigh & Mie Scattering Sky Dome
// -------------------------------------------------------------------------------------------------

export const MAP2_SKY_VERTEX_SHADER = /* glsl */ `
varying vec3 vWorldPosition;
varying vec3 vRayDirection;

void main() {
  vec4 worldPos = modelMatrix * vec4(position, 1.0);
  vWorldPosition = worldPos.xyz;
  vRayDirection = normalize(worldPos.xyz);
  
  // Keep sky dome centered at camera position
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

export const MAP2_SKY_FRAGMENT_SHADER = /* glsl */ `
uniform vec3 uSunDir;
uniform vec3 uSunColor;
uniform vec3 uMoonDir;
uniform vec3 uMoonColor;
uniform vec3 uZenithColor;
uniform vec3 uHorizonColor;
uniform vec3 uGroundColor;
uniform float uSunIntensity;
uniform float uMoonIntensity;
uniform float uMieG;
uniform float uRayleighDensity;
uniform float uTime;

varying vec3 vWorldPosition;
varying vec3 vRayDirection;

// Rayleigh Phase Function: P_R(theta) = 3/(16*pi) * (1 + cos^2(theta))
float rayleighPhase(float cosTheta) {
  return 0.0596831 * (1.0 + cosTheta * cosTheta);
}

// Henyey-Greenstein Mie Phase Function: P_M(theta) = 1/(4*pi) * (1 - g^2) / (1 + g^2 - 2g*cos(theta))^(1.5)
float miePhase(float cosTheta, float g) {
  float g2 = g * g;
  float denom = 1.0 + g2 - 2.0 * g * cosTheta;
  return 0.0795775 * (1.0 - g2) / (pow(max(denom, 0.0001), 1.5));
}

void main() {
  vec3 V = normalize(vRayDirection);
  vec3 L_sun = normalize(uSunDir);
  vec3 L_moon = normalize(uMoonDir);

  float cosThetaSun = dot(V, L_sun);
  float cosThetaMoon = dot(V, L_moon);

  // Altitude factor: 0.0 at horizon, 1.0 at zenith, negative below horizon
  float altitude = V.y;

  // Atmospheric gradient interpolation
  vec3 skyBase;
  if (altitude >= 0.0) {
    // Upper hemisphere: Horizon to Zenith gradient with smooth power curve
    float hFactor = pow(clamp(altitude, 0.0, 1.0), 0.55);
    skyBase = mix(uHorizonColor, uZenithColor, hFactor);
  } else {
    // Lower hemisphere (beneath horizon)
    float gFactor = pow(clamp(-altitude * 2.0, 0.0, 1.0), 0.7);
    skyBase = mix(uHorizonColor, uGroundColor, gFactor);
  }

  // 1. Rayleigh single scattering approximation
  float phaseR = rayleighPhase(cosThetaSun) * uRayleighDensity;
  vec3 rayleighColor = uSunColor * phaseR * uSunIntensity;

  // 2. Mie forward scattering solar halo
  float phaseM = miePhase(cosThetaSun, uMieG);
  vec3 mieColor = uSunColor * phaseM * (uSunIntensity * 1.8);

  // 3. Sharp Solar Disc
  float sunDisc = smoothstep(0.9995, 0.9999, cosThetaSun) * uSunIntensity * 6.0;
  vec3 sunDiscColor = uSunColor * sunDisc;

  // 4. Lunar Disc and Cold Glow
  float moonDisc = smoothstep(0.9985, 0.9996, cosThetaMoon) * uMoonIntensity * 2.5;
  float moonHalo = miePhase(cosThetaMoon, 0.85) * uMoonIntensity * 0.4;
  vec3 moonColor = uMoonColor * (moonDisc + moonHalo);

  // Combine components
  vec3 finalColor = skyBase + rayleighColor + mieColor + sunDiscColor + moonColor;

  gl_FragColor = vec4(finalColor, 1.0);
}
`;

export interface Map2SkyShaderUniforms {
  uSunDir: { value: THREE.Vector3 };
  uSunColor: { value: THREE.Color };
  uMoonDir: { value: THREE.Vector3 };
  uMoonColor: { value: THREE.Color };
  uZenithColor: { value: THREE.Color };
  uHorizonColor: { value: THREE.Color };
  uGroundColor: { value: THREE.Color };
  uSunIntensity: { value: number };
  uMoonIntensity: { value: number };
  uMieG: { value: number };
  uRayleighDensity: { value: number };
  uTime: { value: number };
}

export function createMap2SkyMaterial(
  customUniforms?: Partial<Map2SkyShaderUniforms>,
): THREE.ShaderMaterial {
  const uniforms: Map2SkyShaderUniforms = {
    uSunDir: { value: new THREE.Vector3(0, 1, 0) },
    uSunColor: { value: new THREE.Color('#fff4e6') },
    uMoonDir: { value: new THREE.Vector3(0, -1, 0) },
    uMoonColor: { value: new THREE.Color('#8fa8d6') },
    uZenithColor: { value: new THREE.Color('#0284c7') },
    uHorizonColor: { value: new THREE.Color('#bae6fd') },
    uGroundColor: { value: new THREE.Color('#1e293b') },
    uSunIntensity: { value: 1.0 },
    uMoonIntensity: { value: 0.0 },
    uMieG: { value: 0.76 },
    uRayleighDensity: { value: 1.0 },
    uTime: { value: 0 },
    ...customUniforms,
  };

  return new THREE.ShaderMaterial({
    vertexShader: MAP2_SKY_VERTEX_SHADER,
    fragmentShader: MAP2_SKY_FRAGMENT_SHADER,
    uniforms: uniforms as unknown as { [uniform: string]: THREE.IUniform },
    side: THREE.BackSide,
    depthWrite: false,
    fog: false,
  });
}

// -------------------------------------------------------------------------------------------------
// Map 2 Starfield Shaders: Procedural Twinkling Nocturnal Celestial Sphere
// -------------------------------------------------------------------------------------------------

export const MAP2_STARFIELD_VERTEX_SHADER = /* glsl */ `
attribute float aSize;
attribute float aPhase;
attribute float aTwinkleSpeed;
attribute vec3 aStarColor;

uniform float uTime;
uniform float uOpacity;

varying vec3 vStarColor;
varying float vAlpha;

void main() {
  vStarColor = aStarColor;
  
  // Twinkle brightness modulation
  float twinkle = sin(uTime * aTwinkleSpeed + aPhase) * 0.5 + 0.5;
  vAlpha = uOpacity * (0.4 + 0.6 * twinkle);

  vec4 mvPosition = modelViewMatrix * vec4(position, 1.0);
  gl_PointSize = aSize * (300.0 / -mvPosition.z) * (0.8 + 0.4 * twinkle);
  gl_Position = projectionMatrix * mvPosition;
}
`;

export const MAP2_STARFIELD_FRAGMENT_SHADER = /* glsl */ `
varying vec3 vStarColor;
varying float vAlpha;

void main() {
  if (vAlpha <= 0.001) discard;

  // Soft circular point shape
  vec2 coord = gl_PointCoord - vec2(0.5);
  float distSq = dot(coord, coord);
  if (distSq > 0.25) discard;

  float radialGlow = 1.0 - smoothstep(0.0, 0.25, distSq);
  float coreSpark = 1.0 - smoothstep(0.0, 0.05, distSq);

  vec3 col = vStarColor + vec3(coreSpark * 0.5);
  float alpha = vAlpha * radialGlow;

  gl_FragColor = vec4(col, alpha);
}
`;

export interface Map2StarfieldShaderUniforms {
  uTime: { value: number };
  uOpacity: { value: number };
}

export function createMap2StarfieldMaterial(
  customUniforms?: Partial<Map2StarfieldShaderUniforms>,
): THREE.ShaderMaterial {
  const uniforms: Map2StarfieldShaderUniforms = {
    uTime: { value: 0 },
    uOpacity: { value: 0 },
    ...customUniforms,
  };

  return new THREE.ShaderMaterial({
    vertexShader: MAP2_STARFIELD_VERTEX_SHADER,
    fragmentShader: MAP2_STARFIELD_FRAGMENT_SHADER,
    uniforms: uniforms as unknown as { [uniform: string]: THREE.IUniform },
    transparent: true,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
    fog: false,
  });
}

// -------------------------------------------------------------------------------------------------
// Map 2 Water Shaders: Multi-Octave Gerstner Waves, Beer-Lambert Extinction, Shoreline Foam & Fresnel
// -------------------------------------------------------------------------------------------------

export const MAP2_WATER_VERTEX_SHADER = /* glsl */ `
#include <common>
#include <fog_pars_vertex>
#include <shadowmap_pars_vertex>

uniform float uTime;
uniform float uSeaLevel;

// 4 Octaves of Gerstner Waves
// Each wave: vec4(dirX, dirZ, amplitude, wavelength), vec2(speed, steepness)
uniform vec4 uWave0;
uniform vec2 uWaveParams0;
uniform vec4 uWave1;
uniform vec2 uWaveParams1;
uniform vec4 uWave2;
uniform vec2 uWaveParams2;
uniform vec4 uWave3;
uniform vec2 uWaveParams3;

varying vec3 vWorldPosition;
varying vec3 vNormal;
varying vec3 vViewPosition;
varying vec2 vWaterUv;
varying float vWaveCrest;

// Gerstner Wave displacement and normal calculation step
void evaluateGerstnerWave(
  vec4 wave,
  vec2 params,
  vec3 pos,
  float time,
  inout vec3 displaced,
  inout vec3 tangent,
  inout vec3 binormal
) {
  vec2 dir = normalize(wave.xy);
  float amp = wave.z;
  float wavelength = wave.w;
  float speed = params.x;
  float steepness = params.y;

  float k = 6.2831853 / wavelength; // 2 * PI / L
  float omega = sqrt(9.81 * k) * speed;
  float phase = k * dot(dir, pos.xz) - omega * time;

  // Q factor to prevent vertex inversion (steepness / (k * A * waveCount))
  float Q = steepness / (k * amp * 4.0 + 1e-4);

  float cosP = cos(phase);
  float sinP = sin(phase);

  // Trochoidal horizontal and vertical displacement
  displaced.x += Q * amp * dir.x * cosP;
  displaced.z += Q * amp * dir.y * cosP;
  displaced.y += amp * sinP;

  // Tangent partial derivatives along X
  tangent.x -= Q * dir.x * dir.x * (k * amp) * sinP;
  tangent.y += dir.x * (k * amp) * cosP;
  tangent.z -= Q * dir.x * dir.y * (k * amp) * sinP;

  // Binormal partial derivatives along Z
  binormal.x -= Q * dir.x * dir.y * (k * amp) * sinP;
  binormal.y += dir.y * (k * amp) * cosP;
  binormal.z -= Q * dir.y * dir.y * (k * amp) * sinP;
}

void main() {
  vWaterUv = uv;

  vec3 pos = position;
  vec3 displaced = vec3(0.0);
  vec3 tangent = vec3(1.0, 0.0, 0.0);
  vec3 binormal = vec3(0.0, 0.0, 1.0);

  evaluateGerstnerWave(uWave0, uWaveParams0, pos, uTime, displaced, tangent, binormal);
  evaluateGerstnerWave(uWave1, uWaveParams1, pos, uTime, displaced, tangent, binormal);
  evaluateGerstnerWave(uWave2, uWaveParams2, pos, uTime, displaced, tangent, binormal);
  evaluateGerstnerWave(uWave3, uWaveParams3, pos, uTime, displaced, tangent, binormal);

  vec3 finalPos = pos + displaced;
  finalPos.y += uSeaLevel;

  vWaveCrest = displaced.y;

  // Analytical Normal computed via cross product of tangent and binormal
  vec3 calcNormal = normalize(cross(binormal, tangent));
  vec3 transformed = finalPos;
  vec3 objectNormal = calcNormal;
  vec3 transformedNormal = normalMatrix * objectNormal;

  vec4 worldPosition = modelMatrix * vec4(transformed, 1.0);
  vWorldPosition = worldPosition.xyz;
  vNormal = normalize(transformedNormal);

  vec4 mvPosition = viewMatrix * worldPosition;
  vViewPosition = -mvPosition.xyz;
  gl_Position = projectionMatrix * mvPosition;

  #include <fog_vertex>
  #include <shadowmap_vertex>
}
`;

export const MAP2_WATER_FRAGMENT_SHADER = /* glsl */ `
#include <common>
#include <fog_pars_fragment>
#include <shadowmap_pars_fragment>

uniform float uTime;
uniform vec3 uSunDir;
uniform vec3 uSunColor;
uniform vec3 uShallowColor;
uniform vec3 uDeepColor;
uniform vec3 uHorizonHazeColor;
uniform vec3 uFoamColor;
uniform float uWaterTransparency;
uniform float uRoughness;
uniform float uFresnelF0;
uniform float uSeaLevel;

varying vec3 vWorldPosition;
varying vec3 vNormal;
varying vec3 vViewPosition;
varying vec2 vWaterUv;
varying float vWaveCrest;

// Procedural 2D Hash
float waterHash(vec2 p) {
  p = fract(p * vec2(234.34, 567.89));
  p += dot(p, p + 67.89);
  return fract(p.x * p.y);
}

// Procedural 2D Value Noise
float waterNoise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  f = f * f * (3.0 - 2.0 * f);

  float a = waterHash(i);
  float b = waterHash(i + vec2(1.0, 0.0));
  float c = waterHash(i + vec2(0.0, 1.0));
  float d = waterHash(i + vec2(1.0, 1.0));

  return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

// Multi-octave FBM for dynamic shoreline foam & ripples
float waterFBM(vec2 p) {
  float v = 0.0;
  float a = 0.5;
  mat2 rot = mat2(0.8, 0.6, -0.6, 0.8);
  for (int i = 0; i < 3; i++) {
    v += a * waterNoise(p);
    p = rot * p * 2.1 + vec2(31.4, 15.9);
    a *= 0.5;
  }
  return v;
}

// Approximate terrain height field analytically for self-contained optical depth extinction
float getApproxTerrainHeight(vec2 xz) {
  // Analytical approximation matching Map2 domain warped terrain basin
  float r = length(xz) * 0.015;
  float base = 15.0 * sin(xz.x * 0.018) * cos(xz.y * 0.018) + (r * r * 20.0);
  return clamp(base, 0.0, 80.0);
}

void main() {
  vec3 N = normalize(vNormal);
  vec3 V = normalize(cameraPosition - vWorldPosition);
  vec3 L = normalize(uSunDir);

  // Micro-ripple perturbation via animated FBM
  vec2 uvRipple = vWorldPosition.xz * 0.25 + vec2(uTime * 0.04, uTime * 0.03);
  vec2 rippleOffset = vec2(
    waterNoise(uvRipple * 2.0) - 0.5,
    waterNoise(uvRipple * 2.0 + vec2(12.3, 45.6)) - 0.5
  ) * 0.12;
  N = normalize(N + vec3(rippleOffset.x, 0.0, rippleOffset.y));

  // 1. Beer-Lambert Optical Depth Absorption
  // Delta y = Water Surface Y - Submerged Terrain Y
  float terrainY = getApproxTerrainHeight(vWorldPosition.xz);
  float waterDepth = max(vWorldPosition.y - terrainY, 0.0);

  // Extinction factor: I = I_0 * exp(-alpha * delta_y)
  float depthExtinction = 1.0 - exp(-waterDepth * 0.22);
  vec3 baseWaterColor = mix(uShallowColor, uDeepColor, depthExtinction);

  // 2. Fresnel Reflection (Schlick approximation with F0 = 0.02)
  float NdotV = max(dot(N, V), 0.0);
  float fresnel = uFresnelF0 + (1.0 - uFresnelF0) * pow(1.0 - NdotV, 5.0);

  // Sky environment reflection
  vec3 skyReflection = mix(uHorizonHazeColor, vec3(0.85, 0.95, 1.0), fresnel);

  // 3. Specular Sun Glitter (Blinn-Phong / GGX approximation)
  vec3 H = normalize(L + V);
  float NdotH = max(dot(N, H), 0.0);
  float specular = pow(NdotH, 128.0) * (1.0 - uRoughness);
  vec3 sunGlint = uSunColor * specular * 1.8;

  // 4. Dynamic Shoreline Surf and Foam Band
  // Surf pulsation near shorelines / shallows
  float surfSurge = sin(uTime * 1.6 + (vWorldPosition.x + vWorldPosition.z) * 0.12);
  float foamThreshold = 1.2 + 0.4 * surfSurge;
  float shorelineFoamBand = 1.0 - smoothstep(0.0, foamThreshold, waterDepth);

  // Foam cresting on highest Gerstner wave peaks
  float waveCrestFoam = smoothstep(0.45, 0.65, vWaveCrest);

  float totalFoamMask = max(shorelineFoamBand, waveCrestFoam * 0.75);
  if (totalFoamMask > 0.02) {
    float foamNoiseVal = waterFBM(vWorldPosition.xz * 0.8 + vec2(uTime * 0.12));
    float foamPattern = smoothstep(0.35, 0.75, foamNoiseVal) * totalFoamMask;
    baseWaterColor = mix(baseWaterColor, uFoamColor, foamPattern * 0.9);
  }

  // 5. Final Color Composition
  vec3 composite = mix(baseWaterColor, skyReflection, fresnel * 0.65) + sunGlint;
  float alpha = mix(uWaterTransparency, 0.95, depthExtinction);

  gl_FragColor = vec4(composite, alpha);

  #include <fog_fragment>
}
`;

export interface Map2WaterShaderUniforms {
  uTime: { value: number };
  uSeaLevel: { value: number };
  uSunDir: { value: THREE.Vector3 };
  uSunColor: { value: THREE.Color };
  uShallowColor: { value: THREE.Color };
  uDeepColor: { value: THREE.Color };
  uHorizonHazeColor: { value: THREE.Color };
  uFoamColor: { value: THREE.Color };
  uWaterTransparency: { value: number };
  uRoughness: { value: number };
  uFresnelF0: { value: number };
  uWave0: { value: THREE.Vector4 };
  uWaveParams0: { value: THREE.Vector2 };
  uWave1: { value: THREE.Vector4 };
  uWaveParams1: { value: THREE.Vector2 };
  uWave2: { value: THREE.Vector4 };
  uWaveParams2: { value: THREE.Vector2 };
  uWave3: { value: THREE.Vector4 };
  uWaveParams3: { value: THREE.Vector2 };
}

export function createMap2WaterMaterial(
  customUniforms?: Partial<Map2WaterShaderUniforms>,
): THREE.ShaderMaterial {
  const uniforms: Map2WaterShaderUniforms = {
    ...THREE.UniformsUtils.clone(THREE.UniformsLib.fog),
    uTime: { value: 0 },
    uSeaLevel: { value: 12.0 },
    uSunDir: { value: new THREE.Vector3(120, 180, 80).normalize() },
    uSunColor: { value: new THREE.Color('#fff7ed') },
    uShallowColor: { value: new THREE.Color('#48ddca') }, // Crystal Turquoise
    uDeepColor: { value: new THREE.Color('#051d3b') },    // Deep Ocean Navy
    uHorizonHazeColor: { value: new THREE.Color('#bae6fd') },
    uFoamColor: { value: new THREE.Color('#f8fafc') },
    uWaterTransparency: { value: 0.72 },
    uRoughness: { value: 0.08 },
    uFresnelF0: { value: 0.02 },
    // Wave 0: Dir [1.0, 0.2], Amp 0.35, Wavelength 14.0, Speed 1.0, Steepness 0.45
    uWave0: { value: new THREE.Vector4(1.0, 0.2, 0.35, 14.0) },
    uWaveParams0: { value: new THREE.Vector2(1.0, 0.45) },
    // Wave 1: Dir [0.4, 0.9], Amp 0.20, Wavelength 7.5, Speed 1.25, Steepness 0.35
    uWave1: { value: new THREE.Vector4(0.4, 0.9, 0.20, 7.5) },
    uWaveParams1: { value: new THREE.Vector2(1.25, 0.35) },
    // Wave 2: Dir [-0.6, 0.5], Amp 0.10, Wavelength 4.0, Speed 1.6, Steepness 0.25
    uWave2: { value: new THREE.Vector4(-0.6, 0.5, 0.10, 4.0) },
    uWaveParams2: { value: new THREE.Vector2(1.6, 0.25) },
    // Wave 3: Dir [-0.3, -0.9], Amp 0.05, Wavelength 2.0, Speed 2.0, Steepness 0.20
    uWave3: { value: new THREE.Vector4(-0.3, -0.9, 0.05, 2.0) },
    uWaveParams3: { value: new THREE.Vector2(2.0, 0.20) },
    ...customUniforms,
  };

  return new THREE.ShaderMaterial({
    vertexShader: MAP2_WATER_VERTEX_SHADER,
    fragmentShader: MAP2_WATER_FRAGMENT_SHADER,
    uniforms: uniforms as unknown as { [uniform: string]: THREE.IUniform },
    transparent: true,
    depthWrite: false,
    fog: true,
    side: THREE.DoubleSide,
  });
}

// -------------------------------------------------------------------------------------------------
// Map 2 Waterfall Spray & Foam Curtain Shaders
// -------------------------------------------------------------------------------------------------

export const MAP2_WATERFALL_VERTEX_SHADER = /* glsl */ `
#include <common>
#include <fog_pars_vertex>

uniform float uTime;
uniform float uFlowSpeed;

varying vec2 vUv;
varying vec3 vWorldPosition;
varying float vFlowY;

void main() {
  vUv = uv;
  vec4 worldPosition = modelMatrix * vec4(position, 1.0);
  vWorldPosition = worldPosition.xyz;
  vFlowY = position.y;

  vec4 mvPosition = viewMatrix * worldPosition;
  gl_Position = projectionMatrix * mvPosition;

  #include <fog_vertex>
}
`;

export const MAP2_WATERFALL_FRAGMENT_SHADER = /* glsl */ `
#include <common>
#include <fog_pars_fragment>

uniform float uTime;
uniform vec3 uSprayColor;
uniform vec3 uWaterColor;
uniform float uOpacity;

varying vec2 vUv;
varying vec3 vWorldPosition;
varying float vFlowY;

// Safe procedural noise
float wfHash(vec2 p) {
  return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453);
}

float wfNoise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  return mix(
    mix(wfHash(i), wfHash(i + vec2(1.0, 0.0)), f.x),
    mix(wfHash(i + vec2(0.0, 1.0)), wfHash(i + vec2(1.0, 1.0)), f.x),
    f.y
  );
}

void main() {
  // Accelerating downward flow UV scroll
  float scroll = uTime * 2.8;
  vec2 flowUv = vec2(vUv.x * 2.0, vUv.y * 3.0 - scroll);

  // Vertical foam streak noise
  float streak1 = wfNoise(flowUv);
  float streak2 = wfNoise(flowUv * 2.0 + vec2(3.1, 1.4));
  float streaks = streak1 * 0.6 + streak2 * 0.4;

  // Foam froth intensity
  float foamMask = smoothstep(0.2, 0.7, streaks);
  vec3 col = mix(uWaterColor, uSprayColor, foamMask);

  // Soft edge fading at top and bottom
  float edgeFade = smoothstep(0.0, 0.1, vUv.y) * smoothstep(1.0, 0.9, vUv.y) * smoothstep(0.0, 0.1, vUv.x) * smoothstep(1.0, 0.9, vUv.x);
  float alpha = uOpacity * (0.5 + 0.5 * foamMask) * edgeFade;

  gl_FragColor = vec4(col, alpha);

  #include <fog_fragment>
}
`;

export interface Map2WaterfallShaderUniforms {
  uTime: { value: number };
  uFlowSpeed: { value: number };
  uSprayColor: { value: THREE.Color };
  uWaterColor: { value: THREE.Color };
  uOpacity: { value: number };
}

export function createMap2WaterfallMaterial(
  customUniforms?: Partial<Map2WaterfallShaderUniforms>,
): THREE.ShaderMaterial {
  const uniforms: Map2WaterfallShaderUniforms = {
    ...THREE.UniformsUtils.clone(THREE.UniformsLib.fog),
    uTime: { value: 0 },
    uFlowSpeed: { value: 2.8 },
    uSprayColor: { value: new THREE.Color('#f1f5f9') }, // White foam spray
    uWaterColor: { value: new THREE.Color('#38bdf8') }, // Cyan water stream
    uOpacity: { value: 0.85 },
    ...customUniforms,
  };

  return new THREE.ShaderMaterial({
    vertexShader: MAP2_WATERFALL_VERTEX_SHADER,
    fragmentShader: MAP2_WATERFALL_FRAGMENT_SHADER,
    uniforms: uniforms as unknown as { [uniform: string]: THREE.IUniform },
    transparent: true,
    depthWrite: false,
    side: THREE.DoubleSide,
    fog: true,
  });
}
