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
| **Licence texts** | `npm run check:licenses` | Rust | 440 distributed components, 247 distinct texts, 32 recorded gaps |
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

**Missing step:** one `npm run tauri:dev` by a human, confirming the app renders and IPC works with
no CSP violation in the webview console. Until that happens the policy is *validated*, not *verified*.

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

## 3. Blockers that are not engineering

Neither can be closed by writing code, and neither is marked closed.

### 3.1 Licence texts — packaged for 408 of 440, blocked on 32

**Updated 2026-07-27.** This section previously read "Licence texts are not packaged", which was
accurate then and is not now.

`licensing/THIRD_PARTY_LICENSES.txt` reproduces the licence and copyright notices of the distributed
components verbatim, from the exact installed versions, with a SHA-256 per source file in
`licensing/third-party-index.json` so any entry can be re-verified against the package it came from.
Coverage is **408 of 440** distributed components and **247** distinct texts.

The residual **32** are components whose *published artifact contains no licence file at all* —
`naga`, `webview2-com`, the five `unic-*` crates, `neo4rs`, `ts-rs`, `@react-three/fiber` and others,
enumerated with the exact reason and upstream in `licensing/UNRESOLVED.md`.

**Engineering cannot close these, and did not pretend to.** The canonical SPDX text of MIT contains
no copyright holder, and reproducing the holder's notice is precisely what MIT requires; substituting
a generic text would look like compliance and would not be it. Closing one means a human obtaining
the licence file from the upstream repository at the tag matching the version, recording where it
came from, and re-running the generator.

**Release stays blocked for those 32 components.** The gate is `npm run check:licenses-complete`,
which exits non-zero while any remain. It is deliberately **not** in CI: it fails by design today,
and a permanently-red required check is a check people learn to ignore. It is a release-time step,
run by whoever signs off the distribution.

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
