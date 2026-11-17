# Contributing to Anima-Engine

Thanks for your interest. This document covers the licensing terms your
contribution arrives under, and the gates a change has to pass.

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

Current code plus fresh tests win over prose. Files under `docs/archive/` are
superseded and must not be used as implementation plans.

## Where a change goes

The trunk is **`feat/ecological-selection`**. Open pull requests against it, not against `main`;
`main` is fast-forwarded onto the trunk at milestones. Both branches are protected: a direct push to
the trunk is rejected, and a pull request merges only when the `Rust (src-tauri)` and `Frontend` CI
jobs are both green. This holds for documentation-only changes too.

If you are a coding agent, or are dispatching one, read
[`docs/how-to/AGENT_WORKFLOW.md`](docs/how-to/AGENT_WORKFLOW.md) as well: it holds the card queue, the
owner's recorded decisions and the per-card authorization lines.

## Gates

Run these before opening a pull request. CI (`.github/workflows/ci.yml`) runs the
same set — Rust on `windows-latest`, frontend on `ubuntu-latest`.

Frontend, from the repository root:

```bash
npm run lint && npm run test && npm run test:frontend && npm run build
```

```bash
node scripts/eslint_ratchet.mjs
```

```bash
npm run check:docs
```

The ESLint ratchet holds the warning baseline at 0, so a warning is a failure. `check:docs` checks
links, document metadata and the IPC reference, and it reads untracked files, so a new document is
checked before you stage it.

Backend, from `src-tauri/`:

```bash
cargo fmt --check
```

```bash
cargo clippy --all-targets --features desktop -- -D warnings && cargo test --features desktop
```

`--features desktop` is not optional. Several test files are gated on the `networking` or `ml-wgpu`
feature, both by a crate-level `#![cfg(feature = "…")]` and by a `[[test]] required-features` entry,
so a bare `cargo test` does not schedule them at all. That silently skips the migration, cross-shard
and GPU-fallback coverage. `node scripts/check_test_targets.mjs <captured-output> --profile desktop`
is the policy gate: every zero-test target and every `#[ignore]`d test must be named with a reason in
`scripts/test_target_policy.mjs`, and anything unlisted fails. Run one `cargo test` at a time; a full
Rust build is heavy.

Advisories are gated too: `cargo audit` in `src-tauri/` and `npm run check:audit`, which audits both
npm packages. A high or critical npm advisory fails unless `scripts/npm_audit_policy.mjs` names it
with a reason, and a named one must stay out of `npm audit --omit=dev` (what ships).

Adding a Rust dependency also has to pass the license and source gate, from `src-tauri/`:

```bash
cargo deny check
```

Its allow-list is `src-tauri/deny.toml`. A license nobody has classified fails rather than passes,
and the five MPL-2.0 crates already in the graph are enumerated one by one — so a **new** MPL
dependency stops the gate and gets the ADR the open-source policy asks for.

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
