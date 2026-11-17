---
title: Tài liệu lưu trữ
status: active
owner: maintainers
last_reviewed: 2026-11-17
---

# Tài liệu lưu trữ

Các file trong thư mục này chỉ dùng để truy nguyên quyết định. Chúng có thể chứa số
dòng hoặc liên kết của phiên bản code cũ.

- Không dùng archive làm nguồn triển khai.
- Không sửa archive để phản ánh kiến trúc mới.
- Mỗi file phải ghi `status: superseded` và trỏ tới nguồn thay thế.
- Link checker phát hành có thể bỏ qua nội dung link lịch sử trong archive; banner và
  `superseded_by` vẫn phải trỏ đúng. `npm run check:docs` cưỡng chế hai điều trên: `status` phải
  là `superseded`/`deprecated`, và `superseded_by` phải trỏ tới một file có thật.

## Danh mục

| Tài liệu lưu trữ | Thay bằng |
|---|---|
| [Bản nháp Claude — Creature Morphogenesis](CREATURE_MORPHOGENESIS_CLAUDE_DRAFT.md) | [`explanation/CREATURE_MORPHOGENESIS.md`](../explanation/CREATURE_MORPHOGENESIS.md) |
| [Bản nháp Claude — Creature Morphogenesis Plan](CREATURE_MORPHOGENESIS_PLAN_CLAUDE_DRAFT.md) | [`planning/CREATURE_MORPHOGENESIS_PLAN.md`](../planning/CREATURE_MORPHOGENESIS_PLAN.md) |
| [Kế hoạch thực thi Phase 1 (M3–M7)](PHASE1_EXECUTION_PLAN.md) — từng là `plan.md` ở gốc repo | [`planning/STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md) |
| [Báo cáo bàn giao Phase 6 (T13)](PHASE6_HANDOFF_REPORT.md) — từng là `handoff.md` ở gốc repo | [`planning/STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md) |
| [Báo cáo kiểm toán mã nguồn 2026-08-14](AUDIT_REPORT.md) — từng là `AUDIT_REPORT.md` ở gốc repo. Khuyến nghị SIM-01 của nó **sai** và đã bị bác | [`ai/monitoring/audit-triage-2026-11-14.md`](../ai/monitoring/audit-triage-2026-11-14.md) |
