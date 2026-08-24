---
title: Tài liệu lưu trữ
status: active
owner: maintainers
last_reviewed: 2026-07-24
---

# Tài liệu lưu trữ

Các file trong thư mục này chỉ dùng để truy nguyên quyết định. Chúng có thể chứa số
dòng hoặc liên kết của phiên bản code cũ.

- Không dùng archive làm nguồn triển khai.
- Không sửa archive để phản ánh kiến trúc mới.
- Mỗi file phải ghi `status: superseded` và trỏ tới nguồn thay thế.
- Link checker phát hành có thể bỏ qua nội dung link lịch sử trong archive; banner và
  `superseded_by` vẫn phải trỏ đúng.
