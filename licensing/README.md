# `licensing/` — what ships, and under what terms

Everything here except [`upstream/`](upstream/) is **generated**. Do not edit any of it by hand; the
`--check` gates compare bytes and will fail on a manual edit exactly as they fail on a stale one.

| File | Written by | What it is |
|---|---|---|
| [`THIRD_PARTY_LICENSES.txt`](THIRD_PARTY_LICENSES.txt) | `npm run gen:licenses` | **The file that ships.** Component identity block for every distributed component, then each distinct licence text once. |
| [`third-party-index.json`](third-party-index.json) | `npm run gen:licenses` | Machine-readable provenance: SPDX, ecosystem, origin, and the SHA-256 of every source licence file — as installed, or as vendored. |
| [`UNRESOLVED.md`](UNRESOLVED.md) | `npm run gen:licenses` | Distributed components whose licence text could not be obtained, with the exact reason **and the search that failed**. |
| [`bundle-closure.json`](bundle-closure.json) | `npm run build` | The npm distribution boundary, measured from the real module graph by a plugin in `vite.config.ts`. |
| [`upstream/`](upstream/) | **hand-maintained, reviewed** | Vendored upstream licence texts, with [`upstream/sources.json`](upstream/sources.json) recording where every byte came from. The one thing here a human writes. |

Two more artifacts live outside this directory because they are read at the repository root:
[`../NOTICE`](../NOTICE) (the attribution inventory) and [`../sbom.cdx.json`](../sbom.cdx.json)
(CycloneDX 1.5).

## The three closures, and why they are not one

This is the distinction the previous generation of these artifacts got wrong, in both directions at
the same time.

| Closure | Definition | Distributed? | Count |
|---|---|---|---|
| `cargo-desktop` | `cargo tree --features desktop -e normal` | yes — linked into the binary | 419 |
| `npm-bundle` | packages with **rendered bytes in `dist/`** | yes — Tauri packages `dist/` | 21 |
| `npm-install-only` | in the production install closure, no bytes in `dist/` | **no** — `node_modules` is not shipped | 18 |

`npm ls --omit=dev --all` answers "what did npm install", which was used as if it answered "what
ships". Measured, the two differ by 21 components:

- **18 over-attributed.** `@types/react`, `csstype`, `@webgpu/types`, `js-tokens`, `@babel/runtime`
  and others leave no bytes in the product. Type-only packages contain no runtime code at all, so no
  bundler could put them there.
- **3 under-attributed.** `vite`, `rolldown` and `@oxc-project/runtime` are compiled *into* the
  shipped chunks — `vite/preload-helper.js`, `rolldown/runtime.js`, and four
  `@oxc-project/runtime` helpers. None is a production dependency; `@oxc-project/runtime` is not
  installed at all, because rolldown carries the helper sources inside its own distribution. No
  reading of `package.json` can find any of them.

Only the bundler knows, so the bundler is what reports: `bundle-closure.json` is written during
`npm run build` and `npm run check:bundle-closure` fails when a fresh build disagrees with the
committed copy.

## The vendored store, and why it is not a loophole

32 distributed components published no licence file in their artifact at all. No amount of reading
`node_modules/` or the cargo registry could close them, because the text is simply not there — but
it *is* in the upstream repository, at the revision the release was cut from. [`upstream/`](upstream/)
holds those bytes.

The layout **is** the provenance:

```
licensing/upstream/github.com/<owner>/<repo>/<commit>/<path-in-repo>
https://raw.githubusercontent.com/<owner>/<repo>/<commit>/<path-in-repo>
```

The store path is the tail of the raw URL, byte for byte, so location and provenance cannot drift
apart. The ref segment must be a 40-hex commit, which is what makes `main`, `master`, `trunk` and
`HEAD` unspellable — a rule rather than a blocklist, because a blocklist of branch names is a list
someone eventually gets around by naming a branch `release`.

**How a commit was tied to a released version.** For a crate, the published `.crate` carries
`.cargo_vcs_info.json`, written by `cargo publish` from the tree it packaged; that is the
publisher's own record and it is stronger evidence than a tag. Ten of the nineteen repositories have
no tag at the relevant commit at all, so for those the crate manifest at that commit is read back and
checked to declare the exact name, version and licence we ship. For npm, the registry's `gitHead` for
that exact version plays the same role. `@oxc-project/runtime` needed more, because rolldown compiles
its helpers into `dist/` and it is never installed: the tag `crates_v0.139.0` resolves to the commit
that set `npm/runtime/package.json` to 0.139.0, the published tarball's four shipped helper sources
are byte-identical to that commit's, and the tarball's own `LICENSE` is byte-identical to the
repository-root `LICENSE` vendored here.

**What keeps it honest**, all enforced by `scripts/lib/upstream_licenses.mjs` and all fail-closed:

- **Installed text wins.** A mapping for a component whose artifact *does* carry its licence is an
  error, not a fallback — so the day an upstream starts shipping its own text, the stale pin fails
  loudly instead of quietly shadowing the real thing.
- **Every mapping must be used.** A mapping for a component that is not in the graph, is not
  distributed, or is not currently unresolved stops the run. So does a vendored file no component
  maps.
- Hash, byte length, commit, ref, purl/name/version agreement, path traversal, absolute paths,
  symlink and junction escape, untracked files, duplicate ids, duplicate purls, byte ordering.
  [`../tests/frontend/upstreamLicenses.test.ts`](../tests/frontend/upstreamLicenses.test.ts) breaks
  each one on purpose; a validator that never rejects anything is indistinguishable from one that
  returns true.
- **`npm run verify:upstream-licenses`** re-fetches every pinned URL into a temporary directory and
  compares. It is the only part of this system that touches the network, it is opt-in, and it never
  writes back: a verifier that repairs what it finds turns a tampered store into a clean one and
  reports success.

**One deliberate exception, and its argument is in the file.** `selectors` 0.36.1 declares MPL-2.0
and `servo/stylo` publishes no copy of the MPL at the release commit — a recursive tree scan finds
only the Apache/MIT texts belonging to `malloc_size_of` and `servo_arc`. Its text therefore comes
from `servo/servo`'s own `LICENSE`: the primary repository of the same GitHub organisation, the
project named in the crate's `authors` field, pinned to the commit that last modified that file. The
mapping is marked `project-repository` rather than `release-tree`, carries a written justification,
and records the component's own repository and commit alongside. The loader requires all of that; the
test suite asserts this is still the only one.

## What is and is not established

**Established.** Every distributed component is named with its exact locked version, its declared
SPDX expression, and — for 439 of 440 — the verbatim text of its licence: 408 read out of the
installed artifact, 31 vendored from a pinned upstream commit, each with a SHA-256 that can be
re-checked against the package or re-fetched from its URL. The SBOM validates against the official
CycloneDX 1.5 schema, vendored in [`../schemas/cyclonedx/`](../schemas/cyclonedx/) and pinned to a
specification commit. All of it regenerates deterministically: no timestamps, byte-order sorting, a
serial number derived from content.

**Not established.**

- **1 component still has no obtainable licence text.** `hexf-parse` 0.2.1 declares CC0-1.0 and
  neither its artifact nor its repository — at the release commit, or at any commit before it —
  contains a licence file of any kind; the only `LICENSE` that project has ever committed arrived
  three and a half years later and is a *different* licence. The search is recorded in
  [`UNRESOLVED.md`](UNRESOLVED.md) with its evidence. No CC0-1.0 text is substituted from a licence
  list: CC0 imposes no attribution condition, so the omission carries no notice obligation, but
  engineering does not get to package a text a project has never published and record it as that
  project's licence file. **Closing that row is a legal decision, not an engineering one.** The gate
  is `npm run check:licenses-complete`, which exits non-zero while any remain — deliberately not in
  CI, because a permanently-red required check is one people learn to ignore.
- **`neo4rs` and `neo4rs-macros` are packaged from a licence *statement*, not a licence text.** That
  project has never published a licence file at any revision. Its `README.md` at the release commit
  states dual Apache-2.0/MIT and links `LICENSE-APACHE` and `LICENSE-MIT`, neither of which exists,
  while `Cargo.toml` declares MIT alone. The README is vendored verbatim and marked
  `licence-statement`; both upstream facts are reproduced unaltered rather than reconciled into a
  reconstructed MIT text with a guessed copyright holder. Whether that discharges the obligation is
  a legal question this repository does not answer.
- **`zune-inflate` declares `MIT OR Apache-2.0 OR Zlib` and upstream publishes only the Zlib text**
  (plus a copyright notice covering all three). Every licence-bearing file present at the pinned
  release is packaged; no option is selected on the owner's behalf, and no MIT or Apache-2.0 text is
  supplied for options upstream did not publish.
- **No legal review has been performed.** This is an inventory and a text bundle, not an assessment
  of whether each third-party licence is compatible with the project's `MIT OR Apache-2.0`
  distribution. The MPL-2.0 and
  copyleft-adjacent components in the graph are in the "needs explicit review" class of
  [`../docs/governance/OPEN_SOURCE_POLICY.md`](../docs/governance/OPEN_SOURCE_POLICY.md), and that
  review has not happened. Nothing here constitutes legal sign-off.
- **These artifacts describe a Windows build.** `cargo tree` resolves for the host target. A macOS
  or Linux build links a different set and must regenerate them on that platform.

## Regenerating

```
npm run build            # rewrites licensing/bundle-closure.json from the real module graph
npm run gen:compliance   # gen:licenses -> gen:sbom -> gen:notice, in that order
```

The order matters: `NOTICE` and `sbom.cdx.json` both read `third-party-index.json` — for the
coverage number and for the licence expression of the component that has no manifest to declare one
— so generating either against a stale index would put a claim in one artifact that no other backs
up.

To verify without regenerating — which is what CI does — `npm run check:compliance`, plus
`npm run check:bundle-closure` after a build. None of those reaches the network.
`npm run verify:upstream-licenses` does, and is run by hand.
