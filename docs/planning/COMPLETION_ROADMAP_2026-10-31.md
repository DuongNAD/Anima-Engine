---
title: Nghiên cứu nâng cấp và hoàn thiện Anima Engine (đo 2026-10-31)
status: proposed
owner: maintainers
last_reviewed: 2026-10-31
review_cycle: monthly
---

# Nghiên cứu nâng cấp và hoàn thiện Anima Engine

> **Tài liệu này thay thế §1 của `COMPLETION_ROADMAP_2026-10-16.md`**
> — phần "công việc đang nằm ở đâu" — vì tình trạng nó mô tả đã **không còn tồn tại**: khối WIP mà
> tài liệu đó dặn *"không merge cả khối"* đã được đẩy nguyên khối lên `main` ngày 2026-10-22.
> Các giai đoạn **HT2–HT5** của tài liệu đó **vẫn đúng và vẫn là nguồn chuẩn**; chỉ phần mở đầu và
> thứ tự ưu tiên được viết lại ở đây.
>
> ⚠️ **Mọi tham chiếu tới `COMPLETION_ROADMAP_2026-10-16.md` ở đây là tham chiếu không có link, vì
> file đó chỉ tồn tại trên `origin/main` — nó chưa có trong cây này.** Đó không phải thiếu sót của
> tài liệu; đó là một triệu chứng của chính vấn đề được chẩn đoán ở §2: hai cây mã đã xa nhau tới
> mức kế hoạch và phép đo của cùng một dự án không còn nằm cạnh nhau được. Khi hai cây hợp nhất,
> đổi các tham chiếu này thành link tương đối và chạy `node scripts/check_docs_links.mjs`.
>
> §1 dưới đây là **đo hiện tại** theo quy tắc 9 của
> [chính sách tài liệu](../governance/DOCUMENTATION_POLICY.md): mỗi hàng có lệnh + ngày + cấu hình.
> Khi hai cây mã hợp nhất lại thành một, các số này phải được chuyển vào
> [`STATE_OF_THE_PROJECT.md` §1](STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền) thành
> một mục `§1.g` và tài liệu này trỏ về đó thay vì giữ bảng riêng.

---

## 0. Kết luận trong năm câu

1. Dự án **không thiếu tính năng** — nó thiếu **một cây mã duy nhất mà mọi gate cùng xanh trên đó**.
2. `main` đã **đỏ CI liên tục 10 ngày** (từ `a43872c`, 2026-10-21), và trong số các gate đỏ có
   `cargo test`.
3. Hai gate **tái lập** của adapter thí nghiệm sống đang **SAI trên `main`**: một nhánh fork tại
   tick 60 **không** trùng tiền tố với run không bị ngắt, và một run `save → load → chạy tiếp`
   **không** trùng với run liền mạch. Mọi khẳng định khoa học của dự án đứng trên hai gate đó.
4. Nhánh `feat/ecological-selection` — bị bỏ lại từ 2026-09-04, 68 commit chưa merge, 33 commit
   phía sau `main` — lại là cây **có hai gate đó xanh**, và là cây duy nhất **đã nối sinh thái vào
   hàm fitness** (F-018). `main` có **cái chết** nhưng **không** có chọn lọc; nhánh có **cả hai**.
5. Vì vậy việc đầu tiên không phải viết thêm mã, mà là **chọn một cây làm nền** và hợp nhất phần
   đúng của cây kia vào đó, từng cụm một, qua gate.

---

## 1. Đo hôm nay

### 1.a Nhánh làm việc `feat/ecological-selection` @ `c0344c9`

Cấu hình: macOS (Darwin 27.2, arm64) · rustc/cargo **1.96.0** · node **24.19.0** · chạy tuần tự,
máy rảnh.

| Loại | Gate | Lệnh | Kết quả |
|---|---|---|---|
| 📏 | Backend test | `cargo test --features desktop --no-fail-fast` | **exit 0** — **1153 passed · 0 failed · 2 ignored**, **107** dòng `test result`, 3 target `running 0 tests` |
| 📏 | ↳ Tái lập adapter sống | (trong lượt trên) `tests/live_experiment_tests.rs` | **17/17 ok** — gồm `a_fork_from_a_known_tick_shares_its_prefix_and_diverges_only_afterwards` và `run_k_then_save_load_then_run_the_rest_matches_an_uninterrupted_run` |
| 🔴 | Format | `cargo fmt --check` | **exit 1** — **67** khối `Diff in`, gần như toàn bộ trong các file test do `c0344c9` thêm |
| 🔴 | Advisory Rust | `cargo audit` | **exit 1** — **11 vulnerability** + `chacha20 0.10.1` yanked (1 allowed warning) |
| 🔴 | Lint frontend | `npm run lint` | **exit 1** — **8 error · 72 warning** |
| 🔴 | Ratchet ESLint | `node scripts/eslint_ratchet.mjs` | **exit 1** — 72 > baseline **0** |
| 🔴 | Chính sách target test | `node scripts/check_test_targets.mjs …` | **không parse được** — conflict marker ở `:78`, `:108`, `:155` (F-017, mở từ 2026-08) |
| 🔴 | Typecheck `tests/` | `npm run typecheck:tests` | **exit 1** — **27 lỗi TS** (phần lớn `TS6133` biến không dùng, cộng hai lỗi kiểu ở `phase3_adversarial`). Đo lại có/không bản sửa mock ở §1.e: **cùng 27 lỗi**, nên nó có trước |
| 📏 | Test frontend (`src/`) | `npm run test` | **exit 0** — **15 file · 113 passed** |
| 🔴 | Test frontend (`tests/`) | `npm run test:frontend` | **exit 1** — **6/47 file đỏ · 17/463 test đỏ · 8 error** |
| 📏 | Build | `npm run build` | **exit 0** |

**17 test đỏ + 2 file không collect được, chia đúng bốn cụm, và không cụm nào là lỗi mô phỏng:**

| Cụm | File | Số test đỏ | Nguyên nhân đã xác định |
|---|---|---|---|
| 1 | `map_viewport_stress.test.tsx` | **8** | **Đúng cái bẫy CLAUDE.md đã ghi**: `:54` dùng `vi.fn().mockImplementation(() => ({…}))`. Vitest in thẳng `The vi.fn() mock did not use 'function' or 'class'` và 8 lần `… is not a constructor`. Sửa arrow → `function` là xong cả file |
| 2 | `noticeInventory.test.ts` (5) + `thirdPartyLicenses.test.ts` (1) | **6** | `NOTICE` trên nhánh **chưa sinh lại** — thiếu hẳn mục npm (`expected 0 to be greater than 15`) và chuỗi `licensing/THIRD_PARTY_LICENSES.txt`. Bản sửa template generator nằm trên `main` (`363fd0e`), nhánh chưa có |
| 3 | `organism_library.test.tsx` | **3** | Test tìm `button` tên `"Thư viện Sinh vật"` mà UI không có, kèm cảnh báo `act(...)` — khẳng định viết trước phần được nối dây |
| 4 | `fantasy_map.test.tsx`, `fantasy_map_stress.test.tsx` | **0** (không collect được) | `Failed to resolve import "echarts-for-react" from "playground/fantasy-map/App.tsx"` — phụ thuộc npm không được khai |

Cụm 1, 2, 4 là **cơ học**: sửa dạng mock, sinh lại `NOTICE`, khai phụ thuộc (hoặc loại `playground/`
khỏi glob test). Chỉ cụm 3 cần quyết định: sửa UI cho khớp test, hay sửa test cho khớp UI.

### 1.e `playground/` bị `.gitignore` loại, nhưng bốn file test đã commit lại kiểm chính nó

Đây là phát hiện **mới**, không có trong roadmap 16/10, và nó giải thích cụm 3 và cụm 4 ở trên đúng
hơn cách tôi phân loại ban đầu.

`.gitignore:73` có `/playground/`. Dưới thư mục đó **đúng 1 file được track** (`RabbitVisualizer.tsx`,
mà `src/App.tsx:22` lazy-import — đường đó lành). Ba module còn lại **không được track**:

| Module (không track, bị gitignore) | File test đã commit kiểm nó |
|---|---|
| `playground/fantasy-map/MapViewport.tsx` | `tests/frontend/map_viewport_stress.test.tsx` |
| `playground/fantasy-map/App.tsx` | `tests/frontend/fantasy_map.test.tsx`, `fantasy_map_stress.test.tsx` |
| `playground/frontend/MockDataGenerator.ts` | `src/__tests__/MockDataGenerator.test.ts` |

**Bốn file test đã commit có đối tượng kiểm thử không nằm trong version control.** Chúng xanh trên
máy này vì file tồn tại cục bộ; trên một bản clone sạch — hoặc trên CI — chúng **không resolve nổi
import**. Trên `origin/main` cả bảy file (3 module + 4 test) đều **không tồn tại**, nên CI chưa bao
giờ gặp chuyện này; đây là nợ riêng của nhánh, do commit test cuối (`c0344c9`) sinh ra.

Vì vậy cụm 4 **không** sửa được bằng `npm i echarts-for-react`: cài một phụ thuộc cho mã bị
gitignore chỉ làm máy này xanh thêm. Việc thật là một **quyết định**: hoặc `playground/` trở thành
mã thật (track, khai phụ thuộc, vào phạm vi lint), hoặc bốn file test kia bị loại. Xem
[QĐ-D](#4-bốn-quyết-định-chỉ-chủ-dự-án-đưa-ra-được).

`playground/` cũng là nơi **7 trong 8 lỗi ESLint** nằm. Một thư mục `.gitignore` đã loại đang kéo đỏ
**ba** gate cùng lúc (lint, test, typecheck).

### 1.b `main` @ `a43872c` — CI đỏ, lượt `39644804767` (2026-10-21)

Đây là **lượt CI thật trên runner GitHub**, không phải đo lại trên máy này.

| Job | Gate đỏ | Gate xanh |
|---|---|---|
| Rust (`windows-latest`) | `cargo fmt --check` · `cargo audit` (**5** vuln) · `cargo deny check` · `cargo clippy --all-targets -D warnings` (**12** lint) · **`cargo test`** · "No test target ran zero tests" · **"Default build excludes Neo4j, the WebSocket stack and Burn/WGPU"** | parity binding ts-rs · license · SBOM · NOTICE · flora footprint |
| Frontend (`ubuntu-latest`) | `npm audit` (root) · `npm audit` (`tests/`) · **ratchet ESLint** | lint · typecheck · `test` · `test:frontend` · `build` · bundle closure · CSP · bundle budget · **`test:e2e`** · docs links |

Hai hàng đáng đọc kỹ hơn phần còn lại:

- **Gate tách feature (G2 #2) đỏ.** `STATE §2` xếp nó ở bậc *Live integrated* vì CI kiểm bằng
  `cargo tree` chứ không bằng "biên dịch được". Nó đỏ nghĩa là có một `use` vô điều kiện mới — đúng
  chế độ hỏng mà gate được viết ra để bắt, và nó đã bắt được.
- **`test:e2e` xanh nhưng đã bị thu hẹp.** `tests/package.json:8` trên `main` là
  `playwright test e2e/tiers/`. `ipc_contract`, `console_hygiene`, canonical views, phase3/phase6
  **không còn chạy**. Một màu xanh hẹp hơn hôm qua không phải một màu xanh.

### 1.c Hai gate tái lập đang SAI trên `main`

Trích nguyên văn từ log lượt `39644804767`:

```
assertion `left == right` failed: a control branch forked at tick 60 diverged from an uninterrupted run
  left: 1740163834
 right: 1989393324
test a_fork_from_a_known_tick_shares_its_prefix_and_diverges_only_afterwards ... FAILED

assertion `left == right` failed: a live run resumed from a checkpoint diverged from an uninterrupted
one; some piece of trajectory-relevant state is missing from the snapshot
  left: 1989393324
 right: 1740163834
test run_k_then_save_load_then_run_the_rest_matches_an_uninterrupted_run ... FAILED

test result: FAILED. 15 passed; 2 failed; 0 ignored
```

Hai test này **xanh trên nhánh** (§1.a). Và phép so sánh là **cùng một bộ, cùng kích cỡ**: cả hai
cây đều lên lịch **17** test ở target đó — `main` cho `15 passed; 2 failed`, nhánh cho
`17 passed; 0 failed`. Nhánh không xanh nhờ có ít test hơn.

Đây là dự báo đã đúng:
`COMPLETION_ROADMAP_2026-10-16.md` §4.1 nêu tên nghi phạm là
cụm A và nêu cả giả thuyết — anh chị em cùng `lineage_id` tạo *tie* trong một phép sort được
tuyên bố là tất định qua save/load. Mã trên `main` khớp giả thuyết đó từng dòng:

```rust
// src-tauri/src/core/agent_systems.rs — main @ a43872c
400:  let mut to_despawn: Vec<(String, Entity, &'static str)> = Vec::new();
408:  to_despawn.push((lineage_id.0.clone(), e, cause));
411:  to_despawn.sort_unstable_by(|a, b| a.0.cmp(&b.0));
```

Khoá sort là `lineage_id`, **không** là khoá duy nhất giữa anh chị em. `sort_unstable_by` không giữ
thứ tự các phần tử bằng nhau, nên thứ tự despawn phụ thuộc vào thứ tự lặp query — và thứ tự đó
không sống sót qua một vòng save/load. Hai checksum ở trên là hệ quả, không phải trùng hợp.

Dòng `400` và `408` còn vi phạm hợp đồng **zero-heap-allocation** mà `CLAUDE.md` tuyên bố cho tick
path: một `Vec` mới mỗi tick, cộng một `String` clone mỗi cá thể chết. Gate alloc không bắt, nên
không gate nào đỏ vì lý do đó.

### 1.d Ba khoản `main` đang mang mà roadmap 16/10 đã dặn gỡ trước khi hạ cánh

Đọc mã 2026-10-31 trên `origin/main`:

| Khoản | Neo | Hiện trạng |
|---|---|---|
| **Giá trị khoa học bịa** | `core/telemetry_export.rs:264-265` `herbivore_biomass = total_animals * 0.70` / `* 0.30`; `:271` `biomass: 10000.0`; `commands/simulation.rs` `RunProvenance::new(42, …)` | **còn nguyên**. Tỉ lệ 70/30 cố định, sàn dự phòng 10000, và seed `42` cứng được trình bày như *provenance* của một lần chạy |
| **Hai lệnh IPC không tồn tại** | `dispatch_causal_intervention`, `get_causal_branch_tree` | **còn nguyên**. Đối chiếu 17 call site `invoke(...)` ở `src/` với 34 `#[tauri::command]` ở `src-tauri/src`: đúng hai tên này không có bên Rust. Panel Research Dashboard gọi vào khoảng không |
| **Rò EU theo cấu trúc** | `core/live_experiment.rs` (`PathogenTransmission`) + `core/environmental_systems.rs:177` | **còn nguyên, và cơ chế đã rõ**: can thiệp ghi nợ `pool.animals` qua `energy.transfer(…Animals→Detritus…)`, nhưng census mỗi tick đặt lại `pool.animals = total` từ dự trữ của agent — mà **không agent nào bị trừ**. Khoản nợ bay hơi, khoản có ở detritus thì ở lại. Mỗi lần can thiệp nổ, `closed_total_eu` tăng |

Khoản thứ ba đáng nói riêng: **bảo toàn EU đóng bit-exact là khẳng định nền của dự án**
(`STATE §2`, bậc *Live integrated*). Một can thiệp đã ship có thể phá nó, và **không test nào bắn
`PathogenTransmission` rồi đo sổ cái** — các test có nhắc bốn loại can thiệp mới đều ở
`causal_branch_tree_tests.rs`, `adversarial_branch_tree_parity_tests.rs`,
`deterministic_replay_tests.rs`, tức parity và replay, không phải closure.

---

## 2. Chẩn đoán: cùng một mục tiêu, hai hiện thực, chỉ một tôn trọng hợp đồng

Cả `main` và nhánh đều nhắm vào **cùng một khoảng trống** mà
[`STATE §3.19`](STATE_OF_THE_PROJECT.md#319-đóng-vòng-sinh-thái--mục-p0-mới-mở-2026-08-25) mô tả:
sinh thái không nối vào chọn lọc, và không có cái chết. Hai cây đi hai đường.

| | `main` @ `a43872c` | `feat/ecological-selection` @ `c0344c9` |
|---|---|---|
| **Hàm fitness** | `agent_systems.rs:707` — `cumulative_distance + tick_count` · **F-018 chưa sửa** | `agent_systems.rs:554` — `rule.score(eval, &tracker, homeo, &genotype.0)` qua `evolution::fitness::selected_rule` · **F-018 đã sửa** |
| **Cái chết** | có — `agent_death_system`, `energy <= 0.0` | có — `core/mortality.rs`, có ngưỡng `starve_ticks` / `dehydration_ticks` |
| **Cấp phát trong tick** | `Vec::new()` + `String` clone mỗi tick | không — xếp `ReclaimAndDespawnAgentCommand` qua `Commands` |
| **Thứ tự tất định** | sort theo khoá **không duy nhất** | không sort; dùng nguyên thứ tự command queue |
| **Đường năng lượng** | `agent_systems.rs:522` — `pool.detritus += overhead_waste;` ghi **thẳng** vào pool, ngoài `total_cost` và ngoài `EnergyLedger`. Chú thích ngay trên nó viết *"0-leakage detritus routing"*, và số học **có thể** cân (`debit_reserve` trừ cha đúng `debited`, cộng `child_endowment` + `overhead_waste`) — nhưng sổ cái **không thấy** đường này, và hard rule của `CLAUDE.md` tồn tại chính vì một đường như vậy *"leaks EU while looking entirely reasonable"*. Đây là **bypass sổ cái đã xác định**, chưa phải rò đã chứng minh | **không thêm đường nào** — tái dùng primitive đã bank xác chết vào detritus (rule B của design) |
| **Tắt cờ ⇒ bit-exact** | không có cờ | có — thiếu `MortalityPolicy` hoặc `enabled == false` thì `return` trước khi chạm gì |
| **Vị trí trong lịch** | — | khai trong `build_tick_schedule`, sau `metabolic_decay_system`, trước `ecosystem_census_system`, có lý do viết kèm |
| **Hai gate tái lập** | 🔴 **đỏ** | ✅ **xanh** |

Nhánh làm đúng **bốn** điều hợp đồng đòi mà `main` không làm: không cấp phát trong tick path,
không thêm đường năng lượng ngoài sổ cái, tắt cờ là bit-exact, và nêu rõ vị trí trong lịch. Đó
không phải thẩm mỹ — đó chính là lý do hai gate tái lập còn xanh trên nhánh.

**Nhánh chỉ thêm 4 file nguồn** so với `main`: `core/mortality.rs`, `core/rng.rs`, `core/spawn.rs`,
`evolution/fitness.rs`. Nó **không** trùng lặp Map 2, Research Dashboard, sweep hay telemetry của
`main`. Hai cây **bù nhau**, không cạnh tranh — điều này làm việc hợp nhất khả thi hơn nhiều so với
vẻ ngoài "68 ahead / 33 behind".

**Điều nhánh thiếu** và `main` đã có: bản sửa F-017 (`286e45a`), template `gen_notice.mjs`
(`363fd0e`), nâng `rustls` (`0d224ec`), `rust-toolchain.toml` (`a0c1b09`), binding
`WorkloadContext.ts` sinh lại (`b021e44`).

---

## 3. Lộ trình, viết lại phần đầu

Giữ nguyên **HT2–HT5** của roadmap 16/10. Thay **HT0–HT1** của tài liệu đó bằng **HT0′–HT2′**
dưới đây, vì hiện trạng nó mô tả đã khác.

### HT0′ — Chọn nền và dừng chảy máu (ưu tiên tuyệt đối)

| ID | Việc | Định nghĩa xong |
|---|---|---|
| HT0′.1 | **Quyết định nền** ([QĐ-A](#4-ba-quyết-định-chỉ-chủ-dự-án-đưa-ra-được)): `main` là nền, nhánh hợp nhất vào — hay nhánh là nền cho tầng sinh thái. **Khuyến nghị: `main` là nền**, vì Map 2 / Dashboard / sweep đã ở đó và lớn hơn nhiều; nhưng **4 file của nhánh thay thế** phần demographics của cụm A trên `main`, không nằm cạnh nó | Một dòng quyết định ghi vào tài liệu này; không nhánh nào bị xoá trước khi ghi |
| HT0′.2 | **Hoàn nguyên hoặc sửa cụm A trên `main`** cho tới khi hai gate ở §1.c **xanh lại**. Đây là việc duy nhất chặn mọi thứ khác: trong lúc chúng đỏ, không phép đo nào trên `main` có nghĩa | `cargo test -p anima-engine --test live_experiment_tests` exit 0 **trên `main`**; checksum ghi vào PR |
| HT0′.3 | **PR sửa đỏ `main`**, hẹp và không kèm tính năng: `cargo fmt`, 12 lint clippy, `cargo deny`, advisory (`cargo audit` — 5 vuln trên `main`, cộng hai `npm audit`), ratchet ESLint về 0, và **gate tách feature** — tìm `use` vô điều kiện mới bằng `cargo tree --no-default-features -e normal` | CI xanh **cả hai job** trên `main` |
| HT0′.4 | **Hoàn nguyên việc thu hẹp `test:e2e`** về bộ đủ của CI, và thêm gate danh sách spec (HT0.4(d) của roadmap 16/10) | Bỏ một spec khỏi lệnh ⇒ gate đỏ và nêu tên spec |
| HT0′.5 | **Gỡ ba khoản ở §1.d**: xoá giá trị bịa (hoặc đánh dấu rõ là placeholder và chặn khỏi đường xuất dữ liệu); cài hai lệnh IPC còn thiếu **hoặc** gỡ panel gọi chúng; viết **test sổ cái đóng** cho `PathogenTransmission` **trước** rồi sửa nếu nó đỏ (quy tắc Q4: finding là giả thuyết tới khi có test đỏ) | Mỗi khoản: một test đỏ trước, xanh sau |

**Không mở tính năng mới khi `main` còn đỏ** — quy tắc này của roadmap 16/10 giữ nguyên, và nó vừa
bị vi phạm một lần với giá là 10 ngày đỏ.

### HT1′ — Hợp nhất nhánh sinh thái, từng cụm

Thứ tự này chọn theo **rủi ro tăng dần**, mỗi cụm một PR:

| Thứ tự | Cụm | Vì sao ở đây |
|---|---|---|
| 1 | Ba sửa cơ học của §1.a: arrow → `function` ở `map_viewport_stress.test.tsx`, sinh lại `NOTICE`, khai `echarts-for-react` (hoặc loại `playground/` khỏi glob) | Không chạm mô phỏng; đóng **14/17** test đỏ và cả 2 file không collect được |
| 2 | `evolution/fitness.rs` + cờ `FitnessRuleSetting` | **Đây là F-018, gốc của kết quả null ở E2.** Tắt cờ ⇒ bit-exact, nên rủi ro thấp mà giá trị cao nhất |
| 3 | `core/mortality.rs` + `MortalityPolicy`, **thay cho** demographics của cụm A | Phải sau HT0′.2, và phải đo hai gate tái lập trước/sau |
| 4 | `core/rng.rs`, `core/spawn.rs` | Hạ tầng, đi kèm 2–3 |
| 5 | Bộ test nhánh O / Q / H (observables, trục MAP-Elites, O(segment)) | Cần fmt sạch trước; `check_test_targets.mjs` phải sống lại trước khi tin số target |

Điều kiện chung mỗi cụm, lấy nguyên từ HT1.3 của roadmap 16/10: rustfmt, clippy, ratchet 0,
test xanh, **hai gate tái lập xanh**, không assertion nào bị nới, `test:e2e` đủ bộ.

### HT2′ — Rồi mới tới khoa học

Sau khi có **một** cây xanh: tiếp tục đúng **HT2** của roadmap 16/10 như đã viết ở đó. Một ghi chú
đã thay đổi: **HT2.1 (quần thể luân chuyển) nay rẻ hơn nhiều**, vì cụm 2 và 3 ở trên cung cấp đúng hai
mảnh nó cần — một fitness mà sinh thái chạm được, và một cái chết tất định. E3 chỉ nên chạy **sau**
đó, vì chạy trước là lặp lại E2: đo phương sai của bộ sinh số ngẫu nhiên.

---

## 4. Quyết định chỉ chủ dự án đưa ra được

| # | Quyết định | Vì sao máy không quyết được |
|---|---|---|
| **QĐ-A** | **Cây nào là nền**, và cụm A trên `main` bị hoàn nguyên hay bị sửa tại chỗ | Đánh đổi giữa việc mất công đã làm và việc giữ một hiện thực vi phạm bốn khoản hợp đồng. Khuyến nghị: nền là `main`, cụm A bị **thay** bởi `mortality.rs` của nhánh |
| **QĐ-B** | **Phạm vi `AGENTS.md`** — nó khai `src-tauri/src` là chỉ đọc và `src-tauri/tests` là vùng ghi duy nhất, ở phạm vi toàn repo, trong khi mọi việc ở HT0′–HT1′ bắt buộc ghi vào `src-tauri/src` | Đây là hợp đồng, không phải mã. Cần miễn trừ tường minh hoặc thu hẹp phạm vi. Mục tiên quyết này đã mở từ `STATE §3.19` và vẫn chưa được đóng |
| ~~**QĐ-D**~~ | ✅ **ĐÃ QUYẾT 2026-10-31 tại `26c8a96`: `playground/` là nháp.** Căn cứ là so hai viewport, không phải sở thích: `src/PixiViewport.tsx` (1296 dòng, track, `src/App.tsx:3` import, 7 file test) là *presentational* — nhận `zoom`/pan làm prop, **không** có wheel/drag/inertia/clamp. `playground/fantasy-map/MapViewport.tsx` (1182 dòng, không track) tự giữ tất cả, kể cả `Math.max(1.0, Math.min(5.0, …))` ở `:1108`. Nên 8 test tính chất kia **không có đối tượng tương ứng** ở bản ship — không port được, vì hành vi đó không ở đó. Đã xoá 4 file test và loại `playground` khỏi phạm vi ESLint | Đo: lint **8 error · 72 warning → 0 · 0**, ratchet xanh; `test:frontend` **17 đỏ → 9 đỏ** |
| **QĐ-E** | **`organism_library` — xây tính năng hay bỏ test** (mục mới, mở 2026-10-31). `tests/frontend/organism_library.test.tsx` (3 test đỏ) tìm một button `"Thư viện Sinh vật"` không có trong `App.tsx`, và gọi lệnh Tauri `spawn_preset_organism` **không tồn tại trong `src-tauri/src`**. Tức nó kiểm một tính năng **không có ở cả hai phía** — cùng hạng với hai lệnh IPC chết của Research Dashboard ở §1.d | Không đọc mã ra được: tuỳ chủ dự án có muốn Organism Library hay không. Đây là 3 trong 9 test đỏ còn lại |
| **QĐ-C** | **`hexf-parse`** — vẫn là hàng chặn phát hành duy nhất của §1.c trong STATE, và vẫn là quyết định pháp lý | Không có lời giải kỹ thuật: upstream chưa bao giờ publish văn bản licence cho bản 0.2.1 |

Hai việc **cần tay người** (không phải quyết định): chạy app desktop bản **release** với
`ANIMA_TICK_CAPTURE` ở 1.000 agent (HT4.1), và xác minh app khởi động dưới CSP. Cả hai vẫn mở.

---

## 5. Điều tài liệu này **không** khẳng định

- **Chưa đo `main` trên máy này.** §1.b đọc từ lượt CI `39644804767`; không chạy lại cục bộ, nên
  không loại trừ việc một gate đỏ ở đó đã được sửa bởi commit chưa push ở đâu đó.
- **Chưa chứng minh rò EU bằng một lần chạy.** §1.d mô tả cơ chế từ mã nguồn và nó khớp từng dòng,
  nhưng **chưa có test đỏ** — đó chính là việc HT0′.5.
- **Không khẳng định nhánh "tốt hơn".** Nhánh có hai gate tái lập xanh và F-018 đã sửa; nó cũng có
  67 khối fmt lệch, 11 advisory, lint đỏ và 17 test frontend đỏ. Nó tốt hơn **ở đúng bốn khoản
  hợp đồng** ở §2, không tốt hơn nói chung.
- **Không ước lượng lịch.** Các con số person-week ở roadmap 16/10 là *mục tiêu* và tài liệu này
  không gia hạn chúng; HT0′ chưa từng được ước lượng vì nó không tồn tại khi tài liệu đó viết.
