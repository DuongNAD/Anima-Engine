// Types for the importable half of `check_text_hygiene.mjs`. See `lib/licensing.d.mts` for why
// these declarations are hand-written.

export declare const TEXT_EXTENSIONS: ReadonlySet<string>;

/** Byte offsets of disallowed control characters, capped at `limit`. TAB, LF and CR are text. */
export declare function findControlByteOffsets(bytes: Uint8Array, limit?: number): number[];
