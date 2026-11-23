---
title: Why the Gates Exist
status: active
owner: architecture
last_reviewed: 2026-11-23
review_cycle: quarterly
---

# Why the Gates Exist

This document explains **which real failure mode each check (gate) in Anima Engine prevents** — especially *"exit 0 while wrong"* failures where suites appear green even though a guarantee broke. Authoritative counts, test totals, and verification history remain in the single canonical sources:

- Why the gate suite is trusted: [`STATE_OF_THE_PROJECT.md` §2.1](../planning/STATE_OF_THE_PROJECT.md#21-vì-sao-bộ-gate-này-đáng-tin) and measurement tables in [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).
- Six historical process failures that shaped the gate architecture: [`COMPLETION_ROADMAP_2026-10-16.md` §1.2](../planning/COMPLETION_ROADMAP_2026-10-16.md#12-sáu-lỗi-quy-trình-đã-dẫn-tới-đây--mỗi-lỗi-thành-một-gate-ở-ht04).
- Findings ledger (including tracked issues such as `F-026` and historical gates such as `F-054`): [`FINDINGS.md`](../../FINDINGS.md).
- Paper data tables (`determinism_matrix.md`, `scaling.md`, `eu_residual.md`): [`docs/paper/data/README.md`](../paper/data/README.md).

---

## 1. Core Principle: Preventing "Exit 0 While Wrong"

Historically, the largest risk in this repository was not noisy crashes that turn CI red immediately, but **false green signals**: empty feature-gated test binaries reporting `running 0 tests` and exiting `0`; frontend mocks swallowing mismatched Rust IPC field names; agents editing source code or loosening assertions; or a gate script's regex silently failing to match anything.

To close these loopholes, the gate system across `.github/workflows/ci.yml`, `.github/workflows/hardening.yml`, `justfile`, and `scripts/` repeats four structural patterns:

1. **Inline negative controls (`selfTest` / `--self-test`):** five fast gates (`check_docs_metadata.mjs`, `check_ipc_reference.mjs`, `check_root_hygiene.mjs`, `check_npm_audit.mjs`, `check_e2e_specs.mjs`) run synthetic fault fixtures *before* scanning the repository and exit `2` if their own detector misses a fault; meanwhile `compare_checksums.mjs --self-test` (exits `2` on failure) and `verify_e3_artifacts.mjs --self-test` / `verify_esel1_artifacts.mjs --self-test` / `verify_sel2p_artifacts.mjs --self-test` (exit `1` on failure) expose explicit `--self-test` suites.
2. **Bidirectional allow-lists with mandatory reasons:** `test_target_policy.mjs`, `root_allowlist.json`, `npm_audit_policy.mjs`, `e2e_spec_policy.mjs`, and `NO_FRONT_MATTER` in `check_docs_metadata.mjs` reject both unlisted entries and stale exemptions so exception lists cannot silently accumulate or rot.
3. **Cross-boundary / cross-process oracles:** subsystems do not self-certify (`cargo tree` inspects the resolved dependency graph instead of trusting compilation alone; `node:crypto` and `git cat-file` re-hash Rust experiment artifacts; `verify_dataflow.py` reconciles `dataflow.json` against `cargo modules`; `check_flora_footprint.mjs` runs real `three` in Node instead of Vitest mocks).
4. **One-way ratchets and anti-cheat guards:** locking floors/ceilings (`eslint_ratchet.mjs` anchored at `0`, `check_bundle_budget.mjs` size and route-split limits) alongside agent verification rules in `scripts/verify.sh` (Rules 1–8 including Rule 4b).

---

## 2. Group 1 — Build, Feature Isolation, and Rust Supply-Chain Gates

- **Pinned toolchain (`rust-toolchain.toml` in `ci.yml`, `hardening.yml`, `determinism-matrix.yml`):** allowing floating compiler updates (such as `rustc 1.98` adding new clippy lints or `1.100.0` drifting from the CI pin) turns untouched code red and invalidates Windows build caches. Pinning a single version in `rust-toolchain.toml` and keying caches on its hash ensures CI failures reflect repository changes.
- **Formatting and security advisories (`cargo fmt --check`, `cargo audit`, `cargo deny check`):** `cargo audit` (configured by `src-tauri/.cargo/audit.toml`) and `cargo deny check` (`src-tauri/deny.toml`) enforce approved licenses (`MIT OR Apache-2.0` plus vetted exemptions) and block transitive vulnerabilities (such as earlier `burn/train` transitive pulls). Both tools remain intentional complements: as [`FINDINGS.md` (`F-032`/`F-033`)](../../FINDINGS.md#f-033--miễn-trừ-tokio-vừa-viết-vào-denytoml-có-bản-vá-trong-tầm-với-1533) documents, `cargo audit` reads the lockfile directly while `cargo deny` evaluates target dependency graphs, so their coverage is not identical.
- **Crate & feature boundary checks via `cargo tree` (`.github/workflows/ci.yml`):** alongside `cargo clippy --all-targets --features desktop -- -D warnings`, `cargo clippy -p anima-worldgen --all-targets --features reader,generate,bake -- -D warnings`, and `cargo clippy --all-targets --no-default-features -- -D warnings`, CI runs three separate `cargo tree` assertions requiring **0 matches**:
  - workspace `--no-default-features` forbids 10 optional heavy families (`neo4rs`, `tokio-tungstenite`, `burn-wgpu`, `naga`, `zstd-sys`, `wgpu`, `ktx2`, `meshopt`, `bc7enc`, `cubecl-wgpu`);
  - default `anima-engine` (`cargo tree -p anima-engine`) forbids those 10 families **and** `anima-worldgen`;
  - `anima-worldgen --features reader,generate,bake` forbids `bevy_ecs`, `tauri`, and `wgpu`.
  Compile checks alone miss transitive `Cargo.toml` leaks (when no Rust source imports the leaked crate under default features): `cargo build` and `cargo clippy` pass while shipping unwanted crates in the dependency tree.

---

## 3. Group 2 — Test Execution Coverage & Anti-Empty-Target Gates

- **`scripts/check_test_targets.mjs` + `scripts/test_target_policy.mjs`:** when running `cargo test --features desktop` and `cargo test -p anima-worldgen --features "reader,generate,bake"`, four silent failures can still exit `0` or skip coverage:
  1. A test binary aborts between `Running ...` and `running N tests` (e.g., Windows DLL load failures).
  2. A test target with `#![cfg(feature = "...")]` compiles without its feature flag into an empty binary that prints `running 0 tests` and exits `0`.
  3. Tests are silenced with `#[ignore]` to bypass failures.
  4. `required-features` in `Cargo.toml` is misspelled or an exemption in policy goes stale.
  `test_target_policy.mjs` enforces bidirectional `EMPTY_TARGETS`, `IGNORED_TESTS`, `FEATURE_GATED_TARGETS`, and `PROFILES` (`default`, `desktop`, `worldgen-cpu`) lists with a 40-character minimum reason per exemption. Earlier history recorded `check_test_targets.mjs` itself committed with unresolved conflict markers ([`FINDINGS.md` (`F-017`)](../../FINDINGS.md)).
- **`src-tauri/tests/feature_gated_targets_tests.rs`:** cross-checks crate-level `#![cfg(feature = "...")]` attributes, `Cargo.toml` `[[test]] required-features` entries, and `scripts/test_target_policy.mjs` inside `cargo test` itself.

---

## 4. Group 3 — Determinism, Schedule Order, and Local Hardening Gates

- **Preventing determinism drift and ECS schedule ambiguity (`hardening.yml` & Rust gates):**
  - `src-tauri/tests/determinism_tests.rs` pins the 200-tick golden `world_hash`, includes the anti-vacuity `world_hash_bat_duoc_thay_doi_that` check (proving the hash changes when world state changes), and checks `build_tick_schedule` determinism.
  - `src-tauri/tests/determinism_gate_tests.rs` spawns `child_entry_point` in **separate OS processes** to detect cross-process leaks (`HashMap` `RandomState`, allocator addresses, `Uuid::new_v4()`).
  - `src-tauri/tests/sim_determinism_tests.rs` verifies `SimRng` / `derived_rng`, enforces `no_process_random_source_in_backend_sources`, and checks `live_simulation_does_not_bypass_the_deterministic_id_gate` against `next_entity_id`.
  - `src-tauri/tests/schedule_ambiguity_tests.rs` and `src-tauri/tests/schedule_total_order_tests.rs` (plus `f041_child_entry_point` across subprocesses) enforce the lesson of [`FINDINGS.md` (`F-041`)](../../FINDINGS.md): without total system ordering and `auto_insert_apply_deferred = false`, identical seeds can hash differently across processes.
  - `.github/workflows/hardening.yml` runs `cargo test --release`, repeats the release test suite 5 times under `RAYON_NUM_THREADS=1` (locally via `just flaky` / `scripts/flaky_check.sh`), verifies `determinism_tests`, and scans **both `src-tauri/src` and `src-tauri/crates`** to ban non-deterministic `(thread_rng|ThreadRng)` and `rand::rng` calls in source files.
- **Measurement workflows vs blocking gates:** `.github/workflows/determinism-matrix.yml` runs `cross_platform_checksum_probe` alongside `scripts/paper/compare_checksums.mjs --self-test` across Windows, Linux, and macOS to **measure** cross-platform evidence for [`docs/paper/data/README.md`](../paper/data/README.md); it does not act as a PR merge blocker.
- **Local anti-cheat harness (`justfile`, `scripts/verify.sh`, `scripts/perf_gate.sh`):** `just verify` (`scripts/verify.sh`) enforces agent verification rules (Rules 1–8 and Rule 4b):
  1. Restricts writes to `<crate>/tests/` and `FINDINGS.md`, using `git add --intent-to-add` so untracked test files are checked ([`FINDINGS.md` (`F-024`)](../../FINDINGS.md#f-024--cổng-verify-tự-hủy-im-lặng-khi-diff-không-chứa-file-test-nào)).
  2. Bans `#[ignore]` and `#[should_panic]`.
  3. Bans deleting existing assertions.
  4. Bans non-deterministic test primitives (`thread_rng`, `rand::random`, `SystemTime::now`, `Instant::now`) and `unsafe` / `transmute` / `from_raw_parts` field-bypass tricks.
  5. Rejects empty error checks (`is_ok()` alone) and requires adding real assertions on mutant-killer tasks.
  6. Requires 3 consecutive `--release` passes plus `cargo mutants` verification confirming the target mutant is killed (rejecting both `"Found 0 mutants"` and timeouts).
  `just perf-gate` (`scripts/perf_gate.sh`) automatically reverts optimization edits via `git checkout` if tests break or Criterion shows no statistically significant win.

---

## 5. Group 4 — IPC Boundary, Module Architecture, and Frontend Gates

- **Rust ↔ TypeScript parity (`ts-rs`, `ipc_registration_tests.rs`, `check_ipc_arg_case.mjs`):**
  - `cargo test --lib export_bindings` + `git diff --exit-code` on `src/types/generated` ensures any change to Rust IPC payloads (`SimulationTickPayload`, `SimulationStatus`, etc.) includes regenerated `.ts` bindings — preventing drift like the former `head_directions` map/array mismatch.
  - `src-tauri/tests/ipc_registration_tests.rs` proves every `#[tauri::command]` is wired into `tauri::generate_handler!`, catching missing handler registrations invisible to compilers and UI mocks until runtime (`Unknown command`).
  - `scripts/check_ipc_arg_case.mjs` (`npm run check:ipc`) validates object keys in frontend `invoke(...)` calls against Rust command parameter names, because `tauri-macros` expects `camelCase`: passing `snake_case` silently drops `Option<T>` parameters into `None` while Vitest mocks pass.
- **Module dependency graph (`scripts/verify_dataflow.py`, `dataflow.json`):** compares declared architecture edges in `dataflow.json` against `cargo modules` (after filtering `"owns"` edges) and fails on unexpected coupling or bidirectional cycles (note [`FINDINGS.md` (`F-026`)](../../FINDINGS.md) tracks an open `justfile` `--count` summary bug in the local `dataflow-truth` recipe).
- **Frontend quality & security (`ci.yml`):**
  - `scripts/check_npm_audit.mjs` + `scripts/npm_audit_policy.mjs`: separates justified `DEV_ONLY_EXEMPTIONS` and runs `npm audit --omit=dev` (avoiding dev-only false alarms like `picomatch`).
  - `scripts/eslint_ratchet.mjs`: locks `DEFAULT_BASELINE = 0` so ESLint warnings cannot regress.
  - `npm run typecheck:tests`: runs `tsc -p tests/tsconfig.json --noEmit` because Vite builds only check `src/` and Vitest transpiles without typechecking.
  - `scripts/check_csp_compat.mjs`: requires strict CSP (`default-src 'self'`, no `'unsafe-inline'`/`'unsafe-eval'` in `script-src`) in `tauri.conf.json` and scans `dist/*.html` for inline `<script>` or external CDN tags (as seen in `public/ecosystem.html`).
  - `scripts/check_bundle_budget.mjs`: enforces per-chunk (`CHUNK_BUDGET_KIB`) and total bundle (`TOTAL_BUDGET_KIB`) limits and enforces route splitting so the 2D dashboard (`index.html`) never eagerly pulls Three.js while 3D routes (`landscape.html`, `map2.html`) do. Following `F-054` (when filename prefix `THREE_D_CHUNK` alone missed direct `three` imports folded into chunks like `defineProperty-*.js`), PR `#197` (`427a0e6`) added content-based detection (`THREE_D_CONTENT` / `hasThreeJsSignature(chunk.content)`) alongside `THREE_D_CHUNK` so route-split leaks are caught regardless of bundler chunk naming.
  - `scripts/check_flora_footprint.mjs`: executes against real `three` geometry in Node to validate `FLORA_FOOTPRINT_UNIT_RADIUS`, closing the gap left by Vitest mocking `@react-three/fiber` / canvas rendering.
  - `scripts/check_e2e_specs.mjs` + `scripts/e2e_spec_policy.mjs` (`SCHEDULED_SPECS`, `EXCLUDED_SPECS`) + `tests/e2e/tauri-mock.ts` + `tests/e2e/real_backend.spec.ts` + `tests/frontend/mapManifestEvidence.test.ts`: prevents `test:e2e` from silently narrowing to a subset of specs, avoids attaching to another process's Vite port, and hashes canonical `map_manifest.json` PNG evidence.

---

## 6. Group 5 — Experiment Evidence & Preregistration Gates

In computational experiments, a primary failure mode is **"plausible numbers from stale, dummy, or silently edited inputs"** (such as 1×1 placeholder images, hardcoded seed `42`, or post-hoc fixture edits recorded in [`COMPLETION_ROADMAP_2026-10-16.md` §1.2](../planning/COMPLETION_ROADMAP_2026-10-16.md#12-sáu-lỗi-quy-trình-đã-dẫn-tới-đây--mỗi-lỗi-thành-một-gate-ở-ht04)). These gates enforce cross-language verification:

- **`scripts/verify_e2_artifacts.mjs` (`npm run check:e2`):** recomputes every file digest in `checksums.sha256` with `node:crypto` (independent of Rust SHA-256 code), reads committed preregistered manifests directly from git blobs (`git cat-file blob HEAD:...`) to resist Windows CRLF noise, ensures `seed_order` excludes `smoke_seed`, reconciles `effects.json` against `paired-report.json` and `per-seed-deltas.csv`, checks `execution-lock.json`, rejects `cargo run` in `reproduction_command`, and checks `E2-G1`, `E2-G3`, `E2-G4`, and `E2-G6`.
- **`scripts/verify_e3_artifacts.mjs` (`npm run check:e3`), `scripts/verify_esel1_artifacts.mjs`, `scripts/verify_sel2p_artifacts.mjs`:** apply analogous independent JS verification to E3 and ecological-selection artifacts.
- **Backend Rust gates (`e2_seam_tests.rs`, `prereg_e2_manifest_tests.rs`, `e2_frozen_manifest_tests.rs`):** verify `E2-G1`/`E2-G3`/`E2-G6`, enforce `the_preregistered_reproduction_never_launches_the_app`, and pin byte-identity of frozen manifests directly inside `cargo test` (`e2_frozen_manifest_tests.rs`).

---

## 7. Group 6 — Compliance, Repository Hygiene, and Documentation Gates

- **Licensing & real shipped closures (`check_bundle_closure.mjs`, `gen_third_party_licenses.mjs`, `gen_sbom.mjs`, `check_sbom_schema.mjs`, `gen_notice.mjs`, `verify_upstream_licenses.mjs`):** `npm ls` misses bundler-tree-shaken closures in `dist/` (`vite`/`rolldown`) while counting dev-only packages; `check_bundle_closure.mjs` compares the real built closure in `licensing/bundle-closure.json`. `check_sbom_schema.mjs` validates `sbom.cdx.json` offline against SHA-256-pinned CycloneDX schemas at `schemas/cyclonedx/PROVENANCE.json`; these compliance artifacts are generated and checked on Windows CI because `cargo tree --target` resolves proc-macros using the host target ([`FINDINGS.md` (`F-025`)](../../FINDINGS.md#f-025--cargo-tree---target-x-không-độc-lập-host-proc-macro-được-resolve-theo-host-nên-artifact-licence-không-sinh-được-từ-máy-khác)). `gen_notice.mjs --check` guards `NOTICE` against stale license text or inventory drift.
- **Text & root hygiene (`check_text_hygiene.mjs`, `check_root_hygiene.mjs` + `root_allowlist.json`):** `check_text_hygiene.mjs` rejects bare ASCII control characters (preventing NUL-byte corruption that once turned `.mjs` scripts into binary diffs during review). `check_root_hygiene.mjs` enforces `root_allowlist.json` bi-directionally to keep temporary scripts out of the repository root.
- **Anti-doc-rot gates (`check_docs_links.mjs`, `check_docs_metadata.mjs`, `check_ipc_reference.mjs`):**
  - `check_docs_links.mjs` validates relative Markdown file links and `#anchor` targets (supporting Unicode/Vietnamese headings via `\p{L}\p{N}`).
  - `check_docs_metadata.mjs` enforces YAML front matter, restricts `status: completed` to `docs/ai/`, requires a `README.md` index in every `docs/` directory, and checks generated index consistency.
  - `check_ipc_reference.mjs` enforces two-way agreement between `docs/reference/IPC_CONTRACT.md` and actual `generate_handler!` / `.emit*(` calls in Rust.

---

## 8. Current Scope and Evidence In Flight

- **Tracked issues:** remaining open findings (such as `F-026` on local `just dataflow-truth --count`) and historical fixed entries (`F-054`) are tracked transparently in [`FINDINGS.md`](../../FINDINGS.md).
- **Upcoming paper tables:** existing trunk data tables live in [`docs/paper/data/README.md`](../paper/data/README.md) (`determinism_matrix.md`, `scaling.md`, `eu_residual.md`). The historical P1 incident ledger and full M2 gate negative-control matrix are planned work items under [`2026-11-05-swarm-master-plan.md`](../ai/planning/2026-11-05-swarm-master-plan.md) (in flight on dedicated PRs, not yet landed on trunk).
