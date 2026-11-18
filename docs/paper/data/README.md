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
| [`scaling.md`](scaling.md) | M3 | Chi phí mỗi tick của lịch trình live thật (`LiveExperimentAdapter` / `build_tick_schedule`) tăng thế nào theo số lượng agent (`10` → `5.000`) ở cấu hình `--release`? |
