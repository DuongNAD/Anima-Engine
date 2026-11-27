---
title: Release Checklist (0.9)
status: active
owner: maintainers
last_reviewed: 2026-11-27
review_cycle: per-release
---

# Release Checklist (`0.9`)

Every technical gate, compliance verification, World v2 / Iris Xe check, installer build step, and human governance step required before cutting `v0.9.0-rc.1` or tagging `v0.9.0`. Every npm script below maps directly to [`package.json`](../../package.json); authoritative gate status lives in [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).

> **Target `0.9` Release Sequence (`QD-FOCUS FOCUS-2` `#314`, `WB-D4 (a)` `#295`, `MAP-D8` `#331`/`#332`):**
> `WV2-BIG-HYDRO` (`#327`) + `WV2-BIG-FLORA` (`#320`) + `G-SHAPE` diagnosis (`#322`/`#323`) → `WV2-BIG-3` (`#332`, `8d58a580`, `MAP-D8` `9/15` accepted) replaced provisional `C01` once (kept as `wv2_c01_40960.toml` `288` B, SHA-256 `f460fa73…`; `81.92 km` at `4096²` / `20 m` cells, `world_sha256 = 01956959…`, `sim_world_identity = 1d313005…`, `sim_256.anmw3` `1,445,892` B, `release_archive_sha256 = 4fdad1df…`) → one public tile-pack release (`world_v2_1d313005_tiles.tar.gz`, `190,071,986` B, SHA-256 `4fdad1df…` — **published by owner on pre-release `v0.9.0-internal`, `2026-11-27T04:48:13Z`, targeting `6c6b86c7`**) → Dell Intel Iris Xe performance gate (`#289`, **PASS with flags** `"Chấp nhận ĐẠT"`, `#341` `aeb51835` / `#342` `874a53fc`) → owner's go to merge `WV2-6` (`#291` @ `9c208e45`, all CI checks green, waiting on `SIM-TREES-SEA`) → real `WV2-LOOK` card (`QD-WV2-LOOK` `#293`, `LR-LOOK-1`) → `v0.9.0-rc.1` → three recorded `MP-5.1` sessions (§5.1) → `v0.9.0` on fast-forwarded `main`. Under `FOCUS-1`, `0.9` stays in scope freeze until `v0.9.0` tags.

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
  npm run check:sel2p2
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
- [ ] *(Only if legacy `world_256.anmw` bytes changed)* Regenerate `map_manifest.json` and `artifacts/world_256.anmw` together (the separate `81.92 km` `4096² @ 20 m` World v2 artifacts were set in `WV2-BIG-3` `#332` `8d58a580`: `world_sha256 = 01956959…`, `sim_world_identity = 1d313005…`, `sim_256.anmw3` `1,445,892` B, `release_archive_sha256 = 4fdad1df…` for `world_v2_1d313005_tiles.tar.gz` `190,071,986` B; kept prior `C01` preset copy `wv2_c01_40960.toml` SHA-256 `f460fa73…`):
  ```powershell
  npm run gen:world-manifest
  ```

---

## 4. World v2, Hardware Gate, Installer Builds & Clean-Machine Verification (`MP-5.3`)

- [x] Complete `WV2-BIG-HYDRO` (`#327`), `WV2-BIG-FLORA` (`#320`), and `G-SHAPE` diagnosis (`#322`/`#323`) (`QD-FOCUS FOCUS-2` step 1, `#314`), obtain owner `yes` (`MAP-D8`, `9/15`), and merge `WV2-BIG-3` (`#332`, `8d58a580`) replacing provisional `C01` once (preserved as a byte copy at `src-tauri/crates/anima-worldgen/presets/wv2_c01_40960.toml`, `288` B, SHA-256 `f460fa73b5a9d458d1ce8eeef1ca7be47042ea38fba3162ece3d94ea92fd3f2e`) with the `81.92 km` (`4096² @ 20 m`) `BIG_4096_v2` world (`WB-D1` / `WB-D2`, `#295`: `world_sha256 = 01956959c751458d279a9e675046de5627fb6095e1451138c41d750a7f88d821`, `sim_world_identity = 1d3130054d6939425003817fccf78b284e34632b64b7443e5d6b380e60203e31`, `sim_256.anmw3` = `1,445,892` B, `index.json` = `936,863` B / `3,817` entries / `3,560` `.ktx2` tiles, `release_archive_sha256 = 4fdad1df3acd6ad32075365e754fbb56e5c511ff01ebaa47ae14d9267c7d7d0f`).
- [x] Obtain owner's final `yes` naming `world_v2_1d313005_tiles.tar.gz` (`190,071,986` B, SHA-256 `4fdad1df3acd6ad32075365e754fbb56e5c511ff01ebaa47ae14d9267c7d7d0f`, `QD-1126AM` `#314`) and publish the single public World v2 tile pack once on GitHub Releases before `WV2-6` CI / `v0.9.0-rc.1` (`QD-GATES-1125 (4)` `#288`, `FOCUS-2` step 3 complete: published by owner as pre-release [`v0.9.0-internal`](https://github.com/DuongNAD/Anima-Engine/releases/tag/v0.9.0-internal) on `2026-11-27T04:48:13Z` targeting `6c6b86c7c859aa03f55a346bf12deb6bdec815ec`).
- [x] Run the Dell Intel Iris Xe performance gate ([`2026-11-26-wv2-6-iris-xe-runbook.md`](../ai/planning/2026-11-26-wv2-6-iris-xe-runbook.md) §6, `#342` `874a53fc`; owner decision `"Chấp nhận ĐẠT"` **PASS with flags** recorded in `#341` `aeb51835` item `(5)` and hub `test_runs/anima-dell_IRISXE-RUN/SUMMARY.md`: `6/6` valid runs, `100%` Intel Iris Xe 3D work, AC overlay `Balanced`, Median FPS `21.51 -> 64.94` PASS, `p5` FPS `7.81 -> 24.57` PASS, draw-call peak `20 -> 27 <= 60` PASS). **Flags:** (1) `A1` `p5` `24.57 < 25` (`NR-D1`); (2) every run `>20%` away from its build's other runs; (3) Antigravity & Task Manager open during runs; (4) one `msedgewebview2` process grew to `4.35–4.65 GB` (`WEBVIEW-MEM`); (5) app does not exit within `20 s` of window closing (`APP-EXIT`).
- [ ] Decide on merging `WV2-6` (`#291` @ `9c208e45` — every CI check green, owner's merge decision, waiting on `SIM-TREES-SEA` check of `A1`), then land the real `WV2-LOOK` rendering card (`QD-WV2-LOOK` `#293`, `LR-LOOK-1` `#314`).
- [ ] Verify the merged CI `Release Build` workflow ([`.github/workflows/release-build.yml`](../../.github/workflows/release-build.yml), `#233`; proven green in run `45268921473`) completes green on **both** `windows-latest` and `macos-latest` (`QD-M4`: macOS unsigned if `NM-2` checksums match Windows class, `#255`) and uploads non-empty artifacts (`anima-engine-windows-installers` with `.msi`/`.exe`; `anima-engine-macos-bundles` with `.dmg`/`.app`; 14-day retention for `v0.9.0-rc.1` per `QD-REL09` G3).
- [ ] *(Local build alternative)* Build installers strictly via `npm run tauri:build` (passes `--features desktop`; never bare `tauri build`).
- [ ] Install the built Windows (and macOS per `QD-M4`) installer on a clean machine following [`INSTALL.md`](INSTALL.md), verify launch + **Start Simulation** + **Landscape Showcase** (`~100` target animals per `QD-SCALE-TRIAL`), and commit the installation record under `docs/ai/deployment/` (`MP-5.3`).

---

## 5. Stage 5 Governance & `MP-5.1` External User Sessions

Per [`DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md` §5](../planning/DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md#5-lộ-trình-theo-chặng) and [`0.9.0.md` §1](../release/0.9.0.md#1-release-path-and-owner-gates):

- [x] Decision baseline recorded: `QD-REL09` (`G1 = B` `v0.9.0-rc.1` first -> `v0.9.0`, `G2 = A` unsigned installers, `G3 = C` workflow artifacts for RC / GitHub Release for `v0.9.0`, `G4 = A` owner fast-forwards `main` before tagging), `QD-OPENC` (`QD-M4` macOS unsigned if `NM-2` matches Windows class, `QD-M8` signing before `v1.0.0`, `MP-5.6` bundled sample world in `0.9` / creature export in `1.0`), `QD-WV2-D2` (`WB-D1..WB-D8`), and `QD-FOCUS` (`FOCUS-1..6`, [`AGENT_WORKFLOW.md` §4](../how-to/AGENT_WORKFLOW.md#4-khối-decisions-để-dán-vào-prompt-0)).
- [x] Land `MP-5.2` repository community files (`SECURITY.md` with GitHub private vulnerability reporting under `QD-GATES-1127 (8)` `#336`, PR template, and trial feedback channel — merged in `#335`, `e6316704`, `verdicts/MP-5.2-DOCS_e9799fbd.md`).
- [ ] Complete `WV2-LOOK` (`FOCUS-2` step 5) and cut `v0.9.0-rc.1`, then run and record **at least three `MP-5.1` external-user install & first-run sessions** following §5.1 below.
- [ ] Address `MP-5.1` trial blockers (keeping `FOCUS-1` scope freeze), owner fast-forwards `main` from verified `feat/ecological-selection`, owner bumps version & tags `v0.9.0` (`G4`), and owner publishes GitHub Release artifacts + single public World v2 tile pack (`world_v2_1d313005_tiles.tar.gz`, `190,071,986` B, SHA-256 `4fdad1df3acd6ad32075365e754fbb56e5c511ff01ebaa47ae14d9267c7d7d0f` — published on pre-release `v0.9.0-internal` `2026-11-27` and linked for `v0.9.0`) matching the tagged commit and hashes (`world_sha256 01956959…`, `sim_world_identity 1d313005…`).

### 5.1 `MP-5.1` External-User Session Protocol

Each of the three required `MP-5.1` sessions validates unguided onboarding and first-run clarity on **`v0.9.0-rc.1`** (or trial candidate) under [`OWNER_TRIAL_RUN.md`](OWNER_TRIAL_RUN.md):

1. **What the tester gets:**
   - The GitHub Actions **Release Build** workflow run link for `v0.9.0-rc.1` (unsigned Windows `.msi`/`.exe`, or macOS `.dmg`/`.app` if `QD-M4` applies; `QD-REL09` G1–G3 `#255`) plus the expected commit SHA, `sim_world_identity` (`1d3130054d6939425003817fccf78b284e34632b64b7443e5d6b380e60203e31`), and release archive SHA (`4fdad1df3acd6ad32075365e754fbb56e5c511ff01ebaa47ae14d9267c7d7d0f`).
   - Access to the single public `81.92 km` (`4096² @ 20 m`) World v2 tile pack (`world_v2_1d313005_tiles.tar.gz`, `190,071,986` B, SHA-256 `4fdad1df…`, published under the owner's release gate at `FOCUS-2` step 3 on pre-release `v0.9.0-internal` on `2026-11-27`; pre-merge `QD-TRIAL-V3` local builds remain internal only and are not given to external `MP-5.1` testers).
   - [`INSTALL.md`](INSTALL.md) steps for WebView2 prerequisites, bypassing unsigned SmartScreen/Gatekeeper warnings, AC power guidance, **Low** quality recommendation for integrated GPUs, and `ANIMA_FOUNDING_POPULATION` target guidance (`~100` shared-trial target per `QD-SCALE-TRIAL` `#255`; `200` recommended in [`OWNER_TRIAL_RUN.md`](OWNER_TRIAL_RUN.md) when launching with an env var).
   - Honest scope framing from [`OWNER_TRIAL_RUN.md` §4](OWNER_TRIAL_RUN.md#4-what-to-tell-your-friend-honest-framing) and [`0.9.0.md` §3–4](../release/0.9.0.md#3-scientific-status-strictly-bounded-by-merged-resultmd) (early pre-1.0 prototype; background fauna/caves carry `"Decoration, not simulated"` labels per `C14` `#269`; do not claim unmeasured `"autonomous living AI"`).
2. **What the owner watches and records (without prematurely rescuing):**
   - Observe **where they stopped** (every hesitation, UI dead-end, WebView2/SmartScreen/Gatekeeper friction, startup wait before **Start Simulation** activates, camera/speed confusion, performance drop, or crash/quarantine).
   - Record verbatim **what they asked** (terms, controls, genotype/decoration labels, or installation steps they did not understand without explanation).
   - Record host OS/GPU, graphics tier preset, approximate FPS/smoothness, founder setting, and any screenshots or `trial-log.txt` if an error occurred.
3. **Where session notes go:**
   - Write each session record (and any `MP-5.3` clean-machine install verification) under **`docs/ai/deployment/`** (committed via PR without user PII or secrets, linked from release verification before bumping `v0.9.0-rc.1` to `v0.9.0`).
