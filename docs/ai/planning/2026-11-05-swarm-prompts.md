---
title: Prompt cho swarm Antigravity — lead, verifier, luật swarm và thẻ task
status: proposed
owner: maintainers
last_reviewed: 2026-11-05
review_cycle: weekly
---

# Prompt cho swarm Antigravity

> Kế hoạch, thứ tự đợt và lý do của từng thẻ ở
> [`2026-11-05-swarm-master-plan.md`](2026-11-05-swarm-master-plan.md). File này chỉ chứa thứ để
> **dán**.
>
> Prompt viết bằng tiếng Anh, theo quy ước của brief 31/10 (brief đó từng ghi việc lẫn tiếng Việt vào
> prompt tiếng Anh là một lỗi). Phần giải thích quanh prompt viết bằng tiếng Việt.

## Cách dùng

1. Dán **PROMPT 0** vào agent lead của teamwork. Điền khối `DECISIONS`; ô nào để trống thì thẻ phụ
   thuộc vào ô đó sẽ không chạy.
2. Lead tự lấy BLOCK 0 (từ brief 31/10), BLOCK S và các thẻ trong file này qua `git show`, rồi giao
   việc cho worker. Mỗi worker nhận đúng: **BLOCK 0 + BLOCK S + một thẻ**, không tóm tắt lại.
3. Nếu teamwork không cho lead tự tạo agent con: bảo lead *"print the full worker prompt for card
   X"*, rồi bạn mở agent mới và dán vào.
4. Mỗi worker xong thì lead giao cho một **VERIFIER** (agent mới, context sạch).
5. Bạn chỉ merge PR nào verifier đã ghi `VERIFIED`, và chỉ sau khi tự đọc diff.

Thẻ có nhãn **(brief)** lấy thân task nguyên văn từ
`docs/ai/planning/2026-10-31-antigravity-brief.md` trên nhánh `feat/ecological-selection`; thẻ ở đây
chỉ bọc thêm phần swarm (base, nhánh, path được phép, kiểm thêm).

---

## §1 PROMPT 0 — Lead

```text
ROLE
You are the LEAD of a team of coding agents working on the Anima-Engine repository
(github.com/DuongNAD/Anima-Engine). You plan, dispatch, and audit. You do NOT write
production code, you do NOT merge, and you do NOT edit a worker's branch to make it pass.
The owner merges. Your output is a status table the owner can trust.

SOURCES (read with git, do not paraphrase from memory)
  git fetch origin
  PLAN   = git show origin/claude/funny-thompson-jj42fe:docs/ai/planning/2026-11-05-swarm-master-plan.md
  CARDS  = git show origin/claude/funny-thompson-jj42fe:docs/ai/planning/2026-11-05-swarm-prompts.md
  BRIEF  = git show origin/feat/ecological-selection:docs/ai/planning/2026-10-31-antigravity-brief.md
  (If the owner has merged these files into feat/ecological-selection, read them from there.)
  BLOCK 0 = the fenced text under "## BLOCK 0" in BRIEF.
  BLOCK S = the fenced text under "## §2 BLOCK S" in CARDS.
  VERIFIER TEMPLATE = the fenced text under "## §3 VERIFIER" in CARDS.
  Each card = one fenced block under a "### CARD <ID>" heading in CARDS.
  Cards marked (brief) also need the brief section they name, extracted verbatim from BRIEF.

DECISIONS — filled by the owner. An empty value blocks every card that lists it.
  QD-A'     (which mortality system survives HT-C)          =
  QD-B      (src-tauri/src write authorization line)        =
  QD-C      (licence path: A = drop ml-wgpu from desktop | B = legal decision) =
  QD-2/QD-H (Map 2 and Research Dashboard: keep code, remove fake data | remove feature) =
  QD-4      (population turnover mechanism)                 =
  QD-5      (biological reproduction ADR approved? yes/no + ADR path) =
  QD-6      (IPC contract location)                         =
  QD-F028   (commit real icons: yes/no)                     =
  QD-EN     (experiment names: E2R = brain redux, E3 = productivity gradient? yes/no) =
  QD-P1     (paper type)                                    =
  QD-P2     (where paper drafts live)                       =
  QD-1      (release level target)                          =
  R5-AUTH   (per-card authorizations to write R5 files, one line each) =
  CURRENT WAVE =

PROCEDURE
1. Preflight. Run and paste: `git fetch origin`, `git log --oneline -1` for
   origin/main, origin/feat/ecological-selection, origin/fix/ci-gates-green. Record
   these SHAs; every card's BASE is checked against them.
2. Eligibility. For CURRENT WAVE, list every card. A card is ELIGIBLE only if:
   a. each DEPENDS ON item is satisfied, PROVEN by a command (for a merged card:
      `git merge-base --is-ancestor <its merge sha> origin/feat/ecological-selection`
      exits 0; for an owner step: the owner said so in this conversation);
   b. every DECISION it lists has a value above;
   c. its AUTHORIZATION, if required, appears in R5-AUTH.
   Print the eligibility table before dispatching anything.
3. Conflict check. Two cards may run at the same time only if their ALLOWED PATHS do
   not overlap. Treat these as shared and serialize any two cards that list one:
   .github/workflows/ci.yml, package.json, tests/package.json, src-tauri/Cargo.toml,
   src-tauri/Cargo.lock, src-tauri/src/core/simulation_schedule.rs,
   src-tauri/src/core/live_experiment.rs, src-tauri/src/core/agent_systems.rs,
   src-tauri/src/lib.rs.
4. Capacity. At most 2 cards tagged [RUST] at once (CPU contention on this machine
   turns test timeouts into fake regressions). Up to 4 more non-[RUST] cards.
5. Dispatch. Give each worker exactly: BLOCK 0, then BLOCK S, then the card verbatim
   (plus the named brief section for (brief) cards, plus the R5-AUTH line if any).
   Never shorten, merge, or reword cards. Never give one worker two cards.
6. Verification. When a worker reports, start a NEW agent with the VERIFIER TEMPLATE,
   the card, the worker's report, and the branch name. Never let the worker verify
   itself, and never verify it yourself.
7. Verdicts. Accept only VERIFIED. On MISMATCH: send the verifier's list back to the
   SAME worker once. A second MISMATCH stops that card and goes to the owner.
8. Stop the whole wave and report to the owner immediately if any worker or verifier
   finds: a path outside ALLOWED PATHS; a deleted, skipped, ignored or loosened test;
   a narrowed test script; a fabricated value; a write to an R5 file without R5-AUTH;
   a new file at the repository root; a push to any branch but the worker's own.
9. Never: merge; push to main or feat/ecological-selection; close, approve, or comment
   on a PR; delete a branch; edit a card; run `npm run tauri:dev`, `cargo run`, or the
   desktop app.

STATUS REPORT (after every dispatch round and at wave end)
  | card | worker branch | PR | verdict | DONE WHEN result (exit code) | blocker |
  Then: cards still blocked and the exact decision or dependency blocking each.
  Then: findings that need the owner (one line each, with the file:line or command).
Do not summarize a card as done unless its verdict is VERIFIED.
```

---

## §2 BLOCK S — luật swarm (dán sau BLOCK 0 cho mọi worker)

```text
SWARM RULES — these add to BLOCK 0; where they differ, the stricter rule wins.

S1  ISOLATION. You work in your own git worktree on your own branch:
      git fetch origin
      git worktree add ../anima-wt/<CARD-ID> -b <BRANCH> <BASE-SHA>
    Do every edit, build and commit inside that directory. Before each edit and each
    commit, run `git rev-parse --show-toplevel` and `git branch --show-current`; if
    either is not yours, stop. Never open, read or edit another worker's worktree.

S2  SCOPE. ALLOWED PATHS in your card is the complete list of paths you may create,
    modify or delete. It is also the explicit permission BLOCK 0 R6 refers to. Before
    every commit run `git diff --name-only <BASE-SHA>`; every path must match ALLOWED
    PATHS, and the count must be <= MAX FILES. If the task cannot be done inside them,
    stop and report which path you need and why.

S3  GIT. Push only your branch: `git push -u origin <BRANCH>`. Open a DRAFT pull
    request to PR TARGET. Never merge, close, approve or comment on any pull request.
    Never push to main or feat/ecological-selection. Never rebase or force-push a
    branch you did not create in this task.

S4  DEPENDENCIES. Before starting, prove each DEPENDS ON item with the command the
    card gives. If one fails, stop and report BLOCKED. Do not do a dependency's work.

S5  DECISIONS. If a DECISION or AUTHORIZATION line in your card is empty or says
    "<owner fills>", stop and report BLOCKED. Never choose for the owner.

S6  RESOURCES. Run one cargo command at a time. Use `cargo test --features desktop -j 2`
    unless the card says otherwise. Never run `npm run tauri:dev`, `cargo run`, or the
    desktop app. When your PR is pushed and verified, `cargo clean` in your worktree.

S7  SELF-CHECK before you report. Run each and paste the output (empty is fine):
      git diff <BASE-SHA> -- '*test*' '*tests/*' | grep -nE '^\+.*(>=|<=|#\[ignore\]|should_panic|\.skip\(|test\.skip|it\.skip|describe\.skip|xit\(|todo!)'
      git diff --diff-filter=D --name-only <BASE-SHA> -- '*test*' '*tests/*'
      git diff <BASE-SHA> -- package.json tests/package.json | grep -n '"test\|"lint\|"typecheck'
      git diff --name-only <BASE-SHA> | grep -v '/' || true      # new or changed root files
    Each non-empty line must be justified in the report, or reverted.

S8  REPORT. Use BLOCK 0's report format, and add at the top:
      CARD <ID> · BASE <sha> · HEAD <sha> · BRANCH <name> · PR <url>
      git diff --stat <BASE-SHA>..HEAD   (pasted)
      TESTS ADDED / MODIFIED / REMOVED   (file::name, one per line)
    If DONE WHEN did not exit 0, the first word of your report is NOT DONE.

S9  DATES AND NUMBERS. Write the real date you ran something. Every number in a doc you
    write either comes from a command you ran (give command and date) or is quoted from
    a file (give path:line and say it is that file's measurement). Never round, smooth
    or "estimate" a measured number.

S10 LANGUAGE. Reports in English. Internal docs in Vietnamese unless the card says
    English (paper material is English).
```

---

## §3 VERIFIER

```text
ROLE
You are the VERIFIER for card <CARD-ID>. You did not write this work. Your only job is to
find every place where the worker's report and the branch disagree, or where the branch
breaks the card or BLOCK 0/BLOCK S. You may not edit, commit, push, or post on GitHub.

INPUTS: the card text, the worker's report, the branch name.

STEPS
1. git fetch origin; git worktree add ../anima-wt/verify-<CARD-ID> origin/<BRANCH> --detach
   Record HEAD. It must equal the HEAD in the report.
2. Scope: git diff --name-only <BASE-SHA>..HEAD. Every path must match ALLOWED PATHS;
   count must be <= MAX FILES. List violations.
3. Green-optimization scan: run BLOCK S S7's four commands yourself. Any line the report
   does not justify is a violation.
4. Run every DONE WHEN command yourself from a clean build. Paste raw output. Compare exit
   codes and counts with the report.
5. Every symbol, test name, file or commit the report cites: locate it with grep or
   `git cat-file -t` at HEAD. A citation you cannot locate is a MISMATCH.
6. Every WHAT CHANGED line must correspond to a hunk in `git diff <BASE-SHA>..HEAD`; every
   hunk must be covered by a WHAT CHANGED line. List both kinds of gap.
7. Run the card's VERIFY EXTRA checks.
8. Look for R3 violations in the diff: hardcoded seeds, fixed ratios, constants presented
   as measurements, synthetic data, mock fallbacks on a real data path, images generated
   to make a hash differ.

VERDICT — first line, one of:
  VERIFIED
  MISMATCH: <numbered list, each with the command output that shows it>
  BLOCKED: <why verification could not run>
Then the raw outputs of steps 2–7.
```

---

## §4 Dòng uỷ quyền mẫu (chủ dự án điền vào `R5-AUTH`)

```text
AUTHORIZATION (owner, <YYYY-MM-DD>): for card <CARD-ID> only, you may write to <exact paths>.
This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these paths
and no others.
```

---

## §5 Thẻ task

### Đợt 1

### CARD T1

```text
CARD T1 — B3b: the five gates B3/B4 turned red (brief)
WAVE 1 · [RUST] · MAX FILES 25
DEPENDS ON  none (#38 and #39 are already ancestors of fix/ci-gates-green, checked 2026-11-05 with
            `git merge-base --is-ancestor`)
BASE        origin/fix/ci-gates-green (record its sha; on 2026-11-05 it was 78b2e85)
BRANCH      swarm/t1-b3b
PR TARGET   fix/ci-gates-green
DECISION    QD-B
AUTHORIZATION  none beyond what the brief section states
ALLOWED PATHS  the paths in the brief section's own AUTHORIZATION paragraph: src-tauri/Cargo.lock,
               src-tauri/deny.toml, scripts/check_bundle_budget.mjs, licensing/bundle-closure.json,
               and vite.config.ts only if its task 3 concludes a chunking-config change.
               Add any other path only if the brief section names it, and say where it does.
TASK BODY   Extract the fenced block under "### B3b" from
            origin/feat/ecological-selection:docs/ai/planning/2026-10-31-antigravity-brief.md
            and follow it verbatim. This card overrides only BASE, BRANCH, PR TARGET and
            ALLOWED PATHS.
VERIFY EXTRA  The verifier re-runs every gate B3b names and checks that no exemption was
              added to audit.toml/deny.toml without the forward check
              `cargo tree -e all --features desktop --target all | grep -c " <crate> v"`.
```

### CARD T2

```text
CARD T2 — HT-C: merge the PR stack into the trunk (brief)
WAVE 1 · [RUST] · MAX FILES n/a (merge commit only)
DEPENDS ON  T1 merged into fix/ci-gates-green (check: `git log --oneline -1 origin/fix/ci-gates-green`
            is no longer 78b2e85, and contains T1's commits)
BASE        origin/feat/ecological-selection (record sha)
BRANCH      swarm/t2-htc-merge
PR TARGET   feat/ecological-selection
DECISION    QD-B (QD-A' is NOT needed: this card measures, the owner decides afterwards)
ALLOWED PATHS  the merge itself; plus, only if the brief requires it, one new test file for the
               one-agent starvation test and the Default impl of HomeostaticState in src-tauri/src/ai/hrrl.rs
TASK BODY   Extract the fenced block under "### HT-C" from the brief and follow it verbatim.
            Leave PROJECT.md, AGENTS.md and docs/planning/STATE_OF_THE_PROJECT.md for the owner,
            exactly as it says.
VERIFY EXTRA  The verifier reruns the 180-tick checksum in at least 6 separate processes on the
              merge and confirms the six values agree; and confirms the starvation test exists and
              what it shows.
```

### CARD D0

```text
CARD D0 — Audit open pull requests and remote branches (read-only)
WAVE 1 · MAX FILES 1
DEPENDS ON  none
BASE        origin/feat/ecological-selection
BRANCH      swarm/d0-pr-branch-audit
PR TARGET   feat/ecological-selection
DECISION    none
ALLOWED PATHS  docs/ai/monitoring/pr-branch-audit-<YYYY-MM-DD>.md

GOAL
Tell the owner, with evidence, which open pull requests and remote branches still carry
work that is not on the trunk, and which are superseded. Change nothing on GitHub.

CONTEXT (observed 2026-11-05)
Open PRs: #30, #31, #34, #35, #36, #37, #38, #39, #40, #41. About 60 remote branches
(`git ls-remote --heads origin`). #35/#36/#37 (HT0.5a/b/c) carry changes whose
counterparts reached main in ef9bc3a and a18c19f, but the files are not byte-identical
(235, 279 and 1025 diff lines). #34 re-measured main at 1b3e7ef, which main has moved past.

STEPS
1. For each open PR and each remote branch: commits ahead/behind origin/main and
   origin/feat/ecological-selection; `git cherry -v <trunk> <branch>` to find patches
   already applied; last commit date; PR CI conclusion (GitHub, read-only).
2. For partially superseded PRs (#35–#37), list each hunk that is NOT on the trunk.
3. Classify each: SUPERSEDED (all patches on trunk) / REBASE (live work) / STALE-DOC /
   UNKNOWN. Give the command output that supports the class.
4. Do NOT close, comment on, or delete anything.

DONE WHEN
  the report exists, every row has a class and a supporting command, and
  `node scripts/check_docs_links.mjs` exits 0.
```

### CARD D1

```text
CARD D1 — Triage AUDIT_REPORT.md (read-only)
WAVE 1 · MAX FILES 1
DEPENDS ON  none
BASE        origin/feat/ecological-selection
BRANCH      swarm/d1-audit-triage
PR TARGET   feat/ecological-selection
DECISION    none
ALLOWED PATHS  docs/ai/monitoring/audit-triage-<YYYY-MM-DD>.md

GOAL
Give every finding in the automated audit a status with evidence. BLOCK 0 R9 applies in
full: an audit finding is a hypothesis. One of them (SIM-01) was applied as written and
blinded every agent's sensors while the suite stayed green.

CONTEXT
Two copies exist on main: AUDIT_REPORT.md and docs/AUDIT_REPORT.md (the trunk may have
moved them; find them with `git ls-files | grep -i audit_report`). Read both; treat their
union as the finding list.

STEPS
1. For each finding: locate the cited code at the trunk sha (`git show <sha>:<path>` and
   line). If the code no longer exists or was changed, say by which commit
   (`git log -L` or `git log -S`).
2. Status: FIXED (commit), FALSE (the code shows the claim is wrong — quote it), OPEN
   (still present), or NEEDS-TEST (only a running test could decide).
3. For OPEN and NEEDS-TEST: write a proposed follow-up card in the template of §6 of
   docs/ai/planning/2026-11-05-swarm-prompts.md, with a failing-test-first step.
4. Do not change code and do not write tests in this card.

DONE WHEN
  every finding has a status and evidence; the two copies are reconciled into one list;
  `node scripts/check_docs_links.mjs` exits 0.
```

### CARD P1

```text
CARD P1 — Incident catalogue for the methods paper (read-only)
WAVE 1 · MAX FILES 3
DEPENDS ON  none
BASE        origin/feat/ecological-selection
BRANCH      swarm/p1-incidents
PR TARGET   the branch named in QD-P2
DECISION    QD-P2
ALLOWED PATHS  docs/paper/data/incidents.csv, docs/paper/data/incidents.md,
               scripts/paper/incident_dates.mjs

GOAL
A verifiable table of every documented failure of the kind "a gate exited 0 while
something was wrong", for the case-study section of a methods paper.

SOURCES (all on the trunk unless noted)
  FINDINGS.md
  docs/planning/COMPLETION_ROADMAP_2026-10-16.md §1.2 (six process failures) — on main
  docs/planning/STATE_OF_THE_PROJECT.md §1.e, §3.0 ("bẫy" lists), §4
  artifacts/experiments/e2-evolved-brain-default/RESULT.md §7
  commit messages of fa938c5 (#38) and 03b331f (#39)

COLUMNS
  id, title, class, introduced_commit, detected_commit, detected_how,
  introduced_date, detected_date, days_to_detection, was_suite_green (yes/no/unknown),
  source (path:line)
class is one of: vacuous-gate, fabricated-evidence, loosened-test, narrowed-command,
silent-semantic-change, nondeterminism, contract-overwrite, toolchain-drift, other.

RULES
- A row needs a source anchor and at least one commit hash verified by
  `git cat-file -t <hash>` (paste the outputs). No anchor, no row.
- Dates come from `git show -s --format=%cI <hash>`, computed by the script, never by hand.
- Where a doc gives an interval ("red for two and a half months") and the commits give
  another, report both and say which is which.
- Do not interpret beyond the source text.

DONE WHEN
  node scripts/paper/incident_dates.mjs --check   exits 0 (it recomputes every date and day
  count from git and compares with the CSV)
```

### CARD P2

```text
CARD P2 — Related work with verified citations
WAVE 1 · MAX FILES 2
DEPENDS ON  none
BASE        origin/feat/ecological-selection
BRANCH      swarm/p2-related-work
PR TARGET   the branch named in QD-P2
DECISION    QD-P1, QD-P2
ALLOWED PATHS  docs/paper/related_work.md, docs/paper/references.bib

GOAL
A related-work section for the paper type in QD-P1, in which every citation exists and
says what we claim it says.

TOPICS (start here; add others only with a reason)
  artificial-life simulators with evolving agents (e.g. Polyworld, Avida, Tierra,
  Framsticks, ALIEN, The Bibites, JaxLife, and the per-agent-network work cited as
  arXiv 2510.18221 in docs/research/MAP_AND_ML_UPGRADE_RESEARCH.md);
  reproducibility and documentation of agent-based models (e.g. the ODD protocol);
  preregistration in computational/simulation science;
  deterministic simulation and replay in ECS/game engines;
  closed energy/mass budgets in ecological simulation;
  quality-diversity / MAP-Elites.

HARD RULES — LLM-invented references are the main risk of this card
- Cite only sources you OPENED in this session. For each, record: full reference, DOI or
  stable URL, access date, and one verbatim quote (<= 40 words) from the source that
  supports the exact sentence in which you cite it.
- If you cannot open a source, list it under "UNVERIFIED — not cited" and keep it out of
  references.bib.
- Never describe a paper's result beyond what your quote supports.
- Do not cite from memory, from another paper's bibliography, or from this repository's
  docs without opening the original.
- Copyleft projects (Avida, ALIEN, Thrive, SLiM): cite papers/docs only; do not read or
  quote their source code (docs/research/OPEN_SOURCE_LANDSCAPE.md).

DONE WHEN
  every entry in references.bib has a DOI or URL and a matching quote in related_work.md;
  the owner can open each link (the verifier opens a random 30 % and reports which).
VERIFY EXTRA  the verifier opens at least 30 % of the links and checks each quote verbatim.
```

### CARD P3

```text
CARD P3 — E2 figures generated from the artifacts
WAVE 1 · MAX FILES 5
DEPENDS ON  none
BASE        origin/feat/ecological-selection
BRANCH      swarm/p3-e2-figures
PR TARGET   the branch named in QD-P2
DECISION    QD-P2
ALLOWED PATHS  scripts/paper/e2_figures.mjs, docs/paper/figures/e2_*.svg,
               docs/paper/figures/e2_captions.md

GOAL
Figures for experiment E2, produced by a script that reads the committed artifacts, so no
number in a figure was typed by hand.

INPUTS
  artifacts/experiments/e2-evolved-brain-default/paired-report.json
    pairs[i].control.series and pairs[i].treatment.series (30 samples each; inspect the
    item shape yourself before using it)
  artifacts/experiments/e2-evolved-brain-default/RESULT.md §4 (first-zero ticks table,
    and means control 5250 / treatment 8650)

STEPS
1. Run `npm run check:e2` first and paste the output; if it fails, stop.
2. Node only, no new dependencies; write SVG directly.
3. Figure A: live.mean_agent_energy over ticks, both arms, all 12 seeds.
   Figure B: first tick at which each arm reads 0, per seed (paired dots).
4. The script recomputes the first-zero tick per seed per arm from the series and asserts
   it equals the RESULT.md §4 table, and that the means are 5250 and 8650. If not, exit 1
   and report the difference — do not adjust anything.
5. Captions state: E2 at commit 9c57184, headless adapter, N = 12 seeds of ONE world
   design, sample period 600 ticks, and that first-zero tick was NOT a registered
   observable (RESULT.md §6).

DONE WHEN
  node scripts/paper/e2_figures.mjs --check   exits 0 and regenerates byte-identical SVGs
```

### Đợt 2 — sau khi T2 đã merge vào `feat/ecological-selection`

Mọi thẻ Đợt 2 dùng chung:

```text
BASE        origin/feat/ecological-selection, at a sha that contains T2's merge
            (check: `git merge-base --is-ancestor <T2 merge sha> origin/feat/ecological-selection`)
PR TARGET   feat/ecological-selection
```

### CARD G1

```text
CARD G1 — B2: allocator instrument and birth/death allocations (brief), rebased onto the trunk
WAVE 2 · [RUST] · MAX FILES 12
DEPENDS ON  T2 merged; QD-A' decided
BRANCH      swarm/g1-b2-zero-alloc
DECISION    QD-B, QD-A'
ALLOWED PATHS  src-tauri/src/core/agent_systems.rs, src-tauri/src/core/mortality.rs,
               src-tauri/src/core/spawn.rs, src-tauri/tests/common/allocator.rs,
               src-tauri/tests/thread_tracking_allocator_tests.rs,
               src-tauri/tests/*zero_alloc*.rs, src-tauri/tests/*allocation*.rs
TASK BODY   The fenced block under "### B2" in the brief. Work already exists on PR #41
            (branch fix/birth-death-zero-alloc: 36e18d8, 2c5423b). Cherry-pick those commits
            onto your base first; then finish B2's DONE WHEN against the trunk, where QD-A'
            decides which mortality system remains.
VERIFY EXTRA  The verifier removes the fix in a scratch copy and confirms the extended gate goes
              RED naming the allocating system; then restores it.
```

### CARD G2

```text
CARD G2 — B5: restore the full E2E suite and gate the spec list (brief)
WAVE 2 · MAX FILES 8
DEPENDS ON  T2 merged
BRANCH      swarm/g2-e2e-restore
DECISION    none
ALLOWED PATHS  tests/package.json, tests/e2e/**, scripts/e2e_spec_policy.mjs,
               scripts/check_e2e_specs.mjs, .github/workflows/ci.yml
TASK BODY   The fenced block under "### B5" in the brief.
NOTE        .github/workflows/ci.yml is shared: the lead must not run G9 at the same time.
VERIFY EXTRA  The verifier deletes one spec from the committed list in a scratch copy and pastes
              the red output naming it.
```

### CARD G3

```text
CARD G3 — B6(a): remove fabricated scientific values from telemetry and provenance (brief)
WAVE 2 · [RUST] · MAX FILES 8
DEPENDS ON  T2 merged
BRANCH      swarm/g3-telemetry-real-values
DECISION    QD-B
ALLOWED PATHS  src-tauri/src/core/telemetry_export.rs, src-tauri/src/commands/simulation.rs,
               src-tauri/src/core/energy*.rs, src-tauri/tests/telemetry*.rs,
               src/types/generated/** (regenerated only, never hand-edited)
TASK BODY   Part (a) of the fenced block under "### B6" in the brief.
CONTEXT     On main at a43872c: RunProvenance::new(42, …) at commands/simulation.rs:675;
            total_animals * 0.70 / 0.30 at telemetry_export.rs:264-265; biomass: 10000.0 at :271;
            invented floors (.max(1.0), .max(0.1), .max(0.05)) around :302-309. Re-locate on the trunk.
RULE        Replacing one constant with another is not a fix. Either compute from the running
            world (the run's real seed; biomass by trophic role from the agents themselves; flux
            from an EnergyFluxMatrix that something actually writes), or remove the output.
VERIFY EXTRA  `grep -nE '\b42\b|0\.70|0\.30|10000\.0' <the two files>` at HEAD, each hit justified.
```

### CARD G4

```text
CARD G4 — B6(b): dead IPC calls and synthetic data on the Research Dashboard (brief)
WAVE 2 · MAX FILES 12
DEPENDS ON  T2 merged
BRANCH      swarm/g4-dashboard-real-data
DECISION    QD-2/QD-H
ALLOWED PATHS  src/components/ResearchDashboard/**, tests/frontend/**ResearchDashboard**,
               tests/frontend/**research**, src/__tests__/**Research**
               (plus src-tauri/src/commands/** and src/types/generated/** ONLY if QD-H says
               "implement the commands", with an R5-AUTH line for the IPC contract doc)
TASK BODY   Part (b) of the fenced block under "### B6" in the brief.
DEFAULT     If QD-H says "keep code, remove fake data": a panel whose backend command does not
            exist shows an explicit "not available in this build" state and calls nothing;
            delete the Math.sin divergence traces and invented checksums; DataExportDialog
            surfaces the error instead of a mock response.
VERIFY EXTRA  `grep -rnE 'Math\.sin|mock|Fallback|fake|placeholder' src/components/ResearchDashboard`
              at HEAD, each hit justified.
```

### CARD G5

```text
CARD G5 — B6(c): ledger bypass and the suspected PathogenTransmission EU leak (brief)
WAVE 2 · [RUST] · MAX FILES 8
DEPENDS ON  G1 merged (both edit agent_systems.rs)
BRANCH      swarm/g5-ledger-closure
DECISION    QD-B, QD-A'
ALLOWED PATHS  src-tauri/src/core/agent_systems.rs, src-tauri/src/core/live_experiment.rs,
               src-tauri/src/core/environmental_systems.rs, src-tauri/src/core/energy*.rs,
               src-tauri/tests/ledger*.rs, src-tauri/tests/eu_*.rs
TASK BODY   Part (c) of the fenced block under "### B6" in the brief. The failing test is
            committed FIRST, as its own commit, and the report shows it red at that commit.
RULE        CLAUDE.md: any new energy charge goes into total_cost in metabolic_decay_system,
            never a separate deduction.
VERIFY EXTRA  The verifier checks out the test-only commit and confirms it is red; then HEAD and
              confirms it is green.
```

### CARD G6

```text
CARD G6 — Remove Map 2's fabricated evidence
WAVE 2 · MAX FILES 20
DEPENDS ON  T2 merged
BRANCH      swarm/g6-map2-evidence
DECISION    QD-2/QD-H
ALLOWED PATHS  scripts/verify_map2_evidence.ts, artifacts/maps/map2/**, map2_manifest.json,
               map2_high_angle.png, map2_live_preview.png, map2_orbit.png, screenshot.png,
               tests/frontend/map2/map2ManifestEvidence.test.ts, tests/frontend/map2/**Evidence**,
               docs/ai/TEST_READY.md, docs/ai/testing/**map2**

GOAL
No file in the repository presents itself as captured evidence of Map 2 unless it was
captured. Keep the Map 2 code unless QD-2 says remove the feature.

CONTEXT (observed on main @ a43872c, 2026-11-05)
- scripts/verify_map2_evidence.ts:18-21 defines MINIMAL_PNG_BYTES (a 1x1 PNG); :54-66 writes
  it plus an HTML comment per view so each "canonical view" gets a distinct hash.
- artifacts/maps/map2/views/*.png are 8 files of 114–125 bytes: those fake images.
- The roadmap of 2026-10-16 §4.1 cluster I says artifacts/maps/map2/world_256.anmw is a copy of
  Map 1's world artifact. Verify by hash before acting on it.
- Root PNGs (map2_high_angle.png 549,595 B, map2_live_preview.png, map2_orbit.png, screenshot.png)
  are real-size images committed to the repository root by an agent.
- TEST_READY.md claims "Certification Status: 100% PASS (Zero Defects)" with no command or date.
  (The trunk may have moved it to docs/ai/; find it.)

STEPS
1. Delete the fake-image code path. Either replace it with a real capture (model it on
   `npm run capture:views` / scripts/capture_map2_live.mjs if those really render), or
   delete the script and the 8 fake PNGs, and make the Map 2 manifest say evidence is
   NOT CAPTURED. Do not generate any image you did not render.
2. If world_256.anmw for Map 2 is byte-identical to Map 1's, remove it or label it in the
   manifest as a copy of Map 1 — never as Map 2's world.
3. Move the root PNGs out of the root: delete them unless a doc references them (then move
   under docs/ai/ and fix the reference).
4. Replace TEST_READY.md's certificate with either a measured table (command + date + raw
   counts) or the line "planned, not measured".
5. Any test that asserted the fake hashes is now red: fix it to assert the truthful state
   (e.g. "evidence not captured"), not to accept anything.

DONE WHEN
  npm run test:frontend exits 0; npm run lint and node scripts/eslint_ratchet.mjs exit 0;
  `find artifacts/maps/map2 -name '*.png' -size -1k` prints nothing;
  `grep -rn MINIMAL_PNG scripts/` prints nothing.
```

### CARD G7

```text
CARD G7 — Restore loosened assertions and deleted tests
WAVE 2 · [RUST] · MAX FILES 12
DEPENDS ON  T2 merged
BRANCH      swarm/g7-restore-assertions
DECISION    QD-B (only if a fix is needed in src-tauri/src; otherwise none)
ALLOWED PATHS  src-tauri/tests/persistence_tests.rs, src-tauri/tests/persistence_stress_tests.rs,
               src-tauri/tests/tauri_ipc_tests.rs, src-tauri/tests/tick_capture_tests.rs,
               src-tauri/tests/predator_prey_combat_tests.rs, src-tauri/tests/anima_lab_cli_tests.rs,
               src-tauri/src/bin/anima-lab.rs (the manifest size cap only)

GOAL
Every assertion the 2026-10-22 landing weakened is exact again, and the four deleted
anima-lab tests are back — under TODAY's semantics, not by blindly restoring old numbers.

CONTEXT (from docs/planning/COMPLETION_ROADMAP_2026-10-16.md §4.1, clusters A and G, on main)
  exact count -> `>=` at persistence_tests.rs:137,185; persistence_stress_tests.rs:370;
  tauri_ipc_tests.rs:232; tick_capture_tests.rs:160.
  predator_prey_combat_tests.rs:230 compares a constant with itself.
  anima-lab: 4 tests deleted (two oversized-manifest tests, closed stdout, report fixture),
  and the manifest size cap raised from 8 MiB to 16 MiB.
Line numbers are from main @ a43872c; re-locate on the trunk.

STEPS
1. For each assertion, find the pre-landing version: `git show 1b3e7ef:<path>`.
2. Mortality and reproduction now exist, so the old exact value may be wrong today. For each
   site, derive the exact value the current semantics produce (it is deterministic: same seed,
   same schedule), assert THAT exactly, and write in a comment why the number is what it is.
   If you cannot derive it, or it differs between processes, that is a finding: stop and report.
3. The constant-vs-itself assertion must check a value computed by the code under test.
4. Restore the four anima-lab tests from git history. Restore the 8 MiB cap, or keep 16 MiB only
   with a written reason in the code and the report.

DONE WHEN
  cargo test --features desktop -j 2 --no-fail-fast  -> 0 failed (paste the summary lines)
  `git diff <BASE> -- src-tauri/tests | grep -nE '^\+.*>='` prints nothing
VERIFY EXTRA  For each restored exact assertion, the verifier changes the expected value by one in
              a scratch copy and confirms the test goes red.
```

### CARD G8

```text
CARD G8 — Move the IPC contract to docs/reference and rebuild it from the code
WAVE 2 · MAX FILES 6
DEPENDS ON  T2 merged; the owner has resolved the PROJECT.md conflict left by T2
BRANCH      swarm/g8-ipc-contract
DECISION    QD-6
AUTHORIZATION  R5-AUTH for G8 covering: docs/reference/IPC_CONTRACT.md, docs/reference/README.md,
               PROJECT.md (pointer section only), CLAUDE.md (the "Tauri IPC contract" section only)
ALLOWED PATHS  the four paths above, scripts/check_ipc_params.mjs (or the script behind
               `npm run check:ipc`), scripts/check_ipc_contract_doc.mjs, package.json

GOAL
One canonical IPC contract that cannot be overwritten silently and cannot drift from the code.

STEPS
1. Recover the last human-reviewed text: `git show 1b3e7ef:PROJECT.md`, section
   "Interface Contracts".
2. Rebuild the lists from the code, not from the old text: every #[tauri::command] and its
   registration in generate_handler!, every event name passed to emit, the parameter keys
   (`npm run check:ipc` already checks the case of parameter keys). Mark each entry with the
   file:line it came from.
3. Write docs/reference/IPC_CONTRACT.md. PROJECT.md and CLAUDE.md get a one-paragraph pointer.
4. Add a gate (`--check` mode) that fails when a registered command or event is missing from
   the doc, or the doc lists one the code does not register. Wire it into package.json.

DONE WHEN
  the new gate exits 0; removing one command from the doc makes it exit 1 naming it (paste);
  npm run check:ipc exits 0; node scripts/check_docs_links.mjs exits 0.
```

### CARD G9

```text
CARD G9 — Repository-root hygiene gate
WAVE 2 · MAX FILES 4
DEPENDS ON  T2 merged; G6 merged (it clears the root PNGs)
BRANCH      swarm/g9-root-hygiene
DECISION    none
ALLOWED PATHS  scripts/check_root_hygiene.mjs, scripts/root_allowlist.json, package.json,
               .github/workflows/ci.yml

GOAL
A file committed to the repository root that is not on a reviewed allow-list turns CI red.
History: 42 files of an unrelated Python exercise (with .pyc) and several agent scratch
files were committed to the root.

STEPS
1. Allow-list = the tracked root entries at your base (`git ls-tree --name-only HEAD`),
   minus anything the plan marks for removal. Each entry gets a one-line reason.
2. The script fails on a root entry not in the list AND on a list entry that no longer
   exists (both directions, like scripts/test_target_policy.mjs).
3. Wire into package.json and the frontend job of ci.yml.

DONE WHEN
  the gate exits 0; `touch scratch.md && git add scratch.md` then the gate exits 1 naming it
  (paste, then undo); CI on the draft PR runs the step.
```

### Đợt 3 — sau khi Đợt 2 đã merge

### CARD R0

```text
CARD R0 — Re-measure every gate at one trunk commit
WAVE 3 · [RUST] · MAX FILES 2
DEPENDS ON  every Wave 2 card merged or explicitly dropped by the owner
BASE        origin/feat/ecological-selection (record sha; every row must be at this sha)
BRANCH      swarm/r0-remeasure
DECISION    none
AUTHORIZATION  optional R5-AUTH for docs/planning/STATE_OF_THE_PROJECT.md; without it, write
               the proposed section to the second file below and the owner applies it
ALLOWED PATHS  docs/ai/testing/remeasure-<YYYY-MM-DD>-<sha>.md,
               docs/ai/testing/state-1g-proposed-<YYYY-MM-DD>.md

GOAL
One table in which every gate CI runs, plus the reproducibility gates, was run at the same
commit, by the commands in STATE §5, with raw output.

STEPS
1. Run every command of docs/planning/STATE_OF_THE_PROJECT.md §5 (and every step of
   .github/workflows/ci.yml that runs on your platform), sequentially, never in parallel.
2. Run live_experiment_tests in 6 separate processes; run the 180-tick checksum test
   (`a_fork_from_a_known_tick_shares_its_prefix_and_diverges_only_afterwards`, --nocapture)
   in 6 separate processes; report all values.
3. Counts: tests passed/failed/ignored per profile; targets; test files by directory.
4. Say which platform you ran on. Steps that only run on windows-latest are reported as
   "not run here — see CI run <id>" with the GitHub Actions conclusion.
5. Write the proposed STATE §1.g in the style of the existing §1.f (type column, command,
   result). Nothing in it may come from a run other than yours or the cited CI run.

DONE WHEN
  every row has a command, a date, the sha, and raw output; rows that failed are marked failed.
```

### CARD T3

```text
CARD T3 — Open the trunk-to-main pull request (do not merge)
WAVE 3 · MAX FILES 0
DEPENDS ON  R0 shows every gate green; the trunk's CI is green on GitHub
BASE        origin/feat/ecological-selection
BRANCH      none (the PR is from feat/ecological-selection to main)
DECISION    QD-F028 (real icons committed, so the PR does not break Linux/macOS builds)
ALLOWED PATHS  none in the repository; one GitHub pull request

STEPS
1. Check for a PR template (.github/pull_request_template.md and the other standard
   locations) and mirror its headings.
2. Body: what the trunk brings to main (by area, with commit ranges), the R0 report path,
   the CI run URL, the list of owner decisions it embodies, and "Effect on published
   evidence: E2 is frozen at 9c57184 and runs with restore: None" (cite RESULT.md).
3. Open it as a draft. Do not merge, approve, or request reviewers.

DONE WHEN
  the PR exists as a draft and its CI has started; paste the URL.
```

### CARD S1

```text
CARD S1 — HT2.1: a population that turns over inside the live experiment adapter
WAVE 3 · [RUST] · MAX FILES 15
DEPENDS ON  R0
BASE        origin/feat/ecological-selection
BRANCH      swarm/s1-population-turnover
DECISION    QD-B, QD-4, QD-5, QD-A'
ALLOWED PATHS  src-tauri/src/core/live_experiment.rs, src-tauri/src/core/mortality.rs,
               src-tauri/src/core/spawn.rs, src-tauri/src/core/simulation_schedule.rs,
               src-tauri/src/core/agent_systems.rs, src-tauri/tests/live_experiment_tests.rs,
               src-tauri/tests/live_turnover_tests.rs (new)

REQUIRED READING (in order): CLAUDE.md sections "creature spawn and morphology" and
"alternate world laws"; docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md;
docs/decisions/ADR-0001; docs/decisions/ADR-0003; docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md;
artifacts/experiments/e2-evolved-brain-default/RESULT.md §4 and §11.

WHY
E2 was null because the population froze: no starvation death, and the adapter ran no
evolutionary replacement (finding E2-F3), so ten bodies sat at zero energy for thousands
of ticks. An experiment without turnover measures nothing about selection.

STEP 0 — MEASURE BEFORE WRITING CODE
The trunk already has manifest keys such as live.mortality, live.ecological_fitness and
live.founders (the E3 draft lists them as constants). Run a smoke manifest with mortality
and reproduction enabled for 6,000 ticks and report the live.agent_count series. If the
population already turns over, this card becomes: prove it with tests (steps 3–5) and
write no engine code. Report STEP 0 before going further.

STEPS
1. If turnover is missing, implement the mechanism QD-4 names, inside the tick,
   deterministically, without reading the evolution thread's channel.
2. Development happens once at birth; spawning consumes a phenotype (CLAUDE.md hard rule).
   No Lamarck: learned brain state never flows into a genotype.
3. Tests: (a) with turnover OFF, the world checksum is bit-identical to BASE;
   (b) with turnover ON, live.agent_count changes within one run and the population's
   composition changes; (c) the checksum with turnover ON agrees across 6 separate processes;
   (d) a fork from a tick AFTER the first death and birth matches the uninterrupted run, and
   so does save at that tick → load → run the rest; (e) closed EU residual with turnover ON.
4. R8: report checksums before/after; state the effect on E2 (frozen at 9c57184).
5. If any default-path trajectory changed, bump LIVE_MODEL_VERSION and say why.

DONE WHEN
  cargo test --features desktop -j 2 --no-fail-fast -> 0 failed; the 5 tests above exist
  and pass; tests/schedule_ambiguity_tests.rs passes.
```

### CARD S2

```text
CARD S2 — Observables needed by E2R and E3
WAVE 3 · [RUST] · MAX FILES 8
DEPENDS ON  S1 merged (both edit live_experiment.rs)
BRANCH      swarm/s2-observables
DECISION    QD-B, QD-EN
ALLOWED PATHS  src-tauri/src/core/live_experiment.rs, src-tauri/src/core/observables*.rs,
               src-tauri/tests/observables_tests.rs, src-tauri/tests/live_experiment_tests.rs,
               src/types/generated/** (regenerated only)

GOAL
Add the observables two preregistrations will need, without changing any trajectory.

NEEDED
1. An energy metric that integrates over the run instead of sampling its end — e.g. the
   cumulative sum of mean agent energy per tick (RESULT.md §11 item 1). Cumulative ⇒
   Aggregation::Instant (CLAUDE.md: evolution.births is Instant for the same reason).
2. Region-split body-mass observables for E3 (west half / east half of the 256×256 grid,
   matching the E3 draft's Rect{0,0,127,255} and Rect{128,0,255,255}).
3. Every observable is finite on every tick, including when zero agents are alive (drive()
   stops a run on a non-finite value). Choose and document the empty-population value; do
   not use a sentinel that would enter a paired delta as data.
Time-to-floor stays a DERIVED quantity computed from the series by the analysis script,
unless you can define it as a finite per-tick value; say which you chose and why.

RULES
- Reading must not perturb: a test runs the same seed with observables sampled every tick
  and never, and asserts identical world checksums.
- The registry is append-only: existing ids keep their meaning and units.

DONE WHEN
  the non-perturbation test and the registry tests pass; cargo test --features desktop -j 2
  -> 0 failed; the ts-rs parity gate is clean (`git diff --exit-code -- src/types/generated`
  after cargo test).
```

### CARD S3

```text
CARD S3 — HT2.3 / G2 gate #1: one source for the ecological constants
WAVE 3 · [RUST] · MAX FILES 10
DEPENDS ON  R0
BRANCH      swarm/s3-laws-single-source
DECISION    QD-B
ALLOWED PATHS  src-tauri/crates/anima-domain/src/laws.rs (or its real path: locate first),
               src-tauri/crates/anima-domain/src/**, src-tauri/src/core/ecology.rs,
               src-tauri/src/core/reference_world.rs, src-tauri/tests/law_convergence_tests.rs (new)

GOAL
A change to one ecological law in the domain crate changes the behaviour of BOTH engines
(the live Bevy world and ReferenceEvolutionWorld) in the same direction. Today they can
hold different values for the same law.

CONTEXT
docs/planning/COMPLETION_ROADMAP_2026-10-16.md, HT2.3 and cluster F: laws.rs carries MTE,
Holling, Lindeman 0.40 and reproduction efficiency 0.60, while the live sim reads
ecology::metabolic_rate and its own LINDEMAN_EFFICIENCY; a comment next to the constant says
~10 % and PROJECT.md once said ~30 %.

RULES
- Baseline first: every value keeps what the LIVE engine uses today, so every existing
  checksum stays identical. Where the two engines disagree today, do NOT pick a winner:
  record it as a finding (documentation policy rule 6) and keep each engine's current value
  behind the single source until the owner decides.
- Prefer laws carried in the run's law set (immutable within a run, per ADR-0002) over a
  global const, so a test can build two law sets that differ in one field.

DONE WHEN
  a test builds two law sets differing in one constant and shows both engines move in the
  documented direction; all existing checksum tests unchanged; cargo test --features desktop
  -j 2 -> 0 failed.
```

### CARD S4

```text
CARD S4 — OSS-072 MRCA: rebase PR #31 onto the trunk
WAVE 3 · [RUST] · MAX FILES 15
DEPENDS ON  R0
BRANCH      swarm/s4-mrca
DECISION    QD-B
ALLOWED PATHS  the files PR #31 changes (`git diff --name-only origin/main...origin/feat/oss-072-mrca`),
               plus docs/reference/IPC_CONTRACT.md if G8 has landed and R5-AUTH covers it
TASK
Cherry-pick the commits of origin/feat/oss-072-mrca (tip b14e836) onto your base. Resolve the
known conflicts in TODO.md and src-tauri/src/commands/evolution.rs by keeping both sides'
intent. Any new IPC command goes into the IPC contract (G8) — if G8 has not landed, list it in
the report instead. The MRCA of a DAG is a SET, not a node; keep the tests that say so.
DONE WHEN
  PR #31's tests pass on your branch; cargo test --features desktop -j 2 -> 0 failed.
```

### CARD M1

```text
CARD M1 — Cross-platform reproducibility matrix
WAVE 3 · [RUST] · MAX FILES 4
DEPENDS ON  R0; QD-F028 = yes and icons committed (otherwise Linux/macOS cannot compile)
BRANCH      swarm/m1-determinism-matrix
DECISION    QD-F028
ALLOWED PATHS  .github/workflows/determinism-matrix.yml (new; do NOT edit ci.yml),
               src-tauri/tests/cross_platform_checksum_probe.rs (new),
               scripts/paper/compare_checksums.mjs (new), docs/paper/data/determinism_matrix.md

GOAL
Measure whether the same seed and manifest give the same world checksum on Windows, Linux
and macOS. Today's evidence covers separate processes on ONE machine only.

STEPS
1. The probe test prints one JSON line per (seed, tick) for a fixed list of seeds and ticks
   (include 180, 1800 and 18000), plus the observable values, for both the live adapter and
   ReferenceEvolutionWorld.
2. The workflow runs it on windows-latest, ubuntu-latest and macos-latest (workflow_dispatch
   and pull_request), uploads each output as an artifact.
3. The compare script builds the matrix and names the first (seed, tick) that differs.
4. Report the result AS MEASURED. If platforms disagree, that is a finding about floating
   point across libm implementations — record it; do not round, quantize, or change
   arithmetic in this card.

DONE WHEN
  the workflow ran on the draft PR on all three OSes (paste the run URL and the matrix).
```

### CARD M2

```text
CARD M2 — Negative controls for every gate (a table for the paper)
WAVE 3 · [RUST] · MAX FILES 4
DEPENDS ON  R0
BRANCH      swarm/m2-gate-controls
DECISION    QD-P2
ALLOWED PATHS  scripts/paper/gate_controls.mjs, scripts/paper/gate_faults/** (patch files as data),
               docs/paper/data/gate_controls.md

GOAL
For each gate, show that injecting the fault it exists to catch turns it red AND that its
message names the fault. A gate that stays green under its own fault is vacuous — this
project has already had several.

GATES (extend if you find more in ci.yml): check_test_targets (empty target; unlisted
#[ignore]), feature separation via cargo tree, ts-rs binding parity, thread_rng source scan,
ESLint ratchet, conflict-marker scan, schedule ambiguity (SimRng and energy pairs),
zero-alloc, closed-EU tests, live_experiment fork and save/load, check:csp, check:bundle,
docs links, check:ipc, check:e2 digests, E2E spec list (G2), root hygiene (G9).

STEPS
1. Each fault is a minimal patch stored as a file under scripts/paper/gate_faults/.
2. The runner applies one patch in a throwaway worktree, runs the gate, records exit code
   and the line that names the fault, reverts. Faults are NEVER committed to source files.
3. Table: gate · fault · exit code · naming line (verbatim) · verdict (CATCHES / VACUOUS).

DONE WHEN
  every gate has a row; any VACUOUS row is listed first in the report as a finding (do not
  fix it in this card).
```

### CARD M3

```text
CARD M3 — Headless scaling curve
WAVE 3 · [RUST] · MAX FILES 4
DEPENDS ON  R0
BRANCH      swarm/m3-scaling
DECISION    QD-P2
ALLOWED PATHS  src-tauri/benches/live_schedule_scaling.rs (new), src-tauri/Cargo.toml ([[bench]]
               entry only), docs/paper/data/scaling.md

GOAL
An honest statement of how tick cost grows with agent count on the real live schedule.

STEPS
1. Build the world with core::simulation_schedule::build_tick_schedule (never a hand-made
   .chain() subset). Agent counts: 10, 100, 1000, and 5000 if it completes.
2. Release profile, warm-up, then p50/p95 per tick (Criterion medians from estimates.json,
   not the slope estimate — STATE §3.0 explains the difference).
3. Record CPU model, core count and OS from commands. This is a headless lower bound; it is
   NOT app frame time (no renderer, no IPC, no Tauri) and the doc must say so.

DONE WHEN
  cargo bench --bench live_schedule_scaling runs; the doc has the table, the machine spec and
  the commands.
```

### CARD M4

```text
CARD M4 — Closed-EU residual matrix
WAVE 3 · [RUST] · MAX FILES 2
DEPENDS ON  G5 merged
BRANCH      swarm/m4-eu-matrix
DECISION    QD-B (only for a fix; this card should not need one)
ALLOWED PATHS  src-tauri/tests/eu_residual_matrix_tests.rs (new), docs/paper/data/eu_residual.md

GOAL
Show that the closed energy ledger holds in every combination of policy flags and every
intervention kind, not just the default path.

STEPS
1. Enumerate the flags that change energy flow (evolved brains, lifetime learning, mortality,
   ecological fitness, turnover, brain_metabolic_cost > 0, …) and every InterventionKind in the
   domain crate. Locate them; list them with file:line.
2. For each combination (full factorial if under ~200 runs, otherwise a covering design you
   describe), run a fixed number of ticks and record the closed_eu_total residual.
3. Tolerance: state it before running and justify it; E2 measured −2.6e-9 EU on 1.5e5 EU.
4. If a combination leaks, STOP: commit the failing test, mark the PR "finding — not ready",
   and report. Do not widen the tolerance and do not fix it here.

DONE WHEN
  the test passes for every combination, or the report names the leaking combination with the
  failing test committed.
```

### Đợt 4 — thí nghiệm

### CARD E2R-P

```text
CARD E2R-P — Preregister E2R (the brain question, redone with power)
WAVE 4 · [RUST] · MAX FILES 20
DEPENDS ON  S1 and S2 merged
BRANCH      swarm/e2r-prereg
DECISION    QD-EN, QD-4
ALLOWED PATHS  src-tauri/tests/fixtures/experiments_e2r/**, src-tauri/tests/prereg_e2r_manifest_tests.rs,
               src-tauri/examples/run_e2r_brain_experiment.rs (or a generalized runner),
               artifacts/experiments/e2r-*/smoke/**,
               docs/ai/{requirements,design,planning,testing}/<date>-experiment-e2r-*.md

MODEL ON E2 — read all of these first:
  src-tauri/tests/fixtures/experiments_e2/, src-tauri/tests/prereg_e2_manifest_tests.rs,
  src-tauri/examples/run_e2_brain_experiment.rs, scripts/verify_e2_artifacts.mjs,
  docs/ai/*/2026-07-27-experiment-e2-evolved-brain-default.md,
  artifacts/experiments/e2-evolved-brain-default/RESULT.md (especially §4, §6, §10, §11).

WHAT CHANGES FROM E2 (from RESULT.md §11)
1. A population that turns over (S1's mechanism, enabled in both arms).
2. An endpoint BEFORE the floor, chosen from the smoke run only, or the integrated metric
   from S2 as primary. Decide which and write why.
3. Time-to-floor as a registered (derived) observable, with its sampling period stated.
SAME AS E2: question, arms (evolved = true vs default shared BrainModel), materiality rule
(all three: CI excludes 0, |d_z| >= 0.8, |delta| >= 5 % of control mean), N = 12 seeds,
minimum 10 complete pairs, decision rule for ADR-0003 (flip the default only on a material
positive effect).

SEEDS
A new arithmetic sequence that overlaps none of: E2 700001…711001, E3 draft 730001…741001,
smoke seeds 999983 and 999979. Proposal: 750001 + 1000·k, k = 0..11; a new smoke seed. The
smoke seed must be absent from both experimental manifests so the runner refuses it.

RULES
- Run ONLY the smoke seed. No experimental seed may run in this card. Mechanically enforce
  it as E2 did (execution lock / manifest seed lists).
- The prereg test asserts that the docs, the JSON fixture and the manifests agree.
- Status of the docs: "awaiting owner approval". Commit the preregistration BEFORE any
  experimental run exists.

DONE WHEN
  cargo test --features desktop --test prereg_e2r_manifest_tests -> all pass;
  the smoke summary shows the chosen endpoint is before the floor in both arms (paste it).
```

### CARD E2R-X

```text
CARD E2R-X — Run E2R exactly once
WAVE 4 · [RUST] · MAX FILES 30
DEPENDS ON  E2R-P merged; the owner wrote "E2R preregistration approved at <sha>"
BRANCH      swarm/e2r-run
DECISION    owner approval line (paste it here):
ALLOWED PATHS  artifacts/experiments/e2r-*/**, scripts/verify_e2r_artifacts.mjs, package.json
               (one script entry: check:e2r), docs/decisions/ADR-0003-*.md ONLY with R5-AUTH

PRECONDITIONS (check and paste)
  git merge-base --is-ancestor <approved prereg sha> HEAD   -> exit 0
  the ensemble output directory does not exist yet
STEPS
1. cargo build --release --features desktop --example <runner>; record the binary SHA-256.
2. Run the ensemble once. Never `cargo run`.
3. RESULT.md with E2's structure: the question as registered; audit disclosure of anything
   run before the preregistration; headline; a POWER CHECK section (was the endpoint before
   the floor in both arms, per seed?); every observable with the same statistics; an
   "unregistered observations" section kept separate; defects found; reproduction; provenance
   and checksums; inference boundary (copy E2 §10's "not permitted" list and extend it);
   decision by the registered rule.
4. scripts/verify_e2r_artifacts.mjs recomputes every digest with node:crypto.
5. If the harness breaks mid-run: stop, report, do not rerun. A rerun needs the owner, and
   the broken run is kept under superseded/ as E2 did.

DONE WHEN
  npm run check:e2r exits 0; RESULT.md exists; every number in it traces to a file in the
  artifact directory.
```

### CARD E3-P

```text
CARD E3-P — Turn the E3 draft into a preregistration (WP-1 to WP-4)
WAVE 4 · [RUST] · MAX FILES 20
DEPENDS ON  S2 merged
BRANCH      swarm/e3-prereg
DECISION    QD-EN
ALLOWED PATHS  src-tauri/tests/fixtures/experiments_e3/**, src-tauri/tests/prereg_e3_manifest_tests.rs,
               artifacts/experiments/e3-*/smoke/**,
               docs/ai/planning/2026-08-26-experiment-e3-productivity-gradient-body-mass.md
TASK
Follow §9 of docs/ai/planning/2026-08-26-experiment-e3-productivity-gradient-body-mass.md
(on the trunk) in order: WP-1 fixture + matching test; WP-2 smoke at seed 999979 proving (a)
ResourceField is present headless, (b) the two interventions really change
live.standing_crop between arms, (c) detritus > 0 before tick 3000; WP-3 fix T and warm-up
from WP-2 and record them; WP-4 freeze the doc (status: frozen, parent commit).
WP-2 HAS THE POWER TO KILL THE DESIGN: if (b) fails, stop and report — do not tune the
interventions until it passes.
If S2 added region-split observables, register them as secondary here, before any data.
DONE WHEN
  the prereg test passes; the WP-2 smoke report shows (a), (b), (c) with numbers; no
  experimental seed has run.
```

### CARD E3-X

```text
CARD E3-X — Run E3 exactly once (WP-5)
WAVE 4 · [RUST] · MAX FILES 30
DEPENDS ON  E3-P merged; owner wrote "E3 preregistration approved at <sha>"
BRANCH      swarm/e3-run
DECISION    owner approval line (paste it here):
ALLOWED PATHS  artifacts/experiments/e3-*/**, scripts/verify_e3_artifacts.mjs, package.json
               (one script entry: check:e3)
TASK
Same procedure and RESULT.md structure as CARD E2R-X, with E3's registered metrics (§6 of the
draft), materiality rule (§7), and failure handling (§8). Report the limitation the draft
states before the data: global observables measure DISPERSION, not spatial differentiation,
unless region-split observables were registered in E3-P.
DONE WHEN
  npm run check:e3 exits 0; RESULT.md exists; every number traces to an artifact file.
```

### CARD S5

```text
CARD S5 — OSS-073: Avida-style line of descent
WAVE 4 · [RUST] · MAX FILES 10
DEPENDS ON  S4 merged
BRANCH      swarm/s5-line-of-descent
DECISION    QD-B
ALLOWED PATHS  src-tauri/src/evolution/lineage*.rs, src-tauri/src/evolution/**descent**,
               src-tauri/tests/line_of_descent_tests.rs (new)
TASK
Using the MRCA from S4, extract the line of descent of the final dominant lineage and detect
one evolutionary event on it (e.g. the first ancestor whose trait value crosses a declared
threshold), deterministically. Avida is copyleft: work from its papers only, never its source
(docs/research/OPEN_SOURCE_LANDSCAPE.md). STATE §3.15.1 holds the original item.
DONE WHEN
  a deterministic test finds the event at the same tick in 6 separate processes;
  cargo test --features desktop -j 2 -> 0 failed.
```

### Đợt 5 — phát hành và bài báo

### CARD R1

```text
CARD R1 — Licence path (A): ship desktop without ml-wgpu
WAVE 5 · [RUST] · MAX FILES 8
DEPENDS ON  QD-C = A
BRANCH      swarm/r1-drop-ml-wgpu
DECISION    QD-C
AUTHORIZATION  R5-AUTH for R1 if CLAUDE.md/README.md text is to change; otherwise write the
               proposed text into the ADR draft and the owner applies it
ALLOWED PATHS  src-tauri/Cargo.toml ([features] only), .github/workflows/ci.yml (feature checks
               only), docs/ai/design/<date>-adr-draft-ship-desktop-without-ml-wgpu.md

CONTEXT
hexf-parse 0.2.1 reaches the binary only via naga -> wgpu -> burn-wgpu -> ml-wgpu -> desktop
(roadmap 2026-10-16 HT5.1). The GPU learner is already off by default at runtime and leaks
in wgpu's registry (PR #30).

STEPS
1. Remove ml-wgpu from the desktop feature; keep ml-wgpu as a non-distributed research feature.
2. Prove hexf-parse is gone with the FORWARD check (never `cargo tree -i`, which exits non-zero
   on ambiguous specs and prints nothing — F-025):
     cargo tree -e all --features desktop --target all | grep -c " hexf-parse v"   -> 0
3. The CPU learner fallback still runs: cargo test --features desktop -j 2 -> 0 failed.
4. BLOCK 0 R12: do NOT regenerate NOTICE, licensing/third-party-index.json or sbom.cdx.json on
   macOS or Linux. They will be stale; say so. The owner regenerates them on Windows (plan H5).
   Keep this PR a draft until that commit is added.
5. Draft the ADR (docs/ai/design/…): context, decision, what is lost (GPU learner in shipped
   builds), rollback.

DONE WHEN
  the forward check prints 0; cargo test passes; the ADR draft exists; the report lists every
  licensing gate that is stale pending H5.
```

### CARD R2

```text
CARD R2 — CHANGELOG, install guide, release notes (drafts)
WAVE 5 · MAX FILES 4
DEPENDS ON  QD-1
BRANCH      swarm/r2-release-docs
DECISION    QD-1
ALLOWED PATHS  CHANGELOG.md (only with a root allow-list entry from G9 in the same PR),
               scripts/root_allowlist.json, docs/how-to/INSTALL.md, docs/ai/deployment/<date>-release-notes-draft.md
RULES
- Every CHANGELOG line cites a commit sha that `git cat-file -t` resolves. Group by
  conventional-commit type. No claim beyond the commit subject.
- "Known limitations" points to STATE §1.d and the latest RESULT.md files; it does not copy
  numbers (documentation policy rule 8).
- INSTALL.md: commands you could not run on this platform (Windows, WebView2, MSVC) are marked
  "not verified on <platform> — owner verifies in H8".
DONE WHEN
  a script or command you include re-checks every sha in CHANGELOG.md (paste its output);
  node scripts/check_docs_links.mjs exits 0.
```

### CARD P4

```text
CARD P4 — Claims ledger for the paper
WAVE 5 · MAX FILES 3
DEPENDS ON  P1, M1, M2, M3, M4, E2R-X merged (E3-X optional)
BRANCH      swarm/p4-claims-ledger
DECISION    QD-P1, QD-P2
ALLOWED PATHS  docs/paper/claims.csv, scripts/paper/check_claims.mjs, docs/paper/outline.md

GOAL
Every number the paper may state lives in one row that names where it comes from, and a
script re-reads the source and fails if the number moved.

COLUMNS
  claim_id (C-001…), draft_sentence, value, unit, source_file, source_locator (JSON path, CSV
  row/column, or file:line), reproduce_command, commit, measured_date,
  kind (measured-current | measured-historical | design-parameter | target), status
RULES
- Historical numbers (E2 at 9c57184, the 24-process acceptance) stay labelled historical.
- A sentence with words like "in general", "across worlds", "experiment-ready", "proves" is
  rejected unless a source states exactly that; RESULT.md §10 and STATE §1.d list the forbidden
  upgrades.
- outline.md: the paper's sections, each listing the claim ids it will use.
DONE WHEN
  node scripts/paper/check_claims.mjs exits 0; changing one value in claims.csv makes it exit 1
  naming the row (paste, then undo).
```

### CARD P5

```text
CARD P5 — Figures and tables from data
WAVE 5 · MAX FILES 10
DEPENDS ON  P4
BRANCH      swarm/p5-figures
DECISION    QD-P2
ALLOWED PATHS  scripts/paper/*.mjs (new figure scripts only), docs/paper/figures/**
TASK
Same rules as CARD P3, for: E2R results (and E3 if run), the gate negative-control table (M2),
the incident timeline (P1), the determinism matrix (M1), the scaling curve (M3), the EU
residual table (M4). Each script has a --check mode that regenerates byte-identical output and
asserts its key numbers equal the claims.csv rows it uses.
DONE WHEN
  every figure script's --check exits 0.
```

### CARD P6

```text
CARD P6 — Reproduction package
WAVE 5 · MAX FILES 3
DEPENDS ON  E2R-X merged
BRANCH      swarm/p6-reproduce
DECISION    QD-P2
ALLOWED PATHS  docs/paper/REPRODUCE.md, scripts/paper/reproduce_all.mjs
TASK
One command per result (build the runner, run, verify digests: check:e2, check:e2r, check:e3),
the expected wall clock from the RESULT.md files (cited, not re-estimated), the hardware each
was measured on, and what a reviewer needs installed. Run every command yourself on a fresh
clone in a new worktree and paste the outputs; a command you could not run is marked so.
DONE WHEN
  reproduce_all.mjs --verify-only (digests only, no rerun) exits 0 on a fresh clone.
```

### CARD P7

```text
CARD P7 — Draft the paper sections (English)
WAVE 5 · MAX FILES 10
DEPENDS ON  P4, P5, P6, P2
BRANCH      swarm/p7-draft
DECISION    QD-P1, QD-P2, QD-P3
ALLOWED PATHS  docs/paper/draft/*.md
RULES
- Every sentence with a number ends with its claim id, e.g. "[C-014]". No id, no number.
- Every citation uses a key that exists in references.bib (verified in P2).
- Limitations section quotes the inference boundaries of each RESULT.md verbatim.
- This is a draft for the owner to rewrite. Do not polish prose at the expense of precision.
- Sections: introduction, system, evidence discipline (gates + negative controls), case studies
  (incidents; E2 null → diagnosis → E2R), results (determinism matrix, scaling, EU residual),
  limitations, availability, AI-use statement placeholder for the owner (QD-P3).
DONE WHEN
  a script lists every number in the draft without a claim id: output empty (paste);
  node scripts/paper/check_claims.mjs exits 0.
```

### CARD P8

```text
CARD P8 — Adversarial audit of the draft (run on a DIFFERENT model, e.g. Claude)
WAVE 5 · MAX FILES 1
DEPENDS ON  P7
BRANCH      swarm/p8-audit
DECISION    none
ALLOWED PATHS  docs/paper/audit/<YYYY-MM-DD>.md
ROLE
You did not write this paper. Find every sentence the evidence does not support.
STEPS
1. For each sentence in docs/paper/draft/*.md: find its claim id in claims.csv; open the
   source file at the locator; does the source say exactly this? Flag any upgrade
   (documentation policy rule 10: "instrumentation shipped" is not "measured";
   "headless adapter verified" is not "experiment-ready"; one world design is not "worlds").
2. For each citation: open the source (DOI/URL) and check the quoted support.
3. Do not edit the draft. Output: sentence · problem · evidence (path:line or URL) · severity.
DONE WHEN
  every sentence of the draft was checked (state the count).
```

### Đợt 6 — hoàn thiện sản phẩm tới v1.0

Mọi thẻ Đợt 6 dùng chung phần đọc bắt buộc và luật sau:

```text
COMMON TO WAVE 6
REQUIRED READING: CLAUDE.md (both "Required reading" sections and their hard rules);
docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md; docs/decisions/ADR-0001, ADR-0003;
docs/explanation/CREATURE_MORPHOGENESIS.md; docs/planning/CREATURE_MORPHOGENESIS_PLAN.md;
docs/reference/ENERGY_LEDGER_CONTRACT.md; docs/reference/DETERMINISM_CONTRACT.md;
WORLD_SIMULATION_PLAN.md §7 and §9; SIMULATION_RULES.md.
SCOPE: only what the "watershed – grassland – rabbit – wolf" slice needs, never the whole
milestone.
INVARIANTS each card must keep and prove with tests: closed EU bit-exact (or the stated
tolerance); deterministic checksum across 6 processes; zero heap allocation in tick systems;
save/load and fork trajectory-exact; legacy/default path bit-identical when the new feature
is off.
R8: each card that changes simulation semantics reports checksums before and after and the
effect on published experiment evidence.
BASE origin/feat/ecological-selection · PR TARGET feat/ecological-selection · DECISION QD-B
```

### CARD V0

```text
CARD V0 — Creature Morphogenesis P1–P2: develop at birth, phenotype in the save
WAVE 6 · [RUST] · MAX FILES 15 · DEPENDS ON R0, QD-5
BRANCH swarm/v0-develop-at-birth
ALLOWED PATHS src-tauri/src/evolution/genotype*.rs, src-tauri/src/evolution/phenotype*.rs,
  src-tauri/src/core/spawn.rs, src-tauri/src/core/snapshot*.rs, src-tauri/tests/morphogenesis_*.rs
TASK  Implement phases P1–P2 of docs/planning/CREATURE_MORPHOGENESIS_PLAN.md: development
  happens once at genesis/birth (develop_at_birth); ECS spawning consumes a DevelopedPhenotype;
  restore and migration carry the serialized phenotype and never re-develop; decode_genotype
  never reads the environment. Snapshot schema version bumps with a migration test.
DONE WHEN the plan's P1–P2 gates exist and pass; old saves load (migration test);
  cargo test --features desktop -j 2 -> 0 failed.
```

### CARD V1

```text
CARD V1 — DynamicFields into the live schedule
WAVE 6 · [RUST] · MAX FILES 12 · DEPENDS ON R0
BRANCH swarm/v1-dynamic-fields-live
ALLOWED PATHS src-tauri/src/core/simulation_schedule.rs, src-tauri/src/core/dynamic_fields*.rs,
  src-tauri/src/core/world_systems.rs, src-tauri/src/core/snapshot*.rs, src-tauri/tests/dynamic_fields_live_tests.rs
TASK  The climate/water/soil fields (S15–S20) run headless today and are absent from
  build_tick_schedule (there is no plant_soil_weather phase in core/tick_capture.rs). Insert them
  into the ONE schedule builder, behind a flag that defaults off, with save/load of their state,
  water and nutrient budgets conserved, zero-alloc.
DONE WHEN with the flag off, every checksum unchanged; with it on: budget residuals per pool,
  fork/save-load exact, 6-process checksum agreement; cargo test -> 0 failed.
```

### CARD V2

```text
CARD V2 — SpeciesProfile data for rabbit and wolf
WAVE 6 · [RUST] · MAX FILES 10 · DEPENDS ON V0
BRANCH swarm/v2-species-profile
ALLOWED PATHS src-tauri/src/core/species*.rs (new or existing), src-tauri/src/core/ecology.rs,
  src-tauri/src/core/agent_systems.rs, src-tauri/tests/species_profile_tests.rs
TASK  Diet, habitat and reproduction rate become data in a SpeciesProfile; diet queries stop
  hard-coding two guilds (M5.1, M6.1). Values carry units and a source or a "design parameter"
  label (documentation policy rule 9).
DONE WHEN no diet branch keyed on a hard-coded guild remains (grep, pasted); default profiles
  reproduce today's checksums; cargo test -> 0 failed.
```

### CARD V3

```text
CARD V3 — Age, life stages, death, costly reproduction, maturation
WAVE 6 · [RUST] · MAX FILES 15 · DEPENDS ON V0, V2, and an ADR for QD-5 accepted by the owner
BRANCH swarm/v3-life-cycle
ALLOWED PATHS src-tauri/src/core/mortality.rs, src-tauri/src/core/spawn.rs,
  src-tauri/src/core/life_cycle*.rs (new), src-tauri/src/core/simulation_schedule.rs,
  src-tauri/tests/life_cycle_tests.rs
TASK  As the accepted ADR specifies (M5.2–M5.3). Births go through development (V0).
  Reproduction cost goes into total_cost in metabolic_decay_system, never a separate deduction.
DONE WHEN a census shows births − deaths = Δpopulation every tick (closed); a resource-limited
  population plateaus in a test; all common invariants hold.
```

### CARD V4

```text
CARD V4 — Carcasses and decomposition
WAVE 6 · [RUST] · MAX FILES 10 · DEPENDS ON V3
BRANCH swarm/v4-decomposition
ALLOWED PATHS src-tauri/src/core/decomposition*.rs (new), src-tauri/src/core/mortality.rs,
  src-tauri/src/core/ecology.rs, src-tauri/tests/decomposition_tests.rs
TASK  A dead body becomes a carcass cohort that returns biomass to the right pools over time
  (M5.6, M6.3), through EnergyLedger transfers only.
DONE WHEN EU bit-exact across a death → decomposition cycle; carcass state survives save/load.
```

### CARD V5

```text
CARD V5 — Soil water, nutrients and grass biomass from the fields
WAVE 6 · [RUST] · MAX FILES 12 · DEPENDS ON V1
BRANCH swarm/v5-soil-grass
ALLOWED PATHS src-tauri/src/core/dynamic_fields*.rs, src-tauri/src/core/ecology.rs,
  src-tauri/src/core/resource_field*.rs, src-tauri/tests/soil_grass_tests.rs
TASK  Grass growth reads soil water and nutrients from the authoritative fields; large trees'
  canopy/roots feed back (M3.2–M3.3, M4.2, M4.4).
DONE WHEN water and nutrient budgets neither create nor destroy (residual per pool, pasted);
  a drought scenario lowers grass biomass in the documented direction.
```

### CARD V6

```text
CARD V6 — The four slice interventions as InterventionCommand with CauseId
WAVE 6 · [RUST] · MAX FILES 12 · DEPENDS ON V4, V5
BRANCH swarm/v6-slice-interventions
ALLOWED PATHS src-tauri/crates/anima-domain/src/intervention.rs, src-tauri/src/core/live_experiment.rs,
  src-tauri/src/core/environmental_systems.rs, src-tauri/tests/slice_interventions_tests.rs
TASK  30-day drought; remove 80 % of wolves; clear 40 % of headwater forest; nutrient loading
  around the lake (M8.1, M8.5). Each goes through the queue with a CauseId and the ledger; none
  edits a population count directly.
DONE WHEN each fires at its declared tick (test); EU residual within tolerance for each; the
  causal ledger shows the root cause for each effect.
```

### CARD V7

```text
CARD V7 — Causal timeline and control-vs-treatment view (real data only)
WAVE 6 · MAX FILES 12 · DEPENDS ON V6, G4
BRANCH swarm/v7-causal-ui
ALLOWED PATHS src/components/ResearchDashboard/**, src/components/Causal*/**, tests/frontend/**causal**
TASK  Show the causal ledger as a timeline; clicking an effect shows its root cause; a
  control-vs-treatment chart from real exported runs (M9.2, M9.4). BLOCK 0 R3: no synthetic
  curve, no mock fallback on a data path; when data is missing the UI says so.
DONE WHEN tests render real fixture exports from a headless run (committed with their
  generating command); npm run test:frontend, lint and ratchet exit 0.
```

### CARD V8

```text
CARD V8 — Slice acceptance: four scenarios, multi-seed, seven observations
WAVE 6 · [RUST] · MAX FILES 20 · DEPENDS ON V6
BRANCH swarm/v8-slice-acceptance
ALLOWED PATHS src-tauri/tests/fixtures/slice/**, src-tauri/tests/slice_acceptance_tests.rs,
  artifacts/slice/**, docs/ai/testing/<date>-slice-acceptance.md
TASK  Scenario files for the four interventions; run each multi-seed against a control; report
  the seven observations of WORLD_SIMULATION_PLAN §9 (predator–prey cycles, trophic cascade,
  water/soil/tree feedback, simple eutrophication, causal timeline, control vs treatment,
  deterministic save/replay). Each observation has a pass criterion written BEFORE running.
DONE WHEN the report states each criterion and its measured outcome, pass or fail, with the
  artifact path; failures are reported, not tuned away.
```

### CARD V9

```text
CARD V9 — Parameter registry and sensitivity (M11.1–M11.3)
WAVE 6 · [RUST] · MAX FILES 12 · DEPENDS ON V8
BRANCH swarm/v9-parameter-registry
ALLOWED PATHS src-tauri/src/core/parameters*.rs (new), src-tauri/src/core/sweeps.rs,
  src-tauri/tests/parameter_registry_tests.rs, docs/ai/testing/<date>-sensitivity.md
TASK  Every constant that governs the slice is registered with unit, default, plausible range
  and source/label. Run a one-at-a-time and a Sobol/LHS sweep (the sweep module exists; its
  HashMap serialization was a known defect — use BTreeMap) over the slice's observables.
DONE WHEN registry test lists every constant read by the slice systems (grep-driven);
  sensitivity table with commands and seeds.
```

### CARD V10

```text
CARD V10 — Soak test (M11.4)
WAVE 6 · [RUST] · MAX FILES 4 · DEPENDS ON V8
BRANCH swarm/v10-soak
ALLOWED PATHS src-tauri/tests/soak_tests.rs (marked so the default suite stays fast; follow
  scripts/test_target_policy.mjs for any #[ignore] — it needs a listed reason), .github/workflows/soak.yml,
  docs/ai/testing/<date>-soak.md
TASK  Run the slice headless for a long horizon (state it) and assert: no NaN/Inf in any
  observable, EU drift within tolerance, memory flat after warm-up (sampled), no unbounded
  collection growth. A scheduled workflow runs it weekly.
DONE WHEN one full run's numbers are in the doc, with the command and machine.
```

### CARD V11

```text
CARD V11 — Five example scenarios a newcomer can run (M11.6)
WAVE 6 · MAX FILES 12 · DEPENDS ON V8, DOC1
BRANCH swarm/v11-example-scenarios
ALLOWED PATHS examples/scenarios/** or src-tauri/tests/fixtures/examples/** (choose one, say why),
  docs/tutorials/02-example-scenarios.md
TASK  Five scenarios, each with one command and the expected result stated as a checkable
  value (checksum or observable range) produced by running it. Mark the commands for DOC8.
DONE WHEN each command was run on a fresh clone and matches its stated result (pasted).
```

### Luồng tài liệu

```text
COMMON TO DOC CARDS
BASE origin/feat/ecological-selection · PR TARGET feat/ecological-selection
Language: Vietnamese, matching the existing docs (README.en.md is the English mirror).
Front matter per docs/governance/DOCUMENTATION_POLICY.md (title, status, owner,
last_reviewed, review_cycle). Diátaxis placement: tutorials learn, how-to do, reference
state, explanation explain. Never redefine a contract: link to docs/reference/ or the ADR
(policy rule 7). Never copy a measured number: link to STATE §1 (policy rule 8).
Every command you write was run by you, in a fresh worktree, and its real output (trimmed,
with "…" where trimmed) is shown. A command you could not run on this platform is labelled
"not verified on <platform>". Mark headless, fast commands with <!-- doc-test --> for DOC8.
DONE WHEN (all DOC cards): node scripts/check_docs_links.mjs exits 0, plus the card's own.
```

### CARD DOC1

```text
CARD DOC1 — Tutorial: from clone to a first headless experiment
WAVE docs · MAX FILES 3 · DEPENDS ON R0
BRANCH swarm/doc1-first-experiment
ALLOWED PATHS docs/tutorials/01-first-headless-experiment.md, docs/tutorials/README.md
TASK  Clone → npm install → a fast backend test target → build the experiment runner in
  release → run the E2 smoke manifest (never `cargo run`, never the desktop app) → verify the
  digests with npm run check:e2 → read the summary. Explain what each output means and what it
  does not mean (inference boundary).
DONE WHEN a second fresh worktree following only the tutorial reaches the same digests (paste).
```

### CARD DOC2

```text
CARD DOC2 — How-to guides
WAVE docs · MAX FILES 8 · DEPENDS ON R0 (the observable guide waits for S2)
BRANCH swarm/doc2-how-to
ALLOWED PATHS docs/how-to/RUN_EXPERIMENTS.md, docs/how-to/WRITE_A_MANIFEST.md,
  docs/how-to/ADD_AN_OBSERVABLE.md, docs/how-to/ADD_AN_INTERVENTION.md, docs/how-to/ADD_A_GATE.md,
  docs/how-to/REPRODUCE_E2.md, docs/how-to/README.md
TASK  One task per page, steps only. Use the anima-lab CLI (ensemble, sweep, branch) as it
  exists at your base — read its --help, do not describe flags from memory. ADD_A_GATE explains
  the negative-control requirement (a gate must be shown red on its own fault).
DONE WHEN every command was run (outputs pasted in the report).
```

### CARD DOC3

```text
CARD DOC3 — Explanation: architecture, and why the gates exist
WAVE docs · MAX FILES 3 · DEPENDS ON R0
BRANCH swarm/doc3-explanation
ALLOWED PATHS docs/explanation/ARCHITECTURE.md, docs/explanation/WHY_THE_GATES.md,
  docs/explanation/README.md
TASK  ARCHITECTURE: sim thread, the single schedule builder, ECS phases, IPC boundary, world
  artifact, experiment runner and adapters, with code anchors by symbol name. WHY_THE_GATES:
  each gate and the failure it exists for, linking STATE §2.1, the incident catalogue (P1) and
  the negative-control table (M2) instead of copying them.
DONE WHEN every symbol named is found by grep at your base (paste the greps).
```

### CARD DOC4

```text
CARD DOC4 — Reference generated from code, with a parity gate
WAVE docs · MAX FILES 6 · DEPENDS ON G8, S2
BRANCH swarm/doc4-generated-reference
AUTHORIZATION R5-AUTH for DOC4 covering the three docs/reference files below and docs/reference/README.md
ALLOWED PATHS scripts/gen_reference_docs.mjs, docs/reference/OBSERVABLES.md,
  docs/reference/ANIMA_LAB_CLI.md, docs/reference/MANIFEST_SCHEMA.md, docs/reference/README.md, package.json
TASK  Generate the observable registry (id, unit, aggregation, meaning) from the code, the CLI
  reference from --help, and the manifest schema from the types. `--check` fails when the code
  and the docs disagree.
DONE WHEN --check exits 0; editing one generated line makes it exit 1 (paste, undo).
```

### CARD DOC5

```text
CARD DOC5 — Reconcile contradictory docs (roadmap 2026-10-16 HT1.7)
WAVE docs · MAX FILES 15 · DEPENDS ON R0
BRANCH swarm/doc5-reconcile
ALLOWED PATHS docs/** except docs/reference/**, docs/decisions/**, docs/planning/STATE_OF_THE_PROJECT.md;
  plus TODO.md, OPEN_SOURCE_ADOPTION_PLAN.md location as found
TASK  For each contradiction listed in HT1.7 (duplicate §3.16/§3.17 rows in STATE; duplicate
  OSS-003/004 rows; "LICENSE is proprietary" in the 2026-07-27 deployment record; README and
  BENCHMARK_BASELINE saying the desktop app never ran though a debug run exists; §3.9 unsafe
  already has SAFETY comments; §3.8 title says two ADRs are proposed; EVOLUTION_EXPERIMENT_CONTRACT
  ER06 cites develop_at_birth which may not exist; AE2.5 plan front matter still active;
  docs/planning/README.md calling TODO.md the backlog) and any you find: state the finding,
  pick the canonical source per policy rule 6, fix the non-canonical one. Historical records
  get a banner pointing forward; never rewrite history. For R5 files (STATE, README, CLAUDE.md,
  ADRs, docs/reference) write the proposed change into the report instead.
DONE WHEN every listed item has a resolution line in the report with file:line.
```

### CARD DOC6

```text
CARD DOC6 — Contributor guide for people and agents
WAVE docs · MAX FILES 3 · DEPENDS ON DOC5
BRANCH swarm/doc6-contributing
ALLOWED PATHS CONTRIBUTING.md, docs/how-to/CONTRIBUTE_WITH_AGENTS.md, docs/how-to/README.md
TASK  Worktree-per-task, stage by path, PR-only, the gates to run before pushing, how to write
  a task card (§6 of docs/ai/planning/2026-11-05-swarm-prompts.md), how to run the swarm (lead,
  worker, verifier), and which files are human-reviewed contracts. AGENTS.md is R5: put any
  proposed AGENTS.md change in the report.
DONE WHEN a reader can find, from CONTRIBUTING.md alone, every command CI runs (cross-check
  against ci.yml, pasted).
```

### CARD DOC7

```text
CARD DOC7 — Proposed refresh of README.md, README.en.md and CLAUDE.md
WAVE docs · MAX FILES 3 · DEPENDS ON DOC1–DOC5
BRANCH swarm/doc7-readme-proposal
ALLOWED PATHS docs/ai/implementation/<date>-readme-claude-md-proposal.md (and, only with
  R5-AUTH for DOC7, README.md, README.en.md, CLAUDE.md)
TASK  Bring the "what is in the code" table, the boundaries list and the commands up to date
  with the trunk; keep README a map that links rather than copies (policy rule 4). For CLAUDE.md,
  propose only changes that the trunk's code makes necessary (new commands, new gates, new
  contracts), each with the commit that made it necessary.
DONE WHEN the proposal lists each change with its reason and source commit.
```

### CARD DOC8

```text
CARD DOC8 — Gate: the commands in the docs still run
WAVE docs · MAX FILES 3 · DEPENDS ON DOC1
BRANCH swarm/doc8-doc-commands-gate
ALLOWED PATHS scripts/check_doc_commands.mjs, .github/workflows/docs-commands.yml (new), package.json
TASK  Extract every fenced command preceded by <!-- doc-test --> in docs/tutorials and
  docs/how-to, run them in order in a fresh checkout, fail on the first non-zero exit and name
  the doc and line. Scheduled weekly plus workflow_dispatch (not on every PR: it builds release).
DONE WHEN the workflow ran green on the draft PR; breaking one marked command makes the script
  exit 1 naming it (paste, undo).
```

### Đợt 7 — phát triển tiếp (1.x)

Mỗi việc Đợt 7 trong kế hoạch tổng bắt đầu bằng hai thẻ dưới đây, rồi mới tới thẻ thực thi. Riêng
tách crate (X1) đã có prompt sẵn: `docs/ai/planning/2026-10-31-antigravity-brief-crate-split.md`
trên thân cây; chạy nó qua PROMPT 0 như một đợt riêng.

### CARD F-REQ

```text
CARD F-REQ — Requirements and design for feature <SLUG>   (owner fills <SLUG> and GOAL)
WAVE 7 · MAX FILES 2
BRANCH swarm/freq-<slug>
DECISION owner states the GOAL in one paragraph here:
ALLOWED PATHS docs/ai/requirements/<date>-feature-<slug>.md, docs/ai/design/<date>-feature-<slug>.md
TASK
Requirements: the question or user need, in scope, out of scope, acceptance criteria that a
command can check. Design: anchors by symbol name (grep them, paste), which invariants it
touches (closed EU, determinism, zero-alloc, save/load, legacy path), which contracts in
docs/reference/ and which ADRs apply, whether a new ADR is required (documentation policy
"ADR bắt buộc khi…"), the tests that would prove it, and the risks. No code.
DONE WHEN every acceptance criterion names the command that will check it.
```

### CARD F-PLAN

```text
CARD F-PLAN — Plan and task cards for feature <SLUG>
WAVE 7 · MAX FILES 1 · DEPENDS ON F-REQ merged and owner-approved
BRANCH swarm/fplan-<slug>
ALLOWED PATHS docs/ai/planning/<date>-feature-<slug>.md
TASK
Break the design into cards using the template in §6 of
docs/ai/planning/2026-11-05-swarm-prompts.md. Each card <= ~15 files; a simulation-semantics
change is its own card (BLOCK 0 R8); a failing test comes first where a bug or invariant is
involved; ALLOWED PATHS of cards meant to run in parallel do not overlap; mark [RUST].
DONE WHEN the cards form a dependency order with no cycle and every card has DONE WHEN commands.
```

---

## §6 Mẫu thẻ cho việc mới

Dùng mẫu này khi bạn (hoặc thẻ F-PLAN) viết thêm task. Ô nào không điền được thì việc đó chưa đủ rõ để
giao cho agent.

```text
CARD <ID> — <imperative title>
WAVE <n> · [RUST if it builds/tests Rust] · MAX FILES <n>
DEPENDS ON  <card ids / owner steps>, each with the command that proves it
BASE        <branch>; record its sha
BRANCH      swarm/<id>-<slug>
PR TARGET   <branch> (draft)
DECISION    <QD ids, or none>
AUTHORIZATION  <R5-AUTH needed? for which paths>
ALLOWED PATHS  <exact paths or narrow globs>

GOAL
<one paragraph: the outcome, and why it matters>

CONTEXT
<facts with file:line and the date they were observed; mark anything read-not-run as a hypothesis>

STEPS
<numbered; failing test first for a bug or an invariant; what NOT to do>

DONE WHEN
<commands that exit 0, and what their output must show>

VERIFY EXTRA
<a check the verifier runs that the worker could not fake, e.g. a negative control>
```
