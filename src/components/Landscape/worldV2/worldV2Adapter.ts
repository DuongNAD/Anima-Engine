import { FloraType, WORLD_GEN_VERSION, type LakeBasin, type World } from '../utils/worldGen';
import {
  ELEV_MIN_M,
  ELEV_SPAN_M,
  FLORA_SECTORS_PER_SIDE,
  SEA_LEVEL_NORM,
  WaterKind,
  type WorldV2LoadedBundle,
} from './types';

export const WV2_SPECIES_TO_FLORA: ReadonlyArray<FloraType> = [
  FloraType.Pine, // 0
  FloraType.Round, // 1
  FloraType.Jungle, // 2
  FloraType.Cactus, // 3
  FloraType.Rock, // 4
  FloraType.Acacia, // 5
  FloraType.Palm, // 6
  FloraType.Bush, // 7
];

export interface WorldV2RenderMeta {
  schemaTag: string;
  simIdentity8: string;
  bundleIdentity8: string;
  mip: number;
  size: number;
  tileCountVerified: number;
  floraCount: number;
  caveCount: number;
}

function clamp01(v: number): number {
  return v < 0 ? 0 : v > 1 ? 1 : v;
}

function bilinearSample(src: Float32Array, srcSize: number, u: number, v: number): number {
  const gx = clamp01(u) * (srcSize - 1);
  const gy = clamp01(v) * (srcSize - 1);
  const x0 = Math.floor(gx);
  const y0 = Math.floor(gy);
  const x1 = Math.min(srcSize - 1, x0 + 1);
  const y1 = Math.min(srcSize - 1, y0 + 1);
  const fx = gx - x0;
  const fy = gy - y0;
  const a = src[y0 * srcSize + x0];
  const b = src[y0 * srcSize + x1];
  const c = src[y1 * srcSize + x0];
  const d = src[y1 * srcSize + x1];
  return (a * (1 - fx) + b * fx) * (1 - fy) + (c * (1 - fx) + d * fx) * fy;
}

function extractLakeBasinsFromMask(
  waterNorm: Float32Array,
  size: number,
): LakeBasin[] {
  // Flood-fill on at most a 256^2 proxy grid to keep lake-plane extraction O(256^2).
  const step = Math.max(1, Math.floor(size / 256));
  const gs = Math.floor(size / step);
  const visited = new Uint8Array(gs * gs);
  const basins: LakeBasin[] = [];
  const stack: number[] = [];

  for (let sy = 0; sy < gs; sy += 1) {
    for (let sx = 0; sx < gs; sx += 1) {
      const si = sy * gs + sx;
      if (visited[si]) continue;
      const lvl = waterNorm[sy * step * size + sx * step];
      if (lvl <= 0) continue;

      visited[si] = 1;
      stack.push(si);
      let minX = sx;
      let maxX = sx;
      let minY = sy;
      let maxY = sy;
      let sumLvl = 0;
      let cells = 0;

      while (stack.length > 0) {
        const cur = stack.pop()!;
        const cx = cur % gs;
        const cy = (cur - cx) / gs;
        const clvl = waterNorm[cy * step * size + cx * step];
        sumLvl += clvl;
        cells += 1;
        if (cx < minX) minX = cx;
        if (cx > maxX) maxX = cx;
        if (cy < minY) minY = cy;
        if (cy > maxY) maxY = cy;

        const neighbors = [
          cx > 0 ? cur - 1 : -1,
          cx + 1 < gs ? cur + 1 : -1,
          cy > 0 ? cur - gs : -1,
          cy + 1 < gs ? cur + gs : -1,
        ];
        for (const nb of neighbors) {
          if (nb < 0 || visited[nb]) continue;
          const nbx = nb % gs;
          const nby = (nb - nbx) / gs;
          if (waterNorm[nby * step * size + nbx * step] > 0) {
            visited[nb] = 1;
            stack.push(nb);
          }
        }
      }

      if (cells >= 1) {
        basins.push({
          level: sumLvl / cells,
          minX: minX * step,
          maxX: Math.min(size - 1, (maxX + 1) * step),
          minY: minY * step,
          maxY: Math.min(size - 1, (maxY + 1) * step),
        });
      }
    }
  }
  return basins;
}

/**
 * Adapts a SHA-256 verified `WorldV2LoadedBundle` into the `World` SoA interface used by
 * camera rig, spawn finder, minimap, and Three.js overlays — without calling v1 `generateWorld`.
 */
export function buildWorldFromV2Bundle(bundle: WorldV2LoadedBundle): World {
  const { size, extentM, sim } = bundle;
  const n = size * size;

  const elevation = new Float32Array(n);
  const moisture = new Float32Array(n);
  const temperature = new Float32Array(n);
  const flow = new Float32Array(n);
  const slope = new Float32Array(n);
  const water = new Float32Array(n);
  const riverAmt = new Uint8Array(n);
  const shore = new Float32Array(n);
  const biome = new Uint8Array(n);

  const invSizeMinus1 = size > 1 ? 1 / (size - 1) : 1;

  for (let y = 0; y < size; y += 1) {
    const v = y * invSizeMinus1;
    for (let x = 0; x < size; x += 1) {
      const i = y * size + x;
      const u = x * invSizeMinus1;

      elevation[i] = clamp01((bundle.heightM[i] - ELEV_MIN_M) / ELEV_SPAN_M);
      biome[i] = bundle.biomeIdsRgba[i * 4];

      moisture[i] = clamp01(bilinearSample(sim.moisture, sim.width, u, v));
      temperature[i] = clamp01(bilinearSample(sim.temperature, sim.width, u, v));
      slope[i] = clamp01(bilinearSample(sim.slope, sim.width, u, v));

      const o4 = i * 4;
      const flowNorm = bundle.hydroFlowRgba[o4 + 2] / 255;
      const rivSpray = bundle.hydroFlowRgba[o4 + 3];
      flow[i] = flowNorm;
      const wk = bundle.waterKind[i];
      riverAmt[i] =
        wk === WaterKind.River || wk === WaterKind.DeltaTurbid || wk === WaterKind.Waterfall
          ? Math.max(128, rivSpray)
          : rivSpray;

      // shore_sdf channel R packs [(sdf_m + 500) / 1000].
      const sdfM = (bundle.shoreSdfRg[i * 2] / 255) * 1000.0 - 500.0;
      shore[i] = sdfM >= 0 ? clamp01(1.0 - sdfM / 180.0) : 0;

      if (wk === WaterKind.FreshwaterLake || wk === WaterKind.EndorheicLake) {
        water[i] = clamp01((bundle.waterHeightM[i] - ELEV_MIN_M) / ELEV_SPAN_M);
      }
    }
  }

  const lakeBasins = extractLakeBasinsFromMask(water, size);

  let totalFlora = 0;
  for (const s of bundle.floraSectors) totalFlora += s.instances.length;
  const floraX = new Float32Array(totalFlora);
  const floraZ = new Float32Array(totalFlora);
  const floraScale = new Float32Array(totalFlora);
  const floraType = new Uint8Array(totalFlora);
  let fi = 0;
  for (const sec of bundle.floraSectors) {
    for (const inst of sec.instances) {
      const u = (sec.sectorX + inst.x_local_u16 / 65535.0) / FLORA_SECTORS_PER_SIDE;
      const v = (sec.sectorZ + inst.z_local_u16 / 65535.0) / FLORA_SECTORS_PER_SIDE;
      floraX[fi] = (u - 0.5) * size;
      floraZ[fi] = (v - 0.5) * size;
      floraScale[fi] = 0.35 + (inst.scale_u8 / 255.0) * 1.45;
      floraType[fi] = WV2_SPECIES_TO_FLORA[inst.species_id] ?? FloraType.Bush;
      fi += 1;
    }
  }

  const waterfallCount = sim.waterfalls.length;
  const waterfallX = new Float32Array(waterfallCount);
  const waterfallZ = new Float32Array(waterfallCount);
  const waterfallTopE = new Float32Array(waterfallCount);
  const waterfallDrop = new Float32Array(waterfallCount);
  const waterfallYaw = new Float32Array(waterfallCount);
  for (let i = 0; i < waterfallCount; i += 1) {
    const wf = sim.waterfalls[i];
    waterfallX[i] = (wf.x_m / extentM - 0.5) * size;
    waterfallZ[i] = (wf.z_m / extentM - 0.5) * size;
    waterfallTopE[i] = clamp01((wf.lip_elev_m - ELEV_MIN_M) / ELEV_SPAN_M);
    waterfallDrop[i] = Math.max(0.002, wf.drop_m / ELEV_SPAN_M);
    waterfallYaw[i] = wf.yaw_rad;
  }

  const caveCount = sim.caveEntrances.length;
  const caveX = new Float32Array(caveCount);
  const caveZ = new Float32Array(caveCount);
  const caveE = new Float32Array(caveCount);
  const caveYaw = new Float32Array(caveCount);
  for (let i = 0; i < caveCount; i += 1) {
    const cv = sim.caveEntrances[i];
    caveX[i] = (cv.x_m / extentM - 0.5) * size;
    caveZ[i] = (cv.z_m / extentM - 0.5) * size;
    caveE[i] = clamp01((cv.elevation_m - ELEV_MIN_M) / ELEV_SPAN_M);
    caveYaw[i] = cv.yaw_rad;
  }

  return {
    version: WORLD_GEN_VERSION,
    seed: bundle.seed,
    size,
    elevation,
    moisture,
    temperature,
    flow,
    slope,
    water,
    riverAmt,
    shore,
    lakeBasins,
    biome,
    floraX,
    floraZ,
    floraScale,
    floraType,
    floraCount: totalFlora,
    waterfallX,
    waterfallZ,
    waterfallTopE,
    waterfallDrop,
    waterfallYaw,
    waterfallCount,
    caveX,
    caveZ,
    caveE,
    caveYaw,
    caveCount,
    seaLevel: SEA_LEVEL_NORM,
  };
}

export function worldV2RenderMeta(bundle: WorldV2LoadedBundle): WorldV2RenderMeta {
  let floraCount = 0;
  for (const s of bundle.floraSectors) floraCount += s.instances.length;
  return {
    schemaTag: bundle.schemaTag,
    simIdentity8: bundle.simWorldIdentity.slice(0, 8),
    bundleIdentity8: bundle.bundleIdentity.slice(0, 8),
    mip: bundle.mip,
    size: bundle.size,
    tileCountVerified: bundle.tileCountVerified,
    floraCount,
    caveCount: bundle.sim.caveEntrances.length,
  };
}
