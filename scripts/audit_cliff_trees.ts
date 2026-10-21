import * as THREE from 'three';
import { sampleMap2Terrain } from '../src/components/Map2/utils/map2TerrainGen';
import { Map2Biome } from '../src/components/Map2/utils/map2BiomeConfig';
import {
  Map2FloraSpecies,
  FLORA_CANOPY_RADII,
  isTallFlora,
} from '../src/components/Map2/utils/map2FloraGeometry';

function hash2D(x: number, z: number, seed = 0): number {
  let h = Math.imul((x * 374761393) | 0, (z * 668265263) | 0) + Math.imul(seed, 1274126177);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

const ALL_SPECIES: Map2FloraSpecies[] = [
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

interface FloraPlacement {
  species: Map2FloraSpecies;
  x: number;
  y: number;
  z: number;
  scale: number;
  rotationY: number;
  tiltX: number;
  tiltZ: number;
  colorTint: THREE.Color;
  isSpecimen: boolean;
  isTall: boolean;
}

function generateTestPlacements(
  size = 200,
  waterLevel = 12.0,
  spawnClearanceRadius = 6.0,
  quality: 'high' | 'low' = 'high',
) {
  const bySpecies = new Map<Map2FloraSpecies, FloraPlacement[]>();
  ALL_SPECIES.forEach((sp) => bySpecies.set(sp, []));
  const placements: FloraPlacement[] = [];
  const tallTrees: FloraPlacement[] = [];
  const specimenTrees: FloraPlacement[] = [];

  const halfSize = size * 0.48; // [-96m, 96m]
  const gridRes = quality === 'high' ? 60 : 36;
  const step = (halfSize * 2) / gridRes;

  const dummyTint = new THREE.Color();

  for (let gx = 0; gx < gridRes; gx++) {
    for (let gz = 0; gz < gridRes; gz++) {
      const jx = (hash2D(gx, gz, 1) - 0.5) * step * 0.88;
      const jz = (hash2D(gx, gz, 2) - 0.5) * step * 0.88;
      const wx = -halfSize + (gx + 0.5) * step + jx;
      const wz = -halfSize + (gz + 0.5) * step + jz;

      const distToSpawn = Math.sqrt(wx * wx + wz * wz);
      if (distToSpawn < 24.0) {
        if (distToSpawn > 6.0 && hash2D(gx, gz, 3) < 0.22) {
          const sample = sampleMap2Terrain(wx, wz);
          if (!sample.isWater && sample.height > waterLevel) {
            const sp = hash2D(gx, gz, 4) < 0.5 ? Map2FloraSpecies.Wildflower : Map2FloraSpecies.MeadowGrassTuft;
            const p: FloraPlacement = {
              species: sp,
              x: wx,
              y: sample.height,
              z: wz,
              scale: 0.8 + hash2D(gx, gz, 5) * 0.4,
              rotationY: hash2D(gx, gz, 6) * Math.PI * 2,
              tiltX: sample.normal.z * 0.2,
              tiltZ: -sample.normal.x * 0.2,
              colorTint: dummyTint.clone(),
              isSpecimen: false,
              isTall: false,
            };
            bySpecies.get(sp)?.push(p);
            placements.push(p);
          }
        }
        continue;
      }

      const sample = sampleMap2Terrain(wx, wz);
      const { height, steepness, isWater, biomeId, normal, moisture } = sample;

      if (isWater || height <= waterLevel) continue;
      if (height > 52.0) continue;
      if (steepness > 0.42) continue;

      const n1 = Math.sin(wx * 0.025 + 1.2) * Math.cos(wz * 0.025 + 0.8);
      const n2 = Math.sin(wx * 0.055 - 0.7) * Math.cos(wz * 0.052 + 1.3) * 0.5;
      const n3 = Math.sin(wx * 0.012 + wz * 0.014 + 2.0) * 0.35;
      const rawGladeNoise = (n1 + n2 + n3 + 1.85) / 3.7;

      const moistureFactor = 0.65 + 0.70 * moisture;
      const slopeFactor = Math.max(0.0, 1.0 - 2.2 * steepness);
      const elevationFactor = height <= 34.0 ? 1.0 : Math.max(0.0, 1.0 - (height - 34.0) / 16.0);

      const groveIntensity = rawGladeNoise * moistureFactor * slopeFactor * elevationFactor;
      const isTreeGrove = groveIntensity > 0.52;

      const rSelect = hash2D(gx, gz, 7);
      const rScale = hash2D(gx, gz, 8);

      let chosenSpecies: Map2FloraSpecies | null = null;
      let baseScale = 1.0;
      let isSpecimen = false;

      switch (biomeId) {
        case Map2Biome.TROPICAL_RAINFOREST: {
          if (isTreeGrove) {
            if (rSelect < 0.16) {
              chosenSpecies = Map2FloraSpecies.TropicalPalm;
              baseScale = 2.3 + rScale * 0.8;
            } else if (rSelect < 0.32) {
              chosenSpecies = Map2FloraSpecies.LushJungleTree;
              baseScale = 2.5 + rScale * 0.9;
            } else if (rSelect < 0.68) {
              chosenSpecies = Map2FloraSpecies.MeadowGrassTuft;
              baseScale = 1.2 + rScale * 0.5;
            } else if (rSelect < 0.88) {
              chosenSpecies = Map2FloraSpecies.Wildflower;
              baseScale = 1.0 + rScale * 0.4;
            }
          } else {
            if (rSelect < 0.020) {
              chosenSpecies = Map2FloraSpecies.TropicalPalm;
              baseScale = 3.6 + rScale * 0.8;
              isSpecimen = true;
            } else if (rSelect < 0.38) {
              chosenSpecies = Map2FloraSpecies.MeadowGrassTuft;
              baseScale = 1.1 + rScale * 0.4;
            } else if (rSelect < 0.58) {
              chosenSpecies = Map2FloraSpecies.Wildflower;
              baseScale = 1.0 + rScale * 0.4;
            }
          }
          break;
        }

        case Map2Biome.ARID_SAVANNA: {
          if (isTreeGrove) {
            if (rSelect < 0.30) {
              chosenSpecies = Map2FloraSpecies.UmbrellaAcacia;
              baseScale = 2.2 + rScale * 0.8;
            } else if (rSelect < 0.62) {
              chosenSpecies = Map2FloraSpecies.DesertScrub;
              baseScale = 1.2 + rScale * 0.5;
            } else if (rSelect < 0.85) {
              chosenSpecies = Map2FloraSpecies.MeadowGrassTuft;
              baseScale = 1.0 + rScale * 0.4;
            }
          } else {
            if (rSelect < 0.020) {
              chosenSpecies = Map2FloraSpecies.UmbrellaAcacia;
              baseScale = 3.6 + rScale * 0.9;
              isSpecimen = true;
            } else if (rSelect < 0.28) {
              chosenSpecies = Map2FloraSpecies.MeadowGrassTuft;
              baseScale = 0.9 + rScale * 0.4;
            } else if (rSelect < 0.46) {
              chosenSpecies = Map2FloraSpecies.DesertScrub;
              baseScale = 1.0 + rScale * 0.4;
            }
          }
          break;
        }

        case Map2Biome.DESERT_DUNES: {
          if (rSelect < 0.16) {
            chosenSpecies = Map2FloraSpecies.SaguaroCactus;
            baseScale = 1.8 + rScale * 0.8;
          } else if (rSelect < 0.38) {
            chosenSpecies = Map2FloraSpecies.DesertScrub;
            baseScale = 1.0 + rScale * 0.4;
          }
          break;
        }

        case Map2Biome.TEMPERATE_PLAINS: {
          if (isTreeGrove) {
            if (rSelect < 0.18) {
              chosenSpecies = Map2FloraSpecies.BroadleafDeciduous;
              baseScale = 2.2 + rScale * 0.8;
            } else if (rSelect < 0.31) {
              chosenSpecies = Map2FloraSpecies.ConiferPine;
              baseScale = 2.0 + rScale * 0.7;
            } else if (rSelect < 0.68) {
              chosenSpecies = Map2FloraSpecies.MeadowGrassTuft;
              baseScale = 1.1 + rScale * 0.4;
            } else if (rSelect < 0.88) {
              chosenSpecies = Map2FloraSpecies.Wildflower;
              baseScale = 0.9 + rScale * 0.4;
            }
          } else {
            if (rSelect < 0.020) {
              chosenSpecies = Map2FloraSpecies.BroadleafDeciduous;
              baseScale = 3.6 + rScale * 0.9;
              isSpecimen = true;
            } else if (rSelect < 0.32) {
              chosenSpecies = Map2FloraSpecies.MeadowGrassTuft;
              baseScale = 1.0 + rScale * 0.4;
            } else if (rSelect < 0.52) {
              chosenSpecies = Map2FloraSpecies.Wildflower;
              baseScale = 0.9 + rScale * 0.4;
            }
          }
          break;
        }

        case Map2Biome.SNOW_PEAKS: {
          if (height <= 46.0 && steepness <= 0.38) {
            if (rSelect < 0.18) {
              chosenSpecies = Map2FloraSpecies.ConiferPine;
              baseScale = 1.5 + rScale * 0.6;
            } else if (rSelect < 0.45) {
              chosenSpecies = Map2FloraSpecies.CliffMoss;
              baseScale = 1.1 + rScale * 0.5;
            }
          }
          break;
        }

        case Map2Biome.ROCKY_CLIFFS: {
          if (steepness <= 0.38) {
            if (rSelect < 0.42) {
              chosenSpecies = Map2FloraSpecies.CliffMoss;
              baseScale = 1.1 + rScale * 0.5;
            } else if (rSelect < 0.58) {
              chosenSpecies = Map2FloraSpecies.ConiferPine;
              baseScale = 1.4 + rScale * 0.5;
            }
          }
          break;
        }

        case Map2Biome.BIOLUMINESCENT_CAVES: {
          if (rSelect < 0.35) {
            chosenSpecies = Map2FloraSpecies.GlowingMushroom;
            baseScale = 1.2 + rScale * 0.6;
          } else if (rSelect < 0.55) {
            chosenSpecies = Map2FloraSpecies.CrystalCluster;
            baseScale = 1.3 + rScale * 0.7;
          }
          break;
        }

        default:
          break;
      }

      if (!chosenSpecies) continue;

      const canopyR = FLORA_CANOPY_RADII[chosenSpecies] * baseScale;
      if (distToSpawn < spawnClearanceRadius + canopyR) continue;

      const isTall = isTallFlora(chosenSpecies);
      const rotationY = hash2D(gx, gz, 7) * Math.PI * 2;
      const tiltX = isTall ? 0 : normal.z * 0.2;
      const tiltZ = isTall ? 0 : -normal.x * 0.2;

      const p: FloraPlacement = {
        species: chosenSpecies,
        x: wx,
        y: height,
        z: wz,
        scale: baseScale,
        rotationY,
        tiltX,
        tiltZ,
        colorTint: dummyTint.clone(),
        isSpecimen,
        isTall,
      };

      bySpecies.get(chosenSpecies)?.push(p);
      placements.push(p);
      if (isTall) {
        tallTrees.push(p);
        if (isSpecimen) {
          specimenTrees.push(p);
        }
      }
    }
  }

  return { placements, bySpecies, tallTrees, specimenTrees };
}

const gen = generateTestPlacements(200, 12.0, 6.0, 'high');

console.log('=== TALL TREES ON SHEER CLIFFS (s >= 0.38) ===');
for (const t of gen.tallTrees) {
  const s = sampleMap2Terrain(t.x, t.z);
  if (s.steepness >= 0.38) {
    console.log(`- ${t.species} at (${t.x.toFixed(2)}, ${t.y.toFixed(2)}, ${t.z.toFixed(2)}): steepness=${s.steepness.toFixed(4)}, biome=${s.biomeId}`);
  }
}

console.log('\n=== TALL TREES NEAR WATER (h <= 12.0) ===');
let nearWaterCount = 0;
for (const t of gen.tallTrees) {
  const s = sampleMap2Terrain(t.x, t.z);
  if (t.y <= 12.0 || s.isWater) {
    nearWaterCount++;
    console.log(`- ${t.species} at (${t.x.toFixed(2)}, ${t.y.toFixed(2)}, ${t.z.toFixed(2)}): height=${t.y.toFixed(2)}`);
  }
}
if (nearWaterCount === 0) console.log('None! Zero tall trees in water.');

console.log('\n=== TALL TREES ON SNOW PEAKS (h >= 46.0) ===');
let snowPeakCount = 0;
for (const t of gen.tallTrees) {
  if (t.y >= 46.0) {
    snowPeakCount++;
    console.log(`- ${t.species} at (${t.x.toFixed(2)}, ${t.y.toFixed(2)}, ${t.z.toFixed(2)}): height=${t.y.toFixed(2)}`);
  }
}
if (snowPeakCount === 0) console.log('None! Zero tall trees on snow peaks >= 46.0m.');
