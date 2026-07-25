import { describe, it, expect } from 'vitest';
import {
  validateMapManifest,
  CANONICAL_VIEW_IDS,
} from '../components/Landscape/utils/mapManifest';

// A structurally complete manifest mirroring the committed sample at repo root (map_manifest.json).
// Kept inline so this test stays self-contained and pure (no fs, no worldGen/three imports).
function validManifest(): Record<string, unknown> {
  return {
    schemaVersion: 1,
    worldArtifact: {
      path: 'artifacts/world_128.anmw',
      magic: 'ANMW',
      version: 1,
      width: 128,
      height: 128,
      seaLevel: 0.42,
      checksum: 'sha256:00',
    },
    coordinateSystem: {
      worldMinXZ: -100,
      worldMaxXZ: 100,
      worldMinY: 0,
      worldMaxY: 10,
      gridDim: 128,
    },
    biomeTaxonomy: { canonicalCount: 22, legacyCount: 11 },
    views: [
      { id: 'overview', imagePath: 'map-views/overview.png', camera: { position: [0, 95, 95], target: [0, 0, 0] } },
      { id: 'navigation', imagePath: 'map-views/navigation.png', camera: { position: [-60, 45, -60], target: [-40, 0, -40] } },
      { id: 'collision', imagePath: 'map-views/collision.png', camera: { position: [50, 30, 50], target: [30, 2, 30] } },
      { id: 'lighting', imagePath: 'map-views/lighting.png', camera: { position: [0, 80, -90], target: [0, 5, 0] } },
      { id: 'spawn', imagePath: 'map-views/spawn.png', camera: { position: [20, 25, 20], target: [10, 1, 10] } },
      { id: 'water', imagePath: 'map-views/water.png', camera: { position: [-80, 22, 12], target: [-95, 0, 0] } },
      { id: 'biome_transition', imagePath: 'map-views/biome_transition.png', camera: { position: [40, 32, -40], target: [20, 2, -20] } },
      { id: 'ecosystem', imagePath: 'map-views/ecosystem.png', camera: { position: [-30, 35, 60], target: [-10, 1, 40] } },
    ],
  };
}

/** Deep clone so each mutation test starts from a pristine valid manifest. */
function clone(m: Record<string, unknown>): Record<string, unknown> {
  return JSON.parse(JSON.stringify(m)) as Record<string, unknown>;
}

describe('validateMapManifest (S05 — map manifest schema)', () => {
  it('accepts the sample manifest with all eight canonical views', () => {
    const res = validateMapManifest(validManifest());
    expect(res.ok).toBe(true);
    expect(res.errors).toEqual([]);
  });

  it('lists exactly the eight canonical view ids', () => {
    // Guards the enum against drift with WORLD_SIMULATION_PLAN.md §13.
    expect([...CANONICAL_VIEW_IDS]).toEqual([
      'overview',
      'navigation',
      'collision',
      'lighting',
      'spawn',
      'water',
      'biome_transition',
      'ecosystem',
    ]);
  });

  it('rejects a manifest missing a required top-level field (worldArtifact)', () => {
    const m = clone(validManifest());
    delete m.worldArtifact;
    const res = validateMapManifest(m);
    expect(res.ok).toBe(false);
    expect(res.errors).toContain('missing required field: worldArtifact');
  });

  it('rejects a manifest missing required coordinateSystem / biomeTaxonomy / views / schemaVersion', () => {
    for (const field of ['coordinateSystem', 'biomeTaxonomy', 'views', 'schemaVersion']) {
      const m = clone(validManifest());
      delete m[field];
      const res = validateMapManifest(m);
      expect(res.ok).toBe(false);
      expect(res.errors).toContain(`missing required field: ${field}`);
    }
  });

  it("rejects a view whose required imagePath was removed", () => {
    const m = clone(validManifest());
    const views = m.views as Array<Record<string, unknown>>;
    delete views[0].imagePath;
    const res = validateMapManifest(m);
    expect(res.ok).toBe(false);
    expect(res.errors).toContain('views[0].imagePath must be a string');
  });

  it('rejects an unknown (non-canonical) view id', () => {
    const m = clone(validManifest());
    const views = m.views as Array<Record<string, unknown>>;
    views[2].id = 'minimap';
    const res = validateMapManifest(m);
    expect(res.ok).toBe(false);
    expect(res.errors.some((e) => e.includes('is not a canonical view id'))).toBe(true);
  });

  it('rejects a camera without a valid position/target triple', () => {
    const m = clone(validManifest());
    const views = m.views as Array<Record<string, unknown>>;
    (views[1].camera as Record<string, unknown>).position = [0, 0];
    const res = validateMapManifest(m);
    expect(res.ok).toBe(false);
    expect(res.errors).toContain('views[1].camera.position must be a [x, y, z] number triple');
  });

  it('rejects an empty views array', () => {
    const m = clone(validManifest());
    m.views = [];
    const res = validateMapManifest(m);
    expect(res.ok).toBe(false);
    expect(res.errors).toContain('views must contain at least one view');
  });

  it('rejects a non-object manifest', () => {
    expect(validateMapManifest(null).ok).toBe(false);
    expect(validateMapManifest(42).ok).toBe(false);
    expect(validateMapManifest([]).ok).toBe(false);
  });
});
