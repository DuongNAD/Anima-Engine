---
title: Nhật ký dùng AI
status: active
owner: maintainers
last_reviewed: 2026-11-17
review_cycle: monthly
---

# Nhật ký dùng AI

Chủ dự án quyết ngày 2026-11-17 (QĐ-P3): mở nhật ký này ngay, vì nơi nhận bài nào cũng đòi khai việc dùng AI
và nhật ký không dựng lại được về sau. Nó ghi **công cụ nào, phiên bản nào, dùng vào việc gì**. Nó không phải
bản khai báo nộp kèm bài; bản đó được viết từ đây khi đã chọn nơi nộp.

Quy tắc ghi: chỉ ghi điều kiểm được, kèm cách kiểm. Điều không kiểm được thì ghi vào mục "Chưa biết", không
ước lượng.

## 1. Mã và tài liệu trong repo

Nguồn: trailer `Co-Authored-By` trong lịch sử của thân cây `feat/ecological-selection`, đếm ngày 2026-11-17
bằng `git log origin/feat/ecological-selection --format=%b | grep -i '^Co-Authored-By:'`.

| Công cụ | Phiên bản ghi trong trailer | Số commit |
|---|---|---|
| Claude Code | Claude Opus 5 | 266 (thêm 2 ghi "1M context") |
| Claude Code | Claude Opus 5.5 | 65 |
| Claude Code | Claude Sonnet 5.5 | 43 |
| Claude Code | Claude Opus 4.8 | 25 (thêm 1 ghi "1M context") |
| Claude Code | Claude Fable 5 | 19 |

Thân cây có 727 commit, từ 2026-06-12 tới 2026-11-17; 421 commit mang ít nhất một trailer như trên. Mọi commit
đứng tên tài khoản của chủ dự án, nên **commit không có trailer không có nghĩa là không dùng AI**.

Việc AI đã làm, theo chính các tài liệu của repo:

- Viết mã Rust và TypeScript, test, script kiểm tra và tài liệu, theo thẻ công việc
  ([`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md)). Mỗi thẻ kết thúc bằng một pull request mà chủ dự án
  đọc rồi merge.
- Soạn kế hoạch, ADR ở dạng nháp và bản đăng ký thí nghiệm. Ba ADR (0005–0007) do một phiên AI quyết theo
  uỷ quyền, rồi được chủ dự án xác nhận ngày 2026-11-17
  ([sổ quyết định](../planning/COMPLETION_ROADMAP_2026-10-31.md#41-quyết-định-sau-2026-11-14-ghi-thêm-2026-11-17)).
- Kiểm chứng độc lập: một phiên AI khác, ngữ cảnh sạch, chạy lại điều kiện "xong khi" của từng thẻ.
- Chạy thí nghiệm E2 và lượt chạy lại của nó, và viết `RESULT.md`; các con số trong đó được một script tính
  lại từ artifact (`npm run check:e2`).

## 2. Công cụ khác đã dùng

| Công cụ | Dùng vào việc gì | Bằng chứng trong repo |
|---|---|---|
| Antigravity (model Gemini) | Swarm làm thẻ, và soạn một số báo cáo | [Kế hoạch swarm](../ai/planning/2026-11-05-swarm-master-plan.md); commit của nó không mang trailer |
| Một lượt agent, chưa rõ công cụ | Đẩy một khối 188 file lên `main` ngày 2026-10-22 | [Kế hoạch swarm](../ai/planning/2026-11-05-swarm-master-plan.md) §0 |
| Codex | Một số nhánh làm việc | 43 nhánh `codex/*` trên remote ngày 2026-11-17 (`git branch -r`) |
| Gemini API | Một tính năng **của sản phẩm** (meta-AI đề xuất biến cố môi trường), không phải công cụ viết mã. Chế độ tất định không gọi nó | `evolution/meta_ai.rs`; [hợp đồng tất định §3](../reference/DETERMINISM_CONTRACT.md) |

## 3. Lỗi do AI gây ra mà dự án đã ghi lại

Mục này tồn tại vì một bài báo về kỷ luật bằng chứng phải khai cả phần này.

- Một báo cáo kiểm toán tự động khuyên một bản sửa làm mọi cảm biến mù; bản sửa đã được áp rồi phải gỡ
  ([bản lưu trữ](../archive/AUDIT_REPORT.md), mục SIM-01).
- Một lượt agent đẩy bằng chứng bịa (ảnh PNG 1×1, seed cứng, dữ liệu giả trên dashboard) và nới assertion;
  các thẻ G3, G4, G6 và G7 của kế hoạch swarm được lập ra để gỡ chúng, và G3 chưa xong
  ([kế hoạch swarm](../ai/planning/2026-11-05-swarm-master-plan.md) §0, §1;
  [hàng đợi thẻ](../how-to/AGENT_WORKFLOW.md#3-hàng-đợi-thẻ)).
- `PROJECT.md` từng ghi là xong nhiều tính năng không có trong mã; sửa ngày 2026-11-17.

Danh sách đầy đủ các lỗi "exit 0 mà sai" là việc của thẻ P1.

## 4. Chưa biết

- Số commit do Antigravity hoặc Codex viết: không có trailer, không đếm được từ lịch sử.
- Phiên bản model Gemini mà Antigravity dùng ở từng đợt.
- Mức độ con người sửa lại từng thay đổi do AI viết trước khi merge.
- Việc dùng AI ngoài repo: nghiên cứu bên ngoài, các báo cáo mà
  [bản tổng hợp 13/11](../research/RESEARCH_SYNTHESIS_2026-11-13.md) dựa vào.

## 5. Cách ghi tiếp

Sau mỗi đợt làm việc, thêm một dòng vào bảng dưới: ngày, công cụ và phiên bản, việc đã làm, pull request.
Đếm lại bảng ở mục 1 bằng lệnh đã ghi trước khi trích nó vào bài.

| Ngày | Công cụ, phiên bản | Việc | PR |
|---|---|---|---|
| 2026-11-17 | Claude Code, Claude Fable 5.2 | Rà soát toàn bộ tài liệu so với mã, ghi sổ quyết định, mở nhật ký này | #97 |
| 2026-11-24 | Antigravity (model Gemini theo cấu hình hub; không có trailer git) trên 3 máy; Claude Code (`Claude Opus 5.5` trong trailer của lead) | Đợt song song ba máy + vòng kiểm chéo: khoa học E3b/SEL2-P2/law-fork & WR1–WR2 (`anima-case`), Map v2 WV2-2..WV2-4 (`anima-mac`), trạng thái/CI 0.9/tài liệu/ADR (`anima-dell`), kiểm cuối & merge, giữ nửa Cargo khi Dell thiếu RAM, kiểm `#199`/`#231`, tạo `#230`/`#248`/`#254` (`lead`) | 28 PR (`#199`, `#230`, `#232–#254`, `#256–#258`; bảng chi tiết bên dưới) |

### Đợt 2026-11-24 (thân cây tới `f195a361`)

Mục này ghi lại đợt làm việc ngày `2026-11-24` ở snapshot thân cây `feat/ecological-selection` tại `f195a361` (phần ghi năm `2026-11-17` phía trên giữ nguyên không sửa). Mọi số liệu dưới đây kiểm trực tiếp từ `git`, `gh` CLI và các tệp điều phối/verdict trên hub.

#### Công cụ, model theo hồ sơ hub và trailer git

| Vai trò / Máy | Công cụ & môi trường | Ghi nhận model trên hub / git | Nguồn kiểm |
|---|---|---|---|
| `anima-case` (Win 1, `DESKTOP-3EPV830`, `agent-win`) | Antigravity trên Windows 1 | Tài liệu thiết lập hub hướng dẫn chọn `Gemini` (`Gemini 4 Neon Extra`); các PR `win/*` không mang trailer `Co-Authored-By` | `PROMPTS_2026-11-24_00_HUONG_DAN_SETUP.md`, `git log b0413f2e..f195a361` |
| `anima-dell` (Win 2, `DESKTOP-IT8UKCD`) | Antigravity trên Windows 2 | Tài liệu thiết lập hub hướng dẫn chọn `Gemini` (`Gemini 4 Neon Extra`); các PR `dell/*` không mang trailer `Co-Authored-By` | `PROMPTS_2026-11-24_00_HUONG_DAN_SETUP.md`, `git log b0413f2e..f195a361` |
| `anima-mac` (MacBook Air M5) | Antigravity trên Mac | Tài liệu thiết lập hub hướng dẫn chọn `Gemini` (`Gemini 4 Neon Extra`); `26/27` commit trong `#199` (phần của `anima-mac`) và toàn bộ commit nội dung trong `#242`, `#249` không mang trailer `Co-Authored-By` | `PROMPTS_2026-11-24_00_HUONG_DAN_SETUP.md`, `git log b0413f2e..f195a361` |
| `lead` (phiên điều phối, gửi dưới tên `owner` trên hub) | Claude Code (trên Mac) | `7` commit trong `b0413f2e..f195a361` mang `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`: `3` trong `#230`, `2` trong `#248`, `1` trong `#254`, `1/27` commit (`15eaf15f`, merge thân cây ngày 2026-11-23) trong `#199` | `git log b0413f2e..f195a361 --grep='^Co-Authored-By:'` |

Đếm lại toàn bộ trailer `Co-Authored-By` trên thân cây tới `f195a361` bằng `git log f195a361 --format=%b | grep -i '^Co-Authored-By:' | sort | uniq -c` (không sửa bảng mục 1 ngày `2026-11-17`):
thân cây có `1357` commit (`2026-06-12` tới `2026-11-24`), trong đó `565` commit mang trailer `Co-Authored-By`: `266` `Claude Opus 5` (thêm `2` ghi `"1M context"`), `178` `Claude Opus 5.5`, `52` `Claude Sonnet 5.5`, `25` `Claude Opus 4.8` (thêm `1` ghi `"1M context"`), `22` `Claude Fable 5.2`, `19` `Claude Fable 5`.

#### Phần việc từng máy và vòng kiểm chéo (review ring)

- **`anima-case` (Win 1):** Hai làn song song — Làn E khoa học (`C1` runner `#234`, `C2` smoke mù `1` seed `999953` × `3` nhánh `#240`, `C3` chạy `192` lượt xác nhận `#245`, `C4` `RESULT.md` `#252`, `C5`/`C5b` nháp SEL2-P2, khoá `D1–D7` & probe `VIAB-2` `#247`, `#258`, `C6` nháp law-fork `#253`; cùng `C0` `#231` đóng băng E3b merge lúc `2026-11-23T21:08:59Z`), Làn W engine (`WR1` `#236`, `WR2` `#246`), và `C11` `ISLAND-INVENTORY` (kiểm kê đọc-chỉ trên hub làm đầu vào cho `#256`, không mở PR).
- **`anima-mac` (Mac):** Tuyến Map v2 / `anima-worldgen` (`WV2-2` `#199`, `WV2-3` `#242`, `WV2-4` `#249`) và điều phối lưới OxideSwarm khi được kích hoạt. Hai PR tiền tố `mac/` khác là `#248` (`mac/decisions-1124b`) và `#254` (`mac/fix-wr2-world-identity`) gồm các commit nội dung mang trailer `Claude Opus 5.5` do phiên `lead` Claude Code trên Mac tạo, còn `#199` cũng có một commit merge thân cây (`15eaf15f`) từ `lead`.
- **`anima-dell` (Win 2):** Tuyến trạng thái, phát hành và tài liệu (`D1` đồng bộ `STATE_OF_THE_PROJECT` / `TODO` `#232`, `#241`, `#243`; `D2` workflow CI build 0.9 không publish/ký `#233`; `D3` tài liệu phát hành 0.9 `#238`; `D4` nháp ADR-0013 & spec `V0–V1` `#235`; `D6` dịch Anh/Việt `5` trang công khai `#237`; `D7`/`D8` spec `V2–V11` & checklist `H10/H11` `#239`, `#244`; `D9` đồng bộ `FINDINGS` `#250`; `D10` rà `3` tài liệu quá hạn `#251`; `D12` ADR-0014 & thẻ `I1–I11` `#256`; `D20` ghi lỗi kiểm toán sinh học `F-069..F-080` `#257`).
- **`lead` (Claude Code trên Mac):** Kiểm tra cuối và merge các PR từ các máy khi có verdict `VERIFIED` hợp lệ và CI xanh; kiểm trực tiếp hai PR đầu đợt chờ sẵn (`#199`, `#231`); đảm nhận phần Cargo trong kiểm chéo các PR `anima-case` khi `anima-dell` dưới ngưỡng 5 GiB RAM (xem mục vòng kiểm chéo bên dưới); và trực tiếp tạo rồi merge các PR của chính `lead` (`#230`, `#248`, `#254`, không có verdict chéo riêng trong `verdicts/`), đồng thời hỗ trợ merge thân cây (`15eaf15f`) ở `#199`.
- **Vòng kiểm chéo (review ring):** Theo quy tắc đợt (`PROMPTS_2026-11-24_00_HUONG_DAN_SETUP.md`), không máy nào tự review bài của mình: **`anima-mac` do `anima-case` kiểm trên Windows; `anima-case` do `anima-dell` kiểm; `anima-dell` do `anima-mac` kiểm** (riêng `#199` và `#231` do `lead` kiểm trực tiếp, và mọi PR đều qua `lead` kiểm cuối trước khi merge). Ngay trong đợt `f195a361` ban ngày, theo quy tắc ngưỡng RAM `5 GiB` (`A1` mục `6` và `PROMPTS_2026-11-24b` / `QD-WAVE1124b`), khi `anima-dell` dưới ngưỡng RAM này thì `anima-dell` kiểm phần không chạy Cargo còn `lead` kiểm phần Cargo cho `anima-case` (`#234`, `#236`, `#240`, `#245` [kèm `anima-dell` chạy lại `3` seed `--replay` trên Windows khi đạt `>= 5 GiB` RAM], `#258`), và `lead` kiểm toàn bộ `#246`; cách chia không-Cargo (`anima-dell`) / Cargo (`lead`) này tiếp tục sang hàng đợi đêm `2026-11-24` (`PROMPTS_2026-11-24c_NIGHT_*`).

#### Các PR đã merge (từ `gh pr list --state merged --search "merged:>=2026-11-24"`)

Truy vấn trên GitHub lọc các PR đã merge tới snapshot thân cây `f195a361` trả về `28` PR sau (cột `Thời điểm merge (UTC)` lấy theo trường `mergedAt` của `gh`: khớp từng giây với thời điểm `%cI` của commit merge trong git ở `9/28` PR và muộn hơn `%cI` đúng `+1s` ở `19/28` PR):

| PR | Merge commit | Thời điểm merge (UTC) | Nhánh | Tiêu đề |
|---|---|---|---|---|
| #199 | `ed634d8c` | `2026-11-24T00:24:34Z` | `mac/wv2-2` | feat(worldgen): implement WV2-2 stages S4/S6/S7/S8/S9, lean 4096² pipeline, and anima-worldgen CLI |
| #230 | `56cc6ca9` | `2026-11-24T00:23:39Z` | `lead/map-d3` | docs: record owner decisions MAP-D3, QD-E3B-D1a and QD-WAVE1124 (wave authorizations) |
| #232 | `f074a243` | `2026-11-24T00:23:45Z` | `dell/d1-state-sync` | [dell] D1: reconcile STATE_OF_THE_PROJECT and TODO with trunk through #231 (STATE-SYNC) |
| #233 | `f1766846` | `2026-11-24T02:23:44Z` | `dell/d2-rel09-ci` | [dell] D2: add unsigned Release Build CI workflow for Windows & macOS installers (REL09-CI) |
| #234 | `454771e5` | `2026-11-24T00:23:51Z` | `win/e3b-runner` | feat(e3b): add C1 E3B-RUNNER experiment runner |
| #235 | `87d10aa4` | `2026-11-24T02:23:53Z` | `dell/d4-v-adr` | docs(planning): add proposed ADR-0013 lifecycle and V0/V1 specs (D4 V-ADR) |
| #236 | `ee4fc287` | `2026-11-24T04:47:49Z` | `win/wr1` | feat(core): WR1 world record format and zero-feedback gate (ADR-0008 H1) |
| #237 | `8d523415` | `2026-11-24T04:49:08Z` | `dell/d6-doc-en` | [dell] D6: translate 5 Vietnamese public pages to English with .vi.md siblings (DOC-EN) |
| #238 | `ba344eaa` | `2026-11-24T02:24:11Z` | `dell/d3-rel09-docs` | [dell] D3: add release 0.9 install guide, release notes draft, and release checklist (REL09-DOCS) |
| #239 | `b9d29ce3` | `2026-11-24T02:24:02Z` | `dell/d7-v-specs` | docs(planning): V2-V6 specs for v1.0 vertical slice (D7 V-SPECS) |
| #240 | `9eeb3130` | `2026-11-24T04:47:43Z` | `win/e3b-smoke` | E3b blind smoke triple (C2 E3B-SMOKE, seed 999953, WP-2 PASS) |
| #241 | `6ce22273` | `2026-11-24T04:47:55Z` | `dell/d1-state-sync-r2` | [dell] D1: reconcile STATE_OF_THE_PROJECT and TODO with trunk through #199/#234 (STATE-SYNC r2) |
| #242 | `21abdfba` | `2026-11-24T07:41:33Z` | `mac/wv2-3` | feat(world-v2): WV2-3 ANMW v3 reader integration, WorldIdentity sim_world_sha256, and G-SIM harness |
| #243 | `92bf0df0` | `2026-11-24T06:35:11Z` | `dell/d1-state-sync-r3` | [dell] D1: reconcile STATE_OF_THE_PROJECT and TODO with trunk 8d523415 (STATE-SYNC r3) |
| #244 | `cf3fb24a` | `2026-11-24T07:41:39Z` | `dell/d8-v-specs-2` | docs(planning): add proposed V7-V11 specs and H10/H11 release checklists (D8 V-SPECS-2) |
| #245 | `4a5ee26a` | `2026-11-24T08:47:35Z` | `win/e3b-run` | data(e3b): run 192 confirmation simulations and rescope pre-run artifact check |
| #246 | `a0f2a2da` | `2026-11-24T09:19:28Z` | `win/wr2` | feat(core): WR2 pre-allocated ring buffer, writer thread, and zero-alloc gate |
| #247 | `f976e66d` | `2026-11-24T08:47:41Z` | `win/sel2p2-draft` | docs(prereg): add SEL2-P2 preregistration draft (C5, decisions D1-D7 open) |
| #248 | `590aa032` | `2026-11-24T10:05:35Z` | `mac/decisions-1124b` | docs(workflow): record 2026-11-24 owner decisions and wave authorizations |
| #249 | `f195a361` | `2026-11-24T15:57:22Z` | `mac/wv2-4` | feat(worldgen): WV2-4 G-SHAPE + G-COMP v2 verifier & provisional C01 world (MAP-D4) |
| #250 | `071ef259` | `2026-11-24T10:52:26Z` | `dell/d9-findings-sync` | docs(findings): sync OPEN/mitigated entries and add F-067/F-068 (D9 FINDINGS-SYNC) |
| #251 | `186a60e7` | `2026-11-24T12:15:24Z` | `dell/d10-doc-review` | docs(review): audit and refresh 3 overdue documents (D10 DOC-REVIEW) |
| #252 | `6d0fdeb2` | `2026-11-24T14:14:04Z` | `win/e3b-result` | docs(e3b): C4. E3B-RESULT — unblinded confirmation verdict and RESULT.md |
| #253 | `902e505d` | `2026-11-24T14:14:10Z` | `win/law1-draft` | docs(law1): C6. LAW1-DRAFT — first law-fork experiment preregistration draft |
| #254 | `facc800c` | `2026-11-24T13:26:43Z` | `mac/fix-wr2-world-identity` | test(world-record): fix trunk test build (missing sim_world_sha256 in WR2 writer test) |
| #256 | `9b3131a0` | `2026-11-24T15:57:00Z` | `dell/d12-island-adr` | docs(adr): D12. ISLAND-ADR — ADR-0014 shared island architecture & I1-I11 cards (QD-SHAREDISLAND) |
| #257 | `6e2324d2` | `2026-11-24T15:57:06Z` | `dell/d20-audit-findings` | docs(findings): D20. AUDIT-FINDINGS — file biology-audit defects F-069..F-080 |
| #258 | `fce7b981` | `2026-11-24T15:57:13Z` | `win/sel2p2-viab2` | feat(sel2-p2): C5b. SEL2P2-VIAB2 — lock D1-D7, blind VIAB-2 probe & 12-seed results |

Ghi chú phạm vi bảng:
- PR `#231` (`1a2d1d62`, `win/e3b-freeze`, `C0` đóng băng E3b) merge lúc `2026-11-23T21:08:59Z` (`2026-11-24T04:08:59+07:00` ICT) nên nằm trước mốc ngày UTC `>=2026-11-24` của lệnh `gh pr list` và không có trong `28` dòng trên.
- Các PR còn mở tại snapshot `f195a361` hoặc tạo/merge sau `f195a361` chưa đưa vào bảng PR đã merge ở trên.

#### Chưa biết trong đợt 2026-11-24

- Phiên bản model Gemini thực tế chạy trong từng phiên hoặc từng commit Antigravity trên ba máy (`anima-case`, `anima-dell`, `anima-mac`): hướng dẫn thiết lập hub ghi `Gemini 4 Neon Extra`, nhưng Antigravity không ghi trailer vào git và hub không lưu telemetry model tự động nên không kiểm chứng được phiên bản chính xác ở từng thời điểm hay việc chuyển model.
- Phiên bản model chính xác của các báo cáo Gemini Deep Research bên ngoài repo được tham khảo trong đợt này.
- Mức độ con người (chủ dự án) đọc, chỉnh sửa tay hoặc phản hồi trực tiếp ở từng commit trước khi merge, cũng như các lượt chạy kiểm thử nội bộ không ghi lại trong PR.
- Mọi nhận xét định tính về chất lượng hay tốc độ: không có thước đo có nguồn kiểm chứng trong đợt này nên không ghi.
