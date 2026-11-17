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
