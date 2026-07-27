// Types for `upstream_licenses.mjs`.
//
// Hand-written for the same reason as `licensing.d.mts`: the module is `.mjs`, run directly by
// `node` with no build step, and `tests/frontend/upstreamLicenses.test.ts` exercises it under the
// tests package's `strict` typecheck without an `any` or a cast anywhere.
//
// Keep the two in step. A signature that drifts here is a test asserting against a shape the
// implementation does not have, which is worse than no test at all.

export declare const STORE_REL: string;
export declare const MANIFEST_REL: string;
export declare const SUPPORTED_SCHEMA_VERSION: number;
export declare const MATERIAL_TYPES: ReadonlySet<string>;
export declare const PROVENANCE_KINDS: ReadonlySet<string>;
export declare const MUTABLE_REFS: ReadonlySet<string>;

export declare function sha256hex(buf: Buffer | string): string;

export interface RawUrlParts {
  owner: string;
  repo: string;
  commit: string;
  pathInRepo: string;
}
export declare function parseRawUrl(url: string): RawUrlParts | null;
export declare function isSafeStorePath(id: string): boolean;
export declare function trackedStoreFiles(root: string): Set<string>;

export type MaterialType = 'licence-text' | 'notice' | 'licence-statement';
export type ProvenanceKind = 'release-tree' | 'project-repository';

export interface UpstreamSource {
  id: string;
  url: string;
  repository: string;
  commit: string;
  pathInRepo: string;
  filename: string;
  materialType: MaterialType;
  spdx: string | null;
  bytes: number;
  sha256: string;
  retrieved: string;
  /** The vendored bytes, read during validation so nothing re-reads a file it did not hash. */
  raw: Buffer;
}

export interface UpstreamProvenance {
  kind: ProvenanceKind;
  repository: string;
  commit: string;
  tag: string | null;
  evidence: string[];
  /** `project-repository` only: the component's own repository, and the commit it was released from. */
  componentRepository?: string;
  componentCommit?: string;
  componentTag?: string;
  justification?: string;
}

export interface UpstreamComponent {
  purl: string;
  name: string;
  version: string;
  ecosystem: 'cargo' | 'npm';
  declaredSpdx: string;
  provenance: UpstreamProvenance;
  sources: string[];
  /** Why the material vendored here is what upstream publishes, where that needs saying. */
  material?: string;
  resolvedSources: UpstreamSource[];
}

export interface BlockedComponent {
  purl: string;
  name: string;
  version: string;
  ecosystem: 'cargo' | 'npm';
  declaredSpdx: string;
  repository: string;
  /** The revision that was searched, so "not found" is a statement about a specific tree. */
  commit: string;
  tag: string | null;
  investigated: string;
  evidence: string[];
}

export interface UpstreamStore {
  sources: Map<string, UpstreamSource>;
  byPurl: Map<string, UpstreamComponent>;
  blockedByPurl: Map<string, BlockedComponent>;
}

export declare function loadUpstreamStore(
  root: string,
  options?: { tracked?: Set<string> },
): UpstreamStore;

export declare function storeProvenancePath(id: string): string;
export declare function storeFilePath(root: string, id: string): string;
