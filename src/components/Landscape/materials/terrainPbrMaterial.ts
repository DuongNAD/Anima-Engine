import * as THREE from 'three';
import type { TerrainPbrArrays } from '../assets/worldAssets';
import { ATMOSPHERE_FRAGMENT_PARS, bindAtmosphereUniforms } from './atmosphere';

// prototype, to be replaced — W3-lite 6-layer biome/slope/height PBR splat terrain material with seamless procedural fallback.

export interface TerrainPbrMaterialParams {
  colorMap: THREE.Texture;
  normalMap: THREE.Texture;
  roughnessMap: THREE.Texture;
  detailMap: THREE.Texture;
  riverMaskMap: THREE.Texture;
  splatMap: THREE.Texture;
  heightUnits: number;
  horizonMap?: THREE.Texture | null;
  sunDir?: [number, number, number];
  normalScale?: number;
  /**
   * WV2-LOOK (World v2 only): biome surface classes (`lookMaskMap`) + tileable noise switch the
   * material to slope/altitude-driven rock/scree/soil/snow shading, baked AO from the colour
   * map's alpha, and height-attenuated aerial perspective. Absent = the W3-lite path, unchanged.
   */
  lookMaskMap?: THREE.Texture | null;
  lookNoiseMap?: THREE.Texture | null;
  /** WV2-LOOK coast: baked shore SDF (R = (sdf_m + 500) / 1000, + on land; G = wave exposure). */
  lookShoreMap?: THREE.Texture | null;
  elevMinM?: number;
  elevSpanM?: number;
}

export interface TerrainPbrShaderHandle {
  applyPbrArrays(arrays: TerrainPbrArrays | null): void;
  updateTime(timeSec: number): void;
  updateSunDir(sunDir: [number, number, number]): void;
  dispose(): void;
}

function makeDummyLayerArray(r: number, g: number, b: number, a: number): THREE.DataArrayTexture {
  const layerCount = 6;
  const data = new Uint8Array(layerCount * 4);
  for (let i = 0; i < layerCount; i++) {
    const o = i * 4;
    data[o] = r;
    data[o + 1] = g;
    data[o + 2] = b;
    data[o + 3] = a;
  }
  const tex = new THREE.DataArrayTexture(data, 1, 1, layerCount);
  tex.format = THREE.RGBAFormat;
  tex.type = THREE.UnsignedByteType;
  tex.colorSpace = THREE.NoColorSpace;
  tex.minFilter = THREE.LinearFilter;
  tex.magFilter = THREE.LinearFilter;
  tex.generateMipmaps = false;
  tex.needsUpdate = true;
  return tex;
}

// prototype, to be replaced — WV2-LOOK terrain shading (World v2 only).
//
// Colour comes from surface physics rather than one flat biome swatch: bedrock with strata on
// steep faces, scree/talus on rocky gentle ground, soil on mid slopes, varied vegetation tints
// on gentle low ground, and snow by altitude AND slope so steep faces show rock through it.
// Slope is read from the baked `normal_xy` layer (true metres), altitude from world Y.
const WV2_LOOK_PARS = /* glsl */ `
uniform sampler2D uLookMask;
uniform sampler2D uLookNoise;
uniform sampler2D uLookShore;
uniform float uElevMinM;
uniform float uElevSpanM;
`;

const WV2_LOOK_MAP_FRAGMENT = /* glsl */ `
#include <map_fragment>
// ---- WV2-LOOK surface ------------------------------------------------------------------
float lkAo = clamp(diffuseColor.a * 1.25, 0.0, 1.3);
diffuseColor.a = 1.0;
vec3 lkWp = vW3WorldPos;
vec4 lkMask = texture2D(uLookMask, vMapUv);
vec3 lkBn = texture2D(normalMap, vMapUv).xyz * 2.0 - 1.0;
float lkSlope = 1.0 - clamp(lkBn.z, 0.0, 1.0);
float lkHm = (lkWp.y / uHeightUnits) * uElevSpanM + uElevMinM;
// Steep faces: every baked layer (slope, AO, horizon, albedo) is a heightfield texel stretched
// down the face, which reads as long vertical streaks. The projection weights and the
// "how steep" decisions therefore come from the smooth GEOMETRIC normal, steep faces take
// biplanar procedural noise (side planes, blended by the normal), and the stretched layers
// are damped there (lkStretch) and re-broken with isotropic noise.
vec3 lkGn = normalize(vW3WorldNormal);
float lkGSlope = 1.0 - clamp(lkGn.y, 0.0, 1.0);
float lkStretch = smoothstep(0.20, 0.45, lkGSlope);
vec2 lkP = lkWp.xz;
float lkSteepW = smoothstep(0.10, 0.30, lkGSlope);
float lkAxX = abs(lkGn.x) / max(abs(lkGn.x) + abs(lkGn.z), 1e-3);
vec2 lkSA = vec2(lkWp.z, lkWp.y * 1.2);
vec2 lkSB = vec2(lkWp.x, lkWp.y * 1.2);
float lkCamDist = length(vViewPosition);
vec4 lkNLo = texture2D(uLookNoise, lkP * 0.0019);
vec4 lkNMid = texture2D(uLookNoise, lkP * 0.0105 + vec2(0.31, 0.17));
vec4 lkNHi = texture2D(uLookNoise, lkP * 0.071 + vec2(0.13, 0.57));
// The finest octave only matters within ~300 units (it mips to flat grey beyond), so it is
// fetched only there. Explicit gradients keep mip selection valid inside the branches.
vec3 lkWdx = dFdx(lkWp);
vec3 lkWdy = dFdy(lkWp);
float lkVhiW = 1.0 - smoothstep(230.0, 300.0, lkCamDist);
vec4 lkNVHi = vec4(0.5);
if (lkVhiW > 0.0) {
  lkNVHi = mix(vec4(0.5),
    textureGrad(uLookNoise, lkP * 0.53 + vec2(0.71, 0.29), lkWdx.xz * 0.53, lkWdy.xz * 0.53), lkVhiW);
}
if (lkSteepW > 0.01) {
  // Side planes: x-y for faces looking along +-z, z-y for faces looking along +-x.
  vec2 dA = vec2(lkWdx.z, lkWdx.y * 1.2);
  vec2 dAy = vec2(lkWdy.z, lkWdy.y * 1.2);
  vec2 dB = vec2(lkWdx.x, lkWdx.y * 1.2);
  vec2 dBy = vec2(lkWdy.x, lkWdy.y * 1.2);
  vec4 midS = mix(textureGrad(uLookNoise, lkSB * 0.03 + vec2(0.31, 0.17), dB * 0.03, dBy * 0.03),
                  textureGrad(uLookNoise, lkSA * 0.03 + vec2(0.77, 0.43), dA * 0.03, dAy * 0.03), lkAxX);
  vec4 hiS = mix(textureGrad(uLookNoise, lkSB * 0.071 + vec2(0.13, 0.57), dB * 0.071, dBy * 0.071),
                 textureGrad(uLookNoise, lkSA * 0.071 + vec2(0.53, 0.21), dA * 0.071, dAy * 0.071), lkAxX);
  lkNMid = mix(lkNMid, midS, lkSteepW);
  lkNHi = mix(lkNHi, hiS, lkSteepW);
  if (lkVhiW > 0.0) {
    vec4 vhiS = mix(textureGrad(uLookNoise, lkSB * 0.53 + vec2(0.71, 0.29), dB * 0.53, dBy * 0.53),
                    textureGrad(uLookNoise, lkSA * 0.53 + vec2(0.27, 0.61), dA * 0.53, dAy * 0.53), lkAxX);
    lkNVHi = mix(lkNVHi, mix(vec4(0.5), vhiS, lkVhiW), lkSteepW);
  }
}
// Damp the stretched data on steep faces: slope leans on the geometric normal, AO is
// re-broken with isotropic noise.
lkSlope = mix(lkSlope, lkGSlope * 1.15 + (lkNHi.a - 0.5) * 0.12, lkStretch * 0.75);
lkAo = mix(lkAo, 0.80 + 0.28 * lkNHi.r, lkStretch * 0.7);
float lkNear = 1.0 - smoothstep(40.0, 200.0, lkCamDist);

vec3 lkMacro = diffuseColor.rgb;
// Vegetated / bare ground: break the biome swatch with lush-cool <-> dry-warm drift.
float lkDry = smoothstep(0.30, 0.72, lkNLo.r * 0.55 + lkNMid.r * 0.45 + (1.0 - lkAo) * -0.25);
vec3 lkGround = lkMacro
  * mix(vec3(0.84, 0.97, 0.80), vec3(1.26, 1.07, 0.70), lkDry)
  * mix(0.78, 1.15, lkNMid.g)
  * mix(0.88, 1.10, lkNHi.b);
// Distant vegetation reads olive, not lawn: pull saturation and value down a little, and give
// dense canopy biomes a dappled crown texture so the ground between trunks is not bare paint.
float lkGLum = dot(lkGround, vec3(0.2126, 0.7152, 0.0722));
// Bright meadow greens desaturate more than dark ones (the lime swatches read as lawn).
lkGround = mix(vec3(lkGLum), lkGround, mix(0.70, 0.52, smoothstep(0.08, 0.20, lkGLum))) * 0.82;
// ...and lose some value too: sunlit pale meadow was the brightest, flattest thing in frame.
lkGround *= mix(1.0, 0.84, smoothstep(0.05, 0.14, lkGLum));
// Meadows: mottled, slightly dry tufts so open grass does not read as smooth plastic.
lkGround *= mix(0.86, 1.10, lkNHi.r) * mix(0.92, 1.06, lkNVHi.a);
lkGround = mix(lkGround, lkGround * vec3(1.12, 1.0, 0.72), smoothstep(0.55, 0.85, lkNHi.g) * 0.45);
// Closed canopy where the baked trees are (lookMask B is built from the flora points): dark
// crown texture with sunlit tops and shadowed gaps, ragged at the edge. The 3D trees stand on it.
// Wide, noise-driven transition: a sharp threshold on the bilinear 20 m mask shows its texels.
float lkCanopy = smoothstep(0.20, 0.85, pow(lkMask.b, 1.4) * 1.15 + (lkNHi.g - 0.5) * 0.45 + (lkNMid.b - 0.5) * 0.2);
// Near the camera the 3D trees are the crowns; the painted crown pattern flattens into a
// shadowed forest floor there instead of a second, camouflage-like layer of crowns.
float lkCrown = mix(0.42, smoothstep(0.30, 0.78, lkNVHi.g * 0.6 + lkNHi.b * 0.4), smoothstep(25.0, 110.0, lkCamDist));
vec3 lkCanopyCol = mix(vec3(0.012, 0.023, 0.011), vec3(0.043, 0.074, 0.026), lkCrown);
lkCanopyCol *= mix(vec3(0.85, 1.0, 1.08), vec3(1.15, 1.04, 0.85), lkDry) * mix(0.85, 1.15, lkNMid.g);
lkGround = mix(lkGround, lkCanopyCol, lkCanopy * (1.0 - lkMask.g));
// Soil showing through on moderately steep vegetated ground.
float lkSoil = smoothstep(0.09, 0.24, lkSlope + (lkNMid.b - 0.5) * 0.14)
  * (1.0 - lkMask.g) * (1.0 - lkMask.a * 0.8);
vec3 lkSoilCol = mix(vec3(0.075, 0.056, 0.040), vec3(0.15, 0.115, 0.080), lkNHi.r);
lkGround = mix(lkGround, lkSoilCol, lkSoil * 0.6);
// Rock-biome gentle ground -> scree / talus, lichen-stained below the high alpine.
vec3 lkScree = mix(vec3(0.15, 0.145, 0.135), vec3(0.27, 0.25, 0.22), lkNHi.g)
  * mix(0.82, 1.14, lkNVHi.r);
float lkLichen = smoothstep(0.52, 0.80, lkNMid.a) * (1.0 - smoothstep(2600.0, 3600.0, lkHm));
lkScree = mix(lkScree, vec3(0.10, 0.12, 0.07), lkLichen * 0.55);
lkGround = mix(lkGround, lkScree, lkMask.r);
// Bedrock on steep faces (and on rock-biome moderate slopes), with altitude strata.
float lkSteepLo = 0.25 + (lkNMid.r - 0.5) * 0.12;
float lkRock = smoothstep(lkSteepLo, lkSteepLo + 0.15, lkSlope + (lkNHi.a - 0.5) * 0.08);
lkRock = max(lkRock, lkMask.r * smoothstep(0.12, 0.26, lkSlope + (lkNHi.a - 0.5) * 0.16));
vec4 lkRHi = lkNHi;
float lkStrataMask = smoothstep(0.35, 0.75, lkNMid.b);
float lkStrata = sin(lkHm * 0.042 + (lkNMid.r - 0.5) * 5.0 + (lkNLo.g - 0.5) * 9.0 + (lkRHi.r - 0.5) * 2.0);
vec3 lkRockCol = mix(vec3(0.095, 0.094, 0.098), vec3(0.20, 0.168, 0.135),
  smoothstep(0.30, 0.70, lkNLo.b + (lkNMid.g - 0.5) * 0.4));
lkRockCol *= 1.0 + lkStrata * 0.10 * lkStrataMask;
lkRockCol *= mix(0.88, 1.12, lkRHi.g);
lkGround = mix(lkGround, lkRockCol, lkRock);
// Snow: altitude band with a noisy snowline, the baked snow/glacier biomes, and slope shedding.
float lkSnowLine = 3500.0 + (lkNLo.a - 0.5) * 900.0 + (lkNMid.a - 0.5) * 380.0;
float lkSnowRaw = max(smoothstep(lkSnowLine - 140.0, lkSnowLine + 280.0, lkHm), lkMask.g);
float lkShed = 1.0 - smoothstep(0.24, 0.46, lkSlope + (lkNHi.b - 0.5) * 0.14);
// On steep faces snow survives on ledges and in patches (isotropic), not in fall-line strips.
float lkLedge = smoothstep(0.35, 0.85, sin(lkWp.y * 1.6 + (lkNMid.r - 0.5) * 6.0) * 0.5 + 0.5)
  * smoothstep(0.40, 0.70, lkNMid.g);
// Ledge/patch detail is sub-pixel past ~0.5 km of view distance: fade it to its mean there or
// it aliases into white speckle along the snow edge in the overview.
float lkPatchNear = 1.0 - smoothstep(250.0, 700.0, lkCamDist);
lkLedge = mix(0.12, lkLedge, lkPatchNear);
lkShed = mix(lkShed, max(lkShed, lkLedge * 0.85), lkStretch);
float lkSnow = smoothstep(0.30, 0.62, lkSnowRaw * lkShed
  + (lkNHi.b - 0.5) * mix(0.30, 0.22, lkStretch) * mix(0.5, 1.0, lkPatchNear)
  + (lkNMid.b - 0.5) * 0.45 * lkStretch);
vec3 lkSnowCol = vec3(0.70, 0.73, 0.78)
  * mix(0.92, 1.05, lkNMid.b)
  * mix(0.92, 1.02, lkNHi.r)
  * mix(0.95, 1.01, lkNVHi.a);
lkGround = mix(lkGround, lkSnowCol, lkSnow);
// Snow fills the baked rills: their AO / cast-shadow texels must not paint fall-line stripes.
lkAo = mix(lkAo, 0.92 + 0.08 * lkNHi.r, lkSnow * 0.75);

// Close-range grain. Procedural only: the CC0 photo layers tile every ~12 units (~400 m),
// so at this world scale their pebbles read as boulder-sized camouflage.
float lkGrain = mix(0.90, 1.10, lkNVHi.r) * mix(0.95, 1.05, lkNVHi.b);
float lkDetail = mix(1.0, lkGrain, lkNear * (0.35 + lkRock * 0.4 + lkMask.r * 0.2));
lkGround *= lkDetail;
// Seen through the water: absorption tint below sea level.
// Micro relief for the normal hook: two decorrelated, filtered noise channels used directly as
// a tangent-space tilt (no derivatives, so no 2x2-pixel blocks; mip-filtered with distance).
vec2 lkG = (vec2(lkNVHi.g, lkNVHi.b) - 0.5) * (1.0 - lkSnow * 0.7) * 0.9
         + (vec2(lkNHi.b, lkNHi.g) - 0.5) * 0.5;
// ---- Coast (reference board CST-01, CST-03, CST-04) ------------------------------------------
float lkHAbove = (lkWp.y - uAtmoSeaY) * (uElevSpanM / uHeightUnits);   // metres above sea
// Every coast term needs ground within ~60 m of sea level and ~320 m of the shore: the whole
// block (SDF fetch included) is skipped elsewhere, which is most of the screen.
vec2 lkUvDx = dFdx(vMapUv);
vec2 lkUvDy = dFdy(vMapUv);
float lkWet = 0.0;
float lkSpray = 0.0;
vec2 lkRipple = vec2(0.0);   // tangent-space tilt from beach ripples and the berm face
if (lkHAbove < 60.0 && lkHAbove > -45.0) {   // deeper bed is hidden by near-opaque water
  vec4 lkShoreS = textureGrad(uLookShore, vMapUv, lkUvDx, lkUvDy);
  float lkShoreM = lkShoreS.r * 1000.0 - 500.0;        // metres from the shore, + on land
  float lkExpo = lkShoreS.g;
  float lkCoast = 1.0 - smoothstep(150.0, 320.0, lkShoreM);
  // Sea bed: below the waterline it is sand, silt and rock, never meadow or canopy.
  float lkSeabed = (1.0 - smoothstep(-1.0, 0.6, lkHAbove)) * (1.0 - smoothstep(-10.0, 25.0, lkShoreM));
  if (lkCoast > 0.0 || lkSeabed > 0.0) {
    vec3 lkBedCol = mix(vec3(0.17, 0.155, 0.12), vec3(0.075, 0.08, 0.075), smoothstep(1.5, 18.0, -lkHAbove))
      * mix(0.85, 1.12, lkNHi.r);
    lkGround = mix(lkGround, mix(lkBedCol, lkGround, lkRock * 0.5), lkSeabed);
    // Beach (CST-03): a narrow, darker sand strip between the surf and the slope. Beach-class
    // ground higher up becomes dune grass instead of a wide pale band; gentle ground right at
    // the waterline gets a strip of sand/shingle even where the biome is not Beach.
    float lkBeachBand = (1.0 - smoothstep(3.0, 11.0, lkHAbove + (lkNHi.g - 0.5) * 4.0)) * step(-0.5, lkHAbove);
    float lkWaterStrip = (1.0 - smoothstep(1.2, 4.5, lkHAbove)) * step(-0.5, lkHAbove)
      * (1.0 - smoothstep(0.10, 0.24, lkSlope)) * lkCoast;
    float lkSandW = max(lkMask.a * lkBeachBand, lkWaterStrip) * (1.0 - lkSnow);
    vec3 lkSandCol = mix(vec3(0.105, 0.095, 0.078), vec3(0.16, 0.14, 0.11), lkNMid.g) * mix(0.9, 1.08, lkNVHi.r);
    vec3 lkDuneCol = mix(vec3(0.075, 0.08, 0.045), vec3(0.11, 0.10, 0.065), lkNHi.b);
    lkGround = mix(lkGround, lkDuneCol, lkMask.a * (1.0 - lkBeachBand) * lkCoast * 0.75 * (1.0 - lkRock));
    lkGround = mix(lkGround, lkSandCol, lkSandW * (1.0 - lkRock * 0.7));
    // Beach relief through normals (sub-texel for the height data): ripples parallel to the
    // shore and a berm, a low ridge ~2 m above the sea whose seaward face catches the light.
    if (lkSandW > 0.02 && lkNear > 0.01) {
      vec2 lkTx = 1.0 / vec2(textureSize(uLookShore, 0));
      float sx = textureGrad(uLookShore, vMapUv + vec2(lkTx.x, 0.0), lkUvDx, lkUvDy).r;
      float sz = textureGrad(uLookShore, vMapUv + vec2(0.0, lkTx.y), lkUvDx, lkUvDy).r;
      vec2 lkLand = normalize(vec2(sx, sz) - lkShoreS.r + vec2(1e-5));
      float ripPh = lkShoreM * 0.70 + (lkNMid.r - 0.5) * 6.0 + (lkNHi.g - 0.5) * 2.0;
      float berm = smoothstep(0.9, 1.7, lkHAbove) * (1.0 - smoothstep(1.7, 2.6, lkHAbove));
      lkRipple = lkLand * (cos(ripPh) * 0.11 * (1.0 - lkWet * 0.6) - berm * 0.3) * lkSandW * lkNear;
      lkGround *= 1.0 + 0.10 * berm * lkSandW;
    }
    // Swash band (CST-01, CST-04): ground just above the waterline is wet, so darker here and
    // glossier below (roughness hook + sky reflection at the output stage).
    lkWet = (1.0 - smoothstep(0.5, 1.1 + 1.0 * lkExpo, lkHAbove + (lkNVHi.b - 0.5) * 0.4))
      * step(-0.4, lkHAbove) * (1.0 - smoothstep(25.0, 70.0, lkShoreM)) * step(-30.0, lkShoreM);
    lkGround *= mix(1.0, 0.42, lkWet);
    // Spray haze over the surf-side ground (applied with the aerial perspective).
    lkSpray = lkExpo * (1.0 - smoothstep(40.0, 260.0, lkShoreM)) * (1.0 - smoothstep(0.0, 45.0, lkHAbove))
      * step(-1.0, lkHAbove);
  }
}
float lkUnder = clamp((uAtmoSeaY - lkWp.y) / 4.0, 0.0, 1.0);
lkGround = mix(lkGround, lkGround * vec3(0.42, 0.66, 0.70), lkUnder);
// Baked cavity: part on albedo, the rest on indirect light (see aomap hook below).
lkGround *= mix(1.0, lkAo, 0.45);
diffuseColor.rgb = lkGround;
float lkAoInd = clamp(lkAo, 0.35, 1.0);
float lkSunVis = 1.0;
{
  vec4 rivMask = texture2D(uRiverMask, vMapUv);
  float riv = rivMask.r;
  if (riv > 0.01) {
    vec2 rawFlow = (rivMask.gb * 255.0 - 128.0) / 127.0;
    float flowLen = length(rawFlow);
    float validFlow = smoothstep(0.35, 0.75, rivMask.a) * step(0.2, flowLen);
    vec2 flowDir = flowLen > 0.2 ? (rawFlow / flowLen) : vec2(0.0);
    float phaseJitter = texture2D(uDetail, vMapUv * 24.0).r;
    float p0 = fract(uTime * 0.30 + phaseJitter);
    float p1 = fract(uTime * 0.30 + phaseJitter + 0.5);
    float blend = abs(2.0 * p0 - 1.0);
    vec2 advUvBase = vMapUv * 220.0;
    float adv0 = texture2D(uDetail, advUvBase - flowDir * (p0 * 1.25)).r * 2.0;
    float adv1 = texture2D(uDetail, advUvBase - flowDir * (p1 * 1.25)).r * 2.0;
    float advected = mix(adv0, adv1, blend);
    diffuseColor.rgb = mix(diffuseColor.rgb, lkMacro * mix(1.0, advected, validFlow), clamp(riv * 0.85, 0.0, 0.85));
  }
  if (uUseBakedHorizon > 0.5) {
    vec4 hzn = texture2D(uHorizonMap, vMapUv) * 1.5707963;
    vec3 sd = normalize(uSunDir);
    vec4 azW = vec4(max(sd.x, 0.0), max(-sd.x, 0.0), max(-sd.z, 0.0), max(sd.z, 0.0));
    float wSum = max(0.0001, azW.x + azW.y + azW.z + azW.w);
    float thetaH = dot(hzn, azW) / wSum;
    float sunElev = asin(clamp(sd.y, -1.0, 1.0));
    lkSunVis = smoothstep(thetaH - 0.14, thetaH + 0.22, sunElev);
    // The horizon texels are stretched down cliffs too: soften and re-break them there.
    lkSunVis = mix(lkSunVis, clamp(lkSunVis * 0.45 + 0.30 + 0.35 * lkNHi.g, 0.0, 1.0),
                   max(lkStretch * 0.8, lkSnow * 0.6));
  }
  // Moving shadows of the cumulus layer overhead (direct sun only).
  lkSunVis *= atmoCloudShadow(lkWp);
}
`;

const WV2_LOOK_NORMAL_FRAGMENT = /* glsl */ `
#include <normal_fragment_maps>
{
  // Snow buries micro relief; on steep faces the baked fine normals are stretched texels.
  mapN.xy *= mix(1.0, 0.25, lkSnow) * mix(1.0, 0.4, lkStretch);
  normal = normalize(tbn * mapN);
  float lkNf = 1.0 - smoothstep(15.0, 140.0, lkCamDist);
  if (lkNf > 0.01) {
    vec2 g = lkG;
    float rough = 0.20 + lkRock * 0.8 + lkMask.r * 0.3 - lkSnow * 0.15;
    mapN.xy += g * 0.16 * rough * lkNf + lkRipple;
    normal = normalize(tbn * mapN);
  }
}
`;

// Output stage (after tone mapping, in display space like the sky dome): wet ground mirrors the
// sky at grazing angles, spray softens surf-side ground, then the shared aerial perspective.
const WV2_LOOK_OUTPUT = /* glsl */ `
{
  if (lkWet > 0.01) {
    vec3 lkV = normalize(cameraPosition - vW3WorldPos);
    vec3 lkWn = normalize(vec3(lkBn.x * 0.35, 1.0, lkBn.y * 0.35));
    float lkF = 0.02 + 0.98 * pow(1.0 - max(dot(lkWn, lkV), 0.0), 5.0);
    vec3 lkR = reflect(-lkV, lkWn);
    lkR.y = abs(lkR.y);
    gl_FragColor.rgb = mix(gl_FragColor.rgb, atmoSkyColor(normalize(lkR)), clamp(lkF * 1.6 + 0.08, 0.0, 0.7) * lkWet);
  }
  vec3 lkHd = normalize(vW3WorldPos - cameraPosition);
  vec3 lkHazeC = atmoSkyColor(normalize(vec3(lkHd.x, lkHd.y * 0.2, lkHd.z)));
  if (lkSpray > 0.0) gl_FragColor.rgb = mix(gl_FragColor.rgb, mix(lkHazeC, vec3(0.92), 0.35), lkSpray * 0.32);
  gl_FragColor.rgb = mix(gl_FragColor.rgb, lkHazeC, atmoHazeAmount(cameraPosition, vW3WorldPos));
}
`;

const WV2_LOOK_LIGHTS_BEGIN = THREE.ShaderChunk.lights_fragment_begin.replace(
  'getDirectionalLightInfo( directionalLight, directLight );',
  'getDirectionalLightInfo( directionalLight, directLight );\n\t\tdirectLight.color *= lkSunVis;',
);

// Map edge: the C01 tile has land touching its border, which read as a cut slab. Within a
// noisy 0.5-3.4 km band the surface is pulled down under the (opaque, deep) border ocean, so the
// world ends in a coastline instead of a vertical cut. Rendering only; colliders, creature and
// flora heights still read the data (flora skips this band).
const WV2_LOOK_EDGE_VERTEX = /* glsl */ `
#include <begin_vertex>
{
  vec2 lkUv0 = uv;
  float lkD = min(min(lkUv0.x, 1.0 - lkUv0.x), min(lkUv0.y, 1.0 - lkUv0.y));
  float lkEn = texture2D(uLookNoise, lkUv0 * 2.0).r * 0.6 + texture2D(uLookNoise, lkUv0 * 7.0).g * 0.4;
  float lkW = 0.012 + 0.07 * lkEn * lkEn;
  float lkFall = smoothstep(0.0, lkW, lkD);
  transformed.y = min(transformed.y, mix(uAtmoSeaY - 20.0, transformed.y, lkFall));
}
`;

function applyWv2LookShader(shader: THREE.WebGLProgramParametersWithUniforms): void {
  shader.vertexShader =
    'varying vec3 vW3WorldPos;\nvarying vec3 vW3WorldNormal;\nuniform sampler2D uLookNoise;\nuniform float uAtmoSeaY;\n' +
    shader.vertexShader.replace('#include <begin_vertex>', WV2_LOOK_EDGE_VERTEX).replace(
      '#include <worldpos_vertex>',
      `#include <worldpos_vertex>
        vec4 w3WorldPos4 = modelMatrix * vec4( transformed, 1.0 );
        vW3WorldPos = w3WorldPos4.xyz;
        vW3WorldNormal = normalize( mat3( modelMatrix ) * objectNormal );`,
    );
  shader.fragmentShader =
    `precision highp sampler2DArray;
uniform sampler2D uDetail;
uniform sampler2D uRiverMask;
uniform sampler2D uSplatMap;
uniform sampler2D uHorizonMap;
uniform float uUseBakedHorizon;
uniform vec3 uSunDir;
uniform float uTime;
uniform float uPbrEnabled;
uniform float uHeightUnits;
uniform sampler2DArray uPbrAlbedo;
uniform sampler2DArray uPbrNormal;
varying vec3 vW3WorldPos;
varying vec3 vW3WorldNormal;
${WV2_LOOK_PARS}
${ATMOSPHERE_FRAGMENT_PARS}
vec4 w3SampleAntiTile(sampler2DArray texArr, vec2 wp, float layer) {
  vec2 uvA = wp * 0.085;
  vec2 rotWp = vec2(wp.x * 0.7986 - wp.y * 0.6018, wp.x * 0.6018 + wp.y * 0.7986);
  vec2 uvB = rotWp * 0.021 + vec2(0.37, 0.71);
  vec4 a = texture(texArr, vec3(uvA, layer));
  vec4 b = texture(texArr, vec3(uvB, layer));
  return mix(a, b, 0.44);
}
vec4 w3SampleTriplanarRock(sampler2DArray texArr, vec3 wp, vec3 wn) {
  vec3 blend = abs(wn);
  blend = max(blend - vec3(0.2), vec3(0.001));
  blend /= (blend.x + blend.y + blend.z);
  vec4 cx = w3SampleAntiTile(texArr, wp.zy, 2.0);
  vec4 cy = w3SampleAntiTile(texArr, wp.xz, 2.0);
  vec4 cz = w3SampleAntiTile(texArr, wp.xy, 2.0);
  return cx * blend.x + cy * blend.y + cz * blend.z;
}
` +
    shader.fragmentShader
      .replace('#include <map_fragment>', WV2_LOOK_MAP_FRAGMENT)
      .replace('#include <normal_fragment_maps>', WV2_LOOK_NORMAL_FRAGMENT)
      .replace('#include <lights_fragment_begin>', WV2_LOOK_LIGHTS_BEGIN)
      .replace(
        '#include <aomap_fragment>',
        '#include <aomap_fragment>\n\treflectedLight.indirectDiffuse *= lkAoInd;',
      )
      .replace(
        '#include <roughnessmap_fragment>',
        '#include <roughnessmap_fragment>\n\troughnessFactor = mix(roughnessFactor, 0.16, lkWet);',
      )
      .replace('#include <fog_fragment>', WV2_LOOK_OUTPUT);
}

export function createTerrainPbrMaterial(
  params: TerrainPbrMaterialParams,
): { material: THREE.MeshStandardMaterial; handle: TerrainPbrShaderHandle } {
  const dummyAlbedo = makeDummyLayerArray(180, 180, 180, 255);
  const dummyNormal = makeDummyLayerArray(128, 128, 255, 255);
  const dummyHorizon = new THREE.DataTexture(
    new Uint8Array([0, 0, 0, 0]),
    1,
    1,
    THREE.RGBAFormat,
    THREE.UnsignedByteType,
  );
  dummyHorizon.needsUpdate = true;
  // Far inland, no exposure: the coast terms vanish when no shore SDF is supplied.
  const dummyShore = new THREE.DataTexture(
    new Uint8Array([255, 0, 0, 255]),
    1,
    1,
    THREE.RGBAFormat,
    THREE.UnsignedByteType,
  );
  dummyShore.needsUpdate = true;
  const hasHorizon = Boolean(params.horizonMap);
  const look = Boolean(params.lookMaskMap && params.lookNoiseMap);
  const initSun = params.sunDir ?? [0.6, 0.7, 0.35];

  const uniforms: {
    uDetail: { value: THREE.Texture };
    uRiverMask: { value: THREE.Texture };
    uSplatMap: { value: THREE.Texture };
    uHorizonMap: { value: THREE.Texture };
    uUseBakedHorizon: { value: number };
    uSunDir: { value: THREE.Vector3 };
    uTime: { value: number };
    uPbrEnabled: { value: number };
    uHeightUnits: { value: number };
    uPbrAlbedo: { value: THREE.DataArrayTexture };
    uPbrNormal: { value: THREE.DataArrayTexture };
  } = {
    uDetail: { value: params.detailMap },
    uRiverMask: { value: params.riverMaskMap },
    uSplatMap: { value: params.splatMap },
    uHorizonMap: { value: params.horizonMap ?? dummyHorizon },
    uUseBakedHorizon: { value: hasHorizon ? 1 : 0 },
    uSunDir: { value: new THREE.Vector3(initSun[0], initSun[1], initSun[2]) },
    uTime: { value: 0 },
    uPbrEnabled: { value: 0 },
    uHeightUnits: { value: Math.max(1, params.heightUnits) },
    uPbrAlbedo: { value: dummyAlbedo },
    uPbrNormal: { value: dummyNormal },
  };
  const lookUniforms = {
    uLookMask: { value: params.lookMaskMap ?? params.detailMap },
    uLookNoise: { value: params.lookNoiseMap ?? params.detailMap },
    uLookShore: { value: params.lookShoreMap ?? dummyShore },
    uElevMinM: { value: params.elevMinM ?? -1500 },
    uElevSpanM: { value: params.elevSpanM ?? 7200 },
  };

  const normScale = params.normalScale ?? 1;
  const material = new THREE.MeshStandardMaterial({
    map: params.colorMap,
    normalMap: params.normalMap,
    normalScale: new THREE.Vector2(normScale, normScale),
    roughnessMap: params.roughnessMap,
    roughness: 1.0,
    metalness: 0.0,
  });

  material.onBeforeCompile = (shader) => {
    shader.uniforms.uDetail = uniforms.uDetail;
    shader.uniforms.uRiverMask = uniforms.uRiverMask;
    shader.uniforms.uSplatMap = uniforms.uSplatMap;
    shader.uniforms.uHorizonMap = uniforms.uHorizonMap;
    shader.uniforms.uUseBakedHorizon = uniforms.uUseBakedHorizon;
    shader.uniforms.uSunDir = uniforms.uSunDir;
    shader.uniforms.uTime = uniforms.uTime;
    shader.uniforms.uPbrEnabled = uniforms.uPbrEnabled;
    shader.uniforms.uHeightUnits = uniforms.uHeightUnits;
    shader.uniforms.uPbrAlbedo = uniforms.uPbrAlbedo;
    shader.uniforms.uPbrNormal = uniforms.uPbrNormal;

    if (look) {
      shader.uniforms.uLookMask = lookUniforms.uLookMask;
      shader.uniforms.uLookNoise = lookUniforms.uLookNoise;
      shader.uniforms.uLookShore = lookUniforms.uLookShore;
      shader.uniforms.uElevMinM = lookUniforms.uElevMinM;
      shader.uniforms.uElevSpanM = lookUniforms.uElevSpanM;
      bindAtmosphereUniforms(shader.uniforms);
      applyWv2LookShader(shader);
      return;
    }

    shader.vertexShader =
      'varying vec3 vW3WorldPos;\nvarying vec3 vW3WorldNormal;\n' +
      shader.vertexShader.replace(
        '#include <worldpos_vertex>',
        `#include <worldpos_vertex>
        vec4 w3WorldPos4 = modelMatrix * vec4( transformed, 1.0 );
        vW3WorldPos = w3WorldPos4.xyz;
        vW3WorldNormal = normalize( mat3( modelMatrix ) * objectNormal );`,
      );

    shader.fragmentShader =
      `precision highp sampler2DArray;
uniform sampler2D uDetail;
uniform sampler2D uRiverMask;
uniform sampler2D uSplatMap;
uniform sampler2D uHorizonMap;
uniform float uUseBakedHorizon;
uniform vec3 uSunDir;
uniform float uTime;
uniform float uPbrEnabled;
uniform float uHeightUnits;
uniform sampler2DArray uPbrAlbedo;
uniform sampler2DArray uPbrNormal;
varying vec3 vW3WorldPos;
varying vec3 vW3WorldNormal;

vec4 w3SampleAntiTile(sampler2DArray texArr, vec2 wp, float layer) {
  vec2 uvA = wp * 0.085;
  vec2 rotWp = vec2(wp.x * 0.7986 - wp.y * 0.6018, wp.x * 0.6018 + wp.y * 0.7986);
  vec2 uvB = rotWp * 0.021 + vec2(0.37, 0.71);
  vec4 a = texture(texArr, vec3(uvA, layer));
  vec4 b = texture(texArr, vec3(uvB, layer));
  return mix(a, b, 0.44);
}

vec4 w3SampleTriplanarRock(sampler2DArray texArr, vec3 wp, vec3 wn) {
  vec3 blend = abs(wn);
  blend = max(blend - vec3(0.2), vec3(0.001));
  blend /= (blend.x + blend.y + blend.z);
  vec4 cx = w3SampleAntiTile(texArr, wp.zy, 2.0);
  vec4 cy = w3SampleAntiTile(texArr, wp.xz, 2.0);
  vec4 cz = w3SampleAntiTile(texArr, wp.xy, 2.0);
  return cx * blend.x + cy * blend.y + cz * blend.z;
}

void w3ComputeLayerWeights(
  vec2 mapUv,
  out float wGrass,
  out float wForest,
  out float wRock,
  out float wSand,
  out float wSnow,
  out float wMud
) {
  vec4 spl = texture2D(uSplatMap, mapUv);
  vec3 wn = normalize(vW3WorldNormal);
  float slopeAmt = 1.0 - clamp(wn.y, 0.0, 1.0);
  wRock = smoothstep(0.16, 0.44, slopeAmt);

  float hNorm = clamp(vW3WorldPos.y / max(uHeightUnits, 1.0), 0.0, 1.0);
  float snowNoise = texture2D(uDetail, vW3WorldPos.xz * 0.012).r - 0.5;
  float snowHeight = smoothstep(0.62, 0.80, hNorm + snowNoise * 0.14) * (1.0 - wRock * 0.9);
  wSnow = max(spl.a * (1.0 - wRock * 0.85), snowHeight);

  float rem = max(0.0, 1.0 - wRock - wSnow);
  float specSum = max(0.001, spl.r + spl.g + spl.b);
  float specScale = min(1.0, 1.0 / max(1.0, specSum));
  wForest = spl.r * specScale * rem;
  wSand = spl.g * specScale * rem;
  wMud = spl.b * specScale * rem;
  wGrass = max(0.0, rem - wForest - wSand - wMud);

  float total = max(0.001, wGrass + wForest + wRock + wSand + wSnow + wMud);
  wGrass /= total;
  wForest /= total;
  wRock /= total;
  wSand /= total;
  wSnow /= total;
  wMud /= total;
}
` +
      shader.fragmentShader
        .replace(
          '#include <map_fragment>',
          `#include <map_fragment>
          {
            float dtl = texture2D(uDetail, vMapUv * 220.0).r * 2.0;
            vec4 rivMask = texture2D(uRiverMask, vMapUv);
            float riv = rivMask.r;
            if (uPbrEnabled > 0.5) {
              float wG, wF, wR, wSa, wSn, wM;
              w3ComputeLayerWeights(vMapUv, wG, wF, wR, wSa, wSn, wM);
              vec3 wn = normalize(vW3WorldNormal);
              vec3 pbrSrgb = vec3(0.0);
              if (wG > 0.01) pbrSrgb += w3SampleAntiTile(uPbrAlbedo, vW3WorldPos.xz, 0.0).rgb * wG;
              if (wF > 0.01) pbrSrgb += w3SampleAntiTile(uPbrAlbedo, vW3WorldPos.xz, 1.0).rgb * wF;
              if (wR > 0.01) {
                vec3 rockRgb = w3SampleTriplanarRock(uPbrAlbedo, vW3WorldPos, wn).rgb;
                float rockLum = dot(rockRgb, vec3(0.299, 0.587, 0.114));
                pbrSrgb += mix(rockRgb, vec3(rockLum), 0.72) * wR;
              }
              if (wSa > 0.01) pbrSrgb += w3SampleAntiTile(uPbrAlbedo, vW3WorldPos.xz, 3.0).rgb * wSa;
              if (wSn > 0.01) pbrSrgb += w3SampleAntiTile(uPbrAlbedo, vW3WorldPos.xz, 4.0).rgb * wSn;
              if (wM > 0.01) pbrSrgb += w3SampleAntiTile(uPbrAlbedo, vW3WorldPos.xz, 5.0).rgb * wM;

              vec3 pbrLin = pow(max(pbrSrgb, vec3(0.001)), vec3(2.2));
              float pbrLuma = dot(pbrLin, vec3(0.299, 0.587, 0.114));
              float pbrDistFade = 1.0 - smoothstep(180.0, 620.0, length(vViewPosition));
              float microContrast = mix(0.68, 1.36, clamp(pbrLuma * 1.5, 0.0, 1.0));
              vec3 tintedPbr = diffuseColor.rgb * microContrast;
              float rawMix = mix(0.05, 0.22, pbrDistFade);
              vec3 pbrBlended = mix(tintedPbr, pbrLin * mix(0.75, 1.2, microContrast), rawMix);
              pbrBlended *= mix(1.0, dtl, 0.16);
              diffuseColor.rgb = mix(pbrBlended, diffuseColor.rgb, clamp(riv * 0.85, 0.0, 0.85));
            } else {
              diffuseColor.rgb *= mix(1.0, dtl, 0.34);
            }
            if (riv > 0.01) {
              // C2 downstream river advection: visual cue derived strictly from the D8 elevation
              // gradient (never simulated hydrodynamics), using Vlachos (SIGGRAPH 2010) dual
              // half-cycle flow-map blending with per-pixel noise phase jitter to eliminate pulse seams.
              vec2 rawFlow = (rivMask.gb * 255.0 - 128.0) / 127.0;
              float flowLen = length(rawFlow);
              float validFlow = smoothstep(0.35, 0.75, rivMask.a) * step(0.2, flowLen);
              vec2 flowDir = flowLen > 0.2 ? (rawFlow / flowLen) : vec2(0.0);

              float phaseJitter = texture2D(uDetail, vMapUv * 24.0).r;
              float p0 = fract(uTime * 0.30 + phaseJitter);
              float p1 = fract(uTime * 0.30 + phaseJitter + 0.5);
              float blend = abs(2.0 * p0 - 1.0);
              vec2 advUvBase = vMapUv * 220.0;
              float adv0 = texture2D(uDetail, advUvBase - flowDir * (p0 * 1.25)).r * 2.0;
              float adv1 = texture2D(uDetail, advUvBase - flowDir * (p1 * 1.25)).r * 2.0;
              float advected = mix(adv0, adv1, blend);

              float flow1 = texture2D(uDetail, vMapUv * 260.0 + vec2(uTime * 0.025, uTime * 0.045)).r * 2.0;
              float flow2 = texture2D(uDetail, vMapUv * 140.0 - vec2(uTime * 0.018, uTime * 0.03)).r * 2.0;
              float fallbackFlow = flow1 * 0.55 + flow2 * 0.45;
              float flowMod = mix(fallbackFlow, advected, validFlow);
              diffuseColor.rgb *= mix(1.0, flowMod, riv * 0.32);
            }
            if (uUseBakedHorizon > 0.5) {
              vec4 hzn = texture2D(uHorizonMap, vMapUv) * 1.5707963;
              vec3 sd = normalize(uSunDir);
              vec4 azW = vec4(
                max(sd.x, 0.0),
                max(-sd.x, 0.0),
                max(-sd.z, 0.0),
                max(sd.z, 0.0)
              );
              float wSum = max(0.0001, azW.x + azW.y + azW.z + azW.w);
              float thetaH = dot(hzn, azW) / wSum;
              float sunElev = asin(clamp(sd.y, -1.0, 1.0));
              float sunVis = smoothstep(thetaH - 0.08, thetaH + 0.14, sunElev);
              diffuseColor.rgb *= mix(0.45, 1.0, sunVis);
            }
          }`,
        )
        .replace(
          '#include <normal_fragment_maps>',
          `#include <normal_fragment_maps>
          {
            float distFade = (1.0 - smoothstep(140.0, 520.0, length(vViewPosition))) * uPbrEnabled;
            if (distFade > 0.01) {
              float wG, wF, wR, wSa, wSn, wM;
              w3ComputeLayerWeights(vMapUv, wG, wF, wR, wSa, wSn, wM);
              vec3 wn = normalize(vW3WorldNormal);
              vec3 pbrNs = vec3(0.0);
              if (wG > 0.01) pbrNs += w3SampleAntiTile(uPbrNormal, vW3WorldPos.xz, 0.0).xyz * wG;
              if (wF > 0.01) pbrNs += w3SampleAntiTile(uPbrNormal, vW3WorldPos.xz, 1.0).xyz * wF;
              if (wR > 0.01) pbrNs += w3SampleTriplanarRock(uPbrNormal, vW3WorldPos, wn).xyz * wR;
              if (wSa > 0.01) pbrNs += w3SampleAntiTile(uPbrNormal, vW3WorldPos.xz, 3.0).xyz * wSa;
              if (wSn > 0.01) pbrNs += w3SampleAntiTile(uPbrNormal, vW3WorldPos.xz, 4.0).xyz * wSn;
              if (wM > 0.01) pbrNs += w3SampleAntiTile(uPbrNormal, vW3WorldPos.xz, 5.0).xyz * wM;
              vec3 microN = pbrNs * 2.0 - 1.0;
              mapN.xy += microN.xy * (0.65 * distFade);
              normal = normalize(tbn * mapN);
            }
          }`,
        );
  };

  material.customProgramCacheKey = () =>
    look ? 'wv2-look-terrain-v2' : hasHorizon ? 'w3-lite-terrain-pbr-v4-hzn' : 'w3-lite-terrain-pbr-v3';

  const handle: TerrainPbrShaderHandle = {
    applyPbrArrays(arrays) {
      if (arrays) {
        uniforms.uPbrAlbedo.value = arrays.albedoArray;
        uniforms.uPbrNormal.value = arrays.normalArray;
        uniforms.uPbrEnabled.value = 1;
      } else {
        uniforms.uPbrAlbedo.value = dummyAlbedo;
        uniforms.uPbrNormal.value = dummyNormal;
        uniforms.uPbrEnabled.value = 0;
      }
    },
    updateTime(timeSec) {
      uniforms.uTime.value = timeSec;
    },
    updateSunDir(sunDir) {
      uniforms.uSunDir.value.set(sunDir[0], sunDir[1], sunDir[2]);
    },
    dispose() {
      dummyAlbedo.dispose();
      dummyNormal.dispose();
      dummyHorizon.dispose();
      dummyShore.dispose();
      material.dispose();
    },
  };

  return { material, handle };
}
