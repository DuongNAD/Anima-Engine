import * as THREE from 'three';
import type { World } from '../utils/worldGen';
import { WaterKind, type WorldV2LoadedBundle } from './types';

// prototype, to be replaced — WV2-LOOK r10 canonical biome grass tables and 512^2 grass mask.
//
// Biomes follow `BIOME_SRGB_PALETTE` in `types.ts`:
// 0 Ocean, 1 Beach, 2 Desert, 3 Savanna, 4 Grassland, 5 Shrubland, 6 Forest, 7 Jungle, 8 Taiga,
// 9 Tundra, 10 Swamp, 11 Rock, 12 Snow, 13 River, 14 Lake, 15 Mangrove, 16 Chaparral, 17 Steppe,
// 18 Alpine, 19 Badlands, 20 Glacier, 21 Bog.

/** Grass/low-herb density per canonical biome (strictly 0 on sand, rock, snow, mud, water). */
export const BIOME_GRASS_COVER: ReadonlyArray<number> = [
  0, // 0 Ocean
  0, // 1 Beach (sand)
  0, // 2 Desert (sand)
  0.95, // 3 Savanna
  1.0, // 4 Grassland
  0.78, // 5 Shrubland
  0.55, // 6 Forest
  0.45, // 7 Jungle
  0.45, // 8 Taiga
  0.6, // 9 Tundra
  0.85, // 10 Swamp
  0, // 11 Rock
  0, // 12 Snow
  0, // 13 River
  0, // 14 Lake
  0, // 15 Mangrove (tidal mudflat)
  0.68, // 16 Chaparral
  0.92, // 17 Steppe
  0.62, // 18 Alpine
  0, // 19 Badlands (rock/mud)
  0, // 20 Glacier
  0.62, // 21 Bog
];

/** Grass height multiplier per canonical biome (r9 tables, 1.0 = meadow). */
export const BIOME_GRASS_HEIGHT: ReadonlyArray<number> = [
  0, 0, 0, 1.7, 1.0, 0.8, 0.6, 0.7, 0.5, 0.35, 1.5, 0, 0, 0, 0, 0, 0.8, 0.9, 0.4, 0, 0, 0.8,
];

/** Grass dryness per canonical biome (0 lush green .. 1 dry straw). */
export const BIOME_GRASS_DRY: ReadonlyArray<number> = [
  0, 0.95, 0.95, 0.75, 0.25, 0.55, 0.15, 0.05, 0.3, 0.5, 0.1, 0.5, 0, 0, 0, 0.2, 0.65, 0.7, 0.45, 0.85, 0, 0.2,
];

/** Wildflower / flowering-herb suitability per canonical biome (0..1). */
export const BIOME_HERB_SUITABILITY: ReadonlyArray<number> = [
  0, 0, 0, 0.55, 1.0, 0.75, 0.35, 0.25, 0.2, 0.45, 0.4, 0, 0, 0, 0, 0, 0.7, 0.85, 0.9, 0, 0, 0.45,
];

const BARE_BIOMES = new Set([0, 1, 2, 11, 12, 13, 14, 15, 19, 20]);

/**
 * Shared GLSL patch field so the terrain turf shader and 3D grass blades agree on dry vs lush
 * patches (`lkGrassPatch(worldXZ)` in [-1, 1]).
 */
export const GRASS_PATCH_GLSL = /* glsl */ `
float lkGrassPatch(vec2 p){
  return sin(p.x*0.83+1.7)*sin(p.y*0.71+0.4)*0.6+sin(p.x*2.9-p.y*2.3+3.1)*0.4;
}
`;

/**
 * prototype, to be replaced — WV2-LOOK r12 shared GLSL for Voronoi world-space grass clumps,
 * metre-space scrolling wind gust waves, and synchronized linear lush/dry grass colours used
 * across both 3D blades and far-field terrain turf.
 */
export const GRASS_FIELD_GLSL = /* glsl */ `
float lkGrassHash12(vec2 p){
  vec3 p3=fract(vec3(p.xyx)*0.1031);
  p3+=dot(p3,p3.yzx+33.33);
  return fract((p3.x+p3.y)*p3.z);
}
vec3 lkGrassHash32(vec2 p){
  vec3 p3=fract(vec3(p.xyx)*vec3(0.1031,0.1030,0.0973));
  p3+=dot(p3,p3.yxz+33.33);
  return fract((p3.xxy+p3.yzz)*p3.zyx);
}
float lkGrassVNoise(vec2 p){
  vec2 i=floor(p);
  vec2 f=fract(p);
  vec2 u=f*f*(3.0-2.0*f);
  float a=lkGrassHash12(i);
  float b=lkGrassHash12(i+vec2(1.0,0.0));
  float c=lkGrassHash12(i+vec2(0.0,1.0));
  float d=lkGrassHash12(i+vec2(1.0,1.0));
  return mix(mix(a,b,u.x),mix(c,d,u.x),u.y);
}
vec4 lkGrassClump(vec2 worldM){
  vec2 p=worldM*0.46;
  vec2 base=floor(p-0.5);
  float minD=9.0;
  vec3 bestH=vec3(0.5);
  for(int dy=0;dy<=1;dy+=1){
    for(int dx=0;dx<=1;dx+=1){
      vec2 cell=base+vec2(float(dx),float(dy));
      vec3 h=lkGrassHash32(cell);
      vec2 pt=cell+vec2(0.18+0.64*h.x,0.18+0.64*h.y);
      vec2 df=pt-p;
      float d=dot(df,df);
      if(d<minD){
        minD=d;
        bestH=h;
      }
    }
  }
  return vec4(bestH,sqrt(minD));
}
vec3 lkGrassWind(vec2 worldM,float timeSec){
  vec2 wDir=vec2(0.8805,0.4741);
  vec2 wPerp=vec2(-wDir.y,wDir.x);
  float g1=lkGrassVNoise(worldM*0.052-wDir*(timeSec*3.6));
  float g2=lkGrassVNoise(worldM*0.16-wDir*(timeSec*6.1)+vec2(5.3,2.7));
  float gust=smoothstep(0.16,0.90,g1*0.66+g2*0.34);
  float swerve=(g2-0.5)*0.42;
  vec2 v=wDir*(0.16+0.84*gust)+wPerp*swerve*gust;
  return vec3(v,gust);
}
vec3 lkGrassAlbedo(float dry,float t,float tintShift){
  vec3 lushRoot=vec3(0.015,0.036,0.008);
  vec3 lushMid=vec3(0.056,0.118,0.022);
  vec3 lushTip=vec3(0.128,0.205,0.044);
  vec3 dryRoot=vec3(0.042,0.035,0.011);
  vec3 dryMid=vec3(0.138,0.110,0.035);
  vec3 dryTip=vec3(0.255,0.202,0.078);
  float tMid=smoothstep(0.0,0.52,t);
  float tTip=smoothstep(0.42,1.0,t);
  vec3 cLush=mix(mix(lushRoot,lushMid,tMid),lushTip,tTip);
  vec3 cDry=mix(mix(dryRoot,dryMid,tMid),dryTip,tTip);
  vec3 c=mix(cLush,cDry,clamp(dry,0.0,1.0));
  c*=mix(vec3(0.92,1.03,0.96),vec3(1.08,0.98,0.88),clamp(tintShift,0.0,1.0));
  return c;
}
`;

function smoothstepJS(edge0: number, edge1: number, x: number): number {
  const t = Math.min(1, Math.max(0, (x - edge0) / (edge1 - edge0)));
  return t * t * (3 - 2 * t);
}

/**
 * 512^2 RGBA8 grass suitability & traits texture built from verified World v2 tile layers:
 * - R: cover density (0..1), forced to 0 on sand/rock/snow/mud/water
 * - G: grass height / 2 (0..1)
 * - B: biome base dryness (0..1)
 * - A: wildflower / herb suitability (0..1)
 */
export function buildWorldV2GrassMask(bundle: WorldV2LoadedBundle, world: World): THREE.DataTexture {
  const { size, normalRg, biomeIdsRgba, biomeWeightsRgba, shoreSdfRg, heightM, waterKind } = bundle;
  const { slope, water, riverAmt } = world;
  const M = 512;
  const mask = new Uint8Array(M * M * 4);

  for (let y = 0; y < M; y += 1) {
    const sy = Math.min(size - 1, Math.round((y / (M - 1)) * (size - 1)));
    for (let x = 0; x < M; x += 1) {
      const sx = Math.min(size - 1, Math.round((x / (M - 1)) * (size - 1)));
      const ci = sy * size + sx;
      const o = (y * M + x) * 4;

      const nx = (normalRg[ci * 2] / 255) * 2 - 1;
      const nz = (normalRg[ci * 2 + 1] / 255) * 2 - 1;
      const ny = Math.sqrt(Math.max(0, 1 - nx * nx - nz * nz));
      const bakedSlope = 1 - ny;
      const localSlope = Math.max(bakedSlope, slope?.[ci] ?? 0);

      const elev = heightM[ci];
      const shoreM = (shoreSdfRg[ci * 2] / 255) * 1000 - 500;
      const isWater =
        waterKind[ci] !== WaterKind.None ||
        (water?.[ci] ?? 0) > 0 ||
        (riverAmt?.[ci] ?? 0) > 24;

      let wSum = 0;
      let cov = 0;
      let hgt = 0;
      let dry = 0;
      let herb = 0;
      let bareW = 0;
      for (let k = 0; k < 4; k += 1) {
        const w = biomeWeightsRgba[ci * 4 + k];
        if (w === 0) continue;
        const b = biomeIdsRgba[ci * 4 + k];
        cov += (BIOME_GRASS_COVER[b] ?? 0) * w;
        hgt += (BIOME_GRASS_HEIGHT[b] ?? 0) * w;
        dry += (BIOME_GRASS_DRY[b] ?? 0.4) * w;
        herb += (BIOME_HERB_SUITABILITY[b] ?? 0) * w;
        if (BARE_BIOMES.has(b)) bareW += w;
        wSum += w;
      }
      if (wSum === 0) {
        const b = biomeIdsRgba[ci * 4];
        cov = BIOME_GRASS_COVER[b] ?? 0;
        hgt = BIOME_GRASS_HEIGHT[b] ?? 0;
        dry = BIOME_GRASS_DRY[b] ?? 0.4;
        herb = BIOME_HERB_SUITABILITY[b] ?? 0;
        bareW = BARE_BIOMES.has(b) ? 1 : 0;
      } else {
        cov /= wSum;
        hgt /= wSum;
        dry /= wSum;
        herb /= wSum;
        bareW /= wSum;
      }

      const slopeKeep = 1 - smoothstepJS(0.16, 0.25, localSlope);
      const elevKeep = smoothstepJS(3.5, 10.0, elev) * smoothstepJS(18.0, 42.0, shoreM) * (1 - smoothstepJS(3250, 3550, elev));
      const bareKeep = 1 - smoothstepJS(0.18, 0.45, bareW);

      const finalCov = isWater ? 0 : cov * slopeKeep * elevKeep * bareKeep;
      const finalHerb = finalCov > 0.05 ? herb * finalCov : 0;

      mask[o] = Math.round(Math.min(1, Math.max(0, finalCov)) * 255);
      mask[o + 1] = Math.round(Math.min(2, Math.max(0, hgt)) * 127.5);
      mask[o + 2] = Math.round(Math.min(1, Math.max(0, dry)) * 255);
      mask[o + 3] = Math.round(Math.min(1, Math.max(0, finalHerb)) * 255);
    }
  }

  const tex = new THREE.DataTexture(mask, M, M, THREE.RGBAFormat, THREE.UnsignedByteType);
  tex.minFilter = THREE.LinearFilter;
  tex.magFilter = THREE.LinearFilter;
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.needsUpdate = true;
  return tex;
}
