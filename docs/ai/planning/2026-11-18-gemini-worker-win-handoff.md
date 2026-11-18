---
title: Giao việc cho Gemini trên Windows (vai agent-win) — R0 trước, R1c sau
status: active
owner: maintainers
last_reviewed: 2026-11-18
---

# Giao việc cho Gemini trên Windows — R0 trước, R1c sau

Ngày 2026-11-18 chủ dự án quyết định giao vai **`agent-win`** cho **Gemini** chạy trên máy Windows của
chủ dự án ("bắt đầu giao việc cho gemini trên win để nó làm worker win"). Tên vai vẫn là `agent-win`, nên
mọi hàng của hàng đợi thẻ ở [`AGENT_WORKFLOW.md`](../../how-to/AGENT_WORKFLOW.md) §3 vẫn đúng. Chỉ công cụ
chạy vai đó đổi: trước là Claude Code, nay là Gemini.

Tài liệu này giữ **prompt để dán** cho thẻ đầu tiên và phần đầu cần ghép thêm vào prompt R1c. Nó không định
nghĩa lại thẻ nào. R0 được định nghĩa ở mục `CARD R0` trong [file prompt swarm](2026-11-05-swarm-prompts.md),
R1c ở [bản giao việc R1c](2026-11-17-agent-win-handoff-r1c.md).

## Vì sao R0 trước

- **R0 chỉ đo, không sửa mã.** Thẻ chỉ ghi hai file dưới `docs/ai/testing/`. Với một worker mới, đây là
  cách thử ít rủi ro nhất: nếu worker hiểu sai một luật, cái giá là một PR tài liệu bị đóng, không phải một
  lần nâng thư viện học máy làm hỏng phép đo.
- **R0 đang mở và là cổng ra của chặng 3.** Đợt 2 đã merge hết ngày 2026-11-18 (G3 #107, G9 #114). R1c
  không có lý do phải đi trước.
- **Windows là nền của job Rust trong CI**, nên một lượt đo đầy đủ trên Windows so được trực tiếp với CI.
- **R1c sửa đường học máy**, chỗ nhạy về số nhất của engine. Nên giao nó khi đã thấy worker làm đúng một thẻ.

Chủ dự án có thể đảo thứ tự bằng cách dán prompt R1c trước. Khi đó phần đầu ở mục "Prompt 2" vẫn phải được
ghép vào.

## Khác biệt khi worker là Gemini, không phải Claude Code

- **Gemini không tự nạp `CLAUDE.md`.** Gemini CLI nạp `GEMINI.md` theo mặc định, mà repo không có file đó.
  Vì vậy prompt kể tên từng file phải đọc và không dựa vào việc công cụ tự nạp file nào.
- **Không có subagent verifier.** Prompt R1c cũ yêu cầu worker tự mở một verifier với ngữ cảnh sạch. Với
  Gemini, phiên điều phối trên Mac (`agent-mac`) chạy lại các điều kiện "xong khi" từ PR và so với `git diff`.
  Worker chỉ đánh dấu PR sẵn sàng sau khi phiên đó báo đã kiểm.
- **Các luật không đổi:** QĐ-B (chỉ ghi vào `src-tauri/src` khi có dòng `AUTHORIZATION`), một tiến trình cargo
  một lúc, không chạy app desktop, không merge, không `--admin`.

## Prompt 1 — thẻ R0 (dán ngay)

Prompt viết bằng tiếng Anh theo quy ước của file prompt swarm. Thẻ R0 cho phép một dòng uỷ quyền tuỳ chọn để
ghi thẳng vào `STATE_OF_THE_PROJECT.md`. Prompt dưới đây **không** cấp dòng đó: worker viết bản đề xuất vào
file thứ hai, và phiên điều phối đưa nó vào STATE sau khi kiểm.

```text
ROLE
You are agent-win, a coding worker for Anima-Engine running on the owner's Windows 11 x64 machine.
You are Gemini; the other worker, agent-mac, is Claude Code on a Mac and coordinates the card queue.
Reply to the owner in Vietnamese. Code, commits and pull requests stay in English.

READ FIRST (your tool does not load these on its own; open each one)
Run `git fetch origin`, then read from origin/feat/ecological-selection (the trunk):
- CLAUDE.md: the standing rules for every agent in this repository, not only Claude. Every rule in it
  applies to you. Read the "Commands" section closely.
- AGENTS.md: which paths are read-only and how an AUTHORIZATION line overrides that.
- docs/how-to/AGENT_WORKFLOW.md: sections 1, 3, 4 and 5.
- docs/planning/STATE_OF_THE_PROJECT.md: section 1.h (the style to copy) and section 5.
- docs/ai/planning/2026-11-05-swarm-prompts.md: the block "CARD R0". That block is the definition of
  done.
- .github/workflows/ci.yml: the steps of the `rust` job run on windows-latest, the same platform as
  you.

YOUR CARD: R0, re-measure every gate at one trunk commit
Record the sha of origin/feat/ecological-selection when you start. Every row must be measured at that
sha. If the trunk moves while you work, keep your sha; do not mix commits.

BRANCH AND PR
Work in a fresh worktree on branch win/r0-remeasure, cut from that sha. Open a DRAFT pull request
against feat/ecological-selection as soon as the first file exists, to claim the card. Never merge,
never use --admin, never push to the trunk or to main.

WHAT YOU MAY WRITE
Only these two files, and nothing else:
  docs/ai/testing/remeasure-<YYYY-MM-DD>-<short-sha>.md
  docs/ai/testing/state-1x-proposed-<YYYY-MM-DD>.md
Give both the front matter that `npm run check:docs` requires (copy it from a neighbouring file in
docs/ai/testing/) and add both to the index docs/ai/testing/README.md. `npm run check:docs` must pass
on your branch. You have no AUTHORIZATION line in this prompt, so you may NOT edit
docs/planning/STATE_OF_THE_PROJECT.md, TODO.md, FINDINGS.md or docs/how-to/AGENT_WORKFLOW.md, and
nothing under src-tauri/src or src-tauri/tests.

HOW TO RUN THINGS ON WINDOWS
- One cargo process at a time. Never run two cargo commands in parallel. A full Rust build is heavy
  and has crashed a development machine before.
- Never launch the desktop app (`npm run tauri:dev`, `cargo run`, `tauri dev`). Headless only.
- Always pass `--features desktop` to cargo test, as CI does. A bare `cargo test` silently skips eight
  feature-gated test files.
- Capture cargo test output with BOTH streams in write order, from src-tauri/:
    cmd /c "cargo test --features desktop > ..\out-desktop.txt 2>&1"
  Do NOT use a PowerShell pipeline; it reorders the streams and makes the policy script under-report.
  Then, from the repo root:
    node scripts/check_test_targets.mjs out-desktop.txt --profile desktop
  Do the same with no features into out-default.txt and `--profile default`.
- Run TypeScript scripts with `node scripts/run_ts.mjs`, never esbuild (it is not installed).
- Do not commit the captured out*.txt files. Quote what you need in the report.

STEPS (from CARD R0, made concrete)
1. Frontend gates, from the repo root, one after another: `npm ci`, `npm ci --prefix tests`,
   `npm run lint`, `node scripts/eslint_ratchet.mjs`, `npm run typecheck:tests`, `npm run test`,
   `npm run test:frontend`, `npm run build`, `npm run check:csp`, `npm run check:bundle`,
   `npm run check:audit`, `npm run check:root-hygiene`, `npm run check:text-hygiene`,
   `npm run check:docs`, `npm run check:ipc`.
2. Backend gates, from src-tauri/: `cargo fmt --check`;
   `cargo clippy --all-targets --features desktop -- -D warnings`;
   `cargo clippy --all-targets --no-default-features -- -D warnings`; the two captured test runs above
   with their policy checks; `cargo audit`; `cargo deny check`. Then every remaining step of the `rust`
   job in ci.yml, using the exact command that step runs.
3. Reproducibility: run `cargo test --features desktop --test live_experiment_tests` in 6 separate
   processes, one after another, and report each result. Run the test
   `a_fork_from_a_known_tick_shares_its_prefix_and_diverges_only_afterwards` with `-- --nocapture`
   in 6 separate processes and report every checksum it prints. Never pin or compare against an
   absolute checksum from a document; checksums differ between platforms. Report whether the six runs
   agree with each other.
4. Counts: tests passed / failed / ignored per profile, number of test targets per profile, and test
   files by directory.
5. Steps you cannot run here (the frontend job runs on ubuntu-latest in CI; Playwright E2E needs a
   release build): report them as "not run here — see CI run <id>" with that run's conclusion, taken
   from `gh run list --branch feat/ecological-selection --workflow ci.yml` for your sha. If you do run
   Playwright, build `cargo build --release --features desktop` first and say so.
6. In the second file, write the proposed section 1.i of STATE in the style of section 1.h: type
   column, command, result. Nothing in it may come from a run other than yours or the CI run you cite.

DONE WHEN
Every row has a command, a date, the sha, the platform, and raw output (trimmed to the lines that
matter, never paraphrased). A row that failed is marked failed with its output; do not re-run until it
passes and report only the pass. If a gate fails, measure it, report it, and continue with the next;
do not fix anything.

HANDOFF
When both files are written and `npm run check:docs` passes, push, leave the PR in draft, and report
to the owner. agent-mac reruns a sample of your rows and compares the report with `git diff` before
the PR is marked ready. Do not mark it ready yourself.

REPORT (to the owner, in Vietnamese)
Card id, base sha, branch, PR number, the table of gates with pass/fail, then SURPRISES: anything that
did not match what STATE section 1.h or CLAUDE.md says.
```

## Prompt 2 — thẻ R1c (dán sau khi R0 đã được kiểm)

Dùng nguyên prompt trong [bản giao việc R1c](2026-11-17-agent-win-handoff-r1c.md#prompt-để-dán), nhưng
**ghép khối dưới đây lên đầu**. Khối này thay hai chỗ của prompt cũ vốn giả định worker là Claude Code: vai
trò, và verifier.

```text
PREAMBLE FOR A GEMINI WORKER (this block overrides the prompt below where they differ)
- You are Gemini, not Claude Code. Where the prompt below says "the Claude Code session", read
  "the Gemini session". Your tool does not load CLAUDE.md or AGENTS.md on its own: open and read
  both before anything else; every rule in them applies to you.
- You cannot start a subagent. Ignore the rule "start a fresh verifier (clean context)". Instead,
  when the work is done, push, leave the PR in draft, and report. agent-mac (the coordinating
  session) reruns DONE WHEN from scratch and compares your report with `git diff`. Do not mark the PR
  ready yourself.
- Everything else in the prompt below stands, including the AUTHORIZATION line and its list of paths.
```

Khi chủ dự án dán Prompt 2, dòng `AUTHORIZATION` của R1c bắt đầu có hiệu lực, như bản giao việc R1c đã ghi.
Lúc đó dòng ấy được chép vào `R5-AUTH`.

## Điều tài liệu này không khẳng định

- Không nói Gemini làm được thẻ như Claude Code. R0 được đặt trước chính là để thử điều đó trên một thẻ ít rủi
  ro.
- Không nói máy Windows đã có `gh` đăng nhập, `cargo-audit` và `cargo-deny`. Nếu thiếu, worker báo thiếu; nó
  không được tự cài từ nguồn ngoài danh sách của `ci.yml`.
- Câu "Gemini CLI nạp `GEMINI.md` theo mặc định" nói về cấu hình mặc định của công cụ. Nếu chủ dự án đã cấu
  hình khác, prompt vẫn đúng vì nó kể tên từng file phải đọc.
