import React, { useMemo, useRef, useState, useEffect } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';
import type { World } from './utils/worldGen';
import { sceneElapsed } from './utils/sceneClock';
import { OCEAN_SHELF_BAND_FRACTION } from './utils/oceanShelf';
import { ELEV_SPAN_M, WORLD_EXTENT_M, type WorldV2LoadedBundle } from './worldV2/types';
import { ATMOSPHERE_FRAGMENT_PARS, atmosphereUniforms } from './materials/atmosphere';

export interface WorldWaterProps {
  world: World;
  worldV2?: WorldV2LoadedBundle | null;
  /** Decorative inland lakes/ponds (`ADR-0010 §4`). Ocean remains visible when false. */
  showDecorativeLakes?: boolean;
  renderSize?: number;
  heightRatio?: number;
  /** World-space direction TO the sun (the scene directional light's position direction). */
  sunDir?: [number, number, number];
}

const vertexShader = /* glsl */ `
  uniform float uTime;
  uniform float uWaveAmp;
  uniform float uTerrainSize;
  varying vec3 vWorldPos;

  void main() {
    // Swell is applied in WORLD space (always along +Y), so the same shader serves both the
    // rotated ocean plane and the merged, pre-transformed lake mesh. It dies out with
    // distance: far vertices are further apart than the wavelength, so displacing them just
    // aliases into moire bands and a jagged horizon silhouette.
    vec4 world = modelMatrix * vec4(position, 1.0);
    float fade = 1.0 - smoothstep(uTerrainSize * 0.5, uTerrainSize * 1.4,
                                  distance(cameraPosition.xz, world.xz));
    float w = sin(world.x * 0.03 + uTime * 0.6) * 0.6
            + cos(world.z * 0.025 + uTime * 0.45) * 0.6;
    world.y += w * uWaveAmp * fade;

    vWorldPos = world.xyz;
    gl_Position = projectionMatrix * viewMatrix * world;
  }
`;

const fragmentShader = /* glsl */ `
  precision highp float;

  uniform float uTime;
  uniform sampler2D uHeightMap;
  uniform float uTerrainSize;   // world-space extent of the terrain (= renderSize)
  // How far past the terrain footprint the sea floor takes to fall from the border height to
  // abyssal. See the note in main(): this is what stops the world's edge being drawn in water.
  uniform float uShelfBand;
  uniform float uSeaY;
  uniform vec3 uSunDir;
  uniform vec3 uSunColor;
  uniform vec3 uShallow;
  uniform vec3 uDeep;
  uniform float uOpacity;
  // Manual fog, synced from scene.fog each frame. ShaderMaterials opt out of three's built-in
  // fog, but the water MUST fade exactly like the fogged terrain or the horizon shows a hard
  // seam of un-fogged, aliasing water.
  uniform vec3 uFogColor;
  uniform float uFogNear;
  uniform float uFogFar;
  // Lake-basin mask (only sampled when uMaskOn=1, i.e. for the merged lake mesh). A basin's
  // bbox quad hangs over any DOWNHILL terrain that sits below the spill level but OUTSIDE
  // the basin — depth alone can't hide it (depth > 0 there), which used to leave sharp
  // floating water shards knifing through every slope. The mask pins water to actual
  // flooded cells.
  uniform sampler2D uLakeMask;
  uniform float uMaskOn;
  uniform sampler2D uShoreSdf;
  uniform sampler2D uHydroFlow;
  uniform float uUseWorldV2Water;
  // WV2-LOOK (World v2 only): metres per scene unit vertically / horizontally, SDF texel size.
  uniform float uDepthMPerUnit;
  uniform float uMPerUnitH;
  uniform float uSdfTexel;

  varying vec3 vWorldPos;
  ${ATMOSPHERE_FRAGMENT_PARS}

  float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123); }
  float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
               mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
  }
  float fbm(vec2 p) {
    float v = 0.0, a = 0.5;
    for (int k = 0; k < 4; k++) { v += a * noise(p); p *= 2.0; a *= 0.5; }
    return v;
  }
  // WV2-LOOK foam lace: sin-free hash and a 3x3 Worley (F1, F2) for cellular foam.
  vec2 lkHash22(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
  }
  // Sin-free value noise: the sin() hash turns into straight-line patterns once its argument
  // reaches ~1e5 (metre-scale coordinates on a 41 km map do).
  float lkNoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    float a = lkHash22(i).x;
    float b = lkHash22(i + vec2(1.0, 0.0)).x;
    float c = lkHash22(i + vec2(0.0, 1.0)).x;
    float d = lkHash22(i + vec2(1.0, 1.0)).x;
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
  }
  vec2 lkWorley(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    float d1 = 8.0;
    float d2 = 8.0;
    for (int y = -1; y <= 1; y++) {
      for (int x = -1; x <= 1; x++) {
        vec2 g = vec2(float(x), float(y));
        vec2 r = g + lkHash22(i + g) - f;
        float d = dot(r, r);
        if (d < d1) { d2 = d1; d1 = d; } else if (d < d2) { d2 = d; }
      }
    }
    return sqrt(vec2(d1, d2));
  }
  // Foam from a density field: a Worley web whose strands widen with density until they
  // flood the cells (dense crest), thin to lace behind it, and vanish at zero. aaW is the
  // pixel footprint in cell units; past one cell per pixel the expected coverage is used.
  float lkFoamLace(vec2 cellP, float dens, float aaW) {
    float d = clamp(dens, 0.0, 1.3);
    // Marbled coverage: density plus a two-octave marble, thresholded with an edge as wide as
    // the pixel footprint, so foam edges are soft at every distance and never Voronoi-straight.
    float marble = lkNoise(cellP) * 0.6 + lkNoise(cellP * 2.3 + 7.1) * 0.4;
    float v = d * 1.15 + (marble - 0.5) * 0.9;
    float e = 0.06 + aaW * 0.8;
    float cover = smoothstep(0.55 - e, 0.55 + e, v);
    // Bubble holes inside moderately dense foam (Worley F1 spots), faded out past ~1 cell/pixel
    // and not computed at all where they could not be resolved or there is no foam.
    float resolved = 1.0 - smoothstep(0.4, 1.3, aaW);
    float holeR = 0.30 * (1.0 - smoothstep(0.6, 1.1, d));
    if (resolved <= 0.0 || cover <= 0.0 || holeR <= 0.0) return cover;
    vec2 w = lkWorley(cellP * 1.7 + 3.3);
    float holes = 1.0 - smoothstep(holeR - aaW - 0.03, holeR + aaW + 0.03, w.x);
    return cover * (1.0 - holes * 0.8 * resolved);
  }

  void main() {
    // Terrain world-Y under this fragment (heightmap stores world height directly).
    //
    // Outside the heightmap's footprint there is no terrain, and what the ocean does there decides
    // whether the world has a visible edge. Two wrong answers, in order:
    //
    //   clamp alone      the border texel smears out to infinity. Where land touches the border
    //                    that zeroes the water depth -> alpha 0 -> a sky-coloured hole fanning
    //                    across the horizon.
    //   times a step   the floor drops to abyssal in ONE TEXEL. Depth jumps from a shelf's few
    //                    units to the full column, so depthN goes 0->1, the colour snaps from
    //                    uShallow to uDeep and alpha from 0.32 to 0.92 — along a perfectly
    //                    straight line, on all four sides. That is the hard cyan cut-plane
    //                    independent review kept rejecting the canonical views for, and it was
    //                    never a framing problem: any elevated camera in orbit or fly mode sees
    //                    the world's square boundary drawn in water.
    //
    // The floor instead *descends* from the border height to abyssal across uShelfBand. The ramp
    // is exactly 1 at the boundary, so the sea floor is continuous with the terrain it continues
    // from, and reaches 0 well before the fog does — a continental shelf falling away into deep
    // water, which is also what the world is. Land at the border keeps its height for a few units
    // and then drops off, so the hole the clamp used to open never forms either.
    vec2 uv = (vWorldPos.xz + uTerrainSize * 0.5) / uTerrainSize;
    float borderDist = max(abs(vWorldPos.x), abs(vWorldPos.z)) - uTerrainSize * 0.5;
    float outside = max(borderDist, 0.0);
    float shelf = 1.0 - smoothstep(0.0, uShelfBand, outside);
    float floorY = texture2D(uHeightMap, clamp(uv, 0.0, 1.0)).r * shelf;
    float depth = max(vWorldPos.y - floorY, 0.0);

    // Smoothly drive the outer ocean into opaque deep water with a gently wavy outer margin
    // (boxN + nEdge from 0.84..0.98) plus a guaranteed hard floor inside -4%..-1% so coastal
    // shoals dissolve organically instead of forming a straight square perimeter or glacier sky cutout.
    float isOcean = 1.0 - step(0.5, uMaskOn);
    float boxN = max(abs(vWorldPos.x), abs(vWorldPos.z)) / (uTerrainSize * 0.5);

    if (uUseWorldV2Water > 0.5) {
      // ---- WV2-LOOK ocean and coast (World v2 only; reference board CST-01..04) -------------
      // A wider, noisier map-edge fall-off so the shelf colour does not stop on the map's line.
      // (Only near the map edge: the legacy edge noise above is no longer paid on every pixel.)
      float nEdge2 = 0.0;
      if (boxN > 0.78) {
        nEdge2 = (noise(vWorldPos.xz * 0.004) - 0.5) * 0.10 + (noise(vWorldPos.xz * 0.017) - 0.5) * 0.05;
      }
      float vEdge = smoothstep(0.86, 1.02, boxN + nEdge2) * isOcean;
      float depthM = depth * uDepthMPerUnit;
      float camD = distance(cameraPosition, vWorldPos);
      float det = 1.0 - smoothstep(uTerrainSize * 0.12, uTerrainSize * 0.48, camD);

      // Distance to the shore in metres (shore_sdf: negative under water, clamped at 500 m) and
      // the bake's wave exposure. Lakes have no ocean surf.
      vec4 sdfS = texture2D(uShoreSdf, clamp(uv, 0.0, 1.0));
      float sdfSigned = sdfS.r * 1000.0 - 500.0;           // + on land
      float offM = max(0.0, -sdfSigned);
      float expo = sdfS.g;
      if (uMaskOn > 0.5) { offM = 500.0; expo = 0.0; }
      // Near the map edge the terrain is sunk under the sea for looks (terrain material) while the
      // SDF still calls it land: no surf there, and the sea stays opaque over it.
      float edgeBand = smoothstep(0.86, 0.93, boxN) * isOcean;
      // Off the data the sea floor falls away: the sunk edge band and everything past the map
      // read as deep open ocean (no shallow rim tracing the map's straight border).
      depthM = mix(depthM, max(depthM, 400.0), edgeBand);

      // Depth colour (CST-04): clear and green over the swash and bar, teal over the shelf,
      // saturated deep blue offshore; the light tint is held to the near-shore band.
      float nearShore = (1.0 - smoothstep(280.0, 500.0, offM)) * (1.0 - edgeBand);
      float dClear = 1.0 - exp(-depthM / 3.0);
      float dTeal = 1.0 - exp(-depthM / 12.0);
      float dDeep = max(1.0 - exp(-depthM / 35.0), (1.0 - nearShore) * isOcean);
      dClear = mix(dClear, 1.0, vEdge);
      dTeal = mix(dTeal, 1.0, vEdge);
      dDeep = mix(dDeep, 1.0, vEdge);
      vec3 wcol = mix(vec3(0.22, 0.50, 0.46), vec3(0.06, 0.32, 0.37), dClear);
      wcol = mix(wcol, vec3(0.02, 0.11, 0.27), dDeep);
      // Abyssal plain beyond the island: a touch darker the further off the map.
      wcol = mix(wcol, vec3(0.012, 0.075, 0.21), smoothstep(1.0, 1.8, boxN) * isOcean);

      // Surf zone (CST-01..03): breaker lines parallel to the shore, each a white crest with a
      // streaky foam trail seaward of it, travelling shoreward; foam thins toward the beach.
      // Driven by offshore distance from the SDF, so the lines follow the coastline's shape.
      float surfOuter = mix(170.0, 420.0, expo);
      float zone = (1.0 - smoothstep(surfOuter * 0.70, surfOuter, offM))
                 * (1.0 - smoothstep(14.0, 32.0, depthM)) * (1.0 - vEdge) * (1.0 - edgeBand) * isOcean;
      const float LAMBDA = 62.0;
      float fwM = fwidth(offM);                              // across-shore metres per pixel
      // Full pixel footprint in metres (looking along a shore, offM barely changes per pixel).
      vec2 lkFw = fwidth(vWorldPos.xz);
      float fpM = max(lkFw.x, lkFw.y) * uMPerUnitH;
      float fwPhase = fwM / LAMBDA;
      float aa = 1.0 - smoothstep(0.08, 0.35, fwPhase);
      float inner = smoothstep(0.0, surfOuter * 0.55, offM);
      float spent = mix(0.45, 1.0, inner);
      float foamAmt = 0.0;
      float faceW = 0.0;
      vec2 waveTilt = vec2(0.0);
      if (zone > 0.001) {
        // Landward direction from the SDF over a +-3 texel (~120 m) baseline: the 8-bit field is
        // flat between neighbouring texels, so a 1-texel difference flickered per pixel.
        float tx = uSdfTexel * 3.0;
        vec2 g = vec2(
          texture2D(uShoreSdf, clamp(uv + vec2(tx, 0.0), 0.0, 1.0)).r - texture2D(uShoreSdf, clamp(uv - vec2(tx, 0.0), 0.0, 1.0)).r,
          texture2D(uShoreSdf, clamp(uv + vec2(0.0, tx), 0.0, 1.0)).r - texture2D(uShoreSdf, clamp(uv - vec2(0.0, tx), 0.0, 1.0)).r);
        vec2 landward = normalize(g + vec2(1e-5));
        // World metres, and a coordinate squashed across the shore (it moves only 20% as fast
        // as the position when you walk seaward) for streaks and cells that trail behind the
        // breakers. The direction only ever multiplies offM (<= 500 m), never absolute position,
        // so a small wobble in it cannot shear the pattern.
        vec2 pM = vWorldPos.xz * uMPerUnitH;
        vec2 pAcross = pM + landward * offM * 0.8;
        float warp = (lkNoise(pM * 0.0045 + 3.1) - 0.5) * 0.9 + (lkNoise(pM * 0.017 + 7.7) - 0.5) * 0.3
                   + (lkNoise(pM * 0.08 + vec2(2.3, uTime * 0.2)) - 0.5) * 0.05;
        float phase = (offM + uTime * 6.0) / LAMBDA + warp;
        float ph = fract(phase);
        float waveId = floor(phase);
        // Breaking sections: each crest breaks along stretches, with density varying along it.
        float section = (0.15 + 0.85 * smoothstep(0.22, 0.62, lkNoise(pAcross * 0.005 + waveId * 3.7)))
                      * mix(0.55, 1.0, lkNoise(pAcross * 0.03 + waveId * 1.9));
        // Edge widths from the pixel footprint (in phase units), so every threshold stays soft.
        float wPh = max(fpM / LAMBDA * 2.5, 0.005);
        float crest = smoothstep(0.0, wPh, ph) * (1.0 - smoothstep(0.06, 0.17 + wPh, ph));
        float trail = exp(-ph * 2.6);
        // Streaks run across the shore, trailing behind (seaward of) the breaker.
        float streak = lkNoise(pAcross * 0.05 + waveId * 5.3) * 0.6
                     + lkNoise(pAcross * 0.13 + vec2(0.0, uTime * 0.3)) * 0.4;
        float trailD = trail * smoothstep(0.30, 0.62, streak + trail * 0.30);
        // Whitewater between breakers in the inner surf: patches that thin shoreward.
        float sheet = smoothstep(0.58, 0.82, streak) * 0.45 * (1.0 - inner * 0.6);
        float dens = (crest * 1.7 + trailD * 1.15 + sheet) * section * spent;
        // Cellular lace: ~5 m cells, elongated across the shore, drifting shoreward.
        vec2 cellP = (pAcross - landward * mod(uTime, 120.0) * 1.5) / 5.0;
        float lace = lkFoamLace(cellP, dens, fpM / 5.0 + 0.02);
        // Thresholding a steep density ramp re-sharpens the crest's leading edge, so the edge is
        // softened again after the threshold, over a few pixels.
        lace *= mix(smoothstep(0.0, wPh * 1.6, ph), 1.0, smoothstep(0.12, 0.25, ph));
        // Far away the band is soft and broken, not dashed.
        float farBand = (0.18 + 0.32 * section) * spent;
        foamAmt = mix(farBand, lace, aa) * zone;
        // Rolling face just shoreward of each crest: darker, thinner (more transparent) water.
        faceW = smoothstep(0.80, 0.985 - wPh, ph) * (1.0 - smoothstep(1.0 - wPh * 1.5, 1.0, ph)) * section * zone * aa;
        // Wave relief through the normal only (the sea mesh is far too coarse to displace):
        // a gentle back sloping seaward from the crest and a steep face toward the shore,
        // continuous through the wrap at the crest.
        float dhdp = -2.6 * exp(-ph * 2.6) * smoothstep(0.0, 0.04 + wPh, ph)
                   + smoothstep(0.80, 1.0, ph) * 6.0 * (1.0 - smoothstep(0.96 - wPh, 1.0, ph));
        float slope = dhdp * 1.4 / LAMBDA * section * zone * aa;   // ~1.4 m breakers
        waveTilt = landward * slope * 4.0;   // normal tilts toward the beach on the face
        // Aerated water between the breakers is milkier and greener.
        wcol = mix(wcol, vec3(0.30, 0.55, 0.52), zone * 0.30);
        wcol = mix(wcol, vec3(0.05, 0.30, 0.33), faceW * 0.45);
      }
      // Swash (CST-01): spent foam running up the last metres of the beach as thin lace.
      float swash = (1.0 - smoothstep(2.0, 16.0 + 8.0 * expo, offM)) * isOcean * (1.0 - edgeBand);
      if (swash > 0.001) {
        // Marbled, not cellular: two octaves of value noise, thresholded with a footprint-wide
        // edge so it never sparkles.
        vec2 sp = vWorldPos.xz * uMPerUnitH / 6.0 + vec2(mod(uTime, 200.0) * 0.12, -mod(uTime, 200.0) * 0.09);
        float m = lkNoise(sp) * 0.65 + lkNoise(sp * 2.7 + 5.1) * 0.35;
        float sw = fpM / 6.0 * 0.6 + 0.02;
        float swashD = swash * (0.45 + 0.4 * lkNoise(sp * 0.13 + 2.0));
        swash = smoothstep(0.62 - swashD * 0.3 - sw, 0.62 - swashD * 0.3 + sw, m) * swashD * 1.4 * mix(0.6, 1.0, det);
      }
      foamAmt = max(foamAmt, swash * (1.0 - vEdge));

      // Ripples, sun glints, sky fresnel.
      vec3 nrm = vec3(0.0, 1.0, 0.0);
      if (det > 0.002) {
        vec2 rw = vWorldPos.xz * 1.1;
        float r1 = noise(rw + vec2(uTime * 0.35, uTime * 0.21));
        float r2 = noise(rw * 2.3 - vec2(uTime * 0.27, -uTime * 0.19));
        float r3 = noise(rw * 0.37 + vec2(-uTime * 0.08, uTime * 0.05));
        float rx = (r1 - 0.5) * 0.22 + (r2 - 0.5) * 0.12 + (r3 - 0.5) * 0.10;
        float rz = (noise(rw.yx + 7.3 + uTime * 0.3) - 0.5) * 0.22 + (r2 - 0.5) * 0.08 + (r3 - 0.5) * 0.08;
        nrm = normalize(vec3(rx * det, 1.0, rz * det));
      }
      nrm = normalize(nrm + vec3(waveTilt.x, 0.0, waveTilt.y));
      vec3 vdir = normalize(cameraPosition - vWorldPos);
      vec3 sdir = normalize(uSunDir);
      float ndh2 = max(dot(nrm, normalize(sdir + vdir)), 0.0);
      float spec2 = pow(ndh2, 220.0) * 1.4 * det + pow(ndh2, 24.0) * 0.06;
      float diff2 = clamp(dot(nrm, sdir) * 0.45 + 0.64, 0.0, 1.0);
      // Cloud shadows on the sea: no glints, a darker body.
      float cloudSh = atmoCloudShadow(vWorldPos);
      spec2 *= cloudSh * cloudSh;
      diff2 *= mix(0.70, 1.0, cloudSh);
      vec3 rn = normalize(vec3(nrm.x * 0.3, 1.0, nrm.z * 0.3));
      float schlick = 0.02 + 0.98 * pow(1.0 - max(dot(rn, vdir), 0.0), 5.0);
      vec3 rdir = reflect(-vdir, rn);
      rdir.y = abs(rdir.y);
      float refl = clamp(schlick * 1.1 + 0.03, 0.0, 0.8);
      // Reflection over transmission: absorb is how much of the bed the water column hides.
      // Very shallow water is mostly the (wet) bed plus sky sheen, not a green film.
      float absorb = mix(0.10, 0.97, dTeal) * (1.0 - 0.35 * faceW);
      vec3 body = wcol * diff2;
      float a = refl + (1.0 - refl) * absorb;
      vec3 c = (refl * atmoSkyColor(normalize(rdir)) + (1.0 - refl) * absorb * body) / max(a, 1e-3);
      c += uSunColor * spec2 * (1.0 - foamAmt);
      // Foam: white, lit by the sun, slightly blue in its own shade; opaque.
      vec3 foamCol = vec3(0.86, 0.90, 0.93) * (0.82 + 0.18 * diff2);
      c = mix(c, foamCol, clamp(foamAmt, 0.0, 1.0));
      a = mix(a, 0.97, clamp(foamAmt * 1.3, 0.0, 1.0));
      a *= smoothstep(0.0, 0.8, depthM);
      // The sea plane is infinite: keep it out of below-sea-level hollows the shore SDF calls land.
      a *= 1.0 - smoothstep(5.0, 40.0, sdfSigned) * isOcean * (1.0 - edgeBand);
      a = mix(a, 1.0, vEdge);
      if (uMaskOn > 0.5) {
        a *= smoothstep(0.2, 0.55, texture2D(uLakeMask, clamp(uv, 0.0, 1.0)).r);
      }

      // Spray (CST-01): a faint bright haze over the breaking surf, then the shared aerial
      // perspective (whose low marine layer softens what lies behind the surf).
      vec3 hdir = normalize(vWorldPos - cameraPosition);
      vec3 hazeC = atmoSkyColor(normalize(vec3(hdir.x, hdir.y * 0.2, hdir.z)));
      float fwdSun = pow(max(dot(hdir, normalize(uSunDir)), 0.0), 6.0);
      c = mix(c, mix(hazeC, vec3(0.95), 0.4), zone * (0.08 + 0.12 * expo) * (1.0 + 1.5 * fwdSun) * (0.6 + 0.4 * clamp(foamAmt * 2.0, 0.0, 1.0)));
      float hz = atmoHazeAmount(cameraPosition, vWorldPos);
      c = mix(c, hazeC, hz);
      a = mix(a, 1.0, hz * hz);
      gl_FragColor = vec4(c, a);
      return;
    }

    float nEdge = (noise(vWorldPos.xz * 0.009) - 0.5) * 0.06;
    float edgeSoft = smoothstep(0.84, 0.98, boxN + nEdge) * isOcean;
    float edgeHard = smoothstep(-uTerrainSize * 0.04, -uTerrainSize * 0.01, borderDist) * isOcean;
    float edgeFade = max(edgeSoft, edgeHard);
    float depthN = clamp(depth / (uSeaY * 0.7 + 0.001), 0.0, 1.0);
    depthN = mix(depthN, 1.0, edgeFade);
    vec3 color = mix(uShallow, uDeep, depthN);

    // Micro-detail dies completely before overview distance (~0.48 * terrainSize) so open ocean
    // never shimmers into white speckle noise.
    float camDist = distance(cameraPosition, vWorldPos);
    float detail = 1.0 - smoothstep(uTerrainSize * 0.12, uTerrainSize * 0.48, camDist);

    // Animated micro-normal for soft sun sheen.
    vec2 sw = vWorldPos.xz * 0.22;
    float n1 = noise(sw + vec2(uTime * 0.20, uTime * 0.13));
    float n2 = noise(sw * 1.9 - vec2(uTime * 0.16, -uTime * 0.11));
    vec3 normal = normalize(vec3((n1 - 0.5) * 0.48 * detail, 1.0, (n2 - 0.5) * 0.48 * detail));

    vec3 viewDir = normalize(cameraPosition - vWorldPos);
    vec3 sunDir = normalize(uSunDir);
    vec3 halfDir = normalize(sunDir + viewDir);
    float ndh = max(dot(normal, halfDir), 0.0);
    float spec = pow(ndh, 28.0) * 0.42 * detail * detail + pow(ndh, 8.0) * 0.12;
    float diff = clamp(dot(normal, sunDir) * 0.45 + 0.64, 0.0, 1.0);
    color = color * diff + uSunColor * spec;

    // Grazing angles reflect atmospheric horizon tint smoothly.
    float fres = pow(1.0 - max(dot(normal, viewDir), 0.0), 4.0);
    color = mix(color, uFogColor, clamp(fres * 0.85, 0.0, 0.62));

    // Transparent in coastal shallows (showing sandy/coral floor), seamlessly opaque in deep ocean
    // so the end of the terrain mesh at ±renderSize/2 never shows through the water.
    float oceanDeepOpaque = mix(uOpacity, 1.0, smoothstep(0.45, 0.92, depthN) * isOcean);
    float alpha = mix(0.36, 1.0, pow(depthN, 0.78)) * oceanDeepOpaque;

    // Shoreline foam strictly inside terrain boundaries & close camera range.
    float surf = sin(uTime * 0.9 + (vWorldPos.x + vWorldPos.z) * 0.045)
               + 0.5 * sin(uTime * 1.7 - (vWorldPos.x - vWorldPos.z) * 0.06);
    float foamReach = 0.95 + surf * 0.35;
    float waveExposure = 1.0;
    float sdfFoamBoost = 0.0;
    if (uUseWorldV2Water > 0.5) {
      vec4 sdfSample = texture2D(uShoreSdf, clamp(uv, 0.0, 1.0));
      float sdfM = sdfSample.r * 1000.0 - 500.0;
      waveExposure = mix(0.45, 1.55, sdfSample.g);
      float shoreRing = 1.0 - smoothstep(0.0, 95.0, abs(sdfM));
      sdfFoamBoost = shoreRing * (0.28 + 0.22 * sin(sdfM * 0.18 - uTime * 2.1)) * sdfSample.g;
      vec4 hydroSample = texture2D(uHydroFlow, clamp(uv, 0.0, 1.0));
      float estuaryMix = smoothstep(0.15, 0.85, hydroSample.a) * (1.0 - edgeFade) * 0.25;
      color = mix(color, mix(uShallow, vec3(0.42, 0.68, 0.72), 0.45), estuaryMix);
    }
    float foamBand = (1.0 - smoothstep(0.0, max(0.35, foamReach * waveExposure), depth)) * detail * detail * (1.0 - edgeFade);
    foamBand = clamp(foamBand + sdfFoamBoost * detail * (1.0 - edgeFade), 0.0, 1.0);
    if (foamBand > 0.001) {
      float f = fbm(vWorldPos.xz * 0.55 + vec2(0.0, uTime * 0.55));
      float foam = smoothstep(0.48, 0.90, f) * foamBand * 0.76;
      color = mix(color, vec3(0.96, 0.99, 1.0), foam);
      alpha = mix(alpha, 0.92, foam);
    }

    // Fade to 0 right at inland/coastal shoreline, then lock to opaque deep water past the border.
    float shoreWater = smoothstep(0.0, uSeaY * 0.03 + 0.001, depth);
    alpha *= shoreWater;
    alpha = mix(alpha, 1.0, edgeFade);

    // Lakes only: clip to flooded basin cells.
    if (uMaskOn > 0.5) {
      alpha *= smoothstep(0.2, 0.55, texture2D(uLakeMask, clamp(uv, 0.0, 1.0)).r);
    }

    // Smooth atmospheric fog + horizon melt so the ocean blends cleanly into the sky dome.
    float fogF = smoothstep(uFogNear * 0.72, uFogFar, camDist);
    color = mix(color, uFogColor, fogF);
    alpha = mix(alpha, 1.0, fogF * fogF);

    gl_FragColor = vec4(color, alpha);
  }
`;

/** The parts of the water uniforms that come from the world and the frame, not the palette. */
interface WaterUniformInputs {
  heightTex: THREE.Texture;
  lakeMaskTex: THREE.Texture;
  shoreSdfTex: THREE.Texture;
  hydroFlowTex: THREE.Texture;
  useWorldV2Water: number;
  renderSize: number;
  seaY: number;
  /** WV2-LOOK: metres per scene unit, vertical and horizontal; shore-SDF texel size in UV. */
  depthMPerUnit: number;
  mPerUnitH: number;
  sdfTexel: number;
  /** Seed only: `useFrame` writes `uSunDir` every frame from the live prop. */
  sunDir: [number, number, number];
}

/**
 * Build one water surface's uniform block.
 *
 * Module scope rather than a closure inside the component, because as a closure it captured five
 * values that its callers' dependency lists did not name — two lint-suppressed `useMemo`s whose
 * suppression was load-bearing, since listing `sunDir` honestly would have rebuilt the ocean's
 * uniforms on every render of a moving sun. Taking the inputs explicitly separates the two kinds:
 * what the block is *built* from (below) and what the frame loop *drives* (uTime, uSunDir, fog).
 */
function makeWaterUniforms(
  inputs: WaterUniformInputs,
  shallow: string,
  deep: string,
  opacity: number,
  waveAmp: number,
  maskOn = 0,
) {
  return {
    uTime: { value: 0 },
    uWaveAmp: { value: waveAmp },
    uHeightMap: { value: inputs.heightTex },
    uTerrainSize: { value: inputs.renderSize },
    uShelfBand: { value: inputs.renderSize * OCEAN_SHELF_BAND_FRACTION },
    uSeaY: { value: inputs.seaY },
    uSunDir: { value: new THREE.Vector3(...inputs.sunDir) },
    uSunColor: { value: new THREE.Color('#fff4d6') },
    uShallow: { value: new THREE.Color(shallow) },
    uDeep: { value: new THREE.Color(deep) },
    uOpacity: { value: opacity },
    uFogColor: { value: new THREE.Color('#cfe4f2') },
    uFogNear: { value: inputs.renderSize * 0.8 },
    uFogFar: { value: inputs.renderSize * 3.2 },
    uLakeMask: { value: inputs.lakeMaskTex },
    uMaskOn: { value: maskOn },
    uShoreSdf: { value: inputs.shoreSdfTex },
    uHydroFlow: { value: inputs.hydroFlowTex },
    uUseWorldV2Water: { value: inputs.useWorldV2Water },
    uDepthMPerUnit: { value: inputs.depthMPerUnit },
    uMPerUnitH: { value: inputs.mPerUnitH },
    uSdfTexel: { value: inputs.sdfTexel },
    ...atmosphereUniforms,
  };
}

export const WorldWater: React.FC<WorldWaterProps> = ({
  world,
  worldV2 = null,
  showDecorativeLakes = true,
  renderSize = 400,
  heightRatio = 0.13,
  sunDir = [1, 1.5, 0.6],
}) => {
  const oceanMat = useRef<THREE.ShaderMaterial>(null);

  const heightUnits = renderSize * heightRatio;
  const seaY = world.seaLevel * heightUnits;

  // Heightmap: Float32 red texture storing the terrain's WORLD height per cell, so the shader
  // reads depth = waterY - terrainY directly.
  const heightTex = useMemo(() => {
    const { size, elevation } = world;
    const data = new Float32Array(size * size);
    for (let i = 0; i < data.length; i++) data[i] = elevation[i] * heightUnits;
    const tex = new THREE.DataTexture(data, size, size, THREE.RedFormat, THREE.FloatType);
    tex.minFilter = THREE.LinearFilter;
    tex.magFilter = THREE.LinearFilter;
    tex.wrapS = THREE.ClampToEdgeWrapping;
    tex.wrapT = THREE.ClampToEdgeWrapping;
    tex.needsUpdate = true;
    return tex;
  }, [world, heightUnits]);

  // Per-cell lake mask (1 where standing water fills the cell), soft-edged by the GPU's
  // bilinear filter so basin shorelines stay smooth.
  const lakeMaskTex = useMemo(() => {
    const { size, water } = world;
    const data = new Uint8Array(size * size);
    for (let i = 0; i < data.length; i++) data[i] = water[i] > 0 ? 255 : 0;
    const tex = new THREE.DataTexture(data, size, size, THREE.RedFormat, THREE.UnsignedByteType);
    tex.minFilter = THREE.LinearFilter;
    tex.magFilter = THREE.LinearFilter;
    tex.wrapS = THREE.ClampToEdgeWrapping;
    tex.wrapT = THREE.ClampToEdgeWrapping;
    tex.needsUpdate = true;
    return tex;
  }, [world]);

  const { shoreSdfTex, hydroFlowTex } = useMemo(() => {
    if (worldV2) {
      const n = worldV2.size * worldV2.size;
      const sdfRgba = new Uint8Array(n * 4);
      for (let i = 0; i < n; i += 1) {
        sdfRgba[i * 4] = worldV2.shoreSdfRg[i * 2];
        sdfRgba[i * 4 + 1] = worldV2.shoreSdfRg[i * 2 + 1];
        sdfRgba[i * 4 + 3] = 255;
      }
      const sTex = new THREE.DataTexture(sdfRgba, worldV2.size, worldV2.size, THREE.RGBAFormat, THREE.UnsignedByteType);
      sTex.minFilter = THREE.LinearFilter;
      sTex.magFilter = THREE.LinearFilter;
      sTex.wrapS = THREE.ClampToEdgeWrapping;
      sTex.wrapT = THREE.ClampToEdgeWrapping;
      sTex.needsUpdate = true;

      const hTex = new THREE.DataTexture(
        new Uint8Array(worldV2.hydroFlowRgba),
        worldV2.size,
        worldV2.size,
        THREE.RGBAFormat,
        THREE.UnsignedByteType,
      );
      hTex.minFilter = THREE.LinearFilter;
      hTex.magFilter = THREE.LinearFilter;
      hTex.wrapS = THREE.ClampToEdgeWrapping;
      hTex.wrapT = THREE.ClampToEdgeWrapping;
      hTex.needsUpdate = true;
      return { shoreSdfTex: sTex, hydroFlowTex: hTex };
    }
    const empty = new Uint8Array([128, 0, 0, 255]);
    const sTex = new THREE.DataTexture(empty, 1, 1, THREE.RGBAFormat, THREE.UnsignedByteType);
    const hTex = new THREE.DataTexture(new Uint8Array([128, 128, 0, 0]), 1, 1, THREE.RGBAFormat, THREE.UnsignedByteType);
    sTex.needsUpdate = true;
    hTex.needsUpdate = true;
    return { shoreSdfTex: sTex, hydroFlowTex: hTex };
  }, [worldV2]);

  // The sun direction the uniform blocks are *seeded* with, captured at mount. `useFrame` writes
  // `uSunDir` from the live prop every frame, so this is an initial condition — pinning it is what
  // lets the two memos below list every value they read instead of suppressing the rule.
  const [uniformInputSunDir] = useState<[number, number, number]>(() => sunDir);
  const uniformInputs = useMemo<WaterUniformInputs>(
    () => ({
      heightTex,
      lakeMaskTex,
      shoreSdfTex,
      hydroFlowTex,
      useWorldV2Water: worldV2 ? 1 : 0,
      renderSize,
      seaY,
      depthMPerUnit: ELEV_SPAN_M / Math.max(1e-6, heightUnits),
      mPerUnitH: (worldV2?.extentM ?? WORLD_EXTENT_M) / renderSize,
      sdfTexel: 1 / Math.max(1, worldV2?.size ?? 1),
      sunDir: uniformInputSunDir,
    }),
    [heightTex, lakeMaskTex, shoreSdfTex, hydroFlowTex, worldV2, renderSize, seaY, heightUnits, uniformInputSunDir],
  );

  const oceanUniforms = useMemo(
    // WV2-LOOK: the swell amplitude was +-1 unit = +-43 m of sea level at World v2 scale, so
    // the shelf flooded and drained between frames. Keep about a metre: the surf bands carry
    // the wave motion now.
    () => makeWaterUniforms(uniformInputs, '#2d99a2', '#103b66', 0.92, uniformInputs.useWorldV2Water ? 0.03 : 0.85),
    [uniformInputs],
  );

  // One shared material for every lake plane (uniforms updated once per frame).
  const lakeMaterial = useMemo(
    () =>
      new THREE.ShaderMaterial({
        vertexShader,
        fragmentShader,
        uniforms: makeWaterUniforms(uniformInputs, '#3b9cc0', '#164870', 0.88, 0.35, 1),
        transparent: true,
        depthWrite: false,
        fog: false,
      }),
    [uniformInputs],
  );

  // Lakes & ponds: one bbox quad per basin at its spill level, merged into (up to) TWO
  // geometries — liquid water and FROZEN water. Cold-climate basins (polar fringe, high
  // mountains via the lapse rate) render as opaque ice sheets instead of animated water.
  // The liquid fragment shader fades alpha to 0 over dry land, so each bbox quad only
  // shows across its actual basin; ice quads are shrunk slightly to hug their basin.
  const { lakesGeom, iceGeom } = useMemo(() => {
    const { size, temperature } = world;
    const lakeBasins = world.lakeBasins ?? []; // tolerate an older/partial world
    if (lakeBasins.length === 0) return { lakesGeom: null, iceGeom: null };
    const margin = (renderSize / (size - 1)) * 2;
    const toWorld = (g: number) => (g / (size - 1) - 0.5) * renderSize;
    const liquid: THREE.BufferGeometry[] = [];
    const ice: THREE.BufferGeometry[] = [];
    for (const b of lakeBasins) {
      const cx = Math.round((b.minX + b.maxX) / 2);
      const cy = Math.round((b.minY + b.maxY) / 2);
      const frozen = (temperature?.[cy * size + cx] ?? 0.5) < 0.19;
      const m = frozen ? margin * 0.25 : margin;
      const x0 = toWorld(b.minX) - m;
      const x1 = toWorld(b.maxX) + m;
      const z0 = toWorld(b.minY) - m;
      const z1 = toWorld(b.maxY) + m;
      const w = Math.max(0.001, x1 - x0);
      const h = Math.max(0.001, z1 - z0);
      const seg = frozen ? 1 : Math.max(1, Math.min(12, Math.round(Math.max(w, h) / 14)));
      const geom = new THREE.PlaneGeometry(w, h, seg, seg)
        .rotateX(-Math.PI / 2)
        .translate((x0 + x1) / 2, b.level * heightUnits + (frozen ? 0.12 : 0), (z0 + z1) / 2);
      (frozen ? ice : liquid).push(geom);
    }
    const mergedLiquid = liquid.length ? mergeGeometries(liquid, false) : null;
    const mergedIce = ice.length ? mergeGeometries(ice, false) : null;
    [...liquid, ...ice].forEach((p) => p.dispose());
    return { lakesGeom: mergedLiquid, iceGeom: mergedIce };
  }, [world, renderSize, heightUnits]);

  useFrame((state) => {
    const t = sceneElapsed(state.clock);
    const fog = state.scene.fog as THREE.Fog | null; // WorldWeather owns scene.fog (linear)
    const mats = [oceanMat.current, lakeMaterial];
    for (const mat of mats) {
      if (!mat) continue;
      mat.uniforms.uTime.value = t;
      mat.uniforms.uSunDir.value.set(sunDir[0], sunDir[1], sunDir[2]);
      if (fog && typeof fog.near === 'number') {
        mat.uniforms.uFogColor.value.copy(fog.color);
        mat.uniforms.uFogNear.value = fog.near;
        mat.uniforms.uFogFar.value = fog.far;
      }
    }
  });

  useEffect(() => {
    return () => {
      heightTex.dispose();
      lakeMaskTex.dispose();
      shoreSdfTex.dispose();
      hydroFlowTex.dispose();
      lakeMaterial.dispose();
      lakesGeom?.dispose();
      iceGeom?.dispose();
    };
  }, [heightTex, lakeMaskTex, shoreSdfTex, hydroFlowTex, lakeMaterial, lakesGeom, iceGeom]);

  return (
    <group name="world-water">
      {/* Ocean plane at sea level. Its half-width must exceed the camera's max orbit offset
          (renderSize*3.2) PLUS the far clip (renderSize*11), or the far-clip circle pokes past
          the plane's rim and the sky dome shines through as a bright wedge on the horizon.
          The shader has long since melted the water into the fog haze by that distance. */}
      <mesh rotation-x={-Math.PI / 2} position={[0, seaY, 0]} name="world-ocean" frustumCulled={false}>
        <planeGeometry args={[renderSize * 30, renderSize * 30, 128, 128]} />
        <shaderMaterial
          ref={oceanMat}
          vertexShader={vertexShader}
          fragmentShader={fragmentShader}
          uniforms={oceanUniforms}
          transparent
          depthWrite={false}
          fog={false}
        />
      </mesh>

      {/* Every liquid lake & pond in ONE merged mesh (a single draw call, §6 decorative gate). */}
      {showDecorativeLakes && lakesGeom ? (
        <mesh geometry={lakesGeom} material={lakeMaterial} name="world-lakes" />
      ) : null}

      {/* Frozen lakes: opaque pale ice sheets over cold-climate basins. */}
      {/* WV2-LOOK prototype: World v2 skips the frozen-lake bbox quads. On the clipped 5,700 m
          summit plateau they read as pale squares on the snow; the snow shading covers them. */}
      {showDecorativeLakes && iceGeom && !worldV2 ? (
        <mesh geometry={iceGeom} name="world-lake-ice">
          <meshStandardMaterial color="#d7e9f2" roughness={0.35} metalness={0} />
        </mesh>
      ) : null}
    </group>
  );
};

export default WorldWater;
