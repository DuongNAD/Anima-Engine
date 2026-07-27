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
2. **Attribution đã có, văn bản license còn thiếu 1 chỗ.** Một sản phẩm proprietary vẫn phải
   attribution cho các thành phần permissive được phân phối. [`NOTICE`](../../NOTICE) (inventory 458
   thành phần) và [`licensing/`](../../licensing/) (văn bản license nguyên văn của **439/440** thành
   phần được phân phối) đều đã sinh tự động và có gate CI. Trong đó 408 đọc thẳng từ artifact đã
   cài, còn **31** lấy từ upstream tại đúng **commit bất biến** của bản phát hành, lưu nguyên byte ở
   [`licensing/upstream/`](../../licensing/upstream/) kèm manifest chứng cứ. Phần **chưa đóng được**
   còn **1** thành phần (`hexf-parse` 0.2.1) — upstream chưa từng publish văn bản license cho bản
   đó; [`licensing/UNRESOLVED.md`](../../licensing/UNRESOLVED.md) ghi đầy đủ những gì đã tìm. Đây là
   mục 3.16 trong [`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md).

## Liên kết

- Ứng viên đang được đánh giá: [khảo sát nguồn mở](../research/OPEN_SOURCE_LANDSCAPE.md).
- Luồng việc tích hợp và trạng thái thực thi:
  [kế hoạch áp dụng nguồn mở](../planning/OPEN_SOURCE_ADOPTION_PLAN.md).
- Quyết định đã chốt: [ADR index](../decisions/README.md).
