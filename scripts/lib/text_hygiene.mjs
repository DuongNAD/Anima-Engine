export const TEXT_EXTENSIONS = new Set([
  '.mjs', '.js', '.cjs', '.ts', '.tsx', '.rs', '.json', '.md', '.yml', '.yaml', '.toml', '.html',
  '.css', '.txt', '.sh', '.ps1', '.py',
]);

const ALLOWED = new Set([0x09, 0x0a, 0x0d]); // TAB, LF, CR

/**
 * Byte offsets of the disallowed control characters in a buffer, up to `limit`.
 *
 * Kept in a shebang-free module so Vitest and other ESM consumers can import the detector without
 * asking their transform pipeline to parse the executable `check_text_hygiene.mjs` entry point.
 */
export function findControlByteOffsets(bytes, limit = 5) {
  const offsets = [];
  for (let i = 0; i < bytes.length; i++) {
    const b = bytes[i];
    if ((b < 0x20 && !ALLOWED.has(b)) || b === 0x7f) {
      offsets.push(i);
      if (offsets.length >= limit) break;
    }
  }
  return offsets;
}
