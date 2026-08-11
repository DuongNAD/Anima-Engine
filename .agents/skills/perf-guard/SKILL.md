---
name: perf-guard
description: Proposes and validates performance optimizations, keeping only changes that beat the benchmark baseline with statistical significance and auto-reverting the rest. Use when the task mentions performance, optimization, benchmark, criterion, profiling, hot path, or slow.
---

# Perf Guard

**Module duy nhất agent được sửa code nguồn.** Điều kiện bắt buộc:
- đang ở nhánh riêng `hardening/perf-<tên>`
- đã chạy `just bench-baseline` và commit baseline
Không đủ điều kiện → từ chối task.

## Quy trình
1. `just bench-baseline` nếu chưa có baseline.
2. Tìm hot path bằng profiler (`samply`, `py-spy`). **Không đoán.**
   Không có dữ liệu profiler → dừng, báo "cần profile trước".
3. Đề xuất **đúng một** thay đổi. Viết ra trước khi code:
   - hot path nào (`file:line`)
   - % thời gian nó chiếm theo profiler
   - vì sao thay đổi này làm nó nhanh hơn
4. Sửa. Thay đổi phải **không đổi hành vi quan sát được**.
5. `just perf-gate`.
   - có benchmark chậm đi → tự revert, task fail
   - không cải thiện có ý nghĩa thống kê → tự revert, task fail
   - qua → giữ

## Cấm
- Sửa test để làm benchmark nhìn đẹp hơn
- Gộp nhiều tối ưu vào một lần đo — không phân biệt được cái nào có tác dụng
- Đổi thuật toán sang xấp xỉ mà không nói rõ trong báo cáo
- Tự chạy `git checkout` để cứu thay đổi bị gate revert

## Báo cáo
```
Hot path:    <file>:<line>  (<x>% thời gian theo profiler)
Thay đổi:    <1 câu>
Hành vi:     KHÔNG ĐỔI | ĐỔI -> <mô tả, cần người duyệt>
Kết quả:     <tên bench> <cũ> -> <mới> (<±%>)
perf-gate:   PASS | REVERTED(<lý do>)
```
