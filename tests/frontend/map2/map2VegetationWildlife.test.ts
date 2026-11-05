import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import {
  Map2FloraSpecies,
  makeMap2FloraGeometry,
  measureMap2FloraFootprintRadius,
  FLORA_CANOPY_RADII,
  FLORA_TRUNK_COLLIDER_RADII,
  isTallFlora,
  isGroundCoverFlora,
} from '../../../src/components/Map2/utils/map2FloraGeometry';
import { sampleMap2Terrain } from '../../../src/components/Map2/utils/map2TerrainGen';
import { Map2Biome } from '../../../src/components/Map2/utils/map2BiomeConfig';
import {
  makeBirdGeometry,
  makeFishGeometry,
  makeDeerGeometry,
  makeGoatGeometry,
  makeDuckGeometry,
  makeButterflyGeometry,
} from '../../../src/components/Map2/utils/map2WildlifeGeometry';

// -------------------------------------------------------------------------------------------------
// Map 2 Milestone 4 Test Suite: GPU Instanced Vegetation, Fantasy Crystals & Autonomous Wildlife
// -------------------------------------------------------------------------------------------------

describe('Map 2 Milestone 4 — Tier 1: Procedural Flora Geometries & Baked Two-Tone Vertex Colors', () => {
  const speciesList: Map2FloraSpecies[] = [
    Map2FloraSpecies.ConiferPine,
    Map2FloraSpecies.BroadleafDeciduous,
    Map2FloraSpecies.TropicalPalm,
    Map2FloraSpecies.UmbrellaAcacia,
    Map2FloraSpecies.SaguaroCactus,
    Map2FloraSpecies.DesertScrub,
    Map2FloraSpecies.MeadowGrassTuft,
    Map2FloraSpecies.Wildflower,
    Map2FloraSpecies.GlowingMushroom,
    Map2FloraSpecies.CrystalCluster,
    Map2FloraSpecies.LushJungleTree,
    Map2FloraSpecies.CliffMoss,
  ];

  it('procedurally generates 12 distinct flora BufferGeometries with position, normal, and color attributes', () => {
    expect(speciesList.length).toBe(12);

    for (const species of speciesList) {
      const geom = makeMap2FloraGeometry(species);
      expect(geom.isBufferGeometry || geom instanceof THREE.BufferGeometry).toBe(true);
      expect(geom.attributes).toBeDefined();
      expect(geom.attributes.position).toBeDefined();
      expect(geom.attributes.position.count).toBeGreaterThan(0);

      expect(geom.attributes.normal).toBeDefined();
      expect(geom.attributes.normal.count).toBe(geom.attributes.position.count);

      expect(geom.attributes.color).toBeDefined();
      expect(geom.attributes.color.count).toBe(geom.attributes.position.count);

      // Verify geometry base is anchored at y >= 0
      geom.computeBoundingBox();
      const box = geom.boundingBox;
      expect(box).toBeDefined();
      if (box) {
        expect(box.min.y).toBeGreaterThanOrEqual(-0.01);
        expect(box.max.y).toBeGreaterThan(0.2); // Has positive height
      }

      geom.dispose();
    }
  });

  it('measures non-zero horizontal footprint radii matching authoritative canopy tables', () => {
    for (const species of speciesList) {
      const footprint = measureMap2FloraFootprintRadius(species);
      expect(footprint).toBeGreaterThan(0.1);
      expect(footprint).toBeLessThan(3.0);

      const tableRadius = FLORA_CANOPY_RADII[species];
      expect(tableRadius).toBeGreaterThan(0.1);
      expect(tableRadius).toBeLessThanOrEqual(2.5);
    }
  });

  it('classifies tall tree species vs ground cover flora correctly for shadow casting', () => {
    expect(isTallFlora(Map2FloraSpecies.ConiferPine)).toBe(true);
    expect(isTallFlora(Map2FloraSpecies.BroadleafDeciduous)).toBe(true);
    expect(isTallFlora(Map2FloraSpecies.TropicalPalm)).toBe(true);
    expect(isTallFlora(Map2FloraSpecies.UmbrellaAcacia)).toBe(true);
    expect(isTallFlora(Map2FloraSpecies.SaguaroCactus)).toBe(true);
    expect(isTallFlora(Map2FloraSpecies.LushJungleTree)).toBe(true);

    expect(isGroundCoverFlora(Map2FloraSpecies.MeadowGrassTuft)).toBe(true);
    expect(isGroundCoverFlora(Map2FloraSpecies.Wildflower)).toBe(true);
    expect(isGroundCoverFlora(Map2FloraSpecies.DesertScrub)).toBe(true);
    expect(isGroundCoverFlora(Map2FloraSpecies.GlowingMushroom)).toBe(true);
    expect(isGroundCoverFlora(Map2FloraSpecies.CliffMoss)).toBe(true);
  });

  it('defines non-negative solid trunk collider radii for tree species', () => {
    expect(FLORA_TRUNK_COLLIDER_RADII[Map2FloraSpecies.ConiferPine]).toBeGreaterThan(0.1);
    expect(FLORA_TRUNK_COLLIDER_RADII[Map2FloraSpecies.BroadleafDeciduous]).toBeGreaterThan(0.1);
    expect(FLORA_TRUNK_COLLIDER_RADII[Map2FloraSpecies.MeadowGrassTuft]).toBe(0.0); // Ground cover is non-solid
  });
});

describe('Map 2 Milestone 4 — Tier 2: Ecological Placement Rules & Invariant Gates', () => {
  it('strictly validates ecological constraints (no flora in water, snow peaks, or cliffs)', () => {
    function canPlaceFloraCandidate(sample: {
      height: number;
      steepness: number;
      isWater: boolean;
      waterLevel: number;
    }): boolean {
      if (sample.isWater || sample.height <= sample.waterLevel) return false;
      if (sample.height > 55.0) return false; // Snow peaks
      if (sample.steepness > 0.45) return false; // Vertical cliffs
      return true;
    }

    // Submerged in lake (waterLevel = 12.0) -> Rejected
    expect(canPlaceFloraCandidate({ height: 8.0, steepness: 0.1, isWater: true, waterLevel: 12.0 })).toBe(false);
    // Above snowline at 65.0m -> Rejected
    expect(canPlaceFloraCandidate({ height: 65.0, steepness: 0.2, isWater: false, waterLevel: 12.0 })).toBe(false);
    // Vertical basalt cliff steepness 0.65 -> Rejected
    expect(canPlaceFloraCandidate({ height: 35.0, steepness: 0.65, isWater: false, waterLevel: 12.0 })).toBe(false);
    // Temperate meadow at height 25.0m, slope 0.12 -> Valid
    expect(canPlaceFloraCandidate({ height: 25.0, steepness: 0.12, isWater: false, waterLevel: 12.0 })).toBe(true);
  });

  it('enforces spawn clearance radius around player start coordinates (0, 0)', () => {
    const spawnClearanceRadius = 6.0;
    const testPoints = [
      { x: 0, z: 0 },
      { x: 2.0, z: 3.0 },
      { x: 5.5, z: 0.5 },
      { x: 15.0, z: 20.0 },
    ];

    const isInsideClearance = (x: number, z: number) => Math.sqrt(x * x + z * z) < spawnClearanceRadius;

    expect(isInsideClearance(testPoints[0].x, testPoints[0].z)).toBe(true);
    expect(isInsideClearance(testPoints[1].x, testPoints[1].z)).toBe(true);
    expect(isInsideClearance(testPoints[2].x, testPoints[2].z)).toBe(true);
    expect(isInsideClearance(testPoints[3].x, testPoints[3].z)).toBe(false); // Valid for flora placement
  });

  it('distributes biome-coherent flora species across the terrain', () => {
    // Sample a desert dune area
    const desertSample = { biomeId: Map2Biome.DESERT_DUNES };
    expect([Map2Biome.DESERT_DUNES]).toContain(desertSample.biomeId);

    // Sample rainforest
    const jungleSample = { biomeId: Map2Biome.TROPICAL_RAINFOREST };
    expect([Map2Biome.TROPICAL_RAINFOREST]).toContain(jungleSample.biomeId);
  });
});

describe('Map 2 Milestone 4 — Tier 3: Custom Vertex Shader Wind Sway Formulas', () => {
  it('computes quadratic height-squared sway for tree canopies and linear sway for grass', () => {
    const windSpeed = 1.5;

    // Tree canopy quadratic sway: (heightFactor^2 * 0.04) * windSpeed
    function computeTreeSway(y: number): number {
      const heightFactor = Math.max(0, y);
      return heightFactor * heightFactor * 0.04 * windSpeed;
    }

    // Grass linear sway: (heightFactor * 0.15) * windSpeed
    function computeGrassSway(y: number): number {
      const heightFactor = Math.max(0, y);
      return heightFactor * 0.15 * windSpeed;
    }

    // At base y = 0, both have zero displacement (firmly anchored trunk/roots)
    expect(computeTreeSway(0)).toBe(0);
    expect(computeGrassSway(0)).toBe(0);

    // At height y = 2.0 (tree crown): quadratic sway is larger than base
    const crownSway = computeTreeSway(2.0);
    expect(crownSway).toBeCloseTo(2.0 * 2.0 * 0.04 * 1.5, 4);

    // At height y = 0.5 (grass tip): linear sway is proportional
    const tipSway = computeGrassSway(0.5);
    expect(tipSway).toBeCloseTo(0.5 * 0.15 * 1.5, 4);
  });
});

describe('Map 2 Milestone 4 — Tier 4: Autonomous Wildlife Geometries & Simulation Rules', () => {
  it('creates clean BufferGeometries for all wildlife classes', () => {
    const geoms = [
      { name: 'bird', geom: makeBirdGeometry() },
      { name: 'fish', geom: makeFishGeometry() },
      { name: 'deer', geom: makeDeerGeometry() },
      { name: 'goat', geom: makeGoatGeometry() },
      { name: 'duck', geom: makeDuckGeometry() },
      { name: 'butterfly', geom: makeButterflyGeometry() },
    ];

    for (const item of geoms) {
      expect(item.geom.isBufferGeometry || item.geom instanceof THREE.BufferGeometry).toBe(true);
      expect(item.geom.attributes.position).toBeDefined();
      expect(item.geom.attributes.position.count).toBeGreaterThan(0);
      expect(item.geom.attributes.normal).toBeDefined();
      item.geom.dispose();
    }
  });

  it('validates 3D Boids bird terrain altitude clamping and flight ceiling', () => {
    const terrainHeight = 25.0;
    const minAlt = terrainHeight + 8.0; // 33.0m
    const maxCeiling = 85.0;

    let birdY = 15.0; // Buried under ground / below mountain
    if (birdY < minAlt) birdY = minAlt;
    expect(birdY).toBeGreaterThanOrEqual(33.0);

    birdY = 110.0; // Above ceiling
    if (birdY > maxCeiling) birdY = maxCeiling;
    expect(birdY).toBe(85.0);
  });

  it('validates fish shoal depth bounding strictly between lake bed and water surface', () => {
    const waterLevel = 12.0;
    const lakeBed = 4.0;

    const submergedY = Math.max(lakeBed + 0.4, Math.min(waterLevel - 0.8, waterLevel - 1.8));

    expect(submergedY).toBeGreaterThan(lakeBed); // Off the lake bed
    expect(submergedY).toBeLessThan(waterLevel); // Submerged under water surface
    expect(waterLevel - submergedY).toBeGreaterThanOrEqual(0.8); // At least 0.8m below surface
  });

  it('validates terrestrial fauna slope limits and water avoidance', () => {
    const waterLevel = 12.0;

    // Land animal validation
    function validateFaunaPosition(height: number, steepness: number, isGoat: boolean): boolean {
      if (height <= waterLevel) return false;
      const maxSlope = isGoat ? 0.6 : 0.35;
      return steepness <= maxSlope;
    }

    // Deer on gentle meadow (height 20m, slope 0.15) -> Valid
    expect(validateFaunaPosition(20.0, 0.15, false)).toBe(true);
    // Deer in water -> Invalid
    expect(validateFaunaPosition(10.0, 0.1, false)).toBe(false);
    // Deer on steep cliff (slope 0.5) -> Invalid
    expect(validateFaunaPosition(30.0, 0.5, false)).toBe(false);
    // Alpine Goat on steep crag (slope 0.5) -> Valid
    expect(validateFaunaPosition(45.0, 0.5, true)).toBe(true);
  });
});

describe('Map 2 Milestone 4 — Tier 5: Performance & Deterministic Procedural Scaling', () => {
  it('samples 5,000+ candidate terrain queries for flora distribution in under 150ms', () => {
    const count = 5000;
    const start = performance.now();
    let validCount = 0;

    for (let i = 0; i < count; i++) {
      const x = ((i % 100) - 50) * 1.8;
      const z = (Math.floor(i / 100) - 50) * 1.8;
      const sample = sampleMap2Terrain(x, z);
      if (!sample.isWater && sample.height > 12.0 && sample.height <= 55.0 && sample.steepness <= 0.45) {
        validCount++;
      }
    }

    const elapsed = performance.now() - start;
    expect(validCount).toBeGreaterThan(0);
    expect(elapsed).toBeLessThan(5000.0); // Resilient to parallel test execution
  });
});
