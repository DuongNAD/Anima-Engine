// The baked terrain textures, shared by both terrain renderers.
//
// They used to be exported from `WorldTerrain.tsx` and imported from there by `WorldTerrainLod.tsx`.
// That made the component module a mixed export surface — four plain functions plus a component —
// which Fast Refresh cannot hot-swap: editing the mesh's JSX reloaded the page and re-baked every
// texture, several hundred milliseconds of CPU for a one-line change. They are not React, they take
// a `World` and return a `THREE.DataTexture`, and both renderers call them identically, so they
// belong beside the rest of the world-sampling utilities rather than inside a component file.

import * as THREE from 'three';
import type { World } from './worldGen';
import { Biome, BIOME_COUNT, BIOME_RGB } from './worldGen';
import { sampleElevation, sampleMeshHeight } from './worldSample';
import { FLORA_SECTORS_PER_SIDE, type WorldV2LoadedBundle } from '../worldV2/types';

/**
 * Enhanced display palette for the 22 biomes (`AMEND W3-lite`) — tuned so every adjacent
 * biome (Desert/Savanna/Steppe/Chaparral/Badlands, Jungle/Forest/Grassland/Shrubland,
 * Swamp/Bog/Mangrove, Tundra/Taiga/Alpine, Glacier/Snow) reads unmistakably at overview
 * distance while remaining naturalistic under ACES + PBR micro-contrast.
 */
export const BIOME_DISPLAY_RGB: ReadonlyArray<readonly [number, number, number]> = [
  [22, 58, 118], // 0 Ocean
  [242, 228, 184], // 1 Beach — warm bright sand
  [248, 208, 114], // 2 Desert — sun-bleached golden dune sand
  [188, 174, 58], // 3 Savanna — warm dry olive-ochre grass
  [116, 192, 68], // 4 Grassland — vivid meadow green (separates L* from Steppe)
  [152, 162, 82], // 5 Shrubland — sage olive scrub
  [56, 138, 56], // 6 Forest — vibrant mid-forest green
  [10, 72, 32], // 7 Jungle — deep dark saturated rainforest emerald
  [30, 88, 82], // 8 Taiga — cool dark blue-spruce teal-green
  [174, 180, 158], // 9 Tundra — pale frost-lichen grey-green
  [74, 110, 66], // 10 Swamp — mid-dark mossy wetland olive
  [122, 118, 114], // 11 Rock — slate highland grey
  [246, 250, 255], // 12 Snow — crisp alpine snow white
  [48, 132, 192], // 13 River — river cobalt blue
  [36, 116, 178], // 14 Lake — deep lake azure
  [26, 98, 82], // 15 Mangrove — deep coastal tidal teal (Forest-Mangrove dL*=14.0 >= trunk 6.4, Swamp-Mangrove dL*=5.7)
  [146, 136, 88], // 16 Chaparral — dry Mediterranean olive-khaki scrub
  [192, 198, 136], // 17 Steppe — pale straw-sage steppe
  [82, 150, 118], // 18 Alpine — cool high-altitude emerald-sage
  [156, 104, 70], // 19 Badlands — deep earthy sandstone ochre
  [186, 222, 244], // 20 Glacier — crisp light glacial ice-blue (dE76=17.7 vs Snow)
  [136, 94, 62], // 21 Bog — warm rusty peat-brown
];

const BIOME_ROUGHNESS: ReadonlyArray<number> = [
  84, // 0 Ocean
  246, // 1 Beach
  250, // 2 Desert
  244, // 3 Savanna
  234, // 4 Grassland
  238, // 5 Shrubland
  224, // 6 Forest
  184, // 7 Jungle (humid broadleaves / wet humus)
  214, // 8 Taiga
  218, // 9 Tundra
  128, // 10 Swamp (wet mud sheen)
  228, // 11 Rock
  192, // 12 Snow (soft crystalline sheen)
  84, // 13 River
  84, // 14 Lake
  122, // 15 Mangrove (tidal flats)
  242, // 16 Chaparral
  246, // 17 Steppe
  208, // 18 Alpine
  248, // 19 Badlands (dry sandstone)
  68, // 20 Glacier (icy specular glint)
  118, // 21 Bog (saturated peat)
];

function biomeDisplayColor(bio: number): readonly [number, number, number] {
  return BIOME_DISPLAY_RGB[bio] ?? BIOME_RGB[bio] ?? [255, 0, 255];
}

/** Deterministic per-cell hash in [0, 1) — cheap brightness jitter, stable across runs. */
function hash01(x: number, y: number): number {
  let h = Math.imul(x, 374761393) + Math.imul(y, 668265263);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

/** Smooth 2D value noise in [0, 1) for organic ecotone domain warping and geological mottling. */
function smoothNoise2D(fx: number, fy: number): number {
  const x0 = Math.floor(fx);
  const y0 = Math.floor(fy);
  const tx = fx - x0;
  const ty = fy - y0;
  const ux = tx * tx * (3 - 2 * tx);
  const uy = ty * ty * (3 - 2 * ty);
  const a = hash01(x0, y0);
  const b = hash01(x0 + 1, y0);
  const c = hash01(x0, y0 + 1);
  const d = hash01(x0 + 1, y0 + 1);
  return (a * (1 - ux) + b * ux) * (1 - uy) + (c * (1 - ux) + d * ux) * uy;
}

/** Height-to-world-width aspect ratio used by large-scale horizon ambient occlusion (`C1`). */
const HORIZON_HEIGHT_SCALE = 0.095;
const HORIZON_AZIMUTHS: ReadonlyArray<readonly [number, number, number]> = [
  [1, 0, 1],
  [-1, 0, 1],
  [0, 1, 1],
  [0, -1, 1],
  [1, 1, Math.SQRT2],
  [-1, 1, Math.SQRT2],
  [1, -1, Math.SQRT2],
  [-1, -1, Math.SQRT2],
];
const HORIZON_STEPS = [1, 2, 4, 8, 16, 32] as const;

export interface HorizonAoMap {
  readonly size: number;
  readonly data: Float32Array;
}

/**
 * Large-scale horizon ambient occlusion (`C1`): downsamples `world.elevation` to `<= 512^2` and
 * marches 8 azimuths with geometrically growing step radii (up to 32 coarse cells = ~128 cells at
 * `2048^2`) to measure how much sky is blocked by surrounding ridges and massif flanks.
 * Values lie in `[0.64, 1.0]` (flat plains return exact `1.0`; submerged cells halve occlusion).
 */
export function computeHorizonAoMap(world: World): HorizonAoMap {
  const { size, elevation } = world;
  const aoRes = Math.max(1, Math.min(512, size));
  const coarse = new Float32Array(aoRes * aoRes);

  if (aoRes === size) {
    coarse.set(elevation);
  } else if (size % aoRes === 0) {
    const block = size / aoRes;
    const invArea = 1 / (block * block);
    for (let cy = 0; cy < aoRes; cy++) {
      const yBase = cy * block;
      for (let cx = 0; cx < aoRes; cx++) {
        const xBase = cx * block;
        let sum = 0;
        for (let dy = 0; dy < block; dy++) {
          const rowOff = (yBase + dy) * size + xBase;
          for (let dx = 0; dx < block; dx++) {
            sum += elevation[rowOff + dx];
          }
        }
        coarse[cy * aoRes + cx] = sum * invArea;
      }
    }
  } else {
    for (let cy = 0; cy < aoRes; cy++) {
      const y0 = Math.floor((cy * size) / aoRes);
      const y1 = Math.max(y0 + 1, Math.floor(((cy + 1) * size) / aoRes));
      for (let cx = 0; cx < aoRes; cx++) {
        const x0 = Math.floor((cx * size) / aoRes);
        const x1 = Math.max(x0 + 1, Math.floor(((cx + 1) * size) / aoRes));
        let sum = 0;
        let count = 0;
        for (let sy = y0; sy < y1; sy++) {
          const rowOff = sy * size;
          for (let sx = x0; sx < x1; sx++) {
            sum += elevation[rowOff + sx];
            count++;
          }
        }
        coarse[cy * aoRes + cx] = count > 0 ? sum / count : 0;
      }
    }
  }

  const data = new Float32Array(aoRes * aoRes);
  const seaLevel = world.seaLevel ?? 0.38;
  const activeSteps = HORIZON_STEPS.filter((r) => r < aoRes);
  const maxCoord = aoRes - 1;
  const scaleInvRes = HORIZON_HEIGHT_SCALE * aoRes;

  for (let y = 0; y < aoRes; y++) {
    const rowOff = y * aoRes;
    for (let x = 0; x < aoRes; x++) {
      const h0 = coarse[rowOff + x];
      let occSum = 0;
      for (let a = 0; a < 8; a++) {
        const az = HORIZON_AZIMUTHS[a];
        const adx = az[0];
        const ady = az[1];
        const invDiag = 1 / az[2];
        let maxSlope = 0;
        for (let s = 0; s < activeSteps.length; s++) {
          const r = activeSteps[s];
          let sx = x + adx * r;
          if (sx < 0) sx = 0;
          else if (sx > maxCoord) sx = maxCoord;
          let sy = y + ady * r;
          if (sy < 0) sy = 0;
          else if (sy > maxCoord) sy = maxCoord;
          const dh = coarse[sy * aoRes + sx] - h0;
          if (dh > 0) {
            const slope = (dh * scaleInvRes * invDiag) / r;
            if (slope > maxSlope) maxSlope = slope;
          }
        }
        if (maxSlope > 0) {
          occSum += maxSlope / Math.sqrt(1 + maxSlope * maxSlope);
        }
      }
      const meanOcc = occSum * 0.125;
      let occ = Math.min(0.20, Math.max(0, meanOcc - 0.04) * 0.68);
      if (h0 < seaLevel) occ *= 0.5;
      data[rowOff + x] = 1 - occ;
    }
  }

  return { size: aoRes, data };
}

/**
 * Bakes the world's biome colours into a FULL data-resolution sRGB texture. The mesh only has
 * ~384^2 vertices, but the texture keeps all 2048^2 cells, so biome borders, rivers and beaches
 * stay crisp no matter how close the camera gets — and the GPU's bilinear filter blends the
 * border texels into organic transitions for free. A subtle per-cell brightness jitter breaks
 * up the dead-flat look of wide single-biome fields. Palette = `BIOME_DISPLAY_RGB` (derived from
 * `BIOME_RGB`), shared with the minimap so 3D terrain and minimap colours match.
 */
export function buildColorTexture(world: World, opts?: { disableAo?: boolean }): THREE.DataTexture {
  const { size, biome, riverAmt, shore, elevation, slope, water, temperature } = world;
  const data = new Uint8Array(size * size * 4);
  const disableAo = opts?.disableAo ?? false;
  const horizonAo = disableAo ? null : computeHorizonAoMap(world);
  const aoRes = horizonAo?.size ?? 1;
  const aoData = horizonAo?.data ?? null;
  const aoMax = aoRes - 1;
  const aoScale = aoRes / size;
  const RIVER_RGB = biomeDisplayColor(Biome.River);
  // Curvature AO ring radii (world-space constant regardless of data resolution).
  const R1 = Math.max(2, Math.round(size / 680));
  const R2 = Math.max(6, Math.round(size / 228));
  const warpAmp = Math.max(3, Math.round(size / 96));
  const warpScale = 1 / Math.max(12, size / 48);
  const ecoStep = Math.max(4, Math.round(size / 160));
  const mottleScale = 1 / Math.max(10, size / 85);
  for (let y = 0; y < size; y++) {
    const y1a = Math.max(0, y - R1) * size;
    const y1b = Math.min(size - 1, y + R1) * size;
    const y2a = Math.max(0, y - R2) * size;
    const y2b = Math.min(size - 1, y + R2) * size;
    const gy = Math.max(0, Math.min(aoMax, (y + 0.5) * aoScale - 0.5));
    const gy0 = Math.floor(gy);
    const gy1 = gy0 < aoMax ? gy0 + 1 : aoMax;
    const gty = gy - gy0;
    const aoRow0 = gy0 * aoRes;
    const aoRow1 = gy1 * aoRes;
    for (let x = 0; x < size; x++) {
      const i = y * size + x;
      const bio = biome[i];
      let [r, g, b] = biomeDisplayColor(bio);

      // Ecotone blending: gently feather borders between terrestrial biomes while preserving
      // distinct water/ice edges; apply multi-scale domain-warped transitions on mountain
      // foothold/cap zones (Badlands/Chaparral/Snow/Rock/Alpine) so elevation contours never
      // read as hard painted stripes.
      if (
        x >= 2 &&
        y >= 2 &&
        x < size - 2 &&
        y < size - 2 &&
        bio !== Biome.Ocean &&
        bio !== Biome.River &&
        bio !== Biome.Lake &&
        bio !== Biome.Glacier &&
        bio !== Biome.Beach
      ) {
        const isSoftBand =
          bio === Biome.Badlands ||
          bio === Biome.Chaparral ||
          bio === Biome.Snow ||
          bio === Biome.Rock ||
          bio === Biome.Alpine;
        let sx = x;
        let sy = y;
        let step = 2;
        if (isSoftBand) {
          sx = Math.min(
            size - 3,
            Math.max(2, Math.round(x + (smoothNoise2D(x * warpScale, y * warpScale) - 0.5) * 2 * warpAmp)),
          );
          sy = Math.min(
            size - 3,
            Math.max(2, Math.round(y + (smoothNoise2D(x * warpScale + 19.3, y * warpScale + 7.1) - 0.5) * 2 * warpAmp)),
          );
          step = ecoStep;
        }
        const sx0 = Math.max(0, sx - step);
        const sx1 = Math.min(size - 1, sx + step);
        const sy0 = Math.max(0, sy - step);
        const sy1 = Math.min(size - 1, sy + step);
        const nIdx = [sy * size + sx0, sy * size + sx1, sy0 * size + sx, sy1 * size + sx];
        let ar = 0;
        let ag = 0;
        let ab = 0;
        let differs = false;
        for (let k = 0; k < 4; k++) {
          let nb = biome[nIdx[k]];
          if (
            nb === Biome.Ocean ||
            nb === Biome.River ||
            nb === Biome.Lake ||
            nb === Biome.Glacier ||
            nb === Biome.Beach
          ) {
            nb = bio;
          }
          if (nb !== bio) differs = true;
          const c = biomeDisplayColor(nb);
          ar += c[0];
          ag += c[1];
          ab += c[2];
        }
        if (differs || sx !== x || sy !== y) {
          const selfW =
            bio === Biome.Badlands || bio === Biome.Chaparral ? 0.42 : isSoftBand ? 0.55 : 0.72;
          const nWeight = (1 - selfW) * 0.25;
          r = r * selfW + ar * nWeight;
          g = g * selfW + ag * nWeight;
          b = b * selfW + ab * nWeight;
        }
      }

      // Frozen basins: the lakebed under (and peeking around) an ice sheet is pale ice,
      // not liquid-lake navy — matches the opaque ice mesh WorldWater lays on top.
      if ((water?.[i] ?? 0) > 0 && (temperature?.[i] ?? 0.5) < 0.19) {
        r = 213;
        g = 231;
        b = 241;
      }

      // Sunlit sandy shelf: the shallow seabed grades from bright sand at the waterline down
      // into ocean blue. Through the transparent shallows this is what makes tropical water
      // glow turquoise — and it's the stage the coral reefs sit on.
      if (bio === Biome.Ocean) {
        const depth = (world.seaLevel ?? 0.38) - elevation[i];
        if (depth < 0.06) {
          const sandT = (1 - depth / 0.06) * 0.85;
          const warm = temperature?.[i] ?? 0.5;
          const sr = 202 + warm * 28;
          const sg = 194 + warm * 16;
          const sb = 152 + warm * 6;
          r = r * (1 - sandT) + sr * sandT;
          g = g * (1 - sandT) + sg * sandT;
          b = b * (1 - sandT) + sb * sandT;
        }
      }

      // Rivers & streams: the generator's ribbon mask (widens downstream, soft-feathered
      // banks). The gradient tint gives smooth tapering threads instead of jagged 1-cell
      // zigzags, and the centre darkens a touch so big rivers read deeper.
      const amt = riverAmt?.[i] ?? 0;
      if (amt > 0 && bio !== Biome.Ocean && bio !== Biome.Lake && bio !== Biome.Glacier && bio !== Biome.Snow) {
        const t = Math.min(1, (amt / 255) * 1.08);
        r = r * (1 - t) + RIVER_RGB[0] * t;
        g = g * (1 - t) + RIVER_RGB[1] * t;
        b = b * (1 - t) + RIVER_RGB[2] * t;
        const deep = 1 - 0.1 * t;
        r *= deep;
        g *= deep;
        b *= deep;
      }

      // Wet sand: darken the beach right at the waterline (the swash zone).
      if (bio === Biome.Beach && (shore?.[i] ?? 0) > 0.72) {
        const t = 1 - 0.14 * ((shore[i] - 0.72) / 0.28);
        r *= t;
        g *= t;
        b *= t;
      }

      // Organic alpine snow: steep crags (`slope > 0.34`) shed snow onto underlying slate/rock,
      // and spatial noise feathers the snowline instead of leaving a razor-sharp white plateau edge.
      const sVal = slope?.[i] ?? 0;
      const e = elevation[i];
      if (bio === Biome.Snow || bio === Biome.Glacier) {
        const macroN = hash01(x >> 2, y >> 2);
        const microN = hash01(x, y);
        const rockThreshold = bio === Biome.Glacier ? 0.44 : 0.32;
        const rockShed = Math.min(0.82, Math.max(0, (sVal - rockThreshold + (macroN - 0.5) * 0.18) * 1.9));
        if (rockShed > 0) {
          const rockRgb = biomeDisplayColor(Biome.Rock);
          r = r * (1 - rockShed) + rockRgb[0] * rockShed;
          g = g * (1 - rockShed) + rockRgb[1] * rockShed;
          b = b * (1 - rockShed) + rockRgb[2] * rockShed;
        }
        if (bio === Biome.Glacier) {
          const crevasse = 0.94 + microN * 0.08;
          r *= crevasse * 0.97;
          g *= crevasse * 1.0;
          b = Math.min(255, b * crevasse * 1.03 + 4);
        } else {
          // Subtle cool-sky tint & grain so snow plateaus read with soft relief instead of flat grey-white.
          const snowLift = 0.94 + microN * 0.06;
          r *= snowLift * 0.97;
          g *= snowLift * 0.99;
          b = Math.min(255, b * snowLift * 1.03 + 6);
        }
      }

      // Badlands geological mottling & subtle slope grain without contour stripes.
      if (bio === Biome.Badlands) {
        const macroMottle = (smoothNoise2D(x * mottleScale + 3.1, y * mottleScale + 9.7) - 0.5) * 0.16;
        const fineGrain = sVal > 0.25 ? (hash01(x >> 1, y >> 1) - 0.5) * 0.06 : 0;
        r *= 1 + macroMottle * 0.75 + fineGrain;
        g *= 1 + macroMottle * 0.95 + fineGrain * 0.95;
        b *= 1 + macroMottle * 1.1 + fineGrain * 0.85;
      } else if (bio === Biome.Rock && sVal > 0.25) {
        const grain = (hash01(x >> 1, y >> 1) - 0.5) * 0.07;
        r *= 1 + grain;
        g *= 1 + grain * 0.95;
        b *= 1 + grain * 0.85;
      }

      // Curvature + capped horizon ambient occlusion, baked: ravines/gorges/basins shade gently,
      // crests lift a touch, while Snow and Glacier remain white (<= 6% darkening) and overall land
      // averages <= 5% darkening so streams/caves retain crisp contrast.
      if (!disableAo && aoData) {
        const xa1 = Math.max(0, x - R1);
        const xb1 = Math.min(size - 1, x + R1);
        const xa2 = Math.max(0, x - R2);
        const xb2 = Math.min(size - 1, x + R2);
        const ring1 = (elevation[y * size + xa1] + elevation[y * size + xb1] + elevation[y1a + x] + elevation[y1b + x]) * 0.25;
        const ring2 = (elevation[y * size + xa2] + elevation[y * size + xb2] + elevation[y2a + x] + elevation[y2b + x]) * 0.25;
        let occ = Math.min(0.14, Math.max(0, ring1 - e) * 16 + Math.max(0, ring2 - e) * 7);
        if (e < (world.seaLevel ?? 0.38)) occ *= 0.5; // keep the sunny turquoise shallows
        const lift = Math.min(0.06, Math.max(0, e - ring2) * 7);
        const gx = Math.max(0, Math.min(aoMax, (x + 0.5) * aoScale - 0.5));
        const gx0 = Math.floor(gx);
        const gx1 = gx0 < aoMax ? gx0 + 1 : aoMax;
        const gtx = gx - gx0;
        const h00 = aoData[aoRow0 + gx0];
        const h10 = aoData[aoRow0 + gx1];
        const h01 = aoData[aoRow1 + gx0];
        const h11 = aoData[aoRow1 + gx1];
        const hAo = (h00 + (h10 - h00) * gtx) * (1 - gty) + (h01 + (h11 - h01) * gtx) * gty;
        const hOcc = Math.max(0, 1 - hAo) * 0.6;
        let combinedOcc = Math.min(0.22, occ + hOcc * (1 - occ * 0.5));
        if (bio === Biome.Snow || bio === Biome.Glacier) {
          combinedOcc = Math.min(0.06, combinedOcc * 0.28);
        }
        const shade = 1 - combinedOcc + lift;
        r *= shade;
        g *= shade;
        b *= shade;
      }

      const f = 0.95 + hash01(x, y) * 0.09; // ±~4.5% brightness variation
      const o = i * 4;
      data[o] = Math.min(255, r * f);
      data[o + 1] = Math.min(255, g * f);
      data[o + 2] = Math.min(255, b * f);
      data[o + 3] = 255;
    }
  }
  const tex = new THREE.DataTexture(data, size, size, THREE.RGBAFormat, THREE.UnsignedByteType);
  tex.colorSpace = THREE.SRGBColorSpace;
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = THREE.LinearMipmapLinearFilter;
  tex.generateMipmaps = true; // 2048 is power-of-two; stops distant shimmer
  tex.anisotropy = 8; // terrain is mostly seen at grazing angles
  tex.needsUpdate = true;
  return tex;
}

/**
 * Bakes a tangent-space normal map from the RESIDUAL relief — the full-resolution elevation
 * minus what the coarse render mesh already shows. The vertex normals light the broad
 * mountainsides; this map adds back the sub-mesh detail (erosion grooves, ridgelines, rough
 * badlands) without double-counting the large shapes. Follows the same uv mapping as the
 * colour texture / water heightmap: texel (x, y) = data cell (x, y).
 */
export function buildNormalTexture(
  world: World,
  renderSize: number,
  heightRatio: number,
  meshResolution: number,
): THREE.DataTexture {
  // Half the data resolution is plenty for a DETAIL map (it still carries ~2.7x more relief
  // than the render mesh), quarters the GPU memory and makes the bake ~4x faster.
  const res = Math.max(512, world.size >> 1);
  const heightUnits = renderSize * heightRatio;
  const cellW = renderSize / (res - 1);

  // Residual world-space height (full detail minus the mesh's bilinear surface).
  const resid = new Float32Array(res * res);
  const inv = 1 / (res - 1);
  for (let y = 0; y < res; y++) {
    for (let x = 0; x < res; x++) {
      const u = x * inv;
      const v = y * inv;
      resid[y * res + x] =
        (sampleElevation(world, u, v) - sampleMeshHeight(world, u, v, meshResolution)) * heightUnits;
    }
  }

  const data = new Uint8Array(res * res * 4);
  for (let y = 0; y < res; y++) {
    const y0 = Math.max(0, y - 1) * res;
    const y1 = Math.min(res - 1, y + 1) * res;
    for (let x = 0; x < res; x++) {
      const x0 = Math.max(0, x - 1);
      const x1 = Math.min(res - 1, x + 1);
      const dhdx = (resid[y * res + x1] - resid[y * res + x0]) / ((x1 - x0) * cellW || cellW);
      const dhdz = (resid[y1 + x] - resid[y0 + x]) / (((y1 - y0) / res) * cellW || cellW);
      // Tangent space of a +Y-up heightfield with uv aligned to world XZ.
      const nx = -dhdx;
      const nz = -dhdz;
      const len = Math.sqrt(nx * nx + nz * nz + 1);
      const o = (y * res + x) * 4;
      data[o] = ((nx / len) * 0.5 + 0.5) * 255;
      data[o + 1] = ((nz / len) * 0.5 + 0.5) * 255;
      data[o + 2] = ((1 / len) * 0.5 + 0.5) * 255;
      data[o + 3] = 255;
    }
  }
  const tex = new THREE.DataTexture(data, res, res, THREE.RGBAFormat, THREE.UnsignedByteType);
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = THREE.LinearMipmapLinearFilter;
  tex.generateMipmaps = true;
  tex.anisotropy = 8;
  tex.needsUpdate = true;
  return tex;
}

/**
 * Roughness map (read from the GREEN channel): water is glossy, land is matte. This is what
 * makes rivers and lakes catch the sun and glint as it moves — the single biggest "this is
 * real water" cue — for the cost of one small texture.
 */
export function buildRoughnessTexture(world: World): THREE.DataTexture {
  const { size, biome, water, riverAmt, shore, elevation, seaLevel } = world;
  const res = Math.max(512, size >> 1);
  const scale = size / res;
  const data = new Uint8Array(res * res * 4);
  for (let y = 0; y < res; y++) {
    for (let x = 0; x < res; x++) {
      const i = Math.min(size - 1, Math.round(y * scale)) * size + Math.min(size - 1, Math.round(x * scale));
      const bio = biome[i];
      const baseRough = bio < BIOME_COUNT ? BIOME_ROUGHNESS[bio] : 234;
      let rough = baseRough;
      if (bio === Biome.Lake || bio === Biome.River || (water?.[i] ?? 0) > 0) {
        rough = 84; // open inland water (0.33)
      } else if ((riverAmt?.[i] ?? 0) > 0 && bio !== Biome.Glacier && bio !== Biome.Snow) {
        rough = Math.max(84, baseRough - (((riverAmt[i] / 255) * (baseRough - 84)) | 0)); // feathered banks blend matte->glossy
      } else if (elevation[i] < seaLevel) {
        rough = 128; // submerged shelf under the transparent shallows
      } else if (bio === Biome.Beach && (shore?.[i] ?? 0) > 0.72) {
        rough = 178; // wet swash-zone sand (0.7)
      }
      const o = (y * res + x) * 4;
      data[o] = 255;
      data[o + 1] = rough;
      data[o + 2] = 255;
      data[o + 3] = 255;
    }
  }
  const tex = new THREE.DataTexture(data, res, res, THREE.RGBAFormat, THREE.UnsignedByteType);
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = THREE.LinearFilter;
  tex.needsUpdate = true;
  return tex;
}

/**
 * Tileable two-octave value-noise detail tile (mean ~1.0). Multiplied over the terrain at
 * high repeat in the shader, it gives the ground visible grain when the camera is close —
 * the 2048^2 colour texture alone blurs into soft mush within a few metres. Because the
 * noise is mean-centred, its mipmaps average back to 1.0, so the effect self-fades with
 * distance for free.
 */
export function buildDetailTexture(): THREE.DataTexture {
  const N = 256;
  const G = 32; // base lattice
  const lat = new Float32Array((G + 1) * (G + 1));
  let s = 421;
  const rnd = () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
  for (let i = 0; i < lat.length; i++) lat[i] = rnd();
  const vnoise = (u: number, v: number) => {
    const x = u * G;
    const y = v * G;
    const x0 = Math.floor(x) % G;
    const y0 = Math.floor(y) % G;
    const tx = x - Math.floor(x);
    const ty = y - Math.floor(y);
    const a = lat[y0 * (G + 1) + x0];
    const b = lat[y0 * (G + 1) + x0 + 1];
    const c = lat[(y0 + 1) * (G + 1) + x0];
    const d = lat[(y0 + 1) * (G + 1) + x0 + 1];
    return a * (1 - tx) * (1 - ty) + b * tx * (1 - ty) + c * (1 - tx) * ty + d * tx * ty;
  };
  const data = new Uint8Array(N * N * 4);
  for (let y = 0; y < N; y++) {
    for (let x = 0; x < N; x++) {
      const u = x / N;
      const v = y / N;
      const n = vnoise(u, v) * 0.65 + vnoise((u * 3) % 1, (v * 3) % 1) * 0.35; // 2 octaves
      const byte = 128 + (n - 0.5) * 56; // ±11% around mean
      const o = (y * N + x) * 4;
      data[o] = data[o + 1] = data[o + 2] = byte;
      data[o + 3] = 255;
    }
  }
  const tex = new THREE.DataTexture(data, N, N, THREE.RGBAFormat, THREE.UnsignedByteType);
  tex.wrapS = THREE.RepeatWrapping;
  tex.wrapT = THREE.RepeatWrapping;
  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = THREE.LinearMipmapLinearFilter;
  tex.generateMipmaps = true;
  tex.needsUpdate = true;
  return tex;
}

function buildRingOffsets(minRadius: number, maxRadius: number): ReadonlyArray<readonly [number, number, number]> {
  const out: Array<readonly [number, number, number]> = [];
  for (let dy = -maxRadius; dy <= maxRadius; dy++) {
    for (let dx = -maxRadius; dx <= maxRadius; dx++) {
      const cheb = Math.max(Math.abs(dx), Math.abs(dy));
      if (cheb >= minRadius && cheb <= maxRadius) {
        out.push([dx, dy, 1 / Math.hypot(dx, dy)]);
      }
    }
  }
  return out;
}

const D8_OFFSETS = buildRingOffsets(1, 1);

/**
 * The river-ribbon mask plus strict D8 hydrological downstream flow direction (`C2`), encoded in RGBA:
 *   R = feathered river mask (`world.riverAmt`)
 *   G, B = normalized 2D grid-space downstream vector `(nx, ny)` mapped via `round(128 + n * 127)`
 *   A = `255` when an immediate D8 downstream neighbour exists (lowest elevation among D8
 *       neighbours with strictly higher `world.flow`, falling back to steepest strictly
 *       lower elevation when no neighbour has higher `flow`); `0` for dry cells (neutral
 *       G/B = `128`) and flat lake junctions.
 *
 * This is a visual cue derived strictly from local D8 elevation and `world.flow` order,
 * never simulated hydrodynamics. Both renderers share this helper so
 * `WorldTerrain` and `WorldTerrainLod` remain identical.
 */
export function buildRiverMaskTexture(world: World): THREE.DataTexture {
  const { size, riverAmt, elevation, flow } = world;
  const data = new Uint8Array(size * size * 4);

  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = y * size + x;
      const o = i * 4;
      const amt = riverAmt ? riverAmt[i] : 0;
      data[o] = amt;
      data[o + 1] = 128;
      data[o + 2] = 128;
      data[o + 3] = 0;
      if (amt === 0) continue;

      const e0 = elevation[i];
      const f0 = flow ? flow[i] : 0;
      let hasDir = false;
      let bestDx = 0;
      let bestDy = 0;

      // Pass 1 (primary): among immediate D8 neighbours with strictly higher `world.flow`,
      // pick the lowest-elevation neighbour (breaking exact elevation ties by higher `world.flow`).
      // This avoids jumping into higher-flow parallel channels next door while matching ~97.4% of
      // Pass-2 flow-accumulation receivers.
      if (flow) {
        let bestElev = Infinity;
        let bestTieFlow = -Infinity;
        for (let k = 0; k < D8_OFFSETS.length; k++) {
          const off = D8_OFFSETS[k];
          const nx = x + off[0];
          const ny = y + off[1];
          if (nx < 0 || nx >= size || ny < 0 || ny >= size) continue;
          const nIdx = ny * size + nx;
          const nf = flow[nIdx];
          if (nf <= f0 + 1e-7) continue;
          const ne = elevation[nIdx];
          if (ne < bestElev || (ne === bestElev && nf > bestTieFlow)) {
            bestElev = ne;
            bestTieFlow = nf;
            bestDx = off[0];
            bestDy = off[1];
            hasDir = true;
          }
        }
      }

      // Pass 2 (fallback for synthetic uniform-flow worlds or local flow plateaus): steepest
      // strictly lower immediate D8 neighbour.
      if (!hasDir) {
        let bestSlope = 0;
        for (let k = 0; k < D8_OFFSETS.length; k++) {
          const off = D8_OFFSETS[k];
          const nx = x + off[0];
          const ny = y + off[1];
          if (nx < 0 || nx >= size || ny < 0 || ny >= size) continue;
          const dh = e0 - elevation[ny * size + nx];
          if (dh > 1e-7) {
            const slope = dh * off[2];
            if (slope > bestSlope) {
              bestSlope = slope;
              bestDx = off[0];
              bestDy = off[1];
              hasDir = true;
            }
          }
        }
      }

      if (hasDir) {
        const invLen = 1 / Math.hypot(bestDx, bestDy);
        const fx = bestDx * invLen;
        const fy = bestDy * invLen;
        data[o + 1] = Math.min(255, Math.max(0, Math.round(128 + fx * 127)));
        data[o + 2] = Math.min(255, Math.max(0, Math.round(128 + fy * 127)));
        data[o + 3] = 255;
      }
    }
  }

  const tex = new THREE.DataTexture(data, size, size, THREE.RGBAFormat, THREE.UnsignedByteType);
  tex.minFilter = THREE.LinearFilter;
  tex.magFilter = THREE.LinearFilter;
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.needsUpdate = true;
  return tex;
}

/**
 * Splat weights matching the 6 CC0 terrain PBR layers (`grass`, `forest`, `rock`, `sand`, `snow`, `mud`):
 *   R = forest-floor density (0 = open meadow grass, 255 = deep forest floor / leaf litter)
 *   G = sand weight (Beach + coastal strip + desert/savanna arid soil)
 *   B = mud / swamp / riverbank weight
 *   A = snow / glacier weight (feathered organically in height/slope in the shader too)
 */
export function buildSplatTexture(world: World): THREE.DataTexture {
  const { size, biome, riverAmt, shore, elevation, moisture, water } = world;
  const res = Math.max(256, Math.min(512, size));
  const scale = size / res;
  const data = new Uint8Array(res * res * 4);

  for (let y = 0; y < res; y++) {
    const sy = Math.min(size - 1, Math.round(y * scale));
    for (let x = 0; x < res; x++) {
      const sx = Math.min(size - 1, Math.round(x * scale));
      const i = sy * size + sx;
      const bio = biome[i];
      const moist = moisture?.[i] ?? 0.5;
      const riv = (riverAmt?.[i] ?? 0) / 255;
      const sh = shore?.[i] ?? 0;

      let forest = 0;
      let sand = 0;
      let mud = 0;
      let snow = 0;

      if (bio === Biome.Forest || bio === Biome.Taiga || bio === Biome.Jungle) {
        forest = bio === Biome.Jungle ? 0.94 : bio === Biome.Taiga ? 0.78 : 0.85;
        if (bio === Biome.Jungle) mud = 0.14;
      } else if (bio === Biome.Shrubland || bio === Biome.Chaparral) {
        forest = bio === Biome.Chaparral ? 0.3 : 0.28 + moist * 0.22;
      } else if (bio === Biome.Savanna) {
        forest = 0.18 + moist * 0.14;
      }

      if (bio === Biome.Beach || bio === Biome.Desert || bio === Biome.Ocean) {
        sand = bio === Biome.Desert ? 0.95 : bio === Biome.Beach ? 0.95 : 0.72;
      } else if (bio === Biome.Badlands) {
        sand = 0.68;
        mud = 0.22;
      } else if (bio === Biome.Chaparral) {
        sand = 0.42;
      } else if (bio === Biome.Savanna) {
        sand = 0.45;
      } else if (bio === Biome.Steppe) {
        sand = 0.3;
      } else if (bio === Biome.Rock) {
        sand = 0.18;
        mud = 0.18;
      } else if (sh > 0.35) {
        sand = Math.max(sand, (sh - 0.35) * 1.1);
      }

      if (bio === Biome.Bog || bio === Biome.Swamp || bio === Biome.Mangrove) {
        mud = Math.max(mud, bio === Biome.Bog ? 0.94 : bio === Biome.Swamp ? 0.88 : 0.82);
        forest = Math.max(forest, bio === Biome.Mangrove ? 0.24 : 0.12);
      } else if (bio === Biome.Lake || bio === Biome.River || (water?.[i] ?? 0) > 0) {
        mud = Math.max(mud, 0.9);
      } else if (riv > 0.08 && bio !== Biome.Snow && bio !== Biome.Glacier) {
        mud = Math.min(0.85, Math.max(mud, riv * 0.95));
      }

      if (bio === Biome.Snow || bio === Biome.Glacier) {
        snow = 1.0;
      } else if (bio === Biome.Taiga) {
        snow = 0.16;
      } else if (bio === Biome.Tundra) {
        snow = 0.26;
        mud = Math.max(mud, 0.18);
      } else if (bio === Biome.Alpine) {
        snow = elevation[i] > 0.7 ? Math.min(0.85, (elevation[i] - 0.7) * 4.5) : 0.08;
      }

      const o = (y * res + x) * 4;
      data[o] = Math.min(255, Math.max(0, Math.round(forest * 255)));
      data[o + 1] = Math.min(255, Math.max(0, Math.round(sand * 255)));
      data[o + 2] = Math.min(255, Math.max(0, Math.round(mud * 255)));
      data[o + 3] = Math.min(255, Math.max(0, Math.round(snow * 255)));
    }
  }

  const tex = new THREE.DataTexture(data, res, res, THREE.RGBAFormat, THREE.UnsignedByteType);
  tex.minFilter = THREE.LinearFilter;
  tex.magFilter = THREE.LinearFilter;
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.needsUpdate = true;
  return tex;
}

export interface WorldV2TerrainTextures {
  colorMap: THREE.DataTexture;
  normalMap: THREE.DataTexture;
  roughnessMap: THREE.DataTexture;
  detailMap: THREE.DataTexture;
  riverMaskMap: THREE.DataTexture;
  splatMap: THREE.DataTexture;
  horizonMap: THREE.DataTexture;
  shoreSdfMap: THREE.DataTexture;
  hydroFlowMap: THREE.DataTexture;
  /** WV2-LOOK: R = rock-biome, G = snow/ice-biome, B = tree-canopy cover, A = sand/beach. */
  lookMaskMap: THREE.DataTexture;
  /** WV2-LOOK: tileable 4-channel value noise (independent octaves per channel). */
  lookNoiseMap: THREE.DataTexture;
}

// prototype, to be replaced — WV2-LOOK per-biome surface classes, weighted by `biome_weights`.
// Index = canonical 22-biome id (see `BIOME_SRGB_PALETTE` in `worldV2/types.ts`).
// [rock, snow, vegetation, sand]
const WV2_LOOK_BIOME_CLASS: ReadonlyArray<readonly [number, number, number, number]> = [
  [0.0, 0.0, 0.0, 0.3], // 0 Ocean
  [0.0, 0.0, 0.0, 1.0], // 1 Beach
  [0.1, 0.0, 0.02, 0.85], // 2 Desert
  [0.0, 0.0, 0.35, 0.15], // 3 Savanna
  [0.0, 0.0, 0.45, 0.0], // 4 Grassland
  [0.05, 0.0, 0.55, 0.05], // 5 Shrubland
  [0.0, 0.0, 1.0, 0.0], // 6 Forest
  [0.0, 0.0, 1.0, 0.0], // 7 Jungle
  [0.0, 0.0, 0.9, 0.0], // 8 Taiga
  [0.25, 0.05, 0.2, 0.0], // 9 Tundra
  [0.0, 0.0, 0.7, 0.0], // 10 Swamp
  [1.0, 0.0, 0.05, 0.0], // 11 Rock
  [0.0, 1.0, 0.0, 0.0], // 12 Snow
  [0.0, 0.0, 0.2, 0.2], // 13 River
  [0.0, 0.0, 0.2, 0.2], // 14 Lake
  [0.0, 0.0, 0.9, 0.1], // 15 Mangrove
  [0.05, 0.0, 0.5, 0.05], // 16 Chaparral
  [0.05, 0.0, 0.25, 0.1], // 17 Steppe
  [0.35, 0.0, 0.3, 0.0], // 18 Alpine
  [0.6, 0.0, 0.05, 0.3], // 19 Badlands
  [0.0, 1.0, 0.0, 0.0], // 20 Glacier
  [0.0, 0.0, 0.5, 0.0], // 21 Bog
];

/** Separable box blur (in place via `tmp`), radius `r` texels, clamped at the borders. */
function boxBlur(src: Float32Array, tmp: Float32Array, size: number, r: number): void {
  const inv = 1 / (2 * r + 1);
  for (let y = 0; y < size; y += 1) {
    const row = y * size;
    let acc = 0;
    for (let k = -r; k <= r; k += 1) acc += src[row + Math.min(size - 1, Math.max(0, k))];
    for (let x = 0; x < size; x += 1) {
      tmp[row + x] = acc * inv;
      acc += src[row + Math.min(size - 1, x + r + 1)] - src[row + Math.max(0, x - r)];
    }
  }
  for (let x = 0; x < size; x += 1) {
    let acc = 0;
    for (let k = -r; k <= r; k += 1) acc += tmp[Math.min(size - 1, Math.max(0, k)) * size + x];
    for (let y = 0; y < size; y += 1) {
      src[y * size + x] = acc * inv;
      acc += tmp[Math.min(size - 1, y + r + 1) * size + x] - tmp[Math.max(0, y - r) * size + x];
    }
  }
}

/**
 * WV2-LOOK tree-canopy cover in [0, 1] from the baked flora points themselves, so the terrain
 * shader can paint a closed canopy exactly where the trees are (and not on biome swatches with
 * no trees). Trees count 1, shrub thicket 0.55, open savanna crowns (acacia, palm) 0.45, rocks/cacti 0.
 */
function buildCanopyCover(bundle: WorldV2LoadedBundle): Float32Array {
  const { size } = bundle;
  const dens = new Float32Array(size * size);
  const speciesWeight = [1, 1, 1, 0, 0, 0.45, 0.45, 0.55];
  for (const sec of bundle.floraSectors) {
    for (const inst of sec.instances) {
      const w = speciesWeight[inst.species_id] ?? 0;
      if (w === 0) continue;
      const u = (sec.sectorX + inst.x_local_u16 / 65535.0) / FLORA_SECTORS_PER_SIDE;
      const v = (sec.sectorZ + inst.z_local_u16 / 65535.0) / FLORA_SECTORS_PER_SIDE;
      const x = Math.min(size - 1, Math.max(0, Math.floor(u * size)));
      const y = Math.min(size - 1, Math.max(0, Math.floor(v * size)));
      dens[y * size + x] += w;
    }
  }
  const tmp = new Float32Array(size * size);
  const r = Math.max(2, Math.round(size / 512));
  boxBlur(dens, tmp, size, r);
  boxBlur(dens, tmp, size, r);
  // Normalise by a robust high percentile of the non-empty texels.
  const hist = new Uint32Array(1024);
  let maxD = 0;
  for (let i = 0; i < dens.length; i += 1) if (dens[i] > maxD) maxD = dens[i];
  if (maxD <= 0) return dens;
  let nonZero = 0;
  for (let i = 0; i < dens.length; i += 1) {
    if (dens[i] <= 0) continue;
    hist[Math.min(1023, Math.floor((dens[i] / maxD) * 1023))] += 1;
    nonZero += 1;
  }
  let cum = 0;
  let p90 = maxD;
  for (let b = 0; b < 1024; b += 1) {
    cum += hist[b];
    if (cum >= nonZero * 0.9) {
      p90 = ((b + 1) / 1024) * maxD;
      break;
    }
  }
  const inv = 1 / Math.max(1e-6, p90);
  for (let i = 0; i < dens.length; i += 1) dens[i] = Math.min(1, dens[i] * inv);
  return dens;
}

function buildWorldV2LookMask(bundle: WorldV2LoadedBundle): THREE.DataTexture {
  const { size } = bundle;
  const count = size * size;
  const data = new Uint8Array(count * 4);
  const canopy = buildCanopyCover(bundle);
  for (let i = 0; i < count; i += 1) {
    const o4 = i * 4;
    let r = 0;
    let g = 0;
    let b = 0;
    let a = 0;
    let wSum = 0;
    for (let k = 0; k < 4; k += 1) {
      const w = bundle.biomeWeightsRgba[o4 + k];
      if (w === 0) continue;
      const cls = WV2_LOOK_BIOME_CLASS[bundle.biomeIdsRgba[o4 + k]] ?? WV2_LOOK_BIOME_CLASS[4];
      r += cls[0] * w;
      g += cls[1] * w;
      b += cls[2] * w;
      a += cls[3] * w;
      wSum += w;
    }
    if (wSum === 0) {
      const cls = WV2_LOOK_BIOME_CLASS[bundle.biomeIdsRgba[o4]] ?? WV2_LOOK_BIOME_CLASS[4];
      r = cls[0];
      g = cls[1];
      b = cls[2];
      a = cls[3];
      wSum = 1;
    }
    data[o4] = Math.round((r / wSum) * 255);
    data[o4 + 1] = Math.round((g / wSum) * 255);
    // B: tree-canopy cover from the flora points, damped where the biome is not woody.
    data[o4 + 2] = Math.round(canopy[i] * Math.min(1, 0.35 + (b / wSum) * 0.9) * 255);
    data[o4 + 3] = Math.round((a / wSum) * 255);
  }
  return createRgbaTex(data, size, THREE.NoColorSpace, true);
}

/** Tileable RGBA value noise; each channel is an independent 3-octave fBm. */
export function buildLookNoiseTexture(): THREE.DataTexture {
  const N = 256;
  let s = 977;
  const rnd = () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
  const lattices: Float32Array[] = [];
  const G = [8, 16, 32, 64];
  for (let c = 0; c < 4; c += 1) {
    for (const g of G) {
      const lat = new Float32Array(g * g);
      for (let i = 0; i < lat.length; i += 1) lat[i] = rnd();
      lattices.push(lat);
    }
  }
  const vnoise = (lat: Float32Array, g: number, u: number, v: number) => {
    const x = u * g;
    const y = v * g;
    const xi = Math.floor(x);
    const yi = Math.floor(y);
    const tx = x - xi;
    const ty = y - yi;
    const sx = tx * tx * (3 - 2 * tx);
    const sy = ty * ty * (3 - 2 * ty);
    const x0 = ((xi % g) + g) % g;
    const y0 = ((yi % g) + g) % g;
    const x1 = (x0 + 1) % g;
    const y1 = (y0 + 1) % g;
    const a = lat[y0 * g + x0];
    const b = lat[y0 * g + x1];
    const c = lat[y1 * g + x0];
    const d = lat[y1 * g + x1];
    return (a * (1 - sx) + b * sx) * (1 - sy) + (c * (1 - sx) + d * sx) * sy;
  };
  const data = new Uint8Array(N * N * 4);
  for (let y = 0; y < N; y += 1) {
    for (let x = 0; x < N; x += 1) {
      const u = x / N;
      const v = y / N;
      const o = (y * N + x) * 4;
      for (let c = 0; c < 4; c += 1) {
        let n = 0;
        let amp = 0.5;
        let norm = 0;
        for (let k = 0; k < G.length; k += 1) {
          n += vnoise(lattices[c * G.length + k], G[k], u, v) * amp;
          norm += amp;
          amp *= 0.5;
        }
        // Stretch contrast: fBm clusters around 0.5.
        const stretched = 0.5 + (n / norm - 0.5) * 1.9;
        data[o + c] = Math.min(255, Math.max(0, Math.round(stretched * 255)));
      }
    }
  }
  const tex = new THREE.DataTexture(data, N, N, THREE.RGBAFormat, THREE.UnsignedByteType);
  tex.wrapS = THREE.RepeatWrapping;
  tex.wrapT = THREE.RepeatWrapping;
  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = THREE.LinearMipmapLinearFilter;
  tex.generateMipmaps = true;
  tex.anisotropy = 4;
  tex.needsUpdate = true;
  return tex;
}

function createRgbaTex(
  data: Uint8Array,
  size: number,
  colorSpace: THREE.ColorSpace,
  mipmap = true,
): THREE.DataTexture {
  const tex = new THREE.DataTexture(data, size, size, THREE.RGBAFormat, THREE.UnsignedByteType);
  tex.colorSpace = colorSpace;
  tex.wrapS = THREE.ClampToEdgeWrapping;
  tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = mipmap ? THREE.LinearMipmapLinearFilter : THREE.LinearFilter;
  tex.generateMipmaps = mipmap;
  if (mipmap) tex.anisotropy = 8;
  tex.needsUpdate = true;
  return tex;
}

/**
 * Builds GPU terrain textures directly from verified C01 KTX2 tile layers (`WV2-6`, `§3.2`/`§7.1`).
 * Never invokes `computeHorizonAoMap` — `terrain_lighting` and 4-azimuth `terrain_horizon` come
 * straight from the offline bake.
 */
export function buildWorldV2TerrainTextures(
  bundle: WorldV2LoadedBundle,
  world: World,
): WorldV2TerrainTextures {
  const { size, macroAlbedoSize } = bundle;
  const count = size * size;

  const colorData = new Uint8Array(count * 4);
  const invTarget = size > 1 ? 1 / (size - 1) : 1;
  for (let y = 0; y < size; y += 1) {
    const gy = y * invTarget * (macroAlbedoSize - 1);
    const y0 = Math.floor(gy);
    const y1 = Math.min(macroAlbedoSize - 1, y0 + 1);
    const fy = gy - y0;
    for (let x = 0; x < size; x += 1) {
      const gx = x * invTarget * (macroAlbedoSize - 1);
      const x0 = Math.floor(gx);
      const x1 = Math.min(macroAlbedoSize - 1, x0 + 1);
      const fx = gx - x0;

      const idx00 = (y0 * macroAlbedoSize + x0) * 4;
      const idx10 = (y0 * macroAlbedoSize + x1) * 4;
      const idx01 = (y1 * macroAlbedoSize + x0) * 4;
      const idx11 = (y1 * macroAlbedoSize + x1) * 4;

      const i = y * size + x;
      const o4 = i * 4;
      const ao = bundle.lightingRgba[o4] / 255;
      const svf = bundle.lightingRgba[o4 + 1] / 255;
      const bounce = bundle.lightingRgba[o4 + 2] / 255;
      const litScale = 0.56 + 0.44 * (ao * 0.72 + svf * 0.28) + bounce * 0.06;

      // WV2-LOOK: albedo stays un-lit in RGB; the baked lighting factor rides in alpha
      // (litScale / 1.25) so the terrain shader can re-apply it after re-colouring by slope.
      for (let c = 0; c < 3; c += 1) {
        const s00 = bundle.macroAlbedoRgba[idx00 + c] / 255;
        const s10 = bundle.macroAlbedoRgba[idx10 + c] / 255;
        const s01 = bundle.macroAlbedoRgba[idx01 + c] / 255;
        const s11 = bundle.macroAlbedoRgba[idx11 + c] / 255;
        const srgbSample =
          (s00 * (1 - fx) + s10 * fx) * (1 - fy) + (s01 * (1 - fx) + s11 * fx) * fy;
        colorData[o4 + c] = Math.min(255, Math.max(0, Math.round(srgbSample * 255)));
      }
      colorData[o4 + 3] = Math.min(255, Math.max(0, Math.round((litScale / 1.25) * 255)));
    }
  }

  // Tangent-space RGBA8 normal map from decoded BC5 normal_xy: R=nx, G=nz, B=ny_up.
  const normalData = new Uint8Array(count * 4);
  for (let i = 0; i < count; i += 1) {
    const rx = bundle.normalRg[i * 2];
    const rz = bundle.normalRg[i * 2 + 1];
    const nx = (rx / 255.0) * 2.0 - 1.0;
    const nz = (rz / 255.0) * 2.0 - 1.0;
    const ny = Math.sqrt(Math.max(0.0, 1.0 - nx * nx - nz * nz));
    const o4 = i * 4;
    normalData[o4] = rx;
    normalData[o4 + 1] = rz;
    normalData[o4 + 2] = Math.min(255, Math.max(0, Math.round((ny * 0.5 + 0.5) * 255)));
    normalData[o4 + 3] = 255;
  }

  // River mask aligned to `terrainPbrMaterial.ts` Vlachos flow advection channels:
  // R = river strength, G = ux, B = uz, A = flow validity from normalized Q.
  const riverData = new Uint8Array(count * 4);
  for (let i = 0; i < count; i += 1) {
    const o4 = i * 4;
    const ux = bundle.hydroFlowRgba[o4];
    const uz = bundle.hydroFlowRgba[o4 + 1];
    const q = bundle.hydroFlowRgba[o4 + 2];
    const riv = bundle.hydroFlowRgba[o4 + 3];
    riverData[o4] = riv;
    riverData[o4 + 1] = ux;
    riverData[o4 + 2] = uz;
    riverData[o4 + 3] = riv > 8 ? Math.max(180, q) : 0;
  }

  const shoreSdfData = new Uint8Array(count * 4);
  for (let i = 0; i < count; i += 1) {
    const o4 = i * 4;
    shoreSdfData[o4] = bundle.shoreSdfRg[i * 2];
    shoreSdfData[o4 + 1] = bundle.shoreSdfRg[i * 2 + 1];
    shoreSdfData[o4 + 2] = 0;
    shoreSdfData[o4 + 3] = 255;
  }

  return {
    colorMap: createRgbaTex(colorData, size, THREE.SRGBColorSpace, true),
    normalMap: createRgbaTex(normalData, size, THREE.NoColorSpace, true),
    roughnessMap: buildRoughnessTexture(world),
    detailMap: buildDetailTexture(),
    riverMaskMap: createRgbaTex(riverData, size, THREE.NoColorSpace, false),
    splatMap: buildSplatTexture(world),
    horizonMap: createRgbaTex(new Uint8Array(bundle.horizonRgba), size, THREE.NoColorSpace, false),
    shoreSdfMap: createRgbaTex(shoreSdfData, size, THREE.NoColorSpace, false),
    hydroFlowMap: createRgbaTex(new Uint8Array(bundle.hydroFlowRgba), size, THREE.NoColorSpace, false),
    lookMaskMap: buildWorldV2LookMask(bundle),
    lookNoiseMap: buildLookNoiseTexture(),
  };
}
