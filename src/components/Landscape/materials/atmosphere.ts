import * as THREE from 'three';
import { buildLookNoiseTexture } from '../utils/worldTerrainTextures';

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
  /** Physical (Preetham) sky weight: 1 by day on the World v2 look path, 0 = legacy gradient. */
  uAtmoPhys: { value: number };
  uAtmoBetaR: { value: THREE.Vector3 };
  uAtmoBetaM: { value: THREE.Vector3 };
  uAtmoSunE: { value: number };
  uAtmoMieG: { value: number };
  uAtmoSkyExposure: { value: number };
  /** Display-space Preetham samples for the cheap per-pixel sky (haze, reflections, clouds). */
  uAtmoZenP: { value: THREE.Color };
  uAtmoHzSun: { value: THREE.Color };
  uAtmoHzSide: { value: THREE.Color };
  uAtmoHzAway: { value: THREE.Color };
  /**
   * Cumulus slab: 0 = off; global cloudiness offset; base height and depth (units); metres per
   * unit horizontally and vertically; wind (m/s).
   */
  uAtmoCloudOn: { value: number };
  uAtmoCloudCover: { value: number };
  uAtmoCloudY: { value: number };
  uAtmoCloudTop: { value: number };
  uAtmoMPerUnit: { value: number };
  uAtmoMPerUnitY: { value: number };
  uAtmoWind: { value: THREE.Vector2 };
  uAtmoTime: { value: number };
  uAtmoCloudTex: { value: THREE.Texture | null };
  /**
   * 512^2 RG over twice the map (centred): r = max-pooled ground height (units), g = terrain
   * cloud forcing 0..1. `uAtmoRenderSize` is the map's size in units.
   */
  uAtmoGroundTex: { value: THREE.Texture | null };
  uAtmoRenderSize: { value: number };
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
  uAtmoPhys: { value: 0 },
  uAtmoBetaR: { value: new THREE.Vector3() },
  uAtmoBetaM: { value: new THREE.Vector3() },
  uAtmoSunE: { value: 0 },
  uAtmoMieG: { value: 0.8 },
  uAtmoSkyExposure: { value: 0.25 },
  uAtmoZenP: { value: new THREE.Color(0.23, 0.47, 0.75) },
  uAtmoHzSun: { value: new THREE.Color(0.95, 0.96, 0.97) },
  uAtmoHzSide: { value: new THREE.Color(0.91, 0.93, 0.94) },
  uAtmoHzAway: { value: new THREE.Color(0.88, 0.9, 0.91) },
  uAtmoCloudOn: { value: 0 },
  uAtmoCloudCover: { value: 0 },
  uAtmoCloudY: { value: 120 },
  uAtmoCloudTop: { value: 50 },
  uAtmoMPerUnit: { value: 34 },
  uAtmoMPerUnitY: { value: 43 },
  // The generator's prevailing wind blows west to east (anima-worldgen `climate.rs`, u_x > 0),
  // so the wet windward slopes face -x: drift the cells along +x.
  uAtmoWind: { value: new THREE.Vector2(7.5, 0.9) },
  uAtmoTime: { value: 0 },
  uAtmoCloudTex: { value: null },
  uAtmoGroundTex: { value: null },
  uAtmoRenderSize: { value: 1200 },
};

// ---- Preetham sky (the model in three/examples/jsm/objects/Sky.js), parameters per sun ----------
const TOTAL_RAYLEIGH = [5.804542996261093e-6, 1.3562911419845635e-5, 3.0265902468824876e-5];
const MIE_CONST = [1.8399918514433978e14, 2.7798023919660528e14, 4.0790479543861094e14];
const SKY_TURBIDITY = 2.5;
const SKY_RAYLEIGH = 1.2;
const SKY_MIE_COEFF = 0.005;

/**
 * Per-frame sky inputs for a unit sun direction. `physWeight` blends the Preetham sky in (day)
 * against the legacy gradient (night, where Preetham has no moon or stars).
 */
export function updateAtmosphereSun(sunDir: THREE.Vector3, physWeight: number): void {
  const cutoff = 1.6110731556870734;
  const zen = Math.acos(THREE.MathUtils.clamp(sunDir.y, -1, 1));
  atmosphereUniforms.uAtmoSunE.value = 1000 * Math.max(0, 1 - Math.exp(-((cutoff - zen) / 1.5)));
  atmosphereUniforms.uAtmoBetaR.value.set(
    TOTAL_RAYLEIGH[0] * SKY_RAYLEIGH,
    TOTAL_RAYLEIGH[1] * SKY_RAYLEIGH,
    TOTAL_RAYLEIGH[2] * SKY_RAYLEIGH,
  );
  const c = 0.2 * SKY_TURBIDITY * 10e-18;
  atmosphereUniforms.uAtmoBetaM.value.set(
    0.434 * c * MIE_CONST[0] * SKY_MIE_COEFF,
    0.434 * c * MIE_CONST[1] * SKY_MIE_COEFF,
    0.434 * c * MIE_CONST[2] * SKY_MIE_COEFF,
  );
  atmosphereUniforms.uAtmoPhys.value = physWeight;
  // Samples for the cheap per-pixel sky: zenith, and the horizon toward, beside and away from
  // the sun (exactly the dome's colours there, so the haze meets the dome without a seam).
  tmpSun.copy(sunDir).normalize();
  const hx = tmpSun.x;
  const hz = tmpSun.z;
  const hl = Math.hypot(hx, hz) || 1;
  preethamDisplay(tmpDir.set(0, 1, 0), tmpSun, atmosphereUniforms.uAtmoZenP.value);
  preethamDisplay(tmpDir.set(hx / hl, 0, hz / hl), tmpSun, atmosphereUniforms.uAtmoHzSun.value);
  preethamDisplay(tmpDir.set(-hz / hl, 0, hx / hl), tmpSun, atmosphereUniforms.uAtmoHzSide.value);
  preethamDisplay(tmpDir.set(-hx / hl, 0, -hz / hl), tmpSun, atmosphereUniforms.uAtmoHzAway.value);
}

// JS twin of `atmoPreetham` (same constants, same ACES fit), for the few samples the cheap
// per-pixel sky interpolates between.
function acesFit(c: number[]): number[] {
  const inM = [
    [0.59719, 0.076, 0.0284],
    [0.35458, 0.90834, 0.13383],
    [0.04823, 0.01566, 0.83777],
  ];
  const outM = [
    [1.60475, -0.10208, -0.00327],
    [-0.53108, 1.10813, -0.07276],
    [-0.07367, -0.00605, 1.07602],
  ];
  const mul = (m: number[][], v: number[]) => [0, 1, 2].map((i) => m[0][i] * v[0] + m[1][i] * v[1] + m[2][i] * v[2]);
  const x = mul(inM, c).map((v) => (v * (v + 0.0245786) - 0.000090537) / (v * (0.983729 * v + 0.432951) + 0.238081));
  return mul(outM, x).map((v) => Math.min(1, Math.max(0, v)));
}

function preethamDisplay(d: THREE.Vector3, sun: THREE.Vector3, out: THREE.Color): void {
  const u = atmosphereUniforms;
  const za = Math.acos(Math.max(0, d.y));
  const inv = 1 / (Math.cos(za) + 0.15 * Math.pow(93.885 - za * 57.29578, -1.253));
  const bR = [u.uAtmoBetaR.value.x, u.uAtmoBetaR.value.y, u.uAtmoBetaR.value.z];
  const bM = [u.uAtmoBetaM.value.x, u.uAtmoBetaM.value.y, u.uAtmoBetaM.value.z];
  const ct = d.dot(sun);
  const rP = 0.05968310365946075 * (1 + Math.pow(ct * 0.5 + 0.5, 2));
  const g = u.uAtmoMieG.value;
  const mP = (0.07957747154594767 * (1 - g * g)) / Math.pow(1 - 2 * g * ct + g * g, 1.5);
  const mixT = Math.min(1, Math.max(0, Math.pow(1 - sun.y, 5)));
  const c: number[] = [];
  for (let i = 0; i < 3; i += 1) {
    const fex = Math.exp(-(bR[i] * 8.4e3 * inv + bM[i] * 1.25e3 * inv));
    const ratio = (bR[i] * rP + bM[i] * mP) / (bR[i] + bM[i]);
    let lin = Math.pow(u.uAtmoSunE.value * ratio * (1 - fex), 1.5);
    lin *= 1 - mixT + mixT * Math.pow(u.uAtmoSunE.value * ratio * fex, 0.5);
    c.push((lin + 0.1 * fex) * 0.04 + [0, 0.0003, 0.00075][i]);
  }
  const t = acesFit(c.map((v) => (v * u.uAtmoSkyExposure.value) / 0.6));
  out.setRGB(Math.pow(t[0], 1 / 2.2), Math.pow(t[1], 1 / 2.2), Math.pow(t[2], 1 / 2.2), THREE.LinearSRGBColorSpace);
}

const tmpDir = new THREE.Vector3();
const tmpSun = new THREE.Vector3();

let cloudNoiseTex: THREE.DataTexture | null = null;


/**
 * Configure the haze and cloud layer for a scene of `renderSize` units showing a world of
 * `extentM` metres whose elevation span `elevSpanM` maps to `heightUnits`, sea surface at `seaY`.
 * Every distance is set in metres and converted, so a larger world keeps the same look.
 */
export function configureAtmosphere(
  renderSize: number,
  heightUnits: number,
  seaY: number,
  extentM: number,
  elevSpanM: number,
): void {
  const unitsPerM = renderSize / extentM;
  const unitsPerMY = heightUnits / elevSpanM;
  // ~50% haze over ~23 km for a sea-level ray.
  atmosphereUniforms.uAtmoDensity.value = Math.LN2 / (23000 * unitsPerM);
  // Haze scale height ~2.2 km.
  atmosphereUniforms.uAtmoScaleH.value = 2200 * unitsPerMY;
  atmosphereUniforms.uAtmoSeaY.value = seaY;
  // A thin salt-spray/marine layer (~60 m scale height): ~15% extra haze over 2 km at sea
  // level, so surf-side cliffs and headlands behind the surf lose contrast (CST-01). Cameras
  // above a few hundred metres look through almost none of it.
  atmosphereUniforms.uAtmoMarineDensity.value = -Math.log(0.85) / (2000 * unitsPerM);
  atmosphereUniforms.uAtmoMarineH.value = 60 * unitsPerMY;
  // Trade cumulus: flat bases ~1.9 km above the sea, tops up to ~2.4 km above that.
  atmosphereUniforms.uAtmoCloudY.value = seaY + 1900 * unitsPerMY;
  atmosphereUniforms.uAtmoCloudTop.value = 2400 * unitsPerMY;
  atmosphereUniforms.uAtmoMPerUnit.value = 1 / unitsPerM;
  atmosphereUniforms.uAtmoMPerUnitY.value = 1 / unitsPerMY;
  if (!cloudNoiseTex) cloudNoiseTex = buildLookNoiseTexture();
  atmosphereUniforms.uAtmoCloudTex.value = cloudNoiseTex;
}

let groundTex: THREE.DataTexture | null = null;

/**
 * Terrain inputs of the cumulus, as one 512^2 RG float texture spanning twice the map (centred),
 * so island cloud trails can run out over the sea:
 *  - r: max-pooled ground height in scene units, so the slab stops at the terrain;
 *  - g: where the land makes cloud. Daytime heating over land (more over wet ground: `moisture`
 *    is the generator's own rain field), lift where the slope rises into the prevailing wind,
 *    and high ground; then carried downwind as a fading trail and blurred.
 * `elevation`/`moisture` are the world's `size^2` grids (elevation normalised to `heightUnits`).
 */
export function setAtmosphereGround(
  elevation: Float32Array,
  moisture: Float32Array | null,
  size: number,
  heightUnits: number,
  renderSize: number,
): void {
  const R = 512;
  const H = R / 2; // map cells span the inner half of the texture
  const step = size / H;
  const u = atmosphereUniforms;
  const mPerUY = u.uAtmoMPerUnitY.value;
  const seaY = u.uAtmoSeaY.value;
  const texelM = (2 * renderSize * u.uAtmoMPerUnit.value) / R;
  const gmax = new Float32Array(R * R);
  const hm = new Float32Array(R * R); // mean height above the sea (m)
  const wet = new Float32Array(R * R);
  for (let y = 0; y < H; y += 1) {
    for (let x = 0; x < H; x += 1) {
      let m = 0;
      let sum = 0;
      let w = 0;
      let n = 0;
      const y0 = Math.floor(y * step);
      const x0 = Math.floor(x * step);
      for (let yy = y0; yy < Math.min(size, Math.floor((y + 1) * step)); yy += 1) {
        for (let xx = x0; xx < Math.min(size, Math.floor((x + 1) * step)); xx += 1) {
          const e = elevation[yy * size + xx];
          if (e > m) m = e;
          sum += e;
          w += moisture ? moisture[yy * size + xx] : 0.5;
          n += 1;
        }
      }
      const o = (y + H / 2) * R + x + H / 2;
      gmax[o] = m * heightUnits;
      hm[o] = Math.max(0, ((sum / Math.max(n, 1)) * heightUnits - seaY) * mPerUY);
      wet[o] = w / Math.max(n, 1);
    }
  }
  const clampI = (v: number) => Math.min(R - 1, Math.max(0, v));
  const blur = (a: Float32Array, r: number) => {
    const t = new Float32Array(R * R);
    for (let y = 0; y < R; y += 1) {
      for (let x = 0; x < R; x += 1) {
        let acc = 0;
        for (let k = -r; k <= r; k += 1) acc += a[y * R + clampI(x + k)];
        t[y * R + x] = acc / (2 * r + 1);
      }
    }
    for (let y = 0; y < R; y += 1) {
      for (let x = 0; x < R; x += 1) {
        let acc = 0;
        for (let k = -r; k <= r; k += 1) acc += t[clampI(y + k) * R + x];
        a[y * R + x] = acc / (2 * r + 1);
      }
    }
  };
  blur(hm, 4);
  blur(wet, 4);
  const wind = u.uAtmoWind.value.clone().normalize();
  const at = (a: Float32Array, x: number, y: number) => a[clampI(Math.round(y)) * R + clampI(Math.round(x))];
  const f0 = new Float32Array(R * R);
  for (let y = 1; y < R - 1; y += 1) {
    for (let x = 1; x < R - 1; x += 1) {
      const i = y * R + x;
      const h = hm[i];
      const land = THREE.MathUtils.smoothstep(h, 10, 250);
      // Rise per metre along the wind, measured over ~3 texels.
      const rise = (h - at(hm, x - wind.x * 3, y - wind.y * 3)) / (3 * texelM);
      f0[i] =
        land * (0.08 + 0.3 * wet[i]) +
        0.55 * THREE.MathUtils.smoothstep(rise, 0.01, 0.1) +
        0.2 * THREE.MathUtils.smoothstep(h, 800, 2500);
    }
  }
  const forcing = new Float32Array(R * R);
  for (let y = 0; y < R; y += 1) {
    for (let x = 0; x < R; x += 1) {
      let f = 0;
      for (let k = 0; k < 14; k += 1) f = Math.max(f, at(f0, x - wind.x * k * 2, y - wind.y * k * 2) * 0.92 ** k);
      forcing[y * R + x] = Math.min(1, f);
    }
  }
  blur(forcing, 3);
  const data = new Float32Array(R * R * 2);
  for (let i = 0; i < R * R; i += 1) {
    const edge = i < R || i >= R * (R - 1) || i % R === 0 || i % R === R - 1;
    data[i * 2] = gmax[i];
    data[i * 2 + 1] = edge ? 0 : forcing[i];
  }
  groundTex?.dispose();
  groundTex = new THREE.DataTexture(data, R, R, THREE.RGFormat, THREE.FloatType);
  groundTex.minFilter = THREE.LinearFilter;
  groundTex.magFilter = THREE.LinearFilter;
  groundTex.wrapS = THREE.ClampToEdgeWrapping;
  groundTex.wrapT = THREE.ClampToEdgeWrapping;
  groundTex.needsUpdate = true;
  u.uAtmoGroundTex.value = groundTex;
  u.uAtmoRenderSize.value = renderSize;
}

/** Turn the cumulus (and its shadows) on or off. */
export function setAtmosphereClouds(on: boolean): void {
  atmosphereUniforms.uAtmoCloudOn.value = on ? 1 : 0;
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
uniform float uAtmoPhys;
uniform vec3 uAtmoBetaR;
uniform vec3 uAtmoBetaM;
uniform float uAtmoSunE;
uniform float uAtmoMieG;
uniform float uAtmoSkyExposure;
uniform vec3 uAtmoZenP;
uniform vec3 uAtmoHzSun;
uniform vec3 uAtmoHzSide;
uniform vec3 uAtmoHzAway;
uniform float uAtmoCloudOn;
uniform float uAtmoCloudCover;
uniform float uAtmoCloudY;
uniform float uAtmoCloudTop;
uniform float uAtmoMPerUnit;
uniform float uAtmoMPerUnitY;
uniform vec2 uAtmoWind;
uniform float uAtmoTime;
uniform sampler2D uAtmoCloudTex;
uniform sampler2D uAtmoGroundTex;
uniform float uAtmoRenderSize;
` +
  // Preetham sky radiance (as three/examples/jsm/objects/Sky.js), tone-mapped with three's ACES fit
  // and sRGB-encoded here, so the dome, the haze and the water's sky reflection share one
  // display-space colour and meet without a seam.
  `vec3 atmoAcesFit(vec3 c){
const mat3 inM=mat3(vec3(0.59719,0.07600,0.02840),vec3(0.35458,0.90834,0.13383),vec3(0.04823,0.01566,0.83777));
const mat3 outM=mat3(vec3(1.60475,-0.10208,-0.00327),vec3(-0.53108,1.10813,-0.07276),vec3(-0.07367,-0.00605,1.07602));
c=inM*c;
vec3 a=c*(c+0.0245786)-0.000090537;
vec3 b=c*(0.983729*c+0.4329510)+0.238081;
return clamp(outM*(a/b),0.0,1.0);
}
vec3 atmoPreetham(vec3 d){
vec3 sd=normalize(uAtmoSunDir);
float za=acos(max(0.0,d.y));
float inv=1.0/(cos(za)+0.15*pow(93.885-za*57.29578,-1.253));
vec3 fex=exp(-(uAtmoBetaR*8.4e3*inv+uAtmoBetaM*1.25e3*inv));
float ct=dot(d,sd);
float rPhase=0.05968310365946075*(1.0+pow(ct*0.5+0.5,2.0));
float g2=uAtmoMieG*uAtmoMieG;
float mPhase=0.07957747154594767*(1.0-g2)/pow(1.0-2.0*uAtmoMieG*ct+g2,1.5);
vec3 ratio=(uAtmoBetaR*rPhase+uAtmoBetaM*mPhase)/(uAtmoBetaR+uAtmoBetaM);
vec3 lin=pow(uAtmoSunE*ratio*(1.0-fex),vec3(1.5));
lin*=mix(vec3(1.0),pow(uAtmoSunE*ratio*fex,vec3(0.5)),clamp(pow(1.0-sd.y,5.0),0.0,1.0));
vec3 c=(lin+vec3(0.1)*fex)*0.04+vec3(0.0,0.0003,0.00075);
c=atmoAcesFit(c*uAtmoSkyExposure/0.6);
return pow(c,vec3(1.0/2.2));
}
` +
  // Terrain inputs at pM (metres, world xz): r = ground height (units), g = land cloud forcing.
  // Border texels are zero, so beyond the texture the sea gets none.
  `vec2 atmoCloudDom(vec2 pM){
return texture2D(uAtmoGroundTex,pM/(uAtmoMPerUnit*uAtmoRenderSize*2.0)+0.5).rg;
}
` +
  // Cloudiness 0..1 at pM: the land forcing, plus open-sea weather that drifts with the wind:
  // ~100 km regions decide clear versus cloudy sea, and inside the cloudy ones the cells line
  // up in wind-parallel streets ~4 km apart. `region` is sampled once per pixel by the caller.
  `float atmoCloudC(vec2 pM,float land,float region){
vec2 q=pM-uAtmoWind*uAtmoTime;
vec2 w=normalize(uAtmoWind);
float street=0.5+0.5*cos(dot(q,vec2(-w.y,w.x))*0.0015+region*7.0);
float sea=smoothstep(0.45,0.8,region)*(0.12+0.26*street);
return clamp(sea+land*0.5+uAtmoCloudCover,0.0,0.8);
}
float atmoCloudRegion(vec2 pM){
vec4 r=texture2D(uAtmoCloudTex,(pM-uAtmoWind*uAtmoTime)/97000.0+0.53);
return r.a*0.7+r.b*0.3;
}
` +
  // Local cloud top as a fraction of the slab (0 = clear sky) for cloudiness C. Noise in
  // wind-aligned coordinates (cells stretched along the wind) is thresholded at 1 - C, so a
  // cloudy region merges into wide fields and a clear one keeps a few small cells; inside a cell
  // the top rises like a dome, higher where it is cloudier.
  `float atmoCloudTop(vec2 pM,float C,int oct){
vec2 q=pM-uAtmoWind*uAtmoTime;
vec2 w=normalize(uAtmoWind);
vec2 a=vec2(dot(q,w)*0.62,dot(q,vec2(-w.y,w.x)));
float n=texture2D(uAtmoCloudTex,a/7200.0+0.13).r*0.6+texture2D(uAtmoCloudTex,a/2600.0+0.71).g*0.4;
if(oct>0)n+=(texture2D(uAtmoCloudTex,a/900.0+0.37).b-0.5)*0.22;
n=0.5+(n-0.5)*1.45;
float x=(n-(1.0-C))/max(C,0.05);
return x<=0.0?0.0:sqrt(min(x*1.8,1.0))*mix(0.25,1.0,C);
}
` +
  // Sun transmittance through the cumulus for a point on the ground (1 = clear sun): the same
  // cells, met where the sun ray crosses the lower slab.
  `float atmoCloudShadow(vec3 p){
if(uAtmoCloudOn<0.5)return 1.0;
vec3 sd=normalize(uAtmoSunDir);
if(sd.y<=0.02)return 1.0;
float lvl=uAtmoCloudY+0.2*uAtmoCloudTop;
` +
  // Ground at or above the cloud level has no cumulus over it (the puffs clear the massif).
  `if(p.y>=lvl)return 1.0;
vec3 c=p+sd*((lvl-p.y)/sd.y);
vec2 pM=c.xz*uAtmoMPerUnit;
vec2 dom=atmoCloudDom(pM);
float C=atmoCloudC(pM,dom.g,atmoCloudRegion(pM));
return 1.0-0.58*smoothstep(0.0,0.45,atmoCloudTop(pM,C,0))*(1.0-smoothstep(lvl-8.0,lvl,dom.r));
}
vec3 atmoSkyColorLegacy(vec3 d){
float h=clamp(d.y,0.0,1.0);
vec3 col=mix(uAtmoHorizon,uAtmoZenith,pow(h,0.58));
float dayMix=smoothstep(0.18,0.72,max(uAtmoHorizon.r,max(uAtmoHorizon.g,uAtmoHorizon.b)));
vec3 hazeBelt=mix(uAtmoHorizon,vec3(0.77,0.87,0.94),dayMix*0.72);
col=mix(hazeBelt,col,smoothstep(0.0,0.16,abs(d.y)));
float s=max(dot(d,normalize(uAtmoSunDir)),0.0);
col+=uAtmoSunColor*pow(s,10.0)*0.16*uAtmoGlow;
return col;
}
` +
  // Cheap per-pixel version of the physical sky for haze, reflections and clouds: the horizon
  // colour interpolated by azimuth to the sun between three exact samples, rising to the zenith
  // sample (the Preetham sky reaches its zenith colour within ~30 degrees), plus a Mie glow. The
  // exact model runs only on the dome.
  `vec3 atmoSkyFast(vec3 d){
vec2 sh=normalize(uAtmoSunDir.xz+vec2(1e-5));
float az=dot(normalize(d.xz+vec2(1e-5)),sh);
vec3 hz=az>0.0?mix(uAtmoHzSide,uAtmoHzSun,az*az):mix(uAtmoHzSide,uAtmoHzAway,az*az);
vec3 col=mix(hz,uAtmoZenP,pow(clamp(d.y,0.0,1.0),0.38));
float s=max(dot(d,normalize(uAtmoSunDir)),0.0);
return min(col+vec3(0.10,0.09,0.07)*pow(s,24.0),vec3(1.0));
}
vec3 atmoSkyColor(vec3 d){
if(uAtmoPhys<=0.0)return atmoSkyColorLegacy(d);
vec3 p=atmoSkyFast(d);
return uAtmoPhys>=1.0?p:mix(atmoSkyColorLegacy(d),p,uAtmoPhys);
}
` +
  // Mean of exp(-y/H) along a straight ray from height y0 to y1 (closed form).
  `float atmoLayerMean(float y0,float y1,float H){
float dy=y1-y0;
float e0=exp(-y0/H);
float e1=exp(-y1/H);
return abs(dy)>0.02*H?(e0-e1)*H/dy:e0;
}
float atmoHazeAmount(vec3 camP,vec3 p){
float d=length(p-camP);
float y0=max(camP.y-uAtmoSeaY,0.0);
float y1=max(p.y-uAtmoSeaY,0.0);
float tau=uAtmoDensity*atmoLayerMean(y0,y1,max(uAtmoScaleH,1.0))
+uAtmoMarineDensity*atmoLayerMean(y0,y1,max(uAtmoMarineH,0.05));
return 1.0-exp(-d*tau);
}
vec3 atmoApply(vec3 col,vec3 p){
vec3 dir=normalize(p-cameraPosition);
vec3 hazeCol=atmoSkyColor(normalize(vec3(dir.x,dir.y*0.2,dir.z)));
return mix(col,hazeCol,atmoHazeAmount(cameraPosition,p));
}
`;

/** Replacement for `#include <fog_fragment>`; `worldPosExpr` is a vec3 world-space position. */
export function atmosphereFragmentApply(worldPosExpr: string): string {
  return `gl_FragColor.rgb = atmoApply( gl_FragColor.rgb, ${worldPosExpr} );`;
}
