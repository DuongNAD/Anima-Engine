import * as THREE from 'three';

// prototype, to be replaced — WV2-LOOK aerial perspective shared by the World v2 terrain, flora
// and ocean.
//
// The sky dome writes its gradient straight to the framebuffer (no tone mapping, no colour-space
// conversion), so a distant ridge can only melt into it if the haze is mixed in the same space,
// AFTER three's `tonemapping_fragment` + `colorspace_fragment`: exactly where `fog_fragment`
// sits. `ATMOSPHERE_FRAGMENT_APPLY` therefore replaces `#include <fog_fragment>` and reuses the
// dome's own horizon formula through one set of shared uniforms that `WorldSky` refreshes every
// frame.
//
// The haze is a height-attenuated exponential: density falls off with altitude above sea level,
// and the optical depth along the ray is integrated in closed form. A camera near the ground
// sees distant ridges fade to the sky; the overview camera, looking down through thin air,
// keeps the map readable.

export interface AtmosphereUniforms {
  uAtmoHorizon: { value: THREE.Color };
  uAtmoZenith: { value: THREE.Color };
  uAtmoSunDir: { value: THREE.Vector3 };
  uAtmoSunColor: { value: THREE.Color };
  uAtmoGlow: { value: number };
  /** Extinction per scene unit at sea level. */
  uAtmoDensity: { value: number };
  /** Haze scale height, scene units. */
  uAtmoScaleH: { value: number };
  /** Sea level, scene units (world Y). */
  uAtmoSeaY: { value: number };
  /** Low marine/spray layer: extinction per unit at sea level, and its scale height (units). */
  uAtmoMarineDensity: { value: number };
  uAtmoMarineH: { value: number };
}

/** One live set, written by `WorldSky`, read by every World v2 material that hazes. */
export const atmosphereUniforms: AtmosphereUniforms = {
  uAtmoHorizon: { value: new THREE.Color('#cfe6f2') },
  uAtmoZenith: { value: new THREE.Color('#3f7fd0') },
  uAtmoSunDir: { value: new THREE.Vector3(0, 1, 0) },
  uAtmoSunColor: { value: new THREE.Color('#fff2cc') },
  uAtmoGlow: { value: 1 },
  uAtmoDensity: { value: 0.0011 },
  uAtmoScaleH: { value: 60 },
  uAtmoSeaY: { value: 35 },
  uAtmoMarineDensity: { value: 0 },
  uAtmoMarineH: { value: 1 },
};

/** Configure the haze for a scene of `renderSize` units whose sea surface sits at `seaY`. */
export function configureAtmosphere(renderSize: number, heightUnits: number, seaY: number): void {
  // ~50% haze at ~0.55 * renderSize (≈ 23 km on the 41 km C01 world) for a sea-level ray.
  atmosphereUniforms.uAtmoDensity.value = Math.LN2 / (renderSize * 0.55);
  // Haze scale height ≈ 2.2 km of the 7.2 km elevation span.
  atmosphereUniforms.uAtmoScaleH.value = heightUnits * (2200 / 7200);
  atmosphereUniforms.uAtmoSeaY.value = seaY;
  // A thin salt-spray/marine layer (~60 m scale height): ~15% extra haze over 2 km at sea
  // level, so surf-side cliffs and headlands behind the surf lose contrast (CST-01). Cameras
  // above a few hundred metres look through almost none of it.
  atmosphereUniforms.uAtmoMarineDensity.value = -Math.log(0.85) / ((2000 / 40960) * renderSize);
  atmosphereUniforms.uAtmoMarineH.value = heightUnits * (60 / 7200);
}

/** Attach the shared uniforms to a shader passed through `onBeforeCompile`. */
export function bindAtmosphereUniforms(uniforms: Record<string, THREE.IUniform>): void {
  for (const [k, v] of Object.entries(atmosphereUniforms)) uniforms[k] = v;
}

export const ATMOSPHERE_FRAGMENT_PARS = /* glsl */ `
uniform vec3 uAtmoHorizon;
uniform vec3 uAtmoZenith;
uniform vec3 uAtmoSunDir;
uniform vec3 uAtmoSunColor;
uniform float uAtmoGlow;
uniform float uAtmoDensity;
uniform float uAtmoScaleH;
uniform float uAtmoSeaY;
uniform float uAtmoMarineDensity;
uniform float uAtmoMarineH;

vec3 atmoSkyColor(vec3 d) {
  float h = clamp(d.y, 0.0, 1.0);
  vec3 col = mix(uAtmoHorizon, uAtmoZenith, pow(h, 0.58));
  float dayMix = smoothstep(0.18, 0.72, max(uAtmoHorizon.r, max(uAtmoHorizon.g, uAtmoHorizon.b)));
  vec3 hazeBelt = mix(uAtmoHorizon, vec3(0.77, 0.87, 0.94), dayMix * 0.72);
  col = mix(hazeBelt, col, smoothstep(0.0, 0.16, abs(d.y)));
  float s = max(dot(d, normalize(uAtmoSunDir)), 0.0);
  col += uAtmoSunColor * pow(s, 10.0) * 0.16 * uAtmoGlow;
  return col;
}

// Mean of exp(-y/H) along a straight ray from height y0 to y1 (closed form).
float atmoLayerMean(float y0, float y1, float H) {
  float dy = y1 - y0;
  float e0 = exp(-y0 / H);
  float e1 = exp(-y1 / H);
  return abs(dy) > 0.02 * H ? (e0 - e1) * H / dy : e0;
}

float atmoHazeAmount(vec3 camP, vec3 p) {
  float d = length(p - camP);
  float y0 = max(camP.y - uAtmoSeaY, 0.0);
  float y1 = max(p.y - uAtmoSeaY, 0.0);
  float tau = uAtmoDensity * atmoLayerMean(y0, y1, max(uAtmoScaleH, 1.0))
            + uAtmoMarineDensity * atmoLayerMean(y0, y1, max(uAtmoMarineH, 0.05));
  return 1.0 - exp(-d * tau);
}

vec3 atmoApply(vec3 col, vec3 p) {
  vec3 dir = normalize(p - cameraPosition);
  vec3 hazeCol = atmoSkyColor(normalize(vec3(dir.x, dir.y * 0.2, dir.z)));
  return mix(col, hazeCol, atmoHazeAmount(cameraPosition, p));
}
`;

/** Replacement for `#include <fog_fragment>`; `worldPosExpr` is a vec3 world-space position. */
export function atmosphereFragmentApply(worldPosExpr: string): string {
  return `gl_FragColor.rgb = atmoApply( gl_FragColor.rgb, ${worldPosExpr} );`;
}
