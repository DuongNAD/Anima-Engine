---
name: determinism-fixer
description: Makes a codebase reproducible by replacing unseeded RNG with explicitly seeded generators and removing wall-clock and hash iteration order dependencies. Use when the task mentions determinism, reproducibility, seed, thread_rng, flaky test, or golden hash.
---

# Determinism Fixer

Đây là skill **duy nhất** được phép sửa code nguồn, và chỉ trên nhánh riêng
`hardening/phase0-determinism`. Ngoài nhánh đó, từ chối task.

## Phạm vi mỗi task
**Đúng một call site.** Prompt liệt kê nhiều → làm cái đầu tiên, báo cáo, dừng.

## Quy trình
1. `just seed-audit` để lấy danh sách nguồn phi tất định còn lại.
2. Đọc hàm chứa call site. Xác định RNG nên đến từ đâu:
   - hàm là system/handler → lấy từ resource/context dùng chung
   - hàm thuần → thêm tham số rng vào cuối chữ ký
3. Sửa **một** call site. Cập nhật **mọi** caller. Không để lại call site cũ.
4. `just test` phải xanh y như trước khi sửa. Số test pass phải BẰNG, không ít hơn.
5. `just flaky 5` phải 5/5.

## Quy ước cứng
- Seed truyền tường minh từ config, giá trị mặc định = `HD_SEED`.
- Không bao giờ `from_entropy()` / `seed()` không tham số trong đường chạy chính.
- Map/set có thứ tự duyệt ảnh hưởng state → đổi sang cấu trúc có thứ tự, hoặc
  sort trước khi duyệt. Ghi rõ đã chọn cách nào.
- Wall-clock trong đường chạy chính → thay bằng tick counter.
  Trong log/telemetry thì giữ nguyên, chỉ ghi chú.

## Cấm
- Sửa quá một call site trong một task
- Đổi hành vi thuật toán — chỉ đổi *nguồn* ngẫu nhiên, không đổi phân phối
- Xoá hoặc `#[ignore]` test đang fail

## Báo cáo
```
Call site:      <file>:<line>
Cách sửa:       resource | tham số | cấu trúc có thứ tự | tick counter
Caller cập nhật: <danh sách file:line>
just test:      PASS(<n> test) | FAIL(<test nào>)
just flaky 5:   5/5 | <n>/5
```
