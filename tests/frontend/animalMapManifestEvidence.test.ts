import { describe, expect, it } from 'vitest';
import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  utimesSync,
} from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { WORLD_GEN_VERSION } from '@/components/Landscape/utils/worldGen';
import {
  SHARED_WORLD_SEED,
  SHARED_WORLD_SHAPE,
  SHARED_WORLD_SIZE,
} from '@/utils/sharedWorld';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '../..');
const MANIFEST_PATH = resolve(ROOT, 'animal-map.manifest.json');

function loadManifest(): Record<string, unknown> {
  expect(
    existsSync(MANIFEST_PATH),
    `animal-map.manifest.json must be committed at ${MANIFEST_PATH}; run \`npm run gen:manifest\``,
  ).toBe(true);
  return JSON.parse(readFileSync(MANIFEST_PATH, 'utf8')) as Record<string, unknown>;
}

describe('animal-map-vision manifest — the committed main-world evidence', () => {
  it('pins LF checkout bytes for the byte-for-byte freshness gate', () => {
    const result = spawnSync(
      'git',
      ['check-attr', 'eol', '--', 'animal-map.manifest.json'],
      { cwd: ROOT, encoding: 'utf8' },
    );

    expect(result.status, result.stderr).toBe(0);
    expect(result.stdout.trim()).toBe('animal-map.manifest.json: eol: lf');
  });

  it('uses the MCP schema and the exact world identity rendered by the app', () => {
    const manifest = loadManifest();
    const generated = manifest._generated as Record<string, unknown>;

    expect(manifest.schemaVersion).toBe('1.0');
    expect(generated.seed).toBe(SHARED_WORLD_SEED);
    expect(generated.size).toBe(SHARED_WORLD_SIZE);
    expect(generated.shape).toBe(SHARED_WORLD_SHAPE);
    expect(generated.worldGenVersion).toBe(WORLD_GEN_VERSION);
  });

  it('publishes complete, internally unique map evidence', () => {
    const manifest = loadManifest();
    const map = manifest.map as Record<string, unknown>;
    const biomes = map.biomes as Array<Record<string, unknown>>;
    const entities = manifest.entities as Array<Record<string, unknown>>;
    const navigation = manifest.navigation as Record<string, unknown>;
    const nodes = navigation.nodes as Array<Record<string, unknown>>;
    const edges = navigation.edges as Array<Record<string, unknown>>;

    expect(biomes).toHaveLength(22);
    expect(new Set(biomes.map((biome) => biome.id)).size).toBe(biomes.length);
    expect(new Set(entities.map((entity) => entity.id)).size).toBe(entities.length);
    expect(Number(navigation.navmeshCoverage)).toBeGreaterThan(0);
    expect(nodes.length).toBeGreaterThanOrEqual(2);
    expect(edges.length).toBeGreaterThanOrEqual(1);
  });

  it('is byte-current with a fresh deterministic export', () => {
    const temp = mkdtempSync(join(tmpdir(), 'anima-map-manifest-check-'));
    const candidate = resolve(temp, 'animal-map.manifest.json');
    copyFileSync(MANIFEST_PATH, candidate);
    const oldTimestamp = new Date('2000-01-01T00:00:00.000Z');
    utimesSync(candidate, oldTimestamp, oldTimestamp);
    const before = readFileSync(candidate);
    const beforeMtime = statSync(candidate).mtimeMs;

    try {
      const result = spawnSync(
        process.execPath,
        [
          resolve(ROOT, 'scripts/run_ts.mjs'),
          resolve(ROOT, 'scripts/gen_map_manifest.ts'),
          '--check',
        ],
        { cwd: temp, encoding: 'utf8' },
      );

      expect(
        result.status,
        `manifest freshness check failed:\n${result.stdout}\n${result.stderr}`,
      ).toBe(0);
      expect(
        readFileSync(candidate).equals(before),
        'check mode must never rewrite the evidence file',
      ).toBe(true);
      expect(statSync(candidate).mtimeMs, 'check mode must not open the evidence file for writing').toBe(
        beforeMtime,
      );
    } finally {
      rmSync(temp, { recursive: true, force: true });
    }
  }, 120_000);
});
