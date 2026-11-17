---
title: Chạy coding agent trên Anima Engine
status: active
owner: maintainers
last_reviewed: 2026-11-17
review_cycle: monthly
---

# Chạy coding agent trên Anima Engine

Một trang cho người bật agent (swarm Antigravity, Claude Code, Codex…). Trang này **không** là kế
hoạch mới. Nó gom những gì cần để bắt đầu và trỏ về đúng tài liệu theo vai (luật L6 của
[kế hoạch 13/11](../planning/DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md)):

| Vai | Tài liệu |
|---|---|
| Thứ tự chặng, luật điều phối L1–L9 | [Kế hoạch định hướng 13/11](../planning/DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md) |
| Danh sách thẻ, đợt, quyết định chặn thẻ | [Kế hoạch swarm 05/11](../ai/planning/2026-11-05-swarm-master-plan.md) |
| Prompt lead, luật swarm, verifier, từng thẻ | [File prompt swarm](../ai/planning/2026-11-05-swarm-prompts.md) |
| Prompt chi tiết của B3b, HT-C…; BLOCK 0 (luật đứng) | [Brief 31/10](../ai/planning/2026-10-31-antigravity-brief.md) |
| Quyết định của chủ dự án | [Roadmap 31/10 §4](../planning/COMPLETION_ROADMAP_2026-10-31.md#4-quyết-định-chỉ-chủ-dự-án-đưa-ra-được) |
| Số đo | [STATE §1.h](../planning/STATE_OF_THE_PROJECT.md#1h-đo-2026-11-17-tại-6f14124--ci-của-lần-fast-forward-main-và-một-lượt-chạy-cục-bộ-macos) (CI của `main` = thân cây tại `6f14124` cộng một lượt chạy cục bộ macOS, 2026-11-17) là số mới nhất; banner đầu [STATE §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền) trỏ tới nó |

Trạng thái trong trang này ghi ngày **2026-11-17**. Khi trang và một tài liệu ở bảng trên lệch nhau,
tài liệu ở bảng thắng; sửa trang này.

## 1. Đang ở đâu

- **2026-11-17: chặng 1 và chặng 2** của kế hoạch 13/11 **đã đóng**. Thân cây và `main` được bảo vệ, hai job
  CI là gate bắt buộc ([STATE §3.00](../planning/STATE_OF_THE_PROJECT.md#300-việc-kế-tiếp--cập-nhật-2026-11-17)).
  Chặng 0 vẫn còn các ô quyết định trống ở dưới. Theo thứ tự, chặng kế là chặng 3. Thứ tự không đổi:
  xanh → gộp → đo lại → khoa học → phát hành → nhiều máy.
- **Thân cây là `feat/ecological-selection`** (QĐ-A, 2026-11-01). Chồng PR #38 → #40 trên
  `fix/ci-gates-green` **đã được gộp vào** thân cây ở #45 (thẻ T2, 2026-11-15).
- **Tiến độ từng chặng** nằm ở một bảng duy nhất:
  [STATE §3.00](../planning/STATE_OF_THE_PROJECT.md#bảng-tiến-độ-theo-chặng-đối-chiếu-2026-11-17).
  Trang này chỉ giữ trạng thái từng thẻ (§3).
- **2026-11-16:** T2 (HT-C) đã merge thành PR #45 (2026-11-15), và `main` đã được fast-forward lên thân cây
  (a43872c → bb78e63; `main` là tổ tiên, 0 commit riêng). PR vẫn nhắm vào `feat/ecological-selection`
  trong đợt "readable world"; `main` được kéo lên thân cây ở các mốc.
- **Đã quyết 2026-11-14:** QĐ-A′ (giữ mortality của thân cây), QĐ-B (uỷ quyền theo thẻ), QĐ-F028
  (commit icon thật). **Còn trống:** QĐ-C, QĐ-1, QĐ-EN, QĐ-P1…P3, QĐ-M1…M8 (QĐ-2/H quyết 2026-11-17, dưới).
- **Đã quyết 2026-11-16** (phiên lead theo uỷ quyền của chủ dự án trong chat): QĐ-E2M = B (#59),
  QĐ-4 = A và QĐ-5 = có ([ADR-0005](../decisions/ADR-0005-live-population-turnover.md)), QĐ-LOC = A
  ([ADR-0006](../decisions/ADR-0006-locomotion-actuation.md)), QĐ-ECO = A
  ([ADR-0007](../decisions/ADR-0007-live-ecology-sustainability.md)).
- **Đã quyết 2026-11-17** (phiên lead theo uỷ quyền "làm theo bản kế hoạch"): QĐ-ECO-B = opt-in (thẻ B của
  [ADR-0007](../decisions/ADR-0007-live-ecology-sustainability.md), #87; chỉ cho thăm dò S43, không mặc định).
  **Còn trống, chờ chủ dự án:** QĐ-SEL1b (thí nghiệm chọn lọc theo khan hiếm làm lại hay không, với yếu tố
  nào; xem chẩn đoán sau E-SEL1) và việc có làm thẻ C1 của ADR-0007 không.
- **Đã quyết 2026-11-17** (phiên lead, cùng uỷ quyền "theo bản kế hoạch"): **QĐ-2/QĐ-H** theo đề xuất của
  [kế hoạch swarm](../ai/planning/2026-11-05-swarm-master-plan.md) §2: gỡ mọi bằng chứng và dữ liệu giả ngay
  (G4, G6), giữ mã; Map 2 và Research Dashboard có ship hay không thì quyết sau. Chủ dự án đảo lại được.
- **Hai máy từ 2026-11-17, giống OxideSwarm:** `agent-mac` (Claude Code trên Mac) làm G6 → G4 → G9 và phần
  còn lại của G8; `agent-win` (Claude Code trên Windows, cùng nền với job Rust của CI) làm G5 → G3, rồi R0
  khi chủ dự án nói Đợt 2 đã xong. Nhánh `mac/…` và `win/…`; nhận thẻ bằng một draft PR; mỗi máy chỉ sửa
  `ALLOWED PATHS` của thẻ mình. `agent-mac` là người ghi duy nhất của `STATE`, `TODO.md`, `FINDINGS.md` và
  trang này; `agent-win` đề xuất dòng cho các file đó trong thân PR của nó.
- **Cây làm việc đã dọn (MP-0.5):** 10 file test dở dang và phần thêm F-036 vào `FINDINGS.md` đã
  được chuyển nguyên byte sang nhánh **local** `wip/hardening-m2-m4-tests` (`f9ff5f1`, chưa push).

## 2. Việc của chủ dự án trước khi bật agent

1. **Đưa thay đổi lên thân cây qua PR và merge nó.** Lead đọc kế hoạch, thẻ và brief bằng
   `git show origin/…`; thay đổi chưa lên thân cây thì agent không thấy, và worktree tạo từ `HEAD` không
   chứa file chưa commit. Từ 2026-11-17 thân cây được bảo vệ: push thẳng bị từ chối, và PR chỉ merge
   được khi `Rust (src-tauri)` và `Frontend` cùng xanh.
2. **Mở phiên Claude Code mới** nếu dùng Claude: hook rustfmt chỉ nạp khi phiên bắt đầu.
3. Mở teamwork Antigravity, dán **PROMPT 0 (Lead)** từ
   [file prompt swarm §1](../ai/planning/2026-11-05-swarm-prompts.md), rồi điền khối `DECISIONS`
   bằng khối ở §4 dưới đây.
4. Đọc diff và merge từng PR. Agent không bao giờ merge.

## 3. Hàng đợi thẻ

Đối chiếu `gh pr list --state all` và `git branch -r` ngày **2026-11-17**. Định nghĩa từng thẻ ở
[kế hoạch swarm §3](../ai/planning/2026-11-05-swarm-master-plan.md); prompt ở
[file prompt](../ai/planning/2026-11-05-swarm-prompts.md). Ký hiệu: ✅ đã merge · 🔄 đang làm ·
⬜ chưa bắt đầu · ⛔ bị chặn.

### Đợt 1 — đã đóng

| Thẻ | Việc | Trạng thái |
|---|---|---|
| **D0** | Kiểm kê PR mở và nhánh remote | ✅ #42 |
| **D1** | Phân loại finding của `AUDIT_REPORT.md` | ✅ #43 |
| **T1** `[RUST]` | **B3b** — năm gate mà PR #40 làm đỏ | ✅ #44 |
| **T2** `[RUST]` | **HT-C** — gộp chồng PR vào thân cây | ✅ #45 (2026-11-15) |
| **P1–P3** | Bài báo | ⛔ QĐ-P1, QĐ-P2 trống. Luật L4: ô quyết định trống thì không chạy |

### Đợt 2 — đang chạy (chặng 3)

| Thẻ | Việc | Trạng thái | Máy | Ghi chú |
|---|---|---|---|---|
| **G1** `[RUST]` | **B2** — zero-alloc ở đường sinh/tử | ✅ #48 | — | |
| **G2** | **B5** — `test:e2e` đủ bộ, gate danh sách spec | ✅ #47 | — | |
| **G3** `[RUST]` | **B6a** — giá trị bịa trong telemetry và provenance | ⬜ chưa có PR | `agent-win` | Thẻ kế tiếp của `agent-win` |
| **G4** | **B6b** — lệnh IPC không tồn tại, dữ liệu giả trên dashboard | ✅ #98 (2026-11-17) | — | QĐ-2/QĐ-H quyết 2026-11-17 |
| **G5** `[RUST]` | **B6c** — bypass sổ cái, nghi rò EU của `PathogenTransmission` | ✅ #95 (2026-11-17) | — | |
| **G6** | Gỡ bằng chứng giả của Map 2 | ✅ #94 (2026-11-17) | — | |
| **G7** `[RUST]` | Khôi phục assertion bị nới và test bị xoá | ✅ #49 | — | |
| **G8** | Hợp đồng IPC | ✅ phần lớn, 2026-11-14 | `agent-mac` | Có [`IPC_CONTRACT.md`](../reference/IPC_CONTRACT.md) và gate `scripts/check_ipc_reference.mjs`. Còn thiếu so với thẻ: `file:line` cho từng mục |
| **G9** | Gate vệ sinh thư mục gốc | 🔄 nhánh `mac/g9-root-hygiene`, chưa có PR | `agent-mac` | |
| **R0** `[RUST]` | Đo lại toàn bộ gate của STATE §5, soạn bản vá §1 | ⛔ chờ cả Đợt 2 merge | `agent-win` | Cổng ra của chặng 3 |

### Đợt 3 trở đi

Hai thẻ của Đợt 3 đã merge **trước** R0, theo quyết định của phiên lead ngày 2026-11-16 (§1): **S1**
(#66, `live.reproduction`) và **S4** (#75, OSS-072 MRCA). STATE §1.h đã đo một commit có cả hai; R0 vẫn
là cổng ra của chặng 3 và chạy sau khi Đợt 2 merge.
**T3** (mở PR thân cây → `main`) đã được thay bằng cách làm thực tế: `main` được fast-forward lên thân cây
ở các mốc. **M1** (ma trận tái lập đa nền tảng) cũng đã merge trước R0, ở #100 và #102. Các thẻ còn lại
của Đợt 3 (S2, S3, M2–M4) và mọi thẻ từ Đợt 4 chưa bắt đầu.

Những việc đã merge mà **không** có thẻ trong kế hoạch swarm (ADR-0005…0007, vận động, sinh thái opt-in,
bản đăng ký E-SEL1, các thẻ "thế giới đọc được") được liệt kê ở
[STATE §3.00](../planning/STATE_OF_THE_PROJECT.md#300-việc-kế-tiếp--cập-nhật-2026-11-17).

Lead chạy tối đa **hai** thẻ `[RUST]` cùng lúc, mỗi thẻ một worktree riêng. Build Rust đầy đủ nặng
và đã từng làm treo máy phát triển; nếu chỉ có máy này, chạy **một**.

## 4. Khối `DECISIONS` để dán vào PROMPT 0

Dán nguyên văn. Ô để trống là chủ ý: thẻ cần ô đó sẽ bị lead từ chối.

```text
QD-A'     = keep core/mortality.rs::mortality_system (trunk); drop main's
            agent_systems.rs::agent_mortality_system from the schedule after HT-C (owner, 2026-11-14)
QD-B      = per-card authorization. AGENTS.md stays the default (src-tauri/src read-only); a card
            may write src-tauri/src or an R5 file only with an AUTHORIZATION line in R5-AUTH that
            names its exact paths (owner, 2026-11-14)
QD-C      =
QD-2/QD-H = remove every piece of fabricated evidence and data now (G4, G6); keep the code;
            ship-or-not decided later (lead session under the owner's "follow the plan", 2026-11-17)
QD-4      = A: continuous energy-gated reproduction in the live adapter behind manifest key
            live.reproduction (absent/0 = off); ADR-0005 (owner's delegate in chat, 2026-11-16)
QD-5      = yes: ADR-0005, scope = continuous reproduction in live experiments only;
            EvolutionaryReplacement unchanged (owner's delegate in chat, 2026-11-16)
QD-LOC    = A: a steering drive writes InertiaComponent.target_velocity; speed from the agent's own
            CPG/brain output, not a per-class constant; ADR-0006 (owner's delegate in chat, 2026-11-16)
QD-ECO    = A: opt-in live keys for a steering drive floor and thirst steering so founders can move
            and reach water; default world unchanged; ADR-0007 (owner's delegate in chat, 2026-11-16)
QD-ECO-B  = opt-in: live.kill_hydration, live.kill_efficiency, live.target_min_energy (ADR-0007 card B,
            #87); for S43 exploration only, never default (owner's delegate in chat, 2026-11-17)
QD-SEL1b  =
QD-ECO-C1 =
QD-6      = docs/reference/IPC_CONTRACT.md. It and its gate (scripts/check_ipc_reference.mjs, in
            npm run check:docs) exist on the trunk since 2026-11-14; card G8 only adds file:line
            per entry
QD-F028   = yes: copy src-tauri/icons/ from origin/feat/ecological-selection (owner, 2026-11-14)
QD-EN     =
QD-P1     =
QD-P2     =
QD-1      =
R5-AUTH   =
  AUTHORIZATION (owner, 2026-11-14): for card T1 only, you may write to the paths in its
  ALLOWED PATHS (src-tauri/Cargo.lock, src-tauri/deny.toml, scripts/check_bundle_budget.mjs,
  licensing/bundle-closure.json, and vite.config.ts only if its task 3 concludes a chunking-config
  change) and to src-tauri/icons/ (copied unchanged from origin/feat/ecological-selection, QD-F028).
  This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these paths and no
  others.
  AUTHORIZATION (owner, 2026-11-14): for card D0 only, you may also add one row for the report you
  write to docs/ai/README.md. The report needs the front matter of
  docs/governance/DOCUMENTATION_POLICY.md; npm run check:docs must exit 0.
CURRENT WAVE = 2
```

Dòng uỷ quyền của T2 dưới đây **đã dùng xong** (T2 merge ngày 2026-11-15) và được giữ làm mẫu cho
dòng uỷ quyền của thẻ sau:

```text
  AUTHORIZATION (owner, <YYYY-MM-DD>): for card T2 only, you may write the merge commit on
  swarm/t2-htc-merge, one new test file under src-tauri/tests/ for the one-agent starvation test,
  and the Default impl of HomeostaticState in src-tauri/src/ai/hrrl.rs. PROJECT.md, AGENTS.md and
  docs/planning/STATE_OF_THE_PROJECT.md stay with the owner.
```

## 5. Luật cho mọi agent

Nguồn đầy đủ: BLOCK 0 của brief, BLOCK S của file prompt, §4 của kế hoạch 13/11,
[`AGENTS.md`](../../AGENTS.md), [`CLAUDE.md`](../../CLAUDE.md). Bản tóm tắt:

- Mỗi thẻ: worktree riêng, nhánh riêng, chỉ sửa `ALLOWED PATHS`, mở **draft PR**, không merge.
- Ô `DECISION` hoặc `AUTHORIZATION` trống thì **dừng và báo**, không chọn thay chủ dự án (L4).
- Một verifier mới, ngữ cảnh sạch, chạy lại `DONE WHEN` từ đầu và đối chiếu báo cáo với `git diff`.
  Báo cáo dán **nguyên** output lệnh.
- Không nới hay xoá assertion, không thêm `#[ignore]` để né, không bịa seed, số hay ảnh làm bằng chứng.
- Không chạy app desktop. Không sửa `CLAUDE.md`, `AGENTS.md`, `PROJECT.md`, `README*`, `STATE`
  nếu thẻ không uỷ quyền rõ (R5).
- Tài liệu mới trong `docs/`: front matter theo chính sách, một dòng trong chỉ mục của thư mục.
- Đổi lệnh hoặc event IPC: sửa [`IPC_CONTRACT.md`](../reference/IPC_CONTRACT.md) trong cùng commit.
- Kiểu dữ liệu mà hợp đồng sinh vật nêu tên (`DevelopedPhenotype`, `EnvSample`…) **chưa có trong
  mã**; đừng tự thêm chúng ngoài thẻ — xem bảng "Trạng thái triển khai" của
  [hợp đồng](../reference/CREATURE_DEVELOPMENT_CONTRACT.md).

## 6. Lệnh xác minh (macOS, từ gốc repo)

Tài liệu và frontend:

```bash
npm run check:docs
```

```bash
npm run check:ipc
```

```bash
npm run lint && node scripts/eslint_ratchet.mjs
```

```bash
npm run test && npm run test:frontend && npm run build
```

Backend — **một** tiến trình `cargo test` tại một thời điểm, hứng cả hai luồng vào một file:

```bash
cd src-tauri && cargo fmt --check
```

```bash
cd src-tauri && cargo test --features desktop -j 2 > ../out.txt 2>&1; cd .. && node scripts/check_test_targets.mjs out.txt --profile desktop
```

Luồng hardening theo `AGENTS.md` (agent chỉ ghi `src-tauri/tests`) dùng `just verify` và chỉ coi là
xong khi thấy chuỗi `QUA CONG`.

## 7. Bẫy đã biết khi giao việc

Mỗi dòng dưới đây đã được kiểm ngày 2026-11-14; dòng nào ghi ngày khác thì được kiểm lại ngày đó.

- **Số mới nhất của STATE là §1.h, đo tại `6f14124`** *(kiểm 2026-11-17)*. Mọi thứ merge sau commit đó
  chưa nằm trong lượt đo nào. Đừng suy số từ danh sách PR; lượt đo lại sau Đợt 2 là việc của thẻ R0.
- **~~`fix/ci-gates-green` không biên dịch trên macOS~~ — đã hết hiệu lực 2026-11-15:** nhánh đó đã
  gộp vào thân cây ở #45, và cả thân cây lẫn `main` mang icon PNG thật (F-028 đã đóng).
- **~~`PROJECT.md` trên `main` đã bị ghi đè~~ — đã hết hiệu lực 2026-11-16:** `main` nay mang `PROJECT.md` của thân cây. Trước đó nó đã bị ghi đè bằng một tài liệu khác (kế hoạch swarm §1). Khi T2 gộp,
  giữ bản thân cây; mục Interface Contracts của nó đã được đối chiếu với mã ngày 2026-11-14.
- **Hook rustfmt trước 2026-11-14 không chạy trên macOS** (gọi `powershell.exe`). File `.rs` sửa trên
  Mac trước ngày đó có thể chưa format; roadmap 31/10 đo được 67 khối lệch `cargo fmt`.
- **`npm run check:docs` đọc cả file chưa `git add`.** Một tài liệu mới thiếu front matter hoặc thiếu
  dòng chỉ mục sẽ làm gate đỏ ngay trên máy, trước khi lên CI.
- **Còn hai mâu thuẫn giữa các tài liệu kế hoạch chưa sửa** (kế hoạch 13/11 §12, kiểm lại 2026-11-17):
  tên "E3" chỉ hai thí nghiệm khác nhau (chờ QĐ-EN); `H1`–`H4` chỉ những việc khác nhau trong
  `EXECUTION_PLAN` và kế hoạch swarm — khi trích phải ghi kèm tên tài liệu. Mâu thuẫn thứ ba đã hết:
  roadmap 16/10 có trên thân cây từ khi hai cây hợp nhất.

## 8. Sau mỗi đợt

Chủ dự án merge, ghi số đo vào STATE §1 (một tiểu mục `§1.x` mới, tiếp sau tiểu mục mới nhất, hiện là §1.h), thêm
một dòng vào [`TODO.md`](../../TODO.md), rồi sửa §1 và §3 của trang này.
