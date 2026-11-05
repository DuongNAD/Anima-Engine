import * as THREE from 'three';

// -------------------------------------------------------------------------------------------------
// Map 2 Biome Taxonomy & Ecological Classification Architecture
// -------------------------------------------------------------------------------------------------

/**
 * 8 Canonical Biomes for Map 2 Procedural Open-World.
 */
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

export interface BiomeMetadata {
  id: Map2Biome;
  key: string;
  nameVi: string;
  nameEn: string;
  emoji: string;
  color: string;
  roughness: number;
  metalness: number;
  descriptionVi: string;
  descriptionEn: string;
  floraList: string[];
  faunaList: string[];
}

export const BIOME_METADATA: Record<Map2Biome, BiomeMetadata> = {
  [Map2Biome.SNOW_PEAKS]: {
    id: Map2Biome.SNOW_PEAKS,
    key: 'snow_peaks',
    nameVi: 'Đỉnh Tuyết Vĩnh Cửu',
    nameEn: 'Glacial Snow Peaks',
    emoji: '❄️',
    color: '#f8fbff',
    roughness: 0.9,
    metalness: 0.05,
    descriptionVi: 'Đỉnh núi phủ băng tuyết quanh năm, không khí lạnh buốt với gió rít dữ dội.',
    descriptionEn: 'Glacial ice sheets, perpetual frost, and jagged alpine peaks above the snowline.',
    floraList: [],
    faunaList: ['SnowLeopard', 'MountainGoat', 'AlpineEagle'],
  },
  [Map2Biome.ROCKY_CLIFFS]: {
    id: Map2Biome.ROCKY_CLIFFS,
    key: 'rocky_cliffs',
    nameVi: 'Vách Đá Dốc Đứng',
    nameEn: 'Alpine Rocky Cliffs',
    emoji: '🧗',
    color: '#4a4742',
    roughness: 0.95,
    metalness: 0.15,
    descriptionVi: 'Vách đá bazan dựng đứng, sườn dốc đá dăm và các tầng địa chất lộ thiên.',
    descriptionEn: 'Sheer basalt columns, steep scree talus slopes, and dramatic vertical crags.',
    floraList: ['CliffMoss', 'GnarledPine'],
    faunaList: ['MountainGoat', 'PeregrineFalcon'],
  },
  [Map2Biome.TEMPERATE_PLAINS]: {
    id: Map2Biome.TEMPERATE_PLAINS,
    key: 'temperate_plains',
    nameVi: 'Đồng Cỏ Ôn Đới',
    nameEn: 'Lush Temperate Plains',
    emoji: '🌿',
    color: '#5ea844',
    roughness: 0.75,
    metalness: 0.0,
    descriptionVi: 'Thảm cỏ xanh mướt uốn lượn hiền hòa, rực rỡ sắc hoa dại và cây bóng mát.',
    descriptionEn: 'Rolling emerald grasslands, wildflower meadows, and scattered birch groves.',
    floraList: ['OakTree', 'BirchTree', 'FieldGrass', 'Wildflower'],
    faunaList: ['Rabbit', 'Deer', 'Songbird', 'Fox'],
  },
  [Map2Biome.TROPICAL_RAINFOREST]: {
    id: Map2Biome.TROPICAL_RAINFOREST,
    key: 'tropical_rainforest',
    nameVi: 'Rừng Mưa Nhiệt Đới',
    nameEn: 'Dense Tropical Rainforest',
    emoji: '🌴',
    color: '#146328',
    roughness: 0.65,
    metalness: 0.0,
    descriptionVi: 'Tán rừng nhiệt đới rậm rạp, đất mùn ẩm ướt, dương xỉ khổng lồ và dây leo chằng chịt.',
    descriptionEn: 'Multi-layered jungle canopy, damp humus soil, giant ferns, and exotic orchids.',
    floraList: ['JunglePalm', 'FernTree', 'LushCanopyTree', 'ExoticFlower'],
    faunaList: ['Toucan', 'Jaguar', 'TreeFrog', 'Monkey'],
  },
  [Map2Biome.ARID_SAVANNA]: {
    id: Map2Biome.ARID_SAVANNA,
    key: 'arid_savanna',
    nameVi: 'Thảo Nguyên Khô Cằn',
    nameEn: 'Arid Savanna',
    emoji: '🌾',
    color: '#cbb25b',
    roughness: 0.85,
    metalness: 0.0,
    descriptionVi: 'Vùng đất nắng vàng với cỏ cháy, cây keo dù và cây bao báp cổ thụ.',
    descriptionEn: 'Sun-baked golden grasses, umbrella acacias, and ancient baobab trees.',
    floraList: ['AcaciaTree', 'BaobabTree', 'DryGrass'],
    faunaList: ['Lion', 'Zebra', 'Giraffe', 'Vulture'],
  },
  [Map2Biome.DESERT_DUNES]: {
    id: Map2Biome.DESERT_DUNES,
    key: 'desert_dunes',
    nameVi: 'Cồn Cát Sa Mạc',
    nameEn: 'Desert Sand Dunes',
    emoji: '🏜️',
    color: '#e8ba60',
    roughness: 0.9,
    metalness: 0.0,
    descriptionVi: 'Những đụn cát uốn lượn theo chiều gió với gợn sóng cát tinh xảo và xương rồng kiên cường.',
    descriptionEn: 'Wind-sculpted parabolic sand dunes, micro-ripples, and hardy saguaro cacti.',
    floraList: ['SaguaroCactus', 'BarrelCactus', 'DesertShrub'],
    faunaList: ['DesertFox', 'Camel', 'Rattlesnake', 'Scorpion'],
  },
  [Map2Biome.HYDROLOGY_RIVER_LAKE]: {
    id: Map2Biome.HYDROLOGY_RIVER_LAKE,
    key: 'hydrology_river_lake',
    nameVi: 'Lưu Vực Sông Hồ',
    nameEn: 'River & Lake Hydrology',
    emoji: '💧',
    color: '#3a84a7',
    roughness: 0.1,
    metalness: 0.2,
    descriptionVi: 'Làn nước trong vắt phản chiếu bầu trời, dòng sông uốn lượn và bờ cát mịn.',
    descriptionEn: 'Crystal clear freshwater river networks, waterfalls, deltas, and deep cobalt tarns.',
    floraList: [],
    faunaList: ['RiverTrout', 'Salmon', 'Otter', 'Heron'],
  },
  [Map2Biome.BIOLUMINESCENT_CAVES]: {
    id: Map2Biome.BIOLUMINESCENT_CAVES,
    key: 'bioluminescent_caves',
    nameVi: 'Hang Pha Lê Phát Quang',
    nameEn: 'Bioluminescent Crystal Caves',
    emoji: '🔮',
    color: '#9d4edd',
    roughness: 0.4,
    metalness: 0.6,
    descriptionVi: 'Hang ngầm huyền ảo với các cột thạch anh thạch nhũ phát quang sắc tím thạch anh và lục lam.',
    descriptionEn: 'Subterranean grottoes with pulsating amethyst and cyan crystal clusters and glowing spores.',
    floraList: ['GlowingMushroom', 'CrystalSpire', 'BioluminescentMoss'],
    faunaList: ['GlowBat', 'CaveSalamander', 'CrystalBeetle'],
  },
};

export const MAP2_BIOME_NAMES_VI: Record<Map2Biome, string> = {
  [Map2Biome.SNOW_PEAKS]: 'Đỉnh Tuyết Vĩnh Cửu',
  [Map2Biome.ROCKY_CLIFFS]: 'Vách Đá Dốc Đứng',
  [Map2Biome.TEMPERATE_PLAINS]: 'Đồng Cỏ Ôn Đới',
  [Map2Biome.TROPICAL_RAINFOREST]: 'Rừng Mưa Nhiệt Đới',
  [Map2Biome.ARID_SAVANNA]: 'Thảo Nguyên Khô Cằn',
  [Map2Biome.DESERT_DUNES]: 'Cồn Cát Sa Mạc',
  [Map2Biome.HYDROLOGY_RIVER_LAKE]: 'Lưu Vực Sông Hồ',
  [Map2Biome.BIOLUMINESCENT_CAVES]: 'Hang Pha Lê Phát Quang',
};

/**
 * Authoritative Biome Ecology and Ecological Rule Verification System for Map 2.
 */
export class Map2BiomeEcologySystem {
  public readonly waterLevel = 12.0;
  public readonly snowLine = 48.0;
  public readonly cliffThreshold = 0.55;

  /**
   * Classifies primary biome using elevation, slope, temperature, moisture, and cave mask.
   */
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
    if (height >= this.snowLine || (height > 40.0 && temperature < 0.25)) {
      return Map2Biome.SNOW_PEAKS;
    }

    // 4. Rocky Cliffs
    if (steepness >= this.cliffThreshold) {
      return Map2Biome.ROCKY_CLIFFS;
    }

    // 5. Climatic Biome Matrix (Temperature vs Moisture)
    if (temperature > 0.52) {
      if (moisture < 0.35) return Map2Biome.DESERT_DUNES;
      if (moisture < 0.58) return Map2Biome.ARID_SAVANNA;
      return Map2Biome.TROPICAL_RAINFOREST;
    } else {
      if (moisture > 0.55) return Map2Biome.TROPICAL_RAINFOREST;
      return Map2Biome.TEMPERATE_PLAINS;
    }
  }

  /**
   * Computes continuous soft-max ecotone weights summing strictly to 1.0 (Partition of Unity).
   */
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
    const snowStart = Math.min(55.0, this.snowLine - 10.0);
    if (height > snowStart && height < this.snowLine && primary !== Map2Biome.SNOW_PEAKS) {
      const snowFactor = (height - snowStart) / (this.snowLine - snowStart);
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

  /**
   * Ecological placement constraints for vegetation.
   */
  public canPlaceFlora(biome: Map2Biome, height: number, steepness: number): boolean {
    // Zero flora in water
    if (height <= this.waterLevel) return false;
    // Zero flora on high snow peaks
    if (height >= this.snowLine || biome === Map2Biome.SNOW_PEAKS) return false;
    // Zero flora on vertical rock cliffs
    if (steepness >= this.cliffThreshold) return false;
    return true;
  }

  /**
   * Flora taxonomy by biome.
   */
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

  /**
   * Validates fauna habitat medium integrity.
   */
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

export const defaultBiomeSystem = new Map2BiomeEcologySystem();

export function getBiomeMetadata(biome: Map2Biome): BiomeMetadata {
  return BIOME_METADATA[biome] || BIOME_METADATA[Map2Biome.TEMPERATE_PLAINS];
}
