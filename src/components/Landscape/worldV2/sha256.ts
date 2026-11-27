import { TileIntegrityError, type TileManifestEntry } from './types';

function toHex(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let hex = '';
  for (let i = 0; i < bytes.length; i += 1) {
    hex += bytes[i].toString(16).padStart(2, '0');
  }
  return hex;
}

/**
 * Computes lowercase SHA-256 hex digest using WebCrypto (`SubtleCrypto`).
 * Throws loudly if `crypto.subtle` is unavailable rather than skipping verification.
 */
export async function computeSha256Hex(bytes: Uint8Array): Promise<string> {
  const subtle = globalThis.crypto?.subtle;
  if (!subtle) {
    throw new Error('WebCrypto subtle.digest is unavailable; cannot verify World v2 tile SHA-256.');
  }
  const copy = new Uint8Array(bytes.byteLength);
  copy.set(bytes);
  const digest = await subtle.digest('SHA-256', copy.buffer);
  return toHex(digest);
}

/**
 * Verifies tile byte length and WebCrypto SHA-256 against `index.json`.
 * Throws `TileIntegrityError` on any mismatch; never falls back silently.
 */
export async function verifyTileBytes(
  entry: Pick<TileManifestEntry, 'path' | 'byte_len' | 'sha256'>,
  bytes: Uint8Array,
): Promise<string> {
  const expected = entry.sha256.toLowerCase();
  const actual = await computeSha256Hex(bytes);
  if (bytes.byteLength !== entry.byte_len) {
    throw new TileIntegrityError(
      `Tile ${entry.path} byte length mismatch: expected ${entry.byte_len}, got ${bytes.byteLength} (sha256=${actual})`,
      entry.path,
      expected,
      actual,
    );
  }
  if (actual !== expected) {
    throw new TileIntegrityError(
      `Tile ${entry.path} SHA-256 mismatch: expected ${expected}, got ${actual}`,
      entry.path,
      expected,
      actual,
    );
  }
  return actual;
}
