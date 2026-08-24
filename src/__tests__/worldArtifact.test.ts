import { describe, it, expect } from 'vitest';
import {
  encodeWorldArtifact,
  decodeWorldArtifact,
  worldToArtifact,
  fnv1a32,
  WORLD_ARTIFACT_HEADER_LEN,
  WORLD_ARTIFACT_VERSION,
  CANONICAL_WORLD_SCALE,
  type WorldArtifactData,
  type WorldLike,
} from '../components/Landscape/utils/worldArtifact';

function tinyWorld(): WorldArtifactData {
  const n = 16; // 4x4, all values f32-exact so this matches the Rust fixture byte-for-byte
  const elevation = new Float32Array(n);
  const moisture = new Float32Array(n);
  const temperature = new Float32Array(n);
  const flow = new Float32Array(n);
  const biome = new Uint8Array(n);
  for (let i = 0; i < n; i++) {
    elevation[i] = i * 0.5;
    moisture[i] = i * 0.25;
    temperature[i] = i * 0.125;
    flow[i] = i;
    biome[i] = i;
  }
  // seed/generatorVersion/worldScale mirror the Rust `tiny_artifact()` metadata exactly.
  return {
    width: 4,
    height: 4,
    seaLevel: 0.5,
    seed: 1337,
    generatorVersion: 20,
    worldScale: CANONICAL_WORLD_SCALE,
    elevation,
    moisture,
    temperature,
    flow,
    biome,
  };
}

describe('worldArtifact codec (shared frontend↔backend format)', () => {
  it('round-trips a world exactly, including identity metadata', () => {
    const w = tinyWorld();
    const buf = encodeWorldArtifact(w);
    expect(buf.byteLength).toBe(WORLD_ARTIFACT_HEADER_LEN + 16 * 17);
    const d = decodeWorldArtifact(buf);
    expect(d.width).toBe(4);
    expect(d.height).toBe(4);
    expect(d.seaLevel).toBe(0.5);
    expect(d.seed).toBe(1337);
    expect(d.generatorVersion).toBe(20);
    expect(d.worldScale).toBe(CANONICAL_WORLD_SCALE);
    expect(Array.from(d.elevation)).toEqual(Array.from(w.elevation));
    expect(Array.from(d.moisture)).toEqual(Array.from(w.moisture));
    expect(Array.from(d.temperature)).toEqual(Array.from(w.temperature));
    expect(Array.from(d.flow)).toEqual(Array.from(w.flow));
    expect(Array.from(d.biome)).toEqual(Array.from(w.biome));
  });

  it('writes the ANMW magic, the v2 tag and a stored checksum', () => {
    const buf = encodeWorldArtifact(tinyWorld());
    const b = new Uint8Array(buf);
    expect([b[0], b[1], b[2], b[3]]).toEqual([0x41, 0x4e, 0x4d, 0x57]); // "ANMW"
    const dv = new DataView(buf);
    expect(dv.getUint32(4, true)).toBe(WORLD_ARTIFACT_VERSION); // version = 2
    expect(dv.getUint32(8, true)).toBe(4); // width
    // The stored checksum equals FNV-1a over the field payload (offset 36..end).
    const payload = new Uint8Array(buf, WORLD_ARTIFACT_HEADER_LEN);
    expect(dv.getUint32(32, true)).toBe(fnv1a32(payload));
  });

  it('rejects a corrupt magic', () => {
    const buf = encodeWorldArtifact(tinyWorld());
    new Uint8Array(buf)[0] = 0x58; // 'X'
    expect(() => decodeWorldArtifact(buf)).toThrow(/magic/);
  });

  it('rejects a flipped payload byte via the checksum (S06 integrity)', () => {
    const buf = encodeWorldArtifact(tinyWorld());
    const b = new Uint8Array(buf);
    b[b.length - 1] ^= 0xff; // corrupt a biome byte
    expect(() => decodeWorldArtifact(buf)).toThrow(/checksum/);
  });

  it('rejects an old artifact version with a regenerate hint (S09)', () => {
    const buf = encodeWorldArtifact(tinyWorld());
    new DataView(buf).setUint32(4, 1, true); // pretend it is a v1 artifact
    expect(() => decodeWorldArtifact(buf)).toThrow(/version.*regenerate|regenerate/);
  });

  it('worldToArtifact encodes a World (and downsamples) into a decodable artifact', () => {
    const src = 4;
    const n = src * src;
    const fill = (f: (i: number) => number) => {
      const a = new Float32Array(n);
      for (let i = 0; i < n; i++) a[i] = f(i);
      return a;
    };
    const biome = new Uint8Array(n);
    for (let i = 0; i < n; i++) biome[i] = i % 22;
    const world: WorldLike = {
      size: src,
      seed: 42,
      version: 20,
      seaLevel: 0.3,
      elevation: fill((i) => i * 0.125),
      moisture: fill((i) => i * 0.0625),
      temperature: fill((i) => i * 0.03125),
      flow: fill((i) => i * 0.25),
      biome,
    };

    const full = decodeWorldArtifact(worldToArtifact(world));
    expect(full.width).toBe(4);
    expect(full.seed).toBe(42);
    expect(full.generatorVersion).toBe(20);
    expect(Array.from(full.biome)).toEqual(Array.from(biome));

    const down = decodeWorldArtifact(worldToArtifact(world, 2));
    expect(down.width).toBe(2);
    expect(down.height).toBe(2);
    expect(down.seaLevel).toBeCloseTo(0.3);
    expect(down.seed).toBe(42); // identity survives downsampling
    expect(down.biome.length).toBe(4);
    expect(down.elevation.length).toBe(4);
  });
});
