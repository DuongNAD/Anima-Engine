import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, it } from 'vitest';
import {
  LAND_ANCHORS,
  createSyntheticTickGenerator,
  selectWorldV2LandAnchors,
  serializeSyntheticTickFixture,
  type SyntheticAnchorWorld,
} from '../../src/components/Landscape/utils/syntheticTicks';
import {
  isWorldV2Search,
  resolveTickSourceFromSearch,
} from '../../src/components/Landscape/utils/tickRecording';
import { CANONICAL_XZ_EXTENT } from '../../src/components/Landscape/utils/mapManifest';
import { decodeAnmwV3Sim } from '../../src/components/Landscape/utils/worldArtifactV3';
import { Biome } from '../../src/components/Landscape/utils/worldGen';

const HERE = dirname(fileURLToPath(import.meta.url));
const SIM_256_PATH = resolve(HERE, '../../artifacts/world-v2/sim_256.anmw3');

function sampleWorldBiome(world: SyntheticAnchorWorld, cx: number, cz: number): Biome {
  const u = Math.min(1, Math.max(0, (cx + CANONICAL_XZ_EXTENT * 0.5) / CANONICAL_XZ_EXTENT));
  const v = Math.min(1, Math.max(0, (cz + CANONICAL_XZ_EXTENT * 0.5) / CANONICAL_XZ_EXTENT));
  const gx = Math.round(u * (world.size - 1));
  const gz = Math.round(v * (world.size - 1));
  return world.biome[gz * world.size + gx] as Biome;
}

function makeEasternContinentWorld(size = 128): SyntheticAnchorWorld {
  const n = size * size;
  const elevation = new Float32Array(n);
  const biome = new Uint8Array(n);
  const slope = new Float32Array(n);
  const seaLevel = 0.25;

  for (let z = 0; z < size; z += 1) {
    for (let x = 0; x < size; x += 1) {
      const idx = z * size + x;
      const cx = (x / (size - 1) - 0.5) * CANONICAL_XZ_EXTENT;
      const cz = (z / (size - 1) - 0.5) * CANONICAL_XZ_EXTENT;
      // Western half (including all v1 LAND_ANCHORS at x < 0) is deep Ocean; eastern half is Forest/Grassland.
      if (cx > 12 && cx < 78 && cz > -65 && cz < 65) {
        elevation[idx] = 0.42;
        biome[idx] = cx < 45 ? Biome.Forest : Biome.Grassland;
        slope[idx] = 0.05;
      } else {
        elevation[idx] = 0.1;
        biome[idx] = Biome.Ocean;
        slope[idx] = 0.01;
      }
    }
  }

  return { size, elevation, biome, slope, seaLevel };
}

describe('N3 World v2 deterministic synthetic creature anchors', () => {
  afterEach(() => {
    delete (window as { __world?: unknown }).__world;
  });

  it('preserves byte-identical v1 synthetic fixtures and ignores __world when world=v1', () => {
    const v1JsonA = serializeSyntheticTickFixture({ agents: 12, ticks: 10, seed: 0x1234 });
    const v1JsonB = serializeSyntheticTickFixture({ agents: 12, ticks: 10, seed: 0x1234 });
    expect(v1JsonA).toBe(v1JsonB);

    const parsed = JSON.parse(v1JsonA) as {
      params: Record<string, unknown>;
      frames: Array<{ payload: unknown }>;
    };
    expect('anchors' in parsed.params).toBe(false);

    (window as { __world?: SyntheticAnchorWorld | null }).__world = makeEasternContinentWorld();
    const srcV1 = resolveTickSourceFromSearch('?replay=synthetic&world=v1&agents=12&ticks=10&seed=4660');
    expect(srcV1).not.toBeNull();
    let v1FirstPayload: unknown = null;
    const unsub = srcV1!.subscribe((p) => {
      v1FirstPayload = p;
    });
    unsub();

    const directGen = createSyntheticTickGenerator({ agents: 12, ticks: 10, seed: 0x1234 });
    expect(JSON.stringify(v1FirstPayload)).toBe(JSON.stringify(directGen.tickAt(0)));
  });

  it('selects inland land anchors on an eastern-land world where v1 anchors sit in ocean', () => {
    const eastWorld = makeEasternContinentWorld();
    for (const [vx, vz] of LAND_ANCHORS) {
      expect(sampleWorldBiome(eastWorld, vx, vz)).toBe(Biome.Ocean);
    }

    const anchorsA = selectWorldV2LandAnchors(eastWorld);
    const anchorsB = selectWorldV2LandAnchors(eastWorld);
    expect(anchorsA).toEqual(anchorsB);
    expect(anchorsA.length).toBe(LAND_ANCHORS.length);
    for (const [ax, az] of anchorsA) {
      const b = sampleWorldBiome(eastWorld, ax, az);
      expect(b).not.toBe(Biome.Ocean);
      expect(ax).toBeGreaterThan(12);
    }

    const gen = createSyntheticTickGenerator({ agents: 24, anchors: anchorsA });
    for (const tickIdx of [0, 45, 90, 135]) {
      const tick = gen.tickAt(tickIdx);
      const roots = tick.segments.filter((s) => s.parent_segment_id === null);
      expect(roots.length).toBe(24);
      for (const root of roots) {
        expect(sampleWorldBiome(eastWorld, root.x, root.z)).not.toBe(Biome.Ocean);
      }
    }
    for (const el of gen.tickAt(0).environmental_state.elements) {
      expect(sampleWorldBiome(eastWorld, el.x, el.y)).not.toBe(Biome.Ocean);
    }
  });

  it('places BIG_4096_v2 synthetic creatures and fixtures on terrestrial biomes instead of western sea', () => {
    const raw = readFileSync(SIM_256_PATH);
    const sim = decodeAnmwV3Sim(new Uint8Array(raw.buffer, raw.byteOffset, raw.byteLength));
    const v2World: SyntheticAnchorWorld = {
      size: sim.width,
      elevation: sim.elevation,
      biome: sim.biome22,
      slope: sim.slope,
      seaLevel: sim.seaLevelNorm,
    };

    // Baseline check: old v1 anchors mostly sit in the BIG_4096_v2 western sea.
    const oldSeaHits = LAND_ANCHORS.filter(([x, z]) => sampleWorldBiome(v2World, x, z) === Biome.Ocean).length;
    expect(oldSeaHits).toBeGreaterThan(0);

    const anchors = selectWorldV2LandAnchors(v2World);
    expect(anchors.length).toBe(LAND_ANCHORS.length);
    for (const [ax, az] of anchors) {
      const b = sampleWorldBiome(v2World, ax, az);
      expect(
        [Biome.Ocean, Biome.Lake, Biome.River, Biome.Beach, Biome.Glacier, Biome.Snow, Biome.Rock, Biome.Alpine].includes(b),
        `Anchor [${ax}, ${az}] unexpectedly hit ${Biome[b]}`,
      ).toBe(false);
    }

    (window as { __world?: SyntheticAnchorWorld | null }).__world = v2World;
    expect(isWorldV2Search('?replay=synthetic&world=v2&agents=24', 'test')).toBe(true);
    expect(isWorldV2Search('?replay=synthetic&world=v1&agents=24', 'production')).toBe(false);
    expect(isWorldV2Search('?replay=synthetic&agents=24', 'production')).toBe(true);

    const srcV2 = resolveTickSourceFromSearch('?replay=synthetic&world=v2&agents=24');
    expect(srcV2).not.toBeNull();
    let payload: ReturnType<ReturnType<typeof createSyntheticTickGenerator>['tickAt']> | null = null;
    const unsub = srcV2!.subscribe((p) => {
      payload = p;
    });
    unsub();

    expect(payload).not.toBeNull();
    const roots = payload!.segments.filter((s) => s.parent_segment_id === null);
    expect(roots.length).toBe(24);
    for (const root of roots) {
      const b = sampleWorldBiome(v2World, root.x, root.z);
      expect(b, `Root agent ${root.agent_id} at (${root.x}, ${root.z}) hit ${Biome[b]}`).not.toBe(Biome.Ocean);
      expect(b, `Root agent ${root.agent_id} at (${root.x}, ${root.z}) hit Lake`).not.toBe(Biome.Lake);
    }
    for (const el of payload!.environmental_state.elements) {
      const b = sampleWorldBiome(v2World, el.x, el.y);
      expect(b).not.toBe(Biome.Ocean);
    }
  });

  it('falls back safely to LAND_ANCHORS when world is null or all ocean', () => {
    expect(selectWorldV2LandAnchors(null)).toEqual(LAND_ANCHORS);
    expect(selectWorldV2LandAnchors(undefined)).toEqual(LAND_ANCHORS);

    const oceanOnly: SyntheticAnchorWorld = {
      size: 16,
      elevation: new Float32Array(256).fill(0.05),
      biome: new Uint8Array(256).fill(Biome.Ocean),
      seaLevel: 0.2,
    };
    expect(selectWorldV2LandAnchors(oceanOnly)).toEqual(LAND_ANCHORS);
  });
});
