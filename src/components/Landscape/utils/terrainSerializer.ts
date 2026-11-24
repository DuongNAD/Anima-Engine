import { TerrainData, TerrainCell, FloraPlacement, BiomeType } from './terrainGenerator';

export const TERRAIN_SCHEMA_VERSION = 1;

export const BIOME_LIST: BiomeType[] = [
  'ocean',
  'beach',
  'grassland',
  'forest',
  'taiga',
  'alpine rock',
  'snow peaks',
  'desert',
  'jungle',
  'volcanic',
  'glacier',
];

export const FLORA_TYPE_LIST: FloraPlacement['type'][] = [
  'Oak',
  'Pine',
  'Bush',
  'Rock',
  'Palm',
  'Cactus',
  'Jungle',
  'Birch',
  'Flowers',
  'Dead Trunk',
  'Snow Pine',
  'Ice Rock',
];

const BIOME_INDEX_MAP: Record<string, number> = {};
BIOME_LIST.forEach((b, idx) => {
  BIOME_INDEX_MAP[b] = idx;
});

const FLORA_TYPE_INDEX_MAP: Record<string, number> = {};
FLORA_TYPE_LIST.forEach((f, idx) => {
  FLORA_TYPE_INDEX_MAP[f] = idx;
});

export interface SerializedTerrainChunk {
  version: number;
  seed: string | number;
  chunkX: number;
  chunkY: number;
  width: number;
  height: number;
  lastAccessed: number;

  elevation: ArrayBuffer;
  moisture: ArrayBuffer;
  temperature: ArrayBuffer;
  biomes: ArrayBuffer;
  hydroFlags: ArrayBuffer;
  waterY?: ArrayBuffer;

  flora: {
    types: ArrayBuffer;
    transforms: ArrayBuffer;
  };
}

/**
 * Fast binary serialization for terrain data into ArrayBuffer objects.
 */
export function serializeChunk(
  terrain: TerrainData,
  chunkX = 0,
  chunkY = 0
): SerializedTerrainChunk {
  const width = terrain.width;
  const height = terrain.height;
  const size = width * height;

  const elevationArr = new Float32Array(size);
  const moistureArr = new Uint8Array(size);
  const tempArr = new Float32Array(size);
  const biomesArr = new Uint8Array(size);
  const hydroFlagsArr = new Uint8Array(size);
  const waterYArr = new Float32Array(size);

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = y * width + x;
      const cell = terrain.grid[y][x];

      elevationArr[i] = cell.elevation;
      moistureArr[i] = Math.max(0, Math.min(255, Math.round(cell.moisture)));
      tempArr[i] = cell.temperature;
      biomesArr[i] = BIOME_INDEX_MAP[cell.biome] ?? 2;

      let flag = 0;
      const isRiverVal = cell.isRiver;
      const isRiverBool = typeof isRiverVal === 'number' ? isRiverVal > 0 : !!isRiverVal;
      if (isRiverBool) flag |= 0x01; // Bit 0: isRiver
      if (cell.isWaterfall) flag |= 0x02; // Bit 1: isWaterfall
      if (cell.isLake) flag |= 0x04; // Bit 2: isLake

      let riverType = 0;
      if (typeof isRiverVal === 'number' && isRiverVal >= 1 && isRiverVal <= 3) {
        riverType = isRiverVal;
      } else if (isRiverBool) {
        riverType = cell.isWaterfall ? 2 : 1;
      }
      flag |= (riverType & 0x03) << 3; // Bits 3-4: river type (1=normal, 2=waterfall, 3=pond)

      hydroFlagsArr[i] = flag;
      waterYArr[i] = cell.waterY !== undefined ? cell.waterY : -999.0;
    }
  }

  const floraCount = terrain.flora.length;
  const floraTypesArr = new Uint8Array(floraCount);
  const floraTransformsArr = new Float32Array(floraCount * 3);

  for (let i = 0; i < floraCount; i++) {
    const f = terrain.flora[i];
    floraTypesArr[i] = FLORA_TYPE_INDEX_MAP[f.type] ?? 0;
    floraTransformsArr[i * 3] = f.x;
    floraTransformsArr[i * 3 + 1] = f.y;
    floraTransformsArr[i * 3 + 2] = f.scale;
  }

  return {
    version: TERRAIN_SCHEMA_VERSION,
    seed: terrain.seed,
    chunkX,
    chunkY,
    width,
    height,
    lastAccessed: Date.now(),
    elevation: elevationArr.buffer.slice(elevationArr.byteOffset, elevationArr.byteOffset + elevationArr.byteLength),
    moisture: moistureArr.buffer.slice(moistureArr.byteOffset, moistureArr.byteOffset + moistureArr.byteLength),
    temperature: tempArr.buffer.slice(tempArr.byteOffset, tempArr.byteOffset + tempArr.byteLength),
    biomes: biomesArr.buffer.slice(biomesArr.byteOffset, biomesArr.byteOffset + biomesArr.byteLength),
    hydroFlags: hydroFlagsArr.buffer.slice(hydroFlagsArr.byteOffset, hydroFlagsArr.byteOffset + hydroFlagsArr.byteLength),
    waterY: waterYArr.buffer.slice(waterYArr.byteOffset, waterYArr.byteOffset + waterYArr.byteLength),
    flora: {
      types: floraTypesArr.buffer.slice(floraTypesArr.byteOffset, floraTypesArr.byteOffset + floraTypesArr.byteLength),
      transforms: floraTransformsArr.buffer.slice(floraTransformsArr.byteOffset, floraTransformsArr.byteOffset + floraTransformsArr.byteLength),
    },
  };
}

/**
 * Reconstructs terrain data from deserialized TypedArrays.
 */
export function deserializeChunk(serialized: SerializedTerrainChunk): TerrainData {
  const width = serialized.width;
  const height = serialized.height;

  const elevationArr = new Float32Array(serialized.elevation);
  const moistureArr = new Uint8Array(serialized.moisture);
  const tempArr = new Float32Array(serialized.temperature);
  const biomesArr = new Uint8Array(serialized.biomes);
  const hydroFlagsArr = new Uint8Array(serialized.hydroFlags);
  const waterYArr = serialized.waterY ? new Float32Array(serialized.waterY) : null;

  const grid: TerrainCell[][] = [];
  for (let y = 0; y < height; y++) {
    const row: TerrainCell[] = [];
    for (let x = 0; x < width; x++) {
      const i = y * width + x;
      const flag = hydroFlagsArr[i];

      const isRiverBit = (flag & 0x01) !== 0;
      const isWaterfall = (flag & 0x02) !== 0;
      const isLake = (flag & 0x04) !== 0;
      const riverType = (flag >> 3) & 0x03;

      let isRiver: boolean | number = false;
      if (riverType > 0) {
        isRiver = riverType;
      } else if (isRiverBit) {
        isRiver = true;
      }

      const wY = waterYArr ? waterYArr[i] : -999.0;

      row.push({
        x,
        y,
        elevation: elevationArr[i],
        moisture: moistureArr[i],
        temperature: tempArr[i],
        biome: BIOME_LIST[biomesArr[i]] ?? 'grassland',
        isRiver,
        isWaterfall,
        isLake,
        waterY: wY !== -999.0 ? wY : undefined,
      });
    }
    grid.push(row);
  }

  const floraTypesArr = new Uint8Array(serialized.flora.types);
  const floraTransformsArr = new Float32Array(serialized.flora.transforms);
  const floraCount = floraTypesArr.length;
  const flora: FloraPlacement[] = [];

  for (let i = 0; i < floraCount; i++) {
    flora.push({
      x: floraTransformsArr[i * 3],
      y: floraTransformsArr[i * 3 + 1],
      type: FLORA_TYPE_LIST[floraTypesArr[i]] ?? 'Oak',
      scale: floraTransformsArr[i * 3 + 2],
    });
  }

  return {
    width,
    height,
    seed: serialized.seed,
    grid,
    flora,
    cached: true,
  };
}
