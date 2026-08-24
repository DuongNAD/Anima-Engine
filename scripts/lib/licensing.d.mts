// Types for `licensing.mjs`.
//
// Hand-written rather than generated, because the module is `.mjs` — the same form as every other
// generator script in `scripts/`, run directly by `node` with no build step. The declaration exists
// so `tests/frontend/thirdPartyLicenses.test.ts` can exercise the pure helpers under the tests
// package's `strict` typecheck without an `any` or a cast anywhere.
//
// Keep the two in step: a signature that drifts here is a test asserting against a shape the
// implementation does not have, which is worse than no test at all.

export declare function isLicenseFileName(name: string): boolean;
export declare const SCHEMA_DIR: string;
export declare const SPDX_IDS: ReadonlySet<string>;

export declare function sha256hex(buf: Buffer | string): string;
export declare function byteCompare(a: string, b: string): number;

/** The subset of `fs.Dirent` this predicate reads, so a test can build one by hand. */
export interface LicenseDirEntry {
  name: string;
  isFile(): boolean;
}
export declare function isSafeLicenseEntry(entry: LicenseDirEntry): boolean;

export declare function decodeLicenseText(buf: Buffer): { text: string; lossless: boolean };

export interface LicenseText {
  filename: string;
  sourceSha256: string;
  sourceBytes: number;
  text: string;
  textSha256: string;
  lossless: boolean;
}
export declare function collectLicenseTexts(dir: string): LicenseText[];

export type Ecosystem = 'cargo' | 'npm';
export type ComponentOrigin = 'cargo-registry' | 'node_modules' | 'toolchain-injected';

export interface InventoryComponent {
  ecosystem: Ecosystem;
  name: string;
  version: string;
  purl: string;
  declaredLicense: string | null;
  declaredLicenseFile: string | null;
  description: string | null;
  repository: string | null;
  distributed: boolean;
  origin: ComponentOrigin;
  sourceDir: string | null;
}

export interface Inventory {
  components: InventoryComponent[];
  /** `name@version` -> its `name@version` dependencies, byte-sorted. */
  edges: Map<string, string[]>;
  rootDeps: string[];
}

export declare function cargoInventory(root: string): Inventory;
export declare function npmInventory(root: string): Inventory;

export type LicenseChoice = { expression: string } | { license: { id: string } | { name: string } };
export declare function isSpdxExpression(value: string): boolean;
export declare function licenseNode(raw: string | null | undefined): LicenseChoice[] | undefined;

export declare function provenancePath(
  component: Pick<InventoryComponent, 'ecosystem' | 'name' | 'version' | 'sourceDir'>,
  filename: string,
  root: string,
): string;

export declare function deterministicSerialNumber(canonicalContent: string): string;
