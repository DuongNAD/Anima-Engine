---
phase: deployment
feature: anima-completion
title: Release readiness — Completion & Hardening pass
description: What must be true before this ships, what is already gated, and the two blockers that are not engineering
status: active
owner: maintainers
last_reviewed: 2026-07-27
plan: ../planning/2026-07-27-feature-anima-completion.md
implementation: ../implementation/2026-07-27-feature-anima-completion.md
---

# Release readiness — Completion & Hardening pass

> ## 📜 Historical package record — not current status
>
> This document belongs to **one dated work package** on base `6caeeb4`. Every count in it
> (test totals, warning counts, target counts, coverage numbers) is a **historical measurement**:
> true when the command ran during that package, and not a description of the tree today.
>
> **Current measured status lives in exactly one place:**
> [`docs/planning/STATE_OF_THE_PROJECT.md` §1](../../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).


This pass is hardening, not a feature: there is no flag to roll out and no schema to migrate that a
user chooses. What it changes about *deployment* is the set of things that must be true before a
build is distributed, and which of those a machine now checks.

## 1. Gates that run automatically

Each is wired into `.github/workflows/ci.yml` and fails the build. The Job column matters because
three of these moved this pass — they existed as package scripts that nothing invoked, and a script
is not a gate until something fails on it.

| Gate | Command | Job | Guards |
|---|---|---|---|
| Backend tests | `cargo test --features desktop` | Rust | the whole simulation contract |
| Empty targets | `node scripts/check_test_targets.mjs` | Rust | a feature-gated file compiling to an empty binary |
| Format / lint | `cargo fmt --check`, `cargo clippy --all-targets -D warnings` (both feature modes) | Rust | — |
| Feature split | `cargo tree` on a default build | Rust | Neo4j / WebSocket / Burn-WGPU staying out of headless |
| Binding drift | `cargo test --lib export_bindings` + `git diff --exit-code` | Rust | a Rust IPC struct changing without the TS binding |
| Rust advisories | `cargo audit` | Rust | two ignored entries, each with recorded verification |
| **Licence texts** | `npm run check:licenses` | Rust | 440 distributed components, 266 distinct texts, 1 recorded gap |
| **SBOM freshness** | `npm run check:sbom` | Rust | 458 components, 459 dependency records |
| **SBOM validity** | `npm run check:sbom-schema` | Rust | official CycloneDX 1.5 schema, vendored and pinned by checksum |
| **NOTICE freshness** | `npm run check:notice` | Rust | 419 crates + 21 distributed npm + 18 install-only |
| **Bundle closure** | `npm run check:bundle-closure` | Frontend (post-build) | the npm distribution boundary drifting from a fresh build |
| **Text hygiene** | `npm run check:text-hygiene` | Frontend | raw control bytes making a source file binary to git |
| **Flora footprint** | `npm run check:flora-footprint` | Rust | declared canopy radii vs the real geometry |
| npm advisories | `npm audit --audit-level=high` (both packages) | Frontend | — |
| ESLint | `npm run lint` + ratchet | Frontend | 0 errors; warnings may not grow |
| Frontend suites | `npm run test`, `npm run test:frontend` | Frontend | includes the manifest, NOTICE/SBOM and binding-authority gates |
| Build | `npm run build` | Frontend | `tsc` strict + two Vite entries |
| **CSP compatibility** | `npm run check:csp` | Frontend | no external origin, no inline script, hardening directives present |
| **Bundle budget + split** | `npm run check:bundle` | Frontend | per-chunk budgets **and** that `index.html` never statically loads the three.js chunk |
| E2E (browser) | `npm run test:e2e` | Frontend | zero fail, zero skip |
| Doc links | `node scripts/check_docs_links.mjs` | Frontend | — |

Bold rows are new to CI in this pass.

## 2. Gates that need a human, and why

These are not oversights. Each names the specific reason a machine in this repository cannot run it.

### 2.1 The app booting under the new CSP

`npm run check:csp` validates *shipped artifacts against the declared policy* — no external origins,
no inline `<script>` bodies, hardening directives present. It cannot prove the app boots under that
policy, because that needs the Tauri webview, and CLAUDE.md records that running the full backend on
the development machine has crashed it.

**Done — 2026-07-27.** The project owner ran `npm run tauri:dev` from
`.worktrees/feature-anima-completion` at `0f5b4d3`, opened the webview's DevTools, reloaded so the
console covered a full page load, and drove IPC from it. The policy is now *verified*, not merely
validated:

- The app renders. "Anima-Engine Control Center" mounted, the status panel updated live, and the
  window reached **91 781 ticks**.
- **No CSP violation.** Across a full page load the console held four
  `[TAURI] Couldn't find callback id …` warnings — the expected consequence of reloading a webview
  while Rust holds promises from the previous page — and the React DevTools notice. Nothing matching
  `Refused to …` or `Content Security Policy`.
- **IPC works from the webview.** `window.__TAURI_INTERNALS__` was defined, and
  `toggle_simulation`, `start_tick_capture`, `get_tick_capture_status` and `export_tick_capture` all
  returned real values — which is also how the tick-capture evidence in
  [`BENCHMARK_BASELINE.md`](../../../BENCHMARK_BASELINE.md) was produced.

The screenshot is held by the project owner; this repository still has no committed location for
app screenshots, and [BENCHMARKING.md](../../how-to/BENCHMARKING.md) declines to invent one because
`map-views/` is byte-pinned by a manifest and a stray file there disturbs a green gate.

**One defect this found, and it was not the one being looked for.** The first launch showed a blank
window: `devUrl` is `http://localhost:5173`, Node has resolved `localhost` verbatim since v17, and
`::1` came first — which on that machine was a Vite dev server belonging to an unrelated project.
The webview loaded *that* application inside the Anima window. CSP was never the problem; two
projects sharing Vite's default port were. See §2.4.

### 2.2 Real-backend E2E

`tests/e2e/real_backend.spec.ts` declares no test unless `ANIMA_E2E_REQUIRE_BACKEND=1`, and fails
closed on every missing precondition when it is set. Two are missing:

1. `cargo build --release --features desktop` — no CI job builds it;
2. a `tauri-driver` WebDriver session — not wired up here at all.

Spawning the binary and pointing Playwright at a Vite page is *not* a substitute; that is precisely
what the five deleted specs did, and it connected nothing. Set the flag on a job that has both.

### 2.3 Canonical view capture

`npm run capture:views` needs a GPU. Measured: 0.27 fps on SwiftShader against 46.7 fps on hardware,
and the software path is a different rasteriser, so its output is evidence about a renderer nobody
ships. The harness fails closed rather than falling back.

**How a GPU-less CI runner still holds the line:** the eight PNGs are committed, `map_manifest.json`
carries each one's SHA-256, and `mapManifestEvidence.test.ts` verifies all eight. Re-shooting needs
hardware; checking that the committed bytes are the described bytes does not.

Re-run the capture whenever `WORLD_GEN_VERSION`, the world identity, or a canonical camera pose
changes — all three invalidate every image — then regenerate the manifest and commit both.

### 2.4 `devUrl` names a hostname, and a hostname is not an address

**Found by running §2.1, and it cost the first launch.** `tauri.conf.json` sets
`devUrl: "http://localhost:5173"`. Vite's dev server binds whatever `localhost` resolves to; since
Node 17 that resolution is *verbatim*, so `::1` is tried first on Windows and Linux alike. Nothing in
the app pins which of the two addresses it gets, and 5173 is Vite's default — which every other Vite
project on the machine also wants.

What that produced, measured on 2026-07-27: `[::1]:5173` was held by an unrelated project's dev
server while Anima's own beforeDevCommand server bound `127.0.0.1:5173`. Both succeeded, because one
had claimed only the v6 side. The webview asked for `localhost`, got `::1`, and rendered **the other
project's application inside the Anima window** — a blank page with a foreign widget in the corner.
No error anywhere: the dev server logged `ready in 190 ms`, Tauri loaded a page that returned 200,
and the CSP permits `http://localhost:5173`, which is what both servers were.

This is the third instance of the same fault in one day. The other two were
`tests/e2e/playwright.config.ts` and `tests/e2e/capture.config.ts`, both fixed in `72a6e34` and
`0f5b4d3` by binding the dev server to the literal address the client probes.

**Fixed after the measuring run, in its own commit.** Changing `devUrl` during §2.1 would have
changed the thing being measured, so it waited. The fix removes the resolution step rather than
racing it:

- `devUrl` is `http://127.0.0.1:5173` — a literal address, not a name with two answers.
- `vite.config.ts` sets `server.host = "127.0.0.1"`. That is the single place every launch of the
  dev server passes through, so `npm run dev`, `beforeDevCommand` and both Playwright configs agree
  by construction rather than by each remembering a flag.
- `devCsp` follows: `script-src` and `connect-src` name `http://127.0.0.1:5173` and
  `ws://127.0.0.1:5173`, or the page loads and HMR's websocket is refused.
- `.claude/launch.json` too, so the preview tooling opens the same address the app does.

The production `csp` block is untouched — it names no dev origin and never did.

**Verified, and the limit of that verification stated.** A dev server started from this config binds
`127.0.0.1:5199` and nothing else: `Get-NetTCPConnection` lists one row, `http://127.0.0.1:5199/`
returns 200 with `<title>Anima Engine</title>`, and `[::1]:5199` refuses the connection — so there is
no v6 socket left for another project to answer on. A non-default port was used because the app under
§2.1 still held 5173. `npm run check:csp`, `npm run build` and `npm run lint` + ratchet are green.

What that does **not** prove is the webview end: no agent in this repository may run
`npm run tauri:dev`, so *"the Tauri window loads Anima under the new `devCsp`"* is still a
human-verified step. It is cheap to fold into the next owner run — the app either shows
"Anima-Engine Control Center" or it does not — but until someone does it, this section claims a
correctly bound dev server, not a correctly booted app.

## 3. Blockers that are not engineering

Neither can be closed by writing code, and neither is marked closed.

### 3.1 Licence texts — packaged for 439 of 440, blocked on 1

**Updated 2026-07-27 (second pass).** This section first read "Licence texts are not packaged", then
"packaged for 408 of 440, blocked on 32". Both were accurate when written. Neither is now.

`licensing/THIRD_PARTY_LICENSES.txt` reproduces the licence and copyright notices of the distributed
components verbatim, with a SHA-256 per source file in `licensing/third-party-index.json` so any
entry can be re-verified. Coverage is **439 of 440** distributed components and **266** distinct
texts: **408** read out of the installed artifact, **31** vendored from upstream.

**How the 31 were closed.** Their published artifacts contain no licence file at all, so the text was
taken from the upstream repository at the **immutable commit that release was published from** —
never from a branch. 39 files, 24 commits, 19 repositories, stored byte-for-byte under
`licensing/upstream/` with the evidence in `licensing/upstream/sources.json`. The commit↔version link
is the publisher's own record: `.cargo_vcs_info.json` inside the published `.crate` for Rust, the
registry's `gitHead` for npm, corroborated by a resolved release tag where one exists and by reading
the crate manifest back at that commit where one does not. `scripts/lib/upstream_licenses.mjs` reads
the store fail-closed — hash, length, commit, ref, purl, traversal, symlink escape, untracked files,
unused mappings — and `npm run verify:upstream-licenses` re-fetches every pinned URL and compares
bytes. Nothing in CI touches the network.

**The residual 1** is `hexf-parse` 0.2.1. It declares CC0-1.0, and neither its artifact nor its
repository contains a licence file at the release commit or at any commit before it; the only
`LICENSE` that project has ever committed arrived three and a half years later and is the Zero-Clause
BSD text — a *different* licence. `licensing/UNRESOLVED.md` records the whole search. No CC0-1.0 text
is substituted from a licence list: engineering does not get to package a text a project has never
published and record it as that project's licence file.

**Release stays blocked for that component.** The gate is `npm run check:licenses-complete`, which
exits non-zero while any remain. It is deliberately **not** in CI: it fails by design today, and a
permanently-red required check is a check people learn to ignore. It is a release-time step, run by
whoever signs off the distribution. Closing it is a legal decision about whether the crates.io
declaration alone suffices for a public-domain dedication that imposes no attribution condition.

Two further rows are packaged but warrant a legal read, and are flagged in `licensing/README.md`:
`neo4rs`/`neo4rs-macros`, whose project has never published a licence file and whose README statement
is vendored verbatim in its place; and `zune-inflate`, which declares `MIT OR Apache-2.0 OR Zlib`
while upstream publishes only the Zlib text plus a copyright notice — every file present at the
pinned release is packaged, and no option is chosen on the owner's behalf.

`npm run check:licenses` *is* in CI, and fails on any change to the artifacts — so a newly added
dependency with no licence text turns the build red on the commit that introduces it, rather than
being discovered at release.

Two paths, and choosing between them is an owner call: package each component's licence file into the
bundle at build time, or obtain a legal review that says the current form suffices.

### 3.2 `LICENSE` scope language

`LICENSE` is proprietary and now carries a scope section separating code / model weights / datasets /
assets. That is legal text, not documentation. It should not be treated as reviewed because it is
committed — a maintainer or legal reviewer must approve the scope language and the distribution
obligations before release.

## 4. Migration effects on an existing installation

Nothing here requires user action, but three things change under an installed app and are worth
knowing before a build goes out.

| Change | Effect on first launch after upgrade |
|---|---|
| `WORLD_GEN_VERSION` 20 → 21 | The IndexedDB cache key changes, so the world regenerates once (~7 s in-browser at 2048²). Flora placement differs slightly — species now follow the cell an instance lands in. |
| Autosave moves to `saves/autosave.json` | The old `default_save.json` is read once and adopted; it is never written back and never deleted. A user who downgrades still has it. |
| Save path confinement | Saves written to arbitrary paths by an older build are no longer loadable by name. They are importable: copy the file into `<app data>/legacy-import/` and use the import command. Documented in `PROJECT.md`. |

## 5. Rollback

Every change in this pass is a code change with no persisted state of its own, so rollback is
`git revert` of the relevant commit — with two exceptions worth stating:

- Reverting the worldgen ordering fix (`fd3dc79`) should also revert `WORLD_GEN_VERSION` to 20, or
  the cache key stays changed and the world regenerates a second time.
- Reverting the autosave move (`ae1fb40`) after a user has launched the new build strands
  `saves/autosave.json`, because the old code only reads `default_save.json`. Copy it back manually,
  or accept the loss of one session.
