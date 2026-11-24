---
title: Release Checklist (0.9)
status: active
owner: maintainers
last_reviewed: 2026-11-24
review_cycle: per-release
---

# Release Checklist (`0.9`)

Every technical gate, compliance verification, installer build check, and human release step required before cutting a `v0.9.0` tag or shipping trial installers. Every npm script below maps directly to [`package.json`](../../package.json); authoritative gate status lives in [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).

---

## 1. Frontend, Docs & Integrity Gates (Run Sequentially from Repo Root)

Run one Node command at a time on constrained hosts (see [`STATE_OF_THE_PROJECT.md` §4](../planning/STATE_OF_THE_PROJECT.md#4-bẫy-đã-biết--đọc-trước-khi-sửa)):

- [ ] Clean install both npm packages:
  ```powershell
  npm ci
  npm ci --prefix tests
  ```
- [ ] Dependency vulnerability gate (root & `tests/`, policy in `scripts/npm_audit_policy.mjs`):
  ```powershell
  npm run check:audit
  ```
- [ ] Text, root, and conflict-marker hygiene:
  ```powershell
  npm run check:text-hygiene
  npm run check:root-hygiene
  npm run check:conflict-markers
  ```
- [ ] Linting, zero-warning ratchet, and test-package typecheck:
  ```powershell
  npm run lint
  node scripts/eslint_ratchet.mjs
  npm run typecheck:tests
  ```
- [ ] Unit, frontend, and production build suites:
  ```powershell
  npm run test
  npm run test:frontend
  npm run build
  ```
- [ ] Built-artifact closure, CSP, and bundle route-split budgets (`dist/` required):
  ```powershell
  npm run check:bundle-closure
  npm run check:csp
  npm run check:bundle
  ```
- [ ] E2E spec parity & Playwright suite:
  ```powershell
  node scripts/check_e2e_specs.mjs
  npm run test:e2e
  ```
- [ ] Documentation, IPC contract, and doc-command gates:
  ```powershell
  npm run check:ipc
  npm run check:docs
  node scripts/check_doc_commands.mjs --static
  npm run check:doc-commands
  ```
- [ ] Preregistered experiment artifact checks:
  ```powershell
  npm run check:e2
  npm run check:e3
  npm run check:e3b
  ```

---

## 2. Backend Rust Gates (Run from `src-tauri/` unless noted)

On 16 GiB Windows hosts (`DESKTOP-IT8UKCD`), enforce `AGENT_RULES.md` §5: wait until zero `cargo`/`rustc` processes appear twice `60 s` apart, `>= 5 GiB` free physical RAM, `>= 20 GiB` free disk, `CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, and low priority; never run two Cargo commands back to back without re-checking.

- [ ] Format, advisory audit, and license deny check:
  ```powershell
  cargo fmt --check
  cargo audit
  cargo deny check
  ```
- [ ] Clippy across desktop, `anima-worldgen`, and `--no-default-features` profiles:
  ```powershell
  cargo clippy --all-targets --features desktop -- -D warnings
  cargo clippy -p anima-worldgen --all-targets --features reader,generate,bake -- -D warnings
  cargo clippy --all-targets --no-default-features -- -D warnings
  ```
- [ ] Verify `--no-default-features`, default `anima-engine`, and `anima-worldgen` dependency exclusions match [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) (`cargo tree` steps).
- [ ] Verify `ts-rs` bindings (`cargo test --lib export_bindings`) leave `src/types/generated/` clean (`git diff --exit-code -- ../src/types/generated`).
- [ ] Full desktop test suite + test-target policy gate (on Windows use `cmd /c` so stdout/stderr stay in write order; on macOS/Linux use `>` + `2>&1`):
  ```powershell
  cmd /c "cargo test --features desktop > ..\cargo-test-output.txt 2>&1"
  cd ..
  node scripts/check_test_targets.mjs cargo-test-output.txt --profile desktop
  ```
- [ ] `anima-worldgen` CPU test suite + test-target policy gate (from `src-tauri/`):
  ```powershell
  cmd /c "cargo test -p anima-worldgen --features reader,generate,bake > ..\cargo-test-worldgen-output.txt 2>&1"
  cd ..
  node scripts/check_test_targets.mjs cargo-test-worldgen-output.txt --profile worldgen-cpu
  ```
- [ ] Module dataflow graph parity (run from `src-tauri/`):
  ```powershell
  cargo modules dependencies --lib --no-externs --package anima-engine > ..\dataflow-truth.dot
  python ..\scripts\verify_dataflow.py ..\dataflow.json ..\dataflow-truth.dot 1
  ```
- [ ] Desktop release binary build (`--features desktop`):
  ```powershell
  cargo build --release --features desktop
  ```

---

## 3. Compliance Artifacts (Must Regenerate on Windows)

As documented in [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) and `F-025`, `cargo tree --features desktop` resolves platform-specific crates, so compliance artifacts describe a **Windows** build and must be regenerated on Windows before release checks:

- [ ] Regenerate third-party license bundle, CycloneDX SBOM, and `NOTICE` on Windows:
  ```powershell
  npm run gen:compliance
  ```
- [ ] Run every compliance, schema, upstream, and flora check from repo root:
  ```powershell
  npm run check:compliance
  npm run check:licenses-complete
  npm run verify:upstream-licenses
  npm run check:flora-footprint
  ```
- [ ] Confirm [`licensing/UNRESOLVED.md`](../../licensing/UNRESOLVED.md) reports **`0 component(s) unresolved`** and upstream license elections remain valid.
- [ ] *(Only if world artifact bytes changed)* Regenerate `map_manifest.json` and `artifacts/world_256.anmw` together:
  ```powershell
  npm run gen:world-manifest
  ```

---

## 4. Installer Builds & Clean-Machine Verification (`MP-5.3`)

- [ ] Verify the CI `Release Build` workflow (`workflow_dispatch` / PR `#233`) completes green on **both** `windows-latest` and `macos-latest` and uploads non-empty artifacts (`anima-engine-windows-installers` with `.msi`/`.exe`; `anima-engine-macos-bundles` with `.dmg`/`.app`).
- [ ] *(Local build alternative)* Build installers strictly via `npm run tauri:build` (passes `--features desktop`; never bare `tauri build`).
- [ ] Install the built Windows (and macOS if `QD-M4` selects it) installer on a clean machine following [`INSTALL.md`](INSTALL.md), verify launch + simulation start + landscape showcase, and commit the installation record under `docs/ai/deployment/` (`MP-5.3`).

---

## 5. Stage 5 Governance & `MP-5.1` External User Sessions

Per [`DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md` §5](../planning/DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md#5-lộ-trình-theo-chặng) and [`0.9.0.md` §1](../release/0.9.0.md#1-open-release-gates-owner-decisions-required-before-tagging):

- [ ] Resolve the 4 open release gates in [`0.9.0.md`](../release/0.9.0.md) (version/tag, signing `QD-M8`, publishing venue, `main` branch sync), plus `QD-M4` (second platform) and `MP-5.6` (sample world / creature export scope).
- [ ] Land `MP-5.2` repository community files (`SECURITY.md`, PR template, feedback channel).
- [ ] Run and record **at least three `MP-5.1` external-user install & first-run sessions** while observing (log where each user pauses, what questions they ask, and whether WebView2 / SmartScreen / Gatekeeper prompts were clear; see [`OWNER_TRIAL_RUN.md`](OWNER_TRIAL_RUN.md)).
- [ ] Fast-forward/sync `main` (if decided), tag `v0.9.0`, and verify downloaded release artifacts match the tagged commit.
