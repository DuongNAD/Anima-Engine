---
title: Chỉ mục nghiên cứu
status: active
owner: architecture
last_reviewed: 2026-07-26
review_cycle: quarterly
---

# Nghiên cứu

Thư mục này giữ **bằng chứng bên ngoài**: khảo sát, đánh giá ứng viên, nguồn khoa học và số đo
dẫn đến một quyết định. Nó không phải nguồn sự thật về hiện trạng — nguồn đó là code và
[Reference](../reference/README.md). Khi một nghiên cứu ở đây dẫn tới quyết định, quyết định được
ghi thành [ADR](../decisions/README.md), còn tài liệu nghiên cứu giữ lại lý do.

| Tài liệu | Phạm vi | Trạng thái |
|---|---|---|
| [`OPEN_SOURCE_LANDSCAPE.md`](OPEN_SOURCE_LANDSCAPE.md) | Đánh giá từng dự án nguồn mở theo hạng Adopt / Pilot / Oracle / Reference / Reject, kèm biên tích hợp và hạng rủi ro license | `accepted` · review 2026-07-26 |
| [`MAP_AND_ML_UPGRADE_RESEARCH.md`](MAP_AND_ML_UPGRADE_RESEARCH.md) | Đào sâu hai mảng: sinh thế giới/bản đồ, và mô hình machine (ML/tiến hoá) | `proposed` |

## Cách đọc

- Bảng quyết định trong khảo sát là **đánh giá kỹ thuật, không phải tư vấn pháp lý**. License phải
  được xác minh theo đúng tag/commit trước khi merge.
- Một mục ghi `Adopt` **không** có nghĩa là nó đã được thêm. Trạng thái thực thi nằm ở
  [kế hoạch áp dụng nguồn mở](../planning/OPEN_SOURCE_ADOPTION_PLAN.md); đợt review 2026-07-26 tìm
  thấy bốn mục `Adopt` chưa từng được thực thi.
- Quy trình nhập một thành phần nằm ở
  [chính sách nguồn mở](../governance/OPEN_SOURCE_POLICY.md).

## Nhịp bảo trì

Review hàng quý theo [chính sách tài liệu](../governance/DOCUMENTATION_POLICY.md): dự án upstream
còn được duy trì không, license/tag có đổi không, benchmark có còn chứng minh lợi ích không, và có
gỡ được dependency mà vẫn đọc được dữ liệu cũ không. Mỗi đợt review phải ghi **bằng chứng cho từng
thay đổi quyết định**, không sửa lặng lẽ ô trong bảng.
