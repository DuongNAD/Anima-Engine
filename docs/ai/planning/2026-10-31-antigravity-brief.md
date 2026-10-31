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

GROUP 2 — LOCKFILE-ONLY. Five advisories. These need a reasoned exemption, NOT a bump.
  zip 0.6.6                       RUSTSEC-2026-0367, -0368, -0369
  zstd-safe 5.0.2+zstd.1.5.2      RUSTSEC-2026-0403
  zstd-sys 2.0.16+zstd.1.5.7      RUSTSEC-2026-0410

  Each is in Cargo.lock and in no compiled graph, reached only as
    burn-core 0.13.2 -> (burn-candle -> candle-core | burn-tch -> tch -> torch-sys)
                     -> zip 0.6.6 -> zstd -> zstd-safe -> zstd-sys
  and Cargo.toml enables neither backend: burn = { features = ["std", "autodiff"] }.
  src-tauri/.cargo/audit.toml explains why an advisory can be reported against a
  crate that is never built: a lockfile is feature-agnostic and records every
  optional dependency of every resolved package.

  USE THE FORWARD CHECK, NOT `cargo tree -i`:
    cargo tree -e all --features desktop --target all | grep -c " <crate> v"   -> 0
  Corrected 2026-10-31, after this very block got it wrong. An earlier version of
  this task listed webpki-roots 1.0.9 (RUSTSEC-2026-0381) here as a sixth
  lockfile-only advisory, on the evidence that
  `cargo tree -i webpki-roots --features desktop -e all` printed nothing. It
  printed nothing because it FAILED:
    error: specification `webpki-roots` is ambiguous
    help: re-run this command with one of the following specifications
      webpki-roots@0.26.11
      webpki-roots@1.0.11
  Two versions in the lockfile make `-i` ambiguous, it exits non-zero, writes to
  stderr, and writes nothing to stdout — which reads exactly like "not in the
  graph". It is in the graph:
    webpki-roots v1.0.x <- webpki-roots v0.26.11 <- {neo4rs, ureq} <- anima-engine
  (0.26.11 is a semver-bridge wrapper over 1.0.x). So it SHIPS, and it was bumped
  to 1.0.11 rather than exempted. Writing that exemption would have suppressed a
  shipped vulnerability and broken R3.
  The forward check above has no `-i`, so no ambiguity, and it distinguishes the
  two cases plainly: 0 for a crate that is never compiled, 2 for webpki-roots.
  ALWAYS CHECK THE EXIT CODE. A command that fails and prints nothing is not a
  negative result.

  Note how large those bumps would be if you tried: zip 0.6.6 -> >=9.0.0 and
  zstd-safe 5.0.2 -> >=8.2.0 are many majors. Do not attempt them for code that is
  never built.

GROUP 1 HAS A THIRD CRATE because of the correction above:
  webpki-roots 1.0.9   RUSTSEC-2026-0381  -> needs >=1.0.10
  `cargo update -p webpki-roots@1.0.9` takes it to 1.0.11. Note the version in the
  package spec: without it, cargo refuses the command as ambiguous.

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
Report the five ignores you added, one line each, with the command you ran for each,
and report the exit code of every `cargo tree` you relied on.
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

AUTHORIZATION
This task writes to src-tauri/src, which AGENTS.md declares read-only. The owner
has authorized it explicitly for this task (QĐ-B). R5 still applies to CLAUDE.md,
AGENTS.md, PROJECT.md, STATE_OF_THE_PROJECT.md, docs/reference/ and the ADRs.

STEP 0 — main DOES NOT COMPILE on macOS or Linux. Fix that locally first.
Measured 2026-10-31:
    error: proc macro panicked  --> src/lib.rs:194:16  .build(tauri::generate_context!())
    message: failed to read icon .../src-tauri/icons/32x32.png: Invalid PNG signature.
Four of five icons on origin/main are 12-byte TEXT files containing the word
"placeholder" (32x32.png, 128x128.png, 128x128@2x.png, icon.icns). Not Git LFS —
.gitattributes has zero filter=lfs lines. CI never sees it because the Rust job runs
only on windows-latest. Full account: FINDINGS.md F-028.
This is NOT a regression from the 2026-10-22 push: merge-base 043930c already had
the placeholders and `git log 043930c..origin/main -- src-tauri/icons` is empty.
So: copy valid icons from branch feat/ecological-selection into your main worktree
(src-tauri/icons/, they are real PNGs there) as an UNCOMMITTED local build fix. Do
NOT commit them as part of this task — whether the real icons belong in the
repository is an open question for the owner, and mixing it into a determinism fix
would violate R8. If you are on Windows you can skip this step; say which you did.

THE TWO FAILURES — CI run 39644804767 on main @ a43872c
  a_fork_from_a_known_tick_shares_its_prefix_and_diverges_only_afterwards  FAILED
    "a control branch forked at tick 60 diverged from an uninterrupted run"
    left: 1740163834   right: 1989393324
  run_k_then_save_load_then_run_the_rest_matches_an_uninterrupted_run      FAILED
    "some piece of trajectory-relevant state is missing from the snapshot"
These two assertions are what every scientific claim in this project rests on: that
a fork shares its prefix, and that save/load is trajectory-exact. They are false on
main right now. Both PASS on branch feat/ecological-selection, same 17-test suite.

THE BISECT IS ALREADY NARROWED TO TWO COMMITS — measured 2026-10-31
Last green main is 1b3e7ef ("merge: get main green again (#33)", 2026-10-16). Of the
20 commits between it and a43872c, exactly TWO touch live_experiment.rs,
agent_systems.rs, snapshot.rs, simulation_state.rs or simulation_schedule.rs. In
chronological order:

  1d301c3  feat(core): add live agent demographics, trophic ecology rules, ...
             agent_systems.rs         +367
             simulation_schedule.rs   +56/-     <- the ONE place the tick schedule
                                                   is built; load-bearing for
                                                   determinism per CLAUDE.md
             environmental_systems.rs +52       <- the census
             simulation_loop.rs       +65
             ecology.rs, components.rs, ecs.rs, sim_rules.rs, world_systems.rs

  add568e  feat(science): add parameter sweeps, causal branch trees, telemetry
             live_experiment.rs       +165      <- the file under test
             experiment.rs +183, scenario.rs +61, and new branch_tree.rs,
             sweeps.rs, telemetry_export.rs

Test at 1d301c3 FIRST. If the two tests are red there, it is the demographics
commit. If green, it is add568e. That is one or two builds, not a five-step bisect.
Report which commit turns them red before you change anything.

TWO HYPOTHESES, in order — treat both as hypotheses (R9), not answers:
  H1 (for 1d301c3): src-tauri/src/core/agent_systems.rs:400 builds a Vec, :408
     pushes lineage_id.0.clone(), :411 calls sort_unstable_by on that lineage_id.
     lineage_id is NOT unique between siblings, and sort_unstable_by does not
     preserve the order of equal elements, so despawn order falls out of query
     iteration order — which does not survive a save/load round trip.
  H2 (for either): the schedule changed ORDER rather than only adding systems, or
     an observables() read was inserted into the resume path and is perturbing the
     world it reads. The roadmap raised the second one; nobody has checked it.
Say which hypothesis the bisect supports, and say so even if it is neither.

RULES
1. Reproduce and localise BEFORE fixing. Do not fix what you cannot name.
2. If the cause is the sort, the fix is a TOTALLY ORDERED key, not a stable sort
   bolted on top. Explain which key you chose and why it is unique per entity.
   Read src-tauri/src/core/mortality.rs on branch feat/ecological-selection first:
   it solves the same problem with no Vec, no clone and no sort, by queueing the
   existing ReclaimAndDespawnAgentCommand and letting the command queue order it.
3. R2: do not touch either test. They are the specification.
4. R8: this is a simulation-semantics change. Report the deterministic checksum
   before and after, and state the effect on already-published experiment evidence
   (artifacts/experiments/e2-evolved-brain-default/RESULT.md).
5. Do not fix the other red CI gates here (fmt, clippy, audit, feature separation).
   Those are B3 and B4. One task, one concern.

DONE WHEN
  cd src-tauri && cargo test --features desktop -p anima-engine --test live_experiment_tests
    -> "test result: ok. 17 passed; 0 failed"
  and paste: the commit the bisect names, the checksum before and after, which
  hypothesis held, and whether you were on Windows or supplied local icons.
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
licensing/third-party-index.json.

EXPECT 419, and corrected 2026-10-31: an earlier version of this block said not
to. That was wrong. Counting the crate set of
`cargo tree --features desktop -e normal --target x86_64-pc-windows-msvc` gives 423,
which decomposes exactly as 2 workspace crates + 2 host-resolved proc-macro leaks
(swift-rs@1.0.7, base64@0.21.7 — see FINDINGS.md F-025) + **419** third-party. The
difference from the committed index is only three TLS version swaps, so task A6
added no crate at all. If your regeneration on Windows produces anything other than
419, that is a finding worth reporting, not a number to accept quietly.

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

If a test that is NOT one of these six fails on Windows while it passes on macOS,
that is a finding, not something to fix in passing: say which test and stop. Line
endings and path separators are the usual causes and both have bitten this project
before — scripts/check_test_targets.mjs exists partly because PowerShell wrapped
cargo's output and made a gate under-report.
```

### A7′ — ❌ **ĐÃ THỬ VÀ BỊ TỪ CHỐI 2026-10-31. Đừng gửi lại.** → dùng **A7**

> Thí nghiệm đã **thành công ở chỗ quan trọng**: nó trả lời dứt khoát câu hỏi "sinh artifact licence
> trên Mac được không". Đáp án là **không**, và lý do là một hành vi cargo, không phải lỗi dự án.
>
> Ghim `cargo tree --target x86_64-pc-windows-msvc` và `cargo metadata --filter-platform` làm
> `gen:compliance` **exit 1** ở guard có sẵn `licensing.mjs:243`. Nguyên nhân: `cargo tree` resolve
> crate **proc-macro và cây con của chúng theo HOST**, vì proc-macro chạy trong tiến trình compiler.
> `tauri-utils` khai `swift-rs` dưới `cfg(target_os = "macos")`; qua proc-macro `tauri-macros`,
> cargo đánh giá cfg đó theo macOS và kéo `swift-rs@1.0.7` + `base64@0.21.7` vào — mà
> `--filter-platform` thì loại. Hai lệnh bất đồng **theo thiết kế** khi cross-resolve.
>
> `-e no-proc-macro` sẽ làm chúng khớp lại, nhưng nó loại `tauri-macros`, `syn`, `quote`,
> `proc-macro2`, `serde_derive`, `thiserror-impl` — những crate **đang có trong** inventory. Đó là
> đổi phạm vi tài liệu tuân thủ, tức quyết định pháp lý, không phải sửa lỗi.
>
> **Hai thứ thí nghiệm này để lại, giá trị hơn việc nó định làm:**
> 1. **419 nay được chứng minh**, không còn là "con số đã commit": 423 crate của
>    `cargo tree --target windows` = 2 workspace + 2 rò host + **419** third-party.
> 2. **A6 không thêm crate nào** — hiệu với index chỉ là ba cặp đổi phiên bản TLS. Nên lời dặn
>    *"DO NOT EXPECT 419"* trong block A7 là **sai**; 419 mới là đúng, và A7 đã được sửa.
>
> Đầy đủ ở [FINDINGS.md F-025](../../../FINDINGS.md). Bản thử còn trong `git stash@{0}`.

<details>
<summary>Block gốc, giữ làm bản ghi lịch sử — <b>không dùng</b></summary>

### A7′ — Make the licensing artifacts reproducible on any host (**thay cho A7**)

> Thay thế A7. A7 nói "chạy lại trên Windows"; task này sửa nguyên nhân để **mọi máy** chạy được,
> nên artifact licence không còn chỉ kiểm được ở runner Windows. Hai lệnh cargo đã được xác minh
> chạy trên macOS trước khi viết block này.

```text
GOAL
Make the licensing generator resolve the WINDOWS dependency graph explicitly, so
`npm run gen:compliance` produces the committed artifacts on any host, not only on
Windows. Then regenerate and report hashes so the owner can confirm byte-identity
against a real Windows run.

AUTHORIZATION — READ THIS
This task edits scripts/, which AGENTS.md lists under "Cấm tuyệt đối". The owner
has directed this work explicitly. That authorization covers scripts/lib/licensing.mjs
and the generated artifacts named under FILES, and nothing else. R5 still applies to
CLAUDE.md, AGENTS.md, PROJECT.md, STATE_OF_THE_PROJECT.md, docs/reference/ and the ADRs.

WHY THIS EXISTS
scripts/lib/licensing.mjs:27 states the problem in its own words — "`cargo tree`
resolves for the host target. This repository's desktop target is Windows and CI
runs the Rust job on windows-latest, so the committed artifacts describe a Windows
build." It is recorded as a property of the product. The consequence was not: a
developer on macOS or Linux CANNOT run `npm run check:notice` and get a meaningful
answer — they get the macOS inventory and a red gate. So CI is the only place that
gate means anything, which is the "a gate that only fails where nobody is looking"
pattern this repository has already paid for several times.

MEASURED ON macOS 2026-10-31, BOTH WORK — you are not exploring, you are implementing:
  cargo tree --features desktop -e normal --target x86_64-pc-windows-msvc
      -> exit 0, 19 occurrences of windows-sys/webview2 in the output
  cargo metadata --format-version 1 --features desktop \
      --filter-platform x86_64-pc-windows-msvc
      -> exit 0, 577 packages, windows-sys present, no objc2 and no gtk
Neither needs `rustup target add`: resolution evaluates cfg, it does not need the
target's std. If either DOES demand an installed target on your machine, stop and
report that instead of installing one.

WHAT TO CHANGE — two call sites, both in scripts/lib/licensing.mjs
  :195  cargo tree  ['tree', ...CARGO_FEATURES, '-e','normal','--prefix','none','--no-dedupe']
        add the target
  :207  cargo metadata  ['metadata','--format-version','1', ...CARGO_FEATURES]
        add --filter-platform with the same triple
Put the triple in ONE named constant next to CARGO_FEATURES, with a comment saying
why Windows (the desktop target, and the platform CI's Rust job checks). Do NOT add
an env-var override: these artifacts describe one platform by design, and a knob
invites two developers to commit two different inventories.

Then UPDATE the "Host-target caveat" comment at :24-32. It currently explains that
the artifacts describe Windows because that is where the generator happened to run.
After this change they describe Windows because the code says so. That comment is
load-bearing — the A7 task block and two planning documents cite it — so it has to
stop describing the old behaviour.

FILES
  scripts/lib/licensing.mjs          (the change)
  NOTICE, licensing/third-party-index.json, licensing/THIRD_PARTY_LICENSES.txt,
  sbom.cdx.json                      (regenerated — NEVER hand-edit any of them)

DO NOT EXPECT 419
The committed index says counts.cargo = 419. A rough count on macOS with the
Windows target gives 423, and the host count is 400. The 423-vs-419 gap is NOT
explained yet and you are not expected to make it 419:
  - task A6 (commit f73086c) moved rustls to 0.23.46, rustls-webpki to 0.103.16 and
    webpki-roots to 1.0.11, and a version bump can add transitive dependencies;
  - or `--target` resolution may differ from native Windows resolution.
Those two causes are indistinguishable from one host, which is why the hashes below
matter. Report the number the generator produces and do not reverse-engineer it.

RULES
Use the project's own command: npm run gen:compliance (= gen:licenses && gen:sbom
&& gen:notice), in that order. If a check still fails afterwards, report on the
generator or the index — never edit a generated file to make a check pass.
`npm run check:licenses-complete` is EXPECTED to fail on hexf-parse 0.2.1: upstream
never published a licence text for that release and closing the row is a legal
decision, not an engineering one (STATE_OF_THE_PROJECT.md §1.c). Not yours to fix.

DONE WHEN
  npm run check:notice        -> exit 0
  npm run check:licenses      -> exit 0
  npm run check:sbom          -> exit 0
  npm run check:sbom-schema   -> exit 0
  npm run test:frontend       -> 0 failed   (was 6: noticeInventory 5 + thirdPartyLicenses 1)
  cd src-tauri && cargo test --features desktop --no-fail-fast
                              -> 1153 passed, 0 failed  (nothing here touches Rust,
                                 so a different number is a red flag, not a result)
AND REPORT, so a Windows run can be compared byte for byte:
  shasum -a 256 NOTICE licensing/third-party-index.json \
                licensing/THIRD_PARTY_LICENSES.txt sbom.cdx.json
  plus counts.cargo from the regenerated index, your OS, and `rustc --version`.
The owner will run the identical command on Windows. If the four hashes match, the
--target path is proven faithful and these artifacts become verifiable anywhere. If
they do NOT match, that is a more valuable finding than this task: it means a second
platform axis exists that --target does not capture, and it must be named before any
licensing artifact is trusted. Say so plainly rather than adjusting anything.
```

</details>

### B1b — F-029: declare the SimRng draw order, and gate it (stacks on PR #38)

```text
GOAL
Make the live world's RNG draw order independent of the process, and add the gate
that would have caught it. FINDINGS.md F-029.

AUTHORIZATION
Writes to src-tauri/src and src-tauri/tests. The owner authorized src-tauri/src
(QĐ-B). R5 still applies to CLAUDE.md, AGENTS.md, PROJECT.md,
STATE_OF_THE_PROJECT.md, docs/reference/ and the ADRs.

BRANCH FROM PR #38, NOT FROM main
Base your branch on `fix/live-world-restores-its-spatial-grid` (PR #38). Reason: on
main today the two reproducibility gates fail in roughly half of processes BECAUSE
the order varies. Fix the order first and they would fail — or pass — consistently,
depending which order you pick, and you would not be able to tell your ordering
change from #38's grid fix. #38 makes them pass under BOTH orders, so on top of it
any remaining failure is yours.
main also does not compile on macOS/Linux: four of five src-tauri/icons/ files are
12-byte "placeholder" text files (F-028). Supply valid icons locally, UNCOMMITTED,
as in B1 step 0. On Windows you can skip that.

THE PROBLEM, measured 2026-10-31
Five systems take ResMut<SimRng> on main:
    agent_systems.rs:441          autonomous_reproduction_system   (added by 1d301c3)
    agent_systems.rs:677          check_epoch_completion_system
    environmental_systems.rs:217  seed_dropping_system
    world_systems.rs:190          spawn_food_system
    world_systems.rs:761          manual_migration_system
Taking ResMut on one resource makes Bevy SERIALISE them — it does not decide which
runs first. SimRng's own doc comment (core/resources.rs:347) states the invariant
conditionally, and that condition is what broke:
    "same seed plus SAME TICK ORDER yields the same run"
    "Systems take ResMut<SimRng>, which makes Bevy serialise them against one another."
Bevy orders conflicting unordered systems by a per-process hash seed, so the same
seed and manifest produce different worlds in different processes. Measured effect:
the tick-1 offspring lands at x = 75.77 or x = 71.47 depending on the process.

WHAT IS ORDERED TODAY, AND WHAT IS NOT — read this before you add edges
  simulation_schedule.rs:134  spawn_food_system.after(apply_environmental_effects_system)
  simulation_schedule.rs:151  seed_dropping_system.after(apply_environmental_effects_system)
  simulation_schedule.rs:142  check_epoch_completion_system.after(metabolic_decay_system)
  simulation_schedule.rs:146  manual_migration_system.after(integrate_physics_system)
                                                     .before(check_migration_boundaries_system)
  simulation_schedule.rs:203  autonomous_reproduction_system — inside a .chain() with
                              agent_mortality_system, apply_deferred and
                              ecosystem_census_system, ordered .after() six systems,
                              NONE of which is spawn_food_system or seed_dropping_system
So at least three pairs are unordered, and one of them PREDATES 1d301c3:
    autonomous_reproduction_system <-> spawn_food_system
    autonomous_reproduction_system <-> seed_dropping_system
    spawn_food_system              <-> seed_dropping_system      <- older than 1d301c3
Do not take that list as complete. ENUMERATE it, with the tool: the test helper
`ambiguous_pairs()` in src-tauri/tests/schedule_ambiguity_tests.rs already walks
`schedule.graph().conflicting_systems()`, which is Bevy's own answer to this
question. Report every unordered pair among the five, and say which ones predate
1d301c3 — that changes whether this is a regression or a latent bug.

THE ORDER YOU CHOOSE IS A SEMANTIC DECISION, NOT A FORMALITY
Picking an order picks one of the two worlds. State the order you declared and WHY
in that order — e.g. does food spawn before offspring exist, or after? The
precedent to follow is in the same file, simulation_schedule.rs:144-145, where the
authors ordered the two migration producers and wrote the reason beside it: "An
explicit observer request wins a contested queue slot over automatic boundary
traffic. The order also makes the two producers deterministic with respect to each
other." Write a comment of that quality next to each edge you add.

THE GATE — mirror the energy one
src-tauri/tests/schedule_ambiguity_tests.rs has exactly two tests:
    report_every_ambiguous_pair_in_the_tick_schedule   prints, asserts nothing
    no_two_systems_that_move_energy_are_left_unordered asserts, energy only
That scope was deliberate; the module docs say "an ambiguity is only a bug if the
two orders give different answers. Most do not." True — and RNG-drawing systems are
the other class where the order ALWAYS matters. Add the mirror:
    no_two_systems_that_draw_simrng_are_left_unordered
Model it on the energy test, in the same file, using the same helper. It must name
the offending pair when it fails, not just count.

RULES
R8 applies and this one is unusual: unlike PR #38, the deterministic checksum WILL
change, because you are choosing one of two trajectories. That is expected. Report
the uninterrupted checksum before and after, and say so plainly.
Checked for you: NO test freezes a live checksum as a literal — they compare two
runs against each other — so nothing should need a baseline updated. If you find a
frozen number anywhere, stop and report it instead of editing it (R2).
Published experiment evidence: E2 is frozen at 9c57184, long before any of these
systems existed, and runs from_manifest with restore: None. State the effect
explicitly rather than assuming it.
Do not fix the other red CI gates (fmt, clippy, audit, feature separation) — B3/B4.

DONE WHEN
  1) the new gate exists and PASSES
  2) NEGATIVE CONTROL, required: remove one ordering edge you added, show the gate
     goes RED and NAMES that pair, then restore it. Paste both outputs.
  3) cd src-tauri && cargo test --features desktop --no-fail-fast  -> 0 failed
  4) cargo test --features desktop -p anima-engine --test live_experiment_tests
     run in TEN separate processes -> 17 passed, 0 failed, all ten
  5) print the tick-1 position of the first offspring in five separate processes
     and show it is now identical in all five (it was 75.77 or 71.47 before)
  6) report: every unordered pair you found, which predate 1d301c3, the order you
     declared and why, and the checksum before and after
```
