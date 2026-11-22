# Contributing to Anima-Engine

Thanks for your interest. This document covers the licensing terms your
contribution arrives under, the contributor and agent-swarm workflow, and every
gate a change has to pass.

> Tiếng Việt — tóm tắt điều khoản: bạn đóng góp theo đúng license của dự án
> (`MIT OR Apache-2.0`). Không có CLA, không phải ký gì, không chuyển giao
> copyright — bạn giữ bản quyền phần mình viết.

## License of contributions (inbound = outbound)

Anima-Engine is dual-licensed under **`MIT OR Apache-2.0`** — a user may choose
either. Contributions arrive under the same terms.

Unless you state otherwise in writing, any contribution you intentionally submit
for inclusion in this work, as defined in the Apache License 2.0, is dual
licensed as above, without any additional terms or conditions. This restates
Apache-2.0 §5; nothing here is an extra obligation on top of it.

There is **no CLA** and no copyright assignment. You keep the copyright in what
you wrote. Contributors are credited by the Git history, which is the record.

If a contribution includes code you did not write, say so in the pull request and
name the source and its license. Code under **GPL, AGPL, SSPL or any
source-available or unlicensed terms cannot be merged**, because it cannot be
redistributed under `MIT OR Apache-2.0`. See
[`docs/governance/OPEN_SOURCE_POLICY.md`](docs/governance/OPEN_SOURCE_POLICY.md)
for the full classification and the intake process for a new dependency.

Assets you contribute (images, icons, sounds, generated maps) fall under the same
dual license unless the pull request says otherwise and supplies the terms.

## Before you write code

Read [`CLAUDE.md`](CLAUDE.md) — it holds the rules that do not change between
sessions — and [`docs/planning/STATE_OF_THE_PROJECT.md`](docs/planning/STATE_OF_THE_PROJECT.md),
which holds what was last *measured* green and the prioritized backlog.

Some areas have required reading before the first edit, because the traps there
produce code that runs, returns finite numbers and is silently wrong:

| Touching | Read first |
|---|---|
| Genotype, phenotype, genesis, birth, epoch replacement, agent brains, action gates | [`docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md`](docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md), then [ADR-0001](docs/decisions/ADR-0001-creature-development-lifecycle.md) and [ADR-0003](docs/decisions/ADR-0003-evolved-per-agent-brains.md) |
| World laws, scenarios, exotic energy, energy pathways, lineage diagnostics, World Lab UI | [`docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md`](docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md), then [ADR-0002](docs/decisions/ADR-0002-world-laws-and-exotic-energy.md) |
| The Tauri IPC surface (commands and events) | [`docs/reference/IPC_CONTRACT.md`](docs/reference/IPC_CONTRACT.md). Add or remove a command and its row in the same commit; `npm run check:docs` fails when they disagree |
| Reproduction, locomotion or ecology in the live world (the `live.*` manifest keys) | [ADR-0005](docs/decisions/ADR-0005-live-population-turnover.md), [ADR-0006](docs/decisions/ADR-0006-locomotion-actuation.md), [ADR-0007](docs/decisions/ADR-0007-live-ecology-sustainability.md). Every key is opt-in: an absent key must leave the world exactly as it was |
| Anything under `docs/` | [`docs/governance/DOCUMENTATION_POLICY.md`](docs/governance/DOCUMENTATION_POLICY.md). A new document needs front matter and a row in its directory's README index |

If you are a coding agent (or dispatching one), also read [`AGENTS.md`](AGENTS.md)
(hardening rules and `just verify`) and [`docs/how-to/AGENT_WORKFLOW.md`](docs/how-to/AGENT_WORKFLOW.md)
(card queue, owner decisions, and per-card authorization lines).

Current code plus fresh tests win over prose. Files under `docs/archive/` are
superseded and must not be used as implementation plans.

## Where a change goes

- **PR-only policy:** The trunk is **`feat/ecological-selection`**. Open pull
  requests against `feat/ecological-selection`, not against `main`; `main` is
  fast-forwarded onto the trunk at milestones. Both branches are protected: direct
  pushes are rejected, and a pull request merges only when both the
  `Rust (src-tauri)` and `Frontend` CI jobs are green. This holds for
  documentation-only changes too.
  Note that `.github/workflows/ci.yml` triggers on `pull_request` and on `push`
  to `[main, feat/ecological-selection]` — pushing a bare feature branch without
  opening a PR runs **no** CI; opening a PR runs both jobs on the merge commit.
- **Worktree-per-task workflow:** Work in an isolated git worktree branched from
  `origin/feat/ecological-selection` so concurrent tasks never collide and the main
  checkout stays clean:
  ```bash
  git fetch origin
  git worktree add ../anima-wt/<task-id> -b <branch> origin/feat/ecological-selection
  ```
  Use `mac/...`, `win/...`, or `swarm/...` branch names for agent tasks (and
  descriptive branches for human PRs). Inside the worktree, verify
  `git rev-parse --show-toplevel` and `git branch --show-current` before editing
  or committing, never touch another task's worktree, and never leave scratch
  files (`plan.md`, `progress.md`, `audit.md`, etc.) inside the repository.
- **Stage by explicit path:** Always stage files with `git add <path>...` —
  **never** `git add -A` or `git add .` — so untracked logs, `target_*` build
  folders, or concurrent session artifacts cannot slip into a commit.
- **Never launch the desktop app in automated/agent runs:** Do not run
  `npm run tauri:dev` or `cargo run` in automated or agent sessions unless a
  human specifically needs GUI verification (`CLAUDE.md` / `AGENT_WORKFLOW.md`).

## Gates

Run the relevant gates locally before opening or updating a pull request. The
commands below match `.github/workflows/ci.yml` line for line — `Frontend` runs
on `ubuntu-latest` from the repository root `.`, and `Rust (src-tauri)` runs on
`windows-latest` from `src-tauri/` (except the compliance steps marked `.`).

### Frontend & Docs (repo root `.`, Node 22)

1. Install both npm packages (`tests/` has its own `package-lock.json` and `node_modules`):
   ```bash
   npm ci && npm ci --prefix tests
   ```
2. Dependency advisories (`scripts/npm_audit_policy.mjs`):
   ```bash
   npm run check:audit
   ```
   *(Invokes `node scripts/check_npm_audit.mjs`, auditing both root and `tests/`. High
   or critical findings fail unless named with a reason in `scripts/npm_audit_policy.mjs`
   and absent from `npm audit --omit=dev`.)*
3. Text hygiene, root hygiene, preregistered E2/E3 artifacts, and IPC argument casing:
   ```bash
   npm run check:text-hygiene && npm run check:root-hygiene && npm run check:e2 && npm run check:e3 && npm run check:ipc
   ```
   *(`check:text-hygiene` rejects raw ASCII control bytes in tracked/untracked text files;
   `check:root-hygiene` checks root entries against `scripts/root_allowlist.json`;
   `check:e3` verifies E3 artifacts match the committed preregistration;
   `check:ipc` verifies frontend `invoke` argument casing matches Rust Tauri commands.)*
4. Lint, ESLint ratchet (0 warnings), `tests/` typecheck, unit & frontend suites, production build:
   ```bash
   npm run lint && node scripts/eslint_ratchet.mjs && npm run typecheck:tests && npm run test && npm run test:frontend && npm run build
   ```
   The ESLint ratchet holds the warning baseline at 0, so any warning is a failure.
5. Post-build `dist/` gates (must run **after** `npm run build`):
   ```bash
   npm run check:bundle-closure && npm run check:csp && npm run check:bundle
   ```
   *(If a lockfile change altered the shipped JS closure, `npm run build` updates
   `licensing/bundle-closure.json` — this is OS-independent and safe to commit.)*
6. Playwright browser E2E (zero fail, zero skip):
   ```bash
   (cd tests && npx playwright install --with-deps chromium)
   node scripts/check_e2e_specs.mjs && npm run test:e2e
   ```
   *(On CI failure, `tests/test-results/` is uploaded as the `playwright-report` artifact.)*
7. Documentation & IPC contract gates:
   ```bash
   npm run check:docs
   ```
   `check:docs` runs `node scripts/check_docs_links.mjs`,
   `node scripts/check_docs_metadata.mjs`, and `node scripts/check_ipc_reference.mjs`,
   scanning untracked `.md` files too so new docs are checked before staging.

### Rust Backend (`src-tauri/`, pinned `rust-toolchain.toml`)

Run one `cargo` process at a time; a full Rust build is heavy.

1. Pinned toolchain & formatting (`Show toolchain` prints `rustc --version`, `cargo --version`, `cargo clippy --version`):
   ```bash
   rustup toolchain install
   cargo fmt --check
   ```
2. Advisories & license/source allow-list (install `cargo-audit` and `cargo-deny` with `--locked` once):
   ```bash
   cargo install cargo-audit --locked || cargo audit --version
   cargo audit
   cargo install cargo-deny --locked || cargo deny --version
   cargo deny check
   ```
   Its allow-list is `src-tauri/deny.toml`. Unclassified licenses fail, and the
   five MPL-2.0 crates already in the graph are enumerated one by one so any new
   MPL dependency stops the gate and requires an ADR.
3. Clippy (`desktop` and `anima-worldgen` CPU features):
   ```bash
   cargo clippy --all-targets --features desktop -- -D warnings
   cargo clippy -p anima-worldgen --all-targets --features reader,generate,bake -- -D warnings
   ```
4. Feature-separation gate (`G2` — default build excludes Neo4j, WebSocket stack, Burn/WGPU, and `anima-worldgen`; `anima-worldgen` excludes `bevy_ecs`, `tauri`, and `wgpu`):
   ```bash
   cargo clippy --all-targets --no-default-features -- -D warnings
   leaked=$(cargo tree --no-default-features -e normal --prefix none | grep -cE '^(neo4rs|tokio-tungstenite|burn-wgpu|naga|zstd-sys|wgpu(-[a-z0-9-]+)?|ktx2|meshopt|bc7enc[a-z0-9_]*|cubecl-wgpu) v' || true)
   test "$leaked" -eq 0
   engine_leaked=$(cargo tree -p anima-engine -e normal --prefix none | grep -cE '^(neo4rs|tokio-tungstenite|burn-wgpu|naga|zstd-sys|wgpu(-[a-z0-9-]+)?|ktx2|meshopt|bc7enc[a-z0-9_]*|cubecl-wgpu|anima-worldgen) v' || true)
   test "$engine_leaked" -eq 0
   worldgen_leaked=$(cargo tree -p anima-worldgen --features reader,generate,bake -e normal --prefix none | grep -cE '^(bevy_ecs|tauri|tauri-[a-z0-9_-]+|wgpu(-[a-z0-9-]+)?) v' || true)
   test "$worldgen_leaked" -eq 0
   ```
5. TypeScript IPC binding parity (`G1.4` — fails if Rust IPC structs changed without regenerating `src/types/generated/`):
   ```bash
   cargo test --lib export_bindings && git diff --exit-code -- ../src/types/generated
   ```
6. Desktop test suite + empty-target policy (`ANIMA_USE_GPU=0` forces the CPU backend on GPU-less runners):
   ```bash
   set -o pipefail
   ANIMA_USE_GPU=0 cargo test --features desktop -- --nocapture 2>&1 | tee ../cargo-test-output.txt
   node ../scripts/check_test_targets.mjs ../cargo-test-output.txt --profile desktop
   ```
   `--features desktop` is not optional: several test files are gated behind
   `networking` or `ml-wgpu` (`#![cfg(feature = "...")]` and `[[test]] required-features`),
   so a bare `cargo test` silently skips migration, cross-shard, and GPU-fallback tests.
   CI passes `--profile desktop` to `check_test_targets.mjs` so `FEATURE_GATED_TARGETS`
   are enforced alongside the zero-empty-target check. On Windows, redirect at the OS shell level
   (`cmd /c "cargo test --features desktop --no-fail-fast > ..\cargo-test-output.txt 2>&1"`)
   — do not pipe through PowerShell or run inside Git Bash.
7. `anima-worldgen` CPU test suite + empty-target policy:
   ```bash
   set -o pipefail
   cargo test -p anima-worldgen --features "reader,generate,bake" -- --nocapture 2>&1 | tee ../cargo-test-worldgen-output.txt
   node ../scripts/check_test_targets.mjs ../cargo-test-worldgen-output.txt --profile worldgen-cpu
   ```
8. Module dependency graph vs `dataflow.json` (requires `cargo-modules` and `python3`):
   ```bash
   cargo modules --version || cargo install cargo-modules --locked
   cargo modules dependencies --lib --no-externs --package anima-engine > ../dataflow-truth.dot
   python3 ../scripts/verify_dataflow.py ../dataflow.json ../dataflow-truth.dot 1
   ```

### Compliance, Licensing & Flora Geometry (repo root `.`, after `npm ci --no-audit --no-fund`)

```bash
npm run check:licenses && npm run check:sbom && npm run check:sbom-schema && npm run check:notice && npm run check:flora-footprint
```

> **Windows host-target warning (`R12`):** `check:licenses`, `check:sbom`, and
> `check:notice` evaluate `cargo tree --features desktop` for the **host target**.
> The committed `NOTICE`, `licensing/third-party-index.json`, and `sbom.cdx.json`
> describe a **Windows** build (`windows-latest` in CI). Running these three checks
> on macOS or Linux will report host-target differences, and **contributors and
> agents MUST NEVER regenerate `NOTICE`, `licensing/third-party-index.json`, or
> `sbom.cdx.json` on macOS or Linux** (run `npm run gen:compliance` on Windows
> only). By contrast, `npm run check:sbom-schema`, `npm run check:flora-footprint`,
> and `licensing/bundle-closure.json` (updated by `npm run build`) are OS-independent.

### Supplemental Workflows & Local Gates

- **Determinism & hardening (`.github/workflows/hardening.yml`):** runs
  `cargo test --release`, 5× `RAYON_NUM_THREADS=1 cargo test --release --quiet`,
  `cargo test --release --test determinism_tests`, and a source grep across
  `src-tauri/src` and `src-tauri/crates` (`*.rs`, excluding comment-only lines)
  banning `(thread_rng|ThreadRng)` and `rand::rng`. Hardening agents restricted to
  `src-tauri/tests/` end every task with `just verify` per [`AGENTS.md`](AGENTS.md).
- **Cross-OS determinism matrix (`.github/workflows/determinism-matrix.yml`):**
  on-demand / path-filtered 3-OS run executing
  `ANIMA_M1_FULL=1 cargo test --release --test cross_platform_checksum_probe -- --nocapture`
  and `node scripts/paper/compare_checksums.mjs --self-test`.
- **Additional local verification scripts:** `npm run check:map-vision-manifest`.

## Working with the agent swarm

Agent orchestration rules, active card queues, and owner decisions live in
[`docs/how-to/AGENT_WORKFLOW.md`](docs/how-to/AGENT_WORKFLOW.md) and the prompt
playbook [`docs/ai/planning/2026-11-05-swarm-prompts.md`](docs/ai/planning/2026-11-05-swarm-prompts.md).
Do not copy the live queue or `DECISIONS` block into other docs; link there instead.

- **Swarm roles (`PROMPT 0` / `BLOCK S` / `VERIFIER`):**
  - **Lead:** verifies `DEPENDS ON`, `DECISIONS`, and `R5-AUTH`; serializes shared
    paths and `[RUST]` capacity (max 2 concurrent Rust builds); dispatches verbatim
    cards; never writes code or merges PRs.
  - **Worker:** works in an isolated worktree and branch, edits only `ALLOWED PATHS`
    (respecting `MAX FILES`), executes `DONE WHEN`, files a structured report
    (`WHAT CHANGED`, `WHY`, `VERIFY` raw output, `NOT DONE`, `SURPRISES`), and
    opens a Draft PR.
  - **Verifier:** starts in a fresh clean-context session on a detached worktree at
    `origin/<BRANCH>`, checks scope and `S7` anti-green-optimization scans, re-runs
    `DONE WHEN` from a clean build, verifies every cited symbol and diff hunk, runs
    `VERIFY EXTRA`, and issues `VERIFIED` or `MISMATCH`.
  - **Owner:** provides `DECISIONS` and `R5-AUTH`, reviews the diff, and merges
    only after `VERIFIED` and green CI.
- **Feature lifecycle (7 steps):** Follow
  [`docs/ai/planning/2026-11-05-swarm-master-plan.md` §7](docs/ai/planning/2026-11-05-swarm-master-plan.md#7-quy-trình-phát-triển-một-tính-năng-mới-sau-này):
  (1) Requirements (`docs/ai/requirements/`) → (2) Design (`docs/ai/design/`) →
  (3) ADR if touching tick order, determinism, source of truth, save schema, or ≥ 2
  subsystems (`docs/decisions/ADR-0000-template.md`) → (4) Plan + task cards →
  (5) Swarm execution (worker + verifier PRs) → (6) Docs updated in the same PR →
  (7) Measurement & status update (`STATE_OF_THE_PROJECT.md` §1 + `TODO.md`).
- **How to write a task card:** Use the mandatory template in
  [`docs/ai/planning/2026-11-05-swarm-prompts.md` §6](docs/ai/planning/2026-11-05-swarm-prompts.md#6-mẫu-thẻ-cho-việc-mới).
  Every card must populate: `CARD <ID> — <title>`, `WAVE` / `[RUST]` / `MAX FILES`,
  `DEPENDS ON` (with proving command), `BASE` (branch + SHA), `BRANCH`, `PR TARGET`,
  `DECISION`, `AUTHORIZATION`, `ALLOWED PATHS`, `GOAL`, `CONTEXT` (`file:line` + date),
  `STEPS`, `DONE WHEN`, and `VERIFY EXTRA`.
- **Human-reviewed contract files (R5) & read-only boundaries:**
  The following R5 contract files are human-reviewed and **read-only by default**:
  - [`CLAUDE.md`](CLAUDE.md)
  - [`AGENTS.md`](AGENTS.md)
  - [`PROJECT.md`](PROJECT.md)
  - [`README.md`](README.md) and [`README.vi.md`](README.vi.md) *(note: `BLOCK 0` also lists `README.en.md`, though on trunk `README.md` is English and `README.vi.md` is Vietnamese)*
  - [`docs/planning/STATE_OF_THE_PROJECT.md`](docs/planning/STATE_OF_THE_PROJECT.md)
  - `docs/decisions/ADR-*.md`
  - `docs/reference/*`
  In addition, `src-tauri/src/**` is **read-only by default** under [`AGENTS.md`](AGENTS.md).
  Modifying any R5 file or `src-tauri/src/**` requires an explicit
  `AUTHORIZATION (owner, <YYYY-MM-DD>)` line in `R5-AUTH` naming the exact allowed
  paths ([`docs/how-to/AGENT_WORKFLOW.md` §4](docs/how-to/AGENT_WORKFLOW.md#4-khối-decisions-để-dán-vào-prompt-0)).
  Without that line, **never** edit those files directly — write any proposed edits
  (including proposed updates to `AGENTS.md`) into your task report or PR description
  for human review instead.

## House rules

1. A change to simulation law updates [`SIMULATION_RULES.md`](SIMULATION_RULES.md)
   and its tests.
2. A change to an exchange format is versioned, with a migration and a
   Rust/TypeScript fixture.
3. The tick hot path (physics, CPG, collision) **allocates nothing on the heap** —
   tests assert `allocs == 0`.
4. EU is a closed system. A new energy charge goes through `total_cost`, never a
   separate deduction.
5. An architectural or large dependency decision gets an ADR.
6. A new open-source dependency passes the license check, a benchmark and a
   documented rollback path.

No Prettier and no rustfmt argument: match the surrounding style. Rust files are
formatted by rustfmt.

Two rules about evidence, both learned the expensive way:

- **A number has one home.** Measured results go in
  [`STATE_OF_THE_PROJECT.md` §1](docs/planning/STATE_OF_THE_PROJECT.md), with the command and the
  date. Other documents point there instead of copying a figure that will go stale.
- **Do not make a gate green by weakening it.** No loosened assertion, no new `#[ignore]`, no
  narrowed test command, no invented seed, value or image used as evidence. If a gate is wrong, say
  so in the pull request and fix the gate in the open.

## Reporting a security issue

Do not open a public issue. Use GitHub's private reporting — the **Security** tab
of the repository, "Report a vulnerability" — with the details and a
reproduction, and allow time for a fix before disclosure.
