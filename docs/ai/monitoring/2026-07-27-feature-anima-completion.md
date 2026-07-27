---
phase: monitoring
feature: anima-completion
title: Observability — Completion & Hardening pass
description: What each new gate would report if it broke, and the signals that would say a fix has regressed
status: active
owner: maintainers
last_reviewed: 2026-07-27
design: ../design/2026-07-27-feature-anima-completion.md
implementation: ../implementation/2026-07-27-feature-anima-completion.md
---

# Observability — Completion & Hardening pass

> ## 📜 Historical package record — not current status
>
> This document belongs to **one dated work package** on base `6caeeb4`. Every count in it
> (test totals, warning counts, target counts, coverage numbers) is a **historical measurement**:
> true when the command ran during that package, and not a description of the tree today.
>
> **Current measured status lives in exactly one place:**
> [`docs/planning/STATE_OF_THE_PROJECT.md` §1](../../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).


A hardening pass emits no telemetry. What it produces is a set of gates, and the useful monitoring
question is: **when one of these goes red, what has actually happened?**

Every finding in this pass was a claim that was true when written and stopped being true without
anything noticing. So each row below names the signal, the likely cause, and — where relevant — the
way the check itself could go quiet.

## 1. Gate signals

### `mapManifestEvidence.test.ts`

| Failing assertion | What it means |
|---|---|
| artifact missing / checksum mismatch | `artifacts/world_256.anmw` was not regenerated after a worldgen or identity change. Run `npm run gen:world-manifest`. |
| `_generated.seed`/`sourceSize` mismatch | Someone changed `sharedWorld.ts` (the app's world identity) without regenerating. **This is the important one** — it is the exact defect the gate was written for. |
| artifact header seed mismatch | The manifest and the bytes disagree, i.e. the manifest was edited by hand. |
| a view's SHA-256 mismatch | The captures are stale relative to the world. Re-run `npm run capture:views` on a GPU machine. |
| `captured` count below 8 | A capture failed and the manifest recorded the honest absence. Look at the capture run, not at this test. |

**How it could go quiet:** if `map_manifest.json` stopped being tracked. It is deliberately committed
while `artifacts/` and `animal-map.manifest.json` are generated — see `.gitignore`, which says so.

### `check:flora-footprint`

Declared canopy radii drifted from the geometry `floraGeometry.ts` builds. Almost always because
someone edited a flora mesh: the fix is to take the measured numbers, not to widen the tolerance.

**How it could go quiet:** it runs through `run_ts.mjs`, so a bundling failure would surface as a
non-zero exit with the probe's output — the wrapper checks for that rather than treating a silent
run as a pass.

### `check:bundle`

| Failing assertion | What it means |
|---|---|
| a per-chunk budget | Ordinary growth. Raising the number is a decision to make in the same commit, with a reason. |
| `index.html` statically loads the three.js chunk | Something now imports `three` or `react-three-fiber` at module scope from the dashboard entry. The 2D dashboard would pay 836 KiB for a renderer it does not use. |
| `landscape.html` does **not** reference it | Either the 3D scene stopped loading its renderer, or the chunk was renamed — in which case the assertion above has become vacuous. This row exists so it cannot. |

### `check:notice` / `check:sbom`

A dependency was added, removed or bumped without regenerating. Both read
`cargo tree --features desktop` and the npm production closure, so they also go red when a *feature*
changes what ships — which is the point.

**How they could go quiet:** by resolving the wrong graph. Both pass `--features desktop` explicitly;
without it `cargo metadata` resolves the default graph, versions differ (measured: `phf` is 0.13.1
default vs 0.11.3 under `desktop`), keys miss, and misses used to become "UNKNOWN licence". Eighteen
crates were reported unlicensed for exactly that reason.

### `console_hygiene.spec.ts`

The app is emitting warnings of its own. The two this pass removed both fired *per frame*, so a
regression is loud rather than subtle: the failure message groups by text with a count.

**How it could go quiet:** `ACCEPTED_THIRD_PARTY` growing. It has one entry (`THREE.Clock`, owned by
react-three-fiber), listed individually with the reason it cannot be fixed here, precisely so that
adding to it is a visible act in a diff rather than a widened pattern.

### `ipcBindingAuthority.test.ts`

| Failing assertion | What it means |
|---|---|
| a consumer re-declares a generated type | A hand-written mirror came back. It sits outside the ts-rs drift gate by construction. |
| `App.tsx` no longer imports from `types/generated/` | The positive control. Without it the check above would pass for a file that stopped using the types at all. |
| the hand-written count ≠ 4 | Either a new unprotected payload appeared, or `LineageGraphState`/`MigrationPayload` gained a ts-rs source — in which case lower the number in the same commit. |

### `real_backend.spec.ts`

Only declares tests when `ANIMA_E2E_REQUIRE_BACKEND=1`. If it is failing, that flag is set on a job
that lacks either the release binary or a WebDriver endpoint. That is the intended behaviour: it
fails closed rather than skipping, because a permanent amber is how the previous five specs stayed
green for months.

## 2. Signals that a fix has regressed, which no gate covers

Stated because "we have a test" is not the same as "we would notice".

| Regression | Would a gate catch it? | What to watch |
|---|---|---|
| Spawn lands in foliage again on a **future** world | Partly. `floraClearance.test.ts` runs `findSpawn` at the shipped 2048² identity and asserts clearance, so a change to the picker or the policy is caught. A change to *worldgen* that makes every scored cell unclear falls to the deterministic push-out fallback, which is correct but untested against a real dense world. | The `spawn` canonical capture. It is a photograph of exactly this. |
| The app fails to boot under the CSP | **No.** `check:csp` validates artifacts against the declared policy, not a running app. | A human `npm run tauri:dev`; see the deployment doc §2.1. |
| Autosave silently stops being written | **No.** The exit handler is best-effort by design — there is nobody left to tell — and it is not exercised by any test. | `saves/autosave.json` mtime after a normal quit. |
| Legacy import used as a write path | Structurally impossible: `resolve_save_path` is the only resolver the write paths use and it cannot produce a path in the import directory. Asserted by `nothing_writes_into_the_legacy_import_directory`. | — |
| Canonical captures drift from the world | Yes, via the manifest checksums — but only after someone regenerates the manifest. A world change with **no** regeneration turns the artifact checksum red first. | `mapManifestEvidence` is the first thing to go red on any world change. |

## 3. What to re-measure, and when

Numbers in this pass that were measured rather than assumed, and would need re-measuring if the
inputs move:

| Measurement | Value | Re-measure when |
|---|---|---|
| Capture throughput, software vs hardware GL | 0.27 fps vs 46.7 fps | The scene's cost changes materially (flora count, shadow pass), or a Playwright/Chromium major. |
| The historic spawn's geometry | canopy R=1.340 at D=1.896 → 90° subtended | Never — it is recorded evidence about a specific reproduction, and `floraClearance.test.ts` states it as geometry rather than as a coordinate so a world change cannot invalidate it. |
| Per-route JS fetched | `/` 17 files, no three.js chunk; `/landscape.html` fetches it | Any change to entry-level imports. `check:bundle` asserts the property, so this is a re-derivation of the number, not of the fact. |
| Flora canopy unit radii | Pine 0.55 … Acacia 0.95 | Automatically, by `check:flora-footprint`, on every CI run. |
| npm production closure | 45 packages | Automatically, by `check:notice`. |
