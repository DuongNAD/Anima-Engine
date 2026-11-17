---
title: Giao việc cho agent-win — thẻ R1c (nâng burn), rồi R0
status: active
owner: maintainers
last_reviewed: 2026-11-17
---

# Giao việc cho `agent-win` — thẻ R1c, rồi R0

Chủ dự án yêu cầu giao việc tiếp cho `agent-win` ngày 2026-11-17. Tài liệu này giữ **prompt để dán** vào
phiên Claude Code trên máy Windows, và lý do chọn thẻ. Nó không định nghĩa lại thẻ: định nghĩa và bảy điều
kiện "xong khi" của R1c ở
[kế hoạch swarm](2026-11-05-swarm-master-plan.md#thẻ-thêm-ngày-2026-11-17-từ-quyết-định-của-chủ-dự-án), quyết
định QĐ-C = C và chứng cứ ở
[roadmap 31/10 §4.3](../../planning/COMPLETION_ROADMAP_2026-10-31.md#4-quyết-định-chỉ-chủ-dự-án-đưa-ra-được).

## Vì sao là R1c, và vì sao là máy Windows

- **R1c không phụ thuộc thẻ nào đang mở.** G3 của `agent-win` (PR #107) đã qua verifier và CI xanh, đang chờ
  chủ dự án merge. R0 thì phải chờ cả Đợt 2 merge. R1c chạy được ngay.
- **Hồ sơ giấy phép phải sinh trên Windows** ([`FINDINGS.md`](../../../FINDINGS.md) F-025). Điều kiện 6 của
  R1c, vốn là việc riêng của con người (thẻ H5) nếu thẻ chạy trên macOS, làm được ngay trong thẻ khi nó chạy
  trên Windows.
- **Windows là nền của job Rust trong CI**, nên kết quả `cargo test` tại chỗ so được với CI.
- **R1c gỡ hai thứ đang chặn phát hành:** văn bản giấy phép còn thiếu của `hexf-parse`, và, nếu phép đo ở
  điều kiện 5 cho phép, chỗ rò bộ nhớ của đường GPU. Nó cũng làm `zip` 0.6.6 rời lockfile, tức gỡ được các
  miễn trừ advisory đang phải thêm dần vào `audit.toml`.

## Thứ tự việc của `agent-win`

| Thứ tự | Thẻ | Điều kiện bắt đầu |
|---|---|---|
| 0 | G3 (PR #107) | Đã xong phần của agent. Chỉ quay lại nếu chủ dự án hoặc review yêu cầu |
| 1 | **R1c** — nâng `burn` | Ngay, sau khi chủ dự án dán prompt dưới đây (dòng `AUTHORIZATION` nằm trong prompt) |
| 2 | **R0** — đo lại toàn bộ | Chỉ khi chủ dự án nói Đợt 2 đã merge hết (G3 và G9). Không tự bắt đầu |

## Dòng uỷ quyền

Theo QĐ-B, một thẻ chỉ được ghi vào `src-tauri/src` khi có dòng `AUTHORIZATION` của chủ dự án liệt kê đúng
path. Dòng cho R1c nằm trong prompt dưới đây. Phiên rà soát tài liệu **soạn** danh sách path; **chủ dự án
cấp** uỷ quyền bằng việc dán prompt. Nếu chủ dự án sửa danh sách khi dán, bản đã dán là bản có hiệu lực. Sau
khi dán, dòng đó được chép vào `R5-AUTH` trong khối `DECISIONS` ở
[`AGENT_WORKFLOW.md`](../../how-to/AGENT_WORKFLOW.md) §4; trước lúc đó nó chưa nằm ở đấy.

## Prompt để dán

Dán nguyên văn vào phiên `agent-win`. Prompt viết bằng tiếng Anh theo quy ước của file prompt swarm.

```text
ROLE
You are agent-win: the Claude Code session on the owner's Windows 11 x64 machine, co-developing
Anima-Engine with agent-mac (Claude Code on a Mac). You reply to the owner in Vietnamese; code,
commits and pull requests stay in English.

READ FIRST
Run `git fetch origin`, then read these from origin/feat/ecological-selection (the trunk):
- CLAUDE.md: the standing rules, and the paragraph that begins "burn is pinned at 0.13.2".
- docs/how-to/AGENT_WORKFLOW.md: sections 1, 3, 4 and 5.
- docs/ai/planning/2026-11-05-swarm-master-plan.md, section "Thẻ thêm ngày 2026-11-17": card R1c
  and its seven done-when conditions. Those seven conditions are the definition of done.
- docs/planning/COMPLETION_ROADMAP_2026-10-31.md, section 4.3: decision QD-C = C and its evidence.
- src-tauri/.cargo/audit.toml: every ignore there names the check that justifies it.

WHERE YOUR WORK STANDS
- G5 is merged (#95).
- G3 is PR #107: verified, CI green, waiting for the owner to merge. Do nothing more on it unless
  the owner or a review asks.

YOUR NEXT CARD: R1c, upgrade burn
Decision QD-C = C (owner, 2026-11-17): keep the GPU learner in the shipped build by upgrading
`burn`, `burn-ndarray` and `burn-wgpu` from 0.13.2 to the 0.22 line. The reason: `hexf-parse` 0.2.1
has no published licence text and blocks distribution. It reaches the build only through
naga 0.19.2 <- wgpu 0.19.4 <- burn-wgpu 0.13.2 <- feature ml-wgpu. The dependency lists on crates.io
say naga 22 to 29 still depend on hexf-parse, naga 30.0.2 does not, and burn-wgpu 0.22.1 goes through
cubecl-wgpu 0.11.4, which requires wgpu ^30. That is a reading of declared dependencies, not a build.

STEP 0, BEFORE ANY CODE
Confirm the premise with a real resolution: on a scratch copy of src-tauri/Cargo.toml, resolve the
upgraded tree and run `cargo tree --features desktop -e normal -i hexf-parse` from src-tauri/. If
hexf-parse is still in the tree, stop and report; the decision rests on that fact. If the 0.22 line
cannot be reached in one step, it is fine to stop at the first version whose tree has no hexf-parse,
and to say which one.

BRANCH AND PR
Work in your own worktree on branch win/r1c-burn-upgrade, cut from origin/feat/ecological-selection.
Claim the card by opening a DRAFT pull request against feat/ecological-selection early. Never merge.
Keep the change to what the upgrade forces: no refactor alongside it.

DONE WHEN (each shown in the PR body with the raw command output)
1. `cargo tree --features desktop -e normal -i hexf-parse` reports the package is not in the tree,
   and `cargo tree --no-default-features -e normal` still shows no burn-wgpu and no naga.
2. `cargo fmt --check`, `cargo clippy --all-targets --features desktop -- -D warnings`,
   `cargo clippy --all-targets --no-default-features -- -D warnings` and
   `cargo test --features desktop` pass. brain_parity_tests, gpu_brain_batch_parity_tests,
   a2c_loss_direction_tests, burn_neural_net_tests and phase5_burn_wgpu_fallback are green with no
   assertion loosened, no test deleted, no new #[ignore]. Capture the test run with
   cmd /c "cargo test --features desktop -- --nocapture > ..\out.txt 2>&1"
   (not a PowerShell pipeline), then run
   node scripts/check_test_targets.mjs out.txt --profile desktop
3. Old saves still load, or a migration is written and declared. Snapshot schema 7 stores the shared
   learner as two burn binary records (`training_model_record` and `optimizer_record` in
   `SavedLearnerWorkerState`, written with `BinBytesRecorder`). That format belongs to the burn
   version, so this is the place most likely to break silently. Write a test that loads a schema-7
   snapshot with a shared learner produced BEFORE the upgrade (generate the fixture from the
   pre-upgrade commit) and either restores it or refuses it with a named error. If the format
   changed, say so and propose the schema bump; do not decide SCHEMA_VERSION policy silently.
4. On the CPU backend, compare the world checksum before and after the upgrade, for the default
   world and for one world with lifetime learning on, by running the same test at both commits.
   Never pin an absolute checksum; compare runs. If they differ, report it; whether
   LIVE_MODEL_VERSION is bumped is the owner's decision, so ask.
5. Measure the GPU path's memory growth on the new wgpu the way PR #30 did: a counting global
   allocator, live heap = alloc minus free, not RSS, with ANIMA_USE_GPU=1, headless, for a few
   minutes. Report MB per minute. Leave the GPU default OFF (`gpu_backend_requested` in
   src-tauri/src/core/resources.rs) unless the measurement is flat, and even then only propose it.
6. Regenerate the licence artefacts on this machine, because they describe a Windows build:
   `npm run gen:compliance`, then `npm run check:compliance` and `npm run check:licenses-complete`.
   The last one must exit 0, and licensing/UNRESOLVED.md must list no component. If it still lists
   one, report which and why; do not edit a generated file by hand.
7. Draft docs/decisions/ADR-0009-<slug>.md from ADR-0000-template.md, status `proposed`, and add its
   row to docs/decisions/README.md; `npm run check:docs` must pass. Put in the PR body a proposed
   replacement for the CLAUDE.md paragraph that begins "burn is pinned at 0.13.2". Do not edit
   CLAUDE.md.

ALSO, IF THE LOCKFILE ALLOWS IT
After the upgrade, some crates ignored in src-tauri/.cargo/audit.toml may have left Cargo.lock (zip,
zstd-safe, zstd-sys, gix-fs, gix-features, libsqlite3-sys). Remove an ignore only when its crate is
no longer in Cargo.lock, show `cargo audit` and `cargo deny check` both exiting 0 afterwards, and
list in the PR which ignores went and which stayed.

RULES THAT DO NOT CHANGE
- One cargo process at a time. Never launch the desktop app.
- Do not touch docs/planning/STATE_OF_THE_PROJECT.md, TODO.md, FINDINGS.md or
  docs/how-to/AGENT_WORKFLOW.md. Propose lines for them in the PR body.
- `BrainGenotype`'s weight layout is w[out * fan_in + in], the transpose of Burn's
  [d_input, d_output]. Use `ActorCriticModel::from_flat_weights` and keep the EB-S02 parity gate
  green. There is one A2C objective, `simulation_loop::a2c_loss`, with actor coefficient +td.
- If a condition cannot be met, stop and report what you found instead of weakening it.
- When the work is done, start a fresh verifier (clean context) that reruns DONE WHEN from scratch
  and compares your report with `git diff`. Only then mark the PR ready for review.

AUTHORIZATION (owner, 2026-11-17): for card R1c only, you may write to src-tauri/Cargo.toml,
src-tauri/Cargo.lock, src-tauri/src/ai/model.rs, src-tauri/src/core/training.rs,
src-tauri/src/core/snapshot.rs, src-tauri/src/core/simulation_loop.rs,
src-tauri/benches/tick_systems.rs, the five test files named in condition 2, one new test file and
its fixtures under src-tauri/tests/ for condition 3, src-tauri/.cargo/audit.toml and
src-tauri/deny.toml (only to remove ignores whose crate has left Cargo.lock), the generated licence
artefacts (NOTICE, sbom.cdx.json and the generated files under licensing/), and
docs/decisions/ADR-0009-*.md with its row in docs/decisions/README.md. This overrides AGENTS.md's
read-only declaration and BLOCK 0 R5 for exactly these paths and no others. If the upgrade forces a
change to any other file under src-tauri/src, stop and ask the owner, naming the file and the reason.

AFTER R1c
Your next card is R0 (re-measure every gate of STATE section 5 on one trunk commit and draft the
section 1 patch). Start it only when the owner says Wave 2 is merged, which means G3 (#107) and G9.
Do not start it on your own.

REPORT
Card id, base and head commit, branch, PR number, then the seven conditions one by one with raw
output, then SURPRISES: anything you found that the card did not expect.
```

## Điều tài liệu này không khẳng định

- Không nói rằng nâng `burn` chắc chắn gỡ được `hexf-parse`. Căn cứ là danh sách phụ thuộc đã khai trên
  crates.io; bước 0 của prompt tồn tại để kiểm bằng một lần phân giải thật trước khi viết mã.
- Không nói rằng bản `wgpu` mới hết rò bộ nhớ. Điều kiện 5 đo điều đó.
- Danh sách path trong dòng `AUTHORIZATION` là ước lượng từ các file đang dùng `burn` trên thân cây tại
  `49eac9a` (`git grep -l 'burn::\|burn_ndarray\|burn_wgpu' -- src-tauri`: bốn file dưới `src-tauri/src`,
  một bench, năm file test); nó chưa được thử bằng một lần nâng thật, nên prompt có lối dừng và hỏi.
- Không nói rằng G9 sẵn sàng merge. Nhánh `mac/g9-root-hygiene` đã qua verifier, nhưng allow-list của nó
  viết trước khi `TASK.md` bị xoá và `AUDIT_REPORT.md` được chuyển đi; xem hàng G9 ở
  [`AGENT_WORKFLOW.md`](../../how-to/AGENT_WORKFLOW.md) §3.
