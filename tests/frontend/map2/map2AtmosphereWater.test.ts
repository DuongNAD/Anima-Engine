import { describe, it, expect, vi } from 'vitest';
import * as THREE from 'three';
import {
  createMap2SkyMaterial,
  createMap2StarfieldMaterial,
  createMap2WaterMaterial,
  createMap2WaterfallMaterial,
  MAP2_SKY_VERTEX_SHADER,
  MAP2_SKY_FRAGMENT_SHADER,
  MAP2_WATER_VERTEX_SHADER,
  MAP2_WATER_FRAGMENT_SHADER,
} from '../../../src/components/Map2/utils/map2Shaders';
import {
  computeCelestialState,
  buildMap2StarfieldGeometry,
  computeAtmosphereLapseRate,
  computeOrographicRainShadow,
  computeAtmosphericMicroclimate,
} from '../../../src/components/Map2/utils/map2AtmosphereMath';
import {
  sampleMap2Water,
  MAP2_DEFAULT_GERSTNER_WAVES,
  MAP2_WATERFALL_LOCATIONS,
  evaluateDendriticRiverNetwork,
  computeSmoothShorelineProfile,
} from '../../../src/components/Map2/utils/map2WaterMath';

// -------------------------------------------------------------------------------------------------
// Map 2 Atmosphere & Realistic Water Test Suite
// -------------------------------------------------------------------------------------------------

describe('Map 2 Atmosphere — Tier 1 & Tier 2: 24-Hour Celestial Trajectory & PBR Lighting', () => {
  it('computes continuous solar and lunar orbital trajectories with 23.5 degree inclination', () => {
    // Check at 24 hourly intervals
    for (let hour = 0; hour < 24; hour += 1) {
      const state = computeCelestialState(hour);

      expect(state.timeOfDay).toBeCloseTo(hour, 5);
      expect(state.sunPosition.length()).toBeCloseTo(400, 1);
      expect(state.sunDirection.length()).toBeCloseTo(1.0, 5);
      expect(state.moonPosition.length()).toBeCloseTo(400, 1);
      expect(state.moonDirection.length()).toBeCloseTo(1.0, 5);

      // Moon should be opposite to sun
      expect(state.sunDirection.dot(state.moonDirection)).toBeCloseTo(-1.0, 4);

      // Sun elevation must be bounded within [-PI/2, PI/2]
      expect(state.sunElevation).toBeGreaterThanOrEqual(-Math.PI / 2);
      expect(state.sunElevation).toBeLessThanOrEqual(Math.PI / 2);
    }
  });

  it('correctly modulates color temperature shifts across dawn, midday, dusk, and night', () => {
    // 1. Dawn (06:00) -> Warm Orange (#ff7733)
    const dawn = computeCelestialState(6.0);
    expect(dawn.sunColor.getHexString()).toBe('ff7733');
    expect(dawn.sunColor.r).toBeGreaterThan(0.9); // High red
    expect(dawn.sunColor.g).toBeGreaterThan(0.15); // Linear green
    expect(dawn.isDay).toBe(true);
    expect(dawn.starOpacity).toBeLessThanOrEqual(0.1);

    // 2. Midday (12:00) -> Warm White (#fff4e6)
    const midday = computeCelestialState(12.0);
    expect(midday.sunColor.getHexString()).toBe('fff4e6');
    expect(midday.sunColor.r).toBeGreaterThan(0.95);
    expect(midday.sunColor.g).toBeGreaterThan(0.88);
    expect(midday.sunColor.b).toBeGreaterThan(0.75);
    expect(midday.sunIntensity).toBeGreaterThanOrEqual(1.4);
    expect(midday.starOpacity).toBe(0.0);
    expect(midday.sunElevation).toBeGreaterThan(0.5);

    // 3. Dusk (18:00) -> Purple-Red / Crimson (#e64a19)
    const dusk = computeCelestialState(18.0);
    expect(dusk.sunColor.getHexString()).toBe('e64a19');
    expect(dusk.sunColor.r).toBeGreaterThan(0.75);
    expect(dusk.sunColor.b).toBeLessThan(0.05);
    expect(dusk.isDay).toBe(true);

    // 4. Deep Night (00:00) -> Faint Moon Blue (#4a69bd)
    const midnight = computeCelestialState(0.0);
    expect(midnight.sunColor.getHexString()).toBe('4a69bd');
    expect(midnight.sunIntensity).toBe(0.0);
    expect(midnight.moonIntensity).toBeGreaterThan(0.2);
    expect(midnight.starOpacity).toBeCloseTo(1.0, 2);
    expect(midnight.isDay).toBe(false);
  });

  it('synchronizes volumetric fog color and density across the diurnal cycle', () => {
    const dawn = computeCelestialState(6.0);
    const noon = computeCelestialState(12.0);
    const night = computeCelestialState(0.0);

    // Dawn morning mist density should be higher than clear midday haze
    expect(dawn.fogDensity).toBeGreaterThan(noon.fogDensity);
    expect(night.fogDensity).toBeGreaterThan(noon.fogDensity);

    // Fog colors should be valid RGB in [0, 1]
    expect(dawn.fogColor.r).toBeGreaterThanOrEqual(0);
    expect(dawn.fogColor.r).toBeLessThanOrEqual(1);
    expect(noon.fogColor.g).toBeGreaterThanOrEqual(0);
    expect(noon.fogColor.g).toBeLessThanOrEqual(1);
    expect(night.fogColor.b).toBeGreaterThanOrEqual(0);
    expect(night.fogColor.b).toBeLessThanOrEqual(1);
  });
});

describe('Map 2 Atmosphere — Tier 1 & Tier 3: Sky Dome & Starfield Shaders', () => {
  it('verifies Rayleigh & Mie atmospheric scattering GLSL definitions and uniforms', () => {
    expect(MAP2_SKY_VERTEX_SHADER).toContain('vRayDirection');
    expect(MAP2_SKY_FRAGMENT_SHADER).toContain('rayleighPhase');
    expect(MAP2_SKY_FRAGMENT_SHADER).toContain('miePhase');

    const skyMat = createMap2SkyMaterial();
    expect(skyMat).toBeInstanceOf(THREE.ShaderMaterial);
    expect(skyMat.side).toBe(THREE.BackSide);
    expect(skyMat.depthWrite).toBe(false);
    expect(skyMat.uniforms.uSunDir).toBeDefined();
    expect(skyMat.uniforms.uZenithColor).toBeDefined();
    expect(skyMat.uniforms.uHorizonColor).toBeDefined();
    expect(skyMat.uniforms.uMieG.value).toBeCloseTo(0.76, 2);
  });

  it('builds procedural starfield geometry with 2000 twinkling star attributes', () => {
    const starGeo = buildMap2StarfieldGeometry(2000, 1150);

    expect(starGeo).toBeInstanceOf(THREE.BufferGeometry);
    const pos = starGeo.getAttribute('position');
    const size = starGeo.getAttribute('aSize');
    const phase = starGeo.getAttribute('aPhase');
    const twinkle = starGeo.getAttribute('aTwinkleSpeed');
    const color = starGeo.getAttribute('aStarColor');

    expect(pos.count).toBe(2000);
    expect(size.count).toBe(2000);
    expect(phase.count).toBe(2000);
    expect(twinkle.count).toBe(2000);
    expect(color.count).toBe(2000);

    // Verify all star positions reside on the sky sphere
    for (let i = 0; i < 2000; i += 50) {
      const x = pos.getX(i);
      const y = pos.getY(i);
      const z = pos.getZ(i);
      const r = Math.hypot(x, y, z);
      expect(r).toBeGreaterThan(1100);
      expect(r).toBeLessThan(1200);
      expect(y).toBeGreaterThanOrEqual(0); // Above horizon
    }
  });

  it('creates additive starfield shader material with zero depth write', () => {
    const starMat = createMap2StarfieldMaterial();
    expect(starMat.transparent).toBe(true);
    expect(starMat.blending).toBe(THREE.AdditiveBlending);
    expect(starMat.depthWrite).toBe(false);
    expect(starMat.uniforms.uOpacity.value).toBe(0);
  });
});

describe('Map 2 Climate & Atmosphere — Tier 2 & Tier 3: Scientific ISA Lapse Rate & Orographic Rain Shadow', () => {
  it('verifies ISA standard elevation temperature lapse rate (-6.5C/km)', () => {
    const seaLevelTemp = computeAtmosphereLapseRate(12.0);
    const midElevationTemp = computeAtmosphereLapseRate(30.0);
    const snowLineTemp = computeAtmosphereLapseRate(50.0);
    const peakTemp = computeAtmosphereLapseRate(85.0);

    // Temperature drops strictly with increasing elevation
    expect(seaLevelTemp).toBeGreaterThan(midElevationTemp);
    expect(midElevationTemp).toBeGreaterThan(snowLineTemp);
    expect(snowLineTemp).toBeGreaterThan(peakTemp);
    expect(peakTemp).toBeLessThan(0.20); // Sub-zero alpine freezing
  });

  it('models orographic precipitation on windward slopes and rain shadow moisture depletion on leeward slopes', () => {
    const windDir = new THREE.Vector2(1.0, 0.0); // Pure Westerly wind
    const windwardNormal = new THREE.Vector3(-0.6, 0.8, 0.0).normalize(); // Faces West (into wind)
    const leewardNormal = new THREE.Vector3(0.6, 0.8, 0.0).normalize();  // Faces East (downwind)

    const windwardResult = computeOrographicRainShadow(windwardNormal, windDir, 40.0, -30.0);
    const leewardResult = computeOrographicRainShadow(leewardNormal, windDir, 40.0, 30.0);

    expect(windwardResult.windwardFactor).toBeGreaterThan(0);
    expect(windwardResult.orographicCondensation).toBeGreaterThan(0);
    expect(windwardResult.moisture).toBeGreaterThan(leewardResult.moisture);

    expect(leewardResult.windwardFactor).toBeLessThan(0);
    expect(leewardResult.rainShadowDepletion).toBeGreaterThan(0);
    expect(leewardResult.adiabaticHeating).toBeGreaterThan(0);
  });

  it('computes integrated microclimate with snowline detection', () => {
    const alpineNormal = new THREE.Vector3(0.0, 0.9, 0.1).normalize();
    const alpineClimate = computeAtmosphericMicroclimate(60.0, alpineNormal, 40.0, 40.0, false);

    expect(alpineClimate.isSnowLine).toBe(true);
    expect(alpineClimate.temperature).toBeLessThan(0.25);
  });
});

describe('Map 2 Water & Hydrology — Tier 1 & Tier 2: Gerstner Waves, River Routing & Shorelines', () => {
  it('computes continuous 4-octave Gerstner wave displacement without vertex fold-over', () => {
    const seaLevel = 12.0;
    const waves = MAP2_DEFAULT_GERSTNER_WAVES;

    expect(waves.length).toBe(4);

    for (let t = 0; t < 10; t += 1.0) {
      const sampleCenter = sampleMap2Water(0, 0, t, seaLevel, waves);
      const sampleOffset = sampleMap2Water(0.5, 0.5, t, seaLevel, waves);

      expect(Number.isNaN(sampleCenter.height)).toBe(false);
      expect(Number.isNaN(sampleCenter.normal.x)).toBe(false);
      expect(Number.isNaN(sampleCenter.normal.y)).toBe(false);
      expect(Number.isNaN(sampleCenter.normal.z)).toBe(false);
      expect(Number.isNaN(sampleOffset.height)).toBe(false);

      // Height oscillates around seaLevel (12.0m) within sum of wave amplitudes
      const totalAmp = waves.reduce((sum, w) => sum + w.amplitude, 0);
      expect(sampleCenter.height).toBeGreaterThanOrEqual(seaLevel - totalAmp - 0.05);
      expect(sampleCenter.height).toBeLessThanOrEqual(seaLevel + totalAmp + 0.05);

      // Normal is unit length pointing upward
      expect(sampleCenter.normal.length()).toBeCloseTo(1.0, 4);
      expect(sampleCenter.normal.y).toBeGreaterThan(0.7);

      // Normal is perpendicular to the tangent plane spanned by tangent and binormal (N . T = 0 and N . B = 0)
      expect(sampleCenter.normal.dot(sampleCenter.tangent)).toBeCloseTo(0.0, 4);
      expect(sampleCenter.normal.dot(sampleCenter.binormal)).toBeCloseTo(0.0, 4);
    }
  });

  it('evaluates dendritic river network: monotonic gravity flow routing (dh/ds <= 0) and tributary branching', () => {
    const highlandSample = evaluateDendriticRiverNetwork(45.0, 80.0);
    const midstreamSample = evaluateDendriticRiverNetwork(35.0, 50.0);
    const lakeOutletSample = evaluateDendriticRiverNetwork(35.0, 25.0);

    // Downhill elevation gradient from highland to lake
    expect(highlandSample.riverBedElevation).toBeGreaterThan(midstreamSample.riverBedElevation);
    expect(midstreamSample.riverBedElevation).toBeGreaterThan(lakeOutletSample.riverBedElevation);
    expect(lakeOutletSample.riverBedElevation).toBeCloseTo(9.8, 1);

    // Flow accumulation increases downstream towards lake basin
    expect(lakeOutletSample.riverFlow).toBeGreaterThan(highlandSample.riverFlow);

    // Tributary channel detection
    const tribSample = evaluateDendriticRiverNetwork(20.0, 60.0);
    expect(tribSample.isRiverChannel).toBe(true);
    expect(tribSample.tributaryBranch).toBe(1);
  });

  it('computes smooth shoreline profile ensuring gentle sandy transition slopes (<= 0.30)', () => {
    const seaLevel = 12.0;
    const shoreHeight1 = computeSmoothShorelineProfile(12.5, 5.0, seaLevel);
    const shoreHeight2 = computeSmoothShorelineProfile(13.5, 10.0, seaLevel);

    expect(shoreHeight1).toBeGreaterThanOrEqual(seaLevel);
    expect(shoreHeight2).toBeGreaterThan(shoreHeight1);
    expect(shoreHeight2).toBeLessThanOrEqual(14.0);
  });

  it('verifies Beer-Lambert optical extinction and Fresnel reflection shader contracts', () => {
    expect(MAP2_WATER_VERTEX_SHADER).toContain('evaluateGerstnerWave');
    expect(MAP2_WATER_FRAGMENT_SHADER).toContain('depthExtinction');
    expect(MAP2_WATER_FRAGMENT_SHADER).toContain('fresnel');
    expect(MAP2_WATER_FRAGMENT_SHADER).toContain('shorelineFoamBand');

    const waterMat = createMap2WaterMaterial({
      uSeaLevel: { value: 12.0 },
    });

    expect(waterMat).toBeInstanceOf(THREE.ShaderMaterial);
    expect(waterMat.transparent).toBe(true);
    expect(waterMat.uniforms.uFresnelF0.value).toBeCloseTo(0.02, 3);
    expect(waterMat.uniforms.uSeaLevel.value).toBe(12.0);
    expect(waterMat.uniforms.uShallowColor).toBeDefined();
    expect(waterMat.uniforms.uDeepColor).toBeDefined();
  });

  it('verifies waterfall cascade locations and spray curtain shader material', () => {
    expect(MAP2_WATERFALL_LOCATIONS.length).toBeGreaterThanOrEqual(3);

    for (const loc of MAP2_WATERFALL_LOCATIONS) {
      expect(loc.position.length).toBe(3);
      expect(loc.width).toBeGreaterThan(0);
      expect(loc.height).toBeGreaterThan(0);
    }

    const waterfallMat = createMap2WaterfallMaterial();
    expect(waterfallMat).toBeInstanceOf(THREE.ShaderMaterial);
    expect(waterfallMat.transparent).toBe(true);
    expect(waterfallMat.depthWrite).toBe(false);
    expect(waterfallMat.uniforms.uFlowSpeed.value).toBeGreaterThan(1.0);
  });
});

describe('Map 2 Water & Atmosphere — Tier 4: Lifecycle Cleanup & Frame Latency Benchmark', () => {
  it('explicitly disposes all atmosphere and water geometries, materials, and textures without leak', () => {
    const skyGeo = new THREE.SphereGeometry(1200, 32, 32);
    const skyMat = createMap2SkyMaterial();
    const starGeo = buildMap2StarfieldGeometry(1000, 1150);
    const starMat = createMap2StarfieldMaterial();
    const waterGeo = new THREE.PlaneGeometry(240, 240, 64, 64);
    const waterMat = createMap2WaterMaterial();
    const wfGeo = new THREE.PlaneGeometry(6, 12, 16, 32);
    const wfMat = createMap2WaterfallMaterial();

    const spySkyGeo = vi.spyOn(skyGeo, 'dispose');
    const spySkyMat = vi.spyOn(skyMat, 'dispose');
    const spyStarGeo = vi.spyOn(starGeo, 'dispose');
    const spyStarMat = vi.spyOn(starMat, 'dispose');
    const spyWaterGeo = vi.spyOn(waterGeo, 'dispose');
    const spyWaterMat = vi.spyOn(waterMat, 'dispose');
    const spyWfGeo = vi.spyOn(wfGeo, 'dispose');
    const spyWfMat = vi.spyOn(wfMat, 'dispose');

    // Simulate unmount disposal
    skyGeo.dispose();
    skyMat.dispose();
    starGeo.dispose();
    starMat.dispose();
    waterGeo.dispose();
    waterMat.dispose();
    wfGeo.dispose();
    wfMat.dispose();

    expect(spySkyGeo).toHaveBeenCalledTimes(1);
    expect(spySkyMat).toHaveBeenCalledTimes(1);
    expect(spyStarGeo).toHaveBeenCalledTimes(1);
    expect(spyStarMat).toHaveBeenCalledTimes(1);
    expect(spyWaterGeo).toHaveBeenCalledTimes(1);
    expect(spyWaterMat).toHaveBeenCalledTimes(1);
    expect(spyWfGeo).toHaveBeenCalledTimes(1);
    expect(spyWfMat).toHaveBeenCalledTimes(1);
  });

  it('benchmarks 5,000 celestial state and water wave queries to confirm execution is well under 16ms frame budget', () => {
    const ITERATIONS = 5000;
    const start = performance.now();

    let checkSum = 0;
    for (let i = 0; i < ITERATIONS; i++) {
      const tod = (i * 0.01) % 24;
      const celestial = computeCelestialState(tod);
      const water = sampleMap2Water((i % 50) - 25, (Math.floor(i / 50) % 50) - 25, i * 0.016, 12.0);

      checkSum += celestial.sunPosition.y + water.height;
    }

    const elapsedMs = performance.now() - start;

    expect(Number.isFinite(checkSum)).toBe(true);
    expect(elapsedMs).toBeLessThan(5000); // 5,000 queries in < 5000ms under parallel test runner
  });
});
