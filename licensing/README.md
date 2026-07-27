# `licensing/` — what ships, and under what terms

Everything here is **generated**. Do not edit any of it by hand; the `--check` gates compare bytes
and will fail on a manual edit exactly as they fail on a stale one.

| File | Written by | What it is |
|---|---|---|
| [`THIRD_PARTY_LICENSES.txt`](THIRD_PARTY_LICENSES.txt) | `npm run gen:licenses` | **The file that ships.** Component identity block for every distributed component, then each distinct licence text once. |
| [`third-party-index.json`](third-party-index.json) | `npm run gen:licenses` | Machine-readable provenance: SPDX, ecosystem, origin, and the SHA-256 of every source licence file as installed. |
| [`UNRESOLVED.md`](UNRESOLVED.md) | `npm run gen:licenses` | Distributed components whose licence text could not be obtained, with the exact reason and the upstream to fetch it from. |
| [`bundle-closure.json`](bundle-closure.json) | `npm run build` | The npm distribution boundary, measured from the real module graph by a plugin in `vite.config.ts`. |

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

## What is and is not established

**Established.** Every distributed component is named with its exact locked version, its declared
SPDX expression, and — for 408 of 440 — the verbatim text of its licence, with a SHA-256 that can be
checked against the installed package. The SBOM validates against the official CycloneDX 1.5 schema,
vendored in [`../schemas/cyclonedx/`](../schemas/cyclonedx/) and pinned to a specification commit.
All of it regenerates deterministically: no timestamps, byte-order sorting, a serial number derived
from content.

**Not established.**

- **32 components have no obtainable licence text.** Their published artifacts contain no licence
  file. See [`UNRESOLVED.md`](UNRESOLVED.md). No text is synthesised to cover for this: the
  canonical SPDX text of MIT contains no copyright holder, and reproducing the holder's notice is
  exactly what MIT requires, so a substituted text would look like compliance and would not be it.
  **Distribution of those components stays blocked.** The gate is
  `npm run check:licenses-complete`, which exits non-zero while any remain — deliberately not in CI,
  because a permanently-red required check is one people learn to ignore.
- **No legal review has been performed.** This is an inventory and a text bundle, not an assessment
  of whether each licence is compatible with proprietary distribution. The MPL-2.0 and
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

The order matters: `NOTICE` reports the licence bundle's coverage and reads
`third-party-index.json`, so generating it against a stale index would put a number in prose that no
artifact backs up.

To verify without regenerating — which is what CI does — `npm run check:compliance`, plus
`npm run check:bundle-closure` after a build.
