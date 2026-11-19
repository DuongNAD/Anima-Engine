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

/**
 * Enhanced display palette for the 22 biomes (`AMEND W3-lite`) — tuned so every adjacent
 * biome (Desert/Savanna/Steppe/Chaparral/Badlands, Jungle/Forest/Grassland/Shrubland,
 * Swamp/Bog/Mangrove, Tundra/Taiga/Alpine, Glacier/Snow) reads unmistakably at overview
 * distance while remaining naturalistic under ACES + PBR micro-contrast.
 */
export const BIOME_DISPLAY_RGB: ReadonlyArray<readonly [number, number, number]> = [
  [22, 58, 118], // 0 Ocean
  [242, 228, 184], // 1 Beach — warm bright sand
  [234, 186, 88], // 2 Desert — golden dune sand
  [198, 174, 72], // 3 Savanna — warm dry ochre-gold grass
  [128, 200, 78], // 4 Grassland — fresh lime-meadow green
  [152, 162, 82], // 5 Shrubland — sage olive scrub
  [56, 138, 56], // 6 Forest — vibrant mid-forest green
  [14, 86, 40], // 7 Jungle — deep saturated rainforest emerald
  [34, 96, 88], // 8 Taiga — cool dark blue-spruce teal-green
  [160, 166, 146], // 9 Tundra — pale frost-lichen grey-green
  [58, 90, 56], // 10 Swamp — dark mossy wetland green
  [122, 118, 114], // 11 Rock — slate highland grey
  [246, 250, 255], // 12 Snow — crisp alpine snow white
  [48, 132, 192], // 13 River — river cobalt blue
  [36, 116, 178], // 14 Lake — deep lake azure
  [38, 118, 98], // 15 Mangrove — coastal teal-olive
  [166, 126, 74], // 16 Chaparral — warm tawny-bronze scrub
  [188, 194, 134], // 17 Steppe — pale straw-sage steppe
  [90, 158, 126], // 18 Alpine — cool high-altitude emerald-sage
  [186, 86, 52], // 19 Badlands — iron-rich terracotta red-ochre
  [204, 228, 244], // 20 Glacier — cool pale ice-blue-white
  [112, 76, 50], // 21 Bog — rich rusty peat-brown
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

/**
 * Bakes the world's biome colours into a FULL data-resolution sRGB texture. The mesh only has
 * ~384^2 vertices, but the texture keeps all 2048^2 cells, so biome borders, rivers and beaches
 * stay crisp no matter how close the camera gets — and the GPU's bilinear filter blends the
 * border texels into organic transitions for free. A subtle per-cell brightness jitter breaks
 * up the dead-flat look of wide single-biome fields. Palette = BIOME_RGB, same as the minimap.
 */
export function buildColorTexture(world: World): THREE.DataTexture {
  const { size, biome, riverAmt, shore, elevation, slope, water, temperature } = world;
  const data = new Uint8Array(size * size * 4);
  const RIVER_RGB = biomeDisplayColor(Biome.River);
  // Curvature AO ring radii (world-space constant regardless of data resolution).
  const R1 = Math.max(2, Math.round(size / 680));
  const R2 = Math.max(6, Math.round(size / 228));
  for (let y = 0; y < size; y++) {
    const y1a = Math.max(0, y - R1) * size;
    const y1b = Math.min(size - 1, y + R1) * size;
    const y2a = Math.max(0, y - R2) * size;
    const y2b = Math.min(size - 1, y + R2) * size;
    for (let x = 0; x < size; x++) {
      const i = y * size + x;
      const bio = biome[i];
      let [r, g, b] = biomeDisplayColor(bio);

      // Ecotone blending: gently feather borders between terrestrial biomes while preserving
      // distinct water/ice edges so all 22 biomes stay crisp and readable.
      if (
        x >= 2 &&
        y >= 2 &&
        x < size - 2 &&
        y < size - 2 &&
        bio !== Biome.Ocean &&
        bio !== Biome.River &&
        bio !== Biome.Lake &&
        bio !== Biome.Glacier
      ) {
        const nIdx = [i - 2, i + 2, i - 2 * size, i + 2 * size];
        let ar = 0;
        let ag = 0;
        let ab = 0;
        let differs = false;
        for (let k = 0; k < 4; k++) {
          let nb = biome[nIdx[k]];
          if (nb === Biome.Ocean || nb === Biome.River || nb === Biome.Lake || nb === Biome.Glacier) nb = bio;
          if (nb !== bio) differs = true;
          const c = biomeDisplayColor(nb);
          ar += c[0];
          ag += c[1];
          ab += c[2];
        }
        if (differs) {
          r = r * 0.72 + ar * 0.07;
          g = g * 0.72 + ag * 0.07;
          b = b * 0.72 + ab * 0.07;
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

      // Rock / Badlands mineral tone variation without synthetic `sin(e * 320.0)` contour stripes.
      if ((bio === Biome.Rock || bio === Biome.Badlands) && sVal > 0.25) {
        const grain = (hash01(x >> 1, y >> 1) - 0.5) * 0.07;
        r *= 1 + grain;
        g *= 1 + grain * 0.95;
        b *= 1 + grain * 0.85;
      }

      // Curvature ambient occlusion, baked: ravines/gorges/basins shade, crests lift a touch.
      // Sun-independent (like real sky occlusion), so it layers cleanly under the dynamic
      // lighting and gives the relief its depth even at noon.
      {
        const xa1 = Math.max(0, x - R1);
        const xb1 = Math.min(size - 1, x + R1);
        const xa2 = Math.max(0, x - R2);
        const xb2 = Math.min(size - 1, x + R2);
        const ring1 = (elevation[y * size + xa1] + elevation[y * size + xb1] + elevation[y1a + x] + elevation[y1b + x]) * 0.25;
        const ring2 = (elevation[y * size + xa2] + elevation[y * size + xb2] + elevation[y2a + x] + elevation[y2b + x]) * 0.25;
        // Gentle: strong enough to seat ravines and river beds, not enough to dim whole
        // lowland basins (a broad valley is open to the sky, not occluded).
        let occ = Math.min(0.22, Math.max(0, ring1 - e) * 22 + Math.max(0, ring2 - e) * 10);
        if (e < (world.seaLevel ?? 0.38)) occ *= 0.5; // keep the sunny turquoise shallows
        const lift = Math.min(0.08, Math.max(0, e - ring2) * 8);
        const shade = 1 - occ + lift;
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

/**
 * The river-ribbon mask, feathered, so the shader knows where flowing water lies.
 *
 * Both terrain renderers built this inline and identically. One copy, because the two must agree:
 * `WorldTerrainLod` is only a partitioning of `WorldTerrain` and is verified to be pixel-identical.
 */
export function buildRiverMaskTexture(world: World): THREE.DataTexture {
  const { size, riverAmt } = world;
  const data = new Uint8Array(size * size);
  if (riverAmt) data.set(riverAmt);
  const tex = new THREE.DataTexture(data, size, size, THREE.RedFormat, THREE.UnsignedByteType);
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
