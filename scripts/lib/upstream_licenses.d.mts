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
export declare const ELECTABLE_LICENCES: ReadonlySet<string>;
export declare const APACHE_UNFILLED_COPYRIGHT: string;

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

export interface ElectionAttribution {
  name: string;
  version: string;
  /** The manifest's `authors`, verbatim; checked against `cargo metadata` on every run. */
  authors: string[];
  repository: string;
}

export interface Election {
  purl: string;
  name: string;
  version: string;
  ecosystem: 'cargo';
  /** Must equal the component's declared expression, and be a pure `A OR B [OR C]` disjunction. */
  declaredSpdx: string;
  chosen: 'Apache-2.0';
  /** The SHA-256 of the decoded Apache-2.0 text the inventory ships most widely. */
  textSha256: string;
  attribution: ElectionAttribution;
  /** The revision that was searched for a licence file and found to have none. */
  commit: string;
  tag: string | null;
  investigated: string;
  evidence: string[];
}

export interface UpstreamStore {
  sources: Map<string, UpstreamSource>;
  byPurl: Map<string, UpstreamComponent>;
  blockedByPurl: Map<string, BlockedComponent>;
  electionsByPurl: Map<string, Election>;
}

/** The fields of an inventory component an election is checked against. */
export interface ElectionComponent {
  purl: string;
  declaredLicense: string | null;
  declaredLicenseFile: string | null;
  repository: string | null;
  authors?: string[];
  distributed: boolean;
  /** The installed crate directory, which `resolveElections` scans itself. */
  sourceDir?: string | null;
}

/** One distinct text read out of an installed artifact, and the components that carry it. */
export interface PooledText {
  textSha256: string;
  text: string;
  purls: Set<string>;
}

export interface ElectedArtifactScan {
  /** False when the directory is missing: "looked nowhere" never reads as "found nothing". */
  exists: boolean;
  /** Licence-like files or directories (directories end in `/`) at any depth, relative, sorted. */
  licenceFiles: string[];
  /** `path:line` of every line carrying a copyright notice, in any file. */
  notices: string[];
  /** Symlinks and other entries the scan did not follow. */
  unscanned: string[];
}

export declare function isLicenceLikeName(name: string): boolean;
export declare function copyrightNoticeLines(text: string): number[];
export declare function scanElectedArtifact(dir: string | null | undefined): ElectedArtifactScan;
export declare function disjunctionOperands(expr: string): string[] | null;
export declare function isApache2Text(text: string): boolean;
export declare function electionProblems(
  election: Election,
  component: ElectionComponent,
  scan: ElectedArtifactScan,
): string[];
export declare function electedText(
  election: Election,
  pool: PooledText[],
): { problems: string[]; text?: string };
export declare function electionSelfTest(): {
  failures: string[];
  controls: number;
  rejects: number;
  accepts: number;
};
export declare function resolveElections(
  electionsByPurl: Map<string, Election>,
  components: ElectionComponent[],
  pool: PooledText[],
): Map<string, { election: Election; text: string; textSha256: string }>;

export declare function loadUpstreamStore(
  root: string,
  options?: { tracked?: Set<string> },
): UpstreamStore;

export declare function storeProvenancePath(id: string): string;
export declare function storeFilePath(root: string, id: string): string;
