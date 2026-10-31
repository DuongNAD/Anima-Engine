---
title: Brief cho Antigravity — tách crate con (S0–S5)
status: active
owner: maintainers
last_reviewed: 2026-10-31
review_cycle: per-release
---

# Brief cho Antigravity — tách crate

> **Cách dùng:** dán **BLOCK 0** của
> [brief chính](2026-10-31-antigravity-brief.md) trước, rồi **BLOCK 0-S** dưới đây, rồi **một** task
> mỗi lượt. Sau mỗi task: đọc `git diff`, chạy bộ ba V1/V2/V3, chỉ sang task sau khi cả ba xanh.
>
> **S0.1 là task A1 của brief chính.** Làm nó trước; không có nó thì V2 không đo được và kế hoạch
> này mất một trong ba chân.
>
> Kế hoạch đầy đủ, kèm số đo và lý do từng bước:
> [`CRATE_SPLIT_PLAN_2026-10-31.md`](../../planning/CRATE_SPLIT_PLAN_2026-10-31.md).

---

## BLOCK 0-S — Standing rules cho việc tách crate (dán sau BLOCK 0)

```text
You are extracting Cargo workspace crates out of a 38,000-line `core/` module. This is a
PURE REFACTOR program: no behaviour may change at any step.

WHY THIS SHAPE
Workspace members compile into the same binary, so the project's global invariants
(closed-EU bit-exact, determinism, zero-alloc tick, save/load trajectory-exactness) are
still asserted in one test over one running world. Do not propose separate repositories,
separate processes, or semver boundaries — that would destroy exactly those invariants.

THE VERIFICATION TRIAD — all three, every step, no exceptions
V1  GRAPH    just dataflow-truth && just dataflow-verify 1     -> exit 0
             The real module dependency graph must match the declared contract in
             dataflow.json. A new edge or a cycle makes this red and names the edge.
V2  TESTS    baseline measured 2026-10-31 on this branch: 1153 passed, 0 failed, across
             107 targets, with 3 allow-listed empty and 2 allow-listed ignored.
             total test count must GO UP by exactly the number of tests moved into the
             new crate, and:
               cargo test --features desktop --no-fail-fast > out.txt 2>&1
               node scripts/check_test_targets.mjs out.txt --profile desktop   -> exit 0
             "The count stayed the same" is a FAILURE, not a pass. src-tauri/Cargo.toml
             records why: when anima-domain was created, its 20 tests silently dropped out
             of every gate until default-members was added — the crate still compiled as a
             dependency, so nothing looked wrong. Report the before and after numbers.
V3  TRAJECTORY  the deterministic checksum must be BIT-IDENTICAL before and after.
             Moving code must not change RNG draw order. One differing bit means the
             refactor was not pure. Measure it BEFORE you touch anything, and paste both.

RULES SPECIFIC TO THIS PROGRAM
S1  ONE CRATE PER PR. Never two extractions in one commit — the whole point of one step
    per PR is that V3 can be bisected to the step that broke it.
S2  Keep call sites unchanged. After moving a module into a crate, re-export it from its
    old path (`pub use anima_ecs_types::components;`) so no caller has to change in the
    same PR. Mechanical call-site rewrites are a separate, later PR.
S3  Move code, do not rewrite it. No renaming, no signature changes, no "while I'm here"
    cleanups, no reformatting beyond what rustfmt does. A diff that shows moved lines and
    nothing else is the goal; anything else makes V3 impossible to interpret.
S4  A new crate must not depend on anima-engine. Check it:
      cargo tree -p <new-crate> | grep anima-engine     -> no output
S5  Features must stay separated. After every step:
      cargo tree --no-default-features -e normal | grep -E 'neo4rs|tokio-tungstenite|burn-wgpu|naga'
    -> no output. A new crate that unconditionally pulls a gated dependency breaks a CI gate.
S6  Eight test targets on this branch carry a crate-level #![cfg(feature = ...)] plus a
    matching [[test]] required-features entry. Measured 2026-10-31, the gate prints:
      test targets: 107, empty: 3 (allow-listed 3), ignored: 2 (allow-listed 2),
      feature-gated running under "desktop": 8
    READ that number from the gate output; do not assume it. CLAUDE.md and
    STATE_OF_THE_PROJECT.md §1.a both still say seven — a number measured 2026-07-27,
    before commit c0344c9 added adversarial_challenger_tests.rs. If your step changes the
    count in either direction, name which target moved and why.
S7  If a step turns out to need a behaviour change to work, STOP and report. Do not make
    the behaviour change. It belongs in its own PR, before or after, with its own evidence.

DO NOT PROPOSE A REWRITE. The engine passes 1153 tests with 0 failures and both
reproducibility gates are green on this branch. The measured problem is integration
discipline, not architecture. If you think a module should be rewritten, say so in
SURPRISES and move on.
```

---

## S0.2 — Khai hợp đồng phụ thuộc và bật nó thành gate

```text
GOAL
Fix the producer, then create dataflow.json describing the module dependency graph
AS IT IS TODAY, make `just dataflow-verify 1` pass against it, and add that check
to CI.

AUTHORIZATION
This task edits justfile and .github/workflows/ci.yml. AGENTS.md lists justfile
under "Cấm tuyệt đối". The owner has directed this work explicitly, and that
authorization covers justfile, dataflow.json and ci.yml — nothing else. R5 still
applies to CLAUDE.md, AGENTS.md, PROJECT.md, STATE_OF_THE_PROJECT.md,
docs/reference/ and the ADRs. Do NOT change scripts/verify_dataflow.py: read it,
and if it is wrong, report that instead of patching around it.

STEP 0 — THE PRODUCER IS BROKEN. FIX IT FIRST.
`just dataflow-truth` exits 1 today:
    Error: Multiple packages present in workspace,
    please explicitly select one via --package flag.
    Packages present in workspace:
    - anima-engine
    - anima-domain
The recipe at justfile:150 runs `cargo modules dependencies --lib --no-externs`
with no package selection. It was written before crates/anima-domain existed, so
adding that member broke it and nobody noticed — the gate it feeds was never wired
to CI. Add `--package anima-engine`. Verified working on macOS 2026-10-31:
    cd src-tauri && cargo modules dependencies --lib --no-externs --package anima-engine
    -> exit 0, 4665 lines of DOT
This is the same shape as a trap already recorded in STATE_OF_THE_PROJECT.md: "a
gate placed on an artifact cannot see that the artifact's producer has died."

MEASURED TRUTH — you are not discovering this, you are declaring it
Collapsed to module level, the graph has 16 `uses` edges:
    anima_engine_lib            -> core
    anima_engine_lib::AppState  -> core
    ai        -> core | evolution | physics
    commands  -> AppState | core | evolution
    core      -> ai | commands | evolution | physics
    evolution -> ai | core
    physics   -> ai | core
and SIX cyclic pairs:
    ai <-> core        ai <-> evolution    ai <-> physics
    core <-> evolution core <-> physics    commands <-> core

Note two things, because a hand count got both wrong before cargo-modules was run:
  - `commands <-> core` IS a cycle. F-005 does not mention it.
  - `evolution <-> physics` is NOT a cycle in the lib graph, even though
    `grep 'crate::physics'` finds a hit in evolution/ — that import is inside a
    cfg(test) module, which cargo-modules does not analyse.
Declare what cargo-modules prints. If your run disagrees with the 16/6 above, say
so and stop: that means the graph moved since 2026-10-31 and the numbers in
docs/planning/CRATE_SPLIT_PLAN_2026-10-31.md need updating too.

DECLARE REALITY, NOT THE TARGET
The current graph CONTAINS those six cycles and dataflow.json must describe them,
so the gate is GREEN on its first run. A gate that is red the day it lands gets
ignored. The cycles are removed later, in S1, and dataflow.json is updated in the
same PR that removes them (S1.6). scripts/verify_dataflow.py already prints the
cyclic pairs on success as a LUU Y notice — so a green gate still names them.

THE FILE FORMAT — read scripts/verify_dataflow.py before writing it
    { "edges": [ { "from": "...", "to": "...", "invariant": "..." }, ... ] }
The verifier fails in four ways and you must satisfy all four:
  1. an edge in your claim that is not in the truth  -> "edge BIA"
  2. an edge in the truth you did not claim          -> "edge BO SOT"
  3. an `invariant` of fewer than three words        -> "invariant rong tuech"
  4. an `invariant` containing the word "flow"       -> same
So every edge needs a CHECKABLE invariant, and "data flows from A to B" is
explicitly rejected. Write what must hold, e.g. for physics -> core something like
"physics reads ECS component types only, never calls a tick system" — a claim a
reader could disprove. Do not write 16 variations of one sentence; if you cannot
say what an edge guarantees, say so in SURPRISES.

FILES
  justfile          (recipe at :150, add --package)
  dataflow.json     (new, repo root — the path scripts/verify_dataflow.py expects)
  .github/workflows/ci.yml   (one new step in the Rust job)

DONE WHEN
  1) just dataflow-truth                 -> exit 0
  2) just dataflow-verify 1              -> exit 0, prints
                                            "QUA CONG — 16 edge khop hoan toan"
                                            and lists the 6 cyclic pairs as LUU Y
  3) NEGATIVE CONTROL, required. The throwaway edge has to be one that does NOT
     already exist, or the gate correctly stays green and you learn nothing. Of the
     16 edges above, `evolution -> physics` is absent from the lib graph — that is
     the one to use, and the reason it is absent is explained two paragraphs up.
     So: add `use crate::physics::spatial as _;` to a NON-TEST file under
     src-tauri/src/evolution/ (not inside a `#[cfg(test)] mod`, or cargo-modules
     will not see it at all — that is the whole reason the hand count was wrong).
     Re-run 1) and 2), confirm the gate goes RED and NAMES the new
     evolution -> physics edge as an "edge BO SOT", then revert and confirm green.
     Paste both outputs.
     Do NOT use `ai -> physics` for this: it is already one of the 16, so adding
     that import proves nothing.
     A gate nobody has seen fail is not a gate — this project has shipped three of
     those, and src-tauri/src is otherwise out of scope for this task, so revert
     the throwaway before you finish.
  4) the new CI step runs in the Rust job and is green
  5) cd src-tauri && cargo test --features desktop --no-fail-fast
     -> 1153 passed, 0 failed   (nothing here touches Rust; a different number
        means you changed more than this task)
Report the 16 invariants you wrote, one line each.
```

## S0.3 — Xoá shim `core/engine.rs`

```text
GOAL
Delete src-tauri/src/core/engine.rs and point its callers at the real modules.

CONTEXT
The whole file is four `pub use super::*` lines:
    pub use super::agent_systems::*;
    pub use super::networking_systems::*;
    pub use super::resources::{AgentEpochStats, EvolutionQueue, EvolutionReceiver, EvolutionSender};
    pub use super::simulation_lifecycle::*;
It is a historical shim: docs/archive/PHASE1_EXECUTION_PLAN.md records that the real code
moved out of engine.rs long ago. It adds a hop to the dependency graph that hides where a
symbol actually lives — `evolution/` reaches `core::engine` twice, and the graph shows an
edge to `engine` rather than to agent_systems or resources.

FILES
  src-tauri/src/core/engine.rs   (delete)
  src-tauri/src/core/mod.rs      (remove the module declaration)
  every caller of crate::core::engine:: — find them with
    grep -rn 'core::engine' src-tauri/src src-tauri/tests

RULES
Glob re-exports mean a caller may be using a symbol without naming its home module. For
each call site, name the real module explicitly. Do not replace one glob with another.

DONE WHEN
  V1: just dataflow-truth && just dataflow-verify 1  -> exit 0 with dataflow.json updated
      to show the edges now pointing at the real modules instead of `engine`
  V2: cargo test --features desktop --no-fail-fast   -> 1153 passed, 0 failed, and
      check_test_targets.mjs exit 0 (count unchanged here — no tests moved)
  V3: deterministic checksum bit-identical
DEPENDS ON  S0.2
```

## S1.2 — Crate `anima-ecs-types`, bước đầu (template cho mọi bước sau)

```text
GOAL
Create crates/anima-ecs-types and move the two pure type modules into it:
core/components.rs and core/ecs.rs. Re-export both from their old paths.

CONTEXT — why this crate is the keystone
Every dependency cycle in this workspace exists because ECS component types live next to
the systems that use them. Measured 2026-10-31:
  core/components.rs   670 lines, 33 pub struct/enum, 0 pub fn      <- pure types already
  core/ecs.rs          117 lines,  0 pub struct/enum, 1 pub fn
The cyclic edges are almost all a component type, not logic:
  physics   -> core::ecs::CognitiveState, core::ecs::InertiaComponent
  core      -> physics::dynamics::RigidBody
  evolution -> core::components, core::ecs, ai::hrrl::HomeostaticState, ai::cpg::CpgOscillator
So the cycles are not a deep design fault — a layer is missing. This task creates it.

FILES
  crates/anima-ecs-types/Cargo.toml   (new)
  crates/anima-ecs-types/src/lib.rs   (new)
  crates/anima-ecs-types/src/components.rs   (moved from src-tauri/src/core/components.rs)
  crates/anima-ecs-types/src/ecs.rs          (moved from src-tauri/src/core/ecs.rs)
  src-tauri/src/core/mod.rs           (re-export, per rule S2)
  src-tauri/Cargo.toml                (add the dependency)
  dataflow.json                       (update to the new reality)

RULES
The crate may depend on bevy_ecs, glam and anima-domain. NOTHING ELSE from this repo.
License: use `license.workspace = true` — src-tauri/Cargo.toml states workspace licensing
exists precisely so a later crate cannot ship without one.
default-members is already `[".", "crates/*"]`, so the glob picks the crate up. That is
necessary and NOT sufficient: V2 still has to be measured, because the glob says nothing
about [[test]] targets.
Rule S3: move the lines, do not rewrite them. If a type needs an import path fixed, fix the
import, not the type.

DONE WHEN
  V1  just dataflow-truth && just dataflow-verify 1   -> exit 0
  V2  cargo test --features desktop --no-fail-fast    -> total passed must be
      1153 + (tests that moved into the new crate). Report:
        - the before number (1153)
        - the after number
        - how many #[test] functions moved
      and `cargo test -p anima-ecs-types` on its own, with its count.
      Then: node scripts/check_test_targets.mjs out.txt --profile desktop  -> exit 0,
      reporting 7/7 feature-gated targets.
  V3  deterministic checksum bit-identical before/after. Paste both.
  S4  cargo tree -p anima-ecs-types | grep anima-engine   -> no output
  S5  cargo tree --no-default-features -e normal | grep -E 'neo4rs|tokio-tungstenite|burn-wgpu|naga'
      -> no output
DEPENDS ON  S0.2, S0.3
```

## S1.3 — Chuyển `SimRng` (bước rủi ro nhất của cả kế hoạch)

```text
GOAL
Move src-tauri/src/core/determinism.rs (368 lines, SimRng) into anima-ecs-types.

CONTEXT
This is the single highest-risk step in the whole program. SimRng is the determinism core:
rand_chacha is used directly so a draw position can be saved and restored, and every
trajectory in the project depends on draw ORDER. A change in when or how the generator is
constructed changes every run, and the two reproducibility gates
(a_fork_from_a_known_tick..., run_k_then_save_load...) are what would notice.

RULES — stricter than the other steps
1. MEASURE V3 FIRST, before editing anything. Record the deterministic checksum and the
   output of `cargo test --features desktop -p anima-engine --test live_experiment_tests`.
2. Move the file verbatim. Do not change a single expression — not an import order inside a
   function, not a `let` binding, nothing. If the code does not compile after moving, report
   what blocks it rather than adjusting the logic to fit.
3. The source-scanning test that fails if `thread_rng()` reappears must still run and still
   see the moved code. Find it (grep -rn 'thread_rng' src-tauri/tests) and confirm its scan
   path covers crates/ as well as src/. If it does not, say so — that is a gate hole and it
   is MORE important than this task.

DONE WHEN
  V3  checksum bit-identical, AND live_experiment_tests -> 17 passed; 0 failed
  V1, V2, S4, S5 as in S1.2
  the thread_rng() source-scan test still covers the moved file (state how you verified it)
DEPENDS ON  S1.2
```

## S1.4 / S1.5 — Chuyển type của `ai` và `physics`

```text
GOAL
Move the pure types out of ai/ and physics/ into anima-ecs-types, leaving systems behind.
  ai/hrrl.rs          206 lines, 6 pub struct, 0 pub fn  -> types move, whole file likely moves
  ai/cpg.rs            60 lines, 2 pub struct, 1 pub fn  -> CpgOscillator moves, keep the fn in ai
  physics/dynamics.rs 174 lines, 2 pub struct, 2 pub fn  -> RigidBody moves, keep the fns in physics

CONTEXT
These are the types that make physics, ai and evolution point at each other:
  physics   -> ai::hrrl::HomeostaticState, ai::hrrl::LastTransitionState
  evolution -> ai::hrrl::HomeostaticState, ai::cpg::CpgOscillator, ai::model::BrainModel
  core      -> physics::dynamics::RigidBody   (the whole core -> physics edge, 4 call sites:
               core/agent_systems.rs, core/simulation_state.rs, core/spawn.rs, core/world_systems.rs)

DO NOT TOUCH ai::model::BrainModel in this task. ai/model.rs depends on
evolution::brain_genotype::{BrainGenotype, LearnScratch, learn_step, action_index} and that
edge is INTENTIONAL: CLAUDE.md states there is one A2C objective, brain_genotype::learn_step
is the canonical one, and the EB-S02 parity gate exists to keep ai/model.rs matching it. So
anima-ai depends on anima-evolution by design. Do not "break the cycle" there.

DONE WHEN
  V1 shows the `core -> physics` edge GONE, and the type edges from physics and evolution
     into ai GONE
  V1, V2, V3, S4, S5 all green
  the EB-S02 parity gate still passes (find and name it)
DEPENDS ON  S1.3
```

## S1.6 — Khai đồ thị không vòng và chứng minh

```text
GOAL
Update dataflow.json to a graph with NO cycles among {ai, core, physics, evolution}, and
prove the real graph matches it.

CONTEXT
S1.2 through S1.5 removed the type edges that created the cycles. This task is where the
project's dependency contract stops describing a cyclic mess and starts describing a layering.
This closes finding F-005, which has been open since 2026-08-11 with "Test khoá lại: chưa có".

DONE WHEN
  1) just dataflow-truth && just dataflow-verify 1   -> exit 0 on the acyclic contract
  2) cargo modules dependencies --lib --no-externs shows no cycle among ai, core, physics,
     evolution. State how you checked for cycles, and paste the edge list.
  3) NEGATIVE CONTROL: reintroduce one removed edge, show the gate goes RED and names it,
     revert.
  4) Update FINDINGS.md F-005: the measured count is SIX cyclic pairs not three (it omitted
     evolution), the gate that now locks it, and status FIXED with this commit.
     FINDINGS.md is the one file AGENTS.md allows you to write besides src-tauri/tests.
  5) cargo test --features desktop --no-fail-fast  -> 0 failed
DEPENDS ON  S1.5
```

## S2.2 — Cạnh sai thật

```text
GOAL
Remove the one cyclic edge that is NOT a type: physics calling a system function in core.

CONTEXT
  src-tauri/src/physics/*.rs calls crate::core::agent_systems::apply_inertia_to_oscillators
A physics module reaching up into the systems layer is a real layering fault, not a missing
type. Two options; pick one and write down why:
  (a) move apply_inertia_to_oscillators into anima-physics — correct if the function is
      physics logic that core merely schedules;
  (b) invert it — core calls into physics instead — correct if the function needs
      system-level context (Commands, Res, Query) that does not belong in a leaf crate.
Read the function before choosing. Report which it is and the evidence.

RULES
This is the step most likely to tempt a behaviour change. Rule S7: if making the edge go
away requires changing what the code does, STOP and report. The schedule order in
core::simulation_schedule::build_tick_schedule is the one place the tick schedule is built
and it is load-bearing for determinism — if your change affects when this function runs
relative to others, that is a behaviour change, not a refactor.

DONE WHEN
  V1 shows no physics -> core edge
  V3 checksum bit-identical  (this is the real test of whether you changed behaviour)
  V2, S4, S5 green
DEPENDS ON  S2.1
```

## S2.1, S2.3, S2.4, S3, S4 — cùng khuôn

```text
Các bước còn lại dùng đúng khuôn của S1.2: tạo crate, chuyển file nguyên trạng, pub use lại
từ đường cũ, cập nhật dataflow.json, rồi V1/V2/V3/S4/S5.

  S2.1  anima-physics      588 dòng, 3 file       <- nhỏ nhất, làm trước để học quy trình
  S2.3  anima-evolution    4.3k                   genotype, brain_genotype 1165, mutation,
                                                  crossover, fitness, map_elites, lineage,
                                                  newick, simplify
  S2.4  anima-ai           2.1k                   model (Burn), hrrl/cpg systems.
                                                  GIỮ anima-ai -> anima-evolution.
                                                  Kiểm thêm: cargo tree --no-default-features
                                                  không chứa burn-wgpu/naga.
  S3.1  anima-persistence  ~4k                    snapshot 1687, simulation_state 1397,
                                                  world_artifact 780, sha256 173.
                                                  Kiểm thêm: 8 target feature-gated vẫn chạy
                                                  (đo 2026-10-31; đọc từ output của gate).
  S4.1  anima-lab          ~16k                   trait ExperimentModel + experiment_runner 4497
                                                  + experiment 2783
  S4.2  anima-lab (tiếp)                          reference_world 1628, scenario 849,
                                                  evolution_pathway 1484, exotic_energy 1839,
                                                  observer 895
  S4.3  core/live_experiment.rs Ở LẠI anima-engine — nó là adapter (LiveExperimentAdapter:
        ExperimentModel). Kiểm thêm: live_experiment_tests -> 17 passed.
  S4.4  cargo tree -p anima-lab không chứa anima-engine.

Một bước = một PR. Không gộp.
```

---

## Hai điều phải xong trước khi giao bước nào

1. **S0.1 (= A1 của brief chính)** — `scripts/check_test_targets.mjs` đang chết vì ba khối conflict
   marker. Trong lúc nó chết, **V2 không đo được**, và tách crate khi không đo được số test là đúng
   tổ hợp đã làm 20 test của `anima-domain` rơi khỏi mọi gate.
2. **Quyết định thứ tự với HT-C** — tách crate đổi chỗ gần như mọi file. Merge nhánh 68 commit vào
   giữa lúc đó là tự tạo xung đột toàn cây. Chọn **A1 → HT-C → tách**, hoặc **A1 → tách → HT-C**;
   không trộn. Đây là quyết định của chủ dự án, không phải của agent.
