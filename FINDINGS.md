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
- **Phạm vi thật (người kiểm bổ sung):** không chỉ một dòng. `mark_offline()` cũng
  equivalent vì nó chỉ đặt `is_online=false` (vốn đã false) và gán `neo4j_graph=None`
  (field private, chỉ đọc trong nhánh `if self.is_online()` không bao giờ chạy).
  Mọi mutant trong nhánh online của `FallbackLineageTracker` đều cùng loại này.
- **Đã xử lý:** thêm `exclude_re` vào `src-tauri/.cargo/mutants.toml` kèm lý do, để
  chúng không sinh task rác mỗi lượt quét. Gỡ ra khi nào struct nhận được mock qua DI.
- **Trạng thái:** OPEN

### F-003 — Luật xử lý hoà fitness trong MAP-Elites chưa từng được quyết định
- **Ngày:** 2026-08-11
- **Loại:** TEST-GAP
- **Vị trí:** `src-tauri/src/evolution/map_elites.rs:43` (`add_individual`)
  và `:73` (`select_parent`)
- **Phát hiện bởi:** mutant
- **Triệu chứng:** cả hai chỗ dùng `>`; đổi thành `>=` thì không test nào phát
  hiện. Nghĩa là hành vi khi hai cá thể **hoà fitness** — giữ cá thể cũ hay thay
  bằng cá thể mới — chưa ai chọn có ý thức, nó chỉ là mặc định của lần gõ đầu.
  Với MAP-Elites, thay khi hoà làm archive xoay vòng giữa các cá thể ngang tài,
  khiến lineage nhiễu mà chất lượng không tăng; tournament lấy ứng viên bốc sau
  thì thiên vị theo thứ tự bốc thay vì theo fitness.
- **Tái hiện:** `just verify "src/evolution/map_elites.rs:43:35: replace > with >= in MapElitesArchive::add_individual"`
- **Test khoá lại:**
  `tests/map_elites_tests.rs::add_individual_hoa_fitness_thi_giu_ca_the_cu`
  `tests/map_elites_tests.rs::select_parent_hoa_fitness_thi_giu_ung_vien_truoc`
- **Trạng thái:** FIXED — hành vi hiện tại (giữ cái cũ / giữ ứng viên trước) nay
  đã được khoá bằng test, không còn đổi ngầm được

### F-004 — FallbackLineageTracker::is_online không thể test được vì thiếu mock/DI cho Neo4j
- **Ngày:** 2026-08-11
- **Loại:** TEST-GAP
- **Vị trí:** `src-tauri/src/evolution/lineage.rs:186` và `:190` — và **cả nhánh online** của struct
- **Phát hiện bởi:** mutant
- **Triệu chứng:** `FallbackLineageTracker` chỉ chuyển sang `is_online == true` khi kết nối thành công tới dịch vụ Neo4j thật qua mạng. Trong môi trường unit test (không có server Neo4j), `is_online` luôn giữ giá trị mặc định `false`. Do đó, đột biến `replace FallbackLineageTracker::is_online -> bool with false` trả về `false` hoàn toàn trùng khớp với giá trị thực tế trong môi trường test, không thể tạo ra hành vi quan sát được sai từ API công khai nếu không refactor code nguồn để hỗ trợ injection/mocking.
- **Tái hiện:** `just verify "src/evolution/lineage.rs:187:9: replace FallbackLineageTracker::is_online -> bool with false"`
- **Test khoá lại:** chưa có — cần refactor `FallbackLineageTracker` (thêm constructor nhận status mock hoặc trait `Neo4jClient`) để test được.
- **Trạng thái:** OPEN

### F-005 — Ba cặp phụ thuộc vòng giữa các module lõi
- **Ngày:** 2026-08-11
- **Loại:** SMELL
- **Vị trí:** `src-tauri/src/{ai,core,physics}/`
- **Phát hiện bởi:** dataflow
- **Triệu chứng:** đồ thị phụ thuộc thật (`cargo modules dependencies --lib`) cho
  thấy ba cặp trỏ lẫn nhau: `ai ↔ core`, `ai ↔ physics`, `core ↔ physics`.
  Nghĩa là không module nào trong ba module lõi có thể test, đọc hay thay thế độc
  lập với hai module kia. Đây cũng là lý do F-002 khó sửa: muốn có `test_world()`
  dùng chung thì phải quyết định nó thuộc về đâu, mà cả ba đều phụ thuộc nhau.
- **Tái hiện:** `just dataflow-truth && just dataflow-verify`
- **Test khoá lại:** chưa có — cần khai contract phụ thuộc rồi cho CI chặn
- **Trạng thái:** OPEN

### F-006 — get_bin_coords nuốt NaN và gán vào ô gốc, im lặng
- **Ngày:** 2026-08-11
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/evolution/map_elites.rs:29` (`get_bin_coords`)
- **Phát hiện bởi:** fuzz (giả thuyết fuzz, kiểm bằng tay vì cargo-fuzz bị chặn — xem F-007)
- **Triệu chứng:** `(f / resolution).floor() as i32` với `f = NaN` cho ra `0`, vì
  `as i32` trong Rust là saturating cast và NaN quy về 0. Kết quả: mọi vector đặc
  trưng chứa NaN đều rơi vào **ô (0,0)** — cùng ô với gốc toạ độ hợp lệ. Một agent
  có đặc trưng hoá NaN sẽ tranh ô với một elite thật và có thể đẩy nó ra nếu
  fitness cao hơn. Không panic, không log, không test nào bắt được.
  Kèm theo: mọi giá trị ≥ ~2^31 × resolution đều bão hoà về cùng một ô biên, nên
  các hành vi rất khác nhau bị gộp làm một.
- **Tái hiện:**
  ```rust
  let a = MapElitesArchive::new(0.5);
  assert_eq!(a.get_bin_coords(&[f32::NAN, f32::NAN]), a.get_bin_coords(&[0.0, 0.0]));
  ```
- **Test khoá lại:** chưa có
- **Đề xuất sửa:** `get_bin_coords` trả `Option<(i32,i32)>` và trả `None` khi đặc
  trưng không hữu hạn; `add_individual` bỏ qua cá thể như vậy và đếm vào một
  counter để thấy được. Sửa im lặng bằng cách clamp là giấu lỗi, không phải sửa.
- **Trạng thái:** OPEN — chưa chứng minh được NaN thật sự phát sinh trong đường
  chạy sim, nhưng hành vi khi nó phát sinh thì sai rõ ràng.

### F-007 — Logic sim hàn chặt vào vỏ Tauri, chặn cả fuzz lẫn Miri
- **Ngày:** 2026-08-11
- **Loại:** SMELL
- **Vị trí:** `src-tauri/src/lib.rs:352` — `.run(tauri::generate_context!())`
- **Phát hiện bởi:** fuzz
- **Triệu chứng:** `cargo fuzz build` thất bại vì `generate_context!` cần toàn bộ
  pipeline build của Tauri. Cùng gốc rễ khiến `cargo miri` không chạy được (Miri
  không chạy FFI). Hệ quả: **hai lớp công cụ tìm bug mạnh nhất đều không dùng
  được**, và mỗi mutant phải build lại cả tauri + wgpu + burn.
- **Tái hiện:** `cd src-tauri && cargo +nightly fuzz build bin_coords`
- **Đề xuất sửa:** tách logic thuần (`evolution`, `physics`, `ai/cpg`) ra crate
  `anima-core` không phụ thuộc Tauri. Mở khoá được ba thứ cùng lúc: fuzz, Miri,
  và mutation testing nhanh hơn nhiều lần.
- **Trạng thái:** OPEN
