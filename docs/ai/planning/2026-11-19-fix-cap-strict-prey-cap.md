---
phase: planning
feature: ecological-selection
kind: thẻ
title: Thẻ FIX-CAP (F-052) — khoá live.strict_prey_cap đếm nghiêm ngặt mỗi lần xếp hàng sinh sản
description: Bổ sung khoá opt-in live.strict_prey_cap giữ nguyên hành vi mềm lịch sử khi vắng mặt/0 để E3 đóng băng và mọi thí nghiệm trước đó tái lập từng bit, đồng thời chặn vượt trần con mồi và thú săn mồi trong cùng một tick khi bật.
status: completed
owner: anima-mac
last_reviewed: 2026-11-19
---

# Thẻ FIX-CAP (F-052) — Khoá `live.strict_prey_cap`

## 1. Lỗi gốc (`verdicts/DIAG-SEL2.md`, F-052)

- Trong `autonomous_reproduction_system` (`src-tauri/src/core/agent_systems.rs`), `prey_count` và `pred_count` chỉ được đếm **một lần** trước vòng lặp qua các cá thể mẹ đã sắp xếp (`sorted_entities`) và không hề tăng lên khi `execute_reproduction_birth` xếp hàng `SpawnOffspringCommand`.
- Hệ quả: `prey_cap` hoạt động như một *cổng mở theo tick* thay vì *trần quần thể*. Hễ dân số đứng ngay dưới trần, toàn bộ cá thể mẹ đủ điều kiện trong cùng một tick đều đồng loạt sinh con (ví dụ theo đo đạc trong `verdicts/DIAG-SEL2.md`: cấu hình nền `cap = 80` vọt lên `142–151`; `60` cá thể khởi sinh với `cap = 80` thành `120` ngay tick 1; `200` cá thể khởi sinh với `cap = 320` thành `400` ngay tick 1). Cổng thú săn mồi (`pred_count >= 8 || prey_count < pred_count * 5`) mắc đúng lỗi tương tự.

## 2. Thiết kế sửa lỗi opt-in

1. **Chính sách & khoá thực nghiệm (`resources.rs`, `live_experiment.rs`):**
   - Bổ sung trường `pub strict: bool` vào `PreyCapPolicy` (mặc định `false`; giữ nguyên chữ ký 15 tham số của `autonomous_reproduction_system` và tự động giữ parity trong `start_from_manifest_tests`). Desktop app không chèn `PreyCapPolicy` nên giữ nguyên hành vi mềm lịch sử (`strict = false`, `cap = DEFAULT_PREY_CAP = 80`).
   - Đăng ký khoá boolean chính xác `LIVE_KEY_STRICT_PREY_CAP = "live.strict_prey_cap"` trong `LIVE_KEYS` (`26 -> 27`), `LiveWorldConfig.strict_prey_cap` (`#[serde(default)] = false`), validation `0.0 / 1.0` trong `LiveWorldConfig::from_initial_conditions`, và truyền vào `PreyCapPolicy { cap, strict }` tại `insert_policy_resources`.
2. **Đếm tăng tại chỗ trong `autonomous_reproduction_system` (`agent_systems.rs`):**
   - Khi `strict_cap == true` và `execute_reproduction_birth` trả về `true` (`spawned`), tăng ngay `pred_count += 1` (nếu `is_predator`) hoặc `prey_count += 1` (nếu là con mồi).
   - Không thêm cấp phát heap nào trên tick path, không rút thêm `SimRng`, giữ nguyên thứ tự duyệt `sorted_entities`.

## 3. Lý do giữ cơ chế opt-in (`live.strict_prey_cap = 0`/vắng mặt mặc định)

- Tiền đăng ký **E3** đã đóng băng trước đó cùng toàn bộ chuỗi thực nghiệm lịch sử (`S12–S15`, `E2`, `E-SEL1`, `SEL2-P`) đều chạy trên đường dẫn cổng mềm gốc. Việc giữ mặc định `0.0`/vắng mặt = hành vi cũ bảo toàn tái lập **bit-for-bit** cho toàn bộ E3 và các manifest cũ; E3 bổ sung `"live.strict_prey_cap": 0` vào `default_keys_left_absent` kèm dòng tu chỉnh đề ngày trong tài liệu kế hoạch E3 mà không đổi bất kỳ tham số hay lần chạy nào.
- Các thực nghiệm mới (`SEL2-P2` và hậu duệ) bật `live.strict_prey_cap = 1.0` để đo quần thể thật sự bị ghìm bởi `prey_cap`, trần `8` thú săn mồi và tỷ lệ `5:1` con mồi / thú săn mồi.

## 4. Kiểm chứng (`src-tauri/tests/strict_prey_cap_tests.rs`)

- Kiểm tra đăng ký khoá, từ chối ngoài `{0.0, 1.0}` / `NaN` / `Infinity`, và tương thích ngược `serde`.
- Chứng minh với `live.strict_prey_cap = 1.0` (`60` khởi sinh / `cap = 80` qua `3.000` tick, và `100` khởi sinh / `cap = 80` sau chết qua `3.000` tick): `prey_count <= 80` tại mọi tick đã lấy mẫu, `total_births > 0`, trong khi nhánh legacy (vắng khoá) vượt trần ngay trên cùng cấu hình.
- Chứng minh vắng khoá và `0.0` cho checksum trùng khớp từng bit ở cùng tiến trình qua `1.000` tick.
- Chứng minh ở thú săn mồi: cả trần `pred_count <= 8` lẫn quy tắc tỷ lệ `5:1` (`pred_count = 3, prey_count = 17, prey_cap = 17 -> strict dừng ở 4 trong khi legacy vọt lên 6`).
