---
name: crash-triage
description: Triages crashes, panics, hangs and undefined behavior from fuzzing artifacts or sanitizer logs into minimal reproducing tests. Use when the task mentions fuzz, fuzzing, crash artifact, panic, Miri, undefined behavior, UB, sanitizer, or a log file under logs/.
---

# Crash Triage

Nhiệm vụ là **phân loại và tái hiện**, KHÔNG phải sửa. Fix nằm trong code nguồn —
việc đó của con người.

## Đầu vào
Một file từ `just crashes` hoặc `logs/miri-<date>.log`.

## Quy trình
1. Đọc log. Trích: loại lỗi, stack frame **đầu tiên nằm trong code của repo này**
   (bỏ frame của thư viện ngoài), `file:line`.
2. Gán đúng một nhãn:
   - `UB` — sanitizer/Miri báo undefined behavior
   - `PANIC-LOGIC` — unwrap/index out of bounds trên dữ liệu **hợp lệ**
   - `PANIC-INPUT` — lỗi trên input mà API vốn không hứa nhận
   - `HANG` — timeout, không có stack
   - `DUP` — trùng crash đã có (ghi rõ trùng F-nào)
3. Viết test tái hiện **tối thiểu** trong file test regression:
   - input là hằng số viết thẳng trong test, KHÔNG đọc file artifact
   - chỉ được dùng `should_panic`/`pytest.raises` khi nhãn là `PANIC-INPUT`,
     kèm comment giải thích tại sao input đó nằm ngoài hợp đồng
   - không rút gọn được dưới 20 dòng → báo "cần minimize thêm" và dừng
4. Ghi mục mới vào `FINDINGS.md`.
5. `just verify` (không tham số).

## Cấm
- Sửa code nguồn
- Gộp nhiều crash vào một test
- Nhét dữ liệu nhị phân dài vào test

## Báo cáo
```
Artifact:  <đường dẫn>
Nhãn:      UB | PANIC-LOGIC | PANIC-INPUT | HANG | DUP(F-xxx)
Frame gốc: <file>:<line>
Test:      <file>::<tên>
FINDINGS:  F-<số>
Verify:    PASS | FAIL(<lý do>)
```
