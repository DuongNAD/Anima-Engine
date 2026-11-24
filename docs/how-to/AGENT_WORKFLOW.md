---
title: Chạy coding agent trên Anima Engine
status: active
owner: maintainers
last_reviewed: 2026-11-24
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

- **2026-11-23 (đối chiếu của lead).** Ba worker Gemini nhận thẻ qua hub: `anima-mac` (Mac),
  `anima-case` (Windows 1, RTX; vai `agent-win`) và `anima-dell` (Dell Iris Xe; im lặng từ 2026-11-20).
  Phiên Claude Code điều phối là lead: giao thẻ, thẩm định và ghi verdict. Bốn quyết định ngày này ở §4:
  QD-OFFQ (giữ việc làm ngoài hàng đợi, đóng băng PR mới của `anima-mac`), QD-DOCLANG (tài liệu công khai
  bằng tiếng Anh), QD-VIAB1C, và các dòng `R5-AUTH` mới. VIAB-1 đã có kết quả (#215): một ô E3b đạt mọi chỉ
  tiêu, SEL2-P2 không ô nào đạt vì nhánh mut0 không sống được (chẩn đoán ở #216). Bản nháp đăng ký E3b là
  #217 và #218, chờ chủ dự án quyết D1–D5. Trạng thái từng thẻ: §3, mục "2026-11-19 → 2026-11-23".
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
  (commit icon thật). **Còn trống:** QĐ-M1…M8 (các ô khác quyết 2026-11-17, dưới).
- **Đã quyết 2026-11-16** (phiên lead theo uỷ quyền của chủ dự án trong chat): QĐ-E2M = B (#59),
  QĐ-4 = A và QĐ-5 = có ([ADR-0005](../decisions/ADR-0005-live-population-turnover.md)), QĐ-LOC = A
  ([ADR-0006](../decisions/ADR-0006-locomotion-actuation.md)), QĐ-ECO = A
  ([ADR-0007](../decisions/ADR-0007-live-ecology-sustainability.md)).
- **Đã quyết 2026-11-17** (phiên lead theo uỷ quyền "làm theo bản kế hoạch"): QĐ-ECO-B = opt-in (thẻ B của
  [ADR-0007](../decisions/ADR-0007-live-ecology-sustainability.md), #87; chỉ cho thăm dò S43, không mặc định).
  QĐ-SEL1b và thẻ C1 của ADR-0007, lúc đó còn trống, được chủ dự án quyết cùng ngày (dòng dưới).
- **Đã quyết 2026-11-17** (phiên lead, cùng uỷ quyền "theo bản kế hoạch"): **QĐ-2/QĐ-H** theo đề xuất của
  [kế hoạch swarm](../ai/planning/2026-11-05-swarm-master-plan.md) §2: gỡ mọi bằng chứng và dữ liệu giả ngay
  (G4, G6), giữ mã; Map 2 và Research Dashboard có ship hay không thì quyết sau. Chủ dự án đảo lại được.
- **Chủ dự án quyết trực tiếp 2026-11-17, trong chat:** xác nhận mọi quyết định mà phiên lead đã ra theo
  uỷ quyền ở trên; QĐ-SEL1b = A; QĐ-ECO-C1 = có; QĐ-EN = E2R và E3; F-044 = tách bộ đếm; QĐ-C = C (nâng
  `burn`); QĐ-1 = mức 2; QĐ-PAST, QĐ-TIME và QĐ-LAW1. Bản ghi đầy đủ:
  [roadmap 31/10 §4.1–§4.3](../planning/COMPLETION_ROADMAP_2026-10-31.md#4-quyết-định-chỉ-chủ-dự-án-đưa-ra-được).
  Thẻ sinh ra từ đó: [kế hoạch swarm, "Thẻ thêm ngày 2026-11-17"](../ai/planning/2026-11-05-swarm-master-plan.md#thẻ-thêm-ngày-2026-11-17-từ-quyết-định-của-chủ-dự-án).
- **Hai máy từ 2026-11-17, giống OxideSwarm:** `agent-mac` (Claude Code trên Mac) làm G6 → G4 → G9 và phần
  còn lại của G8; `agent-win` (Claude Code trên Windows, cùng nền với job Rust của CI) làm G5 → G3, rồi **R1c** (giao
  ngày 2026-11-17, [bản giao việc](../ai/planning/2026-11-17-agent-win-handoff-r1c.md)), rồi R0
  khi chủ dự án nói Đợt 2 đã xong. **Từ 2026-11-18 vai `agent-win` do Gemini chạy** (QD-WIN), làm R0 trước rồi R1c
  ([bản giao việc cho Gemini](../ai/planning/2026-11-18-gemini-worker-win-handoff.md)); Gemini không mở được verifier
  riêng, nên `agent-mac` kiểm lại PR của nó trước khi PR được đánh dấu sẵn sàng. Từ chiều 2026-11-18 thẻ được giao qua thư mục Drive `Anima_Agent_Hub` (QD-HUB, luật bản 4: agent không bao giờ merge; từ tối cùng ngày phiên lead tự merge PR nó đã kiểm, QD-LEAD). `agent-mac` cùng ngày nhận S10. Nhánh `mac/…` và `win/…`; nhận thẻ bằng một draft PR; mỗi máy chỉ sửa
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

### 2026-11-19 → 2026-11-23 (đối chiếu 2026-11-23)

Nguồn: `gh pr list --state all`, `git branch -r` và các verdict của lead ngày 2026-11-23. "Đã thẩm định"
nghĩa là lead đã kiểm lại ở đúng commit đầu PR và PR chờ merge.

| Thẻ | Máy | PR | Trạng thái |
|---|---|---|---|
| SEL2-P-impl / -fix, E3-P, LAND1 | mac / case / dell | #152, #165, #164, #166 | ✅ |
| E3-X (impl, run) | case | #178, #181 | ✅ `insufficient_evidence` |
| F051 / F051b, FIX-CAP / FIX-CAP2 | mac | #174 / #179, #177 / #180 | ✅ |
| SEL2P2-PREP, FOUNDER-LAYOUT | mac | #182, #186 | ✅ |
| WV2-D, WV2-1a/b/c | lead, mac | #192, #193–#195 | ✅ |
| Gate rỗng + F-054, F-061, RUSTSEC url | mac, mac, lead | #197, #198, #203 | ✅ |
| Quyết định 2026-11-23; sửa CLAUDE.md/README | lead | #212, #213 | ✅ |
| VIAB-1 | case | #215 | 🔄 đã thẩm định, chờ merge |
| E3B-P, E3B-PWR (nháp đăng ký E3b) | case | #217, #218 | 🔄 đã thẩm định; chờ chủ dự án quyết D1–D5 |
| NR-SPIKE-PROTOCOL (chỉ tài liệu) | case | — | 🔄 |
| F-066 (phả hệ live) | lead | #207 | 🔄 đã thẩm định sau vòng sửa ADR-0008, chờ merge |
| P1 (danh mục sự cố) | mac | #211 | 🔄 đã thẩm định, chờ merge |
| DOC1, DOC2, DOC3, DOC5+6 | mac | #204, #205, #202, #201 | 🔄 đã thẩm định, chờ merge |
| DOC7 (đề xuất), DOC8 (gate lệnh docs) | mac | #209, #208 | 🔄 sửa vòng 3 |
| M2 (đối chứng âm) | mac (dell #188 sẽ đóng) | #196 | 🔄 sửa vòng 3 |
| R5 (F-053, F-055…F-065) | mac | #200 | 🔄 sửa theo verdict (F-064 bảng Sobol) |
| Gate dấu xung đột | mac | #210 | 🔄 sửa theo verdict |
| WV2-2 | mac | #199 | ⛔ làm lại: thác nước bậc thang toàn cục |
| Mẫu thống kê đăng ký; chẩn đoán mut0 | lead | #214, #216 | 🔄 chờ merge |
| D6 F053-FIX, D7 F054-FIX (giao dell) | — | — | chuyển cho mac, đã nằm ở #200 / #197 |

### Đợt 1 — đã đóng

| Thẻ | Việc | Trạng thái |
|---|---|---|
| **D0** | Kiểm kê PR mở và nhánh remote | ✅ #42 |
| **D1** | Phân loại finding của `AUDIT_REPORT.md` | ✅ #43 |
| **T1** `[RUST]` | **B3b** — năm gate mà PR #40 làm đỏ | ✅ #44 |
| **T2** `[RUST]` | **HT-C** — gộp chồng PR vào thân cây | ✅ #45 (2026-11-15) |
| **P1–P3** | Bài báo | ⬜ QĐ-P1, QĐ-P2 và QĐ-P3 đã quyết ngày 2026-11-17, nên không còn bị chặn. Bản nháp viết ở repo riêng tư; thẻ nào ghi vào repo này chỉ được ghi dữ liệu và nhật ký dùng AI |

### Đợt 2 — đã merge hết ngày 2026-11-18 (chặng 3 chỉ còn R0)

| Thẻ | Việc | Trạng thái | Máy | Ghi chú |
|---|---|---|---|---|
| **G1** `[RUST]` | **B2** — zero-alloc ở đường sinh/tử | ✅ #48 | — | |
| **G2** | **B5** — `test:e2e` đủ bộ, gate danh sách spec | ✅ #47 | — | |
| **G3** `[RUST]` | **B6a** — giá trị bịa trong telemetry và provenance | ✅ #107 (2026-11-18), CI chạy lại xanh trên thân cây đã có S6 và S8 | — | |
| **G4** | **B6b** — lệnh IPC không tồn tại, dữ liệu giả trên dashboard | ✅ #98 (2026-11-17) | — | QĐ-2/QĐ-H quyết 2026-11-17 |
| **G5** `[RUST]` | **B6c** — bypass sổ cái, nghi rò EU của `PathogenTransmission` | ✅ #95 (2026-11-17) | — | |
| **G6** | Gỡ bằng chứng giả của Map 2 | ✅ #94 (2026-11-17) | — | |
| **G7** `[RUST]` | Khôi phục assertion bị nới và test bị xoá | ✅ #49 | — | |
| **G8** | Hợp đồng IPC | ✅ phần lớn, 2026-11-14 | `agent-mac` | Có [`IPC_CONTRACT.md`](../reference/IPC_CONTRACT.md) và gate `scripts/check_ipc_reference.mjs`. Còn thiếu so với thẻ: `file:line` cho từng mục |
| **G9** | Gate vệ sinh thư mục gốc | ✅ #114 (2026-11-18), dựng lại trên thân cây: allow-list bỏ `TASK.md` và `AUDIT_REPORT.md`, thêm `CITATION.cff` | — | Cổng mới: `npm run check:root-hygiene`. Sáu mục còn ghi `REVIEW:` chờ chủ dự án quyết giữ hay bỏ |
| **R0** `[RUST]` | Đo lại toàn bộ gate của STATE §5, soạn bản vá §1 | ✅ #123 (2026-11-18): mọi cổng xanh tại `46e25d5` trừ một báo động giả CRLF trên Windows (sửa ở #125); số ở STATE §1.i | `agent-win` (Gemini, qua hub) | Cổng ra của chặng 3: **đã qua**. Phiên điều phối kiểm chéo số test với CI Windows của PR #122 |

### Đợt 3 trở đi

Hai thẻ của Đợt 3 đã merge **trước** R0, theo quyết định của phiên lead ngày 2026-11-16 (§1): **S1**
(#66, `live.reproduction`) và **S4** (#75, OSS-072 MRCA). STATE §1.h đã đo một commit có cả hai; R0 vẫn
là cổng ra của chặng 3 và chạy sau khi Đợt 2 merge.
**T3** (mở PR thân cây → `main`) đã được thay bằng cách làm thực tế: `main` được fast-forward lên thân cây
ở các mốc. **M1** (ma trận tái lập đa nền tảng) cũng đã merge trước R0, ở #100 và #102. Các thẻ còn lại
của Đợt 3 (S2, S3, M2–M4) và mọi thẻ từ Đợt 4 chưa bắt đầu.

Mười thẻ được thêm ngày 2026-11-17 từ quyết định của chủ dự án (S6, S7, S8, SEL2-P, SEL2-X, R1c, L0, L1,
L2-P, L2-X). Ba thẻ đã chạy ngay trong ngày: **S8** ✅ #106 (F-044), **S6** ✅ #110 (khoá `live.prey_cap` và
`live.resource_capacity_scale`, kèm [lượt đo thăm dò](../ai/planning/2026-11-17-s6-sel2-exploration.md)),
và **L0** ✅ [ADR-0008](../decisions/ADR-0008-world-past-and-time.md) soạn ở #109 và được chủ dự án chấp nhận ngày 2026-11-18, kèm năm câu trả lời và
quy tắc kỷ engine; các bước của nó là năm thẻ WR1–WR5 chưa bắt đầu. **R1c** (nâng `burn`) được giao cho `agent-win` cùng ngày:
[bản giao việc](../ai/planning/2026-11-17-agent-win-handoff-r1c.md) giữ prompt để dán và dòng uỷ quyền đã
soạn; thẻ chưa có PR. Năm thẻ còn lại chưa bắt đầu. Lượt đo của S6 **không** tìm ra cấu hình khan hiếm thức
ăn nào khả thi, nên SEL2-P cần một quyết định của chủ dự án trước khi chạy. Quyết định đó có ngày 2026-11-18 (QĐ-SEL2a–d,
[đề xuất làm lại SEL2](../ai/planning/2026-11-18-sel2-redesign-proposal.md)): bốn thẻ mới **S9** (tách nguyên nhân
chết), **S10** (khoá `live.plant_reserve`), **SEL2-E** (đo thăm dò lần hai) và **SEL2-D** (đối chứng drift).
**S9** ✅ #117 ([kết quả](../ai/planning/2026-11-18-s9-death-causes.md)): khát gây 71–80 % số cái chết ở cả bốn ô
đã đo, nên điều kiện 4 của SEL2-E (chết chủ yếu vì đói) chưa đạt ở ô nào; cách nới (thêm hồ hay nới ân hạn khát)
là việc chủ dự án quyết trước SEL2-E; chủ dự án chọn thêm hồ (QĐ-SEL2e). **S10** ✅ #124
([ghi chú](../ai/planning/2026-11-18-s10-plant-reserve.md)). **SEL2-E** ✅ #128, **kết quả âm**
([kết quả](../ai/planning/2026-11-18-sel2e-exploration.md)): không ô nào khả thi; ba phép đo tiếp theo cho thấy khi
khan hiếm, quần thể dao động bùng nổ và sụp đổ rồi tuyệt chủng vì sinh sản dừng ở mật độ thấp. SEL2-P chờ chủ dự án
định hướng. **SEL2-D** ✅ #130. Phép chẩn đoán đáy chu kỳ (#131) cho thấy con vật không bao giờ lái tới đồng cỏ, nên
thẻ mới **S11** (lái tới cỏ, opt-in) và thẻ sửa **F045** (khôi phục checkpoint) được giao cho `anima-mac` qua hub.
**S11** kết thúc là một phát hiện, mã không merge (PR #134 đóng, nhánh `dell/s11-grass-steering` giữ ở `95044c4b`;
[kết quả](../ai/planning/2026-11-18-s11-grass-steering.md)): khi bỏ khát, con mồi không thiếu ăn, và lái tới cỏ không
tăng lượng ăn. Thứ giới hạn là khát (mọi hồ nằm trên một hàng), nên thẻ kế tiếp là **S12** (`live.lake_layout`, rải hồ
trên lưới hai chiều), giao cho `anima-dell`. **R1c** ✅ #127 (2026-11-18, `burn` 0.22.1, schema 8,
`LIVE_MODEL_VERSION` `/2`); điều kiện giấy phép còn lại của nó là QD-C2. Sau R1c, `anima-case` làm **M4** (ma trận
dư EU) rồi **M3** (đường cong scaling): **M3** ✅ #138; **M4** ✅ #137, vòng đầu bị trả về vì ma trận không có lượt nào có chết hay săn mồi
(verdict M4_064bc43c trên hub), vòng hai đạt. **S12** ✅ #135. Hàng việc từ 2026-11-19: `anima-dell` S13 (đo 200.000 tick
trên hồ đã rải) rồi M2; `anima-case` S2 (observable tích luỹ cho SEL2-P); `anima-mac` làm tiếp F045 sau khi dừng
khoảng 7 giờ ngày 2026-11-18 (merge thân cây vào nhánh, không rebase): **F045** ✅ #140, rồi F046 (chẩn đoán, chỉ đọc).
**S13** ✅ #139 cho kết quả âm (0/12 ô khả thi); **S14** ✅ #142 và **S15** ✅ #145 dẫn tới QD-SEL2g (chuẩn gộp). **F046** (chẩn đoán), **F047** và **F048** ✅ #146 (khôi phục suy luận đang chờ ở app desktop). **S2** ✅ #148. Bản nháp **SEL2-P** ✅ #147. Hàng việc sáng 2026-11-19: `anima-mac` SEL2-P-impl, `anima-case` E3-P, `anima-dell` M2. Định nghĩa các thẻ ở
[kế hoạch swarm](../ai/planning/2026-11-05-swarm-master-plan.md#thẻ-thêm-ngày-2026-11-17-từ-quyết-định-của-chủ-dự-án).

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
QD-C      = C: upgrade burn from 0.13.2 to the 0.22 line so hexf-parse leaves the dependency tree and
            the GPU learner stays; neither A nor B. Card R1c replaces R1 (owner in chat, 2026-11-17)
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
QD-SEL1b  = A: a prey reproduction cap key, then pasture scarcity; an exploratory measurement before
            any new preregistration. Cards S6, SEL2-P, SEL2-X (owner in chat, 2026-11-17)
QD-R1c    = card R1c bumps SCHEMA_VERSION to 8: a schema-7 snapshot restores the world, its two learner
            records are refused with a named error and a warning, and the shared learner starts fresh;
            and R1c moves to burn-flex, not burn-ndarray 0.22 (owner in chat, 2026-11-18;
            docs/ai/planning/2026-11-18-research-burn-and-determinism.md)
QD-SEL2   = the next step of QD-SEL1b is cards S9 -> S10 -> SEL2-E -> SEL2-P, plus SEL2-D; SEL2-E's
            viability conditions add ">= 30 generations" and "deaths mostly by starvation"; SEL2-P's
            seed count is decided after SEL2-E (owner in chat, 2026-11-18; docs/ai/planning/
            2026-11-18-sel2-redesign-proposal.md)
QD-SEL2e  = after S9 (#117: thirst caused 71-80% of deaths), SEL2-E relieves water by adding lakes:
            live.lakes in {2, 4, 8}; the dehydration grace stays at its default (owner in chat,
            2026-11-18, "Thêm hồ")
QD-SEL2g  = SEL2's standing-crop ratio g uses a POOLED reference: the mean second-half crop of the
            prey_cap-80 / scale-1.0 baseline runs that survive, over a baseline seed set declared
            before the run (at least 3 survivors, else C2 fails); every scarce seed uses that one value
            (owner in chat, 2026-11-19: "chọn chuẩn gộp (b)"; verdicts S14_4e6d312f, S15_4342b895;
            docs/ai/planning/2026-11-19-sel2-p-preregistration-draft.md section 6)
QD-VIS    = the 3D map must BE the simulated world, semi-realistic, with mountains, caves, forests and
            rivers, streamed by camera; creatures are continuous bodies grown from their morphology;
            done in parallel on Windows 1 (owner in chat, 2026-11-19). Follow-up "oke theo ý bạn" taken
            as: accept ADR-0010 (pinned, tiled world artifact), skip CREATURE_MORPHOGENESIS_PLAN.md:36
            for the body renderer, keep 30 FPS at Low on Iris Xe with 1,000 creatures (cap near-tier
            bodies). docs/ai/design/2026-11-19-feature-real-world-and-bodies.md
QD-NATIVE = a native renderer (wgpu / Bevy, in the same process as the simulation, native surface in the
            Tauri window, HTML UI kept) is the long-term direction because it removes per-tick IPC and
            gives the full GPU; it is done later, not now. Near-term work stays on Three.js/WebGL2 and
            prefers renderer-agnostic pieces (world artifact, payload, body-mesh algorithm). The
            measuring spike and ADR-0011 come when that work starts (owner in chat, 2026-11-19: "Làm
            spike", then "tôi nghĩ vẫn sẽ làm vẽ native vì sẽ tối ưu hơn oke nhưng sẽ làm ở tương lai")
QD-SEL2P  = the SEL2-P preregistration is approved (owner in chat, 2026-11-19 ~12:05 +07: "Duyệt"), on the
            lead's stated conditions: the smoke (seed 999961, 200,000 ticks, blind) runs now with the
            preregistered commands; the freeze and the 20 confirmation seeds wait until card SEL2-P-fix
            (verdict SEL2-P-impl_6456eb4a, defects D1-D3 at least) has merged and the smoke passes C1..C5
QD-SEL2P2 = the SEL2-P smoke failed its preregistered gate (founder boom and crash at prey_cap 320, confirmed
            independently; verdict SEL2-P-smoke_296475cc), so SEL2-P stops at the smoke as registered. M0 continues
            as: a read-only diagnosis (card DIAG-SEL2), a measurement-only exploration, then a new preregistration
            SEL2-P2 with a new smoke seed that discloses this history (owner in chat, 2026-11-19: "Chẩn đoán rồi
            đăng ký lại")
QD-MACSHARE = the Mac is shared with OxideSwarm: each project may run one cargo process at the same time; an
            Anima cargo there runs as CARGO_BUILD_JOBS=4 nice -n 10, only with at least 30% memory and 20 GB
            disk free, and never touches an OxideSwarm process (owner in chat, 2026-11-19: "Xếp lịch xen kẽ
            song song đi"; hub AGENT_RULES §5). The Dell does NOT share: after it ran out of RAM and shut down on
            2026-11-20 while building in parallel, an Anima cargo runs there only when no other cargo or rustc runs
            on the machine
QD-E3HOST = Windows 1 is the single E3-X host (owner in chat, 2026-11-20: "Win 1 (sau W11)"); E3-X ran there at
            24c6fb1c and ended at insufficient_evidence (6/12 valid pairs), merged in #181
QD-VIAB   = after E3 (insufficient_evidence) and SEL2-P (smoke failed) hit the same collapse of the scarce arms, one
            shared measurement-only viability exploration (VIAB-1) comes first, then new preregistrations E3b and
            SEL2-P2 with fresh seeds, designed from survival and gate metrics only, never from body mass (owner in
            chat, 2026-11-20: "Khảo sát chung rồi đăng ký lại cả hai")
QD-MAPV2  = the map aims for the most realistic world possible, over a long horizon (owner in chat, 2026-11-20:
            "Tôi muốn làm tốt nhất có thể, làm trong thời gian dài cx đc"). The plan is World v2: an offline,
            deterministic Rust generator with real erosion, hydrology and climate, written as a pinned artifact. Then a
            native-renderer spike and ADR-0011, then the renderer milestones R1-R6. Each phase has a gate (G-REF, G-SHAPE,
            G-COMP, G-PERF, G-SIM). The Three.js viewer stays as the fallback until parity. This LIFTS the graphics
            freeze in QD-SCOPE; science continues under QD-VIAB on other machines. "Light and beautiful,
            optimized" ("vừa nhẹ vừa đẹp, tối ưu", owner, 2026-11-20) is a hard requirement. Every system has a
            per-frame millisecond budget on Iris Xe (Low) and RTX (High), measured by GPU timers in the desktop
            app (plan §3b). When two options both pass G-REF, the lighter one wins.
            Heavy, camera-independent work is baked offline on the OxideSwarm grid: World v2 at high resolution,
            static lighting, impostors, mesh LODs, texture compression, tile pyramids and vegetation placement.
            Viewers only stream and draw (plan §3c; owner, 2026-11-20: "để các máy khác gánh").
            docs/ai/requirements/2026-11-20-feature-map-v2.md
            AMENDED 2026-11-20 (owner in chat: "oke đồng ý cho bạn tự làm"): the World v2 design is accepted
            (card WV2-D, four independent review rounds), with the lead decisions LD-1 to LD-5: one regional
            physical scale, 40.96 km at one fixed latitude, creatures drawn at a declared stylised size (LD-1);
            sim_world_identity = sha256(sim_256.anmw3) pinned by experiments and saves, bundle_identity =
            sha256(index.json) for viewer tiles, a viewer re-bake never changes the former (LD-2); sim_256 and
            index.json committed, tile packs a release asset fetched at build time only, never at runtime (LD-3);
            the desktop switchover card WV2-7 lands before or with the new viewer WV2-6, and a v3 file is an
            explicit error until then (LD-4); the setting is a tropical coastal massif modelled on the Sierra
            Nevada de Santa Marta, with biome variety from altitude and rain shadow (LD-5).
            docs/ai/design/2026-11-20-world-v2-design.md
QD-ASSETS = CC0 art for the map: ambientCG and Poly Haven only, licence CC0-1.0, at most 60 MB
            downloaded and 20 MB committed under public/assets/world/ with SOURCES.json (one licence_url
            per entry) and a NOTICE section generated from it; KTX-Software (toktx) and gltf-transform
            allowed as local tools, never committed (owner in chat, 2026-11-19: "Cho tải CC0", "Đồng ý cả
            hai"). The graphics pass W3-lite runs on Windows 1 after B1 ("Win 1 sau B1"). CORRECTED the
            same day: Quaternius was first listed here as CC0, but its licence page now says Quaternius
            Asset License v1.0 (updated 2026-08-28; no redistribution as standalone assets or packs, which
            a public repository would risk), so it is not covered by the owner's CC0-only grant
QD-OXS    = Anima work may run on the OxideSwarm grid (owner in chat, 2026-11-19: "Cho phép dùng
            OxideSwarm chạy việc của Anima"), through the existing grid procedure (only anima-mac submits,
            only to GRID WORKER, never to friends' machines): Rust builds and tests, a registered
            experiment's preregistered runs (all confirmation seeds of one experiment on one OS family),
            and offline map jobs whose output is a pinned, hashed artifact. Runtime rendering stays on the
            viewer's own machine. Anima agents still never open, build or edit the OxideSwarm checkout
QD-TRIAL  = the owner's hands-on trial of map + bodies + experiment worlds runs on Windows 1; the lead
            pauses anima-case for it (owner in chat, 2026-11-19)
QD-DOCIDX = build a generated docs index so pull requests stop conflicting on docs/ai/README.md (owner in
            chat, 2026-11-19: "Làm"); until it merges, the lead keeps resolving those rows by hand
QD-ISLAND = the project's flagship scientific question is the island rule's mechanism (resource
            limitation vs predation release), tested with preregistered multi-world archipelagos; SEL2-P
            is its first brick; island size = carrying capacity, not area (owner in chat, 2026-11-19:
            "Nhận", "Sức chứa"; docs/ai/requirements/2026-11-19-feature-island-evolution-observatory.md)
QD-SCOPE  = scope freeze for about 3 months from 2026-11-19: continue SEL2-P, E3-P, M2, LAND1 and the
            trial cards; paused: native renderer, exotic-energy extensions, W4 streaming, the full tiled
            artifact (W1-W3 full), Inhabit federation; after the trial, graphics = bug fixes only. Public
            docs and papers in English; an external reviewer before the island preregistration (owner in
            chat, 2026-11-19: "Đồng ý", "Cả hai"). AMENDED 2026-11-20 by QD-MAPV2: the native renderer, the full
            tiled world artifact and graphics work are no longer paused; they follow the Map v2 phases.
            AMENDED 2026-11-24 by QD-SHAREDISLAND: Inhabit federation is no longer paused; it is the main goal
QD-REALMS = real ecosystems in water, sky and underground are added one at a time after the SEL2-P
            result, each with its own ADR: water first, then sky, then underground (walk-in caves);
            until then fish, birds, wildlife and caves stay labelled decoration (owner in chat,
            2026-11-19: "Theo từng bậc")
QD-WIN    = the agent-win role is run by Gemini on the owner's Windows machine from 2026-11-18; R0
            first, then R1c (owner in chat; docs/ai/planning/2026-11-18-gemini-worker-win-handoff.md)
QD-HUB    = Gemini workers (anima-case = agent-win on Windows, anima-mac on the Mac) take cards through
            the Drive folder Anima_Agent_Hub, rules version 4: agents never merge, the coordinating
            session verifies, the owner merges (owner in chat, 2026-11-18: "Giữ không tự merge, sửa hub
            đi")
QD-LEAD   = the coordinating (lead) session may itself merge a pull request it has verified, once
            both required checks are green at the verified head; worker agents still never merge (owner
            in chat, 2026-11-18: "Bạn đc phép tự merge và hoàn thiện dự án")
QD-OFFQ   = work done on 2026-11-21..22 without a queue slot (DOC1-DOC8, P1 incidents, the conflict-marker
            gate, the R5 findings fixes F-055..F-065 and F053-FIX) is kept and finished through its
            open draft PRs; DOC8 (#208) is rewritten to the card's size (extract marked blocks, run
            them, fail on zero blocks and on "running 0 tests", report doc:line). M2 keeps #196 as
            the base; the owner closes #188 after #196 merges. No new PR or branch from anima-mac
            until the lead has verdicts on its open drafts (owner in chat, 2026-11-23: "Duyệt theo
            đề xuất", lead review of 2026-11-23)
QD-DOCLANG= QD-SCOPE's "public docs in English" stands: tutorials, how-to and explanation pages are
            written in English. A Vietnamese version may be kept next to the English one as a
            translation, never instead of it (owner in chat, 2026-11-23: "Duyệt theo đề xuất")
QD-VIAB1C = VIAB-1 continues as registered, without a re-run after the F-055/F-057 fixes, provided no
            VIAB-1 family enables live.grass_steering and live.land_mask together (the only
            combination F-055 changes). If one does, that family stops and the lead decides (owner in
            chat, 2026-11-23: "Duyệt theo đề xuất")
QD-E3B    = E3b preregistration decisions D1-D5, from the lead's recommendation after an independent
            review of four research reports and checks against VIAB-1 (#215):
            - D1: keep live.predator_fraction = 0.3 (the calibrated cell), disclosed as effectively
              herbivore-only;
            - D2: directional H1 T > S with the 4-label R10 table;
            - D3: complete-pair analysis is primary for T - S; contrasts involving U add Lee bounds and
              survival (RMST) as a separate endpoint;
            - D4: the per-seed log variance ratio ln[(var_T+e)/(var_S+e)], e = 5e-4, SESOI +/-ln(1.5),
              N = 64;
            - D5: a declared 2-segment founder, the genesis genotype minus its tail leaf, behind the
              opt-in key live.founder_genotype.
            E3b covers about 4.0 / 1.9 / 4.0 generations (U/S/T) in 18k ticks, which is why D5 removes
            the 3.3 -> 2.5 founder sweep. (Owner in chat, 2026-11-23: "Duyệt theo đề xuất".)
QD-PLAYRULES = rules of play on a shared island (QD-SHAREDISLAND): (1) one creature's lifespan is about 1-2 hours
            of real time, slow enough to watch hunting and growing up, through a declared real-time ecology
            profile pinned in the manifest (science runs keep their own profiles; faster than real time only
            with no guest present); (2) animals reproduce sexually (two sexes, mates found by proximity and
            signals, recombination of homologous parts plus mutation, no offspring from an incompatible pair,
            compatibility falling with genetic distance so reproductive isolation can emerge, "species" only
            for isolated clusters passing AE-S11/AE-S14); plants are hermaphrodite with outcrossing, selfing at
            an inbreeding cost and clonal grass spread; (3) an inhabited organism that dies is not respawned:
            its carcass decomposes and the guest may inhabit an offspring, a young relative or another allowed
            organism; (4) if all life dies, the host opens a new era; (5) a guest controls one organism with no
            powers beyond that organism's own, environment interventions are host-only, and the host may remove
            a guest; (6) built for a small LAN group now, with foundations for more (stable IDs, interest
            management, record compaction). Points (2) and (3) were delegated to the lead as "the most
            biologically correct". (Owner in chat, 2026-11-24.)
QD-SIGNALS = a language system means creatures communicating: evolved signalling, not scripted meanings, over
            chemical (the existing pheromone grid), acoustic and visual channels, with an energy cost through
            total_cost and eavesdropping by predators; "language" is claimed only with evidence (information
            carried, receivers' response, shuffled-signal controls); a guest speaks only through its organism's
            channels, and human chat is out of world. ADR-0017 (proposed). (Owner in chat, 2026-11-24: "hệ thống
            ngôn ngữ", "Sinh vật giao tiếp với nhau".)
QD-ECOSCALE = the ecosystem sustains itself through cycles, not caps: energy from sunlight flows up the food chain
            under the closed-EU contract, detritus and carcasses return nutrients through decomposers (V4, V5),
            and populations are regulated by food, predation and space (density dependence); live.prey_cap
            becomes a legacy option, off on shared islands once that works. Organisms of very different sizes
            use three tiers: individuals (medium and large animals, trees, shrubs; inhabitable), swarms or
            super-individuals (a mosquito swarm, a fish school), and per-cell population fields (grass,
            plankton, soil fauna, larvae). Pace scales with size (Kleiber); QD-PLAYRULES' 1-2 h lifespan is for
            the typical inhabitable animal. Milestone 1 inhabits individuals only. ADR-0018 (proposed); numbers
            stay D-slots. (Owner in chat, 2026-11-24: "oke đồng ý".)
QD-WV25   = the three World v2 WV2-5 gates, answered in advance: (a) staging CC0 source assets for grid bake
            workers is allowed within QD-ASSETS (ambientCG and Poly Haven only, CC0-1.0 only, at most 60 MB
            downloaded and 20 MB committed, SOURCES.json with one licence_url per entry, GRID WORKER jobs under
            QD-OXS); (b) the baked tile pack is not published now: it stays an internal artifact and is published
            as a GitHub Release asset with v0.9.0 (QD-REL09 G3); (c) the lead may approve a new crate with a
            permissive licence (MIT, Apache-2.0, BSD-2/3, ISC, Zlib, or a dual licence containing one), given its
            licence, transitive dependency list, a clean cargo audit and the reason; any other licence goes to
            the owner. (Owner in chat, 2026-11-24, choosing the recommended option for each gate.)
QD-REL09  = release 0.9 gates, all as anima-dell recommended in REL09-DOCS: G1 (B) a release candidate
            v0.9.0-rc.1 first, for the three MP-5.1 trial sessions, then v0.9.0 after fixes; G2 (A) unsigned
            installers, with the SmartScreen / Gatekeeper steps of docs/how-to/INSTALL.md (QD-M8 signing stays
            a later decision); G3 (C) the rc goes to trial users as release-build workflow artifacts only, and
            v0.9.0 is then published as a GitHub Release; G4 (A) main is fast-forwarded to the release commit
            and the tag is set on main (the owner pushes main and the tag; agents never do). Timing follows
            QD-TRIAL-V2: the rc is cut after WV2-7 lands. (Owner in chat, 2026-11-24, choosing the
            recommended option for each of G1-G4.)
QD-ADR0014 = ADR-0014 (shared island, host and guests; PR #256) is accepted with all 24 D-slots as recommended:
            immediate release on disconnect; exclusive possession; bincode quantized deltas at about 20 Hz with a
            LAN cap of 16 guests; interest-area culling with a low-rate global summary; guest-input overflow
            rejected at the socket; LAN discovery by IP:port + PIN and a UDP beacon; adaptive 2-3 tick playout,
            no client prediction; third-person default camera, sensory first person after milestone 1, free
            spectator camera; grass cells watch-only; one day = 20 minutes and one year = 90 minutes of real
            time; World v2 C01 terrain after WV2-7; a headless prehistory before guests; persistent islands;
            G2 producers-first by a closed-EU viability trigger with at least 3 founder pools; the G3 spike
            time-boxed to 2 weeks (success: an emergent lineage passes the G2 screen); the listed chronicle
            triggers, extinct-branch compaction, clade threshold and naming proposed in the ADR and approved
            by the lead; opt-in OpenRouter :free narration through a provider-agnostic endpoint with the key in
            an environment variable or keychain. The Internet milestone (ISLAND-D7) stays open. (Owner in chat,
            2026-11-24: "Duyệt theo khuyến nghị".)
QD-SHAREDISLAND = the project's main goal is now many machines in one island: one host machine runs the
            simulation and holds the island's truth; guests on other machines watch (Spectate) or inhabit
            (Inhabit) over the network; every guest action reaches the host as a declared intervention with
            a CauseId (ADR-0004) and is recorded in the world record (ADR-0008, WR1/WR2). Only the host
            simulates, so cross-OS checksum differences do not matter; lockstep co-simulation of one island
            stays rejected (direction plan 13/11, chặng 7). First milestone: LAN only (no Internet until
            authentication and TLS exist), guests watch and inhabit one organism, design first (ADR-0014)
            then implementation cards; the other lanes keep running and S7 is deferred, not cancelled. The
            design proposals for animation (the pose is the simulation; guests send intents, never poses),
            camera (third person by default, a "sensory" first person later), island start (pinned terrain,
            a prehistory run before guests arrive, a persistent island) and an island chronicle (facts from
            the record and the lineage tracker, "lineage" not "species" without AE-S11, optional AI-written
            prose on the host with the owner's own API key, never recorded) are D-slots in ADR-0014, not yet
            decided. AMENDS QD-SCOPE: "Inhabit federation" is no longer paused; it is the main goal. Not to be
            confused with QD-ISLAND (2026-11-19, the island-rule science question), which is unchanged.
            (Owner in chat, 2026-11-24: "cái này á", then "Đúng, làm tiếp".)
QD-LIFE   = to be like the real world, evolution and growing up apply to both animals and plants, and
            inhabiting applies to both: animals and individual plants (trees, shrubs; a grass cell is a
            population and is only watched). Animals get physical growth toward their genotype's adult size
            (amendment to ADR-0013, which today keeps body size fixed from birth); plants become evolving
            organisms (ADR-0015: individual trees and shrubs, grass as per-cell populations carrying a trait
            distribution; energy from sunlight under the closed-EU contract; opt-in, legacy path
            bit-identical). The map changes too (ADR-0016), by timescale: the living surface first, then
            recorded local disturbances as delta overlays, then slow landscape change and large events
            (earthquake, eruption, new land) as era transitions re-baked by the World v2 generator; every
            change is a process law plus a trigger (a seeded natural-event process in the world law, or a
            CauseId intervention) plus a scale rule. (Owner in chat, 2026-11-24: "tiến hoá và phát triển
            lớn lên cần phải áp dụng với cả động vật và thực vật", "đóng vai áp dụng cả với thực vật", "sự
            thay đổi cx cần áp dụng với map".)
QD-GENESIS = first organisms in three phases: G1 fixed, declared founders for the demo (today's 2-segment
            animal founder with the assigned predator fraction; one fixed grass, shrub and tree genotype
            once ADR-0015 lands), labelled "designed founders"; G2 a screened founder pool sampled from a
            declared distribution on its own seeded stream and kept only if it passes a viability screen
            (VIAB-1/VIAB-2 style), producers first, then herbivores, then predators, with a robustness check
            across pools; G3 emergent first organisms from a primordial world law, as a time-boxed research
            spike with its own ADR and an explicit statement that chemistry-level origin of life is not
            modelled. Every run declares its phase in the genesis manifest; experiments never mix phases.
            (Owner in chat, 2026-11-24: "hướng 1 đặt 1 fix cứng để làm được demo đã, tiếp theo sẽ sàng lọc
            1 số để lấy mẫu nâng cấp, cuối cùng làm thật sự sinh vật khởi đầu".)
MAP-D4    = World v2 WV2-4 official artifact (PR #249): no pre-registered candidate passes the 15 gates at
            4096^2 (C01 passes 2/15, C08 1/15); the root cause is a truncated channel network
            (omega_max = 3), which cascades into zero waterfalls, lake area 11-31x below the floor, a
            smooth cliff-less coast and ~4,600 needle spikes at 10 m. Decision: Options 3 + 1. Commit C01
            (ranked first by the pre-registered rule) as a provisional world under artifacts/world-v2/
            with gates.json recording all 15 gates as measured, "status": "provisional"; gates and
            thresholds stay defined and unchanged; tests asserting an unmet gate on the official artifact
            are #[ignore]d by name with a MAP-D4 reason in scripts/test_target_policy.mjs. The physics
            rework (channel initiation and runoff, knickpoint waterfalls, lakes, coastal cliffs, spike
            smoothing) is a later card WV2-PHYS2. (Owner in chat, 2026-11-24: "Tạm dùng C01 + sửa vật
            lý sau".)
QD-VIAB2-R = SEL2-P2 VIAB-2 needs a blind 12-seed x 200,000-tick runner that the trunk lacks (the SEL2-P
            runner rejects drift_reset_ticks, runs one seed and emits trait_mass). It is written as one new
            example src-tauri/examples/run_viab2_probe.rs with its own test, on the trunk's
            run_live_drift_pair path, blind by construction; run_sel2p_experiment.rs and src-tauri/src stay
            unchanged. (Owner in chat, 2026-11-24: "Cho viết một file example mới".)
QD-SEL2P2-D1..D7 = SEL2-P2 owner decisions on the draft of #247, all as the author recommended (owner in
            chat, 2026-11-24: "duyệt theo khuyến nghị"): D1 (a) live.founder_genotype = 1.0 (2-segment
            founder) in every arm, plus a blind 200,000-tick VIAB-2 pre-check before freeze (12 seeds
            880001..=880012, survival and gate metrics only, pass at >= 11/12 survival in each of
            scarce_mut0, selected, scarce_drift; disclose the mutation.rs:211 operator-4 wall); D2 (a)
            resource_capacity_scale = 0.75, founder_layout = 0, drift_reset_ticks = 2000, with a
            pre-approved escalation to founder_layout = 2 only if VIAB-2 at fl = 0 falls short on min_N or
            C2; D3 (a) m = 3 {H1 selected-drift, H2 selected-mut0, H3 drift-mut0}, Holm, alpha 0.05;
            D4 (b) primary endpoint ln(z_sel / z_drift), SESOI +-ln(1.10); D5 (a) N = 48 pairs, seeds
            900001 + 1000*k for k = 0..47 (144 runs) on Windows 1; D6 (a) attrition per the #214 template
            R3 (complete pairs primary iff p_ext <= 5% and pi_LL >= 0.85, with Lee bounds and RMST(200k),
            else Lachin worst-rank over all N), drift_valid == true and has_shortfall == false per
            contrast; D7 (a) QD-SEL2g pooled baseline of 12 seeds 970501..=970512,
            min_surviving_baseline_seeds = 6, smoke C1 >= 5, C2 in [0.4, 0.6], C3 >= 30, C4 > 0.50,
            C5 > 0. Merging the later freeze PR is a separate owner approval, as #231 was.
QD-E3B-T  = the frozen E3b test no_trait_mass_observable_in_smoke_manifests_or_pre_run_artifact_paths
            (src-tauri/tests/prereg_e3b_manifest_tests.rs, frozen in ebe636d, #231) asserted that
            confirmation/ does not exist, which only holds before the run. It is rescoped in E3B-RUN
            (#245) in the strict form: the recursive trait_mass scan still covers every file except
            confirmation/ and a top-level RESULT.md, and a confirmation/ bundle must carry
            smoke_reference.wp2_passed == true; both negative controls fail as required. Smoke-only
            scanning (the author's option 1) was declined. (Owner in chat, 2026-11-24: "Cho sửa, giữ
            chặt".)
QD-WAVE1124b = second queue for anima-dell, documents only, no cargo build (its free RAM is often
            below its 5 GiB floor): D8 V-SPECS-2 (V7-V11 and H10/H11), D9 FINDINGS-SYNC, D10 DOC-REVIEW,
            D11 AI-USE-LOG; at most two of its own docs PRs open; reviewer anima-mac. While anima-dell
            cannot build, the lead takes the cargo half of its reviews of anima-case (wave rule A1
            point 6); a three-seed replay of a Windows run stays on Windows. Full text: hub file
            PROMPTS_2026-11-24b_WIN2_anima-dell.md. (Owner in chat, 2026-11-24: "Giao thêm cho Dell".)
QD-TRIAL-V2 = the owner's next hands-on trial waits for World v2 milestone 1 (WV2-7, the app on the
            bundled sim_256 world); no trial installer is built from the trunk before then, though the
            release-build workflow (#233) can produce one on request. (Owner in chat, 2026-11-24: "tôi
            nghĩ xong map v2 tôi sẽ thử nghiệm sau", then "Chỉ merge, bỏ build".)
QD-WAVE1124 = a 24-hour autonomous wave on three machines (owner in chat, 2026-11-24: "Duyệt, sẽ dán"):
            anima-case (Windows 1) runs lane E (E3b smoke, confirmation run and RESULT, then SEL2-P2 and
            law-fork drafts) and lane W (world record WR1, WR2 per ADR-0008, then S7 after the E3b
            result); anima-dell (Windows 2) keeps STATE and TODO (their single writer this wave), builds a
            release-0.9 installer workflow that never publishes, writes INSTALL / release notes / checklist,
            ADR-0013 (proposed) and the V0-V6 card specs, and English versions of the Vietnamese public pages;
            anima-mac finishes WV2-2 and runs WV2-3 -> WV2-4 -> WV2-5 (stopping at the asset and release
            gates) -> WV2-7. Peer review ring: case reviews mac, dell reviews case, mac reviews dell; the
            lead re-checks and merges after a peer VERIFIED and green checks (QD-LEAD); #199 and #231 stay
            with the lead. L2 is five open code PRs for the wave (mac 2, case 2, dell 1). Agents may free
            disk themselves below 25 GB, only from their own finished worktrees' target/ directories, merged
            clean worktrees and cargo clean (rule A6 of the wave prompt). Full text: the hub files
            PROMPTS_2026-11-24_*.md; the AUTHORIZATION lines are copied into R5-AUTH below.
MAP-D3    = World v2 Card 3 after WV2-2-PHYS Step B: record and move on. The gates the physics does not
            meet (waterfall_count, river_surface_fraction, lake_surface_fraction / large_lakes,
            endorheic_humid_fraction in the parked A6 band, Taiga at 4096²) stay defined and unchanged,
            and are recorded as not met with the physical reason. The waterfall shortfall comes from
            discharge: little flow crosses the caprock, most of which is drier than PET. An independent
            review found no code defect. The tests holding those gates are #[ignore]d by name with a
            MAP-D3 reason in scripts/test_target_policy.mjs, and nothing is loosened or tuned. Improving
            them is a later card. (Owner in chat, 2026-11-24: "Ghi nhận, đi tiếp".)
QD-E3B-D1a = E3b keeps live.predator_fraction = 0.3 and is declared a two-trophic experiment: with the
            2-segment founder, predators persist to T = 18,000 in 11-12/12 seeds per arm (VIAB-1b, #228).
            Recorded in the E3b preregistration draft v2 (#229). (Owner in chat, 2026-11-23: "Giữ 0.3,
            có thú săn".)
MAP-D2    = the design amendments for WV2-2-PHYS are approved: A1 (a dipping marginal sedimentary column,
            with lithology evaluated against a datum advected with uplift), A2 (an S2b paleo-glacial
            pass), A3 (rift segmentation) and A5 (a continuous cover transition in S3, an adaptation).
            A4 (G 0.85 -> 0.25) was withdrawn on its source, and the endorheic definitions stay parked.
            The plan predicts 5-19 waterfalls per seed, below the 20-80 gate on most seeds; a shortfall
            is reported, never tuned. Recorded in the World v2 design under "Amendments after
            acceptance". (Owner in chat, 2026-11-23: "Duyệt, làm Step B".)
MAP-D1    = the World v2 Card 3 gates stay as accepted; the terrain physics is reworked until it meets
            them by process (WV2-2-PHYS: plan first, 1-D column test first). Amending the spec ranges
            was declined. (Owner in chat, 2026-11-23: "làm lại vật lý".)
QD-C2     = utf8-chars 3.0.7 (no licence file; the one row in licensing/UNRESOLVED.md): ask upstream
            and defer, and R1c merges as is. Upstream: A1-Triard/utf8-chars issue #4 and pull request #5
            (licence files). Fallback: if no utf8-chars release with licence texts exists when release
            work starts, the release build drops ml-wgpu (CPU burn-flex learner only) through a separate
            card; the desktop feature set CI tests keeps it (owner in chat, 2026-11-18: "Bạn đề xuất").
            Amended 2026-11-19: resolved by electing Apache-2.0 under the crate's declared
            MIT OR Apache-2.0 (PR #175); the owner asked for the licence row to be cleared and authorised
            the means (owner in chat, 2026-11-19: "giấy phép có cách nào k, bạn có thể tự nhắn hỏi hoặc
            làm j tuỳ"). MIT's notice condition points at a holder's notice the crate does not have;
            Apache-2.0's text is complete as published and its section 4 conditions have nothing to
            retain here, so nothing is synthesised. Recorded under `elections` in
            licensing/upstream/sources.json and enforced fail-closed by scripts/lib/upstream_licenses.mjs,
            which scans the installed crate to the bottom; licensing/UNRESOLVED.md reports 0 and the
            ml-wgpu fallback is no longer needed. Open legal question: the election meets Apache-2.0's
            conditions, but whether a bare Cargo `license` field is the author's grant is a legal
            question; the project treats the published crate's declared licence as the grant, as the
            Rust ecosystem does, and this is not legal advice. The upstream issue and pull request stay
            open; the day the graph moves to a utf8-chars release that ships licence files, the run
            fails loudly (the 3.0.7 election goes unused, and "installed text wins" refuses one moved
            to the new version) and the election is removed
QD-ECO-C1 = yes: live.predator_cost_mult + live.strike_radius, and a water-source key; opt-in; a
            preregistered measurement on at least 20 seeds. Card S7 (owner in chat, 2026-11-17)
QD-F044   = split the starvation and thirst grace counters, before any new preregistration.
            Card S8 (owner in chat, 2026-11-17)
QD-PAST   = the past of a world that has run is immutable; going back means a fork; the exception
            table is in roadmap 31/10 section 4.2 (owner in chat, 2026-11-17). Storage design decided
            2026-11-18: ADR-0008 accepted with its section 4 answers and the section 5 engine-era rule
QD-TIME   = "one day is two real hours" is a viewing speed, not a simulation law; the calendar is a
            declared world parameter. Needs an ADR, card L0 (owner in chat, 2026-11-17)
QD-LAW1   = the first law-fork experiment runs right after the first powered experiment, before
            v1.0.0. Cards L1, L2-P, L2-X (owner in chat, 2026-11-17)
QD-6      = docs/reference/IPC_CONTRACT.md. It and its gate (scripts/check_ipc_reference.mjs, in
            npm run check:docs) exist on the trunk since 2026-11-14; card G8 only adds file:line
            per entry
QD-F028   = yes: copy src-tauri/icons/ from origin/feat/ecological-selection (owner, 2026-11-14)
QD-E2M    = B: amendment_1_live_dormant_eu in e2-preregistration.json plus a real re-run of the
            whole E2 ensemble (PR #59). The four manifests in src-tauri/tests/fixtures/experiments_e2/
            must stay byte-identical to the copies under artifacts/experiments/e2-evolved-brain-default/
            (check:e2 and tests/e2_frozen_manifest_tests.rs); never edit either side. LIVE_OBSERVABLE_IDS
            is frozen; a new live observable goes in LIVE_EXTENDED_OBSERVABLE_IDS (owner's delegate,
            2026-11-16; confirmed by the owner the same day)
QD-EN     = E2R is the rerun of the brain question; E3 is the productivity gradient (owner in
            chat, 2026-11-17)
QD-P1     = yes: write the methods paper (owner in chat, 2026-11-17); topic as proposed in the swarm
            plan: evidence discipline for ALife simulation, E2 then E2R as the case study
QD-P2     = paper drafts live in a private repository, not here: this repository is public. Only
            data and the AI-use log stay under docs/paper/ (owner in chat, 2026-11-17)
QD-P3     = keep an AI-use log from now on: docs/paper/AI_USE_LOG.md (owner in chat, 2026-11-17)
QD-1      = level 2: a single-machine 0.9 first, then 1.0 (owner in chat, 2026-11-17)
QD-AE-C   = law-fork experiments get four control arms besides exotic_energy=None: ghost field
            (must equal the baseline checksum), unrewarded costly sham, iso-energetic, elevated
            primary; contract ER06/ER07 amended by a card that tests the ghost field first, after the
            SEL2 chain (owner in chat, 2026-11-18; docs/ai/planning/
            2026-11-18-research-law-fork-and-multi-machine.md)
QD-M7     = Contribute is a batch (fixed-seed epochs, compared exactly with the host); Inhabit is an
            island (each device its own world, sparse migration through the host); T5 adds a
            retroactive replay of a volunteer lineage that sweeps an island. The 25/08 design is
            amended in card NM-4 (owner in chat, 2026-11-18)
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
  AUTHORIZATION (owner, 2026-11-18): for card S9 only, you may write to src-tauri/src/core/mortality.rs,
  src-tauri/src/core/resources.rs (VitalStatistics only), src-tauri/src/core/live_experiment.rs (two new
  ids in LIVE_EXTENDED_OBSERVABLE_IDS and the code that reports them), one new test file under
  src-tauri/tests/, and src/types/generated/** only if ts-rs regenerates it. This overrides AGENTS.md's
  read-only declaration and BLOCK 0 R5 for exactly these paths and no others.
  AUTHORIZATION (owner, 2026-11-18, extending the S9 line above at the S9 session's request): for card
  S9 only, you may also write to src-tauri/src/core/experiment/observables.rs (only the two count(...)
  entries for live.deaths_starvation and live.deaths_dehydration in ObservableRegistry::live_default())
  and src-tauri/tests/prereg_esel1_manifest_tests.rs (only
  the_requested_observables_are_exactly_what_the_live_adapter_emits, compared against the 22 ids frozen
  when E-SEL1 registered). The six E-SEL1 fixture manifests stay untouched.
  AUTHORIZATION (owner, 2026-11-18): for card S10 only, you may write to
  src-tauri/src/core/environmental_systems.rs (herbivore_grazing_system only),
  src-tauri/src/core/aggregate_population.rs (the per-capita grazing call only),
  src-tauri/src/core/live_experiment.rs (the live.plant_reserve key), src-tauri/src/core/resources.rs
  (only if the reserve must live in a resource), one new test file under src-tauri/tests/, and
  src/types/generated/** only if ts-rs regenerates it. This overrides AGENTS.md's read-only declaration
  and BLOCK 0 R5 for exactly these paths and no others.
  AUTHORIZATION (owner, 2026-11-18, extending the S10 line above at a worker's request, "Duyệt"): for
  card S10 only, you may also write to src-tauri/tests/e2_seam_tests.rs and
  src-tauri/tests/eco_card_b_tests.rs (only the pinned LIVE_KEYS.len() count, 21 -> 22), and in
  src-tauri/src/core/aggregate_population.rs thread the plant reserve from
  dormant_cohort_ecology_system into DormantCohorts grazing.
  AUTHORIZATION (owner, 2026-11-18, sent to anima-case through Anima_Agent_Hub on "Gửi R1c"): for card
  R1c only, you may write to src-tauri/Cargo.toml, src-tauri/Cargo.lock, src-tauri/src/ai/model.rs,
  src-tauri/src/core/training.rs, src-tauri/src/core/snapshot.rs, src-tauri/src/core/simulation_loop.rs,
  src-tauri/benches/tick_systems.rs, the five test files named in condition 2 of R1c, one new test
  file and its fixtures under src-tauri/tests/ for condition 3, src-tauri/.cargo/audit.toml and
  src-tauri/deny.toml (only to remove ignores whose crate has left Cargo.lock), the generated licence
  artefacts (NOTICE, sbom.cdx.json and the generated files under licensing/), and
  docs/decisions/ADR-0009-*.md with its row in docs/decisions/README.md. This overrides AGENTS.md's
  read-only declaration and BLOCK 0 R5 for exactly these paths and no others.
  AUTHORIZATION (owner, 2026-11-18, "Cấp uỷ quyền SEL2-D theo đề xuất"): for card SEL2-D only, you may
  write to src-tauri/src/core/drift_control.rs (new module), src-tauri/src/core/mod.rs (only
  `pub mod drift_control;`), src-tauri/src/core/resources.rs (only LIVE_DRIFT_SELECTION in sim_stream),
  src-tauri/src/core/mortality.rs (mortality_system drift record/replay branch only),
  src-tauri/src/core/agent_systems.rs (autonomous_reproduction_system drift record/replay branch and
  its shared birth helper only), src-tauri/src/core/live_experiment.rs (live.drift_control and
  live.drift_reset_ticks keys, DriftControlState wiring, ensure_observables_supported, and the paired
  drift runner functions), src-tauri/src/core/experiment_runner/ensemble.rs (only extracting
  summarize_seed_pairs from run_paired_ensemble_with_control), src-tauri/tests/e2_seam_tests.rs and
  src-tauri/tests/eco_card_b_tests.rs (only the pinned LIVE_KEYS.len() count for the new keys),
  dataflow.json at the repository root (only what scripts/verify_dataflow.py needs for the new module's
  edges and shifted via line numbers), one new test file src-tauri/tests/sel2_drift_control_tests.rs,
  and one result note under docs/ai/planning/ with its row in docs/ai/README.md. This overrides
  AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these paths and no others.
  AUTHORIZATION (owner, 2026-11-18, extending the R1c line, given in chat: "Duyệt theo đề xuất"): for
  card R1c only, you may also write to: - src-tauri/deny.toml: only per-crate licence exceptions,
  MPL-2.0 for `buildid` and `colored`, BSL-1.0 for `xxhash-rust`, each with a comment naming the crate,
  its version and the dependency path that brings it in. No change to the global allow list. -
  scripts/lib/licensing.mjs: only the output limit of the `cargo tree` call in `cargoInventory` (raise
  `maxBuffer`, or read the output as a stream), with no change to what is inventoried or how it is
  parsed. - licensing/upstream/sources.json and licensing/upstream/**: remove the entries of crates that
  left Cargo.lock; add the upstream licence texts of new crates whose .crate ships none, fetched from
  the crate's declared repository at the published version's tag or commit, each pinned by SHA-256 in
  sources.json with its source URL. - src-tauri/src/core/live_experiment.rs: only `LIVE_MODEL_VERSION`,
  "live-bevy-world/1" -> "live-bevy-world/2". Condition 4 showed tick > 0 trajectories change in the
  default world, which under ADR-0008 section 5 opens a new engine era. Say so in ADR-0009. This
  overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these paths and no others. If
  any test or script pins the old version string, or anything else outside these paths must change, stop
  and send BLOCKED naming the file.
  AUTHORIZATION (owner, 2026-11-18, given in chat: "Cả hai"): for card F045 only, you may write to src-
  tauri/src/physics/spatial.rs (an indexed-in-grid marker and its in-place update in
  rebuild_spatial_grid_system only), src-tauri/src/evolution/genotype.rs (decode_genotype's initial
  grid-index marker only), src-tauri/src/core/world_systems.rs (spawn_food_system's initial grid-index
  marker only), src-tauri/src/core/environmental_systems.rs (seed_dropping_system's initial grid-index
  marker only), src-tauri/src/core/simulation_state.rs (SerializedAgent.sex,
  SavedSimulationState.next_node_id, the saved unindexed-collider record, and their serialize/restore
  helpers only), src-tauri/src/core/live_experiment.rs (build, install_runtime_resources' NextNodeId
  restore, genesis' initial grid-index marker, and populate_spatial_grid filtering only), src-
  tauri/src/core/simulation_loop.rs (the NextNodeId restore fallback and populate_spatial_grid on
  restore only), src-tauri/src/core/snapshot.rs (only the agent_fixture and two_segment_agent_fixture
  struct literals for the new defaulted fields), src-tauri/tests/living_world_dormancy_tests.rs, src-
  tauri/tests/brain_persistence_tests.rs, src-tauri/tests/persistence_tests.rs, src-
  tauri/tests/persistence_stress_tests.rs, src-tauri/tests/environmental_elements_tests.rs and src-
  tauri/tests/environmental_elements_stress_tests.rs (only adding `None` for the new fields to existing
  SerializedAgent/SavedSimulationState struct literals), one new test file src-
  tauri/tests/f045_restore_equivalence_tests.rs, dataflow.json at the repository root (only what
  scripts/verify_dataflow.py needs), and one result note under docs/ai/planning/ with its row in
  docs/ai/README.md. This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these
  paths and no others.
  AUTHORIZATION (owner, 2026-11-18, given in chat: "cứ tự làm theo ý bạn", delegating the card to the
  lead): for card S11 only, you may write to src-tauri/src/core/locomotion.rs (steering_drive_system and
  new grass-steering constants only), src-tauri/src/core/resources.rs (only a `grass_steering: bool`
  field on LocomotionPolicy and its disabled()/default), src-tauri/src/core/live_experiment.rs (the
  live.grass_steering key, its validation and its LocomotionPolicy wiring only), src-
  tauri/tests/e2_seam_tests.rs and src-tauri/tests/eco_card_b_tests.rs (only the pinned LIVE_KEYS.len()
  count and, in eco_card_b_tests.rs, `grass_steering: false` in existing LocomotionPolicy literals),
  src-tauri/tests/steering_zero_energy_tests.rs (only `grass_steering: false` in existing
  LocomotionPolicy literals), one new test file src-tauri/tests/grass_steering_tests.rs, one new example
  src-tauri/examples/sel2_grass_steering.rs, dataflow.json at the repository root (only what
  scripts/verify_dataflow.py needs), and one result note under docs/ai/planning/ with its row in
  docs/ai/README.md. This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these
  paths and no others.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-18): for card S12 only, you
  may write to src-tauri/src/core/live_experiment.rs (the live.lake_layout key, its validation,
  LiveWorldConfig's field and the lake placement in genesis only; lattice_point may gain a sibling
  function but its existing behaviour must not change), src-tauri/tests/e2_seam_tests.rs and
  src-tauri/tests/eco_card_b_tests.rs (only the pinned LIVE_KEYS.len() count), one new test file
  src-tauri/tests/lake_layout_tests.rs, one new example src-tauri/examples/sel2_lake_layout.rs,
  dataflow.json at the repository root (only what scripts/verify_dataflow.py needs), and one result
  note under docs/ai/planning/ with its row in docs/ai/README.md. This overrides AGENTS.md's read-only
  declaration and BLOCK 0 R5 for exactly these paths and no others.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-18): for card M4 only, you
  may write to one new test file src-tauri/tests/eu_residual_matrix_tests.rs, one new example
  src-tauri/examples/eu_residual_matrix.rs (only if the full factorial exceeds the 120 s test budget),
  one new doc docs/paper/data/eu_residual.md, and its row in docs/paper/data/README.md. This overrides
  AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these paths and no others.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-18): for card M3 only, you
  may write to one new bench src-tauri/benches/live_schedule_scaling.rs, src-tauri/Cargo.toml (one
  [[bench]] entry only), one new doc docs/paper/data/scaling.md, and its row in
  docs/paper/data/README.md. This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for
  exactly these paths and no others.
  -- Copied 2026-11-23 by the lead from the hub's WORK_PLAN.md "R5-AUTH additions" block (last
  -- modified 2026-11-20), as the note below this block requires. Verbatim; a line struck through with ~~
  -- was withdrawn on the hub. These lines authorize cards that have since merged; they are a record. --
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-18): for card S13 only, you may write to one new example src-tauri/examples/sel2_spread_lakes.rs and one result note under docs/ai/planning/ with its row in docs/ai/README.md. No other path.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card S2 only, you may write to src-tauri/src/core/live_experiment.rs (appending to LIVE_EXTENDED_OBSERVABLE_IDS, computing the new observables where the existing ones are computed, and one accumulator field on LiveExperimentAdapter with its update after each tick and its save/load handling inside this file only), src-tauri/src/core/experiment/observables.rs (registry entries for the new ids only), src-tauri/tests/observables_tests.rs, src-tauri/tests/live_experiment_tests.rs, one new test file src-tauri/tests/s2_observables_tests.rs, src/types/generated/** (regenerated by ts-rs only), and one result note under docs/ai/planning/ with its row in docs/ai/README.md. This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these paths and no others.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19, extending the S2 line at anima-case's BLOCKED): for card S2 only, you may also write to src-tauri/tests/death_cause_split_tests.rs, only the_cause_split_is_appended_after_the_existing_extended_ids: assert that LIVE_EXTENDED_OBSERVABLE_IDS[10..12] is [live.deaths_starvation, live.deaths_dehydration] (a fixed position instead of the last two), keeping its message. This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly this path and no others.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card M2 only, you may write to scripts/paper/gate_controls.mjs, scripts/paper/gate_faults/** (patch files as data), docs/paper/data/gate_controls.md and its row in docs/paper/data/README.md. No other path.
  ~~AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card W3-lite only, you may write to src/components/Landscape/WorldTerrain.tsx, WorldTerrainLod.tsx, WorldWater.tsx, WorldSky.tsx, WorldVegetation.tsx, WorldWeather.tsx, WorldWaterfalls.tsx, src/components/Landscape/utils/worldTerrainTextures.ts, utils/skyParams.ts, utils/qualityTiers.ts, utils/floraGeometry.ts, utils/chunkLod.ts, new files under src/components/Landscape/materials/, tests/frontend/** (existing tests only with a stated reason), scripts/check_bundle_budget.mjs (budget numbers only, with reason), and one result note under docs/ai/planning/ with its README row. Nothing under src-tauri/.~~ SUPERSEDED by the next line.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19; the owner's own words in chat: "Cho tải CC0" for the downloads, "Win 1 sau B1" for the machine): for card W3-lite only, you may write to src/components/Landscape/WorldTerrain.tsx, WorldTerrainLod.tsx, WorldWater.tsx, WorldSky.tsx, WorldVegetation.tsx, WorldWeather.tsx, WorldWaterfalls.tsx, src/components/Landscape/WorldShowcase.tsx (mount and lighting lines only), src/components/Landscape/utils/worldTerrainTextures.ts, utils/skyParams.ts, utils/qualityTiers.ts, utils/floraGeometry.ts, utils/chunkLod.ts, utils/oceanShelf.ts, new files under src/components/Landscape/materials/ and src/components/Landscape/assets/, public/assets/world/** (new), public/assets/basis/** (new, copied from three), scripts/gen_notice.mjs (the art-assets section only), NOTICE (regenerated only), tests/frontend/** (edits to existing tests only with a stated reason), scripts/check_bundle_budget.mjs (budget numbers only, with reason), and one result note under docs/ai/planning/ with its row in docs/ai/README.md. No other path. Downloads: only what ASSETS lists.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19, extending the W3-lite line): for card W3-lite only, you may also write to src/components/Landscape/WorldCaves.tsx, src/components/Landscape/utils/caveGeometry.ts and src/components/Landscape/WorldMinimap.tsx. Not worldGen.ts, not src/utils/sharedWorld.ts.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card LAND1 only, you may write to src-tauri/src/core/resources.rs (the policy resource and its default only), src-tauri/src/core/terrain_navigation.rs (new) and its declaration in src-tauri/src/core/mod.rs, src-tauri/src/core/simulation_schedule.rs (registering the one system only), src-tauri/src/core/live_experiment.rs (live.land_mask key, validation, LiveWorldConfig field, policy insertion, lattice snapping in genesis only), src-tauri/src/core/simulation_loop.rs (enabling the policy and land-only genesis in SimulationEngine::start only), src-tauri/src/core/world_systems.rs (spawn_food_system land check only), src-tauri/src/core/components.rs and src-tauri/src/evolution/genotype.rs (a last-walkable-position component and its insertion at spawn only, if needed), src-tauri/tests/e2_seam_tests.rs and eco_card_b_tests.rs (pinned LIVE_KEYS.len() only), src-tauri/tests/land_mask_tests.rs (new), dataflow.json, and one result note with its README row. This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these paths and no others.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card W11 only, you may write to src/components/Landscape/utils/worldTerrainTextures.ts, src/components/Landscape/materials/terrainPbrMaterial.ts, src/components/Landscape/utils/worldGen.worker.ts and utils/worldCache.ts (only if the AO bake must move off the main thread), tests/frontend/** (new tests; existing ones only with a stated reason), map-views/*.png, map_manifest.json, artifacts/world_256.anmw and artifacts/map_evidence.json (regenerated by their scripts only, never by hand), and one result note under docs/ai/planning/ (no README row: npm run gen:docs-index is the lead's). No other path.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card C4 only, you may write to src/components/Landscape/materials/terrainPbrMaterial.ts (Low-tier trims only), src/components/Landscape/utils/qualityTiers.ts (a flag only), tests/frontend/** (a test for the flag), and one result note under docs/ai/planning/ (no README row). No other path.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19, extending the LAND1 line at anima-dell's BLOCKED): for card LAND1 only, you may also write to src-tauri/tests/lake_layout_tests.rs (the pinned LIVE_KEYS.len() only), src-tauri/src/core/environmental_systems.rs (seed_dropping_system's policy-gated land check only), and src-tauri/src/core/agent_systems.rs (execute_reproduction_birth and apply_staggered_evolution_system: the policy-gated walkable spawn position and the LastWalkablePosition value at spawn only). This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these changes and no others.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card S14 only, you may write to one new example src-tauri/examples/sel2_lakes16_scale.rs and one result note under docs/ai/planning/ with its row in docs/ai/README.md. No other path.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card F047 only, you may write to one new test file src-tauri/tests/f047_restore_divergence_tests.rs and one result note under docs/ai/planning/ with its row in docs/ai/README.md. This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these paths and no others.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card S15 only, you may write to one new example src-tauri/examples/sel2_reference_check.rs and one result note under docs/ai/planning/ with its row in docs/ai/README.md. No other path.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card SEL2-P-draft only, you may write to docs/ai/planning/2026-11-19-sel2-p-preregistration-draft.md and its row in docs/ai/README.md. No other path.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card SEL2-P-impl only, you may write to src-tauri/tests/fixtures/experiments_sel2p/** (new), src-tauri/examples/run_sel2p_experiment.rs (new), src-tauri/tests/prereg_sel2p_manifest_tests.rs (new), scripts/sel2p_smoke_driver.mjs (new), scripts/verify_sel2p_artifacts.mjs (new), docs/ai/planning/2026-11-19-sel2-p-preregistration-draft.md, and one result note under docs/ai/planning/ with its row in docs/ai/README.md. This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these paths and no others. Nothing under src-tauri/src.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19; the owner approved the SEL2-P preregistration in chat: "Duyệt"): for card SEL2-P-smoke only, you may write to artifacts/experiments/sel2-p-food-scarcity/smoke/** (new, runner output only, never edited by hand) and one result note under docs/ai/planning/ with its row in docs/ai/README.md. No other path. Running: only the three commands in WHAT 2.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card SEL2-P-fix only, you may write to src-tauri/tests/fixtures/experiments_sel2p/**, src-tauri/examples/run_sel2p_experiment.rs, src-tauri/tests/prereg_sel2p_manifest_tests.rs, scripts/sel2p_smoke_driver.mjs, scripts/verify_sel2p_artifacts.mjs, docs/ai/planning/2026-11-19-sel2-p-preregistration-draft.md, docs/ai/planning/2026-11-19-sel2-p-impl.md, and the rows of those two notes in docs/ai/README.md. This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these paths and no others. Nothing under src-tauri/src: if the runner needs a library change, stop and send BLOCKED naming it.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19, extending the SEL2-P-fix line): for card SEL2-P-fix only, you may also write to src/components/ExperimentWorldControl.tsx (the bundled SEL2-P manifest copy only) and tests/frontend/experimentWorldControl.test.tsx (the N4 test only).
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card W2-lite only, you may write to src-tauri/src/core/live_experiment.rs (one pub function applying genesis and world-law policy resources to a World, extracted from build_live_world, which calls it; no behaviour change), src-tauri/src/core/simulation_loop.rs (a pending-manifest slot on SimulationEngine and its use in start() instead of the app's genesis; nothing else), src-tauri/src/commands/simulation.rs (the new command), src-tauri/src/commands/mod.rs and src-tauri/src/lib.rs (registering it only), docs/reference/IPC_CONTRACT.md (its row), src/types/generated/** (ts-rs output only), src/components/SimulationControls.tsx and one new frontend file for the control and its bundled manifest list, src-tauri/tests/start_from_manifest_tests.rs (new), one new test under tests/frontend/, dataflow.json (only what scripts/verify_dataflow.py needs), and one result note under docs/ai/planning/ with its README row. This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these paths and no others.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19, extending the W2-lite lines): for card W2-lite only, you may also write to CLAUDE.md (the command count in the "Tauri IPC contract" paragraph only) and, in src/App.tsx, the disabled condition of the ExperimentWorldControl you render. Everything else stays within the existing W2-lite lines.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19, extending the W2-lite line at anima-mac's BLOCKED): for card W2-lite only, you may also write to src/App.tsx (importing and rendering ExperimentWorldControl in the simulation controls row, and syncing the running state after a successful start only). src/components/SimulationControls.tsx is withdrawn from the W2-lite line. This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly this change and no other.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card E3-P only, you may write to src-tauri/tests/fixtures/experiments_e3/** (new), src-tauri/tests/prereg_e3_manifest_tests.rs (new), src-tauri/examples/e3_smoke.rs (new, only if needed), artifacts/experiments/e3-*/smoke/** (new), docs/ai/planning/2026-08-26-experiment-e3-productivity-gradient-body-mass.md, and its row in docs/ai/README.md. This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these paths and no others. Nothing under src-tauri/src.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card V1 only: src/components/Landscape/** , src/components/Landscape/utils/** , tests/frontend/** (edits to existing tests only with a stated reason), one result note under docs/ai/planning/ with its README row. For card B1 only: src/components/Landscape/LiveAgents.tsx, src/components/Landscape/agentInterpolation.ts, src/components/Landscape/utils/** , tests/frontend/** , scripts/check_bundle_budget.mjs (budget numbers only, with reason), one result note under docs/ai/planning/ with its README row. Nothing under src-tauri/ for either.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19, extending the B1 lines): for card B1 only, you may also write to src/components/Landscape/WorldShowcase.tsx, the simulated-entities HUD label only (choosing the synthetic wording when the tick source is synthetic). This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly this change and no other.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19, extending the B1 line): for card B1 only, you may also write to src/components/Landscape/WorldShowcase.tsx (passing qualityTier={effectiveQuality} to LiveAgents only). This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly this change and no other.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card SPIKE-NR only: on branch win/spike-native-render (never merged) any file the prototype needs, including Cargo.toml/Cargo.lock and new crates; on branch win/spike-native-render-note only the result note under docs/ai/planning/ and its README row. Never accept a licence, EULA or terms on the owner's behalf.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19): for card F048 only, you may write to src-tauri/src/core/simulation_state.rs (one new pub routing function for restored pending-inference responses and its doc comment only), src-tauri/src/core/simulation_loop.rs (only the pending-inference restore in SimulationEngine::start, replaced by a call to that function), src-tauri/src/core/snapshot.rs (only the pending_by_lineage keying in validation), src-tauri/tests/f047_restore_divergence_tests.rs, one new test file src-tauri/tests/f048_pending_restore_tests.rs, and one result note under docs/ai/planning/ with its row in docs/ai/README.md. This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these paths and no others.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-19, extending the F048 line at anima-mac's BLOCKED): for card F048 only, you may also write to src-tauri/src/core/simulation_state.rs inside SavedSharedLearningState::validate, only its pending_inference duplicate check, keyed by (lineage_id, request_id) instead of lineage_id. This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly this change and no other.
  AUTHORIZATION (owner via the lead under the owner's delegation, 2026-11-18): for card R1c only, you may also remove the --no-dedupe flag from cargoInventory's cargo tree call in scripts/lib/licensing.mjs, provided the crate set with and without it is identical on the base and head lockfiles. No other change to the licensing scripts, rules or tests.
  AUTHORIZATION (owner, 2026-11-23, "Duyệt theo đề xuất" on the lead's review): for the R5 findings
  fixes (PR #200, F-055..F-065 and F053-FIX) only, you may write to the 17 files that PR already
  touches: FINDINGS.md, src-tauri/crates/anima-domain/src/intervention.rs,
  src-tauri/src/commands/simulation.rs, src-tauri/src/core/{branch_tree.rs, dynamic_fields.rs,
  experiment/observables.rs, live_experiment.rs, locomotion.rs, simulation_loop.rs, sweeps.rs,
  terrain_navigation.rs}, src-tauri/src/evolution/newick.rs, and src-tauri/tests/{
  domain_and_fields_hardening_tests.rs, observables_tests.rs, post_194_hardening_audit_tests.rs,
  prereg_sel2p_manifest_tests.rs, scientific_pipeline_hardening_tests.rs}, plus
  src-tauri/src/core/experiment/interventions.rs (manifest check radius > 0) and
  src-tauri/src/core/exotic_energy/forcings.rs (checked add), both named in the 2026-11-23
  verdict. F-064 may move to its own PR with the same paths. This overrides AGENTS.md's
  read-only declaration and BLOCK 0 R5 for exactly these paths and no others.
  AUTHORIZATION (owner, 2026-11-23): for F-066 (PR #207, lead) only, the 28 files that PR touches
  (live lineage tracking under src-tauri/src/core and src-tauri/src/evolution, their tests, the
  pinned LIVE_KEYS counts, and the E3 preregistration fixture's key list). This overrides AGENTS.md's
  read-only declaration and BLOCK 0 R5 for exactly these paths and no others.
  AUTHORIZATION (owner, 2026-11-23): for card WV2-2 (PR #199) only: src-tauri/crates/anima-worldgen/**
  (src, src/bin and tests), src-tauri/Cargo.lock, scripts/test_target_policy.mjs, and one card note
  docs/ai/planning/2026-11-2x-wv2-2.md. This overrides AGENTS.md's read-only declaration and BLOCK 0
  R5 for exactly these paths and no others.
  AUTHORIZATION (owner, 2026-11-23): for the conflict-marker gate (PR #210) and card P1 (PR #211)
  only, in addition to each card's own paths: .github/workflows/ci.yml (one step in the Frontend job
  for #210; the Frontend checkout's fetch-depth for #211), package.json (one script for #210), and
  each PR's single test file under tests/frontend/. This overrides AGENTS.md's read-only
  declaration and BLOCK 0 R5 for exactly these paths and no others.
  AUTHORIZATION (owner via the lead, 2026-11-23, sent to anima-case through the hub): for card VIAB-1
  only, you may add the probe source unchanged under artifacts/experiments/viab1-shared-viability/probe/
  (src/main.rs, Cargo.toml, Cargo.lock, never added to the Cargo workspace) and three lines for them in
  that directory's checksums.sha256, besides the card's own artifacts and result note.
  AUTHORIZATION (owner via the lead, 2026-11-23): for cards E3B-P and E3B-PWR only (documents only),
  docs/ai/planning/2026-11-23-e3b-preregistration-draft.md. For card NR-SPIKE-PROTOCOL only (documents
  only), docs/ai/planning/2026-11-23-nr-spike-protocol.md. No code, manifest or fixture.
  AUTHORIZATION (owner via the lead, 2026-11-23): for card FOUNDER-2SEG only, you may write to
  src-tauri/src/core/live_experiment.rs (the live.founder_genotype key constant, LIVE_KEYS, the config
  field, the validation and the parse arm, and the founder genotype in genesis only), the LIVE_KEYS.len()
  assertions in src-tauri/tests/ (e2_seam, eco_card_b, founder_layout, grass_steering, lake_layout,
  land_mask, strict_prey_cap, live_lineage and any other test pinning that count),
  src-tauri/tests/fixtures/experiments_e3/e3-preregistration.json (one default-absent key),
  src-tauri/tests/prereg_e3_manifest_tests.rs (only if the key list needs it), one new test file
  src-tauri/tests/founder_genotype_tests.rs, one amendment line in
  docs/ai/planning/2026-08-26-experiment-e3-productivity-gradient-body-mass.md, one card note under
  docs/ai/planning/, and dataflow.json (only if verify_dataflow.py needs it). This overrides AGENTS.md's
  read-only declaration and BLOCK 0 R5 for exactly these paths and no others.
  -- Wave 2026-11-24 (QD-WAVE1124): verbatim from the hub files PROMPTS_2026-11-24_*.md, backticks
  -- removed. The owner approved them in chat ("Duyệt, sẽ dán") and grants them by pasting the prompts. --
  AUTHORIZATION (owner, 2026-11-24, to anima-case, given by pasting the wave prompt): for card
  E3B-RUNNER only, you may write one new example src-tauri/examples/run_e3b_experiment.rs, one new test
  src-tauri/tests/e3b_runner_tests.rs, package.json (one script line only) and one note under
  docs/ai/planning/. **Not** src-tauri/src/**, not the frozen fixtures, not the frozen doc, not the
  verifier. If the runner needs anything under src-tauri/src, stop: BLOCKED to owner.
  AUTHORIZATION (owner, 2026-11-24, to anima-case, given by pasting the wave prompt): for card E3B-SMOKE
  only, you may write artifacts/experiments/e3b-productivity-gradient-body-mass/smoke/** (runner output
  only, never edited by hand) and one result note under docs/ai/planning/. No other path.
  AUTHORIZATION (owner, 2026-11-24, to anima-case, given by pasting the wave prompt): for card E3B-RUN
  only, you may write artifacts/experiments/e3b-productivity-gradient-body-mass/confirmation/** (runner
  output only) and the result note. No other path.
  AUTHORIZATION (owner, 2026-11-24, to anima-case, given by pasting the wave prompt): for card
  E3B-RESULT only, you may write artifacts/experiments/e3b-productivity-gradient-body-mass/RESULT.md and
  one note under docs/ai/planning/. No other path.
  AUTHORIZATION (owner, 2026-11-24, to anima-case, given by pasting the wave prompt): for card
  SEL2P2-DRAFT only, you may write one new doc docs/ai/planning/<date>-sel2-p2-preregistration-draft.md.
  No other path.
  AUTHORIZATION (owner, 2026-11-24, to anima-case, given by pasting the wave prompt): for card
  LAW1-DRAFT only, one new doc docs/ai/planning/<date>-law1-fork-experiment-draft.md. No other path.
  AUTHORIZATION (owner, 2026-11-24, to anima-case, given by pasting the wave prompt): for card WR1 only,
  you may write a new module under src-tauri/src/core/world_record/ (or one file src-
  tauri/src/core/world_record.rs), its mod declaration in src-tauri/src/core/mod.rs, src-
  tauri/src/core/simulation_schedule.rs (registering one recording system, inactive unless a recording
  resource is present), new tests src-tauri/tests/world_record_*_tests.rs, src-tauri/Cargo.toml only if
  a hashing crate already in the tree must become a direct dependency (no new crate), and one note under
  docs/ai/planning/. **No** new LIVE_KEYS entry and no manifest key: the E3b freeze pins the live-key
  partition. Recording is switched on through a Rust API that tests use. No other path.
  AUTHORIZATION (owner, 2026-11-24, to anima-case, given by pasting the wave prompt): for card WR2 only,
  the WR1 module paths, src-tauri/src/core/simulation_schedule.rs (the same one system only), new tests
  src-tauri/tests/world_record_*_tests.rs, and one note. No other path.
  AUTHORIZATION (owner, 2026-11-24, to anima-case, given by pasting the wave prompt): for card S7 only,
  you may write docs/decisions/ADR-0007-live-ecology-sustainability.md (an amendment section, owner
  approves it in the PR), src-tauri/src/core/live_experiment.rs (the three keys in LIVE_KEYS and their
  parsing), src-tauri/src/core/resources.rs (their policy resources and defaults), src-
  tauri/src/core/agent_systems.rs (only where the predator's cost and strike radius are applied, and the
  water-source hook), the tests that pin LIVE_KEYS.len() (the pinned count only), new tests src-
  tauri/tests/s7_*_tests.rs, and one note.
  AUTHORIZATION (owner, 2026-11-24, to anima-mac, given by pasting the wave prompt): for card WV2-3
  only, you may write the paths the design's Card 4 spec and the §8 ownership table assign to WV2-3, and
  only those: src-tauri/crates/anima-worldgen/**, src-tauri/src/core/world_artifact.rs, src-
  tauri/src/core/ecs.rs (the loud v3 rejection only), src-tauri/src/core/live_experiment.rs and
  docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md (only what the card names), every WorldIdentity { ...
  } literal site the card lists (re-count them at your base with rg -n "WorldIdentity \{" src-tauri;
  include sites added since the design), src-tauri/tests/world_v2_sim_gate_tests.rs, src-
  tauri/Cargo.toml / Cargo.lock (as the card needs), NOTICE and the licensing artifacts (regenerated,
  never hand-edited), scripts/test_target_policy.mjs, one note under docs/ai/planning/. Before your
  first code commit, list the exact files in the draft PR body; the reviewer checks the diff against
  that list. Anything else under src-tauri/src: stop, BLOCKED.
  AUTHORIZATION (owner, 2026-11-24, to anima-mac, given by pasting the wave prompt): for card WV2-4
  only: the paths the Card 5 spec and §8 assign to WV2-4 (src-tauri/crates/anima-worldgen/**, the verify
  subcommand in src/bin/anima-worldgen.rs, artifacts/world-v2/ with preset_v2.toml, sim_256.anmw3,
  index.json, gates.json, the CI / test-policy files the table lists) and one note. Nothing under src-
  tauri/src.
  AUTHORIZATION (owner, 2026-11-24, to anima-mac, given by pasting the wave prompt): for card WV2-5
  only: the paths the Card 6 spec and §8 assign to WV2-5 (src/bin/anima-bake.rs, the crate, its bake
  feature dependencies, grid job scripts, wv2_5_gpu_bake_tests with its policy entries,
  artifacts/world-v2/index.json updates) and one note. Grid bake jobs are allowed under QD-OXS (offline
  map jobs with pinned, hashed output, GRID WORKER only).
  AUTHORIZATION (owner, 2026-11-24, to anima-mac, given by pasting the wave prompt): for card WV2-7
  only: the paths of the Card 7 spec and §8 (src-tauri/src/core/world_artifact.rs, src-
  tauri/src/core/ecs.rs AppArtifactOrGenerated, the audit of the lake threshold in simulation_loop.rs,
  src-tauri/tauri.conf.json static bundle.resources with only the two committed files) and one note.
  AUTHORIZATION (owner, 2026-11-24, to anima-dell, given by pasting the wave prompt): for card STATE-
  SYNC only, you may write docs/planning/STATE_OF_THE_PROJECT.md (the banner at the top of §1, the stage
  board in §3.00, the "done" lists under §3.00, and §4 traps with a date) and TODO.md. Not §1's
  measurement tables (§1.a–§1.i): a measurement is added only by a measurement card. Not FINDINGS.md:
  propose finding entries in the PR body for the lead.
  AUTHORIZATION (owner, 2026-11-24, to anima-dell, given by pasting the wave prompt): for card REL09-CI
  only, you may write one new workflow .github/workflows/release-build.yml and one note under
  docs/ai/planning/. Not ci.yml, not tauri.conf.json, not package.json. If a change there is needed,
  BLOCKED with the diff you would make.
  AUTHORIZATION (owner, 2026-11-24, to anima-dell, given by pasting the wave prompt): for card
  REL09-DOCS only, you may write docs/how-to/INSTALL.md (new, English), docs/release/0.9.0.md (new
  release-notes draft, with a README.md index in docs/release/ if the docs policy requires one) and
  docs/how-to/RELEASE_CHECKLIST.md (new). Nothing at the repository root: the root-hygiene gate forbids
  new root files.
  AUTHORIZATION (owner, 2026-11-24, to anima-dell, given by pasting the wave prompt): for card V-ADR
  only, you may write one new ADR docs/decisions/ADR-0013-<slug>.md with status: proposed (check that
  0013 is free; 0011 is reserved for the native renderer), its row in docs/decisions/README.md, and two
  notes under docs/ai/planning/: a V0 card spec and a V1 card spec.
  AUTHORIZATION (owner, 2026-11-24, to anima-dell, given by pasting the wave prompt): for card DOC-EN
  only, you may write the pages you list in the PR body. For each one, the English text becomes the page
  at its current path. The Vietnamese text moves, unchanged, to a sibling <NAME>.vi.md with its own
  front matter, as a translation (QD-DOCLANG: "next to the English one, never instead of it"). You may
  also update the index README.md of those three directories. Before writing, check
  scripts/check_docs_metadata.mjs for how a .vi.md sibling must be indexed. If the checker cannot accept
  siblings, BLOCKED with the error.
  AUTHORIZATION (owner, 2026-11-24, to anima-dell, given by pasting the wave prompt): for card V-SPECS
  only, one new note per card under docs/ai/planning/ (V2 SpeciesProfile, V3 lifecycle per your ADR-0013
  draft, V4 carcasses and decomposition with a bit-exact EU ledger, V5 soil water, nutrients and grass
  biomass as fields, V6 the four interventions with CauseId). No other path.
  AUTHORIZATION (owner via the lead, 2026-11-24): for card WV2-2 (PR #199) only, you may also write
  scripts/paper/gate_faults/feature_separation_worldgen.patch: regenerate it against the branch's
  src-tauri/crates/anima-worldgen/Cargo.toml so that it injects exactly the same fault (one added line
  bevy_ecs = "0.13.0" in [dependencies]) and nothing else. WV2-2 grew that Cargo.toml, so the M2 fault
  patch no longer applied and gateControls.test.ts failed (CI job 137908480045). No other path.
  -- Added during the wave, 2026-11-24, by the lead in chat with the owner; the hub message or file named
  -- in each line holds the full text. --
  AUTHORIZATION (owner, 2026-11-24, in chat to the lead: "Cho sửa, giữ chặt"; hub MSG_20261124_055500):
  for card E3B-RUN only, you may also edit src-tauri/tests/prereg_e3b_manifest_tests.rs lines 609-627
  (the end of no_trait_mass_observable_in_smoke_manifests_or_pre_run_artifact_paths) in one separate
  commit, in the strict form of QD-E3B-T, with negative controls (a) trait_mass under smoke/ and
  (b) wp2_passed = false both shown failing. No other line of that file and no other path.
  AUTHORIZATION (owner, 2026-11-24, in chat to the lead: "Giao thêm cho Dell"; hub
  PROMPTS_2026-11-24b_WIN2_anima-dell.md), to anima-dell: for card V-SPECS-2 only, new files
  docs/ai/planning/2026-11-24-v7-*-spec.md through 2026-11-24-v11-*-spec.md and one
  docs/ai/planning/2026-11-24-h10-h11-release-gates.md; for card FINDINGS-SYNC only, FINDINGS.md
  (status changes only with evidence, dated lines appended, two new entries from anima-mac's WV2-3
  report); for card DOC-REVIEW only, exactly the documents that check_docs_metadata.mjs --report lists
  as overdue when the card starts (not docs/archive/, docs/ai/README.md, CLAUDE.md or AGENTS.md); for
  card AI-USE-LOG only, an appended entry in docs/paper/AI_USE_LOG.md. No other path.
  AUTHORIZATION (owner, 2026-11-24, in chat to the lead: "duyệt theo khuyến nghị"; hub
  MSG_20261124_080000), to anima-case: for card SEL2P2-VIAB2 only, you may (1) edit
  docs/ai/planning/2026-11-24-sel2-p2-preregistration-draft.md to fill D1-D7 with QD-SEL2P2-D1..D7
  (options not chosen stay, marked "not chosen"); (2) write runner output under
  artifacts/experiments/sel2-p2/viab2/**; (3) write one note docs/ai/planning/<date>-sel2-p2-viab2-results.md.
  No change under src-tauri/; a missing runner mode or key is BLOCKED, not written. Blind: no body mass
  and no trait_mass value is computed, printed or committed.
  AUTHORIZATION (owner, 2026-11-24, in chat to the lead; hub MSG_20261124_084500), to anima-mac: for card
  WV2-4 only, within its existing paths, commit C01 as the provisional world under MAP-D4
  (artifacts/world-v2/{preset_v2.toml,sim_256.anmw3,index.json,gates.json}, gates.json with all 15 gates as
  measured), MAP-D4 #[ignore] reasons with named rows in scripts/test_target_policy.mjs, and a MAP-D4 section in
  the WV2-4 note. No threshold change anywhere.
  AUTHORIZATION (owner, 2026-11-24, in chat to the lead; hub MSG_20261124_084500), to anima-case: for card
  SEL2P2-VIAB2 only, in addition to its paths above, one new example src-tauri/examples/run_viab2_probe.rs,
  one new test src-tauri/tests/viab2_probe_tests.rs, and src-tauri/Cargo.toml only for an [[example]] /
  [[test]] entry if required-features are needed. Not src-tauri/src/**, not run_sel2p_experiment.rs, not any
  frozen fixture. This overrides AGENTS.md's read-only declaration and BLOCK 0 R5 for exactly these paths and
  no others.
  AUTHORIZATION (lead under the owner's delegation, 2026-11-24; hub MSG_20261124_110000), to anima-mac: for
  card WV2-4 only, add exactly three lines to .gitattributes (a comment, artifacts/world-v2/*.json text eol=lf,
  artifacts/world-v2/*.toml text eol=lf), matching R0's artifacts/maps pin (#123). Nothing else in that file.
  AUTHORIZATION (owner, 2026-11-24, in chat to the lead; hub MSG_20261124_112000 and MSG_20261124_124500 to
  MSG_20261124_140000), to anima-dell, documents only: for card ISLAND-ADR, one new
  docs/decisions/ADR-0014-shared-island-host-and-guests.md (proposed) and one
  docs/ai/planning/2026-11-24-island-cards.md; for card GROWTH-AMEND, a new "Physical growth" section in
  docs/decisions/ADR-0013-live-creature-lifecycle.md (stays proposed) and nothing else in that file; for card
  PLANT-EVO-ADR, one new docs/decisions/ADR-0015-evolving-plants.md (proposed) and one
  docs/ai/planning/2026-11-24-plant-cards.md; for card WORLD-CHANGE-ADR, one new
  docs/decisions/ADR-0016-changing-world-surface.md (proposed) and one
  docs/ai/planning/2026-11-24-world-change-cards.md; for each card, one row in docs/decisions/README.md only if
  npm run check:docs requires it. No other path.
  AUTHORIZATION (owner, 2026-11-24, in chat to the lead; hub MSG_20261124_150000), to anima-dell, documents
  only: for card SIGNALS-ADR, one new docs/decisions/ADR-0017-evolved-communication.md (proposed) and one
  docs/ai/planning/2026-11-24-signals-cards.md; no other path. (hub MSG_20261124_144500) for card WIP-BACKUP,
  in the Dell main checkout only: create and push one branch dell/wip-backup-20261124 holding its uncommitted
  changes (secrets and build noise excluded and reported), then switch back; no stash, reset, clean, deletion,
  force or PR.
  AUTHORIZATION (owner, 2026-11-24, in chat to the lead; hub MSG_20261124_153000), to anima-dell, documents
  only: for card ECOSYSTEM-ADR, one new docs/decisions/ADR-0018-ecosystem-scales-and-cycles.md (proposed) and
  one docs/ai/planning/2026-11-24-ecosystem-cards.md; no other path.
  AUTHORIZATION (owner, 2026-11-24, in chat to the lead; hub MSG_20261124_112000), to anima-case: card
  ISLAND-INVENTORY writes one hub file test_runs/anima-case_island_inventory_20261124.md and nothing in the
  repository.
CURRENT WAVE = 3 (Đợt 3 trở đi, từ 2026-11-18; chặng 4 của kế hoạch 13/11)
```

Dòng uỷ quyền của **R1c** đã được chép vào `R5-AUTH` ở trên (các dòng ngày 2026-11-18). Mọi dòng uỷ quyền cấp trên hub từ 2026-11-18 đến 2026-11-20 cũng đã được chép vào khối đó ngày 2026-11-23; từ nay lead chép dòng mới vào đây cùng lúc gửi nó lên hub.

Dòng uỷ quyền của T2 dưới đây **đã dùng xong** (T2 merge ngày 2026-11-15) và được giữ làm ví dụ lịch sử.
Mẫu để soạn dòng mới là §4 của [file prompt swarm](../ai/planning/2026-11-05-swarm-prompts.md); dòng cho
một thẻ ghi vào `src-tauri/src` phải có câu "This overrides AGENTS.md's read-only declaration and BLOCK 0
R5 for exactly these paths and no others", câu mà dòng của T2 không có:

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
- **Còn hai mâu thuẫn giữa các tài liệu kế hoạch** (kế hoạch 13/11 §12, kiểm lại 2026-11-18):
  tên "E3" chỉ hai thí nghiệm khác nhau (QĐ-EN đã quyết ngày 2026-11-17: E2R là câu hỏi về não, E3 là
  gradient năng suất; roadmap 16/10 vẫn dùng tên cũ và có ghi chú ở đầu trang); `H1`–`H4` chỉ những việc khác nhau trong
  `EXECUTION_PLAN` và kế hoạch swarm — khi trích phải ghi kèm tên tài liệu. Mâu thuẫn thứ ba đã hết:
  roadmap 16/10 có trên thân cây từ khi hai cây hợp nhất.

## 8. Sau mỗi đợt

Chủ dự án merge, ghi số đo vào STATE §1 (một tiểu mục `§1.x` mới, tiếp sau tiểu mục mới nhất, hiện là §1.h), thêm
một dòng vào [`TODO.md`](../../TODO.md), rồi sửa §1 và §3 của trang này.
