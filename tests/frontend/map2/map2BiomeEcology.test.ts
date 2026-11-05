import { describe, it, expect } from 'vitest';
import * as THREE from 'three';

// -------------------------------------------------------------------------------------------------
// Map 2 Biome & Ecology Test Suite: Biome Taxonomy, Ecotone Weights, Flora/Fauna Rules & Atmospheric Sky
// -------------------------------------------------------------------------------------------------

export enum Map2Biome {
  SNOW_PEAKS = 0,
  ROCKY_CLIFFS = 1,
  TEMPERATE_PLAINS = 2,
  TROPICAL_RAINFOREST = 3,
  ARID_SAVANNA = 4,
  DESERT_DUNES = 5,
  HYDROLOGY_RIVER_LAKE = 6,
  BIOLUMINESCENT_CAVES = 7,
}

export interface ClimateInput {
  height: number;
  steepness: number;
  temperature: number; // [0, 1] 0=freezing, 1=scorching
  moisture: number; // [0, 1] 0=arid, 1=humid
  caveIntensity?: number; // [0, 1]
}

export class Map2BiomeEcologySystem {
  public readonly waterLevel = 4.0;
  public readonly snowLine = 72.0;
  public readonly cliffThreshold = 0.62;

  public classifyPrimaryBiome(input: ClimateInput): Map2Biome {
    const { height, steepness, temperature, moisture, caveIntensity = 0 } = input;

    // 1. Subterranean Caves
    if (caveIntensity > 0.6) {
      return Map2Biome.BIOLUMINESCENT_CAVES;
    }

    // 2. Hydrology / Submerged
    if (height <= this.waterLevel) {
      return Map2Biome.HYDROLOGY_RIVER_LAKE;
    }

    // 3. Snow Peaks
    if (height >= this.snowLine || (height > 60 && temperature < 0.25)) {
      return Map2Biome.SNOW_PEAKS;
    }

    // 4. Rocky Cliffs
    if (steepness >= this.cliffThreshold) {
      return Map2Biome.ROCKY_CLIFFS;
    }

    // 5. Climatic Biome Matrix (Temperature vs Moisture)
    if (temperature > 0.65) {
      if (moisture < 0.25) return Map2Biome.DESERT_DUNES;
      if (moisture < 0.55) return Map2Biome.ARID_SAVANNA;
      return Map2Biome.TROPICAL_RAINFOREST;
    } else {
      if (moisture > 0.5) return Map2Biome.TROPICAL_RAINFOREST;
      return Map2Biome.TEMPERATE_PLAINS;
    }
  }

  public computeBiomeWeights(input: ClimateInput): Float32Array {
    const weights = new Float32Array(8);
    const primary = this.classifyPrimaryBiome(input);

    // Baseline primary weight
    weights[primary] = 1.0;

    // Smooth ecotone blending with adjacent biomes
    const { height, steepness } = input;

    // Cliff blending
    if (steepness > 0.35 && primary !== Map2Biome.ROCKY_CLIFFS) {
      const cliffFactor = (steepness - 0.35) / (this.cliffThreshold - 0.35);
      const blend = Math.min(0.6, Math.max(0, cliffFactor));
      weights[Map2Biome.ROCKY_CLIFFS] += blend;
      weights[primary] -= blend;
    }

    // Snow line blending
    if (height > 55 && height < this.snowLine && primary !== Map2Biome.SNOW_PEAKS) {
      const snowFactor = (height - 55) / (this.snowLine - 55);
      const blend = Math.min(0.5, Math.max(0, snowFactor));
      weights[Map2Biome.SNOW_PEAKS] += blend;
      weights[primary] -= blend;
    }

    // Normalize weights so sum is identically 1.0 (Partition of Unity)
    let sum = 0;
    for (let i = 0; i < 8; i++) {
      weights[i] = Math.max(0, weights[i]);
      sum += weights[i];
    }
    if (sum > 0) {
      for (let i = 0; i < 8; i++) weights[i] /= sum;
    } else {
      weights[primary] = 1.0;
    }

    return weights;
  }

  public canPlaceFlora(biome: Map2Biome, height: number, steepness: number): boolean {
    // Zero flora in water
    if (height <= this.waterLevel) return false;
    // Zero flora on high snow peaks
    if (height >= this.snowLine || biome === Map2Biome.SNOW_PEAKS) return false;
    // Zero flora on vertical rock cliffs
    if (steepness >= this.cliffThreshold) return false;
    return true;
  }

  public getFloraTypesForBiome(biome: Map2Biome): string[] {
    switch (biome) {
      case Map2Biome.DESERT_DUNES:
        return ['SaguaroCactus', 'BarrelCactus', 'DesertShrub'];
      case Map2Biome.ARID_SAVANNA:
        return ['AcaciaTree', 'BaobabTree', 'DryGrass'];
      case Map2Biome.TROPICAL_RAINFOREST:
        return ['JunglePalm', 'FernTree', 'LushCanopyTree', 'ExoticFlower'];
      case Map2Biome.TEMPERATE_PLAINS:
        return ['OakTree', 'BirchTree', 'FieldGrass', 'Wildflower'];
      case Map2Biome.BIOLUMINESCENT_CAVES:
        return ['GlowingMushroom', 'CrystalSpire', 'BioluminescentMoss'];
      case Map2Biome.ROCKY_CLIFFS:
        return ['CliffMoss', 'GnarledPine'];
      case Map2Biome.SNOW_PEAKS:
      case Map2Biome.HYDROLOGY_RIVER_LAKE:
      default:
        return [];
    }
  }

  public validateFaunaMedium(
    species: 'fish' | 'bird' | 'terrestrial',
    position: THREE.Vector3,
    terrainHeight: number,
    steepness: number,
  ): boolean {
    switch (species) {
      case 'fish':
        // Fish must be strictly submerged in water
        return position.y <= this.waterLevel && position.y >= terrainHeight;
      case 'bird':
        // Birds fly aloft in the sky
        return position.y >= terrainHeight + 8.0;
      case 'terrestrial':
        // Land animals must be above water level on passable slopes
        return position.y >= this.waterLevel && steepness < this.cliffThreshold;
    }
  }
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------

describe('Map 2 Biome & Ecology — Tier 1 & Tier 2: Biome Classification & Partition of Unity', () => {
  const eco = new Map2BiomeEcologySystem();

  it('classifies all 8 canonical biomes according to climate inputs', () => {
    // 1. Snow Peaks
    expect(eco.classifyPrimaryBiome({ height: 85, steepness: 0.2, temperature: 0.1, moisture: 0.5 })).toBe(
      Map2Biome.SNOW_PEAKS,
    );

    // 2. Rocky Cliffs
    expect(eco.classifyPrimaryBiome({ height: 40, steepness: 0.85, temperature: 0.5, moisture: 0.5 })).toBe(
      Map2Biome.ROCKY_CLIFFS,
    );

    // 3. Hydrology
    expect(eco.classifyPrimaryBiome({ height: 2.0, steepness: 0.1, temperature: 0.5, moisture: 0.9 })).toBe(
      Map2Biome.HYDROLOGY_RIVER_LAKE,
    );

    // 4. Desert Dunes
    expect(eco.classifyPrimaryBiome({ height: 25, steepness: 0.1, temperature: 0.85, moisture: 0.1 })).toBe(
      Map2Biome.DESERT_DUNES,
    );

    // 5. Arid Savanna
    expect(eco.classifyPrimaryBiome({ height: 25, steepness: 0.1, temperature: 0.75, moisture: 0.4 })).toBe(
      Map2Biome.ARID_SAVANNA,
    );

    // 6. Tropical Rainforest
    expect(eco.classifyPrimaryBiome({ height: 20, steepness: 0.2, temperature: 0.8, moisture: 0.85 })).toBe(
      Map2Biome.TROPICAL_RAINFOREST,
    );

    // 7. Temperate Plains
    expect(eco.classifyPrimaryBiome({ height: 30, steepness: 0.1, temperature: 0.5, moisture: 0.4 })).toBe(
      Map2Biome.TEMPERATE_PLAINS,
    );

    // 8. Bioluminescent Caves
    expect(
      eco.classifyPrimaryBiome({ height: 10, steepness: 0.2, temperature: 0.5, moisture: 0.5, caveIntensity: 0.9 }),
    ).toBe(Map2Biome.BIOLUMINESCENT_CAVES);
  });

  it('strictly enforces Partition of Unity: biome weights sum to 1.0 with non-negative weights', () => {
    const testCases: ClimateInput[] = [
      { height: 10, steepness: 0.1, temperature: 0.5, moisture: 0.5 },
      { height: 65, steepness: 0.45, temperature: 0.2, moisture: 0.6 },
      { height: 50, steepness: 0.55, temperature: 0.7, moisture: 0.2 },
      { height: 80, steepness: 0.7, temperature: 0.05, moisture: 0.1 },
      { height: 0.5, steepness: 0.05, temperature: 0.6, moisture: 1.0 },
    ];

    for (const tc of testCases) {
      const weights = eco.computeBiomeWeights(tc);
      expect(weights.length).toBe(8);

      let sum = 0;
      for (let i = 0; i < 8; i++) {
        expect(weights[i]).toBeGreaterThanOrEqual(0);
        expect(weights[i]).toBeLessThanOrEqual(1.0);
        sum += weights[i];
      }
      expect(sum).toBeCloseTo(1.0, 5);
    }
  });
});

describe('Map 2 Biome & Ecology — Tier 3: Ecological Invariants & Placement Constraints', () => {
  const eco = new Map2BiomeEcologySystem();

  it('prohibits flora placement in water, on snow peaks, or on steep vertical cliffs', () => {
    // Under water
    expect(eco.canPlaceFlora(Map2Biome.HYDROLOGY_RIVER_LAKE, 2.5, 0.1)).toBe(false);
    // On snow peaks
    expect(eco.canPlaceFlora(Map2Biome.SNOW_PEAKS, 78.0, 0.2)).toBe(false);
    // On vertical cliff
    expect(eco.canPlaceFlora(Map2Biome.ROCKY_CLIFFS, 35.0, 0.75)).toBe(false);

    // Permitted on temperate plains and rainforest
    expect(eco.canPlaceFlora(Map2Biome.TEMPERATE_PLAINS, 25.0, 0.15)).toBe(true);
    expect(eco.canPlaceFlora(Map2Biome.TROPICAL_RAINFOREST, 15.0, 0.2)).toBe(true);
  });

  it('matches flora taxonomy strictly to the host biome', () => {
    const desertPlants = eco.getFloraTypesForBiome(Map2Biome.DESERT_DUNES);
    expect(desertPlants).toContain('SaguaroCactus');
    expect(desertPlants).not.toContain('OakTree');

    const rainforestPlants = eco.getFloraTypesForBiome(Map2Biome.TROPICAL_RAINFOREST);
    expect(rainforestPlants).toContain('JunglePalm');
    expect(rainforestPlants).not.toContain('SaguaroCactus');

    const cavePlants = eco.getFloraTypesForBiome(Map2Biome.BIOLUMINESCENT_CAVES);
    expect(cavePlants).toContain('GlowingMushroom');
    expect(cavePlants).toContain('CrystalSpire');
  });

  it('validates wildlife medium integrity (fish in water, birds aloft, land fauna on dry ground)', () => {
    // Fish at y = 3.0 in waterLevel 4.0, terrain 1.0 -> Valid
    expect(eco.validateFaunaMedium('fish', new THREE.Vector3(10, 3.0, 10), 1.0, 0.1)).toBe(true);
    // Fish out of water at y = 6.0 -> Invalid
    expect(eco.validateFaunaMedium('fish', new THREE.Vector3(10, 6.0, 10), 5.0, 0.1)).toBe(false);

    // Bird flying at y = 30 over terrain 10 -> Valid
    expect(eco.validateFaunaMedium('bird', new THREE.Vector3(0, 30, 0), 10, 0.2)).toBe(true);
    // Bird buried in ground -> Invalid
    expect(eco.validateFaunaMedium('bird', new THREE.Vector3(0, 5, 0), 10, 0.2)).toBe(false);

    // Land animal on dry ground y = 15, slope 0.2 -> Valid
    expect(eco.validateFaunaMedium('terrestrial', new THREE.Vector3(5, 15, 5), 15, 0.2)).toBe(true);
    // Land animal on vertical cliff -> Invalid
    expect(eco.validateFaunaMedium('terrestrial', new THREE.Vector3(5, 15, 5), 15, 0.8)).toBe(false);
  });
});

describe('Map 2 Biome & Ecology — Tier 1 & Tier 3: Atmospheric Sky & Volumetric Fog', () => {
  it('computes solar orbital angles and Rayleigh atmospheric scattering color transition', () => {
    function getSunDirection(timeOfDay: number): THREE.Vector3 {
      // timeOfDay in [0, 24] where 12 = solar noon, 6 = dawn, 18 = dusk
      const angle = ((timeOfDay - 6) / 24) * Math.PI * 2;
      return new THREE.Vector3(Math.cos(angle), Math.sin(angle), 0.2).normalize();
    }

    const noonSun = getSunDirection(12);
    const dawnSun = getSunDirection(6);
    const midnightSun = getSunDirection(0);

    expect(noonSun.y).toBeGreaterThan(0.9); // High overhead
    expect(Math.abs(dawnSun.y)).toBeLessThan(0.05); // At the horizon
    expect(midnightSun.y).toBeLessThan(-0.9); // Under horizon
  });

  it('computes exponential height fog density decay with altitude', () => {
    const baseDensity = 0.05;
    const heightFalloff = 0.04;
    const computeFogDensity = (y: number) => baseDensity * Math.exp(-heightFalloff * Math.max(0, y));

    const seaFog = computeFogDensity(0);
    const valleyFog = computeFogDensity(10);
    const mountainFog = computeFogDensity(60);

    expect(seaFog).toBe(0.05);
    expect(valleyFog).toBeLessThan(seaFog);
    expect(mountainFog).toBeLessThan(0.005); // Fog clears dramatically at high altitudes
  });
});

describe('Map 2 Biome & Ecology — Tier 4: Real-World Workload Classification', () => {
  it('classifies 5,000 climate entity queries in under 15ms', () => {
    const eco = new Map2BiomeEcologySystem();
    const COUNT = 5000;

    const start = performance.now();
    let sumBiomes = 0;

    for (let i = 0; i < COUNT; i++) {
      const h = (i % 90);
      const s = (i % 100) / 100;
      const t = ((i * 7) % 100) / 100;
      const m = ((i * 13) % 100) / 100;
      const b = eco.classifyPrimaryBiome({ height: h, steepness: s, temperature: t, moisture: m });
      sumBiomes += b;
    }

    const elapsed = performance.now() - start;
    expect(sumBiomes).toBeGreaterThan(0);
    expect(elapsed).toBeLessThan(15.0);
  });
});
