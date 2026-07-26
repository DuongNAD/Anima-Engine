---
title: Chỉ mục quản trị
status: active
owner: maintainers
last_reviewed: 2026-07-26
review_cycle: quarterly
---

# Quản trị

Thư mục này trả lời câu hỏi **duy trì hệ thống thế nào**: tài liệu được tổ chức và review ra sao,
và một thành phần bên thứ ba được nhận vào theo quy trình nào.

| Tài liệu | Phạm vi | Trạng thái |
|---|---|---|
| [`DOCUMENTATION_POLICY.md`](DOCUMENTATION_POLICY.md) | Cấu trúc Diátaxis, metadata bắt buộc, quy tắc nguồn sự thật, ma trận thay đổi, quy trình ADR | `accepted` |
| [`OPEN_SOURCE_POLICY.md`](OPEN_SOURCE_POLICY.md) | Phân loại license, hồ sơ bắt buộc cho dependency, quy trình 9 bước nhận thành phần, xử lý sự cố | `proposed` |

## Hai điều cần biết trước khi thêm bất cứ thứ gì từ bên ngoài

1. **Anima Engine là proprietary.** [`LICENSE`](../../LICENSE) ở thư mục gốc là *all rights
   reserved*. Hệ quả trực tiếp: thành phần copyleft (GPL/AGPL) bị **chặn cứng cho mọi đường tiếp xúc
   với code** — kể cả đọc source rồi viết lại. Xem
   [chính sách nguồn mở](OPEN_SOURCE_POLICY.md).
2. **Repository chưa có `NOTICE`.** Một sản phẩm proprietary vẫn phải attribution cho các thành phần
   permissive được phân phối. Đây là mục 3.16 trong
   [`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md).

## Liên kết

- Ứng viên đang được đánh giá: [khảo sát nguồn mở](../research/OPEN_SOURCE_LANDSCAPE.md).
- Luồng việc tích hợp và trạng thái thực thi:
  [kế hoạch áp dụng nguồn mở](../planning/OPEN_SOURCE_ADOPTION_PLAN.md).
- Quyết định đã chốt: [ADR index](../decisions/README.md).
