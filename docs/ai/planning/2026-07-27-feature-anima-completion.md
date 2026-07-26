---
phase: planning
feature: anima-completion
title: Plan & Evidence Ledger — Completion & Hardening pass
description: Ordered work packages, each with a gate, an artifact and a commit
status: active
owner: maintainers
last_reviewed: 2026-07-27
requirements: ../requirements/2026-07-27-feature-anima-completion.md
design: ../design/2026-07-27-feature-anima-completion.md
testing: ../testing/2026-07-27-feature-anima-completion.md
---

# Plan & Evidence Ledger — Completion & Hardening pass

Branch `feature-anima-completion`, worktree `.worktrees/feature-anima-completion`, base
`6caeeb4`.

## Status legend

`[ ]` not started · `[~]` in progress · `[x]` done, gate green and recorded · `[!]` blocked, blocker
named below.

## 0. Baseline — measured 2026-07-27 at `6caeeb4`

Reproduced, not quoted. Every row is a command run in this worktree on this machine, with the raw
output kept under the session evidence directory and summarised here.

| Gate | Command | Result | Exit |
|---|---|---|---|
| Backend test | `cargo test --features desktop --no-fail-fast` | **746 passed · 0 failed · 4 ignored**, 77 result lines, **0 compile warnings** | 0 |
| Empty targets | `node scripts/check_test_targets.mjs <out>` | **75 targets, 0 empty** | 0 |
| Format | `cargo fmt --check` | clean | 0 |
| DevKit lint | `ai-devkit lint` | all checks passed | 0 |
| DevKit skills | `ai-devkit skill list` | 20 skills, all 19 required present | 0 |

Remaining baseline rows are recorded in [the testing doc](../testing/2026-07-27-feature-anima-completion.md)
as they complete; nothing is marked green here from memory.

**Deviation from the reviewed base's reported numbers: none.** The directive quoted 746/0/4 and that
is what reproduced.

## 1. Work packages, in execution order

Ordered by return, not by difficulty. Each package is a commit or a small run of commits.

| # | Package | Requirement | Definition of done |
|---|---|---|---|
| WP0 | Baseline + lifecycle docs + this ledger | — | Baseline table above filled from fresh runs; requirements/design/planning/testing docs exist |
| WP2 | Grid dimension reconciliation | R4, R5 | `DEFAULT_GRID_DIM` equals `MapSettings::default()`, tied by an assertion; four documents corrected; transform property tests at border/corner/non-default dims green |
| WP5 | Lineage cumulative mutations → compression | R9 | `Option<u32>` field lands with `serde(default)`; UI count identical pre/post compression on one fixture; a v4 save still reads; `compact` reaches the `2·samples` bound |
| WP4 | Security: CSP, save/load, unsafe | R6, R7, R8 | CSP explicit and app-compatible; save/load takes a name resolved under app-data with escape refused and a tested migration reader; both `unsafe impl` proven, encapsulated or removed |
| WP1 | Map manifest becomes evidence | R1, R2 | Manifest generated with a real artifact and real SHA-256; evidence test loads the committed file and checks checksum + existence; MCP gates run on the generated MCP-schema manifest |
| WP3 | E2E isolation + identity | R3 | Own port, own server, Anima fingerprint asserted; wrong app fails rather than skips |
| WP6 | Generated bindings authoritative | — | Handwritten IPC mirrors replaced by generated types; drift fails a gate |
| WP7 | ESLint to zero | R11 | 0 errors, 0 warnings, ratchet baseline lowered, no rule relaxed |
| WP8 | NOTICE + inventory + SBOM | R10 | `NOTICE` generated from the shipped graph; `LICENSE` scope split; inventory includes `burn`/`burn-wgpu`; SBOM reproducible |
| WP9 | Bundle budget gate | — | Heavy 3D deps split; a budget gate fails on regression |

## 2. Evidence ledger

One row per verified finding. `Status` is only `closed` when the named gate has been run **after**
the fix and its output recorded.

| ID | Finding | Severity | Gate that proves it closed | Artifact | Status | Commit |
|---|---|---|---|---|---|---|
| A1 | `map_manifest.json` references a non-existent artifact and 8 non-existent PNGs; checksum is 64 zeroes | High | evidence test opens the committed manifest, hashes the artifact | `map_manifest.json`, `artifacts/` | open | — |
| A2 | S05 validates an inline fixture, never the committed file | High | the same evidence test, which fails if the file is absent | `src/__tests__/` | open | — |
| A3 | Playwright adopts any server on 5173 and downgrades app mismatch to a skip | High | E2E identity assertion; wrong app ⇒ failure | `tests/e2e/playwright.config.ts` | open | — |
| B1 | `DEFAULT_GRID_DIM = 128` vs `MapSettings::default()` = 256; four docs publish a 2× wrong scale | High | drift assertion binding constant to `MapSettings::default()` | `sim_rules.rs`, 4 docs | open | — |
| C1 | `security.csp: null` | Medium | explicit policy present; app still loads | `tauri.conf.json` | open | — |
| C2 | save/load accept an unconstrained path from the frontend | High | escape attempt refused; migration reader test | `commands/simulation.rs` | open | — |
| C3 | 2 `unsafe impl Send/Sync` with no proof | Medium | proof present, or the `unsafe` gone | `ai/model.rs` | open | — |
| D1 | Lineage memory unbounded; compression off because the mutation count walks edges | High | count identical pre/post compression; node count at `2·samples` | `evolution/lineage.rs` | open | — |
| D2 | No `NOTICE`; `LICENSE` has no scope split; `burn` absent from inventory | Medium | `NOTICE` covers the shipped graph | `NOTICE`, `LICENSE` | open | — |
| D3 | 491 ESLint warnings, frozen by a ratchet that only blocks growth | Medium | 0 warnings, baseline lowered | `eslint.config.js` | open | — |

## 3. Decisions that are not mine to take

Recorded per operating rule 7: alternatives, evidence, recommendation, and the exact blocked task.
Independent work continues around them.

### DEC-1 — EB-S04 re-baseline, and the per-agent-brain default

**Blocked task:** §3.1 of the status doc — flipping `BrainPolicy::default().evolved` to `true`.

**Evidence.** 11 of 12 EB gates pass. EB-S04 demands a bit-identical trajectory against a baseline
captured *before* shared-model initialisation became seeded. It fails because the initialisation
was deliberately improved (random → seeded, so two runs of one world stop diverging invisibly), not
because anything regressed.

**Alternatives.**

1. *Re-baseline EB-S04 against the seeded build*, record in ADR-0003 why the old reference was
   discarded, then decide the default separately. Cost: the old baseline stops being a regression
   net for anything else it happened to cover.
2. *Keep EB-S04 as-is and leave it permanently amber.* Cost: a gate nobody can turn green stops
   being read at all, which is worse than no gate.
3. *Reconstruct a bit-identical seeded-equivalent baseline.* Cost: substantial, and it re-encodes a
   comparison the seeding change made meaningless.

**Recommendation:** (1). A gate that cannot pass by correct code is a broken gate, and the honest
repair is an explicit re-baseline with the reason written down.

**Why I am not taking it:** it discards a scientific reference point, and §3.1 states the default
decision must be recorded as an ADR decision item. That is an owner's call, not an implementer's.

### DEC-2 — Canonical map view capture

**Blocked task:** producing the eight canonical view PNGs.

**Evidence.** No capture pipeline exists; `scripts/gen_map_manifest.ts` documents
`pipeline.panorama` as a placeholder for exactly this reason. CLAUDE.md forbids running the full
Tauri backend on this machine (it has crashed it).

**Alternatives.** (1) build a Playwright-driven WebGL capture harness against `landscape.html` —
real renderer, real pixels, but a new subsystem; (2) render an orthographic raster from worldgen
data — cheap, but it is *not* what the app draws, so it would be evidence for a different thing;
(3) declare the views uncaptured and gate the invariant instead.

**Recommendation:** (3) now, (1) as a follow-up feature. Implemented in WP1: views carry
`captured: false` and the gate enforces "anything claiming captured must exist".

**Why I am not taking (1) here:** it is a feature, and a capture harness nobody has validated
produces screenshots that *look* like evidence without being it.
