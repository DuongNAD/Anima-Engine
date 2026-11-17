---
title: Nghiên cứu nâng cấp và hoàn thiện Anima Engine (đo 2026-10-31)
status: proposed
owner: maintainers
last_reviewed: 2026-11-17
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
>
> **Cập nhật 2026-11-17 — hai cây đã hợp nhất, nên ba điều ở trên đã đổi.**
>
> - `main` được fast-forward lên `feat/ecological-selection` ngày 2026-11-16. Chẩn đoán "hai cây phân
>   kỳ" ở §2 và lộ trình HT0′–HT1′ ở §3 nay là **bản ghi lịch sử** của việc đã làm xong: T1 là #44, T2
>   (HT-C) là #45.
> - [`COMPLETION_ROADMAP_2026-10-16.md`](COMPLETION_ROADMAP_2026-10-16.md) nay có trong cây này. Các chỗ
>   nhắc tới nó bên dưới vẫn để dạng chữ như lúc viết; link ở dòng này là link chuẩn.
> - §1 dưới đây **không** được chuyển vào STATE. `§1.g` và `§1.h` của STATE là các lượt đo mới trên thân
>   cây (2026-11-16 và 2026-11-17), không phải bản chép của bảng này. §1 ở đây là **đo lịch sử** ngày
>   2026-10-31; số hiện hành ở [bảng mới nhất của STATE §1](STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).
> - Phần còn sống của tài liệu này là **§4, sổ quyết định của chủ dự án**. Tiến độ theo chặng ở
>   [STATE §3.00](STATE_OF_THE_PROJECT.md#300-việc-kế-tiếp--cập-nhật-2026-11-17).

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
[QĐ-D](#4-quyết-định-chỉ-chủ-dự-án-đưa-ra-được).

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
| HT0′.1 | **⚠️ Khuyến nghị trong ô này đã bị quyết ngược ngày 2026-11-01 — nền là `feat/ecological-selection`; xem [§4](#4-quyết-định-chỉ-chủ-dự-án-đưa-ra-được).** **Quyết định nền** ([QĐ-A](#4-quyết-định-chỉ-chủ-dự-án-đưa-ra-được)): `main` là nền, nhánh hợp nhất vào — hay nhánh là nền cho tầng sinh thái. **Khuyến nghị: `main` là nền**, vì Map 2 / Dashboard / sweep đã ở đó và lớn hơn nhiều; nhưng **4 file của nhánh thay thế** phần demographics của cụm A trên `main`, không nằm cạnh nó | Một dòng quyết định ghi vào tài liệu này; không nhánh nào bị xoá trước khi ghi |
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
| ~~**QĐ-A** (nửa "cây nào là nền")~~ | ✅ **ĐÃ QUYẾT 2026-11-01 bởi chủ dự án: `feat/ecological-selection` là nền; chồng PR #38→#39→#40 gộp VÀO nó.** Đây là **ngược** với khuyến nghị cũ ở chính ô này ("nền là `main`"), và khuyến nghị cũ được viết **trước khi** bề mặt hợp nhất được đo. Căn cứ đo, [2026-11-01-merge-surface-measurement.md](../ai/planning/2026-11-01-merge-surface-measurement.md): hai cây rẽ ở `043930c` và **không bao nhau** — `feat` hơn 102 commit (tuyến bằng chứng: `src-tauri/tests/` 153 lượt sửa, `FINDINGS.md` 27, hạ tầng mutation), chồng PR hơn 36 (tuyến CI-xanh + frontend: `src/components/` 57, artifact giấy phép). Gộp ba chiều **đối xứng về số xung đột** — 9 file, y nhau theo cả hai chiều — nên lựa chọn **không phải** chuyện tốn ít công hơn mà là **lịch sử nào thành thân cây**, và đó đúng là thứ chỉ chủ dự án quyết được. **Nửa còn lại của QĐ-A vẫn MỞ** (xem ô dưới) | Đo: 29 file tự gộp / 9 xung đột, cả 9 là `CONFLICT (content)`; đúng **một** là code (`genotype.rs`, 1 hunk, 14 dòng) và bản sai của nó **không biên dịch được**; nhưng **19 file `.rs` do cả hai nhánh sửa** và 18 tự gộp im lặng, gồm `simulation_schedule.rs` |
| ~~**QĐ-A′**~~ | ✅ **ĐÃ QUYẾT 2026-11-14 bởi chủ dự án**, theo cách đặt câu hỏi của [kế hoạch swarm 05/11 §2](../ai/planning/2026-11-05-swarm-master-plan.md) và [kế hoạch 13/11 §7](DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md): sau HT-C **giữ `core/mortality.rs::mortality_system` của thân cây**; `agent_systems.rs::agent_mortality_system` của `main` bị bỏ khỏi lịch. Câu hỏi gốc của ô này rộng hơn (hoàn nguyên hay sửa tại chỗ **cả** cụm A); phần ngoài mortality chưa được hỏi riêng. Văn bản gốc: **Cụm A trên `main` bị hoàn nguyên hay bị sửa tại chỗ** — nửa chưa quyết của QĐ-A | Vẫn là đánh đổi giữa mất công đã làm và giữ một hiện thực vi phạm bốn khoản hợp đồng; quyết định ngày 2026-11-01 chỉ chọn chiều gộp, không chọn việc này. Lưu ý một chỗ tài liệu này đã cũ: ô gốc viện `agent_death_system`, nhưng tên đó **không tồn tại** trong `src-tauri/src` — hàm thật là `agent_systems.rs::agent_mortality_system`. Đọc mã trước khi quyết |
| ~~**QĐ-B**~~ | ✅ **ĐÃ QUYẾT 2026-11-14 bởi chủ dự án: uỷ quyền theo thẻ.** `AGENTS.md` giữ nguyên làm mặc định (`src-tauri/src` chỉ đọc). Thẻ nào cần ghi vào `src-tauri/src` hoặc file R5 mang một dòng `AUTHORIZATION (owner, <ngày>)` theo mẫu §4 của [file prompt swarm](../ai/planning/2026-11-05-swarm-prompts.md), liệt kê **đúng** path, chỉ cho thẻ đó. Không có dòng đó thì thẻ không chạy. Văn bản gốc: **Phạm vi `AGENTS.md`** — nó khai `src-tauri/src` là chỉ đọc và `src-tauri/tests` là vùng ghi duy nhất, ở phạm vi toàn repo, trong khi mọi việc ở HT0′–HT1′ bắt buộc ghi vào `src-tauri/src` | Đây là hợp đồng, không phải mã. Cần miễn trừ tường minh hoặc thu hẹp phạm vi. Mục tiên quyết này đã mở từ `STATE §3.19` và vẫn chưa được đóng |
| ~~**QĐ-F028**~~ | ✅ **ĐÃ QUYẾT 2026-11-14 bởi chủ dự án: commit icon thật.** Thân cây đã có icon PNG/ICNS thật; `origin/fix/ci-gates-green` còn file text `placeholder` (F-028) nên không biên dịch trên macOS. Thẻ T1 được chép `src-tauri/icons/` từ `origin/feat/ecological-selection` sang nhánh của nó | Kiểm 2026-11-14: `file src-tauri/icons/32x32.png` trên thân cây cho `PNG image data, 32 x 32`; trên `fix/ci-gates-green` blob là 12 byte `placeholder\n` |
| ~~**QĐ-D**~~ | ✅ **ĐÃ QUYẾT 2026-10-31 tại `26c8a96`: `playground/` là nháp.** Căn cứ là so hai viewport, không phải sở thích: `src/PixiViewport.tsx` (1296 dòng, track, `src/App.tsx:3` import, 7 file test) là *presentational* — nhận `zoom`/pan làm prop, **không** có wheel/drag/inertia/clamp. `playground/fantasy-map/MapViewport.tsx` (1182 dòng, không track) tự giữ tất cả, kể cả `Math.max(1.0, Math.min(5.0, …))` ở `:1108`. Nên 8 test tính chất kia **không có đối tượng tương ứng** ở bản ship — không port được, vì hành vi đó không ở đó. Đã xoá 4 file test và loại `playground` khỏi phạm vi ESLint | Đo: lint **8 error · 72 warning → 0 · 0**, ratchet xanh; `test:frontend` **17 đỏ → 9 đỏ** |
| ~~**QĐ-E**~~ | ✅ **ĐÃ QUYẾT 2026-10-31: bỏ test.** `tests/frontend/organism_library.test.tsx` có đúng 3 test và **cả 3 đỏ**, nên xoá cả file không mất coverage nào. Tính năng vắng ở **cả hai phía**: `get_organism_presets` và `spawn_preset_organism` đều không có trong `src-tauri/src`, và không có `"Thư viện Sinh vật"` nào trong `src/`. Nếu sau này muốn Organism Library thì viết test lại cùng lúc với tính năng — đó là thứ tự TDD đúng, còn giữ một test không có đối tượng chỉ làm gate đỏ mãi | Đo: `test:frontend` **9 đỏ → 6 đỏ**; 6 cái còn lại đều là HT-A7 (cần Windows) |
| ~~**QĐ-C**~~ | ✅ **ĐÃ QUYẾT 2026-11-17 bởi chủ dự án: lối C, nâng `burn`** (xem §4.3). Hàng này vẫn chặn phát hành cho tới khi thẻ nâng `burn` merge và hồ sơ giấy phép được sinh lại. Văn bản gốc: **`hexf-parse`** — vẫn là hàng chặn phát hành duy nhất của §1.c trong STATE, và vẫn là quyết định pháp lý | Không có lời giải kỹ thuật: upstream chưa bao giờ publish văn bản licence cho bản 0.2.1 |

### 4.1 Quyết định sau 2026-11-14 (ghi thêm 2026-11-17)

Kế hoạch 13/11 (MP-0.1) đặt sổ quyết định ở mục này, nhưng các quyết định từ 2026-11-16 mới chỉ được ghi ở
[`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md) §1 và §4 và trong ADR. Bảng dưới gom chúng về một chỗ,
mỗi dòng trỏ tới nơi ghi đầy đủ. Nó không thêm quyết định nào.

Trừ QĐ-6 (khối `DECISIONS` không ghi ai quyết), mọi dòng trong bảng này do phiên lead quyết theo uỷ
quyền của chủ dự án trong chat. Dòng xác nhận của chủ dự án nằm ngay dưới bảng.

| # | Quyết định | Ngày | Ghi đầy đủ ở |
|---|---|---|---|
| QĐ-E2M | **B**: đính chính có ngày cho bản đăng ký E2, rồi chạy lại thật | 2026-11-16 | #59; `artifacts/experiments/e2-evolved-brain-default/RESULT.md` |
| QĐ-4 | **A**: sinh sản liên tục có điều kiện năng lượng trong adapter sống, sau khoá `live.reproduction` | 2026-11-16 | [ADR-0005](../decisions/ADR-0005-live-population-turnover.md) |
| QĐ-5 | **Có**, phạm vi chỉ là sinh sản liên tục trong thí nghiệm sống; `EvolutionaryReplacement` không đổi | 2026-11-16 | [ADR-0005](../decisions/ADR-0005-live-population-turnover.md) |
| QĐ-LOC | **A**: bộ lái ghi `target_velocity`, tốc độ lấy từ CPG/não của chính sinh vật | 2026-11-16 | [ADR-0006](../decisions/ADR-0006-locomotion-actuation.md) |
| QĐ-ECO | **A**: khoá opt-in cho sàn bộ lái và lái theo khát; thế giới mặc định không đổi | 2026-11-16 | [ADR-0007](../decisions/ADR-0007-live-ecology-sustainability.md) |
| QĐ-ECO-B | **Opt-in**: ba khoá thu nhập của thú săn, chỉ để thăm dò S43, không bao giờ mặc định | 2026-11-17 | [ADR-0007](../decisions/ADR-0007-live-ecology-sustainability.md), #87 |
| QĐ-2 / QĐ-H | Gỡ mọi bằng chứng và dữ liệu giả ngay (G4, G6), giữ mã; Map 2 và Research Dashboard có ship hay không thì quyết sau | 2026-11-17 | [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md) §1 |
| QĐ-6 | Hợp đồng IPC ở `docs/reference/IPC_CONTRACT.md` | 2026-11-14 | [`IPC_CONTRACT.md`](../reference/IPC_CONTRACT.md) |

**Chủ dự án xác nhận toàn bộ bảng trên ngày 2026-11-17, trong chat.** Từ dòng này, các quyết định đó là
quyết định của chủ dự án, không còn là "đang áp dụng theo uỷ quyền".

**Chủ dự án quyết thêm ngày 2026-11-17, trong chat** ("tất cả những đề xuất trên tôi đồng ý"), theo mười hai
câu hỏi của phiên rà soát tài liệu:

| # | Quyết định | Hệ quả |
|---|---|---|
| QĐ-SEL1b | **A**: thêm khoá trần sinh sản của con mồi, rồi tạo khan hiếm đồng cỏ; đo thăm dò trước khi đăng ký | Một bản đăng ký mới cho thí nghiệm chọn lọc theo khan hiếm. Chẩn đoán: [`2026-11-17-esel1-scarcity-diagnosis.md`](../ai/planning/2026-11-17-esel1-scarcity-diagnosis.md) |
| QĐ-ECO-C1 | **Có**, cả hai: thẻ `live.predator_cost_mult` + `live.strike_radius`, và một khoá nguồn nước. Opt-in; đo tiền đăng ký trên ít nhất 20 seed | [ADR-0007](../decisions/ADR-0007-live-ecology-sustainability.md), phương án C |
| QĐ-EN | **E2R** là lượt làm lại câu hỏi về não; **E3** là gradient năng suất | Hết nhập nhằng tên "E3" (kế hoạch 13/11 §12) |
| F-044 | **Tách** bộ đếm ân hạn đói và khát thành hai, **trước** khi đăng ký thí nghiệm kế tiếp | Đổi định dạng lưu và quỹ đạo của mọi cấu hình có hai mức ân hạn khác nhau; phải khai, xem [`FINDINGS.md`](../../FINDINGS.md) F-044 |
| QĐ-LAW1 | Thí nghiệm rẽ luật chạy **ngay sau thí nghiệm có sức mạnh đầu tiên, trước `v1.0.0`** | Phần AE4 mà thí nghiệm cần (trường năng lượng lạ trên thế giới sống) được kéo lên trước thẻ X9. Chi tiết ở §4.2 |
| QĐ-TIME | "Một ngày bằng 2 giờ thực" là **tốc độ xem**; lịch của thế giới là tham số khai báo | Chi tiết ở §4.2. Cần ADR trước khi có mã |
| QĐ-C | **C**: nâng `burn` lên dòng 0.22 để `hexf-parse` rời cây phụ thuộc và giữ được GPU. Không chọn A, không chọn B | Thẻ R1 của kế hoạch swarm không chạy; thay bằng thẻ nâng `burn`. Chi tiết ở §4.3 |
| QĐ-1 | **Mức 2**: bản 0.9 chạy một máy trước, rồi mới tới 1.0 | Thẻ R2; chặng 5 của kế hoạch 13/11 |
| #56, #68 | Đổi nhánh đích sang thân cây rồi merge | Luật L1. Cùng ngày: #56 đã merge; #68 đã đổi nhánh đích và được cập nhật theo thân cây, chờ CI chạy lại |
| Người ghi tài liệu | Phiên rà soát tài liệu được giao rà và sửa **toàn bộ** tài liệu trong đợt 2026-11-17, kể cả `STATE`, `TODO.md`, `FINDINGS.md` và trang hàng đợi thẻ. Ngoài đợt đó, bốn file này vẫn thuộc `agent-mac` | [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md) §1 |

Các thẻ mã sinh ra từ bảng này được định nghĩa ở
[kế hoạch swarm §3](../ai/planning/2026-11-05-swarm-master-plan.md#thẻ-thêm-ngày-2026-11-17-từ-quyết-định-của-chủ-dự-án).
Chưa thẻ nào trong số đó có dòng `AUTHORIZATION`: chủ dự án ghi dòng đó khi giao thẻ.

**Còn trống ngày 2026-11-17, xếp theo chặng cần tới** (câu hỏi và đề xuất ở
[kế hoạch 13/11 §7](DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md#7-hàng-đợi-quyết-định-của-chủ-dự-án), trừ
hai dòng cuối):

| Cần trước | Quyết định | Chặn |
|---|---|---|
| Chặng 0 (MP-0.6) | QĐ-M1 — tài liệu nào là `accepted` cho mỗi vai | Mọi tài liệu kế hoạch vẫn `proposed` |
| Chặng 3 | QĐ-M2 — tách crate trước hay sau `v1.0.0` | `CRATE_SPLIT_PLAN` |
| Chặng 4 hoặc 7 | QĐ-M6 — mức tất định giữa các nền tảng | NM-3 trên hai nền tảng, T5 |
| Chặng 5 | QĐ-P1, QĐ-P2, QĐ-P3 — có viết bài báo phương pháp không; nếu có thì loại bài, chỗ để bản nháp, khai báo dùng AI | Thẻ P1–P9 |
| Chặng 5 | QĐ-M4 — nền tảng phát hành thứ hai; QĐ-M8 — ký bản cài và tự cập nhật | MP-5.4, MP-5.5 |
| Chặng 7 | QĐ-M3, QĐ-M5, QĐ-M7 — lớp vận chuyển, `Inhabit` có là mục tiêu không, sửa thiết kế 25/08 | Nhánh F |
| Trước AE4 | QĐ-PAST — chỉ còn thiết kế lưu trữ | §4.2 ngay dưới |
| Không gắn chặng | `LIVE_MODEL_VERSION` có tăng sau #76 không; có bật vận động mặc định trong app không | STATE §3.00 P3 |

### 4.2 Ba mục về quá khứ, thời gian và rẽ luật (ghi 2026-11-17, theo lời chủ dự án trong chat)

Mục này ghi một nguyên tắc chủ dự án nêu trực tiếp và hai đề xuất mà chủ dự án đã nhận cùng ngày. Phần "đọc
mã" được đối chiếu trên thân cây ngày 2026-11-17; không lệnh build hay test nào được chạy, và mục này không
mang số đo. Cả ba cần một ADR trước khi có dòng mã nào.

| # | Nội dung | Trạng thái |
|---|---|---|
| **QĐ-PAST** | **Quá khứ của một thế giới đã chạy là bất biến.** Không sửa trạng thái ở một tick đã qua; chỉ một số trường hợp đặc biệt được phép. Lưu trữ phải đủ để dựng lại quá khứ và nhẹ nhất có thể | **Nguyên tắc và bảng ngoại lệ: chủ dự án quyết 2026-11-17**, đúng như đề xuất. Thiết kế lưu trữ **còn trống** |
| **QĐ-TIME** | Tách ba thứ: tick và vòng đời sinh vật; lịch của thế giới (bao nhiêu tick là một ngày, một năm); tốc độ xem. "Một ngày bằng 2 giờ thực" là **tốc độ xem**, không phải luật mô phỏng | **Chủ dự án quyết 2026-11-17**, đúng như đề xuất |
| **QĐ-LAW1** | Thí nghiệm "rẽ luật" đầu tiên trên thế giới sống: cùng bản đồ và seed, một nhánh đối chứng và một nhánh thêm một nguồn năng lượng lạ, tiền đăng ký, nhiều seed. Chạy **sau** khi chọn lọc trên thế giới sống đo được | **Chủ dự án quyết 2026-11-17**: chạy ngay sau thí nghiệm có sức mạnh đầu tiên, trước `v1.0.0`. Phụ thuộc bản đăng ký của QĐ-SEL1b và phần AE4 nó cần |

#### QĐ-PAST — cái đã khớp, cái chưa, phần đã chốt và đề xuất lưu trữ

**Đã khớp với nguyên tắc** (hợp đồng và mã hiện có):

- Luật thế giới bất biến trong một run; muốn đổi luật thì phải tạo một fork mới
  ([`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) ER01).
- Mỗi run mang `run_id`, `parent_run_id` và `fork_tick` (ER08). Fork từ một checkpoint tạo một run **mới**;
  run gốc không bị chạm.
- Sổ nhân quả là chỉ-ghi-thêm (`anima-domain/src/causal.rs`), và một can thiệp chỉ có hiệu lực từ tick nó
  khai trở đi.

**Chưa khớp hôm nay** (đọc mã):

- App desktop giữ đúng một `autosave.json`, ghi khi thoát và nạp lại khi mở. `save_simulation_state` ghi đè
  một save cùng tên. Một thế giới chạy trong app vì thế **không có lịch sử**: chỉ có trạng thái mới nhất.
- `LineageTracker::compact` bỏ các nhánh tuyệt chủng và genotype của node bị cắt. Đó là quên chi tiết, không
  phải sửa quá khứ. Theo bảng ngoại lệ đã chốt bên dưới, việc này được phép; điều bị mất đã được khai ở
  STATE §3.15.1.

**Đã chốt (chủ dự án, 2026-11-17, trong chat: "như đề xuất") — "quay lại quá khứ" chỉ có một nghĩa: fork.**
Tạo một thế giới mới từ một checkpoint cũ, với id riêng và `parent_run_id` trỏ về thế giới gốc. Thế giới gốc
giữ nguyên. Hai dòng thời gian cùng tồn tại nên không có nghịch lý. Các trường hợp đặc biệt:

| Trường hợp | Quyết định |
|---|---|
| Đổi định dạng save khi nâng phiên bản (migrate) | Cho phép: đổi cách ghi, không đổi nội dung. Đã là hành vi hiện có ([`SNAPSHOT_CONTRACT.md`](../reference/SNAPSHOT_CONTRACT.md) §5) |
| Xoá hẳn một thế giới | Cho phép, cả thế giới, không xoá từng đoạn |
| Nén lưu trữ làm mất chi tiết (ví dụ nén phả hệ) | Cho phép nếu sự kiện đã ghi không đổi và điều bị mất được khai trước |
| Một save hỏng | Không sửa tại chỗ. Fork từ checkpoint lành gần nhất |
| Sửa trạng thái ở một tick đã qua rồi chạy tiếp như cùng một thế giới | **Không bao giờ** |

**Đề xuất, chưa chốt — lưu trữ đủ mà nhẹ: lưu nguyên nhân, không lưu mọi trạng thái.** Vì thế giới tất định, quá khứ
dựng lại được từ ít dữ liệu hơn nhiều so với lưu từng tick:

1. Manifest, seed và fingerprint của luật: ghi một lần.
2. Một nhật ký sự kiện chỉ-ghi-thêm: can thiệp và hành động của người quan sát, mỗi mục có tick và `CauseId`.
3. Checkpoint thưa làm mốc, để không phải phát lại từ khởi nguyên.
4. Checksum của thế giới ở các mốc định kỳ, để kiểm một lần phát lại có đúng không.
5. Mỗi bản ghi mang checksum của bản trước nó, nên sửa một mục trong quá khứ sẽ làm gãy chuỗi và lộ ra.

Hai giới hạn phải ghi cùng đề xuất. Phát lại chỉ đúng trên **cùng bản build và cùng lớp nền tảng**
([`DETERMINISM_CONTRACT.md`](../reference/DETERMINISM_CONTRACT.md) §1), nên checkpoint thưa vẫn cần để một
bản build mới đọc được thế giới cũ. Và đường app mặc định **chưa** tất định (uuid thật, đồng hồ thật, AI
ngoài), nên một thế giới muốn có quá khứ dựng lại được phải chạy ở `DeterministicMode`. Dung lượng của cách
lưu này chưa ai đo.

**Nhiều thế giới không cần phần cứng khác.** Mỗi thế giới là một run với manifest và seed riêng. `anima-lab`
chạy nhiều thế giới trên một máy (ensemble nhiều seed, `sweep` có `--threads`, `branch`). App desktop hiện
giữ một engine, tức một thế giới tại một thời điểm. Máy thứ hai chỉ cần khi muốn chạy song song nhiều hơn
một máy kham được. Khi đó cùng một seed trên hai nền tảng khác nhau **không** hứa cho cùng một thế giới, nên
các thế giới đem so với nhau phải chạy trên cùng lớp nền tảng cho tới khi có QĐ-M6.

#### QĐ-TIME — vì sao 2 giờ một ngày không nên là luật

Hợp đồng thời gian hiện tại ([`SIMULATION_RULES.md`](../../SIMULATION_RULES.md) §4) là 60 tick mỗi giây, một
epoch 1.000 tick và một năm mô phỏng 6.000 tick. Mô phỏng chưa có đơn vị "ngày"; chu kỳ ngày–đêm của cảnh 3D
là đồng hồ hiển thị (`sceneClock.ts`). Ở 60 tick mỗi giây, 2 giờ thực là 432.000 tick, tức 72 năm mô phỏng
theo hằng số hôm nay, và chứa hàng trăm epoch. Nếu kéo vòng đời sinh vật dài ra cho hợp với một ngày như
vậy, một thế hệ sẽ mất nhiều ngày thực và tiến hoá không còn xem được.

Đề xuất: giữ tick và vòng đời ngắn; đưa lịch thành tham số khai báo của thế giới (vắng thì như cũ); để người
xem chọn tốc độ, kể cả 2 giờ một ngày; thí nghiệm chạy không cửa sổ, nhanh hết mức. Đưa chu kỳ ngày–đêm vào
luật (ánh sáng, nhiệt) là đổi luật mô phỏng: cần sửa `SIMULATION_RULES.md` kèm test trong cùng thay đổi.

#### QĐ-LAW1 — vì sao phải sau chọn lọc

Khung cho việc đổi một yếu tố của thế giới đã có: ADR-0002 và
[`ALTERNATE_EVOLUTIONARY_REGIMES.md`](../explanation/ALTERNATE_EVOLUTIONARY_REGIMES.md). "Linh khí", "mana"
hay một loại khí mới đều là tên hiển thị của cùng một thứ trong mã: một trường năng lượng ngoại lai có ngân
sách riêng, tác động qua hấp thụ có chi phí, không sửa thẳng gen hay fitness (ER03). Hai chỗ còn thiếu:

- Thế giới sống chưa có trường đó; adapter của nó từ chối `laws.exotic_energy`. Nối sang là AE4, kế hoạch
  swarm xếp là X9, sau `v1.0.0`. Chủ dự án đã quyết đổi thứ tự đó: phần AE4 mà thí nghiệm cần được làm
  trước `v1.0.0`.
- Đổi luật chỉ tạo ra hướng tiến hoá khác khi chọn lọc hoạt động. Hôm nay tử vong, fitness sinh thái và sinh
  sản liên tục tắt mặc định; E2 null; E-SEL1 đóng mà không có thí nghiệm (STATE §3.00, §3.1). Thêm một nguồn
  năng lượng lúc này nhiều khả năng cho thêm quỹ đạo, chưa chắc cho thêm kiểu kết cục.

### 4.3 QĐ-C — một lối thứ ba: nâng `burn` thì `hexf-parse` tự biến mất (kiểm 2026-11-17)

Chủ dự án nói trong chat ngày 2026-11-17 rằng muốn **giữ GPU**, rồi **chọn lối C** cùng ngày. Mục này ghi
dữ kiện đã dẫn tới lựa chọn đó.

**Vấn đề pháp lý là gì, nói cho đúng cỡ.** Dùng GPU không phải vấn đề pháp lý. Vấn đề là **một** thư viện
nhỏ, `hexf-parse` 0.2.1, khai giấy phép CC0-1.0 nhưng chưa từng phát hành kèm văn bản giấy phép, trong khi
gate của chính dự án đòi mọi thành phần được phân phối phải có văn bản gốc
([`licensing/UNRESOLVED.md`](../../licensing/UNRESOLVED.md)). CC0 không đặt điều kiện ghi công, nên đây là
một khoảng trống hồ sơ do quy tắc của dự án tự đặt ra, không phải một tranh chấp.

**Dữ kiện mới.** `hexf-parse` vào bản build qua đúng một đường:

```text
hexf-parse 0.2.1 <- naga 0.19.2 <- wgpu 0.19.4 <- burn-wgpu 0.13.2 <- anima-engine (feature ml-wgpu)
```

Đường này lấy từ `cargo tree --features desktop -e normal -i hexf-parse` trong `src-tauri/`. Tra danh sách
phụ thuộc đã khai trên crates.io cùng ngày (`https://crates.io/api/v1/crates/<tên>/<bản>/dependencies`):

| Crate | Bản | Còn phụ thuộc `hexf-parse`? |
|---|---|---|
| `naga` | 22.1.0 đến 29.0.1 (tám bản đã tra) | Có |
| `naga` | 30.0.2 | **Không** |
| `burn-wgpu` | 0.22.1, bản ổn định mới nhất | Đi qua `cubecl-wgpu` 0.11.4, vốn đòi `wgpu ^30` |

Nghĩa là nâng `burn` từ 0.13.2 lên dòng 0.22 sẽ kéo `naga` lên 30 và `hexf-parse` rời khỏi cây phụ thuộc,
không cần quyết định pháp lý nào. **Chưa kiểm:** đây là danh sách phụ thuộc đã khai, chưa phải một lần build
thật của dự án ở bản mới; và chưa ai đo bản `wgpu` mới có còn rò bộ nhớ hay không.

**Ba lựa chọn sau dữ kiện này (chủ dự án chọn C):**

| | Nội dung | Giữ GPU trong bản cài? | Cái giá |
|---|---|---|---|
| A | Bỏ `ml-wgpu` khỏi feature `desktop` | Không; chỉ khi tự build | Rẻ nhất. Mã GPU vẫn còn |
| B | Chủ dự án ghi một quyết định: nhận khai báo CC0-1.0 trên crates.io là đủ | Có | Là quyết định pháp lý của chủ dự án. Đường GPU vẫn rò bộ nhớ |
| C | Nâng `burn` lên dòng 0.22 | Có | Chín bản minor, làm vỡ `ai/model.rs` và `core/training.rs` (`CLAUDE.md` ghi 0.14 đã vỡ). Chạm learner nhạy số, nên phải giữ gate parity EB-S02 và `a2c_loss_direction_tests` xanh. Cần thẻ riêng có uỷ quyền ghi vào `src-tauri/src` |

C là lối duy nhất có thể đóng cả hai việc cùng lúc: khoảng trống giấy phép, và chỗ rò bộ nhớ của `wgpu-core`
0.19.4 (ghi chú ở `core/resources.rs`). Ba cách sửa chỗ rò đã được ghi ở PR #30: mặc định CPU (đã làm), nâng
`burn`/`wgpu`, hoặc cache layout bên trong `burn-wgpu`. Dù chọn gì, lượt chạy khoa học vẫn dùng CPU theo hợp
đồng tất định.

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
