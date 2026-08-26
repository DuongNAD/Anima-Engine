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

### F-023 — `adversarial_coverage_tests` đỏ flaky: counter toàn tiến trình bị test anh em nhiễu
- **Ngày:** 2026-08-26
- **Loại:** BUG
- **Vị trí:** `src-tauri/tests/adversarial_coverage_tests.rs:138,172,276` (điểm mở cửa sổ đo); gốc rễ:
  `src-tauri/tests/common/allocator.rs` — `TrackingAllocator::start_tracking()` đếm **toàn tiến trình**
- **Phát hiện bởi:** determinism (full `just test`, chạy release không feature)
- **Triệu chứng:** `test_spatial_grid_rebuild_allocates_on_new_cell` failed 1 lần trong full suite với
  `Expected zero allocations on out of bounds insert — left: 4, right: 0`; chạy riêng file đó thì xanh
  7/7 ở CẢ cấu hình desktop lẫn trần. Nguyên nhân đúng như doc comment của `allocator.rs` đã cảnh báo:
  libtest cho mỗi `#[test]` một thread, `TEST_LOCK` chỉ tuần tự hoá thân một phần test, và counter toàn
  tiến trình gán mọi allocation trong cửa sổ đo — kể cả của thread test khác đang chạy song song.
  Đây là một trong các suite "latent" mà tài liệu nêu tên: đường đo đơn-thread nhưng vẫn dùng counter
  rộng. Cả ba đường được đo (`rebuild_spatial_grid_system` ×2, `combat_system`) đều không nhả worker
  thread (không rayon/spawn trong `physics/spatial.rs`), nên chế độ per-thread là đúng policy, không
  phải nới lỏng — assertion `allocs == 0` giữ nguyên.
- **Tái hiện:** lặp `cd src-tauri && cargo test --release` đủ nhiều lần trên máy đang load → fail rải rác
  ở một trong ba test đo allocation của file này; `cargo test --release --test adversarial_coverage_tests`
  đứng riêng thì xanh ổn định.
- **Test khoá lại:** `src-tauri/tests/adversarial_coverage_tests.rs` — ba cửa sổ đo chuyển sang
  `ALLOCATOR.start_tracking_this_thread()`; assertion giữ nguyên.
- **Trạng thái:** FIXED (commit kèm theo trên `feat/ecological-selection`)

### F-022 — Cổng `verify` tự hủy im lặng khi diff không chứa file test nào
- **Ngày:** 2026-08-26
- **Loại:** BUG
- **Vị trí:** `scripts/verify.sh:41`
- **Phát hiện bởi:** tay (`bash -x ./scripts/verify.sh`, trace dừng đúng tại dòng 41, EXIT=1, không một byte output)
- **Triệu chứng:** khi cây làm việc không có thay đổi nào dưới `src-tauri/**/tests/` (ví dụ: kiểm tra lại một trạng thái ĐÃ commit, hoặc diff chỉ chạm tài liệu/nguồn), `grep -E "$GUARD_RE"` trong pipeline của `GUARD_DIRS` không khớp dòng nào → exit 1. Với `set -o pipefail` + `set -e` và pipeline KHÔNG có `|| true` (khác mọi pipeline khác trong cùng script), phép gán thất bại và script thoát ngay với mã 1, **trước cả luật 2**, không in `TU CHỐI` và cũng không bao giờ tới được `QUA CONG`. Hệ quả: cổng chống gian lận không thể phán "đạt" cho bất kỳ công việc nào mà diff của nó không kèm file test mới — kể cả các việc hợp lệ như sửa tài liệu hay commit lại file nguồn bị thiếu.
- **Tái hiện:** commit sạch toàn bộ thay đổi (hoặc đứng ở HEAD sau 893098b) rồi chạy `just verify` → exit 1, stdout/stderr rỗng. So sánh: trước khi commit, cùng lệnh in `TU CHỐI: ...` bình thường vì law 1 chặn sớm trước dòng 41.
- **Đề xuất sửa (cho chủ dự án — `scripts/` nằm ngoài vùng ghi của agent):** thêm `|| true` cho phép gán ở dòng 41, giống cách đã làm với `CHANGED`/`BAD`/`NONKIT`; hoặc đổi thành `grep ... | sed ... | sort -u || true`. Không thay đổi ngữ nghĩa luật nào.
- **Trạng thái:** OPEN

### F-018 — Sinh thái không nối vào chọn lọc: cả crate có đúng một fitness và nó là quãng đường
- **Ngày:** 2026-08-25
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/core/agent_systems.rs:528` (định nghĩa fitness duy nhất),
  `src-tauri/src/core/world_systems.rs:171` (`tick_count` tăng vô điều kiện)
- **Phát hiện bởi:** tay (khảo sát toàn bộ mã nguồn, lần theo dữ liệu thay vì theo tài liệu)
- **Triệu chứng:** `fitness = cumulative_distance + tick_count`, và `tick_count` tăng mỗi tick cho
  mọi agent có `FeatureTracker` bất kể trạng thái sinh lý. Sau một epoch, `tick_count` gần như bằng
  nhau ở mọi cá thể, nên xếp hạng fitness rút gọn thành xếp hạng quãng đường. Toàn bộ tầng sinh
  thái — NPP theo biome, Kleiber, Boltzmann, Holling II/III, Lindeman, mùa vụ — **không có đường
  nào** chạm tới hàm này. Một cá thể ăn no và một cá thể kiệt sức từ tick 300 có cơ hội truyền gene
  bằng nhau nếu đi được quãng đường như nhau.
- **Tái hiện:** `grep -rn 'fitness =' src-tauri/src --include='*.rs'` → đúng 1 kết quả gán.
- **Test khoá lại:** chưa có. Gate đề xuất: **CA-S01** ở
  `docs/ai/testing/2026-08-25-feature-ecological-selection.md` — đổi một tham số sinh thái phải đổi
  phân bố tính trạng, và cùng test đó phải **không** đổi trên chế độ fitness cũ.
- **Trạng thái:** OPEN — feature `ecological-selection` nhánh F.

### F-019 — Không có cái chết: `energy <= 0` chỉ `continue`, nên không có áp lực chọn lọc nào
- **Ngày:** 2026-08-25
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/core/agent_systems.rs:452`
- **Phát hiện bởi:** tay (cùng lượt khảo sát với F-018)
- **Triệu chứng:** cạn năng lượng hoặc cạn nước không kết thúc gì — hệ đánh giá chỉ `continue`, agent
  vẫn đi, vẫn trao đổi chất, vẫn được thay thế đúng lịch ở cuối epoch. Không tử vong nghĩa là không
  có sức chứa môi trường thật, không bùng–sụp, không Red Queen, không phân hoá r/K. Đáng chú ý:
  cơ chế trả EU của xác về `detritus` (`ReclaimAndDespawnAgentCommand`,
  `src-tauri/src/core/agent_systems.rs:112-152`) **đã viết xong, nguyên tử, và đã có gate** — thứ
  thiếu chỉ là điều kiện gọi nó.
- **Tái hiện:** `grep -rn 'energy <= 0' src-tauri/src --include='*.rs'`
- **Test khoá lại:** chưa có. Gate đề xuất: **CA-S02** — NPP = 0 phải làm quần thể sập về 0, và với
  tử vong tắt thì **không** sập.
- **Trạng thái:** OPEN — feature `ecological-selection` nhánh M.
- **Ghi chú ranh giới:** invariant `"starvation must not kill"` tại
  `src-tauri/src/core/aggregate_population.rs:1653` nói về quần thể tổng hợp AE3, một mô hình khác
  có hợp đồng khác. Nó **không** phải mục tiêu sửa của finding này.

### F-020 — `AgentEvaluation` tính đúng đại lượng cần cho fitness sinh thái rồi bị vứt đi
- **Ngày:** 2026-08-25
- **Loại:** SMELL
- **Vị trí:** `src-tauri/src/core/agent_systems.rs:523` (bind `_eval, _homeo`),
  `:452-462` (`survival_ticks` có điều kiện sống)
- **Phát hiện bởi:** tay
- **Triệu chứng:** `AgentEvaluation.survival_ticks` chỉ tăng khi `energy > 0 && hydration > 0` — tức
  đúng nghĩa "số tick sống được", đại lượng mà một fitness sinh thái cần, và nó đã được tính mỗi
  tick từ trước. Nhưng `check_epoch_completion_system` bind nó thành `_eval` rồi đọc `FeatureTracker`
  thay vì nó. Đây là lý do F-018 rẻ để sửa: phần khó đã tồn tại, chỉ là không ai đọc.
- **Tái hiện:** đọc `src-tauri/src/core/agent_systems.rs:523` và `:452`.
- **Test khoá lại:** chưa có. Gate đề xuất: **FI-S02**.
- **Trạng thái:** OPEN — feature `ecological-selection` nhánh F, task F-3.

### F-021 — Trục MAP-Elites thứ hai bằng đúng hàm fitness trừ một hằng số
- **Ngày:** 2026-08-25
- **Loại:** BUG
- **Vị trí:** `src-tauri/src/core/agent_systems.rs:531` (`foraging_range = cumulative_distance`) so với
  `:528` (`fitness = cumulative_distance + tick_count`); mô tả trục tại
  `src-tauri/src/core/ecology.rs:161-174`
- **Phát hiện bởi:** tay
- **Triệu chứng:** MAP-Elites hoạt động được nhờ mô tả hành vi **độc lập** với fitness; đó là cách nó
  soi sáng một không gian thay vì leo một đỉnh. Ở đây trục 2 là fitness trừ một hằng số, nên kho
  thực chất chỉ khám phá **một** chiều (khối lượng cơ thể). Kho không hỏng — nó làm ít hơn một nửa
  việc mà doc comment tại `ecology.rs:163-168` mô tả ("rewards ECOLOGICAL diversity … instead of a
  single locomotion optimum"). Chưa đối chứng: `FORAGING_REFERENCE = 200.0` có `clamp`, nên trục còn
  có thể bão hoà — chưa đo.
- **Tái hiện:** `sed -n '526,532p' src-tauri/src/core/agent_systems.rs`
- **Test khoá lại:** chưa có. Gate đề xuất: **QD-S02** — tương quan hạng giữa trục 2 và fitness phải
  **không** đơn điệu trên một quần thể fixture.
- **Trạng thái:** OPEN — feature `ecological-selection` nhánh Q. Sửa F-018 trước; nếu công thức
  fitness mới không chứa quãng đường thì finding này đóng theo.

### F-022 — Bảy hệ thống chạy O(agent × segment) trong khi mẫu sửa đúng đã có sẵn trong cùng file
- **Ngày:** 2026-08-25
- **Loại:** SMELL
- **Vị trí:** `src-tauri/src/core/world_systems.rs:121, 236, 248, 337, 436, 454`,
  `src-tauri/src/core/agent_systems.rs:468, 877, 890`,
  `src-tauri/src/core/environmental_systems.rs:282`
- **Phát hiện bởi:** tay
- **Triệu chứng:** các hệ thống lặp **toàn bộ** segment trong thế giới ở **bên trong** vòng lặp agent,
  lọc bằng `if parent.0 == agent_entity`. Với 10 agent (mặc định) điều đó vô hại; với 1000 agent × 3
  segment, riêng `metabolic_decay_system` là 3 triệu vòng mỗi tick. `sensory_system` còn quét tuyến
  tính toàn bộ danh sách thức ăn và mồi cho **từng** agent, trong khi `SpatialHashGrid` đã được dựng
  lại mỗi tick và đang được dùng cho raycast **ngay bên dưới trong cùng hàm**. Mẫu sửa đúng đã có
  trong repo: `combat_system` (`world_systems.rs:300-340`) dựng `centroid_index: Local<EntityHashMap>`
  một lần rồi quét segment đúng một lượt — O(A + S), không cấp phát trong tick path.
- **Tái hiện:** `grep -rn 'segment_query.iter()' src-tauri/src --include='*.rs'`
- **Test khoá lại:** chưa có. Gate đề xuất: **PF-S04** — thời gian mỗi tick tăng **tuyến tính** theo
  số agent, không bậc hai; và **PF-S01** — cùng seed cho cùng checksum trước và sau (refactor thuần).
- **Trạng thái:** OPEN — feature `ecological-selection` nhánh H.
- **Bẫy khi sửa:** `EntityHashMap` không duyệt theo thứ tự chèn. Chỗ nào kết quả phụ thuộc thứ tự
  duyệt (ai giành miếng ăn cuối trong một ô) phải sắp xếp tường minh, nếu không checksum sẽ đổi.

### F-017 — `scripts/check_test_targets.mjs` đã commit với ba khối merge conflict còn nguyên, gate xác minh chết ngay khi chạy
- **Ngày:** 2026-08-25
- **Loại:** BUG
- **Vị trí:** `scripts/check_test_targets.mjs:78` (`<<<<<<< ours`), `:103/:108`
  (`=======`, khối hai), `:155/:273` (khối ba) — cả ba khối conflict đều nằm
  trong bản đã commit trên nhánh `feat/living-world`.
- **Phát hiện bởi:** tay (chạy cổng xác minh của living-world sau khi xong M3/F1).
- **Triệu chứng:** script chết ngay khi parse — `node scripts/check_test_targets.mjs`
  ném `SyntaxError: Unexpected token '<<'`. Hệ quả kép: (1) cổng
  "check_test_targets" của vòng lặp xác minh không chạy được chút nào; (2)
  `npm run lint` báo `Parsing error: Merge conflict marker encountered` cho file
  này, góp phần làm rổ eslint gate. Ngoài ra cùng lần lint đó, `playground/` và
  `tests/frontend/` có sẵn 9 lỗi eslint khác (chủ yếu `no-useless-assignment`,
  `@ts-ignore`) cũng đã commit — baseline ratchet 0 không phản ánh cây mã hiện tại.
- **Tái hiện:** `node scripts/check_test_targets.mjs /tmp/anima-test.txt --profile desktop`
  → SyntaxError; `npm run lint` → 82 problems.
- **Test khoá lại:** không áp dụng được — `scripts/` cấm sửa theo AGENTS.md.
- **Cách sửa đề xuất cho owner:** resolve ba khối conflict trong
  `check_test_targets.mjs` (giữ nhánh `theirs` có chú thích PowerShell-wrap vì
  đó là bản mới hơn có đo đạc), rồi chạy lại `npm run lint`; quyết định riêng
  về 9 lỗi eslint tồn đọng ở `playground/` + `tests/frontend/`.
- **Trạng thái:** OPEN — chờ owner sửa `scripts/`.

### F-014 — Không thể quét mutation cho anima-core: crate nằm trong workspace cha nhưng không thuộc members/exclude
- **Ngày:** 2026-08-24
- **Loại:** BUG
- **Vị trí:** `src-tauri/anima-core/Cargo.toml` (thiếu bảng `[workspace]`) và/hoặc
  `src-tauri/Cargo.toml:members = ["crates/*"]` (không chứa `anima-core`)
- **Phát hiện bởi:** tay (`just hunt-file src/mutation.rs 2 mutation_operator_tests,mutation_invariants anima-core`)
- **Triệu chứng:** mọi lệnh cargo chạy trong `src-tauri/anima-core` đều chết ngay:
  `cargo locate-project failed: current package believes it's in a workspace
  when it's not`. Hệ quả: không tái tạo được quét mutation cho anima-core —
  `mutants.out/missed.txt` (25 dòng, 22:33) là dữ liệu cũ từ TRƯỚC khi
  `tests/mutation_operator_tests.rs` được thêm; header file test này khẳng định
  nó giết đúng 25 mutant đó nhưng KHÔNG có bằng chứng tươi. Mutation score của
  anima-core hiện không thể đo = số đo nói dối.
- **Tái hiện:** `just hunt-file src/mutation.rs 2 "" anima-core` → error trên.
- **Test khoá lại:** không áp dụng được — cả hai file đều là config build, cấm
  sửa theo AGENTS.md.
- **Cách sửa đề xuất cho owner** (một trong hai):
  (a) thêm bảng `[workspace]` rỗng vào cuối `src-tauri/anima-core/Cargo.toml`
  (tách khỏi workspace cha — đúng tinh thần "crate thuần, build độc lập"), hoặc
  (b) thêm `"anima-core"` vào `workspace.exclude` của `src-tauri/Cargo.toml`.
  Sau khi sửa, chạy lại: `just hunt-file src/mutation.rs 2
  mutation_operator_tests,mutation_invariants anima-core` để có missed.txt tươi.
- **Trạng thái:** OPEN — chờ owner sửa config.
- **FIXED (2026-08-24):** owner chỉ thị "làm nốt" → chọn phương án (a): thêm
  bảng `[workspace]` rỗng (kèm chú thích) vào `src-tauri/anima-core/Cargo.toml`.
  Không ảnh hưởng build crate chính (src-tauri không phụ thuộc anima-core).
  Quét tươi mở khoá — kéo theo phát hiện F-015.

### F-015 — anima-core không biên dịch được sau merge 157f85d: import path stale + validator tham chiếu ngược crate cha
- **Ngày:** 2026-08-24
- **Loại:** BUG
- **Vị trí:** `src-tauri/anima-core/src/crossover.rs:1-2`, `src/mutation.rs:1`,
  `src/map_elites.rs:1,82` (path `crate::evolution::…` / `crate::core::…` không
  tồn tại trong crate tách rời), và `tests/evolution_robustness_tests.rs:1-4`,
  `tests/map_elites_tests.rs:1-6` (import nhầm `anima_engine_lib` — crate cha),
  `tests/crossover_logic_tests.rs:45,93,141,219,258` (dùng `child.nodes` trên
  `Result` — viết cho API cũ trả thẳng genotype).
- **Phát hiện bởi:** mutation scan (F-014 vừa mở khoá thì baseline build chết:
  `ERROR cargo build failed in an unmutated tree`).
- **Triệu chứng:** 5 lỗi E0433 ở lib + 13+ lỗi ở test → toàn bộ fuzz/miri/
  mutation của anima-core tê liệt từ merge 157f85d; mutation score "70%" trước
  đó không thể tái tạo. `map_elites.rs:82` gọi
  `crate::core::components::validate_morphology_payload` — hàm này là
  `pub(crate)` ở crate cha, không thể gọi chéo (sẽ đảo dependency).
- **Tái hiện:** `just hunt-file src/mutation.rs 2 "" anima-core` (trước fix) →
  E0433 hàng loạt.
- **Cách sửa (owner ủy quyền qua chỉ thị "làm nốt đi" sau khi F-014 được báo):**
  1. Sửa import path về module top-level của anima-core (`crate::genotype`,
     `crate::mutation`).
  2. Port `validate_morphology_payload` thành
     `anima-core/src/components.rs` (thuần, trả `Result<(), String>`, giữ
     `MAX_MORPHOLOGY_NODES = 128` đồng bộ bản gốc); call site đổi sang
     `crate::components::…`.
  3. Sửa test: thêm `.expect()` cho API `Result` mới; đổi import về
     `anima_core::`; derive `PartialEq` cho 3 struct genotype (additive);
     thêm `[dev-dependencies] serde_json` cho test round-trip JSON.
  4. Viết thêm 6 test khoá 17 mutant sống (xem dưới).
- **Kết quả mutation (quét tươi, chỉ test 2 suite mutation):** 83 mutant của
  `src/mutation.rs`: 76 caught, 7 unviable, **0 missed** — 100% mutant khả thi
  bị giết (test mới: `mutation_operator_tests.rs::op1_giu_nguyen_anchor…`,
  `op2_phat_anchor…`, `op3_khong_them_node…`, `op4_khong_bo_node…`,
  `op3_anchor_z_cua_con…`, `loi_node_id_exhausted…`).
- **Bài học:** khi tách crate, phải chạy `cargo check` trên crate MỚI ngay —
  copy file giữ nguyên path cũ thì mọi cổng phía sau đều đo trên xác chết.
- **BỔ SUNG (2026-08-24) — quét TOÀN crate:** `just hunt-file src 2 '' anima-core`
  (81 mutant): lần đầu 26 missed → viết test khoá lại cho crossover/map_elites/
  components → còn 4 → phân tích thêm → **0 missed**. Chi tiết các test mới:
  `crossover_logic_tests::graft_subtree_giu_do_thi_hop_le_tren_seed_sweep`,
  `map_elites_tests::from_saved_tu_choi_*` (3 test),
  `map_elites_tests::add_individual_khong_thay_the…`,
  `map_elites_tests::select_parent_replay_khop_mo_phong_rut`,
  `components_validation_tests.rs` (4 test). Mutant cuối cùng là equivalent —
  xem F-016.
- **Trạng thái:** FIXED (commit kèm biên bản này).

### F-016 — Mutant "&&"→"||" ở prune cạnh khi ghép subtree là tương đương trên miền input là cây
- **Ngày:** 2026-08-24
- **Loại:** SMELL
- **Vị trí:** `src-tauri/anima-core/src/crossover.rs:121`
  (`child.edges.retain(|e| !v_sub_ids.contains(&e.source_node) && !v_sub_ids.contains(&e.target_node))`)
- **Phát hiện bởi:** mutant (sống sót 3 vòng test sweep 128 seed)
- **Triệu chứng:** thoạt nhìn `&&`→`||` giữ thêm cạnh lơ lửng vào node đã bị
  xoá → phá validity. Nhưng: (1) `child` tại điểm prune luôn là CÂY (xây từ
  hai cây), (2) cạnh incoming vào gốc subtree `v` đã bị `edges.remove()` ở
  dòng 113 TRƯỚC khi retain. Trên cây, cạnh duy nhất cắt qua ranh giới một
  subtree chính là cạnh vào gốc của nó → sau khi bỏ cạnh đó, không còn cạnh
  nào chạm v_sub ngoài các cạnh nội bộ → `!A||!B` và `!A&&!B` chọn cùng tập
  cạnh. Không tồn tại test hành vi phân biệt được trên miền input hiện có.
- **Tái hiện:** `just hunt-file src 2 '' anima-core` → mutant trên báo Missed
  (trước khi skip).
- **Test khoá lại:** không có — không thể có trên miền input là cây.
- **Đã xử lý:** `just skip 'crossover\.rs:121:57: replace && with \|\| in
  crossover_genotypes' …` thêm vào `exclude_re` của
  `src-tauri/anima-core/.cargo/mutants.toml`.
- **Điều kiện gỡ ra:** khi `child` tại điểm prune cho phép DAG/nhiều cạnh vào
  cùng subtree (không còn là cây đơn) — khi đó cross-edge nhiều hơn một và
  mutant PHẢI bị giết bằng test.
- **Trạng thái:** OPEN (đã loại khỏi mutation gate, chờ điều kiện gỡ)



### F-013 — Mutant "replace + with -" ở z_down wrap toroidal KHÔNG tương đương, đã giết bằng test hành vi
- **Ngày:** 2026-08-24
- **Loại:** TEST-GAP
- **Vị trí:** `src-tauri/src/ai/pheromone.rs:196` (`let z_down = (z + 1) % GRID_SIZE;`;
  chuỗi mutant đầy đủ sau merge 157f85d:
  `src/ai/pheromone.rs:196:25: replace + with - in update_pheromone_grid_system`.
  `missed.txt` cũ ghi `197:29` là số dòng TRƯỚC merge — đã lỗi thời.)
- **Phát hiện bởi:** mutant (`src/ai/pheromone.rs:196:25: replace + with - in update_pheromone_grid_system` sống sót trong quét trước merge)
- **Triệu chứng:** khác với mutant tương đương ở dòng 190 (F-011), mutant này
  **phân biệt được**: đổi `(z + 1)` thành `(z - 1)` thì với z = 0, `(0 - 1)`
  usize underflow wrap thành `2^64 - 1 ≡ 127 (mod 128)`, nên ô (z=0) đọc láng
  giềng `down` từ hàng 127 thay vì hàng 1. Không có test hành vi nào đặt
  pheromone lệch để phân biệt hai hướng wrap theo trục z.
- **Tái hiện:** `cd src-tauri && cargo mutants --file src/ai/pheromone.rs --baseline skip`
  → mutant trên báo Missed.
- **Test khoá lại:** `src-tauri/tests/pheromone_tests.rs::test_pheromone_diffusion_z_wraps_forward_not_backward_at_z_zero`
  (đặt 10.0 chỉ tại hàng 1, chạy system 1 tick, assert ô (z=0,x=0) = 1.0;
  mutant cho đúng 0.0).
- **Trạng thái:** FIXED bằng test (chờ `just hunt` xác nhận mutant chết).

### F-011 — Mutant "replace + with -" ở wrap toroidal là tương đương, đã loại bằng skip
- **Ngày:** 2026-08-24
- **Loại:** SMELL
- **Vị trí:** `src-tauri/src/ai/pheromone.rs:195` (`let z_up = (z + GRID_SIZE - 1) % GRID_SIZE;`)
- **Phát hiện bởi:** mutant (`src/ai/pheromone.rs:190:23: replace + with - in update_pheromone_grid_system` sống sót qua quét shard, thấy trong `src-tauri/mutants.out/missed.txt`)
- **Triệu chứng:** với usize và overflow-checks tắt (mặc định profile release,
  thứ mà cargo-mutants dùng), `(z - GRID_SIZE - 1) % GRID_SIZE` cho kết quả
  **giống hệt** `(z + GRID_SIZE - 1) % GRID_SIZE` với mọi z ∈ [0,128): phép trừ
  underflow wrap thành `2^64 - (129 - z)` mà `2^64 ≡ 0 (mod 128)` và
  `-129 ≡ 127 (mod 128)`, nên sau `% GRID_SIZE` hai biểu thức trùng khít. Không
  tồn tại test hành vi nào phân biệt được — mọi giá trị thay thế đều cho cùng
  output lưới.
- **Tái hiện:** `cd src-tauri && cargo mutants --file src/ai/pheromone.rs --baseline skip`
  → mutant trên báo Missed.
- **Test khoá lại:** không có — không thể có, đây là equivalent mutant.
- **Đã xử lý:** `just skip 'pheromone\.rs:190:23: replace \+ with - in update_pheromone_grid_system' ...`
  thêm vào `exclude_re` của `src-tauri/.cargo/mutants.toml`.
- **BỔ SUNG (2026-08-24):** merge 157f85d đổi số dòng, biểu thức `z_up` giờ ở
  `pheromone.rs:195` nên regex cũ ghim `190:23` không còn khớp — mutant tương
  đương sẽ tái xuất hiện dạng Missed ở quét tươi. Đã thêm regex mới bằng lệnh
  chính thức `just skip 'pheromone\.rs:195:23: replace \+ with - in
  update_pheromone_grid_system' ...`. Regex cũ để nguyên trong toml (dead
  pattern, vô hại). Bài học: exclusion ghim theo số dòng dễ vỡ khi src đổi —
  các exclude sau này nên neo theo nội dung thay vì `file:line:col`.
- **Điều kiện gỡ ra:** GRID_SIZE không còn là luỹ thừa của 2; hoặc bật
  overflow-checks trong profile release/test; hoặc đổi sang số có dấu +
  `rem_euclid`. Khi đó mutant không còn tương đương và PHẢI bị giết bằng test.
- **Trạng thái:** OPEN (đã loại khỏi mutation gate, chờ điều kiện gỡ)

### F-012 — Merge 157f85d đồng bộ src theo origin/main nhưng bỏ sót 11+ suite test
- **Ngày:** 2026-08-24
- **Loại:** BUG
- **Vị trí:** 11 file trong `src-tauri/tests/`: flora_tests (39 chỗ dùng API đã
  xoá), flora_stress_boundary_tests (31), challenger_terrain_tests (18),
  milestone5_challenger_stress_tests (17), env_physics_tests (16),
  adversarial_terrain_challenger_tests (10), challenger_m5_settings_tests (10),
  milestone4_tests (9), terrain_tests (5), challenger_stress_tests (3),
  challenger_m6_weather_tests (2). Ngoài ra conflict marker còn nguyên trong
  `src-tauri/tests/common/allocator.rs:21-55` (đã giải trong session này).
- **Phát hiện bởi:** tay (`just test` exit 101)
- **Triệu chứng:** commit merge 157f85d thay toàn bộ `src-tauri/src` bằng phiên
  bản origin/main (xoá Flora/FloraSpawnSettings/spawn/grow/wither_flora_system,
  TerrainGrid → TerrainMap, EnvironmentState bị xoá/đổi tên, core::rng không
  còn được khai báo trong core/mod.rs dù file rng.rs vẫn tồn tại) nhưng KHÔNG
  cập nhật vùng test. Hệ quả: nhiều test binary không biên dịch được
  (`E0432/E0422/E0425/E0433`), `just test` exit 101, cổng verify luật 7 không
  thể qua. 11 file trên KHÔNG tồn tại trên origin/main — chúng là suite mồ côi
  của kiến trúc cũ. 4 file khác (meta_ai_stress_tests, challenger_meta_ai_tests,
  hrrl_tests, ecs_simulation_tests) đã đồng bộ về bản main (diff không xoá
  assertion nào).
- **Tái hiện:** `just test` → exit 101; ví dụ một lỗi:
  `tests/flora_stress_boundary_tests.rs:6` import
  `anima_engine_lib::core::ecs::Flora` → E0432.
- **Test khoá lại:** không áp dụng được — bế tắc cấu trúc: cổng verify cấm xoá
  assertion đã có (luật 3) nên không được xoá/gọt suite mồ côi; đồng thời cấm sửa
  `src-tauri/src` nên không được hồi sinh API cũ. Cần chủ sở hữu kit quyết định
  một trong hai hướng: (a) gỡ bỏ chính thức các suite mồ côi khỏi vùng guard
  kèm biên bản, hoặc (b) chấp nhận revert phần xoá flora/terrain API trong src.
- **Trạng thái:** OPEN — chờ quyết định kiến trúc phía owner.
- **QUYẾT ĐỊNH OWNER (2026-08-24):** chọn phương án (a) — xoá 11 suite mồ côi.
  Ủy quyền cho agent kèm tiêu chí bắt buộc (cả hai phải thoả đồng thời):
  (i) API được test đã biến mất khỏi `src-tauri/src`, và (ii) file không tồn tại
  trên nhánh gốc `origin/main`. Đã đối chiếu: cả 11 file thoả cả hai tiêu chí
  (`git cat-file -e origin/main:<path>` → KHÔNG tồn tại; số chỗ dùng API đã đếm
  ở trên). Việc xoá nằm trong commit riêng kèm mục FINDINGS này làm biên bản;
  ngoài 11 file này không được xoá/gọt bất kỳ assertion nào khác.
- **BỔ SUNG (2026-08-24):** quét mở rộng theo symbol phát hiện thêm ở vòng build
  sau (`get_default_presets`, `build_preset_genotype`,
  `process_preset_spawns_system`, `BevyPresetSpawnTrigger`, `BevyLineageTracker`,
  `commands::EnvironmentSettings`, `sync_environment_settings_system`,
  `receive_manual_disasters_system`, `BevyEnvironmentSettings`,
  `BevyManualDisaster`, `update_day_night_cycle_system` — đều xác nhận không còn
  trong `src-tauri/src`) cho thấy đúng 1 file thứ 12 cùng cảnh ngộ:
  `src-tauri/tests/organism_library_tests.rs`, không tồn tại trên origin/main →
  xoá theo cùng uỷ quyền và tiêu chí.


### F-010 — verify.sh chết im lặng (exit 1, không output) khi không có thay đổi nào trong thư mục tests
- **Ngày:** 2026-08-24
- **Loại:** BUG
- **Vị trí:** `scripts/verify.sh:41` (`GUARD_DIRS=$(...)`)
- **Phát hiện bởi:** tay
- **Triệu chứng:** dòng 41 là một phép gán qua pipeline
  `grep -E "$GUARD_RE" | sed | sort -u` mà KHÔNG có `|| true`. Khi working tree
  sạch (hoặc mọi thay đổi đều ngoài vùng tests), `grep` trả exit 1 vì không có
  dòng nào khớp; `set -eo pipefail` ở đầu script khiến nó thoát ngay tại đó,
  exit 1, không in bất kỳ thông báo nào — người chạy chỉ thấy
  `error: recipe 'verify' failed` từ just. Nghịch lý: cổng "chống gian lận"
  tự chết đúng khi agent KHÔNG gian lận (tree sạch).
- **Tái hiện:** trên working tree sạch: `just verify` → exit 1, log trống.
  Trace xác nhận: `bash -x ./scripts/verify.sh "" ""` dừng ngay sau `+ GUARD_DIRS=`.
- **Test khoá lại:** không áp dụng được (script thuộc hardening kit, cấm sửa theo
  AGENTS.md; cần người sở hữu kit sửa bằng cách thêm `|| true` hoặc xử lý rỗng
  tường minh trước khi gán `GUARD_DIRS`).
- **Trạng thái:** OPEN

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
- **Đã sửa:** tách `evolution::{genotype,mutation,crossover,map_elites}` ra crate
  `anima-core`. `ai` và `physics` KHÔNG tách được — cả hai dính `core::ecs`.
  Năm tham chiếu ngược của nhóm evolution nằm gọn trong một hàm `decode_genotype`,
  đã chuyển sang `core/spawn.rs`.
- **Kết quả đo được:**
  | | trước | sau |
  |---|---|---|
  | `cargo fuzz` | hỏng khi build | 191.792.110 lượt / 91 giây |
  | `cargo miri` | không chạy | chạy được |
  | mutation | 24 mutant / 55 phút | 135 mutant / 2 phút |
- **Trạng thái:** FIXED (b6c636b)

### F-008 — Không test nào khẳng định genotype hỏng bị TỪ CHỐI
- **Ngày:** 2026-08-11
- **Loại:** TEST-GAP
- **Vị trí:** `src-tauri/anima-core/src/mutation.rs:6` (`is_valid_genotype`)
- **Phát hiện bởi:** mutant (`replace is_valid_genotype -> bool with true` sống sót)
- **Triệu chứng:** mọi lời gọi `is_valid_genotype` trong toàn bộ test đều khẳng
  định `true` — không một chỗ nào khẳng định `false`. Hàm có thể luôn trả `true`
  mà cả suite vẫn xanh. Nguy hiểm thật chứ không chỉ thiếu điểm: `mutate_genotype`
  dùng chính hàm này để quyết định giữ hay hoàn tác một đột biến, nên hàm hỏng
  nghĩa là genotype không hợp lệ được nhận, rồi `decode_genotype` dựng sinh vật lỗi.
- **Tái hiện:** `cd src-tauri/anima-core && cargo mutants --file src/mutation.rs`
- **Test khoá lại:** `anima-core/tests/genotype_validity.rs` — 6 test, phủ cả 5
  điều kiện từ chối (rỗng, nhiều gốc, thành phần rời rạc, chu trình, cạnh trỏ tới
  node không tồn tại) kèm một ca đối chứng khẳng định `true`
- **Trạng thái:** FIXED — bản thân hàm vốn đúng, 6/6 test pass ngay lần đầu

### F-009 — Một phần mutant sống là equivalent do khoảng lấy mẫu đối xứng
- **Ngày:** 2026-08-11
- **Loại:** SMELL
- **Vị trí:** `anima-core/src/{mutation,crossover}.rs` — 10 chỗ dạng
  `x + rng.gen_range(-a..a)`
- **Phát hiện bởi:** mutant
- **Triệu chứng:** đổi `+` thành `-` không phân biệt được, vì khoảng lấy mẫu đối
  xứng quanh 0 nên hai biểu thức cho **cùng một phân phối**. Không test hành vi
  nào bắt được; chỉ test ghim giá trị theo seed mới bắt, mà test đó giòn và không
  khẳng định điều gì có nghĩa.
- **Ý nghĩa:** trần thực tế của mutation score **không phải 100%**. Trước khi ép
  score lên, phải phân loại mutant sống thành "thiếu test" và "equivalent" — nếu
  không sẽ đốt công vào việc bất khả thi.
- **Đã xử lý:** `exclude_re` trong `anima-core/.cargo/mutants.toml` kèm điều kiện
  gỡ ra (nếu khoảng lấy mẫu trở thành bất đối xứng)
- **Trạng thái:** OPEN
