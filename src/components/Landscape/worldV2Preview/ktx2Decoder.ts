import { unzlibSync } from 'three/examples/jsm/libs/fflate.module.js';
import {
  ELEV_MIN_M,
  ELEV_SPAN_M,
  TILE_ACTIVE_SIZE,
  TILE_GUTTER,
  TILE_TOTAL_SIZE,
  type DecodedKtx2,
  type Ktx2LevelEntry,
} from './types';

export const KTX2_MAGIC = new Uint8Array([
  0xab, 0x4b, 0x54, 0x58, 0x20, 0x32, 0x30, 0xbb, 0x0d, 0x0a, 0x1a, 0x0a,
]);
export const KTX2_HEADER_LEN = 80;
export const KTX2_LEVEL_LEN = 24;

export const VK_FORMAT_R8_UNORM = 9;
export const VK_FORMAT_R8G8_UNORM = 16;
export const VK_FORMAT_R8G8B8A8_UNORM = 37;
export const VK_FORMAT_R8G8B8A8_SRGB = 43;
export const VK_FORMAT_R16_UNORM = 70;
export const VK_FORMAT_R16G16_UNORM = 77;
export const VK_FORMAT_BC5_UNORM_BLOCK = 141;

export const KTX2_SUPERCOMPRESSION_NONE = 0;
export const KTX2_SUPERCOMPRESSION_ZLIB = 3;

const R16_MAX = 65535.0;

function readU32(view: DataView, offset: number): number {
  return view.getUint32(offset, true);
}

function readU64Safe(view: DataView, offset: number): number {
  const lo = view.getUint32(offset, true);
  const hi = view.getUint32(offset + 4, true);
  const val = hi * 0x100000000 + lo;
  if (!Number.isSafeInteger(val)) {
    throw new Error(`KTX2 u64 overflow at offset ${offset}`);
  }
  return val;
}

/**
 * Parses KTX2 v2.0 header and level table, then decompresses mip level 0 (`NONE` or `ZLIB`)
 * matching `src-tauri/crates/anima-worldgen/src/bake/pack_ktx2.rs:647-767`.
 */
export function decodeKtx2Level0(bytes: Uint8Array): DecodedKtx2 {
  if (bytes.byteLength < KTX2_HEADER_LEN) {
    throw new Error(`KTX2 truncated header: ${bytes.byteLength} bytes`);
  }
  for (let i = 0; i < KTX2_MAGIC.length; i += 1) {
    if (bytes[i] !== KTX2_MAGIC[i]) {
      throw new Error('Invalid KTX2 magic header');
    }
  }

  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const vkFormat = readU32(view, 12);
  const typeSize = readU32(view, 16);
  const width = readU32(view, 20);
  const height = readU32(view, 24);
  const levelCountRaw = readU32(view, 40);
  const levelCount = Math.max(1, levelCountRaw);
  const supercompressionScheme = readU32(view, 44);

  const tableEnd = KTX2_HEADER_LEN + levelCount * KTX2_LEVEL_LEN;
  if (bytes.byteLength < tableEnd) {
    throw new Error(`KTX2 truncated level index: need ${tableEnd}, got ${bytes.byteLength}`);
  }

  const levels: Ktx2LevelEntry[] = [];
  for (let m = 0; m < levelCount; m += 1) {
    const off = KTX2_HEADER_LEN + m * KTX2_LEVEL_LEN;
    const byteOffset = readU64Safe(view, off);
    const byteLength = readU64Safe(view, off + 8);
    const uncompressedByteLength = readU64Safe(view, off + 16);
    const end = byteOffset + byteLength;
    if (end > bytes.byteLength || byteLength === 0 || uncompressedByteLength === 0) {
      throw new Error(`KTX2 mip ${m} byte range invalid (${byteOffset}..${end})`);
    }
    levels.push({ byteOffset, byteLength, uncompressedByteLength });
  }

  const level0Entry = levels[0];
  const raw = bytes.subarray(level0Entry.byteOffset, level0Entry.byteOffset + level0Entry.byteLength);

  let level0: Uint8Array;
  if (supercompressionScheme === KTX2_SUPERCOMPRESSION_NONE) {
    level0 = new Uint8Array(raw);
  } else if (supercompressionScheme === KTX2_SUPERCOMPRESSION_ZLIB) {
    level0 = unzlibSync(raw);
  } else {
    throw new Error(`Unsupported KTX2 supercompression scheme: ${supercompressionScheme}`);
  }

  if (level0.byteLength !== level0Entry.uncompressedByteLength) {
    throw new Error(
      `KTX2 mip 0 decompressed length mismatch: expected ${level0Entry.uncompressedByteLength}, got ${level0.byteLength}`,
    );
  }

  return {
    vkFormat,
    typeSize,
    width,
    height,
    levelCount,
    supercompressionScheme,
    level0,
  };
}

function bc4Palette(a0: number, a1: number): Uint8Array {
  const p = new Uint8Array(8);
  p[0] = a0;
  p[1] = a1;
  if (a0 > a1) {
    for (let c = 2; c < 8; c += 1) {
      p[c] = Math.trunc(((8 - c) * a0 + (c - 1) * a1 + 3) / 7);
    }
  } else {
    for (let c = 2; c < 6; c += 1) {
      p[c] = Math.trunc(((6 - c) * a0 + (c - 1) * a1 + 2) / 5);
    }
    p[6] = 0;
    p[7] = 255;
  }
  return p;
}

function decodeBc4Block(blocks: Uint8Array, offset: number, dst: Uint8Array): void {
  const palette = bc4Palette(blocks[offset], blocks[offset + 1]);
  let bits = 0n;
  for (let b = 0; b < 6; b += 1) {
    bits |= BigInt(blocks[offset + 2 + b]) << BigInt(8 * b);
  }
  for (let i = 0; i < 16; i += 1) {
    const code = Number((bits >> BigInt(3 * i)) & 7n);
    dst[i] = palette[code];
  }
}

/**
 * Decodes BC5 UNORM blocks (`VK_FORMAT_BC5_UNORM_BLOCK = 141`) into row-major `[r, g]` `Uint8Array`.
 * Matches `decode_bc5_unorm` in `pack_ktx2.rs:235-272`.
 */
export function decodeBc5Unorm(blocks: Uint8Array, width: number, height: number): Uint8Array {
  if (width % 4 !== 0 || height % 4 !== 0) {
    throw new Error(`BC5 decode requires width and height divisible by 4 (got ${width}x${height})`);
  }
  const bxCnt = width / 4;
  const byCnt = height / 4;
  const expected = bxCnt * byCnt * 16;
  if (blocks.byteLength !== expected) {
    throw new Error(`BC5 byte length mismatch: expected ${expected}, got ${blocks.byteLength}`);
  }

  const out = new Uint8Array(width * height * 2);
  const rDec = new Uint8Array(16);
  const gDec = new Uint8Array(16);
  let off = 0;

  for (let by = 0; by < byCnt; by += 1) {
    for (let bx = 0; bx < bxCnt; bx += 1) {
      decodeBc4Block(blocks, off, rDec);
      decodeBc4Block(blocks, off + 8, gDec);
      off += 16;
      for (let ly = 0; ly < 4; ly += 1) {
        const row = ((by * 4 + ly) * width + bx * 4) * 2;
        for (let lx = 0; lx < 4; lx += 1) {
          const i = ly * 4 + lx;
          out[row + lx * 2] = rDec[i];
          out[row + lx * 2 + 1] = gDec[i];
        }
      }
    }
  }
  return out;
}

/**
 * Decodes `VK_FORMAT_R16_UNORM` tile texels into elevation in metres `[-1500, +5700]`.
 */
export function decodeR16HeightM(raw: Uint8Array, width: number, height: number): Float32Array {
  const count = width * height;
  if (raw.byteLength !== count * 2) {
    throw new Error(`R16 height length mismatch: expected ${count * 2}, got ${raw.byteLength}`);
  }
  const view = new DataView(raw.buffer, raw.byteOffset, raw.byteLength);
  const out = new Float32Array(count);
  for (let i = 0; i < count; i += 1) {
    const q = view.getUint16(i * 2, true);
    out[i] = ELEV_MIN_M + (q / R16_MAX) * ELEV_SPAN_M;
  }
  return out;
}

/**
 * Decodes `VK_FORMAT_R16G16_UNORM` water surface texels into `(waterHeightM, waterKind)`.
 */
export function decodeWaterR16G16(
  raw: Uint8Array,
  width: number,
  height: number,
): { waterHeightM: Float32Array; waterKind: Uint8Array } {
  const count = width * height;
  if (raw.byteLength !== count * 4) {
    throw new Error(`R16G16 water length mismatch: expected ${count * 4}, got ${raw.byteLength}`);
  }
  const view = new DataView(raw.buffer, raw.byteOffset, raw.byteLength);
  const waterHeightM = new Float32Array(count);
  const waterKind = new Uint8Array(count);
  for (let i = 0; i < count; i += 1) {
    const hQ = view.getUint16(i * 4, true);
    const wk = view.getUint16(i * 4 + 2, true);
    waterHeightM[i] = ELEV_MIN_M + (hQ / R16_MAX) * ELEV_SPAN_M;
    waterKind[i] = Math.min(255, wk);
  }
  return { waterHeightM, waterKind };
}

/**
 * Copies active `256 x 256` texels from a `260 x 260` gutter-padded tile into a stitched grid.
 */
export function copyActiveTilePatch(
  src: Float32Array | Uint8Array,
  dst: Float32Array | Uint8Array,
  channels: number,
  dstGridWidth: number,
  tx: number,
  ty: number,
  totalSize = TILE_TOTAL_SIZE,
  activeSize = TILE_ACTIVE_SIZE,
  gutter = TILE_GUTTER,
): void {
  const expectedSrcLen = totalSize * totalSize * channels;
  if (src.length !== expectedSrcLen) {
    throw new Error(`Tile buffer size mismatch: expected ${expectedSrcLen}, got ${src.length}`);
  }
  for (let r = 0; r < activeSize; r += 1) {
    const srcStart = ((r + gutter) * totalSize + gutter) * channels;
    const srcEnd = srcStart + activeSize * channels;
    const dstStart = ((ty * activeSize + r) * dstGridWidth + tx * activeSize) * channels;
    for (let k = 0; k < srcEnd - srcStart; k += 1) {
      dst[dstStart + k] = src[srcStart + k];
    }
  }
}
