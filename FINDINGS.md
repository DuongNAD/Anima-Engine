# FINDINGS — bug thật tìm được

Đây là metric duy nhất thật sự quan trọng. Mutation score, coverage, số warning
đều chỉ là proxy. Nếu sau một tuần file này vẫn rỗng → verifier chọn sai mục
tiêu, đổi mục tiêu chứ đừng đổ thêm token.

Quy tắc ghi:
- Chỉ ghi khi có test tái hiện được, hoặc log crash cụ thể.
- `Loại` chỉ được là một trong:
  - `BUG` — hành vi sai thật, đã chứng minh
  - `SMELL` — đáng ngờ, chưa chứng minh
  - `TEST-GAP` — code đúng, test thiếu (**không tính vào mục tiêu bug**)
- Không có `file:line` và lệnh tái hiện thì không được ghi.

---

## Mẫu (copy khối này, đừng sửa mẫu gốc)

### F-000 — <tiêu đề một dòng>
- **Ngày:** ____-__-__
- **Loại:** BUG | SMELL | TEST-GAP
- **Vị trí:** `<path>:<line>`
- **Phát hiện bởi:** mutant | fuzz | miri | determinism | dataflow | tay
- **Triệu chứng:** <1–2 câu, hành vi quan sát được sai như thế nào>
- **Tái hiện:** `just ...`
- **Test khoá lại:** `<test file>::<tên test>`
- **Trạng thái:** OPEN | FIXED (<commit>)

---

<!-- ghi mục mới ngay dưới đây -->

### F-001 — MAP-Elites chọn cha mẹ trên HashMap nên sim không tái lập được
- **Ngày:** 2026-08-11
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/evolution/map_elites.rs:14` (khai báo) và `:52` (`select_parent`)
- **Phát hiện bởi:** determinism
- **Triệu chứng:** `select_parent` gọi `self.grid.values().choose(rng)` trên
  `std::collections::HashMap`. `choose` lấy mẫu bằng cách duyệt tuần tự iterator,
  mà thứ tự duyệt HashMap phụ thuộc `RandomState` — được gieo lại từ OS **mỗi lần
  khởi động tiến trình**. Hệ quả: cùng một seed RNG, cùng một archive, hai lần
  chạy vẫn chọn ra cha mẹ khác nhau. Seed RNG một mình KHÔNG sửa được lỗi này.
  Toàn bộ nhánh tiến hoá sau đó phân kỳ.
- **Tái hiện:** trước bản vá, chạy `cargo test --release --test evolution_robustness_tests`
  nhiều lần và in ra phần tử được chọn — thứ tự đổi giữa các lần chạy.
- **Test khoá lại:** `src-tauri/tests/evolution_robustness_tests.rs::test_selection_bias_tournament`
- **Trạng thái:** FIXED — đổi `HashMap` sang `BTreeMap` (khoá `(i32,i32)` vốn đã `Ord`)

### F-002 — Mỗi file test tự lắp World riêng nên thêm resource nào cũng vỡ
- **Ngày:** 2026-08-11
- **Loại:** SMELL
- **Vị trí:** 23 chỗ `World::new()` rải trong 7 file `src-tauri/tests/`
- **Phát hiện bởi:** determinism
- **Triệu chứng:** không file test nào dùng `init_world()`; mỗi file tự chèn tập
  resource vừa đủ cho riêng nó. Thêm một resource bắt buộc vào bất kỳ system nào
  làm chúng panic **lúc chạy** chứ không phải lúc build, và thông báo lỗi
  (`Resource requested by ... does not exist`) không chỉ ra file test nào sai.
  Thêm `SimRng` đã làm vỡ 1 test kiểu này.
- **Tái hiện:** thêm một `Res<T>` mới vào `spawn_food_system` rồi `cargo test`
- **Test khoá lại:** chưa có — cần một helper `test_world()` dùng chung
- **Trạng thái:** OPEN
