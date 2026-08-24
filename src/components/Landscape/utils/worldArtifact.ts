// Neutral, language-agnostic World Artifact codec — the SHARED source of truth that lets the rich
// frontend generator (worldGen.ts, 22 biomes) hand its authoritative world to the Rust backend
// simulation (src-tauri/src/core/world_artifact.rs), so agents live in the SAME world that is
// rendered instead of the two sides generating disconnected worlds (see WORLD_DESIGN.md, direction
// A→B). The byte layout MUST stay identical to the Rust module; a committed fixture + a cross-language
// cargo test guard against drift.
//
//   offset  type       field
//   0       u8[4]      magic = "ANMW"
//   4       u32 LE     version
//   8       u32 LE     width
//   12      u32 LE     height
//   16      f32 LE     seaLevel
//   20      u32 LE     seed              (world identity)
//   24      u32 LE     generatorVersion  (which generator produced this world)
//   28      f32 LE     worldScale        (world-units per axis; canonical 200 = MapBounds extent)
//   32      u32 LE     checksum          (FNV-1a 32 of the field payload, offset 36..end)
//   36      f32[n] LE  elevation         (n = width*height)
//   ...     f32[n] LE  moisture
//   ...     f32[n] LE  temperature
//   ...     f32[n] LE  flow
//   ...     u8[n]      biome             (frontend 22-biome enum indices)
//
// v2 added the seed / generatorVersion / worldScale metadata and the checksum integrity field on top
// of the v1 layout. A v1 artifact is rejected by the v2 decoder ("regenerate the world"); worlds are
// cheap to regenerate from their seed so there is no in-place migration.

export const WORLD_ARTIFACT_VERSION = 2;
export const WORLD_ARTIFACT_HEADER_LEN = 36;
/** World-units per axis — must match sim_rules WORLD_MAX_XZ - WORLD_MIN_XZ (200) and the Rust CANONICAL_WORLD_SCALE. */
export const CANONICAL_WORLD_SCALE = 200;
const MAGIC = [0x41, 0x4e, 0x4d, 0x57] as const; // "ANMW"

/**
 * FNV-1a 32-bit hash over a byte slice. `Math.imul` gives the 32-bit wrapping multiply that matches
 * Rust's `u32::wrapping_mul`, so this is byte-identical to `world_artifact::fnv1a_32` — a checksum
 * computed here verifies on the backend and vice-versa.
 */
export function fnv1a32(bytes: Uint8Array): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < bytes.length; i++) {
    hash ^= bytes[i];
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

export interface WorldArtifactData {
  width: number;
  height: number;
  seaLevel: number;
  seed: number;
  generatorVersion: number;
  worldScale: number;
  elevation: Float32Array;
  moisture: Float32Array;
  temperature: Float32Array;
  flow: Float32Array;
  biome: Uint8Array;
}

/**
 * Encode a world into the neutral binary artifact. All JS engines are little-endian, and the field
 * offsets are 4-byte aligned, so typed-array copies produce the exact LE layout the Rust side reads.
 * The payload checksum is computed after the arrays are written.
 */
export function encodeWorldArtifact(w: WorldArtifactData): ArrayBuffer {
  const n = w.width * w.height;
  const buf = new ArrayBuffer(WORLD_ARTIFACT_HEADER_LEN + n * 4 * 4 + n);
  const dv = new DataView(buf);
  dv.setUint8(0, MAGIC[0]);
  dv.setUint8(1, MAGIC[1]);
  dv.setUint8(2, MAGIC[2]);
  dv.setUint8(3, MAGIC[3]);
  dv.setUint32(4, WORLD_ARTIFACT_VERSION, true);
  dv.setUint32(8, w.width, true);
  dv.setUint32(12, w.height, true);
  dv.setFloat32(16, w.seaLevel, true);
  dv.setUint32(20, w.seed >>> 0, true);
  dv.setUint32(24, w.generatorVersion >>> 0, true);
  dv.setFloat32(28, w.worldScale, true);
  // offset 32 (checksum) is filled once the payload is in place.

  let off = WORLD_ARTIFACT_HEADER_LEN;
  new Float32Array(buf, off, n).set(w.elevation.subarray(0, n));
  off += n * 4;
  new Float32Array(buf, off, n).set(w.moisture.subarray(0, n));
  off += n * 4;
  new Float32Array(buf, off, n).set(w.temperature.subarray(0, n));
  off += n * 4;
  new Float32Array(buf, off, n).set(w.flow.subarray(0, n));
  off += n * 4;
  new Uint8Array(buf, off, n).set(w.biome.subarray(0, n));

  const payload = new Uint8Array(buf, WORLD_ARTIFACT_HEADER_LEN);
  dv.setUint32(32, fnv1a32(payload), true);
  return buf;
}

/** Decode a neutral world artifact. Throws on bad magic / version / truncation / checksum mismatch. */
export function decodeWorldArtifact(buf: ArrayBuffer): WorldArtifactData {
  if (buf.byteLength < WORLD_ARTIFACT_HEADER_LEN) throw new Error('artifact too short for header');
  const dv = new DataView(buf);
  if (
    dv.getUint8(0) !== MAGIC[0] ||
    dv.getUint8(1) !== MAGIC[1] ||
    dv.getUint8(2) !== MAGIC[2] ||
    dv.getUint8(3) !== MAGIC[3]
  ) {
    throw new Error('bad artifact magic (expected ANMW)');
  }
  const version = dv.getUint32(4, true);
  if (version !== WORLD_ARTIFACT_VERSION) {
    throw new Error(
      `unsupported artifact version ${version} (expected ${WORLD_ARTIFACT_VERSION}); regenerate the world`,
    );
  }
  const width = dv.getUint32(8, true);
  const height = dv.getUint32(12, true);
  const seaLevel = dv.getFloat32(16, true);
  const seed = dv.getUint32(20, true);
  const generatorVersion = dv.getUint32(24, true);
  const worldScale = dv.getFloat32(28, true);
  const checksum = dv.getUint32(32, true);
  const n = width * height;
  const payloadLen = n * 4 * 4 + n;
  const expected = WORLD_ARTIFACT_HEADER_LEN + payloadLen;
  if (buf.byteLength < expected) throw new Error('artifact truncated (arrays)');
  // Reject trailing bytes so a padded/corrupt buffer is not silently accepted (kept identical to the
  // Rust decoder's exact-length check).
  if (buf.byteLength > expected) {
    throw new Error(`artifact has ${buf.byteLength - expected} trailing byte(s) after the payload`);
  }

  const payload = new Uint8Array(buf, WORLD_ARTIFACT_HEADER_LEN, payloadLen);
  const actual = fnv1a32(payload);
  if (actual !== checksum) {
    throw new Error(`artifact checksum mismatch (header ${checksum}, computed ${actual}); world data is corrupt`);
  }

  let off = WORLD_ARTIFACT_HEADER_LEN;
  const take = (): Float32Array => {
    const a = new Float32Array(buf.slice(off, off + n * 4));
    off += n * 4;
    return a;
  };
  const elevation = take();
  const moisture = take();
  const temperature = take();
  const flow = take();
  const biome = new Uint8Array(buf.slice(off, off + n));
  return { width, height, seaLevel, seed, generatorVersion, worldScale, elevation, moisture, temperature, flow, biome };
}

/** The per-cell fields + identity of a generated `World` that the shared artifact carries. */
export interface WorldLike {
  size: number;
  seed: number;
  version: number;
  seaLevel: number;
  elevation: Float32Array;
  moisture: Float32Array;
  temperature: Float32Array;
  flow: Float32Array;
  biome: Uint8Array;
}

/**
 * Encode a generated `World` into the shared artifact, carrying its identity (seed, generator
 * version) so a consumer can prove which world it is. The render world is huge (e.g. 2048²); the
 * simulation only needs a modest grid, so pass `targetSize` to nearest-neighbour downsample to a
 * compact sim-resolution artifact before encoding (keeps the payload small and the sim cheap). The
 * backend downsamples again to its own working resolution.
 */
export function worldToArtifact(w: WorldLike, targetSize?: number): ArrayBuffer {
  const src = w.size;
  const t = Math.max(1, Math.min(targetSize ?? src, src));
  if (t === src) {
    return encodeWorldArtifact({
      width: src,
      height: src,
      seaLevel: w.seaLevel,
      seed: w.seed,
      generatorVersion: w.version,
      worldScale: CANONICAL_WORLD_SCALE,
      elevation: w.elevation,
      moisture: w.moisture,
      temperature: w.temperature,
      flow: w.flow,
      biome: w.biome,
    });
  }
  const n = t * t;
  const elevation = new Float32Array(n);
  const moisture = new Float32Array(n);
  const temperature = new Float32Array(n);
  const flow = new Float32Array(n);
  const biome = new Uint8Array(n);
  for (let ty = 0; ty < t; ty++) {
    const sy = Math.min(src - 1, Math.floor(((ty + 0.5) * src) / t));
    for (let tx = 0; tx < t; tx++) {
      const sx = Math.min(src - 1, Math.floor(((tx + 0.5) * src) / t));
      const si = sy * src + sx;
      const ti = ty * t + tx;
      elevation[ti] = w.elevation[si];
      moisture[ti] = w.moisture[si];
      temperature[ti] = w.temperature[si];
      flow[ti] = w.flow[si];
      biome[ti] = w.biome[si];
    }
  }
  return encodeWorldArtifact({
    width: t,
    height: t,
    seaLevel: w.seaLevel,
    seed: w.seed,
    generatorVersion: w.version,
    worldScale: CANONICAL_WORLD_SCALE,
    elevation,
    moisture,
    temperature,
    flow,
    biome,
  });
}
