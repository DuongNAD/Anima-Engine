import * as THREE from 'three';
import { sampleMap2Terrain } from './map2TerrainGen';
import { Map2Biome } from './map2BiomeConfig';
import { Map2FloraSpecies, isTallFlora, FLORA_CANOPY_RADII } from './map2FloraGeometry';

// -------------------------------------------------------------------------------------------------
// Map 2 flora placement: the deterministic, React-free half of `Map2Vegetation`. Split out so the
// component module exports only components (Fast Refresh), and so scripts can import the placement
// engine without pulling in React.
// -------------------------------------------------------------------------------------------------

export interface FloraPlacement {
  species: Map2FloraSpecies;
  x: number;
  y: number;
  z: number;
  scale: number;
  rotationY: number;
  tiltX: number;
  tiltZ: number;
  colorTint: THREE.Color;
  isSpecimen?: boolean;
  isTall?: boolean;
}

/**
 * Fast deterministic spatial hash returning a float in [0, 1).
 */
export function hash2D(x: number, z: number, seed = 0): number {
  let h = Math.imul((x * 374761393) | 0, (z * 668265263) | 0) + Math.imul(seed, 1274126177);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

export const ALL_SPECIES: Map2FloraSpecies[] = [
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

/**
 * Procedural generation engine adhering to natural ecological carrying capacity,
 * open glade dominance (>= 70%), solitary specimen isolation (>= 14.0m),
 * and clean ecological slope/elevation constraints.
 */
export function generatePlacements(
  size = 200,
  waterLevel = 12.0,
  spawnClearanceRadius = 6.0,
  quality: 'high' | 'low' = 'high',
): {
  placements: FloraPlacement[];
  bySpecies: Map<Map2FloraSpecies, FloraPlacement[]>;
  tallTrees: FloraPlacement[];
  specimenTrees: FloraPlacement[];
  groundCover: FloraPlacement[];
} {
  const bySpecies = new Map<Map2FloraSpecies, FloraPlacement[]>();
  ALL_SPECIES.forEach((sp) => bySpecies.set(sp, []));
  const placements: FloraPlacement[] = [];
  const tallTrees: FloraPlacement[] = [];
  const specimenTrees: FloraPlacement[] = [];
  const groundCover: FloraPlacement[] = [];

  const tallTreePositions: Array<{ x: number; z: number }> = [];
  const specimenTreePositions: Array<{ x: number; z: number }> = [];

  const halfSize = size * 0.48; // [-96m, 96m]
  const gridRes = quality === 'high' ? 60 : 36;
  const step = (halfSize * 2) / gridRes;

  const dummyTint = new THREE.Color();

  for (let gx = 0; gx < gridRes; gx++) {
    for (let gz = 0; gz < gridRes; gz++) {
      // Stratified spatial jitter within grid cell
      const jx = (hash2D(gx, gz, 1) - 0.5) * step * 0.88;
      const jz = (hash2D(gx, gz, 2) - 0.5) * step * 0.88;
      const wx = -halfSize + (gx + 0.5) * step + jx;
      const wz = -halfSize + (gz + 0.5) * step + jz;

      // 1. Spawn clearance & vista preservation (Wide, serene, open meadow around player spawn)
      const distToSpawn = Math.sqrt(wx * wx + wz * wz);
      if (distToSpawn < 24.0) {
        // Inside 6.0m: strictly clear of any flora
        // Between 6.0m and 24.0m: sparse ground wildflowers & grass tufts, 0 tall trees to keep vistas clear
        if (distToSpawn > 6.0 && hash2D(gx, gz, 3) < 0.22) {
          const sample = sampleMap2Terrain(wx, wz);
          if (!sample.isWater && sample.height > waterLevel && sample.height <= 52.0 && sample.steepness <= 0.42) {
            const sp = hash2D(gx, gz, 4) < 0.5 ? Map2FloraSpecies.Wildflower : Map2FloraSpecies.MeadowGrassTuft;
            const rFlowerTint = hash2D(gx, gz, 9);
            const flowerTint = new THREE.Color();
            if (sp === Map2FloraSpecies.Wildflower) {
              if (rFlowerTint < 0.25) flowerTint.setRGB(1.2, 0.4, 0.5);
              else if (rFlowerTint < 0.5) flowerTint.setRGB(1.2, 0.9, 0.2);
              else if (rFlowerTint < 0.75) flowerTint.setRGB(0.3, 0.8, 1.2);
              else flowerTint.setRGB(0.9, 0.4, 1.2);
            } else {
              const g = 0.85 + rFlowerTint * 0.3;
              flowerTint.setRGB(g * 0.88, g * 1.05, g * 0.75);
            }
            const p: FloraPlacement = {
              species: sp,
              x: wx,
              y: sample.height,
              z: wz,
              scale: 0.8 + hash2D(gx, gz, 5) * 0.4,
              rotationY: hash2D(gx, gz, 6) * Math.PI * 2,
              tiltX: sample.normal.z * 0.2,
              tiltZ: -sample.normal.x * 0.2,
              colorTint: flowerTint,
              isSpecimen: false,
              isTall: false,
            };
            bySpecies.get(sp)?.push(p);
            placements.push(p);
            groundCover.push(p);
          }
        }
        continue;
      }

      // 2. Query geological terrain sample
      const sample = sampleMap2Terrain(wx, wz);
      const { height, steepness, isWater, biomeId, normal, moisture } = sample;

      // 3. Strict Ecological Constraints:
      // - Zero vegetation in water (height <= waterLevel, isWater)
      if (isWater || height <= waterLevel) continue;
      // - Zero vegetation on high glacial snow peaks (height > 52.0m)
      if (height > 52.0) continue;
      // - Zero vegetation on sheer vertical cliffs (steepness > 0.42)
      if (steepness > 0.42) continue;

      // 4. Multi-Octave Organic Glade & Grove Partitioning:
      // Allocates >= 70% of ground area to open grassy glades and meadows, ~28% to organic tree groves
      const n1 = Math.sin(wx * 0.025 + 1.2) * Math.cos(wz * 0.025 + 0.8);
      const n2 = Math.sin(wx * 0.055 - 0.7) * Math.cos(wz * 0.052 + 1.3) * 0.5;
      const n3 = Math.sin(wx * 0.012 + wz * 0.014 + 2.0) * 0.35;
      const rawGladeNoise = (n1 + n2 + n3 + 1.85) / 3.7;

      // Environmental modulation:
      // Moisture bonus (trees cluster in moist river valleys and humid lowlands)
      const moistureFactor = 0.65 + 0.70 * moisture;
      // Slope penalty (steep slopes are open grass / rocky crags)
      const slopeFactor = Math.max(0.0, 1.0 - 2.2 * steepness);
      // High elevation falloff (sub-alpine treeline transition above 34m)
      const elevationFactor = height <= 34.0 ? 1.0 : Math.max(0.0, 1.0 - (height - 34.0) / 16.0);

      const groveIntensity = rawGladeNoise * moistureFactor * slopeFactor * elevationFactor;
      const isTreeGrove = groveIntensity > 0.44;

      const rSelect = hash2D(gx, gz, 7);
      const rScale = hash2D(gx, gz, 8);
      const rTint = hash2D(gx, gz, 9);

      let chosenSpecies: Map2FloraSpecies | null = null;
      let baseScale = 1.0;
      let isSpecimen = false;

      switch (biomeId) {
        case Map2Biome.TROPICAL_RAINFOREST: {
          if (isTreeGrove) {
            // 44% tree probability in rainforest grove with organic canopy spacing
            if (rSelect < 0.22) {
              chosenSpecies = Map2FloraSpecies.TropicalPalm;
              baseScale = 2.3 + rScale * 0.8;
            } else if (rSelect < 0.44) {
              chosenSpecies = Map2FloraSpecies.LushJungleTree;
              baseScale = 2.5 + rScale * 0.9;
            } else if (rSelect < 0.74) {
              chosenSpecies = Map2FloraSpecies.MeadowGrassTuft;
              baseScale = 1.2 + rScale * 0.5;
            } else if (rSelect < 0.90) {
              chosenSpecies = Map2FloraSpecies.Wildflower;
              baseScale = 1.0 + rScale * 0.4;
            }
          } else {
            // Open glade / riverbank clearing (>=65% open glade)
            if (rSelect < 0.020) {
              // Rare solitary specimen palm on riverbank clearing (3.6x - 4.4x scale)
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
            // 38% tree probability in acacia copse
            if (rSelect < 0.38) {
              chosenSpecies = Map2FloraSpecies.UmbrellaAcacia;
              baseScale = 2.2 + rScale * 0.8;
            } else if (rSelect < 0.68) {
              chosenSpecies = Map2FloraSpecies.DesertScrub;
              baseScale = 1.2 + rScale * 0.5;
            } else if (rSelect < 0.88) {
              chosenSpecies = Map2FloraSpecies.MeadowGrassTuft;
              baseScale = 1.0 + rScale * 0.4;
            }
          } else {
            // Vast open savanna grassland (>=70% open area)
            if (rSelect < 0.020) {
              // Majestic specimen umbrella acacia on solitary knoll (3.6x - 4.5x scale)
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
          // Arid landscape with sparse desert vegetation
          if (rSelect < 0.20) {
            chosenSpecies = Map2FloraSpecies.SaguaroCactus;
            baseScale = 1.8 + rScale * 0.8;
          } else if (rSelect < 0.42) {
            chosenSpecies = Map2FloraSpecies.DesertScrub;
            baseScale = 1.0 + rScale * 0.4;
          }
          break;
        }

        case Map2Biome.TEMPERATE_PLAINS: {
          if (isTreeGrove) {
            // 42% tree probability in temperate birch/oak grove
            if (rSelect < 0.24) {
              chosenSpecies = Map2FloraSpecies.BroadleafDeciduous;
              baseScale = 2.2 + rScale * 0.8;
            } else if (rSelect < 0.42) {
              chosenSpecies = Map2FloraSpecies.ConiferPine;
              baseScale = 2.0 + rScale * 0.7;
            } else if (rSelect < 0.74) {
              chosenSpecies = Map2FloraSpecies.MeadowGrassTuft;
              baseScale = 1.1 + rScale * 0.4;
            } else if (rSelect < 0.90) {
              chosenSpecies = Map2FloraSpecies.Wildflower;
              baseScale = 0.9 + rScale * 0.4;
            }
          } else {
            // Expansive rolling emerald meadow (>=70% open glade)
            if (rSelect < 0.020) {
              // Solitary ancient heritage oak (3.6x - 4.5x scale)
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
          // Clean snow peaks: no trees above 46m, sparse dwarf conifers below 46m on gentle slopes
          if (height <= 46.0 && steepness <= 0.38) {
            if (rSelect < 0.24) {
              chosenSpecies = Map2FloraSpecies.ConiferPine;
              baseScale = 1.5 + rScale * 0.6;
            } else if (rSelect < 0.48) {
              chosenSpecies = Map2FloraSpecies.CliffMoss;
              baseScale = 1.1 + rScale * 0.5;
            }
          }
          break;
        }

        case Map2Biome.ROCKY_CLIFFS: {
          // Clean rocky crags: no trees on sheer slopes > 0.38
          if (steepness <= 0.38) {
            if (rSelect < 0.42) {
              chosenSpecies = Map2FloraSpecies.CliffMoss;
              baseScale = 1.1 + rScale * 0.5;
            } else if (rSelect < 0.64) {
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

      const isTall = isTallFlora(chosenSpecies);

      // Unified Strict Slope Cap for all tall trees (steepness < 0.38) and alpine treeline (< 46.0m)
      if (isTall) {
        if (steepness >= 0.38 || height >= 46.0) {
          continue;
        }

        // Specimen Tree Spatial Isolation (>= 14.0m clearance against all tall trees)
        if (isSpecimen) {
          let isolated = true;
          for (let i = 0; i < tallTreePositions.length; i++) {
            const t = tallTreePositions[i];
            if (Math.hypot(wx - t.x, wz - t.z) < 14.0) {
              isolated = false;
              break;
            }
          }
          if (!isolated) {
            continue;
          }
        } else {
          // Regular grove tall tree: ensure it does not crowd an existing specimen tree (< 14.0m)
          let specimenClear = true;
          for (let i = 0; i < specimenTreePositions.length; i++) {
            const st = specimenTreePositions[i];
            if (Math.hypot(wx - st.x, wz - st.z) < 14.0) {
              specimenClear = false;
              break;
            }
          }
          if (!specimenClear) {
            continue;
          }
        }
      }

      // Canopy clearance check against spawn point
      const canopyR = FLORA_CANOPY_RADII[chosenSpecies] * baseScale;
      if (distToSpawn < spawnClearanceRadius + canopyR) continue;

      // Subtle per-instance color variation
      if (chosenSpecies === Map2FloraSpecies.Wildflower) {
        if (rTint < 0.25) dummyTint.setRGB(1.2, 0.4, 0.5);
        else if (rTint < 0.5) dummyTint.setRGB(1.2, 0.9, 0.2);
        else if (rTint < 0.75) dummyTint.setRGB(0.3, 0.8, 1.2);
        else dummyTint.setRGB(0.9, 0.4, 1.2);
      } else if (chosenSpecies === Map2FloraSpecies.MeadowGrassTuft) {
        if (biomeId === Map2Biome.ARID_SAVANNA) {
          const g = 0.85 + rTint * 0.2;
          dummyTint.setRGB(g * 1.15, g * 0.95, g * 0.5);
        } else {
          const g = 0.85 + rTint * 0.3;
          dummyTint.setRGB(g * 0.88, g * 1.05, g * 0.75);
        }
      } else if (chosenSpecies === Map2FloraSpecies.GlowingMushroom) {
        if (rTint < 0.5) dummyTint.setRGB(0.8, 0.4, 1.3);
        else dummyTint.setRGB(0.3, 1.1, 1.2);
      } else if (chosenSpecies === Map2FloraSpecies.CrystalCluster) {
        if (rTint < 0.5) dummyTint.setRGB(1.1, 0.5, 1.3);
        else dummyTint.setRGB(0.4, 1.0, 1.3);
      } else {
        const lum = 0.85 + rTint * 0.3;
        const warm = 0.95 + hash2D(gx, gz, 6) * 0.1;
        dummyTint.setRGB(lum * warm, lum, lum * (2 - warm));
      }

      const rotationY = hash2D(gx, gz, 7) * Math.PI * 2;
      const tiltX = isTall ? 0 : (normal.z * 0.2);
      const tiltZ = isTall ? 0 : (-normal.x * 0.2);

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
        tallTreePositions.push({ x: wx, z: wz });
        if (isSpecimen) {
          specimenTrees.push(p);
          specimenTreePositions.push({ x: wx, z: wz });
        }
      } else {
        groundCover.push(p);
      }
    }
  }

  return { placements, bySpecies, tallTrees, specimenTrees, groundCover };
}
