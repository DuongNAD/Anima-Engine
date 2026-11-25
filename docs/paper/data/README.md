---
title: Bảng số đo cho bài báo
status: active
owner: maintainers
last_reviewed: 2026-11-17
review_cycle: per-release
---

# Bảng số đo cho bài báo

Mỗi file là kết quả của một thẻ M, ghi đúng như đo được, kể cả khi kết quả bất lợi.

| Tài liệu | Thẻ | Câu hỏi |
|---|---|---|
| [`determinism_matrix.md`](determinism_matrix.md) | M1 | Cùng seed và manifest có cho cùng thế giới trên Windows, Linux và macOS không? |
| [`gate_controls.md`](gate_controls.md) | M2 | Mỗi cổng kiểm tra tự động có thực sự đỏ lên và gọi đúng tên lỗi khi tiêm lỗi tương ứng hay vượt xanh một cách rỗng? |
| [`scaling.md`](scaling.md) | M3 | Chi phí mỗi tick của lịch trình live thật (`LiveExperimentAdapter` / `build_tick_schedule`) tăng thế nào theo số lượng agent (`10` → `5.000`) ở cấu hình `--release`? |
| [`eu_residual.md`](eu_residual.md) | M4 | Sổ cái năng lượng đóng (`live.closed_eu_total`) có bảo toàn EU trên mọi tổ hợp cờ chính sách và mọi `InterventionKind` không? |
| [`incidents.md`](incidents.md) | P1 | Những sự cố nào đã được ghi nhận trong repo khi một gate thoát mã 0 dù có sai sót? (kèm [`incidents.csv`](incidents.csv)) |
| [`baseline-2026-08-10.txt`](baseline-2026-08-10.txt), [`baseline-2026-08-11.txt`](baseline-2026-08-11.txt) | — | Early code-size baselines written by `just baseline` (lines of code, test count) at `00e96a5` and `3a688a0`; moved here from the repo root on 2026-11-24 |
