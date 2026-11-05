import { describe, it, expect } from 'vitest';

// -------------------------------------------------------------------------------------------------
// Map 2 Manifest Validation Test Suite: Schema, 8 Canonical Views, Spawn Clearance & Quality Gates
// -------------------------------------------------------------------------------------------------

export interface Map2CanonicalView {
  id: string;
  name: string;
  description: string;
  camera: {
    position: [number, number, number];
    target: [number, number, number];
    fov: number;
  };
}

export interface Map2ManifestSchema {
  mapId: string;
  mapName: string;
  version: string;
  seed: string;
  bounds: {
    minX: number;
    maxX: number;
    minY: number;
    maxY: number;
    minZ: number;
    maxZ: number;
  };
  spawnPoint: {
    position: [number, number, number];
    bearing: number;
  };
  canonicalViews: Record<string, Map2CanonicalView>;
  biomes: string[];
  checksum?: string;
}

export const MAP2_CANONICAL_VIEW_IDS = [
  'overview',
  'navigation',
  'collision',
  'lighting',
  'spawn',
  'water',
  'biome_transition',
  'ecosystem',
] as const;

export function validateMap2Manifest(manifest: unknown): { ok: boolean; errors: string[] } {
  const errors: string[] = [];

  if (!manifest || typeof manifest !== 'object') {
    return { ok: false, errors: ['Manifest must be a non-null object'] };
  }

  const m = manifest as Partial<Map2ManifestSchema>;

  if (m.mapId !== 'map2_openworld') {
    errors.push(`Invalid mapId: expected 'map2_openworld', got '${m.mapId}'`);
  }

  if (!m.mapName || typeof m.mapName !== 'string') {
    errors.push('mapName is required and must be a string');
  }

  if (!m.version || typeof m.version !== 'string') {
    errors.push('version is required and must be a string');
  }

  if (!m.seed || typeof m.seed !== 'string') {
    errors.push('seed is required and must be a non-empty string');
  }

  if (!m.bounds || typeof m.bounds !== 'object') {
    errors.push('bounds object is required');
  } else {
    if (m.bounds.minX >= m.bounds.maxX) errors.push('bounds: minX must be < maxX');
    if (m.bounds.minY >= m.bounds.maxY) errors.push('bounds: minY must be < maxY');
    if (m.bounds.minZ >= m.bounds.maxZ) errors.push('bounds: minZ must be < maxZ');
  }

  if (!m.spawnPoint || !Array.isArray(m.spawnPoint.position) || m.spawnPoint.position.length !== 3) {
    errors.push('spawnPoint.position must be a 3-element coordinate array [x, y, z]');
  }

  if (!m.canonicalViews || typeof m.canonicalViews !== 'object') {
    errors.push('canonicalViews object is required');
  } else {
    for (const viewId of MAP2_CANONICAL_VIEW_IDS) {
      const view = m.canonicalViews[viewId];
      if (!view) {
        errors.push(`Missing required canonical view: '${viewId}'`);
        continue;
      }
      if (!view.camera || !Array.isArray(view.camera.position) || view.camera.position.length !== 3) {
        errors.push(`Canonical view '${viewId}': camera.position must be [x, y, z]`);
      }
      if (!view.camera || !Array.isArray(view.camera.target) || view.camera.target.length !== 3) {
        errors.push(`Canonical view '${viewId}': camera.target must be [x, y, z]`);
      }
      if (typeof view.camera?.fov !== 'number' || view.camera.fov < 30 || view.camera.fov > 90) {
        errors.push(`Canonical view '${viewId}': camera.fov must be between 30 and 90 degrees`);
      }
    }
  }

  if (!Array.isArray(m.biomes) || m.biomes.length < 8) {
    errors.push('biomes must contain at least 8 distinct biomes');
  }

  if (m.checksum) {
    if (!/^sha256:[0-9a-f]{64}$/.test(m.checksum)) {
      errors.push('checksum must be formatted as sha256:<64 hex chars>');
    }
    if (m.checksum === `sha256:${'0'.repeat(64)}`) {
      errors.push('placeholder all-zero checksum is prohibited');
    }
  }

  return { ok: errors.length === 0, errors };
}

export function buildStandardMap2Manifest(): Map2ManifestSchema {
  return {
    mapId: 'map2_openworld',
    mapName: 'Map 2 — Photorealistic Open World Ecosystem',
    version: '1.0.0',
    seed: 'anima_map2_photorealistic_v1',
    bounds: {
      minX: -100,
      maxX: 100,
      minY: 0,
      maxY: 100,
      minZ: -100,
      maxZ: 100,
    },
    spawnPoint: {
      position: [0, 16.7, 0],
      bearing: 0,
    },
    canonicalViews: {
      overview: {
        id: 'overview',
        name: 'Whole World Aerial Overview',
        description: 'Spectator bird-eye perspective showing all biomes and geological transitions',
        camera: { position: [0, 110, 110], target: [0, 15, 0], fov: 55 },
      },
      navigation: {
        id: 'navigation',
        name: 'Valley Navigation Pathway',
        description: 'First-person ground path across temperate plains into tropical forest',
        camera: { position: [-15, 18, 20], target: [0, 17, -10], fov: 60 },
      },
      collision: {
        id: 'collision',
        name: 'Rocky Cliff Collision Face',
        description: 'Steep rocky cliff boundary testing slide mechanics and collision bounds',
        camera: { position: [40, 25, -30], target: [55, 35, -30], fov: 50 },
      },
      lighting: {
        id: 'lighting',
        name: 'Atmospheric Sunset Golden Hour',
        description: 'Rayleigh scattering sky dome and volumetric mountain fog during sunset',
        camera: { position: [-45, 30, 45], target: [10, 40, -10], fov: 50 },
      },
      spawn: {
        id: 'spawn',
        name: 'Initial Player Spawn Clearing',
        description: 'Safe temperate clearing with 360-degree panorama of surrounding biome landmarks',
        camera: { position: [0, 16.7, 0], target: [0, 16.7, -20], fov: 65 },
      },
      water: {
        id: 'water',
        name: 'Waterfall and Lake Shoreline',
        description: 'Gerstner wave dynamics, depth fading extinction, and shoreline foam edge',
        camera: { position: [-25, 8, -15], target: [-10, 4, 5], fov: 55 },
      },
      biome_transition: {
        id: 'biome_transition',
        name: 'Desert to Savanna Ecotone',
        description: 'Smooth continuous triplanar splatting transition between sand dunes and arid savanna',
        camera: { position: [50, 20, 40], target: [20, 15, 10], fov: 50 },
      },
      ecosystem: {
        id: 'ecosystem',
        name: 'Bioluminescent Cave & Wildlife Canopy',
        description: 'Subterranean crystal cluster illumination with autonomous flora and fauna',
        camera: { position: [-60, 12, -60], target: [-50, 10, -50], fov: 60 },
      },
    },
    biomes: [
      'Snow Peaks',
      'Rocky Cliffs',
      'Temperate Plains',
      'Tropical Rainforest',
      'Arid Savanna',
      'Desert Dunes',
      'Hydrology River & Lake',
      'Bioluminescent Caves',
    ],
    checksum: 'sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
  };
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------

describe('Map 2 Manifest — Tier 1: Schema Conformance & 8 Canonical Views', () => {
  it('validates a complete, standard Map 2 manifest without errors', () => {
    const manifest = buildStandardMap2Manifest();
    const result = validateMap2Manifest(manifest);
    expect(result.errors).toEqual([]);
    expect(result.ok).toBe(true);
  });

  it('contains exactly all 8 required canonical views with valid metadata and camera FOVs', () => {
    const manifest = buildStandardMap2Manifest();
    const viewKeys = Object.keys(manifest.canonicalViews);

    expect(viewKeys.length).toBe(8);
    for (const expectedId of MAP2_CANONICAL_VIEW_IDS) {
      expect(viewKeys).toContain(expectedId);
      const view = manifest.canonicalViews[expectedId];
      expect(view.id).toBe(expectedId);
      expect(view.name.length).toBeGreaterThan(0);
      expect(view.description.length).toBeGreaterThan(0);
      expect(view.camera.fov).toBeGreaterThanOrEqual(30);
      expect(view.camera.fov).toBeLessThanOrEqual(90);
    }
  });

  it('lists at least 8 distinct biomes in the ecosystem inventory', () => {
    const manifest = buildStandardMap2Manifest();
    expect(manifest.biomes.length).toBeGreaterThanOrEqual(8);
    const uniqueBiomes = new Set(manifest.biomes);
    expect(uniqueBiomes.size).toBe(manifest.biomes.length);
  });
});

describe('Map 2 Manifest — Tier 2: Defect Rejection & Adversarial Invalidation', () => {
  it('rejects manifests missing canonical views or carrying incorrect mapId', () => {
    const broken = buildStandardMap2Manifest();
    broken.mapId = 'map1_legacy';
    delete (broken.canonicalViews as Record<string, unknown>).lighting;

    const result = validateMap2Manifest(broken);
    expect(result.ok).toBe(false);
    expect(result.errors.some((e) => e.includes('mapId'))).toBe(true);
    expect(result.errors.some((e) => e.includes('lighting'))).toBe(true);
  });

  it('rejects all-zero placeholder checksums and malformed sha256 signatures', () => {
    const placeholder = buildStandardMap2Manifest();
    placeholder.checksum = `sha256:${'0'.repeat(64)}`;

    const result1 = validateMap2Manifest(placeholder);
    expect(result1.ok).toBe(false);
    expect(result1.errors.some((e) => e.includes('placeholder'))).toBe(true);

    const malformed = buildStandardMap2Manifest();
    malformed.checksum = 'md5:invalid_hash';
    const result2 = validateMap2Manifest(malformed);
    expect(result2.ok).toBe(false);
  });

  it('rejects canonical views with out-of-bounds FOV (<30 or >90)', () => {
    const invalidFov = buildStandardMap2Manifest();
    invalidFov.canonicalViews.overview.camera.fov = 120;

    const result = validateMap2Manifest(invalidFov);
    expect(result.ok).toBe(false);
    expect(result.errors.some((e) => e.includes('fov'))).toBe(true);
  });
});

describe('Map 2 Manifest — Tier 3: Physical Spawn Clearance & Target Realism', () => {
  it('guarantees player spawn point is within world bounds and above sea level', () => {
    const manifest = buildStandardMap2Manifest();
    const [sx, sy, sz] = manifest.spawnPoint.position;
    const { bounds } = manifest;

    expect(sx).toBeGreaterThanOrEqual(bounds.minX);
    expect(sx).toBeLessThanOrEqual(bounds.maxX);
    expect(sz).toBeGreaterThanOrEqual(bounds.minZ);
    expect(sz).toBeLessThanOrEqual(bounds.maxZ);
    expect(sy).toBeGreaterThan(4.0); // Strictly above water level (4.0)
  });

  it('verifies all canonical camera targets are situated inside world bounds', () => {
    const manifest = buildStandardMap2Manifest();
    const { bounds } = manifest;

    for (const viewId of MAP2_CANONICAL_VIEW_IDS) {
      const target = manifest.canonicalViews[viewId].camera.target;
      expect(target[0], `${viewId} target X`).toBeGreaterThanOrEqual(bounds.minX);
      expect(target[0], `${viewId} target X`).toBeLessThanOrEqual(bounds.maxX);
      expect(target[2], `${viewId} target Z`).toBeGreaterThanOrEqual(bounds.minZ);
      expect(target[2], `${viewId} target Z`).toBeLessThanOrEqual(bounds.maxZ);
    }
  });
});
