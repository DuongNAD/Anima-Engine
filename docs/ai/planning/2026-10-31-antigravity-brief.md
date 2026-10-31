---
title: Brief cho Antigravity (Gemini 4 Argon) — HT-A và HT-B
status: active
owner: maintainers
last_reviewed: 2026-10-31
review_cycle: per-release
---

# Brief cho Antigravity

> **Cách dùng:** dán **BLOCK 0** một lần ở đầu mỗi phiên, rồi dán **một** task block mỗi lượt, theo
> thứ tự. **Đừng dán cả file.** Sau mỗi task: đọc `git diff`, chạy lệnh ở `DONE WHEN`, và chỉ sang
> task sau khi nó exit 0.
>
> **Vì sao không phải một prompt "làm từ đầu đến cuối":** hai lượt agent gần nhất trên repo này đều
> *chạy xong và báo thành công* trong khi để lại bằng chứng bịa, test bị xoá, và — lượt gần nhất —
> 42 file của một dự án **không liên quan** (một palindrome checker Python, kèm `.pyc`) commit vào
> thư mục gốc. Phiên 2026-10-31 vừa xoá chúng. Rào chắn ở BLOCK 0 là *mỗi dòng một sự cố đã xảy
> ra*, không phải lo xa. Một task bị chặn, dừng lại và hỏi vẫn rẻ hơn một PR trông như xanh.
>
> Kế hoạch đầy đủ: [`EXECUTION_PLAN_2026-10-31.md`](../../planning/EXECUTION_PLAN_2026-10-31.md).
> Chẩn đoán + số đo: [`COMPLETION_ROADMAP_2026-10-31.md`](../../planning/COMPLETION_ROADMAP_2026-10-31.md).

---

## BLOCK 0 — Standing rules (paste once per session)

```text
You are working in the Anima-Engine repository: a Tauri v2 desktop app, Rust backend
(Bevy ECS + Burn) with a 60 FPS simulation thread, TypeScript/React/Vite frontend.
Cargo workspace is rooted at src-tauri/. The tests/ directory is a second npm package
with its own node_modules.

READ FIRST, BEFORE ANY EDIT:
  CLAUDE.md                                  (commands, gotchas, hard rules)
  docs/planning/STATE_OF_THE_PROJECT.md §1   (the ONLY authoritative measurement table)
  docs/planning/EXECUTION_PLAN_2026-10-31.md (the task you were given, in context)

HOW THIS PROJECT JUDGES WORK
This codebase's value is its evidence discipline, not its feature count. A gate that
exits 0 while hiding a real failure is the specific thing it was built to prevent. Every
rule below is one incident that already happened here. Breaking one does more damage than
leaving the task unfinished.

R1  NEVER commit conflict markers. Scan before every commit:
      grep -rn '^<<<<<<< \|^=======$\|^>>>>>>> ' --include='*.rs' --include='*.ts' \
        --include='*.tsx' --include='*.mjs' src/ src-tauri/src src-tauri/tests tests/ scripts/
    History: markers were committed to main and kept CI red for two and a half months,
    in the very script that gates test targets.

R2  NEVER delete a test, loosen an assertion, add #[ignore]/skip/should_panic, move a
    fixture, or widen a tolerance in order to get a green result. If a test blocks you,
    report it and stop. History: an exact count was changed to `>=` in five files, one
    assertion was rewritten to compare a constant with itself, four tests were deleted,
    and a raycast fixture was moved from x=99 to x=90 to hide a real sensor bug that made
    every agent blind on every tick.

R3  NEVER fabricate data, evidence, or fallback values. No hardcoded seeds presented as
    run provenance, no fixed ratios standing in for measurements, no synthetic curves or
    invented checksums on a dashboard, no 1x1 PNGs generated to make an evidence hash
    differ. If a value is not available, fail loudly or leave the feature out.
    History: all four of those shipped.

R4  NEVER narrow a test command. Do not edit the `test`, `test:frontend`, `test:e2e`,
    `lint`, or `typecheck` scripts in any package.json to run a smaller set.
    History: `test:e2e` was narrowed to one directory; the IPC-contract and
    console-hygiene specs silently stopped running and CI stayed green.

R5  NEVER write to these files: CLAUDE.md, AGENTS.md, PROJECT.md, README.md, README.en.md,
    docs/planning/STATE_OF_THE_PROJECT.md, docs/decisions/ADR-*.md, docs/reference/*.
    They are human-reviewed contracts. If your task seems to need a change there, stop and
    report. History: PROJECT.md — the file that holds the IPC contract — was overwritten
    twice by another agent's own project docs.

R6  Your scratch files NEVER go in the repository. No plan.md, research.md, design.md,
    audit.md, review.md, progress.md, TASK.md at the repo root; no projects/ directory; no
    __pycache__ or .pyc anywhere. Write notes outside the repo, or under docs/ai/ only when
    the task explicitly says so. History: 42 files of an unrelated Python palindrome
    exercise were committed to the root and to projects/, including .pyc binaries.

R7  One task = one branch = one PR. Stage by explicit path. NEVER `git add -A`, never
    `git add .`, never push to main. If `git status` shows changes you did not make, stop
    and report — they belong to another session.

R8  A commit title must cover everything in the commit. Any change to simulation
    semantics — energy, mass, mortality, reproduction, schedule order, RNG draws — is its
    own PR, with the deterministic checksum measured before and after, and a statement of
    the effect on already-published experiment evidence. History: a commit titled
    "fix(sensors)" also changed five simulation semantics and nobody re-measured.

R9  An automated audit finding is a HYPOTHESIS. Reproduce it with a failing test first,
    then fix it. Never apply a finding because it sounds right. History: audit finding
    SIM-01 was applied as written and made every agent's sensors blind, and the whole
    suite stayed green.

R10 Zero heap allocation in tick systems. Simulation tick paths (physics, CPG, collision,
    metabolism, mortality) must not allocate: no Vec::new, no String clone, no HashMap, no
    sort with an allocating key, inside a per-tick system. Use pre-allocated buffers.
    Tests assert allocs == 0.

R11 Determinism: no thread_rng(), rand::random(), SystemTime::now(), Instant::now(), or
    HashMap iteration order anywhere a simulation value can depend on it. Use SimRng and
    BTreeMap. A source-scanning test fails if thread_rng() reappears.

R12 NEVER regenerate the licensing artifacts (NOTICE, licensing/third-party-index.json,
    sbom.cdx.json) on macOS or Linux. `cargo tree` resolves for the host target; the
    committed artifacts describe a WINDOWS build and CI checks them on windows-latest.
    Regenerating elsewhere produces a different, equally correct inventory and turns a
    green CI gate red. See scripts/lib/licensing.mjs, the "Host-target caveat" comment.

TOOLING FACTS THAT WILL WASTE YOUR TIME IF YOU GUESS
  - esbuild is NOT installed. Vite 8 bundles with rolldown/oxc. Any script calling esbuild
    dies. Run TypeScript scripts with: node scripts/run_ts.mjs <file.ts> [args]
  - Backend tests need the feature flag: cargo test --features desktop
    A bare `cargo test` does not even schedule seven feature-gated test targets.
  - A vi.fn() mock invoked with `new` needs a `function`, not an arrow. Under @vitest/spy 4
    an arrow has no [[Construct]] slot and the spy throws "is not a constructor".
  - Do NOT run `npm run tauri:dev` or `cargo run`. The full Bevy/Tauri backend is heavy and
    has crashed this machine. Headless tests only.
  - src-tauri/src is declared READ-ONLY by AGENTS.md. If your task writes there, the task
    block will say so explicitly and will cite the owner's exemption. Without that line in
    your task, do not write to src-tauri/src.

REPORT FORMAT — end every task with exactly this:
  WHAT CHANGED   file:line for each change, one line each
  WHY            one sentence per change
  VERIFY         the DONE WHEN command, and its raw output, pasted unedited
  NOT DONE       anything in the task you did not finish, and why
  SURPRISES      anything you found that the task did not predict
If the VERIFY command does not exit 0, say so. Do not describe the task as complete.
```

---

## HT-A — Make branch `feat/ecological-selection` green

> Branch is already at `cargo test --features desktop` → **1153 passed, 0 failed**, and both
> live-adapter reproducibility gates green. These tasks are hygiene plus one scope decision.

### A1–A5 — ✅ ĐÃ XONG 2026-10-31, **đừng gửi lại**

| Task | Gate | Kết quả đo | Commit |
|---|---|---|---|
| **A1** | `check_test_targets.mjs` | `exit 0` — 107 target · 3 rỗng (allow-list) · 2 ignore (allow-list) · **8** feature-gated chạy | `bef7768` |
| **A2** | `cargo fmt --check` | `exit 0` — 67 khối trong 7 file test; token multiset **không đổi**, 48 test của 7 target xanh | `4cc5b8d` |
| **A3** | QĐ-D `playground/` | **là nháp** — xoá 4 file test kiểm mã không được track, loại `playground` khỏi ESLint | `26c8a96` |
| **A4** | `lint` + ratchet | `0 errors, 0 warnings (baseline 0)` | `26c8a96` |
| **A5** | `typecheck:tests` | `exit 0` — **18/19 lỗi là do `tests/node_modules` cũ** (vitest 1.6.1 vs lockfile 4.1.10), sửa bằng `npm ci --prefix tests`; 1 lỗi thật là fixture thiếu `length`/`radius` | `9dfbcd6` |

Ngoài ra `QĐ-E` đã quyết: xoá `organism_library.test.tsx` (3 test, cả 3 đỏ, `get_organism_presets` và
`spawn_preset_organism` không tồn tại bên Rust) — commit `296bcc3`.

**Trạng thái nền để task A6/A7 so với:** `cargo test --features desktop` **1153 passed · 0 failed** ·
`npm run test` (src) exit 0 · `npm run test:frontend` **6 đỏ** (toàn bộ là A7) · `npm run build` exit 0 ·
`check_docs_links` 0 gãy.

> ⚠️ **Nếu `npm run typecheck:tests` hay `npm run test:frontend` đỏ bất thường trên máy bạn, kiểm
> `tests/node_modules` trước khi sửa mã:**
> `node -e "console.log(require('./tests/node_modules/vitest/package.json').version)"` phải in
> **4.1.10**. Nếu không, chạy `npm ci --prefix tests`. Một lần install cũ làm 18 lỗi kiểu xuất hiện
> ở 6 file mà không có lỗi mã nào.

### A6 — Rust advisories

```text
GOAL
Make `cargo audit` exit 0 from src-tauri/.

CONTEXT — the dependency graph was measured on 2026-10-31, so you do not have to
guess which of these are real. Eleven advisories, and they split cleanly in two.

GROUP 1 — REACHABLE IN THE SHIPPED BUILD. Five advisories. Bump these.
  rustls 0.23.42       RUSTSEC-2026-0285  -> needs >=0.23.45
                       RUSTSEC-2026-0346  -> needs >=0.23.46
                       RUSTSEC-2026-0348  -> needs >=0.23.46
                       RUSTSEC-2026-0349  -> needs >=0.23.46
  rustls-webpki 0.103.13  RUSTSEC-2026-0382
                       -> needs >=0.103.16,<0.104.0-alpha.1  OR  >=0.104.0-alpha.8

  Both are genuinely in the compiled graph. Measured:
    cargo tree -i rustls --features desktop -e normal
      rustls v0.23.42
      |-- tokio-rustls v0.26.4 -> neo4rs v0.7.3 -> anima-engine
      `-- ureq v2.12.1 -> anima-engine
  So rustls arrives TWICE: through neo4rs (the `neo4j` feature) and through ureq,
  which is a direct dependency of this crate. rustls-webpki comes in under rustls.
  An exemption is NOT acceptable for either — they ship.

  origin/main already did the rustls half, in commit 0d224ec (0.23.42 -> 0.23.46).
  Port that rather than resolving it again, so the two trees do not disagree.
  Then check whether the rustls bump carries rustls-webpki to >=0.103.16 on its
  own; if it does not, bump it explicitly and say so.

GROUP 2 — LOCKFILE-ONLY. Six advisories. These need a reasoned exemption, NOT a bump.
  webpki-roots 1.0.9              RUSTSEC-2026-0381
  zip 0.6.6                       RUSTSEC-2026-0367, -0368, -0369
  zstd-safe 5.0.2+zstd.1.5.2      RUSTSEC-2026-0403
  zstd-sys 2.0.16+zstd.1.5.7      RUSTSEC-2026-0410

  Every one of these is in Cargo.lock and in NO dependency tree. Measured per crate:
    cargo tree -i <crate> --features desktop -e all    -> 0 lines of output
  `-e all` includes dev- and build-dependencies, so this is not "only a dev dep" —
  the selected feature set never compiles them at all. src-tauri/.cargo/audit.toml
  explains why that happens: a lockfile is feature-agnostic and records every
  optional dependency of every resolved package.

  Note how large those bumps would be if you tried: zip 0.6.6 -> >=9.0.0 and
  zstd-safe 5.0.2 -> >=8.2.0 are many majors. Do not attempt them for code that is
  never built.

RULES
Follow the format already in src-tauri/.cargo/audit.toml exactly. Its two
burn-dataset entries are the model, and the file states the standard in its own
words: "Do not add an ignore without recording the check that shows the code is not
reachable, and re-run that check when dependencies move."
So each new ignore must carry: the crate and version, the advisory title and CVSS
if the advisory gives one, why it is unreachable, and THE COMMAND that shows it,
with its expected output. An ignore without that evidence is worse than a red gate,
because it is a red gate someone made look green.

chacha20 0.10.1 is reported as `yanked`, not as a vulnerability. audit.toml sets
`informational_warnings = []`, so yanked findings do not fail the build — it is the
"1 allowed warning" in the output, not one of the eleven. Leave it alone unless you
can bump it without touching the generator, and read this first: rand_chacha is used
DIRECTLY so a draw position can be saved, and every trajectory in the project
depends on the generator. If you change anything in that area, rule R8 applies — it
is a simulation-semantics change, needs its own PR, and needs the deterministic
checksum measured before and after.

DONE WHEN
  cd src-tauri && cargo audit                        -> exit 0
  cd src-tauri && cargo tree -i rustls --features desktop -e normal
                                                     -> shows >=0.23.46
  cd src-tauri && cargo test --features desktop --no-fail-fast
                                                     -> 1153 passed, 0 failed
      (if the count differs from 1153, report the number and which targets changed)
  cd src-tauri && cargo tree --no-default-features -e normal \
      | grep -E 'neo4rs|tokio-tungstenite|burn-wgpu|naga'   -> no output
      (a dependency bump must not leak a gated subsystem into the default build)
  git diff Cargo.lock                                -> report which crates moved
Report the six ignores you added, one line each, with the command you ran for each.
```

---

## HT-B — Make `origin/main` green

> Cut a fresh branch from `origin/main`. **These tasks write to `src-tauri/src`, which
> `AGENTS.md` declares read-only — do not start until the owner has written the exemption
> (QĐ-B).** Paste that line into the task block when they do.

### B1 — The two reproducibility gates (do this before anything else on main)

```text
GOAL
Make tests/live_experiment_tests.rs pass 17/17 on main. Two tests fail today.

CONTEXT
CI run 39644804767 on main @ a43872c:
  a_fork_from_a_known_tick_shares_its_prefix_and_diverges_only_afterwards  FAILED
    "a control branch forked at tick 60 diverged from an uninterrupted run"
    left: 1740163834   right: 1989393324
  run_k_then_save_load_then_run_the_rest_matches_an_uninterrupted_run      FAILED
    "some piece of trajectory-relevant state is missing from the snapshot"
These two assertions are what every scientific claim in this project rests on: that a fork
shares its prefix, and that save/load is trajectory-exact. They are false on main right now.
Both tests PASS on branch feat/ecological-selection, with the same 17-test suite size.

LEADING HYPOTHESIS — treat it as a hypothesis (R9), not an answer:
  src-tauri/src/core/agent_systems.rs:400  let mut to_despawn: Vec<(String, Entity, ...)>
  src-tauri/src/core/agent_systems.rs:408  to_despawn.push((lineage_id.0.clone(), e, cause))
  src-tauri/src/core/agent_systems.rs:411  to_despawn.sort_unstable_by(|a,b| a.0.cmp(&b.0))
lineage_id is NOT unique between siblings, and sort_unstable_by does not preserve the order
of equal elements, so despawn order falls out of query iteration order — which does not
survive a save/load round trip.

RULES
1. FIRST reproduce and localise. Bisect the 2026-10-22 commit range on main by cluster
   (demographics, telemetry, sweeps, branch tree, interventions, Map 2, dashboard) and
   report WHICH commit turns the two tests red. Do not fix before you can name it.
2. Then fix the cause. If it is the sort, the fix is a totally-ordered key, not a stable
   sort bolted on top — explain which key you chose and why it is unique.
3. R2: do not touch either test. They are the specification.
4. R8: this is a simulation-semantics change. Report the deterministic checksum before and
   after, and say whether published experiment evidence is affected.

DONE WHEN
  cd src-tauri && cargo test --features desktop -p anima-engine --test live_experiment_tests
    -> "test result: ok. 17 passed; 0 failed"
  Paste the bisect result (the commit that broke it) and both checksums.
```

### B2 — The allocation the gate did not catch

```text
GOAL
Remove the per-tick heap allocation in the agent death path, AND extend the allocation gate
so it would have caught it.

CONTEXT
src-tauri/src/core/agent_systems.rs:400 allocates a Vec every tick; :408 clones a String per
dying agent. CLAUDE.md states tick systems must not allocate and that tests assert
allocs == 0 — but no existing gate covers agent_death_system, which is why nothing went red.
The branch feat/ecological-selection solves the same problem differently, in
src-tauri/src/core/mortality.rs: no Vec, no clone, no sort; it queues the existing
ReclaimAndDespawnAgentCommand through Commands and lets the command queue order it. Read
that file before designing your fix.

FILES
  src-tauri/src/core/agent_systems.rs
  the existing allocation test (find it: grep -rn "allocs" src-tauri/tests)

RULES
The second half of this task is not optional: a fix without a gate extension leaves the
same hole open. Write the gate so that reverting your fix makes it RED, and report that
negative control.

DONE WHEN
  the allocation gate covers agent_death_system and exits 0
  reverting the fix makes it fail (paste that output too)
  cd src-tauri && cargo test --features desktop --no-fail-fast   -> 0 failed
DEPENDS ON  B1
```

### B3 — Feature separation

```text
GOAL
Make the default build exclude Neo4j, the WebSocket stack and Burn/WGPU again.

CONTEXT
The CI step "Default build excludes Neo4j, the WebSocket stack and Burn/WGPU" is RED on main.
It checks the dependency GRAPH, not compilability, so the cause is a new unconditional `use`
or a dependency that lost its `optional = true` / cfg gate, added by the 2026-10-22 commits.

FILES
  src-tauri/Cargo.toml, and whichever module introduced the unconditional import

DONE WHEN
  cd src-tauri && cargo tree --no-default-features -e normal | \
    grep -E 'neo4rs|tokio-tungstenite|burn-wgpu|naga'     -> no output
  cd src-tauri && cargo tree --no-default-features -e normal | grep criterion  -> no output
  cd src-tauri && cargo test --no-default-features --no-fail-fast   -> 0 failed
  Report WHICH import caused it.
```

### B4 — The rest of main's red CI

```text
GOAL
Make both CI jobs green on main.

CONTEXT — CI run 39644804767, every failing step:
  Rust (windows-latest): cargo fmt --check | cargo audit (5 vulns) | cargo deny check |
    cargo clippy --all-targets -D warnings (12 lints) | cargo test (B1) |
    "No test target ran zero tests" | feature separation (B3)
  Frontend (ubuntu-latest): npm audit (root) | npm audit (tests/) | ESLint warning ratchet
The 12 clippy lints include: manual implementation of .is_multiple_of(), four
needless_range_loop ("the loop variable d is only used to index matrix"), this impl can be
derived, using clone on type SimClock which implements Copy.

RULES
Fix the code for clippy, never add #[allow] without a reason comment in the same place.
R12: do NOT regenerate NOTICE / SBOM / third-party-index — those gates are already GREEN,
leave them alone.
npm 10.9.8 crashes on `npm audit fix`; use npm 11 and then re-verify with npm 10.

DONE WHEN
  every step above exits 0 locally, and the PR's CI shows both jobs green
  Paste the CI run URL.
```

### B5 — Restore the full E2E suite

```text
GOAL
Un-narrow test:e2e, and add a gate that makes narrowing it impossible to do silently.

CONTEXT
tests/package.json line 8 currently reads:
  "test:e2e": "playwright test e2e/tiers/ --config e2e/playwright.config.ts"
It used to run the whole suite. ipc_contract, console_hygiene, the canonical-view specs and
the phase3/phase6 specs therefore do not run, and CI has been green without them. The
ipc_contract spec is the one that previously caught 5 wrong IPC parameter keys across 4
commands — save/load had never worked in the real app, and 6 assertions in 5 test files had
frozen the wrong keys so the suite was green BECAUSE of the bug.

FILES
  tests/package.json, tests/e2e/**, scripts/ (new gate)

RULES
Model the new gate on scripts/test_target_policy.mjs: a committed list of spec files,
compared against what the command actually schedules, failing in BOTH directions (a spec
missing from the run, and a stale entry in the list).
A green E2E row requires TWO things here: exit 0 AND a clean raw output. Grep the full
output for `console.error`, `pageerror` and `TypeError` and report the counts — a previous
run exited 0 while printing four TypeErrors twice each.

DONE WHEN
  npm run test:e2e --prefix tests   -> exit 0, and 0 console.error / pageerror / TypeError
  removing one spec from the list -> the gate goes RED and names that spec (paste it)
DEPENDS ON  B4
```

### B6 — Fabricated values, dead IPC calls, and the ledger bypass

```text
GOAL
Three separate PRs, not one. Each starts with a FAILING test.

(a) FABRICATED SCIENTIFIC VALUES — remove them or make them real
  src-tauri/src/core/telemetry_export.rs:264  herbivore_biomass = total_animals * 0.70
  src-tauri/src/core/telemetry_export.rs:265  carnivore_biomass = total_animals * 0.30
  src-tauri/src/core/telemetry_export.rs:271  biomass: 10000.0
  src-tauri/src/commands/simulation.rs        RunProvenance::new(42, status.tick_count)
A fixed 70/30 split, a 10000 floor, and a hardcoded seed 42 reported as a run's provenance.
Nothing writes EnergyFluxMatrix, so the Sankey export is ALWAYS the fallback.
Either compute these from the running world, or remove the export until they can be. A
"placeholder" that reaches a data-export path is indistinguishable from a measurement.

(b) TWO IPC COMMANDS THAT DO NOT EXIST
The frontend calls `dispatch_causal_intervention` and `get_causal_branch_tree`. There is no
#[tauri::command] for either (17 frontend invoke sites vs 34 backend commands; these two are
the gap). The Research Dashboard panels calling them throw at runtime:
  src/components/ResearchDashboard/CausalInterventionPanel.tsx:115
  src/components/ResearchDashboard/CheckpointStudioPanel.tsx:51
Implement both commands, or remove the panels. If you implement them, add them to the IPC
contract document and regenerate the ts-rs bindings (never hand-edit a generated binding —
WorkloadContext.ts was hand-edited once and the parity gate had never run to catch it).
src/components/ResearchDashboard/CheckpointStudioPanel.tsx:19-45 also contains synthetic sine
curves (Math.sin at :34 and :44 standing in for a divergence trace) and invented checksums, and
src/components/ResearchDashboard/DataExportDialog.tsx:51 catches the failed invoke and returns a
"Fallback mock response for standalone UI testing" — so an export that failed looks like one that
worked. R3 applies: remove all of it.

(c) THE LEDGER BYPASS AND THE SUSPECTED EU LEAK
  src-tauri/src/core/agent_systems.rs:522  pool.detritus += overhead_waste;
    writes straight into the pool, outside total_cost and outside EnergyLedger. The
    arithmetic may balance; the ledger cannot see it. CLAUDE.md's rule exists because such
    a path "leaks EU while looking entirely reasonable".
  PathogenTransmission in src-tauri/src/core/live_experiment.rs debits pool.animals via
    energy.transfer(Animals -> Detritus), but src-tauri/src/core/environmental_systems.rs:177
    re-snapshots `pool.animals = total` from agent reserves each census — and no agent was
    culled. The debit evaporates, the detritus credit stays. Every time the intervention
    fires, closed_total_eu should grow.
  This is READ FROM SOURCE, NOT MEASURED. Write the failing test FIRST (R9): fire
  PathogenTransmission in a live run with the ledger active and assert residual EU <= 1e-3.
  If it passes, say so — the hypothesis is then wrong and that is a valid, valuable result.
  No test currently fires these four interventions against the ledger; the ones that mention
  them cover branch-tree parity and replay only.

DONE WHEN (per PR)
  a failing test exists and is committed first, then passes
  cd src-tauri && cargo test --features desktop --no-fail-fast   -> 0 failed
  residual EU <= 1e-3 in every flag configuration (for (c))
DEPENDS ON  B1
```

---

## Sau HT-A và HT-B

Hợp nhất (HT-C) **cần quyết định của chủ dự án trước** — QĐ-A ở
[roadmap 31/10 §4](../../planning/COMPLETION_ROADMAP_2026-10-31.md). Đừng giao HT-C cho agent
trước khi có dòng quyết định đó, vì nó quyết định `core/mortality.rs` của nhánh **thay** hay
**nằm cạnh** `agent_death_system` của `main`, và hai lựa chọn cho ra hai cây mã khác nhau.

### A7 — NOTICE / artifact licence (⚠️ **cần máy Windows**)

```text
GOAL
Regenerate the licensing artifacts so noticeInventory and thirdPartyLicenses pass.
These are the last 6 of the 6 remaining frontend test failures.

READ THIS FIRST — THIS TASK CANNOT BE DONE ON macOS OR LINUX
scripts/lib/licensing.mjs says so in its own "Host-target caveat" comment:
`cargo tree` resolves for the HOST target, the committed artifacts describe a
WINDOWS build, and CI checks them in the Rust job on windows-latest. Regenerating
on macOS produced 398 Rust crates where the committed inventory has 419 — five
licence tests would go green locally and the CI gate would go red.
If you are not on Windows: STOP, report that, and do nothing else. Rule R12.

CONTEXT
The branch's NOTICE is a 62-line hand-written stub from before OSS-004; it predates
the generated transitive inventory and says so itself ("maintained by hand and is
not a generated, transitively complete inventory"). The failing tests expect the
generated form: an npm section with >=15 versioned components, the string
licensing/THIRD_PARTY_LICENSES.txt, and a Rust crate count matching
licensing/third-party-index.json (which currently says 419).

The generator's template was already fixed on this branch in commit a791248 —
before that, regenerating would have written "Anima-Engine itself is proprietary"
back into a distributed file. Do not re-fix it; check it says MIT OR Apache-2.0.

FILES
  NOTICE, licensing/third-party-index.json, licensing/THIRD_PARTY_LICENSES.txt,
  sbom.cdx.json   (all generated — never hand-edit any of them)

RULES
Use the project's own command, which regenerates all three in the right order:
    npm run gen:compliance      (= gen:licenses && gen:sbom && gen:notice)
Then verify with the check variants. Do not edit a generated file to make a check
pass; if a check still fails, the generator or the index is what to report on.
`npm run check:licenses-complete` is EXPECTED to fail: hexf-parse 0.2.1 has no
upstream licence text and closing that row is a legal decision, not an engineering
one (STATE_OF_THE_PROJECT.md §1.c). That one failure is not yours to fix.

DONE WHEN
  npm run check:notice        -> exit 0
  npm run check:licenses      -> exit 0
  npm run check:sbom          -> exit 0
  npm run check:sbom-schema   -> exit 0
  npm run test:frontend       -> 0 failed   (was 6)
  and report the Rust crate count the regeneration produced, plus the OS and
  rustc version you ran it on, so the next reader knows which host the committed
  artifacts describe.
```
