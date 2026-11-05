import * as THREE from 'three';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';

// -------------------------------------------------------------------------------------------------
// Map 2 Procedural Flora Geometry Architecture (10+ Species with Baked Two-Tone Vertex Colors)
// -------------------------------------------------------------------------------------------------

/**
 * 12 Canonical Flora Species for Map 2 Multi-Biome Open-World Ecosystem.
 */
export enum Map2FloraSpecies {
  ConiferPine = 'conifer_pine',
  BroadleafDeciduous = 'broadleaf_deciduous',
  TropicalPalm = 'tropical_palm',
  UmbrellaAcacia = 'umbrella_acacia',
  SaguaroCactus = 'saguaro_cactus',
  DesertScrub = 'desert_scrub',
  MeadowGrassTuft = 'meadow_grass_tuft',
  Wildflower = 'wildflower',
  GlowingMushroom = 'glowing_mushroom',
  CrystalCluster = 'crystal_cluster',
  LushJungleTree = 'lush_jungle_tree',
  CliffMoss = 'cliff_moss',
}

/**
 * Fills a BufferGeometry's vertex colors with a flat color (hex string) and computes normals.
 * This guarantees that when geometries are merged into a single BufferGeometry, each sub-part
 * retains its distinct tone (e.g. brown wood trunk + green needle canopy).
 */
export function paint(geom: THREE.BufferGeometry, hex: string): THREE.BufferGeometry {
  const c = new THREE.Color(hex);
  const count = geom.attributes.position.count;
  const colors = new Float32Array(count * 3);
  for (let i = 0; i < count; i++) {
    colors[i * 3] = c.r;
    colors[i * 3 + 1] = c.g;
    colors[i * 3 + 2] = c.b;
  }
  geom.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  if (!geom.attributes.normal) {
    geom.computeVertexNormals();
  }
  return geom;
}

/**
 * Merges an array of sub-geometries into a single BufferGeometry.
 * Properly cleans up non-merged parts to prevent memory leaks.
 */
function mergeParts(parts: THREE.BufferGeometry[]): THREE.BufferGeometry {
  // Ensure every part has normals and colors
  parts.forEach((p) => {
    if (!p.attributes.normal) p.computeVertexNormals();
  });
  const merged = mergeGeometries(parts, false) ?? parts[0];
  parts.forEach((p) => {
    if (p !== merged) p.dispose();
  });
  merged.computeVertexNormals();
  return merged;
}

const COLOR_TRUNK_BROWN = '#5c3d24';
const COLOR_TRUNK_DARK = '#422817';
const COLOR_TRUNK_PALM = '#785b3a';
const COLOR_TRUNK_ACACIA = '#694f38';

/**
 * Procedural low-poly geometry generator for 12 distinct Map 2 flora species.
 * Each geometry has its base sitting precisely on y = 0, clean vertex normals,
 * and baked two-tone vertex colors merged into a SINGLE draw call.
 */
export function makeMap2FloraGeometry(species: Map2FloraSpecies): THREE.BufferGeometry {
  const parts: THREE.BufferGeometry[] = [];

  switch (species) {
    // 1. Alpine / Taiga Conifer Pine Tree: Multi-tier tiered conical crown
    case Map2FloraSpecies.ConiferPine: {
      // Trunk
      parts.push(paint(new THREE.CylinderGeometry(0.1, 0.16, 0.7, 6).translate(0, 0.35, 0), COLOR_TRUNK_DARK));
      // Tier 1 (Lowest cone)
      parts.push(paint(new THREE.ConeGeometry(0.72, 1.2, 7).translate(0, 1.15, 0), '#1e4620'));
      // Tier 2 (Middle cone)
      parts.push(paint(new THREE.ConeGeometry(0.55, 1.0, 7).translate(0, 1.85, 0), '#285e2b'));
      // Tier 3 (Top cone)
      parts.push(paint(new THREE.ConeGeometry(0.36, 0.8, 6).translate(0, 2.45, 0), '#387c3c'));
      break;
    }

    // 2. Temperate Deciduous Broadleaf Tree (Oak / Birch): Lush rounded multi-sphere canopy
    case Map2FloraSpecies.BroadleafDeciduous: {
      // Trunk
      parts.push(paint(new THREE.CylinderGeometry(0.12, 0.18, 0.8, 6).translate(0, 0.4, 0), COLOR_TRUNK_BROWN));
      // Main central crown
      parts.push(paint(new THREE.SphereGeometry(0.75, 7, 6).translate(0, 1.35, 0), '#3f8a3a'));
      // Secondary foliage lobes
      parts.push(paint(new THREE.SphereGeometry(0.48, 6, 5).translate(0.38, 1.6, 0.22), '#4d9c46'));
      parts.push(paint(new THREE.SphereGeometry(0.45, 6, 5).translate(-0.35, 1.45, -0.2), '#5ea844'));
      parts.push(paint(new THREE.SphereGeometry(0.38, 5, 4).translate(0.15, 1.9, -0.25), '#6cb24f'));
      break;
    }

    // 3. Tropical Jungle Palm Tree: Curved slender trunk with 6 splayed arched fronds
    case Map2FloraSpecies.TropicalPalm: {
      // Lower trunk segment
      parts.push(paint(new THREE.CylinderGeometry(0.09, 0.14, 1.2, 6).translate(0, 0.6, 0), COLOR_TRUNK_PALM));
      // Upper leaning trunk segment
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.07, 0.09, 0.9, 5).rotateZ(-0.2).translate(0.12, 1.6, 0),
          COLOR_TRUNK_PALM,
        ),
      );
      // Palm Fronds: 6 splayed flattened cones radiating from crown apex
      for (let k = 0; k < 6; k++) {
        const a = (k / 6) * Math.PI * 2;
        parts.push(
          paint(
            new THREE.ConeGeometry(0.12, 1.05, 4)
              .rotateX(Math.PI / 2.2)
              .rotateY(a)
              .translate(0.2 + Math.cos(a) * 0.38, 2.05, Math.sin(a) * 0.38),
            k % 2 === 0 ? '#1b6e2e' : '#28883e',
          ),
        );
      }
      break;
    }

    // 4. Umbrella Acacia Tree: Tall savanna trunk branching into wide flat-topped horizontal canopy
    case Map2FloraSpecies.UmbrellaAcacia: {
      // Main trunk
      parts.push(paint(new THREE.CylinderGeometry(0.08, 0.14, 1.3, 5).translate(0, 0.65, 0), COLOR_TRUNK_ACACIA));
      // Branch splits
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.05, 0.08, 0.6, 4).rotateZ(0.45).translate(0.18, 1.45, 0.08),
          COLOR_TRUNK_ACACIA,
        ),
      );
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.05, 0.08, 0.6, 4).rotateZ(-0.45).translate(-0.18, 1.45, -0.08),
          COLOR_TRUNK_ACACIA,
        ),
      );
      // Flat spreading umbrella canopy
      parts.push(
        paint(
          new THREE.CylinderGeometry(1.15, 0.6, 0.35, 8).translate(0, 1.75, 0),
          '#7ca03e',
        ),
      );
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.85, 0.45, 0.25, 7).translate(0.2, 1.92, 0.1),
          '#8ea846',
        ),
      );
      break;
    }

    // 5. Desert Saguaro Cactus: Ribbed vertical stem with two upward-bent branching arms
    case Map2FloraSpecies.SaguaroCactus: {
      // Main vertical body
      parts.push(paint(new THREE.CylinderGeometry(0.18, 0.22, 1.6, 7).translate(0, 0.8, 0), '#4d7c38'));
      // Dome cap
      parts.push(paint(new THREE.SphereGeometry(0.18, 7, 5).translate(0, 1.6, 0), '#598e42'));
      // Right branch (horizontal + vertical arm)
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.09, 0.11, 0.35, 5).rotateZ(Math.PI / 2).translate(0.25, 0.85, 0),
          '#4d7c38',
        ),
      );
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.08, 0.09, 0.55, 5).translate(0.42, 1.1, 0),
          '#598e42',
        ),
      );
      // Left branch (horizontal + vertical arm)
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.08, 0.1, 0.3, 5).rotateZ(-Math.PI / 2).translate(-0.22, 0.65, 0.05),
          '#4d7c38',
        ),
      );
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.07, 0.08, 0.45, 5).translate(-0.36, 0.85, 0.05),
          '#598e42',
        ),
      );
      break;
    }

    // 6. Desert Scrub & Thorny Shrub: Compact multi-lobe arid bush
    case Map2FloraSpecies.DesertScrub: {
      parts.push(paint(new THREE.SphereGeometry(0.35, 6, 5).scale(1.2, 0.8, 1.1).translate(0, 0.28, 0), '#827f58'));
      parts.push(
        paint(new THREE.SphereGeometry(0.25, 5, 4).translate(0.22, 0.22, 0.12), '#949168'),
      );
      parts.push(
        paint(new THREE.SphereGeometry(0.22, 5, 4).translate(-0.18, 0.24, -0.1), '#75724d'),
      );
      break;
    }

    // 7. Temperate Meadow Grass Tufts: Splayed blades of grass
    case Map2FloraSpecies.MeadowGrassTuft: {
      // 5 splayed blade pyramids
      parts.push(paint(new THREE.ConeGeometry(0.14, 0.45, 4).translate(0, 0.22, 0), '#73a942'));
      parts.push(
        paint(
          new THREE.ConeGeometry(0.11, 0.38, 4).rotateZ(0.25).translate(0.12, 0.18, 0.05),
          '#8ac926',
        ),
      );
      parts.push(
        paint(
          new THREE.ConeGeometry(0.1, 0.35, 4).rotateZ(-0.25).translate(-0.11, 0.16, -0.04),
          '#5ea338',
        ),
      );
      parts.push(
        paint(
          new THREE.ConeGeometry(0.09, 0.32, 4).rotateX(0.22).translate(0.04, 0.15, 0.1),
          '#aad576',
        ),
      );
      break;
    }

    // 8. Temperate Wildflower: Slender green stem with bright colorful blossom petals
    case Map2FloraSpecies.Wildflower: {
      // Green stem & leaves
      parts.push(paint(new THREE.CylinderGeometry(0.02, 0.03, 0.4, 4).translate(0, 0.2, 0), '#4d7c0f'));
      parts.push(
        paint(new THREE.ConeGeometry(0.06, 0.18, 3).rotateZ(0.4).translate(0.06, 0.14, 0), '#65a30d'),
      );
      // Blossom flower center
      parts.push(paint(new THREE.SphereGeometry(0.06, 5, 4).translate(0, 0.42, 0), '#facc15'));
      // Petals
      for (let k = 0; k < 5; k++) {
        const a = (k / 5) * Math.PI * 2;
        parts.push(
          paint(
            new THREE.SphereGeometry(0.05, 4, 3)
              .scale(1.2, 0.4, 1)
              .rotateZ(0.3)
              .rotateY(a)
              .translate(Math.cos(a) * 0.08, 0.42, Math.sin(a) * 0.08),
            '#f43f5e',
          ),
        );
      }
      break;
    }

    // 9. Bioluminescent Glowing Mushrooms: Pale stalk with luminescent vibrant cap
    case Map2FloraSpecies.GlowingMushroom: {
      // Stalk
      parts.push(paint(new THREE.CylinderGeometry(0.06, 0.1, 0.35, 6).translate(0, 0.17, 0), '#e2e8f0'));
      // Glowing domed mushroom cap
      parts.push(
        paint(
          new THREE.SphereGeometry(0.32, 7, 6, 0, Math.PI * 2, 0, Math.PI * 0.55).translate(0, 0.35, 0),
          '#c084fc',
        ),
      );
      // Small secondary mini mushroom
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.03, 0.05, 0.2, 5).rotateZ(0.3).translate(0.18, 0.1, 0.05),
          '#e2e8f0',
        ),
      );
      parts.push(
        paint(
          new THREE.SphereGeometry(0.16, 6, 5, 0, Math.PI * 2, 0, Math.PI * 0.55).translate(0.24, 0.22, 0.05),
          '#38bdf8',
        ),
      );
      break;
    }

    // 10. Bioluminescent Crystal Cluster / Spire: Faceted hexagonal prismatic quartz shards
    case Map2FloraSpecies.CrystalCluster: {
      // Main central spire
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.04, 0.18, 1.4, 6).translate(0, 0.7, 0),
          '#a855f7',
        ),
      );
      // Outer satellite crystal shards
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.02, 0.12, 0.95, 6).rotateZ(0.25).translate(0.2, 0.45, 0.1),
          '#38bdf8',
        ),
      );
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.02, 0.11, 0.8, 6).rotateZ(-0.28).translate(-0.18, 0.38, -0.08),
          '#e879f9',
        ),
      );
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.02, 0.09, 0.65, 5).rotateX(0.22).translate(0.05, 0.3, 0.16),
          '#818cf8',
        ),
      );
      break;
    }

    // 11. Lush Tropical Jungle Giant Canopy Tree: Massive buttressed trunk with layered broad canopy
    case Map2FloraSpecies.LushJungleTree: {
      // Thick buttressed trunk
      parts.push(paint(new THREE.CylinderGeometry(0.15, 0.28, 1.5, 7).translate(0, 0.75, 0), COLOR_TRUNK_DARK));
      // Lower canopy layer
      parts.push(
        paint(
          new THREE.SphereGeometry(1.0, 7, 5).scale(1.1, 0.5, 1.1).translate(0, 1.7, 0),
          '#14532d',
        ),
      );
      // Mid canopy layer
      parts.push(
        paint(
          new THREE.SphereGeometry(0.8, 7, 5).scale(1.0, 0.55, 1.0).translate(0.2, 2.1, 0.15),
          '#166534',
        ),
      );
      // Top canopy crown
      parts.push(
        paint(
          new THREE.SphereGeometry(0.55, 6, 4).scale(1.0, 0.5, 1.0).translate(-0.15, 2.45, -0.1),
          '#15803d',
        ),
      );
      break;
    }

    // 12. Rocky Cliff Moss & Gnarled Alpine Shrub: Compact rugged bonsai-like conifer
    case Map2FloraSpecies.CliffMoss:
    default: {
      // Gnarled twisted mini trunk
      parts.push(
        paint(
          new THREE.CylinderGeometry(0.06, 0.12, 0.5, 5).rotateZ(0.22).translate(0.05, 0.22, 0),
          '#44403c',
        ),
      );
      // Hardy evergreen moss cushion
      parts.push(paint(new THREE.SphereGeometry(0.42, 6, 5).scale(1.2, 0.6, 1.1).translate(0.1, 0.42, 0), '#365314'));
    }
  }

  const finalGeom = mergeParts(parts);
  finalGeom.computeBoundingBox();
  if (finalGeom.boundingBox && finalGeom.boundingBox.min.y !== 0) {
    finalGeom.translate(0, -finalGeom.boundingBox.min.y, 0);
    finalGeom.computeBoundingBox();
  }
  return finalGeom;
}

/**
 * Calculates the bounding horizontal footprint radius of a flora species at unit scale.
 */
export function measureMap2FloraFootprintRadius(species: Map2FloraSpecies): number {
  const geom = makeMap2FloraGeometry(species);
  geom.computeBoundingBox();
  const box = geom.boundingBox;
  let maxR = 0.5;
  if (box) {
    const rX = Math.max(Math.abs(box.min.x), Math.abs(box.max.x));
    const rZ = Math.max(Math.abs(box.min.z), Math.abs(box.max.z));
    maxR = Math.sqrt(rX * rX + rZ * rZ);
  }
  geom.dispose();
  return maxR;
}

/**
 * Authoritative canopy radii for spawn clearance and camera collision checks (at unit scale).
 */
export const FLORA_CANOPY_RADII: Record<Map2FloraSpecies, number> = {
  [Map2FloraSpecies.ConiferPine]: 0.75,
  [Map2FloraSpecies.BroadleafDeciduous]: 0.85,
  [Map2FloraSpecies.TropicalPalm]: 1.1,
  [Map2FloraSpecies.UmbrellaAcacia]: 1.25,
  [Map2FloraSpecies.SaguaroCactus]: 0.45,
  [Map2FloraSpecies.DesertScrub]: 0.38,
  [Map2FloraSpecies.MeadowGrassTuft]: 0.25,
  [Map2FloraSpecies.Wildflower]: 0.15,
  [Map2FloraSpecies.GlowingMushroom]: 0.35,
  [Map2FloraSpecies.CrystalCluster]: 0.35,
  [Map2FloraSpecies.LushJungleTree]: 1.35,
  [Map2FloraSpecies.CliffMoss]: 0.45,
};

/**
 * Authoritative solid trunk collider radii for Walk-mode player push-out resolution.
 */
export const FLORA_TRUNK_COLLIDER_RADII: Record<Map2FloraSpecies, number> = {
  [Map2FloraSpecies.ConiferPine]: 0.16,
  [Map2FloraSpecies.BroadleafDeciduous]: 0.18,
  [Map2FloraSpecies.TropicalPalm]: 0.14,
  [Map2FloraSpecies.UmbrellaAcacia]: 0.14,
  [Map2FloraSpecies.SaguaroCactus]: 0.22,
  [Map2FloraSpecies.DesertScrub]: 0.0, // Non-solid ground cover
  [Map2FloraSpecies.MeadowGrassTuft]: 0.0, // Non-solid
  [Map2FloraSpecies.Wildflower]: 0.0, // Non-solid
  [Map2FloraSpecies.GlowingMushroom]: 0.0, // Non-solid
  [Map2FloraSpecies.CrystalCluster]: 0.15, // Solid crystal spire
  [Map2FloraSpecies.LushJungleTree]: 0.28,
  [Map2FloraSpecies.CliffMoss]: 0.0,
};

/**
 * Returns whether the species is a tall tree that casts shadow and has significant canopy sway.
 */
export function isTallFlora(species: Map2FloraSpecies): boolean {
  switch (species) {
    case Map2FloraSpecies.ConiferPine:
    case Map2FloraSpecies.BroadleafDeciduous:
    case Map2FloraSpecies.TropicalPalm:
    case Map2FloraSpecies.UmbrellaAcacia:
    case Map2FloraSpecies.SaguaroCactus:
    case Map2FloraSpecies.LushJungleTree:
      return true;
    default:
      return false;
  }
}

/**
 * Returns whether the species is low ground cover (grass tufts, wildflowers, shrubs).
 */
export function isGroundCoverFlora(species: Map2FloraSpecies): boolean {
  return !isTallFlora(species);
}
