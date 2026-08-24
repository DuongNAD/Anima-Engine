export declare const TEXT_EXTENSIONS: ReadonlySet<string>;

/** Byte offsets of disallowed control characters, capped at `limit`. TAB, LF and CR are text. */
export declare function findControlByteOffsets(bytes: Uint8Array, limit?: number): number[];
