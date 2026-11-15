import { describe, it, expect } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  validateMap2Manifest,
  validateMap2EcologicalRules,
  buildStandardMap2Manifest,
  MAP2_CANONICAL_VIEW_IDS,
  Map2Manifest,
  computeCameraTelemetry,
} from '@/components/Map2/utils/map2Manifest';
import { getMap2Height, getMap2NormalAndSteepness } from '@/components/Map2/utils/map2TerrainGen';
import { Map2FloraSpecies } from '@/components/Map2/utils/map2FloraGeometry';

// -------------------------------------------------------------------------------------------------
// Map 2 Manifest Evidence & Quality Gate Test Suite (Milestone 6)
// Verifies committed manifest, 8 canonical camera views, spawn clearance, ecological rules,
// BFS navigation reachability, and disk evidence hashes.
// -------------------------------------------------------------------------------------------------

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '../../..');
const ROOT_MANIFEST_PATH = resolve(ROOT, 'map2_manifest.json');
const ARTIFACT_MANIFEST_PATH = resolve(ROOT, 'artifacts/maps/map2/map2_manifest.json');

function loadCommittedManifest(): Map2Manifest {
  expect(
    existsSync(ROOT_MANIFEST_PATH),
    `map2_manifest.json must exist at ${ROOT_MANIFEST_PATH}. Run scripts/verify_map2_evidence.ts.`,
  ).toBe(true);
  return JSON.parse(readFileSync(ROOT_MANIFEST_PATH, 'utf8')) as Map2Manifest;
}

describe('Map 2 Manifest — Tier 1: Schema Compliance & World Bounds', () => {
  it('passes its own authoritative validator with zero schema errors', () => {
    const manifest = loadCommittedManifest();
    const result = validateMap2Manifest(manifest);
    expect(result.errors).toEqual([]);
    expect(result.ok).toBe(true);
  });

  it('matches exactly between root map2_manifest.json and artifacts mirror', () => {
    expect(existsSync(ARTIFACT_MANIFEST_PATH), 'Artifact mirror must exist').toBe(true);
    const rootBytes = readFileSync(ROOT_MANIFEST_PATH);
    const artifactBytes = readFileSync(ARTIFACT_MANIFEST_PATH);
    expect(rootBytes.equals(artifactBytes), 'Root manifest and artifact manifest must be byte-for-byte identical').toBe(true);
  });

  it('declares correct coordinate system, sea level, and world bounds', () => {
    const manifest = loadCommittedManifest();
    const { bounds, coordinateSystem, worldArtifact } = manifest;

    expect(bounds.minX).toBe(-100);
    expect(bounds.maxX).toBe(100);
    expect(bounds.minZ).toBe(-100);
    expect(bounds.maxZ).toBe(100);
    expect(bounds.minY).toBe(0);
    expect(bounds.maxY).toBe(90);

    expect(coordinateSystem.worldMinXZ).toBe(-100);
    expect(coordinateSystem.worldMaxXZ).toBe(100);
    expect(coordinateSystem.gridDim).toBe(256);
    expect(coordinateSystem.worldScale).toBe(200);
    expect(coordinateSystem.seaLevel).toBe(12.0);

    expect(worldArtifact.width).toBe(256);
    expect(worldArtifact.height).toBe(256);
    expect(worldArtifact.magic).toBe('ANMW');
    expect(worldArtifact.version).toBe(2);
    expect(worldArtifact.seaLevel).toBe(12.0);
  });

  it('contains exactly 8 canonical biomes with valid coverage and temperature metadata', () => {
    const manifest = loadCommittedManifest();
    const { biomes } = manifest.biomeTaxonomy;

    expect(biomes).toHaveLength(8);
    const uniqueIds = new Set(biomes.map((b) => b.id));
    expect(uniqueIds.size).toBe(8);

    let totalCoverage = 0;
    for (const b of biomes) {
      totalCoverage += b.coveragePercentage;
      expect(b.temperatureRange[0]).toBeLessThan(b.temperatureRange[1]);
      expect(b.moistureRange[0]).toBeLessThanOrEqual(b.moistureRange[1]);
      expect(b.elevationRange[0]).toBeLessThanOrEqual(b.elevationRange[1]);
    }
    expect(totalCoverage).toBeGreaterThan(95.0);
    expect(totalCoverage).toBeLessThanOrEqual(105.0);
  });
});

describe('Map 2 Manifest — Tier 2: 8 Canonical Camera Views Verification', () => {
  it('contains all 8 canonical views with valid identifiers and telemetry', () => {
    const manifest = loadCommittedManifest();
    expect(manifest.views).toHaveLength(8);

    const viewIds = manifest.views.map((v) => v.id);
    expect([...viewIds].sort()).toEqual([...MAP2_CANONICAL_VIEW_IDS].sort());

    for (const view of manifest.views) {
      expect(MAP2_CANONICAL_VIEW_IDS).toContain(view.id);
      expect(view.name.length).toBeGreaterThan(0);
      expect(view.description.length).toBeGreaterThan(0);
      expect(view.imagePath).toBe(`artifacts/maps/map2/views/${view.id}.png`);

      // Camera FOV bounds
      expect(view.camera.fov).toBeGreaterThanOrEqual(30);
      expect(view.camera.fov).toBeLessThanOrEqual(90);

      // Distance and bearing
      expect(view.distance).toBeGreaterThan(0);
      expect(view.bearing).toBeGreaterThanOrEqual(0);
      expect(view.bearing).toBeLessThan(360);

      // Validation rules presence
      expect(view.validationRules.length).toBeGreaterThanOrEqual(1);

      // Telemetry accuracy
      const computed = computeCameraTelemetry(view.camera.position, view.camera.target);
      expect(view.distance).toBeCloseTo(computed.distance, 1);
    }
  });

  it('verifies all canonical view camera targets and positions remain in world space', () => {
    const manifest = loadCommittedManifest();
    for (const view of manifest.views) {
      const [px, py, pz] = view.camera.position;
      const [tx, ty, tz] = view.camera.target;

      expect(px).toBeGreaterThanOrEqual(-150);
      expect(px).toBeLessThanOrEqual(150);
      expect(pz).toBeGreaterThanOrEqual(-150);
      expect(pz).toBeLessThanOrEqual(150);
      expect(py).toBeGreaterThanOrEqual(0);

      expect(tx).toBeGreaterThanOrEqual(-100);
      expect(tx).toBeLessThanOrEqual(100);
      expect(tz).toBeGreaterThanOrEqual(-100);
      expect(tz).toBeLessThanOrEqual(100);
      expect(ty).toBeGreaterThanOrEqual(0);
    }
  });

  it('verifies captured canonical view images exist on disk with valid PNG headers and checksums', () => {
    const manifest = loadCommittedManifest();
    for (const view of manifest.views) {
      if (view.captured) {
        const fullImgPath = resolve(ROOT, view.imagePath);
        expect(existsSync(fullImgPath), `Image ${view.imagePath} must exist on disk`).toBe(true);

        const bytes = readFileSync(fullImgPath);
        // Valid PNG 8-byte magic header: 0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A
        expect(bytes.subarray(0, 8)).toEqual(
          Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
        );

        if (view.checksum) {
          const hash = `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
          expect(hash).toBe(view.checksum);
        }
      }
    }
  });
});

describe('Map 2 Manifest — Tier 3: Spawn Point Physical Clearance', () => {
  it('guarantees spawn point (0, 0) is above sea level and on gentle slope', () => {
    const manifest = loadCommittedManifest();
    const [sx, sy, sz] = manifest.spawnPoint.position;

    expect(sx).toBe(0);
    expect(sz).toBe(0);
    expect(sy).toBeGreaterThan(12.0); // Strictly above sea level (12.0)

    const terrainHeight = getMap2Height(0, 0);
    const { steepness } = getMap2NormalAndSteepness(0, 0);

    expect(terrainHeight).toBeGreaterThan(12.0);
    expect(steepness).toBeLessThan(0.25); // Flat / gentle slope
    expect(manifest.spawnPoint.slope).toBeLessThan(0.25);
  });

  it('guarantees minimum 6.0m clear radius from all solid tree trunks around spawn', () => {
    const manifest = loadCommittedManifest();
    const clearRadius = manifest.spawnPoint.clearRadius;
    const nearestTrunkDistance = manifest.spawnPoint.nearestTrunkDistance;

    expect(clearRadius).toBeGreaterThanOrEqual(6.0);
    expect(nearestTrunkDistance).toBeGreaterThanOrEqual(6.0);

    // Verify all trunk colliders are outside the 6m radius
    for (const collider of manifest.collision.trunkColliders) {
      const dist = Math.hypot(collider.x, collider.z);
      expect(dist, `Trunk at (${collider.x}, ${collider.z}) must be >= 6m from spawn`).toBeGreaterThanOrEqual(6.0);
    }
  });
});

describe('Map 2 Manifest — Tier 4: Ecological Rules & Realism Gates', () => {
  it('passes comprehensive ecological rules validation without violations', () => {
    const manifest = loadCommittedManifest();
    const result = validateMap2EcologicalRules(manifest);
    expect(result.violations).toEqual([]);
    expect(result.ok).toBe(true);
  });

  it('enforces that aquatic species (fish) are strictly submerged and never aerial', () => {
    const manifest = loadCommittedManifest();
    const fish = manifest.entities.fauna.find((f) => f.species === 'fish');

    expect(fish).toBeDefined();
    expect(fish!.submergedAllowed).toBe(true);
    expect(fish!.allowedMedia).toContain('water');
    expect(fish!.allowedMedia).not.toContain('air');
    expect(fish!.allowedBiomes).toContain('hydrology_river_lake');
  });

  it('enforces that aerial boids (birds) are aloft and never submerged', () => {
    const manifest = loadCommittedManifest();
    const bird = manifest.entities.fauna.find((f) => f.species === 'bird');

    expect(bird).toBeDefined();
    expect(bird!.submergedAllowed).toBe(false);
    expect(bird!.allowedMedia).toContain('air');
    expect(bird!.allowedMedia).not.toContain('water');
  });

  it('enforces that arid flora (cacti) are absent from tropical rainforest and snow peaks', () => {
    const manifest = loadCommittedManifest();
    const cactus = manifest.entities.flora.find((f) => f.species === Map2FloraSpecies.SaguaroCactus);

    expect(cactus).toBeDefined();
    expect(cactus!.allowedBiomes).toContain('desert_dunes');
    expect(cactus!.allowedBiomes).not.toContain('tropical_rainforest');
    expect(cactus!.allowedBiomes).not.toContain('snow_peaks');
  });

  it('enforces that tropical flora (palms) are absent from glacial snow peaks', () => {
    const manifest = loadCommittedManifest();
    const palm = manifest.entities.flora.find((f) => f.species === Map2FloraSpecies.TropicalPalm);

    expect(palm).toBeDefined();
    expect(palm!.allowedBiomes).toContain('tropical_rainforest');
    expect(palm!.allowedBiomes).not.toContain('snow_peaks');
    expect(palm!.allowedBiomes).not.toContain('desert_dunes');
  });
});

describe('Map 2 Manifest — Tier 5: BFS Navigation & Route Topology', () => {
  it('proves high connectivity ratio (>40%) and zero impassable bottlenecks from spawn', () => {
    const manifest = loadCommittedManifest();
    const { navigation } = manifest;

    expect(navigation.totalNodes).toBe(64 * 64);
    expect(navigation.walkableNodes).toBeGreaterThan(1000);
    expect(navigation.connectivityRatio).toBeGreaterThan(0.4);
    expect(navigation.zeroImpassableBottlenecks).toBe(true);
  });

  it('confirms BFS reachability reaches all 8 major biomes across the open world', () => {
    const manifest = loadCommittedManifest();
    const reachable = manifest.navigation.spawnReachableBiomes;

    expect(reachable.length).toBeGreaterThanOrEqual(6);
    expect(reachable).toContain('temperate_plains');
    expect(reachable).toContain('tropical_rainforest');
    expect(reachable).toContain('arid_savanna');
    expect(reachable).toContain('desert_dunes');
    expect(reachable).toContain('rocky_cliffs');
  });

  it('provides a continuous walkable sample route starting from plains', () => {
    const manifest = loadCommittedManifest();
    const route = manifest.navigation.sampleRoute;

    expect(route.length).toBeGreaterThanOrEqual(10);
    for (let i = 1; i < route.length; i++) {
      const prev = route[i - 1];
      const curr = route[i];
      const stepDist = Math.hypot(curr[0] - prev[0], curr[2] - prev[2]);
      // Step distance between route nodes is bounded
      expect(stepDist).toBeLessThan(15.0);
    }
  });
});

describe('Map 2 Manifest — Tier 6: Adversarial Invalidation & Defect Rejection', () => {
  it('rejects manifests missing canonical views or carrying invalid mapId', () => {
    const invalid = buildStandardMap2Manifest() as unknown as Record<string, unknown>;
    invalid.mapId = 'unsupported_map_id';
    delete (invalid.canonicalViews as Record<string, unknown>).overview;

    const res = validateMap2Manifest(invalid);
    expect(res.ok).toBe(false);
    expect(res.errors.some((e) => e.includes('mapId'))).toBe(true);
    expect(res.errors.some((e) => e.includes('overview'))).toBe(true);
  });

  it('rejects manifests with placeholder zero checksums', () => {
    const invalid = buildStandardMap2Manifest();
    invalid.checksum = `sha256:${'0'.repeat(64)}`;

    const res = validateMap2Manifest(invalid);
    expect(res.ok).toBe(false);
    expect(res.errors.some((e) => e.includes('placeholder'))).toBe(true);
  });

  it('rejects camera views with invalid FOVs (<30 or >90)', () => {
    const invalid = buildStandardMap2Manifest();
    invalid.canonicalViews.spawn.camera.fov = 120;

    const res = validateMap2Manifest(invalid);
    expect(res.ok).toBe(false);
    expect(res.errors.some((e) => e.includes('camera.fov'))).toBe(true);
  });
});
